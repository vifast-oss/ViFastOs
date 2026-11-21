// SPDX-License-Identifier: MIT OR Apache-2.0
//! **PCIe ECAM trên QEMU `virt` aarch64** — KT-273 (K4' bước 3, PCI ECAM + virtio-blk).
//!
//! # Vì sao ECAM, và vì sao **không** identity-map được
//!
//! QEMU `virt` aarch64 đặt ECAM ở **`0x40_1000_0000`** (256 MiB) — **không** phải `0x3f00_0000`
//! như bản QEMU cũ / như riscv64. Số này **đo** từ DTB của chính QEMU trên máy đo
//! (`-M virt,gic-version=3,dumpdtb=…` + `dtc`), xem [`crate::resources::PCI_ECAM`].
//!
//! PA ấy có bit 38 = 1 nhưng bit 63..39 = 0 ⇒ **không canonical** với 39-bit VA của ảnh
//! (D2-147), nên **không** identity-map được. Cửa sổ **physmap** cũng **không** dùng được:
//! `PHYSMAP_BASE + PA` với `PA = 0x40_1000_0000` **tràn `u64`** (nửa cao 39-bit chỉ phủ PA
//! `< 2^38` = 256 GiB, còn ECAM ở 256 GiB + 16 MiB). Vì vậy driver map ECAM vào một **cửa sổ VA
//! riêng** ở nửa thấp — [`ECAM_VA_BASE`] = `0x8000_0000`, nối tiếp dãy VA đã dùng
//! (`0x5000_0000` probe, `0x6000_0000` VSpace, `0x7000_0000` EL0) — và đọc qua
//! `VA = ECAM_VA_BASE + (PA - ECAM_BASE)`.
//!
//! Cửa sổ BAR thì gán trong MMIO 32-bit (`0x1000_0000`, dưới 2^38) nên identity-map được, cùng
//! khuôn riscv64.
//!
//! # Cửa sổ ECAM map **đủ 256 bus** bằng block descriptor 2 MiB (KT-302)
//!
//! ECAM phủ **256 bus** (1 MiB/bus) = **256 MiB**. Map bằng trang 4 KiB cần **129** bảng trang
//! (mỗi 2 MiB một bảng mức 3 + một bảng mức 2) — vượt arena boot
//! ([`crate::bootmem::BOOT_ARENA_FRAMES`] = 44) và trần `.data+.bss` 256 KiB (bất biến I7).
//! KT-302 map bằng **block descriptor 2 MiB ở mức 2**: 256 MiB = **128 block** nằm trọn **một**
//! bảng mức 2 (512 entry) ⇒ chỉ tốn **2 frame** (bảng mức 1 + bảng mức 2). [`SCAN_BUSES`] vẫn
//! **suy từ** số byte đã map (`ECAM_MAP_BYTES >> 20`), không viết tay 256.
//!
//! QEMU `virt` đặt **mọi** thiết bị trên bus 0 (đo bằng `info pci` của monitor: host bridge
//! dev 0, virtio-net dev 1, virtio-blk dev 2), nhưng dải quét **đủ 256 bus** là yêu cầu của
//! KT-302: bus ≥ 16 trước đây **không** quét được vì đọc ra ngoài cửa sổ đã map ⇒ data abort.
//!
//! # Không có firmware enumerate ⇒ driver tự gán BAR
//!
//! QEMU `-kernel` **không** có firmware nào enumerate PCI (đo được: `info pci` cho
//! `BAR0/BAR1/BAR4: (not mapped)`), nên driver phải: ghi `0xffff_ffff` vào từng BAR ⇒ đọc mask
//! (PCI §6.2.5.1) ⇒ gán địa chỉ trong cửa sổ MMIO 32-bit ⇒ bật `command.MEMORY|BUS_MASTER`.
//! Logic mask/cap/queue là của `vifast-virtio` (test host); ở đây chỉ MMIO + quyết định địa chỉ.
//!
//! # `unsafe`
//!
//! Không có: mọi truy cập MMIO đi qua [`crate::asm`] (`mmio_read_*`/`mmio_write_*`) — mô-đun
//! duy nhất của crate được `unsafe` (D2-17). Tệp này chỉ **quyết định** địa chỉ/giá trị.

