// SPDX-License-Identifier: MIT OR Apache-2.0
//! Bytecode **hỏng** ⇒ VM trả `VS0501` "bytecode hỏng", **không** panic (VD-52 #2).

use vis_bytecode::{Func, Instr, Module, Span, HASH_LEN, VERSION};

fn module(code: Vec<Instr>) -> Module {
    let n = code.len();
    Module {
        version: VERSION,
        source_hash: [0u8; HASH_LEN],
        consts: vec![],
        funcs: vec![Func {
            name: "main".into(),
            arity: 0,
            nparams: 0,
            nlocals: 1,
            max_stack: 1,
            code,
            spans: vec![Span { start: 0, end: 0 }; n],
        }],
        entry: 0,
    }
}

#[test]
fn pop_rong_bi_tu_choi_khong_panic() {
    let m = module(vec![Instr::Pop, Instr::Halt]);
    let out = vis_vm::run(&m, "", &[]);
    assert_eq!(out.exit, 1);
    assert_eq!(out.codes(), vec!["VS0501"]);
    assert!(
        out.diagnostics[0].message.contains("bytecode hỏng"),
        "thông điệp: {}",
        out.diagnostics[0].message
    );
}

#[test]
fn dich_nhay_ngoai_code_bi_tu_choi() {
    // `decode` đã chặn ở tầng đọc; VM kiểm lại **rẻ** khi module dựng tay.
    let m = module(vec![Instr::Jump(50), Instr::Halt]);
    let out = vis_vm::run(&m, "", &[]);
    assert_eq!(out.exit, 1);
    assert_eq!(out.codes(), vec!["VS0501"]);
}

#[test]
fn const_ngoai_pham_vi_bi_tu_choi() {
    let m = module(vec![Instr::Const(9), Instr::Halt]);
    let out = vis_vm::run(&m, "", &[]);
    assert_eq!(out.exit, 1);
    assert_eq!(out.codes(), vec!["VS0501"]);
}

#[test]
fn chuong_trinh_rong_tra_unit() {
    let m = module(vec![Instr::Halt]);
    let out = vis_vm::run(&m, "", &[]);
    assert_eq!(out.exit, 0);
    assert!(out.diagnostics.is_empty());
    assert_eq!(out.stdout, "");
}
