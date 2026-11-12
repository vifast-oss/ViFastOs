// SPDX-License-Identifier: MIT OR Apache-2.0
//! Hạ **biểu thức** CST → [`HExpr`] (ST-11) — phần đuôi của [`crate::lower`].
//!
//! Không kiểm kiểu: chỉ *điền* kiểu cho literal (VD-43), dựng mặt nạ bản ghi ẩn danh cho record
//! literal và nhận ra enum literal khi tên biến thể đã khai trong tệp. Mọi thứ khác giữ
//! `sym = None` / `Type::Error` và **không** sinh chẩn đoán (VD-44 #1).

use vis_syntax::{Elem, Kind, Node, NodeKind, Span};

use super::{bin_op, lit_kind, un_op, Lowerer};
use crate::hir::{BinOp, HBody, HCond, HExpr, HParam, Lit, LitKind};
use crate::ty::{FieldDef, RecDef, RecId, Type};

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
                let id = RecId(self.recs.len() as u32);
                self.recs.push(RecDef {
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
        let lit = Lit {
            kind,
            text: t.text.clone(),
        };
        let ty = lit.ty();
        HExpr::Lit { lit, ty, span }
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
