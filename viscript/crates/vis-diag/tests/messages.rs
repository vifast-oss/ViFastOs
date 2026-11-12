// SPDX-License-Identifier: MIT OR Apache-2.0
//! ST-03: bảng thông điệp **vi/en/ja** + `render` (cột theo Unicode scalar).
//!
//! Snapshot **20 ca** phủ 12 mã × 3 ngôn ngữ (có tiếng Việt dựng sẵn/tổ hợp và emoji trong nguồn);
//! `message_vi` vẫn là chuẩn của SPEC-lex §7 — test `so_khop_spec_lex_that` của lib không đổi.

use vis_diag::{lookup, render, severity_label, DiagLine, Lang, Severity, CODES, SYNTAX_CODES};

/// 20 ca snapshot: (mã, ngôn ngữ, chuỗi đúng).
const CASES: [(u16, Lang, &str); 20] = [
    (101, Lang::Vi, "chuỗi chưa đóng"),
    (102, Lang::En, "invalid escape"),
    (103, Lang::Ja, "トークン表にない不正な文字です"),
    (104, Lang::En, "invalid numeric literal / i64 overflow"),
    (
        105,
        Lang::Ja,
        "char リテラルはちょうど 1 スカラーである必要があります",
    ),
    (106, Lang::En, "byte string contains a character > 0x7F"),
    (107, Lang::Vi, "tab (cảnh báo-lỗi; tab = 4)"),
    (108, Lang::Ja, "補間 `{` が閉じられていません"),
    (109, Lang::En, "inconsistent indentation / invalid DEDENT"),
    (
        110,
        Lang::Ja,
        "`^` の文脈が不正です（コマンドモードの字句解析器が stage 先頭を判定）",
    ),
    (
        201,
        Lang::En,
        "syntax error: unexpected token / missing operand",
    ),
    (
        202,
        Lang::Ja,
        "`%` がパーセントとして使われています — `50pct` と書いてください",
    ),
    (101, Lang::En, "unterminated string"),
    (101, Lang::Ja, "文字列が閉じられていません"),
    (107, Lang::En, "tab (warning-error; tab = 4)"),
    (107, Lang::Ja, "タブ（警告エラー、タブ = 4）"),
    (201, Lang::Vi, "lỗi cú pháp: token không mong đợi/thiếu vế"),
    (
        201,
        Lang::Ja,
        "構文エラー: 予期しないトークン／項が不足しています",
    ),
    (202, Lang::En, "`%` used as percent — write `50pct`"),
    (202, Lang::Vi, "`%` dùng như phần trăm — viết `50pct`"),
];

#[test]
fn snapshot_20_ca_vi_en_ja() {
    for (id, lang, want) in CASES {
        let c = lookup(id).unwrap_or_else(|| panic!("mã {id} phải có trong sổ"));
        assert_eq!(c.message(lang), want, "{} {:?}", c.short, lang);
    }
}

#[test]
fn moi_ma_du_ba_ngon_ngu() {
    for c in CODES.iter().chain(SYNTAX_CODES.iter()) {
        for lang in [Lang::Vi, Lang::En, Lang::Ja] {
            assert!(
                !c.message(lang).is_empty(),
                "{} thiếu thông điệp {:?}",
                c.short,
                lang
            );
        }
    }
}

#[test]
fn lang_parse_va_nhan_muc() {
    assert_eq!(Lang::parse("vi"), Some(Lang::Vi));
    assert_eq!(Lang::parse("EN"), Some(Lang::En));
    assert_eq!(Lang::parse("ja"), Some(Lang::Ja));
    assert_eq!(Lang::parse("fr"), None);
    assert_eq!(severity_label(Severity::Error, Lang::Vi), "lỗi");
    assert_eq!(severity_label(Severity::Warning, Lang::En), "warning");
    assert_eq!(severity_label(Severity::Error, Lang::Ja), "エラー");
}

fn r(
    path: &str,
    line: u32,
    col: u32,
    sev: Severity,
    code: &str,
    msg: &str,
    src: Option<&str>,
) -> String {
    render(
        &DiagLine {
            path,
            line,
            col,
            severity: sev,
            code,
            message: msg,
            source: src,
        },
        Lang::Vi,
    )
}

