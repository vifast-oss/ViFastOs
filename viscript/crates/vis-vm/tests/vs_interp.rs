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

/// **Danh sách ca lệch phải LUÔN RỖNG** (ST-20b; ST-30b: so **đủ** 338 ca, kể cả ca `.err` ⇒ 312/312). Nếu ai thêm ca vào đây,
/// test sẽ đỏ ngay (`so_biet == 0`) — mục đích là **không** cho nợ kỹ thuật quay lại.
const KNOWN_LECH: [&str; 0] = [];

#[test]
fn vm_khop_interp_tren_312_ca() {
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
    // **SỬA KÈM LÝ DO (ST-28)**: 338 ca sem (319 `.vis` + 19 `.vish`, +10 `generic`).
    assert_eq!(tat_ca.len(), 338, "phải đủ 338 ca");
    let mut khop = 0;
    let mut lech: Vec<String> = Vec::new();
    let mut bo_qua = 0;
    for p in &tat_ca {
        // **SỬA KÈM LÝ DO (ST-30b)**: **không** bỏ ca `.err` nữa — chẩn đoán lúc chạy của hai đường
        // phải **trùng đủ** `(mã, dòng, cột, thông điệp)`; trước đây bỏ qua nên lọt 6 ca lệch mã
        // (`VS0503` interp ↔ `VS0501` VM).
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
    // **SỬA KÈM LÝ DO (ST-30b)**: **không** bỏ ca `.err` nữa ⇒ so **đủ 338** ca (trước 286);
    // 312 khớp + 0 lệch + 26 bỏ qua (ca có lỗi **biên dịch** `VS0x` — hai đường dùng chung front end
    // nên chẩn đoán giống nhau theo cấu trúc, không chạy được để so lúc chạy).
    assert_eq!(
        khop + so_biet + bo_qua,
        338,
        "338 ca (khớp {khop} + lệch {so_biet} + bỏ qua {bo_qua})"
    );
    assert_eq!(
        (khop, bo_qua),
        (312, 26),
        "phải khớp **đủ** 312 ca chạy được (bỏ qua 26 ca lỗi biên dịch), thật khớp {khop} bỏ qua {bo_qua}"
    );
}

/// **ST-30b**: hai đường phải cho **cùng SỐ** chẩn đoán (không chỉ cùng nội dung phần tử đầu) — quét
/// **toàn bộ** 338 ca, kể cả `.err`; ít nhất một ca có chẩn đoán để phép so không rỗng nghĩa.
#[test]
fn hai_duong_cung_so_chan_doan() {
    std::thread::Builder::new()
        .stack_size(256 * 1024 * 1024)
        .spawn(|| {
            let tat_ca = tep_sem();
            let mut so_ca = 0;
            let mut so_ca_co_chan_doan = 0;
            for p in &tat_ca {
                let text = std::fs::read_to_string(p).expect("đọc");
                let mode = if p.extension().map(|x| x == "vish").unwrap_or(false) {
                    vis_syntax::Mode::Command
                } else {
                    vis_syntax::Mode::Script
                };
                let (Some(a), Some(b)) = (chay_interp(&text, mode), chay_vm(&text, mode)) else {
                    continue;
                };
                so_ca += 1;
                assert_eq!(
                    a.diagnostics.len(),
                    b.diagnostics.len(),
                    "{}: interp {} chẩn đoán, vm {}",
                    p.display(),
                    a.diagnostics.len(),
                    b.diagnostics.len()
                );
                for (x, y) in a.diagnostics.iter().zip(b.diagnostics.iter()) {
                    assert_eq!(
                        (x.code_str(), x.line, x.col, &x.message),
                        (y.code_str(), y.line, y.col, &y.message),
                        "{}",
                        p.display()
                    );
                }
                if !a.diagnostics.is_empty() {
                    so_ca_co_chan_doan += 1;
                }
            }
            // **SỬA KÈM LÝ DO (ST-30b)**: 312 ca chạy được (26 ca lỗi biên dịch bị bỏ qua), trong đó
            // **30** ca có chẩn đoán lúc chạy (6 ca chỉ số/map + `.vish` không host + phạm vi) — mỗi ca
            // đúng **1** chẩn đoán ở **cả hai** đường.
            assert_eq!(so_ca, 312, "phải so đủ 312 ca chạy được");
            assert_eq!(
                so_ca_co_chan_doan, 30,
                "30 ca có chẩn đoán lúc chạy (hai đường cùng số)"
            );
        })
        .expect("tạo luồng")
        .join()
        .expect("luồng lỗi");
}
