// SPDX-License-Identifier: MIT OR Apache-2.0
//! Hạ **pattern** CST → [`HPat`] (ST-11) — phần đuôi của [`crate::lower`].
//!
//! §A.4: `pattern := or_pattern`; `_' | literal | '-' INT | 'none' | dotted_path | '(' … ')' |
//! '[' … ']' | '{' … '}' | dotted_path '(' … ')'`. Phân biệt **bind** với **ctor** theo luật:
//! tên viết hoa hoặc đã khai là biến thể ⇒ `HPat::Ctor`; còn lại ⇒ `HPat::Bind` (khai tên trong
//! khung hiện tại). Or-pattern: các vế **dùng chung** binding nên vế sau không báo `VS0302`.

use vis_syntax::{Elem, Kind, Node, Span, Token};

use super::{lit_kind, Lowerer, SymKind};
use crate::hir::{HPat, Lit, LitKind};

impl<'a> Lowerer<'a> {
    pub(super) fn lower_pat(&mut self, node: &Node) -> HPat {
        let span = node.span;
        let kids: Vec<&Node> = node.children().collect();
        let toks: Vec<&Token> = node.tokens(&self.cst.tokens).collect();
        // or-pattern `a | b`: vế đầu khai tên, các vế sau **dùng lại** binding (không VS0302).
        if kids.len() >= 2 && toks.iter().any(|t| t.is_punct("|")) {
            let mut alts = Vec::new();
            for (i, k) in kids.iter().enumerate() {
                let saved = self.suppress_dup;
                self.suppress_dup = i > 0;
                let p = self.lower_pat(k);
                self.suppress_dup = saved;
                alts.push(p);
            }
            return HPat::Or { alts, span };
        }
        // `parse_pattern`/`range_pattern` bọc pattern trong node `Pattern` **rỗng token**:
        // đi xuống node con duy nhất thay vì coi là `_`.
        if toks.is_empty() && kids.len() == 1 {
            return self.lower_pat(kids[0]);
        }
        let Some(first) = toks.first().copied() else {
            return HPat::Wild { span };
        };
        // range pattern `0..10` / `0..=10` / `18..` (không có nhánh phải).
        if let Some(rt) = toks.iter().find(|t| t.is_punct("..") || t.is_punct("..=")) {
            if !kids.is_empty() {
                let inclusive = rt.text == "..=";
                let mut it = kids.iter();
                let lo = it.next().map(|p| Box::new(self.lower_pat(p)));
                let hi = it.next().map(|p| Box::new(self.lower_pat(p)));
                return HPat::Range {
                    lo,
                    hi,
                    inclusive,
                    span,
                };
            }
        }
        match first.kind {
            Kind::Ident if first.text == "_" => HPat::Wild { span },
            Kind::Ident => {
                let name = first.text.clone();
                // `Ten(...)` — ctor có đối số (Kids = các pattern đối số).
                if toks.len() >= 2 && toks[1].is_punct("(") {
                    let args = kids.iter().map(|a| self.lower_pat(a)).collect();
                    return HPat::Ctor {
                        path: vec![name],
                        args,
                        span,
                    };
                }
                // Tên viết hoa / biến thể đã khai ⇒ ctor **không đối số** (`Empty`).
                if self.is_ctor_name(&name) {
                    return HPat::Ctor {
                        path: vec![name],
                        args: Vec::new(),
                        span,
                    };
                }
                let sym = if self.suppress_dup {
                    self.declare_or_reuse(&name, first.span)
                } else {
                    self.declare(&name, SymKind::PatBind, first.span, None)
                };
                HPat::Bind {
                    name,
                    span: first.span,
                    sym,
                }
            }
            Kind::Kw if first.text == "none" => HPat::Ctor {
                path: vec!["none".into()],
                args: Vec::new(),
                span,
            },
            Kind::Kw if first.text == "true" || first.text == "false" => HPat::Lit {
                lit: Lit {
                    kind: LitKind::Bool,
                    text: first.text.clone(),
                },
                span,
            },
            Kind::Punct if first.text == "-" => HPat::Lit {
                lit: Lit {
                    kind: LitKind::Int,
                    text: self.text_of(span).trim().to_string(),
                },
                span,
            },
            Kind::Punct if first.text == "(" => {
                // `(a, b)` = tuple; `(a)` = nhóm; `()` = unit.
                if kids.len() == 1 {
                    self.lower_pat(kids[0])
                } else {
                    let items = kids.iter().map(|a| self.lower_pat(a)).collect();
                    HPat::Tuple { items, span }
                }
            }
            Kind::Punct if first.text == "[" => self.lower_list_pat(node),
            Kind::Punct if first.text == "{" => self.lower_record_pat(node),
            _ => match lit_kind(first.kind) {
                Some(kind) => HPat::Lit {
                    lit: Lit {
                        kind,
                        text: first.text.clone(),
                    },
                    span,
                },
                None => HPat::Wild { span },
            },
        }
    }

