// SPDX-License-Identifier: Apache-2.0 OR MIT
//! **KT-418** — **ngữ cảnh FPU/SIMD eager theo TCB** trên aarch64: `v0..v31` (128 bit) +
//! `fpsr`/`fpcr`, vùng lưu 528 B **ngoài** `TcbCore`.
//!
//! # Vì sao cần
//!
//! `CPACR_EL1.FPEN` đã được bật (`sysreg::CPACR_EL1_FP_SIMD`, KT-269) nên EL0 **chạy được** lệnh
//! NEON — nhưng **không** có vùng lưu theo TCB: hai tác vụ dùng `v0..v31` sẽ ghi đè nhau, và
//! `fpsr`/`fpcr` (chế độ làm tròn, cờ ngoại lệ) cũng không được giữ. Đây đúng lỗ hổng KT-418.
//!
//! # Bố cục vùng lưu (528 B, căn 16 — cùng kiểu `vifast_plat_common::fpu::FpuArea`)
//!
//! | offset | nội dung |
//! |---|---|
//! | `0..512` | `v0..v31`, mỗi thanh ghi **16 B** (`stp qN, qN+1` — 16 cặp) |
//! | `512` | `fpsr` |
//! | `520` | `fpcr` |
//!
//! Cùng offset "thanh ghi thứ `i`" = `i * 16` với riscv64/x86_64 ⇒ một bố cục, ba arch.
//!
//! # Bất biến an toàn của `AREAS`
//!
//! Một TCB chạy trên **một** hart tại một thời điểm ⇒ hai hart không chạm cùng một khe. Tràn khe
//! ⇒ **fail-closed** (không ghi đè vùng của TCB khác, đếm `overflow`).

// **KT-418c** — `use` của inline asm chỉ tồn tại trên **ảnh** aarch64 (`target_os = "none"`):
// host (macOS/Linux, `x86_64` **hoặc** `aarch64`) không được thấy module này gọi `asm!` bằng
// lệnh của aarch64, nếu không `cargo build -p vifast-plat-aarch64 --target x86_64-…` chết ở
// assembler (đo được: 40 lỗi "invalid operand for instruction", xem JOURNAL `kt418c`).
#[cfg(all(target_arch = "aarch64", target_os = "none"))]
use core::arch::asm;
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicU32, Ordering};

use vifast_plat_common::fpu::{FpuArea, FpuTracker, FPU_SLOTS};

/// Bảng vùng lưu theo khe — `UnsafeCell` vì asm ghi trực tiếp vào bộ nhớ.
struct Areas(UnsafeCell<[FpuArea; FPU_SLOTS]>);

// SAFETY: xem "Bất biến an toàn của `AREAS`" ở đầu tệp.
unsafe impl Sync for Areas {}

/// Vùng lưu FPU/SIMD của mọi TCB — **ngoài** `TcbCore` (528 B × 65 = 34.320 B `.bss`).
static AREAS: Areas = Areas(UnsafeCell::new([FpuArea::ZERO; FPU_SLOTS]));

/// Sổ khe theo TCB (bookkeeping thuần, test host ở `vifast-plat-common/tests/fpu.rs`).
static TRACKER: FpuTracker<FPU_SLOTS> = FpuTracker::new();

/// TCB đang chạy trên hart này — `u32::MAX` = chưa gắn (đường kernel thuần).
///
/// Vì sao **không** đọc `Machine::current(0)` trong hook: `save_current` có thể chạy trong handler
/// trap, nơi khoá `Machine` có thể đang bị giữ (khoá không tái nhập) ⇒ deadlock. Ảnh gọi
/// [`bind_current`] **một lần** trước khi xuống EL0, đúng khuôn `user::bind_core_tcb` của riscv64.
static CURRENT_TCB: AtomicU32 = AtomicU32::new(u32::MAX);

/// Gắn TCB đang chạy (gọi trước `eret_to_el0`).
pub fn bind_current(tcb: u32) {
    CURRENT_TCB.store(tcb, Ordering::Release);
}

/// TCB đang chạy (`None` = chưa gắn).
pub fn current_tcb() -> Option<u32> {
    let value = CURRENT_TCB.load(Ordering::Acquire);
    if value == u32::MAX {
        None
    } else {
        Some(value)
    }
}

/// Stub host: **panic** thay vì no-op — gọi asm đặc quyền của aarch64 trên host là lỗi lập
/// trình, không phải đường sản phẩm (`KT-167`/`KT-418c`).
#[cfg(not(all(target_arch = "aarch64", target_os = "none")))]
fn host_stub(name: &str) -> ! {
    panic!("vifast-plat-aarch64: {name} chỉ chạy trên aarch64 (host stub, KT-167)")
}

/// Con trỏ tới vùng lưu của `slot`.
fn area_ptr(slot: usize) -> *mut FpuArea {
    AREAS.0.get().cast::<FpuArea>().wrapping_add(slot)
}

