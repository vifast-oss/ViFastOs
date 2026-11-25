// SPDX-License-Identifier: MIT OR Apache-2.0
//! PCI configuration space + capability list của virtio-pci 1.0 — phần **thuần**.
//!
//! Nguồn: PCI Local Bus 3.0 §6 (header, BAR, cap list) + Virtio 1.0 §4.1.4 (`struct
//! virtio_pci_cap`, notify multiplier).
//!
//! Kernel cài [`PciCfg`] bằng ECAM (`kernel/vifast-plat-riscv64/src/pci.rs`); host test cài bằng
//! mảng 256 byte mô phỏng đúng quy tắc BAR của QEMU (ghi `0xffff_ffff` ⇒ đọc lại mask cỡ). Nhờ
//! vậy cùng một đoạn logic "mask → cỡ BAR" được kiểm trên host trước khi chạm phần cứng.

/// Vendor của mọi thiết bị virtio (Red Hat / QEMU).
pub const VENDOR_VIRTIO: u16 = 0x1af4;
/// `virtio-blk` bản transitional (legacy + modern) — **device id mà KT-72 nhắm**.
pub const VIRTIO_BLK_MODERN: u16 = 0x1001;
/// `virtio-blk` bản modern-only (`disable-legacy=on`), cùng driver.
pub const VIRTIO_BLK_MODERN_ONLY: u16 = 0x1042;

/// Offset `vendor_id` trong header kiểu 0.
pub const CFG_VENDOR_ID: u8 = 0x00;
/// Offset `device_id`.
pub const CFG_DEVICE_ID: u8 = 0x02;
/// Offset `command` (bit 1 = memory space, bit 2 = bus master).
pub const CFG_COMMAND: u8 = 0x04;
/// Offset `status` (bit 0 = cap list có).
pub const CFG_STATUS: u8 = 0x06;
/// Offset `header_type` (bit 7 = nhiều function).
pub const CFG_HEADER_TYPE: u8 = 0x0e;
/// Offset BAR0; BAR thứ `i` ở `CFG_BAR0 + 4*i`.
pub const CFG_BAR0: u8 = 0x10;
/// Offset capability pointer (chỉ hợp lệ khi `status & 0x10`).
pub const CFG_CAP_PTR: u8 = 0x34;
/// `command` bit 1 — bật decode memory space (không bật ⇒ đọc BAR ra 0xffff_ffff).
pub const COMMAND_MEMORY: u16 = 1 << 1;
/// `command` bit 2 — bật bus master (virtio DMA **cần**).
pub const COMMAND_BUS_MASTER: u16 = 1 << 2;
/// `status` bit 4 — có capability list.
pub const STATUS_CAP_LIST: u16 = 1 << 4;
/// Capability id cho vendor-specific (virtio dùng id này cho mọi cap).
pub const CAP_ID_VENDOR: u8 = 0x09;
/// Số BAR của header kiểu 0.
pub const BAR_COUNT: usize = 6;
/// Trần số vòng đi capability list (chống vòng lặp vô hạn trên thiết bị hỏng).
pub const CAP_WALK_LIMIT: usize = 64;

/// Lỗi đọc/giải mã config space. Không có biến thể "đoán": thiếu cap ⇒ lỗi.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PciError {
    /// Không thấy function virtio-blk nào trên bus đã quét (không PANIC: driver báo lỗi).
    NoDevice,
    /// Không có capability list (`status` thiếu bit 4) hoặc con trỏ 0.
    NoCapList,
    /// Đi quá [`CAP_WALK_LIMIT`] vòng — chain hỏng.
    CapWalkLimit,
    /// `next` trỏ về chính nó / về offset đã thăm — chain có vòng.
    CapLoop,
    /// Thiếu một trong bốn capability bắt buộc (common/notify/device/isr).
    CapMissing,
    /// BAR của capability chưa được gán địa chỉ (0) — chưa enumerate.
    BarUnassigned,
    /// Địa chỉ định gán **chồng lấn** BAR của function khác / vùng đã map / vùng nền tảng.
    BarConflict,
}

