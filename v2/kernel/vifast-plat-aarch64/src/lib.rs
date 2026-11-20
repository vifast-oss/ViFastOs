// SPDX-License-Identifier: MIT OR Apache-2.0
//! `vifast-plat-aarch64` — platform AArch64 phần **host-testable** (KT-160 / KT-167).
//!
//! K4' là milestone lớn; đêm 2026-11-19 mở **hai bước** có tiêu chí đo được:
//!
//! - **Bước 1** (KT-160 / D2-147):
//!   - [`format`]: định dạng **AArch64 VMSA 4 KiB granule, 4 mức, 39-bit VA canonical, 48-bit
//!     PA** — descriptor 64-bit với `V`, `AP[2:1]`, `AF`, `UXN/PXN`, `SH[1:0]`, `AttrIndx`
//!     (MAIR), `nG`; `impl PageTableFormat for Aarch64` (trait **dùng chung** ở `vifast-pte`
//!     từ KT-164);
//!   - [`mmu`]: `Mmu` **thuần dữ liệu** ([`PageTable`]) — map/unmap/translate/walk/destroy
//!     trên nguồn frame trừu tượng ([`TableMemory`]), **không** asm, **không** `unsafe`.
//! - **Bước 2** (KT-167):
//!   - [`sysreg`]: encoding **thuần dữ liệu** của `TCR_EL1`, `MAIR_EL1`, `SCTLR_EL1`,
//!     `TTBR0_EL1`/`TTBR1_EL1`, `VBAR_EL1`, `ESR_EL1`, `FAR_EL1`, `ELR_EL1`, `SPSR_EL1`,
//!     `DAIF` + builder const (`Tcr`, `Mair`, `Sctlr`, `Ttbr`, `Vbar`, `Esr`, `Spsr`, `Daif`);
//!   - [`tlb`]: mã thao tác `TLBI` (`vmalle1`, `vae1`, `aside1`, `vmalle1is`, `vae1is`,
//!     `aside1is`) dạng hằng encoding host-testable + hàm asm;
//!   - [`barrier`]: `dsb ish`, `dmb ish`, `isb` (asm) + encoding host-testable;
//!   - [`asm`]: `msr`/`mrs` thanh ghi hệ thống + `TLBI`/barrier thật — **mô-đun duy nhất** có
//!     `unsafe` (D2-17), có **stub host panic** để `cargo test` trên host vẫn chạy.
//!
//! # Crate này **không** làm gì (bước 3 của K4')
//!
//! Không boot aarch64, không linker script, không QEMU, không dựng `VSpace` thật, không tuyên bố
//! K4' đạt — D2-147. Bước 2 chỉ chốt **encoding + asm nguyên thủy**; việc nối `TTBR*`/`TCR`/`MAIR`
//! vào một đường boot là bước 3.
//!
//! # Vì sao `no_std` + `deny(unsafe_code)` (không còn `forbid`)
//!
//! Phần **dữ liệu** ([`format`], [`mmu`], [`sysreg`], [`tlb`], [`barrier`]) vẫn **không** có
//! `unsafe` nào. `asm` là ngoại lệ duy nhất: `asm!` buộc phải nằm trong `unsafe`, và D2-17 cho
//! phép `unsafe` trong `kernel/vifast-plat-*`. Vì `forbid` **không thể** bị `allow` ở module con,
//! gốc crate đổi sang `deny` và **chỉ** [`asm`] `#![allow(unsafe_code)]` — mọi module khác vẫn
//! bị `deny` chặn. Đây là điều kiện của KT-167, không phải nới luật: `unsafe` vẫn nằm trong danh
//! sách đóng D2-17.
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64
//! cargo build --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64 --target aarch64-unknown-none
//! ```

#![no_std]
#![deny(unsafe_code)]

pub mod asm;
pub mod barrier;
pub mod boot_mmu;
pub mod bootmem;
pub mod format;
pub mod mmu;
pub mod pl011;
pub mod resources;
pub mod split_mmu;
pub mod sysreg;
pub mod tlb;
pub mod vector;

/// Dừng máy **sạch** (KT-247): `PSCI SYSTEM_OFF` qua `hvc #0`, dự phòng `wfi` vô hạn.
///
/// QEMU `virt` với `-kernel` đặt PSCI conduit = HVC (khách ở EL1) ⇒ `SYSTEM_OFF` làm QEMU
/// thoát mã 0. Nếu lệnh không tắt được (môi trường không có PSCI), hàm **không** im lặng: nó
/// chuyển sang [`asm::wfi_forever`] — runner phân biệt bằng marker đã in trước đó.
///
/// `code` chỉ để phân loại trong log (giống riscv64: PSCI không mang mã thoát).
pub fn debug_halt(_code: usize) -> ! {
    asm::psci_system_off();
    asm::wfi_forever()
}

