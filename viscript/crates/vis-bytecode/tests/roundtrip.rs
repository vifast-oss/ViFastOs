// SPDX-License-Identifier: MIT OR Apache-2.0
//! ST-19 — **round-trip** VBC1 trên **toàn bộ** bộ ngữ nghĩa (328 ca, ST-27 +5 `.vish` ngoài) + 30 golden biểu thức.
//!
//! Bất biến mỗi ca:
//! 1. codegen thành công (không `Unsupported`);
//! 2. `module.total_code() > 0` (chống xanh-giả: mỗi tệp **có** lệnh);
//! 3. `decode(encode(m)) == m` (cấu trúc);
//! 4. `encode(decode(encode(m))) == encode(m)` (byte);
//! 5. `m.is_well_formed()` (chỉ số const/hàm hợp lệ, bảng span khớp).

use std::path::{Path, PathBuf};

use vis_bytecode::{compile, decode, encode};

fn goc() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../")
}

/// Số ca **cố ý lỗi tĩnh** (có `.err`) — không sinh mã.
fn so_ca_loi(ds: &[PathBuf]) -> usize {
    ds.iter()
        .filter(|p| p.with_extension("err").exists())
        .count()
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

fn bien_dich(p: &Path) -> Result<vis_bytecode::Module, String> {
    let src = std::fs::read_to_string(p).map_err(|e| format!("đọc {}: {e}", p.display()))?;
    let mode = if p.extension().map(|x| x == "vish").unwrap_or(false) {
        vis_syntax::Mode::Command
    } else {
        vis_syntax::Mode::Script
    };
    let parsed = vis_syntax::parse(&src, mode);
    assert!(
        parsed.diagnostics.is_empty(),
        "{}: parse phải sạch: {:?}",
        p.display(),
        parsed
            .diagnostics
            .iter()
            .map(|d| (d.code_str(), d.message.clone()))
            .collect::<Vec<_>>()
    );
    let lexed = vis_syntax::lex(&src, mode);
    let h = vis_types::lower(&parsed.cst, &lexed);
    assert!(
        h.diagnostics.is_empty(),
        "{}: lower phải sạch: {:?}",
        p.display(),
        h.diagnostics
            .iter()
            .map(|d| (d.code.short, d.message.clone()))
            .collect::<Vec<_>>()
    );
    compile(&h.hir, &h.symbols, &src).map_err(|e| format!("{}: {e}", p.display()))
}

#[test]
fn roundtrip_tren_toan_bo_323_ca() {
    let tat_ca = tep_sem();
    // **SỬA KÈM LÝ DO (ST-35)**: +8 ca `generic/*` ⇒ 346 ca (327 `.vis` + 19 `.vish`).
    assert_eq!(tat_ca.len(), 363, "phải đủ 355 ca (336 .vis + 28 .vish)");
    // Ca **cố ý lỗi tĩnh** (có `.err`) không sinh mã — `sem.rs` kiểm chúng bằng chẩn đoán.
    let so_loi = so_ca_loi(&tat_ca);
    let ds: Vec<PathBuf> = tat_ca
        .iter()
        .filter(|p| !p.with_extension("err").exists())
        .cloned()
        .collect();
    assert_eq!(ds.len() + so_loi, 363);
    let mut loi = Vec::new();
    let mut tong_lenh = 0usize;
    for p in &ds {
        match bien_dich(p) {
            Err(e) => loi.push(e),
            Ok(m) => {
                assert!(
                    m.total_code() > 0,
                    "{}: module rỗng (chống xanh-giả)",
                    p.display()
                );
                tong_lenh += m.total_code();
                assert!(m.is_well_formed(), "{}: module không hợp lệ", p.display());
                let bytes = encode(&m);
                let m2 = decode(&bytes).unwrap_or_else(|e| panic!("{}: decode: {e}", p.display()));
                assert_eq!(m2, m, "{}: decode(encode) ≠ module", p.display());
                assert_eq!(
                    encode(&m2),
                    bytes,
                    "{}: encode(decode(encode)) ≠ encode",
                    p.display()
                );
            }
        }
    }
    assert!(loi.is_empty(), "ca không biên dịch được: {loi:#?}");
    assert!(
        tong_lenh > 278,
        "tổng số lệnh phải > số ca, thật {tong_lennh}",
        tong_lennh = tong_lenh
    );
}

#[test]
fn roundtrip_30_golden_bieu_thuc() {
    let p = goc().join("test/expr/golden.expr");
    let text = std::fs::read_to_string(&p).expect("đọc golden.expr");
    // Chỉ ca **hợp lệ** (vế phải là sexp `(`); các ca cố ý lỗi (`!VS…`, `paren`) không sinh mã.
    let dong: Vec<&str> = text
        .lines()
        .filter(|l| !l.trim_start().starts_with("//") && !l.trim().is_empty())
        .filter(|l| {
            let rhs = l.split(" => ").nth(1).unwrap_or("");
            rhs.starts_with('(')
        })
        .collect();
    assert!(dong.len() >= 20, "đọc được {} golden hợp lệ", dong.len());
    let mut ok = 0;
    for (i, l) in dong.iter().enumerate() {
        let expr = l.split(" => ").next().expect("có `=>`");
        let src = format!("print({expr})\n");
        let parsed = vis_syntax::parse(&src, vis_syntax::Mode::Script);
        assert!(
            parsed.diagnostics.is_empty(),
            "golden #{i} `{expr}`: {:?}",
            parsed
                .diagnostics
                .iter()
                .map(|d| d.message.clone())
                .collect::<Vec<_>>()
        );
        let lexed = vis_syntax::lex(&src, vis_syntax::Mode::Script);
        let h = vis_types::lower(&parsed.cst, &lexed);
        let m = compile(&h.hir, &h.symbols, &src)
            .unwrap_or_else(|e| panic!("golden #{i} `{expr}`: {e}"));
        assert!(m.total_code() > 0);
        let bytes = encode(&m);
        assert_eq!(decode(&bytes).expect("decode"), m);
        ok += 1;
    }
    assert_eq!(ok, dong.len());
}

#[test]
fn code_len_khong_rong_va_co_span() {
    let ds = tep_sem();
    for p in ds.iter().take(10) {
        let m = bien_dich(p).unwrap_or_else(|e| panic!("{e}"));
        for f in &m.funcs {
            assert!(!f.code.is_empty(), "{}: hàm `{}` rỗng", p.display(), f.name);
            assert_eq!(f.code.len(), f.spans.len(), "bảng span phải khớp code");
            assert!(f.max_stack >= 1, "max_stack phải ≥ 1");
        }
    }
}
