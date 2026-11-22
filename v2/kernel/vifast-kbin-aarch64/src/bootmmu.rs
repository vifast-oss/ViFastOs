// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-338c** — miền **boot/MMU**: dựng hai bảng trang và bật MMU (tách khỏi `main.rs` để giữ R1).
//!
//! Tệp này gồm **hai** phần gắn chặt với nhau:
//!
//! 1. **trạng thái đo** của boot — các trang probe trong `.bss` (probe MMU, physmap, VSpace) và
//!    các ô `AtomicU64` công bố PA/VA **đọc lúc PC còn ở nửa thấp** (sau khi nhảy nửa cao, `adrp`
//!    trả alias nên không đọc lại được PA của symbol);
//! 2. **[`mmu_bringup`]** — `SplitMmu` trên **một** arena chia sẻ: bảng **nửa thấp** (identity ảnh
//!    W^X + UART/GIC + RAM boot + trang probe) và bảng **nửa cao** (alias ảnh + physmap), rồi bật
//!    MMU đúng [`MMU_ENABLE_SEQUENCE`] sáu bước, in marker **trước và sau từng bước** (R5: sai
//!    thứ tự ⇒ treo không log).
//!
//! Mọi lỗi bảng trang là `PANIC` (fail-loud), không im lặng bỏ trang.

use core::fmt::Write;
use core::sync::atomic::{AtomicU64, Ordering};

use vifast_plat_aarch64::boot_mmu::{
    apply_step, map_identity, mmu_is_on, sctlr_for_mmu_on, MMU_ENABLE_SEQUENCE,
};
use vifast_plat_aarch64::bootmem;
use vifast_plat_aarch64::format::{MemAttr, Perms};
use vifast_plat_aarch64::resources::{GIC, GIC_CPU, UART0};
use vifast_plat_aarch64::split_mmu::{high_alias_va, SplitMmu, KERNEL_HIGH_BASE, PHYSMAP_BASE};
use vifast_plat_aarch64::sysreg::{MAIR_EL1_KERNEL, TCR_EL1_KERNEL};
use vifast_plat_aarch64::vspace::VSpaceId;
use vifast_plat_aarch64::{asm, debug_halt};

use crate::highentry::high_entry;
use crate::kmain;
use crate::{bootram, smpboot, vector_base, wxprobe, Console};

extern "C" {
    /// Biên vùng ảnh do `linker.ld` cấp (đọc **PA** khi PC còn ở nửa thấp).
    static __text_start: u8;
    static __kernel_end: u8;
    static __data_start: u8;
    static __stack_top: u8;
    static __smp_stacks_start: u8;
    static __smp_stacks_end: u8;
    /// Điểm vào của hart phụ — chỉ lấy **địa chỉ** (PA) để truyền cho PSCI `CPU_ON`.
    fn _start_secondary();
}
// ---------------------------------------------------------------------------------------------
// KT-249 — trạng thái đo của MMU
// ---------------------------------------------------------------------------------------------

/// Marker ghi **trước** khi bật MMU, đọc lại **sau** khi bật: cùng một địa chỉ ảo (identity) nên
/// giá trị phải không đổi. Đây là phép đo "MMU bật mà bản đồ đúng" của B3.3.
pub(crate) static MMU_MARKER: AtomicU64 = AtomicU64::new(0);
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
pub(crate) const PHYSMAP_MAGIC: u64 = 0x5048_5953_4D41_5001;

