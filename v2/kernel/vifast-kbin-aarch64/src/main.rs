// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ảnh boot **aarch64** của ViFastOs — KT-247 (B3.1) + KT-248 (B3.2) + KT-249 (B3.3) +
//! KT-253 (B3.4) + KT-254 (B3.5) + KT-259 (B3.9: **2 hart**).
//!
//! # Đường boot
//!
//! ```text
//!   qemu-system-aarch64 -M virt,gic-version=3 -cpu cortex-a72 -m 128M -nographic -smp 2 -kernel <elf>
//!        │
//!        ├─ QEMU nạp ELF tại 0x4008_0000 (linker.ld), nhảy vào _start ở EL1, x0 = địa chỉ DTB
//!        ├─ _start: sp ← __stack_top (slot 0 arena per-hart); zero .bss; gọi kmain
//!        ├─ kmain (VA **thấp**, identity): PL011 → VBAR_EL1 → brk #0 → dựng **hai** bảng trang
//!        │     (nửa thấp + nửa cao) → bật MMU (TCR→MAIR→TTBR0→TTBR1→SCTLR→ISB)
//!        ├─ jump_high: sp ← VA nửa cao của stack, br tới VA nửa cao của high_entry
//!        ├─ high_entry (VA **nửa cao**): đo PC thật + physmap, phát TAP 6/7
//!        ├─ VSpace thật (KT-254): map/walk/activate/unmap + fault `FaultKind::Vm` — TAP 8/9
//!        ├─ KT-259: GICv3 (distributor + redistributor hart 0) → công bố thanh ghi MMU →
//!        │     PSCI `CPU_ON(1, _start_secondary, 1)` → hart 1 online (TAP 10) → SGI 0 làm IPI
//!        │     (TAP 11) → cả hai hart in hart id (TAP 12)
//!        └─ debug_halt: PSCI SYSTEM_OFF (hvc #0, x0 = 0x8400_0008) ⇒ QEMU thoát mã 0
//! ```
//!
//! # KT-259 — hai hart (B3.9, D2-202)
//!
//! - **`_start_secondary`** (asm): hart 1 khởi động với MMU **tắt** (PSCI reset state) nên nó
//!   chạy ở **PA**; nó đọc `MPIDR_EL1` (**không** tin `x0`/context id), tự chọn stack **per-hart**
//!   trong arena `__smp_stacks_start` (guard 4 KiB + stack 64 KiB mỗi hart), bật `CPACR_EL1`
//!   (bài học KT-254: compiler phát NEON) rồi gọi [`kmain_secondary`].
//! - **[`kmain_secondary`]** nạp **đúng** bộ `TCR/MAIR/TTBR0/TTBR1/SCTLR` mà hart 0 đã công bố
//!   (dùng lại [`MMU_ENABLE_SEQUENCE`] — **một** định nghĩa chuỗi), cài `VBAR_EL1` VA thấp, khởi
//!   tạo GICv3 **của chính nó** (wake redistributor, enable `SGI 0`, `ICC_*_EL1`), in
//!   `# smp: hart=1 online`, rồi chờ IPI.
//! - **GICv3**: distributor `0x0800_0000` (`ARE_NS | EnableGrp1NS`), redistributor
//!   `0x080A_0000 + hart × 0x2_0000`; IPI = `SGI 0` gửi bằng `ICC_SGI1R_EL1`, nhận ở ô vector
//!   **IRQ EL1h** (slot 5) — handler đọc `ICC_IAR1_EL1`, in, đếm, ghi `ICC_EOIR1_EL1`.
//! - **Đo được**: TAP 10 (`hart 1 online`), TAP 11 (`ipi 0->1`), TAP 12 (`both harts report`).
//!
//! **Giới hạn đã biết (ghi rõ, không giấu)**: chỉ **2 hart**; **không** smp 4, **không** timer
//! per-hart, **không** preemption, **không** U-mode; hart 1 chạy ở **VA thấp** (hart 0 ở nửa cao)
//! nhưng **cùng** bảng trang `VSpace` — không có "mỗi hart một address space". **Không** tuyên bố
//! K4' đạt (tiêu chí K4' là "như K3": smp 1|2|4 + soak 1.000).
//!
//! # Phạm vi (D2-195 + D2-199 + D2-202)
//!
//! Bước 3 mở **B3.1–B3.5 + B3.8 + B3.9**. Ảnh này có B3.1–B3.5 và B3.9; **chưa** TLBI shootdown
//! (B3.6), **chưa** U-mode (B3.7). **Không** tuyên bố K4' đạt.

#![no_std]
#![no_main]

use core::arch::global_asm;
use core::fmt::{self, Write};
use core::sync::atomic::{AtomicU64, Ordering};

use vifast_plat_aarch64::boot_mmu::{
    apply_step, map_identity, mmu_is_on, sctlr_for_mmu_on, MMU_ENABLE_SEQUENCE,
};
use vifast_plat_aarch64::bootmem;
use vifast_plat_aarch64::format::{MemAttr, Perms};
use vifast_plat_aarch64::gic;
use vifast_plat_aarch64::mmu::{MapError, TableMemory};
use vifast_plat_aarch64::resources::{GIC, GIC_CPU, UART0};
use vifast_plat_aarch64::smp;
use vifast_plat_aarch64::split_mmu::{high_alias_va, SplitMmu, KERNEL_HIGH_BASE, PHYSMAP_BASE};
use vifast_plat_aarch64::sysreg::{Esr, ExceptionClass, MAIR_EL1_KERNEL, TCR_EL1_KERNEL};
use vifast_plat_aarch64::vector::{self, VectorKind};
use vifast_plat_aarch64::vspace::{Frame, VSpace, VSpaceId};
use vifast_plat_aarch64::{asm, debug_halt, pl011, tlb};
use vifast_plat_common::fault::{vm_reason, FaultAccess, FaultKind, VmFaultReason};

