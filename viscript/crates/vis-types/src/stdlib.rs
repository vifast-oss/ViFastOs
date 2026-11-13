// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Bảng chữ ký `std.*`** (ST-26, VD-43): một nguồn sự thật cho `tyck` và (gián tiếp) `vis-interp`.
//!
//! Tên đầy đủ có tiền tố module (`str.split`, `list.sort`, `time.now_ms`) — nhờ vậy `std.str.join` và
//! `std.path.join` không đụng nhau, và không có tên trần nào bị nhập vào phạm vi người dùng.
//!
//! Kiểu tham số: `Ty::Any` = **không kiểm kiểu tĩnh** (chỉ kiểm **số đối số**), ngữ nghĩa kiểm lúc chạy
//! với `VS0501`. Đây là bắc cầu cho tới khi có generic (`list[T]`/`map[K,V]`) — nợ VS1c ghi JOURNAL.

use crate::ty::Type;

/// Một chỗ trong chữ ký: hoặc kiểu cụ thể, hoặc "nhận mọi kiểu".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ty {
    Int,
    Str,
    Bool,
    ListStr,
    /// Khoá của `list.sort_by`: `fn(T) -> int` **hoặc** `fn(T) -> str` (ST-56).
    FnKhoa,
    /// **Không** kiểm kiểu tĩnh (chỉ số đối số).
    Any,
    /// `Result[str, str]` — ST-33 (`std.fs.read`, `std.fs.list` lỗi trả `str`).
    ResultStrStr,
    /// `Result[(), str]` — ST-33 (`std.fs.write`).
    ResultUnitStr,
    /// `Result[list[str], str]` — ST-33 (`std.fs.list`).
    ResultListStrStr,
}

impl Ty {
    /// Kiểu tĩnh tương ứng; `None` = [`Ty::Any`] (bỏ kiểm kiểu).
    pub fn kieu(self) -> Option<Type> {
        match self {
            Ty::Int => Some(Type::Int),
            Ty::Str => Some(Type::Str),
            Ty::Bool => Some(Type::Bool),
            Ty::ListStr => Some(Type::List(Box::new(Type::Str))),
            Ty::ResultStrStr => Some(Type::Result(Box::new(Type::Str), Box::new(Type::Str))),
            Ty::ResultUnitStr => Some(Type::Result(Box::new(Type::Unit), Box::new(Type::Str))),
            Ty::ResultListStrStr => Some(Type::Result(
                Box::new(Type::List(Box::new(Type::Str))),
                Box::new(Type::Str),
            )),
            // Closure chưa suy kiểu hai chiều (VS0 nợ, xem JOURNAL ST-26) ⇒ **không** kiểm tĩnh;
            // tầng chạy kiểm "gọi được" và "khoá trả `int` **hoặc** `str`" (VS0501, ST-56).
            Ty::FnKhoa => None,
            Ty::Any => None,
        }
    }

    /// Tên hiển thị trong chẩn đoán.
    pub fn ten(self) -> &'static str {
        match self {
            Ty::Int => "int",
            Ty::Str => "str",
            Ty::Bool => "bool",
            Ty::ListStr => "list[str]",
            // ST-56: khoá nhận **cả** `str` ⇒ tên hiển thị nói đúng hợp đồng mới.
            Ty::FnKhoa => "fn(T) -> int | str",
            Ty::Any => "bất kỳ",
            Ty::ResultStrStr => "Result[str, str]",
            Ty::ResultUnitStr => "Result[(), str]",
            Ty::ResultListStrStr => "Result[list[str], str]",
        }
    }
}

/// Một hàm `std.*`: tên đầy đủ + tham số + kiểu trả.
#[derive(Debug, Clone, Copy)]
pub struct StdFn {
    /// Tên đầy đủ `"str.split"`.
    pub name: &'static str,
    pub params: &'static [Ty],
    pub ret: Ty,
}

impl StdFn {
    pub const fn so_doi(&self) -> usize {
        self.params.len()
    }
}

/// Module `std` được hỗ trợ ở ST-26 (mỗi phần tử là tên ngắn).
pub const MODULES: [&str; 7] = ["str", "list", "map", "path", "time", "json", "fs"];

/// `std.io` **chưa** làm — giữ tên để thông điệp rõ (ST-33: `std.fs` **đã có**).
pub const MODULES_HOAN: [&str; 1] = ["io"];

