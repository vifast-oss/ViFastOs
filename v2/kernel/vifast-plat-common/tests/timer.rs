// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho timer SBI (KT-47) — logic thuần ở `vifast-plat-common::timer`.
//!
//! D2-72 Q12: asm/CSR/SBI (`rdtime`, `ecall`, `sie`/`sstatus`) không chạy được trên host và không
//! thuộc Miri, nhưng **ngữ nghĩa** timer đã tách khỏi asm nên test được ở đây:
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common
//! ```
//!
//! Phủ: hằng số `[TK]` (100 Hz, 10 MHz, 100.000 tick/timebase, 10 ms), quy đổi tick↔ns, phân loại
//! một tick (`classify`: đúng hẹn / trễ ≤ 1 tick / **mất tick** / ngắt sớm), bộ đếm 1.000 tick
//! không mất, phát hiện mất tick + tự căn lại deadline tuyệt đối, monotonic, bằng chứng
//! `sstatus.SIE`, và `record_tick` trước `arm`.

use vifast_plat_common::timer::{
    classify, ns_to_ticks, ticks_to_ns, TimerCounters, NS_PER_SEC, TICK_DELTA_MAX_NS,
    TICK_DELTA_MIN_NS, TICK_HZ, TICK_NS, TICK_TICKS, TIMEBASE_HZ,
};
use vifast_plat_common::trap::{SSTATUS_SIE, SSTATUS_SPIE};

#[test]
fn hang_so_thiet_ke_dung_d2_72_q6() {
    // Hằng số `[TK]`: timebase 10 MHz, tick 100 Hz, một tick = 100.000 timebase = 10 ms.
    assert_eq!(TIMEBASE_HZ, 10_000_000);
    assert_eq!(TICK_HZ, 100);
    assert_eq!(TICK_TICKS, 100_000);
    assert_eq!(TICK_NS, 10_000_000);
    assert_eq!(TICK_TICKS * TICK_HZ, TIMEBASE_HZ);
    assert_eq!(NS_PER_SEC / TICK_HZ, TICK_NS);
    // DoD KT-47: Δ tick ∈ [9,11] ms.
    assert_eq!(
        (TICK_DELTA_MIN_NS, TICK_DELTA_MAX_NS),
        (9_000_000, 11_000_000)
    );
}

#[test]
fn doi_don_vi_tick_va_ns_chinh_xac() {
    assert_eq!(ticks_to_ns(0), 0);
    assert_eq!(ticks_to_ns(1), 100);
    assert_eq!(ticks_to_ns(TICK_TICKS), TICK_NS);
    assert_eq!(ticks_to_ns(TIMEBASE_HZ), NS_PER_SEC);
    assert_eq!(ns_to_ticks(TICK_NS), TICK_TICKS);
    assert_eq!(ns_to_ticks(NS_PER_SEC), TIMEBASE_HZ);
    // 1 giờ vẫn nằm trong u64 (10 MHz × 3600 = 3,6e10 tick; ns = 3,6e12).
    assert_eq!(ticks_to_ns(TIMEBASE_HZ * 3600), NS_PER_SEC * 3600);
    // Làm tròn xuống khi ns lẻ.
    assert_eq!(ns_to_ticks(99), 0);
    assert_eq!(ns_to_ticks(100), 1);
}

#[test]
fn classify_dung_hen_va_tre_nho_hon_mot_tick() {
    let first = classify(0, TICK_TICKS, TICK_TICKS + 50);
    assert_eq!(first.delta, TICK_TICKS + 50);
    assert_eq!(first.late_by, 50);
    assert_eq!(first.missed, 0);
    assert!(!first.is_late());
    assert_eq!(first.next_deadline, 2 * TICK_TICKS);
    assert!(first.next_deadline > TICK_TICKS + 50);
    assert_eq!(first.delta_ns(), TICK_NS + 5_000);

    // Trễ dưới **một** tick: chỉ là độ trễ ngắt, KHÔNG mất tick.
    let under = classify(0, TICK_TICKS, 2 * TICK_TICKS - 1);
    assert_eq!(under.missed, 0);
    assert!(!under.is_late());
    assert_eq!(under.next_deadline, 2 * TICK_TICKS);

    // Trễ trọn **một** tick: deadline kế (đúng bằng `now`) không còn được giao riêng ⇒ mất 1 tick.
    let edge = classify(0, TICK_TICKS, 2 * TICK_TICKS);
    assert_eq!(edge.missed, 1);
    assert!(edge.is_late());
    assert_eq!(edge.late_by, TICK_TICKS);
    assert_eq!(edge.next_deadline, 3 * TICK_TICKS);
}

