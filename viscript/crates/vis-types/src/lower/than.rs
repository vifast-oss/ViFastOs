// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-48**: thân `cmd` bằng **pipeline lệnh** — hạ vào cùng `HIR` bằng Lowerer **con** dùng chung
//! `Symbols`/`TypeTable`; tách khỏi `lower.rs`/`cmd.rs` cho luật R1 (≤1.500 dòng/tệp).

use crate::cmdsig::CmdSig;
use crate::hir::{FnKind, HExpr, HItem, HStmt, Lit, LitKind};
use crate::lower::{FrameKind, HFrame, LowerOptions, Lowerer, Symbols};
use crate::ty::{RecId, Type, TypeTable};
use vis_syntax::{Cst, Diagnostic, Elem, LexOutput, Node, NodeKind, Span};

/// **ST-48**: kết quả hạ **thân `cmd` lệnh**.
pub(crate) struct ThanOut {
    pub expr: Option<HExpr>,
    pub diagnostics: Vec<Diagnostic>,
}

/// **ST-48**: hạ một thân `cmd` lệnh bằng Lowerer **con** (chung `Symbols`/`TypeTable`), không
/// chạy prelude/khai báo lại; trả biểu thức của **pipeline duy nhất**.
pub(crate) struct ThanCtx {
    pub sigs: Vec<crate::cmdsig::CmdSig>,
    pub file_long: Option<RecId>,
    pub span: Span,
}

pub(crate) fn lower_than(
    symbols: &mut Symbols,
    types: &mut TypeTable,
    cst: &Cst,
    lex: &LexOutput,
    opts: LowerOptions,
    ctx: ThanCtx,
) -> ThanOut {
    let ThanCtx {
        sigs,
        file_long,
        span,
    } = ctx;
    let mut l = Lowerer::new(cst, lex, opts, symbols, types);
    if l.symbols.frames.is_empty() {
        l.symbols.frames.push(HFrame {
            parent: None,
            span: cst.root.span,
            kind: FrameKind::File,
        });
    }
    l.symbols.frames.push(HFrame {
        parent: Some(0),
        span,
        kind: FrameKind::Fn,
    });
    let khung = (l.symbols.frames.len() - 1) as u32;
    l.scope.push(khung);
    l.declare_builtins();
    l.cmd_sigs.extend(sigs.clone());
    l.file_long = file_long;
    // Đúng **một** pipeline: `cmd_file` trả mỗi dòng một node `CmdPipe`/`CmdStage` (không bọc
    // `CmdItems`), còn `.vish` bọc trong `CmdItems` — nhận cả hai dạng.
    let mut pipes: Vec<Node> = cst
        .root
        .children()
        .filter(|n| matches!(n.kind, NodeKind::CmdPipe | NodeKind::CmdStage))
        .cloned()
        .collect();
    let dong: Vec<Node> = cst
        .root
        .children()
        .filter(|n| n.kind == NodeKind::CmdItems)
        .cloned()
        .collect();
    for d in &dong {
        if let Some(p) = d
            .children()
            .find(|n| matches!(n.kind, NodeKind::CmdPipe | NodeKind::CmdStage))
        {
            pipes.push(p.clone());
        }
    }
    let so = pipes
        .len()
        .max(if dong.is_empty() { 0 } else { dong.len() });
    let node = pipes.first().cloned();
    if so > 1 {
        l.diag(
            &vis_diag::SYNTAX_CODES[0],
            "thân `cmd`: đúng **một** pipeline lệnh (nhiều pipeline chưa hỗ trợ)",
            span,
        );
        return ThanOut {
            expr: None,
            diagnostics: l.diags,
        };
    }
    let Some(node) = node else {
        l.diag(
            &vis_diag::SYNTAX_CODES[0],
            "thân `cmd`: không thấy pipeline lệnh",
            span,
        );
        return ThanOut {
            expr: None,
            diagnostics: l.diags,
        };
    };
    let expr = l.ha_pipeline(&node, &sigs);
    ThanOut {
        expr,
        diagnostics: l.diags,
    }
}

/// `true` nếu tên là tên giá trị dựng sẵn (VD-44 #3).
pub fn is_builtin_value(name: &str) -> bool {
    crate::lower::BUILTIN_VALUE_NAMES.contains(&name)
}

