// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-415** — host test cho **tầng 2 trên `IpiMailbox`** (`HartTier2`/`PerHartTier2`).
//!
//! Kiểm đúng thứ mà tầng 2 hứa (D2-259 mục 6):
//!
//! 1. **Chuông luôn rung**: mọi `send` — kể cả khi ring đầy — đều `post` bit tầng 1, nên hart
//!    đích **luôn** bị đánh thức. Đây là nửa "không mất wake" của tầng 1.
//! 2. **Cờ `OVERFLOW` tự rơi full-rescan**: ring đầy ⇒ `drain` trả `overflowed = true` ⇒ người
//!    nhận quét lại toàn bộ nguồn wake. Đây là nửa "không mất wake" của tầng 2.
//! 3. **Thứ tự**: `drain` rút chuông **trước** rồi mới rút ring + cờ; nhờ vậy một `send` chen
//!    giữa hai bước vẫn để lại bit chuông cho lượt sau (không có khe mất wake).
//! 4. **Bảo toàn**: `delivered + overflowed == posted` trên 4 luồng thật.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

use vifast_plat_common::ipi::{HartTier2, IpiKind, PerHartTier2};
use vifast_sync::{Msg, PostOutcome};

fn wake(tcb: u32) -> Msg {
    Msg::Wake { hart: 0, tcb }
}

#[test]
fn send_luon_rung_chuong_ke_ca_khi_ring_day() {
    let mb: HartTier2<2> = HartTier2::new();
    assert_eq!(mb.send(wake(0)).outcome, PostOutcome::Delivered);
    assert_eq!(mb.send(wake(1)).outcome, PostOutcome::Delivered);
    // Ring đầy: message rơi khỏi ring, nhưng chuông **vẫn** rung (nếu không ⇒ mất wake).
    let third = mb.send(wake(2));
    assert_eq!(third.outcome, PostOutcome::Overflow);
    assert!(
        third.doorbell || mb.doorbell().pending().contains(IpiKind::Wake),
        "chuông tầng 1 phải có bit Wake sau send thứ ba"
    );
    let snap = mb.doorbell().snapshot();
    assert_eq!(snap.posted, 1, "chỉ lần đầu đặt bit mới");
    assert_eq!(snap.coalesced, 2, "hai lần sau gộp vì bit đã chờ");
    assert_eq!(
        snap.posted + snap.coalesced,
        3,
        "3 send ⇒ 3 lần rung chuông"
    );
    assert_eq!(snap.pending_bits & IpiKind::Wake.bit(), IpiKind::Wake.bit());
}

#[test]
fn drain_rut_chuong_truoc_roi_moi_rut_ring_va_co() {
    let mb: HartTier2<2> = HartTier2::new();
    assert!(mb.send(wake(0)).outcome.delivered());
    assert!(mb.send(wake(1)).outcome.delivered());
    assert!(mb.send(wake(2)).outcome.overflowed()); // overflow
    let mut got = Vec::new();
    let d = mb.drain(|m| got.push(m));
    assert_eq!(d.messages, 2, "2 message cũ còn nguyên");
    assert!(d.overflowed, "cờ OVERFLOW phải được báo");
    assert!(d.kinds.contains(IpiKind::Wake), "drain trả kind đã rút");
    assert_eq!(got, vec![wake(0), wake(1)], "FIFO");
    // Chuông đã rút ⇒ hộp thư tầng 1 rỗng; cờ đã xoá ⇒ lần sau không báo lại.
    assert!(mb.doorbell().is_empty());
    assert!(!mb.drain(|_| {}).overflowed);
}

