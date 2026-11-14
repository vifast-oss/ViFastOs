// SPDX-License-Identifier: MIT OR Apache-2.0
//! **trait / `impl` / `derives`** (ST-29) — tách khỏi `lower.rs` cho luật R1 (≤ 1.500 dòng/tệp).
//!
//! Dispatch **tĩnh**: `x.m(…)` với `x` kiểu tĩnh `Record`/`Enum` ⇒ hạ thẳng `Call(impl_fn)` (chèn `self`
//! đầu) ⇒ interp và VM dùng **cùng HIR** (không bảng `impls`, không bump `VBC1`).
//! `derives Eq/Show/Json` = **kiểm + đăng ký**, ngữ nghĩa dùng cái có sẵn.

use vis_diag::TYPE_CODES;
use vis_syntax::{Node, NodeKind};

use super::{FrameKind, Lowerer, SymKind};
use crate::hir::{BinOp, HExpr, HFn, HImpl, HItem, HParam, HStmt, InterpPart};
use crate::ty::Type;

/// Chữ ký một phương thức trait: `(tên, số tham số, kiểu trả, kiểu từng tham số)` (ST-29).
pub type PhuongThucTrait = (String, usize, Type, Vec<Type>);

/// **ST-57 (VD-98 (1))** — chữ ký **công khai** của `trait`: dùng để **xuất qua module**
/// (`pub_traits`) và kiểm `impl`/siêu trait ở tệp khác. Thay tuple `ChuKyTrait` cũ (thêm
/// `supertraits`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitSig {
    pub name: String,
    /// Siêu trait khai `trait T: A, B:` — kiểm **một tầng** (VD-98 (2)).
    pub supertraits: Vec<String>,
    pub methods: Vec<PhuongThucTrait>,
}

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

    /// Lượt 1b — thu thập `trait` (tên + **siêu trait** + chữ ký phương thức).
    pub(crate) fn collect_trait(&mut self, node: &Node) {
        let Some(name) = self.decl_name(node) else {
            return;
        };
        // **ST-57**: `trait T: A, B:` — các `DottedPath` trước dấu `:` cuối (parser đã sẵn).
        let supertraits: Vec<String> = node
            .children()
            .filter(|n| n.kind == NodeKind::DottedPath)
            .map(|p| self.dotted_text(p))
            .collect();
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
        self.traits.push(TraitSig {
            name,
            supertraits,
            methods,
        });
    }

    /// Lượt 1b — thu thập `impl`: khai sym cho từng phương thức + ghi bảng `(kiểu, tên) → sym`.
    pub(crate) fn collect_impl(&mut self, node: &Node) {
        let ty = node
            .child(NodeKind::Type)
            .map(|t| self.resolve_type(t))
            .unwrap_or(Type::Error);
        // **ST-57 (VD-98 (2))**: ghi cặp `(kiểu, trait)` để kiểm **siêu trait** sau vòng 1b.
        if let Some(p) = node.child(NodeKind::DottedPath) {
            let tn = self.dotted_text(p);
            self.impl_pairs.push((ty.clone(), tn, node.span));
        }
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
            // **ST-57 (VD-93 (3))**: chưa **sinh mã** derive cho kiểu **generic** (cần dispatch theo
            // `T` — VS1b) ⇒ chẩn đoán **tĩnh**, không sinh.
            if !self.generics_of(node).is_empty() {
                self.diag(
                    &TYPE_CODES[2],
                    format!("derive `{d}` trên kiểu **generic** chưa hỗ trợ ở VS0 (không sinh mã — VD-98)"),
                    node.span,
                );
                break;
            }
            // Trường `float`/`Fn` với `Eq`: `==` trên số thực là **không toàn phần** ⇒ `VS0403` **tĩnh**.
            if d == "Eq" {
                for t in field_tys {
                    // **ST-60c (B5)**: đệ quy **sâu** (list/map/opt/result/tuple/generic).
                    if co_float_sau(t) {
                        self.diag(
                            &TYPE_CODES[2],
                            format!(
                                "derive `Eq` không dùng được: trường `{}` (số thực/hàm so sánh không toàn phần — VD-93)",
                                t.name()
                            ),
                            node.span,
                        );
                        break;
                    }
                }
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
                // **ST-57 (VD-98 (1))**: trait nội tệp **hoặc** trait `pub` nhập từ module khác.
                let noi_tep = self.traits.iter().find(|t| &t.name == tn).cloned();
                let nhap = self.traits_ngoai.iter().find(|t| &t.name == tn).cloned();
                let Some(sig) = noi_tep.clone().or(nhap) else {
                    self.diag(
                        &TYPE_CODES[2],
                        format!(
                            "không có trait `{tn}` trong **chương trình** (mồ côi **thật** hoặc trait chưa khai `pub`/chưa `use`; VD-98)"
                        ),
                        node.span,
                    );
                    return;
                };
                // Mồ côi **thật** = trait **nhập** *và* kiểu **nhập** ⇒ `impl` phải ở tệp khai một trong hai.
                if noi_tep.is_none() {
                    if let Some(tk) = self.ten_kieu(ty) {
                        if self.kieu_ngoai.contains(&tk) {
                            self.diag(
                                &TYPE_CODES[2],
                                format!(
                                    "mồ côi: trait `{tn}` và kiểu `{tk}` đều ở module khác — `impl` phải ở tệp khai trait hoặc kiểu (VD-98)"
                                ),
                                node.span,
                            );
                            return;
                        }
                    }
                }
                let mong = sig.methods;
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
                                &TYPE_CODES[4],
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
                            &TYPE_CODES[4],
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

// ---------------------------------------------------------------------------
// ST-57 (VD-93/VD-98 (3)) — `derives` **sinh HIR** ở lượt 1c
// ---------------------------------------------------------------------------

impl<'a> Lowerer<'a> {
    /// Lượt **1c** — sinh `impl` **thật** cho `derives Show`/`Eq` của mọi `type`/`enum` trong tệp.
    ///
    /// Chạy **sau** 1b và **trước** mọi lượt hạ thân ⇒ `p.show()` ở câu **trước** khai báo vẫn phân
    /// giải được (chỉ số sym + bảng `impls` đã có). Item sinh ra **nối cuối** `items` (interp/VM
    /// đăng ký hàm `impl` theo `sym` — không phân biệt sinh hay viết tay).
    pub(crate) fn sinh_derives(&mut self) {
        let mut ra: Vec<HItem> = Vec::new();
        for child in self.cst.root.children() {
            match child.kind {
                NodeKind::TypeDecl => self.sinh_derive_rec(child, &mut ra),
                NodeKind::EnumDecl => self.sinh_derive_enum(child, &mut ra),
                _ => {}
            }
        }
        self.derived.extend(ra);
    }

    fn sinh_derive_rec(&mut self, node: &Node, ra: &mut Vec<HItem>) {
        let Some(name) = self.decl_name(node) else {
            return;
        };
        let derives = self.derives_of(node);
        // Kiểu **generic** ⇒ `kiem_derives` báo `VS0403`, ở đây **không sinh** (VD-98 (3)).
        if derives.is_empty() || !self.generics_of(node).is_empty() {
            return;
        }
        let Some((_, id)) = self.rec_names.iter().find(|(n, _)| *n == name).cloned() else {
            return;
        };
        let truong: Vec<(String, Type)> = node
            .children()
            .filter(|n| n.kind == NodeKind::TypeField)
            .filter_map(|f| {
                Some((
                    self.decl_name(f)?,
                    f.child(NodeKind::Type)
                        .map(|t| self.resolve_type(t))
                        .unwrap_or(Type::Error),
                ))
            })
            .collect();
        self.sinh_derive(
            DeriveNguon {
                node,
                ty: Type::Record(id),
                ten: name,
                derives,
                truong,
                la_enum: false,
            },
            ra,
        );
    }

    fn sinh_derive_enum(&mut self, node: &Node, ra: &mut Vec<HItem>) {
        let Some(name) = self.decl_name(node) else {
            return;
        };
        let derives = self.derives_of(node);
        if derives.is_empty() || !self.generics_of(node).is_empty() {
            return;
        }
        let Some((_, id)) = self.enum_names.iter().find(|(n, _)| *n == name).cloned() else {
            return;
        };
        // `enum show` **cắt** (nợ, VD-98 (3)); `enum eq` dùng `==` sẵn có.
        self.sinh_derive(
            DeriveNguon {
                node,
                ty: Type::Enum(id),
                ten: name,
                derives,
                truong: Vec::new(),
                la_enum: true,
            },
            ra,
        );
    }

    fn sinh_derive(&mut self, src: DeriveNguon, ra: &mut Vec<HItem>) {
        let mut methods: Vec<HFn> = Vec::new();
        let span = src.node.span;
        // **ST-60c (B3)**: `impl` **sinh từ `derives`** phải vào `impl_pairs` như impl viết tay —
        // nếu không `impl TraitCon for T` (với `trait TraitCon: Show`) bị `VS0403` **oan**. Chỉ ghi
        // cặp khi **thật sự** sinh được (Eq bị bỏ vì `float`/hàm ⇒ **không** ghi ⇒ thiếu thật vẫn đỏ).
        let mut sinh_show = false;
        let mut sinh_eq = false;
        if src.derives.iter().any(|d| d == "Show") {
            // **ST-60c (B4)**: `enum … derives Show` nay **sinh mã thật** (trước: im lặng ⇒
            // `e.show()` ra `VS0403` khó hiểu). Thân: **một** phần nội suy ⇒ `show` của **giá trị**
            // (`Value::Enum` in `BiénThể` hoặc `BiénThể(args…)`), không đệ quy vào phương thức.
            let f = if src.la_enum {
                self.sinh_show_enum(src.ty.clone(), &src.ten, span)
            } else {
                self.sinh_show(src.ty.clone(), &src.ten, &src.truong, span)
            };
            if let Some(f) = f {
                methods.push(f);
                sinh_show = true;
            }
        }
        if src.derives.iter().any(|d| d == "Eq") && !co_truong_khong_eq(&src.truong) {
            if let Some(f) = self.sinh_eq(src.ty.clone(), &src.ten, span) {
                methods.push(f);
                sinh_eq = true;
            }
        }
        if sinh_show {
            self.impl_pairs
                .push((src.ty.clone(), "Show".to_string(), span));
        }
        if sinh_eq {
            self.impl_pairs
                .push((src.ty.clone(), "Eq".to_string(), span));
        }
        if !methods.is_empty() {
            ra.push(HItem::Impl(HImpl {
                trait_name: None,
                ty: src.ty,
                span,
                methods,
            }));
        }
    }

    /// Khai một phương thức sinh: sym + **khung riêng** + tham số; `dung(ids)` dựng **thân** từ chỉ
    /// số sym của tham số (đúng thứ tự `tham`). `None` nếu **người dùng đã viết tay** cùng tên
    /// (VD-93 (4): người dùng **thắng**, không sinh bản trùng).
    fn sinh_pt(
        &mut self,
        ty: &Type,
        m: PtSinh,
        dung: impl FnOnce(&[u32]) -> Vec<HStmt>,
    ) -> Option<HFn> {
        let (ten, ten_pt, span) = (m.ten, m.ten_pt, m.span);
        if self
            .impls
            .iter()
            .any(|(t, n, _)| n == ten_pt && self.ten_kieu(t).as_deref() == Some(ten))
        {
            return None;
        }
        let sym = self.declare(&format!("{ten}.{ten_pt}"), SymKind::Item, span, None);
        self.impls.push((ty.clone(), ten_pt.to_string(), sym));
        self.push_frame(FrameKind::Fn, span);
        let mut ids: Vec<u32> = Vec::with_capacity(m.tham.len());
        let mut params: Vec<HParam> = Vec::with_capacity(m.tham.len());
        for (n, t) in &m.tham {
            let s = self.declare(n, SymKind::Param, span, Some(t.clone()));
            ids.push(s);
            params.push(HParam {
                name: (*n).to_string(),
                span,
                sym: s,
                ty: t.clone(),
                named: false,
                is_self: false,
                is_var_self: false,
                default: None,
                variadic: false,
            });
        }
        let body = dung(&ids);
        self.pop_frame();
        Some(HFn {
            name: format!("{ten}.{ten_pt}"),
            span,
            sym,
            kind: super::FnKind::ImplMethod,
            is_pub: false,
            is_async: false,
            generics: Vec::new(),
            params,
            ret: m.ret,
            body,
        })
    }

    /// `fn show(self) -> str` — **đúng byte** `Value::show` của bản ghi (tên động rỗng):
    /// `{x: 1, ten: a}`. Dựng bằng `Interp` (không `+` nối chuỗi — tyck không có `(Add, Str, Str)`).
    /// **ST-60c (B4)**: `show` cho **enum** = **một** phần nội suy của `self` ⇒ in đúng dạng giá trị
    /// (`A`, `B(1, 2)`) như `Value::show` lúc chạy; không gọi lại phương thức (không đệ quy).
    fn sinh_show_enum(&mut self, ty: Type, ten: &str, span: vis_syntax::Span) -> Option<HFn> {
        let ty2 = ty.clone();
        self.sinh_pt(
            &ty,
            PtSinh {
                ten,
                ten_pt: "show",
                ret: Type::Str,
                tham: vec![("self", ty2)],
                span,
            },
            move |ids| {
                let me = ids[0];
                let parts: Vec<InterpPart> = vec![InterpPart::Expr(Box::new(HExpr::Var {
                    name: "self".into(),
                    sym: Some(me),
                    span,
                }))];
                vec![HStmt::Return {
                    value: Some(HExpr::Interp { parts, span }),
                    span,
                }]
            },
        )
    }

    fn sinh_show(
        &mut self,
        ty: Type,
        ten: &str,
        truong: &[(String, Type)],
        span: vis_syntax::Span,
    ) -> Option<HFn> {
        let truong = truong.to_vec();
        self.sinh_pt(
            &ty,
            PtSinh {
                ten,
                ten_pt: "show",
                ret: Type::Str,
                tham: vec![("self", ty.clone())],
                span,
            },
            move |ids| {
                let me = ids[0];
                let mut parts: Vec<InterpPart> = Vec::new();
                if truong.is_empty() {
                    parts.push(InterpPart::Str("{}".into()));
                } else {
                    for (i, (fn_, _)) in truong.iter().enumerate() {
                        if i > 0 {
                            parts.push(InterpPart::Str(", ".into()));
                        }
                        parts.push(InterpPart::Str(format!("{fn_}: ")));
                        parts.push(InterpPart::Expr(Box::new(HExpr::Field {
                            base: Box::new(HExpr::Var {
                                name: "self".into(),
                                sym: Some(me),
                                span,
                            }),
                            name: fn_.clone(),
                            optional: false,
                            span,
                        })));
                    }
                    parts.insert(0, InterpPart::Str("{".into()));
                    parts.push(InterpPart::Str("}".into()));
                }
                vec![HStmt::Return {
                    value: Some(HExpr::Interp { parts, span }),
                    span,
                }]
            },
        )
    }

    /// `fn eq(self, other) -> bool` — `self == other` (hai đường gọi cùng `eq_value`).
    fn sinh_eq(&mut self, ty: Type, ten: &str, span: vis_syntax::Span) -> Option<HFn> {
        self.sinh_pt(
            &ty,
            PtSinh {
                ten,
                ten_pt: "eq",
                ret: Type::Bool,
                tham: vec![("self", ty.clone()), ("other", ty.clone())],
                span,
            },
            |ids| {
                vec![HStmt::Return {
                    value: Some(HExpr::Bin {
                        op: BinOp::Eq,
                        lhs: Box::new(HExpr::Var {
                            name: "self".into(),
                            sym: Some(ids[0]),
                            span,
                        }),
                        rhs: Box::new(HExpr::Var {
                            name: "other".into(),
                            sym: Some(ids[1]),
                            span,
                        }),
                        span,
                    }),
                    span,
                }]
            },
        )
    }
}

/// Nguồn một lượt sinh `derives` (gói tham số cho [`Lowerer::sinh_derive`]).
pub(super) struct DeriveNguon<'n> {
    node: &'n Node,
    ty: Type,
    ten: String,
    derives: Vec<String>,
    truong: Vec<(String, Type)>,
    la_enum: bool,
}

/// Tham số một phương thức **sinh** (gói cho [`Lowerer::sinh_pt`]).
pub(super) struct PtSinh<'a> {
    ten: &'a str,
    ten_pt: &'a str,
    ret: Type,
    tham: Vec<(&'a str, Type)>,
    span: vis_syntax::Span,
}

/// **ST-60c (B5)**: kiểu có chứa **`float`/hàm** ở **bất kỳ độ sâu** không — `Eq` đòi so sánh
/// **toàn phần** nên đệ quy qua `List`/`Map`/`Opt`/`Result`/`Tuple`/`TypeRef` (generic) (VD-93 (3)).
fn co_float_sau(t: &Type) -> bool {
    match t {
        Type::Float | Type::Fn(..) => true,
        Type::List(x) | Type::Opt(x) => co_float_sau(x),
        Type::Map(k, v) | Type::Result(k, v) => co_float_sau(k) || co_float_sau(v),
        // `TypeRef` = `tuple` nhiều phần tử, `Task[T]`, generic chưa thay ⇒ soi **mọi** đối số.
        Type::TypeRef { args, .. } => args.iter().any(co_float_sau),
        // Bản ghi lồng: cần `TypeTable` ⇒ ngoài tầm hàm thuần (nợ, xem JOURNAL).
        _ => false,
    }
}

/// Kiểu của một trường có **so sánh toàn phần** không (đệ quy **sâu** — ST-60c).
fn co_truong_khong_eq(truong: &[(String, Type)]) -> bool {
    truong.iter().any(|(_, t)| co_float_sau(t))
}

// ---------------------------------------------------------------------------
// ST-57 (VD-98 (2)) — siêu trait
// ---------------------------------------------------------------------------

impl<'a> Lowerer<'a> {
    /// **ST-57 (VD-98 (2))**: mỗi `impl Trait for Type`, mọi **siêu trait** của `Trait` phải có
    /// `impl` cùng kiểu — kiểm **một tầng** (bắc cầu = nợ), `VS0403` (không cấp mã mới).
    pub(crate) fn kiem_sieu_trait(&mut self) {
        let pairs = self.impl_pairs.clone();
        for (ty, tn, span) in &pairs {
            let sieu: Vec<String> = self
                .traits
                .iter()
                .find(|t| &t.name == tn)
                .or_else(|| self.traits_ngoai.iter().find(|t| &t.name == tn))
                .map(|t| t.supertraits.clone())
                .unwrap_or_default();
            for st in sieu {
                if !pairs.iter().any(|(t2, n2, _)| n2 == &st && t2 == ty) {
                    self.diag(
                        &vis_diag::TYPE_CODES[2],
                        format!(
                            "`impl {tn} for …` thiếu `impl {st} for …` (siêu trait `{st}` — kiểm **một tầng**, VD-98)"
                        ),
                        *span,
                    );
                }
            }
        }
    }
}
