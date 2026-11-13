// SPDX-License-Identifier: MIT OR Apache-2.0
//! Hạ **biểu thức** CST → [`HExpr`] (ST-11) — phần đuôi của [`crate::lower`].
//!
//! Không kiểm kiểu: chỉ *điền* kiểu cho literal (VD-43), dựng mặt nạ bản ghi ẩn danh cho record
//! literal và nhận ra enum literal khi tên biến thể đã khai trong tệp. Mọi thứ khác giữ
//! `sym = None` / `Type::Error` và **không** sinh chẩn đoán (VD-44 #1).

use vis_syntax::{lex, parse, Cst, Elem, Kind, LexOutput, Mode, Node, NodeKind, Span};

use super::{bin_op, lit_kind, un_op, Lowerer};
use crate::hir::{BinOp, HBody, HCond, HExpr, HParam, InterpPart, Lit, LitKind};
use crate::stdlib as vis_types_stdlib;
use crate::ty::{FieldDef, RecDef, RecId, Type};
use vis_diag::{NAME_CODES, TYPE_CODES};

impl<'a> Lowerer<'a> {
    /// Hạ một biểu thức; không bao giờ panic — node lạ ⇒ [`HExpr::Error`].
    pub(crate) fn lower_expr(&mut self, node: &Node) -> HExpr {
        let span = node.span;
        match node.kind {
            NodeKind::Literal => self.lower_literal(node),
            NodeKind::Ident => {
                let name = self.text_of(span).to_string();
                self.lower_var_use(&name, span)
            }
            NodeKind::BinOp => {
                let mut lhs: Option<HExpr> = None;
                let mut rhs: Option<HExpr> = None;
                let mut op_text = String::new();
                for e in &node.elems {
                    match e {
                        Elem::Node(n) => {
                            if lhs.is_none() {
                                lhs = Some(self.lower_expr(n));
                            } else {
                                rhs = Some(self.lower_expr(n));
                            }
                        }
                        Elem::Token(i) => {
                            if let Some(t) = self.tok(*i) {
                                if !op_text.is_empty() {
                                    op_text.clear();
                                }
                                op_text.push_str(&t.text);
                            }
                        }
                    }
                }
                let op = bin_op(&op_text).unwrap_or(BinOp::Add);
                HExpr::Bin {
                    op,
                    lhs: Box::new(lhs.unwrap_or(HExpr::Error { span })),
                    rhs: Box::new(rhs.unwrap_or(HExpr::Error { span })),
                    span,
                }
            }
            NodeKind::UnOp => {
                let op_text = node
                    .tokens(&self.cst.tokens)
                    .next()
                    .map(|t| t.text.clone())
                    .unwrap_or_default();
                let op = un_op(&op_text).unwrap_or(crate::hir::UnOp::Neg);
                let expr = node
                    .children()
                    .next()
                    .map(|c| self.lower_expr(c))
                    .unwrap_or(HExpr::Error { span });
                HExpr::Un {
                    op,
                    expr: Box::new(expr),
                    span,
                }
            }
            // `(expr)` — HIR không có nút `paren` (xem quy ước sexp ở `hir.rs`).
            NodeKind::Paren => node
                .children()
                .next()
                .map(|c| self.lower_expr(c))
                .unwrap_or(HExpr::Error { span }),
            NodeKind::Tuple => {
                let items: Vec<HExpr> = node.children().map(|c| self.lower_expr(c)).collect();
                let ty = if items.is_empty() {
                    Type::Unit
                } else {
                    Type::TypeRef {
                        name: "tuple".into(),
                        args: items.iter().map(|i| self.type_of(i)).collect(),
                    }
                };
                HExpr::Tuple { items, ty, span }
            }
            NodeKind::ListLit => {
                let items: Vec<HExpr> = node.children().map(|c| self.lower_expr(c)).collect();
                let elem = first_known(items.iter().map(|i| self.type_of(i)));
                HExpr::List { items, elem, span }
            }
            NodeKind::SetLit => {
                let items: Vec<HExpr> = node.children().map(|c| self.lower_expr(c)).collect();
                let elem = first_known(items.iter().map(|i| self.type_of(i)));
                HExpr::Set { items, elem, span }
            }
            NodeKind::MapLit => {
                let mut entries = Vec::new();
                for e in node.children().filter(|n| n.kind == NodeKind::MapEntry) {
                    let mut it = e.children();
                    let k = it
                        .next()
                        .map(|n| self.lower_expr(n))
                        .unwrap_or(HExpr::Error { span: e.span });
                    let v = it
                        .next()
                        .map(|n| self.lower_expr(n))
                        .unwrap_or(HExpr::Error { span: e.span });
                    entries.push((k, v));
                }
                let key = first_known(entries.iter().map(|(k, _)| self.type_of(k)));
                let value = first_known(entries.iter().map(|(_, v)| self.type_of(v)));
                HExpr::Map {
                    entries,
                    key,
                    value,
                    span,
                }
            }
            NodeKind::RecordLit => {
                let mut fields: Vec<(String, HExpr)> = Vec::new();
                let mut defs: Vec<FieldDef> = Vec::new();
                for f in node.children().filter(|n| n.kind == NodeKind::RecordField) {
                    let name = self.decl_name(f).unwrap_or_default();
                    let value = match f.children().next() {
                        Some(v) => self.lower_expr(v),
                        // `{ ten }` — viết tắt `ten: ten`.
                        None => {
                            let n2 = name.clone();
                            self.lower_var_use(&n2, f.span)
                        }
                    };
                    defs.push(FieldDef {
                        name: name.clone(),
                        ty: self.type_of(&value),
                        span: f.span,
                        has_default: false,
                    });
                    fields.push((name, value));
                }
                let id = RecId(self.tbl.recs().len() as u32);
                self.tbl.push_rec(RecDef {
                    name: None,
                    fields: defs,
                    span,
                    anonymous: true,
                });
                HExpr::Record {
                    fields,
                    ty: Type::Record(id),
                    span,
                }
            }
            NodeKind::Field | NodeKind::OptField => {
                let base = node
                    .children()
                    .next()
                    .map(|c| self.lower_expr(c))
                    .unwrap_or(HExpr::Error { span });
                let name = node
                    .elems
                    .iter()
                    .rev()
                    .find_map(|e| match e {
                        Elem::Token(i) => self.tok(*i).map(|t| t.text.clone()),
                        Elem::Node(_) => None,
                    })
                    .unwrap_or_default();
                HExpr::Field {
                    base: Box::new(base),
                    name,
                    optional: node.kind == NodeKind::OptField,
                    span,
                }
            }
            NodeKind::Call | NodeKind::GenericCall => {
                let callee = node
                    .children()
                    .next()
                    .map(|c| self.lower_expr(c))
                    .unwrap_or(HExpr::Error { span });
                let (args, named) = node
                    .child(NodeKind::Args)
                    .map(|a| self.lower_args(a))
                    .unwrap_or_default();
                // ST-26: `str.split(…)`, `list.sort(…)` — **tên đầy đủ** `std.<module>.<fn>` khi tệp đã
                // `use std.<module>`; gọi vào tên đã biết nhưng hoãn ⇒ `VS0301` "chưa hỗ trợ (VS1b)".
                // ST-29: **dispatch tĩnh** — `x.m(…)` với `x` kiểu tĩnh record/enum và `m` không
                // phải phương thức nội tại ⇒ hạ thẳng `Call(impl_fn)` (chèn `self/dt` đầu).
                if let Some(e) = self.dispatch_tinh(&callee, &args, span) {
                    return e;
                }
                // ST-26: chuyển `str.split(…)` sang tên đầy đủ `std.str.split` (nếu tệp đã `use`).
                if let HExpr::Field {
                    base,
                    name: f,
                    optional: false,
                    ..
                } = &callee
                {
                    if let HExpr::Var { name: m, .. } = &**base {
                        if self.opts.std_mods.iter().any(|x| x == m) {
                            return self.std_call_named(m.clone(), f.clone(), args, span);
                        }
                    }
                }
                self.maybe_enum_lit(callee, args, named, span)
            }
            NodeKind::Index => {
                let mut it = node.children();
                let base = it
                    .next()
                    .map(|c| self.lower_expr(c))
                    .unwrap_or(HExpr::Error { span });
                let index = it
                    .next()
                    .map(|c| self.lower_expr(c))
                    .unwrap_or(HExpr::Error { span });
                HExpr::Index {
                    base: Box::new(base),
                    index: Box::new(index),
                    span,
                }
            }
            NodeKind::TryOp => {
                let inner = node
                    .children()
                    .next()
                    .map(|c| self.lower_expr(c))
                    .unwrap_or(HExpr::Error { span });
                HExpr::Try {
                    expr: Box::new(inner),
                    span,
                }
            }
            NodeKind::WithOp => {
                let mut it = node.children();
                let base = it
                    .next()
                    .map(|c| self.lower_expr(c))
                    .unwrap_or(HExpr::Error { span });
                let mut fields = Vec::new();
                for f in node.children().filter(|n| n.kind == NodeKind::WithField) {
                    let name = self.decl_name(f).unwrap_or_default();
                    let value = f
                        .children()
                        .next()
                        .map(|c| self.lower_expr(c))
                        .unwrap_or(HExpr::Error { span: f.span });
                    fields.push((name, value));
                }
                HExpr::With {
                    base: Box::new(base),
                    fields,
                    span,
                }
            }
            NodeKind::IfExpr => self.lower_if_expr(node),
            NodeKind::MatchExpr => {
                let subject = node
                    .children()
                    .find(|c| c.kind != NodeKind::Suite)
                    .map(|c| self.lower_expr(c))
                    .unwrap_or(HExpr::Error { span });
                let arms = node
                    .child(NodeKind::Suite)
                    .map(|s| self.lower_arms(s))
                    .unwrap_or_default();
                HExpr::Match {
                    subject: Box::new(subject),
                    arms,
                    ty: Type::Error,
                    span,
                }
            }
            NodeKind::Lambda => self.lower_lambda(node),
            NodeKind::RefExpr => {
                let inner = node
                    .children()
                    .next()
                    .map(|c| self.lower_expr(c))
                    .unwrap_or(HExpr::Error { span });
                HExpr::Ref {
                    expr: Box::new(inner),
                    span,
                }
            }
            NodeKind::AwaitExpr => {
                let inner = node
                    .children()
                    .next()
                    .map(|c| self.lower_expr(c))
                    .unwrap_or(HExpr::Error { span });
                HExpr::Await {
                    expr: Box::new(inner),
                    span,
                }
            }
            NodeKind::CmdBlock => HExpr::CmdBlock {
                raw: self.text_of(span).to_string(),
                span,
            },
            _ => HExpr::Error { span },
        }
    }

