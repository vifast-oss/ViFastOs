// SPDX-License-Identifier: MIT OR Apache-2.0
//! Tính chất **kiểm được** của một lượt lex — dùng cho fuzz ST-04 và test bất biến.
//!
//! Mỗi hàm trả về danh sách vi phạm (rỗng = đạt). Không phải "test kỳ vọng": đây là bất biến
//! cấu trúc mà lexer phải giữ với **mọi** đầu vào UTF-8, kể cả đầu vào rác.

use crate::lex::{Diagnostic, Kind, LexOutput, Token};

/// Toạ độ (dòng, cột) tại `offset` theo văn bản đã NFC; tab = 4 cột (SPEC-lex §0.5).
fn line_col_at(text: &str, offset: usize) -> (u32, u32) {
    let mut line = 1u32;
    let mut col = 1u32;
    for c in text[..offset].chars() {
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

/// Mã từ vựng hợp lệ của SPEC-lex §7.
const LEX_CODES: [u16; 10] = [101, 102, 103, 104, 105, 106, 107, 108, 109, 110];

/// Kiểm một token về span/vị trí/text.
fn check_token(text: &str, tok: &Token, out: &mut Vec<String>) {
    let s = tok.span;
    if s.start > s.end || s.end > text.len() {
        out.push(format!("span ngoài biên: {s:?} (len {})", text.len()));
        return;
    }
    if !text.is_char_boundary(s.start) || !text.is_char_boundary(s.end) {
        out.push(format!("span cắt giữa ký tự: {s:?}"));
        return;
    }
    if text[s.start..s.end] != tok.text {
        out.push(format!(
            "text không khớp nguồn tại {s:?}: token {:?}",
            tok.text
        ));
    }
    if tok.kind == Kind::Eof && s.start != text.len() {
        out.push(format!("EOF không ở cuối tệp: {s:?}"));
    }
    let (line, col) = line_col_at(text, s.start);
    if (tok.line, tok.col) != (line, col) {
        out.push(format!(
            "vị trí lệch tại {s:?}: khai {}:{} thật {}:{}",
            tok.line, tok.col, line, col
        ));
    }
}

fn check_diag(text: &str, d: &Diagnostic, out: &mut Vec<String>) {
    if !LEX_CODES.contains(&d.code.id) {
        out.push(format!("mã ngoài sổ từ vựng: {}", d.code.short));
    }
    if d.span.start > d.span.end || d.span.end > text.len() {
        out.push(format!("span chẩn đoán ngoài biên: {:?}", d.span));
        return;
    }
    if !text.is_char_boundary(d.span.start) || !text.is_char_boundary(d.span.end) {
        out.push(format!("span chẩn đoán cắt giữa ký tự: {:?}", d.span));
        return;
    }
    let (line, col) = line_col_at(text, d.span.start);
    if (d.line, d.col) != (line, col) {
        out.push(format!(
            "vị trí chẩn đoán lệch tại {:?}: khai {}:{} thật {}:{}",
            d.span, d.line, d.col, line, col
        ));
    }
}

/// Toàn bộ bất biến của một lượt lex.
///
/// 1. Có đúng một `EOF`, đứng cuối, ở cuối văn bản.
/// 2. Token xếp theo thứ tự, không chồng lấn, span trong biên và **không cắt giữa ký tự**.
/// 3. `text[span] == token.text`; `line`/`col` khớp lại được từ span.
/// 4. Chẩn đoán chỉ dùng mã `VS0101..VS0110`, span/vị trí cùng quy tắc.
/// 5. `INDENT`/`DEDENT` cân bằng (kể cả DEDENT ngầm ở EOF, SPEC-lex §4.6).
/// 6. Không có `DEDENT` nào đứng trước `INDENT` đầu tiên (ngăn xếp không âm).
pub fn violations(out: &LexOutput) -> Vec<String> {
    let text = out.text.as_str();
    let mut bad = Vec::new();

    let mut prev_end = 0usize;
    let mut indent = 0i64;
    for tok in &out.tokens {
        if tok.span.start < prev_end {
            bad.push(format!(
                "chồng lấn: {} bắt đầu {} trước khi token trước kết thúc {}",
                tok.kind.name(),
                tok.span.start,
                prev_end
            ));
        }
        prev_end = prev_end.max(tok.span.end);
        check_token(text, tok, &mut bad);
        match tok.kind {
            Kind::Indent => {
                indent += 1;
            }
            Kind::Dedent => {
                indent -= 1;
                if indent < 0 {
                    bad.push("DEDENT khi chưa có INDENT".to_string());
                }
            }
            _ => {}
        }
    }
    if indent != 0 {
        bad.push(format!("INDENT/DEDENT lệch: {indent}"));
    }
    let eofs = out.tokens.iter().filter(|t| t.kind == Kind::Eof).count();
    if eofs != 1 {
        bad.push(format!("phải có đúng một EOF, có {eofs}"));
    }
    if out.tokens.last().map(|t| t.kind) != Some(Kind::Eof) {
        bad.push("token cuối không phải EOF".to_string());
    }

    for d in &out.diagnostics {
        check_diag(text, d, &mut bad);
    }
    bad
}
