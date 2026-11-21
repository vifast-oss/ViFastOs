// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-74** — `Stop` + `run_task`: vòng chạy một tác vụ U-mode (KT-65), tách khỏi
//! [`crate::user`] để giữ luật cấu trúc **R1** (≤ 1.500 dòng/tệp).
//!
//! **KT-280** — thêm [`apply_ipc_return`]: nửa thứ hai của hợp đồng **resume** sau một syscall
//! chặn (`Call`/`Recv`), cùng lý do R1 (tệp này giữ phần "chạy/resume tác vụ" của `user`).
//!
//! Ngữ nghĩa **không** đổi: `crate::user::enter_loaded` vẫn là nơi hạ `sret` và ghi loại/mã dừng
//! vào static của `user`; tệp này chỉ **dịch** chúng thành [`Stop`] cho scheduler của ảnh test.
//! `user` tái xuất [`Stop`]/[`run_task`] nên mọi ảnh test giữ nguyên đường dẫn cũ.

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use vifast_core::{ObjId, TcbState};
use vifast_plat_common::trap::TrapFrame;

use crate::init_pages::LoadedInit;
use crate::user;

/// **KT-280** — nạp `a0..a6` của trapframe từ **vùng nhận IPC** của TCB `tcb` (message vừa được
/// core giao) — nửa thứ hai của hợp đồng resume sau một syscall **chặn**.
///
/// # Vì sao cần
///
/// `Call`/`Recv` chặn trong core (`BlockedOnReply`/`BlockedOnRecv`) rồi được đánh thức khi message
/// tới: lúc đó `dispatch` đã trả `Switched`, nên **không** có `SyscallReturn` nào được ghi vào
/// trapframe. Khi scheduler resume tác vụ, kết quả syscall phải lấy từ **cùng** nguồn mà
/// `vifast_core::syscall::received` dùng cho đường không chặn: `Machine::tcb_ipc_area` (D2-83 —
/// core sở hữu message, arch chỉ đọc).
///
/// Bố cục trả về đúng `spec/abi.toml`: `a0 = error` (0 = OK), `a1 = value` (số MR), `a2 =
/// MessageInfo` đã đóng gói, `a3..a6 = MR0..MR3`. Trả `false` ⇔ TCB chưa nhận message nào (không
/// ghi gì vào trapframe).
pub fn apply_ipc_return(tf: &mut TrapFrame, tcb: ObjId) -> bool {
    let Some(area) = crate::core_bridge::with_machine(|m| m.tcb_ipc_area(tcb).ok()) else {
        return false;
    };
    let Some(info) = area.info else {
        return false;
    };
    tf.set_arg(0, 0);
    tf.set_arg(1, u64::from(info.length()));
    tf.set_arg(2, info.pack());
    for index in 0..4 {
        tf.set_arg(3 + index, area.mr(index).unwrap_or(0));
    }
    // **KT-282** — MR4..MR63 của message vừa nhận phải ghi vào **IPC buffer** của tác vụ (VA đã
    // bind bằng `Tcb.Configure` tham số `ipc_buffer?`). Ở đây `satp` = **root kernel** (hàm này
    // chạy sau `enter_loaded`), nên **không** ghi trực tiếp được: đặt cờ để `enter_loaded` ghi
    // ngay sau khi chuyển sang root user (một lần, đúng lúc). Message ≤ 4 MR ⇒ không chạm gì.
    crate::ipc_buffer::request_write(tcb);
    true
}

/// Vì sao một lượt [`run_task`] trả về S-mode.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stop {
    /// Tác vụ gọi `Exit` (syscall 14). `code` = mã thoát.
    Exit { code: u64 },
    /// Tác vụ gọi `Yield` (syscall 8) — điểm nhường CPU của scheduler.
    Yield,
    /// Tác vụ chặn trong core (không dùng ở KT-65; giữ để không panic).
    Blocked(TcbState),
    /// Tác vụ gọi `DebugHalt` (13).
    Halt { code: u64 },
    /// Tác vụ U-mode **fault VM** trong pha pager (KT-70): trapframe giữ `sepc` để **chạy lại**
    /// instruction sau khi pager map trang; chi tiết qua `crate::user::pager_fault`.
    Fault,
}

