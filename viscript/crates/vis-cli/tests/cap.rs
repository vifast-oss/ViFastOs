// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-72 (VD-106 (2))** — `vis run --dry` in **quyền khai báo** của `main` rồi exit 0,
//! **không chạy** chương trình. Dữ liệu lấy từ **CST** (`FnDecl main` + `Params`).

use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp_dir(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static DEM: AtomicUsize = AtomicUsize::new(0);
    let n = DEM.fetch_add(1, Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!("vis-cap-{tag}-{}-{}", std::process::id(), n));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("thư mục tạm");
    d
}

fn ghi(p: &Path, s: &str) {
    std::fs::write(p, s).expect("ghi tệp");
}

fn chay(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_vis"))
        .current_dir(dir)
        .args(args)
        .output()
        .expect("chạy `vis`");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).to_string(),
        String::from_utf8_lossy(&o.stderr).to_string(),
    )
}

/// Có quyền ⇒ in đúng thứ tự tham số, `stdout` **rỗng** (không chạy `print`).
#[test]
fn dry_in_danh_sach() {
    let d = tmp_dir("danh-sach");
    ghi(
        &d.join("f.vis"),
        "fn main(data: cap[Dir] @ p\"./data\", out: cap[Dir] @ p\"./out\" (write), net: cap[Net] @ \"api.example.com\"):\n    print(9)\n",
    );
    let (code, out, err) = chay(&d, &["run", "--dry", "f.vis"]);
    assert_eq!(code, 0, "stderr: {err}");
    assert_eq!(out, "", "`--dry` không được chạy `print`");
    assert_eq!(
        err,
        "data: cap[Dir] ./data\nout: cap[Dir] ./out (write)\nnet: cap[Net] api.example.com\n"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// Không quyền ⇒ in `(không khai báo quyền)`, exit 0.
#[test]
fn dry_khong_quyen() {
    let d = tmp_dir("khong-quyen");
    ghi(&d.join("f.vis"), "fn main():\n    print(1)\n");
    let (code, out, err) = chay(&d, &["run", "--dry", "f.vis"]);
    assert_eq!(code, 0, "stderr: {err}");
    assert_eq!(out, "");
    assert_eq!(err, "(không khai báo quyền)\n");
    let _ = std::fs::remove_dir_all(&d);
}

/// `--dry` **không chạy** chương trình: cùng tệp, chạy thật in `KHONG`, còn `--dry` thì stdout rỗng.
#[test]
fn dry_khong_chay() {
    let d = tmp_dir("khong-chay");
    ghi(&d.join("f.vis"), "fn main():\n    print(\"KHONG\")\n");
    let (c0, o0, e0) = chay(&d, &["run", "f.vis"]);
    assert_eq!(c0, 0, "stderr: {e0}");
    assert_eq!(o0, "KHONG\n");
    let (c1, o1, e1) = chay(&d, &["run", "--dry", "f.vis"]);
    assert_eq!(c1, 0, "stderr: {e1}");
    assert_eq!(o1, "", "`--dry` phải **không chạy** chương trình");
    assert_eq!(e1, "(không khai báo quyền)\n");
    let _ = std::fs::remove_dir_all(&d);
}

/// `cap` **không in được** (bất biến VD-106; `--dry` giữ 3 ca riêng ở trên).
#[test]
fn cap_khong_in_duoc() {
    let d = tmp_dir("khong-in");
    ghi(
        &d.join("f.vis"),
        "fn f(d: cap[Dir]) -> bool:\n    print(d)\n\nfn main():\n    print(0)\n",
    );
    let (code, out, err) = chay(&d, &["check", "f.vis"]);
    assert_eq!(code, 1, "stderr: {err}");
    assert!(out.contains("VS0412"), "stdout: {out}");
    assert!(out.contains("không in được"), "stdout: {out}");
    let _ = std::fs::remove_dir_all(&d);
}
