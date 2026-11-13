// SPDX-License-Identifier: MIT OR Apache-2.0
//! ST-19 — `decode` phải **từ chối** dữ liệu hỏng bằng `DecodeError` **cụ thể**, không panic.

use vis_bytecode::decode::{decode, DecodeError};
use vis_bytecode::{encode, Const, Func, Instr, Module, Span, HASH_LEN, MAGIC, VERSION};

fn module_mau() -> Module {
    let mut f = Func {
        name: "main".into(),
        arity: 0,
        nparams: 0,
        nlocals: 1,
        max_stack: 2,
        code: vec![Instr::Const(0), Instr::Return],
        spans: vec![Span { start: 0, end: 1 }, Span { start: 0, end: 1 }],
    };
    f.code.push(Instr::Halt);
    f.spans.push(Span { start: 0, end: 0 });
    Module {
        version: VERSION,
        source_hash: [7u8; HASH_LEN],
        consts: vec![Const {
            kind: 0,
            text: "1".into(),
        }],
        funcs: vec![f],
        entry: 0,
    }
}

#[test]
fn module_mau_roundtrip_va_hop_le() {
    let m = module_mau();
    let b = encode(&m);
    assert_eq!(&b[..4], &MAGIC, "magic phải là VBC1");
    assert_eq!(decode(&b).expect("decode"), m);
    assert!(m.is_well_formed());
}

#[test]
fn magic_sai_bi_tu_choi() {
    let mut b = encode(&module_mau());
    b[0] = b'X';
    assert_eq!(decode(&b), Err(DecodeError::BadMagic(*b"XBC1")));
}

#[test]
fn version_sai_bi_tu_choi() {
    let mut b = encode(&module_mau());
    b[4..6].copy_from_slice(&2u16.to_le_bytes());
    assert_eq!(decode(&b), Err(DecodeError::BadVersion(2)));
}

#[test]
fn cat_cut_bi_tu_choi_khong_panic() {
    let b = encode(&module_mau());
    for n in [0, 1, 3, 5, 9, 20, b.len() - 1] {
        let r = decode(&b[..n]);
        assert!(r.is_err(), "cắt còn {n} byte phải lỗi");
    }
}

#[test]
fn do_dai_khai_bao_qua_lon_bi_tu_choi() {
    let mut b = encode(&module_mau());
    // Sửa `n_const` (ngay sau magic+version+hash) thành số khổng lồ.
    let off = 4 + 2 + HASH_LEN;
    b[off..off + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    match decode(&b) {
        Err(DecodeError::BadLength(..)) => {}
        other => panic!("phải BadLength, được {other:?}"),
    }
}

#[test]
fn tag_la_bi_tu_choi() {
    let mut g = module_mau();
    // Tag 250 không có trong bảng lệnh.
    g.funcs[0].code[0] = Instr::Halt;
    g.funcs[0].code.push(Instr::Pop);
    g.funcs[0].spans.push(Span { start: 0, end: 0 });
    let mut b = encode(&g);
    // Byte cuối cùng trước bảng span là `n_spans` — tìm tag 250 bằng cách thay lệnh `Pop` (tag 5)
    // ở vị trí đã biết: quét từ đầu tệp tới chỗ có `tag=5` cuối cùng.
    let vt = b
        .iter()
        .rposition(|x| *x == 5)
        .expect("phải có tag Pop = 5");
    b[vt] = 250;
    match decode(&b) {
        Err(DecodeError::BadTag(250)) => {}
        other => panic!("phải BadTag(250), được {other:?}"),
    }
}

#[test]
fn chi_so_const_ngoai_pham_vi_bi_tu_choi() {
    let mut g = module_mau();
    g.funcs[0].code[0] = Instr::Const(9);
    let b = encode(&g);
    match decode(&b) {
        Err(DecodeError::BadConstIdx(9)) => {}
        other => panic!("phải BadConstIdx(9), được {other:?}"),
    }
}

#[test]
fn chi_so_ham_ngoai_pham_vi_bi_tu_choi() {
    let mut g = module_mau();
    g.funcs[0].code[0] = Instr::Call(9, 0);
    let b = encode(&g);
    match decode(&b) {
        Err(DecodeError::BadFuncIdx(9)) => {}
        other => panic!("phải BadFuncIdx(9), được {other:?}"),
    }
}

#[test]
fn dich_nhay_ngoai_code_bi_tu_choi() {
    let mut g = module_mau();
    g.funcs[0].code[0] = Instr::Jump(1000);
    let b = encode(&g);
    match decode(&b) {
        Err(DecodeError::BadJump(1000)) => {}
        other => panic!("phải BadJump(1000), được {other:?}"),
    }
}

#[test]
fn utf8_hong_bi_tu_choi() {
    let mut g = module_mau();
    g.funcs[0].name = "main".into();
    let mut b = encode(&g);
    // Tên hàm nằm sau const pool; đổi 1 byte của tên thành 0xFF.
    let vt = {
        let mut i = b.len();
        while i > 0 {
            if b[i - 1] == b'n' {
                break;
            }
            i -= 1;
        }
        i - 1
    };
    b[vt] = 0xFF;
    match decode(&b) {
        Err(DecodeError::BadUtf8) => {}
        other => panic!("phải BadUtf8, được {other:?}"),
    }
}

#[test]
fn khong_co_entry_hop_le_bi_tu_choi() {
    let mut g = module_mau();
    g.entry = 5;
    let b = encode(&g);
    match decode(&b) {
        Err(DecodeError::BadFuncIdx(5)) => {}
        other => panic!("phải BadFuncIdx(5), được {other:?}"),
    }
}
