// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Khoá MCS bọc lưu/tắt `DAIF.I`** — KT-338b (D2-244/D2-245), cùng khuôn
//! `vifast-plat-riscv64/src/irq_lock.rs` (KT-336c) và `vifast-plat-x86_64/src/irq_lock.rs`
//! (KT-336a) nhưng dùng `PSTATE.DAIF` của AArch64.
//!
//! `vifast_sync::McsLock` **không** tự tắt ngắt. Dùng nó nguyên trạng cho tài nguyên dùng chung
//! giữa luồng thường và handler ngắt sẽ **tự deadlock**: hart đang giữ khoá bị ngắt trên chính
//! nó, handler lấy khoá ⇒ quay lại [`McsLock::lock`] của **cùng hart** (bẫy D2-238). Mọi chỗ
//! dùng khoá cho tài nguyên đụng ngắt (ở đây: `core_bridge::LOCKED` cho `Machine`) đi qua
//! [`IrqMcsLock`]:
//!
//! ```text
//!   đọc DAIF (mrs) ─► mask I (msr daifset,#2) ─► McsLock::lock ─► vùng găng ─► unlock ─► khôi phục DAIF
//! ```
//!
//! # Vì sao **hai** lệnh (đọc rồi mask) mà vẫn đúng
//!
//! RISC-V có `csrrc` "đọc **và** xoá trong một lệnh". AArch64 **không** có lệnh tương đương cho
//! `DAIF`: phải `mrs x, daif` rồi `msr daifset, #2`. Khe giữa hai lệnh **nằm trước** vùng găng
//! (khoá chưa lấy, `in_cs` chưa tăng), nên một IRQ chen vào đó chỉ chạy handler rồi trả về; giá
//! trị `DAIF` đã lưu vẫn là giá trị **trước** khi vào vùng găng, và `msr daifset` chạy tiếp sau
//! ⇒ **không** có cửa sổ "đang giữ khoá mà ngắt bật". Đây là điểm yếu đã biết, ghi rõ thay vì
//! che: nó rộng đúng **một** instruction và không thể chứa vùng găng.
//!
//! # Vì sao `DAIF` phải **khôi phục theo giá trị đã lưu**
//!
//! Gọi từ handler ngắt (`I = 1` — phần cứng mask khi vào trap) thì nhả khoá **không** được mở
//! `I` (sẽ bật ngắt giữa handler). Gọi từ luồng thường (`I = 0`, sau `msr daifclr,#2` của
//! `smpboot`) thì phải mở lại. [`IrqState`] lưu **cả** `DAIF` **trước** khi mask và giá trị
//! [`IrqState::restore`] ghi lại **đúng** giá trị ấy — không suy diễn, không hardcode.
//!
//! # Trần chờ **hữu hạn**
//!
//! [`IrqMcsLock::lock_irqsave`] chờ theo MCS (FIFO, không trần — dùng cho khoá lá như
//! `core_bridge`); [`IrqMcsLock::lock_irqsave_bounded`] trả `None` sau `spins` vòng — deadlock
//! thành **đỏ có chẩn đoán**, không treo im lặng. Bộ đếm `acquired`/`contended`/`in_cs`/
//! `overlap` là **bằng chứng đo được** (TAP `lock` của `KT-338b`).
//!
//! # Đột biến (chỉ để chứng minh bộ kiểm có răng — D2-05)
//!
//! - `mut-no-irq` (feature): [`IrqState::save_and_disable`] **không** mask `I` ⇒ ngắt chạy ngay
//!   trong vùng găng ⇒ TAP `lock: irq` ĐỎ (`inside_enabled=1`).
//! - `mut-no-irq-restore` (feature): [`IrqMcsGuard`] **bỏ** nhánh khôi phục `DAIF` ⇒ sau khi rảnh
//!   khoá `DAIF.I` vẫn `1` ⇒ TAP `lock: irq` ĐỎ (`after_enabled=0`). Đây là **đột biến (ii)** mà
//!   `KT-338b` bắt buộc (D2-244: "bỏ `sstatus.SIE` restore trong unlock ⇒ TAP khoá/ngắt đỏ").
//!
//! `unsafe` ở đây thuộc danh sách đóng `kernel/vifast-plat-*` (D2-17) và **chỉ** gồm `mrs daif`/
//! `msr daif`/`msr daifclr` của hart hiện tại — uỷ quyền cho [`crate::asm`] (module duy nhất được
//! `allow(unsafe_code)` của crate); tệp này **không** tự viết `unsafe`.

use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicU32, Ordering};

use vifast_sync::{Guard as McsGuard, McsLock};

use crate::sysreg::DAIF_I;

