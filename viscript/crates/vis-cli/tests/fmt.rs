// SPDX-License-Identifier: MIT OR Apache-2.0
//! `vis fmt` — giao diện CLI của ST-07 (đỏ trước, D2-05).
//!
//! Chốt: `vis fmt <một-tệp>` ⇒ stdout, exit 0; `--write` ⇒ ghi tại chỗ + `đã sửa|giữ nguyên`;
//! `--check` ⇒ exit 0/1 (in đường dẫn cần sửa)/2 (tệp lỗi cú pháp); `.vish` ⇒ `Bail out!` + 2
//! (ST-06 đã có CmdCST nhưng **chưa có đặc tả định dạng** chế độ lệnh — vẫn từ chối).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test/fmt")
}

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_vis")
}

fn tmp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vis-fmt-cli-{}-{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("tạo thư mục tạm");
    d
}

/// Chép fixture ra thư mục tạm (để `--write` không sửa tệp tĩnh trong repo).
fn copy_fixture(name: &str, dir: &Path) -> PathBuf {
    let src = fixture_dir().join(name);
    let dst = dir.join(name);
    std::fs::copy(&src, &dst).unwrap_or_else(|e| panic!("chép {}: {e}", src.display()));
    dst
}

fn run(args: &[String]) -> Output {
    Command::new(bin())
        .args(args)
        .output()
        .expect("phải chạy được binary `vis`")
}

