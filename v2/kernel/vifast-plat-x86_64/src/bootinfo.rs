// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-402** (D2-259) — đọc bảng **thật** của QEMU `q35` cho x86_64 và ghép [`BootInfo`]:
//!
//! - **PVH `hvm_start_info`** (QEMU nạp `-kernel` ELF64 qua note Xen, `%ebx` = địa chỉ): magic,
//!   version, `memmap_paddr`/`memmap_entries` (E820 dạng PVH — `type 1` = RAM usable);
//! - **fw_cfg** (I/O port `0x510`/`0x511`, chuẩn QEMU): `etc/e820` (khi PVH không khai memmap) và
//!   `etc/acpi/tables` — blob nối tiếp chứa `MADT`/`FADT`/`MCFG`;
//! - **UART**: COM1 ISA `0x3F8` (hằng **kiến trúc PC**, không phải địa chỉ QEMU `virt`).
//!
//! Phần **thuần** (parse MADT/FADT/MCFG/E820, lỗi có tên) nằm ở
//! [`vifast_plat_common::bootinfo`] và đã host-test; tệp này chỉ là lớp **đọc MMIO/port I/O**.
//! Không có hằng địa chỉ PLIC/GIC/APIC/ECAM nào viết tay ở đây: LAPIC lấy từ `MADT`, ECAM lấy từ
//! `MCFG`, RAM lấy từ memmap/E820.
//!
//! # Vì sao `fw_cfg`, không quét bộ nhớ
//!
//! QEMU **không** tự đặt bảng ACPI vào RAM khách: nó dựng bảng trong `fw_cfg`
//! (`etc/acpi/tables`, `etc/table-loader`) để firmware đời đầu chép vào RAM. Đường boot PVH của
//! ta **không chạy firmware**, nên đọc thẳng blob `fw_cfg` là nguồn thật duy nhất — không có
//! `RSDP` nào trong RAM để quét (quét sẽ ra rác và che mất lỗi).

#![allow(unsafe_code)] // D2-17: crate `kernel/vifast-plat-*`; chỉ phần đọc PA cần `unsafe`.

use vifast_plat_common::bootinfo::{
    acpi_checksum_ok, build_acpi_bootinfo, BootInfo, BootInfoError, E820_ENTRY_BYTES,
    PVH_MEMMAP_ENTRY_BYTES,
};

/// Cổng chọn mục fw_cfg (x86 I/O port, 16-bit).
const FW_CFG_CTL: u16 = 0x510;
/// Cổng dữ liệu fw_cfg (8-bit, luôn `CTL + 1`).
const FW_CFG_DATA: u16 = 0x511;
/// Mục chữ ký (`"QEMU"`).
const FW_CFG_SIGNATURE: u16 = 0x0000;
/// Mục thư mục file.
const FW_CFG_FILE_DIR: u16 = 0x0019;
/// Số entry thư mục tối đa chấp nhận (chặn vòng lặp rác).
const FW_CFG_MAX_ENTRIES: u32 = 512;
/// Độ dài tên file tối đa trong entry thư mục (`struct FWCfgFile.name[56]`).
const FW_CFG_NAME_BYTES: usize = 56;
/// Trần buffer E820 (QEMU `q35` ~7–12 entry 20 B; PVH 24 B/entry).
const E820_BUF_BYTES: usize = 1024;
/// Trần buffer một bảng ACPI cần đọc (`MADT`/`FADT`/`MCFG` đều < 1 KiB; `FADT` = 276 B).
const ACPI_TABLE_BUF_BYTES: usize = 1024;
/// Cỡ header chung bảng ACPI (`struct acpi_table_header`).
const ACPI_HEADER_BYTES: usize = 36;
/// Trần buffer RSDT/XSDT (bảng gốc chỉ chứa con trỏ, QEMU ~100 B; trần rộng cho an toàn).
const SDT_BUF_BYTES: usize = 4096;
/// Vùng BIOS chứa `RSDP` (`0xE_0000..0x10_0000`) — nơi firmware đặt RSDP khi không có
/// `hvm_start_info.rsdp_paddr`.
const RSDP_SCAN_START: u64 = 0x000E_0000;
/// Điểm kết thúc vùng quét RSDP.
const RSDP_SCAN_END: u64 = 0x0010_0000;

