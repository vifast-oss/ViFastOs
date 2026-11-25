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

use vifast_plat_common::irq_lock::IrqSave;

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
