// SPDX-License-Identifier: MIT OR Apache-2.0
//! Platform riscv64 cho QEMU `virt` — mốc **K3** (KT-44).
//!
//! Phạm vi vẫn hẹp có chủ đích: entry `_start` (S-mode, OpenSBI `fw_dynamic` nạp trực tiếp tại
//! `0x8020_0000`), MMIO UART 16550 (`debug_put_str`), SBI SRST (`debug_halt`), và **cài đặt**
//! `Platform` của `vifast-plat-common` trên phần cứng ảo: `rdtime` cho `now`, `tp` cho `hart_id`.
//! Trap/timer/PLIC/MMU/SMP **không** thuộc KT-44 — KT-45..49.
//!
//! Địa chỉ UART/PLIC/CLINT là **dữ kiện nền tảng QEMU virt** (D2-72 Q7), không phải hằng số thiết
//! kế `[TK]`; `spec/kernel.toml` **không** mở ở K3 (D2-72 Q1) nên chúng nằm ở đây và ở
//! `v2/docs/k3-riscv64.md`.
//!
//! Crate này **có `unsafe`** vì MMIO và asm không viết "safe" được; nằm trong danh sách đóng
//! `kernel/vifast-plat-*` của D2-17. Mọi phần **logic** dùng chung ở `vifast-plat-common`
//! (`#![forbid(unsafe_code)]`).
#![no_std]
#![allow(unsafe_code)] // crate thuộc danh sách đóng của D2-17 (kernel/vifast-plat-*)

use core::arch::{asm, global_asm};

use vifast_plat_common::Platform;

/// UART0 của QEMU `virt` (16550A) — dữ kiện nền tảng, D2-72 Q7.
pub const UART0_BASE: usize = 0x1000_0000;
/// Dòng IRQ của UART0 trên PLIC của `virt` (D2-72 Q7).
pub const UART0_IRQ: u32 = 10;
/// PLIC cổ điển của `virt` (D2-72 Q7/Q8); `ndev` đọc từ register, không giả định.
pub const PLIC_BASE: usize = 0x0c00_0000;
/// CLINT/ACLINT của `virt` (`aclint=on`; D2-72 Q7).
pub const CLINT_BASE: usize = 0x0200_0000;
/// Trang test của `virt` (`test@100000`; D2-72 Q7).
pub const TEST_PAGE_BASE: usize = 0x0010_0000;
/// Timebase QEMU `virt`: 10 MHz (DTB `timebase-frequency`; D2-72 Q6).
pub const TIME_HZ: u64 = 10_000_000;

// Entry point: OpenSBI nhảy vào S-mode với `a0 = hartid`, `a1 = địa chỉ DTB` (SBI spec).
// Giữ hartid ở `tp` để `Riscv64Virt::hart_id` đọc lại mà không cần `static mut`; đặt `sp` từ
// linker script (`.stack` NOLOAD) rồi vào `rust_main` do crate ảnh cung cấp.
global_asm!(
    ".section .text.entry",
    ".globl _start",
    "_start:",
    "mv tp, a0", // hartid (a0) — OpenSBI truyền; `a1` là DTB (KT-45 trở đi mới dùng)
    "la sp, __stack_top",
    "call rust_main",
    "j .",
);

/// Nền tảng QEMU `virt` riscv64 — zero-sized, dựng được trong `static`/`const`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Riscv64Virt;

impl Platform for Riscv64Virt {
    fn put_str(&self, s: &str) {
        debug_put_str(s);
    }

    fn halt(&self, code: usize) -> ! {
        debug_halt(code)
    }

    fn now(&self) -> u64 {
        read_time()
    }

    fn tick_hz(&self) -> u64 {
        TIME_HZ
    }

    fn hart_id(&self) -> usize {
        read_hart_id()
    }
}

/// `rdtime` (Zicntr): S-mode đọc được vì OpenSBI đặt `mcounteren.TM`; tick 10 MHz.
/// Đây là **stub** của K3 — timer thật (`set_timer`, tick 100 Hz) là KT-47 (D2-72 Q6).
fn read_time() -> u64 {
    let ticks: u64;
    // SAFETY: `rdtime` chỉ đọc CSR `time`, không tác dụng phụ, không chạm bộ nhớ.
    unsafe { asm!("rdtime {}", out(reg) ticks, options(nomem, nostack)) };
    ticks
}

/// Hart id đọc từ `tp` — `_start` đã chép `a0` (hartid OpenSBI truyền) vào đó.
fn read_hart_id() -> usize {
    let id: usize;
    // SAFETY: `tp` do chính `_start` đặt từ `a0`; kernel `no_std` chưa dùng TLS/`tp`.
    unsafe { asm!("mv {}, tp", out(reg) id, options(nomem, nostack, preserves_flags)) };
    id
}

/// In một byte ra UART — ghi thẳng THR; QEMU không cần chờ LSR.
pub fn debug_put_byte(byte: u8) {
    // SAFETY: MMIO của thiết bị 16550 trên QEMU virt; ghi một byte vào THR.
    unsafe {
        core::ptr::write_volatile(UART0_BASE as *mut u8, byte);
    }
}

/// `DebugPutStr` mức platform (syscall 12 dùng lại hàm này ở K3 trở đi).
pub fn debug_put_str(s: &str) {
    for byte in s.bytes() {
        debug_put_byte(byte);
    }
}

/// `DebugHalt` mức platform (syscall 13). Thoát QEMU qua SBI SRST:
/// `EID=0x53525354`, `FID=0` (system_reset), `a0=0` (shutdown), `a1=0` (no reason).
///
/// **Không** in marker: mỗi ảnh tự in dòng `# k3 halt` / `# ktest halt` của mình trước khi gọi,
/// để TAP sạch và runner phân biệt được ảnh nào (D2-08).
pub fn debug_halt(_code: usize) -> ! {
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
