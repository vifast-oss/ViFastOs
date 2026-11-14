// SPDX-License-Identifier: MIT OR Apache-2.0
//! Vòng lặp **fetch–decode–execute** trên [`Module`] (ST-20).
//!
//! Ngăn xếp giá trị **động** (`Vec<Value>`), không dựa vào `max_stack` (VD-54 (2)); ngăn xếp
//! khung tách riêng để `return` lồng nhau sạch; iterator của `for` nằm ở `iters` (không lẫn với
//! ngăn xếp giá trị).

use vis_bytecode::{Func, Instr, Module};
use vis_interp::builtin::{self, Host};
use vis_interp::interp::{Abort, Trap, IDX, R, RT};
use vis_interp::value::{self, Value, VmFunc};
use vis_interp::RunOutput;
use vis_syntax::Span;
use vis_types::hir::LitKind;

/// Chạy một module VBC1 (đường tắt tiện dụng).
pub fn run(m: &Module, text: &str, args: &[String]) -> RunOutput {
    Vm::new(m, text).run(args)
}

/// Như [`run`] nhưng gắn **host hệ thống tệp** (ST-33). `None` ⇒ `std.fs.*` ⇒ `VS0604`.
pub fn run_with_fs<'a>(
    m: &'a Module,
    text: &'a str,
    args: &[String],
    fs: Option<&'a mut dyn vis_interp::builtin::FsHost>,
) -> RunOutput {
    run_with_hosts(m, text, args, fs, None)
}

/// **ST-63 (VD-105)**: như [`run_with_fs`] nhưng gắn **cả host mạng** (`std.net.*`); `None` ở host
/// nào ⇒ `VS0604`. Đây là cửa CLI dùng để hai đường (interp/VM) nhận **cùng** host.
pub fn run_with_hosts<'a>(
    m: &'a Module,
    text: &'a str,
    args: &[String],
    fs: Option<&'a mut dyn vis_interp::builtin::FsHost>,
    net: Option<&'a mut dyn vis_interp::builtin::NetHost>,
) -> RunOutput {
    let mut vm = Vm::new(m, text);
    if let Some(f) = fs {
        vm.set_fs(f);
    }
    if let Some(g) = net {
        vm.set_net(g);
    }
    vm.run(args)
}

/// Cap số khung gọi lồng nhau (VD-49).
pub const MAX_FRAMES: usize = 2_000;
/// Cap số **lệnh** đã thực thi — cùng ngưỡng 10^7 với interp (khác đơn vị đếm).
pub const MAX_STEPS: usize = 10_000_000;

/// Lỗi của VM khi bytecode hỏng — **không** panic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VmError {
    /// `pop` trên ngăn xếp rỗng.
    StackUnderflow,
    /// Đích nhảy ngoài `code`.
    BadJump(i32),
    /// Chỉ số const/hàm ngoài phạm vi.
    BadIdx(u32),
}

impl core::fmt::Display for VmError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            VmError::StackUnderflow => write!(f, "ngăn xếp rỗng"),
            VmError::BadJump(d) => write!(f, "đích nhảy ngoài code: {d}"),
            VmError::BadIdx(i) => write!(f, "chỉ số ngoài phạm vi: {i}"),
        }
    }
}

struct Frame {
    fi: u32,
    pc: usize,
    locals: Vec<Value>,
    captured: Vec<Value>,
    /// Ngăn xếp **của khung gọi** được cất khi gọi hàm (mỗi khung một ngăn xếp riêng).
    st_cu: Vec<Value>,
}

/// Máy ảo: giữ module + văn bản nguồn + `stdout` + số bước.
pub struct Vm<'a> {
    m: &'a Module,
    text: &'a str,
    out: String,
    steps: usize,
    depth: usize,
    /// Độ sâu ngăn xếp giá trị **lớn nhất đã dùng** (bất biến của fuzz ST-21).
    dinh: usize,
    /// Host hệ thống tệp (ST-33); `None` ⇒ `VS0604` — VM **không** biết `std::fs`.
    fs: Option<&'a mut dyn vis_interp::builtin::FsHost>,
    /// **ST-63 (VD-105)**: host mạng; `None` ⇒ `VS0604` — VM **không** biết `std::net`.
    net: Option<&'a mut dyn vis_interp::builtin::NetHost>,
    /// **ST-60**: hàng đợi tác vụ `async` — cùng runtime với interp (`vis_interp::rt`).
    sched: std::cell::RefCell<vis_interp::rt::Scheduler>,
}

