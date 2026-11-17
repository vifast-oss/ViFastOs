// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-70** — VFS tầng 1: **pager + page cache** trên ảnh `vifast-kbin-ktest-user` (TAP 30..34).
//!
//! | TAP | Điều kiện đo |
//! |---|---|
//! | `ok 30 - pager fault den server va nap trang` | U-mode chạm trang chưa map ⇒ trap phần cứng phân loại `FaultKind::Vm{reason=NotMapped, access=Read}`; kernel giao fault cho **pager server U-mode** (`inject_fault` core + message IDL `pager.fault`); pager map Frame ⇒ walk client VSpace valid/leaf/U/PA ∈ pool; client chạy tiếp đọc **đúng dữ liệu** |
//! | `ok 31 - pager hit khong doc backend` | fault của client B vào `(0,2)`/`(0,4)` **hit** cache đã có từ client A: `reads` trước/sau **không đổi**, cùng một PA trong hai VSpace (nhiều mapping, một bản) |
//! | `ok 32 - pager eviction LRU khong mat du lieu` | vượt ngưỡng ⇒ `evictions>0`; VA bị trục xuất **mất mapping** (walk invalid) rồi nạp lại **đúng dữ liệu** (PA đổi, `reloads>0`, client A 5/5 trang OK) |
//! | `ok 33 - pager hai client chia se mot trang` | `clients=2`, hai trang chia sẻ cùng PA, `entries=unique_keys≤4`, cả hai client 0 byte sai |
//! | `ok 35 - pager ca am quyen va khong ghi de` | `Frame.Map` với aux = **CNode cap** (`P_SELF`, không phải VSpace) ⇒ `WrongObjectType` (2 — core kiểm loại cap phụ trước); `Frame.Map` vào VA **đã map** trong VSpace client B ⇒ `AlreadyMapped` (10), PTE khối kết quả **không đổi** (PA walk = PA cấp, magic còn nguyên) |
//! | `ok 34 - pager page ngoai file tra ma` | fault vào page 99 ⇒ pager trả **mã `InvalidArgument`** (tên+id từ spec), **không** đọc backend (reads không đổi), client B dừng với mã lỗi; dữ liệu 4 trang trước đó vẫn đúng |
//!
//! Bằng chứng `# pager:` do **kernel** in từ biến đo (D2-102): khối stats/results đọc qua
//! **physmap**, PTE walk từ object layer, `FaultKind` từ core (`take_last_fault`), hằng thiết kế
//! in kèm `const:`. Không `unsafe` trong tệp này.

use core::fmt::Write;

use vifast_core::{
    FaultAccess, FaultKind, KError, Machine, Message, MessageInfo, ObjId, ObjType, SlotRef,
    TcbState, VmFaultReason,
};
use vifast_idl::pager::{decode, encode_fault, Request, METHOD_FAULT, METHOD_STOP};
use vifast_plat_common::mmu::{PTE_READ, PTE_USER};
use vifast_plat_common::obj::{CapId, ObjKind};
use vifast_plat_common::trap::{TrapFrame, REG_SP};
use vifast_plat_common::PlatformWriter;
use vifast_plat_riscv64::core_bridge;
use vifast_plat_riscv64::mem_bridge;
use vifast_plat_riscv64::obj as aobj;
use vifast_plat_riscv64::user::{self, LoadedInit, PagerFault, Stop};
use vifast_plat_riscv64::Riscv64Virt;
use vifast_vfs::{byte_at, Stat, CACHE_CAPACITY, PAGE_SIZE};

#[path = "../init/pager_common.rs"]
mod common;

use common::{
    result, A_BASE, A_TABLE, B_BASE, B_FILE1_BASE, B_PROBE_VA, B_TABLE, C_RESULT, C_VSPACE,
    DISK_FRAMES, P_DISK0, P_EP_FAULT, P_EP_REPLY, P_POOL0, P_STATS, P_VSPACE, P_VS_A, P_VS_B,
    RESULT_MAGIC_B, VA_RESULT,
};

static PLATFORM: Riscv64Virt = Riscv64Virt;

static PAGER_SERVER_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/pager_server.elf"));
static PAGER_CLIENT_A_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/pager_client_a.elf"));
static PAGER_CLIENT_B_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/pager_client_b.elf"));

// ---------------------------------------------------------------------------------------------
// Bố cục (khớp `init/pager_common.rs` + chương trình U-mode)
// ---------------------------------------------------------------------------------------------

/// Ô bảng dải user của ba tác vụ (sau KT-69 dùng 1/2; pager dùng 1, A 2, B 3).
const LOAD_SLOT_PAGER: usize = 1;
const LOAD_SLOT_A: usize = 2;
const LOAD_SLOT_B: usize = 3;

/// Slot cspace **gốc** cho TCB/SC/Endpoint của pha pager (13..24 — các pha trước đã dọn).
const R_TCB_PAGER: usize = 13;
const R_TCB_A: usize = 14;
const R_TCB_B: usize = 15;
const R_TCB_KERN: usize = 16;
const R_SC_PAGER: usize = 17;
const R_SC_A: usize = 18;
const R_SC_B: usize = 19;
const R_EP_FAULT: usize = 20;
const R_EP_REPLY: usize = 21;
const R_CSPACE: usize = 22;

/// Slot cspace của client cho cap Endpoint fault.
const C_FAULT_EP: usize = 5;
/// Slot cspace của **kernel** (TCB điều phối): self, ep_fault, ep_reply.
const K_EP_FAULT: usize = 1;
const K_EP_REPLY: usize = 2;

/// `u64::MAX` = chưa có.
const NONE_U64: u64 = u64::MAX;

/// Rig của pha pager.
struct Rig {
    cn_kern: ObjId,
    tcb_pager: ObjId,
    tcb_a: ObjId,
    tcb_b: ObjId,
    tcb_kern: ObjId,
    vs_a: CapId,
    vs_b: CapId,
    core_vs_a: ObjId,
    core_vs_b: ObjId,
    stats_pa: u64,
    result_a_pa: u64,
    result_b_pa: u64,
    pool_pa: [u64; CACHE_CAPACITY],
}

/// Một tác vụ U-mode chạy tiếp được.
struct Task<'a> {
    loaded: &'a LoadedInit,
    tf: TrapFrame,
    tcb: ObjId,
    slot: usize,
    exited: bool,
    code: u64,
}

impl<'a> Task<'a> {
    fn new(loaded: &'a LoadedInit, tcb: ObjId, slot: usize) -> Self {
        let mut tf = TrapFrame::zeroed();
        tf.x[REG_SP] = user::USER_STACK_TOP;
        tf.sepc = loaded.entry;
        Self {
            loaded,
            tf,
            tcb,
            slot,
            exited: false,
            code: NONE_U64,
        }
    }
}

