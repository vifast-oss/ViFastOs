// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Bảng ngữ nghĩa dùng chung** (ST-20, VD-54 #4): toán tử, 11 tên dựng sẵn, bộ phương thức lõi.
//!
//! `vis-interp` và `vis-vm` **gọi** các hàm ở đây — **không** chép ngữ nghĩa. Hàm nào cần tác dụng
//! phụ thì nhận [`Host`] (ghi `stdout`, gọi closure cho `map/filter/each`).
//!
//! Tập phương thức đã khoá (tối thiểu để chạy VS0): `list`: `len get first last push map filter each
//! sum join`; `str`: `len upper lower split chars contains`; `map`: `len get has keys values`. Ngoài
//! tập này ⇒ **`VS0501` "chưa hỗ trợ ở VS0"** (không panic).

use vis_syntax::Span;
use vis_types::hir::BinOp;

use crate::interp::{Abort, Trap, DIV0, RT};
use crate::value::Value;

/// **Chủ nhà** cho các hàm ngữ nghĩa cần tác dụng phụ.
/// **Chương trình ngoài** (ST-27): `vis-interp` **không** biết OS — chỉ biết trait này.
///
/// `vis-cli` cài bằng `std::process::Command`; test/không host ⇒ `None` ⇒ `VS0604`.
/// Kết quả **đầy đủ** của một chương trình ngoài (ST-34, cho `| complete`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KetQua {
    pub stdout: String,
    pub stderr: String,
    pub code: i32,
}

pub trait Ngoai {
    /// Chạy `ten` với `args`; `stdin` (ST-34) ghi vào stdin rồi **đóng**;
    /// `raw` ⇒ trả `str` (nguyên văn) thay vì `list[str]`.
    fn chay(
        &mut self,
        ten: &str,
        args: &[String],
        raw: bool,
        stdin: &str,
        span: Span,
    ) -> crate::interp::R<Value>;

    /// Chạy và trả **đủ** `stdout/stderr/code` (ST-34) — cho stage `| complete`; exit≠0 **không** là lỗi.
    /// Mặc định: chưa hỗ trợ (host tự cài).
    fn chay_day_du(
        &mut self,
        ten: &str,
        args: &[String],
        stdin: &str,
        span: Span,
    ) -> Result<KetQua, crate::interp::Abort> {
        let _ = (ten, args, stdin);
        Err(trap(
            &vis_diag::COMMAND_CODES[3],
            "`complete` chưa hỗ trợ ở host này",
            span,
        ))
    }
}

/// **Host hệ thống tệp** (ST-33) — `vis-interp`/`vis-vm` **không** biết `std::fs`; `vis-cli` cài bằng
/// `std::fs`. Thiếu host ⇒ `VS0604` "chưa hỗ trợ ở môi trường này" (cùng mẫu `Ngoai`, VD-60 (2)).
/// Một mục thư mục **kèm metadata** (ST-42) — cho `ls`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MucFs {
    pub name: String,
    pub size: u64,
    pub modified_ns: i64,
    pub is_file: bool,
    /// ST-46: bit quyền kiểu POSIX (0 = chưa biết ⇒ in `-` trong `ls --long`).
    pub mode: u32,
}

/// `ls --long` (ST-46): quyền `rw-r--r--` từ bit POSIX; `0` ⇒ `-`.
pub fn chuoi_quyen(mode: u32) -> String {
    if mode == 0 {
        return "-".to_string();
    }
    let mut s = String::with_capacity(9);
    for (bit, ch) in [
        (0o400, 'r'),
        (0o200, 'w'),
        (0o100, 'x'),
        (0o040, 'r'),
        (0o020, 'w'),
        (0o010, 'x'),
        (0o004, 'r'),
        (0o002, 'w'),
        (0o001, 'x'),
    ] {
        s.push(if mode & bit != 0 { ch } else { '-' });
    }
    s
}

/// `ls --long` (ST-46): `YYYY-MM-DD HH:MM` theo **UTC** (civil-from-days, không thêm crate).
pub fn thoi_gian_utc(ns: i64) -> String {
    if ns == 0 {
        return "-".to_string();
    }
    let mut giay = ns.div_euclid(1_000_000_000);
    let ngay = giay.div_euclid(86_400);
    giay = giay.rem_euclid(86_400);
    // Howard Hinnant: days since 1970-01-01 → (y, m, d).
    let z = ngay + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}",
        giay / 3600,
        (giay % 3600) / 60
    )
}

pub trait FsHost {
    /// Đọc cả tệp thành chuỗi (UTF-8).
    fn read(&mut self, p: &str) -> Result<String, String>;
    /// Ghi đè cả tệp.
    fn write(&mut self, p: &str, s: &str) -> Result<(), String>;
    /// Tệp/thư mục có tồn tại.
    fn exists(&mut self, p: &str) -> bool;
    /// Tên các mục trong thư mục, **đã sắp xếp**, **không** đệ quy.
    fn list(&mut self, p: &str) -> Result<Vec<String>, String>;

    /// ST-42: mục thư mục **kèm metadata** (`ls`). Default: **chưa hỗ trợ** ⇒ impl cũ vẫn biên dịch.
    fn list_entries(&mut self, _p: &str) -> Result<Vec<MucFs>, String> {
        Err("môi trường này không cung cấp metadata thư mục".to_string())
    }
}

/// **Host mạng** (ST-63, VD-100/VD-105) — `vis-interp`/`vis-vm` **không** biết `std::net`; `vis-cli`
/// cài TCP thật. Thiếu host ⇒ `VS0604` (cùng mẫu [`FsHost`], VD-60 (2)). Handle là **chuỗi mờ** —
/// chương trình không thấy địa chỉ/socket; body **nhị phân** (`Vec<u8>`) để không cắt sai UTF-8.
pub trait NetHost {
    /// Nối TCP tới `dia_chi` (`host:port`), chờ tối đa `timeout_ms`. Trả **handle mờ**.
    fn connect(&mut self, dia_chi: &str, timeout_ms: u64) -> Result<String, String>;
    /// **ST-65**: lắng nghe trên `dia_chi`. Trả **handle mờ**.
    fn listen(&mut self, dia_chi: &str) -> Result<String, String>;
    /// **ST-65**: địa chỉ local của tay.
    fn local_addr(&mut self, tay: &str) -> Result<String, String>;
    /// **ST-65**: nhận một kết nối, chờ tối đa `timeout_ms`. Trả **handle mờ**.
    fn accept(&mut self, tay: &str, timeout_ms: u64) -> Result<String, String>;
    /// Đọc tối đa `toi_da` **byte**; `Ok(vec![])` = **EOF** (không phải lỗi). Hết giờ ⇒ `Err`.
    fn read(&mut self, tay: &str, toi_da: usize, timeout_ms: u64) -> Result<Vec<u8>, String>;
    /// Ghi cả `du_lieu` (body nhị phân).
    fn write(&mut self, tay: &str, du_lieu: &[u8]) -> Result<(), String>;
    /// Đóng tay; tay lạ ⇒ `Err`.
    fn close(&mut self, tay: &str) -> Result<(), String>;

