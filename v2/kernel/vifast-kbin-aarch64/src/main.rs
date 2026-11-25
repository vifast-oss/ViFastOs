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
//!        ├─ KT-273/275/277/279/289 (ở `el0_done`, sau U-mode): PCI ECAM + virtio-blk (TAP 19..21) →
//!        │     virtio-net + DHCP/ARP/ICMP (TAP 22..25) → TCP client + DNS (TAP 33..35) →
//!        │     W^X + cache DMA (TAP 26/27) → virtio-gpu 2D + console trên framebuffer (TAP 28..30)
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

use vifast_plat_aarch64::{debug_halt, pl011};

#[cfg(feature = "ktest")]
use crate::bootmmu::mmu_bringup;
#[cfg(feature = "ktest")]
use crate::trap::{BRK_COUNT, BRK_ELR, BRK_ESR, BRK_FAR, BRK_SLOT, BRK_SPSR, OTHER_TRAPS};
/// **KT-338c** (D2-08): ảnh sản phẩm **không** kéo mô-đun/crate nào của harness — mọi import
/// dưới đây chỉ tồn tại ở ảnh kiểm, nên chuỗi/mã kiểm không có **gốc tham chiếu** trong ảnh
/// sản phẩm (bị loại ở MIR, không phải "tắt lúc chạy").
#[cfg(feature = "ktest")]
use core::sync::atomic::Ordering;
#[cfg(feature = "ktest")]
use vifast_plat_aarch64::sysreg::{Esr, ExceptionClass};
#[cfg(feature = "ktest")]
use vifast_plat_aarch64::vector::{self, VectorKind};
#[cfg(feature = "ktest")]
use vifast_plat_aarch64::{asm, smp};

/// **KT-259**: trạng thái SMP + `smp_bringup` + `kmain_secondary` nằm ở tệp riêng (luật R1:
/// tệp trong `kernel/` ≤ 1.500 dòng). Tên `smpboot` để **không** đụng `vifast_plat_aarch64::smp`.
#[cfg(feature = "ktest")]
mod smpboot;

/// **KT-262**: bản ghi fault `FaultKind::Vm` **theo hart** — TAP 9 (hart 0) và TAP 13 (hart 1)
/// phải phân biệt được **hart nào** đã fault, nên không dùng một ô toàn cục.
#[cfg(feature = "ktest")]
mod faults;

/// **KT-269** (B3.7, D2-206): U-mode — stub EL0 (`.el0`), handler `svc`, `eret` xuống EL0 và
/// đường `Exit` quay về EL1h. Tách tệp riêng (luật R1: tệp trong `kernel/` ≤ 1.500 dòng).
#[cfg(feature = "ktest")]
mod umode;

/// **KT-273** (K4' bước 3): PCI ECAM + virtio-blk — TAP 19/20/21, chạy ở `el0_done` (sau U-mode)
/// để số hiệu TAP tuần tự. Tách tệp riêng (luật R1).
#[cfg(feature = "ktest")]
mod blk;

/// **KT-275** (K4' bước 3, D1 trên aarch64): virtio-net + DHCP + ARP + ICMP — TAP 22/23/24/25,
/// chạy ở `el0_done` **sau** `blk` để số hiệu TAP tuần tự. Tách tệp riêng (luật R1).
#[cfg(feature = "ktest")]
mod net;
/// **KT-418** — cứng hóa `PAN` + ca âm data abort (TAP 50) — chỉ ảnh kiểm (D2-08).
#[cfg(feature = "ktest")]
mod pan_probe;
/// **KT-277** (K4' bước 3): **W^X toàn ảnh** + **cache maintenance trước DMA** — TAP 22/23, chạy
/// ở `el0_done` sau `blk` (số hiệu TAP tuần tự). Tách tệp riêng (luật R1).
#[cfg(feature = "ktest")]
mod wxprobe;

/// **KT-279** (K4' bước 3, D1 phần 4 trên aarch64): **virtio-gpu 2D + console trên framebuffer**
/// — TAP 28/29/30, chạy ở `el0_done` **sau** `wxprobe` để số hiệu TAP tuần tự. Tách tệp riêng
/// (luật R1).
#[cfg(feature = "ktest")]
mod gpu;

/// **KT-283** (K4' bước 3, D1 phần 3 trên aarch64): **virtio-input (bàn phím)** — TAP 31/32, chạy
/// ở `el0_done` **sau** `gpu` để số hiệu TAP tuần tự. Tách tệp riêng (luật R1).
#[cfg(feature = "ktest")]
mod input;
/// Hai hàm map cửa sổ vào `VSpace` (`map_window_into`/`map_identity_into`) — tách khỏi `main.rs`
/// để giữ trần R1 (1.500 dòng/tệp); dùng chung cho ECAM/BAR/framebuffer/UART/GIC.
#[cfg(any(feature = "ktest", feature = "product-shell"))]
#[cfg_attr(not(feature = "ktest"), allow(dead_code))]
mod mapwin;