use vifast_virtio::pci::{
    bar_probe, first_conflict, parse_caps, read_ids, BarKind, PciCfg, VirtioCaps, BAR_COUNT,
    CFG_BAR0, CFG_COMMAND, CFG_DEVICE_ID, CFG_HEADER_TYPE, CFG_STATUS, CFG_VENDOR_ID,
    COMMAND_BUS_MASTER, COMMAND_MEMORY, STATUS_CAP_LIST,
};

use crate::asm;
use crate::resources::{GIC, GICR, GIC_CPU, PCI_ECAM, PCI_MMIO32, RAM, UART0};

/// Re-export hai kiểu mà **ảnh** cần để gọi driver mà không phải phụ thuộc trực tiếp
/// `vifast-virtio` (ảnh chỉ phụ thuộc crate arch — một chiều, D2-17).
pub use vifast_virtio::pci::{PciError, VirtioKind};

/// Base ECAM của QEMU `virt` aarch64 — **dẫn xuất** từ bảng tài nguyên (một nguồn, KT-81).
pub const ECAM_BASE: u64 = PCI_ECAM.base;

/// Base **VA** của cửa sổ ECAM trong ảnh — nửa thấp, 2 MiB căn lề, nối tiếp dãy VA đã dùng
/// (`0x5000_0000` probe, `0x6000_0000` VSpace, `0x7000_0000` EL0).
///
/// **Vì sao cần cửa sổ riêng** (không identity, không physmap): xem tài liệu đầu tệp — PA của
/// ECAM không canonical 39-bit **và** `PHYSMAP_BASE + PA` tràn `u64`.
pub const ECAM_VA_BASE: u64 = 0x8000_0000;

/// Cỡ ECAM driver **map** — **256 MiB = 256 bus**, map bằng block 2 MiB (KT-302).
///
/// 256 MiB bằng trang 4 KiB cần 129 frame (vượt arena 44 frame + trần I7); bằng block 2 MiB chỉ
/// tốn 2 frame. Xem tài liệu đầu tệp.
pub const ECAM_MAP_BYTES: u64 = 256 << 20;

/// Số bus quét — **suy từ** cửa sổ ECAM đã map, không viết tay.
///
/// `u16` vì 256 **không** vừa `u8`; số bus PCI vẫn là 8 bit (0..=255) nên [`Ecam::bus`] giữ
/// `u8` và vòng lặp ép `bus as u8` sau khi đã chặn biên bằng `SCAN_BUSES`.
pub const SCAN_BUSES: u16 = (ECAM_MAP_BYTES >> 20) as u16;

/// Số device trên một bus (PCI: 0..31).
pub const SCAN_DEVICES: u8 = 32;

/// Số function của một device (PCI: 0..7).
pub const SCAN_FUNCTIONS: u8 = 8;

/// Cửa sổ MMIO 32-bit của PCIe — nơi driver **gán BAR** (dẫn xuất từ bảng tài nguyên).
pub const PCI_MMIO_WINDOW_BASE: u64 = PCI_MMIO32.base;

/// Cỡ cửa sổ MMIO 32-bit (dùng để kiểm BAR gán ra nằm trong cửa sổ).
pub const PCI_MMIO_WINDOW_BYTES: u64 = PCI_MMIO32.size;

/// Cỡ **một leaf** 2 MiB của cửa sổ PCIe — mỗi loại thiết bị một leaf (khuôn riscv64).
pub const BAR_ASSIGN_BYTES: u64 = 2 << 20;

/// Địa chỉ driver **gán** cho BAR modern của virtio-blk — đầu cửa sổ PCIe, căn 2 MiB.
pub const BAR_ASSIGN_BASE: u64 = PCI_MMIO_WINDOW_BASE;

/// Trần số BAR thu thập từ function khác trên bus (QEMU virt: host bridge + NIC + blk).
pub const MAX_OTHER_BARS: usize = 8;