    /// **ST-64**: nhật ký cho **tầng giao thức** (HTTP) khi `--explain-perms`; mặc định **không in**.
    /// CLI cài để in `NET http GET …` ra stderr; interp/VM không biết stderr.
    fn ghi_nhat_ky(&mut self, _dong: &str) {}
}

pub trait Host {
    /// Bộ đệm `stdout` của chương trình.
    fn out(&mut self) -> &mut String;
    /// Gọi một giá trị hàm (closure/builtin) với đối số đã đánh giá.
    fn call(&mut self, f: &Value, args: Vec<Value>, span: Span) -> crate::interp::R<Value>;
    /// Hệ thống tệp (ST-33). Mặc định: `None` ⇒ môi trường không có host ⇒ `VS0604`.
    fn fs(&mut self) -> Option<&mut dyn FsHost> {
        None
    }

    /// **ST-63 (VD-105)**: host mạng (TCP). Mặc định: `None` ⇒ `VS0604`.
    fn net(&mut self) -> Option<&mut dyn NetHost> {
        None
    }

    /// Chương trình **ngoài** (ST-27). Mặc định: `None` ⇒ môi trường không có host ⇒ `VS0604`.
    fn ngoai(
        &mut self,
        _ten: &str,
        _args: &[String],
        _raw: bool,
        _stdin: &str,
        _span: Span,
    ) -> Option<crate::interp::R<Value>> {
        None
    }

    /// **ST-60 (VD-95)**: hàng đợi **tác vụ** của môi trường (`$tac_vu`/`$spawn`/`$await` dùng).
    /// Một `RefCell` để hai lời gọi lồng nhau (`await` trong thân tác vụ) **không** mượn chồng.
    fn sched(&self) -> &std::cell::RefCell<crate::rt::Scheduler>;

    /// **ST-60**: chạy **thân** tác vụ theo **handle của host** — interp: `sym` của `HFn`; VM: chỉ
    /// số hàm bytecode. Handle **không** bao giờ lộ ra chương trình.
    fn goi_than(&mut self, fi: u32, args: Vec<Value>, span: Span) -> crate::interp::R<Value>;

    /// **ST-60**: tên hàm theo handle (cho thông điệp `VS0506`); mặc định rỗng.
    fn ten_than(&self, _fi: u32) -> String {
        String::new()
    }

    /// Như [`Host::ngoai`] nhưng trả **đủ** `stdout/stderr/code` (ST-34, cho `| complete`).
    fn ngoai_day_du(
        &mut self,
        _ten: &str,
        _args: &[String],
        _stdin: &str,
        _span: Span,
    ) -> Option<Result<KetQua, crate::interp::Abort>> {
        None
    }
}

/// Lỗi runtime kèm `span` (cùng cấu trúc với `Interp::trap`).
fn trap(code: &'static vis_diag::Code, msg: impl Into<String>, span: Span) -> Abort {
    Abort::Trap(Trap::new(code, msg, span))
}

/// Chữ ký builtin (số đối số tối thiểu/tối đa).
#[derive(Debug, Clone, Copy)]
pub struct BuiltinSig {
    pub name: &'static str,
    pub min: usize,
    pub max: usize,
}

/// Bảng chữ ký: **8 tên dựng sẵn** cũ + **3 lệnh chế độ lệnh** (ST-18).
// **ST-79**: `assert` (dùng cho `test fn`) — 22 mục.
pub const SIGS: [BuiltinSig; 22] = [
    BuiltinSig {
        name: "print",
        min: 1,
        max: 1,
    },
    BuiltinSig {
        name: "assert",
        min: 1,
        max: 1,
    },
    BuiltinSig {
        name: "len",
        min: 1,
        max: 1,
    },
    BuiltinSig {
        name: "str",
        min: 1,
        max: 1,
    },
    BuiltinSig {
        name: "echo",
        min: 1,
        max: 4,
    },
    BuiltinSig {
        name: "lines",
        min: 1,
        max: 1,
    },
    BuiltinSig {
        name: "take",
        min: 2,
        max: 2,
    },
    BuiltinSig {
        name: "ls",
        min: 0,
        max: 3,
    },
    BuiltinSig {
        name: "open",
        min: 1,
        max: 1,
    },
    BuiltinSig {
        name: "render",
        min: 1,
        max: 2,
    },
    BuiltinSig {
        name: "get",
        min: 2,
        max: 2,
    },
    BuiltinSig {
        name: "select",
        min: 2,
        max: 2,
    },
    BuiltinSig {
        name: "sort_by",
        min: 2,
        max: 3,
    },
    BuiltinSig {
        name: "where",
        min: 2,
        max: 2,
    },
    BuiltinSig {
        name: "each",
        min: 2,
        max: 2,
    },
    BuiltinSig {
        name: "first",
        min: 1,
        max: 1,
    },
    BuiltinSig {
        name: "count",
        min: 1,
        max: 1,
    },
    BuiltinSig {
        name: "Ok",
        min: 1,
        max: 1,
    },
    BuiltinSig {
        name: "Err",
        min: 1,
        max: 1,
    },
    BuiltinSig {
        name: "Some",
        min: 1,
        max: 1,
    },
    BuiltinSig {
        name: "None",
        min: 0,
        max: 0,
    },
    BuiltinSig {
        name: "ref",
        min: 1,
        max: 1,
    },
];

/// Tra chữ ký theo tên.
pub fn sig(name: &str) -> Option<BuiltinSig> {
    SIGS.iter().copied().find(|s| s.name == name)
}

/// `true` nếu `name` là builtin.
pub fn builtin_name(name: &str) -> Option<&'static str> {
    sig(name).map(|s| s.name)
}

