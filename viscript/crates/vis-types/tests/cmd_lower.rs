// SPDX-License-Identifier: MIT OR Apache-2.0
//! ST-18 — `cmd_lower`: `.vish`/`cmd { … }` hạ xuống **cùng HIR** (VD-22 pha 2).
//!
//! Khoá: pipeline ⇒ lồng call theo **khe ống** (tham số vị trí đầu tiên khớp kiểu), `;` ⇒ nhiều
//! câu lệnh tuần tự, và **4 mã** `06xx` (VS0601..VS0604) đúng vị trí.

use vis_syntax::{lex, parse, Mode};
use vis_types::lower;

fn ha(src: &str) -> (Vec<String>, Vec<u32>) {
    let lexed = lex(src, Mode::Command);
    let out = parse(src, Mode::Command);
    let h = lower(&out.cst, &lexed);
    assert!(h.error.is_none(), "lỗi cấp API: {:?}", h.error);
    let ma: Vec<String> = h
        .diagnostics
        .iter()
        .map(|d| d.code.short.to_string())
        .collect();
    let vi_tri: Vec<u32> = h.diagnostics.iter().map(|d| d.line).collect();
    // Số câu lệnh = số dòng lệnh đã hạ (mỗi pipeline một `HStmt`).
    let so_cau = h.hir.stmts.len();
    (
        ma.iter()
            .cloned()
            .chain(std::iter::once(format!("stmts={so_cau}")))
            .collect::<Vec<_>>()
            .into_iter()
            .chain(vi_tri.iter().map(|n| format!("dòng {n}")))
            .collect(),
        vi_tri,
    )
}

fn khong_loi(src: &str) -> usize {
    let lexed = lex(src, Mode::Command);
    let out = parse(src, Mode::Command);
    let h = lower(&out.cst, &lexed);
    assert!(
        h.diagnostics.is_empty(),
        "phải sạch, được: {:?}",
        h.diagnostics
            .iter()
            .map(|d| (d.code.short, d.message.clone()))
            .collect::<Vec<_>>()
    );
    h.hir.stmts.len()
}

#[test]
fn pipeline_ba_stage_ha_thanh_ba_call_long_nhau() {
    // `echo "a\nb" | lines | take 1` ⇒ `take(lines(echo(...)), 1)` — khe ống điền `xs` của `take`.
    assert_eq!(khong_loi("echo \"a\\nb\" | lines | take 1\n"), 1);
}

#[test]
fn cham_phay_thanh_hai_cau_lenh_tuan_tu() {
    assert_eq!(khong_loi("echo \"a\"; echo \"b\"\n"), 2);
    assert_eq!(khong_loi("echo \"a\"\necho \"b\"\n"), 2);
}

#[test]
fn flag_bool_dung_default() {
    assert_eq!(khong_loi("echo \"a\" --upper\n"), 1);
}

#[test]
fn lenh_la_la_vs0601() {
    let (ma, _) = ha("khong_co \"x\"\n");
    assert!(ma.contains(&"VS0601".to_string()), "được: {ma:?}");
}

#[test]
fn sai_doi_so_la_vs0602() {
    let (ma, _) = ha("echo\n");
    assert!(ma.contains(&"VS0602".to_string()), "được: {ma:?}");
    let (ma2, _) = ha("echo \"a\" \"b\" \"c\" \"d\"\n");
    assert!(ma2.contains(&"VS0602".to_string()), "được: {ma2:?}");
}

#[test]
fn noi_khong_duoc_la_vs0603() {
    // `echo` trả `str`, `take` cần `list[str]` ⇒ không có khe ống nào khớp.
    let (ma, _) = ha("echo \"a\" | take 1\n");
    assert!(ma.contains(&"VS0603".to_string()), "được: {ma:?}");
}

#[test]
fn chay_nen_la_vs0604() {
    let (ma, _) = ha("echo \"a\" &\n");
    assert!(ma.contains(&"VS0604".to_string()), "được: {ma:?}");
}

#[test]
fn chuong_trinh_ngoai_la_vs0604() {
    let (ma, _) = ha("^git status\n");
    assert!(ma.contains(&"VS0604".to_string()), "được: {ma:?}");
}

#[test]
fn chan_doan_co_dong_cot_that() {
    let lexed = lex("khong_co \"x\"\n", Mode::Command);
    let out = parse("khong_co \"x\"\n", Mode::Command);
    let h = lower(&out.cst, &lexed);
    let d = h.diagnostics.first().expect("phải có chẩn đoán");
    assert_eq!(d.line, 1, "chẩn đoán phải ở dòng 1, được {}", d.line);
    assert!(d.col >= 1, "cột phải ≥ 1, được {}", d.col);
}
