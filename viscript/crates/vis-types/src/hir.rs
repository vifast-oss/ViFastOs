// SPDX-License-Identifier: MIT OR Apache-2.0
//! HIR của **chế độ script** (ST-11) — mặt nạ *đã hạ* từ CST, giữ `Span`, đã giải tên **giá trị**
//! và đã điền kiểu cho literal (VD-44 #1: hạ **permissive**, biểu diễn được mọi thứ ở Phụ lục A;
//! giới hạn VS0 chỉ báo ở tầng kiểm kiểu).
//!
//! # Quy ước sexp HIR
//!
//! `sexp` là **dạng ngoặc hoá** dùng cho test/log, cùng quy ước với VD-34 (`golden.expr`) — chỉ
//! khác **một** điểm: HIR **không có** nút `paren` (ngoặc tròn là chuyện của CST), nên
//! `(1 + 2) * 3` ở CST là `(* (paren (+ 1 2)) 3)` thì ở HIR là `(* (+ 1 2) 3)`.
//! Literal in **nguyên văn nguồn** (`1`, `"a"`, `50pct`, `true`); các toán tử giữ nguyên chữ viết.

use std::fmt;

use vis_syntax::Span;

use crate::ty::{EnumId, RecId, Type};

/// Chương trình đã hạ: **item** (khai báo cấp tệp, được "treo" lên trước — §14.3 "khai báo
/// trước, thân sau") và **câu lệnh cấp tệp** theo đúng thứ tự nguồn.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Hir {
    pub items: Vec<HItem>,
    pub stmts: Vec<HStmt>,
    pub span: Span,
}

/// Khai báo cấp tệp.
#[derive(Debug, Clone, PartialEq)]
pub enum HItem {
    Fn(HFn),
    Rec(HRec),
    Enum(HEnum),
    Alias(HAlias),
    Extern(HSig),
    Const(HConst),
    Use(HUse),
    /// `trait` — khung **VS1** (ST-11 chỉ giữ chữ ký, chưa kiểm).
    Trait(HTrait),
    /// `impl` — khung **VS1**.
    Impl(HImpl),
}

impl HItem {
    /// Tên khai báo (nếu có).
    pub fn name(&self) -> Option<&str> {
        match self {
            HItem::Fn(f) => Some(&f.name),
            HItem::Rec(r) => Some(&r.name),
            HItem::Enum(e) => Some(&e.name),
            HItem::Alias(a) => Some(&a.name),
            HItem::Extern(x) => Some(&x.name),
            HItem::Const(c) => Some(&c.name),
            HItem::Trait(t) => Some(&t.name),
            HItem::Impl(_) | HItem::Use(_) => None,
        }
    }

    /// Mã (chỉ số) trong bảng ký hiệu.
    pub fn sym(&self) -> Option<u32> {
        match self {
            HItem::Fn(f) => Some(f.sym),
            HItem::Rec(r) => Some(r.sym),
            HItem::Enum(e) => Some(e.sym),
            HItem::Alias(a) => Some(a.sym),
            HItem::Extern(x) => Some(x.sym),
            HItem::Const(c) => Some(c.sym),
            HItem::Trait(t) => Some(t.sym),
            HItem::Impl(_) | HItem::Use(_) => None,
        }
    }

    /// Span của khai báo.
    pub fn span(&self) -> Span {
        match self {
            HItem::Fn(f) => f.span,
            HItem::Rec(r) => r.span,
            HItem::Enum(e) => e.span,
            HItem::Alias(a) => a.span,
            HItem::Extern(x) => x.span,
            HItem::Const(c) => c.span,
            HItem::Use(u) => u.span,
            HItem::Trait(t) => t.span,
            HItem::Impl(i) => i.span,
        }
    }

