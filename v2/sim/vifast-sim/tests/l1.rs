// SPDX-License-Identifier: MIT OR Apache-2.0
//! L1 smoke của KT-08: **1.000 seed**, mỗi seed chạy 64 bước, kiểm tất định + bất biến store.

use vifast_sim::{trace_hash, Harness};

#[test]
fn l1_1000_seed_tat_dinh() {
    let mut hashes = std::collections::BTreeSet::new();
    for seed in 0..1000u64 {
        let mut a = Harness::new(seed, 8192, 4);
        let mut b = Harness::new(seed, 8192, 4);
        a.run(64);
        b.run(64);
        assert_eq!(
            a.trace_lines(),
            b.trace_lines(),
            "seed {seed}: hai lần chạy khác nhau"
        );
        assert_eq!(
            trace_hash(&a.trace_lines()),
            a.hash(),
            "seed {seed}: hash không tái lập"
        );
        assert!(a.store.invariant(), "seed {seed}: bất biến store vỡ");
        assert_eq!(a.events().len(), 64, "seed {seed}: phải đúng 64 event");
        hashes.insert(a.hash());
    }
    // 1.000 seed phải cho rất nhiều trace khác nhau (không suy biến về một trace).
    assert!(hashes.len() > 900, "trace quá trùng: {}", hashes.len());
    println!("# L1 OK: 1000 seed, {} trace khác nhau", hashes.len());
}
