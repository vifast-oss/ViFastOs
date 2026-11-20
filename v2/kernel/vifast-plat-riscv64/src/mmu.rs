// SPDX-License-Identifier: MIT OR Apache-2.0
//! MMU **Sv39 thật** trên QEMU `virt` — KT-46 (asm/CSR/arena; logic thuần ở
//! `vifast-plat-common::mmu`).
//!
//! # Bố cục ánh xạ (QEMU `virt`, `-m 128M`, RAM vật lý `0x8000_0000..0x8800_0000`)
//!
//! ```text
//!  Vùng                        VA (Sv39)                    PA                    quyền
//!  kernel identity             0x0000_0000_8020_0000 + Δ    = VA                  RWX image / RW NX stack
//!  kernel nửa cao              0xFFFF_FFC0_4000_0000 + Δ    = 0x8020_0000 + Δ     RWX image / RW NX stack
//!  physmap RAM                 0xFFFF_FFC0_0000_0000 + PA    = PA                 V|R|W|A|D, **NX**, 2 MiB
//!  UART0                       0x0000_0000_1000_0000        = UART0               V|R|W|A|D, Device 4 KiB
//!  PLIC (2 cửa sổ 2 MiB)       0x0000_0000_0c00_0000        = PLIC                V|R|W|A|D, Device 2 MiB ×2
//!                              0x0000_0000_0c20_0000        (KT-48: priority/enable | context)
//!  guard page                  0x0000_0000_8020_4000        (không map)           **NONE** (fault thật)
//! ```
//!
//! `Δ = VA - 0x8020_0000`. Kernel **liên kết thấp** (`linker.ld` `0x8020_0000`) nên nó chạy ở
//! identity mapping; nửa cao là **alias** để đọc/ghi (và để user root sau này copy entry — K3
//! chưa có user root). Guard page là **NONE** ở cả ba đường (identity, nửa cao, physmap — chunk
//! 2 MiB chứa guard được tách thành 4 KiB để khoét đúng trang), nên chạm vào là page fault thật
//! `scause=13/15` (D2-73).
//!
//! # Arena
//!
//! Root page table + mọi bảng trung gian cấp từ RAM **sau `__kernel_end`** (arena 64 KiB, bump
//! allocator trong [`PageTableBuilder`]) — **không** `static`, không tính vào trần
//! `.data+.bss ≤ 262.144 B` (D2-72 Q5). Arena nằm trong `[__kernel_end, __kernel_end+64K)` và được
//! identity-map RW NX (cùng nửa cao).
//!
//! # Bật MMU **hai bước**
//!
//! 1. Dựng **xong** toàn bộ bảng (identity + nửa cao + physmap + MMIO + guard NONE) — chưa đụng
//!    `satp`.
//! 2. `csrw satp` (mode 8) + `sfence.vma`, đọc lại `satp`, walk phần mềm lại và so PTE
//!    trước/sau; kiểm alias ghi/đọc qua nửa cao + physmap.
//!
//! KT-49: vùng NOLOAD per-hart (`MAX_HARTS` record `guard | stack | trap stack | syscall stack`,
//! xem [`vifast_plat_common::smp`]) thay cho **một** guard/stack/trap stack của KT-46. Mọi guard
//! của **từng hart** là NONE ở cả ba alias (identity, nửa cao, physmap); stack/trap stack/syscall
//! stack của mọi hart là leaf 4 KiB RW NX. Hart thứ cấp [`adopt_satp`] cùng root của BSP khi vào
//! S-mode (SBI HSM đánh thức với `satp = 0`).
//!
//! Không timer/PLIC/ASID/U-mode ở KT-46 (KT-47/48/51/53).

use core::arch::asm;
#[cfg(feature = "ktest")]
use core::fmt::Write;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use vifast_plat_common::mmu::{
    align_up, satp_asid, satp_sv39, satp_sv39_asid, walk_with, MapError, PageTableBuilder,
    PteFlags, PteMemory, Walk, PAGE_BYTES,
};
use vifast_plat_common::plic::PLIC_LOW_WINDOW_BYTES;
use vifast_plat_common::smp::{
    guard_offset as smp_guard_offset, stack_bottom_offset, stack_top_offset, syscall_bottom_offset,
    syscall_top_offset, trap_bottom_offset, trap_top_offset, HART_AREA_BYTES, HART_GUARD_BYTES,
    HART_STRIDE, MAX_HARTS,
};
#[cfg(feature = "ktest")]
use vifast_plat_common::trap::EXC_LOAD_PAGE_FAULT;
use vifast_plat_common::trap::{Exception, ExceptionKind, TrapFrame};
#[cfg(feature = "ktest")]
#[cfg(feature = "ktest")]
use vifast_plat_common::{Platform, PlatformWriter};

#[cfg(feature = "ktest")]
use crate::{instruction_len, Riscv64Virt};
use crate::{PLIC_BASE, UART0_BASE};

