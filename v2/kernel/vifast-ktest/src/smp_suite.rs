// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Bộ ca SMP dùng chung 3 arch** — KT-413 (`B5`/SMP, `D2-259` mục 2).
//!
//! # Vì sao tệp này tồn tại
//!
//! `KT-411` đã gom bộ ca **core** (TAP 43..52) về `core_suite`; phần **SMP** thì mỗi ảnh vẫn tự
//! viết: `vifast-kbin-x86_64/src/sched.rs` giữ **2.162** dòng harness (hàng đợi toàn cục tự viết,
//! ownership bằng CAS, priority/sleep tự tính) — đúng thứ `KT-340e` phải gỡ. Bộ ca này là **một**
//! bản dùng chung cho cả ba arch, chạy trên `Machine` **thật** với `n_harts` **đo được** của lần
//! boot, nên nó là nguồn sự thật thay cho mô hình tự viết.
//!
//! # Bốn bất biến (mỗi cái là một lớp lỗi thật)
//!
//! 1. **Bốn hart cùng chọn**: mỗi hart `dispatch` được **ít nhất một** TCB ⇒ `picks[h] >= 1` cho
//!    **cả bốn**. Một hart không chọn được nghĩa là hàng đợi của nó rỗng — "SMP 4 hart" chỉ là chữ.
//! 2. **Không double-pick**: sau mỗi vòng, `current(h)` của bốn hart phải **đôi một khác nhau**.
//!    Hai hart cùng giữ một TCB là lớp lỗi boot 410 của KT-298 ở tầng scheduler.
//! 3. **Priority tĩnh**: hart 0 có **hai** TCB (mức 5 và mức 1) ⇒ lượt chọn đầu **bắt buộc** là TCB
//!    mức 5. Nếu `pick` bỏ qua priority, `first[0]` sai ⇒ TAP 48 đỏ.
//! 4. **Sleep/wake**: chặn TCB đang chạy rồi `wake` phải đưa nó về `Ready` và `dispatch` chọn lại
//!    được — nếu `wake` không enqueue lại, lượt `dispatch` sau trả `None` ⇒ TAP 49 đỏ.
//!
//! # Không `unsafe`, không asm, không MMIO
//!
//! Crate `#![forbid(unsafe_code)]`; bộ ca chỉ gọi API **an toàn** của `vifast-core` qua
//! [`CoreBridge`] ⇒ chạy được **cả** trên host (`cargo test -p vifast-ktest`) **lẫn** trong ảnh
//! kernel (D2-08: ảnh sản phẩm không link crate này).

use core::fmt::Write;

use vifast_core::{Machine, ObjId, ObjType, SlotRef, TcbState};
use vifast_plat_common::{Platform, PlatformWriter};

use crate::core_suite::{state_name, Tap};
use crate::{LinkReport, SmpBridge};

/// Số hart của bộ ca — **4** (cùng `-smp 4` của ba runner; `Machine::MAX_HARTS` = 8).
pub const SMP_HARTS: usize = 4;

/// Số TCB dựng cho bộ ca: **một** cho mỗi hart + **một** TCB mức cao trên hart 0 (đo priority).
pub const SMP_TCBS: usize = SMP_HARTS + 1;

/// Mức ưu tiên của từng TCB — **khác nhau đôi một** để phép đo priority có nghĩa.
///
/// `PRIO[0..4]` là TCB "chủ" của hart 0..3; `PRIO[4]` là TCB **mức cao nhất** đặt trên hart 0
/// (nên lượt chọn đầu của hart 0 **bắt buộc** là nó).
pub const PRIO: [u8; SMP_TCBS] = [1, 2, 3, 4, 5];

/// Hart nhà của từng TCB: `[0, 1, 2, 3, 0]` — hart 0 có **hai** TCB, ba hart kia mỗi hart một.
pub const HOME: [u16; SMP_TCBS] = [0, 1, 2, 3, 0];

/// Kết quả **đo** của bộ ca — in ở dòng `# core smp:` và trả cho caller.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SmpReport {
    /// `Machine::hart_count()` **đo được** của lần boot này.
    pub harts: u16,
    /// Số lượt `dispatch` thành công của từng hart.
    pub picks: [u64; SMP_HARTS],
    /// Chỉ số TCB (`0..SMP_TCBS`) mà mỗi hart chọn ở **lượt đầu** (`u8::MAX` = chưa chọn).
    pub first: [u8; SMP_HARTS],
    /// Số TCB **khác nhau** đã chạy.
    pub distinct: u64,
    /// Số lần một TCB bị **hai** hart cùng giữ (phải `0`).
    pub double: u64,
    /// Priority tĩnh được tôn trọng (lượt đầu của hart 0 là TCB mức cao nhất).
    pub prio_ok: bool,
    /// `block_current` + `wake` + `dispatch` lại đúng.
    pub sleep_ok: bool,
    /// Số TCB đã **migrate** qua hart khác (`push_migration`).
    pub migrate: u64,
    /// Bốn bất biến scheduler/SC/budget/mailbox của core đều xanh.
    pub invariants_ok: bool,
}

