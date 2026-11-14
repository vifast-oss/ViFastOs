// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-68 (VD-102 (3)(4))** — phân tích **một tệp** cho LSP + **vị trí UTF-16**.
//!
//! Đường phân tích giống `vis check`: lex → parse → lower → tyck, gộp **lex + parse + lower + tyck**
//! rồi sắp theo `(span.start, mã)`. **Giả định**: `text` trả về là `lexed.text` (đã **NFC** +
//! **CRLF→LF**), mọi `span` trỏ vào chuỗi đó — nên vị trí phải tính trên `lexed.text`, **không** trên
//! văn bản gốc.
//!
//! LSP: **0-based**, đơn vị **UTF-16 code unit**, **tab = 1 cột**. Hàm `modules::vi_tri` của CLI là
//! 1-based/scalar/tab=4 nên **không dùng** ở đây.

use vis_diag::Severity;
use vis_syntax::{Diagnostic, Mode, Span};
use vis_types::{check, lower_into, HItem, HPat, HStmt, LowerOptions, Symbols, TypeTable};

/// Một chẩn đoán đã chuẩn hoá cho LSP.
pub struct ChanDoan {
    pub span: Span,
    pub ma: &'static str,
    /// 1 = Error, 2 = Warning (LSP `DiagnosticSeverity`).
    pub severity: u8,
    pub message: String,
}

/// Một gợi ý (completion) hoặc mục tra cứu (hover).
pub struct GoiY {
    pub label: String,
    /// Mã `CompletionItemKind` của LSP (3 function, 6 variable, 11 interface, 13 enum, 14 keyword,
    /// 21 constant, 22 struct, 25 type parameter).
    pub kind: u8,
    pub detail: Option<String>,
}

/// Kết quả: văn bản **đã chuẩn hoá** (mốc của span) + chẩn đoán **đã sắp** + dữ liệu hover/completion.
pub struct KetQua {
    pub text: String,
    pub ds: Vec<ChanDoan>,
    /// Item cấp tệp (fn/record/enum/alias/const/trait) — thứ tự khai.
    pub items: Vec<GoiY>,
    /// Biến/tham số có kiểu suy ra: `(tên, kiểu đã `show`)`.
    pub bien: Vec<(String, String)>,
}

/// Chữ ký `fn` cho `detail`/hover: `fn add(a: int, b: int) -> int`.
pub(crate) fn chu_ky_fn(f: &vis_types::HFn, tbl: &TypeTable) -> String {
    let ps: Vec<String> = f
        .params
        .iter()
        .map(|p| format!("{}: {}", p.name, tbl.show(&p.ty)))
        .collect();
    format!("fn {}({}) -> {}", f.name, ps.join(", "), tbl.show(&f.ret))
}

pub(crate) fn goi_y_item(it: &HItem, tbl: &TypeTable) -> Option<GoiY> {
    match it {
        HItem::Fn(f) => Some(GoiY {
            label: f.name.clone(),
            kind: 3,
            detail: Some(chu_ky_fn(f, tbl)),
        }),
        HItem::Rec(r) => Some(GoiY {
            label: r.name.clone(),
            kind: 22,
            detail: Some(format!("type {}", r.name)),
        }),
        HItem::Enum(e) => Some(GoiY {
            label: e.name.clone(),
            kind: 13,
            detail: Some(format!("enum {}", e.name)),
        }),
        HItem::Alias(a) => Some(GoiY {
            label: a.name.clone(),
            kind: 25,
            detail: Some(format!("type {}", a.name)),
        }),
        HItem::Const(c) => Some(GoiY {
            label: c.name.clone(),
            kind: 21,
            detail: Some(format!("const {}: {}", c.name, tbl.show(&c.ty))),
        }),
        HItem::Trait(t) => Some(GoiY {
            label: t.name.clone(),
            kind: 11,
            detail: Some(format!("trait {}", t.name)),
        }),
        HItem::Extern(x) => Some(GoiY {
            label: x.name.clone(),
            kind: 3,
            detail: None,
        }),
        HItem::Impl(_) | HItem::Use(_) => None,
    }
}

