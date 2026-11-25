// SPDX-License-Identifier: MIT OR Apache-2.0
//! **`Plan`** — bộ tính **bố cục** bump cho arena động (`KT-410`, `D2-259` mục 5).
//!
//! # Vì sao có tệp này
//!
//! [`crate::Arena`] là bump allocator **thật**: nó giữ `*mut u8` nên `new`/`alloc`/`reset` đều
//! `unsafe` và chỉ dùng được khi đã có vùng nhớ. Nhưng tầng **chính sách** (cắt arena từ
//! `BootInfo`, kiểm "có đủ chỗ cho `Machine<UntypedStore>` không", tính offset từng bảng) phải
//! chạy được **trên host, không `unsafe`** — đó là toàn bộ lý do `vifast-plat-common` giữ
//! `#![forbid(unsafe_code)]`.
//!
//! [`Plan`] là **nửa an toàn** của cùng thuật toán: nó tính **offset** đúng căn lề trên một
//! ngân sách byte, **không** giữ con trỏ, **không** `unsafe`. Tầng có `physmap` gọi
//! [`Plan::alloc`] để lấy offset rồi tự cộng vào base vật lý. Nhờ vậy:
//!
//! - cùng **một** công thức căn lề cho cả host test và bare-metal (không lệch);
//! - `Plan` kiểm **tràn** (`checked_add`) và **hết chỗ** (`None`) — fail-closed, không panic;
//! - test host chứng minh được "arena 617.208 B vừa đúng `Machine<UntypedStore>`" mà không cần
//!   một byte RAM thật nào.
//!
//! # Hợp đồng
//!
//! - `alloc(size, align)` trả offset **tương đối so với base** của arena; `align` phải là luỹ
//!   thừa của 2 (kiểm bằng `debug_assert` + `align == 0` ⇒ `None`).
//! - Căn lề tính **tương đối so với base** — caller phải bảo đảm `base` căn ít nhất `align`
//!   (cùng hợp đồng với [`crate::Arena`]).
//! - Không giải phóng từng block: arena động của mô hình General cắt **một lần** lúc boot.

/// Bộ tính bố cục bump **an toàn** trên một ngân sách byte (không con trỏ, không `unsafe`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plan {
    capacity: usize,
    cursor: usize,
}

impl Plan {
    /// Kế hoạch rỗng trên `capacity` byte.
    #[must_use]
    pub const fn new(capacity: usize) -> Self {
        Self {
            capacity,
            cursor: 0,
        }
    }

    /// Ngân sách byte của cả vùng.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Số byte đã cấp (gồm đệm căn lề).
    #[must_use]
    pub const fn used(&self) -> usize {
        self.cursor
    }

    /// Số byte còn lại.
    #[must_use]
    pub const fn free(&self) -> usize {
        self.capacity - self.cursor
    }

    /// Hỏi offset cấp được cho `size` byte căn `align` — **không** tiêu byte.
    ///
    /// Trả `None` khi `align == 0`, `align` không phải luỹ thừa 2, tràn `usize`, hoặc hết chỗ.
    #[must_use]
    pub fn probe(&self, size: usize, align: usize) -> Option<usize> {
        if align == 0 || !align.is_power_of_two() {
            return None;
        }
        let aligned = self.cursor.checked_add(align - 1)? & !(align - 1);
        let end = aligned.checked_add(size)?;
        (end <= self.capacity).then_some(aligned)
    }

    /// Cấp `size` byte căn `align`; trả offset tương đối so với base, hoặc `None` (không panic).
    pub fn alloc(&mut self, size: usize, align: usize) -> Option<usize> {
        let at = self.probe(size, align)?;
        self.cursor = at.checked_add(size)?;
        Some(at)
    }

    /// Đưa watermark về 0 (dựng lại bố cục từ đầu).
    pub fn reset(&mut self) {
        self.cursor = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_can_le_va_het_cho_tra_none() {
        let mut p = Plan::new(64);
        assert_eq!(p.alloc(1, 1), Some(0));
        assert_eq!(p.alloc(4, 4), Some(4));
        assert_eq!(p.alloc(8, 8), Some(8));
        assert_eq!(p.used(), 16);
        assert_eq!(p.free(), 48);
        // Hết chỗ ⇒ None, **không** tiêu byte.
        assert_eq!(p.alloc(49, 1), None);
        assert_eq!(p.used(), 16);
        // Căn lề sai ⇒ None (fail-closed).
        assert_eq!(p.alloc(1, 0), None);
        assert_eq!(p.alloc(1, 3), None);
        // `probe` không tiêu byte.
        assert_eq!(p.probe(8, 8), Some(16));
        assert_eq!(p.used(), 16);
        p.reset();
        assert_eq!(p.used(), 0);
    }

    #[test]
    fn plan_tran_usize_tra_none() {
        let mut p = Plan::new(usize::MAX);
        assert_eq!(p.alloc(1, 1), Some(0));
        assert_eq!(p.probe(usize::MAX, 1), None, "tràn ⇒ None");
        // Căn lề khổng lồ nhưng vẫn vừa ngân sách ⇒ **được** (không phải lỗi tràn).
        assert_eq!(
            p.probe(1, 1 << (usize::BITS - 1)),
            Some(1 << (usize::BITS - 1))
        );
    }
}
