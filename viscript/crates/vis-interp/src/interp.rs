// SPDX-License-Identifier: MIT OR Apache-2.0
//! Bộ thông dịch cây trên HIR: môi trường `sym → Value`, câu lệnh, biểu thức, pattern, gọi hàm.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use vis_diag::{Code, RUNTIME_CODES, TYPE_CODES};
use vis_syntax::{Diagnostic, Span};
use vis_types::hir::*;
use vis_types::lower::Symbols;
use vis_types::TypeTable;

use crate::gan::{gan_chi_so, gan_truong};
use crate::value::*;

/// `VS0501` — lỗi runtime chung.
pub const RT: &Code = &RUNTIME_CODES[0];
/// `VS0502` — chia/lấy dư cho 0.
pub const DIV0: &Code = &RUNTIME_CODES[1];
/// `VS0503` — chỉ số ngoài phạm vi.
pub const IDX: &Code = &RUNTIME_CODES[2];
/// **ST-60** — `VS0506`: lỗi trong **thân tác vụ** `async` (message kèm mã gốc).
pub const TASK_ERR: &Code = &RUNTIME_CODES[5];
/// **ST-61 (VD-96)** — `VS0504`: gửi vào kênh đã `close`.
pub const SEND_CLOSED: &Code = &RUNTIME_CODES[3];
/// **ST-61 (VD-96)** — `VS0505`: deadlock (kênh chặn, không còn tác vụ chạy được).
pub const DEADLOCK: &Code = &RUNTIME_CODES[4];
/// `VS0412` — gán cho `let`/tham số (phòng thủ: tyck đã chặn trước).
pub(crate) const MISMATCH: &Code = &TYPE_CODES[3];

/// **ST-63 (VD-105 (6))**: phần **gọi hàm/closure** tách khỏi `interp.rs` để giữ **R1 ≤ 1.500 dòng**
/// — chuyển **nguyên văn**, không đổi ngữ nghĩa.
mod goi;

/// Lỗi runtime kèm `span`.
#[derive(Debug, Clone)]
pub struct Trap {
    pub code: &'static Code,
    pub msg: String,
    pub span: Span,
}

impl Trap {
    pub fn new(code: &'static Code, msg: impl Into<String>, span: Span) -> Self {
        Self {
            code,
            msg: msg.into(),
            span,
        }
    }

    /// Chẩn đoán có **`line`/`col` thật** từ `span` + văn bản (VD-46).
    pub fn to_diag(&self, text: &str) -> Diagnostic {
        let (line, col) = pos(text, self.span);
        Diagnostic {
            code: self.code,
            message: self.msg.clone(),
            span: self.span,
            line,
            col,
        }
    }
}

/// Dừng chương trình: lỗi runtime, hoặc `return`/`?` trả sớm.
#[derive(Debug)]
pub enum Abort {
    Trap(Trap),
    Ret(Value),
}

pub type R<T> = Result<T, Abort>;

/// Đối số đã eval: (vị trí, có tên).
pub(crate) type Args = (Vec<Value>, Vec<(String, Value)>);

/// Nhánh `match` khớp + binding của nó.
pub(crate) type Picked<'s> = (&'s HArm, Vec<(u32, Value)>);

/// Luồng điều khiển trong thân: bình thường / `break` / `continue`.
#[derive(Debug, PartialEq)]
pub(crate) enum Ctrl {
    Normal,
    Break,
    Continue,
}

pub(crate) struct Interp<'a> {
    pub(crate) fns: HashMap<u32, &'a HFn>,
    consts: HashMap<u32, &'a HExpr>,
    /// `enum`/`type` khai trong tệp: `sym → (tên, là_enum)` để `E.A`/`User(...)` có "thẻ kiểu".
    tags: HashMap<u32, (String, bool)>,
    main: Option<&'a HFn>,
    stmts: &'a [HStmt],
    pub(crate) env: Env,
    pub(crate) mutable: HashSet<u32>,
    pub(crate) out: String,
    pub steps: usize,
    /// Giá trị của câu lệnh `Expr` **vừa chạy** (để `value` cấp tệp, không eval hai lần).
    last: Option<Value>,
    /// Độ sâu vòng lặp đang chạy (ST-13b mục 8: `break`/`continue` ngoài vòng ⇒ `VS0501`).
    loop_depth: u32,
    /// Độ sâu hàm đang chạy (ST-13b mục 8: `?` ngoài hàm ⇒ `VS0501`).
    fn_depth: u32,
    /// Độ sâu **khung gọi** đang mở (ST-15): vượt [`MAX_CALL_DEPTH`] ⇒ `VS0501`, không tràn stack.
    depth: u32,
    types: &'a TypeTable,
    #[allow(dead_code)]
    symbols: &'a Symbols,
    /// Host chương trình ngoài (ST-27); `None` ⇒ `VS0604` — interp **không** biết OS.
    pub(crate) ngoai: Option<&'a mut dyn crate::builtin::Ngoai>,
    /// Host hệ thống tệp (ST-33); `None` ⇒ `VS0604` — interp **không** biết `std::fs`.
    pub(crate) fs: Option<&'a mut dyn crate::builtin::FsHost>,
    /// **ST-63 (VD-105)**: host mạng; `None` ⇒ `VS0604` — interp **không** biết `std::net`.
    pub(crate) net: Option<&'a mut dyn crate::builtin::NetHost>,
    /// **ST-60**: hàng đợi tác vụ `async` (một `Scheduler` cho mỗi lần chạy).
    pub(crate) sched: std::cell::RefCell<crate::rt::Scheduler>,
    /// **ST-72b (VD-103/VD-106)**: quyền `main` — `(chỉ số tham số, giá trị)` theo thứ tự khai.
    caps: Vec<(u32, Value)>,
}

