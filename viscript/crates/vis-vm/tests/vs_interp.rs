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

/// **Danh sách ca lệch phải LUÔN RỖNG** (ST-20b; ST-35: so **đủ** 346 ca, kể cả ca `.err` ⇒ 320/320). Nếu ai thêm ca vào đây,
/// test sẽ đỏ ngay (`so_biet == 0`) — mục đích là **không** cho nợ kỹ thuật quay lại.
const KNOWN_LECH: [&str; 0] = [];

#[test]
fn vm_khop_interp_tren_320_ca() {
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
    // **SỬA KÈM LÝ DO (ST-35)**: +8 ca `generic/*` (instantiation thật) ⇒ 346 ca (327 `.vis` + 19 `.vish`).
    // **SỬA KÈM LÝ DO (ST-56)**: +6 ca nợ lõi A ⇒ **376 ca** (333 `.vis` + 43 `.vish`).
    // **SỬA KÈM LÝ DO (ST-56b)**: +6 ca phép gán ⇒ **382 ca** (339 `.vis` + 43 `.vish`).
    // **SỬA KÈM LÝ DO (ST-57)**: +6 ca `trait/` ⇒ **388 ca** (345 `.vis` + 43 `.vish`).
    // **SỬA KÈM LÝ DO (ST-59)**: +6 ca `vish/` ⇒ **394 ca** (345 `.vis` + 49 `.vish`).
    assert_eq!(tat_ca.len(), 394, "phải đủ 394 ca");
    let mut khop = 0;
    let mut lech: Vec<String> = Vec::new();
    let mut bien_dich = 0;
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
            // **ST-32**: ca lỗi **biên dịch** — không chạy được ở đường nào, nhưng **đường bytecode**
            // vẫn phải được **chạm tới**: `compile()` không panic (codegen tin tyck ⇒ có thể ra module;
            // front end mới là cổng). Đếm riêng để khoá **đủ 346**.
            let lexed = vis_syntax::lex(&text, mode);
            let parsed = vis_syntax::parse(&text, mode);
            let h = vis_types::lower(&parsed.cst, &lexed);
            let kq = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                compile(&h.hir, &h.symbols, &text)
            }));
            assert!(
                kq.is_ok(),
                "{}: `compile()` panic trên HIR lỗi",
                p.display()
            );
            bien_dich += 1;
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
    // **SỬA KÈM LÝ DO (ST-35)**: **0 ca bỏ qua** — 320 ca **chạy được** so interp ↔ VM, **26** ca lỗi
    // **biên dịch** đi qua **đường bytecode** (`compile()` không panic). Tổng **346**.
    // **SỬA KÈM LÝ DO (ST-57)**: **388** ca; **356** ca chạy được khớp + **32** ca lỗi biên dịch.
    // **SỬA KÈM LÝ DO (ST-56)**: +6 ca ⇒ **376**; **348** ca chạy được khớp + **28** ca lỗi biên dịch.
    // **SỬA KÈM LÝ DO (ST-56b)**: +6 ca ⇒ **382**; **352** ca chạy được khớp + **30** ca lỗi biên dịch.
    assert_eq!(
        khop + so_biet + bien_dich,
        394,
        "394 ca (khớp {khop} + lệch {so_biet} + lỗi biên dịch {bien_dich})"
    );
    assert_eq!(
        (khop, bien_dich),
        (360, 34),
        "360 ca chạy được khớp + 34 ca lỗi biên dịch chạm bytecode, thật {khop}/{bien_dich}"
    );
}

/// **ST-30b**: hai đường phải cho **cùng SỐ** chẩn đoán (không chỉ cùng nội dung phần tử đầu) — quét
/// **toàn bộ** 346 ca, kể cả `.err`; ít nhất một ca có chẩn đoán để phép so không rỗng nghĩa.
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
            // **SỬA KÈM LÝ DO (ST-35)**: 320 ca chạy được (26 ca lỗi biên dịch bị bỏ qua), trong đó
            // **30** ca có chẩn đoán lúc chạy (6 ca chỉ số/map + `.vish` không host + phạm vi) — mỗi ca
            // đúng **1** chẩn đoán ở **cả hai** đường.
            // **SỬA KÈM LÝ DO (ST-56)**: 344 → **348** ca chạy được; **31** ca có chẩn đoán lúc
            // chạy (+`ban-ghi/self-amp-thieu-truong` VS0501).
            // **SỬA KÈM LÝ DO (ST-56b)**: **352** ca chạy được (+4 ca xanh); chẩn đoán lúc chạy
            // vẫn **31** (2 ca `.err` mới là lỗi **biên dịch**, không tới lúc chạy).
            assert_eq!(so_ca, 360, "phải so đủ 360 ca chạy được");
            assert_eq!(
                so_ca_co_chan_doan, 32,
                "32 ca có chẩn đoán lúc chạy (hai đường cùng số)"
            );
        })
        .expect("tạo luồng")
        .join()
        .expect("luồng lỗi");
}
