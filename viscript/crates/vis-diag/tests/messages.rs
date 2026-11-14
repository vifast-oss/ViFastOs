// SPDX-License-Identifier: MIT OR Apache-2.0
//! ST-03: bảng thông điệp **vi/en/ja** + `render` (cột theo Unicode scalar).
//!
//! Snapshot **20 ca** phủ 12 mã × 3 ngôn ngữ (có tiếng Việt dựng sẵn/tổ hợp và emoji trong nguồn);
//! `message_vi` vẫn là chuẩn của SPEC-lex §7 — test `so_khop_spec_lex_that` của lib không đổi.

use vis_diag::{all_codes, lookup, render, severity_label, DiagLine, Lang, Severity, NAME_CODES};

/// 20 ca snapshot: (mã, ngôn ngữ, chuỗi đúng).
// **SỬA KÈM LÝ DO (ST-78)**: +9 snapshot cho 3 mã runtime mới (`VS0504`/`VS0505`/`VS0506`) × 3
// ngôn ngữ ⇒ **29 ca**; câu `VS0506` viết lại rõ nghĩa (bỏ meta "message kèm mã gốc"), id và thứ
// tự khối `RUNTIME_CODES` không đổi.
const CASES: [(u16, Lang, &str); 29] = [
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
    (
        504,
        Lang::Vi,
        "gửi vào kênh đã `close` (không nhận thêm phần tử)",
    ),
    (
        504,
        Lang::En,
        "send on a closed channel (no more elements accepted)",
    ),
    (
        504,
        Lang::Ja,
        "閉じたチャネルへの送信（これ以上受け付けません）",
    ),
    (
        505,
        Lang::Vi,
        "deadlock: kênh bị chặn và không còn tác vụ nào chạy được (khoá)",
    ),
    (
        505,
        Lang::En,
        "deadlock: the channel is blocked and no task is runnable",
    ),
    (
        505,
        Lang::Ja,
        "デッドロック: チャネルがブロックされ、実行可能なタスクがありません",
    ),
    (
        506,
        Lang::Vi,
        "tác vụ (`async fn`) lỗi: lỗi phát sinh trong thân tác vụ (kèm mã gốc)",
    ),
    (
        506,
        Lang::En,
        "task (`async fn`) failed: an error occurred inside the task body (original code included)",
    ),
    (
        506,
        Lang::Ja,
        "タスク (`async fn`) でエラー: タスク本体でエラーが発生しました（元のコード付き）",
    ),
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
    // ST-11 (VD-44 #7): phủ **cả ba** sổ đã cấp — 01xx + 02xx + 03xx.
    for c in all_codes() {
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

/// Bộ 6 snapshot cho **sổ tên** `03xx` (ST-11, VD-42/VD-44 #7): vi/en/ja cho cả 2 mã.
#[test]
fn snapshot_6_ca_so_ten_03xx() {
    let cases: [(u16, Lang, &str); 6] = [
        (301, Lang::Vi, "tên không tồn tại"),
        (301, Lang::En, "name does not exist"),
        (301, Lang::Ja, "名前が見つかりません"),
        (302, Lang::Vi, "khai báo trùng trong cùng phạm vi"),
        (302, Lang::En, "duplicate declaration in the same scope"),
        (302, Lang::Ja, "同じスコープで重複した宣言です"),
    ];
    for (id, lang, want) in cases {
        let c = lookup(id).unwrap_or_else(|| panic!("mã {id} phải có trong sổ"));
        assert_eq!(c.message(lang), want, "{} {:?}", c.short, lang);
    }
    // `lookup_str` tra được mã ở sổ thứ ba; `lookup` của khối chưa cấp vẫn `None`.
    assert_eq!(
        vis_diag::lookup_str("vs0302").map(|c| c.id),
        Some(302),
        "lookup_str phải tra cả sổ 03xx"
    );
    for c in NAME_CODES.iter() {
        assert_eq!(
            lookup(c.id).map(|x| x.short),
            Some(c.short),
            "lookup phải tra sổ tên"
        );
    }
    // Sửa kỳ vọng cũ kèm lý do: ST-12 đã cấp `04xx`; **ST-13 đã cấp** `05xx` (VD-42) ⇒ `lookup(501)`
    // nay `Some`; khối **chưa** cấp là `06xx`.
    assert!(lookup(401).is_some(), "ST-12 đã cấp sổ kiểu `04xx`");
    assert!(lookup(501).is_some(), "ST-13 đã cấp sổ runtime `05xx`");
    // **SỬA KÈM LÝ DO (ST-18)**: ST-18 cấp sổ thứ sáu `06xx` ⇒ `lookup(601)` nay `Some`; mã chưa cấp
    // là `701` (khối `07xx` của vòng sau).
    assert!(lookup(601).is_some(), "ST-18 đã cấp sổ lệnh `06xx`");
    // **SỬA KÈM LÝ DO (ST-25)**: ST-25 cấp sổ thứ bảy `07xx` (module) ⇒ `lookup(701)` nay `Some`;
    // mã chưa cấp là `801` (khối `08xx` của vòng sau).
    assert!(lookup(701).is_some(), "ST-25 đã cấp sổ module `07xx`");
    assert!(lookup(801).is_none(), "mã 801 chưa cấp (khối `08xx`)");
}

/// Bộ 12 snapshot cho **sổ kiểu** `04xx` (ST-12, VD-42): 4 mã × vi/en/ja.
#[test]
fn snapshot_12_ca_so_kieu_04xx() {
    let cases: [(u16, Lang, &str); 12] = [
        (401, Lang::Vi, "sai số lượng hoặc thứ tự đối số"),
        (401, Lang::En, "wrong number or order of arguments"),
        (401, Lang::Ja, "引数の数または順序が正しくありません"),
        (402, Lang::Vi, "`match` không xét hết trường hợp"),
        (402, Lang::En, "`match` is not exhaustive"),
        (402, Lang::Ja, "`match` が網羅的ではありません"),
        // **SỬA KÈM LÝ DO (ST-29)**: `VS0403` mở rộng nghĩa sang **phương thức** (trait/impl) —
        // Agent 0 chốt giữ mã, không cấp `VS0404`.
        (403, Lang::Vi, "trường, kiểu hoặc phương thức không tồn tại"),
        (403, Lang::En, "field, type or method does not exist"),
        (403, Lang::Ja, "フィールド・型・メソッドが存在しません"),
        (412, Lang::Vi, "kiểu không khớp"),
        (412, Lang::En, "type mismatch"),
        (412, Lang::Ja, "型が一致しません"),
    ];
    for (id, lang, want) in cases {
        let c = lookup(id).unwrap_or_else(|| panic!("mã {id} phải có trong sổ"));
        assert_eq!(c.message(lang), want, "{} {:?}", c.short, lang);
    }
}

/// Bộ 9 snapshot cho **sổ runtime** `05xx` (ST-13, VD-42): 3 mã × vi/en/ja.
#[test]
fn snapshot_9_ca_so_runtime_05xx() {
    let cases: [(u16, Lang, &str); 9] = [
        (
            501,
            Lang::Vi,
            "lỗi runtime: phép toán hoặc tính năng chưa hỗ trợ ở VS0",
        ),
        (
            501,
            Lang::En,
            "runtime error: operation or feature not supported in VS0",
        ),
        (
            501,
            Lang::Ja,
            "実行時エラー: VS0 では未対応の演算または機能です",
        ),
        (502, Lang::Vi, "chia hoặc lấy dư cho 0"),
        (502, Lang::En, "division or remainder by zero"),
        (502, Lang::Ja, "0 による除算または剰余"),
        (503, Lang::Vi, "chỉ số ngoài phạm vi"),
        (503, Lang::En, "index out of range"),
        (503, Lang::Ja, "インデックスが範囲外です"),
    ];
    for (id, lang, want) in cases {
        let c = lookup(id).unwrap_or_else(|| panic!("mã {id} phải có trong sổ"));
        assert_eq!(c.message(lang), want, "{} {:?}", c.short, lang);
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

/// Bộ 9 snapshot cho **sổ module** `07xx` (ST-25, VD-57): 3 mã × vi/en/ja.
#[test]
fn snapshot_9_ca_so_module_07xx() {
    let cases: [(u16, Lang, &str); 9] = [
        (701, Lang::Vi, "không thấy tệp module"),
        (701, Lang::En, "module file not found"),
        (701, Lang::Ja, "モジュールファイルが見つかりません"),
        (702, Lang::Vi, "vòng phụ thuộc giữa các tệp module"),
        (702, Lang::En, "dependency cycle between module files"),
        (702, Lang::Ja, "モジュールファイル間の循環依存"),
        (703, Lang::Vi, "`use` trùng hoặc mơ hồ"),
        (703, Lang::En, "duplicate or ambiguous `use`"),
        (703, Lang::Ja, "重複または曖昧な `use`"),
    ];
    for (id, lang, want) in cases {
        let c = lookup(id).unwrap_or_else(|| panic!("mã {id} phải có trong sổ module"));
        assert!(
            c.message(lang).contains(want),
            "{} {:?}: {:?} không chứa {:?}",
            c.short,
            lang,
            c.message(lang),
            want
        );
    }
    assert_eq!(vis_diag::MODULE_CODES.len(), 3);
    assert_eq!(vis_diag::BOOKS.len(), 7, "ST-25: bảy sổ mã");
    // **SỬA KÈM LÝ DO (ST-50, Agent 0: VS0605 sai sổ — trường là lỗi kiểu)**: `VS0605` đổi tên
    // thành **VS0405**, chuyển sang sổ `04xx` ⇒ `06xx` **4**, `04xx` **5**; `all_codes()` **vẫn 29**
    // (đổi tên, không thêm mã); `BOOKS` vẫn 7; `next_block()` vẫn **801**.
    assert_eq!(vis_diag::COMMAND_CODES.len(), 4, "ST-50: 06xx còn 4 mã");
    assert_eq!(
        vis_diag::TYPE_CODES.len(),
        5,
        "ST-50: 04xx có 5 mã (thêm VS0405)"
    );
    // **SỬA KÈM LÝ DO (ST-60)**: +**VS0506** (lỗi thân tác vụ `async`) ⇒ **30** mã.
    // **SỬA KÈM LÝ DO (ST-61)**: +**VS0504**/**VS0505** (kênh) ⇒ **32** mã.
    assert_eq!(vis_diag::all_codes().count(), 32);
    assert_eq!(vis_diag::next_block(), 801, "khối kế tiếp là 08xx");
}