impl<'a> Interp<'a> {
    pub(crate) fn new(hir: &'a Hir, symbols: &'a Symbols, types: &'a TypeTable) -> Self {
        let mut fns = HashMap::new();
        let mut consts = HashMap::new();
        let mut tags = HashMap::new();
        let mut main = None;
        for item in &hir.items {
            match item {
                HItem::Fn(f) => {
                    fns.insert(f.sym, f);
                    if f.name == "main" {
                        main = Some(f);
                    }
                }
                HItem::Const(c) => {
                    consts.insert(c.sym, &c.value);
                }
                HItem::Enum(e) => {
                    tags.insert(e.sym, (e.name.clone(), true));
                }
                // ST-29: phương thức `impl` là hàm thường (dispatch tĩnh trỏ thẳng `sym`).
                HItem::Impl(i) => {
                    for m in &i.methods {
                        fns.insert(m.sym, m);
                    }
                }
                HItem::Rec(r) => {
                    tags.insert(r.sym, (r.name.clone(), false));
                }
                _ => {}
            }
        }
        Self {
            fns,
            consts,
            tags,
            main,
            stmts: &hir.stmts,
            env: Rc::new(std::cell::RefCell::new(HashMap::new())),
            mutable: HashSet::new(),
            out: String::new(),
            steps: 0,
            last: None,
            loop_depth: 0,
            fn_depth: 0,
            depth: 0,
            types,
            symbols,
            ngoai: None,
            fs: None,
            net: None,
            sched: std::cell::RefCell::new(crate::rt::Scheduler::new()),
            caps: Vec::new(),
        }
    }

    /// Gắn host chương trình ngoài (ST-27) — chỉ dùng trong crate (`run_with_ngoai`).
    #[allow(dead_code)]
    pub(crate) fn set_ngoai(&mut self, n: &'a mut dyn crate::builtin::Ngoai) {
        self.ngoai = Some(n);
    }

    /// Gắn host hệ thống tệp (ST-33) — chỉ dùng trong crate (`run_with_hosts`).
    #[allow(dead_code)]
    pub(crate) fn set_fs(&mut self, f: &'a mut dyn crate::builtin::FsHost) {
        self.fs = Some(f);
    }

    /// **ST-63 (VD-105)**: gắn host mạng — chỉ dùng trong crate (`run_with_hosts`).
    #[allow(dead_code)]
    pub(crate) fn set_net(&mut self, n: &'a mut dyn crate::builtin::NetHost) {
        self.net = Some(n);
    }

    /// **ST-72b**: gắn giá trị quyền cho các tham số `cap` của `main`.
    pub(crate) fn set_caps(&mut self, caps: Vec<(u32, Value)>) {
        self.caps = caps;
    }

    pub(crate) fn take_stdout(&mut self) -> String {
        std::mem::take(&mut self.out)
    }

    pub(crate) fn run_program(&mut self, args: &[String]) -> R<Option<Value>> {
        let v = self.run_program_than(args)?;
        // **ST-60 (VD-90/VD-95)**: join mọi tác vụ **theo thứ tự tạo** — chỉ khi main **thành công**
        // (`?` ở trên đã chặn đường lỗi).
        crate::rt::join_all(self)?;
        Ok(v)
    }

