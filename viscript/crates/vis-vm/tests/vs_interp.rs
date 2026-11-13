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

/// **Danh sách ca lệch phải LUÔN RỖNG** (ST-20b): differential 276/276 (ST-27: +4 ngoài). Nếu ai thêm ca vào đây,
/// test sẽ đỏ ngay (`so_biet == 0`) — mục đích là **không** cho nợ kỹ thuật quay lại.
const KNOWN_LECH: [&str; 0] = [];

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
    // **SỬA KÈM LÝ DO (ST-27)**: 328 ca sem (309 `.vis` + 19 `.vish`, +5 ngoài).
    assert_eq!(tat_ca.len(), 328, "phải đủ 328 ca");
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
        so_biet, 0,
        "**không** còn ca lệch nào (danh sách phải rỗng, thật {})",
        so_biet
    );
    // **SỬA KÈM LÝ DO (ST-27)**: +4 ca `.vish` chương trình ngoài **không có host** (interp và VM
    // đều `VS0604`) ⇒ khớp; ca `.err` (`ngoai-thoat-ma`) không vào bộ này.
    assert_eq!(
        khop + so_biet + bo_qua,
        276,
        "276 ca (khớp {khop} + lệch {so_biet} + bỏ qua {bo_qua})"
    );
    assert_eq!(khop, 276, "phải khớp **đủ** 276 ca, thật {khop}");
}
