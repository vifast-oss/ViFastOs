// SPDX-License-Identifier: MIT OR Apache-2.0
//! Parser + CST lossless — ST-05 (tài liệu thiết kế Phụ lục A, A.1–A.3/A.5).
//!
//! API công khai: [`parse`] → [`ParseOutput`] (văn bản đã chuẩn hoá + CST + chẩn đoán).
//!
//! # Hợp đồng
//!
//! - **Lossless**: nguồn parse thành công ⇒ `cst.render(&out.text) == out.text` (khoảng trắng là
//!   gap giữa hai lá, chú thích được gắn tường minh — xem [`crate::cst`]).
//! - **Mỗi tệp lỗi đúng 1 lỗi đầu** (VD-35): parser dừng ngay chẩn đoán cú pháp đầu tiên ở cấp
//!   tệp, không báo dồn, không panic; CST vẫn chứa toàn bộ token để render lại được.
//! - Mã lỗi cú pháp lấy từ **sổ riêng** `vis_diag::SYNTAX_CODES` (`VS0201`, `VS0202`) — không
//!   sửa sổ từ vựng `CODES` (VD-35).
//!
//! Chế độ lệnh (`Mode::Command`) là ST-06: vòng này parser trả `Program` chứa nguyên token
//! (lossless) và **không** phát chẩn đoán cú pháp.

pub mod cmd;
pub mod expr;

use crate::cst::{Cst, Elem, Node, NodeKind};
use crate::lex::token::{Kind, Span, Token};
use crate::lex::{self, Diagnostic, LexOutput, Mode};

/// Kết quả một lượt parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseOutput {
    /// Văn bản đã chuẩn hoá (mốc của mọi `span`).
    pub text: String,
    pub cst: Cst,
    /// Chẩn đoán từ vựng + **tối đa một** chẩn đoán cú pháp (lỗi đầu).
    pub diagnostics: Vec<Diagnostic>,
}

/// Parse một văn bản nguồn theo chế độ.
pub fn parse(src: &str, mode: Mode) -> ParseOutput {
    let LexOutput {
        text,
        tokens,
        diagnostics,
    } = lex::lex(src, mode);
    let mut diags = diagnostics;
    let mut cmd_sexp = None;
    let root = if mode == Mode::Command {
        let mut p = Parser::new(&tokens, &text);
        match p.cmd_file() {
            Ok((n, s)) => {
                cmd_sexp = Some(s);
                n
            }
            Err(d) => {
                // Lỗi **tầng**: đã có chẩn đoán từ vựng thì không thêm lỗi cú pháp (mỗi tệp đúng
                // **1** lỗi đầu, VD-35) — chế độ lệnh giữ quy tắc này cho golden ST-06.
                if diags.is_empty() {
                    diags.push(d.0);
                }
                p.raw_program_from_current()
            }
        }
    } else {
        let mut p = Parser::new(&tokens, &text);
        match p.file() {
            Ok(n) => n,
            Err(d) => {
                diags.push(d.0);
                p.raw_program_from_current()
            }
        }
    };
    let mut cst = Cst::new(root, tokens);
    if let Some(s) = cmd_sexp {
        cst.set_cmd_sexp(s);
    }
    cst.attach_comments();
    ParseOutput {
        text,
        cst,
        diagnostics: diags,
    }
}

/// Bộ parse đệ quy xuống trên luồng token (bỏ chú thích khỏi luồng cú pháp; chúng được gắn vào
/// CST bằng [`Cst::attach_comments`]).
pub struct Parser<'a> {
    toks: &'a [Token],
    /// Văn bản nguồn đã NFC — cần cho chế độ lệnh (cắt biểu thức trong `( … )`, thân closure).
    text: &'a str,
    /// Chỉ số token **không phải chú thích** (kể cả `EOF` ở cuối).
    sig: Vec<u32>,
    pos: usize,
}

/// Lỗi cú pháp đầu tiên — chuyển thành [`Diagnostic`] ở [`parse`].
pub struct PErr(pub Diagnostic);

type PResult<T> = Result<T, PErr>;

impl<'a> Parser<'a> {
    fn new(toks: &'a [Token], text: &'a str) -> Self {
        let sig = toks
            .iter()
            .enumerate()
            .filter(|(_, t)| !matches!(t.kind, Kind::Comment | Kind::DocComment))
            .map(|(i, _)| i as u32)
            .collect();
        Self {
            toks,
            text,
            sig,
            pos: 0,
        }
    }

    // ----- truy cập token -----

