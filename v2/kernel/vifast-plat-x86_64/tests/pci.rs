// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho `vifast-plat-x86_64::pci` — KT-274 (K5' bước 5: PCI + virtio-net).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-x86_64 --test pci
//! ```
//!
//! Bốn lớp, mỗi lớp bắt một lớp lỗi khác nhau:
//!
//! 1. **Legacy `0xCF8`/`0xCFC`** — mã hoá địa chỉ theo PCI Local Bus 3.0 §3.2.2.3.1: bit 31 bật,
//!    bus 16..23, device 11..15, function 8..10, offset 2..7 (bit 0..1 **bị che** vì thanh ghi
//!    cấu hình là 32 bit). Giá trị kỳ vọng tính tay, không so với hằng của crate.
//! 2. **ECAM** — `bus<<20 | dev<<15 | func<<12 | off` (PCIe §7.2.2, một function 4 KiB).
//! 3. **ACPI RSDP/RSDT/XSDT/MCFG** — tìm bảng theo chữ ký + **kiểm checksum** (một bảng hỏng
//!    không được coi là hợp lệ), đọc `base_address`/`start_bus`/`end_bus` của MCFG.
//! 4. **Ca âm** — chữ ký sai, checksum sai, bảng cụt, không có RSDP ⇒ `None` (fail-closed, không
//!    đoán địa chỉ ECAM).

use vifast_plat_x86_64::pci::{
    checksum_ok, ecam_addr, find_mcfg, find_rsdp, legacy_cf8, parse_mcfg, parse_rsdp, table_length,
    table_signature, AccessKind, McfgEntry, PhysMem, Rsdp, ACPI_HEADER_BYTES, CF8_ENABLE,
    EBDA_SCAN_END, EBDA_SCAN_START, MCFG_ENTRY_BYTES, MCFG_ENTRY_OFFSET, MCFG_RESERVED_BYTES,
    MCFG_SIGNATURE, RSDP_SIGNATURE, RSDP_V1_BYTES, RSDP_V2_BYTES,
};

// ---------------------------------------------------------------------------------------------
// Bộ nhớ giả: một vùng liên tục ở `base` — đủ để đặt RSDP + RSDT + MCFG.
// ---------------------------------------------------------------------------------------------

/// Bộ nhớ vật lý giả cho test host: `bytes[i]` là byte tại `base + i`.
struct FakeMem {
    base: u64,
    bytes: Vec<u8>,
}

impl FakeMem {
    fn new(base: u64, len: usize) -> Self {
        Self {
            base,
            bytes: vec![0u8; len],
        }
    }

    fn put(&mut self, addr: u64, data: &[u8]) {
        let off = (addr - self.base) as usize;
        self.bytes[off..off + data.len()].copy_from_slice(data);
    }
}

impl PhysMem for FakeMem {
    fn read(&self, addr: u64, out: &mut [u8]) -> bool {
        if addr < self.base {
            return false;
        }
        let off = (addr - self.base) as usize;
        if off + out.len() > self.bytes.len() {
            return false;
        }
        out.copy_from_slice(&self.bytes[off..off + out.len()]);
        true
    }
}

/// Đặt checksum vào byte `sum_index` sao cho tổng `bytes[..len]` == 0 (mod 256).
fn fix_checksum(bytes: &mut [u8], len: usize, sum_index: usize) {
    bytes[sum_index] = 0;
    let sum: u8 = bytes[..len].iter().fold(0u8, |a, b| a.wrapping_add(*b));
    bytes[sum_index] = 0u8.wrapping_sub(sum);
}

/// Dựng một bảng ACPI 36 byte header + `body`, checksum đúng.
fn acpi_table(sig: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let len = ACPI_HEADER_BYTES + body.len();
    let mut t = vec![0u8; len];
    t[0..4].copy_from_slice(sig);
    t[4..8].copy_from_slice(&(len as u32).to_le_bytes());
    t[8] = 2; // revision
    t[36..].copy_from_slice(body);
    fix_checksum(&mut t, len, 9);
    t
}