    fn run_program_than(&mut self, args: &[String]) -> R<Option<Value>> {
        // Hằng cấp tệp trước (mọi khai báo item là "hoisted").
        let consts: Vec<(u32, &'a HExpr)> = self.consts.iter().map(|(k, v)| (*k, *v)).collect();
        for (sym, e) in consts {
            let v = self.eval(e)?;
            self.env.borrow_mut().insert(sym, v);
        }
        if let Some(f) = self.main {
            let argv = Value::List(args.iter().map(|s| Value::Str(s.clone())).collect());
            let v = if f.params.is_empty() {
                self.call_hfn(f, Vec::new(), Vec::new(), f.span)?
            } else if self.caps.is_empty() {
                // Hành vi cũ: `main` có tham số không `cap` ⇒ nhận **list args** ở tham số 1.
                self.call_hfn(f, vec![argv], Vec::new(), f.span)?
            } else {
                // **ST-72b**: đặt `Value::Cap` đúng **chỉ số tham số**; tham số khác giữ `argv`
                // (tương thích đường cũ). `caps` do CLI tính từ CST `main`.
                let mut gt: Vec<Value> = Vec::with_capacity(f.params.len());
                for i in 0..f.params.len() {
                    match self.caps.iter().find(|(pos, _)| *pos as usize == i) {
                        Some((_, v)) => gt.push(v.clone()),
                        None => gt.push(argv.clone()),
                    }
                }
                self.call_hfn(f, gt, Vec::new(), f.span)?
            };
            return Ok(Some(v));
        }
        let n = self.stmts.len();
        let mut last = None;
        for (i, s) in self.stmts.iter().enumerate() {
            self.last = None;
            match self.exec_stmt(s)? {
                Ctrl::Normal => {
                    if i + 1 == n {
                        last = self.last.take();
                    }
                }
                Ctrl::Break | Ctrl::Continue => {
                    return Err(Abort::Trap(Trap::new(
                        RT,
                        "`break`/`continue` ngoài vòng lặp",
                        s.span(),
                    )))
                }
            }
        }
        Ok(last)
    }

    // ---------------------------------------------------------------------
    // Câu lệnh
    // ---------------------------------------------------------------------

    fn exec_block(&mut self, body: &[HStmt]) -> R<Ctrl> {
        for s in body {
            // VD-47: giá trị của khối = giá trị câu lệnh **cuối** (`Expr`/`if`/`match`/khối con).
            self.last = None;
            match self.exec_stmt(s)? {
                Ctrl::Normal => {}
                c => return Ok(c),
            }
        }
        Ok(Ctrl::Normal)
    }

    /// Chạy thân lấy **giá trị**: biểu thức cuối cùng (nếu có), còn lại `()`.
    fn exec_body(&mut self, body: &[HStmt]) -> R<Value> {
        match self.exec_block(body)? {
            Ctrl::Normal => Ok(self.last.take().unwrap_or(Value::Unit)),
            _ => Ok(Value::Unit),
        }
    }

    fn exec_stmt(&mut self, s: &HStmt) -> R<Ctrl> {
        self.steps += 1;
        match s {
            HStmt::Let { pat, init, .. } => {
                let v = self.eval(init)?;
                self.bind_pat(pat, &v);
                Ok(Ctrl::Normal)
            }
            HStmt::Var { sym, init, .. } => {
                let v = self.eval(init)?;
                self.env.borrow_mut().insert(*sym, v);
                self.mutable.insert(*sym);
                Ok(Ctrl::Normal)
            }
            HStmt::Assign {
                target,
                op,
                value,
                span,
            } => {
                // **ST-56b (VD-97)**: gán **trường/phần tử** của biến `var` — **cùng** ngữ nghĩa
                // codegen (xem `gan_truong`/`gan_chi_so`): thứ tự đánh giá **giống** codegen
                // (nạp ô → chỉ số → giá trị) và kết quả **ghi lại ô** (giá trị, không aliasing).
                // Chỉ `=`; gán ghép trên trường/phần tử đã bị tyck chặn trước.
                if matches!(op, AssignOp::Set) {
                    match target {
                        HExpr::Field { base, name, .. } => {
                            if let HExpr::Var { sym: Some(id), .. } = &**base {
                                let cu = self.doc_o(*id, target.span())?;
                                let rhs = self.eval(value)?;
                                let moi = gan_truong(cu, name, rhs, *span)?;
                                self.ghi_o(*id, moi, *span)?;
                                return Ok(Ctrl::Normal);
                            }
                        }
                        HExpr::Index { base, index, .. } => {
                            if let HExpr::Var { sym: Some(id), .. } = &**base {
                                let cu = self.doc_o(*id, target.span())?;
                                let i = self.eval(index)?;
                                let rhs = self.eval(value)?;
                                let moi = gan_chi_so(cu, i, rhs, *span)?;
                                self.ghi_o(*id, moi, *span)?;
                                return Ok(Ctrl::Normal);
                            }
                            // **ST-60b**: `p.xs[0] = v` (một tầng trường, gốc `var`) — đọc trường,
                            // gán phần tử, rồi **ghi lại trường** rồi ghi lại biến: đúng chuỗi
                            // `p = p with xs: (p.xs[i] = v)` mà codegen phát (`Field`/`Set`/`With`).
                            if let HExpr::Field {
                                base: goc, name, ..
                            } = &**base
                            {
                                if let HExpr::Var { sym: Some(id), .. } = &**goc {
                                    let cu = self.doc_o(*id, target.span())?;
                                    let truong = self.field_of(cu, name, *span)?;
                                    let i = self.eval(index)?;
                                    let rhs = self.eval(value)?;
                                    let moi_truong = gan_chi_so(truong, i, rhs, *span)?;
                                    let cu2 = self.doc_o(*id, target.span())?;
                                    let moi = gan_truong(cu2, name, moi_truong, *span)?;
                                    self.ghi_o(*id, moi, *span)?;
                                    return Ok(Ctrl::Normal);
                                }
                            }
                        }
                        _ => {}
                    }
                }
                let cur = self.lvalue_get(target)?;
                let rhs = self.eval(value)?;
                let new = match op {
                    AssignOp::Set => rhs,
                    _ => {
                        let bop = match op {
                            AssignOp::Add => BinOp::Add,
                            AssignOp::Sub => BinOp::Sub,
                            AssignOp::Mul => BinOp::Mul,
                            AssignOp::Div => BinOp::Div,
                            _ => BinOp::Rem,
                        };
                        crate::builtin::bin(bop, cur, rhs, *span)?
                    }
                };
                self.lvalue_set(target, new, *span)?;
                Ok(Ctrl::Normal)
            }
            HStmt::Expr(e) => {
                let v = self.eval(e)?;
                self.last = Some(v);
                Ok(Ctrl::Normal)
            }
            HStmt::If {
                cond,
                then,
                elifs,
                else_,
                ..
            } => {
                // VD-47: `if` **có** `else` cho giá trị của nhánh đã chạy; **không** `else` ⇒ `()`.
                let has_else = matches!(else_, Some(b) if !b.is_empty());
                let mut ctrl = Ctrl::Normal;
                let mut ran = false;
                if self.eval_cond(cond)? {
                    ctrl = self.exec_block(then)?;
                    ran = true;
                }
                if !ran {
                    for (c, body) in elifs {
                        if self.eval_cond(c)? {
                            ctrl = self.exec_block(body)?;
                            ran = true;
                            break;
                        }
                    }
                }
                if !ran {
                    if let Some(b) = else_ {
                        if !b.is_empty() {
                            ctrl = self.exec_block(b)?;
                        }
                    }
                }
                if !has_else {
                    self.last = None;
                }
                Ok(ctrl)
            }
            HStmt::For {
                pat, iter, body, ..
            } => {
                let it = self.eval(iter)?;
                let items = self.iter_items(it, iter.span())?;
                self.loop_depth += 1;
                let mut ctrl = Ok(Ctrl::Normal);
                for v in items {
                    self.bind_pat(pat, &v);
                    match self.exec_block(body) {
                        Ok(Ctrl::Break) => break,
                        Ok(Ctrl::Normal) => {}
                        Ok(Ctrl::Continue) => {}
                        Err(e) => {
                            ctrl = Err(e);
                            break;
                        }
                    }
                }
                self.loop_depth -= 1;
                self.last = None;
                ctrl
            }
            HStmt::While { cond, body, .. } => {
                let mut guard = 0usize;
                self.loop_depth += 1;
                while self.eval(cond)?.truthy(cond.span())? {
                    guard += 1;
                    if guard > 10_000_000 {
                        return Err(Abort::Trap(Trap::new(
                            RT,
                            "vòng lặp quá lâu (>10^7 bước)",
                            s.span(),
                        )));
                    }
                    let c = self.exec_block(body)?;
                    if c == Ctrl::Break {
                        break;
                    }
                }
                self.loop_depth -= 1;
                self.last = None;
                Ok(Ctrl::Normal)
            }
            HStmt::Loop { body, .. } => {
                self.loop_depth += 1;
                for _ in 0..10_000_000usize {
                    if self.exec_block(body)? == Ctrl::Break {
                        self.loop_depth -= 1;
                        self.last = None;
                        return Ok(Ctrl::Normal);
                    }
                }
                self.loop_depth -= 1;
                Err(Abort::Trap(Trap::new(
                    RT,
                    "`loop` quá lâu (>10^7 bước)",
                    s.span(),
                )))
            }
            HStmt::Scope { body, .. } => self.exec_block(body),
            HStmt::Return { value, .. } => {
                let v = match value {
                    Some(e) => self.eval(e)?,
                    None => Value::Unit,
                };
                Err(Abort::Ret(v))
            }
            HStmt::Break { .. } => {
                if self.loop_depth == 0 {
                    return Err(Abort::Trap(Trap::new(
                        RT,
                        "`break` ngoài vòng lặp (VS0 chỉ dùng trong `for`/`while`/`loop`)",
                        s.span(),
                    )));
                }
                Ok(Ctrl::Break)
            }
            HStmt::Continue { .. } => {
                if self.loop_depth == 0 {
                    return Err(Abort::Trap(Trap::new(
                        RT,
                        "`continue` ngoài vòng lặp",
                        s.span(),
                    )));
                }
                Ok(Ctrl::Continue)
            }
            HStmt::Match { subject, arms, .. } => {
                let subj = self.eval(subject)?;
                let Some((arm, binds)) = self.pick_arm(arms, &subj)? else {
                    return Err(Abort::Trap(Trap::new(
                        RT,
                        "`match` không khớp nhánh nào (không xét hết bị lọt)",
                        s.span(),
                    )));
                };
                for (sym, v) in binds {
                    self.env.borrow_mut().insert(sym, v);
                }
                self.exec_block(&arm.body)
            }
            HStmt::Block { body, .. } => self.exec_block(body),
        }
    }

    fn eval_cond(&mut self, c: &HCond) -> R<bool> {
        match c {
            HCond::Expr(e) => {
                let v = self.eval(e)?;
                v.truthy(e.span())
            }
            HCond::Let { pat, value, .. } => {
                let v = self.eval(value)?;
                let target = match &v {
                    Value::Opt(Some(x)) => (**x).clone(),
                    Value::Opt(None) => return Ok(false),
                    other => other.clone(),
                };
                let snapshot = self.env.borrow().clone();
                if !self.bind_pat_bool(pat, &target) {
                    *self.env.borrow_mut() = snapshot;
                    return Ok(false);
                }
                Ok(true)
            }
        }
    }

    // ---------------------------------------------------------------------
    // Biểu thức
    // ---------------------------------------------------------------------

    fn eval(&mut self, e: &HExpr) -> R<Value> {
        self.steps += 1;
        match e {
            HExpr::Lit { lit, span, .. } => {
                lit_value(lit).map_err(|m| Abort::Trap(Trap::new(RT, m, *span)))
            }
            HExpr::Var { name, sym, span } => match sym {
                Some(id) => {
                    if let Some(v) = self.env.borrow().get(id).cloned() {
                        return Ok(v);
                    }
                    if let Some(f) = self.fns.get(id) {
                        return Ok(Value::Fn(Rc::new(Closure {
                            name: f.name.clone(),
                            params: f.params.iter().map(|p| p.sym).collect(),
                            body: HBody::Stmts(f.body.clone()),
                            env: self.env.clone(),
                        })));
                    }
                    // `enum`/`type` khai trong tệp: thẻ kiểu để `E.A` / `User(...)` chạy được.
                    if let Some((n, is_enum)) = self.tags.get(id).cloned() {
                        return Ok(if is_enum {
                            Value::Enum(n, String::new(), Vec::new())
                        } else {
                            Value::Record(n, Vec::new())
                        });
                    }
                    // `print`/`len`/`str`/`Ok`/... **có** `sym` (ST-11 khai 8 tên dựng sẵn) ⇒ tra tên.
                    if self.symbols.kind_of(*id) == Some(vis_types::lower::SymKind::Builtin) {
                        let n = self.symbols.name_of(*id).unwrap_or_default();
                        if let Some(b) = builtin_name(n) {
                            return Ok(Value::Builtin(b));
                        }
                    }
                    Err(Abort::Trap(Trap::new(
                        RT,
                        format!("tên `{name}` chưa có giá trị lúc chạy"),
                        *span,
                    )))
                }
                None => match builtin_name(name) {
                    Some(b) => Ok(Value::Builtin(b)),
                    None => Err(Abort::Trap(Trap::new(
                        RT,
                        format!("tên `{name}` chưa hỗ trợ ở VS0"),
                        *span,
                    ))),
                },
            },
            HExpr::Bin { op, lhs, rhs, span } => match op {
                BinOp::Coalesce => {
                    let l = self.eval(lhs)?;
                    match l {
                        Value::Opt(Some(v)) => Ok(*v),
                        Value::Opt(None) => self.eval(rhs),
                        other => Ok(other),
                    }
                }
                BinOp::And => {
                    let l = self.eval(lhs)?;
                    if l.truthy(lhs.span())? {
                        let r = self.eval(rhs)?;
                        Ok(Value::Bool(r.truthy(rhs.span())?))
                    } else {
                        Ok(Value::Bool(false))
                    }
                }
                BinOp::Or => {
                    let l = self.eval(lhs)?;
                    if l.truthy(lhs.span())? {
                        Ok(Value::Bool(true))
                    } else {
                        let r = self.eval(rhs)?;
                        Ok(Value::Bool(r.truthy(rhs.span())?))
                    }
                }
                BinOp::Pipe => {
                    let l = self.eval(lhs)?;
                    let f = self.eval(rhs)?;
                    self.call_value(f, vec![l], Vec::new(), *span)
                }
                _ => {
                    let l = self.eval(lhs)?;
                    let r = self.eval(rhs)?;
                    crate::builtin::bin(*op, l, r, *span)
                }
            },
            HExpr::Un { op, expr, span } => {
                let v = self.eval(expr)?;
                crate::builtin::un(*op, v, *span)
            }
            HExpr::Call {
                callee,
                args,
                named,
                span,
            } => self.eval_call(callee, args, named, *span),
            HExpr::Field {
                base,
                name,
                optional,
                span,
            } => {
                let b = self.eval(base)?;
                let b = if *optional {
                    match b {
                        Value::Opt(None) => return Ok(Value::Opt(None)),
                        Value::Opt(Some(x)) => *x,
                        other => other,
                    }
                } else {
                    b
                };
                self.field_of(b, name, *span)
            }
            HExpr::Index { base, index, span } => {
                let b = self.eval(base)?;
                let i = self.eval(index)?;
                self.index_of(b, i, *span)
            }
            HExpr::List { items, .. } => {
                let mut vs = Vec::with_capacity(items.len());
                for it in items {
                    vs.push(self.eval(it)?);
                }
                Ok(Value::List(vs))
            }
            HExpr::Set { items, .. } => {
                let mut vs: Vec<Value> = Vec::new();
                for it in items {
                    let v = self.eval(it)?;
                    if !vs.iter().any(|x| x.eq_value(&v)) {
                        vs.push(v);
                    }
                }
                Ok(Value::List(vs))
            }
            HExpr::Map { entries, .. } => {
                let mut kv = Vec::new();
                for (k, v) in entries {
                    let kk = self.eval(k)?;
                    let vv = self.eval(v)?;
                    kv.push((kk, vv));
                }
                Ok(Value::Map(kv))
            }
            HExpr::Record { fields, ty, span } => {
                let name = self.type_name(ty);
                let mut out = Vec::with_capacity(fields.len());
                for (n, v) in fields {
                    out.push((n.clone(), self.eval(v)?));
                    let _ = span;
                }
                Ok(Value::Record(name, out))
            }
            HExpr::EnumLit {
                path,
                variant,
                args,
                ty,
                span,
                ..
            } => {
                let mut vs = Vec::with_capacity(args.len());
                for a in args {
                    vs.push(self.eval(a)?);
                }
                let name = self.type_name(ty);
                let name = if name.is_empty() {
                    path.first().cloned().unwrap_or_default()
                } else {
                    name
                };
                let _ = span;
                Ok(Value::Enum(name, variant.clone(), vs))
            }
            HExpr::Opt { value, .. } => match value {
                None => Ok(Value::Opt(None)),
                Some(v) => {
                    let inner = self.eval(v)?;
                    Ok(Value::Opt(Some(Box::new(inner))))
                }
            },
            HExpr::Try { expr, .. } if self.fn_depth == 0 => {
                // ST-13b (mục 8): `?` ngoài hàm là lỗi (§4.4) — trước đây bị nuốt thành `return`.
                Err(Abort::Trap(Trap::new(
                    RT,
                    "`?` chỉ dùng **trong hàm** (§4.4)",
                    e.span(),
                )))
            }
            HExpr::Try { expr, .. } => match self.eval(expr)? {
                Value::Opt(Some(v)) => Ok(*v),
                Value::Opt(None) => Err(Abort::Ret(Value::Opt(None))),
                Value::Res(Ok(v)) => Ok(*v),
                Value::Res(Err(e)) => Err(Abort::Ret(Value::Res(Err(e)))),
                other => Err(Abort::Trap(Trap::new(
                    RT,
                    format!("`?` cần `T?`/`Result`, được `{}`", other.show()),
                    e.span(),
                ))),
            },
            HExpr::With { base, fields, span } => {
                let b = self.eval(base)?;
                match b {
                    Value::Record(name, mut cur) => {
                        for (n, v) in fields {
                            let nv = self.eval(v)?;
                            match cur.iter_mut().find(|(f, _)| f == n) {
                                Some(slot) => slot.1 = nv,
                                None => {
                                    return Err(Abort::Trap(Trap::new(
                                        RT,
                                        format!("bản ghi không có trường `{n}`"),
                                        *span,
                                    )))
                                }
                            }
                        }
                        Ok(Value::Record(name, cur))
                    }
                    other => Err(Abort::Trap(Trap::new(
                        RT,
                        format!("`with` cần bản ghi, được `{}`", other.show()),
                        *span,
                    ))),
                }
            }
            HExpr::If {
                cond,
                then,
                elifs,
                else_,
                ..
            } => {
                if self.eval_cond(cond)? {
                    return self.exec_body(then);
                }
                for (c, body) in elifs {
                    if self.eval_cond(c)? {
                        return self.exec_body(body);
                    }
                }
                self.exec_body(else_)
            }
            HExpr::Match {
                subject,
                arms,
                span,
                ..
            } => {
                let subj = self.eval(subject)?;
                match self.pick_arm(arms, &subj)? {
                    Some((arm, binds)) => {
                        for (sym, v) in binds {
                            self.env.borrow_mut().insert(sym, v);
                        }
                        self.exec_body(&arm.body)
                    }
                    None => Err(Abort::Trap(Trap::new(
                        RT,
                        "`match` không khớp nhánh nào (không xét hết bị lọt)",
                        *span,
                    ))),
                }
            }
            HExpr::Lambda { params, body, .. } => Ok(Value::Fn(Rc::new(Closure {
                name: String::new(),
                params: params.iter().map(|p| p.sym).collect(),
                body: body.clone(),
                // ST-15b (mục 2): **chụp snapshot** môi trường tại điểm tạo closure — interp dùng
                // một env phẳng và khôi phục tham số khi hàm trả về, nên nếu giữ `Rc` chung thì
                // closure thoát khỏi hàm sẽ mất biến đã bắt. Chọn snapshot (ghi JOURNAL): biến
                // `var` cập nhật **sau** khi tạo closure thì closure vẫn thấy giá trị lúc tạo.
                env: Rc::new(std::cell::RefCell::new(self.env.borrow().clone())),
            }))),
            HExpr::Tuple { items, .. } => {
                let mut vs = Vec::with_capacity(items.len());
                for it in items {
                    vs.push(self.eval(it)?);
                }
                Ok(Value::List(vs))
            }
            // **ST-61 (VD-96)**: `chan[T](n)` ⇒ `$kenh(n)` (một hiện thực ở `builtin`, dùng chung
            // cho cả interp lẫn VM).
            HExpr::ChanNew { cap, span, .. } => {
                let c = self.eval(cap)?;
                crate::builtin::call(self, "$kenh", vec![c], *span)
            }
            // **ST-56 (VD-92)**: `ref(expr)` giữ **nguyên kiểu trong**, hạ **identity** — **không**
            // tạo giá trị mới, **không** mượn thật (borrow/vòng đời là nợ **VS2+**).
            HExpr::Ref { expr, .. } => self.eval(expr),
            // **ST-60 (VD-90)**: `await t` chạy tác vụ **tới hoàn tất** (hoặc trả cache).
            HExpr::Await { expr, span } => {
                let v = self.eval(expr)?;
                match v {
                    Value::Task(t) => crate::rt::await_task(self, &t, *span),
                    _ => Err(Abort::Trap(Trap::new(
                        RT,
                        "`await` cần một `Task[T]`",
                        *span,
                    ))),
                }
            }
            // **ST-60**: `spawn f(args)` = tạo tác vụ + đưa vào hàng đợi FIFO.
            HExpr::Spawn {
                callee,
                args,
                named,
                span,
            } => {
                let _ = named;
                let id = match &**callee {
                    HExpr::Var { sym: Some(id), .. } => Some(*id),
                    _ => None,
                };
                let Some(id) = id else {
                    return Err(Abort::Trap(Trap::new(
                        RT,
                        "`spawn` cần **tên** hàm `async` cấp tệp",
                        *span,
                    )));
                };
                let Some(f) = self.fns.get(&id).copied() else {
                    return Err(Abort::Trap(Trap::new(
                        RT,
                        "`spawn` cần **tên** hàm `async` cấp tệp",
                        *span,
                    )));
                };
                let mut pos = Vec::with_capacity(args.len());
                for a in args {
                    pos.push(self.eval(a)?);
                }
                let t = self
                    .sched
                    .get_mut()
                    .spawn(f.sym, pos, f.name.clone(), *span);
                Ok(Value::Task(t))
            }
            // ST-14: chuỗi nội suy — đánh giá từng mảnh rồi ghép (không parse lại lúc chạy).
            HExpr::Interp { parts, .. } => {
                let mut out = String::new();
                for p in parts {
                    match p {
                        vis_types::InterpPart::Str(t) => out.push_str(t),
                        vis_types::InterpPart::Expr(e) => {
                            let v = self.eval(e)?;
                            out.push_str(&v.show());
                        }
                    }
                }
                Ok(Value::Str(out))
            }
            HExpr::CmdBlock { span, .. } => Err(Abort::Trap(Trap::new(
                RT,
                "khối `cmd` là VS1 — chưa hỗ trợ ở VS0",
                *span,
            ))),
            HExpr::Error { span } => Err(Abort::Trap(Trap::new(
                RT,
                "biểu thức không hạ được (HIR `error`)",
                *span,
            ))),
        }
    }

    fn type_name(&self, ty: &vis_types::Type) -> String {
        match ty {
            vis_types::Type::Record(id) => self
                .types
                .rec(*id)
                .and_then(|r| r.name.clone())
                .unwrap_or_default(),
            vis_types::Type::Enum(id) => self
                .types
                .enum_def(*id)
                .map(|e| e.name.clone())
                .unwrap_or_default(),
            _ => String::new(),
        }
    }

    // ---------------------------------------------------------------------
    // Lấy/đặt giá trị (lvalue), trường, chỉ số, vòng lặp
    // ---------------------------------------------------------------------

    /// **ST-56b** — nạp **giá trị ô** của biến (dùng cho gán trường/phần tử; đúng thứ tự codegen).
    fn doc_o(&mut self, id: u32, span: Span) -> R<Value> {
        self.env
            .borrow()
            .get(&id)
            .cloned()
            .ok_or_else(|| Abort::Trap(Trap::new(RT, "tên chưa có giá trị", span)))
    }

    /// **ST-56b** — ghi lại **ô** của biến; chỉ `var`/`var self` (tyck đã chặn trước, đây là chốt 2).
    fn ghi_o(&mut self, id: u32, v: Value, span: Span) -> R<()> {
        if !self.mutable.contains(&id) {
            return Err(Abort::Trap(Trap::new(
                MISMATCH,
                "không gán được trường của `let`/tham số — chỉ `var` mới gán được (§4.1)",
                span,
            )));
        }
        self.env.borrow_mut().insert(id, v);
        Ok(())
    }

    fn lvalue_get(&mut self, target: &HExpr) -> R<Value> {
        match target {
            HExpr::Var { sym: Some(id), .. } => self
                .env
                .borrow()
                .get(id)
                .cloned()
                .ok_or_else(|| Abort::Trap(Trap::new(RT, "tên chưa có giá trị", target.span()))),
            HExpr::Var { span, .. } => Err(Abort::Trap(Trap::new(
                RT,
                "gán cho tên chưa phân giải được",
                *span,
            ))),
            other => self.eval(other),
        }
    }

    fn lvalue_set(&mut self, target: &HExpr, v: Value, span: Span) -> R<()> {
        match target {
            HExpr::Var { sym: Some(id), .. } => {
                if !self.mutable.contains(id) {
                    return Err(Abort::Trap(Trap::new(
                        MISMATCH,
                        "không gán được cho `let`/tham số (chỉ `var` mới gán được, §4.1)",
                        span,
                    )));
                }
                self.env.borrow_mut().insert(*id, v);
                Ok(())
            }
            HExpr::Field { .. } | HExpr::Index { .. } => Err(Abort::Trap(Trap::new(
                RT,
                "gán vào trường/phần tử chưa hỗ trợ ở VS0 (dùng `with` cho bản ghi)",
                span,
            ))),
            _ => Err(Abort::Trap(Trap::new(RT, "đích gán không hợp lệ", span))),
        }
    }

    fn field_of(&mut self, b: Value, name: &str, span: Span) -> R<Value> {
        match b {
            Value::Record(_, fields) => fields
                .into_iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| v)
                .ok_or_else(|| {
                    Abort::Trap(Trap::new(
                        RT,
                        format!("bản ghi không có trường `{name}`"),
                        span,
                    ))
                }),
            Value::Enum(en, variant, args) => {
                if variant == name {
                    Ok(Value::Enum(en, variant, args))
                } else if args.is_empty() {
                    // `Shape.Empty` — biến thể không đối số.
                    Ok(Value::Enum(en, name.to_string(), Vec::new()))
                } else {
                    Err(Abort::Trap(Trap::new(
                        RT,
                        format!("enum không có biến thể `{name}`"),
                        span,
                    )))
                }
            }
            Value::Opt(Some(x)) => self.field_of(*x, name, span),
            other => Err(Abort::Trap(Trap::new(
                RT,
                format!("`{}` không có trường `{name}`", other.show()),
                span,
            ))),
        }
    }