    fn lower_literal(&mut self, node: &Node) -> HExpr {
        let span = node.span;
        let Some(t) = node.tokens(&self.cst.tokens).next() else {
            return HExpr::Error { span };
        };
        if t.kind == Kind::Kw {
            return match t.text.as_str() {
                "none" => HExpr::Opt {
                    value: None,
                    ty: Type::Opt(Box::new(Type::Error)),
                    span,
                },
                "true" | "false" => HExpr::Lit {
                    lit: Lit {
                        kind: LitKind::Bool,
                        text: t.text.clone(),
                    },
                    ty: Type::Bool,
                    span,
                },
                _ => HExpr::Error { span },
            };
        }
        let Some(kind) = lit_kind(t.kind) else {
            return HExpr::Error { span };
        };
        // ST-14 (VD-47a): chuỗi có `{expr}` ⇒ hạ **từng mảnh** thành `HExpr::Interp`.
        if kind == LitKind::Str
            && t.kind != Kind::RawStr
            && t.kind != Kind::RawMulti
            && t.text.contains('{')
        {
            let ngoac = if t.kind == Kind::MultiStr { 3 } else { 1 };
            if t.text.len() >= ngoac * 2 {
                let trong = &t.text[ngoac..t.text.len() - ngoac];
                return self.chuoi_noi_suy(trong, ngoac, span);
            }
        }
        let lit = Lit {
            kind,
            text: t.text.clone(),
        };
        let ty = lit.ty();
        HExpr::Lit { lit, ty, span }
    }

