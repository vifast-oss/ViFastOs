// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-67 (VD-102)** — dựng/giải JSON-RPC 2.0 trên [`Value`] của `vis-interp`.
//!
//! **Không** thêm crate: dùng lại `vis_interp::json::decode` và `vis_interp::stdlib::json`
//! (cùng encoder với `json.encode` của ngôn ngữ ⇒ snapshot tất định, thứ tự khoá = thứ tự chèn).

use vis_interp::stdlib::json;
use vis_interp::value::Value;

/// Một cặp khoá–giá trị của `Value::Map` (giữ **thứ tự chèn** — encoder tôn trọng thứ tự này).
pub fn kv(k: &str, v: Value) -> (Value, Value) {
    (Value::Str(k.to_string()), v)
}

/// Mã hoá compact (không pretty) — đúng byte mà frame mang.
pub fn so_json(v: &Value) -> String {
    let mut s = String::new();
    json(v, false, &mut s);
    s
}

/// `id` của lỗi "không xác định được request" — JSON `null`.
pub fn id_null() -> Value {
    Value::Opt(None)
}

/// Phản hồi thành công: `{"jsonrpc":"2.0","id":<id>,"result":<result>}`.
pub fn phan_hoi(id: &Value, result: Value) -> Vec<u8> {
    let m = Value::Map(vec![
        kv("jsonrpc", Value::Str("2.0".to_string())),
        kv("id", id.clone()),
        kv("result", result),
    ]);
    so_json(&m).into_bytes()
}

/// Phản hồi lỗi: `{"jsonrpc":"2.0","id":<id>,"error":{"code":…,"message":…,"data":…}}`.
pub fn loi(id: &Value, code: i64, message: &str, data: Value) -> Vec<u8> {
    let err = Value::Map(vec![
        kv("code", Value::Int(code)),
        kv("message", Value::Str(message.to_string())),
        kv("data", data),
    ]);
    let m = Value::Map(vec![
        kv("jsonrpc", Value::Str("2.0".to_string())),
        kv("id", id.clone()),
        kv("error", err),
    ]);
    so_json(&m).into_bytes()
}

/// Lấy trường `khoa` của object JSON (map giữ thứ tự) — `None` nếu không phải map/không có khoá.
pub fn lay<'a>(kv: &'a [(Value, Value)], khoa: &str) -> Option<&'a Value> {
    kv.iter().find_map(|(k, v)| match k {
        Value::Str(s) if s == khoa => Some(v),
        _ => None,
    })
}

/// Lấy chuỗi từ một `Value` (chỉ `Value::Str`).
pub fn chuoi(v: &Value) -> Option<&str> {
    match v {
        Value::Str(s) => Some(s),
        _ => None,
    }
}