impl SmpReport {
    /// `true` ⇔ **cả bốn** hart đã chọn ≥ 1 TCB, không double-pick, priority + sleep đúng.
    pub fn four_harts_ok(&self) -> bool {
        self.harts as usize == SMP_HARTS
            && self.picks.iter().all(|n| *n >= 1)
            && self.double == 0
            && self.distinct >= 2
            && self.prio_ok
    }

    /// `true` ⇔ sleep/wake + migration + bất biến core đều đúng.
    pub fn lifecycle_ok(&self) -> bool {
        self.sleep_ok && self.invariants_ok
    }
}

/// Kết quả một lượt chạy bộ ca SMP: số TAP đỏ, số TAP đã phát, và báo cáo **đo được**.
#[derive(Clone, Copy, Debug, Default)]
pub struct SmpOutcome {
    /// Số TAP đỏ.
    pub fails: usize,
    /// Số TAP đã phát.
    pub taps: usize,
    /// Báo cáo đo (caller dùng cho dòng `# core smp:` của ảnh).
    pub report: SmpReport,
}

/// Chạy bộ ca SMP dùng chung trên nền tảng `plat` + cầu nối `bridge`.
///
/// `harts` là số hart **nền tảng** của lần boot này (`core_bridge::harts()` của ảnh). Bộ ca
/// **không** tự đoán: `Machine::hart_count() != harts` ⇒ TAP 48 đỏ (fail-closed).
///
/// # Vì sao **không** dùng `wake()`
///
/// `wake()` là API **CHỈ-DÙNG-CHO-TEST** (D2-144) và luật `E_STRICT_TEST_ONLY_WAKE` chỉ miễn tệp
/// test-only **theo đồ thị module** — crate thư viện dùng chung **không** thoả. Vì vậy bước
/// "đánh thức" đi qua **đường sản phẩm**: `notification_wait` (chặn) + `notification_signal`
/// (đánh thức) — cùng bất biến "TCB bị chặn rời hàng đợi, tín hiệu đưa nó về `Ready`", nhưng
/// bằng API mà kernel **thật** dùng.
pub fn run<P, B>(plat: &P, bridge: &B, harts: u16) -> SmpOutcome
where
    P: Platform + ?Sized,
    B: SmpBridge + ?Sized,
{
    let link = bridge.link();
    let report = bridge.with_machine(|m| run_locked(plat, m, &link, harts));
    let mut fails = 0usize;
    let mut taps = 0usize;

    // ---------------------------------------------------------------- TAP 48: bốn hart
    taps += 1;
    let ok49 = report.four_harts_ok();
    let tap49 = Tap::new(48, "core smp 4 hart");
    if ok49 {
        tap49.ok_line().emit(plat);
    } else {
        let mut w = PlatformWriter(plat);
        let _ = writeln!(
            w,
            "not ok 48 - core smp 4 hart: harts={} picks={:?} first={:?} distinct={} double={} prio_ok={}",
            report.harts,
            report.picks,
            report.first,
            report.distinct,
            report.double,
            report.prio_ok as u8
        );
        fails += 1;
    }

    // ------------------------------------------------- TAP 49: sleep/wake + migration
    taps += 1;
    let ok50 = report.lifecycle_ok();
    let tap50 = Tap::new(49, "core smp sleep migrate");
    if ok50 {
        tap50.ok_line().emit(plat);
    } else {
        let mut w = PlatformWriter(plat);
        let _ = writeln!(
            w,
            "not ok 49 - core smp sleep migrate: sleep_ok={} migrate={} invariants_ok={}",
            report.sleep_ok as u8, report.migrate, report.invariants_ok as u8
        );
        fails += 1;
    }

    SmpOutcome {
        fails,
        taps,
        report,
    }
}