/// Dựng RSDP bản 2.0 (36 byte) trỏ tới `xsdt`.
fn rsdp_v2(xsdt: u64) -> Vec<u8> {
    let mut r = vec![0u8; RSDP_V2_BYTES];
    r[0..8].copy_from_slice(&RSDP_SIGNATURE);
    r[15] = 2; // revision 2.0
    r[20..24].copy_from_slice(&(RSDP_V2_BYTES as u32).to_le_bytes());
    r[24..32].copy_from_slice(&xsdt.to_le_bytes());
    fix_checksum(&mut r, RSDP_V1_BYTES, 8);
    // Checksum mở rộng phủ **toàn** bảng (36 byte) — byte 8 đã dùng cho checksum 20 byte nên
    // phải tính lại cả hai cùng lúc: sửa byte 8 rồi tính lại checksum mở rộng ở một byte khác.
    let sum: u8 = r.iter().fold(0u8, |a, b| a.wrapping_add(*b));
    // `sum` hiện bằng 0 trên 20 byte đầu; phần đuôi 16 byte cộng thêm `sum` ⇒ bù ở byte 32.
    r[32] = 0u8.wrapping_sub(sum);
    r
}

// ---------------------------------------------------------------------------------------------
// 1. Legacy 0xCF8/0xCFC
// ---------------------------------------------------------------------------------------------

#[test]
fn legacy_cf8_matches_pci_spec() {
    // bus 0, dev 2, func 0, off 0x00 ⇒ 0x8000_1000 (bit 31 + dev<<11).
    assert_eq!(legacy_cf8(0, 2, 0, 0x00), 0x8000_1000);
    // bus 0, dev 0, func 0, off 0x0c ⇒ 0x8000_000c (class/revision).
    assert_eq!(legacy_cf8(0, 0, 0, 0x0c), 0x8000_000c);
    // bus 0xff, dev 0x1f, func 7, off 0xfc ⇒ 0x80ff_fffc.
    assert_eq!(legacy_cf8(0xff, 0x1f, 7, 0xfc), 0x80ff_fffc);
    // Bit 0..1 của offset **bị che**: thanh ghi cấu hình là 32 bit, đọc theo word.
    assert_eq!(legacy_cf8(0, 2, 0, 0x13), legacy_cf8(0, 2, 0, 0x10));
    // Device/function ngoài dải bị che, **không** tràn sang trường khác.
    assert_eq!(legacy_cf8(0, 0x22, 0, 0), legacy_cf8(0, 2, 0, 0));
    assert_eq!(legacy_cf8(0, 0, 0x0a, 0), legacy_cf8(0, 0, 2, 0));
    // Bit 31 luôn bật (enable) — không có nó thì QEMU trả 0xffff_ffff cho mọi đọc.
    assert_eq!(legacy_cf8(0, 0, 0, 0) & CF8_ENABLE, CF8_ENABLE);
}

// ---------------------------------------------------------------------------------------------
// 2. ECAM
// ---------------------------------------------------------------------------------------------

#[test]
fn ecam_addr_layout() {
    const BASE: u64 = 0xb000_0000;
    // Function 0/0/0, thanh ghi 0 ⇒ đúng base.
    assert_eq!(ecam_addr(BASE, 0, 0, 0, 0), BASE);
    // Device 2 ⇒ +2<<15 = 0x1_0000.
    assert_eq!(ecam_addr(BASE, 0, 2, 0, 0), BASE + 0x1_0000);
    assert_eq!(ecam_addr(BASE, 0, 2, 0, 0x0c), BASE + 0x1_0000 + 0x0c);
    // Bus 1 ⇒ +1<<20 = 0x10_0000.
    assert_eq!(ecam_addr(BASE, 1, 0, 0, 0), BASE + 0x10_0000);
    // Function 7 ⇒ +7<<12 = 0x7000; offset 0xff là byte cuối của **cấu hình** 256 byte.
    assert_eq!(ecam_addr(BASE, 0, 0, 7, 0xff), BASE + 0x7000 + 0xff);
    // Device/function ngoài dải bị che (5 bit / 3 bit).
    assert_eq!(ecam_addr(0, 0, 0x22, 0, 0), ecam_addr(0, 0, 2, 0, 0));
    assert_eq!(ecam_addr(0, 0, 0, 0x0a, 0), ecam_addr(0, 0, 0, 2, 0));
    // Hai function liền nhau cách đúng 4 KiB (một function = 4 KiB cấu hình).
    assert_eq!(
        ecam_addr(BASE, 0, 2, 1, 0) - ecam_addr(BASE, 0, 2, 0, 0),
        0x1000
    );
}

