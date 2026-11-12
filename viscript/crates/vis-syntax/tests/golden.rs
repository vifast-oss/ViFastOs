// SPDX-License-Identifier: MIT OR Apache-2.0
//! Golden token — `docs/viscript/SPEC-lex.md` §8, 60 ca (48 script + 12 lệnh).
//!
//! Đây là test **đỏ trước** của ST-04 (D2-05): viết xong, chạy khi lexer còn là khung rỗng,
//! phải đỏ 60/60 (log ở `viscript/agent/JOURNAL/st04-red-golden.log`), rồi mới hiện thực.
//!
//! # Kỳ vọng lấy từ đâu
//!
//! Nguyên văn `SPEC-lex.md` §8. Hai chỗ ví dụ của §8 **lược** token cho gọn (#56, #58), tệp
//! `.tok` ghi danh sách **đầy đủ** và có dòng `//` chép lại nguyên văn ví dụ gốc — hướng là
//! **siết**, không phải nới (xem `agent/BLOCKED.md` §3 Q1). Các ca còn lại giữ nguyên văn, kể cả
//! `"{x" => !VS0108` (#26).
//!
//! # Cách so
//!
//! - Bỏ `EOF`; bỏ các `DEDENT` do EOF sinh (§4.6) — trừ khi dòng kỳ vọng có `DEDENT`.
//! - Dòng chỉ có `!VSxxxx` ⇒ **chỉ** kiểm chẩn đoán, không đối chiếu token.
//! - `RANGE`/`RANGE_EQ` là bí danh của `PUNCT("..")`/`PUNCT("..=")`.
//! - Một từ **viết thường** không thuộc bảng kind (ví dụ `env` của §8 #50) được so là
//!   `IDENT(text)`; mọi mục còn lại (tên viết HOA lạ, ký hiệu lạ, …) là **lỗi fixture** ⇒
//!   `item_from` panic, không có nhánh "khớp text bất kể kind".

use std::path::PathBuf;

use vis_syntax::lex::lit;
use vis_syntax::{lex, Kind, Mode, Token};

/// Số ca tối thiểu theo QD §7.5 — không được giảm.
const EXPECTED_CASES: usize = 60;
const SCRIPT_CASES: usize = 48;
const CMD_CASES: usize = 12;

#[derive(Debug, Clone, PartialEq)]
enum Item {
    /// Chỉ so kind.
    Kind(Kind),
    /// So kind + payload chuẩn hoá.
    KindPayload(Kind, String),
    /// Dấu câu viết trực tiếp (`??`, `|>`, `(`, …).
    Punct(String),
}

#[derive(Debug)]
struct Case {
    file: &'static str,
    line: usize,
    /// Số ca trong SPEC-lex §8 (1..=60).
    spec: u32,
    input: String,
    items: Vec<Item>,
    codes: Vec<String>,
}

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Giải escape của tệp `.tok` (chỉ `\\ \n \t \r`; khác là lỗi tệp fixture).
fn unescape(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut it = src.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('\\') => out.push('\\'),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            other => panic!("escape lạ trong tệp .tok: \\{other:?}"),
        }
    }
    out
}

fn kind_by_name(name: &str) -> Option<Kind> {
    Some(match name {
        "IDENT" => Kind::Ident,
        "KW" => Kind::Kw,
        "INT" => Kind::Int,
        "FLOAT" => Kind::Float,
        "CHAR" => Kind::Char,
        "STRING" => Kind::Str,
        "RAW_STRING" => Kind::RawStr,
        "MULTI_STRING" => Kind::MultiStr,
        "RAW_MULTI" => Kind::RawMulti,
        "BYTES" => Kind::Bytes,
        "PATH" => Kind::Path,
        "SIZE" => Kind::Size,
        "DURATION" => Kind::Duration,
        "PERCENT" => Kind::Percent,
        "PUNCT" => Kind::Punct,
        "NEWLINE" => Kind::Newline,
        "INDENT" => Kind::Indent,
        "DEDENT" => Kind::Dedent,
        "EOF" => Kind::Eof,
        "COMMENT" => Kind::Comment,
        "DOC_COMMENT" => Kind::DocComment,
        "WORD" => Kind::Word,
        "DOLLAR" => Kind::Dollar,
        "CARET" => Kind::Caret,
        "SET_LBRACE" => Kind::SetLbrace,
        _ => return None,
    })
}

