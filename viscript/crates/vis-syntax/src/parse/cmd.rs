// SPDX-License-Identifier: MIT OR Apache-2.0
//! Parser **chế độ lệnh** (`Mode::Command`) — ST-06, Phụ lục A.7 + VD-40.
//!
//! Sinh **CmdCST chưa định kiểu** (giống `vis-syntax` giai đoạn vòng 1): từ trần, cờ, biến, glob
//! còn nguyên dạng; `cmd_lower` (vòng 2, VD-22) mới quyết kiểu theo chữ ký lệnh. Cây node dùng cho
//! CST lossless; sexp (quy ước ở `viscript/test/cmd/golden.cmd`) dựng song song để golden đối chiếu.
//!
//! # Chốt theo VD-40
//!
//! 1. Một dòng = `(items <pipeline> …)` khi có `;`; một pipeline trần giữ nguyên `(pipe …)`.
//!    Tệp nhiều dòng = `(shell <item>…)`; tệp **một** dòng ⇒ sexp của dòng đó.
//! 2. `-5` là **từ trần** `(word -5)`; `-l`/`--oneline`/`--` là cờ. `cmd_lower` phân biệt số/cờ.
//! 3. `[ab]` (lexer cắt `[`, `ab`, `]`) được **ghép** thành một `(word [ab])` khi các span **liền
//!    kề**; có khoảng trắng bên trong ⇒ lỗi. List literal chỉ trong `( … )`.
//! 4. `--` là `(flag --)`; `cmd_lower` hiểu là kết thúc cờ.
//! 5. `|>` ở chế độ lệnh ⇒ lỗi (không có luật sinh).
//!
//! Biểu thức trong `( … )` và thân closure được **cắt theo span** rồi parse lại bằng
//! `Mode::Script` (lexer chế độ lệnh không sinh token biểu thức như `+`, `>=`…), nên sexp bên
//! trong dùng đúng quy ước ST-05 (`(field x size)`, `(> a b)`…).

use super::{PResult, Parser};
use crate::cst::{Elem, NodeKind};
use crate::lex::token::{Kind, Span};

/// **ST-48**: kiểu thân `cmd` (xem `Parser::than_cmd_kiem`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KieuThan {
    Script,
    Command,
    Mixed,
}

/// Từ khoá **khai báo** ở cột 1 trong `.vish` (ST-44) — phần còn lại là stage.
pub(crate) const KHAI: [&str; 13] = [
    "type", "enum", "cmd", "fn", "use", "const", "trait", "impl", "alias", "newtype", "extern",
    "pub", "async",
];

impl<'a> Parser<'a> {
    fn tok_span(&self, e: &Elem) -> Span {
        match e {
            Elem::Token(i) => self.toks[*i as usize].span,
            Elem::Node(n) => n.span,
        }
    }

    fn text_at(&self, span: Span) -> String {
        self.text[span.start.min(self.text.len())..span.end.min(self.text.len())].to_string()
    }

    /// Toàn tệp chế độ lệnh: `(shell <item>*)`, mỗi dòng một item.
    pub fn cmd_file(&mut self) -> PResult<(crate::cst::Node, String)> {
        let mut elems = Vec::new();
        let mut items: Vec<String> = Vec::new();
        while !self.at_eof() {
            if self.at_kind(Kind::Newline) {
                elems.push(self.bump());
                continue;
            }
            // **ST-48**: thân lệnh của `cmd` là dòng **thụt** ⇒ lexer sinh `INDENT`/`DEDENT`;
            // chúng là dấu cấu trúc của **script**, không phải stage — bỏ qua ở chế độ lệnh.
            if self.at_kind(Kind::Indent) || self.at_kind(Kind::Dedent) {
                self.bump();
                continue;
            }
            let (e, s) = self.cmd_line()?;
            let vua_khai = matches!(&e, Elem::Node(n) if n.kind == NodeKind::CmdKhai);
            elems.push(e);
            items.push(s);
            if self.at_kind(Kind::Newline) {
                elems.push(self.bump());
            } else if vua_khai && !self.at_eof() {
                // **ST-48**: sau `cmd …:` (thân lệnh, có thể **rỗng**) — khối khai báo đã dừng sau
                // dòng `:`; dòng kế tiếp (thân thụt hoặc stage mới) do vòng lặp này parse tiếp.
                continue;
            } else if !self.at_eof() {
                return Err(self.err_here(format!(
                    "mong xuống dòng sau lệnh, thật `{}`",
                    self.tok().text
                )));
            }
        }
        let node = self.node(NodeKind::CmdShell, elems);
        let sexp = match items.len() {
            0 => "(shell)".to_string(),
            1 => items.pop().unwrap_or_default(),
            _ => format!("(shell {})", items.join(" ")),
        };
        Ok((node, sexp))
    }

