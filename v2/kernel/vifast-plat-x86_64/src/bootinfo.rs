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
//! # Hai nguồn bảng ACPI, theo thứ tự
//!
//! 1. **`RSDP` trong RAM** (đường thường): QEMU `q35` chạy SeaBIOS; SeaBIOS chép bảng từ `fw_cfg`
//!    vào RAM rồi cài `RSDP` trong vùng BIOS `0xE_0000..0x10_0000`. Ta đi
//!    `RSDP → RSDT/XSDT → MADT/FADT/MCFG` — đây là đường "firmware **đã** cài bảng".
//! 2. **`fw_cfg` trực tiếp** (**fallback, KT-402b** — `D2-260` mục 5): khi RAM **không** có `RSDP`
//!    (firmware không chạy hoặc không cài bảng) thì đọc thẳng blob `etc/acpi/tables` mà QEMU dựng.
//!    Blob là bảng **trước khi firmware vá** (checksum chưa tính, con trỏ xuyên bảng còn là offset
//!    tương đối), nên fallback làm **đúng bước firmware**: đọc `etc/table-loader` để kiểm blob đúng
//!    là bộ bảng firmware được lệnh cài, rồi **tính lại** checksum ACPI cho từng bảng. Ba bảng ta
//!    dùng — `MADT`/`FADT`/`MCFG` — **tự chứa** nên không cần áp các lệnh vá con trỏ; con trỏ duy
//!    nhất trong `FADT` (`X_DSDT`) **không** được theo.
//!
//! Không nhánh nào rơi về hằng số QEMU: thiếu/hỏng bảng ⇒ lỗi **có tên** ⇒ caller halt
//! (fail-closed, `D2-259`).

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
/// Cỡ một entry `struct bios_linker_entry` của `etc/table-loader` (đo trên QEMU 11.1.3: entry
/// `ALLOCATE` của `etc/acpi/rsdp` ở offset 0, của `etc/acpi/tables` ở offset 128 ⇒ 64 B/entry).
const LINKER_ENTRY_BYTES: usize = 64;
/// `BIOS_LINKER_LOADER_COMMAND_ALLOCATE` — firmware chép file `name` vào vùng đệm.
const LINKER_CMD_ALLOCATE: u32 = 0x1;
/// Độ dài trường tên file trong entry `ALLOCATE` (`char file[56]`).
const LINKER_NAME_BYTES: usize = 56;
/// Trần buffer `etc/table-loader` (QEMU cấp 4 KiB; phần sau entry cuối là `0`).
const LINKER_BUF_BYTES: usize = 4096;
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
    //
    // **KT-402b** (`D2-260` mục 5) — **không** có `RSDP` trong RAM (firmware không chạy/không cài
    // bảng) ⇒ **fallback `fw_cfg`**: đọc thẳng blob `etc/acpi/tables` QEMU dựng. Chỉ fallback khi
    // *thiếu* RSDP; RSDP **có** mà bảng hỏng thì vẫn fail-closed (không che lỗi hỏng bảng).
    match find_rsdp(start_info) {
        Ok(rsdp_pa) => {
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
        Err(BootInfoError::MissingTable("rsdp")) => acpi_from_fw_cfg(&e820[..e820_len], stride),
        Err(e) => Err(e),
    }
}

