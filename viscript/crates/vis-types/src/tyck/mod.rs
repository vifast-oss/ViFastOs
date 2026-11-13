// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Kiểm kiểu VS0** (ST-12) — chạy trên HIR của ST-11, báo mã `04xx` (VD-42) và `VS0301` cho
//! tên **kiểu** không tồn tại; tên **giá trị** chưa khai vẫn theo VD-45 (`Permissive`).
//!
//! # Hợp đồng
//!
//! - Đầu vào: HIR đã hạ ([`crate::hir::Hir`]) + [`crate::lower::Symbols`] + [`crate::TypeTable`].
//! - **Không** panic trên HIR hợp lệ; `Type::Error` lan truyền **im lặng** (thiết kế §14.3) và
//!   tương thích với mọi kiểu.
//! - Mã: `VS0401` sai số/thứ tự đối số, `VS0402` `match` không xét hết, `VS0403` trường/kiểu
//!   không tồn tại, `VS0412` kiểu không khớp, `VS0301` tên **kiểu** không tồn tại.
//! - `%`/`/` cho 0 là **runtime** (ST-13), **không** báo ở đây (VD-43 #2).
//!
//! # Phân tầng
//!
//! [`mod`]: khung + môi trường + câu lệnh; [`num`]: bảng toán tử số học/so sánh; [`expr`]: biểu
//! thức; [`pat`]: pattern + **xét hết**.

mod expr;
mod num;
mod pat;
mod truong;

use std::collections::HashSet;

use expr::FnSig;

use vis_diag::{NAME_CODES, TYPE_CODES};
use vis_syntax::{Diagnostic, Span};

use crate::hir::{HExpr, HImpl, HItem, HStmt, Hir};
use crate::lower::{SymKind, Symbols};
use crate::ty::{EnumId, RecId, Type, TypeTable};

/// Mã `VS0401` — sai số lượng/thứ tự đối số.
pub(crate) const ARG_COUNT: &vis_diag::Code = &TYPE_CODES[0];
/// Mã `VS0402` — `match` không xét hết.
pub(crate) const NON_EXHAUSTIVE: &vis_diag::Code = &TYPE_CODES[1];
/// Mã `VS0403` — trường/kiểu không tồn tại.
pub(crate) const NO_FIELD: &vis_diag::Code = &TYPE_CODES[2];
/// Mã `VS0412` — kiểu không khớp (ST-50: `04xx` có **5** mã, 0412 ở **index 4**).
pub(crate) const MISMATCH: &vis_diag::Code = &TYPE_CODES[4];
/// Mã `VS0301` — tên **kiểu** không tồn tại (ST-12; ST-11 không báo).
pub(crate) const NO_TYPE: &vis_diag::Code = &NAME_CODES[0];

/// Kết quả một lượt kiểm kiểu.
#[derive(Debug, Clone, Default)]
pub struct CheckOutput {
    pub diags: Vec<Diagnostic>,
    /// Kiểu của **mọi** biểu thức, theo thứ tự duyệt (tiền tự).
    pub types_of_expr: Vec<Type>,
    /// Kiểu của các biểu thức **cấp tệp**, theo thứ tự nguồn.
    pub file_expr_types: Vec<Type>,
    /// Kiểu của từng item cấp tệp (`fn`/`const`/…).
    pub item_types: Vec<(String, Type)>,
}

impl CheckOutput {
    /// `true` nếu không có chẩn đoán.
    pub fn is_clean(&self) -> bool {
        self.diags.is_empty()
    }

    /// Mã chẩn đoán theo thứ tự.
    pub fn codes(&self) -> Vec<&'static str> {
        self.diags.iter().map(|d| d.code.short).collect()
    }

    /// Kiểu "giá trị của chương trình": biểu thức cấp tệp **cuối**, không có thì `()`.
    pub fn value_type(&self) -> Type {
        self.file_expr_types.last().cloned().unwrap_or(Type::Unit)
    }
}

/// ST-28 — **xoá kiểu**: thay mọi `TypeRef` trỏ tới tham số kiểu bằng `{error}` (không còn thông tin
/// tĩnh), đệ quy vào list/map/optional/Result/hàm. Dùng cho thân hàm generic và mọi chỗ khác.
pub(crate) fn erase_gen(ty: &Type, gens: &[String]) -> Type {
    match ty {
        Type::TypeRef { name, args } if args.is_empty() && gens.iter().any(|g| g == name) => {
            Type::Error
        }
        Type::TypeRef { name, args } => Type::TypeRef {
            name: name.clone(),
            args: args.iter().map(|a| erase_gen(a, gens)).collect(),
        },
        Type::List(t) => Type::List(Box::new(erase_gen(t, gens))),
        Type::Map(k, v) => Type::Map(Box::new(erase_gen(k, gens)), Box::new(erase_gen(v, gens))),
        Type::Opt(t) => Type::Opt(Box::new(erase_gen(t, gens))),
        Type::Result(a, b) => {
            Type::Result(Box::new(erase_gen(a, gens)), Box::new(erase_gen(b, gens)))
        }
        Type::Fn(ps, r) => Type::Fn(
            ps.iter()
                .map(|p| erase_gen(p, gens))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            Box::new(erase_gen(r, gens)),
        ),
        other => other.clone(),
    }
}

/// ST-28 — **thay** tham số kiểu bằng kiểu đã suy: `T` → `int`; chưa suy ⇒ `{error}`.
pub(crate) fn thay_gen(ty: &Type, bang: &[(String, Type)]) -> Type {
    match ty {
        Type::TypeRef { name, args } if args.is_empty() => {
            match bang.iter().find(|(n, _)| n == name) {
                Some((_, t)) => t.clone(),
                None => Type::Error,
            }
        }
        Type::TypeRef { name, args } => Type::TypeRef {
            name: name.clone(),
            args: args.iter().map(|a| thay_gen(a, bang)).collect(),
        },
        Type::List(t) => Type::List(Box::new(thay_gen(t, bang))),
        Type::Map(k, v) => Type::Map(Box::new(thay_gen(k, bang)), Box::new(thay_gen(v, bang))),
        Type::Opt(t) => Type::Opt(Box::new(thay_gen(t, bang))),
        Type::Result(a, b) => {
            Type::Result(Box::new(thay_gen(a, bang)), Box::new(thay_gen(b, bang)))
        }
        Type::Fn(ps, r) => Type::Fn(
            ps.iter()
                .map(|p| thay_gen(p, bang))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            Box::new(thay_gen(r, bang)),
        ),
        other => other.clone(),
    }
}

