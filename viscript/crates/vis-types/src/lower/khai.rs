// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Khai báo trong `.vish`** (ST-44, VD-77): tách vùng **item ở đầu tệp** khỏi vùng **stage**.
//!
//! Luật (Agent 0 duyệt): dòng **cột 1** bắt đầu bằng từ khoá item (`type`, `enum`, `cmd`, `fn`,
//! `use`, `const`, `trait`, `impl`, `alias`, `newtype`, `extern`, `pub`, `async`) hoặc `let` ⇒ thuộc
//! vùng khai báo; dòng cột 1 đầu tiên **khác** các từ khoá đó (và không trống/không phải chú thích)
//! ⇒ **hết** vùng khai báo, từ đó là stage. Dòng **thụt** luôn thuộc vùng đang mở (thân item).
//!
//! Hạ: tiền tố parse lại bằng `Mode::Script` (span **tuyệt đối** vì tiền tố bắt đầu ở byte 0), rồi
//! `lower_into` vào **cùng** bảng ký hiệu/kiểu; hậu tố chạy `run_shell` như cũ (bỏ các dòng có
//! `span.start` < mốc tách).

use vis_syntax::{Cst, NodeKind, Span};

/// Từ khoá mở đầu **item** ở cột 1 trong `.vish`.
pub(crate) const TU_KHOA_ITEM: [&str; 13] = [
    "type", "enum", "cmd", "fn", "use", "const", "trait", "impl", "alias", "newtype", "extern",
    "pub", "async",
];

/// Dòng cột 1 có phải **khai báo** không (`let` tính là khai báo cấp tệp).
pub(crate) fn la_khai_bao(dong: &str) -> bool {
    let d = dong.trim_end();
    if d.is_empty() || d.starts_with('#') || d.starts_with("//") {
        return true; // dòng trống/chú thích **giữ** trong vùng khai báo
    }
    if d.starts_with(' ') || d.starts_with('\t') {
        return true; // dòng thụt: thân item (hoặc tiếp nối) — thuộc vùng đang mở
    }
    let tu = d
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .next()
        .unwrap_or("");
    if TU_KHOA_ITEM.contains(&tu) {
        return true;
    }
    // `let` **khai báo** chỉ khi có **chú thích kiểu** (`:` trước `=`); `let big = ls …` là
    // `shell_let` (ST-18) ⇒ vẫn là stage.
    if tu == "let" {
        return let_la_khai(d);
    }
    false
}

/// `let` là **khai báo** khi có chú thích kiểu (`:` trước `=`) **và** RHS là **biểu thức script**
/// (không `|`, không chuyển hướng `>`/`<`, không `;`). `let x: int = ls | grep foo` là shell-let
/// (golden `test/cmd/golden.cmd` ca 24) ⇒ vẫn là stage.
pub(crate) fn let_la_khai(d: &str) -> bool {
    let Some((trai, phai)) = d.split_once('=') else {
        return false;
    };
    if !trai.contains(':') {
        return false;
    }
    let rhs = phai.trim();
    !rhs.is_empty()
        && !rhs.contains('|')
        && !rhs.contains(';')
        && !rhs.contains('>')
        && !rhs.contains('<')
        && !rhs.contains('&')
}

/// Mốc byte bắt đầu **vùng stage**; `None` nếu cả tệp là khai báo (không có stage).
/// Trả `(mốc, có_khai_báo)`.
pub(crate) fn tach(text: &str) -> (usize, bool) {
    let mut co = false;
    let mut off = 0usize;
    for dong in text.split_inclusive('\n') {
        let than = dong.trim_end_matches(['\n', '\r']);
        if !la_khai_bao(than) {
            return (off, co);
        }
        // Chỉ dòng **cột 1** mới mở vùng khai báo; dòng thụt/trống **không** đổi `co`.
        let cot1 = !than.is_empty() && !than.starts_with([' ', '\t']) && !than.starts_with('#');
        if cot1 && !than.starts_with("//") {
            co = true;
        }
        off += dong.len();
    }
    (off, co)
}

