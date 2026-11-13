// SPDX-License-Identifier: MIT OR Apache-2.0
//! Hạ **câu lệnh** CST → [`HStmt`] (ST-11) — phần đuôi của [`crate::lower`].

use vis_syntax::{Elem, Kind, Node, NodeKind, Span};

use super::{FrameKind, Lowerer, SymKind, NAME_CODES};
use crate::hir::{AssignOp, HArm, HCond, HElse, HExpr, HPat, HStmt};
use crate::ty::Type;

impl<'a> Lowerer<'a> {
    // ----- câu lệnh -----

    pub(super) fn lower_suite(&mut self, node: &Node) -> Vec<HStmt> {
        self.push_frame(FrameKind::Block, node.span);
        let mut out = Vec::new();
        for child in node.children() {
            if let Some(s) = self.lower_stmt(child) {
                out.push(s);
            }
        }
        self.pop_frame();
        out
    }

    pub(super) fn lower_stmt(&mut self, node: &Node) -> Option<HStmt> {
        match node.kind {
            NodeKind::LetStmt => Some(self.lower_let(node)),
            NodeKind::VarStmt => Some(self.lower_var(node)),
            NodeKind::AssignStmt => Some(self.lower_assign(node)),
            NodeKind::ReturnStmt => {
                let value = node.children().next().map(|c| self.lower_expr(c));
                Some(HStmt::Return {
                    value,
                    span: node.span,
                })
            }
            NodeKind::BreakStmt => Some(HStmt::Break { span: node.span }),
            NodeKind::ContinueStmt => Some(HStmt::Continue { span: node.span }),
            NodeKind::ForStmt => {
                let iter = node
                    .children()
                    .find(|c| c.kind != NodeKind::Pattern && c.kind != NodeKind::Suite)
                    .map(|c| self.lower_expr(c))
                    .unwrap_or(HExpr::Error { span: node.span });
                self.push_frame(FrameKind::Block, node.span);
                let pat = node
                    .child(NodeKind::Pattern)
                    .map(|p| self.lower_pat(p))
                    .unwrap_or(HPat::Wild { span: node.span });
                let body = node
                    .child(NodeKind::Suite)
                    .map(|s| self.lower_suite(s))
                    .unwrap_or_default();
                self.pop_frame();
                Some(HStmt::For {
                    pat,
                    iter,
                    body,
                    span: node.span,
                })
            }
            NodeKind::WhileStmt => {
                let cond = node
                    .children()
                    .find(|c| c.kind != NodeKind::Suite)
                    .map(|c| self.lower_expr(c))
                    .unwrap_or(HExpr::Error { span: node.span });
                let body = node
                    .child(NodeKind::Suite)
                    .map(|s| self.lower_suite(s))
                    .unwrap_or_default();
                Some(HStmt::While {
                    cond,
                    body,
                    span: node.span,
                })
            }
            NodeKind::LoopStmt => {
                let body = node
                    .child(NodeKind::Suite)
                    .map(|s| self.lower_suite(s))
                    .unwrap_or_default();
                Some(HStmt::Loop {
                    body,
                    span: node.span,
                })
            }
            NodeKind::ScopeStmt => {
                self.push_frame(FrameKind::Block, node.span);
                let label = self.first_ident(node).map(|(n, sp)| {
                    self.declare(&n, SymKind::ScopeBind, sp, None);
                    n
                });
                let body = node
                    .child(NodeKind::Suite)
                    .map(|s| self.lower_suite(s))
                    .unwrap_or_default();
                self.pop_frame();
                Some(HStmt::Scope {
                    label,
                    body,
                    span: node.span,
                })
            }
            NodeKind::IfStmt => Some(self.lower_if(node)),
            NodeKind::MatchStmt => {
                let subject = node
                    .children()
                    .find(|c| c.kind != NodeKind::Suite)
                    .map(|c| self.lower_expr(c))
                    .unwrap_or(HExpr::Error { span: node.span });
                let arms = node
                    .child(NodeKind::Suite)
                    .map(|s| self.lower_arms(s))
                    .unwrap_or_default();
                Some(HStmt::Match {
                    subject,
                    arms,
                    span: node.span,
                })
            }
            NodeKind::ExprStmt => {
                let e = node
                    .children()
                    .next()
                    .map(|c| self.lower_expr(c))
                    .unwrap_or(HExpr::Error { span: node.span });
                Some(HStmt::Expr(e))
            }
            NodeKind::Suite => Some(HStmt::Block {
                body: self.lower_suite(node),
                span: node.span,
            }),
            _ => None,
        }
    }