/// Kiểm kiểu một chương trình đã hạ.
pub fn check(hir: &Hir, symbols: &Symbols, types: &TypeTable) -> CheckOutput {
    let mut c = Checker::new(symbols, types);
    c.run(hir);
    c.finish()
}

/// `true` nếu biểu thức là **literal số nguyên** (§3.5: chỉ literal được nâng `int → float`).
pub(crate) fn is_int_lit(e: &HExpr) -> bool {
    matches!(e, HExpr::Lit { lit, .. } if lit.kind == crate::hir::LitKind::Int)
}

pub(crate) struct Checker<'a> {
    symbols: &'a Symbols,
    types: &'a TypeTable,
    diags: Vec<Diagnostic>,
    /// `env[sym]` = kiểu của ký hiệu `sym` (chỉ số trong [`Symbols::syms`]).
    env: Vec<Option<Type>>,
    /// `mutable[sym]` = gán được (`var`, `var self`).
    mutable: Vec<bool>,
    types_of_expr: Vec<Type>,
    file_expr_types: Vec<Type>,
    item_types: Vec<(String, Type)>,
    /// Kiểu trả về của hàm đang kiểm (`None` = đang ở cấp tệp).
    cur_ret: Option<Type>,
    /// >0 ⇒ **không** ghi chẩn đoán (lượt khai báo trước, tránh báo trùng — ST-12b).
    quiet: u32,
    /// Chữ ký đầy đủ của hàm **khai trong tệp** (tên tham số + có mặc định), theo `sym`.
    sigs: Vec<Option<FnSig>>,
    /// Tham số kiểu đang có hiệu lực (ST-28) — `resolve_ann` giữ `TypeRef{"T"}` thay vì `VS0301`.
    generic_scope: Vec<String>,
}

impl<'a> Checker<'a> {
    fn new(symbols: &'a Symbols, types: &'a TypeTable) -> Self {
        let n = symbols.syms.len();
        Self {
            generic_scope: Vec::new(),
            symbols,
            types,
            diags: Vec::new(),
            env: vec![None; n],
            mutable: vec![false; n],
            types_of_expr: Vec::new(),
            file_expr_types: Vec::new(),
            item_types: Vec::new(),
            cur_ret: None,
            quiet: 0,
            sigs: vec![None; n],
        }
    }

    fn finish(self) -> CheckOutput {
        CheckOutput {
            diags: self.diags,
            types_of_expr: self.types_of_expr,
            file_expr_types: self.file_expr_types,
            item_types: self.item_types,
        }
    }

    // ----- chẩn đoán + tiện ích -----

    /// Ghi chẩn đoán. `line`/`col` để **0**: `check` chỉ có HIR (không có văn bản nguồn) —
    /// tầng `vis` (ST-14) tính lại từ `span` khi in. Ghi rõ ở `JOURNAL` ST-12.
    pub(crate) fn diag(
        &mut self,
        code: &'static vis_diag::Code,
        msg: impl Into<String>,
        span: Span,
    ) {
        self.diags.push(Diagnostic {
            code,
            message: msg.into(),
            span,
            line: 0,
            col: 0,
        });
    }

    /// Chữ ký của hàm theo `sym` (nếu là hàm khai trong tệp).
    pub(crate) fn sig_of(&self, sym: u32) -> Option<&FnSig> {
        self.sigs.get(sym as usize).and_then(|s| s.as_ref())
    }

    pub(crate) fn note(&mut self, ty: Type) -> Type {
        self.types_of_expr.push(ty.clone());
        ty
    }

    // ----- thăm dò môi trường -----

    pub(crate) fn ty_of(&self, sym: u32) -> Type {
        self.env
            .get(sym as usize)
            .and_then(|t| t.clone())
            .unwrap_or(Type::Error)
    }

    pub(crate) fn set_ty(&mut self, sym: u32, ty: Type) {
        if let Some(slot) = self.env.get_mut(sym as usize) {
            *slot = Some(ty);
        }
    }

    pub(crate) fn set_mutable(&mut self, sym: u32) {
        if let Some(slot) = self.mutable.get_mut(sym as usize) {
            *slot = true;
        }
    }

    fn is_mutable(&self, sym: u32) -> bool {
        self.mutable.get(sym as usize).copied().unwrap_or(false)
    }

    // ----- kiểu: so khớp, tự nâng, tên kiểu -----