/// **KT-259**: trạng thái SMP + `smp_bringup` + `kmain_secondary` nằm ở tệp riêng (luật R1:
/// tệp trong `kernel/` ≤ 1.500 dòng). Tên `smpboot` để **không** đụng `vifast_plat_aarch64::smp`.
mod smpboot;

/// `MAX_HARTS` của crate arch phải **trùng** hằng dùng chung (D2-195 Q5) — lệch là lỗi biên dịch.
const _: () = assert!(smp::MAX_HARTS == vifast_plat_common::smp::MAX_HARTS);

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
    // 0. FP/SIMD: trình biên dịch phát NEON cho hàm lớn (`PageTable::unmap` zero 176 B stack);
    //    `CPACR_EL1.FPEN = 0` ⇒ Undefined Instruction (EC=0x07). Bật **trước** mọi mã Rust.
    "mov x9, #{cpacr}",
    "msr cpacr_el1, x9",
    "isb",
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
    // ---------------------------------------------------------------------------------------
    // KT-259 — điểm vào của **hart phụ** (PSCI `CPU_ON` nhảy vào đây).
    //
    // Trạng thái do PSCI đặt: EL1h, `DAIF` mask đủ, **MMU tắt**, cache tắt, `x0 = context_id`.
    // Vì MMU tắt nên mọi địa chỉ ở đây là **PA** — và ảnh liên kết tại PA (`0x4008_0000`) nên
    // `adrp`/`add` cho đúng PA của symbol (không cần bảng trang để chạy đoạn này).
    //
    // Hart id lấy từ `MPIDR_EL1`, **không** từ `x0`: `x0` là giá trị hart gọi tự đặt, sai thì
    // hart phụ chạy trên stack của hart khác mà không ai biết. `x0` được chuyển tiếp vào
    // `kmain_secondary` như `ctx` để **so** và in bằng chứng.
    ".globl _start_secondary",
    "_start_secondary:",
    // 1. hart id = Aff0 của MPIDR_EL1 (topology phẳng của QEMU virt)
    "mrs x19, mpidr_el1",
    "and x19, x19, #0xff",
    // 2. sp = __smp_stacks_start + (hart + 1) * HART_STRIDE  (đỉnh slot của **hart này**)
    "adrp x9, __smp_stacks_start",
    "add x9, x9, :lo12:__smp_stacks_start",
    // `mov` không mã hoá được immediate 16-bit lệch (0x1_1000) ⇒ dựng bằng `movz` + `movk`.
    "movz x10, #{stride_lo}",
    "movk x10, #{stride_hi}, lsl #16",
    "madd x9, x19, x10, x9",
    "add x9, x9, x10",
    "mov sp, x9",
    // 3. FP/SIMD **trước** mọi mã Rust — compiler phát NEON cho hàm lớn (bài học KT-254:
    //    `CPACR_EL1.FPEN = 0` ⇒ Undefined Instruction EC=0x07).
    "mov x9, #{cpacr}",
    "msr cpacr_el1, x9",
    "isb",
    // 4. vào Rust: x0 = hart (từ MPIDR), x1 = context_id (PSCI chuyển nguyên văn)
    "mov x1, x0",
    "mov x0, x19",
    "bl kmain_secondary",
    // `kmain_secondary` không trả về; nếu có thì dừng an toàn.
    "7:",
    "wfi",
    "b 7b",
    cpacr = const vifast_plat_aarch64::sysreg::CPACR_EL1_FP_SIMD,
    stride_lo = const (smp::HART_STRIDE & 0xFFFF),
    stride_hi = const (smp::HART_STRIDE >> 16),
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
    // Biên vùng ảnh do `linker.ld` cấp — dùng để map **đúng** khoảng ảnh chiếm (kể cả `.stack`
    // NOLOAD, vì stack phải dùng được sau khi bật MMU).
    static __text_start: u8;
    static __kernel_end: u8;
    // Đỉnh stack (cuối `.stack` NOLOAD) — KT-253 dùng để tính SP nửa cao.
    static __stack_top: u8;
    // **KT-259**: arena stack per-hart (NOLOAD) — `_start_secondary` và `HartStacks` dùng.
    static __smp_stacks_start: u8;
    static __smp_stacks_end: u8;
    /// Điểm vào của hart phụ — chỉ lấy **địa chỉ** (PA) để truyền cho PSCI `CPU_ON`.
    fn _start_secondary();
}

/// Địa chỉ bảng vector của ảnh này (VA **thấp** — nơi linker đặt nó).
fn vector_base() -> u64 {
    core::ptr::addr_of!(__vector_base) as u64
}

/// Ô vector **IRQ** khi đang ở EL1 dùng `SP_EL0` (`0x080`) — KT-259.
const IRQ_SLOT_EL1T: u64 = 1;
/// Ô vector **IRQ** khi đang ở EL1 dùng `SP_ELx` (`0x280`) — ô mà kernel này nhận (EL1h).
const IRQ_SLOT_EL1H: u64 = 5;

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
// KT-253 — trạng thái đo của nửa cao + physmap
// ---------------------------------------------------------------------------------------------

/// Trang **physmap probe**: ghi magic qua VA thấp (identity) **trước** khi nhảy, đọc lại qua
/// **cửa sổ physmap** (`PHYSMAP_BASE + PA`) **sau** khi nhảy. Đây là phép đo "physmap đọc được
/// PA bất kỳ đã map" của B3.4 — nếu cửa sổ physmap thiếu, lần đọc fault và bảng vector in
/// `# trap:` rồi dừng (runner đỏ), không im lặng.
#[repr(align(4096))]
struct PhysProbePage([AtomicU64; 512]);

/// Trang physmap probe — `.bss`, nằm trong khoảng ảnh nên được map cả identity (nửa thấp) lẫn
/// physmap (nửa cao).
static PHYS_PROBE: PhysProbePage = PhysProbePage([const { AtomicU64::new(0) }; 512]);

/// Giá trị ghi vào trang physmap probe trước khi nhảy.
const PHYSMAP_MAGIC: u64 = 0x5048_5953_4D41_5001;

