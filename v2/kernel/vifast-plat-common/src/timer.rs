// SPDX-License-Identifier: MIT OR Apache-2.0
//! Timer **SBI TIME** (KT-47): hằng số thiết kế `[TK]`, phân loại tick, bộ đếm tick/late/max_delta.
//!
//! Tệp này **không có `unsafe`** (`vifast-plat-common` là `#![forbid(unsafe_code)]`): asm/CSR
//! (`rdtime`, `ecall` SBI, `sie`/`sstatus`) nằm ở `kernel/vifast-plat-riscv64` (D2-17), còn *ngữ
//! nghĩa* timer — đơn vị, deadline tuyệt đối, phát hiện mất tick, đo drift, đếm tick — ở đây để
//! **test được trên host** (D2-72 Q12; `tests/timer.rs`).
//!
//! # Hằng số `[TK]` (D2-72 Q6)
//!
//! | Hằng | Giá trị | Nghĩa |
//! |---|---|---|
//! | [`TIMEBASE_HZ`] | 10.000.000 | timebase QEMU `virt`; `rdtime` đếm theo đơn vị này |
//! | [`TICK_HZ`] | 100 | tick 10 ms |
//! | [`TICK_TICKS`] | 100.000 | một tick tính bằng đơn vị timebase |
//! | [`TICK_NS`] | 10.000.000 | một tick tính bằng ns |
//!
//! # Mỗi hart một deadline **tuyệt đối**
//!
//! Handler nạp `sbi_set_timer(deadline)` với deadline **tuyệt đối** (không phải `now + TICK`):
//! deadline kế là các bội số của [`TICK_TICKS`] tính từ deadline cũ, nên sai số **không cộng dồn**
//! (không trôi — D2-72 Q6 "mỗi hart một deadline"). Mỗi ngắt timer, [`classify`] tính:
//!
//! - `delta = now − last` — khoảng cách giữa hai lần quan sát tick (đo drift, đơn vị timebase);
//! - `late_by = now − deadline` — trễ so với deadline đã hẹn;
//! - `missed = late_by / TICK_TICKS` — số deadline bị bỏ: mỗi chu kỳ tick **trọn vẹn** trôi qua
//!   sau deadline thì deadline đó không còn được giao riêng nữa; `missed > 0` ⇔ **mất tick**
//!   (yêu cầu KT-47: "phát hiện mất tick (`trễ > 1 tick`)"). Trễ dưới một tick thì `missed = 0`
//!   (chỉ trễ do độ trễ ngắt, không mất tick).
//!
//! `next_deadline = deadline + (missed+1)·TICK_TICKS` luôn **> now** nên SBI TIME không bắn lại
//! ngay; nếu trễ hơn một tick thì các deadline bị bỏ được nhảy qua **một lần** (không dồn interrupt).
//!
//! [`TimerCounters`] (toàn cục: [`TIMER_STATS`]) cộng dồn `ticks/late/missed/max_delta/min_delta/
//! max_late` và đếm bằng chứng `sstatus` (`trap_spie_on` — SIE trước trap thấy qua SPIE;
//! `handler_sie_off` — SIE=0 trong handler) cho TAP KT-47.
//!
//! **Không** thuộc KT-47: PLIC/UART IRQ (KT-48), SMP/PerHart (KT-49), preemption thật (KT-49+).

use core::sync::atomic::{AtomicU64, Ordering};

use crate::trap::{SSTATUS_SIE, SSTATUS_SPIE};

