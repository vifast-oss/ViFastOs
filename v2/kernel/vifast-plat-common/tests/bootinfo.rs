// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-402** — test host cho `BootInfo` v1: DTB/DTB hỏng + ACPI/MADT/FADT/MCFG/E820.
//!
//! Hai vế bắt buộc (khuôn KT-81):
//!
//! 1. **Xanh trên bảng thật** (dựng lại đúng hình dạng DTB QEMU `virt` riscv64/aarch64 và blob
//!    ACPI `q35`) — parser đọc ra đúng số hart, bản đồ RAM, địa chỉ PLIC/GIC/APIC/UART/ECAM.
//! 2. **Đỏ trên bảng hỏng**: magic sai, `totalsize` tràn, offset tràn, token lạ, chuỗi hỏng,
//!    checksum ACPI sai, thiếu MADT/MCFG, E820 hỏng — mỗi ca phải trả **lỗi có tên**, không panic
//!    và không đọc ngoài biên.

use vifast_plat_common::bootinfo::{
    parse_acpi, parse_e820, parse_fadt, parse_fdt, parse_madt, parse_mcfg, BootArch, BootInfo,
    BootInfoError, BootSource, UartKind, BOOTINFO_VERSION, E820_ENTRY_BYTES, FDT_MAGIC,
    MAX_USABLE_RAM, PVH_MEMMAP_ENTRY_BYTES, X86_COM1_BASE,
};

// =====================================================================================
// Dựng FDT/DTB tổng hợp (đúng bố cục header v17: header 40 B, rsvmap 16 B, struct, strings)
// =====================================================================================

struct FdtBuilder {
    structs: Vec<u8>,
    strings: Vec<u8>,
    names: Vec<String>,
}

impl FdtBuilder {
    fn new() -> Self {
        FdtBuilder {
            structs: Vec::new(),
            strings: Vec::new(),
            names: Vec::new(),
        }
    }

    fn u32(&mut self, value: u32) {
        self.structs.extend_from_slice(&value.to_be_bytes());
    }

    fn pad(&mut self) {
        while self.structs.len() % 4 != 0 {
            self.structs.push(0);
        }
    }

    fn begin(&mut self, name: &str) {
        self.u32(1);
        self.structs.extend_from_slice(name.as_bytes());
        self.structs.push(0);
        self.pad();
    }

    fn end(&mut self) {
        self.u32(2);
    }

    fn name_off(&mut self, name: &str) -> u32 {
        if let Some(i) = self.names.iter().position(|n| n == name) {
            // Tính lại offset: chuỗi đã nằm trong `strings` theo đúng thứ tự đã thêm.
            return self.names.iter().take(i).map(|n| n.len() as u32 + 1).sum();
        }
        let off = self.strings.len() as u32;
        self.strings.extend_from_slice(name.as_bytes());
        self.strings.push(0);
        self.names.push(name.to_string());
        off
    }

    fn prop(&mut self, name: &str, value: &[u8]) {
        self.u32(3);
        self.u32(value.len() as u32);
        let off = self.name_off(name);
        self.u32(off);
        self.structs.extend_from_slice(value);
        self.pad();
    }

    fn prop_u32(&mut self, name: &str, value: u32) {
        self.prop(name, &value.to_be_bytes());
    }

    fn finish(mut self, boot_cpuid: u32) -> Vec<u8> {
        self.u32(9); // FDT_END
        let off_struct = 56u32;
        let off_strings = off_struct + self.structs.len() as u32;
        let totalsize = off_strings + self.strings.len() as u32;
        let mut out = vec![0u8; totalsize as usize];
        out[0..4].copy_from_slice(&FDT_MAGIC.to_be_bytes());
        out[4..8].copy_from_slice(&totalsize.to_be_bytes());
        out[8..12].copy_from_slice(&off_struct.to_be_bytes());
        out[12..16].copy_from_slice(&off_strings.to_be_bytes());
        out[16..20].copy_from_slice(&40u32.to_be_bytes()); // off_mem_rsvmap
        out[20..24].copy_from_slice(&17u32.to_be_bytes()); // version
        out[24..28].copy_from_slice(&16u32.to_be_bytes()); // last_comp_version
        out[28..32].copy_from_slice(&boot_cpuid.to_be_bytes());
        out[32..36].copy_from_slice(&(self.strings.len() as u32).to_be_bytes());
        out[36..40].copy_from_slice(&(self.structs.len() as u32).to_be_bytes());
        out[off_struct as usize..off_strings as usize].copy_from_slice(&self.structs);
        out[off_strings as usize..].copy_from_slice(&self.strings);
        out
    }
}

