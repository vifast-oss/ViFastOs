// SPDX-License-Identifier: MIT OR Apache-2.0
//! Hạ **HIR** (`vis-types`) xuống [`Module`] của VBC1 (ST-19).
//!
//! Nguyên tắc:
//! - **Không** chép ngữ nghĩa toán tử: `Bin`/`Un` mang **chính** `vis_types::BinOp`/`UnOp` — VM
//!   (ST-20) sẽ dùng chung bảng ngữ nghĩa với `vis-interp`.
//! - `match` hạ thành **chuỗi test + nhảy** (VM không cần biết pattern).
//! - Closure **chụp snapshot** cả khung hiện tại (VD-51): `MakeClosure(fidx, nfree)` với
//!   `nfree = số ô cục bộ của hàm cha tại điểm tạo`; trong closure đọc bằng `LoadCaptured`.
//! - `max_stack`: đếm **bảo thủ** trong lúc phát (upper bound ≥ độ sâu thật). **ST-21 kiểm lại
//!   độc lập từ byte stream** — không tin giá trị này.

use std::collections::HashMap;

use vis_syntax::Span;
use vis_types::{HBody, HCond, HExpr, HItem, HPat, HStmt, Hir, InterpPart, LitKind, Symbols};

use crate::format::{binop_tag, unop_tag, Const, Func, Instr, Module, HASH_LEN};

/// Lỗi codegen — **không** panic; mỗi lỗi nêu biến thể HIR + span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodegenError {
    /// Biến thể VS1 chưa biểu diễn được (kèm tên + span).
    Unsupported(&'static str, Span),
    /// Quá nhiều ô cục bộ / đối số cho `u16`.
    TooMany(u16),
    /// Module không có hàm `main`.
    NoEntry,
}

impl core::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CodegenError::Unsupported(k, s) => {
                write!(
                    f,
                    "chưa biểu diễn được `{k}` tại byte {}..{}",
                    s.start, s.end
                )
            }
            CodegenError::TooMany(n) => write!(f, "quá nhiều ô cục bộ/đối số: {n}"),
            CodegenError::NoEntry => write!(f, "không có hàm `main`"),
        }
    }
}

/// Hàm đang phát.
struct F {
    name: String,
    arity: u16,
    nparams: u16,
    code: Vec<Instr>,
    spans: Vec<crate::format::Span>,
    /// `sym` → ô cục bộ.
    locals: HashMap<u32, u16>,
    nlocals: u16,
    cur: i32,
    max: i32,
}

impl F {
    fn new(name: String) -> Self {
        Self {
            name,
            arity: 0,
            nparams: 0,
            code: Vec::new(),
            spans: Vec::new(),
            locals: HashMap::new(),
            nlocals: 0,
            cur: 0,
            max: 0,
        }
    }

    fn slot(&mut self, sym: u32, span: Span) -> Result<u16, CodegenError> {
        if let Some(s) = self.locals.get(&sym) {
            return Ok(*s);
        }
        let s = self.nlocals;
        if s == u16::MAX {
            return Err(CodegenError::TooMany(s));
        }
        self.nlocals += 1;
        self.locals.insert(sym, s);
        let _ = span;
        Ok(s)
    }

    fn push(&mut self, ins: Instr, span: Span, dang: i32) {
        self.code.push(ins);
        self.spans.push(crate::format::Span {
            start: span.start as u32,
            end: span.end as u32,
        });
        // Kẹp tại 0: đây là **ước lượng trên** cho `max_stack`, không phải kiểm kiểu ngăn xếp —
        // verifier ST-21 mới kiểm độ sâu thật từ byte stream (và có quyền từ chối module).
        self.cur = (self.cur + dang).max(0);
        if self.cur > self.max {
            self.max = self.cur;
        }
    }

    /// Vị trí lệnh kế (để tính đích nhảy tương đối).
    fn here(&self) -> usize {
        self.code.len()
    }

    fn patch(&mut self, at: usize, target: usize) {
        let off = target as i64 - at as i64;
        let off = off as i32;
        if let Some(ins) = self.code.get_mut(at) {
            *ins = match *ins {
                Instr::Jump(_) => Instr::Jump(off),
                Instr::JumpIfFalse(_) => Instr::JumpIfFalse(off),
                Instr::JumpIfNotTrue(_) => Instr::JumpIfNotTrue(off),
                Instr::JumpIfNone(_) => Instr::JumpIfNone(off),
                Instr::JumpIfSome(_) => Instr::JumpIfSome(off),
                Instr::IterNext(_) => Instr::IterNext(off),
                other => other,
            };
        }
    }

    fn finish(self) -> Func {
        Func {
            name: self.name,
            arity: self.arity,
            nparams: self.nparams,
            nlocals: self.nlocals.max(1),
            max_stack: self.max.max(1) as u16,
            code: self.code,
            spans: self.spans,
        }
    }
}

