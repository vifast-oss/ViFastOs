// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-402** (D2-259) — `BootInfo` v1: cấu hình nền tảng **đọc từ bảng thật**, dùng chung 3 arch.
//!
//! # Vì sao
//!
//! Trước KT-402, địa chỉ PLIC/GIC/UART/PCIe ECAM, số hart và bản đồ RAM nằm trong **hằng số viết
//! tay** của từng arch (`resources.rs`, `gic.rs`, `plic.rs`, `apic.rs`). Chúng đúng với QEMU `virt`
//! mặc định nhưng **không có cách nào biết** firmware có đưa cấu hình khác hay không — và một
//! hằng sai cho ra MMIO đọc/ghi sai mà không có lỗi biên dịch nào.
//!
//! Tệp này định nghĩa **một** kiểu [`BootInfo`] có phiên bản ([`BOOTINFO_VERSION`]) + hai bộ
//! parser **thuần dữ liệu** (không asm, không MMIO, không `unsafe` — crate này
//! `#![forbid(unsafe_code)]`):
//!
//! - [`parse_fdt`] — FDT/DTB (QEMU `virt` riscv64/aarch64): `/memory`, `/cpus`, PLIC, GIC, UART,
//!   `pci-host-ecam-generic`, `timebase-frequency`;
//! - [`parse_acpi`] — ACPI (x86_64): `MADT` (hart thật + LAPIC), `FADT` (checksum/PM1a), `MCFG`
//!   (PCIe ECAM) + E820 (RAM usable).
//!
//! Phần **chạm bộ nhớ/MMIO** (đọc DTB ở PA, đọc `fw_cfg`) nằm ở crate arch tương ứng
//! (`vifast-plat-{riscv64,aarch64,x86_64}::bootinfo`) — nơi duy nhất có `unsafe`, đúng D2-17.
//!
//! # Nguồn thứ ba: `BootSource::Uefi` (D2-260)
//!
//! Boot stub UEFI (`v2/boot/uefi-x86_64`) chụp `GetMemoryMap` ngay trước `ExitBootServices` và
//! bàn giao **bảng nhớ thật** — không phải ACPI, cũng không phải DTB. Vì vậy [`BootSource`] có
//! thêm `Uefi` (`name() => "uefi"`) và [`BootInfo::validate`] **fail-closed theo nguồn**: `Uefi`
//! chỉ đòi `n_ram > 0` + mọi vùng RAM hợp lệ (ACPI/DTB chưa parse ở bước stub), còn `Dtb`/`Acpi`
//! giữ nguyên 100 % điều kiện nghiêm ngặt. [`BOOTINFO_VERSION`] **giữ nguyên 1**: thêm một biến
//! thể enum không đổi ngữ nghĩa trường nào của struct.
//!
//! # Fail-closed có tên
//!
//! Mọi lỗi là một biến thể **có tên** của [`BootInfoError`]; [`BootInfoError::name`] trả mã chữ
//! ổn định (không phải số trần) để log/runner đối chiếu được. Parser **không panic** và **không
//! đọc ngoài biên**: mọi phép đọc đi qua [`slice`]/[`be32`]/[`le32`] đều kiểm độ dài trước.
//! Bảng hỏng ⇒ `Err`, không có giá trị mặc định nào được dùng thay (đó là "fail-closed").

use core::fmt;

use crate::smp::MAX_HARTS;

/// Phiên bản `BootInfo` (v1) — tăng khi đổi **ngữ nghĩa** trường, không chỉ thêm trường.
pub const BOOTINFO_VERSION: u16 = 1;

/// Số vùng RAM usable tối đa giữ trong `BootInfo` (đủ cho QEMU 3 arch; vượt ⇒ lỗi có tên).
pub const MAX_USABLE_RAM: usize = 16;

/// Cỡ DTB tối đa chấp nhận (byte). DTB QEMU `virt` ~6–10 KiB; trần này chặn `totalsize` rác.
pub const MAX_FDT_BYTES: usize = 1 << 20;

/// Cỡ header chung của một bảng ACPI (`struct acpi_table_header`).
pub const ACPI_HEADER_BYTES: usize = 36;

/// Cỡ một entry E820 của QEMU (`etc/e820`): `addr u64 | size u64 | type u32`.
pub const E820_ENTRY_BYTES: usize = 20;

/// Cỡ một entry memmap PVH (`hvm_memmap_table_entry`): `addr u64 | size u64 | type u32 | reserved u32`.
pub const PVH_MEMMAP_ENTRY_BYTES: usize = 24;

/// `type` của entry E820 "usable RAM".
pub const E820_TYPE_RAM: u32 = 1;

/// Magic FDT (`0xd00d_feed`, big-endian ở offset 0).
pub const FDT_MAGIC: u32 = 0xd00d_feed;

const FDT_BEGIN_NODE: u32 = 1;
const FDT_END_NODE: u32 = 2;
const FDT_PROP: u32 = 3;
const FDT_NOP: u32 = 4;
const FDT_END: u32 = 9;

/// Trần độ sâu cây FDT (QEMU `virt` ~5; trần này chặn vòng lặp rác).
const FDT_MAX_DEPTH: usize = 12;

const NAME_MAX: usize = 48;
const REG_MAX: usize = 96;
const COMPAT_MAX: usize = 160;
const DTYPE_MAX: usize = 24;

/// Kiến trúc của `BootInfo` — quyết định trường nào **bắt buộc** có.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootArch {
    /// `riscv64` (QEMU `virt`, DTB + PLIC).
    Riscv64,
    /// `aarch64` (QEMU `virt`, DTB + GIC).
    Aarch64,
    /// `x86_64` (QEMU `q35`, ACPI + E820).
    X86_64,
}

impl BootArch {
    /// Tên chữ ổn định (dùng trong log `# bootinfo:`).
    pub const fn name(self) -> &'static str {
        match self {
            BootArch::Riscv64 => "riscv64",
            BootArch::Aarch64 => "aarch64",
            BootArch::X86_64 => "x86_64",
        }
    }
}

/// Nguồn cấu hình đã đọc.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootSource {
    /// FDT/DTB do firmware truyền (`riscv64`/`aarch64`).
    Dtb,
    /// Bảng ACPI (`x86_64`).
    Acpi,
    /// Bảng nhớ UEFI (`GetMemoryMap`) do boot stub UEFI bàn giao (`x86_64`, **D2-260**).
    ///
    /// Đây là nguồn **thứ ba**, không phải ACPI: stub UEFI chụp `GetMemoryMap` ngay trước
    /// `ExitBootServices` rồi bàn giao; ACPI/DTB (nếu cần) được parse **sau** từ
    /// `EFI_CONFIGURATION_TABLE` (RSDP/DTB PA ghi trong bản ghi bàn giao). Vì vậy
    /// [`BootInfo::validate`] **fail-closed theo nguồn**: `Uefi` chỉ đòi `n_ram > 0` + mọi vùng
    /// RAM hợp lệ, còn `Dtb`/`Acpi` giữ nguyên 100 % điều kiện nghiêm ngặt.
    Uefi,
}