/// `Walk` chẩn đoán phải ghi được mọi mức của format AArch64 — kiểm lúc biên dịch.
const _: () = assert!(mmu::MAX_LEVELS >= format::AARCH64_LEVELS);

pub use boot_mmu::{
    apply_step, enable_mmu, level1_base, map_identity, mmu_is_on, sctlr_for_mmu_on, start_level,
    ttbr0_base, ttbr1_base, MmuStep, MMU_ENABLE_SEQUENCE,
};
pub use bootmem::{
    boot_memory, shared_boot_memory, BootMemory, SharedBootMemory, BOOT_ARENA_FRAMES,
    BOOT_ARENA_WORDS,
};
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
pub use split_mmu::{
    half_of, high_alias_va, map_high_alias, map_physmap, physmap_va, Half, SplitMmu,
    HIGH_HALF_BASE, KERNEL_HIGH_BASE, PHYSMAP_BASE,
};
pub use sysreg::{
    Daif, ElrEl1, Esr, ExceptionClass, FarEl1, Granule, Ips, Mair, Sctlr, Spsr, SpsrMode, Tcr,
    TcrCache, Ttbr, Vbar, MAIR_ATTR_DEVICE_NGNRE, MAIR_ATTR_DEVICE_NGNRNE, MAIR_ATTR_NORMAL_NC,
    MAIR_ATTR_NORMAL_WB, MAIR_ATTR_NORMAL_WT, MAIR_EL1_KERNEL, MAIR_SLOTS, MAIR_SLOT_MASK,
    MAIR_SLOT_SHIFT, SCTLR_A, SCTLR_ATA, SCTLR_BT0, SCTLR_BT1, SCTLR_C, SCTLR_CP15BEN, SCTLR_DSSBS,
    SCTLR_DZE, SCTLR_E0E, SCTLR_EE, SCTLR_EIS, SCTLR_ENALS, SCTLR_ENAS0, SCTLR_ENASR, SCTLR_ENDA,
    SCTLR_ENDB, SCTLR_ENIA, SCTLR_ENIB, SCTLR_ENTP2, SCTLR_EOS, SCTLR_EPAN, SCTLR_I, SCTLR_IESB,
    SCTLR_ITD, SCTLR_ITFSB, SCTLR_LSMAOE, SCTLR_M, SCTLR_NTLSMD, SCTLR_NTWE, SCTLR_NTWI, SCTLR_SA,
    SCTLR_SA0, SCTLR_SED, SCTLR_SPAN, SCTLR_TSCXT, SCTLR_UCI, SCTLR_UCT, SCTLR_UMA, SCTLR_WXN,
    SPSR_C, SPSR_M_MASK, SPSR_M_SHIFT, SPSR_N, SPSR_V, SPSR_Z, TCR_A1, TCR_A1_SHIFT, TCR_AS,
    TCR_AS_SHIFT, TCR_EL1_KERNEL, TCR_EPD0, TCR_EPD0_SHIFT, TCR_EPD1, TCR_EPD1_SHIFT, TCR_IPS_MASK,
    TCR_IPS_SHIFT, TCR_IRGN0_MASK, TCR_IRGN0_SHIFT, TCR_IRGN1_MASK, TCR_IRGN1_SHIFT,
    TCR_ORGN0_MASK, TCR_ORGN0_SHIFT, TCR_ORGN1_MASK, TCR_ORGN1_SHIFT, TCR_SH0_MASK, TCR_SH0_SHIFT,
    TCR_SH1_MASK, TCR_SH1_SHIFT, TCR_T0SZ_MASK, TCR_T0SZ_SHIFT, TCR_T1SZ_MASK, TCR_T1SZ_SHIFT,
    TCR_TBI0, TCR_TBI0_SHIFT, TCR_TBI1, TCR_TBI1_SHIFT, TCR_TG0_MASK, TCR_TG0_SHIFT, TCR_TG1_MASK,
    TCR_TG1_SHIFT, TTBR_ASID_MASK, TTBR_ASID_SHIFT, TTBR_BADDR_MASK, TTBR_BADDR_SHIFT, TTBR_CNP,
    VBAR_BASE_MASK, VBAR_BASE_SHIFT,
};
pub use tlb::{
    tlbi_encoding, tlbi_va_arg, TLBI_ASIDE1, TLBI_ASIDE1IS, TLBI_SYS_BASE, TLBI_VAE1, TLBI_VAE1IS,
    TLBI_VMALLE1, TLBI_VMALLE1IS,
};
pub use vector::{
    is_aligned, vector_offset, VectorKind, VECTOR_ALIGN, VECTOR_COUNT, VECTOR_STRIDE,
};