#[test]
fn render_dong_tieu_de_va_nguon() {
    // Tiếng Việt dựng sẵn (1 scalar) trước chỗ lỗi ⇒ cột đếm theo scalar, không theo byte.
    let src = "x = \"ố\" §";
    let got = r(
        "a.vis",
        1,
        9,
        Severity::Error,
        "VS0103",
        "ký tự lạ",
        Some(src),
    );
    assert_eq!(
        got,
        "a.vis:1:9: lỗi VS0103: ký tự lạ\nx = \"ố\" §\n        ^"
    );
}

#[test]
fn render_canh_theo_emoji_va_tab() {
    // 🚀 = 1 scalar (astral) nhưng 4 byte ⇒ con trỏ phải canh theo scalar.
    let src = "x = \"🚀\" §";
    let got = r(
        "e.vis",
        2,
        9,
        Severity::Error,
        "VS0103",
        "ký tự lạ",
        Some(src),
    );
    assert_eq!(
        got,
        "e.vis:2:9: lỗi VS0103: ký tự lạ\nx = \"🚀\" §\n        ^"
    );
    // Tab = 4 cột (SPEC-lex §4) khi canh con trỏ.
    let got = r(
        "t.vis",
        1,
        5,
        Severity::Warning,
        "VS0107",
        "tab",
        Some("\t§"),
    );
    assert_eq!(got, "t.vis:1:5: cảnh báo VS0107: tab\n\t§\n    ^");
}

#[test]
fn render_khong_nguon_chi_mot_dong() {
    let got = r(
        "a.vis",
        3,
        2,
        Severity::Error,
        "VS0101",
        "chuỗi chưa đóng",
        None,
    );
    assert_eq!(got, "a.vis:3:2: lỗi VS0101: chuỗi chưa đóng");
}

/// Một ca snapshot `render`: nguồn + vị trí **theo scalar** + ngôn ngữ + output đầy đủ.
struct RC {
    path: &'static str,
    line: u32,
    col: u32,
    sev: Severity,
    code: &'static str,
    msg: &'static str,
    lang: Lang,
    src: Option<&'static str>,
    want: &'static str,
}

const E: Severity = Severity::Error;
const W: Severity = Severity::Warning;

