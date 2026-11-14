// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-60b**: `CodegenError` — lỗi của codegen (tách khỏi `codegen.rs` cho luật **R1** ≤1.500
//! dòng/tệp). Nội dung/ngữ nghĩa **không** đổi; `codegen` re-export để đường dẫn cũ vẫn dùng được.

use vis_syntax::Span;

/// Lỗi codegen — **không** panic; mỗi lỗi nêu biến thể HIR + span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodegenError {
    /// Biến thể VS1 chưa biểu diễn được (kèm tên + span).
    Unsupported(&'static str, Span),
    /// Quá nhiều ô cục bộ / đối số cho `u16`.
    TooMany(u16),
    /// Module không có hàm `main`.
    NoEntry,
    /// **ST-54**: front end đã báo lỗi ⇒ codegen **từ chối** (mã + dòng:cột + thông điệp).
    ChanDoan {
        short: &'static str,
        dong: u32,
        cot: u32,
        msg: String,
    },
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
            CodegenError::ChanDoan {
                short,
                dong,
                cot,
                msg,
            } => write!(f, "{short} tại {dong}:{cot}: {msg}"),
            CodegenError::KhongHopLe(e) => write!(f, "mã sinh ra không hợp lệ: {e}"),
        }
    }
}
