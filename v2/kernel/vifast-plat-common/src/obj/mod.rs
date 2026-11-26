// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-417b (`D2-262` `Q2(c)`, bước 1/2)** — **shim tương thích mỏng** của object layer.
//!
//! Toàn bộ mã object/VM (`Machine`, `ObjKind`, `ObjError`, `MapFlags`, `FlushRequest`,
//! `PhysMem`, `CapId`, `WalkInfo`, `ObjCounts`, `FrameBudget`, `MAX_*`, …) đã **dời** sang
//! [`vifast_core::vm`] để có **một** nguồn sự thật trong crate core. Tệp này chỉ re-export
//! nguyên vẹn tên cũ nên **33 caller** (`v2/kernel/**`, `v2/sim/**`, test host) **không phải
//! sửa một dòng nào** và hành vi không đổi.
//!
//! Vì sao shim thay vì xoá ngay: `KT-417d` (W8a) mới là bước **hợp nhất** bảng VM vào
//! [`vifast_core::KernelStore`] + `ArchVmOps`, xoá bảng thứ hai + `core_foreign.rs` +
//! `[limits.plat]`. Giữ shim ở đây trong đúng một đợt để `KT-417b` không phải sửa 33 caller
//! (mỗi caller là một điểm rủi ro hồi quy không cần thiết cho một bước refactor cơ học).
//!
//! Lưu ý: `spec/kernel.toml` `[limits.plat]` đã trỏ `source` sang
//! `v2/kernel/vifast-core/src/vm/mod.rs#MAX_*` (5 dòng) — shim **không** khai lại hằng nào
//! (một nguồn sự thật; `check_kernel.py` đọc `const` ngay trong tệp `source`).

pub use vifast_core::vm::*;
