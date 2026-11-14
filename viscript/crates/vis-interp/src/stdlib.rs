// SPDX-License-Identifier: MIT OR Apache-2.0
//! **`std.*` lõi** (ST-26): ngữ nghĩa dùng **chung** cho interp và VM (VD-55) — một hiện thực duy nhất.
//!
//! Tên đầy đủ có tiền tố module (`str.split`, `list.sort`, `time.now_ms`) — `tyck` kiểm chữ ký theo
//! [`vis_types::stdlib`], tầng này kiểm **kiểu lúc chạy** (sai ⇒ `VS0501` nêu hàm + kiểu nhận được).
//!
//! Mọi hàm **thuần** trừ `std.time.*` (đọc đồng hồ trong Rust, **không** thêm method vào [`Host`]);
//! `std.fs` **không** ở đây (ST-27), `std.json.decode` **chưa** có (VS1b — `VS0301` ở tầng hạ).

use std::sync::OnceLock;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use vis_diag::{COMMAND_CODES, RUNTIME_CODES};
use vis_syntax::Span;
use vis_types::stdlib::{sig_of, Ty};

use crate::builtin::Host;
use crate::interp::{Abort, Trap, R};
use crate::value::{eq, Value};

/// `Err(String)` của host ⇒ `Result` của VS0 (`Err(msg)`), **không** phải trap.
/// **ST-72b (VD-103/VD-106)**: dấu **cưỡng chế quyền** trong `Err` của host — `hop_nhat_fs` đổi
/// thành **trap `VS0604`** (không bắt được), thay vì `Err(str)` dữ liệu (vượt quyền là lỗi **môi
/// trường**, không phải dữ liệu chương trình).
pub const DAU_QUYEN: &str = "\u{1}VS0604:";

fn hop_nhat_fs(r: Result<Value, String>, span: Span) -> Result<Value, Abort> {
    match r {
        Ok(v) => Ok(Value::Res(Ok(Box::new(v)))),
        Err(e) => match e.strip_prefix(DAU_QUYEN) {
            Some(msg) => Err(Abort::Trap(Trap::new(
                &COMMAND_CODES[3],
                msg.to_string(),
                span,
            ))),
            None => Ok(Value::Res(Err(Box::new(Value::Str(e))))),
        },
    }
}

/// Không có host ⇒ `VS0604` (cùng câu với chương trình ngoài, VD-60 (2)).
fn khong_host(name: &str, span: Span) -> Abort {
    Abort::Trap(Trap::new(
        &COMMAND_CODES[3],
        format!("`std.{name}` chưa hỗ trợ ở môi trường này (không có host)"),
        span,
    ))
}

fn loi(msg: impl Into<String>, span: Span) -> Abort {
    Abort::Trap(Trap::new(&RUNTIME_CODES[0], msg, span))
}

/// Kiểm số đối số + kiểu theo bảng chữ ký; trả `Err(Abort)` nếu sai.
fn kiem(name: &str, args: &[Value], span: Span) -> Result<(), Abort> {
    let Some(sig) = sig_of(name) else {
        return Err(loi(
            format!("hàm `std.{name}` không có trong bảng chữ ký"),
            span,
        ));
    };
    if args.len() != sig.so_doi() {
        return Err(loi(
            format!("`{name}` cần {} đối số, được {}", sig.so_doi(), args.len()),
            span,
        ));
    }
    for (i, (a, t)) in args.iter().zip(sig.params.iter()).enumerate() {
        let ok = match t {
            // **ST-58 (VD-94/VD-99)**: biến kiểu / `list[T]` / `map[K,V]` — thân hàm tự kiểm như
            // trước với `Any` (tyck đã chặn **tĩnh** phần lớn; dữ liệu không kiểu vẫn tới đây).
            Ty::Var(_) | Ty::List(_) | Ty::Map(..) => true,
            Ty::Int => matches!(a, Value::Int(_)),
            Ty::Str => matches!(a, Value::Str(_)),
            Ty::Bool => matches!(a, Value::Bool(_)),
            Ty::ListStr => {
                matches!(a, Value::List(xs) if xs.iter().all(|v| matches!(v, Value::Str(_))))
            }
            Ty::FnKhoa => matches!(a, Value::Fn(_) | Value::Vm(_) | Value::Builtin(_)),
            // ST-33: `std.fs` trả `Result[..]` — kiểu **trả**, không dùng để kiểm đối số.
            Ty::ResultStrStr | Ty::ResultUnitStr | Ty::ResultListStrStr => true,
        };
        if !ok {
            return Err(loi(
                format!(
                    "`{name}`: đối số {} cần `{}`, được `{}`",
                    i + 1,
                    t.ten(),
                    a.show()
                ),
                span,
            ));
        }
    }
    Ok(())
}

