// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-79 (VD-108)** — `vis test` chạy **`test fn`** có `await`/`spawn`/`chan`: mỗi test một dòng
//! TAP, join cuối khối, task lỗi ⇒ `not ok` + `VS0506`, kênh deadlock ⇒ `not ok` + `VS0505`
//! (không treo), và **`--vm` cho cùng byte** với interp.
//!
//! Ghi chú phạm vi: `async fn` **trong** thân `test` là **không** hỗ trợ (thân test là chuỗi câu
//! lệnh, không phải vùng item) — dùng `async fn` **cấp tệp**, đúng nợ đã ghi ở R11 §5.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

fn tam(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static DEM: AtomicUsize = AtomicUsize::new(0);
    let n = DEM.fetch_add(1, Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!("vis-vt-{tag}-{}-{}", std::process::id(), n));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("tmp");
    d
}

fn ghi(d: &Path, ten: &str, src: &str) -> PathBuf {
    let p = d.join(ten);
    std::fs::write(&p, src).expect("ghi test");
    p
}

fn chay(d: &Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_vis"))
        .current_dir(d)
        .args(args)
        .output()
        .expect("chạy `vis`");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).to_string(),
        String::from_utf8_lossy(&o.stderr).to_string(),
    )
}

/// Chạy `vis test` và `vis test --vm`; khẳng định **cùng byte**; trả `(exit, tap)` của interp.
fn hai_duong(d: &Path, f: &Path) -> (i32, String) {
    let (c1, o1, e1) = chay(d, &["test", f.to_str().unwrap()]);
    let (c2, o2, e2) = chay(d, &["test", "--vm", f.to_str().unwrap()]);
    assert_eq!(
        o1, o2,
        "TAP lệch interp/VM:\n--- interp ---\n{o1}\n--- vm ---\n{o2}"
    );
    assert_eq!(c1, c2, "exit lệch: {e1} / {e2}");
    (c1, o1)
}

const AWAIT: &str =
    "async fn f() -> int:\n    return 7\n\ntest fn t_await():\n    assert(await f() == 7)\n";
const TASK: &str = "async fn g() -> int:\n    return 1\n\ntest fn t_task():\n    let t = g()\n    assert(await t == 1)\n";
const KENH: &str =
    "test fn t_kenh():\n    let c = chan[int](2)\n    c.send(5)\n    assert(c.recv() == Some(5))\n";
const SPAWN3: &str = "async fn f(n: int) -> int:\n    n + 1\n\ntest fn t_spawn():\n    let a = f(1)\n    let b = f(2)\n    let c = f(3)\n    assert(await a == 2)\n    assert(await b == 3)\n    assert(await c == 4)\n";
const LOI: &str = "async fn h() -> int:\n    assert(false)\n    1\n\ntest fn t_loi():\n    assert(await h() == 1)\n";
const DEADLOCK: &str =
    "test fn t_deadlock():\n    let c = chan[int](1)\n    assert(c.recv() == Some(1))\n";
const HON: &str = "test fn t_thuong():\n    assert(1 + 1 == 2)\n\nasync fn k() -> int:\n    3\n\ntest fn t_async():\n    assert(await k() == 3)\n";
const KHONG_AWAIT: &str = "async fn h() -> int:\n    assert(false)\n    1\n\ntest fn t_khong_await():\n    let t = h()\n    print(\"tao task\")\n";

#[test]
fn await_fn_cap_tep() {
    let d = tam("await");
    let f = ghi(&d, "a.vis", AWAIT);
    let (c, tap) = hai_duong(&d, &f);
    assert_eq!(c, 0, "{tap}");
    assert_eq!(tap, "TAP version 13\n1..1\nok 1 - t_await\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn task_roi_await() {
    let d = tam("task");
    let f = ghi(&d, "a.vis", TASK);
    let (c, tap) = hai_duong(&d, &f);
    assert_eq!(c, 0, "{tap}");
    assert_eq!(tap, "TAP version 13\n1..1\nok 1 - t_task\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn kenh_send_recv() {
    let d = tam("kenh");
    let f = ghi(&d, "a.vis", KENH);
    let (c, tap) = hai_duong(&d, &f);
    assert_eq!(c, 0, "{tap}");
    assert_eq!(tap, "TAP version 13\n1..1\nok 1 - t_kenh\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn spawn_ba_task_dung_thu_tu() {
    let d = tam("spawn3");
    let f = ghi(&d, "a.vis", SPAWN3);
    let (c, tap) = hai_duong(&d, &f);
    assert_eq!(c, 0, "{tap}");
    assert_eq!(tap, "TAP version 13\n1..1\nok 1 - t_spawn\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn task_loi_not_ok_vs0506() {
    let d = tam("loi");
    let f = ghi(&d, "a.vis", LOI);
    let (c, tap) = hai_duong(&d, &f);
    assert_eq!(c, 1, "{tap}");
    assert!(tap.contains("not ok 1 - t_loi"), "{tap}");
    assert!(tap.contains("VS0506"), "{tap}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn kenh_deadlock_not_ok_khong_treo() {
    let d = tam("deadlock");
    let f = ghi(&d, "a.vis", DEADLOCK);
    let t = Instant::now();
    let (c, tap) = hai_duong(&d, &f);
    let giay = t.elapsed().as_secs_f64();
    assert!(giay < 5.0, "deadlock treo {giay} s: {tap}");
    assert_eq!(c, 1, "{tap}");
    assert!(tap.contains("not ok 1 - t_deadlock"), "{tap}");
    assert!(tap.contains("VS0505"), "{tap}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn hon_test_thuong_va_async() {
    let d = tam("hon");
    let f = ghi(&d, "a.vis", HON);
    let (c, tap) = hai_duong(&d, &f);
    assert_eq!(c, 0, "{tap}");
    assert_eq!(
        tap,
        "TAP version 13\n1..2\nok 1 - t_thuong\nok 2 - t_async\n"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// **Join cuối khối**: task tạo mà **không** await vẫn phải chạy; lỗi của nó làm ca đỏ `VS0506`.
#[test]
fn khong_await_van_join_va_loi() {
    let d = tam("join");
    let f = ghi(&d, "a.vis", KHONG_AWAIT);
    let (c, tap) = hai_duong(&d, &f);
    assert_eq!(c, 1, "{tap}");
    assert!(tap.contains("not ok 1 - t_khong_await"), "{tap}");
    assert!(tap.contains("VS0506"), "{tap}");
    let _ = std::fs::remove_dir_all(&d);
}