/// Nơi kernel được link/nạp (`linker.ld`, D2-72 Q7) — base của identity mapping.
pub const KERNEL_PHYS_BASE: u64 = 0x8020_0000;
/// Base **nửa cao** của kernel: `0xFFFF_FFC0_4000_0000` (canonical Sv39). Nằm ngoài cửa sổ
/// physmap để hai alias không đè nhau; cách `PHYSMAP_BASE` 1 GiB.
pub const KERNEL_HIGH_BASE: u64 = 0xFFFF_FFC0_4000_0000;
/// Base **physmap**: VA = `PHYSMAP_BASE + PA` (RAM `0x8000_0000..0x8800_0000`), NX.
pub const PHYSMAP_BASE: u64 = 0xFFFF_FFC0_0000_0000;
/// RAM vật lý của QEMU `virt` (`-m 128M`).
pub const RAM_PHYS_BASE: u64 = 0x8000_0000;
/// Cỡ RAM được physmap (`-m 128M`, D2-72 Q2).
pub const RAM_BYTES: u64 = 128 << 20;
/// Cỡ arena page table sau `__kernel_end` (đủ ~10 bảng 4 KiB + dư).
pub const ARENA_BYTES: usize = 64 * 1024;
/// KT-51: hình Sv39 của `PROBE_VA` mà [`remap_probe`] dùng (tính tay, không thêm dependency).
#[cfg(feature = "ktest")]
const _: () = {
    assert!(((PROBE_VA >> 30) & 0x1ff) == 2);
    assert!(((PROBE_VA >> 21) & 0x1ff) == 4);
    assert!(((PROBE_VA >> 12) & 0x1ff) == 0);
    assert!(PROBE_VA & (LARGE_2M - 1) == 0);
    assert!(PROBE_PA_A & (LARGE_2M - 1) == 0);
    assert!(PROBE_PA_B & (LARGE_2M - 1) == 0);
    assert!(PROBE_PA_A >= RAM_PHYS_BASE + (2 << 20)); // ngoài image + arena
    assert!(PROBE_PA_B + (2 << 20) <= RAM_PHYS_BASE + RAM_BYTES);
    assert!(((PROBE2_VA >> 30) & 0x1ff) == 2);
    assert!(((PROBE2_VA >> 21) & 0x1ff) == 5);
    assert!(((PROBE2_VA >> 12) & 0x1ff) == 0);
    assert!(PROBE2_VA & (LARGE_2M - 1) == 0);
    assert!(PROBE2_PA_C & (LARGE_2M - 1) == 0);
    assert!(PROBE2_PA_D & (LARGE_2M - 1) == 0);
    assert!(PROBE2_PA_D + (2 << 20) <= RAM_PHYS_BASE + RAM_BYTES);
    assert!(PROBE2_PA_C != PROBE_PA_A && PROBE2_PA_C != PROBE_PA_B);
};
/// Cỡ một leaf lớn 2 MiB (physmap).
const LARGE_2M: u64 = 1 << 21;
/// Cửa sổ PLIC thứ nhất (KT-48): priority (`0x0`) + pending (`0x1000`) + enable
/// (`0x2000 + 0x80*ctx`) — leaf 2 MiB Device phủ tới context 15 của `MAX_HARTS = 8`.
pub const PLIC_WINDOW_LOW_BASE: u64 = PLIC_BASE as u64;
/// Cửa sổ PLIC thứ hai (KT-48): threshold/claim `0x20_0000 + 0x1000*ctx` — leaf 2 MiB Device.
pub const PLIC_WINDOW_HIGH_BASE: u64 = PLIC_BASE as u64 + PLIC_LOW_WINDOW_BYTES;
/// Cỡ mỗi cửa sổ PLIC (2 MiB) — dùng cho TAP/log.
pub const PLIC_WINDOW_BYTES: u64 = PLIC_LOW_WINDOW_BYTES;
/// Số VA lấy mẫu PTE trước/sau khi bật `satp`.
pub const WALK_SAMPLES: usize = 8;
/// **KT-51 (ảnh test)**: VA 2 MiB dành riêng cho phép đo "stale mapping thật". Nằm ngoài mọi
/// mapping khác của kernel (identity chỉ tới `arena_end ≈ 0x804b_8000`), trong cùng bảng cấp 1 với
/// leaf 2 MiB của image (chỉ số 4) nên **không** cần bảng mới, không đổi budget/arena.
#[cfg(feature = "ktest")]
pub const PROBE_VA: u64 = 0x0000_0000_8080_0000;
/// PA của leaf thử nghiệm (2 MiB, vùng RAM trống `0x8080_0000..0x80a0_0000`).
#[cfg(feature = "ktest")]
pub const PROBE_PA_A: u64 = 0x8080_0000;
/// PA thứ hai để remap cùng VA (vùng RAM trống kế tiếp).
#[cfg(feature = "ktest")]
pub const PROBE_PA_B: u64 = 0x80a0_0000;
/// **Probe #2** (KT-51): VA riêng cho TAP 27/28 — hart khác **chưa** từng chạm VA này nên TLB của
/// nó sẽ nạp từ mapping hiện tại ngay trước khi remap (phép đo stale không bị nhiễu bởi entry cũ
/// của TAP 25/26).
#[cfg(feature = "ktest")]
pub const PROBE2_VA: u64 = 0x0000_0000_80a0_0000;
/// PA thứ nhất của probe #2.
#[cfg(feature = "ktest")]
pub const PROBE2_PA_C: u64 = 0x80c0_0000;
/// PA thứ hai của probe #2.
#[cfg(feature = "ktest")]
pub const PROBE2_PA_D: u64 = 0x80e0_0000;

// Linker symbols (`linker.ld`). Chỉ lấy **địa chỉ**, không đọc nội dung.
extern "C" {
    static __hart_area_base: u8;
    static __kernel_end: u8;
    static __stack_guard_start: u8;
    static __stack_guard_end: u8;
    static __stack_bottom: u8;
    static __stack_top: u8;
    static __trap_stack_bottom: u8;
    static __trap_stack_top: u8;
    static __syscall_stack_bottom: u8;
    static __syscall_stack_top: u8;
}

/// Địa chỉ của một linker symbol.
fn sym(symbol: &u8) -> u64 {
    symbol as *const u8 as u64
}

/// Bố cục runtime lấy từ linker symbol — dựng một lần trong [`init`].
#[derive(Clone, Copy, Debug)]
struct Layout {
    hart_area_base: u64,
    kernel_end: u64,
    arena_start: u64,
    arena_end: u64,
    guard_start: u64,
    guard_end: u64,
    stack_start: u64,
    stack_end: u64,
    trap_stack_start: u64,
    trap_stack_end: u64,
    syscall_stack_start: u64,
    syscall_stack_end: u64,
}

impl Layout {
    fn detect() -> Self {
        // SAFETY: chỉ lấy địa chỉ symbol linker, không dereference.
        let hart_area_base = unsafe { sym(&__hart_area_base) };
        let kernel_end = unsafe { sym(&__kernel_end) };
        let guard_start = unsafe { sym(&__stack_guard_start) };
        let guard_end = unsafe { sym(&__stack_guard_end) };
        let stack_start = unsafe { sym(&__stack_bottom) };
        let stack_end = unsafe { sym(&__stack_top) };
        let trap_stack_start = unsafe { sym(&__trap_stack_bottom) };
        let trap_stack_end = unsafe { sym(&__trap_stack_top) };
        let syscall_stack_start = unsafe { sym(&__syscall_stack_bottom) };
        let syscall_stack_end = unsafe { sym(&__syscall_stack_top) };
        let arena_start = align_up(kernel_end, PAGE_BYTES);
        Self {
            hart_area_base,
            kernel_end,
            arena_start,
            arena_end: arena_start + ARENA_BYTES as u64,
            guard_start,
            guard_end,
            stack_start,
            stack_end,
            trap_stack_start,
            trap_stack_end,
            syscall_stack_start,
            syscall_stack_end,
        }
    }
}

/// Báo cáo MMU cho TAP/log — địa chỉ **thật** đo từ lần boot này (không viết tay trong tài liệu).
#[derive(Clone, Copy, Debug)]
pub struct MmuReport {
    /// Giá trị `satp` đã ghi (mode 8 + PPN root).
    pub satp: u64,
    /// Địa chỉ vật lý bảng gốc (= `arena_start`).
    pub root: u64,
    /// Số bảng 4 KiB đã cấp trong arena.
    pub tables: usize,
    /// Byte arena đã dùng (bội số 4 KiB).
    pub used: usize,
    /// Arena `[start, end)`.
    pub arena_start: u64,
    pub arena_end: u64,
    /// `__kernel_end` (trước khi căn lề arena).
    pub kernel_end: u64,
    /// Base vùng NOLOAD per-hart (`__hart_area_base`) — guard/stack/trap stack của `MAX_HARTS`.
    pub hart_area_base: u64,
    /// Bước record per-hart (`HART_STRIDE`, đối chiếu linker ở [`init`]).
    pub hart_stride: u64,
    /// Guard page `[start, end)` — NONE.
    pub guard_start: u64,
    pub guard_end: u64,
    /// `.stack` boot.
    pub stack_start: u64,
    pub stack_end: u64,
    /// `.trap_stack`.
    pub trap_stack_start: u64,
    pub trap_stack_end: u64,
    /// Base nửa cao + physmap.
    pub high_base: u64,
    pub physmap_base: u64,
    /// Alias nửa cao + physmap ghi/đọc khớp identity (hardware walk thật).
    pub alias_ok: bool,
    /// PTE của [`WALK_SAMPLES`] VA **trước** khi `csrw satp`.
    pub pre_ptes: [u64; WALK_SAMPLES],
    /// PTE đọc lại **sau** khi bật `satp` — phải bằng `pre_ptes` (đọc lại PTE).
    pub post_ptes: [u64; WALK_SAMPLES],
}