    /// ST-28 — **hợp nhất** kiểu khai báo generic với kiểu đối số thật; trả `false` (và báo
    /// `VS0412`) khi mâu thuẫn. `bang` giữ `T → int` đã suy.
    pub(crate) fn hop_nhat(
        &mut self,
        mau: &Type,
        that: &Type,
        gens: &[String],
        bang: &mut Vec<(String, Type)>,
        ten: &str,
        span: Span,
    ) -> bool {
        if matches!(that, Type::Error) {
            return true;
        }
        match mau {
            Type::TypeRef { name, args } if args.is_empty() && gens.iter().any(|g| g == name) => {
                match bang.iter().find(|(n, _)| n == name) {
                    Some((_, da)) => {
                        if self.same(da, that) {
                            true
                        } else {
                            let msg = format!(
                                "`{ten}`: tham số kiểu `{name}` đã suy là `{}`, đối số này có `{}`",
                                self.types.show(da),
                                self.types.show(that)
                            );
                            self.diag(MISMATCH, msg, span);
                            false
                        }
                    }
                    None => {
                        bang.push((name.clone(), that.clone()));
                        true
                    }
                }
            }
            // ST-35: `Hop[T]` — thay ràng buộc đã biết; khớp theo **instantiation** (id riêng) nên
            // `Hop[int]` ≠ `Hop[str]`; chưa suy được `T` ⇒ **bỏ qua** (giữ hành vi xoá kiểu ST-28).
            Type::TypeRef { name, args } if !args.is_empty() => {
                let cu: Vec<Type> = args.iter().map(|a| thay_gen(a, bang)).collect();
                if cu
                    .iter()
                    .any(|t| matches!(t, Type::TypeRef { args, .. } if args.is_empty()))
                {
                    return true;
                }
                if let Some(id) = self.types.find_rec_inst(name, &cu) {
                    return self.hop_nhat(&Type::Record(id), that, gens, bang, ten, span);
                }
                if let Some(id) = self.types.find_enum_inst(name, &cu) {
                    return self.hop_nhat(&Type::Enum(id), that, gens, bang, ten, span);
                }
                // Chưa có instantiation trong bảng (vd chưa ai viết `let x: Hop[int]`): khớp
                // **cấu trúc** theo vị trí tham số của trường ⇒ vẫn buộc được `T` và báo sai kiểu.
                if let (Type::Record(that_id), Some(base)) = (that, self.types.find_rec_base(name))
                {
                    let (Some(def), Some(that_def)) = (
                        self.types.rec(base).cloned(),
                        self.types.rec(*that_id).cloned(),
                    ) else {
                        return self.hop_nhat_loi(mau, that, ten, span);
                    };
                    let pos = self.types.rec_pos(base).to_vec();
                    if def.fields.len() == that_def.fields.len()
                        && def
                            .fields
                            .iter()
                            .zip(that_def.fields.iter())
                            .all(|(f, g)| f.name == g.name)
                    {
                        let mut ok = true;
                        for (i, g) in that_def.fields.iter().enumerate() {
                            match pos.get(i).copied().flatten() {
                                Some(p) => {
                                    let want = cu.get(p).cloned().unwrap_or(Type::Error);
                                    if !self.hop_nhat(&want, &g.ty, gens, bang, ten, span) {
                                        ok = false;
                                    }
                                }
                                None => {
                                    if !self.same(&def.fields[i].ty, &g.ty) {
                                        ok = false;
                                    }
                                }
                            }
                        }
                        return ok;
                    }
                }
                if self.same(mau, that) {
                    true
                } else {
                    self.hop_nhat_loi(mau, that, ten, span)
                }
            }
            Type::List(a) => match that {
                Type::List(b) => self.hop_nhat(a, b, gens, bang, ten, span),
                _ => self.hop_nhat_loi(mau, that, ten, span),
            },
            Type::Opt(a) => match that {
                Type::Opt(b) => self.hop_nhat(a, b, gens, bang, ten, span),
                _ => true, // tự nâng `T → T?` (§4.4)
            },
            Type::Map(k1, v1) => match that {
                Type::Map(k2, v2) => {
                    self.hop_nhat(k1, k2, gens, bang, ten, span)
                        && self.hop_nhat(v1, v2, gens, bang, ten, span)
                }
                _ => self.hop_nhat_loi(mau, that, ten, span),
            },
            Type::Result(a1, b1) => match that {
                Type::Result(a2, b2) => {
                    self.hop_nhat(a1, a2, gens, bang, ten, span)
                        && self.hop_nhat(b1, b2, gens, bang, ten, span)
                }
                _ => self.hop_nhat_loi(mau, that, ten, span),
            },
            _ => {
                if self.same(mau, that) {
                    true
                } else {
                    self.hop_nhat_loi(mau, that, ten, span)
                }
            }
        }
    }

    fn hop_nhat_loi(&mut self, mau: &Type, that: &Type, ten: &str, span: Span) -> bool {
        let msg = format!(
            "`{ten}`: cần `{}`, được `{}`",
            self.types.show(mau),
            self.types.show(that)
        );
        self.diag(MISMATCH, msg, span);
        false
    }

    /// So khớp **cấu trúc**; `{error}` tương thích mọi kiểu (lan truyền im lặng).
    pub(crate) fn same(&self, a: &Type, b: &Type) -> bool {
        match (a, b) {
            (Type::Error, _) | (_, Type::Error) => true,
            (Type::Record(x), Type::Record(y)) => self.same_record(*x, *y),
            (Type::Enum(x), Type::Enum(y)) => self.same_enum(*x, *y),
            (Type::List(x), Type::List(y)) => self.same(x, y),
            (Type::Map(k1, v1), Type::Map(k2, v2)) => self.same(k1, k2) && self.same(v1, v2),
            (Type::Opt(x), Type::Opt(y)) => self.same(x, y),
            (Type::Result(a1, b1), Type::Result(a2, b2)) => self.same(a1, a2) && self.same(b1, b2),
            (Type::Fn(p1, r1), Type::Fn(p2, r2)) => {
                p1.len() == p2.len()
                    && p1.iter().zip(p2.iter()).all(|(x, y)| self.same(x, y))
                    && self.same(r1, r2)
            }
            (Type::TypeRef { name: n1, args: a1 }, Type::TypeRef { name: n2, args: a2 }) => {
                n1 == n2
                    && a1.len() == a2.len()
                    && a1.iter().zip(a2.iter()).all(|(x, y)| self.same(x, y))
            }
            _ => a == b,
        }
    }

    /// ST-35: bản gốc ↔ bản instantiation **cùng tên enum** ⇒ coi là cùng họ (kiểm đối số ở
    /// `coerce_e` khi là literal); hai instantiation **khác đối số** vẫn **khác** (`Or[int,str] ≠ Or[str,int]`).
    fn same_enum(&self, x: EnumId, y: EnumId) -> bool {
        if x == y {
            return true;
        }
        let nx = self.types.enum_def(x).map(|d| d.name.clone());
        let ny = self.types.enum_def(y).map(|d| d.name.clone());
        match (self.types.enum_inst_of(x), self.types.enum_inst_of(y)) {
            (Some((b, _)), None) => ny.as_deref() == Some(b.as_str()),
            (None, Some((b, _))) => nx.as_deref() == Some(b.as_str()),
            _ => false,
        }
    }

    fn same_record(&self, x: RecId, y: RecId) -> bool {
        if x == y {
            return true;
        }
        let (Some(rx), Some(ry)) = (self.types.rec(x), self.types.rec(y)) else {
            return false;
        };
        rx.fields.len() == ry.fields.len()
            && rx
                .fields
                .iter()
                .zip(ry.fields.iter())
                .all(|(f, g)| f.name == g.name && self.same(&f.ty, &g.ty))
    }

    /// Ép `from` về `to` **có báo lỗi**: tự nâng `T → T?`, literal `int` ở vị trí `float` (§3.5).
    /// ST-35: `Hop[{error}]` (đã xoá generic) ⇒ bản ghi **gốc** `Hop`; `Or[..]` ⇒ enum gốc.
    fn chuan(&self, t: &Type) -> Type {
        if let Type::TypeRef { name, .. } = t {
            if let Some(id) = self.types.find_rec_base(name) {
                return Type::Record(id);
            }
            if let Some(id) = self.types.find_enum_base(name) {
                return Type::Enum(id);
            }
        }
        t.clone()
    }

