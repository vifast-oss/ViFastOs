// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Adapter PCIe ECAM trên QEMU `virt` aarch64** — KT-334 (trước: driver đầy đủ KT-273).
//!
//! Quy trình quét/đo BAR/capability đã chuyển về [`vifast_virtio::pcibus`] (crate thuần, test
//! host). Tệp này giữ phần **thuộc nền tảng aarch64**:
//!
//! - ECAM ở PA `0x40_1000_0000` — PA này **không canonical** 39-bit (bit 38 = 1) và
//!   `PHYSMAP_BASE + PA` tràn `u64`, nên driver map một **cửa sổ VA riêng** [`ECAM_VA_BASE`]
//!   (nửa thấp, 256 MiB = 256 bus bằng block 2 MiB, KT-302) và đọc `VA = ECAM_VA_BASE + (PA −
//!   ECAM_BASE)`;
//! - MMIO đi qua [`crate::asm`] — mô-đun duy nhất của crate được `unsafe` (D2-17); tệp này
//!   **không** có `unsafe`;
//! - driver **tự gán** BAR trong cửa sổ MMIO 32-bit (QEMU `-kernel` không có firmware enumerate).
//!
//! Báo cáo [`PciReport`] dùng chung 3 arch; TAP của ảnh aarch64 in cùng các trường như trước.

use vifast_virtio::pci::{
    PciCfg, CFG_DEVICE_ID, CFG_HEADER_TYPE, CFG_STATUS, CFG_VENDOR_ID, STATUS_CAP_LIST,
};
use vifast_virtio::pcibus::{discover, DriverAssign, PciBus, PciPolicy, ScanRange};

use crate::asm;
use crate::resources::{GIC, GICR, GIC_CPU, PCI_ECAM, PCI_MMIO32, RAM, UART0};

/// Re-export các kiểu mà **ảnh**/test cần (một chiều: ảnh chỉ phụ thuộc crate arch, D2-17).
pub use vifast_virtio::pci::{BarKind, PciError, VirtioKind};
pub use vifast_virtio::pcibus::{bar_kind_name, PciFail, PciReport};

/// Base ECAM của QEMU `virt` aarch64 — **dẫn xuất** từ bảng tài nguyên (một nguồn, KT-81).
pub const ECAM_BASE: u64 = PCI_ECAM.base;

/// Base **VA** của cửa sổ ECAM trong ảnh — nửa thấp, 2 MiB căn lề, nối tiếp dãy VA đã dùng.
pub const ECAM_VA_BASE: u64 = 0x8000_0000;

/// Cỡ ECAM driver **map** — **256 MiB = 256 bus**, map bằng block 2 MiB (KT-302).
pub const ECAM_MAP_BYTES: u64 = 256 << 20;

/// Số bus quét — **suy từ** cửa sổ ECAM đã map, không viết tay.
pub const SCAN_BUSES: u16 = (ECAM_MAP_BYTES >> 20) as u16;

/// Số device trên một bus (PCI: 0..31).
pub const SCAN_DEVICES: u8 = 32;

/// Số function của một device (PCI: 0..7).
pub const SCAN_FUNCTIONS: u8 = 8;

/// Cửa sổ MMIO 32-bit của PCIe — nơi driver **gán BAR** (dẫn xuất từ bảng tài nguyên).
pub const PCI_MMIO_WINDOW_BASE: u64 = PCI_MMIO32.base;

/// Cỡ cửa sổ MMIO 32-bit.
pub const PCI_MMIO_WINDOW_BYTES: u64 = PCI_MMIO32.size;

/// Cỡ **một leaf** 2 MiB của cửa sổ PCIe — mỗi loại thiết bị một leaf (khuôn riscv64).
pub const BAR_ASSIGN_BYTES: u64 = 2 << 20;

/// Địa chỉ driver **gán** cho BAR modern của virtio-blk — đầu cửa sổ PCIe, căn 2 MiB.
pub const BAR_ASSIGN_BASE: u64 = PCI_MMIO_WINDOW_BASE;

/// Trần số BAR thu thập từ function khác trên bus (QEMU virt: host bridge + NIC + blk).
pub const MAX_OTHER_BARS: usize = 8;

/// Vùng nền tảng **không được** gán BAR chồng lên: UART0, GICD, GICC, GICR, ECAM đã map, RAM.
pub const RESERVED_RANGES: [(u64, u64); 6] = [
    (UART0.base, UART0.size),
    (GIC.base, GIC.size),
    (GIC_CPU.base, GIC_CPU.size),
    (GICR.base, GICR.size),
    (ECAM_BASE, ECAM_MAP_BYTES),
    (RAM.base, RAM.size),
];

