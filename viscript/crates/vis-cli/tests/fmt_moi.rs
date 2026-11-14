// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-77 (VD-108)** — `vis fmt` cho cú pháp mới ở mức CLI: `.vish` (CmdCST) + `--check` trên
//! **thư mục** (và toàn `test/`). Ba ca mạnh: trước ST-77 fmt **bail out** `.vish`, **không nhận**
//! thư mục, và làm **mất thụt lề** khối `cmd` nếu chỉ dùng render script.

use std::path::{Path, PathBuf};
use std::process::Command;

fn tam(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static DEM: AtomicUsize = AtomicUsize::new(0);
    let n = DEM.fetch_add(1, Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!("vis-fmt-{tag}-{}-{}", std::process::id(), n));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("tmp");
    d
}

fn vis(args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_vis"))
        .args(args)
        .output()
        .expect("chạy `vis`");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).to_string(),
        String::from_utf8_lossy(&o.stderr).to_string(),
    )
}

/// `.vish` có **tab** ở thân `cmd` ⇒ `vis fmt` phải thành công, tab → 4 dấu cách, **giữ** thụt lề.
#[test]
fn fmt_vish_than_cmd_giu_thu_le() {
    let d = tam("vish");
    let p = d.join("a.vish");
    std::fs::write(
        &p,
        "cmd h() -> list[int]:\n\tlet a = [1]\n\ta\n\nprint(h())\n",
    )
    .expect("ghi");
    let (ma, out, err) = vis(&["fmt", p.to_str().unwrap()]);
    assert_eq!(ma, 0, "stderr: {err}");
    assert_eq!(
        out,
        "cmd h() -> list[int]:\n    let a = [1]\n    a\n\nprint(h())\n"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// `--check <thư mục>` đệ quy `.vis` + `.vish`; in tệp lệch, exit 1; sau `--write` thì sạch.
#[test]
fn fmt_check_thu_muc_co_vish() {
    let d = tam("dir");
    std::fs::write(d.join("a.vis"), "fn f():\n  1\n").expect("ghi");
    std::fs::write(d.join("b.vish"), "cmd h() -> int:\n\t1\n").expect("ghi");
    let (ma, out, _) = vis(&["fmt", "--check", d.to_str().unwrap()]);
    assert_eq!(ma, 1, "phải exit 1 khi có tệp lệch");
    assert!(out.contains("a.vis"), "stdout: {out}");
    assert!(out.contains("b.vish"), "stdout: {out}");
    let (ma, _, err) = vis(&["fmt", "--write", d.to_str().unwrap()]);
    assert_eq!(ma, 0, "stderr: {err}");
    let (ma, out, err) = vis(&["fmt", "--check", d.to_str().unwrap()]);
    assert_eq!(ma, 0, "stderr: {err}; stdout: {out}");
    assert_eq!(out, "");
    let _ = std::fs::remove_dir_all(&d);
}

/// Ca mạnh nhất: `vis fmt --check test/` xanh trên **toàn bộ** fixture (trừ input cố ý: `test/fmt`,
/// `test/check`, `test/doc-examples`, tệp có sibling `.err` — allowlist nhỏ nhất, có lý do trong mã).
#[test]
fn fmt_check_toan_bo_test() {
    let viscript = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (ma, out, err) = vis(&["fmt", "--check", viscript.join("test").to_str().unwrap()]);
    assert_eq!(ma, 0, "stdout: {out}\nstderr: {err}");
    assert_eq!(out, "", "không được còn tệp lệch");
}
