// SPDX-License-Identifier: MIT OR Apache-2.0
//! PCI trên x86_64 — **ECAM** (q35, bảng ACPI `MCFG`) và **legacy `0xCF8`/`0xCFC`** (pc/i440fx)
//! — KT-274 (K5' bước 5).
//!
//! # Vì sao **hai** đường truy cập cấu hình
//!
//! Đo trên QEMU 11.1.3 (log `v2/agent/JOURNAL/`):
//!
//! | Máy | ECAM | Đường dùng |
//! |---|---|---|
//! | `-M q35` | `pcie-mmcfg-mmio` @ **`0xB000_0000`**, 256 MiB (bus 0..255) — bảng `MCFG` mô tả | **ECAM** |
//! | `-M pc` (i440fx) | **không có** (không có `MCFG`) | **legacy `0xCF8`/`0xCFC`** |
//!
//! Chọn ECAM khi có (một phép đọc 32 bit cho mỗi thanh ghi, không cần khoá tuần tự, là đường chuẩn
//! của PCIe); legacy là **đường dự phòng** cho `-M pc`. Địa chỉ ECAM **không** viết tay: đọc từ
//! `MCFG` trong ACPI (RSDP → RSDT/XSDT → `MCFG`), có kiểm checksum từng bảng. Chỉ khi ACPI không
//! cho `MCFG` (hoặc ECAM đọc ra `0xffff`) mới rơi xuống legacy — và ảnh in rõ `# pci: legacy …`.
//!
//! # Vì sao **không** gán BAR
//!
//! Trên x86, **SeaBIOS** đã enumerate và gán BAR trước khi ảnh chạy (đo được trên q35: virtio-net
//! `BAR4 = 0xFE00_0000`, `BAR1 = 0xFEBD_1000`; trên `-M pc` cũng `0xFE00_0000`). Vì vậy driver
//! **đọc** BAR rồi **kiểm nó nằm trong cửa sổ MMIO đã map** ([`crate::boot::mmio_mapped`]) — không
//! ghi đè địa chỉ firmware đã cấp. Khác riscv64 (`virt` không có firmware enumerate ⇒ driver tự
//! gán), đây là khác biệt **nền tảng**, không phải lựa chọn.
//!
//! # Phần nào test được trên host
//!
//! Mã hoá địa chỉ ([`legacy_cf8`], [`ecam_addr`]) và **toàn bộ** phần ACPI ([`parse_rsdp`],
//! [`parse_mcfg`], [`find_mcfg`]) là **thuần** trên byte/lát cắt — `tests/pci.rs` chốt giá trị
//! tính tay + ca âm (checksum sai, bảng cụt, không RSDP). Phần chạm MMIO/cổng I/O đi qua
//! [`crate::asm`] (mô-đun duy nhất có `unsafe`), nên tệp này **không** có `unsafe`.

use vifast_virtio::pci::{
    bar_probe, parse_caps, BarKind, PciCfg, PciError, VirtioCaps, VirtioKind, BAR_COUNT, CFG_BAR0,
    CFG_COMMAND, CFG_DEVICE_ID, CFG_HEADER_TYPE, CFG_VENDOR_ID, COMMAND_BUS_MASTER, COMMAND_MEMORY,
};

use crate::asm;
use crate::boot;

// ---------------------------------------------------------------------------------------------
// Legacy 0xCF8/0xCFC — PCI Local Bus 3.0 §3.2.2.3.1
// ---------------------------------------------------------------------------------------------

/// Cổng `CONFIG_ADDRESS` (32 bit).
pub const CF8_PORT: u16 = 0x0cf8;
/// Cổng `CONFIG_DATA` (32 bit).
pub const CFC_PORT: u16 = 0x0cfc;
/// Bit 31 của `CONFIG_ADDRESS` — bật thì chu kỳ cấu hình mới được phát.
pub const CF8_ENABLE: u32 = 0x8000_0000;
/// Vị trí trường bus trong `CONFIG_ADDRESS`.
pub const CF8_BUS_SHIFT: u32 = 16;
/// Vị trí trường device (5 bit).
pub const CF8_DEV_SHIFT: u32 = 11;
/// Vị trí trường function (3 bit).
pub const CF8_FUNC_SHIFT: u32 = 8;
/// Mặt nạ offset: thanh ghi cấu hình là 32 bit ⇒ bit 0..1 **bị che**.
pub const CF8_OFF_MASK: u32 = 0xfc;
/// Số byte cấu hình của một function (PCI §6.1).
pub const CFG_SPACE_BYTES: u16 = 256;