    fn index_of(&mut self, b: Value, i: Value, span: Span) -> R<Value> {
        match (b, i) {
            (Value::List(xs), Value::Int(n)) => {
                let len = xs.len() as i64;
                if n < 0 {
                    return Err(Abort::Trap(Trap::new(
                        IDX,
                        format!("chỉ số âm `{n}` (dùng `.last()` hoặc `xs[i]?`)"),
                        span,
                    )));
                }
                xs.get(n as usize).cloned().ok_or_else(|| {
                    Abort::Trap(Trap::new(
                        IDX,
                        format!("chỉ số {n} ngoài phạm vi (độ dài {len})"),
                        span,
                    ))
                })
            }
            (Value::Map(kv), k) => kv
                .iter()
                .find(|(kk, _)| kk.eq_value(&k))
                .map(|(_, v)| v.clone())
                .ok_or_else(|| {
                    Abort::Trap(Trap::new(
                        IDX,
                        format!("khoá `{}` không có trong map", k.show()),
                        span,
                    ))
                }),
            (Value::Bytes(bs), Value::Int(n)) => {
                let len = bs.len() as i64;
                if n < 0 || n >= len {
                    return Err(Abort::Trap(Trap::new(
                        IDX,
                        format!("chỉ số {n} ngoài phạm vi (độ dài {len})"),
                        span,
                    )));
                }
                Ok(Value::Int(bs[n as usize] as i64))
            }
            (other, idx) => Err(Abort::Trap(Trap::new(
                RT,
                format!("không chỉ số được `{}` bằng `{}`", other.show(), idx.show()),
                span,
            ))),
        }
    }