/// VA nửa cao mong đợi của [`high_entry`] (tính **trước** khi nhảy, đọc lại sau khi nhảy).
static HIGH_EXPECT_VA: AtomicU64 = AtomicU64::new(0);
/// PA mong đợi của [`high_entry`].
static HIGH_EXPECT_PA: AtomicU64 = AtomicU64::new(0);
/// Kết quả `translate(HIGH_EXPECT_VA)` bằng **walk phần mềm** trên bảng nửa cao (trước khi nhảy).
static HIGH_TRANSLATED_PA: AtomicU64 = AtomicU64::new(u64::MAX);
/// VA nửa cao của `kmain` (in trước khi nhảy — "VA nửa cao của kmain" theo yêu cầu B3.4).
static KMAIN_HIGH_VA: AtomicU64 = AtomicU64::new(0);
/// PA của `kmain`.
static KMAIN_PA: AtomicU64 = AtomicU64::new(0);
/// Kết quả `translate(KMAIN_HIGH_VA)`.
static KMAIN_TRANSLATED_PA: AtomicU64 = AtomicU64::new(u64::MAX);
/// VA physmap của trang [`PHYS_PROBE`] (`PHYSMAP_BASE + PA`).
static PHYSMAP_PROBE_VA: AtomicU64 = AtomicU64::new(0);
/// PA của trang [`PHYS_PROBE`].
static PHYSMAP_PROBE_PA: AtomicU64 = AtomicU64::new(0);
/// Số lần [`high_entry`] đã chạy (phải đúng **1**).
static HIGH_ENTRY_COUNT: AtomicU64 = AtomicU64::new(0);
/// PA của `__text_start` (địa chỉ **liên kết**, đọc lúc PC còn ở nửa thấp).
///
/// **Vì sao phải lưu**: sau khi nhảy sang nửa cao, `adrp` là **PC-relative** nên
/// `addr_of!(__text_start)` trả **alias nửa cao** của symbol, không phải PA liên kết. Giá trị
/// PA thật phải đọc **trước** khi nhảy rồi truyền qua static (static được đọc qua `adrp` ⇒ đi
/// qua `TTBR1`, đúng bản đồ nửa cao).
static TEXT_START_PA: AtomicU64 = AtomicU64::new(0);
/// PA của `__vector_base` (địa chỉ liên kết, đọc lúc PC còn ở nửa thấp).
static VECTOR_BASE_PA: AtomicU64 = AtomicU64::new(0);
/// PA của `__kernel_end` (đọc lúc PC còn ở nửa thấp — cùng lý do `TEXT_START_PA`).
static KERNEL_END_PA: AtomicU64 = AtomicU64::new(0);
/// PA của arena bảng trang (đọc lúc PC còn ở nửa thấp).
///
/// `TableMemory` phải dùng **PA** trong descriptor; sau khi nhảy nửa cao, `addr_of!(ARENA.0)`
/// là alias nửa cao nên không dùng làm PA được (xem `bootmem::shared_boot_memory_at`).
static ARENA_PA: AtomicU64 = AtomicU64::new(0);

// ---------------------------------------------------------------------------------------------
// KT-254 — VSpace + fault `FaultKind::Vm`
// ---------------------------------------------------------------------------------------------

/// ASID của VSpace dựng trong ảnh (nhãn `FaultKind::Vm.vspace`).
const VSPACE_ID: VSpaceId = 1;
/// VA **người dùng** của trang probe VSpace — ngoài RAM 128 MiB, **không** identity.
const VSPACE_VA: u64 = 0x6000_0000;
/// Giá trị ghi vào trang probe VSpace **trước** khi `activate` (đọc lại qua VA của VSpace).
const VSPACE_MAGIC: u64 = 0x5653_5041_4345_0001;

/// Trang **VSpace probe**: ghi magic qua VA thấp (identity) trước khi `activate`, đọc lại qua
/// **VA người dùng** của VSpace sau `activate` — chứng minh `TTBR0` đã đổi sang bảng của VSpace.
#[repr(align(4096))]
struct VSpaceProbePage([AtomicU64; 512]);

/// Trang VSpace probe — `.bss`, nằm trong khoảng ảnh nên được map cả identity (bảng kernel) lẫn
/// alias nửa cao (bảng của VSpace).
static VSPACE_PROBE: VSpaceProbePage = VSpaceProbePage([const { AtomicU64::new(0) }; 512]);

/// PA của trang [`VSPACE_PROBE`] (đọc lúc PC còn ở nửa thấp).
static VSPACE_PROBE_PA: AtomicU64 = AtomicU64::new(0);
/// `1` ⇔ lần fault kế tiếp là fault **được chờ** của phép đo VSpace (handler ghi lại rồi resume).
static EXPECT_VSPACE_FAULT: AtomicU64 = AtomicU64::new(0);
/// `1` ⇔ fault đã bắt được phân loại là `FaultKind::Vm`.
static FAULT_KIND_VM: AtomicU64 = AtomicU64::new(0);
/// `FaultKind::Vm.va` của fault đã bắt.
static FAULT_VA: AtomicU64 = AtomicU64::new(0);
/// `FaultKind::Vm.access` (mã ABI) của fault đã bắt.
static FAULT_ACCESS: AtomicU64 = AtomicU64::new(u64::MAX);
/// `FaultKind::Vm.reason` (mã ABI) của fault đã bắt.
static FAULT_REASON: AtomicU64 = AtomicU64::new(u64::MAX);
/// `ESR_EL1` thô của fault đã bắt.
static FAULT_ESR: AtomicU64 = AtomicU64::new(0);
/// `FAR_EL1` của fault đã bắt.
static FAULT_FAR: AtomicU64 = AtomicU64::new(0);
/// `ELR_EL1` của fault đã bắt.
static FAULT_ELR: AtomicU64 = AtomicU64::new(0);

// ---------------------------------------------------------------------------------------------
// KT-250 — TAP trên serial
// ---------------------------------------------------------------------------------------------

