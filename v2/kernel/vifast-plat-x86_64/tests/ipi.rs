// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-340d** — host test cho hợp đồng **thuần** của `ipi` (D2-05: test trước, hành vi sau).
//!
//! Những gì `cargo test` trên host **phải** chốt được (không cần QEMU, không cần MMIO):
//!
//! - lựa chọn hart **đích** của một shootdown **không bao giờ** chứa hart gọi (D2-68 —
//!   self-target bị cấm, không có "flush chính mình bằng IPI");
//! - ACK chỉ hợp lệ khi `seq` **khớp** và **khác 0** (`0` là "ô rỗng" — một ô rỗng không bao giờ
//!   được coi là "đã ACK", nếu không thì phép chờ ACK biến thành phép chờ **luôn đúng**);
//! - tổ hợp bit `mode` (`MODE_OBSERVE`/`MODE_CHAIN`/`MODE_REMAP`) rời nhau — gộp bit là gộp hai
//!   phép đo khác nhau;
//! - vector IPI **đúng** `0x30` (khớp stub `KT-278`/`IDT`) và `MAX_HARTS` khớp crate dùng chung
//!   (lệch là `active_on`/`online_mask` tràn `u32`).

use vifast_plat_common::ipi::{IpiKind, KindSet};
use vifast_plat_x86_64::ipi::{
    ack_ok, mode_has, post_decision, shootdown_targets, FlushRequest, IPI_VECTOR, MAX_HARTS,
    MODE_CHAIN, MODE_OBSERVE, MODE_REMAP, NO_CHAIN,
};

#[test]
fn ipi_vector_is_the_dt_stub_vector() {
    // `0x30` có stub riêng trong `exc::stub_slot`; đổi vector mà không đổi IDT ⇒ IPI rơi vào
    // `isr_unknown` và **không** ai drain hộp thư.
    assert_eq!(IPI_VECTOR, 0x30);
}

#[test]
fn max_harts_matches_shared_crate() {
    assert_eq!(MAX_HARTS, vifast_plat_common::smp::MAX_HARTS);
    // `active_on` của `VSpace` và `online_mask` ở đây là `u32` ⇒ trần cứng 32.
    assert!(MAX_HARTS <= 32);
}

#[test]
fn shootdown_never_targets_the_calling_hart() {
    // D2-68 — hart gọi flush **cục bộ** rồi mới IPI các hart khác.
    assert_eq!(shootdown_targets(0b1111, 0), 0b1110);
    assert_eq!(shootdown_targets(0b1111, 1), 0b1101);
    assert_eq!(shootdown_targets(0b0010, 1), 0);
    // Hart ngoài dải: **không** loại trừ gì (fail-closed — vẫn gửi cho mọi hart đang active,
    // thay vì im lặng bỏ qua một hart).
    assert_eq!(shootdown_targets(0b1111, MAX_HARTS), 0b1111);
}

#[test]
fn ack_requires_matching_nonzero_seq() {
    assert!(ack_ok(7, 7));
    assert!(!ack_ok(6, 7));
    // Ô rỗng (`acked = 0`) với `seq = 0` **không** phải ACK — nếu không, mọi lần chờ đều xanh.
    assert!(!ack_ok(0, 0));
    assert!(!ack_ok(0, 1));
    assert!(!ack_ok(1, 0));
}

#[test]
fn flush_mode_bits_are_distinct() {
    for bit in [MODE_OBSERVE, MODE_CHAIN, MODE_REMAP] {
        assert_ne!(bit, 0);
    }
    assert_eq!(MODE_OBSERVE & MODE_CHAIN, 0);
    assert_eq!(MODE_OBSERVE & MODE_REMAP, 0);
    assert_eq!(MODE_CHAIN & MODE_REMAP, 0);
    let all = MODE_OBSERVE | MODE_CHAIN | MODE_REMAP;
    assert!(mode_has(all, MODE_OBSERVE) && mode_has(all, MODE_CHAIN) && mode_has(all, MODE_REMAP));
    assert!(!mode_has(MODE_REMAP, MODE_OBSERVE));
}

#[test]
fn flush_request_constructors() {
    let flush = FlushRequest::flush(3, 0x1000);
    assert_eq!(flush.seq, 3);
    assert_eq!(flush.va, 0x1000);
    assert_eq!(flush.mode, 0);
    assert!(!flush.has(MODE_OBSERVE));
    // `NO_CHAIN` **không** được trùng một hart hợp lệ (hart 0 là BSP).
    assert!(NO_CHAIN >= MAX_HARTS as u64);
    assert_eq!(flush.chain, NO_CHAIN);
    let observe = FlushRequest::observe(4, 0x2000);
    assert!(observe.has(MODE_OBSERVE));
    assert!(!observe.has(MODE_CHAIN) && !observe.has(MODE_REMAP));
}

#[test]
fn post_decision_offline_before_local() {
    // D2-77 — hart **chưa online** phải bị loại **trước** nhánh self-target: một hart chưa chạy
    // không có ai drain, nên kind nằm lại vô chủ.
    // 0 = InvalidHart, 1 = Offline, 2 = Local, 3 = Remote (thứ tự của `PostDecision`).
    assert_eq!(post_decision(0, MAX_HARTS, 0b1), 0);
    assert_eq!(post_decision(0, 1, 0b1), 1);
    assert_eq!(post_decision(0, 0, 0b1), 2);
    assert_eq!(post_decision(0, 1, 0b11), 3);
}

#[test]
fn kindset_tlbflush_is_one_bit() {
    // Hộp thư dùng chung: `TlbFlush` là bit 0 ⇒ coalesce theo kind (hai yêu cầu cùng lúc chỉ một
    // IPI), đúng ngữ nghĩa `KT-50`/`D2-68`.
    let mut set = KindSet::empty();
    assert!(set.insert(IpiKind::TlbFlush));
    assert!(!set.insert(IpiKind::TlbFlush));
    assert!(set.contains(IpiKind::TlbFlush));
    assert_eq!(set.count(), 1);
    assert_eq!(set.bits(), IpiKind::TlbFlush.bit());
}