/// **KT-338a** — vùng RAM boot ngoài ảnh + mapping identity (tách khỏi `main.rs` để giữ R1).
/// **KT-339a**: ảnh `product-shell` cũng dùng (arena ngoài ảnh là điều kiện của trần 256 KiB).
#[cfg(any(feature = "ktest", feature = "product-shell"))]
#[cfg_attr(not(feature = "ktest"), allow(dead_code))]
mod bootram;

/// **KT-338c** — miền **boot/MMU**: trạng thái đo + `mmu_bringup` (tách khỏi `main.rs` theo miền).
#[cfg(feature = "ktest")]
mod bootmmu;

/// **KT-338c** — miền **nửa cao + VSpace + SMP + U-mode** (`high_entry`).
#[cfg(feature = "ktest")]
mod highentry;

/// **KT-338c** — miền **trap**: handler 16 ô vector (`brk`, IRQ GICv3, fault được chờ).
#[cfg(feature = "ktest")]
mod trap;

/// **KT-338b** — rig `vifast-core::Machine` + chương trình EL0 thứ hai (IPC từ U-mode). Tách tệp
/// riêng để giữ R1 (≤ 1.500 dòng/tệp) và để `umode.rs` chỉ còn "cổng vào U-mode".
#[cfg(feature = "ktest")]
mod coretest;

/// **KT-339** — K3.5 trên aarch64: nạp ELF `procsrv`/`shell`/`echo` (cùng nguồn với riscv64),
/// chạy ở EL0 qua `svc #0` — TAP 42/43/44. Chỉ ảnh **kiểm** (`.data+.bss` sản phẩm 65.536 B <
/// `size_of::<Machine>()`; quyết định ngân sách ở `JOURNAL/kt339-user.md`).
#[cfg(feature = "ktest")]
mod k5user;

/// **KT-412** (`D2-259` mục 6) — bộ ca **SMP dùng chung** (`vifast_ktest::smp_suite`, TAP 47..49)
/// trên `Machine` thật của ảnh: `set_harts(n)` từ `BootInfo`, 4 hart preemptive, luật đọc
/// `published` §4.16. Cùng **một** hàm mà ảnh `riscv64` gọi. Chỉ ảnh kiểm (D2-08).
#[cfg(feature = "ktest")]
mod smpcore;

/// **KT-339a** — shell **sản phẩm dùng chung** trên aarch64: `Machine` trong `.corespace`
/// NOLOAD + loader ELF + rig `svc #0` + PL011; ảnh kiểm chạy **cùng** mã để phát TAP 45/46.
#[cfg(any(feature = "ktest", feature = "product-shell"))]
mod pshell;

/// **KT-305**: tự kiểm `translate`/`unmap` trên **block descriptor** của ECAM (bảng trang thật)
/// + in `# pci: unmap … blocks_split=1`. Tách tệp riêng để giữ trần R1.
#[cfg(feature = "ktest")]
mod ecam;

/// **KT-338c** (D2-08) — nhánh **ảnh sản phẩm**: `kmain` rẽ vào `product::boot` **trước** dòng
/// TAP đầu tiên. Luôn được biên dịch (cả hai ảnh): ảnh sản phẩm cần `product::boot` + handler
/// vector tối thiểu; ảnh kiểm dùng hằng [`product::KTEST`].
mod product;

/// `MAX_HARTS` của crate arch phải **trùng** hằng dùng chung (D2-195 Q5) — lệch là lỗi biên dịch.
#[cfg(feature = "ktest")]
const _: () = assert!(smp::MAX_HARTS == vifast_plat_common::smp::MAX_HARTS);

/// Dòng marker **duy nhất** của ảnh — runner (`v2/test/boot/run-aarch64.sh`, KT-250) so **nguyên
/// văn** dòng này và đòi nó xuất hiện **đúng một lần**.
pub(crate) const BOOT_MARKER: &str = "# viFastOs aarch64: boot ok\n";

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
    // `CPACR_EL1` — FP/SIMD cho **mọi** hàm Rust (kể cả ảnh sản phẩm: `core::fmt` phát NEON).
    cpacr = const vifast_plat_aarch64::sysreg::CPACR_EL1_FP_SIMD,
);