/// Mã hoá `CONFIG_ADDRESS` cho `(bus, dev, func, off)`.
///
/// `off` bị che hai bit thấp (mọi truy cập đi theo **word** 32 bit); `dev`/`func` bị che theo độ
/// rộng trường để một giá trị ngoài dải **không** tràn sang trường khác (đó là lỗi im lặng).
pub const fn legacy_cf8(bus: u8, dev: u8, func: u8, off: u8) -> u32 {
    CF8_ENABLE
        | ((bus as u32) << CF8_BUS_SHIFT)
        | (((dev as u32) & 0x1f) << CF8_DEV_SHIFT)
        | (((func as u32) & 0x7) << CF8_FUNC_SHIFT)
        | ((off as u32) & CF8_OFF_MASK)
}

// ---------------------------------------------------------------------------------------------
// ECAM — PCIe §7.2.2
// ---------------------------------------------------------------------------------------------

/// Vị trí trường bus trong địa chỉ ECAM (mỗi bus 1 MiB).
pub const ECAM_BUS_SHIFT: u32 = 20;
/// Vị trí trường device (mỗi device 32 KiB).
pub const ECAM_DEV_SHIFT: u32 = 15;
/// Vị trí trường function (mỗi function 4 KiB).
pub const ECAM_FUNC_SHIFT: u32 = 12;
/// Số byte cấu hình của một bus trong ECAM.
pub const ECAM_BUS_BYTES: u64 = 1 << ECAM_BUS_SHIFT;

/// Địa chỉ MMIO của `(bus, dev, func, off)` trong ECAM có gốc `base`.
pub const fn ecam_addr(base: u64, bus: u8, dev: u8, func: u8, off: u8) -> u64 {
    base + ((bus as u64) << ECAM_BUS_SHIFT)
        + (((dev as u64) & 0x1f) << ECAM_DEV_SHIFT)
        + (((func as u64) & 0x7) << ECAM_FUNC_SHIFT)
        + (off as u64)
}

// ---------------------------------------------------------------------------------------------
// ACPI: RSDP → RSDT/XSDT → MCFG
// ---------------------------------------------------------------------------------------------

/// Chữ ký `RSDP` (8 byte, **không** có NUL kết thúc trong 8 byte này).
pub const RSDP_SIGNATURE: [u8; 8] = *b"RSD PTR ";
/// Chữ ký bảng `MCFG` (Memory Mapped Configuration space).
pub const MCFG_SIGNATURE: [u8; 4] = *b"MCFG";
/// Chữ ký bảng `XSDT` (bảng mục lục 64-bit).
pub const XSDT_SIGNATURE: [u8; 4] = *b"XSDT";
/// Chữ ký bảng `RSDT` (bảng mục lục 32-bit).
pub const RSDT_SIGNATURE: [u8; 4] = *b"RSDT";
/// Cỡ header chung của một bảng ACPI (ACPI §5.2.6).
pub const ACPI_HEADER_BYTES: usize = 36;
/// Cỡ một allocation structure trong `MCFG` (ACPI §5.2.6.2).
pub const MCFG_ENTRY_BYTES: usize = 16;
/// Số byte **reserved** giữa header `MCFG` và allocation structure đầu tiên (ACPI 6.4 §5.2.6.2:
/// trường `Reserved` 8 byte ở offset `0x24`) — **đo được**: bảng thật của SeaBIOS trên q35 dài
/// `0x3C` = 44 + 16, và `base_address` nằm ở offset **44**, không phải 36.
pub const MCFG_RESERVED_BYTES: usize = 8;
/// Offset của allocation structure đầu tiên trong `MCFG`.
pub const MCFG_ENTRY_OFFSET: usize = ACPI_HEADER_BYTES + MCFG_RESERVED_BYTES;
/// Cỡ RSDP bản 1.0 (ACPI §5.2.5.1).
pub const RSDP_V1_BYTES: usize = 20;
/// Cỡ RSDP bản 2.0+ (có `length` + `xsdt_address`).
pub const RSDP_V2_BYTES: usize = 36;
/// Đầu cửa sổ quét `RSDP`: EBDA (ACPI §5.2.2 nói "16 byte căn lề trong 1 KiB đầu của EBDA").
pub const EBDA_SCAN_START: u64 = 0x000e_0000;
/// Cuối cửa sổ quét `RSDP`: hết vùng BIOS ROM (1 MiB).
pub const EBDA_SCAN_END: u64 = 0x0010_0000;
/// Bước quét `RSDP`: 16 byte (đặc tả đòi căn 16).
pub const EBDA_SCAN_STEP: u64 = 16;
/// Trần cỡ một bảng ACPI driver đọc (đủ cho `XSDT`/`MCFG` thật; chặn `length` rác).
pub const MAX_ACPI_TABLE_BYTES: usize = 4096;

/// `true` ⇔ tổng mọi byte (mod 256) bằng 0 — checksum của ACPI §5.2.6.
pub fn checksum_ok(bytes: &[u8]) -> bool {
    bytes.iter().fold(0u8, |acc, b| acc.wrapping_add(*b)) == 0
}

