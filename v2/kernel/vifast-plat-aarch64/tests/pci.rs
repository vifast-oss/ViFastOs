// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho `vifast_plat_aarch64::pci` — KT-273 (K4' bước 3, PCI ECAM + virtio-blk).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64 --test pci
//! ```
//!
//! Phần **thuần** của driver PCI được chốt ở đây trước khi chạm phần cứng (D2-05):
//!
//! 1. **Địa chỉ ECAM**: `bus<<20 | dev<<15 | func<<12 | reg` — công thức của PCIe ECAM, kiểm
//!    bằng số tính tay cho cả ba trường và cho bus biên (`0`, `SCAN_BUSES-1`).
//! 2. **ECAM không identity-map được**: PA `0x40_1000_0000` có bit 38 = 1 nhưng bit 63..39 = 0 ⇒
//!    **không canonical** với 39-bit VA; VA physmap của nó thì canonical. Đây là lý do driver
//!    đọc ECAM qua cửa sổ physmap chứ không identity (khác riscv64).
//! 3. **Cửa sổ quét suy từ cửa sổ ECAM đã map** (`SCAN_BUSES = ECAM_MAP_BYTES >> 20`), không
//!    viết tay 256; và cửa sổ map không vượt vùng `PCI_ECAM` của bảng tài nguyên.
//! 4. **BAR gán trong cửa sổ MMIO 32-bit** và **không chồng** vùng nền tảng nào
//!    (`RESERVED_RANGES`) — kiểm bằng `first_conflict` của `vifast-virtio` (cùng hàm driver dùng).
//! 5. **Bảng tài nguyên aarch64** có PCI ECAM/MMIO/PIO và **không** cặp nào chồng lấn.
//! 6. **Tên loại BAR** in từ dữ liệu (D2-102), không viết chuỗi trùng lặp trong test.

// Test này **ghim hằng số** của driver (địa chỉ ECAM, cửa sổ BAR, bố cục trang DMA) —
// khẳng định trên hằng là mục đích, không phải sơ suất (cùng khuôn `vifast-plat-x86_64/tests`).
#![allow(clippy::assertions_on_constants)]

use vifast_plat_aarch64::format::Aarch64;
use vifast_plat_aarch64::pci::{
    bar_assign_base, bar_kind_name, ecam_pa, ecam_va, Ecam, BAR_ASSIGN_BASE, BAR_ASSIGN_BYTES,
    ECAM_BASE, ECAM_MAP_BYTES, ECAM_VA_BASE, PCI_MMIO_WINDOW_BASE, PCI_MMIO_WINDOW_BYTES,
    RESERVED_RANGES, SCAN_BUSES, SCAN_DEVICES, SCAN_FUNCTIONS,
};
use vifast_plat_aarch64::resources::{self, PCI_ECAM, PCI_MMIO32, PCI_PIO};
use vifast_plat_aarch64::split_mmu::PHYSMAP_BASE;
use vifast_virtio::pci::{first_conflict, BarKind, VirtioKind, CONFLICT_RESERVED};

/// `true` ⇔ hai khoảng `[a, a+alen)` và `[b, b+blen)` chồng nhau (khoảng rỗng ⇒ không).
fn overlaps(a: u64, alen: u64, b: u64, blen: u64) -> bool {
    alen != 0 && blen != 0 && a < b.saturating_add(blen) && b < a.saturating_add(alen)
}

// ---------------------------------------------------------------- 1. địa chỉ ECAM

#[test]
fn ecam_address_is_bus_dev_func_reg() {
    // Số tính tay: bus 0, dev 0, func 0, reg 0 ⇒ đúng base.
    assert_eq!(ecam_pa(0, 0, 0, 0), ECAM_BASE);
    // bus 1 ⇒ +1 MiB; dev 1 ⇒ +32 KiB; func 1 ⇒ +4 KiB; reg 0x10 ⇒ +16.
    assert_eq!(ecam_pa(1, 0, 0, 0), ECAM_BASE + 0x10_0000);
    assert_eq!(ecam_pa(0, 1, 0, 0), ECAM_BASE + 0x8000);
    assert_eq!(ecam_pa(0, 0, 1, 0), ECAM_BASE + 0x1000);
    assert_eq!(ecam_pa(0, 0, 0, 0x10), ECAM_BASE + 0x10);
    // Tổ hợp: bus 2, dev 3, func 4, reg 0x34.
    assert_eq!(
        ecam_pa(2, 3, 4, 0x34),
        ECAM_BASE + 2 * 0x10_0000 + 3 * 0x8000 + 4 * 0x1000 + 0x34
    );
    // Bus cuối của cửa sổ quét vẫn nằm trong vùng ECAM khai ở bảng tài nguyên.
    let last = ecam_pa(SCAN_BUSES - 1, SCAN_DEVICES - 1, SCAN_FUNCTIONS - 1, 0xff);
    assert!(last < PCI_ECAM.end(), "ECAM cuối 0x{last:x} vượt vùng khai");
}

