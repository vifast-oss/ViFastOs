// SPDX-License-Identifier: Apache-2.0
//! **ST-89 (VD-110)** — VM có host chương trình ngoài: hai đường (`run` interp vs `run --vm`) phải
//! **cùng byte** stdout/exit/chẩn đoán trên các dạng lệnh ngoài: `echo`, pipe qua `cat`, `| complete`,
//! `sh -c "exit 3"`, lệnh không tồn tại.

use std::path::{Path, PathBuf};
use std::process::Command;

fn tam(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static DEM: AtomicUsize = AtomicUsize::new(0);
    let n = DEM.fetch_add(1, Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!("vis-ngoai-vm-{tag}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("tmp");
    d
}

fn chay(f: &Path, vm: bool) -> (i32, Vec<u8>, Vec<u8>) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_vis"));
    c.arg("run");
    if vm {
        c.arg("--vm").arg("--no-cache");
    }
    let o = c.arg(f).output().expect("chạy vis");
    (o.status.code().unwrap_or(-1), o.stdout, o.stderr)
}

/// Chạy cả hai đường, so **byte** `(exit, stdout, stderr)`; trả kết quả để ca kiểm tiếp.
fn so_hai(tag: &str, src: &str) -> (i32, Vec<u8>, Vec<u8>) {
    let d = tam(tag);
    let f = d.join("a.vish");
    std::fs::write(&f, src).expect("ghi");
    let a = chay(&f, false);
    let b = chay(&f, true);
    assert_eq!(
        a, b,
        "hai đường phải cùng byte ({tag}):\ninterp {a:?}\nvm {b:?}"
    );
    let _ = std::fs::remove_dir_all(&d);
    a
}

/// (a) `echo` trong `.vish` ⇒ hai đường cùng `a\n`, exit 0.
#[test]
fn echo_vm_cung_byte() {
    let (ma, out, err) = so_hai("echo", "echo \"a\"\n");
    assert_eq!(ma, 0, "{err:?}");
    assert_eq!(out, b"a\n");
}

/// (b) pipe qua `/bin/cat` ⇒ hai đường cùng `[a]\n`.
#[test]
fn cat_pipe_vm_cung_byte() {
    let (ma, out, err) = so_hai("cat", "/bin/echo a | /bin/cat\n");
    assert_eq!(ma, 0, "{err:?}");
    assert_eq!(out, b"[a]\n");
}

/// (c) `| complete` giữ mã thoát ≠ 0 (`false` ⇒ code 1) ở **cả hai** đường.
#[test]
fn complete_ma_thoat_vm_cung_byte() {
    let (ma, out, err) = so_hai("complete", "/usr/bin/false | complete\n");
    assert_eq!(ma, 0, "{err:?}");
    assert!(
        String::from_utf8_lossy(&out).contains("code    1"),
        "{out:?}"
    );
}

/// (d) lệnh không tồn tại ⇒ **cùng** `VS0501` + exit 1.
#[test]
fn lenh_khong_ton_tai_vm_cung_chan_doan() {
    let (ma, out, err) = so_hai("khong", "/khong/co/lenh\n");
    assert_eq!(ma, 1);
    assert!(out.is_empty());
    assert!(String::from_utf8_lossy(&err).contains("VS0501"), "{err:?}");
    assert!(
        String::from_utf8_lossy(&err).contains("không chạy được"),
        "{err:?}"
    );
}

/// (e) `sh -c "exit 3" | complete` ⇒ hai đường cùng `code    3`.
#[test]
fn sh_exit3_vm_cung_byte() {
    let (ma, out, err) = so_hai("sh", "/bin/sh -c \"exit 3\" | complete\n");
    assert_eq!(ma, 0, "{err:?}");
    assert!(
        String::from_utf8_lossy(&out).contains("code    3"),
        "{out:?}"
    );
}
