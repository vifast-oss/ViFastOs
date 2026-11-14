// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-63 (VD-105 (6))**: phần **gọi hàm/closure** của interp — tách khỏi `interp.rs` cho luật
//! **R1** (≤1.500 dòng/tệp). Chuyển **nguyên văn**, không đổi một dòng ngữ nghĩa nào.
//!
//! Đây là **mô-đun con** của `interp` nên vẫn thấy `Interp`/`Closure`/`HFn` + `MAX_CALL_DEPTH`
//! như trước; `interp.rs` chỉ còn phần câu lệnh/biểu thức.

use vis_syntax::Span;
use vis_types::hir::*;

use crate::interp::{builtin_name, Abort, Args, Interp, Trap, MAX_CALL_DEPTH, R, RT};
use crate::value::*;

impl<'a> Interp<'a> {
    // ---------------------------------------------------------------------
    // Gọi hàm
    // ---------------------------------------------------------------------

    pub(crate) fn eval_call(
        &mut self,
        callee: &HExpr,
        args: &[HExpr],
        named: &[(String, HExpr)],
        span: Span,
    ) -> R<Value> {
        // 1) Phương thức trên list/str/map: `xs.push(1)`, `s.upper()`.
        if let HExpr::Field {
            base,
            name,
            optional: false,
            ..
        } = callee
        {
            let recv = self.eval(base)?;
            // **ST-61 (VD-96)**: kênh cũng đi **một** nhánh `builtin::method` (như list/str/map) ⇒
            // interp ↔ VM cùng một hiện thực `send`/`recv`/`close`.
            if matches!(
                recv,
                Value::List(_) | Value::Str(_) | Value::Map(_) | Value::Chan(_)
            ) {
                let mut vs = Vec::with_capacity(args.len());
                for a in args {
                    vs.push(self.eval(a)?);
                }
                let kq = crate::builtin::method(self, recv, name, vs, span)?;
                // ST-15b (mục 1, §7.1 ngữ nghĩa giá trị + CoW): `push` trên **`var`** phải **ghi lại**
                // biến (bản thân list đổi), không chỉ trả list mới. `push` trên `let`/biểu thức tạm
                // vẫn "trả list mới" (không có lvalue để ghi) — ghi rõ trong JOURNAL.
                if name == "push" {
                    if let HExpr::Var { sym: Some(id), .. } = &**base {
                        if self.mutable.contains(id) {
                            self.env.borrow_mut().insert(*id, kq.clone());
                        }
                    }
                }
                return Ok(kq);
            }
        }
        // 2) Gọi thẳng hàm khai trong tệp (kể cả đệ quy) — theo `sym`.
        if let HExpr::Var { sym: Some(id), .. } = callee {
            if let Some(f) = self.fns.get(id).copied() {
                let (pos, nm) = self.eval_args(args, named)?;
                // **ST-60 (VD-90)**: gọi `async fn` **không** chạy thân — trả `Task[T]` (cùng một
                // đường với codegen: `$tac_vu` + handle, ở đây handle = `sym`).
                if f.is_async {
                    let t = self.sched.get_mut().tao(f.sym, pos, f.name.clone(), span);
                    return Ok(Value::Task(t));
                }
                return self.call_hfn(f, pos, nm, span);
            }
        }
        // 3) Hàm dựng sẵn.
        if let HExpr::Var {
            name, sym: None, ..
        } = callee
        {
            if builtin_name(name).is_some() {
                let (pos, _nm) = self.eval_args(args, named)?;
                return crate::builtin::call(self, name, pos, span);
            }
        }
        // 4) Còn lại: giá trị hàm (closure/lambda/builtin value).
        let f = self.eval(callee)?;
        let (pos, nm) = self.eval_args(args, named)?;
        self.call_value(f, pos, nm, span)
    }

    fn eval_args(&mut self, args: &[HExpr], named: &[(String, HExpr)]) -> R<Args> {
        let mut pos = Vec::with_capacity(args.len());
        for a in args {
            pos.push(self.eval(a)?);
        }
        let mut nm = Vec::with_capacity(named.len());
        for (n, a) in named {
            nm.push((n.clone(), self.eval(a)?));
        }
        Ok((pos, nm))
    }

    pub(crate) fn call_value(
        &mut self,
        f: Value,
        pos: Vec<Value>,
        named: Vec<(String, Value)>,
        span: Span,
    ) -> R<Value> {
        match f {
            Value::Fn(c) => self.call_closure(&c, pos, span),
            Value::Builtin(name) => crate::builtin::call(self, name, pos, span),
            Value::Record(name, fields) if fields.is_empty() => {
                // `User(name: "An", age: 20)` — khởi tạo bản ghi theo tên kiểu.
                let _ = pos;
                Ok(Value::Record(name, named))
            }
            other => {
                let _ = named;
                Err(Abort::Trap(Trap::new(
                    RT,
                    format!("`{}` không phải hàm", other.show()),
                    span,
                )))
            }
        }
    }

