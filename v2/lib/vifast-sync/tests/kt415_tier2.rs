// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-415** — hộp thư **tầng 2** (`Tier2Mailbox<N>`) trên `Mailbox<N>`: cờ `OVERFLOW` tự rơi
//! **full-rescan** ⇒ **0 mất wake** (D2-259 mục 6).
//!
//! # Vì sao có tầng 2
//!
//! `Mailbox<N>` (tầng payload) đầy ⇒ `Busy`; `IpiMailbox` (tầng chuông) chỉ nói "có việc" và
//! **không bao giờ đầy**. Nếu caller bỏ message khi `Busy` thì **mất wake** — đúng lỗi mà tầng 2
//! phải chặn bằng **toán học**, không bằng "nhớ kiểm".
//!
//! Bất biến của tầng 2 (kiểm ở đây):
//!
//! 1. **Bảo toàn**: `delivered + overflowed == posted` — mọi lần `post` đều có kết cục đo được.
//! 2. **Không mất wake**: `overflowed > 0 ⇒ full_rescans > 0` — có message bị rơi thì **bắt buộc**
//!    có ít nhất một lần người nhận chạy full-rescan (quét lại toàn bộ nguồn wake).
//! 3. **Cờ tự xoá**: `take()` trả `overflowed` **một lần** rồi xoá; lần sau `false`.
//! 4. **Không ghi đè**: message đã vào ring không bị message sau đè (FIFO của `Mailbox`).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

use vifast_sync::{Msg, PostOutcome, Tier2Mailbox};

fn wake(tcb: u32) -> Msg {
    Msg::Wake { hart: 0, tcb }
}

#[test]
fn day_ring_bat_overflow_va_khong_mat_message_cu() {
    let mb: Tier2Mailbox<4> = Tier2Mailbox::new();
    for tcb in 0..4 {
        assert_eq!(mb.post(wake(tcb)), PostOutcome::Delivered);
    }
    // Ring đầy: message thứ 5 **không** vào ring, nhưng **không** mất wake — cờ OVERFLOW bật.
    assert_eq!(mb.post(wake(4)), PostOutcome::Overflow);
    assert!(mb.overflow(), "OVERFLOW phải bật khi ring đầy");

    let mut got = Vec::new();
    let drain = mb.take(|m| got.push(m));
    assert_eq!(drain.messages, 4, "4 message cũ còn nguyên (không ghi đè)");
    assert!(drain.overflowed, "take() phải trả cờ OVERFLOW");
    assert_eq!(got, (0..4).map(wake).collect::<Vec<_>>(), "FIFO");
    // Cờ đã được xoá bởi take() ⇒ lần sau không báo lại.
    assert!(!mb.overflow());
    assert!(!mb.take(|_| {}).overflowed);
    // Ring đã trống ⇒ gửi lại được.
    assert_eq!(mb.post(wake(9)), PostOutcome::Delivered);
}

#[test]
fn bat_bien_bao_toan_delivered_cong_overflowed_bang_posted() {
    let mb: Tier2Mailbox<2> = Tier2Mailbox::new();
    let mut posted = 0u64;
    for tcb in 0..10 {
        let outcome = mb.post(wake(tcb));
        assert!(outcome.delivered() || outcome.overflowed());
        posted += 1;
        if tcb % 3 == 2 {
            mb.take(|_| {});
        }
    }
    let snap = mb.snapshot();
    assert_eq!(snap.posted, posted);
    assert_eq!(
        snap.delivered + snap.overflowed,
        snap.posted,
        "mọi lần post phải có kết cục: delivered hoặc overflowed"
    );
    assert!(
        snap.overflowed > 0,
        "ring 2 slot + 10 post ⇒ phải có overflow"
    );
    assert!(
        snap.full_rescans > 0,
        "có overflow ⇒ người nhận phải full-rescan ít nhất một lần"
    );
}

