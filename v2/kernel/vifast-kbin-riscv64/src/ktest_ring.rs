// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-69** — IDL + ring chia sẻ trên ảnh `vifast-kbin-ktest-user` (TAP 25..29).
//!
//! | TAP | Điều kiện đo |
//! |---|---|
//! | `ok 25 - idl stub sinh tu vidl` | hai tiến trình U-mode **riêng** (`ring_server.elf`, `ring_client.elf`) nói giao thức **IDL sinh từ `ring.vidl`** (`attach` mang **cap Frame**, `sync`, `finish`); kernel đối chiếu method id/label đã thấy với hằng sinh (`vifast_idl::METHOD_*`), `caps` nhận đúng 1, cả hai `Exit(0)` |
//! | `ok 26 - ring day` | producer gửi tới **đầy** (8 slot) rồi `send` thứ 9 trả **mã 9** (`NotEnoughMemory`), `head` không tăng, **không chặn**; `head` lúc đầy = sức chứa |
//! | `ok 27 - ring rong` | consumer khi ring **rỗng** (trước lô đầu) `recv` trả **mã 14** (`Busy`), `tail` không đổi, **không chặn** |
//! | `ok 28 - ring thu tu va wrap` | 40 message (5 vòng wrap), `seq` 0..39 **đúng thứ tự**, không mất/nhân đôi/torn, tổng payload hai phía **bằng nhau**, header `check` xanh |
//! | `ok 29 - ring race co kiem soat` | hai bên chạy **xen kẽ** theo scheduler (≥10 bước mỗi bên), ring từng **đầy** khi cả hai còn sống (`peak_used=8`), mapping **hai VSpace cùng PA**, cap/ipc/objmem/byte bất biến, chuyển cap `installed=1 failed=0` |
//!
//! Bằng chứng `# idl:`/`# ring:` do **kernel** in từ biến đo (D2-102): header/slot đọc qua
//! **physmap** (object layer), stats do hai chương trình ghi vào Frame chia sẻ, cap/PTE đọc từ
//! `Machine`/object layer — không tin lời chương trình. Hằng **thiết kế** in kèm nhãn `const:`.
//!
//! Không `unsafe` trong tệp này (R5): mọi truy cập core/object layer qua API an toàn.

use core::fmt::Write;

use vifast_core::vm::{CapId, ObjKind};
use vifast_core::{KError, Machine, NonReadyTarget, ObjId, ObjType, SlotRef, TcbState};
use vifast_plat_common::PlatformWriter;
use vifast_plat_riscv64::core_bridge;
use vifast_plat_riscv64::mem_bridge;
use vifast_plat_riscv64::obj as aobj;
use vifast_plat_riscv64::trap::{TrapFrame, REG_SP};
use vifast_plat_riscv64::user::{self, Stop};
use vifast_plat_riscv64::Riscv64Virt;
use vifast_pte::sv39::{PTE_READ, PTE_USER, PTE_WRITE};
use vifast_ring::{Mem, Ring, Stat, PAYLOAD_OFF};

static PLATFORM: Riscv64Virt = Riscv64Virt;

/// Hai ELF U-mode **riêng** do `build.rs` sinh từ nguồn dùng stub IDL + ring (KT-69).
static RING_SERVER_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/ring_server.elf"));
static RING_CLIENT_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/ring_client.elf"));

// ---------------------------------------------------------------------------------------------
// Bố cục CSpace (khớp hằng trong hai chương trình U-mode)
// ---------------------------------------------------------------------------------------------

