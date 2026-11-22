// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Transport PCI dùng chung** cho thiết bị virtio — phần **thuần** (KT-334, D2-231/D2-239).
//!
//! # Vì sao có tệp này
//!
//! `pci.rs` của ba `vifast-plat-{riscv64,aarch64,x86_64}` chứa **cùng một** quy trình: quét
//! bus/device/function, đo cỡ BAR bằng mask (PCI §6.2.5.1), đọc bốn capability của virtio-pci,
//! kiểm `offset+length <= bar_size`, gán (hoặc kiểm BAR firmware đã gán), bật `command` memory +
//! bus-master, đọc lại để chứng minh. Ba bản sao đó đã lệch nhau ở **chỗ then chốt** trong quá
//! khứ (thiếu kiểm `status` capability, so sánh readback chưa che 4 bit thuộc tính, …).
//!
//! Tệp này giữ phần **quyết định**; mỗi platform chỉ còn:
//!
//! 1. cài [`PciBus`] (địa chỉ ECAM/CF8 của mình — phần chạm phần cứng),
//! 2. khai [`PciPolicy`] (dải quét, BAR gán ở đâu, vùng nền tảng nào bị cấm),
//! 3. cấu trúc lại [`PciReport`] cho TAP.
//!
//! # Chế độ gán BAR
//!
//! - [`discover`] — **driver tự gán** BAR (riscv64, aarch64): đo cỡ → chọn BAR memory lớn nhất →
//!   kiểm chồng lấn (BAR function khác + vùng nền tảng + kiểm riêng của platform) → ghi → đọc lại.
//! - [`probe_assigned`] — **firmware đã gán** BAR (x86_64/SeaBIOS): chỉ đọc, kiểm BAR nằm trong
//!   cửa sổ MMIO đã map, **không** ghi đè cấu hình firmware.
//!
//! Cả hai **không PANIC**: lỗi trả [`PciFail`] kèm `stage` để TAP in ra chặng đo được.

use crate::pci::{
    bar_probe, first_conflict, parse_caps, range_overlaps, read_ids, Bar, BarKind, PciCfg,
    PciError, VirtioCaps, VirtioKind, BAR_COUNT, CFG_BAR0, CFG_COMMAND, CFG_HEADER_TYPE,
    CFG_VENDOR_ID, COMMAND_BUS_MASTER, COMMAND_MEMORY,
};

/// Offset `revision_id` trong header kiểu 0.
pub const CFG_REVISION: u8 = 0x08;
/// Offset `prog_if` (class code byte thấp).
pub const CFG_PROG_IF: u8 = 0x09;
/// Offset `subclass`.
pub const CFG_SUBCLASS: u8 = 0x0a;
/// Offset `class_code` (byte cao của class 24 bit).
pub const CFG_CLASS: u8 = 0x0b;

/// Cỡ BAR nhỏ nhất driver chấp nhận (dưới nữa là BAR hỏng/rác).
pub const BAR_MIN_BYTES: u64 = 0x1000;

/// Lỗi khởi tạo PCI kèm **chặng** — fail-closed, in được lý do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PciFail {
    /// Mã lỗi.
    pub error: PciError,
    /// Chặng: `nodevice`/`chosen`/`size`/`caps`/`capbar`/`caprange`/`assign`/`conflict`/`mmu`/
    /// `bar-index`/`bar-io`/`bar-zero`/`bar-size`/`bar-unmapped`/`readback`/`command`.
    pub stage: &'static str,
}

impl PciFail {
    /// Dựng lỗi tại một chặng.
    pub const fn at(error: PciError, stage: &'static str) -> Self {
        Self { error, stage }
    }

    /// Chặng (tiện cho TAP in `stage=`).
    pub const fn stage(self) -> &'static str {
        self.stage
    }
}

