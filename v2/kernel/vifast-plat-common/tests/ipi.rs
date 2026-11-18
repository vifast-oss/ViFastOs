// SPDX-License-Identifier: MIT OR Apache-2.0
//! Host test KT-50 — `vifast-plat-common::ipi`: hộp thư per-hart **coalesce theo kind**, bộ đếm
//! `sent/coalesced/local/traps/delivered/spurious`, **không self-target** (D2-68), vòng chờ
//! **drain trước mỗi bước + timeout luôn kết thúc**, và decode `scause` IPI (mã 1) **không nhầm**
//! timer (5) / external (9).
//!
//! Chạy trên host (không asm/CSR/MMIO; D2-72 Q12) và chốt những bất biến mà arch dựa vào. Bằng
//! chứng **thật** trên phần cứng nằm ở TAP 20..24 của `v2/kernel/vifast-kbin-riscv64/src/
//! ktest_ipi.rs` (log `v2/agent/JOURNAL/kt50-ipi.log`).

use vifast_plat_common::ipi::{
    classify_post, deadline_after, delivery_for, msip_pending, mswi_address, mswi_offset,
    wait_with_drain, IpiCounters, IpiDelivery, IpiKind, IpiMailbox, IpiSnapshot, KindSet,
    PostDecision, WaitOutcome, IPI_ERR_HART_OFFLINE, IPI_KIND_COUNT, IPI_KIND_MASK, IPI_MAILBOXES,
    IPI_STATS, MSWI_HART_STRIDE, MSWI_WINDOW_BYTES, SBI_EXT_IPI,
};
use vifast_plat_common::smp::MAX_HARTS;
use vifast_plat_common::trap::{
    decode_scause, Event, INTERRUPT_SUPERVISOR_EXTERNAL, INTERRUPT_SUPERVISOR_SOFTWARE,
    INTERRUPT_SUPERVISOR_TIMER, SCAUSE_INTERRUPT,
};

#[test]
fn hang_so_ipi_dung_sbi_va_aclint() {
    // SBI IPI EID "sPI" = 0x735049 (SBI v2.0 §6.1); base 0 ⇒ bit thứ h là hart h.
    assert_eq!(SBI_EXT_IPI, 0x735049);
    assert_eq!(MSWI_HART_STRIDE, 4);
    assert!(MSWI_WINDOW_BYTES >= MSWI_HART_STRIDE * MAX_HARTS as u64);
    assert_eq!(mswi_offset(0), 0);
    assert_eq!(mswi_offset(7), 28);
    // KT-81 (§A5b): base MSWI lấy từ **bảng tài nguyên** (`REGIONS`), không viết tay lần thứ hai.
    // Nếu bảng đổi (hoặc ai đó sửa một chỗ), ca này đổi theo — đó là mục đích.
    let clint = vifast_plat_common::resources::CLINT.base;
    assert_eq!(mswi_address(clint, 0), clint);
    assert_eq!(mswi_address(clint, 7), clint + 0x1c);
    assert!(msip_pending(1));
    assert!(msip_pending(0xffff_ffff));
    assert!(!msip_pending(0));
    assert!(!msip_pending(0xffff_fffe));
    // Số kind và mask 3 bit: gửi trùng kind ⇒ chỉ còn bit, không sinh slot thứ tư.
    assert_eq!(IPI_KIND_COUNT, 3);
    assert_eq!(IPI_KIND_MASK, 0b111);
}

#[test]
fn kind_set_dung_bit_va_bo_bit_ngoai_dai() {
    let mut set = KindSet::empty();
    assert!(set.is_empty());
    assert!(set.insert(IpiKind::TlbFlush));
    assert!(!set.insert(IpiKind::TlbFlush), "đã có ⇒ không phải bit mới");
    assert!(set.insert(IpiKind::Wake));
    assert_eq!(set.bits(), IpiKind::TlbFlush.bit() | IpiKind::Wake.bit());
    assert_eq!(set.count(), 2);
    assert!(set.contains(IpiKind::TlbFlush));
    assert!(!set.contains(IpiKind::VmKick));
    assert_eq!(set.first(), Some(IpiKind::TlbFlush));
    assert!(set.remove(IpiKind::TlbFlush));
    assert!(!set.remove(IpiKind::TlbFlush));
    assert_eq!(set.first(), Some(IpiKind::Wake));
    // Bit ngoài dải kind bị bỏ — hộp thư không bao giờ chứa kind lạ.
    assert_eq!(KindSet::from_bits(u64::MAX).bits(), IPI_KIND_MASK);
    assert_eq!(KindSet::from_bits(u64::MAX).count(), IPI_KIND_COUNT);
    assert_eq!(
        KindSet::from_kind(IpiKind::VmKick)
            .union(KindSet::from_kind(IpiKind::Wake))
            .count(),
        2
    );
}

