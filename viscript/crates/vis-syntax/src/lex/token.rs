// SPDX-License-Identifier: MIT OR Apache-2.0
//! Kiểu token + bảng từ khoá + bảng dấu câu (`docs/viscript/SPEC-lex.md` §1).

/// Vị trí byte `[start, end)` trong **văn bản đã NFC** (SPEC-lex §0.4: mọi offset tính bằng byte).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub const fn len(&self) -> usize {
        self.end - self.start
    }

    pub const fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

/// Loại token — đúng bảng §1 của SPEC-lex.
///
/// Ghi chú đối chiếu (SPEC-lex §1 liệt kê một số mục trong `PUNCT`, §5–§8 đặt tên riêng):
/// - `..`/`..=` là `PUNCT` theo §1; golden §8 gọi là `RANGE`/`RANGE_EQ` — đó là **bí danh của
///   cùng token** (`Kind::Punct` với `text` `".."`/`"..="`), không phải kind riêng.
/// - `$`, `^`, `set{` là `DOLLAR`/`CARET`/`SET_LBRACE` vì §5–§6 và §8 đòi kind riêng (ngữ cảnh
///   chế độ lệnh). `#` không bao giờ thành `PUNCT`: chú thích thắng (SPEC-lex §5, hàng `#`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// `UAX #31 + _`, chữ số không đứng đầu (SPEC-lex §1).
    Ident,
    /// Từ khoá tiếng Anh (tài liệu thiết kế §2.1). `test` **không** thuộc đây (từ khoá theo
    /// ngữ cảnh `test fn …`): lexer trả `Ident`, parser điện phân.
    Kw,
    Int,
    Float,
    Char,
    Str,
    RawStr,
    MultiStr,
    RawMulti,
    Bytes,
    Path,
    Size,
    Duration,
    Percent,
    /// Dấu câu; `text` là chính chữ viết (`"??"`, `"|>"`, `"%"`…).
    Punct,
    Newline,
    Indent,
    Dedent,
    Eof,
    /// `#` tới cuối dòng (script: luôn; lệnh: chỉ khi đứng đầu từ).
    Comment,
    /// `##` ở đầu dòng (sau thụt lề) — tài liệu gắn khai báo kế tiếp (tài liệu thiết kế §2.3).
    DocComment,
    /// Từ trần chế độ lệnh (SPEC-lex §6).
    Word,
    /// `$` — biến phiên / `$env.NAME`.
    Dollar,
    /// `^` ở đầu stage chế độ lệnh (ép gọi ngoài).
    Caret,
    /// `set{` dính liền — set literal (SPEC-lex §5).
    SetLbrace,
}

impl Kind {
    /// Toàn bộ kind của bảng §1 (dùng cho test phủ và log fuzz).
    pub const ALL: [Kind; 25] = [
        Kind::Ident,
        Kind::Kw,
        Kind::Int,
        Kind::Float,
        Kind::Char,
        Kind::Str,
        Kind::RawStr,
        Kind::MultiStr,
        Kind::RawMulti,
        Kind::Bytes,
        Kind::Path,
        Kind::Size,
        Kind::Duration,
        Kind::Percent,
        Kind::Punct,
        Kind::Newline,
        Kind::Indent,
        Kind::Dedent,
        Kind::Eof,
        Kind::Comment,
        Kind::DocComment,
        Kind::Word,
        Kind::Dollar,
        Kind::Caret,
        Kind::SetLbrace,
    ];
}

impl Kind {
    /// Tên kind đúng như SPEC-lex §8 viết (dùng cho log/so sánh và `Debug`).
    pub const fn name(self) -> &'static str {
        match self {
            Kind::Ident => "IDENT",
            Kind::Kw => "KW",
            Kind::Int => "INT",
            Kind::Float => "FLOAT",
            Kind::Char => "CHAR",
            Kind::Str => "STRING",
            Kind::RawStr => "RAW_STRING",
            Kind::MultiStr => "MULTI_STRING",
            Kind::RawMulti => "RAW_MULTI",
            Kind::Bytes => "BYTES",
            Kind::Path => "PATH",
            Kind::Size => "SIZE",
            Kind::Duration => "DURATION",
            Kind::Percent => "PERCENT",
            Kind::Punct => "PUNCT",
            Kind::Newline => "NEWLINE",
            Kind::Indent => "INDENT",
            Kind::Dedent => "DEDENT",
            Kind::Eof => "EOF",
            Kind::Comment => "COMMENT",
            Kind::DocComment => "DOC_COMMENT",
            Kind::Word => "WORD",
            Kind::Dollar => "DOLLAR",
            Kind::Caret => "CARET",
            Kind::SetLbrace => "SET_LBRACE",
        }
    }
}