/// Đưa TCB `Inactive/Ready` → `Running{hart:0}` (như `ktest_capa`).
fn set_running(m: &mut Machine, tcb: ObjId) -> bool {
    if matches!(m.tcb_state(tcb), Ok(TcbState::Running { .. })) {
        return true;
    }
    if m.tcb_state(tcb) == Ok(TcbState::Inactive) && m.transition_tcb(tcb, TcbState::Ready).is_err()
    {
        return false;
    }
    m.transition_tcb(tcb, TcbState::Running { hart: 0 }).is_ok()
}

fn task_ready(m: &mut Machine, tcb: ObjId) {
    let _ = m.transition_tcb(tcb, TcbState::Ready);
}

fn abi_error_name(code: u16) -> &'static str {
    let mut index = 0u16;
    while index < vifast_abi::ERROR_COUNT as u16 {
        if vifast_abi::ERRORS[index as usize].1 == code {
            return vifast_abi::ERRORS[index as usize].0;
        }
        index += 1;
    }
    "?"
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

/// Đọc một bộ đếm trong khối stats qua **physmap**.
fn stat(pa: u64, s: Stat) -> u64 {
    aobj::machine()
        .read_phys_u64(pa + s.offset() as u64)
        .unwrap_or(0)
}

fn result_word(pa: u64, field: u64) -> u64 {
    aobj::machine().read_phys_u64(pa + field * 8).unwrap_or(0)
}

/// Quy VA của client về `(file, page)` theo đúng vùng đã cấu hình.
fn page_of(client: usize, va: u64) -> Option<(u32, u32)> {
    if client == 0 {
        if (A_BASE..A_BASE + 3 * 4096).contains(&va) {
            return Some((0, ((va - A_BASE) / 4096) as u32));
        }
        None
    } else if va == B_PROBE_VA {
        Some((0, 99))
    } else if (B_FILE1_BASE..B_FILE1_BASE + 2 * 4096).contains(&va) {
        Some((1, ((va - B_FILE1_BASE) / 4096) as u32))
    } else if (B_BASE..B_BASE + 3 * 4096).contains(&va) {
        Some((0, ((va - B_BASE) / 4096) as u32))
    } else {
        None
    }
}

fn va_of(client: usize, file: u32, page: u32) -> u64 {
    if client == 0 {
        A_BASE + u64::from(page) * 4096
    } else {
        common::b_va(file, page)
    }
}

/// Walk VSpace của client tại `va`: trả `Some((pa, raw))` nếu leaf `U|R` hợp lệ.
fn walk_client(vs: CapId, va: u64) -> Option<(u64, u64)> {
    let walk = aobj::machine().walk(vs, va).ok()?;
    if walk.valid && walk.leaf && walk.raw & PTE_USER != 0 && walk.raw & PTE_READ != 0 {
        Some((walk.phys, walk.raw))
    } else {
        None
    }
}

/// Tạo một Frame 4 KiB từ Untyped + trả `(key bắc cầu, PA)` — qua **object layer**.
fn new_frame(load_untyped: CapId, size_bits: u8) -> Option<(u64, u64)> {
    let m = aobj::machine();
    let cap = m.retype(load_untyped, ObjKind::Frame, size_bits).ok()?;
    let id = m.cap_key(cap).ok()?;
    let pa = m.frame_pa(cap).ok()?;
    Some((mem_bridge::make_key(cap, id), pa))
}

