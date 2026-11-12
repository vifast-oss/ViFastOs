// SPDX-License-Identifier: MIT OR Apache-2.0
//! ST-03: `Diagnostic.col`/`line` đếm theo **Unicode scalar** (1-based), tab = 4 cột (SPEC-lex §4).
//!
//! Lexer đã đếm bằng `char` từ ST-04 — bộ test này **khoá lại** (không sửa lexer): tiếng Việt
//! dựng sẵn (NFC) + tổ hợp (NFD), emoji astral (1 scalar), emoji gia đình (ZWJ, nhiều scalar), tab.

use vis_syntax::{parse, Mode};

/// Cột (1-based) của chẩn đoán **đầu tiên**.
fn col(src: &str) -> (u32, u32) {
    let out = parse(src, Mode::Script);
    let d = out
        .diagnostics
        .first()
        .unwrap_or_else(|| panic!("nguồn phải có lỗi: {src:?}"));
    (d.line, d.col)
}

#[test]
fn tieng_viet_dung_san_va_to_hop() {
    // "ố" NFC = 1 scalar; chỗ lỗi `§` sau 8 scalar ⇒ cột 9 (nếu đếm byte: ố = 2 byte ⇒ 10).
    assert_eq!(col("x = \"ố\" §"), (1, 9));
    // Cùng chữ nhưng gõ NFD (o + U+0301): chuẩn hoá NFC trước khi lex ⇒ vẫn cột 9.
    assert_eq!(col("x = \"o\u{301}\" §"), (1, 9));
}

#[test]
fn emoji_astral_la_mot_scalar() {
    // 🚀 = 1 scalar (4 byte UTF-8) ⇒ `§` vẫn ở cột 9.
    assert_eq!(col("x = \"🚀\" §"), (1, 9));
}

#[test]
fn emoji_gia_dinh_nhieu_scalar_va_zwj() {
    // "👨‍👩‍👧" = 3 emoji + 2 ZWJ = 5 scalar ⇒ `§` ở cột 13 (byte thì khác hẳn).
    assert_eq!(col("x = \"👨‍👩‍👧\" §"), (1, 13));
}

#[test]
fn tab_bon_cot_va_vi_tri_sau_tab() {
    // Tab báo VS0107 ở cột 1 (mã đầu); sau tab lexer coi như 4 cột ⇒ `§` ở cột 5.
    let out = parse("\t§", Mode::Script);
    let vi_tri: Vec<(u32, u32, &'static str)> = out
        .diagnostics
        .iter()
        .filter(|d| d.code.id / 100 == 1) // chỉ tầng từ vựng (parser thêm VS0201 hệ quả)
        .map(|d| (d.line, d.col, d.code_str()))
        .collect();
    assert_eq!(vi_tri, vec![(1, 1, "VS0107"), (1, 5, "VS0103")]);
}

#[test]
fn dong_theo_scalar_khong_phai_byte() {
    // Dòng 2 sau tiếng Việt + emoji: vẫn là dòng 2, cột 1.
    assert_eq!(col("x = \"ố🚀\"\n§"), (2, 1));
}
