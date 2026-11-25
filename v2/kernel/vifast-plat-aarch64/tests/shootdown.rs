// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho `TLBI` shootdown đa hart — KT-262 (K4' bước 3, B3.6).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64
//! ```
//!
//! Đây là ca kiểm **đỏ trước** của KT-262 (D2-05; log `v2/agent/JOURNAL/kt262-red.log`): tệp
//! này được viết **trước** `src/shootdown.rs`, nên lần chạy đầu nó **không biên dịch được**
//! (`unresolved import vifast_plat_aarch64::shootdown`).
//!
//! Kỳ vọng **viết tay từ đặc tả**, không lấy hằng của crate để so:
//!
//! - **DDI 0487** — thứ tự `TLBI` bắt buộc: `dsb ishst` (chờ store bảng trang hoàn tất) →
//!   `tlbi vae1is` → `dsb ish` (chờ `TLBI` hoàn tất) → `isb` (context sync). Handler của hart
//!   phụ **không** có `dsb ishst`: nó không phải bên ghi bảng trang.
//! - **IHI 0069** — SGI `0..15`; KT-262 dùng **ba** SGI khác nhau (IPI 0, shootdown 1, lệnh 2)
//!   để dòng `# smp: ipi recv=` của KT-259 không bị đếm lẫn.
//! - **ARM DEN 0022** — không liên quan ở đây (PSCI đã kiểm ở `tests/smp.rs`).
//!
//! # Vì sao test này **không** gọi `apply_local`/`invalidate`
//!
//! Hai hàm ấy chạm asm đặc quyền (`tlbi`, `dsb`) và MMIO GIC; trên host `asm` là **stub panic**
//! (D2-17). Phần kiểm được trên host là phần **quyết định**: thứ tự bước, tập hart đích, bộ đếm
//! ack + trần chờ, và điều kiện `ok()` của kết quả. Phần chạy thật do QEMU đo (TAP 13/14).
// Khẳng định dưới đây **cố ý** so sánh hằng số (số SGI, trần chờ ack) — chốt giá trị
// tuyệt đối; một hằng đổi giá trị thì test phải đỏ (cùng khuôn `tests/boot.rs`).
#![allow(clippy::assertions_on_constants)]

use vifast_plat_aarch64::gic::{SGI_COMMAND, SGI_IPI, SGI_SHOOTDOWN};
use vifast_plat_aarch64::shootdown::{
    collect_acks, AckCounter, Shootdown, Step, Targets, ACK_WAIT_LIMIT, LOCAL_SEQUENCE,
    REMOTE_SEQUENCE,
};
use vifast_plat_aarch64::smp::MAX_HARTS;

// ---------------------------------------------------------------- thứ tự bước

/// Chuỗi **cục bộ** phải đúng bốn bước và đúng thứ tự của DDI 0487:
/// `dsb ishst` → `tlbi vae1is` → `dsb ish` → `isb`.
///
/// Đột biến "bỏ `dsb ish`" hay "đảo `tlbi` lên trước `dsb ishst`" làm test này **ĐỎ** — đó là
/// tầng duy nhất quan sát được đột biến ấy (QEMU TCG không mô hình hoá barrier; ghi ở journal).
#[test]
fn local_sequence_is_the_architectural_order() {
    assert_eq!(LOCAL_SEQUENCE.len(), 4);
    assert_eq!(LOCAL_SEQUENCE[0], Step::DsbIshst);
    assert_eq!(LOCAL_SEQUENCE[1], Step::TlbiVae1is);
    assert_eq!(LOCAL_SEQUENCE[2], Step::DsbIsh);
    assert_eq!(LOCAL_SEQUENCE[3], Step::Isb);
}

/// Handler SGI của hart phụ **không** có `dsb ishst`: nó không ghi bảng trang, chỉ xoá TLB của
/// chính nó. Ba bước còn lại giữ nguyên thứ tự.
#[test]
fn remote_sequence_has_no_store_barrier() {
    assert_eq!(REMOTE_SEQUENCE.len(), 3);
    assert_eq!(REMOTE_SEQUENCE[0], Step::TlbiVae1is);
    assert_eq!(REMOTE_SEQUENCE[1], Step::DsbIsh);
    assert_eq!(REMOTE_SEQUENCE[2], Step::Isb);
    assert!(!REMOTE_SEQUENCE.contains(&Step::DsbIshst));
}

