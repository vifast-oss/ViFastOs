// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-48**: thân `cmd` bằng **pipeline lệnh** — hạ vào cùng `HIR` bằng Lowerer **con** dùng chung
//! `Symbols`/`TypeTable`; tách khỏi `lower.rs`/`cmd.rs` cho luật R1 (≤1.500 dòng/tệp).

use crate::cmdsig::CmdSig;
use crate::hir::{FnKind, HExpr, HItem, HStmt};
use crate::lower::{FrameKind, HFrame, LowerOptions, Lowerer, Symbols};
use crate::ty::{RecId, Type, TypeTable};
use vis_syntax::{Cst, Diagnostic, Elem, LexOutput, Node, NodeKind, Span};

/// **ST-48**: kết quả hạ **thân `cmd` lệnh**.
pub(crate) struct ThanOut {
    pub expr: Option<HExpr>,
    pub ty: Type,
    pub diagnostics: Vec<Diagnostic>,
}

/// **ST-48**: hạ một thân `cmd` lệnh bằng Lowerer **con** (chung `Symbols`/`TypeTable`), không
/// chạy prelude/khai báo lại; trả biểu thức của **pipeline duy nhất**.
pub(crate) struct ThanCtx {
    pub sigs: Vec<crate::cmdsig::CmdSig>,
    pub file_long: Option<RecId>,
    /// Bản ghi đã biết ở Lowerer cha (prelude + item) — để sugar `where`/`it` thấy **trường**.
    pub recs: Vec<(String, RecId)>,
    pub span: Span,
    /// `true` = thân `cmd` (báo "thân cmd…"); `false` = nhóm `( … )`.
    pub la_than: bool,
}

/// **ST-49**: như [`lower_than`] nhưng cho **nhóm** `( … )` — dùng chung một đường hạ.
pub(crate) fn lower_nhom(
    symbols: &mut Symbols,
    types: &mut TypeTable,
    cst: &Cst,
    lex: &LexOutput,
    opts: LowerOptions,
    ctx: ThanCtx,
) -> ThanOut {
    lower_than(symbols, types, cst, lex, opts, ctx)
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
        recs,
        span,
        la_than,
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
    l.rec_names = recs.clone();
    l.prelude_recs = recs;
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
        let msg = if la_than {
            "thân `cmd`: đúng **một** pipeline lệnh (nhiều pipeline chưa hỗ trợ)"
        } else {
            "trong `( … )`: nhóm chỉ hạ được **một** pipeline lệnh"
        };
        l.diag(&vis_diag::SYNTAX_CODES[0], msg, span);
        return ThanOut {
            expr: None,
            ty: Type::Error,
            diagnostics: l.diags,
        };
    }
    let Some(node) = node else {
        let msg = if la_than {
            "thân `cmd`: không thấy pipeline lệnh"
        } else {
            "trong `( … )`: không phải biểu thức hay pipeline lệnh"
        };
        l.diag(&vis_diag::SYNTAX_CODES[0], msg, span);
        return ThanOut {
            expr: None,
            ty: Type::Error,
            diagnostics: l.diags,
        };
    };
    let expr = l.ha_pipeline(&node, &sigs);
    let ty = l.last_stage_ty.clone();
    ThanOut {
        expr,
        ty,
        diagnostics: l.diags,
    }
}

