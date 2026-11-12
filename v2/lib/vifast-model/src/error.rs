// SPDX-License-Identifier: MIT OR Apache-2.0
//! Mã lỗi của ABI — tên/số lấy từ `vifast-abi` (sinh từ `spec/abi.toml`, D2-11).

pub use vifast_abi::{CORE_OBJECTS, ERRORS};

/// 19 mã lỗi của `spec/abi.toml`; thứ tự trùng `vifast_abi::ERRORS`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KError {
    InvalidCapability,
    WrongObjectType,
    InsufficientRights,
    WxViolation,
    InvalidArgument,
    Alignment,
    SlotOccupied,
    SlotEmpty,
    NotEnoughMemory,
    AlreadyMapped,
    NotMapped,
    MissingPageTable,
    HasChildren,
    Busy,
    Cancelled,
    Timeout,
    IllegalOperation,
    NotSupported,
    RangeError,
}

impl KError {
    /// Chỉ số trong `vifast_abi::ERRORS` / `vifast_model::ERRORS` (0-based).
    pub const fn spec_index(self) -> usize {
        match self {
            KError::InvalidCapability => 0,
            KError::WrongObjectType => 1,
            KError::InsufficientRights => 2,
            KError::WxViolation => 3,
            KError::InvalidArgument => 4,
            KError::Alignment => 5,
            KError::SlotOccupied => 6,
            KError::SlotEmpty => 7,
            KError::NotEnoughMemory => 8,
            KError::AlreadyMapped => 9,
            KError::NotMapped => 10,
            KError::MissingPageTable => 11,
            KError::HasChildren => 12,
            KError::Busy => 13,
            KError::Cancelled => 14,
            KError::Timeout => 15,
            KError::IllegalOperation => 16,
            KError::NotSupported => 17,
            KError::RangeError => 18,
        }
    }

    /// Mã lỗi từ chỉ số spec; `None` ngoài dải 0..19.
    pub const fn from_index(index: usize) -> Option<Self> {
        Some(match index {
            0 => KError::InvalidCapability,
            1 => KError::WrongObjectType,
            2 => KError::InsufficientRights,
            3 => KError::WxViolation,
            4 => KError::InvalidArgument,
            5 => KError::Alignment,
            6 => KError::SlotOccupied,
            7 => KError::SlotEmpty,
            8 => KError::NotEnoughMemory,
            9 => KError::AlreadyMapped,
            10 => KError::NotMapped,
            11 => KError::MissingPageTable,
            12 => KError::HasChildren,
            13 => KError::Busy,
            14 => KError::Cancelled,
            15 => KError::Timeout,
            16 => KError::IllegalOperation,
            17 => KError::NotSupported,
            18 => KError::RangeError,
            _ => return None,
        })
    }

    /// Số hiệu trong ABI (`spec/abi.toml`).
    pub fn id(self) -> u16 {
        ERRORS[self.spec_index()].1
    }

    /// Tên trong ABI (`spec/abi.toml`).
    pub fn name(self) -> &'static str {
        ERRORS[self.spec_index()].0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ma_loi_lay_dung_tu_spec() {
        assert_eq!(ERRORS.len(), 19);
        for (i, (name, id)) in ERRORS.iter().enumerate() {
            let k = KError::from_index(i).expect("đủ 19 mã lỗi");
            assert_eq!(k.name(), *name, "tên mã lỗi {i}");
            assert_eq!(k.id(), *id, "số mã lỗi {i}");
            assert_eq!(k.spec_index(), i);
        }
        assert_eq!(KError::from_index(19), None);
        assert_eq!(KError::InvalidCapability.id(), 1);
        assert_eq!(KError::NotSupported.id(), 18);
        assert_eq!(KError::RangeError.id(), 19);
    }
}