/// RSDP đã giải mã.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rsdp {
    /// Phiên bản (0 = ACPI 1.0, 2 = ACPI 2.0+).
    pub revision: u8,
    /// Địa chỉ `RSDT` (32 bit) — luôn có.
    pub rsdt: u32,
    /// Địa chỉ `XSDT` (64 bit) — chỉ có từ bản 2.0.
    pub xsdt: u64,
    /// Độ dài bảng (20 với bản 1.0, 36 với bản 2.0).
    pub length: u32,
}

/// Giải mã `RSDP` từ `bytes` (ít nhất [`RSDP_V1_BYTES`] byte).
///
/// Từ chối (trả `None`, **không** đoán): chữ ký sai, checksum 20 byte đầu sai, bản 2.0 mà thiếu
/// 36 byte / `length` vô lý / checksum mở rộng sai.
pub fn parse_rsdp(bytes: &[u8]) -> Option<Rsdp> {
    if bytes.len() < RSDP_V1_BYTES || bytes[..8] != RSDP_SIGNATURE {
        return None;
    }
    if !checksum_ok(&bytes[..RSDP_V1_BYTES]) {
        return None;
    }
    let revision = bytes[15];
    let rsdt = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    if revision < 2 {
        return Some(Rsdp {
            revision,
            rsdt,
            xsdt: 0,
            length: RSDP_V1_BYTES as u32,
        });
    }
    if bytes.len() < RSDP_V2_BYTES {
        return None;
    }
    let length = u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    if (length as usize) < RSDP_V2_BYTES || length as usize > bytes.len() {
        return None;
    }
    if !checksum_ok(&bytes[..length as usize]) {
        return None;
    }
    let xsdt = u64::from_le_bytes([
        bytes[24], bytes[25], bytes[26], bytes[27], bytes[28], bytes[29], bytes[30], bytes[31],
    ]);
    Some(Rsdp {
        revision,
        rsdt,
        xsdt,
        length,
    })
}

/// Chữ ký của một bảng ACPI (4 byte đầu), `None` nếu lát cắt quá ngắn.
pub fn table_signature(bytes: &[u8]) -> Option<[u8; 4]> {
    if bytes.len() < 4 {
        return None;
    }
    Some([bytes[0], bytes[1], bytes[2], bytes[3]])
}

/// Độ dài bảng ACPI (trường `length` 32 bit ở offset 4), `None` nếu lát cắt quá ngắn.
pub fn table_length(bytes: &[u8]) -> Option<u32> {
    if bytes.len() < 8 {
        return None;
    }
    Some(u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]))
}

/// Một allocation structure của `MCFG`: vùng ECAM của một segment PCI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct McfgEntry {
    /// Địa chỉ vật lý gốc của ECAM.
    pub base: u64,
    /// Segment group (0 với PC).
    pub segment: u16,
    /// Bus đầu tiên được mô tả.
    pub bus_start: u8,
    /// Bus cuối cùng được mô tả.
    pub bus_end: u8,
}

/// Giải mã bảng `MCFG` (header + allocation structure **đầu tiên**).
///
/// Từ chối: chữ ký sai, `length` < header + entry hoặc > lát cắt, checksum sai, `bus_end <
/// bus_start`. Không đoán địa chỉ ECAM khi bảng hỏng — rơi xuống legacy là quyết định của người
/// gọi, không phải của hàm này.
pub fn parse_mcfg(bytes: &[u8]) -> Option<McfgEntry> {
    if table_signature(bytes)? != MCFG_SIGNATURE {
        return None;
    }
    let length = table_length(bytes)? as usize;
    if length < MCFG_ENTRY_OFFSET + MCFG_ENTRY_BYTES || length > bytes.len() {
        return None;
    }
    if !checksum_ok(&bytes[..length]) {
        return None;
    }
    let e = &bytes[MCFG_ENTRY_OFFSET..MCFG_ENTRY_OFFSET + MCFG_ENTRY_BYTES];
    let base = u64::from_le_bytes([e[0], e[1], e[2], e[3], e[4], e[5], e[6], e[7]]);
    let segment = u16::from_le_bytes([e[8], e[9]]);
    let bus_start = e[10];
    let bus_end = e[11];
    if bus_end < bus_start {
        return None;
    }
    Some(McfgEntry {
        base,
        segment,
        bus_start,
        bus_end,
    })
}

/// Nguồn đọc bộ nhớ **vật lý** (identity map) — kernel cài bằng [`crate::asm::read_u8`], host test
/// cài bằng một mảng. Nhờ trait này toàn bộ phần ACPI chạy được trên host (D2-05).
pub trait PhysMem {
    /// Đọc `out.len()` byte từ `addr`; `false` nếu địa chỉ ngoài vùng đọc được.
    fn read(&self, addr: u64, out: &mut [u8]) -> bool;
}

