// SPDX-License-Identifier: MIT OR Apache-2.0
//! Dữ liệu **đường boot x86_64** — KT-257 (K5' bước 1): bảng trang long mode, GDT tối thiểu,
//! header multiboot2 và note PVH.
//!
//! Tệp này **thuần dữ liệu** (không asm, không `unsafe`): asm của ảnh `vifast-kbin-x86_64` lấy
//! từng hằng ở đây làm `const` operand của `global_asm!`, nên test host
//! (`tests/boot.rs`) chốt **đúng giá trị asm nạp** — không có bản thứ hai của con số.
//!
//! # Đường boot thật trên QEMU 11.1.3
//!
//! QEMU `-kernel` cho x86 **không** dùng multiboot2: `hw/i386/multiboot.c` chỉ tìm magic
//! multiboot **1** (`0x1BADB002`) và, nếu tìm thấy, **từ chối** ảnh ELF64
//! (`"Cannot load x86-64 image, give a 32bit one."`). Đường QEMU thật sự nhận ELF64 là **PVH**
//! (`hw/i386/x86-common.c:load_elfboot`): nó đọc note `"Xen"` type
//! [`PVH_NOTE_TYPE`] trong một `PT_NOTE` segment, lấy **địa chỉ entry 32-bit** từ desc rồi nhảy
//! vào đó ở **32-bit protected mode** với `%ebx` = `hvm_start_info`.
//!
//! Vì vậy ảnh mang **cả hai** header:
//!
//! - **multiboot2** ([`MULTIBOOT2_MAGIC`] …) — đúng đặc tả, để GRUB/multiboot2 cũng nạp được
//!   (GRUB nhảy vào `_start` ở 32-bit protected mode, `eax = 0x36D76289`);
//! - **PVH note** ([`PVH_NOTE_TYPE`]) — đường `qemu-system-x86_64 -kernel <ELF>` thực tế dùng.
//!
//! `_start` **không** phụ thuộc `eax`/`ebx` của giao thức nào: nó chỉ ghi lại hai thanh ghi ấy để
//! `kmain` in bằng chứng, rồi tự dựng stack/bảng trang/GDT.
//!
//! # Nguồn
//!
//! - **Intel SDM Vol 3A** ch. 4 "Paging" — PML4E/PDPTE/PDE 2 MiB (`PS = 1`), bit `P/RW/PS`.
//! - **Intel SDM Vol 3A** §9.8.5 "Initializing IA-32e Mode" — thứ tự `CR4.PAE` → `CR3` →
//!   `IA32_EFER.LME` → `CR0.PG|PE` → `ljmp` vào đoạn code 64-bit.
//! - **Intel SDM Vol 3A** §3.4.5 "Segment Descriptors" — GDT: code 64-bit `L = 1`, `D/B = 0`,
//!   access `0x9A`; data access `0x92`.
//! - **Multiboot2 specification** §3.1 — magic `0xE85250D6`, architecture `0` (i386 protected
//!   mode), header 24 byte, checksum bù 32-bit, end tag `{0, 0, 8}`.
//! - **Xen PVH boot protocol** — note `"Xen"`, `XEN_ELFNOTE_PHYS32_ENTRY = 18`, desc 4 byte;
//!   `hvm_start_info.magic = 0x336ec578`.

use crate::control::{CR0_PE, CR0_PG, CR4_PAE, EFER_LMA, EFER_LME, MSR_IA32_EFER};
use crate::format::{PTE_PCD, PTE_PRESENT, PTE_PS, PTE_RW};

/// Địa chỉ nạp của ảnh: 1 MiB — trên vùng EBDA/IVT/option ROM của PC, dưới 128 MiB RAM của QEMU.
pub const BOOT_LOAD_ADDR: u64 = 0x0010_0000;

/// Cỡ một lá bảng trang lớn: 2 MiB (`PS = 1` ở mức PD).
pub const LARGE_PAGE_BYTES: u64 = 1 << 21;

/// Số entry của một bảng trang 4 KiB / 8 B.
pub const PD_ENTRIES: usize = 512;

/// Identity map 1 GiB: phủ ảnh kernel (1 MiB), VGA (`0xB8000`), ROM (`0xC0000`) và mọi MMIO thấp.
///
/// 1 GiB = 512 × 2 MiB ⇒ **một** bảng PD là đủ, không cần bảng PT mức cuối.
pub const IDENTITY_MAP_BYTES: u64 = 1 << 30;

