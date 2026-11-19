// SPDX-License-Identifier: MIT OR Apache-2.0
//! `vifast-plat-x86_64` — platform x86_64 phần **host-testable** (KT-186 / D2-163, K5' bước M2).
//!
//! K5' là milestone lớn; D2-163 duyệt **M1** (phạm vi "như K3", boot tạm QEMU `-kernel`,
//! `spec/abi.toml` `[calling.x86_64]` **chưa mở**) và cho làm **M2** — bước **không cần QEMU**,
//! đo bằng `cargo test` trên host, đúng khuôn K4' bước một (D2-147).
//!
//! # Nội dung
//!
//! - [`format`]: định dạng **x86_64 4-level paging, 4 KiB, 39-bit VA canonical, 48-bit PA** —
//!   descriptor `P/RW/US/PWT/PCD/A/D/PS/PAT/G/NX`; `impl PageTableFormat for X86_64` (trait
//!   **dùng chung** ở `vifast-pte` từ KT-164);
//! - [`mmu`]: `Mmu` **thuần dữ liệu** — re-export walker dùng chung `PageTable<F, M>` +
//!   [`TableMemory`](mmu::TableMemory) từ `vifast-pte` (KT-186), type alias [`X86Mmu`](mmu::X86Mmu);
//! - [`control`]: hằng bit `CR0`/`CR4`/`IA32_EFER` + địa chỉ MSR (`IA32_EFER`, `STAR`, `LSTAR`,
//!   `FMASK`, `PAT`, …) — bước M4/M7 sẽ nạp;
//! - [`calling`]: bảng thanh ghi **System V AMD64 ABI** + **đề xuất** ánh xạ syscall ViFastOs
//!   (`syscall` vs `int 0x80`) — bản nháp cho M7, **chưa** phải ABI.
//!
//! # Crate này **không** làm gì
//!
//! Không boot, không QEMU, không linker script, không GDT/IDT/TSS, không `CR3`/`INVLPG`, không
//! `syscall`/`sysret` thật, không tuyên bố K5' đạt — D2-163. Bước M3 mới là entry + GDT/IDT/TSS;
//! M4 mới bật paging thật; M7 mới mở `spec/` `[calling.x86_64]` theo khuôn KT-05.
//!
//! # Vì sao `forbid(unsafe_code)`
//!
//! Bước M2 là **hợp đồng dữ liệu**; `unsafe` chỉ được phép trong danh sách đóng D2-17 và bước
//! M2 **không** cần. `forbid` (không phải `deny`) để không ai `allow` cục bộ mà lọt.
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-x86_64
//! cargo build --manifest-path v2/kernel/Cargo.toml -p vifast-plat-x86_64 --target x86_64-unknown-none
//! ```

#![no_std]
#![forbid(unsafe_code)]

pub mod calling;
pub mod control;
pub mod format;
pub mod mmu;

/// `Walk` chẩn đoán phải ghi được mọi mức của format x86_64 — kiểm lúc biên dịch.
const _: () = assert!(mmu::MAX_LEVELS >= format::X86_64_LEVELS);

pub use calling::{
    Reg, INT80_MR_REGS, INT80_RETURN_MSGINFO_REG, INT80_VECTOR, SYSCALL_ARG_REGS,
    SYSCALL_CLOBBERED, SYSCALL_MR_REGS, SYSCALL_MSGINFO_REG, SYSCALL_MSRS, SYSCALL_NUMBER_REG,
    SYSCALL_RETURN_ERROR_REG, SYSCALL_RETURN_MSGINFO_REG, SYSCALL_RETURN_VALUE_REG, SYSV_ARG_REGS,
    SYSV_CALLEE_SAVED, SYSV_CALLER_SAVED, SYSV_DIRECTION_FLAG_CLEAR, SYSV_FRAME_POINTER,
    SYSV_RED_ZONE_BYTES, SYSV_RETURN_REGS, SYSV_STACK_ALIGN_BYTES, SYSV_STACK_POINTER,
    SYSV_VARIADIC_VECTOR_COUNT_REG, SYSV_VECTOR_ARG_REGS, SYSV_VECTOR_RETURN_REGS,
};
pub use control::{
    CR0_AM, CR0_CD, CR0_EM, CR0_ET, CR0_MP, CR0_NE, CR0_NW, CR0_PE, CR0_PG, CR0_TS, CR0_WP,
    CR3_ADDR_MASK, CR3_PCD, CR3_PCID_MASK, CR3_PWT, CR4_CET, CR4_DE, CR4_FSGSBASE, CR4_LA57,
    CR4_MCE, CR4_OSFXSR, CR4_OSXMMEXCPT, CR4_OSXSAVE, CR4_PAE, CR4_PCE, CR4_PCIDE, CR4_PGE,
    CR4_PKE, CR4_PKS, CR4_PSE, CR4_PVI, CR4_SMAP, CR4_SMEP, CR4_SMXE, CR4_TSD, CR4_UMIP, CR4_VME,
    CR4_VMXE, EFER_FFXSR, EFER_LMA, EFER_LME, EFER_LMSLE, EFER_NXE, EFER_SCE, EFER_SVME, EFER_TCE,
    LONG_MODE_CR0, LONG_MODE_CR4, LONG_MODE_EFER, MSR_IA32_APIC_BASE, MSR_IA32_EFER,
    MSR_IA32_FMASK, MSR_IA32_FS_BASE, MSR_IA32_GS_BASE, MSR_IA32_KERNEL_GS_BASE, MSR_IA32_LSTAR,
    MSR_IA32_PAT, MSR_IA32_STAR,
};
pub use format::{
    is_canonical_bits, Descriptor, MemAttr, PageTableFormat, Perms, Shareability, IA32_PAT_DEFAULT,
    PTE_ACCESSED, PTE_ADDR_MASK, PTE_ADDR_SHIFT, PTE_DIRTY, PTE_GLOBAL, PTE_NX, PTE_PAT,
    PTE_PAT_LARGE, PTE_PCD, PTE_PRESENT, PTE_PS, PTE_PWT, PTE_RESERVED_MASK, PTE_RW, PTE_USER,
    X86_64, X86_64_ENTRIES, X86_64_INDEX_BITS, X86_64_LEVELS, X86_64_MAX_VA_BITS, X86_64_PAGE_BITS,
    X86_64_PAGE_BYTES, X86_64_PA_BITS, X86_64_VA_BITS,
};
pub use mmu::{
    MapError, Mmu, PageTable, TableMemory, Translation, Walk, WalkOutcome, WalkStep, X86Mmu,
    MAX_LEVELS,
};