impl BootSource {
    /// Tên chữ ổn định (dùng trong log).
    pub const fn name(self) -> &'static str {
        match self {
            BootSource::Dtb => "dtb",
            BootSource::Acpi => "acpi",
            BootSource::Uefi => "uefi",
        }
    }
}

/// Loại UART mà firmware khai.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UartKind {
    /// 16550A/NS16550 (QEMU `virt` riscv64, COM1 x86_64).
    Ns16550,
    /// ARM PL011 (QEMU `virt` aarch64).
    Pl011,
}

impl UartKind {
    /// Tên chữ ổn định (dùng trong log).
    pub const fn name(self) -> &'static str {
        match self {
            UartKind::Ns16550 => "ns16550",
            UartKind::Pl011 => "pl011",
        }
    }
}

/// Một vùng RAM usable `[base, base + size)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RamRegion {
    /// Địa chỉ vật lý bắt đầu.
    pub base: u64,
    /// Kích thước (byte).
    pub size: u64,
}

impl RamRegion {
    /// Địa chỉ kết thúc **không bao gồm** (bão hoà khi tràn).
    pub const fn end(&self) -> u64 {
        self.base.saturating_add(self.size)
    }

    /// `true` ⇔ vùng hợp lệ (kích thước > 0, không tràn địa chỉ).
    ///
    /// **Không** đòi căn 4 KiB: E820/memmap PVH khai theo **byte** (QEMU `q35` có vùng usable
    /// `0x9fc00` byte). Bên dùng phải tự căn trang khi cấp phát — parser không được bỏ vùng thật.
    pub const fn is_valid(&self) -> bool {
        self.size > 0 && self.base.checked_add(self.size).is_some()
    }
}

/// UART mà firmware khai: địa chỉ MMIO (hoặc I/O port với COM1) + loại.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Uart {
    /// Địa chỉ gốc.
    pub base: u64,
    /// Loại thiết bị.
    pub kind: UartKind,
}

/// **`BootInfo` v1** — cấu hình nền tảng dùng chung 3 arch, mọi trường **đọc từ bảng thật**.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BootInfo {
    /// Phiên bản cấu trúc ([`BOOTINFO_VERSION`]).
    pub version: u16,
    /// Kiến trúc.
    pub arch: BootArch,
    /// Nguồn cấu hình.
    pub source: BootSource,
    /// Hart đang chạy boot (FDT `boot_cpuid_phys` / MADT processor id).
    pub boot_hart: u32,
    /// Số hart **thật** đếm được từ bảng (không phải hằng `-smp`).
    pub n_harts: u16,
    /// Id các hart theo thứ tự bảng.
    pub hart_ids: [u32; MAX_HARTS],
    /// Bản đồ vùng RAM usable.
    pub ram: [RamRegion; MAX_USABLE_RAM],
    /// Số vùng RAM đã ghi (`<= MAX_USABLE_RAM`).
    pub n_ram: u8,
    /// PLIC base (`riscv64`).
    pub plic: Option<u64>,
    /// GIC distributor base (`aarch64`).
    pub gic: Option<u64>,
    /// GIC CPU interface / redistributor base (`aarch64`).
    pub gic_cpu: Option<u64>,
    /// LAPIC base (`x86_64`, từ `MADT`).
    pub apic: Option<u64>,
    /// UART firmware khai.
    pub uart: Option<Uart>,
    /// PCIe ECAM base.
    pub pcie_ecam: Option<u64>,
    /// Tần số timebase (Hz) — có trên DTB (`/cpus/timebase-frequency`).
    pub timebase_hz: Option<u64>,
    /// `PM1a_CNT_BLK` từ FADT (`x86_64`) — bằng chứng FADT đã parse, không chỉ kiểm chữ ký.
    pub pm1a_cnt: Option<u64>,
}

impl BootInfo {
    /// Khởi tạo rỗng cho một arch/nguồn; parser điền dần và **kiểm đủ trường** trước khi trả.
    pub const fn new(arch: BootArch, source: BootSource) -> Self {
        BootInfo {
            version: BOOTINFO_VERSION,
            arch,
            source,
            boot_hart: 0,
            n_harts: 0,
            hart_ids: [0; MAX_HARTS],
            ram: [RamRegion { base: 0, size: 0 }; MAX_USABLE_RAM],
            n_ram: 0,
            plic: None,
            gic: None,
            gic_cpu: None,
            apic: None,
            uart: None,
            pcie_ecam: None,
            timebase_hz: None,
            pm1a_cnt: None,
        }
    }

    /// Các vùng RAM usable đã ghi.
    pub fn usable_ram(&self) -> &[RamRegion] {
        &self.ram[..usize::from(self.n_ram)]
    }

    /// Id các hart đã ghi.
    pub fn harts(&self) -> &[u32] {
        &self.hart_ids[..usize::from(self.n_harts)]
    }

    /// Tổng dung lượng RAM usable (byte), bão hoà khi tràn.
    pub fn usable_ram_bytes(&self) -> u64 {
        self.usable_ram()
            .iter()
            .fold(0u64, |acc, r| acc.saturating_add(r.size))
    }

    /// Ghi một vùng RAM usable; `size == 0` bị **bỏ qua** (không phải lỗi), vượt trần ⇒ lỗi.
    pub fn push_ram(&mut self, base: u64, size: u64) -> Result<(), BootInfoError> {
        if size == 0 {
            return Ok(());
        }
        let n = usize::from(self.n_ram);
        if n >= MAX_USABLE_RAM {
            return Err(BootInfoError::TooManyRegions);
        }
        let region = RamRegion { base, size };
        if !region.is_valid() {
            return Err(BootInfoError::BadRegion);
        }
        self.ram[n] = region;
        self.n_ram += 1;
        Ok(())
    }

    /// Ghi một hart id; trùng id **không** tính hai lần; vượt trần ⇒ lỗi.
    pub fn push_hart(&mut self, id: u32) -> Result<(), BootInfoError> {
        if self.harts().contains(&id) {
            return Ok(());
        }
        let n = usize::from(self.n_harts);
        if n >= MAX_HARTS {
            return Err(BootInfoError::TooManyHarts);
        }
        self.hart_ids[n] = id;
        self.n_harts += 1;
        Ok(())
    }

