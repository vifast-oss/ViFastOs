// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-67/68/69 (VD-102)** — `vis lsp --stdio`: khung LSP + JSON-RPC 2.0 + dispatch.
//!
//! `initialize` (capabilities cố định), `initialized` (notification, không frame), `shutdown`
//! (`result:null`), `exit` (0 nếu đã shutdown, 1 nếu chưa), method lạ `-32601`, body JSON hỏng
//! `-32700`, request trước `initialize` `-32002`; `didOpen/didChange/didClose` ⇒ publishDiagnostics
//! (ST-68); `textDocument/completion` + `textDocument/hover` (ST-69, dùng văn bản đã lưu).
//!
//! stdout **tuyệt đối sạch**: chỉ frame; mọi lỗi giao thức ghi ra stderr.

mod chan_doan;
mod goi_y;
mod jsonrpc;
mod khung;
mod module;
mod phan_tich;
mod vi_tri;

use std::collections::HashMap;
use std::io::{self, BufReader};
use std::process::ExitCode;

use vis_interp::json;
use vis_interp::value::Value;
use vis_syntax::Mode;

/// Chạy server LSP tới khi `exit` hoặc **EOF stdin**. Chỉ nhận `--stdio` (mặc định cũng là stdio).
pub fn run(args: &[String]) -> ExitCode {
    for a in args {
        match a.as_str() {
            "--stdio" => {}
            khac => {
                eprintln!("vis lsp: cờ lạ `{khac}` (dùng --stdio)");
                return ExitCode::from(2);
            }
        }
    }
    let stdin = io::stdin();
    let mut doc = khung::Doc::moi(BufReader::new(stdin.lock()));
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut da_khoi_tao = false;
    let mut da_shutdown = false;
    // **ST-69**: văn bản gần nhất theo URI (didOpen/didChange) — request completion/hover đọc lại.
    let mut van_ban: HashMap<String, String> = HashMap::new();

    // Ghi frame; lỗi ghi (stdout đóng) ⇒ dừng với mã 1 (không panic).
    macro_rules! gui {
        ($body:expr) => {
            if let Err(e) = khung::ghi(&mut out, &$body) {
                eprintln!("vis lsp: {e}");
                return ExitCode::from(1);
            }
        };
    }

    loop {
        let body = match doc.doc_frame() {
            Ok(Some(b)) => b,
            Ok(None) => return ExitCode::SUCCESS, // EOF stdin — thoát sạch
            Err(e) => {
                eprintln!("vis lsp: {e}");
                return ExitCode::from(1);
            }
        };
        let text = match std::str::from_utf8(&body) {
            Ok(t) => t,
            Err(_) => {
                gui!(jsonrpc::loi(
                    &jsonrpc::id_null(),
                    -32700,
                    "JSON không hợp lệ",
                    Value::Str("body không phải UTF-8".to_string()),
                ));
                continue;
            }
        };
        let v = match json::decode(text) {
            Ok(v) => v,
            Err(e) => {
                gui!(jsonrpc::loi(
                    &jsonrpc::id_null(),
                    -32700,
                    "JSON không hợp lệ",
                    Value::Str(e),
                ));
                continue;
            }
        };
        let Value::Map(kv) = &v else {
            gui!(jsonrpc::loi(
                &jsonrpc::id_null(),
                -32600,
                "request không phải object",
                Value::Opt(None),
            ));
            continue;
        };
        let id = jsonrpc::lay(kv, "id").cloned();
        let method = jsonrpc::lay(kv, "method")
            .and_then(jsonrpc::chuoi)
            .map(str::to_string);
        let Some(method) = method else {
            // Object nhưng thiếu `method`: request hỏng (notification cũng không hợp lệ).
            if let Some(id) = id {
                gui!(jsonrpc::loi(
                    &id,
                    -32600,
                    "thiếu `method`",
                    Value::Opt(None)
                ));
            }
            continue;
        };
        match method.as_str() {
            "initialize" => {
                da_khoi_tao = true;
                if let Some(id) = id {
                    gui!(jsonrpc::phan_hoi(&id, ket_qua_khoi_tao()));
                }
            }
            // Notification: KHÔNG frame (kể cả trước initialize).
            "initialized" => {}
            // **ST-68**: văn bản mở/đổi (full sync) ⇒ tính lại chẩn đoán + `publishDiagnostics`.
            "textDocument/didOpen" => {
                if let Some((uri, text)) = jsonrpc::van_ban_mo(kv) {
                    van_ban.insert(uri.clone(), text.clone());
                    gui!(chan_doan::than(&uri, Some(&text), mode_cua(&uri)));
                }
            }
            "textDocument/didChange" => {
                if let Some((uri, text)) = jsonrpc::van_ban_doi(kv) {
                    van_ban.insert(uri.clone(), text.clone());
                    gui!(chan_doan::than(&uri, Some(&text), mode_cua(&uri)));
                }
            }
            "textDocument/didClose" => {
                if let Some(uri) = jsonrpc::uri_cua(kv) {
                    van_ban.remove(&uri);
                    gui!(chan_doan::than(&uri, None, mode_cua(&uri)));
                }
            }
            // **ST-69**: completion + hover (dùng văn bản đã lưu; chưa mở ⇒ rỗng).
            "textDocument/completion" => {
                if let Some(id) = id {
                    if !da_khoi_tao {
                        gui!(jsonrpc::loi(
                            &id,
                            -32002,
                            "chưa `initialize`",
                            Value::Opt(None)
                        ));
                    } else if let Some((uri, line, ch)) = jsonrpc::vi_tri_van_ban(kv) {
                        match van_ban.get(&uri) {
                            // Chưa mở ⇒ nội dung rỗng ⇒ `[]` (không lỗi).
                            None => gui!(jsonrpc::phan_hoi(&id, Value::List(Vec::new()))),
                            Some(text) => {
                                let kq = module::phan_tich_uri(&uri, text, mode_cua(&uri));
                                let byte = vi_tri::byte_tu_vi_tri(&kq.text, line, ch);
                                gui!(goi_y::tra_loi_completion(&kq, byte, &id));
                            }
                        }
                    }
                }
            }
            "textDocument/hover" => {
                if let Some(id) = id {
                    if !da_khoi_tao {
                        gui!(jsonrpc::loi(
                            &id,
                            -32002,
                            "chưa `initialize`",
                            Value::Opt(None)
                        ));
                    } else if let Some((uri, line, ch)) = jsonrpc::vi_tri_van_ban(kv) {
                        match van_ban.get(&uri) {
                            None => gui!(jsonrpc::phan_hoi(&id, Value::Opt(None))),
                            Some(text) => {
                                let kq = module::phan_tich_uri(&uri, text, mode_cua(&uri));
                                let byte = vi_tri::byte_tu_vi_tri(&kq.text, line, ch);
                                gui!(goi_y::tra_loi_hover(&kq, byte, &id));
                            }
                        }
                    }
                }
            }
            "shutdown" => {
                if !da_khoi_tao {
                    if let Some(id) = id {
                        gui!(jsonrpc::loi(
                            &id,
                            -32002,
                            "chưa `initialize`",
                            Value::Opt(None)
                        ));
                    }
                    continue;
                }
                da_shutdown = true;
                if let Some(id) = id {
                    gui!(jsonrpc::phan_hoi(&id, Value::Opt(None)));
                }
            }
            "exit" => {
                return if da_shutdown {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::from(1)
                };
            }
            khac => {
                if !da_khoi_tao {
                    if let Some(id) = id {
                        gui!(jsonrpc::loi(
                            &id,
                            -32002,
                            "chưa `initialize`",
                            Value::Opt(None)
                        ));
                    }
                    continue;
                }
                if let Some(id) = id {
                    gui!(jsonrpc::loi(
                        &id,
                        -32601,
                        "method không tồn tại",
                        Value::Str(khac.to_string())
                    ));
                }
            }
        }
    }
}

