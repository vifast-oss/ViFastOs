// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-93b (VD-111)** — `.vish`: `fn main` **chỉ** để khai `cap`; thân có câu lệnh ⇒ **`VS0201`**
//! tại câu lệnh đầu, exit 2, **không chạy** (trước đây im lặng: pipeline bị bỏ hoặc thân main chạy).
//! Thân rỗng/chỉ `pass` + pipeline cấp tệp ⇒ hợp lệ, chạy bình thường; `.vis` không bị ảnh hưởng.

use std::path::{Path, PathBuf};
use std::process::Command;

type Kq = (i32, Vec<u8>, String);

fn tam(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static DEM: AtomicUsize = AtomicUsize::new(0);
    let d = std::env::temp_dir().join(format!(
        "vis-vish-main-{tag}-{}-{}",
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

fn chay(d: &Path, args: &[&str]) -> Kq {
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

/// Chạy interp + `--vm --no-cache` cùng tệp.
fn hai_duong(d: &Path, ten: &str) -> (Kq, Kq) {
    (
        chay(d, &["run", ten]),
        chay(d, &["run", "--vm", "--no-cache", ten]),
    )
}

/// (a) thân `main` **rỗng** (`pass`) + pipeline ⇒ hợp lệ, chạy `[a]`, hai đường cùng byte.
#[test]
fn main_rong_va_pipeline_hop_le() {
    let d = tam("a");
    ghi(
        &d,
        "a.vish",
        "fn main(p: cap[Proc]):\n    pass\n\n/bin/echo a\n",
    );
    let (a, b) = hai_duong(&d, "a.vish");
    assert_eq!(a.0, 0, "interp stderr: {}", a.2);
    assert_eq!(b.0, 0, "vm stderr: {}", b.2);
    assert_eq!(a.1, b"[a]\n");
    assert_eq!(a.1, b.1, "hai đường cùng byte");
    let _ = std::fs::remove_dir_all(&d);
}

/// (b) `main` có `print` + pipeline ⇒ **VS0201** dòng 2, exit 2, **không** in gì.
#[test]
fn main_co_lenh_bao_vs0201() {
    let d = tam("b");
    ghi(
        &d,
        "b.vish",
        "fn main():\n    print(\"x\")\n\n/bin/echo a\n",
    );
    let (a, b) = hai_duong(&d, "b.vish");
    assert_eq!(a.0, 2, "stderr: {}", a.2);
    assert_eq!(b.0, 2, "stderr: {}", b.2);
    assert!(a.1.is_empty(), "interp không được chạy: {:?}", a.1);
    assert!(b.1.is_empty(), "vm không được chạy: {:?}", b.1);
    for e in [&a.2, &b.2] {
        assert!(e.contains(":2:5: error VS0201"), "{e}");
        assert!(e.contains("chỉ dùng để khai `cap`"), "{e}");
        assert!(e.contains("chuyển câu lệnh ra cấp tệp"), "{e}");
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// (c) `main` có `let` + pipeline ⇒ VS0201, dòng câu lệnh đầu.
#[test]
fn main_co_let_bao_vs0201() {
    let d = tam("c");
    ghi(&d, "c.vish", "fn main():\n    let x = 1\n\n/bin/echo a\n");
    let (a, b) = hai_duong(&d, "c.vish");
    assert_eq!(a.0, 2, "stderr: {}", a.2);
    assert_eq!(b.0, 2, "stderr: {}", b.2);
    assert!(a.2.contains(":2:5: error VS0201"), "{}", a.2);
    assert!(b.2.contains(":2:5: error VS0201"), "{}", b.2);
    let _ = std::fs::remove_dir_all(&d);
}

/// (d) `.vis` có `fn main` bình thường ⇒ **không** bị ảnh hưởng, hai đường in `x`.
#[test]
fn vis_main_binh_thuong_khong_doi() {
    let d = tam("d");
    ghi(&d, "d.vis", "fn main():\n    print(\"x\")\n");
    let (a, b) = hai_duong(&d, "d.vis");
    assert_eq!(a.0, 0, "interp stderr: {}", a.2);
    assert_eq!(b.0, 0, "vm stderr: {}", b.2);
    assert_eq!(a.1, b"x\n");
    assert_eq!(a.1, b.1, "hai đường cùng byte");
    let _ = std::fs::remove_dir_all(&d);
}

/// (e) `.vish` `main` có `assert` + pipeline ⇒ VS0201.
#[test]
fn main_co_assert_bao_vs0201() {
    let d = tam("e");
    ghi(
        &d,
        "e.vish",
        "fn main():\n    assert 1 == 1\n\n/bin/echo a\n",
    );
    let (a, b) = hai_duong(&d, "e.vish");
    assert_eq!(a.0, 2, "stderr: {}", a.2);
    assert_eq!(b.0, 2, "stderr: {}", b.2);
    assert!(a.2.contains(":2:5: error VS0201"), "{}", a.2);
    assert!(b.2.contains(":2:5: error VS0201"), "{}", b.2);
    let _ = std::fs::remove_dir_all(&d);
}
