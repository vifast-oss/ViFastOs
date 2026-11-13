// SPDX-License-Identifier: MIT OR Apache-2.0
//! Lỗi **cục bộ** của mô hình hình PTE (KT-35).
//!
//! Đây **không** phải bảng mã lỗi ABI: tên lỗi lấy đúng `name` của `spec/abi.toml`
//! (`[error]` số 5 `InvalidArgument`) để không sinh tên thứ hai, nhưng crate **không** phụ thuộc
//! `vifast-abi` (D2-21, D2-65 Q8) và **không** đổi `spec/`. Bảy mã lỗi VM dùng lại
//! (`WxViolation` 4, `Alignment` 6, `AlreadyMapped` 10, `NotMapped` 11, `MissingPageTable` 12,
//! `HasChildren` 13, `RangeError` 19) thuộc KT-36..KT-39; KT-35 chỉ cần mã từ chối `size_bits`.

/// `size_bits` không thuộc `{12, 21, 30}` — lỗi model/local, **không** đổi ABI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PteError {
    /// Cỡ trang không phải 4 KiB / 2 MiB / 1 GiB; tương ứng `InvalidArgument` (số 5) của ABI.
    InvalidArgument,
}

impl PteError {
    /// Tên mã lỗi đúng như `spec/abi.toml` — dùng khi in log/test, không phải chữ ký ABI.
    pub const fn code_name(self) -> &'static str {
        match self {
            PteError::InvalidArgument => "InvalidArgument",
        }
    }
}

impl core::fmt::Display for PteError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{}: size_bits phải là 12 (4 KiB) / 21 (2 MiB) / 30 (1 GiB)",
            self.code_name()
        )
    }
}
