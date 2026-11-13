// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Chiếu trường** của chế độ lệnh (ST-40b): `get <trường>`, `select <t1>, <t2>, …` ⇒ lambda chiếu trên
//! `it` (bản ghi **ẩn danh** cho `select`). Tách khỏi `cmd.rs` để giữ giới hạn **1.500 dòng/tệp** (R1).

use super::*;
use vis_diag::COMMAND_CODES;

/// Tách theo dấu `,` ở **độ sâu 0** (bỏ qua `,` trong `{}`/`[]`/`()` và chuỗi) — ST-40a.
pub(super) fn tach_phay(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut sau = 0usize;
    let mut depth = 0i32;
    let mut trong_chuoi = false;
    let mut thoat = false;
    for (i, c) in s.char_indices() {
        if thoat {
            thoat = false;
            continue;
        }
        match c {
            '"' if !trong_chuoi || depth >= 0 => trong_chuoi = !trong_chuoi,
            '\\' if trong_chuoi => thoat = true,
            '{' | '[' | '(' if !trong_chuoi => depth += 1,
            '}' | ']' | ')' if !trong_chuoi => depth -= 1,
            ',' if !trong_chuoi && depth == 0 => {
                out.push(s[sau..i].trim());
                sau = i + 1;
            }
            _ => {}
        }
    }
    out.push(s[sau..].trim());
    out
}

/// ST-40b: hạ `get <trường>` / `select <t1>, <t2>, …` thành lambda chiếu.
///
/// `get` ⇒ `it => it.<trường>` (một trường); `select` ⇒ `it => {<t1>: it.<t1>, …}` (bản ghi **ẩn
/// danh**, giữ **đúng thứ tự đối số**). Trường không có trên `it` ⇒ `VS0405`; `it` kiểu `{error}`
/// ⇒ để **kiểm động** (không báo oan). Trả `None` khi danh sách tên rỗng/sai (để đường thường báo
/// `VS0602` thiếu đối số).
impl<'a> Lowerer<'a> {
    pub(super) fn ha_chieu(
        &mut self,
        lenh: &str,
        start: usize,
        end: usize,
        it_ty: &Type,
        span: Span,
    ) -> Option<HExpr> {
        let text = self.text.get(start..end.min(self.text.len()))?;
        let mut ten: Vec<(String, Span)> = Vec::new();
        let mut bo: Vec<String> = Vec::new();
        for phan in tach_phay(text) {
            let p = phan.trim();
            if p.is_empty() {
                continue;
            }
            let p = p.strip_prefix("it.").unwrap_or(p);
            if p.is_empty() || !p.chars().all(|c| c == '_' || c.is_alphanumeric()) {
                return None;
            }
            let off = self.text[start..end]
                .find(p)
                .map(|i| start + i)
                .unwrap_or(start);
            ten.push((p.to_string(), Span::new(off, off + p.len())));
            bo.push(p.to_string());
        }
        let _ = bo;
        if ten.is_empty() {
            return None;
        }
        if lenh == "get" && ten.len() != 1 {
            self.diag(&COMMAND_CODES[1], "`get` cần đúng 1 tên trường", span);
            return None;
        }
        let rec = match it_ty {
            Type::Record(id) => self.tbl.rec(*id).cloned(),
            _ => None,
        };
        // Khai `it` trong khung riêng rồi dựng thân.
        self.push_frame(FrameKind::Fn, span);
        let psym = self.declare("it", SymKind::Param, span, Some(it_ty.clone()));
        let base = HExpr::Var {
            name: "it".into(),
            sym: Some(psym),
            span,
        };
        let mut truong: Vec<(String, Type, HExpr)> = Vec::new();
        for (ten, tspan) in &ten {
            let ty = match &rec {
                Some(r) => match r.fields.iter().find(|f| &f.name == ten) {
                    Some(f) => f.ty.clone(),
                    None => {
                        self.diag(
                            vis_diag::ma_truong(),
                            format!("`it` không có trường `{ten}`"),
                            *tspan,
                        );
                        Type::Error
                    }
                },
                // Không phải bản ghi (kể cả `{error}`) ⇒ **kiểm động**: giữ nguyên hành vi cũ,
                // không báo `VS0405` oan (Agent 0 chốt: `( [1,2] ) | select a` phải chạy động).
                None => Type::Error,
            };
            truong.push((
                ten.clone(),
                ty,
                HExpr::Field {
                    base: Box::new(base.clone()),
                    name: ten.clone(),
                    optional: false,
                    span: *tspan,
                },
            ));
        }
        let expr = if lenh == "get" {
            let (_, ty, e) = truong.pop()?;
            let lam = HExpr::Lambda {
                params: vec![HParam {
                    name: "it".into(),
                    span,
                    sym: psym,
                    ty: it_ty.clone(),
                    named: false,
                    is_self: false,
                    is_var_self: false,
                    variadic: false,
                    default: None,
                }],
                ret: ty,
                body: crate::hir::HBody::Expr(Box::new(e)),
                span,
            };
            self.pop_frame();
            return Some(lam);
        } else {
            let fields_def: Vec<crate::ty::FieldDef> = truong
                .iter()
                .map(|(n, t, _)| crate::ty::FieldDef {
                    name: n.clone(),
                    ty: t.clone(),
                    span,
                    has_default: false,
                })
                .collect();
            let id = self.tbl.push_rec(crate::ty::RecDef {
                name: None,
                fields: fields_def,
                span,
                anonymous: true,
            });
            let ty = Type::Record(id);
            let fields = truong
                .into_iter()
                .map(|(n, _, e)| (n, e))
                .collect::<Vec<_>>();
            HExpr::Record {
                fields,
                ty: ty.clone(),
                span,
            }
        };
        let lam = HExpr::Lambda {
            params: vec![HParam {
                name: "it".into(),
                span,
                sym: psym,
                ty: it_ty.clone(),
                named: false,
                is_self: false,
                is_var_self: false,
                variadic: false,
                default: None,
            }],
            ret: match &expr {
                HExpr::Record { ty, .. } => ty.clone(),
                _ => Type::Error,
            },
            body: crate::hir::HBody::Expr(Box::new(expr)),
            span,
        };
        self.pop_frame();
        Some(lam)
    }
}
