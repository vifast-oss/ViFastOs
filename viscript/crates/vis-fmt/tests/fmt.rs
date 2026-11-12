// SPDX-License-Identifier: MIT OR Apache-2.0
//! ST-07 — 4 tiêu chí §18.4 của `vis-fmt` (đỏ trước, D2-05).
//!
//! (a) lossless trên tệp **đã đúng chuẩn** (điểm bất động); (b) idempotent; (c) tab → 4 dấu cách;
//! (d) tệp lỗi cú pháp **không** được sửa. Kèm 2 test bắt buộc: **token không đổi** và **đầu ra NFC**.

use std::path::PathBuf;
use vis_fmt::format;
use vis_syntax::{lex, Kind, Mode};

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test/fmt")
}

fn fixture(name: &str) -> String {
    let p = fixture_dir().join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("đọc {}: {e}", p.display()))
}

fn format_ok(src: &str) -> String {
    format(src).unwrap_or_else(|e| panic!("phải format được nguồn:\n{src}\n— lỗi: {e:?}"))
}

/// 15 khối ```vis đủ thân của tài liệu (cùng bộ loại trừ VD-36 như ST-05/ST-08; trích tự động).
fn doc_blocks() -> Vec<String> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/viscript/ViScript-thiet-ke.md");
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("đọc {}: {e}", p.display()));
    const EXCLUDED: [usize; 7] = [305, 432, 445, 610, 662, 737, 1412];
    let mut out = Vec::new();
    let mut it = text.lines().enumerate();
    while let Some((idx, line)) = it.next() {
        if line.trim_end() == "```vis" {
            let start = idx + 1;
            let mut body = String::new();
            for (_, l) in it.by_ref() {
                if l.trim_end() == "```" {
                    break;
                }
                body.push_str(l);
                body.push('\n');
            }
            if !EXCLUDED.contains(&start) {
                out.push(body);
            }
        }
    }
    out
}

/// 30 nguồn của golden biểu thức (`test/expr/golden.expr`; bỏ dòng chú thích `//`).
fn golden_sources() -> Vec<String> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test/expr/golden.expr");
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("đọc {}: {e}", p.display()));
    text.lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .filter_map(|l| l.split_once(" => ").map(|(src, _)| src.to_string()))
        .collect()
}

fn static_fixtures() -> Vec<(String, String)> {
    [
        "chuan.vis",
        "sai_thut_le.vis",
        "tab.vis",
        "crlf.vis",
        "nfd.vis",
        "duoi_dong_trang.vis",
    ]
    .iter()
    .map(|n| (n.to_string(), fixture(n)))
    .collect()
}

/// Nguồn **hợp lệ** để kiểm (a)/(b)/(token): tài liệu + golden + fixture tĩnh (trừ tệp lỗi).
fn valid_sources() -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = doc_blocks()
        .into_iter()
        .enumerate()
        .map(|(i, b)| (format!("khối tài liệu #{i}"), b))
        .collect();
    out.extend(
        golden_sources()
            .into_iter()
            .enumerate()
            .map(|(i, s)| (format!("golden #{i}"), s)),
    );
    out.extend(static_fixtures());
    out
}

/// Nội dung **từng chú thích** (`COMMENT` + `DOC_COMMENT`), đúng thứ tự nguồn.
fn chu_thich(text: &str) -> Vec<String> {
    lex(text, Mode::Script)
        .tokens
        .into_iter()
        .filter(|t| matches!(t.kind, Kind::Comment | Kind::DocComment))
        .map(|t| t.text)
        .collect()
}

fn non_trivia(text: &str) -> Vec<(Kind, String)> {
    lex(text, Mode::Script)
        .tokens
        .into_iter()
        .filter(|t| {
            !matches!(
                t.kind,
                Kind::Comment
                    | Kind::DocComment
                    | Kind::Newline
                    | Kind::Indent
                    | Kind::Dedent
                    | Kind::Eof
            )
        })
        .map(|t| (t.kind, t.text))
        .collect()
}

fn thut_le_boi_4(line: &str) -> bool {
    let indent = line.len() - line.trim_start_matches(' ').len();
    indent.is_multiple_of(4)
}

