// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-77c** — ô `static` **một lần ghi** cho một object lớn, cấp phát trong `.bss` của ảnh.
//!
//! # Vì sao cần
//!
//! `Fs` của `vifast-fs` là một struct ≈ 13 KiB (64 inode + bitmap + journal). Dựng nó **trên
//! stack** qua nhiều tầng trả-về-theo-giá-trị (open → mount → empty → sret slot) làm tràn guard
//! page 64 KiB — **đúng lỗi đã đo ở KT-75** (`ktest_persist.rs:915`: "nhét thêm 16 KiB cache vào
//! cùng khung đã tràn guard page 64 KiB"). KT-75 chữa bằng cách tách khung; ở runtime shell của
//! ảnh **sản phẩm** thì `Fs` phải **sống suốt** vòng đọc lệnh, nên cách chắc chắn là để nó trong
//! `.bss` và chỉ chuyển **tham chiếu**.
//!
//! # Vì sao `unsafe` nằm ở **crate này** (không phải `vifast-kbin-riscv64`)
//!
//! Luật R5 (`v2/bin/v2`) chỉ cho `unsafe` trong danh sách đóng D2-17: `kernel/vifast-plat-*`,
//! `boot/`, `vifast-sync`, `vifast-objmem`. `vifast-kbin-riscv64` **không** thuộc danh sách ⇒ ô
//! `static` này là API **an toàn** cho crate đó, `unsafe` nằm gọn ở đây (cùng khuôn với
//! `core_bridge::MACHINE` — `static mut MaybeUninit<Machine>`).
//!
//! # An toàn
//!
//! - Chỉ **một** lần ghi: `taken` chuyển `false → true` bằng `compare_exchange`; lần thứ hai trả
//!   `None` (không ghi đè, không có hai `&mut` cùng lúc).
//! - Người dùng là runtime kernel **tuần tự trên BSP** (hart phụ park trong `rust_secondary_main`),
//!   đúng giả định đã dùng cho `Machine` (D2-84) — không có truy cập song song.

use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, Ordering};

/// Ô `static` chứa **một** `T` dựng một lần, trả `&'static mut T`.
pub struct StaticSlot<T> {
    inner: UnsafeCell<MaybeUninit<T>>,
    taken: AtomicBool,
}

// SAFETY: xem khối "An toàn" ở đầu tệp — một lần ghi, một người dùng tuần tự trên BSP.
unsafe impl<T: Send> Sync for StaticSlot<T> {}

impl<T> Default for StaticSlot<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> StaticSlot<T> {
    /// Ô chưa có giá trị — dùng được trong `static` (không cấp phát, `.bss`).
    pub const fn new() -> Self {
        Self {
            inner: UnsafeCell::new(MaybeUninit::uninit()),
            taken: AtomicBool::new(false),
        }
    }

    /// Dựng `T` **một lần** bằng `make` rồi trả `&'static mut T`.
    ///
    /// `None` ⇔ `make` trả `None` (giữ ô trống cho lần sau) hoặc ô đã được dựng trước đó.
    pub fn init_with(&self, make: impl FnOnce() -> Option<T>) -> Option<&'static mut T> {
        if self
            .taken
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return None;
        }
        let value = match make() {
            Some(value) => value,
            None => {
                self.taken.store(false, Ordering::SeqCst);
                return None;
            }
        };
        // SAFETY: `taken` vừa chuyển false→true nên chỉ nhánh này ghi; `UnsafeCell` cho phép ghi
        // qua `&self`; sau khi ghi, `assume_init_mut` hợp lệ vì giá trị đã được khởi tạo đủ.
        unsafe {
            let slot = &mut *self.inner.get();
            slot.write(value);
            Some(slot.assume_init_mut())
        }
    }

    /// `true` ⇔ ô đã có giá trị (chẩn đoán, không cấp quyền truy cập).
    pub fn is_filled(&self) -> bool {
        self.taken.load(Ordering::SeqCst)
    }
}

/// **KT-347** — ô `static` **tái dùng** cho buffer scratch tuần tự (ví dụ byte ELF đọc từ FS).
///
/// Khác [`StaticSlot`] (một lần ghi): ô này dùng lại được nhiều lượt. Bất biến an toàn: chỉ
/// **một** người dùng tuần tự (vòng lặp chính của kernel trên BSP — cùng giả định D2-84 của
/// `Machine`) và **không** giữ hai `&mut` cùng lúc; `borrow` trả `&mut` trong thời gian ngắn.
pub struct Scratch<T> {
    inner: UnsafeCell<T>,
}

// SAFETY: xem khối "bất biến an toàn" ở trên — một người dùng tuần tự trên BSP.
unsafe impl<T: Send> Sync for Scratch<T> {}

impl<T> Scratch<T> {
    /// Ô chứa `value` — dùng được trong `static` (không cấp phát).
    pub const fn new(value: T) -> Self {
        Self {
            inner: UnsafeCell::new(value),
        }
    }

    /// Mượn **tạm** `&mut T` (bất biến: một người dùng tuần tự, không giữ lâu).
    ///
    /// Tên `borrow_mut` (không phải `borrow`) để không bị nhầm với `core::borrow::Borrow`.
    /// `clippy::mut_from_ref` là **chủ ý** của kiểu này: `Scratch<T>` tồn tại đúng để cho
    /// một `static` (không `static mut`) trả `&mut T` theo bất biến "một người dùng tuần tự
    /// trên BSP" — cùng lý do `UnsafeCell` tồn tại; `Sync` được `unsafe impl` có ghi rõ
    /// khối an toàn ở đầu tệp.
    #[allow(clippy::mut_from_ref)]
    pub fn borrow_mut(&self) -> &mut T {
        // SAFETY: xem khối "bất biến an toàn" ở trên.
        unsafe { &mut *self.inner.get() }
    }
}
