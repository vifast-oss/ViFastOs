// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-64 (VD-100/VD-105)** — `std.http.get`/`get_timeout`: **giao thức HTTP nằm trong
//! `vis-interp`** (⇒ interp ↔ VM **cùng byte**, codegen không sửa), socket thô qua `NetHost`.
//!
//! 6 ca e2e + 1 ca đường mặc định, server **loopback thật** trong test Rust; mỗi ca chạy **cả hai
//! đường** (`run` và `run --vm --no-cache`) và so **từng byte**. Ca 1 thêm **cache hit** của `--vm`.
//! Sem **không** đổi (VD-105 (7): ca net/http nằm ở Rust).

use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// **Chống flake**: ca timeout đo thời gian nên các ca trong binary này chạy **tuần tự** (chỉ giảm
/// nhiễu tài nguyên; **không** nới ngưỡng nào).
fn kho() -> std::sync::MutexGuard<'static, ()> {
    static KHO: Mutex<()> = Mutex::new(());
    KHO.lock().unwrap_or_else(|e| e.into_inner())
}

fn tam(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vis-http-{tag}-{}", std::process::id()));
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

/// Chạy **hai đường** + so byte (stdout/stderr/exit); trả kết quả interp.
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

/// Server loopback: mỗi kết nối đọc request tới `\r\n\r\n`, (tuỳ) ngủ `ngu_ms`, ghi `phan_hoi`,
/// đóng; ghi lại **mọi byte request** vào log chung. Trả `(127.0.0.1:<cổng>, log)`.
fn server(phan_hoi: Vec<u8>, so_ket_noi: usize, ngu_ms: u64) -> (String, Arc<Mutex<Vec<u8>>>) {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let addr = l.local_addr().expect("local_addr").to_string();
    let log = Arc::new(Mutex::new(Vec::new()));
    let log2 = Arc::clone(&log);
    std::thread::spawn(move || {
        for _ in 0..so_ket_noi {
            let Ok((mut s, _)) = l.accept() else { break };
            let _ = s.set_read_timeout(Some(Duration::from_millis(2_000)));
            let mut req = Vec::new();
            let mut buf = [0u8; 4096];
            while req.len() < 65_536 && !req.windows(4).any(|w| w == b"\r\n\r\n") {
                match s.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => req.extend_from_slice(&buf[..n]),
                    Err(_) => break,
                }
            }
            log2.lock().expect("log").extend_from_slice(&req);
            if ngu_ms > 0 {
                std::thread::sleep(Duration::from_millis(ngu_ms));
            }
            let _ = s.write_all(&phan_hoi);
            let _ = s.shutdown(Shutdown::Both);
        }
    });
    (addr, log)
}

/// Dựng phản hồi HTTP/1.1 với `Content-Length` **tường minh**.
fn dap_ung(status: &str, body: &[u8], cl: usize) -> Vec<u8> {
    let mut r = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {cl}\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n"
    )
    .into_bytes();
    r.extend_from_slice(body);
    r
}

/// Chương trình in **body** (Ok) hoặc **thông điệp lỗi** (Err) — không nối chuỗi.
fn chuong_trinh(url: &str) -> String {
    format!(
        "use std.http\n\
         match http.get(\"{url}\"):\n\
         \x20   Ok(s): print(s)\n\
         \x20   Err(e): print(e)\n"
    )
}

