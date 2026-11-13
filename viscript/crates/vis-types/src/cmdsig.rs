// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Chữ ký lệnh chế độ shell** (ST-37, VD-69) — **một** nguồn cho cả `tyck`-khi-gọi và **khe ống**.
//!
//! `CmdSig` có thể **generic** (`take[T](xs: list[T], n: int) -> list[T]`); khe ống chọn theo
//! **khả năng nhận** (hợp nhất một chiều, §9.5) chứ không so bằng kiểu. `SigSource` là **seam** duy
//! nhất cấp chữ ký — `vis-std`/prelude (ST-40/41) cắm vào mà **không** sửa `lower`/`tyck`.

use crate::hir::HParam;
use crate::ty::Type;

/// Chữ ký một lệnh chế độ shell.
#[derive(Debug, Clone, PartialEq)]
pub struct CmdSig {
    pub name: String,
    /// `Some` nếu là hàm `.vis`/`impl` đã hạ; `None` nếu là **builtin** của chế độ lệnh.
    pub sym: Option<u32>,
    /// Tham số kiểu của chữ ký (`take[T]`) — rỗng nếu không generic.
    pub generics: Vec<String>,
    pub params: Vec<HParam>,
    pub ret: Type,
}

impl CmdSig {
    /// Chữ ký **không** generic.
    pub fn thuong(name: &str, params: Vec<HParam>, ret: Type) -> Self {
        Self {
            name: name.to_string(),
            sym: None,
            generics: Vec::new(),
            params,
            ret,
        }
    }
}

/// **Seam duy nhất** cấp chữ ký lệnh cho bộ hạ `.vish` (ST-37).
pub trait SigSource {
    fn sigs(&self) -> &[CmdSig];
}

/// Nguồn mặc định: **3 builtin** của chế độ lệnh (`echo`, `lines`, `take`).
pub struct BuiltinSigs {
    sigs: Vec<CmdSig>,
}

impl Default for BuiltinSigs {
    fn default() -> Self {
        Self::mac_dinh()
    }
}

impl BuiltinSigs {
    /// Bảng builtin hiện hành: `echo(a, b="", c="", --upper:bool=false)`, `lines(s)`,
    /// `take[T](xs: list[T], n: int) -> list[T]`.
    pub fn mac_dinh() -> Self {
        let tham = |ten: &str, ty: Type, default: Option<crate::hir::HExpr>| HParam {
            name: ten.to_string(),
            span: vis_syntax::Span::new(0, 0),
            sym: 0,
            ty,
            named: false,
            is_self: false,
            is_var_self: false,
            variadic: false,
            default,
        };
        let lit_s = |s: &str| {
            Some(crate::hir::HExpr::Lit {
                ty: Type::Str,
                lit: crate::hir::Lit {
                    kind: crate::hir::LitKind::Str,
                    text: s.to_string(),
                },
                span: vis_syntax::Span::new(0, 0),
            })
        };
        let lit_b = |b: bool| {
            Some(crate::hir::HExpr::Lit {
                ty: Type::Bool,
                lit: crate::hir::Lit {
                    kind: crate::hir::LitKind::Bool,
                    text: b.to_string(),
                },
                span: vis_syntax::Span::new(0, 0),
            })
        };

        let mut co = tham("upper", Type::Bool, lit_b(false));
        co.named = true;

        let t = Type::TypeRef {
            name: "T".into(),
            args: Vec::new(),
        };
        let take = CmdSig {
            name: "take".into(),
            sym: None,
            generics: vec!["T".into()],
            params: vec![
                tham("xs", Type::List(Box::new(t.clone())), None),
                tham("n", Type::Int, None),
            ],
            ret: Type::List(Box::new(t.clone())),
        };

        // ST-38: `where`/`each`/`first`/`count` — closure-arg; `each` **chỉ** tác dụng phụ (trả `()`).
        let ds = |t: &Type| Type::List(Box::new(t.clone()));
        let fn_bool = Type::Fn(vec![t.clone()].into(), Box::new(Type::Bool));
        let fn_unit = Type::Fn(vec![t.clone()].into(), Box::new(Type::Unit));
        // ST-39: `sort_by(xs, f, --desc)` — lệnh **shell** (khác `std.list.sort_by` khoá `fn(int)->int`).
        let k_ty = Type::TypeRef {
            name: "K".into(),
            args: Vec::new(),
        };
        let mut desc = tham("desc", Type::Bool, None);
        desc.named = true;
        let sort_by = CmdSig {
            name: "sort_by".into(),
            sym: None,
            generics: vec!["T".into(), "K".into()],
            params: vec![
                tham("xs", ds(&t), None),
                tham("f", Type::Fn(vec![t.clone()].into(), Box::new(k_ty)), None),
                desc,
            ],
            ret: ds(&t),
        };
        let where_ = CmdSig {
            name: "where".into(),
            sym: None,
            generics: vec!["T".into()],
            params: vec![tham("xs", ds(&t), None), tham("f", fn_bool, None)],
            ret: ds(&t),
        };
        let each = CmdSig {
            name: "each".into(),
            sym: None,
            generics: vec!["T".into()],
            params: vec![tham("xs", ds(&t), None), tham("f", fn_unit, None)],
            ret: Type::Unit,
        };
        let first = CmdSig {
            name: "first".into(),
            sym: None,
            generics: vec!["T".into()],
            params: vec![tham("xs", ds(&t), None)],
            ret: Type::Opt(Box::new(t.clone())),
        };
        let count = CmdSig {
            name: "count".into(),
            sym: None,
            generics: vec!["T".into()],
            params: vec![tham("xs", ds(&t), None)],
            ret: Type::Int,
        };
        Self {
            sigs: vec![
                CmdSig::thuong(
                    "echo",
                    vec![
                        tham("a", Type::Str, None),
                        tham("b", Type::Str, lit_s("\"\"")),
                        tham("c", Type::Str, lit_s("\"\"")),
                        co,
                    ],
                    Type::Str,
                ),
                CmdSig::thuong(
                    "lines",
                    vec![tham("s", Type::Str, None)],
                    Type::List(Box::new(Type::Str)),
                ),
                take,
                sort_by,
                where_,
                each,
                first,
                count,
            ],
        }
    }
}