/// Timebase QEMU `virt`: 10 MHz (DTB `timebase-frequency`; D2-72 Q6) `[TK]`.
pub const TIMEBASE_HZ: u64 = 10_000_000;
/// Tick của kernel: 100 Hz = 10 ms (D2-72 Q6) `[TK]`.
pub const TICK_HZ: u64 = 100;
/// Một tick tính bằng đơn vị timebase: `TIMEBASE_HZ / TICK_HZ` = 100.000.
pub const TICK_TICKS: u64 = TIMEBASE_HZ / TICK_HZ;
/// Số ns một giây.
pub const NS_PER_SEC: u64 = 1_000_000_000;
/// Một tick tính bằng ns (10 ms).
pub const TICK_NS: u64 = NS_PER_SEC / TICK_HZ;
/// Cận dưới của Δ tick đo được: 9 ms `[TK]` (DoD KT-47: Δ ∈ [9,11] ms).
pub const TICK_DELTA_MIN_NS: u64 = 9_000_000;
/// Cận trên của Δ tick đo được: 11 ms `[TK]` (DoD KT-47: Δ ∈ [9,11] ms).
pub const TICK_DELTA_MAX_NS: u64 = 11_000_000;

// Bất biến của hằng số: sai một trong số này thì mọi phép đo drift/ns đều vô nghĩa.
const _: () = {
    assert!(TICK_TICKS == 100_000);
    assert!(TICK_NS == 10_000_000);
    assert!(TICK_DELTA_MIN_NS < TICK_NS);
    assert!(TICK_NS < TICK_DELTA_MAX_NS);
};

/// Đổi tick timebase → ns (10 MHz nên **chính xác tuyệt đối**: `ticks × 100`).
pub const fn ticks_to_ns(ticks: u64) -> u64 {
    ((ticks as u128 * NS_PER_SEC as u128) / TIMEBASE_HZ as u128) as u64
}

/// Đổi ns → tick timebase (làm tròn xuống).
pub const fn ns_to_ticks(ns: u64) -> u64 {
    ((ns as u128 * TIMEBASE_HZ as u128) / NS_PER_SEC as u128) as u64
}

/// Kết quả phân loại **một** ngắt timer (hàm thuần — test host được).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TickSample {
    /// `now − last` (đơn vị timebase) — dùng đo drift Δ tick.
    pub delta: u64,
    /// `now − deadline` (0 nếu ngắt đến sớm hơn deadline).
    pub late_by: u64,
    /// Số deadline bị bỏ (`late_by / TICK_TICKS`); `> 0` ⇔ trễ hơn một tick ⇔ mất tick.
    pub missed: u64,
    /// Deadline kế — luôn **> now**.
    pub next_deadline: u64,
}

impl TickSample {
    /// `true` ⇔ tick này **mất tick** (đã bỏ ít nhất một deadline, tức trễ trọn một chu kỳ).
    pub const fn is_late(&self) -> bool {
        self.missed > 0
    }

    /// `delta` của tick này tính bằng ns (đo drift).
    pub const fn delta_ns(&self) -> u64 {
        ticks_to_ns(self.delta)
    }
}

/// Phân loại một ngắt timer tại `now`: `last` = lần quan sát trước, `deadline` = deadline đã hẹn.
///
/// Thuần (không asm/atomic) nên host test kiểm được cả trường hợp trễ/mất tick mà phần cứng khó
/// tạo ra: xem `tests/timer.rs`.
pub const fn classify(last: u64, deadline: u64, now: u64) -> TickSample {
    let delta = now.saturating_sub(last);
    if now < deadline {
        // Ngắt đến sớm hơn deadline (SBI TIME không làm vậy): giữ nguyên deadline, không tính trễ.
        return TickSample {
            delta,
            late_by: 0,
            missed: 0,
            next_deadline: deadline,
        };
    }
    let late_by = now - deadline;
    // late_by ∈ [k·TICK, (k+1)·TICK) ⇒ đã bỏ k deadline (0 khi trễ < 1 tick: chỉ là độ trễ ngắt).
    let missed = late_by / TICK_TICKS;
    let next_deadline = deadline.saturating_add((missed + 1).saturating_mul(TICK_TICKS));
    TickSample {
        delta,
        late_by,
        missed,
        next_deadline,
    }
}

