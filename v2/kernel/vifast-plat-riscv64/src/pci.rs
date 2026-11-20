// SPDX-License-Identifier: MIT OR Apache-2.0
//! PCI trên QEMU `virt` riscv64 — **ECAM** — KT-72 (K6), chỉ ảnh test (`feature = "ktest"`).
//!
//! # Vì sao ECAM (không legacy I/O port)
//!
//! RISC-V **không** có lệnh `in`/`out`; "legacy I/O port" trên `virt` chỉ là một cửa sổ MMIO
//! (`0x0300_0000`, `gpex_ioport_window`) mà truy cập phải đi qua `0xCF8/0xCFC` hai bước. ECAM
//! (`0x3000_0000`, `pcie-mmcfg-mmio`, 256 MiB = bus<<20 | dev<<15 | func<<12 | reg) là **một**
//! phép đọc/ghi 32 bit cho mỗi thanh ghi ⇒ ít lệnh hơn, không cần khoá tuần tự, và là đường
//! chuẩn của PCIe. Chọn ECAM; ghi rõ ở đây và `docs/k6-virtio-blk.md`.
//!
//! # Không có firmware enumerate ⇒ driver tự gán BAR
//!
//! OpenSBI **không** enumerate PCI (đo được: `info pci` của QEMU trong lúc boot cho
//! `BAR0/BAR1/BAR4: (not mapped)`), nên driver phải: ghi `0xffff_ffff` vào từng BAR ⇒ đọc mask
//! (quy tắc PCI §6.2.5.1) ⇒ gán địa chỉ trong cửa sổ `pcie-mmio` `0x4000_0000..0x7fff_ffff` ⇒
//! bật `command.MEMORY|BUS_MASTER`. Logic mask/cap ở `vifast-virtio` (test host); ở đây chỉ MMIO.
//!
//! `unsafe` (MMIO) nằm trong danh sách đóng `kernel/vifast-plat-*` của D2-17. Không lệnh đặc
//! quyền nào chạy ở U-mode: driver chỉ chạy kernel-mode.

use core::ptr::{read_volatile, write_volatile};

use vifast_virtio::pci::{
    bar_probe, first_conflict, parse_caps, read_ids, BarKind, PciCfg, PciError, VirtioCaps,
    VirtioKind, BAR_COUNT, CFG_BAR0, CFG_COMMAND, CFG_VENDOR_ID, COMMAND_BUS_MASTER,
    COMMAND_MEMORY,
};

use crate::mmu::{
    PLIC_WINDOW_BYTES, PLIC_WINDOW_HIGH_BASE, PLIC_WINDOW_LOW_BASE, RAM_BYTES, RAM_PHYS_BASE,
};

/// Base ECAM của QEMU `virt` (dữ kiện nền tảng, D2-72 Q7): `info mtree` ⇒
/// `alias pcie-ecam @pcie-mmcfg-mmio`, 256 MiB (bus<<20).
///
/// **KT-81 (Agent 0 phiếu lượt 5 §A5b)**: **dẫn xuất** từ [`vifast_plat_common::resources::PCI_ECAM`]
/// — trước đây là số viết tay thứ hai, có thể lệch với bảng `REGIONS` mà không ai thấy.
pub const ECAM_BASE: u64 = vifast_plat_common::resources::PCI_ECAM.base;
/// Cỡ ECAM driver **map** (2 MiB): đủ bus 0 và 1 (mỗi function 4 KiB; bus 1 MiB).
pub const ECAM_MAP_BYTES: u64 = 2 << 20;
/// Cửa sổ MMIO PCIe của QEMU `virt` (`alias pcie-mmio @gpex_mmio_window`, 1 GiB).
///
/// **KT-81 (§A5b)**: dẫn xuất từ [`vifast_plat_common::resources::PCI_MMIO32`].
pub const PCI_MMIO_WINDOW_BASE: u64 = vifast_plat_common::resources::PCI_MMIO32.base;
/// Cỡ cửa sổ MMIO PCIe (1 GiB — dùng để kiểm BAR gán ra nằm trong cửa sổ).
///
/// **KT-81 (§A5b)**: dẫn xuất từ [`vifast_plat_common::resources::PCI_MMIO32`].
pub const PCI_MMIO_WINDOW_BYTES: u64 = vifast_plat_common::resources::PCI_MMIO32.size;
/// Cỡ leaf 2 MiB dùng để map ECAM + BAR (đủ cho một BAR modern 0x4000).
pub const PCI_MAP_LEAF_BYTES: u64 = 2 << 20;