impl<'a> Vm<'a> {
    /// VM cho một module + văn bản nguồn (để đổi span byte → `dòng:cột`).
    pub fn new(m: &'a Module, text: &'a str) -> Self {
        Self {
            m,
            text,
            out: String::new(),
            steps: 0,
            depth: 0,
            dinh: 0,
            fs: None,
            net: None,
            sched: std::cell::RefCell::new(vis_interp::rt::Scheduler::new()),
        }
    }

    /// Độ sâu ngăn xếp **lớn nhất** đã dùng trong lần chạy.
    pub fn max_stack_used(&self) -> usize {
        self.dinh
    }

    /// Lấy `stdout` đã in.
    pub fn take_stdout(&mut self) -> String {
        std::mem::take(&mut self.out)
    }

    /// Chạy `main` của module.
    pub fn run(&mut self, args: &[String]) -> RunOutput {
        // **Bắt buộc** xác minh trước khi chạy (VD-52 #3): bytecode có thể đến từ cache.
        if let Err(e) = vis_bytecode::verify(self.m) {
            return RunOutput {
                stdout: String::new(),
                diagnostics: vec![vis_syntax::Diagnostic {
                    code: &vis_diag::RUNTIME_CODES[0],
                    message: format!("bytecode hỏng: {e}"),
                    span: Span::new(0, 0),
                    line: 1,
                    col: 1,
                }],
                value: None,
                exit: 1,
                steps: 0,
            };
        }
        let argv = Value::List(args.iter().map(|s| Value::Str(s.clone())).collect());
        // **ST-60 (VD-90/VD-95)**: join mọi tác vụ **theo thứ tự tạo**, **chỉ khi main thành công**
        // (và **trước** `take_stdout` ⇒ thân tác vụ in vẫn vào stdout, đúng thứ tự).
        let kq = match self.chay_chuong_trinh(&argv) {
            Ok(v) => match vis_interp::rt::join_all(self) {
                Ok(()) => Ok(v),
                Err(e) => Err(e),
            },
            Err(e) => Err(e),
        };
        match kq {
            Ok(v) => {
                let exit = value::exit_code(&v);
                RunOutput {
                    stdout: self.take_stdout(),
                    diagnostics: Vec::new(),
                    value: Some(v),
                    exit,
                    steps: self.steps,
                }
            }
            Err(Abort::Trap(t)) => RunOutput {
                stdout: self.take_stdout(),
                diagnostics: vec![t.to_diag(self.text)],
                value: None,
                exit: 1,
                steps: self.steps,
            },
            Err(Abort::Ret(v)) => RunOutput {
                stdout: self.take_stdout(),
                diagnostics: Vec::new(),
                value: Some(v),
                exit: 0,
                steps: self.steps,
            },
        }
    }

    fn chay_chuong_trinh(&mut self, argv: &Value) -> R<Value> {
        let entry = self.m.entry;
        let f = self.func(entry)?;
        let mut locals = vec![Value::Unit; f.nlocals.max(1) as usize];
        if f.nparams > 0 && f.arity == 0 {
            if let Some(l) = locals.first_mut() {
                *l = argv.clone();
            }
        }
        let mut th = Th::new(self.m, entry, locals, Vec::new());
        let v = th.exec(self)?;
        Ok(v)
    }

    fn func(&self, i: u32) -> R<&Func> {
        self.m
            .funcs
            .get(i as usize)
            .ok_or_else(|| self.hong(VmError::BadIdx(i), Span::new(0, 0)))
    }

    fn hong(&self, e: VmError, span: Span) -> Abort {
        Abort::Trap(Trap::new(RT, format!("bytecode hỏng: {e}"), span))
    }

    fn lit(&self, i: u32, span: Span) -> R<Value> {
        let c = self
            .m
            .consts
            .get(i as usize)
            .ok_or_else(|| self.hong(VmError::BadIdx(i), span))?;
        let kind = lit_kind(c.kind).ok_or_else(|| self.hong(VmError::BadIdx(i), span))?;
        let lit = vis_types::hir::Lit {
            kind,
            text: c.text.clone(),
        };
        vis_interp::interp::lit_value(&lit)
            .map_err(|e| Abort::Trap(Trap::new(&vis_diag::TYPE_CODES[0], e, span)))
    }

    fn ten(&self, i: u32, span: Span) -> R<String> {
        let c = self
            .m
            .consts
            .get(i as usize)
            .ok_or_else(|| self.hong(VmError::BadIdx(i), span))?;
        Ok(match c.kind {
            5 => c.text.trim_matches('"').to_string(),
            _ => c.text.clone(),
        })
    }
}

