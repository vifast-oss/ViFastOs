// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-20 — **MCS budget** cho mô hình TCB: `SchedContext` có `budget`/`period`/`consumed`/
//! `refill` + timer ảo `tick(n)`, TCB chỉ chạy khi SC còn budget, hết budget ⇒
//! `Sleeping{reason = Budget}`, refill đầu chu kỳ ⇒ `Ready`. **Model-only `[TK]`**, không phải
//! spec/ABI (D2-49(b,e,h), D2-16).
//!
//! Nguồn ngữ nghĩa: thiết kế §4.5 (`docs/v2/ViFastOs-v2-thiet-ke.md:288-311`, bảng chuyển trạng
//! thái ngân sách `:302-311`), **D2-49(e)** ("budget kiểm **chu kỳ thẳng hàng**; cửa sổ trượt/
//! refill sporadic **hoãn sau K6**" — D2-16), **D2-49(h)** (tick ảo, không timer HW/QEMU),
//! kế hoạch `review/agent0/K1B-PLAN.md` §5 KT-20 (DoD 1-11).
//!
//! Quy ước mô hình được chốt ở đây (ghi rõ, xem thêm `v2/agent/STATE.md` mục KT-20):
//!
//! 1. **Chu kỳ thẳng hàng**: `refill = bội số kế tiếp của period` (tính từ lúc
//!    `SchedControl.Configure`); mọi SC cùng `period` có cùng biên. **Không** có cửa sổ trượt.
//! 2. **Charge trước, refill sau**: một tick chạy TCB đang `Running` cho đơn vị `[now, now+1)`,
//!    charge vào `consumed` **của thời điểm `now`**, rồi `now += 1`, rồi mới xử lý biên refill tại
//!    `now` mới. Vì thế tổng charge trong cửa sổ `[k·period, (k+1)·period)` ≤ `budget`.
//! 3. **TCB không SC / SC chưa `SchedControl.Configure` = không có budget ⇒ không chạy**:
//!    `pick`/`dispatch` bỏ qua (ngữ nghĩa §4.5, K1B-PLAN §5 KT-20 DoD 4 — **sửa theo verdict
//!    reviewer-kt20 mục 1**, không phải "không giới hạn"). TCB muốn chạy phải có SC đã
//!    `Configure`; passive server chạy bằng donation ở KT-21. Rig test KT-18/19 gắn một SC mặc
//!    định **test-only** cho TCB cần chạy (không đổi production, không nới kỳ vọng cũ).
//! 4. **Không dồn budget**: refill đặt `consumed = 0`, phần chưa dùng của chu kỳ cũ mất.
//! 5. Charge **chỉ** khi `Running`: chặn/ngủ/treo không bị trừ; `tick` không tự `dispatch`
//!    (dispatch vẫn là op tường minh, như KT-19).

use vifast_sim::kobj::{
    KError, Machine, ObjId, ObjType, ScInfo, SleepReason, SlotRef, SuspendMode, TcbState,
};
use vifast_sim::Lcg;

fn slot(cnode: ObjId, s: usize) -> SlotRef {
    SlotRef::new(cnode, s)
}

/// Bất biến scheduler (`check_sched`) + ngân sách sau **mỗi** thao tác công khai.
fn kiem(m: &Machine, buoc: &str) {
    if let Err(e) = m.check_sched() {
        panic!("check_sched sau {buoc}: {e}");
    }
}

// ---------------------------------------------------------------------------
// Rig: CNode 32 slot, 1 SchedControl, 1 Endpoint, 1 Reply, 3 SchedContext,
// 3 TCB (slot TCB 16..18). SC/TCB gắn bằng model API `set_sched_params` (D2-49(b)).
// ---------------------------------------------------------------------------

const S_CTL: usize = 0;
const S_UNTYPED: usize = 1;
const S_EP: usize = 2;
const S_REP: usize = 3;
const S_SC: [usize; 3] = [8, 9, 10];
const S_TCB: [usize; 3] = [16, 17, 18];

struct Rig {
    m: Machine,
    c0: ObjId,
    ctl: SlotRef,
    sc: [SlotRef; 3],
    ts: [SlotRef; 3],
    t: [ObjId; 3],
}

fn rig() -> Rig {
    let mut m = Machine::new();
    let c0 = m.add_cnode(32);
    let ctl = m.add_kernel_object(ObjType::SchedControl);
    let u0 = m.add_untyped(0x1000, 65536);
    let ep = m.add_kernel_object(ObjType::Endpoint);
    let rep = m.add_kernel_object(ObjType::Reply);
    for (i, obj) in [(S_CTL, ctl), (S_UNTYPED, u0), (S_EP, ep), (S_REP, rep)] {
        let cap = m.root_cap(obj);
        m.put_cap(c0, i, cap).expect("cap object vào CNode");
    }
    let mut sc = [SlotRef::new(0, 0); 3];
    let mut ts = [SlotRef::new(0, 0); 3];
    let mut t = [0u32; 3];
    for i in 0..3 {
        let sc_obj = m.add_kernel_object(ObjType::SchedContext);
        let cap = m.root_cap(sc_obj);
        m.put_cap(c0, S_SC[i], cap).expect("cap SchedContext");
        sc[i] = slot(c0, S_SC[i]);
    }
    for i in 0..3 {
        let tcb = m.add_kernel_object(ObjType::Tcb);
        let cap = m.root_cap(tcb);
        m.put_cap(c0, S_TCB[i], cap).expect("cap TCB");
        t[i] = tcb;
        ts[i] = slot(c0, S_TCB[i]);
    }
    Rig {
        m,
        c0,
        ctl: slot(c0, S_CTL),
        sc,
        ts,
        t,
    }
}

