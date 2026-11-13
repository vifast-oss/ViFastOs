// SPDX-License-Identifier: MIT OR Apache-2.0
//! ST-21 — **verifier** VBC1: hợp lệ phải qua; từng loại hỏng phải bị **từ chối cụ thể** (13 biến thể).
//!
//! Mọi ca đều dựng `Module` bằng tay (đúng cái verifier thấy) — không phụ thuộc codegen.

use vis_bytecode::verify::{load_verified, need_stack, verify, LoadError, VerifyError};
use vis_bytecode::{encode, Const, Func, Instr, Module, Span, HASH_LEN, VERSION};
use vis_types::{BinOp, UnOp};

fn f(code: Vec<Instr>, nlocals: u16, max_stack: u16) -> Func {
    let n = code.len();
    Func {
        name: "f".into(),
        arity: 0,
        nparams: 0,
        nlocals,
        max_stack,
        code,
        spans: vec![Span { start: 0, end: 0 }; n],
    }
}

fn m(funcs: Vec<Func>, consts: Vec<Const>) -> Module {
    Module {
        version: VERSION,
        source_hash: [0u8; HASH_LEN],
        consts,
        funcs,
        entry: 0,
    }
}

fn cst(text: &str) -> Const {
    Const {
        kind: 0,
        text: text.into(),
    }
}

/// Module **hợp lệ** tối thiểu: in hằng rồi `Return`.
fn hop_le() -> Module {
    m(
        vec![f(
            vec![Instr::Const(0), Instr::CallBuiltin(1, 1), Instr::Return],
            1,
            2,
        )],
        vec![
            cst("1"),
            Const {
                kind: 5,
                text: "print".into(),
            },
        ],
    )
}

#[test]
fn module_hop_le_qua() {
    assert_eq!(verify(&hop_le()), Ok(()));
}

#[test]
fn version_sai() {
    let mut x = hop_le();
    x.version = 2;
    assert_eq!(verify(&x), Err(VerifyError::BadVersion(2)));
}

#[test]
fn entry_ngoai_pham_vi() {
    let mut x = hop_le();
    x.entry = 9;
    assert_eq!(verify(&x), Err(VerifyError::BadEntry(9)));
}

#[test]
fn entry_khi_khong_co_ham() {
    let x = m(vec![], vec![]);
    assert_eq!(verify(&x), Err(VerifyError::BadEntry(0)));
}

#[test]
fn chi_so_const_ngoai_pham_vi() {
    let mut x = hop_le();
    x.funcs[0].code[0] = Instr::Const(9);
    assert_eq!(
        verify(&x),
        Err(VerifyError::BadConstIdx {
            func: 0,
            pc: 0,
            idx: 9
        })
    );
}

#[test]
fn chi_so_ham_ngoai_pham_vi() {
    let mut x = hop_le();
    x.funcs[0].code[0] = Instr::Call(5, 0);
    assert_eq!(
        verify(&x),
        Err(VerifyError::BadFuncIdx {
            func: 0,
            pc: 0,
            idx: 5
        })
    );
}

#[test]
fn dicht_nhay_ngoai_code() {
    let mut x = hop_le();
    x.funcs[0].code[0] = Instr::Jump(99);
    match verify(&x) {
        Err(VerifyError::BadJump {
            func: 0,
            pc: 0,
            off: 99,
        }) => {}
        other => panic!("phải BadJump, được {other:?}"),
    }
}

#[test]
fn ngang_xep_am_bi_tu_choi() {
    // `Return` khi ngăn xếp rỗng.
    let x = m(vec![f(vec![Instr::Return], 1, 1)], vec![]);
    assert_eq!(
        verify(&x),
        Err(VerifyError::StackUnderflow { func: 0, pc: 0 })
    );
}

