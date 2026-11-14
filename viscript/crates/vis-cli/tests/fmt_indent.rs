// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-85 (VD-109)** — nhánh B: thụt lề `.vish` **có ngữ nghĩa ở cấp tệp** (đo được) ⇒ **không**
//! chuẩn hoá; `fmt --check` chỉ **cảnh báo** (không đỏ) và `fmt --indent-report` **đếm** dòng lệch.

use std::path::{Path, PathBuf};
use std::process::Command;

fn tam(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static DEM: AtomicUsize = AtomicUsize::new(0);
    let n = DEM.fetch_add(1, Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!("vis-fmtind-{tag}-{}-{}", std::process::id(), n));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("tmp");
    d
}

fn ghi(d: &Path, ten: &str, src: &str) -> PathBuf {
    let p = d.join(ten);
    std::fs::write(&p, src).expect("ghi");
    p
}

fn vis(args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_vis"))
        .args(args)
        .output()
        .expect("chạy vis");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).to_string(),
        String::from_utf8_lossy(&o.stderr).to_string(),
    )
}

const HAI: &str = "cmd f() -> list[int]:\n  let a = [1, 2]\n  let b = a\n  b\n\nprint(f())\n";
const BON: &str = "cmd f() -> list[int]:\n    let a = [1, 2]\n    let b = a\n    b\n\nprint(f())\n";

/// (a) Thân `cmd` 2 dấu cách ⇒ báo **3 dòng lệch** (không sửa).
#[test]
fn report_hai_dau_cach() {
    let d = tam("hai");
    let f = ghi(&d, "a.vish", HAI);
    let (c, out, err) = vis(&["fmt", "--indent-report", f.to_str().unwrap()]);
    assert_eq!(c, 0, "{err}");
    assert!(out.contains(": 3 dòng lệch"), "{out}");
    assert!(out.contains("1 tệp, 3 dòng lệch"), "{out}");
    assert_eq!(
        std::fs::read_to_string(&f).unwrap(),
        HAI,
        "không được sửa tệp"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// (b) Thân 4 dấu cách ⇒ **0** dòng lệch.
#[test]
fn report_bon_dau_cach_sach() {
    let d = tam("bon");
    let f = ghi(&d, "a.vish", BON);
    let (c, out, _) = vis(&["fmt", "--indent-report", f.to_str().unwrap()]);
    assert_eq!(c, 0);
    assert!(out.contains("0 tệp, 0 dòng lệch"), "{out}");
    let _ = std::fs::remove_dir_all(&d);
}

/// (c) Hỗn hợp 2/4 trong khối + dòng cấp tệp thụt 1 ⇒ đếm đúng, không sửa.
#[test]
fn report_hon_hop() {
    let d = tam("hon-hop");
    let src = "cmd f() -> int:\n  let a = 1\n    let b = 2\n    b\n\n print(f())\n";
    let f = ghi(&d, "a.vish", src);
    let (c, out, _) = vis(&["fmt", "--indent-report", f.to_str().unwrap()]);
    assert_eq!(c, 0);
    assert!(out.contains(": 2 dòng lệch"), "{out}");
    assert_eq!(std::fs::read_to_string(&f).unwrap(), src);
    let _ = std::fs::remove_dir_all(&d);
}

/// (d) `--check` trên `.vish` lệch thụt lề ⇒ **exit 0**, stderr **cảnh báo**, không stdout.
#[test]
fn check_canh_bao_khong_do() {
    let d = tam("check");
    let f = ghi(&d, "a.vish", HAI);
    let (c, out, err) = vis(&["fmt", "--check", f.to_str().unwrap()]);
    assert_eq!(c, 0, "cảnh báo không được làm đỏ: {err}");
    assert_eq!(out, "", "không có tệp lệch định dạng");
    assert!(err.contains("cảnh báo"), "{err}");
    assert!(err.contains("thụt lề"), "{err}");
    assert_eq!(std::fs::read_to_string(&f).unwrap(), HAI);
    let _ = std::fs::remove_dir_all(&d);
}

/// (e) Thư mục 2 tệp lệch ⇒ tổng **2 tệp**; tệp chuẩn không tính.
#[test]
fn report_thu_muc() {
    let d = tam("thu-muc");
    ghi(&d, "a.vish", HAI);
    ghi(&d, "b.vish", &HAI.replace("  ", " "));
    ghi(&d, "c.vish", BON);
    let (c, out, _) = vis(&["fmt", "--indent-report", d.to_str().unwrap()]);
    assert_eq!(c, 0);
    assert!(out.contains("2 tệp,"), "{out}");
    let _ = std::fs::remove_dir_all(&d);
}

/// (f) `vis fmt` mặc định **không** đổi thụt lề `.vish` (giữ nguyên văn) — khoá hành vi cũ.
#[test]
fn fmt_mac_dinh_giu_nguyen() {
    let d = tam("giu");
    let f = ghi(&d, "a.vish", HAI);
    let (c, out, err) = vis(&["fmt", f.to_str().unwrap()]);
    assert_eq!(c, 0, "{err}");
    assert_eq!(out, HAI, "fmt không được chuẩn hoá thụt lề .vish");
    let _ = std::fs::remove_dir_all(&d);
}