/// Tên bước là **dữ liệu** in vào log — đổi tên là đổi thứ người đọc thấy, nên chốt nguyên văn.
#[test]
fn step_names_are_stable() {
    assert_eq!(Step::DsbIshst.name(), "dsb ishst");
    assert_eq!(Step::TlbiVae1is.name(), "tlbi vae1is");
    assert_eq!(Step::DsbIsh.name(), "dsb ish");
    assert_eq!(Step::Isb.name(), "isb");
}

/// Ba SGI phải **khác nhau**: dùng chung một số thì `note_ipi` của KT-259 đếm cả shootdown/lệnh
/// ⇒ dòng `# smp: ipi recv=1` xuất hiện nhiều lần và runner ĐỎ (đúng ca âm (g)).
#[test]
fn sgi_numbers_are_distinct_and_in_sgi_range() {
    assert_eq!(SGI_IPI, 0);
    assert_eq!(SGI_SHOOTDOWN, 1);
    assert_eq!(SGI_COMMAND, 2);
    assert!(SGI_SHOOTDOWN < 16 && SGI_COMMAND < 16);
    assert_ne!(SGI_IPI, SGI_SHOOTDOWN);
    assert_ne!(SGI_IPI, SGI_COMMAND);
    assert_ne!(SGI_SHOOTDOWN, SGI_COMMAND);
}

// ---------------------------------------------------------------- tập hart đích

/// `Targets::NONE` không gửi SGI nào và chờ **0** ack (đường của `unmap` khi chưa có hart phụ).
#[test]
fn targets_none_is_empty() {
    let none = Targets::NONE;
    assert!(none.is_empty());
    assert_eq!(none.count(), 0);
    assert_eq!(none.mask(), 0);
    assert_eq!(none.harts().count(), 0);
}

/// `all_but(0, 0b1111)` = ba hart phụ — **không** gửi SGI cho chính hart gọi (nó đã tự `TLBI`).
#[test]
fn targets_all_but_excludes_caller() {
    let targets = Targets::all_but(0, 0b1111);
    assert_eq!(targets.mask(), 0b1110);
    assert_eq!(targets.count(), 3);
    assert!(!targets.contains(0));
    assert!(targets.contains(1));
    assert!(targets.contains(2));
    assert!(targets.contains(3));
}

/// Hart gọi không nằm trong `online` ⇒ tập đích **không đổi** (không bịa hart).
#[test]
fn targets_all_but_ignores_offline_caller() {
    let targets = Targets::all_but(5, 0b1111);
    assert_eq!(targets.mask(), 0b1111);
    assert_eq!(targets.count(), 4);
}

/// Hart id ngoài `MAX_HARTS` không tạo bit nào (không tràn `u16`).
#[test]
fn targets_all_but_rejects_out_of_range_hart() {
    let targets = Targets::all_but(MAX_HARTS, 0b1111);
    assert_eq!(targets.mask(), 0b1111);
    assert_eq!(Targets::all_but(usize::MAX, 0b1111).mask(), 0b1111);
}

/// Mặt nạ đầu vào bị cắt đúng `MAX_HARTS` bit — bit rác phía trên không thành hart.
#[test]
fn targets_mask_is_clamped_to_max_harts() {
    let targets = Targets::from_mask(0xFFFF);
    assert_eq!(targets.mask(), (1u16 << MAX_HARTS) - 1);
    assert_eq!(targets.count(), MAX_HARTS as u32);
    assert!(!targets.contains(MAX_HARTS));
}

/// `harts()` duyệt **tăng dần** và bỏ hart không có trong mặt nạ.
#[test]
fn targets_harts_iterates_ascending() {
    let targets = Targets::from_mask(0b1010);
    let harts: [usize; 2] = [1, 3];
    assert_eq!(
        targets.harts().collect::<std::vec::Vec<_>>(),
        harts.to_vec()
    );
}

// ---------------------------------------------------------------- bộ đếm ack