#[cfg(feature = "ktest")]
global_asm!(
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
    // **KT-262**: giữ `x9` **gốc** trước khi dùng nó làm chỉ số ô. KT-248 dùng thẳng
    // `mov x9, #idx` ⇒ `x9` của mã bị ngắt bị **mất**, và sau `eret` nó bằng chỉ số ô (4 với
    // `brk`/data abort ở EL1h). Đó là lỗi **thật**, chỉ lộ ra khi mã sau điểm fault còn giữ `x9`
    // là con trỏ sống (KT-262: `faults::get` đọc mảng theo hart ⇒ `ldar x9, [x9]` với `x9 = 4`
    // ⇒ data abort `FAR=0x4`, log `kt262-red.log`). Khung 16 B giữ `sp` căn 16.
    "str x9, [sp, #-16]!",
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
    // **KT-269**: `x5` = con trỏ khung trap (sau `sub sp, sp, #176`) — handler U-mode đọc
    // `x8` (số hiệu syscall) và ghi `x0`/`x1` (error/value) qua con trỏ này. `x5` đã được cất
    // ở `[sp, #32]` nên dùng lại thanh ghi là an toàn.
    "mov x5, sp",
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
    // Khôi phục `x9` **gốc** (đã cất ở khung ngoài của stub) — xem `VIFAST_VEC`.
    "ldr x9, [sp], #16",
    "eret",
    "vec_halt:",
    "wfi",
    "b vec_halt",
    stride = const vifast_plat_aarch64::vector::VECTOR_STRIDE,
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
    // **KT-277** — ranh giới W^X: `[__text_start, __data_start)` là vùng **RX**,
    // `[__data_start, __kernel_end)` là vùng **RW** (xem `linker.ld`).
    static __data_start: u8;
    // Đỉnh stack (cuối `.stack` NOLOAD) — KT-253 dùng để tính SP nửa cao.
    static __stack_top: u8;
    // **KT-259**: arena stack per-hart (NOLOAD) — `_start_secondary` và `HartStacks` dùng.
    static __smp_stacks_start: u8;
    static __smp_stacks_end: u8;
    /// Điểm vào của hart phụ — chỉ lấy **địa chỉ** (PA) để truyền cho PSCI `CPU_ON`.
    #[cfg(feature = "ktest")]
    fn _start_secondary();
}

/// Địa chỉ bảng vector của ảnh này (VA **thấp** — nơi linker đặt nó).
pub(crate) fn vector_base() -> u64 {
    core::ptr::addr_of!(__vector_base) as u64
}

// **KT-262**: bản ghi fault `FaultKind::Vm` **theo hart** nằm ở `crate::faults` (mảng
// `[AtomicU64; MAX_HARTS]` + mặt nạ "đang chờ"/"đã thấy"). KT-254 dùng một ô toàn cục vì chỉ
// hart 0 sinh fault; KT-262 đo fault trên **hart 1** nên một ô sẽ ghi đè lẫn nhau và TAP 9/TAP 13
// không còn phân biệt được hart nào đã fault.

// ---------------------------------------------------------------------------------------------
// KT-250 — TAP trên serial
// ---------------------------------------------------------------------------------------------

/// Số ca kiểm TAP mà ảnh phát ra (khớp `1..N` và `EXPECT_TESTS` của runner).
///
/// 5 ca của KT-249 + 2 ca của KT-253 (nửa cao, physmap) + 2 ca của KT-254 (`VSpace` map/walk/
/// activate, và fault `FaultKind::Vm` sau `unmap`) + 3 ca của KT-259 (hart 1 online, IPI 0→1,
/// mọi hart báo alive) + 3 ca của KT-262 (`tlbi: hart 1 sees unmap`, `tlbi: ack from all harts`,
/// `smp: 4 harts online`) + **3 ca của KT-269** (EL0 vào được, `svc` được xử lý, EL0 chạy tiếp
/// sau `svc` rồi `Exit`) + **3 ca của KT-273** (`pci: ecam`, `pci: virtio-blk found`,
/// `blk: sector0`) + **4 ca của KT-275** (`net: device`, `net: dhcp`, `net: arp`, `net: ping`)
/// + **2 ca của KT-277** (`wx: text RX (write faults), data RW`,
/// `dma: cache maintenance before/after DMA`) + **3 ca của KT-279** (`gpu: device`, `gpu: draw`,
/// `gpu: console`) + **2 ca của KT-283** (`input: device`, `input: key`) + **3 ca của KT-289**
/// (`net: tcp connect`, `net: tcp data`, `net: dns`).
#[cfg(feature = "ktest")]
/// Số TAP của ảnh **kiểm** — phải khớp `EXPECT_TESTS` của `run-aarch64.sh` và số dòng `ok` thật.
///
/// **KT-412** (`D2-259` mục 6): 46 → **49** (thêm TAP 47..49 của bộ ca SMP dùng chung
/// `vifast_ktest::smp_suite`).
pub const TAP_TESTS: usize = 50;

