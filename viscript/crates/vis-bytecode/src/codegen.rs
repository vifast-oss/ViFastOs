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

mod pattern;

use std::collections::HashMap;

use vis_syntax::Span;
use vis_types::{HBody, HCond, HExpr, HItem, HPat, HStmt, Hir, InterpPart, LitKind, Symbols};

use crate::format::{binop_tag, unop_tag};
use crate::format::{Const, Func, Instr, Module, HASH_LEN};
use crate::util::{assign_binop, can_test, la_builtin};

/// Lỗi codegen — **không** panic; mỗi lỗi nêu biến thể HIR + span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodegenError {
    /// Biến thể VS1 chưa biểu diễn được (kèm tên + span).
    Unsupported(&'static str, Span),
    /// Quá nhiều ô cục bộ / đối số cho `u16`.
    TooMany(u16),
    /// Module không có hàm `main`.
    NoEntry,
    /// Mã vừa sinh **không qua** verifier (bug codegen — báo, không nới).
    KhongHopLe(crate::verify::VerifyError),
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
            CodegenError::KhongHopLe(e) => write!(f, "mã sinh ra không hợp lệ: {e}"),
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
        // Quy ước: `đích = chỉ_số_lệnh_nhảy + offset` (VM: `pc_sau_khi_đọc - 1 + off`).
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

    /// Chốt hàm: `max_stack` lấy từ **chính** hàm mô phỏng của verifier (một nguồn sự thật).
    fn finish(self) -> Result<Func, CodegenError> {
        if std::env::var("VBC_DBG").is_ok() {
            for (i, ins) in self.code.iter().enumerate() {
                eprintln!("{i:3} {ins:?}");
            }
        }
        let need = crate::verify::need_stack(&self.code, 0).map_err(CodegenError::KhongHopLe)?;
        Ok(Func {
            name: self.name,
            arity: self.arity,
            nparams: self.nparams,
            nlocals: self.nlocals.max(1),
            max_stack: need,
            code: self.code,
            spans: self.spans,
        })
    }
}

