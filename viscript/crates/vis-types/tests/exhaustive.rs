// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-12 — test ĐỎ TRƯỚC (D2-05)**: `match` phải **xét hết** (`VS0402`), VD-43 #5.
//!
//! Quy ước như `tests/tyck.rs`: mỗi ca là `(chương trình tự chứa, kỳ vọng)`; `// kỳ vọng: ok(<kiểu>)`
//! ghi **trên** ca = `let y = match …` rồi `y` ⇒ kiểu của `y`; `!VS0402` = **đúng một** chẩn đoán.
//!
//! Luật xét hết (thiết kế §4.3 + VD-43 #5): enum ⇒ mọi biến thể; `bool` ⇒ `true`/`false`;
//! `T?` ⇒ `some`/`none` (hoặc `Some`/`None`); `Result` ⇒ `ok`/`err`; kiểu **vô hạn** (`int`, `str`,
//! `list`, `map`, …) ⇒ **bắt buộc `_`**; bản ghi ⇒ đủ **mọi** trường, không `..`; **guard không tính**;
//! or-pattern tính cả hai vế.

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
        "ca phải **hạ** sạch (chẩn đoán hạ: {:?})\nnguồn: {src}",
        h.codes()
    );
    let out = check(&h.hir, &h.symbols, &h.types);
    (out, h.types)
}