    pub(crate) fn call_closure(&mut self, c: &Closure, pos: Vec<Value>, span: Span) -> R<Value> {
        self.depth += 1;
        if self.depth > MAX_CALL_DEPTH {
            self.depth -= 1;
            return Err(Abort::Trap(Trap::new(
                RT,
                format!(
                    "gọi hàm quá sâu (>{} khung) — đệ quy không có điểm dừng?",
                    MAX_CALL_DEPTH
                ),
                span,
            )));
        }
        self.fn_depth += 1;
        let kq = self.call_closure_trong(c, pos, span);
        self.fn_depth -= 1;
        self.depth -= 1;
        kq
    }

    fn call_closure_trong(&mut self, c: &Closure, pos: Vec<Value>, span: Span) -> R<Value> {
        if pos.len() != c.params.len() {
            return Err(Abort::Trap(Trap::new(
                RT,
                format!(
                    "hàm `{}` cần {} đối số, được {}",
                    if c.name.is_empty() { "<fn>" } else { &c.name },
                    c.params.len(),
                    pos.len()
                ),
                span,
            )));
        }
        let saved_env = self.env.clone();
        self.env = c.env.clone();
        let mut old = Vec::with_capacity(c.params.len());
        for (i, p) in c.params.iter().enumerate() {
            old.push((*p, self.env.borrow().get(p).cloned()));
            self.env.borrow_mut().insert(*p, pos[i].clone());
        }
        let r = match &c.body {
            HBody::Expr(e) => self.eval(e),
            HBody::Stmts(st) => self.exec_body(st),
        };
        for (p, v) in old {
            match v {
                Some(v) => {
                    self.env.borrow_mut().insert(p, v);
                }
                None => {
                    self.env.borrow_mut().remove(&p);
                }
            }
        }
        self.env = saved_env;
        match r {
            Ok(v) => Ok(v),
            Err(Abort::Ret(v)) => Ok(v),
            Err(t) => Err(t),
        }
    }

    pub(crate) fn call_hfn(
        &mut self,
        f: &'a HFn,
        pos: Vec<Value>,
        named: Vec<(String, Value)>,
        span: Span,
    ) -> R<Value> {
        self.depth += 1;
        if self.depth > MAX_CALL_DEPTH {
            self.depth -= 1;
            return Err(Abort::Trap(Trap::new(
                RT,
                format!(
                    "gọi hàm quá sâu (>{} khung) — đệ quy không có điểm dừng?",
                    MAX_CALL_DEPTH
                ),
                span,
            )));
        }
        self.fn_depth += 1;
        let kq = self.call_hfn_trong(f, pos, named, span);
        self.fn_depth -= 1;
        self.depth -= 1;
        kq
    }

    fn call_hfn_trong(
        &mut self,
        f: &'a HFn,
        pos: Vec<Value>,
        named: Vec<(String, Value)>,
        span: Span,
    ) -> R<Value> {
        // ST-27: hàm có `...rest` nhận **thêm** đối số vị trí.
        let bien_di = f.params.last().map(|p| p.variadic).unwrap_or(false);
        if !bien_di && pos.len() + named.len() > f.params.len() {
            return Err(Abort::Trap(Trap::new(
                RT,
                format!(
                    "hàm `{}` nhận {} đối số, được {}",
                    f.name,
                    f.params.len(),
                    pos.len() + named.len()
                ),
                span,
            )));
        }
        let mut old = Vec::with_capacity(f.params.len());
        for (i, p) in f.params.iter().enumerate() {
            old.push((p.sym, self.env.borrow().get(&p.sym).cloned()));
            // ST-27: `...rest` gom **mọi** đối số vị trí còn lại thành `list[str]`.
            if p.variadic {
                let con: Vec<Value> = pos.iter().skip(i).cloned().collect();
                self.env.borrow_mut().insert(p.sym, Value::List(con));
                continue;
            }
            let v = if let Some(v) = pos.get(i) {
                Some(v.clone())
            } else {
                named
                    .iter()
                    .find(|(n, _)| *n == p.name)
                    .map(|(_, v)| v.clone())
            };
            let v = match v {
                Some(v) => v,
                None => match &p.default {
                    Some(d) => self.eval(d)?,
                    None => {
                        return Err(Abort::Trap(Trap::new(
                            RT,
                            format!("thiếu đối số `{}` của `{}`", p.name, f.name),
                            span,
                        )))
                    }
                },
            };
            self.env.borrow_mut().insert(p.sym, v);
            if p.is_var_self {
                self.mutable.insert(p.sym);
            }
        }
        let r = self.exec_body(&f.body);
        for (p, v) in old {
            match v {
                Some(v) => {
                    self.env.borrow_mut().insert(p, v);
                }
                None => {
                    self.env.borrow_mut().remove(&p);
                }
            }
        }
        match r {
            Ok(v) => Ok(v),
            Err(Abort::Ret(v)) => Ok(v),
            Err(t) => Err(t),
        }
    }
}