impl<'a> Vm<'a> {
    /// Gắn host hệ thống tệp (ST-33) — chỉ dùng qua [`crate::run_with_fs`].
    pub fn set_fs(&mut self, f: &'a mut dyn vis_interp::builtin::FsHost) {
        self.fs = Some(f);
    }

    /// **ST-63 (VD-105)**: gắn host mạng — chỉ dùng qua [`crate::run_with_hosts`].
    pub fn set_net(&mut self, n: &'a mut dyn vis_interp::builtin::NetHost) {
        self.net = Some(n);
    }
}

impl Host for Vm<'_> {
    fn out(&mut self) -> &mut String {
        &mut self.out
    }

    fn fs(&mut self) -> Option<&mut dyn vis_interp::builtin::FsHost> {
        self.fs
            .as_mut()
            .map(|f| &mut **f as &mut dyn vis_interp::builtin::FsHost)
    }

    /// **ST-63 (VD-105)**: host mạng; `None` ⇒ `std.net.*` ⇒ `VS0604`.
    fn net(&mut self) -> Option<&mut dyn vis_interp::builtin::NetHost> {
        self.net
            .as_mut()
            .map(|n| &mut **n as &mut dyn vis_interp::builtin::NetHost)
    }

    fn sched(&self) -> &std::cell::RefCell<vis_interp::rt::Scheduler> {
        &self.sched
    }

    /// **ST-60**: handle của VM là **chỉ số hàm bytecode** — chạy bằng `goi_ham_bytecode`.
    fn goi_than(&mut self, fi: u32, args: Vec<Value>, span: Span) -> R<Value> {
        let f = VmFunc {
            idx: fi,
            captured: Vec::new(),
        };
        self.goi_ham_bytecode(&f, args, span)
    }

    fn ten_than(&self, fi: u32) -> String {
        self.m
            .funcs
            .get(fi as usize)
            .map(|f| f.name.clone())
            .unwrap_or_default()
    }

    fn call(&mut self, f: &Value, args: Vec<Value>, span: Span) -> R<Value> {
        match f {
            Value::Vm(v) => self.goi_ham_bytecode(v, args, span),
            Value::Builtin(n) => builtin::call(self, n, args, span),
            other => Err(Abort::Trap(Trap::new(
                RT,
                format!("`{}` không phải hàm", other.show()),
                span,
            ))),
        }
    }
}

impl Vm<'_> {
    fn goi_ham_bytecode(&mut self, f: &VmFunc, args: Vec<Value>, span: Span) -> R<Value> {
        if self.depth + 1 >= MAX_FRAMES {
            return Err(Abort::Trap(Trap::new(
                RT,
                format!("gọi hàm quá sâu (>{MAX_FRAMES} khung) — đệ quy không có điểm dừng?"),
                span,
            )));
        }
        let fc = self.func(f.idx)?;
        let mut locals = vec![Value::Unit; fc.nlocals.max(1) as usize];
        for (i, a) in args.into_iter().enumerate() {
            if i < locals.len() {
                locals[i] = a;
            }
        }
        let mut th = Th::new(self.m, f.idx, locals, f.captured.clone());
        self.depth += 1;
        let v = th.exec(self);
        self.depth -= 1;
        v
    }
}

/// Trạng thái thực thi của **một** lần gọi (khung + ngăn xếp giá trị + iterator).
struct Th<'m> {
    m: &'m Module,
    frames: Vec<Frame>,
    st: Vec<Value>,
    iters: Vec<(Vec<Value>, usize)>,
}

impl<'m> Th<'m> {
    fn new(m: &'m Module, fi: u32, locals: Vec<Value>, captured: Vec<Value>) -> Self {
        Self {
            m,
            frames: vec![Frame {
                fi,
                pc: 0,
                locals,
                captured,
                st_cu: Vec::new(),
            }],
            st: Vec::new(),
            iters: Vec::new(),
        }
    }

    fn f(&self) -> &Func {
        // `fi` do `Vm::func` đã kiểm trước khi dựng khung.
        &self.m.funcs[self.frames.last().expect("khung").fi as usize]
    }

    fn pop(&mut self, vm: &Vm<'_>, span: Span) -> R<Value> {
        self.st
            .pop()
            .ok_or_else(|| vm.hong(VmError::StackUnderflow, span))
    }

