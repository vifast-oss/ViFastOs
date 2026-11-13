// SPDX-License-Identifier: MIT OR Apache-2.0
//! **`Render`** (ST-41, §9.6) — hiển thị **giá trị cuối pipeline** của chế độ lệnh:
//! `list[bản ghi]` ⇒ **bảng** (căn trái cho `str`, căn phải cho số/`size`/`duration`/`percent`);
//! bản ghi **đơn** ⇒ **khối khoá–giá trị**; `list` rỗng ⇒ `(rỗng)`; còn lại ⇒ `None` (dùng `Show` cũ).
//!
//! Độ rộng cột đếm **số `char` sau NFC** (emoji/CJK/NFC tiếng Việt = 1), **không** đếm byte.

use crate::value::Value;

/// Độ rộng hiển thị: số `char` sau **NFC** (chốt ST-41).
pub fn rong(s: &str) -> usize {
    use unicode_normalization::UnicodeNormalization;
    s.nfc().collect::<String>().chars().count()
}

fn dem(s: &str, n: usize) -> String {
    let d = rong(s);
    let pad = n.saturating_sub(d);
    format!("{s}{}", " ".repeat(pad))
}

fn trai(s: &str, n: usize) -> String {
    let d = rong(s);
    let pad = n.saturating_sub(d);
    format!("{}{s}", " ".repeat(pad))
}

fn so(v: &Value) -> bool {
    matches!(
        v,
        Value::Int(_) | Value::Float(_) | Value::Size(_) | Value::Duration(_) | Value::Percent(_)
    )
}

/// Bảng cho `list[bản ghi]`: cột = trường của **bản ghi đầu**, giữ thứ tự; 2 dấu cách giữa cột.
fn bang(items: &[Value]) -> String {
    let mut cot: Vec<String> = Vec::new();
    for it in items {
        if let Value::Record(_, fields) = it {
            for (k, _) in fields {
                if !cot.contains(k) {
                    cot.push(k.clone());
                }
            }
        }
    }
    let mut hang: Vec<Vec<String>> = Vec::new();
    for it in items {
        let mut r: Vec<String> = Vec::new();
        for c in &cot {
            let s = match it {
                Value::Record(_, fields) => fields
                    .iter()
                    .find(|(k, _)| k == c)
                    .map(|(_, v)| v.show())
                    .unwrap_or_default(),
                other => other.show(),
            };
            r.push(s);
        }
        hang.push(r);
    }
    let mut phai: Vec<bool> = Vec::new();
    for (j, _) in cot.iter().enumerate() {
        let mut p = true;
        let mut co_so = false;
        for it in items {
            if let Value::Record(_, fields) = it {
                if let Some((_, v)) = fields.iter().find(|(k, _)| k == &cot[j]) {
                    if so(v) {
                        co_so = true;
                    } else {
                        p = false;
                    }
                }
            }
        }
        phai.push(co_so && p);
    }
    let mut rong_cot: Vec<usize> = cot.iter().map(|c| rong(c)).collect();
    for r in &hang {
        for (j, c) in r.iter().enumerate() {
            rong_cot[j] = rong_cot[j].max(rong(c));
        }
    }
    let mut out = String::new();
    let mut dong: Vec<String> = Vec::new();
    for (j, c) in cot.iter().enumerate() {
        dong.push(if phai[j] {
            trai(c, rong_cot[j])
        } else {
            dem(c, rong_cot[j])
        });
    }
    out.push_str(dong.join("  ").trim_end());
    out.push('\n');
    for r in &hang {
        let mut d: Vec<String> = Vec::new();
        for (j, c) in r.iter().enumerate() {
            d.push(if phai[j] {
                trai(c, rong_cot[j])
            } else {
                dem(c, rong_cot[j])
            });
        }
        out.push_str(d.join("  ").trim_end());
        out.push('\n');
    }
    out
}

/// Khối `khoá  giá trị` cho **một** bản ghi (căn khoá theo độ rộng lớn nhất).
fn khoi(fields: &[(String, Value)]) -> String {
    let nhan = fields.iter().map(|(k, _)| rong(k)).max().unwrap_or(0);
    let mut out = String::new();
    for (k, v) in fields {
        out.push_str(&dem(k, nhan));
        out.push_str("  ");
        out.push_str(&v.show());
        out.push('\n');
    }
    out
}

/// `Some(chuỗi)` nếu giá trị cuối pipeline cần `Render`; `None` ⇒ dùng `Show` cũ.
///
/// `co_ban_ghi` = kiểu **tĩnh** của phần tử là **bản ghi** (do bộ hạ truyền qua đối số thứ hai) —
/// `(rỗng)` **chỉ** cho `list[bản ghi]` rỗng; `list[str]`/`list[int]` rỗng vẫn `[]` (chốt Agent 0).
pub fn in_gia_tri(v: &Value, co_ban_ghi: bool) -> Option<String> {
    match v {
        Value::Record(_, fields) if !fields.is_empty() => Some(khoi(fields)),
        Value::List(items) if items.is_empty() && co_ban_ghi => Some("(rỗng)\n".to_string()),
        Value::List(items)
            if !items.is_empty() && items.iter().all(|x| matches!(x, Value::Record(_, _))) =>
        {
            Some(bang(items))
        }
        _ => None,
    }
}
