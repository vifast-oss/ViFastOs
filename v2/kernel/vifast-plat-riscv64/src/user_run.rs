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

/// Chạy một tác vụ U-mode từ `loaded` với trapframe `tf` (đã có `sepc`/`sp`/regs). Trả về khi
/// tác vụ `Yield`/`Exit` (đa nhiệm) hoặc `DebugHalt`; `tf` **không** bị sửa tại chỗ — bản mới
/// nằm ở [`user::saved_tf`].
pub fn run_task(loaded: &LoadedInit, tf: &mut TrapFrame) -> Stop {
    user::enter_loaded(loaded, tf);
    decode(user::stop_kind(), user::stop_code())
}