impl Rig {
    fn configure(&mut self, i: usize, budget: u64, period: u64) {
        self.m
            .sched_control_configure(self.ctl, self.sc[i], budget, period)
            .expect("SchedControl.Configure hợp lệ");
        kiem(&self.m, "SchedControl.Configure");
    }

    fn attach(&mut self, tcb: usize, sched: usize) {
        self.m
            .set_sched_params(self.ts[tcb], Some(self.sc[sched]))
            .expect("set_sched_params");
        kiem(&self.m, "set_sched_params");
    }

    fn detach(&mut self, tcb: usize) {
        self.m
            .set_sched_params(self.ts[tcb], None)
            .expect("set_sched_params(None)");
        kiem(&self.m, "set_sched_params(None)");
    }

    /// `Inactive → Ready` (cặp ALLOWED của bảng KT-18; enqueue qua một cửa KT-19).
    fn ready(&mut self, i: usize) {
        self.m
            .transition_tcb(self.t[i], TcbState::Ready)
            .expect("Inactive→Ready");
        kiem(&self.m, "transition Ready");
    }

    fn dispatch(&mut self) -> Option<ObjId> {
        let got = self.m.dispatch(0).expect("dispatch");
        kiem(&self.m, "dispatch");
        got
    }

    fn tick(&mut self, n: u64) {
        self.m.tick(n).expect("tick");
        kiem(&self.m, "tick");
    }

    fn info(&self, i: usize) -> ScInfo {
        self.m.sched_info(self.sc[i]).expect("sched_info")
    }

    fn state(&self, i: usize) -> TcbState {
        self.m.tcb_state(self.t[i]).expect("tcb_state")
    }

    fn prio(&mut self, i: usize, p: u8) {
        self.m
            .tcb_set_priority(self.ts[i], p, 255)
            .expect("priority");
        kiem(&self.m, "Tcb.SetPriority");
    }
}

fn sleep_budget() -> TcbState {
    TcbState::Sleeping {
        reason: SleepReason::Budget,
    }
}

// ---------------------------------------------------------------------------
// 1. Thời gian ảo `now` + `tick(n)` tất định; không đọc wall clock (DoD 1).
// ---------------------------------------------------------------------------

/// Một dòng trace của test tất định: `(now, consumed[3], refill[3], charged[3])`.
type TraceDong = (u64, [u64; 3], [u64; 3], [u64; 3]);

#[test]
fn thoi_gian_ao_tat_dinh_cung_kich_ban_cung_moc_refill() {
    fn script(seed: u64) -> Vec<TraceDong> {
        let mut r = rig();
        let mut rng = Lcg::new(seed);
        r.configure(0, 2, 5);
        r.attach(0, 0);
        r.ready(0);
        assert_eq!(r.dispatch(), Some(r.t[0]));
        let mut trace = Vec::new();
        let mut expected_now = 0u64;
        for _ in 0..24 {
            let n = rng.below(3); // 0, 1, 2
            r.tick(n);
            expected_now += n;
            assert_eq!(r.m.virtual_now(), expected_now);
            trace.push((
                r.m.virtual_now(),
                [r.info(0).consumed, r.info(1).consumed, r.info(2).consumed],
                [r.info(0).refill, r.info(1).refill, r.info(2).refill],
                [r.info(0).charged, r.info(1).charged, r.info(2).charged],
            ));
        }
        trace
    }
    for seed in 0..16 {
        assert_eq!(
            script(seed),
            script(seed),
            "seed {seed}: tick phải tất định"
        );
    }

    // `now` bắt đầu 0 và `tick(0)` không đổi gì.
    let mut r = rig();
    assert_eq!(r.m.virtual_now(), 0);
    r.tick(0);
    assert_eq!(r.m.virtual_now(), 0);
    r.tick(7);
    assert_eq!(r.m.virtual_now(), 7);
}

// ---------------------------------------------------------------------------
// 2. `SchedControl.Configure` kiểm `budget > 0`, `period > 0`, `budget ≤ period`;
//    cấu hình hợp lệ đặt `consumed = 0`, `refill` = biên kế tiếp (DoD 2).
// ---------------------------------------------------------------------------

