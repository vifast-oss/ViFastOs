// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test host (`cargo test --lib`) — chạy **cùng** thân kiểm với ảnh `.efi`, không QEMU:
//!
//! 1. bố cục `repr(C)` của các bảng UEFI (khoá ABI: `size_of`/`offset_of` — sai offset là gọi
//!    hàm rác trên máy thật);
//! 2. `bootinfo` include từ kernel **không** phải bản sao: `MAX_HARTS` khớp
//!    `vifast-plat-common/src/smp.rs`, tên lỗi kernel đúng mã chữ;
//! 3. `memmap::validate` — ca xanh + **toàn bộ ca âm có tên** (đỏ-trước ở mức hàm);
//! 4. `UefiHandoffV1` — checksum/verify, magic sai/version sai/bị sửa ⇒ lỗi có tên.

use core::mem::{offset_of, size_of};

use vifast_boot_uefi::bootinfo::{
    BootArch, BootInfo, BootInfoError, BootSource, BOOTINFO_VERSION, MAX_USABLE_RAM,
};
use vifast_boot_uefi::efi::{
    find_config_table, BootServices, ConfigurationTable, MemoryDescriptor, SimpleTextOutput,
    SystemTable, TableHeader, ACPI_10_TABLE_GUID, ACPI_20_TABLE_GUID, DTB_TABLE_GUID,
};
use vifast_boot_uefi::handoff::{
    HandoffRam, UefiHandoffV1, HANDOFF_BYTES, HANDOFF_MAGIC, HANDOFF_RESERVED_INVARIANT,
};
use vifast_boot_uefi::memmap::{
    apply_fault, validate, MapInfo, UefiError, DESC_MIN, MAX_DESCRIPTORS, PAGE_BYTES,
};

// ---------------------------------------------------------------- bố cục ABI UEFI

#[test]
fn table_header_is_24_bytes() {
    assert_eq!(size_of::<TableHeader>(), 24);
}

#[test]
fn memory_descriptor_layout_is_uefi_2_10() {
    assert_eq!(size_of::<MemoryDescriptor>(), 40);
    assert_eq!(size_of::<MemoryDescriptor>(), DESC_MIN);
    assert_eq!(offset_of!(MemoryDescriptor, ty), 0);
    assert_eq!(offset_of!(MemoryDescriptor, pad), 4);
    assert_eq!(offset_of!(MemoryDescriptor, physical_start), 8);
    assert_eq!(offset_of!(MemoryDescriptor, virtual_start), 16);
    assert_eq!(offset_of!(MemoryDescriptor, number_of_pages), 24);
    assert_eq!(offset_of!(MemoryDescriptor, attribute), 32);
}

#[test]
fn boot_services_offsets_are_uefi_2_10() {
    // 27 con trỏ sau header 24 B ⇒ ExitBootServices ở offset 232, GetMemoryMap ở 56.
    assert_eq!(size_of::<BootServices>(), 240);
    assert_eq!(offset_of!(BootServices, get_memory_map), 56);
    assert_eq!(offset_of!(BootServices, exit_boot_services), 232);
}

#[test]
fn system_table_and_config_table_layout() {
    assert_eq!(size_of::<SystemTable>(), 120);
    assert_eq!(offset_of!(SystemTable, con_out), 64);
    assert_eq!(offset_of!(SystemTable, boot_services), 96);
    assert_eq!(offset_of!(SystemTable, number_of_table_entries), 104);
    assert_eq!(offset_of!(SystemTable, configuration_table), 112);
    assert_eq!(size_of::<ConfigurationTable>(), 24);
    assert_eq!(size_of::<SimpleTextOutput>(), 80);
}

// ---------------------------------------------------------------- bootinfo gốc của kernel