/// Vùng nền tảng **không được** gán BAR chồng lên: UART0, GICD, GICC, GICR, ECAM đã map, RAM.
///
/// RAM phủ luôn vùng ảnh kernel (`0x4008_0000`) nên không cần mục riêng cho ảnh. Đây là **dữ
/// kiện nền tảng** lấy từ bảng [`resources`], không phải số đo.
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
// Nếu ai đổi hằng số mà quên kiểm, ảnh **không biên dịch được** thay vì gán BAR đè im lặng.
const _: () = assert!(ECAM_MAP_BYTES <= PCI_ECAM.size);
const _: () = assert!(ECAM_VA_BASE.is_multiple_of(2 << 20));
const _: () = assert!(ECAM_VA_BASE + ECAM_MAP_BYTES <= (1u64 << 38));
const _: () = assert!(BAR_ASSIGN_BASE >= PCI_MMIO_WINDOW_BASE);
const _: () =
    assert!(BAR_ASSIGN_BASE + BAR_ASSIGN_BYTES <= PCI_MMIO_WINDOW_BASE + PCI_MMIO_WINDOW_BYTES);
const _: () = assert!(!reserved_conflict(BAR_ASSIGN_BASE, BAR_ASSIGN_BYTES));

/// Địa chỉ **vật lý** của thanh ghi `reg` trong config space của `(bus, dev, func)`.
///
/// Công thức PCIe ECAM: `base + (bus << 20) + (dev << 15) + (func << 12) + reg`.
pub const fn ecam_pa(bus: u8, dev: u8, func: u8, reg: u8) -> u64 {
    ECAM_BASE + ((bus as u64) << 20) + ((dev as u64) << 15) + ((func as u64) << 12) + reg as u64
}

/// Địa chỉ **ảo** của thanh ghi `reg` — qua cửa sổ ECAM riêng ([`ECAM_VA_BASE`]).
pub const fn ecam_va(bus: u8, dev: u8, func: u8, reg: u8) -> u64 {
    ECAM_VA_BASE + (ecam_pa(bus, dev, func, reg) - ECAM_BASE)
}

/// Địa chỉ gán BAR cho **loại** thiết bị `kind`: mỗi loại một leaf 2 MiB (khuôn riscv64).
///
/// KT-273 chỉ dùng [`VirtioKind::Blk`] (leaf 0); các leaf khác **chưa** được map trong ảnh này
/// — hàm vẫn khai đủ để hai driver cùng có mặt không tranh một trang khi mở rộng.
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
    /// Địa chỉ **ảo** (physmap) của `reg` trong config space của function này.
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
        Ecam::write32(self, off, value)
    }
}

/// Số đo của phép **kiểm ECAM** — bằng chứng "ECAM map đúng và giải mã thật".
///
/// Ba trường đầu là **đọc từ thiết bị**, không hằng: host bridge của PCIe **luôn** có class
/// `0x060000` (PCI class code "bridge/host"), và một slot **trống** phải trả `0xffff` — nếu
/// ECAM trỏ sai chỗ, hai điều kiện này không thể cùng đúng.
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
///
/// Đây là phép "quét bus/device/function" của KT-273: vòng lặp chạy đủ ba tầng; biên bus là
/// [`SCAN_BUSES`] (suy từ cửa sổ ECAM đã map — xem tài liệu đầu tệp).
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

