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

// ===== ST-68: diagnostics + vị trí UTF-16 =====

use vis_interp::value::Value;

fn s(x: &str) -> Value {
    Value::Str(x.to_string())
}
fn m(kv: Vec<(&str, Value)>) -> Value {
    Value::Map(
        kv.into_iter()
            .map(|(k, v)| (Value::Str(k.to_string()), v))
            .collect(),
    )
}
fn so(v: &Value) -> String {
    let mut o = String::new();
    vis_interp::stdlib::json(v, false, &mut o);
    o
}
fn init() -> String {
    so(&m(vec![
        ("jsonrpc", s("2.0")),
        ("id", Value::Int(1)),
        ("method", s("initialize")),
        ("params", m(vec![])),
    ]))
}
fn did_open(uri: &str, text: &str) -> String {
    so(&m(vec![
        ("jsonrpc", s("2.0")),
        ("method", s("textDocument/didOpen")),
        (
            "params",
            m(vec![(
                "textDocument",
                m(vec![
                    ("uri", s(uri)),
                    ("languageId", s("viscript")),
                    ("version", Value::Int(1)),
                    ("text", s(text)),
                ]),
            )]),
        ),
    ]))
}
fn did_change(uri: &str, text: &str) -> String {
    so(&m(vec![
        ("jsonrpc", s("2.0")),
        ("method", s("textDocument/didChange")),
        (
            "params",
            m(vec![
                (
                    "textDocument",
                    m(vec![("uri", s(uri)), ("version", Value::Int(2))]),
                ),
                (
                    "contentChanges",
                    Value::List(vec![m(vec![("text", s(text))])]),
                ),
            ]),
        ),
    ]))
}
fn did_close(uri: &str) -> String {
    so(&m(vec![
        ("jsonrpc", s("2.0")),
        ("method", s("textDocument/didClose")),
        (
            "params",
            m(vec![("textDocument", m(vec![("uri", s(uri))]))]),
        ),
    ]))
}

/// Một diagnostic LSP (thứ tự khoá cố định: range, severity, code, source, message).
fn chan(l0: i64, c0: i64, l1: i64, c1: i64, sev: i64, ma: &str, msg: &str) -> String {
    format!(
        r#"{{"range":{{"start":{{"line":{l0},"character":{c0}}},"end":{{"line":{l1},"character":{c1}}}}},"severity":{sev},"code":"{ma}","source":"vis","message":"{msg}"}}"#
    )
}

/// Body `textDocument/publishDiagnostics` (diagnostics đã ghép sẵn).
fn publish(uri: &str, ds: &str) -> String {
    format!(
        r#"{{"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{{"uri":"{uri}","diagnostics":[{ds}]}}}}"#
    )
}

const URI: &str = "file:///a.vis";

/// Ca 9 — `didOpen` lỗi kiểu ⇒ publish VS0412 `0:13–0:16` sev 1.
#[test]
fn lsp_chan_doan_did_open() {
    let vao = format!(
        "{}{}",
        khung(&init()),
        khung(&did_open(URI, "let x: int = \"a\"\n"))
    );
    let (code, out, err) = chay(&vao);
    assert_eq!(code, 0, "stderr: {err}");
    let body = publish(
        URI,
        &chan(
            0,
            13,
            0,
            16,
            1,
            "VS0412",
            "khai báo `let`: cần `int`, được `str`",
        ),
    );
    assert_eq!(out, format!("{}{}", khung(INIT), khung(&body)));
}

/// Ca 10 — `didOpen` **sạch** ⇒ publish với `diagnostics:[]`.
#[test]
fn lsp_chan_doan_sach() {
    let vao = format!(
        "{}{}",
        khung(&init()),
        khung(&did_open(URI, "let x: int = 1\n"))
    );
    let (code, out, err) = chay(&vao);
    assert_eq!(code, 0, "stderr: {err}");
    assert_eq!(out, format!("{}{}", khung(INIT), khung(&publish(URI, ""))));
}

/// Ca 11 — `didChange` (full sync) ⇒ publish VS0201 `0:8–0:8` "mong biểu thức".
#[test]
fn lsp_chan_doan_did_change() {
    let vao = format!(
        "{}{}{}",
        khung(&init()),
        khung(&did_open(URI, "let x: int = 1\n")),
        khung(&did_change(URI, "let x = \n"))
    );
    let (code, out, err) = chay(&vao);
    assert_eq!(code, 0, "stderr: {err}");
    let body = publish(
        URI,
        &chan(
            0,
            8,
            0,
            8,
            1,
            "VS0201",
            "mong biểu thức, thật `\\\\n` (NEWLINE)",
        ),
    );
    assert_eq!(
        out,
        format!(
            "{}{}{}",
            khung(INIT),
            khung(&publish(URI, "")),
            khung(&body)
        )
    );
}

