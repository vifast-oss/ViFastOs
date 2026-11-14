// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-60**: `impl Host for Interp` — tách khỏi `interp.rs` cho luật **R1** (≤1.500 dòng/tệp).
//!
//! Host là **ranh giới duy nhất** giữa interp và môi trường (ST-27/ST-33): `out`, gọi hàm,
//! chương trình ngoài, hệ thống tệp, và từ ST-60 là **hàng đợi tác vụ** `async`.

use vis_syntax::Span;

use crate::interp::{Abort, Interp, Trap, R, RT};
use crate::value::Value;

impl crate::builtin::Host for Interp<'_> {
    fn out(&mut self) -> &mut String {
        &mut self.out
    }

    fn call(&mut self, f: &Value, args: Vec<Value>, span: Span) -> R<Value> {
        self.call_value(f.clone(), args, Vec::new(), span)
    }

    fn sched(&self) -> &std::cell::RefCell<crate::rt::Scheduler> {
        &self.sched
    }

    /// **ST-60**: handle của interp là **`sym`** của `HFn`.
    fn goi_than(&mut self, fi: u32, args: Vec<Value>, span: Span) -> R<Value> {
        let Some(f) = self.fns.get(&fi).copied() else {
            return Err(Abort::Trap(Trap::new(
                RT,
                "tác vụ trỏ hàm không tồn tại",
                span,
            )));
        };
        self.call_hfn(f, args, Vec::new(), span)
    }

    fn ten_than(&self, fi: u32) -> String {
        self.fns
            .get(&fi)
            .map(|f| f.name.clone())
            .unwrap_or_default()
    }

    fn fs(&mut self) -> Option<&mut dyn crate::builtin::FsHost> {
        self.fs
            .as_mut()
            .map(|f| &mut **f as &mut dyn crate::builtin::FsHost)
    }

    /// **ST-63 (VD-105)**: host mạng (TCP) của CLI; `None` ⇒ `std.net.*` ⇒ `VS0604`.
    fn net(&mut self) -> Option<&mut dyn crate::builtin::NetHost> {
        self.net
            .as_mut()
            .map(|n| &mut **n as &mut dyn crate::builtin::NetHost)
    }

    fn ngoai(
        &mut self,
        ten: &str,
        args: &[String],
        raw: bool,
        stdin: &str,
        span: Span,
    ) -> Option<R<Value>> {
        let n = self.ngoai.as_mut()?;
        Some(n.chay(ten, args, raw, stdin, span))
    }

    fn ngoai_day_du(
        &mut self,
        ten: &str,
        args: &[String],
        stdin: &str,
        span: Span,
    ) -> Option<Result<crate::builtin::KetQua, Abort>> {
        let n = self.ngoai.as_mut()?;
        Some(n.chay_day_du(ten, args, stdin, span))
    }
}