// ---------------------------------------------------------------------------------------------
// MMIO map 3..4 GiB — KT-266 (K5' bước 3)
// ---------------------------------------------------------------------------------------------
//
// LAPIC (`0xFEE0_0000`) và IOAPIC (`0xFEC0_0000`) nằm **trên** 3 GiB, ngoài identity map 1 GiB
// của KT-257. Truy cập chúng khi chưa map ⇒ `#PF` ngay lần đọc `LAPIC_ID` đầu tiên. Vì vậy ảnh
// dựng **thêm một** bảng PD (512 × 2 MiB) phủ `0xC000_0000..0x1_0000_0000` và gắn vào
// `PDPT[3]`; identity map 1 GiB **giữ nguyên** (không đổi hợp đồng KT-257).
//
// Vùng 3..4 GiB của PC là **MMIO** (PCI hole, IOAPIC, LAPIC, BIOS) nên lá 2 MiB đặt thêm
// `PCD = 1` (cache disable) — đúng cách map MMIO, không phụ thuộc QEMU có mô phỏng cache hay không.

/// Gốc vùng **MMIO map**: 3 GiB (`PDPT[3]`).
pub const MMIO_MAP_BASE: u64 = 0xC000_0000;
/// Cỡ vùng MMIO map: 1 GiB = 512 × 2 MiB.
pub const MMIO_MAP_BYTES: u64 = 1 << 30;
/// Chỉ số `PDPT` của vùng MMIO map (`3 GiB / 1 GiB`).
pub const MMIO_PDPT_INDEX: usize = 3;
/// Cờ lá 2 MiB cho vùng MMIO: `P | RW | PS | PCD` (uncached).
pub const PD_MMIO_FLAGS: u64 = PTE_PRESENT | PTE_RW | PTE_PS | PTE_PCD;

/// Entry **bảng** (PML4E/PDPTE): `PA | P | RW`.
pub const fn table_entry(pa: u64) -> u64 {
    pa | PTE_PRESENT | PTE_RW
}

/// Entry **lá 2 MiB** (PDE): `PA | P | RW | PS`.
///
/// `index` là chỉ số trong PD; địa chỉ vật lý là `index × 2 MiB`. Không đặt `US` (kernel-only),
/// không đặt `NX` (bước 1 chưa bật `EFER.NXE`, và vùng này chứa cả code lẫn stack).
pub const fn pd_entry(index: usize) -> u64 {
    ((index as u64) * LARGE_PAGE_BYTES) | PTE_PRESENT | PTE_RW | PTE_PS
}

/// Entry **lá 2 MiB** của vùng MMIO map: `MMIO_MAP_BASE + index × 2 MiB | P | RW | PS | PCD`.
pub const fn mmio_pd_entry(index: usize) -> u64 {
    MMIO_MAP_BASE + ((index as u64) * LARGE_PAGE_BYTES) | PD_MMIO_FLAGS
}

/// Một bước của chuỗi bật long mode — **thứ tự** là hợp đồng (SDM §9.8.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LongModeStep {
    /// `CR4.PAE = 1` — bắt buộc trước khi nạp `CR3` cho long mode.
    Cr4Pae,
    /// `CR3 = PML4` (địa chỉ vật lý).
    Cr3,
    /// `IA32_EFER.LME = 1` (bit 8) qua `wrmsr`.
    EferLme,
    /// `CR0.PG = 1` và `CR0.PE = 1` — lúc này `EFER.LMA` tự bật.
    Cr0PgPe,
    /// `ljmp` vào đoạn code 64-bit (selector `0x08`).
    Ljmp64,
}

impl LongModeStep {
    /// Tên ổn định để in log/asm — không phải chuỗi hiển thị cho người dùng cuối.
    pub const fn name(self) -> &'static str {
        match self {
            LongModeStep::Cr4Pae => "cr4.pae",
            LongModeStep::Cr3 => "cr3",
            LongModeStep::EferLme => "efer.lme",
            LongModeStep::Cr0PgPe => "cr0.pg|pe",
            LongModeStep::Ljmp64 => "ljmp64",
        }
    }
}

/// Chuỗi bật long mode **đúng thứ tự** — asm đi theo đúng dãy này.
pub const LONG_MODE_SEQUENCE: [LongModeStep; 5] = [
    LongModeStep::Cr4Pae,
    LongModeStep::Cr3,
    LongModeStep::EferLme,
    LongModeStep::Cr0PgPe,
    LongModeStep::Ljmp64,
];