    fn pop_n(&mut self, vm: &Vm<'_>, n: usize, span: Span) -> R<Vec<Value>> {
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            v.push(self.pop(vm, span)?);
        }
        v.reverse();
        Ok(v)
    }

    fn day(&self, fr: &Frame, pc: i64, vm: &Vm<'_>, span: Span, what: &str) -> R<()> {
        let len = vm.func(fr.fi)?.code.len() as i64;
        if pc < 0 || pc > len {
            return Err(Abort::Trap(Trap::new(
                RT,
                format!("bytecode hỏng: {} nhảy ngoài code ({pc}/{len})", what),
                span,
            )));
        }
        Ok(())
    }

    fn exec(&mut self, vm: &mut Vm<'_>) -> R<Value> {
        loop {
            // Ghi nhận độ sâu ngăn xếp **lớn nhất** (bất biến fuzz ST-21: `dinh ≤ need`).
            if self.st.len() > vm.dinh {
                vm.dinh = self.st.len();
            }
            let Some(fr) = self.frames.last() else {
                return Ok(Value::Unit);
            };
            let f = self.f();
            if fr.pc >= f.code.len() {
                // Hết mã mà không `Return`/`Halt` — trả `()`.
                let v = Value::Unit;
                self.frames.pop();
                match self.frames.last_mut() {
                    None => return Ok(v),
                    Some(f) => {
                        self.st = std::mem::take(&mut f.st_cu);
                        self.st.push(v);
                    }
                }
                continue;
            }
            let ins = f.code[fr.pc];
            let span = f
                .spans
                .get(fr.pc)
                .map(|s| Span::new(s.start as usize, s.end as usize))
                .unwrap_or_else(|| Span::new(0, 0));
            self.frames.last_mut().expect("khung").pc += 1;
            vm.steps += 1;
            if vm.steps > MAX_STEPS {
                return Err(Abort::Trap(Trap::new(
                    RT,
                    "chạy quá lâu (>10^7 lệnh)",
                    span,
                )));
            }
            match ins {
                Instr::Const(i) => {
                    let v = vm.lit(i, span)?;
                    self.st.push(v);
                }
                Instr::LoadLocal(i) => {
                    let fr = self.frames.last().expect("khung");
                    let v = fr.locals.get(i as usize).cloned().unwrap_or(Value::Unit);
                    self.st.push(v);
                }
                Instr::StoreLocal(i) | Instr::Assign(i) | Instr::BindLocal(i) => {
                    let v = self.pop(vm, span)?;
                    let fr = self.frames.last_mut().expect("khung");
                    if (i as usize) < fr.locals.len() {
                        fr.locals[i as usize] = v;
                    } else {
                        fr.locals.resize(i as usize + 1, Value::Unit);
                        fr.locals[i as usize] = v;
                    }
                }
                Instr::LoadCaptured(i) => {
                    let fr = self.frames.last().expect("khung");
                    let v = fr.captured.get(i as usize).cloned().unwrap_or(Value::Unit);
                    self.st.push(v);
                }
                Instr::Pop => {
                    self.pop(vm, span)?;
                }
                Instr::Dup => {
                    let v = self
                        .st
                        .last()
                        .cloned()
                        .ok_or_else(|| vm.hong(VmError::StackUnderflow, span))?;
                    self.st.push(v);
                }
                Instr::Bin(op) => {
                    let r = self.pop(vm, span)?;
                    let l = self.pop(vm, span)?;
                    let v = builtin::bin(op, l, r, span)?;
                    self.st.push(v);
                }
                Instr::Un(op) => {
                    let v = self.pop(vm, span)?;
                    let v = builtin::un(op, v, span)?;
                    self.st.push(v);
                }
                Instr::Call(fi, n) => {
                    let f = vm.func(fi)?;
                    let args = self.pop_n(vm, n as usize, span)?;
                    let mut locals = vec![Value::Unit; f.nlocals.max(1) as usize];
                    for (i, a) in args.into_iter().enumerate() {
                        if i < locals.len() {
                            locals[i] = a;
                        }
                    }
                    if self.frames.len() + vm.depth + 1 >= MAX_FRAMES {
                        return Err(Abort::Trap(Trap::new(
                            RT,
                            format!("gọi hàm quá sâu (>{MAX_FRAMES} khung) — đệ quy không có điểm dừng?"),
                            span,
                        )));
                    }
                    // Mỗi khung một ngăn xếp: cất ngăn xếp khung gọi, mở ngăn xếp mới.
                    let cu = std::mem::take(&mut self.st);
                    if let Some(f) = self.frames.last_mut() {
                        f.st_cu = cu;
                    }
                    self.frames.push(Frame {
                        fi,
                        pc: 0,
                        locals,
                        captured: Vec::new(),
                        st_cu: Vec::new(),
                    });
                }
                Instr::CallBuiltin(i, n) => {
                    let ten = vm.ten(i, span)?;
                    let args = self.pop_n(vm, n as usize, span)?;
                    let v = builtin::call(vm, &ten, args, span)?;
                    self.st.push(v);
                }
                Instr::CallMethod(i, n) => {
                    let ten = vm.ten(i, span)?;
                    let args = self.pop_n(vm, n as usize, span)?;
                    let recv = self.pop(vm, span)?;
                    let v = builtin::method(vm, recv, &ten, args, span)?;
                    self.st.push(v);
                }
                Instr::Jump(d) => {
                    let pc = {
                        let fr = self.frames.last().expect("khung");
                        let pc = fr.pc as i64 - 1 + d as i64;
                        self.day(fr, pc, vm, span, "Jump")?;
                        pc
                    };
                    self.frames.last_mut().expect("khung").pc = pc as usize;
                }
                Instr::JumpIfFalse(d) => {
                    let v = self.pop(vm, span)?;
                    let ok = v.truthy(span)?;
                    if !ok {
                        let pc = {
                            let fr = self.frames.last().expect("khung");
                            let pc = fr.pc as i64 - 1 + d as i64;
                            self.day(fr, pc, vm, span, "JumpIfFalse")?;
                            pc
                        };
                        self.frames.last_mut().expect("khung").pc = pc as usize;
                    }
                }
                Instr::JumpIfNotTrue(d) => {
                    let v = self
                        .st
                        .last()
                        .cloned()
                        .ok_or_else(|| vm.hong(VmError::StackUnderflow, span))?;
                    if !v.truthy(span)? {
                        let pc = {
                            let fr = self.frames.last().expect("khung");
                            let pc = fr.pc as i64 - 1 + d as i64;
                            self.day(fr, pc, vm, span, "JumpIfNotTrue")?;
                            pc
                        };
                        self.frames.last_mut().expect("khung").pc = pc as usize;
                    }
                }
                Instr::JumpIfNone(d) => {
                    // Luôn **pop** (1→0); nhảy khi đúng điều kiện.
                    let v = self.pop(vm, span)?;
                    if matches!(v, Value::Opt(None)) {
                        let pc = {
                            let fr = self.frames.last().expect("khung");
                            let pc = fr.pc as i64 - 1 + d as i64;
                            self.day(fr, pc, vm, span, "JumpIfNone")?;
                            pc
                        };
                        self.frames.last_mut().expect("khung").pc = pc as usize;
                    }
                }
                Instr::JumpIfSome(d) => {
                    // Luôn **pop** (1→0); nhảy khi đúng điều kiện.
                    let v = self.pop(vm, span)?;
                    if matches!(v, Value::Opt(Some(_))) {
                        let pc = {
                            let fr = self.frames.last().expect("khung");
                            let pc = fr.pc as i64 - 1 + d as i64;
                            self.day(fr, pc, vm, span, "JumpIfSome")?;
                            pc
                        };
                        self.frames.last_mut().expect("khung").pc = pc as usize;
                    }
                }
                Instr::Return | Instr::Halt => {
                    // `Halt` **không** lấy giá trị đỉnh (chương trình kết thúc bằng câu lệnh ⇒ `()`).
                    let v = if matches!(ins, Instr::Halt) {
                        Value::Unit
                    } else {
                        self.st.pop().unwrap_or(Value::Unit)
                    };
                    self.frames.pop();
                    match self.frames.last_mut() {
                        None => return Ok(v),
                        Some(f) => {
                            // Khôi phục ngăn xếp khung gọi rồi đẩy giá trị trả.
                            self.st = std::mem::take(&mut f.st_cu);
                            self.st.push(v);
                        }
                    }
                }
                Instr::MakeList(n) | Instr::MakeTuple(n) | Instr::MakeSet(n) => {
                    let items = self.pop_n(vm, n as usize, span)?;
                    self.st.push(Value::List(items));
                }
                Instr::MakeMap(n) => {
                    let flat = self.pop_n(vm, 2 * n as usize, span)?;
                    let mut pairs = Vec::with_capacity(n as usize);
                    let mut it = flat.into_iter();
                    while let (Some(k), Some(v)) = (it.next(), it.next()) {
                        pairs.push((k, v));
                    }
                    self.st.push(Value::Map(pairs));
                }
                Instr::MakeRecord(t, n) => {
                    let ten = vm.ten(t, span)?;
                    let flat = self.pop_n(vm, 2 * n as usize, span)?;
                    let mut fields = Vec::with_capacity(n as usize);
                    let mut it = flat.into_iter();
                    while let (Some(k), Some(v)) = (it.next(), it.next()) {
                        fields.push((k.show(), v));
                    }
                    self.st.push(Value::Record(ten, fields));
                }
                Instr::MakeEnum(v, n) => {
                    let duong = vm.ten(v, span)?;
                    let args = self.pop_n(vm, n as usize, span)?;
                    let (ten, bien_the) = match duong.rsplit_once('.') {
                        Some((a, b)) => (a.to_string(), b.to_string()),
                        None => (String::new(), duong),
                    };
                    self.st.push(Value::Enum(ten, bien_the, args));
                }
                Instr::MakeSome => {
                    let v = self.pop(vm, span)?;
                    // **Không** bọc hai lớp: `T?` → `T?` (giữ phẳng như interp).
                    match v {
                        Value::Opt(_) => self.st.push(v),
                        other => self.st.push(Value::Opt(Some(Box::new(other)))),
                    }
                }
                Instr::MakeNone => self.st.push(Value::Opt(None)),
                Instr::MakeOk => {
                    let v = self.pop(vm, span)?;
                    self.st.push(Value::Res(Ok(Box::new(v))));
                }
                Instr::MakeErr => {
                    let v = self.pop(vm, span)?;
                    self.st.push(Value::Res(Err(Box::new(v))));
                }
                Instr::MakeClosure(fi, n) => {
                    let cap = self.pop_n(vm, n as usize, span)?;
                    self.st.push(Value::Vm(std::rc::Rc::new(VmFunc {
                        idx: fi,
                        captured: cap,
                    })));
                }
                Instr::Field(i) => {
                    let ten = vm.ten(i, span)?;
                    let v = self.pop(vm, span)?;
                    let v = field(&v, &ten, span)?;
                    self.st.push(v);
                }
                Instr::Index => {
                    let idx = self.pop(vm, span)?;
                    let base = self.pop(vm, span)?;
                    let v = index(&base, &idx, span)?;
                    self.st.push(v);
                }
                Instr::Set => {
                    let v = self.pop(vm, span)?;
                    let idx = self.pop(vm, span)?;
                    let base = self.pop(vm, span)?;
                    let v = set(&base, &idx, v, span)?;
                    self.st.push(v);
                }
                Instr::With(n) => {
                    let flat = self.pop_n(vm, 2 * n as usize, span)?;
                    let mut pairs = Vec::with_capacity(n as usize);
                    let mut it = flat.into_iter();
                    while let (Some(k), Some(v)) = (it.next(), it.next()) {
                        pairs.push((k.show(), v));
                    }
                    let base = self.pop(vm, span)?;
                    let v = with(&base, pairs, span)?;
                    self.st.push(v);
                }
                Instr::Interp(n) => {
                    let parts = self.pop_n(vm, n as usize, span)?;
                    let mut s = String::new();
                    for p in parts {
                        s.push_str(&p.show());
                    }
                    self.st.push(Value::Str(s));
                }
                Instr::IterInit => {
                    let it = self.pop(vm, span)?;
                    let items = value::iter_items(it, span)?;
                    self.iters.push((items, 0));
                }
                Instr::IterNext(d) => {
                    let Some((items, i)) = self.iters.last_mut() else {
                        return Err(vm.hong(VmError::StackUnderflow, span));
                    };
                    if *i >= items.len() {
                        self.iters.pop();
                        let pc = {
                            let fr = self.frames.last().expect("khung");
                            let pc = fr.pc as i64 - 1 + d as i64;
                            self.day(fr, pc, vm, span, "IterNext")?;
                            pc
                        };
                        self.frames.last_mut().expect("khung").pc = pc as usize;
                    } else {
                        let v = items[*i].clone();
                        *i += 1;
                        self.st.push(v);
                    }
                }
                Instr::TestLit(i) => {
                    let want = vm.lit(i, span)?;
                    let v = self.pop(vm, span)?;
                    self.st.push(Value::Bool(value::eq(&v, &want)));
                }
                Instr::TestCtor(i, n) => {
                    let ten = vm.ten(i, span)?;
                    let v = self.pop(vm, span)?;
                    let var_muon = ten.rsplit('.').next().unwrap_or(&ten).to_string();
                    // `some`/`none`/`ok`/`err` là **biến thể dựng sẵn** của `Value::Opt`/`Value::Res`.
                    let thap = var_muon.to_lowercase();
                    let ok = match &v {
                        Value::Enum(_, var, args) => *var == var_muon && args.len() == n as usize,
                        Value::Opt(Some(_)) => thap == "some" && n == 1,
                        Value::Opt(None) => thap == "none" && n == 0,
                        Value::Res(Ok(_)) => thap == "ok" && n == 1,
                        Value::Res(Err(_)) => thap == "err" && n == 1,
                        _ => false,
                    };
                    self.st.push(Value::Bool(ok));
                }
                Instr::TestTuple(n) | Instr::TestList(n) => {
                    let v = self.pop(vm, span)?;
                    let ok = match &v {
                        Value::List(xs) => xs.len() == n as usize,
                        // `()` khớp mẫu tuple rỗng.
                        Value::Unit => n == 0,
                        _ => false,
                    };
                    self.st.push(Value::Bool(ok));
                }
                Instr::TestRecord(_, n, open) => {
                    let v = self.pop(vm, span)?;
                    let ok =
                        matches!(&v, Value::Record(_, fs) if fs.len() == n as usize || open != 0);
                    self.st.push(Value::Bool(ok));
                }
                Instr::TestRange(a, b, incl) => {
                    let lo = vm.lit(a, span)?;
                    let hi = vm.lit(b, span)?;
                    let v = self.pop(vm, span)?;
                    let ok = match (&v, &lo, &hi) {
                        (Value::Int(x), Value::Int(l), Value::Int(h)) => {
                            if incl != 0 {
                                x >= l && x <= h
                            } else {
                                x >= l && x < h
                            }
                        }
                        _ => false,
                    };
                    self.st.push(Value::Bool(ok));
                }
                Instr::CallValue(n) => {
                    let args = self.pop_n(vm, n as usize, span)?;
                    let f = self.pop(vm, span)?;
                    let v = builtin::Host::call(vm, &f, args, span)?;
                    self.st.push(v);
                }
                Instr::IsUnit => {
                    let v = self.pop(vm, span)?;
                    self.st.push(Value::Bool(v.is_unit()));
                }
                Instr::PushUnit => self.st.push(Value::Unit),
                Instr::Swap => {
                    let b = self.pop(vm, span)?;
                    let a = self.pop(vm, span)?;
                    self.st.push(b);
                    self.st.push(a);
                }
                Instr::BoolAnd => {
                    let b = self.pop(vm, span)?;
                    let a = self.pop(vm, span)?;
                    self.st
                        .push(Value::Bool(a.truthy(span)? && b.truthy(span)?));
                }
                Instr::BoolOr => {
                    let b = self.pop(vm, span)?;
                    let a = self.pop(vm, span)?;
                    self.st
                        .push(Value::Bool(a.truthy(span)? || b.truthy(span)?));
                }
                Instr::UnwrapOpt => {
                    let v = self.pop(vm, span)?;
                    match v {
                        Value::Opt(Some(x)) => self.st.push((*x).clone()),
                        other => self.st.push(other),
                    }
                }
                Instr::Unwrap(i) => {
                    let v = self.pop(vm, span)?;
                    let x = match &v {
                        Value::Enum(_, _, args) => args.get(i as usize).cloned(),
                        Value::Opt(Some(x)) if i == 0 => Some((**x).clone()),
                        Value::Res(Ok(x)) if i == 0 => Some((**x).clone()),
                        Value::Res(Err(x)) if i == 0 => Some((**x).clone()),
                        Value::List(xs) => xs.get(i as usize).cloned(),
                        _ => None,
                    };
                    let x = x.ok_or_else(|| {
                        Abort::Trap(Trap::new(
                            RT,
                            format!("không lấy được đối số thứ {i}"),
                            span,
                        ))
                    })?;
                    self.st.push(x);
                }
                Instr::Try => {
                    let v = self.pop(vm, span)?;
                    match &v {
                        Value::Opt(None) | Value::Res(Err(_)) => {
                            self.frames.pop();
                            match self.frames.last_mut() {
                                None => return Ok(v),
                                Some(f) => {
                                    self.st = std::mem::take(&mut f.st_cu);
                                    self.st.push(v);
                                }
                            }
                        }
                        Value::Opt(Some(x)) => {
                            let x = (**x).clone();
                            self.st.push(x);
                        }
                        Value::Res(Ok(x)) => {
                            let x = (**x).clone();
                            self.st.push(x);
                        }
                        other => self.st.push(other.clone()),
                    }
                }
            }
        }
    }
}