/// VA nửa cao mong đợi của [`high_entry`] (tính **trước** khi nhảy, đọc lại sau khi nhảy).
pub(crate) static HIGH_EXPECT_VA: AtomicU64 = AtomicU64::new(0);
/// PA mong đợi của [`high_entry`].
pub(crate) static HIGH_EXPECT_PA: AtomicU64 = AtomicU64::new(0);
/// Kết quả `translate(HIGH_EXPECT_VA)` bằng **walk phần mềm** trên bảng nửa cao (trước khi nhảy).
pub(crate) static HIGH_TRANSLATED_PA: AtomicU64 = AtomicU64::new(u64::MAX);
/// VA nửa cao của `kmain` (in trước khi nhảy — "VA nửa cao của kmain" theo yêu cầu B3.4).
pub(crate) static KMAIN_HIGH_VA: AtomicU64 = AtomicU64::new(0);
/// PA của `kmain`.
pub(crate) static KMAIN_PA: AtomicU64 = AtomicU64::new(0);
/// Kết quả `translate(KMAIN_HIGH_VA)`.
pub(crate) static KMAIN_TRANSLATED_PA: AtomicU64 = AtomicU64::new(u64::MAX);
/// VA physmap của trang [`PHYS_PROBE`] (`PHYSMAP_BASE + PA`).
pub(crate) static PHYSMAP_PROBE_VA: AtomicU64 = AtomicU64::new(0);
/// PA của trang [`PHYS_PROBE`].
pub(crate) static PHYSMAP_PROBE_PA: AtomicU64 = AtomicU64::new(0);
/// Số lần [`high_entry`] đã chạy (phải đúng **1**).
pub(crate) static HIGH_ENTRY_COUNT: AtomicU64 = AtomicU64::new(0);
/// PA của `__text_start` (địa chỉ **liên kết**, đọc lúc PC còn ở nửa thấp).
///
/// **Vì sao phải lưu**: sau khi nhảy sang nửa cao, `adrp` là **PC-relative** nên
/// `addr_of!(__text_start)` trả **alias nửa cao** của symbol, không phải PA liên kết. Giá trị
/// PA thật phải đọc **trước** khi nhảy rồi truyền qua static (static được đọc qua `adrp` ⇒ đi
/// qua `TTBR1`, đúng bản đồ nửa cao).
pub(crate) static TEXT_START_PA: AtomicU64 = AtomicU64::new(0);
/// PA của `__vector_base` (địa chỉ liên kết, đọc lúc PC còn ở nửa thấp).
pub(crate) static VECTOR_BASE_PA: AtomicU64 = AtomicU64::new(0);
/// PA của `__kernel_end` (đọc lúc PC còn ở nửa thấp — cùng lý do `TEXT_START_PA`).
pub(crate) static KERNEL_END_PA: AtomicU64 = AtomicU64::new(0);

// ---------------------------------------------------------------------------------------------
// KT-254 — VSpace + fault `FaultKind::Vm`
// ---------------------------------------------------------------------------------------------

/// ASID của VSpace dựng trong ảnh (nhãn `FaultKind::Vm.vspace`).
pub(crate) const VSPACE_ID: VSpaceId = 1;
/// VA **người dùng** của trang probe VSpace — ngoài RAM 128 MiB, **không** identity.
///
/// `pub(crate)` từ KT-262: `smpboot` dùng **cùng** VA này cho phép đo shootdown (hart 1 đọc nó
/// để nạp TLB, rồi đọc lại sau `unmap` để sinh fault).
pub(crate) const VSPACE_VA: u64 = 0x6000_0000;
/// Giá trị ghi vào trang probe VSpace **trước** khi `activate` (đọc lại qua VA của VSpace).
pub(crate) const VSPACE_MAGIC: u64 = 0x5653_5041_4345_0001;

/// Trang **VSpace probe**: ghi magic qua VA thấp (identity) trước khi `activate`, đọc lại qua
/// **VA người dùng** của VSpace sau `activate` — chứng minh `TTBR0` đã đổi sang bảng của VSpace.
#[repr(align(4096))]
struct VSpaceProbePage([AtomicU64; 512]);

/// Trang VSpace probe — `.bss`, nằm trong khoảng ảnh nên được map cả identity (bảng kernel) lẫn
/// alias nửa cao (bảng của VSpace).
static VSPACE_PROBE: VSpaceProbePage = VSpaceProbePage([const { AtomicU64::new(0) }; 512]);

/// PA của trang [`VSPACE_PROBE`] (đọc lúc PC còn ở nửa thấp).
pub(crate) static VSPACE_PROBE_PA: AtomicU64 = AtomicU64::new(0);
/// Kết quả các phép đo của B3.3 + thông tin nhảy nửa cao của B3.4 — `mmu_bringup` trả về để
/// `kmain` phát TAP và thực hiện bước nhảy.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct MmuOutcome {
    /// `SCTLR_EL1.M == 1` sau `ISB`.
    pub(crate) on: bool,
    /// Marker ghi trước khi bật, đọc lại qua VA sau khi bật — không đổi.
    pub(crate) marker_ok: bool,
    /// Đọc trang probe qua **VA không identity** ra đúng magic.
    pub(crate) probe_ok: bool,
    /// VA nửa cao của [`high_entry`] (đích của bước nhảy).
    pub(crate) high_entry_va: u64,
    /// SP nửa cao (đỉnh stack alias sang nửa cao).
    pub(crate) high_sp: u64,
}