/// Bộ phát mã.
struct Gen {
    consts: Vec<Const>,
    funcs: Vec<Func>,
    /// `sym` hàm → chỉ số trong `funcs`.
    fmap: HashMap<u32, u32>,
    /// `sym` enum → tên enum (để `E.A` thành `MakeEnum`, không phải `Field`).
    enums: HashMap<u32, String>,
    /// Tên hàm → chỉ số (dự phòng khi `sym` của lời gọi là `None`).
    fmap_ten: HashMap<String, u32>,
    /// Tên hàm → danh sách `(tên tham số, là named)` để xếp đối số theo **tên**.
    fparams: HashMap<String, Vec<(String, bool)>>,
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
        enums: HashMap::new(),
        fmap_ten: HashMap::new(),
        fparams: HashMap::new(),
        cho: Vec::new(),
        hash: crate::format::source_hash(text),
    };
    // Lượt 1: cấp chỉ số cho mọi hàm cấp tệp + ghi tên enum.
    for it in &hir.items {
        if let HItem::Enum(e) = it {
            g.enums.insert(e.sym, e.name.clone());
        }
        if let HItem::Fn(f) = it {
            g.fmap.insert(f.sym, g.funcs.len() as u32);
            g.fmap_ten.insert(f.name.clone(), g.funcs.len() as u32);
            g.fparams.insert(
                f.name.clone(),
                f.params.iter().map(|p| (p.name.clone(), p.named)).collect(),
            );
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
            // Tham số có `default`: nếu ô còn `()` (không được truyền) ⇒ gán default.
            for p in &f.params {
                if let Some(d) = &p.default {
                    let slot = st.f.slot(p.sym, p.span)?;
                    st.f.push(Instr::LoadLocal(slot), p.span, 1);
                    st.f.push(Instr::IsUnit, p.span, 0);
                    let j = st.f.here();
                    st.f.push(Instr::JumpIfFalse(0), p.span, -1);
                    st.expr(d)?;
                    st.f.push(Instr::StoreLocal(slot), p.span, -1);
                    let end = st.f.here();
                    st.f.patch(j, end);
                }
            }
            st.than(&HBody::Stmts(f.body.clone()), f.params.len() as u16, f.span)?;
            st.f.push(Instr::Halt, f.span, 0);
            let done = st.f.finish()?;
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
        // Giá trị chương trình: interp trả giá trị **câu lệnh cuối nếu là biểu thức**, còn lại `()`.
        match hir.stmts.split_last() {
            Some((last, rest)) => {
                for s in rest {
                    st.cau(s)?;
                }
                if let HStmt::Expr(e) = last {
                    st.expr(e)?;
                    st.f.push(Instr::Return, e.span(), 0);
                } else {
                    st.cau(last)?;
                    st.f.push(Instr::Halt, hir.span, 0);
                }
            }
            None => st.f.push(Instr::Halt, hir.span, 0),
        }
        let done = st.f.finish()?;
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
        // **Tham số trước** (ô 0..n — VM đổ đối số vào đầu khung), biến bắt giữ ở `captured`.
        for p in &c.params {
            let s = st.f.nlocals;
            st.f.nlocals += 1;
            st.f.locals.insert(p.sym, s);
        }
        for (i, (sym, _s)) in c.captured.iter().enumerate() {
            st.captured.insert(*sym, i as u16);
        }
        let base = c.params.len() as u16;
        st.than(&c.body, base, Span::new(0, 0))?;
        st.f.push(Instr::Return, Span::new(0, 0), 0);
        let done = st.f.finish()?;
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
                pat,
                ty,
                init,
                span,
                ..
            } => {
                self.expr(init)?;
                // Tự nâng `T → T?` theo **kiểu khai** (§4.4) — interp làm ở `let`.
                if matches!(ty, Some(vis_types::Type::Opt(_))) && !matches!(init, HExpr::Opt { .. })
                {
                    self.f.push(Instr::MakeSome, *span, 0);
                }
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
                // `x.push(v)` — interp ghi **trả lại** ô (CoW, VD-51): `x = List([...])`.
                if let HExpr::Call {
                    callee,
                    args,
                    named,
                    span,
                } = e
                {
                    if let HExpr::Field { base, name, .. } = &**callee {
                        if name == "push" {
                            if let HExpr::Var { sym: Some(id), .. } = &**base {
                                let slot = self.slot_cua(*id)?;
                                self.f.push(Instr::LoadLocal(slot), *span, 1);
                                for a in args {
                                    self.expr(a)?;
                                }
                                let c = self.g.cst_str(name);
                                self.f.push(
                                    Instr::CallMethod(c, args.len() as u16),
                                    *span,
                                    -i32::from(args.len() as u16),
                                );
                                let _ = named;
                                self.f.push(Instr::Assign(slot), *span, -1);
                                return self.net_stack(*span);
                            }
                        }
                    }
                }
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
                if let Some(v) = self.loops.pop() {
                    for b in v.breaks {
                        self.f.patch(b, end);
                    }
                }
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
            HStmt::Scope { body, span, .. } | HStmt::Block { body, span } => {
                // Khối **có giá trị**: câu lệnh cuối là biểu thức ⇒ giữ lại rồi `Pop`.
                self.nhanh_gia_tri(body)?;
                self.f.push(Instr::Pop, *span, -1);
                self.net_stack(*span)
            }
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
                let mut guardas: Vec<usize> = Vec::new();
                for arm in arms.iter() {
                    self.f.push(Instr::Dup, *span, 1);
                    let jf = self.test(&arm.pat, *span)?;
                    let br = self.f.here();
                    self.f.push(Instr::JumpIfFalse(0), *span, -1);
                    if let Some(j) = jf {
                        self.f.patch(j, br + 1);
                    }
                    // Bind **trước** guard (interp bind rồi mới xét guard); giữ subject để `Pop`.
                    self.bind(&arm.pat, *span, false)?;
                    if let Some(gd) = &arm.guard {
                        self.expr(gd)?;
                        let g = self.f.here();
                        self.f.push(Instr::JumpIfFalse(0), *span, -1);
                        guardas.push(g);
                    }
                    self.f.push(Instr::Pop, *span, -1);
                    self.khoi(&arm.body)?;
                    jend.push(self.f.here());
                    self.f.push(Instr::Jump(0), *span, 0);
                    let next = self.f.here();
                    self.f.patch(br, next);
                    for g in guardas.drain(..) {
                        self.f.patch(g, next);
                    }
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
                self.f.push(Instr::Dup, *span, 1);
                let _ = self.test(pat, *span)?;
                let at = self.f.here();
                self.f.push(Instr::JumpIfFalse(0), *span, -1);
                // Đúng: gán biến pattern rồi bỏ giá trị.
                self.bind(pat, *span, false)?;
                self.f.push(Instr::Pop, *span, -1);
                Ok(Some(at))
            }
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
                    Some(id) if self.g.fmap.contains_key(id) => {
                        // Tên hàm dùng như **giá trị** ⇒ closure 0 biến bắt.
                        let fi = self.g.fmap[id];
                        self.f.push(Instr::MakeClosure(fi, 0), *span, 1);
                    }
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
                use vis_types::BinOp;
                match op {
                    // `and`/`or`/`??` **đoản mạch** — không gọi bảng ngữ nghĩa (interp cũng vậy).
                    BinOp::And => {
                        self.expr(lhs)?;
                        self.f.push(Instr::Dup, *span, 1);
                        let j = self.f.here();
                        self.f.push(Instr::JumpIfFalse(0), *span, -1);
                        self.f.push(Instr::Pop, *span, -1);
                        self.expr(rhs)?;
                        let end = self.f.here();
                        self.f.patch(j, end);
                        Ok(())
                    }
                    BinOp::Or => {
                        self.expr(lhs)?;
                        self.f.push(Instr::Dup, *span, 1);
                        self.f.push(Instr::Un(vis_types::UnOp::Not), *span, 0);
                        let j = self.f.here();
                        self.f.push(Instr::JumpIfFalse(0), *span, -1);
                        self.f.push(Instr::Pop, *span, -1);
                        self.expr(rhs)?;
                        let end = self.f.here();
                        self.f.patch(j, end);
                        Ok(())
                    }
                    BinOp::Coalesce => {
                        self.expr(lhs)?;
                        self.f.push(Instr::Dup, *span, 1);
                        let jn = self.f.here();
                        self.f.push(Instr::JumpIfNone(0), *span, -1);
                        self.f.push(Instr::UnwrapOpt, *span, 0);
                        let jend = self.f.here();
                        self.f.push(Instr::Jump(0), *span, 0);
                        let lnone = self.f.here();
                        self.f.patch(jn, lnone);
                        self.f.push(Instr::Pop, *span, -1);
                        self.expr(rhs)?;
                        let end = self.f.here();
                        self.f.patch(jend, end);
                        Ok(())
                    }
                    _ => {
                        self.expr(lhs)?;
                        self.expr(rhs)?;
                        self.f.push(Instr::Bin(*op), *span, -1);
                        Ok(())
                    }
                }
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
                        // Xác định **cách gọi trước** khi đẩy đối số (thứ tự ngăn xếp: hàm ⇒ đối số).
                        let fi = match sym {
                            Some(id) if self.g.fmap.contains_key(id) => Some(self.g.fmap[id]),
                            _ => self.g.fmap_ten.get(name).copied(),
                        };
                        let dong = fi.is_none() && !la_builtin(name);
                        // Gọi bằng **`--tên`**: xếp lại đối số theo **thứ tự tham số** của hàm.
                        if !named.is_empty() {
                            if let Some(ds) = self.g.fparams.get(name).cloned() {
                                let mut vt = 0usize;
                                for (pname, la_named) in ds.iter() {
                                    if !*la_named {
                                        if let Some(a) = args.get(vt) {
                                            self.expr(a)?;
                                            vt += 1;
                                        } else {
                                            self.f.push(Instr::PushUnit, *span, 1);
                                        }
                                    } else if let Some((_, a)) =
                                        named.iter().find(|(n, _)| n == pname)
                                    {
                                        self.expr(a)?;
                                    } else {
                                        self.f.push(Instr::PushUnit, *span, 1);
                                    }
                                }
                                let fi =
                                    self.g.fmap_ten.get(name).copied().or_else(|| {
                                        sym.and_then(|id| self.g.fmap.get(&id).copied())
                                    });
                                match fi {
                                    Some(fi) => {
                                        let tong = ds.len() as u16;
                                        self.f.push(
                                            Instr::Call(fi, tong),
                                            *span,
                                            1 - i32::from(tong),
                                        );
                                        return self.net_stack(*span);
                                    }
                                    None => {
                                        let c = self.g.cst_str(name);
                                        let tong = ds.len() as u16;
                                        self.f.push(
                                            Instr::CallBuiltin(c, tong),
                                            *span,
                                            1 - i32::from(tong),
                                        );
                                        return self.net_stack(*span);
                                    }
                                }
                            }
                        }
                        if dong {
                            if let Some(id) = sym {
                                if let Some(c) = self.captured.get(id).copied() {
                                    self.f.push(Instr::LoadCaptured(c), *span, 1);
                                } else {
                                    let slot = self.slot_cua(*id)?;
                                    self.f.push(Instr::LoadLocal(slot), *span, 1);
                                }
                            }
                        }
                        for a in args {
                            self.expr(a)?;
                        }
                        for (_, a) in named {
                            self.expr(a)?;
                        }
                        match fi {
                            Some(fi) => self.f.push(Instr::Call(fi, n), *span, 1 - i32::from(n)),
                            None if dong && sym.is_some() => {
                                self.f.push(Instr::CallValue(n), *span, -i32::from(n))
                            }
                            None => {
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
                        // Gọi **kết quả biểu thức** (vd `f(1)(2)`) ⇒ đẩy hàm trước, rồi đối số.
                        self.expr(other)?;
                        for a in args {
                            self.expr(a)?;
                        }
                        self.f.push(Instr::CallValue(n), *span, -i32::from(n));
                    }
                }
                self.net_stack(*span)
            }
            HExpr::Field {
                base,
                name,
                optional,
                span,
            } => {
                // `E.A` khi `E` là **enum đã khai**: dựng biến thể, không phải truy cập trường.
                if let HExpr::Var { sym: Some(id), .. } = &**base {
                    if let Some(ten) = self.g.enums.get(id).cloned() {
                        let c = self.g.cst_str(&format!("{ten}.{name}"));
                        self.f.push(Instr::MakeEnum(c, 0), *span, 1);
                        return Ok(());
                    }
                }
                if *optional {
                    // `x?.y`: `none` ⇒ `none`; `some(v)` ⇒ `some(v.y)`.
                    self.expr(base)?;
                    self.f.push(Instr::Dup, *span, 1);
                    let jn = self.f.here();
                    self.f.push(Instr::JumpIfNone(0), *span, -1);
                    self.f.push(Instr::UnwrapOpt, *span, 0);
                    let c = self.g.cst_str(name);
                    self.f.push(Instr::Field(c), *span, 0);
                    self.f.push(Instr::MakeSome, *span, 0);
                    let jend = self.f.here();
                    self.f.push(Instr::Jump(0), *span, 0);
                    let lnone = self.f.here();
                    self.f.patch(jn, lnone);
                    self.f.push(Instr::Pop, *span, -1);
                    self.f.push(Instr::MakeNone, *span, 1);
                    let end = self.f.here();
                    self.f.patch(jend, end);
                    return Ok(());
                }
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
                path,
                variant,
                args,
                span,
                ..
            } => {
                for a in args {
                    self.expr(a)?;
                }
                // Giữ **cả** tên enum (nếu biết) để `MakeEnum` dựng đúng `Value::Enum`.
                let mut ten = path.join(".");
                if ten.is_empty() {
                    ten = variant.clone();
                } else {
                    ten = format!("{ten}.{variant}");
                }
                let v = self.g.cst_str(&ten);
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
                let mut guardas: Vec<usize> = Vec::new();
                let mut cuoi: Vec<(usize, Vec<usize>)> = Vec::new();
                for (i, arm) in arms.iter().enumerate() {
                    self.f.push(Instr::Dup, *span, 1);
                    let jf = self.test(&arm.pat, *span)?;
                    let br = self.f.here();
                    self.f.push(Instr::JumpIfFalse(0), *span, -1);
                    if let Some(j) = jf {
                        self.f.patch(j, br + 1);
                    }
                    // Bind **trước** guard (interp bind rồi mới xét guard); giữ subject để `Pop`.
                    self.bind(&arm.pat, *span, false)?;
                    if let Some(gd) = &arm.guard {
                        self.expr(gd)?;
                        let g = self.f.here();
                        self.f.push(Instr::JumpIfFalse(0), *span, -1);
                        guardas.push(g);
                    }
                    self.f.push(Instr::Pop, *span, -1);
                    self.nhanh_gia_tri(&arm.body)?;
                    jend.push(self.f.here());
                    self.f.push(Instr::Jump(0), *span, 0);
                    let next = self.f.here();
                    if i + 1 == arms.len() {
                        // Nhánh **cuối** trượt ⇒ phải `Pop` subject trước khi đẩy giá trị dự phòng.
                        cuoi.push((br, guardas.clone()));
                    } else {
                        self.f.patch(br, next);
                        for g in guardas.drain(..) {
                            self.f.patch(g, next);
                        }
                    }
                }
                let lpop = self.f.here();
                self.f.push(Instr::Pop, *span, -1);
                for (br, gs) in cuoi {
                    self.f.patch(br, lpop);
                    for g in gs {
                        self.f.patch(g, lpop);
                    }
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
                let mut guardas: Vec<usize> = Vec::new();
                let mut cuoi: Vec<(usize, Vec<usize>)> = Vec::new();
                for (i, arm) in arms.iter().enumerate() {
                    self.f.push(Instr::Dup, *span, 1);
                    let jf = self.test(&arm.pat, *span)?;
                    let br = self.f.here();
                    self.f.push(Instr::JumpIfFalse(0), *span, -1);
                    if let Some(j) = jf {
                        self.f.patch(j, br + 1);
                    }
                    // Bind **trước** guard (interp bind rồi mới xét guard); giữ subject để `Pop`.
                    self.bind(&arm.pat, *span, false)?;
                    if let Some(gd) = &arm.guard {
                        self.expr(gd)?;
                        let g = self.f.here();
                        self.f.push(Instr::JumpIfFalse(0), *span, -1);
                        guardas.push(g);
                    }
                    self.f.push(Instr::Pop, *span, -1);
                    self.nhanh_gia_tri(&arm.body)?;
                    jend.push(self.f.here());
                    self.f.push(Instr::Jump(0), *span, 0);
                    let next = self.f.here();
                    if i + 1 == arms.len() {
                        // Nhánh **cuối** trượt ⇒ `Pop` subject trước khi đẩy giá trị dự phòng.
                        cuoi.push((br, guardas.clone()));
                    } else {
                        self.f.patch(br, next);
                        for g in guardas.drain(..) {
                            self.f.patch(g, next);
                        }
                    }
                }
                let lpop = self.f.here();
                self.f.push(Instr::Pop, *span, -1);
                for (br, gs) in cuoi {
                    self.f.patch(br, lpop);
                    for g in gs {
                        self.f.patch(g, lpop);
                    }
                }
                let c = self.unit_cst_x();
                self.f.push(Instr::Const(c), *span, 1);
                let end = self.f.here();
                for j in jend {
                    self.f.patch(j, end);
                }
                Ok(())
            }
            HStmt::Scope { body, .. } | HStmt::Block { body, .. } => self.nhanh_gia_tri(body),
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
                // Câu lệnh cuối là biểu thức ⇒ **giữ** giá trị làm kết quả hàm (giống interp).
                self.nhanh_gia_tri(st)?;
                self.f.push(Instr::Return, span, 0);
                self.net_stack(span)
            }
        }
    }
}

/// `binop_tag` của lệnh `Bin` — tiện cho test.
pub fn bin_tag(op: vis_types::BinOp) -> u8 {
    binop_tag(op)
}

/// `unop_tag` của lệnh `Un` — tiện cho test.
pub fn un_tag(op: vis_types::UnOp) -> u8 {
    unop_tag(op)
}
