// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-97 (VD-112)** — `--grant` cho `cap[Proc]` = **allowlist lệnh**:
//! lặp `--grant p=<lệnh>`; khớp **basename** hoặc **đường dẫn canonicalize**; không grant ⇒ không giới
//! hạn (tương thích). Grant Proc gán cho `cap[Dir]` ⇒ lỗi kiểu rõ. Mọi ca chạy **cả hai đường**, so byte.

use std::path::{Path, PathBuf};
use std::process::Command;

type Kq = (i32, Vec<u8>, String);

fn tam(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static DEM: AtomicUsize = AtomicUsize::new(0);
    let d = std::env::temp_dir().join(format!(
        "vis-procgrant-{tag}-{}-{}",
        std::process::id(),
        DEM.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("tmp");
    d
}

fn chay(d: &Path, ten: &str, args: &[&str], vm: bool) -> Kq {
    let mut c = Command::new(env!("CARGO_BIN_EXE_vis"));
    c.current_dir(d).arg("run").arg("--no-cache");
    if vm {
        c.arg("--vm");
    }
    c.args(args).arg(ten);
    let o = c.output().expect("chạy vis");
    (
        o.status.code().unwrap_or(-1),
        o.stdout,
        String::from_utf8_lossy(&o.stderr).to_string(),
    )
}

/// Ghi tệp + chạy hai đường, đòi **byte y hệt**.
fn ca(d: &Path, ten: &str, src: &str, args: &[&str]) -> Kq {
    std::fs::write(d.join(ten), src).expect("ghi");
    let a = chay(d, ten, args, false);
    let b = chay(d, ten, args, true);
    assert_eq!(a, b, "hai đường lệch ({ten}):\n interp {a:?}\n vm {b:?}");
    a
}

fn vish(cmd: &str) -> String {
    format!(
        "# SPDX-License-Identifier: MIT OR Apache-2.0\nfn main(p: cap[Proc]):\n    pass\n\n{cmd}\n"
    )
}

/// (a) grant `/bin/echo` + chạy `/bin/echo` ⇒ chạy.
#[test]
fn grant_tuyet_doi_echo_chay() {
    let d = tam("a");
    let r = ca(
        &d,
        "a.vish",
        &vish("/bin/echo a"),
        &["--strict-caps", "--grant", "p=/bin/echo"],
    );
    assert_eq!(r.0, 0, "{r:?}");
    assert_eq!(r.1, b"[a]\n");
    let _ = std::fs::remove_dir_all(&d);
}

/// (b) grant `/bin/echo` + chạy `/usr/bin/grep` ⇒ `VS0604` allowlist, exit 2.
#[test]
fn grant_echo_chan_grep() {
    let d = tam("b");
    let r = ca(
        &d,
        "b.vish",
        &vish("/usr/bin/grep -E \"^\" /etc/hosts"),
        &["--strict-caps", "--grant", "p=/bin/echo"],
    );
    assert_eq!(r.0, 2, "{r:?}");
    let e = &r.2;
    assert!(e.contains("VS0604"), "{e}");
    assert!(
        e.contains("không nằm trong allowlist") && e.contains("cap[Proc]"),
        "{e}"
    );
    assert!(r.1.is_empty(), "không được chạy: {:?}", r.1);
    let _ = std::fs::remove_dir_all(&d);
}

/// (c) grant **basename** `echo` + chạy `/bin/echo` ⇒ chạy (so cả basename).
#[test]
fn grant_basename_echo_chay() {
    let d = tam("c");
    let r = ca(
        &d,
        "c.vish",
        &vish("/bin/echo a"),
        &["--strict-caps", "--grant", "p=echo"],
    );
    assert_eq!(r.0, 0, "{r:?}");
    assert_eq!(r.1, b"[a]\n");
    let _ = std::fs::remove_dir_all(&d);
}

/// (d) **không** grant ⇒ không giới hạn: `/usr/bin/grep` chạy được (tương thích).
#[test]
fn khong_grant_khong_gioi_han() {
    let d = tam("d");
    let r = ca(
        &d,
        "d.vish",
        &vish("/usr/bin/grep -E \"^\" /etc/hosts"),
        &["--strict-caps"],
    );
    assert_eq!(r.0, 0, "{r:?}");
    assert!(!r.1.is_empty(), "grep phải in hosts");
    let _ = std::fs::remove_dir_all(&d);
}

/// (e) **hai** grant (echo + grep) + pipeline 2 stage ⇒ chạy, `[a]`.
#[test]
fn hai_grant_pipeline_hai_stage() {
    let d = tam("e");
    let r = ca(
        &d,
        "e.vish",
        &vish("/bin/echo a | /usr/bin/grep a"),
        &[
            "--strict-caps",
            "--grant",
            "p=/bin/echo",
            "--grant",
            "p=/usr/bin/grep",
        ],
    );
    assert_eq!(r.0, 0, "{r:?}");
    assert_eq!(r.1, b"[a]\n");
    let _ = std::fs::remove_dir_all(&d);
}

/// (f) `--grant x=/bin/echo` cho **`cap[Dir]`** ⇒ lỗi rõ “kiểu không khớp”, exit 2.
#[test]
fn grant_proc_cho_cap_dir_la_loi_kieu() {
    let d = tam("f");
    let r = ca(
        &d,
        "f.vis",
        "fn main(x: cap[Dir] @ p\"./\"):\n    print(1)\n",
        &["--strict-caps", "--grant", "x=/bin/echo"],
    );
    assert_eq!(r.0, 2, "{r:?}");
    assert!(r.2.contains("kiểu không khớp"), "{}", r.2);
    assert!(
        r.2.contains("cap[Proc]") && r.2.contains("cap[Dir]"),
        "{}",
        r.2
    );
    assert!(r.1.is_empty());
    let _ = std::fs::remove_dir_all(&d);
}