    /// Kiểm **đủ trường bắt buộc** theo arch — parser gọi trước khi trả `Ok`.
    ///
    /// **Fail-closed theo nguồn** (D2-260): `Dtb`/`Acpi` giữ nguyên 100 % điều kiện nghiêm ngặt
    /// (hart + PLIC/GIC/APIC + UART + PCIe ECAM); `Uefi` chỉ đòi `n_ram > 0` + **mọi** vùng RAM
    /// hợp lệ, vì ở bước stub bàn giao `GetMemoryMap` chưa có ACPI/DTB để điền các trường còn lại.
    /// Không nguồn nào được phép "thiếu RAM" hay có vùng RAM tràn địa chỉ.
    pub fn validate(&self) -> Result<(), BootInfoError> {
        if self.version != BOOTINFO_VERSION {
            return Err(BootInfoError::UnsupportedVersion {
                what: "bootinfo",
                got: u32::from(self.version),
            });
        }
        if self.n_ram == 0 {
            return Err(BootInfoError::MissingMemory);
        }
        if self.source == BootSource::Uefi {
            // Bảng nhớ UEFI là nguồn **duy nhất** đã có ở bước stub: chỉ kiểm phần nó khai.
            for r in self.usable_ram() {
                if !r.is_valid() {
                    return Err(BootInfoError::BadRegion);
                }
            }
            return Ok(());
        }
        if self.n_harts == 0 {
            return Err(BootInfoError::MissingHarts);
        }
        if !self.harts().contains(&self.boot_hart) {
            return Err(BootInfoError::MissingProperty("boot-hart"));
        }
        match self.arch {
            BootArch::Riscv64 => {
                if self.plic.is_none() {
                    return Err(BootInfoError::MissingNode("plic"));
                }
            }
            BootArch::Aarch64 => {
                if self.gic.is_none() {
                    return Err(BootInfoError::MissingNode("gic"));
                }
            }
            BootArch::X86_64 => {
                if self.apic.is_none() {
                    return Err(BootInfoError::MissingNode("apic"));
                }
            }
        }
        if self.uart.is_none() {
            return Err(BootInfoError::MissingNode("uart"));
        }
        if self.pcie_ecam.is_none() {
            return Err(BootInfoError::MissingNode("pcie-ecam"));
        }
        Ok(())
    }

    /// Ghi các dòng `# bootinfo:` — **một** bản dùng chung 3 arch (không chép ở kbin).
    pub fn write_log<W: fmt::Write>(&self, w: &mut W) -> fmt::Result {
        writeln!(
            w,
            "# bootinfo: v={} arch={} source={} n_harts={} boot_hart={}",
            self.version,
            self.arch.name(),
            self.source.name(),
            self.n_harts,
            self.boot_hart,
        )?;
        writeln!(
            w,
            "# bootinfo: ram n={} bytes=0x{:x}",
            self.n_ram,
            self.usable_ram_bytes()
        )?;
        for (i, r) in self.usable_ram().iter().enumerate() {
            writeln!(w, "# bootinfo: ram[{i}]=0x{:x}+0x{:x}", r.base, r.size)?;
        }
        write!(w, "# bootinfo: harts=")?;
        for (i, h) in self.harts().iter().enumerate() {
            if i > 0 {
                write!(w, ",")?;
            }
            write!(w, "{h}")?;
        }
        write_opt(w, "timebase", self.timebase_hz)?;
        writeln!(w)?;
        write!(w, "# bootinfo:")?;
        write_opt(w, "plic", self.plic)?;
        write_opt(w, "gic", self.gic)?;
        write_opt(w, "gic_cpu", self.gic_cpu)?;
        write_opt(w, "apic", self.apic)?;
        match self.uart {
            Some(u) => write!(w, " uart={}@0x{:x}", u.kind.name(), u.base)?,
            None => write!(w, " uart=none")?,
        }
        write_opt(w, "ecam", self.pcie_ecam)?;
        writeln!(w)?;
        write!(
            w,
            "# bootinfo: acpi={}",
            if self.source == BootSource::Acpi {
                "yes"
            } else {
                "no"
            }
        )?;
        match self.pm1a_cnt {
            Some(v) => writeln!(w, " pm1a=0x{v:x}")?,
            None => writeln!(w, " pm1a=none")?,
        }
        Ok(())
    }
}

fn write_opt<W: fmt::Write>(w: &mut W, name: &str, value: Option<u64>) -> fmt::Result {
    match value {
        Some(v) => write!(w, " {name}=0x{v:x}"),
        None => write!(w, " {name}=none"),
    }
}