fn chuoi<'a>(name: &str, v: &'a Value, i: usize, span: Span) -> Result<&'a str, Abort> {
    match v {
        Value::Str(s) => Ok(s),
        other => Err(loi(
            format!(
                "`{name}`: đối số {} cần `str`, được `{}`",
                i + 1,
                other.show()
            ),
            span,
        )),
    }
}

fn so(name: &str, v: &Value, i: usize, span: Span) -> Result<i64, Abort> {
    match v {
        Value::Int(n) => Ok(*n),
        other => Err(loi(
            format!(
                "`{name}`: đối số {} cần `int`, được `{}`",
                i + 1,
                other.show()
            ),
            span,
        )),
    }
}

fn danh_sach<'a>(name: &str, v: &'a Value, i: usize, span: Span) -> Result<&'a [Value], Abort> {
    match v {
        Value::List(xs) => Ok(xs),
        other => Err(loi(
            format!(
                "`{name}`: đối số {} cần `list`, được `{}`",
                i + 1,
                other.show()
            ),
            span,
        )),
    }
}

fn ban_do<'a>(
    name: &str,
    v: &'a Value,
    i: usize,
    span: Span,
) -> Result<&'a [(Value, Value)], Abort> {
    match v {
        Value::Map(kv) => Ok(kv),
        other => Err(loi(
            format!(
                "`{name}`: đối số {} cần `map`, được `{}`",
                i + 1,
                other.show()
            ),
            span,
        )),
    }
}