/// Dựng rig: ba CNode/TCB/SC, hai Endpoint, bảy Frame + cap VSpace cho pager/client.
fn build_rig(
    root: ObjId,
    us: SlotRef,
    ctl: SlotRef,
    load_untyped: CapId,
    stage: &mut &'static str,
) -> Option<(Rig, LoadedInit, LoadedInit, LoadedInit)> {
    *stage = "load pager";
    let pager = match user::load_init_slot(PAGER_SERVER_ELF, load_untyped, LOAD_SLOT_PAGER) {
        Ok(loaded) => loaded,
        Err(error) => {
            *stage = error.name();
            return None;
        }
    };
    *stage = "load client a";
    let client_a = match user::load_init_slot(PAGER_CLIENT_A_ELF, load_untyped, LOAD_SLOT_A) {
        Ok(loaded) => loaded,
        Err(error) => {
            *stage = error.name();
            return None;
        }
    };
    *stage = "load client b";
    let client_b = match user::load_init_slot(PAGER_CLIENT_B_ELF, load_untyped, LOAD_SLOT_B) {
        Ok(loaded) => loaded,
        Err(error) => {
            *stage = error.name();
            return None;
        }
    };

    // ---- object layer: key VSpace + 9 Frame (3 pool, 5 đĩa 4 KiB, stats, 2 result)
    *stage = "object keys";
    let vs_pager_key = {
        let m = aobj::machine();
        let id = m.cap_key(pager.vspace).ok()?;
        mem_bridge::make_key(pager.vspace, id)
    };
    let vs_a_key = {
        let m = aobj::machine();
        let id = m.cap_key(client_a.vspace).ok()?;
        mem_bridge::make_key(client_a.vspace, id)
    };
    let vs_b_key = {
        let m = aobj::machine();
        let id = m.cap_key(client_b.vspace).ok()?;
        mem_bridge::make_key(client_b.vspace, id)
    };
    *stage = "object frames";
    let mut pool_pa = [0u64; CACHE_CAPACITY];
    let mut frame_keys = [0u64; CACHE_CAPACITY];
    let mut index = 0usize;
    while index < CACHE_CAPACITY {
        let (key, pa) = new_frame(load_untyped, 12)?;
        frame_keys[index] = key;
        pool_pa[index] = pa;
        index += 1;
    }
    let mut disk_keys = [0u64; DISK_FRAMES as usize];
    let mut disk_index = 0usize;
    while disk_index < DISK_FRAMES as usize {
        let (key, _pa) = new_frame(load_untyped, 12)?;
        disk_keys[disk_index] = key;
        disk_index += 1;
    }
    let (stats_key, stats_pa) = new_frame(load_untyped, 12)?;
    let (result_a_key, result_a_pa) = new_frame(load_untyped, 12)?;
    let (result_b_key, result_b_pa) = new_frame(load_untyped, 12)?;

    let rig = core_bridge::with_machine(|m| -> Option<Rig> {
        *stage = "cnodes";
        let cn_pager = m.add_cnode(32);
        let cn_a = m.add_cnode(16);
        let cn_b = m.add_cnode(16);
        let cn_kern = m.add_cnode(16);
        // Slot 0 của mỗi CNode = cap **CNode** của chính nó (không phải TCB) — pager dùng
        // `P_SELF` làm aux sai loại cho ca âm TAP 35.
        for cn in [cn_pager, cn_a, cn_b, cn_kern] {
            let self_cap = m.root_cap(cn);
            m.put_cap(cn, 0, self_cap).ok()?;
        }
        let root_self = m.root_cap(root);
        m.put_cap(root, R_CSPACE, root_self).ok()?;
        *stage = "endpoints";
        let ep_fault_cap = m.retype(us, ObjType::Endpoint, 0, 0).ok()?;
        let ep_reply_cap = m.retype(us, ObjType::Endpoint, 0, 0).ok()?;
        m.put_cap(root, R_EP_FAULT, ep_fault_cap).ok()?;
        m.put_cap(root, R_EP_REPLY, ep_reply_cap).ok()?;
        m.copy(
            SlotRef::new(root, R_EP_FAULT),
            SlotRef::new(cn_pager, P_EP_FAULT as usize),
        )
        .ok()?;
        m.copy(
            SlotRef::new(root, R_EP_REPLY),
            SlotRef::new(cn_pager, P_EP_REPLY as usize),
        )
        .ok()?;
        m.copy(
            SlotRef::new(root, R_EP_FAULT),
            SlotRef::new(cn_kern, K_EP_FAULT),
        )
        .ok()?;
        m.copy(
            SlotRef::new(root, R_EP_REPLY),
            SlotRef::new(cn_kern, K_EP_REPLY),
        )
        .ok()?;
        m.copy(
            SlotRef::new(root, R_EP_FAULT),
            SlotRef::new(cn_a, C_FAULT_EP),
        )
        .ok()?;
        m.copy(
            SlotRef::new(root, R_EP_FAULT),
            SlotRef::new(cn_b, C_FAULT_EP),
        )
        .ok()?;

        *stage = "vspaces";
        let vs_pager_obj = m.add_foreign_object(ObjType::VSpace, vs_pager_key).ok()?;
        let vs_a_obj = m.add_foreign_object(ObjType::VSpace, vs_a_key).ok()?;
        let vs_b_obj = m.add_foreign_object(ObjType::VSpace, vs_b_key).ok()?;
        {
            let cap = m.root_cap(vs_pager_obj);
            m.put_cap(cn_pager, P_VSPACE as usize, cap)
        }
        .ok()?;
        {
            let cap = m.root_cap(vs_a_obj);
            m.put_cap(cn_pager, P_VS_A as usize, cap)
        }
        .ok()?;
        {
            let cap = m.root_cap(vs_b_obj);
            m.put_cap(cn_pager, P_VS_B as usize, cap)
        }
        .ok()?;
        {
            let cap = m.root_cap(vs_a_obj);
            m.put_cap(cn_a, C_VSPACE as usize, cap)
        }
        .ok()?;
        {
            let cap = m.root_cap(vs_b_obj);
            m.put_cap(cn_b, C_VSPACE as usize, cap)
        }
        .ok()?;

        *stage = "frame caps";
        for (slot, frame_key) in frame_keys.iter().enumerate() {
            let obj = m.add_foreign_object(ObjType::Frame, *frame_key).ok()?;
            {
                let cap = m.root_cap(obj);
                m.put_cap(cn_pager, P_POOL0 as usize + slot, cap)
            }
            .ok()?;
        }
        for (slot, disk_key) in disk_keys.iter().enumerate() {
            let obj = m.add_foreign_object(ObjType::Frame, *disk_key).ok()?;
            let cap = m.root_cap(obj);
            m.put_cap(cn_pager, P_DISK0 as usize + slot, cap).ok()?;
        }
        let stats_obj = m.add_foreign_object(ObjType::Frame, stats_key).ok()?;
        {
            let cap = m.root_cap(stats_obj);
            m.put_cap(cn_pager, P_STATS as usize, cap)
        }
        .ok()?;
        let result_a_obj = m.add_foreign_object(ObjType::Frame, result_a_key).ok()?;
        {
            let cap = m.root_cap(result_a_obj);
            m.put_cap(cn_a, C_RESULT as usize, cap)
        }
        .ok()?;
        let result_b_obj = m.add_foreign_object(ObjType::Frame, result_b_key).ok()?;
        {
            let cap = m.root_cap(result_b_obj);
            m.put_cap(cn_b, C_RESULT as usize, cap)
        }
        .ok()?;

        *stage = "tbcs";
        let mut tcb_objs = [0u32; 4];
        let plans = [
            (R_TCB_PAGER, R_SC_PAGER, cn_pager),
            (R_TCB_A, R_SC_A, cn_a),
            (R_TCB_B, R_SC_B, cn_b),
            (R_TCB_KERN, R_SC_PAGER, cn_kern),
        ];
        for (index, (tcb_slot, sc_slot, cnode)) in plans.into_iter().enumerate() {
            let tcb_cap = match m.retype(us, ObjType::Tcb, 0, 0) {
                Ok(cap) => cap,
                Err(error) => {
                    *stage = "tcb retype fail";
                    let mut w = PlatformWriter(&PLATFORM);
                    let _ = writeln!(w, "# pager: tcb retype err={:?} index={index}", error);
                    return None;
                }
            };
            let tcb = m.cap_obj(tcb_cap).ok()?;
            m.put_cap(root, tcb_slot, tcb_cap).ok()?;
            if index != 3 {
                let sc_cap = m.retype(us, ObjType::SchedContext, 0, 0).ok()?;
                m.put_cap(root, sc_slot, sc_cap).ok()?;
            }
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
            if index == 3 {
                m.transition_tcb(tcb, TcbState::Ready).ok()?;
            }
            tcb_objs[index] = tcb;
        }
        m.tcb_set_fault_ep(SlotRef::new(root, R_TCB_A), SlotRef::new(cn_a, C_FAULT_EP))
            .ok()?;
        m.tcb_set_fault_ep(SlotRef::new(root, R_TCB_B), SlotRef::new(cn_b, C_FAULT_EP))
            .ok()?;
        Some(Rig {
            cn_kern,
            tcb_pager: tcb_objs[0],
            tcb_a: tcb_objs[1],
            tcb_b: tcb_objs[2],
            tcb_kern: tcb_objs[3],
            vs_a: client_a.vspace,
            vs_b: client_b.vspace,
            core_vs_a: vs_a_obj,
            core_vs_b: vs_b_obj,
            stats_pa,
            result_a_pa,
            result_b_pa,
            pool_pa,
        })
    });
    rig.map(|rig| (rig, pager, client_a, client_b))
}

/// Chạy một tác vụ U-mode tới `Yield`/`Exit`/`Fault`.
fn run_task(task: &mut Task<'_>) -> Stop {
    user::set_active_slot(task.slot);
    user::bind_core_tcb(task.tcb);
    let entered = core_bridge::with_machine(|m| set_running(m, task.tcb));
    if !entered {
        return Stop::Blocked(TcbState::Inactive);
    }
    let stop = user::run_task(task.loaded, &mut task.tf);
    task.tf = user::saved_tf();
    match stop {
        Stop::Yield => core_bridge::with_machine(|m| task_ready(m, task.tcb)),
        Stop::Exit { code } => {
            task.exited = true;
            task.code = code;
        }
        _ => {}
    }
    stop
}

/// Kết quả một fault đã giao pager.
struct Served {
    status: u32,
    /// Pager đã tiêu thụ fault entry của **core** (`last_fault`) trong lúc phục vụ fault này.
    core_fault: bool,
}