/// `reg` với 2 address-cell + 2 size-cell (root `/virt` của QEMU).
fn reg2(base: u64, size: u64) -> Vec<u8> {
    let mut v = Vec::new();
    for cell in [
        (base >> 32) as u32,
        base as u32,
        (size >> 32) as u32,
        size as u32,
    ] {
        v.extend_from_slice(&cell.to_be_bytes());
    }
    v
}

/// `reg` với 1 address-cell (node `/cpus/cpu@N`).
fn reg1(addr: u32) -> Vec<u8> {
    addr.to_be_bytes().to_vec()
}

/// DTB hình dạng QEMU `virt` riscv64 (đo từ `-M virt,dumpdtb`): memory, cpus, PLIC, 16550, PCIe.
fn dtb_riscv(n_cpus: u32) -> Vec<u8> {
    let mut b = FdtBuilder::new();
    b.begin("");
    b.prop_u32("#address-cells", 2);
    b.prop_u32("#size-cells", 2);
    b.prop("compatible", b"riscv-virtio\0");
    b.begin("memory@80000000");
    b.prop("device_type", b"memory\0");
    b.prop("reg", &reg2(0x8000_0000, 0x0800_0000));
    b.end();
    b.begin("cpus");
    b.prop_u32("#address-cells", 1);
    b.prop_u32("#size-cells", 0);
    b.prop_u32("timebase-frequency", 10_000_000);
    for i in 0..n_cpus {
        b.begin(&format!("cpu@{i}"));
        b.prop("reg", &reg1(i));
        b.prop("status", b"okay\0");
        b.prop("riscv,isa", b"rv64imafdc\0");
        b.end();
    }
    b.end();
    b.begin("soc");
    b.begin("plic@c000000");
    b.prop("compatible", b"riscv,plic0\0");
    b.prop("reg", &reg2(0x0c00_0000, 0x0400_0000));
    b.end();
    b.begin("serial@10000000");
    b.prop("compatible", b"ns16550a\0");
    b.prop("reg", &reg2(0x1000_0000, 0x100));
    b.end();
    b.begin("pci@30000000");
    b.prop("compatible", b"pci-host-ecam-generic\0");
    b.prop("reg", &reg2(0x3000_0000, 0x1000_0000));
    b.end();
    b.end();
    b.begin("chosen");
    b.prop("stdout-path", b"/soc/serial@10000000\0");
    b.end();
    b.end();
    b.finish(0)
}

/// DTB hình dạng QEMU `virt` aarch64 (`gic-version=3`): GICv3 dist + redistributor, PL011, PCIe.
fn dtb_aarch64(n_cpus: u32) -> Vec<u8> {
    let mut b = FdtBuilder::new();
    b.begin("");
    b.prop_u32("#address-cells", 2);
    b.prop_u32("#size-cells", 2);
    b.prop("compatible", b"linux,dummy-virt\0");
    b.begin("memory@40000000");
    b.prop("device_type", b"memory\0");
    b.prop("reg", &reg2(0x4000_0000, 0x0800_0000));
    b.end();
    b.begin("cpus");
    b.prop_u32("#address-cells", 1);
    b.prop_u32("#size-cells", 0);
    b.prop_u32("timebase-frequency", 62_500_000);
    for i in 0..n_cpus {
        b.begin(&format!("cpu@{i}"));
        b.prop("reg", &reg1(i));
        b.prop("status", b"okay\0");
        b.end();
    }
    b.end();
    b.begin("intc@8000000");
    b.prop("compatible", b"arm,gic-v3\0");
    // 2 entry: GICD 0x0800_0000 + 0x1_0000; GICR 0x080a_0000 + 0xf6_0000.
    let mut reg = reg2(0x0800_0000, 0x1_0000);
    reg.extend_from_slice(&reg2(0x080a_0000, 0x00f6_0000));
    b.prop("reg", &reg);
    b.end();
    b.begin("pl011@9000000");
    b.prop("compatible", b"arm,pl011\0");
    b.prop("reg", &reg2(0x0900_0000, 0x1000));
    b.end();
    b.begin("pcie@10000000");
    b.prop("compatible", b"pci-host-ecam-generic\0");
    b.prop("reg", &reg2(0x40_1000_0000, 0x1000_0000));
    b.end();
    b.begin("chosen");
    b.prop("stdout-path", b"/pl011@9000000\0");
    b.end();
    b.end();
    b.finish(0)
}

fn expect_err<T>(result: Result<T, BootInfoError>, want: BootInfoError) {
    match result {
        Ok(_) => panic!("mong đợi lỗi {want:?}, nhưng parse thành công"),
        Err(got) => assert_eq!(got, want, "mã lỗi sai"),
    }
}

// =====================================================================================
// DTB — vế xanh
// =====================================================================================