/// `true` ⇔ `hvm_start_info` dùng được (magic PVH).
pub const PVH_START_MAGIC: u32 = 0x336e_c578;

/// Đọc cấu hình thật từ PVH `hvm_start_info` (`%ebx` lúc `_start`) + fw_cfg ⇒ [`BootInfo`].
pub fn detect(boot_ebx: u32) -> Result<BootInfo, BootInfoError> {
    let start_info = u64::from(boot_ebx);
    if start_info == 0 {
        return Err(BootInfoError::NullPointer);
    }
    let magic = read_pa_u32(start_info)?;
    if magic != PVH_START_MAGIC {
        return Err(BootInfoError::BadMagic {
            what: "pvh",
            got: magic,
        });
    }
    let version = read_pa_u32(start_info + 4)?;
    if version != 1 {
        return Err(BootInfoError::UnsupportedVersion {
            what: "pvh",
            got: version,
        });
    }
    let memmap_pa = read_pa_u64(start_info + 40)?;
    let memmap_entries = read_pa_u32(start_info + 48)?;

    let mut e820 = [0u8; E820_BUF_BYTES];
    let (e820_len, stride) = if memmap_entries > 0 && memmap_pa != 0 {
        let bytes = memmap_entries as usize * PVH_MEMMAP_ENTRY_BYTES;
        if bytes > e820.len() {
            return Err(BootInfoError::Truncated {
                needed: bytes,
                have: e820.len(),
            });
        }
        read_pa_bytes(memmap_pa, &mut e820[..bytes])?;
        (bytes, PVH_MEMMAP_ENTRY_BYTES)
    } else {
        let (selector, size) = fw_find_file("etc/e820", "e820")?;
        let len = fw_read_file(selector, size, &mut e820)?;
        (len, E820_ENTRY_BYTES)
    };

    // Bảng ACPI **thật** do firmware cài vào RAM: RSDP từ PVH (`rsdp_paddr`, nếu QEMU điền) hoặc
    // quét vùng BIOS `0xE_0000..0x10_0000`; rồi đi RSDT/XSDT tới `MADT`/`FADT`/`MCFG`.
    let rsdp_pa = find_rsdp(start_info)?;
    let mut madt = [0u8; ACPI_TABLE_BUF_BYTES];
    let mut fadt = [0u8; ACPI_TABLE_BUF_BYTES];
    let mut mcfg = [0u8; ACPI_TABLE_BUF_BYTES];
    let madt_len = acpi_find_table(rsdp_pa, b"APIC", "madt", &mut madt)?;
    let fadt_len = acpi_find_table(rsdp_pa, b"FACP", "fadt", &mut fadt)?;
    let mcfg_len = acpi_find_table(rsdp_pa, b"MCFG", "mcfg", &mut mcfg)?;
    build_acpi_bootinfo(
        &madt[..madt_len],
        &fadt[..fadt_len],
        &mcfg[..mcfg_len],
        &e820[..e820_len],
        stride,
    )
}

// ---------------------------------------------------------------------------------------------
// fw_cfg (I/O port) — đọc tuần tự, không đọc ngoài `size`
// ---------------------------------------------------------------------------------------------

fn fw_select(selector: u16) {
    crate::asm::outw(FW_CFG_CTL, selector);
}

fn fw_byte() -> u8 {
    crate::asm::inb(FW_CFG_DATA)
}

fn fw_be16() -> u16 {
    u16::from_be_bytes([fw_byte(), fw_byte()])
}

fn fw_be32() -> u32 {
    u32::from_be_bytes([fw_byte(), fw_byte(), fw_byte(), fw_byte()])
}

