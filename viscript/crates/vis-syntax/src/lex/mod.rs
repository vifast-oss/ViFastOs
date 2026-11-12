// SPDX-License-Identifier: MIT OR Apache-2.0
//! Lexer ViScript — `docs/viscript/SPEC-lex.md` (APPROVED 2026-11-12), ST-04.
//!
//! Trình tự một lượt lex ([`lex`]):
//! 1. **Chuẩn hoá NFC toàn tệp** trước khi lex (SPEC-lex §0.1, VD-11).
//! 2. Theo chế độ ([`Mode`]): script `.vis` hay lệnh `.vish`/REPL (SPEC-lex §6).
//! 3. Trả [`LexOutput`] gồm văn bản đã NFC (mốc của mọi offset), token và chẩn đoán.
//!
//! Lỗi từ vựng **không** dừng lex (trừ chuỗi chưa đóng): token vẫn ra, chẩn đoán đi kèm, để
//! `vis check` báo nhiều lỗi một lượt.

pub mod lit;
pub mod token;
pub mod verify;

pub use token::{Kind, Span, Token};

use token::{is_keyword, longest_punct};

/// Chế độ cú pháp — SPEC-lex §6.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// `.vis` — script: `#` luôn là chú thích, `'a'` là `char`, có thụt lề.
    Script,
    /// `.vish`/REPL — lệnh: `'…'` thô, `#` chỉ ở đầu từ, `$x`/`^`, từ trần `WORD`.
    Command,
}

/// Một chẩn đoán từ vựng.
///
/// Mã **lấy từ sổ `vis-diag`** (không tự đặt chuỗi mã): `VS0101..VS0110` của SPEC-lex §7.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: &'static vis_diag::Code,
    pub message: String,
    pub span: Span,
    pub line: u32,
    pub col: u32,
}

impl Diagnostic {
    /// Mã dạng `VS01xx` (tiện cho log/so khớp).
    pub fn code_str(&self) -> &'static str {
        self.code.short
    }
}

/// Kết quả một lượt lex.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexOutput {
    /// Văn bản đã **chuẩn hoá CRLF→LF rồi NFC** — mọi `span` của token/chẩn đoán trỏ vào chuỗi này.
    pub text: String,
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Chuẩn hoá NFC toàn văn bản — bước bắt buộc trước khi lex (SPEC-lex §0.1, VD-11).
pub fn normalize_nfc(src: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    src.nfc().collect()
}

/// Chuẩn hoá đầu vào của lexer: **CRLF → LF** (SPEC-lex §1, hàng `NEWLINE`) rồi **NFC** toàn tệp
/// (§0.1). Mọi offset của token/chẩn đoán trỏ vào kết quả của hàm này ([`LexOutput::text`]).
///
/// Hệ quả: một `\r` **đứng một mình** (không có `\n` ngay sau) không phải newline — nó là ký tự
/// lạ ⇒ `VS0103`. Phép thay chạy **một lượt** như mọi chuẩn hoá CRLF: `"\r\r\n"` ⇒ `"\r\n"`
/// (một `\r` lạ + một NEWLINE), không nuốt thêm.
///
/// Ngoài ra `LexOutput::text` là mốc của **mọi** `span`, nên vị trí byte/dòng/cột đều tính trên
/// văn bản đã chuẩn hoá — không phải trên byte gốc của tệp.
pub fn normalize_source(src: &str) -> String {
    if src.contains("\r\n") {
        normalize_nfc(&src.replace("\r\n", "\n"))
    } else {
        normalize_nfc(src)
    }
}

/// Lex một văn bản nguồn theo chế độ (hàm thuần, SPEC-lex §0.2).
pub fn lex(src: &str, mode: Mode) -> LexOutput {
    let text = normalize_source(src);
    let (tokens, diagnostics) = {
        let mut lx = Lexer::new(&text, mode);
        lx.run();
        (lx.tokens, lx.diags)
    };
    LexOutput {
        text,
        tokens,
        diagnostics,
    }
}

