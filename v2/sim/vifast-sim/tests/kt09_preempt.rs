// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-09 — preempt trên sim: scheduler tối thiểu chen **giữa các bước**, cùng seed ⇒
//! cùng trace, và bất biến tổng byte được kiểm **sau mỗi** event (thiết kế §9.6 mục 9).
//!
//! Ba thread độc lập (CNode + Untyped riêng) để thứ tự chen nào cũng hợp lệ; trace ghi
//! tên thread nên seed khác ⇒ trace khác.

use std::collections::BTreeSet;

use vifast_sim::kobj::{Machine, ObjType, SlotRef};
use vifast_sim::scenario::{run_interleaved, ScenarioStep, Step};

fn slot(cnode: u32, slot: usize) -> SlotRef {
    SlotRef::new(cnode, slot)
}

fn build() -> (Machine, Vec<ScenarioStep>) {
    let mut m = Machine::new();
    let mut steps = Vec::new();
    for name in ["a", "b", "c"] {
        let c = m.add_cnode(8);
        let u = m.add_untyped(0x1000, 4096);
        let cap = m.root_cap(u);
        m.put_cap(c, 0, cap).expect("cap Untyped");
        steps.push(ScenarioStep {
            thread: name,
            step: Step::Retype {
                src: slot(c, 0),
                ty: ObjType::Tcb,
                size_bits: 0,
                flags: 0,
                dst: slot(c, 1),
            },
            expect: Ok(()),
            expect_state: None,
        });
        steps.push(ScenarioStep {
            thread: name,
            step: Step::Retype {
                src: slot(c, 0),
                ty: ObjType::Endpoint,
                size_bits: 0,
                flags: 0,
                dst: slot(c, 2),
            },
            expect: Ok(()),
            expect_state: None,
        });
        steps.push(ScenarioStep {
            thread: name,
            step: Step::Copy {
                src: slot(c, 1),
                dst: slot(c, 3),
            },
            expect: Ok(()),
            expect_state: None,
        });
        steps.push(ScenarioStep {
            thread: name,
            step: Step::Revoke {
                cnode: c,
                slot: Some(1),
            },
            expect: Ok(()),
            expect_state: None,
        });
        steps.push(ScenarioStep {
            thread: name,
            step: Step::Delete { slot: slot(c, 2) },
            expect: Ok(()),
            expect_state: None,
        });
    }
    (m, steps)
}

#[test]
fn chen_giua_cac_buoc_cung_seed_cung_trace() {
    let mut traces = BTreeSet::new();
    for seed in 0..16u64 {
        let (mut a, sa) = build();
        let (mut b, sb) = build();
        let ra = run_interleaved(&mut a, &sa, seed).unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        let rb = run_interleaved(&mut b, &sb, seed).unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        assert_eq!(
            ra.trace, rb.trace,
            "seed {seed}: cùng seed phải cho cùng trace"
        );
        assert_eq!(ra.events, sa.len(), "seed {seed}: mỗi bước đúng một event");
        assert_eq!(
            ra.checked_bytes, ra.events,
            "seed {seed}: bất biến byte phải được kiểm sau MỖI event"
        );
        a.check_all_bytes()
            .unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        traces.insert(ra.trace.join("\n"));
    }
    assert!(
        traces.len() >= 2,
        "16 seed phải chen khác nhau ít nhất 2 kiểu, chỉ thấy {}",
        traces.len()
    );
}

#[test]
fn chen_khong_lam_hong_byte_cua_tung_untyped() {
    let (mut m, steps) = build();
    run_interleaved(&mut m, &steps, 9).expect("chen với seed 9");
    m.check_all_bytes().expect("bất biến tổng byte");
    // Mỗi thread: TCB 512 + Endpoint 64, cap phụ đã revoke, Endpoint cap đã delete.
    assert_eq!(m.total_used(), 3 * (512 + 64));
}

/// Cùng seed chạy lại trên máy mới ⇒ trace byte-for-byte (không phụ thuộc thời gian).
#[test]
fn chay_lai_cung_seed_van_tat_dinh() {
    for seed in [0u64, 3, 7, 15] {
        let (mut a, sa) = build();
        let (mut b, sb) = build();
        let ra = run_interleaved(&mut a, &sa, seed).expect("lần 1");
        let rb = run_interleaved(&mut b, &sb, seed).expect("lần 2");
        assert_eq!(ra.trace, rb.trace, "seed {seed}");
        assert_eq!(ra.events, rb.events);
    }
}