/// Tìm file trong thư mục fw_cfg: trả `(selector, size)`. `code` là tên chữ cho lỗi có tên.
fn fw_find_file(name: &str, code: &'static str) -> Result<(u16, u32), BootInfoError> {
    fw_select(FW_CFG_SIGNATURE);
    let sig = [fw_byte(), fw_byte(), fw_byte(), fw_byte()];
    if &sig != b"QEMU" {
        return Err(BootInfoError::BadMagic {
            what: "fw-cfg",
            got: u32::from_be_bytes(sig),
        });
    }
    fw_select(FW_CFG_FILE_DIR);
    let count = fw_be32();
    if count > FW_CFG_MAX_ENTRIES {
        return Err(BootInfoError::BadTable("fw-cfg-dir"));
    }
    for _ in 0..count {
        let size = fw_be32();
        let selector = fw_be16();
        let _reserved = fw_be16();
        let mut name_buf = [0u8; FW_CFG_NAME_BYTES];
        for b in &mut name_buf {
            *b = fw_byte();
        }
        let nlen = name_buf
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(FW_CFG_NAME_BYTES);
        if &name_buf[..nlen] == name.as_bytes() {
            return Ok((selector, size));
        }
    }
    Err(BootInfoError::MissingTable(code))
}

/// Đọc tối đa `out.len()` byte của một file fw_cfg; `size` lớn hơn buffer ⇒ `Truncated`.
fn fw_read_file(selector: u16, size: u32, out: &mut [u8]) -> Result<usize, BootInfoError> {
    let n = size as usize;
    if n > out.len() {
        return Err(BootInfoError::Truncated {
            needed: n,
            have: out.len(),
        });
    }
    fw_select(selector);
    for b in &mut out[..n] {
        *b = fw_byte();
    }
    Ok(n)
}

// ---------------------------------------------------------------------------------------------
// ACPI: RSDP → RSDT/XSDT → MADT/FADT/MCFG (bảng firmware đã cài trong RAM)
// ---------------------------------------------------------------------------------------------

/// Tìm `RSDP` thật: ưu tiên `hvm_start_info.rsdp_paddr` (offset 32), nếu 0 thì quét vùng BIOS
/// `0xE_0000..0x10_0000` theo bước 16 (ACPI yêu cầu RSDP căn 16) và kiểm checksum 20 byte đầu.
fn find_rsdp(start_info: u64) -> Result<u64, BootInfoError> {
    let hinted = read_pa_u64(start_info + 32)?;
    if hinted != 0 {
        let mut hdr = [0u8; ACPI_HEADER_BYTES];
        if read_pa_bytes(hinted, &mut hdr).is_ok()
            && &hdr[..8] == b"RSD PTR "
            && acpi_checksum_ok(&hdr[..20])
        {
            return Ok(hinted);
        }
    }
    let mut pa = RSDP_SCAN_START;
    while pa + ACPI_HEADER_BYTES as u64 <= RSDP_SCAN_END {
        let mut hdr = [0u8; ACPI_HEADER_BYTES];
        read_pa_bytes(pa, &mut hdr)?;
        if &hdr[..8] == b"RSD PTR " && acpi_checksum_ok(&hdr[..20]) {
            return Ok(pa);
        }
        pa += 16;
    }
    Err(BootInfoError::MissingTable("rsdp"))
}