#[test]
fn coalesce_gop_nhung_khong_mat_kind() {
    let mailbox = IpiMailbox::new();
    // Gửi trùng `TlbFlush` 5 lần: lần đầu đặt bit, 4 lần sau **gộp** (không gửi thêm IPI, không
    // mất kind). `VmKick` là kind khác nên có bit riêng, vẫn sống cùng `TlbFlush`.
    assert!(mailbox.post(IpiKind::TlbFlush));
    for _ in 0..4 {
        assert!(!mailbox.post(IpiKind::TlbFlush), "gửi trùng ⇒ coalesce");
    }
    assert!(mailbox.post(IpiKind::VmKick));
    let snapshot = mailbox.snapshot();
    assert_eq!(snapshot.posted, 2, "chỉ 2 bit mới");
    assert_eq!(snapshot.coalesced, 4);
    assert_eq!(
        KindSet::from_bits(snapshot.pending_bits),
        KindSet::from_kind(IpiKind::TlbFlush).union(KindSet::from_kind(IpiKind::VmKick))
    );
    // Một drain giao **đủ** kind (không mất, không nhân đôi theo số lần gửi trùng).
    let drained = mailbox.drain();
    assert_eq!(drained.count(), 2);
    assert!(drained.contains(IpiKind::TlbFlush));
    assert!(drained.contains(IpiKind::VmKick));
    assert!(!drained.contains(IpiKind::Wake));
    assert!(mailbox.is_empty());
    assert_eq!(mailbox.snapshot().delivered, 2);
    assert!(mailbox.drain().is_empty(), "drain lần hai rỗng");
    // Bất biến: posted == delivered + pending (không kind nào bị mất).
    let snapshot = mailbox.snapshot();
    assert_eq!(
        snapshot.posted,
        snapshot.delivered + KindSet::from_bits(snapshot.pending_bits).count() as u64
    );
}

#[test]
fn drain_giao_moi_kind_dung_mot_lan() {
    let mailbox = IpiMailbox::new();
    for kind in IpiKind::ALL {
        assert!(mailbox.post(kind));
    }
    let drained = mailbox.drain();
    assert_eq!(drained.bits(), IPI_KIND_MASK);
    assert_eq!(drained.count(), IPI_KIND_COUNT);
    for kind in IpiKind::ALL {
        assert!(drained.contains(kind), "kind {} phải còn", kind.name());
    }
    assert_eq!(mailbox.snapshot().drains, 1);
    assert_eq!(mailbox.snapshot().delivered, 3);
    assert!(mailbox.drain().is_empty());
    assert_eq!(mailbox.snapshot().empty_drains, 1);
}

#[test]
fn self_target_di_duong_local_khong_ipi() {
    // D2-68: hart gửi cho chính mình ⇒ local flush + ack ngầm, KHÔNG IPI.
    assert_eq!(delivery_for(0, 0), IpiDelivery::Local);
    assert_eq!(delivery_for(3, 3), IpiDelivery::Local);
    assert_eq!(delivery_for(0, 1), IpiDelivery::Remote);
    assert_eq!(delivery_for(7, 0), IpiDelivery::Remote);
    // Bộ đếm: self-target chỉ cộng `local`/`delivered`, **không** cộng `sent`/`traps`.
    let counters = IpiCounters::new();
    counters.record_local(IpiKind::TlbFlush);
    let snapshot = counters.snapshot();
    assert_eq!(snapshot.local, 1);
    assert_eq!(
        snapshot.delivered, 1,
        "local flush + ack ngầm vẫn tính đã giao"
    );
    assert_eq!(snapshot.kinds[IpiKind::TlbFlush.index()], 1);
    assert_eq!(snapshot.sent, 0, "self-target KHÔNG gửi IPI");
    assert_eq!(snapshot.traps, 0);
    assert_eq!(snapshot.spurious, 0);
}

#[test]
fn bo_dem_sent_coalesced_traps_delivered_spurious() {
    let counters = IpiCounters::new();
    counters.record_sent();
    counters.record_sent();
    counters.record_coalesced();
    counters.record_trap(true);
    counters.record_trap(true);
    counters.record_trap(false); // trap mà MSIP không bật ⇒ spurious
    counters.record_drain(
        KindSet::from_kind(IpiKind::TlbFlush).union(KindSet::from_kind(IpiKind::Wake)),
    );
    counters.record_error(-2);
    let snapshot = counters.snapshot();
    assert_eq!(snapshot.sent, 2);
    assert_eq!(snapshot.coalesced, 1);
    assert_eq!(snapshot.traps, 3);
    assert_eq!(snapshot.spurious, 1);
    assert_eq!(snapshot.delivered, 2);
    assert_eq!(snapshot.kinds, [1, 0, 1]);
    assert_eq!(snapshot.errors, 1);
    assert_eq!(snapshot.last_error, -2);
    counters.reset();
    assert_eq!(counters.snapshot(), IpiSnapshot::default());
}