    pub(crate) fn coerce(&mut self, from: &Type, to: &Type, span: Span, what: &str) -> bool {
        // ST-13b (mục 1): khối phân kỳ (`never`) nhận mọi kiểu — không có giá trị để sai kiểu.
        if matches!(from, Type::Never) || matches!(to, Type::Never) {
            return true;
        }
        let (from, to) = (&self.chuan(from), &self.chuan(to));
        if self.same(from, to) {
            return true;
        }
        if let Type::Opt(inner) = to {
            if self.same(from, inner) {
                return true; // tự nâng `T → T?` (§4.4)
            }
        }
        // ST-12b (BLOCKER #2): **không** nâng `int → float` đại trà — chỉ literal số nguyên
        // (§3.5). Việc nâng literal do `coerce_e` quyết định theo biểu thức nguồn.
        let msg = format!(
            "{what}: cần `{}`, được `{}`",
            self.types.show(to),
            self.types.show(from)
        );
        self.diag(MISMATCH, msg, span);
        false
    }

    /// Ép kiểu **có biết biểu thức nguồn**: chỉ nâng `int → float` khi nguồn là **literal** số
    /// nguyên (§3.5, ngoại lệ duy nhất của "không chuyển đổi ngầm").
    pub(crate) fn coerce_e(&mut self, e: &HExpr, from: &Type, to: &Type, what: &str) -> bool {
        if matches!((from, to), (Type::Int, Type::Float)) && is_int_lit(e) {
            return true;
        }
        // ST-35: `Or.Trai(1)` gán vào `Or[int, str]` — kiểm **đối số biến thể** theo bản
        // instantiation (bản gốc đã xoá kiểu nên không tự bắt được).
        if let (HExpr::EnumLit { variant, args, .. }, Type::Enum(inst)) = (e, to) {
            if let Some(def) = self.types.enum_def(*inst).cloned() {
                if let Some(v) = def.variants.iter().find(|v| &v.name == variant) {
                    let fields = v.fields.clone();
                    for (i, a) in args.iter().enumerate() {
                        if let Some(f) = fields.get(i) {
                            let t = self.kiem_expr(a);
                            let want = f.ty.clone();
                            if !self.coerce_e(a, &t, &want, "đối số biến thể") {
                                return false;
                            }
                        }
                    }
                    return true;
                }
            }
        }
        let span = e.span();
        self.coerce(from, to, span, what)
    }

    /// Ép kiểu có **cờ literal** (tầng toán tử chỉ còn kiểu + cờ).
    pub(crate) fn coerce_lit(
        &mut self,
        from: &Type,
        to: &Type,
        span: Span,
        what: &str,
        from_lit: bool,
    ) -> bool {
        if from_lit && matches!((from, to), (Type::Int, Type::Float)) {
            return true;
        }
        self.coerce(from, to, span, what)
    }

    /// Phân giải một kiểu **chú thích**: `TypeRef` chưa biết ⇒ `VS0301` (VD-43 #3).
    pub(crate) fn resolve_ann(&mut self, ty: &Type, span: Span) -> Type {
        match ty {
            Type::Int
            | Type::Float
            | Type::Size
            | Type::Duration
            | Type::Percent
            | Type::Str
            | Type::Bool
            | Type::Bytes
            | Type::Path
            | Type::Char
            | Type::Unit
            | Type::Never
            | Type::Error
            | Type::Record(_)
            | Type::Enum(_) => ty.clone(),
            Type::Opt(inner) => {
                let t = self.resolve_ann(inner, span);
                if t.is_opt() {
                    self.diag(MISMATCH, "optional không lồng (`T??`, §4.4)", span);
                    return Type::Error;
                }
                Type::Opt(Box::new(t))
            }
            Type::List(t) => Type::List(Box::new(self.resolve_ann(t, span))),
            Type::Map(k, v) => Type::Map(
                Box::new(self.resolve_ann(k, span)),
                Box::new(self.resolve_ann(v, span)),
            ),
            Type::Result(a, b) => Type::Result(
                Box::new(self.resolve_ann(a, span)),
                Box::new(self.resolve_ann(b, span)),
            ),
            Type::Fn(args, ret) => Type::Fn(
                args.iter()
                    .map(|a| self.resolve_ann(a, span))
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                Box::new(self.resolve_ann(ret, span)),
            ),
            Type::TypeRef { name, args } => {
                // ST-28: `T` là **tham số kiểu** đang có hiệu lực ⇒ giữ nguyên làm **dấu** để
                // hợp nhất ở call-site; nơi dùng cho thân hàm sẽ được `erase_gen` xoá thành `{error}`.
                if args.is_empty() && self.generic_scope.iter().any(|g| g == name) {
                    return Type::TypeRef {
                        name: name.clone(),
                        args: Vec::new(),
                    };
                }
                self.resolve_named(name, args, span)
            }
        }
    }