/// Trạng thái MMU toàn cục (một hart ở KT-46; PerHart là KT-49).
static SATP_VALUE: AtomicU64 = AtomicU64::new(0);
static ROOT_PHYS: AtomicU64 = AtomicU64::new(0);
static TABLE_COUNT: AtomicU64 = AtomicU64::new(0);
static MMU_ENABLED: AtomicBool = AtomicBool::new(false);
/// Cờ **chỉ dùng cho probe test** (`cfg(feature = "ktest")`): load fault ở guard được resume một
/// lần để kiểm cả `scause=13` và `scause=15` trong một boot. Ảnh sản phẩm không có cờ/probe này;
/// mọi guard fault (load hay store) là **fatal**.
#[cfg(feature = "ktest")]
static GUARD_RECOVER_LOAD: AtomicBool = AtomicBool::new(false);
/// Biến thăm dò alias identity ↔ nửa cao ↔ physmap (nằm trong `.bss`, identity-mapped).
static ALIAS_PROBE: AtomicU64 = AtomicU64::new(0);

const ALIAS_MARKER_A: u64 = 0xA5A5_5A5A_C3C3_3C3C;
const ALIAS_MARKER_B: u64 = 0x5A5A_A5A5_3C3C_C3C3;

/// Đọc bộ nhớ vật lý (page table) bằng con trỏ — chỉ dùng cho walk phần mềm.
///
/// Bảng nằm trong arena identity-mapped nên địa chỉ vật lý cũng là VA; đọc sau khi bật `satp`
/// vẫn đúng. Nếu arena chưa map (chưa bật MMU) thì PA == VA sẵn.
struct PhysMemory;

impl PteMemory for PhysMemory {
    fn load(&self, table_phys: u64, index: usize) -> Option<u64> {
        if table_phys & (PAGE_BYTES - 1) != 0 || index >= 512 {
            return None;
        }
        // SAFETY: `table_phys` là bảng 4 KiB do builder cấp trong arena identity-mapped; mỗi
        // entry 8 B nằm trong bảng. Chỉ đọc, không ghi.
        Some(unsafe { core::ptr::read_volatile((table_phys as *const u64).add(index)) })
    }
}

/// Dựng page table từ arena và bật `satp` (hai bước: map xong mới `csrw satp` + `sfence.vma`).
///
/// Panic (qua `PanicHandler`) nếu arena không đủ, mapping sai, hoặc `satp` đọc lại khác — không
/// bao giờ bật MMU với bảng dở.
pub fn init() -> MmuReport {
    let layout = Layout::detect();
    // Linker đặt guard/stack/trap/arena sang 2 MiB kế tiếp (`ALIGN(2M)`): miền kernel image
    // `[0x8020_0000, 0x8040_0000)` map được bằng đúng một leaf 2 MiB, còn guard page ở vùng
    // NOLOAD phía sau mới có thể là NONE 4 KiB. Nếu linker đổi, panic thay vì map sai.
    if layout.guard_start != KERNEL_PHYS_BASE + LARGE_2M {
        panic!("mmu: guard khong o 0x8040_0000");
    }
    if layout.arena_end > layout.guard_start + LARGE_2M {
        panic!("mmu: NOLOAD + arena vuot 2 MiB");
    }
    if layout.kernel_end > layout.arena_start {
        panic!("mmu: kernel_end > arena_start");
    }
    // KT-49: linker phải đặt đúng vùng NOLOAD per-hart mà `plat-common::smp` mô tả — guard của
    // hart 0 ở base, stack/trap stack theo offsets, và `__kernel_end` đúng cuối record hart 7.
    // Sai ⇒ panic ngay (không chạy với stack hart này đè guard hart khác).
    if layout.guard_start != layout.hart_area_base
        || layout.stack_start != layout.hart_area_base + stack_bottom_offset(0)
        || layout.stack_end != layout.hart_area_base + stack_top_offset(0)
        || layout.trap_stack_start != layout.hart_area_base + trap_bottom_offset(0)
        || layout.trap_stack_end != layout.hart_area_base + trap_top_offset(0)
        || layout.syscall_stack_start != layout.hart_area_base + syscall_bottom_offset(0)
        || layout.syscall_stack_end != layout.hart_area_base + syscall_top_offset(0)
        || layout.kernel_end != layout.hart_area_base + HART_AREA_BYTES
    {
        panic!("mmu: bo cuc per-hart trong linker.ld khong khop plat-common::smp");
    }

    // SAFETY: arena là vùng RAM sau `__kernel_end` (không thuộc ảnh, không ai dùng); trait
    // `PageTableBuilder` chỉ ghi trong `[arena_start, arena_end)`.
    let arena =
        unsafe { core::slice::from_raw_parts_mut(layout.arena_start as *mut u8, ARENA_BYTES) };
    let mut builder = match PageTableBuilder::new(arena, layout.arena_start) {
        Ok(builder) => builder,
        Err(error) => fail(error),
    };
    if let Err(error) = build_tables(&mut builder, &layout) {
        fail(error);
    }
    let root = builder.root();
    let tables = builder.tables();
    let used = builder.used_bytes();
    if root != layout.arena_start || tables < 4 || used == 0 {
        panic!("mmu: arena/root khong hop le");
    }

    let samples = sample_vas(&layout);
    let mut pre_ptes = [0u64; WALK_SAMPLES];
    for (index, va) in samples.iter().enumerate() {
        pre_ptes[index] = walk_with(&PhysMemory, root, *va).raw();
    }
    let _ = builder;

    let satp = satp_sv39(root);
    // Bước 2: bảng đã xong — mới bật `satp` + `sfence.vma`. Một asm block để không có thao tác
    // bộ nhớ nào chen giữa hai lệnh.
    // SAFETY: `satp` mode 8 + PPN bảng gốc đã dựng; mọi VA kernel đang chạy (PC/sp/globals/UART)
    // đã được identity-map trước lệnh này. `sfence.vma` xoá TLB cũ (bare mode).
    unsafe {
        asm!(
            "csrw satp, {satp}",
            "sfence.vma",
            satp = in(reg) satp,
            options(nostack, preserves_flags),
        );
    }
    let readback = read_satp();
    if readback != satp {
        panic!("mmu: satp doc lai khac");
    }
    // Đặt cờ **sau** khi `satp` đã bật; TAP đọc lại bằng `mmu_enabled()`.
    MMU_ENABLED.store(true, Ordering::SeqCst);
    ROOT_PHYS.store(root, Ordering::SeqCst);
    SATP_VALUE.store(satp, Ordering::SeqCst);
    TABLE_COUNT.store(tables as u64, Ordering::SeqCst);

    let mut post_ptes = [0u64; WALK_SAMPLES];
    for (index, va) in samples.iter().enumerate() {
        post_ptes[index] = walk_sw(*va).raw();
    }
    let alias_ok = alias_selftest(&layout);
    MmuReport {
        satp,
        root,
        tables,
        used,
        arena_start: layout.arena_start,
        arena_end: layout.arena_end,
        kernel_end: layout.kernel_end,
        hart_area_base: layout.hart_area_base,
        hart_stride: HART_STRIDE,
        guard_start: layout.guard_start,
        guard_end: layout.guard_end,
        stack_start: layout.stack_start,
        stack_end: layout.stack_end,
        trap_stack_start: layout.trap_stack_start,
        trap_stack_end: layout.trap_stack_end,
        high_base: KERNEL_HIGH_BASE,
        physmap_base: PHYSMAP_BASE,
        alias_ok,
        pre_ptes,
        post_ptes,
    }
}

