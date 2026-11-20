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

use vifast_plat_aarch64::boot_mmu::{
    apply_step, map_identity, mmu_is_on, sctlr_for_mmu_on, ttbr0_base, MMU_ENABLE_SEQUENCE,
};
use vifast_plat_aarch64::bootmem;
use vifast_plat_aarch64::format::{Aarch64, MemAttr, Perms};
use vifast_plat_aarch64::mmu::PageTable;
use vifast_plat_aarch64::resources::{GIC, GIC_CPU, UART0};
use vifast_plat_aarch64::sysreg::{Esr, ExceptionClass, MAIR_EL1_KERNEL, TCR_EL1_KERNEL};
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
    // Biên vùng ảnh do `linker.ld` cấp — dùng để identity map **đúng** khoảng ảnh chiếm
    // (kể cả `.stack` NOLOAD, vì stack phải dùng được sau khi bật MMU).
    static __text_start: u8;
    static __kernel_end: u8;
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
// KT-249 — trạng thái đo của MMU
// ---------------------------------------------------------------------------------------------

/// Marker ghi **trước** khi bật MMU, đọc lại **sau** khi bật: cùng một địa chỉ ảo (identity) nên
/// giá trị phải không đổi. Đây là phép đo "MMU bật mà bản đồ đúng" của B3.3.
static MMU_MARKER: AtomicU64 = AtomicU64::new(0);
/// Giá trị marker — hằng để không thể "đọc lại chính thứ vừa ghi" một cách vô nghĩa.
const MMU_MARKER_VALUE: u64 = 0x5649_4641_5354_0001;

/// Trang **probe** cho phép đo "MMU thật sự bật": nó nằm trong `.bss` (PA == VA trước MMU) và
/// được map vào một **VA không identity** ([`PROBE_VA`]). Đọc qua VA ấy:
///
/// - MMU **bật** ⇒ dịch sang PA của trang này ⇒ đọc ra [`PROBE_MAGIC`];
/// - MMU **tắt** ⇒ đọc thẳng PA `0x5000_0000` (ngoài RAM 128 MiB của QEMU) ⇒ giá trị khác.
///
/// Đây là phép đo **không thể** đạt được bằng cách chỉ đọc `SCTLR_EL1.M` (bit có thể bật mà
/// translation chưa có hiệu lực).
#[repr(align(4096))]
struct ProbePage([AtomicU64; 512]);

/// Trang probe — `.bss`, zero bởi `_start`.
static PROBE_PAGE: ProbePage = ProbePage([const { AtomicU64::new(0) }; 512]);

/// VA **không identity** của trang probe (ngoài RAM 128 MiB của QEMU `virt`).
const PROBE_VA: u64 = 0x5000_0000;
/// Giá trị nhận dạng ghi vào trang probe trước khi bật MMU.
const PROBE_MAGIC: u64 = 0x4D4D_5500_0000_0001;

// ---------------------------------------------------------------------------------------------
// KT-250 — TAP trên serial
// ---------------------------------------------------------------------------------------------

/// Số ca kiểm TAP mà ảnh phát ra (khớp `1..N` và `EXPECT_TESTS` của runner).
pub const TAP_TESTS: usize = 5;