    fn iter_items(&mut self, it: Value, span: Span) -> R<Vec<Value>> {
        crate::value::iter_items(it, span)
    }

    /// Chọn nhánh `match`: thử pattern rồi guard; trả binding của nhánh khớp.
    fn pick_arm<'s>(&mut self, arms: &'s [HArm], subj: &Value) -> R<Option<Picked<'s>>> {
        for arm in arms {
            let snapshot = self.env.borrow().clone();
            if !self.bind_pat_bool(&arm.pat, subj) {
                *self.env.borrow_mut() = snapshot;
                continue;
            }
            if let Some(g) = &arm.guard {
                let gv = self.eval(g)?;
                if !gv.truthy(g.span())? {
                    *self.env.borrow_mut() = snapshot;
                    continue;
                }
            }
            // Binding của nhánh: các sym vừa thêm/đổi so với snapshot.
            let now = self.env.borrow().clone();
            let mut binds = Vec::new();
            for (k, v) in now {
                binds.push((k, v));
            }
            *self.env.borrow_mut() = snapshot;
            return Ok(Some((arm, binds)));
        }
        Ok(None)
    }

    // ---------------------------------------------------------------------
    // Mã lỗi chung
    // ---------------------------------------------------------------------
}

/// Đọc literal thành `Value` (lỗi → thông điệp cho `VS0501`).
pub fn lit_value(lit: &Lit) -> std::result::Result<Value, String> {
    match lit.kind {
        LitKind::Int => parse_int_lit(&lit.text)
            .map(Value::Int)
            .ok_or_else(|| format!("literal int không đọc được: `{}`", lit.text)),
        LitKind::Float => lit
            .text
            .replace('_', "")
            .parse::<f64>()
            .map(Value::Float)
            .map_err(|_| format!("literal float không đọc được: `{}`", lit.text)),
        LitKind::Bool => match lit.text.as_str() {
            "true" => Ok(Value::Bool(true)),
            "false" => Ok(Value::Bool(false)),
            other => Err(format!("literal bool lạ: `{other}`")),
        },
        LitKind::Char => parse_char_lit(&lit.text)
            .map(Value::Char)
            .ok_or_else(|| format!("literal char không đọc được: `{}`", lit.text)),
        LitKind::Str => Ok(Value::Str(parse_str_lit(&lit.text, false))),
        LitKind::Bytes => parse_bytes_lit(&lit.text)
            .map(Value::Bytes)
            .ok_or_else(|| format!("literal bytes không đọc được: `{}`", lit.text)),
        LitKind::Path => Ok(Value::Path(parse_str_lit(&lit.text, false))),
        LitKind::Size => parse_size(&lit.text)
            .map(Value::Size)
            .ok_or_else(|| format!("literal size không đọc được: `{}`", lit.text)),
        LitKind::Duration => parse_duration(&lit.text)
            .map(Value::Duration)
            .ok_or_else(|| format!("literal duration không đọc được: `{}`", lit.text)),
        LitKind::Percent => parse_percent(&lit.text)
            .map(Value::Percent)
            .ok_or_else(|| format!("literal percent không đọc được: `{}`", lit.text)),
    }
}

