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

/// `let` là **khai báo** khi RHS là **biểu thức script** — **một nguồn sự thật** ở
/// [`vis_syntax::let_la_khai`] (parser và tầng hạ dùng **cùng** luật; ST-59 bỏ đòi chú thích kiểu).
pub(crate) fn let_la_khai(d: &str) -> bool {
    vis_syntax::let_la_khai(d)
}

/// **ST-59**: thân `cmd` = khai báo + **một** pipeline (một nguồn: [`vis_syntax::than_khai_lenh`]).
pub(crate) fn than_khai_lenh(dong: &[String]) -> bool {
    vis_syntax::than_khai_lenh(dong)
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

/// **ST-48**: kiểu thân `cmd` — script (ST-44), lệnh (một pipeline), hay **trộn** (báo `VS0201`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KieuThan {
    Script,
    Command,
    Mixed,
    /// **ST-59**: **vùng khai báo** (`let …`) + **một** pipeline cuối.
    KhaiLenh,
}

/// **ST-48**: một `cmd` có thân không phải script. `dedent` = văn bản thân đã bỏ thụt chung;
/// `map[o]` = offset **tuyệt đối** trong tệp của byte `o` trong `dedent` (dùng dịch span token).
#[derive(Debug, Clone)]
pub(crate) struct ThanLenh {
    pub ten: String,
    pub kieu: KieuThan,
    /// Dòng thân **đã bỏ thụt chung** (chỉ dòng khác trắng).
    pub dong: Vec<String>,
    pub dedent: String,
    pub map: Vec<usize>,
    /// Vùng **tuyệt đối** của thân trong tệp (`[dau, cuoi)`).
    pub dau: usize,
    pub cuoi: usize,
    /// Đầu dòng `cmd …:` (để xoá cả khai báo khi thân rỗng).
    pub khai_dau: usize,
}