#[test]
fn bootinfo_is_included_from_kernel_not_copied() {
    // `include_str!` đọc **chính** nguồn sự thật; lệch số hart là đỏ (không thể trôi âm thầm).
    const SMP: &str = include_str!("../../../kernel/vifast-plat-common/src/smp.rs");
    assert!(
        SMP.contains("pub const MAX_HARTS: usize = 8;"),
        "smp.rs đổi MAX_HARTS ⇒ shim trong src/lib.rs phải đổi theo"
    );
    // Ảnh `.efi` dùng đúng tên lỗi của kernel (không phải bản chép).
    assert_eq!(BootInfoError::MissingMemory.name(), "missing-memory");
    assert_eq!(BootInfoError::BadChecksum { what: "x" }.name(), "bad-checksum");
    assert_eq!(BootInfoError::UnsupportedVersion { what: "x", got: 2 }.name(), "unsupported-version");
    assert_eq!(BootInfoError::TooManyRegions.name(), "too-many-regions");
    // **D2-260** — nguồn thứ ba `Uefi` có trong `BootInfo` v1, **giữ nguyên** `BOOTINFO_VERSION`.
    assert_eq!(BootSource::Uefi.name(), "uefi");
    assert_eq!(BootSource::Dtb.name(), "dtb");
    assert_eq!(BootSource::Acpi.name(), "acpi");
    assert_eq!(BOOTINFO_VERSION, 1);
    let info = BootInfo::new(BootArch::X86_64, BootSource::Uefi);
    assert_eq!(info.version, 1);
    assert_eq!(info.source, BootSource::Uefi);
    assert_eq!(MAX_USABLE_RAM, 16);
}

// ---------------------------------------------------------------- `BootSource::Uefi` (D2-260)

/// `BootInfo` nguồn `Uefi` **không** đòi `apic`/`uart`/`pcie_ecam` (ACPI chưa parse ở bước stub)
/// nhưng **vẫn** fail-closed khi không có RAM hoặc vùng RAM hỏng.
#[test]
fn bootsource_uefi_validate_is_fail_closed_by_source() {
    let mut info = BootInfo::new(BootArch::X86_64, BootSource::Uefi);
    info.n_harts = 1;
    info.hart_ids[0] = 0;
    info.boot_hart = 0;
    // Không có RAM ⇒ `missing-memory` (không được "cho qua vì là Uefi").
    assert_eq!(info.validate(), Err(BootInfoError::MissingMemory));
    info.push_ram(0x10_0000, 0x1000).expect("vùng hợp lệ");
    // Đủ RAM ⇒ hợp lệ dù `apic`/`uart`/`pcie_ecam` đều `None` (đúng bước stub `GetMemoryMap`).
    assert_eq!(info.validate(), Ok(()));
    // Vùng RAM tràn địa chỉ (đặt thẳng vào mảng, bỏ qua `push_ram`) ⇒ `bad-region`.
    let mut bad = info;
    bad.ram[0].size = u64::MAX;
    assert_eq!(bad.validate(), Err(BootInfoError::BadRegion));
    // `n_ram` khai 0 dù mảng có dữ liệu ⇒ vẫn `missing-memory`.
    let mut zero = info;
    zero.n_ram = 0;
    assert_eq!(zero.validate(), Err(BootInfoError::MissingMemory));
}

/// `Dtb`/`Acpi` **giữ nguyên 100 %** điều kiện nghiêm ngặt — thêm `Uefi` không được nới chúng.
#[test]
fn bootsource_dtb_acpi_keep_strict_conditions() {
    // `Acpi` thiếu `apic` ⇒ `missing-node:apic` (dù có RAM).
    let mut acpi = BootInfo::new(BootArch::X86_64, BootSource::Acpi);
    acpi.n_harts = 1;
    acpi.hart_ids[0] = 0;
    acpi.boot_hart = 0;
    acpi.push_ram(0x10_0000, 0x1000).expect("vùng hợp lệ");
    assert_eq!(
        acpi.validate(),
        Err(BootInfoError::MissingNode("apic")),
        "Acpi vẫn phải đòi apic"
    );
    // `Dtb` thiếu PLIC ⇒ `missing-node:plic`; thiếu hart ⇒ `missing-harts`.
    let mut dtb = BootInfo::new(BootArch::Riscv64, BootSource::Dtb);
    dtb.push_ram(0x8000_0000, 0x1000).expect("vùng hợp lệ");
    assert_eq!(dtb.validate(), Err(BootInfoError::MissingHarts));
    dtb.n_harts = 1;
    dtb.hart_ids[0] = 0;
    dtb.boot_hart = 0;
    assert_eq!(dtb.validate(), Err(BootInfoError::MissingNode("plic")));
    // Cùng dữ liệu nhưng nguồn `Uefi` ⇒ hợp lệ (fail-closed **theo nguồn**, không phải nới chung).
    let mut uefi = dtb;
    uefi.source = BootSource::Uefi;
    assert_eq!(uefi.validate(), Ok(()));
}