/// Tìm **khai báo đứng sau stage**: dòng cột 1 trong hậu tố mở đầu bằng từ khoá item ⇒ `(line, col)`.
pub(crate) fn khai_sau_stage(text: &str, tu: usize) -> Option<(u32, u32)> {
    let mut dong = 1u32;
    // Đếm dòng tới mốc tách.
    for b in text[..tu].bytes() {
        if b == b'\n' {
            dong += 1;
        }
    }
    for d in text[tu..].split_inclusive('\n') {
        let than = d.trim_end_matches(['\n', '\r']);
        if !than.is_empty() && !than.starts_with(' ') && !than.starts_with('\t') {
            let tu_khoa = than
                .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .next()
                .unwrap_or("");
            let la_let_khai = tu_khoa == "let" && { let_la_khai(than) };
            if TU_KHOA_ITEM.contains(&tu_khoa) || la_let_khai {
                return Some((dong, 1));
            }
        }
        dong += 1;
    }
    None
}

/// Tên các khai báo **kiểu/hàm/lệnh** ở cấp tệp trong CST tiền tố — để kiểm trùng **prelude**.
pub(crate) fn ten_khai(cst: &Cst) -> Vec<(String, Span)> {
    use vis_syntax::Kind;
    let mut out = Vec::new();
    for n in cst.root.children() {
        let can = matches!(
            n.kind,
            NodeKind::TypeDecl | NodeKind::EnumDecl | NodeKind::CmdDecl | NodeKind::FnDecl
        );
        if !can {
            continue;
        }
        if let Some(t) = n
            .tokens(&cst.tokens)
            .find(|t| t.kind == Kind::Ident)
            .map(|t| t.text.clone())
        {
            out.push((t, n.span));
        }
    }
    out
}

impl<'a> super::Lowerer<'a> {
    /// Hạ **vùng khai báo** của `.vish` (ST-44): parse lại tiền tố bằng `Mode::Script` rồi
    /// `lower_into` vào **cùng** bảng ký hiệu/kiểu; chẩn đoán trùng **prelude** ⇒ `VS0302`,
    /// khai báo **sau** stage ⇒ `VS0201` kèm gợi ý.
    pub(crate) fn ha_tien_to_khai(&mut self, moc: usize, prelude: &[(String, crate::ty::RecId)]) {
        let prefix = &self.text[..moc.min(self.text.len())];
        let po = vis_syntax::parse::parse(prefix, vis_syntax::lex::Mode::Script);
        for d in &po.diagnostics {
            self.diags.push(d.clone());
        }
        // Trùng tên **prelude** (`FileEntry`/`ProcEntry`) ⇒ `VS0302`.
        let ten_prelude: Vec<&str> = prelude.iter().map(|(n, _)| n.as_str()).collect();
        for (ten, span) in ten_khai(&po.cst) {
            if ten_prelude.contains(&ten.as_str()) {
                let msg = format!("khai báo trùng tên prelude `{ten}`");
                self.diag(&vis_diag::NAME_CODES[1], msg, span);
            }
        }
        let lex = vis_syntax::lex::lex(prefix, vis_syntax::lex::Mode::Script);
        let mut opts = self.opts.clone();
        for (n, id) in prelude {
            opts.imported_types
                .push((n.clone(), crate::ty::Type::Record(*id)));
        }
        let out = crate::lower::lower_into(self.symbols, self.tbl, &po.cst, &lex, opts);
        self.items.extend(out.hir.items);
        self.stmts.extend(out.hir.stmts);
        self.diags.extend(out.diagnostics);
        if let Some((line, col)) = khai_sau_stage(self.text, moc) {
            let sp = super::Lowerer::span_dong_cot(self.text, line, col);
            self.diag(
                &vis_diag::SYNTAX_CODES[0],
                "khai báo phải đứng **trước** stage đầu tiên (`.vish`: item ở đầu tệp, rồi mới tới\
                 pipeline)",
                sp,
            );
        }
    }

    /// Đổi `(dòng, cột)` — cột đếm theo **Unicode scalar** như chẩn đoán — thành `Span` byte.
    pub(crate) fn span_dong_cot(text: &str, line: u32, col: u32) -> vis_syntax::Span {
        let mut off = 0usize;
        for (i, d) in text.split_inclusive('\n').enumerate() {
            if i as u32 + 1 == line {
                let mut byte = off;
                for (k, ch) in d.char_indices() {
                    if k as u32 + 1 == col {
                        return vis_syntax::Span::new(off + k, off + k + ch.len_utf8());
                    }
                    byte = off + k;
                }
                let _ = byte;
                return vis_syntax::Span::new(off, off + d.trim_end().len().max(1));
            }
            off += d.len();
        }
        vis_syntax::Span::new(0, 0)
    }
}
