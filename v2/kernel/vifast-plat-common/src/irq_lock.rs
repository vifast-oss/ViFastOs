// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Khoá MCS bọc lưu/tắt ngắt — MỘT bản dùng chung 3 arch** (KT-411, nợ `KT-409b`).
//!
//! # Vì sao tệp này tồn tại
//!
//! `KT-336a` (x86_64), `KT-336c` (riscv64) và `KT-338b` (aarch64) mỗi arch viết **một** bản
//! `irq_lock.rs` (408/395/367 LOC = **1.170 LOC**) với **cùng** logic khoá/đếm/guard; chỉ khác
//! **ba** hàm chạm CSR/`RFLAGS`/`DAIF`. Ba bản đã trôi thật (aarch64 có `mut-no-irq-restore`,
//! riscv64 có `mut-leak-unlock`, x86_64 có cả hai). Tệp này giữ **một** bản; mỗi arch chỉ còn
//! `IrqState` + `impl IrqSave` (~60 LOC).
//!
//! # Vì sao `IrqSave` là trait chứ không phải `cfg(target_arch)`
//!
//! AGENTS §4.8 **cấm** thêm `cfg(target_arch)` vào kernel. Trait để crate arch tự cài phần
//! asm/CSR của mình (D2-17: `unsafe` chỉ trong `kernel/vifast-plat-*`), còn tệp này
//! `#![forbid(unsafe_code)]` và chạy được trên host.
//!
//! # Bất biến giữ nguyên (không nới ngưỡng, không đổi ngữ nghĩa)
//!
//! - Tắt ngắt **trước** khi chờ khoá; nhả khoá **trước** khi khôi phục ngắt.
//! - Khôi phục ngắt theo **giá trị đã lưu** (gọi từ handler thì không bật lại).
//! - `lock_irqsave` chờ theo MCS (FIFO, không trần); `lock_irqsave_bounded` trả `None` sau
//!   `spins` vòng — deadlock thành **đỏ có chẩn đoán**, không treo im lặng.
//! - Bộ đếm `acquired`/`contended`/`in_cs`/`overlap` là **bằng chứng đo được** (TAP khoá/ngắt).
//!
//! # Đột biến (chỉ để chứng minh bộ kiểm có răng — D2-05)
//!
//! - `mut-leak-unlock`: [`IrqMcsGuard`] bỏ nhả khoá ở **một** nhánh `Drop` ⇒ lần lấy kế tiếp
//!   chạm trần ⇒ TAP khoá ĐỎ (không treo).
//! - `mut-no-lock`: bỏ hẳn phần `McsLock` (vẫn đếm `in_cs`) ⇒ hai ngữ cảnh cùng vào vùng găng
//!   ⇒ `overlap > 0` ⇒ TAP khoá ĐỎ.
//!
//! Hai móc này **chỉ** tồn tại khi bật feature tương ứng; `v2/test/mutants.toml` lật
//! `feature = "mut-*"` trên **bản sao** của cây (KT-403 r2), nên không khai báo ở `[features]`.

use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicU32, Ordering};

#[cfg(any(feature = "mut-leak-unlock", feature = "mut-no-lock"))]
use core::sync::atomic::AtomicBool;

use vifast_sync::{Guard as McsGuard, McsLock};

/// Phần **arch** của khoá: lưu trạng thái ngắt che được rồi tắt nó, và khôi phục đúng giá trị.
///
/// Cài đặt nằm ở crate arch (`vifast-plat-{riscv64,aarch64,x86_64}`) vì cần CSR/`RFLAGS`/`DAIF`
/// — `unsafe` thuộc danh sách đóng `kernel/vifast-plat-*` (D2-17).
pub trait IrqSave: Copy {
    /// Đọc-và-tắt ngắt; trả trạng thái **trước** khi tắt.
    fn save_and_disable() -> Self;

    /// `true` ⇔ ngắt đang bật **trước** khi tắt.
    fn was_enabled(&self) -> bool;

    /// Khôi phục ngắt **chỉ khi** trước đó đang bật.
    fn restore(self);
}

