// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-23 — **tích hợp K1b**: bước kịch bản TCB/scheduler, interleaver tôn trọng lịch,
//! 1.000 seed × 64 bước LCG trên mô hình TCB + scheduler 1 hart + MCS budget/donation + hộp
//! thư, phủ **46/46 cặp chuyển trạng thái** (100%) + **73/73 cặp cấm** bị thử, và chốt DoD K1b.
//!
//! **Model-only `[TK]`, không phải spec/ABI.** Nguồn: K1B-PLAN §5 KT-23; **D2-49** (1 hart,
//! hoãn push migration K1c; model API; donation tự động; bảng `ALLOWED` model-only; budget chu
//! kỳ thẳng hàng; tick ảo), **D2-50** (không SC ⇒ **không chạy**, trừ donation), **D2-51** (kẹp
//! MCP, reply chỉ trả donation khi đúng donor), **D2-52** (chỉ sửa fixture 12), **Q7** (parity
//! 10k chỉ phủ Untyped/CNode — **không** phải oracle TCB/scheduler).
//!
//! Test đỏ trước (D2-05) — xem `JOURNAL/kt23-red-compile.log` (bước `TcbSuspend/Resume/
//! TcbReadState/SchedTick/SchedPick`, `K1bReport`, `check_budget/check_sc_refs` chưa có ⇒
//! E0599/E0004) và `JOURNAL/kt23-red-interleave.log` (interleaver KT-09 chọn thread theo trạng
//! thái TCB, **không** theo ready queue/budget ⇒ bế tắc trên kịch bản TCB `Ready`).
//!
//! Quy ước mô hình của test:
//! 1. Sau **mỗi** bước kiểm **bốn** miền bất biến: `check_sched`, `check_budget`,
//!    `check_sc_refs` và `check_all_bytes` (DoD 2) — vi phạm là panic **ngay**; bộ đếm vi phạm
//!    của test phải là **0** khi in.
//! 2. `forbidden_attempts` đếm ở **một cửa** `transition_tcb` (cặp `.` của bảng `ALLOWED`);
//!    mọi cặp cấm trong ma trận phải trả `IllegalOperation` và **không** đổi trạng thái.
//! 3. Coverage **46/46** là 48 cặp của bảng trừ **2 cặp `Stopping` reserved** cho K1c
//!    ([`vifast_sim::tcb::coverage_design`]); 2 cặp reserved vẫn được đi qua cổng trong ma trận
//!    (đếm riêng `hit_table` = 48), **không** hạ tiêu chí 46.
//! 4. Interleaver chỉ chọn bước của TCB **chạy được** (Running, hoặc Ready trong ready queue và
//!    còn budget, D2-50); bước quản trị (suspend/resume/read/tick/pick/set-params) luôn chọn được.

use vifast_sim::kobj::{
    KError, Machine, ObjId, ObjType, ScInfo, SleepReason, SlotRef, SuspendMode, TcbInfo, TcbState,
    ALLOWED, TCB_STATE_COUNT,
};
use vifast_sim::mailbox::{MailboxMsg, MAILBOX_CAP};
use vifast_sim::scenario::{run_interleaved, ScenarioError, ScenarioStep, Step};
use vifast_sim::tcb::coverage_design;
use vifast_sim::Lcg;

fn slot(cnode: ObjId, s: usize) -> SlotRef {
    SlotRef::new(cnode, s)
}

fn sleep_budget() -> TcbState {
    TcbState::Sleeping {
        reason: SleepReason::Budget,
    }
}

// ===========================================================================
// 1. Ma trận 11×11: 46/46 cặp thiết kế + 73/73 cặp cấm (DoD 3)
// ===========================================================================

/// `ObjId` đại diện trong trạng thái chặn — ma trận chỉ cần **hình dạng** trạng thái, các id
/// này không cần là object thật (`transition_tcb` không kiểm payload; KT-18 làm y hệt).
struct Ids {
    ep: ObjId,
    rep: ObjId,
    ntfn: ObjId,
    frame: ObjId,
}

fn ids() -> Ids {
    Ids {
        ep: 1,
        rep: 2,
        ntfn: 3,
        frame: 4,
    }
}

/// `TcbState` ứng với **chỉ số** của bảng `ALLOWED` (thứ tự khớp `vifast_abi::TCB_STATES`).
fn state_for(index: usize, ids: &Ids) -> TcbState {
    match index {
        0 => TcbState::Inactive,
        1 => TcbState::Ready,
        2 => TcbState::Running { hart: 0 },
        3 => TcbState::BlockedOnSend {
            ep: ids.ep,
            call: true,
            fault: None,
        },
        4 => TcbState::BlockedOnRecv {
            ep: ids.ep,
            reply: None,
        },
        5 => TcbState::BlockedOnReply { reply: ids.rep },
        6 => TcbState::BlockedOnNotification { ntfn: ids.ntfn },
        7 => TcbState::BlockedOnAddress {
            frame: ids.frame,
            offset: 16,
        },
        8 => sleep_budget(),
        9 => TcbState::Stopping,
        10 => TcbState::Dead,
        _ => panic!("chỉ số trạng thái ngoài 0..11"),
    }
}

/// Đưa một TCB **mới** từ `Inactive` tới `index` chỉ bằng các cặp `ALLOWED` (đúng đường của
/// bảng, không cửa sau) — giống KT-18; đây là cách duy nhất chạm `Stopping` ở K1b.
fn drive_to(m: &mut Machine, tcb: ObjId, index: usize, ids: &Ids) {
    let path: &[usize] = match index {
        0 => &[],
        1 => &[1],
        2 => &[1, 2],
        3 => &[1, 2, 3],
        4 => &[1, 2, 4],
        5 => &[1, 2, 5],
        6 => &[1, 2, 6],
        7 => &[1, 2, 7],
        8 => &[1, 2, 8],
        9 => &[1, 9],
        10 => &[10],
        _ => panic!("chỉ số trạng thái ngoài 0..11"),
    };
    for step in path {
        m.transition_tcb(tcb, state_for(*step, ids))
            .expect("đường dẫn trạng thái của bảng phải ALLOWED");
    }
}

