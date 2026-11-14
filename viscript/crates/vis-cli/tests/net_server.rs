// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-65 (VD-100/VD-105)** — **server TCP**: `net.listen`/`local_addr`/`accept(_timeout)`/
//! `close_listen` qua `NetHost` (socket thật trong CLI). 5 ca:
//! (1) một tiến trình `vis` tự phục vụ một client Rust; (2) e2e **hai tiến trình `vis`** bàn giao cổng
//! qua **tệp** (3 vòng, mỗi vòng ≤3 s); (3) `accept_timeout` 80 ms ⇒ `Err` (≤400 ms); (4) cổng bận ⇒
//! `Err`; (5) không host ⇒ **`VS0604`** in-process cả hai đường. Ca 1 chạy cả `run` và
//! `run --vm --no-cache`; **không** thêm ca `test/sem` (VD-105 (7)).

use std::io::Read;
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// **Chống flake**: các ca dựng tiến trình/server song song làm lệch phép đo thời gian ⇒ **tuần tự
/// hoá** trong một test binary (không nới ngưỡng nào; chỉ giảm nhiễu tài nguyên).
fn kho() -> std::sync::MutexGuard<'static, ()> {
    static KHO: Mutex<()> = Mutex::new(());
    KHO.lock().unwrap_or_else(|e| e.into_inner())
}

fn tam(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vis-srv-{tag}-{}", std::process::id()));
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

fn chay(p: &Path, vm: bool, no_cache: bool) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_vis"));
    c.arg("run");
    if vm {
        c.arg("--vm");
        if no_cache {
            c.arg("--no-cache");
        }
    }
    let o = c.arg(p).output().expect("chạy `vis run`");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).to_string(),
        String::from_utf8_lossy(&o.stderr).to_string(),
    )
}