fn so_ca(src: &str, want: &str) -> Option<String> {
    let (out, table) = run(src);
    let got = if out.is_clean() {
        format!("ok({})", table.show(&out.value_type()))
    } else {
        format!("!{}", out.codes().join(","))
    };
    if got == want {
        // `ok(())` không được "xanh" nhờ **không kiểm gì** (khung rỗng của pha đỏ).
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

const CASES: &[(&str, &str)] = &[
    // kỳ vọng: ok(int) — enum 2 biến thể, xét đủ
    (
        "enum E:\n    A\n    B\nlet x: E = E.A\nlet y = match x:\n    A: 1\n    B: 2\ny",
        "ok(int)",
    ),
    // kỳ vọng: !VS0402 — thiếu `B`
    (
        "enum E:\n    A\n    B\nlet x: E = E.A\nlet y = match x:\n    A: 1\ny",
        "!VS0402",
    ),
    // kỳ vọng: ok(int) — `_` phủ hết
    (
        "enum E:\n    A\n    B\nlet x: E = E.A\nlet y = match x:\n    _: 1\ny",
        "ok(int)",
    ),
    // kỳ vọng: !VS0402 — arm có **guard** không chứng minh phủ (A vẫn thiếu)
    (
        "enum E:\n    A\n    B\nlet x: E = E.A\nlet y = match x:\n    A if true: 1\n    B: 2\ny",
        "!VS0402",
    ),
    // kỳ vọng: ok(int) — or-pattern tính **cả hai** vế
    (
        "enum E:\n    A\n    B\nlet x: E = E.A\nlet y = match x:\n    A | B: 1\ny",
        "ok(int)",
    ),
    // kỳ vọng: ok(int) — `bool` xét `true` + `false`
    (
        "let x: bool = true\nlet y = match x:\n    true: 1\n    false: 2\ny",
        "ok(int)",
    ),
    // kỳ vọng: !VS0402 — thiếu `false`
    (
        "let x: bool = true\nlet y = match x:\n    true: 1\ny",
        "!VS0402",
    ),
    // kỳ vọng: ok(int) — `bool` với `_`
    (
        "let x: bool = true\nlet y = match x:\n    _: 1\ny",
        "ok(int)",
    ),
    // kỳ vọng: !VS0402 — `true` có guard ⇒ `true` vẫn chưa phủ
    (
        "let x: bool = true\nlet y = match x:\n    true if false: 1\n    false: 2\ny",
        "!VS0402",
    ),
    // kỳ vọng: ok(int) — `T?` xét `some`/`none`
    (
        "let x: int? = 5\nlet y = match x:\n    some(v): v\n    none: 0\ny",
        "ok(int)",
    ),
    // kỳ vọng: !VS0402 — thiếu `none`
    (
        "let x: int? = 5\nlet y = match x:\n    some(v): v\ny",
        "!VS0402",
    ),
    // kỳ vọng: ok(int) — `Some`/`None` viết hoa cũng được
    (
        "let x: int? = 5\nlet y = match x:\n    Some(v): v\n    None: 0\ny",
        "ok(int)",
    ),
    // kỳ vọng: ok(int) — `T?` với `_`
    (
        "let x: int? = 5\nlet y = match x:\n    _: 0\ny",
        "ok(int)",
    ),
    // kỳ vọng: ok(int) — `Result` xét `Ok`/`Err`
    (
        "fn f() -> Result[int, str]:\n    Ok(1)\nlet r = f()\nlet y = match r:\n    Ok(v): v\n    Err(e): 0\ny",
        "ok(int)",
    ),
    // kỳ vọng: !VS0402 — thiếu `Err`
    (
        "fn f() -> Result[int, str]:\n    Ok(1)\nlet r = f()\nlet y = match r:\n    Ok(v): v\ny",
        "!VS0402",
    ),
    // kỳ vọng: ok(int) — `int` (vô hạn) **bắt buộc** `_`
    (
        "let x = 1\nlet y = match x:\n    _: 1\ny",
        "ok(int)",
    ),
    // kỳ vọng: !VS0402 — `int` chỉ có literal ⇒ thiếu
    ("let x = 1\nlet y = match x:\n    1: 1\ny", "!VS0402"),
    // kỳ vọng: ok(int) — `str` (vô hạn) với `_`
    (
        "let x = \"a\"\nlet y = match x:\n    _: 1\ny",
        "ok(int)",
    ),
    // kỳ vọng: ok(int) — bản ghi: đủ **mọi** trường, không `..`
    (
        "type U:\n    name: str\n    age: int\nlet u: U = {name: \"a\", age: 1}\nlet y = match u:\n    {name, age}: 1\ny",
        "ok(int)",
    ),
    // kỳ vọng: !VS0402 — bản ghi có `..` ⇒ chưa phủ hết trường
    (
        "type U:\n    name: str\n    age: int\nlet u: U = {name: \"a\", age: 1}\nlet y = match u:\n    {name, ..}: 1\ny",
        "!VS0402",
    ),
    // kỳ vọng: ok(int) — `list` (vô hạn) với `_`
    (
        "let xs = [1]\nlet y = match xs:\n    _: 1\ny",
        "ok(int)",
    ),
    // kỳ vọng: !VS0402 — `list` với pattern literal ⇒ không phủ
    (
        "let xs = [1]\nlet y = match xs:\n    [a]: a\ny",
        "!VS0402",
    ),
    // kỳ vọng: !VS0412 — hai arm khác kiểu (match dùng như biểu thức)
    (
        "enum E:\n    A\n    B\nlet x: E = E.A\nlet y = match x:\n    A: 1\n    B: \"a\"\ny",
        "!VS0412",
    ),
    // kỳ vọng: !VS0402 — `match` là **câu lệnh** vẫn phải xét hết (thiếu `B`)
    (
        "enum E:\n    A\n    B\nlet x: E = E.A\nmatch x:\n    A: print(1)",
        "!VS0402",
    ),
    // kỳ vọng: ok(()) — `match` câu lệnh xét đủ ⇒ sạch
    (
        "enum E:\n    A\n    B\nlet x: E = E.A\nmatch x:\n    A: print(1)\n    B: print(2)",
        "ok(())",
    ),
    // kỳ vọng: ok(int) — biến thể **có đối số**: binding trong arm có kiểu
    (
        "enum E:\n    A(v: int)\n    B\nlet x: E = E.B\nlet y = match x:\n    A(v): v\n    B: 0\ny",
        "ok(int)",
    ),
    // kỳ vọng: !VS0401 — biến thể `A(v: int)` nhưng pattern `A` thiếu đối số
    (
        "enum E:\n    A(v: int)\n    B\nlet x: E = E.B\nlet y = match x:\n    A: 1\n    B: 0\ny",
        "!VS0401",
    ),
    // kỳ vọng: ok(int) — biến thể có đối số, xét đủ với `_` buộc đối số
    (
        "enum E:\n    A(v: int)\n    B\nlet x: E = E.B\nlet y = match x:\n    A(_): 1\n    B: 0\ny",
        "ok(int)",
    ),
];

#[test]
fn match_phai_xet_het() {
    let mut sai = Vec::new();
    for (src, want) in CASES {
        if let Some(m) = so_ca(src, want) {
            sai.push(m);
        }
    }
    assert!(
        sai.is_empty(),
        "{}/{} ca sai\n{}",
        sai.len(),
        CASES.len(),
        sai.join("\n---\n")
    );
}

/// Tiêu chí ST-12: ≥ **20** ca `match` — giảm là đỏ.
#[test]
fn du_20_ca_match() {
    assert!(CASES.len() >= 20, "phải ≥ 20 ca, thật {}", CASES.len());
}