/// `bootinfo_from_memmap` — hàm **dùng chung** ảnh `.efi` + test host: bảng nhớ hợp lệ ⇒
/// `BootInfo` nguồn `Uefi`; bảng nhớ không còn vùng usable ⇒ `missing-memory`.
#[test]
fn bootinfo_from_memmap_uses_uefi_source() {
    let (buf, info) = valid_map();
    let st = unsafe { validate(buf.as_ptr(), &info) }.expect("bảng mẫu hợp lệ");
    let bi = vifast_boot_uefi::bootinfo_from_memmap(&st).expect("phải dựng được BootInfo");
    assert_eq!(bi.source, BootSource::Uefi);
    assert_eq!(bi.arch, BootArch::X86_64);
    assert_eq!(bi.version, 1);
    assert_eq!(bi.n_ram, st.n_ram);
    assert_eq!(bi.usable_ram_bytes(), st.usable_bytes);
    assert_eq!(bi.n_harts, 1);
    assert_eq!(bi.boot_hart, 0);
    assert_eq!(bi.apic, None, "chưa parse ACPI ⇒ None, không khai man");
    // Ghi log: dòng đầu **đúng** định dạng kernel mà runner grep.
    let mut buf = String::new();
    let mut w = LogString::new(&mut buf);
    bi.write_log(&mut w).expect("ghi log");
    assert!(
        buf.contains("# bootinfo: v=1 arch=x86_64 source=uefi n_harts=1 boot_hart=0"),
        "{buf}"
    );
    assert!(buf.contains("# bootinfo: ram n=2 bytes=0x"), "{buf}");
}

/// Ca âm **của `BootInfo::validate` theo nguồn**: bảng nhớ vẫn hợp lệ theo UEFI nhưng **không
/// còn** vùng usable nào ⇒ `memmap::validate` xanh, `bootinfo_from_memmap` đỏ `missing-memory`.
#[test]
fn no_usable_fault_passes_memmap_but_fails_bootinfo() {
    let (mut buf, mut info) = valid_map();
    // SAFETY: buffer đủ `capacity`; `info` khớp bảng vừa dựng.
    let injected = unsafe { apply_fault(buf.as_mut_ptr(), &mut info, "no-usable") };
    assert!(injected, "fault no-usable phải tiêm được");
    // SAFETY: như trên.
    let st = unsafe { validate(buf.as_ptr(), &info) }.expect("bảng nhớ vẫn hợp lệ theo UEFI");
    assert_eq!(st.n_usable, 0);
    assert_eq!(st.n_ram, 0);
    assert_eq!(
        vifast_boot_uefi::bootinfo_from_memmap(&st),
        Err(BootInfoError::MissingMemory),
        "không có RAM usable ⇒ BootInfo phải fail-closed"
    );
}

// ---------------------------------------------------------------- bảng cấu hình UEFI

