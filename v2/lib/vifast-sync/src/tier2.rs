// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Hộp thư tầng 2** — `Tier2Mailbox<N>` trên [`Mailbox<N>`] (KT-415, D2-259 mục 6).
//!
//! # Hai tầng, hai nhiệm vụ
//!
//! | Tầng | Kiểu | Nói gì | Đầy? |
//! |---|---|---|---|
//! | 1 — chuông | `vifast_plat_common::ipi::IpiMailbox` | "hart này **có việc**" (bitmask 3 kind) | **không bao giờ** (coalesce) |
//! | 2 — payload | [`Tier2Mailbox<N>`] (bọc [`Mailbox<N>`]) | "việc **gì**" (7 loại message có payload) | **có** (`N` slot) |
//!
//! Tầng 1 một mình **không đủ** để biết phải đánh thức TCB nào / xả VA nào; tầng 2 một mình
//! **không đủ** để bảo đảm wake (đầy ⇒ `Busy`). Tầng 2 ở đây bổ sung đúng một thứ mà
//! [`Mailbox`] thiếu: **cờ `OVERFLOW`** — khi ring đầy, message **không** vào ring nhưng cờ bật,
//! và người nhận **bắt buộc** rơi về hành động **bảo thủ toàn phần**:
//!
//! - `Msg::Wake` ⇒ **full-rescan** scheduler (quét lại mọi TCB runnable của hart);
//! - `Msg::TlbFlush` ⇒ **full-flush** TLB (xoá toàn bộ, không chỉ một VA).
//!
//! # Vì sao **0 mất wake** (lập luận, không phải lời hứa)
//!
//! Mỗi `post` có **đúng hai** kết cục đo được: [`PostOutcome::Delivered`] (message nằm trong
//! ring) hoặc [`PostOutcome::Overflow`] (cờ `OVERFLOW` bật). Bất biến
//! `delivered + overflowed == posted` là **bảo toàn** — không có nhánh nào nuốt message mà
//! không để lại dấu. Người nhận gọi [`Tier2Mailbox::take`] mỗi lần được chuông tầng 1 đánh thức;
//! `take` rút **hết** ring rồi đọc-và-xoá cờ. Vì vậy:
//!
//! - message **vào được** ring ⇒ được giao (FIFO, không ghi đè);
//! - message **không vào được** ring ⇒ cờ bật ⇒ lượt `take` kế tiếp trả `overflowed = true` ⇒
//!   người nhận full-rescan/full-flush ⇒ wake **vẫn** xảy ra (bảo thủ, không chính xác theo từng
//!   TCB — đúng đánh đổi của tầng 2).
//!
//! # Thứ tự bộ nhớ (bắt buộc)
//!
//! Người gửi: ghi ring (`Mailbox::send` dùng `Release` trên `seq`) **rồi** đặt cờ `OVERFLOW`
//! (`Release`). Người nhận: rút ring (`Acquire`) **rồi** `swap(false, AcqRel)` cờ. Nhờ vậy cờ
//! đặt **sau** khi ring đầy được nhìn thấy ở lượt `take` **sau** lần rút cuối — không có khe
//! "đầy nhưng cờ chưa bật mà người nhận đã rút xong".
//!
//! Tầng 1 (chuông) là thứ **đánh thức**; tầng 2 **không** tự gửi IPI. Ghép hai tầng nằm ở
//! `vifast_plat_common::ipi::HartTier2` (crate kernel), nơi `post` tầng 2 luôn được theo sau bởi
//! `post` chuông — kể cả khi ring đầy.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::mailbox::{Mailbox, Msg};
use crate::Busy;

/// Kết cục một lần [`Tier2Mailbox::post`] — **chỉ hai** giá trị, nên bảo toàn kiểm được.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PostOutcome {
    /// Message đã vào ring: người nhận sẽ thấy **đúng** message này.
    Delivered,
    /// Ring đầy: message **không** vào ring, nhưng `OVERFLOW` đã bật ⇒ người nhận
    /// **full-rescan**/**full-flush** ở lượt `take` kế tiếp.
    Overflow,
}

impl PostOutcome {
    /// `true` ⇔ message nằm trong ring (không cần full-rescan cho riêng nó).
    pub const fn delivered(self) -> bool {
        matches!(self, Self::Delivered)
    }

    /// `true` ⇔ cờ `OVERFLOW` đã bật (người nhận phải làm hành động bảo thủ toàn phần).
    pub const fn overflowed(self) -> bool {
        matches!(self, Self::Overflow)
    }

    /// Tên ổn định cho log/TAP.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Delivered => "delivered",
            Self::Overflow => "overflow",
        }
    }
}

/// Kết quả một lần [`Tier2Mailbox::take`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tier2Drain {
    /// Số message đã rút khỏi ring.
    pub messages: usize,
    /// `true` ⇔ cờ `OVERFLOW` đang bật (đã đọc-và-xoá) ⇒ người nhận **phải** full-rescan.
    pub overflowed: bool,
}

/// Ảnh chụp bộ đếm tầng 2 — `Copy` để TAP/log đọc nhất quán.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tier2Snapshot {
    /// Số lần `post` (mọi kết cục).
    pub posted: u64,
    /// Số lần message **vào** ring.
    pub delivered: u64,
    /// Số lần ring đầy ⇒ cờ bật.
    pub overflowed: u64,
    /// Số lượt `take` **thấy** cờ bật (⇒ đã full-rescan).
    pub full_rescans: u64,
    /// Số message đang chiếm slot (chẩn đoán).
    pub pending: usize,
    /// Cờ `OVERFLOW` hiện tại.
    pub overflow: bool,
}