#[test]
fn max_stack_khai_nho_hon_bi_tu_choi() {
    // `Const; Const; Bin; Return` cần 2, khai 1.
    let mut x = hop_le();
    x.funcs[0].code = vec![
        Instr::Const(0),
        Instr::Const(0),
        Instr::Bin(BinOp::Add),
        Instr::Return,
    ];
    x.funcs[0].spans = vec![Span { start: 0, end: 0 }; 4];
    x.funcs[0].max_stack = 1;
    assert_eq!(
        verify(&x),
        Err(VerifyError::NeedStack {
            func: 0,
            declared: 1,
            need: 2
        })
    );
}

#[test]
fn max_stack_du_thi_qua() {
    let mut x = hop_le();
    x.funcs[0].code = vec![
        Instr::Const(0),
        Instr::Const(0),
        Instr::Bin(BinOp::Add),
        Instr::Return,
    ];
    x.funcs[0].spans = vec![Span { start: 0, end: 0 }; 4];
    x.funcs[0].max_stack = need_stack(&x.funcs[0].code, 0).expect("need");
    assert_eq!(verify(&x), Ok(()));
}

#[test]
fn code_rong_bi_tu_choi() {
    let mut x = hop_le();
    x.funcs[0].code.clear();
    x.funcs[0].spans.clear();
    assert_eq!(verify(&x), Err(VerifyError::EmptyCode { func: 0 }));
}

#[test]
fn roi_khoi_cuoi_ham_bi_tu_choi() {
    let mut x = hop_le();
    x.funcs[0].code = vec![Instr::Const(0)];
    x.funcs[0].spans = vec![Span { start: 0, end: 0 }; 1];
    x.funcs[0].max_stack = 1;
    assert_eq!(verify(&x), Err(VerifyError::FallsOffEnd { func: 0, pc: 0 }));
}

#[test]
fn bang_span_lech_bi_tu_choi() {
    let mut x = hop_le();
    x.funcs[0].spans.pop();
    assert_eq!(
        verify(&x),
        Err(VerifyError::SpanMismatch {
            func: 0,
            code: 3,
            spans: 2
        })
    );
}

#[test]
fn arity_lon_hon_nparams_bi_tu_choi() {
    let mut x = hop_le();
    x.funcs[0].arity = 2;
    x.funcs[0].nparams = 1;
    x.funcs[0].nlocals = 1;
    assert_eq!(
        verify(&x),
        Err(VerifyError::BadArity {
            func: 0,
            arity: 2,
            nparams: 1,
            nlocals: 1
        })
    );
}

#[test]
fn nlocals_nho_hon_nparams_bi_tu_choi() {
    let mut x = hop_le();
    x.funcs[0].nparams = 2;
    x.funcs[0].nlocals = 1;
    assert!(matches!(verify(&x), Err(VerifyError::BadArity { .. })));
}

#[test]
fn closure_thieu_bien_bat_bi_tu_choi() {
    // Hàm 1 dùng `LoadCaptured(2)` ⇒ cần ≥ 3 biến bắt; hàm 0 tạo closure với 0.
    let mut x = m(
        vec![
            f(vec![Instr::MakeClosure(1, 0), Instr::Return], 1, 1),
            f(vec![Instr::LoadCaptured(2), Instr::Return], 1, 1),
        ],
        vec![],
    );
    x.funcs[0].code.insert(0, Instr::Const(0));
    x.funcs[0].spans = vec![Span { start: 0, end: 0 }; 3];
    x.consts = vec![cst("1")];
    assert_eq!(
        verify(&x),
        Err(VerifyError::ClosureFree {
            func: 0,
            pc: 1,
            nfree: 0,
            used: 3
        })
    );
}

#[test]
fn o_cuc_bo_ngoai_pham_vi_bi_tu_choi() {
    let mut x = hop_le();
    x.funcs[0].nlocals = 2;
    x.funcs[0].code = vec![Instr::LoadLocal(5), Instr::Return];
    x.funcs[0].spans = vec![Span { start: 0, end: 0 }; 2];
    x.funcs[0].max_stack = 1;
    assert_eq!(
        verify(&x),
        Err(VerifyError::LocalOverflow {
            func: 0,
            pc: 0,
            slot: 5,
            nlocals: 2
        })
    );
}

