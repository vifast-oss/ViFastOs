// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-73 (VD-107)** — `vis run --sandbox-check`: **probe Landlock chỉ đọc** + mức cưỡng chế runtime.
//!
//! In ra **stdout** (lệnh chẩn đoán, khác `--dry` in stderr); **không** chạy chương trình.
//! macOS: `landlock: không hỗ trợ trên macos` + exit **3**; Linux: đọc `/sys/kernel/security/lsm`
//! (không syscall) ⇒ có **0** / không **3** / không đọc được **4**.

use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp_dir(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static DEM: AtomicUsize = AtomicUsize::new(0);
    let n = DEM.fetch_add(1, Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!("vis-sb-{tag}-{}-{}", std::process::id(), n));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("thư mục tạm");
    d
}

fn ghi(p: &Path, s: &str) {
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

/// Ca 1 — macOS/khác: `không hỗ trợ trên <os>` + exit 3.
#[cfg(target_os = "macos")]
#[test]
fn sandbox_check_macos() {
    let d = tmp_dir("macos");
    ghi(&d.join("f.vis"), "fn main():\n    print(1)\n");
    let (code, out, err) = chay(&d, &["run", "--sandbox-check", "f.vis"]);
    assert_eq!(code, 3, "stderr: {err}");
    assert!(
        out.contains("landlock: không hỗ trợ trên macos"),
        "stdout: {out}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// Ca 2 — **luôn** in `cưỡng chế runtime:` và `cap điều kiện:`.
#[test]
fn sandbox_check_luon_in_cuong_che() {
    let d = tmp_dir("in");
    ghi(&d.join("f.vis"), "fn main():\n    print(1)\n");
    let (code, out, err) = chay(&d, &["run", "--sandbox-check", "f.vis"]);
    assert_eq!(code, 3, "macOS: {err}");
    assert!(out.contains("cưỡng chế runtime:"), "stdout: {out}");
    assert!(out.contains("cap điều kiện:"), "stdout: {out}");
    let _ = std::fs::remove_dir_all(&d);
}

/// Ca 3 — **không chạy** chương trình: `print(1)` không được xuất hiện như kết quả chạy.
#[test]
fn sandbox_check_khong_chay() {
    let d = tmp_dir("khong-chay");
    ghi(&d.join("f.vis"), "fn main():\n    print(\"DA-CHAY\")\n");
    let (_, out, _) = chay(&d, &["run", "--sandbox-check", "f.vis"]);
    assert!(!out.contains("DA-CHAY"), "stdout: {out}");
    let _ = std::fs::remove_dir_all(&d);
}

/// Ca 4 — `main(d: cap[Dir] @ p".")` ⇒ `cưỡng chế runtime: bật`.
#[test]
fn sandbox_check_voi_cap() {
    let d = tmp_dir("co-cap");
    ghi(
        &d.join("f.vis"),
        "fn main(d: cap[Dir] @ p\".\"):\n    print(1)\n",
    );
    let (_, out, _) = chay(&d, &["run", "--sandbox-check", "f.vis"]);
    assert!(out.contains("cưỡng chế runtime: bật"), "stdout: {out}");
    let _ = std::fs::remove_dir_all(&d);
}

/// Ca 5 — `main()` (không cap) ⇒ `cưỡng chế runtime: tắt`.
#[test]
fn sandbox_check_khong_cap() {
    let d = tmp_dir("khong-cap");
    ghi(&d.join("f.vis"), "fn main():\n    print(1)\n");
    let (_, out, _) = chay(&d, &["run", "--sandbox-check", "f.vis"]);
    assert!(out.contains("cưỡng chế runtime: tắt"), "stdout: {out}");
    let _ = std::fs::remove_dir_all(&d);
}

/// Ca 6 — cờ lạ cạnh `--sandbox-check` vẫn lỗi exit 2.
#[test]
fn sandbox_check_co_la() {
    let d = tmp_dir("co-la");
    ghi(&d.join("f.vis"), "fn main():\n    print(1)\n");
    let (code, _out, err) = chay(&d, &["run", "--sandbox-check", "--la", "f.vis"]);
    assert_eq!(code, 2, "stderr: {err}");
    assert!(err.contains("--la"), "stderr: {err}");
    let _ = std::fs::remove_dir_all(&d);
}

/// Linux — nhánh đọc `/sys/kernel/security/lsm` (không syscall): 0/3/4 + nêu LSM.
#[cfg(target_os = "linux")]
#[test]
fn sandbox_check_linux_doc_lsm() {
    let d = tmp_dir("linux");
    ghi(&d.join("f.vis"), "fn main():\n    print(1)\n");
    let (code, out, err) = chay(&d, &["run", "--sandbox-check", "f.vis"]);
    assert!(code == 0 || code == 3 || code == 4, "exit {code}: {err}");
    assert!(out.contains("landlock:"), "stdout: {out}");
    if code == 0 {
        assert!(out.contains("landlock: có (LSM: "), "stdout: {out}");
    }
    let _ = std::fs::remove_dir_all(&d);
}
