// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-93 (VD-111)** — `cap[Proc]` ở biên host: `--strict-caps` + **không** khai `cap[Proc]` ⇒
//! chương trình ngoài bị **`VS0604`**, exit 2; có `cap[Proc]` ⇒ chạy; **mặc định** không đổi.
//! Cả hai đường (interp + VM) dùng **cùng** `Ngoai::Runner` và cùng HIR.

use std::path::{Path, PathBuf};
use std::process::Command;

fn tam(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static DEM: AtomicUsize = AtomicUsize::new(0);
    let d = std::env::temp_dir().join(format!(
        "vis-cap-proc-{tag}-{}-{}",
        std::process::id(),
        DEM.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("tmp");
    d
}

fn ghi(d: &Path, ten: &str, src: &str) -> PathBuf {
    let p = d.join(ten);
    std::fs::write(&p, src).expect("ghi");
    p
}

fn chay(d: &Path, args: &[&str]) -> (i32, Vec<u8>, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_vis"))
        .current_dir(d)
        .args(args)
        .output()
        .expect("chạy vis");
    (
        o.status.code().unwrap_or(-1),
        o.stdout,
        String::from_utf8_lossy(&o.stderr).to_string(),
    )
}

type Kq = (i32, Vec<u8>, String);

/// Chạy interp + VM, trả `(interp, vm)`; helper so byte ở ca cho phép.
fn hai_duong(d: &Path, ten: &str, strict: bool) -> (Kq, Kq) {
    let mut a = vec!["run"];
    let mut b = vec!["run", "--vm", "--no-cache"];
    if strict {
        a.push("--strict-caps");
        b.push("--strict-caps");
    }
    a.push(ten);
    b.push(ten);
    (chay(d, &a), chay(d, &b))
}

/// **SỬA KÈM LÝ DO (ST-93b)**: thân `main` chỉ được **rỗng/`pass`** (câu lệnh thật ⇒ `VS0201`);
/// trước đây dùng `let __cap = 1` để giữ chỗ — nay `pass` mới hợp lệ.
fn ca_main_proc(d: &Path) -> PathBuf {
    ghi(
        d,
        "main.vish",
        "fn main(p: cap[Proc]):\n    pass\n\n/bin/echo a\n",
    )
}

/// (a) strict + **không** cap + `/bin/echo` ⇒ `VS0604` nêu `cap[Proc]`, exit 2.
#[test]
fn strict_khong_cap_thi_chan() {
    let d = tam("a");
    ghi(&d, "a.vish", "/bin/echo a\n");
    let ((ma, out, err), _) = hai_duong(&d, "a.vish", true);
    assert_eq!(ma, 2, "stderr: {err}");
    assert!(out.is_empty(), "không được chạy: {out:?}");
    assert!(err.contains("VS0604"), "{err}");
    assert!(err.contains("cap[Proc]"), "phải nêu cap[Proc]: {err}");
    let _ = std::fs::remove_dir_all(&d);
}

/// (b) strict + `main(p: cap[Proc])` + pipeline cấp tệp ⇒ **chạy**, hai đường **cùng byte**.
#[test]
fn strict_co_cap_proc_thi_chay_hai_duong() {
    let d = tam("b");
    ca_main_proc(&d);
    let (a, b) = hai_duong(&d, "main.vish", true);
    assert_eq!(a.0, 0, "interp stderr: {}", a.2);
    assert_eq!(b.0, 0, "vm stderr: {}", b.2);
    assert_eq!(a.1, b"[a]\n", "stdout interp");
    assert_eq!(a.1, b.1, "hai đường cùng byte");
    let _ = std::fs::remove_dir_all(&d);
}

/// (c) **không** strict + **không** cap ⇒ chạy như cũ (tương thích host suite 54 ca).
#[test]
fn khong_strict_khong_cap_van_chay() {
    let d = tam("c");
    ghi(&d, "c.vish", "/bin/echo a\n");
    let (a, b) = hai_duong(&d, "c.vish", false);
    assert_eq!(a.0, 0, "interp stderr: {}", a.2);
    assert_eq!(b.0, 0, "vm stderr: {}", b.2);
    assert_eq!(a.1, b.1, "hai đường cùng byte");
    assert_eq!(a.1, b"[a]\n");
    let _ = std::fs::remove_dir_all(&d);
}

/// (d) strict + cap[Proc] + `sh -c "exit 3" | complete` ⇒ `code 3`, hai đường.
#[test]
fn strict_cap_proc_sh_exit3() {
    let d = tam("d");
    ghi(
        &d,
        "d.vish",
        "fn main(p: cap[Proc]):\n    pass\n\n/bin/sh -c \"exit 3\" | complete\n",
    );
    let (a, b) = hai_duong(&d, "d.vish", true);
    assert_eq!(a.0, 0, "stderr: {}", a.2);
    assert_eq!(b.0, 0, "stderr: {}", b.2);
    assert_eq!(a.1, b.1);
    let s = String::from_utf8_lossy(&a.1);
    assert!(s.contains("code    3"), "complete: {s:?}");
    let _ = std::fs::remove_dir_all(&d);
}

/// (e) strict + cap[Proc] + pipeline **2 stage** ⇒ chạy, hai đường cùng byte.
#[test]
fn strict_cap_proc_pipeline_hai_stage() {
    let d = tam("e");
    ghi(
        &d,
        "e.vish",
        "fn main(p: cap[Proc]):\n    pass\n\n/bin/echo a | /bin/cat\n",
    );
    let (a, b) = hai_duong(&d, "e.vish", true);
    assert_eq!(a.0, 0, "stderr: {}", a.2);
    assert_eq!(b.0, 0, "stderr: {}", b.2);
    assert_eq!(a.1, b.1);
    assert_eq!(a.1, b"[a]\n");
    let _ = std::fs::remove_dir_all(&d);
}

/// (f) strict + `cap[Dir]` (không Proc) + `/bin/echo` ⇒ `VS0604` nêu **`cap[Proc]`**, không nêu Dir.
#[test]
fn strict_cap_dir_khong_thay_the_proc() {
    let d = tam("f");
    ghi(
        &d,
        "f.vish",
        "fn main(x: cap[Dir] @ p\"./\"):\n    pass\n\n/bin/echo a\n",
    );
    let ((ma, out, err), _) = hai_duong(&d, "f.vish", true);
    assert_eq!(ma, 2, "stderr: {err}");
    assert!(out.is_empty());
    assert!(err.contains("VS0604"), "{err}");
    assert!(err.contains("cap[Proc]"), "{err}");
    assert!(!err.contains("cap[Dir]"), "không được đòi Dir: {err}");
    let _ = std::fs::remove_dir_all(&d);
}