impl PciError {
    /// Tên để in evidence (D2-102: không in số trần).
    pub const fn name(self) -> &'static str {
        match self {
            PciError::NoDevice => "NoDevice",
            PciError::NoCapList => "NoCapList",
            PciError::CapWalkLimit => "CapWalkLimit",
            PciError::CapLoop => "CapLoop",
            PciError::CapMissing => "CapMissing",
            PciError::BarUnassigned => "BarUnassigned",
            PciError::BarConflict => "BarConflict",
        }
    }
}

/// Truy cập config space 256 byte của **một** function (bus/dev/func đã chọn từ trước).
pub trait PciCfg {
    /// Đọc 8 bit tại `off` (byte offset trong 256 byte đầu).
    fn read8(&self, off: u8) -> u8;
    /// Đọc 16 bit little-endian.
    fn read16(&self, off: u8) -> u16;
    /// Đọc 32 bit little-endian.
    fn read32(&self, off: u8) -> u32;
    /// Ghi 32 bit (dùng cho BAR probe/gán; mọi offset khác chỉ ghi khi test).
    fn write32(&mut self, off: u8, value: u32);

    /// Ghi 16 bit — mặc định đọc-sửa-ghi 32 bit (impl chỉ cần cài [`PciCfg::write32`]).
    fn write16(&mut self, off: u8, value: u16) {
        let aligned = off & !0x3;
        let shift = ((off & 0x3) as u32) * 8;
        let word = self.read32(aligned);
        let mask = 0xffffu32 << shift;
        self.write32(aligned, (word & !mask) | ((value as u32) << shift));
    }
}

/// Loại BAR theo bit thuộc tính đọc lại từ mask.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarKind {
    /// BAR không gian I/O (bit 0 = 1) — legacy, K6 **không** dùng.
    Io,
    /// BAR memory 32 bit.
    Memory32,
    /// BAR memory 64 bit (chiếm hai thanh ghi liền nhau).
    Memory64,
}

/// Một BAR đã đo cỡ từ mask.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bar {
    /// Chỉ số BAR (0..5).
    pub index: u8,
    /// Loại không gian/độ rộng.
    pub kind: BarKind,
    /// Cỡ (byte) suy từ mask — luỹ thừa 2.
    pub size: u64,
    /// Bit prefetchable (bit 3) — BAR modern của virtio đặt bit này.
    pub prefetchable: bool,
}

impl Bar {
    /// `true` ⇔ BAR này chiếm thêm thanh ghi kế (`index + 1`).
    pub const fn occupies_two(&self) -> bool {
        matches!(self.kind, BarKind::Memory64)
    }
}

/// Loại thiết bị virtio mà driver K6 tìm (KT-72 blk; KT-73 console; KT-245 net; KT-255 input;
/// KT-263 gpu).
///
/// Một enum thay cho hai hàm quét riêng: driver PCI quét **một** vòng bus/device/function và so
/// khớp theo `(vendor, device)` — thêm thiết bị thứ hai **không** nhân đôi code ECAM/BAR
/// (D2-110A tinh thần "một chỗ quyết định").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VirtioKind {
    /// `virtio-blk` (0x1001/0x1042).
    Blk,
    /// `virtio-console`/`virtio-serial` (0x1003/0x1043).
    Console,
    /// **KT-245** `virtio-net` (0x1000/0x1041).
    Net,
    /// **KT-255** `virtio-keyboard` (0x1012/0x1052).
    Input,
    /// **KT-263** `virtio-gpu` (0x1050/0x1010).
    Gpu,
    /// **KT-418b** `virtio-rng` (0x1004/0x1044) — device id **4**.
    Rng,
}