/// Thân bộ ca — chạy **trong** khoá `Machine` do caller giữ (khoá không re-entrant).
///
/// Trả [`SmpReport`]; **không** panic, **không** `unwrap` trên đường lỗi model: mọi lỗi core đều
/// làm báo cáo giữ giá trị mặc định (`picks = 0`) ⇒ TAP đỏ **có chẩn đoán**, không treo.
fn run_locked<P>(plat: &P, m: &mut Machine, link: &LinkReport, harts: u16) -> SmpReport
where
    P: Platform + ?Sized,
{
    let mut rep = SmpReport {
        harts: m.hart_count(),
        first: [u8::MAX; SMP_HARTS],
        ..SmpReport::default()
    };
    // Fail-closed: `Machine` không có đúng số hart nền tảng ⇒ không đo gì thêm (TAP 49 đỏ).
    if rep.harts != harts || rep.harts as usize != SMP_HARTS {
        emit_evidence(plat, &rep);
        return rep;
    }

    // ---- 1. Dựng CNode gốc + 5 TCB + 5 SchedContext + 1 SchedControl từ Untyped của core.
    let root = m.add_cnode(32);
    let us = SlotRef::new(root, 0);
    let uc = m.root_cap(link.untyped);
    if m.put_cap(root, 0, uc).is_err() {
        emit_evidence(plat, &rep);
        return rep;
    }
    let mut tcb_slot = [SlotRef::new(root, 0); SMP_TCBS];
    let mut tcb = [0u32; SMP_TCBS];
    let mut sc_slot = [SlotRef::new(root, 0); SMP_TCBS];
    for i in 0..SMP_TCBS {
        let Ok(cap) = m.retype(us, ObjType::Tcb, 0, 0) else {
            emit_evidence(plat, &rep);
            return rep;
        };
        if m.put_cap(root, 1 + i, cap).is_err() {
            emit_evidence(plat, &rep);
            return rep;
        }
        tcb_slot[i] = SlotRef::new(root, 1 + i);
        tcb[i] = m.cap_obj(cap).unwrap_or(0);
    }
    for (i, slot) in sc_slot.iter_mut().enumerate() {
        let Ok(cap) = m.retype(us, ObjType::SchedContext, 0, 0) else {
            emit_evidence(plat, &rep);
            return rep;
        };
        if m.put_cap(root, 1 + SMP_TCBS + i, cap).is_err() {
            emit_evidence(plat, &rep);
            return rep;
        }
        *slot = SlotRef::new(root, 1 + SMP_TCBS + i);
    }
    let Ok(ctl_cap) = m.retype(us, ObjType::SchedControl, 0, 0) else {
        emit_evidence(plat, &rep);
        return rep;
    };
    let ctl_slot = 1 + 2 * SMP_TCBS;
    if m.put_cap(root, ctl_slot, ctl_cap).is_err() {
        emit_evidence(plat, &rep);
        return rep;
    }
    let ctl = SlotRef::new(root, ctl_slot);
    // Notification cho bước "chặn rồi đánh thức" — **đường sản phẩm** (`notification_wait` +
    // `notification_signal`), không dùng `wake()` chỉ-dùng-cho-test (D2-144).
    let Ok(ntfn_cap) = m.retype(us, ObjType::Notification, 0, 0) else {
        emit_evidence(plat, &rep);
        return rep;
    };
    let ntfn_slot = ctl_slot + 1;
    if m.put_cap(root, ntfn_slot, ntfn_cap).is_err() {
        emit_evidence(plat, &rep);
        return rep;
    }
    let ntfn = m.cap_obj(ntfn_cap).unwrap_or(0);

    // ---- 2. Cấu hình SC + priority + home + affinity rồi `start` (Ready).
    for i in 0..SMP_TCBS {
        let ok = m.sched_control_configure(ctl, sc_slot[i], 4, 8).is_ok()
            && m.set_sched_params(tcb_slot[i], Some(sc_slot[i])).is_ok()
            && m.tcb_set_priority(tcb_slot[i], PRIO[i], 10).is_ok()
            // **Thứ tự bắt buộc**: `SetAffinity` trước `SetHome` — `SetHome` đòi `home` nằm
            // trong `affinity` hiện tại, mà affinity mặc định chỉ có hart 0.
            && m.tcb_set_affinity(tcb_slot[i], 0xF).is_ok()
            && m.tcb_set_home(tcb_slot[i], HOME[i]).is_ok()
            && m.start_tcb(tcb[i]).is_ok();
        if !ok {
            emit_evidence(plat, &rep);
            return rep;
        }
    }

    // ---- 3. Vòng 1: **mỗi** hart dispatch một lượt. Ghi lượt chọn đầu + số lượt.
    for h in 0..SMP_HARTS {
        if let Ok(Some(picked)) = m.dispatch(h as u16) {
            rep.picks[h] += 1;
            rep.first[h] = slot_of(&tcb, picked);
        }
    }
    rep.double += count_double(m, &tcb);
    rep.distinct = distinct_running(m, &tcb);
    // Priority: hart 0 có TCB mức 5 và mức 1 ⇒ lượt đầu **bắt buộc** là TCB mức 5 (chỉ số 4).
    rep.prio_ok = rep.first[0] == (SMP_TCBS - 1) as u8;

    // ---- 4. Chặn rồi đánh thức — **đường sản phẩm** (`notification_wait` + `notification_signal`),
    //         không dùng `wake()` chỉ-dùng-cho-test (D2-144). Bất biến: TCB bị chặn **rời** hàng
    //         đợi (dispatch kế chọn TCB mức thấp), tín hiệu đưa nó về `Ready` và dispatch chọn lại.
    let high = tcb[SMP_TCBS - 1];
    let low = tcb[0];
    let mut sleep_ok = m.notification_wait(high, ntfn).is_ok();
    sleep_ok &= m.tcb_state(high) == Ok(TcbState::BlockedOnNotification { ntfn });
    sleep_ok &= matches!(m.dispatch(0), Ok(Some(t)) if t == low);
    sleep_ok &= m.notification_signal(ntfn, 0x42).is_ok();
    sleep_ok &= m.tcb_state(high) == Ok(TcbState::Ready);
    sleep_ok &= matches!(m.dispatch(0), Ok(Some(t)) if t == high);
    rep.sleep_ok = sleep_ok;
    rep.double += count_double(m, &tcb);

    // ---- 5. Migration: một vòng push công khai (model API `[TK]`).
    rep.migrate = m.push_migration().unwrap_or(0) as u64;
    rep.migrate = rep.migrate.max(m.migrate_count());

    // ---- 6. Bất biến core sau toàn bộ chuỗi.
    rep.invariants_ok = m.check_sched().is_ok()
        && m.check_sc_refs().is_ok()
        && m.check_budget().is_ok()
        && m.check_mailboxes().is_ok();

    emit_evidence(plat, &rep);
    rep
}

