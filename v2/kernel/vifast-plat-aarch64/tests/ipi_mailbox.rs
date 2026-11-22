// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-338c** (D2-244 tầng 1) — host test cho **hộp thư IPI bitmask dùng chung** trên aarch64.
//!
//! Phần **thuần** kiểm được trên host: ánh xạ `kind → SGI`, **coalesce** của `IpiMailbox`
//! (bit vừa đặt ⇒ `true`, bit đang chờ ⇒ `false` — đúng ngữ nghĩa D2-68/D2-77), `drain` giao
//! **đủ mọi kind** và **xoá** hộp thư, và bộ đếm `stats()` là số **đo** (không phải hằng).
//!
//! `post()` thật gửi SGI qua `ICC_SGI1R_EL1` nên **không** gọi trên host (asm aarch64 không chạy
//! trên x86_64/arm64 macOS); đường ấy được đo bằng QEMU ở `run-aarch64.sh` (TAP 11 +
//! `# ipi: doorbell … drained=1 empty=0`) — host test ở đây chỉ chốt **bitmask**.
//!
//! Dùng hart 6/7 để không đụng test khác (`MAX_HARTS = 8`).

use vifast_plat_aarch64::ipi::{self as doorbell, PostOutcome};
use vifast_plat_common::ipi::IpiKind;

#[test]
fn sgi_for_maps_three_kinds_to_three_distinct_sgis() {
    let wake = doorbell::sgi_for(IpiKind::Wake);
    let tlb = doorbell::sgi_for(IpiKind::TlbFlush);
    let kick = doorbell::sgi_for(IpiKind::VmKick);
    assert_eq!(wake, vifast_plat_aarch64::gic::SGI_IPI);
    assert_eq!(tlb, vifast_plat_aarch64::gic::SGI_SHOOTDOWN);
    assert_eq!(kick, vifast_plat_aarch64::gic::SGI_COMMAND);
    // Ba số **khác nhau** là điều kiện sống còn của handler (note_ipi chỉ đếm SGI 0).
    assert!(wake < 16 && tlb < 16 && kick < 16);
    assert_ne!(wake, tlb);
    assert_ne!(wake, kick);
    assert_ne!(tlb, kick);
}

#[test]
fn post_outcome_names_are_stable() {
    assert_eq!(PostOutcome::Sent.name(), "sent");
    assert_eq!(PostOutcome::Coalesced.name(), "coalesced");
    assert_eq!(PostOutcome::Invalid.name(), "invalid");
    assert_eq!(PostOutcome::Local.name(), "local");
    assert!(PostOutcome::Sent.queued());
    assert!(PostOutcome::Coalesced.queued());
    assert!(!PostOutcome::Invalid.queued());
    assert!(!PostOutcome::Local.queued());
}

#[test]
fn post_bit_only_coalesces_and_drain_delivers_all_kinds() {
    let hart = 7;
    // Hộp thư sạch trước khi đo (test khác có thể đã chạm hart này).
    let _ = doorbell::drain_hart(hart);
    assert!(
        doorbell::post_bit_only(hart, IpiKind::TlbFlush),
        "bit đầu phải là bit mới"
    );
    assert!(
        !doorbell::post_bit_only(hart, IpiKind::TlbFlush),
        "cùng kind đang chờ ⇒ coalesce (không nhân đôi message)"
    );
    assert!(
        doorbell::post_bit_only(hart, IpiKind::Wake),
        "kind khác vẫn giữ bit riêng"
    );
    assert_eq!(doorbell::pending(hart).count(), 2);
    let kinds = doorbell::drain_hart(hart);
    assert!(kinds.contains(IpiKind::TlbFlush));
    assert!(kinds.contains(IpiKind::Wake));
    assert!(!kinds.contains(IpiKind::VmKick));
    assert!(doorbell::pending(hart).is_empty(), "drain phải xoá hộp thư");
    // Lần nữa: sau khi drain, bit được coi là **mới** (không mất kind vì coalesce cũ).
    assert!(doorbell::post_bit_only(hart, IpiKind::Wake));
    assert!(!doorbell::drain_hart(hart).is_empty());
}

#[test]
fn stats_are_measured_not_constant() {
    let hart = 6;
    let _ = doorbell::drain_hart(hart);
    let before = doorbell::stats();
    assert!(
        doorbell::post_bit_only(hart, IpiKind::VmKick),
        "bit đầu phải là bit mới"
    );
    // Cùng kind đang chờ ⇒ **coalesce**: `false` (không gửi thêm SGI), bit vẫn còn.
    assert!(!doorbell::post_bit_only(hart, IpiKind::VmKick));
    let drained = doorbell::drain_hart(hart);
    assert_eq!(drained.count(), 1);
    let after = doorbell::stats();
    // `posted` +2, `coalesced` +1, `drained` +1 — số **đo** theo hành vi vừa gây ra.
    assert!(after.posted >= before.posted + 2);
    assert!(after.coalesced >= before.coalesced + 1);
    assert!(after.drained >= before.drained + 1);
}

#[test]
fn post_bit_only_rejects_invalid_hart_without_touching_mailbox() {
    // `MAX_HARTS = 8` ⇒ hart 99 ngoài dải: `post_bit_only` trả `false`, không panic, không ghi bit.
    // (`post()` — bản gửi SGI thật — **không** gọi được trên host: nó đọc `MPIDR_EL1` qua stub
    // panic; đường ấy do QEMU đo ở `run-aarch64.sh`.)
    assert!(!doorbell::post_bit_only(99, IpiKind::Wake));
    assert_eq!(
        doorbell::pending(99),
        vifast_plat_common::ipi::KindSet::empty()
    );
}