/// Không bao giờ trả: biến lỗi mapping thành panic có thông điệp ổn định.
fn fail(error: MapError) -> ! {
    panic!("mmu: map loi {:?}", error)
}

/// VA lấy mẫu PTE trước/sau khi bật `satp`.
fn sample_vas(layout: &Layout) -> [u64; WALK_SAMPLES] {
    [
        KERNEL_PHYS_BASE,
        KERNEL_HIGH_BASE,
        PHYSMAP_BASE + RAM_PHYS_BASE,
        UART0_BASE as u64,
        PLIC_BASE as u64,
        layout.stack_start,
        layout.trap_stack_start,
        layout.arena_start,
    ]
}

/// Dựng toàn bộ mapping (chưa bật MMU): identity kernel + nửa cao, physmap NX, MMIO Device, guard
/// NONE. Bảng trung gian do [`PageTableBuilder`] cấp từ arena.
fn build_tables(builder: &mut PageTableBuilder<'_>, layout: &Layout) -> Result<(), MapError> {
    map_kernel(builder, layout)?;
    map_mmio(builder)?;
    map_physmap(builder, layout)?;
    Ok(())
}

/// Identity + nửa cao cho kernel.
///
/// - **Image** `[0x8020_0000, 0x8040_0000)`: **một leaf 2 MiB** RWX (yêu cầu "kernel nửa cao
///   2 MiB"; W^X đầy đủ là KT-56) — linker đặt vùng NOLOAD sang 2 MiB kế tiếp nên guard không nằm
///   trong leaf này.
/// - **NOLOAD** `[0x8040_0000, arena_end)` (guard + `.stack` + `.trap_stack` + arena): leaf 4 KiB;
///   guard **không map** (NONE), stack/trap/arena RW NX. Physmap cũng khoét guard (xem
///   [`map_physmap`]).
fn map_kernel(builder: &mut PageTableBuilder<'_>, layout: &Layout) -> Result<(), MapError> {
    builder.map(
        KERNEL_PHYS_BASE,
        KERNEL_PHYS_BASE,
        1,
        PteFlags::kernel_rwx(),
    )?;
    // KT-51: **chỉ ảnh test** — leaf 2 MiB RW NX cho phép đo "stale mapping" (map VA→PA_A, remap
    // VA→PA_B, đọc không sfence phải ra giá trị cũ). Leaf nằm trong bảng cấp 1 đã có (cùng chỉ số
    // root với leaf image) nên không cấp thêm bảng/arena; ảnh sản phẩm **không** có mapping này.
    #[cfg(feature = "ktest")]
    builder.map(PROBE_VA, PROBE_PA_A, 1, PteFlags::kernel_rw_nx())?;
    // Probe #2 (TAP 27/28): leaf thứ hai trong **cùng** bảng cấp 1 — không thêm bảng/arena.
    #[cfg(feature = "ktest")]
    builder.map(PROBE2_VA, PROBE2_PA_C, 1, PteFlags::kernel_rw_nx())?;
    builder.map(
        KERNEL_HIGH_BASE,
        KERNEL_PHYS_BASE,
        1,
        PteFlags::kernel_rwx(),
    )?;
    let mut va = layout.guard_start;
    while va < layout.arena_end {
        // KT-49: guard **của từng hart** là NONE; mọi trang khác (stack/trap stack mọi hart +
        // arena) là leaf 4 KiB RW NX ở cả identity và nửa cao.
        if !is_guard_page(va) {
            let flags = PteFlags::kernel_rw_nx();
            builder.map(va, va, 0, flags)?;
            let high = KERNEL_HIGH_BASE + (va - KERNEL_PHYS_BASE);
            builder.map(high, va, 0, flags)?;
        }
        va += PAGE_BYTES;
    }
    Ok(())
}

/// MMIO Device: UART0 **4 KiB** + **hai cửa sổ PLIC 2 MiB** (KT-48 — priority/pending/enable ở
/// cửa sổ thấp, threshold/claim ở cửa sổ cao; đủ context 15 của `MAX_HARTS = 8`). Không map CLINT
/// (timer qua SBI — KT-47; KT-50 đọc/xoá `sip.SSIP` bằng CSR vì PMP của firmware **không** mở
/// ACLINT cho S-mode — đo được `load access fault` ở `0x0200_0004`, xem `v2/docs/k3-riscv64.md`
/// §13.4).
fn map_mmio(builder: &mut PageTableBuilder<'_>) -> Result<(), MapError> {
    builder.map(
        UART0_BASE as u64,
        UART0_BASE as u64,
        0,
        PteFlags::device_rw(),
    )?;
    builder.map(
        PLIC_WINDOW_LOW_BASE,
        PLIC_WINDOW_LOW_BASE,
        1,
        PteFlags::device_rw(),
    )?;
    builder.map(
        PLIC_WINDOW_HIGH_BASE,
        PLIC_WINDOW_HIGH_BASE,
        1,
        PteFlags::device_rw(),
    )?;
    // KT-72 (chỉ ảnh test): PCIe ECAM 2 MiB (bus 0/1) + 2 MiB đầu cửa sổ MMIO PCIe nơi driver
    // gán BAR modern của virtio-blk. Cả hai là Device RW NX; ảnh sản phẩm **không** map (không
    // đổi `.text`/mapping của soak).
    #[cfg(any(
        feature = "vblk",
        feature = "vcon",
        feature = "block",
        feature = "net",
        feature = "input",
        feature = "gpu"
    ))]
    {
        builder.map(
            crate::pci::ECAM_BASE,
            crate::pci::ECAM_BASE,
            1,
            PteFlags::device_rw(),
        )?;
        builder.map(
            crate::pci::PCI_MMIO_WINDOW_BASE,
            crate::pci::PCI_MMIO_WINDOW_BASE,
            1,
            PteFlags::device_rw(),
        )?;
        // KT-73: trang 2 MiB **thứ hai** của cửa sổ PCIe — BAR console gán ở `0x4020_0000`
        // (blk ở `0x4000_0000`), nên chỉ ảnh console cần leaf này.
        #[cfg(feature = "vcon")]
        builder.map(
            crate::pci::BAR_ASSIGN_BASE_CONSOLE,
            crate::pci::BAR_ASSIGN_BASE_CONSOLE,
            1,
            PteFlags::device_rw(),
        )?;
        // KT-245: leaf **thứ ba** của cửa sổ PCIe — BAR net gán ở `0x4040_0000`. Cửa sổ phải liền
        // một dải (`PCI_DRIVER_LEAVES`) vì `pci::mmu_range_conflict` kiểm bằng một khoảng duy nhất;
        // leaf 1 (console) map luôn cho liền dải dù ảnh net không dùng.
        #[cfg(feature = "net")]
        {
            builder.map(
                crate::pci::BAR_ASSIGN_BASE_CONSOLE,
                crate::pci::BAR_ASSIGN_BASE_CONSOLE,
                1,
                PteFlags::device_rw(),
            )?;
            builder.map(
                crate::pci::BAR_ASSIGN_BASE_NET,
                crate::pci::BAR_ASSIGN_BASE_NET,
                1,
                PteFlags::device_rw(),
            )?;
        }
        // KT-255: leaf **thứ tư** của cửa sổ PCIe — BAR input gán ở `0x4060_0000`. Cùng lý do
        // KT-245: cửa sổ phải **liền một dải** (`PCI_DRIVER_LEAVES` = 4) vì
        // `pci::mmu_range_conflict` kiểm bằng một khoảng duy nhất; leaf 1/2 map luôn cho liền dải
        // dù ảnh input không dùng.
        #[cfg(feature = "input")]
        {
            builder.map(
                crate::pci::BAR_ASSIGN_BASE_CONSOLE,
                crate::pci::BAR_ASSIGN_BASE_CONSOLE,
                1,
                PteFlags::device_rw(),
            )?;
            builder.map(
                crate::pci::BAR_ASSIGN_BASE_NET,
                crate::pci::BAR_ASSIGN_BASE_NET,
                1,
                PteFlags::device_rw(),
            )?;
            builder.map(
                crate::pci::BAR_ASSIGN_BASE_INPUT,
                crate::pci::BAR_ASSIGN_BASE_INPUT,
                1,
                PteFlags::device_rw(),
            )?;
        }
        // KT-263: leaf **thứ năm** của cửa sổ PCIe — BAR gpu gán ở `0x4080_0000`. Cùng lý do
        // KT-245/255: cửa sổ phải **liền một dải** (`PCI_DRIVER_LEAVES` = 5) vì
        // `pci::mmu_range_conflict` kiểm bằng một khoảng duy nhất; leaf 1..4 map luôn cho liền dải
        // dù ảnh gpu không dùng.
        #[cfg(feature = "gpu")]
        {
            builder.map(
                crate::pci::BAR_ASSIGN_BASE_CONSOLE,
                crate::pci::BAR_ASSIGN_BASE_CONSOLE,
                1,
                PteFlags::device_rw(),
            )?;
            builder.map(
                crate::pci::BAR_ASSIGN_BASE_NET,
                crate::pci::BAR_ASSIGN_BASE_NET,
                1,
                PteFlags::device_rw(),
            )?;
            builder.map(
                crate::pci::BAR_ASSIGN_BASE_INPUT,
                crate::pci::BAR_ASSIGN_BASE_INPUT,
                1,
                PteFlags::device_rw(),
            )?;
            builder.map(
                crate::pci::BAR_ASSIGN_BASE_GPU,
                crate::pci::BAR_ASSIGN_BASE_GPU,
                1,
                PteFlags::device_rw(),
            )?;
        }
    }
    Ok(())
}

