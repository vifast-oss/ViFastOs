// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-68** — pha **capalloc** trên ảnh `vifast-kbin-ktest-user` (TAP 20..24).
//!
//! | TAP | Điều kiện đo |
//! |---|---|
//! | `ok 20 - capalloc cap vong` | **hai tiến trình U-mode thật** (`capa_server.elf` + `capa_client.elf`): client gửi cap CNode của mình (KT-67b) → server **cấp** cap Frame vào slot rảnh bằng `CNode.Copy` (quét lỗi 7) → client **dùng** cap (map/ghi/đọc/unmap) → tự dựng **cây 2 tầng** → server `Revoke` (đúng 2 cap) + `Delete` (trả slot) → cấp lại **cùng slot** → server `Revoke` **cap nguồn** ⇒ xoá **xuyên CNode** |
//! | `ok 21 - capalloc mint copy move` | `Mint` badge + thu hẹp quyền (badge trên Frame ⇒ 5); `Copy` giữ quyền/badge; `Move` xoá nguồn + đích bị xoá trước + con reparent |
//! | `ok 22 - capalloc revoke cay delete reparent` | `Revoke` đúng cây con 2 tầng (không lan nhánh khác) + `Delete` reparent MDB + `Revoke` cap nguồn xuyên CNode |
//! | `ok 23 - capalloc ca am` | ô rỗng ⇒ 8; thiếu `RIGHT_WRITE` ⇒ 3; quyền vượt nguồn ⇒ 3; đích bận ⇒ 7; slot ngoài bảng ⇒ 19; **`Mutate` quyền ngoài `RIGHTS_ALL` ⇒ 5** / **`Rotate` hai ô rỗng ⇒ 8** (KT-417 đã cài, giữ **có đếm**) + object khác ⇒ 18; thiếu/thừa MR ⇒ 5; CPTR cũ sau Delete/Revoke ⇒ 8; cap chết (ngoài slot) `alive=0` |
//! | `ok 24 - capalloc bat bien` | sau chu kỳ: `check_caps`/`check_bytes`/`check_ipc`/`check_objmem` xanh; `live_cap_count` và **slot rảnh** của hai CNode trở về mốc đầu (`slot_rfree`); `cap_transfer` delta đúng; không cap trỏ object chết |
//!
//! Bằng chứng `# capa:` do **kernel** in (số đo thật: cap, PA, PTE, physmap) — D2-80. Chương
//! trình U-mode chỉ in dấu `U: capa …` qua `DebugPutStr`; mọi kết luận TAP lấy từ trạng thái
//! `Machine` + object layer.
//!
//! Không `unsafe` trong tệp này (R5): mọi truy cập core/object layer qua API an toàn.

use core::fmt::Write;

use vifast_core::invoke;
use vifast_core::{
    KError, Machine, NonReadyTarget, ObjId, ObjType, SlotRef, TcbState, RIGHTS_ALL, RIGHT_READ,
    RIGHT_WRITE,
};
use vifast_plat_common::obj::{CapId, ObjKind};
use vifast_plat_common::PlatformWriter;
use vifast_plat_riscv64::core_bridge;
use vifast_plat_riscv64::mem_bridge;
use vifast_plat_riscv64::obj as aobj;
use vifast_plat_riscv64::trap::{TrapFrame, REG_SP};
use vifast_plat_riscv64::user::{self, Stop};
use vifast_plat_riscv64::Riscv64Virt;

static PLATFORM: Riscv64Virt = Riscv64Virt;

/// Hai chương trình U-mode **riêng** do `build.rs` sinh (KT-68).
static CAPA_SERVER_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/capa_server.elf"));
static CAPA_CLIENT_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/capa_client.elf"));

// ---------------------------------------------------------------------------------------------
// Bố cục CSpace (khớp hằng trong hai chương trình U-mode)
// ---------------------------------------------------------------------------------------------

/// CNode **server** (16 slot).
const S_SELF: usize = 0;
const S_EP_REQ: usize = 1;
const S_EP_RESP: usize = 2;
const S_FRAME: usize = 5;
/// CNode **client** (16 slot).
const C_SELF: usize = 0;
const C_EP_REQ: usize = 1;
const C_EP_RESP: usize = 2;
const C_VSPACE: usize = 3;
/// Slot tạm của rig trong cspace gốc (13..24 đã dùng ở pha KT-67a/67b) — 25..28.
const TMP_TCB_S: usize = 25;
const TMP_TCB_C: usize = 26;
const TMP_SC_S: usize = 27;
const TMP_SC_C: usize = 28;

/// Ô bảng dải user **riêng cho hai chương trình** (`MAX_PROCS = 4`; slot 1/2 đã dùng ở
/// KT-67a/67b) — nit reviewer KT-68: cùng slot sẽ dùng chung ô debug `RANGE_SLOTS`.
const LOAD_SLOT_S: usize = 0;
const LOAD_SLOT_C: usize = 3;
/// VA client map Frame; giá trị client ghi lần **hai** (kernel đọc physmap); cỡ Frame 4 KiB.
const VA_MAP: u64 = 0x4100_0000;
const MAGIC2: u64 = 0x8877_6655_4433_2211;
const FRAME_BITS: u8 = 12;

