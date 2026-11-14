// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-63 (VD-100/VD-105)** — `std.net` **TCP client** qua `NetHost`: 5 ca e2e **loopback thật**
//! (+1 ca `--explain-perms`), mỗi ca chạy **cả hai đường** (`run` và `run --vm --no-cache`) và so
//! **từng byte** stdout/stderr/exit. Server là `TcpListener` thật trong test Rust (không mock, không
//! mạng ngoài). `Chan`/kênh không liên quan — đây là host mạng, ca nằm ở Rust theo VD-105 (7).

use std::io::Write;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

fn tam(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vis-net-{tag}-{}", std::process::id()));
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
    .expect("ghi chương trình");
    p
}

fn chay(p: &Path, vm: bool, no_cache: bool, explain: bool) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_vis"));
    c.arg("run");
    if vm {
        c.arg("--vm");
        if no_cache {
            c.arg("--no-cache");
        }
    }
    if explain {
        c.arg("--explain-perms");
    }
    let o = c.arg(p).output().expect("chạy `vis run`");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).to_string(),
        String::from_utf8_lossy(&o.stderr).to_string(),
    )
}

/// Chạy **hai đường** + so byte; trả kết quả đường interp.
fn ca(d: &Path, ten: &str, src: &str) -> (i32, String, String) {
    let p = ghi(d, ten, src);
    let a = chay(&p, false, false, false);
    let b = chay(&p, true, true, false);
    assert_eq!(
        a, b,
        "interp ≠ `--vm --no-cache`\n  interp: {a:?}\n  vm: {b:?}"
    );
    a
}

/// Server loopback nhận `n` kết nối, mỗi kết nối gửi `du` rồi đóng; trả `127.0.0.1:<cổng>`.
/// `n` = số lần chạy của test (interp + `--vm` [+ cache hit]).
fn server_n(du: &'static str, n: usize) -> String {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let addr = l.local_addr().expect("local_addr").to_string();
    std::thread::spawn(move || {
        for _ in 0..n {
            if let Ok((mut s, _)) = l.accept() {
                let _ = s.write_all(du.as_bytes());
            }
        }
    });
    addr
}

/// Ca 1 — nối TCP + `read` ra đúng dữ liệu server gửi; kiểm thêm nhánh **cache hit** của `--vm`.
#[test]
fn tcp_noi_va_doc() {
    let d = tam("doc");
    let addr = server_n("hi", 3);
    let src = format!(
        "use std.net\n\
         \n\
         fn doc(t: str) -> str:\n\
         \x20   let r = match net.read(t, 64, 500):\n\
         \x20       Ok(s): s\n\
         \x20       Err(e): \"lỗi: {{e}}\"\n\
         \x20   r\n\
         \n\
         fn main():\n\
         \x20   match net.connect(\"{addr}\"):\n\
         \x20       Ok(t): print(doc(t))\n\
         \x20       Err(e): print(\"lỗi: {{e}}\")\n"
    );
    let (code, out, err) = ca(&d, "doc", &src);
    assert_eq!(code, 0, "stderr: {err}");
    assert_eq!(out, "hi\n", "phải đọc đúng 'hi' (stderr: {err})");

    // **Fast-path cache (ST-23)**: chạy `--vm` (có cache) **lần hai** — lần này là **cache hit**;
    // vẫn phải có host mạng, nếu quên truyền host ở nhánh cache ⇒ `VS0604`.
    let p = d.join("doc.vis");
    let hit = chay(&p, true, false, false);
    assert_eq!(hit.0, 0, "cache hit: exit {} (stderr: {})", hit.0, hit.2);
    assert_eq!(hit.1, "hi\n", "cache hit: stdout {:?}", hit.1);
    let _ = std::fs::remove_dir_all(&d);
}

/// Ca 2 — cổng đóng ⇒ `Err("… bị từ chối …")` **exit 0** (lỗi mạng là dữ liệu, không phải trap).
#[test]
fn tcp_tu_choi() {
    let d = tam("refused");
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = l.local_addr().expect("addr").to_string();
    drop(l); // đóng listener ⇒ connect bị từ chối
    let src = format!(
        "use std.net\n\
         fn main():\n\
         \x20   match net.connect(\"{addr}\"):\n\
         \x20       Ok(t): print(\"nối {{t}}\")\n\
         \x20       Err(e): print(\"lỗi: {{e}}\")\n"
    );
    let (code, out, err) = ca(&d, "refused", &src);
    assert_eq!(
        code, 0,
        "exit phải 0 (lỗi mạng là Err, không trap); stderr: {err}"
    );
    assert!(out.contains("bị từ chối"), "stdout: {out:?}");
    let _ = std::fs::remove_dir_all(&d);
}