    /// Sexp để test/log.
    pub fn sexp(&self) -> String {
        match self {
            HItem::Fn(f) => f.sexp(),
            HItem::Rec(r) => format!("(rec {})", r.name),
            HItem::Enum(e) => format!("(enum {})", e.name),
            HItem::Alias(a) if a.is_newtype => format!("(newtype {} = {})", a.name, a.ty),
            HItem::Alias(a) => format!("(alias {} = {})", a.name, a.ty),
            HItem::Extern(x) => format!("(extern {})", x.name),
            HItem::Const(c) => format!("(const {} = {})", c.name, c.value.sexp()),
            HItem::Use(u) => format!("(use {})", u.path.join(".")),
            HItem::Trait(t) => format!("(trait {})", t.name),
            HItem::Impl(i) => {
                let head = match &i.trait_name {
                    Some(t) => format!("{t} for {}", i.ty.name()),
                    None => i.ty.name(),
                };
                format!("(impl {head})")
            }
        }
    }
}

/// Một hàm/phương thức/khai báo `cmd`/`test fn`.
#[derive(Debug, Clone, PartialEq)]
pub struct HFn {
    pub name: String,
    pub span: Span,
    pub sym: u32,
    pub kind: FnKind,
    pub is_pub: bool,
    pub is_async: bool,
    /// Tên tham số kiểu (`fn first[T](…)`); **chưa** kiểm (generic là VS1).
    pub generics: Vec<String>,
    pub params: Vec<HParam>,
    /// Kiểu trả về; không ghi ⇒ `()`.
    pub ret: Type,
    pub body: Vec<HStmt>,
}

impl HFn {
    fn sexp(&self) -> String {
        let params: Vec<String> = self.params.iter().map(|p| p.name.clone()).collect();
        format!("(fn {}({}) -> {})", self.name, params.join(" "), self.ret)
    }
}

/// Loại hàm — dùng chung một `HFn` cho cả 4 dạng khai báo (ít trùng lặp, dễ duyệt).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FnKind {
    Fn,
    Cmd,
    Test,
    TraitMethod,
    ImplMethod,
}

/// Tham số hình thức (vị trí, `--tên`, `self`, `var self`).
#[derive(Debug, Clone, PartialEq)]
pub struct HParam {
    pub name: String,
    pub span: Span,
    pub sym: u32,
    pub ty: Type,
    /// `--ten` — chỉ truyền theo tên / là cờ ở chế độ lệnh.
    pub named: bool,
    pub is_self: bool,
    pub is_var_self: bool,
    pub default: Option<HExpr>,
}

/// `extern cmd name(params)` — chưa có thân.
#[derive(Debug, Clone, PartialEq)]
pub struct HSig {
    pub name: String,
    pub span: Span,
    pub sym: u32,
    pub params: Vec<HParam>,
}

/// Trường của bản ghi/enum (đã hạ kiểu + giá trị mặc định).
#[derive(Debug, Clone, PartialEq)]
pub struct HField {
    pub name: String,
    pub span: Span,
    pub ty: Type,
    pub default: Option<HExpr>,
    pub has_default: bool,
}

/// `type Name derives …: field+`.
#[derive(Debug, Clone, PartialEq)]
pub struct HRec {
    pub id: RecId,
    pub name: String,
    pub span: Span,
    pub sym: u32,
    pub fields: Vec<HField>,
    pub derives: Vec<String>,
}

/// `enum Name derives …: variant+`.
#[derive(Debug, Clone, PartialEq)]
pub struct HEnum {
    pub id: EnumId,
    pub name: String,
    pub span: Span,
    pub sym: u32,
    pub variants: Vec<HVariant>,
    pub derives: Vec<String>,
}

/// Một biến thể enum.
#[derive(Debug, Clone, PartialEq)]
pub struct HVariant {
    pub name: String,
    pub span: Span,
    pub fields: Vec<HField>,
}

/// `alias Name = type` (`newtype` cùng cấu trúc, `is_newtype = true`).
#[derive(Debug, Clone, PartialEq)]
pub struct HAlias {
    pub name: String,
    pub span: Span,
    pub sym: u32,
    pub ty: Type,
    pub is_newtype: bool,
}

/// `const Name: type = expr`.
#[derive(Debug, Clone, PartialEq)]
pub struct HConst {
    pub name: String,
    pub span: Span,
    pub sym: u32,
    pub ty: Type,
    pub value: HExpr,
}

/// `use a.b.c` (module là VS1 — ST-11 chỉ ghi lại đường dẫn).
#[derive(Debug, Clone, PartialEq)]
pub struct HUse {
    pub path: Vec<String>,
    pub span: Span,
}