/// Số method **lấy từ `spec/abi.toml`** (`[object].CNode`: 0/1/2/4/5; Frame 11: 0/1).
const CNODE_COPY: u32 = invoke::CNODE_COPY;
const CNODE_MINT: u32 = invoke::CNODE_MINT;
const CNODE_MOVE: u32 = invoke::CNODE_MOVE;
const CNODE_DELETE: u32 = invoke::CNODE_DELETE;
const CNODE_REVOKE: u32 = invoke::CNODE_REVOKE;

/// Hook arch tối thiểu cho invocation kernel-level (CNode không dùng hook arch).
struct CapaEnv;

impl vifast_core::syscall::SyscallEnv for CapaEnv {
    fn put_str(&mut self, _ptr: u64, len: u64) -> Result<u64, u64> {
        Ok(len)
    }
    fn resolve_addr(&mut self, _va: u64) -> Result<(u32, u32), u64> {
        Err(11)
    }
    fn now_ns(&mut self) -> u64 {
        0
    }
    fn features(&mut self) -> u64 {
        0
    }
    fn num_harts(&mut self) -> u64 {
        vifast_plat_riscv64::smp::hart_count() as u64
    }
}

/// Đưa TCB `Inactive/Ready` → `Running{hart:0}` (như `ktest_capx`).
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

/// `MessageInfo` 35 bit: `length` = số MR dùng, `label` = method, `caps = 0`.
fn info(label: u32, len: u8) -> u64 {
    u64::from(len) << 28 | u64::from(label)
}

/// Gọi invocation kernel-level trên cap CNode **của server** (self cap slot 0, CSpace `s`).
fn kinvoke(
    m: &mut Machine,
    tcb_s: ObjId,
    s: ObjId,
    method: u32,
    len: u8,
    mrs: [u64; 4],
) -> Result<(u64, u64), u64> {
    invoke::dispatch(
        m,
        tcb_s,
        SlotRef::new(s, S_SELF),
        info(method, len),
        &mrs,
        &mut CapaEnv,
    )
}

fn cap_info_at(m: &Machine, cnode: ObjId, slot: usize) -> Option<vifast_core::CapInfo> {
    m.cap_at(cnode, slot).ok().and_then(|c| m.cap_info(c).ok())
}

/// Số slot **rảnh** của CNode (lỗi `SlotEmpty`) — `slot_rfree` của bằng chứng.
fn free_slots(m: &Machine, cnode: ObjId) -> usize {
    let n = m.cnode_slot_count(cnode).unwrap_or(0);
    (0..n)
        .filter(|slot| m.cap_at(cnode, *slot) == Err(KError::SlotEmpty))
        .count()
}

/// Mốc đo trước pha (để chứng minh không rò).
#[derive(Clone, Copy, Default)]
struct Milestone {
    live_caps: usize,
    cap_high: usize,
    s_free: usize,
    c_free: usize,
    installed: u64,
    failed: u64,
}

/// Rig của pha capalloc.
struct Rig {
    s: ObjId,
    c: ObjId,
    tcb_s: ObjId,
    tcb_c: ObjId,
    frame_obj: ObjId,
    frame_pa: u64,
    client_vspace: CapId,
}