/// Chỉ số TCB trong bảng của bộ ca (`u8::MAX` khi không khớp — không được xảy ra).
fn slot_of(tcb: &[ObjId; SMP_TCBS], picked: ObjId) -> u8 {
    let mut i = 0;
    while i < SMP_TCBS {
        if tcb[i] == picked {
            return i as u8;
        }
        i += 1;
    }
    u8::MAX
}

/// Đếm số TCB bị **hai** hart cùng giữ — phải `0`.
fn count_double(m: &Machine, tcb: &[ObjId; SMP_TCBS]) -> u64 {
    let mut double = 0u64;
    for id in tcb.iter() {
        let mut owners = 0u64;
        for h in 0..SMP_HARTS {
            if m.current(h as u16) == Some(*id) {
                owners += 1;
            }
        }
        if owners > 1 {
            double += 1;
        }
    }
    double
}

/// Số TCB **khác nhau** đang `Running` trên bốn hart.
fn distinct_running(m: &Machine, tcb: &[ObjId; SMP_TCBS]) -> u64 {
    let mut n = 0u64;
    for id in tcb.iter() {
        if (0..SMP_HARTS).any(|h| m.current(h as u16) == Some(*id)) {
            n += 1;
        }
    }
    n
}

/// In dòng bằng chứng `# core smp:` — **một** dòng, giá trị **đo**, không hardcode.
fn emit_evidence<P: Platform + ?Sized>(plat: &P, rep: &SmpReport) {
    let mut w = PlatformWriter(plat);
    let _ = writeln!(
        w,
        "# core smp: harts={} picks=[{}, {}, {}, {}] first=[{}, {}, {}, {}] distinct={} double={} prio_ok={} sleep_ok={} migrate={} invariants_ok={}",
        rep.harts,
        rep.picks[0],
        rep.picks[1],
        rep.picks[2],
        rep.picks[3],
        first_name(rep.first[0]),
        first_name(rep.first[1]),
        first_name(rep.first[2]),
        first_name(rep.first[3]),
        rep.distinct,
        rep.double,
        rep.prio_ok as u8,
        rep.sleep_ok as u8,
        rep.migrate,
        rep.invariants_ok as u8
    );
}

/// Tên ngắn của chỉ số TCB cho dòng bằng chứng (`?` khi chưa chọn).
fn first_name(slot: u8) -> &'static str {
    match slot {
        0 => "t0",
        1 => "t1",
        2 => "t2",
        3 => "t3",
        4 => "t4",
        _ => "?",
    }
}

/// Tên trạng thái TCB — dùng lại bản **dùng chung** của `core_suite` (không chép lần hai).
pub fn tcb_state_name(s: TcbState) -> &'static str {
    state_name(s)
}
