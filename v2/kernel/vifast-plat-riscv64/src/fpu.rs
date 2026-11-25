// SPDX-License-Identifier: Apache-2.0 OR MIT
//! **KT-418** — **ngữ cảnh FPU/SIMD eager theo TCB** trên riscv64: `fs0..fs31` + `fcsr`, bật
//! `sstatus.FS` cho U-mode, lưu khi phần cứng báo **bẩn**, khôi phục khi khe hợp lệ.
//!
//! # Vì sao cần (đo được trước KT-418)
//!
//! `sstatus.FS` mặc định **0 (Off)** và `user::enter_loaded_with_root` **không** đặt nó ⇒ mọi
//! lệnh dấu phẩy động ở U-mode trap `illegal instruction` (`scause = 2`). Ngay cả khi bật, không
//! có vùng lưu theo TCB thì hai tác vụ dùng FP sẽ **ghi đè** thanh ghi của nhau qua mỗi lượt
//! `Yield` (trapframe 288 B **không** chứa `f0..f31`).
//!
//! # Cơ chế (eager + theo dõi bẩn bằng phần cứng)
//!
//! - **Vào U-mode** ([`restore_current`]): khe hợp lệ ⇒ `fld` 32 thanh ghi + `fscsr`, rồi
//!   `FS = Clean`; khe **chưa** hợp lệ ⇒ `FS = Initial` (trạng thái **mới**, không phải rác của
//!   TCB khác).
//! - **Ra khỏi U-mode** ([`save_current`]): `FS = Dirty` ⇒ `fsd` 32 thanh ghi + `frcsr` vào khe
//!   của TCB, đánh dấu hợp lệ, `FS = Clean`; `FS ≠ Dirty` ⇒ **bỏ lưu** (tác vụ không dùng FP từ
//!   lần lưu trước) — đây là phần "theo dõi bẩn" **miễn phí** nhờ phần cứng.
//!
//! `fld`/`fsd` của chính hàm lưu/khôi phục **cũng** làm `FS = Dirty`, nên mọi đường đều đặt
//! `FS = Clean` **sau** asm — không có khe "bẩn giả" ở lượt sau.
//!
//! # Vùng lưu nằm **ngoài** `TcbCore`
//!
//! `TcbCore` là thân ABI ≤ 496 B (`spec/abi.toml:159` `tcb = 512`); 272 B vùng FPU nằm trong
//! `.bss` của crate này ([`AREAS`]), khoá theo `ObjId` của TCB qua
//! [`vifast_plat_common::fpu::FpuTracker`] — **0** trường mới trong `TcbCore`, **0** sửa `spec/`.
//!
//! # Bất biến an toàn của `AREAS`
//!
//! Một TCB chạy trên **một** hart tại một thời điểm (bất biến của scheduler core: `Running` gắn
//! với một hart), nên hai hart **không** chạm cùng một khe. Tràn khe ⇒ **fail-closed**: không ghi
//! đè vùng của TCB khác (đếm `overflow`, TAP đỏ nếu xảy ra).

use core::arch::asm;
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicU32, Ordering};

use vifast_plat_common::fpu::{FpuArea, FpuTracker, FPU_SLOTS};

/// Vị trí bit `sstatus.FS` (privileged spec: `FS[14:13]`).
pub const SSTATUS_FS_SHIFT: u64 = 13;
/// Mặt nạ `sstatus.FS`.
pub const SSTATUS_FS_MASK: u64 = 0b11 << SSTATUS_FS_SHIFT;
/// `FS = 0` — FP **tắt**: lệnh FP trap.
pub const FS_OFF: u64 = 0;
/// `FS = 1` — trạng thái **mới** (không cần khôi phục).
pub const FS_INITIAL: u64 = 1;
/// `FS = 2` — thanh ghi **khớp** vùng lưu (không cần lưu).
pub const FS_CLEAN: u64 = 2;
/// `FS = 3` — thanh ghi **bẩn** (phải lưu trước khi đổi tác vụ).
pub const FS_DIRTY: u64 = 3;

/// Bảng vùng lưu theo khe — `UnsafeCell` vì asm ghi trực tiếp vào bộ nhớ.
struct Areas(UnsafeCell<[FpuArea; FPU_SLOTS]>);

