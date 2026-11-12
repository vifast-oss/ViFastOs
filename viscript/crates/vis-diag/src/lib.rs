// SPDX-License-Identifier: MIT OR Apache-2.0
//! `vis-diag` — khung chẩn đoán của dòng ViScript (ST-01).
//!
//! Vòng 1 mới có **định dạng mã lỗi** `VS` + 4 chữ số (sổ mã đầy đủ thuộc ST-02b).
//! Không `unsafe` (luật dòng ViScript), không phụ thuộc ngoài.

/// Định dạng một mã lỗi ViScript: `VS` + 4 chữ số, ví dụ `VS0001`.
pub fn code(n: u16) -> String {
    format!("VS{n:04}")
}

/// Chẩn đoán tối thiểu: mã + thông điệp + vị trí byte (0-based).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diag {
    pub code: String,
    pub msg: String,
    pub offset: usize,
}

impl Diag {
    pub fn new(n: u16, msg: impl Into<String>, offset: usize) -> Self {
        Self {
            code: code(n),
            msg: msg.into(),
            offset,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ma_co_bon_chu_so() {
        assert_eq!(code(1), "VS0001");
        assert_eq!(code(9999), "VS9999");
    }

    #[test]
    fn chan_doan_giu_vi_tri() {
        let d = Diag::new(12, "token lạ", 7);
        assert_eq!(d.code, "VS0012");
        assert_eq!(d.msg, "token lạ");
        assert_eq!(d.offset, 7);
    }
}