/// Hai khoảng `[a, a+alen)` và `[b, b+blen)` có chồng nhau không (khoảng rỗng ⇒ không).
const fn ranges_overlap(a: u64, alen: u64, b: u64, blen: u64) -> bool {
    alen != 0 && blen != 0 && a < b.saturating_add(blen) && b < a.saturating_add(alen)
}

/// `true` ⇔ `[assigned, assigned+size)` chồng một vùng trong [`RESERVED_RANGES`].
const fn reserved_conflict(assigned: u64, size: u64) -> bool {
    let mut i = 0;
    while i < RESERVED_RANGES.len() {
        if ranges_overlap(assigned, size, RESERVED_RANGES[i].0, RESERVED_RANGES[i].1) {
            return true;
        }
        i += 1;
    }
    false
}

// ---- Bất biến **lúc biên dịch**: cửa sổ BAR nằm trong cửa sổ PCIe và không đè vùng nền tảng.
const _: () = assert!(ECAM_MAP_BYTES <= PCI_ECAM.size);
const _: () = assert!(ECAM_VA_BASE.is_multiple_of(2 << 20));
const _: () = assert!(ECAM_VA_BASE + ECAM_MAP_BYTES <= (1u64 << 38));
const _: () = assert!(BAR_ASSIGN_BASE >= PCI_MMIO_WINDOW_BASE);
const _: () =
    assert!(BAR_ASSIGN_BASE + BAR_ASSIGN_BYTES <= PCI_MMIO_WINDOW_BASE + PCI_MMIO_WINDOW_BYTES);
const _: () = assert!(!reserved_conflict(BAR_ASSIGN_BASE, BAR_ASSIGN_BYTES));

/// Địa chỉ **vật lý** của thanh ghi `reg` trong config space của `(bus, dev, func)`.
pub const fn ecam_pa(bus: u8, dev: u8, func: u8, reg: u8) -> u64 {
    ECAM_BASE + ((bus as u64) << 20) + ((dev as u64) << 15) + ((func as u64) << 12) + reg as u64
}

/// Địa chỉ **ảo** của thanh ghi `reg` — qua cửa sổ ECAM riêng ([`ECAM_VA_BASE`]).
pub const fn ecam_va(bus: u8, dev: u8, func: u8, reg: u8) -> u64 {
    ECAM_VA_BASE + (ecam_pa(bus, dev, func, reg) - ECAM_BASE)
}

/// Địa chỉ gán BAR cho **loại** thiết bị `kind`: mỗi loại một leaf 2 MiB (khuôn riscv64).
pub const fn bar_assign_base(kind: VirtioKind) -> u64 {
    let leaf = match kind {
        VirtioKind::Blk => 0,
        VirtioKind::Console => 1,
        VirtioKind::Net => 2,
        VirtioKind::Input => 3,
        VirtioKind::Gpu => 4,
    };
    BAR_ASSIGN_BASE + leaf * BAR_ASSIGN_BYTES
}

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
    /// Địa chỉ **ảo** (cửa sổ ECAM riêng) của `reg` trong config space của function này.
    #[inline]
    pub const fn addr(&self, reg: u8) -> u64 {
        ecam_va(self.bus, self.dev, self.func, reg)
    }

    /// Đọc 8 bit config space.
    #[inline]
    pub fn read8(&self, off: u8) -> u8 {
        asm::mmio_read_u8(self.addr(off))
    }

    /// Đọc 16 bit config space.
    #[inline]
    pub fn read16(&self, off: u8) -> u16 {
        asm::mmio_read_u16(self.addr(off))
    }

    /// Đọc 32 bit config space.
    #[inline]
    pub fn read32(&self, off: u8) -> u32 {
        asm::mmio_read_u32(self.addr(off))
    }

    /// Ghi 32 bit config space (BAR probe/gán).
    #[inline]
    pub fn write32(&self, off: u8, value: u32) {
        asm::mmio_write_u32(self.addr(off), value);
    }

    /// Ghi 16 bit config space (`command`).
    #[inline]
    pub fn write16(&self, off: u8, value: u16) {
        asm::mmio_write_u16(self.addr(off), value);
    }

    /// `true` ⇔ function này **có mặt** (vendor ≠ `0xffff`).
    #[inline]
    pub fn present(&self) -> bool {
        self.read16(CFG_VENDOR_ID) != 0xffff
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
        Ecam::write32(self, off, value);
    }
}

/// Bus ECAM của ảnh: mọi `(bus, dev, func)` đọc/ghi qua cùng cửa sổ VA.
pub struct EcamBus;

impl PciBus for EcamBus {
    type Cfg<'a> = Ecam;

    fn cfg(&mut self, bus: u8, dev: u8, func: u8) -> Ecam {
        Ecam { bus, dev, func }
    }

