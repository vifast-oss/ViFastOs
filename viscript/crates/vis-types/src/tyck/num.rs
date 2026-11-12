// SPDX-License-Identifier: MIT OR Apache-2.0
//! Bảng **số học** VS0 (VD-43 #2) — dùng cho `+ - * / % **` và khoảng `.. ../..=`.
//!
//! Nguyên tắc: **cùng đơn vị** mới `+ -`; nhân/chia với `int`/`percent` giữ đơn vị; `size / size`
//! và `duration / duration` cho `float`; trộn đơn vị ⇒ `None` (caller báo `VS0412`).
//! Chia/mũ cho 0 hay tràn là **runtime** (ST-13) — ở đây **không** xét.

use crate::hir::BinOp;
use crate::ty::Type;

/// Nâng `int → float` **chỉ khi** phía `int` là **literal số nguyên** (thiết kế §3.5 — ngoại lệ
/// duy nhất; ST-12b BLOCKER #2: biến `int` **không** được nâng).
pub(super) fn promote(l: &Type, r: &Type, l_lit: bool, r_lit: bool) -> (Type, Type) {
    match (l, r) {
        (Type::Int, Type::Float) if l_lit => (Type::Float, Type::Float),
        (Type::Float, Type::Int) if r_lit => (Type::Float, Type::Float),
        _ => (l.clone(), r.clone()),
    }
}

/// Kiểu kết quả của toán tử số học; `None` = không hợp lệ (⇒ `VS0412`).
pub(super) fn so_hoc(op: BinOp, l: &Type, r: &Type, l_lit: bool, r_lit: bool) -> Option<Type> {
    use BinOp::{Add, Div, Mul, Pow, Range, RangeIncl, Rem, Sub};
    use Type::{Char, Duration, Float, Int, Percent, Size};

    if l.is_error() || r.is_error() {
        return Some(Type::Error);
    }
    let (l, r) = promote(l, r, l_lit, r_lit);
    // Khoảng `a..b`: hai đầu cùng kiểu số; kết quả là `range[T]` (mã hoá nội bộ của ST-11/12).
    if matches!(op, Range | RangeIncl) {
        let ok = matches!(
            (&l, &r),
            (Int, Int) | (Size, Size) | (Duration, Duration) | (Char, Char)
        );
        if !ok {
            return None;
        }
        return Some(Type::TypeRef {
            name: "range".into(),
            args: vec![l],
        });
    }
    Some(match (op, &l, &r) {
        (Add | Sub, Int, Int) => Type::Int,
        (Add | Sub, Float, Float) => Type::Float,
        (Add | Sub, Size, Size) => Type::Size,
        (Add | Sub, Duration, Duration) => Type::Duration,
        (Add | Sub, Percent, Percent) => Type::Percent,
        (Mul, Int, Int) => Type::Int,
        (Mul, Float, Float) => Type::Float,
        (Mul, Size, Int) | (Mul, Int, Size) => Type::Size,
        (Mul, Size, Percent) | (Mul, Percent, Size) => Type::Size,
        (Mul, Duration, Int) | (Mul, Int, Duration) => Type::Duration,
        (Mul, Duration, Percent) | (Mul, Percent, Duration) => Type::Duration,
        (Mul, Percent, Int) | (Mul, Int, Percent) => Type::Int,
        (Div, Int, Int) => Type::Int,
        (Div, Float, Float) => Type::Float,
        (Div, Size, Int) => Type::Size,
        (Div, Duration, Int) => Type::Duration,
        (Div, Size, Size) => Type::Float,
        (Div, Duration, Duration) => Type::Float,
        (Div, Percent, Int) => Type::Percent,
        // VD-48(b): `percent / percent` ⇒ **float** (tỉ lệ của hai tỉ lệ); giữ `percent / int ⇒ percent`.
        (Div, Percent, Percent) => Type::Float,
        (Rem, Int, Int) => Type::Int,
        (Rem, Size, Int) => Type::Size,
        (Rem, Duration, Int) => Type::Duration,
        // ST-13b (mục 5, VD-48): `size % size ⇒ size`, `duration % duration ⇒ duration`,
        // `percent % percent ⇒ percent` — đối xứng với runtime.
        (Rem, Size, Size) => Type::Size,
        (Rem, Duration, Duration) => Type::Duration,
        (Rem, Percent, Percent) => Type::Percent,
        (Pow, Int, Int) => Type::Int,
        (Pow, Float, Float) => Type::Float,
        _ => return None,
    })
}

/// Kiểu **sắp thứ tự được** (`< <= > >=`): số, `str`, `bytes`, `path`, `char` (VD-43 #4).
pub(super) fn ordered(t: &Type) -> bool {
    matches!(
        t,
        Type::Int
            | Type::Float
            | Type::Size
            | Type::Duration
            | Type::Percent
            | Type::Str
            | Type::Bytes
            | Type::Path
            | Type::Char
    )
}