/// Khung `trait` (VS1): chữ ký phương thức, thân có thể rỗng.
#[derive(Debug, Clone, PartialEq)]
pub struct HTrait {
    pub name: String,
    pub span: Span,
    pub sym: u32,
    pub supertraits: Vec<String>,
    pub methods: Vec<HFn>,
}

/// Khung `impl` (VS1).
#[derive(Debug, Clone, PartialEq)]
pub struct HImpl {
    pub trait_name: Option<String>,
    pub ty: Type,
    pub span: Span,
    pub methods: Vec<HFn>,
}

/// Câu lệnh.
#[derive(Debug, Clone, PartialEq)]
pub enum HStmt {
    Let {
        pat: HPat,
        ty: Option<Type>,
        init: HExpr,
        else_: Option<HElse>,
        span: Span,
    },
    Var {
        name: String,
        span: Span,
        sym: u32,
        ty: Option<Type>,
        init: HExpr,
        is_self: bool,
    },
    Assign {
        target: HExpr,
        op: AssignOp,
        value: HExpr,
        span: Span,
    },
    Expr(HExpr),
    If {
        cond: HCond,
        then: Vec<HStmt>,
        elifs: Vec<(HCond, Vec<HStmt>)>,
        else_: Option<Vec<HStmt>>,
        span: Span,
    },
    For {
        pat: HPat,
        iter: HExpr,
        body: Vec<HStmt>,
        span: Span,
    },
    While {
        cond: HExpr,
        body: Vec<HStmt>,
        span: Span,
    },
    Loop {
        body: Vec<HStmt>,
        span: Span,
    },
    Scope {
        label: Option<String>,
        body: Vec<HStmt>,
        span: Span,
    },
    Return {
        value: Option<HExpr>,
        span: Span,
    },
    Break {
        span: Span,
    },
    Continue {
        span: Span,
    },
    Match {
        subject: HExpr,
        arms: Vec<HArm>,
        span: Span,
    },
    /// Khối lồng không gắn với cấu trúc điều khiển (giữ chỗ cho ST-12/13).
    Block {
        body: Vec<HStmt>,
        span: Span,
    },
}

impl HStmt {
    pub fn span(&self) -> Span {
        match self {
            HStmt::Let { span, .. }
            | HStmt::Var { span, .. }
            | HStmt::Assign { span, .. }
            | HStmt::If { span, .. }
            | HStmt::For { span, .. }
            | HStmt::While { span, .. }
            | HStmt::Loop { span, .. }
            | HStmt::Scope { span, .. }
            | HStmt::Return { span, .. }
            | HStmt::Break { span }
            | HStmt::Continue { span }
            | HStmt::Match { span, .. }
            | HStmt::Block { span, .. } => *span,
            HStmt::Expr(e) => e.span(),
        }
    }

    pub fn sexp(&self) -> String {
        match self {
            HStmt::Let { pat, init, .. } => format!("(let {} {})", pat.sexp(), init.sexp()),
            HStmt::Var { name, init, .. } => format!("(var {name} {})", init.sexp()),
            HStmt::Assign {
                target, op, value, ..
            } => format!("({} {} {})", op.text(), target.sexp(), value.sexp()),
            HStmt::Expr(e) => e.sexp(),
            HStmt::If { cond, .. } => format!("(if {})", cond.sexp()),
            HStmt::For { pat, iter, .. } => format!("(for {} {})", pat.sexp(), iter.sexp()),
            HStmt::While { cond, .. } => format!("(while {})", cond.sexp()),
            HStmt::Loop { .. } => "(loop)".into(),
            HStmt::Scope { label, .. } => match label {
                Some(l) => format!("(scope {l})"),
                None => "(scope)".into(),
            },
            HStmt::Return { value, .. } => match value {
                Some(v) => format!("(return {})", v.sexp()),
                None => "(return)".into(),
            },
            HStmt::Break { .. } => "(break)".into(),
            HStmt::Continue { .. } => "(continue)".into(),
            HStmt::Match { subject, .. } => format!("(match {})", subject.sexp()),
            HStmt::Block { .. } => "(block)".into(),
        }
    }
}