// SAFETY: xem "Bất biến an toàn của `AREAS`" ở đầu tệp — một TCB chỉ chạy trên một hart tại một
// thời điểm, nên hai hart không ghi cùng một khe; `FpuArea` là dữ liệu thuần (không có tham
// chiếu bên trong).
unsafe impl Sync for Areas {}

/// Vùng lưu FPU của mọi TCB — **ngoài** `TcbCore` (272 B × 65 = 17.680 B `.bss`).
static AREAS: Areas = Areas(UnsafeCell::new([FpuArea::ZERO; FPU_SLOTS]));

/// Sổ khe theo TCB (bookkeeping thuần, test host ở `vifast-plat-common/tests/fpu.rs`).
static TRACKER: FpuTracker<FPU_SLOTS> = FpuTracker::new();

/// TCB đang chạy — `u32::MAX` = chưa gắn (đường kernel thuần / ảnh không có U-mode).
///
/// Vì sao **không** đọc `user::core_tcb()`: module này phải biên dịch được ở ảnh **không** bật
/// feature `user` (ảnh `ktest-guard` chỉ có `ktest`) — `user` là feature của crate, không phải
/// hằng số của arch. `user::bind_core_tcb` gọi [`bind_current`] nên đường U-mode vẫn đúng một
/// nguồn.
static CURRENT_TCB: AtomicU32 = AtomicU32::new(u32::MAX);

/// Gắn TCB đang chạy (gọi từ `user::bind_core_tcb`).
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

/// Con trỏ tới vùng lưu của `slot` (`slot < FPU_SLOTS` do [`FpuTracker`] bảo đảm).
fn area_ptr(slot: usize) -> *mut FpuArea {
    AREAS.0.get().cast::<FpuArea>().wrapping_add(slot)
}

/// Đọc `sstatus.FS` (0..3).
pub fn read_fs() -> u64 {
    let value: u64;
    // SAFETY: chỉ đọc CSR `sstatus` của hart hiện tại.
    unsafe { asm!("csrr {}, sstatus", out(reg) value, options(nomem, nostack)) };
    (value & SSTATUS_FS_MASK) >> SSTATUS_FS_SHIFT
}

/// Đặt `sstatus.FS` = `fs` (0..3) — xoá hai bit rồi đặt lại, không đụng bit khác.
pub fn set_fs(fs: u64) {
    let bits = (fs & 0b11) << SSTATUS_FS_SHIFT;
    // SAFETY: chỉ sửa hai bit `FS` của CSR `sstatus` của hart hiện tại.
    unsafe {
        asm!(
            "csrc sstatus, {mask}",
            "csrs sstatus, {bits}",
            mask = in(reg) SSTATUS_FS_MASK,
            bits = in(reg) bits,
            options(nomem, nostack),
        );
    }
}

/// Lưu `f0..f31` + `fcsr` vào `area`.
///
/// # Safety
///
/// `area` phải trỏ tới một [`FpuArea`] hợp lệ, ghi được, **không** alias với vùng khác đang dùng.
unsafe fn save_raw(area: *mut FpuArea) {
    let base = area.cast::<u8>();
    let mut fcsr: u64;
    // SAFETY: caller bảo đảm `area` hợp lệ; `fsd` ghi 8 B mỗi thanh ghi vào đúng offset của
    // `FpuArea.words[0..32]`, `frcsr` đọc thanh ghi điều khiển FP (không đụng bộ nhớ).
    unsafe {
        asm!(
            "fsd f0, 0({b})",
            "fsd f1, 16({b})",
            "fsd f2, 32({b})",
            "fsd f3, 48({b})",
            "fsd f4, 64({b})",
            "fsd f5, 80({b})",
            "fsd f6, 96({b})",
            "fsd f7, 112({b})",
            "fsd f8, 128({b})",
            "fsd f9, 144({b})",
            "fsd f10, 160({b})",
            "fsd f11, 176({b})",
            "fsd f12, 192({b})",
            "fsd f13, 208({b})",
            "fsd f14, 224({b})",
            "fsd f15, 240({b})",
            "fsd f16, 256({b})",
            "fsd f17, 272({b})",
            "fsd f18, 288({b})",
            "fsd f19, 304({b})",
            "fsd f20, 320({b})",
            "fsd f21, 336({b})",
            "fsd f22, 352({b})",
            "fsd f23, 368({b})",
            "fsd f24, 384({b})",
            "fsd f25, 400({b})",
            "fsd f26, 416({b})",
            "fsd f27, 432({b})",
            "fsd f28, 448({b})",
            "fsd f29, 464({b})",
            "fsd f30, 480({b})",
            "fsd f31, 496({b})",
            "frcsr {t}",
            "sd {t}, 512({b})",
            b = in(reg) base,
            t = out(reg) fcsr,
            options(nostack),
        );
    }
    let _ = fcsr;
}

