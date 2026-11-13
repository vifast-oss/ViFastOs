// SPDX-License-Identifier: MIT OR Apache-2.0
//! `vis-bytecode` — **VBC1**, định dạng bytecode nội bộ của ViScript (ST-19, VD-52).
//!
//! Crate **thuần dữ liệu**: không phụ thuộc OS, không `unsafe`, không I/O. Ba phần:
//!
//! - [`format`] — mô hình `Module`/`Func`/`Instr` + bố cục tệp + hash nguồn.
//! - [`encode`] / [`decode`] — ghi/đọc **little-endian**, độ dài tường minh; dữ liệu hỏng trả
//!   [`decode::DecodeError`], **không** panic.
//! - [`codegen`] — hạ **HIR** (`vis-types`) xuống [`format::Module`].
//!
//! **Chưa** có VM/verifier (ST-20/ST-21). Mọi bytecode nạp từ cache **phải** qua verifier khi nó
//! tồn tại (VD-52 #3) — đó là hàng rào bù cho hash nguồn **không phải** SHA-256 (xem [`format`]).

pub mod codegen;
pub mod decode;
pub mod encode;
pub mod format;
pub mod util;

pub use codegen::{compile, CodegenError};
pub use decode::{decode, DecodeError};
pub use encode::encode;
pub use format::{source_hash, Const, Func, Instr, Module, Span, HASH_LEN, MAGIC, VERSION};