/// Toàn bộ số đo của lần tìm thiết bị + gán/kiểm BAR — **giá trị đọc lại từ thiết bị**, không hằng.
///
/// Đây là **hợp** các trường mà ba platform từng khai riêng; trường không áp dụng (ví dụ
/// `ecam_pa` trên x86_64) để 0 — fail-closed: 0 không thể bị đọc nhầm thành địa chỉ hợp lệ.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PciReport {
    /// Đường truy cập (mã do platform quy ước; x86_64: ECAM/legacy).
    pub access: u8,
    /// Gốc ECAM (0 nếu dùng CF8/legacy).
    pub ecam_base: u64,
    /// Số function **có mặt** đã thấy trước khi dừng quét.
    pub functions_seen: u32,
    /// Bus của thiết bị.
    pub bus: u8,
    /// Device.
    pub dev: u8,
    /// Function.
    pub func: u8,
    /// Vendor id.
    pub vendor: u16,
    /// Device id.
    pub device: u16,
    /// Class code 24 bit.
    pub class: u32,
    /// Revision id.
    pub revision: u8,
    /// Subclass.
    pub subclass: u8,
    /// Programming interface.
    pub prog_if: u8,
    /// `header_type`.
    pub header_type: u8,
    /// Chỉ số BAR chứa capability.
    pub bar_index: u8,
    /// Loại BAR.
    pub bar_kind: BarKind,
    /// Cỡ BAR đo từ mask.
    pub bar_size: u64,
    /// Địa chỉ BAR dùng để truy cập (driver gán **hoặc** firmware đã gán).
    pub bar_base: u64,
    /// Địa chỉ driver **gán** (bằng `bar_base` với chế độ firmware).
    pub bar_assigned: u64,
    /// Giá trị **đọc lại** sau khi gán (bằng `bar_base` với chế độ firmware).
    pub bar_readback: u64,
    /// `command` đọc lại sau khi bật memory + bus master.
    pub command: u16,
    /// Bốn capability + `notify_off_multiplier`.
    pub caps: VirtioCaps,
    /// Địa chỉ ECAM của function (0 nếu không áp dụng).
    pub ecam_addr: u64,
    /// Địa chỉ **PA** của ECAM (0 nếu không áp dụng).
    pub ecam_pa: u64,
    /// Địa chỉ **VA** của ECAM (0 nếu không áp dụng).
    pub ecam_va: u64,
    /// Số BAR **có địa chỉ** của function khác đã đối chiếu chồng lấn.
    pub other_bars: u16,
    /// Số vùng nền tảng đã đối chiếu.
    pub reserved_ranges: u16,
    /// `true` ⇔ đã kiểm chồng lấn trước khi gán.
    pub overlap_checked: bool,
    /// `true` ⇔ mọi capability có `offset + length <= bar_size`.
    pub cap_ranges_ok: bool,
}

/// Dải quét bus/device/function của một platform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScanRange {
    /// Số bus (aarch64 suy từ cửa sổ ECAM đã map; riscv64 = 1).
    pub buses: u16,
    /// Số device mỗi bus.
    pub devices: u8,
    /// Số function mỗi device.
    pub functions: u8,
}

impl ScanRange {
    /// Dải quét tường minh.
    pub const fn new(buses: u16, devices: u8, functions: u8) -> Self {
        Self {
            buses,
            devices,
            functions,
        }
    }
}

/// Truy cập config space PCI **theo địa chỉ (bus, device, function)** của một platform.
///
/// `Cfg` là kiểu `PciCfg` của platform (`Ecam` trên riscv64/aarch64, `Function` trên x86_64).
/// Nhờ trait này, vòng quét và phép đo BAR nằm **một chỗ** trong crate thuần.
pub trait PciBus {
    /// Kiểu truy cập config space của một function.
    type Cfg<'a>: PciCfg
    where
        Self: 'a;

    /// Truy cập config space của `(bus, dev, func)`.
    fn cfg(&mut self, bus: u8, dev: u8, func: u8) -> Self::Cfg<'_>;

    /// Địa chỉ bằng chứng của function: `(ecam_addr, ecam_pa, ecam_va)`.
    ///
    /// Mặc định `(0, 0, 0)` cho platform không dùng ECAM (hoặc không muốn in).
    fn evidence(&self, _bus: u8, _dev: u8, _func: u8) -> (u64, u64, u64) {
        (0, 0, 0)
    }
}

