// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho khoá COM1 — KT-316.
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-x86_64 --test com
//! ```
//!
//! Đây là **ca kiểm đỏ-trước** (D2-05) của KT-316: mô-đun [`vifast_plat_x86_64::com`] chưa tồn
//! tại khi tệp này được viết, nên lượt chạy đầu **không biên dịch được** (đỏ đúng lý do) — cùng
//! khuôn `tests/boot.rs` của KT-257. Sau khi hiện thực, các khẳng định dưới đây chốt **bất biến
//! loại trừ lẫn nhau** của khoá: bốn luồng cùng tăng một biến trong vùng găng ⇒ kết quả **đúng**
//! số lần (không mất cập nhật), `overlap = 0` (không hai hart cùng trong vùng găng), và
//! `contended ≥ 1` khi **chắc chắn** có một luồng phải chờ.
//!
//! Vì sao test này **không** chạy được trên ảnh: `cli`/`sti`/`pushfq` chỉ có trên bare-metal;
//! trên host `enter_critical` là no-op (xem `com.rs`), nên phần **loại trừ** (CAS) vẫn được đo
//! thật, còn phần **ngắt** được đo bằng QEMU (`run-x86_64.sh` TAP 28).

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use vifast_plat_x86_64::com::ComLock;

/// Bốn luồng × 100 lần: mỗi lần giữ khoá, đọc–sửa–ghi một biến **không** nguyên tử. Nếu khoá
/// không loại trừ được thì kết quả **nhỏ hơn** 400 (mất cập nhật) — đây là phép đo "khoá có
/// tác dụng", không phải "khoá tồn tại".
#[test]
fn lock_serialises_four_threads() {
    let lock = Arc::new(ComLock::new());
    let counter = Arc::new(AtomicU32::new(0));
    let mut handles = Vec::new();
    for _ in 0..4 {
        let lock = Arc::clone(&lock);
        let counter = Arc::clone(&counter);
        handles.push(std::thread::spawn(move || {
            for _ in 0..100 {
                let _guard = lock.lock();
                let value = counter.load(Ordering::Relaxed);
                std::thread::yield_now();
                counter.store(value + 1, Ordering::Relaxed);
            }
        }));
    }
    for handle in handles {
        handle.join().expect("luồng thử không panic");
    }
    assert_eq!(counter.load(Ordering::Relaxed), 400);
    assert_eq!(lock.acquired(), 400);
    assert_eq!(lock.overlap(), 0);
    assert!(lock.ok());
}

/// `ok()` **đòi** ít nhất một lần lấy khoá: khoá chưa từng được dùng thì không được tự khen
/// (`ok = false`) — đây là điều kiện để TAP 28 bắt được đột biến "bỏ khoá" (`no-com-lock`).
#[test]
fn ok_requires_an_acquisition() {
    let lock = ComLock::new();
    assert!(!lock.ok());
    {
        let _guard = lock.lock();
    }
    assert_eq!(lock.acquired(), 1);
    assert_eq!(lock.overlap(), 0);
    assert!(lock.ok());
}

/// `contended` đếm **số lần phải chờ**: luồng chính giữ khoá, luồng phụ báo "sắp lấy" rồi lấy;
/// sau khi luồng phụ chắc chắn đang quay, nhả khoá ⇒ `contended ≥ 1`. Không đo thời gian, chỉ đo
/// **có** một lần chờ (không đòi đúng số lần — số lần phụ thuộc lập lịch host).
#[test]
fn contended_counts_waiters() {
    let lock = Arc::new(ComLock::new());
    let guard = lock.lock();
    let (ready_tx, ready_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let worker_lock = Arc::clone(&lock);
    let worker = std::thread::spawn(move || {
        ready_tx.send(()).expect("báo sẵn sàng");
        let _guard = worker_lock.lock();
        done_tx.send(()).expect("báo xong");
    });
    ready_rx.recv().expect("luồng phụ đã chạy");
    // Luồng phụ đã gửi tín hiệu **ngay trước** `lock()`; cho nó vào vòng quay rồi mới nhả.
    std::thread::sleep(Duration::from_millis(50));
    drop(guard);
    done_rx.recv().expect("luồng phụ đã lấy được khoá");
    worker.join().expect("luồng thử không panic");
    assert!(lock.contended() >= 1);
    assert_eq!(lock.overlap(), 0);
    assert_eq!(lock.acquired(), 2);
}