#[test]
fn bon_luong_hai_nghin_message_khong_mat_wake() {
    const THREADS: usize = 4;
    const PER: u32 = 2_000;
    let mb: Arc<Tier2Mailbox<8>> = Arc::new(Tier2Mailbox::new());
    let delivered = Arc::new(AtomicUsize::new(0));
    let rescan_rounds = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::new();
    for t in 0..THREADS {
        let mb = Arc::clone(&mb);
        handles.push(thread::spawn(move || {
            for i in 0..PER {
                let outcome = mb.post(wake((t as u32) * PER + i));
                assert!(outcome.delivered() || outcome.overflowed());
            }
        }));
    }
    // Người nhận: rút tới khi mọi producer xong **và** ring rỗng. Mỗi lần `take` thấy
    // `overflowed` ⇒ một lượt full-rescan (mô hình hoá "quét lại toàn bộ nguồn wake").
    let mb_rx = Arc::clone(&mb);
    let delivered_rx = Arc::clone(&delivered);
    let rescan_rx = Arc::clone(&rescan_rounds);
    let rx = thread::spawn(move || loop {
        let d = mb_rx.take(|_| {
            delivered_rx.fetch_add(1, Ordering::Relaxed);
        });
        if d.overflowed {
            rescan_rx.fetch_add(1, Ordering::Relaxed);
        }
        if d.messages == 0
            && !d.overflowed
            && mb_rx.snapshot().posted as usize >= THREADS * PER as usize
        {
            break;
        }
        thread::yield_now();
    });
    for h in handles {
        h.join().expect("producer join");
    }
    rx.join().expect("consumer join");

    let snap = mb.snapshot();
    assert_eq!(snap.posted as usize, THREADS * PER as usize);
    assert_eq!(
        snap.delivered + snap.overflowed,
        snap.posted,
        "bảo toàn: delivered + overflowed == posted"
    );
    assert_eq!(
        delivered.load(Ordering::Relaxed) as u64,
        snap.delivered,
        "số message người nhận thực nhận khớp bộ đếm"
    );
    if snap.overflowed > 0 {
        assert!(
            rescan_rounds.load(Ordering::Relaxed) > 0,
            "có overflow ⇒ phải có lượt full-rescan"
        );
    }
}

#[test]
fn full_rescan_cuu_wake_bi_roi() {
    // Mô hình scheduler 8 TCB: producer gửi Wake cho từng TCB; ring 2 slot nên phần lớn bị rơi.
    // Người nhận **chỉ** dựa vào message sẽ bỏ sót; nhờ OVERFLOW ⇒ full-rescan đánh thức **cả 8**.
    const TCBS: u32 = 8;
    let mb: Tier2Mailbox<2> = Tier2Mailbox::new();
    let mut runnable = [false; TCBS as usize];
    for tcb in 0..TCBS {
        let outcome = mb.post(wake(tcb));
        assert!(outcome.delivered() || outcome.overflowed());
    }
    let d = mb.take(|m| {
        if let Msg::Wake { tcb, .. } = m {
            runnable[tcb as usize] = true;
        }
    });
    assert!(d.overflowed, "ring 2 slot + 8 wake ⇒ phải overflow");
    if d.overflowed {
        // Full-rescan: quét lại **toàn bộ** nguồn wake (đây là ngữ nghĩa tầng 2 bảo đảm).
        for slot in runnable.iter_mut() {
            *slot = true;
        }
    }
    assert!(
        runnable.iter().all(|r| *r),
        "0 mất wake: sau full-rescan mọi TCB đều runnable"
    );
}

#[test]
fn take_rong_khong_bat_overflow() {
    let mb: Tier2Mailbox<2> = Tier2Mailbox::new();
    let d = mb.take(|_| {});
    assert_eq!(d.messages, 0);
    assert!(!d.overflowed);
    assert_eq!(mb.snapshot().full_rescans, 0);
}