/// **DoD 3** — mọi cặp `ALLOWED` có hit; mọi cặp **cấm** bị thử ít nhất một lần, trả
/// `IllegalOperation`, **không** đổi trạng thái; `k1b_report` cho `hit == allowed == 46`,
/// `forbidden_pairs_attempted == forbidden_pairs_total == 73`, `forbidden_attempts > 0`.
#[test]
fn ma_tran_46_cap_va_73_cap_cam() {
    let ids = ids();
    let mut m = Machine::new();
    // SC mặc định **test-only** cho mọi TCB của ma trận (D2-50): TCB vào `Running` phải có
    // ngân sách, nếu không `check_budget` (đúng) sẽ đỏ ở report cuối.
    let c0 = m.add_cnode(8);
    let ctl = m.add_kernel_object(ObjType::SchedControl);
    let sc = m.add_kernel_object(ObjType::SchedContext);
    for (s, obj) in [(0usize, ctl), (1usize, sc)] {
        let cap = m.root_cap(obj);
        m.put_cap(c0, s, cap).expect("cap ma trận");
    }
    let ctl_s = slot(c0, 0);
    let sc_s = slot(c0, 1);
    m.sched_control_configure(ctl_s, sc_s, 1000, 10_000)
        .expect("SC mặc định test-only");

    let mut allowed_ok = 0usize;
    let mut forbidden_ok = 0usize;
    for (from, row) in ALLOWED.iter().enumerate() {
        for (to, is_allowed) in row.iter().enumerate() {
            let tcb = m.add_kernel_object(ObjType::Tcb);
            let tcb_s = slot(c0, 2);
            let cap = m.root_cap(tcb);
            m.put_cap(c0, 2, cap).expect("cap TCB ma trận");
            m.set_sched_params(tcb_s, Some(sc_s))
                .expect("SC mặc định (test-only)");
            drive_to(&mut m, tcb, from, &ids);
            let before = m.tcb_state(tcb).expect("trạng thái nguồn");
            let target = state_for(to, &ids);
            let got = m.transition_tcb(tcb, target);
            if *is_allowed {
                allowed_ok += 1;
                assert!(
                    got.is_ok(),
                    "cặp ALLOWED ({from}->{to}) phải Ok, nhận {got:?}"
                );
                assert_eq!(m.tcb_state(tcb), Ok(target), "({from}->{to}) phải tới đích");
                assert!(
                    m.pair_hit_count(from, to) >= 1,
                    "cặp ALLOWED ({from}->{to}) phải có hit"
                );
            } else {
                forbidden_ok += 1;
                assert_eq!(
                    got,
                    Err(KError::IllegalOperation),
                    "cặp cấm ({from}->{to}) phải IllegalOperation"
                );
                assert_eq!(
                    m.tcb_state(tcb),
                    Ok(before),
                    "cặp cấm ({from}->{to}) **không** được đổi trạng thái"
                );
                assert_eq!(
                    m.pair_hit_count(from, to),
                    0,
                    "cặp cấm ({from}->{to}) không được đếm hit"
                );
                assert_eq!(
                    m.forbidden_attempt_count(from, to),
                    1,
                    "cặp cấm ({from}->{to}) phải được đếm đúng 1 lần thử"
                );
            }
            // Slot tạm của ma trận: gỡ cap để không tái dùng slot (mỗi cặp một TCB).
            let _ = m.delete(tcb_s);
        }
    }
    assert_eq!(allowed_ok, 48, "bảng phải có 48 cặp ALLOWED");
    assert_eq!(forbidden_ok, 73, "bảng 11×11 phải có 121 − 48 = 73 cặp cấm");

    let r = m.k1b_report();
    assert_eq!(
        r.transition.allowed, 46,
        "46 cặp suy ra từ thiết kế (48 − 2 reserved)"
    );
    assert_eq!(r.transition.reserved, 2, "đúng 2 cặp `Stopping` reserved");
    assert_eq!(
        r.transition.hit, r.transition.allowed,
        "coverage thiết kế phải 46/46 = 100%, không hạ tiêu chí"
    );
    assert_eq!(
        r.transition.hit_table, 48,
        "ma trận đi cả 2 cặp reserved qua cổng ⇒ toàn bảng 48/48"
    );
    assert_eq!(r.transition.table_allowed, 48);
    assert_eq!(
        r.transition.forbidden_pairs_total, 73,
        "tổng cặp cấm phải là 73"
    );
    assert_eq!(
        r.transition.forbidden_pairs_attempted, 73,
        "MỌI cặp cấm phải được thử ít nhất một lần (thiếu cặp ⇒ test đỏ)"
    );
    assert!(
        r.transition.forbidden_attempts >= 73,
        "forbidden_attempts phải > 0 (thực tế {})",
        r.transition.forbidden_attempts
    );
    assert_eq!(
        r.sched_violations
            + r.sc_ref_violations
            + r.budget_violations
            + r.mailbox_violations
            + r.byte_violations,
        0,
        "bất biến phải sạch: {r:?}"
    );
    println!(
        "# KT-23 ma trận: transitions hit/allowed = {}/{} (bảng {}/{}, reserved {}), \
         forbidden_attempts={} cặp {}/{}",
        r.transition.hit,
        r.transition.allowed,
        r.transition.hit_table,
        r.transition.table_allowed,
        r.transition.reserved,
        r.transition.forbidden_attempts,
        r.transition.forbidden_pairs_attempted,
        r.transition.forbidden_pairs_total
    );
}

/// Ca cấm viết **từ lý do thiết kế** (không đọc bảng ra kỳ vọng) — chống test tự chứng minh
/// vòng tròn: `Inactive → Running` phải qua `Ready`; `Sleeping → Running` phải refill rồi `pick`;
/// `Dead` là trạng thái cuối; `Ready → Sleeping` chỉ `Running` hết budget mới ngủ.
#[test]
fn cap_cam_theo_thiet_ke_bi_tu_choi() {
    let ids = ids();
    let mut m = Machine::new();
    for (from, to, ly_do) in [
        (0usize, 2usize, "Inactive→Running phải qua Ready"),
        (8, 2, "Sleeping→Running phải qua Ready sau refill"),
        (10, 1, "Dead không ra được"),
        (1, 8, "chỉ Running hết budget mới Sleeping"),
    ] {
        let tcb = m.add_kernel_object(ObjType::Tcb);
        drive_to(&mut m, tcb, from, &ids);
        let before = m.tcb_state(tcb).expect("trạng thái nguồn");
        assert_eq!(
            m.transition_tcb(tcb, state_for(to, &ids)),
            Err(KError::IllegalOperation),
            "{ly_do}"
        );
        assert_eq!(m.tcb_state(tcb), Ok(before), "{ly_do}: state không đổi");
    }
    // 4 cặp cấm vừa thử đều được đếm (nguồn của `forbidden_attempts`).
    assert_eq!(m.forbidden_attempt_count(0, 2), 1);
    assert_eq!(m.forbidden_attempt_count(8, 2), 1);
    assert_eq!(m.forbidden_attempt_count(10, 1), 1);
    assert_eq!(m.forbidden_attempt_count(1, 8), 1);
}

// ===========================================================================
// 2. Interleaver tôn trọng scheduler (DoD 1)
// ===========================================================================