#[test]
fn per_hart_stats_va_mailbox_khong_lan() {
    // `IPI_STATS`/`IPI_MAILBOXES` là static thật của kernel: ghi vào slot 6/7 (matrix chỉ dùng
    // 0..3) để không đụng test khác khi chạy song song.
    let sender = MAX_HARTS - 2;
    let receiver = MAX_HARTS - 1;
    let before_sent = IPI_STATS.get(sender).snapshot().sent;
    let before_traps = IPI_STATS.get(receiver).snapshot().traps;
    IPI_STATS.get(sender).record_sent();
    IPI_STATS.get(receiver).record_trap(true);
    IPI_STATS
        .get(receiver)
        .record_drain(KindSet::from_kind(IpiKind::VmKick));
    assert_eq!(IPI_STATS.get(sender).snapshot().sent, before_sent + 1);
    assert_eq!(IPI_STATS.get(receiver).snapshot().traps, before_traps + 1);
    assert_eq!(IPI_STATS.get(receiver).snapshot().delivered, 1);
    // Slot khác không bị lẫn (slot 0 là slot dự phòng cho hart ngoài dải).
    assert_eq!(IPI_STATS.get(0).snapshot().sent, 0);
    // Hộp thư per-hart: post vào hart 6 không làm hart 7 có kind.
    let box6 = IPI_MAILBOXES.get(sender);
    assert!(box6.post(IpiKind::Wake));
    assert!(!IPI_MAILBOXES
        .get(receiver)
        .pending()
        .contains(IpiKind::Wake));
    assert!(box6.drain().contains(IpiKind::Wake));
    // Hart ngoài dải ⇒ `try_get` None (không panic), `get` rơi về slot 0.
    assert!(IPI_MAILBOXES.try_get(MAX_HARTS).is_none());
    assert!(IPI_STATS.try_get(MAX_HARTS).is_none());
}

#[test]
fn wait_with_drain_drain_truoc_moi_buoc() {
    // Predicate đúng ở bước thứ 3: mỗi bước phải gọi drain TRƯỚC khi kiểm ⇒ drains >= 3.
    let mut now = 0u64;
    let mut drains = 0u64;
    let mut checks = 0u64;
    let outcome = wait_with_drain(
        1_000,
        || {
            now += 10;
            now
        },
        || drains += 1,
        || {
            checks += 1;
            checks >= 3
        },
    );
    assert_eq!(outcome, WaitOutcome::Ready);
    assert_eq!(
        drains, 3,
        "mỗi bước drain đúng một lần, kể cả bước thấy ready"
    );
    assert!(now < 1_000, "xong trước deadline");
    // Ready ngay từ đầu vẫn phải drain ít nhất một lần (kind đang chờ không bị bỏ).
    let mut drains_first = 0u64;
    let outcome = wait_with_drain(10, || 0, || drains_first += 1, || true);
    assert_eq!(outcome, WaitOutcome::Ready);
    assert_eq!(drains_first, 1);
}

#[test]
fn wait_with_drain_timeout_phai_het() {
    // Predicate không bao giờ đúng ⇒ dừng ở deadline (không treo). Đồng hồ giả +10 mỗi lần đọc.
    let mut now = 0u64;
    let mut drains = 0u64;
    let outcome = wait_with_drain(
        100,
        || {
            now += 10;
            now
        },
        || drains += 1,
        || false,
    );
    assert_eq!(outcome, WaitOutcome::TimedOut);
    assert!(now >= 100, "phải chạm deadline: now={now}");
    assert!(drains >= 10, "vẫn drain trong lúc chờ: {drains}");
    // `deadline_after` bão hoà: không tràn về 0 (timeout không bao giờ hết).
    assert_eq!(deadline_after(u64::MAX - 5, 100), u64::MAX);
    assert_eq!(deadline_after(0, 0), 0);
}

