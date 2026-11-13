// SPDX-License-Identifier: MIT OR Apache-2.0
//! Đọc byte **VBC1** thành [`Module`] — **không** panic với dữ liệu hỏng.
//!
//! Mọi trường đều kiểm: magic, version, độ dài tường minh (không đọc quá `len`), tag lạ, chỉ số
//! const/hàm ngoài phạm vi, UTF-8 hỏng, đích nhảy ngoài `code`. Vi phạm ⇒ [`DecodeError`] **cụ thể**
//! (không `unwrap`).
//!
//! **Cache**: bytecode nạp từ cache (§14.5, VD-52 #3) **luôn** phải qua **verifier** (ST-21) sau
//! `decode` — hash nguồn chỉ là khoá cache, **không** phải bằng chứng an toàn.

use crate::format::{Const, Func, Instr, Module, Span, HASH_LEN, MAGIC, VERSION};
use vis_types::{BinOp, UnOp};

/// Lỗi giải mã — **9 biến thể** (module/const/func/code/span).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// Không đủ 4 byte magic hoặc magic khác `VBC1`.
    BadMagic([u8; 4]),
    /// Phiên bản khác [`VERSION`].
    BadVersion(u16),
    /// Hết byte giữa chừng (cắt cụt) — kèm vị trí.
    Truncated(usize),
    /// Độ dài khai báo vượt quá dữ liệu còn lại — kèm `(khai_báo, còn_lại)`.
    BadLength(usize, usize),
    /// Tag lệnh/hằng lạ.
    BadTag(u8),
    /// Chỉ số const ngoài phạm vi.
    BadConstIdx(u32),
    /// Chỉ số hàm ngoài phạm vi.
    BadFuncIdx(u32),
    /// Đích nhảy ngoài `code` của hàm.
    BadJump(i32),
    /// Chuỗi không phải UTF-8 hợp lệ.
    BadUtf8,
}

impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DecodeError::BadMagic(m) => write!(f, "magic sai: {m:?} (cần VBC1)"),
            DecodeError::BadVersion(v) => write!(f, "version {v} ≠ {VERSION}"),
            DecodeError::Truncated(at) => write!(f, "hết byte tại vị trí {at}"),
            DecodeError::BadLength(want, have) => write!(f, "độ dài {want} > còn lại {have}"),
            DecodeError::BadTag(t) => write!(f, "tag lạ: {t}"),
            DecodeError::BadConstIdx(i) => write!(f, "chỉ số const ngoài phạm vi: {i}"),
            DecodeError::BadFuncIdx(i) => write!(f, "chỉ số hàm ngoài phạm vi: {i}"),
            DecodeError::BadJump(d) => write!(f, "đích nhảy ngoài code: {d}"),
            DecodeError::BadUtf8 => write!(f, "chuỗi không phải UTF-8"),
        }
    }
}

struct Doc<'a> {
    b: &'a [u8],
    p: usize,
}

impl<'a> Doc<'a> {
    fn new(b: &'a [u8]) -> Self {
        Self { b, p: 0 }
    }

    fn con(&self, n: usize) -> Result<(), DecodeError> {
        if self.p + n > self.b.len() {
            return Err(DecodeError::Truncated(self.p));
        }
        Ok(())
    }

    fn u8(&mut self) -> Result<u8, DecodeError> {
        self.con(1)?;
        let v = self.b[self.p];
        self.p += 1;
        Ok(v)
    }

    fn u16(&mut self) -> Result<u16, DecodeError> {
        self.con(2)?;
        let v = u16::from_le_bytes([self.b[self.p], self.b[self.p + 1]]);
        self.p += 2;
        Ok(v)
    }

    fn u32(&mut self) -> Result<u32, DecodeError> {
        self.con(4)?;
        let v = u32::from_le_bytes([
            self.b[self.p],
            self.b[self.p + 1],
            self.b[self.p + 2],
            self.b[self.p + 3],
        ]);
        self.p += 4;
        Ok(v)
    }

    fn i32(&mut self) -> Result<i32, DecodeError> {
        Ok(self.u32()? as i32)
    }

    fn bytes(&mut self) -> Result<Vec<u8>, DecodeError> {
        let n = self.u32()? as usize;
        if n > self.b.len() - self.p {
            return Err(DecodeError::BadLength(n, self.b.len() - self.p));
        }
        let v = self.b[self.p..self.p + n].to_vec();
        self.p += n;
        Ok(v)
    }

