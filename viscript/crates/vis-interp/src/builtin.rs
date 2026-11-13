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
pub trait FsHost {
    /// Đọc cả tệp thành chuỗi (UTF-8).
    fn read(&mut self, p: &str) -> Result<String, String>;
    /// Ghi đè cả tệp.
    fn write(&mut self, p: &str, s: &str) -> Result<(), String>;
    /// Tệp/thư mục có tồn tại.
    fn exists(&mut self, p: &str) -> bool;
    /// Tên các mục trong thư mục, **đã sắp xếp**, **không** đệ quy.
    fn list(&mut self, p: &str) -> Result<Vec<String>, String>;
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
pub const SIGS: [BuiltinSig; 15] = [
    BuiltinSig {
        name: "print",
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

pub fn call(
    host: &mut dyn Host,
    name: &str,
    args: Vec<Value>,
    span: Span,
) -> crate::interp::R<Value> {
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
        "ref" => Err(trap(RT, "`ref(x)` là VS1 — chưa hỗ trợ ở VS0", span)),
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
        other => Err(trap(
            RT,
            format!("`{}` không có phương thức `{name}`", other.show()),
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
