// SPDX-License-Identifier: MIT OR Apache-2.0
//! Golden trace + kiểm tất định **qua hai tiến trình** (review #11):
//! cùng seed phải cho byte giống nhau, và máy so sánh phải **bắt được** lệch.

use std::process::Command;

const SEED: u64 = 7;
const STEPS: u64 = 200;
const TASKS: u32 = 4;
const CAP: usize = 4096;
const GOLDEN: &str = include_str!("golden/seed-7.s200.t4.cap4096.txt");

fn fresh_dump() -> String {
    let mut h = vifast_sim::Harness::new(SEED, CAP, TASKS);
    h.run(STEPS);
    let mut s = h.dump(STEPS).join("\n");
    s.push('\n');
    s.push_str(&format!("# trace-hash={:016x}\n", h.dump_hash(STEPS)));
    s
}

#[test]
fn golden_khop_tung_byte() {
    assert_eq!(fresh_dump(), GOLDEN, "trace trôi so với golden");
}

/// Ca âm của chính máy so sánh: đổi 1 byte trong golden thì **phải** thấy khác.
#[test]
fn may_so_sanh_bat_duoc_lech_1_byte() {
    let mut broken: Vec<u8> = GOLDEN.as_bytes().to_vec();
    let pos = broken
        .iter()
        .position(|b| *b == b'r')
        .expect("golden phải có chữ r");
    broken[pos] = b'X';
    let broken = String::from_utf8(broken).expect("utf8");
    assert_ne!(fresh_dump(), broken, "so sánh không bắt được lệch 1 byte");
}

/// Hai **tiến trình riêng** cùng seed ⇒ stdout giống nhau và giống golden.
#[test]
fn hai_tien_trinh_cung_seed_giong_nhau() {
    let bin = env!("CARGO_BIN_EXE_vifast-sim");
    let run = || {
        let out = Command::new(bin)
            .args([
                "--seed",
                &SEED.to_string(),
                "--steps",
                &STEPS.to_string(),
                "--tasks",
                &TASKS.to_string(),
                "--cap",
                &CAP.to_string(),
            ])
            .output()
            .expect("chạy được binary");
        assert!(out.status.success(), "binary exit khác 0");
        String::from_utf8(out.stdout).expect("stdout utf8")
    };
    let a = run();
    let b = run();
    assert_eq!(a, b, "hai tiến trình cùng seed cho kết quả khác nhau");
    assert_eq!(a, GOLDEN, "tiến trình khác golden");
}