#[test]
fn configure_tu_choi_budget_0_va_budget_lon_hon_period() {
    let mut r = rig();
    // SC chưa cấu hình: mọi giá trị 0, `configured = false`.
    let info = r.info(0);
    assert!(!info.configured, "SC mới phải chưa cấu hình");
    assert_eq!(
        (
            info.budget,
            info.period,
            info.consumed,
            info.refill,
            info.charged
        ),
        (0, 0, 0, 0, 0)
    );
    assert_eq!(r.m.sched_consumed(r.sc[0]), Ok(0));

    // Ba ca sai: budget = 0, period = 0, budget > period.
    assert_eq!(
        r.m.sched_control_configure(r.ctl, r.sc[0], 0, 5),
        Err(KError::InvalidArgument)
    );
    assert_eq!(
        r.m.sched_control_configure(r.ctl, r.sc[0], 5, 0),
        Err(KError::InvalidArgument)
    );
    assert_eq!(
        r.m.sched_control_configure(r.ctl, r.sc[0], 6, 5),
        Err(KError::InvalidArgument)
    );
    // Không phải SchedControl / không phải SchedContext.
    assert_eq!(
        r.m.sched_control_configure(r.sc[0], r.sc[0], 2, 5),
        Err(KError::WrongObjectType)
    );
    assert_eq!(
        r.m.sched_control_configure(r.ctl, r.ts[0], 2, 5),
        Err(KError::WrongObjectType)
    );
    // Ca sai **không** đổi trạng thái SC.
    assert!(!r.info(0).configured);
    assert_eq!(r.m.sched_consumed(r.sc[0]), Ok(0));
    assert_eq!(
        r.m.sched_remaining(r.sc[0]),
        Ok(0),
        "SC chưa cấu hình ⇒ không có budget (0), không phải vô hạn"
    );
    kiem(&r.m, "configure lỗi");

    // Cấu hình hợp lệ: budget 2, period 5 tại now = 0 ⇒ refill = 5.
    r.configure(0, 2, 5);
    let info = r.info(0);
    assert!(info.configured);
    assert_eq!((info.budget, info.period), (2, 5));
    assert_eq!((info.consumed, info.refill, info.charged), (0, 5, 0));
    assert_eq!(r.m.sched_remaining(r.sc[0]), Ok(2));
    assert_eq!(r.m.sched_consumed(r.sc[0]), Ok(0));

    // Sau vài tick, cấu hình lại ở now = 3, period 5 ⇒ refill = 5 (biên toàn cục),
    // consumed đặt lại 0 (không dồn budget — quy ước 4).
    r.attach(0, 0);
    r.ready(0);
    assert_eq!(r.dispatch(), Some(r.t[0]));
    r.tick(1);
    assert_eq!(r.m.sched_consumed(r.sc[0]), Ok(1));
    r.configure(0, 3, 5);
    let info = r.info(0);
    assert_eq!((info.budget, info.period), (3, 5));
    assert_eq!((info.consumed, info.refill), (0, 5));
    assert!(info.refill > r.m.virtual_now());
}

// ---------------------------------------------------------------------------
// 3. Charge **chỉ** khi `Running`: syscall/blocked/suspended không bị trừ (DoD 5).
// ---------------------------------------------------------------------------

#[test]
fn chi_charge_khi_running_qua_duong_ipc_that() {
    let mut r = rig();
    r.configure(0, 4, 16);
    r.attach(0, 0);
    // Server cũng cần SC đã Configure mới được `pick` (KT-20/reviewer mục 1).
    r.configure(1, 4, 16);
    r.attach(1, 1);
    r.prio(0, 10); // client cao hơn server
    r.prio(1, 5);
    r.ready(0);
    assert_eq!(r.dispatch(), Some(r.t[0]));
    r.tick(2);
    assert_eq!(r.m.sched_consumed(r.sc[0]), Ok(2));
    assert_eq!(r.state(0), TcbState::Running { hart: 0 });

    // `Call` không có người nhận ⇒ BlockedOnSend: không charge dù tick bao nhiêu.
    r.m.call(r.t[0], slot(r.c0, S_EP), 0x42).expect("Call");
    kiem(&r.m, "Call");
    assert!(matches!(r.state(0), TcbState::BlockedOnSend { .. }));
    r.tick(5);
    assert_eq!(
        r.m.sched_consumed(r.sc[0]),
        Ok(2),
        "TCB chặn không được charge"
    );
    assert_eq!(r.m.virtual_now(), 7);

    // Server `Recv` ⇒ client BlockedOnReply; vẫn không charge.
    r.m.recv(r.t[1], slot(r.c0, S_EP), Some(slot(r.c0, S_REP)))
        .expect("Recv");
    kiem(&r.m, "Recv");
    assert!(matches!(r.state(0), TcbState::BlockedOnReply { .. }));
    r.tick(3);
    assert_eq!(
        r.m.sched_consumed(r.sc[0]),
        Ok(2),
        "BlockedOnReply cũng không được charge"
    );
    assert_eq!(r.m.virtual_now(), 10);

    // Server (không SC, không giới hạn) chạy `Reply`; client về Ready rồi chạy tiếp.
    assert_eq!(
        r.dispatch(),
        Some(r.t[1]),
        "server mức 5 chạy khi client chặn"
    );
    r.m.reply(r.t[1], slot(r.c0, S_REP)).expect("Reply");
    kiem(&r.m, "Reply");
    assert_eq!(r.state(0), TcbState::Ready);
    assert_eq!(r.dispatch(), Some(r.t[0]), "client mức 10 thắng server");
    r.tick(1);
    assert_eq!(
        r.m.sched_consumed(r.sc[0]),
        Ok(3),
        "chạy lại sau Reply mới bị charge"
    );
}