/// Thiết lập: `c0` 128 slot; `ep0`, `rep0`, 2 SC đã `Configure`, 2 TCB đều có SC riêng và
/// `Ready` trước khi chạy kịch bản.
///
/// Vì sao server **có** SC ở đây (khác ca passive của KT-21): interleaver chỉ chạy bước của TCB
/// có ngân sách (D2-50); server passive **chưa** từng `Recv` thì `Call` của client chỉ xếp hàng
/// chứ không đánh thức nó ⇒ "bế tắc" **đúng ngữ nghĩa**, không phải lỗi interleaver. Ca server
/// passive chạy bằng donation được kiểm ở `kt21_donation` và ở fuzz (TCB 6..8 không SC).
struct ScRig {
    m: Machine,
    ep0_s: SlotRef,
    rep0_s: SlotRef,
    t0: ObjId,
    t1: ObjId,
    t0_s: SlotRef,
    t1_s: SlotRef,
}

/// Slot cố định của rig kịch bản.
const S_EP0: usize = 0;
const S_REP0: usize = 1;
const S_CTL: usize = 2;
const S_SC0: usize = 8;
const S_SC1: usize = 9;
const S_T0: usize = 16;
const S_T1: usize = 17;

fn sc_rig() -> ScRig {
    let mut m = Machine::new();
    let c0 = m.add_cnode(128);
    let ep0 = m.add_kernel_object(ObjType::Endpoint);
    let rep0 = m.add_kernel_object(ObjType::Reply);
    let ctl = m.add_kernel_object(ObjType::SchedControl);
    let sc0 = m.add_kernel_object(ObjType::SchedContext);
    let sc1 = m.add_kernel_object(ObjType::SchedContext);
    for (s, obj) in [
        (S_EP0, ep0),
        (S_REP0, rep0),
        (S_CTL, ctl),
        (S_SC0, sc0),
        (S_SC1, sc1),
    ] {
        let cap = m.root_cap(obj);
        m.put_cap(c0, s, cap).expect("cap vào CNode");
    }
    let ctl_s = slot(c0, S_CTL);
    m.sched_control_configure(ctl_s, slot(c0, S_SC0), 4, 8)
        .expect("Configure SC0");
    m.sched_control_configure(ctl_s, slot(c0, S_SC1), 4, 8)
        .expect("Configure SC1");
    let t0 = m.add_kernel_object(ObjType::Tcb);
    let t1 = m.add_kernel_object(ObjType::Tcb);
    for (s, obj) in [(S_T0, t0), (S_T1, t1)] {
        let cap = m.root_cap(obj);
        m.put_cap(c0, s, cap).expect("cap TCB");
    }
    let t0_s = slot(c0, S_T0);
    let t1_s = slot(c0, S_T1);
    m.set_sched_params(t0_s, Some(slot(c0, S_SC0)))
        .expect("t0 có SC");
    m.set_sched_params(t1_s, Some(slot(c0, S_SC1)))
        .expect("t1 có SC riêng");
    m.tcb_set_priority(t0_s, 5, 255).expect("t0 priority");
    m.tcb_set_priority(t1_s, 5, 255).expect("t1 priority");
    for obj in [t0, t1] {
        m.transition_tcb(obj, TcbState::Ready)
            .expect("Inactive→Ready");
    }
    ScRig {
        m,
        ep0_s: slot(c0, S_EP0),
        rep0_s: slot(c0, S_REP0),
        t0,
        t1,
        t0_s,
        t1_s,
    }
}

fn step(thread: &'static str, step: Step) -> ScenarioStep {
    ScenarioStep {
        thread,
        step,
        expect: Ok(()),
        expect_state: None,
    }
}

/// Kịch bản trộn bước mới (`TcbSuspend/Resume/ReadState`, `SchedTick/Pick`) + `Call`/`Recv`;
/// mọi bước đều **Ok ở mọi thứ tự** (không đặt `expect_state` vì thứ tự do seed quyết định).
fn kich_ban(r: &ScRig) -> Vec<ScenarioStep> {
    vec![
        step(
            "client",
            Step::Call {
                tcb: r.t0,
                ep: r.ep0_s,
                mr: 7,
            },
        ),
        step("client", Step::TcbReadState { tcb: r.t0_s }),
        step(
            "server",
            Step::Recv {
                tcb: r.t1,
                ep: r.ep0_s,
                reply: Some(r.rep0_s),
            },
        ),
        step("server", Step::TcbReadState { tcb: r.t1_s }),
        step(
            "init",
            Step::TcbSuspend {
                tcb: r.t0_s,
                mode: SuspendMode::Abort,
            },
        ),
        step("init", Step::TcbResume { tcb: r.t0_s }),
        step(
            "init",
            Step::TcbSuspend {
                tcb: r.t1_s,
                mode: SuspendMode::Abort,
            },
        ),
        step("init", Step::TcbResume { tcb: r.t1_s }),
        step("sched", Step::SchedTick { n: 1 }),
        step("sched", Step::SchedPick { hart: 0 }),
        step("sched", Step::SchedTick { n: 2 }),
        step("sched", Step::SchedPick { hart: 0 }),
    ]
}

/// DoD 1 — `run_interleaved` chọn bước theo scheduler; cùng seed ⇒ cùng trace; mọi bước Ok;
/// bất biến byte kiểm sau **mỗi** event.
#[test]
fn interleaved_ton_trong_scheduler_va_tat_dinh() {
    let mut traces = std::collections::BTreeSet::new();
    for seed in 0..64u64 {
        let mut a = sc_rig();
        let mut b = sc_rig();
        let sa = kich_ban(&a);
        let sb = kich_ban(&b);
        let ra = run_interleaved(&mut a.m, &sa, seed)
            .unwrap_or_else(|e| panic!("seed {seed}: kịch bản phải chạy, nhận {e}"));
        let rb = run_interleaved(&mut b.m, &sb, seed).expect("chạy lại");
        assert_eq!(ra.trace, rb.trace, "seed {seed}: cùng seed ⇒ cùng trace");
        assert_eq!(ra.events, sa.len(), "seed {seed}: mỗi bước một event");
        assert_eq!(
            ra.checked_bytes, ra.events,
            "seed {seed}: bất biến byte phải kiểm sau MỖI event"
        );
        a.m.k1b_audit()
            .unwrap_or_else(|e| panic!("seed {seed}: bất biến sau kịch bản: {e}"));
        // Bất biến scheduler liên quan trực tiếp tới interleaver: `Ready + suspended` KHÔNG
        // được nằm trong ready queue; `Running` phải là `current`.
        for (s, obj) in [(a.t0_s, a.t0), (a.t1_s, a.t1)] {
            let info = a.m.tcb_info(s).expect("info");
            if info.state == TcbState::Ready && info.suspended {
                assert!(
                    !a.m.ready_contains(0, obj),
                    "seed {seed}: Ready+suspended nằm trong ready queue"
                );
            }
            if matches!(info.state, TcbState::Running { .. }) {
                assert_eq!(a.m.current(0), Some(obj), "seed {seed}: Running ≠ current");
            }
        }
        traces.insert(ra.trace.join("\n"));
    }
    assert!(
        traces.len() >= 2,
        "64 seed phải chen khác nhau ít nhất 2 kiểu, chỉ thấy {}",
        traces.len()
    );
    println!(
        "# KT-23 interleaved: 64 seed, {} trace khác nhau, 12 bước/kịch bản",
        traces.len()
    );
}