impl SigSource for BuiltinSigs {
    fn sigs(&self) -> &[CmdSig] {
        &self.sigs
    }
}

/// Hợp nhất **một chiều** `mau` với `that` (ST-37): buộc được tham số kiểu thì `true`.
/// `{error}` ở `that` **nhận mọi** kiểu (như tyck `hop_nhat`).
pub fn hop_nhat_cmd(
    mau: &Type,
    that: &Type,
    gens: &[String],
    bang: &mut Vec<(String, Type)>,
) -> bool {
    if matches!(that, Type::Error) {
        return true;
    }
    match mau {
        Type::TypeRef { name, args } if args.is_empty() && gens.iter().any(|g| g == name) => {
            match bang.iter().find(|(n, _)| n == name) {
                Some((_, da)) => same_cmd(da, that) || matches!(that, Type::Error),
                None => {
                    bang.push((name.clone(), that.clone()));
                    true
                }
            }
        }
        Type::List(a) => match that {
            Type::List(b) => hop_nhat_cmd(a, b, gens, bang),
            _ => false,
        },
        Type::Map(k1, v1) => match that {
            Type::Map(k2, v2) => {
                hop_nhat_cmd(k1, k2, gens, bang) && hop_nhat_cmd(v1, v2, gens, bang)
            }
            _ => false,
        },
        Type::Opt(a) => match that {
            Type::Opt(b) => hop_nhat_cmd(a, b, gens, bang),
            _ => true, // nâng `T → T?`
        },
        Type::Result(a1, b1) => match that {
            Type::Result(a2, b2) => {
                hop_nhat_cmd(a1, a2, gens, bang) && hop_nhat_cmd(b1, b2, gens, bang)
            }
            _ => false,
        },
        _ => same_cmd(mau, that),
    }
}

/// So bằng **cấu trúc** (đã thay hết tham số kiểu), `{error}` nhận mọi kiểu.
pub fn same_cmd(a: &Type, b: &Type) -> bool {
    match (a, b) {
        (Type::Error, _) | (_, Type::Error) => true,
        (Type::Int, Type::Int)
        | (Type::Float, Type::Float)
        | (Type::Str, Type::Str)
        | (Type::Bool, Type::Bool)
        | (Type::Unit, Type::Unit)
        | (Type::Bytes, Type::Bytes)
        | (Type::Path, Type::Path)
        | (Type::Char, Type::Char)
        | (Type::Never, Type::Never) => true,
        (Type::List(x), Type::List(y)) | (Type::Opt(x), Type::Opt(y)) => same_cmd(x, y),
        (Type::Map(k1, v1), Type::Map(k2, v2)) => same_cmd(k1, k2) && same_cmd(v1, v2),
        (Type::Result(a1, b1), Type::Result(a2, b2)) => same_cmd(a1, a2) && same_cmd(b1, b2),
        (Type::Record(x), Type::Record(y)) => x == y,
        (Type::Enum(x), Type::Enum(y)) => x == y,
        (Type::TypeRef { name: n1, args: a1 }, Type::TypeRef { name: n2, args: a2 }) => {
            n1 == n2 && a1.len() == a2.len() && a1.iter().zip(a2).all(|(x, y)| same_cmd(x, y))
        }
        _ => false,
    }
}