/// Gọi một hàm `std.*` theo tên đầy đủ.
pub fn call(host: &mut dyn Host, name: &str, args: Vec<Value>, span: Span) -> R<Value> {
    kiem(name, &args, span)?;
    match name {
        // ----- std.str -----
        "str.split" => {
            let s = chuoi(name, &args[0], 0, span)?;
            let sep = chuoi(name, &args[1], 1, span)?;
            let ds: Vec<Value> = if sep.is_empty() {
                // Tách theo **ký tự** (Unicode scalar) khi dấu phân cách rỗng.
                s.chars().map(|c| Value::Str(c.to_string())).collect()
            } else {
                s.split(sep).map(|x| Value::Str(x.to_string())).collect()
            };
            Ok(Value::List(ds))
        }
        "str.join" => {
            let xs = danh_sach(name, &args[0], 0, span)?;
            let sep = chuoi(name, &args[1], 1, span)?;
            let mut phan: Vec<String> = Vec::with_capacity(xs.len());
            for (i, v) in xs.iter().enumerate() {
                match v {
                    Value::Str(s) => phan.push(s.clone()),
                    other => {
                        return Err(loi(
                            format!("`{name}`: phần tử {} cần `str`, được `{}`", i, other.show()),
                            span,
                        ))
                    }
                }
            }
            Ok(Value::Str(phan.join(sep)))
        }
        "str.trim" | "str.upper" | "str.lower" => {
            let s = chuoi(name, &args[0], 0, span)?;
            Ok(Value::Str(match name {
                "str.trim" => s.trim().to_string(),
                "str.upper" => s.to_uppercase(),
                _ => s.to_lowercase(),
            }))
        }
        "str.starts_with" | "str.ends_with" | "str.contains" => {
            let s = chuoi(name, &args[0], 0, span)?;
            let p = chuoi(name, &args[1], 1, span)?;
            let ok = match name {
                "str.starts_with" => s.starts_with(p),
                "str.ends_with" => s.ends_with(p),
                _ => s.contains(p),
            };
            Ok(Value::Bool(ok))
        }
        "str.repeat" => {
            let s = chuoi(name, &args[0], 0, span)?;
            let n = so(name, &args[1], 1, span)?;
            if n < 0 {
                return Err(loi(
                    format!("`{name}`: số lần lặp phải ≥ 0, được {n}"),
                    span,
                ));
            }
            // Cap an toàn: 10^7 ký tự (như cap range) — tránh cấp phát khổng lồ.
            let tong = s.len().saturating_mul(n as usize);
            if tong > 10_000_000 {
                return Err(loi(
                    format!("`{name}`: kết quả quá lớn ({tong} byte > 10^7)"),
                    span,
                ));
            }
            Ok(Value::Str(s.repeat(n as usize)))
        }
        "str.replace" => {
            let s = chuoi(name, &args[0], 0, span)?;
            let tu = chuoi(name, &args[1], 1, span)?;
            let den = chuoi(name, &args[2], 2, span)?;
            Ok(Value::Str(s.replace(tu, den)))
        }
        // ----- std.list -----
        "list.sort" => {
            let xs = danh_sach(name, &args[0], 0, span)?.to_vec();
            let mut ds = xs.clone();
            ds.sort_by(|a, b| a.cmp_value(b).unwrap_or(std::cmp::Ordering::Equal));
            Ok(Value::List(ds))
        }
        // ST-53: **cùng lõi** `sap_xep_theo_khoa` với shell `sort_by`; ST-56: khoá nhận **`int`
        // hoặc `str`** (một kiểu trong cả lần gọi) — ca `int` cũ **không đổi**.
        "list.sort_by" => {
            let xs = danh_sach(name, &args[0], 0, span)?.to_vec();
            crate::builtin::sap_xep_theo_khoa(
                host,
                &xs,
                &args[1],
                false,
                crate::builtin::KieuKhoa::ChiIntHoacStr,
                name,
                span,
            )
        }
        "list.reverse" => {
            let mut xs = danh_sach(name, &args[0], 0, span)?.to_vec();
            xs.reverse();
            Ok(Value::List(xs))
        }
        "list.unique" => {
            let xs = danh_sach(name, &args[0], 0, span)?;
            let mut out: Vec<Value> = Vec::new();
            for x in xs {
                if !out.iter().any(|y| eq(y, x)) {
                    out.push(x.clone());
                }
            }
            Ok(Value::List(out))
        }
        "list.concat" => {
            let a = danh_sach(name, &args[0], 0, span)?.to_vec();
            let b = danh_sach(name, &args[1], 1, span)?;
            let mut out = a;
            out.extend_from_slice(b);
            Ok(Value::List(out))
        }
        "list.chunk" => {
            let xs = danh_sach(name, &args[0], 0, span)?;
            let n = so(name, &args[1], 1, span)?;
            if n <= 0 {
                return Err(loi(
                    format!("`{name}`: kích thước khối phải ≥ 1, được {n}"),
                    span,
                ));
            }
            let n = n as usize;
            Ok(Value::List(
                xs.chunks(n).map(|c| Value::List(c.to_vec())).collect(),
            ))
        }
        "list.take" | "list.drop" => {
            let xs = danh_sach(name, &args[0], 0, span)?;
            let n = so(name, &args[1], 1, span)?;
            let n = n.max(0) as usize;
            let out: Vec<Value> = if name == "list.take" {
                xs.iter().take(n).cloned().collect()
            } else {
                xs.iter().skip(n).cloned().collect()
            };
            Ok(Value::List(out))
        }
        // ----- std.map -----
        "map.keys" | "map.values" => {
            let kv = ban_do(name, &args[0], 0, span)?;
            let out: Vec<Value> = if name == "map.keys" {
                kv.iter().map(|(k, _)| k.clone()).collect()
            } else {
                kv.iter().map(|(_, v)| v.clone()).collect()
            };
            Ok(Value::List(out))
        }
        "map.merge" => {
            let a = ban_do(name, &args[0], 0, span)?.to_vec();
            let b = ban_do(name, &args[1], 1, span)?;
            let mut out = a;
            for (k, v) in b {
                match out.iter_mut().find(|(k0, _)| eq(k0, k)) {
                    Some(slot) => slot.1 = v.clone(),
                    None => out.push((k.clone(), v.clone())),
                }
            }
            Ok(Value::Map(out))
        }
        "map.get_or" => {
            let kv = ban_do(name, &args[0], 0, span)?;
            let k = chuoi(name, &args[1], 1, span)?;
            let mac_dinh = args[2].clone();
            let kvk = Value::Str(k.to_string());
            Ok(kv
                .iter()
                .find(|(k0, _)| eq(k0, &kvk))
                .map(|(_, v)| v.clone())
                .unwrap_or(mac_dinh))
        }
        "map.remove" => {
            let kv = ban_do(name, &args[0], 0, span)?;
            let k = chuoi(name, &args[1], 1, span)?;
            let kvk = Value::Str(k.to_string());
            Ok(Value::Map(
                kv.iter().filter(|(k0, _)| !eq(k0, &kvk)).cloned().collect(),
            ))
        }
        // ----- std.path (thuần chuỗi, không chạm đĩa) -----
        "path.join" => {
            let a = chuoi(name, &args[0], 0, span)?;
            let b = chuoi(name, &args[1], 1, span)?;
            Ok(Value::Str(noi_duong(a, b)))
        }
        "path.basename" | "path.dirname" | "path.ext" | "path.stem" => {
            let p = chuoi(name, &args[0], 0, span)?;
            Ok(Value::Str(match name {
                "path.basename" => basename(p),
                "path.dirname" => dirname(p),
                "path.ext" => ext(p),
                _ => stem(p),
            }))
        }
        "path.is_abs" => {
            let p = chuoi(name, &args[0], 0, span)?;
            Ok(Value::Bool(p.starts_with('/')))
        }
        // ----- std.time (đọc đồng hồ trong Rust; **không** qua Host) -----
        "time.now_ms" => {
            let ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            Ok(Value::Int(ms))
        }
        "time.monotonic_ms" => {
            static GOC: OnceLock<Instant> = OnceLock::new();
            let goc = GOC.get_or_init(Instant::now);
            Ok(Value::Int(goc.elapsed().as_millis() as i64))
        }
        // ----- std.json (encode + decode — ST-33) -----
        "json.decode" => {
            let s = chuoi(name, &args[0], 0, span)?;
            match crate::json::decode(s) {
                Ok(v) => Ok(Value::Res(Ok(Box::new(v)))),
                Err(e) => Ok(Value::Res(Err(Box::new(Value::Str(e))))),
            }
        }
        "json.encode" => {
            let mut s = String::new();
            json(&args[0], false, &mut s);
            Ok(Value::Str(s))
        }
        "json.encode_pretty" => {
            let mut s = String::new();
            json(&args[0], true, &mut s);
            Ok(Value::Str(s))
        }
        // ----- std.fs (ST-33) — **chỉ qua `Host`**, không OS trong crate này -----
        "fs.read" => {
            let p = chuoi(name, &args[0], 0, span)?;
            let host = host.fs().ok_or_else(|| khong_host(name, span))?;
            hop_nhat_fs(host.read(p).map(Value::Str), span)
        }
        "fs.write" => {
            let p = chuoi(name, &args[0], 0, span)?;
            let v = chuoi(name, &args[1], 1, span)?;
            let host = host.fs().ok_or_else(|| khong_host(name, span))?;
            hop_nhat_fs(host.write(p, v).map(|()| Value::Unit), span)
        }
        "fs.exists" => {
            let p = chuoi(name, &args[0], 0, span)?;
            let host = host.fs().ok_or_else(|| khong_host(name, span))?;
            Ok(Value::Bool(host.exists(p)))
        }
        "fs.list" => {
            let p = chuoi(name, &args[0], 0, span)?;
            let host = host.fs().ok_or_else(|| khong_host(name, span))?;
            hop_nhat_fs(
                host.list(p)
                    .map(|xs| Value::List(xs.into_iter().map(Value::Str).collect())),
                span,
            )
        }
        // ----- std.net (ST-63, VD-100/VD-105) — **chỉ qua `Host::net`**, không OS trong crate này.
        // Thiếu host ⇒ `VS0604`; lỗi mạng ⇒ `Err(str)` (dữ liệu, **không** trap). -----
        "net.connect" | "net.connect_timeout" => {
            let dia_chi = chuoi(name, &args[0], 0, span)?;
            let timeout = if args.len() == 2 {
                so(name, &args[1], 1, span)?
            } else {
                // Mặc định 1 s — đủ rộng cho loopback, không treo chương trình.
                1_000
            };
            let host = host.net().ok_or_else(|| khong_host(name, span))?;
            let r = host.connect(dia_chi, timeout.max(0) as u64);
            hop_nhat_fs(r.map(Value::Str), span)
        }
        "net.read" => {
            let tay = chuoi(name, &args[0], 0, span)?;
            let toi_da = so(name, &args[1], 1, span)?;
            let timeout = so(name, &args[2], 2, span)?;
            let host = host.net().ok_or_else(|| khong_host(name, span))?;
            let r = host.read(tay, toi_da.max(0) as usize, timeout.max(0) as u64);
            // Body là **byte**; ngôn ngữ chỉ có `str` ⇒ giải mã **lossy** (không cắt giữa ký tự).
            // `Ok(vec![])` = EOF ⇒ `""` (không phải lỗi). Nợ: API byte thật cho tới khi có `buf`.
            hop_nhat_fs(
                r.map(|b| Value::Str(String::from_utf8_lossy(&b).into_owned())),
                span,
            )
        }
        "net.write" => {
            let tay = chuoi(name, &args[0], 0, span)?;
            let du = chuoi(name, &args[1], 1, span)?;
            let host = host.net().ok_or_else(|| khong_host(name, span))?;
            hop_nhat_fs(host.write(tay, du.as_bytes()).map(|()| Value::Unit), span)
        }
        "net.close" => {
            let tay = chuoi(name, &args[0], 0, span)?;
            let host = host.net().ok_or_else(|| khong_host(name, span))?;
            hop_nhat_fs(host.close(tay).map(|()| Value::Unit), span)
        }
        // **ST-65 (VD-100/VD-105)**: server TCP — `listen`/`local_addr`/`accept(_timeout)`/
        // `close_listen`; host giữ listener thật; thiếu host ⇒ `VS0604`; `accept` không timeout = chặn.
        "net.listen" => {
            let dia_chi = chuoi(name, &args[0], 0, span)?;
            let host = host.net().ok_or_else(|| khong_host(name, span))?;
            hop_nhat_fs(host.listen(dia_chi).map(Value::Str), span)
        }
        "net.local_addr" => {
            let tay = chuoi(name, &args[0], 0, span)?;
            let host = host.net().ok_or_else(|| khong_host(name, span))?;
            hop_nhat_fs(host.local_addr(tay).map(Value::Str), span)
        }
        "net.accept" | "net.accept_timeout" => {
            let tay = chuoi(name, &args[0], 0, span)?;
            let timeout = if args.len() == 2 {
                so(name, &args[1], 1, span)?
            } else {
                0
            };
            let host = host.net().ok_or_else(|| khong_host(name, span))?;
            hop_nhat_fs(
                host.accept(tay, timeout.max(0) as u64).map(Value::Str),
                span,
            )
        }
        "net.close_listen" => {
            let tay = chuoi(name, &args[0], 0, span)?;
            let host = host.net().ok_or_else(|| khong_host(name, span))?;
            hop_nhat_fs(host.close(tay).map(|()| Value::Unit), span)
        }
        // **ST-64 (VD-100/VD-105)**: giao thức HTTP **trong crate này** (`net.rs`), socket qua host.
        // Lỗi HTTP/mạng là `Err(str)` (dữ liệu); thiếu host ⇒ `VS0604`. Mặc định 5000 ms.
        "http.get" | "http.get_timeout" => {
            let url = chuoi(name, &args[0], 0, span)?;
            let timeout = if args.len() == 2 {
                so(name, &args[1], 1, span)?
            } else {
                5_000
            };
            let host = host.net().ok_or_else(|| khong_host(name, span))?;
            hop_nhat_fs(
                crate::net::http_get(host, url, timeout.max(0) as u64).map(Value::Str),
                span,
            )
        }
        other => Err(loi(
            format!("`std.{other}` chưa cài (bảng chữ ký có nhưng tầng chạy thiếu)"),
            span,
        )),
    }
}