/// `true` ⇔ ngắt IRQ của hart đang chạy **đang mở** (`PSTATE.I = 0`).
///
/// Đây là phép đọc **duy nhất** mà ca kiểm dùng để đo "ngắt đã tắt trong vùng găng" và "đã khôi
/// phục sau khi nhả" — cùng nguồn với [`IrqState::save_and_disable`], nên hai phép đo không thể
/// lệch nhau vì hai hằng khác nhau.
pub fn irq_enabled() -> bool {
    #[cfg(all(target_arch = "aarch64", target_os = "none"))]
    {
        crate::asm::read_daif() & DAIF_I == 0
    }
    #[cfg(not(all(target_arch = "aarch64", target_os = "none")))]
    {
        // Trên host không có `PSTATE`: coi như ngắt đang tắt (không có `DAIF` để mở).
        false
    }
}

/// Trạng thái `DAIF` **trước** khi vào vùng găng.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IrqState {
    /// Giá trị `DAIF` đã lưu (nguyên vẹn 4 bit mask, không suy diễn).
    saved: u64,
    /// `true` ⇔ `PSTATE.I` đang mở **trước** khi mask.
    was_enabled: bool,
}

impl IrqState {
    /// Đọc `DAIF` rồi mask `I`; trả trạng thái đã lưu.
    ///
    /// Đột biến `mut-no-irq` (feature) **không** mask `I` — chỉ dùng cho ca đột biến, không phải
    /// feature sản phẩm.
    pub fn save_and_disable() -> Self {
        #[cfg(all(target_arch = "aarch64", target_os = "none"))]
        {
            let saved = crate::asm::read_daif();
            if !cfg!(feature = "mut-no-irq") {
                // Chỉ đổi bit `I`, giữ nguyên D/A/F đã có — `DAIF` ghi lại bằng `msr daif` của
                // `asm` (không tự viết asm ở đây, D2-17).
                crate::asm::write_daif(saved | DAIF_I);
            }
            IrqState {
                saved,
                was_enabled: saved & DAIF_I == 0,
            }
        }
        #[cfg(not(all(target_arch = "aarch64", target_os = "none")))]
        {
            IrqState {
                saved: DAIF_I,
                was_enabled: false,
            }
        }
    }

    /// `PSTATE.I` **trước** khi mask (`true` ⇔ ngắt đang mở).
    pub fn was_enabled(&self) -> bool {
        self.was_enabled
    }

    /// Giá trị `DAIF` đã lưu — bằng chứng cho log/TAP (không phải API điều khiển).
    pub fn saved(&self) -> u64 {
        self.saved
    }

    /// Khôi phục **đúng** `DAIF` đã lưu (cả bốn bit, không chỉ `I`).
    pub fn restore(self) {
        #[cfg(all(target_arch = "aarch64", target_os = "none"))]
        {
            if cfg!(feature = "mut-no-irq-restore") {
                // Đột biến (ii): **không** khôi phục ⇒ `DAIF.I` ở lại `1` sau khi nhả khoá.
                return;
            }
            // Chỉ ghi lại khi giá trị **khác** hiện tại: tránh `msr daif` vô ích trên đường nóng
            // và tránh vô tình mở `F`/`A`/`D` khi chúng đã bị mask trong lúc giữ khoá.
            let now = crate::asm::read_daif();
            if now != self.saved {
                // SAFETY: `msr daif` ghi bốn bit mask ngắt của hart hiện tại từ giá trị đã đọc
                // trước khi vào vùng găng; uỷ quyền cho `asm` (D2-17).
                crate::asm::write_daif(self.saved);
            }
        }
        #[cfg(not(all(target_arch = "aarch64", target_os = "none")))]
        {
            let _ = self.saved;
        }
    }
}

/// Guard chỉ tắt/mở ngắt, **không** lấy khoá — dùng cho vùng găng một-hart (không cần MCS).
#[derive(Clone, Debug)]
pub struct IrqGuard {
    state: IrqState,
}

impl IrqGuard {
    /// Tắt ngắt tới khi guard rơi.
    pub fn new() -> Self {
        IrqGuard {
            state: IrqState::save_and_disable(),
        }
    }

    /// `PSTATE.I` trước khi tắt.
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

/// Khoá MCS + bộ đếm bằng chứng; mọi lần lấy khoá đều **tắt ngắt trước**.
pub struct IrqMcsLock<T> {
    lock: McsLock<T>,
    /// Số lần lấy khoá **thành công** (bằng chứng khoá được dùng, không phải trang trí).
    acquired: AtomicU32,
    /// Số lần phải **chờ**.
    contended: AtomicU32,
    /// Số người đang trong vùng găng — phải luôn `0` hoặc `1`.
    in_cs: AtomicU32,
    /// Số lần phát hiện **hai** người trong vùng găng — phải luôn `0`.
    overlap: AtomicU32,
}

impl<T> IrqMcsLock<T> {
    /// Khoá mới, chưa ai giữ; dùng được trong `static`.
    pub const fn new(value: T) -> Self {
        IrqMcsLock {
            lock: McsLock::new(value),
            acquired: AtomicU32::new(0),
            contended: AtomicU32::new(0),
            in_cs: AtomicU32::new(0),
            overlap: AtomicU32::new(0),
        }
    }