// ---------------------------------------------------------------------------
// 4. Hết budget ⇒ `Running → Sleeping{Budget}` qua `transition_tcb`, gỡ khỏi
//    ready queue; `consumed == budget` đúng biên (không `budget + 1`) (DoD 6).
// ---------------------------------------------------------------------------

#[test]
fn het_budget_sang_sleeping_budget_va_go_khoi_hang() {
    let mut r = rig();
    r.configure(0, 2, 4);
    r.attach(0, 0);
    r.ready(0);
    assert_eq!(r.dispatch(), Some(r.t[0]));
    assert!(r.m.tcb_has_budget(r.t[0]));

    r.tick(1);
    assert_eq!(r.m.sched_consumed(r.sc[0]), Ok(1));
    assert_eq!(r.state(0), TcbState::Running { hart: 0 });

    r.tick(1);
    assert_eq!(
        r.m.sched_consumed(r.sc[0]),
        Ok(2),
        "charge đúng biên budget"
    );
    assert_eq!(
        r.state(0),
        sleep_budget(),
        "hết budget ⇒ Sleeping{{Budget}}"
    );
    assert!(!r.m.tcb_info(r.ts[0]).expect("TcbInfo").suspended);
    assert!(
        !r.m.ready_contains(0, r.t[0]),
        "Sleeping{{Budget}} không nằm ready queue"
    );
    assert_eq!(r.m.ready_len(0), 0);
    assert_eq!(r.m.current(0), None);
    assert!(r.m.sched_idle(0));
    assert!(!r.m.tcb_has_budget(r.t[0]));

    // Timer ở biên chu kỳ kế tiếp: refill = 4.
    let info = r.info(0);
    assert_eq!((info.consumed, info.refill), (2, 4));
    assert_eq!(r.m.virtual_now(), 2);

    // now 2 → 3: chưa tới biên, vẫn ngủ.
    r.tick(1);
    assert_eq!(r.state(0), sleep_budget());
    assert_eq!(r.m.sched_consumed(r.sc[0]), Ok(2));

    // now 3 → 4: biên refill ⇒ consumed = 0, Sleeping{Budget} → Ready + enqueue.
    r.tick(1);
    assert_eq!(r.m.virtual_now(), 4);
    assert_eq!(r.m.sched_consumed(r.sc[0]), Ok(0));
    assert_eq!(r.info(0).refill, 8, "biên kế tiếp = 8");
    assert_eq!(
        r.state(0),
        TcbState::Ready,
        "refill ⇒ Ready (không suspended)"
    );
    assert!(r.m.ready_contains(0, r.t[0]));
    assert!(r.m.tcb_has_budget(r.t[0]));
    assert_eq!(r.dispatch(), Some(r.t[0]));
    assert_eq!(r.state(0), TcbState::Running { hart: 0 });
}

// ---------------------------------------------------------------------------
// 5. Không bao giờ charge quá `budget`; không tick nào "chạy" khi `consumed == budget`
//    (DoD 8 — phần biên).
// ---------------------------------------------------------------------------

#[test]
fn khong_charge_qua_budget_va_khong_chay_khi_da_het() {
    let mut r = rig();
    r.configure(0, 3, 7);
    r.attach(0, 0);
    r.ready(0);
    r.dispatch();
    let mut max = 0;
    for _ in 0..30 {
        r.tick(1);
        let info = r.info(0);
        assert!(info.consumed <= info.budget, "consumed vượt budget");
        max = max.max(info.consumed);
        if info.consumed == info.budget {
            assert_eq!(
                r.state(0),
                sleep_budget(),
                "đã hết budget thì không được Running/Ready"
            );
            assert!(!r.m.ready_contains(0, r.t[0]));
        }
        // `refill` luôn ở tương lai sau mỗi tick (biên đã xử lý xong).
        assert!(info.refill > r.m.virtual_now(), "refill phải > now");
    }
    assert_eq!(max, 3, "phải chạm đúng budget, không vượt");
    assert_eq!(r.info(0).charged, 3, "tổng charge = budget một chu kỳ");
    kiem(&r.m, "30 tick");
}

// ---------------------------------------------------------------------------
// 6. Refill: `Sleeping{Budget}` + `!suspended` → `Ready`; `+suspended` giữ
//    `Sleeping{Budget}`; `Resume` không phát lại gì (DoD 7, bảng §4.5).
// ---------------------------------------------------------------------------

