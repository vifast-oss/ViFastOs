// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-56b (VD-97)**: gán **trường** / **chỉ số** — tách khỏi `interp.rs` cho luật **R1**.
//!
//! Ngữ nghĩa **đúng** như `Instr::With`/`Instr::Set` của VM (cùng thông điệp ⇒ interp ↔ `--vm`
//! **cùng byte**); thiếu trường / sai kiểu / ngoài phạm vi ⇒ `VS0501`.

use vis_syntax::Span;

use crate::interp::{Abort, Trap, R, RT};
use crate::value::eq;
use crate::value::Value;

/// **ST-56b (VD-97)**: `x.f = v` ⇒ `x = x with f: v` — **đúng** ngữ nghĩa `Instr::With` của VM
/// (cùng thông điệp để interp ↔ `--vm` **cùng byte**); thiếu trường/không phải bản ghi ⇒ `VS0501`.
pub(crate) fn gan_truong(base: Value, name: &str, v: Value, span: Span) -> R<Value> {
    match base {
        Value::Record(ten, mut fs) => match fs.iter_mut().find(|(k, _)| k == name) {
            Some(slot) => {
                slot.1 = v;
                Ok(Value::Record(ten, fs))
            }
            None => Err(Abort::Trap(Trap::new(
                RT,
                format!("bản ghi `{ten}` không có trường `{name}`"),
                span,
            ))),
        },
        other => Err(Abort::Trap(Trap::new(
            RT,
            format!("`with` không dùng được cho `{}`", other.show()),
            span,
        ))),
    }
}

/// **ST-56b (VD-97)**: `x[i] = v` — **đúng** ngữ nghĩa `Instr::Set` của VM (list: tại chỗ theo chỉ
/// số, ngoài phạm vi ⇒ `VS0501`; map: thay khoá **hoặc thêm** khoá mới).
pub(crate) fn gan_chi_so(base: Value, idx: Value, v: Value, span: Span) -> R<Value> {
    match (base, idx) {
        (Value::List(mut xs), Value::Int(i)) => {
            if (i as usize) < xs.len() {
                xs[i as usize] = v;
                Ok(Value::List(xs))
            } else {
                Err(Abort::Trap(Trap::new(
                    RT,
                    format!("chỉ số {i} ngoài phạm vi"),
                    span,
                )))
            }
        }
        (Value::Map(mut kv), k) => {
            if let Some(e) = kv.iter_mut().find(|(kk, _)| eq(kk, &k)) {
                e.1 = v;
            } else {
                kv.push((k, v));
            }
            Ok(Value::Map(kv))
        }
        _ => Err(Abort::Trap(Trap::new(RT, "gán chỉ số không hợp lệ", span))),
    }
}
