// SPDX-License-Identifier: MIT OR Apache-2.0
//! `Mmu` dữ liệu cho x86_64 — dùng walker **dùng chung** ở `vifast-pte` (KT-186 / D2-163).
//!
//! Trước KT-186, walker `PageTable<F, M>` + trait [`TableMemory`] nằm trong
//! `vifast-plat-aarch64` (KT-160). Audit K5' §2.4 chỉ ra rằng x86_64 dùng lại khuôn đó thì phải
//! chuyển về `vifast-pte`, nếu không sẽ có **bản thứ hai**. KT-186 đã chuyển; tệp này chỉ còn
//! re-export + type alias cho x86_64, **không** asm, **không** `unsafe`.
//!
//! Phần **arch** (`CR3`, `CR0.PG`, `CR4.PAE`, `EFER.LME`, `INVLPG`, `MOV CR3`) thuộc bước M4
//! (crate arch, D2-17) — bước M2 chỉ chốt phần dữ liệu map/unmap/translate/walk/destroy.

pub use vifast_pte::{
    MapError, Mmu, PageTable, TableMemory, Translation, Walk, WalkOutcome, WalkStep, MAX_LEVELS,
};

use crate::format::X86_64;

/// `Mmu` của x86_64 trên một nguồn frame `M` — alias của walker dùng chung.
pub type X86Mmu<M> = PageTable<X86_64, M>;
