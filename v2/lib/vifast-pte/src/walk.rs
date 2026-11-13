// SPDX-License-Identifier: MIT OR Apache-2.0
//! Hình ba cấp của VA 39 bit (K2-PLAN Q2, D2-65).
//!
//! VA 39 bit chia `9 / 9 / 9 / 12`: cấp 2 (gốc) = bit `38..30`, cấp 1 = bit `29..21`,
//! cấp 0 = bit `20..12`, offset = bit `11..0`. Mỗi bảng có `2^9 = 512` entry.
//!
//! [`walk_shape`] **chỉ** tách hình: không đọc bộ nhớ, không kiểm canonical/alignment, không
//! panic — kể cả `u64::MAX`. Bit cao hơn `VA_BITS` bị **cắt** (mặt nạ 9 bit mỗi cấp), đúng quy
//! ước của KT-35: `va = 1 << 38` cho đúng ba chỉ số `[256, 0, 0]`. Việc kiểm `RangeError` /
//! canonical là của KT-37.

use crate::{PAGE_SIZE_BITS, PTE_LEVELS};

/// Số bit chỉ số của một cấp (`2^9 = 512` entry/bảng).
const INDEX_BITS: u32 = 9;

/// Mặt nạ chỉ số một cấp.
const INDEX_MASK: u64 = (1u64 << INDEX_BITS) - 1;

/// Vị trí bit của chỉ số cấp 0 (cấp lá 4 KiB) — suy từ `PAGE_SIZE_BITS[0]`, không chép tay.
const SHIFT_L0: u32 = PAGE_SIZE_BITS[0] as u32;

/// Vị trí bit của chỉ số cấp 1.
const SHIFT_L1: u32 = SHIFT_L0 + INDEX_BITS;

/// Vị trí bit của chỉ số cấp 2 (gốc).
const SHIFT_L2: u32 = SHIFT_L1 + INDEX_BITS;

/// Hình của một VA: đúng [`PTE_LEVELS`] chỉ số, mỗi chỉ số `0..512`.
///
/// Thứ tự `[cấp 2, cấp 1, cấp 0]` = `[gốc, giữa, lá]`. Với VA 39 bit, ba chỉ số cộng offset
/// 12 bit dựng lại **đúng** phần VA đã cắt (xem test `walk_shape_property_10k_va`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WalkShape {
    indices: [u16; PTE_LEVELS],
}

impl WalkShape {
    /// Chỉ số của cả ba cấp, `[cấp 2, cấp 1, cấp 0]`.
    pub const fn indices(self) -> [u16; PTE_LEVELS] {
        self.indices
    }

    /// Số chỉ số (luôn bằng [`PTE_LEVELS`]).
    pub const fn len(self) -> usize {
        PTE_LEVELS
    }

    /// Luôn `false`: hình VA 39 bit không bao giờ rỗng.
    pub const fn is_empty(self) -> bool {
        false
    }

    /// Chỉ số cấp 2 (bảng gốc), bit `38..30`.
    pub const fn level2(self) -> u16 {
        self.indices[0]
    }

    /// Chỉ số cấp 1, bit `29..21`.
    pub const fn level1(self) -> u16 {
        self.indices[1]
    }

    /// Chỉ số cấp 0 (bảng lá 4 KiB), bit `20..12`.
    pub const fn level0(self) -> u16 {
        self.indices[2]
    }
}

impl Default for WalkShape {
    fn default() -> Self {
        WalkShape {
            indices: [0; PTE_LEVELS],
        }
    }
}

/// Tách VA thành ba chỉ số 9 bit (cấp 2, cấp 1, cấp 0) — hàm `const`, không panic.
///
/// Không kiểm canonical, không kiểm căn lề: chỉ là **hình**. Bit `>= VA_BITS` bị cắt bởi mặt
/// nạ (`INDEX_MASK`) nên mọi `u64` đều trả về hình hợp lệ.
pub const fn walk_shape(va: u64) -> WalkShape {
    WalkShape {
        indices: [
            ((va >> SHIFT_L2) & INDEX_MASK) as u16,
            ((va >> SHIFT_L1) & INDEX_MASK) as u16,
            ((va >> SHIFT_L0) & INDEX_MASK) as u16,
        ],
    }
}
