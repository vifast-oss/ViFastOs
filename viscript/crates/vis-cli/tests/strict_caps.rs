// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-81 (VD-108)** — `--strict-caps`: mọi truy cập `std.fs`/`net` phải nằm trong `cap` đã khai.
//! **Mặc định không đổi** (không cờ ⇒ không siết). Mỗi ca chạy **cả** interp và `--vm --no-cache`,
//! so **cùng** stdout/exit/mã.

use std::path::{Path, PathBuf};
use std::process::Command;

fn tam(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static DEM: AtomicUsize = AtomicUsize::new(0);
    let n = DEM.fetch_add(1, Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!("vis-strict-{tag}-{}-{}", std::process::id(), n));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("tmp");
    d
}

fn ghi(p: &Path, s: &str) {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).expect("mkdir");
    }
    std::fs::write(p, s).expect("ghi");
}

fn chay(d: &Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_vis"))
        .current_dir(d)
        .args(args)
        .output()
        .expect("chạy `vis`");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).to_string(),
        String::from_utf8_lossy(&o.stderr).to_string(),
    )
}

/// Chạy interp và `--vm --no-cache` với cùng cờ; khẳng định cùng exit/stdout; trả interp.
fn ca(d: &Path, them: &[&str]) -> (i32, String, String) {
    let mut a = vec!["run"];
    a.extend_from_slice(them);
    a.push("f.vis");
    let mut b = vec!["run", "--vm", "--no-cache"];
    b.extend_from_slice(them);
    b.push("f.vis");
    let r1 = chay(d, &a);
    let r2 = chay(d, &b);
    assert_eq!(r1.0, r2.0, "exit lệch: {r1:?} vs {r2:?}");
    assert_eq!(r1.1, r2.1, "stdout lệch: {r1:?} vs {r2:?}");
    r1
}

const DOC: &str = "use std.fs\n\nfn main():\n    match fs.read(\"in.txt\"):\n        ok(s):\n            print(s)\n        err(e):\n            print(\"ERR\")\n";

/// (a) **Không** cờ, **không** `cap` ⇒ chạy (khoá tương thích mặc định).
#[test]
fn mac_dinh_khong_siet() {
    let d = tam("mac-dinh");
    ghi(&d.join("in.txt"), "xin chào\n");
    ghi(&d.join("f.vis"), DOC);
    let (c, out, err) = ca(&d, &[]);
    assert_eq!(c, 0, "stderr: {err}");
    assert_eq!(out, "xin chào\n\n", "fs.read giữ newline + print thêm một");
    let _ = std::fs::remove_dir_all(&d);
}

/// (b) `--strict-caps`, **không** `cap` ⇒ `VS0604` nêu **cần khai `cap[Dir]`** + đường dẫn, exit 2.
#[test]
fn strict_khong_cap_doc_thi_vs0604() {
    let d = tam("strict-doc");
    ghi(&d.join("in.txt"), "xin chào\n");
    ghi(&d.join("f.vis"), DOC);
    let (c, out, err) = ca(&d, &["--strict-caps"]);
    assert_eq!(c, 2, "stderr: {err}");
    assert!(err.contains("VS0604"), "stderr: {err}");
    assert!(err.contains("cần khai `cap[Dir]`"), "stderr: {err}");
    assert!(err.contains("in.txt"), "phải nêu đường dẫn: {err}");
    assert_eq!(out, "", "không được in kết quả");
    let _ = std::fs::remove_dir_all(&d);
}

/// (c) `cap[Dir]` đúng root + đọc **trong** root + `--strict-caps` ⇒ chạy.
#[test]
fn strict_cap_dir_trong_root() {
    let d = tam("cap-trong");
    let root = d.join("root");
    ghi(&root.join("a.txt"), "trong\n");
    let r = root.canonicalize().expect("canonical root");
    ghi(
        &d.join("f.vis"),
        &format!(
            "use std.fs\n\nfn main(d: cap[Dir] @ p\"{}\"):\n    match fs.read(\"{}/a.txt\"):\n        ok(s):\n            print(s)\n        err(e):\n            print(\"ERR\")\n",
            r.display(),
            r.display()
        ),
    );
    let (c, out, err) = ca(&d, &["--strict-caps"]);
    assert_eq!(c, 0, "stderr: {err}");
    assert_eq!(out, "trong\n\n");
    let _ = std::fs::remove_dir_all(&d);
}

