// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-30 — **stress host** đa luồng (K1C-PLAN §5 KT-30 DoD 5; D2-53 Q9 phương án (a): stress có
//! biên + Miri, **không** loom, **không** explorer tự viết).
//!
//! Chỉ assert **bất biến**, **không** assert trace/thứ tự: thứ tự luồng không tất định.
//!
//! Không chạy dưới Miri (`#![cfg(not(miri))]`): Miri tuần tự hoá luồng và mô phỏng từng lệnh
//! nguyên tử, 4 luồng × 500 op quay vòng vượt xa ngân sách; phần cần Miri (an toàn bộ nhớ của
//! lock/mailbox) nằm ở `kt30_sync.rs` (chỉ một luồng). Lý do ghi theo KT-30 DoD 6.
#![cfg(not(miri))]

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use vifast_sync::{Mailbox, McsLock, Msg};

const THREADS: usize = 4;
const OPS: usize = 500;
/// Biên `[ƯL]` của stress (KT-30 DoD 5). Vượt ⇒ `T-INFRA` + log, **không** retry (D2-07).
const BOUND: Duration = Duration::from_secs(3);

/// Chờ tới khi `cond()` đúng; quá `bound` thì panic rõ ràng (không treo harness).
fn wait_until<F: Fn() -> bool>(bound: Duration, cond: F, what: &str) {
    let t0 = Instant::now();
    while !cond() {
        assert!(
            t0.elapsed() < bound,
            "quá biên {bound:?} khi chờ: {what} (đo {:?})",
            t0.elapsed()
        );
        std::thread::yield_now();
    }
}

/// 4 luồng × 500 op `lock/unlock` + `send`: **mutual exclusion** (bộ đếm tăng đúng) và
/// **không mất message** (tổng `drain` = tổng `send` trả `Ok`).
#[test]
fn stress_4_luong_500_op_mutual_exclusion_va_khong_mat_message() {
    let t0 = Instant::now();
    let lock = Arc::new(McsLock::new(0u64));
    // Sức chứa nhỏ (16) + nhường CPU ⇒ vừa chạy đường đồng thời, vừa chạm backpressure `Busy`.
    let mbox: Arc<Mailbox<16>> = Arc::new(Mailbox::new());
    let done = Arc::new(AtomicBool::new(false));
    let sent_ok: Arc<[AtomicUsize; THREADS]> =
        Arc::new(std::array::from_fn(|_| AtomicUsize::new(0)));
    let busy: Arc<[AtomicUsize; THREADS]> = Arc::new(std::array::from_fn(|_| AtomicUsize::new(0)));
    let got: Arc<[AtomicUsize; THREADS]> = Arc::new(std::array::from_fn(|_| AtomicUsize::new(0)));

    // Một người nhận duy nhất (MPSC): rút FIFO tới khi người gửi xong và hộp thư rỗng.
    let consumer = {
        let mbox = Arc::clone(&mbox);
        let done = Arc::clone(&done);
        let got = Arc::clone(&got);
        std::thread::spawn(move || {
            let mut total = 0usize;
            loop {
                let mut n = 0usize;
                while let Some(msg) = mbox.try_recv() {
                    if let Msg::Wake { hart, tcb: _ } = msg {
                        got[hart as usize].fetch_add(1, Ordering::Relaxed);
                    }
                    n += 1;
                }
                total += n;
                if n == 0 && done.load(Ordering::Acquire) && mbox.try_recv().is_none() {
                    break;
                }
                if n == 0 {
                    std::thread::yield_now();
                }
            }
            total
        })
    };

    let mut producers = Vec::new();
    for t in 0..THREADS {
        let lock = Arc::clone(&lock);
        let mbox = Arc::clone(&mbox);
        let sent_ok = Arc::clone(&sent_ok);
        let busy = Arc::clone(&busy);
        producers.push(std::thread::spawn(move || {
            for i in 0..OPS {
                {
                    let mut guard = lock.lock();
                    *guard += 1;
                }
                if mbox
                    .send(Msg::Wake {
                        hart: t as u16,
                        tcb: i as u32,
                    })
                    .is_ok()
                {
                    sent_ok[t].fetch_add(1, Ordering::Relaxed);
                } else {
                    busy[t].fetch_add(1, Ordering::Relaxed);
                }
                // Nhường CPU để người nhận thật sự chạy **đồng thời** với người gửi.
                if i % 4 == 0 {
                    std::thread::yield_now();
                }
            }
        }));
    }
    for p in producers {
        p.join().unwrap();
    }
    done.store(true, Ordering::Release);
    let drained = consumer.join().unwrap();

    // (1) Mutual exclusion: 2.000 lần tăng không mất cập nhật nào.
    let counter = *lock.lock();
    assert_eq!(
        counter,
        (THREADS * OPS) as u64,
        "mutual exclusion: bộ đếm phải tăng đúng {} lần",
        THREADS * OPS
    );
    assert_eq!(lock.queue_len(), 0, "hàng phải rỗng sau khi mọi luồng xong");
    assert_eq!(lock.next_owner(), None);

    // (2) Không mất message: tổng drain = tổng send Ok (không assert thứ tự).
    let total_ok: usize = (0..THREADS)
        .map(|t| sent_ok[t].load(Ordering::Relaxed))
        .sum();
    assert_eq!(drained, total_ok, "không mất message: drain == send Ok");
    let total_busy: usize = (0..THREADS).map(|t| busy[t].load(Ordering::Relaxed)).sum();
    assert_eq!(
        total_ok + total_busy,
        THREADS * OPS,
        "mỗi lần send hoặc Ok hoặc Busy"
    );
    assert!(
        total_ok >= 100,
        "stress phải chạy được đường đồng thời (send Ok {total_ok} quá nhỏ)"
    );
    for t in 0..THREADS {
        assert_eq!(
            got[t].load(Ordering::Relaxed),
            sent_ok[t].load(Ordering::Relaxed),
            "hart {t}: số message nhận được == số gửi thành công"
        );
    }
    assert_eq!(mbox.len(), 0);
    assert_eq!(mbox.drain(|_| {}), 0);

    // (3) Ack theo bộ đếm thế hệ: chưa ack ⇒ pending = generation; ack xong ⇒ 0.
    let generation = mbox.generation();
    assert_eq!(generation as usize, total_ok);
    assert_eq!(mbox.pending(generation), generation);
    assert_eq!(mbox.ack(generation), generation);
    assert_eq!(mbox.pending(generation), 0);
    assert_eq!(mbox.acked(), generation);

    let elapsed = t0.elapsed();
    println!(
        "KT-30 stress: {THREADS} luồng × {OPS} op; send Ok {total_ok}, drain {drained}, {elapsed:?}"
    );
    assert!(
        elapsed <= BOUND,
        "biên [ƯL] {BOUND:?} cho stress, đo {elapsed:?}: vượt ⇒ T-INFRA (D2-07), không retry/nới ngưỡng"
    );
}