#[test]
fn classify_tre_hon_mot_tick_la_mat_tick() {
    // Trễ 1 tick + 1 đơn vị timebase ⇒ đã bỏ đúng một deadline.
    let one = classify(TICK_TICKS, TICK_TICKS, TICK_TICKS + TICK_TICKS + 1);
    assert_eq!(one.late_by, TICK_TICKS + 1);
    assert_eq!(one.missed, 1);
    assert!(one.is_late());
    assert_eq!(one.next_deadline, 3 * TICK_TICKS);
    assert!(one.next_deadline > TICK_TICKS + TICK_TICKS + 1);

    // Trễ đúng 2 tick ⇒ bỏ 2 deadline; deadline kế nhảy qua 3 chu kỳ (không dồn interrupt).
    let two = classify(0, TICK_TICKS, 3 * TICK_TICKS);
    assert_eq!(two.missed, 2);
    assert_eq!(two.next_deadline, 4 * TICK_TICKS);
    assert!(two.next_deadline > 3 * TICK_TICKS);
}

#[test]
fn classify_ngat_som_giu_nguyen_deadline() {
    // SBI TIME không bắn sớm, nhưng nếu có thì không được đẩy deadline ra xa.
    let early = classify(0, 5 * TICK_TICKS, 3 * TICK_TICKS);
    assert_eq!(early.late_by, 0);
    assert_eq!(early.missed, 0);
    assert_eq!(early.next_deadline, 5 * TICK_TICKS);
    assert_eq!(early.delta, 3 * TICK_TICKS);
}

#[test]
fn classify_khong_tran_khi_moc_rat_lon() {
    let sample = classify(0, 0, u64::MAX);
    // saturating: không panic, không wrap; deadline kế kẹt ở u64::MAX (không bắn lại được).
    assert_eq!(sample.next_deadline, u64::MAX);
    assert!(sample.is_late());
}

#[test]
fn counters_1000_tick_khong_mat_tick() {
    let counters = TimerCounters::new();
    let start = 1_000_000u64;
    let first = start + TICK_TICKS;
    counters.arm(start, first);
    assert!(counters.is_armed());
    // Jitter "đo được" giả lập: 30..900 đơn vị timebase (3..90 µs) — trong một tick.
    for index in 0..1_000u64 {
        // Jitter "đo được" giả lập 30..870 đơn vị timebase (3..87 µs) — dưới một tick.
        let jitter = 30 + ((index * 17 + 5) % 29) * 30;
        let now = first + index * TICK_TICKS + jitter;
        let sample = counters.record_tick(now).expect("đã arm");
        assert_eq!(sample.missed, 0, "tick {index}");
        assert!(!sample.is_late(), "tick {index}");
        assert!(sample.delta + 900 >= TICK_TICKS, "tick {index}");
        assert!(sample.delta <= TICK_TICKS + 900, "tick {index}");
        assert!(sample.next_deadline > now, "tick {index}");
    }
    let snap = counters.snapshot();
    assert_eq!(snap.ticks, 1_000);
    assert_eq!(snap.late, 0);
    assert_eq!(snap.missed, 0);
    assert_eq!(snap.non_monotonic, 0);
    assert_eq!(snap.first, start);
    assert_eq!(
        snap.last,
        first + 999 * TICK_TICKS + (30 + ((999 * 17 + 5) % 29) * 30)
    );
    assert_eq!(snap.max_late, 870);
    assert!(snap.min_delta + 900 >= TICK_TICKS);
    assert!(snap.max_delta <= TICK_TICKS + 900);
    assert!(snap.min_delta > TICK_TICKS - 900);
    // Δ tick đo được ∈ [9,11] ms như DoD KT-47.
    assert!(ticks_to_ns(snap.min_delta) >= TICK_DELTA_MIN_NS);
    assert!(ticks_to_ns(snap.max_delta) <= TICK_DELTA_MAX_NS);
}