/// Chính sách gán BAR khi **driver tự gán** (riscv64, aarch64).
#[derive(Clone, Copy, Debug)]
pub struct DriverAssign {
    /// Địa chỉ gán cho từng loại thiết bị (mỗi loại một leaf).
    pub base: fn(VirtioKind) -> u64,
    /// Base cửa sổ MMIO 32-bit đã map.
    pub window_base: u64,
    /// Cỡ cửa sổ MMIO 32-bit.
    pub window_bytes: u64,
    /// Cỡ BAR lớn nhất chấp nhận.
    pub max_bar: u64,
}

/// Chính sách tìm thiết bị của một platform.
#[derive(Clone, Copy)]
pub struct PciPolicy {
    /// Loại thiết bị virtio cần tìm.
    pub kind: VirtioKind,
    /// Dải quét.
    pub scan: ScanRange,
    /// Cách gán/kiểm BAR.
    pub assign: DriverAssign,
    /// Vùng nền tảng **cấm** gán BAR vào (base, size).
    pub reserved: &'static [(u64, u64)],
    /// Kiểm chồng lấn riêng của platform (ví dụ leaf page table của riscv64).
    pub extra_conflict: Option<fn(u64, u64) -> bool>,
}

/// Đo một BAR **đang có** của function rồi **khôi phục nguyên giá trị cũ**.
///
/// Trả `(base, Bar)`; `None` nếu BAR I/O, mask rác, hoặc cỡ không phải luỹ thừa 2 (không đoán).
/// Việc khôi phục giá trị cũ là bắt buộc với chế độ firmware (x86_64): driver **không** được
/// phá cấu hình SeaBIOS đã gán khi chỉ đi đo.
pub fn probe_bar(cfg: &mut impl PciCfg, index: u8) -> Option<(u64, Bar)> {
    if index as usize >= BAR_COUNT {
        return None;
    }
    let off = CFG_BAR0 + index * 4;
    let raw_low = cfg.read32(off);
    if raw_low & 1 != 0 {
        return None;
    }
    let is_64 = raw_low & 0b110 == 0b100;
    let raw_high = if is_64 { cfg.read32(off + 4) } else { 0 };
    let base = ((raw_low as u64) & !0xf) | ((raw_high as u64) << 32);
    cfg.write32(off, 0xffff_ffff);
    let mask_low = cfg.read32(off);
    let mask_high = if is_64 {
        cfg.write32(off + 4, 0xffff_ffff);
        let high = cfg.read32(off + 4);
        cfg.write32(off + 4, raw_high);
        high
    } else {
        0
    };
    cfg.write32(off, raw_low);
    let bar = bar_probe(index, mask_low, mask_high)?;
    Some((base, bar))
}