/// Số ca kiểm TAP mà ảnh phát ra (khớp `1..N` và `EXPECT_TESTS` của runner).
///
/// 5 ca của KT-249 + 2 ca của KT-253 (nửa cao, physmap) + 2 ca của KT-254 (`VSpace` map/walk/
/// activate, và fault `FaultKind::Vm` sau `unmap`) + 3 ca của KT-259 (hart 1 online, IPI 0→1,
/// cả hai hart báo alive).
pub const TAP_TESTS: usize = 12;

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

    // ---- KT-259: **IRQ** (ô 1 = EL1t, ô 5 = EL1h). `ESR_EL1` của IRQ là 0 (không có syndrome),
    // nên phân loại bằng **ô vector**, không bằng `ESR`. Handler đọc `ICC_IAR1_EL1` (nhận ngắt),
    // in + đếm nếu là `SGI 0`, rồi **bắt buộc** ghi `ICC_EOIR1_EL1` — không ghi thì ngắt không bao
    // giờ tới lại và phép đo IPI sẽ treo thay vì đỏ.
    if slot == IRQ_SLOT_EL1T || slot == IRQ_SLOT_EL1H {
        let intid = gic::ack_irq();
        if !gic::is_spurious(intid) {
            smpboot::note_ipi(intid);
            gic::end_irq(intid);
        }
        return 1;
    }

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
    // ---- KT-254: fault **được chờ** của phép đo VSpace (đọc VA sau `unmap`).
    //
    // Phân loại bằng `ESR_EL1`/`FAR_EL1` thật (không suy diễn từ chỗ khác), dựng
    // `FaultKind::Vm` của ABI (`vifast-plat-common::fault`, D2-56a), ghi lại rồi **resume**
    // (`ELR += 4` — instruction `ldr` 4 byte, không thực thi nên resume an toàn). Chỉ làm vậy
    // khi cờ `EXPECT_VSPACE_FAULT` bật; mọi fault khác vẫn **fatal** như trước.
    if let Some(abort) = vifast_plat_aarch64::fault::classify_esr(esr_raw, far) {
        if EXPECT_VSPACE_FAULT.load(Ordering::Relaxed) == 1 {
            EXPECT_VSPACE_FAULT.store(0, Ordering::Relaxed);
            let access = FaultAccess::from_code(abort.access.code());
            let reason = vm_reason(access, false, None);
            let kind = FaultKind::Vm {
                vspace: VSPACE_ID as u64,
                va: abort.va,
                access,
                reason,
            };
            if let Some((_vspace, va, access, reason)) = kind.vm() {
                FAULT_KIND_VM.store(1, Ordering::Relaxed);
                FAULT_VA.store(va, Ordering::Relaxed);
                FAULT_ACCESS.store(access.code(), Ordering::Relaxed);
                FAULT_REASON.store(reason.code(), Ordering::Relaxed);
            }
            FAULT_ESR.store(esr_raw, Ordering::Relaxed);
            FAULT_FAR.store(far, Ordering::Relaxed);
            FAULT_ELR.store(elr, Ordering::Relaxed);
            asm::write_elr_el1(elr + 4);
            return 1;
        }
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

    // ---- KT-249 + KT-253: dựng hai bảng trang, bật MMU (TCR→MAIR→TTBR0→TTBR1→SCTLR→ISB).
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

    // ---- KT-253: nhảy sang **VA nửa cao**. Từ đây PC/SP ở nửa cao; `high_entry` đo và phát
    // TAP 6/7. Marker **trước** bước nhảy (R5): nếu treo/fault ở đây, log cho biết đúng bước.
    let _ = writeln!(
        console,
        "# high: jump begin entry=0x{:x} sp=0x{:x}",
        mmu.high_entry_va, mmu.high_sp
    );
    asm::jump_high(mmu.high_entry_va, mmu.high_sp);
}

