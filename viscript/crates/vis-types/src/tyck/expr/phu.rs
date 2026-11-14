// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-61**: phụ trợ nhỏ của `tyck/expr.rs` — tách ra để `expr.rs` giữ **R1** ≤ 1.500 dòng.
//! Đây là các hàm **thuần** (không trạng thái), chuyển nguyên văn từ `expr.rs`.

use crate::ty::Type;

/// `percent` và `float` so sánh được **theo tỉ lệ** (§3.1/§3.6) — đặc cách duy nhất.
pub(super) fn is_percent_float(a: &Type, b: &Type) -> bool {
    matches!(
        (a, b),
        (Type::Percent, Type::Float) | (Type::Float, Type::Percent)
    )
}

/// 8 tên dựng sẵn (VD-44 #3) — dùng khi `sym = None`.
pub(super) fn builtin_is_value(name: &str) -> bool {
    matches!(
        name,
        "print"
            | "len"
            | "str"
            | "Ok"
            | "Err"
            | "Some"
            | "None"
            | "ref"
            | "sort_by"
            | "get"
            | "select"
    )
}

/// Kiểu của 8 tên giá trị dựng sẵn (không phải hàm) — `None`/`Some`… không có kiểu riêng ở đây.
pub(super) fn builtin_value_ty(name: &str) -> Type {
    match name {
        "None" => Type::Opt(Box::new(Type::Error)),
        _ => Type::Error,
    }
}
