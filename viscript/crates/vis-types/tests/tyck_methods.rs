// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-13 — tyck cho bảng phương thức lõi + giá trị câu lệnh cuối (VD-47).**
//!
//! Tệp **riêng** (không sửa `tests/tyck.rs` của ST-12) để log đỏ ST-12 vẫn khớp harness của nó.
//!
//! Quy ước như `tests/tyck.rs`: mỗi ca `(chương trình tự chứa, kỳ vọng)` với `// kỳ vọng: ok(<kiểu>)`
//! hoặc `// kỳ vọng: !VSxxxx` ghi **trên** ca; `ok(...)` = 0 chẩn đoán + `value_type()` đúng.

use vis_syntax::{lex, parse, Mode};
use vis_types::{check, lower, CheckOutput, TypeTable};

fn run(src: &str) -> (CheckOutput, TypeTable) {
    let p = parse(src, Mode::Script);
    assert!(
        p.diagnostics.is_empty(),
        "ca phải parse sạch: {:?}\nnguồn: {src}",
        p.diagnostics
            .iter()
            .map(|d| d.code_str())
            .collect::<Vec<_>>()
    );
    let h = lower(&p.cst, &lex(src, Mode::Script));
    assert!(
        h.is_clean(),
        "ca phải hạ sạch: {:?}\nnguồn: {src}",
        h.codes()
    );
    (check(&h.hir, &h.symbols, &h.types), h.types)
}

fn so_ca(src: &str, want: &str) -> Option<String> {
    let (out, table) = run(src);
    let got = if out.is_clean() {
        format!("ok({})", table.show(&out.value_type()))
    } else {
        format!("!{}", out.codes().join(","))
    };
    if got == want {
        if out.types_of_expr.is_empty() {
            return Some(format!(
                "nguồn: {src}\n   muốn: {want}\n   được: {got}\n   nhưng **chưa kiểm biểu thức nào**"
            ));
        }
        return None;
    }
    let diag: Vec<String> = out
        .diags
        .iter()
        .map(|d| format!("{} {}:{} {}", d.code.short, d.line, d.col, d.message))
        .collect();
    Some(format!(
        "nguồn: {src}\n   muốn: {want}\n   được: {got}\n   chẩn đoán: {diag:?}"
    ))
}

fn kiem(ten: &str, cases: &[(&str, &str)]) {
    let mut sai = Vec::new();
    for (src, want) in cases {
        if let Some(m) = so_ca(src, want) {
            sai.push(m);
        }
    }
    assert!(
        sai.is_empty(),
        "{ten}: {}/{} ca sai\n{}",
        sai.len(),
        cases.len(),
        sai.join("\n---\n")
    );
}

/// Bảng phương thức lõi (ST-13) — 8 ca.
const PHUONG_THUC: &[(&str, &str)] = &[
    // kỳ vọng: ok(int)
    ("let xs = [1, 2, 3]\nxs.len()", "ok(int)"),
    // kỳ vọng: ok(int?)
    ("let xs = [1, 2, 3]\nxs.get(0)", "ok(int?)"),
    // kỳ vọng: ok(int)
    ("let xs = [1, 2, 3]\nxs.sum()", "ok(int)"),
    // kỳ vọng: ok(str)
    ("\"abc\".upper()", "ok(str)"),
    // kỳ vọng: ok(list[str])
    ("let m = {\"a\": 1}\nm.keys()", "ok(list[str])"),
    // kỳ vọng: ok(str)
    ("let xs = [1]\nxs.join(\"-\")", "ok(str)"),
    // kỳ vọng: ok(list[int]) — `map` lấy kiểu trả của hàm
    (
        "fn inc(x: int) -> int:\n    x + 1\nlet xs = [1, 2]\nxs.map(inc)",
        "ok(list[int])",
    ),
    // kỳ vọng: !VS0403 — phương thức **không có** trong bảng
    ("let xs = [1]\nxs.sort()", "!VS0403"),
];

/// VD-47: `if`/`match` ở **câu lệnh cuối** cho giá trị; không `else` ⇒ `()`.
const GIA_TRI_DUOI: &[(&str, &str)] = &[
    // kỳ vọng: ok(int)
    (
        "fn f(x: int) -> int:\n    if x > 0: 1 else: 2\nf(1)",
        "ok(int)",
    ),
    // kỳ vọng: ok(int) — `match` đuôi
    (
        "fn f(x: int) -> int:\n    match x:\n        1: 10\n        _: 20\nf(1)",
        "ok(int)",
    ),
    // kỳ vọng: ok(int) — `if` lồng ở đuôi khối con
    (
        "fn f(x: int) -> int:\n    if x > 0:\n        if x > 1: 10 else: 20\n    else: 30\nf(5)",
        "ok(int)",
    ),
    // kỳ vọng: ok(()) — `if` **không** `else` ở đuôi ⇒ `()`
    ("fn f(x: int):\n    if x > 0: 1\nf(1)", "ok(())"),
];

#[test]
fn bang_phuong_thuc_loi() {
    kiem("phương thức", PHUONG_THUC);
}

#[test]
fn gia_tri_cau_lenh_cuoi_vd47() {
    kiem("giá trị đuôi", GIA_TRI_DUOI);
}

/// Tiêu chí: ≥ 4 phương thức + 1 tên lạ — giảm là đỏ.
#[test]
fn du_5_ca_phuong_thuc() {
    assert!(
        PHUONG_THUC.len() >= 5,
        "phải ≥ 5 ca, thật {}",
        PHUONG_THUC.len()
    );
}