/// Lưu `v0..v31` + `fpsr`/`fpcr` vào `area`.
///
/// # Safety
///
/// `area` phải trỏ tới một [`FpuArea`] hợp lệ, ghi được, **không** alias.
pub unsafe fn save_raw(area: *mut FpuArea) {
    // SAFETY: caller bảo đảm `area` hợp lệ; `stp qN, qN+1` ghi 32 B mỗi cặp vào đúng offset
    // (0..512), `mrs` đọc thanh ghi hệ thống FP (không đụng bộ nhớ).
    #[cfg(all(target_arch = "aarch64", target_os = "none"))]
    unsafe {
        let base = area.cast::<u8>();
        asm!(
            "stp q0, q1, [{b}, #0]",
            "stp q2, q3, [{b}, #32]",
            "stp q4, q5, [{b}, #64]",
            "stp q6, q7, [{b}, #96]",
            "stp q8, q9, [{b}, #128]",
            "stp q10, q11, [{b}, #160]",
            "stp q12, q13, [{b}, #192]",
            "stp q14, q15, [{b}, #224]",
            "stp q16, q17, [{b}, #256]",
            "stp q18, q19, [{b}, #288]",
            "stp q20, q21, [{b}, #320]",
            "stp q22, q23, [{b}, #352]",
            "stp q24, q25, [{b}, #384]",
            "stp q26, q27, [{b}, #416]",
            "stp q28, q29, [{b}, #448]",
            "stp q30, q31, [{b}, #480]",
            "mrs {t0}, fpsr",
            "mrs {t1}, fpcr",
            "str {t0}, [{b}, #512]",
            "str {t1}, [{b}, #520]",
            b = in(reg) base,
            t0 = out(reg) _,
            t1 = out(reg) _,
            options(nostack),
        );
    }
    #[cfg(not(all(target_arch = "aarch64", target_os = "none")))]
    {
        let _ = area;
        host_stub("save_raw")
    }
}

/// Khôi phục `v0..v31` + `fpsr`/`fpcr` từ `area`.
///
/// # Safety
///
/// `area` phải trỏ tới một [`FpuArea`] hợp lệ, đọc được, đã ghi bởi [`save_raw`].
pub unsafe fn restore_raw(area: *const FpuArea) {
    // SAFETY: caller bảo đảm `area` hợp lệ; `ldp qN, qN+1` đọc 32 B mỗi cặp từ đúng offset,
    // `msr` ghi thanh ghi hệ thống FP.
    #[cfg(all(target_arch = "aarch64", target_os = "none"))]
    unsafe {
        let base = area.cast::<u8>();
        asm!(
            "ldp q0, q1, [{b}, #0]",
            "ldp q2, q3, [{b}, #32]",
            "ldp q4, q5, [{b}, #64]",
            "ldp q6, q7, [{b}, #96]",
            "ldp q8, q9, [{b}, #128]",
            "ldp q10, q11, [{b}, #160]",
            "ldp q12, q13, [{b}, #192]",
            "ldp q14, q15, [{b}, #224]",
            "ldp q16, q17, [{b}, #256]",
            "ldp q18, q19, [{b}, #288]",
            "ldp q20, q21, [{b}, #320]",
            "ldp q22, q23, [{b}, #352]",
            "ldp q24, q25, [{b}, #384]",
            "ldp q26, q27, [{b}, #416]",
            "ldp q28, q29, [{b}, #448]",
            "ldp q30, q31, [{b}, #480]",
            "ldr {t0}, [{b}, #512]",
            "ldr {t1}, [{b}, #520]",
            "msr fpsr, {t0}",
            "msr fpcr, {t1}",
            b = in(reg) base,
            t0 = out(reg) _,
            t1 = out(reg) _,
            options(nostack),
        );
    }
    #[cfg(not(all(target_arch = "aarch64", target_os = "none")))]
    {
        let _ = area;
        host_stub("restore_raw")
    }
}

/// **Vào EL0**: khôi phục ngữ cảnh FPU/SIMD của TCB đang chạy (hoặc trạng thái **mới**).
///
/// Gọi **một cửa** ngay trước `eret_to_el0` — mọi đường xuống EL0 đi qua đó.
pub fn restore_current() {
    let Some(tcb) = current_tcb() else {
        return;
    };
    if TRACKER.is_valid(tcb) {
        if let Some(slot) = TRACKER.slot_of(tcb) {
            // SAFETY: `slot` do tracker cấp cho đúng `tcb`; vùng đã được `save_raw` ghi.
            unsafe { restore_raw(area_ptr(slot)) };
            TRACKER.record_restore();
            return;
        }
    }
    TRACKER.record_fresh();
}

/// **Ra khỏi EL0**: lưu ngữ cảnh FPU/SIMD của TCB đang chạy.
///
/// aarch64 **không** có cờ "bẩn" phần cứng tương đương `sstatus.FS`/`CR0.TS` (FPEN chỉ bật/tắt
/// trap), nên đường này **luôn** lưu khi tác vụ dừng — eager thuần, đúng chữ "eager" của
/// KT-418; `skipped` chỉ tăng khi không có TCB hiện tại (đường kernel thuần).
pub fn save_current() {
    let Some(tcb) = current_tcb() else {
        TRACKER.record_skip();
        return;
    };
    let Some(slot) = TRACKER.slot_for(tcb) else {
        // Tràn khe ⇒ fail-closed: không ghi đè vùng của TCB khác.
        return;
    };
    // SAFETY: `slot` vừa được cấp cho `tcb`; không hart nào khác chạy cùng TCB này.
    unsafe { save_raw(area_ptr(slot)) };
    TRACKER.mark_valid(tcb);
    TRACKER.record_save();
}

/// Ảnh chụp bộ đếm FPU — **bằng chứng** cho TAP/journal.
pub fn stats() -> vifast_plat_common::fpu::FpuStats {
    TRACKER.stats()
}

/// Khe của `tcb` (bằng chứng: hai TCB phải hai khe khác nhau).
pub fn slot_of(tcb: u32) -> Option<usize> {
    TRACKER.slot_of(tcb)
}

/// `true` ⇔ `tcb` đã có vùng lưu hợp lệ.
pub fn is_valid(tcb: u32) -> bool {
    TRACKER.is_valid(tcb)
}