    /// Hạ chuỗi **có nội suy** (ST-14): tách mảnh chữ + `{expr}`, hạ từng `{expr}` **bằng chính
    /// `Lowerer` này** (chia sẻ `symbols`/`scope` ⇒ tên trong nội suy giải đúng ngữ cảnh), với CST
    /// của mảnh được **đệm khoảng trắng** tới đúng **offset tuyệt đối** trong tệp để `span` (và do đó
    /// `dòng:cột`) trỏ **đúng chỗ**.
    fn chuoi_noi_suy(&mut self, trong: &str, ngoac: usize, span: Span) -> HExpr {
        let mut parts: Vec<InterpPart> = Vec::new();
        let mut chu = String::new();
        let mut i = 0usize;
        let mut hong = false;
        while i < trong.len() {
            let c = trong[i..].chars().next().unwrap();
            let n = c.len_utf8();
            let sau = &trong[i + n..];
            match c {
                '{' if sau.starts_with('{') => {
                    chu.push('{');
                    i += n + 1;
                }
                '}' if sau.starts_with('}') => {
                    chu.push('}');
                    i += n + 1;
                }
                '{' => {
                    // quét tới `}` cân bằng (cho phép `{ f({a: 1}) }` ở mức ngoặc)
                    let start = i + n;
                    let mut depth = 1usize;
                    let mut j = start;
                    let mut end = None;
                    while j < trong.len() {
                        let d = trong[j..].chars().next().unwrap();
                        if d == '{' {
                            depth += 1;
                        } else if d == '}' {
                            depth -= 1;
                            if depth == 0 {
                                end = Some(j);
                                break;
                            }
                        }
                        j += d.len_utf8();
                    }
                    let Some(end) = end else {
                        self.diag(
                            &vis_diag::SYNTAX_CODES[0],
                            "nội suy `{` không có `}` đóng (§A.6)",
                            Span {
                                start: span.start + ngoac + i,
                                end: span.start + ngoac + trong.len(),
                            },
                        );
                        hong = true;
                        break;
                    };
                    if !chu.is_empty() {
                        parts.push(InterpPart::Str(std::mem::take(&mut chu)));
                    }
                    let off = span.start + ngoac + start;
                    match self.ha_manh(&trong[start..end], off) {
                        Some(e) => parts.push(InterpPart::Expr(Box::new(e))),
                        None => {
                            hong = true;
                            break;
                        }
                    }
                    i = end + 1;
                }
                '\\' => {
                    let (c2, dung) = giai_escape(sau);
                    chu.push(c2);
                    i += n + dung;
                }
                _ => {
                    chu.push(c);
                    i += n;
                }
            }
        }
        if !chu.is_empty() {
            parts.push(InterpPart::Str(chu));
        }
        if hong && parts.is_empty() {
            return HExpr::Error { span };
        }
        HExpr::Interp { parts, span }
    }