#[test]
fn diem_bat_dong_tai_lieu_15_khoi() {
    // (a) khối tài liệu đủ thân đã đúng chuẩn (đo trước: không tab, không khoảng trắng cuối dòng,
    // thụt lề bội 4, LF, đúng 1 newline cuối) ⇒ phải là điểm bất động byte-for-byte.
    let blocks = doc_blocks();
    assert_eq!(blocks.len(), 15, "15 khối đủ thân (VD-36)");
    for (i, b) in blocks.iter().enumerate() {
        let f = format_ok(b);
        assert_eq!(&f, b, "khối tài liệu #{i} phải là điểm bất động của fmt");
    }
}

#[test]
fn diem_bat_dong_mau_chuan() {
    let src = fixture("chuan.vis");
    assert_eq!(
        format_ok(&src),
        src,
        "tệp đã đúng chuẩn phải là điểm bất động"
    );
}

#[test]
fn idempotent_tren_moi_nguon() {
    // (b) fmt(fmt(x)) == fmt(x) trên mọi nguồn hợp lệ; nguồn lỗi thì lỗi ổn định.
    let mut ok = 0usize;
    for (name, src) in valid_sources() {
        match format(&src) {
            Ok(f) => {
                let g = format(&f).unwrap_or_else(|e| panic!("{name}: fmt lại phải được: {e:?}"));
                assert_eq!(g, f, "{name}: fmt không idempotent");
                ok += 1;
            }
            Err(e) => {
                assert!(format(&src).is_err(), "{name}: lỗi phải ổn định ({e:?})");
            }
        }
    }
    // 15 khối + 30 golden (27 hợp lệ + 3 `!ERR`) + 6 fixture tĩnh ⇒ 48 nguồn format được.
    assert_eq!(valid_sources().len(), 51, "15 khối + 30 golden + 6 fixture");
    assert!(ok >= 48, "phải phủ đủ nguồn hợp lệ, thật {ok}");
}

#[test]
fn tab_duoc_sua_thanh_bon_dau_cach() {
    // (c) tab → 4 dấu cách; thụt lề ra là bội số 4; parse đầu ra vẫn 0 lỗi.
    let f = format_ok(&fixture("tab.vis"));
    assert!(!f.contains('\t'), "đầu ra còn tab:\n{f}");
    for (i, line) in f.lines().enumerate() {
        assert!(
            thut_le_boi_4(line),
            "dòng {i} thụt lề không bội 4: {line:?}"
        );
    }
    let out = vis_syntax::parse(&f, Mode::Script);
    assert!(
        out.diagnostics.is_empty(),
        "đầu ra phải parse sạch: {:?}",
        out.diagnostics
    );
    assert!(
        f.contains("\n    let x = a + 1\n"),
        "tab 1 cấp phải thành 4 dấu cách:\n{f}"
    );
    assert!(
        f.contains("\n        x\n"),
        "tab 2 cấp phải thành 8 dấu cách:\n{f}"
    );
}

#[test]
fn sai_thut_le_duoc_sua_ve_bon() {
    let f = format_ok(&fixture("sai_thut_le.vis"));
    assert_eq!(
        f,
        "# SPDX-License-Identifier: MIT OR Apache-2.0\n\nfn f(a: int) -> int:\n    let x = a + 1\n    if x > 0:\n        x\n    else:\n        0 - x\n"
    );
}

#[test]
fn crlf_chuan_hoa_va_giu_nguyen_trong_dong() {
    let f = format_ok(&fixture("crlf.vis"));
    assert!(!f.contains('\r'), "CRLF phải chuẩn hoá LF");
    assert_eq!(
        f,
        "# SPDX-License-Identifier: MIT OR Apache-2.0\n\nlet x = 1\nlet y = x + 1\n"
    );
    // Khoảng trắng **trong dòng** (giữa token, trước chú thích) được giữ nguyên văn.
    let g = format_ok(&fixture("chuan.vis"));
    assert!(g.contains("let s = a + b      # khoảng trắng trong dòng được giữ"));
}

#[test]
fn khoang_trang_cuoi_dong_va_mot_newline_cuoi() {
    let f = format_ok(&fixture("duoi_dong_trang.vis"));
    for (i, line) in f.lines().enumerate() {
        assert_eq!(
            line,
            line.trim_end(),
            "dòng {i} còn khoảng trắng cuối: {line:?}"
        );
    }
    assert!(f.ends_with('\n'), "phải có newline cuối tệp");
    assert!(
        !f.ends_with("\n\n"),
        "chỉ đúng **một** newline cuối tệp: {f:?}"
    );
}

