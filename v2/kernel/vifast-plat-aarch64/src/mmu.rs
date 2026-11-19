// SPDX-License-Identifier: MIT OR Apache-2.0
//! `Mmu` **thuần dữ liệu** — re-export từ crate dùng chung `vifast-pte` (KT-186 / D2-163).
//!
//! Trước KT-186, tệp này chứa **toàn bộ** walker `PageTable<F, M>` + trait [`TableMemory`]
//! (KT-160). Audit K5' (`v2/agent/reports/K5-X86-READINESS.md` §2.4) chỉ ra rằng nếu x86_64
//! dùng lại khuôn này thì phải chuyển về `vifast-pte`, nếu không sẽ có **bản thứ hai** — đúng
//! lớp lỗi "hai bản trùng" mà KT-164 vừa đóng. KT-186 đã chuyển; tệp này chỉ còn re-export để
//! đường dẫn cũ `vifast_plat_aarch64::mmu::*` giữ nguyên.
//!
//! Phần **arch** (`TTBR0_EL1`/`TTBR1_EL1`/`MAIR_EL1`/`TCR_EL1`/`TLBI`/`dsb`/`isb`) vẫn ở
//! [`crate::sysreg`]/[`crate::tlb`]/[`crate::barrier`]/[`crate::asm`] — D2-17.

pub use vifast_pte::{
    MapError, Mmu, PageTable, TableMemory, Translation, Walk, WalkOutcome, WalkStep, MAX_LEVELS,
};
