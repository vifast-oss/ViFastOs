// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ảnh **sản phẩm** riscv64 cho K3 (KT-44, KT-45).
//!
//! Không chứa chuỗi/đường dẫn `ktest` (D2-08); ảnh test là bin `vifast-kbin-ktest`
//! (`src/ktest.rs`, feature `ktest`). KT-45 cài `stvec` **direct mode** ngay đầu boot và in lại
//! `stvec` để chứng minh vector thật trên phần cứng; trap thật được kiểm trong ảnh test.
#![no_std]
#![no_main]

use core::fmt::Write;

use vifast_plat_common::{PanicHandler, Platform, PlatformWriter};
use vifast_plat_riscv64::{read_trap_vector, Riscv64Virt};

/// Nền tảng QEMU `virt` — zero-sized, dựng được trong `static`.
static PLATFORM: Riscv64Virt = Riscv64Virt;
/// Một handler panic dùng chung cho cả ảnh sản phẩm và ảnh test (`vifast-plat-common`).
static PANIC: PanicHandler<Riscv64Virt> = PanicHandler::new(Riscv64Virt);

/// Được `_start` (trong `vifast-plat-riscv64`) gọi sau khi đặt `sp`/`tp`/`sscratch`.
#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    PLATFORM.put_str("TAP version 13\n1..1\n");
    PLATFORM.put_str("# k3 image=product (kernel thật trên QEMU virt, không phải model)\n");
    // KT-45: cài vector trap direct-mode trước mọi việc khác; in lại stvec để runner thấy giá trị
    // thật (không phải hằng số viết tay trong tài liệu — QD §9 Q12).
    PLATFORM.set_trap_vector(vifast_plat_riscv64::trap_vector());
    let stvec = read_trap_vector();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# hart={} tick_hz={} time={} stvec=0x{:x} direct={}",
            PLATFORM.hart_id(),
            PLATFORM.tick_hz(),
            PLATFORM.now(),
            stvec,
            stvec & 0b11 == 0
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