/// DoD 1 (ca âm) — **budget**: TCB hết budget (`Sleeping{Budget}`) **không** chạy được; chỉ khi
/// có `SchedTick` đủ refill thì bước `Call` mới chạy. Chứng minh interleaver tôn trọng
/// `SchedContext` budget, không chỉ trạng thái TCB.
#[test]
fn interleaved_chan_tcb_het_budget() {
    // (a) Không có tick ⇒ `Call` bị chặn ⇒ bế tắc.
    let mut r = sc_rig();
    r.m.dispatch(0).expect("pick t0");
    assert_eq!(r.m.current(0), Some(r.t0));
    r.m.tick(4).expect("charge hết budget 4");
    assert_eq!(r.m.tcb_state(r.t0), Ok(sleep_budget()));
    let chi_call = vec![step(
        "client",
        Step::Call {
            tcb: r.t0,
            ep: r.ep0_s,
            mr: 1,
        },
    )];
    match run_interleaved(&mut r.m, &chi_call, 0) {
        Err(ScenarioError::Deadlock { remaining }) => assert_eq!(remaining, 1),
        other => panic!("TCB hết budget phải bị chặn (Deadlock), nhận {other:?}"),
    }

    // (b) Có `SchedTick{100}` (biên refill 8 ⇒ 104) ⇒ t0 `Ready` lại và `Call` chạy.
    let mut r = sc_rig();
    r.m.dispatch(0).expect("pick t0");
    r.m.tick(4).expect("charge hết budget");
    let steps = vec![
        step("sched", Step::SchedTick { n: 100 }),
        step(
            "client",
            Step::Call {
                tcb: r.t0,
                ep: r.ep0_s,
                mr: 1,
            },
        ),
    ];
    let rep = run_interleaved(&mut r.m, &steps, 3).expect("tick refill rồi Call phải chạy");
    assert_eq!(rep.events, 2, "phải chạy đủ 2 bước");
    assert!(matches!(
        r.m.tcb_state(r.t0),
        Ok(TcbState::BlockedOnSend { .. }) | Ok(TcbState::BlockedOnReply { .. })
    ));
    r.m.k1b_audit().expect("bất biến sau refill + Call");
}

/// DoD 1 (ca âm) — **ready queue**: TCB `Ready + suspended` **không** nằm trong hàng nên bước
/// syscall của nó bị chặn; `TcbResume` (bước quản trị) gỡ cờ rồi mới chạy được.
#[test]
fn interleaved_chan_tcb_suspended() {
    let mut r = sc_rig();
    r.m.tcb_suspend(r.t0_s, SuspendMode::Abort)
        .expect("Suspend(Abort) trên Ready");
    assert!(
        !r.m.ready_contains(0, r.t0),
        "Ready+suspended không được nằm trong ready queue"
    );
    let chi_call = vec![step(
        "client",
        Step::Call {
            tcb: r.t0,
            ep: r.ep0_s,
            mr: 2,
        },
    )];
    match run_interleaved(&mut r.m, &chi_call, 5) {
        Err(ScenarioError::Deadlock { remaining }) => assert_eq!(remaining, 1),
        other => panic!("TCB suspended phải bị chặn, nhận {other:?}"),
    }
    let steps = vec![
        step("init", Step::TcbResume { tcb: r.t0_s }),
        step(
            "client",
            Step::Call {
                tcb: r.t0,
                ep: r.ep0_s,
                mr: 2,
            },
        ),
    ];
    let rep = run_interleaved(&mut r.m, &steps, 5).expect("Resume rồi Call phải chạy");
    assert_eq!(rep.events, 2);
    r.m.k1b_audit().expect("bất biến sau Resume + Call");
}

// ===========================================================================
// 3. 1.000 seed × 64 bước: TCB + scheduler + budget/donation + hộp thư (DoD 2)
// ===========================================================================

/// Slot cố định của rig fuzz.
const F_UNTYPED: usize = 0;
const F_EP: [usize; 2] = [1, 2];
const F_REP: usize = 3;
const F_CTL: usize = 4;
const F_NTFN: usize = 5;
const F_SC: usize = 8; // 8..14
const F_TCB: usize = 16; // 16..24
const F_FREE: usize = 32; // 32..40: slot tạm cho Retype/Delete
const N_TCB: usize = 8;
const N_SC: usize = 6;
const N_FREE: usize = 8;

/// Rig fuzz: CNode 128 slot, 1 Untyped 64 KiB, 2 Endpoint, 1 Reply, 1 Notification,
/// 1 SchedControl, 6 SC (budget 4 / period 8), 8 TCB (0..6 có SC + `Ready`, 6..8 `Inactive`
/// **không SC** — D2-50), tất cả có `fault_ep = ep[1]` để `Exit` mang fault.
struct FuzzRig {
    m: Machine,
    c0: ObjId,
    ep: [ObjId; 2],
    ep_s: [SlotRef; 2],
    rep_s: SlotRef,
    ntfn_s: SlotRef,
    t: [ObjId; N_TCB],
    ts: [SlotRef; N_TCB],
}