    fn lower_let(&mut self, node: &Node) -> HStmt {
        // §4.1: hạ **giá trị trước**, rồi mới khai tên — `let x = x + 1` phải thấy `x` **ngoài**;
        // `let x = 1` ngoài + `let x = x + 1` trong khối thì RHS trỏ `x` khung **ngoài**.
        let ty = node.child(NodeKind::Type).map(|t| self.resolve_type(t));
        let init = node
            .children()
            .filter(|c| !matches!(c.kind, NodeKind::Pattern | NodeKind::Type | NodeKind::Suite))
            .last()
            .map(|c| self.lower_expr(c))
            .unwrap_or(HExpr::Error { span: node.span });
        let pat = node
            .child(NodeKind::Pattern)
            .map(|p| self.lower_pat(p))
            .unwrap_or(HPat::Wild { span: node.span });
        // ST-29: ghi kiểu tĩnh của biến khi có **chú thích** (`let h: Hop = …`).
        if let Some(t) = &ty {
            if let HPat::Bind { sym, .. } = &pat {
                if !matches!(t, Type::Error) {
                    self.type_env.insert(*sym, t.clone());
                }
            }
        }
        let else_ = self.lower_let_else(node);
        HStmt::Let {
            pat,
            ty,
            init,
            else_,
            span: node.span,
        }
    }

    /// `else [IDENT]: suite` — tên lỗi (nếu có) là khai báo **trong** khối `else`.
    fn lower_let_else(&mut self, node: &Node) -> Option<HElse> {
        let suite = node.child(NodeKind::Suite)?;
        let mut bind: Option<(String, Span)> = None;
        let mut seen_else = false;
        for e in &node.elems {
            match e {
                Elem::Token(i) => {
                    let Some(t) = self.tok(*i) else { continue };
                    if t.kind == Kind::Kw && t.text == "else" {
                        seen_else = true;
                    } else if seen_else && t.kind == Kind::Ident {
                        bind = Some((t.text.clone(), t.span));
                    } else if seen_else && t.is_punct(":") {
                        break;
                    }
                }
                Elem::Node(n) if n.kind == NodeKind::Suite => break,
                Elem::Node(_) => {}
            }
        }
        self.push_frame(FrameKind::Block, suite.span);
        let bind_sym = bind
            .as_ref()
            .map(|(n, sp)| self.declare(n, SymKind::ElseBind, *sp, None));
        let body = self.lower_suite(suite);
        self.pop_frame();
        Some(HElse {
            bind: bind.map(|(n, _)| n),
            bind_sym,
            body,
            span: suite.span,
        })
    }

    fn lower_var(&mut self, node: &Node) -> HStmt {
        let is_self = self.has_token(node, "self");
        let (name, span) = match self.first_ident(node) {
            Some((n, sp)) => (n, sp),
            None if is_self => ("self".to_string(), node.span),
            None => (String::new(), node.span),
        };
        let ty = node.child(NodeKind::Type).map(|t| self.resolve_type(t));
        let init = node
            .children()
            .filter(|c| c.kind != NodeKind::Type)
            .last()
            .map(|c| self.lower_expr(c))
            .unwrap_or(HExpr::Error { span: node.span });
        let sym = if name.is_empty() {
            0
        } else {
            self.declare(&name, SymKind::Var, span, ty.clone())
        };
        HStmt::Var {
            name,
            span: node.span,
            sym,
            ty,
            init,
            is_self,
        }
    }

    fn lower_assign(&mut self, node: &Node) -> HStmt {
        // Dạng 1: lvalue là **node** (`a.b = 1`, `xs[0] = 1`).
        // Dạng 2: gán ghép `x += 1` — token `Ident` + cặp dấu `+` `=` (VD-37 mục 4).
        let mut target: Option<HExpr> = None;
        let mut value: Option<&Node> = None;
        let mut op_text = String::new();
        // Đích gán: **hoãn** `VS0301` của `lower_var_use` (Strict) — dưới đây báo **đúng một**
        // lần ở gốc lvalue, tránh 2 chẩn đoán cho cùng một `x = 1`.
        let saved = self.suppress_name_diag;
        self.suppress_name_diag = true;
        for e in &node.elems {
            match e {
                Elem::Node(n) => {
                    if target.is_none() {
                        target = Some(self.lower_expr(n));
                    } else {
                        value = Some(n);
                    }
                }
                Elem::Token(i) => {
                    let Some(t) = self.tok(*i) else { continue };
                    if target.is_none() && t.kind == Kind::Ident {
                        let span = t.span;
                        let name = t.text.clone();
                        target = Some(self.lower_var_use(&name, span));
                    } else if t.kind == Kind::Punct && t.text != "=" {
                        op_text.push_str(&t.text);
                    }
                }
            }
        }
        self.suppress_name_diag = saved;
        let op = match op_text.as_str() {
            "+" => AssignOp::Add,
            "-" => AssignOp::Sub,
            "*" => AssignOp::Mul,
            "/" => AssignOp::Div,
            "%" => AssignOp::Rem,
            _ => AssignOp::Set,
        };
        // Gán cho tên **chưa khai** là lỗi chắc chắn (VD-44 #2/VD-45) — báo ở **mọi** chính sách
        // **đúng một lần**, kể cả đích phức hợp `a.b = 1` / `xs[0] = 1` (báo ở **gốc** lvalue).
        if let Some(target) = target.as_ref() {
            if let Some((name, span)) = lvalue_root(target) {
                self.diag(
                    &NAME_CODES[0],
                    format!(
                        "tên không tồn tại: `{name}` chưa được khai báo (dùng `let`/`var` trước)"
                    ),
                    span,
                );
            }
        }
        let value = value
            .map(|v| self.lower_expr(v))
            .unwrap_or(HExpr::Error { span: node.span });
        HStmt::Assign {
            target: target.unwrap_or(HExpr::Error { span: node.span }),
            op,
            value,
            span: node.span,
        }
    }

