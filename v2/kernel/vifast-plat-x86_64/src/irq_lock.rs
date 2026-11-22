// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Khoá MCS bọc lưu/tắt ngắt** — KT-336a (D2-232/D2-238).
//!
//! `vifast_sync::McsLock` **không** tự tắt ngắt. Dùng nó nguyên trạng cho tài nguyên dùng chung
//! giữa luồng thường và handler ngắt sẽ **tự deadlock**: hart đang giữ khoá bị ngắt trên chính
//! nó, handler in/lấy khoá ⇒ quay lại [`McsLock::lock`] của **cùng hart** (bẫy D2-238, KT-316 đã
//! gặp với khoá COM1). Vì vậy mọi chỗ dùng khoá cho tài nguyên đụng ngắt đi qua [`IrqMcsLock`]:
//!
//! ```text
//!   save IF → cli ──► McsLock::lock/try_lock ──► vùng găng ──► McsLock unlock ──► khôi phục IF
//! ```
//!
//! Thứ tự **bắt buộc**: tắt ngắt **trước** khi chờ khoá; nhả khoá **trước** khi bật lại ngắt.
//! Nhờ vậy không có cửa sổ "đang giữ khoá mà ngắt bật".
//!
//! # Vì sao `IF` phải **khôi phục theo giá trị đã lưu**
//!
//! Gọi từ handler ngắt (`IF = 0`) thì nhả khoá **không** được `sti` (sẽ bật ngắt giữa handler);
//! gọi từ luồng thường (`IF = 1`) thì phải `sti` lại. [`IrqState`] lưu `RFLAGS.IF` **trước** khi
//! `cli` và [`IrqGuard`]/[`IrqMcsGuard`] khôi phục đúng giá trị ấy.
//!
//! # Trần chờ **hữu hạn**
//!
//! [`IrqMcsLock::lock_irqsave`] chờ theo MCS (FIFO, không trần — dùng cho khoá lá như COM1);
//! [`IrqMcsLock::lock_irqsave_bounded`] trả `None` sau `spins` vòng — deadlock thành **đỏ có
//! chẩn đoán**, không treo im lặng (đột biến "quên `unlock` ở nhánh lỗi" bị bắt). Bộ đếm
//! `acquired`/`contended`/`in_cs`/`overlap` là **bằng chứng đo được** (TAP 34 x86_64).
//!
//! # Đột biến (chỉ để chứng minh bộ kiểm có răng)
//!
//! - `mut-no-irq` (feature): `IrqState::save_and_disable` **không** `cli` ⇒ ngắt mềm chạy ngay
//!   trong vùng găng ⇒ TAP 34 ĐỎ.
//! - `mut-leak-unlock` (feature): [`IrqMcsGuard`] bỏ nhả khoá ở **một** nhánh `Drop` (có arm) ⇒
//!   lần lấy kế tiếp chạm trần ⇒ TAP 34 ĐỎ (không treo). Chỉ là feature đột biến.
//! - `mut-no-lock` (feature): bỏ hẳn phần `McsLock` (vẫn đếm `in_cs`) ⇒ hai ngữ cảnh cùng vào
//!   vùng găng ⇒ `overlap > 0` ⇒ TAP 34 ĐỎ.
//! - **`mut-no-irq-restore`** (feature, **KT-340c**): [`IrqState::restore`] **không** `sti` ⇒ sau
//!   khi nhả khoá `RFLAGS.IF` vẫn `0` ⇒ TAP `lock: RFLAGS.IF` (43) ĐỎ (`irq_after=0`). Đây là
//!   **đột biến (ii)** mà `KT-340c` bắt buộc (D2-244: "bỏ khôi phục `RFLAGS.IF` trong unlock ⇒
//!   TAP khoá/ngắt đỏ"); cùng ý với `mut-no-irq-restore` của aarch64 (KT-338b).

use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicU32, Ordering};

#[cfg(any(feature = "mut-leak-unlock", feature = "mut-no-lock"))]
use core::sync::atomic::AtomicBool;

use vifast_sync::{Guard as McsGuard, McsLock};