/// Physmap RAM `0x8000_0000..0x8800_0000` bằng leaf 2 MiB, **NX**.
///
/// Chunk 2 MiB chứa guard page được tách thành 4 KiB và **khoét** guard ⇒ guard cũng NONE qua
/// physmap (không có cửa sau đọc/ghi trang bảo vệ).
fn map_physmap(builder: &mut PageTableBuilder<'_>, layout: &Layout) -> Result<(), MapError> {
    let guard_chunk = layout.guard_start & !(LARGE_2M - 1);
    let mut pa = RAM_PHYS_BASE;
    while pa < RAM_PHYS_BASE + RAM_BYTES {
        if pa == guard_chunk {
            let mut page = pa;
            while page < pa + LARGE_2M {
                // KT-49: khoét **mọi** guard per-hart khỏi physmap (không có cửa sau qua physmap).
                if !is_guard_page(page) {
                    builder.map(PHYSMAP_BASE + page, page, 0, PteFlags::kernel_rw_nx())?;
                }
                page += PAGE_BYTES;
            }
        } else {
            builder.map(PHYSMAP_BASE + pa, pa, 1, PteFlags::kernel_rw_nx())?;
        }
        pa += LARGE_2M;
    }
    Ok(())
}

/// Ghi lại `satp` với **ASID** `asid` (giữ MODE/PPN), cập nhật [`SATP_VALUE`], đọc lại xác nhận.
///
/// **Không** `sfence.vma` ở đây (KT-51): đổi **thẻ** ASID không làm sai translation — entry cũ
/// mang thẻ cũ không khớp thẻ mới. Chỉ khi ASID được **phát lại** ở thế hệ mới mới cần xả TLB, và
/// lúc đó `tlb::replace_lease` phát `FlushScope::All` (Q4). Cập nhật `SATP_VALUE` để hart thứ cấp
/// `adopt_satp` nhận luôn ASID đúng.
pub fn set_satp_asid(asid: u16) -> bool {
    let root = ROOT_PHYS.load(Ordering::SeqCst);
    if root == 0 {
        return false;
    }
    let satp = satp_sv39_asid(root, asid);
    // SAFETY: `satp` mode 8 + PPN bảng gốc đã dựng + ASID 8 bit; mọi VA kernel đang chạy đã được
    // identity-map từ trước. Chỉ ghi CSR của hart hiện tại.
    unsafe {
        asm!(
            "csrw satp, {satp}",
            satp = in(reg) satp,
            options(nostack, preserves_flags),
        );
    }
    SATP_VALUE.store(satp, Ordering::SeqCst);
    read_satp() == satp
}

/// ASID đang nạp trên hart hiện tại (đọc `satp` CSR) — bằng chứng TAP "mọi hart cùng ASID".
pub fn current_satp_asid() -> u16 {
    satp_asid(read_satp())
}

/// Đọc lại `satp` (CSR) — TAP dùng để chứng minh mode 8/PPN root.
fn read_satp() -> u64 {
    let value: u64;
    // SAFETY: `csrr satp` chỉ đọc CSR, không tác dụng phụ.
    unsafe { asm!("csrr {}, satp", out(reg) value, options(nomem, nostack)) };
    value
}

/// Alias hardware thật: ghi qua identity/write nửa cao, đọc lại qua nửa cao/physmap.
fn alias_selftest(layout: &Layout) -> bool {
    let low = core::ptr::addr_of!(ALIAS_PROBE) as u64;
    if !(low >= KERNEL_PHYS_BASE && low < layout.arena_end) {
        return false;
    }
    let high = KERNEL_HIGH_BASE + (low - KERNEL_PHYS_BASE);
    let physmap = PHYSMAP_BASE + low;
    ALIAS_PROBE.store(ALIAS_MARKER_A, Ordering::SeqCst);
    // SAFETY: các VA này vừa được map (identity, nửa cao, physmap) — đọc/ghi 8 B trong trang
    // `.bss`; không đụng vùng khác.
    let read_high = unsafe { core::ptr::read_volatile(high as *const u64) };
    let read_physmap = unsafe { core::ptr::read_volatile(physmap as *const u64) };
    unsafe { core::ptr::write_volatile(high as *mut u64, ALIAS_MARKER_B) };
    let read_low = ALIAS_PROBE.load(Ordering::SeqCst);
    let ok =
        read_high == ALIAS_MARKER_A && read_physmap == ALIAS_MARKER_A && read_low == ALIAS_MARKER_B;
    ALIAS_PROBE.store(0, Ordering::SeqCst);
    ok
}

/// `true` ⇔ `satp` đã được bật (mode 8) — TAP `mmu on`.
pub fn mmu_enabled() -> bool {
    MMU_ENABLED.load(Ordering::SeqCst)
}

/// `satp` hiện tại (đọc lại CSR, không lấy cache).
pub fn satp_now() -> u64 {
    read_satp()
}

