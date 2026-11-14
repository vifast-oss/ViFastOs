// SPDX-License-Identifier: MIT OR Apache-2.0
//! Timer **SBI TIME** trên QEMU `virt` — KT-47 (asm/CSR/SBI; logic thuần ở
//! `vifast-plat-common::timer`).
//!
//! # Ràng buộc thiết kế (D2-72 Q6)
//!
//! - **SBI TIME**: EID `0x5449_4D45`, FID 0 = `sbi_set_timer(uint64 deadline)`; deadline là
//!   **tuyệt đối**, đơn vị timebase. Không dùng CLINT trực tiếp (không map `0x0200_0000` —
//!   KT-46 chỉ map UART/PLIC), không Sstc (`-cpu rv64` không bảo đảm).
//! - **Timebase 10 MHz** (`rdtime`), **tick 100 Hz** = [`TICK_TICKS`](vifast_plat_common::timer::TICK_TICKS)
//!   = 100.000 đơn vị timebase = 10 ms.
//! - **Mỗi hart một deadline** (một hart ở KT-47; PerHart là KT-49): deadline nằm trong
//!   [`TIMER_STATS`], handler nạp deadline kế = bội số `TICK_TICKS` của deadline cũ
//!   (tuyệt đối ⇒ không trôi).
//!
//! # `sstatus.SIE` khi vào/ra trap (P2a — KT-45 đã bỏ `csrw sstatus`)
//!
//! Hardware khi trap: `SPIE ← SIE`, `SIE ← 0`, `SPP ← 1`. `sret`: `SIE ← SPIE`. Vì vậy:
//!
//! - [`enable_timer`] là chỗ **duy nhất** bật cổng **timer**: `sie.STIE = 1` **trước** rồi
//!   `sstatus.SIE = 1` (bật nguồn trước, mở cổng sau — không có cửa sổ ngắt khi nguồn chưa sẵn
//!   sàng). KT-48 thêm `plic::enable_external` cùng hợp đồng cho ngắt external (`sie.SEIE` trước
//!   rồi `sstatus.SIE`); cả hai đều **không** bị trap entry/handler ghi đè.
//! - `trap_entry`/handler **không** ghi `sstatus` (KT-45 P2a); trong handler `SIE = 0` (hardware
//!   xoá) nên **không** có trap lồng; `sret` tự khôi phục `SIE` từ `SPIE` ⇒ sau trap SIE = 1 trở
//!   lại. Handler chỉ *đếm* bằng chứng (`trap_spie_on`, `handler_sie_off`) chứ không sửa.
//! - [`disable_timer`] đảo thứ tự: `sstatus.SIE = 0` rồi `sie.STIE = 0`.
//!
//! `unsafe` nằm trong danh sách đóng `kernel/vifast-plat-*` của D2-17.
//!
//! **Không** thuộc KT-47: PLIC/UART IRQ (KT-48), SMP/PerHart (KT-49), IPI (KT-50), preemption
//! (KT-49+).

use core::arch::asm;

use vifast_plat_common::timer::{TickSample, TICK_TICKS, TIMEBASE_HZ, TIMER_STATS};

/// EID SBI TIME (SBI v2.0 §4) — D2-72 Q6.
pub const SBI_EXT_TIME: u64 = 0x5449_4D45;
/// FID 0 của SBI TIME: `sbi_set_timer(uint64 deadline)`.
pub const SBI_TIME_SET_TIMER: u64 = 0;
/// `sie.STIE` — supervisor timer interrupt enable (bit 5).
pub const SIE_STIE: u64 = 1 << 5;
/// `sstatus.SIE` — supervisor interrupt enable (bit 1). Trùng
/// [`vifast_plat_common::trap::SSTATUS_SIE`].
pub const SSTATUS_SIE: u64 = 1 << 1;

/// Thông tin lần **arm** đầu tiên — TAP/log in ra bằng chứng (số đo thật của lần boot này).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimerArm {
    /// `rdtime` tại thời điểm arm.
    pub now: u64,
    /// Deadline đầu = `now + TICK_TICKS`.
    pub first_deadline: u64,
    /// `true` ⇔ đọc lại thấy `sie.STIE = 1`.
    pub sie_stie: bool,
    /// `true` ⇔ đọc lại thấy `sstatus.SIE = 1`.
    pub sstatus_sie: bool,
}

/// `rdtime` — timebase 10 MHz, monotonic, S-mode đọc được (OpenSBI đặt `mcounteren.TM`).
pub fn now_ticks() -> u64 {
    let ticks: u64;
    // SAFETY: `rdtime` chỉ đọc CSR `time`, không tác dụng phụ, không chạm bộ nhớ.
    unsafe { asm!("rdtime {}", out(reg) ticks, options(nomem, nostack)) };
    ticks
}

