// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-347 (U1a)** — lõi **bump allocator** thuần an toàn cho `vifast-rt`.
//!
//! # Vì sao tách khỏi `lib.rs`
//!
//! phần `unsafe` của `GlobalAlloc::alloc` phải nằm trong danh sách đóng D2-17.
//! Shim ABI U-mode `v2/user/runtime/lib.rs` **đã** nằm trong danh sách đó (D2-253) nên phần
//! phần `unsafe` của `GlobalAlloc` sống ở đó; ở đây chỉ còn **số học offset trên một vùng đệm**, không
//! con trỏ, không `unsafe` ⇒ kiểm được bằng `rustc --test` trên **host** (D2-05: test đỏ trước).
//!
//! # Hợp đồng
//!
//! `Bump::new(cap)` quản lý `cap` byte. `alloc(size, align)` trả **offset** của vùng đã cấp
//! (đã căn `align`) hoặc `None` khi **không đủ chỗ** (fail-closed: caller phải báo lỗi có tên —
//! không bao giờ trả offset nằm ngoài `cap`, không tràn số, không cấp chồng lấn).
//! `align` phải là luỹ thừa của 2 và > 0; `size = 0` vẫn tiêu 0 byte (trả offset hiện tại đã căn).

/// Bộ cấp phát bump: con trỏ tăng một chiều, không thu hồi từng khối.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Bump {
    /// Offset kế tiếp (đã cấp tới đâu).
    offset: usize,
    /// Tổng dung lượng vùng đệm.
    cap: usize,
}

#[allow(dead_code)] // shim chỉ dùng `restore`/`alloc`; host test và `vifast-rt` dùng cả bộ API.
impl Bump {
    /// Bộ cấp phát rỗng trên `cap` byte.
    pub const fn new(cap: usize) -> Self {
        Self { offset: 0, cap }
    }

    /// Đặt lại về đầu vùng (dùng cho test; runtime cấp một chiều).
    pub fn reset(&mut self) {
        self.offset = 0;
    }

    /// Dựng lại trạng thái từ `used` — dùng khi offset nằm trong biến nguyên tử (`GlobalAlloc`).
    ///
    /// `None` ⇔ `used > cap` (trạng thái không hợp lệ ⇒ caller fail-closed, không cắt bớt im lặng).
    pub const fn restore(cap: usize, used: usize) -> Option<Self> {
        if used <= cap {
            Some(Self { offset: used, cap })
        } else {
            None
        }
    }

    /// Số byte đã cấp (tính cả phần đệm căn lề).
    pub const fn used(&self) -> usize {
        self.offset
    }

    /// Tổng dung lượng.
    pub const fn cap(&self) -> usize {
        self.cap
    }

    /// Số byte còn lại (chặn dưới 0).
    pub const fn remaining(&self) -> usize {
        self.cap.saturating_sub(self.offset)
    }

    /// Cấp `size` byte căn `align`; trả `offset` hoặc `None` nếu không đủ chỗ.
    ///
    /// Không `unsafe`, không con trỏ: caller (`v2/user/runtime/lib.rs`) cộng offset vào địa chỉ
    /// vùng đệm. `align` không phải luỹ thừa 2 ⇒ `None` (fail-closed, không panic).
    pub fn alloc(&mut self, size: usize, align: usize) -> Option<usize> {
        if align == 0 || !align.is_power_of_two() {
            return None;
        }
        // Căn offset hiện tại lên bội số `align` — `checked_add` chặn tràn số.
        let aligned = self.offset.checked_add(align - 1)? & !(align - 1);
        let end = aligned.checked_add(size)?;
        if end > self.cap {
            return None;
        }
        self.offset = end;
        Some(aligned)
    }
}

#[cfg(test)]
mod tests {
    use super::Bump;

    #[test]
    fn cap_rong_khong_cap_duoc_byte_nao() {
        let mut b = Bump::new(0);
        assert_eq!(b.alloc(1, 1), None);
        assert_eq!(b.used(), 0);
    }

    #[test]
    fn cap_lien_tiep_dung_offset_va_can_le() {
        let mut b = Bump::new(64);
        assert_eq!(b.alloc(1, 1), Some(0));
        assert_eq!(b.alloc(1, 1), Some(1));
        // 2 → căn 8 ⇒ 8, chiếm 8..16.
        assert_eq!(b.alloc(8, 8), Some(8));
        assert_eq!(b.used(), 16);
        assert_eq!(b.remaining(), 48);
    }

    #[test]
    fn vuot_cap_tra_none_va_khong_doi_offset() {
        let mut b = Bump::new(10);
        assert_eq!(b.alloc(6, 1), Some(0));
        // 6 → còn 4 byte, xin 5 ⇒ None và offset **không** đổi.
        assert_eq!(b.alloc(5, 1), None);
        assert_eq!(b.used(), 6);
    }

    #[test]
    fn can_le_lam_tieu_them_byte_nhung_van_dung_cap() {
        let mut b = Bump::new(9);
        assert_eq!(b.alloc(1, 1), Some(0));
        // 1 → căn 8 ⇒ 8, xin 1 ⇒ end = 9 = cap ⇒ vừa khít.
        assert_eq!(b.alloc(1, 8), Some(8));
        assert_eq!(b.alloc(1, 1), None);
    }

    #[test]
    fn align_khong_hop_le_fail_closed() {
        let mut b = Bump::new(64);
        assert_eq!(b.alloc(4, 0), None);
        assert_eq!(b.alloc(4, 3), None);
        assert_eq!(b.used(), 0);
    }

    #[test]
    fn size_0_khong_tieu_byte_va_van_can_le() {
        let mut b = Bump::new(16);
        assert_eq!(b.alloc(1, 1), Some(0));
        let at = b.alloc(0, 8);
        assert_eq!(at, Some(8));
        assert_eq!(b.used(), 8);
    }

    #[test]
    fn cap_lon_khong_tran_so() {
        let mut b = Bump::new(usize::MAX);
        assert_eq!(b.alloc(usize::MAX, 1), Some(0));
        // Đã dùng hết; xin thêm 1 ⇒ None (không tràn `end`).
        assert_eq!(b.alloc(1, 1), None);
    }

    #[test]
    fn reset_cho_lai_tu_dau() {
        let mut b = Bump::new(16);
        assert_eq!(b.alloc(16, 1), Some(0));
        assert_eq!(b.alloc(1, 1), None);
        b.reset();
        assert_eq!(b.alloc(16, 1), Some(0));
    }
}