    fn resolve_named(&mut self, name: &str, args: &[Type], span: Span) -> Type {
        let resolved: Vec<Type> = args.iter().map(|a| self.resolve_ann(a, span)).collect();
        match (name, resolved.len()) {
            ("list", 1) => return Type::List(Box::new(resolved[0].clone())),
            ("map", 2) => {
                return Type::Map(Box::new(resolved[0].clone()), Box::new(resolved[1].clone()))
            }
            ("Result" | "result", 2) => {
                return Type::Result(Box::new(resolved[0].clone()), Box::new(resolved[1].clone()))
            }
            ("Option" | "option", 1) => return Type::Opt(Box::new(resolved[0].clone())),
            // ST-11 mã hoá kiểu tuple là `TypeRef{name:"tuple"}` — giữ nguyên (kiểu cấu trúc).
            ("tuple", _) => {
                return Type::TypeRef {
                    name: "tuple".into(),
                    args: resolved,
                }
            }
            (_, 0) => {
                if let Some(t) = Type::builtin(name) {
                    return t;
                }
            }
            _ => {}
        }
        // ST-35: tham số kiểu **đang có hiệu lực** (`T` trong `fn f[T]`) ⇒ giữ ký hiệu.
        if resolved.is_empty() && self.generic_scope.iter().any(|g| g == name) {
            return Type::TypeRef {
                name: name.to_string(),
                args: Vec::new(),
            };
        }
        // ST-35: kiểu **người dùng generic** — instantiation đã dựng trong bảng, hoặc bản gốc.
        if let Some(id) = self.types.find_rec_inst(name, &resolved) {
            return Type::Record(id);
        }
        if let Some(id) = self.types.find_enum_inst(name, &resolved) {
            return Type::Enum(id);
        }
        if resolved.is_empty() {
            if let Some(id) = self.types.find_rec_base(name) {
                return Type::Record(id);
            }
            if let Some(id) = self.types.find_enum_base(name) {
                return Type::Enum(id);
            }
        } else if resolved
            .iter()
            .any(|t| matches!(t, Type::TypeRef { args, .. } if args.is_empty()))
        {
            return Type::TypeRef {
                name: name.to_string(),
                args: resolved,
            };
        }
        // VD-48(a): kiểu **ngoài VS0** (`AnyError`, `cap`, `ref`, `Json`, `set`, `stream`, …) vẫn
        // là `VS0301` nhưng thông điệp phải nói rõ "không có ở VS0 (vis-std/vòng 2)".
        let msg = if la_kieu_ngoai_vs0(name) {
            format!(
                "kiểu `{name}` không có ở VS0 — thuộc vis-std/vòng 2; ở VS0 dùng kiểu khác hoặc bỏ chú thích"
            )
        } else {
            format!(
                "tên không tồn tại: kiểu `{name}` không có trong VS0 (VD-43) và không được khai trong tệp"
            )
        };
        self.diag(NO_TYPE, msg, span);
        Type::Error
    }

    // ----- duyệt chương trình -----

    fn run(&mut self, hir: &Hir) {
        self.quiet += 1;
        self.khai_bao_item(hir);
        self.quiet -= 1;
        for item in &hir.items {
            self.kiem_item(item);
        }
        for stmt in &hir.stmts {
            let t = self.kiem_stmt(stmt);
            if let HStmt::Expr(_) = stmt {
                self.file_expr_types.push(t);
            }
        }
    }

    /// Lượt 1 — chữ ký item vào môi trường (để gọi được hàm khai sau).
    fn khai_bao_item(&mut self, hir: &Hir) {
        for item in &hir.items {
            match item {
                HItem::Fn(f) => {
                    // ST-28: chữ ký giữ `TypeRef{"T"}` để hợp nhất ở call-site…
                    let luu = std::mem::take(&mut self.generic_scope);
                    self.generic_scope = f.generics.clone();
                    let raw_params: Vec<Type> = f
                        .params
                        .iter()
                        .map(|p| self.resolve_ann(&p.ty, p.span))
                        .collect();
                    let raw_ret = self.resolve_ann(&f.ret, f.span);
                    self.generic_scope = luu;
                    // …nhưng thân hàm và mọi chỗ khác dùng bản **đã xoá kiểu** (`{error}`).
                    // ST-35: sau khi xoá generic, `Hop[?]` phải quay về **bản ghi gốc** `Hop`
                    // (không để `TypeRef{Hop}` lửng ⇒ trường không tra được trong thân hàm).
                    let params: Vec<Type> = raw_params
                        .iter()
                        .map(|t| self.chuan(&erase_gen(t, &f.generics)))
                        .collect();
                    let ret = self.chuan(&erase_gen(&raw_ret, &f.generics));
                    let ty = Type::Fn(params.clone().into_boxed_slice(), Box::new(ret.clone()));
                    self.set_ty(f.sym, ty.clone());
                    self.item_types.push((f.name.clone(), ty));
                    if let Some(slot) = self.sigs.get_mut(f.sym as usize) {
                        *slot = Some(FnSig {
                            name: f.name.clone(),
                            params: f
                                .params
                                .iter()
                                .zip(params.iter())
                                .map(|(p, t)| {
                                    (p.name.clone(), t.clone(), p.default.is_some(), p.named)
                                })
                                .collect(),
                            ret,
                            variadic: f.params.last().map(|p| p.variadic).unwrap_or(false),
                            generics: f.generics.clone(),
                            raw_params,
                            raw_ret,
                        });
                    }
                }
                HItem::Rec(r) => {
                    let ty = Type::Record(r.id);
                    self.set_ty(r.sym, ty.clone());
                    self.item_types.push((r.name.clone(), ty));
                }
                HItem::Enum(e) => {
                    let ty = Type::Enum(e.id);
                    self.set_ty(e.sym, ty.clone());
                    self.item_types.push((e.name.clone(), ty));
                }
                HItem::Alias(a) => {
                    let ty = self.resolve_ann(&a.ty, a.span);
                    self.set_ty(a.sym, ty.clone());
                    self.item_types.push((a.name.clone(), ty));
                }
                HItem::Const(c) => {
                    let ty = self.resolve_ann(&c.ty, c.span);
                    self.set_ty(c.sym, ty.clone());
                    self.item_types.push((c.name.clone(), ty));
                }
                HItem::Extern(x) => {
                    let params: Vec<Type> = x
                        .params
                        .iter()
                        .map(|p| self.resolve_ann(&p.ty, p.span))
                        .collect();
                    let ty = Type::Fn(params.into_boxed_slice(), Box::new(Type::Error));
                    self.set_ty(x.sym, ty);
                }
                // **ST-56b (VD-97)**: thân phương thức `impl` kiểm **đích gán** (xem `kiem_impl`).
                HItem::Impl(im) => self.kiem_impl(im),
                HItem::Trait(_) | HItem::Use(_) => {}
            }
        }
    }

