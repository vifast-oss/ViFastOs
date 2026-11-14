// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-68 (VD-102 (3))** — dựng notification `textDocument/publishDiagnostics`.

use std::path::Path;

use vis_diag::Severity;
use vis_interp::value::Value;
use vis_syntax::Mode;

use super::jsonrpc::{m, s, thong_bao};
use super::phan_tich::{self, ChanDoan};
use super::vi_tri;

/// **ST-80**: chẩn đoán **module-aware** cho tệp **có trên đĩa** — nạp cả cây `use`, nhưng chỉ
/// giữ chẩn đoán của **tệp chính** (`rel` == tên tệp). Chẩn đoán tệp phụ **không** publish vào
/// tài liệu chính (LSP một tài liệu); lỗi thiếu module được `modules` gán cho tệp chính nên **vẫn**
/// hiện. Trả `None` nếu nạp lỗi ⇒ rơi về đường một tệp.
fn chan_doan_module(path: &Path, text: &str) -> Option<Vec<ChanDoan>> {
    let ten = path.file_name()?.to_string_lossy().to_string();
    let l = crate::modules::load_full_voi_text(path.to_str()?, text, true, true).ok()?;
    let mut ds: Vec<ChanDoan> = l
        .diags
        .iter()
        .filter(|(rel, _)| rel == &ten)
        .map(|(_, d)| ChanDoan {
            span: d.span,
            ma: d.code.short,
            severity: match d.code.severity {
                Severity::Error => 1,
                Severity::Warning => 2,
            },
            message: d
                .message
                .replace('\n', "\\n")
                .replace('\r', "\\r")
                .replace('\t', "\\t"),
        })
        .collect();
    ds.sort_by(|a, b| a.span.start.cmp(&b.span.start).then(a.ma.cmp(b.ma)));
    Some(ds)
}

/// Notification chẩn đoán; `text = None` ⇒ `diagnostics:[]` (đóng tệp).
pub fn than(uri: &str, text: Option<&str>, mode: Mode) -> Vec<u8> {
    let ds = match text {
        None => Vec::new(),
        Some(t) => match super::uri_thanh_duong(uri)
            .filter(|p| Path::new(p).is_file())
            .and_then(|p| chan_doan_module(Path::new(&p), t))
        {
            Some(ds) => ds.iter().map(|d| gia_tri(t, d)).collect(),
            None => {
                let kq = phan_tich::phan_tich(t, mode);
                kq.ds.iter().map(|d| gia_tri(&kq.text, d)).collect()
            }
        },
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