/// CSpace **server** (producer) — 16 slot.
const S_SELF: usize = 0;
const S_EP_SEND: usize = 1;
const S_EP_RECV: usize = 2;
const S_VSPACE: usize = 3;
const S_FRAME: usize = 5;
/// CSpace **client** (consumer) — 16 slot; `C_RECV` = ô đầu receive window.
const C_SELF: usize = 0;
const C_EP_RECV: usize = 1;
const C_EP_SEND: usize = 2;
const C_VSPACE: usize = 3;
const C_RECV: usize = 4;
/// Slot tạm của rig trong cspace gốc — 25..28 (capa đã xoá cap tạm của nó ở cuối TAP 24;
/// cspace gốc chỉ 32 slot nên **không** dùng 32).
const TMP_TCB_S: usize = 25;
const TMP_TCB_C: usize = 26;
const TMP_SC_S: usize = 27;
const TMP_SC_C: usize = 28;
/// Ô bảng dải user tái dùng sau KT-67b (capx giải phóng VSpace của slot 1/2).
const LOAD_SLOT_S: usize = 1;
const LOAD_SLOT_C: usize = 2;

/// VA ring trong VSpace của **từng** tiến trình (khác VSpace, cùng PA Frame).
const VA_RING: u64 = 0x4200_0000;
/// Hình học thiết kế của ring (`const:` trong bằng chứng — D2-102a).
const RING_CAP: u32 = 8;
const RING_SLOT_BYTES: u32 = 32;
/// Số message tổng (5 vòng wrap với 8 slot).
const RING_TOTAL: u64 = 40;
/// Giá trị payload thứ `n` = `n + 1` ⇒ tổng 1..=40 = 820.
const RING_SUM: u64 = 820;
/// Số bước scheduler tối đa (chặn treo im lặng).
const MAX_STEPS: usize = 400;

/// Rig của pha ring.
struct Rig {
    s: ObjId,
    c: ObjId,
    tcb_s: ObjId,
    tcb_c: ObjId,
    frame_obj: ObjId,
    frame_pa: u64,
    server_vspace: CapId,
    client_vspace: CapId,
}

/// Đọc Frame qua **physmap** (kernel) — impl [`Mem`] chỉ đọc; ghi là no-op (kernel không ghi
/// Frame của user trong pha này).
struct PhysMem {
    pa: u64,
}

impl Mem for PhysMem {
    fn load(&self, offset: usize) -> u64 {
        aobj::machine()
            .read_phys_u64(self.pa + offset as u64)
            .unwrap_or(0)
    }
    fn store(&self, _offset: usize, _value: u64) {}
    fn acquire(&self, offset: usize) -> u64 {
        self.load(offset)
    }
    fn store_rel(&self, _offset: usize, _value: u64) {}
}

/// Đưa TCB `Inactive/Ready` → `Running{hart:0}` (như `ktest_capa`).
fn set_running(m: &mut Machine, tcb: ObjId) -> bool {
    if matches!(m.tcb_state(tcb), Ok(TcbState::Running { .. })) {
        return true;
    }
    if m.tcb_state(tcb) == Ok(TcbState::Inactive) && m.start_tcb(tcb).is_err() {
        return false;
    }
    m.transition_tcb(tcb, NonReadyTarget::Running { hart: 0 })
        .is_ok()
}

fn task_ready(m: &mut Machine, tcb: ObjId) {
    // **D2-187/D2-194** — không còn cửa "đặt `Ready` vô điều kiện": chọn cửa khớp trạng thái
    // nguồn. `wake` là API **test-only** (D2-144) và đúng ngữ nghĩa cho `BlockedOn*`/`Sleeping`.
    let _ = match m.tcb_state(tcb) {
        Ok(TcbState::Inactive) => m.start_tcb(tcb),
        Ok(TcbState::Ready) => Ok(()),
        Ok(TcbState::Running { .. }) => m.tcb_yield(tcb),
        _ => m.wake(tcb),
    };
}

/// `true` ⇔ TCB đang chờ (không chạy được ở bước này) — pha này **không** dùng IPC chặn nên
/// nhánh này chỉ là lưới an toàn; gặp là TAP đỏ.
fn blocked(state: TcbState) -> bool {
    !matches!(
        state,
        TcbState::Inactive | TcbState::Ready | TcbState::Running { .. }
    )
}

fn cap_info_at(m: &Machine, cnode: ObjId, slot: usize) -> Option<vifast_core::CapInfo> {
    m.cap_at(cnode, slot).ok().and_then(|c| m.cap_info(c).ok())
}

