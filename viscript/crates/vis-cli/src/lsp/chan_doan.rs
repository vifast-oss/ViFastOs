// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-68 (VD-102 (3))** — dựng notification `textDocument/publishDiagnostics`.

use vis_interp::value::Value;
use vis_syntax::Mode;

use super::jsonrpc::{m, s, thong_bao};
use super::phan_tich::{self, ChanDoan};
use super::vi_tri;

/// Notification chẩn đoán; `text = None` ⇒ `diagnostics:[]` (đóng tệp).
pub fn than(uri: &str, text: Option<&str>, mode: Mode) -> Vec<u8> {
    let ds = match text {
        None => Vec::new(),
        Some(t) => {
            let kq = phan_tich::phan_tich(t, mode);
            kq.ds.iter().map(|d| gia_tri(&kq.text, d)).collect()
        }
    };
    thong_bao(
        "textDocument/publishDiagnostics",
        m(vec![("uri", s(uri)), ("diagnostics", Value::List(ds))]),
    )
}

fn gia_tri(text: &str, d: &ChanDoan) -> Value {
    let ((l0, c0), (l1, c1)) = vi_tri::khoang(text, d.span);
    let pos = |line: u32, character: u32| {
        m(vec![
            ("line", Value::Int(line as i64)),
            ("character", Value::Int(character as i64)),
        ])
    };
    m(vec![
        (
            "range",
            m(vec![("start", pos(l0, c0)), ("end", pos(l1, c1))]),
        ),
        ("severity", Value::Int(d.severity as i64)),
        ("code", s(d.ma)),
        ("source", s("vis")),
        ("message", s(&d.message)),
    ])
}
