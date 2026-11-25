// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-70** — nơi giữ trapframe của tác vụ U-mode vừa dừng (tách khỏi `user.rs` để giữ luật
//! cấu trúc R1 ≤ 1500 dòng/tệp). Scheduler đọc bản sao này để chạy tiếp tác vụ
//! (`user::saved_tf`); `on_u_exception` ghi nó trước khi trả `Stop`.

use vifast_plat_riscv64_logic::trap::TrapFrame;

/// Trapframe của tác vụ vừa dừng — scheduler đọc để chạy tiếp.
static mut SAVED_TF: TrapFrame = TrapFrame::zeroed();

/// Ghi trapframe của tác vụ vừa dừng ([`crate::user::run_task`] gọi trước khi trả `Stop`).
/// `#[allow(dead_code)]`: build không bật feature `user` không có call site (cùng một crate).
#[allow(dead_code)]
pub(crate) fn save(tf: &TrapFrame) {
    // SAFETY: một hart, `user_enter` đã trả về S-mode nên không có ai đọc/ghi đồng thời.
    unsafe {
        core::ptr::write(core::ptr::addr_of_mut!(SAVED_TF), *tf);
    }
}

/// Bản sao trapframe của tác vụ vừa dừng.
pub fn saved() -> TrapFrame {
    // SAFETY: như `save` — chỉ đọc sau khi handler đã ghi xong trong cùng luồng.
    unsafe { core::ptr::read(core::ptr::addr_of!(SAVED_TF)) }
}
