// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ảnh boot **aarch64** đầu tiên của ViFastOs — KT-247 (K4' bước 3, B3.1 / D2-195).
//!
//! # Đường boot
//!
//! ```text
//!   qemu-system-aarch64 -M virt -cpu cortex-a72 -m 128M -nographic -kernel <elf>
//!        │
//!        ├─ QEMU nạp ELF tại 0x4008_0000 (linker.ld), nhảy vào _start ở EL1, x0 = địa chỉ DTB
//!        ├─ _start: sp ← __stack_top; zero .bss; gọi kmain
//!        ├─ kmain: PL011 init (IBRD/FBRD/LCR_H/CR/ICR) → in **một** dòng marker
//!        └─ debug_halt: PSCI SYSTEM_OFF (hvc #0, x0 = 0x8400_0008) ⇒ QEMU thoát mã 0
//! ```
//!
//! # Vì sao `_start` nằm trong `global_asm!` của **ảnh** (không phải crate arch)
//!
//! `_start` là **hợp đồng giữa linker script và ảnh**: nó đọc đúng các symbol mà `linker.ld`
//! của ảnh này cấp (`__stack_top`, `__bss_start`, `__bss_end`). Đặt nó cạnh linker script giữ
//! hai thứ đó **một chỗ**; crate arch chỉ cấp asm nguyên thủy (D2-17). `global_asm!` **không**
//! cần `unsafe` nên ảnh vẫn không có `unsafe` (luật R5).
//!
//! # Phạm vi (D2-195)
//!
//! Bước 3 mở **B3.1–B3.3 + B3.8**. Ảnh này là B3.1: **không** MMU, **không** bảng vector,
//! **không** U-mode. KT-248 thêm bảng vector, KT-249 bật MMU, KT-250 thêm TAP + runner.
//!
//! # Không tuyên bố gì thêm
//!
//! Ảnh in **một** dòng marker rồi tắt máy. "Boot được một dòng" **không** phải "K4' đạt": tiêu
//! chí K4' là "như K3" (TAP đầy đủ smp 1|2|4 + soak 1.000) — bước sau, không phải bước 3.

#![no_std]
#![no_main]

use core::arch::global_asm;
use core::fmt::{self, Write};

use vifast_plat_aarch64::{debug_halt, pl011};

/// Dòng marker **duy nhất** của ảnh B3.1 — runner (`v2/test/boot/run-aarch64.sh`, KT-250) so
/// **nguyên văn** dòng này và đòi nó xuất hiện **đúng một lần**.
pub const BOOT_MARKER: &str = "# viFastOs aarch64: boot ok\n";

// ---------------------------------------------------------------------------------------------
// Entry point — hợp đồng với `linker.ld`
// ---------------------------------------------------------------------------------------------
//
// QEMU `virt` nhảy vào `_start` ở **EL1** (chế độ `EL1h`, `SP_EL1`) với `x0` = địa chỉ DTB.
// Không có firmware nào đặt stack/zero `.bss` hộ, nên `_start` phải tự làm cả hai:
//
//   1. `sp ← __stack_top` (đỉnh `.stack` NOLOAD, 64 KiB) — trước mọi lệnh dùng stack;
//   2. zero `[__bss_start, __bss_end)` theo từng 8 byte — **không** dựa vào "QEMU tình cờ trả 0"
//      (cùng lý do KT-45 P4 bên riscv64: RAM thật không được đảm bảo bằng 0);
//   3. `bl kmain` (x0 vẫn là DTB — `kmain` chưa dùng nhưng giữ đúng hợp đồng gọi).
//
// `adrp`/`add :lo12:` thay `ldr =symbol` để không cần literal pool và không phụ thuộc
// relocation động (ảnh link tĩnh, `-C relocation-model=static`).
global_asm!(
    ".section .text.entry, \"ax\"",
    ".globl _start",
    "_start:",
    // 1. stack
    "adrp x9, __stack_top",
    "add x9, x9, :lo12:__stack_top",
    "mov sp, x9",
    // 2. zero .bss (x9 = con trỏ chạy, x10 = cuối)
    "adrp x9, __bss_start",
    "add x9, x9, :lo12:__bss_start",
    "adrp x10, __bss_end",
    "add x10, x10, :lo12:__bss_end",
    "1:",
    "cmp x9, x10",
    "b.hs 2f",
    "str xzr, [x9], #8",
    "b 1b",
    "2:",
    // 3. vào Rust (x0 = DTB do QEMU truyền)
    "bl kmain",
    // `kmain` không trả về; nếu có thì dừng an toàn thay vì chạy vào rác.
    "3:",
    "wfi",
    "b 3b",
);

// ---------------------------------------------------------------------------------------------
// Console tối giản trên PL011
// ---------------------------------------------------------------------------------------------

/// `core::fmt::Write` trên [`pl011`] — để `panic!`/`write!` in được số mà không cần `alloc`.
struct Console;

impl Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let _ = pl011::write_str(s);
        Ok(())
    }
}

/// `#[panic_handler]` của ảnh: in `PANIC` + thông điệp rồi `debug_halt(1)`.
///
/// Dòng `PANIC` **không bao giờ** là `ok` trong TAP, nên runner coi lần boot đó là đỏ (cùng hợp
/// đồng với `vifast_plat_common::PanicHandler` bên riscv64; ảnh này chưa cần cả `Platform` nên
/// dùng bản tối giản tại chỗ).
#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    let mut console = Console;
    let _ = writeln!(console, "PANIC: {info}");
    debug_halt(1)
}

// ---------------------------------------------------------------------------------------------
// kmain
// ---------------------------------------------------------------------------------------------

/// Điểm vào Rust, được `_start` gọi sau khi đã có stack và `.bss` sạch.
///
/// `dtb` = địa chỉ DTB QEMU truyền trong `x0` (giữ trong chữ ký để KT-250 in magic DTB làm bằng
/// chứng "địa chỉ nền tảng là thật"; B3.1 chưa đọc DTB).
#[no_mangle]
pub extern "C" fn kmain(_dtb: u64) -> ! {
    // Khởi tạo PL011 theo đúng chuỗi DDI 0183 (tắt → chia tần → khung → xoá ngắt → bật). QEMU
    // đã cấu hình sẵn nên bước này không bắt buộc để in, nhưng làm đủ để ảnh không phụ thuộc
    // trạng thái reset của firmware.
    let _steps = pl011::init();
    let _ = pl011::write_str(BOOT_MARKER);
    debug_halt(0)
}
