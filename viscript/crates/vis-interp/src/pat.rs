// SPDX-License-Identifier: MIT OR Apache-2.0
//! Khớp **pattern** (ST-13b/ST-15b) — tách khỏi `interp.rs` cho gọn (R1 ≤ 1.500 dòng).

use crate::interp::{lit_value, Interp};
use crate::value::Value;
use vis_types::HPat;

impl<'a> Interp<'a> {
    pub(crate) fn bind_pat(&mut self, pat: &HPat, v: &Value) {
        let _ = self.bind_pat_bool(pat, v);
    }

    /// Khớp pattern và **ghi** binding; `false` ⇒ đã hoàn nguyên môi trường.
    pub(crate) fn bind_pat_bool(&mut self, pat: &HPat, v: &Value) -> bool {
        match pat {
            HPat::Wild { .. } | HPat::Rest { .. } => true,
            HPat::Bind { sym, .. } => {
                self.env.borrow_mut().insert(*sym, v.clone());
                true
            }
            HPat::Lit { lit, .. } => match lit_value(lit) {
                Ok(l) => l.eq_value(v),
                Err(_) => false,
            },
            HPat::Or { alts, .. } => {
                for a in alts {
                    let snap = self.env.borrow().clone();
                    if self.bind_pat_bool(a, v) {
                        return true;
                    }
                    *self.env.borrow_mut() = snap;
                }
                false
            }
            HPat::Ctor { path, args, .. } => {
                let name = path.last().cloned().unwrap_or_default();
                let low = name.to_ascii_lowercase();
                match v {
                    Value::Enum(_, variant, vargs) => {
                        if variant != &name && variant.to_ascii_lowercase() != low {
                            return false;
                        }
                        self.bind_all(args, vargs)
                    }
                    Value::Opt(Some(x)) => match low.as_str() {
                        "some" => self.bind_all(args, &[(**x).clone()]),
                        _ => false,
                    },
                    Value::Opt(None) => low == "none" && args.is_empty(),
                    Value::Res(Ok(x)) => match low.as_str() {
                        "ok" => self.bind_all(args, &[(**x).clone()]),
                        _ => false,
                    },
                    Value::Res(Err(e)) => match low.as_str() {
                        "err" => self.bind_all(args, &[(**e).clone()]),
                        _ => false,
                    },
                    Value::Bool(b) => match low.as_str() {
                        "true" => *b,
                        "false" => !*b,
                        _ => false,
                    },
                    // `T → T?` nâng **ngầm** ở mức kiểu (ST-12): giá trị chạy có thể là `T` trần,
                    // nên `some(v)` phải khớp cả giá trị **chưa bọc** (ST-14, ca `optional/match-some`).
                    other if low == "some" => {
                        if matches!(other, Value::Opt(None) | Value::Unit) {
                            false
                        } else {
                            self.bind_all(args, std::slice::from_ref(other))
                        }
                    }
                    _ => false,
                }
            }
            HPat::Tuple { items, .. } => match v {
                // ST-15: `()` là pattern của `unit` (không phải list rỗng).
                Value::Unit if items.is_empty() => true,
                Value::List(xs) => {
                    if xs.len() != items.len() {
                        return false;
                    }
                    let mut snap_ok = true;
                    for (p, x) in items.iter().zip(xs.iter()) {
                        if !self.bind_pat_bool(p, x) {
                            snap_ok = false;
                            break;
                        }
                    }
                    snap_ok
                }
                _ => false,
            },
            HPat::List { items, rest, .. } => match v {
                Value::List(xs) => {
                    if xs.len() < items.len() {
                        return false;
                    }
                    let head = &xs[..items.len()];
                    for (p, x) in items.iter().zip(head.iter()) {
                        if !self.bind_pat_bool(p, x) {
                            return false;
                        }
                    }
                    if let Some(r) = rest {
                        let tail = Value::List(xs[items.len()..].to_vec());
                        return self.bind_pat_bool(r, &tail);
                    }
                    xs.len() == items.len()
                }
                _ => false,
            },
            HPat::Record { fields, .. } => match v {
                Value::Record(_, vfields) => {
                    for (n, sub) in fields {
                        if n.is_empty() {
                            continue;
                        }
                        let Some((_, val)) = vfields.iter().find(|(f, _)| f == n) else {
                            return false;
                        };
                        if let Some(p) = sub {
                            if !self.bind_pat_bool(p, val) {
                                return false;
                            }
                        }
                    }
                    true
                }
                _ => false,
            },
            HPat::Range {
                lo, hi, inclusive, ..
            } => {
                let lo_v = match lo {
                    Some(p) => match self.lit_of_pat(p) {
                        Some(v) => v,
                        None => return false,
                    },
                    None => return false,
                };
                let hi_v = match hi {
                    Some(p) => match self.lit_of_pat(p) {
                        Some(v) => v,
                        None => return false,
                    },
                    None => return false,
                };
                let Some(a) = v.cmp_value(&lo_v) else {
                    return false;
                };
                let Some(b) = v.cmp_value(&hi_v) else {
                    return false;
                };
                let ok_lo = a != std::cmp::Ordering::Less;
                let ok_hi = if *inclusive {
                    b != std::cmp::Ordering::Greater
                } else {
                    b == std::cmp::Ordering::Less
                };
                ok_lo && ok_hi
            }
        }
    }

    fn lit_of_pat(&self, p: &HPat) -> Option<Value> {
        match p {
            HPat::Lit { lit, .. } => lit_value(lit).ok(),
            _ => None,
        }
    }

    pub(crate) fn bind_all(&mut self, pats: &[HPat], vals: &[Value]) -> bool {
        if pats.len() != vals.len() {
            return false;
        }
        for (p, v) in pats.iter().zip(vals.iter()) {
            if !self.bind_pat_bool(p, v) {
                return false;
            }
        }
        true
    }
}
