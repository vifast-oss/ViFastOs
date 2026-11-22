// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Khoá MCS bọc lưu/tắt ngắt `sstatus.SIE`** — KT-336c (D2-243), cùng khuôn `KT-336a` cho
//! `x86_64` (`v2/kernel/vifast-plat-x86_64/src/irq_lock.rs`) nhưng dùng CSR của riscv64.
//!
//! `vifast_sync::McsLock` **không** tự tắt ngắt. Dùng nó nguyên trạng cho tài nguyên dùng chung
//! giữa luồng thường và handler ngắt sẽ **tự deadlock**: hart đang giữ khoá bị ngắt trên chính
//! nó, handler lấy khoá ⇒ quay lại [`McsLock::lock`] của **cùng hart** (bẫy D2-238, KT-316 đã
//! gặp với khoá COM1). Mọi chỗ dùng khoá cho tài nguyên đụng ngắt đi qua [`IrqMcsLock`]:
//!
//! ```text
//!   đọc+ tắt SIE (csrrc) ──► McsLock::lock/try_lock ──► vùng găng ──► McsLock unlock ──► khôi phục SIE
//! ```
//!
//! # Vì sao một lệnh `csrrc` (không phải `csrr` rồi `csrc`)
//!
//! `csrrc rd, sstatus, rs1` **đọc giá trị cũ vào `rd` và xoá bit trong cùng một lệnh** ⇒ trạng
//! thái đã lưu và cửa sổ tắt ngắt không thể lệch nhau (không có khe giữa hai lệnh để trap chạy
//! với `SIE` vẫn bật trong khi ta tưởng đã tắt). Đây là điểm mạnh hơn khuôn `pushfq`/`cli` của
//! x86_64, nơi hai lệnh vẫn nằm trên cùng một hart với `IF` chưa tắt — bên đó chấp nhận được vì
//! `pushfq` không có tác dụng phụ, còn ở đây một lệnh là đúng ngữ nghĩa "lưu **và** tắt".
//!
//! # Thứ tự **bắt buộc**
//!
//! Tắt ngắt **trước** khi chờ khoá; nhả khoá **trước** khi bật lại ngắt. Nhờ vậy không có cửa sổ
//! "đang giữ khoá mà ngắt bật".
//!
//! # Vì sao `SIE` phải **khôi phục theo giá trị đã lưu**
//!
//! Gọi từ handler ngắt (`SIE = 0` — phần cứng xoá khi vào trap, `SPIE` giữ giá trị cũ) thì nhả
//! khoá **không** được bật `SIE` (sẽ bật ngắt giữa handler). Gọi từ luồng thường (`SIE = 1`) thì
//! phải bật lại. [`IrqState`] lưu `sstatus.SIE` **trước** khi tắt và [`IrqGuard`]/
//! [`IrqMcsGuard`] khôi phục đúng giá trị ấy.
//!
//! # Trần chờ **hữu hạn**
//!
//! [`IrqMcsLock::lock_irqsave`] chờ theo MCS (FIFO, không trần — dùng cho khoá lá như
//! `core_bridge`); [`IrqMcsLock::lock_irqsave_bounded`] trả `None` sau `spins` vòng — deadlock
//! thành **đỏ có chẩn đoán**, không treo im lặng. Bộ đếm `acquired`/`contended`/`in_cs`/`overlap`
//! là **bằng chứng đo được** (TAP 70 ảnh `ktest` riscv64).
//!
//! # Đột biến (chỉ để chứng minh bộ kiểm có răng — D2-05)
//!
//! - `mut-no-irq` (feature): [`IrqState::save_and_disable`] **không** `csrc sstatus,SIE` ⇒ ngắt
//!   mềm chạy ngay trong vùng găng ⇒ TAP 70 ĐỎ.
//! - `mut-leak-unlock` (feature): [`IrqMcsGuard`] bỏ nhả khoá ở **một** nhánh `Drop` (có arm) ⇒
//!   lần lấy kế tiếp chạm trần ⇒ TAP 70 ĐỎ (không treo).
//! - `mut-no-lock` (feature): bỏ hẳn phần `McsLock` (vẫn đếm `in_cs`) ⇒ hai ngữ cảnh cùng vào
//!   vùng găng ⇒ `overlap > 0` ⇒ TAP 70 ĐỎ.
//!
//! `unsafe` ở đây thuộc danh sách đóng `kernel/vifast-plat-*` (D2-17) và **chỉ** gồm đọc/ghi CSR
//! `sstatus` của hart hiện tại; không MMIO, không con trỏ thô.

use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicU32, Ordering};

#[cfg(any(feature = "mut-leak-unlock", feature = "mut-no-lock"))]
use core::sync::atomic::AtomicBool;