#[test]
fn refill_ton_trong_suspended_va_resume_khong_phat_lai() {
    let mut r = rig();
    r.configure(0, 2, 4);
    r.attach(0, 0);
    r.ready(0);
    r.dispatch();
    r.tick(2);
    assert_eq!(r.state(0), sleep_budget());

    // Suspend(Abort) trên Sleeping{Budget}: giữ Sleeping + bật cờ (bảng §4.5).
    r.m.tcb_suspend(r.ts[0], SuspendMode::Abort)
        .expect("Suspend");
    kiem(&r.m, "Suspend khi Sleeping{{Budget}}");
    assert_eq!(r.state(0), sleep_budget());
    assert!(r.m.tcb_info(r.ts[0]).expect("TcbInfo").suspended);
    assert!(!r.m.ready_contains(0, r.t[0]));

    // Tới biên 4: `consumed` về 0 nhưng TCB **vẫn** Sleeping vì suspended.
    r.tick(2);
    assert_eq!(r.m.virtual_now(), 4);
    assert_eq!(r.m.sched_consumed(r.sc[0]), Ok(0));
    assert_eq!(
        r.state(0),
        sleep_budget(),
        "suspended thì refill không đánh thức"
    );
    assert!(!r.m.ready_contains(0, r.t[0]));

    // Resume: chỉ xoá cờ, không phát lại, vẫn chờ biên kế tiếp (8).
    r.m.tcb_resume(r.ts[0]).expect("Resume");
    kiem(&r.m, "Resume");
    assert_eq!(r.state(0), sleep_budget(), "Resume không đánh thức");
    assert!(!r.m.ready_contains(0, r.t[0]));

    // Biên 8: refill đánh thức ⇒ Ready + enqueue.
    r.tick(4);
    assert_eq!(r.m.virtual_now(), 8);
    assert_eq!(r.state(0), TcbState::Ready);
    assert!(r.m.ready_contains(0, r.t[0]));
    assert_eq!(r.m.sched_consumed(r.sc[0]), Ok(0));
    assert_eq!(r.info(0).refill, 12);
}

// ---------------------------------------------------------------------------
// 7. `pick`/`dispatch` **chỉ** chọn TCB có SC còn budget: TCB Ready nhưng hết
//    budget bị bỏ qua; refill xong lại được chọn (follow-up KT-19, DoD 4).
// ---------------------------------------------------------------------------

#[test]
fn pick_bo_qua_tcb_ready_het_budget_va_nhan_lai_sau_refill() {
    let mut r = rig();
    r.configure(0, 1, 4);
    r.attach(0, 0);
    r.ready(0);
    assert_eq!(r.dispatch(), Some(r.t[0]));
    r.tick(1);
    assert_eq!(r.state(0), sleep_budget());
    assert_eq!(r.m.sched_consumed(r.sc[0]), Ok(1));

    // Đánh thức sớm (model API `wake`): Ready + nằm trong hàng nhưng **hết** budget.
    r.m.wake(r.t[0]).expect("wake");
    kiem(&r.m, "wake TCB hết budget");
    assert_eq!(r.state(0), TcbState::Ready);
    assert!(r.m.ready_contains(0, r.t[0]));
    assert!(!r.m.tcb_has_budget(r.t[0]));
    assert_eq!(r.m.sched_pick(), None, "pick phải bỏ qua TCB hết budget");
    assert_eq!(
        r.dispatch(),
        None,
        "dispatch không được chọn TCB hết budget"
    );
    assert_eq!(r.m.current(0), None);
    assert_eq!(
        r.m.ready_len(0),
        1,
        "TCB vẫn Ready trong hàng, chỉ không được chọn"
    );

    // TCB **không SC** cũng không được chọn (reviewer-kt20 mục 1) — khác "không giới hạn".
    r.ready(1);
    assert!(!r.m.tcb_has_budget(r.t[1]), "không SC ⇒ không có budget");
    assert_eq!(r.m.sched_pick(), None, "không SC cũng bị bỏ qua");
    assert_eq!(r.dispatch(), None);

    // now 1 → 4: refill SC0 ⇒ TCB hết budget trở lại hàng chọn được (t[1] vẫn bị bỏ qua).
    r.tick(3);
    assert_eq!(r.m.virtual_now(), 4);
    assert_eq!(r.m.sched_consumed(r.sc[0]), Ok(0));
    assert!(r.m.tcb_has_budget(r.t[0]));
    assert!(r.m.ready_contains(0, r.t[0]));
    assert_eq!(r.m.sched_pick(), Some(r.t[0]));
    assert_eq!(r.dispatch(), Some(r.t[0]));
    assert_eq!(r.m.ready_len(0), 1, "t[1] vẫn Ready nhưng không có budget");
}

// ---------------------------------------------------------------------------
// 8. TCB **không SC** hoặc SC chưa `Configure` ⇒ **không có budget ⇒ không chạy**:
//    `pick`/`dispatch` bỏ qua; sau `Configure` mới chạy được; gỡ SC khi đang `Running`
//    ⇒ ngủ ngay (reviewer-kt20 mục 1 — sửa theo review, không phải "không giới hạn").
// ---------------------------------------------------------------------------

