// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Sugar lệnh** (ST-38/ST-39/ST-45): vị từ ngầm `it`, closure-arg, khoá ngầm `sort_by`, tách stage.
//!
//! Tách khỏi `cmd.rs` (ST-45) để giữ **R1 ≤ 1.500 dòng/tệp** — cùng khuôn `chieu.rs` (ST-40b).

use vis_diag::{COMMAND_CODES, TYPE_CODES};
use vis_syntax::{Node, Span};

use super::cmd::ten_kieu;
use super::{FrameKind, SymKind};
use crate::hir::HExpr;
use crate::ty::Type;

impl<'a> super::Lowerer<'a> {
    pub(super) fn arg_start(&self, node: &Node) -> Option<usize> {
        // Parser có thể **không** sinh token cho mọi từ (vd `size > 5mb` thành redirect) ⇒ quét
        // thẳng văn bản: hết **tên lệnh**, bỏ khoảng trắng ⇒ đó là đầu vùng đối số.
        let ten = node.tokens(&self.cst.tokens).next()?;
        let mut i = ten.span.end;
        let bytes = self.text.as_bytes();
        while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
            i += 1;
        }
        Some(i)
    }

    /// `true` nếu `name` đã **khai** trong phạm vi hiện tại (biến/hàm/const/dựng sẵn).
    pub(super) fn da_khai(&self, name: &str) -> bool {
        self.symbols
            .syms
            .iter()
            .any(|s| s.name == name && s.kind != SymKind::Generic)
    }

    /// ST-40a: đăng ký `FileEntry`/`ProcEntry` vào bảng kiểu của tệp `.vish` **và** bảng tên.
    pub(crate) fn dang_ky_prelude(&mut self) -> Vec<(String, crate::ty::RecId)> {
        let mut file_id = None;
        let mut ds = Vec::new();
        for (ten, id) in crate::prelude::dang_ky_vish(self.tbl) {
            if ten == "FileEntry" {
                file_id = Some(id);
            }
            self.rec_names.push((ten.clone(), id));
            self.prelude_recs.push((ten.clone(), id));
            ds.push((ten, id));
        }
        // ST-42: `ls` trả `list[FileEntry]` — `RecId` cấp **theo tệp** nên phải **vá** ret của chữ ký
        // (bảng `BuiltinSigs` là tĩnh, không biết id).
        if let Some(id) = file_id {
            for s in self.cmd_sigs.iter_mut() {
                if s.name == "ls" {
                    s.ret = Type::List(Box::new(Type::Record(id)));
                }
            }
        }
        ds
    }

    /// Kết thúc stage (gặp `|`/`;`/`&` ở độ sâu 0, hoặc xuống dòng) — dùng cho sugar `sort_by`.
    pub(super) fn stage_end(&self, start: usize) -> usize {
        let bytes = self.text.as_bytes();
        let mut depth = 0i32;
        let mut i = start;
        while i < bytes.len() {
            match bytes[i] {
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => depth -= 1,
                b'|' | b';' | b'&' | b'\n' if depth <= 0 => return i,
                _ => {}
            }
            i += 1;
        }
        bytes.len()
    }

    /// ST-40a: kiểm **tĩnh** thân vị từ khi `it` có kiểu bản ghi **thật** (prelude):
    /// - `it.<trường>` không tồn tại ⇒ **`VS0605`**;
    /// - `it.<trường> <op> <literal>` mà kiểu trường ≠ kiểu literal ⇒ **`VS0412`**.
    ///
    /// Span **tuyệt đối** (văn bản thân bắt đầu ở `base`). Không kiểu được ⇒ bỏ qua (kiểm động như cũ).
    pub(super) fn kiem_it(&mut self, body: &str, base: usize, it_ty: &Type) {
        let Type::Record(id) = it_ty else { return };
        let Some(rec) = self.tbl.rec(*id).cloned() else {
            return;
        };
        let lexed = vis_syntax::lex(body, vis_syntax::Mode::Script);
        let toks: Vec<(String, usize, usize)> = lexed
            .tokens
            .iter()
            .map(|t| (t.text.clone(), t.span.start, t.span.end))
            .collect();
        let mut i = 0usize;
        while i < toks.len() {
            if toks[i].0 == "it" && toks.get(i + 1).map(|t| t.0 == ".").unwrap_or(false) {
                if let Some(f) = toks.get(i + 2) {
                    let span = Span::new(base + f.1, base + f.2);
                    match rec.fields.iter().find(|x| x.name == f.0) {
                        None => self.diag(
                            &COMMAND_CODES[4],
                            format!("`it` không có trường `{}`", f.0),
                            span,
                        ),
                        Some(fd) => {
                            let op = toks.get(i + 3).map(|t| t.0.as_str()).unwrap_or("");
                            if matches!(op, ">" | "<" | ">=" | "<=" | "==" | "!=") {
                                if let Some(lit) = toks.get(i + 4) {
                                    let lt = self.kieu_text(&lit.0);
                                    if !matches!(lt, Type::Error) && lt != fd.ty {
                                        self.diag(
                                            &TYPE_CODES[3],
                                            format!(
                                                "so sánh `it.{}` (`{}`) với `{}`",
                                                f.0,
                                                ten_kieu(&fd.ty),
                                                ten_kieu(&lt)
                                            ),
                                            Span::new(base + lit.1, base + lit.2),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                i += 1;
                continue;
            }
            i += 1;
        }
    }

    /// ST-38: **vị từ ngầm `it`** — `where size > 5mb` ⇒ `it => it.size > 5mb`.
    ///
    /// Luật (Agent 0 chốt): chỉ đổi **định danh trần chưa khai báo**, không đổi `it` viết tường minh,
    /// không đổi tên đã khai; không đổi luật nào khác. `where 1` vẫn là **lỗi kiểm tĩnh** (`VS0602`
    /// "đối số sai kiểu" vì `1` không phải hàm) — ghi JOURNAL.
    pub(super) fn ha_sugar(
        &mut self,
        start: usize,
        end_co: Option<usize>,
        pty: Type,
        rty: Type,
    ) -> Option<HExpr> {
        // Kết thúc **stage**: gặp `|`/`;`/`&` ở độ sâu 0, hoặc xuống dòng.
        let mut depth = 0i32;
        let mut end = self.text.len();
        let bytes = self.text.as_bytes();
        let mut i = start;
        while end == self.text.len() && i < bytes.len() {
            let c = bytes[i];
            match c {
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => depth -= 1,
                b'|' | b';' | b'&' if depth <= 0 => {
                    end = i;
                    break;
                }
                b'\n' if depth <= 0 => {
                    end = i;
                    break;
                }
                _ => {}
            }
            i += 1;
        }
        let end = end_co.unwrap_or(end);
        let text = self
            .text
            .get(start..end.min(self.text.len()))?
            .trim()
            .to_string();
        if text.is_empty() {
            return None;
        }
        let lexed = vis_syntax::lex(&text, vis_syntax::Mode::Script);
        let mut chen: Vec<usize> = Vec::new();
        let toks: Vec<(usize, String)> = lexed
            .tokens
            .iter()
            .map(|t| (t.span.start, t.text.clone()))
            .collect();
        for (i, (off, txt)) in toks.iter().enumerate() {
            if txt != "it" && self.da_khai(txt) {
                continue;
            }
            if txt == "it"
                || txt.is_empty()
                || !txt.starts_with(|c: char| c.is_alphabetic() || c == '_')
            {
                continue;
            }
            if matches!(txt.as_str(), "and" | "or" | "not" | "true" | "false") {
                continue;
            }
            if toks
                .get(i.wrapping_sub(1))
                .map(|(_, p)| p == ".")
                .unwrap_or(false)
            {
                continue;
            }
            if txt == "it" || self.da_khai(txt) {
                continue;
            }
            // `it` tường minh: `it.x` — không đổi; `f(` là lời gọi hàm — không đổi.
            if toks.get(i + 1).map(|(_, p)| p == "(").unwrap_or(false) {
                continue;
            }
            // `time.now()`, `it.is_file()` — tên **module/trường** (theo sau là `.`) ⇒ không đổi.
            if toks.get(i + 1).map(|(_, p)| p == ".").unwrap_or(false) {
                continue;
            }
            chen.push(*off);
        }
        let mut src_body = String::new();
        let mut last = 0usize;
        for off in &chen {
            src_body.push_str(&text[last..*off]);
            src_body.push_str("it.");
            last = *off;
        }
        src_body.push_str(&text[last..]);
        self.tam_n += 1;
        if matches!(&pty, Type::Record(_)) {
            self.kiem_it(&src_body, start, &pty);
        }
        let prefix = format!("let _c{} = it => ", self.tam_n);
        if start < prefix.len() {
            return None;
        }
        let pad = start - prefix.len();
        let src = format!("{prefix}{}{src_body}", " ".repeat(pad));
        let lexed2 = vis_syntax::lex(&src, vis_syntax::Mode::Script);
        let parsed = vis_syntax::parse(&src, vis_syntax::Mode::Script);
        self.push_frame(FrameKind::Fn, Span::new(start, end));
        let scope = self.scope.clone();
        let mut sub = super::Lowerer::new(
            &parsed.cst,
            &lexed2,
            self.opts.clone(),
            self.symbols,
            self.tbl,
        );
        sub.scope = scope;
        sub.run();
        let out = sub.finish();
        self.pop_frame();
        self.diags.extend(out.diagnostics);
        let init = match out.hir.stmts.first() {
            Some(crate::hir::HStmt::Let { init, .. }) => init.clone(),
            _ => return None,
        };
        match init {
            HExpr::Lambda {
                params, body, span, ..
            } => {
                let mut params = params;
                if let Some(p) = params.first_mut() {
                    p.ty = pty;
                }
                Some(HExpr::Lambda {
                    params,
                    ret: rty,
                    body,
                    span,
                })
            }
            _ => None,
        }
    }
}
