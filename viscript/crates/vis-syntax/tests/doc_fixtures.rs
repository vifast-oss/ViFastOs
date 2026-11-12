// SPDX-License-Identifier: MIT OR Apache-2.0
//! Fixture tài liệu — `docs/viscript/*.md` (ST-05, DoD "fixture tài liệu parse").
//!
//! Trích **tự động** mọi khối ```vis từ tài liệu (không chép tay). Theo **VD-36** (Agent 0):
//! - Bộ fixture của ST-05 = **đúng 15 khối đủ thân** phải parse 0 lỗi + CST lossless;
//! - **7 khối loại trừ tường minh** (6 khối `...` + 1 khối khối 1412) — QD §7.4 cấm `...` trong
//!   fixture nên đó là ví dụ lược thân, không phải chương trình; khối 1412 hỏng vì **`Entry(trường:
//!   …)`** (dòng 1429, constructor có trường tên **không có trong §A.3**) và **`assert`** (dòng
//!   1438, không có trong §A.1/A.2) — hai lỗ hổng tài liệu ghi ở báo cáo R2, **không** tự thêm
//!   cú pháp (VD-08).
//!
//! Máy kiểm: đúng **22** khối ```vis, đúng **15** parse sạch, đúng **7** loại trừ; thêm/bớt là đỏ.
//! Khối ```vish-session là pipeline chế độ lệnh ⇒ ST-06, không tính.

use std::path::{Path, PathBuf};

use vis_syntax::{parse, Mode};

/// Khối ```vis phải parse sạch (cấp tệp hoặc thân đầy đủ).
const TOTAL_VIS_BLOCKS: usize = 22;
const FULL_BODIED: usize = 15;
const EXCLUDED_COUNT: usize = 7;

/// Khối **loại trừ tường minh** — (dòng bắt đầu ```vis trong `ViScript-thiet-ke.md`, lý do).
const EXCLUDED: [(usize, &str); EXCLUDED_COUNT] = [
    (
        305,
        "`...` ở 312-315 (`for i in 0..10: ...`) — QD §7.4 cấm `...` trong fixture, là ví dụ lược thân",
    ),
    (
        432,
        "`...` ở 436 (`fn max_by[...] -> T?: ...`) — thân lược, không phải chương trình",
    ),
    (
        445,
        "`...` ở 450 (`fn show(self) -> str: ...`) — thân lược",
    ),
    (
        610,
        "`...` ở 613 (`pub cmd ls(...): ...`) — thân lược",
    ),
    (
        662,
        "`...` ở 663 (`extern cmd git(..., ...rest: str)`) — tham số biến thiên `...` không có token trong SPEC-lex §1",
    ),
    (
        737,
        "`...` ở 739 (`fn main(...): ...`) — thân lược",
    ),
    (
        1412,
        "lỗi đầu: `Entry(path: str(e.path), size: …, modified: …)` ở 1429 — constructor có **trường tên** \
         không có trong §A.3 (lỗ hổng 1); `assert xs == [3, 2, 1]` ở 1438 cũng không có trong §A.1/A.2 \
         (lỗ hổng 2 của tài liệu) — VD-36, ghi ở báo cáo R2",
    ),
];

struct Block {
    /// Dòng (1-based) của ```vis mở khối.
    start_line: usize,
    body: String,
}

fn docs_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../docs/viscript")
}

fn md_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("không đọc được {}: {e}", dir.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .collect();
    v.sort();
    v
}

/// Trích khối ```vis (đúng ngôn ngữ `vis`) từ một tệp markdown.
fn extract_vis_blocks(path: &Path) -> Vec<Block> {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("không đọc được {}: {e}", path.display()));
    let mut blocks = Vec::new();
    let mut it = text.lines().enumerate();
    while let Some((idx, line)) = it.next() {
        if line.trim_end() == "```vis" {
            let start_line = idx + 1;
            let mut body = String::new();
            for (_, l) in it.by_ref() {
                if l.trim_end() == "```" {
                    break;
                }
                body.push_str(l);
                body.push('\n');
            }
            blocks.push(Block { start_line, body });
        }
    }
    blocks
}

fn doc_file_with_blocks() -> PathBuf {
    docs_dir().join("ViScript-thiet-ke.md")
}

