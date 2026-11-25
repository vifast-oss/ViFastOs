// SPDX-License-Identifier: MIT OR Apache-2.0
//! **`HeapTable<T>`** — bảng **cấp phát động** dùng được làm bảng của `Machine` (`KT-410`, `B2`).
//!
//! # Vì sao ở đây
//!
//! [`vifast_core::store::KernelStore`] đòi mỗi bảng là `Copy + Deref<Target = [T]> + DerefMut`.
//! Bảng trên `alloc` **không thể** là `Copy` nếu nó *sở hữu* `Vec` — nhưng **có thể** nếu nó chỉ
//! giữ `(con trỏ thô, độ dài)`. Đó là `HeapTable`: **16 B**, `Copy`, `Deref` về `[T]`.
//!
//! Việc dựng/giải phóng vùng là `unsafe` ⇒ phải nằm trong crate thuộc danh sách đóng `D2-17`
//! (`vifast-objmem`), **không** trong `vifast-core` (`#![forbid(unsafe_code)]`).
//!
//! # Hợp đồng
//!
//! - [`HeapTable::EMPTY`] là bảng **rỗng hợp lệ** (`len = 0`, con trỏ dangling) — `Deref` trả
//!   slice rỗng, **không** UB.
//! - [`HeapTable::alloc_zeroed`] cấp `len` phần tử **đã zero** (mọi bit 0). Với `T` mà "zero" là
//!   giá trị hợp lệ (`u16`, `u32`, `Meta`/`Cap`/`TcbData`… đều có `EMPTY` toàn-0 hoặc được ghi
//!   đè trước khi đọc) đây là trạng thái rỗng của mô hình.
//! - Bảng **không** tự giải phóng (`Copy` không có `Drop`): caller gọi [`HeapTable::free`] **một
//!   lần** cho mỗi bảng đã cấp, sau khi `Machine` không còn dùng.
//! - `HeapTable` **không** mang identity: hai bảng cùng `(ptr, len)` là **cùng** vùng nhớ. Chép
//!   `HeapTable` là **chia sẻ** vùng, không nhân bản dữ liệu — đúng ngữ nghĩa `Copy` của
//!   `KernelStore` (mọi bảng của `Machine` là `Copy`).
//! - `len` phải khớp cỡ đã cấp; `Deref` dựng `slice::from_raw_parts(ptr, len)` nên `len` sai ⇒ UB.
//!   Đây là hợp đồng của người gọi, giống `Arena::new`.

use core::alloc::Layout;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;
use core::slice;

/// Bảng `Copy` trên vùng `alloc` — `(con trỏ thô, độ dài)`.
///
/// `#[repr(C)]` để `(ptr, len)` có bố cục xác định (chẩn đoán/kích thước 16 B trên 64-bit).
#[repr(C)]
pub struct HeapTable<T> {
    ptr: *mut T,
    len: usize,
}

// SAFETY: `HeapTable` chỉ là `(ptr, len)`; `Send`/`Sync` phụ thuộc `T` như `&[T]`/`&mut [T]`.
unsafe impl<T: Send> Send for HeapTable<T> {}
// SAFETY: xem trên.
unsafe impl<T: Sync> Sync for HeapTable<T> {}

impl<T> Clone for HeapTable<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for HeapTable<T> {}

impl<T> core::fmt::Debug for HeapTable<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("HeapTable")
            .field("len", &self.len)
            .finish_non_exhaustive()
    }
}

impl<T> HeapTable<T> {
    /// Bảng **rỗng hợp lệ** (`len = 0`, con trỏ dangling) — dùng được trong `const`.
    pub const EMPTY: Self = Self {
        ptr: NonNull::<T>::dangling().as_ptr(),
        len: 0,
    };

    /// Bọc một vùng đã cấp `(ptr, len)` — **không** cấp phát, **không** kiểm.
    ///
    /// # Safety
    /// `ptr..ptr + len` phải là vùng `T` hợp lệ, ghi được, sống ít nhất bằng mọi `Machine` dùng
    /// bảng này, và **không** bị `free` hai lần.
    pub const unsafe fn from_raw_parts(ptr: *mut T, len: usize) -> Self {
        Self { ptr, len }
    }

