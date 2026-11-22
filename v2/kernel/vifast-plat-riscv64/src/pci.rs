// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Adapter PCI riscv64** — ECAM + chính sách gán BAR riêng của nền tảng (KT-334).
//!
//! Trước KT-334 tệp này chứa **toàn bộ** quy trình quét/đo BAR/capability (và bản sao gần y hệt
//! nằm ở aarch64). Nay quy trình ấy ở [`vifast_virtio::pcibus`] — crate **thuần**, test host được.
//! Ở đây chỉ còn phần thuộc nền tảng:
//!
//! - [`Ecam`] — truy cập config space qua cửa sổ ECAM 2 MiB đã map Device RW (con trỏ thô,
//!   `unsafe` thuộc D2-17);
//! - [`policy`] — dải quét, địa chỉ gán BAR theo từng loại thiết bị, vùng nền tảng bị cấm;
//! - [`mmu_range_conflict`] — kiểm **riêng của riscv64**: trang đã map bởi mapping khác thì không
//!   được gán BAR vào (walk bảng trang của chính ảnh).
//!
//! Báo cáo [`PciReport`] dùng chung 3 arch; TAP của `ktest-vblk` in cùng các trường như trước.

use core::ptr::{read_volatile, write_volatile};

use vifast_virtio::pci::{PciCfg, VirtioKind, CFG_VENDOR_ID};
use vifast_virtio::pcibus::{discover, DriverAssign, PciBus, PciPolicy, ScanRange};

pub use vifast_virtio::pci::BarKind;
pub use vifast_virtio::pcibus::{bar_kind_name, PciFail, PciReport};

use crate::mmu::{
    PLIC_WINDOW_BYTES, PLIC_WINDOW_HIGH_BASE, PLIC_WINDOW_LOW_BASE, RAM_BYTES, RAM_PHYS_BASE,
};

/// Base PA của ECAM (PCIe host bridge của QEMU `virt`).
pub const ECAM_BASE: u64 = vifast_plat_common::resources::PCI_ECAM.base;
/// Cửa sổ ECAM driver map (2 MiB = bus 0..2).
pub const ECAM_MAP_BYTES: u64 = 2 << 20;
/// Base cửa sổ MMIO 32-bit cấp cho BAR.
pub const PCI_MMIO_WINDOW_BASE: u64 = vifast_plat_common::resources::PCI_MMIO32.base;
/// Cỡ cửa sổ MMIO 32-bit.
pub const PCI_MMIO_WINDOW_BYTES: u64 = vifast_plat_common::resources::PCI_MMIO32.size;
/// Cỡ một leaf 2 MiB dùng cho mỗi loại thiết bị.
pub const PCI_MAP_LEAF_BYTES: u64 = 2 << 20;

const _: () = assert!(ECAM_MAP_BYTES <= vifast_plat_common::resources::PCI_ECAM.size);
const _: () = assert!(PCI_MMIO_WINDOW_BYTES == vifast_plat_common::resources::PCI_MMIO32.size);
const _: () = assert!(PCI_MMIO_WINDOW_BASE == vifast_plat_common::resources::PCI_MMIO32.base);

/// BAR của virtio-blk ở leaf đầu cửa sổ PCIe.
pub const BAR_ASSIGN_BASE: u64 = PCI_MMIO_WINDOW_BASE;
/// BAR của virtio-console (leaf kế).
pub const BAR_ASSIGN_BASE_CONSOLE: u64 = PCI_MMIO_WINDOW_BASE + PCI_MAP_LEAF_BYTES;
/// BAR của virtio-net.
pub const BAR_ASSIGN_BASE_NET: u64 = PCI_MMIO_WINDOW_BASE + 2 * PCI_MAP_LEAF_BYTES;
/// BAR của virtio-input.
pub const BAR_ASSIGN_BASE_INPUT: u64 = PCI_MMIO_WINDOW_BASE + 3 * PCI_MAP_LEAF_BYTES;
/// BAR của virtio-gpu.
pub const BAR_ASSIGN_BASE_GPU: u64 = PCI_MMIO_WINDOW_BASE + 4 * PCI_MAP_LEAF_BYTES;