#[test]
fn counters_phat_hien_mat_tick_va_tu_can_lai_deadline() {
    let counters = TimerCounters::new();
    counters.arm(0, TICK_TICKS);
    // Tick 1 đúng hẹn.
    assert_eq!(counters.record_tick(TICK_TICKS + 40).unwrap().missed, 0);
    // Tick 2 đến trễ 2,5 tick (deadline 200.000, now 450.000) ⇒ mất 2 tick.
    let late = counters.record_tick(450_000).unwrap();
    assert_eq!(late.late_by, 250_000);
    assert_eq!(late.missed, 2);
    assert!(late.is_late());
    assert_eq!(late.next_deadline, 500_000); // bội số tuyệt đối, không cộng dồn từ `now`
                                             // Tick 3 sau khi căn lại: đúng hẹn (deadline 500.000 ⇒ now 500.050).
    let realigned = counters.record_tick(500_050).unwrap();
    assert_eq!(realigned.missed, 0);
    assert_eq!(realigned.next_deadline, 600_000);
    let snap = counters.snapshot();
    assert_eq!(snap.ticks, 3);
    assert_eq!(snap.late, 1);
    assert_eq!(snap.missed, 2);
    assert_eq!(snap.max_late, 250_000);
}

#[test]
fn counters_bat_vi_pham_monotonic() {
    let counters = TimerCounters::new();
    counters.arm(100, 200);
    counters.record_tick(200).unwrap();
    // `now` không tăng (hoặc giảm) ⇒ vi phạm monotonic bị đếm.
    counters.record_tick(200).unwrap();
    counters.record_tick(150).unwrap();
    let snap = counters.snapshot();
    assert_eq!(snap.ticks, 3);
    assert_eq!(snap.non_monotonic, 2);
}

#[test]
fn counters_dem_bang_chung_sstatus_sie() {
    let counters = TimerCounters::new();
    counters.arm(0, TICK_TICKS);
    for index in 0..3u64 {
        counters.record_tick(TICK_TICKS * (index + 1) + 20).unwrap();
        // Trapframe lúc trap: hardware đã làm SPIE ← SIE = 1 và SIE ← 0; trong handler SIE = 0.
        counters.record_sstatus(SSTATUS_SPIE | 0x6100, 0);
    }
    let snap = counters.snapshot();
    assert_eq!(snap.ticks, 3);
    assert_eq!(snap.trap_spie_on, 3);
    assert_eq!(snap.handler_sie_off, 3);
    // Trường hợp xấu: trap từ ngữ cảnh tắt ngắt / handler còn bật ngắt ⇒ không cộng. Lưu ý SIE
    // trần (không phải SPIE) **không** phải bằng chứng: vào trap hardware đã xoá SIE.
    counters.record_sstatus(SSTATUS_SIE, SSTATUS_SIE);
    let snap = counters.snapshot();
    assert_eq!(snap.trap_spie_on, 3);
    assert_eq!(snap.handler_sie_off, 3);
}

#[test]
fn counters_chua_arm_thi_record_tick_tra_none() {
    let counters = TimerCounters::new();
    assert!(!counters.is_armed());
    assert_eq!(counters.record_tick(123), None);
    assert_eq!(counters.snapshot().ticks, 0);
    counters.arm(0, TICK_TICKS);
    assert!(counters.record_tick(50).is_some());
    assert_eq!(counters.snapshot().ticks, 1);
}

#[test]
fn arm_lai_xoa_bo_dem_va_dat_moc_moi() {
    let counters = TimerCounters::new();
    counters.arm(0, TICK_TICKS);
    counters.record_tick(TICK_TICKS + 10).unwrap();
    counters.record_tick(2 * TICK_TICKS + 20).unwrap();
    assert_eq!(counters.snapshot().ticks, 2);
    counters.arm(7_000, 7_000 + TICK_TICKS);
    let snap = counters.snapshot();
    assert!(snap.armed);
    assert_eq!(snap.ticks, 0);
    assert_eq!(snap.late, 0);
    assert_eq!(snap.missed, 0);
    assert_eq!(snap.first, 7_000);
    assert_eq!(snap.last, 7_000);
    assert_eq!(snap.deadline, 7_000 + TICK_TICKS);
    counters.reset();
    let empty = counters.snapshot();
    assert!(!empty.armed);
    assert_eq!(empty.ticks, 0);
    assert_eq!(empty.deadline, 0);
}