/// Bộ nhớ vật lý của ảnh: identity map phủ `0..1 GiB` nên EBDA/BIOS ROM đọc trực tiếp được.
pub struct KernelMem;

impl PhysMem for KernelMem {
    fn read(&self, addr: u64, out: &mut [u8]) -> bool {
        // Identity map 1 GiB phủ toàn bộ vùng quét RSDP (0xE0000..0x100000) và mọi bảng ACPI
        // dưới 1 MiB mà firmware đặt; địa chỉ ngoài vùng đó không được đọc (fail-closed).
        if addr < boot::IDENTITY_MAP_BYTES
            && addr.saturating_add(out.len() as u64) <= boot::IDENTITY_MAP_BYTES
        {
            for (index, byte) in out.iter_mut().enumerate() {
                *byte = asm::read_u8(addr + index as u64);
            }
            true
        } else {
            false
        }
    }
}

/// Tìm `RSDP` trong cửa sổ EBDA/BIOS ROM ([`EBDA_SCAN_START`]..[`EBDA_SCAN_END`], bước 16 byte).
pub fn find_rsdp(mem: &impl PhysMem) -> Option<(u64, Rsdp)> {
    let mut addr = EBDA_SCAN_START;
    while addr < EBDA_SCAN_END {
        let mut buf = [0u8; RSDP_V2_BYTES];
        if mem.read(addr, &mut buf) && buf[..8] == RSDP_SIGNATURE {
            if let Some(rsdp) = parse_rsdp(&buf) {
                return Some((addr, rsdp));
            }
        }
        addr += EBDA_SCAN_STEP;
    }
    None
}

/// Đọc một bảng ACPI đầy đủ (header + thân) vào `buf`; trả độ dài thật.
fn read_table(
    mem: &impl PhysMem,
    addr: u64,
    buf: &mut [u8; MAX_ACPI_TABLE_BYTES],
) -> Option<usize> {
    let mut header = [0u8; ACPI_HEADER_BYTES];
    if !mem.read(addr, &mut header) {
        return None;
    }
    let length = table_length(&header)? as usize;
    if !(ACPI_HEADER_BYTES..=MAX_ACPI_TABLE_BYTES).contains(&length) {
        return None;
    }
    if !mem.read(addr, &mut buf[..length]) {
        return None;
    }
    Some(length)
}

/// Tìm bảng `MCFG` trong `XSDT` (entry 8 byte).
fn find_in_xsdt(mem: &impl PhysMem, xsdt: u64) -> Option<McfgEntry> {
    let mut buf = [0u8; MAX_ACPI_TABLE_BYTES];
    let length = read_table(mem, xsdt, &mut buf)?;
    if table_signature(&buf[..length])? != XSDT_SIGNATURE {
        return None;
    }
    let count = (length - ACPI_HEADER_BYTES) / 8;
    for index in 0..count {
        let at = ACPI_HEADER_BYTES + index * 8;
        let table = u64::from_le_bytes([
            buf[at],
            buf[at + 1],
            buf[at + 2],
            buf[at + 3],
            buf[at + 4],
            buf[at + 5],
            buf[at + 6],
            buf[at + 7],
        ]);
        if table == 0 {
            continue;
        }
        if let Some(entry) = read_mcfg(mem, table) {
            return Some(entry);
        }
    }
    None
}

/// Tìm bảng `MCFG` trong `RSDT` (entry 4 byte) — đường của ACPI 1.0.
fn find_in_rsdt(mem: &impl PhysMem, rsdt: u64) -> Option<McfgEntry> {
    let mut buf = [0u8; MAX_ACPI_TABLE_BYTES];
    let length = read_table(mem, rsdt, &mut buf)?;
    if table_signature(&buf[..length])? != RSDT_SIGNATURE {
        return None;
    }
    let count = (length - ACPI_HEADER_BYTES) / 4;
    for index in 0..count {
        let at = ACPI_HEADER_BYTES + index * 4;
        let table = u32::from_le_bytes([buf[at], buf[at + 1], buf[at + 2], buf[at + 3]]) as u64;
        if table == 0 {
            continue;
        }
        if let Some(entry) = read_mcfg(mem, table) {
            return Some(entry);
        }
    }
    None
}

/// Đọc bảng tại `addr` và giải mã nếu nó là `MCFG`.
fn read_mcfg(mem: &impl PhysMem, addr: u64) -> Option<McfgEntry> {
    let mut buf = [0u8; MAX_ACPI_TABLE_BYTES];
    let length = read_table(mem, addr, &mut buf)?;
    parse_mcfg(&buf[..length])
}