/// Chờ tệp handshake có nội dung `host:port` (deadline 5 s, poll 10 ms).
fn cho_dia_chi(p: &Path) -> String {
    let han = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(s) = std::fs::read_to_string(p) {
            let s = s.trim().to_string();
            if !s.is_empty() {
                return s;
            }
        }
        assert!(
            Instant::now() < han,
            "quá 5 s chưa thấy địa chỉ trong {}",
            p.display()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Dừng tiến trình con (nếu còn) và trả mã thoát.
fn dung(mut c: Child) -> i32 {
    for _ in 0..500 {
        if let Ok(Some(st)) = c.try_wait() {
            return st.code().unwrap_or(-1);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let _ = c.kill();
    let _ = c.wait();
    -1
}

/// Chương trình server: `listen(:0)` → ghi địa chỉ vào tệp → `accept` → ghi `pong` → đóng.
fn chuong_trinh_server(duong: &str) -> String {
    format!(
        "use std.net\n\
         use std.fs\n\
         \n\
         fn phuc_vu(duong: str) -> Result[(), str]:\n\
         \x20   let l = net.listen(\"127.0.0.1:0\")?\n\
         \x20   let a = net.local_addr(l)?\n\
         \x20   fs.write(duong, a)?\n\
         \x20   let c = net.accept(l)?\n\
         \x20   net.write(c, \"pong\")?\n\
         \x20   net.close(c)?\n\
         \x20   net.close_listen(l)\n\
         \n\
         fn main():\n\
         \x20   match phuc_vu(\"{duong}\"):\n\
         \x20       Ok(_): print(\"xong\")\n\
         \x20       Err(e): print(\"lỗi: {{e}}\")\n"
    )
}

/// Chương trình client: nối tới `host:port` đã biết (địa chỉ nhúng trong nguồn).
fn chuong_trinh_client(dia_chi: &str) -> String {
    format!(
        "use std.net\n\
         \n\
         fn lay(a: str) -> Result[str, str]:\n\
         \x20   let c = net.connect(a)?\n\
         \x20   let s = net.read(c, 64, 2000)?\n\
         \x20   let _ = net.close(c)\n\
         \x20   Ok(s)\n\
         \n\
         fn main():\n\
         \x20   match lay(\"{dia_chi}\"):\n\
         \x20       Ok(s): print(s)\n\
         \x20       Err(e): print(\"lỗi: {{e}}\")\n"
    )
}

fn nhan_pong(addr: &str) -> String {
    let mut s = TcpStream::connect(addr).expect("client Rust nối");
    let _ = s.set_read_timeout(Some(Duration::from_secs(3)));
    let mut du = String::new();
    let _ = s.read_to_string(&mut du);
    let _ = s.shutdown(Shutdown::Both);
    du
}

/// Ca 1 — **một** tiến trình `vis` phục vụ một client Rust: `listen(:0)` → `local_addr` →
/// `accept` → `write("pong")`; client Rust nhận `pong`. Chạy cả `run` và `run --vm --no-cache`.
#[test]
fn server_mot_lan() {
    let _k = kho();
    let d = tam("mot");
    for vm in [false, true] {
        let duong = d.join(format!("addr-{vm}.txt"));
        let p = ghi(
            &d,
            &format!("server-{vm}"),
            &chuong_trinh_server(duong.to_str().unwrap()),
        );
        let t0 = Instant::now();
        let con = Command::new(env!("CARGO_BIN_EXE_vis"))
            .arg("run")
            .args(if vm {
                vec!["--vm", "--no-cache"]
            } else {
                vec![]
            })
            .arg(&p)
            .spawn()
            .expect("spawn `vis run`");
        let addr = cho_dia_chi(&duong);
        let pong = nhan_pong(&addr);
        let code = dung(con);
        let dt = t0.elapsed();
        assert_eq!(
            pong, "pong",
            "vm={vm}: client Rust phải nhận `pong`, được {pong:?}"
        );
        assert_eq!(code, 0, "vm={vm}: server phải exit 0");
        assert!(dt.as_secs_f64() <= 3.0, "vm={vm}: e2e {dt:?} > 3 s");
        eprintln!("server_mot_lan vm={vm}: {} ms", dt.as_millis());
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// Ca 2 — e2e **hai tiến trình `vis`**: A (`vis run server.vis`) bàn giao cổng qua **tệp**;
/// B (`vis run client.vis`) nối + đọc `pong`. **3 vòng**, mỗi vòng ≤3 s.
#[test]
fn server_hai_tien_trinh() {
    let _k = kho();
    let d = tam("hai");
    for vong in 0..3 {
        let duong = d.join(format!("addr-{vong}.txt"));
        let a = ghi(
            &d,
            &format!("a-{vong}"),
            &chuong_trinh_server(duong.to_str().unwrap()),
        );
        let t0 = Instant::now();
        let con_a = Command::new(env!("CARGO_BIN_EXE_vis"))
            .arg("run")
            .arg(&a)
            .spawn()
            .expect("spawn server A");
        let addr = cho_dia_chi(&duong);
        let b = ghi(&d, &format!("b-{vong}"), &chuong_trinh_client(&addr));
        let (code_b, out_b, err_b) = chay(&b, vong % 2 == 1, vong % 2 == 1);
        let code_a = dung(con_a);
        let dt = t0.elapsed();
        assert_eq!(code_b, 0, "vòng {vong}: B stderr {err_b}");
        assert_eq!(out_b, "pong\n", "vòng {vong}: B stdout {out_b:?}");
        assert_eq!(code_a, 0, "vòng {vong}: A phải exit 0");
        assert!(
            dt.as_secs_f64() <= 3.0,
            "vòng {vong}: e2e {dt:?} > 3 s (đo thật {:.0} ms)",
            dt.as_secs_f64() * 1000.0
        );
        eprintln!("server_hai_tien_trinh vòng {vong}: {} ms", dt.as_millis());
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// Ca 3 — `accept_timeout(L, 80)` không ai nối ⇒ `Err` "quá thời gian", **≤400 ms**/đường.
#[test]
fn accept_het_gio() {
    let _k = kho();
    let d = tam("timeout");
    let src = "use std.net\n\
               \n\
               fn thu() -> Result[str, str]:\n\
               \x20   let l = net.listen(\"127.0.0.1:0\")?\n\
               \x20   net.accept_timeout(l, 80)\n\
               \n\
               fn main():\n\
               \x20   match thu():\n\
               \x20       Ok(c): print(\"có khách {c}\")\n\
               \x20       Err(e): print(e)\n";
    let p = ghi(&d, "timeout", src);
    for vm in [false, true] {
        let t0 = Instant::now();
        let (code, out, err) = chay(&p, vm, true);
        let dt = t0.elapsed();
        assert_eq!(code, 0, "vm={vm}: stderr: {err}");
        assert!(
            out.contains("quá thời gian"),
            "vm={vm}: phải báo quá thời gian, stdout: {out:?}"
        );
        assert!(
            dt.as_millis() <= 400,
            "vm={vm}: **treo** {dt:?} — phải ≤400 ms"
        );
        eprintln!("accept_het_gio vm={vm}: {} ms", dt.as_millis());
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// Ca 4 — `listen` trên cổng **đã bận** ⇒ `Err` rõ, không panic, exit 0.
#[test]
fn cong_ban() {
    let _k = kho();
    let d = tam("ban");
    let l = TcpListener::bind("127.0.0.1:0").expect("bind giữ cổng");
    let addr = l.local_addr().expect("addr").to_string();
    let src = format!(
        "use std.net\n\
         match net.listen(\"{addr}\"):\n\
         \x20   Ok(l): print(\"không ngờ \" + l)\n\
         \x20   Err(e): print(e)\n"
    );
    // `Ok(l): print("không ngờ " + l)` — không nối chuỗi; đổi sang nội suy.
    let src = src.replace("\"không ngờ \" + l", "\"không ngờ {l}\"");
    for vm in [false, true] {
        let p = ghi(&d, &format!("ban-{vm}"), &src);
        let (code, out, err) = chay(&p, vm, true);
        assert_eq!(code, 0, "vm={vm}: stderr: {err}");
        assert!(
            out.contains("bận") || out.contains("đã được dùng"),
            "vm={vm}: phải báo cổng bận, stdout: {out:?}"
        );
    }
    drop(l);
    let _ = std::fs::remove_dir_all(&d);
}

/// Ca 5 — **không host** ⇒ `VS0604` ở **cả hai** đường (listener), cùng byte.
#[test]
fn server_khong_host() {
    let _k = kho();
    let src = "use std.net\n\
               fn main():\n\
               \x20   match net.listen(\"127.0.0.1:0\"):\n\
               \x20       Ok(l): print(l)\n\
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
        "phải sạch kiểu: {:?}",
        t.diags
            .iter()
            .map(|d| (d.code.short, d.line, d.col, d.message.clone()))
            .collect::<Vec<_>>()
    );
    let a = vis_interp::run(&text, &h.hir, &h.symbols, &h.types, &[]);
    assert_eq!(a.codes(), ["VS0604"], "interp: {:?}", a.codes());
    let m = vis_bytecode::compile(&h.hir, &h.symbols, &text).expect("compile");
    let b = vis_vm::run(&m, &text, &[]);
    assert_eq!(b.codes(), ["VS0604"], "vm: {:?}", b.codes());
    assert_eq!(
        (a.stdout.clone(), a.exit, a.codes()),
        (b.stdout.clone(), b.exit, b.codes()),
        "hai đường phải cùng byte khi thiếu host"
    );
    assert_eq!(
        a.diagnostics.first().map(|d| d.message.clone()),
        b.diagnostics.first().map(|d| d.message.clone()),
        "cùng thông điệp `VS0604`"
    );
}