/// Tên trạng thái TCB ổn định cho log/TAP (không in `{:?}` có số hart).
///
/// **KT-314** — chuyển từ `user.rs` sang đây để `user.rs` giữ trần R1 1.500 dòng (cùng lý do tệp
/// này được tách khỏi `user` ở KT-74/KT-280); hành vi **không** đổi.
pub(crate) fn tcb_state_name(state: TcbState) -> &'static str {
    match state {
        TcbState::Inactive => "Inactive",
        TcbState::Ready => "Ready",
        TcbState::Running { .. } => "Running",
        TcbState::BlockedOnSend { .. } => "BlockedOnSend",
        TcbState::BlockedOnRecv { .. } => "BlockedOnRecv",
        TcbState::BlockedOnReply { .. } => "BlockedOnReply",
        TcbState::BlockedOnNotification { .. } => "BlockedOnNotification",
        TcbState::BlockedOnAddress { .. } => "BlockedOnAddress",
        TcbState::Sleeping { .. } => "Sleeping",
        TcbState::Stopping => "Stopping",
        TcbState::Dead => "Dead",
    }
}

/// Dịch `(loại, mã)` do `on_u_exception` ghi thành [`Stop`] — cùng bảng số như trước KT-74.
fn decode(kind: u32, code: u64) -> Stop {
    match kind {
        1 => Stop::Exit { code },
        2 => Stop::Yield,
        3 => Stop::Blocked(TcbState::Inactive),
        4 => Stop::Halt { code },
        5 => Stop::Fault,
        _ => Stop::Blocked(TcbState::Inactive),
    }
}

// ---------------------------------------------------------------------------------------------
// KT-314 — `Tcb.Resume`: core **giao** `TcbData.entry`, arch **ghi `sepc`** (D2-83)
// ---------------------------------------------------------------------------------------------

/// TCB đang chờ nạp `sepc` — `u32::MAX` = không có (cùng khuôn `user::CORE_TCB`).
static RESUME_TCB: AtomicU32 = AtomicU32::new(u32::MAX);
/// Điểm vào (`TcbData.entry`) mà `Tcb.Resume` đã giao cho arch.
static RESUME_PC: AtomicU64 = AtomicU64::new(0);

/// **KT-314** — `Tcb.Resume` vừa chuyển `tcb` sang `Running`: giữ `entry` để lần vào U-mode **kế
/// tiếp** của **đúng** TCB đó nạp vào `sepc`.
///
/// Vì sao phải chờ: `Resume` là invocation của **tác vụ giám sát** (caller), không phải của tác
/// vụ đích — `sepc` của đích chỉ ghi được lúc scheduler `sret` vào nó. Core **không** ghi CSR
/// (D2-83) nên nó gọi hook `SyscallEnv::tcb_resume`, hook này chỉ **giữ** giá trị; [`run_task`]
/// tiêu thụ ở lượt chạy kế tiếp (một lần — `take` xoá).
pub(crate) fn arm_resume_pc(tcb: ObjId, entry: u64) {
    RESUME_PC.store(entry, Ordering::Release);
    RESUME_TCB.store(tcb, Ordering::Release);
}

/// Lấy + **xoá** điểm vào đã chờ cho `tcb` (`None` = không có, hoặc đang chờ TCB khác).
fn take_resume_pc(tcb: ObjId) -> Option<u64> {
    if RESUME_TCB.load(Ordering::Acquire) != tcb {
        return None;
    }
    RESUME_TCB.store(u32::MAX, Ordering::Release);
    Some(RESUME_PC.swap(0, Ordering::AcqRel))
}

/// **KT-314** — nạp `sepc` từ `TcbData.entry` nếu `Tcb.Resume` đã chờ đúng TCB đang chạy.
///
/// Đây là chỗ **duy nhất** biến `TcbData.entry` thành `sepc` — không có đường thứ hai (rig
/// **không** đặt `tf.sepc` cho tác vụ được resume: nó để `0`, nên nếu hook không nạp thì tác vụ
/// fault ở `0` và TAP đỏ).
fn apply_resume_pc(tf: &mut TrapFrame) {
    let Some(tcb) = user::core_tcb() else {
        return;
    };
    if let Some(pc) = take_resume_pc(tcb) {
        tf.sepc = pc;
    }
}

