// SPDX-License-Identifier: MIT OR Apache-2.0
//! Bump allocator thô + căn lề trên vùng người gọi cấp (QD §3.5: **không** `Vec<u8>`).

use core::alloc::Layout;
use core::ptr::NonNull;

/// Bump allocator thô: `alloc(Layout)` cấp vùng đúng căn lề, hết chỗ trả `None`.
///
/// Không giải phóng từng block (Untyped thu hồi bằng `Revoke` → [`Arena::reset`]);
/// `destroy` của object không trả bộ nhớ cho arena.
pub struct Arena {
    base: *mut u8,
    len: usize,
    cursor: usize,
}

impl Arena {
    /// # Safety
    /// `base..base + len` phải là vùng nhớ hợp lệ, ghi được, và không có tham chiếu
    /// nào khác truy cập vùng đó trong thời gian `Arena` sống.
    pub unsafe fn new(base: *mut u8, len: usize) -> Self {
        Self {
            base,
            len,
            cursor: 0,
        }
    }

    /// Cấp `layout.size()` byte căn lề `layout.align()`; hết chỗ trả `None` (không panic).
    pub fn alloc(&mut self, layout: Layout) -> Option<NonNull<u8>> {
        let align = layout.align();
        let aligned = self.cursor.checked_add(align - 1)? & !(align - 1);
        let end = aligned.checked_add(layout.size())?;
        if end > self.len {
            return None;
        }
        self.cursor = end;
        // SAFETY: `new` bảo đảm `base..base + len` hợp lệ; `aligned < end <= len`.
        let ptr = unsafe { self.base.add(aligned) };
        NonNull::new(ptr)
    }

    /// `pa..pa + len` có nằm trọn trong vùng đã cấp lúc `new` không (không cấp phát gì).
    ///
    /// Dùng để chặn `pa` giả/ngoài vùng trước khi đọc header (KT-15).
    pub(crate) fn chua(&self, pa: u64, len: usize) -> bool {
        let base = self.base as usize;
        let Ok(start) = usize::try_from(pa) else {
            return false;
        };
        let Some(end) = start.checked_add(len) else {
            return false;
        };
        let Some(vung_end) = base.checked_add(self.len) else {
            return false;
        };
        start >= base && end <= vung_end
    }

    /// Số byte đã cấp (gồm padding căn lề).
    #[must_use]
    pub fn used(&self) -> usize {
        self.cursor
    }

    /// Tổng số byte của vùng được cấp lúc `new`.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.len
    }

    /// Đưa watermark về 0 (Untyped `Revoke` đã finalise mọi object con trước đó).
    ///
    /// # Safety
    /// Chỉ gọi khi mọi object trong arena đã `destroy`; mọi `ObjRef` cũ trở nên vô hiệu.
    pub unsafe fn reset(&mut self) {
        self.cursor = 0;
    }
}
