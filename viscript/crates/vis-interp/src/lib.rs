// SPDX-License-Identifier: MIT OR Apache-2.0
//! **`vis-interp` — thông dịch cây trên HIR (ST-13, VS0).**
//!
//! Chạy chương trình đã hạ (`vis_types::hir::Hir`) theo ngữ nghĩa VD-43: `let` bất biến / `var` gán
//! được; `if/while/for/match`; `return`; `?` (Result/Option, trả sớm); `??`; `|>`; `with`; chỉ số;
//! gọi hàm (tham số tên `--b` + mặc định), closure; 8 tên dựng sẵn + bộ phương thức lõi.
//!
//! # Lỗi runtime — sổ `05xx` (VD-42)
//!
//! `VS0501` chung (tràn `i64`, phép/tính năng chưa hỗ trợ ở VS0), `VS0502` chia/lấy dư cho 0,
//! `VS0503` chỉ số ngoài phạm vi. **Không panic** trên dữ liệu người dùng; chẩn đoán có `span` +
//! `line`/`col` **thật** tính từ `text` + `span` (VD-46 bắt buộc ở tầng này).
//!
//! # API
//!
//! `run(text, &hir, &symbols, &types, &args) -> RunOutput` — chạy `main` nếu có (đối số `args`),
//! không có thì chạy các câu lệnh cấp tệp theo thứ tự. `text` cần cho `line`/`col` của chẩn đoán.
//!
//! ```
//! use vis_interp::run;
//! use vis_syntax::{lex, parse, Mode};
//! use vis_types::lower;
//!
//! let src = "print(1 + 2)";
//! let p = parse(src, Mode::Script);
//! let h = lower(&p.cst, &lex(src, Mode::Script));
//! let out = run(src, &h.hir, &h.symbols, &h.types, &[]);
//! assert!(out.is_clean());
//! assert_eq!(out.stdout, "3\n");
//! assert_eq!(out.exit, 0);
//! ```
//!
//! Chia 0 ⇒ một chẩn đoán `VS0502` (không panic):
//!
//! ```
//! # use vis_interp::run;
//! # use vis_syntax::{lex, parse, Mode};
//! # use vis_types::lower;
//! let src = "print(1 / 0)";
//! let p = parse(src, Mode::Script);
//! let h = lower(&p.cst, &lex(src, Mode::Script));
//! let out = run(src, &h.hir, &h.symbols, &h.types, &[]);
//! assert_eq!(out.codes(), ["VS0502"]);
//! assert_eq!(out.exit, 1);
//! ```

pub mod value;

mod builtin;
mod interp;

pub use value::{Closure, Value};

use vis_syntax::Diagnostic;
use vis_types::hir::Hir;
use vis_types::lower::Symbols;
use vis_types::TypeTable;

/// Kết quả một lần chạy.
#[derive(Debug, Clone, Default)]
pub struct RunOutput {
    /// Toàn bộ `stdout` đã in (`print`).
    pub stdout: String,
    /// Chẩn đoán runtime (`VS05xx`), tối đa một cái — lỗi đầu tiên dừng chương trình.
    pub diagnostics: Vec<Diagnostic>,
    /// Giá trị `main` trả (hoặc biểu thức cấp tệp cuối).
    pub value: Option<Value>,
    /// Mã thoát: `0` nếu sạch; nếu `main` trả `int` thì dùng số đó (cắt 8 bit); lỗi ⇒ `1`.
    pub exit: i32,
    /// Số bước đã chạy — **chống xanh-giả** trong test (`out("")` vẫn phải chạy thật).
    pub steps: usize,
}

impl RunOutput {
    /// `true` nếu không có chẩn đoán.
    pub fn is_clean(&self) -> bool {
        self.diagnostics.is_empty()
    }

    /// Mã chẩn đoán theo thứ tự.
    pub fn codes(&self) -> Vec<&'static str> {
        self.diagnostics.iter().map(|d| d.code.short).collect()
    }
}

/// Chạy một chương trình đã hạ.
pub fn run(
    text: &str,
    hir: &Hir,
    symbols: &Symbols,
    types: &TypeTable,
    args: &[String],
) -> RunOutput {
    let mut it = interp::Interp::new(hir, symbols, types);
    let res = it.run_program(args);
    let stdout = it.take_stdout();
    let steps = it.steps;
    match res {
        Ok(value) => {
            let exit = match &value {
                Some(Value::Int(i)) => (*i & 0xff) as i32,
                _ => 0,
            };
            RunOutput {
                stdout,
                diagnostics: Vec::new(),
                value,
                exit,
                steps,
            }
        }
        Err(interp::Abort::Ret(value)) => RunOutput {
            stdout,
            diagnostics: Vec::new(),
            value: Some(value),
            exit: 0,
            steps,
        },
        Err(interp::Abort::Trap(trap)) => RunOutput {
            stdout,
            diagnostics: vec![trap.to_diag(text)],
            value: None,
            exit: 1,
            steps,
        },
    }
}
