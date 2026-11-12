// SPDX-License-Identifier: MIT OR Apache-2.0
//! Cross-check KT-12 (D2-38): dump text của sim (KT-08) ↔ bản ghi 32 B; golden nhị phân tất định.

use std::fs;
use std::path::PathBuf;

use vifast_sim::Harness;
use vifast_traceformat::{decode_all, Record, Ring, KIND_SCHED};

const SEED: u64 = 7;
const STEPS: u64 = 200;
const TASKS: u32 = 4;
const CAP: usize = 4096;

fn records() -> Vec<Record> {
    let mut h = Harness::new(SEED, CAP, TASKS);
    h.run(STEPS);
    h.events()
        .iter()
        .map(|e| {
            Record::new(
                e.step,
                0,
                KIND_SCHED,
                e.task.unwrap_or(u32::MAX),
                e.used as u64,
                e.value,
            )
        })
        .collect()
}

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test/trace/golden-sim-seed7.bin")
}

#[test]
fn text_va_nhi_phan_khop_tung_ban_ghi() {
    let mut ring = Ring::new(1024);
    for r in records() {
        ring.push(r);
    }
    let bytes = ring.to_bytes();
    let back = decode_all(&bytes).expect("giải mã được dump của chính mình");
    assert_eq!(back.len(), STEPS as usize);
    assert_eq!(back, records(), "thứ tự/giá trị phải khớp dump text");
}

#[test]
fn golden_nhi_phan_tat_dinh() {
    let mut ring = Ring::new(1024);
    for r in records() {
        ring.push(r);
    }
    let bytes = ring.to_bytes();
    let path = golden_path();
    if std::env::var("WRITE_GOLDEN").is_ok() {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, &bytes).unwrap();
    }
    let golden = fs::read(&path).expect("phải có golden nhị phân đã commit");
    assert_eq!(bytes, golden, "dump nhị phân trôi so với golden");
    assert_eq!(golden.len(), STEPS as usize * 32);
}

#[test]
fn ring_ghi_de_khi_day_va_van_giai_ma_duoc() {
    let mut ring = Ring::new(8);
    for r in records() {
        ring.push(r);
    }
    assert_eq!(ring.len(), 8);
    let back = decode_all(&ring.to_bytes()).unwrap();
    assert_eq!(back, records()[STEPS as usize - 8..].to_vec());
}

#[test]
fn golden_bi_cat_bi_bat() {
    let golden = fs::read(golden_path()).expect("golden");
    let cut = &golden[..golden.len() - 3];
    assert!(decode_all(cut).is_err(), "dump cụt phải bị bắt");
}