/// Toàn bộ hàm `std.*` của ST-26.
pub const FUNCS: [StdFn; 38] = [
    // ----- std.str -----
    StdFn {
        name: "str.split",
        params: &[Ty::Str, Ty::Str],
        ret: Ty::ListStr,
    },
    StdFn {
        name: "str.join",
        params: &[Ty::ListStr, Ty::Str],
        ret: Ty::Str,
    },
    StdFn {
        name: "str.trim",
        params: &[Ty::Str],
        ret: Ty::Str,
    },
    StdFn {
        name: "str.upper",
        params: &[Ty::Str],
        ret: Ty::Str,
    },
    StdFn {
        name: "str.lower",
        params: &[Ty::Str],
        ret: Ty::Str,
    },
    StdFn {
        name: "str.starts_with",
        params: &[Ty::Str, Ty::Str],
        ret: Ty::Bool,
    },
    StdFn {
        name: "str.ends_with",
        params: &[Ty::Str, Ty::Str],
        ret: Ty::Bool,
    },
    StdFn {
        name: "str.contains",
        params: &[Ty::Str, Ty::Str],
        ret: Ty::Bool,
    },
    StdFn {
        name: "str.repeat",
        params: &[Ty::Str, Ty::Int],
        ret: Ty::Str,
    },
    StdFn {
        name: "str.replace",
        params: &[Ty::Str, Ty::Str, Ty::Str],
        ret: Ty::Str,
    },
    // ----- std.list (phần tử là `Any` — nợ generic VS1c) -----
    StdFn {
        name: "list.sort",
        params: &[Ty::Any],
        ret: Ty::Any,
    },
    StdFn {
        name: "list.sort_by",
        params: &[Ty::Any, Ty::FnKhoa],
        ret: Ty::Any,
    },
    StdFn {
        name: "list.reverse",
        params: &[Ty::Any],
        ret: Ty::Any,
    },
    StdFn {
        name: "list.unique",
        params: &[Ty::Any],
        ret: Ty::Any,
    },
    StdFn {
        name: "list.concat",
        params: &[Ty::Any, Ty::Any],
        ret: Ty::Any,
    },
    StdFn {
        name: "list.chunk",
        params: &[Ty::Any, Ty::Int],
        ret: Ty::Any,
    },
    StdFn {
        name: "list.take",
        params: &[Ty::Any, Ty::Int],
        ret: Ty::Any,
    },
    StdFn {
        name: "list.drop",
        params: &[Ty::Any, Ty::Int],
        ret: Ty::Any,
    },
    // ----- std.map -----
    StdFn {
        name: "map.keys",
        params: &[Ty::Any],
        ret: Ty::Any,
    },
    StdFn {
        name: "map.values",
        params: &[Ty::Any],
        ret: Ty::Any,
    },
    StdFn {
        name: "map.merge",
        params: &[Ty::Any, Ty::Any],
        ret: Ty::Any,
    },
    StdFn {
        name: "map.get_or",
        params: &[Ty::Any, Ty::Str, Ty::Any],
        ret: Ty::Any,
    },
    StdFn {
        name: "map.remove",
        params: &[Ty::Any, Ty::Str],
        ret: Ty::Any,
    },
    // ----- std.path (thuần chuỗi) -----
    StdFn {
        name: "path.join",
        params: &[Ty::Str, Ty::Str],
        ret: Ty::Str,
    },
    StdFn {
        name: "path.basename",
        params: &[Ty::Str],
        ret: Ty::Str,
    },
    StdFn {
        name: "path.dirname",
        params: &[Ty::Str],
        ret: Ty::Str,
    },
    StdFn {
        name: "path.ext",
        params: &[Ty::Str],
        ret: Ty::Str,
    },
    StdFn {
        name: "path.stem",
        params: &[Ty::Str],
        ret: Ty::Str,
    },
    StdFn {
        name: "path.is_abs",
        params: &[Ty::Str],
        ret: Ty::Bool,
    },
    // ----- std.time (đọc đồng hồ trong Rust, không qua Host) -----
    StdFn {
        name: "time.now_ms",
        params: &[],
        ret: Ty::Int,
    },
    StdFn {
        name: "time.monotonic_ms",
        params: &[],
        ret: Ty::Int,
    },
    // ----- std.json (chỉ encode; decode hoãn — xem `HA_CHUA_HO_TRO`) -----
    StdFn {
        name: "json.encode",
        params: &[Ty::Any],
        ret: Ty::Str,
    },
    StdFn {
        name: "json.encode_pretty",
        params: &[Ty::Any],
        ret: Ty::Str,
    },
    // ST-33: `json.decode` — trả `Result[Any, str]` nhưng `Type` **không** có `Any` ⇒ `Ty::Any`
    // (bỏ kiểm tĩnh như `list.*`/`map.*`, VD-59 (3)); giá trị thật là `Result` lúc chạy.
    StdFn {
        name: "json.decode",
        params: &[Ty::Str],
        ret: Ty::Any,
    },
    // ----- std.fs (ST-33) — **đi qua `Host`**, không OS trong crate này -----
    StdFn {
        name: "fs.read",
        params: &[Ty::Str],
        ret: Ty::ResultStrStr,
    },
    StdFn {
        name: "fs.write",
        params: &[Ty::Str, Ty::Str],
        ret: Ty::ResultUnitStr,
    },
    StdFn {
        name: "fs.exists",
        params: &[Ty::Str],
        ret: Ty::Bool,
    },
    StdFn {
        name: "fs.list",
        params: &[Ty::Str],
        ret: Ty::ResultListStrStr,
    },
];

/// Tên **đã biết nhưng chưa hỗ trợ** ở VS1c — dùng để báo `VS0301` với thông điệp riêng.
/// (ST-33: `json.decode` **đã có** ⇒ bảng **rỗng**; giữ API cho các tên hoãn sau này.)
pub const HA_CHUA_HO_TRO: [&str; 0] = [];

/// Tra chữ ký theo tên đầy đủ.
pub fn sig_of(name: &str) -> Option<&'static StdFn> {
    FUNCS.iter().find(|f| f.name == name)
}

/// `true` nếu `name` là hàm `std.*` đã cấp.
pub fn la_std(name: &str) -> bool {
    sig_of(name).is_some()
}

/// `true` nếu `name` là tên std **đã biết nhưng hoãn** (báo thông điệp riêng).
pub fn hoan(name: &str) -> bool {
    HA_CHUA_HO_TRO.contains(&name)
}

/// `true` nếu `m` là module std được hỗ trợ.
pub fn module_ho_tro(m: &str) -> bool {
    MODULES.contains(&m)
}

/// Mọi hàm thuộc module `m` (tên đầy đủ), theo thứ tự khai.
pub fn funcs_cua(m: &str) -> impl Iterator<Item = &'static StdFn> {
    let tien = format!("{m}.");
    FUNCS.iter().filter(move |f| f.name.starts_with(&tien))
}