/// In một dòng TAP `ok N - <tên>` / `not ok N - <tên>`; trả `ok`.
///
/// TAP 13 (Test Anything Protocol): dòng `ok`/`not ok` phải bắt đầu dòng, số hiệu liên tục, và
/// `#` là comment. Runner (`v2/test/boot/run-aarch64.sh`) đếm **đúng** `N` dòng `ok`, đòi
/// `1..N`, và coi mọi `not ok`/`PANIC` là đỏ.
#[cfg(feature = "ktest")]
pub(crate) fn tap(console: &mut Console, n: usize, name: &str, ok: bool) -> bool {
    let _ = writeln!(console, "{} {n} - {name}", if ok { "ok" } else { "not ok" });
    ok
}

// ---------------------------------------------------------------------------------------------
// Console tối giản trên PL011
// ---------------------------------------------------------------------------------------------

/// `core::fmt::Write` trên [`pl011`] — để `panic!`/`write!` in được số mà không cần `alloc`.
pub(crate) struct Console;

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

/// **KT-346** (D2-252) — danh tính bản dựng của **ảnh sản phẩm**, in ngay sau `# conf image:`
/// để đóng nợ P20: trước KT-346 chỉ `riscv64` có dòng này, nên `repro-build.sh`/`repro-check.sh`
/// không chạy được cho hai arch còn lại. Giá trị do `build.rs` đặt qua `cargo:rustc-env` (tính
/// **tại thời điểm build** từ `v2/test/boot/build-rev.sh`); `inputs` là dấu vân tay **nội dung**
/// build-inputs (không phải sha256 ảnh — sha256 ảnh phụ thuộc thư mục build, xem §6.3 AGENTS).
pub(crate) const BUILD_LINE: &str = concat!(
    "# build: algo=",
    env!("VIFAST_BUILD_ALGO"),
    " inputs=",
    env!("VIFAST_BUILD_INPUTS"),
    " dirty=",
    env!("VIFAST_BUILD_DIRTY"),
    "\n"
);

// ---------------------------------------------------------------------------------------------
// kmain
// ---------------------------------------------------------------------------------------------

/// Điểm vào Rust, được `_start` gọi sau khi đã có stack và `.bss` sạch.
///
/// `dtb` = địa chỉ DTB QEMU truyền trong `x0` (giữ trong chữ ký để KT-250 in magic DTB làm bằng
/// chứng "địa chỉ nền tảng là thật"; B3.1/B3.2 chưa đọc DTB).
#[no_mangle]
pub extern "C" fn kmain(dtb: u64) -> ! {
    // **KT-402** (D2-259) — parse DTB **thật** ngay đầu boot (paging tắt, `dtb` là PA). Đọc trước
    // mọi thứ khác để bảng hỏng ⇒ fail-closed có tên trước khi bật MMU/U-mode.
    let bootinfo = vifast_plat_aarch64::bootinfo::detect(dtb);
    let mut console = Console;
    // **KT-338c** (D2-08): ảnh **sản phẩm** (không feature `ktest`) rẽ vào `product::boot` —
    // nó trả `!`, nên **toàn bộ** harness phía dưới là unreachable và bị loại khỏi ảnh. Đây
    // không phải lời hứa: runner quét `strings` **bằng máy** + có ca âm (ảnh ktest phải đếm > 0).
    if !product::KTEST {
        if !vifast_plat_aarch64::bootinfo::write_log(&bootinfo, &mut console) {
            debug_halt(9)
        }
        product::boot(dtb, &mut console);
    }

    // **KT-338c** (D2-08): thân kiểm bị `cfg` gỡ khỏi ảnh sản phẩm **trước** name-resolution.
    #[cfg(feature = "ktest")]
    {
        // TAP 13: dòng phiên bản phải là dòng **đầu tiên**; plan `1..N` ngay sau.
        let _ = writeln!(console, "TAP version 13");
        let _ = writeln!(console, "1..{TAP_TESTS}");
        // **KT-402** — `# bootinfo:` (DTB thật); bảng hỏng ⇒ halt có tên, runner đỏ đúng.
        if !vifast_plat_aarch64::bootinfo::write_log(&bootinfo, &mut console) {
            debug_halt(9)
        }

        // ---- **KT-265 / D2-80** — bằng chứng neo **ảnh**: `# conf image: build=<16 hex> text=<len>`
        // (FNV-1a 64 trên `.text` **đã nạp**, khuôn riscv64). `soak-aarch64.sh` băm lại **cùng**
        // section từ tệp ELF (`test/conformance/image_digest.py`) và **từ chối** log không có/lệch
        // hash. In **trước** mọi ca kiểm: log của một boot hỏng vẫn phải trả lời được "ảnh nào".
        vifast_plat_aarch64::image::report_image_digest();

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
    // Chỉ tới được khi **không** biên dịch harness: `product::boot` đã trả `!`.
    #[cfg(not(feature = "ktest"))]
    unreachable!()
}