#[test]
fn config_table_lookup_by_guid() {
    let mut tables = [
        ConfigurationTable {
            vendor_guid: [0xAA; 16],
            vendor_table: 0x1000 as *mut core::ffi::c_void,
        },
        ConfigurationTable {
            vendor_guid: ACPI_20_TABLE_GUID,
            vendor_table: 0x7f00_0000 as *mut core::ffi::c_void,
        },
        ConfigurationTable {
            vendor_guid: DTB_TABLE_GUID,
            vendor_table: 0x7f00_1000 as *mut core::ffi::c_void,
        },
    ];
    assert_eq!(find_config_table(&tables, &ACPI_20_TABLE_GUID), 0x7f00_0000);
    assert_eq!(find_config_table(&tables, &DTB_TABLE_GUID), 0x7f00_1000);
    assert_eq!(
        find_config_table(&tables, &ACPI_10_TABLE_GUID),
        0,
        "không có ⇒ 0"
    );
    // GUID ACPI 1.0 là **GUID khác** — không được khớp lẫn với ACPI 2.0.
    assert_ne!(ACPI_10_TABLE_GUID, ACPI_20_TABLE_GUID);
    // Mảng rỗng ⇒ 0 (không panic).
    assert_eq!(find_config_table(&[], &ACPI_20_TABLE_GUID), 0);
    // Bảng đầu tiên khớp ⇒ trả bảng đó (không phải bảng cuối).
    tables[0].vendor_guid = ACPI_20_TABLE_GUID;
    tables[0].vendor_table = 0x5000 as *mut core::ffi::c_void;
    assert_eq!(find_config_table(&tables, &ACPI_20_TABLE_GUID), 0x5000);
}

// ---------------------------------------------------------------- bảng nhớ giả lập

/// Dựng buffer mô tả UEFI với stride 48 (40 + 8 đệm) như OVMF.
fn map_fixture(descs: &[(u32, u64, u64, u64)], extra_stride: usize) -> (Vec<u8>, MapInfo) {
    let stride = DESC_MIN + extra_stride;
    let mut buf = vec![0u8; stride * descs.len()];
    for (i, &(ty, phys, pages, attr)) in descs.iter().enumerate() {
        let d = MemoryDescriptor {
            ty,
            pad: 0,
            physical_start: phys,
            virtual_start: phys,
            number_of_pages: pages,
            attribute: attr,
        };
        let off = i * stride;
        // SAFETY: `off + 40 <= buf.len()`.
        unsafe {
            core::ptr::write_unaligned(buf.as_mut_ptr().add(off).cast::<MemoryDescriptor>(), d);
        }
    }
    let info = MapInfo {
        map_size: buf.len(),
        desc_size: stride,
        desc_version: 1,
        map_key: 0x1234_5678,
        capacity: 64 * 1024,
    };
    (buf, info)
}

/// Bảng mẫu hợp lệ: loader code, conventional kề nhau (phải gộp), runtime, ACPI, MMIO.
fn valid_map() -> (Vec<u8>, MapInfo) {
    map_fixture(
        &[
            (1, 0x0000_0000, 0x10, 8),          // loader code 64 KiB @0 — usable
            (7, 0x0010_0000, 0x20, 8),          // conventional 128 KiB @1 MiB — usable
            (7, 0x0012_0000, 0x10, 8),          // kề ngay sau ⇒ gộp thành 192 KiB
            (6, 0x0020_0000, 0x08, 0x8000_0008), // runtime data 32 KiB
            (9, 0x0030_0000, 0x04, 8),          // ACPI reclaim 16 KiB
            (11, 0xFEBF_0000, 0x01, 1),         // MMIO UC
        ],
        8,
    )
}

fn expect_err(err: UefiError) -> &'static str {
    err.name()
}

#[test]
fn valid_map_ok_and_usable_regions_merged() {
    let (buf, info) = valid_map();
    // SAFETY: buffer đủ `map_size` byte.
    let st = unsafe { validate(buf.as_ptr(), &info) }.expect("bảng mẫu phải hợp lệ");
    assert_eq!(st.count, 6);
    assert_eq!(st.n_usable, 3);
    assert_eq!(st.usable_bytes, 0x10 * PAGE_BYTES + 0x30 * PAGE_BYTES);
    assert_eq!(st.n_runtime, 1);
    assert_eq!(st.n_acpi, 1);
    assert_eq!(st.n_reserved, 0);
    assert_eq!(st.highest_phys, 0xFEBF_0000 + PAGE_BYTES);
    // Vùng kề được gộp: 2 vùng (loader code, rồi conventional 0x30 trang).
    assert_eq!(st.n_ram, 2);
    assert_eq!(st.usable_ram()[0].base, 0);
    assert_eq!(st.usable_ram()[0].size, 0x10 * PAGE_BYTES);
    assert_eq!(st.usable_ram()[1].base, 0x0010_0000);
    assert_eq!(st.usable_ram()[1].size, 0x30 * PAGE_BYTES);
    assert_ne!(st.sum, 0);
}