/// In một dòng TAP `ok N - <tên>` / `not ok N - <tên>`; trả `ok`.
///
/// TAP 13 (Test Anything Protocol): dòng `ok`/`not ok` phải bắt đầu dòng, số hiệu liên tục, và
/// `#` là comment. Runner (`v2/test/boot/run-aarch64.sh`) đếm **đúng** `N` dòng `ok`, đòi
/// `1..N`, và coi mọi `not ok`/`PANIC` là đỏ.
fn tap(console: &mut Console, n: usize, name: &str, ok: bool) -> bool {
    let _ = writeln!(console, "{} {n} - {name}", if ok { "ok" } else { "not ok" });
    ok
}

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
    let mut console = Console;
    // TAP 13: dòng phiên bản phải là dòng **đầu tiên**; plan `1..N` ngay sau.
    let _ = writeln!(console, "TAP version 13");
    let _ = writeln!(console, "1..{TAP_TESTS}");

    // ---- Ca 1: PL011 nạp đủ chuỗi khởi tạo (DDI 0183: tắt → chia tần → khung → xoá ngắt → bật).
    let steps = pl011::init();
    if !tap(
        &mut console,
        1,
        "pl011: init sequence applied",
        steps == pl011::INIT_SEQUENCE.len(),
    ) {
        debug_halt(5)
    }

    // ---- KT-248: cài bảng vector rồi chứng minh nó sống bằng một `brk #0` cố ý.
    let base = vector_base();
    let vbar = vector::install(base);
    let _ = writeln!(
        console,
        "# vector: base=0x{base:x} vbar=0x{vbar:x} slots={} stride=0x{:x} align=0x{:x} aligned={}",
        vector::VECTOR_COUNT,
        vector::VECTOR_STRIDE,
        vector::VECTOR_ALIGN,
        vector::is_aligned(base) as u8,
    );
    if !tap(
        &mut console,
        2,
        "vector: 16 slots, VBAR_EL1 installed",
        vbar == base && vector::is_aligned(base),
    ) {
        debug_halt(5)
    }

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
    let brk_ok = count == 1
        && slot == 4
        && esr.ec() == Some(ExceptionClass::Brk64)
        && elr == expected
        && OTHER_TRAPS.load(Ordering::Relaxed) == 0;
    if !tap(
        &mut console,
        3,
        "brk #0: ec=0x3c, elr matches instruction",
        brk_ok,
    ) {
        debug_halt(5)
    }

    // ---- KT-249: bật MMU (TCR → MAIR → TTBR0 → SCTLR → ISB) trên identity map.
    let mmu = mmu_bringup(&mut console);
    if !tap(&mut console, 4, "mmu: SCTLR_EL1.M=1 after ISB", mmu.on) {
        debug_halt(5)
    }
    if !tap(
        &mut console,
        5,
        "mmu: identity marker + non-identity probe VA",
        mmu.marker_ok && mmu.probe_ok,
    ) {
        debug_halt(5)
    }

    // Marker **duy nhất** của ảnh — chỉ in khi **mọi** ca TAP đã xanh (nếu in sớm, một ảnh hỏng
    // vẫn có marker và runner dễ nhận nhầm là boot sạch).
    let _ = pl011::write_str(BOOT_MARKER);
    let _ = writeln!(console, "# viFastOs aarch64: halt");
    debug_halt(0)
}

/// Kết quả ba phép đo của B3.3 — `mmu_bringup` trả về để `kmain` phát TAP.
#[derive(Clone, Copy, Debug, Default)]
struct MmuOutcome {
    /// `SCTLR_EL1.M == 1` sau `ISB`.
    on: bool,
    /// Marker ghi trước khi bật, đọc lại qua VA sau khi bật — không đổi.
    marker_ok: bool,
    /// Đọc trang probe qua **VA không identity** ra đúng magic.
    probe_ok: bool,
}