/// Ca 1 — HTTP 200: body UTF-8 **đúng từng byte**; thêm nhánh `--vm` **cache hit**.
#[test]
fn http_200_byte() {
    let _k = kho();
    let d = tam("200");
    let body = "chào 😀".as_bytes().to_vec();
    let cl = body.len();
    // 4 lần chạy: interp, vm --no-cache, vm (cache lần 1), vm (cache hit).
    let (addr, _log) = server(dap_ung("200 OK", &body, cl), 4, 0);
    let (code, out, err) = ca(&d, "ok", &chuong_trinh(&format!("http://{addr}/chao")));
    assert_eq!(code, 0, "stderr: {err}");
    assert_eq!(out, "chào 😀\n", "body phải đúng từng byte: {out:?}");
    let p = d.join("ok.vis");
    for i in 0..2 {
        let hit = chay(&p, true, false, false);
        assert_eq!(hit.0, 0, "cache lần {i}: exit {}", hit.0);
        assert_eq!(hit.1, "chào 😀\n", "cache lần {i}: stdout {:?}", hit.1);
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// Ca 2 — HTTP 404 ⇒ `Err` chứa `404`, **exit 0** (lỗi HTTP là dữ liệu `Err`).
#[test]
fn http_404() {
    let _k = kho();
    let d = tam("404");
    let (addr, _log) = server(dap_ung("404 Not Found", b"", 0), 2, 0);
    let (code, out, err) = ca(&d, "nf", &chuong_trinh(&format!("http://{addr}/khong")));
    assert_eq!(code, 0, "stderr: {err}");
    assert_eq!(out.trim_end(), "HTTP 404", "stdout: {out:?}");
    let _ = std::fs::remove_dir_all(&d);
}

/// Ca 3 — `get_timeout(url, 80)` với server ngủ 300 ms ⇒ `Err` "quá thời gian", **≤ 400 ms**/đường.
#[test]
fn http_het_gio() {
    let _k = kho();
    let d = tam("timeout");
    let (addr, _log) = server(dap_ung("200 OK", b"muon", 4), 2, 300);
    let src = format!(
        "use std.http\n\
         match http.get_timeout(\"http://{addr}/\", 80):\n\
         \x20   Ok(s): print(s)\n\
         \x20   Err(e): print(e)\n"
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

/// Ca 4 — cổng đóng ⇒ `Err` "bị từ chối", exit 0.
#[test]
fn http_tu_choi() {
    let _k = kho();
    let d = tam("refused");
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = l.local_addr().expect("addr").to_string();
    drop(l);
    let (code, out, err) = ca(&d, "refused", &chuong_trinh(&format!("http://{addr}/")));
    assert_eq!(code, 0, "exit phải 0; stderr: {err}");
    assert!(out.contains("bị từ chối"), "stdout: {out:?}");
    let _ = std::fs::remove_dir_all(&d);
}

/// Ca 5 — giới hạn: phản hồi 8 MiB + 1 ⇒ `Err` cap (không OOM); `Content-Length` lớn hơn body thật
/// ⇒ `Err` (không treo).
#[test]
fn http_gioi_han() {
    let _k = kho();
    let d = tam("cap");
    // 5a: `Content-Length` 8 MiB + 1 ⇒ server **không cần** gửi hết body; client phải từ chối sớm.
    let cl = 8 * 1024 * 1024 + 1;
    let (addr, _log) = server(dap_ung("200 OK", b"", cl), 2, 0);
    let (code, out, err) = ca(&d, "cap", &chuong_trinh(&format!("http://{addr}/to")));
    assert_eq!(code, 0, "stderr: {err}");
    assert!(
        out.contains("8 MiB") || out.contains("giới hạn"),
        "phải báo vượt giới hạn: {out:?}"
    );
    // 5b: `Content-Length: 1000` nhưng chỉ gửi 3 byte rồi đóng ⇒ `Err`, **không treo**.
    let (addr2, _log2) = server(dap_ung("200 OK", b"abc", 1000), 2, 0);
    let t0 = Instant::now();
    let (code2, out2, err2) = ca(&d, "cut", &chuong_trinh(&format!("http://{addr2}/cut")));
    let dt = t0.elapsed();
    assert_eq!(code2, 0, "stderr: {err2}");
    assert!(
        out2.contains("Content-Length") || out2.contains("đóng"),
        "phải báo thiếu body: {out2:?}"
    );
    assert!(dt.as_millis() <= 2_000, "cut: **treo** {dt:?}");
    let _ = std::fs::remove_dir_all(&d);
}

/// Ca 6 — sai kiểu `http.get(42)` ⇒ **`VS0412` tĩnh**; thiếu `use std.http` ⇒ `VS0501` lúc chạy.
/// (Thực tế harness: `vis check` in TAP + **exit 1**; tên cấp tệp **chưa** kiểm tĩnh — nợ có sẵn.)
#[test]
fn http_sai_kieu() {
    let _k = kho();
    let d = tam("type");
    let p = ghi(&d, "sai-kieu", "use std.http\nprint(http.get(42))\n");
    let o = Command::new(env!("CARGO_BIN_EXE_vis"))
        .arg("check")
        .arg(&p)
        .output()
        .expect("vis check");
    let out = String::from_utf8_lossy(&o.stdout).to_string();
    assert_eq!(o.status.code(), Some(1), "stdout: {out}");
    assert!(
        out.contains("VS0412") && out.contains("cần `str`"),
        "phải là lỗi **kiểu** VS0412: {out}"
    );
    // Thiếu `use`: `vis check` permissive cho tên cấp tệp (nợ có sẵn), nhưng `run` phải đỏ rõ.
    let p2 = ghi(&d, "thieu-use", "print(http.get(\"http://127.0.0.1:1\"))\n");
    let o2 = Command::new(env!("CARGO_BIN_EXE_vis"))
        .arg("run")
        .arg(&p2)
        .output()
        .expect("vis run");
    let err2 = String::from_utf8_lossy(&o2.stderr).to_string();
    assert_eq!(o2.status.code(), Some(1), "stderr: {err2}");
    assert!(
        err2.contains("VS0501") && err2.contains("http"),
        "thiếu `use` phải báo tên `http` chưa hỗ trợ (VS0501): {err2}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// Ca 7 — đường mặc định `/`, request đúng dạng + `Host` + `Connection: close`; `--explain-perms`
/// in `NET http GET 127.0.0.1:…`.
#[test]
fn http_duong_mac_dinh() {
    let _k = kho();
    let d = tam("path");
    let (addr, log) = server(dap_ung("200 OK", b"ok", 2), 2, 0);
    let src = format!(
        "use std.http\n\
         match http.get(\"http://{addr}\"):\n\
         \x20   Ok(s): print(s)\n\
         \x20   Err(e): print(e)\n"
    );
    let p = ghi(&d, "mac-dinh", &src);
    let a = chay(&p, false, false, true);
    let b = chay(&p, true, true, true);
    assert_eq!(a.0, 0, "interp: {:?}", a.2);
    assert_eq!(a.1, "ok\n");
    assert_eq!(a, b, "hai đường phải cùng byte (cả stderr)");
    assert!(
        a.2.contains("NET http GET 127.0.0.1"),
        "stderr phải có `NET http GET 127.0.0.1`: {:?}",
        a.2
    );
    let req = String::from_utf8_lossy(&log.lock().expect("log")).to_string();
    assert!(
        req.starts_with("GET / HTTP/1.1\r\n"),
        "dòng request đầu: {:?}",
        req.lines().next()
    );
    assert!(
        req.contains(&format!("Host: {addr}\r\n")),
        "thiếu Host: {req:?}"
    );
    assert!(
        req.contains("Connection: close\r\n"),
        "thiếu Connection: close: {req:?}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// Ca 8 — **path + query**: `http://host/a/b?x=1&y=2` ⇒ request line đúng
/// `GET /a/b?x=1&y=2 HTTP/1.1`; body server trả về nguyên vẹn.
#[test]
fn http_path_query() {
    let _k = kho();
    let d = tam("path-query");
    let (addr, log) = server(dap_ung("200 OK", b"ok", 2), 2, 0);
    let (code, out, err) = ca(
        &d,
        "path-query",
        &chuong_trinh(&format!("http://{addr}/a/b?x=1&y=2")),
    );
    assert_eq!(code, 0, "stderr: {err}");
    assert_eq!(out, "ok\n", "stdout: {out:?}");
    let req = String::from_utf8_lossy(&log.lock().expect("log")).to_string();
    assert!(
        req.starts_with("GET /a/b?x=1&y=2 HTTP/1.1\r\n"),
        "request line: {:?}",
        req.lines().next()
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// Ca 9 — body **rỗng**: `204 No Content` + `Content-Length: 0` ⇒ `Ok("")`, không lỗi.
#[test]
fn http_body_rong() {
    let _k = kho();
    let d = tam("rong");
    let (addr, _log) = server(dap_ung("204 No Content", b"", 0), 2, 0);
    let src = format!(
        "use std.http\n\
         match http.get(\"http://{addr}/rong\"):\n\
         \x20   Ok(s): print(\"({{s}})\")\n\
         \x20   Err(e): print(e)\n"
    );
    let (code, out, err) = ca(&d, "rong", &src);
    assert_eq!(code, 0, "stderr: {err}");
    assert_eq!(out, "()\n", "body rỗng phải `Ok(\"\")`, stdout: {out:?}");
    let _ = std::fs::remove_dir_all(&d);
}