/// `kind` số của const pool → `LitKind`.
fn lit_kind(k: u8) -> Option<LitKind> {
    Some(match k {
        0 => LitKind::Int,
        1 => LitKind::Float,
        2 => LitKind::Size,
        3 => LitKind::Duration,
        4 => LitKind::Percent,
        5 => LitKind::Str,
        6 => LitKind::Bytes,
        7 => LitKind::Path,
        8 => LitKind::Char,
        9 => LitKind::Bool,
        _ => return None,
    })
}

fn field(v: &Value, name: &str, span: Span) -> R<Value> {
    // `.` trên `some(x)` tự bóc (interp làm vậy) — `?.` mới là đường có kiểm `none`.
    if let Value::Opt(Some(x)) = v {
        return field(x, name, span);
    }
    match v {
        Value::Record(_, fs) => fs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
            .ok_or_else(|| {
                Abort::Trap(Trap::new(
                    RT,
                    format!("bản ghi không có trường `{name}`"),
                    span,
                ))
            }),
        Value::Enum(_, _, args) => {
            let i: usize = name.parse().map_err(|_| {
                Abort::Trap(Trap::new(RT, format!("`{name}` không phải trường"), span))
            })?;
            args.get(i)
                .cloned()
                .ok_or_else(|| Abort::Trap(Trap::new(RT, format!("không có đối số thứ {i}"), span)))
        }
        other => Err(Abort::Trap(Trap::new(
            RT,
            format!("`{}` không có trường `{name}`", other.show()),
            span,
        ))),
    }
}