/// Dựng rig: hai CNode + hai Endpoint + hai TCB + cap Frame (object layer) + VSpace hai bên.
fn build_rig(
    root: ObjId,
    us: SlotRef,
    ctl: SlotRef,
    load_untyped: CapId,
    stage: &mut &'static str,
) -> Option<(Rig, user::LoadedInit, user::LoadedInit)> {
    *stage = "load server";
    let server = match user::load_init_slot(RING_SERVER_ELF, load_untyped, LOAD_SLOT_S) {
        Ok(loaded) => loaded,
        Err(error) => {
            *stage = error.name();
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(w, "# ring: load server err={:?}", error);
            return None;
        }
    };
    *stage = "load client";
    let client = match user::load_init_slot(RING_CLIENT_ELF, load_untyped, LOAD_SLOT_C) {
        Ok(loaded) => loaded,
        Err(error) => {
            *stage = error.name();
            return None;
        }
    };
    *stage = "vspace keys";
    let server_vs_key = {
        let m = aobj::machine();
        let id = m.cap_key(server.vspace).ok()?;
        mem_bridge::make_key(server.vspace, id)
    };
    let client_vs_key = {
        let m = aobj::machine();
        let id = m.cap_key(client.vspace).ok()?;
        mem_bridge::make_key(client.vspace, id)
    };
    *stage = "object frame";
    let (frame_key, frame_pa) = {
        let m = aobj::machine();
        let cap = m.retype(load_untyped, ObjKind::Frame, 12).ok()?;
        match (m.cap_key(cap), m.frame_pa(cap)) {
            (Ok(id), Ok(pa)) => Some((mem_bridge::make_key(cap, id), pa)),
            _ => None,
        }
    }?;
    *stage = "core rig";
    let rig = core_bridge::with_machine(|m| -> Option<Rig> {
        let s = m.add_cnode(16);
        let c = m.add_cnode(16);
        *stage = "core root self";
        let s_self = m.root_cap(s);
        m.put_cap(s, S_SELF, s_self).ok()?;
        let c_self = m.root_cap(c);
        m.put_cap(c, C_SELF, c_self).ok()?;
        *stage = "core endpoints";
        // Hai Endpoint **một chiều**: server→client (A) và client→server (B). Dùng chung một
        // Endpoint hai chiều sẽ để bên gửi đọc lại message của chính nó (hàng đợi chung).
        let ep_send = m.retype(us, ObjType::Endpoint, 0, 0).ok()?;
        let ep_recv = m.retype(us, ObjType::Endpoint, 0, 0).ok()?;
        m.put_cap(s, S_EP_SEND, ep_send).ok()?;
        m.put_cap(s, S_EP_RECV, ep_recv).ok()?;
        m.copy(SlotRef::new(s, S_EP_SEND), SlotRef::new(c, C_EP_RECV))
            .ok()?;
        m.copy(SlotRef::new(s, S_EP_RECV), SlotRef::new(c, C_EP_SEND))
            .ok()?;
        *stage = "core frame obj";
        // Frame của object layer (D2-83) — cap đại diện trong CSpace **server**.
        let frame_obj = m.add_foreign_object(ObjType::Frame, frame_key).ok()?;
        let frame_cap = m.root_cap(frame_obj);
        m.put_cap(s, S_FRAME, frame_cap).ok()?;
        *stage = "core vspace s";
        // VSpace **của từng tiến trình** — cap trong cspace của chính nó (Frame.Map cần).
        let vs_s = m.add_foreign_object(ObjType::VSpace, server_vs_key).ok()?;
        let vs_s_cap = m.root_cap(vs_s);
        m.put_cap(s, S_VSPACE, vs_s_cap).ok()?;
        let vs_c = m.add_foreign_object(ObjType::VSpace, client_vs_key).ok()?;
        let vs_c_cap = m.root_cap(vs_c);
        m.put_cap(c, C_VSPACE, vs_c_cap).ok()?;
        *stage = "core tcb loop";
        // Hai TCB có cspace riêng + SC + độ ưu tiên.
        let mut tcb_objs = [0u32; 2];
        for (index, (tcb_slot, sc_slot, cnode)) in
            [(TMP_TCB_S, TMP_SC_S, s), (TMP_TCB_C, TMP_SC_C, c)]
                .into_iter()
                .enumerate()
        {
            *stage = "core tcb retype";
            let tcb_cap = match m.retype(us, ObjType::Tcb, 0, 0) {
                Ok(cap) => cap,
                Err(error) => {
                    let mut w = PlatformWriter(&PLATFORM);
                    let _ = writeln!(w, "# ring: tcb retype err={:?} index={index}", error);
                    return None;
                }
            };
            let tcb = m.cap_obj(tcb_cap).ok()?;
            m.put_cap(root, tcb_slot, tcb_cap).ok()?;
            *stage = "core sc retype";
            let sc = match m.retype(us, ObjType::SchedContext, 0, 0) {
                Ok(cap) => cap,
                Err(error) => {
                    let mut w = PlatformWriter(&PLATFORM);
                    let _ = writeln!(w, "# ring: sc retype err={:?} index={index}", error);
                    return None;
                }
            };
            m.put_cap(root, sc_slot, sc).ok()?;
            *stage = "core tcb configure";
            m.tcb_configure(SlotRef::new(root, tcb_slot), Some(SlotRef::new(cnode, 0)))
                .ok()?;
            *stage = "core sc configure";
            m.sched_control_configure(ctl, SlotRef::new(root, sc_slot), 500, 5_000)
                .ok()?;
            m.set_sched_params(
                SlotRef::new(root, tcb_slot),
                Some(SlotRef::new(root, sc_slot)),
            )
            .ok()?;
            m.tcb_set_priority(SlotRef::new(root, tcb_slot), 3, 10)
                .ok()?;
            tcb_objs[index] = tcb;
        }
        Some(Rig {
            s,
            c,
            tcb_s: tcb_objs[0],
            tcb_c: tcb_objs[1],
            frame_obj,
            frame_pa,
            server_vspace: server.vspace,
            client_vspace: client.vspace,
        })
    });
    rig.map(|rig| (rig, server, client))
}

