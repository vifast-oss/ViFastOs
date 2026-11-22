// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho khoá MCS bọc lưu/tắt ngắt — KT-336a (D2-232/D2-238).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-x86_64 --test irq_lock
//! ```
//!
//! Đây là **ca kiểm đỏ-trước** (D2-05): mô-đun [`vifast_plat_x86_64::irq_lock`] chưa tồn tại khi
//! tệp này được viết, nên lượt chạy đầu **không biên dịch được** — đỏ đúng lý do. Sau khi hiện
//! thực, các khẳng định dưới đây chốt bốn bất biến **host-đo-được**:
//!
//! 1. **loại trừ**: bốn luồng cùng đọc–sửa–ghi một biến **không** nguyên tử trong vùng găng ⇒
//!    kết quả đúng **số lần** (không mất cập nhật) và `overlap = 0`;
//! 2. **không reentrant**: `try_lock_irqsave` trong vùng găng trả `None`; guard chỉ nhả **một**
//!    lần (`in_cs` về `0`);
//! 3. **trần chờ hữu hạn**: `lock_irqsave_bounded` trả `None` khi khoá đang bị giữ — deadlock
//!    thành **đỏ có chẩn đoán**, không treo im lặng (đột biến quên `unlock` bị bắt);
//! 4. **đếm chờ**: `contended ≥ 1` khi **chắc chắn** có một luồng phải chờ.
//!
//! Phần **ngắt** (`cli`/`sti`/`pushfq`) chỉ có trên bare-metal: trên host `IrqState` là no-op nên
//! phần loại trừ vẫn được đo thật, còn phần "ngắt bị hoãn" được đo bằng QEMU (`run-x86_64.sh`
//! TAP 34 + dòng `# lock: irq …`).

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use vifast_plat_x86_64::irq_lock::IrqMcsLock;

/// Bốn luồng × 100 lần đọc–sửa–ghi **không** nguyên tử trong vùng găng. Khoá không loại trừ ⇒
/// kết quả **nhỏ hơn** 400 (mất cập nhật), không phải "có một khoá nào đó".
#[test]
fn irq_lock_serialises_four_threads() {
    let lock = Arc::new(IrqMcsLock::new(0u32));
    let counter = Arc::new(AtomicU32::new(0));
    let mut handles = Vec::new();
    for _ in 0..4 {
        let lock = Arc::clone(&lock);
        let counter = Arc::clone(&counter);
        handles.push(std::thread::spawn(move || {
            for _ in 0..100 {
                let _guard = lock.lock_irqsave();
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
    assert_eq!(lock.in_cs(), 0);
    assert!(lock.ok());
}

/// `try_lock_irqsave` trả `None` khi khoá đang bị giữ, và lấy lại được **chỉ sau** khi guard bị
/// nhả. Đây là phép kiểm "khoá thật sự giữ" ở mức API.
#[test]
fn irq_try_lock_none_while_held_va_lay_lai_sau_drop() {
    let lock = IrqMcsLock::new(());
    let guard = lock.try_lock_irqsave();
    assert!(
        guard.is_some(),
        "khoá rảnh ⇒ try_lock_irqsave phải lấy được"
    );
    assert!(
        lock.try_lock_irqsave().is_none(),
        "đang giữ ⇒ try_lock_irqsave trả None"
    );
    assert_eq!(lock.in_cs(), 1, "đúng một người trong vùng găng");
    drop(guard);
    assert_eq!(lock.in_cs(), 0, "guard nhả ⇒ in_cs về 0");
    let again = lock.try_lock_irqsave();
    assert!(again.is_some(), "nhả xong ⇒ lấy lại được");
    drop(again);
    assert_eq!(lock.overlap(), 0);
}

/// `lock_irqsave_bounded` là **trần chờ hữu hạn**: đang bị giữ ⇒ trả `None` sau `spins` vòng,
/// **không** treo. Đây là bất biến bắt đột biến "quên `unlock` ở nhánh lỗi": lần lấy kế tiếp
/// đỏ có chẩn đoán thay vì treo boot.
#[test]
fn irq_bounded_tra_none_khi_dang_bi_giu() {
    let lock = IrqMcsLock::new(());
    let guard = lock.lock_irqsave();
    assert!(
        lock.lock_irqsave_bounded(8).is_none(),
        "bounded không được chờ vô hạn khi khoá bị giữ"
    );
    assert_eq!(
        lock.in_cs(),
        1,
        "người thử thất bại không được vào vùng găng"
    );
    drop(guard);
    let again = lock.lock_irqsave_bounded(8);
    assert!(again.is_some(), "nhả xong ⇒ bounded lấy được");
    drop(again);
    assert_eq!(lock.in_cs(), 0);
}

/// **Không reentrant**: gọi `lock_irqsave` lần hai trong vùng găng **không** được acquire ngay
/// (khoá MCS không reentrant) — dùng `try_lock_irqsave` để kiểm mà không treo.
#[test]
fn irq_lock_khong_reentrant() {
    let lock = IrqMcsLock::new(7u8);
    let guard = lock.lock_irqsave();
    assert_eq!(*guard, 7);
    assert!(
        lock.try_lock_irqsave().is_none(),
        "không reentrant: lần hai phải thất bại"
    );
    assert_eq!(lock.in_cs(), 1);
    drop(guard);
    assert_eq!(lock.in_cs(), 0);
    assert_eq!(lock.overlap(), 0);
}

/// `contended` đếm **số lần phải chờ**: luồng chính giữ khoá, luồng phụ xin; sau khi chắc chắn
/// luồng phụ đang quay mới nhả ⇒ `contended ≥ 1`. Chỉ đo "**có** một lần chờ", không đòi đúng
/// số lần (số lần phụ thuộc lịch host) — cùng khuôn `tests/com.rs` của KT-316.
#[test]
fn irq_contended_dem_so_lan_phai_cho() {
    let lock = Arc::new(IrqMcsLock::new(()));
    let guard = lock.lock_irqsave();
    let (ready_tx, ready_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let worker_lock = Arc::clone(&lock);
    let worker = std::thread::spawn(move || {
        ready_tx.send(()).expect("báo sẵn sàng");
        let _guard = worker_lock.lock_irqsave();
        done_tx.send(()).expect("báo xong");
    });
    ready_rx.recv().expect("luồng phụ đã chạy");
    std::thread::sleep(Duration::from_millis(50));
    drop(guard);
    done_rx.recv().expect("luồng phụ đã lấy được khoá");
    worker.join().expect("luồng thử không panic");
    assert!(lock.contended() >= 1);
    assert_eq!(lock.overlap(), 0);
    assert_eq!(lock.acquired(), 2);
    assert_eq!(lock.in_cs(), 0);
}

/// `IrqState` **đúng cặp**: `save_and_disable` rồi `restore` không panic và trạng thái ngắt
/// trả về **giá trị đã lưu** (trên host là no-op nên chỉ kiểm được cặp gọi + khoá vẫn hoạt
/// động sau đó). Phần giá trị `IF` thật do QEMU TAP 34 đo.
#[test]
fn irq_state_cap_va_tra_ve_dung_cap() {
    let state = vifast_plat_x86_64::irq_lock::IrqState::save_and_disable();
    let lock = IrqMcsLock::new(());
    {
        let guard = lock.lock_irqsave();
        assert_eq!(*guard, ());
        assert_eq!(lock.in_cs(), 1);
    }
    state.restore();
    assert_eq!(lock.in_cs(), 0);
    assert!(lock.ok());
}
