// SPDX-License-Identifier: MIT OR Apache-2.0
//! "Mỗi tệp lỗi đúng **1 lỗi đầu**" (VD-35) — ngoài 3 ca `!ERR` của golden biểu thức.
//!
//! Parser phải dừng ở chẩn đoán cú pháp đầu tiên ở cấp tệp, không báo dồn, không panic; mã lấy từ
//! sổ cú pháp `vis_diag::SYNTAX_CODES` (`VS0201` chung, `VS0202` cho `50%`).

use vis_syntax::{parse, Mode};

fn diags(src: &str) -> Vec<(String, usize, String)> {
    let out = parse(src, Mode::Script);
    out.diagnostics
        .iter()
        .map(|d| (d.code_str().to_string(), d.span.start, d.message.clone()))
        .collect()
}

#[test]
fn moi_tep_loi_dung_mot_loi_dau() {
    // Nguồn hỏng do parser (lexer sạch): mỗi tệp đúng **1** chẩn đoán, không panic.
    let bad = [
        "let x =\n",             // thiếu vế phải
        "fn f(:\n    x\n",       // tham số hỏng
        "if :\n",                // thiếu điều kiện
        "1 +\n",                 // thiếu toán hạng
        "a .. b .. c",           // khoảng không kết hợp
        "1 < x < 10",            // so sánh không kết hợp
        "match s:\n    A(:\n",   // pattern hỏng
        "let m = {\"a\" 1}\n",   // map thiếu ':'
        "xs[0\n",                // thiếu ']'
        "f(1, --\n",             // tham số có tên thiếu tên
        "1 2\n",                 // thiếu NEWLINE sau câu lệnh (§A.2)
        "let x = 1 let y = 2\n", // thiếu NEWLINE giữa hai câu lệnh
        // **SỬA KÈM LÝ DO (ST-83, VD-109)**: `assert <expr>` là cú pháp **hợp lệ** (soft keyword đầu
        // câu lệnh) ⇒ ca cũ `assert xs == [...]` nay **0 lỗi**; thay bằng ca vẫn hỏng cú pháp: thiếu
        // xuống dòng giữa `assert` và toán hạng thứ hai.
        "assert 1 2\n",
    ];
    for src in bad {
        let d = diags(src);
        assert_eq!(
            d.len(),
            1,
            "nguồn {src:?} phải đúng **1** lỗi đầu, thật {}: {d:?}",
            d.len()
        );
        assert_eq!(d[0].0, "VS0201", "nguồn {src:?}: mã phải là VS0201");
        assert!(
            d[0].1 > 0,
            "nguồn {src:?}: lỗi phải tại chỗ vi phạm (không phải token đầu)"
        );
    }
}

#[test]
fn ba_ca_err_cua_golden_dung_ma_va_vi_tri() {
    // Hai ca không kết hợp: lỗi tại **toán tử thứ hai**, không phải đầu tệp.
    for src in ["a .. b .. c", "1 < x < 10", "1 < x <= 10"] {
        let d = diags(src);
        assert_eq!(d.len(), 1, "{src:?}: đúng 1 lỗi");
        assert_eq!(d[0].0, "VS0201");
        assert!(d[0].1 > 0, "{src:?}: lỗi không được báo ở token đầu");
    }
    // `50%`: VS0202 + gợi ý `50pct` (L3, golden #30).
    let d = diags("50%");
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].0, "VS0202");
    assert!(
        d[0].2.contains("50pct"),
        "thông điệp phải gợi ý `50pct`: {:?}",
        d[0].2
    );
    assert!(d[0].1 > 0, "lỗi phải tại `%`, không phải đầu tệp");
}

fn has_kind(n: &vis_syntax::Node, kind: vis_syntax::NodeKind) -> bool {
    n.kind == kind || n.children().any(|c| has_kind(c, kind))
}

#[test]
fn tep_sach_khong_co_chan_doan() {
    for src in [
        "let x = 1\n",
        "fn f(a: int) -> int:\n    a + 1\n",
        "match s:\n    A: 1\n    B(x): x\n",
        "1 + 2 * 3",
        // Tệp **không có newline cuối** kết thúc trong khối: lexer sinh DEDENT ngầm ở EOF mà
        // không sinh NEWLINE trước nó (SPEC-lex §4.6) ⇒ parser phải chấp nhận (review vòng 2).
        "fn f():\n    1",
        "if x:\n    y",
        "match s:\n    A: 1",
        "type T:\n    a: int",
        "fn f():\n    if x: y",
    ] {
        assert!(
            diags(src).is_empty(),
            "nguồn {src:?} phải sạch chẩn đoán, thật {:?}",
            diags(src)
        );
    }
    // `self` là primary của A.3 ⇒ thân method phải parse được.
    for src in ["self.x\n", "self.f(1)\n", "self\n", "self.a.b = 1\n"] {
        let out = parse(src, Mode::Script);
        assert!(
            out.diagnostics.is_empty(),
            "nguồn {src:?} phải sạch chẩn đoán, thật {:?}",
            out.diagnostics
        );
    }
    let out = parse("self.f(1)\n", Mode::Script);
    assert!(
        has_kind(&out.cst.root, vis_syntax::NodeKind::Call),
        "`self.f(1)` phải có node Call"
    );
    assert!(
        has_kind(&out.cst.root, vis_syntax::NodeKind::Field),
        "`self.x` phải có node field"
    );
}
