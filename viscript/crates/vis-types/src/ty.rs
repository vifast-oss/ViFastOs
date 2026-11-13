// SPDX-License-Identifier: MIT OR Apache-2.0
//! Hệ kiểu của VS0 — [`Type`] (VD-43) + bảng kiểu người dùng ([`TypeTable`]).
//!
//! # Phạm vi ST-11
//!
//! Hạ (lower) **không kiểm kiểu**: nó chỉ *điền* kiểu cho literal và dựng mặt nạ kiểu cho bản
//! ghi/enum. Tên kiểu **chưa khai trong tệp** giữ nguyên dạng [`Type::TypeRef`] —
//! **không** báo `VS0301` cho kiểu ở ST-11 (VD-44 #2); ST-12 phân giải kiểu rồi mới báo.
//!
//! Kiểu dựng sẵn theo VD-43: `int`, `size`, `duration`, `percent`, `str`, `bool`, `bytes`,
//! `path`, `char`, cùng `float` (thiết kế §3.1, §3.6; VD-44 #1 yêu cầu hạ được `float`),
//! `list[T]`, `map[K, V]`, `Result[T, E]`, `fn(A, B) -> C`, `T?`, `()` và `{error}`.

use std::fmt;

use vis_syntax::Span;

/// Chỉ số bản ghi trong [`TypeTable`] (`type …:` khai báo hoặc bản ghi ẩn danh).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RecId(pub u32);

/// Chỉ số enum trong [`TypeTable`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EnumId(pub u32);

/// Kiểu của VS0.
///
/// `TypeRef` là **tên kiểu chưa phân giải** (VD-44 #2): `Config`, `AnyError`, `Ctx`, `cap[Dir]`,
/// `ref[T]`… đều đi qua đây ở ST-11 mà **không** sinh chẩn đoán. ST-12 thay bằng kiểu thật.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Type {
    Int,
    Float,
    Size,
    Duration,
    Percent,
    Str,
    Bool,
    Bytes,
    Path,
    Char,
    List(Box<Type>),
    Map(Box<Type>, Box<Type>),
    Record(RecId),
    Enum(EnumId),
    Opt(Box<Type>),
    Result(Box<Type>, Box<Type>),
    Fn(Box<[Type]>, Box<Type>),
    /// Tên kiểu chưa phân giải (kèm đối số generic nếu có): `Config`, `cap[Dir]`, `Json`.
    TypeRef {
        name: String,
        args: Vec<Type>,
    },
    Unit,
    /// **Phân kỳ** (`return`, khối toàn `return`, `match` mọi nhánh `return`) — ST-13b mục 1:
    /// tương thích với **mọi** kiểu (không bao giờ cho giá trị).
    Never,
    /// `{error}` — kiểu "không biết/không hợp lệ", lan truyền im lặng (thiết kế §14.3).
    Error,
}

impl Type {
    /// `true` nếu là `{error}`.
    pub const fn is_error(&self) -> bool {
        matches!(self, Type::Error)
    }

    /// `true` nếu là `T?`.
    pub const fn is_opt(&self) -> bool {
        matches!(self, Type::Opt(_))
    }

    /// Bỏ **một** lớp `Option`; kiểu khác trả nguyên nó.
    pub fn skip_opt(&self) -> &Type {
        match self {
            Type::Opt(t) => t,
            t => t,
        }
    }

    /// Kiểu dựng sẵn theo tên (VD-43 + `float`). `None` nếu không phải kiểu dựng sẵn.
    pub fn builtin(name: &str) -> Option<Type> {
        Some(match name {
            "int" => Type::Int,
            "float" => Type::Float,
            "size" => Type::Size,
            "duration" => Type::Duration,
            "percent" => Type::Percent,
            "str" => Type::Str,
            "bool" => Type::Bool,
            "bytes" => Type::Bytes,
            "path" => Type::Path,
            "char" => Type::Char,
            "unit" => Type::Unit,
            _ => return None,
        })
    }

    /// Tên hiển thị **ổn định** (chỉ dùng mã kiểu, không cần bảng): `list[int]`, `int?`,
    /// `fn(int) -> bool`, `record#0`, `{error}`.
    ///
    /// Muốn tên **có thật** của bản ghi/enum thì dùng [`TypeTable::show`].
    pub fn name(&self) -> String {
        match self {
            Type::Int => "int".into(),
            Type::Float => "float".into(),
            Type::Size => "size".into(),
            Type::Duration => "duration".into(),
            Type::Percent => "percent".into(),
            Type::Str => "str".into(),
            Type::Bool => "bool".into(),
            Type::Bytes => "bytes".into(),
            Type::Path => "path".into(),
            Type::Char => "char".into(),
            Type::List(t) => format!("list[{}]", t.name()),
            Type::Map(k, v) => format!("map[{}, {}]", k.name(), v.name()),
            Type::Record(id) => format!("record#{}", id.0),
            Type::Enum(id) => format!("enum#{}", id.0),
            Type::Opt(t) => format!("{}?", t.name()),
            Type::Result(a, b) => format!("Result[{}, {}]", a.name(), b.name()),
            Type::Fn(args, ret) => {
                let mut s = String::from("fn(");
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        s.push_str(", ");
                    }
                    s.push_str(&a.name());
                }
                s.push_str(") -> ");
                s.push_str(&ret.name());
                s
            }
            Type::TypeRef { name, args } => {
                if args.is_empty() {
                    name.clone()
                } else {
                    let inner: Vec<String> = args.iter().map(Type::name).collect();
                    format!("{name}[{}]", inner.join(", "))
                }
            }
            Type::Unit => "()".into(),
            Type::Never => "never".into(),
            Type::Error => "{error}".into(),
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name())
    }
}