    fn string(&mut self) -> Result<String, DecodeError> {
        String::from_utf8(self.bytes()?).map_err(|_| DecodeError::BadUtf8)
    }
}

/// Mã hoá ngược của [`crate::encode::encode_instr`].
fn decode_instr(d: &mut Doc<'_>, n_code: u32) -> Result<Instr, DecodeError> {
    let _ = n_code;
    let tag = d.u8()?;
    Ok(match tag {
        0 => Instr::Const(d.u32()?),
        1 => Instr::LoadLocal(d.u16()?),
        2 => Instr::StoreLocal(d.u16()?),
        3 => Instr::Assign(d.u16()?),
        4 => Instr::LoadCaptured(d.u16()?),
        5 => Instr::Pop,
        6 => {
            let t = d.u8()?;
            Instr::Bin(crate::format::binop_from_tag(t).ok_or(DecodeError::BadTag(t))?)
        }
        7 => {
            let t = d.u8()?;
            Instr::Un(crate::format::unop_from_tag(t).ok_or(DecodeError::BadTag(t))?)
        }
        8 => Instr::Call(d.u32()?, d.u16()?),
        9 => Instr::CallBuiltin(d.u32()?, d.u16()?),
        10 => Instr::CallMethod(d.u32()?, d.u16()?),
        11 => Instr::Jump(d.i32()?),
        12 => Instr::JumpIfFalse(d.i32()?),
        13 => Instr::JumpIfNotTrue(d.i32()?),
        14 => Instr::JumpIfNone(d.i32()?),
        15 => Instr::JumpIfSome(d.i32()?),
        16 => Instr::Return,
        17 => Instr::Halt,
        18 => Instr::MakeList(d.u16()?),
        19 => Instr::MakeSet(d.u16()?),
        20 => Instr::MakeTuple(d.u16()?),
        21 => Instr::MakeMap(d.u16()?),
        22 => Instr::MakeRecord(d.u32()?, d.u16()?),
        23 => Instr::MakeEnum(d.u32()?, d.u16()?),
        24 => Instr::MakeSome,
        25 => Instr::MakeNone,
        26 => Instr::MakeOk,
        27 => Instr::MakeErr,
        28 => Instr::MakeClosure(d.u32()?, d.u16()?),
        29 => Instr::Field(d.u32()?),
        30 => Instr::Index,
        31 => Instr::Set,
        32 => Instr::With(d.u16()?),
        33 => Instr::Interp(d.u16()?),
        34 => Instr::IterInit,
        35 => Instr::IterNext(d.i32()?),
        36 => Instr::TestLit(d.u32()?),
        37 => Instr::TestCtor(d.u32()?, d.u16()?),
        38 => Instr::TestTuple(d.u16()?),
        39 => Instr::TestList(d.u16()?),
        40 => Instr::TestRecord(d.u32()?, d.u16()?, d.u8()?),
        41 => Instr::TestRange(d.u32()?, d.u32()?, d.u8()?),
        42 => Instr::BindLocal(d.u16()?),
        43 => Instr::Try,
        44 => Instr::Dup,
        45 => Instr::Unwrap(d.u16()?),
        46 => Instr::CallValue(d.u16()?),
        47 => Instr::UnwrapOpt,
        48 => Instr::IsUnit,
        49 => Instr::BoolOr,
        50 => Instr::BoolAnd,
        51 => Instr::Swap,
        52 => Instr::PushUnit,
        t => return Err(DecodeError::BadTag(t)),
    })
}