/// Nhánh `else` của `let` (giá trị dự phòng khi lỗi, §A.2).
#[derive(Debug, Clone, PartialEq)]
pub struct HElse {
    /// `else err: …` — tên nhận lỗi (khai trong khối `else`).
    pub bind: Option<String>,
    pub bind_sym: Option<u32>,
    pub body: Vec<HStmt>,
    pub span: Span,
}

/// Toán tử gán.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignOp {
    Set,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

impl AssignOp {
    pub const fn text(self) -> &'static str {
        match self {
            AssignOp::Set => "=",
            AssignOp::Add => "+=",
            AssignOp::Sub => "-=",
            AssignOp::Mul => "*=",
            AssignOp::Div => "/=",
            AssignOp::Rem => "%=",
        }
    }
}

/// Một arm của `match` (§A.2 `match_arm := pattern ['if' expr] ':' suite`).
#[derive(Debug, Clone, PartialEq)]
pub struct HArm {
    pub pat: HPat,
    pub guard: Option<HExpr>,
    pub body: Vec<HStmt>,
    pub span: Span,
}

/// Điều kiện `if`/`elif`: `if let PAT = EXPR` hoặc biểu thức thường (§A.2 `if_cond`).
#[derive(Debug, Clone, PartialEq)]
pub enum HCond {
    Expr(HExpr),
    Let { pat: HPat, value: HExpr, span: Span },
}

impl HCond {
    pub fn sexp(&self) -> String {
        match self {
            HCond::Expr(e) => e.sexp(),
            HCond::Let { pat, value, .. } => format!("(if-let {} {})", pat.sexp(), value.sexp()),
        }
    }
}

/// Pattern (§A.4).
#[derive(Debug, Clone, PartialEq)]
pub enum HPat {
    Wild {
        span: Span,
    },
    Bind {
        name: String,
        span: Span,
        sym: u32,
    },
    Lit {
        lit: Lit,
        span: Span,
    },
    /// Biến thể enum / constructor: `Circle(r)`, `some(x)`, `ok(v)`, `Empty`, `none`.
    Ctor {
        path: Vec<String>,
        args: Vec<HPat>,
        span: Span,
    },
    Tuple {
        items: Vec<HPat>,
        span: Span,
    },
    List {
        items: Vec<HPat>,
        rest: Option<Box<HPat>>,
        span: Span,
    },
    Record {
        name: Option<String>,
        fields: Vec<(String, Option<HPat>)>,
        open: bool,
        span: Span,
    },
    Range {
        lo: Option<Box<HPat>>,
        hi: Option<Box<HPat>>,
        inclusive: bool,
        span: Span,
    },
    Or {
        alts: Vec<HPat>,
        span: Span,
    },
    /// `..` / `..rest` trong pattern list/bản ghi.
    Rest {
        name: Option<String>,
        span: Span,
    },
}

impl HPat {
    pub fn span(&self) -> Span {
        match self {
            HPat::Wild { span }
            | HPat::Bind { span, .. }
            | HPat::Lit { span, .. }
            | HPat::Ctor { span, .. }
            | HPat::Tuple { span, .. }
            | HPat::List { span, .. }
            | HPat::Record { span, .. }
            | HPat::Range { span, .. }
            | HPat::Or { span, .. }
            | HPat::Rest { span, .. } => *span,
        }
    }