/// Tìm `MCFG` qua ACPI: `RSDP` → `XSDT` (bản 2.0) → `RSDT` (dự phòng) → `MCFG`.
pub fn find_mcfg(mem: &impl PhysMem) -> Option<McfgEntry> {
    let (_, rsdp) = find_rsdp(mem)?;
    if rsdp.revision >= 2 && rsdp.xsdt != 0 {
        if let Some(entry) = find_in_xsdt(mem, rsdp.xsdt) {
            return Some(entry);
        }
    }
    if rsdp.rsdt != 0 {
        return find_in_rsdt(mem, rsdp.rsdt as u64);
    }
    None
}

// ---------------------------------------------------------------------------------------------
// Truy cập cấu hình: ECAM hoặc legacy
// ---------------------------------------------------------------------------------------------

/// Loại đường truy cập cấu hình đã chọn — in vào evidence (D2-102: in chữ, không in số trần).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessKind {
    /// ECAM (MMIO) — `-M q35`.
    Ecam,
    /// Legacy `0xCF8`/`0xCFC` — `-M pc`.
    Legacy,
}

impl AccessKind {
    /// Tên để in evidence.
    pub const fn name(self) -> &'static str {
        match self {
            AccessKind::Ecam => "ecam",
            AccessKind::Legacy => "legacy",
        }
    }
}

/// ECAM: gốc MMIO + dải bus mà `MCFG` mô tả.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ecam {
    base: u64,
    bus_start: u8,
    bus_end: u8,
}

impl Ecam {
    /// Dựng từ `MCFG`.
    pub const fn new(base: u64, bus_start: u8, bus_end: u8) -> Self {
        Self {
            base,
            bus_start,
            bus_end,
        }
    }

    /// Gốc MMIO.
    pub const fn base(&self) -> u64 {
        self.base
    }

    /// Dải bus `(đầu, cuối)`.
    pub const fn buses(&self) -> (u8, u8) {
        (self.bus_start, self.bus_end)
    }
}

/// Đường truy cập cấu hình đã chọn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    /// ECAM.
    Ecam(Ecam),
    /// Legacy `0xCF8`/`0xCFC` (chỉ bus 0).
    Legacy,
}

impl Access {
    /// Loại.
    pub const fn kind(&self) -> AccessKind {
        match self {
            Access::Ecam(_) => AccessKind::Ecam,
            Access::Legacy => AccessKind::Legacy,
        }
    }

    /// Gốc ECAM (0 với legacy).
    pub const fn ecam_base(&self) -> u64 {
        match self {
            Access::Ecam(ecam) => ecam.base,
            Access::Legacy => 0,
        }
    }

    /// Dải bus quét.
    pub const fn buses(&self) -> (u8, u8) {
        match self {
            Access::Ecam(ecam) => (ecam.bus_start, ecam.bus_end),
            Access::Legacy => (0, 0),
        }
    }

    /// Một function cụ thể — cài [`PciCfg`] để dùng chung logic BAR/cap của `vifast-virtio`.
    pub const fn function(&self, bus: u8, dev: u8, func: u8) -> Function<'_> {
        Function {
            access: self,
            bus,
            dev,
            func,
        }
    }
}

/// Một function PCI: `(bus, dev, func)` + đường truy cập.
#[derive(Clone, Copy, Debug)]
pub struct Function<'a> {
    access: &'a Access,
    bus: u8,
    dev: u8,
    func: u8,
}

impl Function<'_> {
    /// Bus.
    pub const fn bus(&self) -> u8 {
        self.bus
    }
    /// Device.
    pub const fn dev(&self) -> u8 {
        self.dev
    }
    /// Function.
    pub const fn func(&self) -> u8 {
        self.func
    }

    /// Đọc 8 bit cấu hình.
    pub fn read8(&self, off: u8) -> u8 {
        match self.access {
            Access::Ecam(ecam) => {
                asm::read_u8(ecam_addr(ecam.base, self.bus, self.dev, self.func, off))
            }
            Access::Legacy => {
                asm::outl(CF8_PORT, legacy_cf8(self.bus, self.dev, self.func, off));
                let word = asm::inl(CFC_PORT);
                (word >> (((off & 3) as u32) * 8)) as u8
            }
        }
    }

    /// Đọc 16 bit cấu hình.
    pub fn read16(&self, off: u8) -> u16 {
        match self.access {
            Access::Ecam(ecam) => {
                asm::read_u16(ecam_addr(ecam.base, self.bus, self.dev, self.func, off))
            }
            Access::Legacy => {
                asm::outl(CF8_PORT, legacy_cf8(self.bus, self.dev, self.func, off));
                let word = asm::inl(CFC_PORT);
                (word >> (((off & 2) as u32) * 8)) as u16
            }
        }
    }

    /// Đọc 32 bit cấu hình.
    pub fn read32(&self, off: u8) -> u32 {
        match self.access {
            Access::Ecam(ecam) => {
                asm::read_u32(ecam_addr(ecam.base, self.bus, self.dev, self.func, off))
            }
            Access::Legacy => {
                asm::outl(CF8_PORT, legacy_cf8(self.bus, self.dev, self.func, off));
                asm::inl(CFC_PORT)
            }
        }
    }

    /// Ghi 32 bit cấu hình (BAR probe/gán, `command`).
    pub fn write32(&mut self, off: u8, value: u32) {
        match self.access {
            Access::Ecam(ecam) => asm::write_u32(
                ecam_addr(ecam.base, self.bus, self.dev, self.func, off),
                value,
            ),
            Access::Legacy => {
                asm::outl(CF8_PORT, legacy_cf8(self.bus, self.dev, self.func, off));
                asm::outl(CFC_PORT, value);
            }
        }
    }
}

