// SPDX-License-Identifier: MIT OR Apache-2.0
//! Khoá **VD-37** — 5 chỗ Phụ lục A lệch ví dụ tài liệu, mỗi chỗ một test (Agent 0 chốt
//! 2026-11-12, commit `c45f682`):
//!
//! 1. Cấp tệp nhận `(item | stmt)*` (A.1 chỉ viết `item*`).
//! 2. Khối một dòng dừng ở `else`/`elif` cùng dòng (A.2 đòi `NEWLINE`).
//! 3. Giá trị trường `with` parse mức `or_expr` để không nuốt `|>` (golden #17).
//! 4. `+=` là cặp token **dính nhau** `+` `=` (SPEC-lex §1 không có token `+=`); có khoảng trắng
//!    giữa `+` và `=` thì **không** phải `+=` (chiều ngược — VD-37 yêu cầu khoá cả hai).
//! 5. `...` **không** được thêm vào văn phạm (VD-08); khối dùng nó đã loại trừ theo VD-36.

use vis_syntax::cst::{Elem, Node, NodeKind};
use vis_syntax::{parse, Mode};

fn has_kind(n: &Node, kind: NodeKind) -> bool {
    if n.kind == kind {
        return true;
    }
    n.children().any(|c| has_kind(c, kind))
}

fn count_punct_tokens(n: &Node, toks: &[vis_syntax::Token], text: &str) -> usize {
    let mut count = 0;
    for e in &n.elems {
        match e {
            Elem::Token(i) => {
                let t = &toks[*i as usize];
                if t.kind == vis_syntax::Kind::Punct && t.text == text {
                    count += 1;
                }
            }
            Elem::Node(c) => count += count_punct_tokens(c, toks, text),
        }
    }
    count
}

#[test]
fn vd37_1_cau_lenh_o_cap_te() {
    // A.1 viết `item*`, nhưng tài liệu (dòng 290/342/535/549) chạy câu lệnh ở cấp tệp.
    let src = "let x = 1\nx + 1\n";
    let out = parse(src, Mode::Script);
    assert!(
        out.diagnostics.is_empty(),
        "câu lệnh cấp tệp phải parse sạch: {:?}",
        out.diagnostics
    );
    assert!(has_kind(&out.cst.root, NodeKind::LetStmt));
    assert!(has_kind(&out.cst.root, NodeKind::ExprStmt));
    // Item vẫn nhận ở cấp tệp (không phá A.1).
    let out2 = parse("fn f() -> int:\n    1\n", Mode::Script);
    assert!(out2.diagnostics.is_empty());
    assert!(has_kind(&out2.cst.root, NodeKind::FnDecl));
}

#[test]
fn vd37_2_khoi_mot_dong_dung_o_else() {
    // A.2 đòi `simple_stmt NEWLINE`, nhưng `if loud: a else: b` (dòng 342/406) không có NEWLINE
    // trước `else`.
    for src in [
        "let label = if ok: \"OK\" else: \"FAIL\"\n",
        "if loud: s.upper() else: s\n",
        "let x = if a: 1 elif b: 2 else: 3\n",
    ] {
        let out = parse(src, Mode::Script);
        assert!(
            out.diagnostics.is_empty(),
            "{src:?} phải parse sạch: {:?}",
            out.diagnostics
        );
        assert!(out.cst.render(&out.text) == out.text);
        assert!(
            has_kind(&out.cst.root, NodeKind::IfExpr) || has_kind(&out.cst.root, NodeKind::IfStmt)
        );
    }
}

#[test]
fn vd37_3_with_khong_nuot_pipe() {
    // Golden #17: `u with age: 21 |> f` = `(u with age: 21) |> f`.
    let out = parse("u with age: 21 |> f", Mode::Script);
    assert!(out.diagnostics.is_empty(), "{:?}", out.diagnostics);
    assert_eq!(
        out.cst.sexp_expr().as_deref(),
        Some("(|> (with u age: 21) f)")
    );
    // Giá trị trường vẫn nhận toán tử mức thấp nhất *dưới* `|>`.
    let out2 = parse("u with age: 21 + 1", Mode::Script);
    assert!(out2.diagnostics.is_empty(), "{:?}", out2.diagnostics);
    assert_eq!(
        out2.cst.sexp_expr().as_deref(),
        Some("(with u age: (+ 21 1))")
    );
}

#[test]
fn vd37_4_ghep_toan_tu_gan_hai_chieu() {
    // Chiều thuận: `x += 1` (hai token dính nhau) là AssignStmt với lá `+` và `=`.
    let out = parse("x += 1\n", Mode::Script);
    assert!(
        out.diagnostics.is_empty(),
        "`x += 1` phải parse sạch: {:?}",
        out.diagnostics
    );
    assert!(has_kind(&out.cst.root, NodeKind::AssignStmt));
    assert_eq!(count_punct_tokens(&out.cst.root, &out.cst.tokens, "+"), 1);
    assert_eq!(count_punct_tokens(&out.cst.root, &out.cst.tokens, "="), 1);
    // Chiều ngược (VD-37 yêu cầu): `x + = 1` có khoảng trắng ⇒ **không** phải `+=` ⇒ lỗi.
    let bad = parse("x + = 1\n", Mode::Script);
    assert_eq!(
        bad.diagnostics.len(),
        1,
        "`x + = 1` phải là 1 lỗi, thật {}",
        bad.diagnostics.len()
    );
    assert!(
        !has_kind(&bad.cst.root, NodeKind::AssignStmt),
        "`x + = 1` không được hiểu là gán ghép"
    );
    // Đối xứng với `-=` (review yêu cầu khoá cả họ toán tử).
    let sub = parse("x -= 1\n", Mode::Script);
    assert!(
        sub.diagnostics.is_empty(),
        "`x -= 1` phải sạch: {:?}",
        sub.diagnostics
    );
    assert!(has_kind(&sub.cst.root, NodeKind::AssignStmt));
    let sub_bad = parse("x - = 1\n", Mode::Script);
    assert_eq!(
        sub_bad.diagnostics.len(),
        1,
        "`x - = 1` phải là 1 lỗi, thật {}",
        sub_bad.diagnostics.len()
    );
    assert!(!has_kind(&sub_bad.cst.root, NodeKind::AssignStmt));
    // Gán thường `x = 1` vẫn là AssignStmt.
    let plain = parse("x = 1\n", Mode::Script);
    assert!(plain.diagnostics.is_empty());
    assert!(has_kind(&plain.cst.root, NodeKind::AssignStmt));
}

#[test]
fn vd37_5_khong_them_cu_phap_ba_cham() {
    // `...` không có trong văn phạm (A.1/A.2) và không có token ở SPEC-lex §1 ⇒ phải là lỗi,
    // không được âm thầm nhận (VD-08: đổi ngôn ngữ phải qua Agent 0 + SPEC).
    for src in ["fn f():\n    ...\n", "extern cmd g(..., ...rest: str)\n"] {
        let out = parse(src, Mode::Script);
        assert_eq!(
            out.diagnostics.len(),
            1,
            "{src:?} phải đúng 1 lỗi (không thêm cú pháp `...`), thật {}",
            out.diagnostics.len()
        );
    }
}
