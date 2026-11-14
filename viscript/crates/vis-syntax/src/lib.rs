// SPDX-License-Identifier: MIT OR Apache-2.0
//! `vis-syntax` — lexer + parser + CST + `CmdCST` của dòng ViScript (tài liệu thiết kế §14.2).
//!
//! **ST-04** mới dựng **lexer** theo [`docs/viscript/SPEC-lex.md`] (APPROVED 2026-11-12).
//! Parser/CST là ST-05.
//!
//! Luật của dòng ViScript: không `unsafe`, không tệp quá 1.500 dòng, SPDX `//` cho Rust.
//!
//! # Hợp đồng cơ bản
//!
//! - Đầu vào được **chuẩn hoá NFC toàn tệp trước khi lex** (SPEC-lex §0.1, VD-11); mọi offset
//!   trả về tính trên văn bản **đã NFC** ([`LexOutput::text`]) nên `text[span] == token.text`.
//! - Lexer **thuần**: cùng đầu vào + cùng chế độ ⇒ cùng đầu ra; không đọc tệp, không giờ, không
//!   phụ thuộc kiểu.
//! - Lỗi từ vựng không làm dừng lex (trừ chuỗi chưa đóng): chẩn đoán đi kèm danh sách token để
//!   ST-05/`vis-diag` báo nhiều lỗi một lượt.
//!
//! # Ví dụ
//!
//! ```
//! use vis_syntax::{lex, Mode, Kind};
//!
//! let out = lex("let x = 0xff", Mode::Script);
//! assert!(out.diagnostics.is_empty());
//! let kinds: Vec<Kind> = out.tokens.iter().map(|t| t.kind).collect();
//! assert_eq!(kinds, [Kind::Kw, Kind::Ident, Kind::Punct, Kind::Int, Kind::Eof]);
//! ```

pub mod cst;
pub mod lex;
pub mod parse;

pub use cst::{Cst, Elem, Node, NodeKind};
pub use lex::lit;
pub use lex::token::{is_keyword, Kind, Span, Token, KEYWORDS, PUNCTS};
pub use lex::verify;
pub use lex::{lex, Diagnostic, LexOutput, Mode};
pub use parse::cmd::{
    dinh_danh_chua_bind, let_la_khai, let_la_khai_voi, ten_khai_bao, ten_mac_dinh, tham_so_cua,
    than_khai_lenh, than_khai_lenh_voi,
};
pub use parse::{parse, parse_voi, ParseOutput};