    /// **ST-48**: phân loại thân `cmd` đang đứng ở dòng hiện tại (dòng `:` đầu tiên của nó).
    /// `Script` = thân script (ST-44 giữ nguyên); `Command` = thân một pipeline lệnh; `Mixed` =
    /// mơ hồ (nhiều dòng mà **cả** script **và** lệnh đều parse sạch) ⇒ tầng hạ báo `VS0201`.
    fn than_cmd_kiem(&self) -> (KieuThan, Vec<String>) {
        let start = self.tok().span.start;
        let dau = self.text[..start.min(self.text.len())]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        let sau_dong = self.text[dau..]
            .find('\n')
            .map(|i| dau + i + 1)
            .unwrap_or(self.text.len());
        let mut dong: Vec<String> = Vec::new();
        let mut p = sau_dong;
        while p < self.text.len() {
            let het = self.text[p..]
                .find('\n')
                .map(|i| p + i)
                .unwrap_or(self.text.len());
            let than = &self.text[p..het];
            if !than.trim().is_empty() && !than.starts_with([' ', '\t']) {
                break; // dòng cột 1 ⇒ hết thân
            }
            dong.push(than.to_string());
            p = if het < self.text.len() { het + 1 } else { het };
        }
        let khac: Vec<String> = dong
            .iter()
            .filter(|l| !l.trim().is_empty())
            .cloned()
            .collect();
        if khac.is_empty() {
            return (KieuThan::Command, khac); // thân rỗng ⇒ tầng hạ báo `VS0201`
        }
        let le = khac
            .iter()
            .map(|l| l.len() - l.trim_start().len())
            .min()
            .unwrap_or(0);
        let noi: Vec<String> = khac
            .iter()
            .map(|l| l.get(le..).unwrap_or(l).to_string())
            .collect();
        let than = noi.join("\n");
        let script = crate::parse::parse(&than, crate::lex::Mode::Script)
            .diagnostics
            .is_empty();
        let lenh = crate::parse::parse(&than, crate::lex::Mode::Command)
            .diagnostics
            .is_empty();
        let kieu = match (script, lenh) {
            (true, false) => KieuThan::Script,
            (false, _) => KieuThan::Command,
            (true, true) => {
                if noi.len() == 1 {
                    KieuThan::Script
                } else {
                    KieuThan::Mixed
                }
            }
        };
        (kieu, khac)
    }

    /// Văn bản **dòng hiện tại** (từ đầu dòng tới trước `\n`) — dùng cho luật `let` khai báo.
    fn cmd_dong_nay(&self) -> String {
        let start = self.tok().span.start;
        let base = self.text[..start.min(self.text.len())]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        let end = self.text[base..]
            .find('\n')
            .map(|i| base + i)
            .unwrap_or(self.text.len());
        self.text[base..end].to_string()
    }

