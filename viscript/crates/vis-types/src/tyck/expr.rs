// SPDX-License-Identifier: MIT OR Apache-2.0
//! Kiểu của **biểu thức** (ST-12) — số học, so sánh, gọi hàm, trường, chỉ số, bản ghi/enum.

use crate::hir::{BinOp, HCond, HExpr, HPat, HStmt, UnOp};
use crate::ty::{EnumId, Type};

use super::{num, Checker, ARG_COUNT, MISMATCH, NO_FIELD};

impl<'a> Checker<'a> {
    /// Kiểu một biểu thức (ghi vào `types_of_expr`).
    pub(crate) fn kiem_expr(&mut self, e: &HExpr) -> Type {
        let t = self.kiem_expr_trong(e);
        self.note(t.clone())
    }

    fn kiem_expr_trong(&mut self, e: &HExpr) -> Type {
        match e {
            HExpr::Lit { ty, .. } => ty.clone(),
            HExpr::Var { name, sym, .. } => match sym {
                Some(id) => self.ty_of(*id),
                None => builtin_value_ty(name),
            },
            HExpr::Bin { op, lhs, rhs, span } => {
                let l = self.kiem_expr(lhs);
                let r = self.kiem_expr(rhs);
                self.kiem_bin(*op, &l, &r, lhs.span(), rhs.span(), *span)
            }
            HExpr::Un { op, expr, span } => {
                let t = self.kiem_expr(expr);
                match op {
                    UnOp::Neg => {
                        let ok = matches!(
                            t,
                            Type::Int
                                | Type::Float
                                | Type::Size
                                | Type::Duration
                                | Type::Percent
                                | Type::Error
                        );
                        if ok {
                            t
                        } else {
                            self.diag(
                                MISMATCH,
                                format!("`-` một ngôi cần kiểu số, được `{}`", self.show(&t)),
                                *span,
                            );
                            Type::Error
                        }
                    }
                    UnOp::Not => {
                        if matches!(t, Type::Bool | Type::Error) {
                            Type::Bool
                        } else {
                            self.diag(
                                MISMATCH,
                                format!("`not` cần `bool`, được `{}`", self.show(&t)),
                                *span,
                            );
                            Type::Error
                        }
                    }
                }
            }
            HExpr::Call {
                callee,
                args,
                named,
                span,
            } => self.kiem_call(callee, args, named, *span),
            HExpr::Field {
                base,
                name,
                optional,
                span,
            } => self.kiem_field(base, name, *optional, *span),
            HExpr::Index { base, index, span } => self.kiem_index(base, index, *span),
            HExpr::List {
                items,
                elem: _,
                span,
            } => {
                let elem = self.unify_all(items, "phần tử `list`", *span);
                Type::List(Box::new(elem))
            }
            HExpr::Set {
                items,
                elem: _,
                span,
            } => {
                let elem = self.unify_all(items, "phần tử `set`", *span);
                Type::List(Box::new(elem))
            }
            HExpr::Map {
                entries,
                key: _,
                value: _,
                span,
            } => {
                let mut kt = Type::Error;
                let mut vt = Type::Error;
                for (i, (k, v)) in entries.iter().enumerate() {
                    let kt2 = self.kiem_expr(k);
                    let vt2 = self.kiem_expr(v);
                    if i == 0 {
                        kt = kt2;
                        vt = vt2;
                    } else {
                        self.coerce(&kt2, &kt, k.span(), "khoá `map`");
                        self.coerce(&vt2, &vt, v.span(), "giá trị `map`");
                    }
                }
                let _ = span;
                Type::Map(Box::new(kt), Box::new(vt))
            }
            HExpr::Record { fields, ty, .. } => {
                for (_, v) in fields {
                    let _ = self.kiem_expr(v);
                }
                ty.clone()
            }
            HExpr::EnumLit {
                enum_id,
                variant,
                args,
                span,
                ..
            } => self.kiem_enum_lit(*enum_id, variant, args, *span),
            HExpr::Opt { value, ty, .. } => match value {
                None => Type::Opt(Box::new(Type::Error)),
                Some(v) => {
                    let t = self.kiem_expr(v);
                    let _ = ty;
                    Type::Opt(Box::new(t))
                }
            },
            HExpr::Try { expr, span } => {
                let t = self.kiem_expr(expr);
                match t {
                    Type::Opt(inner) | Type::Result(inner, _) => (*inner).clone(),
                    Type::Error => Type::Error,
                    other => {
                        self.diag(
                            MISMATCH,
                            format!(
                                "`?` chỉ dùng cho `T?`/`Result`, được `{}`",
                                self.show(&other)
                            ),
                            *span,
                        );
                        Type::Error
                    }
                }
            }
            HExpr::With { base, fields, span } => {
                let bt = self.kiem_expr(base);
                match bt {
                    Type::Record(id) => {
                        for (n, v) in fields {
                            let vt = self.kiem_expr(v);
                            match self.types.field(id, n) {
                                Some(f) => {
                                    let want = f.ty.clone();
                                    self.coerce(&vt, &want, v.span(), "`with` cập nhật trường");
                                }
                                None => self.diag(
                                    NO_FIELD,
                                    format!("bản ghi không có trường `{n}` (VD-43 #6)"),
                                    *span,
                                ),
                            }
                        }
                        Type::Record(id)
                    }
                    Type::Error => Type::Error,
                    other => {
                        self.diag(
                            MISMATCH,
                            format!("`with` cần bản ghi, được `{}`", self.show(&other)),
                            *span,
                        );
                        Type::Error
                    }
                }
            }
            HExpr::If {
                cond,
                then,
                elifs,
                else_,
                span,
                ..
            } => {
                self.kiem_cond(cond);
                let mut acc = self.than_value(then);
                for (c, body) in elifs {
                    self.kiem_cond(c);
                    let t = self.than_value(body);
                    self.unify_branch(&mut acc, &t, *span);
                }
                if !else_.is_empty() {
                    let t = self.than_value(else_);
                    self.unify_branch(&mut acc, &t, *span);
                } else {
                    acc = Type::Unit;
                }
                acc
            }
            HExpr::Match {
                subject,
                arms,
                span,
                ..
            } => {
                let subj = self.kiem_expr(subject);
                let mut acc: Option<Type> = None;
                for arm in arms {
                    self.bind_pat(&arm.pat, &subj);
                    if let Some(g) = &arm.guard {
                        let gt = self.kiem_expr(g);
                        self.coerce(&gt, &Type::Bool, g.span(), "guard `if`");
                    }
                    let t = self.than_value(&arm.body);
                    match &mut acc {
                        None => acc = Some(t),
                        Some(a) => self.unify_branch(a, &t, arm.span),
                    }
                }
                self.xet_het(&subj, arms, *span);
                acc.unwrap_or(Type::Unit)
            }
            HExpr::Lambda {
                params,
                ret,
                body,
                span,
            } => {
                let mut pts: Vec<Type> = Vec::new();
                for p in params {
                    let t = self.resolve_ann(&p.ty, p.span);
                    self.set_ty(p.sym, t.clone());
                    pts.push(t);
                }
                let saved = self.cur_ret.clone();
                self.cur_ret = None;
                let body_ty = match body {
                    crate::hir::HBody::Expr(e) => self.kiem_expr(e),
                    crate::hir::HBody::Stmts(st) => self.than_value(st),
                };
                self.cur_ret = saved;
                let declared = self.resolve_ann(ret, *span);
                let out = if declared.is_error() {
                    body_ty
                } else {
                    self.coerce(&body_ty, &declared, *span, "thân lambda");
                    declared
                };
                Type::Fn(pts.into_boxed_slice(), Box::new(out))
            }
            HExpr::Tuple { items, .. } => {
                let tys: Vec<Type> = items.iter().map(|i| self.kiem_expr(i)).collect();
                Type::TypeRef {
                    name: "tuple".into(),
                    args: tys,
                }
            }
            HExpr::Ref { expr, .. } => {
                let _ = self.kiem_expr(expr);
                Type::Error
            }
            HExpr::Await { expr, .. } => {
                let _ = self.kiem_expr(expr);
                Type::Error
            }
            HExpr::CmdBlock { .. } | HExpr::Error { .. } => Type::Error,
        }
    }