impl PciCfg for Function<'_> {
    fn read8(&self, off: u8) -> u8 {
        Function::read8(self, off)
    }
    fn read16(&self, off: u8) -> u16 {
        Function::read16(self, off)
    }
    fn read32(&self, off: u8) -> u32 {
        Function::read32(self, off)
    }
    fn write32(&mut self, off: u8, value: u32) {
        Function::write32(self, off, value)
    }
}

// ---------------------------------------------------------------------------------------------
// Quét bus/device/function
// ---------------------------------------------------------------------------------------------

/// Offset `class_code`/`revision` (32 bit: rev | prog_if | subclass | class).
pub const CFG_CLASS_REVISION: u8 = 0x08;
/// Bit 7 của `header_type` = thiết bị **đa function** (PCI §6.1).
pub const HEADER_TYPE_MULTI_FUNCTION: u8 = 0x80;
/// Số device tối đa trên một bus.
pub const SCAN_DEVICES: u8 = 32;
/// Số function tối đa của một device.
pub const SCAN_FUNCTIONS: u8 = 8;
/// Trần số function **có mặt** thu thập để in evidence.
pub const MAX_DEVICES: usize = 16;

/// Lớp thiết bị (PCI §6.2.1 `class_code`).
pub const CLASS_STORAGE: u8 = 0x01;
/// Mạng.
pub const CLASS_NETWORK: u8 = 0x02;
/// Hiển thị.
pub const CLASS_DISPLAY: u8 = 0x03;
/// Cầu nối.
pub const CLASS_BRIDGE: u8 = 0x06;

/// Tên lớp thiết bị để in evidence.
pub const fn class_name(class: u8) -> &'static str {
    match class {
        CLASS_STORAGE => "storage",
        CLASS_NETWORK => "network",
        CLASS_DISPLAY => "display",
        CLASS_BRIDGE => "bridge",
        _ => "other",
    }
}

/// Một function **có mặt** (vendor ≠ `0xffff`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PciDevice {
    /// Bus.
    pub bus: u8,
    /// Device.
    pub dev: u8,
    /// Function.
    pub func: u8,
    /// Vendor id.
    pub vendor: u16,
    /// Device id.
    pub device: u16,
    /// `class_code`.
    pub class: u8,
    /// `subclass`.
    pub subclass: u8,
    /// `prog_if`.
    pub prog_if: u8,
    /// `revision`.
    pub revision: u8,
    /// `header_type` (bit 7 = đa function).
    pub header_type: u8,
}

impl PciDevice {
    /// Giá trị rỗng cho mảng `[PciDevice; MAX_DEVICES]`.
    pub const EMPTY: Self = Self {
        bus: 0,
        dev: 0,
        func: 0,
        vendor: 0,
        device: 0,
        class: 0,
        subclass: 0,
        prog_if: 0,
        revision: 0,
        header_type: 0,
    };

    /// `true` ⇔ thiết bị đa function.
    pub const fn multi_function(&self) -> bool {
        self.header_type & HEADER_TYPE_MULTI_FUNCTION != 0
    }
}

/// Kết quả quét bus/device/function.
#[derive(Clone, Copy, Debug)]
pub struct ScanReport {
    /// Đường truy cập đã chọn.
    pub access: AccessKind,
    /// Gốc ECAM (0 với legacy).
    pub ecam_base: u64,
    /// Dải bus đã quét.
    pub ecam_buses: (u8, u8),
    /// Số function **có mặt** đã thấy.
    pub functions_seen: u32,
    /// Các function có mặt (tối đa [`MAX_DEVICES`]).
    pub devices: [PciDevice; MAX_DEVICES],
    /// Số phần tử hợp lệ của [`ScanReport::devices`].
    pub count: usize,
}