/// Bộ phát mã.
struct Gen {
    consts: Vec<Const>,
    funcs: Vec<Func>,
    /// `sym` hàm → chỉ số trong `funcs`.
    fmap: HashMap<u32, u32>,
    /// Closure chờ phát: (chỉ số hàm, thân, tham số, snapshot locals của cha).
    cho: Vec<Cho>,
    hash: [u8; HASH_LEN],
}

struct Cho {
    idx: u32,
    params: Vec<vis_types::HParam>,
    body: HBody,
    captured: Vec<(u32, u16)>,
    name: String,
}

impl Gen {
    fn cst(&mut self, kind: LitKind, text: &str) -> u32 {
        let kind = match kind {
            LitKind::Int => 0,
            LitKind::Float => 1,
            LitKind::Size => 2,
            LitKind::Duration => 3,
            LitKind::Percent => 4,
            LitKind::Str => 5,
            LitKind::Bytes => 6,
            LitKind::Path => 7,
            LitKind::Char => 8,
            LitKind::Bool => 9,
        };
        let c = Const {
            kind,
            text: text.to_string(),
        };
        if let Some(i) = self.consts.iter().position(|x| *x == c) {
            return i as u32;
        }
        self.consts.push(c);
        (self.consts.len() - 1) as u32
    }

    fn cst_str(&mut self, s: &str) -> u32 {
        self.cst(LitKind::Str, s)
    }
}

/// Hạ một chương trình HIR xuống module VBC1.
pub fn compile(hir: &Hir, _symbols: &Symbols, text: &str) -> Result<Module, CodegenError> {
    let mut g = Gen {
        consts: Vec::new(),
        funcs: Vec::new(),
        fmap: HashMap::new(),
        cho: Vec::new(),
        hash: crate::format::source_hash(text),
    };
    // Lượt 1: cấp chỉ số cho mọi hàm cấp tệp.
    for it in &hir.items {
        if let HItem::Fn(f) = it {
            g.fmap.insert(f.sym, g.funcs.len() as u32);
            g.funcs.push(Func::default());
        }
    }
    // `main` — câu lệnh cấp tệp.
    let main_idx = g.funcs.len() as u32;
    g.funcs.push(Func::default());
    // Lượt 2: thân hàm cấp tệp.
    for it in &hir.items {
        if let HItem::Fn(f) = it {
            let idx = g.fmap[&f.sym] as usize;
            let mut st = Emit {
                g: &mut g,
                f: F::new(f.name.clone()),
                loops: Vec::new(),
                captured: HashMap::new(),
            };
            st.f.arity = f.params.iter().filter(|p| !p.named).count() as u16;
            st.f.nparams = f.params.len() as u16;
            for p in &f.params {
                st.f.slot(p.sym, p.span)?;
            }
            st.than(&HBody::Stmts(f.body.clone()), f.params.len() as u16, f.span)?;
            st.f.push(Instr::Halt, f.span, 0);
            let done = st.f.finish();
            g.funcs[idx] = done;
        }
    }
    // `main`.
    {
        let mut st = Emit {
            g: &mut g,
            f: F::new("main".to_string()),
            loops: Vec::new(),
            captured: HashMap::new(),
        };
        for s in &hir.stmts {
            st.cau(s)?;
        }
        st.f.push(Instr::Halt, hir.span, 0);
        let done = st.f.finish();
        g.funcs[main_idx as usize] = done;
    }
    // Closure đã gom trong lúc phát.
    while let Some(c) = g.cho.pop() {
        let mut st = Emit {
            g: &mut g,
            f: F::new(c.name.clone()),
            loops: Vec::new(),
            captured: HashMap::new(),
        };
        st.f.arity = c.params.iter().filter(|p| !p.named).count() as u16;
        st.f.nparams = c.params.len() as u16;
        // Ô 0..nfree là biến bắt; tham số bắt đầu sau đó.
        for (i, (_sym, _slot)) in c.captured.iter().enumerate() {
            st.f.nlocals = st.f.nlocals.max(i as u16 + 1);
        }
        for p in &c.params {
            let s = st.f.nlocals;
            st.f.nlocals += 1;
            st.f.locals.insert(p.sym, s);
        }
        for (sym, _s) in &c.captured {
            st.captured.insert(*sym, 0);
        }
        let base = c.params.len() as u16;
        st.than(&c.body, base, Span::new(0, 0))?;
        st.f.push(Instr::Return, Span::new(0, 0), 0);
        let done = st.f.finish();
        let i = c.idx as usize;
        if i < g.funcs.len() {
            g.funcs[i] = done;
        }
    }
    let module = Module {
        version: crate::format::VERSION,
        source_hash: g.hash,
        consts: g.consts,
        funcs: g.funcs,
        entry: main_idx,
    };
    if module.funcs.is_empty() {
        return Err(CodegenError::NoEntry);
    }
    Ok(module)
}