/// **ST-59**: hạ **khai báo (Script) + một pipeline (Command)** — trả `(câu lệnh, kiểu, chẩn đoán)`.
///
/// Hai [`Lowerer`] **con** (dùng chung `Symbols`/`TypeTable`) trong **hai khung lồng nhau**: khung
/// khai báo, rồi khung lệnh có cha là khung khai báo ⇒ `resolve` thấy biến vừa khai.
fn lower_than_khai_lenh(
    symbols: &mut Symbols,
    types: &mut TypeTable,
    khai: (&Cst, &LexOutput),
    lenh: (&Cst, &LexOutput),
    opts: LowerOptions,
    ctx: ThanCtx,
) -> (Vec<HStmt>, Type, Vec<Diagnostic>) {
    let ThanCtx {
        sigs,
        file_long,
        recs,
        span,
        ..
    } = ctx;
    // (1) khai báo — Script
    let mut l = Lowerer::new(khai.0, khai.1, opts.clone(), symbols, types);
    if l.symbols.frames.is_empty() {
        l.symbols.frames.push(HFrame {
            parent: None,
            span,
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
    l.rec_names = recs.clone();
    l.prelude_recs = recs;
    let mut stmts: Vec<HStmt> = Vec::new();
    for ch in khai.0.root.children() {
        if let Some(s) = l.lower_stmt(ch) {
            stmts.push(s);
        }
    }
    let mut diags = std::mem::take(&mut l.diags);
    let type_env = l.type_env.clone();
    drop(l);
    // (2) pipeline — Command, khung lồng trong khung khai báo
    let mut l2 = Lowerer::new(lenh.0, lenh.1, opts, symbols, types);
    l2.symbols.frames.push(HFrame {
        parent: Some(khung),
        span,
        kind: FrameKind::Fn,
    });
    let k2 = (l2.symbols.frames.len() - 1) as u32;
    l2.scope.push(khung);
    l2.scope.push(k2);
    l2.type_env = type_env;
    l2.declare_builtins();
    l2.cmd_sigs.extend(sigs);
    l2.file_long = file_long;
    let sigs2 = l2.cmd_sigs.clone();
    let node = lenh.0.root.children().find(|n| {
        matches!(
            n.kind,
            NodeKind::CmdPipe | NodeKind::CmdStage | NodeKind::CmdItems
        )
    });
    let node = node.and_then(|n| {
        if n.kind == NodeKind::CmdItems {
            n.children()
                .find(|c| matches!(c.kind, NodeKind::CmdPipe | NodeKind::CmdStage))
                .cloned()
        } else {
            Some(n.clone())
        }
    });
    let mut ty = Type::Error;
    if let Some(n) = node {
        if let Some(e) = l2.ha_pipeline(&n, &sigs2) {
            ty = l2.last_stage_ty.clone();
            stmts.push(HStmt::Expr(e));
        }
    }
    diags.extend(std::mem::take(&mut l2.diags));
    (stmts, ty, diags)
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
            // **ST-59**: khai báo + **một** pipeline ⇒ hạ **hai phần** (không còn `VS0201`).
            if t.kieu == crate::lower::khai::KieuThan::KhaiLenh {
                self.ha_than_khai_lenh(t, &sigs, span);
                continue;
            }
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
                    recs: self.rec_names.clone(),
                    span,
                    la_than: true,
                },
            );
            self.diags.extend(out.diagnostics);
            let Some(expr) = out.expr else {
                // **ST-59**: thân không hạ được (nhiều pipeline/rỗng) ⇒ body **`{error}`** như
                // đường `Mixed` — tránh tyck thêm `VS0412` khiến `check` ↔ `run` lệch **số** chẩn đoán.
                dat_loi(self, t);
                continue;
            };
            if let Some(HItem::Fn(f)) = self
                .items
                .iter_mut()
                .find(|it| matches!(it, HItem::Fn(g) if g.name == t.ten && g.kind == FnKind::Cmd))
            {
                f.body = vec![HStmt::Expr(expr)];
            }
        }
    }

    /// **ST-59**: hạ thân `cmd` **khai báo + một pipeline** — hai Lowerer **con** dùng chung
    /// `Symbols`/`TypeTable`: phần khai báo (`let …`) theo **Script**, phần cuối theo **Command**;
    /// khung của phần lệnh **lồng** trong khung khai báo ⇒ tên đã khai thấy được (và
    /// `bien_gia_tri` nhận nó làm stage biểu thức).
    pub(crate) fn ha_than_khai_lenh(
        &mut self,
        t: &crate::lower::khai::ThanLenh,
        sigs: &[CmdSig],
        span: Span,
    ) {
        // Số dòng khai báo đầu (không có dòng trống trong `dong`).
        let mut i = 0usize;
        while i < t.dong.len() && crate::lower::khai::let_la_khai(t.dong[i].trim()) {
            i += 1;
        }
        if i == 0 || i >= t.dong.len() {
            self.diag(
                &vis_diag::SYNTAX_CODES[0],
                "thân `cmd`: không tách được vùng khai báo và pipeline cuối",
                span,
            );
            return;
        }
        let off: usize = t.dong[..i].iter().map(|l| l.len() + 1).sum();
        let off = off.min(t.dedent.len());
        let (khai_txt, lenh_txt) = (&t.dedent[..off], &t.dedent[off..]);
        let khai = self.than_parse(t, khai_txt, vis_syntax::Mode::Script);
        let lenh = self.than_parse(t, lenh_txt, vis_syntax::Mode::Command);
        let (khai, lenh) = match (khai, lenh) {
            (Some(a), Some(b)) => (a, b),
            _ => {
                self.diag(
                    &vis_diag::SYNTAX_CODES[0],
                    "thân `cmd`: khai báo/pipeline cuối không parse được",
                    span,
                );
                return;
            }
        };
        let out = lower_than_khai_lenh(
            self.symbols,
            self.tbl,
            (&khai.0, &khai.1),
            (&lenh.0, &lenh.1),
            self.opts.clone(),
            ThanCtx {
                sigs: sigs.to_vec(),
                file_long: self.file_long,
                recs: self.rec_names.clone(),
                span,
                la_than: true,
            },
        );
        self.diags.extend(out.2);
        if let Some(HItem::Fn(f)) = self
            .items
            .iter_mut()
            .find(|it| matches!(it, HItem::Fn(g) if g.name == t.ten && g.kind == FnKind::Cmd))
        {
            f.body = out.0;
        }
    }

    /// Lex + parse một mẩu thân với **span tuyệt đối** (dịch qua `t.map`), trả CST + LexOutput.
    fn than_parse(
        &self,
        t: &crate::lower::khai::ThanLenh,
        txt: &str,
        mode: vis_syntax::Mode,
    ) -> Option<(Cst, LexOutput)> {
        let lexed = vis_syntax::lex(txt, mode);
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
            loi.clear();
            return None;
        }
        let parsed = vis_syntax::parse_voi(self.text, toks, mode);
        Some((
            parsed.cst,
            LexOutput {
                text: self.text.to_string(),
                tokens: Vec::new(),
                diagnostics: Vec::new(),
            },
        ))
    }

    /// **ST-49**: hạ nội dung nhóm `( … )` như **một pipeline lệnh** (xoá đường tự-dựng-Call cũ).
    /// Newline trong ngoặc đổi thành **dấu cách** (1 byte → 1 byte) ⇒ offset tuyệt đối không đổi.
    pub(super) fn lower_nhom(&mut self, g: &Node, sigs: &[CmdSig]) -> Option<(HExpr, Type)> {
        let text = self.vach_text(g)?;
        let bat_dau = match g.elems.first() {
            Some(Elem::Token(i)) => self.tok(*i).map(|t| t.span.end).unwrap_or(0),
            _ => 0,
        };
        let phang = text.replace(['\n', '\r'], " ");
        let lead = phang.len() - phang.trim_start().len();
        let trim = phang.trim();
        let at = bat_dau + lead;
        let lexed = vis_syntax::lex(trim, vis_syntax::lex::Mode::Command);
        if !lexed.diagnostics.is_empty() {
            return None;
        }
        let mut toks = lexed.tokens.clone();
        for tk in &mut toks {
            tk.span = Span::new(tk.span.start + at, tk.span.end + at);
        }
        let parsed = vis_syntax::parse_voi(self.text, toks, vis_syntax::lex::Mode::Command);
        let out = crate::lower::lower_nhom(
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
                sigs: sigs.to_vec(),
                file_long: self.file_long,
                recs: self.rec_names.clone(),
                span: g.span,
                la_than: false,
            },
        );
        self.diags.extend(out.diagnostics);
        out.expr.map(|e| (e, out.ty))
    }

    /// **ST-49**: kiểu tĩnh của nhóm-lệnh (cho `kieu_stage`): stage **đầu** của pipeline.
    pub(super) fn kieu_nhom(&self, g: &Node, sigs: &[CmdSig]) -> Type {
        let Some(text) = self.vach_text(g) else {
            return Type::Error;
        };
        let phang = text.replace(['\n', '\r'], " ");
        let trim = phang.trim();
        let parsed = vis_syntax::parse(trim, vis_syntax::lex::Mode::Command);
        let Some(pipe) = parsed
            .cst
            .root
            .children()
            .find(|n| n.kind == NodeKind::CmdPipe)
        else {
            return Type::Error;
        };
        let Some(stage) = pipe.children().find(|n| n.kind == NodeKind::CmdStage) else {
            return Type::Error;
        };
        let Some(ten) = stage.elems.first().and_then(|e| match e {
            Elem::Token(i) => parsed.cst.tokens.get(*i as usize).map(|t| t.text.clone()),
            Elem::Node(n) => n.tokens(&parsed.cst.tokens).next().map(|t| t.text.clone()),
        }) else {
            return Type::Error;
        };
        let Some(sig) = sigs.iter().find(|s| s.name == ten) else {
            return Type::Error;
        };
        let mut ret = sig.ret.clone();
        // `ls --long` ⇒ bản ghi 6 trường (cùng luật với `kieu_stage` của stage thường).
        if ten == "ls" && trim.contains("--long") {
            if let Some(id) = self.file_long {
                ret = Type::List(Box::new(Type::Record(id)));
            }
        }
        ret
    }
}