    /// Con trỏ tới phần tử đầu (dangling khi `len == 0`).
    #[must_use]
    pub const fn as_ptr(&self) -> *mut T {
        self.ptr
    }

    /// Số phần tử.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Bảng rỗng?
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl<T: Copy> HeapTable<T> {
    /// Cấp `len` phần tử **đã zero** trên heap; `len == 0` ⇒ [`HeapTable::EMPTY`].
    ///
    /// # Panics
    /// `len * size_of::<T>()` vượt `isize::MAX`, hoặc allocator từ chối.
    #[must_use]
    pub fn alloc_zeroed(len: usize) -> Self {
        if len == 0 {
            return Self::EMPTY;
        }
        let layout = Layout::array::<T>(len).expect("HeapTable: layout tràn");
        // SAFETY: `layout` có `size > 0` (vì `len > 0` và `size_of::<T>() > 0`; nếu `T` là ZST thì
        // `Layout::array` cho size 0 ⇒ `alloc` trả null ⇒ panic có kiểm soát bên dưới).
        let raw = unsafe { alloc::alloc::alloc_zeroed(layout) };
        let ptr = NonNull::new(raw.cast::<T>())
            .unwrap_or_else(|| alloc::alloc::handle_alloc_error(layout));
        Self {
            ptr: ptr.as_ptr(),
            len,
        }
    }

    /// Giải phóng vùng đã cấp bởi [`HeapTable::alloc_zeroed`].
    ///
    /// # Safety
    /// `self` phải do [`HeapTable::alloc_zeroed`] trả về (hoặc [`HeapTable::from_raw_parts`] với
    /// vùng `alloc` cùng `Layout`), **chưa** được `free`, và không còn ai dùng vùng này.
    pub unsafe fn free(self) {
        if self.len == 0 {
            return;
        }
        let layout = Layout::array::<T>(self.len).expect("HeapTable: layout tràn");
        // SAFETY: điều kiện của `free`.
        unsafe { alloc::alloc::dealloc(self.ptr.cast::<u8>(), layout) };
    }
}

impl<T> Deref for HeapTable<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        // SAFETY: hợp đồng của `HeapTable` (xem đầu tệp): `ptr..ptr + len` là vùng `T` hợp lệ.
        unsafe { slice::from_raw_parts(self.ptr, self.len) }
    }
}

impl<T> DerefMut for HeapTable<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        // SAFETY: như `deref`; `&mut self` loại trừ truy cập đồng thời qua chính giá trị này.
        unsafe { slice::from_raw_parts_mut(self.ptr, self.len) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heap_table_rong_hop_le() {
        let t: HeapTable<u32> = HeapTable::EMPTY;
        assert_eq!(t.len(), 0);
        assert!(t.is_empty());
        assert_eq!(&*t, &[] as &[u32], "Deref trên bảng rỗng trả slice rỗng");
        assert_eq!(core::mem::size_of::<HeapTable<u32>>(), 16, "ptr + len");
    }

    #[test]
    fn heap_table_cap_ghi_doc_va_giai_phong() {
        let mut t: HeapTable<u32> = HeapTable::alloc_zeroed(64);
        assert_eq!(t.len(), 64);
        assert!(t.iter().all(|&v| v == 0), "alloc_zeroed phải zero");
        t[0] = 7;
        t[63] = 9;
        assert_eq!(t[0], 7);
        assert_eq!(t[63], 9);
        // `Copy` là **chia sẻ** vùng, không nhân bản dữ liệu.
        let u = t;
        assert_eq!(u[0], 7, "bản sao thấy cùng dữ liệu");
        assert_eq!(u.as_ptr(), t.as_ptr(), "cùng con trỏ");
        // SAFETY: `t` do `alloc_zeroed` cấp, chưa free, không còn dùng.
        unsafe { t.free() };
    }

    #[test]
    fn heap_table_len_0_khong_cap_phat() {
        let t: HeapTable<u64> = HeapTable::alloc_zeroed(0);
        assert!(t.is_empty());
        // SAFETY: bảng rỗng ⇒ `free` là no-op.
        unsafe { t.free() };
    }
}