#[test]
fn dtb_riscv_doc_dung_hinh_qemu_virt() {
    let dtb = dtb_riscv(4);
    let info = parse_fdt(&dtb, BootArch::Riscv64).expect("DTB QEMU virt riscv64 phải parse được");
    assert_eq!(info.version, BOOTINFO_VERSION);
    assert_eq!(info.arch, BootArch::Riscv64);
    assert_eq!(info.source, BootSource::Dtb);
    assert_eq!(info.n_harts, 4, "n_harts phải là số hart thật trong /cpus");
    assert_eq!(info.harts(), &[0, 1, 2, 3]);
    assert_eq!(info.boot_hart, 0);
    assert_eq!(info.n_ram, 1);
    assert_eq!(info.usable_ram()[0].base, 0x8000_0000);
    assert_eq!(info.usable_ram()[0].size, 0x0800_0000);
    assert_eq!(info.usable_ram_bytes(), 0x0800_0000);
    assert_eq!(info.plic, Some(0x0c00_0000));
    assert_eq!(info.gic, None);
    assert_eq!(
        info.uart.map(|u| (u.base, u.kind)),
        Some((0x1000_0000, UartKind::Ns16550))
    );
    assert_eq!(info.pcie_ecam, Some(0x3000_0000));
    assert_eq!(info.timebase_hz, Some(10_000_000));
    info.validate().expect("BootInfo phải qua validate");
}

#[test]
fn dtb_aarch64_chon_gic_va_pl011() {
    let dtb = dtb_aarch64(2);
    let info = parse_fdt(&dtb, BootArch::Aarch64).expect("DTB QEMU virt aarch64 phải parse được");
    assert_eq!(info.n_harts, 2);
    assert_eq!(info.harts(), &[0, 1]);
    assert_eq!(info.gic, Some(0x0800_0000));
    assert_eq!(info.gic_cpu, Some(0x080a_0000));
    assert_eq!(info.plic, None);
    assert_eq!(
        info.uart.map(|u| (u.base, u.kind)),
        Some((0x0900_0000, UartKind::Pl011))
    );
    assert_eq!(info.pcie_ecam, Some(0x40_1000_0000));
    assert_eq!(info.usable_ram()[0].base, 0x4000_0000);
}

#[test]
fn dtb_chon_uart_theo_stdout_path() {
    // Có hai UART; `/chosen/stdout-path` chỉ đúng cái thứ hai ⇒ phải chọn cái đó, không chọn
    // "ứng viên đầu tiên" một cách mù quáng.
    let mut b = FdtBuilder::new();
    b.begin("");
    b.prop_u32("#address-cells", 2);
    b.prop_u32("#size-cells", 2);
    b.begin("memory@80000000");
    b.prop("device_type", b"memory\0");
    b.prop("reg", &reg2(0x8000_0000, 0x0800_0000));
    b.end();
    b.begin("cpus");
    b.prop_u32("#address-cells", 1);
    b.prop_u32("#size-cells", 0);
    b.begin("cpu@0");
    b.prop("reg", &reg1(0));
    b.end();
    b.end();
    b.begin("plic@c000000");
    b.prop("compatible", b"riscv,plic0\0");
    b.prop("reg", &reg2(0x0c00_0000, 0x0400_0000));
    b.end();
    b.begin("serial@10000000");
    b.prop("compatible", b"ns16550a\0");
    b.prop("reg", &reg2(0x1000_0000, 0x100));
    b.end();
    b.begin("serial@10001000");
    b.prop("compatible", b"ns16550a\0");
    b.prop("reg", &reg2(0x1000_1000, 0x100));
    b.end();
    b.begin("pci@30000000");
    b.prop("compatible", b"pci-host-ecam-generic\0");
    b.prop("reg", &reg2(0x3000_0000, 0x1000_0000));
    b.end();
    b.begin("chosen");
    b.prop("stdout-path", b"/serial@10001000:115200\0");
    b.end();
    b.end();
    let dtb = b.finish(0);
    let info = parse_fdt(&dtb, BootArch::Riscv64).expect("phải parse được");
    assert_eq!(info.uart.map(|u| u.base), Some(0x1000_1000));
}

