// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ghi [`Module`] thành byte **VBC1** — little-endian, mọi độ dài tường minh.

use crate::format::{binop_tag, unop_tag, Func, Instr, Module, Span, HASH_LEN, MAGIC, VERSION};

fn u16b(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}
fn u32b(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}
fn i32b(out: &mut Vec<u8>, v: i32) {
    out.extend_from_slice(&v.to_le_bytes());
}
fn bytes(out: &mut Vec<u8>, s: &[u8]) {
    u32b(out, s.len() as u32);
    out.extend_from_slice(s);
}

/// Mã hoá **một** lệnh (tag + toán hạng) — cũng là đơn vị test của `decode`.
pub fn encode_instr(out: &mut Vec<u8>, i: Instr) {
    out.push(i.tag());
    match i {
        Instr::Const(x) | Instr::TestLit(x) | Instr::Field(x) => u32b(out, x),
        Instr::CallBuiltin(n, k) => {
            u32b(out, n);
            u16b(out, k);
        }
        Instr::CallMethod(x, n) => {
            u32b(out, x);
            u16b(out, n);
        }
        Instr::Call(f, n) | Instr::MakeClosure(f, n) | Instr::MakeEnum(f, n) => {
            u32b(out, f);
            u16b(out, n);
        }
        Instr::Jump(d)
        | Instr::JumpIfFalse(d)
        | Instr::JumpIfNotTrue(d)
        | Instr::JumpIfNone(d)
        | Instr::JumpIfSome(d)
        | Instr::IterNext(d) => i32b(out, d),
        Instr::LoadLocal(x)
        | Instr::StoreLocal(x)
        | Instr::Assign(x)
        | Instr::LoadCaptured(x)
        | Instr::MakeList(x)
        | Instr::MakeSet(x)
        | Instr::MakeTuple(x)
        | Instr::MakeMap(x)
        | Instr::With(x)
        | Instr::Interp(x)
        | Instr::TestTuple(x)
        | Instr::TestList(x)
        | Instr::BindLocal(x) => u16b(out, x),
        Instr::MakeRecord(t, n) | Instr::TestCtor(t, n) => {
            u32b(out, t);
            u16b(out, n);
        }
        Instr::TestRecord(t, n, o) => {
            u32b(out, t);
            u16b(out, n);
            out.push(o);
        }
        Instr::TestRange(a, b, f) => {
            u32b(out, a);
            u32b(out, b);
            out.push(f);
        }
        Instr::Bin(op) => out.push(binop_tag(op)),
        Instr::Un(op) => out.push(unop_tag(op)),
        Instr::Pop
        | Instr::Return
        | Instr::Halt
        | Instr::MakeSome
        | Instr::MakeNone
        | Instr::MakeOk
        | Instr::MakeErr
        | Instr::Index
        | Instr::Set
        | Instr::IterInit
        | Instr::Try
        | Instr::Dup => {}
        Instr::Unwrap(i) => u16b(out, i),
        Instr::CallValue(n) => u16b(out, n),
        Instr::UnwrapOpt => {}
        Instr::IsUnit | Instr::BoolOr | Instr::BoolAnd | Instr::Swap | Instr::PushUnit => {}
    }
}

fn encode_func(out: &mut Vec<u8>, f: &Func) {
    bytes(out, f.name.as_bytes());
    u16b(out, f.arity);
    u16b(out, f.nparams);
    u16b(out, f.nlocals);
    u16b(out, f.max_stack);
    u32b(out, f.code.len() as u32);
    for i in &f.code {
        encode_instr(out, *i);
    }
    u32b(out, f.spans.len() as u32);
    for s in &f.spans {
        u32b(out, s.start);
        u32b(out, s.end);
    }
}

/// Mã hoá module — kết quả **tất định** (cùng `Module` ⇒ cùng bytes).
pub fn encode(m: &Module) -> Vec<u8> {
    let mut out = Vec::with_capacity(64 + m.total_code() * 4);
    out.extend_from_slice(&MAGIC);
    u16b(&mut out, m.version.max(VERSION));
    let h: [u8; HASH_LEN] = m.source_hash;
    out.extend_from_slice(&h);
    u32b(&mut out, m.consts.len() as u32);
    for c in &m.consts {
        out.push(c.kind);
        bytes(&mut out, c.text.as_bytes());
    }
    u32b(&mut out, m.funcs.len() as u32);
    for f in &m.funcs {
        encode_func(&mut out, f);
    }
    u32b(&mut out, m.entry);
    out
}

/// Span rỗng — tiện cho test.
pub fn span(start: u32, end: u32) -> Span {
    Span { start, end }
}