/// Ảnh chụp bộ đếm timer — `Copy` để TAP đọc nhất quán.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TimerSnapshot {
    /// Số ngắt timer đã xử lý (= số tick đã quan sát).
    pub ticks: u64,
    /// Số tick trễ hơn một tick (có deadline bị bỏ) — TAP `no lost tick` đòi `0`.
    pub late: u64,
    /// Tổng số deadline bị bỏ trên mọi tick (0 ⇔ không mất tick nào).
    pub missed: u64,
    /// Số tick có `now <= last` (vi phạm monotonic) — TAP `monotonic` đòi `0`.
    pub non_monotonic: u64,
    /// `delta` lớn nhất giữa hai tick (đơn vị timebase) — đo drift.
    pub max_delta: u64,
    /// `delta` nhỏ nhất giữa hai tick (đơn vị timebase) — đo drift.
    pub min_delta: u64,
    /// `late_by` lớn nhất từng thấy (đơn vị timebase).
    pub max_late: u64,
    /// `rdtime` lúc arm (mốc đầu).
    pub first: u64,
    /// `rdtime` của tick gần nhất.
    pub last: u64,
    /// Deadline đã hẹn cho tick kế.
    pub deadline: u64,
    /// `true` ⇔ đã arm (đã nạp deadline đầu).
    pub armed: bool,
    /// Số trap timer mà `trapframe.sstatus.SPIE = 1` — hardware chuyển `SIE` sang `SPIE` khi vào
    /// trap, nên đây mới là bằng chứng "ngữ cảnh bị ngắt **đang bật** ngắt".
    pub trap_spie_on: u64,
    /// Số trap timer mà `sstatus.SIE = 0` **trong handler** (hardware xoá SIE khi vào trap).
    pub handler_sie_off: u64,
}

/// Bộ đếm timer toàn cục — một hart ở KT-47 (PerHart là KT-49).
///
/// Toàn bộ trường là `AtomicU64` dù một hart: handler ghi, code TAP đọc — không cần `unsafe`,
/// không cần `static mut` (D2-17).
#[derive(Debug)]
pub struct TimerCounters {
    ticks: AtomicU64,
    late: AtomicU64,
    missed: AtomicU64,
    non_monotonic: AtomicU64,
    max_delta: AtomicU64,
    min_delta: AtomicU64,
    max_late: AtomicU64,
    first: AtomicU64,
    last: AtomicU64,
    deadline: AtomicU64,
    armed: AtomicU64,
    trap_spie_on: AtomicU64,
    handler_sie_off: AtomicU64,
}

impl TimerCounters {
    /// Bộ đếm rỗng — `const` để đặt trong `static`.
    pub const fn new() -> Self {
        Self {
            ticks: AtomicU64::new(0),
            late: AtomicU64::new(0),
            missed: AtomicU64::new(0),
            non_monotonic: AtomicU64::new(0),
            max_delta: AtomicU64::new(0),
            min_delta: AtomicU64::new(0),
            max_late: AtomicU64::new(0),
            first: AtomicU64::new(0),
            last: AtomicU64::new(0),
            deadline: AtomicU64::new(0),
            armed: AtomicU64::new(0),
            trap_spie_on: AtomicU64::new(0),
            handler_sie_off: AtomicU64::new(0),
        }
    }

    /// Xoá toàn bộ bộ đếm (ảnh thật không reset; test host/TAP đầu vòng dùng).
    pub fn reset(&self) {
        for field in [
            &self.ticks,
            &self.late,
            &self.missed,
            &self.non_monotonic,
            &self.max_delta,
            &self.min_delta,
            &self.max_late,
            &self.first,
            &self.last,
            &self.deadline,
            &self.trap_spie_on,
            &self.handler_sie_off,
        ] {
            field.store(0, Ordering::Relaxed);
        }
        self.armed.store(0, Ordering::Relaxed);
    }

    /// Nạp deadline **đầu** (`now + TICK_TICKS`) và xoá bộ đếm — gọi trước khi bật ngắt timer.
    pub fn arm(&self, now: u64, first_deadline: u64) {
        self.reset();
        self.first.store(now, Ordering::Relaxed);
        self.last.store(now, Ordering::Relaxed);
        self.deadline.store(first_deadline, Ordering::Relaxed);
        self.armed.store(1, Ordering::Relaxed);
    }

    /// `true` ⇔ đã arm (có deadline đang chờ).
    pub fn is_armed(&self) -> bool {
        self.armed.load(Ordering::Relaxed) != 0
    }

