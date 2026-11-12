// SPDX-License-Identifier: MIT OR Apache-2.0
//! Golden **CmdCST** — ST-06 pha A (harness **đỏ trước**, D2-05).
//!
//! 33 ca: 12 hàng bảng §9.4 (đúng thứ tự) + 10 ca mơ hồ + 1 ca `.vish` nhiều dòng + 10 ca bổ sung
//! sau review vòng 1; quy ước sexp ở đầu `test/cmd/golden.cmd`.
//! Harness khoá: đủ **22** ca và đủ **12** hàng §9.4; sexp thật; lossless; ca `!ERR` đúng **1**
//! chẩn đoán và lỗi **không** ở token đầu.

use std::path::PathBuf;
use vis_syntax::{parse, Mode};

const EXPECTED_CASES: usize = 33;
const ROWS_9_4: usize = 12;

fn golden() -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test/cmd/golden.cmd");
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("đọc {}: {e}", p.display()))
}

/// Tách `<nguồn> => <sexp>`: dùng lần xuất hiện **cuối cùng** của `" => "` mà phần sau bắt đầu
/// bằng `(` hoặc `!` (nguồn có thể chứa `=>` của closure — xem đầu `golden.cmd`).
fn tach(line: &str) -> (String, String) {
    let mut cat = None;
    for (i, _) in line.match_indices(" => ") {
        let rest = &line[i + 4..];
        if rest.starts_with('(') || rest.starts_with('!') {
            cat = Some(i);
        }
    }
    let i = cat.unwrap_or_else(|| panic!("dòng không có dấu phân cách ` => `: {line:?}"));
    // `\n` (hai ký tự) trong trường nguồn = xuống dòng thật (ca `.vish` nhiều dòng).
    let src = line[..i].replace("\\n", "\n");
    (src, line[i + 4..].to_string())
}

fn cases() -> Vec<(String, String)> {
    golden()
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.is_empty() && !l.starts_with("//"))
        .map(tach)
        .collect()
}

/// Số hiệu trong các comment `// #<n> …` (dùng để khoá 12 hàng §9.4 + 10 ca mơ hồ).
fn so_hieu() -> Vec<usize> {
    golden()
        .lines()
        .filter_map(|l| {
            let rest = l.strip_prefix("// #")?;
            rest.split_whitespace().next()?.parse().ok()
        })
        .collect()
}

#[test]
fn dung_22_ca_va_12_hang_bang_9_4() {
    let cs = cases();
    assert_eq!(
        cs.len(),
        EXPECTED_CASES,
        "phải đúng 22 ca (§9.4 12 hàng + 10 ca mơ hồ)"
    );
    let so = so_hieu();
    let hang: Vec<usize> = so.iter().copied().filter(|n| *n <= ROWS_9_4).collect();
    assert_eq!(
        hang,
        (1..=ROWS_9_4).collect::<Vec<_>>(),
        "12 hàng §9.4 phải có đúng thứ tự #1..#12"
    );
    let con_lai: Vec<usize> = so.iter().copied().filter(|n| *n > ROWS_9_4).collect();
    assert_eq!(
        con_lai,
        (13..=EXPECTED_CASES).collect::<Vec<_>>(),
        "#13..#33 phải đánh số liên tục (mơ hồ + .vish + bổ sung sau review)"
    );
}

/// Kiểm **một** ca (sexp hoặc `!ERR`); mỗi ca là một test riêng để log đỏ đếm được 22/22.
fn kiem_ca(i: usize) {
    let (src, mong) = cases()
        .into_iter()
        .nth(i)
        .unwrap_or_else(|| panic!("không có ca thứ {i}"));
    let out = parse(&src, Mode::Command);
    if mong.starts_with("!ERR") {
        assert_eq!(
            out.diagnostics.len(),
            1,
            "{src:?}: phải đúng **1** lỗi đầu, thật {:?}",
            out.diagnostics
        );
        let d = &out.diagnostics[0];
        assert!(
            d.span.start > 0,
            "{src:?}: lỗi không được ở token đầu: {d:?}"
        );
        if let Some(needle) = mong.strip_prefix("!ERR \"") {
            let needle = needle.trim_end_matches('"');
            assert!(
                d.message.contains(needle),
                "{src:?}: thông điệp phải chứa {needle:?}: {:?}",
                d.message
            );
        }
    } else {
        assert!(
            out.diagnostics.is_empty(),
            "{src:?}: ca hợp lệ không được có chẩn đoán: {:?}",
            out.diagnostics
        );
        assert_eq!(out.cst.sexp_cmd(), mong, "nguồn {src:?}: CmdCST sai");
    }
}

macro_rules! ca {
    ($($f:ident = $i:expr;)*) => {
        $(
            #[test]
            fn $f() {
                kiem_ca($i);
            }
        )*
    };
}

ca! {
    ca01 = 0; ca02 = 1; ca03 = 2; ca04 = 3; ca05 = 4; ca06 = 5;
    ca07 = 6; ca08 = 7; ca09 = 8; ca10 = 9; ca11 = 10; ca12 = 11;
    ca13 = 12; ca14 = 13; ca15 = 14; ca16 = 15; ca17 = 16; ca18 = 17;
    ca19 = 18; ca20 = 19; ca21 = 20; ca22 = 21; ca23 = 22;
    ca24 = 23; ca25 = 24; ca26 = 25; ca27 = 26; ca28 = 27; ca29 = 28;
    ca30 = 29; ca31 = 30; ca32 = 31; ca33 = 32;
}

#[test]
fn lossless_moi_ca() {
    let mut sai = Vec::new();
    for (src, _) in cases() {
        let out = parse(&src, Mode::Command);
        let lai = out.cst.render(&out.text);
        if lai != out.text {
            sai.push(format!("{src:?}: render khác nguồn: {lai:?}"));
        }
    }
    assert!(sai.is_empty(), "không lossless:\n{}", sai.join("\n"));
}