    /// Khối khai báo `.vish`: gom token từ dòng cột 1 tới **hết** các dòng thụt (thân item), dừng
    /// trước dòng cột 1 kế tiếp. Không sinh sexp cú pháp (nội dung hạ bằng đường script).
    fn cmd_khai(&mut self) -> PResult<(Elem, String)> {
        // **ST-48**: `cmd` có thể có **thân lệnh** (pipeline) — thử parse thân như **script**;
        // hỏng ⇒ thân lệnh ⇒ khối khai báo **dừng sau dòng `:`**, các dòng thân do parser lệnh
        // chính parse thành stage (span tuyệt đối, cùng CST).
        if self.at_kind(Kind::Word) && self.tok().text == "cmd" {
            let (kieu, _) = self.than_cmd_kiem();
            if kieu != KieuThan::Script {
                let mut elems: Vec<Elem> = Vec::new();
                while !self.at_eof() {
                    let het = self.at_kind(Kind::Newline);
                    elems.push(self.bump());
                    if het {
                        break;
                    }
                }
                return Ok((
                    Elem::Node(Box::new(self.node(NodeKind::CmdKhai, elems))),
                    "(khai)".to_string(),
                ));
            }
        }
        let mut elems: Vec<Elem> = Vec::new();
        let mut sau_dong_dau = false;
        loop {
            if self.at_eof() {
                break;
            }
            if sau_dong_dau {
                let t = self.tok();
                if !self.at_kind(Kind::Newline) && t.col == 1 {
                    break;
                }
            }
            if self.at_kind(Kind::Newline) {
                // Kết thúc khối **trước** newline cuối nếu dòng kế tiếp là cột 1 (hoặc EOF) — để
                // `cmd_file` tự tiêu thụ newline như mọi dòng.
                let mut j = self.pos;
                while j < self.sig.len() && self.toks[self.sig[j] as usize].kind == Kind::Newline {
                    j += 1;
                }
                let het = match self.sig.get(j) {
                    None => true,
                    Some(&ti) => {
                        let t = &self.toks[ti as usize];
                        t.kind == Kind::Eof || t.col == 1
                    }
                };
                if het {
                    break;
                }
                elems.push(self.bump());
                sau_dong_dau = true;
                continue;
            }
            elems.push(self.bump());
        }
        Ok((
            Elem::Node(Box::new(self.node(NodeKind::CmdKhai, elems))),
            "(khai)".to_string(),
        ))
    }

    fn cmd_at_line_end(&self) -> bool {
        self.at_kind(Kind::Newline) || self.at_eof()
    }

    fn cmd_line(&mut self) -> PResult<(Elem, String)> {
        // ST-44: dòng **cột 1** mở đầu bằng từ khoá khai báo (hoặc `let`) ⇒ khối **raw**; nội dung
        // được parse lại bằng `Mode::Script` ở tầng hạ (tiền tố tệp). Không đổi `.vish` không khai báo.
        if self.at_kind(Kind::Word) && self.tok().col == 1 {
            let w = self.tok().text.clone();
            let let_khai = w == "let" && {
                let dong = self.cmd_dong_nay();
                match dong.split_once('=') {
                    None => false,
                    Some((trai, phai)) => {
                        let rhs = phai.trim();
                        trai.contains(':')
                            && !rhs.is_empty()
                            && !rhs.contains('|')
                            && !rhs.contains(';')
                            && !rhs.contains('>')
                            && !rhs.contains('<')
                            && !rhs.contains('&')
                            && crate::parse::parse(
                                &format!("let __x = {rhs}"),
                                crate::lex::Mode::Script,
                            )
                            .diagnostics
                            .is_empty()
                    }
                }
            };
            if KHAI.contains(&w.as_str()) || let_khai {
                return self.cmd_khai();
            }
        }
        // `let` = `shell_let` (chỉ ở đầu dòng, khi **không** phải khai báo cấp tệp).
        if self.at_kind(Kind::Word) && self.tok().text == "let" {
            let (e, s) = self.cmd_let()?;
            if !(self.cmd_at_line_end() || self.at_punct(";")) {
                return Err(self.err_here(format!(
                    "mong xuống dòng sau `let`, thật `{}`",
                    self.tok().text
                )));
            }
            return Ok((e, s));
        }
        let mut elems = Vec::new();
        let mut its = Vec::new();
        loop {
            let (e, s) = self.cmd_pipeline()?;
            elems.push(e);
            its.push(s);
            if self.at_punct(";") {
                elems.push(self.bump());
                if self.cmd_at_line_end() {
                    return Err(self.err_here("`;` không có vế sau"));
                }
                continue;
            }
            break;
        }
        if its.len() == 1 {
            let e = elems.pop().expect("một item");
            Ok((e, its.pop().unwrap_or_default()))
        } else {
            let node = self.node(NodeKind::CmdItems, elems);
            Ok((
                Elem::Node(Box::new(node)),
                format!("(items {})", its.join(" ")),
            ))
        }
    }