pub fn bin(op: BinOp, l: Value, r: Value, span: Span) -> crate::interp::R<Value> {
    use BinOp::*;
    match op {
        Range | RangeIncl => match (&l, &r) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Range(*a, *b, matches!(op, RangeIncl))),
            _ => Err(trap(
                RT,
                format!(
                    "`{op:?}` cần hai `int`, được `{}` và `{}`",
                    l.show(),
                    r.show()
                ),
                span,
            )),
        },
        Eq => Ok(Value::Bool(l.eq_value(&r))),
        Ne => Ok(Value::Bool(!l.eq_value(&r))),
        Lt | Le | Gt | Ge => {
            let Some(ord) = l.cmp_value(&r) else {
                return Err(trap(
                    RT,
                    format!("không so sánh được `{}` và `{}`", l.show(), r.show()),
                    span,
                ));
            };
            use std::cmp::Ordering::*;
            let b = match op {
                Lt => ord == Less,
                Le => ord != Greater,
                Gt => ord == Greater,
                _ => ord != Less,
            };
            Ok(Value::Bool(b))
        }
        Add | Sub | Mul | Div | Rem | Pow => arith(op, l, r, span),
        And | Or | Coalesce | Pipe => Err(trap(
            RT,
            format!("toán tử `{op:?}` phải xử lý ở tầng biểu thức"),
            span,
        )),
    }
}

fn arith(op: BinOp, l: Value, r: Value, span: Span) -> crate::interp::R<Value> {
    use BinOp::*;
    // `int` ↔ `float`: nâng literal int (thiết kế §3.5, ngoại lệ duy nhất).
    let (l, r) = match (l, r) {
        (Value::Int(a), Value::Float(b)) => (Value::Float(a as f64), Value::Float(b)),
        (Value::Float(a), Value::Int(b)) => (Value::Float(a), Value::Float(b as f64)),
        (l, r) => (l, r),
    };
    // `percent` với số: `percent * int -> int`, `percent * size -> size` (§3.6).
    if let (Value::Percent(p), rhs) = (&l, &r) {
        match (op, rhs) {
            (Mul, Value::Int(n)) => return Ok(Value::Int((p * (*n as f64)) as i64)),
            (Mul, Value::Size(n)) => return Ok(Value::Size((p * (*n as f64)) as i64)),
            (Mul, Value::Duration(n)) => return Ok(Value::Duration((p * (*n as f64)) as i64)),
            (Add | Sub, Value::Percent(q)) => {
                return Ok(Value::Percent(if matches!(op, Add) {
                    p + q
                } else {
                    p - q
                }))
            }
            (Div, Value::Int(n)) => {
                if *n == 0 {
                    return Err(trap(DIV0, "chia cho 0", span));
                }
                return Ok(Value::Percent(p / (*n as f64)));
            }
            _ => {}
        }
    }
    if let (Value::Int(n), Value::Percent(p)) = (&l, &r) {
        if matches!(op, Mul) {
            return Ok(Value::Int(((*n as f64) * p) as i64));
        }
    }
    // ST-13b (mục 5): đối xứng `size/duration * percent` và `percent / percent` (VD-48: **float**).
    if let (Value::Size(n), Value::Percent(p)) = (&l, &r) {
        if matches!(op, Mul) {
            return Ok(Value::Size(((*n as f64) * p) as i64));
        }
    }
    if let (Value::Duration(n), Value::Percent(p)) = (&l, &r) {
        if matches!(op, Mul) {
            return Ok(Value::Duration(((*n as f64) * p) as i64));
        }
    }
    if let (Value::Percent(a), Value::Percent(b)) = (&l, &r) {
        match op {
            // VD-48(b): `percent / percent` ⇒ **float** (tỉ lệ của hai tỉ lệ).
            Div => {
                if *b == 0.0 {
                    return Err(trap(DIV0, "chia cho 0", span));
                }
                return Ok(Value::Float(a / b));
            }
            Rem => {
                if *b == 0.0 {
                    return Err(trap(DIV0, "lấy dư cho 0", span));
                }
                return Ok(Value::Percent(a % b));
            }
            _ => {}
        }
    }
    if let (Value::Size(a), Value::Size(b)) = (&l, &r) {
        if matches!(op, Rem) {
            if *b == 0 {
                return Err(trap(DIV0, "lấy dư cho 0", span));
            }
            return Ok(Value::Size(a % b));
        }
    }
    if let (Value::Duration(a), Value::Duration(b)) = (&l, &r) {
        if matches!(op, Rem) {
            if *b == 0 {
                return Err(trap(DIV0, "lấy dư cho 0", span));
            }
            return Ok(Value::Duration(a % b));
        }
    }
    match (op, l, r) {
        // int
        (Add, Value::Int(a), Value::Int(b)) => {
            Ok(Value::Int(a.checked_add(b).ok_or_else(|| overflow(span))?))
        }
        (Sub, Value::Int(a), Value::Int(b)) => {
            Ok(Value::Int(a.checked_sub(b).ok_or_else(|| overflow(span))?))
        }
        (Mul, Value::Int(a), Value::Int(b)) => {
            Ok(Value::Int(a.checked_mul(b).ok_or_else(|| overflow(span))?))
        }
        (Div, Value::Int(a), Value::Int(b)) => {
            if b == 0 {
                return Err(trap(DIV0, "chia cho 0 (`/0`)", span));
            }
            Ok(Value::Int(a.checked_div(b).ok_or_else(|| overflow(span))?))
        }
        (Rem, Value::Int(a), Value::Int(b)) => {
            if b == 0 {
                return Err(trap(DIV0, "lấy dư cho 0 (`%0`)", span));
            }
            Ok(Value::Int(a.wrapping_rem(b)))
        }
        (Pow, Value::Int(a), Value::Int(b)) => {
            if b < 0 {
                return Err(trap(RT, "`int ** int` với số mũ âm (VD-43 #2)", span));
            }
            let e = u32::try_from(b).map_err(|_| overflow(span))?;
            Ok(Value::Int(a.checked_pow(e).ok_or_else(|| overflow(span))?))
        }
        // float (IEEE 754 — `/0.0` không lỗi)
        (Add, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
        (Sub, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
        (Mul, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
        (Div, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a / b)),
        (Rem, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a % b)),
        (Pow, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a.powf(b))),
        // size
        (Add, Value::Size(a), Value::Size(b)) => {
            Ok(Value::Size(a.checked_add(b).ok_or_else(|| overflow(span))?))
        }
        (Sub, Value::Size(a), Value::Size(b)) => {
            Ok(Value::Size(a.checked_sub(b).ok_or_else(|| overflow(span))?))
        }
        (Mul, Value::Size(a), Value::Int(b)) => {
            Ok(Value::Size(a.checked_mul(b).ok_or_else(|| overflow(span))?))
        }
        (Mul, Value::Int(a), Value::Size(b)) => {
            Ok(Value::Size(a.checked_mul(b).ok_or_else(|| overflow(span))?))
        }
        (Div, Value::Size(a), Value::Int(b)) => {
            if b == 0 {
                return Err(trap(DIV0, "chia cho 0", span));
            }
            Ok(Value::Size(a / b))
        }
        (Div, Value::Size(a), Value::Size(b)) => {
            if b == 0 {
                return Err(trap(DIV0, "chia cho 0", span));
            }
            Ok(Value::Float(a as f64 / b as f64))
        }
        (Rem, Value::Size(a), Value::Int(b)) => {
            if b == 0 {
                return Err(trap(DIV0, "lấy dư cho 0", span));
            }
            Ok(Value::Size(a % b))
        }
        // duration
        (Add, Value::Duration(a), Value::Duration(b)) => Ok(Value::Duration(
            a.checked_add(b).ok_or_else(|| overflow(span))?,
        )),
        (Sub, Value::Duration(a), Value::Duration(b)) => Ok(Value::Duration(
            a.checked_sub(b).ok_or_else(|| overflow(span))?,
        )),
        (Mul, Value::Duration(a), Value::Int(b)) => Ok(Value::Duration(
            a.checked_mul(b).ok_or_else(|| overflow(span))?,
        )),
        (Mul, Value::Int(a), Value::Duration(b)) => Ok(Value::Duration(
            a.checked_mul(b).ok_or_else(|| overflow(span))?,
        )),
        (Div, Value::Duration(a), Value::Int(b)) => {
            if b == 0 {
                return Err(trap(DIV0, "chia cho 0", span));
            }
            Ok(Value::Duration(a / b))
        }
        (Div, Value::Duration(a), Value::Duration(b)) => {
            if b == 0 {
                return Err(trap(DIV0, "chia cho 0", span));
            }
            Ok(Value::Float(a as f64 / b as f64))
        }
        (Rem, Value::Duration(a), Value::Int(b)) => {
            if b == 0 {
                return Err(trap(DIV0, "lấy dư cho 0", span));
            }
            Ok(Value::Duration(a % b))
        }
        // percent
        (Add, Value::Percent(a), Value::Percent(b)) => Ok(Value::Percent(a + b)),
        (Sub, Value::Percent(a), Value::Percent(b)) => Ok(Value::Percent(a - b)),
        // còn lại: chưa hỗ trợ ở VS0
        (op, l, r) => Err(trap(
            RT,
            format!(
                "`{op:?}` chưa hỗ trợ cho `{}` và `{}` ở VS0",
                l.show(),
                r.show()
            ),
            span,
        )),
    }
}