/// Chọn BAR memory **lớn nhất** của function (modern virtio-pci: BAR4 64-bit).
pub fn choose_bar(cfg: &mut impl PciCfg) -> Option<(u8, BarKind, u64)> {
    let mut chosen: Option<(u8, BarKind, u64)> = None;
    let mut index = 0u8;
    while (index as usize) < BAR_COUNT {
        let off = CFG_BAR0 + index * 4;
        let raw = cfg.read32(off);
        let is_io = raw & 1 != 0;
        let is_64 = !is_io && raw & 0b110 == 0b100;
        if !is_io {
            if let Some((_, bar)) = probe_bar(cfg, index) {
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
    chosen
}

/// `true` ⇔ mọi capability nằm **trọn** trong BAR (`offset + length <= bar_size`).
pub fn caps_fit(caps: &VirtioCaps, bar_size: u64) -> bool {
    let fits = |cap: &crate::pci::VirtioCap| cap.offset as u64 + cap.length as u64 <= bar_size;
    fits(&caps.common) && fits(&caps.notify) && fits(&caps.device) && fits(&caps.isr)
}

/// Bật decode memory + bus master (DMA) rồi đọc lại — virtio cần **cả hai**.
fn enable_command(cfg: &mut impl PciCfg) -> Result<u16, PciFail> {
    let command = cfg.read16(CFG_COMMAND) | COMMAND_MEMORY | COMMAND_BUS_MASTER;
    cfg.write16(CFG_COMMAND, command);
    let command = cfg.read16(CFG_COMMAND);
    if command & (COMMAND_MEMORY | COMMAND_BUS_MASTER) != COMMAND_MEMORY | COMMAND_BUS_MASTER {
        return Err(PciFail::at(PciError::BarUnassigned, "command"));
    }
    Ok(command)
}

/// Ghi BAR `index` = `assigned` rồi đọc lại; so sánh trên **địa chỉ** (che 4 bit thuộc tính).
fn assign_bar(
    cfg: &mut impl PciCfg,
    index: u8,
    kind: BarKind,
    assigned: u64,
) -> Result<u64, PciFail> {
    let off = CFG_BAR0 + index * 4;
    cfg.write32(off, assigned as u32);
    if kind == BarKind::Memory64 {
        cfg.write32(off + 4, (assigned >> 32) as u32);
    }
    // Thanh ghi BAR giữ 4 bit thuộc tính (prefetchable/kind/space) ở lần đọc lại — địa chỉ thật
    // là phần đã che 4 bit thấp (PCI §6.2.5.1). So sánh trên địa chỉ, không trên raw.
    let attr_mask = 0xfu64;
    let readback = ((cfg.read32(off) as u64) & !attr_mask)
        | if kind == BarKind::Memory64 {
            (cfg.read32(off + 4) as u64) << 32
        } else {
            0
        };
    if readback != assigned {
        return Err(PciFail::at(PciError::BarUnassigned, "readback"));
    }
    Ok(readback)
}

/// Quét **mọi function khác** trong dải quét, đo BAR memory đã có địa chỉ và kiểm chồng lấn với
/// `[assigned, assigned+size)`.
///
/// Trả số BAR đã đối chiếu. Ghi mask rồi **khôi phục** giá trị cũ (xem [`probe_bar`]) — không phá
/// cấu hình thiết bị khác. Fail-closed: chồng lấn ⇒ `BarConflict`.
fn conflict_with_other_bars<B: PciBus>(
    link: &mut B,
    scan: ScanRange,
    target: (u8, u8, u8),
    assigned: u64,
    size: u64,
    reserved: &[(u64, u64)],
) -> Result<u16, PciFail> {
    let mut count = 0u16;
    for bus in 0..scan.buses {
        // Dải quét do `ScanRange` chặn; ép sau khi biên do vòng lặp bảo đảm.
        let bus = bus as u8;
        for dev in 0..scan.devices {
            for func in 0..scan.functions {
                if (bus, dev, func) == target {
                    continue;
                }
                let mut cfg = link.cfg(bus, dev, func);
                if cfg.read16(CFG_VENDOR_ID) == 0xffff {
                    continue;
                }
                let mut index = 0u8;
                while (index as usize) < BAR_COUNT {
                    let off = CFG_BAR0 + index * 4;
                    let raw = cfg.read32(off);
                    let is_io = raw & 1 != 0;
                    let is_64 = !is_io && raw & 0b110 == 0b100;
                    if !is_io && raw & !0xf != 0 {
                        if let Some((base, bar)) = probe_bar(&mut cfg, index) {
                            if base != 0 {
                                count = count.saturating_add(1);
                                if range_overlaps(assigned, size, base, bar.size) {
                                    return Err(PciFail::at(PciError::BarConflict, "other_bar"));
                                }
                            }
                        }
                    }
                    index += if is_64 { 2 } else { 1 };
                }
            }
        }
    }
    if let Some(name) = first_conflict(assigned, size, &[], reserved) {
        return Err(PciFail::at(PciError::BarConflict, name));
    }
    Ok(count)
}

/// Quét dải, tìm thiết bị `policy.kind`, **tự gán** BAR rồi đọc capability.
pub fn discover<B: PciBus>(link: &mut B, policy: &PciPolicy) -> Result<PciReport, PciFail> {
    let mut functions_seen = 0u32;
    let mut found: Option<(u8, u8, u8)> = None;
    'scan: for bus in 0..policy.scan.buses {
        // Dải quét đã bị `ScanRange` chặn ở `u16`; ép sau khi biên do vòng lặp bảo đảm.
        let bus = bus as u8;
        for dev in 0..policy.scan.devices {
            for func in 0..policy.scan.functions {
                let cfg = link.cfg(bus, dev, func);
                if cfg.read16(CFG_VENDOR_ID) == 0xffff {
                    continue;
                }
                functions_seen += 1;
                let (vendor, device) = read_ids(&cfg);
                if policy.kind.matches(vendor, device) {
                    found = Some((bus, dev, func));
                    break 'scan;
                }
            }
        }
    }
    let (bus, dev, func) = found.ok_or(PciFail::at(PciError::NoDevice, "nodevice"))?;
    // Giai đoạn 1: đọc nhận dạng + chọn BAR + capability (chỉ cần `cfg`, không cần `link`).
    let facts = {
        let mut cfg = link.cfg(bus, dev, func);
        let (vendor, device) = read_ids(&cfg);
        let revision = cfg.read8(CFG_REVISION);
        let prog_if = cfg.read8(CFG_PROG_IF);
        let subclass = cfg.read8(CFG_SUBCLASS);
        let class =
            ((cfg.read8(CFG_CLASS) as u32) << 16) | ((subclass as u32) << 8) | prog_if as u32;
        let header_type = cfg.read8(CFG_HEADER_TYPE);
        let (bar_index, bar_kind, bar_size) =
            choose_bar(&mut cfg).ok_or(PciFail::at(PciError::BarUnassigned, "chosen"))?;
        if !(BAR_MIN_BYTES..=policy.assign.max_bar).contains(&bar_size) {
            return Err(PciFail::at(PciError::BarUnassigned, "size"));
        }
        // Capability **trước** khi gán: cần biết BAR chứa common/notify/device/isr có đúng BAR này.
        let caps = parse_caps(&cfg).map_err(|error| PciFail::at(error, "caps"))?;
        if caps.bar != bar_index || caps.common.bar != bar_index {
            return Err(PciFail::at(PciError::CapMissing, "capbar"));
        }
        let cap_ranges_ok = caps_fit(&caps, bar_size);
        if !cap_ranges_ok {
            return Err(PciFail::at(PciError::CapMissing, "caprange"));
        }
        (
            vendor,
            device,
            class,
            revision,
            subclass,
            prog_if,
            header_type,
            bar_index,
            bar_kind,
            bar_size,
            caps,
        )
    };
    let (
        vendor,
        device,
        class,
        revision,
        subclass,
        prog_if,
        header_type,
        bar_index,
        bar_kind,
        bar_size,
        caps,
    ) = facts;
    let cap_ranges_ok = caps_fit(&caps, bar_size);
    let assigned = (policy.assign.base)(policy.kind);
    if assigned < policy.assign.window_base
        || assigned + bar_size > policy.assign.window_base + policy.assign.window_bytes
    {
        return Err(PciFail::at(PciError::BarUnassigned, "assign"));
    }
    // CAO-2 (D2-110C): kiểm chồng lấn **trước** khi ghi BAR; chồng lấn ⇒ fail-closed.
    let other_bars = conflict_with_other_bars(
        link,
        policy.scan,
        (bus, dev, func),
        assigned,
        bar_size,
        policy.reserved,
    )?;
    if let Some(conflict) = policy.extra_conflict {
        if conflict(assigned, bar_size) {
            return Err(PciFail::at(PciError::BarConflict, "mmu"));
        }
    }
    // Giai đoạn 2: ghi BAR + đọc lại + bật command (cần `cfg` mới, `link` đã hết mượn).
    let (readback, command) = {
        let mut cfg = link.cfg(bus, dev, func);
        let readback = assign_bar(&mut cfg, bar_index, bar_kind, assigned)?;
        let command = enable_command(&mut cfg)?;
        (readback, command)
    };
    let (ecam_addr, ecam_pa, ecam_va) = link.evidence(bus, dev, func);
    Ok(PciReport {
        access: 0,
        ecam_base: 0,
        functions_seen,
        bus,
        dev,
        func,
        vendor,
        device,
        class,
        revision,
        subclass,
        prog_if,
        header_type,
        bar_index,
        bar_kind,
        bar_size,
        bar_base: assigned,
        bar_assigned: assigned,
        bar_readback: readback,
        command,
        caps,
        ecam_addr,
        ecam_pa,
        ecam_va,
        other_bars,
        reserved_ranges: policy.reserved.len() as u16,
        overlap_checked: true,
        cap_ranges_ok,
    })
}

/// Tìm thiết bị `kind` trong một function **đã biết**, kiểm BAR **firmware đã gán** và đọc
/// capability (x86_64/SeaBIOS — **không** ghi đè địa chỉ firmware).
pub fn probe_assigned<C: PciCfg>(
    cfg: &mut C,
    kind: VirtioKind,
    mapped: fn(u64, u64) -> bool,
    max_bar: u64,
) -> Result<PciReport, PciFail> {
    let (vendor, device) = read_ids(cfg);
    if !kind.matches(vendor, device) {
        return Err(PciFail::at(PciError::NoDevice, "nodevice"));
    }
    let revision = cfg.read8(CFG_REVISION);
    let prog_if = cfg.read8(CFG_PROG_IF);
    let subclass = cfg.read8(CFG_SUBCLASS);
    let class = ((cfg.read8(CFG_CLASS) as u32) << 16) | ((subclass as u32) << 8) | prog_if as u32;
    let header_type = cfg.read8(CFG_HEADER_TYPE);
    let caps = parse_caps(cfg).map_err(|error| PciFail::at(error, "caps"))?;
    let bar_index = caps.bar;
    if bar_index as usize >= BAR_COUNT {
        return Err(PciFail::at(PciError::BarUnassigned, "bar-index"));
    }
    let off = CFG_BAR0 + bar_index * 4;
    let raw_low = cfg.read32(off);
    if raw_low & 1 != 0 {
        // BAR không gian I/O: virtio-pci modern dùng BAR memory — không đoán.
        return Err(PciFail::at(PciError::BarUnassigned, "bar-io"));
    }
    let is_64 = raw_low & 0b110 == 0b100;
    let raw_high = if is_64 { cfg.read32(off + 4) } else { 0 };
    let bar_base = ((raw_low as u64) & !0xf) | ((raw_high as u64) << 32);
    if bar_base == 0 {
        return Err(PciFail::at(PciError::BarUnassigned, "bar-zero"));
    }
    let (_, bar) =
        probe_bar(cfg, bar_index).ok_or(PciFail::at(PciError::BarUnassigned, "bar-size"))?;
    if !(BAR_MIN_BYTES..=max_bar).contains(&bar.size) {
        return Err(PciFail::at(PciError::BarUnassigned, "bar-size"));
    }
    if !mapped(bar_base, bar.size) {
        return Err(PciFail::at(PciError::BarUnassigned, "bar-unmapped"));
    }
    let cap_ranges_ok = caps_fit(&caps, bar.size);
    if !cap_ranges_ok {
        return Err(PciFail::at(PciError::CapMissing, "caprange"));
    }
    let command = enable_command(cfg)?;
    Ok(PciReport {
        access: 0,
        ecam_base: 0,
        functions_seen: 0,
        bus: 0,
        dev: 0,
        func: 0,
        vendor,
        device,
        class,
        revision,
        subclass,
        prog_if,
        header_type,
        bar_index,
        bar_kind: bar.kind,
        bar_size: bar.size,
        bar_base,
        bar_assigned: bar_base,
        bar_readback: bar_base,
        command,
        caps,
        ecam_addr: 0,
        ecam_pa: 0,
        ecam_va: 0,
        other_bars: 0,
        reserved_ranges: 0,
        overlap_checked: true,
        cap_ranges_ok,
    })
}

/// Tên kiểu BAR để in evidence (D2-102: in từ biến, không chép chuỗi trong test).
pub const fn bar_kind_name(kind: BarKind) -> &'static str {
    match kind {
        BarKind::Io => "io",
        BarKind::Memory32 => "mem32",
        BarKind::Memory64 => "mem64",
    }
}