#[test]
fn desc_version_must_be_1() {
    let (buf, mut info) = valid_map();
    info.desc_version = 2;
    assert_eq!(
        unsafe { validate(buf.as_ptr(), &info) }.unwrap_err(),
        UefiError::MapDescVersion
    );
    assert_eq!(expect_err(UefiError::MapDescVersion), "map-desc-version");
}

#[test]
fn desc_size_too_small_or_misaligned_is_named_error() {
    for bad in [0usize, 39, 41, 44] {
        let (buf, mut info) = valid_map();
        info.desc_size = bad;
        assert_eq!(
            unsafe { validate(buf.as_ptr(), &info) }.unwrap_err(),
            UefiError::MapDescSize,
            "desc_size={bad}"
        );
    }
    assert_eq!(UefiError::MapDescSize.name(), "map-desc-size");
}

#[test]
fn map_size_must_be_multiple_of_stride() {
    let (buf, mut info) = valid_map();
    info.map_size += 1;
    assert_eq!(
        unsafe { validate(buf.as_ptr(), &info) }.unwrap_err(),
        UefiError::MapSize
    );
    assert_eq!(UefiError::MapSize.name(), "map-size");
}

#[test]
fn empty_map_is_named_error() {
    let (buf, mut info) = valid_map();
    info.map_size = 0;
    assert_eq!(
        unsafe { validate(buf.as_ptr(), &info) }.unwrap_err(),
        UefiError::MapEmpty
    );
    assert_eq!(UefiError::MapEmpty.name(), "map-empty");
}

#[test]
fn map_bigger_than_buffer_is_truncated_not_read() {
    let (buf, mut info) = valid_map();
    // map_size hợp lệ theo stride nhưng vượt đệm ⇒ từ chối trước khi đọc.
    info.capacity = info.map_size - info.desc_size;
    assert_eq!(
        unsafe { validate(buf.as_ptr(), &info) }.unwrap_err(),
        UefiError::MapTruncated
    );
    assert_eq!(UefiError::MapTruncated.name(), "map-truncated");
    // Quá trần số mô tả cũng bị chặn.
    let mut info2 = valid_map().1;
    info2.capacity = 64 * 1024 * 1024;
    info2.map_size = info2.desc_size * (MAX_DESCRIPTORS + 1);
    assert_eq!(
        unsafe { validate(buf.as_ptr(), &info2) }.unwrap_err(),
        UefiError::MapTruncated
    );
}

#[test]
fn bad_regions_are_rejected() {
    // NumberOfPages == 0.
    let (buf, info) = map_fixture(&[(7, 0x1000, 0, 8)], 0);
    assert_eq!(
        unsafe { validate(buf.as_ptr(), &info) }.unwrap_err(),
        UefiError::MapBadRegion
    );
    // PhysicalStart lệch trang.
    let (buf, info2) = map_fixture(&[(7, 0x1001, 1, 8)], 0);
    assert_eq!(
        unsafe { validate(buf.as_ptr(), &info2) }.unwrap_err(),
        UefiError::MapBadRegion
    );
    // Tràn địa chỉ (u64::MAX trang).
    let (buf, info3) = map_fixture(&[(7, 0x1000, u64::MAX, 8)], 0);
    assert_eq!(
        unsafe { validate(buf.as_ptr(), &info3) }.unwrap_err(),
        UefiError::MapBadRegion
    );
    assert_eq!(UefiError::MapBadRegion.name(), "map-bad-region");
}

#[test]
fn bad_type_outside_uefi_range_rejected() {
    for bad in [0x11u32, 0xFFFF_FFFF] {
        let (buf, info) = map_fixture(&[(bad, 0x1000, 1, 8)], 0);
        assert_eq!(
            unsafe { validate(buf.as_ptr(), &info) }.unwrap_err(),
            UefiError::MapBadType
        );
    }
    assert_eq!(UefiError::MapBadType.name(), "map-bad-type");
}

