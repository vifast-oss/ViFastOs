// SPDX-License-Identifier: MIT OR Apache-2.0
//! Platform tối thiểu cho QEMU `virt` riscv64 ở mốc **K0.5** (KT-07).
//!
//! Phạm vi hẹp có chủ đích — chỉ đủ để chứng minh boot:
//! - entry `_start` (S-mode, OpenSBI nạp trực tiếp tại `0x8020_0000`),
//! - `DebugPutStr(&str)` ra UART 16550 của QEMU virt,
//! - `DebugHalt(code)` qua SBI SRST (OpenSBI loại system-reset).
//!
//! Địa chỉ UART `0x1000_0000` và layout 16550 là **dữ kiện nền tảng QEMU virt**,
//! không phải hằng số thiết kế `[TK]` — thiết kế §5.1 chỉ quy định "OpenSBI nạp trực tiếp".
//! `spec/kernel.toml` (thiết kế §4.22) chưa tồn tại tới KT-04a, nên các tham số này
//! là **tạm thời** và sẽ được chuyển thành dữ liệu sinh từ spec ở K1.
//!
//! Crate này **có `unsafe`** vì MMIO và asm không viết "safe" được; nằm trong danh
//! sách đóng `kernel/vifast-plat-*` của D2-17 (thiết kế §8: unsafe chỉ trong danh sách
//! đóng + `kernel/vifast-plat-*` + `boot/`). **Không** thêm `unsafe` ra ngoài crate này.
#![no_std]
#![allow(unsafe_code)] // crate thuộc danh sách đóng của D2-17 (kernel/vifast-plat-*)

use core::arch::{asm, global_asm};

/// UART0 của QEMU `virt` (16550A).
const UART0_BASE: usize = 0x1000_0000;

// Entry point: đặt con trỏ stack từ linker script rồi nhảy vào `rust_main`
// (do crate ảnh boot cung cấp). Chỉ hart BSP chạy ở K0.5 (`-smp 1`).
global_asm!(
    ".section .text.entry",
    ".globl _start",
    "_start:",
    "la sp, __stack_top",
    "call rust_main",
    "j .",
);

/// In một byte ra UART — ghi thẳng THR; QEMU không cần chờ LSR.
pub fn debug_put_byte(byte: u8) {
    // SAFETY: MMIO của thiết bị 16550 trên QEMU virt; ghi một byte vào THR.
    unsafe {
        core::ptr::write_volatile(UART0_BASE as *mut u8, byte);
    }
}

/// `DebugPutStr` mức platform cho K0.5 (syscall 12 dùng lại hàm này ở K1c).
pub fn debug_put_str(s: &str) {
    for byte in s.bytes() {
        debug_put_byte(byte);
    }
}

/// `DebugHalt` mức platform cho K0.5 (syscall 13). Thoát QEMU qua SBI SRST:
/// `EID=0x53525354`, `FID=0` (system_reset), `a0=0` (shutdown), `a1=0` (no reason).
/// Mã thoát khác 0 chỉ để log; QEMU vẫn kết thúc như một lần tắt máy sạch.
pub fn debug_halt(code: usize) -> ! {
    debug_put_str(if code == 0 {
        "# k0.5 halt\n"
    } else {
        "# k0.5 halt (loi)\n"
    });
    // SAFETY: ecall SBI SRST theo SBI v2.0; OpenSBI đã nạp trước khi vào S-mode.
    unsafe {
        asm!(
            "ecall",
            in("a7") 0x5352_5354usize, // SRST
            in("a6") 0usize,          // FID 0 = system_reset
            in("a0") 0usize,          // reset_type = shutdown
            in("a1") 0usize,          // reset_reason = none
            options(noreturn),
        );
    }
}
