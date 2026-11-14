// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-94 (VD-112)** — kênh: **`chan[T](0)` bị từ chối** (rendezvous không thể trên scheduler một
//! luồng), `close`/`recv`/`send` sau close, deadlock `VS0505` **không treo**, và **drain task `Moi`**
//! (spawn không await vẫn join cuối, kể cả lỗi). Mọi ca chạy **cả hai đường**, so **byte**.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

type Kq = (i32, Vec<u8>, String);

fn tam(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static DEM: AtomicUsize = AtomicUsize::new(0);
    let d = std::env::temp_dir().join(format!(
        "vis-kenh94-{tag}-{}-{}",
        std::process::id(),
        DEM.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("tmp");
    d
}

fn chay(d: &Path, ten: &str, vm: bool) -> (Kq, Duration) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_vis"));
    c.current_dir(d).arg("run");
    if vm {
        c.arg("--vm").arg("--no-cache");
    }
    let t0 = Instant::now();
    let o = c.arg(ten).output().expect("chạy vis");
    (
        (
            o.status.code().unwrap_or(-1),
            o.stdout,
            String::from_utf8_lossy(&o.stderr).to_string(),
        ),
        t0.elapsed(),
    )
}

/// Ghi + chạy hai đường; khẳng định **byte y hệt**; trả `(kết quả, thời gian chậm nhất)`.
fn ca(d: &Path, ten: &str, src: &str) -> (Kq, Duration) {
    let f = d.join(format!("{ten}.vis"));
    std::fs::write(
        &f,
        format!("# SPDX-License-Identifier: MIT OR Apache-2.0\n{src}"),
    )
    .expect("ghi");
    let (a, ta) = chay(d, &format!("{ten}.vis"), false);
    let (b, tb) = chay(d, &format!("{ten}.vis"), true);
    assert_eq!(a, b, "hai đường lệch ({ten}):\n interp {a:?}\n vm {b:?}");
    (a, ta.max(tb))
}

/// (1) `chan[T](0)` **literal** ⇒ **từ chối khi tạo** `VS0505`, exit 1, **không treo** (<5 s).
#[test]
fn chan_0_literal_bi_tu_choi() {
    let d = tam("lit");
    let (a, dt) = ca(
        &d,
        "c0",
        "fn main():\n    let c = chan[int](0)\n    let _ = c.send(1)\n",
    );
    assert_eq!(a.0, 1, "{:?}", a);
    let e = &a.2;
    assert!(e.contains("VS0505") && e.contains("capacity 0"), "{e}");
    assert!(dt < Duration::from_secs(5), "treo {dt:?}");
    let _ = std::fs::remove_dir_all(&d);
}

/// (2) capacity 0 **động** (`let n = 0`) ⇒ runtime `VS0505`, exit 1, không treo.
#[test]
fn chan_0_dong_runtime_vs0505() {
    let d = tam("dong");
    let (a, dt) = ca(
        &d,
        "cd",
        "fn main():\n    let n = 0\n    let c = chan[int](n)\n    let _ = c.send(1)\n",
    );
    assert_eq!(a.0, 1, "{:?}", a);
    let e = &a.2;
    assert!(e.contains("VS0505") && e.contains("capacity 0"), "{e}");
    assert!(dt < Duration::from_secs(5), "treo {dt:?}");
    let _ = std::fs::remove_dir_all(&d);
}

/// (3) `chan[T](0)` trong **hàm** (không chỉ `main`) ⇒ cùng từ chối runtime, exit 1.
#[test]
fn chan_0_trong_ham_cung_chan() {
    let d = tam("ham");
    let (a, dt) = ca(
        &d,
        "h",
        "fn mo() -> int:\n    let c = chan[int](0)\n    0\n\nfn main():\n    print(mo())\n",
    );
    assert_eq!(a.0, 1, "{a:?}");
    assert!(
        a.2.contains("VS0505") && a.2.contains("capacity 0"),
        "{}",
        a.2
    );
    assert!(dt < Duration::from_secs(5));
    let _ = std::fs::remove_dir_all(&d);
}

/// (4) `chan(0)` không còn treo: đo < 5 s trên cả hai đường.
#[test]
fn chan_0_do_thoi_gian_khong_treo() {
    let d = tam("time");
    let (_, dt) = ca(
        &d,
        "t",
        "fn main():\n    let c = chan[str](0)\n    c.close()\n    print(c.recv())\n",
    );
    assert!(dt < Duration::from_secs(5), "treo {dt:?}");
    let _ = std::fs::remove_dir_all(&d);
}

/// (5) `recv` **sau close** ⇒ `none` (cả hai đường, byte y hệt).
#[test]
fn recv_sau_close_none() {
    let d = tam("recv");
    let (a, _) = ca(
        &d,
        "r",
        "fn main():\n    let c = chan[int](1)\n    c.close()\n    print(c.recv())\n    print(c.recv())\n",
    );
    assert_eq!(a.0, 0, "{:?}", a);
    assert_eq!(a.1, b"none\nnone\n");
    let _ = std::fs::remove_dir_all(&d);
}

/// (6) `send` **sau close** ⇒ `VS0504`, exit 1 (cả hai đường).
#[test]
fn send_sau_close_vs0504() {
    let d = tam("send");
    let (a, _) = ca(
        &d,
        "s",
        "fn main():\n    let c = chan[int](1)\n    c.close()\n    let _ = c.send(1)\n",
    );
    assert_eq!(a.0, 1, "{:?}", a);
    assert!(a.2.contains("VS0504"));
    let _ = std::fs::remove_dir_all(&d);
}

/// (7) `recv` kênh **rỗng chưa close**, không task ⇒ `VS0505`, **< 5 s**.
#[test]
fn recv_rong_khong_task_vs0505() {
    let d = tam("rong");
    let (a, dt) = ca(
        &d,
        "e",
        "fn main():\n    let c = chan[int](1)\n    print(c.recv())\n",
    );
    assert_eq!(a.0, 1, "{:?}", a);
    assert!(a.2.contains("VS0505"));
    assert!(dt < Duration::from_secs(5), "treo {dt:?}");
    let _ = std::fs::remove_dir_all(&d);
}

/// (8) task **spawn không await** vẫn in ra (drain join cuối).
#[test]
fn drain_task_in_khong_await() {
    let d = tam("drain");
    let (a, _) = ca(
        &d,
        "d",
        "async fn in_so(n: int) -> int:\n    print(n)\n    n\n\nlet t = spawn in_so(7)\nprint(\"main\")\n",
    );
    assert_eq!(a.0, 0, "{:?}", a);
    assert_eq!(a.1, b"main\n7\n");
    let _ = std::fs::remove_dir_all(&d);
}

/// (9) task **lỗi** không await ⇒ `VS0506`, exit 1.
#[test]
fn drain_task_loi_vs0506() {
    let d = tam("loi");
    let (a, _) = ca(
        &d,
        "l",
        "async fn hong() -> int:\n    1 / 0\n\nlet t = spawn hong()\nprint(\"main\")\n",
    );
    assert_eq!(a.0, 1, "{:?}", a);
    assert!(a.2.contains("VS0506"));
    let _ = std::fs::remove_dir_all(&d);
}

/// (10) **100 task** spawn không await ⇒ đủ 100 dòng, **thứ tự spawn** (0..99), hai đường byte y hệt.
#[test]
fn drain_100_task_thu_tu_tat_dinh() {
    let d = tam("100");
    let mut src = String::from("async fn in_so(n: int) -> int:\n    print(n)\n    n\n\n");
    src.push_str("var i = 0\nwhile i < 100:\n    let t = spawn in_so(i)\n    i = i + 1\n");
    let (a, _) = ca(&d, "n", &src);
    assert_eq!(a.0, 0, "{:?}", a);
    let out = String::from_utf8(a.1).expect("utf8");
    let dong: Vec<&str> = out.lines().collect();
    assert_eq!(dong.len(), 100, "{out:?}");
    for (i, l) in dong.iter().enumerate() {
        assert_eq!(*l, i.to_string(), "thứ tự spawn: {out:?}");
    }
    let _ = std::fs::remove_dir_all(&d);
}