/// Kết quả `initialize` — **cố định** (không version/rootUri) để snapshot tất định.
fn ket_qua_khoi_tao() -> Value {
    let completion = Value::Map(vec![jsonrpc::kv(
        "triggerCharacters",
        Value::List(vec![Value::Str(".".to_string())]),
    )]);
    let capabilities = Value::Map(vec![
        jsonrpc::kv("textDocumentSync", Value::Int(1)),
        jsonrpc::kv("positionEncoding", Value::Str("utf-16".to_string())),
        jsonrpc::kv("hoverProvider", Value::Bool(true)),
        jsonrpc::kv("completionProvider", completion),
    ]);
    let server_info = Value::Map(vec![jsonrpc::kv("name", Value::Str("vis".to_string()))]);
    Value::Map(vec![
        jsonrpc::kv("capabilities", capabilities),
        jsonrpc::kv("serverInfo", server_info),
    ])
}

/// **ST-80**: `textDocument.uri` (`file://…`) → đường dẫn tệp; `None` cho URI không phải `file:`,
/// percent-decode cơ bản (`%20`, UTF-8), chấp nhận đường dẫn tuyệt đối (`file:///…`).
pub(crate) fn uri_thanh_duong(uri: &str) -> Option<String> {
    let rest = uri.strip_prefix("file://")?;
    if !rest.starts_with('/') {
        return None; // `file://host/...` — LSP một máy, không hỗ trợ host
    }
    let b = rest.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let h = std::str::from_utf8(&b[i + 1..i + 3]).ok()?;
            if let Ok(v) = u8::from_str_radix(h, 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8(out).ok()
}

/// `didOpen`: `params.textDocument.{uri,text}`.
/// `didChange` full sync: lấy `contentChanges[last].text`.
/// Chế độ phân tích theo đuôi URI (`.vish` ⇒ Command).
fn mode_cua(uri: &str) -> Mode {
    if uri.ends_with(".vish") {
        Mode::Command
    } else {
        Mode::Script
    }
}