/// `RFLAGS.IF` **đang bật**? — phép đọc **duy nhất** mà ca kiểm khoá/ngắt dùng
/// (`# core lock: irq_before=… irq_inside=… irq_after=…` + TAP 43).
///
/// Cùng nguồn với [`IrqState::save_and_disable`] (`asm::interrupts_enabled`), nên hai phép đo
/// "đã tắt trong vùng găng" và "đã khôi phục sau khi nhả" không thể lệch vì hai hằng khác nhau.
pub fn irq_enabled() -> bool {
    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    {
        crate::asm::interrupts_enabled()
    }
    #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
    {
        // Trên host không có `RFLAGS`: coi như ngắt đang tắt (không có `cli`/`sti`).
        false
    }
}

/// Trạng thái ngắt che được (`RFLAGS.IF`) **trước** khi vào vùng găng.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IrqState {
    was_enabled: bool,
}

impl IrqState {
    /// Lưu `RFLAGS.IF` rồi tắt ngắt (`cli`). Trên host là no-op (không có `cli`/`pushfq`).
    ///
    /// Đột biến `mut-no-irq` (feature) **không** tắt ngắt; đột biến `no-com-cli` (KT-316) giữ
    /// nguyên ngữ nghĩa cũ — cả hai chỉ dùng cho ca đột biến, không phải feature sản phẩm.
    pub fn save_and_disable() -> Self {
        #[cfg(all(target_arch = "x86_64", target_os = "none"))]
        {
            if cfg!(feature = "no-com-cli") || cfg!(feature = "mut-no-irq") {
                return IrqState { was_enabled: false };
            }
            let was_enabled = crate::asm::interrupts_enabled();
            crate::asm::cli();
            IrqState { was_enabled }
        }
        #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
        {
            IrqState { was_enabled: false }
        }
    }

    /// `RFLAGS.IF` **trước** khi `cli` (`true` ⇔ ngắt đang bật).
    pub fn was_enabled(&self) -> bool {
        self.was_enabled
    }

    /// Bật lại ngắt **chỉ khi** trước đó đang bật — khôi phục đúng trạng thái đã lưu.
    ///
    /// Đột biến `mut-no-irq-restore` (KT-340c): **bỏ** nhánh khôi phục ⇒ `IF` ở lại `0` sau khi
    /// nhả khoá ⇒ TAP `lock: RFLAGS.IF` ĐỎ (`irq_after=0`). Chỉ dùng cho kiểm.
    pub fn restore(self) {
        #[cfg(all(target_arch = "x86_64", target_os = "none"))]
        {
            if cfg!(feature = "mut-no-irq-restore") {
                return;
            }
            if self.was_enabled {
                crate::asm::sti();
            }
        }
    }
}

/// Guard **chỉ** giữ trạng thái ngắt (không giữ khoá) — dùng khi cần tắt ngắt quanh một đoạn
/// không lấy khoá. `Drop` khôi phục `IF` theo giá trị đã lưu.
pub struct IrqGuard {
    state: IrqState,
}

impl IrqGuard {
    /// Tắt ngắt, trả guard khôi phục khi `drop`.
    pub fn new() -> Self {
        IrqGuard {
            state: IrqState::save_and_disable(),
        }
    }

    /// `RFLAGS.IF` trước khi tắt.
    pub fn was_enabled(&self) -> bool {
        self.state.was_enabled()
    }
}

impl Default for IrqGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for IrqGuard {
    fn drop(&mut self) {
        self.state.restore();
    }
}

#[cfg(feature = "mut-leak-unlock")]
static MUT_LEAK_ARMED: AtomicBool = AtomicBool::new(false);

#[cfg(feature = "mut-no-lock")]
static MUT_NO_LOCK_ARMED: AtomicBool = AtomicBool::new(false);

/// Đột biến `mut-no-lock`: arm để bỏ hẳn phần khoá MCS (vẫn đếm `in_cs`) **trong phạm vi** ca
/// kiểm — không làm gãy COM1/scheduler của cả boot.
#[cfg(feature = "mut-no-lock")]
pub fn mut_arm_no_lock() {
    MUT_NO_LOCK_ARMED.store(true, Ordering::SeqCst);
}

/// Đột biến `mut-no-lock`: tắt arm sau phép đo.
#[cfg(feature = "mut-no-lock")]
pub fn mut_disarm_no_lock() {
    MUT_NO_LOCK_ARMED.store(false, Ordering::SeqCst);
}

/// `true` ⇔ đột biến `mut-no-lock` đang **armed** (chỉ có tác dụng trong phạm vi ca kiểm).
#[inline]
fn mut_no_lock() -> bool {
    #[cfg(feature = "mut-no-lock")]
    {
        MUT_NO_LOCK_ARMED.load(Ordering::SeqCst)
    }
    #[cfg(not(feature = "mut-no-lock"))]
    {
        false
    }
}