    fn cmd_pipeline(&mut self) -> PResult<(Elem, String)> {
        let mut elems = Vec::new();
        let mut ss = Vec::new();
        loop {
            let (e, s) = self.cmd_stage()?;
            elems.push(e);
            ss.push(s);
            if self.at_punct("|") {
                elems.push(self.bump());
                continue;
            }
            break;
        }
        let pipe_node = self.node(NodeKind::CmdPipe, elems);
        let pipe_sexp = format!("(pipe {})", ss.join(" "));
        if self.at_punct("&") {
            let amp = self.bump();
            if !self.cmd_at_line_end() {
                return Err(self.err_here("`&` nền chỉ được ở **cuối dòng** (VD-40 #1)"));
            }
            let e2 = vec![Elem::Node(Box::new(pipe_node)), amp];
            let n = self.node(NodeKind::CmdBg, e2);
            Ok((Elem::Node(Box::new(n)), format!("(bg {pipe_sexp})")))
        } else {
            Ok((Elem::Node(Box::new(pipe_node)), pipe_sexp))
        }
    }

    fn cmd_stage(&mut self) -> PResult<(Elem, String)> {
        let mut elems = Vec::new();
        let mut parts = Vec::new();
        let mut head_la_group = false;
        if self.at_punct("(") {
            let (e, s) = self.cmd_group()?;
            elems.push(e);
            parts.push(s);
            head_la_group = true;
        } else if self.at_kind(Kind::Caret) {
            let c = self.bump();
            elems.push(c);
            if !self.at_kind(Kind::Word) {
                return Err(self.err_here(format!(
                    "mong tên chương trình sau `^`, thật `{}`",
                    self.tok().text
                )));
            }
            let name = self.tok().text.clone();
            elems.push(self.bump());
            parts.push(format!("(ext {name})"));
        } else if self.at_kind(Kind::Dollar) {
            let (e, s) = self.cmd_dollar()?;
            elems.push(e);
            parts.push(s);
        } else if self.at_kind(Kind::Word) {
            let name = self.tok().text.clone();
            elems.push(self.bump());
            parts.push(format!("(cmd {name})"));
        } else {
            return Err(self.err_here(format!("mong lệnh, thật `{}`", self.tok().text)));
        }
        if head_la_group {
            // A.7 `stage := word_cmd arg* | '(' expr ')'` — nhóm không nhận tham số.
            if !(self.cmd_at_line_end()
                || self.at_punct("|")
                || self.at_punct(";")
                || self.at_punct("&")
                || self.at_punct("|>"))
            {
                return Err(
                    self.err_here("stage `( … )` không nhận tham số (A.7: nhóm là cả stage)")
                );
            }
        } else {
            while !self.cmd_at_line_end()
                && !self.at_punct("|")
                && !self.at_punct(";")
                && !self.at_punct("&")
                && !self.at_punct("|>")
            {
                let (e, s) = self.cmd_arg()?;
                elems.push(e);
                parts.push(s);
            }
        }
        let node = self.node(NodeKind::CmdStage, elems);
        Ok((
            Elem::Node(Box::new(node)),
            format!("(stage {})", parts.join(" ")),
        ))
    }

    fn cmd_arg(&mut self) -> PResult<(Elem, String)> {
        if self.at_punct("(") {
            return self.cmd_group();
        }
        if self.at_punct("[") {
            return self.cmd_word_run();
        }
        if self.at_kind(Kind::Dollar) {
            return self.cmd_dollar();
        }
        match self.tok().kind {
            Kind::Str
            | Kind::RawStr
            | Kind::MultiStr
            | Kind::RawMulti
            | Kind::Bytes
            | Kind::Path => {
                let t = self.tok().text.clone();
                Ok((self.bump(), format!("(str {t})")))
            }
            Kind::Word => {
                if self.cmd_closure_ahead() {
                    return self.cmd_closure();
                }
                self.cmd_word_run()
            }
            // `>` `<` … ra token Punct nhưng ở CmdCST **chưa định kiểu** vẫn là "từ" để
            // `cmd_lower` quyết (so sánh/redirect/…). Xem VD-40 + JOURNAL ST-06.
            Kind::Punct
                if matches!(
                    self.tok().text.as_str(),
                    ">" | "<" | ">=" | "<=" | "==" | "!="
                ) =>
            {
                let t = self.tok().text.clone();
                Ok((self.bump(), format!("(word {t})")))
            }
            _ => Err(self.err_here(format!("mong tham số, thật `{}`", self.tok().text))),
        }
    }

