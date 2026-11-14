// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-85b (VD-109)** — `.vish` **không có block cấp tệp** (§9.5): dòng cấp tệp thụt lề phải báo
//! **`VS0201`** rõ, không còn bị **bỏ im lặng**; các dạng hợp lệ (thân `cmd`, nhóm nhiều dòng, `.vis`
//! script) giữ nguyên.

use std::path::{Path, PathBuf};
use std::process::Command;

fn tam(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static DEM: AtomicUsize = AtomicUsize::new(0);
    let n = DEM.fetch_add(1, Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!("vis-vind-{tag}-{}-{}", std::process::id(), n));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("tmp");
    d
}

fn ghi(d: &Path, ten: &str, src: &str) -> PathBuf {
    let p = d.join(ten);
    std::fs::write(&p, src).expect("ghi");
    p
}

fn run(f: &Path, vm: bool) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_vis"));
    c.arg("run");
    if vm {
        c.arg("--vm").arg("--no-cache");
    }
    let o = c.arg(f).output().expect("chạy vis");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).to_string(),
        String::from_utf8_lossy(&o.stderr).to_string(),
    )
}

/// Cả interp + VM: cùng exit/stdout/mã; trả kết quả interp.
fn hai_duong(f: &Path, mong: i32) -> (String, String) {
    let (c1, o1, e1) = run(f, false);
    let (c2, o2, e2) = run(f, true);
    assert_eq!(c1, c2, "exit lệch: {e1} / {e2}");
    assert_eq!(o1, o2, "stdout lệch: {o1:?} / {o2:?}");
    assert_eq!(c1, mong, "exit: {e1}");
    (o1, e1)
}

fn loi_thut_le(e: &str) -> bool {
    e.contains("VS0201") && e.contains("câu lệnh cấp tệp không được thụt lề")
}

/// (a) Cả tệp thụt 1 dấu cách ⇒ `VS0201` từng dòng (trước đây **im lặng**, exit 0).
#[test]
fn ca_tep_thut_bao_loi() {
    let d = tam("ca-tep");
    let f = ghi(&d, "a.vish", " print(\"x\")\n print(\"y\")\n");
    let (o, e) = hai_duong(&f, 2);
    assert_eq!(o, "", "không được chạy im lặng");
    assert!(loi_thut_le(&e), "{e}");
    assert!(e.contains(":1:2:") && e.contains(":2:2:"), "{e}");
    let _ = std::fs::remove_dir_all(&d);
}

/// (b) `topmix`: dòng 1 thụt, dòng 2 cột 1 ⇒ báo lỗi dòng 1, **không** in `y`.
#[test]
fn dong_thut_dau_bao_loi() {
    let d = tam("topmix");
    let f = ghi(&d, "a.vish", " print(\"x\")\nprint(\"y\")\n");
    let (o, e) = hai_duong(&f, 2);
    assert_eq!(o, "");
    assert!(loi_thut_le(&e), "{e}");
    assert!(e.contains(":1:2:"), "{e}");
    let _ = std::fs::remove_dir_all(&d);
}

/// (c) `topmix2`: dòng 2 thụt ⇒ báo lỗi dòng 2 (trước đây in cả `y` lẫn `x`).
#[test]
fn dong_thut_sau_bao_loi() {
    let d = tam("topmix2");
    let f = ghi(&d, "a.vish", "print(\"y\")\n print(\"x\")\n");
    let (o, e) = hai_duong(&f, 2);
    assert_eq!(o, "");
    assert!(loi_thut_le(&e), "{e}");
    assert!(e.contains(":2:2:"), "{e}");
    let _ = std::fs::remove_dir_all(&d);
}

/// (d) `.vis` **script** với thân `fn` thụt lề vẫn hợp lệ (không phá script).
#[test]
fn vis_script_van_hop_le() {
    let d = tam("script");
    let f = ghi(&d, "a.vis", "fn main():\n    print(\"ok\")\n");
    let (o, e) = hai_duong(&f, 0);
    assert_eq!(o, "ok\n", "{e}");
    let _ = std::fs::remove_dir_all(&d);
}

/// (e) Thân `cmd` thụt 2 dấu cách vẫn chạy (không bị báo oan).
#[test]
fn than_cmd_thut_van_chay() {
    let d = tam("cmd");
    let f = ghi(
        &d,
        "a.vish",
        "cmd f() -> int:\n  let a = 5\n  a\n\nprint(f())\n",
    );
    let (o, e) = hai_duong(&f, 0);
    assert_eq!(o, "5\n", "{e}");
    let _ = std::fs::remove_dir_all(&d);
}

/// (f) Nhóm `( … )` nhiều dòng ở cấp tệp — dòng tiếp nối thụt vẫn hợp lệ (`test/cmd/nhom/nhieu-dong`).
#[test]
fn nhom_nhieu_dong_van_hop_le() {
    let d = tam("nhom");
    let f = ghi(&d, "a.vish", "( 1 +\n  1 )\n");
    let (o, e) = hai_duong(&f, 0);
    assert_eq!(o, "2\n", "{e}");
    let _ = std::fs::remove_dir_all(&d);
}

/// (g) Khai báo `fn` + thân thụt lề ở `.vish` vẫn hợp lệ (không báo oan khối khai báo).
#[test]
fn khai_bao_fn_trong_vish_van_hop_le() {
    let d = tam("fn");
    let f = ghi(
        &d,
        "a.vish",
        "fn gap(x: int) -> int:\n    return x + 1\n\nprint(gap(4))\n",
    );
    let (o, e) = hai_duong(&f, 0);
    assert_eq!(o, "5\n", "{e}");
    let _ = std::fs::remove_dir_all(&d);
}