/// (d) `cap[Dir]` root + đọc **ngoài** root + `--strict-caps` ⇒ `VS0604`.
#[test]
fn strict_cap_dir_ngoai_root() {
    let d = tam("cap-ngoai");
    let root = d.join("root");
    ghi(&root.join("a.txt"), "trong\n");
    ghi(&d.join("ngoai.txt"), "ngoai\n");
    let r = root.canonicalize().expect("canonical root");
    ghi(
        &d.join("f.vis"),
        &format!(
            "use std.fs\n\nfn main(d: cap[Dir] @ p\"{}\"):\n    match fs.read(\"{}/../ngoai.txt\"):\n        ok(s):\n            print(s)\n        err(e):\n            print(\"ERR\")\n",
            r.display(),
            r.display()
        ),
    );
    let (c, out, err) = ca(&d, &["--strict-caps"]);
    assert_eq!(c, 2, "stderr: {err}");
    assert!(err.contains("VS0604") && err.contains("cap[Dir]"), "{err}");
    assert_eq!(out, "", "không được in kết quả");
    let _ = std::fs::remove_dir_all(&d);
}

/// (e) `--strict-caps`, không cap, `net.connect` loopback ⇒ `VS0604` **cần `cap[Net]`**.
#[test]
fn strict_khong_cap_net_thi_vs0604() {
    let d = tam("strict-net");
    ghi(
        &d.join("f.vis"),
        "use std.net\n\nfn main():\n    match net.connect(\"127.0.0.1:9\"):\n        ok(_):\n            print(\"OK\")\n        err(e):\n            print(\"ERR\")\n",
    );
    let (c, out, err) = ca(&d, &["--strict-caps"]);
    assert_eq!(c, 2, "stderr: {err}");
    assert!(err.contains("VS0604"), "{err}");
    assert!(err.contains("cần khai `cap[Net]`"), "{err}");
    assert_eq!(out, "");
    let _ = std::fs::remove_dir_all(&d);
}

/// (f) `--strict-caps` + `--fs-mock` ⇒ **vẫn** kiểm quyền trên đường mock (khoá lựa chọn).
#[test]
fn strict_van_kiem_tren_fs_mock() {
    let d = tam("strict-mock");
    ghi(
        &d.join("mock.json"),
        r#"{ "files": { "a.txt": "noi dung" } }"#,
    );
    ghi(
        &d.join("f.vis"),
        "use std.fs\n\nfn main():\n    match fs.read(\"a.txt\"):\n        ok(s):\n            print(s)\n        err(e):\n            print(\"ERR\")\n",
    );
    let (c, out, err) = ca(&d, &["--strict-caps", "--fs-mock", "mock.json"]);
    assert_eq!(c, 2, "stderr: {err}");
    assert!(err.contains("VS0604") && err.contains("cap[Dir]"), "{err}");
    assert_eq!(out, "", "mock **không** được miễn kiểm");
    let _ = std::fs::remove_dir_all(&d);
}

/// (g) `vis test --strict-caps` — truyền cờ xuống `vis run`, TAP `not ok` + `VS0604`.
#[test]
fn vis_test_strict_caps_truyen_co() {
    let d = tam("test-strict");
    ghi(&d.join("in.txt"), "xin chào\n");
    ghi(
        &d.join("t.vis"),
        "use std.fs\n\ntest fn t_doc():\n    match fs.read(\"in.txt\"):\n        ok(s):\n            print(s)\n        err(e):\n            print(\"ERR\")\n",
    );
    let (c1, o1, e1) = chay(&d, &["test", "--strict-caps", "t.vis"]);
    let (c2, o2, e2) = chay(&d, &["test", "--strict-caps", "--vm", "t.vis"]);
    assert_eq!(o1, o2, "TAP lệch interp/VM: {o1} vs {o2}");
    assert_eq!(c1, c2, "exit lệch: {e1} / {e2}");
    assert_eq!(c1, 1, "{e1}");
    assert!(o1.contains("not ok 1 - t_doc"), "{o1}");
    assert!(o1.contains("VS0604"), "{o1}");
    let _ = std::fs::remove_dir_all(&d);
}
