// SPDX-License-Identifier: MIT OR Apache-2.0
//! ST-19 — `codegen`: HIR ⇒ VBC1; khoá **hình dạng** mã sinh ra (không chỉ "chạy được").

use vis_bytecode::{compile, Instr, Module};

fn ma(src: &str) -> Module {
    let p = vis_syntax::parse(src, vis_syntax::Mode::Script);
    assert!(
        p.diagnostics.is_empty(),
        "parse: {:?}",
        p.diagnostics
            .iter()
            .map(|d| d.message.clone())
            .collect::<Vec<_>>()
    );
    let l = vis_syntax::lex(src, vis_syntax::Mode::Script);
    let h = vis_types::lower(&p.cst, &l);
    assert!(
        h.diagnostics.is_empty(),
        "lower: {:?}",
        h.diagnostics
            .iter()
            .map(|d| d.message.clone())
            .collect::<Vec<_>>()
    );
    compile(&h.hir, &h.symbols, src).expect("codegen")
}

#[test]
fn ham_co_arity_va_max_stack_ghi_trong_module() {
    let m = ma("fn add(a: int, b: int) -> int:\n    a + b\nprint(add(1, 2))\n");
    let add = m
        .funcs
        .iter()
        .find(|f| f.name == "add")
        .expect("phải có hàm `add`");
    assert_eq!(add.arity, 2);
    assert_eq!(add.nparams, 2);
    assert!(
        add.max_stack >= 2,
        "max_stack phải ≥ 2, thật {}",
        add.max_stack
    );
    assert!(add.code.iter().any(|i| matches!(i, Instr::Bin(_))));
    // Mỗi lệnh đều có span (bảng song song).
    assert_eq!(add.code.len(), add.spans.len());
}

#[test]
fn match_ha_thanh_test_va_nhay_khong_co_lenh_match() {
    let m = ma(
        "enum S:\n    A(n: int)\n    B\nlet s: S = S.B\nlet x = match s:\n    A(n): n\n    B: 0\nprint(x)\n",
    );
    let main = &m.funcs[m.entry as usize];
    let co_test = main
        .code
        .iter()
        .any(|i| matches!(i, Instr::TestCtor(..) | Instr::TestLit(_)));
    assert!(co_test, "match phải hạ thành Test* + nhảy");
    let co_nhay = main
        .code
        .iter()
        .any(|i| matches!(i, Instr::JumpIfFalse(_) | Instr::JumpIfNotTrue(_)));
    assert!(co_nhay, "phải có nhảy có điều kiện");
    assert!(main.code.len() >= 4);
}

#[test]
fn closure_co_make_closure_va_ham_con() {
    let m = ma("let inc = (x) => x + 1\nprint(inc(1))\n");
    let main = &m.funcs[m.entry as usize];
    assert!(
        main.code
            .iter()
            .any(|i| matches!(i, Instr::MakeClosure(..))),
        "phải có MakeClosure"
    );
    assert!(m.funcs.len() >= 2, "closure phải là hàm riêng trong module");
    for f in &m.funcs {
        assert!(!f.code.is_empty(), "hàm `{}` không được rỗng", f.name);
    }
}

#[test]
fn vong_lap_co_nhay_lui() {
    let m = ma("var n = 0\nwhile n < 3:\n    n += 1\nprint(n)\n");
    let main = &m.funcs[m.entry as usize];
    let co_lui = main.code.windows(1).enumerate().any(|(pc, w)| match w[0] {
        Instr::Jump(off) => (pc as i64 + 1 + off as i64) >= 0 && off < 0,
        _ => false,
    });
    assert!(co_lui, "vòng lặp phải có nhảy lùi (offset âm)");
}

#[test]
fn bien_di_khong_ho_tro_bao_loi_ro_rang() {
    // Codegen phải báo lỗi **có tên biến thể**, không panic/im lặng.
    // **SỬA KÈM LÝ DO (ST-56)**: `ref` **đã** hạ **identity** (VD-92) ⇒ ca này đổi sang `await`.
    // **SỬA KÈM LÝ DO (ST-60)**: `await` **đã** hạ thật (`$await`, VD-95) ⇒ ca này đổi sang khối
    // `cmd { … }` (vẫn `Unsupported("cmd-block")`).
    let src = "fn main():\n    let x = cmd { ls . }\n    print(x)\n";
    let p = vis_syntax::parse(src, vis_syntax::Mode::Script);
    let l = vis_syntax::lex(src, vis_syntax::Mode::Script);
    let h = vis_types::lower(&p.cst, &l);
    if h.diagnostics.is_empty() {
        match compile(&h.hir, &h.symbols, src) {
            Ok(_) => panic!("`cmd {{ … }}` phải bị từ chối ở VS0 (chưa hạ được)"),
            Err(e) => {
                let s = format!("{e}");
                assert!(s.contains("cmd-block"), "thông điệp phải nêu biến thể: {s}");
            }
        }
    }
}