    fn cmd_let(&mut self) -> PResult<(Elem, String)> {
        let mut elems = vec![self.bump()]; // `let`
        if !self.at_kind(Kind::Word) {
            return Err(self.err_here("mong tên sau `let`"));
        }
        // Chế độ lệnh: `:` và `=` **không** phải metachar nên có thể dính trong WORD
        // (`x:` / `x` + `:` / `=`), hoặc đứng rời — A.7 `shell_let := 'let' IDENT [':' type] '=' pipeline`.
        let raw = self.tok().text.clone();
        let mut name = raw.clone();
        let mut co_type = false;
        if name.ends_with(':') {
            name = name.trim_end_matches(':').to_string();
            co_type = true;
        }
        elems.push(self.bump());
        if !co_type && self.at_kind(Kind::Word) && self.tok().text == ":" {
            elems.push(self.bump());
            co_type = true;
        }
        let mut ty: Option<String> = None;
        if co_type {
            let tstart = self.tok().span.start;
            let mut tend = tstart;
            let mut depth = 0usize;
            loop {
                if depth == 0 && self.at_kind(Kind::Word) && self.tok().text == "=" {
                    break;
                }
                if self.cmd_at_line_end() || self.at_eof() {
                    return Err(self.err_here("thiếu `=` trong `let` (A.7)"));
                }
                let t = self.tok();
                if t.is_punct("(") || t.is_punct("[") || t.is_punct("{") {
                    depth += 1;
                } else if t.is_punct(")") || t.is_punct("]") || t.is_punct("}") {
                    depth = depth.saturating_sub(1);
                }
                tend = t.span.end;
                elems.push(self.bump());
            }
            let text = self.text_at(Span::new(tstart, tend));
            ty = Some(text.trim().to_string());
        }
        if !(self.at_kind(Kind::Word) && self.tok().text == "=") {
            return Err(self.err_here(format!(
                "mong `=` sau tên trong `let`, thật `{}`",
                self.tok().text
            )));
        }
        elems.push(self.bump());
        let (pipe, pipe_sexp) = self.cmd_pipeline()?;
        elems.push(pipe);
        let node = self.node(NodeKind::CmdLet, elems);
        let sexp = match ty {
            None => format!("(let {name} {pipe_sexp})"),
            Some(t) => format!("(let {name} {t} {pipe_sexp})"),
        };
        Ok((Elem::Node(Box::new(node)), sexp))
    }

    fn cmd_dollar(&mut self) -> PResult<(Elem, String)> {
        let mut elems = vec![self.bump()]; // `$`
        let mut segs: Vec<String> = Vec::new();
        loop {
            if !(self.at_kind(Kind::Ident) || self.at_kind(Kind::Word)) {
                return Err(
                    self.err_here(format!("mong tên biến sau `$`, thật `{}`", self.tok().text))
                );
            }
            segs.push(self.tok().text.clone());
            elems.push(self.bump());
            if self.at_punct(".") {
                elems.push(self.bump());
                continue;
            }
            break;
        }
        let node = self.node(
            if segs.first().map(String::as_str) == Some("env") {
                NodeKind::CmdEnv
            } else {
                NodeKind::CmdVar
            },
            elems,
        );
        let sexp = if segs.len() == 2 && segs[0] == "env" {
            format!("(env {})", segs[1])
        } else {
            format!("(var {})", segs.join(" "))
        };
        Ok((Elem::Node(Box::new(node)), sexp))
    }