/// Hart **thứ cấp** adopt `satp` của BSP (SBI HSM đánh thức hart với `satp = 0`): ghi lại `satp`
/// mode 8 + `sfence.vma`, đọc lại xác nhận. Trả `false` nếu MMU chưa bật (hart vào quá sớm).
pub fn adopt_satp() -> bool {
    let satp = SATP_VALUE.load(Ordering::SeqCst);
    if satp == 0 || !MMU_ENABLED.load(Ordering::SeqCst) {
        return false;
    }
    // SAFETY: bảng trang đã dựng xong ở BSP (mọi VA kernel đã map identity); hart này chỉ ghi
    // `satp` của chính nó + `sfence.vma` TLB cục bộ.
    unsafe {
        asm!(
            "csrw satp, {satp}",
            "sfence.vma",
            satp = in(reg) satp,
            options(nostack, preserves_flags),
        );
    }
    read_satp() == satp
}

/// PPN bảng gốc đã cấp (= `arena_start`).
pub fn root_phys() -> u64 {
    ROOT_PHYS.load(Ordering::SeqCst)
}

/// Số bảng 4 KiB đã cấp (gốc + trung gian).
pub fn table_count() -> usize {
    TABLE_COUNT.load(Ordering::SeqCst) as usize
}

/// Walk **phần mềm** trên bảng đang hoạt động — dùng đối chiếu với walk phần cứng.
pub fn walk_sw(va: u64) -> Walk {
    walk_with(&PhysMemory, ROOT_PHYS.load(Ordering::SeqCst), va)
}

/// Guard page **của hart 0** `[start, end)` — NONE ở mọi alias (tương thích KT-46).
pub fn guard_range() -> (u64, u64) {
    guard_range_of(0)
}

/// `.stack` boot **của hart 0** `[bottom, top)` (tương thích KT-46).
pub fn stack_range() -> (u64, u64) {
    stack_range_of(0)
}

/// `.trap_stack` **của hart 0** `[bottom, top)` (tương thích KT-46).
pub fn trap_stack_range() -> (u64, u64) {
    trap_stack_range_of(0)
}

/// Base vùng NOLOAD per-hart (`__hart_area_base`, linker thật).
pub fn hart_area_base() -> u64 {
    // SAFETY: chỉ lấy địa chỉ symbol linker, không dereference.
    unsafe { sym(&__hart_area_base) }
}

/// Bước record per-hart (`HART_STRIDE = guard + stack + trap stack + syscall stack`, KT-66).
pub const fn hart_stride() -> u64 {
    HART_STRIDE
}

/// Số hart mà vùng NOLOAD được đặt (`MAX_HARTS`).
pub const fn harts() -> usize {
    MAX_HARTS
}

/// Guard page `[start, end)` của `hart`.
pub fn guard_range_of(hart: usize) -> (u64, u64) {
    let base = hart_area_base() + smp_guard_offset(hart.min(MAX_HARTS - 1));
    (base, base + HART_GUARD_BYTES)
}

/// Stack `[bottom, top)` của `hart`.
pub fn stack_range_of(hart: usize) -> (u64, u64) {
    let base = hart_area_base();
    (
        base + stack_bottom_offset(hart.min(MAX_HARTS - 1)),
        base + stack_top_offset(hart.min(MAX_HARTS - 1)),
    )
}

/// Trap stack `[bottom, top)` của `hart`.
pub fn trap_stack_range_of(hart: usize) -> (u64, u64) {
    let base = hart_area_base();
    (
        base + trap_bottom_offset(hart.min(MAX_HARTS - 1)),
        base + trap_top_offset(hart.min(MAX_HARTS - 1)),
    )
}

/// `true` ⇔ `pa` là **guard của một hart** (trong vùng NOLOAD per-hart).
pub fn is_guard_page(pa: u64) -> bool {
    guard_owner(pa).is_some()
}

/// Hart **sở hữu** guard chứa `pa` (`None` nếu `pa` không phải guard của hart nào).
///
/// Dùng để chứng minh "mỗi hart chạm guard riêng ⇒ fault đúng hart": handler so
/// `guard_owner(stval)` với `hart_id()` hiện tại.
pub fn guard_owner(pa: u64) -> Option<usize> {
    let base = hart_area_base();
    if pa < base || pa >= base + HART_AREA_BYTES {
        return None;
    }
    let offset = pa - base;
    if offset % HART_STRIDE < HART_GUARD_BYTES {
        Some((offset / HART_STRIDE) as usize)
    } else {
        None
    }
}

/// Đỉnh stack của `hart` (`sp` ban đầu) — bằng chứng per-hart.
pub fn stack_top_of(hart: usize) -> u64 {
    hart_area_base() + stack_top_offset(hart.min(MAX_HARTS - 1))
}

/// Đỉnh trap stack của `hart` (`sscratch` ban đầu) — bằng chứng per-hart.
pub fn trap_top_of(hart: usize) -> u64 {
    hart_area_base() + trap_top_offset(hart.min(MAX_HARTS - 1))
}

/// **KT-66**: syscall stack `[bottom, top)` của `hart` — NOLOAD per-hart, identity-mapped thấp
/// (dùng được khi `satp` = root user, khác physmap/nửa cao).
pub fn syscall_stack_range_of(hart: usize) -> (u64, u64) {
    let base = hart_area_base();
    (
        base + syscall_bottom_offset(hart.min(MAX_HARTS - 1)),
        base + syscall_top_offset(hart.min(MAX_HARTS - 1)),
    )
}

/// Đỉnh syscall stack của `hart` (`sp` khởi đầu khi dispatch).
pub fn syscall_top_of(hart: usize) -> u64 {
    hart_area_base() + syscall_top_offset(hart.min(MAX_HARTS - 1))
}

/// Arena page table `[start, end)`.
pub fn arena_range() -> (u64, u64) {
    let layout = Layout::detect();
    (layout.arena_start, layout.arena_end)
}

/// VA nửa cao của một PA thuộc kernel/arena, nếu có.
pub fn high_alias(pa: u64) -> Option<u64> {
    let layout = Layout::detect();
    if pa >= KERNEL_PHYS_BASE && pa < layout.arena_end {
        Some(KERNEL_HIGH_BASE + (pa - KERNEL_PHYS_BASE))
    } else {
        None
    }
}

/// VA physmap của một PA thuộc RAM, nếu có.
pub fn physmap_alias(pa: u64) -> Option<u64> {
    if (RAM_PHYS_BASE..RAM_PHYS_BASE + RAM_BYTES).contains(&pa) {
        Some(PHYSMAP_BASE + pa)
    } else {
        None
    }
}

/// Đọc 8 B tại VA **đã map** — wrapper an toàn cho TAP/ảnh kernel (unsafe nằm trong crate này).
pub fn read_u64(va: u64) -> u64 {
    // SAFETY: caller (TAP) chỉ truyền VA vừa kiểm bằng `walk_sw` là leaf RW.
    unsafe { core::ptr::read_volatile(va as *const u64) }
}

/// Ghi 8 B tại VA **đã map** — wrapper an toàn cho TAP/ảnh kernel.
pub fn write_u64(va: u64, value: u64) {
    // SAFETY: caller (TAP) chỉ truyền VA vừa kiểm bằng `walk_sw` là leaf RW.
    unsafe { core::ptr::write_volatile(va as *mut u64, value) };
}