fn run_strs(args: &[&str]) -> Output {
    let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    run(&owned)
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn s(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

#[test]
fn mot_tep_in_ra_stdout_giu_nguyen_tep() {
    let dir = tmp_dir("stdout");
    let file = copy_fixture("chuan.vis", &dir);
    let before = std::fs::read_to_string(&file).expect("đọc fixture");
    let o = run_strs(&["fmt", &s(&file)]);
    assert_eq!(o.status.code(), Some(0), "tệp sạch ⇒ exit 0");
    assert_eq!(
        stdout(&o),
        before,
        "(a) tệp đã đúng chuẩn ⇒ in đúng nguyên văn"
    );
    assert_eq!(
        std::fs::read_to_string(&file).expect("đọc lại"),
        before,
        "chế độ stdout **không** được ghi tệp"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_sua_roi_giu_nguyen() {
    let dir = tmp_dir("write");
    let file = copy_fixture("sai_thut_le.vis", &dir);
    let o = run_strs(&["fmt", "--write", &s(&file)]);
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(stdout(&o), format!("đã sửa {}\n", s(&file)));
    let after = std::fs::read_to_string(&file).expect("đọc sau khi sửa");
    assert!(
        after.contains("\n    let x = a + 1\n"),
        "phải thụt 4 dấu cách:\n{after}"
    );
    assert!(!after.contains("\n  let x"), "không còn thụt 2:\n{after}");

    // Chạy lại ⇒ điểm bất động ⇒ `giữ nguyên`.
    let o2 = run_strs(&["fmt", "--write", &s(&file)]);
    assert_eq!(o2.status.code(), Some(0));
    assert_eq!(stdout(&o2), format!("giữ nguyên {}\n", s(&file)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn check_ma_thoat_0_1_2() {
    let dir = tmp_dir("check");
    let good = copy_fixture("chuan.vis", &dir);
    let bad = copy_fixture("sai_thut_le.vis", &dir);
    let err = copy_fixture("loi_cu_phap.vis", &dir);

    let o = run_strs(&["fmt", "--check", &s(&good)]);
    assert_eq!(o.status.code(), Some(0), "đã đúng chuẩn ⇒ 0");

    let o = run_strs(&["fmt", "--check", &s(&bad)]);
    assert_eq!(o.status.code(), Some(1), "cần sửa ⇒ 1");
    assert!(
        stdout(&o).contains(&s(&bad)),
        "phải in đường dẫn cần sửa: {:?}",
        stdout(&o)
    );

    let o = run_strs(&["fmt", "--check", &s(&err)]);
    assert_eq!(o.status.code(), Some(2), "tệp lỗi cú pháp ⇒ 2");
    assert!(
        stderr(&o).contains("# VS"),
        "stderr phải nêu lỗi đầu: {}",
        stderr(&o)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn check_nhieu_tep_va_tep_loi() {
    let dir = tmp_dir("check-multi");
    let good = copy_fixture("chuan.vis", &dir);
    let bad = copy_fixture("tab.vis", &dir);
    let o = run_strs(&["fmt", "--check", &s(&good), &s(&bad)]);
    assert_eq!(o.status.code(), Some(1));
    let out = stdout(&o);
    assert!(out.contains(&s(&bad)), "phải in tệp cần sửa: {out}");
    assert!(!out.contains(&s(&good)), "không in tệp đã đúng: {out}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn loi_cu_phap_ra_stderr_exit_2() {
    let dir = tmp_dir("err");
    let err = copy_fixture("loi_cu_phap.vis", &dir);
    let o = run_strs(&["fmt", &s(&err)]);
    assert_eq!(o.status.code(), Some(2));
    assert!(stderr(&o).contains("# VS0201"), "stderr: {}", stderr(&o));
    assert_eq!(stdout(&o), "", "không in gì khi tệp lỗi");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_khong_ghi_tep_loi() {
    let dir = tmp_dir("err-write");
    let err = copy_fixture("loi_cu_phap.vis", &dir);
    let before = std::fs::read_to_string(&err).expect("đọc fixture");
    let o = run_strs(&["fmt", "--write", &s(&err)]);
    assert_eq!(o.status.code(), Some(2));
    assert!(
        stderr(&o).contains("# VS0201"),
        "lỗi cú pháp phải được nêu (không phải usage): {}",
        stderr(&o)
    );
    assert_eq!(
        std::fs::read_to_string(&err).expect("đọc lại"),
        before,
        "(d) `--write` **không** được ghi tệp lỗi"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn vish_bail_out_exit_2() {
    let dir = tmp_dir("vish");
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test/check/cmd_mode.vish");
    let dst = dir.join("cmd_mode.vish");
    std::fs::copy(&src, &dst).expect("chép .vish");
    let o = run_strs(&["fmt", &s(&dst)]);
    assert_eq!(o.status.code(), Some(2));
    let out = stdout(&o);
    assert!(
        out.contains("Bail out! định dạng chế độ lệnh (.vish) chưa có đặc tả — chưa hỗ trợ"),
        "thiếu `Bail out!`: {out}"
    );
    assert!(
        !out.contains("đã sửa"),
        "bail out không được coi là đã sửa: {out}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn thieu_tham_so_va_co_la_la_usage() {
    let o = run_strs(&["fmt"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(
        stderr(&o).contains("dùng: vis fmt"),
        "stderr: {}",
        stderr(&o)
    );
    assert_eq!(stdout(&o), "", "usage không in stdout");

    let o = run_strs(&["fmt", "--json"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(
        stderr(&o).contains("dùng: vis fmt"),
        "stderr: {}",
        stderr(&o)
    );
}

#[test]
fn khong_panic_khi_pipe_dong() {
    // `vis fmt tệp | head` đóng stdout sớm: lần ghi sau của `vis` gặp `BrokenPipe` — phải **im
    // lặng thoát 0**, không panic (`failed printing to stdout`).
    let dir = tmp_dir("pipe");
    let mut big = String::new();
    for i in 0..20_000 {
        big.push_str(&format!("let x{i} = {i}\n"));
    }
    let file = dir.join("big.vis");
    std::fs::write(&file, &big).expect("ghi tệp lớn");
    let mut child = Command::new(bin())
        .args(["fmt", &s(&file)])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn vis");
    drop(child.stdout.take()); // đóng đầu đọc ngay
    let out = child.wait_with_output().expect("chờ vis");
    assert_eq!(
        out.status.code(),
        Some(0),
        "BrokenPipe phải thoát 0 im lặng; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !String::from_utf8_lossy(&out.stderr).contains("panic"),
        "không được panic: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);
}
