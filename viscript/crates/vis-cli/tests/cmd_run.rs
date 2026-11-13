// SPDX-License-Identifier: MIT OR Apache-2.0
//! ST-18 — `.vish` chạy được: `vis check` = parse + `cmd_lower` + tyck, `vis run` chạy qua interp.
//!
//! Chạy **qua binary** (`CARGO_BIN_EXE_vis`) như `sem.rs`: mỗi ca là một tệp `.vish` tạm, so
//! **stdout byte**, mã thoát và mã chẩn đoán. Đỏ trước ST-18 vì `vis run *.vish` còn `Bail out!`.

use std::path::PathBuf;
use std::process::Command;

fn vis() -> &'static str {
    env!("CARGO_BIN_EXE_vis")
}

fn tam(ten: &str, noi_dung: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("vis-cmd-run-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("tạo thư mục tạm");
    let p = dir.join(format!("{ten}.vish"));
    std::fs::write(&p, noi_dung).expect("ghi tệp tạm");
    p
}

fn chay(ten: &str, src: &str) -> (i32, String, String) {
    let p = tam(ten, src);
    let out = Command::new(vis())
        .arg("run")
        .arg(&p)
        .output()
        .expect("chạy vis run");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

fn check(ten: &str, src: &str) -> (i32, String) {
    let p = tam(ten, src);
    let out = Command::new(vis())
        .arg("check")
        .arg(&p)
        .output()
        .expect("chạy vis check");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr),
    )
}

#[test]
fn pipeline_ba_stage_chay() {
    let (code, out, err) = chay("ba-stage", "echo \"x\\ny\\nz\" | lines | take 2\n");
    assert_eq!((code, out.as_str()), (0, "[x, y]\n"), "stderr: {err}");
}

#[test]
fn flag_co_default() {
    // `--flag` + default: `--upper: bool = false` ⇒ bật thì in hoa (§A.7 `flag`, VD-40 #2).
    let (code, out, err) = chay("flag", "echo \"a\" \"b\" --upper\n");
    assert_eq!((code, out.as_str()), (0, "A B\n"), "stderr: {err}");
}

#[test]
fn hai_item_bang_cham_phay() {
    let (code, out, err) = chay("semi", "echo \"a\"; echo \"b\"\n");
    assert_eq!((code, out.as_str()), (0, "a\nb\n"), "stderr: {err}");
}

#[test]
fn lenh_khong_ton_tai_la_vs0601() {
    let (code, out, err) = chay("khong-co", "khong_co \"x\"\n");
    assert_eq!(code, 2, "thiếu lệnh là lỗi **tĩnh**: {err} / {out}");
    assert!(err.contains("VS0601"), "stderr: {err}");
}

#[test]
fn sai_doi_so_la_vs0602() {
    let (code, _, err) = chay("sai-args", "echo\n");
    assert_eq!(code, 2, "stderr: {err}");
    assert!(err.contains("VS0602"), "stderr: {err}");
}

#[test]
fn noi_khong_duoc_la_vs0603() {
    let (code, _, err) = chay("noi", "echo \"a\" | take 1\n");
    assert_eq!(code, 2, "stderr: {err}");
    assert!(err.contains("VS0603"), "stderr: {err}");
}

#[test]
fn chay_nen_la_vs0604() {
    let (code, _, err) = chay("nen", "echo \"a\" &\n");
    assert_eq!(code, 2, "stderr: {err}");
    assert!(err.contains("VS0604"), "stderr: {err}");
}

#[test]
fn check_khong_con_bail_out() {
    let (code, out) = check("ok", "echo \"a\" | lines\n");
    assert_eq!(code, 0, "vis check phải xanh: {out}");
    assert!(!out.contains("Bail out!"), "không còn Bail out: {out}");
}

#[test]
fn check_bao_lenh_la_vs0601() {
    let (code, out) = check("bad", "khong_co \"x\"\n");
    assert_eq!(code, 1, "TAP `not ok`: {out}");
    assert!(out.contains("VS0601"), "TAP: {out}");
}