    /// Ghi một ngắt timer tại `now`; trả deadline kế (luôn **> now**) để handler gọi `set_timer`.
    ///
    /// Trả `None` nếu chưa arm (ngắt lạc) — handler không nạp deadline thì không bắn lại.
    pub fn record_tick(&self, now: u64) -> Option<TickSample> {
        if self.armed.load(Ordering::Relaxed) == 0 {
            return None;
        }
        let last = self.last.load(Ordering::Relaxed);
        let deadline = self.deadline.load(Ordering::Relaxed);
        let sample = classify(last, deadline, now);
        let index = self.ticks.fetch_add(1, Ordering::Relaxed);
        if now <= last {
            self.non_monotonic.fetch_add(1, Ordering::Relaxed);
        }
        if sample.is_late() {
            self.late.fetch_add(1, Ordering::Relaxed);
            self.missed.fetch_add(sample.missed, Ordering::Relaxed);
        }
        self.max_delta.fetch_max(sample.delta, Ordering::Relaxed);
        if index == 0 {
            self.min_delta.store(sample.delta, Ordering::Relaxed);
        } else {
            self.min_delta.fetch_min(sample.delta, Ordering::Relaxed);
        }
        self.max_late.fetch_max(sample.late_by, Ordering::Relaxed);
        self.last.store(now, Ordering::Relaxed);
        self.deadline.store(sample.next_deadline, Ordering::Relaxed);
        Some(sample)
    }

    /// Ghi bằng chứng `sstatus` của một trap timer: `trap_sstatus` = giá trị **trong trapframe**
    /// (lúc trap), `handler_sstatus` = `csrr sstatus` **trong** handler.
    ///
    /// KT-47 (P2a — hardware, không phải kernel):
    /// - khi vào trap hardware làm `SPIE ← SIE`, `SIE ← 0` ⇒ trapframe có **`SPIE = 1`** (ngữ cảnh
    ///   bị ngắt đang bật ngắt) và `SIE = 0`; đếm bằng `trap_spie_on`;
    /// - trong handler `SIE = 0` (nên không có trap lồng) ⇒ đếm bằng `handler_sie_off`;
    /// - `sret` khôi phục `SIE ← SPIE` ⇒ cần cả hai bộ đếm bằng `ticks` (TAP `timer irq`).
    pub fn record_sstatus(&self, trap_sstatus: u64, handler_sstatus: u64) {
        if trap_sstatus & SSTATUS_SPIE != 0 {
            self.trap_spie_on.fetch_add(1, Ordering::Relaxed);
        }
        if handler_sstatus & SSTATUS_SIE == 0 {
            self.handler_sie_off.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Đọc nhất quán (một hart; từng trường `Relaxed` — đủ cho TAP KT-47).
    pub fn snapshot(&self) -> TimerSnapshot {
        TimerSnapshot {
            ticks: self.ticks.load(Ordering::Relaxed),
            late: self.late.load(Ordering::Relaxed),
            missed: self.missed.load(Ordering::Relaxed),
            non_monotonic: self.non_monotonic.load(Ordering::Relaxed),
            max_delta: self.max_delta.load(Ordering::Relaxed),
            min_delta: self.min_delta.load(Ordering::Relaxed),
            max_late: self.max_late.load(Ordering::Relaxed),
            first: self.first.load(Ordering::Relaxed),
            last: self.last.load(Ordering::Relaxed),
            deadline: self.deadline.load(Ordering::Relaxed),
            armed: self.armed.load(Ordering::Relaxed) != 0,
            trap_spie_on: self.trap_spie_on.load(Ordering::Relaxed),
            handler_sie_off: self.handler_sie_off.load(Ordering::Relaxed),
        }
    }
}

impl Default for TimerCounters {
    fn default() -> Self {
        Self::new()
    }
}

/// Bộ đếm timer của ảnh kernel (một hart ở KT-47; PerHart là KT-49).
pub static TIMER_STATS: TimerCounters = TimerCounters::new();