/// Gửi `fault` bằng IDL + giao fault cho pager qua core, rồi chạy pager tới `served`.
///
/// **Không** giữ khoá `core_bridge::with_machine` trong lúc `run_task` (khoá không tái nhập):
/// mọi thao tác core đi qua closure ngắn, `run_task` chạy ngoài khoá.
fn deliver_and_pump(
    rig: &Rig,
    client: usize,
    file: u32,
    page: u32,
    va: u64,
    pager: &mut Task<'_>,
) -> Option<Served> {
    // 1) core: `FaultKind::Vm` đi fault_ep của client (ghi `last_fault` cho pager + đếm fault).
    let tcb = if client == 0 { rig.tcb_a } else { rig.tcb_b };
    let vspace = if client == 0 {
        rig.core_vs_a
    } else {
        rig.core_vs_b
    };
    core_bridge::with_machine(|m| {
        task_ready(m, tcb);
        m.inject_fault(
            tcb,
            FaultKind::Vm {
                vspace,
                va,
                access: FaultAccess::Read,
                reason: VmFaultReason::NotMapped,
            },
        )
    })
    .ok()?;
    // 2) message IDL `pager.fault` (4 MR) — kernel dùng chính stub sinh từ `pager.vidl`.
    let (_packed, mr) = encode_fault(client as u32, file, page, va);
    let words = [mr[0], mr[1], mr[2], mr[3]];
    let info = MessageInfo::new(METHOD_FAULT, 0, false, 0, 4).ok()?;
    let message = Message::new(info, &words).ok()?;
    core_bridge::with_machine(|m| {
        task_ready(m, rig.tcb_kern);
        m.nb_send(rig.tcb_kern, SlotRef::new(rig.cn_kern, K_EP_FAULT), message)
    })
    .ok()?;
    // 3) chạy pager tới khi kernel nhận `served` (tối đa 8 bước).
    let mut core_fault = false;
    for _ in 0..8 {
        let stop = run_task(pager);
        if matches!(stop, Stop::Fault) {
            return None;
        }
        if !core_fault {
            if let Ok(Some((_sender, kind))) =
                core_bridge::with_machine(|m| m.take_last_fault(rig.tcb_pager))
            {
                if matches!(kind, FaultKind::Vm { .. }) {
                    core_fault = true;
                }
            }
        }
        let answer = core_bridge::with_machine(|m| {
            task_ready(m, rig.tcb_kern);
            m.nb_recv(rig.tcb_kern, SlotRef::new(rig.cn_kern, K_EP_REPLY))
        });
        let Ok(Some(message)) = answer else {
            continue;
        };
        let mr = [message.mr[0], message.mr[1], message.mr[2], message.mr[3]];
        if let Some(Request::Served {
            page: got_page,
            status,
            ..
        }) = decode(message.info.label(), message.info.length(), &mr)
        {
            if got_page == page {
                return Some(Served { status, core_fault });
            }
        }
    }
    None
}

/// Bằng chứng đo cho các TAP (mọi số từ biến — D2-102).
#[derive(Default)]
struct Evidence {
    first_fault: Option<(u64, u64, u64, u64)>, // client, va, access, reason
    core_fault_seen: bool,
    served_ok: u32,
    rejected: u32,
    reject_client: u64,
    reject_page: u64,
    reject_status: u64,
    reject_reads_before: u64,
    reject_reads_after: u64,
    reject_exit: u64,
    mapping_bad: u32,
    hit_delta_zero: bool,
    hit_checked: bool,
    hit_reads_before: u64,
    hit_reads_after: u64,
    pa_p0_first: u64,
    pa_p0_reload: u64,
    neg_aux: u64,
    neg_over: u64,
    neg_pa_same: bool,
    neg_walk_pa: u64,
    neg_magic: u64,
    p2_equal: bool,
    evict_seen: bool,
    evict_walk_gone: bool,
    evict_reload_pa_changed: bool,
    reloads: u64,
    kernel_data_ok: u32,
    kernel_data_bad: u32,
    a_ok_pages: u64,
    a_bad_pages: u64,
    a_ok_bytes: u64,
    a_bad_bytes: u64,
    b_ok_pages: u64,
    b_bad_pages: u64,
    b_ok_bytes: u64,
    b_bad_bytes: u64,
    pager_exit: u64,
    a_exit: u64,
    b_exit: u64,
    steps: usize,
    fault_kind_name: &'static str,
    access_name: &'static str,
    reason_name: &'static str,
    a_pages_done: usize,
    b_pages_done: usize,
}

/// Trạng thái tracking mapping theo client, **theo trang** (PA đã thấy gần nhất + ever-seen).
struct Mapping {
    a_page_pa: [u64; 3],
    a_page_seen: [bool; 3],
    b0_page_pa: [u64; 3],
    b0_page_seen: [bool; 3],
    b1_page_pa: [u64; 2],
    b1_page_seen: [bool; 2],
}

impl Mapping {
    fn new() -> Self {
        Self {
            a_page_pa: [NONE_U64; 3],
            a_page_seen: [false; 3],
            b0_page_pa: [NONE_U64; 3],
            b0_page_seen: [false; 3],
            b1_page_pa: [NONE_U64; 2],
            b1_page_seen: [false; 2],
        }
    }

    fn set_page(&mut self, client: usize, file: u32, page: u32, pa: u64) -> bool {
        // true ⇔ trang **đã từng** được map với PA khác (nạp lại sau trục xuất).
        if client == 0 {
            let idx = page as usize;
            let changed = self.a_page_seen[idx] && self.a_page_pa[idx] != pa;
            self.a_page_pa[idx] = pa;
            self.a_page_seen[idx] = true;
            changed
        } else if file == 0 {
            let idx = page as usize;
            let changed = self.b0_page_seen[idx] && self.b0_page_pa[idx] != pa;
            self.b0_page_pa[idx] = pa;
            self.b0_page_seen[idx] = true;
            changed
        } else {
            let idx = page as usize;
            let changed = self.b1_page_seen[idx] && self.b1_page_pa[idx] != pa;
            self.b1_page_pa[idx] = pa;
            self.b1_page_seen[idx] = true;
            changed
        }
    }

    fn page_pa(&self, client: usize, file: u32, page: u32) -> u64 {
        if client == 0 {
            self.a_page_pa[page as usize]
        } else if file == 0 {
            self.b0_page_pa[page as usize]
        } else {
            self.b1_page_pa[page as usize]
        }
    }
}

/// Kết quả một bước chạy client.
enum StepOut {
    Yield,
    Fault,
    Exit(u64),
    Blocked,
    Halt,
}