// ---------------------------------------------------------------------------------------------
// KT-320 — `Tcb.Configure` `vspace?`: core **giao** danh tính VSpace, arch **chọn root `satp`**
// ---------------------------------------------------------------------------------------------

/// TCB đang chờ áp `VSpace` — `u32::MAX` = không có (cùng khuôn `RESUME_TCB`).
static VSPACE_TCB: AtomicU32 = AtomicU32::new(u32::MAX);
/// `ObjId` (core) của `VSpace` mà `Tcb.Configure` đã giao cho arch.
static VSPACE_OBJ: AtomicU32 = AtomicU32::new(0);
/// `(obj, root_pa, asid, match)` của lần **tiêu thụ** gần nhất — bằng chứng cho TAP/rig.
static VSPACE_APPLIED: AtomicU32 = AtomicU32::new(0);
static VSPACE_ROOT: AtomicU64 = AtomicU64::new(0);
static VSPACE_ASID: AtomicU32 = AtomicU32::new(0);
static VSPACE_MATCH: AtomicU32 = AtomicU32::new(0);

/// **KT-320** — `Tcb.Configure` vừa ghi `TcbData.vspace` cho `tcb`: giữ **danh tính** object để
/// lần vào U-mode **kế tiếp** của **đúng** TCB đó chọn root `satp` từ nó.
///
/// Vì sao phải chờ: `Configure` là invocation của **tác vụ giám sát** (caller), không phải của
/// tác vụ đích — `satp` của đích chỉ ghi được lúc scheduler `sret` vào nó. Core **không** ghi CSR
/// (D2-83) nên nó gọi hook `SyscallEnv::tcb_vspace`, hook này chỉ **giữ** giá trị; [`run_task`]
/// tiêu thụ ở lượt chạy kế tiếp (một lần — `take` xoá).
pub(crate) fn arm_vspace(tcb: ObjId, vspace: ObjId) {
    VSPACE_OBJ.store(vspace, Ordering::Release);
    VSPACE_TCB.store(tcb, Ordering::Release);
}

/// Lấy + **xoá** `VSpace` đã chờ cho `tcb` (`None` = không có, hoặc đang chờ TCB khác).
fn take_vspace(tcb: ObjId) -> Option<ObjId> {
    if VSPACE_TCB.load(Ordering::Acquire) != tcb {
        return None;
    }
    VSPACE_TCB.store(u32::MAX, Ordering::Release);
    let obj = VSPACE_OBJ.swap(0, Ordering::AcqRel);
    if obj == 0 {
        None
    } else {
        Some(obj)
    }
}

/// **KT-320** — `(obj, root_pa, asid, match)` của lần **tiêu thụ** `vspace?` gần nhất; `None` =
/// chưa có lần nào. `match` = root của VSpace đã cấu hình **khớp** root của ảnh đã nạp.
pub fn vspace_applied() -> Option<(ObjId, u64, u16, bool)> {
    let obj = VSPACE_APPLIED.load(Ordering::Acquire);
    if obj == 0 {
        return None;
    }
    Some((
        obj,
        VSPACE_ROOT.load(Ordering::Acquire),
        VSPACE_ASID.load(Ordering::Acquire) as u16,
        VSPACE_MATCH.load(Ordering::Acquire) != 0,
    ))
}

/// **KT-320** — giải `ObjId` (core) của `VSpace` thành `(root_pa, asid)` của object layer.
///
/// D2-83: core **không** sở hữu PTE/PA — danh tính object đi qua `Machine::foreign_key` (khoá
/// đóng gói `CapId`+`object_id` của object layer, cùng khuôn `Frame.Map`), rồi object layer trả
/// root PA/ASID. `None` = object đã chết/không phải `VSpace` ⇒ fail-closed.
fn resolve_vspace(obj: ObjId) -> Option<(u64, u16)> {
    let key = crate::core_bridge::with_machine(|m| m.foreign_key(obj).ok())?;
    let (cap, _) = crate::mem_bridge::decode_key(key);
    let m = crate::obj::machine();
    let root = m.vspace_root_pa(cap).ok()?;
    let asid = m.vspace_asid(cap).ok()?;
    Some((root, asid))
}

