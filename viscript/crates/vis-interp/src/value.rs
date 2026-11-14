// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Giá trị lúc chạy** (`Value`) + `Show` (`str(x)` = `x.show()`, thiết kế §3.5) + đọc literal
//! `size`/`duration`/`percent` và escape chuỗi.
//!
//! # Đơn vị (chờ Agent 0 chốt — VD-53)
//!
//! - `size` lưu **byte** (`kb` = 1000, `kib` = 1024 — §3.1).
//! - `duration` lưu **nanosecond** (`ns us ms s m h d w`).
//! - `percent` lưu **tỉ lệ 0..1** (`50pct` = `0.5` — §3.6: "percent lưu tỉ lệ 0..1").
//! - `Show`: `size`/`duration` in theo **đơn vị lớn nhất chia hết** (`5000` ⇒ `5kb`,
//!   `250000000` ns ⇒ `250ms`), không chia hết thì về đơn vị gốc (`b`, `ns`); `percent` in
//!   `ratio*100` + `pct` (`0.5` ⇒ `50pct`).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use vis_types::hir::HBody;

/// Môi trường: **một** bảng `sym → giá trị` (ST-11 cấp `sym` duy nhất cho mọi khai báo nên
/// shadowing tự nhiên đúng); closure giữ `Rc` để bắt môi trường.
pub type Env = Rc<RefCell<HashMap<u32, Value>>>;

/// Hàm/closure: thân HIR + môi trường bắt được.
#[derive(Debug, Clone)]
pub struct Closure {
    pub name: String,
    pub params: Vec<u32>,
    pub body: HBody,
    pub env: Env,
}

/// Giá trị lúc chạy của VS0.
#[derive(Debug, Clone)]
pub enum Value {
    Unit,
    Int(i64),
    Float(f64),
    /// Byte.
    Size(i64),
    /// Nanosecond.
    Duration(i64),
    /// Tỉ lệ 0..1.
    Percent(f64),
    Str(String),
    Bool(bool),
    Bytes(Vec<u8>),
    Char(char),
    Path(String),
    List(Vec<Value>),
    /// Giữ **thứ tự chèn** để `keys()`/`Show` ổn định.
    Map(Vec<(Value, Value)>),
    /// Tên kiểu (rỗng nếu ẩn danh) + trường theo thứ tự khai.
    Record(String, Vec<(String, Value)>),
    /// Tên enum + tên biến thể + đối số.
    Enum(String, String, Vec<Value>),
    Opt(Option<Box<Value>>),
    Res(Result<Box<Value>, Box<Value>>),
    /// Nội bộ: `a..b` / `a..=b` (chỉ dùng cho `for`).
    Range(i64, i64, bool),
    /// Nội bộ: hàm dựng sẵn chưa gọi.
    Builtin(&'static str),
    Fn(Rc<Closure>),
    /// **Chỉ VM tạo** (ST-20): hàm bytecode + biến bắt (snapshot, VD-51). `interp` không bao giờ
    /// sinh biến thể này; `Show` in `<hàm>`.
    Vm(Rc<VmFunc>),
    /// **ST-60**: tác vụ `async` — chỉ `await`/join dùng; `Show` in `<tác vụ>` (không lộ handle).
    Task(Rc<crate::rt::Task>),
}

/// Hàm bytecode (ST-20): chỉ số hàm trong `Module` + biến bắt **snapshot**.
#[derive(Debug, Clone)]
pub struct VmFunc {
    pub idx: u32,
    pub captured: Vec<Value>,
}

impl Value {
    /// `true` nếu là `()` — tiện cho `each`.
    pub fn is_unit(&self) -> bool {
        matches!(self, Value::Unit)
    }