// **KT-81 (§A5b)** — phép kiểm **khớp bảng** ngay lúc biên dịch: driver map ECAM 2 MiB và dùng
// toàn bộ cửa sổ MMIO 32-bit; nếu ai đó thu nhỏ vùng trong `REGIONS` mà không sửa driver (hoặc
// ngược lại), ảnh **không biên dịch được** thay vì map ra ngoài vùng đã khai.
const _: () = assert!(ECAM_MAP_BYTES <= vifast_plat_common::resources::PCI_ECAM.size);
const _: () = assert!(PCI_MMIO_WINDOW_BYTES == vifast_plat_common::resources::PCI_MMIO32.size);
const _: () = assert!(PCI_MMIO_WINDOW_BASE == vifast_plat_common::resources::PCI_MMIO32.base);
/// Địa chỉ driver **gán** cho BAR modern của virtio-blk (đầu cửa sổ, căn 2 MiB) — KT-72.
pub const BAR_ASSIGN_BASE: u64 = PCI_MMIO_WINDOW_BASE;
/// Địa chỉ driver **gán** cho BAR modern của virtio-console — KT-73: **khác** base của blk để hai
/// driver (nếu cùng có mặt) không tranh một trang; vẫn trong cửa sổ PCIe và qua `first_conflict`.
pub const BAR_ASSIGN_BASE_CONSOLE: u64 = PCI_MMIO_WINDOW_BASE + PCI_MAP_LEAF_BYTES;
/// Địa chỉ driver **gán** cho BAR modern của virtio-net — **KT-245**: leaf thứ **ba** của cửa sổ
/// PCIe. Giữ nguyên leaf 1 cho console (ảnh `ktest-con` đã đo `assigned=0x40200000`) và leaf 0 cho
/// blk; mỗi loại một leaf ⇒ hai driver cùng có mặt không tranh một trang.
pub const BAR_ASSIGN_BASE_NET: u64 = PCI_MMIO_WINDOW_BASE + 2 * PCI_MAP_LEAF_BYTES;
/// Địa chỉ driver **gán** cho BAR modern của virtio-input — **KT-255**: leaf thứ **tư** của cửa sổ
/// PCIe. Giữ nguyên leaf 0 (blk), 1 (console), 2 (net) ⇒ ảnh `ktest-vblk`/`ktest-con`/`ktest-net`
/// đã đo (`assigned=0x40000000`/`0x40200000`/`0x40400000`) **không đổi**; mỗi loại một leaf ⇒ hai
/// driver cùng có mặt không tranh một trang.
pub const BAR_ASSIGN_BASE_INPUT: u64 = PCI_MMIO_WINDOW_BASE + 3 * PCI_MAP_LEAF_BYTES;
/// Địa chỉ driver **gán** cho BAR modern của virtio-gpu — **KT-263**: leaf thứ **năm** của cửa sổ
/// PCIe. Giữ nguyên leaf 0 (blk), 1 (console), 2 (net), 3 (input) ⇒ ảnh `ktest-vblk`/`ktest-con`/
/// `ktest-net`/`ktest-input` đã đo (`assigned=0x40000000`/`0x40200000`/`0x40400000`/`0x40600000`)
/// **không đổi**; mỗi loại một leaf ⇒ hai driver cùng có mặt không tranh một trang.
pub const BAR_ASSIGN_BASE_GPU: u64 = PCI_MMIO_WINDOW_BASE + 4 * PCI_MAP_LEAF_BYTES;
/// Số leaf 2 MiB của cửa sổ PCIe mà driver **identity-map** (`mmu::map_mmio`).
///
/// **KT-245**: cửa sổ phải **liền một dải** từ [`PCI_MMIO_WINDOW_BASE`] vì `mmu_range_conflict`
/// kiểm "leaf đã map có nằm trong cửa sổ của chính driver" bằng một khoảng duy nhất. Ảnh net gán
/// BAR ở leaf **2** ⇒ phải map **3** leaf (leaf 1 chỉ để trống, không tốn PA). Ảnh blk (1 leaf) và
/// ảnh console (2 leaf) **không đổi** — số đo `assigned=0x40200000` của KT-73 giữ nguyên.
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
/// Số byte cửa sổ PCIe driver **identity-map** cho BAR của mình (`mmu::map_mmio`).
/// Hai hằng phải khớp nhau, nếu lệch thì `mmu_range_conflict` báo `BarConflict` (fail-closed) —
/// đúng ca KT-73 đã bắt được trước khi chạy.
pub const PCI_DRIVER_WINDOW_BYTES: u64 = PCI_DRIVER_LEAVES * PCI_MAP_LEAF_BYTES;

