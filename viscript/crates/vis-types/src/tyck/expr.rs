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
                let (l_lit, r_lit) = (super::is_int_lit(lhs), super::is_int_lit(rhs));
                self.kiem_bin(
                    *op,
                    (&l, &r),
                    (l_lit, r_lit),
                    (lhs.span(), rhs.span()),
                    *span,
                )
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
                        self.coerce_e(k, &kt2, &kt, "khoá `map`");
                        self.coerce_e(v, &vt2, &vt, "giá trị `map`");
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
                    Type::Opt(inner) => {
                        // ST-12b BLOCKER #1 (§4.4): `T?` chỉ dùng `?` khi hàm trả `U?`.
                        if self.try_hop_le(true) {
                            (*inner).clone()
                        } else {
                            self.diag(
                                MISMATCH,
                                format!(
                                    "`?` trên `{}` chỉ dùng được khi hàm trả `{}?`, hàm này trả `{}` (§4.4)",
                                    self.show(&Type::Opt(inner.clone())),
                                    self.show(&inner),
                                    self.show(&self.ham_tra())
                                ),
                                *span,
                            );
                            Type::Error
                        }
                    }
                    Type::Result(inner, _) => {
                        if self.try_hop_le(false) {
                            (*inner).clone()
                        } else {
                            self.diag(
                                MISMATCH,
                                format!(
                                    "`?` trên `Result` chỉ dùng được khi hàm trả `Result[…, …]`, hàm này trả `{}` (§4.4)",
                                    self.show(&self.ham_tra())
                                ),
                                *span,
                            );
                            Type::Error
                        }
                    }
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
                                    self.coerce_e(v, &vt, &want, "`with` cập nhật trường");
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
            // ST-14: chuỗi nội suy — kiểm **từng** phần biểu thức, kiểu của cả chuỗi là `str`.
            HExpr::Interp { parts, .. } => {
                for p in parts {
                    if let crate::hir::InterpPart::Expr(e) = p {
                        self.kiem_expr(e);
                    }
                }
                Type::Str
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
                self.coerce_e(it, &t, &acc, what);
            }
        }
        acc
    }

    pub(crate) fn unify_branch(&mut self, acc: &mut Type, t: &Type, span: vis_syntax::Span) {
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
        (l, r): (&Type, &Type),
        (l_lit, r_lit): (bool, bool),
        (lspan, rspan): (vis_syntax::Span, vis_syntax::Span),
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
            | BinOp::RangeIncl => match num::so_hoc(op, l, r, l_lit, r_lit) {
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
                self.coerce_lit(r, &inner, rspan, "`??`", r_lit);
                inner
            }
            BinOp::Eq | BinOp::Ne => {
                if matches!(l, Type::Fn(..)) || matches!(r, Type::Fn(..)) {
                    self.diag(MISMATCH, "không so sánh được hàm (VD-43 #4)", span);
                    return Type::Bool;
                }
                if is_percent_float(l, r) {
                    return Type::Bool;
                }
                let (a, b) = num::promote(l, r, l_lit, r_lit);
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
                // `{error}` lan truyền im lặng (lambda chưa có kiểu tham số ở VS0).
                if l.is_error() || r.is_error() {
                    return Type::Bool;
                }
                // ST-12b (MAJOR #6): `percent` so sánh `float` **theo tỉ lệ** (§3.1/§3.6).
                if is_percent_float(l, r) {
                    return Type::Bool;
                }
                let (a, b) = num::promote(l, r, l_lit, r_lit);
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
        // Hàm dựng sẵn: kiểm **chữ ký** (ST-12b BLOCKER #3) — cả khi `sym = None` lẫn khi `sym`
        // trỏ tới ký hiệu `SymKind::Builtin` (ST-11 khai 8 tên dựng sẵn).
        if let HExpr::Var { name, sym, .. } = callee {
            // ST-26: `std.<module>.<fn>` — kiểm chữ ký theo bảng `vis_types::stdlib`.
            if let Some(t) = self.kiem_std(name, args, span) {
                return t;
            }
            let la_builtin = match sym {
                None => builtin_is_value(name),
                Some(id) => self.symbols.kind_of(*id) == Some(crate::lower::SymKind::Builtin),
            };
            // ST-50: **lệnh shell** dựng bởi bộ hạ cũng kiểm chữ ký (`where`/`each`/`select`/
            // `get`/`sort_by` không nằm trong 8 tên giá trị dựng sẵn nhưng là Call có tên).
            let la_lenh = matches!(
                name.as_str(),
                "where" | "each" | "select" | "get" | "sort_by"
            );
            if la_builtin || la_lenh {
                if let Some(t) = self.kiem_builtin(name, args, span) {
                    return t;
                }
            }
        }
        // 0) Phương thức lõi của list/str/map (ST-13, VD-47).
        if let HExpr::Field {
            base,
            name,
            optional: false,
            ..
        } = callee
        {
            let recv = self.kiem_expr(base);
            if let Some(t) = self.kiem_method(&recv, name, args, span) {
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
            if sig.generics.is_empty() {
                return sig.ret;
            }
            // ST-28: **hợp nhất** kiểu khai báo (còn `T`) với kiểu đối số thật ⇒ suy `T`; mâu thuẫn
            // ⇒ `VS0412`. Kiểu trả sau khi thay `T` ⇒ giữ được kiểm tĩnh ở ngữ cảnh
            // (`let n: int = lay("x")` ⇒ `VS0412`).
            let arg_tys: Vec<Type> = args.iter().map(|a| self.kiem_expr(a)).collect();
            let mut bang: Vec<(String, Type)> = Vec::new();
            for (i, t) in arg_tys.iter().enumerate() {
                if let Some(raw) = sig.raw_params.get(i) {
                    let raw = raw.clone();
                    self.hop_nhat(&raw, t, &sig.generics, &mut bang, &sig.name, args[i].span());
                }
            }
            return crate::tyck::thay_gen(&sig.ret_raw(), &bang);
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
                        self.coerce_e(&args[i], a, &p, "đối số");
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
        let required = sig.params.iter().filter(|p| !p.2 && !sig.variadic).count();
        let toi_da = if sig.variadic {
            usize::MAX
        } else {
            sig.params.len()
        };
        if total > toi_da || total < required {
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
                // ST-27: ô `...rest` — mọi đối số từ đây trở đi là **phần tử**, không phải list.
                let want = match (&p.1, sig.variadic && i + 1 == sig.params.len()) {
                    (Type::List(elem), true) => (**elem).clone(),
                    _ => p.1.clone(),
                };
                self.coerce_e(&args[i], t, &want, "đối số");
            } else if sig.variadic {
                // ST-27: đối số **thừa** rơi vào `...rest` — kiểm theo **kiểu phần tử** của nó.
                if let Some(p) = sig.params.last() {
                    if let Type::List(elem) = &p.1 {
                        let want = (**elem).clone();
                        self.coerce_e(&args[i], t, &want, "đối số");
                    }
                }
            }
        }
        let mut da_gap: Vec<&str> = Vec::new();
        for (n, a) in named {
            let t = self.kiem_expr(a);
            match sig.params.iter().find(|p| &p.0 == n) {
                Some(p) if p.3 => {
                    // ST-12b MAJOR #5: `--tên` chỉ dùng cho tham số **khai `--`**, và không lặp.
                    if da_gap.contains(&n.as_str()) {
                        self.diag(ARG_COUNT, format!("đối số tên `--{n}` bị lặp"), a.span());
                    }
                    da_gap.push(n);
                    let want = p.1.clone();
                    self.coerce_e(a, &t, &want, "đối số có tên");
                }
                Some(_) => self.diag(
                    ARG_COUNT,
                    format!(
                        "`{n}` là tham số **vị trí** của `{}` — không truyền bằng `--{n}` (§A.3)",
                        sig.name
                    ),
                    a.span(),
                ),
                None => self.diag(
                    ARG_COUNT,
                    format!("hàm `{}` không có tham số tên `{n}`", sig.name),
                    a.span(),
                ),
            }
        }
    }

    /// Phương thức lõi trên `list`/`str`/`map` (ST-13): trả `None` nếu `recv` không phải kiểu có
    /// phương thức (để nhánh bản ghi/enum xử lý). Phương thức lạ ⇒ `VS0403` (tyck chặn trước khi chạy).
    fn kiem_method(
        &mut self,
        recv: &Type,
        name: &str,
        args: &[HExpr],
        span: vis_syntax::Span,
    ) -> Option<Type> {
        let arg_tys: Vec<Type> = args.iter().map(|a| self.kiem_expr(a)).collect();
        let elem = |t: &Type| match t {
            Type::List(e) => (**e).clone(),
            _ => Type::Error,
        };
        let out = match recv {
            Type::List(_) => {
                let e = elem(recv);
                match name {
                    "len" => {
                        self.arity(args, 0, name, span);
                        Type::Int
                    }
                    "get" => {
                        self.arity(args, 1, name, span);
                        self.arg_ty(&arg_tys, 0, &Type::Int, name, args, span);
                        Type::Opt(Box::new(e))
                    }
                    "first" | "last" => {
                        self.arity(args, 0, name, span);
                        Type::Opt(Box::new(e))
                    }
                    "push" => {
                        self.arity(args, 1, name, span);
                        self.arg_ty(&arg_tys, 0, &e, name, args, span);
                        Type::List(Box::new(e))
                    }
                    "map" => {
                        self.arity(args, 1, name, span);
                        let f = arg_tys.first().cloned().unwrap_or(Type::Error);
                        match f {
                            Type::Fn(_, ret) => Type::List(ret),
                            Type::Error => Type::List(Box::new(Type::Error)),
                            other => {
                                self.diag(
                                    MISMATCH,
                                    format!("`map` cần hàm, được `{}`", self.show(&other)),
                                    span,
                                );
                                Type::List(Box::new(Type::Error))
                            }
                        }
                    }
                    "filter" => {
                        self.arity(args, 1, name, span);
                        // ST-13b (mục 6): `filter` phải nhận **hàm 1 tham số** trả `bool`.
                        match arg_tys.first() {
                            Some(Type::Fn(params, ret)) => {
                                if params.len() != 1 {
                                    self.diag(MISMATCH, "`filter` cần hàm **1 tham số**", span);
                                }
                                if !matches!(**ret, Type::Bool | Type::Error) {
                                    self.diag(MISMATCH, "`filter` cần hàm trả `bool`", span);
                                }
                            }
                            Some(Type::Error) | None => {}
                            Some(other) => {
                                let o = other.clone();
                                self.diag(
                                    MISMATCH,
                                    format!("`filter` cần **hàm**, được `{}`", self.show(&o)),
                                    span,
                                );
                            }
                        }
                        Type::List(Box::new(e))
                    }
                    "each" => {
                        self.arity(args, 1, name, span);
                        // ST-13b (mục 6): `each` phải nhận **hàm 1 tham số**.
                        match arg_tys.first() {
                            Some(Type::Fn(params, _)) => {
                                if params.len() != 1 {
                                    let n = params.len();
                                    self.diag(
                                        MISMATCH,
                                        format!("`each` cần hàm **1 tham số**, được {n}"),
                                        span,
                                    );
                                }
                            }
                            Some(Type::Error) | None => {}
                            Some(other) => {
                                let o = other.clone();
                                self.diag(
                                    MISMATCH,
                                    format!("`each` cần **hàm**, được `{}`", self.show(&o)),
                                    span,
                                );
                            }
                        }
                        Type::Unit
                    }
                    "sum" => {
                        self.arity(args, 0, name, span);
                        // ST-13b (mục 6): `sum` chỉ trên list **số**.
                        if !matches!(e, Type::Int | Type::Float | Type::Error) {
                            let t = e.clone();
                            self.diag(
                                MISMATCH,
                                format!(
                                    "`sum` chỉ dùng cho list số, phần tử là `{}`",
                                    self.show(&t)
                                ),
                                span,
                            );
                            return Some(Type::Error);
                        }
                        e
                    }
                    "join" => {
                        self.arity(args, 1, name, span);
                        self.arg_ty(&arg_tys, 0, &Type::Str, name, args, span);
                        Type::Str
                    }
                    _ => {
                        self.diag(
                            NO_FIELD,
                            format!(
                                "`list` không có phương thức `{name}` (ST-13: len get first last push map filter each sum join)"
                            ),
                            span,
                        );
                        Type::Error
                    }
                }
            }
            Type::Str => match name {
                "len" => {
                    self.arity(args, 0, name, span);
                    Type::Int
                }
                "upper" | "lower" => {
                    self.arity(args, 0, name, span);
                    Type::Str
                }
                "chars" => {
                    self.arity(args, 0, name, span);
                    Type::List(Box::new(Type::Str))
                }
                "contains" => {
                    self.arity(args, 1, name, span);
                    self.arg_ty(&arg_tys, 0, &Type::Str, name, args, span);
                    Type::Bool
                }
                "split" => {
                    self.arity(args, 1, name, span);
                    self.arg_ty(&arg_tys, 0, &Type::Str, name, args, span);
                    Type::List(Box::new(Type::Str))
                }
                _ => {
                    self.diag(
                        NO_FIELD,
                        format!("`str` không có phương thức `{name}` (ST-13: len upper lower split chars contains)"),
                        span,
                    );
                    Type::Error
                }
            },
            Type::Map(k, v) => {
                let kk = (**k).clone();
                let vv = (**v).clone();
                match name {
                    "len" => {
                        self.arity(args, 0, name, span);
                        Type::Int
                    }
                    "get" => {
                        self.arity(args, 1, name, span);
                        self.arg_ty(&arg_tys, 0, &kk, name, args, span);
                        Type::Opt(Box::new(vv))
                    }
                    "has" => {
                        self.arity(args, 1, name, span);
                        self.arg_ty(&arg_tys, 0, &kk, name, args, span);
                        Type::Bool
                    }
                    "keys" => {
                        self.arity(args, 0, name, span);
                        Type::List(Box::new(kk))
                    }
                    "values" => {
                        self.arity(args, 0, name, span);
                        Type::List(Box::new(vv))
                    }
                    _ => {
                        self.diag(
                            NO_FIELD,
                            format!("`map` không có phương thức `{name}` (ST-13: len get has keys values)"),
                            span,
                        );
                        Type::Error
                    }
                }
            }
            _ => return None,
        };
        Some(out)
    }

    fn arity(&mut self, args: &[HExpr], want: usize, name: &str, span: vis_syntax::Span) {
        if args.len() != want {
            self.diag(
                ARG_COUNT,
                format!("`{name}` cần {want} đối số, được {}", args.len()),
                span,
            );
        }
    }

    fn arg_ty(
        &mut self,
        got: &[Type],
        i: usize,
        want: &Type,
        name: &str,
        args: &[HExpr],
        span: vis_syntax::Span,
    ) {
        if let Some(g) = got.get(i) {
            let g = g.clone();
            let w = want.clone();
            let at = args.get(i).map(|a| a.span()).unwrap_or(span);
            self.coerce(&g, &w, at, &format!("đối số `{name}`"));
        }
    }

    /// `?` có hợp lệ không: hàm đang kiểm phải trả `U?` (với `T?`) hoặc `Result[…, …]` (với
    /// `Result`); ngoài hàm (hoặc trong lambda) `cur_ret = None` ⇒ **cho phép** (không có ngữ cảnh).
    fn try_hop_le(&self, opt: bool) -> bool {
        match &self.cur_ret {
            None => true,
            Some(Type::Error) => true,
            Some(Type::Opt(_)) => opt,
            Some(Type::Result(..)) => !opt,
            Some(_) => false,
        }
    }

    fn ham_tra(&self) -> Type {
        self.cur_ret.clone().unwrap_or(Type::Unit)
    }

    /// Chữ ký `std.*` (ST-26): `None` nếu `name` không phải `std.*`.
    fn kiem_std(&mut self, name: &str, args: &[HExpr], span: vis_syntax::Span) -> Option<Type> {
        let ngan = name.strip_prefix("std.")?;
        let sig = crate::stdlib::sig_of(ngan)?;
        let tys: Vec<Type> = args.iter().map(|a| self.kiem_expr(a)).collect();
        if tys.len() != sig.so_doi() {
            self.diag(
                ARG_COUNT,
                format!("`{ngan}` cần {} đối số, được {}", sig.so_doi(), tys.len()),
                span,
            );
        }
        for (i, (t, mong)) in tys.iter().zip(sig.params.iter()).enumerate() {
            let Some(mong) = mong.kieu() else { continue };
            if !matches!(t, Type::Error) && *t != mong {
                self.diag(
                    MISMATCH,
                    format!(
                        "`{ngan}`: đối số {} cần `{}`, được `{}`",
                        i + 1,
                        mong.name(),
                        t.name()
                    ),
                    span,
                );
            }
        }
        sig.ret.kieu().or(Some(Type::Error))
    }

    /// Chữ ký 8 tên dựng sẵn (VD-44 #3): đối số đúng số lượng + đúng kiểu.
    fn kiem_builtin(&mut self, name: &str, args: &[HExpr], span: vis_syntax::Span) -> Option<Type> {
        // ST-42: `ls` ⇒ `list[FileEntry]` (tra bản ghi **theo tên** trong bảng kiểu); `open` ⇒ `str`.
        if name == "ls" || name == "open" {
            let mut tys: Vec<Type> = Vec::new();
            for a in args {
                tys.push(self.kiem_expr(a));
            }
            if name == "open" {
                if tys.len() != 1 {
                    self.diag(
                        ARG_COUNT,
                        format!("`open` cần 1 đối số, được {}", tys.len()),
                        span,
                    );
                }
                return Some(Type::Str);
            }
            if tys.len() > 3 {
                self.diag(
                    ARG_COUNT,
                    format!("`ls` cần 0–3 đối số, được {}", tys.len()),
                    span,
                );
            }
            let el = match self.types.rec_by_name("FileEntry") {
                Some(id) => Type::Record(id),
                None => Type::Error,
            };
            return Some(Type::List(Box::new(el)));
        }
        // ST-40b: `get`/`select` — kiểu ra = `list[<kiểu thân lambda>]` (một lần kiểm thân).
        if name == "get" || name == "select" {
            let xs_ty = args
                .first()
                .map(|a| self.kiem_expr(a))
                .unwrap_or(Type::Error);
            let mut body_ty = Type::Error;
            if let Some(f) = args.get(1) {
                body_ty = match f {
                    HExpr::Lambda { body, .. } => match body {
                        crate::hir::HBody::Expr(e) => self.kiem_expr(e),
                        crate::hir::HBody::Stmts(_) => Type::Unit,
                    },
                    other => self.kiem_expr(other),
                };
            }
            if args.len() != 2 {
                self.diag(
                    ARG_COUNT,
                    format!("`{name}` cần 2 đối số, được {}", args.len()),
                    span,
                );
            }
            let _ = xs_ty;
            return Some(Type::List(Box::new(body_ty)));
        }
        // ST-39: `sort_by` (chế độ shell) — kiểm **khoá** **một lần** trên thân lambda (không kiểm lại
        // qua `tys`), từ chối khoá không so được (`bool`, `map`, `record`, `list`) ⇒ `VS0412`.
        // Runtime có chốt thứ hai (`VS0501`). **Chỉ** khi có ≥ 2 đối số: pipeline `.vis` cũ dùng
        // `sort_by` như **chương trình ngoài** 1 đối số phải giữ nguyên hành vi (không báo arity).
        if name == "sort_by" && args.len() >= 2 {
            let xs_ty = args
                .first()
                .map(|a| self.kiem_expr(a))
                .unwrap_or(Type::Error);
            if let Some(f) = args.get(1) {
                let kt = match f {
                    HExpr::Lambda { body, .. } => match body {
                        crate::hir::HBody::Expr(e) => self.kiem_expr(e),
                        crate::hir::HBody::Stmts(_) => Type::Unit,
                    },
                    other => self.kiem_expr(other),
                };
                let khong_so = matches!(&kt, Type::Bool | Type::Unit)
                    || matches!(&kt, Type::Map(..) | Type::Record(..) | Type::List(..));
                if khong_so {
                    self.diag(
                        super::MISMATCH,
                        format!("`sort_by`: khoá không so được: `{}`", self.show(&kt)),
                        span,
                    );
                }
            }
            if let Some(d) = args.get(2) {
                let dt = self.kiem_expr(d);
                if !matches!(dt, Type::Bool | Type::Error) {
                    self.diag(
                        super::MISMATCH,
                        format!("`sort_by`: `--desc` cần `bool`, được `{}`", self.show(&dt)),
                        span,
                    );
                }
            }
            if args.len() > 3 {
                self.diag(
                    ARG_COUNT,
                    format!("`sort_by` cần 2–3 đối số, được {}", args.len()),
                    span,
                );
            }
            return Some(match xs_ty {
                Type::List(e) => Type::List(e),
                other => Type::List(Box::new(other)),
            });
        }
        // ST-50: `where` — kiểm **thân vị từ** ra `bool` **kể cả** lambda trong ngoặc
        // (`where ( x => 1 )` ⇒ `VS0412`); `{error}` ⇒ kiểm động (không đỏ oan).
        if name == "where" && args.len() >= 2 {
            let xs_ty = args
                .first()
                .map(|a| self.kiem_expr(a))
                .unwrap_or(Type::Error);
            let el = match xs_ty {
                Type::List(e) => *e,
                Type::Error => Type::Error,
                _ => Type::Error,
            };
            let kt = match &args[1] {
                HExpr::Lambda { body, params, .. } => {
                    for p in params {
                        let t = self.resolve_ann(&p.ty, p.span);
                        self.set_ty(p.sym, t);
                    }
                    match body {
                        crate::hir::HBody::Expr(e) => self.kiem_expr(e),
                        crate::hir::HBody::Stmts(_) => Type::Unit,
                    }
                }
                f => self.kiem_expr(f),
            };
            if !matches!(kt, Type::Bool | Type::Error | Type::Fn(..)) {
                self.diag(
                    super::MISMATCH,
                    format!("`where`: thân vị từ phải `bool`, được `{}`", self.show(&kt)),
                    args[1].span(),
                );
            }
            if args.len() != 2 {
                self.diag(
                    ARG_COUNT,
                    format!("`where` cần 2 đối số, được {}", args.len()),
                    span,
                );
            }
            return Some(Type::List(Box::new(el)));
        }
        let tys: Vec<Type> = args.iter().map(|a| self.kiem_expr(a)).collect();
        let arity = |n: usize| args.len() == n;
        let sai_so = format!("`{name}` cần {} đối số, được {}", 0, args.len());
        let _ = sai_so;
        let t0 = || tys.first().cloned().unwrap_or(Type::Error);
        match name {
            "print" => {
                if !arity(1) {
                    self.diag(
                        ARG_COUNT,
                        format!("`print` cần 1 đối số, được {}", args.len()),
                        span,
                    );
                }
                Some(Type::Unit)
            }
            "str" => {
                if !arity(1) {
                    self.diag(
                        ARG_COUNT,
                        format!("`str` cần 1 đối số, được {}", args.len()),
                        span,
                    );
                }
                Some(Type::Str)
            }
            // ST-18: 3 lệnh builtin chế độ lệnh — chữ ký như `sig_builtin()` ở `lower/cmd.rs`.
            "echo" => {
                if tys.len() > 4 {
                    self.diag(
                        ARG_COUNT,
                        format!(
                            "`echo` nhận tối đa 4 đối số (a, b, c, --sep), được {}",
                            tys.len()
                        ),
                        span,
                    );
                }
                Some(Type::Str)
            }
            "lines" => {
                if !arity(1) {
                    self.diag(ARG_COUNT, "`lines` cần 1 đối số", span);
                }
                Some(Type::List(Box::new(Type::Str)))
            }
            "take" => {
                if !arity(2) {
                    self.diag(ARG_COUNT, "`take` cần 2 đối số", span);
                } else {
                    if let Some(t) = tys.first() {
                        if !matches!(t, Type::List(_) | Type::Error) {
                            self.diag(MISMATCH, "`take` cần `list[str]` ở đối số 1", span);
                        }
                    }
                    if let Some(t) = tys.get(1) {
                        if !matches!(t, Type::Int | Type::Error) {
                            self.diag(MISMATCH, "`take` cần `int` ở đối số 2", span);
                        }
                    }
                }
                Some(Type::List(Box::new(Type::Str)))
            }
            "len" => {
                if !arity(1) {
                    self.diag(
                        ARG_COUNT,
                        format!("`len` cần 1 đối số, được {}", args.len()),
                        span,
                    );
                } else if let Some(t) = tys.first() {
                    let ok = matches!(
                        t,
                        Type::Str | Type::List(_) | Type::Map(..) | Type::Bytes | Type::Error
                    );
                    if !ok {
                        let t = t.clone();
                        self.diag(
                            MISMATCH,
                            format!(
                                "`len` không nhận `{}` (chỉ `str`/`list`/`map`/`bytes`)",
                                self.show(&t)
                            ),
                            span,
                        );
                    }
                }
                Some(Type::Int)
            }
            "Some" => {
                if !arity(1) {
                    self.diag(ARG_COUNT, "`Some` cần 1 đối số", span);
                }
                Some(Type::Opt(Box::new(t0())))
            }
            "None" => {
                if !args.is_empty() {
                    self.diag(ARG_COUNT, "`None` không có đối số", span);
                }
                Some(Type::Opt(Box::new(Type::Error)))
            }
            "Ok" => {
                if !arity(1) {
                    self.diag(ARG_COUNT, "`Ok` cần 1 đối số", span);
                }
                Some(Type::Result(Box::new(t0()), Box::new(Type::Error)))
            }
            "Err" => {
                if !arity(1) {
                    self.diag(ARG_COUNT, "`Err` cần 1 đối số", span);
                }
                Some(Type::Result(Box::new(Type::Error), Box::new(t0())))
            }
            "ref" => {
                if !arity(1) {
                    self.diag(ARG_COUNT, "`ref` cần 1 đối số", span);
                }
                Some(Type::Error)
            }
            _ => None,
        }
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
}

impl FnSig {
    /// Kiểu trả đã suy (dùng sau `hop_nhat`).
    pub(crate) fn ret_raw(&self) -> Type {
        self.raw_ret.clone()
    }
}

/// Giữ chỗ: `HPat`/`HStmt` dùng ở module khác (tránh import thừa).
#[allow(dead_code)]
fn _giu(_: &HPat, _: &HStmt) {}

/// `percent` và `float` so sánh được **theo tỉ lệ** (§3.1/§3.6) — đặc cách duy nhất.
fn is_percent_float(a: &Type, b: &Type) -> bool {
    matches!(
        (a, b),
        (Type::Percent, Type::Float) | (Type::Float, Type::Percent)
    )
}

/// 8 tên dựng sẵn (VD-44 #3) — dùng khi `sym = None`.
fn builtin_is_value(name: &str) -> bool {
    matches!(
        name,
        "print"
            | "len"
            | "str"
            | "Ok"
            | "Err"
            | "Some"
            | "None"
            | "ref"
            | "sort_by"
            | "get"
            | "select"
    )
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
    /// `(tên, kiểu, có_mặc_định, khai `--`)` — kiểu đã **xoá generic** (ST-28), dùng cho thân hàm.
    pub params: Vec<(String, Type, bool, bool)>,
    pub ret: Type,
    /// Tham số cuối là `...rest` ⇒ nhận **thêm** đối số vị trí (ST-27).
    pub variadic: bool,
    /// Tham số kiểu của hàm (`fn f[T]`) — ST-28.
    pub generics: Vec<String>,
    /// Kiểu tham số **nguyên bản** (còn `TypeRef{"T"}`) để **hợp nhất** ở call-site.
    pub raw_params: Vec<Type>,
    /// Kiểu trả **nguyên bản** (còn `T`) — thay bằng kiểu đã suy sau khi hợp nhất.
    pub raw_ret: Type,
}