    /// Hạ **một** mảnh `{…}` tại offset tuyệt đối `off` — đệm khoảng trắng để `span` tuyệt đối.
    fn ha_manh(&mut self, manh: &str, off: usize) -> Option<HExpr> {
        // Đệm **đúng `off` byte** nhưng chèn `\n` ở byte cuối: giữ offset tuyệt đối mà **không**
        // sinh `INDENT` giả ở đầu tệp (lexer coi dòng 1 là dòng trắng, mảnh bắt đầu ở cột 1).
        let dem = " ".repeat(off.saturating_sub(1));
        let src = if off == 0 {
            manh.to_string()
        } else {
            format!("{dem}\n{manh}")
        };
        let parsed = parse(&src, Mode::Script);
        if !parsed.diagnostics.is_empty() {
            for d in &parsed.diagnostics {
                self.diag(
                    d.code,
                    d.message.clone(),
                    Span {
                        start: off,
                        end: off + manh.len(),
                    },
                );
            }
            return None;
        }
        let cst: &'static Cst = Box::leak(Box::new(parsed.cst));
        let text: &'static str = Box::leak(src.into_boxed_str());
        let _lex: &'static LexOutput = Box::leak(Box::new(lex(text, Mode::Script)));
        let cu_cst = self.cst;
        let cu_text = self.text;
        self.cst = cst;
        self.text = text;
        let mut ket_qua = None;
        let con: Vec<Node> = self.cst.root.children().cloned().collect();
        // Mảnh phải là **đúng một** câu lệnh biểu thức.
        match con.as_slice() {
            [child] => match self.lower_stmt(child) {
                Some(crate::hir::HStmt::Expr(expr)) => ket_qua = Some(expr),
                _ => self.diag(
                    &vis_diag::SYNTAX_CODES[0],
                    "nội suy `{…}` chỉ nhận **một biểu thức**",
                    Span {
                        start: off,
                        end: off + manh.len(),
                    },
                ),
            },
            _ => self.diag(
                &vis_diag::SYNTAX_CODES[0],
                "nội suy `{…}` chỉ nhận **một biểu thức**",
                Span {
                    start: off,
                    end: off + manh.len(),
                },
            ),
        }
        self.cst = cu_cst;
        self.text = cu_text;
        ket_qua
    }