/// Chạy một bước của client; nếu fault: giao pager, đo mapping/dữ liệu, KHÔNG resume ngay
/// (lần chạy kế tiếp của client sẽ thử lại instruction lỗi).
#[allow(clippy::too_many_arguments)]
fn step_client(
    rig: &Rig,
    pager: &mut Task<'_>,
    task: &mut Task<'_>,
    client: usize,
    step: &mut usize,
    map: &mut Mapping,
    ev: &mut Evidence,
    steps: &mut usize,
) -> StepOut {
    let stop = run_task(task);
    *steps += 1;
    match stop {
        Stop::Yield => StepOut::Yield,
        Stop::Exit { code } => {
            if client == 0 {
                ev.a_exit = code;
            } else {
                ev.b_exit = code;
            }
            StepOut::Exit(code)
        }
        Stop::Blocked(_) => StepOut::Blocked,
        Stop::Halt { .. } => StepOut::Halt,
        Stop::Fault => {
            let fault: PagerFault = user::pager_fault();
            let Some((file, page)) = page_of(client, fault.va) else {
                ev.mapping_bad += 1;
                return StepOut::Fault;
            };
            let expect = if client == 0 {
                A_TABLE.get(*step).copied()
            } else {
                B_TABLE.get(*step).copied()
            };
            if expect != Some((file, page)) {
                ev.mapping_bad += 1;
                return StepOut::Fault;
            }
            if ev.first_fault.is_none() {
                ev.first_fault = Some((client as u64, fault.va, fault.access, fault.reason));
            }
            let reads_before = stat(rig.stats_pa, Stat::Reads);
            let Some(served) = deliver_and_pump(rig, client, file, page, fault.va, pager) else {
                ev.mapping_bad += 1;
                return StepOut::Fault;
            };
            if served.core_fault && !ev.core_fault_seen {
                ev.core_fault_seen = true;
                ev.fault_kind_name = "Vm";
                ev.access_name = FaultAccess::Read.name();
                ev.reason_name = VmFaultReason::NotMapped.name();
            }
            let reads_after = stat(rig.stats_pa, Stat::Reads);
            if served.status == 0 {
                ev.served_ok += 1;
                let vs = if client == 0 { rig.vs_a } else { rig.vs_b };
                let map_now = walk_client(vs, fault.va);
                let Some((pa_now, _raw)) = map_now else {
                    ev.mapping_bad += 1;
                    return StepOut::Fault;
                };
                if !rig.pool_pa.contains(&pa_now) {
                    ev.mapping_bad += 1;
                }
                // Nạp lại sau trục xuất: cùng (client,file,page) nhưng PA **đổi**.
                if map.set_page(client, file, page, pa_now) {
                    ev.evict_reload_pa_changed = true;
                }
                if client == 0 && file == 0 && page == 0 {
                    if ev.pa_p0_first == NONE_U64 {
                        ev.pa_p0_first = pa_now;
                    } else {
                        ev.pa_p0_reload = pa_now;
                    }
                }
                // Trục xuất: một (client,page) **đã từng** map nay không còn mapping (walk invalid),
                // trừ chính trang vừa được phục vụ.
                let mut evicted = false;
                for page_seen in 0..3u32 {
                    if map.a_page_seen[page_seen as usize]
                        && !(client == 0 && page_seen == page)
                        && walk_client(rig.vs_a, va_of(0, 0, page_seen)).is_none()
                    {
                        evicted = true;
                    }
                }
                for page_seen in 0..3u32 {
                    if map.b0_page_seen[page_seen as usize]
                        && !(client == 1 && file == 0 && page_seen == page)
                        && walk_client(rig.vs_b, va_of(1, 0, page_seen)).is_none()
                    {
                        evicted = true;
                    }
                }
                for page_seen in 0..2u32 {
                    if map.b1_page_seen[page_seen as usize]
                        && !(client == 1 && file == 1 && page_seen == page)
                        && walk_client(rig.vs_b, va_of(1, 1, page_seen)).is_none()
                    {
                        evicted = true;
                    }
                }
                if evicted {
                    ev.evict_seen = true;
                    ev.evict_walk_gone = true;
                }
                // Hit: `reads` không đổi trong lúc pager phục vụ fault này.
                if reads_after == reads_before {
                    ev.hit_checked = true;
                    ev.hit_delta_zero = true;
                    ev.hit_reads_before = reads_before;
                    ev.hit_reads_after = reads_after;
                }
                // p2 chia sẻ: PA hai phía bằng nhau ⇒ **một** Frame, nhiều mapping.
                if file == 0 && page == 2 {
                    if client == 0 {
                        if map.b0_page_seen[2] && map.b0_page_pa[2] == map.a_page_pa[2] {
                            ev.p2_equal = true;
                        }
                    } else if map.a_page_seen[2] && map.a_page_pa[2] == map.b0_page_pa[2] {
                        ev.p2_equal = true;
                    }
                }
                // Dữ liệu kernel đọc **độc lập** từ Frame qua physmap, so mẫu `byte_at`.
                let mut bad = 0u32;
                let mut off = 0usize;
                while off < 64 {
                    let word = aobj::machine()
                        .read_phys_u64(pa_now + (off as u64 / 8) * 8)
                        .unwrap_or(0);
                    let byte = (word >> ((off as u64 % 8) * 8)) as u8;
                    if byte != byte_at(file, page, off) {
                        bad += 1;
                    }
                    off += 1;
                }
                if bad == 0 {
                    ev.kernel_data_ok += 1;
                } else {
                    ev.kernel_data_bad += 1;
                }
            } else {
                ev.rejected += 1;
                ev.reject_client = client as u64;
                ev.reject_page = u64::from(page);
                ev.reject_status = u64::from(served.status);
                ev.reject_reads_before = reads_before;
                ev.reject_reads_after = reads_after;
                ev.reject_exit = 0x8000_0000 | u64::from(served.status);
                task.exited = true;
                task.code = ev.reject_exit;
                if client == 0 {
                    ev.a_exit = ev.reject_exit;
                } else {
                    ev.b_exit = ev.reject_exit;
                }
            }
            *step += 1;
            StepOut::Fault
        }
    }
}

/// Chạy tới khi client hoàn tất `target` trang (mỗi trang = một `Yield` sau validate).
#[allow(clippy::too_many_arguments)]
fn advance_to(
    rig: &Rig,
    pager: &mut Task<'_>,
    task: &mut Task<'_>,
    client: usize,
    target: usize,
    step: &mut usize,
    map: &mut Mapping,
    ev: &mut Evidence,
    steps: &mut usize,
) {
    let mut guard = 0usize;
    while guard < 40 {
        guard += 1;
        if task.exited {
            return;
        }
        match step_client(rig, pager, task, client, step, map, ev, steps) {
            StepOut::Yield => {
                if client == 0 {
                    ev.a_pages_done += 1;
                } else {
                    ev.b_pages_done += 1;
                }
                let done = if client == 0 {
                    ev.a_pages_done
                } else {
                    ev.b_pages_done
                };
                if done >= target {
                    return;
                }
            }
            StepOut::Fault => {}
            StepOut::Exit(code) => {
                if client == 0 {
                    ev.a_exit = code;
                } else {
                    ev.b_exit = code;
                }
                return;
            }
            StepOut::Blocked | StepOut::Halt => {
                ev.mapping_bad += 1;
                return;
            }
        }
    }
}