/// Khôi phục `f0..f31` + `fcsr` từ `area`.
///
/// # Safety
///
/// `area` phải trỏ tới một [`FpuArea`] hợp lệ, đọc được, đã được ghi bởi [`save_raw`].
unsafe fn restore_raw(area: *const FpuArea) {
    let base = area.cast::<u8>();
    let fcsr = unsafe { *area.cast::<u8>().add(512).cast::<u64>() };
    // SAFETY: caller bảo đảm `area` hợp lệ; `fld` đọc 8 B mỗi thanh ghi từ đúng offset,
    // `fscsr` ghi thanh ghi điều khiển FP.
    unsafe {
        asm!(
            "fld f0, 0({b})",
            "fld f1, 16({b})",
            "fld f2, 32({b})",
            "fld f3, 48({b})",
            "fld f4, 64({b})",
            "fld f5, 80({b})",
            "fld f6, 96({b})",
            "fld f7, 112({b})",
            "fld f8, 128({b})",
            "fld f9, 144({b})",
            "fld f10, 160({b})",
            "fld f11, 176({b})",
            "fld f12, 192({b})",
            "fld f13, 208({b})",
            "fld f14, 224({b})",
            "fld f15, 240({b})",
            "fld f16, 256({b})",
            "fld f17, 272({b})",
            "fld f18, 288({b})",
            "fld f19, 304({b})",
            "fld f20, 320({b})",
            "fld f21, 336({b})",
            "fld f22, 352({b})",
            "fld f23, 368({b})",
            "fld f24, 384({b})",
            "fld f25, 400({b})",
            "fld f26, 416({b})",
            "fld f27, 432({b})",
            "fld f28, 448({b})",
            "fld f29, 464({b})",
            "fld f30, 480({b})",
            "fld f31, 496({b})",
            "fscsr {t}",
            b = in(reg) base,
            t = in(reg) fcsr,
            options(nostack),
        );
    }
}

/// **Vào U-mode**: khôi phục ngữ cảnh FPU của TCB đang chạy (hoặc đặt trạng thái **mới**).
///
/// Gọi **một cửa** ở [`crate::user::enter_loaded_with_root`] — mọi ảnh đi qua đó nên không có
/// đường "vào U-mode nửa vời".
pub fn restore_current() {
    let Some(tcb) = current_tcb() else {
        return;
    };
    if TRACKER.is_valid(tcb) {
        if let Some(slot) = TRACKER.slot_of(tcb) {
            // SAFETY: `slot` do tracker cấp cho đúng `tcb`; vùng đã được `save_raw` ghi (đánh
            // dấu hợp lệ chỉ sau khi ghi xong).
            unsafe { restore_raw(area_ptr(slot)) };
            TRACKER.record_restore();
            set_fs(FS_CLEAN);
            return;
        }
    }
    TRACKER.record_fresh();
    set_fs(FS_INITIAL);
}

/// **Ra khỏi U-mode**: lưu ngữ cảnh FPU của TCB đang chạy **nếu** phần cứng báo bẩn.
pub fn save_current() {
    let Some(tcb) = current_tcb() else {
        return;
    };
    if read_fs() != FS_DIRTY {
        TRACKER.record_skip();
        set_fs(FS_CLEAN);
        return;
    }
    let Some(slot) = TRACKER.slot_for(tcb) else {
        // Tràn khe ⇒ **fail-closed**: không ghi đè vùng của TCB khác. `FS = Clean` để lượt sau
        // không lưu lại rác; TAP đọc `overflow` để bắt trường hợp này.
        set_fs(FS_CLEAN);
        return;
    };
    // SAFETY: `slot` vừa được cấp cho `tcb`; không hart nào khác chạy cùng TCB này.
    unsafe { save_raw(area_ptr(slot)) };
    TRACKER.mark_valid(tcb);
    TRACKER.record_save();
    set_fs(FS_CLEAN);
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