    /// `nilai_kiểu` cho các vị trí cần suy ra kiểu phần tử (list/map/tuple).
    fn type_of(&self, e: &HExpr) -> Type {
        match e {
            HExpr::Lit { ty, .. }
            | HExpr::Opt { ty, .. }
            | HExpr::Record { ty, .. }
            | HExpr::Tuple { ty, .. }
            | HExpr::If { ty, .. }
            | HExpr::Match { ty, .. }
            | HExpr::EnumLit { ty, .. } => ty.clone(),
            HExpr::List { elem, .. } => Type::List(Box::new(elem.clone())),
            HExpr::Set { elem, .. } => Type::List(Box::new(elem.clone())),
            HExpr::Map { key, value, .. } => {
                Type::Map(Box::new(key.clone()), Box::new(value.clone()))
            }
            HExpr::Lambda { params, ret, .. } => Type::Fn(
                params
                    .iter()
                    .map(|p| p.ty.clone())
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                Box::new(ret.clone()),
            ),
            _ => Type::Error,
        }
    }

    /// (đối số vị trí, đối số có tên) từ node `Args`.
    fn lower_args(&mut self, args: &Node) -> (Vec<HExpr>, Vec<(String, HExpr)>) {
        let mut positional = Vec::new();
        let mut named = Vec::new();
        for e in args.children() {
            match e.kind {
                NodeKind::NamedArg => {
                    let name = self.decl_name(e).unwrap_or_default();
                    let value = e
                        .children()
                        .next()
                        .map(|c| self.lower_expr(c))
                        .unwrap_or(HExpr::Error { span: e.span });
                    named.push((name, value));
                }
                _ => positional.push(self.lower_expr(e)),
            }
        }
        (positional, named)
    }

    /// Gọi `std.<module>.<fn>(…)` khi `<module>` đã `use` (ST-26) — tên đầy đủ `std.m.f`.
    fn std_call_named(&mut self, m: String, f: String, args: Vec<HExpr>, span: Span) -> HExpr {
        let ngan = format!("{m}.{f}");
        if vis_types_stdlib::hoan(&ngan) {
            self.diag(&NAME_CODES[0], format!("`{ngan}` chưa hỗ trợ (VS1b)"), span);
            return HExpr::Error { span };
        }
        if !vis_types_stdlib::la_std(&ngan) {
            self.diag(
                &NAME_CODES[0],
                format!("module `std.{m}` không có hàm `{f}`"),
                span,
            );
            return HExpr::Error { span };
        }
        HExpr::Call {
            callee: Box::new(HExpr::Var {
                name: format!("std.{ngan}"),
                sym: None,
                span,
            }),
            args,
            named: Vec::new(),
            span,
        }
    }