#[test]
fn tcb_khong_sc_hoac_sc_chua_configure_khong_chay() {
    let mut r = rig();
    // (a) TCB không SC: `Ready` vẫn nằm trong hàng (bất biến KT-19) nhưng không được pick.
    r.ready(0);
    assert_eq!(r.state(0), TcbState::Ready);
    assert!(r.m.ready_contains(0, r.t[0]));
    assert!(!r.m.tcb_has_budget(r.t[0]), "không SC ⇒ không có budget");
    assert_eq!(r.m.sched_pick(), None, "không SC ⇒ không được chọn");
    assert_eq!(r.dispatch(), None);
    assert_eq!(r.m.current(0), None);
    r.tick(100);
    assert_eq!(r.state(0), TcbState::Ready, "tick không làm nó chạy");
    assert_eq!(r.m.current(0), None);
    assert_eq!(r.m.virtual_now(), 100);

    // (b) TCB có SC nhưng SC **chưa** `Configure`: cũng không chạy, không charge.
    r.m.set_sched_params(r.ts[1], Some(r.sc[1]))
        .expect("set_sched_params SC chưa cấu hình");
    kiem(&r.m, "set_sched_params SC chưa cấu hình");
    r.ready(1);
    assert!(!r.info(1).configured);
    assert!(!r.m.tcb_has_budget(r.t[1]));
    assert_eq!(r.m.sched_pick(), None);
    assert_eq!(r.dispatch(), None);
    r.tick(10);
    assert_eq!(r.state(1), TcbState::Ready);
    assert_eq!(r.info(1).charged, 0);

    // (c) Sau `Configure`, SC mới có budget ⇒ chạy được.
    r.configure(1, 2, 8);
    assert!(r.m.tcb_has_budget(r.t[1]));
    assert_eq!(r.m.sched_pick(), Some(r.t[1]));
    assert_eq!(r.dispatch(), Some(r.t[1]));
    r.tick(1);
    assert_eq!(r.m.sched_consumed(r.sc[1]), Ok(1));
    assert_eq!(r.state(1), TcbState::Running { hart: 0 });

    // (d) Gỡ SC khỏi TCB đang `Running` ⇒ không còn budget ⇒ ngủ ngay.
    r.detach(1);
    assert_eq!(r.state(1), sleep_budget());
    assert_eq!(r.m.current(0), None);
    assert!(!r.m.ready_contains(0, r.t[1]));
    assert!(!r.m.tcb_has_budget(r.t[1]));
}

// ---------------------------------------------------------------------------
// 9. `yield_current()` không làm mất budget/thời gian; budget còn lại đọc được qua
//    `sched_remaining` (DoD 9 — không đổi API KT-19).
// ---------------------------------------------------------------------------

#[test]
fn yield_khong_mat_budget_va_thoi_gian() {
    let mut r = rig();
    r.configure(0, 5, 10);
    r.attach(0, 0);
    r.ready(0);
    r.dispatch();
    r.tick(2);
    assert_eq!(r.m.sched_remaining(r.sc[0]), Ok(3));
    let now_truoc = r.m.virtual_now();
    let charged_truoc = r.info(0).charged;

    assert_eq!(r.m.yield_current(0).expect("yield"), Some(r.t[0]));
    kiem(&r.m, "yield");
    assert_eq!(r.m.virtual_now(), now_truoc, "yield không trôi thời gian");
    assert_eq!(r.info(0).charged, charged_truoc, "yield không charge");
    assert_eq!(r.m.sched_consumed(r.sc[0]), Ok(2));
    assert_eq!(r.m.sched_remaining(r.sc[0]), Ok(3));
    assert_eq!(r.state(0), TcbState::Running { hart: 0 });

    // Có TCB thứ hai cùng mức: yield chạy TCB kia; budget SC của t[0] giữ nguyên.
    r.configure(1, 4, 8);
    r.attach(1, 1);
    r.ready(1);
    assert_eq!(r.m.yield_current(0).expect("yield"), Some(r.t[1]));
    assert_eq!(r.state(0), TcbState::Ready);
    assert_eq!(
        r.m.sched_consumed(r.sc[0]),
        Ok(2),
        "yield không charge SC t[0]"
    );
    r.tick(1);
    assert_eq!(
        r.m.sched_consumed(r.sc[1]),
        Ok(1),
        "t[1] chạy ⇒ charge SC t[1]"
    );
    assert_eq!(r.m.sched_consumed(r.sc[0]), Ok(2));
    // Yield lại ⇒ t[0] chạy tiếp, budget còn 2 (không mất).
    assert_eq!(r.m.yield_current(0).expect("yield"), Some(r.t[0]));
    r.tick(1);
    assert_eq!(r.m.sched_consumed(r.sc[0]), Ok(3));
    assert_eq!(r.m.sched_remaining(r.sc[0]), Ok(2));
}