fn index(base: &Value, idx: &Value, span: Span) -> R<Value> {
    // ST-30b: **trùng đúng** chẩn đoán của interp (cùng mã `VS0503`, cùng thông điệp) — hai đường
    // phải cho **cùng** danh sách `(mã, dòng, cột, thông điệp)`; trước đây VM dùng `VS0501` + câu ngắn.
    match (base, idx) {
        (Value::List(xs), Value::Int(i)) => {
            let len = xs.len() as i64;
            if *i < 0 {
                return Err(Abort::Trap(Trap::new(
                    IDX,
                    format!("chỉ số âm `{i}` (dùng `.last()` hoặc `xs[i]?`)"),
                    span,
                )));
            }
            xs.get(*i as usize).cloned().ok_or_else(|| {
                Abort::Trap(Trap::new(
                    IDX,
                    format!("chỉ số {i} ngoài phạm vi (độ dài {len})"),
                    span,
                ))
            })
        }
        (Value::Bytes(bs), Value::Int(i)) => {
            let len = bs.len() as i64;
            if *i < 0 || *i >= len {
                return Err(Abort::Trap(Trap::new(
                    IDX,
                    format!("chỉ số {i} ngoài phạm vi (độ dài {len})"),
                    span,
                )));
            }
            Ok(Value::Int(bs[*i as usize] as i64))
        }
        (Value::Map(kv), k) => kv
            .iter()
            .find(|(kk, _)| value::eq(kk, k))
            .map(|(_, v)| v.clone())
            .ok_or_else(|| {
                Abort::Trap(Trap::new(
                    IDX,
                    format!("khoá `{}` không có trong map", k.show()),
                    span,
                ))
            }),
        (other, idx) => Err(Abort::Trap(Trap::new(
            RT,
            format!("không chỉ số được `{}` bằng `{}`", other.show(), idx.show()),
            span,
        ))),
    }
}

