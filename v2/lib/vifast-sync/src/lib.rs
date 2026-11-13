// SPDX-License-Identifier: MIT OR Apache-2.0
//! `vifast-sync` — đồng bộ cấp thấp của kernel: **MCS queue lock** + **hộp thư MPSC cố định**
//! (KT-30, K1C-PLAN §5; thiết kế §4.16 `:407` và §4.17 `:423-428`).
//!
//! Crate nằm trong danh sách **đóng** được dùng `unsafe` của D2-17 (thiết kế §8 `:684,695`);
//! Toàn bộ `unsafe` (8 khối): `Deref`/`DerefMut` của [`mcs::Guard`], 2 chỗ ghi/đọc slot
//! `MaybeUninit` của [`Mailbox`], và 4 `unsafe impl Send/Sync` của hai kiểu đồng bộ. Không có
//! `unsafe fn`, không con trỏ thô, không cfg theo kiến trúc.
//!
//! **no_std** (D2-42): chỉ dùng `core::sync::atomic` + `core::cell::UnsafeCell`; **không**
//! dependency ngoài, không cấp phát (D2-21). Test dùng `std` (crate test riêng) nên crate này
//! không cần feature `std`.
//!
//! Hai kiểu, hai vai trò **khác nhau** — không tuyên bố "lock-free" cho cả crate:
//!
//! | Kiểu | Vai trò | Tính chất |
//! |---|---|---|
//! | [`McsLock`] | BKL dạng **khoá** xếp hàng (S1) | **khoá**, FIFO theo thứ tự xếp hàng |
//! | [`Mailbox`] | hộp thư theo hart (S2) | **lock-free** (MPSC cố định, đầy ⇒ `Busy`) |
//!
//! [`Msg`] đủ **7 loại** của thiết kế §4.17: `Wake/TlbFlush/Reschedule/Migrate/Stop/VmKick/PowerReq`.
#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]
//!
//! Tài liệu mô hình: `v2/docs/k1c-sync.md` (`[TK]`).

mod mailbox;
mod mcs;

pub use mailbox::{Mailbox, Msg, MsgKind, PowerState};
pub use mcs::{Guard, McsLock, MAX_WAITERS};

/// Hộp thư **đầy**: message mới bị từ chối, message cũ còn nguyên (thiết kế §4.17: "đầy ⇒
/// người gửi xử lý hộp thư của mình rồi thử lại" — **backpressure**, không ghi đè, không mất).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Busy;

impl core::fmt::Display for Busy {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("hộp thư đầy (backpressure: xử lý hộp thư của mình rồi thử lại)")
    }
}