    fn kiem_item(&mut self, item: &HItem) {
        match item {
            HItem::Fn(f) => {
                // ST-28: trong thân hàm, tham số kiểu `T` là **đã xoá** (`{error}`) ⇒ phép toán trên
                // `T` được phép tĩnh, sai kiểu thật bắt **lúc chạy** (`VS0501`, nợ VS1b).
                let luu = std::mem::take(&mut self.generic_scope);
                self.generic_scope = f.generics.clone();
                for p in &f.params {
                    let raw = self.resolve_ann(&p.ty, p.span);
                    let ty = self.chuan(&erase_gen(&raw, &f.generics));
                    self.set_ty(p.sym, ty.clone());
                    if p.is_var_self {
                        self.set_mutable(p.sym);
                    }
                    if let Some(d) = &p.default {
                        let dty = self.kiem_expr(d);
                        self.coerce_e(d, &dty, &ty, "giá trị mặc định");
                    }
                }
                let raw_ret = self.resolve_ann(&f.ret, f.span);
                let ret = self.chuan(&erase_gen(&raw_ret, &f.generics));
                self.generic_scope = luu;
                let saved = self.cur_ret.replace(ret.clone());
                self.kiem_than(&f.body, &ret, f.span);
                self.cur_ret = saved;
            }
            HItem::Const(c) => {
                let want = self.resolve_ann(&c.ty, c.span);
                let got = self.kiem_expr(&c.value);
                self.coerce_e(&c.value, &got, &want, "hằng");
            }
            _ => {}
        }
    }

    /// Kiểu **giá trị** của một thân: kiểu biểu thức **cuối**, không có thì `()` (§5.1).
    pub(crate) fn than_value(&mut self, body: &[HStmt]) -> Type {
        let n = body.len();
        let mut value = Type::Unit;
        for (i, s) in body.iter().enumerate() {
            let t = self.kiem_stmt(s);
            if i + 1 == n {
                // §4/§6: câu lệnh cuối của khối **cho giá trị** (`Expr`, `if`/`match` có `else`,
                // khối con); các dạng khác là `()` — VD-47 (sửa lỗ hổng ST-13).
                value = t;
            }
        }
        value
    }

    /// Kiểm một **thân** (suite) và ép kiểu giá trị về `want`.
    pub(crate) fn kiem_than(&mut self, body: &[HStmt], want: &Type, span: Span) {
        let at = body.last().map(HStmt::span).unwrap_or(span);
        let t = self.than_value(body);
        if !want.is_error() {
            self.coerce(&t, want, at, "giá trị trả về");
        }
    }

    /// Kiểm một câu lệnh; trả "kiểu giá trị" (chỉ `HStmt::Expr` có giá trị).
    pub(crate) fn kiem_stmt(&mut self, s: &HStmt) -> Type {
        match s {
            HStmt::Let { pat, ty, init, .. } => {
                let got = self.kiem_expr(init);
                let declared = match ty {
                    Some(ann) => {
                        let ann = self.resolve_ann(ann, init.span());
                        self.coerce_e(init, &got, &ann, "khai báo `let`");
                        ann
                    }
                    None => got,
                };
                self.bind_pat(pat, &declared);
                Type::Unit
            }
            HStmt::Var {
                name,
                span,
                sym,
                ty,
                init,
                ..
            } => {
                let got = self.kiem_expr(init);
                let declared = match ty {
                    Some(ann) => {
                        let ann = self.resolve_ann(ann, init.span());
                        self.coerce_e(init, &got, &ann, "khai báo `var`");
                        ann
                    }
                    None => got,
                };
                let _ = name;
                self.set_ty(*sym, declared);
                self.set_mutable(*sym);
                let _ = span;
                Type::Unit
            }
            HStmt::Assign {
                target,
                op,
                value,
                span,
            } => {
                let wanted = self.kiem_dich(target, *op);
                let got = self.kiem_expr(value);
                let _ = span;
                match op {
                    crate::hir::AssignOp::Set => {
                        self.coerce_e(value, &got, &wanted, "gán");
                    }
                    _ => {
                        // `+=` … ⇒ kiểm như `target + value` rồi ép về kiểu đích.
                        let op2 = match op {
                            crate::hir::AssignOp::Add => crate::hir::BinOp::Add,
                            crate::hir::AssignOp::Sub => crate::hir::BinOp::Sub,
                            crate::hir::AssignOp::Mul => crate::hir::BinOp::Mul,
                            crate::hir::AssignOp::Div => crate::hir::BinOp::Div,
                            _ => crate::hir::BinOp::Rem,
                        };
                        let v_lit = is_int_lit(value);
                        if let Some(res) = num::so_hoc(op2, &wanted, &got, false, v_lit) {
                            self.coerce(&res, &wanted, value.span(), "gán ghép");
                        } else {
                            self.coerce_e(value, &got, &wanted, "gán ghép");
                        }
                    }
                }
                Type::Unit
            }
            HStmt::Expr(e) => self.kiem_expr(e),
            HStmt::If {
                cond,
                then,
                elifs,
                else_,
                span,
            } => {
                // VD-47: `if` ở vị trí câu lệnh **vẫn cho giá trị** (`if x: 1 else: 2` là `int`);
                // không có `else` ⇒ `()`.
                self.kiem_cond(cond);
                let mut acc = self.than_value(then);
                for (c, body) in elifs {
                    self.kiem_cond(c);
                    let t = self.than_value(body);
                    self.unify_branch(&mut acc, &t, *span);
                }
                match else_ {
                    Some(e) if !e.is_empty() => {
                        let t = self.than_value(e);
                        self.unify_branch(&mut acc, &t, *span);
                        acc
                    }
                    _ => Type::Unit,
                }
            }
            HStmt::For {
                pat, iter, body, ..
            } => {
                let it = self.kiem_expr(iter);
                let elem = self.elem_of_iter(&it, iter.span());
                self.bind_pat(pat, &elem);
                // ST-13b (mục 3): thân vòng lặp **bỏ** giá trị cuối, không ép `unit`.
                self.kiem_than_bo(body);
                Type::Unit
            }
            HStmt::While { cond, body, .. } => {
                let t = self.kiem_expr(cond);
                self.coerce(&t, &Type::Bool, cond.span(), "điều kiện `while`");
                self.kiem_than_bo(body);
                Type::Unit
            }
            HStmt::Loop { body, .. } => {
                self.kiem_than_bo(body);
                Type::Unit
            }
            HStmt::Scope { body, .. } => {
                // ST-13b (mục 3): `scope` **cho giá trị khối** — thống nhất với interp.
                self.than_value(body)
            }
            HStmt::Return { value, span } => {
                let want = self.cur_ret.clone().unwrap_or(Type::Error);
                match value {
                    Some(e) => {
                        let t = self.kiem_expr(e);
                        if !want.is_error() {
                            self.coerce(&t, &want, *span, "`return`");
                        }
                    }
                    None => {
                        if !want.is_error() && want != Type::Unit {
                            self.coerce(&Type::Unit, &want, *span, "`return`");
                        }
                    }
                }
                // ST-13b (mục 1): `return` **phân kỳ** — kiểu `never`, nhận mọi kiểu trả về.
                Type::Never
            }
            HStmt::Break { .. } | HStmt::Continue { .. } => Type::Unit,
            HStmt::Match {
                subject,
                arms,
                span,
            } => {
                let subj = self.kiem_expr(subject);
                let mut acc: Option<Type> = None;
                for arm in arms {
                    self.bind_pat(&arm.pat, &subj);
                    if let Some(g) = &arm.guard {
                        let gt = self.kiem_expr(g);
                        self.coerce(&gt, &Type::Bool, g.span(), "guard `if`");
                    }
                    let t = self.than_value(&arm.body);
                    match &mut acc {
                        None => acc = Some(t),
                        Some(a) => self.unify_branch(a, &t, arm.span),
                    }
                }
                self.xet_het(&subj, arms, *span);
                // VD-47: `match` ở vị trí câu lệnh vẫn cho giá trị (hợp nhất các arm).
                acc.unwrap_or(Type::Unit)
            }
            HStmt::Block { body, .. } => {
                // VD-47: khối con ở đuôi **cho giá trị** của nó.
                self.than_value(body)
            }
        }
    }