    fn cmd_group(&mut self) -> PResult<(Elem, String)> {
        let open = self.bump(); // `(`
        let open_span = self.tok_span(&open);
        let mut k = 0usize;
        let mut depth = 1usize;
        loop {
            let t = self.tok_at(k);
            if t.kind == Kind::Eof || t.kind == Kind::Newline {
                return Err(self.err_here("thiếu `)` đóng nhóm"));
            }
            if t.is_punct("(") {
                depth += 1;
            } else if t.is_punct(")") {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            k += 1;
        }
        let close = self.tok_at(k).span;
        let mut elems = vec![open];
        for _ in 0..k {
            elems.push(self.bump());
        }
        elems.push(self.bump()); // `)`
        let slice = self.text_at(Span::new(open_span.end, close.start));
        let slice = slice.trim();
        let sub = crate::parse::parse(slice, crate::lex::Mode::Script);
        if let Some(d) = sub.diagnostics.first() {
            // ST-46: nhóm có thể là **một lệnh** (`( ls ./photos )`) — parser **không** từ chối;
            // tầng hạ quyết định biểu thức hay lệnh. Nhóm nhiều stage (`( a | b )`) **vẫn** lỗi.
            if slice.contains('|') || slice.contains(';') || slice.contains('&') {
                return Err(self.err_here(format!("trong `( … )`: {}", d.message)));
            }
            let node = self.node(NodeKind::CmdGroup, elems);
            return Ok((
                Elem::Node(Box::new(node)),
                format!("(group raw {})", slice.replace(' ', "_")),
            ));
        }
        // ST-44: `()` **rỗng** hợp lệ làm dấu **lời gọi không đối số** (`f() | where …`); tầng hạ
        // quyết định nghĩa (bỏ qua như đối số rỗng). Nhóm rỗng **đứng một mình** vẫn bị hạ báo lỗi.
        let inner = match sub.cst.sexp_expr() {
            Some(x) => x,
            None if slice.is_empty() => "()".to_string(),
            None => return Err(self.err_here("`( … )` không phải một biểu thức")),
        };
        let node = self.node(NodeKind::CmdGroup, elems);
        Ok((Elem::Node(Box::new(node)), format!("(group {inner})")))
    }

    /// Một **từ** của chế độ lệnh: mọi token **liền kề span** trong vùng từ (`WORD`, `[`…`]`, …)
    /// gộp thành **một** `(word …)`; có khoảng trắng ⇒ tách (VD-40 #3, review vòng 1).
    fn cmd_word_run(&mut self) -> PResult<(Elem, String)> {
        let start = self.tok().span.start;
        let mut elems = Vec::new();
        let mut end = start;
        loop {
            if self.at_kind(Kind::Word) {
                let sp = self.tok().span;
                elems.push(self.bump());
                end = sp.end;
            } else if self.at_punct("[") {
                self.cmd_glob_group(&mut elems, &mut end)?;
            } else {
                break;
            }
            let t = self.tok();
            if t.kind == Kind::Eof || t.span.start != end {
                break;
            }
            if !(t.kind == Kind::Word || t.is_punct("[")) {
                break;
            }
        }
        let text = self.text_at(Span::new(start, end));
        let sexp = self.cmd_word_sexp(&text)?;
        let node = self.node(NodeKind::CmdWord, elems);
        Ok((Elem::Node(Box::new(node)), sexp))
    }

    /// `-l` / `--oneline` / `--` là cờ; `-5` là **từ trần**; `--x=expr` tách tại `=` **đầu tiên**.
    fn cmd_word_sexp(&mut self, text: &str) -> PResult<String> {
        let co_gach = text == "--"
            || (text.starts_with("--") && text.len() > 2)
            || (text.starts_with('-')
                && text.len() > 1
                && !text[1..].starts_with(|c: char| c.is_ascii_digit()));
        if co_gach {
            if let Some((name, val)) = text.split_once('=') {
                let v = self.sub_expr(val.trim())?;
                return Ok(format!("(flag {name} {v})"));
            }
            return Ok(format!("(flag {text})"));
        }
        Ok(format!("(word {text})"))
    }

    /// Cụm `[` … `]` (glob char-class / list?) — lexer cắt token; ghép khi span **liền kề**.
    fn cmd_glob_group(&mut self, elems: &mut Vec<Elem>, end: &mut usize) -> PResult<()> {
        let open = self.bump();
        let open_span = self.tok_span(&open);
        elems.push(open);
        *end = open_span.end;
        loop {
            let t = self.tok();
            if t.kind == Kind::Eof || t.kind == Kind::Newline {
                return Err(self.err_here("thiếu `]` đóng glob class"));
            }
            if t.span.start != *end {
                return Err(self.err_here("glob `[ … ]` không được có khoảng trắng bên trong"));
            }
            if t.is_punct("]") {
                elems.push(self.bump());
                *end = t.span.end;
                return Ok(());
            }
            if t.is_punct("(") || t.is_punct("{") || t.is_punct(";") || t.is_punct("|") {
                return Err(self.err_here(format!("glob class không hợp lệ: `{}`", t.text)));
            }
            elems.push(self.bump());
            *end = t.span.end;
        }
    }

    fn sub_expr(&mut self, slice: &str) -> PResult<String> {
        let sub = crate::parse::parse(slice, crate::lex::Mode::Script);
        if let Some(d) = sub.diagnostics.first() {
            return Err(self.err_here(format!("trong biểu thức: {}", d.message)));
        }
        sub.cst
            .sexp_expr()
            .ok_or_else(|| self.err_here("không phải biểu thức"))
    }

    fn cmd_closure_ahead(&self) -> bool {
        if !self.at_kind(Kind::Word) {
            return false;
        }
        if self.tok_at(1).kind == Kind::Word
            && self.tok_at(1).text == "="
            && self.tok_at(2).is_punct(">")
        {
            return true;
        }
        let t = &self.tok().text;
        t.ends_with('=')
            && t.len() > 1
            && self.tok_at(1).is_punct(">")
            && t[..t.len() - 1]
                .chars()
                .all(|c| c == '_' || c.is_alphanumeric())
    }

    fn cmd_closure(&mut self) -> PResult<(Elem, String)> {
        let mut elems = Vec::new();
        let first = self.bump();
        let raw = self.text_at(self.tok_span(&first));
        let name = raw.trim_end_matches('=').to_string();
        elems.push(first);
        if !raw.ends_with('=') {
            if self.at_kind(Kind::Word) && self.tok().text == "=" {
                elems.push(self.bump());
            } else {
                return Err(self.err_here("mong `=>` của closure"));
            }
        }
        let gt = self.expect_punct(">")?;
        let gt_span = self.tok_span(&gt);
        elems.push(gt);
        let mut depth = 0usize;
        let mut last_end = gt_span.end;
        while !self.at_eof() {
            let t = self.tok();
            if t.kind == Kind::Newline {
                break;
            }
            if depth == 0 && (t.is_punct("|") || t.is_punct(";") || t.is_punct("&")) {
                break;
            }
            // ST-39: cờ `--x` **không** thuộc thân closure: `sort_by x => x --desc` ⇒ thân `x`,
            // cờ `--desc` ở stage (nếu nuốt vào thân, bộ hạ đọc `x --desc` thành `-(-desc)`).
            if depth == 0 && t.text.starts_with("--") {
                break;
            }
            if t.is_punct("(") || t.is_punct("[") || t.is_punct("{") {
                depth += 1;
            } else if t.is_punct(")") || t.is_punct("]") || t.is_punct("}") {
                depth = depth.saturating_sub(1);
            }
            last_end = t.span.end;
            elems.push(self.bump());
        }
        if elems.len() <= 3 {
            return Err(self.err_here("closure thiếu thân"));
        }
        let slice = self.text_at(Span::new(gt_span.end, last_end));
        let slice = slice.trim();
        let sub = crate::parse::parse(slice, crate::lex::Mode::Script);
        if let Some(d) = sub.diagnostics.first() {
            return Err(self.err_here(format!("trong thân closure: {}", d.message)));
        }
        let body = sub
            .cst
            .sexp_expr()
            .ok_or_else(|| self.err_here("thân closure không phải biểu thức"))?;
        let node = self.node(NodeKind::CmdClosure, elems);
        Ok((
            Elem::Node(Box::new(node)),
            format!("(closure {name} {body})"),
        ))
    }
}
