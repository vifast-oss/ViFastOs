// SPDX-License-Identifier: MIT OR Apache-2.0
//! Fixture của bộ case âm cho luật cấu trúc (v2/test/rules). Không phải crate thật.
//!
//! Dùng `vifast_abi_stub` để path-dependency `crates/...` (ca âm của R4) vẫn là
//! dependency thật sự — crate biên dịch được để T0 (`v2 t0`) kiểm được sau KT-400.

pub const Y: u8 = vifast_abi_stub::STUB;
