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

// =====================================================================================
// KT-273 (K4' bước 3, PCI ECAM + virtio-blk) — ba vùng PCIe của QEMU `virt` aarch64
// =====================================================================================
//
// **Nguồn số: DTB của chính QEMU 11.1.3 trên máy đo**, không viết tay từ trí nhớ:
//
// ```text
// qemu-system-aarch64 -M virt,gic-version=3,dumpdtb=/tmp/virt.dtb -cpu cortex-a72 -m 128M
// dtc -I dtb -O dts /tmp/virt.dtb | sed -n '/pcie@10000000/,/^};/p'
//   reg    = <0x40 0x10000000 0x00 0x10000000>      ← ECAM: 0x40_1000_0000, 256 MiB
//   ranges = <0x1000000 0x00 0x00 0x00 0x3eff0000 0x00 0x10000        ← PIO 0x3eff_0000, 64 KiB
//             0x2000000 0x00 0x10000000 0x00 0x10000000 0x00 0x2eff0000 ← MMIO32 0x1000_0000
//             0x3000000 0x80 0x00 0x80 0x00 0x80 0x00>                 ← MMIO64 0x80_0000_0000
// ```
//
// **ECAM KHÔNG ở `0x3f00_0000`** như bản QEMU cũ / như riscv64: QEMU `virt` aarch64 bật
// `highmem` mặc định nên đặt ECAM ở **`0x40_1000_0000`** (256 GiB + 16 MiB) và cửa sổ MMIO
// 64-bit ở `0x80_0000_0000`. Đo được bằng `dumpdtb` + `dtc` (lệnh trên), không suy đoán.
//
// Hệ quả **quan trọng** cho MMU: PA `0x40_1000_0000` có bit 38 = 1 nhưng bit 63..39 = 0 ⇒
// **không canonical** với 39-bit VA của ảnh (D2-147), nên ECAM **không** identity-map được —
// driver đọc nó qua cửa sổ **physmap** (`PHYSMAP_BASE + PA`). Đây là lý do kỹ thuật, không phải
// lựa chọn phong cách.

/// PCIe ECAM của QEMU `virt` aarch64 — **256 MiB** (256 bus × 1 MiB), ở `0x40_1000_0000`.
///
/// Driver **không** map hết 256 MiB (xem `pci::ECAM_MAP_BYTES`): mỗi 2 MiB cần một bảng trang
/// mức 3, 256 MiB cần 129 frame — vượt arena boot **64** frame (KT-338a) và trần `.data+.bss` 256 KiB (I7).
pub const PCI_ECAM: Region = Region {
    name: "pci-ecam",
    base: 0x40_1000_0000,
    size: 0x1000_0000,
};

/// Cửa sổ MMIO 32-bit của PCIe (`ranges` entry 2 của DTB) — nơi driver **gán BAR**.
pub const PCI_MMIO32: Region = Region {
    name: "pci-mmio32",
    base: 0x1000_0000,
    size: 0x2eff_0000,
};

/// Cửa sổ I/O port (legacy) của PCIe (`ranges` entry 1) — driver **không** dùng (ECAM + MMIO
/// là đường PCIe), khai để phép kiểm chồng lấn biết vùng này **không** được gán BAR memory vào.
pub const PCI_PIO: Region = Region {
    name: "pci-pio",
    base: 0x3eff_0000,
    size: 0x1_0000,
};

/// GIC **redistributor** của GICv3 — `0x080A_0000 + hart × 0x2_0000`, một dải cho `MAX_HARTS`
/// hart (1 MiB).
///
/// **Vì sao có mục này** (KT-273): KT-259 phải **map thêm** vùng này vào `VSpace` sau khi
/// `activate` (nếu không, lần đọc `GICR_WAKER` đầu tiên fault — đo được `ec=0x25`), nhưng bảng
/// `REGIONS` chưa có nó ⇒ phép kiểm chồng lấn **không** biết vùng này tồn tại. Thêm vào bảng để
/// "một nguồn" đúng nghĩa; giá trị lấy từ [`crate::gic`]/[`crate::smp`], không viết lại số.
pub const GICR: Region = Region {
    name: "gic-redist",
    base: crate::gic::GICR_BASE,
    size: crate::smp::MAX_HARTS as u64 * crate::gic::GICR_STRIDE,
};

/// **Một nguồn** cho mọi vùng MMIO nền tảng của aarch64.
pub const REGIONS: &[Region] = &[
    UART0, GIC, GIC_CPU, GICR, RAM, PCI_ECAM, PCI_MMIO32, PCI_PIO,
];

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
