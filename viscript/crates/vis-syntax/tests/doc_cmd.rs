// SPDX-License-Identifier: MIT OR Apache-2.0
//! Hai khối ```vish-session của tài liệu phải parse **0 lỗi** ở `Mode::Command` + lossless (ST-06).
//!
//! Trích **tự động** từ `docs/viscript/ViScript-thiet-ke.md` (không chép tay); khoá đúng **2** khối
//! để thêm/bớt là đỏ.

use std::path::PathBuf;
use vis_syntax::{parse, Mode};

fn blocks() -> Vec<(usize, String)> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/viscript/ViScript-thiet-ke.md");
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("đọc {}: {e}", p.display()));
    let mut out = Vec::new();
    let mut it = text.lines().enumerate();
    while let Some((idx, line)) = it.next() {
        if line.trim_end() == "```vish-session" {
            let start = idx + 1;
            let mut body = String::new();
            for (_, l) in it.by_ref() {
                if l.trim_end() == "```" {
                    break;
                }
                body.push_str(l);
                body.push('\n');
            }
            out.push((start, body));
        }
    }
    out
}

#[test]
fn hai_khoi_vish_session_parse_sach_va_lossless() {
    let bs = blocks();
    assert_eq!(bs.len(), 2, "tài liệu phải có đúng 2 khối ```vish-session");
    for (start, body) in &bs {
        let out = parse(body, Mode::Command);
        assert!(
            out.diagnostics.is_empty(),
            "khối vish-session @{start} phải 0 lỗi, thật {:?}",
            out.diagnostics
        );
        assert_eq!(
            out.cst.render(&out.text),
            out.text,
            "khối @{start} phải lossless"
        );
        assert!(
            out.cst.sexp_cmd().starts_with('('),
            "khối @{start} phải có CmdCST sexp, thật {:?}",
            out.cst.sexp_cmd()
        );
    }
}