/// Lỗi **có tên** của `BootInfo` — `name()` trả mã chữ ổn định cho log/runner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootInfoError {
    /// Địa chỉ bảng bằng 0 (firmware không truyền).
    NullPointer,
    /// Độ dài khai báo vượt dữ liệu thật (không đọc ngoài biên).
    Truncated {
        /// Số byte cần.
        needed: usize,
        /// Số byte có.
        have: usize,
    },
    /// Magic sai.
    BadMagic {
        /// Bảng nào.
        what: &'static str,
        /// Giá trị đọc được.
        got: u32,
    },
    /// Checksum sai (ACPI: tổng byte phải bằng 0 theo modulo 256).
    BadChecksum {
        /// Bảng nào.
        what: &'static str,
    },
    /// Offset/lệch căn sai trong bảng.
    BadOffset {
        /// Vùng nào.
        what: &'static str,
    },
    /// Cấu trúc bảng hỏng (token lạ, chuỗi không kết thúc, cây sai).
    BadStructure {
        /// Vùng nào.
        what: &'static str,
    },
    /// Phiên bản bảng không hỗ trợ.
    UnsupportedVersion {
        /// Bảng nào.
        what: &'static str,
        /// Phiên bản đọc được.
        got: u32,
    },
    /// Thiếu node bắt buộc (DTB).
    MissingNode(&'static str),
    /// Thiếu thuộc tính bắt buộc (DTB).
    MissingProperty(&'static str),
    /// Thiếu bảng bắt buộc (ACPI).
    MissingTable(&'static str),
    /// Bảng có nhưng nội dung hỏng.
    BadTable(&'static str),
    /// Không có vùng RAM usable nào.
    MissingMemory,
    /// Không có hart nào.
    MissingHarts,
    /// Vượt trần số vùng RAM.
    TooManyRegions,
    /// Vượt trần số hart.
    TooManyHarts,
    /// Vùng RAM không hợp lệ (kích thước 0/không căn trang/tràn địa chỉ).
    BadRegion,
    /// Đọc ngoài biên.
    OutOfBounds,
}

impl BootInfoError {
    /// Mã chữ **ổn định** (dùng trong `# bootinfo: fail-closed err=<mã>`).
    pub const fn name(&self) -> &'static str {
        match self {
            BootInfoError::NullPointer => "null-pointer",
            BootInfoError::Truncated { .. } => "truncated",
            BootInfoError::BadMagic { .. } => "bad-magic",
            BootInfoError::BadChecksum { .. } => "bad-checksum",
            BootInfoError::BadOffset { .. } => "bad-offset",
            BootInfoError::BadStructure { .. } => "bad-structure",
            BootInfoError::UnsupportedVersion { .. } => "unsupported-version",
            BootInfoError::MissingNode(_) => "missing-node",
            BootInfoError::MissingProperty(_) => "missing-property",
            BootInfoError::MissingTable(_) => "missing-table",
            BootInfoError::BadTable(_) => "bad-table",
            BootInfoError::MissingMemory => "missing-memory",
            BootInfoError::MissingHarts => "missing-harts",
            BootInfoError::TooManyRegions => "too-many-regions",
            BootInfoError::TooManyHarts => "too-many-harts",
            BootInfoError::BadRegion => "bad-region",
            BootInfoError::OutOfBounds => "out-of-bounds",
        }
    }

    /// Chi tiết **chữ** của lỗi (tên node/bảng), `""` nếu không có.
    pub const fn detail(&self) -> &'static str {
        match self {
            BootInfoError::MissingNode(s)
            | BootInfoError::MissingProperty(s)
            | BootInfoError::MissingTable(s)
            | BootInfoError::BadTable(s) => s,
            _ => "",
        }
    }
}

impl fmt::Display for BootInfoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())?;
        if !self.detail().is_empty() {
            write!(f, ":{}", self.detail())?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------
// Đọc số nguyên có kiểm biên (không panic, không đọc ngoài biên)
// ---------------------------------------------------------------------------------------------

fn slice(bytes: &[u8], off: usize, len: usize) -> Result<&[u8], BootInfoError> {
    let end = off.checked_add(len).ok_or(BootInfoError::OutOfBounds)?;
    bytes.get(off..end).ok_or(BootInfoError::OutOfBounds)
}

fn be32(bytes: &[u8], off: usize) -> Result<u32, BootInfoError> {
    let b = slice(bytes, off, 4)?;
    Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

fn le32(bytes: &[u8], off: usize) -> Result<u32, BootInfoError> {
    let b = slice(bytes, off, 4)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn le64(bytes: &[u8], off: usize) -> Result<u64, BootInfoError> {
    let b = slice(bytes, off, 8)?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}

fn align4(pos: usize) -> Result<usize, BootInfoError> {
    pos.checked_add(3)
        .map(|v| v & !3)
        .ok_or(BootInfoError::OutOfBounds)
}

// ---------------------------------------------------------------------------------------------
// FDT/DTB
// ---------------------------------------------------------------------------------------------

/// Một node của cây FDT đang được gom thuộc tính (thuộc tính luôn đứng **trước** node con).
#[derive(Clone, Copy)]
struct Pending {
    kind: NodeKind,
    name: [u8; NAME_MAX],
    name_len: usize,
    reg: [u8; REG_MAX],
    reg_len: usize,
    compat: [u8; COMPAT_MAX],
    compat_len: usize,
    device_type: [u8; DTYPE_MAX],
    dtype_len: usize,
    status_disabled: bool,
    timebase: Option<u32>,
}

impl Pending {
    fn new(kind: NodeKind) -> Self {
        Pending {
            kind,
            name: [0; NAME_MAX],
            name_len: 0,
            reg: [0; REG_MAX],
            reg_len: 0,
            compat: [0; COMPAT_MAX],
            compat_len: 0,
            device_type: [0; DTYPE_MAX],
            dtype_len: 0,
            status_disabled: false,
            timebase: None,
        }
    }

    fn set_name(&mut self, bytes: &[u8]) {
        let n = bytes.len().min(NAME_MAX);
        self.name[..n].copy_from_slice(&bytes[..n]);
        self.name_len = n;
    }

    fn name_str(&self) -> &str {
        core::str::from_utf8(&self.name[..self.name_len]).unwrap_or("")
    }

    fn compat_has(&self, needle: &str) -> bool {
        self.compat[..self.compat_len]
            .split(|&b| b == 0)
            .any(|item| item == needle.as_bytes())
    }

    fn device_type_is(&self, want: &[u8]) -> bool {
        &self.device_type[..self.dtype_len] == want
    }
}

/// Loại node theo vị trí trong cây — quyết định cách diễn giải thuộc tính.
#[derive(Clone, Copy, PartialEq, Eq)]
enum NodeKind {
    Root,
    Cpus,
    Cpu,
    Memory,
    Other,
}

#[derive(Clone, Copy)]
struct CellSpec {
    addr: u32,
    size: u32,
}

const DEFAULT_CELLS: CellSpec = CellSpec { addr: 2, size: 2 };

struct Fdt<'a> {
    structs: &'a [u8],
    strings: &'a [u8],
    boot_cpuid: u32,
}

impl<'a> Fdt<'a> {
    fn parse(data: &'a [u8]) -> Result<Self, BootInfoError> {
        if data.len() < 40 {
            return Err(BootInfoError::Truncated {
                needed: 40,
                have: data.len(),
            });
        }
        let magic = be32(data, 0)?;
        if magic != FDT_MAGIC {
            return Err(BootInfoError::BadMagic {
                what: "fdt",
                got: magic,
            });
        }
        let totalsize = be32(data, 4)? as usize;
        if totalsize < 40 || totalsize > data.len() || totalsize > MAX_FDT_BYTES {
            return Err(BootInfoError::Truncated {
                needed: totalsize,
                have: data.len(),
            });
        }
        let data = &data[..totalsize];
        let off_struct = be32(data, 8)? as usize;
        let off_strings = be32(data, 12)? as usize;
        let off_rsvmap = be32(data, 16)? as usize;
        let version = be32(data, 20)?;
        let last_comp = be32(data, 24)?;
        let boot_cpuid = be32(data, 28)?;
        let size_strings = be32(data, 32)? as usize;
        let size_struct = be32(data, 36)? as usize;
        if version < 16 {
            return Err(BootInfoError::UnsupportedVersion {
                what: "fdt",
                got: version,
            });
        }
        if last_comp > version {
            return Err(BootInfoError::UnsupportedVersion {
                what: "fdt-compat",
                got: last_comp,
            });
        }
        if !off_struct.is_multiple_of(4) {
            return Err(BootInfoError::BadOffset { what: "fdt-struct" });
        }
        let struct_end = off_struct
            .checked_add(size_struct)
            .ok_or(BootInfoError::BadOffset { what: "fdt-struct" })?;
        let strings_end =
            off_strings
                .checked_add(size_strings)
                .ok_or(BootInfoError::BadOffset {
                    what: "fdt-strings",
                })?;
        if struct_end > totalsize || strings_end > totalsize || size_struct < 4 {
            return Err(BootInfoError::BadOffset { what: "fdt-block" });
        }
        if off_rsvmap.checked_add(16).is_none_or(|end| end > totalsize) {
            return Err(BootInfoError::BadOffset { what: "fdt-rsvmap" });
        }
        Ok(Fdt {
            structs: &data[off_struct..struct_end],
            strings: &data[off_strings..strings_end],
            boot_cpuid,
        })
    }

    fn string(&self, off: usize) -> Result<&'a str, BootInfoError> {
        let tail = self
            .strings
            .get(off..)
            .ok_or(BootInfoError::BadOffset { what: "fdt-string" })?;
        let len = tail
            .iter()
            .position(|&c| c == 0)
            .ok_or(BootInfoError::BadStructure { what: "fdt-string" })?;
        core::str::from_utf8(&tail[..len]).map_err(|_| BootInfoError::BadStructure {
            what: "fdt-string-utf8",
        })
    }
}

/// Đọc các cặp `(base, size)` của thuộc tính `reg` theo `cells` của node cha.
fn for_each_reg(
    reg: &[u8],
    cells: CellSpec,
    mut f: impl FnMut(u64, u64) -> Result<(), BootInfoError>,
) -> Result<(), BootInfoError> {
    if cells.addr == 0 || cells.addr > 2 || cells.size > 2 {
        return Err(BootInfoError::BadStructure { what: "fdt-cells" });
    }
    let entry_bytes = 4 * (cells.addr as usize + cells.size as usize);
    if reg.len() < entry_bytes {
        return Err(BootInfoError::Truncated {
            needed: entry_bytes,
            have: reg.len(),
        });
    }
    for chunk in reg.chunks_exact(entry_bytes) {
        let mut base = 0u64;
        for i in 0..cells.addr as usize {
            base = (base << 32) | u64::from(be32(chunk, 4 * i)?);
        }
        let mut size = 0u64;
        for i in 0..cells.size as usize {
            size = (size << 32) | u64::from(be32(chunk, 4 * (cells.addr as usize + i))?);
        }
        f(base, size)?;
    }
    Ok(())
}

fn reg_first(reg: &[u8], cells: CellSpec) -> Result<Option<u64>, BootInfoError> {
    let mut first = None;
    for_each_reg(reg, cells, |base, _size| {
        if first.is_none() {
            first = Some(base);
        }
        Ok(())
    })?;
    Ok(first)
}

fn reg_bases(reg: &[u8], cells: CellSpec) -> Result<(Option<u64>, Option<u64>), BootInfoError> {
    let mut bases = [None, None];
    let mut i = 0usize;
    for_each_reg(reg, cells, |base, _size| {
        if i < 2 {
            bases[i] = Some(base);
        }
        i += 1;
        Ok(())
    })?;
    Ok((bases[0], bases[1]))
}

/// Một ứng viên UART gom trong lúc đi cây (chọn theo `/chosen/stdout-path` sau cùng).
#[derive(Clone, Copy)]
struct UartCand {
    base: u64,
    kind: UartKind,
    name: [u8; NAME_MAX],
    name_len: usize,
}

impl UartCand {
    const EMPTY: UartCand = UartCand {
        base: 0,
        kind: UartKind::Ns16550,
        name: [0; NAME_MAX],
        name_len: 0,
    };

    fn name_str(&self) -> &str {
        core::str::from_utf8(&self.name[..self.name_len]).unwrap_or("")
    }
}

struct Walk {
    info: BootInfo,
    chosen_stdout: [u8; NAME_MAX],
    chosen_len: usize,
    uart: [UartCand; 4],
    n_uart: usize,
}

impl Walk {
    fn new(arch: BootArch, boot_cpuid: u32) -> Self {
        let mut info = BootInfo::new(arch, BootSource::Dtb);
        info.boot_hart = boot_cpuid;
        Walk {
            info,
            chosen_stdout: [0; NAME_MAX],
            chosen_len: 0,
            uart: [UartCand::EMPTY; 4],
            n_uart: 0,
        }
    }

    fn set_stdout(&mut self, value: &[u8]) {
        // `/chosen/stdout-path` có thể kèm `:<baud>` — chỉ giữ phần đường dẫn.
        let end = value.iter().position(|&b| b == b':').unwrap_or(value.len());
        let n = end.min(NAME_MAX);
        self.chosen_stdout[..n].copy_from_slice(&value[..n]);
        self.chosen_len = n;
    }

    fn add_uart(&mut self, base: u64, kind: UartKind, name: &[u8]) {
        if self.n_uart >= self.uart.len() {
            return;
        }
        let mut cand = UartCand {
            base,
            kind,
            ..UartCand::EMPTY
        };
        let n = name.len().min(NAME_MAX);
        cand.name[..n].copy_from_slice(&name[..n]);
        cand.name_len = n;
        self.uart[self.n_uart] = cand;
        self.n_uart += 1;
    }

    fn finish_node(&mut self, p: &Pending, parent_cells: CellSpec) -> Result<(), BootInfoError> {
        match p.kind {
            NodeKind::Root => {}
            NodeKind::Cpus => {
                if let Some(f) = p.timebase {
                    self.info.timebase_hz = Some(u64::from(f));
                }
            }
            NodeKind::Cpu => {
                if p.status_disabled {
                    return Ok(());
                }
                let id = reg_first(&p.reg[..p.reg_len], parent_cells)?;
                let id = id.ok_or(BootInfoError::MissingProperty("cpu-reg"))?;
                if id > u64::from(u32::MAX) {
                    return Err(BootInfoError::BadStructure { what: "cpu-id" });
                }
                self.info.push_hart(id as u32)?;
            }
            NodeKind::Memory => {
                if p.device_type_is(b"memory") || p.name_str().starts_with("memory") {
                    let mut info = self.info;
                    for_each_reg(&p.reg[..p.reg_len], parent_cells, |base, size| {
                        info.push_ram(base, size)
                    })?;
                    self.info = info;
                }
            }
            NodeKind::Other => {
                let reg = &p.reg[..p.reg_len];
                if p.compat_has("riscv,plic0")
                    || p.compat_has("sifive,plic-1.0.0")
                    || p.compat_has("riscv,plic")
                {
                    if let Some(base) = reg_first(reg, parent_cells)? {
                        self.info.plic = Some(base);
                    }
                }
                if p.compat_has("arm,gic-v3")
                    || p.compat_has("arm,cortex-a15-gic")
                    || p.compat_has("arm,gic-400")
                {
                    let (dist, cpu) = reg_bases(reg, parent_cells)?;
                    if let Some(base) = dist {
                        self.info.gic = Some(base);
                    }
                    if let Some(base) = cpu {
                        self.info.gic_cpu = Some(base);
                    }
                }
                if p.compat_has("pci-host-ecam-generic") {
                    if let Some(base) = reg_first(reg, parent_cells)? {
                        self.info.pcie_ecam = Some(base);
                    }
                }
                if p.compat_has("ns16550a") || p.compat_has("ns16550") {
                    if let Some(base) = reg_first(reg, parent_cells)? {
                        self.add_uart(base, UartKind::Ns16550, &p.name[..p.name_len]);
                    }
                }
                if p.compat_has("arm,pl011") {
                    if let Some(base) = reg_first(reg, parent_cells)? {
                        self.add_uart(base, UartKind::Pl011, &p.name[..p.name_len]);
                    }
                }
            }
        }
        Ok(())
    }

    fn choose_uart(&mut self) -> Result<(), BootInfoError> {
        if self.n_uart == 0 {
            return Ok(());
        }
        let chosen = &self.chosen_stdout[..self.chosen_len];
        let chosen_name = chosen.rsplit(|&b| b == b'/').next().unwrap_or(&[]);
        let mut pick = 0usize;
        if !chosen_name.is_empty() {
            if let Some(i) =
                (0..self.n_uart).find(|&i| self.uart[i].name_str().as_bytes() == chosen_name)
            {
                pick = i;
            }
        }
        let cand = self.uart[pick];
        self.info.uart = Some(Uart {
            base: cand.base,
            kind: cand.kind,
        });
        Ok(())
    }
}

fn classify(parent: NodeKind, name: &[u8]) -> NodeKind {
    // `cpu@N` — **không** phải `cpu-map` (node con của `/cpus` trong DTB QEMU riscv64, không có
    // `reg`; nhận nhầm nó là CPU sẽ đòi `reg` và fail-closed oan).
    if parent == NodeKind::Cpus && name.starts_with(b"cpu@") {
        NodeKind::Cpu
    } else if name == b"cpus" {
        NodeKind::Cpus
    } else if name.starts_with(b"memory") {
        NodeKind::Memory
    } else {
        NodeKind::Other
    }
}

fn apply_prop(
    p: &mut Pending,
    cells: &mut CellSpec,
    name: &str,
    value: &[u8],
    walk: &mut Walk,
) -> Result<(), BootInfoError> {
    match name {
        // Hình học cell **chỉ áp cho node con**; node khai có thể dùng 0/3 cell (PLIC khai
        // `#address-cells = <0>`, PCIe khai 3) — không được từ chối cả cây vì điều đó. Kiểm
        // `1..=2` diễn ra ở [`for_each_reg`] khi thực sự đọc `reg` của node con.
        "#address-cells" => cells.addr = be32(value, 0)?,
        "#size-cells" => cells.size = be32(value, 0)?,
        "reg" => {
            if value.len() > REG_MAX {
                return Err(BootInfoError::Truncated {
                    needed: value.len(),
                    have: REG_MAX,
                });
            }
            p.reg[..value.len()].copy_from_slice(value);
            p.reg_len = value.len();
        }
        "compatible" => {
            let n = value.len().min(COMPAT_MAX);
            p.compat[..n].copy_from_slice(&value[..n]);
            p.compat_len = n;
        }
        "device_type" => {
            let value = value.strip_suffix(b"\0").unwrap_or(value);
            let n = value.len().min(DTYPE_MAX);
            p.device_type[..n].copy_from_slice(&value[..n]);
            p.dtype_len = n;
        }
        "status" => {
            let value = value.strip_suffix(b"\0").unwrap_or(value);
            p.status_disabled = value == b"disabled";
        }
        "timebase-frequency" => p.timebase = Some(be32(value, 0)?),
        "stdout-path" | "linux,stdout-path" => walk.set_stdout(value),
        _ => {}
    }
    Ok(())
}

/// Duyệt cây FDT một lượt, điền [`BootInfo`]. Mọi offset/token đều kiểm biên trước khi đọc.
fn walk_fdt(fdt: &Fdt<'_>, arch: BootArch) -> Result<BootInfo, BootInfoError> {
    let mut walk = Walk::new(arch, fdt.boot_cpuid);
    let mut cells = [DEFAULT_CELLS; FDT_MAX_DEPTH + 1];
    let mut kinds = [NodeKind::Other; FDT_MAX_DEPTH + 1];
    let mut depth = 0usize;
    let mut started = false;
    let mut pending: Option<Pending> = None;
    // Node đã được chốt lúc gặp con đầu tiên (thuộc tính luôn đứng trước con) — để `END_NODE`
    // của node đó không đòi `pending` lần nữa.
    let mut finalized = [false; FDT_MAX_DEPTH + 1];
    let mut root_closed = false;
    let mut pos = 0usize;

    loop {
        let token = be32(fdt.structs, pos)?;
        pos += 4;
        match token {
            FDT_BEGIN_NODE => {
                if root_closed {
                    return Err(BootInfoError::BadStructure {
                        what: "fdt-multi-root",
                    });
                }
                if !started {
                    depth = 0;
                    cells[0] = DEFAULT_CELLS;
                    kinds[0] = NodeKind::Root;
                    started = true;
                } else {
                    if let Some(parent) = pending.take() {
                        let parent_cells = if depth == 0 {
                            DEFAULT_CELLS
                        } else {
                            cells[depth - 1]
                        };
                        walk.finish_node(&parent, parent_cells)?;
                        finalized[depth] = true;
                    }
                    depth += 1;
                    if depth > FDT_MAX_DEPTH {
                        return Err(BootInfoError::BadStructure { what: "fdt-depth" });
                    }
                    cells[depth] = cells[depth - 1];
                }
                let tail = fdt.structs.get(pos..).ok_or(BootInfoError::OutOfBounds)?;
                let name_len = tail
                    .iter()
                    .position(|&b| b == 0)
                    .ok_or(BootInfoError::BadStructure { what: "fdt-name" })?;
                let name = &tail[..name_len];
                pos = align4(pos + name_len + 1)?;
                let kind = if depth == 0 {
                    NodeKind::Root
                } else {
                    classify(kinds[depth - 1], name)
                };
                kinds[depth] = kind;
                let mut p = Pending::new(kind);
                p.set_name(name);
                pending = Some(p);
            }
            FDT_PROP => {
                let mut p = pending.take().ok_or(BootInfoError::BadStructure {
                    what: "fdt-prop-outside-node",
                })?;
                let len = be32(fdt.structs, pos)? as usize;
                pos += 4;
                let nameoff = be32(fdt.structs, pos)? as usize;
                pos += 4;
                let value = slice(fdt.structs, pos, len)?;
                pos = align4(pos + len)?;
                let pname = fdt.string(nameoff)?;
                apply_prop(&mut p, &mut cells[depth], pname, value, &mut walk)?;
                pending = Some(p);
            }
            FDT_END_NODE => {
                let parent_cells = if depth == 0 {
                    DEFAULT_CELLS
                } else {
                    cells[depth - 1]
                };
                if let Some(p) = pending.take() {
                    walk.finish_node(&p, parent_cells)?;
                } else if finalized[depth] {
                    finalized[depth] = false;
                } else {
                    return Err(BootInfoError::BadStructure {
                        what: "fdt-end-node",
                    });
                }
                if depth == 0 {
                    root_closed = true;
                } else {
                    depth -= 1;
                }
            }
            FDT_NOP => {}
            FDT_END => {
                if pending.is_some() || !root_closed {
                    return Err(BootInfoError::BadStructure { what: "fdt-end" });
                }
                break;
            }
            _ => return Err(BootInfoError::BadStructure { what: "fdt-token" }),
        }
    }
    walk.choose_uart()?;
    Ok(walk.info)
}

/// Parse FDT/DTB thành [`BootInfo`] cho `arch` — **không** panic, **không** đọc ngoài biên.
///
/// Trả lỗi có tên khi: magic sai, `totalsize` vượt dữ liệu, offset/khối tràn, token lạ, chuỗi
/// không kết thúc, thiếu `/memory`/`/cpus`, thiếu PLIC/GIC/UART/ECAM theo arch.
pub fn parse_fdt(data: &[u8], arch: BootArch) -> Result<BootInfo, BootInfoError> {
    if arch == BootArch::X86_64 {
        return Err(BootInfoError::BadStructure { what: "fdt-on-x86" });
    }
    let fdt = Fdt::parse(data)?;
    let info = walk_fdt(&fdt, arch)?;
    info.validate()?;
    Ok(info)
}

// ---------------------------------------------------------------------------------------------
// ACPI
// ---------------------------------------------------------------------------------------------

/// `true` ⇔ checksum ACPI hợp lệ (tổng mọi byte ≡ 0 mod 256).
pub fn acpi_checksum_ok(table: &[u8]) -> bool {
    table.iter().fold(0u8, |acc, &b| acc.wrapping_add(b)) == 0
}

/// Tìm bảng ACPI theo chữ ký trong **blob nối tiếp** (`etc/acpi/tables` của QEMU fw_cfg).
///
/// Chỉ kiểm checksum của bảng **khớp chữ ký** (bảng khác có thể còn trỏ tương đối chưa vá bởi
/// firmware); độ dài thì kiểm cho **mọi** bảng để không trượt sang rác.
pub fn find_acpi_table<'a>(
    blob: &'a [u8],
    sig: &[u8; 4],
    name: &'static str,
) -> Result<&'a [u8], BootInfoError> {
    let mut pos = 0usize;
    while pos + ACPI_HEADER_BYTES <= blob.len() {
        let table = &blob[pos..];
        let len = le32(table, 4)? as usize;
        if len < ACPI_HEADER_BYTES || len > table.len() {
            return Err(BootInfoError::BadTable(name));
        }
        if &table[..4] == sig {
            let table = &table[..len];
            if !acpi_checksum_ok(table) {
                return Err(BootInfoError::BadChecksum { what: name });
            }
            return Ok(table);
        }
        pos += len;
    }
    Err(BootInfoError::MissingTable(name))
}