/// Số bus quét (chỉ bus 0 — `virt` không có cầu nối sau).
pub const SCAN_BUSES: u8 = 1;
/// Số device tối đa trên một bus.
pub const SCAN_DEVICES: u8 = 32;
/// Số function tối đa của **function khác** khi kiểm chồng lấn BAR (multi-function).
pub const SCAN_OTHER_FUNCTIONS: u8 = 8;
/// Số function của chính thiết bị đích (quét function 0; virtio-blk là single-function).
pub const SCAN_FUNCTIONS: u8 = 1;
/// Trần số BAR thu thập từ function khác trên bus 0 (QEMU virt: host bridge + virtio-blk).
pub const MAX_OTHER_BARS: usize = 8;

/// Vùng nền tảng **không được** gán BAR chồng lên (CAO-2 / D2-110C): UART0 4 KiB, hai cửa sổ
/// PLIC 2 MiB, ECAM 2 MiB driver map, và toàn bộ RAM `-m 128M`. Gán BAR vào đây = đè im lặng.
/// Đây là **dữ kiện nền tảng QEMU virt** (D2-72 Q7) lấy từ hằng của `mmu`, không phải số đo.
pub const RESERVED_RANGES: [(u64, u64); 5] = [
    // KT-81 (§A5b): cỡ UART0 lấy từ bảng `REGIONS`, không viết tay `0x1000` lần thứ hai.
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
        // SAFETY: địa chỉ nằm trong cửa sổ ECAM 2 MiB đã map Device RW ở `mmu::map_mmio`
        // (kiểm bằng `mmu::walk_sw` trong TAP); offset do header PCI quy định (< 256).
        unsafe { read_volatile(self.addr(off) as *const u8) }
    }
    /// Đọc 16 bit config space.
    #[inline]
    pub fn read16(&self, off: u8) -> u16 {
        // SAFETY: như `read8`; offset 16-bit căn 2 (đọc đúng thanh ghi PCI).
        unsafe { read_volatile(self.addr(off) as *const u16) }
    }
    /// Đọc 32 bit config space.
    #[inline]
    pub fn read32(&self, off: u8) -> u32 {
        // SAFETY: như `read8`; offset 32-bit căn 4.
        unsafe { read_volatile(self.addr(off) as *const u32) }
    }
    /// Ghi 32 bit config space (BAR probe/gán).
    #[inline]
    pub fn write32(&self, off: u8, value: u32) {
        // SAFETY: như `read32`; đây là thanh ghi cấu hình **của thiết bị**, không phải RAM.
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

/// Toàn bộ số đo của lần tìm thiết bị + gán BAR — **giá trị đọc lại từ thiết bị**, không hằng.
#[derive(Clone, Copy, Debug)]
pub struct PciReport {
    /// Số function có mặt (vendor ≠ 0xffff) đã thấy trên bus 0 trước khi dừng.
    pub functions_seen: u32,
    /// Bus của virtio-blk.
    pub bus: u8,
    /// Device.
    pub dev: u8,
    /// Function.
    pub func: u8,
    /// Vendor id đọc từ config space.
    pub vendor: u16,
    /// Device id đọc từ config space.
    pub device: u16,
    /// Chỉ số BAR mà driver dùng (modern, có đủ 4 capability).
    pub bar_index: u8,
    /// Loại BAR.
    pub bar_kind: BarKind,
    /// Cỡ BAR đo từ mask.
    pub bar_size: u64,
    /// Địa chỉ driver **gán** cho BAR.
    pub bar_assigned: u64,
    /// Giá trị **đọc lại** từ thanh ghi BAR sau khi gán (phải bằng `bar_assigned`).
    pub bar_readback: u64,
    /// `command` đọc lại sau khi bật memory + bus master.
    pub command: u16,
    /// Capability đọc từ chain của thiết bị.
    pub caps: VirtioCaps,
    /// Địa chỉ ECAM của function (bằng chứng map đúng).
    pub ecam_addr: u64,
    /// Số BAR **có địa chỉ** của function khác đã đối chiếu chồng lấn.
    pub other_bars: u16,
    /// Số vùng nền tảng đã đối chiếu.
    pub reserved_ranges: u16,
    /// `true` ⇔ đã kiểm chồng lấn (BAR khác + vùng nền tảng + page table) trước khi gán.
    pub overlap_checked: bool,
    /// `true` ⇔ mọi capability có `offset + length <= bar_size`.
    pub cap_ranges_ok: bool,
}

/// `true` ⇔ có trang nào trong `[assigned, assigned+size)` **đã map** mà **không** phải leaf
/// identity của chính cửa sổ PCIe driver map (`mmu::map_mmio`) — tức gán BAR vào đó sẽ đè lên
/// mapping khác (image/RAM/ECAM/PLIC…). Trang **chưa map** không tính là xung đột (chưa có gì để
/// đè); lớp kiểm còn lại là `RESERVED_RANGES` + BAR function khác.
fn mmu_range_conflict(assigned: u64, size: u64) -> bool {
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

/// Thu thập `(base, size)` các BAR **memory đã có địa chỉ** của mọi function khác trên bus 0.
///
/// Ghi `0xffff_ffff` để đo cỡ rồi **khôi phục nguyên giá trị cũ** (không phá cấu hình thiết bị
/// khác). BAR chưa gán địa chỉ (0) không thể chồng lấn nên bỏ qua.
fn collect_other_bars(target: Ecam) -> ([(u64, u64); MAX_OTHER_BARS], usize, u32) {
    let mut bars = [(0u64, 0u64); MAX_OTHER_BARS];
    let mut count = 0usize;
    let mut seen = 0u32;
    for dev in 0..SCAN_DEVICES {
        for func in 0..SCAN_OTHER_FUNCTIONS {
            let other = Ecam {
                bus: target.bus,
                dev,
                func,
            };
            if other.read16(CFG_VENDOR_ID) == 0xffff {
                continue;
            }
            seen += 1;
            if dev == target.dev && func == target.func {
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
    (bars, count, seen)
}

/// Lỗi khởi tạo PCI kèm **chặng** (để TAP in lý do đo được — fail-closed, không PANIC im lặng).
#[derive(Clone, Copy, Debug)]
pub struct PciFail {
    /// Mã lỗi.
    pub error: PciError,
    /// Chặng: `nodevice`/`probing`/`chosen`/`size`/`caps`/`capbar`/`assign`/`readback`/`command`.
    pub stage: &'static str,
}

impl PciFail {
    const fn at(error: PciError, stage: &'static str) -> Self {
        Self { error, stage }
    }
}

/// Quét bus/device, tìm **virtio-blk**, gán BAR và đọc capability (KT-72; giữ API cũ).
///
/// Trả lỗi [`PciFail`] (không PANIC) để ảnh test in được lý do đo được; TAP sẽ đỏ.
pub fn init() -> Result<PciReport, PciFail> {
    init_for(VirtioKind::Blk)
}

/// Quét bus/device cho **loại thiết bị** `kind`, gán BAR và đọc capability.
///
/// Một đường quét duy nhất cho mọi thiết bị virtio (KT-72 blk, KT-73 console): không nhân đôi
/// logic ECAM/BAR/cap (D2-110A). Chỉ khác **địa chỉ gán BAR** (mỗi loại một trang 2 MiB trong
/// cửa sổ PCIe) — vẫn qua `first_conflict` + kiểm leaf page table như nhau.
pub fn init_for(kind: VirtioKind) -> Result<PciReport, PciFail> {
    let mut functions_seen = 0u32;
    let mut found: Option<(u8, u8, u8)> = None;
    for bus in 0..SCAN_BUSES {
        for dev in 0..SCAN_DEVICES {
            for func in 0..SCAN_FUNCTIONS {
                let ecam = Ecam { bus, dev, func };
                if ecam.read16(CFG_VENDOR_ID) == 0xffff {
                    continue;
                }
                functions_seen += 1;
                let (vendor, device) = read_ids(&ecam);
                if kind.matches(vendor, device) {
                    found = Some((bus, dev, func));
                    break;
                }
            }
            if found.is_some() {
                break;
            }
        }
        if found.is_some() {
            break;
        }
    }
    let (bus, dev, func) = found.ok_or(PciFail::at(PciError::NoDevice, "nodevice"))?;
    let mut ecam = Ecam { bus, dev, func };
    let (vendor, device) = read_ids(&ecam);

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
    if !(0x1000..=PCI_MAP_LEAF_BYTES).contains(&bar_size) {
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
    // Gán BAR: một trang 2 MiB riêng cho từng loại thiết bị (blk ở đầu cửa sổ, console ở trang
    // kế) — căn 2 MiB > mọi cỡ BAR hợp lệ ở đây.
    let assigned = match kind {
        VirtioKind::Blk => BAR_ASSIGN_BASE,
        VirtioKind::Console => BAR_ASSIGN_BASE_CONSOLE,
        VirtioKind::Net => BAR_ASSIGN_BASE_NET,
        VirtioKind::Input => BAR_ASSIGN_BASE_INPUT,
        VirtioKind::Gpu => BAR_ASSIGN_BASE_GPU,
    };
    if assigned < PCI_MMIO_WINDOW_BASE
        || assigned + bar_size > PCI_MMIO_WINDOW_BASE + PCI_MMIO_WINDOW_BYTES
    {
        return Err(PciFail::at(PciError::BarUnassigned, "assign"));
    }
    // ---- CAO-2 (D2-110C): **kiểm chồng lấn TRƯỚC khi ghi BAR**; chồng lấn ⇒ fail-closed.
    // (i) BAR memory đã có địa chỉ của mọi function khác trên bus 0.
    let (other_bars, other_count, other_seen) = collect_other_bars(ecam);
    if let Some(kind) = first_conflict(
        assigned,
        bar_size,
        &other_bars[..other_count],
        &RESERVED_RANGES,
    ) {
        return Err(PciFail::at(PciError::BarConflict, kind));
    }
    // (ii) trang đã map bởi mapping khác (không phải cửa sổ PCIe của chính driver).
    if mmu_range_conflict(assigned, bar_size) {
        return Err(PciFail::at(PciError::BarConflict, "mmu"));
    }
    // (iii) ghi BAR + đọc lại: chỉ **sau** khi (i)/(ii) sạch.
    let _ = other_seen;
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
        bar_index,
        bar_kind,
        bar_size,
        bar_assigned: assigned,
        bar_readback: readback,
        command,
        caps,
        ecam_addr: ecam.addr(CFG_VENDOR_ID),
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