// ---------------------------------------------------------------------------------------------
// 3. ACPI: RSDP → XSDT → MCFG
// ---------------------------------------------------------------------------------------------

#[test]
fn rsdp_v2_parses_and_checksums() {
    let rsdp = rsdp_v2(0x1234_5000);
    let parsed: Rsdp = parse_rsdp(&rsdp).expect("RSDP 2.0 hợp lệ phải parse được");
    assert_eq!(parsed.revision, 2);
    assert_eq!(parsed.xsdt, 0x1234_5000);
    assert_eq!(parsed.length, RSDP_V2_BYTES as u32);
    // Checksum 20 byte đầu và checksum mở rộng (36 byte) đều phải đúng.
    assert!(checksum_ok(&rsdp[..RSDP_V1_BYTES]));
    assert!(checksum_ok(&rsdp[..RSDP_V2_BYTES]));
    // Hỏng một byte ⇒ **không** parse (fail-closed, không đoán địa chỉ bảng).
    let mut broken = rsdp.clone();
    broken[24] ^= 0xff;
    assert!(parse_rsdp(&broken).is_none());
}

#[test]
fn mcfg_entry_decodes_base_and_buses() {
    // MCFG: header 36 byte + **8 byte reserved** + một allocation structure 16 byte (ACPI 6.4
    // §5.2.6.2 — `base_address` ở offset **44**, không phải 36).
    let mut body = vec![0u8; MCFG_RESERVED_BYTES + MCFG_ENTRY_BYTES];
    body[MCFG_RESERVED_BYTES..MCFG_RESERVED_BYTES + 8]
        .copy_from_slice(&0xb000_0000u64.to_le_bytes());
    body[MCFG_RESERVED_BYTES + 8..MCFG_RESERVED_BYTES + 10].copy_from_slice(&0u16.to_le_bytes());
    body[MCFG_RESERVED_BYTES + 10] = 0; // bus đầu
    body[MCFG_RESERVED_BYTES + 11] = 0xff; // bus cuối
    let table = acpi_table(&MCFG_SIGNATURE, &body);
    let entry: McfgEntry = parse_mcfg(&table).expect("MCFG hợp lệ phải parse được");
    assert_eq!(entry.base, 0xb000_0000);
    assert_eq!(entry.segment, 0);
    assert_eq!(entry.bus_start, 0);
    assert_eq!(entry.bus_end, 0xff);
    // Chữ ký + độ dài đọc đúng từ header.
    assert_eq!(table_signature(&table), Some(MCFG_SIGNATURE));
    assert_eq!(table_length(&table), Some(table.len() as u32));
    // Bảng cụt (không có allocation structure) ⇒ `None`, không đọc tràn.
    assert!(parse_mcfg(&table[..ACPI_HEADER_BYTES]).is_none());
    assert!(parse_mcfg(&table[..MCFG_ENTRY_OFFSET]).is_none());
    // Độ dài thật của bảng MCFG tối thiểu = 44 + 16 = 60 (`0x3C` — đúng bảng SeaBIOS đo được).
    assert_eq!(MCFG_ENTRY_OFFSET + MCFG_ENTRY_BYTES, 60);
    // Chữ ký sai ⇒ `None`.
    let mut wrong = table.clone();
    wrong[0] = b'X';
    assert!(parse_mcfg(&wrong).is_none());
}