impl VirtioKind {
    /// Tên để in evidence.
    pub const fn name(self) -> &'static str {
        match self {
            VirtioKind::Blk => "blk",
            VirtioKind::Console => "console",
            VirtioKind::Net => "net",
            VirtioKind::Input => "input",
            VirtioKind::Gpu => "gpu",
            VirtioKind::Rng => "rng",
        }
    }

    /// `true` ⇔ `(vendor, device)` là thiết bị của loại này.
    pub const fn matches(self, vendor: u16, device: u16) -> bool {
        if vendor != VENDOR_VIRTIO {
            return false;
        }
        match self {
            VirtioKind::Blk => device == VIRTIO_BLK_MODERN || device == VIRTIO_BLK_MODERN_ONLY,
            VirtioKind::Console => {
                device == crate::console::VIRTIO_CONSOLE_MODERN
                    || device == crate::console::VIRTIO_CONSOLE_MODERN_ONLY
            }
            // KT-245: id lấy từ `net::` — **một** nguồn, không chép số lần thứ hai.
            VirtioKind::Net => {
                device == crate::net::VIRTIO_NET_MODERN
                    || device == crate::net::VIRTIO_NET_MODERN_ONLY
            }
            // KT-255: id lấy từ `input::` — cùng lý do.
            VirtioKind::Input => {
                device == crate::input::VIRTIO_INPUT_KEYBOARD
                    || device == crate::input::VIRTIO_INPUT_KEYBOARD_MODERN_ONLY
            }
            // KT-263: id lấy từ `gpu::` — cùng lý do.
            VirtioKind::Gpu => {
                device == crate::gpu::VIRTIO_GPU_MODERN
                    || device == crate::gpu::VIRTIO_GPU_TRANSITIONAL
            }
            // KT-418b: id lấy từ `rng::` — cùng lý do.
            VirtioKind::Rng => {
                device == crate::rng::VIRTIO_RNG_MODERN
                    || device == crate::rng::VIRTIO_RNG_MODERN_ONLY
            }
        }
    }
}

/// `true` ⇔ `(vendor_id, device_id)` là virtio-blk mà KT-72 chấp nhận (0x1001/0x1042).
pub const fn is_virtio_blk(vendor: u16, device: u16) -> bool {
    VirtioKind::Blk.matches(vendor, device)
}

/// `true` ⇔ `(vendor_id, device_id)` là virtio-console mà KT-73 chấp nhận (0x1003/0x1043).
pub const fn is_virtio_console(vendor: u16, device: u16) -> bool {
    VirtioKind::Console.matches(vendor, device)
}

/// `true` ⇔ `(vendor_id, device_id)` là virtio-net mà KT-245 chấp nhận (0x1000/0x1041).
pub const fn is_virtio_net(vendor: u16, device: u16) -> bool {
    VirtioKind::Net.matches(vendor, device)
}

/// `true` ⇔ `(vendor_id, device_id)` là virtio-keyboard mà KT-255 chấp nhận (0x1012/0x1052).
pub const fn is_virtio_input(vendor: u16, device: u16) -> bool {
    VirtioKind::Input.matches(vendor, device)
}

/// `true` ⇔ `(vendor_id, device_id)` là virtio-gpu mà KT-263 chấp nhận (0x1050/0x1010).
pub const fn is_virtio_gpu(vendor: u16, device: u16) -> bool {
    VirtioKind::Gpu.matches(vendor, device)
}

/// `true` ⇔ `(vendor_id, device_id)` là virtio-rng mà KT-418b chấp nhận (0x1004/0x1044).
pub const fn is_virtio_rng(vendor: u16, device: u16) -> bool {
    VirtioKind::Rng.matches(vendor, device)
}

/// Đọc `(vendor_id, device_id)` từ config space.
pub fn read_ids(cfg: &impl PciCfg) -> (u16, u16) {
    (cfg.read16(CFG_VENDOR_ID), cfg.read16(CFG_DEVICE_ID))
}

/// `Some(())` ⇔ function này là thiết bị `kind`. Chỉ trả "khớp/không khớp" — id đã đọc ở
/// [`read_ids`] khi cần in evidence.
pub fn find_function(cfg: &impl PciCfg, kind: VirtioKind) -> Option<()> {
    let (vendor, device) = read_ids(cfg);
    if kind.matches(vendor, device) {
        Some(())
    } else {
        None
    }
}

/// `Some(())` ⇔ function này là virtio-blk (giữ API KT-72 cho `virtio_blk`/test cũ).
pub fn find_blk_function(cfg: &impl PciCfg) -> Option<()> {
    find_function(cfg, VirtioKind::Blk)
}

/// Hai khoảng `[a, a+alen)` và `[b, b+blen)` có chồng lấn không (khoảng rỗng ⇒ không).
pub const fn range_overlaps(a: u64, alen: u64, b: u64, blen: u64) -> bool {
    if alen == 0 || blen == 0 {
        return false;
    }
    a < b.saturating_add(blen) && b < a.saturating_add(alen)
}