#[test]
fn ecam_va_is_dedicated_window_and_canonical() {
    // PA của ECAM **không** canonical với 39-bit VA (bit 38 = 1, bit 63..39 = 0) ⇒ không
    // identity-map được.
    assert!(!Aarch64::is_canonical(ECAM_BASE));
    // Cửa sổ **physmap** cũng không dùng được: `PHYSMAP_BASE + PA` **tràn u64** (nửa cao 39-bit
    // chỉ phủ PA < 2^38 = 256 GiB, ECAM ở 256 GiB + 16 MiB). Đây là lý do có cửa sổ VA riêng.
    assert!(PHYSMAP_BASE.checked_add(ECAM_BASE).is_none());
    // Cửa sổ riêng: VA = ECAM_VA_BASE + (PA - ECAM_BASE), canonical, và nằm trọn nửa thấp.
    let va = ecam_va(0, 0, 0, 0);
    assert_eq!(va, ECAM_VA_BASE);
    assert!(Aarch64::is_canonical(va));
    assert!(ECAM_VA_BASE + ECAM_MAP_BYTES <= (1u64 << 38));
    // Bus cuối vẫn canonical và vẫn trong cửa sổ đã map.
    let last = ecam_va(SCAN_BUSES - 1, 31, 7, 0xff);
    assert!(Aarch64::is_canonical(last));
    assert!(last < ECAM_VA_BASE + ECAM_MAP_BYTES);
    // Cửa sổ ECAM **không** đè các VA đã dùng của ảnh (probe/VSpace/EL0 đều < 0x8000_0000).
    assert!(ECAM_VA_BASE >= 0x8000_0000);
}

#[test]
fn ecam_struct_addr_matches_free_function() {
    let ecam = Ecam {
        bus: 3,
        dev: 5,
        func: 2,
    };
    // `Ecam::addr` trả **VA** (cửa sổ ECAM riêng), không phải PA — MMIO đọc qua VA.
    assert_eq!(ecam.addr(0x08), ecam_va(3, 5, 2, 0x08));
    assert_eq!(ecam.addr(0x00), ecam_va(3, 5, 2, 0x00));
    // Và VA ấy suy từ PA bằng đúng một phép dịch cửa sổ.
    assert_eq!(
        ecam.addr(0x10) - ECAM_VA_BASE,
        ecam_pa(3, 5, 2, 0x10) - ECAM_BASE
    );
}

// ---------------------------------------------------------------- 2. cửa sổ quét

#[test]
fn scan_window_is_derived_from_mapped_ecam() {
    // Một bus = 1 MiB (PCIe ECAM); số bus quét **suy** từ số byte đã map, không viết tay.
    assert_eq!(SCAN_BUSES as u64, ECAM_MAP_BYTES >> 20);
    assert!(SCAN_BUSES >= 1);
    // Cửa sổ map không được vượt vùng ECAM khai ở bảng tài nguyên (nếu vượt, driver đọc ra
    // ngoài vùng đã khai — đúng lớp lỗi KT-81 bên riscv64).
    assert!(ECAM_MAP_BYTES <= PCI_ECAM.size);
    assert_eq!(ECAM_BASE, PCI_ECAM.base);
    // Dải quét đầy đủ của PCI là 32 device × 8 function; driver quét đúng dải ấy.
    assert_eq!(SCAN_DEVICES, 32);
    assert_eq!(SCAN_FUNCTIONS, 8);
}

// ---------------------------------------------------------------- 3. BAR

#[test]
fn bar_window_is_inside_mmio_and_clear_of_reserved() {
    // BAR gán ở đầu cửa sổ MMIO 32-bit của QEMU virt (0x1000_0000) — cùng khuôn riscv64.
    assert_eq!(BAR_ASSIGN_BASE, PCI_MMIO_WINDOW_BASE);
    assert!(BAR_ASSIGN_BYTES > 0);
    assert!(BAR_ASSIGN_BASE + BAR_ASSIGN_BYTES <= PCI_MMIO_WINDOW_BASE + PCI_MMIO_WINDOW_BYTES);
    // Cửa sổ BAR **không** chồng vùng nền tảng nào — dùng **đúng** hàm driver dùng
    // (`first_conflict`), nên test và driver không thể lệch nhau về định nghĩa "chồng lấn".
    assert_eq!(
        first_conflict(BAR_ASSIGN_BASE, BAR_ASSIGN_BYTES, &[], &RESERVED_RANGES),
        None
    );
    // Đối chứng dương: gán vào RAM thì hàm **phải** báo chồng (nếu không, phép kiểm là dấu cao su).
    assert_eq!(
        first_conflict(resources::RAM.base, 0x1000, &[], &RESERVED_RANGES),
        Some(CONFLICT_RESERVED)
    );
    // Và gán vào chính ECAM cũng phải bị bắt.
    assert_eq!(
        first_conflict(ECAM_BASE, 0x1000, &[], &RESERVED_RANGES),
        Some(CONFLICT_RESERVED)
    );
}