    /// Kiểu phần tử khi gán/chỉ số (gọi lại `kiem_index` của `expr.rs`).
    pub(crate) fn kiem_index_expr(&mut self, base: &HExpr, index: &HExpr, span: Span) -> Type {
        self.kiem_index(base, index, span)
    }

    /// Kiểu phần tử khi `for` duyệt `iter`.
    fn elem_of_iter(&mut self, it: &Type, span: Span) -> Type {
        match it {
            Type::List(t) => (**t).clone(),
            Type::Map(k, v) => Type::TypeRef {
                name: "tuple".into(),
                args: vec![(**k).clone(), (**v).clone()],
            },
            Type::Str => Type::Char,
            Type::TypeRef { name, args } if name == "range" => {
                // ST-13b (mục 4): VS0 chỉ duyệt range **`int`**; `size`/`duration`/`char` ⇒ VS0412
                // (tyck và runtime phải khớp — runtime chưa hỗ trợ range đơn vị).
                match args.first() {
                    Some(Type::Int) | None => Type::Int,
                    Some(Type::Error) => Type::Error,
                    Some(other) => {
                        let o = other.clone();
                        self.diag(
                            MISMATCH,
                            format!(
                                "`for` chỉ duyệt được range `int` ở VS0, được `{}` (§A.7)",
                                self.show(&o)
                            ),
                            span,
                        );
                        Type::Error
                    }
                }
            }
            Type::Error => Type::Error,
            _ => Type::Error,
        }
    }

    /// Kiểm thân khối **bỏ giá trị cuối** (vòng lặp — ST-13b mục 3).
    pub(crate) fn kiem_than_bo(&mut self, body: &[HStmt]) {
        for s in body {
            self.kiem_stmt(s);
        }
    }

    /// Kiểu của một **đích gán** (chỉ đọc, không báo lỗi gán-cho-`let`).
    fn kiem_dich(&mut self, target: &HExpr, op: crate::hir::AssignOp) -> Type {
        match target {
            HExpr::Var {
                sym: Some(id),
                span,
                ..
            } => {
                let ty = self.ty_of(*id);
                if !self.is_mutable(*id) && !ty.is_error() {
                    let kind = self.symbols.get(*id).map(|s| s.kind);
                    if !matches!(kind, Some(SymKind::Var)) {
                        self.diag(
                            MISMATCH,
                            "không gán được cho `let`/tham số (chỉ `var` mới gán được, §4.1)",
                            *span,
                        );
                    }
                }
                ty
            }
            HExpr::Var { sym: None, .. } => Type::Error,
            HExpr::Field {
                base, name, span, ..
            } => {
                // **ST-56b (VD-97)**: `x.f = v` chỉ hợp lệ khi `x` là biến **`var`** (kể cả
                // `var self`) — hạ **rebind cục bộ** `x = x with f: v`; `let`/tham số (kể cả
                // `self` **không** `var`) là **bất biến** ⇒ `VS0412` tại **phép gán**.
                // Gán ghép `+=` trên trường **chưa** hỗ trợ ⇒ cũng `VS0412` (tyck chặn trước để
                // hai đường không lệch — codegen/interp **không** phải đoán).
                let mutable = match &**base {
                    HExpr::Var { sym: Some(id), .. } => self.is_mutable(*id),
                    _ => false,
                };
                let bt = self.kiem_expr(base);
                if !matches!(op, crate::hir::AssignOp::Set) {
                    self.diag(
                        MISMATCH,
                        "chỉ gán được `=` cho trường/phần tử ở VS0 (gán ghép `+=` … trên trường chưa hỗ trợ, §4.1)",
                        *span,
                    );
                    return Type::Error;
                }
                if mutable {
                    // Đích **hợp lệ**: trường phải có (`VS0405`) và kiểu trường là kiểu đích.
                    return match &bt {
                        Type::Record(id) => match self.types.field(*id, name) {
                            Some(f) => f.ty.clone(),
                            None => {
                                self.diag(
                                    vis_diag::ma_truong(),
                                    format!("bản ghi không có trường `{name}` (VD-43 #6)"),
                                    *span,
                                );
                                Type::Error
                            }
                        },
                        Type::Error => Type::Error,
                        other => {
                            self.diag(
                                MISMATCH,
                                format!(
                                    "không gán được trường của `{}` ở VS0 (§4.1)",
                                    self.types.show(other)
                                ),
                                *span,
                            );
                            Type::Error
                        }
                    };
                }
                match &bt {
                    Type::Error => {
                        // Kiểu chưa rõ (vd `self` — tham số **bất biến**) ⇒ vẫn `VS0412` (VD-97 (1)).
                        self.diag(
                            MISMATCH,
                            "không gán được trường của `let`/tham số — chỉ `var` mới gán được (§4.1)",
                            *span,
                        );
                        Type::Error
                    }
                    Type::Record(_) => {
                        self.diag(
                            MISMATCH,
                            "bản ghi là **bất biến** — dùng `with` để tạo bản sao rồi gán lại `var` (§3.2/§4.1)",
                            *span,
                        );
                        Type::Error
                    }
                    other => {
                        self.diag(
                            MISMATCH,
                            format!(
                                "không gán được trường của `{}` ở VS0 (§4.1)",
                                self.types.show(other)
                            ),
                            *span,
                        );
                        Type::Error
                    }
                }
            }
            HExpr::Index { base, index, span } => {
                // **ST-56b (VD-97)**: chỉ gán được phần tử của biến **`var`** (list/map; rebind
                // giá trị). Gán ghép `+=` trên phần tử **chưa** hỗ trợ ⇒ `VS0412` ở tyck.
                if !matches!(op, crate::hir::AssignOp::Set) {
                    self.diag(
                        MISMATCH,
                        "chỉ gán được `=` cho trường/phần tử ở VS0 (gán ghép `+=` … trên phần tử chưa hỗ trợ, §4.1)",
                        *span,
                    );
                    return Type::Error;
                }
                let _ = self.kiem_expr(base); // vẫn kiểm chẩn đoán của biểu thức gốc
                let mutable = match &**base {
                    HExpr::Var { sym: Some(id), .. } => self.is_mutable(*id),
                    _ => false,
                };
                if !mutable {
                    self.diag(
                        MISMATCH,
                        "chỉ gán được phần tử của `var` — `let`/tham số là bất biến (§4.1)",
                        *span,
                    );
                }
                self.kiem_index_expr(base, index, *span)
            }
            _ => Type::Error,
        }
    }