    /// `Show` — `str(x)` = `x.show()` (§3.5).
    pub fn show(&self) -> String {
        match self {
            Value::Unit => "()".into(),
            Value::Int(i) => i.to_string(),
            Value::Float(f) => show_float(*f),
            Value::Size(n) => show_size(*n),
            Value::Duration(n) => show_duration(*n),
            Value::Percent(r) => show_percent(*r),
            Value::Str(s) => s.clone(),
            Value::Bool(b) => b.to_string(),
            Value::Bytes(b) => show_bytes(b),
            Value::Char(c) => show_char(*c),
            Value::Path(p) => p.clone(),
            Value::List(xs) => {
                let body: Vec<String> = xs.iter().map(Value::show).collect();
                format!("[{}]", body.join(", "))
            }
            Value::Map(kv) => {
                let body: Vec<String> = kv
                    .iter()
                    .map(|(k, v)| format!("{}: {}", show_map_key(k), v.show()))
                    .collect();
                format!("{{{}}}", body.join(", "))
            }
            Value::Record(name, fields) => {
                let body: Vec<String> = fields
                    .iter()
                    .map(|(n, v)| format!("{n}: {}", v.show()))
                    .collect();
                if name.is_empty() {
                    format!("{{{}}}", body.join(", "))
                } else {
                    format!("{name}({})", body.join(", "))
                }
            }
            Value::Enum(_, variant, args) => {
                if args.is_empty() {
                    variant.clone()
                } else {
                    let body: Vec<String> = args.iter().map(Value::show).collect();
                    format!("{variant}({})", body.join(", "))
                }
            }
            Value::Opt(None) => "none".into(),
            Value::Opt(Some(v)) => format!("some({})", v.show()),
            Value::Res(Ok(v)) => format!("Ok({})", v.show()),
            Value::Res(Err(e)) => format!("Err({})", e.show()),
            Value::Range(a, b, incl) => {
                format!("{a}..{}{b}", if *incl { "=" } else { "" })
            }
            Value::Builtin(name) => format!("<builtin {name}>"),
            Value::Fn(c) => {
                if c.name.is_empty() {
                    "<fn>".into()
                } else {
                    format!("<fn {}>", c.name)
                }
            }
            Value::Vm(_) => "<hàm>".to_string(),
            Value::Task(_) => "<tác vụ>".to_string(),
        }
    }

    /// So khớp **cấu trúc** (dùng cho `==`/`!=` và pattern literal); `Percent` so với `Float`
    /// **theo tỉ lệ** (§3.6: `50pct == 0.5`).
    pub fn eq_value(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Percent(a), Value::Float(b)) | (Value::Float(b), Value::Percent(a)) => *a == *b,
            (Value::Int(a), Value::Float(b)) | (Value::Float(b), Value::Int(a)) => {
                (*a as f64) == *b
            }
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Size(a), Value::Size(b)) | (Value::Duration(a), Value::Duration(b)) => a == b,
            (Value::Percent(a), Value::Percent(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Path(a), Value::Path(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Char(a), Value::Char(b)) => a == b,
            (Value::Bytes(a), Value::Bytes(b)) => a == b,
            (Value::Unit, Value::Unit) => true,
            (Value::List(a), Value::List(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.eq_value(y))
            }
            (Value::Map(a), Value::Map(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .all(|(k, v)| b.iter().any(|(k2, v2)| k.eq_value(k2) && v.eq_value(v2)))
            }
            (Value::Record(_, a), Value::Record(_, b)) => {
                a.len() == b.len()
                    && a.iter()
                        .all(|(n, v)| b.iter().any(|(n2, v2)| n == n2 && v.eq_value(v2)))
            }
            (Value::Enum(_, va, aa), Value::Enum(_, vb, ab)) => {
                va == vb && aa.len() == ab.len() && aa.iter().zip(ab).all(|(x, y)| x.eq_value(y))
            }
            (Value::Opt(a), Value::Opt(b)) => match (a, b) {
                (None, None) => true,
                (Some(x), Some(y)) => x.eq_value(y),
                _ => false,
            },
            (Value::Res(a), Value::Res(b)) => match (a, b) {
                (Ok(x), Ok(y)) => x.eq_value(y),
                (Err(x), Err(y)) => x.eq_value(y),
                _ => false,
            },
            _ => false,
        }
    }

    /// `total_cmp` cho kiểu sắp được; `None` = không so được.
    pub fn cmp_value(&self, other: &Value) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => Some(a.cmp(b)),
            (Value::Size(a), Value::Size(b)) | (Value::Duration(a), Value::Duration(b)) => {
                Some(a.cmp(b))
            }
            (Value::Int(a), Value::Float(b)) => (*a as f64).partial_cmp(b),
            (Value::Float(a), Value::Int(b)) => a.partial_cmp(&(*b as f64)),
            (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
            (Value::Float(a), Value::Percent(b)) | (Value::Percent(b), Value::Float(a)) => {
                a.partial_cmp(b)
            }
            (Value::Percent(a), Value::Percent(b)) => a.partial_cmp(b),
            (Value::Str(a), Value::Str(b)) | (Value::Path(a), Value::Path(b)) => Some(a.cmp(b)),
            (Value::Char(a), Value::Char(b)) => Some(a.cmp(b)),
            (Value::Bytes(a), Value::Bytes(b)) => Some(a.cmp(b)),
            (Value::Bool(a), Value::Bool(b)) => Some(a.cmp(b)),
            _ => None,
        }
    }
}

