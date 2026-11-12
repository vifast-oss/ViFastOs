// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ảnh boot riscv64 tối thiểu cho **K0.5** (KT-07).
//!
//! Toàn bộ code trong crate này là **safe**: entry, MMIO UART và SBI nằm ở
//! `vifast-plat-riscv64` (crate thuộc danh sách đóng `kernel/vifast-plat-*` của D2-17).
//!
//! Mục tiêu K0.5: boot qua OpenSBI, in TAP 1 test, halt sạch để QEMU tự thoát.
//! Ảnh này **không** chứa test kernel nào được nhúng — D2-08 ("test kernel không nằm
//! trong boot sản phẩm") sẽ được áp khi có ảnh sản phẩm tách khỏi ảnh `ktest` ở K3;
//! ở K0.5 chỉ có đúng một ảnh kiểm boot, ghi rõ mốc tách ở K3.
#![no_std]
#![no_main]

use vifast_plat_riscv64::{debug_halt, debug_put_str};

/// Được `_start` (trong `vifast-plat-riscv64`) gọi sau khi đặt stack.
#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    debug_put_str("TAP version 13\n1..1\nok 1 - k0.5 boot\n");
    debug_halt(0)
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    debug_put_str("PANIC\n");
    debug_halt(1)
}