    pub(crate) fn show(&self, t: &Type) -> String {
        self.types.show(t)
    }

    /// Duyệt `node` của `if let`: binding thuộc **nhánh** `then` (đã đủ vì binding nằm trong HIR).
    pub(crate) fn kiem_cond(&mut self, cond: &HCond) {
        match cond {
            HCond::Expr(e) => {
                let t = self.kiem_expr(e);
                self.coerce(&t, &Type::Bool, e.span(), "điều kiện `if`");
            }
            HCond::Let { pat, value, .. } => {
                let vt = self.kiem_expr(value);
                let inner = match &vt {
                    Type::Opt(i) => (**i).clone(),
                    Type::Error => Type::Error,
                    other => {
                        // `if let` trên giá trị không optional: vẫn cho, coi như kiểu đó.
                        other.clone()
                    }
                };
                self.bind_pat(pat, &inner);
            }
        }
    }

    /// Kiểu chung của một dãy biểu thức (`list`/`set`).
    fn unify_all(&mut self, items: &[HExpr], what: &str, _span: vis_syntax::Span) -> Type {
        let mut acc = Type::Error;
        for (i, it) in items.iter().enumerate() {
            let t = self.kiem_expr(it);
            if i == 0 {
                acc = t;
            } else {
                self.coerce(&t, &acc, it.span(), what);
            }
        }
        acc
    }