impl ScanReport {
    /// Đường truy cập dựng lại từ báo cáo (để `init_for` không phải giữ tham chiếu).
    pub const fn access(&self) -> Access {
        match self.access {
            AccessKind::Ecam => Access::Ecam(Ecam::new(
                self.ecam_base,
                self.ecam_buses.0,
                self.ecam_buses.1,
            )),
            AccessKind::Legacy => Access::Legacy,
        }
    }

    /// Các function có mặt.
    pub fn devices(&self) -> &[PciDevice] {
        &self.devices[..self.count]
    }

    /// Function đầu tiên khớp `kind`.
    pub fn find(&self, kind: VirtioKind) -> Option<PciDevice> {
        let mut index = 0;
        while index < self.count {
            let device = self.devices[index];
            if kind.matches(device.vendor, device.device) {
                return Some(device);
            }
            index += 1;
        }
        None
    }
}

/// Chọn đường truy cập: ECAM nếu `MCFG` cho một gốc **đã map** và đọc được vendor của `0:0.0`;
/// ngược lại legacy.
fn detect_access() -> Access {
    if let Some(entry) = find_mcfg(&KernelMem) {
        // Chỉ dùng ECAM khi vùng của nó nằm trong cửa sổ MMIO **đã map** — đọc một địa chỉ chưa
        // map sẽ `#PF` và giết ảnh, không phải "rơi xuống legacy".
        if boot::mmio_mapped(entry.base, ECAM_BUS_BYTES) {
            let ecam = Ecam::new(entry.base, entry.bus_start, entry.bus_end);
            let access = Access::Ecam(ecam);
            let probe = access.function(0, 0, 0);
            // Host bridge luôn ở `0:0.0` trên PC; `0xffff` nghĩa là ECAM không giải mã được.
            if probe.read16(CFG_VENDOR_ID) != 0xffff {
                return Access::Ecam(ecam);
            }
        }
    }
    Access::Legacy
}

/// Quét **mọi** bus/device/function trong dải của đường truy cập đã chọn.
///
/// Chỉ function 0 được kiểm cho thiết bị **đơn function** (bit 7 của `header_type` = 0) — đúng
/// cách enumerate của PCI, và tránh đọc 7 function không tồn tại cho mỗi device.
pub fn scan() -> ScanReport {
    let access = detect_access();
    let (bus_start, bus_end) = access.buses();
    let mut report = ScanReport {
        access: access.kind(),
        ecam_base: access.ecam_base(),
        ecam_buses: (bus_start, bus_end),
        functions_seen: 0,
        devices: [PciDevice::EMPTY; MAX_DEVICES],
        count: 0,
    };
    let mut bus = bus_start;
    loop {
        for dev in 0..SCAN_DEVICES {
            for func in 0..SCAN_FUNCTIONS {
                let function = access.function(bus, dev, func);
                let vendor = function.read16(CFG_VENDOR_ID);
                if vendor == 0xffff {
                    continue;
                }
                report.functions_seen += 1;
                let device = function.read16(CFG_DEVICE_ID);
                let class_rev = function.read32(CFG_CLASS_REVISION);
                let header_type = function.read8(CFG_HEADER_TYPE);
                if report.count < MAX_DEVICES {
                    report.devices[report.count] = PciDevice {
                        bus,
                        dev,
                        func,
                        vendor,
                        device,
                        class: (class_rev >> 24) as u8,
                        subclass: (class_rev >> 16) as u8,
                        prog_if: (class_rev >> 8) as u8,
                        revision: class_rev as u8,
                        header_type,
                    };
                    report.count += 1;
                }
                if func == 0 && header_type & HEADER_TYPE_MULTI_FUNCTION == 0 {
                    break;
                }
            }
        }
        if bus == bus_end {
            break;
        }
        bus += 1;
    }
    report
}

// ---------------------------------------------------------------------------------------------
// Tìm thiết bị + BAR + capability
// ---------------------------------------------------------------------------------------------

/// Lỗi khởi tạo PCI kèm **chặng** (fail-closed, in được lý do).
#[derive(Clone, Copy, Debug)]
pub struct PciFail {
    /// Mã lỗi.
    pub error: PciError,
    /// Chặng: `nodevice`/`caps`/`bar-io`/`bar-zero`/`bar-size`/`bar-unmapped`/`command`.
    pub stage: &'static str,
}

impl PciFail {
    const fn at(error: PciError, stage: &'static str) -> Self {
        Self { error, stage }
    }
}

