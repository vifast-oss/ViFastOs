// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ảnh **sản phẩm** riscv64 cho K3 (KT-44).
//!
//! Không chứa chuỗi/đường dẫn `ktest` (D2-08); ảnh test là bin `vifast-kbin-ktest`
//! (`src/ktest.rs`, feature `ktest`). KT-45..49 dựng trap/MMU/timer/PLIC/SMP trên cùng khung này.
#![no_std]
#![no_main]

use core::fmt::Write;

use vifast_plat_common::{PanicHandler, Platform, PlatformWriter};
use vifast_plat_riscv64::Riscv64Virt;

/// Nền tảng QEMU `virt` — zero-sized, dựng được trong `static`.
static PLATFORM: Riscv64Virt = Riscv64Virt;
/// Một handler panic dùng chung cho cả ảnh sản phẩm và ảnh test (`vifast-plat-common`).
static PANIC: PanicHandler<Riscv64Virt> = PanicHandler::new(Riscv64Virt);

/// Được `_start` (trong `vifast-plat-riscv64`) gọi sau khi đặt `sp`/`tp`.
#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    PLATFORM.put_str("TAP version 13\n1..1\n");
    PLATFORM.put_str("# k3 image=product (kernel thật trên QEMU virt, không phải model)\n");
    // In hart/time để chứng minh `Platform` chạy trên phần cứng ảo; số tick thật là KT-47.
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = write!(
            w,
            "# hart={} tick_hz={} time={}\n",
            PLATFORM.hart_id(),
            PLATFORM.tick_hz(),
            PLATFORM.now()
        );
    }
    PLATFORM.put_str("ok 1 - k3 boot\n");
    PLATFORM.put_str("# k3 halt\n");
    PLATFORM.halt(0)
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    PANIC.handle(info)
}