#[test]
fn hai_duong_toi_cung_pc_voi_do_sau_khac_nhau() {
    // Đường nhảy tới `Return` với 1 giá trị, đường rơi tới đó với 2 ⇒ hai độ sâu khác nhau.
    let code = vec![
        Instr::Const(0),       // 0 (1)
        Instr::Dup,            // 1 (2)
        Instr::JumpIfFalse(2), // 2 -> 4 (1 nếu nhảy, 1 nếu rơi)
        Instr::Const(0),       // 3 (2 khi rơi)
        Instr::Return,         // 4
    ];
    let x = m(vec![f(code, 1, 3)], vec![cst("1")]);
    match verify(&x) {
        Err(VerifyError::StackMismatch { func: 0, .. }) => {}
        other => panic!("phải StackMismatch, được {other:?}"),
    }
}

#[test]
fn test_lit_voi_hang_hop_le_qua() {
    let code = vec![
        Instr::Const(0),
        Instr::TestLit(0),
        Instr::Return,
        Instr::Halt,
    ];
    let x = m(vec![f(code, 1, 2)], vec![cst("1")]);
    assert_eq!(verify(&x), Ok(()));
}

#[test]
fn call_method_can_dung_so_doi_so() {
    // `CallMethod(c, 1)` cần 2 giá trị (receiver + 1 đối số); khai 1 ⇒ StackUnderflow.
    let code = vec![Instr::Const(0), Instr::CallMethod(1, 1), Instr::Return];
    let x = m(
        vec![f(code, 1, 2)],
        vec![
            cst("1"),
            Const {
                kind: 5,
                text: "len".into(),
            },
        ],
    );
    assert_eq!(
        verify(&x),
        Err(VerifyError::StackUnderflow { func: 0, pc: 1 })
    );
}

#[test]
fn vong_lap_hop_le_qua_kiem() {
    // for: iter; IterInit; start: IterNext(end); bind; body; Jump(start); end: Halt
    let code = vec![
        Instr::Const(0),      // 0
        Instr::IterInit,      // 1
        Instr::IterNext(3),   // 2 -> 6
        Instr::StoreLocal(0), // 3
        Instr::Jump(-2),      // 4 -> 2
        Instr::Halt,          // 5 (không tới)
        Instr::Halt,          // 6
    ];
    let x = m(vec![f(code, 1, 1)], vec![cst("1")]);
    assert_eq!(verify(&x), Ok(()));
}

#[test]
fn load_verified_tu_choi_byte_hong() {
    let mut b = encode(&hop_le());
    b[0] = b'X';
    assert!(matches!(load_verified(&b), Err(LoadError::Decode(_))));
}

#[test]
fn load_verified_tu_choi_module_khong_hop_le() {
    let mut x = hop_le();
    x.funcs[0].max_stack = 0;
    let b = encode(&x);
    assert!(matches!(load_verified(&b), Err(LoadError::Verify(_))));
}

#[test]
fn load_verified_nhan_module_hop_le() {
    let x = hop_le();
    assert_eq!(load_verified(&encode(&x)).expect("phải qua"), x);
}

#[test]
fn un_hop_le_va_khong_hop_le() {
    assert!(vis_bytecode::verify::hop_le_un(UnOp::Neg));
    assert!(!vis_bytecode::verify::la_doan_mach(BinOp::Add));
    assert!(vis_bytecode::verify::la_doan_mach(BinOp::And));
}

#[test]
fn need_stack_dung_cho_moi_nhanh() {
    // Rẽ nhánh: nhánh rơi sâu tới 3, nhánh nhảy chỉ 2 ⇒ `need` = 3 (cả hai nhánh cân).
    let code = vec![
        Instr::Const(0),       // 0 (1)
        Instr::Dup,            // 1 (2)
        Instr::Const(0),       // 2 (3)
        Instr::JumpIfFalse(3), // 3 -> 6 (2)
        Instr::Pop,            // 4 (1)
        Instr::Return,         // 5 (0)
        Instr::Halt,           // 6 (2)
    ];
    assert_eq!(need_stack(&code, 0).unwrap(), 3);
}