struct Vong {
    /// Đích cần vá khi gặp `break` (về cuối vòng).
    breaks: Vec<usize>,
    /// Đích nhảy của `continue` (về đầu vòng).
    continue_t: usize,
}

struct Emit<'g> {
    g: &'g mut Gen,
    f: F,
    loops: Vec<Vong>,
    captured: HashMap<u32, u16>,
}

impl<'g> Emit<'g> {
    // ----- câu lệnh -----

    fn cau(&mut self, s: &HStmt) -> Result<(), CodegenError> {
        match s {
            HStmt::Let {
                pat, init, span, ..
            } => {
                self.expr(init)?;
                self.bind(pat, *span, true)?;
                Ok(())
            }
            HStmt::Var {
                sym, init, span, ..
            } => {
                self.expr(init)?;
                let slot = self.f.slot(*sym, *span)?;
                self.f.push(Instr::StoreLocal(slot), *span, -1);
                self.net_stack(*span)
            }
            HStmt::Assign {
                target,
                op,
                value,
                span,
            } => {
                if let HExpr::Var { sym: Some(id), .. } = target {
                    match op {
                        vis_types::AssignOp::Set => self.expr(value)?,
                        other => {
                            // `x += v` ⇒ `x = x <op> v`.
                            let slot = self.slot_cua(*id)?;
                            self.f.push(Instr::LoadLocal(slot), *span, 1);
                            self.expr(value)?;
                            self.f.push(Instr::Bin(assign_binop(*other)?), *span, -1);
                        }
                    }
                    let slot = self.slot_cua(*id)?;
                    self.f.push(Instr::Assign(slot), *span, -1);
                    return self.net_stack(*span);
                }
                // `a[i] = v` / `a.f = v` ⇒ `Set` (3→1) rồi `Pop`.
                match target {
                    HExpr::Index { base, index, .. } => {
                        self.expr(base)?;
                        self.expr(index)?;
                        self.expr(value)?;
                        self.f.push(Instr::Set, *span, -2);
                        self.f.push(Instr::Pop, *span, -1);
                        self.net_stack(*span)
                    }
                    HExpr::Field { base, name, .. } => {
                        if let HExpr::Var { sym: Some(id), .. } = &**base {
                            let slot = self.slot_cua(*id)?;
                            self.f.push(Instr::LoadLocal(slot), *span, 1);
                            let n = self.g.cst_str(name);
                            self.f.push(Instr::Const(n), *span, 1);
                            self.expr(value)?;
                            self.f.push(Instr::With(1), *span, -2);
                            self.f.push(Instr::Assign(slot), *span, -1);
                            return self.net_stack(*span);
                        }
                        Err(CodegenError::Unsupported("assign-field", *span))
                    }
                    _ => Err(CodegenError::Unsupported("assign-target", *span)),
                }
            }
            HStmt::Expr(e) => {
                self.expr(e)?;
                self.f.push(Instr::Pop, e.span(), -1);
                self.net_stack(e.span())
            }
            HStmt::If {
                cond,
                then,
                elifs,
                else_,
                span,
            } => {
                let mut jend: Vec<usize> = Vec::new();
                let mut jf = self.cond(cond)?;
                self.khoi(then)?;
                jend.push(self.f.here());
                self.f.push(Instr::Jump(0), *span, 0);
                let mut prev = jf.take();
                for (c, body) in elifs {
                    let at = self.f.here();
                    if let Some(p) = prev {
                        self.f.patch(p, at);
                    }
                    let mut j = self.cond(c)?;
                    self.khoi(body)?;
                    jend.push(self.f.here());
                    self.f.push(Instr::Jump(0), *span, 0);
                    prev = j.take();
                }
                let at = self.f.here();
                if let Some(p) = prev {
                    self.f.patch(p, at);
                }
                if let Some(b) = else_ {
                    self.khoi(b)?;
                }
                let end = self.f.here();
                for j in jend {
                    self.f.patch(j, end);
                }
                Ok(())
            }
            HStmt::For {
                pat,
                iter,
                body,
                span,
            } => {
                self.expr(iter)?;
                self.f.push(Instr::IterInit, *span, 0);
                let start = self.f.here();
                self.f.push(Instr::IterNext(0), *span, 1);
                let next = self.f.code.len() - 1;
                let br = self.f.here();
                self.f.push(Instr::JumpIfFalse(0), *span, -1);
                self.bind(pat, *span, true)?;
                self.loops.push(Vong {
                    breaks: Vec::new(),
                    continue_t: start,
                });
                self.khoi(body)?;
                self.f.push(Instr::Jump(0), *span, 0);
                let back = self.f.code.len() - 1;
                self.f.patch(back, start);
                let end = self.f.here();
                self.f.patch(next, end);
                self.f.patch(br, end);
                if let Some(v) = self.loops.pop() {
                    for b in v.breaks {
                        self.f.patch(b, end);
                    }
                }
                self.f.push(Instr::Pop, *span, -1);
                self.net_stack(*span)
            }
            HStmt::While { cond, body, span } => {
                let start = self.f.here();
                self.expr(cond)?;
                let jf = self.f.here();
                self.f.push(Instr::JumpIfFalse(0), *span, -1);
                self.loops.push(Vong {
                    breaks: Vec::new(),
                    continue_t: start,
                });
                self.khoi(body)?;
                self.f.push(Instr::Jump(0), *span, 0);
                let back = self.f.code.len() - 1;
                self.f.patch(back, start);
                let end = self.f.here();
                self.f.patch(jf, end);
                if let Some(v) = self.loops.pop() {
                    for b in v.breaks {
                        self.f.patch(b, end);
                    }
                }
                Ok(())
            }
            HStmt::Loop { body, span } => {
                let start = self.f.here();
                self.loops.push(Vong {
                    breaks: Vec::new(),
                    continue_t: start,
                });
                self.khoi(body)?;
                self.f.push(Instr::Jump(0), *span, 0);
                let back = self.f.code.len() - 1;
                self.f.patch(back, start);
                let end = self.f.here();
                if let Some(v) = self.loops.pop() {
                    for b in v.breaks {
                        self.f.patch(b, end);
                    }
                }
                Ok(())
            }
            HStmt::Scope { body, .. } | HStmt::Block { body, .. } => self.khoi(body),
            HStmt::Return { value, span } => {
                match value {
                    Some(v) => self.expr(v)?,
                    None => {
                        let c = self.unit_cst();
                        self.f.push(Instr::Const(c), *span, 1);
                    }
                }
                self.f.push(Instr::Return, *span, -1);
                self.net_stack(*span)
            }
            HStmt::Break { span } => {
                let Some(_) = self.loops.last() else {
                    return Err(CodegenError::Unsupported("break", *span));
                };
                let at = self.f.here();
                self.f.push(Instr::Jump(0), *span, 0);
                if let Some(v) = self.loops.last_mut() {
                    v.breaks.push(at);
                }
                Ok(())
            }
            HStmt::Continue { span } => {
                let Some(v) = self.loops.last() else {
                    return Err(CodegenError::Unsupported("continue", *span));
                };
                let t = v.continue_t;
                self.f.push(Instr::Jump(0), *span, 0);
                let at = self.f.code.len() - 1;
                self.f.patch(at, t);
                Ok(())
            }
            HStmt::Match {
                subject,
                arms,
                span,
            } => {
                self.expr(subject)?;
                let mut jend = Vec::new();
                for arm in arms {
                    self.f.push(Instr::Dup, *span, 1);
                    let jf = self.test(&arm.pat, *span)?;
                    let br = self.f.here();
                    self.f.push(Instr::JumpIfFalse(0), *span, -1);
                    self.f.patch(jf, br + 1);
                    if let Some(gd) = &arm.guard {
                        self.expr(gd)?;
                        let g = self.f.here();
                        self.f.push(Instr::JumpIfFalse(0), *span, -1);
                        self.f.patch(g, br + 1);
                    }
                    self.f.push(Instr::Pop, *span, -1);
                    self.khoi(&arm.body)?;
                    jend.push(self.f.here());
                    self.f.push(Instr::Jump(0), *span, 0);
                    let next = self.f.here();
                    self.f.patch(br, next);
                }
                self.f.push(Instr::Pop, *span, -1);
                let end = self.f.here();
                for j in jend {
                    self.f.patch(j, end);
                }
                self.net_stack(*span)
            }
        }
    }

