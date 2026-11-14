// SPDX-License-Identifier: MIT OR Apache-2.0
//! Host test KT-49 — `vifast-plat-common::smp`: bố cục per-hart, `HartMask`, ngữ nghĩa SBI HSM.
//!
//! Các test này chạy trên host (không asm/CSR; D2-72 Q12) và chốt những bất biến mà arch dựa vào:
//! guard/stack/trap stack **không chồng** giữa các hart, mask/bit đúng, hart ngoài DTB bị **từ
//! chối** (không panic), bộ đếm timer/PLIC **PerHart** không lẫn.

use vifast_plat_common::plic::PerHartPlicStats;
use vifast_plat_common::smp::{
    classify_hart_status, guard_offset, hart_bit, hart_of_offset, hart_present, hart_record_offset,
    hart_start_ok, in_hart_area, is_guard_offset, mask_count, mask_of, range_mask,
    stack_bottom_offset, stack_top_offset, start_decision, trap_bottom_offset, trap_top_offset,
    valid_hart, HartMask, HartStatus, StartDecision, HART_AREA_BYTES, HART_GUARD_BYTES,
    HART_STACK_BYTES, HART_STRIDE, HART_TRAP_STACK_BYTES, MAX_HARTS, SBI_ERR_ALREADY_AVAILABLE,
    SBI_ERR_FAILED, SBI_ERR_INVALID_PARAM, SBI_HART_STARTED, SBI_HART_STOPPED, SBI_SUCCESS,
};
use vifast_plat_common::timer::PerHartTimerStats;

#[test]
fn bo_cuc_per_hart_khong_chong_va_can_trang() {
    // Mỗi record: guard 4 KiB + stack 64 KiB + trap stack 16 KiB; guard nằm ngay dưới stack của
    // chính hart đó (tràn stack chạm đúng guard của hart mình — KT-49).
    assert_eq!(
        HART_GUARD_BYTES + HART_STACK_BYTES + HART_TRAP_STACK_BYTES,
        HART_STRIDE
    );
    assert_eq!(HART_AREA_BYTES, HART_STRIDE * MAX_HARTS as u64);
    for hart in 0..MAX_HARTS {
        assert!(valid_hart(hart));
        // Record bắt đầu ở biên trang và các hart không chồng nhau.
        assert_eq!(guard_offset(hart) % 0x1000, 0);
        assert_eq!(stack_bottom_offset(hart), guard_offset(hart) + 0x1000);
        assert_eq!(
            stack_top_offset(hart),
            stack_bottom_offset(hart) + HART_STACK_BYTES
        );
        assert_eq!(trap_bottom_offset(hart), stack_top_offset(hart));
        assert_eq!(
            trap_top_offset(hart),
            trap_bottom_offset(hart) + HART_TRAP_STACK_BYTES
        );
        if hart + 1 < MAX_HARTS {
            assert!(trap_top_offset(hart) <= guard_offset(hart + 1));
        }
        // Guard của hart này nằm ngoài stack của mọi hart khác.
        for other in 0..MAX_HARTS {
            if other == hart {
                continue;
            }
            let (sb, st) = (stack_bottom_offset(other), stack_top_offset(other));
            let g = guard_offset(hart);
            assert!(
                !(g >= sb && g < st),
                "guard hart {hart} rơi vào stack hart {other}"
            );
        }
    }
    assert_eq!(trap_top_offset(MAX_HARTS - 1), HART_AREA_BYTES);
}

#[test]
fn guard_offset_va_chu_so_huu_dung() {
    for hart in 0..MAX_HARTS {
        let g = guard_offset(hart);
        assert!(is_guard_offset(g));
        assert!(is_guard_offset(g + HART_GUARD_BYTES - 1));
        assert!(
            !is_guard_offset(g + HART_GUARD_BYTES),
            "byte đầu stack không phải guard"
        );
        assert!(matches!(hart_of_offset(g), Some(h) if h == hart));
        assert!(matches!(hart_of_offset(stack_top_offset(hart)), Some(h) if h == hart));
    }
    assert!(!in_hart_area(HART_AREA_BYTES));
    assert!(!is_guard_offset(HART_AREA_BYTES));
    assert!(hart_of_offset(HART_AREA_BYTES).is_none());
}

#[test]
fn hart_mask_insert_remove_contains() {
    let mask = HartMask::new();
    assert!(mask.is_empty());
    assert!(!mask.contains(0));
    assert!(mask.insert(0));
    assert!(!mask.insert(0), "insert lần hai không đổi");
    assert!(mask.contains(0));
    assert_eq!(mask.count(), 1);
    assert!(mask.insert(3));
    assert_eq!(mask.bits(), 0b1001);
    assert_eq!(mask.lowest(), Some(0));
    assert_eq!(mask.highest(), Some(3));
    assert!(mask.remove(0));
    assert!(!mask.remove(0));
    assert_eq!(mask.count(), 1);
    assert!(!mask.is_empty());
    mask.clear();
    assert!(mask.is_empty());
    assert_eq!(mask.lowest(), None);
}

#[test]
fn hart_mask_tu_choi_hart_ngoai_dai() {
    let mask = HartMask::new();
    assert!(!mask.insert(MAX_HARTS));
    assert!(!mask.insert(usize::MAX));
    assert!(!mask.contains(MAX_HARTS));
    assert!(!mask.remove(MAX_HARTS));
    assert!(mask.is_empty());
    assert_eq!(hart_bit(MAX_HARTS), 0);
    assert_eq!(hart_bit(7), 1 << 7);
}

