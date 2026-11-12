// SPDX-License-Identifier: MIT OR Apache-2.0
//! Cross-check KT-12 (D2-38) — **không tự quy chiếu**: parse golden TEXT đã ghim của sim
//! (`# scn …`, `step=… kind=run task=… used=… value=…`), dựng bản ghi 32 B từ chính các dòng đó,
//! rồi so byte với golden nhị phân đã commit + giải mã ngược lại.

use std::fs;
use std::path::PathBuf;

use vifast_traceformat::{decode_all, Record, KIND_IDLE, KIND_RUN};

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn text_golden() -> String {
    fs::read_to_string(dir().join("../../sim/vifast-sim/tests/golden/seed-7.s200.t4.cap4096.txt"))
        .expect("golden text KT-08 phải có")
}

fn bin_path() -> PathBuf {
    dir().join("../../test/trace/golden-sim-seed7.bin")
}

/// Mỗi dòng event → Record. `k=v` đọc bằng chuỗi, không dùng lại code sinh của sim.
fn records_from_text(text: &str) -> Vec<Record> {
    let mut out = Vec::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("step=") {
            let mut parts = rest.split(' ');
            let t_ns: u64 = parts.next().expect("số step").parse().expect("step");
            let mut kind = 0u16;
            let mut tcb = u32::MAX;
            let mut a = 0u64;
            let mut b = 0u64;
            for kv in parts {
                let (k, v) = kv.split_once('=').expect("k=v");
                match k {
                    "kind" => {
                        kind = match v {
                            "run" => KIND_RUN,
                            "idle" => KIND_IDLE,
                            other => panic!("kind lạ {other}"),
                        }
                    }
                    "task" => tcb = v.parse().expect("task"),
                    "used" => a = v.parse().expect("used"),
                    "value" => b = v.parse().expect("value"),
                    _ => panic!("trường lạ {k}"),
                }
            }
            out.push(Record::new(t_ns, 0, kind, tcb, a, b));
        }
    }
    out
}

#[test]
fn nhi_phan_la_anh_trung_thanh_cua_text() {
    let text = text_golden();
    let recs = records_from_text(&text);
    assert_eq!(recs.len(), 200, "golden text phải có 200 event");
    let bytes: Vec<u8> = recs.iter().flat_map(|r| r.to_bytes()).collect();
    let golden = fs::read(bin_path()).expect("golden nhị phân đã commit");
    assert_eq!(
        bytes, golden,
        "bản ghi 32 B phải là encoding trung thành của text"
    );
    assert_eq!(
        decode_all(&golden).unwrap(),
        recs,
        "decoder phải trả đúng thứ tự/giá trị"
    );
}

#[test]
fn doi_thu_tu_hoac_kind_thi_golden_khac() {
    let mut recs = records_from_text(&text_golden());
    recs.swap(3, 4);
    let bytes: Vec<u8> = recs.iter().flat_map(|r| r.to_bytes()).collect();
    assert_ne!(
        bytes,
        fs::read(bin_path()).unwrap(),
        "đảo thứ tự phải khác golden"
    );
}

#[test]
fn golden_bi_cat_hoac_them_ban_ghi_deu_bi_bat() {
    let golden = fs::read(bin_path()).unwrap();
    let cut = &golden[..golden.len() - 32];
    assert_eq!(
        decode_all(cut).unwrap().len(),
        199,
        "cụt 1 bản ghi phải còn 199"
    );
    let mut extra = golden.clone();
    extra.extend_from_slice(&[0u8; 32]);
    assert!(
        decode_all(&extra).is_err(),
        "thêm bản ghi kind=0 phải bị bắt"
    );
}