/// Mã hoá JSON **compact** (hoặc pretty 2 dấu cách) cho giá trị VS0.
///
/// Quy ước: `map` theo **thứ tự chèn**; bản ghi theo thứ tự trường; `none` ⇒ `null`; `Result` ⇒
/// `{"ok":…}`/`{"err":…}`; enum ⇒ `{"variant":…, "value":…}`. Khoá chuỗi được escape.
pub fn json(v: &Value, pretty: bool, out: &mut String) {
    json_tai(v, pretty, 0, out);
}

fn thut(depth: usize, out: &mut String) {
    for _ in 0..depth {
        out.push_str("  ");
    }
}

fn mo(pretty: bool, depth: usize, out: &mut String) {
    if pretty {
        out.push('\n');
        thut(depth + 1, out);
    }
}

fn dong(pretty: bool, depth: usize, out: &mut String) {
    if pretty {
        out.push('\n');
        thut(depth, out);
    }
}

fn json_tai(v: &Value, pretty: bool, depth: usize, out: &mut String) {
    fn esc(s: &str, out: &mut String) {
        out.push('"');
        for c in s.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
                c => out.push(c),
            }
        }
        out.push('"');
    }
    match v {
        Value::Unit => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Int(n) => out.push_str(&n.to_string()),
        Value::Float(f) => {
            if f.is_finite() {
                out.push_str(&format!("{f}"))
            } else {
                out.push_str("null")
            }
        }
        Value::Size(n) | Value::Duration(n) => out.push_str(&n.to_string()),
        Value::Percent(p) => out.push_str(&format!("{p}")),
        Value::Str(s) | Value::Path(s) => esc(s, out),
        Value::Char(c) => esc(&c.to_string(), out),
        Value::Bytes(b) => esc(&String::from_utf8_lossy(b), out),
        Value::List(xs) => {
            out.push('[');
            for (i, x) in xs.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                mo(pretty, depth, out);
                json_tai(x, pretty, depth + 1, out);
            }
            if !xs.is_empty() {
                dong(pretty, depth, out);
            }
            out.push(']');
        }
        Value::Map(kv) => {
            out.push('{');
            for (i, (k, val)) in kv.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                mo(pretty, depth, out);
                let kk = match k {
                    Value::Str(s) | Value::Path(s) => s.clone(),
                    other => other.show(),
                };
                esc(&kk, out);
                out.push_str(if pretty { ": " } else { ":" });
                json_tai(val, pretty, depth + 1, out);
            }
            if !kv.is_empty() {
                dong(pretty, depth, out);
            }
            out.push('}');
        }
        Value::Record(_, fs) => {
            out.push('{');
            for (i, (f, val)) in fs.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                mo(pretty, depth, out);
                esc(f, out);
                out.push_str(if pretty { ": " } else { ":" });
                json_tai(val, pretty, depth + 1, out);
            }
            if !fs.is_empty() {
                dong(pretty, depth, out);
            }
            out.push('}');
        }
        Value::Enum(_, var, args) => {
            out.push('{');
            mo(pretty, depth, out);
            esc("variant", out);
            out.push_str(if pretty { ": " } else { ":" });
            esc(var, out);
            out.push(',');
            mo(pretty, depth, out);
            esc("value", out);
            out.push_str(if pretty { ": " } else { ":" });
            match args.len() {
                0 => out.push_str("null"),
                1 => json_tai(&args[0], pretty, depth + 1, out),
                _ => {
                    out.push('[');
                    for (i, a) in args.iter().enumerate() {
                        if i > 0 {
                            out.push(',');
                        }
                        mo(pretty, depth + 1, out);
                        json_tai(a, pretty, depth + 2, out);
                    }
                    dong(pretty, depth + 1, out);
                    out.push(']');
                }
            }
            dong(pretty, depth, out);
            out.push('}');
        }
        Value::Opt(None) => out.push_str("null"),
        Value::Opt(Some(x)) => json_tai(x, pretty, depth, out),
        Value::Res(Ok(x)) => {
            out.push('{');
            mo(pretty, depth, out);
            esc("ok", out);
            out.push_str(if pretty { ": " } else { ":" });
            json_tai(x, pretty, depth + 1, out);
            dong(pretty, depth, out);
            out.push('}');
        }
        Value::Res(Err(e)) => {
            out.push('{');
            mo(pretty, depth, out);
            esc("err", out);
            out.push_str(if pretty { ": " } else { ":" });
            json_tai(e, pretty, depth + 1, out);
            dong(pretty, depth, out);
            out.push('}');
        }
        Value::Range(a, b, _) => {
            out.push('{');
            mo(pretty, depth, out);
            esc("range", out);
            out.push_str(if pretty { ": " } else { ":" });
            out.push('[');
            out.push_str(&a.to_string());
            out.push(',');
            out.push_str(&b.to_string());
            out.push(']');
            dong(pretty, depth, out);
            out.push('}');
        }
        // Hàm/closure không mã hoá được ⇒ `null` (không panic).
        _ => out.push_str("null"),
    }
}

fn noi_duong(a: &str, b: &str) -> String {
    if b.starts_with('/') || a.is_empty() {
        return b.to_string();
    }
    let a = a.trim_end_matches('/');
    format!("{a}/{b}")
}

fn basename(p: &str) -> String {
    let p = p.trim_end_matches('/');
    match p.rsplit_once('/') {
        Some((_, ten)) => ten.to_string(),
        None => p.to_string(),
    }
}

fn dirname(p: &str) -> String {
    let p2 = p.trim_end_matches('/');
    match p2.rsplit_once('/') {
        Some((d, _)) => {
            if d.is_empty() {
                "/".to_string()
            } else {
                d.to_string()
            }
        }
        None => ".".to_string(),
    }
}

fn ext(p: &str) -> String {
    let ten = basename(p);
    match ten.rsplit_once('.') {
        Some((_, e)) if !e.is_empty() && !ten.starts_with('.') => e.to_string(),
        _ => String::new(),
    }
}

fn stem(p: &str) -> String {
    let ten = basename(p);
    match ten.rsplit_once('.') {
        Some((s, e)) if !e.is_empty() && !ten.starts_with('.') => s.to_string(),
        _ => ten,
    }
}