/// Mọi số đo của lần tìm thiết bị — **giá trị đọc lại từ thiết bị**, không hằng.
#[derive(Clone, Copy, Debug)]
pub struct PciReport {
    /// Đường truy cập đã dùng.
    pub access: AccessKind,
    /// Gốc ECAM (0 với legacy).
    pub ecam_base: u64,
    /// Số function có mặt trên bus đã quét.
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
    /// `class_code`.
    pub class: u8,
    /// `subclass`.
    pub subclass: u8,
    /// `prog_if`.
    pub prog_if: u8,
    /// `header_type`.
    pub header_type: u8,
    /// Chỉ số BAR chứa capability (BAR modern của virtio-pci).
    pub bar_index: u8,
    /// Loại BAR.
    pub bar_kind: BarKind,
    /// Cỡ BAR **đo** từ mask.
    pub bar_size: u64,
    /// Địa chỉ BAR firmware đã gán (**đọc lại**, không ghi đè).
    pub bar_base: u64,
    /// Bốn capability + `notify_off_multiplier`.
    pub caps: VirtioCaps,
    /// `command` đọc lại sau khi bật memory + bus master.
    pub command: u16,
}

/// Tìm thiết bị `kind` trong kết quả quét, đọc BAR + capability, bật decode memory/bus-master.
///
/// **Không** gán lại BAR: firmware đã gán (xem đầu tệp). Chỉ **kiểm** BAR nằm trong cửa sổ MMIO
/// đã map — ngoài cửa sổ ⇒ `BarUnassigned` chặng `bar-unmapped` (fail-closed, không đọc bừa).
pub fn init_for(kind: VirtioKind, scan: &ScanReport) -> Result<PciReport, PciFail> {
    let device = scan
        .find(kind)
        .ok_or(PciFail::at(PciError::NoDevice, "nodevice"))?;
    let access = scan.access();
    let mut function = access.function(device.bus, device.dev, device.func);
    // Capability **trước**: nó cho biết BAR nào chứa common/notify/device/isr.
    let caps = parse_caps(&function).map_err(|error| PciFail::at(error, "caps"))?;
    let bar_index = caps.bar;
    if bar_index as usize >= BAR_COUNT {
        return Err(PciFail::at(PciError::BarUnassigned, "bar-index"));
    }
    let off = CFG_BAR0 + bar_index * 4;
    let raw_low = function.read32(off);
    if raw_low & 1 != 0 {
        // BAR không gian I/O: virtio-pci modern dùng BAR memory — không đoán.
        return Err(PciFail::at(PciError::BarUnassigned, "bar-io"));
    }
    let is_64 = raw_low & 0b110 == 0b100;
    let raw_high = if is_64 { function.read32(off + 4) } else { 0 };
    let bar_base = ((raw_low as u64) & !0xf) | ((raw_high as u64) << 32);
    if bar_base == 0 {
        return Err(PciFail::at(PciError::BarUnassigned, "bar-zero"));
    }
    // Đo cỡ bằng mask (PCI §6.2.5.1) rồi **khôi phục** nguyên giá trị firmware đã ghi.
    function.write32(off, 0xffff_ffff);
    let mask_low = function.read32(off);
    let mask_high = if is_64 {
        function.write32(off + 4, 0xffff_ffff);
        let high = function.read32(off + 4);
        function.write32(off + 4, raw_high);
        high
    } else {
        0
    };
    function.write32(off, raw_low);
    let bar = bar_probe(bar_index, mask_low, mask_high)
        .ok_or(PciFail::at(PciError::BarUnassigned, "bar-size"))?;
    if !boot::mmio_mapped(bar_base, bar.size) {
        return Err(PciFail::at(PciError::BarUnassigned, "bar-unmapped"));
    }
    // Bật decode memory + bus master (DMA) — virtio cần cả hai; đọc lại để chứng minh.
    let command = function.read16(CFG_COMMAND) | COMMAND_MEMORY | COMMAND_BUS_MASTER;
    function.write16(CFG_COMMAND, command);
    let command = function.read16(CFG_COMMAND);
    if command & (COMMAND_MEMORY | COMMAND_BUS_MASTER) != COMMAND_MEMORY | COMMAND_BUS_MASTER {
        return Err(PciFail::at(PciError::BarUnassigned, "command"));
    }
    Ok(PciReport {
        access: scan.access,
        ecam_base: scan.ecam_base,
        functions_seen: scan.functions_seen,
        bus: device.bus,
        dev: device.dev,
        func: device.func,
        vendor: device.vendor,
        device: device.device,
        class: device.class,
        subclass: device.subclass,
        prog_if: device.prog_if,
        header_type: device.header_type,
        bar_index,
        bar_kind: bar.kind,
        bar_size: bar.size,
        bar_base,
        caps,
        command,
    })
}

/// Tên kiểu BAR để in evidence.
pub const fn bar_kind_name(kind: BarKind) -> &'static str {
    match kind {
        BarKind::Io => "io",
        BarKind::Memory32 => "mem32",
        BarKind::Memory64 => "mem64",
    }
}