    fn unify_branch(&mut self, acc: &mut Type, t: &Type, span: vis_syntax::Span) {
        if matches!(acc, Type::Error) {
            *acc = t.clone();
            return;
        }
        if matches!(t, Type::Error) || self.same(acc, t) {
            return;
        }
        if let Type::Opt(inner) = acc {
            if self.same(inner, t) {
                return;
            }
        }
        let msg = format!(
            "hai nhánh khác kiểu: `{}` và `{}`",
            self.show(acc),
            self.show(t)
        );
        self.diag(MISMATCH, msg, span);
    }

    fn kiem_bin(
        &mut self,
        op: BinOp,
        l: &Type,
        r: &Type,
        lspan: vis_syntax::Span,
        rspan: vis_syntax::Span,
        span: vis_syntax::Span,
    ) -> Type {
        match op {
            BinOp::Add
            | BinOp::Sub
            | BinOp::Mul
            | BinOp::Div
            | BinOp::Rem
            | BinOp::Pow
            | BinOp::Range
            | BinOp::RangeIncl => match num::so_hoc(op, l, r) {
                Some(t) => t,
                None => {
                    self.diag(
                        MISMATCH,
                        format!(
                            "toán tử `{}`: `{}` và `{}` không cùng kiểu/đơn vị",
                            op.text(),
                            self.show(l),
                            self.show(r)
                        ),
                        span,
                    );
                    Type::Error
                }
            },
            BinOp::Coalesce => {
                let inner = match l {
                    Type::Opt(i) => (**i).clone(),
                    Type::Error => return Type::Error,
                    other => {
                        self.diag(
                            MISMATCH,
                            format!("`??` cần vế trái kiểu `T?`, được `{}`", self.show(other)),
                            lspan,
                        );
                        return Type::Error;
                    }
                };
                self.coerce(r, &inner, rspan, "`??`");
                inner
            }
            BinOp::Eq | BinOp::Ne => {
                if matches!(l, Type::Fn(..)) || matches!(r, Type::Fn(..)) {
                    self.diag(MISMATCH, "không so sánh được hàm (VD-43 #4)", span);
                    return Type::Bool;
                }
                let (a, b) = num::promote(l, r);
                if !self.same(&a, &b) {
                    self.diag(
                        MISMATCH,
                        format!(
                            "so sánh khác kiểu: `{}` và `{}`",
                            self.show(l),
                            self.show(r)
                        ),
                        span,
                    );
                }
                Type::Bool
            }
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                let (a, b) = num::promote(l, r);
                if !self.same(&a, &b) || !num::ordered(&a) {
                    self.diag(
                        MISMATCH,
                        format!(
                            "so sánh thứ tự cần cùng kiểu sắp được: `{}` và `{}`",
                            self.show(l),
                            self.show(r)
                        ),
                        span,
                    );
                }
                Type::Bool
            }
            BinOp::And | BinOp::Or => {
                if !matches!(l, Type::Bool | Type::Error) || !matches!(r, Type::Bool | Type::Error)
                {
                    self.diag(
                        MISMATCH,
                        format!(
                            "`{}` cần `bool`, được `{}` và `{}`",
                            op.text(),
                            self.show(l),
                            self.show(r)
                        ),
                        span,
                    );
                }
                Type::Bool
            }
            BinOp::Pipe => match r {
                Type::Fn(params, ret) => {
                    if params.len() == 1 {
                        let p = params[0].clone();
                        self.coerce(l, &p, lspan, "`|>`");
                    } else if params.is_empty() {
                        // hàm không đối số: `x |> f()` — không kiểm thêm
                    } else {
                        self.diag(
                            ARG_COUNT,
                            format!("`|>` cần hàm **một** đối số, hàm này nhận {}", params.len()),
                            span,
                        );
                    }
                    (**ret).clone()
                }
                Type::Error => Type::Error,
                other => {
                    self.diag(
                        MISMATCH,
                        format!("`|>` cần hàm ở vế phải, được `{}`", self.show(other)),
                        span,
                    );
                    Type::Error
                }
            },
        }
    }

    fn kiem_call(
        &mut self,
        callee: &HExpr,
        args: &[HExpr],
        named: &[(String, HExpr)],
        span: vis_syntax::Span,
    ) -> Type {
        if let HExpr::Var {
            name,
            sym: None,
            span: cspan,
        } = callee
        {
            if let Some(t) = self.kiem_builtin(name, args, *cspan) {
                return t;
            }
        }
        // Hàm **khai trong tệp**: dùng chữ ký đầy đủ (có tên + mặc định).
        let sig = match callee {
            HExpr::Var { sym: Some(id), .. } => self.sig_of(*id).cloned(),
            _ => None,
        };
        if let Some(sig) = sig {
            self.kiem_args(&sig, args, named, span);
            return sig.ret;
        }
        let ct = self.kiem_expr(callee);
        let arg_tys: Vec<Type> = args.iter().map(|a| self.kiem_expr(a)).collect();
        let named_tys: Vec<(String, Type, vis_syntax::Span)> = named
            .iter()
            .map(|(n, a)| (n.clone(), self.kiem_expr(a), a.span()))
            .collect();
        match ct {
            Type::Fn(params, ret) => {
                let total = arg_tys.len() + named_tys.len();
                if total != params.len() {
                    self.diag(
                        ARG_COUNT,
                        format!("hàm cần {} đối số, được {total}", params.len()),
                        span,
                    );
                }
                for (i, a) in arg_tys.iter().enumerate() {
                    if let Some(p) = params.get(i) {
                        let p = p.clone();
                        self.coerce(a, &p, args[i].span(), "đối số");
                    }
                }
                for (n, t, sp) in &named_tys {
                    let _ = (n, t, sp);
                }
                (*ret).clone()
            }
            Type::Record(id) => {
                for (n, a) in named {
                    let at = self.kiem_expr(a);
                    match self.types.field(id, n) {
                        Some(f) => {
                            let want = f.ty.clone();
                            self.coerce(&at, &want, a.span(), "trường bản ghi");
                        }
                        None => {
                            self.diag(NO_FIELD, format!("bản ghi không có trường `{n}`"), a.span())
                        }
                    }
                }
                Type::Record(id)
            }
            Type::Error => Type::Error,
            other => {
                self.diag(
                    MISMATCH,
                    format!("`{}` không phải hàm", self.show(&other)),
                    span,
                );
                Type::Error
            }
        }
    }

    fn kiem_args(
        &mut self,
        sig: &FnSig,
        args: &[HExpr],
        named: &[(String, HExpr)],
        span: vis_syntax::Span,
    ) {
        let total = args.len() + named.len();
        let required = sig.params.iter().filter(|p| !p.2).count();
        if total > sig.params.len() || total < required {
            self.diag(
                ARG_COUNT,
                format!(
                    "hàm `{}` cần {} đối số (tối thiểu {required}), được {total}",
                    sig.name,
                    sig.params.len()
                ),
                span,
            );
        }
        let arg_tys: Vec<Type> = args.iter().map(|a| self.kiem_expr(a)).collect();
        for (i, t) in arg_tys.iter().enumerate() {
            if let Some(p) = sig.params.get(i) {
                let want = p.1.clone();
                self.coerce(t, &want, args[i].span(), "đối số");
            }
        }
        for (n, a) in named {
            let t = self.kiem_expr(a);
            match sig.params.iter().find(|p| &p.0 == n) {
                Some(p) => {
                    let want = p.1.clone();
                    self.coerce(&t, &want, a.span(), "đối số có tên");
                }
                None => self.diag(
                    ARG_COUNT,
                    format!("hàm `{}` không có tham số tên `{n}`", sig.name),
                    a.span(),
                ),
            }
        }
    }

    fn kiem_builtin(
        &mut self,
        name: &str,
        args: &[HExpr],
        _span: vis_syntax::Span,
    ) -> Option<Type> {
        // 8 tên dựng sẵn (VD-44 #3) — mỗi tên một chữ ký tối thiểu.
        let tys: Vec<Type> = args.iter().map(|a| self.kiem_expr(a)).collect();
        let arg0 = || tys.first().cloned().unwrap_or(Type::Error);
        Some(match name {
            "print" => Type::Unit,
            "len" => Type::Int,
            "str" => Type::Str,
            "Ok" => Type::Result(Box::new(arg0()), Box::new(Type::Error)),
            "Err" => Type::Result(Box::new(Type::Error), Box::new(arg0())),
            "Some" => Type::Opt(Box::new(arg0())),
            "None" => Type::Opt(Box::new(Type::Error)),
            "ref" => Type::Error,
            _ => return None,
        })
    }

    fn kiem_enum_lit(
        &mut self,
        enum_id: Option<EnumId>,
        variant: &str,
        args: &[HExpr],
        span: vis_syntax::Span,
    ) -> Type {
        let Some(id) = enum_id else {
            for a in args {
                let _ = self.kiem_expr(a);
            }
            return Type::Error;
        };
        let Some(def) = self.types.enum_def(id) else {
            return Type::Error;
        };
        let fields = def
            .variants
            .iter()
            .find(|v| v.name == variant)
            .map(|v| v.fields.clone());
        let Some(fields) = fields else {
            self.diag(
                NO_FIELD,
                format!("enum `{}` không có biến thể `{variant}`", def.name),
                span,
            );
            return Type::Error;
        };
        if args.len() != fields.len() {
            self.diag(
                ARG_COUNT,
                format!(
                    "biến thể `{variant}` cần {} đối số, được {}",
                    fields.len(),
                    args.len()
                ),
                span,
            );
        }
        for (i, a) in args.iter().enumerate() {
            let t = self.kiem_expr(a);
            if let Some(f) = fields.get(i) {
                let want = f.ty.clone();
                self.coerce(&t, &want, a.span(), "đối số biến thể");
            }
        }
        Type::Enum(id)
    }

    fn kiem_field(
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
                        NO_FIELD,
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
                    self.diag(NO_FIELD, format!("enum không có biến thể `{name}`"), span);
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
                            self.diag(NO_FIELD, format!("bản ghi không có trường `{name}`"), span);
                            Type::Error
                        }
                    },
                    Type::Error => Type::Error,
                    other => {
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
                self.diag(
                    NO_FIELD,
                    format!("`{}` không có trường `{name}`", self.show(other)),
                    span,
                );
                Type::Error
            }
        }
    }

    fn kiem_index(&mut self, base: &HExpr, index: &HExpr, _span: vis_syntax::Span) -> Type {
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

/// Kiểu của 8 tên giá trị dựng sẵn (không phải hàm) — `None`/`Some`… không có kiểu riêng ở đây.
fn builtin_value_ty(name: &str) -> Type {
    match name {
        "None" => Type::Opt(Box::new(Type::Error)),
        _ => Type::Error,
    }
}

/// Chữ ký một hàm **khai trong tệp** (giữ tên tham số + có giá trị mặc định hay không).
#[derive(Debug, Clone)]
pub(crate) struct FnSig {
    pub name: String,
    /// `(tên, kiểu, có_mặc_định)`
    pub params: Vec<(String, Type, bool)>,
    pub ret: Type,
}

/// Giữ chỗ: `HPat`/`HStmt` dùng ở module khác (tránh import thừa).
#[allow(dead_code)]
fn _giu(_: &HPat, _: &HStmt) {}