/// **KT-253** — chạy ở **VA nửa cao** (được [`asm::jump_high`] nhảy tới).
///
/// Phép đo (không thể giả):
///
/// 1. `adr` đọc **PC thật** — phải nằm trong trang của [`HIGH_EXPECT_VA`] (VA nửa cao);
/// 2. `translate(HIGH_EXPECT_VA)` bằng walk phần mềm (làm **trước** khi nhảy) phải bằng
///    [`HIGH_EXPECT_PA`];
/// 3. đọc PA của trang [`PHYS_PROBE`] qua **cửa sổ physmap** phải ra [`PHYSMAP_MAGIC`] — chứng
///    minh physmap đọc được PA bất kỳ đã map, không cần identity.
///
/// Sau đó cài lại `VBAR_EL1` bằng **VA nửa cao** của bảng vector (từ đây cả exception vector
/// cũng chạy ở nửa cao), in marker rồi tắt máy sạch.
#[no_mangle]
pub extern "C" fn high_entry() -> ! {
    let mut console = Console;
    HIGH_ENTRY_COUNT.fetch_add(1, Ordering::Relaxed);
    let _ = writeln!(console, "# high: jump ok");

    // ---- Đo 1: PC thật (đọc bằng `adr`) + PA suy ra từ alias.
    let pc = asm::current_pc();
    let expect_va = HIGH_EXPECT_VA.load(Ordering::Relaxed);
    let expect_pa = HIGH_EXPECT_PA.load(Ordering::Relaxed);
    let translated = HIGH_TRANSLATED_PA.load(Ordering::Relaxed);
    // PA liên kết đọc **trước** khi nhảy (xem `TEXT_START_PA`): sau khi nhảy, `adrp` cho alias
    // nửa cao của symbol chứ không phải PA.
    let text_start = TEXT_START_PA.load(Ordering::Relaxed);
    let pc_pa = text_start + (pc - KERNEL_HIGH_BASE);
    let same_page = (pc >> 12) == (expect_va >> 12);
    // `translate` trả PA **đáy trang** (descriptor chỉ giữ bit [47:12]) ⇒ so với đáy trang.
    let high_ok = same_page && translated == (expect_pa & !0xFFF);
    let _ = writeln!(
        console,
        "# high: va=0x{pc:x} pa=0x{pc_pa:x} match={} expect_va=0x{expect_va:x} expect_pa=0x{expect_pa:x} translated=0x{translated:x} same_page={}",
        high_ok as u8,
        same_page as u8,
    );

    // ---- Đo 2: physmap đọc PA đã ghi trước đó (qua VA thấp, trước khi nhảy).
    let physmap_va = PHYSMAP_PROBE_VA.load(Ordering::Relaxed);
    let physmap_pa = PHYSMAP_PROBE_PA.load(Ordering::Relaxed);
    let read = asm::read_u64(physmap_va);
    let physmap_ok = read == PHYSMAP_MAGIC;
    let _ = writeln!(
        console,
        "# physmap: pa=0x{physmap_pa:x} read=0x{read:016x} match={} va=0x{physmap_va:x} expect=0x{PHYSMAP_MAGIC:016x} base=0x{PHYSMAP_BASE:x}",
        physmap_ok as u8,
    );

    // ---- Cài lại VBAR_EL1 bằng VA **nửa cao** của bảng vector: từ đây cả exception vector
    // cũng đi qua `TTBR1` (nếu map nửa cao của ảnh sai, fault tại đây sẽ lộ ngay).
    let high_vector = high_alias_va(
        VECTOR_BASE_PA.load(Ordering::Relaxed),
        text_start,
        KERNEL_HIGH_BASE,
    );
    let vbar = vector::install(high_vector);
    let _ = writeln!(
        console,
        "# high: vbar=0x{vbar:x} high=0x{high_vector:x} match={}",
        (vbar == high_vector) as u8,
    );

    if !tap(
        &mut console,
        6,
        "high: PC at high VA, translate matches PA",
        high_ok,
    ) {
        debug_halt(5)
    }
    if !tap(
        &mut console,
        7,
        "physmap: read PA via physmap window",
        physmap_ok,
    ) {
        debug_halt(5)
    }

    // ---- KT-254 (B3.5): `VSpace` thật — map **một Frame 4 KiB**, `walk`, `activate`, `unmap`,
    // rồi đọc qua VA sau `unmap` để sinh fault `FaultKind::Vm`.
    let text_start = TEXT_START_PA.load(Ordering::Relaxed);
    let kernel_end = KERNEL_END_PA.load(Ordering::Relaxed);
    let probe_pa = VSPACE_PROBE_PA.load(Ordering::Relaxed);
    let arena_pa = ARENA_PA.load(Ordering::Relaxed);
    let (low_mem, high_mem) = bootmem::shared_boot_memory_at(arena_pa);
    let mut vs = match VSpace::create(VSPACE_ID, low_mem, high_mem) {
        Ok(vs) => vs,
        Err(error) => {
            let _ = writeln!(console, "PANIC: vspace: create lỗi {error:?}");
            debug_halt(3)
        }
    };
    // VSpace phải **tự có** alias nửa cao + physmap của kernel: `activate` đổi cả `TTBR1`, nên
    // nếu thiếu, lệnh kế tiếp (PC/SP/static ở nửa cao) fault ngay.
    let pages_high = match vs.map_kernel_high(
        text_start,
        kernel_end,
        KERNEL_HIGH_BASE,
        Perms::KERNEL_RWX,
        MemAttr::Normal,
    ) {
        Ok(pages) => pages,
        Err(error) => {
            let _ = writeln!(console, "PANIC: vspace: map kernel high lỗi {error:?}");
            debug_halt(3)
        }
    };
    let pages_physmap =
        match vs.map_physmap(text_start, kernel_end, Perms::KERNEL_RW, MemAttr::Normal) {
            Ok(pages) => pages,
            Err(error) => {
                let _ = writeln!(console, "PANIC: vspace: map physmap lỗi {error:?}");
                debug_halt(3)
            }
        };
    // **Ảnh kernel identity** trong bảng nửa thấp của VSpace — bắt buộc, không phải tuỳ chọn:
    // ảnh liên kết ở **PA** (`0x4008_0000`), nên `core::fmt` (vtable của `&dyn Display`) và mọi
    // con trỏ hàm trong `.rodata` là **địa chỉ tuyệt đối nửa thấp**. Đổi `TTBR0` sang bảng không
    // map ảnh ⇒ `blr` tới vtable fault (đo được: prefetch abort `FAR=0x4008_0ab0`). Map với
    // `user = false` ⇒ EL0 **không** đọc được ảnh kernel (đúng bất biến cách ly).
    let pages_image = match map_identity_into(
        &mut vs,
        text_start,
        kernel_end,
        Perms::KERNEL_RWX,
        MemAttr::Normal,
    ) {
        Ok(pages) => pages,
        Err(error) => {
            let _ = writeln!(console, "PANIC: vspace: map image identity lỗi {error:?}");
            debug_halt(3)
        }
    };
    // UART + GIC identity (Device) — kernel vẫn in được sau khi `TTBR0` đổi sang bảng VSpace.
    let pages_uart = match map_identity_into(
        &mut vs,
        UART0.base,
        UART0.end(),
        Perms::KERNEL_RW,
        MemAttr::Device,
    ) {
        Ok(pages) => pages,
        Err(error) => {
            let _ = writeln!(console, "PANIC: vspace: map uart lỗi {error:?}");
            debug_halt(3)
        }
    };
    let pages_gic = match map_identity_into(
        &mut vs,
        GIC.base,
        GIC_CPU.end(),
        Perms::KERNEL_RW,
        MemAttr::Device,
    ) {
        Ok(pages) => pages,
        Err(error) => {
            let _ = writeln!(console, "PANIC: vspace: map gic lỗi {error:?}");
            debug_halt(3)
        }
    };
    // **KT-259**: vùng **redistributor** GICv3 (`0x080A_0000 + hart × 0x2_0000`) — KT-254 chỉ map
    // distributor + CPU interface, nên nếu không map thêm ở đây thì lần đọc `GICR_WAKER` đầu tiên
    // sau `activate` sẽ **translation fault** (đo được khi dựng KT-259: `# trap: … ec=0x25`).
    // Map cả dải `MAX_HARTS` hart (1 MiB) — cùng một bảng mức 3 với GICD nên **không** tốn frame.
    let pages_gicr = match map_identity_into(
        &mut vs,
        gic::GICR_BASE,
        gic::GICR_BASE + (smp::MAX_HARTS as u64) * gic::GICR_STRIDE,
        Perms::KERNEL_RW,
        MemAttr::Device,
    ) {
        Ok(pages) => pages,
        Err(error) => {
            let _ = writeln!(console, "PANIC: vspace: map gicr lỗi {error:?}");
            debug_halt(3)
        }
    };
    // **Một Frame 4 KiB** vào VA người dùng (ngoài RAM, không identity).
    let frame = match Frame::new(probe_pa) {
        Some(frame) => frame,
        None => {
            let _ = writeln!(console, "PANIC: vspace: PA probe không căn 4 KiB");
            debug_halt(3)
        }
    };
    if let Err(error) = vs.map_obj(VSPACE_VA, frame, Perms::USER_RW) {
        let _ = writeln!(console, "PANIC: vspace: map_obj lỗi {error:?}");
        debug_halt(3)
    }
    let walk_pa = match vs.walk(VSPACE_VA) {
        Ok(t) => t.pa,
        Err(error) => {
            let _ = writeln!(console, "PANIC: vspace: walk lỗi {error:?}");
            debug_halt(3)
        }
    };
    let map_ok = walk_pa == probe_pa;
    let _ = writeln!(
        console,
        "# vspace: map va=0x{VSPACE_VA:x} pa=0x{probe_pa:x} walk=0x{walk_pa:x} match={} pages_high={pages_high} pages_physmap={pages_physmap} pages_image={pages_image} pages_mmio={}",
        map_ok as u8,
        pages_uart + pages_gic + pages_gicr,
    );

    // `activate`: TTBR0 ← bảng VSpace, TTBR1 ← bảng nửa cao của VSpace (mã hoá bằng `Ttbr`).
    // Marker **trước/sau** (R5): nếu treo/fault ở bước đổi gốc, log cho biết đúng bước.
    let _ = writeln!(console, "# vspace: activate begin");
    if let Err(error) = vs.activate() {
        let _ = writeln!(console, "PANIC: vspace: activate lỗi {error:?}");
        debug_halt(3)
    }
    let _ = writeln!(console, "# vspace: activate ok");
    let _ = writeln!(
        console,
        "# vspace: activate ttbr0=0x{:x} ttbr1=0x{:x} asid={}",
        vs.ttbr0().unwrap_or(0),
        vs.ttbr1().unwrap_or(0),
        vs.id(),
    );
    let read = asm::read_u64(VSPACE_VA);
    let active_ok = read == VSPACE_MAGIC;
    let _ = writeln!(
        console,
        "# vspace: active read=0x{read:016x} magic=0x{VSPACE_MAGIC:016x} match={}",
        active_ok as u8,
    );
    if !tap(
        &mut console,
        8,
        "vspace: map Frame 4 KiB + walk + activate + read",
        map_ok && active_ok,
    ) {
        debug_halt(5)
    }

    // `unmap` + **TLBI cục bộ** (shootdown đa hart là B3.6): không xoá entry TLB thì lần đọc sau
    // vẫn trúng entry cũ và **không** fault — phép đo sẽ nói dối.
    let unmap_pa = match vs.unmap(VSPACE_VA) {
        Ok(pa) => pa,
        Err(error) => {
            let _ = writeln!(console, "PANIC: vspace: unmap lỗi {error:?}");
            debug_halt(3)
        }
    };
    tlb::vae1(VSPACE_VA);
    asm::dsb_ish();
    asm::isb();
    let walk_after = vs.walk(VSPACE_VA);
    let unmap_ok = unmap_pa == probe_pa && walk_after == Err(MapError::NotMapped);
    let _ = writeln!(
        console,
        "# vspace: unmap pa=0x{unmap_pa:x} walk={walk_after:?} match={}",
        unmap_ok as u8,
    );

    // Đọc qua VA sau `unmap` ⇒ **fault**; handler phân loại `FaultKind::Vm` từ `ESR_EL1`/
    // `FAR_EL1` rồi resume (`ELR += 4`) để `high_entry` in bằng chứng và phát TAP 9.
    EXPECT_VSPACE_FAULT.store(1, Ordering::Relaxed);
    let _ = asm::read_u64(VSPACE_VA);
    let vm = FAULT_KIND_VM.load(Ordering::Relaxed) == 1;
    let fault_va = FAULT_VA.load(Ordering::Relaxed);
    let access = FaultAccess::from_code(FAULT_ACCESS.load(Ordering::Relaxed));
    let reason = VmFaultReason::from_code(FAULT_REASON.load(Ordering::Relaxed));
    let fault_ok = vm
        && fault_va == VSPACE_VA
        && access == FaultAccess::Read
        && reason == VmFaultReason::NotMapped
        && unmap_ok;
    let _ = writeln!(
        console,
        "# fault: kind={} va=0x{fault_va:x} access={} reason={} esr=0x{:016x} far=0x{:x} elr=0x{:x} match={}",
        if vm { "Vm" } else { "?" },
        access.name(),
        reason.name(),
        FAULT_ESR.load(Ordering::Relaxed),
        FAULT_FAR.load(Ordering::Relaxed),
        FAULT_ELR.load(Ordering::Relaxed),
        fault_ok as u8,
    );
    if !tap(
        &mut console,
        9,
        "vspace: read after unmap faults with FaultKind::Vm",
        fault_ok,
    ) {
        debug_halt(5)
    }

    // ---- KT-259 (B3.9): hai hart. Chạy **sau** `activate` để cả hai hart dùng **cùng** bảng
    // trang `VSpace` (hart 1 nạp đúng `TTBR0/TTBR1` mà hart 0 đã công bố) — nếu chạy trước, hart 1
    // sẽ ở lại bảng boot trong khi hart 0 đổi sang bảng `VSpace`, tức hai address space khác nhau.
    let sp_high = asm::read_current_sp();
    let text_start = TEXT_START_PA.load(Ordering::Relaxed);
    smpboot::publish_hart0_sp_pa(text_start + (sp_high - KERNEL_HIGH_BASE));
    if !smpboot::smp_bringup(&mut console) {
        debug_halt(5)
    }

    // Marker **duy nhất** của ảnh — chỉ in khi **mọi** ca TAP đã xanh (nếu in sớm, một ảnh hỏng
    // vẫn có marker và runner dễ nhận nhầm là boot sạch).
    let _ = pl011::write_str(BOOT_MARKER);
    let _ = writeln!(console, "# viFastOs aarch64: halt");
    debug_halt(0)
}

