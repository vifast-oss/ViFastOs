// SPDX-License-Identifier: MIT OR Apache-2.0
//! Golden **30 biểu thức** — `viscript/test/expr/golden.expr` (Agent 0 soạn, VD-34; **chỉ đọc**).
//!
//! Đây là test **đỏ trước** của ST-05 (D2-05): chạy khi parser còn là khung rỗng ⇒ đỏ 30/30
//! (log `viscript/agent/JOURNAL/st05-red-expr.log`), rồi mới hiện thực parser/CST.
//!
//! Mỗi ca kiểm ba thứ (quy ước đầu `golden.expr`):
//! 1. **sexp** của CST đúng kỳ vọng (`(op lhs rhs)`, `(paren X)`, postfix…), hoặc
//! 2. **đúng 1 lỗi** cho ca `!ERR` (VD-35: lỗi đầu rồi dừng), kèm gợi ý nếu kỳ vọng ghi chuỗi;
//! 3. **CST lossless**: `cst.render(text) == text` — kể cả khoảng trắng và chú thích.
//!
//! Lỗi `!ERR` còn phải nằm **sau** token đầu: báo oan ngay đầu tệp là bug (và là cách harness
//! phân biệt "parser chưa có" với "parser báo đúng chỗ").

use std::path::PathBuf;

use vis_syntax::{parse, Mode};

const EXPECTED_CASES: usize = 30;

#[derive(Debug)]
enum Want {
    Sexp(String),
    Err(Option<String>),
}

#[derive(Debug)]
struct Case {
    line: usize,
    src: String,
    want: Want,
}

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test/expr/golden.expr")
}

fn load_cases() -> Vec<Case> {
    let path = fixture_path();
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("không đọc được {}: {e}", path.display()));
    let mut cases = Vec::new();
    for (idx, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        let (src, rhs) = line
            .split_once(" => ")
            .unwrap_or_else(|| panic!("golden.expr:{}: thiếu ' => '", idx + 1));
        let src = src.trim().to_string();
        let rhs = rhs.trim();
        let want = if let Some(rest) = rhs.strip_prefix("!ERR") {
            let rest = rest.trim();
            let hint = rest
                .strip_prefix('"')
                .and_then(|r| r.strip_suffix('"'))
                .map(str::to_string);
            Want::Err(hint)
        } else {
            Want::Sexp(rhs.to_string())
        };
        cases.push(Case {
            line: idx + 1,
            src,
            want,
        });
    }
    cases
}

#[test]
fn golden_30_bieu_thuc_theo_vd34() {
    let cases = load_cases();
    assert_eq!(
        cases.len(),
        EXPECTED_CASES,
        "golden.expr phải có đúng {EXPECTED_CASES} ca (VD-34), không được giảm"
    );

    let mut fails: Vec<String> = Vec::new();
    for case in &cases {
        let out = parse(&case.src, Mode::Script);
        match &case.want {
            Want::Err(hint) => {
                if out.diagnostics.len() != 1 {
                    fails.push(format!(
                        "FAIL golden.expr:{} nguồn={:?}: mong đúng **1** lỗi, thật {} [{}]",
                        case.line,
                        case.src,
                        out.diagnostics.len(),
                        out.diagnostics
                            .iter()
                            .map(|d| d.code_str())
                            .collect::<Vec<_>>()
                            .join(" ")
                    ));
                    continue;
                }
                let d = &out.diagnostics[0];
                if d.span.start == 0 {
                    fails.push(format!(
                        "FAIL golden.expr:{} nguồn={:?}: lỗi báo ở token đầu (byte 0) — phải tại chỗ vi phạm",
                        case.line, case.src
                    ));
                    continue;
                }
                if let Some(h) = hint {
                    if !d.message.contains(h.as_str()) {
                        fails.push(format!(
                            "FAIL golden.expr:{} nguồn={:?}: thông điệp {:?} phải chứa {h:?}",
                            case.line, case.src, d.message
                        ));
                        continue;
                    }
                }
                continue;
            }
            Want::Sexp(want) => {
                if !out.diagnostics.is_empty() {
                    fails.push(format!(
                        "FAIL golden.expr:{} nguồn={:?}: mong parse được, thật {} lỗi [{}] — {:?}",
                        case.line,
                        case.src,
                        out.diagnostics.len(),
                        out.diagnostics
                            .iter()
                            .map(|d| d.code_str())
                            .collect::<Vec<_>>()
                            .join(" "),
                        out.diagnostics[0].message
                    ));
                    continue;
                }
                let rendered = out.cst.render(&out.text);
                if rendered != out.text {
                    fails.push(format!(
                        "FAIL golden.expr:{} nguồn={:?}: CST KHÔNG lossless\n     mong {:?}\n     thật {:?}",
                        case.line, case.src, out.text, rendered
                    ));
                    continue;
                }
                match out.cst.sexp_expr() {
                    Some(got) if got == *want => {}
                    Some(got) => fails.push(format!(
                        "FAIL golden.expr:{} nguồn={:?}\n     sexp mong {want}\n     sexp thật {got}",
                        case.line, case.src
                    )),
                    None => fails.push(format!(
                        "FAIL golden.expr:{} nguồn={:?}: không có biểu thức gốc trong CST",
                        case.line, case.src
                    )),
                }
            }
        }
    }

    if !fails.is_empty() {
        for f in &fails {
            println!("{f}");
        }
        panic!(
            "expr: {} ca ĐỎ / {} ca chạy (golden biểu thức VD-34)",
            fails.len(),
            cases.len()
        );
    }
    println!(
        "expr: {} ca XANH (30 golden biểu thức, CST lossless từng ca)",
        cases.len()
    );
}