/// Ca 12 — emoji: span byte `15..16` ⇒ **UTF-16** `0:13–0:14` (scalar sẽ sai `0:12`).
#[test]
fn lsp_chan_doan_emoji_utf16() {
    let vao = format!(
        "{}{}",
        khung(&init()),
        khung(&did_open(URI, "let s = \"😀\" 1\n"))
    );
    let (code, out, err) = chay(&vao);
    assert_eq!(code, 0, "stderr: {err}");
    let body = publish(
        URI,
        &chan(
            0,
            13,
            0,
            14,
            1,
            "VS0201",
            "mong xuống dòng sau câu lệnh, thật `1`",
        ),
    );
    assert_eq!(out, format!("{}{}", khung(INIT), khung(&body)));
}

/// Ca 13 — tab = **1 cột**: VS0107 `1:0–1:1` (sev 2) + VS0412 `1:14–1:17` (sev 1).
#[test]
fn lsp_chan_doan_tab_mot_cot() {
    let vao = format!(
        "{}{}",
        khung(&init()),
        khung(&did_open(URI, "fn f():\n\tlet x: int = \"a\"\n"))
    );
    let (code, out, err) = chay(&vao);
    assert_eq!(code, 0, "stderr: {err}");
    let ds = format!(
        "{},{}",
        chan(1, 0, 1, 1, 2, "VS0107", "tab (cảnh báo-lỗi; tab = 4)"),
        chan(
            1,
            14,
            1,
            17,
            1,
            "VS0412",
            "khai báo `let`: cần `int`, được `str`"
        ),
    );
    assert_eq!(out, format!("{}{}", khung(INIT), khung(&publish(URI, &ds))));
}

/// Ca 14 — URI `.vish` ⇒ chế độ Command ⇒ VS0201 "mong lệnh, thật `|`" `0:0–0:1`.
#[test]
fn lsp_chan_doan_vish() {
    let uri = "file:///a.vish";
    let vao = format!("{}{}", khung(&init()), khung(&did_open(uri, "|\n")));
    let (code, out, err) = chay(&vao);
    assert_eq!(code, 0, "stderr: {err}");
    let body = publish(uri, &chan(0, 0, 0, 1, 1, "VS0201", "mong lệnh, thật `|`"));
    assert_eq!(out, format!("{}{}", khung(INIT), khung(&body)));
}

/// Ca 15 — `didClose` ⇒ publish `diagnostics:[]` (xoá chẩn đoán cũ).
#[test]
fn lsp_chan_doan_did_close() {
    let vao = format!(
        "{}{}{}",
        khung(&init()),
        khung(&did_open(URI, "let x: int = \"a\"\n")),
        khung(&did_close(URI))
    );
    let (code, out, err) = chay(&vao);
    assert_eq!(code, 0, "stderr: {err}");
    let loi = publish(
        URI,
        &chan(
            0,
            13,
            0,
            16,
            1,
            "VS0412",
            "khai báo `let`: cần `int`, được `str`",
        ),
    );
    assert_eq!(
        out,
        format!("{}{}{}", khung(INIT), khung(&loi), khung(&publish(URI, "")))
    );
}

// ===== ST-69: completion + hover =====

fn pos(line: i64, ch: i64) -> Value {
    m(vec![
        ("line", Value::Int(line)),
        ("character", Value::Int(ch)),
    ])
}
fn req_vi_tri(id: i64, method: &str, uri: &str, line: i64, ch: i64) -> String {
    so(&m(vec![
        ("jsonrpc", s("2.0")),
        ("id", Value::Int(id)),
        ("method", s(method)),
        (
            "params",
            m(vec![
                ("textDocument", m(vec![("uri", s(uri))])),
                ("position", pos(line, ch)),
            ]),
        ),
    ]))
}
fn completion(id: i64, uri: &str, line: i64, ch: i64) -> String {
    req_vi_tri(id, "textDocument/completion", uri, line, ch)
}
fn hover(id: i64, uri: &str, line: i64, ch: i64) -> String {
    req_vi_tri(id, "textDocument/hover", uri, line, ch)
}