    fn unit_cst(&mut self) -> u32 {
        self.g.cst(LitKind::Str, "\"\"")
    }

    fn slot_cua(&mut self, sym: u32) -> Result<u16, CodegenError> {
        if let Some(s) = self.captured.get(&sym) {
            return Ok(*s);
        }
        self.f.slot(sym, Span::new(0, 0))
    }

    fn net_stack(&mut self, _span: Span) -> Result<(), CodegenError> {
        Ok(())
    }

    /// Khối câu lệnh: giá trị của `HStmt::Expr` cuối **vẫn** nằm trên ngăn xếp (nếu khối "có giá trị").
    fn khoi(&mut self, body: &[HStmt]) -> Result<(), CodegenError> {
        for s in body {
            self.cau(s)?;
        }
        Ok(())
    }

    /// Điều kiện: phát biểu thức rồi trả **chỉ số lệnh nhảy-khi-sai** cần vá.
    fn cond(&mut self, c: &HCond) -> Result<Option<usize>, CodegenError> {
        match c {
            HCond::Expr(e) => {
                self.expr(e)?;
                let at = self.f.here();
                self.f.push(Instr::JumpIfFalse(0), e.span(), -1);
                Ok(Some(at))
            }
            HCond::Let { pat, value, span } => {
                self.expr(value)?;
                self.bind(pat, *span, true)?;
                let at = self.f.here();
                self.f.push(Instr::JumpIfFalse(0), *span, -1);
                Ok(Some(at))
            }
        }
    }