#[test]
fn dtb_bo_cpu_disabled_khoi_n_harts() {
    // cpu@0 enabled, cpu@1 `status = "disabled"` ⇒ n_harts = 1, không đếm hart bị tắt.
    let mut b = FdtBuilder::new();
    b.begin("");
    b.prop_u32("#address-cells", 2);
    b.prop_u32("#size-cells", 2);
    b.begin("memory@80000000");
    b.prop("device_type", b"memory\0");
    b.prop("reg", &reg2(0x8000_0000, 0x0800_0000));
    b.end();
    b.begin("cpus");
    b.prop_u32("#address-cells", 1);
    b.prop_u32("#size-cells", 0);
    b.begin("cpu@0");
    b.prop("reg", &reg1(0));
    b.prop("status", b"okay\0");
    b.end();
    b.begin("cpu@1");
    b.prop("reg", &reg1(1));
    b.prop("status", b"disabled\0");
    b.end();
    b.end();
    b.begin("plic@c000000");
    b.prop("compatible", b"riscv,plic0\0");
    b.prop("reg", &reg2(0x0c00_0000, 0x0400_0000));
    b.end();
    b.begin("serial@10000000");
    b.prop("compatible", b"ns16550a\0");
    b.prop("reg", &reg2(0x1000_0000, 0x100));
    b.end();
    b.begin("pci@30000000");
    b.prop("compatible", b"pci-host-ecam-generic\0");
    b.prop("reg", &reg2(0x3000_0000, 0x1000_0000));
    b.end();
    b.end();
    let dtb = b.finish(0);
    let info = parse_fdt(&dtb, BootArch::Riscv64).expect("DTB có hart disabled phải parse được");
    assert_eq!(info.n_harts, 1, "hart disabled không được đếm");
    assert_eq!(info.harts(), &[0]);
}

// =====================================================================================
// DTB — vế đỏ (fail-closed có tên, không panic)
// =====================================================================================

#[test]
fn dtb_magic_sai_tra_loi_co_ten() {
    let mut dtb = dtb_riscv(1);
    dtb[3] ^= 0xFF;
    expect_err(
        parse_fdt(&dtb, BootArch::Riscv64),
        BootInfoError::BadMagic {
            what: "fdt",
            got: u32::from_be_bytes([dtb[0], dtb[1], dtb[2], dtb[3]]),
        },
    );
}

#[test]
fn dtb_totalsize_tran_tra_loi_co_ten() {
    let mut dtb = dtb_riscv(1);
    let len = dtb.len() as u32;
    dtb[4..8].copy_from_slice(&(len + 4096).to_be_bytes());
    expect_err(
        parse_fdt(&dtb, BootArch::Riscv64),
        BootInfoError::Truncated {
            needed: (len + 4096) as usize,
            have: dtb.len(),
        },
    );
}

#[test]
fn dtb_offset_trung_bien_tra_loi_co_ten() {
    let mut dtb = dtb_riscv(1);
    let totalsize = dtb.len() as u32;
    // off_struct = totalsize (khối struct rỗng/ngoài biên).
    dtb[8..12].copy_from_slice(&totalsize.to_be_bytes());
    match parse_fdt(&dtb, BootArch::Riscv64) {
        Err(BootInfoError::BadOffset { .. }) => {}
        other => panic!("mong đợi BadOffset, nhận {other:?}"),
    }
}

#[test]
fn dtb_token_la_tra_loi_co_ten() {
    let mut dtb = dtb_riscv(1);
    // Token đầu tiên của khối struct ở offset 56.
    dtb[56..60].copy_from_slice(&7u32.to_be_bytes());
    expect_err(
        parse_fdt(&dtb, BootArch::Riscv64),
        BootInfoError::BadStructure { what: "fdt-token" },
    );
}

#[test]
fn dtb_chuoi_khong_ket_thuc_tra_loi_co_ten() {
    let mut dtb = dtb_riscv(1);
    // Bỏ byte NUL cuối cùng của khối strings ⇒ mọi tên thuộc tính sẽ không có NUL kết thúc.
    let n = dtb.len();
    let string_size = u32::from_be_bytes([dtb[32], dtb[33], dtb[34], dtb[35]]) as usize;
    let off_strings = u32::from_be_bytes([dtb[12], dtb[13], dtb[14], dtb[15]]) as usize;
    assert_eq!(off_strings + string_size, n);
    dtb[n - 1] = b'X';
    expect_err(
        parse_fdt(&dtb, BootArch::Riscv64),
        BootInfoError::BadStructure { what: "fdt-string" },
    );
}

#[test]
fn dtb_thieu_memory_tra_loi_co_ten() {
    let mut b = FdtBuilder::new();
    b.begin("");
    b.prop_u32("#address-cells", 2);
    b.prop_u32("#size-cells", 2);
    b.begin("cpus");
    b.prop_u32("#address-cells", 1);
    b.prop_u32("#size-cells", 0);
    b.begin("cpu@0");
    b.prop("reg", &reg1(0));
    b.end();
    b.end();
    b.begin("plic@c000000");
    b.prop("compatible", b"riscv,plic0\0");
    b.prop("reg", &reg2(0x0c00_0000, 0x0400_0000));
    b.end();
    b.begin("serial@10000000");
    b.prop("compatible", b"ns16550a\0");
    b.prop("reg", &reg2(0x1000_0000, 0x100));
    b.end();
    b.begin("pci@30000000");
    b.prop("compatible", b"pci-host-ecam-generic\0");
    b.prop("reg", &reg2(0x3000_0000, 0x1000_0000));
    b.end();
    b.end();
    let dtb = b.finish(0);
    expect_err(
        parse_fdt(&dtb, BootArch::Riscv64),
        BootInfoError::MissingMemory,
    );
}