/// `MADT` (APIC) đã parse: hart thật + LAPIC + IOAPIC.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Madt {
    /// Địa chỉ LAPIC MMIO (`MADT.Local APIC Address`).
    pub local_apic: u64,
    /// Địa chỉ IOAPIC đầu tiên (nếu có).
    pub ioapic: Option<u64>,
    /// Số CPU **enabled** đếm được.
    pub n_cpus: u16,
    /// Id các CPU enabled (x2APIC id nếu có, ngược lại APIC id 8-bit).
    pub cpu_ids: [u32; MAX_HARTS],
}

/// Parse `MADT` — kiểm chữ ký, độ dài, checksum, từng entry (không đọc quá `length`).
pub fn parse_madt(table: &[u8]) -> Result<Madt, BootInfoError> {
    if table.len() < 44 {
        return Err(BootInfoError::Truncated {
            needed: 44,
            have: table.len(),
        });
    }
    if &table[..4] != b"APIC" {
        return Err(BootInfoError::BadTable("madt-sig"));
    }
    let len = le32(table, 4)? as usize;
    if len < 44 || len > table.len() {
        return Err(BootInfoError::BadTable("madt-length"));
    }
    let table = &table[..len];
    if !acpi_checksum_ok(table) {
        return Err(BootInfoError::BadChecksum { what: "madt" });
    }
    let local_apic = u64::from(le32(table, 36)?);
    if local_apic == 0 {
        return Err(BootInfoError::MissingNode("apic"));
    }
    let mut madt = Madt {
        local_apic,
        ioapic: None,
        n_cpus: 0,
        cpu_ids: [0; MAX_HARTS],
    };
    let mut pos = 44usize;
    while pos + 2 <= len {
        let ty = table[pos];
        let elen = usize::from(table[pos + 1]);
        if elen < 2 || pos + elen > len {
            return Err(BootInfoError::BadTable("madt-entry"));
        }
        let entry = &table[pos..pos + elen];
        match ty {
            // Processor Local APIC: type,len,acpi_id,apic_id,flags.
            0 if elen >= 8 => {
                let apic_id = u32::from(entry[3]);
                let flags = le32(entry, 4)?;
                if flags & 1 != 0 {
                    madt.push_cpu(apic_id)?;
                }
            }
            // IOAPIC: type,len,id,reserved,address,gsi_base.
            1 if elen >= 12 => {
                if madt.ioapic.is_none() {
                    madt.ioapic = Some(u64::from(le32(entry, 4)?));
                }
            }
            // Processor Local x2APIC: type,len,reserved,x2apic_id,flags,uid.
            9 if elen >= 16 => {
                let apic_id = le32(entry, 4)?;
                let flags = le32(entry, 8)?;
                if flags & 1 != 0 {
                    madt.push_cpu(apic_id)?;
                }
            }
            _ => {}
        }
        pos += elen;
    }
    if madt.n_cpus == 0 {
        return Err(BootInfoError::MissingHarts);
    }
    Ok(madt)
}