/// Trạng thái chạy của lexer.
struct Lexer<'a> {
    src: &'a str,
    /// Offset byte hiện tại trong `src` (đã NFC).
    i: usize,
    line: u32,
    col: u32,
    mode: Mode,
    tokens: Vec<Token>,
    diags: Vec<Diagnostic>,
    /// Ngăn xếp độ sâu thụt lề (theo **cột**, tab = 4) — luôn có phần tử `0`.
    stack: Vec<usize>,
    /// Đơn vị thụt lề = mức thụt đầu tiên > 0 gặp trong tệp (SPEC-lex §4.1).
    unit: Option<usize>,
    /// Độ sâu ngoặc `() [] {}` — trong ngoặc không sinh NEWLINE/INDENT/DEDENT (§4.3).
    depth: usize,
    /// Dòng hiện tại đã có token nội dung chưa (dòng trống/chú thích thì không).
    line_has_code: bool,
    at_line_start: bool,
    /// Dòng này được nối với dòng trước bằng `|>` (§4.5): không tính thụt lề.
    joined: bool,
    /// Chế độ lệnh: đang ở đầu stage (`^` hợp lệ ở đây).
    stage_start: bool,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str, mode: Mode) -> Self {
        Self {
            src,
            i: 0,
            line: 1,
            col: 1,
            mode,
            tokens: Vec::new(),
            diags: Vec::new(),
            stack: vec![0],
            unit: None,
            depth: 0,
            line_has_code: false,
            at_line_start: true,
            joined: false,
            stage_start: true,
        }
    }

    // ---------------------------------------------------------------- vị trí

    fn rest(&self) -> &'a str {
        &self.src[self.i..]
    }

    fn byte(&self) -> Option<u8> {
        self.src.as_bytes().get(self.i).copied()
    }

    fn peek(&self, n: usize) -> Option<u8> {
        self.src.as_bytes().get(self.i + n).copied()
    }

    fn starts_with(&self, s: &str) -> bool {
        self.rest().starts_with(s)
    }

    fn cur_char(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn bump_ascii(&mut self) {
        self.i += 1;
        self.col += 1;
    }

    /// Tiến một **ký tự** bất kỳ, cập nhật dòng/cột — kể cả khi ký tự đó là `\n` (newline nằm
    /// trong nội suy hoặc trong chuỗi nhiều dòng: vẫn phải đếm dòng cho đúng, §4.4).
    fn bump_char(&mut self) {
        if let Some(c) = self.cur_char() {
            self.i += c.len_utf8();
            match c {
                '\n' => {
                    self.line += 1;
                    self.col = 1;
                }
                '\t' => self.col += 4,
                _ => self.col += 1,
            }
        }
    }

    /// Toạ độ (dòng, cột) tại một offset bất kỳ — dùng để quy vị trí lỗi trong literal.
    fn line_col_at(&self, offset: usize) -> (u32, u32) {
        let mut line = 1u32;
        let mut col = 1u32;
        for c in self.src[..offset].chars() {
            if c == '\n' {
                line += 1;
                col = 1;
            } else if c == '\t' {
                col += 4;
            } else {
                col += 1;
            }
        }
        (line, col)
    }

    // ---------------------------------------------------------------- phát

    fn push_at(&mut self, kind: Kind, start: usize, line: u32, col: u32) {
        let text = self.src[start..self.i].to_string();
        self.note_stage(kind, &text);
        if !matches!(
            kind,
            Kind::Newline
                | Kind::Indent
                | Kind::Dedent
                | Kind::Eof
                | Kind::Comment
                | Kind::DocComment
        ) {
            self.line_has_code = true;
            self.joined = false;
        }
        self.tokens.push(Token {
            kind,
            text,
            span: Span::new(start, self.i),
            line,
            col,
        });
    }

    fn push_zero(&mut self, kind: Kind) {
        self.tokens.push(Token {
            kind,
            text: String::new(),
            span: Span::new(self.i, self.i),
            line: self.line,
            col: self.col,
        });
    }

    /// Theo dõi đầu stage chế độ lệnh (SPEC-lex §6: `^` chỉ hợp lệ ở đầu stage).
    fn note_stage(&mut self, kind: Kind, text: &str) {
        if self.mode != Mode::Command {
            return;
        }
        self.stage_start = match kind {
            Kind::Newline => true,
            Kind::Punct => matches!(text, ";" | "|" | "|>" | "&"),
            _ => false,
        };
    }

    fn error(&mut self, id: u16, span: Span, line: u32, col: u32) {
        let code = vis_diag::lookup(id).expect("mã VS01xx phải có trong sổ vis-diag");
        self.diags.push(Diagnostic {
            code,
            message: code.message_vi.to_string(),
            span,
            line,
            col,
        });
    }

    /// Báo lỗi tại một offset byte (tự tính dòng/cột).
    fn error_at(&mut self, id: u16, offset: usize) {
        let (line, col) = self.line_col_at(offset);
        self.error(id, Span::new(offset, self.i.max(offset)), line, col);
    }

    // ---------------------------------------------------------------- vòng chính

    fn run(&mut self) {
        loop {
            if self.at_line_start {
                self.at_line_start = false;
                if self.mode == Mode::Script && self.depth == 0 {
                    self.begin_line();
                } else {
                    self.skip_inline_ws();
                }
            }
            self.skip_inline_ws();
            if self.i >= self.src.len() {
                break;
            }
            match self.byte() {
                Some(b'\n') => {
                    let start = self.i;
                    let (line, col) = (self.line, self.col);
                    self.finish_newline(start, line, col);
                }
                // CRLF đã thành LF ở `normalize_source`; `\r` còn lại là ký tự lạ (§1).
                Some(b'\r') => {
                    let start = self.i;
                    let (line, col) = (self.line, self.col);
                    self.bump_char();
                    self.error(103, Span::new(start, self.i), line, col);
                }
                _ => {
                    if self.mode == Mode::Script {
                        self.token_script();
                    } else {
                        self.token_cmd();
                    }
                }
            }
        }
        self.finish();
    }

    fn skip_inline_ws(&mut self) {
        loop {
            match self.byte() {
                Some(b' ') => {
                    self.i += 1;
                    self.col += 1;
                }
                Some(b'\t') => {
                    let start = self.i;
                    let (line, col) = (self.line, self.col);
                    self.i += 1;
                    self.col += 4;
                    self.error(107, Span::new(start, self.i), line, col);
                }
                _ => return,
            }
        }
    }

    /// Đầu một dòng vật lý ở độ sâu ngoặc 0 (script): đo thụt lề và áp INDENT/DEDENT.
    fn begin_line(&mut self) {
        let start = self.i;
        let (line, col) = (self.line, self.col);
        let indent = self.measure_indent();
        if self.i >= self.src.len() {
            return;
        }
        // Dòng trống hoặc chỉ có chú thích: không INDENT/DEDENT/NEWLINE (§4.2).
        if matches!(self.byte(), Some(b'\n') | Some(b'\r') | Some(b'#')) {
            return;
        }
        if self.joined {
            return;
        }
        self.apply_indent(indent, start, line, col);
    }

    fn measure_indent(&mut self) -> usize {
        let mut width = 0usize;
        loop {
            match self.byte() {
                Some(b' ') => {
                    self.i += 1;
                    self.col += 1;
                    width += 1;
                }
                Some(b'\t') => {
                    let start = self.i;
                    let (line, col) = (self.line, self.col);
                    self.i += 1;
                    self.col += 4;
                    width += 4;
                    self.error(107, Span::new(start, self.i), line, col);
                }
                _ => return width,
            }
        }
    }

    fn apply_indent(&mut self, indent: usize, start: usize, line: u32, col: u32) {
        let top = *self.stack.last().expect("ngăn xếp thụt lề luôn có 0");
        if indent > top {
            match self.unit {
                Some(u) if !indent.is_multiple_of(u) => {
                    self.error(109, Span::new(start, self.i), line, col);
                }
                None => self.unit = Some(indent),
                Some(_) => {}
            }
            self.stack.push(indent);
            let text = self.src[start..self.i].to_string();
            self.tokens.push(Token {
                kind: Kind::Indent,
                text,
                span: Span::new(start, self.i),
                line,
                col,
            });
            self.line_has_code = true;
        } else if indent < top {
            while self.stack.len() > 1 && indent < *self.stack.last().expect("có 0") {
                self.stack.pop();
                self.push_zero(Kind::Dedent);
            }
            if indent != *self.stack.last().expect("có 0") {
                self.error(109, Span::new(start, self.i), line, col);
            }
        }
    }

    /// `\n`: NEWLINE/`|>`/nuốt theo §4 (CRLF đã được chuẩn hoá thành LF trước khi lex).
    fn finish_newline(&mut self, start: usize, line: u32, col: u32) {
        let had_code = self.line_has_code;
        let cont = self.peek_join_next();
        self.i += 1; // '\n'
        self.line += 1;
        self.col = 1;
        self.line_has_code = false;
        self.at_line_start = true;
        self.stage_start = true;
        if self.depth > 0 || !had_code {
            return;
        }
        if cont {
            self.joined = true;
            return;
        }
        self.tokens.push(Token {
            kind: Kind::Newline,
            text: self.src[start..self.i].to_string(),
            span: Span::new(start, self.i),
            line,
            col,
        });
    }

    /// Dòng kế có bắt đầu bằng `|>` không (bỏ qua khoảng trắng/dòng trống) — §4.5.
    fn peek_join_next(&self) -> bool {
        let bytes = self.src.as_bytes();
        let mut j = self.i + 1;
        while j < bytes.len() && matches!(bytes[j], b' ' | b'\t' | b'\r' | b'\n') {
            j += 1;
        }
        self.src[j..].starts_with("|>")
    }

    fn finish(&mut self) {
        while self.stack.len() > 1 {
            self.stack.pop();
            self.push_zero(Kind::Dedent);
        }
        let start = self.i;
        let (line, col) = (self.line, self.col);
        self.push_at(Kind::Eof, start, line, col);
    }

    // ---------------------------------------------------------------- script

    fn token_script(&mut self) {
        let c = match self.byte() {
            Some(c) => c,
            None => return,
        };
        match c {
            b'0'..=b'9' => self.scan_number(),
            b'"' => {
                if self.starts_with("\"\"\"") {
                    self.scan_quoted(Kind::MultiStr);
                } else {
                    self.scan_quoted(Kind::Str);
                }
            }
            b'\'' => self.scan_char(),
            b'#' => self.scan_comment(),
            b'r' if self.starts_with("r\"\"\"") => self.scan_quoted(Kind::RawMulti),
            b'r' if self.starts_with("r\"") => self.scan_quoted(Kind::RawStr),
            b'b' if self.starts_with("b\"") => self.scan_quoted(Kind::Bytes),
            b'p' if self.starts_with("p\"") => self.scan_quoted(Kind::Path),
            _ if self.is_ident_start() => self.scan_ident(),
            _ => self.scan_punct(),
        }
    }

    fn is_ident_start(&self) -> bool {
        match self.cur_char() {
            Some(c) => c == '_' || unicode_ident::is_xid_start(c),
            None => false,
        }
    }

    fn scan_ident(&mut self) {
        let start = self.i;
        let (line, col) = (self.line, self.col);
        self.bump_char();
        while let Some(c) = self.cur_char() {
            if c == '_' || unicode_ident::is_xid_continue(c) {
                self.bump_char();
            } else {
                break;
            }
        }
        let text = &self.src[start..self.i];
        // `set{` dính liền là set literal (§5); có khoảng trắng thì `set` là định danh.
        if text == "set" && self.byte() == Some(b'{') {
            self.bump_ascii();
            self.depth += 1;
            self.push_at(Kind::SetLbrace, start, line, col);
            return;
        }
        let kind = if is_keyword(text) {
            Kind::Kw
        } else {
            Kind::Ident
        };
        self.push_at(kind, start, line, col);
    }

    fn scan_comment(&mut self) {
        let start = self.i;
        let (line, col) = (self.line, self.col);
        // `##` chỉ là tài liệu khi đứng đầu dòng (sau thụt lề) — tài liệu thiết kế §2.3.
        let doc = !self.line_has_code && self.starts_with("##");
        while let Some(c) = self.cur_char() {
            if c == '\n' || c == '\r' {
                break;
            }
            self.bump_char();
        }
        self.push_at(
            if doc { Kind::DocComment } else { Kind::Comment },
            start,
            line,
            col,
        );
    }

    fn scan_char(&mut self) {
        let start = self.i;
        let (line, col) = (self.line, self.col);
        self.bump_ascii(); // '
        let mut count = 0usize;
        while let Some(c) = self.cur_char() {
            if c == '\'' || c == '\n' || c == '\r' {
                break;
            }
            count += 1;
            self.bump_char();
        }
        let closed = self.byte() == Some(b'\'');
        if closed {
            self.bump_ascii();
        }
        if count != 1 || !closed {
            // §1: `char` là **đúng một** scalar; nhiều/ít hơn là VS0105.
            self.error(105, Span::new(start, self.i), line, col);
            return;
        }
        self.push_at(Kind::Char, start, line, col);
    }

    fn scan_punct(&mut self) {
        let start = self.i;
        let (line, col) = (self.line, self.col);
        if let Some(p) = longest_punct(self.rest()) {
            for _ in 0..p.len() {
                self.bump_ascii();
            }
            match p {
                "(" | "[" | "{" => self.depth += 1,
                ")" | "]" | "}" => self.depth = self.depth.saturating_sub(1),
                _ => {}
            }
            self.push_at(Kind::Punct, start, line, col);
            return;
        }
        self.bump_char();
        self.error(103, Span::new(start, self.i), line, col);
    }

    // ---------------------------------------------------------------- số

    fn scan_number(&mut self) {
        let start = self.i;
        let (line, col) = (self.line, self.col);

        // Cơ số 16/2 **chỉ khi** theo sau là chữ số hợp lệ (§1: `0b` đơn lẻ là SIZE(0,b)).
        let mut radix: u64 = 10;
        let mut prefix = 0usize;
        if self.starts_with("0x") && self.peek(2).is_some_and(|b| b.is_ascii_hexdigit()) {
            radix = 16;
            prefix = 2;
        } else if self.starts_with("0b") && matches!(self.peek(2), Some(b'0' | b'1')) {
            radix = 2;
            prefix = 2;
        }
        for _ in 0..prefix {
            self.bump_ascii();
        }
        while let Some(b) = self.byte() {
            let ok = b == b'_' || (b as char).is_digit(radix as u32);
            if !ok {
                break;
            }
            self.bump_ascii();
        }

        // Phần thập phân / số mũ → FLOAT.
        let mut is_float = false;
        if radix == 10 {
            if self.byte() == Some(b'.') {
                match self.peek(1) {
                    Some(b'.') => {} // `1..2` là INT RANGE INT (§2)
                    Some(d) if d.is_ascii_digit() => {
                        self.bump_ascii(); // '.'
                                           // Phần lẻ là `[0-9]+` (§1); `_` chỉ được **giữa hai chữ số** (§2), nên
                                           // `1.2_` / `1.2__3` là literal sai (VS0104) — không nuốt `_` vào token.
                        loop {
                            match self.byte() {
                                Some(b) if b.is_ascii_digit() => self.bump_ascii(),
                                Some(b'_') if self.peek(1).is_some_and(|n| n.is_ascii_digit()) => {
                                    self.bump_ascii();
                                    self.bump_ascii();
                                }
                                Some(b'_') => {
                                    let off = self.i;
                                    self.error_at(104, off);
                                    break;
                                }
                                _ => break,
                            }
                        }
                        is_float = true;
                    }
                    _ => {
                        // §1: `1.` sai ⇒ VS0104 (dấu `.` để token sau xử lý).
                        let off = self.i;
                        self.error_at(104, off);
                    }
                }
            }
            if !is_float && self.scan_exponent() {
                is_float = true;
            } else if is_float {
                self.scan_exponent();
            }
        }

        let text = &self.src[start..self.i];
        // `pct` áp cho cả `INT` và `FLOAT` (§1: `(INT | FLOAT)` + `pct`).
        if self.starts_with("pct") {
            for _ in 0..3 {
                self.bump_ascii();
            }
            self.push_at(Kind::Percent, start, line, col);
            return;
        }
        if is_float {
            self.push_at(Kind::Float, start, line, col);
            return;
        }
        if let Err(e) = lit::decode_int(text) {
            self.error_at(e.code, start + e.at);
            return;
        }

        // Hậu tố đơn vị đo → SIZE; đơn vị thời gian → DURATION (§1, chỉ cho `INT`).
        if let Some(n) = scan_size_suffix(self.rest()) {
            for _ in 0..n {
                self.bump_ascii();
            }
            self.push_at(Kind::Size, start, line, col);
            return;
        }
        if let Some(n) = scan_duration_suffix(self.rest()) {
            for _ in 0..n {
                self.bump_ascii();
            }
            self.push_at(Kind::Duration, start, line, col);
            return;
        }
        self.push_at(Kind::Int, start, line, col);
    }

    /// `e`/`E` [+-]? chữ số → số mũ. Trả `true` nếu đã ăn.
    fn scan_exponent(&mut self) -> bool {
        let mut j = 0usize;
        match self.peek(j) {
            Some(b'e' | b'E') => j += 1,
            _ => return false,
        }
        if matches!(self.peek(j), Some(b'+' | b'-')) {
            j += 1;
        }
        if !self.peek(j).is_some_and(|b| b.is_ascii_digit()) {
            return false;
        }
        j += 1;
        while self.peek(j).is_some_and(|b| b.is_ascii_digit()) {
            j += 1;
        }
        for _ in 0..j {
            self.bump_ascii();
        }
        true
    }

    // ---------------------------------------------------------------- chuỗi

    fn scan_quoted(&mut self, kind: Kind) {
        let start = self.i;
        let (line, col) = (self.line, self.col);
        let (open, close, escapes, interp) = match kind {
            Kind::Str => ("\"", "\"", true, true),
            Kind::MultiStr => ("\"\"\"", "\"\"\"", true, true),
            Kind::Path => ("p\"", "\"", true, true),
            Kind::RawStr => ("r\"", "\"", false, false),
            Kind::RawMulti => ("r\"\"\"", "\"\"\"", false, false),
            Kind::Bytes => ("b\"", "\"", true, false),
            _ => return,
        };
        let multiline = matches!(kind, Kind::MultiStr | Kind::RawMulti);
        for _ in 0..open.len() {
            self.bump_ascii();
        }
        loop {
            if self.i >= self.src.len() {
                self.error(101, Span::new(start, self.src.len()), line, col);
                return;
            }
            if self.starts_with(close) {
                for _ in 0..close.len() {
                    self.bump_ascii();
                }
                break;
            }
            let b = self.byte().unwrap_or(0);
            if b == b'\n' && !multiline {
                // Chuỗi một dòng không được chứa newline thô: coi là chưa đóng (§1).
                self.error(101, Span::new(start, self.i), line, col);
                return;
            }
            if escapes && b == b'\\' {
                self.bump_ascii();
                if self.i < self.src.len() {
                    self.bump_char();
                }
                continue;
            }
            if interp && b == b'{' {
                if self.peek(1) == Some(b'{') {
                    self.bump_ascii();
                    self.bump_ascii();
                    continue;
                }
                if !self.skip_interpolation() {
                    self.error(108, Span::new(start, self.i), line, col);
                    return;
                }
                continue;
            }
            self.bump_char();
        }

        let text = &self.src[start..self.i];
        let decoded = if kind == Kind::Bytes {
            lit::decode_bytes(text).err().map(|e| e.code)
        } else {
            lit::decode_str(kind, text).err().map(|e| e.code)
        };
        if let Some(code) = decoded {
            self.error(code, Span::new(start, self.i), line, col);
            return;
        }
        self.push_at(kind, start, line, col);
    }

    /// Quét `{expr}` của nội suy tới `}` khớp; `false` nếu hết tệp trước khi đóng (§3).
    fn skip_interpolation(&mut self) -> bool {
        let mut depth = 0usize;
        while self.i < self.src.len() {
            match self.byte() {
                Some(b'{') => {
                    depth += 1;
                    self.bump_ascii();
                }
                Some(b'}') => {
                    depth -= 1;
                    self.bump_ascii();
                    if depth == 0 {
                        return true;
                    }
                }
                Some(b'\\') => {
                    self.bump_ascii();
                    if self.i < self.src.len() {
                        self.bump_char();
                    }
                }
                Some(_) => self.bump_char(),
                None => break,
            }
        }
        false
    }

    // ---------------------------------------------------------------- chế độ lệnh

    fn token_cmd(&mut self) {
        let c = match self.byte() {
            Some(c) => c,
            None => return,
        };
        match c {
            b'$' => self.scan_dollar(),
            b'^' => self.scan_caret(),
            b'\'' => self.scan_cmd_raw_string(),
            b'"' => {
                if self.starts_with("\"\"\"") {
                    self.scan_quoted(Kind::MultiStr);
                } else {
                    self.scan_quoted(Kind::Str);
                }
            }
            b'#' => self.scan_comment(),
            b'(' | b')' | b'[' | b']' | b'{' | b'}' | b';' | b'&' | b'>' | b'<' | b'|' => {
                self.scan_punct();
            }
            _ => self.scan_word(),
        }
    }

    /// Metachar chế độ lệnh (SPEC-lex §6) — `#` chỉ ở đầu từ nên không nằm đây.
    fn is_cmd_metachar(c: char) -> bool {
        matches!(
            c,
            ' ' | '\t'
                | '('
                | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | ';'
                | '&'
                | '$'
                | '\''
                | '"'
                | '|'
                | '>'
                | '<'
                | '^'
                | '\n'
                | '\r'
        )
    }

    fn scan_word(&mut self) {
        let start = self.i;
        let (line, col) = (self.line, self.col);
        while let Some(c) = self.cur_char() {
            if Self::is_cmd_metachar(c) {
                break;
            }
            self.bump_char();
        }
        if self.i == start {
            self.bump_char();
            self.error(103, Span::new(start, self.i), line, col);
            return;
        }
        self.push_at(Kind::Word, start, line, col);
    }

    fn scan_dollar(&mut self) {
        let start = self.i;
        let (line, col) = (self.line, self.col);
        self.bump_ascii();
        self.push_at(Kind::Dollar, start, line, col);
        // `$env.NAME` (§6): biến, rồi chuỗi `.TÊN` — mỗi đoạn một token như §8 #50.
        loop {
            if !self.is_ident_start() {
                return;
            }
            let seg = self.i;
            let (l, c) = (self.line, self.col);
            self.bump_char();
            while let Some(ch) = self.cur_char() {
                if ch == '_' || unicode_ident::is_xid_continue(ch) {
                    self.bump_char();
                } else {
                    break;
                }
            }
            self.push_at(Kind::Ident, seg, l, c);
            if self.byte() == Some(b'.')
                && self
                    .peek(1)
                    .is_some_and(|b| b == b'_' || (b as char).is_ascii_alphabetic())
            {
                let dot = self.i;
                let (dl, dc) = (self.line, self.col);
                self.bump_ascii();
                self.push_at(Kind::Punct, dot, dl, dc);
                continue;
            }
            return;
        }
    }

    fn scan_caret(&mut self) {
        let start = self.i;
        let (line, col) = (self.line, self.col);
        self.bump_ascii();
        if self.stage_start {
            self.push_at(Kind::Caret, start, line, col);
        } else {
            // §7 VS0110: `^` ngoài đầu stage.
            self.error(110, Span::new(start, self.i), line, col);
            self.push_at(Kind::Punct, start, line, col);
        }
    }

    /// `'…'` chế độ lệnh = RAW_STRING thô, không escape, không nội suy (§6).
    fn scan_cmd_raw_string(&mut self) {
        let start = self.i;
        let (line, col) = (self.line, self.col);
        self.bump_ascii();
        loop {
            if self.i >= self.src.len() {
                self.error(101, Span::new(start, self.src.len()), line, col);
                return;
            }
            match self.byte() {
                Some(b'\'') => {
                    self.bump_ascii();
                    break;
                }
                Some(b'\n' | b'\r') => {
                    self.error(101, Span::new(start, self.i), line, col);
                    return;
                }
                _ => self.bump_char(),
            }
        }
        self.push_at(Kind::RawStr, start, line, col);
    }
}