/// Bộ đếm ack bắt đầu từ 0 và mỗi `ack()` cộng đúng 1 (giá trị trả là số **sau** khi cộng).
#[test]
fn ack_counter_counts_each_ack() {
    let acks = AckCounter::new();
    assert_eq!(acks.count(), 0);
    assert_eq!(acks.ack(), 1);
    assert_eq!(acks.ack(), 2);
    assert_eq!(acks.count(), 2);
    acks.reset();
    assert_eq!(acks.count(), 0);
}

/// Chờ **0** ack trả về ngay (`Some(0)`) — đường `Targets::NONE` không được chờ vô ích.
#[test]
fn ack_counter_wait_zero_returns_immediately() {
    let acks = AckCounter::new();
    assert_eq!(acks.wait(0, 1), Some(0));
}

/// Chờ ack không bao giờ tới phải **hết trần rồi trả `None`** — không treo im lặng.
#[test]
fn ack_counter_wait_times_out() {
    let acks = AckCounter::new();
    assert_eq!(acks.wait(1, 16), None);
    assert_eq!(acks.count(), 0);
}

/// Ack đã có sẵn ⇒ `wait` trả `Some` ngay vòng đầu.
#[test]
fn ack_counter_wait_sees_existing_acks() {
    let acks = AckCounter::new();
    acks.ack();
    acks.ack();
    assert_eq!(acks.wait(2, 16), Some(0));
}

// ---------------------------------------------------------------- gửi + chờ (phần thuần)

/// Mọi hart đích ack ⇒ `acks == expected`, không hart nào lỗi gửi, không timeout.
#[test]
fn collect_acks_all_targets_ack() {
    let acks = AckCounter::new();
    let targets = Targets::from_mask(0b1110);
    let (got, failed, waited) = collect_acks(targets, &acks, |_hart| {
        acks.ack();
        true
    });
    assert_eq!(got, 3);
    assert_eq!(failed, 0);
    assert!(waited.is_some());
}

/// Gửi SGI cho hart 2 **thất bại** ⇒ bit 2 nằm trong `send_failed` (không im lặng bỏ qua).
#[test]
fn collect_acks_records_send_failure() {
    let acks = AckCounter::new();
    let targets = Targets::from_mask(0b1110);
    let (got, failed, _waited) = collect_acks(targets, &acks, |hart| {
        if hart == 2 {
            return false;
        }
        acks.ack();
        true
    });
    assert_eq!(failed, 1 << 2);
    assert_eq!(got, 2);
}

/// Hart phụ **không** ack (đột biến "không chờ ack"/"handler không ack") ⇒ timeout, `acks` thiếu.
#[test]
fn collect_acks_times_out_when_remote_does_not_ack() {
    let acks = AckCounter::new();
    let targets = Targets::from_mask(0b1110);
    let (got, failed, waited) = collect_acks(targets, &acks, |_hart| true);
    assert_eq!(got, 0);
    assert_eq!(failed, 0);
    assert_eq!(waited, None);
}

// ---------------------------------------------------------------- điều kiện `ok()`

/// `ok()` chỉ đúng khi **đủ** ack, không timeout, không lỗi gửi.
#[test]
fn shootdown_ok_requires_every_ack() {
    let good = Shootdown {
        va: 0x6000_0000,
        len: 0,
        targets: 0b1110,
        expected: 3,
        acks: 3,
        waited: 12,
        timed_out: false,
        send_failed: 0,
    };
    assert!(good.ok());

    let missing = Shootdown { acks: 2, ..good };
    assert!(!missing.ok());

    let timed_out = Shootdown {
        timed_out: true,
        ..good
    };
    assert!(!timed_out.ok());

    let send_failed = Shootdown {
        send_failed: 1 << 1,
        ..good
    };
    assert!(!send_failed.ok());

    let extra = Shootdown { acks: 4, ..good };
    assert!(
        !extra.ok(),
        "ack thừa cũng là sai — không hart nào được ack hai lần"
    );
}

/// Trần chờ ack phải **có** và đủ rộng cho QEMU TCG (đo KT-259: ~1,4×10^4 vòng cho một IPI).
#[test]
fn ack_wait_limit_is_bounded_and_generous() {
    assert!(ACK_WAIT_LIMIT >= 1_000_000);
    assert!(ACK_WAIT_LIMIT <= 1_000_000_000);
}