/// Đột biến `mut-leak-unlock`: arm để lần `Drop` **kế tiếp** của [`IrqMcsGuard`] bỏ nhả khoá.
///
/// Chỉ tồn tại khi bật feature đột biến; không có trong ảnh sản phẩm.
#[cfg(feature = "mut-leak-unlock")]
pub fn mut_arm_leak() {
    MUT_LEAK_ARMED.store(true, Ordering::SeqCst);
}

/// Khoá MCS + bộ đếm bằng chứng; mọi lần lấy khoá đều **tắt ngắt trước**.
pub struct IrqMcsLock<T> {
    lock: McsLock<T>,
    /// Số lần lấy khoá **thành công** (bằng chứng khoá được dùng, không phải trang trí).
    acquired: AtomicU32,
    /// Số lần phải **chờ** (try/acquire đầu tiên thất bại).
    contended: AtomicU32,
    /// Số người đang trong vùng găng — phải luôn `0` hoặc `1`.
    in_cs: AtomicU32,
    /// Số lần phát hiện **hai** người trong vùng găng — phải luôn `0`.
    overlap: AtomicU32,
}

impl<T> IrqMcsLock<T> {
    /// Khoá mới, chưa ai giữ; dùng được trong `static`/`const`.
    pub const fn new(value: T) -> Self {
        IrqMcsLock {
            lock: McsLock::new(value),
            acquired: AtomicU32::new(0),
            contended: AtomicU32::new(0),
            in_cs: AtomicU32::new(0),
            overlap: AtomicU32::new(0),
        }
    }

    /// Lấy khoá **chờ theo MCS (FIFO, không trần)**, tắt ngắt trước khi chờ. Trả guard; `Drop`
    /// nhả khoá rồi khôi phục `IF`.
    pub fn lock_irqsave(&self) -> IrqMcsGuard<'_, T> {
        let irq = IrqState::save_and_disable();
        // Thử nhanh trước để **đếm** tương tranh; thất bại thì mới xếp hàng MCS (FIFO, không
        // trần). Cửa sổ giữa hai lệnh là vô hại: `lock()` vẫn acquire đúng.
        if let Some(guard) = self.lock.try_lock() {
            return self.enter(Some(guard), irq);
        }
        self.contended.fetch_add(1, Ordering::Relaxed);
        let guard = self.lock.lock();
        self.enter(Some(guard), irq)
    }

    /// Lấy khoá với **trần chờ hữu hạn** `spins` vòng (quay `try_lock`, không xếp hàng).
    /// `None` ⇔ hết trần: ngắt đã được khôi phục, **không** treo.
    ///
    /// Dùng cho khoá có thể bị bỏ quên (`scheduler`, test): deadlock thành **đỏ có chẩn đoán**.
    pub fn lock_irqsave_bounded(&self, spins: u32) -> Option<IrqMcsGuard<'_, T>> {
        let irq = IrqState::save_and_disable();
        match self.acquire_bounded(spins) {
            Some(guard) => Some(self.enter(guard, irq)),
            None => {
                irq.restore();
                None
            }
        }
    }

    /// Thử lấy khoá **không chờ**, tắt ngắt trước khi thử. `None` ⇔ đang bị giữ/đang có người
    /// xếp hàng (không reentrant).
    pub fn try_lock_irqsave(&self) -> Option<IrqMcsGuard<'_, T>> {
        let irq = IrqState::save_and_disable();
        let guard = match self.lock.try_lock() {
            Some(guard) => guard,
            None => {
                // `try_lock` thất bại vì tương tranh (hoặc vì đang bị giữ) — đếm vào `contended`.
                self.contended.fetch_add(1, Ordering::Relaxed);
                irq.restore();
                return None;
            }
        };
        if mut_no_lock() {
            // Đột biến: nhả khoá thật rồi giả vờ vẫn giữ (xem `enter`).
            drop(guard);
            return Some(self.enter(None, irq));
        }
        Some(self.enter(Some(guard), irq))
    }

    /// Số lần lấy khoá thành công.
    pub fn acquired(&self) -> u32 {
        self.acquired.load(Ordering::Relaxed)
    }

    /// Số lần phải chờ khoá.
    pub fn contended(&self) -> u32 {
        self.contended.load(Ordering::Relaxed)
    }

    /// Số người **đang** trong vùng găng (0 sau khi mọi guard nhả).
    pub fn in_cs(&self) -> u32 {
        self.in_cs.load(Ordering::Acquire)
    }

    /// Số lần phát hiện chồng lấn (phải luôn `0`).
    pub fn overlap(&self) -> u32 {
        self.overlap.load(Ordering::Relaxed)
    }

    /// `true` ⇔ khoá **đã được dùng** ít nhất một lần **và** chưa từng chồng lấn.
    pub fn ok(&self) -> bool {
        self.acquired() > 0 && self.overlap() == 0
    }

    // ------------------------------------------------------------ nội bộ

    /// Quay `try_lock` tới khi được hoặc hết `spins` vòng. Không xếp hàng MCS (bounded).
    fn acquire_bounded(&self, spins: u32) -> Option<Option<McsGuard<'_, T>>> {
        if mut_no_lock() {
            return Some(None);
        }
        let mut n = 0u32;
        loop {
            if let Some(guard) = self.lock.try_lock() {
                return Some(Some(guard));
            }
            if n == 0 {
                self.contended.fetch_add(1, Ordering::Relaxed);
            }
            if n >= spins {
                return None;
            }
            n += 1;
            core::hint::spin_loop();
        }
    }

    fn enter<'a>(&'a self, guard: Option<McsGuard<'a, T>>, irq: IrqState) -> IrqMcsGuard<'a, T> {
        if self.in_cs.fetch_add(1, Ordering::AcqRel) != 0 {
            self.overlap.fetch_add(1, Ordering::Relaxed);
        }
        self.acquired.fetch_add(1, Ordering::Relaxed);
        IrqMcsGuard {
            lock: self,
            guard,
            irq,
        }
    }
}