/// Đi `RSDP → RSDT/XSDT → bảng có chữ ký `sig``, chỉ đọc đúng bảng khớp vào `out`.
fn acpi_find_table(
    rsdp_pa: u64,
    sig: &[u8; 4],
    code: &'static str,
    out: &mut [u8],
) -> Result<usize, BootInfoError> {
    let mut rsdp = [0u8; ACPI_HEADER_BYTES];
    read_pa_bytes(rsdp_pa, &mut rsdp)?;
    if &rsdp[..8] != b"RSD PTR " {
        return Err(BootInfoError::BadTable("rsdp-sig"));
    }
    if !acpi_checksum_ok(&rsdp[..20]) {
        return Err(BootInfoError::BadChecksum { what: "rsdp" });
    }
    let revision = rsdp[15];
    let (sdt_pa, entry_bytes) = if revision >= 2 {
        (le64(&rsdp, 24)?, 8usize)
    } else {
        (u64::from(le32(&rsdp, 16)?), 4usize)
    };
    if sdt_pa == 0 || sdt_pa + ACPI_HEADER_BYTES as u64 > crate::boot::IDENTITY_MAP_BYTES {
        return Err(BootInfoError::BadOffset { what: "rsdt" });
    }
    let mut header = [0u8; ACPI_HEADER_BYTES];
    read_pa_bytes(sdt_pa, &mut header)?;
    let len = le32(&header, 4)? as usize;
    if len < ACPI_HEADER_BYTES || len > SDT_BUF_BYTES {
        return Err(BootInfoError::BadTable(code));
    }
    let mut sdt = [0u8; SDT_BUF_BYTES];
    read_pa_bytes(sdt_pa, &mut sdt[..len])?;
    if !acpi_checksum_ok(&sdt[..len]) {
        return Err(BootInfoError::BadChecksum { what: "sdt" });
    }
    let entries = (len - ACPI_HEADER_BYTES) / entry_bytes;
    for i in 0..entries {
        let off = ACPI_HEADER_BYTES + i * entry_bytes;
        let pa = if entry_bytes == 8 {
            le64(&sdt, off)?
        } else {
            u64::from(le32(&sdt, off)?)
        };
        if pa == 0 || pa + ACPI_HEADER_BYTES as u64 > crate::boot::IDENTITY_MAP_BYTES {
            continue;
        }
        let mut th = [0u8; ACPI_HEADER_BYTES];
        read_pa_bytes(pa, &mut th)?;
        if &th[..4] != sig {
            continue;
        }
        let tlen = le32(&th, 4)? as usize;
        if tlen < ACPI_HEADER_BYTES || tlen > out.len() {
            return Err(BootInfoError::BadTable(code));
        }
        read_pa_bytes(pa, &mut out[..tlen])?;
        if !acpi_checksum_ok(&out[..tlen]) {
            return Err(BootInfoError::BadChecksum { what: code });
        }
        return Ok(tlen);
    }
    Err(BootInfoError::MissingTable(code))
}

fn le32(bytes: &[u8], off: usize) -> Result<u32, BootInfoError> {
    let b = bytes.get(off..off + 4).ok_or(BootInfoError::OutOfBounds)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn le64(bytes: &[u8], off: usize) -> Result<u64, BootInfoError> {
    let b = bytes.get(off..off + 8).ok_or(BootInfoError::OutOfBounds)?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}

// ---------------------------------------------------------------------------------------------
// Đọc vật lý vùng thấp (PVH start_info + memmap) — identity map 1 GiB của ảnh
// ---------------------------------------------------------------------------------------------

fn read_pa_u32(pa: u64) -> Result<u32, BootInfoError> {
    let mut buf = [0u8; 4];
    read_pa_bytes(pa, &mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

fn read_pa_u64(pa: u64) -> Result<u64, BootInfoError> {
    let mut buf = [0u8; 8];
    read_pa_bytes(pa, &mut buf)?;
    Ok(u64::from_le_bytes(buf))
}

fn read_pa_bytes(pa: u64, out: &mut [u8]) -> Result<(), BootInfoError> {
    let len = out.len() as u64;
    let end = pa
        .checked_add(len)
        .ok_or(BootInfoError::BadOffset { what: "pvh-pa" })?;
    if pa == 0 || end > crate::boot::IDENTITY_MAP_BYTES {
        return Err(BootInfoError::BadOffset { what: "pvh-pa" });
    }
    // SAFETY: `pa` nằm trong identity map 1 GiB đã bật ở `_start` (P/RW), đọc đúng `len` byte
    // (`len` lấy từ cỡ trường cố định hoặc số entry đã kiểm trần). Không ghi.
    unsafe {
        core::ptr::copy_nonoverlapping(pa as usize as *const u8, out.as_mut_ptr(), out.len());
    }
    Ok(())
}

/// In `# bootinfo:` từ kết quả [`detect`] (hoặc dòng `fail-closed` **có tên** khi bảng hỏng).
///
/// Trả `true` ⇔ `BootInfo` hợp lệ. Trả `false` ⇒ **caller phải halt** (fail-closed, D2-259):
/// không có giá trị mặc định nào được dùng thay bảng hỏng.
pub fn write_log<W: core::fmt::Write>(info: &Result<BootInfo, BootInfoError>, w: &mut W) -> bool {
    match info {
        Ok(info) => info.write_log(w).is_ok(),
        Err(e) => {
            let _ =
                core::fmt::Write::write_fmt(w, format_args!("# bootinfo: fail-closed err={e}\n"));
            false
        }
    }
}