    /// ST-29 — **dispatch tĩnh**: `x.m(…)` với `x` có kiểu tĩnh **record/enum** ⇒ tìm `impl` và hạ
    /// thành gọi **trực tiếp** hàm impl (chèn `self`/`dt` làm đối số đầu). Không có ⇒ `VS0403`;
    /// nhiều ⇒ `VS0403` (mơ hồ). Kiểu không tĩnh ⇒ để nguyên (nội tại/`VS0403` ở tầng sau).
    fn dispatch_tinh(&mut self, callee: &HExpr, args: &[HExpr], span: Span) -> Option<HExpr> {
        let HExpr::Field {
            base,
            name,
            optional: false,
            ..
        } = callee
        else {
            return None;
        };
        let Some(ty) = self.static_ty(base) else {
            // ST-29: kiểu bên nhận **không tĩnh** (generic/`{error}`) — nếu tên khớp một phương thức
            // `impl` nào đó thì báo rõ (VS1b), thay vì để lỗi mờ lúc chạy.
            if self.impls.iter().any(|(_, n, _)| n == name) {
                self.diag(
                    &TYPE_CODES[2],
                    format!(
                        "không phân giải được phương thức `{name}` cho kiểu không tĩnh (VS1b — thêm chú thích kiểu)"
                    ),
                    span,
                );
                return Some(HExpr::Error { span });
            }
            return None;
        };
        let ten = self.ten_kieu(&ty)?;
        let khop: Vec<u32> = self
            .impls
            .iter()
            .filter(|(t, n, _)| self.ten_kieu(t).as_deref() == Some(ten.as_str()) && n == name)
            .map(|(_, _, sym)| *sym)
            .collect();
        match khop.len() {
            1 => {
                // Đối số đầu = chính biểu thức nhận (`self`/`dt` theo giá trị).
                let mut ds: Vec<HExpr> = Vec::with_capacity(args.len() + 1);
                ds.push((**base).clone());
                ds.extend(args.iter().cloned());
                Some(HExpr::Call {
                    callee: Box::new(HExpr::Var {
                        name: format!("{ten}.{name}"),
                        sym: Some(khop[0]),
                        span,
                    }),
                    args: ds,
                    named: Vec::new(),
                    span,
                })
            }
            0 => {
                let co_trait = self
                    .traits
                    .iter()
                    .find(|(_, ms)| ms.iter().any(|(mn, _, _, _)| mn == name))
                    .map(|(tn, _)| tn.clone());
                let msg = match co_trait {
                    Some(tn) => format!(
                        "trait `{tn}` có phương thức `{name}` nhưng `{ten}` chưa cài (`impl {tn} for {ten}`)"
                    ),
                    None => format!("không có phương thức `{name}` cho `{ten}` (VS0403)"),
                };
                self.diag(&TYPE_CODES[2], msg, span);
                Some(HExpr::Error { span })
            }
            n => {
                self.diag(
                    &TYPE_CODES[2],
                    format!("mơ hồ: {n} impl cùng cung cấp phương thức `{name}` cho `{ten}`"),
                    span,
                );
                Some(HExpr::Error { span })
            }
        }
    }

    /// `Circle(1.0)` / `Shape.Circle(1.0)` ⇒ [`HExpr::EnumLit`] khi biến thể **đã khai**.
    fn maybe_enum_lit(
        &mut self,
        callee: HExpr,
        args: Vec<HExpr>,
        named: Vec<(String, HExpr)>,
        span: Span,
    ) -> HExpr {
        match &callee {
            HExpr::Var {
                name, sym: None, ..
            } => {
                if let Some((_, eid, _)) = self
                    .variant_index
                    .iter()
                    .find(|(n, _, _)| n == name)
                    .cloned()
                {
                    return HExpr::EnumLit {
                        enum_id: Some(eid),
                        path: vec![name.clone()],
                        variant: name.clone(),
                        args,
                        ty: Type::Enum(eid),
                        span,
                    };
                }
            }
            HExpr::Field {
                base,
                name,
                optional: false,
                ..
            } => {
                if let HExpr::Var {
                    name: enum_name, ..
                } = &**base
                {
                    if let Some((_, eid)) = self
                        .enum_names
                        .iter()
                        .find(|(n, _)| n == enum_name)
                        .cloned()
                    {
                        return HExpr::EnumLit {
                            enum_id: Some(eid),
                            path: vec![enum_name.clone()],
                            variant: name.clone(),
                            args,
                            ty: Type::Enum(eid),
                            span,
                        };
                    }
                }
            }
            _ => {}
        }
        HExpr::Call {
            callee: Box::new(callee),
            args,
            named,
            span,
        }
    }