pub fn un(op: vis_types::hir::UnOp, v: Value, span: Span) -> crate::interp::R<Value> {
    match op {
        vis_types::hir::UnOp::Neg => match v {
            Value::Int(i) => i
                .checked_neg()
                .map(Value::Int)
                .ok_or_else(|| trap(RT, "tràn i64 khi đổi dấu", span)),
            Value::Float(f) => Ok(Value::Float(-f)),
            other => Err(trap(
                RT,
                format!("`-` không dùng được cho `{}`", other.show()),
                span,
            )),
        },
        vis_types::hir::UnOp::Not => Ok(Value::Bool(!v.truthy(span)?)),
    }
}

/// Tách `stdout` thành `list[str]` — **bỏ** newline cuối, không sinh dòng rỗng thừa (ST-34).
fn dong_cua(s: &str) -> Value {
    let mut t = s;
    if let Some(x) = t.strip_suffix('\n') {
        t = x;
    }
    let mut dong: Vec<Value> = Vec::new();
    if !t.is_empty() {
        for d in t.split('\n') {
            dong.push(Value::Str(d.to_string()));
        }
    }
    Value::List(dong)
}

/// Record `{stdout: list[str], stderr: list[str], code: int}` cho `| complete` (ST-34).
fn nha_ket_qua(kq: KetQua) -> Value {
    Value::Record(
        String::new(),
        vec![
            ("stdout".into(), dong_cua(&kq.stdout)),
            ("stderr".into(), dong_cua(&kq.stderr)),
            ("code".into(), Value::Int(kq.code as i64)),
        ],
    )
}

/// **ST-53**: lõi **so sánh khoá dùng chung** cho shell `sort_by` và `std.list.sort_by`.
/// Khoá tính **một lần**/phần tử; sắp xếp **ổn định**; `desc` chỉ đảo comparator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KieuKhoa {
    /// `std.list.sort_by` **ST-56**: `int` **hoặc** `str`, nhưng **một** kiểu trong cả lần gọi
    /// (trộn ⇒ `VS0501` nêu **cả hai** kiểu thật). Cùng comparator `cmp_value` với shell ⇒ thứ tự
    /// chuỗi **nhất quán** shell `sort_by` (byte UTF-8).
    ChiIntHoacStr,
    /// Shell `sort_by`: `int/float/size/duration/percent/str` (bool/không so được ⇒ lỗi).
    Moi,
}

pub(crate) fn sap_xep_theo_khoa(
    host: &mut dyn Host,
    xs: &[Value],
    f: &Value,
    desc: bool,
    kieu: KieuKhoa,
    ten: &str,
    span: Span,
) -> crate::interp::R<Value> {
    let mut cap: Vec<(Value, Value)> = Vec::with_capacity(xs.len());
    // ST-56: kiểu khoá **đã gặp** ở `ChiIntHoacStr` — trộn `int`/`str` ⇒ lỗi (không so được).
    let mut loai_da_gap: Option<&'static str> = None;
    for x in xs.iter().cloned() {
        let k = host.call(f, vec![x.clone()], span)?;
        match kieu {
            KieuKhoa::ChiIntHoacStr => {
                let loai = match &k {
                    Value::Int(_) => "int",
                    Value::Str(_) => "str",
                    other => {
                        return Err(trap(
                            RT,
                            format!(
                                "`{ten}`: khoá phải là `int` hoặc `str`, được `{}`",
                                other.show()
                            ),
                            span,
                        ))
                    }
                };
                match loai_da_gap {
                    None => loai_da_gap = Some(loai),
                    Some(truoc) if truoc != loai => {
                        return Err(trap(
                            RT,
                            format!(
                                "`{ten}`: khoá trộn kiểu trong một lần gọi (`{truoc}` và `{loai}`)"
                            ),
                            span,
                        ))
                    }
                    Some(_) => {}
                }
            }
            KieuKhoa::Moi => {
                if matches!(k, Value::Bool(_)) || k.cmp_value(&k).is_none() {
                    return Err(trap(
                        RT,
                        format!("`{ten}`: khoá không so được: `{}`", k.show()),
                        span,
                    ));
                }
            }
        }
        cap.push((k, x));
    }
    cap.sort_by(|a, b| {
        let o = a.0.cmp_value(&b.0).unwrap_or(std::cmp::Ordering::Equal);
        if desc {
            o.reverse()
        } else {
            o
        }
    });
    Ok(Value::List(cap.into_iter().map(|(_, v)| v).collect()))
}