// ---------------------------------------------------------------------------
// 10. Gắn SC **đã hết** budget cho TCB đang `Running` ⇒ ngủ ngay (Running → Sleeping
//     là cặp ALLOWED; không để lại `Running` hết budget); nhiều TCB cùng SC được đánh
//     thức đủ ở biên refill (mô hình K1b chưa cấm chia SC — KT-21 siết).
// ---------------------------------------------------------------------------

#[test]
fn gan_sc_het_budget_cho_tcb_dang_chay_thi_ngu_ngay() {
    let mut r = rig();
    r.configure(0, 1, 4);
    r.attach(0, 0);
    r.ready(0);
    r.dispatch();
    r.tick(1);
    assert_eq!(r.state(0), sleep_budget());
    assert_eq!(r.m.sched_consumed(r.sc[0]), Ok(1));

    // t[1] chạy bằng SC riêng đã Configure, rồi gắn SC đã cạn ⇒ phải ngủ ngay, không để
    // `Running` mà hết budget (reviewer-kt20 mục 1: không SC/không budget ⇒ không chạy).
    r.configure(1, 4, 8);
    r.attach(1, 1);
    r.ready(1);
    assert_eq!(r.dispatch(), Some(r.t[1]));
    r.m.set_sched_params(r.ts[1], Some(r.sc[0]))
        .expect("set_sched_params SC đã cạn");
    kiem(&r.m, "gắn SC đã cạn");
    assert_eq!(r.state(1), sleep_budget());
    assert_eq!(r.m.current(0), None);
    assert!(!r.m.ready_contains(0, r.t[1]));

    // Biên 4: cả hai TCB dùng chung SC được đánh thức.
    r.tick(3);
    assert_eq!(r.m.virtual_now(), 4);
    assert_eq!(r.state(0), TcbState::Ready);
    assert_eq!(r.state(1), TcbState::Ready);
    assert!(r.m.ready_contains(0, r.t[0]));
    assert!(r.m.ready_contains(0, r.t[1]));
    kiem(&r.m, "refill đánh thức hai TCB");
}

// ---------------------------------------------------------------------------
// 11. Gỡ SC khỏi TCB đang `Sleeping{Budget}`: không còn biên refill nào đánh thức
//     (không rò timer); `wake` thủ công đưa `Ready`, và không SC thì không giới hạn.
// ---------------------------------------------------------------------------

#[test]
fn go_sc_khi_sleeping_budget_khong_con_bien_refill() {
    let mut r = rig();
    r.configure(0, 1, 4);
    r.attach(0, 0);
    r.ready(0);
    r.dispatch();
    r.tick(1);
    assert_eq!(r.state(0), sleep_budget());

    r.detach(0);
    r.tick(10);
    assert_eq!(
        r.state(0),
        sleep_budget(),
        "không SC ⇒ không refill đánh thức"
    );
    assert!(
        !r.m.ready_contains(0, r.t[0]),
        "Sleeping{{Budget}} không nằm trong hàng"
    );

    r.m.wake(r.t[0]).expect("wake");
    // Wake đưa về `Ready` nhưng **không** SC ⇒ vẫn không được chọn (reviewer mục 1).
    r.m.wake(r.t[0]).expect("wake");
    kiem(&r.m, "wake sau khi gỡ SC");
    assert_eq!(r.state(0), TcbState::Ready);
    assert!(!r.m.tcb_has_budget(r.t[0]), "đã gỡ SC ⇒ không có budget");
    assert_eq!(r.m.sched_pick(), None);
    assert_eq!(r.dispatch(), None);

    // Gắn lại SC đã `Configure` (SC còn sống, vẫn refill theo biên dù TCB bị gỡ) ⇒ chạy lại.
    r.attach(0, 0);
    assert!(r.m.tcb_has_budget(r.t[0]));
    assert_eq!(r.dispatch(), Some(r.t[0]));
    r.tick(1);
    // Charge 1 tick làm cạn budget ⇒ `Sleeping{Budget}`, nhưng đúng lúc đó chạm biên 12 nên
    // refill cùng tick đánh thức lại `Ready` (charge trước — refill sau).
    assert_eq!(r.m.sched_consumed(r.sc[0]), Ok(0));
    assert_eq!(r.m.virtual_now(), 12);
    assert_eq!(r.state(0), TcbState::Ready, "refill cùng tick ⇒ Ready");
    assert_eq!(
        r.info(0).charged,
        2,
        "1 tick trước khi gỡ SC + 1 tick sau khi gắn lại"
    );
}

// ---------------------------------------------------------------------------
// 12. Invariant budget: 1.000 seed × 64 bước ngẫu nhiên (stable; Miri giảm seed/bước,
//     `cfg(miri)`) — mỗi SC, **mỗi chu kỳ thẳng hàng** `[k·period, (k+1)·period)` tổng
//     tick chạy ≤ `budget`; `consumed ≤ budget`; không tick nào chạy khi đã cạn
//     (DoD 8).
// ---------------------------------------------------------------------------

#[cfg(miri)]
const N_SEED: u64 = 5;
#[cfg(not(miri))]
const N_SEED: u64 = 1000;
#[cfg(miri)]
const N_STEP: u64 = 16;
#[cfg(not(miri))]
const N_STEP: u64 = 64;