    /// Lấy khoá **chờ theo MCS (FIFO, không trần)**, tắt ngắt trước khi chờ.
    pub fn lock_irqsave(&self) -> IrqMcsGuard<'_, T> {
        let irq = IrqState::save_and_disable();
        if let Some(guard) = self.lock.try_lock() {
            return self.enter(Some(guard), irq);
        }
        self.contended.fetch_add(1, Ordering::Relaxed);
        let guard = self.lock.lock();
        self.enter(Some(guard), irq)
    }

    /// Lấy khoá với **trần chờ hữu hạn** `spins` vòng. `None` ⇔ hết trần: ngắt đã được khôi
    /// phục, **không** treo.
    pub fn lock_irqsave_bounded(&self, spins: u32) -> Option<IrqMcsGuard<'_, T>> {
        let irq = IrqState::save_and_disable();
        let mut n = 0u32;
        loop {
            if let Some(guard) = self.lock.try_lock() {
                return Some(self.enter(Some(guard), irq));
            }
            if n == 0 {
                self.contended.fetch_add(1, Ordering::Relaxed);
            }
            if n >= spins {
                irq.restore();
                return None;
            }
            n += 1;
            core::hint::spin_loop();
        }
    }

    /// Thử lấy khoá **không chờ**, tắt ngắt trước khi thử.
    pub fn try_lock_irqsave(&self) -> Option<IrqMcsGuard<'_, T>> {
        let irq = IrqState::save_and_disable();
        let Some(guard) = self.lock.try_lock() else {
            self.contended.fetch_add(1, Ordering::Relaxed);
            irq.restore();
            return None;
        };
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

/// Guard của [`IrqMcsLock`] — `Drop` nhả khoá **rồi** khôi phục `DAIF` theo giá trị đã lưu.
pub struct IrqMcsGuard<'a, T> {
    lock: &'a IrqMcsLock<T>,
    guard: Option<McsGuard<'a, T>>,
    irq: IrqState,
}

impl<T> Deref for IrqMcsGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.guard
            .as_deref()
            .expect("IrqMcsGuard: guard rỗng (chỉ khi đột biến)")
    }
}

impl<T> DerefMut for IrqMcsGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.guard
            .as_deref_mut()
            .expect("IrqMcsGuard: guard rỗng (chỉ khi đột biến)")
    }
}

impl<T> Drop for IrqMcsGuard<'_, T> {
    fn drop(&mut self) {
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

    /// Dựng khoá, lấy/nhả: bộ đếm chứng minh khoá **thật sự** được dùng và không chồng lấn.
    /// Trên host `DAIF` không tồn tại nên `save_and_disable` trả `was_enabled = false` —
    /// hành vi đó được kiểm **tường minh** (không phải bỏ qua).
    #[test]
    fn irq_mcs_lock_dem_duoc() {
        let lock = IrqMcsLock::new(0u32);
        {
            let mut guard = lock.lock_irqsave();
            assert_eq!(*guard, 0);
            *guard = 7;
            assert_eq!(*guard, 7);
        }
        assert_eq!(lock.acquired(), 1);
        assert_eq!(lock.in_cs(), 0);
        assert_eq!(lock.overlap(), 0);
        assert!(lock.ok());
        assert!(!irq_enabled(), "host không có PSTATE: ngắt luôn 'tắt'");
    }

    /// `try_lock_irqsave` **không** reentrant: lần thứ hai (cùng hart) trả `None` và **không**
    /// để lại `in_cs`/`overlap` sai.
    #[test]
    fn try_lock_khong_reentrant() {
        let lock = IrqMcsLock::new(());
        let first = lock.try_lock_irqsave().expect("lần đầu phải được");
        assert!(lock.try_lock_irqsave().is_none(), "reentrant phải bị chặn");
        drop(first);
        assert!(lock.ok());
        assert_eq!(lock.in_cs(), 0);
        assert_eq!(lock.overlap(), 0);
    }

    /// `lock_irqsave_bounded(0)` khi khoá đang bị giữ: trả `None` **hữu hạn** (không treo) và
    /// khôi phục ngắt (host: no-op).
    #[test]
    fn bounded_het_tran_khong_treo() {
        let lock = IrqMcsLock::new(());
        let held = lock.lock_irqsave();
        assert!(lock.lock_irqsave_bounded(8).is_none());
        drop(held);
        assert!(lock.lock_irqsave_bounded(8).is_some());
    }
}