    /// **ST-56b (VD-97)**: thân **phương thức `impl`** (khung VS1) **chưa** được tyck đầy đủ — nhưng
    /// **phép gán** phải chặn **trước** để interp ↔ `--vm` không lệch:
    ///   * `self` **không** `var` là **tham số bất biến** ⇒ `self.x = v` / `self[i] = v` ⇒ **`VS0412`**
    ///     tại **phép gán**; `var self` **được** (rebind cục bộ, VD-97 (2)).
    ///   * `let`/tham số thường bất biến; `var` (kể cả biến cục bộ trong thân) **được**.
    ///   * gán ghép `+=` trên trường/phần tử **chưa** hỗ trợ ⇒ `VS0412`.
    ///
    /// Chỉ kiểm **đích gán**; kiểm kiểu **đầy đủ** thân `impl` (cần dispatch phương thức trong tyck)
    /// vẫn là **nợ riêng**, ghi JOURNAL.
    fn kiem_impl(&mut self, im: &HImpl) {
        for m in &im.methods {
            let mut duoc_gan: HashSet<u32> = HashSet::new();
            for p in &m.params {
                if p.is_var_self {
                    duoc_gan.insert(p.sym);
                }
            }
            self.kiem_gan_than(&m.body, &mut duoc_gan);
        }
    }

    /// Duyệt thân phương thức, chỉ kiểm **đích gán**; theo dõi biến `var` để **không** báo oan.
    fn kiem_gan_than(&mut self, body: &[HStmt], duoc_gan: &mut HashSet<u32>) {
        for s in body {
            match s {
                HStmt::Var { sym, .. } => {
                    duoc_gan.insert(*sym);
                }
                HStmt::Assign {
                    target, op, span, ..
                } => {
                    let goc_la_var = |t: &HExpr| match t {
                        HExpr::Var { sym: Some(id), .. } => duoc_gan.contains(id),
                        _ => false,
                    };
                    let gan_ghep = !matches!(op, crate::hir::AssignOp::Set);
                    match target {
                        HExpr::Var { sym: Some(id), .. } => {
                            if !duoc_gan.contains(id) {
                                self.diag(
                                    MISMATCH,
                                    "không gán được cho `let`/tham số (chỉ `var` mới gán được, §4.1)",
                                    *span,
                                );
                            }
                        }
                        HExpr::Field { base, .. } => {
                            if gan_ghep {
                                self.diag(MISMATCH, GAN_GHEP, *span);
                            } else if !goc_la_var(base) {
                                self.diag(MISMATCH, GAN_TRUONG_LET, *span);
                            }
                        }
                        HExpr::Index { base, .. } => {
                            if gan_ghep {
                                self.diag(MISMATCH, GAN_GHEP, *span);
                            } else if !goc_la_var(base) {
                                self.diag(MISMATCH, GAN_PHAN_TU_LET, *span);
                            }
                        }
                        _ => {}
                    }
                }
                HStmt::If {
                    then, elifs, else_, ..
                } => {
                    self.kiem_gan_than(then, duoc_gan);
                    for (_, b) in elifs {
                        self.kiem_gan_than(b, duoc_gan);
                    }
                    if let Some(b) = else_ {
                        self.kiem_gan_than(b, duoc_gan);
                    }
                }
                HStmt::For { body, .. }
                | HStmt::While { body, .. }
                | HStmt::Loop { body, .. }
                | HStmt::Scope { body, .. }
                | HStmt::Block { body, .. } => self.kiem_gan_than(body, duoc_gan),
                HStmt::Match { arms, .. } => {
                    for a in arms {
                        self.kiem_gan_than(&a.body, duoc_gan);
                    }
                }
                _ => {}
            }
        }
    }
}

/// **ST-56b (VD-97)**: thông điệp gán ghép trên trường/phần tử (dùng chung cho thân `impl`).
const GAN_GHEP: &str =
    "chỉ gán được `=` cho trường/phần tử ở VS0 (gán ghép `+=` … trên trường chưa hỗ trợ, §4.1)";
/// Thông điệp gán **trường** của `let`/tham số.
const GAN_TRUONG_LET: &str =
    "không gán được trường của `let`/tham số — chỉ `var` mới gán được (§4.1)";
/// Thông điệp gán **phần tử** của `let`/tham số.
const GAN_PHAN_TU_LET: &str = "chỉ gán được phần tử của `var` — `let`/tham số là bất biến (§4.1)";

/// Kiểu **không thuộc VS0** (VD-48(a)) — vẫn `VS0301` nhưng thông điệp nêu rõ vis-std/vòng 2.
fn la_kieu_ngoai_vs0(name: &str) -> bool {
    matches!(
        name,
        "AnyError"
            | "cap"
            | "ref"
            | "weak"
            | "Json"
            | "set"
            | "stream"
            | "Future"
            | "Task"
            | "datetime"
            | "bigint"
            | "decimal"
    )
}