fn show_map_key(k: &Value) -> String {
    match k {
        Value::Str(s) => format!("\"{s}\""),
        other => other.show(),
    }
}

/// `char` phải **escape** ký tự điều khiển (`'\n'` in ra `'\\n'`, không phải xuống dòng thật).
fn show_char(c: char) -> String {
    let mut s = String::from("'");
    match c {
        '\n' => s.push_str("\\n"),
        '\t' => s.push_str("\\t"),
        '\r' => s.push_str("\\r"),
        '\0' => s.push_str("\\0"),
        '\\' => s.push_str("\\\\"),
        '\'' => s.push_str("\\'"),
        other => s.push(other),
    }
    s.push('\'');
    s
}

/// `float`: dùng `Display` (Rust in `1` cho `1.0`, `1.5` cho `1.5`).
fn show_float(f: f64) -> String {
    if f.is_nan() {
        "NaN".into()
    } else if f.is_infinite() {
        if f > 0.0 {
            "inf".into()
        } else {
            "-inf".into()
        }
    } else {
        format!("{f}")
    }
}

const SIZE_UNITS: [(&str, i64); 8] = [
    ("tb", 1_000_000_000_000),
    ("tib", 1_099_511_627_776),
    ("gb", 1_000_000_000),
    ("gib", 1_073_741_824),
    ("mb", 1_000_000),
    ("mib", 1_048_576),
    ("kb", 1000),
    ("kib", 1024),
];

fn show_size(n: i64) -> String {
    if n == 0 {
        return "0b".into();
    }
    for (unit, factor) in SIZE_UNITS {
        if factor > 1 && n % factor == 0 {
            return format!("{}{unit}", n / factor);
        }
    }
    format!("{n}b")
}

const DUR_UNITS: [(&str, i64); 7] = [
    // ST-13b (mục 9): `w` phải có — `parse_duration` nhận `w` nên `Show` phải in ra `w`.
    ("w", 604_800_000_000_000),
    ("d", 86_400_000_000_000),
    ("h", 3_600_000_000_000),
    ("m", 60_000_000_000),
    ("s", 1_000_000_000),
    ("ms", 1_000_000),
    ("us", 1000),
];

fn show_duration(n: i64) -> String {
    if n == 0 {
        return "0ns".into();
    }
    for (unit, factor) in DUR_UNITS {
        if factor > 1 && n % factor == 0 {
            return format!("{}{unit}", n / factor);
        }
    }
    format!("{n}ns")
}

fn show_percent(r: f64) -> String {
    let p = r * 100.0;
    if (p - p.round()).abs() < 1e-9 {
        format!("{}pct", p.round() as i64)
    } else {
        format!("{p}pct")
    }
}

fn show_bytes(b: &[u8]) -> String {
    let mut s = String::from("b\"");
    for x in b {
        match x {
            b'\n' => s.push_str("\\n"),
            b'\t' => s.push_str("\\t"),
            b'"' => s.push_str("\\\""),
            b'\\' => s.push_str("\\\\"),
            0x20..=0x7e => s.push(*x as char),
            _ => s.push_str(&format!("\\x{x:02x}")),
        }
    }
    s.push('"');
    s
}