/// Toàn bộ số đo của lần tìm thiết bị + gán BAR — **giá trị đọc lại từ thiết bị**, không hằng.
#[derive(Clone, Copy, Debug)]
pub struct PciReport {
    /// Số function có mặt đã thấy trước khi tìm ra thiết bị đích.
    pub functions_seen: u32,
    /// Bus của thiết bị.
    pub bus: u8,
    /// Device.
    pub dev: u8,
    /// Function.
    pub func: u8,
    /// Vendor id đọc từ config space.
    pub vendor: u16,
    /// Device id đọc từ config space.
    pub device: u16,
    /// Class code 24 bit.
    pub class: u32,
    /// Revision id.
    pub revision: u8,
    /// Chỉ số BAR mà driver dùng (modern, có đủ 4 capability).
    pub bar_index: u8,
    /// Loại BAR.
    pub bar_kind: BarKind,
    /// Cỡ BAR đo từ mask.
    pub bar_size: u64,
    /// Địa chỉ driver **gán** cho BAR (PA; identity-map nên cũng là VA).
    pub bar_assigned: u64,
    /// Giá trị **đọc lại** từ thanh ghi BAR sau khi gán (phải bằng `bar_assigned`).
    pub bar_readback: u64,
    /// `command` đọc lại sau khi bật memory + bus master.
    pub command: u16,
    /// Capability đọc từ chain của thiết bị.
    pub caps: VirtioCaps,
    /// Địa chỉ **PA** của ECAM cho function này (bằng chứng map đúng).
    pub ecam_pa: u64,
    /// Địa chỉ **VA** (physmap) của ECAM cho function này.
    pub ecam_va: u64,
    /// Số BAR **có địa chỉ** của function khác đã đối chiếu chồng lấn.
    pub other_bars: u16,
    /// Số vùng nền tảng đã đối chiếu.
    pub reserved_ranges: u16,
    /// `true` ⇔ đã kiểm chồng lấn (BAR khác + vùng nền tảng) trước khi gán.
    pub overlap_checked: bool,
    /// `true` ⇔ mọi capability có `offset + length <= bar_size`.
    pub cap_ranges_ok: bool,
}

/// Lỗi khởi tạo PCI kèm **chặng** (để TAP in lý do đo được — fail-closed, không PANIC im lặng).
#[derive(Clone, Copy, Debug)]
pub struct PciFail {
    /// Mã lỗi.
    pub error: PciError,
    /// Chặng: `nodevice`/`probing`/`chosen`/`size`/`caps`/`capbar`/`caprange`/`assign`/`conflict`/
    /// `readback`/`command`.
    pub stage: &'static str,
}

impl PciFail {
    const fn at(error: PciError, stage: &'static str) -> Self {
        Self { error, stage }
    }
}

/// Thu thập `(base, size)` các BAR **memory đã có địa chỉ** của mọi function khác trong dải quét.
///
/// Ghi `0xffff_ffff` để đo cỡ rồi **khôi phục nguyên giá trị cũ** (không phá cấu hình thiết bị
/// khác). BAR chưa gán địa chỉ (0) không thể chồng lấn nên bỏ qua.
fn collect_other_bars(target: Ecam) -> ([(u64, u64); MAX_OTHER_BARS], usize) {
    let mut bars = [(0u64, 0u64); MAX_OTHER_BARS];
    let mut count = 0usize;
    for bus in 0..SCAN_BUSES {
        let bus = bus as u8;
        for dev in 0..SCAN_DEVICES {
            for func in 0..SCAN_FUNCTIONS {
                let other = Ecam { bus, dev, func };
                if !other.present() {
                    continue;
                }
                if bus == target.bus && dev == target.dev && func == target.func {
                    continue;
                }
                let mut index = 0u8;
                while (index as usize) < BAR_COUNT {
                    let off = CFG_BAR0 + index * 4;
                    let raw_low = other.read32(off);
                    let is_io = raw_low & 1 != 0;
                    let is_64 = !is_io && raw_low & 0b110 == 0b100;
                    if !is_io && raw_low & !0xf != 0 {
                        let raw_high = if is_64 { other.read32(off + 4) } else { 0 };
                        let base = ((raw_low as u64) & !0xf) | ((raw_high as u64) << 32);
                        other.write32(off, 0xffff_ffff);
                        let mask_low = other.read32(off);
                        let mask_high = if is_64 {
                            other.write32(off + 4, 0xffff_ffff);
                            other.read32(off + 4)
                        } else {
                            0
                        };
                        other.write32(off, raw_low);
                        if is_64 {
                            other.write32(off + 4, raw_high);
                        }
                        if let Some(bar) = bar_probe(index, mask_low, mask_high) {
                            if count < MAX_OTHER_BARS {
                                bars[count] = (base, bar.size);
                                count += 1;
                            }
                        }
                    }
                    index += if is_64 { 2 } else { 1 };
                }
            }
        }
    }
    (bars, count)
}