impl<'a> super::Lowerer<'a> {
    /// **ST-48**: hạ **thân `cmd` lệnh** (một pipeline) vào `body` của `HItem::Fn` tương ứng.
    /// Thân đã được parser lệnh chính parse thành stage (span tuyệt đối); ở đây parse lại bản
    /// `dedent` với span **dịch tuyệt đối** rồi hạ bằng một Lowerer **con** dùng chung bảng.
    pub(crate) fn ha_than_cmd(&mut self, ds: &[crate::lower::khai::ThanLenh]) {
        if ds.is_empty() {
            return;
        }
        let sigs: Vec<crate::cmdsig::CmdSig> = self
            .items
            .iter()
            .filter_map(|it| match it {
                HItem::Fn(f) if f.kind == FnKind::Cmd => Some(crate::cmdsig::CmdSig {
                    name: f.name.clone(),
                    sym: Some(f.sym),
                    generics: f.generics.clone(),
                    params: f.params.clone(),
                    ret: f.ret.clone(),
                }),
                _ => None,
            })
            .chain(self.cmd_sigs.clone())
            .collect();
        for t in ds {
            let span = match (t.map.first(), t.map.last()) {
                (Some(&a), Some(&b)) => Span::new(a, b.min(self.text.len())),
                _ => Span::new(0, 0),
            };
            let dat_loi = |l: &mut Self, t: &crate::lower::khai::ThanLenh| {
                if let Some(HItem::Fn(f)) = l.items.iter_mut().find(
                    |it| matches!(it, HItem::Fn(g) if g.name == t.ten && g.kind == FnKind::Cmd),
                ) {
                    f.body = vec![HStmt::Expr(HExpr::Error {
                        span: Span::new(0, 0),
                    })];
                }
            };
            if t.kieu == crate::lower::khai::KieuThan::Mixed {
                self.diag(
                    &vis_diag::SYNTAX_CODES[0],
                    "thân `cmd`: lẫn biểu thức **script** và **lệnh** — chọn một kiểu (biểu thức, hoặc                      đúng một pipeline lệnh)",
                    span,
                );
                dat_loi(self, t);
                continue;
            }
            if t.dong.is_empty() {
                self.diag(
                    &vis_diag::SYNTAX_CODES[0],
                    "thân `cmd` rỗng — cần biểu thức script hoặc **một pipeline lệnh**",
                    span,
                );
                dat_loi(self, t);
                continue;
            }
            // Lex bản `dedent` rồi **dịch span** về vị trí thật (dùng `map` từng byte).
            let lexed = vis_syntax::lex(&t.dedent, vis_syntax::Mode::Command);
            let mut toks = lexed.tokens.clone();
            let mut loi = lexed.diagnostics.clone();
            for tk in &mut toks {
                let a = t.map.get(tk.span.start).copied().unwrap_or(0);
                let b = t
                    .map
                    .get(tk.span.end.saturating_sub(1))
                    .copied()
                    .map(|x| x + 1)
                    .unwrap_or(a);
                tk.span = Span::new(a, b);
            }
            if !loi.is_empty() {
                for d in loi.drain(..) {
                    self.diags.push(d);
                }
                continue;
            }
            let parsed = vis_syntax::parse_voi(self.text, toks, vis_syntax::Mode::Command);
            let out = crate::lower::lower_than(
                self.symbols,
                self.tbl,
                &parsed.cst,
                &vis_syntax::LexOutput {
                    text: self.text.to_string(),
                    tokens: parsed.cst.tokens.clone(),
                    diagnostics: Vec::new(),
                },
                self.opts.clone(),
                crate::lower::ThanCtx {
                    sigs: sigs.clone(),
                    file_long: self.file_long,
                    span,
                },
            );
            self.diags.extend(out.diagnostics);
            let Some(expr) = out.expr else { continue };
            if let Some(HItem::Fn(f)) = self
                .items
                .iter_mut()
                .find(|it| matches!(it, HItem::Fn(g) if g.name == t.ten && g.kind == FnKind::Cmd))
            {
                f.body = vec![HStmt::Expr(expr)];
            }
        }
    }

