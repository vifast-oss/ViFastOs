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

use vifast_plat_common::irq_lock::IrqSave;

/// Bit `SIE` (supervisor interrupt enable) của CSR `sstatus` — privileged spec, bit 1. Trùng
/// `vifast_plat_riscv64_logic::trap::SSTATUS_SIE`/`crate::timer::SSTATUS_SIE`; khai lại ở đây để tệp
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