/// Dựng rig: hai CNode + hai Endpoint + hai TCB + cap Frame (object layer) + VSpace client.
fn build_rig(
    root: ObjId,
    us: SlotRef,
    ctl: SlotRef,
    load_untyped: CapId,
    stage: &mut &'static str,
) -> Option<(Rig, user::LoadedInit, user::LoadedInit)> {
    // ---- nạp HAI chương trình U-mode (mỗi đứa một VSpace; slot dải 3 dùng chung vì cùng layout).
    *stage = "load server";
    let server = match user::load_init_slot(CAPA_SERVER_ELF, load_untyped, LOAD_SLOT_S) {
        Ok(loaded) => loaded,
        Err(error) => {
            *stage = error.name();
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(w, "# capa: load server err={:?}", error);
            return None;
        }
    };
    *stage = "load client";
    let client = match user::load_init_slot(CAPA_CLIENT_ELF, load_untyped, LOAD_SLOT_C) {
        Ok(loaded) => loaded,
        Err(error) => {
            *stage = error.name();
            return None;
        }
    };
    *stage = "client vspace key";
    let client_vs_key = {
        let m = aobj::machine();
        let id = m.cap_key(client.vspace).ok()?;
        mem_bridge::make_key(client.vspace, id)
    };
    *stage = "object frame";
    let frame = {
        let m = aobj::machine();
        let cap = m.retype(load_untyped, ObjKind::Frame, FRAME_BITS).ok()?;
        match (m.cap_key(cap), m.frame_pa(cap)) {
            (Ok(id), Ok(pa)) => Some((mem_bridge::make_key(cap, id), pa)),
            _ => None,
        }
    };
    let (frame_key, frame_pa) = frame?;
    *stage = "core rig";
    let rig = core_bridge::with_machine(|m| -> Option<Rig> {
        let s = m.add_cnode(16);
        let c = m.add_cnode(16);
        let s_self = m.root_cap(s);
        m.put_cap(s, S_SELF, s_self).ok()?;
        let c_self = m.root_cap(c);
        m.put_cap(c, C_SELF, c_self).ok()?;
        // Endpoint REQ/RESP — cùng object, cap ở **cả hai** CSpace.
        for (slot_s, slot_c) in [(S_EP_REQ, C_EP_REQ), (S_EP_RESP, C_EP_RESP)] {
            let ep = m.retype(us, ObjType::Endpoint, 0, 0).ok()?;
            m.put_cap(s, slot_s, ep).ok()?;
            m.copy(SlotRef::new(s, slot_s), SlotRef::new(c, slot_c))
                .ok()?;
        }
        // Frame của object layer (D2-83) — cap đại diện trong CSpace server.
        let frame_obj = m.add_foreign_object(ObjType::Frame, frame_key).ok()?;
        let frame_cap = m.root_cap(frame_obj);
        m.put_cap(s, S_FRAME, frame_cap).ok()?;
        // VSpace của client (object layer sở hữu) — cap trong CSpace client.
        let vs = m.add_foreign_object(ObjType::VSpace, client_vs_key).ok()?;
        let vs_cap = m.root_cap(vs);
        m.put_cap(c, C_VSPACE, vs_cap).ok()?;
        // Hai TCB có cspace **riêng** + SC + độ ưu tiên.
        let mut tcb_objs = [0u32; 2];
        for (index, (tcb_slot, sc_slot, cnode)) in
            [(TMP_TCB_S, TMP_SC_S, s), (TMP_TCB_C, TMP_SC_C, c)]
                .into_iter()
                .enumerate()
        {
            let tcb_cap = m.retype(us, ObjType::Tcb, 0, 0).ok()?;
            let tcb = m.cap_obj(tcb_cap).ok()?;
            m.put_cap(root, tcb_slot, tcb_cap).ok()?;
            let sc = m.retype(us, ObjType::SchedContext, 0, 0).ok()?;
            m.put_cap(root, sc_slot, sc).ok()?;
            m.tcb_configure(SlotRef::new(root, tcb_slot), Some(SlotRef::new(cnode, 0)))
                .ok()?;
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
            client_vspace: client.vspace,
        })
    });
    rig.map(|rig| (rig, server, client))
}

/// Một tác vụ U-mode tuần tự (giữ trapframe để chạy tiếp sau `Yield`).
struct Task<'a> {
    loaded: &'a user::LoadedInit,
    tf: TrapFrame,
    tcb: ObjId,
    /// Ô bảng dải user của **chính** tác vụ này (`LOAD_SLOT_S`/`LOAD_SLOT_C`).
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

/// Chạy round-robin server ↔ client tới khi **cả hai** `Exit` (hoặc hết bước).
fn run_pair(rig: &Rig, server: &user::LoadedInit, client: &user::LoadedInit) -> (u64, u64, usize) {
    let mut tasks = [
        Task::new(server, rig.tcb_s, LOAD_SLOT_S),
        Task::new(client, rig.tcb_c, LOAD_SLOT_C),
    ];
    user::enable_multitask(true);
    let mut steps = 0usize;
    while steps < 500 && !tasks.iter().all(|task| task.exited) {
        for task in tasks.iter_mut() {
            if task.exited {
                continue;
            }
            steps += 1;
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
                    task.exited = true;
                    task.code = 0x4000_0000;
                }
            }
        }
    }
    user::enable_multitask(false);
    (tasks[0].code, tasks[1].code, steps)
}