#[test]
fn bar_assign_base_per_kind_is_distinct_and_inside_window() {
    // Mỗi loại virtio một leaf 2 MiB riêng (khuôn riscv64 `BAR_ASSIGN_BASE_*`): hai driver cùng
    // có mặt không tranh một trang.
    let kinds = [
        VirtioKind::Blk,
        VirtioKind::Console,
        VirtioKind::Net,
        VirtioKind::Input,
        VirtioKind::Gpu,
    ];
    let mut i = 0;
    while i < kinds.len() {
        let base = bar_assign_base(kinds[i]);
        assert!(base >= PCI_MMIO_WINDOW_BASE);
        assert!(base + BAR_ASSIGN_BYTES <= PCI_MMIO_WINDOW_BASE + PCI_MMIO_WINDOW_BYTES);
        assert_eq!(base % BAR_ASSIGN_BYTES, 0, "leaf phải căn theo cỡ leaf");
        let mut j = i + 1;
        while j < kinds.len() {
            assert_ne!(base, bar_assign_base(kinds[j]), "hai loại trùng leaf");
            j += 1;
        }
        i += 1;
    }
    // blk ở leaf 0 — ảnh KT-273 chỉ dùng blk nên địa chỉ phải là base của cửa sổ.
    assert_eq!(bar_assign_base(VirtioKind::Blk), BAR_ASSIGN_BASE);
}

#[test]
fn bar_kind_names_are_data_driven() {
    assert_eq!(bar_kind_name(BarKind::Io), "io");
    assert_eq!(bar_kind_name(BarKind::Memory32), "mem32");
    assert_eq!(bar_kind_name(BarKind::Memory64), "mem64");
}

// ---------------------------------------------------------------- 4. bảng tài nguyên

#[test]
fn resources_have_pci_regions_and_no_overlap() {
    // Ba vùng PCIe của QEMU virt aarch64 (đo từ DTB `-M virt,dumpdtb`): ECAM 256 MiB ở
    // 0x40_1000_0000, MMIO 32-bit 0x1000_0000, PIO 0x3eff_0000.
    assert_eq!(PCI_ECAM.base, 0x40_1000_0000);
    assert_eq!(PCI_ECAM.size, 0x1000_0000);
    assert_eq!(PCI_MMIO32.base, 0x1000_0000);
    assert_eq!(PCI_MMIO32.size, 0x2eff_0000);
    assert_eq!(PCI_PIO.base, 0x3eff_0000);
    assert_eq!(PCI_PIO.size, 0x1_0000);
    // Bảng `REGIONS` phải chứa cả ba và **không** cặp nào chồng lấn.
    for region in [PCI_ECAM, PCI_MMIO32, PCI_PIO] {
        assert!(
            resources::REGIONS.iter().any(|r| r.name == region.name),
            "REGIONS thiếu {}",
            region.name
        );
    }
    assert_eq!(resources::first_overlap(), None);
    // ECAM không được đè RAM (RAM `-m 128M` bắt đầu 0x4000_0000 — thấp hơn ECAM rất nhiều).
    assert!(!overlaps(
        PCI_ECAM.base,
        PCI_ECAM.size,
        resources::RAM.base,
        resources::RAM.size
    ));
}

#[test]
fn reserved_ranges_are_pairwise_disjoint() {
    let mut i = 0;
    while i < RESERVED_RANGES.len() {
        let (a, alen) = RESERVED_RANGES[i];
        assert!(alen > 0, "vùng dự trữ {i} rỗng");
        let mut j = i + 1;
        while j < RESERVED_RANGES.len() {
            let (b, blen) = RESERVED_RANGES[j];
            assert!(
                !overlaps(a, alen, b, blen),
                "vùng dự trữ {i} (0x{a:x}+0x{alen:x}) chồng vùng {j} (0x{b:x}+0x{blen:x})"
            );
            j += 1;
        }
        i += 1;
    }
}