/// Tên loại xung đột khi gán BAR — in evidence, **không** in số (D2-102).
pub const CONFLICT_OTHER_BAR: &str = "other_bar";
/// Xung đột với vùng nền tảng (UART/PLIC/ECAM/RAM…).
pub const CONFLICT_RESERVED: &str = "reserved";

/// Kiểm **chồng lấn** của khoảng định gán với BAR các function khác và các vùng nền tảng.
///
/// `None` ⇒ an toàn để ghi BAR. `Some(name)` ⇒ **không** được ghi (driver fail-closed, không đè
/// im lặng). Đây là phần **thuần** của D2-110C: kernel đo BAR/vùng rồi gọi hàm này; host test
/// chứng minh nó bắt được đúng ca chồng lấn (và không bắt oan ca kề nhau).
pub fn first_conflict(
    assigned: u64,
    size: u64,
    other_bars: &[(u64, u64)],
    reserved: &[(u64, u64)],
) -> Option<&'static str> {
    if size == 0 {
        return Some(CONFLICT_RESERVED);
    }
    for (base, len) in other_bars.iter() {
        if range_overlaps(assigned, size, *base, *len) {
            return Some(CONFLICT_OTHER_BAR);
        }
    }
    for (base, len) in reserved.iter() {
        if range_overlaps(assigned, size, *base, *len) {
            return Some(CONFLICT_RESERVED);
        }
    }
    None
}

/// Suy **cỡ** BAR từ giá trị mask đọc lại sau khi ghi `0xffff_ffff` (quy tắc PCI §6.2.5.1):
/// bit thuộc tính (4 bit thấp) giữ nguyên, phần còn lại là mask cỡ ⇒ `size = (!mask)+1`.
///
/// `None` nếu mask 0 (BAR không cài) hoặc cỡ không phải luỹ thừa 2 (thiết bị trả rác — **không**
/// đoán, không làm tròn).
pub fn bar_probe(index: u8, mask_low: u32, mask_high: u32) -> Option<Bar> {
    if index as usize >= BAR_COUNT {
        return None;
    }
    let io = mask_low & 1 != 0;
    let kind = if io {
        BarKind::Io
    } else if mask_low & 0b110 == 0b100 {
        BarKind::Memory64
    } else {
        BarKind::Memory32
    };
    let keep = if io { 0x3 } else { 0xf };
    // Cỡ phải tính trong **đúng độ rộng** của BAR: BAR I/O/32-bit dùng 32 bit, BAR 64-bit dùng
    // 64 bit. `!(mask) + 1` trên u64 với mask 32 bit sẽ ra số 64 bit khổng lồ (lỗi KT-72 bắt
    // được ở test host trước khi chạy QEMU).
    let size = if kind == BarKind::Memory64 {
        let mask = ((mask_high as u64) << 32) | (mask_low as u64 & !keep);
        if mask == 0 {
            return None;
        }
        (!mask).wrapping_add(1)
    } else {
        let mask = mask_low & !(keep as u32);
        if mask == 0 {
            return None;
        }
        (!mask).wrapping_add(1) as u64
    };
    if !size.is_power_of_two() || !(16..=(1u64 << 40)).contains(&size) {
        return None;
    }
    Some(Bar {
        index,
        kind,
        size,
        prefetchable: !io && mask_low & 0b1000 != 0,
    })
}

/// Loại capability virtio.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapType {
    /// `VIRTIO_PCI_CAP_COMMON_CFG` = 1.
    Common,
    /// `VIRTIO_PCI_CAP_NOTIFY_CFG` = 2.
    Notify,
    /// `VIRTIO_PCI_CAP_ISR_CFG` = 3.
    Isr,
    /// `VIRTIO_PCI_CAP_DEVICE_CFG` = 4.
    Device,
    /// `VIRTIO_PCI_CAP_PCI_CFG` = 5 (K6 không dùng).
    PciCfg,
}

impl CapType {
    /// Giải mã byte `cfg_type`.
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            1 => Some(CapType::Common),
            2 => Some(CapType::Notify),
            3 => Some(CapType::Isr),
            4 => Some(CapType::Device),
            5 => Some(CapType::PciCfg),
            _ => None,
        }
    }
}