/// Guard của [`IrqMcsLock`] — `Drop` nhả khoá **rồi** khôi phục `IF` theo giá trị đã lưu.
pub struct IrqMcsGuard<'a, T> {
    lock: &'a IrqMcsLock<T>,
    guard: Option<McsGuard<'a, T>>,
    irq: IrqState,
}

impl<T> Deref for IrqMcsGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.guard
            .as_ref()
            .map(|guard| &**guard)
            .expect("IrqMcsGuard: không có giá trị khi đột biến mut-no-lock")
    }
}

impl<T> DerefMut for IrqMcsGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.guard
            .as_mut()
            .map(|guard| &mut **guard)
            .expect("IrqMcsGuard: không có giá trị khi đột biến mut-no-lock")
    }
}

impl<T> Drop for IrqMcsGuard<'_, T> {
    fn drop(&mut self) {
        #[cfg(feature = "mut-leak-unlock")]
        {
            if MUT_LEAK_ARMED.swap(false, Ordering::SeqCst) {
                // Đột biến: bỏ nhánh nhả khoá. `mem::forget` để `McsGuard` **không** chạy `Drop`
                // (nếu không nó vẫn nhả khoá), và **không** giảm `in_cs`/khôi phục `IF` — đúng
                // hành vi "quên unlock ở nhánh lỗi".
                if let Some(guard) = self.guard.take() {
                    core::mem::forget(guard);
                }
                return;
            }
        }
        // **Giảm `in_cs` TRƯỚC** khi nhả khoá: người lấy khoá kế tiếp tăng `in_cs` sau khi
        // acquire; nếu ta giảm sau thì có thể trừ nhầm lượt tăng của người kia.
        self.lock.in_cs.fetch_sub(1, Ordering::AcqRel);
        if let Some(guard) = self.guard.take() {
            drop(guard);
        }
        self.irq.restore();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mut_no_lock_khong_anh_huong_build_thuong() {
        // `mut-no-lock` là feature đột biến; ở build thường khoá phải thật sự loại trừ.
        let lock = IrqMcsLock::new(0u8);
        let guard = lock.lock_irqsave();
        assert!(lock.try_lock_irqsave().is_none());
        drop(guard);
        assert_eq!(lock.in_cs(), 0);
        assert_eq!(lock.overlap(), 0);
    }

    #[test]
    fn irq_state_was_enabled_doc_duoc() {
        let state = IrqState::save_and_disable();
        // Trên host `IF` không tồn tại ⇒ luôn `false`; trên ảnh đây là giá trị **đã lưu**.
        assert!(!state.was_enabled());
        state.restore();
    }
}