/// Pha **capalloc**: TAP 20..24. Trả `()`; in `ok/not ok` + `# capa:`.
pub fn capalloc_phase(
    root: ObjId,
    us: SlotRef,
    ctl: SlotRef,
    load_untyped: CapId,
    fails: &mut usize,
) {
    let mut stage = "start";
    if let Ok(stats) = aobj::machine().untyped_stats(load_untyped) {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# capa: u0 truoc rig cap={} used={} free={} water={} frames={} objects={} holes={}",
            stats.capacity,
            stats.used,
            stats.free,
            stats.watermark,
            stats.frames,
            stats.objects,
            stats.holes
        );
    }
    let Some((rig, server, client)) = build_rig(root, us, ctl, load_untyped, &mut stage) else {
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(w, "# capa: rig fail stage={stage}");
            if let Ok(stats) = aobj::machine().untyped_stats(load_untyped) {
                let _ = writeln!(
                    w,
                    "# capa: u0 sau fail used={} free={} water={} frames={} objects={}",
                    stats.used, stats.free, stats.watermark, stats.frames, stats.objects
                );
            }
        }
        fail(fails, "dung rig capalloc that bai");
        return;
    };
    let untyped_obj = core_bridge::with_machine(|m| m.cap_obj_at(root, us.slot).ok()).unwrap_or(0);
    let milestone = core_bridge::with_machine(|m| Milestone {
        live_caps: m.live_cap_count(),
        cap_high: m.cap_count(),
        s_free: free_slots(m, rig.s),
        c_free: free_slots(m, rig.c),
        installed: m.cap_transfer_installed(),
        failed: m.cap_transfer_failed(),
    });
    // ---- trạng thái **trước** pha: client chưa có cap Frame ở slot 4..7, VSpace chưa map.
    let pre = core_bridge::with_machine(|m| {
        let empty = (4..8).all(|slot| m.cap_at(rig.c, slot) == Err(KError::SlotEmpty));
        (empty, m.cap_at(rig.s, S_FRAME).is_ok())
    });
    let pre_walk = aobj::machine()
        .walk(rig.client_vspace, VA_MAP)
        .map(|walk| walk.valid)
        .unwrap_or(true);

    // ---- chạy HAI tiến trình U-mode thật (capalloc server + client).
    let (server_code, client_code, steps) = run_pair(&rig, &server, &client);

    // ---- đo **sau** pha U-mode: cap/PTE/PA thật; không tin lời chương trình.
    let after = core_bridge::with_machine(|m| {
        let s5 = cap_info_at(m, rig.s, S_FRAME);
        let s8 = m.cap_at(rig.s, 8);
        let c4 = m.cap_at(rig.c, 4);
        let c5 = m.cap_at(rig.c, 5);
        let c6 = m.cap_at(rig.c, 6);
        let c7 = m.cap_at(rig.c, 7);
        let caps_ok = m.check_caps().is_ok();
        let ipc_ok = m.check_ipc().is_ok();
        let bytes_ok = if untyped_obj != 0 {
            m.check_bytes(untyped_obj).is_ok()
        } else {
            false
        };
        (
            s5.map(|c| (c.obj, c.ty, c.rights, c.alive)),
            s8 == Err(KError::SlotEmpty),
            c4 == Err(KError::SlotEmpty),
            c5 == Err(KError::SlotEmpty),
            c6 == Err(KError::SlotEmpty),
            c7 == Err(KError::SlotEmpty),
            caps_ok,
            ipc_ok,
            bytes_ok,
        )
    });
    let walk_after = aobj::machine().walk(rig.client_vspace, VA_MAP).ok();
    let (walk_valid, walk_pa) = walk_after
        .map(|walk| (walk.valid && walk.leaf && walk.level == 0, walk.phys))
        .unwrap_or((false, 0));
    let phys = aobj::machine().read_phys_u64(walk_pa);
    let objmem_ok = aobj::machine().check_objmem().is_ok();
    let stats = invoke::stats();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let (s5_obj, s5_ty, s5_rights, s5_alive) =
            after.0.unwrap_or((0, ObjType::Untyped, 0, false));
        let _ = writeln!(
            w,
            "# capa: u-mode server_exit={} client_exit={} steps={} src_obj={} src_ty={:?} src_rights=0x{:x} src_alive={}",
            server_code,
            client_code,
            steps,
            s5_obj,
            s5_ty,
            s5_rights,
            s5_alive as u8
        );
        let _ = writeln!(
            w,
            "# capa: cap slot client4..7={}{}{}{} server8={} copies={} revokes={} deletes={} mints={} moves={} denied={}",
            after.2 as u8,
            after.3 as u8,
            after.4 as u8,
            after.5 as u8,
            after.1 as u8,
            stats.copy,
            stats.revoke,
            stats.delete,
            stats.mint,
            stats.moved,
            stats.denied
        );
        let _ = writeln!(
            w,
            "# capa: walk va=0x{:x} valid={} pa=0x{:x} expected_pa=0x{:x} phys=0x{:x} client_vspace={}",
            VA_MAP,
            walk_valid as u8,
            walk_pa,
            rig.frame_pa,
            phys.unwrap_or(0),
            rig.client_vspace.0
        );
        let _ = writeln!(
            w,
            "# capa: pre empty_client={} src_alive={} walk_before={} post caps={} ipc={} bytes={} obj={}",
            pre.0 as u8,
            pre.1 as u8,
            pre_walk as u8,
            after.6 as u8,
            after.7 as u8,
            after.8 as u8,
            objmem_ok as u8
        );
    }
    let ok20 = server_code == 0
        && client_code == 0
        && pre.0
        && pre.1
        && !pre_walk
        && walk_valid
        && walk_pa == rig.frame_pa
        && phys == Some(MAGIC2)
        && after
            .0
            .map(|(obj, ty, rights, alive)| {
                obj == rig.frame_obj && ty == ObjType::Frame && rights == RIGHTS_ALL && alive
            })
            .unwrap_or(false)
        && after.1
        && after.2
        && after.3
        && after.4
        && after.5
        && stats.copy >= 5
        && stats.revoke >= 2
        && stats.delete >= 3
        && after.6
        && after.7
        && after.8
        && objmem_ok;
    tap(
        fails,
        ok20,
        20,
        "capalloc cap vong",
        "server/client/PTE/PA/cap sai",
    );

    // ---- dọn mapping của client trước khi kiểm bất biến cuối.
    let _ = aobj::machine().unmap(rig.client_vspace, VA_MAP);

    // =========================================================================================
    // TAP 21..23 — kernel-level qua đúng `invoke::dispatch` (syscall 0), trên `Machine` thật
    // =========================================================================================
    tap21(fails, &rig);
    tap22(fails, &rig);
    tap23(fails, &rig);

    // =========================================================================================
    // TAP 24 — bất biến + không rò slot/cap sau chu kỳ
    // =========================================================================================
    let end = core_bridge::with_machine(|m| Milestone {
        live_caps: m.live_cap_count(),
        cap_high: m.cap_count(),
        s_free: free_slots(m, rig.s),
        c_free: free_slots(m, rig.c),
        installed: m.cap_transfer_installed(),
        failed: m.cap_transfer_failed(),
    });
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
    let objmem = aobj::machine().check_objmem();
    let objmem_ok = objmem.is_ok();
    let frames_live = objmem.map(|stats| stats.counts.frames).unwrap_or(0);
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# capa: rfree s={}->{} c={}->{} live={}->{} cap_high={}->{} installed+={} failed+={}",
            milestone.s_free,
            end.s_free,
            milestone.c_free,
            end.c_free,
            milestone.live_caps,
            end.live_caps,
            milestone.cap_high,
            end.cap_high,
            end.installed.saturating_sub(milestone.installed),
            end.failed.saturating_sub(milestone.failed)
        );
        let _ = writeln!(
            w,
            "# capa: invariants caps={} ipc={} bytes={} obj={} frames={}",
            caps_ok as u8, ipc_ok as u8, bytes_ok as u8, objmem_ok as u8, frames_live
        );
    }
    let ok24 = caps_ok
        && ipc_ok
        && bytes_ok
        && objmem_ok
        && end.live_caps == milestone.live_caps
        && end.s_free == milestone.s_free
        && end.c_free == milestone.c_free
        && end.installed.saturating_sub(milestone.installed) == 1
        && end.failed.saturating_sub(milestone.failed) == 0
        && end.cap_high >= milestone.cap_high;
    tap(
        fails,
        ok24,
        24,
        "capalloc bat bien",
        "ro cap/slot hoac bat bien do",
    );
    // KT-69: trả **hai VSpace** của rig về object layer sau khi đã đo xong TAP 20..24 — arena
    // 64 KiB còn chỗ cho pha IDL + ring (hai VSpace nữa). Đây là dọn tài nguyên sau đo, không
    // đổi bất kỳ điều kiện TAP nào ở trên (không nới ngưỡng).
    // KT-69: xoá cap tạm trong cspace gốc (25..28) để pha ring dùng lại — cspace gốc 32 slot.
    core_bridge::with_machine(|m| {
        for slot in TMP_TCB_S..=TMP_SC_C {
            let _ = m.delete(SlotRef::new(root, slot));
        }
    });
    server.release();
    client.release();
}

