// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-74** — TLS của U-mode (`Tcb.SetTls`) và trapframe **mới** cho `exec`.
//!
//! Tách khỏi [`crate::user`] để giữ luật cấu trúc **R1** (≤ 1.500 dòng/tệp). Không đổi ngữ nghĩa:
//! `crate::user` **tái xuất** đúng các tên dưới đây nên ảnh test giữ nguyên đường dẫn cũ.
//!
//! Mọi `unsafe` ở tệp này thuộc danh sách đóng `kernel/vifast-plat-*` (D2-17): đọc `USER_KERNEL_CTX`
//! (asm của `user_enter`) và một lệnh `mv tp` **không đặc quyền**; không CSR, không truy cập vật lý.
//! Không có `mv ?, tp` trong **chương trình U-mode** — bằng chứng `tp` đọc từ trapframe (D2-101).

use core::arch::asm;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use vifast_plat_riscv64_logic::trap::{
    TrapFrame, REG_SP, REG_TP, SSTATUS_SIE, SSTATUS_SPIE, SSTATUS_SPP,
};

// Ngữ cảnh kernel do asm `user_enter` (trong `crate::user`) ghi; tệp này chỉ **đọc** phần tử `tp`
// (offset 24 = phần tử thứ ba) để khôi phục hart id trong handler U-mode.
extern "C" {
    static USER_KERNEL_CTX: [u64; 16];
}

use crate::init_pages::LoadedInit;
use crate::user::{core_tcb, read_sstatus, SSTATUS_SUM, USER_STACK_TOP};

// ---------------------------------------------------------------------------------------------
// **KT-74** — `x4`/`tp` của U-mode **quan sát tại trap** (bằng chứng phần cứng cho `Tcb.SetTls`).
// ---------------------------------------------------------------------------------------------

/// `tp` (`x4`) **của U-mode** tại lần `ecall` gần nhất — trap entry lưu đủ 32 GPR vào trapframe
/// nên đây là giá trị **thật của thanh ghi user**, kernel không tự khai. Không cần asm trong
/// chương trình U-mode (giữ luật 0 `unsafe`, D2-93/D2-101).
static TLS_OBSERVED: AtomicU64 = AtomicU64::new(0);
/// Số lần đã quan sát `tp` (chứng minh giá trị không phải "đọc một lần rồi thôi").
static TLS_TRAPS: AtomicU32 = AtomicU32::new(0);

/// `tp` của U-mode tại trap gần nhất (KT-74).
pub fn tls_observed() -> u64 {
    TLS_OBSERVED.load(Ordering::Acquire)
}

/// Số trap U-mode đã ghi `tp` (KT-74).
pub fn tls_trap_count() -> u32 {
    TLS_TRAPS.load(Ordering::Acquire)
}

/// Khôi phục `tp` **của kernel** (hart id) ngay khi trap từ U-mode.
///
/// **KT-74 — vì sao cần:** kernel dùng `tp` làm **hart id** (`read_hart_id` = `mv ?, tp`,
/// `_start` nạp từ `a0`) và **không** dùng `tp` làm TLS. Nhưng `sret` xuống U-mode nạp **toàn bộ**
/// GPR từ trapframe (kể cả `x4`), nên trong lúc U-mode chạy, `tp` là **TLS của tiến trình**;
/// trap entry chỉ đổi `sp`/`sscratch`, **không** đụng `tp`. Trước KT-74 điều này vô hại vì
/// `tp` của user luôn là 0 và handler U-mode **không** gọi `hart_id()`; từ KT-74 `Tcb.SetTls`
/// đặt `tp` khác 0 ⇒ mọi `hart_id()` trong handler sẽ trả rác (đo được: `hart=7` với
/// `tls=0x7431_0001`, `min(MAX_HARTS)` cắt về 7 — `dispatch_on_private_stack` panic
/// “syscall stack chua cap”).
///
/// Vì vậy: đầu handler U-mode, nạp lại `tp` từ `USER_KERNEL_CTX[3]` (do `user_enter` lưu); đường
/// `sret` về U-mode tự khôi phục `x4` từ frame (`ld x4, …` của `trap_entry`) nên **không** rò
/// hart id sang user. Sau `Exit`, `user_exit_resume` cũng nạp lại `tp` từ ctx.
pub(crate) fn restore_kernel_tp() {
    // SAFETY: `USER_KERNEL_CTX` do asm `user_enter` ghi **trước** mọi trap U-mode (chỉ vào đây
    // khi đã ở U-mode), phần tử 3 = `tp` kernel; `mv tp` chỉ đổi GPR, không phải lệnh đặc quyền.
    unsafe {
        let tp = USER_KERNEL_CTX[3];
        asm!("mv tp, {}", in(reg) tp, options(nomem, nostack, preserves_flags));
    }
}

/// **KT-74** — trapframe **mới** cho ảnh sau `exec` (dùng với [`run_task`]).
///
/// Reset **đúng** những gì `exec` phải reset: `sepc = e_entry` của ảnh mới, `sp` = đỉnh stack
/// user, **mọi** thanh ghi `x0..x31` = 0 (kể cả `a0` — chương trình mới **không** được thấy
/// tham số của ảnh cũ), `tp` = TLS **hiện tại của TCB** (exec đã đặt 0 ⇒ `0`), `sstatus` U-mode
/// như [`enter`]. `loaded` là ảnh **mới** — không dùng trapframe của ảnh cũ.
pub fn exec_frame(loaded: &LoadedInit) -> TrapFrame {
    let mut tf = TrapFrame::zeroed();
    tf.x[REG_SP] = USER_STACK_TOP;
    tf.sepc = loaded.entry;
    let current = read_sstatus();
    tf.sstatus = current & !(SSTATUS_SPP | SSTATUS_SPIE | SSTATUS_SIE | SSTATUS_SUM);
    apply_tls(&mut tf);
    tf
}

/// **KT-74** — nạp con trỏ TLS của TCB vào `x4`/`tp` của trapframe U-mode.
///
/// Nguồn **duy nhất** là core (`Machine::tcb_tls`, cùng nguồn với invocation `Tcb.SetTls`) — arch
/// **không** giữ bản sao thứ hai, nên `SetTls` rồi resume là **một** đường. Ảnh không gắn TCB core
/// (`core_tcb() == None`) ⇒ `tp = 0` như trước KT-74.
///
/// RISC-V: `tp` = `x4` là thanh ghi thread-pointer do ABI quy ước; kernel **không** đọc vùng TLS
/// (chỉ chuyển giá trị), nên U-mode tự quản lý nội dung.
pub fn apply_tls(tf: &mut TrapFrame) {
    let tls = core_tcb()
        .and_then(|tcb| crate::core_bridge::with_machine(|m| m.tcb_tls(tcb).ok()))
        .unwrap_or(0);
    tf.x[REG_TP] = tls;
}

/// Ghi `tp` của U-mode **tại trap** (evidence `# tls:`): trap entry đã lưu đủ 32 GPR vào trapframe
/// nên `tf.x[REG_TP]` **là** giá trị thanh ghi user, không phải lời khai của kernel.
pub fn record_tls_trap(tf: &TrapFrame) {
    TLS_OBSERVED.store(tf.x[REG_TP], Ordering::Release);
    let _ = TLS_TRAPS.fetch_add(1, Ordering::Release);
}