impl Madt {
    fn push_cpu(&mut self, id: u32) -> Result<(), BootInfoError> {
        if self.cpu_ids[..usize::from(self.n_cpus)].contains(&id) {
            return Ok(());
        }
        if usize::from(self.n_cpus) >= MAX_HARTS {
            return Err(BootInfoError::TooManyHarts);
        }
        self.cpu_ids[usize::from(self.n_cpus)] = id;
        self.n_cpus += 1;
        Ok(())
    }
}

/// `FADT` (FACP) đã parse: revision + `PM1a_CNT_BLK`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fadt {
    /// `Revision` của bảng.
    pub revision: u8,
    /// `PM1a_CNT_BLK` (ưu tiên `X_PM1a_CNT_BLK` 64-bit nếu có).
    pub pm1a_cnt: Option<u64>,
}

/// Parse `FADT` — kiểm chữ ký, độ dài, checksum; đọc `PM1a_CNT_BLK` theo đúng offset ACPI.
pub fn parse_fadt(table: &[u8]) -> Result<Fadt, BootInfoError> {
    if table.len() < ACPI_HEADER_BYTES {
        return Err(BootInfoError::Truncated {
            needed: ACPI_HEADER_BYTES,
            have: table.len(),
        });
    }
    if &table[..4] != b"FACP" {
        return Err(BootInfoError::BadTable("fadt-sig"));
    }
    let len = le32(table, 4)? as usize;
    if len < ACPI_HEADER_BYTES || len > table.len() {
        return Err(BootInfoError::BadTable("fadt-length"));
    }
    let table = &table[..len];
    if !acpi_checksum_ok(table) {
        return Err(BootInfoError::BadChecksum { what: "fadt" });
    }
    let revision = table[8];
    // `PM1a_CNT_BLK` (u32) ở offset 64 — chỉ đọc khi bảng đủ dài.
    let legacy = if len >= 68 { le32(table, 64)? } else { 0 };
    // `X_PM1a_CNT_BLK` (GAS 12 byte) ở offset 172, địa chỉ 64-bit ở +4.
    let extended = if len >= 184 { le64(table, 176)? } else { 0 };
    let pm1a_cnt = if extended != 0 {
        Some(extended)
    } else if legacy != 0 {
        Some(u64::from(legacy))
    } else {
        None
    };
    Ok(Fadt { revision, pm1a_cnt })
}