// ---------------------------------------------------------------------------------------------
// TAP 21 — Mint badge/rights · Copy giữ rights · Move xoá nguồn
// ---------------------------------------------------------------------------------------------

fn tap21(fails: &mut usize, rig: &Rig) {
    let ok = core_bridge::with_machine(|m| -> bool {
        let before = invoke::stats();
        // Mint EP (slot 1) → slot 3: quyền R|W + badge 0xABC.
        if kinvoke(
            m,
            rig.tcb_s,
            rig.s,
            CNODE_MINT,
            4,
            [
                S_EP_REQ as u64,
                3,
                u64::from(RIGHT_READ | RIGHT_WRITE),
                0xABC,
            ],
        ) != Ok((0, 0))
        {
            return false;
        }
        let minted = cap_info_at(m, rig.s, 3);
        let Some(minted) = minted else { return false };
        let mint_ok = minted.rights == RIGHT_READ | RIGHT_WRITE && minted.badge == Some(0xABC);
        // Badge trên **Frame** (không phải Endpoint/Notification) ⇒ 5.
        let badge_frame = kinvoke(
            m,
            rig.tcb_s,
            rig.s,
            CNODE_MINT,
            4,
            [S_FRAME as u64, 4, u64::from(RIGHTS_ALL), 1],
        ) == Err(KError::InvalidArgument.id() as u64);
        // Copy giữ quyền + badge.
        let copy_ok = kinvoke(m, rig.tcb_s, rig.s, CNODE_COPY, 2, [3, 4, 0, 0]) == Ok((0, 0));
        let copied = cap_info_at(m, rig.s, 4);
        let copy_kept = copy_ok
            && copied
                .map(|c| {
                    c.rights == RIGHT_READ | RIGHT_WRITE
                        && c.badge == Some(0xABC)
                        && c.parent == Some(minted.cap)
                })
                .unwrap_or(false);
        // Move slot 4 → slot 3 (đích đang giữ **cha** của nó): xoá đích trước, con reparent lên
        // ông (EP slot 1), rồi chuyển; nguồn rỗng, cùng CapId.
        let moved_cap = copied.map(|c| c.cap).unwrap_or(0);
        let move_ok = kinvoke(m, rig.tcb_s, rig.s, CNODE_MOVE, 2, [4, 3, 0, 0]) == Ok((0, 0));
        let moved = cap_info_at(m, rig.s, 3);
        let move_kept = move_ok
            && m.cap_at(rig.s, 4) == Err(KError::SlotEmpty)
            && moved
                .map(|c| c.cap == moved_cap && c.badge == Some(0xABC) && c.parent.is_some())
                .unwrap_or(false);
        let ep_cap = m.cap_at(rig.s, S_EP_REQ).ok();
        let parent_ok = moved.map(|c| c.parent == ep_cap).unwrap_or(false);
        // Dọn: xoá cap Mint/Move ở slot 3.
        let cleanup = kinvoke(m, rig.tcb_s, rig.s, CNODE_DELETE, 1, [3, 0, 0, 0]) == Ok((0, 0));
        let after = invoke::stats();
        let counted =
            after.mint > before.mint && after.copy > before.copy && after.moved > before.moved;
        let caps_ok = m.check_caps().is_ok();
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# capa: mint badge=0x{:x} rights=0x{:x} badge_frame={} copy_rights=0x{:x} move_src_empty={} same_cap={} parent_ep={} cleanup={} caps={}",
                moved.and_then(|c| c.badge).unwrap_or(0),
                minted.rights,
                badge_frame as u8,
                copied.map(|c| c.rights).unwrap_or(0),
                (m.cap_at(rig.s, 4) == Err(KError::SlotEmpty)) as u8,
                move_kept as u8,
                parent_ok as u8,
                cleanup as u8,
                caps_ok as u8
            );
        }
        mint_ok
            && badge_frame
            && copy_kept
            && move_ok
            && move_kept
            && parent_ok
            && counted
            && caps_ok
            && cleanup
    });
    tap(
        fails,
        ok,
        21,
        "capalloc mint copy move",
        "mint/copy/move sai",
    );
}

