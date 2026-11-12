// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test đỏ trước của ST-02b: `vis explain` phải chạy được (DoD "vis explain được").

use std::process::Command;

fn vis(args: &[&str]) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_vis"))
        .args(args)
        .output()
        .expect("chạy được binary vis");
    let code = out.status.code().unwrap_or(-1);
    let mut text = String::from_utf8_lossy(&out.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    (code, text)
}

#[test]
fn explain_mot_ma_co_nghia() {
    let (code, text) = vis(&["explain", "VS0103"]);
    assert_eq!(code, 0, "explain mã có thật phải exit 0, được: {text}");
    assert!(text.contains("VS0103"), "{text}");
    assert!(
        text.contains("ký tự lạ"),
        "phải in nghĩa của VS0103: {text}"
    );
}

#[test]
fn explain_ma_la_thi_do() {
    let (code, text) = vis(&["explain", "VS9999"]);
    assert_eq!(code, 1, "mã lạ phải exit 1: {text}");
    assert!(text.contains("không có mã"), "{text}");
}

#[test]
fn explain_khong_tham_so_thi_doi_cach_dung() {
    let (code, text) = vis(&["explain"]);
    assert_eq!(code, 2, "thiếu tham số phải exit 2: {text}");
    assert!(text.contains("dùng:"), "{text}");
}

#[test]
fn list_in_du_10_ma_cua_spec_lex() {
    let (code, text) = vis(&["explain", "--list"]);
    assert_eq!(code, 0, "{text}");
    for n in 101..=110u16 {
        let want = format!("VS{n:04}");
        assert!(text.contains(&want), "bảng thiếu {want}: {text}");
    }
}
