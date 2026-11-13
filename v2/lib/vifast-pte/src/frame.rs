// SPDX-License-Identifier: MIT OR Apache-2.0
//! Cỡ Frame của K2: 4 KiB / 2 MiB / 1 GiB (K2-PLAN Q2, D2-65).
//!
//! Ánh xạ **hai chiều** với `size_bits` của ABI (`Frame`/`Untyped.Retype`): `12 ↔ K4`,
//! `21 ↔ M2`, `30 ↔ G1`. `size_bits` khác ba giá trị đó là **lỗi model/local**
//! ([`PteError::InvalidArgument`]) — không thêm mã lỗi ABI, không đổi `spec/`.

use crate::error::PteError;
use crate::{G1_BITS, K4_BITS, M2_BITS, PTE_LEVELS};

/// Cỡ Frame của K2. Tên biến thể là `<đơn vị><số>` vì Rust không cho định danh bắt đầu bằng số.
///
/// | Biến thể | `size_bits` | Cỡ | Cấp lá |
/// |---|---|---|---|
/// | [`FrameSize::K4`] | 12 | 4 KiB | 0 |
/// | [`FrameSize::M2`] | 21 | 2 MiB | 1 |
/// | [`FrameSize::G1`] | 30 | 1 GiB | 2 |
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FrameSize {
    /// Trang 4 KiB — `size_bits = 12`, lá ở cấp 0.
    K4,
    /// Trang lớn 2 MiB — `size_bits = 21`, lá ở cấp 1.
    M2,
    /// Trang lớn 1 GiB — `size_bits = 30`, lá ở cấp 2.
    G1,
}

impl FrameSize {
    /// Cả ba cỡ theo **đúng thứ tự** của [`PAGE_SIZE_BITS`] (nhỏ → lớn).
    pub const ALL: [FrameSize; PTE_LEVELS] = [FrameSize::K4, FrameSize::M2, FrameSize::G1];

    /// `size_bits` của ABI: `12` / `21` / `30`.
    pub const fn size_bits(self) -> u8 {
        match self {
            FrameSize::K4 => K4_BITS,
            FrameSize::M2 => M2_BITS,
            FrameSize::G1 => G1_BITS,
        }
    }

    /// Cỡ theo byte: `4 KiB` / `2 MiB` / `1 GiB` (`1u64 << size_bits`).
    pub const fn size_bytes(self) -> u64 {
        1u64 << self.size_bits()
    }

    /// Parse `size_bits`; chỉ nhận `12`/`21`/`30`, còn lại [`PteError::InvalidArgument`].
    ///
    /// Hàm **không** panic với bất kỳ `u8` nào và không phụ thuộc `spec/`: đây là kiểm tra
    /// model/local của K2 (KT-35/DoD: test biên `size_bits` lạ).
    pub const fn from_size_bits(size_bits: u8) -> Result<FrameSize, PteError> {
        match size_bits {
            K4_BITS => Ok(FrameSize::K4),
            M2_BITS => Ok(FrameSize::M2),
            G1_BITS => Ok(FrameSize::G1),
            _ => Err(PteError::InvalidArgument),
        }
    }
}

impl TryFrom<u8> for FrameSize {
    type Error = PteError;

    fn try_from(size_bits: u8) -> Result<Self, Self::Error> {
        FrameSize::from_size_bits(size_bits)
    }
}

impl From<FrameSize> for u8 {
    fn from(size: FrameSize) -> Self {
        size.size_bits()
    }
}