// ---------------------------------------------------------------------------------------------
// TAP 22 — Revoke cây 2 tầng · Delete reparent · Revoke cap nguồn xuyên CNode
// ---------------------------------------------------------------------------------------------

fn tap22(fails: &mut usize, rig: &Rig) {
    let ok = core_bridge::with_machine(|m| -> bool {
        let before = m.live_cap_count();
        // Cây: 6 = A (con EP); 7 = B (con A); 9 = C (con B); 10 = D (con EP — nhánh khác).
        // Tránh 8..11: **receive window** của server (cap CNode client được cài ở slot 8).
        let mut build_ok = true;
        for (src, dst) in [
            (S_EP_REQ as u64, 6u64),
            (6, 7),
            (7, 9),
            (S_EP_REQ as u64, 10),
        ] {
            if kinvoke(m, rig.tcb_s, rig.s, CNODE_COPY, 2, [src, dst, 0, 0]) != Ok((0, 0)) {
                build_ok = false;
            }
        }
        let revoke = kinvoke(m, rig.tcb_s, rig.s, CNODE_REVOKE, 1, [6, 0, 0, 0]);
        let revoke_ok = revoke == Ok((0, 2));
        let live_now = m.live_cap_count();
        let subtree_ok = m.cap_at(rig.s, 7) == Err(KError::SlotEmpty)
            && m.cap_at(rig.s, 9) == Err(KError::SlotEmpty)
            && m.cap_at(rig.s, 6).is_ok()
            && m.cap_at(rig.s, 10).is_ok();
        let sibling_ok = m.cap_at(rig.s, 6).is_ok() && m.cap_at(rig.s, 10).is_ok();
        let live_after_revoke = live_now == before + 2;
        // Delete reparent: 6 = A, 7 = B (con A); xoá A ⇒ B cha = EP cap.
        let del_build = kinvoke(m, rig.tcb_s, rig.s, CNODE_COPY, 2, [6, 7, 0, 0]) == Ok((0, 0));
        let b_child = cap_info_at(m, rig.s, 7);
        let delete_ok = kinvoke(m, rig.tcb_s, rig.s, CNODE_DELETE, 1, [6, 0, 0, 0]) == Ok((0, 0));
        let reparented = b_child
            .map(|b| {
                cap_info_at(m, rig.s, 7)
                    .map(|after| after.parent == m.cap_at(rig.s, S_EP_REQ).ok())
                    .unwrap_or(false)
                    && b.cap != 0
            })
            .unwrap_or(false)
            && delete_ok;
        // Revoke **cap nguồn Frame** (slot 5) ⇒ xoá bản sao xuyên CNode trong client cspace.
        let cross_ok = {
            // Cap CNode client trong CSpace server (slot 12) để gọi `Copy` vào client.
            let cnode_cap = m.root_cap(rig.c);
            if m.put_cap(rig.s, 12, cnode_cap).is_err() {
                false
            } else {
                // Gọi `CNode.Copy` trên **cap CNode client** (slot 12): cấp Frame của server
                // (slot 5) vào slot 5 của CNode client.
                let copy = invoke::dispatch(
                    m,
                    rig.tcb_s,
                    SlotRef::new(rig.s, 12),
                    info(CNODE_COPY, 2),
                    &[S_FRAME as u64, 9, 0, 0],
                    &mut CapaEnv,
                );
                let child_in_c = copy == Ok((0, 0)) && m.cap_at(rig.c, 9).is_ok();
                // Revoke cap nguồn (self CNode server, slot 5) ⇒ xoá con **xuyên CNode**.
                let revoked = kinvoke(
                    m,
                    rig.tcb_s,
                    rig.s,
                    CNODE_REVOKE,
                    1,
                    [S_FRAME as u64, 0, 0, 0],
                );
                let revoked_ok = revoked == Ok((0, 1));
                let cleared = m.cap_at(rig.c, 9) == Err(KError::SlotEmpty);
                let src_alive = m.cap_at(rig.s, S_FRAME).is_ok();
                let deleted =
                    kinvoke(m, rig.tcb_s, rig.s, CNODE_DELETE, 1, [12, 0, 0, 0]) == Ok((0, 0));
                child_in_c && revoked_ok && cleared && src_alive && deleted
            }
        };
        // Dọn cây: 6, 7, 10 (nếu còn).
        let _ = kinvoke(m, rig.tcb_s, rig.s, CNODE_DELETE, 1, [7, 0, 0, 0]);
        let _ = kinvoke(m, rig.tcb_s, rig.s, CNODE_DELETE, 1, [6, 0, 0, 0]);
        let _ = kinvoke(m, rig.tcb_s, rig.s, CNODE_DELETE, 1, [10, 0, 0, 0]);
        let caps_ok = m.check_caps().is_ok();
        let src_alive_after = m.cap_at(rig.s, S_FRAME).is_ok();
        let stats = invoke::stats();
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# capa: revoke slot=6 deleted={} subtree={} sibling={} live_before={} live_after={} delete_reparent={} cross_cnode={} src_alive={} caps={} revokes={}",
                match revoke {
                    Ok((_, n)) => n,
                    _ => u32::MAX as u64,
                },
                subtree_ok as u8,
                sibling_ok as u8,
                before,
                live_now,
                reparented as u8,
                cross_ok as u8,
                src_alive_after as u8,
                caps_ok as u8,
                stats.revoke
            );
        }
        revoke_ok
            && build_ok
            && del_build
            && subtree_ok
            && sibling_ok
            && live_after_revoke
            && reparented
            && cross_ok
            && caps_ok
    });
    tap(
        fails,
        ok,
        22,
        "capalloc revoke cay delete reparent",
        "revoke/delete sai",
    );
}

