// SPDX-License-Identifier: MIT OR Apache-2.0
//! `vifast-plat-aarch64` — phần **host-testable** của platform AArch64 (KT-160 / D2-147).
//!
//! K4' là milestone lớn; đêm 2026-11-19 chỉ mở **bước 1** có tiêu chí đo được:
//!
//! - [`format`]: định dạng **AArch64 VMSA 4 KiB granule, 4 mức, 39-bit VA canonical, 48-bit PA**
//!   — descriptor 64-bit với `V`, `AP[2:1]`, `AF`, `UXN/PXN`, `SH[1:0]`, `AttrIndx` (MAIR), `nG`;
//!   `impl PageTableFormat for Aarch64` (trait **dùng chung** ở `vifast-pte` từ KT-164);
//! - [`mmu`]: `Mmu` **thuần dữ liệu** ([`PageTable`]) — map/unmap/translate/walk/destroy trên
//!   nguồn frame trừu tượng ([`TableMemory`]), **không** asm, **không** `unsafe`; `impl Mmu` cho
//!   [`PageTable`] (trait `Mmu` cũng ở `vifast-pte` từ KT-164).
//!
//! # Crate này **không** làm gì (bước 2 của K4')
//!
//! Không `TTBR0_EL1`/`TTBR1_EL1`, không `MAIR_EL1`/`TCR_EL1`, không `TLBI`/`dsb`/`isb`, không
//! boot aarch64, không QEMU, không `cfg(target_arch)`. **Không tuyên bố K4' đạt** — D2-147.
//!
//! # Vì sao `no_std` + `forbid(unsafe_code)`
//!
//! Cùng lý do `vifast-plat-common` (D2-72 Q12): phần logic tách khỏi asm/MMIO nên chạy được trên
//! host, và `unsafe` chỉ được phép trong danh sách đóng D2-17 — crate này **không cần** `unsafe`
//! nào, nên cấm hẳn để không mở đường.
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64
//! ```

#![no_std]
#![forbid(unsafe_code)]

pub mod format;
pub mod mmu;

/// `Walk` chẩn đoán phải ghi được mọi mức của format AArch64 — kiểm lúc biên dịch.
const _: () = assert!(mmu::MAX_LEVELS >= format::AARCH64_LEVELS);

pub use format::{
    is_canonical_bits, mair_byte, mair_index, Aarch64, Descriptor, MemAttr, PageTableFormat, Perms,
    Shareability, AARCH64_ENTRIES, AARCH64_INDEX_BITS, AARCH64_LEVELS, AARCH64_MAX_VA_BITS,
    AARCH64_PAGE_BITS, AARCH64_PAGE_BYTES, AARCH64_PA_BITS, AARCH64_VA_BITS, DESC_ADDR_MASK,
    DESC_ADDR_SHIFT, DESC_AF, DESC_AP_RO, DESC_AP_USER, DESC_ATTR_SHIFT, DESC_NG, DESC_NS,
    DESC_PXN, DESC_SH_SHIFT, DESC_TABLE_OR_PAGE, DESC_UXN, DESC_VALID, MAIR_DEVICE_NGNRNE,
    MAIR_EL1, MAIR_NORMAL_WB_RW_ALLOC,
};
pub use mmu::{
    MapError, Mmu, PageTable, TableMemory, Translation, Walk, WalkOutcome, WalkStep, MAX_LEVELS,
};
