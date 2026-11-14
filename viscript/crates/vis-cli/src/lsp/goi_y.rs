// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-69 (VD-102 (5))** — `textDocument/completion` + `textDocument/hover`.
//!
//! Nguồn gợi ý (khử trùng theo `label`, ưu tiên **item > biến > builtin > từ khoá**, sắp theo **byte**
//! của label, lọc theo **tiền tố** tại con trỏ, cap **200**):
//! item cấp tệp (`fn` 3 / record 22 / enum 13 / alias 25 / const 21 / trait 11), biến-tham số (6),
//! `vis_types::BUILTIN_VALUE_NAMES` (3), `vis_syntax::KEYWORDS` (14).
//!
//! Hover: tra item → biến → builtin → từ khoá; không có ⇒ `result:null`. `range` là **đúng từ** tại
//! con trỏ (0-based **UTF-16**, tab = 1 — dùng lại `phan_tich::khoang`).

use std::collections::HashSet;

use vis_interp::value::Value;
use vis_syntax::{Span, KEYWORDS};
use vis_types::BUILTIN_VALUE_NAMES;

use super::jsonrpc::{m, phan_hoi, s};
use super::phan_tich::{GoiY, KetQua};
use super::vi_tri::khoang;

fn la_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Vị trí bắt đầu từ + **tiền tố** kết thúc tại `byte` (dùng cho completion).
pub fn tien_to(text: &str, byte: usize) -> (usize, String) {
    let b = byte.min(text.len());
    let mut start = b;
    for (i, c) in text[..b].char_indices().rev() {
        if la_ident(c) {
            start = i;
        } else {
            break;
        }
    }
    (start, text[start..b].to_string())
}

/// Biên **byte** của từ tại `byte` (rỗng nếu con trỏ không nằm trong/trên từ).
fn tu_tai(text: &str, byte: usize) -> (usize, usize) {
    let b = byte.min(text.len());
    let mut start = b;
    for (i, c) in text[..b].char_indices().rev() {
        if la_ident(c) {
            start = i;
        } else {
            break;
        }
    }
    let mut end = b;
    for (i, c) in text[b..].char_indices() {
        if la_ident(c) {
            end = b + i + c.len_utf8();
        } else {
            break;
        }
    }
    (start, end)
}

/// Tên biến **ngay trước dấu chấm** cuối cùng trước con trỏ (chỉ `a.` / `a.xy`, không biểu thức).
/// `None` ⇒ không phải truy cập thành viên đơn giản ⇒ dùng fallback toàn bộ.
fn nhan_sau_cham(text: &str, byte: usize) -> Option<String> {
    let b = byte.min(text.len());
    let before = &text[..b];
    let dot = before.rfind('.')?;
    let sau = &before[dot + 1..];
    if !sau.chars().all(la_ident) {
        return None;
    }
    if dot == 0 {
        return None;
    }
    let mut start = dot;
    for (i, c) in text[..dot].char_indices().rev() {
        if la_ident(c) {
            start = i;
        } else {
            break;
        }
    }
    if start == dot {
        return None;
    }
    Some(text[start..dot].to_string())
}

/// **ST-91**: gợi ý **thành viên** theo kiểu tĩnh của biến trước dấu chấm; `None` = không xác định
/// kiểu ⇒ fallback. Kiểu xác định nhưng **không** có trường/phương thức (vd `str`) ⇒ trả **rỗng**.
fn goi_y_thanh_vien(kq: &KetQua, byte: usize, tien_to: &str) -> Option<Vec<GoiY>> {
    let ten = nhan_sau_cham(&kq.text, byte)?;
    // `p.` một mình là cú pháp dở ⇒ HIR gốc có thể rỗng; phân tích lại với sentinel.
    let tv = super::thanh_vien::tu_van_ban_sua(&kq.text, byte);
    let kieu = tv
        .kieu_bien
        .iter()
        .rev()
        .find(|(n, _)| n == &ten)
        .map(|(_, k)| k.clone())?;
    let mut ds: Vec<GoiY> = tv
        .truong
        .iter()
        .filter(|(k, _)| k == &kieu)
        .map(|(_, g)| GoiY {
            label: g.label.clone(),
            kind: g.kind,
            detail: g.detail.clone(),
        })
        .chain(
            tv.phuong
                .iter()
                .filter(|(k, _)| k == &kieu)
                .map(|(_, g)| GoiY {
                    label: g.label.clone(),
                    kind: g.kind,
                    detail: g.detail.clone(),
                }),
        )
        .collect();
    ds.retain(|g| g.label.starts_with(tien_to));
    ds.sort_by(|a, b| a.label.as_bytes().cmp(b.label.as_bytes()));
    ds.truncate(200);
    Some(ds)
}