/// Body `publishDiagnostics` rỗng (đã biết ST-68).
fn publish_rong(uri: &str) -> String {
    publish(uri, "")
}

/// Frame **cuối** trong stdout (bỏ header) — dùng khi không cần snapshot cả chuỗi.
fn lay<'a>(kv: &'a [(Value, Value)], k: &str) -> Option<&'a Value> {
    kv.iter().find_map(|(kk, v)| match kk {
        Value::Str(s) if s == k => Some(v),
        _ => None,
    })
}

fn frame_cuoi(out: &str) -> String {
    let i = out.rfind("Content-Length: ").expect("có frame");
    let j = out[i..].find("\r\n\r\n").expect("header") + i + 4;
    out[j..].to_string()
}

/// Ca 16 — completion tiền tố `ad` ⇒ **đúng 1** item `add` (fn, kind 3, detail chữ ký).
#[test]
fn lsp_completion_tien_to_add() {
    let text = "fn add(a: int, b: int) -> int:\n    a + b\n\nad";
    let vao = format!(
        "{}{}{}",
        khung(&init()),
        khung(&did_open(URI, text)),
        khung(&completion(2, URI, 3, 2))
    );
    let (code, out, err) = chay(&vao);
    assert_eq!(code, 0, "stderr: {err}");
    let a = r#"{"label":"add","kind":3,"detail":"fn add(a: int, b: int) -> int"}"#.to_string();
    let tra = format!(r#"{{"jsonrpc":"2.0","id":2,"result":[{a}]}}"#);
    assert_eq!(
        out,
        format!(
            "{}{}{}",
            khung(INIT),
            khung(&publish_rong(URI)),
            khung(&tra)
        )
    );
}

/// Ca 17 — tiền tố `le` ⇒ `len` (builtin, 3) rồi `let` (keyword, 14), thứ tự byte.
#[test]
fn lsp_completion_tien_to_le() {
    let vao = format!(
        "{}{}{}",
        khung(&init()),
        khung(&did_open(URI, "le")),
        khung(&completion(2, URI, 0, 2))
    );
    let (code, out, err) = chay(&vao);
    assert_eq!(code, 0, "stderr: {err}");
    let tra =
        r#"{"jsonrpc":"2.0","id":2,"result":[{"label":"len","kind":3},{"label":"let","kind":14}]}"#;
    assert_eq!(
        out,
        format!("{}{}{}", khung(INIT), khung(&publish_rong(URI)), khung(tra))
    );
}

/// Ca 18 — sau `.` (tiền tố rỗng) ⇒ **toàn bộ** danh sách (chọn "tất cả", không `[]`): 50 mục
/// = 11 builtin + 40 keyword − 1 trùng (`ref`), cap 200. Lý do: LSP client tự lọc tiếp; rỗng thì
/// không có gì để gợi ý sau `.`.
#[test]
fn lsp_completion_sau_cham() {
    let vao = format!(
        "{}{}{}",
        khung(&init()),
        khung(&did_open(URI, "add.")),
        khung(&completion(2, URI, 0, 4))
    );
    let (code, out, err) = chay(&vao);
    assert_eq!(code, 0, "stderr: {err}");
    let body = frame_cuoi(&out);
    let v = vis_interp::json::decode(&body).expect("JSON frame cuối");
    let Value::Map(kv) = &v else {
        panic!("frame không phải object")
    };
    let Some(Value::List(items)) = lay(kv, "result") else {
        panic!("thiếu result: {body}")
    };
    let labels: Vec<String> = items
        .iter()
        .map(|it| {
            let Value::Map(k) = it else {
                panic!("item không phải object")
            };
            match lay(k, "label") {
                Some(Value::Str(s)) => s.clone(),
                _ => panic!("thiếu label"),
            }
        })
        .collect();
    assert_eq!(
        labels.len(),
        50,
        "11 builtin + 40 keyword − ref: {labels:?}"
    );
    assert!(labels.contains(&"print".to_string()));
    assert!(labels.contains(&"let".to_string()));
    assert_eq!(
        labels.iter().filter(|l| *l == "ref").count(),
        1,
        "khử trùng `ref`"
    );
}

/// Ca 19 — hover `add` ⇒ markdown chữ ký + range đúng từ.
#[test]
fn lsp_hover_add() {
    let text = "fn add(a: int, b: int) -> int:\n    a + b\n\nadd\n";
    let vao = format!(
        "{}{}{}",
        khung(&init()),
        khung(&did_open(URI, text)),
        khung(&hover(2, URI, 3, 1))
    );
    let (code, out, err) = chay(&vao);
    assert_eq!(code, 0, "stderr: {err}");
    let tra = "{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"contents\":{\"kind\":\"markdown\",\"value\":\"```vis\\nfn add(a: int, b: int) -> int\\n```\"},\"range\":{\"start\":{\"line\":3,\"character\":0},\"end\":{\"line\":3,\"character\":3}}}}";
    assert_eq!(
        out,
        format!("{}{}{}", khung(INIT), khung(&publish_rong(URI)), khung(tra))
    );
}

/// Ca 20 — hover tên **không tồn tại** ⇒ `result:null`.
#[test]
fn lsp_hover_khong_ton_tai() {
    let text = "let z = 1\n\nzzz\n";
    let vao = format!(
        "{}{}{}",
        khung(&init()),
        khung(&did_open(URI, text)),
        khung(&hover(2, URI, 2, 1))
    );
    let (code, out, err) = chay(&vao);
    assert_eq!(code, 0, "stderr: {err}");
    let tra = r#"{"jsonrpc":"2.0","id":2,"result":null}"#;
    assert_eq!(
        out,
        format!("{}{}{}", khung(INIT), khung(&publish_rong(URI)), khung(tra))
    );
}

/// Ca 21 — hover biến có kiểu suy ra: `xs: list[int]` ⇒ markdown chứa `list[int]`.
#[test]
fn lsp_hover_bien_kieu() {
    let text = "let xs: list[int] = [1, 2]\n\nxs\n";
    let vao = format!(
        "{}{}{}",
        khung(&init()),
        khung(&did_open(URI, text)),
        khung(&hover(2, URI, 2, 1))
    );
    let (code, out, err) = chay(&vao);
    assert_eq!(code, 0, "stderr: {err}");
    let tra = "{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"contents\":{\"kind\":\"markdown\",\"value\":\"```vis\\nxs: list[int]\\n```\"},\"range\":{\"start\":{\"line\":2,\"character\":0},\"end\":{\"line\":2,\"character\":2}}}}";
    assert_eq!(
        out,
        format!("{}{}{}", khung(INIT), khung(&publish_rong(URI)), khung(tra))
    );
}

/// Ca 22 — completion cho tệp **chưa mở** ⇒ `result:[]`, không lỗi (nội dung rỗng).
#[test]
fn lsp_completion_chua_mo() {
    let uri = "file:///chua-mo.vis";
    let vao = format!("{}{}", khung(&init()), khung(&completion(2, uri, 0, 0)));
    let (code, out, err) = chay(&vao);
    assert_eq!(code, 0, "stderr: {err}");
    let tra = r#"{"jsonrpc":"2.0","id":2,"result":[]}"#;
    assert_eq!(out, format!("{}{}", khung(INIT), khung(tra)));
}

// ---------------------------------------------------------------------------
// **ST-80 (VD-108)** — LSP **module-aware**: chẩn đoán cho tệp **có trên đĩa** phải giống
// `vis check` (nạp `use std.*` / `use ./x`), tệp **không** có trên đĩa giữ đường một tệp.
// ---------------------------------------------------------------------------

use std::path::{Path, PathBuf};

fn thu_muc(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static DEM: AtomicUsize = AtomicUsize::new(0);
    let n = DEM.fetch_add(1, Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!("vis-lsp-{tag}-{}-{}", std::process::id(), n));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("tmp");
    d
}

fn ghi(d: &Path, ten: &str, src: &str) -> PathBuf {
    let p = d.join(ten);
    std::fs::write(&p, src).expect("ghi");
    p
}

/// Bóc `(mã, dòng-0, cột-0)` từ khung `publishDiagnostics` (thứ tự khoá cố định).
fn ds_lsp(out: &str) -> Vec<(String, u32, u32)> {
    let mut kq = Vec::new();
    for phan in out.split("Content-Length: ").skip(1) {
        let Some((_, body)) = phan.split_once("\r\n\r\n") else {
            continue;
        };
        if !body.contains("textDocument/publishDiagnostics") {
            continue;
        }
        let mut i = 0;
        while let Some(j) = body[i..].find("\"code\":\"VS") {
            let c = i + j + 8;
            let ma = body[c..c + 6].to_string();
            let truoc = &body[..c];
            // Lấy **đầu** range (`start`), không phải `end`.
            let p = truoc.rfind("\"start\":").expect("start");
            let s = &truoc[p + 8..];
            let so = |k: &str| -> u32 {
                let q = s.find(k).expect("khoá vị trí");
                s[q + k.len()..]
                    .chars()
                    .take_while(|x| x.is_ascii_digit())
                    .collect::<String>()
                    .parse()
                    .expect("số")
            };
            kq.push((ma, so("\"line\":"), so("\"character\":")));
            i = c + 6;
        }
    }
    kq.sort();
    kq
}

/// `(mã, dòng-1, cột-1)` từ stderr `vis check` (`# VSxxxx d:c thông điệp`).
fn ds_check(file: &Path) -> (i32, Vec<(String, u32, u32)>) {
    let o = Command::new(env!("CARGO_BIN_EXE_vis"))
        .arg("check")
        .arg(file)
        .output()
        .expect("chạy vis check");
    let mut kq = Vec::new();
    // `vis check` in TAP: dòng chẩn đoán nằm ở **stdout** (`# VSxxxx d:c …`).
    for l in String::from_utf8_lossy(&o.stdout).lines() {
        let Some(r) = l.strip_prefix("# ") else {
            continue;
        };
        let mut p = r.split_whitespace();
        let (Some(ma), Some(dc)) = (p.next(), p.next()) else {
            continue;
        };
        if !ma.starts_with("VS") {
            continue;
        }
        let mut it = dc.split(':');
        let (d, c) = (it.next(), it.next());
        if let (Some(d), Some(c)) = (d, c) {
            if let (Ok(d), Ok(c)) = (d.parse::<u32>(), c.parse::<u32>()) {
                kq.push((ma.to_string(), d.saturating_sub(1), c.saturating_sub(1)));
            }
        }
    }
    kq.sort();
    (o.status.code().unwrap_or(-1), kq)
}

fn mo_va_lay(d: &Path, f: &Path) -> Vec<(String, u32, u32)> {
    let uri = format!("file://{}", f.display());
    let text = std::fs::read_to_string(f).unwrap_or_default();
    let vao = format!("{}{}", khung(&init()), khung(&did_open(&uri, &text)));
    let (code, out, err) = chay(&vao);
    assert_eq!(code, 0, "LSP exit: {err}");
    let _ = d;
    ds_lsp(&out)
}

const STD_OK: &str =
    "use std.str\n\nfn main():\n    let xs = str.split(\"a,b\", \",\")\n    print(xs)\n";
const STD_SAI: &str =
    "use std.str\n\nfn main():\n    let xs = str.khong_co(\"a,b\")\n    print(xs)\n";
const UTIL_OK: &str = "use ./util\n\nfn main():\n    print(f())\n";
const UTIL_THIEU: &str = "use ./khong_co\n\nfn main():\n    print(1)\n";

/// (a) `use std.str` + `str.split` hợp lệ ⇒ **không** chẩn đoán giả.
#[test]
fn lsp_module_std_hop_le_khong_bao_gia() {
    let d = thu_muc("std-ok");
    let f = ghi(&d, "a.vis", STD_OK);
    let ds = mo_va_lay(&d, &f);
    assert!(ds.is_empty(), "LSP báo giả: {ds:?}");
    let (c, dc) = ds_check(&f);
    assert_eq!(c, 0, "vis check phải xanh: {dc:?}");
    let _ = std::fs::remove_dir_all(&d);
}

/// (b) `str.khong_co` ⇒ chẩn đoán **thật** giống `vis check`.
#[test]
fn lsp_module_std_sai_giong_vis_check() {
    let d = thu_muc("std-sai");
    let f = ghi(&d, "a.vis", STD_SAI);
    let ds = mo_va_lay(&d, &f);
    let (c, dc) = ds_check(&f);
    assert_eq!(c, 1, "vis check phải đỏ: {dc:?}");
    assert!(!dc.is_empty(), "vis check phải có chẩn đoán");
    assert_eq!(ds, dc, "LSP khác vis check");
    assert_eq!(ds[0].0, "VS0301", "{ds:?}");
    let _ = std::fs::remove_dir_all(&d);
}

/// (c) `use ./util` hợp lệ ⇒ `[]`.
#[test]
fn lsp_module_util_hop_le_sach() {
    let d = thu_muc("util-ok");
    ghi(&d, "util.vis", "pub fn f() -> int:\n    return 1\n");
    let f = ghi(&d, "b.vis", UTIL_OK);
    let ds = mo_va_lay(&d, &f);
    assert!(ds.is_empty(), "LSP báo giả: {ds:?}");
    let (c, dc) = ds_check(&f);
    assert_eq!(c, 0, "vis check phải xanh: {dc:?}");
    let _ = std::fs::remove_dir_all(&d);
}

/// (d) `use ./util` **không tồn tại** ⇒ `VS0701` ở **tệp chính**, giống `vis check`.
#[test]
fn lsp_module_thieu_giong_vis_check() {
    let d = thu_muc("util-thieu");
    let f = ghi(&d, "c.vis", UTIL_THIEU);
    let ds = mo_va_lay(&d, &f);
    let (c, dc) = ds_check(&f);
    assert_eq!(c, 1, "vis check phải đỏ: {dc:?}");
    assert!(
        ds.iter().any(|x| x.0 == "VS0701"),
        "LSP thiếu VS0701: {ds:?}"
    );
    assert_eq!(ds, dc, "LSP khác vis check");
    let _ = std::fs::remove_dir_all(&d);
}

/// (e) Tệp **không** có trên đĩa ⇒ đường một tệp như trước, **không** bịa lỗi module.
#[test]
fn lsp_khong_co_tren_dia_khong_bia_loi_module() {
    let uri = "file:///tmp/vis-khong-ton-tai-st80/a.vis";
    let vao = format!(
        "{}{}",
        khung(&init()),
        khung(&did_open(
            uri,
            "use ./khong_co\n\nfn main():\n    print(1)\n"
        ))
    );
    let (code, out, err) = chay(&vao);
    assert_eq!(code, 0, "LSP exit: {err}");
    let ds = ds_lsp(&out);
    assert!(ds.is_empty(), "không được bịa lỗi module: {ds:?}");
}

/// (f) **So trực tiếp** 3 tệp mẫu: bộ `(mã, dòng, cột-1)` LSP **bằng** `vis check` (bỏ thứ tự).
#[test]
fn lsp_ba_tep_bang_vis_check() {
    let d = thu_muc("so");
    ghi(&d, "util.vis", "pub fn f() -> int:\n    return 1\n");
    let a = ghi(&d, "a.vis", STD_SAI);
    let b = ghi(
        &d,
        "b.vis",
        "use ./util\n\nfn main():\n    print(util.f())\n",
    );
    let c = ghi(&d, "c.vis", UTIL_THIEU);
    for f in [&a, &b, &c] {
        let ds = mo_va_lay(&d, f);
        let (_, dc) = ds_check(f);
        assert_eq!(ds, dc, "lệch ở {}", f.display());
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// Hiệu năng: 5 tệp nhỏ, mỗi tệp `didOpen` + `didChange` (module-aware) — **phần phân tích thêm
/// < 100 ms** so với **nền** (cùng tiến trình, không `didOpen`). Đo cô lập tổng ~9 ms; harness chạy
/// // 29 test LSP nên lấy **hiệu** để loại chi phí khởi động tiến trình. (ST-83.)
#[test]
fn lsp_module_hieu_nang_5_tep() {
    let d = thu_muc("perf");
    let mut vao = khung(&init());
    for i in 0..5 {
        let f = ghi(&d, &format!("t{i}.vis"), STD_OK);
        let uri = format!("file://{}", f.display());
        vao.push_str(&khung(&did_open(&uri, STD_OK)));
        vao.push_str(&khung(&did_change(&uri, STD_OK)));
    }
    // Nền: cùng tiến trình LSP, chỉ `initialize` (không phân tích tệp nào).
    let t = Instant::now();
    let (code, _out, err) = chay(&khung(&init()));
    let nen = t.elapsed().as_millis();
    assert_eq!(code, 0, "LSP exit: {err}");
    let t = Instant::now();
    let (code, _out, err) = chay(&vao);
    let tong = t.elapsed().as_millis();
    assert_eq!(code, 0, "LSP exit: {err}");
    let them = tong.saturating_sub(nen);
    assert!(
        them < 100,
        "phân tích 5 tệp thêm {them} ms (tổng {tong}, nền {nen}; ngưỡng 100)"
    );
    let _ = std::fs::remove_dir_all(&d);
}