/// Một tác vụ U-mode chạy tiếp được (giữ trapframe qua từng `Yield`).
struct Task<'a> {
    loaded: &'a user::LoadedInit,
    tf: TrapFrame,
    tcb: ObjId,
    slot: usize,
    exited: bool,
    code: u64,
}

impl<'a> Task<'a> {
    fn new(loaded: &'a user::LoadedInit, tcb: ObjId, slot: usize) -> Self {
        let mut tf = TrapFrame::zeroed();
        tf.x[REG_SP] = user::USER_STACK_TOP;
        tf.sepc = loaded.entry;
        Self {
            loaded,
            tf,
            tcb,
            slot,
            exited: false,
            code: u64::MAX,
        }
    }
}

/// Chạy hai tác vụ **xen kẽ** (mỗi lượt tới `Yield`/`Exit`), chịu được `Yield` nhiều vòng.
fn run_pair(rig: &Rig, server: &user::LoadedInit, client: &user::LoadedInit) -> (u64, u64, usize) {
    let mut tasks = [
        Task::new(server, rig.tcb_s, LOAD_SLOT_S),
        Task::new(client, rig.tcb_c, LOAD_SLOT_C),
    ];
    user::enable_multitask(true);
    let mut steps = 0usize;
    while steps < MAX_STEPS && !tasks.iter().all(|task| task.exited) {
        let mut progressed = false;
        for task in tasks.iter_mut() {
            if task.exited {
                continue;
            }
            let state = core_bridge::with_machine(|m| m.tcb_state(task.tcb).ok());
            if state.map(blocked).unwrap_or(true) {
                continue;
            }
            steps += 1;
            progressed = true;
            user::set_active_slot(task.slot);
            user::bind_core_tcb(task.tcb);
            let entered = core_bridge::with_machine(|m| set_running(m, task.tcb));
            if !entered {
                task.exited = true;
                continue;
            }
            let stop = user::run_task(task.loaded, &mut task.tf);
            task.tf = user::saved_tf();
            match stop {
                Stop::Yield => core_bridge::with_machine(|m| task_ready(m, task.tcb)),
                Stop::Exit { code } => {
                    task.exited = true;
                    task.code = code;
                }
                Stop::Halt { code } => {
                    task.exited = true;
                    task.code = 0x8000_0000 | code;
                }
                Stop::Fault => {
                    // KT-70: pha capa/ring không bật fault→pager ⇒ Fault là lỗi nặng.
                    task.exited = true;
                }
                Stop::Blocked(_) => {
                    // Không dùng IPC chặn: bỏ qua tác vụ ở vòng sau; TAP sẽ đỏ nếu kẹt.
                }
            }
        }
        if !progressed {
            break;
        }
    }
    user::enable_multitask(false);
    (tasks[0].code, tasks[1].code, steps)
}