/// `MCFG` đã parse: ECAM base của phân đoạn PCI đầu tiên.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mcfg {
    /// Địa chỉ base của ECAM.
    pub ecam: u64,
    /// Bus bắt đầu/kết thúc của phân đoạn.
    pub start_bus: u8,
    /// Bus kết thúc.
    pub end_bus: u8,
}

/// Parse `MCFG` — kiểm chữ ký, độ dài, checksum, rồi entry cấu hình 16 byte.
pub fn parse_mcfg(table: &[u8]) -> Result<Mcfg, BootInfoError> {
    if table.len() < 44 {
        return Err(BootInfoError::Truncated {
            needed: 44,
            have: table.len(),
        });
    }
    if &table[..4] != b"MCFG" {
        return Err(BootInfoError::BadTable("mcfg-sig"));
    }
    let len = le32(table, 4)? as usize;
    if len < 60 || len > table.len() {
        return Err(BootInfoError::BadTable("mcfg-length"));
    }
    let table = &table[..len];
    if !acpi_checksum_ok(table) {
        return Err(BootInfoError::BadChecksum { what: "mcfg" });
    }
    let ecam = le64(table, 44)?;
    if ecam == 0 {
        return Err(BootInfoError::MissingNode("pcie-ecam"));
    }
    Ok(Mcfg {
        ecam,
        start_bus: table[52],
        end_bus: table[53],
    })
}

