// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Instantiation thật** cho `type T[..]` / `enum E[..]` (ST-35) — tách khỏi `lower.rs` cho luật R1.
//!
//! Ý tưởng: mỗi **tổ hợp** đối số kiểu (`Hop[int]`, `Hop[str]`) là **một** `RecId`/`EnumId` riêng, sao
//! từ **template** (trường còn ký hiệu `T`) với `T` đã thay ⇒ mọi so sánh/kiểm trường có sẵn **tự** cho
//! `Hop[int] ≠ Hop[str]`. Payload lúc chạy **không** đổi (xoá kiểu) ⇒ `codegen`/`VBC1` không đụng.

use vis_syntax::{Node, NodeKind};

use super::Lowerer;
use crate::ty::{EnumId, RecId, Type};

/// Thay `TypeRef{T}` bằng kiểu cụ thể (bản sao cục bộ của `tyck::thay_gen`, không phụ thuộc tyck).
pub(crate) fn thay(ty: &Type, bang: &[(String, Type)]) -> Type {
    match ty {
        Type::TypeRef { name, args } if args.is_empty() => bang
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, t)| t.clone())
            .unwrap_or(Type::Error),
        Type::TypeRef { name, args } => Type::TypeRef {
            name: name.clone(),
            args: args.iter().map(|a| thay(a, bang)).collect(),
        },
        Type::List(t) => Type::List(Box::new(thay(t, bang))),
        Type::Map(k, v) => Type::Map(Box::new(thay(k, bang)), Box::new(thay(v, bang))),
        Type::Opt(t) => Type::Opt(Box::new(thay(t, bang))),
        Type::Result(a, b) => Type::Result(Box::new(thay(a, bang)), Box::new(thay(b, bang))),
        Type::Fn(ps, r) => Type::Fn(
            ps.iter().map(|p| thay(p, bang)).collect::<Vec<_>>().into(),
            Box::new(thay(r, bang)),
        ),
        other => other.clone(),
    }
}

impl<'a> Lowerer<'a> {
    /// Dựng **template** trường của `type T[..]` (kiểu còn `TypeRef{"T"}`).
    pub(crate) fn dung_template_rec(&mut self, node: &Node, name: &str) {
        let luu = self.sym_generic;
        self.sym_generic = true;
        let tys: Vec<Type> = node
            .children()
            .filter(|n| n.kind == NodeKind::TypeField)
            .filter_map(|f| f.child(NodeKind::Type))
            .map(|t| self.resolve_type(t))
            .collect();
        self.sym_generic = luu;
        if !tys.is_empty() {
            let params = self.generic_scope.clone();
            // Vị trí tham số kiểu của từng trường (ST-35) — tyck dùng để hợp nhất `Hop[T]` khi
            // chưa có bản instantiation nào được dựng.
            let pos: Vec<Option<usize>> = tys
                .iter()
                .map(|t| match t {
                    Type::TypeRef { name: n, args: a } if a.is_empty() => {
                        params.iter().position(|p| p == n)
                    }
                    _ => None,
                })
                .collect();
            if let Some((_, id)) = self.rec_names.iter().find(|(n, _)| n == name) {
                self.tbl.set_rec_pos(*id, pos);
            }
            self.rec_tmpl.push((name.to_string(), params, tys));
        }
    }

    /// Dựng **template** biến thể của `enum E[..]`.
    pub(crate) fn dung_template_enum(&mut self, node: &Node, name: &str) {
        let luu = self.sym_generic;
        self.sym_generic = true;
        let mut vs: Vec<(String, Vec<Type>)> = Vec::new();
        for v in node.children().filter(|n| n.kind == NodeKind::Variant) {
            let Some(vn) = self.decl_name(v) else {
                continue;
            };
            let tys: Vec<Type> = v
                .children()
                .filter(|n| n.kind == NodeKind::VariantField)
                .filter_map(|f| f.child(NodeKind::Type))
                .map(|t| self.resolve_type(t))
                .collect();
            vs.push((vn, tys));
        }
        self.sym_generic = luu;
        if !vs.is_empty() {
            let params = self.generic_scope.clone();
            let pos: Vec<Vec<Option<usize>>> = vs
                .iter()
                .map(|(_, tys)| {
                    tys.iter()
                        .map(|t| match t {
                            Type::TypeRef { name: n, args: a } if a.is_empty() => {
                                params.iter().position(|p| p == n)
                            }
                            _ => None,
                        })
                        .collect()
                })
                .collect();
            if let Some((_, id)) = self.enum_names.iter().find(|(n, _)| n == name) {
                self.tbl.set_enum_pos(*id, pos);
            }
            self.enum_tmpl.push((name.to_string(), params, vs));
        }
    }

    /// `Hop[int]` ⇒ `RecId` **riêng** (intern theo tổ hợp); `None` nếu chưa có template.
    pub(crate) fn inst_rec(&mut self, name: &str, base: RecId, args: Vec<Type>) -> Option<RecId> {
        if let Some(((_, _), id)) = self
            .inst_cache
            .iter()
            .find(|((n, a), _)| n == name && *a == args)
        {
            return Some(RecId(*id));
        }
        let (params, tmpl) = self
            .rec_tmpl
            .iter()
            .find(|(n, _, _)| n == name)
            .map(|(_, p, v)| (p.clone(), v.clone()))?;
        let bang: Vec<(String, Type)> = params.into_iter().zip(args.clone()).collect();
        let base_def = self.tbl.rec(base)?.clone();
        if base_def.fields.len() != tmpl.len() {
            return None;
        }
        let mut def = base_def;
        for (f, t) in def.fields.iter_mut().zip(tmpl.iter()) {
            f.ty = thay(t, &bang);
        }
        def.name = Some(name.to_string());
        def.anonymous = false;
        let id = self.tbl.push_rec_inst(name.to_string(), args.clone(), def);
        self.inst_cache.push(((name.to_string(), args), id.0));
        Some(id)
    }

    /// `Or[int, str]` ⇒ `EnumId` **riêng** (intern theo tổ hợp).
    pub(crate) fn inst_enum(
        &mut self,
        name: &str,
        base: EnumId,
        args: Vec<Type>,
    ) -> Option<EnumId> {
        if let Some(((_, _), id)) = self
            .inst_cache
            .iter()
            .find(|((n, a), _)| n == name && *a == args)
        {
            return Some(EnumId(*id));
        }
        let (params, vs) = self
            .enum_tmpl
            .iter()
            .find(|(n, _, _)| n == name)
            .map(|(_, p, v)| (p.clone(), v.clone()))?;
        let bang: Vec<(String, Type)> = params.into_iter().zip(args.clone()).collect();
        let mut base_def = self.tbl.enum_def(base)?.clone();
        for (v, tys) in vs.iter() {
            if let Some(dv) = base_def.variants.iter_mut().find(|d| &d.name == v) {
                for (f, t) in dv.fields.iter_mut().zip(tys.iter()) {
                    f.ty = thay(t, &bang);
                }
            }
        }
        let id = self
            .tbl
            .push_enum_inst(name.to_string(), args.clone(), base_def);
        self.inst_cache.push(((name.to_string(), args), id.0));
        Some(id)
    }
}
