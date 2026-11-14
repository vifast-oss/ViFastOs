// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-91 (VD-110)** — thành viên theo **kiểu tĩnh** cho completion sau `.`: trường của `type` +
//! phương thức của `impl` cho kiểu đó, cùng **kiểu của biến** (tham số / `let` có nhãn / `let` suy
//! từ constructor hoặc biến có `sym.ty`). Chỉ đọc HIR + `Symbols` + `TypeTable` — **không** suy kiểu
//! tổng quát, không đoán khi không chắc.

use std::collections::HashSet;

use vis_syntax::Mode;
use vis_types::{lower_into, HExpr, HItem, HPat, HStmt, LowerOptions, Symbols, TypeTable};

use super::phan_tich::{chu_ky_fn, GoiY};

/// Dữ liệu thành viên: kiểu biến + trường/phương thức theo tên kiểu.
#[derive(Default)]
pub struct ThanhVien {
    /// `(tên biến, kiểu)` — tham số, `let` có nhãn, `let` suy được rẻ.
    pub kieu_bien: Vec<(String, String)>,
    /// `(tên kiểu, gợi ý trường)` — kind 5.
    pub truong: Vec<(String, GoiY)>,
    /// `(tên kiểu, gợi ý phương thức)` — kind 2.
    pub phuong: Vec<(String, GoiY)>,
}

fn ten_pat(pat: &HPat, out: &mut Vec<String>) {
    match pat {
        HPat::Bind { name, .. } => out.push(name.clone()),
        HPat::Tuple { items, .. } | HPat::List { items, .. } => {
            for p in items {
                ten_pat(p, out);
            }
        }
        HPat::Ctor { args, .. } => {
            for p in args {
                ten_pat(p, out);
            }
        }
        HPat::Record { fields, .. } => {
            for (_, p) in fields {
                if let Some(p) = p {
                    ten_pat(p, out);
                }
            }
        }
        HPat::Or { alts, .. } => {
            for p in alts {
                ten_pat(p, out);
            }
        }
        _ => {}
    }
}

/// Kiểu **rẻ** của biểu thức khởi tạo: `P(...)` (constructor record), biến có `sym.ty`, literal.
fn kieu_bieu_thuc(
    e: &HExpr,
    syms: &Symbols,
    tbl: &TypeTable,
    rec: &HashSet<String>,
) -> Option<String> {
    match e {
        HExpr::Call { callee, .. } => match &**callee {
            HExpr::Var { name, .. } if rec.contains(name) => Some(name.clone()),
            _ => None,
        },
        HExpr::Var { sym: Some(id), .. } => syms
            .get(*id)
            .and_then(|s| s.ty.as_ref())
            .map(|t| tbl.show(t)),
        HExpr::Lit { ty, .. } => Some(tbl.show(ty)),
        _ => None,
    }
}

fn gom_stmt(
    st: &HStmt,
    syms: &Symbols,
    tbl: &TypeTable,
    rec: &HashSet<String>,
    out: &mut Vec<(String, String)>,
) {
    match st {
        HStmt::Let { pat, ty, init, .. } => {
            let kieu = ty
                .as_ref()
                .map(|t| tbl.show(t))
                .or_else(|| kieu_bieu_thuc(init, syms, tbl, rec));
            if let Some(k) = kieu {
                let mut ten = Vec::new();
                ten_pat(pat, &mut ten);
                for t in ten {
                    out.push((t, k.clone()));
                }
            }
        }
        HStmt::Var {
            name, ty: Some(t), ..
        } => out.push((name.clone(), tbl.show(t))),
        HStmt::If {
            then, elifs, else_, ..
        } => {
            for s in then {
                gom_stmt(s, syms, tbl, rec, out);
            }
            for (_, b) in elifs {
                for s in b {
                    gom_stmt(s, syms, tbl, rec, out);
                }
            }
            if let Some(b) = else_ {
                for s in b {
                    gom_stmt(s, syms, tbl, rec, out);
                }
            }
        }
        HStmt::For { body, .. }
        | HStmt::While { body, .. }
        | HStmt::Loop { body, .. }
        | HStmt::Scope { body, .. }
        | HStmt::Block { body, .. } => {
            for s in body {
                gom_stmt(s, syms, tbl, rec, out);
            }
        }
        HStmt::Match { arms, .. } => {
            for a in arms {
                for s in &a.body {
                    gom_stmt(s, syms, tbl, rec, out);
                }
            }
        }
        _ => {}
    }
}

/// **ST-91**: phân tích lại văn bản với **sentinel** `__vis` chèn tại con trỏ — `p.` một mình là cú
/// pháp **dở** (parser báo “mong định danh”) nên HIR rỗng; chèn tên giả cho cây hợp lệ rồi lấy thành
/// viên. Chỉ dùng cho completion, **không** đổi chẩn đoán/dữ liệu `KetQua` gốc.
pub fn tu_van_ban_sua(text: &str, byte: usize) -> ThanhVien {
    let b = byte.min(text.len());
    let mut t = String::with_capacity(text.len() + 5);
    t.push_str(&text[..b]);
    t.push_str("__vis");
    t.push_str(&text[b..]);
    let lexed = vis_syntax::lex(&t, Mode::Script);
    let parsed = vis_syntax::parse_voi(&t, lexed.tokens.clone(), Mode::Script);
    let mut syms = Symbols::default();
    let mut tbl = TypeTable::new();
    let out = lower_into(
        &mut syms,
        &mut tbl,
        &parsed.cst,
        &lexed,
        LowerOptions::default(),
    );
    gom(&out.hir, &syms, &tbl)
}

/// Gom thành viên từ HIR; **tất định** theo thứ tự khai.
pub fn gom(hir: &vis_types::Hir, syms: &Symbols, tbl: &TypeTable) -> ThanhVien {
    let mut tv = ThanhVien::default();
    let rec: HashSet<String> = hir
        .items
        .iter()
        .filter_map(|it| match it {
            HItem::Rec(r) => Some(r.name.clone()),
            _ => None,
        })
        .collect();
    for it in &hir.items {
        match it {
            HItem::Rec(r) => {
                for f in &r.fields {
                    tv.truong.push((
                        r.name.clone(),
                        GoiY {
                            label: f.name.clone(),
                            kind: 5,
                            detail: Some(format!("{}: {}", f.name, tbl.show(&f.ty))),
                        },
                    ));
                }
            }
            HItem::Impl(imp) => {
                let ten = tbl.show(&imp.ty);
                for f in &imp.methods {
                    tv.phuong.push((
                        ten.clone(),
                        GoiY {
                            label: f.name.clone(),
                            kind: 2,
                            detail: Some(chu_ky_fn(f, tbl)),
                        },
                    ));
                }
            }
            HItem::Fn(f) => {
                for pr in &f.params {
                    tv.kieu_bien.push((pr.name.clone(), tbl.show(&pr.ty)));
                }
                for st in &f.body {
                    gom_stmt(st, syms, tbl, &rec, &mut tv.kieu_bien);
                }
            }
            _ => {}
        }
    }
    for st in &hir.stmts {
        gom_stmt(st, syms, tbl, &rec, &mut tv.kieu_bien);
    }
    // Khử trùng `(kiểu, label)` giữ lần khai đầu (impl trùng không nhân đôi gợi ý).
    let mut da = HashSet::new();
    tv.truong
        .retain(|(k, g)| da.insert((k.clone(), g.label.clone())));
    let mut da = HashSet::new();
    tv.phuong
        .retain(|(k, g)| da.insert((k.clone(), g.label.clone())));
    tv
}