/// Một trường của bản ghi / biến thể enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldDef {
    pub name: String,
    pub ty: Type,
    pub span: Span,
    /// Có giá trị mặc định trong khai báo (`email: str? = none`).
    pub has_default: bool,
}

/// Khai báo bản ghi (`type User: …`) hoặc bản ghi **ẩn danh** (literal `{name: …}`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecDef {
    /// `None` = bản ghi ẩn danh dựng từ literal.
    pub name: Option<String>,
    pub fields: Vec<FieldDef>,
    pub span: Span,
    /// `true` nếu là bản ghi dựng tại chỗ cho một literal (ST-12 có thể bỏ qua khi so khớp).
    pub anonymous: bool,
}

/// Một biến thể enum (`Circle(r: float)`, `Empty`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariantDef {
    pub name: String,
    pub fields: Vec<FieldDef>,
    pub span: Span,
}

/// Khai báo enum (`enum Shape: …`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumDef {
    pub name: String,
    pub variants: Vec<VariantDef>,
    pub span: Span,
}

/// Bảng kiểu người dùng của **một** lượt hạ: bản ghi + enum, đánh số ổn định theo thứ tự khai báo.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypeTable {
    recs: Vec<RecDef>,
    enums: Vec<EnumDef>,
    /// ST-35: **instantiation** — với mỗi `RecId`/`EnumId` sinh từ `Hop[int]`, ghi `(tên gốc, đối số kiểu)`.
    /// `None` = kiểu khai báo gốc. Nhờ vậy tyck hợp nhất được `Hop[T]` ↔ `Hop[int]` **và** giữ
    /// `Hop[int] ≠ Hop[str]` (hai id khác nhau).
    rec_inst: Vec<Option<(String, Vec<Type>)>>,
    enum_inst: Vec<Option<(String, Vec<Type>)>>,
}