/// Đọc `sie` (CSR) — TAP kiểm `STIE`.
pub fn read_sie() -> u64 {
    let value: u64;
    // SAFETY: `csrr sie` chỉ đọc CSR, không tác dụng phụ.
    unsafe { asm!("csrr {}, sie", out(reg) value, options(nomem, nostack)) };
    value
}

/// Đọc `sstatus` (CSR) — TAP kiểm `SIE` lúc trap/trong handler/sau `sret`.
pub fn read_sstatus() -> u64 {
    let value: u64;
    // SAFETY: `csrr sstatus` chỉ đọc CSR, không tác dụng phụ.
    unsafe { asm!("csrr {}, sstatus", out(reg) value, options(nomem, nostack)) };
    value
}

/// Gọi **SBI TIME** `sbi_set_timer(deadline)` — deadline tuyệt đối (đơn vị timebase).
///
/// Deadline ở tương lai đồng thời **xoá** ngắt timer đang chờ; `u64::MAX` = không bao giờ bắn.
pub fn set_timer_deadline(deadline: u64) {
    // SAFETY: `ecall` SBI hợp lệ (OpenSBI đã nạp trước khi vào S-mode); SBI bảo toàn mọi thanh ghi
    // trừ `a0`/`a1` (mã trả về), nên chỉ khai báo hai `lateout` đó. `ecall` có thể ghi bộ nhớ phía
    // firmware ⇒ **không** `nomem`.
    unsafe {
        asm!(
            "ecall",
            in("a7") SBI_EXT_TIME,
            in("a6") SBI_TIME_SET_TIMER,
            in("a0") deadline,
            lateout("a0") _,
            lateout("a1") _,
            options(nostack),
        );
    }
}

/// Bật ngắt timer: `sie.STIE = 1` **rồi** `sstatus.SIE = 1`; trả `true` ⇔ đọc lại thấy cả hai.
pub fn enable_timer() -> bool {
    // SAFETY: ghi hai CSR ngắt của hart hiện tại; giá trị hằng, không chạm bộ nhớ.
    unsafe {
        asm!(
            "csrs sie, {stie}",
            "csrs sstatus, {sie}",
            stie = in(reg) SIE_STIE,
            sie = in(reg) SSTATUS_SIE,
            options(nomem, nostack, preserves_flags),
        );
    }
    read_sie() & SIE_STIE != 0 && read_sstatus() & SSTATUS_SIE != 0
}

/// Tắt ngắt timer: `sstatus.SIE = 0` **rồi** `sie.STIE = 0` (đảo thứ tự [`enable_timer`]).
pub fn disable_timer() {
    // SAFETY: ghi hai CSR ngắt của hart hiện tại; giá trị hằng, không chạm bộ nhớ.
    unsafe {
        asm!(
            "csrc sstatus, {sie}",
            "csrc sie, {stie}",
            stie = in(reg) SIE_STIE,
            sie = in(reg) SSTATUS_SIE,
            options(nomem, nostack, preserves_flags),
        );
    }
}

/// Nạp deadline **đầu** (`now + TICK_TICKS`) rồi bật ngắt timer — dùng cho ảnh sản phẩm và TAP.
///
/// Thứ tự: arm (xoá bộ đếm + đặt `last/deadline`) → `set_timer(deadline)` → `enable_timer()`; nhờ
/// vậy không có cửa sổ "SIE bật nhưng deadline chưa nạp".
pub fn init() -> TimerArm {
    let now = now_ticks();
    let first_deadline = now.saturating_add(TICK_TICKS);
    TIMER_STATS.arm(now, first_deadline);
    set_timer_deadline(first_deadline);
    let _ = enable_timer();
    TimerArm {
        now,
        first_deadline,
        sie_stie: read_sie() & SIE_STIE != 0,
        sstatus_sie: read_sstatus() & SSTATUS_SIE != 0,
    }
}

/// Xử lý **một** ngắt timer trong `trap_dispatch`: đếm tick, ghi bằng chứng `sstatus.SIE`, trả
/// deadline kế để handler nạp `sbi_set_timer`.
///
/// `trap_sstatus` = `sstatus` trong trapframe (lúc trap), `handler_sstatus` = `csrr sstatus` **bây
/// giờ** (trong handler, SIE đã bị hardware xoá). Trả `None` nếu chưa arm (ngắt lạc) — handler nên
/// nạp `u64::MAX` để dập ngắt thay vì bắn lại liên tục.
pub fn on_timer_interrupt(trap_sstatus: u64, handler_sstatus: u64) -> Option<TickSample> {
    let now = now_ticks();
    let sample: TickSample = TIMER_STATS.record_tick(now)?;
    TIMER_STATS.record_sstatus(trap_sstatus, handler_sstatus);
    Some(sample)
}

/// Timebase của QEMU `virt` — TAP/log đối chiếu `TIMEBASE_HZ` (10 MHz, D2-72 Q6).
pub const fn timebase_hz() -> u64 {
    TIMEBASE_HZ
}
