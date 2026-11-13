// SPDX-License-Identifier: MIT OR Apache-2.0
//! `vis-types` — phân giải tên + hạ **CST → HIR** (ST-11); kiểm kiểu VS0 là ST-12.
//!
//! Thiết kế §14.2: `vis-types` phụ thuộc `vis-syntax` + `vis-diag`, **không** phụ thuộc hệ điều
//! hành, không `unsafe`, tệp ≤ 1.500 dòng.
//!
//! # Phạm vi ST-11
//!
//! - [`Type`] theo VD-43 (+ `float` của thiết kế §3.1) và bảng kiểu người dùng [`TypeTable`].
//! - [`Hir`] cho **chế độ script**: item, câu lệnh, biểu thức, pattern (xem [`hir`]).
//! - [`lower`]: hạ CST → HIR, **giải tên giá trị** (khung + khung hàm + khối, shadowing hợp lệ,
//!   khai trùng ⇒ `VS0302`) và **điền kiểu literal** (int/size/duration/percent/str/bool/bytes/
//!   path/char/float, list/map/record/enum, `T?`).
//! - Hạ **permissive** (VD-44 #1): mọi cú pháp của Phụ lục A hạ được, **0 chẩn đoán** cho 15 khối
//!   tài liệu; tên kiểu chưa phân giải giữ dạng [`Type::TypeRef`] (ST-12 mới báo).
//!
//! # Ví dụ
//!
//! ```
//! use vis_syntax::{lex, parse, Mode};
//!
//! let src = "let x = 1\nlet y = x + 2\n";
//! let out = parse(src, Mode::Script);
//! assert!(out.diagnostics.is_empty());
//! let h = vis_types::lower(&out.cst, &lex(src, Mode::Script));
//! assert!(h.is_clean(), "chẩn đoán: {:?}", h.codes());
//! assert_eq!(h.hir.stmts.len(), 2);
//! // `y = x + 2`: `x` phân giải về `let x`.
//! let vis_types::HStmt::Let { init, .. } = &h.hir.stmts[1] else {
//!     panic!("câu lệnh thứ hai phải là `let`");
//! };
//! let vis_types::HExpr::Bin { lhs, .. } = init else {
//!     panic!("giá trị phải là phép cộng");
//! };
//! let vis_types::HExpr::Var { name, sym, .. } = &**lhs else {
//!     panic!("vế trái phải là tên");
//! };
//! assert_eq!(name, "x");
//! assert!(sym.is_some(), "`x` phải phân giải được");
//! ```
//!
//! # Ví dụ lỗi (`VS0302`: khai trùng cùng phạm vi)
//!
//! ```
//! use vis_syntax::{lex, parse, Mode};
//!
//! let src = "let x = 1\nlet x = 2\n";
//! let out = parse(src, Mode::Script);
//! let h = vis_types::lower(&out.cst, &lex(src, Mode::Script));
//! assert_eq!(h.codes(), ["VS0302"]);
//! ```

pub mod hir;
pub mod lower;
pub mod ty;
pub mod tyck;

pub use hir::{
    AssignOp, BinOp, FnKind, HAlias, HArm, HBody, HCond, HConst, HElse, HEnum, HExpr, HField, HFn,
    HImpl, HItem, HParam, HPat, HRec, HSig, HStmt, HTrait, HUse, HVariant, Hir, InterpPart, Lit,
    LitKind, UnOp,
};
pub use lower::{
    is_builtin_value, lower, lower_into, lower_with, FrameKind, HFrame, HSym, HirOutput,
    LowerError, LowerOptions, ModInfo, NamePolicy, SymKind, Symbols, BUILTIN_VALUE_NAMES,
};
pub use ty::{EnumDef, EnumId, FieldDef, RecDef, RecId, Type, TypeTable, VariantDef};
pub use tyck::{check, CheckOutput};
