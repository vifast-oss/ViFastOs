// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-72b (VD-103/VD-106)** — cưỡng chế quyền ở biên host: `cap[Dir]`/`cap[Net]` + `--grant`.
//!
//! Mỗi ca chạy **cả** interp và `--vm --no-cache`, so **cùng** byte (stdout + exit + mã lỗi).
//! Thư mục tạm **duy nhất** mỗi lần gọi (test chạy song song).

use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp_dir(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static DEM: AtomicUsize = AtomicUsize::new(0);
    let n = DEM.fetch_add(1, Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!("vis-q-{tag}-{}-{}", std::process::id(), n));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("thư mục tạm");
    d
}

fn ghi(p: &Path, s: &str) {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).expect("mkdir");
    }
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

/// Chạy cả hai đường, trả `(interp, vm)`; so **cùng** stdout/exit/mã.
fn ca(dir: &Path, them: &[&str]) -> ((i32, String, String), (i32, String, String)) {
    let mut a = vec!["run"];
    a.extend_from_slice(them);
    a.push("f.vis");
    let mut b = vec!["run", "--vm", "--no-cache"];
    b.extend_from_slice(them);
    b.push("f.vis");
    let r1 = chay(dir, &a);
    let r2 = chay(dir, &b);
    assert_eq!(r1.0, r2.0, "exit lệch: {r1:?} vs {r2:?}");
    assert_eq!(r1.1, r2.1, "stdout lệch: {r1:?} vs {r2:?}");
    (r1, r2)
}

/// Ca 1 — đọc tệp **trong** root ⇒ `ok`.
#[test]
fn doc_trong_quyen() {
    let d = tmp_dir("trong");
    let root = d.join("root");
    ghi(&root.join("a.txt"), "xin chào");
    ghi(
        &d.join("f.vis"),
        &format!(
            "use std.fs\n\nfn main(r: cap[Dir] @ p\"{}\"):\n    match fs.read(\"{}/a.txt\"):\n        ok(s):\n            print(s)\n        err(e):\n            print(\"ERR\")\n",
            root.display(),
            root.display()
        ),
    );
    let (r1, _r2) = ca(&d, &[]);
    assert_eq!(r1.0, 0, "stderr: {}", r1.2);
    assert_eq!(r1.1, "xin chào\n");
    let _ = std::fs::remove_dir_all(&d);
}

/// Ca 2 — đọc **ngoài** root ⇒ `VS0604` nêu `cap[Dir]`, exit 2.
#[test]
fn doc_ngoai_quyen() {
    let d = tmp_dir("ngoai");
    let root = d.join("root");
    ghi(&root.join("a.txt"), "trong");
    ghi(&d.join("ngoai.txt"), "ngoai");
    ghi(
        &d.join("f.vis"),
        &format!(
            "use std.fs\n\nfn main(r: cap[Dir] @ p\"{}\"):\n    match fs.read(\"{}/../ngoai.txt\"):\n        ok(s):\n            print(s)\n        err(e):\n            print(\"ERR\")\n",
            root.display(),
            root.display()
        ),
    );
    let (r1, _r2) = ca(&d, &[]);
    assert_eq!(r1.0, 2, "stderr: {}", r1.2);
    assert!(r1.2.contains("VS0604"), "stderr: {}", r1.2);
    assert!(r1.2.contains("cap[Dir]"), "stderr: {}", r1.2);
    assert_eq!(r1.1, "", "không được in kết quả");
    let _ = std::fs::remove_dir_all(&d);
}

/// Ca 3 — `cap[Dir]` **không** `(write)` + `fs.write` ⇒ `VS0604` nêu `(write)`.
#[test]
fn ghi_thieu_co() {
    let d = tmp_dir("thieu-write");
    let root = d.join("root");
    std::fs::create_dir_all(&root).expect("root");
    ghi(
        &d.join("f.vis"),
        &format!(
            "use std.fs\n\nfn main(r: cap[Dir] @ p\"{}\"):\n    match fs.write(\"{}/b.txt\", \"x\"):\n        ok(_):\n            print(\"GHI\")\n        err(e):\n            print(\"ERR\")\n",
            root.display(),
            root.display()
        ),
    );
    let (r1, _r2) = ca(&d, &[]);
    assert_eq!(r1.0, 2, "stderr: {}", r1.2);
    assert!(r1.2.contains("VS0604"), "stderr: {}", r1.2);
    assert!(r1.2.contains("(write)"), "stderr: {}", r1.2);
    let _ = std::fs::remove_dir_all(&d);
}

/// Ca 4 — có `(write)` ⇒ ghi được, đọc lại đúng.
#[test]
fn ghi_co_co() {
    let d = tmp_dir("co-write");
    let root = d.join("root");
    std::fs::create_dir_all(&root).expect("root");
    ghi(
        &d.join("f.vis"),
        &format!(
            "use std.fs\n\nfn main(r: cap[Dir] @ p\"{}\" (write)):\n    match fs.write(\"{}/b.txt\", \"noi dung\"):\n        ok(_):\n            match fs.read(\"{}/b.txt\"):\n                ok(s):\n                    print(s)\n                err(e):\n                    print(\"ERR-DOC\")\n        err(e):\n            print(\"ERR-GHI\")\n",
            root.display(),
            root.display(),
            root.display()
        ),
    );
    let (r1, _r2) = ca(&d, &[]);
    assert_eq!(r1.0, 0, "stderr: {}", r1.2);
    assert_eq!(r1.1, "noi dung\n");
    let _ = std::fs::remove_dir_all(&d);
}

