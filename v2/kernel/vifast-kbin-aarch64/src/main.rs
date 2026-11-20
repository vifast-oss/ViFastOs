// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ảnh boot **aarch64** của ViFastOs — KT-247 (B3.1) + KT-248 (B3.2).
//!
//! # Đường boot
//!
//! ```text
//!   qemu-system-aarch64 -M virt -cpu cortex-a72 -m 128M -nographic -kernel <elf>
//!        │
//!        ├─ QEMU nạp ELF tại 0x4008_0000 (linker.ld), nhảy vào _start ở EL1, x0 = địa chỉ DTB
//!        ├─ _start: sp ← __stack_top; zero .bss; gọi kmain
//!        ├─ kmain: PL011 init → marker → cài VBAR_EL1 → probe `brk #0` → in bằng chứng → halt
//!        └─ debug_halt: PSCI SYSTEM_OFF (hvc #0, x0 = 0x8400_0008) ⇒ QEMU thoát mã 0
//! ```
//!
//! # KT-248 — bảng vector + `VBAR_EL1` + handler đồng bộ
//!
//! - **Bảng 16 ô × `0x80`**, cả bảng căn `0x800`, đặt trong `.text.vectors` (`"ax"`) — **không**
//!   `.bss` (`.bss` là NOLOAD ⇒ bảng không có trong ảnh đã nạp, `VBAR_EL1` trỏ vào rác).
//! - Mỗi ô là một stub 2 instruction: `mov x9, #<ô>` rồi `b vec_common`. Offset của từng ô lấy
//!   từ **hằng của crate arch** (`VECTOR_STRIDE`) qua `const` operand của `global_asm!` — không
//!   có bản thứ hai của con số `0x80` trong asm để lệch với `vifast_plat_aarch64::vector`.
//! - `vec_common` lưu `x0..x18, x29, x30` (176 B), đọc `ESR_EL1`/`FAR_EL1`/`ELR_EL1`/`SPSR_EL1`
//!   rồi gọi [`vec_handler`]. Handler trả `1` ⇒ khôi phục thanh ghi + `eret`; trả `0` ⇒ dừng.
//! - **`brk #0` cố ý**: handler nhận `EC = 0x3C`, ghi lại `ELR_EL1` (địa chỉ instruction `BRK`),
//!   cộng 4 rồi resume ⇒ `kmain` in bằng chứng `ec=0x3c … match=1` và đi tiếp.
//!
//! # Phạm vi (D2-195)
//!
//! Bước 3 mở **B3.1–B3.3 + B3.8**. Ảnh này có B3.1 + B3.2; **chưa** MMU (KT-249), **chưa** TAP
//! (KT-250). **Không** tuyên bố K4' đạt.

#![no_std]
#![no_main]

use core::arch::global_asm;
use core::fmt::{self, Write};
use core::sync::atomic::{AtomicU64, Ordering};

use vifast_plat_aarch64::sysreg::{Esr, ExceptionClass};
use vifast_plat_aarch64::vector::{self, VectorKind};
use vifast_plat_aarch64::{asm, debug_halt, pl011};

/// Dòng marker **duy nhất** của ảnh — runner (`v2/test/boot/run-aarch64.sh`, KT-250) so **nguyên
/// văn** dòng này và đòi nó xuất hiện **đúng một lần**.
pub const BOOT_MARKER: &str = "# viFastOs aarch64: boot ok\n";