/// Pha **pager + page cache**: TAP 30..34.
pub fn pager_phase(root: ObjId, us: SlotRef, ctl: SlotRef, load_untyped: CapId, fails: &mut usize) {
    let mut stage = "start";
    let Some((rig, pager_loaded, a_loaded, b_loaded)) =
        build_rig(root, us, ctl, load_untyped, &mut stage)
    else {
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(w, "# pager: rig fail stage={stage}");
            let core = core_bridge::with_machine(|m| {
                (
                    m.object_count(),
                    m.cap_count(),
                    m.live_cap_count(),
                    m.cnode_slot_count(root).unwrap_or(0),
                )
            });
            let _ = writeln!(
                w,
                "# pager: rig fail core objects={} caps={} live_caps={} root_slots={}",
                core.0, core.1, core.2, core.3
            );
            match aobj::machine().check_objmem() {
                Ok(stats) => {
                    let _ = writeln!(
                        w,
                        "# pager: rig fail counts objects={} caps={} pt={} frames={} vs={} maps={}",
                        stats.counts.objects,
                        stats.counts.caps,
                        stats.counts.page_tables,
                        stats.counts.frames,
                        stats.counts.vspaces,
                        stats.counts.mappings
                    );
                }
                Err(error) => {
                    let _ = writeln!(w, "# pager: rig fail check_objmem={:?}", error);
                }
            }
            if let Ok(stats) = aobj::machine().untyped_stats(load_untyped) {
                let _ = writeln!(
                    w,
                    "# pager: rig fail u0 used={} free={} water={} frames={} objects={} holes={}",
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
            (30, "pager fault den server va nap trang"),
            (31, "pager hit khong doc backend"),
            (32, "pager eviction LRU khong mat du lieu"),
            (33, "pager hai client chia se mot trang"),
            (34, "pager page ngoai file tra ma"),
            (35, "pager ca am quyen va khong ghi de"),
        ] {
            tap(fails, false, number, name, "dung rig pager that bai");
        }
        return;
    };

    let mut pager = Task::new(&pager_loaded, rig.tcb_pager, LOAD_SLOT_PAGER);
    let mut client_a = Task::new(&a_loaded, rig.tcb_a, LOAD_SLOT_A);
    let mut client_b = Task::new(&b_loaded, rig.tcb_b, LOAD_SLOT_B);
    // F1: `Default` cho `pa_p0_first = 0`, nhưng code so với `NONE_U64` ⇒ khởi tạo tường minh
    // qua struct-update (không gán sau `default()` — clippy `field_reassign_with_default`).
    let mut ev = Evidence {
        pa_p0_first: NONE_U64,
        ..Evidence::default()
    };
    let mut map = Mapping::new();
    let mut steps = 0usize;
    let mut a_step = 0usize;
    let mut b_step = 0usize;

    user::enable_multitask(true);
    user::set_pager_faults(true);

    // Pager khởi động: map stats/pool + `ready`.
    let mut ready = false;
    for _ in 0..6 {
        let stop = run_task(&mut pager);
        steps += 1;
        if matches!(stop, Stop::Exit { .. } | Stop::Fault) {
            break;
        }
        core_bridge::with_machine(|m| task_ready(m, rig.tcb_kern));
        if let Ok(Some(message)) = core_bridge::with_machine(|m| {
            m.nb_recv(rig.tcb_kern, SlotRef::new(rig.cn_kern, K_EP_REPLY))
        }) {
            let mr = [message.mr[0], message.mr[1], message.mr[2], message.mr[3]];
            if let Some(Request::Ready) = decode(message.info.label(), message.info.length(), &mr) {
                ready = true;
                break;
            }
        }
    }

    // Hai client: map khối kết quả + tới `Yield` đầu tiên (chưa chạm trang nào).
    let _ = step_client(
        &rig,
        &mut pager,
        &mut client_a,
        0,
        &mut a_step,
        &mut map,
        &mut ev,
        &mut steps,
    );
    let _ = step_client(
        &rig,
        &mut pager,
        &mut client_b,
        1,
        &mut b_step,
        &mut map,
        &mut ev,
        &mut steps,
    );

    // A: 3 trang đầu (p0,p1,p2) — chưa vượt ngưỡng cache 3.
    advance_to(
        &rig,
        &mut pager,
        &mut client_a,
        0,
        3,
        &mut a_step,
        &mut map,
        &mut ev,
        &mut steps,
    );
    // B: (0,2) **hit** cache của A, rồi (1,0),(1,1) miss (mỗi miss có thể trục xuất LRU).
    advance_to(
        &rig,
        &mut pager,
        &mut client_b,
        1,
        3,
        &mut b_step,
        &mut map,
        &mut ev,
        &mut steps,
    );
    // A: p0 **nạp lại** sau khi B trục xuất, p2 nạp lại sau khi B trục xuất tiếp.
    advance_to(
        &rig,
        &mut pager,
        &mut client_a,
        0,
        5,
        &mut a_step,
        &mut map,
        &mut ev,
        &mut steps,
    );
    // B: (0,2) **hit lần hai** — A vừa nạp lại p2 nên cùng Frame, khác mapping.
    advance_to(
        &rig,
        &mut pager,
        &mut client_b,
        1,
        4,
        &mut b_step,
        &mut map,
        &mut ev,
        &mut steps,
    );
    // A: hết bảng ⇒ DONE + Exit(0).
    let mut guard = 0usize;
    while guard < 6 && !client_a.exited {
        guard += 1;
        let _ = step_client(
            &rig,
            &mut pager,
            &mut client_a,
            0,
            &mut a_step,
            &mut map,
            &mut ev,
            &mut steps,
        );
    }
    // B: trang ngoài file ⇒ pager từ chối, kernel dừng client B (không resume).
    if !client_b.exited {
        let _ = step_client(
            &rig,
            &mut pager,
            &mut client_b,
            1,
            &mut b_step,
            &mut map,
            &mut ev,
            &mut steps,
        );
    }

    // Kết thúc: gửi `stop`, chạy pager tới `Exit(0)` (có thể còn fault Exit của client trong hàng).
    {
        let info = MessageInfo::new(METHOD_STOP, 0, false, 0, 0).expect("info stop");
        let message = Message::new(info, &[]).expect("message stop");
        let _ = core_bridge::with_machine(|m| {
            task_ready(m, rig.tcb_kern);
            m.nb_send(rig.tcb_kern, SlotRef::new(rig.cn_kern, K_EP_FAULT), message)
        });
    }
    let mut guard = 0usize;
    while guard < 24 {
        guard += 1;
        let stop = run_task(&mut pager);
        steps += 1;
        match stop {
            Stop::Exit { code } => {
                ev.pager_exit = code;
                break;
            }
            Stop::Yield | Stop::Blocked(_) => {}
            _ => break,
        }
        core_bridge::with_machine(|m| task_ready(m, rig.tcb_kern));
        let _ = core_bridge::with_machine(|m| {
            m.nb_recv(rig.tcb_kern, SlotRef::new(rig.cn_kern, K_EP_REPLY))
        });
    }
    user::set_pager_faults(false);
    user::enable_multitask(false);
    ev.steps = steps;
    ev.reloads = stat(rig.stats_pa, Stat::Reloads);
    ev.a_ok_pages = result_word(rig.result_a_pa, result::OK_PAGES);
    ev.a_bad_pages = result_word(rig.result_a_pa, result::BAD_PAGES);
    ev.a_ok_bytes = result_word(rig.result_a_pa, result::OK_BYTES);
    ev.a_bad_bytes = result_word(rig.result_a_pa, result::BAD_BYTES);
    ev.b_ok_pages = result_word(rig.result_b_pa, result::OK_PAGES);
    ev.b_bad_pages = result_word(rig.result_b_pa, result::BAD_PAGES);
    ev.b_ok_bytes = result_word(rig.result_b_pa, result::OK_BYTES);
    ev.b_bad_bytes = result_word(rig.result_b_pa, result::BAD_BYTES);
    let hits = stat(rig.stats_pa, Stat::Hits);
    let misses = stat(rig.stats_pa, Stat::Misses);
    let evictions = stat(rig.stats_pa, Stat::Evictions);
    let reads = stat(rig.stats_pa, Stat::Reads);
    let sectors = stat(rig.stats_pa, Stat::Sectors);
    let served = stat(rig.stats_pa, Stat::Served);
    let rejected = stat(rig.stats_pa, Stat::Rejected);
    let entries = stat(rig.stats_pa, Stat::Entries);
    let unique = stat(rig.stats_pa, Stat::UniqueKeys);
    let shared = stat(rig.stats_pa, Stat::SharedHits);
    let faults = stat(rig.stats_pa, Stat::Faults);
    let clients = stat(rig.stats_pa, Stat::Clients);
    let magic = stat(rig.stats_pa, Stat::Magic);
    let (_, _, access_code, reason_code) = ev.first_fault.unwrap_or((0, 0, 0, 0));
    let invalid_argument = u64::from(KError::InvalidArgument.id());
    let already_mapped = u64::from(KError::AlreadyMapped.id());
    ev.neg_aux = stat(rig.stats_pa, Stat::NegAux);
    ev.neg_over = stat(rig.stats_pa, Stat::NegOver);
    match walk_client(rig.vs_b, VA_RESULT) {
        Some((pa, _)) => {
            ev.neg_walk_pa = pa;
            ev.neg_pa_same = pa == rig.result_b_pa;
        }
        None => ev.neg_pa_same = false,
    }
    ev.neg_magic = result_word(rig.result_b_pa, result::MAGIC);
    // Trang đã bị trục xuất (còn thấy trong `seen` nhưng VA không còn mapping) — đo tại chỗ.
    let mut evicted: Option<(u32, u32, u64)> = None;
    for page_seen in 0..3u32 {
        if evicted.is_none()
            && map.a_page_seen[page_seen as usize]
            && walk_client(rig.vs_a, va_of(0, 0, page_seen)).is_none()
        {
            evicted = Some((0, page_seen, va_of(0, 0, page_seen)));
        }
    }
    for page_seen in 0..3u32 {
        if evicted.is_none()
            && map.b0_page_seen[page_seen as usize]
            && walk_client(rig.vs_b, va_of(1, 0, page_seen)).is_none()
        {
            evicted = Some((0, page_seen, va_of(1, 0, page_seen)));
        }
    }
    for page_seen in 0..2u32 {
        if evicted.is_none()
            && map.b1_page_seen[page_seen as usize]
            && walk_client(rig.vs_b, va_of(1, 1, page_seen)).is_none()
        {
            evicted = Some((1, page_seen, va_of(1, 1, page_seen)));
        }
    }
    let obj_counts = aobj::machine().check_objmem().ok().map(|s| s.counts);
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# pager: objmem objects={} caps={} frames={} pt={} vs={} maps={}",
            obj_counts.map(|c| c.objects).unwrap_or(0),
            obj_counts.map(|c| c.caps).unwrap_or(0),
            obj_counts.map(|c| c.frames).unwrap_or(0),
            obj_counts.map(|c| c.page_tables).unwrap_or(0),
            obj_counts.map(|c| c.vspaces).unwrap_or(0),
            obj_counts.map(|c| c.mappings).unwrap_or(0)
        );
        let _ = writeln!(
            w,
            "# pager: fault kind={} va=0x{:x} client={} access={} reason={} core_fault={} faults={} ready={} const:page={} const:sector={}",
            ev.fault_kind_name,
            ev.first_fault.map(|f| f.1).unwrap_or(0),
            ev.first_fault.map(|f| f.0).unwrap_or(0),
            ev.access_name,
            ev.reason_name,
            u8::from(ev.core_fault_seen),
            faults,
            u8::from(ready),
            PAGE_SIZE,
            vifast_vfs::SECTOR_SIZE
        );
        let _ = writeln!(
            w,
            "# pager: cache const:capacity={} hits={} misses={} evictions={} reads={} sectors={} served={} rejected={} entries={} unique={} (=entries({}) nếu keys_unique) shared={} reloads={} const:clients={} magic={:#x} const:magic_expected=0x{:x}",
            CACHE_CAPACITY, hits, misses, evictions, reads, sectors, served, rejected, entries, unique, entries, shared, ev.reloads, clients, magic, vifast_vfs::STATS_MAGIC
        );
        let _ = writeln!(
            w,
            "# pager: map p2_a={:#x} p2_b={:#x} p2_same={} (=p2_a==p2_b) hit_delta0={} (reads_before={} reads_after={}) mapping_bad={} walks_ok={}",
            map.page_pa(0, 0, 2),
            map.page_pa(1, 0, 2),
            u8::from(ev.p2_equal),
            u8::from(ev.hit_delta_zero),
            ev.hit_reads_before,
            ev.hit_reads_after,
            ev.mapping_bad,
            ev.kernel_data_ok
        );
        let _ = writeln!(
            w,
            "# pager: evict evictions={} (=misses({})-min(capacity({}),unique({}))) walk_gone={} (file={} page={} va=0x{:x}) pa_changed={} (file=0 page=0 first=0x{:x} reload=0x{:x}) reloads={} kernel_data_ok={} kernel_data_bad={} a_done={} b_done={}",
            evictions,
            misses,
            CACHE_CAPACITY,
            unique,
            u8::from(ev.evict_walk_gone),
            evicted.map(|e| e.0).unwrap_or(0),
            evicted.map(|e| e.1).unwrap_or(0),
            evicted.map(|e| e.2).unwrap_or(0),
            u8::from(ev.evict_reload_pa_changed),
            ev.pa_p0_first,
            ev.pa_p0_reload,
            ev.reloads,
            ev.kernel_data_ok,
            ev.kernel_data_bad,
            ev.a_pages_done,
            ev.b_pages_done
        );
        let _ = writeln!(
            w,
            "# pager: data a_ok={} a_bytes={} (=a_ok*4096) a_bad={} a_bad_bytes={} b_ok={} b_bytes={} (=b_ok*4096) b_bad={} b_bad_bytes={}",
            ev.a_ok_pages, ev.a_ok_bytes, ev.a_bad_pages, ev.a_bad_bytes,
            ev.b_ok_pages, ev.b_ok_bytes, ev.b_bad_pages, ev.b_bad_bytes
        );
        let _ = writeln!(
            w,
            "# pager: reject client={} page={} status={} name={} reads_before={} reads_after={} delta={} (=reads_after-reads_before) exit={:#x}",
            ev.reject_client,
            ev.reject_page,
            ev.reject_status,
            abi_error_name(ev.reject_status as u16),
            ev.reject_reads_before,
            ev.reject_reads_after,
            ev.reject_reads_after.saturating_sub(ev.reject_reads_before),
            ev.reject_exit
        );
        let _ = writeln!(
            w,
            "# pager: exit pager={} a={} b={} steps={} const:starts=3 exits={} (=(pager==0)+(a==0)+(b==0x8000_0000|status))",
            ev.pager_exit,
            ev.a_exit,
            ev.b_exit,
            ev.steps,
            u8::from(ev.pager_exit == 0)
                + u8::from(ev.a_exit == 0)
                + u8::from(ev.b_exit == 0x8000_0000 | invalid_argument)
        );
        let _ = writeln!(
            w,
            "# pager: neg aux_status={} name={} over_status={} name={} pa_same={} (walk=0x{:x} exp=0x{:x}) magic={:#x} const:neg_over=10 (aux=P_SELF là CNode cap, không phải VSpace ⇒ core trả {} trước object layer)",
            ev.neg_aux,
            abi_error_name(ev.neg_aux as u16),
            ev.neg_over,
            abi_error_name(ev.neg_over as u16),
            u8::from(ev.neg_pa_same),
            ev.neg_walk_pa,
            rig.result_b_pa,
            ev.neg_magic,
            abi_error_name(ev.neg_aux as u16)
        );
    }

    // TAP 30 — fault → pager → nạp trang → client đọc đúng.
    let tap30 = ev.core_fault_seen
        && ev.fault_kind_name == "Vm"
        && ev.reason_name == "NotMapped"
        && ev.access_name == "Read"
        && reason_code == 0
        && access_code == 0
        && ev.served_ok >= 1
        && ev.mapping_bad == 0
        && ev.a_ok_pages >= 1;
    tap(
        fails,
        tap30,
        30,
        "pager fault den server va nap trang",
        "fault chua qua core/pager hoac mapping sai",
    );
    // TAP 31 — hit không đọc backend + cùng PA.
    let tap31 = ev.hit_checked && ev.hit_delta_zero && hits >= 2 && ev.p2_equal;
    tap(
        fails,
        tap31,
        31,
        "pager hit khong doc backend",
        "hit khong do duoc hoac PA khac nhau",
    );
    // TAP 32 — eviction LRU không mất dữ liệu.
    let tap32 = evictions >= 1
        && ev.evict_seen
        && ev.evict_walk_gone
        && ev.evict_reload_pa_changed
        && ev.reloads >= 2
        && ev.a_ok_pages == 5
        && ev.a_bad_pages == 0
        && ev.a_bad_bytes == 0
        && ev.kernel_data_bad == 0;
    tap(
        fails,
        tap32,
        32,
        "pager eviction LRU khong mat du lieu",
        "eviction/reload du lieu sai",
    );
    let tap33 = clients == 2
        && ev.p2_equal
        && shared >= 2
        && entries <= CACHE_CAPACITY as u64
        && unique == entries
        && ev.a_bad_bytes == 0
        && ev.b_bad_bytes == 0;
    tap(
        fails,
        tap33,
        33,
        "pager hai client chia se mot trang",
        "khong chung minh duoc mot ban nhieu mapping",
    );
    let tap34 = ev.rejected == 1
        && ev.reject_page == 99
        && ev.reject_client == 1
        && ev.reject_status == invalid_argument
        && abi_error_name(ev.reject_status as u16) == "InvalidArgument"
        && ev.reject_reads_after == ev.reject_reads_before
        && ev.b_exit == ev.reject_exit
        && ev.b_ok_pages == 4
        && ev.b_bad_bytes == 0;
    tap(
        fails,
        tap34,
        34,
        "pager page ngoai file tra ma",
        "ma loi sai hoac doc backend sai",
    );
    // TAP 35 — ca âm **quyền** (aux không phải VSpace) + **không ghi đè** (VA đã map).
    // Đo thật: core kiểm **loại cap phụ** trước object layer ⇒ aux **CNode cap** trả
    // `WrongObjectType` (2), không phải `InvalidCapability`; giữ nguyên mã đo được, không bịa.
    let tap35 = ev.neg_aux == 2
        && abi_error_name(ev.neg_aux as u16) == "WrongObjectType"
        && ev.neg_over == already_mapped
        && abi_error_name(ev.neg_over as u16) == "AlreadyMapped"
        && ev.neg_pa_same
        && ev.neg_magic == RESULT_MAGIC_B
        && ev.mapping_bad == 0
        && ev.b_ok_pages == 4
        && ev.b_bad_bytes == 0;
    tap(
        fails,
        tap35,
        35,
        "pager ca am quyen va khong ghi de",
        "ca am khong do duoc hoac PTE/du lieu bi ghi de",
    );
    // **KT-122** (review toàn dự án #4): bất biến `reads == misses` (mỗi miss đúng **một** lần đọc
    // trang, D2-106) trước đây chỉ **in** một dòng khi sai — không TAP nào đỏ, nên đột biến
    // `reads += 1` → `+= 2` vẫn cho 35 `ok`/0 `not ok`. Nay thành **TAP 36**.
    //
    // `sectors_per_page` viết bằng **literal 8** (một trang 4 KiB / một sector 512 B) thay vì tính
    // từ `PAGE_SIZE / SECTOR_SIZE` — nếu tính từ hai hằng của chính code thì đó lại là "test dùng
    // chung công thức với code" (bài học KT-107).
    let sectors_per_page = 8u64;
    let pager_invariant = reads == misses && sectors == misses * sectors_per_page;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# pager: invariant reads={} misses={} sectors={} const:sectors_per_page={} ok={}",
            reads,
            misses,
            sectors,
            sectors_per_page,
            u8::from(pager_invariant)
        );
    }
    tap(
        fails,
        pager_invariant,
        36,
        "pager bat bien reads==misses",
        "so lan doc trang khac so lan miss (hoac sectors khac misses*8)",
    );
    // Dọn tài nguyên của pha này (trả Untyped cho pha sau — không đổi ngưỡng).
    pager_loaded.release();
    a_loaded.release();
    b_loaded.release();
}