/// Một entry E820/memmap PVH đã parse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct E820Entry {
    /// Địa chỉ bắt đầu.
    pub base: u64,
    /// Kích thước.
    pub size: u64,
    /// Loại (`1` = usable RAM).
    pub kind: u32,
}

/// Kết quả parse E820: các entry usable RAM.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct E820 {
    /// Các entry (đã lọc `type == 1`).
    pub entries: [E820Entry; MAX_USABLE_RAM],
    /// Số entry đã ghi.
    pub n: u8,
    /// Tổng số entry **đọc được** (kể cả loại khác) — bằng chứng không bỏ entry âm thầm.
    pub seen: u16,
}

/// Parse bảng E820 (QEMU `etc/e820`, entry 20 byte) hoặc memmap PVH (entry 24 byte).
///
/// `stride` chỉ nhận [`E820_ENTRY_BYTES`] hoặc [`PVH_MEMMAP_ENTRY_BYTES`]; blob rỗng hoặc độ dài
/// không chia hết ⇒ lỗi có tên. Entry loại `1` (usable RAM) được giữ, entry `size == 0` bỏ qua.
pub fn parse_e820(blob: &[u8], stride: usize) -> Result<E820, BootInfoError> {
    if stride != E820_ENTRY_BYTES && stride != PVH_MEMMAP_ENTRY_BYTES {
        return Err(BootInfoError::BadTable("e820-stride"));
    }
    if blob.is_empty() || !blob.len().is_multiple_of(stride) {
        return Err(BootInfoError::BadTable("e820-length"));
    }
    let mut e820 = E820 {
        entries: [E820Entry {
            base: 0,
            size: 0,
            kind: 0,
        }; MAX_USABLE_RAM],
        n: 0,
        seen: 0,
    };
    for chunk in blob.chunks_exact(stride) {
        e820.seen = e820.seen.saturating_add(1);
        let base = le64(chunk, 0)?;
        let size = le64(chunk, 8)?;
        let kind = le32(chunk, 16)?;
        if kind != E820_TYPE_RAM || size == 0 {
            continue;
        }
        if usize::from(e820.n) >= MAX_USABLE_RAM {
            return Err(BootInfoError::TooManyRegions);
        }
        e820.entries[usize::from(e820.n)] = E820Entry { base, size, kind };
        e820.n += 1;
    }
    if e820.n == 0 {
        return Err(BootInfoError::MissingMemory);
    }
    Ok(e820)
}

/// Parse bảng ACPI + E820 thành [`BootInfo`] cho `x86_64`.
///
/// `tables` là blob nối tiếp (`etc/acpi/tables` của QEMU fw_cfg) chứa `MADT`, `FADT`, `MCFG`.
/// Trả lỗi có tên khi thiếu bảng, checksum sai, độ dài hỏng, không có RAM usable hoặc không có
/// hart nào — **không** rơi về hằng số QEMU.
pub fn parse_acpi(
    tables: &[u8],
    e820_blob: &[u8],
    e820_stride: usize,
) -> Result<BootInfo, BootInfoError> {
    let madt = find_acpi_table(tables, b"APIC", "madt")?;
    let fadt = find_acpi_table(tables, b"FACP", "fadt")?;
    let mcfg = find_acpi_table(tables, b"MCFG", "mcfg")?;
    build_acpi_bootinfo(madt, fadt, mcfg, e820_blob, e820_stride)
}

/// Ghép `BootInfo` x86_64 từ **ba bảng đã tách rời** (`MADT`/`FADT`/`MCFG`) + E820.
///
/// Dùng chung cho host test (blob nối tiếp qua [`parse_acpi`]) và đường đọc `fw_cfg` của crate arch
/// (nơi mỗi bảng được đọc theo **luồng**, không phải blob lớn). Mọi bảng đều parse + kiểm checksum
/// lại ở đây; không có giá trị mặc định nào thay cho bảng hỏng.
pub fn build_acpi_bootinfo(
    madt_table: &[u8],
    fadt_table: &[u8],
    mcfg_table: &[u8],
    e820_blob: &[u8],
    e820_stride: usize,
) -> Result<BootInfo, BootInfoError> {
    let madt = parse_madt(madt_table)?;
    let fadt = parse_fadt(fadt_table)?;
    let mcfg = parse_mcfg(mcfg_table)?;
    let e820 = parse_e820(e820_blob, e820_stride)?;
    let mut info = BootInfo::new(BootArch::X86_64, BootSource::Acpi);
    info.apic = Some(madt.local_apic);
    for &cpu_id in madt.cpu_ids.iter().take(usize::from(madt.n_cpus)) {
        info.push_hart(cpu_id)?;
    }
    info.boot_hart = madt.cpu_ids[0];
    info.pcie_ecam = Some(mcfg.ecam);
    info.pm1a_cnt = fadt.pm1a_cnt;
    if fadt.revision == 0 {
        return Err(BootInfoError::BadTable("fadt-revision"));
    }
    for &e in e820.entries.iter().take(usize::from(e820.n)) {
        info.push_ram(e.base, e.size)?;
    }
    // UART x86: ISA COM1 (`0x3F8`) là chuẩn ISA, không phải hằng QEMU; ACPI không khai UART mặc
    // định. Ghi vào BootInfo để đường boot log được cùng một trường với 2 arch còn lại.
    info.uart = Some(Uart {
        base: X86_COM1_BASE,
        kind: UartKind::Ns16550,
    });
    info.validate()?;
    Ok(info)
}

/// COM1 chuẩn ISA (I/O port) — hằng **kiến trúc PC**, không phải địa chỉ QEMU `virt`.
pub const X86_COM1_BASE: u64 = 0x3F8;

/// Kiểm **một** bảng ACPI độc lập (dùng cho `MADT`/`FADT`/`MCFG` rời) — checksum + độ dài.
pub fn check_acpi_table(
    table: &[u8],
    sig: &[u8; 4],
    name: &'static str,
) -> Result<(), BootInfoError> {
    if table.len() < ACPI_HEADER_BYTES {
        return Err(BootInfoError::Truncated {
            needed: ACPI_HEADER_BYTES,
            have: table.len(),
        });
    }
    if &table[..4] != sig {
        return Err(BootInfoError::BadTable(name));
    }
    let len = le32(table, 4)? as usize;
    if len < ACPI_HEADER_BYTES || len > table.len() {
        return Err(BootInfoError::BadTable(name));
    }
    if !acpi_checksum_ok(&table[..len]) {
        return Err(BootInfoError::BadChecksum { what: name });
    }
    Ok(())
}