    /// Gán pattern từ **đỉnh** ngăn xếp; `pop` = có lấy giá trị khỏi đỉnh không.
    fn bind(&mut self, p: &HPat, span: Span, pop: bool) -> Result<(), CodegenError> {
        match p {
            HPat::Wild { .. } => {
                if pop {
                    self.f.push(Instr::Pop, span, -1);
                }
                Ok(())
            }
            HPat::Bind { sym, .. } => {
                let slot = self.slot_cua(*sym)?;
                self.f.push(Instr::StoreLocal(slot), span, -1);
                Ok(())
            }
            HPat::Tuple { items, .. } => {
                self.f.push(Instr::Dup, span, 1);
                for (i, it) in items.iter().enumerate() {
                    self.f.push(Instr::Dup, span, 1);
                    let c = self.g.cst(LitKind::Int, &i.to_string());
                    self.f.push(Instr::Const(c), span, 1);
                    self.f.push(Instr::Index, span, -1);
                    self.bind(it, span, true)?;
                }
                self.f.push(Instr::Pop, span, -1);
                Ok(())
            }
            _ => {
                if pop {
                    self.f.push(Instr::Pop, span, -1);
                }
                Ok(())
            }
        }
    }

    /// Phát **test** cho pattern: 1→1 (bool). Trả **vị trí lệnh nhảy** chưa vá (nếu có) — với
    /// `Or`/`Ctor` phức tạp, test dùng lệnh `Test*` tuần tự.
    fn test(&mut self, p: &HPat, span: Span) -> Result<usize, CodegenError> {
        match p {
            HPat::Wild { .. } | HPat::Bind { .. } => {
                let c = self.g.cst(LitKind::Bool, "true");
                self.f.push(Instr::Const(c), span, 1);
                Ok(0)
            }
            HPat::Lit { lit, .. } => {
                let c = self.g.cst(lit.kind, &lit.text);
                self.f.push(Instr::TestLit(c), span, 0);
                Ok(0)
            }
            HPat::Ctor { path, args, .. } => {
                let ten = path.last().cloned().unwrap_or_default();
                let n = self.g.cst_str(&ten);
                self.f.push(Instr::TestCtor(n, args.len() as u16), span, 0);
                Ok(0)
            }
            HPat::Tuple { items, .. } => {
                self.f.push(Instr::TestTuple(items.len() as u16), span, 0);
                Ok(0)
            }
            HPat::List { items, rest, .. } => {
                if rest.is_some() {
                    return Err(CodegenError::Unsupported("list-rest-pattern", span));
                }
                self.f.push(Instr::TestList(items.len() as u16), span, 0);
                Ok(0)
            }
            HPat::Record { fields, open, .. } => {
                let t = self.g.cst_str("");
                let names: Vec<u32> = fields
                    .iter()
                    .map(|(n, _)| {
                        let s = n.clone();
                        self.g.cst_str(&s)
                    })
                    .collect();
                let _ = names;
                self.f.push(
                    Instr::TestRecord(t, fields.len() as u16, u8::from(*open)),
                    span,
                    0,
                );
                Ok(0)
            }
            HPat::Range { .. } => {
                let a = self.g.cst(LitKind::Int, "0");
                let b = self.g.cst(LitKind::Int, "0");
                self.f.push(Instr::TestRange(a, b, 0), span, 0);
                Ok(0)
            }
            HPat::Or { alts, .. } => {
                for alt in alts {
                    let _ = self.test(alt, span)?;
                }
                Ok(0)
            }
            HPat::Rest { .. } => Err(CodegenError::Unsupported("rest-pattern", span)),
        }
    }

    // ----- biểu thức -----