/// Ca 5 — **không khai `cap`** ⇒ **không hạn chế** (tương thích; nợ "siết mặc định").
#[test]
fn khong_khai_cap_thi_khong_han_che() {
    let d = tmp_dir("khong-cap");
    let ngoai = d.join("ngoai.txt");
    ghi(&ngoai, "tu do");
    ghi(
        &d.join("f.vis"),
        &format!(
            "use std.fs\n\nfn main():\n    match fs.read(\"{}\"):\n        ok(s):\n            print(s)\n        err(e):\n            print(\"ERR\")\n",
            ngoai.display()
        ),
    );
    let (r1, _r2) = ca(&d, &[]);
    assert_eq!(r1.0, 0, "stderr: {}", r1.2);
    assert_eq!(r1.1, "tu do\n");
    let _ = std::fs::remove_dir_all(&d);
}

/// Ca 6 — `--grant d=<tmp_b>` đổi **root** của cap `d` (không thêm quyền mới).
#[test]
fn grant_doi_duong_dan() {
    let d = tmp_dir("grant");
    let a = d.join("a");
    let b = d.join("b");
    ghi(&a.join("a.txt"), "A");
    ghi(&b.join("b.txt"), "B");
    ghi(&d.join("ngoai.txt"), "NGOAI");
    ghi(
        &d.join("f.vis"),
        &format!(
            "use std.fs\n\nfn main(d: cap[Dir] @ p\"{}\"):\n    match fs.read(\"{}/b.txt\"):\n        ok(s):\n            print(s)\n        err(e):\n            print(\"ERR\")\n    match fs.read(\"{}/a.txt\"):\n        ok(s):\n            print(s)\n        err(e):\n            print(\"ERR\")\n",
            a.display(),
            b.display(),
            a.display()
        ),
    );
    let grant = format!("d={}", b.display());
    let (r1, _r2) = ca(&d, &["--grant", &grant]);
    assert_eq!(r1.0, 2, "stderr: {}", r1.2);
    assert!(r1.2.contains("VS0604"), "stderr: {}", r1.2);
    assert_eq!(r1.1, "B\n", "chỉ đọc được root mới trước khi bị chặn");
    let _ = std::fs::remove_dir_all(&d);
}

/// Ca 7 — chỉ `cap[Dir]` + `net.connect` ⇒ `VS0604` "thiếu `cap[Net]`".
#[test]
fn net_thieu_cap() {
    let d = tmp_dir("net");
    let root = d.join("root");
    std::fs::create_dir_all(&root).expect("root");
    ghi(
        &d.join("f.vis"),
        &format!(
            "use std.net\n\nfn main(r: cap[Dir] @ p\"{}\"):\n    match net.connect(\"127.0.0.1:1\"):\n        ok(h):\n            print(\"NOI\")\n        err(e):\n            print(\"ERR\")\n",
            root.display()
        ),
    );
    let (r1, _r2) = ca(&d, &[]);
    assert_eq!(r1.0, 2, "stderr: {}", r1.2);
    assert!(r1.2.contains("VS0604"), "stderr: {}", r1.2);
    assert!(r1.2.contains("cap[Net]"), "stderr: {}", r1.2);
    let _ = std::fs::remove_dir_all(&d);
}

/// Ca 8 — hai `cap[Dir]` khác root: đọc trong **mỗi** root đều được; đọc **ngoài cả hai** ⇒ `VS0604`.
#[test]
fn hai_cap_dir() {
    let d = tmp_dir("hai-root");
    let a = d.join("a");
    let b = d.join("b");
    ghi(&a.join("a.txt"), "A");
    ghi(&b.join("b.txt"), "B");
    ghi(&d.join("ngoai.txt"), "NGOAI");
    ghi(
        &d.join("f.vis"),
        &format!(
            "use std.fs\n\nfn main(a: cap[Dir] @ p\"{}\", b: cap[Dir] @ p\"{}\"):\n    match fs.read(\"{}/a.txt\"):\n        ok(s):\n            print(s)\n        err(e):\n            print(\"ERR-A\")\n    match fs.read(\"{}/b.txt\"):\n        ok(s):\n            print(s)\n        err(e):\n            print(\"ERR-B\")\n    match fs.read(\"{}/ngoai.txt\"):\n        ok(s):\n            print(s)\n        err(e):\n            print(\"ERR-NGOAI\")\n",
            a.display(),
            b.display(),
            a.display(),
            b.display(),
            d.display()
        ),
    );
    let (r1, _r2) = ca(&d, &[]);
    assert_eq!(r1.0, 2, "stderr: {}", r1.2);
    assert!(r1.2.contains("VS0604"), "stderr: {}", r1.2);
    assert_eq!(
        r1.1, "A\nB\n",
        "đọc ngoài **cả hai** root bị chặn sau hai lần đọc hợp lệ"
    );
    let _ = std::fs::remove_dir_all(&d);
}