fn fuzz_rig() -> FuzzRig {
    let mut m = Machine::new();
    let c0 = m.add_cnode(128);
    let u0 = m.add_untyped(0x1000, 65536);
    let cap = m.root_cap(u0);
    m.put_cap(c0, F_UNTYPED, cap).expect("cap Untyped");
    let mut ep = [0u32; 2];
    let mut ep_s = [slot(c0, 0); 2];
    for (i, s) in F_EP.iter().enumerate() {
        let obj = m.add_kernel_object(ObjType::Endpoint);
        let cap = m.root_cap(obj);
        m.put_cap(c0, *s, cap).expect("cap Endpoint");
        ep[i] = obj;
        ep_s[i] = slot(c0, *s);
    }
    let rep = m.add_kernel_object(ObjType::Reply);
    let rep_cap = m.root_cap(rep);
    m.put_cap(c0, F_REP, rep_cap).expect("cap Reply");
    let ntfn = m.add_kernel_object(ObjType::Notification);
    let ntfn_cap = m.root_cap(ntfn);
    m.put_cap(c0, F_NTFN, ntfn_cap).expect("cap Notification");
    let ctl = m.add_kernel_object(ObjType::SchedControl);
    let ctl_cap = m.root_cap(ctl);
    m.put_cap(c0, F_CTL, ctl_cap).expect("cap SchedControl");
    let ctl_s = slot(c0, F_CTL);

    for i in 0..N_SC {
        let obj = m.add_kernel_object(ObjType::SchedContext);
        let cap = m.root_cap(obj);
        m.put_cap(c0, F_SC + i, cap).expect("cap SC");
        m.sched_control_configure(ctl_s, slot(c0, F_SC + i), 4, 8)
            .expect("Configure SC");
    }

    let mut t = [0u32; N_TCB];
    let mut ts = [slot(c0, 0); N_TCB];
    for i in 0..N_TCB {
        let obj = m.add_kernel_object(ObjType::Tcb);
        let cap = m.root_cap(obj);
        m.put_cap(c0, F_TCB + i, cap).expect("cap TCB");
        t[i] = obj;
        ts[i] = slot(c0, F_TCB + i);
        m.tcb_set_priority(ts[i], 5 + (i as u8 % 3), 255)
            .expect("priority");
        m.tcb_set_fault_ep(ts[i], ep_s[1]).expect("fault_ep");
        if i < N_SC {
            m.set_sched_params(ts[i], Some(slot(c0, F_SC + i)))
                .expect("set_sched_params (test-only)");
            m.transition_tcb(obj, TcbState::Ready).expect("Ready");
        }
    }
    FuzzRig {
        m,
        c0,
        ep,
        ep_s,
        rep_s: slot(c0, F_REP),
        ntfn_s: slot(c0, F_NTFN),
        t,
        ts,
    }
}

/// Áp **một** op lên rig (trả `ok`) — dùng chung cho lượt chạy và lượt đọc bộ đếm.
fn ap_op(r: &mut FuzzRig, op: u64, ti: usize, epi: usize, buoc: u64, rng: &mut Lcg) -> bool {
    match op {
        // Retype + Delete: store object + bất biến byte (không giữ lại object).
        0..=9 => {
            let ty = [
                ObjType::Endpoint,
                ObjType::Reply,
                ObjType::Notification,
                ObjType::SchedContext,
                ObjType::Tcb,
            ][rng.below(5) as usize];
            let free = F_FREE + rng.below(N_FREE as u64) as usize;
            match r.m.retype(slot(r.c0, F_UNTYPED), ty, 0, 0) {
                Ok(cap) => {
                    r.m.put_cap(r.c0, free, cap)
                        .and_then(|()| r.m.delete(slot(r.c0, free)))
                        .is_ok()
                }
                Err(_) => false,
            }
        }
        10..=22 => r.m.call(r.t[ti], r.ep_s[epi], buoc).is_ok(),
        23..=35 => r.m.recv(r.t[ti], r.ep_s[epi], Some(r.rep_s)).is_ok(),
        36..=43 => r.m.reply(r.t[ti], r.rep_s).is_ok(),
        44..=51 => r.m.tcb_suspend(r.ts[ti], SuspendMode::Abort).is_ok(),
        52..=57 => r.m.tcb_resume(r.ts[ti]).is_ok(),
        58..=67 => r.m.tick(1 + rng.below(3)).is_ok(),
        68..=75 => r.m.dispatch(0).is_ok(),
        76..=82 => r.m.exit(r.t[ti], buoc).is_ok(),
        83..=87 => r.m.wake(r.t[ti]).is_ok(),
        88..=91 => r.m.tcb_wait_notification(r.ts[ti], r.ntfn_s).is_ok(),
        92..=94 => {
            r.m.tcb_wait_address(r.ts[ti], r.ep[0], (buoc % 3) as u32)
                .is_ok()
        }
        95..=97 => match r.m.post_mail(0, MailboxMsg::Wake(r.t[ti])) {
            Ok(()) => r.m.process_mail(0).is_ok(),
            Err(_) => false,
        },
        // `finalise` TCB ⇒ `* → Dead` (nguồn của các cặp `*→Dead`). `CNode.Delete` chỉ gỡ cap,
        // **không** finalise object (KT-18: finalise là hook của objmem/Revoke), nên ops phải
        // gọi đúng cửa `finalise_tcb` mới đi được cặp `*→Dead`.
        _ => r.m.finalise_tcb(r.t[ti]).is_ok(),
    }
}

/// Bộ đếm vi phạm của test (DoD 2): phải **0** khi in; panic ngay khi một miền vỡ.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct ViPham {
    sched: u64,
    budget: u64,
    sc_refs: u64,
    mailbox: u64,
    bytes: u64,
    checks: u64,
}

impl ViPham {
    fn tong(&self) -> u64 {
        self.sched + self.budget + self.sc_refs + self.mailbox + self.bytes
    }
}

/// Kiểm **bốn** miền bất biến sau **mỗi** bước (DoD 2) — vi phạm là panic kèm bối cảnh.
fn kiem_bon_mien(m: &Machine, buoc: &str, v: &mut ViPham) {
    v.checks += 4;
    if let Err(e) = m.check_sched() {
        v.sched += 1;
        panic!("check_sched sau {buoc}: {e}");
    }
    if let Err(e) = m.check_budget() {
        v.budget += 1;
        panic!("check_budget sau {buoc}: {e}");
    }
    if let Err(e) = m.check_sc_refs() {
        v.sc_refs += 1;
        panic!("check_sc_refs sau {buoc}: {e}");
    }
    if let Err(e) = m.check_mailboxes() {
        v.mailbox += 1;
        panic!("check_mailboxes sau {buoc}: {e}");
    }
    if let Err(e) = m.check_all_bytes() {
        v.bytes += 1;
        panic!("check_all_bytes sau {buoc}: {e}");
    }
}

#[cfg(miri)]
const N_SEED: u64 = 3;
#[cfg(not(miri))]
const N_SEED: u64 = 1000;
#[cfg(miri)]
const N_STEP: u64 = 8;
#[cfg(not(miri))]
const N_STEP: u64 = 64;

/// Thống kê của **một** seed, đọc từ bộ đếm của mô hình (không bịa số).
struct ThongKe {
    pair_hits: [u32; TCB_STATE_COUNT * TCB_STATE_COUNT],
    forbidden_attempts: u64,
    budget_sleeps: u64,
    refills: u64,
    ticks: u64,
    ep_pushed: u64,
    running_rows: u64,
    donation_rows: u64,
}