impl TypeTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn recs(&self) -> &[RecDef] {
        &self.recs
    }

    pub fn enums(&self) -> &[EnumDef] {
        &self.enums
    }

    /// Ghi một **instantiation** của bản ghi (`Hop[int]`) — id mới, cùng tên gốc.
    pub fn push_rec_inst(&mut self, base: String, args: Vec<Type>, mut def: RecDef) -> RecId {
        let id = RecId(self.recs.len() as u32);
        def.anonymous = false;
        self.recs.push(def);
        self.rec_inst.push(Some((base, args)));
        id
    }

    /// Ghi một **instantiation** của enum (`Or[int, str]`).
    pub fn push_enum_inst(&mut self, base: String, args: Vec<Type>, def: EnumDef) -> EnumId {
        let id = EnumId(self.enums.len() as u32);
        self.enums.push(def);
        self.enum_inst.push(Some((base, args)));
        id
    }

    /// `(tên gốc, đối số)` nếu `id` là một instantiation (ST-35).
    pub fn rec_inst_of(&self, id: RecId) -> Option<&(String, Vec<Type>)> {
        self.rec_inst.get(id.0 as usize).and_then(|x| x.as_ref())
    }

    /// Như [`TypeTable::rec_inst_of`] cho enum.
    pub fn enum_inst_of(&self, id: EnumId) -> Option<&(String, Vec<Type>)> {
        self.enum_inst.get(id.0 as usize).and_then(|x| x.as_ref())
    }

    pub fn rec(&self, id: RecId) -> Option<&RecDef> {
        self.recs.get(id.0 as usize)
    }

    pub fn enum_def(&self, id: EnumId) -> Option<&EnumDef> {
        self.enums.get(id.0 as usize)
    }

    /// Thêm bản ghi, trả `RecId` vừa cấp.
    pub fn push_rec(&mut self, rec: RecDef) -> RecId {
        let id = RecId(self.recs.len() as u32);
        self.recs.push(rec);
        self.rec_inst.push(None);
        id
    }

    /// Thêm enum, trả `EnumId` vừa cấp.
    pub fn push_enum(&mut self, e: EnumDef) -> EnumId {
        let id = EnumId(self.enums.len() as u32);
        self.enums.push(e);
        self.enum_inst.push(None);
        id
    }

    /// Truy cập **ghi** bản ghi theo id (ST-25: hạ đa tệp điền trường sau khi predeclare).
    pub fn rec_mut(&mut self, id: RecId) -> Option<&mut RecDef> {
        self.recs.get_mut(id.0 as usize)
    }

    /// Truy cập **ghi** enum theo id (ST-25).
    pub fn enum_mut(&mut self, id: EnumId) -> Option<&mut EnumDef> {
        self.enums.get_mut(id.0 as usize)
    }

    /// Tên **có thật** của kiểu: bản ghi/enum đã khai in tên, chưa có tên in `record#N`/`enum#N`.
    pub fn show(&self, ty: &Type) -> String {
        match ty {
            Type::Record(id) => match self.rec(*id).and_then(|r| r.name.as_deref()) {
                Some(n) => n.to_string(),
                None => ty.name(),
            },
            Type::Enum(id) => match self.enum_def(*id) {
                Some(e) => e.name.clone(),
                None => ty.name(),
            },
            Type::List(t) => format!("list[{}]", self.show(t)),
            Type::Map(k, v) => format!("map[{}, {}]", self.show(k), self.show(v)),
            Type::Opt(t) => format!("{}?", self.show(t)),
            Type::Result(a, b) => format!("Result[{}, {}]", self.show(a), self.show(b)),
            Type::Fn(args, ret) => {
                let inner: Vec<String> = args.iter().map(|a| self.show(a)).collect();
                format!("fn({}) -> {}", inner.join(", "), self.show(ret))
            }
            Type::TypeRef { name, args } => {
                if args.is_empty() {
                    name.clone()
                } else {
                    let inner: Vec<String> = args.iter().map(|a| self.show(a)).collect();
                    format!("{name}[{}]", inner.join(", "))
                }
            }
            other => other.name(),
        }
    }

    /// Trường của bản ghi theo tên (ST-12 dùng).
    pub fn field(&self, id: RecId, name: &str) -> Option<&FieldDef> {
        self.rec(id)?.fields.iter().find(|f| f.name == name)
    }

    /// Biến thể của enum theo tên.
    pub fn variant(&self, id: EnumId, name: &str) -> Option<&VariantDef> {
        self.enum_def(id)?.variants.iter().find(|v| v.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ten_hien_thi_on_dinh() {
        assert_eq!(Type::Int.name(), "int");
        assert_eq!(Type::Float.name(), "float");
        assert_eq!(
            Type::List(Box::new(Type::Map(
                Box::new(Type::Str),
                Box::new(Type::Int)
            )))
            .name(),
            "list[map[str, int]]"
        );
        assert_eq!(Type::Opt(Box::new(Type::Int)).name(), "int?");
        assert_eq!(
            Type::Result(
                Box::new(Type::Unit),
                Box::new(Type::TypeRef {
                    name: "AnyError".into(),
                    args: vec![],
                })
            )
            .name(),
            "Result[(), AnyError]"
        );
        assert_eq!(
            Type::Fn(
                vec![Type::Int, Type::Str].into_boxed_slice(),
                Box::new(Type::Bool)
            )
            .name(),
            "fn(int, str) -> bool"
        );
        assert_eq!(
            Type::TypeRef {
                name: "cap".into(),
                args: vec![Type::TypeRef {
                    name: "Dir".into(),
                    args: vec![]
                }]
            }
            .name(),
            "cap[Dir]"
        );
        assert_eq!(Type::Error.name(), "{error}");
        assert_eq!(format!("{}", Type::Unit), "()");
    }

    #[test]
    fn kieu_dung_san_va_bang_kieu() {
        for n in [
            "int", "float", "size", "duration", "percent", "str", "bool", "bytes", "path", "char",
        ] {
            assert!(Type::builtin(n).is_some(), "thiếu kiểu dựng sẵn {n}");
        }
        assert!(
            Type::builtin("Json").is_none(),
            "Json là TypeRef, không dựng sẵn"
        );

        let mut t = TypeTable::new();
        let id = t.push_rec(RecDef {
            name: Some("User".into()),
            fields: vec![FieldDef {
                name: "name".into(),
                ty: Type::Str,
                span: Span::new(0, 1),
                has_default: false,
            }],
            span: Span::new(0, 1),
            anonymous: false,
        });
        assert_eq!(t.show(&Type::Record(id)), "User");
        assert_eq!(t.show(&Type::Opt(Box::new(Type::Record(id)))), "User?");
        assert_eq!(t.field(id, "name").map(|f| f.ty.clone()), Some(Type::Str));
        assert!(t.field(id, "age").is_none());

        let anon = t.push_rec(RecDef {
            name: None,
            fields: vec![],
            span: Span::new(0, 1),
            anonymous: true,
        });
        assert_eq!(t.show(&Type::Record(anon)), format!("record#{}", anon.0));
        assert_eq!(t.rec(id).and_then(|r| r.name.as_deref()), Some("User"));
    }
}