#[test]
fn dtb_thieu_plic_tra_loi_co_ten() {
    let mut b = FdtBuilder::new();
    b.begin("");
    b.prop_u32("#address-cells", 2);
    b.prop_u32("#size-cells", 2);
    b.begin("memory@80000000");
    b.prop("device_type", b"memory\0");
    b.prop("reg", &reg2(0x8000_0000, 0x0800_0000));
    b.end();
    b.begin("cpus");
    b.prop_u32("#address-cells", 1);
    b.prop_u32("#size-cells", 0);
    b.begin("cpu@0");
    b.prop("reg", &reg1(0));
    b.end();
    b.end();
    b.begin("serial@10000000");
    b.prop("compatible", b"ns16550a\0");
    b.prop("reg", &reg2(0x1000_0000, 0x100));
    b.end();
    b.begin("pci@30000000");
    b.prop("compatible", b"pci-host-ecam-generic\0");
    b.prop("reg", &reg2(0x3000_0000, 0x1000_0000));
    b.end();
    b.end();
    let dtb = b.finish(0);
    expect_err(
        parse_fdt(&dtb, BootArch::Riscv64),
        BootInfoError::MissingNode("plic"),
    );
}

#[test]
fn dtb_cat_cut_moi_do_dai_khong_panic() {
    // Vế chống đọc ngoài biên: cắt DTB ở **mọi** độ dài < độ dài thật ⇒ luôn `Err`, không panic.
    let dtb = dtb_riscv(2);
    for cut in 0..dtb.len() {
        let r = parse_fdt(&dtb[..cut], BootArch::Riscv64);
        assert!(r.is_err(), "cắt {cut}/{} byte vẫn parse được", dtb.len());
    }
}

#[test]
fn dtb_sua_mot_byte_bat_ky_khong_panic() {
    // Mọi phép đột biến một byte trong header/vùng cấu trúc đều phải là Err hoặc Ok hợp lệ —
    // tuyệt đối không panic/đọc ngoài biên.
    let base = dtb_riscv(2);
    for i in 0..base.len().min(200) {
        let mut dtb = base.clone();
        dtb[i] = dtb[i].wrapping_add(0x5A);
        // `parse_fdt` đã tự `validate()` trước khi trả `Ok`; ở đây chỉ cần **có răng**: mọi
        // kết quả phải là `Ok` hợp lệ hoặc lỗi **có tên**, không panic/không đọc ngoài biên.
        match parse_fdt(&dtb, BootArch::Riscv64) {
            Ok(info) => assert!(
                info.validate().is_ok(),
                "đột biến byte {i}: parse Ok nhưng validate hỏng"
            ),
            Err(e) => assert!(!e.name().is_empty(), "đột biến byte {i}: lỗi không có tên"),
        }
    }
}

// =====================================================================================
// ACPI — bảng tổng hợp
// =====================================================================================

fn acpi_checksum(table: &mut [u8]) {
    table[9] = 0;
    let sum = table.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
    table[9] = 0u8.wrapping_sub(sum);
}

fn acpi_madt(lapic: u32, cpu_ids: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(&lapic.to_le_bytes());
    body.extend_from_slice(&1u32.to_le_bytes()); // PCAT_COMPAT
    for id in cpu_ids {
        body.extend_from_slice(&[0u8, 8, 0, *id]); // Processor Local APIC
        body.extend_from_slice(&1u32.to_le_bytes()); // Enabled
    }
    body.extend_from_slice(&[1u8, 12, 0, 0]); // IOAPIC
    body.extend_from_slice(&0xFEC0_0000u32.to_le_bytes());
    body.extend_from_slice(&0u32.to_le_bytes());
    let len = 36 + body.len();
    let mut table = vec![0u8; len];
    table[..4].copy_from_slice(b"APIC");
    table[4..8].copy_from_slice(&(len as u32).to_le_bytes());
    table[8] = 5;
    table[36..].copy_from_slice(&body);
    acpi_checksum(&mut table);
    table
}

fn acpi_fadt(pm1a_legacy: u32, x_pm1a: u64) -> Vec<u8> {
    let mut table = vec![0u8; 276];
    table[..4].copy_from_slice(b"FACP");
    table[4..8].copy_from_slice(&276u32.to_le_bytes());
    table[8] = 6;
    table[64..68].copy_from_slice(&pm1a_legacy.to_le_bytes());
    table[176..184].copy_from_slice(&x_pm1a.to_le_bytes());
    acpi_checksum(&mut table);
    table
}