/// Một dòng trace: `[now, bước, op, ok, current, ready_len, mail_len, ep len, pushed, popped,
/// removed, (state,suspended,sc_of,donated_sc) × N_TCB]`.
fn run_seed(seed: u64) -> (Vec<Vec<u64>>, ViPham, ThongKe) {
    let mut r = fuzz_rig();
    let mut v = ViPham::default();
    let mut rng = Lcg::new(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ 0x23_1b);
    let mut rows: Vec<Vec<u64>> = Vec::with_capacity(N_STEP as usize);
    let mut tk = ThongKe {
        pair_hits: [0u32; TCB_STATE_COUNT * TCB_STATE_COUNT],
        forbidden_attempts: 0,
        budget_sleeps: 0,
        refills: 0,
        ticks: 0,
        ep_pushed: 0,
        running_rows: 0,
        donation_rows: 0,
    };
    for buoc in 0..N_STEP {
        let op = rng.below(100);
        let ti = rng.below(N_TCB as u64) as usize;
        let epi = rng.below(2) as usize;
        let ok = ap_op(&mut r, op, ti, epi, buoc, &mut rng);
        let buoc_mo_ta = format!("seed {seed} bước {buoc} op {op}");
        kiem_bon_mien(&r.m, &buoc_mo_ta, &mut v);
        let mut row = Vec::with_capacity(11 + 4 * N_TCB);
        row.push(r.m.virtual_now());
        row.push(buoc);
        row.push(op);
        row.push(ok as u64);
        row.push(r.m.current(0).unwrap_or(0) as u64);
        row.push(r.m.ready_len(0) as u64);
        row.push(r.m.mail_len(0).expect("mail_len") as u64);
        let e = r.m.endpoint_info(r.ep[0]).expect("endpoint_info");
        row.push(e.len as u64);
        row.push(e.pushed);
        row.push(e.popped);
        row.push(e.removed);
        let mut co_running = false;
        let mut co_donate = false;
        for i in 0..N_TCB {
            let info = r.m.tcb_info(r.ts[i]).ok();
            let sc = r.m.sc_of(r.t[i]).ok().flatten().unwrap_or(0);
            let donated = r.m.donated_sc(r.t[i]).ok().flatten().unwrap_or(0);
            if matches!(info.map(|x| x.state), Some(TcbState::Running { .. })) {
                co_running = true;
            }
            if donated != 0 {
                co_donate = true;
            }
            row.push(info.map(|x| x.state.index() as u64).unwrap_or(0));
            row.push(info.map(|x| x.suspended as u64).unwrap_or(0));
            row.push(sc as u64);
            row.push(donated as u64);
        }
        if co_running {
            tk.running_rows += 1;
        }
        if co_donate {
            tk.donation_rows += 1;
        }
        rows.push(row);
    }
    // Bộ đếm cuối seed — đọc từ mô hình.
    for from in 0..TCB_STATE_COUNT {
        for to in 0..TCB_STATE_COUNT {
            tk.pair_hits[from * TCB_STATE_COUNT + to] = r.m.pair_hit_count(from, to);
            tk.forbidden_attempts += r.m.forbidden_attempt_count(from, to) as u64;
        }
    }
    let rep = r.m.k1b_report();
    tk.budget_sleeps = rep.budget.budget_sleeps;
    tk.refills = rep.budget.refills;
    tk.ticks = rep.budget.ticks;
    tk.ep_pushed = r.m.endpoint_info(r.ep[0]).expect("endpoint_info").pushed;
    assert_eq!(
        rep.sched_violations
            + rep.sc_ref_violations
            + rep.budget_violations
            + rep.mailbox_violations
            + rep.byte_violations,
        0,
        "seed {seed}: report phải 0 vi phạm"
    );
    (rows, v, tk)
}

/// **DoD 2** — 1.000 seed × 64 bước LCG trộn retype/call/recv/reply/suspend/resume/tick/pick/
/// exit/wait/wake/mail; sau **mỗi** bước kiểm 4 miền bất biến; 0 vi phạm; cùng seed ⇒ cùng
/// trace; in transitions hit/allowed, forbidden_attempts, budget_violations.
#[test]
fn fuzz_1000_seed_64_buoc_k1b() {
    // Cùng seed ⇒ cùng trace (byte-for-byte ở mức `Row`).
    for seed in 0..16 {
        let (a, va, _) = run_seed(seed);
        let (b, vb, _) = run_seed(seed);
        assert_eq!(a, b, "seed {seed} phải tất định");
        assert_eq!(va.tong(), 0, "seed {seed}: vi phạm bất biến");
        assert_eq!(vb.tong(), 0);
        assert_eq!(a.len(), N_STEP as usize);
    }

    let mut v = ViPham::default();
    let mut pair_hits = [0u32; TCB_STATE_COUNT * TCB_STATE_COUNT];
    let mut forbidden_attempts = 0u64;
    let mut budget_sleeps = 0u64;
    let mut refills = 0u64;
    let mut ticks = 0u64;
    let mut mail_pushed = 0u64;
    let mut donation_rows = 0u64;
    let mut running_rows = 0u64;
    for seed in 0..N_SEED {
        let (rows, vv, tk) = run_seed(seed);
        assert_eq!(rows.len(), N_STEP as usize, "seed {seed}: đủ số bước");
        assert_eq!(vv.tong(), 0, "seed {seed}: vi phạm bất biến");
        v.sched += vv.sched;
        v.budget += vv.budget;
        v.sc_refs += vv.sc_refs;
        v.mailbox += vv.mailbox;
        v.bytes += vv.bytes;
        v.checks += vv.checks;
        for (i, n) in tk.pair_hits.iter().enumerate() {
            pair_hits[i] = pair_hits[i].saturating_add(*n);
        }
        forbidden_attempts += tk.forbidden_attempts;
        budget_sleeps += tk.budget_sleeps;
        refills += tk.refills;
        ticks += tk.ticks;
        mail_pushed += tk.ep_pushed;
        donation_rows += tk.donation_rows;
        running_rows += tk.running_rows;
    }
    let (hit, allowed) = coverage_design(&pair_hits);
    println!(
        "# KT-23 fuzz: {N_SEED} seed × {N_STEP} bước, checks={} (0 vi phạm: sched={} budget={} \
         sc_refs={} mailbox={} bytes={})",
        v.checks, v.sched, v.budget, v.sc_refs, v.mailbox, v.bytes
    );
    println!(
        "# KT-23 fuzz: transitions (thao tác) hit/allowed = {hit}/{allowed}; \
         forbidden_attempts (thao tác) = {forbidden_attempts}; budget_sleeps={budget_sleeps}; \
         refills={refills}; ticks={ticks}; donation_rows={donation_rows}; \
         running_rows={running_rows}; mail_pushed={mail_pushed}"
    );
    assert_eq!(
        v.tong(),
        0,
        "0 vi phạm bất biến trên toàn bộ 1.000 seed (DoD 2)"
    );
    assert_eq!(
        v.checks,
        N_SEED * N_STEP * 4,
        "mỗi bước phải kiểm đúng 4 miền bất biến"
    );
    if cfg!(not(miri)) {
        // Kịch bản phải **thực sự** đi qua scheduler + budget + donation (không xanh rỗng).
        assert!(
            budget_sleeps > 0 && refills > 0 && ticks > 0,
            "kịch bản phải thực sự hết budget/refill/tick: {budget_sleeps}/{refills}/{ticks}"
        );
        assert!(
            running_rows > 0 && donation_rows > 0,
            "kịch bản phải thực sự có TCB Running và donation: {running_rows}/{donation_rows}"
        );
        assert!(mail_pushed > 0, "kịch bản phải thực sự đẩy message");
        // Thao tác **hợp lệ** không được đi qua cặp cấm (cặp cấm có ma trận riêng ở DoD 3).
        assert_eq!(
            forbidden_attempts, 0,
            "thao tác hợp lệ không được thử cặp cấm (điều tra nếu có)"
        );
        // Coverage **thao tác**: 45/46 — cặp duy nhất không thao tác nào chạm được là
        // `Stopping → Dead` (K1b **không có** nguồn nhập `Stopping`; 2 cặp nhập `Stopping` là
        // reserved cho K1c). Không hạ tiêu chí 46/46: test ma trận chứng minh 46/46 **qua cổng**
        // (và 48/48 toàn bảng, gồm 2 cặp reserved). Nếu mất thêm cặp nào ⇒ test đỏ.
        assert_eq!(
            (hit, allowed),
            (45, 46),
            "coverage thao tác phải đúng 45/46 (thiếu `Stopping→Dead` là có nguồn gốc thiết kế)"
        );
    }
}