    fn evidence(&self, bus: u8, dev: u8, func: u8) -> (u64, u64, u64) {
        (
            0,
            ecam_pa(bus, dev, func, CFG_VENDOR_ID),
            ecam_va(bus, dev, func, CFG_VENDOR_ID),
        )
    }
}

/// Số đo của phép **kiểm ECAM** — bằng chứng "ECAM map đúng và giải mã thật".
#[derive(Clone, Copy, Debug)]
pub struct EcamProbe {
    /// Vendor id của host bridge (bus 0, dev 0, func 0).
    pub host_vendor: u16,
    /// Device id của host bridge.
    pub host_device: u16,
    /// Class code 24 bit của host bridge (kỳ vọng `0x060000`).
    pub host_class: u32,
    /// Revision id của host bridge.
    pub host_revision: u8,
    /// Vendor id đọc từ một slot **trống** (kỳ vọng `0xffff`).
    pub empty_vendor: u16,
    /// Số function **có mặt** đã thấy trong toàn dải quét.
    pub functions_seen: u32,
    /// Số bus đã quét (= [`SCAN_BUSES`]).
    pub buses_scanned: u16,
}

impl EcamProbe {
    /// `true` ⇔ ECAM giải mã thật: host bridge đúng class, slot trống trả `0xffff`, có ≥ 1 function.
    pub const fn ok(&self) -> bool {
        self.host_vendor != 0xffff
            && self.host_class == 0x0006_0000
            && self.empty_vendor == 0xffff
            && self.functions_seen >= 1
    }
}

/// Quét **toàn dải** `bus 0..SCAN_BUSES × dev 0..32 × func 0..8`, đếm function có mặt, và đọc
/// host bridge + một slot trống làm bằng chứng.
pub fn probe_ecam() -> EcamProbe {
    let host = Ecam {
        bus: 0,
        dev: 0,
        func: 0,
    };
    let host_vendor = host.read16(CFG_VENDOR_ID);
    let host_device = host.read16(CFG_DEVICE_ID);
    let host_revision = host.read8(0x08);
    let host_class = ((host.read8(0x0b) as u32) << 16)
        | ((host.read8(0x0a) as u32) << 8)
        | host.read8(0x09) as u32;
    // Slot **trống** ở cuối dải quét: QEMU không đặt thiết bị ở dev 31/func 7.
    let empty = Ecam {
        bus: 0,
        dev: SCAN_DEVICES - 1,
        func: SCAN_FUNCTIONS - 1,
    };
    let empty_vendor = empty.read16(CFG_VENDOR_ID);
    let mut functions_seen = 0u32;
    for bus in 0..SCAN_BUSES {
        // `SCAN_BUSES = 256` ⇒ bus 0..=255 vừa `u8`; ép sau khi biên đã do vòng lặp chặn.
        let bus = bus as u8;
        for dev in 0..SCAN_DEVICES {
            for func in 0..SCAN_FUNCTIONS {
                let ecam = Ecam { bus, dev, func };
                if ecam.present() {
                    functions_seen += 1;
                }
            }
        }
    }
    EcamProbe {
        host_vendor,
        host_device,
        host_class,
        host_revision,
        empty_vendor,
        functions_seen,
        buses_scanned: SCAN_BUSES,
    }
}

/// Chính sách PCI của aarch64: quét toàn dải ECAM đã map, driver tự gán BAR.
fn policy(kind: VirtioKind) -> PciPolicy {
    PciPolicy {
        kind,
        scan: ScanRange::new(SCAN_BUSES, SCAN_DEVICES, SCAN_FUNCTIONS),
        assign: DriverAssign {
            base: bar_assign_base,
            window_base: PCI_MMIO_WINDOW_BASE,
            window_bytes: PCI_MMIO_WINDOW_BYTES,
            max_bar: BAR_ASSIGN_BYTES,
        },
        reserved: &RESERVED_RANGES,
        extra_conflict: None,
    }
}

/// Quét dải bus/device/function, tìm **virtio-blk**, gán BAR và đọc capability (giữ API cũ).
pub fn init() -> Result<PciReport, PciFail> {
    init_for(VirtioKind::Blk)
}

/// Quét dải bus/device/function cho **loại thiết bị** `kind`, gán BAR và đọc capability.
pub fn init_for(kind: VirtioKind) -> Result<PciReport, PciFail> {
    discover(&mut EcamBus, &policy(kind))
}

/// `true` ⇔ function này khai **có** capability list (`status` bit 4) — chẩn đoán nhanh.
pub fn has_cap_list(ecam: &Ecam) -> bool {
    ecam.read16(CFG_STATUS) & STATUS_CAP_LIST != 0
}

/// `true` ⇔ function này là **multi-function** (`header_type` bit 7).
pub fn is_multifunction(ecam: &Ecam) -> bool {
    ecam.read8(CFG_HEADER_TYPE) & 0x80 != 0
}