#[test]
fn loi_cu_phap_khong_duoc_sua() {
    // (d) tệp lỗi cú pháp: `format` trả lỗi, **không** sửa nội dung.
    let src = fixture("loi_cu_phap.vis");
    let err = format(&src).expect_err("tệp `let x =` phải là lỗi");
    assert_eq!(err.code, "VS0201", "mã lỗi đầu phải là VS0201: {err:?}");
    assert!(err.line > 0 && err.col > 0, "phải có vị trí: {err:?}");
}

#[test]
fn token_khong_doi() {
    // Không mất/đổi token (trừ trivia): re-lex đầu ra ⇒ cùng dãy (kind, text) với đầu vào.
    let mut n = 0usize;
    for (name, src) in valid_sources() {
        if let Ok(f) = format(&src) {
            assert_eq!(
                non_trivia(&src),
                non_trivia(&f),
                "{name}: token bị đổi sau khi format"
            );
            // Chú thích là trivia nhưng **không** được mất/đổi nội dung (VD-39).
            assert_eq!(
                chu_thich(&src),
                chu_thich(&f),
                "{name}: chú thích bị mất hoặc đổi nội dung sau khi format"
            );
            assert_eq!(
                chu_thich(&src).len(),
                chu_thich(&f).len(),
                "{name}: số chú thích trước/sau phải bằng nhau"
            );
            n += 1;
        }
    }
    assert!(n >= 48, "phải so token trên đủ nguồn hợp lệ, thật {n}");
}

#[test]
fn dau_ra_nfc() {
    // Đầu vào NFD ⇒ đầu ra NFC (bằng `LexOutput.text` của lexer).
    let nfd = fixture("nfd.vis");
    let out = format_ok(&nfd);
    let nfc = lex(&nfd, Mode::Script).text;
    assert_eq!(out, nfc, "đầu ra phải là NFC");
    assert_eq!(format_ok(&nfc), out, "NFC là điểm bất động");
}

#[test]
fn dong_noi_giu_thut_le_goc_sau_chu_thich() {
    // MAJOR-1 (review vòng 1): dòng nối trong ngoặc hoặc bắt đầu `|>` giữ thụt lề gốc, kể cả khi
    // dòng trước có chú thích cùng dòng — nếu không, `b,`/`a,`/`|>` bị kéo về cột 0.
    let cases = [
        "let x = foo(\n    a,  # c\n    b,\n)\n",
        "let x = foo(\n    # c\n    a,\n)\n",
        "let xs = a |> b  # c\n        |> f\n",
    ];
    for src in cases {
        assert_eq!(format_ok(src), src, "phải là điểm bất động: {src:?}");
    }
}

#[test]
fn tab_trong_dong_va_trong_chu_thich_duoc_giu() {
    // MAJOR-2/3 (review vòng 1): VD-39 chỉ cho đổi tab ở **phần thụt lề**; tab trong gap cùng dòng
    // và trong thân chú thích giữ nguyên văn.
    let src = "let x =\t1\nlet y = 2\t# a\tb\n";
    let f = format_ok(src);
    assert_eq!(f, src, "tab trong dòng/chú thích phải giữ nguyên");
    assert!(f.contains("=\t1"), "tab trong gap cùng dòng bị đổi: {f:?}");
    assert!(
        f.contains("# a\tb"),
        "tab trong thân chú thích bị đổi: {f:?}"
    );
}

#[test]
fn dong_chi_chu_thich_va_dong_noi_giu_nguyen() {
    // Dòng chỉ có chú thích và dòng nối (`|>` / trong ngoặc) không phải "cấp khối": giữ thụt lề gốc
    // (chỉ đổi tab), nên nguồn đã đúng chuẩn vẫn là điểm bất động.
    let src = "fn f():\n    # chú thích riêng\n    let xs = src.walk()\n        |> filter(x => x > 1)\n    xs\n";
    let f = format_ok(src);
    assert_eq!(f, src, "dòng chú thích + dòng nối phải được giữ nguyên");
    assert_eq!(format_ok(&f), f, "idempotent");
}