/// Tên mã lỗi in qua **cùng** nguồn `KError` sinh từ spec (D2-102b) — không viết số trần.
fn abi_error_name(code: u64) -> &'static str {
    match code {
        x if x == u64::from(KError::NotEnoughMemory.id()) => KError::NotEnoughMemory.name(),
        x if x == u64::from(KError::Busy.id()) => KError::Busy.name(),
        _ => "?",
    }
}

fn tap(fails: &mut usize, ok: bool, number: u32, name: &str, reason: &str) {
    let mut w = PlatformWriter(&PLATFORM);
    if ok {
        let _ = writeln!(w, "ok {number} - {name}");
    } else {
        let _ = writeln!(w, "not ok {number} - {name}: {reason}");
        *fails += 1;
    }
}

/// Pha **IDL + ring**: TAP 25..29. Trả `()`; in `ok/not ok` + `# idl:`/`# ring:`.
pub fn ring_phase(root: ObjId, us: SlotRef, ctl: SlotRef, load_untyped: CapId, fails: &mut usize) {
    let mut stage = "start";
    let Some((rig, server, client)) = build_rig(root, us, ctl, load_untyped, &mut stage) else {
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(w, "# ring: rig fail stage={stage}");
            let core = core_bridge::with_machine(|m| {
                let mut occ = 0u32;
                for slot in 13..34 {
                    if m.cap_at(root, slot).is_ok() {
                        occ |= 1 << slot;
                    }
                }
                (
                    m.object_count(),
                    m.cap_count(),
                    m.live_cap_count(),
                    m.cnode_slot_count(root).unwrap_or(0),
                    occ,
                )
            });
            let _ = writeln!(
                w,
                "# ring: rig fail core objects={} caps={} live_caps={} root_slots={} occ=0x{:x}",
                core.0, core.1, core.2, core.3, core.4
            );
            match aobj::machine().check_objmem() {
                Ok(stats) => {
                    let _ = writeln!(
                        w,
                        "# ring: rig fail counts objects={} caps={} pt={} frames={} vs={} maps={}",
                        stats.counts.objects,
                        stats.counts.caps,
                        stats.counts.page_tables,
                        stats.counts.frames,
                        stats.counts.vspaces,
                        stats.counts.mappings
                    );
                }
                Err(error) => {
                    let _ = writeln!(w, "# ring: rig fail check_objmem={:?}", error);
                }
            }
            if let Ok(stats) = aobj::machine().untyped_stats(load_untyped) {
                let _ = writeln!(
                    w,
                    "# ring: rig fail u0 used={} free={} water={} frames={} objects={} holes={}",
                    stats.used,
                    stats.free,
                    stats.watermark,
                    stats.frames,
                    stats.objects,
                    stats.holes
                );
            }
        }
        for (number, name) in [
            (25, "idl stub sinh tu vidl"),
            (26, "ring day"),
            (27, "ring rong"),
            (28, "ring thu tu va wrap"),
            (29, "ring race co kiem soat"),
        ] {
            tap(fails, false, number, name, "dung rig ring that bai");
        }
        return;
    };
    // Mốc **trước** pha: client chưa có cap Frame, hai VSpace chưa map VA_RING.
    let pre = core_bridge::with_machine(|m| {
        (
            (C_RECV..C_RECV + 4).all(|slot| m.cap_at(rig.c, slot) == Err(KError::SlotEmpty)),
            m.cap_at(rig.s, S_FRAME).is_ok(),
            m.cap_transfer_installed(),
            m.cap_transfer_failed(),
        )
    });
    let pre_walk_s = aobj::machine()
        .walk(rig.server_vspace, VA_RING)
        .map(|walk| walk.valid)
        .unwrap_or(true);
    let pre_walk_c = aobj::machine()
        .walk(rig.client_vspace, VA_RING)
        .map(|walk| walk.valid)
        .unwrap_or(true);
    // ---- chạy hai tiến trình U-mode thật, xen kẽ theo scheduler.
    let (server_code, client_code, steps) = run_pair(&rig, &server, &client);
    // ---- đo **sau** pha: header/slot qua physmap, cap/PTE qua core/object layer.
    let ring = Ring::new(PhysMem { pa: rig.frame_pa });
    let opened = ring.open();
    let _geometry = opened.unwrap_or(vifast_ring::Geometry {
        size: 0,
        mask: 0,
        slot_bytes: 0,
    });
    let header = ring.stats();
    let ring_check = ring.check();
    let stat = |s: Stat| ring.stat(s);
    let (attach_msgs, sync_msgs, finish_msgs) = (
        stat(Stat::AttachMsgs),
        stat(Stat::SyncMsgs),
        stat(Stat::FinishMsgs),
    );
    let labels_seen = stat(Stat::LabelsSeen);
    let (sent_ok, recv_ok) = (stat(Stat::SentOk), stat(Stat::RecvOk));
    let (full_err, full_code, full_head) = (
        stat(Stat::FullErr),
        stat(Stat::FullCode),
        stat(Stat::FullHead),
    );
    let (empty_err, empty_code, empty_code_last, empty_tail) = (
        stat(Stat::EmptyErr),
        stat(Stat::EmptyCode),
        stat(Stat::EmptyCodeLast),
        stat(Stat::EmptyTail),
    );
    let (first_seq, last_seq) = (stat(Stat::FirstSeq), stat(Stat::LastSeq));
    let (order_ok, hdrerr, dups, lost, bad_slot) = (
        stat(Stat::OrderOk),
        stat(Stat::HdrErr),
        stat(Stat::Dups),
        stat(Stat::Lost),
        stat(Stat::BadSlot),
    );
    let (prod_sum, cons_sum, peak_used) = (
        stat(Stat::ProdSum),
        stat(Stat::ConsSum),
        stat(Stat::PeakUsed),
    );
    let attach_caps = stat(Stat::AttachCaps);
    let (server_done, client_done, sync_acks) = (
        stat(Stat::ServerDone),
        stat(Stat::ClientDone),
        stat(Stat::SyncAcks),
    );
    let (server_steps, client_steps) = (stat(Stat::ServerSteps), stat(Stat::ClientSteps));
    // Slot 0 (qua physmap) — bằng chứng độc lập rằng byte thật nằm trong Frame chung.
    let slot0 = [
        aobj::machine()
            .read_phys_u64(rig.frame_pa + PAYLOAD_OFF as u64)
            .unwrap_or(0),
        aobj::machine()
            .read_phys_u64(rig.frame_pa + PAYLOAD_OFF as u64 + 8)
            .unwrap_or(0),
        aobj::machine()
            .read_phys_u64(rig.frame_pa + PAYLOAD_OFF as u64 + 16)
            .unwrap_or(0),
        aobj::machine()
            .read_phys_u64(rig.frame_pa + PAYLOAD_OFF as u64 + 24)
            .unwrap_or(0),
    ];
    let (b4, installed, failed) = core_bridge::with_machine(|m| {
        (
            cap_info_at(m, rig.c, C_RECV),
            m.cap_transfer_installed().saturating_sub(pre.2),
            m.cap_transfer_failed().saturating_sub(pre.3),
        )
    });
    let walk_s = aobj::machine().walk(rig.server_vspace, VA_RING).ok();
    let walk_c = aobj::machine().walk(rig.client_vspace, VA_RING).ok();
    let map_of = |walk: Option<vifast_core::vm::WalkInfo>| {
        walk.map(|w| {
            (
                w.valid && w.leaf && w.level == 0,
                w.phys,
                w.raw & (PTE_READ | PTE_WRITE | PTE_USER) == (PTE_READ | PTE_WRITE | PTE_USER),
            )
        })
        .unwrap_or((false, 0, false))
    };
    let (s_valid, s_pa, s_flags) = map_of(walk_s);
    let (c_valid, c_pa, c_flags) = map_of(walk_c);
    let same_pa = s_valid && c_valid && s_pa == rig.frame_pa && c_pa == rig.frame_pa;
    let untyped_obj = core_bridge::with_machine(|m| m.cap_obj_at(root, us.slot).ok()).unwrap_or(0);
    let (caps_ok, ipc_ok, bytes_ok) = core_bridge::with_machine(|m| {
        (
            m.check_caps().is_ok(),
            m.check_ipc().is_ok(),
            if untyped_obj != 0 {
                m.check_bytes(untyped_obj).is_ok()
            } else {
                false
            },
        )
    });
    let objmem_ok = aobj::machine().check_objmem().is_ok();
    let b4_info = b4.map(|info| (info.obj, info.ty, info.rights, info.alive));
    let frame_cap_ok = b4_info
        .map(|(obj, ty, _rights, alive)| obj == rig.frame_obj && ty == ObjType::Frame && alive)
        .unwrap_or(false);
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# idl: const:iface={} const:version={} const:methods={} attach={} sync={} finish={} seen=0x{:x} attach_msgs={} sync_msgs={} finish_msgs={} sync_acks={} attach_caps={} labels_ok={} const:caps_max=1",
            vifast_idl::IFACE_NAME,
            vifast_idl::IFACE_VERSION,
            vifast_idl::METHOD_COUNT,
            vifast_idl::METHOD_ATTACH,
            vifast_idl::METHOD_SYNC,
            vifast_idl::METHOD_FINISH,
            labels_seen,
            attach_msgs,
            sync_msgs,
            finish_msgs,
            sync_acks,
            attach_caps,
            u8::from(labels_seen == 0b111)
        );
        let _ = writeln!(
            w,
            "# ring: frame=0x{:x} const:va=0x{:x} size={} slot={} head={} tail={} seq={} used={} check={} open={} const:capacity={} const:slot_bytes={}",
            rig.frame_pa,
            VA_RING,
            header.size,
            header.slot_bytes,
            header.head,
            header.tail,
            header.seq,
            header.used,
            u8::from(ring_check),
            u8::from(opened.is_ok()),
            RING_CAP,
            RING_SLOT_BYTES
        );
        let _ = writeln!(
            w,
            "# ring: sent={} recv={} full_err={} full_code={} full_name={} full_head={} empty_err={} empty_code_first={} empty_code_last={} empty_name={} empty_tail={} first={} last={} wrap=head/size={} const:size={} head={} order={} hdrerr={} dups={} lost={} bad={} peak={} const:total={}",
            sent_ok,
            recv_ok,
            full_err,
            full_code,
            abi_error_name(full_code),
            full_head,
            empty_err,
            empty_code,
            empty_code_last,
            abi_error_name(empty_code_last),
            empty_tail,
            first_seq,
            last_seq,
            header.head / u64::from(header.size.max(1)),
            RING_CAP,
            header.head,
            order_ok,
            hdrerr,
            dups,
            lost,
            bad_slot,
            peak_used,
            RING_TOTAL
        );
        let _ = writeln!(
            w,
            "# ring: prod_sum={} cons_sum={} sum_equal={} slot0={}/{}/{}/{} const:sum={}",
            prod_sum,
            cons_sum,
            u8::from(prod_sum == cons_sum),
            slot0[0],
            slot0[1],
            slot0[2],
            slot0[3],
            RING_SUM
        );
        let _ = writeln!(
            w,
            "# ring: map server={} client={} same_pa={} pa=0x{:x} s_pa=0x{:x} c_pa=0x{:x} s_flags={} c_flags={} pre_walk={}/{}",
            u8::from(s_valid),
            u8::from(c_valid),
            u8::from(same_pa),
            rig.frame_pa,
            s_pa,
            c_pa,
            u8::from(s_flags),
            u8::from(c_flags),
            u8::from(pre_walk_s),
            u8::from(pre_walk_c)
        );
        let _ = writeln!(
            w,
            "# ring: caps b4={} b4_ty={:?} b4_alive={} frame_obj={} installed={}->{} failed={}->{} caps={} ipc={} obj={} bytes={} steps=s{}/c{} exits={}/{} run_steps={}",
            u8::from(frame_cap_ok),
            b4_info.map(|(_, ty, _, _)| ty).unwrap_or(ObjType::Untyped),
            u8::from(b4_info.map(|(_, _, _, alive)| alive).unwrap_or(false)),
            b4_info.map(|(obj, _, _, _)| obj).unwrap_or(0),
            pre.2,
            pre.2.saturating_add(installed),
            pre.3,
            pre.3.saturating_add(failed),
            u8::from(caps_ok),
            u8::from(ipc_ok),
            u8::from(objmem_ok),
            u8::from(bytes_ok),
            server_steps,
            client_steps,
            server_code,
            client_code,
            steps
        );
    }
    let idl_ok = server_code == 0
        && client_code == 0
        && attach_msgs >= 1
        && sync_msgs >= 10
        && finish_msgs == 1
        && labels_seen == 0b111
        && server_done == 1
        && client_done == 1
        && attach_caps == 1
        && ring_check
        && opened.is_ok()
        && header.size == RING_CAP
        && header.slot_bytes == RING_SLOT_BYTES
        && sync_acks >= 10;
    tap(
        fails,
        idl_ok,
        25,
        "idl stub sinh tu vidl",
        "handshake/IDL/cap sai (xem # idl:)",
    );
    let full_ok = full_err >= 1
        && full_code == u64::from(vifast_ring::ERR_FULL)
        && full_head == u64::from(RING_CAP)
        && sent_ok >= u64::from(RING_CAP)
        && ring_check;
    tap(
        fails,
        full_ok,
        26,
        "ring day",
        "day khong tra ma 9 hoac head sai",
    );
    let empty_ok = empty_err >= 1
        && empty_code == u64::from(vifast_ring::ERR_EMPTY)
        && empty_code_last == u64::from(vifast_ring::ERR_EMPTY)
        && empty_tail == 0
        && ring_check;
    tap(
        fails,
        empty_ok,
        27,
        "ring rong",
        "rong khong tra ma 14 hoac tail sai",
    );
    let order_ok = sent_ok == RING_TOTAL
        && recv_ok == RING_TOTAL
        && first_seq == 0
        && last_seq == RING_TOTAL - 1
        && order_ok == 1
        && hdrerr == 0
        && dups == 0
        && lost == 0
        && bad_slot == 0
        && prod_sum == RING_SUM
        && cons_sum == RING_SUM
        && header.head == RING_TOTAL
        && header.tail == RING_TOTAL
        && header.size == RING_CAP
        && header.head / u64::from(header.size) >= 5
        && ring_check;
    tap(
        fails,
        order_ok,
        28,
        "ring thu tu va wrap",
        "thu tu/wrap/tong sai (xem # ring:)",
    );
    let race_ok = server_steps >= 10
        && client_steps >= 10
        && peak_used == u64::from(RING_CAP)
        && same_pa
        && s_flags
        && c_flags
        && frame_cap_ok
        && installed == 1
        && failed == 0
        && caps_ok
        && ipc_ok
        && objmem_ok
        && bytes_ok
        && ring_check;
    tap(
        fails,
        race_ok,
        29,
        "ring race co kiem soat",
        "xen ke/mapping/bat bien/cap sai",
    );
    // Dọn mapping + trả VSpace/PageTable về object layer (arena 64 KiB dùng tiếp các pha sau).
    server.release();
    client.release();
}