/// PTE của leaf thử nghiệm KT-51 (đọc qua [`walk_sw`]) — `None` nếu chưa map/không phải leaf.
#[cfg(feature = "ktest")]
pub fn probe_leaf_phys() -> Option<u64> {
    leaf_phys_of(PROBE_VA)
}

/// PTE của leaf **probe #2** (TAP 27/28).
#[cfg(feature = "ktest")]
pub fn probe2_leaf_phys() -> Option<u64> {
    leaf_phys_of(PROBE2_VA)
}

/// PA của leaf tại `va` nếu đang là leaf hợp lệ.
#[cfg(feature = "ktest")]
fn leaf_phys_of(va: u64) -> Option<u64> {
    let walk = walk_sw(va);
    if walk.valid && walk.leaf {
        Some(walk.phys)
    } else {
        None
    }
}

/// Đọc PTE ở `table_phys[index]` (bảng trong arena identity-mapped).
#[cfg(feature = "ktest")]
fn read_pte_at(table_phys: u64, index: usize) -> Option<u64> {
    if table_phys & (PAGE_BYTES - 1) != 0 || index >= 512 {
        return None;
    }
    // SAFETY: bảng 4 KiB do `PageTableBuilder` cấp trong arena identity-mapped; chỉ đọc 8 B.
    Some(unsafe { core::ptr::read_volatile((table_phys as *const u64).add(index)) })
}

/// Ghi PTE ở `table_phys[index]`.
#[cfg(feature = "ktest")]
fn write_pte_at(table_phys: u64, index: usize, value: u64) -> bool {
    if table_phys & (PAGE_BYTES - 1) != 0 || index >= 512 {
        return false;
    }
    // SAFETY: như `read_pte_at`; ghi đúng một entry 8 B trong bảng đã cấp (không cấp bảng mới).
    unsafe {
        core::ptr::write_volatile((table_phys as *mut u64).add(index), value);
    }
    true
}

/// **Remap** leaf 2 MiB của [`PROBE_VA`] sang `pa` (giữ nguyên cờ PTE) — KT-51.
///
/// Chỉ đổi **một** PTE leaf cấp 1 đã có (không cấp bảng, không đụng `satp`), nên đây đúng là thao
/// tác "sửa page table rồi phải `sfence.vma`" mà phép đo stale mapping cần. Trả `false` nếu bảng
/// chưa có/không phải leaf hoặc `pa` không căn 2 MiB.
#[cfg(feature = "ktest")]
pub fn remap_probe(pa: u64) -> bool {
    remap_leaf(PROBE_VA, pa)
}

/// **Remap** leaf 2 MiB của [`PROBE2_VA`] sang `pa` (TAP 27/28) — cùng cơ chế [`remap_probe`].
#[cfg(feature = "ktest")]
pub fn remap_probe2(pa: u64) -> bool {
    remap_leaf(PROBE2_VA, pa)
}

/// Remap leaf 2 MiB tại `va` sang `pa` (chỉ sửa một PTE leaf cấp 1 đã có).
#[cfg(feature = "ktest")]
fn remap_leaf(va: u64, pa: u64) -> bool {
    if pa & (LARGE_2M - 1) != 0 {
        return false;
    }
    let root = ROOT_PHYS.load(Ordering::SeqCst);
    if root == 0 {
        return false;
    }
    // Hình Sv39 (tính tay, không kéo thêm dependency): root index = bit 38..30, level-1 = 29..21.
    let root_index = ((va >> 30) & 0x1ff) as usize;
    let level1_index = ((va >> 21) & 0x1ff) as usize;
    let Some(root_entry) = read_pte_at(root, root_index) else {
        return false;
    };
    let leaf_mask = vifast_plat_common::mmu::PTE_READ
        | vifast_plat_common::mmu::PTE_WRITE
        | vifast_plat_common::mmu::PTE_EXEC;
    if root_entry & vifast_plat_common::mmu::PTE_VALID == 0 || root_entry & leaf_mask != 0 {
        return false;
    }
    let table = (root_entry >> vifast_plat_common::mmu::PTE_PPN_SHIFT) << 12;
    let index = level1_index;
    let Some(old) = read_pte_at(table, index) else {
        return false;
    };
    if old & vifast_plat_common::mmu::PTE_VALID == 0 || old & leaf_mask == 0 {
        return false;
    }
    let flags = old & 0x3ff; // 10 bit thấp: V/R/W/X/U/G/A/D + RSW
    let pte = ((pa >> 12) << vifast_plat_common::mmu::PTE_PPN_SHIFT) | flags;
    write_pte_at(table, index, pte)
}

/// Bảng cấp 0 dự phòng cho **scratch mapping** của probe KT-56 (chỉ ảnh test).
///
/// 4 KiB `.bss`, căn 4 KiB; chỉ dùng khi `map_scratch_4k` cần một bảng cấp 0 mới dưới một
/// entry cấp 1 **chưa dùng** của root kernel (KT-56 dùng VA `0x1000` của `.scn` 26 — root[0]
/// là bảng K0 đã có cho UART/PLIC, entry level-1 `0` chưa dùng). `unmap_scratch_4k` trả entry
/// level-1 về 0 nên bảng này không còn được tham chiếu sau mỗi probe.
#[cfg(feature = "ktest")]
#[repr(C, align(4096))]
struct ScratchL0([u64; 512]);

#[cfg(feature = "ktest")]
static mut SCRATCH_L0: ScratchL0 = ScratchL0([0; 512]);

/// `sfence.vma` toàn bộ (xả TLB sau khi sửa scratch mapping).
#[cfg(feature = "ktest")]
fn sfence_all() {
    // SAFETY: chỉ xả TLB của hart hiện tại, không đụng bộ nhớ.
    unsafe { asm!("sfence.vma zero, zero", options(nostack, preserves_flags)) };
}