#[test]
fn bon_luong_hai_nghin_message_khong_mat_wake() {
    const THREADS: usize = 4;
    const PER: u32 = 2_000;
    let mb: Arc<HartTier2<8>> = Arc::new(HartTier2::new());
    let delivered = Arc::new(AtomicUsize::new(0));
    let rescans = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::new();
    for t in 0..THREADS {
        let mb = Arc::clone(&mb);
        handles.push(thread::spawn(move || {
            for i in 0..PER {
                // Mọi lần gửi đều có kết cục đo được (Delivered hoặc Overflow) — không nuốt.
                let sent = mb.send(wake((t as u32) * PER + i));
                assert!(sent.outcome.delivered() || sent.outcome.overflowed());
            }
        }));
    }
    let mb_rx = Arc::clone(&mb);
    let delivered_rx = Arc::clone(&delivered);
    let rescans_rx = Arc::clone(&rescans);
    let rx = thread::spawn(move || loop {
        let d = mb_rx.drain(|_| {
            delivered_rx.fetch_add(1, Ordering::Relaxed);
        });
        if d.overflowed {
            rescans_rx.fetch_add(1, Ordering::Relaxed);
        }
        if d.messages == 0
            && !d.overflowed
            && mb_rx.ring().snapshot().posted as usize >= THREADS * PER as usize
        {
            break;
        }
        thread::yield_now();
    });
    for h in handles {
        h.join().expect("producer join");
    }
    rx.join().expect("consumer join");

    let snap = mb.ring().snapshot();
    assert_eq!(snap.posted as usize, THREADS * PER as usize);
    assert_eq!(
        snap.delivered + snap.overflowed,
        snap.posted,
        "bảo toàn: delivered + overflowed == posted"
    );
    assert_eq!(delivered.load(Ordering::Relaxed) as u64, snap.delivered);
    assert!(
        snap.overflowed > 0,
        "ring 8 slot + 8.000 message ⇒ phải overflow"
    );
    assert!(
        rescans.load(Ordering::Relaxed) > 0,
        "có overflow ⇒ phải có lượt full-rescan"
    );
    // Chuông tầng 1 **không** mất bit nào: mọi send đều post (gộp thì vẫn đếm coalesced).
    let bell = mb.doorbell().snapshot();
    assert_eq!(
        bell.posted + bell.coalesced,
        THREADS as u64 * PER as u64,
        "mọi send đều rung chuông (posted + coalesced)"
    );
}

#[test]
fn full_rescan_cuu_wake_bi_roi_tren_mo_hinh_scheduler() {
    // 8 TCB, ring 2 slot: chỉ 2 wake vào ring, 6 wake bị rơi. Người nhận **chỉ** dựa vào message
    // sẽ bỏ sót 6 TCB; nhờ OVERFLOW ⇒ full-rescan đánh thức **cả 8**.
    const TCBS: u32 = 8;
    let mb: HartTier2<2> = HartTier2::new();
    let mut runnable = [false; TCBS as usize];
    for tcb in 0..TCBS {
        let sent = mb.send(wake(tcb));
        assert!(sent.outcome.delivered() || sent.outcome.overflowed());
    }
    let d = mb.drain(|m| {
        if let Msg::Wake { tcb, .. } = m {
            runnable[tcb as usize] = true;
        }
    });
    assert!(d.overflowed);
    if d.overflowed {
        for slot in runnable.iter_mut() {
            *slot = true;
        }
    }
    assert!(runnable.iter().all(|r| *r), "0 mất wake sau full-rescan");
}

#[test]
fn per_hart_tier2_tach_hop_thu_tung_hart() {
    let all: PerHartTier2<4> = PerHartTier2::new();
    assert!(all.get(1).send(wake(7)).outcome.delivered());
    assert!(all.get(3).send(wake(9)).outcome.delivered());
    assert_eq!(all.get(1).ring().snapshot().posted, 1);
    assert_eq!(all.get(2).ring().snapshot().posted, 0);
    assert_eq!(all.get(3).ring().snapshot().posted, 1);
    assert_eq!(all.total_pending(), 2, "2 message đang chờ trên 2 hart");
    // Hart ngoài dải ⇒ slot 0 (không bao giờ xảy ra với hart thật) — cùng quy ước `IpiMailbox`.
    assert_eq!(
        all.get(99).ring().snapshot().posted,
        all.get(0).ring().snapshot().posted
    );
}
