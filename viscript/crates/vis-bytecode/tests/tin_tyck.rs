// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-54** — codegen **không tin tyck**: `compile_kiem` từ chối khi front end đã báo lỗi.
//!
//! Dữ kiện đo được (ST-54): HIR của 370 ca **không** mang nút `HExpr::Error`/kiểu `{error}`
//! (0 nút ở cả 318 ca hợp lệ) ⇒ không thể chặn bằng soi kiểu; cửa **duy nhất** là truyền
//! **chẩn đoán front end** vào `compile_kiem`. Cửa chỉ chặn `Severity::Error`; cảnh báo vẫn compile.

use std::path::{Path, PathBuf};

use vis_syntax::{Diagnostic, Span};
use vis_types::{check, lower, FnKind, HExpr, HFn, HItem, HStmt, Hir, Lit, LitKind, Symbols, Type};

const SAI_KIEU: vis_diag::Code = vis_diag::Code {
    id: 412,
    short: "VS0412",
    message_vi: "kiểu không khớp",
    message_en: "type mismatch",
    message_ja: "型が一致しません",
    severity: vis_diag::Severity::Error,
};

const CANH_BAO: vis_diag::Code = vis_diag::Code {
    id: 9999,
    short: "VS0999",
    message_vi: "cảnh báo thử",
    message_en: "test warning",
    message_ja: "テスト警告",
    severity: vis_diag::Severity::Warning,
};

fn chan(code: &'static vis_diag::Code, line: u32, col: u32, msg: &str) -> Diagnostic {
    Diagnostic {
        code,
        message: msg.to_string(),
        span: Span::new(0, 0),
        line,
        col,
    }
}

fn hir_main(body: Vec<HStmt>) -> Hir {
    Hir {
        items: vec![HItem::Fn(HFn {
            name: "main".to_string(),
            span: Span::new(0, 0),
            sym: 1,
            kind: FnKind::Fn,
            is_pub: true,
            is_async: false,
            generics: Vec::new(),
            params: Vec::new(),
            ret: Type::Unit,
            body,
        })],
        stmts: Vec::new(),
        span: Span::new(0, 0),
    }
}

fn hop_le() -> Hir {
    hir_main(vec![HStmt::Expr(HExpr::Lit {
        lit: Lit {
            kind: LitKind::Int,
            text: "1".to_string(),
        },
        ty: Type::Int,
        span: Span::new(0, 0),
    })])
}

fn sem_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test/sem")
}

fn walk(d: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(d).expect("đọc test/sem") {
        let p = e.expect("entry").path();
        if p.is_dir() {
            walk(&p, out);
        } else {
            out.push(p);
        }
    }
}

/// Ca lỗi **biên dịch** (không phải `VS05xx` lúc chạy), trả `(đường .vis/.vish, mã kỳ vọng)`.
fn ca_loi_bien_dich(ma_loc: Option<&str>) -> Vec<(PathBuf, String)> {
    let mut tat = Vec::new();
    walk(&sem_root(), &mut tat);
    let mut ra = Vec::new();
    for e in tat
        .iter()
        .filter(|p| p.extension().map(|x| x == "err").unwrap_or(false))
    {
        let txt = std::fs::read_to_string(e).expect("đọc .err");
        // Dòng đầu có thể là chú thích `#`; lấy **mã đầu tiên** bắt đầu bằng `VS`.
        let ma = txt
            .lines()
            .flat_map(|l| l.split_whitespace())
            .find(|t| t.starts_with("VS"))
            .unwrap_or("")
            .to_string();
        if ma.starts_with("VS05") {
            continue;
        }
        if let Some(m) = ma_loc {
            if ma != m {
                continue;
            }
        }
        let src = e.with_extension(if e.components().any(|c| c.as_os_str() == "vish") {
            "vish"
        } else {
            "vis"
        });
        if src.is_file() {
            ra.push((src, ma));
        }
    }
    ra
}