    fn expr(&mut self, e: &HExpr) -> Result<(), CodegenError> {
        match e {
            HExpr::Lit { lit, span, .. } => {
                let c = self.g.cst(lit.kind, &lit.text);
                self.f.push(Instr::Const(c), *span, 1);
                Ok(())
            }
            HExpr::Var { sym, name, span } => {
                match sym {
                    Some(id) => {
                        if let Some(s) = self.captured.get(id) {
                            let s = *s;
                            self.f.push(Instr::LoadCaptured(s), *span, 1);
                        } else {
                            let slot = self.f.slot(*id, *span)?;
                            self.f.push(Instr::LoadLocal(slot), *span, 1);
                        }
                    }
                    None => {
                        // Tên dựng sẵn / hàm chưa rõ: gọi theo tên 0 đối số (VM phân giải).
                        let n = self.g.cst_str(name);
                        self.f.push(Instr::CallBuiltin(n, 0), *span, 1);
                    }
                }
                Ok(())
            }
            HExpr::Bin { op, lhs, rhs, span } => {
                self.expr(lhs)?;
                self.expr(rhs)?;
                self.f.push(Instr::Bin(*op), *span, -1);
                Ok(())
            }
            HExpr::Un { op, expr, span } => {
                self.expr(expr)?;
                self.f.push(Instr::Un(*op), *span, 0);
                Ok(())
            }
            HExpr::Call {
                callee,
                args,
                named,
                span,
            } => {
                let n = (args.len() + named.len()) as u16;
                match &**callee {
                    HExpr::Var { name, sym, .. } => {
                        for a in args {
                            self.expr(a)?;
                        }
                        for (_, a) in named {
                            self.expr(a)?;
                        }
                        match sym {
                            Some(id) if self.g.fmap.contains_key(id) => {
                                let fi = self.g.fmap[id];
                                self.f.push(Instr::Call(fi, n), *span, 1 - i32::from(n));
                            }
                            _ => {
                                let c = self.g.cst_str(name);
                                self.f
                                    .push(Instr::CallBuiltin(c, n), *span, 1 - i32::from(n));
                            }
                        }
                    }
                    HExpr::Field { base, name, .. } => {
                        self.expr(base)?;
                        for a in args {
                            self.expr(a)?;
                        }
                        let c = self.g.cst_str(name);
                        self.f.push(Instr::CallMethod(c, n), *span, -i32::from(n));
                    }
                    other => {
                        self.expr(other)?;
                        for a in args {
                            self.expr(a)?;
                        }
                        let c = self.g.cst_str("<call>");
                        self.f
                            .push(Instr::CallBuiltin(c, n + 1), *span, -i32::from(n));
                    }
                }
                self.net_stack(*span)
            }
            HExpr::Field {
                base, name, span, ..
            } => {
                self.expr(base)?;
                let c = self.g.cst_str(name);
                self.f.push(Instr::Field(c), *span, 0);
                Ok(())
            }
            HExpr::Index { base, index, span } => {
                self.expr(base)?;
                self.expr(index)?;
                self.f.push(Instr::Index, *span, -1);
                Ok(())
            }
            HExpr::List { items, span, .. } => {
                for it in items {
                    self.expr(it)?;
                }
                self.f.push(
                    Instr::MakeList(items.len() as u16),
                    *span,
                    1 - items.len() as i32,
                );
                self.net_stack(*span)
            }
            HExpr::Set { items, span, .. } => {
                for it in items {
                    self.expr(it)?;
                }
                self.f.push(
                    Instr::MakeSet(items.len() as u16),
                    *span,
                    1 - items.len() as i32,
                );
                self.net_stack(*span)
            }
            HExpr::Tuple { items, span, .. } => {
                for it in items {
                    self.expr(it)?;
                }
                self.f.push(
                    Instr::MakeTuple(items.len() as u16),
                    *span,
                    1 - items.len() as i32,
                );
                self.net_stack(*span)
            }
            HExpr::Map { entries, span, .. } => {
                for (k, v) in entries {
                    self.expr(k)?;
                    self.expr(v)?;
                }
                self.f.push(
                    Instr::MakeMap(entries.len() as u16),
                    *span,
                    1 - 2 * entries.len() as i32,
                );
                self.net_stack(*span)
            }
            HExpr::Record { fields, span, .. } => {
                for (n, v) in fields {
                    let c = self.g.cst_str(n);
                    self.f.push(Instr::Const(c), *span, 1);
                    self.expr(v)?;
                }
                let t = self.g.cst_str("");
                self.f.push(
                    Instr::MakeRecord(t, fields.len() as u16),
                    *span,
                    1 - 2 * fields.len() as i32,
                );
                self.net_stack(*span)
            }
            HExpr::EnumLit {
                variant,
                args,
                span,
                ..
            } => {
                for a in args {
                    self.expr(a)?;
                }
                let v = self.g.cst_str(variant);
                self.f.push(
                    Instr::MakeEnum(v, args.len() as u16),
                    *span,
                    1 - args.len() as i32,
                );
                self.net_stack(*span)
            }
            HExpr::Opt { value, span, .. } => {
                match value {
                    Some(v) => {
                        self.expr(v)?;
                        self.f.push(Instr::MakeSome, *span, 0);
                    }
                    None => self.f.push(Instr::MakeNone, *span, 1),
                }
                Ok(())
            }
            HExpr::Try { expr, span } => {
                self.expr(expr)?;
                self.f.push(Instr::Try, *span, 0);
                Ok(())
            }
            HExpr::With { base, fields, span } => {
                self.expr(base)?;
                for (n, v) in fields {
                    let c = self.g.cst_str(n);
                    self.f.push(Instr::Const(c), *span, 1);
                    self.expr(v)?;
                }
                self.f.push(
                    Instr::With(fields.len() as u16),
                    *span,
                    -2 * fields.len() as i32,
                );
                self.net_stack(*span)
            }
            HExpr::If {
                cond,
                then,
                elifs,
                else_,
                span,
                ..
            } => {
                let mut jend: Vec<usize> = Vec::new();
                let mut jf = self.cond(cond)?;
                self.nhanh_gia_tri(then)?;
                jend.push(self.f.here());
                self.f.push(Instr::Jump(0), *span, 0);
                let mut prev = jf.take();
                for (c, body) in elifs {
                    let at = self.f.here();
                    if let Some(p) = prev {
                        self.f.patch(p, at);
                    }
                    let mut j = self.cond(c)?;
                    self.nhanh_gia_tri(body)?;
                    jend.push(self.f.here());
                    self.f.push(Instr::Jump(0), *span, 0);
                    prev = j.take();
                }
                let at = self.f.here();
                if let Some(p) = prev {
                    self.f.patch(p, at);
                }
                self.nhanh_gia_tri(else_)?;
                let end = self.f.here();
                for j in jend {
                    self.f.patch(j, end);
                }
                Ok(())
            }
            HExpr::Match {
                subject,
                arms,
                span,
                ..
            } => {
                self.expr(subject)?;
                let mut jend = Vec::new();
                for arm in arms {
                    self.f.push(Instr::Dup, *span, 1);
                    let jf = self.test(&arm.pat, *span)?;
                    let br = self.f.here();
                    self.f.push(Instr::JumpIfFalse(0), *span, -1);
                    self.f.patch(jf, br + 1);
                    if let Some(gd) = &arm.guard {
                        self.expr(gd)?;
                        let g = self.f.here();
                        self.f.push(Instr::JumpIfFalse(0), *span, -1);
                        self.f.patch(g, br + 1);
                    }
                    self.f.push(Instr::Pop, *span, -1);
                    self.nhanh_gia_tri(&arm.body)?;
                    jend.push(self.f.here());
                    self.f.push(Instr::Jump(0), *span, 0);
                    let next = self.f.here();
                    self.f.patch(br, next);
                }
                {
                    let c = self.unit_cst_x();
                    self.f.push(Instr::Const(c), *span, 1);
                }
                let end = self.f.here();
                for j in jend {
                    self.f.patch(j, end);
                }
                Ok(())
            }
            HExpr::Lambda {
                params, body, span, ..
            } => {
                let idx = self.g.funcs.len() as u32;
                self.g.funcs.push(Func {
                    name: "<closure>".to_string(),
                    ..Func::default()
                });
                // snapshot **cả khung hiện tại** (VD-51) — thứ tự ô = thứ tự slot.
                let mut pair: Vec<(u32, u16)> =
                    self.f.locals.iter().map(|(k, v)| (*k, *v)).collect();
                pair.sort_by_key(|(_, v)| *v);
                for (_sym, slot) in &pair {
                    let s = *slot;
                    self.f.push(Instr::LoadLocal(s), *span, 1);
                }
                self.g.cho.push(Cho {
                    idx,
                    params: params.clone(),
                    body: body.clone(),
                    captured: pair.clone(),
                    name: "<closure>".to_string(),
                });
                self.f.push(
                    Instr::MakeClosure(idx, pair.len() as u16),
                    *span,
                    1 - pair.len() as i32,
                );
                self.net_stack(*span)
            }
            HExpr::Interp { parts, span } => {
                for p in parts {
                    match p {
                        InterpPart::Str(s) => {
                            let c = self.g.cst(LitKind::Str, &format!("\"{s}\""));
                            self.f.push(Instr::Const(c), *span, 1);
                        }
                        InterpPart::Expr(x) => self.expr(x)?,
                    }
                }
                self.f.push(
                    Instr::Interp(parts.len() as u16),
                    *span,
                    1 - parts.len() as i32,
                );
                self.net_stack(*span)
            }
            HExpr::Error { span } => {
                let c = self.g.cst(LitKind::Str, "\"\"");
                self.f.push(Instr::Const(c), *span, 1);
                Ok(())
            }
            HExpr::Ref { span, .. } => Err(CodegenError::Unsupported("ref", *span)),
            HExpr::Await { span, .. } => Err(CodegenError::Unsupported("await", *span)),
            HExpr::CmdBlock { span, .. } => Err(CodegenError::Unsupported("cmd-block", *span)),
        }
    }