pub fn call(
    host: &mut dyn Host,
    name: &str,
    args: Vec<Value>,
    span: Span,
) -> crate::interp::R<Value> {
    // **ST-60 (VD-95)**: ba builtin **nội bộ** của async — tên có `$` ⇒ người dùng **không** viết
    // được. `Int` của chúng là **handle của host** (interp: `sym`; VM: chỉ số hàm bytecode),
    // **không** phải giá trị chương trình. Phòng thủ: bytecode đột biến ⇒ `VS0501`, **không** panic.
    if let Some(kieu) = name.strip_prefix('$') {
        // **ST-61 (VD-96)**: `$kenh(n)` — tạo kênh đệm `n` (`n ≥ 0`; âm ⇒ `VS0501`).
        if kieu == "kenh" {
            return match args.into_iter().next() {
                Some(Value::Int(cap)) => crate::rt::tao_kenh(host, cap, span),
                _ => Err(trap(
                    crate::interp::RT,
                    "`chan[T](n)` cần dung lượng `int`",
                    span,
                )),
            };
        }
        if kieu == "await" {
            return match args.into_iter().next() {
                Some(Value::Task(t)) => crate::rt::await_task(host, &t, span),
                _ => Err(trap(crate::interp::RT, "`await` cần một `Task[T]`", span)),
            };
        }
        let fi = match args.first() {
            Some(Value::Int(i)) if *i >= 0 && *i <= u32::MAX as i64 => *i as u32,
            _ => {
                return Err(trap(
                    crate::interp::RT,
                    format!("`{name}`: handle tác vụ không hợp lệ"),
                    span,
                ))
            }
        };
        let con: Vec<Value> = args.into_iter().skip(1).collect();
        return match kieu {
            "tac_vu" => {
                let t = host
                    .sched()
                    .borrow_mut()
                    .tao(fi, con, host.ten_than(fi), span);
                Ok(Value::Task(t))
            }
            "spawn" => {
                let t = host
                    .sched()
                    .borrow_mut()
                    .spawn(fi, con, host.ten_than(fi), span);
                Ok(Value::Task(t))
            }
            _ => Err(trap(
                crate::interp::RT,
                format!("builtin nội bộ `{name}` chưa hỗ trợ"),
                span,
            )),
        };
    }
    // ST-27/ST-34: `exec.[complete.][raw.]<ten>` — chương trình ngoài, do **host** chạy.
    // - đối số 0: `argv` (`list[str]`); đối số 1 (ST-34): **stdin** (`str` hoặc `list[str]`);
    // - tiền tố `complete.` ⇒ trả **record** `{stdout, stderr, code}`, exit≠0 **không** là lỗi.
    if let Some(mut ten) = name.strip_prefix("exec.") {
        let mut complete = false;
        if let Some(t) = ten.strip_prefix("complete.") {
            ten = t;
            complete = true;
        }
        let mut raw = false;
        if let Some(t) = ten.strip_prefix("raw.") {
            ten = t;
            raw = true;
        }
        let ds: Vec<String> = match args.first() {
            Some(Value::List(xs)) => xs
                .iter()
                .map(|v| match v {
                    Value::Str(s) => s.clone(),
                    other => other.show(),
                })
                .collect(),
            _ => Vec::new(),
        };
        // **Một** quy tắc cho cả hai đường (ST-34): `list[str]` ⇒ nối `\n` **+ newline cuối**;
        // `str` ⇒ **nguyên văn** (không thêm newline); khác ⇒ `show()`.
        let stdin = match args.get(1) {
            Some(Value::Str(s)) => s.clone(),
            Some(Value::List(xs)) => {
                let mut t = xs
                    .iter()
                    .map(|v| match v {
                        Value::Str(s) => s.clone(),
                        other => other.show(),
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                if !t.is_empty() {
                    t.push('\n');
                }
                t
            }
            Some(other) => other.show(),
            None => String::new(),
        };
        if complete {
            return match host.ngoai_day_du(ten, &ds, &stdin, span) {
                Some(Ok(kq)) => Ok(nha_ket_qua(kq)),
                Some(Err(e)) => Err(e),
                None => Err(trap(
                    &vis_diag::COMMAND_CODES[3],
                    format!(
                        "chương trình ngoài `{ten}` chưa hỗ trợ ở môi trường này (không có host)"
                    ),
                    span,
                )),
            };
        }
        return match host.ngoai(ten, &ds, raw, &stdin, span) {
            Some(kq) => kq,
            None => Err(trap(
                &vis_diag::COMMAND_CODES[3],
                format!("chương trình ngoài `{ten}` chưa hỗ trợ ở môi trường này (không có host)"),
                span,
            )),
        };
    }
    // ST-26: `std.*` — một hiện thực dùng chung cho interp và VM (VD-55).
    if name.starts_with("std.") {
        return crate::stdlib::call(host, name.trim_start_matches("std."), args, span);
    }
    match name {
        "print" => {
            if args.len() != 1 {
                return Err(trap(
                    RT,
                    format!("`print` cần 1 đối số, được {}", args.len()),
                    span,
                ));
            }
            host.out().push_str(&args[0].show());
            host.out().push('\n');
            Ok(Value::Unit)
        }
        "str" => {
            if args.len() != 1 {
                return Err(trap(RT, "`str` cần 1 đối số", span));
            }
            Ok(Value::Str(args[0].show()))
        }
        // **ST-79 (VD-108)**: `assert(điều_kiện)` cho `test fn`; sai ⇒ trap `VS0501` để
        // `vis test` in `not ok` (không thêm mã mới — yêu cầu ST-79).
        "assert" => {
            if args.len() != 1 {
                return Err(trap(RT, "`assert` cần 1 đối số", span));
            }
            match &args[0] {
                Value::Bool(true) => Ok(Value::Unit),
                Value::Bool(false) => Err(trap(RT, "assert thất bại", span)),
                v => Err(trap(
                    RT,
                    format!("`assert` cần `bool`, được `{}`", v.show()),
                    span,
                )),
            }
        }
        // ST-18 — 3 lệnh builtin chế độ lệnh (`echo`/`lines`/`take`), thuần, không FS.
        "echo" => {
            if args.len() > 4 {
                return Err(trap(RT, "`echo` nhận tối đa 4 đối số", span));
            }
            let upper = match args.get(3) {
                Some(Value::Bool(b)) => *b,
                None => false,
                Some(_) => return Err(trap(RT, "`--upper` cần `bool`", span)),
            };
            let mut phan: Vec<String> = Vec::new();
            for a in args.iter().take(3) {
                match a {
                    Value::Str(s) => {
                        if !s.is_empty() {
                            phan.push(s.clone());
                        }
                    }
                    _ => return Err(trap(RT, "`echo` cần đối số `str`", span)),
                }
            }
            let kq = phan.join(" ");
            Ok(Value::Str(if upper { kq.to_uppercase() } else { kq }))
        }
        "lines" => {
            if args.len() != 1 {
                return Err(trap(RT, "`lines` cần 1 đối số", span));
            }
            let Value::Str(s) = &args[0] else {
                return Err(trap(RT, "`lines` cần `str`", span));
            };
            let mut ds: Vec<Value> = s.split('\n').map(|x| Value::Str(x.to_string())).collect();
            if matches!(ds.last(), Some(Value::Str(x)) if x.is_empty()) {
                ds.pop();
            }
            Ok(Value::List(ds))
        }
        "take" => {
            if args.len() != 2 {
                return Err(trap(RT, "`take` cần 2 đối số", span));
            }
            let (Value::List(xs), Value::Int(n)) = (&args[0], &args[1]) else {
                return Err(trap(RT, "`take` cần `list[str]` và `int`", span));
            };
            let n = (*n).max(0) as usize;
            Ok(Value::List(xs.iter().take(n).cloned().collect()))
        }
        // ST-42: `ls`/`open` qua `FsHost` (thật hoặc `--fs-mock`).
        "ls" => {
            let p = match args.first() {
                Some(Value::Str(s)) => s.clone(),
                None => ".".to_string(),
                Some(other) => {
                    return Err(trap(
                        RT,
                        format!("`ls`: đường dẫn cần `str`, được `{}`", other.show()),
                        span,
                    ))
                }
            };
            let all = matches!(args.get(1), Some(Value::Bool(true)));
            let host = host.fs().ok_or_else(|| {
                trap(
                    RT,
                    "`ls` chưa hỗ trợ ở môi trường này (không có host hệ tệp)",
                    span,
                )
            })?;
            let muc = host
                .list_entries(&p)
                .map_err(|e| trap(RT, format!("`ls {p}`: {e}"), span))?;
            let mut ds: Vec<MucFs> = muc
                .into_iter()
                .filter(|m| all || !m.name.starts_with('.'))
                .collect();
            // **Sắp theo tên (byte)** — ổn định giữa hệ tệp.
            ds.sort_by(|a, b| a.name.as_bytes().cmp(b.name.as_bytes()));
            let long = matches!(args.get(2), Some(Value::Bool(true)));
            Ok(Value::List(
                ds.into_iter()
                    .map(|m| {
                        let mut truong = vec![
                            ("name".to_string(), Value::Str(m.name)),
                            ("size".to_string(), Value::Size(m.size as i64)),
                            ("modified".to_string(), Value::Duration(m.modified_ns)),
                            ("is_file".to_string(), Value::Bool(m.is_file)),
                        ];
                        let ten = if long {
                            truong.push(("mode".to_string(), Value::Str(chuoi_quyen(m.mode))));
                            truong.push((
                                "modified_time".to_string(),
                                Value::Str(thoi_gian_utc(m.modified_ns)),
                            ));
                            "FileEntryLong"
                        } else {
                            "FileEntry"
                        };
                        Value::Record(ten.to_string(), truong)
                    })
                    .collect(),
            ))
        }
        "open" => {
            // ST-56 (phòng thủ): bytecode bị **đột biến** (`fuzz_mutate`) có thể gọi với `argc`
            // sai ⇒ `args[0]` panic. Front end **đã** kiểm số đối số ⇒ mã hợp lệ không đổi.
            let Some(Value::Str(p)) = args.first() else {
                return Err(trap(RT, "`open` cần `str`", span));
            };
            let host = host.fs().ok_or_else(|| {
                trap(
                    RT,
                    "`open` chưa hỗ trợ ở môi trường này (không có host hệ tệp)",
                    span,
                )
            })?;
            let s = host
                .read(p)
                .map_err(|e| trap(RT, format!("`open {p}`: {e}"), span))?;
            Ok(Value::Str(s))
        }
        // ST-41: `Render` — **chỉ** cho giá trị cuối pipeline (`boc_render`); `print` giữ nguyên.
        "render" => {
            // ST-56 (phòng thủ): `argc = 0` từ bytecode **đột biến** từng panic ở đây.
            let Some(v) = args.first() else {
                return Err(trap(RT, "`render` cần 1 đối số", span));
            };
            let co_ban_ghi = matches!(args.get(1), Some(Value::Bool(true)));
            match crate::render::in_gia_tri(v, co_ban_ghi) {
                Some(s) => {
                    host.out().push_str(&s);
                    Ok(Value::Unit)
                }
                None => {
                    host.out().push_str(&v.show());
                    host.out().push('\n');
                    Ok(Value::Unit)
                }
            }
        }
        // ST-40b: `get`/`select` — chiếu từng phần tử qua closure (cùng một hiện thực cho hai đường).
        "get" | "select" => {
            // ST-56 (phòng thủ): số đối số — xem ghi chú ở `open`.
            let (Some(Value::List(xs)), Some(f)) = (args.first(), args.get(1)) else {
                return Err(trap(RT, format!("`{name}` cần `list` và hàm"), span));
            };
            let f = f.clone();
            let mut out: Vec<Value> = Vec::with_capacity(xs.len());
            for x in xs.clone() {
                out.push(host.call(&f, vec![x], span)?);
            }
            Ok(Value::List(out))
        }
        // ST-39: `sort_by` — khoá `int/size/duration/percent/str`; **ổn định** cả hai chiều (đảo
        // comparator, `sort_by` của Rust ổn định), `--desc` mặc định `false`.
        "sort_by" => {
            // ST-56 (phòng thủ): số đối số — xem ghi chú ở `open`.
            let (Some(Value::List(xs)), Some(f)) = (args.first(), args.get(1)) else {
                return Err(trap(RT, "`sort_by` cần `list` và hàm", span));
            };
            let desc = match args.get(2) {
                None => false,
                Some(Value::Bool(b)) => *b,
                Some(other) => {
                    return Err(trap(
                        RT,
                        format!("`sort_by`: `--desc` cần `bool`, được `{}`", other.show()),
                        span,
                    ))
                }
            };
            // ST-53: cùng lõi với `std.list.sort_by` (khoá rộng hơn, `--desc`).
            sap_xep_theo_khoa(host, xs, f, desc, KieuKhoa::Moi, "sort_by", span)
        }
        // ST-38: `where`/`each` — closure-arg; **một** hiện thực cho cả hai đường (`Host::call`).
        "where" => {
            if args.len() != 2 {
                return Err(trap(RT, "`where` cần 2 đối số", span));
            }
            let Value::List(xs) = &args[0] else {
                return Err(trap(RT, "`where` cần `list`", span));
            };
            let f = args[1].clone();
            let mut out: Vec<Value> = Vec::new();
            for x in xs.clone() {
                match host.call(&f, vec![x.clone()], span)? {
                    Value::Bool(true) => out.push(x),
                    Value::Bool(false) => {}
                    other => {
                        return Err(trap(
                            RT,
                            format!("`where`: hàm lọc phải trả `bool`, được `{}`", other.show()),
                            span,
                        ))
                    }
                }
            }
            Ok(Value::List(out))
        }
        "each" => {
            if args.len() != 2 {
                return Err(trap(RT, "`each` cần 2 đối số", span));
            }
            let Value::List(xs) = &args[0] else {
                return Err(trap(RT, "`each` cần `list`", span));
            };
            let f = args[1].clone();
            for x in xs.clone() {
                host.call(&f, vec![x], span)?;
            }
            Ok(Value::Unit)
        }
        "first" => {
            if args.len() != 1 {
                return Err(trap(RT, "`first` cần 1 đối số", span));
            }
            let Value::List(xs) = &args[0] else {
                return Err(trap(RT, "`first` cần `list`", span));
            };
            Ok(Value::Opt(xs.first().cloned().map(Box::new)))
        }
        "count" => {
            if args.len() != 1 {
                return Err(trap(RT, "`count` cần 1 đối số", span));
            }
            let Value::List(xs) = &args[0] else {
                return Err(trap(RT, "`count` cần `list`", span));
            };
            Ok(Value::Int(xs.len() as i64))
        }
        "len" => {
            if args.len() != 1 {
                return Err(trap(RT, "`len` cần 1 đối số", span));
            }
            match &args[0] {
                Value::Str(s) => Ok(Value::Int(s.chars().count() as i64)),
                Value::List(xs) => Ok(Value::Int(xs.len() as i64)),
                Value::Map(kv) => Ok(Value::Int(kv.len() as i64)),
                Value::Bytes(b) => Ok(Value::Int(b.len() as i64)),
                other => Err(trap(
                    RT,
                    format!("`len` chưa hỗ trợ cho `{}` ở VS0", other.show()),
                    span,
                )),
            }
        }
        "Some" => {
            if args.len() != 1 {
                return Err(trap(RT, "`Some` cần 1 đối số", span));
            }
            let mut it = args.into_iter();
            let v = it.next().unwrap_or(Value::Unit);
            Ok(Value::Opt(Some(Box::new(v))))
        }
        "None" => Ok(Value::Opt(None)),
        "Ok" => {
            if args.len() != 1 {
                return Err(trap(RT, "`Ok` cần 1 đối số", span));
            }
            let mut it = args.into_iter();
            let v = it.next().unwrap_or(Value::Unit);
            Ok(Value::Res(Ok(Box::new(v))))
        }
        "Err" => {
            if args.len() != 1 {
                return Err(trap(RT, "`Err` cần 1 đối số", span));
            }
            let mut it = args.into_iter();
            let v = it.next().unwrap_or(Value::Unit);
            Ok(Value::Res(Err(Box::new(v))))
        }
        // ST-56 (VD-92): `ref(x)` là **biểu thức** hạ identity — cái tên `ref` **không** phải hàm.
        "ref" => Err(trap(
            RT,
            "`ref` chỉ là biểu thức `ref(x)`, không phải hàm",
            span,
        )),
        other => Err(trap(RT, format!("tên `{other}` chưa hỗ trợ ở VS0"), span)),
    }
}

pub fn method(
    host: &mut dyn Host,
    recv: Value,
    name: &str,
    args: Vec<Value>,
    span: Span,
) -> crate::interp::R<Value> {
    match recv {
        Value::List(xs) => list_method(host, xs, name, args, span),
        Value::Str(s) => str_method(host, s, name, args, span),
        Value::Map(kv) => map_method(host, kv, name, args, span),
        // **ST-61 (VD-96)**: `send`/`recv`/`close` — **một** hiện thực cho cả interp lẫn VM.
        Value::Chan(c) => chan_method(host, c, name, args, span),
        other => Err(trap(
            RT,
            format!("`{}` không có phương thức `{name}`", other.show()),
            span,
        )),
    }
}

/// **ST-61 (VD-96)**: phương thức kênh. Chặn (`send` đầy, `recv` rỗng) ⇒ **drain** hàng đợi sẵn
/// sàng ([`crate::rt::chay_mot`]) rồi thử lại; hết queue ⇒ **`VS0505`** (deadlock, **không** treo).
/// `send` sau `close` ⇒ **`VS0504`**; `close` **idempotent**; `recv` sau close trả nốt hàng rồi `none`.
fn chan_method(
    host: &mut dyn Host,
    c: std::rc::Rc<std::cell::RefCell<crate::rt::Chan>>,
    name: &str,
    args: Vec<Value>,
    span: Span,
) -> crate::interp::R<Value> {
    match name {
        "send" => {
            let Some(v) = args.into_iter().next() else {
                return Err(trap(RT, "`send` cần 1 đối số", span));
            };
            loop {
                {
                    let mut ch = c.borrow_mut();
                    if ch.closed {
                        return Err(trap(
                            crate::interp::SEND_CLOSED,
                            "`send` vào kênh đã `close` (không nhận thêm phần tử)",
                            span,
                        ));
                    }
                    if !ch.day() {
                        ch.buf.push_back(v);
                        return Ok(Value::Unit);
                    }
                }
                if !crate::rt::chay_mot(host)? {
                    return Err(trap(
                        crate::interp::DEADLOCK,
                        "deadlock: `send` vào kênh **đầy** và không còn tác vụ nào chạy được",
                        span,
                    ));
                }
            }
        }
        "recv" => loop {
            {
                let mut ch = c.borrow_mut();
                if let Some(v) = ch.buf.pop_front() {
                    return Ok(Value::Opt(Some(Box::new(v))));
                }
                if ch.closed {
                    return Ok(Value::Opt(None));
                }
            }
            if !crate::rt::chay_mot(host)? {
                return Err(trap(
                    crate::interp::DEADLOCK,
                    "deadlock: `recv` trên kênh **rỗng** và không còn tác vụ nào chạy được",
                    span,
                ));
            }
        },
        // `close` **idempotent** (VD-96 (3)): lần hai **không** lỗi.
        "close" => {
            c.borrow_mut().closed = true;
            Ok(Value::Unit)
        }
        _ => Err(trap(
            RT,
            format!("kênh không có phương thức `{name}` (chỉ `send`, `recv`, `close`)"),
            span,
        )),
    }
}

fn list_method(
    host: &mut dyn Host,
    xs: Vec<Value>,
    name: &str,
    args: Vec<Value>,
    span: Span,
) -> crate::interp::R<Value> {
    match name {
        "len" => Ok(Value::Int(xs.len() as i64)),
        "get" => {
            let Some(Value::Int(i)) = args.first() else {
                return Err(trap(RT, "`get` cần chỉ số `int`", span));
            };
            Ok(Value::Opt(match xs.get(*i as usize) {
                Some(v) if *i >= 0 => Some(Box::new(v.clone())),
                _ => None,
            }))
        }
        "first" => Ok(Value::Opt(xs.first().cloned().map(Box::new))),
        "last" => Ok(Value::Opt(xs.last().cloned().map(Box::new))),
        "push" => {
            if args.len() != 1 {
                return Err(trap(RT, "`push` cần 1 đối số", span));
            }
            let mut out = xs;
            out.push(args[0].clone());
            Ok(Value::List(out))
        }
        "map" => {
            let f = args.first().cloned().unwrap_or(Value::Unit);
            let mut out = Vec::with_capacity(xs.len());
            for x in xs {
                out.push(host.call(&f, vec![x], span)?);
            }
            Ok(Value::List(out))
        }
        "filter" => {
            let f = args.first().cloned().unwrap_or(Value::Unit);
            let mut out = Vec::new();
            for x in xs {
                let keep = host.call(&f, vec![x.clone()], span)?;
                if keep.truthy(span)? {
                    out.push(x);
                }
            }
            Ok(Value::List(out))
        }
        "each" => {
            let f = args.first().cloned().unwrap_or(Value::Unit);
            for x in xs {
                host.call(&f, vec![x], span)?;
            }
            Ok(Value::Unit)
        }
        "sum" => {
            let mut acc: Option<Value> = None;
            for x in xs {
                acc = Some(match acc {
                    None => x,
                    Some(a) => bin(BinOp::Add, a, x, span)?,
                });
            }
            Ok(acc.unwrap_or(Value::Int(0)))
        }
        "join" => {
            let sep = match args.first() {
                Some(Value::Str(s)) => s.clone(),
                _ => return Err(trap(RT, "`join` cần dấu ngăn `str`", span)),
            };
            let parts: Vec<String> = xs.iter().map(Value::show).collect();
            Ok(Value::Str(parts.join(&sep)))
        }
        other => Err(trap(
            RT,
            format!("`list.{other}()` chưa hỗ trợ ở VS0 (danh sách: len get first last push map filter each sum join)"),
            span,
        )),
    }
}

fn str_method(
    _host: &mut dyn Host,
    s: String,
    name: &str,
    args: Vec<Value>,
    span: Span,
) -> crate::interp::R<Value> {
    match name {
        "len" => Ok(Value::Int(s.chars().count() as i64)),
        "upper" => Ok(Value::Str(s.to_uppercase())),
        "lower" => Ok(Value::Str(s.to_lowercase())),
        "chars" => Ok(Value::List(
            s.chars().map(|c| Value::Str(c.to_string())).collect(),
        )),
        "contains" => {
            let Some(Value::Str(sub)) = args.first() else {
                return Err(trap(RT, "`contains` cần `str`", span));
            };
            Ok(Value::Bool(s.contains(sub.as_str())))
        }
        "split" => {
            let Some(Value::Str(sep)) = args.first() else {
                return Err(trap(RT, "`split` cần dấu ngăn `str`", span));
            };
            if sep.is_empty() {
                return Err(trap(RT, "`split` cần dấu ngăn khác rỗng", span));
            }
            Ok(Value::List(
                s.split(sep.as_str()).map(|p| Value::Str(p.to_string())).collect(),
            ))
        }
        other => Err(trap(
            RT,
            format!("`str.{other}()` chưa hỗ trợ ở VS0 (danh sách: len upper lower split chars contains)"),
            span,
        )),
    }
}

fn map_method(
    _host: &mut dyn Host,
    kv: Vec<(Value, Value)>,
    name: &str,
    args: Vec<Value>,
    span: Span,
) -> crate::interp::R<Value> {
    match name {
        "len" => Ok(Value::Int(kv.len() as i64)),
        "get" => {
            let Some(k) = args.first() else {
                return Err(trap(RT, "`get` cần 1 khoá", span));
            };
            Ok(Value::Opt(
                kv.iter()
                    .find(|(kk, _)| kk.eq_value(k))
                    .map(|(_, v)| Box::new(v.clone())),
            ))
        }
        "has" => {
            let Some(k) = args.first() else {
                return Err(trap(RT, "`has` cần 1 khoá", span));
            };
            Ok(Value::Bool(kv.iter().any(|(kk, _)| kk.eq_value(k))))
        }
        "keys" => Ok(Value::List(kv.into_iter().map(|(k, _)| k).collect())),
        "values" => Ok(Value::List(kv.into_iter().map(|(_, v)| v).collect())),
        other => Err(trap(
            RT,
            format!("`map.{other}()` chưa hỗ trợ ở VS0 (danh sách: len get has keys values)"),
            span,
        )),
    }
}
/// Tràn `i64` — `VS0501` (VD-43 #2).
fn overflow(span: Span) -> Abort {
    trap(RT, "tràn i64 lúc chạy (VD-43 #2)", span)
}
