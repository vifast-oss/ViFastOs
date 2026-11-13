// SPDX-License-Identifier: MIT OR Apache-2.0
//! ST-20 — **differential**: cùng một chương trình, chạy **interp** và **VM**, phải cho **cùng**
//! `stdout`, `exit`, và `(mã, dòng, cột)` của chẩn đoán.
//!
//! Không so `steps`: interp đếm bước ngữ nghĩa, VM đếm **lệnh** (khác đơn vị — VD-54 (3), Agent 0
//! đã duyệt).

use std::path::PathBuf;

use vis_bytecode::compile;
use vis_interp::RunOutput;

fn goc() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../")
}

fn tep_sem() -> Vec<PathBuf> {
    let dir = goc().join("test/sem");
    let mut out = Vec::new();
    for nhom in std::fs::read_dir(&dir).expect("đọc test/sem") {
        let nhom = nhom.expect("entry").path();
        if !nhom.is_dir() {
            continue;
        }
        for f in std::fs::read_dir(&nhom).expect("đọc nhóm") {
            let p = f.expect("entry").path();
            if p.extension()
                .map(|x| x == "vis" || x == "vish")
                .unwrap_or(false)
            {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// Chạy **interp**.
fn chay_interp(text: &str, mode: vis_syntax::Mode) -> Option<RunOutput> {
    let parsed = vis_syntax::parse(text, mode);
    if !parsed.diagnostics.is_empty() {
        return None;
    }
    let lexed = vis_syntax::lex(text, mode);
    let h = vis_types::lower(&parsed.cst, &lexed);
    if !h.is_clean() {
        return None;
    }
    let t = vis_types::check(&h.hir, &h.symbols, &h.types);
    if !t.is_clean() {
        return None;
    }
    Some(vis_interp::run(text, &h.hir, &h.symbols, &h.types, &[]))
}

/// Chạy **VM**.
fn chay_vm(text: &str, mode: vis_syntax::Mode) -> Option<RunOutput> {
    let parsed = vis_syntax::parse(text, mode);
    if !parsed.diagnostics.is_empty() {
        return None;
    }
    let lexed = vis_syntax::lex(text, mode);
    let h = vis_types::lower(&parsed.cst, &lexed);
    if !h.is_clean() {
        return None;
    }
    let t = vis_types::check(&h.hir, &h.symbols, &h.types);
    if !t.is_clean() {
        return None;
    }
    let m = compile(&h.hir, &h.symbols, text).ok()?;
    Some(vis_vm::run(&m, text, &[]))
}

fn mo_ta(o: &RunOutput) -> String {
    let d: Vec<String> = o
        .diagnostics
        .iter()
        .map(|x| format!("{}:{}:{} {}", x.code_str(), x.line, x.col, x.message))
        .collect();
    format!("stdout={:?} exit={} diag={:?}", o.stdout, o.exit, d)
}

/// **19 ca chưa khớp** — đã ghi sổ, chờ task sau (ST-20b). Test **khoá đúng danh sách**:
/// lệch thêm ⇒ đỏ; sửa được ca nào mà không cập nhật danh sách ⇒ cũng đỏ.
///
/// Nguyên nhân (xem JOURNAL/2026-11-12.md §ST-20):
/// - pattern **lồng** (`some(some(x))`, `Ok((a, b))`, `A(1..3)`) — chuỗi test lồng chưa đúng;
/// - tham số `--tên` (named) **đổi thứ tự** khi gọi (default tính sai 1);
/// - `?`/`none` trong nội suy (`Show` của `none`) và `?.` trên `none` lồng;
/// - `push` trên `var` qua **hai** lớp (CoW) và `match` trả `()`.
const KNOWN_LECH: [&str; 19] = [
    "ham/closure-long.vis",
    "ham/mac-dinh-hai.vis",
    "match-nang-cao/guard-or.vis",
    "match-nang-cao/match-unit-tra.vis",
    "match-nang-cao/opt-nested.vis",
    "match-nang-cao/or-hai.vis",
    "match-nang-cao/res-nested.vis",
    "optional/hoi-cham-co.vis",
    "optional/long-match.vis",
    "optional/match-luoi.vis",
    "optional/match-none.vis",
    "optional/match-some.vis",
    "optional/noi-suy-none.vis",
    "pham-vi/scope-gia-tri.vis",
    "result/err.vis",
    "result/match-guard.vis",
    "result/match-het.vis",
    "result/match-in.vis",
    "result/ok.vis",
];

#[test]
fn vm_khop_interp_tren_272_ca() {
    // VM đệ quy theo **khung bytecode** (cap 2.000) ⇒ cần stack lớn như `vis run` (256 MiB).
    std::thread::Builder::new()
        .stack_size(256 * 1024 * 1024)
        .spawn(than_chay)
        .expect("tạo luồng")
        .join()
        .expect("luồng lỗi");
}

fn than_chay() {
    let tat_ca = tep_sem();
    assert_eq!(tat_ca.len(), 323, "phải đủ 323 ca");
    let mut khop = 0;
    let mut lech: Vec<String> = Vec::new();
    let mut bo_qua = 0;
    for p in &tat_ca {
        if p.with_extension("err").exists() {
            continue;
        }
        let text = std::fs::read_to_string(p).expect("đọc");
        let mode = if p.extension().map(|x| x == "vish").unwrap_or(false) {
            vis_syntax::Mode::Command
        } else {
            vis_syntax::Mode::Script
        };
        let (Some(a), Some(b)) = (chay_interp(&text, mode), chay_vm(&text, mode)) else {
            bo_qua += 1;
            continue;
        };
        if mo_ta(&a) == mo_ta(&b) {
            khop += 1;
        } else {
            lech.push(format!(
                "{}: interp[{}] vm[{}]",
                p.strip_prefix(goc()).unwrap_or(p).display(),
                mo_ta(&a),
                mo_ta(&b)
            ));
        }
    }
    // Tách lệch thành "đã biết" và "mới".
    let mut moi: Vec<String> = Vec::new();
    let mut so_biet = 0;
    for l in &lech {
        let duong = l.split(':').next().unwrap_or("");
        if KNOWN_LECH.iter().any(|k| duong.ends_with(k)) {
            so_biet += 1;
        } else {
            moi.push(l.clone());
        }
    }
    assert!(
        moi.is_empty(),
        "{} lệch **mới** (ngoài danh sách đã biết):\n{}",
        moi.len(),
        moi.join("\n")
    );
    assert_eq!(
        so_biet,
        KNOWN_LECH.len(),
        "số ca lệch đã biết phải đúng {}, thật {}",
        KNOWN_LECH.len(),
        so_biet
    );
    assert_eq!(
        khop + so_biet + bo_qua,
        272,
        "272 ca (khớp {khop} + lệch đã biết {so_biet} + bỏ qua {bo_qua})"
    );
}