/// Một capability virtio đã đọc (địa chỉ **tương đối** trong BAR `bar`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VirtioCap {
    /// Loại.
    pub cap_type: CapType,
    /// Chỉ số BAR chứa vùng này.
    pub bar: u8,
    /// Offset trong BAR.
    pub offset: u32,
    /// Cỡ vùng (byte).
    pub length: u32,
}

/// Bốn capability bắt buộc + `notify_off_multiplier` + BAR chứa chúng.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VirtioCaps {
    /// Common config (feature/status/queue).
    pub common: VirtioCap,
    /// Notify (doorbell).
    pub notify: VirtioCap,
    /// ISR (đọc 1 byte, K6 chỉ đọc chẩn đoán).
    pub isr: VirtioCap,
    /// Device-specific config (capacity/size_max/…).
    pub device: VirtioCap,
    /// `notify_off_multiplier` (byte) — địa chỉ doorbell = notify.offset + off*multiplier.
    pub notify_off_multiplier: u32,
    /// BAR chứa các vùng (K6 dùng **một** BAR cho cả bốn; khác BAR ⇒ lỗi `CapMissing`).
    pub bar: u8,
}

/// Địa chỉ tuyệt đối của một vùng = BAR base + offset.
pub const fn resolve_cap(cap: &VirtioCap, bar_base: u64) -> u64 {
    bar_base + cap.offset as u64
}

/// Đọc capability list và lấy bốn vùng của virtio-pci.
///
/// Từ chối (không đoán): thiếu `status.cap_list`, chain có vòng, chain quá dài, thiếu vùng bắt
/// buộc, hoặc các vùng không cùng một BAR (driver K6 chỉ map một BAR).
pub fn parse_caps(cfg: &impl PciCfg) -> Result<VirtioCaps, PciError> {
    let status = cfg.read16(CFG_STATUS);
    let ptr = cfg.read8(CFG_CAP_PTR);
    if status & STATUS_CAP_LIST == 0 || ptr == 0 || ptr & 0x3 != 0 {
        return Err(PciError::NoCapList);
    }
    let mut common: Option<VirtioCap> = None;
    let mut notify: Option<(VirtioCap, u32)> = None;
    let mut isr: Option<VirtioCap> = None;
    let mut device: Option<VirtioCap> = None;
    let mut visited: u64 = 0;
    let mut off = ptr;
    let mut steps = 0usize;
    loop {
        steps += 1;
        if steps > CAP_WALK_LIMIT {
            return Err(PciError::CapWalkLimit);
        }
        let bit = (off >> 2) as u32;
        if off & 0x3 != 0 || bit >= 64 || visited & (1u64 << bit) != 0 {
            return Err(PciError::CapLoop);
        }
        visited |= 1u64 << bit;
        let id = cfg.read8(off);
        let next = cfg.read8(off.wrapping_add(1));
        if id == CAP_ID_VENDOR {
            if let Some(cap_type) = CapType::from_u8(cfg.read8(off.wrapping_add(3))) {
                let cap = VirtioCap {
                    cap_type,
                    bar: cfg.read8(off.wrapping_add(4)),
                    offset: cfg.read32(off.wrapping_add(8)),
                    length: cfg.read32(off.wrapping_add(12)),
                };
                match cap_type {
                    CapType::Common => common = Some(cap),
                    CapType::Notify => {
                        let multiplier = cfg.read32(off.wrapping_add(16));
                        notify = Some((cap, multiplier));
                    }
                    CapType::Isr => isr = Some(cap),
                    CapType::Device => device = Some(cap),
                    CapType::PciCfg => {}
                }
            }
        }
        if next == 0 {
            break;
        }
        // PCI **không** bắt buộc `next` tăng dần (cap list chỉ là danh sách liên kết trong 256
        // byte) ⇒ chỉ chống vòng bằng tập offset đã thăm, **không** đòi thứ tự tăng.
        off = next;
    }
    let (common, notify, isr, device) = match (common, notify, isr, device) {
        (Some(c), Some(n), Some(i), Some(d)) => (c, n, i, d),
        _ => return Err(PciError::CapMissing),
    };
    let bar = common.bar;
    if notify.0.bar != bar || isr.bar != bar || device.bar != bar {
        return Err(PciError::CapMissing);
    }
    Ok(VirtioCaps {
        common,
        notify: notify.0,
        isr,
        device,
        notify_off_multiplier: notify.1,
        bar,
    })
}