fn set(base: &Value, idx: &Value, v: Value, span: Span) -> R<Value> {
    match (base, idx) {
        (Value::List(xs), Value::Int(i)) => {
            let mut xs = xs.clone();
            if (*i as usize) < xs.len() {
                xs[*i as usize] = v;
                Ok(Value::List(xs))
            } else {
                Err(Abort::Trap(Trap::new(
                    RT,
                    format!("chỉ số {i} ngoài phạm vi"),
                    span,
                )))
            }
        }
        (Value::Map(kv), k) => {
            let mut kv = kv.clone();
            if let Some(e) = kv.iter_mut().find(|(kk, _)| value::eq(kk, k)) {
                e.1 = v;
            } else {
                kv.push((k.clone(), v));
            }
            Ok(Value::Map(kv))
        }
        _ => Err(Abort::Trap(Trap::new(RT, "gán chỉ số không hợp lệ", span))),
    }
}

fn with(base: &Value, pairs: Vec<(String, Value)>, span: Span) -> R<Value> {
    match base {
        Value::Record(ten, fs) => {
            let mut fs = fs.clone();
            for (k, v) in pairs {
                if let Some(e) = fs.iter_mut().find(|(kk, _)| *kk == k) {
                    e.1 = v;
                } else {
                    return Err(Abort::Trap(Trap::new(
                        RT,
                        format!("bản ghi `{ten}` không có trường `{k}`"),
                        span,
                    )));
                }
            }
            Ok(Value::Record(ten.clone(), fs))
        }
        other => Err(Abort::Trap(Trap::new(
            RT,
            format!("`with` không dùng được cho `{}`", other.show()),
            span,
        ))),
    }
}