/// FIFO **có điều khiển**: luồng chính giữ khoá, B xếp hàng trước, C xếp hàng sau (chờ quan sát
/// `queue_len()`); mở khoá ⇒ B phải được trao trước C.
#[test]
fn stress_fifo_khi_xep_hang_co_dieu_khien() {
    let t0 = Instant::now();
    let lock = Arc::new(McsLock::new(()));
    let order = Arc::new(Mutex::new(Vec::<u8>::new()));

    let spawn = |id: u8| {
        let lock = Arc::clone(&lock);
        let order = Arc::clone(&order);
        std::thread::spawn(move || {
            let guard = lock.lock();
            order.lock().unwrap().push(id);
            drop(guard);
        })
    };

    let guard = lock.lock(); // ticket 0 — "luồng chính"
    assert_eq!(lock.next_owner(), Some(0));

    let b = spawn(1);
    wait_until(BOUND, || lock.queue_len() == 2, "B (ticket 1) xếp hàng");
    let c = spawn(2);
    wait_until(BOUND, || lock.queue_len() == 3, "C (ticket 2) xếp hàng");
    assert_eq!(
        lock.next_owner(),
        Some(0),
        "chủ hiện tại vẫn là luồng chính"
    );
    assert_eq!(lock.queue_len(), 3);

    drop(guard);
    b.join().unwrap();
    c.join().unwrap();

    assert_eq!(
        *order.lock().unwrap(),
        vec![1u8, 2u8],
        "FIFO: B xếp hàng trước phải được trao khoá trước C"
    );
    assert_eq!(lock.queue_len(), 0);
    assert_eq!(lock.next_owner(), None);

    let elapsed = t0.elapsed();
    println!("KT-30 FIFO có điều khiển: {elapsed:?}");
    assert!(
        elapsed <= BOUND,
        "biên [ƯL] {BOUND:?} cho test FIFO, đo {elapsed:?}: vượt ⇒ T-INFRA (D2-07)"
    );
}

/// `try_lock` cũng phải giữ mutual exclusion: không bao giờ có hai chủ cùng lúc.
#[test]
fn stress_try_lock_khong_cho_hai_chu_cung_luc() {
    let t0 = Instant::now();
    let lock = Arc::new(McsLock::new(0u32));
    let inside = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::new();
    for _ in 0..THREADS {
        let lock = Arc::clone(&lock);
        let inside = Arc::clone(&inside);
        handles.push(std::thread::spawn(move || {
            for i in 0..OPS {
                if let Some(mut guard) = lock.try_lock() {
                    let now = inside.fetch_add(1, Ordering::SeqCst) + 1;
                    assert_eq!(now, 1, "mutual exclusion: hai chủ cùng lúc");
                    *guard += 1;
                    std::hint::spin_loop();
                    inside.fetch_sub(1, Ordering::SeqCst);
                }
                if i % 8 == 0 {
                    std::thread::yield_now();
                }
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    // Bất biến chỉ đúng **sau khi join hết** (thread khác có thể đang giữ khoá lúc một thread
    // vừa xong — assertion theo từng join là sai, `reviewer-kt30` mục 1).
    assert_eq!(
        inside.load(Ordering::SeqCst),
        0,
        "không còn ai trong miền tới hạn sau khi join hết"
    );

    let total = *lock.lock();
    assert!(total > 0, "try_lock phải lấy được ít nhất một lần");
    assert_eq!(lock.queue_len(), 0);
    assert_eq!(lock.next_owner(), None);

    let elapsed = t0.elapsed();
    println!("KT-30 try_lock tranh chấp: {total} lần acquire, {elapsed:?}");
    assert!(
        elapsed <= BOUND,
        "biên [ƯL] {BOUND:?} cho try_lock, đo {elapsed:?}: vượt ⇒ T-INFRA (D2-07)"
    );
}
