// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ảnh **test** riscv64 cho K3 (KT-44) — bin riêng, feature `ktest`, nạp qua OpenSBI.
//!
//! D2-08: ảnh test **không** nằm trong ảnh sản phẩm. KT-54 sẽ dựng harness TAP đầy đủ (nhiều
//! test, `expected.txt`, smp 1/2/4); ở KT-44 ảnh này tồn tại để chứng minh tách ảnh + dùng chung
//! `Platform`/`PanicHandler` của `vifast-plat-common`.
//!
//! Dòng `ok 1 - ktest image` là marker mà `v2/test/boot/ktest.sh` đòi — chính là phép đỏ-trước
//! của KT-44 (khi chưa có ảnh test thì `ktest.sh` đỏ).
#![no_std]
#![no_main]

use core::fmt::Write;

use vifast_plat_common::{PanicHandler, Platform, PlatformWriter};
use vifast_plat_riscv64::Riscv64Virt;

static PLATFORM: Riscv64Virt = Riscv64Virt;
static PANIC: PanicHandler<Riscv64Virt> = PanicHandler::new(Riscv64Virt);

#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    PLATFORM.put_str("TAP version 13\n1..1\n");
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = write!(
            w,
            "# ktest image: hart={} tick_hz={} time={}\n",
            PLATFORM.hart_id(),
            PLATFORM.tick_hz(),
            PLATFORM.now()
        );
    }
    PLATFORM.put_str("ok 1 - ktest image\n");
    PLATFORM.put_str("# ktest halt\n");
    PLATFORM.halt(0)
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    PANIC.handle(info)
}