    /// `[first, ..rest]` — phần tử là node `Pattern`, `..IDENT` là **token** (A.4).
    fn lower_list_pat(&mut self, node: &Node) -> HPat {
        let mut items = Vec::new();
        let mut rest: Option<Box<HPat>> = None;
        let mut in_rest = false;
        for e in &node.elems {
            match e {
                Elem::Token(i) => {
                    let Some(t) = self.tok(*i) else { continue };
                    if t.is_punct("..") {
                        in_rest = true;
                    } else if in_rest && t.kind == Kind::Ident {
                        let n = t.text.clone();
                        let sp = t.span;
                        let sym = self.declare(&n, SymKind::PatBind, sp, None);
                        rest = Some(Box::new(HPat::Bind {
                            name: n,
                            span: sp,
                            sym,
                        }));
                        in_rest = false;
                    }
                }
                Elem::Node(n) => items.push(self.lower_pat(n)),
            }
        }
        if rest.is_none() && in_rest {
            rest = Some(Box::new(HPat::Rest {
                name: None,
                span: node.span,
            }));
        }
        HPat::List {
            items,
            rest,
            span: node.span,
        }
    }

    /// `{name, age: 18.., ..}` — tên trường là token `Ident`, pattern là node con.
    fn lower_record_pat(&mut self, node: &Node) -> HPat {
        let mut fields: Vec<(String, Option<HPat>)> = Vec::new();
        let mut open = false;
        let mut pending: Option<(String, Span)> = None;
        for e in &node.elems {
            match e {
                Elem::Token(i) => {
                    let Some(t) = self.tok(*i) else { continue };
                    if t.is_punct("..") {
                        open = true;
                    } else if t.kind == Kind::Ident {
                        if let Some((n, sp)) = pending.take() {
                            // ST-15b (mục 4): trường trần ⇒ Bind thật (xem cuối hàm).
                            let id = self.declare(&n, SymKind::PatBind, sp, None);
                            fields.push((
                                n.clone(),
                                Some(HPat::Bind {
                                    name: n,
                                    span: sp,
                                    sym: id,
                                }),
                            ));
                        }
                        pending = Some((t.text.clone(), t.span));
                    }
                }
                Elem::Node(n) => {
                    let p = self.lower_pat(n);
                    match pending.take() {
                        Some((name, _)) => fields.push((name, Some(p))),
                        None => fields.push((String::new(), Some(p))),
                    }
                }
            }
        }
        if let Some((n, sp)) = pending.take() {
            // ST-15b (mục 4): trường **trần** `{x}` phải **binding** thật (Bind + sym), không chỉ
            // "khớp tên rồi bỏ" — trước đây HIR để `None` ⇒ interp không gán được biến.
            let id = self.declare(&n, SymKind::PatBind, sp, None);
            fields.push((
                n.clone(),
                Some(HPat::Bind {
                    name: n,
                    span: sp,
                    sym: id,
                }),
            ));
        }
        HPat::Record {
            name: None,
            fields,
            open,
            span: node.span,
        }
    }

    fn is_ctor_name(&self, name: &str) -> bool {
        if matches!(name, "some" | "ok" | "err") {
            return true;
        }
        if self.variant_index.iter().any(|(n, _, _)| n == name) {
            return true;
        }
        name.chars().next().map(char::is_uppercase).unwrap_or(false)
    }
}