/// Ca 3 — server ngủ 300 ms, `read(t, 64, 80)` ⇒ `Err` hết giờ; **tổng ≤ 400 ms** (không treo).
#[test]
fn tcp_het_gio_doc() {
    let d = tam("timeout");
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = l.local_addr().expect("addr").to_string();
    std::thread::spawn(move || {
        for _ in 0..2 {
            if let Ok((mut s, _)) = l.accept() {
                std::thread::sleep(std::time::Duration::from_millis(300));
                let _ = s.write_all(b"muon");
            }
        }
    });
    let src = format!(
        "use std.net\n\
         fn doc(t: str) -> str:\n\
         \x20   let r = match net.read(t, 64, 80):\n\
         \x20       Ok(s): s\n\
         \x20       Err(e): e\n\
         \x20   r\n\
         fn main():\n\
         \x20   match net.connect(\"{addr}\"):\n\
         \x20       Ok(t): print(doc(t))\n\
         \x20       Err(e): print(\"lỗi: {{e}}\")\n"
    );
    let p = ghi(&d, "timeout", &src);
    for vm in [false, true] {
        let t0 = Instant::now();
        let (code, out, err) = chay(&p, vm, true, false);
        let dt = t0.elapsed();
        assert_eq!(code, 0, "vm={vm}: stderr: {err}");
        assert!(
            out.contains("quá thời gian"),
            "vm={vm}: phải báo quá thời gian, stdout: {out:?}"
        );
        assert!(
            dt.as_millis() <= 400,
            "vm={vm}: **treo** {dt:?} — phải ≤ 400 ms"
        );
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// Ca 4 — tay (handle) lạ: `read`/`write`/`close` đều `Err` rõ, **không panic**, exit 0, hai đường
/// cùng byte.
#[test]
fn tcp_tay_la() {
    let d = tam("tay-la");
    let src = "use std.net\n\
               fn main():\n\
               \x20   print(net.read(\"C99\", 10, 100))\n\
               \x20   print(net.write(\"C99\", \"x\"))\n\
               \x20   print(net.close(\"C99\"))\n";
    let (code, out, err) = ca(&d, "tay-la", src);
    assert_eq!(code, 0, "stderr: {err}");
    assert_eq!(
        out.matches("Err(").count(),
        3,
        "cả 3 phải trả Err, stdout: {out:?}"
    );
    assert!(
        out.contains("không tồn tại") && !out.contains("panic"),
        "thông điệp phải nêu tay không tồn tại, stdout: {out:?}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// Ca 5 — **không host** ⇒ `VS0604` ở **cả hai** đường (in-process), cùng byte.
#[test]
fn tcp_khong_host() {
    let src = "use std.net\n\
               fn main():\n\
               \x20   match net.connect(\"127.0.0.1:1\"):\n\
               \x20       Ok(t): print(t)\n\
               \x20       Err(e): print(e)\n";
    let text = format!("# SPDX-License-Identifier: MIT OR Apache-2.0\n{src}");
    let p = vis_syntax::parse(&text, vis_syntax::Mode::Script);
    let lx = vis_syntax::lex(&text, vis_syntax::Mode::Script);
    let h = vis_types::lower_with(
        &p.cst,
        &lx,
        vis_types::LowerOptions {
            std_mods: vec!["net".into()],
            ..Default::default()
        },
    );
    let t = vis_types::check(&h.hir, &h.symbols, &h.types);
    assert!(
        t.is_clean(),
        "chương trình phải sạch kiểu sau khi có `std.net`: {:?}",
        t.diags
            .iter()
            .map(|d| (d.code.short, d.line, d.col, d.message.clone()))
            .collect::<Vec<_>>()
    );
    let a = vis_interp::run(&text, &h.hir, &h.symbols, &h.types, &[]);
    assert_eq!(
        a.codes(),
        ["VS0604"],
        "interp phải `VS0604` khi không có host, được {:?}",
        a.codes()
    );
    let m = vis_bytecode::compile(&h.hir, &h.symbols, &text).expect("compile");
    let b = vis_vm::run(&m, &text, &[]);
    assert_eq!(
        b.codes(),
        ["VS0604"],
        "vm phải `VS0604` khi không có host, được {:?}",
        b.codes()
    );
    assert_eq!(
        (a.stdout.clone(), a.exit, a.codes()),
        (b.stdout.clone(), b.exit, b.codes()),
        "hai đường phải **cùng byte** khi thiếu host"
    );
    assert_eq!(
        a.diagnostics.first().map(|d| d.message.clone()),
        b.diagnostics.first().map(|d| d.message.clone()),
        "cùng thông điệp `VS0604`"
    );
}

/// Ca 6 (rẻ) — `--explain-perms` ghi `NET connect 127.0.0.1:…` ra **stderr**.
#[test]
fn tcp_explain_perms() {
    let d = tam("perms");
    let addr = server_n("hi", 2);
    let src = format!(
        "use std.net\n\
         fn doc(t: str) -> str:\n\
         \x20   let r = match net.read(t, 64, 500):\n\
         \x20       Ok(s): s\n\
         \x20       Err(e): e\n\
         \x20   r\n\
         fn main():\n\
         \x20   match net.connect(\"{addr}\"):\n\
         \x20       Ok(t): print(doc(t))\n\
         \x20       Err(e): print(\"lỗi: {{e}}\")\n"
    );
    let p = ghi(&d, "perms", &src);
    for vm in [false, true] {
        let (code, out, err) = chay(&p, vm, true, true);
        assert_eq!(code, 0, "vm={vm}: stderr: {err}");
        assert_eq!(out, "hi\n", "vm={vm}: stdout: {out:?}");
        assert!(
            err.contains("NET connect 127.0.0.1"),
            "vm={vm}: stderr phải có `NET connect 127.0.0.1`, được: {err:?}"
        );
    }
    let _ = std::fs::remove_dir_all(&d);
}