/// Hộp thư **tầng 2**: ring payload [`Mailbox<N>`] + cờ `OVERFLOW` (xem tài liệu module).
///
/// ```
/// use vifast_sync::{Msg, PostOutcome, Tier2Mailbox};
/// let mb: Tier2Mailbox<2> = Tier2Mailbox::new();
/// assert_eq!(mb.post(Msg::Stop { hart: 1 }), PostOutcome::Delivered);
/// assert_eq!(mb.post(Msg::Stop { hart: 2 }), PostOutcome::Delivered);
/// assert_eq!(mb.post(Msg::Stop { hart: 3 }), PostOutcome::Overflow);
/// let d = mb.take(|_| {});
/// assert_eq!(d.messages, 2);
/// assert!(d.overflowed);
/// ```
pub struct Tier2Mailbox<const N: usize> {
    ring: Mailbox<N>,
    overflow: AtomicBool,
    posted: AtomicU64,
    delivered: AtomicU64,
    overflowed: AtomicU64,
    full_rescans: AtomicU64,
}

impl<const N: usize> Tier2Mailbox<N> {
    /// Hộp thư rỗng — `const` để đặt trong `static`.
    ///
    /// # Panics
    ///
    /// `N` phải `≥ 2` (ràng buộc của [`Mailbox::new`]).
    pub const fn new() -> Self {
        Self {
            ring: Mailbox::new(),
            overflow: AtomicBool::new(false),
            posted: AtomicU64::new(0),
            delivered: AtomicU64::new(0),
            overflowed: AtomicU64::new(0),
            full_rescans: AtomicU64::new(0),
        }
    }

    /// Số message tối đa của ring (`N`).
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Gửi một message: vào ring nếu còn chỗ, ngược lại **bật `OVERFLOW`** (không mất wake).
    ///
    /// Không bao giờ trả lỗi "im lặng": caller **luôn** biết kết cục, và dù kết cục nào thì
    /// caller vẫn phải rung chuông tầng 1 (xem `HartTier2`).
    pub fn post(&self, msg: Msg) -> PostOutcome {
        match self.ring.send(msg) {
            Ok(()) => {
                self.posted.fetch_add(1, Ordering::Relaxed);
                self.delivered.fetch_add(1, Ordering::Relaxed);
                PostOutcome::Delivered
            }
            Err(Busy) => {
                // Cờ đặt **sau** khi biết ring đầy; `Release` để người nhận (Acquire/AcqRel ở
                // `take`) thấy nó cùng mọi ghi ring trước đó.
                self.overflow.store(true, Ordering::Release);
                self.posted.fetch_add(1, Ordering::Relaxed);
                self.overflowed.fetch_add(1, Ordering::Relaxed);
                PostOutcome::Overflow
            }
        }
    }

    /// Rút **toàn bộ** ring (FIFO) rồi đọc-và-xoá cờ `OVERFLOW`; gọi `f` cho từng message.
    ///
    /// Thứ tự **bắt buộc**: rút ring **trước**, đọc cờ **sau** — cờ do một `post` đặt trong lúc
    /// đang rút vẫn được lượt này nhìn thấy (bảo thủ: full-rescan thừa còn hơn mất wake).
    pub fn take<F: FnMut(Msg)>(&self, f: F) -> Tier2Drain {
        let messages = self.ring.drain(f);
        let overflowed = self.overflow.swap(false, Ordering::AcqRel);
        if overflowed {
            self.full_rescans.fetch_add(1, Ordering::Relaxed);
        }
        Tier2Drain {
            messages,
            overflowed,
        }
    }

    /// Cờ `OVERFLOW` hiện tại (không xoá).
    pub fn overflow(&self) -> bool {
        self.overflow.load(Ordering::Acquire)
    }

    /// Số message đang chiếm slot ring (chẩn đoán).
    pub fn pending(&self) -> usize {
        self.ring.len()
    }

    /// Ảnh chụp bộ đếm.
    pub fn snapshot(&self) -> Tier2Snapshot {
        Tier2Snapshot {
            posted: self.posted.load(Ordering::Relaxed),
            delivered: self.delivered.load(Ordering::Relaxed),
            overflowed: self.overflowed.load(Ordering::Relaxed),
            full_rescans: self.full_rescans.load(Ordering::Relaxed),
            pending: self.ring.len(),
            overflow: self.overflow.load(Ordering::Acquire),
        }
    }
}

impl<const N: usize> Default for Tier2Mailbox<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> core::fmt::Debug for Tier2Mailbox<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let snap = self.snapshot();
        f.debug_struct("Tier2Mailbox")
            .field("capacity", &N)
            .field("posted", &snap.posted)
            .field("delivered", &snap.delivered)
            .field("overflowed", &snap.overflowed)
            .field("full_rescans", &snap.full_rescans)
            .field("pending", &snap.pending)
            .field("overflow", &snap.overflow)
            .finish()
    }
}

// Bất biến hằng số: sai một trong số này thì "0 mất wake" không còn kiểm được.
const _: () = {
    assert!(PostOutcome::Delivered.delivered());
    assert!(!PostOutcome::Delivered.overflowed());
    assert!(PostOutcome::Overflow.overflowed());
    assert!(!PostOutcome::Overflow.delivered());
};
