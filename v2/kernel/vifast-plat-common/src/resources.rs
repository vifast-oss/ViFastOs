// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-81** — bảng tài nguyên nền tảng: **một nguồn** cho các vùng MMIO, và phép kiểm **chồng lấn**.
//!
//! # Vì sao cần
//!
//! Trước KT-81, địa chỉ UART/PLIC/CLINT/trang test/ECAM nằm rải ở nhiều tệp dưới dạng hằng số viết
//! tay. Hai hậu quả, cả hai đều **im lặng**:
//!
//! 1. **Hai nguồn**: một chỗ sửa, chỗ khác không ⇒ MMU map một địa chỉ, driver đọc một địa chỉ
//!    khác. Không có lỗi biên dịch, không có lỗi lúc chạy — chỉ có dữ liệu sai.
//! 2. **Chồng lấn**: hai vùng gán đè nhau (ví dụ ECAM và một BAR) ⇒ ghi vào vùng này xoá vùng kia.
//!    Không ai kiểm vì mỗi hằng số được khai riêng lẻ.
//!
//! Tệp này gom các vùng ấy vào **một** bảng [`REGIONS`], và [`first_overlap`] kiểm chồng lấn trên
//! chính bảng đó. Các hằng số cũ (`UART0_BASE`, `PLIC_BASE`, …) nay **dẫn xuất** từ bảng, nên
//! không còn đường thứ hai để lệch.
//!
//! # Phạm vi
//!
//! Chỉ **vùng MMIO nền tảng** mà kernel map/đọc trực tiếp. RAM, stack, vùng NOLOAD do linker script
//! quyết định — không nằm ở đây (bảng này không thay linker script).

/// Một vùng tài nguyên: tên (để in log), địa chỉ bắt đầu, kích thước (byte).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    /// Tên **chữ** để in vào log (D2-102: không in số trần trong evidence).
    pub name: &'static str,
    /// Địa chỉ vật lý bắt đầu.
    pub base: u64,
    /// Kích thước vùng (byte); phải > 0.
    pub size: u64,
}

impl Region {
    /// Địa chỉ **kết thúc không bao gồm** (`base + size`), bão hoà khi tràn.
    pub const fn end(&self) -> u64 {
        self.base.saturating_add(self.size)
    }

    /// `true` ⇔ vùng này hợp lệ (kích thước > 0 và không tràn địa chỉ).
    pub const fn is_valid(&self) -> bool {
        self.size > 0 && self.base.checked_add(self.size).is_some()
    }
}

/// UART0 (16550A) của QEMU `virt` — 4 KiB (một trang).
pub const UART0: Region = Region {
    name: "uart0",
    base: 0x1000_0000,
    size: 0x1000,
};
/// PLIC cổ điển của `virt` — cửa sổ ưu tiên + cửa sổ ngắt (64 MiB, đúng như MMU map).
pub const PLIC: Region = Region {
    name: "plic",
    base: 0x0c00_0000,
    size: 0x0400_0000,
};
/// CLINT/ACLINT của `virt` (`aclint=on`) — 64 KiB.
pub const CLINT: Region = Region {
    name: "clint",
    base: 0x0200_0000,
    size: 0x1_0000,
};
/// Trang test của `virt` (`test@100000`) — 4 KiB.
pub const TEST_PAGE: Region = Region {
    name: "test-page",
    base: 0x0010_0000,
    size: 0x1000,
};
/// ECAM PCIe của `virt` (`pci@30000000`) — 256 bus × 32 dev × 8 fn × 4 KiB = 256 MiB.
pub const PCI_ECAM: Region = Region {
    name: "pci-ecam",
    base: 0x3000_0000,
    size: 0x1000_0000,
};
/// Cửa sổ MMIO 32-bit của PCIe (`pci@30000000` ranges) — nơi BAR được cấp.
pub const PCI_MMIO32: Region = Region {
    name: "pci-mmio32",
    base: 0x4000_0000,
    size: 0x4000_0000,
};

/// **Một nguồn** cho mọi vùng MMIO nền tảng. Thêm vùng mới ⇒ thêm ở đây, không khai hằng rời.
pub const REGIONS: &[Region] = &[UART0, PLIC, CLINT, TEST_PAGE, PCI_ECAM, PCI_MMIO32];

/// `true` ⇔ hai vùng **chồng lấn** (giao khác rỗng). Khoảng rỗng/kích thước 0 ⇒ không chồng.
pub const fn overlaps(a: Region, b: Region) -> bool {
    if a.size == 0 || b.size == 0 {
        return false;
    }
    a.base < b.end() && b.base < a.end()
}

/// Cặp vùng **đầu tiên** chồng lấn trong `regions`, `None` nếu bảng sạch.
///
/// Trả **cặp** chứ không trả `bool`: khi đỏ, người đọc cần biết **hai vùng nào** đè nhau, không
/// chỉ biết "có chồng lấn".
pub fn first_overlap(regions: &[Region]) -> Option<(Region, Region)> {
    let mut i = 0;
    while i < regions.len() {
        let mut j = i + 1;
        while j < regions.len() {
            if overlaps(regions[i], regions[j]) {
                return Some((regions[i], regions[j]));
            }
            j += 1;
        }
        i += 1;
    }
    None
}

/// Vùng **đầu tiên** không hợp lệ (kích thước 0 hoặc tràn địa chỉ), `None` nếu mọi vùng hợp lệ.
pub fn first_invalid(regions: &[Region]) -> Option<Region> {
    let mut i = 0;
    while i < regions.len() {
        if !regions[i].is_valid() {
            return Some(regions[i]);
        }
        i += 1;
    }
    None
}