/// Độ dài hậu tố đơn vị kích thước khớp đầu `s` (`kb`, `kib`, `b`…), dài nhất trước.
fn scan_size_suffix(s: &str) -> Option<usize> {
    lit::SIZE_UNITS
        .iter()
        .find(|u| s.starts_with(*u))
        .map(|u| u.len())
}

/// Độ dài chuỗi đơn vị thời gian ghép **giảm dần** (`2h30m` → 5). Đơn vị đứng sau phải nhỏ
/// hơn đơn vị trước, nếu không chuỗi dừng ở đoạn đã đọc (SPEC-lex §1, §2).
fn scan_duration_suffix(s: &str) -> Option<usize> {
    let mut i = 0usize;
    let mut last_rank: Option<u8> = None;
    loop {
        let unit = lit::DURATION_UNITS
            .iter()
            .find(|(u, _)| s[i..].starts_with(u))?;
        if let Some(prev) = last_rank {
            if unit.1 >= prev {
                return Some(i);
            }
        }
        i += unit.0.len();
        last_rank = Some(unit.1);
        let start_digits = i;
        let digits = s[i..]
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(s[i..].len());
        if digits == 0 {
            return Some(i);
        }
        i += digits;
        // Chữ số chỉ thuộc token nếu theo sau là đơn vị **nhỏ hơn**; ngược lại nhả chữ số ra
        // cho token INT kế tiếp (`30m2h` ⇒ DURATION(30m) INT(2)…, không phải DURATION(30m2)).
        let next_ok = lit::DURATION_UNITS
            .iter()
            .any(|(u, r)| s[i..].starts_with(u) && Some(*r) < last_rank);
        if !next_ok {
            return Some(start_digits);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<Kind> {
        lex(src, Mode::Script)
            .tokens
            .into_iter()
            .filter(|t| t.kind != Kind::Eof)
            .map(|t| t.kind)
            .collect()
    }

    fn codes(src: &str) -> Vec<&'static str> {
        lex(src, Mode::Script)
            .diagnostics
            .iter()
            .map(|d| d.code_str())
            .collect()
    }

    fn codes_cmd(src: &str) -> Vec<&'static str> {
        lex(src, Mode::Command)
            .diagnostics
            .iter()
            .map(|d| d.code_str())
            .collect()
    }

    #[test]
    fn nfc_duoc_ap_truoc_khi_lex() {
        let nfd = "'o\u{302}\u{301}'";
        let out = lex(nfd, Mode::Script);
        let t = &out.tokens[0];
        assert_eq!(t.kind, Kind::Char);
        assert_eq!(t.text, "'ố'", "token phải là dạng NFC");
        assert!(out.diagnostics.is_empty());
    }

    #[test]
    fn span_tro_dung_van_ban_da_nfc() {
        let out = lex("let số = 1_0", Mode::Script);
        for t in &out.tokens {
            assert_eq!(&out.text[t.span.start..t.span.end], t.text.as_str());
        }
    }

    #[test]
    fn dedent_sinh_o_eof() {
        let k = kinds("if x:\n    y\n");
        assert_eq!(
            k,
            vec![
                Kind::Kw,
                Kind::Ident,
                Kind::Punct,
                Kind::Newline,
                Kind::Indent,
                Kind::Ident,
                Kind::Newline,
                Kind::Dedent
            ]
        );
    }

    #[test]
    fn dong_trong_va_chu_thich_khong_sinh_token_ket_thuc_dong() {
        let k = kinds("a\n\n# c\nb\n");
        assert_eq!(
            k,
            vec![
                Kind::Ident,
                Kind::Newline,
                Kind::Comment,
                Kind::Ident,
                Kind::Newline
            ]
        );
    }

    #[test]
    fn noi_dong_bang_pipe_arrow() {
        let k = kinds("x\n|> f\n");
        assert_eq!(
            k,
            vec![Kind::Ident, Kind::Punct, Kind::Ident, Kind::Newline]
        );
        assert!(lex("x\n|> f\n", Mode::Script).diagnostics.is_empty());
    }

    #[test]
    fn ngoac_nuot_newline_va_indent() {
        let out = lex("(1 +\n   2)", Mode::Script);
        assert!(out.diagnostics.is_empty());
        assert_eq!(
            out.tokens.iter().map(|t| t.kind).collect::<Vec<_>>(),
            vec![
                Kind::Punct,
                Kind::Int,
                Kind::Punct,
                Kind::Int,
                Kind::Punct,
                Kind::Eof
            ]
        );
    }

    #[test]
    fn tab_sinh_vs0107_va_van_lex_tiep() {
        let out = lex("\tif", Mode::Script);
        assert_eq!(codes("\tif"), vec!["VS0107"]);
        assert_eq!(
            out.tokens.iter().map(|t| t.kind).collect::<Vec<_>>(),
            vec![Kind::Indent, Kind::Kw, Kind::Dedent, Kind::Eof]
        );
        assert_eq!(out.tokens[0].col, 1);
    }

    #[test]
    fn thut_le_sai_don_vi_la_vs0109() {
        assert_eq!(codes("if x:\n    y\n  z"), vec!["VS0109"]);
    }

    #[test]
    fn che_do_lenh_tach_tu_tran() {
        let k: Vec<Kind> = lex("ls -la ~/x", Mode::Command)
            .tokens
            .iter()
            .map(|t| t.kind)
            .collect();
        assert_eq!(k, vec![Kind::Word, Kind::Word, Kind::Word, Kind::Eof]);
    }

    #[test]
    fn the_khong_dau_stage_la_vs0110() {
        assert_eq!(codes_cmd("a^b"), vec!["VS0110"]);
        assert!(codes_cmd("^a").is_empty());
        assert!(codes_cmd("a | ^b").is_empty());
    }

    #[test]
    fn duration_va_size_khong_nham_nhau() {
        let out = lex("5kb 5mib 2h30m 250ms 30m2h", Mode::Script);
        let got: Vec<(&str, String)> = out
            .tokens
            .iter()
            .filter(|t| t.kind != Kind::Eof)
            .map(|t| (t.kind.name(), t.text.clone()))
            .collect();
        assert_eq!(
            got,
            vec![
                ("SIZE", "5kb".into()),
                ("SIZE", "5mib".into()),
                ("DURATION", "2h30m".into()),
                ("DURATION", "250ms".into()),
                ("DURATION", "30m".into()),
                ("DURATION", "2h".into()),
            ]
        );
    }
    #[test]
    fn crlf_duoc_chuan_hoa_thanh_lf_truoc_khi_lex() {
        let src = "a\r\nb\r\n";
        let out = lex(src, Mode::Script);
        assert_eq!(out.text, "a\nb\n", "CRLF phải thành LF trong text");
        assert!(out.diagnostics.is_empty());
        let kinds: Vec<Kind> = out.tokens.iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            vec![
                Kind::Ident,
                Kind::Newline,
                Kind::Ident,
                Kind::Newline,
                Kind::Eof
            ]
        );
        assert_eq!(out.tokens[1].text, "\n");
        // Toạ độ dòng 2 tính trên văn bản đã chuẩn hoá.
        assert_eq!((out.tokens[2].line, out.tokens[2].col), (2, 1));
    }

    #[test]
    fn cr_don_doc_la_ky_tu_la() {
        let out = lex("a\rb", Mode::Script);
        assert_eq!(
            out.diagnostics
                .iter()
                .map(|d| d.code_str())
                .collect::<Vec<_>>(),
            vec!["VS0103"]
        );
    }

    #[test]
    fn gach_duoi_trong_phan_le_cua_float_phai_giua_hai_chu_so() {
        assert_eq!(codes("1.2_"), vec!["VS0104"]);
        assert_eq!(codes("1.2__3"), vec!["VS0104"]);
        let ok = lex("1.2_3", Mode::Script);
        assert!(ok.diagnostics.is_empty());
        assert_eq!(ok.tokens[0].kind, Kind::Float);
        assert_eq!(ok.tokens[0].text, "1.2_3");
    }

    #[test]
    fn bytes_chi_nhan_escape_xnn() {
        assert_eq!(codes("b\"a\\nb\""), vec!["VS0102"]);
        assert!(codes("b\"\\x41\"").is_empty());
    }
    #[test]
    fn cr_dung_truoc_crlf_van_la_ky_tu_la() {
        // `replace("\r\n", "\n")` chạy một lượt: "\r\r\n" ⇒ "\r\n" (một `\r` lạ + LF).
        let out = lex("a\r\r\nb", Mode::Script);
        assert_eq!(out.text, "a\r\nb");
        assert_eq!(
            out.diagnostics
                .iter()
                .map(|d| d.code_str())
                .collect::<Vec<_>>(),
            vec!["VS0103"]
        );
        assert_eq!(
            out.tokens.iter().map(|t| t.kind).collect::<Vec<_>>(),
            vec![Kind::Ident, Kind::Newline, Kind::Ident, Kind::Eof]
        );
    }
}