use vifast_sync::{Guard as McsGuard, McsLock};

/// Bit `SIE` (supervisor interrupt enable) của CSR `sstatus` — privileged spec, bit 1. Trùng
/// `vifast_plat_common::trap::SSTATUS_SIE`/`crate::timer::SSTATUS_SIE`; khai lại ở đây để tệp
/// này **không** phụ thuộc module khác (một nguồn cho phần khoá).
pub const SSTATUS_SIE: u64 = 1 << 1;

/// `true` ⇔ ngắt S-mode của hart đang chạy đang bật (`sstatus.SIE = 1`).
///
/// Đây là phép đọc **duy nhất** mà ca kiểm dùng để đo `if_inside` — cùng nguồn với
/// [`IrqState::save_and_disable`], nên "đã lưu" và "đo được" không thể lệch nhau vì hai hằng khác
/// nhau.
pub fn irq_enabled() -> bool {
    #[cfg(all(target_arch = "riscv64", target_os = "none"))]
    {
        let value: u64;
        // SAFETY: `csrr sstatus` chỉ đọc CSR của hart hiện tại, không tác dụng phụ.
        unsafe {
            core::arch::asm!("csrr {}, sstatus", out(reg) value, options(nomem, nostack));
        }
        value & SSTATUS_SIE != 0
    }
    #[cfg(not(all(target_arch = "riscv64", target_os = "none")))]
    {
        // Trên host không có CSR: coi như ngắt đang tắt (không có `SIE` để bật).
        false
    }
}

/// Trạng thái ngắt che được (`sstatus.SIE`) **trước** khi vào vùng găng.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IrqState {
    was_enabled: bool,
}

impl IrqState {
    /// Đọc-và-xoá `sstatus.SIE` bằng **một** lệnh `csrrc`; trả trạng thái đã lưu.
    ///
    /// Đột biến `mut-no-irq` (feature) **không** tắt ngắt — chỉ dùng cho ca đột biến, không phải
    /// feature sản phẩm.
    pub fn save_and_disable() -> Self {
        #[cfg(all(target_arch = "riscv64", target_os = "none"))]
        {
            if cfg!(feature = "mut-no-irq") {
                // Đột biến (i): giữ nguyên `SIE`; `was_enabled = false` để `restore` cũng không
                // đổi trạng thái (đúng hành vi "quên tắt ngắt", không phải "tắt rồi quên bật").
                return IrqState { was_enabled: false };
            }
            let old: u64;
            // SAFETY: `csrrc` với `sstatus` chỉ đọc giá trị cũ rồi xoá bit `SIE` của hart hiện
            // tại — privileged spec cho S-mode ghi `SIE`. Không chạm bộ nhớ.
            unsafe {
                core::arch::asm!(
                    "csrrc {}, sstatus, {}",
                    out(reg) old,
                    in(reg) SSTATUS_SIE,
                    options(nomem, nostack),
                );
            }
            IrqState {
                was_enabled: old & SSTATUS_SIE != 0,
            }
        }
        #[cfg(not(all(target_arch = "riscv64", target_os = "none")))]
        {
            IrqState { was_enabled: false }
        }
    }

    /// `sstatus.SIE` **trước** khi tắt (`true` ⇔ ngắt đang bật).
    pub fn was_enabled(&self) -> bool {
        self.was_enabled
    }

    /// Bật lại ngắt **chỉ khi** trước đó đang bật — khôi phục đúng trạng thái đã lưu.
    pub fn restore(self) {
        #[cfg(all(target_arch = "riscv64", target_os = "none"))]
        {
            if self.was_enabled {
                // SAFETY: `csrs sstatus, SIE` chỉ bật bit `SIE` của hart hiện tại.
                unsafe {
                    core::arch::asm!(
                        "csrs sstatus, {}",
                        in(reg) SSTATUS_SIE,
                        options(nomem, nostack),
                    );
                }
            }
        }
        #[cfg(not(all(target_arch = "riscv64", target_os = "none")))]
        {
            let _ = self.was_enabled;
        }
    }
}

/// Guard **chỉ** giữ trạng thái ngắt (không giữ khoá) — dùng khi cần tắt ngắt quanh một đoạn
/// không lấy khoá. `Drop` khôi phục `SIE` theo giá trị đã lưu.
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

    /// `sstatus.SIE` trước khi tắt.
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
    /// nhả khoá rồi khôi phục `SIE`.
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

/// Guard của [`IrqMcsLock`] — `Drop` nhả khoá **rồi** khôi phục `SIE` theo giá trị đã lưu.
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
                // (nếu không nó vẫn nhả khoá), và **không** giảm `in_cs`/khôi phục `SIE` — đúng
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