#[test]
fn find_mcfg_walks_rsdp_xsdt_and_tables() {
    // Bố cục: RSDP @ 0xE0000, XSDT @ 0xE1000, một bảng lạ @ 0xE2000, MCFG @ 0xE3000.
    const RSDP_AT: u64 = EBDA_SCAN_START;
    const XSDT_AT: u64 = 0x000e_1000;
    const OTHER_AT: u64 = 0x000e_2000;
    const MCFG_AT: u64 = 0x000e_3000;
    let mut mem = FakeMem::new(EBDA_SCAN_START, 0x4000);

    let mut mcfg_body = vec![0u8; MCFG_RESERVED_BYTES + MCFG_ENTRY_BYTES];
    mcfg_body[MCFG_RESERVED_BYTES..MCFG_RESERVED_BYTES + 8]
        .copy_from_slice(&0xb000_0000u64.to_le_bytes());
    mcfg_body[MCFG_RESERVED_BYTES + 10] = 0;
    mcfg_body[MCFG_RESERVED_BYTES + 11] = 0xff;
    let mcfg = acpi_table(&MCFG_SIGNATURE, &mcfg_body);
    let other = acpi_table(b"FACP", &[0u8; 8]);
    // XSDT: hai entry 8 byte (bảng lạ trước, MCFG sau) — phải **đi hết** danh sách.
    let mut xsdt_body = Vec::new();
    xsdt_body.extend_from_slice(&OTHER_AT.to_le_bytes());
    xsdt_body.extend_from_slice(&MCFG_AT.to_le_bytes());
    let xsdt = acpi_table(b"XSDT", &xsdt_body);

    mem.put(RSDP_AT, &rsdp_v2(XSDT_AT));
    mem.put(XSDT_AT, &xsdt);
    mem.put(OTHER_AT, &other);
    mem.put(MCFG_AT, &mcfg);

    let (found_rsdp, parsed) = find_rsdp(&mem).expect("phải tìm được RSDP trong EBDA");
    assert_eq!(found_rsdp, RSDP_AT);
    assert_eq!(parsed.xsdt, XSDT_AT);
    let entry = find_mcfg(&mem).expect("phải tìm được MCFG qua XSDT");
    assert_eq!(entry.base, 0xb000_0000);
    assert_eq!(entry.bus_end, 0xff);
}

#[test]
fn find_mcfg_rejects_bad_checksum_and_missing_rsdp() {
    // (a) RSDP có chữ ký đúng nhưng checksum sai ⇒ **không** nhận.
    let mut mem = FakeMem::new(EBDA_SCAN_START, 0x4000);
    let mut rsdp = rsdp_v2(0x000e_1000);
    rsdp[8] ^= 0x01; // phá checksum 20 byte đầu
    mem.put(EBDA_SCAN_START, &rsdp);
    assert!(find_rsdp(&mem).is_none());
    assert!(find_mcfg(&mem).is_none());

    // (b) Không có RSDP (bộ nhớ toàn 0) ⇒ `None`, không đoán.
    let empty = FakeMem::new(EBDA_SCAN_START, 0x4000);
    assert!(find_rsdp(&empty).is_none());
    assert!(find_mcfg(&empty).is_none());

    // (c) RSDP trỏ tới XSDT **cụt** (độ dài < header) ⇒ `None`.
    let mut mem2 = FakeMem::new(EBDA_SCAN_START, 0x4000);
    mem2.put(EBDA_SCAN_START, &rsdp_v2(0x000e_1000));
    let mut short = vec![0u8; ACPI_HEADER_BYTES];
    short[0..4].copy_from_slice(b"XSDT");
    short[4..8].copy_from_slice(&8u32.to_le_bytes()); // độ dài vô lý (< header)
    mem2.put(0x000e_1000, &short);
    assert!(find_mcfg(&mem2).is_none());
}

#[test]
// Cửa sổ quét RSDP là **hằng của đặc tả ACPI** — chốt bằng giá trị tuyệt đối là mục đích của ca.
#[allow(clippy::assertions_on_constants)]
fn scan_window_covers_ebda_and_bios_rom() {
    // Cửa sổ quét RSDP là 0xE0000..0x100000 (EBDA + BIOS ROM) — đúng ACPI §5.2.2.
    assert_eq!(EBDA_SCAN_START, 0x000e_0000);
    assert_eq!(EBDA_SCAN_END, 0x0010_0000);
    assert!(EBDA_SCAN_END - EBDA_SCAN_START == 0x2_0000);
    // Tên loại truy cập để in evidence (D2-102: in chữ, không in số trần).
    assert_eq!(AccessKind::Ecam.name(), "ecam");
    assert_eq!(AccessKind::Legacy.name(), "legacy");
}