/// Đọc literal `size` (`5mb`, `1kib`) — trả byte.
pub fn parse_size(text: &str) -> Option<i64> {
    let t = text.trim();
    let digits: String = t
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '_')
        .collect();
    let unit = &t[digits.len()..];
    let n: i64 = digits.replace('_', "").parse().ok()?;
    let factor = match unit {
        "b" => 1,
        "kb" => 1000,
        "mb" => 1_000_000,
        "gb" => 1_000_000_000,
        "tb" => 1_000_000_000_000,
        "kib" => 1024,
        "mib" => 1_048_576,
        "gib" => 1_073_741_824,
        "tib" => 1_099_511_627_776,
        _ => return None,
    };
    n.checked_mul(factor)
}

/// Đọc literal `duration` (`250ms`, `2h30m`) — trả nanosecond.
pub fn parse_duration(text: &str) -> Option<i64> {
    let mut total: i64 = 0;
    let mut num = String::new();
    let mut unit = String::new();
    let mut chars = text.trim().chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_ascii_digit() || c == '_' {
            if !unit.is_empty() {
                total = total.checked_add(scale_duration(&num, &unit)?)?;
                num.clear();
                unit.clear();
            }
            num.push(c);
        } else {
            unit.push(c);
            if let Some(next) = chars.peek() {
                if next.is_ascii_digit() && !num.is_empty() && !unit.is_empty() {
                    // hết một cặp số+đơn vị (đơn vị không thể dài quá 2 ký tự)
                    if unit.len() >= 2 {
                        total = total.checked_add(scale_duration(&num, &unit)?)?;
                        num.clear();
                        unit.clear();
                    }
                }
            }
        }
    }
    if !num.is_empty() && !unit.is_empty() {
        total = total.checked_add(scale_duration(&num, &unit)?)?;
    }
    Some(total)
}

fn scale_duration(num: &str, unit: &str) -> Option<i64> {
    let n: i64 = num.replace('_', "").parse().ok()?;
    let factor = match unit {
        "ns" => 1,
        "us" => 1000,
        "ms" => 1_000_000,
        "s" => 1_000_000_000,
        "m" => 60_000_000_000,
        "h" => 3_600_000_000_000,
        "d" => 86_400_000_000_000,
        "w" => 604_800_000_000_000,
        _ => return None,
    };
    n.checked_mul(factor)
}

/// Đọc literal `percent` (`50pct`, `12.5pct`) — trả **tỉ lệ 0..1**.
pub fn parse_percent(text: &str) -> Option<f64> {
    let t = text.trim().trim_end_matches("pct");
    let v: f64 = t.parse().ok()?;
    Some(v / 100.0)
}

/// Bỏ quote + xử lý escape cho `"..."`, `"""..."""`, `r"..."`, `r"""..."""`.
pub fn parse_str_lit(text: &str, raw: bool) -> String {
    let inner = strip_quotes(text);
    if raw {
        return inner.to_string();
    }
    unescape(inner)
}

/// Bỏ quote + `\xNN` cho `b"..."` → từng byte ASCII.
pub fn parse_bytes_lit(text: &str) -> Option<Vec<u8>> {
    let inner = strip_quotes(text);
    let mut out = Vec::new();
    let mut it = inner.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            if !c.is_ascii() {
                return None;
            }
            out.push(c as u8);
            continue;
        }
        match it.next()? {
            'n' => out.push(b'\n'),
            't' => out.push(b'\t'),
            'r' => out.push(b'\r'),
            '0' => out.push(0),
            '\\' => out.push(b'\\'),
            '"' => out.push(b'"'),
            '\'' => out.push(b'\''),
            'x' => {
                let hi = it.next()?.to_digit(16)? as u8;
                let lo = it.next()?.to_digit(16)? as u8;
                out.push(hi * 16 + lo);
            }
            _ => return None,
        }
    }
    Some(out)
}

