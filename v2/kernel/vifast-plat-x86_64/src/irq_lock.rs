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

use vifast_plat_common::irq_lock::IrqSave;

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

// ---------------------------------------------------------------------------------------------
// **KT-411** — phần khoá/guard/bộ đếm nay là **một** bản dùng chung ở
// `vifast_plat_common::irq_lock` (nợ `KT-409b`: trước đây ba bản 408/395/367 LOC, đã trôi khác
// nhau giữa các arch). Tệp này chỉ còn phần **arch**: `IrqState` ở trên + `impl IrqSave` dưới.
// ---------------------------------------------------------------------------------------------

impl IrqSave for IrqState {
    fn save_and_disable() -> Self {
        // Gọi **phương thức nội tại** (inherent) ở trên — không đệ quy: inherent thắng trait.
        IrqState::save_and_disable()
    }

    fn was_enabled(&self) -> bool {
        IrqState::was_enabled(self)
    }

    fn restore(self) {
        IrqState::restore(self)
    }
}

/// Khoá MCS bọc lưu/tắt ngắt của **arch này** — cùng một bản mã với hai arch còn lại.
pub type IrqMcsLock<T> = vifast_plat_common::irq_lock::IrqMcsLock<T, IrqState>;

/// Guard của [`IrqMcsLock`] — `Drop` nhả khoá rồi khôi phục ngắt theo giá trị đã lưu.
pub type IrqMcsGuard<'a, T> = vifast_plat_common::irq_lock::IrqMcsGuard<'a, T, IrqState>;

// Móc đột biến (chỉ tồn tại khi bật feature tương ứng) — re-export để caller trong ảnh kiểm giữ
// nguyên đường dẫn `vifast_plat_<arch>::irq_lock::mut_*`.
#[cfg(feature = "mut-leak-unlock")]
pub use vifast_plat_common::irq_lock::mut_arm_leak;
#[cfg(feature = "mut-no-lock")]
pub use vifast_plat_common::irq_lock::{mut_arm_no_lock, mut_disarm_no_lock};
