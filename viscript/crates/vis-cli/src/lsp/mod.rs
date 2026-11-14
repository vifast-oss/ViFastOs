// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-67 (VD-102)** — `vis lsp --stdio`: **khung LSP + JSON-RPC 2.0 + dispatch tối thiểu**.
//!
//! Phạm vi vòng này: `initialize` (capabilities cố định), `initialized` (notification, không frame),
//! `shutdown` (`result:null`), `exit` (0 nếu đã shutdown, 1 nếu chưa), method lạ `-32601`, body JSON
//! hỏng `-32700`, request trước `initialize` `-32002`, `exit`/EOF stdin ⇒ dừng.
//! **Chưa** có diagnostics/completion/hover (ST-68/69) — `hoverProvider:true` là khai báo trước.
//!
//! stdout **tuyệt đối sạch**: chỉ frame; mọi lỗi giao thức ghi ra stderr.

mod jsonrpc;
mod khung;

use std::io::{self, BufReader};
use std::process::ExitCode;

use vis_interp::json;
use vis_interp::value::Value;

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