fn acpi_mcfg(ecam: u64, start_bus: u8, end_bus: u8) -> Vec<u8> {
    let mut table = vec![0u8; 60];
    table[..4].copy_from_slice(b"MCFG");
    table[4..8].copy_from_slice(&60u32.to_le_bytes());
    table[8] = 1;
    table[44..52].copy_from_slice(&ecam.to_le_bytes());
    table[52] = start_bus;
    table[53] = end_bus;
    acpi_checksum(&mut table);
    table
}

fn e820_blob(entries: &[(u64, u64, u32)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (base, size, kind) in entries {
        out.extend_from_slice(&base.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&kind.to_le_bytes());
    }
    out
}

fn e820_q35() -> Vec<u8> {
    e820_blob(&[
        (0, 0x9fc00, 1),
        (0x9fc00, 0x400, 2),
        (0xf0000, 0x10000, 2),
        (0x100000, 0x07ee0000, 1),
        (0x7fe0000, 0x20000, 2),
        (0xfffc0000, 0x40000, 2),
    ])
}

fn concat(tables: &[&[u8]]) -> Vec<u8> {
    let mut out = Vec::new();
    for t in tables {
        out.extend_from_slice(t);
    }
    out
}

// =====================================================================================
// ACPI — vế xanh
// =====================================================================================

#[test]
fn acpi_madt_doc_hart_that() {
    let table = acpi_madt(0xFEE0_0000, &[0, 1, 2, 3]);
    let madt = parse_madt(&table).expect("MADT hợp lệ phải parse được");
    assert_eq!(madt.local_apic, 0xFEE0_0000);
    assert_eq!(madt.n_cpus, 4);
    assert_eq!(madt.cpu_ids[..4], [0, 1, 2, 3]);
    assert_eq!(madt.ioapic, Some(0xFEC0_0000));
}

#[test]
fn acpi_fadt_uu_tien_x_pm1a_va_checksum() {
    let table = acpi_fadt(0x0604, 0x0000_0000_0000_1800);
    let fadt = parse_fadt(&table).expect("FADT hợp lệ phải parse được");
    assert_eq!(fadt.revision, 6);
    assert_eq!(fadt.pm1a_cnt, Some(0x1800), "phải ưu tiên X_PM1a_CNT_BLK");
    let table = acpi_fadt(0x0604, 0);
    let fadt = parse_fadt(&table).expect("FADT legacy hợp lệ");
    assert_eq!(fadt.pm1a_cnt, Some(0x0604));
}

#[test]
fn acpi_mcfg_doc_ecam() {
    let table = acpi_mcfg(0xE000_0000, 0, 0xFF);
    let mcfg = parse_mcfg(&table).expect("MCFG hợp lệ phải parse được");
    assert_eq!(mcfg.ecam, 0xE000_0000);
    assert_eq!(mcfg.start_bus, 0);
    assert_eq!(mcfg.end_bus, 0xFF);
}

#[test]
fn acpi_du_bo_bang_dung_hinh_q35() {
    let tables = concat(&[
        &acpi_fadt(0x0604, 0),
        &acpi_madt(0xFEE0_0000, &[0, 1, 2, 3]),
        &acpi_mcfg(0xE000_0000, 0, 0xFF),
    ]);
    let info = parse_acpi(&tables, &e820_q35(), E820_ENTRY_BYTES)
        .expect("ACPI q35 đủ bảng phải parse được");
    assert_eq!(info.arch, BootArch::X86_64);
    assert_eq!(info.source, BootSource::Acpi);
    assert_eq!(info.n_harts, 4);
    assert_eq!(info.harts(), &[0, 1, 2, 3]);
    assert_eq!(info.apic, Some(0xFEE0_0000));
    assert_eq!(info.pcie_ecam, Some(0xE000_0000));
    assert_eq!(info.pm1a_cnt, Some(0x0604));
    assert_eq!(info.uart.map(|u| u.base), Some(X86_COM1_BASE));
    assert_eq!(info.n_ram, 2, "e820 có đúng 2 entry usable RAM");
    assert_eq!(info.usable_ram()[0].base, 0);
    assert_eq!(info.usable_ram()[1].base, 0x100000);
    assert_eq!(info.usable_ram_bytes(), 0x9fc00 + 0x07ee0000);
}

#[test]
fn acpi_e820_memmap_pvh_stride_24() {
    // Memmap PVH: 24 byte/entry (addr u64, size u64, type u32, reserved u32).
    let mut blob = Vec::new();
    for (base, size, kind) in [(0u64, 0x9fc00u64, 1u32), (0x100000, 0x07ee0000, 1)] {
        blob.extend_from_slice(&base.to_le_bytes());
        blob.extend_from_slice(&size.to_le_bytes());
        blob.extend_from_slice(&kind.to_le_bytes());
        blob.extend_from_slice(&0u32.to_le_bytes());
    }
    let e820 = parse_e820(&blob, PVH_MEMMAP_ENTRY_BYTES).expect("memmap PVH hợp lệ");
    assert_eq!(e820.n, 2);
    assert_eq!(e820.seen, 2);
    assert_eq!(e820.entries[1].base, 0x100000);
}

// =====================================================================================
// ACPI — vế đỏ (fail-closed có tên)
// =====================================================================================

#[test]
fn acpi_madt_checksum_sai_tra_loi_co_ten() {
    let mut table = acpi_madt(0xFEE0_0000, &[0, 1]);
    let n = table.len();
    table[n - 1] ^= 0xFF;
    expect_err(
        parse_madt(&table),
        BootInfoError::BadChecksum { what: "madt" },
    );
}

#[test]
fn acpi_madt_do_dai_tran_tra_loi_co_ten() {
    let mut table = acpi_madt(0xFEE0_0000, &[0, 1]);
    table[4..8].copy_from_slice(&0xFFFFu32.to_le_bytes());
    expect_err(parse_madt(&table), BootInfoError::BadTable("madt-length"));
}

#[test]
fn acpi_madt_khong_cpu_enabled_tra_loi_co_ten() {
    // MADT chỉ có entry IOAPIC (không hart nào) ⇒ MissingHarts.
    let mut body = Vec::new();
    body.extend_from_slice(&0xFEE0_0000u32.to_le_bytes());
    body.extend_from_slice(&0u32.to_le_bytes());
    body.extend_from_slice(&[1u8, 12, 0, 0]);
    body.extend_from_slice(&0xFEC0_0000u32.to_le_bytes());
    body.extend_from_slice(&0u32.to_le_bytes());
    let len = 36 + body.len();
    let mut table = vec![0u8; len];
    table[..4].copy_from_slice(b"APIC");
    table[4..8].copy_from_slice(&(len as u32).to_le_bytes());
    table[8] = 5;
    table[36..].copy_from_slice(&body);
    acpi_checksum(&mut table);
    expect_err(parse_madt(&table), BootInfoError::MissingHarts);
}

#[test]
fn acpi_madt_cat_ngan_tra_loi_co_ten() {
    let table = acpi_madt(0xFEE0_0000, &[0, 1]);
    expect_err(
        parse_madt(&table[..40]),
        BootInfoError::Truncated {
            needed: 44,
            have: 40,
        },
    );
}

#[test]
fn acpi_fadt_checksum_sai_tra_loi_co_ten() {
    let mut table = acpi_fadt(0x0604, 0);
    table[70] ^= 0xFF;
    expect_err(
        parse_fadt(&table),
        BootInfoError::BadChecksum { what: "fadt" },
    );
}

#[test]
fn acpi_mcfg_checksum_sai_tra_loi_co_ten() {
    let mut table = acpi_mcfg(0xE000_0000, 0, 0xFF);
    let n = table.len();
    table[n - 1] ^= 0xFF;
    expect_err(
        parse_mcfg(&table),
        BootInfoError::BadChecksum { what: "mcfg" },
    );
}

#[test]
fn acpi_thieu_madt_tra_loi_co_ten() {
    let tables = concat(&[&acpi_fadt(0x0604, 0), &acpi_mcfg(0xE000_0000, 0, 0xFF)]);
    expect_err(
        parse_acpi(&tables, &e820_q35(), E820_ENTRY_BYTES),
        BootInfoError::MissingTable("madt"),
    );
}

#[test]
fn acpi_thieu_mcfg_tra_loi_co_ten() {
    let tables = concat(&[&acpi_fadt(0x0604, 0), &acpi_madt(0xFEE0_0000, &[0, 1])]);
    expect_err(
        parse_acpi(&tables, &e820_q35(), E820_ENTRY_BYTES),
        BootInfoError::MissingTable("mcfg"),
    );
}

#[test]
fn acpi_bang_rac_giua_blob_tra_loi_co_ten() {
    // Một bảng có `length` tràn trước bảng cần tìm ⇒ BadTable, không trượt sang rác.
    let mut bad = vec![0u8; 40];
    bad[..4].copy_from_slice(b"DSDT");
    bad[4..8].copy_from_slice(&0xFFFFu32.to_le_bytes());
    let tables = concat(&[&bad, &acpi_madt(0xFEE0_0000, &[0, 1])]);
    expect_err(
        parse_acpi(&tables, &e820_q35(), E820_ENTRY_BYTES),
        BootInfoError::BadTable("madt"),
    );
}

#[test]
fn acpi_e820_hong_do_dai_tra_loi_co_ten() {
    let tables = concat(&[
        &acpi_fadt(0x0604, 0),
        &acpi_madt(0xFEE0_0000, &[0, 1]),
        &acpi_mcfg(0xE000_0000, 0, 0xFF),
    ]);
    let mut blob = e820_q35();
    blob.push(0); // 20*n + 1 ⇒ không chia hết
    expect_err(
        parse_acpi(&tables, &blob, E820_ENTRY_BYTES),
        BootInfoError::BadTable("e820-length"),
    );
}

#[test]
fn acpi_e820_khong_ram_usable_tra_loi_co_ten() {
    let tables = concat(&[
        &acpi_fadt(0x0604, 0),
        &acpi_madt(0xFEE0_0000, &[0, 1]),
        &acpi_mcfg(0xE000_0000, 0, 0xFF),
    ]);
    let blob = e820_blob(&[(0, 0x9fc00, 2), (0x100000, 0x1000, 3)]);
    expect_err(
        parse_acpi(&tables, &blob, E820_ENTRY_BYTES),
        BootInfoError::MissingMemory,
    );
}

// =====================================================================================
// BootInfo — trần và log
// =====================================================================================

#[test]
fn bootinfo_tran_vung_ram_va_hart_co_ten() {
    let mut info = BootInfo::new(BootArch::Riscv64, BootSource::Dtb);
    for i in 0..MAX_USABLE_RAM {
        info.push_ram(0x1000 * (i as u64 + 1), 0x1000)
            .expect("trong trần phải nhận");
    }
    expect_err_push(info.push_ram(0x20_0000, 0x1000));

    let mut info = BootInfo::new(BootArch::Riscv64, BootSource::Dtb);
    for i in 0u32..8 {
        info.push_hart(i).expect("trong trần phải nhận");
    }
    info.push_hart(0).expect("trùng id không tính hai lần");
    expect_err_push(info.push_hart(8));
}

fn expect_err_push(result: Result<(), BootInfoError>) {
    match result {
        Ok(()) => panic!("mong đợi lỗi, nhưng thành công"),
        Err(BootInfoError::BadRegion) => {}
        Err(BootInfoError::TooManyRegions) => {}
        Err(BootInfoError::TooManyHarts) => {}
        Err(other) => panic!("mã lỗi bất ngờ: {other:?}"),
    }
}

#[test]
fn bootinfo_vung_ram_khong_hop_le_bi_tu_choi() {
    let mut info = BootInfo::new(BootArch::Riscv64, BootSource::Dtb);
    expect_err_push(info.push_ram(u64::MAX - 0x800, 0x1000)); // tràn địa chỉ
    assert_eq!(
        info.push_ram(0, 0),
        Ok(()),
        "size 0 bị bỏ qua, không phải lỗi"
    );
    // E820 khai theo byte: vùng `0x9fc00` byte **hợp lệ**, parser không được bỏ.
    info.push_ram(0, 0x9fc00)
        .expect("vùng RAM không căn 4 KiB vẫn hợp lệ");
}

#[test]
fn bootinfo_log_co_dong_bootinfo_va_arch() {
    let dtb = dtb_riscv(2);
    let info = parse_fdt(&dtb, BootArch::Riscv64).expect("parse được");
    // Ghi qua helper của BootInfo (kiểm đúng dòng log để runner/người đọc grep được).
    let mut buf = String::new();
    let mut w = LogString::new(&mut buf);
    info.write_log(&mut w).expect("ghi log");
    assert!(
        buf.contains("# bootinfo: v=1 arch=riscv64 source=dtb n_harts=2"),
        "{buf}"
    );
    assert!(
        buf.contains("# bootinfo: ram[0]=0x80000000+0x8000000"),
        "{buf}"
    );
    assert!(
        buf.contains("# bootinfo: harts=0,1 timebase=0x989680"),
        "{buf}"
    );
    assert!(buf.contains("plic=0xc000000"), "{buf}");
    assert!(buf.contains("uart=ns16550@0x10000000"), "{buf}");
    assert!(buf.contains("ecam=0x30000000"), "{buf}");
}

// Helper nhỏ chỉ để giữ `core::fmt::Write` mà không phụ thuộc `alloc` trong crate sản phẩm.
struct LogString<'a>(&'a mut String);

impl<'a> LogString<'a> {
    fn new(s: &'a mut String) -> Self {
        LogString(s)
    }
}

impl core::fmt::Write for LogString<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.0.push_str(s);
        Ok(())
    }
}
