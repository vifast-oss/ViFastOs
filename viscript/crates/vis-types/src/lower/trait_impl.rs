// SPDX-License-Identifier: MIT OR Apache-2.0
//! **trait / `impl` / `derives`** (ST-29) — tách khỏi `lower.rs` cho luật R1 (≤ 1.500 dòng/tệp).
//!
//! Dispatch **tĩnh**: `x.m(…)` với `x` kiểu tĩnh `Record`/`Enum` ⇒ hạ thẳng `Call(impl_fn)` (chèn `self`
//! đầu) ⇒ interp và VM dùng **cùng HIR** (không bảng `impls`, không bump `VBC1`).
//! `derives Eq/Show/Json` = **kiểm + đăng ký**, ngữ nghĩa dùng cái có sẵn.

use vis_diag::TYPE_CODES;
use vis_syntax::{Node, NodeKind};

use super::{Lowerer, SymKind};
use crate::hir::{HExpr, HFn};
use crate::ty::Type;

/// Chữ ký một phương thức trait: `(tên, số tham số, kiểu trả, kiểu từng tham số)` (ST-29).
pub(crate) type PhuongThucTrait = (String, usize, Type, Vec<Type>);
/// `trait T` → `(tên trait, danh sách phương thức)` (ST-29).
pub(crate) type ChuKyTrait = (String, Vec<PhuongThucTrait>);

impl<'a> Lowerer<'a> {
    // ----- ST-29: trait / impl (dispatch tĩnh) -----

    /// Tên kiểu để tra `impl` (record/enum đã khai) — `None` nếu kiểu không phải record/enum.
    pub(crate) fn ten_kieu(&self, ty: &Type) -> Option<String> {
        match ty {
            Type::Record(id) => self
                .tbl
                .rec(*id)
                .and_then(|r| r.name.clone())
                .or_else(|| Some(format!("record#{}", id.0))),
            Type::Enum(id) => self
                .tbl
                .enum_def(*id)
                .map(|e| e.name.clone())
                .or_else(|| Some(format!("enum#{}", id.0))),
            _ => None,
        }
    }

    /// **Kiểu tĩnh** của một biểu thức đã hạ — chỉ dùng chú thích tường minh (không suy diễn).
    pub(crate) fn static_ty(&self, e: &HExpr) -> Option<Type> {
        match e {
            HExpr::Var { sym: Some(id), .. } => self.type_env.get(id).cloned(),
            HExpr::Field {
                base,
                name,
                optional: false,
                ..
            } => match self.static_ty(base)? {
                Type::Record(id) => self.tbl.field(id, name).map(|f| f.ty.clone()),
                _ => None,
            },
            HExpr::Lit { ty, .. }
            | HExpr::Record { ty, .. }
            | HExpr::EnumLit { ty, .. }
            | HExpr::Opt { ty, .. }
            | HExpr::Tuple { ty, .. } => Some(ty.clone()),
            _ => None,
        }
    }

    /// Lượt 1b — thu thập `trait` (tên + chữ ký phương thức).
    pub(crate) fn collect_trait(&mut self, node: &Node) {
        let Some(name) = self.decl_name(node) else {
            return;
        };
        let mut methods = Vec::new();
        for m in node.children().filter(|n| n.kind == NodeKind::TraitItem) {
            let Some(mn) = self.decl_name(m) else {
                continue;
            };
            let params: Vec<Type> = m
                .child(NodeKind::Params)
                .map(|p| {
                    p.children()
                        .filter(|n| n.kind == NodeKind::Param)
                        .map(|p| {
                            p.child(NodeKind::Type)
                                .map(|t| self.resolve_type(t))
                                .unwrap_or(Type::Error)
                        })
                        .collect()
                })
                .unwrap_or_default();
            let ret = m
                .child(NodeKind::Type)
                .map(|t| self.resolve_type(t))
                .unwrap_or(Type::Unit);
            methods.push((mn, params.len(), ret, params));
        }
        self.traits.push((name, methods));
    }

    /// Lượt 1b — thu thập `impl`: khai sym cho từng phương thức + ghi bảng `(kiểu, tên) → sym`.
    pub(crate) fn collect_impl(&mut self, node: &Node) {
        let ty = node
            .child(NodeKind::Type)
            .map(|t| self.resolve_type(t))
            .unwrap_or(Type::Error);
        for m in node.children().filter(|n| n.kind == NodeKind::FnDecl) {
            let Some(mn) = self.decl_name(m) else {
                continue;
            };
            let sym = self.declare(&mn, SymKind::Item, m.span, None);
            self.impls.push((ty.clone(), mn, sym));
        }
    }