#[test]
fn bad_attribute_zero_cache_bits_rejected_only_for_usable() {
    // `Attribute = 0` trên vùng usable ⇒ lỗi có tên.
    let (buf, info) = map_fixture(&[(7, 0x1000, 1, 0)], 0);
    assert_eq!(
        unsafe { validate(buf.as_ptr(), &info) }.unwrap_err(),
        UefiError::MapBadAttribute
    );
    assert_eq!(UefiError::MapBadAttribute.name(), "map-bad-attribute");

    // OVMF thật trả 0xF cho mọi mô tả ⇒ **phải chấp nhận** (nếu không là false-red).
    let (buf, info) = map_fixture(&[(7, 0x1000, 1, 0xF)], 0);
    let st = unsafe { validate(buf.as_ptr(), &info) }.expect("0xF phải hợp lệ (OVMF đo được)");
    assert_eq!(st.n_attr_odd, 1, "0xF không đúng-một-bit ⇒ chỉ đếm, không chặn");

    // Vùng **không** usable với attr 0 (MMIO/reserved) được chấp nhận (chỉ đếm).
    let (buf, info) = map_fixture(&[(11, 0xFEBF_0000, 1, 0)], 0);
    let st = unsafe { validate(buf.as_ptr(), &info) }.expect("MMIO attr=0 phải hợp lệ");
    assert_eq!(st.n_attr_odd, 1);
}

#[test]
fn too_many_usable_regions_is_named_error() {
    // 17 vùng conventional không kề nhau ⇒ vượt MAX_USABLE_RAM = 16.
    let mut descs = Vec::new();
    for i in 0..(MAX_USABLE_RAM + 1) {
        descs.push((7u32, 0x10_0000u64 * (i as u64 + 1), 1u64, 8u64));
    }
    let (buf, info) = map_fixture(&descs, 0);
    assert_eq!(
        unsafe { validate(buf.as_ptr(), &info) }.unwrap_err(),
        UefiError::TooManyRegions
    );
    assert_eq!(UefiError::TooManyRegions.name(), "too-many-regions");
}

#[test]
fn injected_faults_map_to_expected_named_errors() {
    // Cùng bộ tiêm lỗi mà `run.sh --selftest` dùng trên QEMU: mỗi tên ⇒ một lỗi có tên.
    let cases = [
        ("map-empty", UefiError::MapEmpty),
        ("desc-version", UefiError::MapDescVersion),
        ("desc-size", UefiError::MapDescSize),
        ("map-size", UefiError::MapSize),
        ("bad-type", UefiError::MapBadType),
        ("bad-page", UefiError::MapBadRegion),
        ("bad-attr", UefiError::MapBadAttribute),
    ];
    for (fault, want) in cases {
        let (mut buf, mut info) = valid_map();
        // SAFETY: buffer đủ `capacity`; `info` khớp bảng vừa dựng.
        let injected = unsafe { apply_fault(buf.as_mut_ptr(), &mut info, fault) };
        assert!(injected, "fault {fault} phải tiêm được");
        // SAFETY: như trên.
        let got = unsafe { validate(buf.as_ptr(), &info) }.unwrap_err();
        assert_eq!(got, want, "fault {fault}");
        assert_eq!(got.name(), want.name());
    }
}

// ---------------------------------------------------------------- bản ghi bàn giao

/// `core::fmt::Write` tối thiểu để test `BootInfo::write_log` (test host có `std`).
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

fn sample_handoff() -> UefiHandoffV1 {
    let st = {
        let (buf, info) = valid_map();
        unsafe { validate(buf.as_ptr(), &info) }.unwrap()
    };
    let info = valid_map().1;
    let mut rec = UefiHandoffV1::build(&st, &info, 0x7f00_0000, 0);
    rec.seal();
    rec
}