/// Map **identity** `[start, end)` vào `VSpace` bằng trang 4 KiB (làm tròn ra ngoài) — dùng cho
/// UART/GIC trong VSpace của ảnh (kernel vẫn cần chúng sau khi `TTBR0` đổi).
fn map_identity_into<M: TableMemory>(
    vs: &mut VSpace<M>,
    start: u64,
    end: u64,
    perms: Perms,
    attr: MemAttr,
) -> Result<usize, MapError> {
    let first = start & !0xFFF;
    let last = (end + 0xFFF) & !0xFFF;
    let mut pages = 0;
    let mut pa = first;
    while pa < last {
        let frame = Frame::new(pa).ok_or(MapError::Unaligned)?;
        vs.map_obj_with(pa, frame, perms, attr)?;
        pages += 1;
        pa += 0x1000;
    }
    Ok(pages)
}

/// Kết quả các phép đo của B3.3 + thông tin nhảy nửa cao của B3.4 — `mmu_bringup` trả về để
/// `kmain` phát TAP và thực hiện bước nhảy.
#[derive(Clone, Copy, Debug, Default)]
struct MmuOutcome {
    /// `SCTLR_EL1.M == 1` sau `ISB`.
    on: bool,
    /// Marker ghi trước khi bật, đọc lại qua VA sau khi bật — không đổi.
    marker_ok: bool,
    /// Đọc trang probe qua **VA không identity** ra đúng magic.
    probe_ok: bool,
    /// VA nửa cao của [`high_entry`] (đích của bước nhảy).
    high_entry_va: u64,
    /// SP nửa cao (đỉnh stack alias sang nửa cao).
    high_sp: u64,
}

