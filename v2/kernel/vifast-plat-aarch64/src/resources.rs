// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Bảng tài nguyên QEMU `virt` (aarch64)** — KT-247 (K4' bước 3, B3.1).
//!
//! Cùng vai trò với [`vifast_plat_common::resources`] bên riscv64 (KT-81): **một** nguồn cho các
//! vùng MMIO nền tảng + phép kiểm **chồng lấn**. Bảng bên `vifast-plat-common` là của QEMU
//! `virt` **riscv64** (UART 16550 tại `0x1000_0000`, PLIC `0x0c00_0000`) — **không** dùng lại
//! được cho aarch64, nên bảng aarch64 nằm ở đây chứ không sửa bảng dùng chung (D2-17: phần arch
//! ở crate arch).
//!
//! # Nguồn số (không viết tay từ trí nhớ)
//!
//! Memory map của `qemu-system-aarch64 -M virt` (`hw/arm/virt.c`, `memmap[]`):
//!
//! | Vùng | Base | Size |
//! |---|---|---|
//! | PL011 UART0 | `0x0900_0000` | `0x1000` |
//! | GIC distributor | `0x0800_0000` | `0x1_0000` |
//! | GIC CPU interface | `0x0801_0000` | `0x1_0000` |
//! | RAM | `0x4000_0000` | (theo `-m`) |
//!
//! # Nợ đã biết (R4 của thiết kế `kt242-k4-buoc3-design.md` §5)
//!
//! Thiết kế đề nghị lấy địa chỉ **từ DTB** thay vì viết tay. Bước 3 **chưa** đọc DTB: `_start`
//! chỉ **kiểm magic** của DTB mà QEMU truyền trong `x0` (in `# dtb: magic=0x…`) để chứng minh
//! con trỏ là thật, còn địa chỉ UART/GIC vẫn là hằng của bảng này. Đọc DTB đầy đủ (FDT parser)
//! là việc của bước sau — ghi rõ ở `v2/agent/JOURNAL/kt247-aarch64-boot.md` §"Còn thiếu".

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

/// PL011 UART0 của QEMU `virt` aarch64 — một trang 4 KiB.
pub const UART0: Region = Region {
    name: "uart0",
    base: 0x0900_0000,
    size: 0x1000,
};

/// GIC distributor (GICv3 `GICD`) của QEMU `virt` aarch64 — 64 KiB.
pub const GIC: Region = Region {
    name: "gic-dist",
    base: 0x0800_0000,
    size: 0x1_0000,
};

/// GIC CPU interface (GICv3 `GICC`/system register interface) — 64 KiB.
pub const GIC_CPU: Region = Region {
    name: "gic-cpu",
    base: 0x0801_0000,
    size: 0x1_0000,
};

/// RAM của QEMU `virt` aarch64 bắt đầu ở `0x4000_0000`; kích thước do `-m` quyết định nên trường
/// `size` ở đây chỉ là **cửa sổ tối thiểu** mà bước 3 giả định (128 MiB, đúng `-m 128M` của
/// runner). Không dùng để giới hạn mapping — mapping lấy từ symbol linker.
pub const RAM: Region = Region {
    name: "ram",
    base: 0x4000_0000,
    size: 0x0800_0000,
};

/// **Một nguồn** cho mọi vùng MMIO nền tảng của aarch64.
pub const REGIONS: &[Region] = &[UART0, GIC, GIC_CPU, RAM];

/// `true` ⇔ hai vùng **chồng lấn** (giao khác rỗng). Khoảng rỗng/kích thước 0 ⇒ không chồng.
pub const fn overlaps(a: Region, b: Region) -> bool {
    if a.size == 0 || b.size == 0 {
        return false;
    }
    a.base < b.end() && b.base < a.end()
}

/// Cặp vùng **đầu tiên** chồng lấn trong [`REGIONS`], `None` nếu bảng sạch.
///
/// Trả **cặp** chứ không trả `bool`: khi đỏ, người đọc cần biết **hai vùng nào** đè nhau.
pub fn first_overlap() -> Option<(Region, Region)> {
    let mut i = 0;
    while i < REGIONS.len() {
        let mut j = i + 1;
        while j < REGIONS.len() {
            if overlaps(REGIONS[i], REGIONS[j]) {
                return Some((REGIONS[i], REGIONS[j]));
            }
            j += 1;
        }
        i += 1;
    }
    None
}
