// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho PLIC cổ điển (KT-48) — logic thuần ở `vifast-plat-common::plic`.
//!
//! D2-72 Q12: MMIO/CSR (`read_volatile`, `csie.SEIE`, `claim/complete`) không chạy được trên host
//! và không thuộc Miri, nhưng **ngữ nghĩa** PLIC (hình học đăng ký, context `2*hart+1`, phân
//! loại claim, bộ đếm claim/complete, `ndev`, limit tắt sau N) đã tách khỏi asm nên test được:
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common
//! ```
//!
//! Phủ: hằng số spec PLIC §4, context M/S của hart 0..3, word/bit enable, offset
//! threshold/claim, hai cửa sổ 2 MiB phủ `MAX_HARTS = 8`, phân loại claim (UART / spurious /
//! line lạ), bộ đếm claim/complete "cân bằng" + "0 IRQ giả", limit tắt sau N, `ndev` không giả
//! định 95, priority > threshold.

use vifast_plat_common::plic::{
    classify_claim, context_of_hart, context_offset, enable_bit, enable_offset, enable_word_index,
    machine_context_of_hart, ndev_covers_uart, priority_offset, threshold_offset,
    uart_limit_reached, ClaimOutcome, PlicSnapshot, PlicStats, PLIC_CLAIM_OFFSET,
    PLIC_CONTEXT_BASE, PLIC_CONTEXT_STRIDE, PLIC_ENABLE_BASE, PLIC_ENABLE_CTX_STRIDE,
    PLIC_HIGH_WINDOW_BYTES, PLIC_LOW_WINDOW_BYTES, PLIC_MAPPED_CONTEXTS, PLIC_MAPPED_HARTS,
    PLIC_MAX_SOURCES, PLIC_PENDING_BASE, PLIC_PRIORITY_BASE, PLIC_SOURCES_PER_WORD, PLIC_THRESHOLD,
    PLIC_THRESHOLD_OFFSET, PLIC_UART_PRIORITY, UART_IRQ_LIMIT,
};

/// IRQ UART0 của QEMU `virt` (dữ kiện nền tảng, D2-72 Q7) — không phải hằng số `[TK]`.
const UART0_IRQ: u32 = 10;

#[test]
fn hang_so_hinh_ho_plic_dung_spec() {
    // PLIC spec §4: priority 0x0, pending 0x1000, enable 0x2000 (0x80/context), context
    // 0x20_0000 (0x1000/context), threshold +0, claim/complete +4; tối đa 1024 nguồn.
    assert_eq!(PLIC_PRIORITY_BASE, 0x0000);
    assert_eq!(PLIC_PENDING_BASE, 0x1000);
    assert_eq!(PLIC_ENABLE_BASE, 0x2000);
    assert_eq!(PLIC_ENABLE_CTX_STRIDE, 0x80);
    assert_eq!(PLIC_CONTEXT_BASE, 0x20_0000);
    assert_eq!(PLIC_CONTEXT_STRIDE, 0x1000);
    assert_eq!(PLIC_THRESHOLD_OFFSET, 0);
    assert_eq!(PLIC_CLAIM_OFFSET, 4);
    assert_eq!(PLIC_MAX_SOURCES, 1023);
    assert_eq!(PLIC_SOURCES_PER_WORD, 32);
}

#[test]
fn context_s_mode_la_hai_hart_cong_mot() {
    // D2-72 Q7: context S-mode `2*hart+1`; M-mode `2*hart` (không dùng ở K3) — hai context khác
    // nhau nên không thể "claim nhầm context" mà vẫn qua test.
    for hart in 0..4usize {
        let s = context_of_hart(hart);
        let m = machine_context_of_hart(hart);
        assert_eq!(s, hart as u32 * 2 + 1);
        assert_eq!(m, hart as u32 * 2);
        assert_ne!(s, m);
    }
    assert_eq!(
        [
            context_of_hart(0),
            context_of_hart(1),
            context_of_hart(2),
            context_of_hart(3)
        ],
        [1, 3, 5, 7]
    );
}