#[test]
fn fixture_tai_lieu_dung_22_khoi_vis() {
    let mut total = 0usize;
    for f in md_files(&docs_dir()) {
        let blocks = extract_vis_blocks(&f);
        if !blocks.is_empty() {
            assert!(
                f.ends_with("ViScript-thiet-ke.md"),
                "khối ```vis lạ trong {}: {} khối",
                f.display(),
                blocks.len()
            );
        }
        total += blocks.len();
    }
    assert_eq!(
        total, TOTAL_VIS_BLOCKS,
        "tài liệu phải có đúng {TOTAL_VIS_BLOCKS} khối ```vis (VD-36)"
    );
    // 7 khối loại trừ phải có thật trong tài liệu (không loại trừ khối ma).
    let blocks = extract_vis_blocks(&doc_file_with_blocks());
    let starts: Vec<usize> = blocks.iter().map(|b| b.start_line).collect();
    for (line, reason) in EXCLUDED {
        assert!(
            !reason.is_empty(),
            "khối loại trừ dòng {line} phải có lý do"
        );
        assert!(
            starts.contains(&line),
            "loại trừ ghi dòng {line} nhưng không có khối ```vis nào bắt đầu ở đó"
        );
    }
}

#[test]
fn fixture_tai_lieu_15_khoi_du_than_parse_sach() {
    let blocks = extract_vis_blocks(&doc_file_with_blocks());
    assert_eq!(blocks.len(), TOTAL_VIS_BLOCKS);

    let mut ok = 0usize;
    let mut failing: Vec<(usize, String)> = Vec::new();
    for b in &blocks {
        let out = parse(&b.body, Mode::Script);
        let lossless = out.cst.render(&out.text) == out.text;
        if out.diagnostics.is_empty() && lossless {
            ok += 1;
        } else {
            let msg = if out.diagnostics.is_empty() {
                "CST KHÔNG lossless".to_string()
            } else {
                format!(
                    "{} — {:?} (dòng {}:{})",
                    out.diagnostics[0].code_str(),
                    out.diagnostics[0].message,
                    out.diagnostics[0].line,
                    out.diagnostics[0].col
                )
            };
            failing.push((b.start_line, msg));
        }
    }

    // Mỗi khối đủ thân phải sạch; mỗi khối loại trừ phải **hỏng** (đúng vai trò) và đúng 1 lỗi.
    let mut unexpected: Vec<String> = Vec::new();
    for (line, msg) in &failing {
        if !EXCLUDED.iter().any(|(l, _)| l == line) {
            unexpected.push(format!(
                "khối dòng {line} (đủ thân) KHÔNG parse sạch: {msg}"
            ));
        }
    }
    for (line, _) in EXCLUDED {
        if !failing.iter().any(|(l, _)| *l == line) {
            unexpected.push(format!(
                "khối loại trừ dòng {line} lại parse sạch — bảng loại trừ đã lệch tài liệu"
            ));
        }
    }
    assert!(
        unexpected.is_empty(),
        "fixture tài liệu sai:\n  {}",
        unexpected.join("\n  ")
    );
    assert_eq!(
        ok, FULL_BODIED,
        "phải có đúng {FULL_BODIED}/{TOTAL_VIS_BLOCKS} khối đủ thân parse sạch (VD-36); đạt {ok}"
    );
    assert_eq!(
        failing.len(),
        EXCLUDED_COUNT,
        "đúng {EXCLUDED_COUNT} khối loại trừ"
    );
}

#[test]
fn khoi_loai_tru_dung_mot_loi_dau() {
    // VD-35: lỗi đầu rồi dừng ⇒ mỗi khối loại trừ đúng **1** chẩn đoán (không báo dồn).
    let blocks = extract_vis_blocks(&doc_file_with_blocks());
    for (line, reason) in EXCLUDED {
        let b = blocks
            .iter()
            .find(|b| b.start_line == line)
            .unwrap_or_else(|| panic!("không có khối dòng {line}"));
        let out = parse(&b.body, Mode::Script);
        assert_eq!(
            out.diagnostics.len(),
            1,
            "khối loại trừ dòng {line} ({reason}) phải đúng 1 lỗi, thật {}",
            out.diagnostics.len()
        );
        assert!(
            out.cst.render(&out.text) == out.text,
            "khối loại trừ dòng {line}: CST vẫn phải lossless kể cả khi lỗi"
        );
    }
}
