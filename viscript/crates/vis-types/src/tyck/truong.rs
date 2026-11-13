// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-50**: truy cập **trường**/chỉ số — tách khỏi `expr.rs` cho luật R1 (≤1.500 dòng/tệp).
//! `kiem_field` là nơi **duy nhất** phát `VS0405` ("không có trường" của bản ghi).

use crate::hir::HExpr;
use crate::ty::Type;

use super::{Checker, MISMATCH, NO_FIELD};

impl<'a> Checker<'a> {
    pub(crate) fn kiem_field(
        &mut self,
        base: &HExpr,
        name: &str,
        optional: bool,
        span: vis_syntax::Span,
    ) -> Type {
        let bt = self.kiem_expr(base);
        match &bt {
            Type::Error => Type::Error,
            Type::Record(id) => match self.types.field(*id, name) {
                Some(f) => f.ty.clone(),
                None => {
                    self.diag(
                        vis_diag::ma_truong(),
                        format!("bản ghi không có trường `{name}` (VD-43 #6)"),
                        span,
                    );
                    Type::Error
                }
            },
            Type::Enum(id) => {
                if self.types.variant(*id, name).is_some() {
                    bt.clone()
                } else {
                    self.diag(
                        vis_diag::ma_truong(),
                        format!("enum không có biến thể `{name}`"),
                        span,
                    );
                    Type::Error
                }
            }
            Type::Opt(inner) => {
                if !optional {
                    self.diag(
                        MISMATCH,
                        format!(
                            "`{}` là optional — dùng `?.` để truy cập trường (§4.4)",
                            self.show(&bt)
                        ),
                        span,
                    );
                    return Type::Error;
                }
                match &**inner {
                    Type::Record(id) => match self.types.field(*id, name) {
                        Some(f) => {
                            let t = f.ty.clone();
                            Type::Opt(Box::new(t))
                        }
                        None => {
                            self.diag(
                                vis_diag::ma_truong(),
                                format!("bản ghi không có trường `{name}`"),
                                span,
                            );
                            Type::Error
                        }
                    },
                    Type::Error => Type::Error,
                    other => {
                        // Kiểu **không phải bản ghi**: giữ mã cũ `VS0403` (không phải "trường
                        // của bản ghi"); chỉ bản ghi/enum mới dùng `VS0405` (ST-50).
                        self.diag(
                            NO_FIELD,
                            format!("`{}` không có trường `{name}`", self.show(other)),
                            span,
                        );
                        Type::Error
                    }
                }
            }
            other => {
                // Kiểu **không phải bản ghi** ⇒ `VS0403` như trước (ST-50 giữ nghĩa cũ).
                self.diag(
                    NO_FIELD,
                    format!("`{}` không có trường `{name}`", self.show(other)),
                    span,
                );
                Type::Error
            }
        }
    }

    pub(crate) fn kiem_index(
        &mut self,
        base: &HExpr,
        index: &HExpr,
        _span: vis_syntax::Span,
    ) -> Type {
        let bt = self.kiem_expr(base);
        let it = self.kiem_expr(index);
        match &bt {
            Type::List(t) => {
                let want = Type::Int;
                self.coerce(&it, &want, index.span(), "chỉ số `list`");
                (**t).clone()
            }
            Type::Map(k, v) => {
                let kk = (**k).clone();
                self.coerce(&it, &kk, index.span(), "khoá `map`");
                (**v).clone()
            }
            Type::Str => {
                self.diag(
                    MISMATCH,
                    "`str` không có `s[i]` — dùng `.chars()`, `.graphemes()`, `.byte_at(i)?` (VD-18)",
                    base.span(),
                );
                Type::Error
            }
            Type::Error => Type::Error,
            other => {
                self.diag(
                    MISMATCH,
                    format!("không chỉ số được kiểu `{}`", self.show(other)),
                    base.span(),
                );
                Type::Error
            }
        }
    }
}