/// **KT-320** — chọn root `satp` cho lượt vào U-mode **kế tiếp** từ `TcbData.vspace` (đã giao
/// qua hook `SyscallEnv::tcb_vspace`).
///
/// Trả `Some((root_pa, asid))` ⇔ TCB đang chạy có `vspace?` đã cấu hình **và** root của nó khớp
/// root của ảnh đã nạp (`loaded.root`) — khi đó `satp` lấy **từ VSpace của TCB**, không phải từ
/// `loaded`. Lệch root ⇒ `None` + cờ `match=0`: **từ chối** vào U-mode (fail-closed) vì ảnh
/// chưa được map vào VSpace khác (`Frame.Map` vào VSpace mới **ngoài phạm vi** KT-320) — chạy
/// tiếp sẽ fault ngay ở byte đầu tiên.
fn apply_vspace(loaded: &LoadedInit) -> VspaceChoice {
    let Some(tcb) = user::core_tcb() else {
        return VspaceChoice::None;
    };
    let Some(obj) = take_vspace(tcb) else {
        return VspaceChoice::None;
    };
    let Some((root, asid)) = resolve_vspace(obj) else {
        // Danh tính object không giải được (đã chết/không phải VSpace) ⇒ fail-closed.
        VSPACE_APPLIED.store(obj, Ordering::Release);
        VSPACE_ROOT.store(0, Ordering::Release);
        VSPACE_ASID.store(0, Ordering::Release);
        VSPACE_MATCH.store(0, Ordering::Release);
        return VspaceChoice::Refuse;
    };
    let khop = root == loaded.root;
    VSPACE_APPLIED.store(obj, Ordering::Release);
    VSPACE_ROOT.store(root, Ordering::Release);
    VSPACE_ASID.store(u32::from(asid), Ordering::Release);
    VSPACE_MATCH.store(u32::from(khop), Ordering::Release);
    if khop {
        VspaceChoice::Use(root, asid)
    } else {
        VspaceChoice::Refuse
    }
}

/// **KT-320** — kết quả [`apply_vspace`]: không có `vspace?` / dùng root của VSpace đã cấu hình /
/// **từ chối** vào U-mode (root lệch ảnh đã nạp hoặc danh tính không giải được).
enum VspaceChoice {
    None,
    Use(u64, u16),
    Refuse,
}

/// Chạy một tác vụ U-mode từ `loaded` với trapframe `tf` (đã có `sepc`/`sp`/regs). Trả về khi
/// tác vụ `Yield`/`Exit` (đa nhiệm) hoặc `DebugHalt`; `tf` **không** bị sửa tại chỗ — bản mới
/// nằm ở [`user::saved_tf`].
///
/// **KT-314**: trước khi `sret`, nạp `sepc` từ `TcbData.entry` nếu `Tcb.Resume` đã chờ TCB này
/// ([`apply_resume_pc`]) — mọi ảnh đi qua đây nên không có đường "resume nửa vời".
///
/// **KT-320**: nếu TCB này có `vspace?` đã cấu hình ([`apply_vspace`]), root `satp` lấy **từ
/// VSpace của TCB**; root lệch root ảnh đã nạp ⇒ **không** vào U-mode (fail-closed, trả
/// `Stop::Blocked` — cùng mã dừng của nhánh `Switched`).
pub fn run_task(loaded: &LoadedInit, tf: &mut TrapFrame) -> Stop {
    apply_resume_pc(tf);
    let root = match apply_vspace(loaded) {
        VspaceChoice::None => None,
        VspaceChoice::Use(root, asid) => Some((root, asid)),
        // `vspace?` đã cấu hình nhưng root lệch ảnh đã nạp (hoặc danh tính không giải được) ⇒
        // **không** vào U-mode: chạy tiếp sẽ fault ở byte đầu tiên vì ảnh chưa được map vào
        // VSpace đó (`Frame.Map` vào VSpace mới ngoài phạm vi KT-320).
        VspaceChoice::Refuse => return Stop::Blocked(TcbState::Inactive),
    };
    user::enter_loaded_with_root(loaded, tf, root);
    decode(user::stop_kind(), user::stop_code())
}
