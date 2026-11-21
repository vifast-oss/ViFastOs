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
    let Some(tcb) = user::bound_core_tcb() else {
        return;
    };
    if let Some(pc) = take_resume_pc(tcb) {
        tf.sepc = pc;
    }
}

/// Chạy một tác vụ U-mode từ `loaded` với trapframe `tf` (đã có `sepc`/`sp`/regs). Trả về khi
/// tác vụ `Yield`/`Exit` (đa nhiệm) hoặc `DebugHalt`; `tf` **không** bị sửa tại chỗ — bản mới
/// nằm ở [`user::saved_tf`].
///
/// **KT-314**: trước khi `sret`, nạp `sepc` từ `TcbData.entry` nếu `Tcb.Resume` đã chờ TCB này
/// ([`apply_resume_pc`]) — mọi ảnh đi qua đây nên không có đường "resume nửa vời".
pub fn run_task(loaded: &LoadedInit, tf: &mut TrapFrame) -> Stop {
    apply_resume_pc(tf);
    user::enter_loaded(loaded, tf);
    decode(user::stop_kind(), user::stop_code())
}