/// Quét các `cmd` cột 1 và phân loại thân. Chỉ trả mục **không** phải `Script` (thân script giữ
/// nguyên đường ST-44); thân rỗng cũng trả (⇒ `VS0201`).
pub(crate) fn than_cmd(text: &str) -> Vec<ThanLenh> {
    let mut ds = Vec::new();
    let mut p = 0usize;
    while p < text.len() {
        let het = text[p..].find('\n').map(|i| p + i).unwrap_or(text.len());
        let dong = &text[p..het];
        let la_cmd = !dong.starts_with([' ', '\t'])
            && dong
                .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .next()
                == Some("cmd")
            && dong.get(3..4) == Some(" ");
        if !la_cmd {
            p = (het + 1).min(text.len());
            continue;
        }
        let ten = dong[3..]
            .trim_start()
            .split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .next()
            .unwrap_or("")
            .to_string();
        // Gom **các dòng thân** (trắng hoặc thụt) ngay sau dòng `cmd`.
        let mut raw: Vec<(usize, usize)> = Vec::new();
        let mut q = (het + 1).min(text.len());
        while q < text.len() {
            let h = text[q..].find('\n').map(|i| q + i).unwrap_or(text.len());
            let l = &text[q..h];
            if !l.trim().is_empty() && !l.starts_with([' ', '\t']) {
                break;
            }
            raw.push((q, h));
            q = (h + 1).min(text.len());
        }
        let khac: Vec<(usize, usize)> = raw
            .iter()
            .copied()
            .filter(|(a, b)| !text[*a..*b].trim().is_empty())
            .collect();
        let le = khac
            .iter()
            .map(|(a, b)| text[*a..*b].len() - text[*a..*b].trim_start().len())
            .min()
            .unwrap_or(0);
        let mut dedent = String::new();
        let mut map: Vec<usize> = Vec::new();
        let mut vi_tri: Vec<Option<(usize, usize, usize)>> = Vec::new(); // (abs, indent, len)
        for &(a, b) in &khac {
            let l = &text[a..b];
            let ind = l.len() - l.trim_start().len();
            let noi = &l[le.min(ind)..];
            if !dedent.is_empty() {
                dedent.push('\n');
                map.push(b);
            }
            let goc = a + le.min(ind);
            vi_tri.push(Some((goc, ind, noi.len())));
            for k in 0..noi.len() {
                dedent.push(noi.as_bytes()[k] as char);
                map.push(goc + k);
            }
        }
        let cuoi_byte = map.last().copied().map(|x| x + 1).unwrap_or(het);
        let dong_dedent: Vec<String> = khac
            .iter()
            .map(|(a, b)| {
                let l = &text[*a..*b];
                let ind = l.len() - l.trim_start().len();
                l[le.min(ind)..].to_string()
            })
            .collect();
        let kieu = if dong_dedent.is_empty() {
            KieuThan::Command
        } else {
            let script = vis_syntax::parse(&dedent, vis_syntax::Mode::Script)
                .diagnostics
                .is_empty();
            let lenh = vis_syntax::parse(&dedent, vis_syntax::Mode::Command)
                .diagnostics
                .is_empty();
            // **Trộn** chỉ khi thân **không** parse được như script nhưng **có** dòng script-clean
            // (hoặc `let`/`return`) — ví dụ `let y = 1` + `ls .`; thân script hợp lệ (script=true)
            // giữ nguyên, kể cả khi vô tình parse được cả chế độ lệnh.
            let co_let = dong_dedent
                .iter()
                .any(|l| l.starts_with("let ") || l.starts_with("return "));
            let co_dong_script = dong_dedent.iter().any(|l| {
                vis_syntax::parse(l, vis_syntax::Mode::Script)
                    .diagnostics
                    .is_empty()
            });
            if crate::lower::khai::than_khai_lenh(&dong_dedent) {
                // **ST-59**: khai báo (`let …`) + **một** pipeline/biểu thức cuối ⇒ hạ **hai phần**
                // (không còn `VS0201`); nhiều pipeline vẫn rơi xuống `Mixed`/lỗi như cũ.
                KieuThan::KhaiLenh
            } else if script && lenh {
                if dong_dedent.len() == 1 || co_let {
                    KieuThan::Script
                } else {
                    KieuThan::Mixed
                }
            } else if lenh && (co_let || co_dong_script) {
                KieuThan::Mixed
            } else {
                match (script, lenh) {
                    (true, false) => KieuThan::Script,
                    (false, _) => KieuThan::Command,
                    (true, true) => KieuThan::Script,
                }
            }
        };
        let _ = vi_tri;
        if kieu != KieuThan::Script {
            ds.push(ThanLenh {
                ten,
                kieu,
                dong: dong_dedent,
                dedent,
                map,
                dau: raw.first().map(|(a, _)| *a).unwrap_or(het),
                cuoi: raw.last().map(|(_, b)| *b).unwrap_or(het),
                khai_dau: p,
            });
            let _ = cuoi_byte;
        }
        p = q;
    }
    ds
}

/// **ST-48**: bản sao tiền tố với thân-lệnh thay bằng `()` — **giữ nguyên byte** (newline giữ, ký
/// tự khác thành dấu cách) để span tuyệt đối không đổi khi parse Script tiền tố.
pub(crate) fn filler(text: &str, ds: &[ThanLenh]) -> String {
    let mut b: Vec<u8> = text.as_bytes().to_vec();
    for t in ds {
        if t.dong.is_empty() {
            // Thân rỗng: **xoá cả khai báo** khỏi tiền tố (giữ byte) — tầng hạ báo `VS0201` riêng.
            let a = t.khai_dau.min(b.len());
            let z = t.cuoi.max(a).min(b.len());
            for x in &mut b[a..z] {
                if *x != b'\n' && *x != b'\r' {
                    *x = b' ';
                }
            }
            continue;
        }
        let a = t.dau.min(b.len());
        let z = t.cuoi.min(b.len());
        if a >= z {
            continue;
        }
        for x in &mut b[a..z] {
            if *x != b'\n' && *x != b'\r' {
                *x = b' ';
            }
        }
        // Đặt `()` ngay sau thụt của dòng đầu thân.
        {
            let doc = &text[a..z];
            let ind = doc.len() - doc.trim_start().len();
            let at = a + ind.min(z - a - 1);
            b[at] = b'(';
            if at + 1 < z {
                b[at + 1] = b')';
            }
        }
    }
    String::from_utf8(b).unwrap_or_else(|_| text.to_string())
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
    pub(crate) fn ha_tien_to_khai(
        &mut self,
        moc: usize,
        prelude: &[(String, crate::ty::RecId)],
        prefix: &str,
    ) {
        let prefix = &prefix[..moc.min(prefix.len())];
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
