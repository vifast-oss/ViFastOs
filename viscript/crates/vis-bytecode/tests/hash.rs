// SPDX-License-Identifier: MIT OR Apache-2.0
//! ST-30 — hash nguồn là **SHA-256 thật** (VD-54 (1)), không còn FNV-1a × 4 seed.
//!
//! Khoá: (i) khớp **vector chuẩn** `sha256("abc")`; (ii) hai nguồn khác nhau ⇒ hash khác; (iii) vẫn
//! đúng **32 byte**; (iv) `VBC1` mã hoá/giải mã giữ nguyên hash.

use vis_bytecode::format::{source_hash, MAGIC, VERSION};
use vis_syntax::{lex, parse, Mode};
use vis_types::lower;

fn sha_hex(b: &[u8; 32]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[test]
fn khong_con_fnv_va_khop_vector_sha256() {
    // `sha256("abc")` — vector chuẩn FIPS 180-4.
    let h = source_hash("abc");
    assert_eq!(
        sha_hex(&h),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        "phải là SHA-256 thật, không phải FNV"
    );
}

#[test]
fn sha256_chuoi_rong() {
    assert_eq!(
        sha_hex(&source_hash("")),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn hai_nguon_khac_nhau_hash_khac() {
    let a = source_hash("print(1)\n");
    let b = source_hash("print(2)\n");
    assert_ne!(a, b, "hai nguồn khác ⇒ hash khác");
    assert_eq!(a.len(), 32);
    // FNV-1a×4 cũ cho cùng 32 byte nhưng **không** khớp vector SHA-256 ở trên ⇒ đã thay thật.
    assert_ne!(
        sha_hex(&a),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn hash_nguon_trong_bytecode_khop_sha256() {
    let p = "# SPDX-License-Identifier: MIT OR Apache-2.0\nprint(1)\n";
    let lexed = lex(p, Mode::Script);
    let out = parse(p, Mode::Script);
    let h = lower(&out.cst, &lexed);
    let m = vis_bytecode::compile(&h.hir, &h.symbols, p).expect("biên dịch được");
    assert_eq!(m.version, VERSION);
    assert_eq!(MAGIC, *b"VBC1");
    assert_eq!(
        m.source_hash,
        source_hash(p),
        "hash trong module = SHA-256 nguồn"
    );
}

#[test]
fn encode_decode_giu_hash() {
    let p = "print(1 + 2)\n";
    let lexed = lex(p, Mode::Script);
    let out = parse(p, Mode::Script);
    let h = lower(&out.cst, &lexed);
    let m = vis_bytecode::compile(&h.hir, &h.symbols, p).expect("biên dịch được");
    let bytes = vis_bytecode::encode(&m);
    let m2 = vis_bytecode::decode(&bytes).expect("giải mã được");
    assert_eq!(m2.source_hash, source_hash(p));
    assert_eq!(m2, m);
}