/// Số leaf 2 MiB của cửa sổ PCIe mà driver **identity-map** (`mmu::map_mmio`).
///
/// Cửa sổ phải **liền một dải** từ [`PCI_MMIO_WINDOW_BASE`] vì [`mmu_range_conflict`] kiểm leaf
/// đã map bằng một khoảng duy nhất. Số leaf theo feature của ảnh (blk 1, console 2, net 3,
/// input 4, gpu 5) — giữ nguyên giá trị đã đo của các ảnh trước KT-334.
pub const PCI_DRIVER_LEAVES: u64 = if cfg!(feature = "gpu") {
    5
} else if cfg!(feature = "input") {
    4
} else if cfg!(feature = "net") {
    3
} else if cfg!(feature = "vcon") {
    2
} else {
    1
};
/// Cỡ cửa sổ PCIe driver map (dùng cho `mmu_range_conflict`).
pub const PCI_DRIVER_WINDOW_BYTES: u64 = PCI_DRIVER_LEAVES * PCI_MAP_LEAF_BYTES;

/// Số bus quét (chỉ bus 0 — `virt` không có cầu nối sau).
pub const SCAN_BUSES: u8 = 1;
/// Số device mỗi bus.
pub const SCAN_DEVICES: u8 = 32;
/// Số function của **function khác** khi kiểm chồng lấn BAR (multi-function).
pub const SCAN_OTHER_FUNCTIONS: u8 = 8;
/// Số function của chính thiết bị đích (quét function 0; virtio-blk là single-function).
pub const SCAN_FUNCTIONS: u8 = 1;
/// Số function khác tối đa đã đối chiếu BAR (giữ tên cũ cho tài liệu/TAP).
pub const MAX_OTHER_BARS: usize = 8;

/// Vùng nền tảng **cấm** gán BAR vào: UART, PLIC (hai cửa sổ), ECAM, RAM.
pub const RESERVED_RANGES: [(u64, u64); 5] = [
    (
        vifast_plat_common::resources::UART0.base,
        vifast_plat_common::resources::UART0.size,
    ),
    (PLIC_WINDOW_LOW_BASE, PLIC_WINDOW_BYTES),
    (PLIC_WINDOW_HIGH_BASE, PLIC_WINDOW_BYTES),
    (ECAM_BASE, ECAM_MAP_BYTES),
    (RAM_PHYS_BASE, RAM_BYTES),
];

/// Một function PCI: ba tham số định vị trong ECAM.
#[derive(Clone, Copy, Debug)]
pub struct Ecam {
    /// Bus.
    pub bus: u8,
    /// Device (0..31).
    pub dev: u8,
    /// Function (0..7).
    pub func: u8,
}

impl Ecam {
    /// Địa chỉ MMIO của `reg` (byte offset) trong config space của function này.
    #[inline]
    fn addr(&self, reg: u8) -> u64 {
        ECAM_BASE
            + ((self.bus as u64) << 20)
            + ((self.dev as u64) << 15)
            + ((self.func as u64) << 12)
            + reg as u64
    }

    /// Đọc 8 bit config space.
    #[inline]
    pub fn read8(&self, off: u8) -> u8 {
        // SAFETY: địa chỉ nằm trong cửa sổ ECAM đã map Device RW (`mmu::map_mmio`); offset < 256.
        unsafe { read_volatile(self.addr(off) as *const u8) }
    }
    /// Đọc 16 bit config space.
    #[inline]
    pub fn read16(&self, off: u8) -> u16 {
        // SAFETY: như `read8`; căn 2.
        unsafe { read_volatile(self.addr(off) as *const u16) }
    }
    /// Đọc 32 bit config space.
    #[inline]
    pub fn read32(&self, off: u8) -> u32 {
        // SAFETY: như `read8`; căn 4.
        unsafe { read_volatile(self.addr(off) as *const u32) }
    }
    /// Ghi 32 bit config space (BAR probe/gán).
    #[inline]
    pub fn write32(&self, off: u8, value: u32) {
        // SAFETY: như `read32`; thanh ghi cấu hình của thiết bị, không phải RAM.
        unsafe { write_volatile(self.addr(off) as *mut u32, value) };
    }
}

