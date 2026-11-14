// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-61 (VD-91/VD-96)** — **kênh** `chan[T]`: `send`/`recv`/`close`, **deadlock có chẩn đoán**
//! (`VS0505`, **không** treo), gửi sau `close` (`VS0504`), `close` idempotent, `recv` sau close trả
//! nốt hàng rồi `none`. Mọi ca chạy **cả hai đường** và so **byte**.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

fn tam(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vis-kenh-{tag}-{}", std::process::id()));
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

fn ca(d: &Path, ten: &str, src: &str) -> (i32, String, String) {
    let p = ghi(d, ten, src);
    let a = chay(&p, false);
    let b = chay(&p, true);
    assert_eq!(
        (a.0, &a.1, &a.2),
        (b.0, &b.1, &b.2),
        "interp ≠ `--vm`\n  interp: {a:?}\n  vm: {b:?}"
    );
    a
}

#[test]
fn tao_send_recv_va_fifo_hai_duong() {
    let d = tam("fifo");
    // Đóng **trước** lần nhận thứ ba (rỗng + chưa close + không task ⇒ `VS0505`, không phải `none`).
    let src = "fn main():\n    let c = chan[int](3)\n    let _ = c.send(1)\n    let _ = c.send(2)\n    print(c.recv())\n    print(c.recv())\n    c.close()\n    print(c.recv())\n";
    let (c, o, e) = ca(&d, "fifo", src);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "some(1)\nsome(2)\nnone\n");
    let _ = std::fs::remove_dir_all(&d);
}

/// **Deadlock phải kết thúc < 1 s** (không treo) và báo `VS0505` **như nhau** hai đường.
#[test]
fn deadlock_khong_treo_va_bao_vs0505() {
    let d = tam("dl");
    for (ten, src) in [
        (
            "gui-day",
            "fn main():\n    let c = chan[int](0)\n    let _ = c.send(1)\n",
        ),
        (
            "nhan-rong",
            "fn main():\n    let c = chan[int](1)\n    print(c.recv())\n",
        ),
        (
            "day-sau-mot",
            "fn main():\n    let c = chan[int](1)\n    let _ = c.send(1)\n    let _ = c.send(2)\n",
        ),
    ] {
        let p = ghi(&d, ten, src);
        for vm in [false, true] {
            let t0 = Instant::now();
            let (c, _, e) = chay(&p, vm);
            let dt = t0.elapsed();
            assert_eq!(c, 1, "{ten} (vm={vm}): stderr: {e}");
            assert!(e.contains("VS0505"), "{ten} (vm={vm}): stderr: {e}");
            assert!(
                dt.as_secs_f64() < 1.0,
                "{ten} (vm={vm}) **treo** {dt:?} — deadlock phải kết thúc < 1 s"
            );
        }
    }
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn close_idempotent_va_nhan_not_hang() {
    let d = tam("close");
    let src = "fn main():\n    let c = chan[int](2)\n    let _ = c.send(1)\n    let _ = c.send(2)\n    c.close()\n    c.close()\n    print(c.recv())\n    print(c.recv())\n    print(c.recv())\n";
    let (c, o, e) = ca(&d, "close", src);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "some(1)\nsome(2)\nnone\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn send_sau_close_la_vs0504() {
    let d = tam("sau-close");
    let src = "fn main():\n    let c = chan[int](1)\n    c.close()\n    let _ = c.send(1)\n";
    let (c, _, e) = ca(&d, "sauclose", src);
    assert_eq!(c, 1, "stderr: {e}");
    assert!(e.contains("VS0504"), "stderr: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

/// `spawn` producer + kênh **có đệm**: `recv` **drain** hàng đợi sẵn sàng rồi lấy được hàng.
#[test]
fn spawn_producer_voi_dem_du_hai_duong() {
    let d = tam("spawn");
    let src = "async fn san(c: Chan[int]) -> int:\n    let _ = c.send(1)\n    0\n\nfn main():\n    let c = chan[int](1)\n    let _ = spawn san(c)\n    print(c.recv())\n";
    let (c, o, e) = ca(&d, "spawn", src);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "some(1)\n");
    let _ = std::fs::remove_dir_all(&d);
}
