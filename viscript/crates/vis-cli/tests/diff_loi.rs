// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-86 (VD-109)** — **ca lỗi** của cú pháp mới: chạy **cả hai đường** (`vis run` và
//! `vis run --vm`) trên cùng tệp, so **byte** stdout/stderr/exit; hai đường phải cùng chẩn đoán.
//! Gồm: `send` sai kiểu, `await` không phải Task, thiếu `cap`, thiếu gói (`VS0701`), deadlock,
//! `assert` sai, send-sau-close.

use std::path::{Path, PathBuf};
use std::process::Command;

fn tam(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static DEM: AtomicUsize = AtomicUsize::new(0);
    let n = DEM.fetch_add(1, Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!("vis-diffloi-{tag}-{}-{}", std::process::id(), n));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("tmp");
    d
}

fn chay(f: &Path, vm: bool) -> (i32, String, String) {
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

fn so_hai_duong(tag: &str, src: &str, ma: &str) {
    let d = tam(tag);
    let f = d.join("a.vis");
    std::fs::write(&f, src).expect("ghi");
    let (c1, o1, e1) = chay(&f, false);
    let (c2, o2, e2) = chay(&f, true);
    assert_eq!(c1, c2, "exit lệch ({tag}):\ninterp {e1}\nvm     {e2}");
    assert_eq!(o1, o2, "stdout lệch ({tag}): {o1:?} / {o2:?}");
    assert_eq!(e1, e2, "stderr lệch ({tag}):\ninterp {e1}\nvm     {e2}");
    assert!(e1.contains(ma), "thiếu `{ma}` ({tag}): {e1}");
    let _ = std::fs::remove_dir_all(&d);
}

/// **ST-86**: lỗi tĩnh — `send` sai kiểu.
#[test]
fn send_sai_kieu_hai_duong() {
    so_hai_duong(
        "send",
        "fn main():\n    let c = chan[int](1)\n    let _ = c.send(\"x\")\n",
        "VS0412",
    );
}

/// **ST-86**: lỗi tĩnh — `await` không phải Task.
#[test]
fn await_khong_phai_task_hai_duong() {
    so_hai_duong("await", "let x = await 5\nprint(x)\n", "VS0412");
}

/// **ST-86**: lỗi tĩnh — thiếu đối số `cap`.
#[test]
fn thieu_cap_hai_duong() {
    so_hai_duong(
        "cap",
        "fn can(c: cap[Dir]) -> int:\n    0\n\nfn main():\n    print(can())\n",
        "VS0401",
    );
}

/// **ST-86**: thiếu gói — chỉ CLI phân giải gói (`VS0701`), hai đường phải cùng mã.
#[test]
fn thieu_pkg_hai_duong() {
    so_hai_duong("pkg", "use pkg.khong_co\n\nprint(1)\n", "VS0701");
}

/// **ST-86/ST-94**: deadlock kênh **động** — `VS0505`, **không treo** (literal `0` đã bị tyck).
#[test]
fn deadlock_hai_duong() {
    so_hai_duong(
        "deadlock",
        "fn main():\n    let n = 0\n    let c = chan[int](n)\n    let _ = c.send(1)\n",
        "VS0505",
    );
}

/// **ST-86**: `assert` sai — `VS0501` (câu lệnh `assert`).
#[test]
fn assert_sai_hai_duong() {
    so_hai_duong("assert", "assert 1 == 2\n", "VS0501");
}

/// **ST-86**: send-sau-close — `VS0504`.
#[test]
fn send_sau_close_hai_duong() {
    so_hai_duong(
        "close",
        "fn main():\n    let c = chan[int](1)\n    c.close()\n    let _ = c.send(1)\n",
        "VS0504",
    );
}