#[test]
fn handoff_roundtrip_and_layout() {
    let rec = sample_handoff();
    assert_eq!(rec.magic, HANDOFF_MAGIC);
    assert_eq!(HANDOFF_BYTES, size_of::<UefiHandoffV1>());
    assert_eq!(rec.verify(), Ok(()));
    assert_eq!(rec.n_ram, 2);
    assert_eq!(rec.ram[0], HandoffRam { base: 0, size: 0x10 * PAGE_BYTES });
    // **D2-260** — `rsdp_pa`/`dtb_pa` (0 = firmware không khai) nằm trong bản ghi.
    assert_eq!(rec.rsdp_pa, 0x7f00_0000);
    assert_eq!(rec.dtb_pa, 0);
    assert_eq!(rec.source, vifast_boot_uefi::handoff::SOURCE_UEFI_MEMMAP);
    assert_eq!(rec.arch, vifast_boot_uefi::handoff::ARCH_X86_64);
    // `repr(C)`: kernel đọc được bằng offset, không phụ thuộc layout Rust.
    assert_eq!(offset_of!(UefiHandoffV1, magic), 0);
    assert_eq!(offset_of!(UefiHandoffV1, rsdp_pa), 80);
    assert_eq!(offset_of!(UefiHandoffV1, dtb_pa), 88);
    assert_eq!(offset_of!(UefiHandoffV1, ram), 96);
    assert_eq!(HANDOFF_BYTES, 352);
    // **Bất biến trang bàn giao**: `reserved_pages` tính đúng số trang 4 KiB cần giữ.
    assert_eq!(
        rec.reserved_pages(0x1000),
        1,
        "352 B tại địa chỉ căn trang ⇒ 1 trang"
    );
    assert_eq!(
        rec.reserved_pages(0x1000 + 0xF00),
        2,
        "352 B vắt qua ranh giới trang ⇒ 2 trang"
    );
    assert_eq!(HANDOFF_RESERVED_INVARIANT, "kernel-must-reserve-or-copy");
}

#[test]
fn handoff_rejects_tampering_with_name() {
    let mut rec = sample_handoff();
    rec.checksum ^= 1;
    assert_eq!(rec.verify(), Err("bad-checksum"));

    let mut rec = sample_handoff();
    rec.magic ^= 1;
    assert_eq!(rec.verify(), Err("bad-magic"));

    let mut rec = sample_handoff();
    rec.version = 2;
    assert_eq!(rec.verify(), Err("unsupported-version"));

    let mut rec = sample_handoff();
    rec.ram[0].size += 1;
    assert_eq!(rec.verify(), Err("bad-checksum"), "sửa payload phải bị checksum bắt");

    let mut rec = sample_handoff();
    rec.size_bytes = 1;
    assert_eq!(rec.verify(), Err("bad-size"));

    // **D2-260** — fail-closed theo nguồn: bản ghi `Uefi` không có RAM / vùng RAM hỏng / sai
    // arch/source / RSDP-DTB lệch căn đều bị từ chối **trước** khi kernel đọc.
    let mut rec = sample_handoff();
    rec.n_ram = 0;
    rec.seal();
    assert_eq!(rec.verify(), Err("missing-memory"));

    let mut rec = sample_handoff();
    rec.ram[0].size = 0;
    rec.seal();
    assert_eq!(rec.verify(), Err("bad-region"));

    let mut rec = sample_handoff();
    rec.ram[0].base = u64::MAX - 0x10;
    rec.ram[0].size = 0x1000;
    rec.seal();
    assert_eq!(rec.verify(), Err("bad-region"), "tràn địa chỉ phải bị bắt");

    let mut rec = sample_handoff();
    rec.arch = 1;
    rec.seal();
    assert_eq!(rec.verify(), Err("bad-arch"));

    let mut rec = sample_handoff();
    rec.source = 1;
    rec.seal();
    assert_eq!(rec.verify(), Err("bad-source"));

    let mut rec = sample_handoff();
    rec.rsdp_pa = 0x7f00_0008; // không căn 16
    rec.seal();
    assert_eq!(rec.verify(), Err("bad-rsdp"));

    let mut rec = sample_handoff();
    rec.dtb_pa = 0x7f00_0004; // không căn 8
    rec.seal();
    assert_eq!(rec.verify(), Err("bad-dtb"));
}
