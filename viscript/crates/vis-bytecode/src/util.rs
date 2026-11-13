// SPDX-License-Identifier: MIT OR Apache-2.0
//! Tiện ích nhỏ của codegen: `can_test`, tên dựng sẵn, ánh xạ `AssignOp` ⇒ `BinOp`.

use vis_types::hir::HPat;
use vis_types::{AssignOp, BinOp};

use crate::codegen::CodegenError;

/// Pattern con có cần **test** không (literal/ctor/tuple/list/record/range/or).
pub fn can_test(p: &HPat) -> bool {
    matches!(
        p,
        HPat::Lit { .. }
            | HPat::Ctor { .. }
            | HPat::Tuple { .. }
            | HPat::List { .. }
            | HPat::Record { .. }
            | HPat::Range { .. }
            | HPat::Or { .. }
    )
}

/// Tên **dựng sẵn** (phải khớp `vis_interp::builtin::SIGS`; test `codegen` khoá lại).
pub fn la_builtin(name: &str) -> bool {
    // ST-26: `std.<module>.<fn>` (tên đầy đủ trong HIR) — bảng chữ ký ở `vis_types::stdlib`.
    if name.starts_with("std.") {
        return vis_types::stdlib::la_std(name.trim_start_matches("std."));
    }
    matches!(
        name,
        "print"
            | "len"
            | "str"
            | "echo"
            | "lines"
            | "take"
            | "Ok"
            | "Err"
            | "Some"
            | "None"
            | "ref"
    )
}

pub fn assign_binop(op: vis_types::AssignOp) -> Result<vis_types::BinOp, CodegenError> {
    Ok(match op {
        AssignOp::Set => BinOp::Add,
        AssignOp::Add => BinOp::Add,
        AssignOp::Sub => BinOp::Sub,
        AssignOp::Mul => BinOp::Mul,
        AssignOp::Div => BinOp::Div,
        AssignOp::Rem => BinOp::Rem,
    })
}