    fn unit_cst_x(&mut self) -> u32 {
        self.g.cst(LitKind::Bool, "false")
    }

    /// Nhánh "có giá trị": câu lệnh cuối là `Expr` ⇒ **giữ** giá trị trên ngăn xếp.
    fn nhanh_gia_tri(&mut self, body: &[HStmt]) -> Result<(), CodegenError> {
        if let Some((last, rest)) = body.split_last() {
            for s in rest {
                self.cau(s)?;
            }
            self.tra_gia_tri(last)
        } else {
            let c = self.unit_cst();
            self.f.push(Instr::Const(c), Span::new(0, 0), 1);
            Ok(())
        }
    }

    /// Như [`Self::cau`] nhưng `HStmt::Expr` **không** bị `Pop` (để lại giá trị).
    fn tra_gia_tri(&mut self, s: &HStmt) -> Result<(), CodegenError> {
        match s {
            HStmt::Expr(e) => self.expr(e),
            HStmt::If {
                cond,
                then,
                elifs,
                else_,
                span,
            } => {
                let mut jend: Vec<usize> = Vec::new();
                let mut jf = self.cond(cond)?;
                self.nhanh_gia_tri(then)?;
                jend.push(self.f.here());
                self.f.push(Instr::Jump(0), *span, 0);
                let mut prev = jf.take();
                for (c, body) in elifs {
                    let at = self.f.here();
                    if let Some(p) = prev {
                        self.f.patch(p, at);
                    }
                    let mut j = self.cond(c)?;
                    self.nhanh_gia_tri(body)?;
                    jend.push(self.f.here());
                    self.f.push(Instr::Jump(0), *span, 0);
                    prev = j.take();
                }
                let at = self.f.here();
                if let Some(p) = prev {
                    self.f.patch(p, at);
                }
                match else_ {
                    Some(b) => self.nhanh_gia_tri(b)?,
                    None => {
                        let c = self.unit_cst();
                        self.f.push(Instr::Const(c), *span, 1);
                    }
                }
                let end = self.f.here();
                for j in jend {
                    self.f.patch(j, end);
                }
                Ok(())
            }
            HStmt::Match {
                subject,
                arms,
                span,
            } => {
                self.expr(subject)?;
                let mut jend = Vec::new();
                for arm in arms {
                    self.f.push(Instr::Dup, *span, 1);
                    let jf = self.test(&arm.pat, *span)?;
                    let br = self.f.here();
                    self.f.push(Instr::JumpIfFalse(0), *span, -1);
                    self.f.patch(jf, br + 1);
                    if let Some(gd) = &arm.guard {
                        self.expr(gd)?;
                        let g = self.f.here();
                        self.f.push(Instr::JumpIfFalse(0), *span, -1);
                        self.f.patch(g, br + 1);
                    }
                    self.f.push(Instr::Pop, *span, -1);
                    self.nhanh_gia_tri(&arm.body)?;
                    jend.push(self.f.here());
                    self.f.push(Instr::Jump(0), *span, 0);
                    let next = self.f.here();
                    self.f.patch(br, next);
                }
                let c = self.unit_cst_x();
                self.f.push(Instr::Const(c), *span, 1);
                let end = self.f.here();
                for j in jend {
                    self.f.patch(j, end);
                }
                Ok(())
            }
            other => {
                self.cau(other)?;
                let c = self.unit_cst();
                self.f.push(Instr::Const(c), other.span(), 1);
                Ok(())
            }
        }
    }