    pub fn sexp(&self) -> String {
        match self {
            HPat::Wild { .. } => "_".into(),
            HPat::Bind { name, .. } => name.clone(),
            HPat::Lit { lit, .. } => lit.text.clone(),
            HPat::Ctor { path, args, .. } => {
                if args.is_empty() {
                    path.join(".")
                } else {
                    let a: Vec<String> = args.iter().map(HPat::sexp).collect();
                    format!("({} {})", path.join("."), a.join(" "))
                }
            }
            HPat::Tuple { items, .. } => {
                let a: Vec<String> = items.iter().map(HPat::sexp).collect();
                format!("(tuple {})", a.join(" "))
            }
            HPat::List { items, rest, .. } => {
                let mut a: Vec<String> = items.iter().map(HPat::sexp).collect();
                if let Some(r) = rest {
                    a.push(r.sexp());
                }
                format!("(list {})", a.join(" "))
            }
            HPat::Record { fields, open, .. } => {
                let mut a: Vec<String> = fields
                    .iter()
                    .map(|(n, p)| match p {
                        Some(p) => format!("{n}: {}", p.sexp()),
                        None => n.clone(),
                    })
                    .collect();
                if *open {
                    a.push("..".into());
                }
                format!("(record {})", a.join(" "))
            }
            HPat::Range {
                lo, hi, inclusive, ..
            } => {
                let l = lo.as_ref().map(|b| b.sexp()).unwrap_or_default();
                let h = hi.as_ref().map(|b| b.sexp()).unwrap_or_default();
                format!("({} {l} {h})", if *inclusive { "..=" } else { ".." })
            }
            HPat::Or { alts, .. } => {
                let a: Vec<String> = alts.iter().map(HPat::sexp).collect();
                format!("(or {})", a.join(" "))
            }
            HPat::Rest { name, .. } => match name {
                Some(n) => format!("..{n}"),
                None => "..".into(),
            },
        }
    }
}

/// Biểu thức.
#[derive(Debug, Clone, PartialEq)]
pub enum HExpr {
    /// Literal **đã điền kiểu** (int/size/duration/percent/str/bool/bytes/path/char/float).
    Lit { lit: Lit, ty: Type, span: Span },
    /// Tên giá trị; `sym = None` khi chưa phân giải được (VD-44 #2: hạ permissive).
    Var {
        name: String,
        sym: Option<u32>,
        span: Span,
    },
    Bin {
        op: BinOp,
        lhs: Box<HExpr>,
        rhs: Box<HExpr>,
        span: Span,
    },
    Un {
        op: UnOp,
        expr: Box<HExpr>,
        span: Span,
    },
    Call {
        callee: Box<HExpr>,
        args: Vec<HExpr>,
        /// `--ten: giá_trị` (§A.3) — giữ thứ tự viết.
        named: Vec<(String, HExpr)>,
        span: Span,
    },
    Field {
        base: Box<HExpr>,
        name: String,
        /// `?.` (khác `?` postfix).
        optional: bool,
        span: Span,
    },
    Index {
        base: Box<HExpr>,
        index: Box<HExpr>,
        span: Span,
    },
    List {
        items: Vec<HExpr>,
        elem: Type,
        span: Span,
    },
    Set {
        items: Vec<HExpr>,
        elem: Type,
        span: Span,
    },
    Map {
        entries: Vec<(HExpr, HExpr)>,
        key: Type,
        value: Type,
        span: Span,
    },
    Record {
        fields: Vec<(String, HExpr)>,
        ty: Type,
        span: Span,
    },
    /// `Shape.Circle(1.0)` / `Circle(1.0)` khi tên là biến thể của enum **đã khai trong tệp**.
    EnumLit {
        enum_id: Option<EnumId>,
        path: Vec<String>,
        variant: String,
        args: Vec<HExpr>,
        ty: Type,
        span: Span,
    },
    /// `none` (value = `None`) và tự nâng `T → T?` (§4.4).
    Opt {
        value: Option<Box<HExpr>>,
        ty: Type,
        span: Span,
    },
    /// `expr?` — `?` postfix.
    Try { expr: Box<HExpr>, span: Span },
    /// `expr with ten: giá_trị …`.
    With {
        base: Box<HExpr>,
        fields: Vec<(String, HExpr)>,
        span: Span,
    },
    If {
        cond: Box<HCond>,
        then: Vec<HStmt>,
        elifs: Vec<(HCond, Vec<HStmt>)>,
        else_: Vec<HStmt>,
        ty: Type,
        span: Span,
    },
    Match {
        subject: Box<HExpr>,
        arms: Vec<HArm>,
        ty: Type,
        span: Span,
    },
    Lambda {
        params: Vec<HParam>,
        ret: Type,
        body: HBody,
        span: Span,
    },
    Tuple {
        items: Vec<HExpr>,
        ty: Type,
        span: Span,
    },
    /// `ref(expr)` — `ref[T]` là VS1, ST-11 chỉ giữ nút.
    Ref { expr: Box<HExpr>, span: Span },
    /// `await postfix` — async là VS1.
    Await { expr: Box<HExpr>, span: Span },
    /// `cmd { … }` — **chưa định kiểu** (VD-22: `cmd_lower` là VS1); giữ nguyên văn.
    CmdBlock { raw: String, span: Span },
    /// Biểu thức không hạ được (giữ chỗ, **không** panic — VD-44 #6).
    Error { span: Span },
}

