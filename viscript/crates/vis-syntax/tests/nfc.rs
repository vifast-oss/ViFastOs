// SPDX-License-Identifier: MIT OR Apache-2.0
//! NFC/NFD — VD-11 / SPEC-lex §0.1: **toàn bộ tệp được chuẩn hoá NFC trước khi lex**, nên
//! cùng một chữ gõ dạng tổ hợp (NFD) hay dựng sẵn (NFC) phải cho **cùng token**.

use unicode_normalization::UnicodeNormalization;
use vis_syntax::{lex, Kind, Mode};

fn nfd(s: &str) -> String {
    s.nfd().collect()
}

fn nfc(s: &str) -> String {
    s.nfc().collect()
}

/// (kind, text) của mọi token trừ EOF — đủ để so hai lượt lex.
fn sig(src: &str, mode: Mode) -> Vec<(&'static str, String)> {
    lex(src, mode)
        .tokens
        .into_iter()
        .filter(|t| t.kind != Kind::Eof)
        .map(|t| (t.kind.name(), t.text))
        .collect()
}

/// Mọi chữ cái tiếng Việt dựng sẵn (Latin-1 + Latin Extended-A/B + Latin Extended Additional).
fn chu_viet() -> String {
    let mut s = String::new();
    for c in 'à'..='ỹ' {
        if !c.is_control() {
            s.push(c);
        }
    }
    for c in [
        'đ', 'Đ', 'ơ', 'Ơ', 'ư', 'Ư', 'ă', 'Ă', 'â', 'Â', 'ê', 'Ê', 'ô', 'Ô',
    ] {
        s.push(c);
    }
    s
}

#[test]
fn moi_chu_viet_deu_dong_nhat_nfc_nfd() {
    let src = format!("let x = \"{}\"", chu_viet());
    let a = lex(&nfc(&src), Mode::Script);
    let b = lex(&nfd(&src), Mode::Script);
    assert_eq!(a, b, "NFC và NFD phải cho cùng token/vị trí");
    assert!(a.diagnostics.is_empty(), "{:?}", a.diagnostics);
}

#[test]
fn dinh_danh_viet_nfc_va_nfd_la_mot_ten() {
    let nfc_src = "số_lượng = 1";
    let nfd_src = nfd(nfc_src);
    assert_ne!(
        nfc_src.as_bytes(),
        nfd_src.as_bytes(),
        "hai dạng phải khác byte"
    );
    let a = lex(nfc_src, Mode::Script);
    let b = lex(&nfd_src, Mode::Script);
    assert_eq!(a, b);
    assert_eq!(a.tokens[0].kind, Kind::Ident);
    assert_eq!(a.tokens[0].text, "số_lượng", "token phải ở dạng NFC");
    // Cột tính trên văn bản đã NFC: `số_lượng` NFD dài 11 scalar nhưng cột của `=` là 12.
    assert_eq!(a.tokens[2].col, 12);
    assert_eq!(b.tokens[2].col, 12);
}

#[test]
fn char_literal_nfd_la_mot_scalar() {
    let src = "'ố'";
    let nfd_src = format!("'{}'", nfd("ố"));
    let a = lex(src, Mode::Script);
    let b = lex(&nfd_src, Mode::Script);
    assert_eq!(a, b);
    assert!(a.diagnostics.is_empty());
    assert_eq!(a.tokens[0].kind, Kind::Char);
    assert_eq!(b.tokens[0].text, "'ố'");
}

#[test]
fn chuoi_va_noi_suy_dong_nhat_nfc_nfd() {
    let src = "p\"tài/{liệu}\"";
    let a = lex(src, Mode::Script);
    let b = lex(&nfd(src), Mode::Script);
    assert_eq!(a, b);
    assert!(a.diagnostics.is_empty());
    assert_eq!(a.tokens[0].kind, Kind::Path);
    assert_eq!(a.tokens[0].text, src);
}

#[test]
fn hieu_ung_phu_tro_cua_nfd_len_vuot_dong_hoan_toan() {
    // Cột/dòng của chẩn đoán cũng tính trên văn bản đã NFC (SPEC-lex §0.4).
    let nfd_src = nfd("x = 'ab'");
    let out = lex(&nfd_src, Mode::Script);
    assert_eq!(out.diagnostics.len(), 1);
    assert_eq!(out.diagnostics[0].code_str(), "VS0105");
    assert_eq!(out.diagnostics[0].col, 5);
    assert_eq!(out.text, "x = 'ab'");
}

#[test]
fn bang_chu_viet_sinh_nfc_khi_in() {
    // `vis fmt` ghi ra NFC (VD-12); nguồn NFD vẫn phải ra token NFC — không còn ký tự tổ hợp.
    let out = lex(&nfd("let ố = 1"), Mode::Script);
    assert!(out.text.nfc().eq(out.text.chars()));
    assert!(!out.text.chars().any(|c| matches!(c, '\u{300}'..='\u{36f}')));
}

#[test]
fn ky_tu_to_hop_ngoai_chuoi_la_ky_tu_la() {
    // Dấu tổ hợp đứng một mình (không có ký tự gốc trước) không phải IDENT → VS0103.
    let out = lex("\u{301}", Mode::Script);
    assert_eq!(
        out.diagnostics
            .iter()
            .map(|d| d.code_str())
            .collect::<Vec<_>>(),
        vec!["VS0103"]
    );
}

#[test]
fn hai_che_do_cung_nhan_nfc() {
    let src = nfd("echo 'tài liệu'");
    let out = lex(&src, Mode::Command);
    assert_eq!(out.text, "echo 'tài liệu'");
    assert_eq!(sig(&src, Mode::Command).len(), 2);
}