/// **KT-249 + KT-253** — dựng **hai** bảng trang (nửa thấp identity + nửa cao alias/physmap),
/// bật MMU theo đúng [`MMU_ENABLE_SEQUENCE`] (6 bước, in marker **trước và sau từng bước** — R5
/// của thiết kế: sai thứ tự ⇒ treo không log), rồi đo ba thứ của B3.3 và chuẩn bị bước nhảy của
/// B3.4.
///
/// Trả [`MmuOutcome`]; mọi lỗi bảng trang là `PANIC` (fail-loud), không im lặng bỏ trang.
pub(crate) fn mmu_bringup(console: &mut Console) -> MmuOutcome {
    let text_start = core::ptr::addr_of!(__text_start) as u64;
    let kernel_end = core::ptr::addr_of!(__kernel_end) as u64;
    let stack_top = core::ptr::addr_of!(__stack_top) as u64;
    // **KT-277** — ranh giới W^X (đọc lúc PC còn ở nửa thấp ⇒ đây là **PA** liên kết).
    let data_start = core::ptr::addr_of!(__data_start) as u64;
    let stacks_start = core::ptr::addr_of!(__smp_stacks_start) as u64;
    let stacks_end = core::ptr::addr_of!(__smp_stacks_end) as u64;
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
    // **KT-277**: công bố bố cục W^X cho `wxprobe` (sau khi nhảy nửa cao, `adrp` trả alias nên
    // không đọc lại được PA của symbol).
    wxprobe::publish_layout(text_start, data_start, kernel_end, stacks_start, stacks_end);
    // **KT-259**: PA của `_start_secondary` — đọc **trước** khi nhảy nửa cao, vì sau đó `adrp`
    // cho alias nửa cao chứ không phải PA, mà PSCI `CPU_ON` cần **PA** (MMU của hart phụ còn tắt).
    smpboot::publish_entry_pa(_start_secondary as *const () as u64);
    VSPACE_PROBE_PA.store(
        core::ptr::addr_of!(VSPACE_PROBE.0) as u64,
        Ordering::Relaxed,
    );

    // ---- **KT-338a**: arena + frame DMA ở RAM thật ngay sau ảnh, không còn `static` trong
    // `.bss` (D2-236); bố cục/bất biến "phải sạch" ở `bootram::prepare`.
    let ram_start = bootram::prepare(kernel_end);
    let ram_end = ram_start + bootmem::BOOT_RAM_BYTES as u64;

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
    // **KT-277**: vùng ảnh map **W^X** — `[__text_start, __data_start)` RX (`.text` +
    // `.text.vectors` + `.el0` + `.rodata`), `[__data_start, __kernel_end)` RW (`.data` + `.bss`
    // + stack), trang guard của stack per-hart **không map** (`wx::map_low`).
    let layout = wxprobe::layout();
    let pages_kernel = match vifast_plat_aarch64::wx::map_low(mmu.low_mut(), &layout) {
        Ok(pages) => pages,
        Err(error) => {
            let _ = writeln!(console, "PANIC: mmu: map kernel lỗi {error:?}");
            debug_halt(3)
        }
    };
    // KT-338a: identity-map vùng RAM boot vào bảng boot (mọi lần ghi entry bảng trang đi qua nó).
    let pages_boot_ram = bootram::map_boot_table(&mut mmu, console, ram_start);
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

    // ---- Bảng **nửa cao** (`TTBR1_EL1`): alias ảnh + physmap (KT-253), **W^X** từ KT-277.
    let pages_high = match vifast_plat_aarch64::wx::map_high(&mut mmu, &layout) {
        Ok(pages) => pages,
        Err(error) => {
            let _ = writeln!(console, "PANIC: mmu: map alias nửa cao lỗi {error:?}");
            debug_halt(3)
        }
    };
    let pages_physmap = match vifast_plat_aarch64::wx::map_phys(&mut mmu, &layout) {
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
        "# mmu: map kernel=0x{text_start:x}-0x{kernel_end:x} pages={pages_kernel} boot_ram=0x{ram_start:x}-0x{ram_end:x} boot_ram_pages={pages_boot_ram} uart={pages_uart} gic={pages_gic} probe_va=0x{PROBE_VA:x} probe_pa=0x{probe_pa:x}",
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
