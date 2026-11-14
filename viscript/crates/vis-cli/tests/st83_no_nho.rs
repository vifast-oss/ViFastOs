// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-83 (VD-109)** — đóng 3 nợ nhỏ: `fs.exists` ngoài quyền ⇒ `VS0604` (không còn `false` im lặng),
//! `assert <đk>` dạng **câu lệnh** (soft keyword), trợ giúp `vis explain` **theo `--lang`**.
//! Mỗi ca chạy **cả** interp và `--vm --no-cache`, so exit/stdout.

use std::path::{Path, PathBuf};
use std::process::Command;

fn tam(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static DEM: AtomicUsize = AtomicUsize::new(0);
    let n = DEM.fetch_add(1, Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!("vis-st83-{tag}-{}-{}", std::process::id(), n));
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

/// (a) `cap[Dir]` root + `fs.exists` **ngoài** root ⇒ `VS0604` nêu `cap[Dir]` + đường dẫn.
#[test]
fn exists_ngoai_quyen_trap() {
    let d = tam("exists-ngoai");
    let root = d.join("root");
    ghi(&root.join("a.txt"), "x");
    ghi(&d.join("ngoai.txt"), "y");
    let r = root.canonicalize().expect("root");
    let ngoai = d.join("ngoai.txt").canonicalize().expect("ngoai");
    ghi(
        &d.join("f.vis"),
        &format!(
            "use std.fs\n\nfn main(r: cap[Dir] @ p\"{}\"):\n    print(fs.exists(\"{}\"))\n",
            r.display(),
            ngoai.display()
        ),
    );
    let (c, out, err) = ca(&d, &[]);
    assert_eq!(c, 2, "stderr: {err}");
    assert!(err.contains("VS0604"), "{err}");
    assert!(
        err.contains("cap[Dir]") && err.contains("ngoai.txt"),
        "{err}"
    );
    assert_eq!(out, "", "không được in `false` im lặng");
    let _ = std::fs::remove_dir_all(&d);
}

/// (b) Trong root: `exists` trả `true`/`false` đúng (không trap).
#[test]
fn exists_trong_quyen_tra_bool() {
    let d = tam("exists-trong");
    let root = d.join("root");
    ghi(&root.join("a.txt"), "x");
    let r = root.canonicalize().expect("root");
    ghi(
        &d.join("f.vis"),
        &format!(
            "use std.fs\n\nfn main(r: cap[Dir] @ p\"{}\"):\n    print(fs.exists(\"{}/a.txt\"))\n    print(fs.exists(\"{}/khong.txt\"))\n",
            r.display(),
            r.display(),
            r.display()
        ),
    );
    let (c, out, err) = ca(&d, &[]);
    assert_eq!(c, 0, "stderr: {err}");
    assert_eq!(out, "true\nfalse\n");
    let _ = std::fs::remove_dir_all(&d);
}

/// (c) Không cap + `--strict-caps` + `fs.exists` ⇒ `VS0604` “cần khai `cap[Dir]`”.
#[test]
fn exists_strict_khong_cap_trap() {
    let d = tam("exists-strict");
    ghi(&d.join("a.txt"), "x");
    ghi(
        &d.join("f.vis"),
        "use std.fs\n\nfn main():\n    print(fs.exists(\"a.txt\"))\n",
    );
    let (c, out, err) = ca(&d, &["--strict-caps"]);
    assert_eq!(c, 2, "stderr: {err}");
    assert!(err.contains("VS0604"), "{err}");
    assert!(
        err.contains("cần khai `cap[Dir]`") && err.contains("a.txt"),
        "{err}"
    );
    assert_eq!(out, "");
    let _ = std::fs::remove_dir_all(&d);
}

/// (d) `assert <đk>` dạng câu lệnh: đúng ⇒ chạy; sai ⇒ `VS0501` “assert thất bại” (interp = VM).
#[test]
fn assert_cau_lenh_va_sai() {
    let d = tam("assert");
    ghi(
        &d.join("f.vis"),
        "fn main():\n    assert 1 == 1\n    print(\"OK\")\n",
    );
    let (c, out, err) = ca(&d, &[]);
    assert_eq!(c, 0, "stderr: {err}");
    assert_eq!(out, "OK\n");
    ghi(&d.join("f.vis"), "fn main():\n    assert 1 == 2\n");
    let (c, out, err) = ca(&d, &[]);
    assert_eq!(c, 1, "stderr: {err}");
    assert!(
        err.contains("VS0501") && err.contains("assert thất bại"),
        "{err}"
    );
    assert_eq!(out, "");
    let _ = std::fs::remove_dir_all(&d);
}

/// (e) `assert <đk>` trong `test fn` ⇒ TAP `not ok` + `VS0501`, interp = `--vm` cùng byte.
#[test]
fn assert_trong_test_fn_not_ok() {
    let d = tam("assert-test");
    ghi(&d.join("t.vis"), "test fn t_assert():\n    assert 1 == 2\n");
    let (c1, o1, e1) = chay(&d, &["test", "t.vis"]);
    let (c2, o2, _) = chay(&d, &["test", "--vm", "t.vis"]);
    assert_eq!(o1, o2, "TAP lệch: {o1} vs {o2}");
    assert_eq!(c1, c2, "exit lệch");
    assert_eq!(c1, 1, "{e1}");
    assert!(o1.contains("not ok 1 - t_assert"), "{o1}");
    assert!(
        o1.contains("VS0501") && o1.contains("assert thất bại"),
        "{o1}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// (f) Trợ giúp `vis explain` **theo `--lang`** — so **nguyên văn** cả 3 ngôn ngữ.
#[test]
fn explain_help_theo_lang() {
    let ki = |args: &[&str]| -> String {
        let o = Command::new(env!("CARGO_BIN_EXE_vis"))
            .args(args)
            .output()
            .expect("chạy vis");
        assert_eq!(o.status.code(), Some(2));
        let mut t = String::from_utf8_lossy(&o.stdout).to_string();
        t.push_str(&String::from_utf8_lossy(&o.stderr));
        t
    };
    let vi = ki(&["explain"]);
    assert_eq!(
        vi,
        "dùng: vis explain <VSxxxx> [--lang vi|en|ja] | vis explain --list [--lang vi|en|ja]\n\
         sổ mã: 7 khối, 32 mã — `vis explain --list` in toàn bộ (mã + câu ngắn)\n"
    );
    let en = ki(&["explain", "--lang", "en"]);
    assert_eq!(
        en,
        "usage: vis explain <VSxxxx> [--lang vi|en|ja] | vis explain --list [--lang vi|en|ja]\n\
         code book: 7 blocks, 32 codes — `vis explain --list` prints all (code + short text)\n"
    );
    let ja = ki(&["explain", "--lang", "ja"]);
    assert_eq!(
        ja,
        "使い方: vis explain <VSxxxx> [--lang vi|en|ja] | vis explain --list [--lang vi|en|ja]\n\
         コード帳: 7 ブロック、32 コード — `vis explain --list` で全件表示（コード + 短い説明）\n"
    );
}