/// **Scratch mapping 4 KiB** vào root kernel — chỉ ảnh test, phục vụ probe trap thật của KT-56.
///
/// `.scn` 26 ghi vào VA `0x1000` (root index 0) mà root kernel đã có bảng K0 cho UART/PLIC;
/// để `stval` của trap **đúng** VA kịch bản (không dùng alias), hàm này tạm nối **một** leaf
/// 4 KiB dưới entry level-1 `0` (entry này luôn invalid trong layout K3 — IMAGE ở root index 2):
/// cấp bảng cấp 0 dự phòng `SCRATCH_L0`, ghi leaf `pa`, `sfence.vma`. [`unmap_scratch_4k`] trả
/// entry về 0. Trả `false` (không sửa gì) nếu entry level-1 đã có mapping.
#[cfg(feature = "ktest")]
pub fn map_scratch_4k(va: u64, pa: u64, read: bool, write: bool, exec: bool) -> bool {
    use vifast_plat_common::mmu::{PTE_ACCESSED, PTE_DIRTY, PTE_READ, PTE_VALID, PTE_WRITE};
    if va & (PAGE_BYTES - 1) != 0 || pa & (PAGE_BYTES - 1) != 0 {
        return false;
    }
    let root = ROOT_PHYS.load(Ordering::SeqCst);
    if root == 0 {
        return false;
    }
    let root_index = ((va >> 30) & 0x1ff) as usize;
    let level1_index = ((va >> 21) & 0x1ff) as usize;
    let level0_index = ((va >> 12) & 0x1ff) as usize;
    let Some(root_entry) = read_pte_at(root, root_index) else {
        return false;
    };
    let leaf_mask = PTE_READ | PTE_WRITE | vifast_plat_common::mmu::PTE_EXEC;
    if root_entry & PTE_VALID == 0 || root_entry & leaf_mask != 0 {
        return false;
    }
    let table = (root_entry >> vifast_plat_common::mmu::PTE_PPN_SHIFT) << 12;
    let Some(old) = read_pte_at(table, level1_index) else {
        return false;
    };
    if old & PTE_VALID != 0 {
        return false;
    }
    // SAFETY: chỉ lấy địa chỉ của static mut, không tạo tham chiếu `&mut`.
    let scratch = unsafe { core::ptr::addr_of_mut!(SCRATCH_L0.0) as *mut u64 };
    if scratch.is_null() {
        return false;
    }
    // SAFETY: `scratch` trỏ tới mảng 512×u64 trong `.bss` của chính ảnh kernel (identity map);
    // zero 4 KiB rồi ghi hai PTE bên dưới. Chỉ một hart chạy probe tại một thời điểm.
    unsafe {
        core::ptr::write_bytes(scratch as *mut u8, 0, PAGE_BYTES as usize);
        let table_pte =
            (((scratch as u64) >> 12) << vifast_plat_common::mmu::PTE_PPN_SHIFT) | PTE_VALID;
        let mut flags = PTE_VALID | PTE_ACCESSED | PTE_DIRTY;
        if read {
            flags |= PTE_READ;
        }
        if write {
            flags |= PTE_WRITE;
        }
        if exec {
            flags |= vifast_plat_common::mmu::PTE_EXEC;
        }
        if flags & (PTE_READ | PTE_WRITE | vifast_plat_common::mmu::PTE_EXEC) == 0 {
            flags |= PTE_READ;
        }
        let leaf = ((pa >> 12) << vifast_plat_common::mmu::PTE_PPN_SHIFT) | flags;
        core::ptr::write_volatile(scratch.add(level0_index), leaf);
        if !write_pte_at(table, level1_index, table_pte) {
            return false;
        }
    }
    sfence_all();
    true
}

/// Gỡ scratch mapping của [`map_scratch_4k`]: trả entry level-1 về 0 + `sfence.vma`.
#[cfg(feature = "ktest")]
pub fn unmap_scratch_4k(va: u64) -> bool {
    use vifast_plat_common::mmu::PTE_VALID;
    if va & (PAGE_BYTES - 1) != 0 {
        return false;
    }
    let root = ROOT_PHYS.load(Ordering::SeqCst);
    if root == 0 {
        return false;
    }
    let root_index = ((va >> 30) & 0x1ff) as usize;
    let level1_index = ((va >> 21) & 0x1ff) as usize;
    let Some(root_entry) = read_pte_at(root, root_index) else {
        return false;
    };
    if root_entry & PTE_VALID == 0 {
        return false;
    }
    let table = (root_entry >> vifast_plat_common::mmu::PTE_PPN_SHIFT) << 12;
    let ok = write_pte_at(table, level1_index, 0);
    sfence_all();
    ok
}

/// Guard page fault **thật** (`scause` 12/13/15, `stval` trong guard page **của hart nào đó**)
/// — KT-46/49. Guard per-hart nên kiểm theo **chủ sở hữu** ([`guard_owner`]), không chỉ guard
/// của hart 0.
pub fn is_guard_fault(exc: &Exception) -> bool {
    if !matches!(
        exc.kind,
        ExceptionKind::InstructionPageFault
            | ExceptionKind::LoadPageFault
            | ExceptionKind::StorePageFault
    ) {
        return false;
    }
    guard_owner(exc.stval).is_some()
}

/// Kết quả xử lý guard fault.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuardOutcome {
    /// Load fault đã được resume **một lần** bởi probe test (để kiểm cả 13 và 15 trong một boot).
    Recovered,
    /// Chính sách mặc định: fatal — dump + halt.
    Fatal,
}

/// Xử lý guard fault. Mặc định **fatal**; chỉ load fault đi kèm cờ [`probe_guard_load`] mới được
/// resume (đúng 1 instruction `ld` 4 B) để TAP kiểm `scause=13` trước khi kiểm `scause=15` fatal.
pub fn handle_guard_fault(tf: &mut TrapFrame) -> GuardOutcome {
    // Resume **chỉ** có trong ảnh test (feature `ktest`); ảnh sản phẩm biên dịch nhánh này ra
    // ngoài ⇒ mọi guard fault luôn fatal, không có probe/chuỗi test trong ảnh sản phẩm.
    #[cfg(feature = "ktest")]
    if tf.scause == EXC_LOAD_PAGE_FAULT && GUARD_RECOVER_LOAD.swap(false, Ordering::SeqCst) {
        // KT-49: chỉ resume khi `stval` nằm trong guard **của chính hart đang chạy** — nếu không,
        // fault vẫn fatal (chạm guard của hart khác phải bị bắt, không được "chữa" hộ).
        let hart = Riscv64Virt.hart_id();
        if guard_owner(tf.stval) == Some(hart) {
            let sepc = tf.sepc;
            let length = instruction_len(ExceptionKind::LoadPageFault, sepc as usize);
            tf.advance_pc(length);
            let mut writer = PlatformWriter(&Riscv64Virt);
            let _ = writeln!(
                writer,
                "# guard fault load scause=0xd stval=0x{:x} sepc=0x{:x} len={} recovered=1 hart={}",
                tf.stval, sepc, length, hart
            );
            return GuardOutcome::Recovered;
        }
    }
    let _ = tf;
    GuardOutcome::Fatal
}

/// Chạm guard page bằng **load** (probe **chỉ ảnh test**, `#[cfg(feature = "ktest")]`): bật cờ
/// resume một lần rồi `ld` 8 B.
///
/// Nếu guard thật sự NONE, page fault `scause=13` vào `stvec`, handler in bằng chứng + resume qua
/// instruction; nếu guard bị map (lỗi), hàm trả giá trị đọc được và cờ vẫn bật — TAP phát hiện.
#[cfg(feature = "ktest")]
pub fn probe_guard_load(addr: u64) -> u64 {
    GUARD_RECOVER_LOAD.store(true, Ordering::SeqCst);
    let value: u64;
    // SAFETY: `addr` là guard page (NONE) — cố ý gây fault; handler của chính kernel xử lý. Nếu
    // fault bị resume, `value` không được dùng (chỉ để giữ instruction `ld`).
    unsafe {
        asm!(
            "ld {value}, 0({addr})",
            value = out(reg) value,
            addr = in(reg) addr,
            options(nostack),
        );
    }
    value
}

/// Chạm guard page bằng **store** (probe **chỉ ảnh test**, `#[cfg(feature = "ktest")]`) — luôn
/// fatal theo chính sách mặc định.
#[cfg(feature = "ktest")]
pub fn probe_guard_store(addr: u64) {
    // SAFETY: `addr` là guard page (NONE) — cố ý gây store page fault `scause=15`; handler dump +
    // halt. Không quay lại.
    unsafe {
        asm!(
            "sd zero, 0({addr})",
            addr = in(reg) addr,
            options(nostack),
        );
    }
}

/// `true` ⇔ cờ resume load-guard vẫn bật (nghĩa là load **không** fault — guard bị map sai).
#[cfg(feature = "ktest")]
pub fn guard_recovery_armed() -> bool {
    GUARD_RECOVER_LOAD.load(Ordering::SeqCst)
}