/// **KT-249 + KT-253** — dựng **hai** bảng trang (nửa thấp identity + nửa cao alias/physmap),
/// bật MMU theo đúng [`MMU_ENABLE_SEQUENCE`] (6 bước, in marker **trước và sau từng bước** — R5
/// của thiết kế: sai thứ tự ⇒ treo không log), rồi đo ba thứ của B3.3 và chuẩn bị bước nhảy của
/// B3.4.
///
/// Trả [`MmuOutcome`]; mọi lỗi bảng trang là `PANIC` (fail-loud), không im lặng bỏ trang.
fn mmu_bringup(console: &mut Console) -> MmuOutcome {
    let text_start = core::ptr::addr_of!(__text_start) as u64;
    let kernel_end = core::ptr::addr_of!(__kernel_end) as u64;
    let stack_top = core::ptr::addr_of!(__stack_top) as u64;
    let probe_pa = core::ptr::addr_of!(PROBE_PAGE.0) as u64;
    let phys_probe_pa = core::ptr::addr_of!(PHYS_PROBE.0) as u64;

    // Marker + magic ghi **trước** khi bật MMU (PA == VA ở thời điểm này).
    MMU_MARKER.store(MMU_MARKER_VALUE, Ordering::Relaxed);
    PROBE_PAGE.0[0].store(PROBE_MAGIC, Ordering::Relaxed);
    PHYS_PROBE.0[0].store(PHYSMAP_MAGIC, Ordering::Relaxed);
    VSPACE_PROBE.0[0].store(VSPACE_MAGIC, Ordering::Relaxed);
    // PA (không phải alias nửa cao) của các vùng ảnh — đọc lúc PC còn ở nửa thấp.
    TEXT_START_PA.store(text_start, Ordering::Relaxed);
    KERNEL_END_PA.store(kernel_end, Ordering::Relaxed);
    VECTOR_BASE_PA.store(vector_base(), Ordering::Relaxed);
    // **KT-259**: PA của `_start_secondary` — đọc **trước** khi nhảy nửa cao, vì sau đó `adrp`
    // cho alias nửa cao chứ không phải PA, mà PSCI `CPU_ON` cần **PA** (MMU của hart phụ còn tắt).
    smpboot::publish_entry_pa(_start_secondary as *const () as u64);
    VSPACE_PROBE_PA.store(
        core::ptr::addr_of!(VSPACE_PROBE.0) as u64,
        Ordering::Relaxed,
    );

    // ---- Hai bảng trang trên **một** arena chia sẻ (KT-253): nếu mỗi bảng có bộ đếm riêng thì
    // cả hai cấp từ frame 0 và ghi đè lên nhau.
    let (low_mem, high_mem) = bootmem::shared_boot_memory();
    let arena_base = low_mem.base_hint();
    let frames_total = low_mem.frames_total();
    let mut mmu = match SplitMmu::<bootmem::SharedBootMemory>::new(low_mem, high_mem) {
        Ok(mmu) => mmu,
        Err(error) => {
            let _ = writeln!(console, "PANIC: mmu: tạo bảng trang lỗi {error:?}");
            debug_halt(3)
        }
    };

    // ---- Bảng **nửa thấp** (`TTBR0_EL1`): identity map ảnh + UART + GIC + trang probe.
    // Vùng ảnh: RWX (W^X đầy đủ là việc của tầng VSpace, B3.5 — không thuộc bước 3).
    let pages_kernel = match map_identity(
        mmu.low_mut(),
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
        mmu.low_mut(),
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
        mmu.low_mut(),
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
    if let Err(error) = mmu.low_mut().map_with(
        PROBE_VA,
        probe_pa,
        Perms::KERNEL_RW,
        MemAttr::Normal,
        vifast_plat_aarch64::format::Shareability::Inner,
    ) {
        let _ = writeln!(console, "PANIC: mmu: map probe lỗi {error:?}");
        debug_halt(3)
    }

    // ---- Bảng **nửa cao** (`TTBR1_EL1`): alias ảnh + physmap (KT-253).
    let pages_high = match mmu.map_high_alias(
        text_start,
        kernel_end,
        KERNEL_HIGH_BASE,
        Perms::KERNEL_RWX,
        MemAttr::Normal,
    ) {
        Ok(pages) => pages,
        Err(error) => {
            let _ = writeln!(console, "PANIC: mmu: map alias nửa cao lỗi {error:?}");
            debug_halt(3)
        }
    };
    let pages_physmap =
        match mmu.map_physmap(text_start, kernel_end, Perms::KERNEL_RW, MemAttr::Normal) {
            Ok(pages) => pages,
            Err(error) => {
                let _ = writeln!(console, "PANIC: mmu: map physmap lỗi {error:?}");
                debug_halt(3)
            }
        };

    let ttbr0 = match mmu.ttbr0_base() {
        Some(pa) => pa,
        None => {
            let _ = writeln!(console, "PANIC: mmu: không có bảng mức 1 cho TTBR0");
            debug_halt(3)
        }
    };
    let ttbr1 = match mmu.ttbr1_base() {
        Some(pa) => pa,
        None => {
            let _ = writeln!(console, "PANIC: mmu: không có bảng mức 1 cho TTBR1");
            debug_halt(3)
        }
    };
    let tcr = TCR_EL1_KERNEL.bits();
    let mair = MAIR_EL1_KERNEL.bits();
    let sctlr = sctlr_for_mmu_on(asm::read_sctlr_el1());
    let _ = writeln!(
        console,
        "# mmu: arena=0x{arena_base:x} frames={frames_total} used={} root_low=0x{:x} root_high=0x{:x} ttbr0=0x{ttbr0:x} ttbr1=0x{ttbr1:x} level=1",
        mmu.low().memory().frames_used(),
        mmu.root_low(),
        mmu.root_high(),
    );
    let _ = writeln!(
        console,
        "# mmu: map kernel=0x{text_start:x}-0x{kernel_end:x} pages={pages_kernel} uart={pages_uart} gic={pages_gic} probe_va=0x{PROBE_VA:x} probe_pa=0x{probe_pa:x}",
    );
    let _ = writeln!(
        console,
        "# mmu: high alias=0x{text_start:x}-0x{kernel_end:x} base=0x{KERNEL_HIGH_BASE:x} pages={pages_high} physmap_base=0x{PHYSMAP_BASE:x} pages_physmap={pages_physmap}",
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
        apply_step(*step, tcr, mair, ttbr0, ttbr1, sctlr);
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

    // ---- KT-253: chuẩn bị bước nhảy nửa cao. Mọi phép đo **phần mềm** (walk trên bảng nửa
    // cao) làm ở đây vì `SplitMmu` nằm trên stack **nửa thấp**; sau khi nhảy, `high_entry` chỉ
    // cần so PC thật với giá trị đã ghi.
    let kmain_pa = kmain as *const () as usize as u64;
    let kmain_va = high_alias_va(kmain_pa, text_start, KERNEL_HIGH_BASE);
    let kmain_translated = match mmu.translate(kmain_va) {
        Ok(t) => t.pa,
        Err(error) => {
            let _ = writeln!(console, "PANIC: mmu: translate kmain nửa cao lỗi {error:?}");
            debug_halt(3)
        }
    };
    let high_entry_pa = high_entry as *const () as usize as u64;
    let high_entry_va = high_alias_va(high_entry_pa, text_start, KERNEL_HIGH_BASE);
    let high_translated = match mmu.translate(high_entry_va) {
        Ok(t) => t.pa,
        Err(error) => {
            let _ = writeln!(console, "PANIC: mmu: translate high_entry lỗi {error:?}");
            debug_halt(3)
        }
    };
    let high_sp = high_alias_va(stack_top, text_start, KERNEL_HIGH_BASE);
    let physmap_probe_va = vifast_plat_aarch64::split_mmu::physmap_va(phys_probe_pa);

    ARENA_PA.store(arena_base, Ordering::Relaxed);
    KMAIN_HIGH_VA.store(kmain_va, Ordering::Relaxed);
    KMAIN_PA.store(kmain_pa, Ordering::Relaxed);
    KMAIN_TRANSLATED_PA.store(kmain_translated, Ordering::Relaxed);
    HIGH_EXPECT_VA.store(high_entry_va, Ordering::Relaxed);
    HIGH_EXPECT_PA.store(high_entry_pa, Ordering::Relaxed);
    HIGH_TRANSLATED_PA.store(high_translated, Ordering::Relaxed);
    PHYSMAP_PROBE_VA.store(physmap_probe_va, Ordering::Relaxed);
    PHYSMAP_PROBE_PA.store(phys_probe_pa, Ordering::Relaxed);

    let _ = writeln!(
        console,
        "# high: kmain va=0x{kmain_va:x} pa=0x{kmain_pa:x} match={} translated=0x{kmain_translated:x}",
        (kmain_translated == (kmain_pa & !0xFFF)) as u8,
    );
    let _ = writeln!(
        console,
        "# high: entry va=0x{high_entry_va:x} pa=0x{high_entry_pa:x} translated=0x{high_translated:x} sp=0x{high_sp:x}",
    );
    let _ = writeln!(
        console,
        "# physmap: probe pa=0x{phys_probe_pa:x} va=0x{physmap_probe_va:x} magic=0x{PHYSMAP_MAGIC:016x}",
    );

    // Ba phép đo trên là **điều kiện** của B3.3, không phải thông tin tham khảo: `kmain` phát
    // TAP từ chúng và dừng máy nếu bất kỳ phép nào sai.
    MmuOutcome {
        on,
        marker_ok: marker == MMU_MARKER_VALUE,
        probe_ok: probe == PROBE_MAGIC,
        high_entry_va,
        high_sp,
    }
}
