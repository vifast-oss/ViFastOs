// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-56 (nợ lõi A.2)** — `ref(expr)` chạy được ở **cả hai** đường (VD-92).
//!
//! `ref(x)` **giữ nguyên kiểu** của `x` và hạ **identity**: **không** tạo giá trị mới, **không**
//! mượn thật (borrow là nợ VS2+). Ca: `ref(1) + 1`, `ref(ref("a"))`, `ref(p).x`; interp ↔ `--vm`
//! **cùng byte** (stdout + stderr + exit). `ref(1, 2)` vẫn là lỗi **cú pháp** `VS0201` (một đối số).

use std::path::{Path, PathBuf};
use std::process::Command;

fn tam(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vis-st56-ref-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("thư mục tạm");
    d
}

fn ghi(d: &Path, ten: &str, src: &str) -> PathBuf {
    let p = d.join(format!("{ten}.vis"));
    std::fs::write(
        &p,
        format!("# SPDX-License-Identifier: MIT OR Apache-2.0\n{src}"),
    )
    .expect("ghi");
    p
}

fn chay(p: &Path, vm: bool) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_vis"));
    c.arg("run");
    if vm {
        c.arg("--vm").arg("--no-cache");
    }
    let o = c.arg(p).output().expect("chạy `vis run`");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).to_string(),
        String::from_utf8_lossy(&o.stderr).to_string(),
    )
}

fn hai_duong(d: &Path, ten: &str, src: &str) -> (i32, String, String) {
    let p = ghi(d, ten, src);
    let a = chay(&p, false);
    let b = chay(&p, true);
    assert_eq!(
        (a.0, &a.1, &a.2),
        (b.0, &b.1, &b.2),
        "interp ≠ `--vm` (exit/stdout/stderr)\n  interp: {a:?}\n  vm: {b:?}"
    );
    a
}

#[test]
fn ref_giu_kieu_va_identity_hai_duong() {
    let d = tam("identity");
    let src = "type P:\n    x: int\n\nfn main():\n    print(ref(1) + 1)\n    print(ref(ref(\"a\")))\n    \
               let p: P = {x: 7}\n    print(ref(p).x)\n    print([ref(2), ref(3)])\n";
    let (c, o, e) = hai_duong(&d, "identity", src);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "2\na\n7\n[2, 3]\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn ref_khong_muon_that_chi_la_identity() {
    // Nếu có **mượn thật** thì `r` phải thấy `x = 2`; identity (VD-92) thì `r` giữ **bản sao** `1`.
    let d = tam("khong-muon");
    let src =
        "fn main():\n    var x = 1\n    let r = ref(x)\n    x = 2\n    print(r)\n    print(x)\n";
    let (c, o, e) = hai_duong(&d, "khong-muon", src);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(
        o, "1\n2\n",
        "`ref` là identity (**chưa có borrow** — nợ VS2+)"
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn ref_khong_con_chan_doan_khi_chay() {
    // Trước ST-56: interp `VS0501` + codegen `Unsupported("ref")`. Nay **không** chẩn đoán nào.
    let d = tam("sach");
    let src = "fn main():\n    print(ref(1))\n";
    let p = ghi(&d, "sach", src);
    let o = Command::new(env!("CARGO_BIN_EXE_vis"))
        .arg("check")
        .arg(&p)
        .output()
        .expect("chạy `vis check`");
    assert_eq!(
        o.status.code().unwrap_or(-1),
        0,
        "`check` phải sạch: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    let (c, out, e) = hai_duong(&d, "sach", src);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(out, "1\n");
    assert!(e.is_empty(), "không được có chẩn đoán: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn ref_hai_doi_so_van_la_vs0201() {
    let d = tam("hai-doi-so");
    let (c, _, e) = hai_duong(&d, "hai-doi-so", "fn main():\n    print(ref(1, 2))\n");
    assert_eq!(c, 2, "lỗi cú pháp ⇒ exit 2, stderr: {e}");
    assert!(e.contains("VS0201"), "stderr: {e}");
    let _ = std::fs::remove_dir_all(&d);
}