    /// ST-46: `( <lệnh> … )` — nhóm làm **stage** nhưng nội dung là **một lệnh** (không `|`/`;`).
    /// Parse nội dung bằng `Mode::Command` **không đệm** rồi cộng bù `at` cho span (chẩn đoán tuyệt đối).
    pub(super) fn lower_nhom_lenh(&mut self, g: &Node, sigs: &[CmdSig]) -> Option<HExpr> {
        let text = self.vach_text(g)?;
        let bat_dau = match g.elems.first() {
            Some(Elem::Token(i)) => self.tok(*i).map(|t| t.span.end).unwrap_or(0),
            _ => 0,
        };
        let sau = &self.text[bat_dau.min(self.text.len())..];
        let lead = sau.chars().take_while(|c| *c == ' ' || *c == '\t').count();
        let at = bat_dau + lead;
        let inner = text.trim();
        let parsed = vis_syntax::parse(inner, vis_syntax::lex::Mode::Command);
        if !parsed.diagnostics.is_empty() {
            return None;
        }
        // Tệp **một dòng** ⇒ root KHÔNG có `CmdItems` (VD-40 #1): pipeline nằm thẳng ở root.
        let dong = parsed
            .cst
            .root
            .children()
            .find(|n| n.kind == NodeKind::CmdItems)
            .cloned()
            .or_else(|| {
                parsed
                    .cst
                    .root
                    .children()
                    .find(|n| n.kind == NodeKind::CmdPipe)
                    .cloned()
            })?;
        let stage = dong.children().find(|n| n.kind == NodeKind::CmdStage)?;
        let doan = |sp: Span| inner.get(sp.start..sp.end).unwrap_or("").to_string();
        let ten = match stage.elems.first()? {
            Elem::Token(i) => parsed.cst.tokens.get(*i as usize)?.text.clone(),
            Elem::Node(n) => n.tokens(&parsed.cst.tokens).next()?.text.clone(),
        };
        let sig = sigs.iter().find(|s| s.name == ten)?.clone();
        if !sig.generics.is_empty() {
            return None;
        }
        let mut args: Vec<Option<HExpr>> = sig.params.iter().map(|p| p.default.clone()).collect();
        let mut vi_tri = 0usize;
        for e in &stage.elems[1..] {
            if let Elem::Node(n) = e {
                if n.kind == NodeKind::CmdGroup {
                    let raw = doan(n.span);
                    let inner_nhom = raw.strip_prefix('(')?.strip_suffix(')')?;
                    while vi_tri < sig.params.len() && sig.params[vi_tri].named {
                        vi_tri += 1;
                    }
                    if vi_tri >= sig.params.len() {
                        return None;
                    }
                    let rel = n.span.start + 1 + (raw.len() - raw.trim_start().len());
                    let e = self.lower_bieu_thuc_tai(inner_nhom, at + rel)?;
                    args[vi_tri] = Some(e);
                    vi_tri += 1;
                    continue;
                }
            }
            let (la_co, raw, sp) = match e {
                Elem::Token(i) => {
                    let t = parsed.cst.tokens.get(*i as usize)?;
                    (
                        t.text.starts_with('-') && t.text != "-",
                        t.text.clone(),
                        t.span,
                    )
                }
                Elem::Node(n) if n.kind == NodeKind::CmdFlag => {
                    let t = n.tokens(&parsed.cst.tokens).next()?;
                    (true, t.text.clone(), n.span)
                }
                Elem::Node(n) => (false, doan(n.span), n.span),
            };
            let sp = Span::new(sp.start + at, sp.end + at);
            if la_co {
                let ten_co = raw.trim_start_matches('-');
                if let Some(i) = sig.params.iter().position(|p| p.named && p.name == ten_co) {
                    args[i] = Some(HExpr::Lit {
                        ty: Type::Bool,
                        lit: Lit {
                            kind: LitKind::Bool,
                            text: "true".into(),
                        },
                        span: sp,
                    });
                }
                continue;
            }
            while vi_tri < sig.params.len() && sig.params[vi_tri].named {
                vi_tri += 1;
            }
            if vi_tri >= sig.params.len() {
                return None;
            }
            let ty = sig.params[vi_tri].ty.clone();
            let e = self.chuyen_doi_text(&raw, &ty, sp)?;
            args[vi_tri] = Some(e);
            vi_tri += 1;
        }
        let mut ds = Vec::new();
        for a in args {
            ds.push(a?);
        }
        Some(HExpr::Call {
            callee: Box::new(HExpr::Var {
                name: ten,
                sym: None,
                span: g.span,
            }),
            args: ds,
            named: Vec::new(),
            span: g.span,
        })
    }
}