fn decode_func(d: &mut Doc<'_>) -> Result<Func, DecodeError> {
    let name = d.string()?;
    let arity = d.u16()?;
    let nparams = d.u16()?;
    let nlocals = d.u16()?;
    let max_stack = d.u16()?;
    let n_code = d.u32()?;
    let mut code = Vec::with_capacity(n_code.min(1 << 20) as usize);
    for _ in 0..n_code {
        code.push(decode_instr(d, n_code)?);
    }
    let n_spans = d.u32()?;
    let mut spans = Vec::with_capacity(n_spans.min(1 << 20) as usize);
    for _ in 0..n_spans {
        spans.push(Span {
            start: d.u32()?,
            end: d.u32()?,
        });
    }
    // Đích nhảy phải nằm trong `code` (kiểm sớm; verifier ST-21 kiểm đầy đủ hơn).
    // Đích = `chỉ_số_lệnh + offset` (cùng quy ước với VM/codegen), phải thuộc `[0, len]` —
    // offset **âm** là hợp lệ (nhảy lùi).
    let len = code.len() as i64;
    for (pc, ins) in code.iter().enumerate() {
        let d_ = match ins {
            Instr::Jump(x)
            | Instr::JumpIfFalse(x)
            | Instr::JumpIfNotTrue(x)
            | Instr::JumpIfNone(x)
            | Instr::JumpIfSome(x)
            | Instr::IterNext(x) => Some(*x as i64),
            _ => None,
        };
        if let Some(off) = d_ {
            let dich = pc as i64 + off;
            if dich < 0 || dich > len {
                return Err(DecodeError::BadJump(off as i32));
            }
        }
    }
    Ok(Func {
        name,
        arity,
        nparams,
        nlocals,
        max_stack,
        code,
        spans,
    })
}

/// Đọc module; mọi vi phạm ⇒ [`DecodeError`], **không** panic.
pub fn decode(b: &[u8]) -> Result<Module, DecodeError> {
    let mut d = Doc::new(b);
    let mut magic = [0u8; 4];
    for (i, slot) in magic.iter_mut().enumerate() {
        *slot = *b.get(i).ok_or(DecodeError::Truncated(i))?;
    }
    if magic != MAGIC {
        return Err(DecodeError::BadMagic(magic));
    }
    d.p = 4;
    let version = d.u16()?;
    if version != VERSION {
        return Err(DecodeError::BadVersion(version));
    }
    d.con(HASH_LEN)?;
    let mut source_hash = [0u8; HASH_LEN];
    source_hash.copy_from_slice(&b[d.p..d.p + HASH_LEN]);
    d.p += HASH_LEN;
    let n_const = d.u32()?;
    if n_const as usize > b.len() {
        return Err(DecodeError::BadLength(n_const as usize, b.len()));
    }
    let mut consts = Vec::with_capacity(n_const as usize);
    for _ in 0..n_const {
        let kind = d.u8()?;
        if kind > 9 {
            return Err(DecodeError::BadTag(kind));
        }
        consts.push(Const {
            kind,
            text: d.string()?,
        });
    }
    let n_func = d.u32()?;
    if n_func as usize > b.len() {
        return Err(DecodeError::BadLength(n_func as usize, b.len()));
    }
    let mut funcs = Vec::with_capacity(n_func as usize);
    for _ in 0..n_func {
        funcs.push(decode_func(&mut d)?);
    }
    let entry = d.u32()?;
    if entry as usize >= funcs.len() {
        return Err(DecodeError::BadFuncIdx(entry));
    }
    // Kiểm chỉ số const/hàm trong từng lệnh.
    for f in &funcs {
        for ins in &f.code {
            match ins {
                Instr::Const(i)
                | Instr::TestLit(i)
                | Instr::Field(i)
                | Instr::CallBuiltin(i, _) => {
                    if *i as usize >= consts.len() {
                        return Err(DecodeError::BadConstIdx(*i));
                    }
                }
                Instr::CallMethod(i, _)
                | Instr::MakeRecord(i, _)
                | Instr::MakeEnum(i, _)
                | Instr::TestRecord(i, _, _)
                | Instr::TestCtor(i, _) => {
                    if *i as usize >= consts.len() {
                        return Err(DecodeError::BadConstIdx(*i));
                    }
                }
                Instr::TestRange(a, bb, _) => {
                    if *a as usize >= consts.len() || *bb as usize >= consts.len() {
                        return Err(DecodeError::BadConstIdx(*a));
                    }
                }
                Instr::Call(fi, _) | Instr::MakeClosure(fi, _) if *fi as usize >= funcs.len() => {
                    return Err(DecodeError::BadFuncIdx(*fi));
                }
                _ => {}
            }
        }
    }
    Ok(Module {
        version,
        source_hash,
        consts,
        funcs,
        entry,
    })
}

/// Tag của `BinOp` — tiện cho test âm (không dùng `format::binop_tag` trực tiếp).
pub fn bin_tag(op: BinOp) -> u8 {
    crate::format::binop_tag(op)
}

/// Tag của `UnOp`.
pub fn un_tag(op: UnOp) -> u8 {
    crate::format::unop_tag(op)
}