#[test]
fn enable_word_va_bit_theo_irq() {
    // IRQ 10 nằm trong word 0, bit 10; enable của context 1 ở 0x2000 + 0x80 = 0x2080.
    assert_eq!(enable_word_index(0), 0);
    assert_eq!(enable_word_index(31), 0);
    assert_eq!(enable_word_index(32), 1);
    assert_eq!(enable_word_index(33), 1);
    assert_eq!(enable_bit(0), 1);
    assert_eq!(enable_bit(10), 1 << 10);
    assert_eq!(enable_bit(31), 1 << 31);
    assert_eq!(enable_bit(32), 1);
    assert_eq!(enable_bit(1023), 1 << 31);
    assert_eq!(enable_offset(context_of_hart(0), UART0_IRQ), 0x2080);
    assert_eq!(enable_offset(context_of_hart(1), UART0_IRQ), 0x2180);
    // Word cuối (IRQ 1023, word 31) của context 15 vẫn nằm trong cửa sổ thấp 2 MiB.
    let last = enable_offset(15, 1023);
    assert_eq!(last, 0x2000 + 0x80 * 15 + 31 * 4);
    assert!(last < PLIC_LOW_WINDOW_BYTES);
}

#[test]
fn offset_threshold_claim_theo_context() {
    // context 1: threshold 0x201000, claim 0x201004; context 7: 0x207000; context 15: 0x20f000.
    assert_eq!(context_offset(context_of_hart(0)), 0x201000);
    assert_eq!(threshold_offset(context_of_hart(0)), 0x201000);
    assert_eq!(
        context_offset(context_of_hart(0)) + PLIC_CLAIM_OFFSET,
        0x201004
    );
    assert_eq!(context_offset(context_of_hart(3)), 0x207000);
    assert_eq!(context_offset(PLIC_MAPPED_CONTEXTS - 1), 0x20f000);
    // Cửa sổ cao 2 MiB phủ context cuối (kể cả claim +4).
    let last = context_offset(PLIC_MAPPED_CONTEXTS - 1) + PLIC_CONTEXT_STRIDE;
    assert!(last - PLIC_CONTEXT_BASE <= PLIC_HIGH_WINDOW_BYTES);
}

#[test]
fn priority_offset_cua_uart_va_nguon_cuoi() {
    assert_eq!(priority_offset(0), 0);
    assert_eq!(priority_offset(1), 4);
    assert_eq!(priority_offset(UART0_IRQ), 0x28);
    assert_eq!(priority_offset(PLIC_MAX_SOURCES), 0xffc);
    assert!(priority_offset(PLIC_MAX_SOURCES) < PLIC_LOW_WINDOW_BYTES);
}

#[test]
fn hai_cua_so_phu_max_harts_8() {
    // D2-72 Q4: MAX_HARTS = 8; hai cửa sổ 2 MiB phủ context S-mode của hart cuối (2*7+1 = 15)
    // và enable của IRQ lớn nhất (probe tới 1024). Đây là cổng chặn "claim sai context".
    assert_eq!(PLIC_MAPPED_HARTS, 8);
    assert_eq!(PLIC_MAPPED_CONTEXTS, 16);
    assert_eq!(context_of_hart(PLIC_MAPPED_HARTS - 1), 15);
    assert!(enable_offset(15, PLIC_MAX_SOURCES) < PLIC_LOW_WINDOW_BYTES);
    assert!(
        context_offset(PLIC_MAPPED_CONTEXTS - 1) + PLIC_CONTEXT_STRIDE - PLIC_CONTEXT_BASE
            <= PLIC_HIGH_WINDOW_BYTES
    );
}