// ===========================================================================
// 4. Exit `Busy`: snapshot đầy đủ (KT-22 review → KT-23)
// ===========================================================================

/// Rig cho ca `Exit` `Busy`: endpoint fault đầy 8 message, client t0 đang **donate** SC của mình
/// cho server passive t1 (`BlockedOnReply`, `Reply.caller = t0`).
struct BusyRig {
    m: Machine,
    ep_fault: ObjId,
    rep0: ObjId,
    sc0_s: SlotRef,
    t0: ObjId,
    t0_s: SlotRef,
    t1: ObjId,
}

const B_EP_FAULT: usize = 0;
const B_EP0: usize = 1;
const B_REP0: usize = 2;
const B_CTL: usize = 3;
const B_SC0: usize = 4;
const B_T0: usize = 16;
const B_T1: usize = 17;
const B_HELPER: usize = 24;
const N_HELPER: usize = 8;

fn busy_rig() -> BusyRig {
    let mut m = Machine::new();
    let c0 = m.add_cnode(128);
    let ep_fault = m.add_kernel_object(ObjType::Endpoint);
    let ep0 = m.add_kernel_object(ObjType::Endpoint);
    let rep0 = m.add_kernel_object(ObjType::Reply);
    let ctl = m.add_kernel_object(ObjType::SchedControl);
    let sc0 = m.add_kernel_object(ObjType::SchedContext);
    for (s, obj) in [
        (B_EP_FAULT, ep_fault),
        (B_EP0, ep0),
        (B_REP0, rep0),
        (B_CTL, ctl),
        (B_SC0, sc0),
    ] {
        let cap = m.root_cap(obj);
        m.put_cap(c0, s, cap).expect("cap vào CNode");
    }
    let ctl_s = slot(c0, B_CTL);
    let sc0_s = slot(c0, B_SC0);
    m.sched_control_configure(ctl_s, sc0_s, 4, 8)
        .expect("Configure SC0");
    let t0 = m.add_kernel_object(ObjType::Tcb);
    let t1 = m.add_kernel_object(ObjType::Tcb);
    for (s, obj) in [(B_T0, t0), (B_T1, t1)] {
        let cap = m.root_cap(obj);
        m.put_cap(c0, s, cap).expect("cap TCB");
    }
    let t0_s = slot(c0, B_T0);
    let t1_s = slot(c0, B_T1);
    m.set_sched_params(t0_s, Some(sc0_s)).expect("t0 có SC");
    m.tcb_set_priority(t0_s, 5, 255).expect("t0 priority");
    m.tcb_set_priority(t1_s, 5, 255).expect("t1 priority");
    let ep_fault_s = slot(c0, B_EP_FAULT);
    m.tcb_set_fault_ep(t0_s, ep_fault_s).expect("t0 fault_ep");
    m.transition_tcb(t0, TcbState::Ready).expect("t0 Ready");
    m.transition_tcb(t1, TcbState::Ready).expect("t1 Ready");
    // Chờ server passive t1 `Recv`, rồi t0 `Call` ⇒ t0 `BlockedOnReply` + donate SC0 cho t1.
    m.recv(t1, slot(c0, B_EP0), Some(slot(c0, B_REP0)))
        .expect("t1 Recv");
    m.call(t0, slot(c0, B_EP0), 1).expect("t0 Call");
    // Làm đầy hộp thư fault: 8 TCB phụ `Exit` mang fault vào `ep_fault` ⇒ `len == cap == 8`.
    for i in 0..N_HELPER {
        let helper = m.add_kernel_object(ObjType::Tcb);
        let cap = m.root_cap(helper);
        m.put_cap(c0, B_HELPER + i, cap).expect("cap helper");
        let hs = slot(c0, B_HELPER + i);
        m.tcb_set_fault_ep(hs, ep_fault_s).expect("fault_ep helper");
        m.exit(helper, i as u64).expect("fault vào hộp thư");
    }
    BusyRig {
        m,
        ep_fault,
        rep0,
        sc0_s,
        t0,
        t0_s,
        t1,
    }
}

