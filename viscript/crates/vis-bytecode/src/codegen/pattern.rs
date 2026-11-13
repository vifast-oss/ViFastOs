// SPDX-License-Identifier: MIT OR Apache-2.0
//! Pattern của `match`/`let` (ST-19/ST-20b): test (kể cả **lồng**), bind, kiểm lồng.
//!
//! Là **module con** của `codegen` để dùng được `Emit` + trường riêng mà không phải nới visibility.

use super::*;

impl Emit<'_> {
    /// Hằng của một đầu mút khoảng (`None` ⇒ biên mở, dùng `0`).
    pub(super) fn cst_pat(&mut self, p: Option<&HPat>) -> u32 {
        match p {
            Some(HPat::Lit { lit, .. }) => self.g.cst(lit.kind, &lit.text),
            _ => self.g.cst(LitKind::Int, "0"),
        }
    }

    /// Phát **test** cho pattern: 1→1 (bool). Trả **vị trí lệnh nhảy** chưa vá (nếu có) — với
    /// `Or`/`Ctor` phức tạp, test dùng lệnh `Test*` tuần tự.
    pub(super) fn test(&mut self, p: &HPat, span: Span) -> Result<Option<usize>, CodegenError> {
        // Bất biến: `test` **tiêu thụ** giá trị đỉnh và để lại **một** `bool`.
        // Pattern lồng: test con trước (mỗi con dùng một bản `Dup`), test ngoài sau, rồi gộp
        // bằng `BoolAnd` (giao hoán nên thứ tự gộp không quan trọng).
        match p {
            HPat::Wild { .. } | HPat::Bind { .. } => {
                let c = self.g.cst(LitKind::Bool, "true");
                self.f.push(Instr::Const(c), span, 1);
                Ok(None)
            }
            HPat::Lit { lit, .. } => {
                let c = self.g.cst(lit.kind, &lit.text);
                self.f.push(Instr::TestLit(c), span, 0);
                Ok(None)
            }
            HPat::Ctor { path, args, .. } => {
                let mut dem = 0usize;
                for (i, sub) in args.iter().enumerate() {
                    if !can_test(sub) {
                        continue;
                    }
                    self.f.push(Instr::Dup, span, 1);
                    self.f.push(Instr::Unwrap(i as u16), span, 0);
                    let _ = self.test(sub, span)?;
                    dem += 1;
                }
                let ten = path.last().cloned().unwrap_or_default();
                let n = self.g.cst_str(&ten);
                self.f.push(Instr::TestCtor(n, args.len() as u16), span, 0);
                for _ in 0..dem {
                    self.f.push(Instr::BoolAnd, span, -1);
                }
                Ok(None)
            }
            HPat::Tuple { items, .. } | HPat::List { items, .. } => {
                let mut dem = 0usize;
                for (i, sub) in items.iter().enumerate() {
                    if !can_test(sub) {
                        continue;
                    }
                    self.f.push(Instr::Dup, span, 1);
                    let c = self.g.cst(LitKind::Int, &i.to_string());
                    self.f.push(Instr::Const(c), span, 1);
                    self.f.push(Instr::Index, span, -1);
                    let _ = self.test(sub, span)?;
                    dem += 1;
                }
                let n = items.len() as u16;
                if matches!(p, HPat::Tuple { .. }) {
                    self.f.push(Instr::TestTuple(n), span, 0);
                } else {
                    self.f.push(Instr::TestList(n), span, 0);
                }
                for _ in 0..dem {
                    self.f.push(Instr::BoolAnd, span, -1);
                }
                Ok(None)
            }
            HPat::Record { fields, open, .. } => {
                let mut dem = 0usize;
                for (name, sub) in fields {
                    let Some(sub) = sub else { continue };
                    if !can_test(sub) {
                        continue;
                    }
                    self.f.push(Instr::Dup, span, 1);
                    let c = self.g.cst_str(name);
                    self.f.push(Instr::Field(c), span, 0);
                    let _ = self.test(sub, span)?;
                    dem += 1;
                }
                let t = self.g.cst_str("");
                self.f.push(
                    Instr::TestRecord(t, fields.len() as u16, u8::from(*open)),
                    span,
                    0,
                );
                for _ in 0..dem {
                    self.f.push(Instr::BoolAnd, span, -1);
                }
                Ok(None)
            }
            HPat::Range {
                lo, hi, inclusive, ..
            } => {
                let a = self.cst_pat(lo.as_deref());
                let b = self.cst_pat(hi.as_deref());
                self.f
                    .push(Instr::TestRange(a, b, u8::from(*inclusive)), span, 0);
                Ok(None)
            }
            HPat::Or { alts, .. } => {
                // Giá trị gốc ở đỉnh; mỗi nhánh test trên **bản sao**, cất `bool` vào ô tạm.
                let mut o: Vec<u16> = Vec::new();
                for alt in alts {
                    self.f.push(Instr::Dup, span, 1);
                    let _ = self.test(alt, span)?;
                    let slot = self.f.nlocals;
                    self.f.nlocals += 1;
                    self.f.push(Instr::StoreLocal(slot), span, -1);
                    o.push(slot);
                }
                for (i, slot) in o.iter().enumerate() {
                    self.f.push(Instr::LoadLocal(*slot), span, 1);
                    if i > 0 {
                        self.f.push(Instr::BoolOr, span, -1);
                    }
                }
                // `[giá trị, bool]` ⇒ `[bool]` (tiêu thụ giá trị cho đúng hợp đồng).
                self.f.push(Instr::Swap, span, 0);
                self.f.push(Instr::Pop, span, -1);
                Ok(None)
            }
            HPat::Rest { .. } => Err(CodegenError::Unsupported("rest-pattern", span)),
        }
    }

    /// **Kiểm lồng**: giả định giá trị đỉnh, **tiêu thụ** nó, nhảy tới `fails` nếu không khớp.
    /// Dùng cho pattern con (`some(some(x))`, `Ok((a, b))`, `A(1)`…).
    pub(super) fn kiem_mot(
        &mut self,
        p: &HPat,
        span: Span,
        fails: &mut Vec<usize>,
    ) -> Result<(), CodegenError> {
        match p {
            HPat::Wild { .. } | HPat::Bind { .. } => {
                self.f.push(Instr::Pop, span, -1);
                Ok(())
            }
            HPat::Lit { .. } | HPat::Range { .. } => {
                let _ = self.test(p, span)?;
                let at = self.f.here();
                self.f.push(Instr::JumpIfFalse(0), span, -1);
                fails.push(at);
                Ok(())
            }
            HPat::Ctor { path, args, .. } => {
                let ten = path.last().cloned().unwrap_or_default();
                let n = self.g.cst_str(&ten);
                self.f.push(Instr::Dup, span, 1);
                self.f.push(Instr::TestCtor(n, args.len() as u16), span, 0);
                let at = self.f.here();
                self.f.push(Instr::JumpIfFalse(0), span, -1);
                fails.push(at);
                for (i, sub) in args.iter().enumerate() {
                    self.f.push(Instr::Dup, span, 1);
                    self.f.push(Instr::Unwrap(i as u16), span, 0);
                    self.kiem_mot(sub, span, fails)?;
                }
                self.f.push(Instr::Pop, span, -1);
                Ok(())
            }
            HPat::Tuple { items, .. } | HPat::List { items, .. } => {
                self.f.push(Instr::Dup, span, 1);
                self.f.push(Instr::TestTuple(items.len() as u16), span, 0);
                let at = self.f.here();
                self.f.push(Instr::JumpIfFalse(0), span, -1);
                fails.push(at);
                for (i, sub) in items.iter().enumerate() {
                    self.f.push(Instr::Dup, span, 1);
                    let c = self.g.cst(LitKind::Int, &i.to_string());
                    self.f.push(Instr::Const(c), span, 1);
                    self.f.push(Instr::Index, span, -1);
                    self.kiem_mot(sub, span, fails)?;
                }
                self.f.push(Instr::Pop, span, -1);
                Ok(())
            }
            HPat::Record { .. } | HPat::Or { .. } | HPat::Rest { .. } => {
                self.f.push(Instr::Pop, span, -1);
                Ok(())
            }
        }
    }

    /// Gán pattern từ **đỉnh** ngăn xếp. `pop = true` ⇒ **tiêu thụ** giá trị; `pop = false` ⇒
    /// giữ nguyên giá trị đỉnh (dùng khi cần giữ subject của `match`).
    pub(super) fn bind(&mut self, p: &HPat, span: Span, pop: bool) -> Result<(), CodegenError> {
        if !pop {
            self.f.push(Instr::Dup, span, 1);
        }
        self.gan_tu_dinh(p, span)
    }

    /// Gán biến của pattern và **tiêu thụ** giá trị đỉnh (mọi nhánh đều kết thúc bằng `Pop` hoặc
    /// `StoreLocal`).
    pub(super) fn gan_tu_dinh(&mut self, p: &HPat, span: Span) -> Result<(), CodegenError> {
        match p {
            HPat::Wild { .. } | HPat::Lit { .. } | HPat::Range { .. } => {
                self.f.push(Instr::Pop, span, -1);
                Ok(())
            }
            HPat::Bind { sym, .. } => {
                let slot = self.slot_cua(*sym)?;
                self.f.push(Instr::StoreLocal(slot), span, -1);
                Ok(())
            }
            HPat::Tuple { items, .. } | HPat::List { items, .. } => {
                for (i, it) in items.iter().enumerate() {
                    self.f.push(Instr::Dup, span, 1);
                    let c = self.g.cst(LitKind::Int, &i.to_string());
                    self.f.push(Instr::Const(c), span, 1);
                    self.f.push(Instr::Index, span, -1);
                    self.gan_tu_dinh(it, span)?;
                }
                self.f.push(Instr::Pop, span, -1);
                Ok(())
            }
            HPat::Ctor { args, .. } => {
                for (i, it) in args.iter().enumerate() {
                    self.f.push(Instr::Dup, span, 1);
                    self.f.push(Instr::Unwrap(i as u16), span, 0);
                    self.gan_tu_dinh(it, span)?;
                }
                self.f.push(Instr::Pop, span, -1);
                Ok(())
            }
            HPat::Record { fields, .. } => {
                for (name, sub) in fields {
                    let c = self.g.cst_str(name);
                    self.f.push(Instr::Dup, span, 1);
                    self.f.push(Instr::Field(c), span, 0);
                    match sub {
                        Some(it) => self.gan_tu_dinh(it, span)?,
                        None => {
                            self.f.push(Instr::Pop, span, -1);
                        }
                    }
                }
                self.f.push(Instr::Pop, span, -1);
                Ok(())
            }
            HPat::Or { alts, .. } => {
                if let Some(first) = alts.first() {
                    self.gan_tu_dinh(first, span)?;
                } else {
                    self.f.push(Instr::Pop, span, -1);
                }
                Ok(())
            }
            HPat::Rest { .. } => Err(CodegenError::Unsupported("rest-pattern", span)),
        }
    }
}