/// **20 ca snapshot `render`** (DoD ST-03): tiếng Việt NFC/NFD, `🚀`, `👨‍👩‍👧`, tab giữa dòng,
/// dòng dài, cột ngoài biên/0, nhãn `lỗi`/`error`/`エラー`, `警告`, nguồn rỗng, CJK, nhiều tab.
const RENDER_CASES: [RC; 20] = [
    RC { path: "a.vis", line: 1, col: 9, sev: E, code: "VS0103", msg: "ký tự lạ", lang: Lang::Vi,
         src: Some("x = \"ố\" §"),
         want: "a.vis:1:9: lỗi VS0103: ký tự lạ\nx = \"ố\" §\n        ^" },
    RC { path: "b.vis", line: 1, col: 9, sev: E, code: "VS0103", msg: "ký tự lạ", lang: Lang::Vi,
         src: Some("x = \"o\u{301}\" §"),
         want: "b.vis:1:9: lỗi VS0103: ký tự lạ\nx = \"o\u{301}\" §\n        ^" },
    RC { path: "c.vis", line: 1, col: 9, sev: E, code: "VS0103", msg: "ký tự lạ", lang: Lang::Vi,
         src: Some("x = \"🚀\" §"),
         want: "c.vis:1:9: lỗi VS0103: ký tự lạ\nx = \"🚀\" §\n        ^" },
    RC { path: "d.vis", line: 1, col: 13, sev: E, code: "VS0103", msg: "ký tự lạ", lang: Lang::Vi,
         src: Some("x = \"👨‍👩‍👧\" §"),
         want: "d.vis:1:13: lỗi VS0103: ký tự lạ\nx = \"👨‍👩‍👧\" §\n            ^" },
    RC { path: "e.vis", line: 1, col: 6, sev: E, code: "VS0103", msg: "ký tự lạ", lang: Lang::Vi,
         src: Some("x\t§"),
         want: "e.vis:1:6: lỗi VS0103: ký tự lạ\nx\t§\n     ^" },
    RC { path: "f.vis", line: 1, col: 46, sev: E, code: "VS0103", msg: "ký tự lạ", lang: Lang::Vi,
         src: Some("x = aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa §"),
         want: "f.vis:1:46: lỗi VS0103: ký tự lạ\nx = aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa §\n                                             ^" },
    RC { path: "g.vis", line: 1, col: 3, sev: E, code: "VS0201",
         msg: "syntax error: unexpected token / missing operand", lang: Lang::En,
         src: Some("let x ="),
         want: "g.vis:1:3: error VS0201: syntax error: unexpected token / missing operand\nlet x =\n  ^" },
    RC { path: "h.vis", line: 1, col: 5, sev: W, code: "VS0107",
         msg: "タブ（警告エラー、タブ = 4）", lang: Lang::Ja,
         src: Some("\t§"),
         want: "h.vis:1:5: 警告 VS0107: タブ（警告エラー、タブ = 4）\n\t§\n    ^" },
    RC { path: "i.vis", line: 1, col: 5, sev: E, code: "VS0103", msg: "ký tự lạ", lang: Lang::Vi,
         src: Some("x = §\n"),
         want: "i.vis:1:5: lỗi VS0103: ký tự lạ\nx = §\n    ^" },
    RC { path: "j.vis", line: 1, col: 9, sev: E, code: "VS0103", msg: "ký tự lạ", lang: Lang::Vi,
         src: Some("x"),
         want: "j.vis:1:9: lỗi VS0103: ký tự lạ\nx\n ^" },
    RC { path: "k.vis", line: 0, col: 0, sev: E, code: "VS0101", msg: "chuỗi chưa đóng", lang: Lang::Vi,
         src: None,
         want: "k.vis:0:0: lỗi VS0101: chuỗi chưa đóng" },
    RC { path: "l.vis", line: 1, col: 11, sev: E, code: "VS0103", msg: "ký tự lạ", lang: Lang::Vi,
         src: Some("s = \"ốốố\" §"),
         want: "l.vis:1:11: lỗi VS0103: ký tự lạ\ns = \"ốốố\" §\n          ^" },
    RC { path: "m.vis", line: 1, col: 10, sev: E, code: "VS0103", msg: "ký tự lạ", lang: Lang::Vi,
         src: Some("t = \"ố🚀\" §"),
         want: "m.vis:1:10: lỗi VS0103: ký tự lạ\nt = \"ố🚀\" §\n         ^" },
    RC { path: "n.vis", line: 2, col: 4, sev: E, code: "VS0103",
         msg: "トークン表にない不正な文字です", lang: Lang::Ja,
         src: Some("abc§"),
         want: "n.vis:2:4: エラー VS0103: トークン表にない不正な文字です\nabc§\n   ^" },
    RC { path: "o.vis", line: 1, col: 2, sev: W, code: "VS0107",
         msg: "tab (warning-error; tab = 4)", lang: Lang::En,
         src: Some("x\t"),
         want: "o.vis:1:2: warning VS0107: tab (warning-error; tab = 4)\nx\t\n ^" },
    RC { path: "p.vis", line: 1, col: 5, sev: E, code: "VS0103", msg: "ký tự lạ", lang: Lang::Vi,
         src: Some("x = §\r\n"),
         want: "p.vis:1:5: lỗi VS0103: ký tự lạ\nx = §\n    ^" },
    RC { path: "q.vis", line: 1, col: 5, sev: E, code: "VS0103", msg: "ký tự lạ", lang: Lang::Vi,
         src: Some("\t§"),
         want: "q.vis:1:5: lỗi VS0103: ký tự lạ\n\t§\n    ^" },
    RC { path: "r.vis", line: 1, col: 1, sev: E, code: "VS0101", msg: "chuỗi chưa đóng", lang: Lang::Vi,
         src: Some(""),
         want: "r.vis:1:1: lỗi VS0101: chuỗi chưa đóng\n\n^" },
    RC { path: "s.vis", line: 1, col: 4, sev: E, code: "VS0103", msg: "ký tự lạ", lang: Lang::Vi,
         src: Some("日本語§"),
         want: "s.vis:1:4: lỗi VS0103: ký tự lạ\n日本語§\n   ^" },
    RC { path: "t.vis", line: 1, col: 10, sev: E, code: "VS0103", msg: "ký tự lạ", lang: Lang::Vi,
         src: Some("a\t\t§"),
         want: "t.vis:1:10: lỗi VS0103: ký tự lạ\na\t\t§\n         ^" },
];

#[test]
fn snapshot_20_render() {
    for c in RENDER_CASES {
        let got = render(
            &DiagLine {
                path: c.path,
                line: c.line,
                col: c.col,
                severity: c.sev,
                code: c.code,
                message: c.msg,
                source: c.src,
            },
            c.lang,
        );
        assert_eq!(got, c.want, "{}", c.path);
    }
}