/// Tách danh sách kỳ vọng thành từng mục, tôn trọng ngoặc (`STRING(a b)` là **một** mục).
///
/// Ngoặc **đứng một mình** ở độ sâu 0 là hai token dấu câu của §8 (`f()?`, `(1 + 2)`), không
/// phải mở đầu payload — nên `(`/`)` rời được đẩy ra thành mục riêng.
fn split_items(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0i32;
    for c in s.chars() {
        match c {
            '(' if depth == 0 && cur.is_empty() => out.push("(".to_string()),
            ')' if depth == 0 => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
                out.push(")".to_string());
            }
            '(' => {
                depth += 1;
                cur.push(c);
            }
            ')' => {
                depth -= 1;
                cur.push(c);
            }
            ' ' if depth == 0 => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn item_from(raw: &str) -> Item {
    // Bí danh của §8 cho hai dấu câu của §1.
    match raw {
        "RANGE" => return Item::Punct("..".to_string()),
        "RANGE_EQ" => return Item::Punct("..=".to_string()),
        _ => {}
    }
    if let Some(k) = kind_by_name(raw) {
        return Item::Kind(k);
    }
    if vis_syntax::PUNCTS.contains(&raw) {
        return Item::Punct(raw.to_string());
    }
    if let Some(open) = raw.find('(') {
        assert!(raw.ends_with(')'), "mục kỳ vọng sai định dạng: {raw}");
        let name = &raw[..open];
        let payload = &raw[open + 1..raw.len() - 1];
        let kind = kind_by_name(name).unwrap_or_else(|| panic!("payload cho tên chưa biết: {raw}"));
        return Item::KindPayload(kind, payload.to_string());
    }
    // Từ viết thường không thuộc bảng kind (ví dụ `env` của §8 #50) được so là `IDENT(text)`
    // — siết hơn "khớp text bất kể kind": `DOLLAR word` thì trượt. Mục còn lại là **lỗi fixture**.
    if raw.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && raw.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Item::KindPayload(Kind::Ident, raw.to_string());
    }
    panic!("mục kỳ vọng không nhận dạng được (lỗi fixture .tok): {raw}");
}

fn parse_file(file: &'static str, path: &PathBuf, mode: Mode, cases: &mut Vec<(Mode, Case)>) {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("không đọc được {}: {e}", path.display()));
    let mut spec: Option<u32> = None;
    for (idx, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim_end();
        if line.trim().is_empty() {
            continue;
        }
        if line.starts_with("//") {
            for tok in line.split('#').skip(1) {
                let digits: String = tok.chars().take_while(char::is_ascii_digit).collect();
                if !digits.is_empty() {
                    spec = Some(digits.parse().expect("số ca phải là số"));
                }
            }
            continue;
        }
        let (lhs, rhs) = line
            .split_once("=>")
            .unwrap_or_else(|| panic!("{file}:{}: thiếu '=>': {line}", idx + 1));
        let mut codes = Vec::new();
        let mut items = Vec::new();
        for it in split_items(rhs.trim()) {
            if let Some(code) = it.strip_prefix('!') {
                assert!(
                    code.len() == 6 && code.starts_with("VS"),
                    "mã chẩn đoán sai định dạng: {it}"
                );
                codes.push(code.to_string());
            } else {
                items.push(item_from(&it));
            }
        }
        cases.push((
            mode,
            Case {
                file,
                line: idx + 1,
                spec: spec
                    .take()
                    .unwrap_or_else(|| panic!("{file}:{}: thiếu '// §8 #N'", idx + 1)),
                input: unescape(lhs.trim()),
                items,
                codes,
            },
        ));
    }
}

fn load_cases() -> Vec<(Mode, Case)> {
    let base = manifest_dir();
    let mut cases = Vec::new();
    parse_file(
        "script.tok",
        &base.join("tests/lex/script.tok"),
        Mode::Script,
        &mut cases,
    );
    parse_file(
        "cmd.tok",
        &base.join("tests/lex/cmd.tok"),
        Mode::Command,
        &mut cases,
    );
    cases
}

/// Payload chuẩn hoá của một token, theo bảng của `SPEC-lex.md` §2–§3.
///
/// Dùng chung bộ giải mã [`vis_syntax::lit`] với lexer — **không** phải so vòng tròn: kỳ vọng ở
/// `.tok` là **giá trị viết tay** (`0xff` ⇒ `INT(255)`, `'ố'` ⇒ `CHAR(ố)`), nên bộ giải mã sai
/// (ví dụ hiểu `0xff` thành 15) vẫn làm ca đỏ.
///
/// `FLOAT`/`DURATION` so **nguyên văn** (`1e-9`, `2h30m`): §8 viết kỳ vọng đúng dạng nguồn, và
/// `f64` không in lại được `1e-9` theo `Display`; giá trị số của chúng do `lit`/ST-05 kiểm.
fn payload_of(tok: &Token) -> Option<String> {
    Some(match tok.kind {
        Kind::Int => lit::decode_int(&tok.text).ok()?.to_string(),
        Kind::Float => tok.text.clone(),
        Kind::Percent => tok.text.strip_suffix("pct")?.to_string(),
        Kind::Size => {
            let (v, u) = lit::size_parts(&tok.text)?;
            format!("{v},{u}")
        }
        Kind::Duration => tok.text.clone(),
        Kind::Char => lit::decode_char(&tok.text).ok()?.to_string(),
        Kind::Str | Kind::Path | Kind::MultiStr | Kind::RawStr | Kind::RawMulti => {
            let s = lit::decode_str(tok.kind, &tok.text).ok()?;
            if s.has_interp() {
                "parts".to_string()
            } else {
                s.text().replace('\n', ",")
            }
        }
        Kind::Ident | Kind::Kw | Kind::Word | Kind::Punct => tok.text.clone(),
        _ => return None,
    })
}

fn item_matches(item: &Item, tok: &Token) -> bool {
    match item {
        Item::Kind(k) => tok.kind == *k,
        Item::KindPayload(k, want) => tok.kind == *k && payload_of(tok).as_deref() == Some(want),
        Item::Punct(s) => tok.is_punct(s),
    }
}

fn describe_item(item: &Item) -> String {
    match item {
        Item::Kind(k) => k.name().to_string(),
        Item::KindPayload(k, p) => format!("{}({p})", k.name()),
        Item::Punct(s) => s.clone(),
    }
}

fn describe_token(tok: &Token) -> String {
    match payload_of(tok) {
        Some(p) if tok.kind != Kind::Ident && tok.kind != Kind::Word && tok.kind != Kind::Punct => {
            format!("{}({})", tok.kind.name(), p)
        }
        _ if tok.kind == Kind::Punct => tok.text.clone(),
        _ => tok.kind.name().to_string(),
    }
}

#[derive(Default)]
struct Failures {
    detail: Vec<String>,
    cases: usize,
    tokens: usize,
}

fn run_case(case: &Case, mode: Mode, fails: &mut Failures) {
    let out = lex(&case.input, mode);
    fails.cases += 1;
    fails.tokens += out.tokens.len();

    let mut problems: Vec<String> = Vec::new();

    // Tập mã chẩn đoán phải **bằng đúng** kỳ vọng: thiếu thì hỏng, mà thừa (VS0107 lạc chỗ,
    // VS0103 ngoài dự kiến…) cũng hỏng — nếu chỉ `contains` thì lexer phát lỗi bừa vẫn "xanh".
    let mut got_codes: Vec<&str> = out.diagnostics.iter().map(|d| d.code_str()).collect();
    got_codes.sort_unstable();
    got_codes.dedup();
    let mut want_codes: Vec<&str> = case.codes.iter().map(String::as_str).collect();
    want_codes.sort_unstable();
    if got_codes != want_codes {
        problems.push(format!(
            "chẩn đoán: mong {want_codes:?}, thật {got_codes:?}"
        ));
    }

    // Dòng chỉ có chẩn đoán ⇒ không đối chiếu token (§8 dùng cho ca lỗi).
    let only_codes = case.items.is_empty();
    if !only_codes {
        let mut toks: Vec<&Token> = out.tokens.iter().filter(|t| t.kind != Kind::Eof).collect();
        let expect_dedent = matches!(case.items.last(), Some(Item::Kind(Kind::Dedent)));
        if !expect_dedent {
            while toks.last().is_some_and(|t| t.kind == Kind::Dedent) {
                toks.pop();
            }
        }
        if toks.len() != case.items.len() {
            problems.push(format!(
                "số token: mong {} [{}], thật {} [{}]",
                case.items.len(),
                case.items
                    .iter()
                    .map(describe_item)
                    .collect::<Vec<_>>()
                    .join(" "),
                toks.len(),
                toks.iter()
                    .map(|t| describe_token(t))
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
        } else {
            for (i, (want, got)) in case.items.iter().zip(toks.iter()).enumerate() {
                if !item_matches(want, got) {
                    problems.push(format!(
                        "token #{i}: mong {}, thật {} (text {:?}, {}:{})",
                        describe_item(want),
                        describe_token(got),
                        got.text,
                        got.line,
                        got.col
                    ));
                }
            }
        }
    }

    if !problems.is_empty() {
        fails.detail.push(format!(
            "FAIL {}:{} (§8 #{}) nguồn={:?}\n     {}",
            case.file,
            case.line,
            case.spec,
            case.input,
            problems.join("\n     ")
        ));
    }
}

fn report(fails: &Failures) {
    if fails.detail.is_empty() {
        println!(
            "golden: {} ca XANH ({} token đã so)",
            fails.cases, fails.tokens
        );
        return;
    }
    for d in &fails.detail {
        println!("{d}");
    }
    panic!(
        "golden: {} ca ĐỎ / {} ca chạy ({} token)",
        fails.detail.len(),
        fails.cases,
        fails.tokens
    );
}

#[test]
fn golden_token_theo_spec_lex_8() {
    let cases = load_cases();

    // Không được giảm số ca (QD §7.5) và phải phủ đúng 60 số §8 một lần mỗi số.
    assert_eq!(
        cases.len(),
        EXPECTED_CASES,
        "golden phải có đúng {EXPECTED_CASES} ca của SPEC-lex §8"
    );
    let mut specs: Vec<u32> = cases.iter().map(|(_, c)| c.spec).collect();
    specs.sort_unstable();
    specs.dedup();
    assert_eq!(
        specs,
        (1..=EXPECTED_CASES as u32).collect::<Vec<_>>(),
        "kỳ vọng phủ đúng §8 #1..#60, không trùng/thiếu"
    );

    let mut fails = Failures::default();
    for (mode, case) in &cases {
        run_case(case, *mode, &mut fails);
    }

    let script = cases.iter().filter(|(m, _)| *m == Mode::Script).count();
    let cmd = cases.iter().filter(|(m, _)| *m == Mode::Command).count();
    assert_eq!((script, cmd), (SCRIPT_CASES, CMD_CASES));

    report(&fails);
}