    fn lower_if(&mut self, node: &Node) -> HStmt {
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
                    self.push_frame(FrameKind::Block, c.span);
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
                HCond::Expr(HExpr::Error { span: node.span }),
                node.child(NodeKind::Suite)
                    .map(|s| self.lower_suite(s))
                    .unwrap_or_default(),
            ),
        };
        let mut elifs = Vec::new();
        for arm in node.children().filter(|n| n.kind == NodeKind::ElifArm) {
            let cnode = arm.children().find(|n| n.kind != NodeKind::Suite);
            let (c, body) = match cnode {
                Some(c) => {
                    let is_let = self.is_let_cond(c);
                    if is_let {
                        self.push_frame(FrameKind::Block, c.span);
                    }
                    let c = self.lower_cond(c);
                    let body = arm
                        .child(NodeKind::Suite)
                        .map(|s| self.lower_suite(s))
                        .unwrap_or_default();
                    if is_let {
                        self.pop_frame();
                    }
                    (c, body)
                }
                None => (HCond::Expr(HExpr::Error { span: arm.span }), Vec::new()),
            };
            elifs.push((c, body));
        }
        let else_ = node
            .children()
            .find(|n| n.kind == NodeKind::ElseArm)
            .and_then(|a| a.child(NodeKind::Suite))
            .map(|s| self.lower_suite(s));
        HStmt::If {
            cond,
            then,
            elifs,
            else_,
            span: node.span,
        }
    }

    pub(super) fn is_let_cond(&self, node: &Node) -> bool {
        node.kind == NodeKind::Pattern
            && node
                .tokens(&self.cst.tokens)
                .next()
                .map(|t| t.kind == Kind::Kw && t.text == "let")
                .unwrap_or(false)
    }

    /// `if_cond := 'let' pattern '=' expr | expr` (§A.2).
    pub(super) fn lower_cond(&mut self, node: &Node) -> HCond {
        if self.is_let_cond(node) {
            let value = node
                .children()
                .filter(|n| n.kind != NodeKind::Pattern)
                .last()
                .map(|n| self.lower_expr(n))
                .unwrap_or(HExpr::Error { span: node.span });
            let pat = node
                .child(NodeKind::Pattern)
                .map(|p| self.lower_pat(p))
                .unwrap_or(HPat::Wild { span: node.span });
            return HCond::Let {
                pat,
                value,
                span: node.span,
            };
        }
        HCond::Expr(self.lower_expr(node))
    }

    pub(super) fn lower_arms(&mut self, suite: &Node) -> Vec<HArm> {
        let mut out = Vec::new();
        for arm in suite.children().filter(|n| n.kind == NodeKind::MatchArm) {
            self.push_frame(FrameKind::Block, arm.span);
            let pat = arm
                .child(NodeKind::Pattern)
                .map(|p| self.lower_pat(p))
                .unwrap_or(HPat::Wild { span: arm.span });
            let guard = arm
                .children()
                .filter(|n| n.kind != NodeKind::Pattern && n.kind != NodeKind::Suite)
                .last()
                .map(|n| self.lower_expr(n));
            let body = arm
                .child(NodeKind::Suite)
                .map(|s| self.lower_suite(s))
                .unwrap_or_default();
            self.pop_frame();
            out.push(HArm {
                pat,
                guard,
                body,
                span: arm.span,
            });
        }
        out
    }
}

/// Gốc của một **lvalue**: đi xuống `field`/`index`; trả tên nếu gốc là tên **chưa phân giải**.
fn lvalue_root(e: &HExpr) -> Option<(&str, Span)> {
    match e {
        HExpr::Var {
            name,
            sym: None,
            span,
        } => Some((name.as_str(), *span)),
        HExpr::Field { base, .. } | HExpr::Index { base, .. } => lvalue_root(base),
        _ => None,
    }
}