/// Quét dải bus/device/function, tìm **virtio-blk**, gán BAR và đọc capability (KT-273).
pub fn init() -> Result<PciReport, PciFail> {
    init_for(VirtioKind::Blk)
}

/// Quét dải bus/device/function cho **loại thiết bị** `kind`, gán BAR và đọc capability.
///
/// Một đường quét duy nhất cho mọi thiết bị virtio (khuôn riscv64 KT-72/73): không nhân đôi
/// logic ECAM/BAR/cap (D2-110A). Chỉ khác **địa chỉ gán BAR** (mỗi loại một leaf 2 MiB).
pub fn init_for(kind: VirtioKind) -> Result<PciReport, PciFail> {
    let mut functions_seen = 0u32;
    let mut found: Option<(u8, u8, u8)> = None;
    'scan: for bus in 0..SCAN_BUSES {
        let bus = bus as u8;
        for dev in 0..SCAN_DEVICES {
            for func in 0..SCAN_FUNCTIONS {
                let ecam = Ecam { bus, dev, func };
                if !ecam.present() {
                    continue;
                }
                functions_seen += 1;
                let (vendor, device) = read_ids(&ecam);
                if kind.matches(vendor, device) {
                    found = Some((bus, dev, func));
                    break 'scan;
                }
            }
        }
    }
    let (bus, dev, func) = found.ok_or(PciFail::at(PciError::NoDevice, "nodevice"))?;
    let ecam = Ecam { bus, dev, func };
    let (vendor, device) = read_ids(&ecam);
    let class = ((ecam.read8(0x0b) as u32) << 16)
        | ((ecam.read8(0x0a) as u32) << 8)
        | ecam.read8(0x09) as u32;
    let revision = ecam.read8(0x08);

    // ---- BAR: đo cỡ bằng mask, chọn BAR memory lớn nhất (modern của virtio-pci = BAR4 64-bit)
    let mut chosen: Option<(u8, BarKind, u64)> = None;
    let mut index = 0u8;
    while (index as usize) < BAR_COUNT {
        let off = CFG_BAR0 + index * 4;
        let raw = ecam.read32(off);
        let is_io = raw & 1 != 0;
        let is_64 = !is_io && raw & 0b110 == 0b100;
        if !is_io {
            // Ghi toàn 1 rồi đọc mask — quy tắc PCI §6.2.5.1. Khôi phục 0 sau khi đo.
            ecam.write32(off, 0xffff_ffff);
            let mask_low = ecam.read32(off);
            let mask_high = if is_64 {
                ecam.write32(off + 4, 0xffff_ffff);
                let high = ecam.read32(off + 4);
                ecam.write32(off + 4, 0);
                high
            } else {
                0
            };
            ecam.write32(off, 0);
            if let Some(bar) = bar_probe(index, mask_low, mask_high) {
                let better = match chosen {
                    None => true,
                    Some((_, _, size)) => bar.size > size,
                };
                if better {
                    chosen = Some((bar.index, bar.kind, bar.size));
                }
            }
        }
        index += if is_64 { 2 } else { 1 };
    }
    let (bar_index, bar_kind, bar_size) =
        chosen.ok_or(PciFail::at(PciError::BarUnassigned, "chosen"))?;
    if !(0x1000..=BAR_ASSIGN_BYTES).contains(&bar_size) {
        return Err(PciFail::at(PciError::BarUnassigned, "size"));
    }
    // Capability **trước** khi gán: cần biết BAR chứa common/notify/device/isr có đúng BAR này.
    let caps = parse_caps(&ecam).map_err(|error| PciFail::at(error, "caps"))?;
    if caps.bar != bar_index || caps.common.bar != bar_index {
        return Err(PciFail::at(PciError::CapMissing, "capbar"));
    }
    // Capability phải nằm **trọn** trong BAR: `offset + length <= bar_size` (không chỉ offset).
    let cap_ranges_ok = caps.common.offset as u64 + caps.common.length as u64 <= bar_size
        && caps.notify.offset as u64 + caps.notify.length as u64 <= bar_size
        && caps.device.offset as u64 + caps.device.length as u64 <= bar_size
        && caps.isr.offset as u64 + caps.isr.length as u64 <= bar_size;
    if !cap_ranges_ok {
        return Err(PciFail::at(PciError::CapMissing, "caprange"));
    }
    // Gán BAR: một leaf 2 MiB riêng cho từng loại thiết bị (căn 2 MiB > mọi cỡ BAR hợp lệ ở đây).
    let assigned = bar_assign_base(kind);
    if assigned < PCI_MMIO_WINDOW_BASE
        || assigned + bar_size > PCI_MMIO_WINDOW_BASE + PCI_MMIO_WINDOW_BYTES
    {
        return Err(PciFail::at(PciError::BarUnassigned, "assign"));
    }
    // ---- CAO-2 (D2-110C): **kiểm chồng lấn TRƯỚC khi ghi BAR**; chồng lấn ⇒ fail-closed.
    // (i) BAR memory đã có địa chỉ của mọi function khác trong dải quét.
    let (other_bars, other_count) = collect_other_bars(ecam);
    if let Some(kind) = first_conflict(
        assigned,
        bar_size,
        &other_bars[..other_count],
        &RESERVED_RANGES,
    ) {
        return Err(PciFail::at(PciError::BarConflict, kind));
    }
    // (ii) ghi BAR + đọc lại: chỉ **sau** khi (i) sạch.
    let off = CFG_BAR0 + bar_index * 4;
    ecam.write32(off, assigned as u32);
    if bar_kind == BarKind::Memory64 {
        ecam.write32(off + 4, (assigned >> 32) as u32);
    }
    // Thanh ghi BAR giữ 4 bit **thuộc tính** (bit 3 prefetchable, bit 2:1 kiểu, bit 0 space) ở
    // lần đọc lại — địa chỉ thật là phần đã che 4 bit thấp (PCI §6.2.5.1). So sánh trên địa chỉ.
    let attr_mask = 0xfu64;
    let readback = ((ecam.read32(off) as u64) & !attr_mask)
        | if bar_kind == BarKind::Memory64 {
            (ecam.read32(off + 4) as u64) << 32
        } else {
            0
        };
    if readback != assigned {
        return Err(PciFail::at(PciError::BarUnassigned, "readback"));
    }
    // Bật decode memory + bus master (DMA), đọc lại.
    let command = ecam.read16(CFG_COMMAND) | COMMAND_MEMORY | COMMAND_BUS_MASTER;
    ecam.write16(CFG_COMMAND, command);
    let command = ecam.read16(CFG_COMMAND);
    if command & (COMMAND_MEMORY | COMMAND_BUS_MASTER) != COMMAND_MEMORY | COMMAND_BUS_MASTER {
        return Err(PciFail::at(PciError::BarUnassigned, "command"));
    }
    Ok(PciReport {
        functions_seen,
        bus,
        dev,
        func,
        vendor,
        device,
        class,
        revision,
        bar_index,
        bar_kind,
        bar_size,
        bar_assigned: assigned,
        bar_readback: readback,
        command,
        caps,
        ecam_pa: ecam_pa(bus, dev, func, CFG_VENDOR_ID),
        ecam_va: ecam_va(bus, dev, func, CFG_VENDOR_ID),
        other_bars: other_count as u16,
        reserved_ranges: RESERVED_RANGES.len() as u16,
        overlap_checked: true,
        cap_ranges_ok,
    })
}

/// Tên kiểu BAR để in evidence (D2-102: in từ biến, không viết chuỗi trùng lặp trong test).
pub const fn bar_kind_name(kind: BarKind) -> &'static str {
    match kind {
        BarKind::Io => "io",
        BarKind::Memory32 => "mem32",
        BarKind::Memory64 => "mem64",
    }
}

/// `true` ⇔ function này khai **có** capability list (`status` bit 4) — chẩn đoán nhanh.
pub fn has_cap_list(ecam: &Ecam) -> bool {
    ecam.read16(CFG_STATUS) & STATUS_CAP_LIST != 0
}

/// `true` ⇔ function này là **multi-function** (`header_type` bit 7).
pub fn is_multifunction(ecam: &Ecam) -> bool {
    ecam.read8(CFG_HEADER_TYPE) & 0x80 != 0
}