#[test]
fn invariant_budget_moi_chu_ky_thang_hang_khong_vuot() {
    use std::collections::HashMap;

    let mut tong_charge = 0u64;
    let mut so_lan_can = 0u64;
    let mut so_lan_ngu = 0u64;
    let mut so_cua_so = 0u64;

    for seed in 0..N_SEED {
        let mut r = rig();
        let mut rng = Lcg::new(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ 0x20);
        // Cấu hình 3 SC (budget ≤ period) ở now = 0 ⇒ mọi biên là bội số của period.
        let mut budget = [0u64; 3];
        let mut period = [0u64; 3];
        for i in 0..3 {
            budget[i] = 1 + rng.below(3);
            period[i] = budget[i] + rng.below(4);
            r.configure(i, budget[i], period[i]);
            r.attach(i, i);
            r.prio(i, 5 + (rng.below(3) as u8));
            r.ready(i);
        }
        r.dispatch();

        // (SC, cửa sổ thẳng hàng) → số tick đã charge; dùng `charged` đơn điệu.
        let mut cua_so: HashMap<(usize, u64), u64> = HashMap::new();

        for _ in 0..N_STEP {
            let op = rng.below(100);
            match op {
                0..=44 => {
                    let now = r.m.virtual_now();
                    let truoc: [u64; 3] = [r.info(0).charged, r.info(1).charged, r.info(2).charged];
                    r.tick(1);
                    for i in 0..3 {
                        let delta = r.info(i).charged - truoc[i];
                        assert!(delta <= 1, "một tick chỉ charge tối đa 1 (SC {i} +{delta})");
                        if delta == 1 {
                            let w = now / period[i];
                            let cu = *cua_so.get(&(i, w)).unwrap_or(&0);
                            assert!(
                                cu < budget[i],
                                "SC {i} cửa sổ {w}: {cu}+1 > budget {} (seed {seed})",
                                budget[i]
                            );
                            cua_so.insert((i, w), cu + 1);
                            tong_charge += 1;
                        }
                    }
                }
                45..=59 => {
                    r.dispatch();
                }
                60..=67 => {
                    if r.m.current(0).is_some() {
                        let _ = r.m.yield_current(0);
                    } else {
                        r.dispatch();
                    }
                }
                68..=74 => {
                    let i = rng.below(3) as usize;
                    let _ = r.m.wake(r.t[i]);
                }
                75..=80 => {
                    let i = rng.below(3) as usize;
                    let _ = r.m.tcb_suspend(r.ts[i], SuspendMode::Abort);
                }
                81..=86 => {
                    let i = rng.below(3) as usize;
                    let _ = r.m.tcb_resume(r.ts[i]);
                }
                87..=91 => {
                    let i = rng.below(3) as usize;
                    let j = rng.below(3) as usize;
                    let _ = r.m.set_sched_params(r.ts[i], Some(r.sc[j]));
                }
                92..=95 => {
                    let i = rng.below(3) as usize;
                    let _ = r.m.set_sched_params(r.ts[i], None);
                }
                _ => {
                    let _ = r.m.block_current(
                        0,
                        TcbState::Sleeping {
                            reason: SleepReason::Timeout,
                        },
                    );
                }
            }
            kiem(&r.m, "bước ngẫu nhiên");

            // Bất biến ngân sách sau mỗi bước.
            for i in 0..3 {
                let info = r.info(i);
                assert!(info.configured, "SC {i} phải còn cấu hình");
                assert!(info.budget > 0 && info.period > 0 && info.budget <= info.period);
                assert!(
                    info.consumed <= info.budget,
                    "SC {i}: consumed {} > budget {} (seed {seed})",
                    info.consumed,
                    info.budget
                );
                assert!(
                    info.refill > r.m.virtual_now(),
                    "SC {i}: refill {} không ở tương lai (now {})",
                    info.refill,
                    r.m.virtual_now()
                );
                if info.consumed == info.budget {
                    so_lan_can += 1;
                }
            }
            for i in 0..3 {
                let state = r.state(i);
                if matches!(
                    state,
                    TcbState::Sleeping {
                        reason: SleepReason::Budget
                    }
                ) {
                    so_lan_ngu += 1;
                    assert!(
                        !r.m.ready_contains(0, r.t[i]),
                        "Sleeping{{Budget}} trong hàng"
                    );
                }
                if matches!(state, TcbState::Running { .. }) {
                    // Running ⇒ SC (nếu có) còn budget: không chạy khi đã cạn.
                    assert!(r.m.tcb_has_budget(r.t[i]), "Running mà SC đã cạn");
                }
            }
        }
        so_cua_so += cua_so.len() as u64;
    }
    assert!(tong_charge > 0, "test phải có tick được charge");
    assert!(so_lan_can > 0, "test phải chạm biên hết budget");
    assert!(so_lan_ngu > 0, "test phải có TCB Sleeping{{Budget}}");
    assert!(so_cua_so > 0, "test phải trải qua cửa sổ thẳng hàng");
}