/// **KT-402b** (`D2-260` mục 5) — nhánh **fallback `fw_cfg`**: đọc bảng ACPI từ blob nối tiếp
/// `etc/acpi/tables` khi RAM **không** có `RSDP`.
///
/// Blob QEMU cấp là **128 KiB** (vùng đệm cố định, phần sau bảng cuối là `0`) nên **không** cấp
/// buffer bằng cả blob: đọc **theo luồng** (header 36 B → `length` → thân bảng), bảng không cần
/// thì bỏ qua đúng `length` của nó. Ba bảng cần (`MADT`/`FADT`/`MCFG`) đều < 1 KiB.
///
/// # Vì sao vẫn đọc `etc/table-loader`
///
/// Blob là bảng **trước khi firmware vá**: QEMU đặt `checksum = 0` cho **mọi** bảng rồi ghi lệnh
/// `ADD_CHECKSUM` vào `etc/table-loader` để firmware tính sau khi vá con trỏ (đo trên QEMU 11.1.3:
/// cả `FACP`/`APIC`/`MCFG` trong blob đều có checksum **sai**). Vì vậy ta **tính lại** checksum
/// đúng công thức ACPI cho từng bảng đọc được — đúng bước firmware làm — rồi để parser chung
/// (`parse_madt`/`parse_fadt`/`parse_mcfg`) kiểm chữ ký/độ dài/trường. Loader được đọc để **kiểm
/// blob đúng là bộ bảng firmware được lệnh cài** (`ALLOCATE` cho `etc/acpi/tables`): thiếu ⇒
/// fail-closed, không đoán.
///
/// Fail-closed: thiếu blob/loader, `length` hỏng/vượt blob, thiếu bảng hay parser chung từ chối ⇒
/// lỗi **có tên**.
fn acpi_from_fw_cfg(e820: &[u8], stride: usize) -> Result<BootInfo, BootInfoError> {
    let (selector, size) = fw_find_file("etc/acpi/tables", "acpi-tables")?;
    let total = size as usize;
    let (loader_sel, loader_size) = fw_find_file("etc/table-loader", "table-loader")?;
    let mut loader = [0u8; LINKER_BUF_BYTES];
    let loader_len = fw_read_file(loader_sel, loader_size, &mut loader)?;
    if !linker_allocates(&loader[..loader_len], "etc/acpi/tables")? {
        return Err(BootInfoError::MissingTable("table-loader"));
    }
    let mut madt = [0u8; ACPI_TABLE_BUF_BYTES];
    let mut fadt = [0u8; ACPI_TABLE_BUF_BYTES];
    let mut mcfg = [0u8; ACPI_TABLE_BUF_BYTES];
    let (mut madt_len, mut fadt_len, mut mcfg_len) = (0usize, 0usize, 0usize);
    fw_select(selector);
    let mut pos = 0usize;
    while pos + ACPI_HEADER_BYTES <= total {
        let mut hdr = [0u8; ACPI_HEADER_BYTES];
        for b in &mut hdr {
            *b = fw_byte();
        }
        // Hết vùng dùng: QEMU cấp blob 128 KiB nhưng phần sau bảng cuối là `0`.
        if hdr[..4].iter().all(|&b| b == 0) {
            break;
        }
        let len = le32(&hdr, 4)? as usize;
        if len < ACPI_HEADER_BYTES || len > total - pos {
            return Err(BootInfoError::BadTable("acpi-tables"));
        }
        let target = match &hdr[..4] {
            b"APIC" => Some((&mut madt, &mut madt_len)),
            b"FACP" => Some((&mut fadt, &mut fadt_len)),
            b"MCFG" => Some((&mut mcfg, &mut mcfg_len)),
            _ => None,
        };
        match target {
            Some((buf, out)) => {
                if len > buf.len() {
                    return Err(BootInfoError::BadTable("acpi-tables"));
                }
                buf[..ACPI_HEADER_BYTES].copy_from_slice(&hdr);
                for b in &mut buf[ACPI_HEADER_BYTES..len] {
                    *b = fw_byte();
                }
                // Blob là bảng **trước khi firmware vá** ⇒ checksum do firmware tính (lệnh
                // `ADD_CHECKSUM`): làm đúng bước ấy, không bỏ qua phép kiểm nào của parser chung.
                buf[9] = 0;
                let sum = buf[..len].iter().fold(0u8, |a, &b| a.wrapping_add(b));
                buf[9] = 0u8.wrapping_sub(sum);
                *out = len;
            }
            None => {
                for _ in ACPI_HEADER_BYTES..len {
                    let _ = fw_byte();
                }
            }
        }
        pos += len;
    }
    if madt_len == 0 {
        return Err(BootInfoError::MissingTable("madt"));
    }
    if fadt_len == 0 {
        return Err(BootInfoError::MissingTable("fadt"));
    }
    if mcfg_len == 0 {
        return Err(BootInfoError::MissingTable("mcfg"));
    }
    build_acpi_bootinfo(
        &madt[..madt_len],
        &fadt[..fadt_len],
        &mcfg[..mcfg_len],
        e820,
        stride,
    )
}

/// `true` ⇔ `etc/table-loader` có lệnh `ALLOCATE` cho file `name` — tức blob ta đọc đúng là bộ
/// bảng firmware được lệnh chép vào RAM. Entry `ALLOCATE` = `command` (u32) + `char file[56]`.
fn linker_allocates(loader: &[u8], name: &str) -> Result<bool, BootInfoError> {
    let mut i = 0usize;
    while i + LINKER_ENTRY_BYTES <= loader.len() {
        let entry = &loader[i..i + LINKER_ENTRY_BYTES];
        let cmd = le32(entry, 0)?;
        // Phần đệm sau entry cuối là `0` (QEMU cấp loader 4 KiB).
        if cmd == 0 {
            break;
        }
        if cmd == LINKER_CMD_ALLOCATE {
            let raw = &entry[4..4 + LINKER_NAME_BYTES];
            let n = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
            if &raw[..n] == name.as_bytes() {
                return Ok(true);
            }
        }
        i += LINKER_ENTRY_BYTES;
    }
    Ok(false)
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
    if !(ACPI_HEADER_BYTES..=SDT_BUF_BYTES).contains(&len) {
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