/// Làm phẳng `\n\r\t` thành `\\n`, `\\r`, `\\t` (cùng quy ước `main.rs::dong_chan_doan`).
fn lam_phang(m: &str) -> String {
    m.replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn them(ds: &mut Vec<ChanDoan>, d: &Diagnostic) {
    ds.push(ChanDoan {
        span: d.span,
        ma: d.code.short,
        severity: match d.code.severity {
            Severity::Error => 1,
            Severity::Warning => 2,
        },
        message: lam_phang(&d.message),
    });
}

/// Phân tích một tệp (không module) — gộp chẩn đoán 4 tầng, sắp theo `(span.start, mã)`.
pub fn phan_tich(text: &str, mode: Mode) -> KetQua {
    let lexed = vis_syntax::lex(text, mode);
    let parsed = vis_syntax::parse_voi(&lexed.text, lexed.tokens.clone(), mode);
    let mut ds: Vec<ChanDoan> = Vec::new();
    for d in &lexed.diagnostics {
        them(&mut ds, d);
    }
    for d in &parsed.diagnostics {
        them(&mut ds, d);
    }
    let mut syms = Symbols::default();
    let mut tbl = TypeTable::new();
    let out = lower_into(
        &mut syms,
        &mut tbl,
        &parsed.cst,
        &lexed,
        LowerOptions::default(),
    );
    for d in &out.diagnostics {
        them(&mut ds, d);
    }
    let co = check(&out.hir, &syms, &tbl);
    for d in &co.diags {
        them(&mut ds, d);
    }
    ds.sort_by(|a, b| a.span.start.cmp(&b.span.start).then(a.ma.cmp(b.ma)));
    let items: Vec<GoiY> = out
        .hir
        .items
        .iter()
        .filter_map(|it| goi_y_item(it, &tbl))
        .collect();
    let mut bien: Vec<(String, String)> = Vec::new();
    for it in &out.hir.items {
        if let HItem::Fn(f) = it {
            for pr in &f.params {
                bien.push((pr.name.clone(), tbl.show(&pr.ty)));
            }
        }
    }
    for st in &out.hir.stmts {
        gom_bien(st, &tbl, &mut bien);
    }
    KetQua {
        text: lexed.text.clone(),
        ds,
        items,
        bien,
    }
}

fn ten_pat(pat: &HPat, ten: &mut Vec<String>) {
    match pat {
        HPat::Bind { name, .. } => ten.push(name.clone()),
        HPat::Tuple { items, .. } | HPat::List { items, .. } => {
            for p in items {
                ten_pat(p, ten);
            }
        }
        HPat::Ctor { args, .. } => {
            for p in args {
                ten_pat(p, ten);
            }
        }
        HPat::Record { fields, .. } => {
            for (_, p) in fields {
                if let Some(p) = p {
                    ten_pat(p, ten);
                }
            }
        }
        HPat::Or { alts, .. } => {
            for p in alts {
                ten_pat(p, ten);
            }
        }
        _ => {}
    }
}

/// Gom biến/tham số **có kiểu** từ HIR (syms của `let` là `PatBind` và không mang kiểu).
fn gom_bien(st: &HStmt, tbl: &TypeTable, out: &mut Vec<(String, String)>) {
    match st {
        HStmt::Let {
            pat, ty: Some(ty), ..
        } => {
            let mut ten = Vec::new();
            ten_pat(pat, &mut ten);
            for t in ten {
                out.push((t, tbl.show(ty)));
            }
        }
        HStmt::Var {
            name, ty: Some(ty), ..
        } => out.push((name.clone(), tbl.show(ty))),
        HStmt::If {
            then, elifs, else_, ..
        } => {
            for s in then {
                gom_bien(s, tbl, out);
            }
            for (_, b) in elifs {
                for s in b {
                    gom_bien(s, tbl, out);
                }
            }
            if let Some(b) = else_ {
                for s in b {
                    gom_bien(s, tbl, out);
                }
            }
        }
        HStmt::For { body, .. }
        | HStmt::While { body, .. }
        | HStmt::Loop { body, .. }
        | HStmt::Scope { body, .. }
        | HStmt::Block { body, .. } => {
            for s in body {
                gom_bien(s, tbl, out);
            }
        }
        HStmt::Match { arms, .. } => {
            for a in arms {
                for s in &a.body {
                    gom_bien(s, tbl, out);
                }
            }
        }
        _ => {}
    }
}