impl HExpr {
    pub fn span(&self) -> Span {
        match self {
            HExpr::Lit { span, .. }
            | HExpr::Var { span, .. }
            | HExpr::Bin { span, .. }
            | HExpr::Un { span, .. }
            | HExpr::Call { span, .. }
            | HExpr::Field { span, .. }
            | HExpr::Index { span, .. }
            | HExpr::List { span, .. }
            | HExpr::Set { span, .. }
            | HExpr::Map { span, .. }
            | HExpr::Record { span, .. }
            | HExpr::EnumLit { span, .. }
            | HExpr::Opt { span, .. }
            | HExpr::Try { span, .. }
            | HExpr::With { span, .. }
            | HExpr::If { span, .. }
            | HExpr::Match { span, .. }
            | HExpr::Lambda { span, .. }
            | HExpr::Tuple { span, .. }
            | HExpr::Ref { span, .. }
            | HExpr::Await { span, .. }
            | HExpr::CmdBlock { span, .. }
            | HExpr::Error { span } => *span,
        }
    }

    /// Sexp theo quy ước ở đầu tệp (VD-34 **không** nút `paren`).
    pub fn sexp(&self) -> String {
        match self {
            HExpr::Lit { lit, .. } => lit.text.clone(),
            HExpr::Var { name, .. } => name.clone(),
            HExpr::Bin { op, lhs, rhs, .. } => {
                format!("({} {} {})", op.text(), lhs.sexp(), rhs.sexp())
            }
            HExpr::Un { op, expr, .. } => format!("({} {})", op.text(), expr.sexp()),
            HExpr::Call {
                callee,
                args,
                named,
                ..
            } => {
                let mut s = format!("(call {}", callee.sexp());
                for a in args {
                    s.push(' ');
                    s.push_str(&a.sexp());
                }
                for (n, v) in named {
                    s.push_str(&format!(" --{n}: {}", v.sexp()));
                }
                s.push(')');
                s
            }
            HExpr::Field {
                base,
                name,
                optional,
                ..
            } => {
                let tag = if *optional { "?field" } else { "field" };
                format!("({tag} {} {name})", base.sexp())
            }
            HExpr::Index { base, index, .. } => {
                format!("(index {} {})", base.sexp(), index.sexp())
            }
            HExpr::List { items, .. } => tagged_items("list", items),
            HExpr::Set { items, .. } => tagged_items("set", items),
            HExpr::Map { entries, .. } => {
                let mut s = String::from("(map");
                for (k, v) in entries {
                    s.push(' ');
                    s.push_str(&k.sexp());
                    s.push(' ');
                    s.push_str(&v.sexp());
                }
                s.push(')');
                s
            }
            HExpr::Record { fields, .. } => {
                let mut s = String::from("(record");
                for (n, v) in fields {
                    s.push_str(&format!(" {n}: {}", v.sexp()));
                }
                s.push(')');
                s
            }
            HExpr::EnumLit {
                path,
                variant,
                args,
                ..
            } => {
                let mut name = path.clone();
                if name.last().map(String::as_str) != Some(variant.as_str()) {
                    name.push(variant.clone());
                }
                let mut s = format!("(enumlit {}", name.join("."));
                for a in args {
                    s.push(' ');
                    s.push_str(&a.sexp());
                }
                s.push(')');
                s
            }
            HExpr::Opt { value, .. } => match value {
                None => "none".into(),
                Some(v) => format!("(opt {})", v.sexp()),
            },
            HExpr::Try { expr, .. } => format!("(try {})", expr.sexp()),
            HExpr::With { base, fields, .. } => {
                let mut s = format!("(with {}", base.sexp());
                for (n, v) in fields {
                    s.push_str(&format!(" {n}: {}", v.sexp()));
                }
                s.push(')');
                s
            }
            HExpr::If { cond, .. } => format!("(if-expr {})", cond.sexp()),
            HExpr::Match { subject, .. } => format!("(match-expr {})", subject.sexp()),
            HExpr::Lambda { params, body, .. } => {
                let p: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
                format!("(lambda ({}) {})", p.join(" "), body.sexp())
            }
            HExpr::Tuple { items, .. } => tagged_items("tuple", items),
            HExpr::Ref { expr, .. } => format!("(ref {})", expr.sexp()),
            HExpr::Await { expr, .. } => format!("(await {})", expr.sexp()),
            HExpr::CmdBlock { raw, .. } => format!("(cmd-block {})", raw.trim()),
            HExpr::Error { .. } => "{error}".into(),
        }
    }
}