/// Bit `CR0` mà asm đặt khi bật long mode: `PG | PE`.
pub const LONG_MODE_CR0_SET: u64 = CR0_PG | CR0_PE;
/// Bit `CR4` mà asm đặt: `PAE`.
pub const LONG_MODE_CR4_SET: u64 = CR4_PAE;
/// Bit `IA32_EFER` mà asm đặt: `LME`.
pub const LONG_MODE_EFER_SET: u64 = EFER_LME;
/// Bit `IA32_EFER` mà `kmain` đọc lại để chứng minh long mode **thật**: `LMA`.
pub const LONG_MODE_EFER_CHECK: u64 = EFER_LMA;
/// MSR `IA32_EFER` — asm `wrmsr`/`rdmsr` cùng một địa chỉ.
pub const LONG_MODE_EFER_MSR: u32 = MSR_IA32_EFER;

// ---------------------------------------------------------------------------------------------
// GDT — **chuyển** sang `crate::gdt` (KT-261) và re-export để asm của ảnh không đổi
// ---------------------------------------------------------------------------------------------
//
// KT-257 để GDT tối thiểu (3 entry) ngay trong tệp này. KT-261 mở rộng thành GDT **thật** 7 entry
// (thêm user code/data + descriptor TSS 16 byte) nên toàn bộ hình học GDT chuyển sang
// [`crate::gdt`] — một nguồn sự thật, có test host riêng. Hai tên dưới giữ nguyên cho asm của
// `_start` (đường boot 32-bit) đang dùng.

pub use crate::gdt::{
    gdt_descriptor, gdt_kernel_code, gdt_kernel_data, GDT_KERNEL_CODE, GDT_KERNEL_DATA, GDT_NULL,
};

/// Selector `CS` sau `ljmp` (index 1, RPL 0) — tên của **đường boot** cho
/// [`crate::gdt::SELECTOR_KERNEL_CODE`].
pub const SELECTOR_CODE: u16 = crate::gdt::SELECTOR_KERNEL_CODE;
/// Selector `DS/ES/SS` sau khi vào long mode (index 2, RPL 0) — tên của **đường boot** cho
/// [`crate::gdt::SELECTOR_KERNEL_DATA`].
pub const SELECTOR_DATA: u16 = crate::gdt::SELECTOR_KERNEL_DATA;

// ---------------------------------------------------------------------------------------------
// Multiboot2 header (dự phòng GRUB) — 24 byte, đặt ở đầu `.text`
// ---------------------------------------------------------------------------------------------

/// Magic multiboot2 (`0xE85250D6`).
pub const MULTIBOOT2_MAGIC: u32 = 0xE852_50D6;
/// Architecture `0` = i386 protected mode (multiboot2 spec §3.1.1).
pub const MULTIBOOT2_ARCH_I386: u32 = 0;
/// Độ dài header: 4 trường 32-bit = 16 byte + end tag 8 byte = 24.
pub const MULTIBOOT2_HEADER_LEN: u32 = 24;
/// Checksum: `2^32 − (magic + arch + len)` để tổng 32-bit của bốn trường bằng 0.
pub const MULTIBOOT2_CHECKSUM: u32 = (0x1_0000_0000u64
    - (MULTIBOOT2_MAGIC as u64 + MULTIBOOT2_ARCH_I386 as u64 + MULTIBOOT2_HEADER_LEN as u64))
    as u32;
/// End tag: `type = 0`, `flags = 0`, `size = 8`.
pub const MULTIBOOT2_END_TAG_SIZE: u32 = 8;

// ---------------------------------------------------------------------------------------------
// PVH note — đường `qemu-system-x86_64 -kernel <ELF>` thật sự dùng
// ---------------------------------------------------------------------------------------------

/// `XEN_ELFNOTE_PHYS32_ENTRY` — note chứa địa chỉ entry 32-bit.
pub const PVH_NOTE_TYPE: u32 = 18;
/// Tên note, kể cả NUL: `"Xen\0"` (4 byte).
pub const PVH_NOTE_NAME: &[u8; 4] = b"Xen\0";
/// Cỡ desc của note: một địa chỉ 32-bit.
pub const PVH_NOTE_DESC_BYTES: usize = 4;
/// `hvm_start_info.magic` — `kmain` đọc tại `%ebx` để chứng minh giao thức PVH.
pub const PVH_START_MAGIC: u32 = 0x336E_C578;

// ---------------------------------------------------------------------------------------------
// Tắt máy
// ---------------------------------------------------------------------------------------------

/// Cổng `PM1a_CNT` của ACPI trên QEMU (`-machine pc`/`q35`): `outw(0x604, 0x2000)` ⇒ QEMU thoát.
pub const ACPI_PM1A_CNT: u16 = 0x604;
/// Giá trị ghi: `SLP_TYP = 0` (S5) | `SLP_EN = 1` ⇒ `0x2000`.
pub const ACPI_SHUTDOWN_VALUE: u16 = 0x2000;