fn parse_int_lit(text: &str) -> Option<i64> {
    let t = text.replace('_', "");
    if let Some(h) = t.strip_prefix("0x") {
        return i64::from_str_radix(h, 16).ok();
    }
    if let Some(b) = t.strip_prefix("0b") {
        return i64::from_str_radix(b, 2).ok();
    }
    if let Some(o) = t.strip_prefix("0o") {
        return i64::from_str_radix(o, 8).ok();
    }
    t.parse().ok()
}

/// Vị trí `dòng:cột` (cột đếm theo **Unicode scalar**, bắt đầu từ 1 — §2.4) từ `span` + văn bản.
pub fn pos(text: &str, span: Span) -> (u32, u32) {
    let mut line = 1u32;
    let mut col = 1u32;
    for (i, c) in text.char_indices() {
        if i >= span.start {
            break;
        }
        match c {
            '\n' => {
                line += 1;
                col = 1;
            }
            // ST-13b (mục 7): tab = **4 cột** (SPEC/VD-12), không phải 1.
            '\t' => col += 4,
            _ => col += 1,
        }
    }
    (line, col)
}

/// 8 tên dựng sẵn (VD-44 #3) mà bộ thông dịch gọi trực tiếp.
/// Cap **độ sâu khung gọi** (ST-15): **2.000** khung (đo 10^4 tràn stack cả trên luồng 256 MiB) —
/// vượt ⇒ `VS0501` thay vì tràn stack process.
pub(crate) const MAX_CALL_DEPTH: u32 = 2_000;