fn tagged_items(tag: &str, items: &[HExpr]) -> String {
    let mut s = format!("({tag}");
    for i in items {
        s.push(' ');
        s.push_str(&i.sexp());
    }
    s.push(')');
    s
}

/// Thân lambda: biểu thức (`x => x * 2`) hoặc khối (`fn(x: int) -> int: …`).
#[derive(Debug, Clone, PartialEq)]
pub enum HBody {
    Expr(Box<HExpr>),
    Stmts(Vec<HStmt>),
}

impl HBody {
    pub fn sexp(&self) -> String {
        match self {
            HBody::Expr(e) => e.sexp(),
            HBody::Stmts(_) => "(stmts)".into(),
        }
    }
}

/// Toán tử nhị phân (đúng 12 mức ưu tiên A.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Pow,
    Range,
    RangeIncl,
    Coalesce,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Pipe,
}

impl BinOp {
    pub const fn text(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Rem => "%",
            BinOp::Pow => "**",
            BinOp::Range => "..",
            BinOp::RangeIncl => "..=",
            BinOp::Coalesce => "??",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
            BinOp::And => "and",
            BinOp::Or => "or",
            BinOp::Pipe => "|>",
        }
    }

    /// Từ chữ viết nguồn (§A.5).
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "+" => BinOp::Add,
            "-" => BinOp::Sub,
            "*" => BinOp::Mul,
            "/" => BinOp::Div,
            "%" => BinOp::Rem,
            "**" => BinOp::Pow,
            ".." => BinOp::Range,
            "..=" => BinOp::RangeIncl,
            "??" => BinOp::Coalesce,
            "==" => BinOp::Eq,
            "!=" => BinOp::Ne,
            "<" => BinOp::Lt,
            "<=" => BinOp::Le,
            ">" => BinOp::Gt,
            ">=" => BinOp::Ge,
            "and" => BinOp::And,
            "or" => BinOp::Or,
            "|>" => BinOp::Pipe,
            _ => return None,
        })
    }
}

/// Toán tử một ngôi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
}

impl UnOp {
    pub const fn text(self) -> &'static str {
        match self {
            UnOp::Neg => "-",
            UnOp::Not => "not",
        }
    }
}

/// Literal: **nguyên văn nguồn** + loại (để `Display`/chẩn đoán in lại đúng như người viết).
#[derive(Debug, Clone, PartialEq)]
pub struct Lit {
    pub kind: LitKind,
    /// Nguyên văn token (đã NFC) — `1`, `"a"`, `50pct`, `p"./x"`, `true`.
    pub text: String,
}

impl Lit {
    /// Kiểu của literal (VD-43).
    pub fn ty(&self) -> Type {
        match self.kind {
            LitKind::Int => Type::Int,
            LitKind::Float => Type::Float,
            LitKind::Size => Type::Size,
            LitKind::Duration => Type::Duration,
            LitKind::Percent => Type::Percent,
            LitKind::Str => Type::Str,
            LitKind::Bytes => Type::Bytes,
            LitKind::Path => Type::Path,
            LitKind::Char => Type::Char,
            LitKind::Bool => Type::Bool,
        }
    }
}

impl fmt::Display for Lit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