    /// Kiểm `derives` (ST-29): chỉ `Eq`/`Show`/`Json`; `Json` không dùng được với trường/biến thể
    /// kiểu **hàm** (closure không mã hoá được).
    pub(crate) fn kiem_derives(&mut self, node: &Node, derives: &[String], field_tys: &[Type]) {
        for d in derives {
            let hop_le = matches!(d.as_str(), "Eq" | "Show" | "Json");
            if !hop_le {
                self.diag(
                    &TYPE_CODES[2],
                    format!("không có `derives {d}` ở VS0 (chỉ có `Eq`, `Show`, `Json`)"),
                    node.span,
                );
                continue;
            }
            if d == "Json" {
                for t in field_tys {
                    let la_ham = match t {
                        Type::Fn(..) => true,
                        Type::Opt(inner) => matches!(**inner, Type::Fn(..)),
                        _ => false,
                    };
                    if la_ham {
                        self.diag(
                            &TYPE_CODES[2],
                            "derive `Json` không dùng được: có trường/biến thể kiểu hàm (closure không mã hoá được)",
                            node.span,
                        );
                        break;
                    }
                }
            }
        }
    }

    /// Kiểm `impl Trait for Type` **khớp** chữ ký trait (ST-29): thiếu/thừa ⇒ `VS0403`; sai số đối số
    /// ⇒ `VS0401`; sai kiểu ⇒ `VS0412`; trait/kiểu ngoài tệp ⇒ mồ côi `VS0403` (§5.4).
    pub(crate) fn kiem_impl_khop(
        &mut self,
        node: &Node,
        trait_name: &Option<String>,
        ty: &Type,
        methods: &[HFn],
    ) {
        let ten_kieu = self.ten_kieu(ty);
        match trait_name {
            None => {
                // `impl Type:` — inherent: không cần trait, nhưng kiểu phải **khai trong tệp**.
                if ten_kieu.is_none() {
                    self.diag(
                        &TYPE_CODES[2],
                        "impl cho kiểu không phải record/enum khai trong tệp (VS1b)",
                        node.span,
                    );
                }
            }
            Some(tn) => {
                let Some((_, mong)) = self.traits.iter().find(|(n, _)| n == tn).cloned() else {
                    self.diag(
                        &TYPE_CODES[2],
                        format!("không có trait `{tn}` trong tệp (mồ côi/VS1b: impl phải ở module khai trait hoặc kiểu)"),
                        node.span,
                    );
                    return;
                };
                // Mồ côi: `Type` không khai trong tệp ⇒ phải có trait khai trong tệp (đã có).
                let co: Vec<&str> = methods.iter().map(|m| m.name.as_str()).collect();
                for (mn, _, _, _) in &mong {
                    if !co.contains(&mn.as_str()) {
                        self.diag(
                            &TYPE_CODES[2],
                            format!("`impl {tn} for …` thiếu phương thức `{mn}` của trait"),
                            node.span,
                        );
                    }
                }
                for m in methods {
                    let Some((_, so_mong, ret_mong, params_mong)) =
                        mong.iter().find(|(n, _, _, _)| n == &m.name)
                    else {
                        self.diag(
                            &TYPE_CODES[2],
                            format!(
                                "`impl {tn} for …` có phương thức `{}` không thuộc trait",
                                m.name
                            ),
                            m.span,
                        );
                        continue;
                    };
                    let so = m.params.len();
                    if so != *so_mong {
                        self.diag(
                            &TYPE_CODES[0],
                            format!(
                                "`{}.{}` cần {} đối số như trait `{tn}`, được {so}",
                                ten_kieu.clone().unwrap_or_default(),
                                m.name,
                                so_mong
                            ),
                            m.span,
                        );
                        continue;
                    }
                    for (i, p) in m.params.iter().enumerate() {
                        let mong = &params_mong[i];
                        if !matches!(p.ty, Type::Error)
                            && !matches!(mong, Type::Error)
                            && p.ty != *mong
                        {
                            self.diag(
                                &TYPE_CODES[3],
                                format!(
                                    "`{}.{}` đối số {} sai kiểu: trait cần `{:?}`, impl có `{:?}`",
                                    ten_kieu.clone().unwrap_or_default(),
                                    m.name,
                                    i + 1,
                                    mong,
                                    p.ty
                                ),
                                m.span,
                            );
                        }
                    }
                    if !matches!(m.ret, Type::Error)
                        && !matches!(ret_mong, Type::Error)
                        && m.ret != ret_mong.clone()
                    {
                        self.diag(
                            &TYPE_CODES[3],
                            format!(
                                "`{}.{}` sai kiểu trả: trait cần `{:?}`, impl có `{:?}`",
                                ten_kieu.clone().unwrap_or_default(),
                                m.name,
                                ret_mong,
                                m.ret
                            ),
                            m.span,
                        );
                    }
                }
            }
        }
    }
}