    /// Thân hàm: `HBody::Expr` ⇒ biểu thức; `HBody::Stmts` ⇒ câu lệnh (giá trị cuối giữ lại).
    fn than(&mut self, b: &HBody, _base: u16, span: Span) -> Result<(), CodegenError> {
        match b {
            HBody::Expr(e) => self.expr(e),
            HBody::Stmts(st) => {
                for s in st {
                    self.cau(s)?;
                }
                let c = self.unit_cst();
                self.f.push(Instr::Return, span, 0);
                let _ = c;
                self.net_stack(span)
            }
        }
    }
}

fn assign_binop(op: vis_types::AssignOp) -> Result<vis_types::BinOp, CodegenError> {
    use vis_types::{AssignOp, BinOp};
    Ok(match op {
        AssignOp::Set => BinOp::Add,
        AssignOp::Add => BinOp::Add,
        AssignOp::Sub => BinOp::Sub,
        AssignOp::Mul => BinOp::Mul,
        AssignOp::Div => BinOp::Div,
        AssignOp::Rem => BinOp::Rem,
    })
}

/// `binop_tag` của lệnh `Bin` — tiện cho test.
pub fn bin_tag(op: vis_types::BinOp) -> u8 {
    binop_tag(op)
}

/// `unop_tag` của lệnh `Un` — tiện cho test.
pub fn un_tag(op: vis_types::UnOp) -> u8 {
    unop_tag(op)
}