fn builtin_name(name: &str) -> Option<&'static str> {
    // ST-27: chương trình ngoài — `builtin::call` sẽ hỏi **host** (`None` ⇒ `VS0604`).
    if name.starts_with("exec.") {
        return Some("exec");
    }
    // ST-26: `std.<module>.<fn>` gọi qua bảng `stdlib` (dùng chung với VM, VD-55).
    if name.starts_with("std.") {
        return name
            .strip_prefix("std.")
            .and_then(vis_types::stdlib::sig_of)
            .map(|f| f.name);
    }
    match name {
        "print" => Some("print"),
        "assert" => Some("assert"),
        "len" => Some("len"),
        "str" => Some("str"),
        "Ok" => Some("Ok"),
        "Err" => Some("Err"),
        "Some" => Some("Some"),
        "None" => Some("None"),
        "ref" => Some("ref"),
        // ST-18: 3 lệnh builtin của chế độ lệnh (VS0) — chữ ký ở `vis-types/lower/cmd.rs`.
        "echo" => Some("echo"),
        "lines" => Some("lines"),
        "take" => Some("take"),
        // ST-38: closure-arg của chế độ lệnh.
        "sort_by" => Some("sort_by"),
        "render" => Some("render"),
        "ls" => Some("ls"),
        "open" => Some("open"),
        "get" => Some("get"),
        "select" => Some("select"),
        "where" => Some("where"),
        "each" => Some("each"),
        "first" => Some("first"),
        "count" => Some("count"),
        _ => None,
    }
}

impl Value {
    /// Chân trị cho `if`/`while`/`and`/`or`/`not`/guard.
    pub fn truthy(&self, span: Span) -> R<bool> {
        match self {
            Value::Bool(b) => Ok(*b),
            Value::Opt(None) => Ok(false),
            Value::Opt(Some(_)) => Ok(true),
            other => Err(Abort::Trap(Trap::new(
                RT,
                format!("điều kiện cần `bool`, được `{}`", other.show()),
                span,
            ))),
        }
    }
}