/// Loại literal mà HIR phân biệt (mỗi loại ⇒ đúng một [`Type`] ở [`Lit::ty`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LitKind {
    Int,
    Float,
    Size,
    Duration,
    Percent,
    Str,
    Bytes,
    Path,
    Char,
    Bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn var(n: &str) -> HExpr {
        HExpr::Var {
            name: n.into(),
            sym: None,
            span: Span::new(0, 0),
        }
    }

    fn lit(k: LitKind, t: &str) -> HExpr {
        let l = Lit {
            kind: k,
            text: t.into(),
        };
        HExpr::Lit {
            ty: l.ty(),
            lit: l,
            span: Span::new(0, 0),
        }
    }

    #[test]
    fn sexp_theo_quy_uoc_vd34_khong_paren() {
        let e = HExpr::Bin {
            op: BinOp::Mul,
            lhs: Box::new(HExpr::Bin {
                op: BinOp::Add,
                lhs: Box::new(lit(LitKind::Int, "1")),
                rhs: Box::new(lit(LitKind::Int, "2")),
                span: Span::new(0, 0),
            }),
            rhs: Box::new(lit(LitKind::Int, "3")),
            span: Span::new(0, 0),
        };
        assert_eq!(e.sexp(), "(* (+ 1 2) 3)");
        assert_eq!(var("a").sexp(), "a");
        let call = HExpr::Call {
            callee: Box::new(var("f")),
            args: vec![lit(LitKind::Str, "\"a\"")],
            named: vec![],
            span: Span::new(0, 0),
        };
        assert_eq!(call.sexp(), "(call f \"a\")");
        let fld = HExpr::Field {
            base: Box::new(var("a")),
            name: "b".into(),
            optional: true,
            span: Span::new(0, 0),
        };
        assert_eq!(fld.sexp(), "(?field a b)");
        let rec = HExpr::Record {
            fields: vec![("age".into(), lit(LitKind::Int, "20"))],
            ty: Type::Error,
            span: Span::new(0, 0),
        };
        assert_eq!(rec.sexp(), "(record age: 20)");
        let m = HExpr::Map {
            entries: vec![(lit(LitKind::Str, "\"a\""), lit(LitKind::Int, "1"))],
            key: Type::Str,
            value: Type::Int,
            span: Span::new(0, 0),
        };
        assert_eq!(m.sexp(), "(map \"a\" 1)");
        assert_eq!(
            HExpr::Opt {
                value: None,
                ty: Type::Error,
                span: Span::new(0, 0)
            }
            .sexp(),
            "none"
        );
    }

    #[test]
    fn toan_tu_va_pattern_co_chu_viet() {
        for op in [
            BinOp::Add,
            BinOp::Range,
            BinOp::Coalesce,
            BinOp::And,
            BinOp::Pipe,
        ] {
            assert_eq!(BinOp::parse(op.text()), Some(op));
        }
        assert_eq!(BinOp::parse("!"), None);
        assert_eq!(AssignOp::Add.text(), "+=");
        assert_eq!(UnOp::Neg.text(), "-");
        assert_eq!(
            HPat::Or {
                alts: vec![
                    HPat::Ctor {
                        path: vec!["Circle".into()],
                        args: vec![HPat::Bind {
                            name: "r".into(),
                            span: Span::new(0, 0),
                            sym: 0
                        }],
                        span: Span::new(0, 0)
                    },
                    HPat::Wild {
                        span: Span::new(0, 0)
                    }
                ],
                span: Span::new(0, 0)
            }
            .sexp(),
            "(or (Circle r) _)"
        );
    }

    #[test]
    fn literal_dien_kieu() {
        for (k, t) in [
            (LitKind::Int, Type::Int),
            (LitKind::Float, Type::Float),
            (LitKind::Size, Type::Size),
            (LitKind::Duration, Type::Duration),
            (LitKind::Percent, Type::Percent),
            (LitKind::Str, Type::Str),
            (LitKind::Bool, Type::Bool),
            (LitKind::Bytes, Type::Bytes),
            (LitKind::Path, Type::Path),
            (LitKind::Char, Type::Char),
        ] {
            assert_eq!(lit(k, "x").sexp(), "x");
            let l = Lit {
                kind: k,
                text: "x".into(),
            };
            assert_eq!(l.ty(), t);
        }
    }
}