#[test]
fn phan_loai_claim_ba_nhanh() {
    assert_eq!(classify_claim(0, UART0_IRQ), ClaimOutcome::Spurious);
    assert_eq!(classify_claim(UART0_IRQ, UART0_IRQ), ClaimOutcome::Uart);
    assert_eq!(classify_claim(11, UART0_IRQ), ClaimOutcome::Unexpected(11));
    assert_eq!(classify_claim(1, UART0_IRQ), ClaimOutcome::Unexpected(1));
    assert_eq!(
        classify_claim(1023, UART0_IRQ),
        ClaimOutcome::Unexpected(1023)
    );
}

#[test]
fn stats_claim_complete_can_bang() {
    let stats = PlicStats::new();
    for _ in 0..UART_IRQ_LIMIT {
        assert_eq!(stats.begin_claim(UART0_IRQ, UART0_IRQ), ClaimOutcome::Uart);
        stats.note_iir(0x2);
        stats.note_complete();
    }
    stats.note_uart_ier_off();
    let snap = stats.snapshot();
    assert_eq!(snap.traps, UART_IRQ_LIMIT);
    assert_eq!(snap.claims, UART_IRQ_LIMIT);
    assert_eq!(snap.completions, UART_IRQ_LIMIT);
    assert_eq!(snap.uart_irqs, UART_IRQ_LIMIT);
    assert!(snap.balanced());
    assert_eq!(snap.fake_irqs(), 0);
    assert_eq!(snap.last_irq, UART0_IRQ as u64);
    assert_eq!(snap.last_iir, 0x2);
    assert_eq!(snap.uart_ier_off, 1);
    stats.reset();
    assert_eq!(stats.snapshot(), PlicSnapshot::default());
}

#[test]
fn stats_dem_irq_gia_va_khong_treo() {
    let stats = PlicStats::new();
    // claim rỗng (spurious) **không** complete (PLIC spec); line lạ thì complete để không treo.
    assert_eq!(stats.begin_claim(0, UART0_IRQ), ClaimOutcome::Spurious);
    assert_eq!(stats.begin_claim(0, UART0_IRQ), ClaimOutcome::Spurious);
    assert_eq!(stats.begin_claim(7, UART0_IRQ), ClaimOutcome::Unexpected(7));
    stats.note_complete();
    let snap = stats.snapshot();
    assert_eq!(snap.traps, 3);
    assert_eq!(snap.claims, 1);
    assert_eq!(snap.completions, 1);
    assert_eq!(snap.spurious, 2);
    assert_eq!(snap.unexpected, 1);
    assert_eq!(snap.fake_irqs(), 3);
    assert_eq!(snap.uart_irqs, 0);
    assert_eq!(snap.last_irq, 7);
    assert!(snap.balanced());
}

#[test]
fn uart_limit_tat_sau_n_ngat() {
    // K3-PLAN §4.1 KT-48: "handler đếm, tắt sau N" — N nhỏ, hữu hạn, không ngập log.
    assert!((1..=64).contains(&UART_IRQ_LIMIT));
    assert!(!uart_limit_reached(0));
    assert!(!uart_limit_reached(UART_IRQ_LIMIT - 1));
    assert!(uart_limit_reached(UART_IRQ_LIMIT));
    assert!(uart_limit_reached(UART_IRQ_LIMIT + 1));
}

#[test]
fn ndev_phai_phu_uart_chu_khong_gia_dinh_95() {
    assert!(ndev_covers_uart(95, UART0_IRQ));
    assert!(ndev_covers_uart(11, UART0_IRQ));
    assert!(ndev_covers_uart(10, UART0_IRQ));
    assert!(!ndev_covers_uart(9, UART0_IRQ));
    assert!(!ndev_covers_uart(0, UART0_IRQ));
    assert!(!ndev_covers_uart(95, 0)); // IRQ 0 không tồn tại trong PLIC
}

#[test]
fn priority_lon_hon_threshold_moi_forward() {
    // PLIC chỉ forward nguồn có priority > threshold; threshold 0 + priority 1 (K3-PLAN §4.1).
    assert_eq!(PLIC_THRESHOLD, 0);
    assert_eq!(PLIC_UART_PRIORITY, 1);
    assert!(PLIC_UART_PRIORITY > PLIC_THRESHOLD);
}