impl PciCfg for Ecam {
    fn read8(&self, off: u8) -> u8 {
        Ecam::read8(self, off)
    }
    fn read16(&self, off: u8) -> u16 {
        Ecam::read16(self, off)
    }
    fn read32(&self, off: u8) -> u32 {
        Ecam::read32(self, off)
    }
    fn write32(&mut self, off: u8, value: u32) {
        Ecam::write32(self, off, value)
    }
}

/// Bus ECAM của ảnh: mọi `(bus, dev, func)` đều đọc/ghi qua cùng cửa sổ PA.
pub struct EcamBus;

impl PciBus for EcamBus {
    type Cfg<'a> = Ecam;

    fn cfg(&mut self, bus: u8, dev: u8, func: u8) -> Ecam {
        Ecam { bus, dev, func }
    }

    fn evidence(&self, bus: u8, dev: u8, func: u8) -> (u64, u64, u64) {
        let ecam = Ecam { bus, dev, func };
        (ecam.addr(CFG_VENDOR_ID), 0, 0)
    }
}

/// `true` ⇔ có trang nào trong `[assigned, assigned+size)` **đã map** mà **không** phải leaf
/// identity của chính cửa sổ PCIe driver map (`mmu::map_mmio`) — tức gán BAR vào đó sẽ đè lên
/// mapping khác (image/RAM/ECAM/PLIC…). Trang **chưa map** không tính là xung đột (chưa có gì để
/// đè); lớp kiểm còn lại là `RESERVED_RANGES` + BAR function khác.
pub fn mmu_range_conflict(assigned: u64, size: u64) -> bool {
    let end = assigned.saturating_add(size);
    let mut page = assigned & !0xfff;
    while page < end {
        let walk = crate::mmu::walk_sw(page);
        if walk.valid {
            let leaf_bytes = 4096u64 << (9 * walk.level as u32);
            let own_window = walk.leaf
                && (PCI_MMIO_WINDOW_BASE..PCI_MMIO_WINDOW_BASE + PCI_DRIVER_WINDOW_BYTES)
                    .contains(&page)
                && walk.phys <= page
                && page - walk.phys < leaf_bytes;
            if !own_window {
                return true;
            }
        }
        page += 0x1000;
    }
    false
}

/// Địa chỉ gán BAR cho từng loại thiết bị (mỗi loại một leaf 2 MiB).
pub const fn bar_assign_base(kind: VirtioKind) -> u64 {
    match kind {
        VirtioKind::Blk => BAR_ASSIGN_BASE,
        VirtioKind::Console => BAR_ASSIGN_BASE_CONSOLE,
        VirtioKind::Net => BAR_ASSIGN_BASE_NET,
        VirtioKind::Input => BAR_ASSIGN_BASE_INPUT,
        VirtioKind::Gpu => BAR_ASSIGN_BASE_GPU,
    }
}

/// Chính sách PCI của riscv64: quét 2 bus, driver tự gán BAR, kiểm leaf page table.
fn policy(kind: VirtioKind) -> PciPolicy {
    PciPolicy {
        kind,
        scan: ScanRange::new(SCAN_BUSES as u16, SCAN_DEVICES, SCAN_FUNCTIONS),
        assign: DriverAssign {
            base: bar_assign_base,
            window_base: PCI_MMIO_WINDOW_BASE,
            window_bytes: PCI_MMIO_WINDOW_BYTES,
            max_bar: PCI_MAP_LEAF_BYTES,
        },
        reserved: &RESERVED_RANGES,
        extra_conflict: Some(mmu_range_conflict),
    }
}

/// Quét bus/device, tìm **virtio-blk**, gán BAR và đọc capability (giữ API cũ của KT-72).
pub fn init() -> Result<PciReport, PciFail> {
    init_for(VirtioKind::Blk)
}

/// Quét bus/device cho **loại thiết bị** `kind`, gán BAR và đọc capability.
pub fn init_for(kind: VirtioKind) -> Result<PciReport, PciFail> {
    discover(&mut EcamBus, &policy(kind))
}
