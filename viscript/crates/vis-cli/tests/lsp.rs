// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-67 (VD-102)** — `vis lsp --stdio`: khung `Content-Length`, JSON-RPC 2.0, dispatch
//! `initialize`/`initialized`/`shutdown`/`exit`. **7 snapshot nguyên văn** từng frame (không parse
//! lại) — mỗi ca kiểm luôn `Content-Length` bằng `body.len()`. stdout phải **tuyệt đối sạch**
//! (chỉ frame), stderr rỗng ở đường thành công. Không thêm ca `test/sem` (VD-105 (7)).

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Đóng một frame LSP: header + body (byte).
fn khung(body: &str) -> String {
    format!("Content-Length: {}\r\n\r\n{body}", body.len())
}

/// Chạy `vis lsp --stdio`, bơm toàn bộ `dau_vao` rồi đóng stdin; **deadline 5 s** rồi kill.
fn chay(dau_vao: &str) -> (i32, String, String) {
    let mut con = Command::new(env!("CARGO_BIN_EXE_vis"))
        .args(["lsp", "--stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn `vis lsp`");
    con.stdin
        .take()
        .expect("stdin")
        .write_all(dau_vao.as_bytes())
        .expect("bơm stdin");
    let han = Instant::now() + Duration::from_secs(5);
    loop {
        if con.try_wait().expect("try_wait").is_some() {
            break;
        }
        if Instant::now() > han {
            let _ = con.kill();
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let o = con.wait_with_output().expect("đọc output");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).to_string(),
        String::from_utf8_lossy(&o.stderr).to_string(),
    )
}

/// Snapshot body phản hồi `initialize` (id 1) — thứ tự khoá **cố định** (không version/rootUri).
const INIT: &str = r#"{"jsonrpc":"2.0","id":1,"result":{"capabilities":{"textDocumentSync":1,"positionEncoding":"utf-16","hoverProvider":true,"completionProvider":{"triggerCharacters":["."]}},"serverInfo":{"name":"vis"}}}"#;
const SHUTDOWN3: &str = r#"{"jsonrpc":"2.0","id":3,"result":null}"#;
const LOI_32601: &str = r#"{"jsonrpc":"2.0","id":2,"error":{"code":-32601,"message":"method không tồn tại","data":"foo/bar"}}"#;
const LOI_32002: &str =
    r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32002,"message":"chưa `initialize`","data":null}}"#;
const LOI_32700: &str = r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"JSON không hợp lệ","data":"JSON không hợp lệ ở byte 1: khoá object phải là chuỗi"}}"#;

fn req(id: i64, method: &str) -> String {
    format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"{method}"}}"#)
}

/// Ca 1 — `initialize` ⇒ đúng **một** frame snapshot; EOF ⇒ exit 0.
#[test]
fn lsp_khoi_tao() {
    let (code, out, err) = chay(&khung(&req(1, "initialize")));
    assert_eq!(code, 0, "stderr: {err}");
    assert_eq!(err, "", "stderr phải rỗng");
    assert_eq!(out, khung(INIT), "frame initialize");
}

/// Ca 2 — `initialized` là **notification** ⇒ **không** frame; shutdown(id 3) + exit ⇒ 0.
#[test]
fn lsp_initialized_khong_frame() {
    let vao = format!(
        "{}{}{}{}",
        khung(&req(1, "initialize")),
        khung(r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#),
        khung(&req(3, "shutdown")),
        khung(r#"{"jsonrpc":"2.0","method":"exit"}"#),
    );
    let (code, out, err) = chay(&vao);
    assert_eq!(code, 0, "stderr: {err}");
    assert_eq!(err, "");
    assert_eq!(
        out,
        format!("{}{}", khung(INIT), khung(SHUTDOWN3)),
        "initialized KHÔNG được sinh frame"
    );
}

/// Ca 3 — `shutdown` ⇒ `result:null`; `exit` sau shutdown ⇒ **0**.
#[test]
fn lsp_shutdown_exit_0() {
    let vao = format!(
        "{}{}{}",
        khung(&req(1, "initialize")),
        khung(&req(3, "shutdown")),
        khung(r#"{"jsonrpc":"2.0","method":"exit"}"#),
    );
    let (code, out, err) = chay(&vao);
    assert_eq!(code, 0, "stderr: {err}");
    assert_eq!(out, format!("{}{}", khung(INIT), khung(SHUTDOWN3)));
}

/// Ca 4 — `exit` **trước** shutdown ⇒ mã thoát **1** (vẫn chỉ frame initialize).
#[test]
fn lsp_exit_truoc_shutdown_1() {
    let vao = format!(
        "{}{}",
        khung(&req(1, "initialize")),
        khung(r#"{"jsonrpc":"2.0","method":"exit"}"#),
    );
    let (code, out, err) = chay(&vao);
    assert_eq!(code, 1, "exit trước shutdown phải 1; stderr: {err}");
    assert_eq!(out, khung(INIT));
}

/// Ca 5 — method lạ ⇒ `-32601` (id 2), data = tên method.
#[test]
fn lsp_method_la_32601() {
    let vao = format!(
        "{}{}",
        khung(&req(1, "initialize")),
        khung(&req(2, "foo/bar")),
    );
    let (code, out, err) = chay(&vao);
    assert_eq!(code, 0, "stderr: {err}");
    assert_eq!(out, format!("{}{}", khung(INIT), khung(LOI_32601)));
}

/// Ca 6 — body JSON hỏng ⇒ `-32700`, `id:null`, `data` = lỗi decode.
#[test]
fn lsp_body_hong_32700() {
    let (code, out, err) = chay(&khung("{"));
    assert_eq!(code, 0, "stderr: {err}");
    assert_eq!(err, "");
    assert_eq!(out, khung(LOI_32700));
}

/// Ca 7 — request **trước** `initialize` ⇒ `-32002`.
#[test]
fn lsp_truoc_initialize_32002() {
    let (code, out, err) = chay(&khung(&req(1, "shutdown")));
    assert_eq!(code, 0, "stderr: {err}");
    assert_eq!(out, khung(LOI_32002));
}

/// Ca 8 (rẻ) — cờ lạ ⇒ stderr + **exit 2**, không đọc stdin.
#[test]
fn lsp_co_la_2() {
    let con = Command::new(env!("CARGO_BIN_EXE_vis"))
        .args(["lsp", "--khong-co"])
        .stdin(Stdio::null())
        .output()
        .expect("chạy `vis lsp --khong-co`");
    assert_eq!(con.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&con.stderr).contains("--khong-co"),
        "stderr phải nêu cờ lạ: {:?}",
        String::from_utf8_lossy(&con.stderr)
    );
    assert_eq!(con.stdout, b"", "stdout phải sạch");
}