// ---------------------------------------------------------------------------------------------
// Entry point — hợp đồng với `linker.ld`
// ---------------------------------------------------------------------------------------------
//
// QEMU `virt` nhảy vào `_start` ở **EL1** (chế độ `EL1h`, `SP_EL1`) với `x0` = địa chỉ DTB.
// Không có firmware nào đặt stack/zero `.bss` hộ, nên `_start` phải tự làm cả hai:
//
//   1. `sp ← __stack_top` (đỉnh `.stack` NOLOAD, 64 KiB) — trước mọi lệnh dùng stack;
//   2. zero `[__bss_start, __bss_end)` theo từng 8 byte — **không** dựa vào "QEMU tình cờ trả 0";
//   3. `bl kmain` (x0 vẫn là DTB — `kmain` chưa dùng nhưng giữ đúng hợp đồng gọi).
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
// KT-248 — bảng vector 16 ô × 0x80, căn 0x800, trong `.text.vectors` (KHÔNG `.bss`)
// ---------------------------------------------------------------------------------------------
//
// `.org` đặt từng stub ở **offset tuyệt đối** trong section; section được linker đặt tại
// `__vector_base` (căn `0x800` bằng `ALIGN(0x800)` trong `linker.ld` + `.p2align 11` ở đây), nên
// offset trong section **chính là** offset từ `VBAR_EL1`. `{stride}` là hằng của crate arch
// (`VECTOR_STRIDE`) ⇒ asm và `vifast_plat_aarch64::vector` không thể lệch nhau.
//
// Khung 176 B (16-aligned) giữ `x0..x18, x29, x30`; `x9` mang chỉ số ô do stub đặt trước khi
// nhảy vào `vec_common`.
global_asm!(
    // Tên input section **không** khớp `*(.text .text.*)` của linker script — nếu khớp, bảng bị
    // nuốt vào `.text` và `__vector_base` (do linker cấp cho output section riêng) trỏ vào chỗ
    // rỗng. Đây là lỗi thật đã gặp khi dựng KT-248: symbol `__vec_*` nằm ở `.text` còn
    // `__vector_base` trỏ vào `.text.vectors` size 0.
    ".section .vectors, \"ax\"",
    ".p2align 11",
    ".macro VIFAST_VEC idx",
    ".org \\idx * {stride}",
    ".globl __vec_\\idx",
    "__vec_\\idx:",
    "mov x9, #\\idx",
    "b vec_common",
    ".endm",
    "VIFAST_VEC 0",
    "VIFAST_VEC 1",
    "VIFAST_VEC 2",
    "VIFAST_VEC 3",
    "VIFAST_VEC 4",
    "VIFAST_VEC 5",
    "VIFAST_VEC 6",
    "VIFAST_VEC 7",
    "VIFAST_VEC 8",
    "VIFAST_VEC 9",
    "VIFAST_VEC 10",
    "VIFAST_VEC 11",
    "VIFAST_VEC 12",
    "VIFAST_VEC 13",
    "VIFAST_VEC 14",
    "VIFAST_VEC 15",
    ".purgem VIFAST_VEC",
    // ---- stub chung: lưu khung, đọc thanh ghi nguyên nhân, gọi handler Rust
    "vec_common:",
    "sub sp, sp, #176",
    "stp x0, x1, [sp, #0]",
    "stp x2, x3, [sp, #16]",
    "stp x4, x5, [sp, #32]",
    "stp x6, x7, [sp, #48]",
    "stp x8, x9, [sp, #64]",
    "stp x10, x11, [sp, #80]",
    "stp x12, x13, [sp, #96]",
    "stp x14, x15, [sp, #112]",
    "stp x16, x17, [sp, #128]",
    "stp x18, x29, [sp, #144]",
    "str x30, [sp, #160]",
    "mov x0, x9",
    "mrs x1, esr_el1",
    "mrs x2, far_el1",
    "mrs x3, elr_el1",
    "mrs x4, spsr_el1",
    "bl vec_handler",
    // x0 = 0 ⇒ dừng; khác 0 ⇒ khôi phục + eret (handler đã sửa ELR_EL1 nếu cần).
    "cbz x0, vec_halt",
    "ldp x0, x1, [sp, #0]",
    "ldp x2, x3, [sp, #16]",
    "ldp x4, x5, [sp, #32]",
    "ldp x6, x7, [sp, #48]",
    "ldp x8, x9, [sp, #64]",
    "ldp x10, x11, [sp, #80]",
    "ldp x12, x13, [sp, #96]",
    "ldp x14, x15, [sp, #112]",
    "ldp x16, x17, [sp, #128]",
    "ldp x18, x29, [sp, #144]",
    "ldr x30, [sp, #160]",
    "add sp, sp, #176",
    "eret",
    "vec_halt:",
    "wfi",
    "b vec_halt",
    stride = const vector::VECTOR_STRIDE,
);

// Địa chỉ bảng vector do **linker** cấp (`__vector_base` trong `linker.ld`).
//
// Lấy **địa chỉ** của một `extern static` là an toàn (không đọc giá trị) ⇒ ảnh không cần
// `unsafe` (luật R5).
extern "C" {
    static __vector_base: u8;
}

/// Địa chỉ bảng vector của ảnh này.
fn vector_base() -> u64 {
    core::ptr::addr_of!(__vector_base) as u64
}

// ---------------------------------------------------------------------------------------------
// Trạng thái trap đã bắt (in bằng chứng sau khi resume)
// ---------------------------------------------------------------------------------------------

/// Số lần `brk #0` đã bắt (phải đúng **1**).
static BRK_COUNT: AtomicU64 = AtomicU64::new(0);
/// Ô vector đã dùng (phải là `4` = `SyncCurrentElSpX`).
static BRK_SLOT: AtomicU64 = AtomicU64::new(u64::MAX);
/// `ESR_EL1` thô của lần `brk` (phải có `EC = 0x3C`).
static BRK_ESR: AtomicU64 = AtomicU64::new(0);
/// `ELR_EL1` của lần `brk` (phải bằng địa chỉ instruction `brk #0`).
static BRK_ELR: AtomicU64 = AtomicU64::new(0);
/// `FAR_EL1` của lần `brk` (BRK không đặt FAR ⇒ 0 hoặc giá trị cũ; chỉ in, không khẳng định).
static BRK_FAR: AtomicU64 = AtomicU64::new(0);
/// `SPSR_EL1` của lần `brk` (phải là `EL1h` = `M[3:0] = 0b0101`).
static BRK_SPSR: AtomicU64 = AtomicU64::new(0);
/// Số trap **không** phải `BRK` (phải 0 — mọi trap khác là lỗi thật, in rồi dừng).
static OTHER_TRAPS: AtomicU64 = AtomicU64::new(0);

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
/// Dòng `PANIC` **không bao giờ** là `ok` trong TAP, nên runner coi lần boot đó là đỏ.
#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    let mut console = Console;
    let _ = writeln!(console, "PANIC: {info}");
    debug_halt(1)
}