/// Bỏ quote + escape cho `'a'` → `char`.
pub fn parse_char_lit(text: &str) -> Option<char> {
    let inner = text.strip_prefix('\'')?.strip_suffix('\'')?;
    unescape(inner).chars().next()
}

fn strip_quotes(text: &str) -> &str {
    for q in ["\"\"\"", "\"", "'''", "'"] {
        if let Some(rest) = text.strip_prefix(q) {
            if let Some(inner) = rest.strip_suffix(q) {
                return inner;
            }
        }
    }
    text
}

fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('0') => out.push('\0'),
            Some('\\') => out.push('\\'),
            Some('"') => out.push('"'),
            Some('\'') => out.push('\''),
            Some('{') => out.push('{'),
            Some('}') => out.push('}'),
            Some('u') => {
                // `\u{XXXX}`
                let mut hex = String::new();
                for c2 in it.by_ref() {
                    match c2 {
                        '{' => {}
                        '}' => break,
                        h => hex.push(h),
                    }
                }
                if let Some(c2) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    out.push(c2);
                }
            }
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Vật phẩm của `for` trên **mọi** kiểu lặp được (list/range/str/map) — dùng **chung** cho interp
/// và VM (ST-20). Khoảng quá dài ⇒ `VS0501` (ST-13b: không cộng tràn `i64`).
pub fn iter_items(it: Value, span: vis_syntax::Span) -> crate::interp::R<Vec<Value>> {
    match it {
        Value::List(xs) => Ok(xs),
        Value::Range(a, b, incl) => {
            // ST-13b (mục 2): **không** cộng tràn — `..=` với `i64::MAX` từng panic.
            let mut out = Vec::new();
            let mut i = a;
            loop {
                let dung = if incl { i > b } else { i >= b };
                if dung {
                    break;
                }
                out.push(Value::Int(i));
                if out.len() > 10_000_000 {
                    return Err(crate::interp::Abort::Trap(crate::interp::Trap::new(
                        crate::interp::RT,
                        "khoảng quá dài (>10^7 phần tử)",
                        span,
                    )));
                }
                match i.checked_add(1) {
                    Some(n) => i = n,
                    None => break, // chạm `i64::MAX` ⇒ hết dải, **không** panic
                }
            }
            Ok(out)
        }
        // ST-13b (mục 4): duyệt chuỗi cho **`char`** (không phải `str`).
        Value::Str(s) => Ok(s.chars().map(Value::Char).collect()),
        Value::Map(kv) => Ok(kv
            .into_iter()
            .map(|(k, v)| Value::List(vec![k, v]))
            .collect()),
        other => Err(crate::interp::Abort::Trap(crate::interp::Trap::new(
            crate::interp::RT,
            format!("`for` không duyệt được `{}`", other.show()),
            span,
        ))),
    }
}

/// Mã thoát: `main` trả `int` ⇒ dùng số đó (cắt 8 bit); còn lại `0`.
pub fn exit_code(v: &Value) -> i32 {
    match v {
        Value::Int(i) => (*i & 0xff) as i32,
        _ => 0,
    }
}

/// `Show` dùng chung (đơn vị `size`/`duration`/`percent`, map/record…).
pub fn show(v: &Value) -> String {
    v.show()
}

/// Bằng nhau **theo ngữ nghĩa VS0** (không phải `PartialEq` của Rust).
pub fn eq(a: &Value, b: &Value) -> bool {
    a.eq_value(b)
}

/// So sánh thứ tự (chỉ cùng kiểu so được) — `None` nếu không so được.
pub fn cmp(a: &Value, b: &Value) -> Option<std::cmp::Ordering> {
    a.cmp_value(b)
}

#[cfg(test)]
mod tests_show_char {
    use super::show_char;

    #[test]
    fn show_char_escape_ky_tu_dieu_khien() {
        // ST-13b (mục 9): ký tự điều khiển phải **escape**, không in ra ký tự thật.
        assert_eq!(show_char('\n'), "'\\n'");
        assert_eq!(show_char('\t'), "'\\t'");
        assert_eq!(show_char('\''), "'\\''");
        assert_eq!(show_char('a'), "'a'");
    }
}