    /// Token hiện tại (đã bỏ chú thích); `EOF` khi hết.
    fn tok(&self) -> &'a Token {
        self.tok_at(0)
    }

    fn tok_at(&self, k: usize) -> &'a Token {
        let idx = (self.pos + k).min(self.sig.len().saturating_sub(1));
        &self.toks[self.sig[idx] as usize]
    }

    fn peek_punct(&self, k: usize, s: &str) -> bool {
        self.tok_at(k).is_punct(s)
    }

    fn peek_kw(&self, k: usize, s: &str) -> bool {
        let t = self.tok_at(k);
        t.kind == Kind::Kw && t.text == s
    }

    fn at_eof(&self) -> bool {
        self.tok().kind == Kind::Eof
    }

    fn at_punct(&self, s: &str) -> bool {
        self.tok().is_punct(s)
    }

    fn at_kw(&self, s: &str) -> bool {
        self.tok().kind == Kind::Kw && self.tok().text == s
    }

    fn at_ident(&self, s: &str) -> bool {
        self.tok().kind == Kind::Ident && self.tok().text == s
    }

    fn at_kind(&self, k: Kind) -> bool {
        self.tok().kind == k
    }

    /// Chỉ số token gốc của vị trí hiện tại (để dựng lá CST).
    fn leaf(&self) -> u32 {
        let idx = self.pos.min(self.sig.len() - 1);
        self.sig[idx]
    }

    /// Ăn token hiện tại và trả lá CST tương ứng.
    fn bump(&mut self) -> Elem {
        let i = self.leaf();
        if self.pos + 1 < self.sig.len() {
            self.pos += 1;
        }
        Elem::Token(i)
    }

    fn eat_punct(&mut self, s: &str) -> Option<Elem> {
        if self.at_punct(s) {
            Some(self.bump())
        } else {
            None
        }
    }

    fn eat_kw(&mut self, s: &str) -> Option<Elem> {
        if self.at_kw(s) {
            Some(self.bump())
        } else {
            None
        }
    }

    /// Kind của token **đã ăn gần nhất** (bỏ chú thích); `None` khi chưa ăn gì.
    fn last_kind(&self) -> Option<Kind> {
        self.pos
            .checked_sub(1)
            .map(|i| self.toks[self.sig[i] as usize].kind)
    }

    /// Bắt buộc kết thúc dòng sau một câu lệnh/item (§A.2: `expr_stmt := expr NEWLINE`,
    /// `suite := simple_stmt NEWLINE`): ăn `NEWLINE` nếu có; chấp nhận khi câu lệnh vừa kết thúc
    /// bằng khối (`DEDENT`), khi **đang đứng trước `DEDENT`/EOF** (lexer sinh DEDENT ngầm ở EOF
    /// mà **không** sinh NEWLINE trước nó — SPEC-lex §4.6), hoặc khi token trước đã là
    /// `NEWLINE`/`DEDENT`. Nhiều hơn **một** `NEWLINE` liền nhau là lỗi (dòng trống không sinh
    /// token — SPEC-lex §4.2).
    fn require_line_end(&mut self, elems: &mut Vec<Elem>) -> PResult<()> {
        if self.at_kind(Kind::Newline) {
            elems.push(self.bump());
            return Ok(());
        }
        if self.at_eof()
            || self.at_kind(Kind::Dedent)
            || matches!(self.last_kind(), Some(Kind::Newline | Kind::Dedent))
        {
            return Ok(());
        }
        Err(self.err_here(format!(
            "mong xuống dòng sau câu lệnh, thật `{}`",
            self.tok().text
        )))
    }

    fn expect_punct(&mut self, s: &str) -> PResult<Elem> {
        if self.at_punct(s) {
            Ok(self.bump())
        } else {
            Err(self.err_here(format!("mong `{s}`, thật `{}`", self.tok().text)))
        }
    }

    fn expect_kw(&mut self, s: &str) -> PResult<Elem> {
        if self.at_kw(s) {
            Ok(self.bump())
        } else {
            Err(self.err_here(format!("mong từ khoá `{s}`, thật `{}`", self.tok().text)))
        }
    }

    fn expect_ident(&mut self) -> PResult<Elem> {
        if self.tok().kind == Kind::Ident {
            Ok(self.bump())
        } else {
            Err(self.err_here(format!("mong định danh, thật `{}`", self.tok().text)))
        }
    }

    fn expect_newline(&mut self) -> PResult<Elem> {
        if self.at_kind(Kind::Newline) || self.at_eof() {
            Ok(self.bump())
        } else {
            Err(self.err_here(format!("mong xuống dòng, thật `{}`", self.tok().text)))
        }
    }

    /// Ăn một trong các dấu câu (dùng cho nhóm toán tử cùng mức).
    fn eat_any_punct(&mut self, ops: &[&str]) -> Option<Elem> {
        if ops.iter().any(|s| self.at_punct(s)) {
            Some(self.bump())
        } else {
            None
        }
    }

    /// Token không mong đợi ⇒ VS0201.
    fn err_here(&mut self, msg: impl Into<String>) -> PErr {
        self.err_code(&vis_diag::SYNTAX_CODES[0], msg)
    }

    pub(crate) fn err_code(&self, code: &'static vis_diag::Code, msg: impl Into<String>) -> PErr {
        let t = self.tok();
        PErr(Diagnostic {
            code,
            message: msg.into(),
            span: t.span,
            line: t.line,
            col: t.col,
        })
    }

    // ----- dựng node -----

    fn node(&self, kind: NodeKind, elems: Vec<Elem>) -> Node {
        let span = self.elems_span(&elems);
        Node::new(kind, span, elems)
    }

    fn elems_span(&self, elems: &[Elem]) -> Span {
        let first = elems.iter().find_map(|e| self.elem_span(e));
        let last = elems.iter().rev().find_map(|e| self.elem_span(e));
        match (first, last) {
            (Some(a), Some(b)) => Span::new(a.start, b.end),
            _ => Span::new(self.tok().span.start, self.tok().span.start),
        }
    }

    fn elem_span(&self, e: &Elem) -> Option<Span> {
        match e {
            Elem::Token(i) => Some(self.toks[*i as usize].span),
            Elem::Node(n) => Some(n.span),
        }
    }

    fn push_node(elems: &mut Vec<Elem>, n: Node) {
        elems.push(Elem::Node(Box::new(n)));
    }

    /// `Program` chứa **toàn bộ** token còn lại (đường lỗi/`Mode::Command`): CST vẫn lossless.
    fn raw_program(&mut self) -> Node {
        let mut elems = Vec::new();
        while !self.at_eof() {
            elems.push(self.bump());
        }
        let span = self.elems_span(&elems);
        Node::new(NodeKind::Program, span, elems)
    }

    fn raw_program_from_current(&mut self) -> Node {
        self.raw_program()
    }

    // ----- A.1 khung tệp và item -----

    /// `file := NEWLINE* (item | stmt)* EOF`.
    ///
    /// A.1 viết `item*`, nhưng ví dụ tài liệu chạy được (dòng 290/342/535/549) đặt **câu lệnh**
    /// ở cấp tệp — script ViScript chạy được như vậy; nên cấp tệp nhận cả `stmt` của A.2.
    fn file(&mut self) -> PResult<Node> {
        let mut elems = Vec::new();
        while self.at_kind(Kind::Newline) {
            elems.push(self.bump());
        }
        while !self.at_eof() {
            if self.at_item_start() {
                let it = self.item()?;
                Self::push_node(&mut elems, it);
            } else {
                let s = self.stmt()?;
                Self::push_node(&mut elems, s);
            }
            self.require_line_end(&mut elems)?;
        }
        Ok(self.node(NodeKind::Program, elems))
    }

    /// Token bắt đầu một item A.1 (khác với câu lệnh).
    fn at_item_start(&self) -> bool {
        if self.at_ident("test") && self.peek_kw(1, "fn") {
            return true;
        }
        [
            "use", "fn", "cmd", "type", "enum", "trait", "impl", "alias", "newtype", "const",
            "extern", "pub", "async",
        ]
        .iter()
        .any(|k| self.at_kw(k))
    }

    fn item(&mut self) -> PResult<Node> {
        if self.at_ident("test") && self.peek_kw(1, "fn") {
            return self.test_decl();
        }
        let mut mods: Vec<Elem> = Vec::new();
        while self.at_kw("pub") || self.at_kw("async") {
            mods.push(self.bump());
        }
        if self.at_kw("fn") {
            return self.fn_decl(mods);
        }
        if self.at_kw("cmd") {
            return self.cmd_decl(mods);
        }
        if self.at_kw("type") {
            return self.type_decl(mods);
        }
        if self.at_kw("enum") {
            return self.enum_decl(mods);
        }
        if self.at_kw("trait") {
            return self.trait_decl(mods);
        }
        if self.at_kw("const") {
            return self.const_decl(mods);
        }
        if !mods.is_empty() {
            return Err(self.err_here(format!(
                "`pub`/`async` phải đứng trước fn/cmd/type/enum/trait/const, thật `{}`",
                self.tok().text
            )));
        }
        if self.at_kw("use") {
            return self.use_decl();
        }
        if self.at_kw("impl") {
            return self.impl_decl();
        }
        if self.at_kw("alias") {
            return self.alias_decl(NodeKind::AliasDecl);
        }
        if self.at_kw("newtype") {
            return self.alias_decl(NodeKind::NewtypeDecl);
        }
        if self.at_kw("extern") {
            return self.extern_decl();
        }
        Err(self.err_here(format!(
            "mong khai báo (use/fn/cmd/type/enum/trait/impl/alias/newtype/const/test/extern), thật `{}`",
            self.tok().text
        )))
    }

    fn use_decl(&mut self) -> PResult<Node> {
        let mut elems = vec![self.expect_kw("use")?];
        let p = self.dotted_path()?;
        Self::push_node(&mut elems, p);
        Ok(self.node(NodeKind::UseDecl, elems))
    }

    /// Đường dẫn `use`: `std.fs` (chấm) **hoặc** `./util`, `./a/b` (tương đối — ST-25).
    ///
    /// Cho phép `.`/`/` làm dấu phân cách; mở đầu bằng `.` (và tuỳ chọn `/`) nghĩa là **tệp tương đối**
    /// so với tệp đang `use`. Mọi dấu đều nằm trong node `DottedPath` ⇒ hạ đọc lại được văn bản gốc.
    fn dotted_path(&mut self) -> PResult<Node> {
        let mut elems = Vec::new();
        // Mở đầu tương đối: `.` hoặc `./`
        if self.at_punct(".") {
            elems.push(self.bump());
            if self.at_punct("/") {
                elems.push(self.bump());
            }
        }
        elems.push(self.expect_ident()?);
        while self.at_punct(".") || self.at_punct("/") {
            elems.push(self.bump());
            elems.push(self.expect_ident()?);
        }
        Ok(self.node(NodeKind::DottedPath, elems))
    }

    fn fn_decl(&mut self, mods: Vec<Elem>) -> PResult<Node> {
        let mut elems = mods;
        elems.push(self.expect_kw("fn")?);
        elems.push(self.expect_ident()?);
        if self.at_punct("[") {
            let g = self.generics()?;
            Self::push_node(&mut elems, g);
        }
        elems.push(self.expect_punct("(")?);
        if !self.at_punct(")") {
            let p = self.params()?;
            Self::push_node(&mut elems, p);
        }
        elems.push(self.expect_punct(")")?);
        if self.eat_punct("->").is_some() {
            let t = self.parse_type()?;
            Self::push_node(&mut elems, t);
        }
        elems.push(self.expect_punct(":")?);
        let s = self.suite()?;
        Self::push_node(&mut elems, s);
        Ok(self.node(NodeKind::FnDecl, elems))
    }

    fn cmd_decl(&mut self, mods: Vec<Elem>) -> PResult<Node> {
        let mut elems = mods;
        elems.push(self.expect_kw("cmd")?);
        elems.push(self.expect_ident()?);
        elems.push(self.expect_punct("(")?);
        if !self.at_punct(")") {
            let p = self.params()?;
            Self::push_node(&mut elems, p);
        }
        elems.push(self.expect_punct(")")?);
        if self.eat_punct("->").is_some() {
            let t = self.parse_type()?;
            Self::push_node(&mut elems, t);
        }
        elems.push(self.expect_punct(":")?);
        let s = self.suite()?;
        Self::push_node(&mut elems, s);
        Ok(self.node(NodeKind::CmdDecl, elems))
    }

    fn extern_decl(&mut self) -> PResult<Node> {
        let mut elems = vec![self.expect_kw("extern")?, self.expect_kw("cmd")?];
        elems.push(self.expect_ident()?);
        elems.push(self.expect_punct("(")?);
        if !self.at_punct(")") {
            let p = self.params()?;
            Self::push_node(&mut elems, p);
        }
        elems.push(self.expect_punct(")")?);
        Ok(self.node(NodeKind::ExternDecl, elems))
    }

    fn test_decl(&mut self) -> PResult<Node> {
        let mut elems = vec![self.bump()]; // `test` là IDENT (từ khoá theo ngữ cảnh)
        elems.push(self.expect_kw("fn")?);
        elems.push(self.expect_ident()?);
        elems.push(self.expect_punct("(")?);
        elems.push(self.expect_punct(")")?);
        elems.push(self.expect_punct(":")?);
        let s = self.suite()?;
        Self::push_node(&mut elems, s);
        Ok(self.node(NodeKind::TestDecl, elems))
    }

    fn type_decl(&mut self, mods: Vec<Elem>) -> PResult<Node> {
        let mut elems = mods;
        elems.push(self.expect_kw("type")?);
        elems.push(self.expect_ident()?);
        if self.at_punct("[") {
            let g = self.generics()?;
            Self::push_node(&mut elems, g);
        }
        if self.at_kw("derives") {
            let d = self.derives()?;
            Self::push_node(&mut elems, d);
        }
        elems.push(self.expect_punct(":")?);
        elems.push(self.expect_newline()?);
        elems.push(self.expect_indent()?);
        while !self.at_kind(Kind::Dedent) && !self.at_eof() {
            let f = self.type_field()?;
            Self::push_node(&mut elems, f);
            self.require_line_end(&mut elems)?;
        }
        if let Some(d) = self.take_dedent() {
            elems.push(d);
        }
        Ok(self.node(NodeKind::TypeDecl, elems))
    }

    fn type_field(&mut self) -> PResult<Node> {
        let mut elems = vec![self.expect_ident()?, self.expect_punct(":")?];
        let t = self.parse_type()?;
        Self::push_node(&mut elems, t);
        if self.eat_punct("=").is_some() {
            let v = self.expr()?;
            Self::push_node(&mut elems, v);
        }
        Ok(self.node(NodeKind::TypeField, elems))
    }

    fn enum_decl(&mut self, mods: Vec<Elem>) -> PResult<Node> {
        let mut elems = mods;
        elems.push(self.expect_kw("enum")?);
        elems.push(self.expect_ident()?);
        if self.at_punct("[") {
            let g = self.generics()?;
            Self::push_node(&mut elems, g);
        }
        if self.at_kw("derives") {
            let d = self.derives()?;
            Self::push_node(&mut elems, d);
        }
        elems.push(self.expect_punct(":")?);
        elems.push(self.expect_newline()?);
        elems.push(self.expect_indent()?);
        while !self.at_kind(Kind::Dedent) && !self.at_eof() {
            let v = self.variant()?;
            Self::push_node(&mut elems, v);
            self.require_line_end(&mut elems)?;
        }
        if let Some(d) = self.take_dedent() {
            elems.push(d);
        }
        Ok(self.node(NodeKind::EnumDecl, elems))
    }

    fn variant(&mut self) -> PResult<Node> {
        let mut elems = vec![self.expect_ident()?];
        if self.at_punct("(") {
            elems.push(self.bump());
            while !self.at_punct(")") {
                let mut f = vec![self.expect_ident()?, self.expect_punct(":")?];
                let t = self.parse_type()?;
                Self::push_node(&mut f, t);
                Self::push_node(&mut elems, self.node(NodeKind::VariantField, f));
                if self.eat_punct(",").is_none() {
                    break;
                }
            }
            elems.push(self.expect_punct(")")?);
        }
        Ok(self.node(NodeKind::Variant, elems))
    }

    fn derives(&mut self) -> PResult<Node> {
        let mut elems = vec![self.bump()]; // derives
        let p = self.dotted_path()?;
        Self::push_node(&mut elems, p);
        while self.eat_punct(",").is_some() {
            let p = self.dotted_path()?;
            Self::push_node(&mut elems, p);
        }
        Ok(self.node(NodeKind::Derives, elems))
    }

    fn alias_decl(&mut self, kind: NodeKind) -> PResult<Node> {
        let mut elems = vec![self.bump(), self.expect_ident()?, self.expect_punct("=")?];
        let t = self.parse_type()?;
        Self::push_node(&mut elems, t);
        Ok(self.node(kind, elems))
    }

    fn trait_decl(&mut self, mods: Vec<Elem>) -> PResult<Node> {
        let mut elems = mods;
        elems.push(self.expect_kw("trait")?);
        elems.push(self.expect_ident()?);
        if self.at_punct("[") {
            let g = self.generics()?;
            Self::push_node(&mut elems, g);
        }
        elems.push(self.expect_punct(":")?);
        // `trait T: A, B:` — danh sách siêu trait trước dấu `:` cuối.
        if !self.at_kind(Kind::Newline) {
            loop {
                let p = self.dotted_path()?;
                Self::push_node(&mut elems, p);
                if self.eat_punct(",").is_none() {
                    break;
                }
            }
            elems.push(self.expect_punct(":")?);
        }
        elems.push(self.expect_newline()?);
        elems.push(self.expect_indent()?);
        while !self.at_kind(Kind::Dedent) && !self.at_eof() {
            let it = self.trait_item()?;
            Self::push_node(&mut elems, it);
            self.require_line_end(&mut elems)?;
        }
        if let Some(d) = self.take_dedent() {
            elems.push(d);
        }
        Ok(self.node(NodeKind::TraitDecl, elems))
    }

    fn trait_item(&mut self) -> PResult<Node> {
        let mut elems = Vec::new();
        if self.at_kw("pub") {
            elems.push(self.bump());
        }
        elems.push(self.expect_kw("fn")?);
        elems.push(self.expect_ident()?);
        elems.push(self.expect_punct("(")?);
        if !self.at_punct(")") {
            let p = self.params()?;
            Self::push_node(&mut elems, p);
        }
        elems.push(self.expect_punct(")")?);
        if self.eat_punct("->").is_some() {
            let t = self.parse_type()?;
            Self::push_node(&mut elems, t);
        }
        if self.eat_punct(":").is_some() {
            let s = self.suite()?;
            Self::push_node(&mut elems, s);
        }
        Ok(self.node(NodeKind::TraitItem, elems))
    }

    fn impl_decl(&mut self) -> PResult<Node> {
        let mut elems = vec![self.expect_kw("impl")?];
        if self.at_kind(Kind::Ident) && self.peek_kw(1, "for") {
            let p = self.dotted_path()?;
            Self::push_node(&mut elems, p);
            elems.push(self.expect_kw("for")?);
        }
        let t = self.parse_type()?;
        Self::push_node(&mut elems, t);
        if self.at_punct("[") {
            let g = self.generics()?;
            Self::push_node(&mut elems, g);
        }
        elems.push(self.expect_punct(":")?);
        elems.push(self.expect_newline()?);
        elems.push(self.expect_indent()?);
        while !self.at_kind(Kind::Dedent) && !self.at_eof() {
            let m = self.fn_decl(Vec::new())?;
            Self::push_node(&mut elems, m);
            self.require_line_end(&mut elems)?;
        }
        if let Some(d) = self.take_dedent() {
            elems.push(d);
        }
        Ok(self.node(NodeKind::ImplDecl, elems))
    }

    fn const_decl(&mut self, mods: Vec<Elem>) -> PResult<Node> {
        let mut elems = mods;
        elems.push(self.expect_kw("const")?);
        elems.push(self.expect_ident()?);
        elems.push(self.expect_punct(":")?);
        let t = self.parse_type()?;
        Self::push_node(&mut elems, t);
        elems.push(self.expect_punct("=")?);
        let v = self.expr()?;
        Self::push_node(&mut elems, v);
        Ok(self.node(NodeKind::ConstDecl, elems))
    }

    // ----- tham số, generic, kiểu -----

    fn params(&mut self) -> PResult<Node> {
        let mut elems = Vec::new();
        loop {
            let p = self.param()?;
            Self::push_node(&mut elems, p);
            if self.eat_punct(",").is_none() {
                break;
            }
        }
        Ok(self.node(NodeKind::Params, elems))
    }

    fn param(&mut self) -> PResult<Node> {
        let mut elems = Vec::new();
        // ST-27: `...rest: str` — lexer không có token `...` (`..` rồi `.`), nhận cả hai.
        if self.at_punct("..") {
            elems.push(self.bump());
            if self.at_punct(".") {
                elems.push(self.bump());
            }
        }
        if self.at_kw("var") && self.peek_kw(1, "self") {
            elems.push(self.bump());
            elems.push(self.bump());
            return Ok(self.node(NodeKind::Param, elems));
        }
        if self.at_kw("self") {
            elems.push(self.bump());
            return Ok(self.node(NodeKind::Param, elems));
        }
        // Tham số có tên: `--ten` (lexer tách `-` `-` vì §1 không có token `--`).
        if self.at_punct("-") && self.peek_punct(1, "-") {
            elems.push(self.bump());
            elems.push(self.bump());
        }
        elems.push(self.expect_ident()?);
        elems.push(self.expect_punct(":")?);
        let t = self.parse_type()?;
        Self::push_node(&mut elems, t);
        // `@ path_lit` (capability) và danh sách quyền `(write)`.
        if self.eat_punct("@").is_some() {
            let t = self.tok();
            if matches!(t.kind, Kind::Path | Kind::Str) {
                elems.push(self.bump());
            } else {
                return Err(self.err_here("`@` phải theo sau là literal đường dẫn"));
            }
        }
        if self.at_punct("(") {
            let g = self.grant_list()?;
            Self::push_node(&mut elems, g);
        }
        if self.eat_punct("=").is_some() {
            let v = self.expr()?;
            Self::push_node(&mut elems, v);
        }
        Ok(self.node(NodeKind::Param, elems))
    }

    fn grant_list(&mut self) -> PResult<Node> {
        let mut elems = vec![self.expect_punct("(")?];
        loop {
            let p = self.dotted_path()?;
            Self::push_node(&mut elems, p);
            if self.eat_punct(",").is_none() {
                break;
            }
        }
        elems.push(self.expect_punct(")")?);
        Ok(self.node(NodeKind::GrantList, elems))
    }

    fn generics(&mut self) -> PResult<Node> {
        let mut elems = vec![self.expect_punct("[")?];
        loop {
            let mut g = vec![self.expect_ident()?];
            if self.eat_punct(":").is_some() {
                if self.at_kw("has") {
                    g.push(self.bump());
                    let rt = self.record_type()?;
                    Self::push_node(&mut g, rt);
                } else {
                    let p = self.dotted_path()?;
                    Self::push_node(&mut g, p);
                }
            }
            Self::push_node(&mut elems, self.node(NodeKind::Generic, g));
            if self.eat_punct(",").is_none() {
                break;
            }
        }
        elems.push(self.expect_punct("]")?);
        Ok(self.node(NodeKind::Generics, elems))
    }

    fn record_type(&mut self) -> PResult<Node> {
        let mut elems = vec![self.expect_punct("{")?];
        while !self.at_punct("}") {
            let mut f = vec![self.expect_ident()?, self.expect_punct(":")?];
            let t = self.parse_type()?;
            Self::push_node(&mut f, t);
            Self::push_node(&mut elems, self.node(NodeKind::TypeField, f));
            if self.eat_punct(",").is_none() {
                break;
            }
        }
        elems.push(self.expect_punct("}")?);
        Ok(self.node(NodeKind::Type, elems))
    }

    /// `type := type_primary '?'?` (A.5); trả `NodeKind::Type` bọc toàn bộ.
    pub(crate) fn parse_type(&mut self) -> PResult<Node> {
        let mut elems = Vec::new();
        let t = self.tok();
        match t.kind {
            Kind::Ident => {
                let p = self.dotted_path()?;
                Self::push_node(&mut elems, p);
                if self.at_punct("[") {
                    let mut g = vec![self.bump()];
                    loop {
                        let ty = self.parse_type()?;
                        Self::push_node(&mut g, ty);
                        if self.eat_punct(",").is_none() {
                            break;
                        }
                    }
                    g.push(self.expect_punct("]")?);
                    Self::push_node(&mut elems, self.node(NodeKind::Generics, g));
                }
            }
            Kind::Kw if t.text == "ref" || t.text == "weak" => {
                elems.push(self.bump());
                elems.push(self.expect_punct("[")?);
                let inner = self.parse_type()?;
                Self::push_node(&mut elems, inner);
                elems.push(self.expect_punct("]")?);
            }
            Kind::Kw if t.text == "cap" => {
                elems.push(self.bump());
                elems.push(self.expect_punct("[")?);
                let p = self.dotted_path()?;
                Self::push_node(&mut elems, p);
                elems.push(self.expect_punct("]")?);
            }
            Kind::Kw if t.text == "fn" => {
                elems.push(self.bump());
                elems.push(self.expect_punct("(")?);
                if !self.at_punct(")") {
                    loop {
                        let ty = self.parse_type()?;
                        Self::push_node(&mut elems, ty);
                        if self.eat_punct(",").is_none() {
                            break;
                        }
                    }
                }
                elems.push(self.expect_punct(")")?);
                elems.push(self.expect_punct("->")?);
                let ty = self.parse_type()?;
                Self::push_node(&mut elems, ty);
            }
            Kind::Punct if t.text == "(" => {
                elems.push(self.bump());
                if !self.at_punct(")") {
                    loop {
                        let ty = self.parse_type()?;
                        Self::push_node(&mut elems, ty);
                        if self.eat_punct(",").is_none() {
                            break;
                        }
                    }
                }
                elems.push(self.expect_punct(")")?);
            }
            Kind::Punct if t.text == "{" => {
                let rt = self.record_type()?;
                elems.extend(rt.elems);
            }
            _ => {
                return Err(self.err_here(format!(
                    "mong kiểu, thật `{}` ({})",
                    t.text,
                    t.kind.name()
                )));
            }
        }
        if self.at_punct("?") {
            elems.push(self.bump());
        }
        Ok(self.node(NodeKind::Type, elems))
    }

    // ----- A.4 pattern -----

    pub(crate) fn parse_pattern(&mut self) -> PResult<Node> {
        let mut elems = Vec::new();
        let first = self.range_pattern()?;
        Self::push_node(&mut elems, first);
        while self.at_punct("|") {
            elems.push(self.bump());
            let r = self.range_pattern()?;
            Self::push_node(&mut elems, r);
        }
        Ok(self.node(NodeKind::Pattern, elems))
    }

    fn range_pattern(&mut self) -> PResult<Node> {
        let p = self.atom_pattern()?;
        if self.at_punct("..") || self.at_punct("..=") {
            let mut elems = vec![Elem::Node(Box::new(p))];
            elems.push(self.bump());
            if !matches!(self.tok().kind, Kind::Newline | Kind::Eof | Kind::Dedent)
                && !self.at_punct(":")
                && !self.at_punct(")")
                && !self.at_punct("]")
                && !self.at_punct("}")
                && !self.at_punct(",")
                && !self.at_punct("|")
                && !self.at_kw("if")
                && !self.at_kw("in")
            {
                let q = self.atom_pattern()?;
                Self::push_node(&mut elems, q);
            }
            return Ok(self.node(NodeKind::Pattern, elems));
        }
        Ok(p)
    }

    fn atom_pattern(&mut self) -> PResult<Node> {
        let t = self.tok();
        match t.kind {
            Kind::Ident => {
                let mut elems = vec![self.bump()];
                // biến thể có trường: `Circle(r)`
                if self.at_punct("(") {
                    elems.push(self.bump());
                    while !self.at_punct(")") {
                        let p = self.parse_pattern()?;
                        Self::push_node(&mut elems, p);
                        if self.eat_punct(",").is_none() {
                            break;
                        }
                    }
                    elems.push(self.expect_punct(")")?);
                }
                Ok(self.node(NodeKind::Pattern, elems))
            }
            Kind::Kw if t.text == "none" || t.text == "true" || t.text == "false" => {
                let e = self.bump();
                Ok(self.node(NodeKind::Pattern, vec![e]))
            }
            Kind::Int
            | Kind::Float
            | Kind::Char
            | Kind::Str
            | Kind::RawStr
            | Kind::MultiStr
            | Kind::RawMulti
            | Kind::Bytes
            | Kind::Size
            | Kind::Duration
            | Kind::Percent => {
                let e = self.bump();
                Ok(self.node(NodeKind::Pattern, vec![e]))
            }
            Kind::Punct if t.text == "-" => {
                let mut elems = vec![self.bump()];
                if self.at_kind(Kind::Int) {
                    elems.push(self.bump());
                }
                Ok(self.node(NodeKind::Pattern, elems))
            }
            Kind::Punct if t.text == "(" => {
                let mut elems = vec![self.bump()];
                if !self.at_punct(")") {
                    loop {
                        let p = self.parse_pattern()?;
                        Self::push_node(&mut elems, p);
                        if self.eat_punct(",").is_none() {
                            break;
                        }
                    }
                }
                elems.push(self.expect_punct(")")?);
                Ok(self.node(NodeKind::Pattern, elems))
            }
            Kind::Punct if t.text == "[" => {
                let mut elems = vec![self.bump()];
                while !self.at_punct("]") {
                    if self.at_punct("..") {
                        elems.push(self.bump());
                        if self.tok().kind == Kind::Ident {
                            elems.push(self.bump());
                        }
                        break;
                    }
                    let p = self.parse_pattern()?;
                    Self::push_node(&mut elems, p);
                    if self.eat_punct(",").is_none() {
                        break;
                    }
                }
                elems.push(self.expect_punct("]")?);
                Ok(self.node(NodeKind::Pattern, elems))
            }
            Kind::Punct if t.text == "{" => {
                let mut elems = vec![self.bump()];
                while !self.at_punct("}") {
                    if self.at_punct("..") {
                        elems.push(self.bump());
                        break;
                    }
                    elems.push(self.expect_ident()?);
                    if self.eat_punct(":").is_some() {
                        elems.push(self.expect_punct(":")?);
                        let p = self.parse_pattern()?;
                        Self::push_node(&mut elems, p);
                    }
                    if self.eat_punct(",").is_none() {
                        break;
                    }
                }
                elems.push(self.expect_punct("}")?);
                Ok(self.node(NodeKind::Pattern, elems))
            }
            _ => Err(self.err_here(format!(
                "mong pattern, thật `{}` ({})",
                t.text,
                t.kind.name()
            ))),
        }
    }

    // ----- A.2 khối và câu lệnh -----

    fn expect_indent(&mut self) -> PResult<Elem> {
        if self.at_kind(Kind::Indent) {
            Ok(self.bump())
        } else {
            Err(self.err_here(format!(
                "mong khối thụt lề (INDENT), thật `{}`",
                self.tok().text
            )))
        }
    }

    fn take_dedent(&mut self) -> Option<Elem> {
        if self.at_kind(Kind::Dedent) {
            Some(self.bump())
        } else {
            None
        }
    }

    /// `suite := simple_stmt NEWLINE | NEWLINE INDENT stmt+ DEDENT`.
    ///
    /// Khối một dòng nội tuyến (`if x: a else: b`) dừng ở `else`/`elif` cùng dòng — Phụ lục B
    /// dùng dạng này; A.2 viết `simple_stmt NEWLINE` nên đây là chỗ nới tối thiểu, ghi ở JOURNAL.
    fn suite(&mut self) -> PResult<Node> {
        let mut elems = Vec::new();
        if self.at_kind(Kind::Newline) {
            elems.push(self.bump());
            elems.push(self.expect_indent()?);
            while !self.at_kind(Kind::Dedent) && !self.at_eof() {
                let s = self.stmt()?;
                Self::push_node(&mut elems, s);
                self.require_line_end(&mut elems)?;
            }
            if let Some(d) = self.take_dedent() {
                elems.push(d);
            }
        } else {
            let s = self.simple_stmt()?;
            Self::push_node(&mut elems, s);
            if self.at_kind(Kind::Newline) {
                elems.push(self.bump());
            } else if self.at_eof()
                || self.at_kind(Kind::Dedent)
                || self.at_kw("else")
                || self.at_kw("elif")
            {
                // nhánh một dòng nội tuyến / khối đóng ở EOF (DEDENT ngầm)
            } else {
                return Err(self.err_here(format!(
                    "mong xuống dòng cuối khối, thật `{}`",
                    self.tok().text
                )));
            }
        }
        Ok(self.node(NodeKind::Suite, elems))
    }

    fn simple_stmt(&mut self) -> PResult<Node> {
        match self.tok().kind {
            Kind::Kw => match self.tok().text.as_str() {
                "return" => self.return_stmt(),
                "break" => self.break_stmt(),
                "continue" => self.continue_stmt(),
                "let" => self.let_stmt(),
                "var" => self.var_stmt(),
                _ => self.expr_stmt(),
            },
            _ => self.expr_stmt(),
        }
    }

    fn stmt(&mut self) -> PResult<Node> {
        match self.tok().kind {
            Kind::Kw => match self.tok().text.as_str() {
                "let" => self.let_stmt(),
                "var" => self.var_stmt(),
                "return" => self.return_stmt(),
                "break" => self.break_stmt(),
                "continue" => self.continue_stmt(),
                "for" => self.for_stmt(),
                "while" => self.while_stmt(),
                "loop" => self.loop_stmt(),
                "scope" => self.scope_stmt(),
                "if" => self.if_stmt(),
                "match" => self.match_stmt(),
                _ => self.expr_stmt(),
            },
            _ => self.expr_stmt(),
        }
    }

    fn let_stmt(&mut self) -> PResult<Node> {
        let mut elems = vec![self.expect_kw("let")?];
        let p = self.parse_pattern()?;
        Self::push_node(&mut elems, p);
        if self.eat_punct(":").is_some() {
            let t = self.parse_type()?;
            Self::push_node(&mut elems, t);
        }
        elems.push(self.expect_punct("=")?);
        let v = self.expr()?;
        Self::push_node(&mut elems, v);
        // `else [IDENT]: suite` — giá trị dự phòng khi lỗi (A.2).
        if self.at_kw("else") {
            elems.push(self.bump());
            if self.tok().kind == Kind::Ident && self.peek_punct(1, ":") {
                elems.push(self.bump());
            }
            elems.push(self.expect_punct(":")?);
            let s = self.suite()?;
            Self::push_node(&mut elems, s);
            return Ok(self.node(NodeKind::LetStmt, elems));
        }
        Ok(self.node(NodeKind::LetStmt, elems))
    }

    fn var_stmt(&mut self) -> PResult<Node> {
        let mut elems = vec![self.expect_kw("var")?];
        if self.at_kw("self") {
            elems.push(self.bump());
        } else {
            elems.push(self.expect_ident()?);
        }
        if self.eat_punct(":").is_some() {
            let t = self.parse_type()?;
            Self::push_node(&mut elems, t);
        }
        elems.push(self.expect_punct("=")?);
        let v = self.expr()?;
        Self::push_node(&mut elems, v);
        Ok(self.node(NodeKind::VarStmt, elems))
    }

    fn return_stmt(&mut self) -> PResult<Node> {
        let mut elems = vec![self.expect_kw("return")?];
        if !self.at_kind(Kind::Newline) && !self.at_eof() {
            let v = self.expr()?;
            Self::push_node(&mut elems, v);
        }
        Ok(self.node(NodeKind::ReturnStmt, elems))
    }

    /// Gán ghép `x += 1`: lexer tách `+` `=` (SPEC-lex §1 không có token `+=`) nên parser nhận
    /// cặp **dính nhau** ngay sau một định danh (A.2 ghi `+=`; ghi ở JOURNAL là chỗ lệch SPEC).
    fn compound_assign_ahead(&self) -> bool {
        if self.tok().kind != Kind::Ident {
            return false;
        }
        let op = self.tok_at(1);
        let eq = self.tok_at(2);
        matches!(op.text.as_str(), "+" | "-" | "*" | "/" | "%")
            && op.kind == Kind::Punct
            && eq.is_punct("=")
            && op.span.end == eq.span.start
    }

    fn expr_stmt(&mut self) -> PResult<Node> {
        if self.compound_assign_ahead() {
            let mut elems = vec![self.bump()]; // lvalue đơn giản (A.2 `lvalue := IDENT postfix_op*`)
            elems.push(self.bump()); // `+`
            elems.push(self.bump()); // `=`
            let v = self.expr()?;
            Self::push_node(&mut elems, v);
            return Ok(self.node(NodeKind::AssignStmt, elems));
        }
        let e = self.expr()?;
        if self.at_punct("=") {
            let mut elems = vec![Elem::Node(Box::new(e))];
            elems.push(self.bump());
            let v = self.expr()?;
            Self::push_node(&mut elems, v);
            return Ok(self.node(NodeKind::AssignStmt, elems));
        }
        let elems = vec![Elem::Node(Box::new(e))];
        Ok(self.node(NodeKind::ExprStmt, elems))
    }

    fn break_stmt(&mut self) -> PResult<Node> {
        let elems = vec![self.bump()];
        Ok(self.node(NodeKind::BreakStmt, elems))
    }

    fn continue_stmt(&mut self) -> PResult<Node> {
        let elems = vec![self.bump()];
        Ok(self.node(NodeKind::ContinueStmt, elems))
    }

    fn for_stmt(&mut self) -> PResult<Node> {
        let mut elems = vec![self.expect_kw("for")?];
        let p = self.parse_pattern()?;
        Self::push_node(&mut elems, p);
        elems.push(self.expect_kw("in")?);
        let e = self.expr()?;
        Self::push_node(&mut elems, e);
        elems.push(self.expect_punct(":")?);
        let s = self.suite()?;
        Self::push_node(&mut elems, s);
        Ok(self.node(NodeKind::ForStmt, elems))
    }

    fn while_stmt(&mut self) -> PResult<Node> {
        let mut elems = vec![self.expect_kw("while")?];
        let e = self.expr()?;
        Self::push_node(&mut elems, e);
        elems.push(self.expect_punct(":")?);
        let s = self.suite()?;
        Self::push_node(&mut elems, s);
        Ok(self.node(NodeKind::WhileStmt, elems))
    }

    fn loop_stmt(&mut self) -> PResult<Node> {
        let mut elems = vec![self.expect_kw("loop")?];
        elems.push(self.expect_punct(":")?);
        let s = self.suite()?;
        Self::push_node(&mut elems, s);
        Ok(self.node(NodeKind::LoopStmt, elems))
    }

    fn scope_stmt(&mut self) -> PResult<Node> {
        let mut elems = vec![self.expect_kw("scope")?];
        if self.tok().kind == Kind::Ident && self.peek_punct(1, ":") {
            elems.push(self.bump());
        }
        elems.push(self.expect_punct(":")?);
        let s = self.suite()?;
        Self::push_node(&mut elems, s);
        Ok(self.node(NodeKind::ScopeStmt, elems))
    }

    fn if_stmt(&mut self) -> PResult<Node> {
        let mut elems = vec![self.expect_kw("if")?];
        let c = self.if_cond()?;
        Self::push_node(&mut elems, c);
        elems.push(self.expect_punct(":")?);
        let s = self.suite()?;
        Self::push_node(&mut elems, s);
        while self.at_kw("elif") {
            let mut arm = vec![self.bump()];
            let c = self.if_cond()?;
            Self::push_node(&mut arm, c);
            arm.push(self.expect_punct(":")?);
            let s = self.suite()?;
            Self::push_node(&mut arm, s);
            Self::push_node(&mut elems, self.node(NodeKind::ElifArm, arm));
        }
        if self.at_kw("else") {
            let mut arm = vec![self.bump()];
            arm.push(self.expect_punct(":")?);
            let s = self.suite()?;
            Self::push_node(&mut arm, s);
            Self::push_node(&mut elems, self.node(NodeKind::ElseArm, arm));
        }
        Ok(self.node(NodeKind::IfStmt, elems))
    }

    fn match_stmt(&mut self) -> PResult<Node> {
        let mut elems = vec![self.expect_kw("match")?];
        let e = self.expr()?;
        Self::push_node(&mut elems, e);
        elems.push(self.expect_punct(":")?);
        let body = self.match_body()?;
        Self::push_node(&mut elems, body);
        Ok(self.node(NodeKind::MatchStmt, elems))
    }

    /// `NEWLINE INDENT match_arm+ DEDENT`; arm := pattern ['if' expr] ':' suite.
    fn match_body(&mut self) -> PResult<Node> {
        let mut elems = vec![self.expect_newline()?, self.expect_indent()?];
        while !self.at_kind(Kind::Dedent) && !self.at_eof() {
            let mut arm = Vec::new();
            let p = self.parse_pattern()?;
            Self::push_node(&mut arm, p);
            if self.at_kw("if") {
                arm.push(self.bump());
                let g = self.expr()?;
                Self::push_node(&mut arm, g);
            }
            arm.push(self.expect_punct(":")?);
            let s = self.suite()?;
            Self::push_node(&mut arm, s);
            Self::push_node(&mut elems, self.node(NodeKind::MatchArm, arm));
            self.require_line_end(&mut elems)?;
        }
        if let Some(d) = self.take_dedent() {
            elems.push(d);
        }
        Ok(self.node(NodeKind::Suite, elems))
    }
}