#[test]
fn hart_mask_tu_bits_va_slice() {
    let mask = HartMask::from_bits(0b1010);
    assert_eq!(mask.bits(), 0b1010);
    assert!(mask.contains(1) && mask.contains(3) && !mask.contains(0));
    assert_eq!(mask_of(&[1, 3]), 0b1010);
    assert_eq!(mask_of(&[1, 3, MAX_HARTS]), 0b1010, "bỏ hart ngoài dải");
    assert_eq!(range_mask(1), 0b1);
    assert_eq!(range_mask(2), 0b11);
    assert_eq!(range_mask(4), 0b1111);
    assert_eq!(range_mask(0), 0);
    assert_eq!(mask_count(0b1011), 3);
    assert_eq!(hart_record_offset(1), HART_STRIDE);
}

#[test]
fn phan_loai_trang_thai_hsm() {
    assert_eq!(classify_hart_status(SBI_HART_STARTED), HartStatus::Started);
    assert_eq!(classify_hart_status(SBI_HART_STOPPED), HartStatus::Stopped);
    assert_eq!(classify_hart_status(2), HartStatus::Pending);
    assert_eq!(classify_hart_status(3), HartStatus::Pending);
    assert_eq!(classify_hart_status(4), HartStatus::Suspended);
    assert_eq!(classify_hart_status(5), HartStatus::Suspended);
    assert_eq!(classify_hart_status(6), HartStatus::Suspended);
    assert_eq!(
        classify_hart_status(SBI_ERR_INVALID_PARAM),
        HartStatus::Invalid
    );
    assert_eq!(
        classify_hart_status(SBI_ERR_FAILED),
        HartStatus::Error(SBI_ERR_FAILED)
    );
}

#[test]
fn quyet_dinh_start_tu_choi_hart_ngoai_dtb() {
    // Hart STOPPED ⇒ start; hart không có trong DTB (`-3`) ⇒ từ chối, **không panic**.
    assert_eq!(start_decision(SBI_HART_STOPPED), StartDecision::Start);
    assert_eq!(start_decision(SBI_ERR_INVALID_PARAM), StartDecision::Reject);
    assert_eq!(start_decision(4), StartDecision::Reject);
    assert_eq!(
        start_decision(SBI_HART_STARTED),
        StartDecision::AlreadyStarted
    );
    assert_eq!(start_decision(2), StartDecision::AlreadyStarted);
    assert_eq!(
        start_decision(SBI_ERR_FAILED),
        StartDecision::Error(SBI_ERR_FAILED)
    );
    assert!(hart_start_ok(SBI_SUCCESS));
    assert!(hart_start_ok(SBI_ERR_ALREADY_AVAILABLE));
    assert!(!hart_start_ok(SBI_ERR_INVALID_PARAM));
    assert!(!hart_start_ok(SBI_ERR_FAILED));
    assert!(hart_present(SBI_HART_STOPPED));
    assert!(hart_present(SBI_HART_STARTED));
    assert!(!hart_present(SBI_ERR_INVALID_PARAM));
}

#[test]
fn timer_stats_per_hart_khong_lan() {
    let stats = PerHartTimerStats::new();
    for hart in 0..MAX_HARTS {
        let slot = stats.get(hart);
        assert!(!slot.is_armed());
        assert_eq!(slot.snapshot().ticks, 0);
    }
    // Hart 1 nhận tick, hart 0 không — số của mỗi hart độc lập.
    stats.get(1).arm(1_000, 101_000);
    assert!(stats.get(1).record_tick(101_000).is_some());
    // Tick kế đúng hạn (deadline trước = 101.000 ⇒ deadline kế = 201.000).
    assert!(stats.get(1).record_tick(201_000).is_some());
    assert_eq!(stats.get(1).snapshot().ticks, 2);
    assert_eq!(stats.get(0).snapshot().ticks, 0);
    assert_eq!(stats.get(2).snapshot().ticks, 0);
    assert_eq!(stats.total_ticks(), 2);
    // Deadline kế sau tick thứ hai = 301.000 (mỗi tick cộng 100.000 — tuyệt đối, không trôi).
    assert_eq!(stats.get(1).snapshot().deadline, 301_000);
    // Hart 0 arm riêng — deadline của nó không lấy từ hart 1.
    stats.get(0).arm(50_000, 150_000);
    assert_eq!(stats.get(0).snapshot().deadline, 150_000);
    // Deadline kế sau tick thứ hai = 301.000 (mỗi tick cộng 100.000 — tuyệt đối, không trôi).
    assert_eq!(stats.get(1).snapshot().deadline, 301_000);
}

#[test]
fn plic_stats_per_hart_khong_lan() {
    let stats = PerHartPlicStats::new();
    // Hart 0 nhận 2 claim UART; hart 1 không nhận gì.
    assert_eq!(
        stats.get(0).begin_claim(10, 10),
        vifast_plat_common::plic::ClaimOutcome::Uart
    );
    stats.get(0).note_complete();
    assert_eq!(
        stats.get(0).begin_claim(10, 10),
        vifast_plat_common::plic::ClaimOutcome::Uart
    );
    stats.get(0).note_complete();
    assert_eq!(stats.get(0).snapshot().uart_irqs, 2);
    assert_eq!(stats.get(0).snapshot().claims, 2);
    assert_eq!(stats.get(0).snapshot().completions, 2);
    assert_eq!(stats.get(1).snapshot().claims, 0);
    assert_eq!(stats.get(1).snapshot().traps, 0);
    assert_eq!(stats.total_claims(), 2);
    assert_eq!(stats.total_uart_irqs(), 2);
    // Hart 2 spurious — không cộng vào hart 0.
    stats.get(2).begin_claim(0, 10);
    assert_eq!(stats.get(2).snapshot().spurious, 1);
    assert_eq!(stats.get(0).snapshot().spurious, 0);
    assert_eq!(stats.total_claims(), 2);
}