// ---------------------------------------------------------------------------------------------
// Handler đồng bộ (Rust) — được `vec_common` gọi
// ---------------------------------------------------------------------------------------------

/// Handler của **mọi** ô vector (16 ô dùng chung một stub).
///
/// Trả `1` ⇒ `vec_common` khôi phục thanh ghi và `eret` (đường resume); trả `0` ⇒ dừng máy.
/// Hàm **không** trả về khi trap không phải `BRK`: nó in nguyên nhân rồi `debug_halt(2)` — một
/// fault thật ở bước 3 là lỗi, không được "resume rồi hy vọng".
#[no_mangle]
pub extern "C" fn vec_handler(slot: u64, esr_raw: u64, far: u64, elr: u64, spsr: u64) -> u64 {
    let esr = Esr::from_bits(esr_raw);
    let ec = esr.ec_raw();
    if esr.ec() == Some(ExceptionClass::Brk64) {
        BRK_COUNT.fetch_add(1, Ordering::Relaxed);
        BRK_SLOT.store(slot, Ordering::Relaxed);
        BRK_ESR.store(esr_raw, Ordering::Relaxed);
        BRK_ELR.store(elr, Ordering::Relaxed);
        BRK_FAR.store(far, Ordering::Relaxed);
        BRK_SPSR.store(spsr, Ordering::Relaxed);
        // `BRK`: ELR_EL1 = **địa chỉ instruction BRK** (DDI 0487 §D17.2.37) ⇒ resume ở ELR + 4.
        asm::write_elr_el1(elr + 4);
        return 1;
    }
    // Trap thật: in đủ nguyên nhân rồi dừng. Không resume — che lỗi là điều tệ nhất có thể làm.
    OTHER_TRAPS.fetch_add(1, Ordering::Relaxed);
    let mut console = Console;
    let _ = writeln!(
        console,
        "# trap: slot={slot} off=0x{:03x} ec=0x{ec:02x} il={} far=0x{far:x} elr=0x{elr:x} spsr=0x{spsr:x}",
        slot * vector::VECTOR_STRIDE,
        esr.il() as u8,
    );
    debug_halt(2)
}

// ---------------------------------------------------------------------------------------------
// kmain
// ---------------------------------------------------------------------------------------------

/// Điểm vào Rust, được `_start` gọi sau khi đã có stack và `.bss` sạch.
///
/// `dtb` = địa chỉ DTB QEMU truyền trong `x0` (giữ trong chữ ký để KT-250 in magic DTB làm bằng
/// chứng "địa chỉ nền tảng là thật"; B3.1/B3.2 chưa đọc DTB).
#[no_mangle]
pub extern "C" fn kmain(_dtb: u64) -> ! {
    // PL011 theo đúng chuỗi DDI 0183 (tắt → chia tần → khung → xoá ngắt → bật).
    let _steps = pl011::init();
    let _ = pl011::write_str(BOOT_MARKER);

    // ---- KT-248: cài bảng vector rồi chứng minh nó sống bằng một `brk #0` cố ý.
    let base = vector_base();
    let vbar = vector::install(base);
    let mut console = Console;
    let _ = writeln!(
        console,
        "# vector: base=0x{base:x} vbar=0x{vbar:x} slots={} stride=0x{:x} align=0x{:x} aligned={}",
        vector::VECTOR_COUNT,
        vector::VECTOR_STRIDE,
        vector::VECTOR_ALIGN,
        vector::is_aligned(base) as u8,
    );

    let expected = asm::brk_probe_addr();
    asm::brk_probe();

    // ---- Bằng chứng sau khi handler resume: EC đúng của BRK, ELR trỏ đúng instruction.
    let count = BRK_COUNT.load(Ordering::Relaxed);
    let slot = BRK_SLOT.load(Ordering::Relaxed);
    let esr = Esr::from_bits(BRK_ESR.load(Ordering::Relaxed));
    let elr = BRK_ELR.load(Ordering::Relaxed);
    let spsr = BRK_SPSR.load(Ordering::Relaxed);
    let far = BRK_FAR.load(Ordering::Relaxed);
    let kind = VectorKind::from_index(slot as usize);
    let _ = writeln!(
        console,
        "# brk: count={count} slot={slot} kind={} off=0x{:03x} ec=0x{:02x} il={} elr=0x{elr:x} expect=0x{expected:x} match={} spsr=0x{spsr:x} far=0x{far:x}",
        kind.map(VectorKind::name).unwrap_or("?"),
        slot * vector::VECTOR_STRIDE,
        esr.ec_raw(),
        esr.il() as u8,
        (elr == expected) as u8,
    );
    let _ = writeln!(
        console,
        "# trap: other={}",
        OTHER_TRAPS.load(Ordering::Relaxed)
    );
    debug_halt(0)
}