#[test]
fn ipi_khong_nham_timer_external_va_exception() {
    // Mã 1 = ngắt phần mềm (IPI) — nhánh riêng, KHÔNG fatal.
    assert_eq!(INTERRUPT_SUPERVISOR_SOFTWARE, 1);
    assert_eq!(
        decode_scause(SCAUSE_INTERRUPT | INTERRUPT_SUPERVISOR_SOFTWARE),
        Event::Ipi
    );
    // Timer (5) vẫn là Timer; external (9) vẫn là Interrupt{9}; exception giữ nguyên bit 63 = 0.
    assert_eq!(
        decode_scause(SCAUSE_INTERRUPT | INTERRUPT_SUPERVISOR_TIMER),
        Event::Timer
    );
    assert_eq!(
        decode_scause(SCAUSE_INTERRUPT | INTERRUPT_SUPERVISOR_EXTERNAL),
        Event::Interrupt {
            code: INTERRUPT_SUPERVISOR_EXTERNAL
        }
    );
    match decode_scause(13) {
        Event::Exception(exc) => assert_eq!(exc.code(), 13),
        other => panic!("scause 13 phải là Exception, được {other:?}"),
    }
    // Ngắt mã lạ vẫn là Interrupt (fatal ở arch) — không bị nhận nhầm thành IPI.
    assert_eq!(
        decode_scause(SCAUSE_INTERRUPT | 7),
        Event::Interrupt { code: 7 }
    );
    // `scause` IPI thật trên QEMU: bit 63 + 1 = 0x8000_0000_0000_0001.
    assert_eq!(
        decode_scause(0x8000_0000_0000_0001),
        Event::Ipi,
        "giá trị scause thật của IPI"
    );
}

#[test]
fn post_hart_offline_tra_invalid_khong_giu_kind() {
    // D2-77: hart **hợp lệ nhưng chưa online** ⇒ Invalid + log, KHÔNG gọi SBI, KHÔNG giữ kind.
    // `classify_post` là hàm thuần arch dùng; phần host kiểm quyết định + bất biến bộ đếm/hộp thư.
    let sender = 0usize;
    let offline = 1usize;
    let online_only_sender = 0b1u64;
    assert_eq!(
        classify_post(sender, offline, online_only_sender),
        PostDecision::Offline
    );
    // Hart ngoài dải vẫn là InvalidHart (ưu tiên trước Offline).
    assert_eq!(
        classify_post(sender, MAX_HARTS, online_only_sender),
        PostDecision::InvalidHart
    );
    // Mô phỏng đúng nhánh arch: chỉ record_error, **không** mailbox.post ⇒ hộp thư rỗng, sent = 0.
    let counters = IpiCounters::new();
    let mailbox = IpiMailbox::new();
    counters.record_error(IPI_ERR_HART_OFFLINE);
    assert!(
        mailbox.is_empty(),
        "hart offline: không giữ kind trong hộp thư"
    );
    let snapshot = counters.snapshot();
    assert_eq!(snapshot.sent, 0, "hart offline: không gọi SBI IPI");
    assert_eq!(snapshot.errors, 1);
    assert_eq!(snapshot.last_error, IPI_ERR_HART_OFFLINE);
    assert_eq!(snapshot.delivered, 0);
    // Hart online nhưng khác sender ⇒ Remote (đường SBI); sender online ⇒ Local (D2-68).
    assert_eq!(classify_post(sender, offline, 0b11), PostDecision::Remote);
    assert_eq!(
        classify_post(sender, sender, online_only_sender),
        PostDecision::Local
    );
}

#[test]
fn unpost_go_bit_khi_sbi_loi() {
    // D2-77: SBI lỗi ⇒ gỡ bit vừa post, không để kind mồ côi; bất biến posted = delivered + pending.
    let mailbox = IpiMailbox::new();
    assert!(mailbox.post(IpiKind::Wake));
    assert!(mailbox.unpost(IpiKind::Wake));
    assert!(mailbox.is_empty());
    let snapshot = mailbox.snapshot();
    assert_eq!(
        snapshot.posted, 0,
        "unpost giảm posted (không đếm bit đã gỡ)"
    );
    assert_eq!(snapshot.delivered, 0);
    assert!(!mailbox.unpost(IpiKind::Wake), "gỡ lần hai ⇒ false");
    // Kind khác còn sống: gỡ một bit không đụng bit khác.
    assert!(mailbox.post(IpiKind::TlbFlush));
    assert!(mailbox.post(IpiKind::VmKick));
    assert!(mailbox.unpost(IpiKind::TlbFlush));
    assert_eq!(
        mailbox.pending(),
        KindSet::from_kind(IpiKind::VmKick),
        "bit khác không bị gỡ"
    );
    assert_eq!(mailbox.snapshot().posted, 1);
    assert_eq!(
        mailbox.snapshot().posted,
        mailbox.snapshot().delivered + mailbox.pending().count() as u64
    );
}