/// **KT-249** — dựng bảng trang, identity map vùng ảnh + UART + GIC, rồi bật MMU theo đúng
/// [`MMU_ENABLE_SEQUENCE`], in marker **trước và sau từng bước** (R5 của thiết kế: sai thứ tự ⇒
/// treo không log), và đo ba thứ:
///
/// 1. `SCTLR_EL1.M == 1` **sau** `ISB` (đọc lại thanh ghi);
/// 2. marker ghi trước khi bật, đọc lại qua VA sau khi bật — **không đổi**;
/// 3. trang probe map vào **VA không identity**: đọc qua VA ra đúng magic ⇒ translation thật sự
///    chạy, không chỉ bit `M` được bật.
fn mmu_bringup(console: &mut Console) -> MmuOutcome {
    let text_start = core::ptr::addr_of!(__text_start) as u64;
    let kernel_end = core::ptr::addr_of!(__kernel_end) as u64;
    let probe_pa = core::ptr::addr_of!(PROBE_PAGE.0) as u64;

    // Marker + magic ghi **trước** khi bật MMU (PA == VA ở thời điểm này).
    MMU_MARKER.store(MMU_MARKER_VALUE, Ordering::Relaxed);
    PROBE_PAGE.0[0].store(PROBE_MAGIC, Ordering::Relaxed);

    let mem = bootmem::boot_memory();
    let arena_base = mem.base_hint();
    let frames_total = mem.frames_total();
    let mut table = match PageTable::<Aarch64, bootmem::BootMemory>::new(mem) {
        Ok(table) => table,
        Err(error) => {
            let _ = writeln!(console, "PANIC: mmu: tạo bảng trang lỗi {error:?}");
            debug_halt(3)
        }
    };
    // Vùng ảnh: RWX (W^X đầy đủ là việc của tầng VSpace, B3.5 — không thuộc bước 3).
    let pages_kernel = match map_identity(
        &mut table,
        text_start,
        kernel_end,
        Perms::KERNEL_RWX,
        MemAttr::Normal,
    ) {
        Ok(pages) => pages,
        Err(error) => {
            let _ = writeln!(console, "PANIC: mmu: map kernel lỗi {error:?}");
            debug_halt(3)
        }
    };
    // UART + GIC là **Device**: truy cập không gộp/không đoán trước (Normal sẽ sai ngữ nghĩa).
    let pages_uart = match map_identity(
        &mut table,
        UART0.base,
        UART0.end(),
        Perms::KERNEL_RW,
        MemAttr::Device,
    ) {
        Ok(pages) => pages,
        Err(error) => {
            let _ = writeln!(console, "PANIC: mmu: map uart lỗi {error:?}");
            debug_halt(3)
        }
    };
    let pages_gic = match map_identity(
        &mut table,
        GIC.base,
        GIC_CPU.end(),
        Perms::KERNEL_RW,
        MemAttr::Device,
    ) {
        Ok(pages) => pages,
        Err(error) => {
            let _ = writeln!(console, "PANIC: mmu: map gic lỗi {error:?}");
            debug_halt(3)
        }
    };
    // Trang probe: **VA không identity** → PA của trang trong `.bss`.
    if let Err(error) = table.map_with(
        PROBE_VA,
        probe_pa,
        Perms::KERNEL_RW,
        MemAttr::Normal,
        vifast_plat_aarch64::format::Shareability::Inner,
    ) {
        let _ = writeln!(console, "PANIC: mmu: map probe lỗi {error:?}");
        debug_halt(3)
    }

    let ttbr0 = match ttbr0_base(&table) {
        Some(pa) => pa,
        None => {
            let _ = writeln!(console, "PANIC: mmu: không có bảng mức 1 cho TTBR0");
            debug_halt(3)
        }
    };
    let tcr = TCR_EL1_KERNEL.bits();
    let mair = MAIR_EL1_KERNEL.bits();
    let sctlr = sctlr_for_mmu_on(asm::read_sctlr_el1());
    let _ = writeln!(
        console,
        "# mmu: arena=0x{arena_base:x} frames={frames_total} used={} root=0x{:x} ttbr0=0x{ttbr0:x} level=1",
        table.memory().frames_used(),
        table.root(),
    );
    let _ = writeln!(
        console,
        "# mmu: map kernel=0x{text_start:x}-0x{kernel_end:x} pages={pages_kernel} uart={pages_uart} gic={pages_gic} probe_va=0x{PROBE_VA:x} probe_pa=0x{probe_pa:x}",
    );
    let _ = writeln!(
        console,
        "# mmu: tcr=0x{tcr:016x} mair=0x{mair:016x} sctlr=0x{sctlr:016x}",
    );

    // Chuỗi bật MMU: marker **trước và sau từng bước** (R5). Nếu treo ở bước nào, log cho biết
    // bước ấy — đây là toàn bộ mục đích của các dòng này.
    for (index, step) in MMU_ENABLE_SEQUENCE.iter().enumerate() {
        let _ = writeln!(
            console,
            "# mmu: step {}/{} {} begin",
            index + 1,
            MMU_ENABLE_SEQUENCE.len(),
            step.name()
        );
        apply_step(*step, tcr, mair, ttbr0, sctlr);
        let _ = writeln!(
            console,
            "# mmu: step {}/{} {} ok",
            index + 1,
            MMU_ENABLE_SEQUENCE.len(),
            step.name()
        );
    }

    // ---- Đo 1: `SCTLR_EL1.M` sau `ISB`.
    let sctlr_now = asm::read_sctlr_el1();
    let on = mmu_is_on(sctlr_now);
    let _ = writeln!(
        console,
        "# mmu: on={} sctlr=0x{sctlr_now:016x} m={}",
        on as u8,
        (sctlr_now & 1) as u8,
    );

    // ---- Đo 2: marker qua VA (identity) không đổi giá trị.
    let marker = MMU_MARKER.load(Ordering::Relaxed);
    let _ = writeln!(
        console,
        "# mmu: marker before=0x{MMU_MARKER_VALUE:016x} after=0x{marker:016x} same={}",
        (marker == MMU_MARKER_VALUE) as u8,
    );

    // ---- Đo 3: đọc qua **VA không identity** ⇒ chỉ đúng khi translation thật sự chạy.
    let probe = asm::read_u64(PROBE_VA);
    let _ = writeln!(
        console,
        "# mmu: probe va=0x{PROBE_VA:x} read=0x{probe:016x} magic=0x{PROBE_MAGIC:016x} match={}",
        (probe == PROBE_MAGIC) as u8,
    );

    // Ba phép đo trên là **điều kiện** của B3.3, không phải thông tin tham khảo: `kmain` phát
    // TAP từ chúng và dừng máy nếu bất kỳ phép nào sai.
    MmuOutcome {
        on,
        marker_ok: marker == MMU_MARKER_VALUE,
        probe_ok: probe == PROBE_MAGIC,
    }
}