/// **KT-23** (follow-up review KT-22 (b)) — `Exit` khi hộp thư `fault_ep` **đầy** trả `Busy` và
/// **không** thay đổi **một** trường nào: `exit_code`, trạng thái, donation/`SchedContext`
/// (hai chiều), `Reply.caller`, hàng đợi endpoint (`len`/`pushed`/`popped`/`removed`).
#[test]
fn exit_busy_khong_doi_donation_reply_va_hang_doi() {
    let mut r = busy_rig();
    assert_eq!(
        r.m.endpoint_info(r.ep_fault).expect("info").len,
        MAILBOX_CAP,
        "hộp thư fault phải đầy trước khi thử Exit"
    );
    // Tiền điều kiện của snapshot: t0 donor, t1 donee, caller của Reply là t0.
    let sc0: ObjId =
        r.m.tcb_info(r.t0_s)
            .expect("t0 info")
            .sched_context
            .expect("t0 có SC riêng");
    assert_eq!(
        r.m.tcb_state(r.t0),
        Ok(TcbState::BlockedOnReply { reply: r.rep0 })
    );
    assert_eq!(
        r.m.sched_donated_to(r.sc0_s).expect("donated_to"),
        Some(r.t1)
    );
    assert_eq!(r.m.sched_donor_of(r.sc0_s).expect("donor"), Some(r.t0));
    assert_eq!(r.m.donated_sc(r.t1).expect("donated_sc"), Some(sc0));
    assert_eq!(r.m.sc_of(r.t1).expect("sc_of"), Some(sc0));
    assert_eq!(r.m.reply_caller(r.rep0).expect("reply_caller"), Some(r.t0));

    let info_truoc: TcbInfo = r.m.tcb_info(r.t0_s).expect("t0 info");
    let sc_truoc: ScInfo = r.m.sched_info(r.sc0_s).expect("sc info");
    let ep_truoc = r.m.endpoint_info(r.ep_fault).expect("ep info");
    let caller_truoc = r.m.reply_caller(r.rep0).expect("reply caller");
    let items_truoc = r.m.endpoint_send_items(r.ep_fault).expect("hàng gửi fault");

    assert_eq!(
        r.m.exit(r.t0, 99),
        Err(KError::Busy),
        "hộp thư fault đầy ⇒ Exit phải Busy (backpressure)"
    );

    // Không một trường nào đổi.
    assert_eq!(
        r.m.tcb_info(r.t0_s).expect("t0 info"),
        info_truoc,
        "TcbInfo đổi"
    );
    assert_eq!(info_truoc.exit_code, None, "exit_code không được ghi");
    assert_eq!(
        r.m.tcb_state(r.t0),
        Ok(TcbState::BlockedOnReply { reply: r.rep0 }),
        "trạng thái không đổi"
    );
    assert_eq!(
        r.m.sched_info(r.sc0_s).expect("sc info"),
        sc_truoc,
        "ScInfo đổi"
    );
    assert_eq!(
        r.m.sched_donated_to(r.sc0_s).expect("donated_to"),
        Some(r.t1),
        "donation phải còn (không trả SC khi Exit Busy)"
    );
    assert_eq!(
        r.m.sched_donor_of(r.sc0_s).expect("donor"),
        Some(r.t0),
        "donor phải còn"
    );
    assert_eq!(
        r.m.donated_sc(r.t1).expect("donated_sc"),
        Some(sc0),
        "donee phải giữ SC được donate"
    );
    assert_eq!(
        r.m.reply_caller(r.rep0).expect("reply caller"),
        caller_truoc,
        "Reply.caller không đổi"
    );
    assert_eq!(
        r.m.endpoint_info(r.ep_fault).expect("ep info"),
        ep_truoc,
        "hàng đợi endpoint đổi (len/pushed/popped/removed)"
    );
    assert_eq!(
        r.m.endpoint_send_items(r.ep_fault).expect("hàng gửi"),
        items_truoc,
        "danh sách message trong hàng đổi"
    );
    r.m.k1b_audit().expect("bất biến sau Exit Busy");
    println!(
        "# KT-23 Exit Busy: exit_code=None, donation giữ nguyên (SC {} ↔ t{}), \
         Reply.caller={:?}, endpoint len={} pushed={} — không đổi",
        sc0, r.t1, caller_truoc, ep_truoc.len, ep_truoc.pushed
    );
}

/// **Bug do fuzz KT-23 tìm ra** (regression): `Exit` mang fault vào `fault_ep` khi endpoint
/// **đang có `Recv` waiter** phải giao **thẳng** cho waiter — message **không** vào hàng, nên
/// bất biến KT-22 "có message ⇒ không `Recv` waiter" giữ; procsrv `Recv` coi như xong ⇒ TCB
/// thoát `Inactive` ngay (D2-25), **không** `BlockedOnSend` trên message không còn trong hàng.
/// Ca âm: TCB đang `Inactive` + `fault_ep` + waiter ⇒ **giữ** `Inactive` (cặp danh tính
/// `(Inactive, Inactive)` bị cấm — cùng quy ước nhánh không `fault_ep`).
#[test]
fn exit_fault_giao_thang_cho_recv_waiter() {
    for (trang_thai_dau, ten) in [(TcbState::Ready, "Ready"), (TcbState::Inactive, "Inactive")] {
        let mut m = Machine::new();
        let c0 = m.add_cnode(32);
        let ep = m.add_kernel_object(ObjType::Endpoint);
        let ep_cap = m.root_cap(ep);
        m.put_cap(c0, 0, ep_cap).expect("cap Endpoint");
        let ep_s = slot(c0, 0);
        let procsrv = m.add_kernel_object(ObjType::Tcb);
        let procsrv_cap = m.root_cap(procsrv);
        m.put_cap(c0, 1, procsrv_cap).expect("cap procsrv");
        m.transition_tcb(procsrv, TcbState::Ready)
            .expect("procsrv Ready");
        m.recv(procsrv, ep_s, None).expect("procsrv Recv");
        assert_eq!(
            m.tcb_state(procsrv),
            Ok(TcbState::BlockedOnRecv { ep, reply: None })
        );
        let thoat = m.add_kernel_object(ObjType::Tcb);
        let thoat_cap = m.root_cap(thoat);
        m.put_cap(c0, 2, thoat_cap).expect("cap TCB thoát");
        let thoat_s = slot(c0, 2);
        m.tcb_set_fault_ep(thoat_s, ep_s).expect("fault_ep");
        if trang_thai_dau != TcbState::Inactive {
            m.transition_tcb(thoat, trang_thai_dau)
                .expect("trạng thái đầu");
        }
        m.exit(thoat, 7).expect("Exit phải giao thẳng cho waiter");
        assert_eq!(
            m.tcb_state(procsrv),
            Ok(TcbState::Ready),
            "{ten}: waiter phải được đánh thức (fault giao thẳng)"
        );
        assert_eq!(
            m.tcb_state(thoat),
            Ok(TcbState::Inactive),
            "{ten}: TCB thoát phải `Inactive` (D2-25), không `BlockedOnSend`"
        );
        assert_eq!(
            m.endpoint_info(ep).expect("endpoint_info").len,
            0,
            "{ten}: fault không được nằm lại trong hàng khi có waiter"
        );
        assert_eq!(m.tcb_info(thoat_s).expect("info").exit_code, Some(7));
        m.check_mailboxes().expect("bất biến mailbox");
        m.k1b_audit().expect("bất biến K1b");
    }
}

// ===========================================================================
// 5. Bảng `Stopping` reserved: KT-23 xác nhận 2 cặp, không hạ tiêu chí
// ===========================================================================

#[test]
fn chi_hai_cap_stopping_la_reserved() {
    assert_eq!(vifast_sim::tcb::reserved_count(), 2);
    assert_eq!(vifast_sim::tcb::forbidden_count(), 73);
    assert_eq!(vifast_sim::tcb::allowed_count(), 48);
    let (hit, allowed) = coverage_design(&[0u32; TCB_STATE_COUNT * TCB_STATE_COUNT]);
    assert_eq!((hit, allowed), (0, 46));
}