/// Hạ + tyck một tệp, trả HIR, symbols, text và **đủ 3 tầng chẩn đoán**.
fn phan_tich(p: &Path) -> (Hir, Symbols, String, Vec<Diagnostic>) {
    let text = std::fs::read_to_string(p).expect("đọc");
    let mode = if p.extension().map(|x| x == "vish").unwrap_or(false) {
        vis_syntax::Mode::Command
    } else {
        vis_syntax::Mode::Script
    };
    let lexed = vis_syntax::lex(&text, mode);
    let parsed = vis_syntax::parse(&text, mode);
    let h = lower(&parsed.cst, &lexed);
    let kq = check(&h.hir, &h.symbols, &h.types);
    let mut chan = Vec::new();
    chan.extend(parsed.diagnostics.iter().cloned());
    chan.extend(h.diagnostics.iter().cloned());
    chan.extend(kq.diags.iter().cloned());
    (h.hir, h.symbols, text, chan)
}

#[test]
fn chan_rong_chuong_trinh_hop_le_compile_duoc() {
    let m = vis_bytecode::compile_kiem(&hop_le(), &Symbols::default(), "print(1)", &[]);
    assert!(m.is_ok(), "chương trình hợp lệ phải compile được");
}

#[test]
fn compile_cu_giu_nguyen_api() {
    // API cũ (không kèm chẩn đoán) vẫn chạy — tương thích ngược.
    assert!(vis_bytecode::compile(&hop_le(), &Symbols::default(), "print(1)").is_ok());
}

#[test]
fn ma_dong_cot_thong_diep_duoc_giu_nguyen() {
    let d = chan(&SAI_KIEU, 7, 13, "kiểu không khớp: cần `int`");
    let err = vis_bytecode::compile_kiem(&hop_le(), &Symbols::default(), "", &[d]).unwrap_err();
    match err {
        vis_bytecode::CodegenError::ChanDoan {
            short,
            dong,
            cot,
            msg,
        } => {
            assert_eq!(short, "VS0412");
            assert_eq!(dong, 7);
            assert_eq!(cot, 13);
            assert_eq!(msg, "kiểu không khớp: cần `int`");
        }
        khac => panic!("phải là ChanDoan, được {khac:?}"),
    }
}

#[test]
fn canh_bao_khong_chan() {
    let w = chan(&CANH_BAO, 1, 1, "cảnh báo thử");
    assert!(
        vis_bytecode::compile_kiem(&hop_le(), &Symbols::default(), "", &[w]).is_ok(),
        "cửa chỉ chặn `Error`; cảnh báo vẫn compile"
    );
}

#[test]
fn canh_bao_truoc_loi_chon_loi() {
    let ds = vec![
        chan(&CANH_BAO, 1, 1, "cảnh báo"),
        chan(&SAI_KIEU, 9, 4, "lỗi thật"),
    ];
    let err = vis_bytecode::compile_kiem(&hop_le(), &Symbols::default(), "", &ds).unwrap_err();
    match err {
        vis_bytecode::CodegenError::ChanDoan {
            short,
            dong,
            cot,
            msg,
        } => {
            assert_eq!((short, dong, cot), ("VS0412", 9, 4));
            assert_eq!(
                msg, "lỗi thật",
                "phải chọn chẩn đoán Error, không phải cảnh báo"
            );
        }
        khac => panic!("phải là ChanDoan, được {khac:?}"),
    }
}

#[test]
fn nhieu_loi_chon_loi_dau_tien() {
    let ds = vec![
        chan(&SAI_KIEU, 2, 2, "lỗi 1"),
        chan(&SAI_KIEU, 3, 3, "lỗi 2"),
    ];
    let err = vis_bytecode::compile_kiem(&hop_le(), &Symbols::default(), "", &ds).unwrap_err();
    match err {
        vis_bytecode::CodegenError::ChanDoan { dong, msg, .. } => {
            assert_eq!((dong, msg.as_str()), (2, "lỗi 1"));
        }
        khac => panic!("phải là ChanDoan, được {khac:?}"),
    }
}

#[test]
fn hexpr_error_bi_tu_choi_khong_panic() {
    let h = hir_main(vec![HStmt::Expr(HExpr::Error {
        span: Span::new(0, 0),
    })]);
    let kq = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        vis_bytecode::compile_kiem(&h, &Symbols::default(), "", &[])
    }));
    let err = kq
        .expect("không được panic")
        .expect_err("phải từ chối HExpr::Error");
    assert!(
        matches!(
            err,
            vis_bytecode::CodegenError::Unsupported("error-expr", _)
        ),
        "được {err:?}"
    );
}