/// Một token (SPEC-lex §0.4: `{kind, text, span, line, col}`; `col` theo **scalar**, 1-based).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: Kind,
    /// Nguyên văn nguồn của token (đã NFC). Với `DEDENT`/`EOF` là chuỗi rỗng.
    pub text: String,
    pub span: Span,
    /// Dòng vật lý, 1-based, tính trên văn bản đã NFC.
    pub line: u32,
    /// Cột theo Unicode scalar, 1-based; tab tiến 4 cột (SPEC-lex §0.5, §4.7).
    pub col: u32,
}

impl Token {
    pub fn new(kind: Kind, text: impl Into<String>, span: Span, line: u32, col: u32) -> Self {
        Self {
            kind,
            text: text.into(),
            span,
            line,
            col,
        }
    }

    /// `true` nếu token là dấu câu có đúng chữ viết `s`.
    pub fn is_punct(&self, s: &str) -> bool {
        self.kind == Kind::Punct && self.text == s
    }
}

/// Từ khoá tiếng Anh — tài liệu thiết kế §2.1, đã **bỏ `as`** và **bỏ `test`** (theo ngữ cảnh).
///
/// 40 mục, xếp ABC để tra cứu bằng tìm nhị phân ở [`is_keyword`].
pub const KEYWORDS: [&str; 40] = [
    "alias", "and", "async", "await", "break", "cmd", "const", "continue", "derives", "dyn",
    "elif", "else", "enum", "extern", "false", "fn", "for", "has", "if", "impl", "in", "let",
    "loop", "match", "newtype", "none", "not", "or", "pub", "ref", "return", "scope", "self",
    "trait", "true", "type", "use", "var", "while", "with",
];

/// `true` nếu `s` là từ khoá tiếng Anh của ngôn ngữ.
pub fn is_keyword(s: &str) -> bool {
    KEYWORDS.binary_search(&s).is_ok()
}

/// Bảng dấu câu của §1. `#`/`##` không có mặt: chú thích thắng (xem [`Kind`]); `..`/`..=` có
/// mặt và được golden §8 gọi là `RANGE`/`RANGE_EQ`. Maximal munch chọn **chuỗi dài nhất** khớp
/// (xem [`longest_punct`]), nên thứ tự mảng không quan trọng.
pub const PUNCTS: [&str; 38] = [
    "..=", "!=", "**", "->", "..", "<=", "==", "=>", ">=", "?.", "??", "|>", "!", "&", "(", ")",
    "*", "+", ",", "-", ".", "/", ":", ";", "<", "=", ">", "?", "@", "[", "\\", "]", "^", "{", "|",
    "}", "~", "%",
];

/// Chữ viết dấu câu dài nhất khớp đầu `s` (maximal munch, SPEC-lex §0.3).
pub fn longest_punct(s: &str) -> Option<&'static str> {
    let mut best: Option<&'static str> = None;
    for p in PUNCTS {
        if s.starts_with(p) && best.is_none_or(|b: &str| p.len() > b.len()) {
            best = Some(p);
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ten_kind_khong_trung_va_du_bang_spec() {
        let mut names: Vec<&str> = Kind::ALL.iter().map(|k| k.name()).collect();
        let n = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), n, "tên kind phải duy nhất");
        assert_eq!(n, 25);
        // Bảng từ khoá xếp ABC để tra nhị phân; bảng dấu câu không trùng nhau.
        let mut kw: Vec<&str> = KEYWORDS.to_vec();
        kw.sort_unstable();
        kw.dedup();
        assert_eq!(kw.len(), KEYWORDS.len());
        assert!(
            KEYWORDS.windows(2).all(|w| w[0] < w[1]),
            "KEYWORDS phải xếp ABC"
        );
        assert!(
            !is_keyword("test"),
            "`test` là từ khoá theo ngữ cảnh, không vào bảng"
        );
        assert!(is_keyword("if"));
        let mut p: Vec<&str> = PUNCTS.to_vec();
        p.sort_unstable();
        p.dedup();
        assert_eq!(p.len(), PUNCTS.len(), "bảng dấu câu không được trùng");
    }
}