    fn lower_if_expr(&mut self, node: &Node) -> HExpr {
        let span = node.span;
        let cond_node = node.children().find(|n| {
            !matches!(
                n.kind,
                NodeKind::ElifArm | NodeKind::ElseArm | NodeKind::Suite
            )
        });
        let (cond, then) = match cond_node {
            Some(c) => {
                let is_let = self.is_let_cond(c);
                if is_let {
                    self.push_frame(super::FrameKind::Block, c.span);
                }
                let cond = self.lower_cond(c);
                let then = node
                    .child(NodeKind::Suite)
                    .map(|s| self.lower_suite(s))
                    .unwrap_or_default();
                if is_let {
                    self.pop_frame();
                }
                (cond, then)
            }
            None => (
                HCond::Expr(HExpr::Error { span }),
                node.child(NodeKind::Suite)
                    .map(|s| self.lower_suite(s))
                    .unwrap_or_default(),
            ),
        };
        let mut elifs = Vec::new();
        for arm in node.children().filter(|n| n.kind == NodeKind::ElifArm) {
            let body = arm
                .child(NodeKind::Suite)
                .map(|s| self.lower_suite(s))
                .unwrap_or_default();
            let c = arm
                .children()
                .find(|n| n.kind != NodeKind::Suite)
                .map(|n| self.lower_cond(n))
                .unwrap_or(HCond::Expr(HExpr::Error { span: arm.span }));
            elifs.push((c, body));
        }
        let else_ = node
            .children()
            .find(|n| n.kind == NodeKind::ElseArm)
            .and_then(|a| a.child(NodeKind::Suite))
            .map(|s| self.lower_suite(s))
            .unwrap_or_default();
        HExpr::If {
            cond: Box::new(cond),
            then,
            elifs,
            else_,
            ty: Type::Error,
            span,
        }
    }

    fn lower_lambda(&mut self, node: &Node) -> HExpr {
        let span = node.span;
        self.push_frame(super::FrameKind::Fn, span);
        let mut params: Vec<HParam> = Vec::new();
        if let Some(lp) = node.child(NodeKind::LambdaParams) {
            for p in lp.children().filter(|n| n.kind == NodeKind::Param) {
                let Some((name, psp)) = self.first_ident(p) else {
                    continue;
                };
                let ty = match p.child(NodeKind::Type) {
                    Some(t) => self.resolve_type(t),
                    None => Type::Error,
                };
                let sym = self.declare(&name, super::SymKind::Param, psp, Some(ty.clone()));
                params.push(HParam {
                    name,
                    span: psp,
                    sym,
                    ty,
                    named: false,
                    is_self: false,
                    is_var_self: false,
                    default: None,
                    variadic: false,
                });
            }
        }
        let ret = node
            .child(NodeKind::Type)
            .map(|t| self.resolve_type(t))
            .unwrap_or(Type::Error);
        let body = match node.child(NodeKind::Suite) {
            Some(s) => HBody::Stmts(self.lower_suite(s)),
            None => {
                let e = node
                    .children()
                    .filter(|n| n.kind != NodeKind::LambdaParams && n.kind != NodeKind::Type)
                    .last()
                    .map(|c| self.lower_expr(c))
                    .unwrap_or(HExpr::Error { span });
                HBody::Expr(Box::new(e))
            }
        };
        self.pop_frame();
        HExpr::Lambda {
            params,
            ret,
            body,
            span,
        }
    }
}

/// Kiểu "đã biết" đầu tiên trong danh sách; `Type::Error` nếu không có.
fn first_known(it: impl Iterator<Item = Type>) -> Type {
    for t in it {
        if !t.is_error() {
            return t;
        }
    }
    Type::Error
}

/// Giải một escape trong chuỗi (`\n`, `\t`, `\"`, `\\`, `\{`, …) — trả (ký tự, số byte đã dùng
/// sau dấu `\`).
fn giai_escape(sau: &str) -> (char, usize) {
    let mut it = sau.chars();
    match it.next() {
        Some('n') => ('\n', 1),
        Some('t') => ('\t', 1),
        Some('r') => ('\r', 1),
        Some('0') => ('\0', 1),
        Some('\\') => ('\\', 1),
        Some('"') => ('"', 1),
        Some('\'') => ('\'', 1),
        Some('{') => ('{', 1),
        Some('}') => ('}', 1),
        Some(c) => (c, c.len_utf8()),
        None => ('\\', 0),
    }
}
