// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-61 (VD-96)** — kiểu của **kênh** `Chan[T]`: `chan[T](n)`, `send`/`recv`/`close`.
//!
//! Tách khỏi `tyck/expr.rs` để giữ **R1** ≤ 1.500 dòng (tiền lệ ST-60 tách `sig.rs`). Đây vẫn là
//! `impl Checker` đặt trong **mô-đun con** của `tyck::expr` nên dùng chung `diag`/`coerce`/`arity`
//! của cha — **một** nguồn kiểu, không nhân bản luật.

use crate::hir::HExpr;
use crate::ty::Type;
use vis_syntax::Span;

use super::super::{Checker, MISMATCH};

impl<'a> Checker<'a> {
    /// `chan[T](n)` — `n` phải `int` (âm là lỗi **lúc chạy** `VS0501`, VD-91); kết quả `Chan[T]`.
    pub(super) fn kiem_chan_new(&mut self, elem: &Type, cap: &HExpr, span: Span) -> Type {
        let et = self.resolve_ann(elem, span);
        let ct = self.kiem_expr(cap);
        if !matches!(ct, Type::Int | Type::Error) {
            self.diag(
                MISMATCH,
                format!(
                    "`chan[T](n)`: `n` phải `int` (dung lượng), được `{}`",
                    self.show(&ct)
                ),
                cap.span(),
            );
        }
        Type::TypeRef {
            name: "Chan".into(),
            args: vec![et],
        }
    }

    /// Phương thức kênh: `send(v)` đòi khớp `T` (**`VS0412` tĩnh**), `recv()` ⇒ `T?`, `close()` ⇒
    /// `()`; tên khác ⇒ `None` (đường chung báo `VS0403` như mọi kiểu không phải bản ghi).
    pub(super) fn kiem_chan_method(
        &mut self,
        c_args: &[Type],
        name: &str,
        arg_tys: &[Type],
        args: &[HExpr],
        span: Span,
    ) -> Option<Type> {
        let t = c_args[0].clone();
        match name {
            "send" => {
                self.arity(args, 1, name, span);
                self.arg_ty(arg_tys, 0, &t, name, args, span);
                Some(Type::Unit)
            }
            "recv" => {
                self.arity(args, 0, name, span);
                Some(Type::Opt(Box::new(t)))
            }
            "close" => {
                self.arity(args, 0, name, span);
                Some(Type::Unit)
            }
            _ => None,
        }
    }
}