#[test]
fn ca_that_sai_kieu_bi_tu_choi() {
    let ca = ca_loi_bien_dich(Some("VS0412"));
    let (src, _) = ca.first().expect("phải có ca VS0412");
    let (hir, syms, text, chan_doan) = phan_tich(src);
    let dau = chan_doan
        .iter()
        .find(|d| d.code.severity == vis_diag::Severity::Error)
        .expect("có lỗi");
    let err = vis_bytecode::compile_kiem(&hir, &syms, &text, &chan_doan).unwrap_err();
    match err {
        vis_bytecode::CodegenError::ChanDoan {
            short,
            dong,
            cot,
            msg,
        } => {
            assert_eq!((short, dong, cot), (dau.code.short, dau.line, dau.col));
            assert_eq!(msg, dau.message);
        }
        khac => panic!("phải là ChanDoan, được {khac:?}"),
    }
}

#[test]
fn ca_that_sai_doi_so_bi_tu_choi() {
    let ca = ca_loi_bien_dich(Some("VS0401"));
    let (src, ma) = ca.first().expect("phải có ca VS0401");
    let (hir, syms, text, chan_doan) = phan_tich(src);
    let err = vis_bytecode::compile_kiem(&hir, &syms, &text, &chan_doan).unwrap_err();
    match err {
        vis_bytecode::CodegenError::ChanDoan { short, .. } => assert_eq!(short, ma),
        khac => panic!("phải là ChanDoan, được {khac:?}"),
    }
}

#[test]
fn ca_that_truong_khong_co_bi_tu_choi() {
    let ca = ca_loi_bien_dich(Some("VS0405"));
    let (src, ma) = ca.first().expect("phải có ca VS0405");
    let (hir, syms, text, chan_doan) = phan_tich(src);
    let err = vis_bytecode::compile_kiem(&hir, &syms, &text, &chan_doan).unwrap_err();
    match err {
        vis_bytecode::CodegenError::ChanDoan { short, .. } => assert_eq!(short, ma),
        khac => panic!("phải là ChanDoan, được {khac:?}"),
    }
}

#[test]
fn moi_ca_loi_bien_dich_khong_panic_va_bi_tu_choi() {
    let ds = ca_loi_bien_dich(None);
    // **SỬA KÈM LÝ DO (ST-56)**: 26 → **28** ca lỗi biên dịch (+`ham/ref-hai-doi-so` VS0201,
    // +`generic/hop-long-sai-kieu` VS0412).
    // **SỬA KÈM LÝ DO (ST-56b)**: **30** ca (+`ban-ghi/self-gan-truong` VS0412,
    // +`list/let-chi-so-gan` VS0412).
    // **SỬA KÈM LÝ DO (ST-57)**: **32** ca (+`trait/sieu-trait-thieu` VS0403, +`trait/derive-eq-float` VS0403).
    // **SỬA KÈM LÝ DO (ST-59)**: **34** ca (+`vish/let-ngoai-vung-khai` VS0201, +`vish/than-hai-pipeline` VS0201).
    assert_eq!(ds.len(), 34, "phải đúng 34 ca lỗi biên dịch");
    for (src, ma) in ds {
        let (hir, syms, text, chan_doan) = phan_tich(&src);
        let kq = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            vis_bytecode::compile_kiem(&hir, &syms, &text, &chan_doan)
        }));
        match kq {
            Ok(Err(vis_bytecode::CodegenError::ChanDoan { short, .. })) => {
                assert_eq!(short, ma, "{}: sai mã chẩn đoán", src.display());
            }
            Ok(Ok(_)) => panic!("{}: codegen vẫn sinh module ({ma})", src.display()),
            Ok(Err(khac)) => panic!("{}: lỗi codegen lạ: {khac}", src.display()),
            Err(_) => panic!("{}: codegen panic", src.display()),
        }
    }
}

#[test]
fn hop_le_khong_bi_chan_boi_canh_bao() {
    let ds = vec![chan(&CANH_BAO, 5, 1, "cảnh báo")];
    assert!(vis_bytecode::compile_kiem(&hop_le(), &Symbols::default(), "", &ds).is_ok());
}