#[cfg(feature = "mut-leak-unlock")]
static MUT_LEAK_ARMED: AtomicBool = AtomicBool::new(false);

#[cfg(feature = "mut-no-lock")]
static MUT_NO_LOCK_ARMED: AtomicBool = AtomicBool::new(false);

/// Đột biến `mut-no-lock`: arm để bỏ hẳn phần khoá MCS (vẫn đếm `in_cs`) **trong phạm vi** ca
/// kiểm — không làm gãy `core_bridge` của cả boot.
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
///
/// `S` là phần arch ([`IrqSave`]) — nhờ vậy **một** bản mã phục vụ cả ba arch.
pub struct IrqMcsLock<T, S: IrqSave> {
    lock: McsLock<T>,
    /// Số lần lấy khoá **thành công** (bằng chứng khoá được dùng, không phải trang trí).
    acquired: AtomicU32,
    /// Số lần phải **chờ** (try/acquire đầu tiên thất bại).
    contended: AtomicU32,
    /// Số người đang trong vùng găng — phải luôn `0` hoặc `1`.
    in_cs: AtomicU32,
    /// Số lần phát hiện **hai** người trong vùng găng — phải luôn `0`.
    overlap: AtomicU32,
    /// `S` chỉ xuất hiện ở kiểu, không ở giá trị.
    arch: PhantomData<S>,
}

impl<T, S: IrqSave> IrqMcsLock<T, S> {
    /// Khoá mới, chưa ai giữ; dùng được trong `static`/`const`.
    pub const fn new(value: T) -> Self {
        IrqMcsLock {
            lock: McsLock::new(value),
            acquired: AtomicU32::new(0),
            contended: AtomicU32::new(0),
            in_cs: AtomicU32::new(0),
            overlap: AtomicU32::new(0),
            arch: PhantomData,
        }
    }

    /// Lấy khoá **chờ theo MCS (FIFO, không trần)**, tắt ngắt trước khi chờ. Trả guard; `Drop`
    /// nhả khoá rồi khôi phục ngắt.
    pub fn lock_irqsave(&self) -> IrqMcsGuard<'_, T, S> {
        let irq = S::save_and_disable();
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
    pub fn lock_irqsave_bounded(&self, spins: u32) -> Option<IrqMcsGuard<'_, T, S>> {
        let irq = S::save_and_disable();
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
    pub fn try_lock_irqsave(&self) -> Option<IrqMcsGuard<'_, T, S>> {
        let irq = S::save_and_disable();
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

    fn enter<'a>(&'a self, guard: Option<McsGuard<'a, T>>, irq: S) -> IrqMcsGuard<'a, T, S> {
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

/// Guard của [`IrqMcsLock`] — `Drop` nhả khoá **rồi** khôi phục ngắt theo giá trị đã lưu.
pub struct IrqMcsGuard<'a, T, S: IrqSave> {
    lock: &'a IrqMcsLock<T, S>,
    guard: Option<McsGuard<'a, T>>,
    irq: S,
}

impl<T, S: IrqSave> Deref for IrqMcsGuard<'_, T, S> {
    type Target = T;

    fn deref(&self) -> &T {
        self.guard
            .as_deref()
            .expect("IrqMcsGuard: không có giá trị khi đột biến mut-no-lock")
    }
}

impl<T, S: IrqSave> DerefMut for IrqMcsGuard<'_, T, S> {
    fn deref_mut(&mut self) -> &mut T {
        self.guard
            .as_deref_mut()
            .expect("IrqMcsGuard: không có giá trị khi đột biến mut-no-lock")
    }
}

impl<T, S: IrqSave> Drop for IrqMcsGuard<'_, T, S> {
    fn drop(&mut self) {
        #[cfg(feature = "mut-leak-unlock")]
        {
            if MUT_LEAK_ARMED.swap(false, Ordering::SeqCst) {
                // Đột biến: bỏ nhánh nhả khoá. `mem::forget` để `McsGuard` **không** chạy `Drop`
                // (nếu không nó vẫn nhả khoá), và **không** giảm `in_cs`/khôi phục ngắt — đúng
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