// ---------------------------------------------------------------------------------------------
// TAP 23 — ca âm
// ---------------------------------------------------------------------------------------------

fn tap23(fails: &mut usize, rig: &Rig) {
    let ok = core_bridge::with_machine(|m| -> bool {
        // Ô rỗng ⇒ 8.
        let empty = kinvoke(m, rig.tcb_s, rig.s, CNODE_DELETE, 1, [10, 0, 0, 0])
            == Err(KError::SlotEmpty.id() as u64);
        // Đích bận ⇒ 7.
        let occupied = kinvoke(
            m,
            rig.tcb_s,
            rig.s,
            CNODE_COPY,
            2,
            [S_EP_REQ as u64, S_EP_REQ as u64, 0, 0],
        ) == Err(KError::SlotOccupied.id() as u64);
        // Quyền vượt nguồn (EP cap đủ 4 bit; 0xFF có bit lạ) ⇒ 3.
        let rights = kinvoke(
            m,
            rig.tcb_s,
            rig.s,
            CNODE_COPY,
            3,
            [S_EP_REQ as u64, 10, 0xFF, 0],
        ) == Err(KError::InsufficientRights.id() as u64);
        // Slot ngoài bảng ⇒ 19.
        let range = kinvoke(
            m,
            rig.tcb_s,
            rig.s,
            CNODE_COPY,
            2,
            [S_EP_REQ as u64, 99, 0, 0],
        ) == Err(KError::RangeError.id() as u64);
        // Cap CNode **chỉ READ** ⇒ 3.
        let ro_cap = m.copy_rights(
            SlotRef::new(rig.s, S_SELF),
            SlotRef::new(rig.s, 10),
            Some(RIGHT_READ),
        );
        let ro_ok = ro_cap.is_ok()
            && invoke::dispatch(
                m,
                rig.tcb_s,
                SlotRef::new(rig.s, 10),
                info(CNODE_COPY, 2),
                &[S_EP_REQ as u64, 11, 0, 0],
                &mut CapaEnv,
            ) == Err(KError::InsufficientRights.id() as u64);
        // **KT-417**: `Mutate` (3) / `Rotate` (6) **đã cài** ⇒ giữ ca âm nhưng theo **luật mới**
        // (vẫn **có đếm** `denied`): quyền ngoài `RIGHTS_ALL` ⇒ `InvalidArgument` 5; hoán vị
        // **hai ô rỗng** ⇒ `SlotEmpty` 8; object khác ⇒ 18 (method ngoài bảng của object đó).
        let denied0 = invoke::stats().denied;
        let mutate = kinvoke(
            m,
            rig.tcb_s,
            rig.s,
            invoke::CNODE_MUTATE,
            2,
            [S_EP_REQ as u64, 0x80, 0, 0],
        ) == Err(KError::InvalidArgument.id() as u64);
        let rotate = kinvoke(m, rig.tcb_s, rig.s, invoke::CNODE_ROTATE, 2, [11, 12, 0, 0])
            == Err(KError::SlotEmpty.id() as u64);
        let other_object = invoke::dispatch(
            m,
            rig.tcb_s,
            SlotRef::new(rig.s, S_EP_REQ),
            info(CNODE_COPY, 2),
            &[S_EP_REQ as u64, 11, 0, 0],
            &mut CapaEnv,
        ) == Err(KError::NotSupported.id() as u64);
        let denied_ok = invoke::stats().denied >= denied0 + 3;
        // Thiếu/thừa MR ⇒ 5.
        let short = kinvoke(
            m,
            rig.tcb_s,
            rig.s,
            CNODE_COPY,
            1,
            [S_EP_REQ as u64, 11, 0, 0],
        ) == Err(KError::InvalidArgument.id() as u64);
        let long = kinvoke(
            m,
            rig.tcb_s,
            rig.s,
            CNODE_COPY,
            5,
            [S_EP_REQ as u64, 11, 0, 0],
        ) == Err(KError::InvalidArgument.id() as u64);
        // D2-104 — arity **chính xác**: thừa MR cho **từng** method ⇒ 5 (không bỏ qua).
        // Copy 2|3, Mint 2|3|4, Move 2, Delete 1, Revoke 0|1.
        let extra_cases: [(u32, u8); 10] = [
            (CNODE_COPY, 4),
            (CNODE_MINT, 5),
            (CNODE_MOVE, 3),
            (CNODE_MOVE, 4),
            (CNODE_DELETE, 2),
            (CNODE_DELETE, 3),
            (CNODE_DELETE, 4),
            (CNODE_REVOKE, 2),
            (CNODE_REVOKE, 3),
            (CNODE_REVOKE, 4),
        ];
        let mut arity_hits = 0u8;
        for (method, len) in extra_cases {
            if kinvoke(
                m,
                rig.tcb_s,
                rig.s,
                method,
                len,
                [S_EP_REQ as u64, 11, 0, 0],
            ) == Err(KError::InvalidArgument.id() as u64)
            {
                arity_hits += 1;
            }
        }
        // Không ca thừa nào được chạm máy trạng thái: đích 11 còn rỗng, EP slot 1 còn sống.
        let arity_ok = arity_hits == 10 && m.cap_at(rig.s, 11) == Err(KError::SlotEmpty);
        // Dọn cap READ-only rồi CPTR cũ ⇒ SlotEmpty (8) — "cap chết" ở mức ABI.
        let freed = kinvoke(m, rig.tcb_s, rig.s, CNODE_DELETE, 1, [10, 0, 0, 0]) == Ok((0, 0));
        let stale = invoke::dispatch(
            m,
            rig.tcb_s,
            SlotRef::new(rig.s, 10),
            info(CNODE_COPY, 2),
            &[S_EP_REQ as u64, 11, 0, 0],
            &mut CapaEnv,
        ) == Err(KError::SlotEmpty.id() as u64);
        // Cap chết **ngoài slot** (root_cap → drop_root_cap): `alive = 0`; không cap nào trỏ
        // object chết (bất biến `check_caps`).
        let dead_ok = {
            let ep = m.cap_at(rig.s, S_EP_REQ).ok();
            match ep.and_then(|cap| m.cap_obj(cap).ok()) {
                Some(obj) => {
                    let dead = m.root_cap(obj);
                    let dropped = m.drop_root_cap(dead).is_ok();
                    let info = m.cap_info(dead).ok().map(|c| c.alive);
                    dropped && info == Some(false) && m.check_caps().is_ok()
                }
                None => false,
            }
        };
        // Bảo đảm Frame cap và EP vẫn sống (không ca âm nào phá object).
        let live_ok = m.cap_at(rig.s, S_FRAME).is_ok()
            && m.cap_at(rig.s, S_EP_REQ).is_ok()
            && m.cap_at(rig.s, S_EP_RESP).is_ok();
        let caps_ok = m.check_caps().is_ok();
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# capa: am empty={} occupied={} rights={} range={} ro_cnode={} mutate={} rotate={} other={} denied={} short={} long={} arity={} stale={} dead_alive={} caps={}",
                empty as u8,
                occupied as u8,
                rights as u8,
                range as u8,
                ro_ok as u8,
                mutate as u8,
                rotate as u8,
                other_object as u8,
                denied_ok as u8,
                short as u8,
                long as u8,
                arity_hits,
                stale as u8,
                dead_ok as u8,
                caps_ok as u8
            );
        }
        empty
            && occupied
            && rights
            && range
            && ro_ok
            && mutate
            && rotate
            && other_object
            && denied_ok
            && short
            && long
            && arity_ok
            && freed
            && stale
            && dead_ok
            && live_ok
            && caps_ok
    });
    tap(fails, ok, 23, "capalloc ca am", "ca am tra sai ma");
}

// ---------------------------------------------------------------------------------------------
// TAP helpers
// ---------------------------------------------------------------------------------------------

fn tap(fails: &mut usize, ok: bool, number: u32, name: &str, reason: &str) {
    let mut w = PlatformWriter(&PLATFORM);
    if ok {
        let _ = writeln!(w, "ok {number} - {name}");
    } else {
        let _ = writeln!(w, "not ok {number} - {name}: {reason}");
        *fails += 1;
    }
}

fn fail(fails: &mut usize, reason: &str) {
    let mut w = PlatformWriter(&PLATFORM);
    let _ = writeln!(w, "not ok 20 - capalloc cap vong: {reason}");
    *fails += 1;
}