/// Danh sách gợi ý tại `byte` — đã khử trùng, lọc tiền tố, sắp theo byte, cap 200.
pub fn ds_goi_y(kq: &KetQua, byte: usize) -> Vec<GoiY> {
    let (_, tien_to) = tien_to(&kq.text, byte);
    if let Some(ds) = goi_y_thanh_vien(kq, byte, &tien_to) {
        return ds;
    }
    let mut ds: Vec<GoiY> = Vec::new();
    let mut da_co: HashSet<String> = HashSet::new();
    for it in &kq.items {
        if da_co.insert(it.label.clone()) {
            ds.push(GoiY {
                label: it.label.clone(),
                kind: it.kind,
                detail: it.detail.clone(),
            });
        }
    }
    for (ten, _ty) in &kq.bien {
        if da_co.insert(ten.clone()) {
            ds.push(GoiY {
                label: ten.clone(),
                kind: 6,
                detail: None,
            });
        }
    }
    for ten in BUILTIN_VALUE_NAMES {
        if da_co.insert(ten.to_string()) {
            ds.push(GoiY {
                label: ten.to_string(),
                kind: 3,
                detail: None,
            });
        }
    }
    for ten in KEYWORDS {
        if da_co.insert(ten.to_string()) {
            ds.push(GoiY {
                label: ten.to_string(),
                kind: 14,
                detail: None,
            });
        }
    }
    ds.retain(|g| g.label.starts_with(&tien_to));
    ds.sort_by(|a, b| a.label.as_bytes().cmp(b.label.as_bytes()));
    ds.truncate(200);
    ds
}

/// Giá trị hover (markdown) cho từ tại `byte` + **span** của từ; `None` ⇒ `result:null`.
fn gia_tri_hover(kq: &KetQua, byte: usize) -> Option<(Span, String)> {
    let (start, end) = tu_tai(&kq.text, byte);
    if start == end {
        return None;
    }
    let tu = &kq.text[start..end];
    let value = if let Some(it) = kq.items.iter().find(|g| g.label == tu) {
        let than = it.detail.clone().unwrap_or_else(|| it.label.clone());
        format!("```vis\n{than}\n```")
    } else if let Some((_ten, kieu)) = kq.bien.iter().find(|(ten, _)| ten == tu) {
        format!("```vis\n{tu}: {kieu}\n```")
    } else if BUILTIN_VALUE_NAMES.contains(&tu) {
        format!("```vis\n<builtin {tu}>\n```")
    } else if KEYWORDS.binary_search(&tu).is_ok() {
        format!("từ khoá: {tu}")
    } else {
        return None;
    };
    Some((Span::new(start, end), value))
}

/// Phản hồi `textDocument/completion` (result = mảng `CompletionItem`).
pub fn tra_loi_completion(kq: &KetQua, byte: usize, id: &Value) -> Vec<u8> {
    let items: Vec<Value> = ds_goi_y(kq, byte)
        .into_iter()
        .map(|g| {
            let mut kv = vec![("label", s(&g.label)), ("kind", Value::Int(g.kind as i64))];
            if let Some(d) = g.detail {
                kv.push(("detail", s(&d)));
            }
            m(kv)
        })
        .collect();
    phan_hoi(id, Value::List(items))
}

/// Phản hồi `textDocument/hover` (`contents` markdown + `range`), hoặc `result:null`.
pub fn tra_loi_hover(kq: &KetQua, byte: usize, id: &Value) -> Vec<u8> {
    let Some((span, value)) = gia_tri_hover(kq, byte) else {
        return phan_hoi(id, Value::Opt(None));
    };
    let ((l0, c0), (l1, c1)) = khoang(&kq.text, span);
    let pos = |line: u32, character: u32| {
        m(vec![
            ("line", Value::Int(line as i64)),
            ("character", Value::Int(character as i64)),
        ])
    };
    phan_hoi(
        id,
        m(vec![
            (
                "contents",
                m(vec![("kind", s("markdown")), ("value", s(&value))]),
            ),
            (
                "range",
                m(vec![("start", pos(l0, c0)), ("end", pos(l1, c1))]),
            ),
        ]),
    )
}
