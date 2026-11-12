// SPDX-License-Identifier: MIT OR Apache-2.0
//! Hồi quy từ fuzz — mỗi ca ở đây là một lỗi **có thật** đã bắt được, thu nhỏ lại.
//!
//! | Ca | Nguồn gốc | Triệu chứng |
//! |---|---|---|
//! | `R-01` | `vis-lex-fuzz --seed 7` ca 161 (2026-11-12) | dòng/cột sai sau newline **trong** chuỗi nhiều dòng hoặc trong nội suy |

use vis_syntax::lex::verify;
use vis_syntax::{lex, Kind, Mode};

fn token_text<'a>(out: &'a vis_syntax::LexOutput, text: &str) -> &'a vis_syntax::Token {
    out.tokens
        .iter()
        .find(|t| t.text == text)
        .unwrap_or_else(|| panic!("không thấy token {text:?}"))
}

/// R-01: `bump_char` cũ không đếm dòng khi ký tự là `\n`, nên mọi token sau một newline nằm
/// trong `"""…"""` (hoặc trong `{expr}` của nội suy) bị lệch dòng/cột — và bất biến
/// [`verify::violations`] bắt được ngay.
#[test]
fn r01_dong_dem_sau_newline_trong_chuoi_va_noi_suy() {
    let src = "\"\"\"a\nb\"\"\"\nx\n";
    let out = lex(src, Mode::Script);
    assert_eq!(verify::violations(&out), Vec::<String>::new());
    assert_eq!(out.tokens[0].kind, Kind::MultiStr);
    let x = token_text(&out, "x");
    assert_eq!((x.line, x.col), (3, 1), "token sau chuỗi nhiều dòng");

    let src = "\"{a\nb}\"\ny\n";
    let out = lex(src, Mode::Script);
    assert_eq!(verify::violations(&out), Vec::<String>::new());
    let y = token_text(&out, "y");
    assert_eq!((y.line, y.col), (3, 1), "token sau nội suy có newline");
}

/// Cùng gốc lỗi nhưng qua chuỗi thô nhiều dòng `r"""…"""` (không escape, không nội suy).
#[test]
fn r01b_chuoi_tho_nhieu_dong() {
    let src = "r\"\"\"a\nb\"\"\"\nz\n";
    let out = lex(src, Mode::Script);
    assert_eq!(verify::violations(&out), Vec::<String>::new());
    assert_eq!(out.tokens[0].kind, Kind::RawMulti);
    assert_eq!(
        (token_text(&out, "z").line, token_text(&out, "z").col),
        (3, 1)
    );
}
