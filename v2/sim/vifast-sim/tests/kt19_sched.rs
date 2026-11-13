// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-19 — scheduler **1 hart** cho mô hình TCB: 256 mức ưu tiên + bitmap, `pick` O(1),
//! `Ready`/`Running` một cửa qua `transition_tcb`, preempt ép tại **mọi** bước, không đói
//! cùng mức, finalise/suspend gỡ khỏi hàng. **Model-only `[TK]`**, không phải spec/ABI.
//!
//! Nguồn ngữ nghĩa: thiết kế §4.8 (`docs/v2/ViFastOs-v2-thiet-ke.md:336-344`), §4.13
//! (`:382-387`), **D2-49(a)** (1 hart, hoãn push migration sang K1c), **D2-49(h)** (tick ảo);
//! kế hoạch `review/agent0/K1B-PLAN.md` §5 KT-19 (DoD 1-8).
//!
//! Bất biến được kiểm **sau mỗi** public op bằng [`Machine::check_sched`] (DoD 3):
//! - không TCB nào trùng trong hàng; bucket khớp `priority`;
//! - mọi TCB trong hàng tồn tại, `Ready`, **không** `suspended`;
//! - `Ready` (không treo) ⇔ có trong hàng; `Running{hart}` ⇔ là `current` của hart đó;
//! - ≤ **1** `Running`/hart; `len`/bitmap khớp số phần tử; `idle ⇔ current == None`.

use vifast_sim::kobj::{KError, Machine, ObjId, ObjType, SlotRef, SuspendMode, TcbState};
use vifast_sim::Lcg;

fn slot(cnode: ObjId, s: usize) -> SlotRef {
    SlotRef::new(cnode, s)
}

/// Kiểm bất biến scheduler — gọi **sau mỗi** public op (DoD 3).
fn kiem(m: &Machine, buoc: &str) {
    if let Err(e) = m.check_sched() {
        panic!("check_sched sau {buoc}: {e}");
    }
}

// ---------------------------------------------------------------------------
// Rig chung: CNode 16 slot + Untyped + Endpoint/Reply/Notification/SchedContext
// + 3 TCB (slot 8/9/10).
// ---------------------------------------------------------------------------

const S_UNTYPED: usize = 0;
const S_EP: usize = 1;
const S_REP: usize = 2;
const S_NTFN: usize = 3;
const S_SC: usize = 4;
const S_CNODE: usize = 5;
const S_CTL: usize = 6;
const S_T0: usize = 8;

/// SC mặc định **test-only** cho rig KT-19: KT-20 siết "TCB không SC/SC chưa Configure không
/// được `pick`" (reviewer-kt20 mục 1), nên rig cũ phải gắn một SC đã `Configure` cho TCB cần
/// chạy. Không đổi production; không đổi kỳ vọng test KT-19 (không có `tick` nên `consumed`
/// luôn 0). Budget = period = 64 là hằng **test**, không phải ngưỡng cổng.
const SC_MAC_DINH: u64 = 64;

/// Gắn SC mặc định cho một TCB (test-only).
fn gan_sc_mac_dinh(m: &mut Machine, c0: ObjId, tcb_slot: usize, sc_slot: usize) {
    m.set_sched_params(slot(c0, tcb_slot), Some(slot(c0, sc_slot)))
        .expect("set_sched_params SC mặc định (test-only)");
}

struct Rig {
    m: Machine,
    c0: ObjId,
    ep: ObjId,
    rep: ObjId,
    ntfn: ObjId,
    t: [ObjId; 3],
    s: [SlotRef; 3],
}

impl Rig {
    fn new() -> Self {
        let mut m = Machine::new();
        let c0 = m.add_cnode(16);
        let u0 = m.add_untyped(0x1000, 65536);
        let ep = m.add_kernel_object(ObjType::Endpoint);
        let rep = m.add_kernel_object(ObjType::Reply);
        let ntfn = m.add_kernel_object(ObjType::Notification);
        let sc = m.add_kernel_object(ObjType::SchedContext);
        let sched_control = m.add_kernel_object(ObjType::SchedControl);
        // CNode thứ hai để `Tcb.Configure` có CSpace hợp lệ (không tự trỏ CNode gốc).
        let c1 = m.add_cnode(4);
        let mut t = [0u32; 3];
        let mut s = [slot(c0, S_T0), slot(c0, S_T0 + 1), slot(c0, S_T0 + 2)];
        for (i, dst) in [
            (S_UNTYPED, u0),
            (S_EP, ep),
            (S_REP, rep),
            (S_NTFN, ntfn),
            (S_SC, sc),
            (S_CNODE, c1),
            (S_CTL, sched_control),
        ]
        .into_iter()
        {
            let cap = m.root_cap(dst);
            m.put_cap(c0, i, cap).expect("cap object");
        }
        // KT-20 (reviewer mục 1): cấp ngân sách mặc định test-only cho SC và gắn cho 3 TCB.
        m.sched_control_configure(slot(c0, S_CTL), slot(c0, S_SC), SC_MAC_DINH, SC_MAC_DINH)
            .expect("SchedControl.Configure SC mặc định (test-only)");
        for i in 0..3 {
            let tcb = m.add_kernel_object(ObjType::Tcb);
            let cap = m.root_cap(tcb);
            m.put_cap(c0, S_T0 + i, cap).expect("cap TCB");
            t[i] = tcb;
            s[i] = slot(c0, S_T0 + i);
            gan_sc_mac_dinh(&mut m, c0, S_T0 + i, S_SC);
        }
        Rig {
            m,
            c0,
            ep,
            rep,
            ntfn,
            t,
            s,
        }
    }

    fn set_prio(&mut self, i: usize, p: u8) {
        self.m
            .tcb_set_priority(self.s[i], p, 255)
            .expect("Tcb.SetPriority");
        kiem(&self.m, "Tcb.SetPriority");
    }

    /// `Inactive → Ready` qua **một cửa** (cặp ALLOWED của bảng KT-18).
    fn ready(&mut self, i: usize) {
        self.m
            .transition_tcb(self.t[i], TcbState::Ready)
            .expect("Inactive→Ready");
        kiem(&self.m, "transition Ready");
    }

    fn dispatch(&mut self, hart: u16) -> Option<ObjId> {
        let got = self.m.dispatch(hart).expect("dispatch");
        kiem(&self.m, "dispatch");
        got
    }
}

/// Đếm TCB đang `Running` trong danh sách đã biết (≤1/hart — DoD 2).
fn dem_running(m: &Machine, tcbs: &[ObjId]) -> Vec<ObjId> {
    tcbs.iter()
        .copied()
        .filter(|t| matches!(m.tcb_state(*t), Ok(TcbState::Running { .. })))
        .collect()
}

// ---------------------------------------------------------------------------
// 1. `pick` O(1) đủ 256 mức 0..=255, mức cao nhất được chọn, cùng mức FIFO (DoD 1).
// ---------------------------------------------------------------------------

/// Số mức ưu tiên của test `pick`: **256** trên stable (DoD 1); dưới Miri chỉ **64** vì
/// `check_sched` (gọi sau **mỗi** op, đúng DoD 3) quét toàn bộ TCB sống — 256 TCB × ~1.000
/// lần kiểm làm Miri chạy >20 phút. Đây **không** phải nới ngưỡng cổng: `gate --fast` trên
/// stable vẫn kiểm đủ 0..=255; Miri chỉ là lệnh kiểm bộ nhớ **rời** (D2-47).
#[cfg(miri)]
const N_PRIO: usize = 64;
#[cfg(not(miri))]
const N_PRIO: usize = 256;

#[test]
fn pick_du_256_muc_uu_tien_cao_nhat_duoc_chon() {
    let mut m = Machine::with_arena_bytes(1 << 20);
    let c0 = m.add_cnode(512);
    // KT-20 (reviewer mục 1): SC mặc định test-only cho mọi TCB cần chạy (không có `tick`
    // trong test này nên `consumed` luôn 0). Slot 256 = SchedControl, 257 = SchedContext.
    let ctl = m.add_kernel_object(ObjType::SchedControl);
    let cap = m.root_cap(ctl);
    m.put_cap(c0, 256, cap).expect("cap SchedControl");
    let sc = m.add_kernel_object(ObjType::SchedContext);
    let cap = m.root_cap(sc);
    m.put_cap(c0, 257, cap).expect("cap SchedContext");
    m.sched_control_configure(slot(c0, 256), slot(c0, 257), SC_MAC_DINH, SC_MAC_DINH)
        .expect("SchedControl.Configure (test-only)");
    let mut tcb = Vec::with_capacity(N_PRIO);
    let mut sl = Vec::with_capacity(N_PRIO);
    for p in 0..N_PRIO {
        let t = m.add_kernel_object(ObjType::Tcb);
        let cap = m.root_cap(t);
        m.put_cap(c0, p, cap).expect("cap TCB");
        let sr = slot(c0, p);
        gan_sc_mac_dinh(&mut m, c0, p, 257);
        m.tcb_set_priority(sr, p as u8, 255).expect("priority p");
        m.transition_tcb(t, TcbState::Ready).expect("Ready");
        tcb.push(t);
        sl.push(sr);
    }
    kiem(&m, "đủ các mức Ready");
    assert_eq!(m.ready_len(0), N_PRIO, "đủ {N_PRIO} TCB trong hàng");
    assert_eq!(m.current(0), None);
    assert!(m.sched_idle(0), "chưa dispatch ⇒ hart rảnh");
    assert_eq!(
        m.sched_pick(),
        Some(tcb[N_PRIO - 1]),
        "mức cao nhất ({}) phải được chọn trước",
        N_PRIO - 1
    );

    // Rút từ cao xuống: dispatch mức cao nhất, rồi Suspend(Abort) đưa nó ra khỏi hàng
    // (Ready + suspended — bảng §4.5). Như vậy kiểm **cả N_PRIO mức** (stable 256) theo
    // đúng thứ tự từ cao xuống.
    for p in (0..N_PRIO).rev() {
        assert_eq!(
            m.sched_pick(),
            Some(tcb[p]),
            "mức cao nhất còn lại phải là {p}"
        );
        let got = m.dispatch(0).expect("dispatch").expect("phải có TCB");
        assert_eq!(got, tcb[p], "dispatch phải chọn đúng mức {p}");
        assert_eq!(m.current(0), Some(tcb[p]));
        assert_eq!(m.sched_cur_prio(0), p as u8);
        assert!(!m.sched_idle(0));
        assert!(
            !m.ready_contains(0, tcb[p]),
            "TCB đang Running không được nằm trong hàng"
        );
        assert_eq!(m.tcb_state(tcb[p]), Ok(TcbState::Running { hart: 0 }));
        m.tcb_suspend(sl[p], SuspendMode::Abort).expect("Suspend");
        kiem(&m, "suspend TCB đang chạy");
        assert_eq!(
            m.tcb_state(tcb[p]),
            Ok(TcbState::Ready),
            "Suspend(Abort) trên Running ⇒ Ready (+ suspended)"
        );
        assert!(
            m.tcb_info(sl[p]).expect("TcbInfo").suspended,
            "cờ suspended trực giao với state"
        );
        assert_eq!(m.ready_len(0), p, "sau suspend mức {p}, hàng còn {p} TCB");
        assert_eq!(m.current(0), None, "không còn TCB Running");
        assert!(m.sched_idle(0));
    }
    assert_eq!(m.sched_pick(), None);
    assert_eq!(m.ready_len(0), 0);
}

#[test]
fn cung_muc_fifo_va_yield_quay_vong() {
    let mut r = Rig::new();
    for i in 0..3 {
        r.set_prio(i, 7);
        r.ready(i);
    }
    assert_eq!(r.m.ready_len(0), 3);
    assert_eq!(
        r.m.sched_pick(),
        Some(r.t[0]),
        "FIFO: TCB wake trước đứng đầu"
    );

    assert_eq!(r.dispatch(0), Some(r.t[0]));
    assert_eq!(
        r.dispatch(0),
        Some(r.t[1]),
        "dispatch tự preempt TCB đang chạy"
    );
    assert_eq!(
        r.m.yield_current(0).expect("yield"),
        Some(r.t[2]),
        "yield đưa TCB hiện tại về đuôi cùng mức"
    );
    assert_eq!(r.m.yield_current(0).expect("yield"), Some(r.t[0]));
    assert_eq!(r.m.yield_current(0).expect("yield"), Some(r.t[1]));
    assert_eq!(
        dem_running(&r.m, &r.t).len(),
        1,
        "luôn đúng một TCB Running trên hart 0"
    );
}

// ---------------------------------------------------------------------------
// 2. Dispatch: `Running→Ready` (enqueue) rồi `Ready→Running`; ≤1 Running/hart;
//    `Ready` ⇔ trong hàng + `!suspended`; tôn trọng priority (DoD 2+7).
// ---------------------------------------------------------------------------

#[test]
fn dispatch_duy_nhat_mot_running_va_ton_trong_uu_tien() {
    let mut r = Rig::new();
    r.set_prio(0, 5); // A
    r.set_prio(1, 5); // B
    r.set_prio(2, 9); // C
    for i in 0..3 {
        r.ready(i);
    }
    assert_eq!(r.m.sched_pick(), Some(r.t[2]), "mức 9 > 5");

    assert_eq!(r.dispatch(0), Some(r.t[2]));
    assert_eq!(r.m.ready_len(0), 2);
    assert!(!r.m.ready_contains(0, r.t[2]));
    assert_eq!(dem_running(&r.m, &r.t), vec![r.t[2]]);

    // Cùng mức cao nhất: dispatch preempt TCB đang chạy rồi chọn lại chính nó (sticky).
    assert_eq!(r.dispatch(0), Some(r.t[2]), "mức 9 vẫn cao nhất");
    assert_eq!(r.m.ready_len(0), 2);

    // TCB mới mức 10 xuất hiện ⇒ preempt C.
    let d = r.m.add_kernel_object(ObjType::Tcb);
    let dcap = r.m.root_cap(d);
    r.m.put_cap(r.c0, 11, dcap).expect("cap D");
    gan_sc_mac_dinh(&mut r.m, r.c0, 11, S_SC);
    r.m.tcb_set_priority(slot(r.c0, 11), 10, 255)
        .expect("priority D");
    r.m.transition_tcb(d, TcbState::Ready).expect("D Ready");
    kiem(&r.m, "D Ready");
    assert_eq!(r.dispatch(0), Some(d), "mức 10 phải preempt mức 9");
    assert_eq!(r.m.tcb_state(r.t[2]), Ok(TcbState::Ready));
    assert!(r.m.ready_contains(0, r.t[2]), "C bị preempt ⇒ về hàng");
    assert_eq!(dem_running(&r.m, &[r.t[0], r.t[1], r.t[2], d]).len(), 1);

    // Hạ D xuống mức 4 ⇒ C (9) lên lại.
    r.m.tcb_set_priority(slot(r.c0, 11), 4, 255)
        .expect("hạ priority D");
    kiem(&r.m, "hạ priority D");
    assert_eq!(r.dispatch(0), Some(r.t[2]));
}

// ---------------------------------------------------------------------------
// 3. `check_sched` sau **mỗi** public op của mô hình (DoD 3).
// ---------------------------------------------------------------------------

#[test]
fn check_sched_sau_moi_op_cong_khai() {
    let mut r = Rig::new();

    // Cấu hình TCB (các model API `[TK]` của KT-18).
    r.m.tcb_configure(r.s[0], Some(slot(r.c0, S_CNODE)))
        .expect("Tcb.Configure");
    kiem(&r.m, "Tcb.Configure");
    r.m.tcb_set_fault_ep(r.s[1], slot(r.c0, S_EP))
        .expect("Tcb.SetFaultEp");
    kiem(&r.m, "Tcb.SetFaultEp");
    r.m.set_sched_params(r.s[0], Some(slot(r.c0, S_SC)))
        .expect("set_sched_params");
    kiem(&r.m, "set_sched_params");
    r.m.tcb_bind_notification(r.s[0], slot(r.c0, S_NTFN))
        .expect("tcb_bind_notification");
    kiem(&r.m, "tcb_bind_notification");
    r.m.tcb_set_affinity(r.s[0], 1).expect("SetAffinity");
    kiem(&r.m, "Tcb.SetAffinity");
    r.m.tcb_set_home(r.s[0], 0).expect("SetHome");
    kiem(&r.m, "Tcb.SetHome");

    // IPC: Call không có người nhận ⇒ BlockedOnSend.
    r.m.transition_tcb(r.t[0], TcbState::Ready).expect("Ready");
    kiem(&r.m, "transition Ready");
    r.m.tcb_set_priority(r.s[0], 5, 9).expect("priority");
    kiem(&r.m, "SetPriority");
    assert_eq!(r.dispatch(0), Some(r.t[0]));
    r.m.call(r.t[0], slot(r.c0, S_EP), 42).expect("Call");
    kiem(&r.m, "Call");
    assert_eq!(r.m.ready_len(0), 0, "Call chặn client ⇒ không còn Ready");
    assert_eq!(r.dispatch(0), None, "hàng rỗng ⇒ idle");
    assert!(r.m.sched_idle(0));

    // Server Recv nhận message ⇒ client BlockedOnReply, server Ready.
    r.m.recv(r.t[1], slot(r.c0, S_EP), Some(slot(r.c0, S_REP)))
        .expect("Recv");
    kiem(&r.m, "Recv");
    assert_eq!(
        r.m.tcb_state(r.t[0]),
        Ok(TcbState::BlockedOnReply { reply: r.rep })
    );
    assert!(r.m.ready_contains(0, r.t[1]), "server Ready phải vào hàng");
    assert_eq!(r.dispatch(0), Some(r.t[1]));
    assert_eq!(r.m.tcb_state(r.t[1]), Ok(TcbState::Running { hart: 0 }));

    // Reply: đánh thức client, server về Ready.
    r.m.reply(r.t[1], slot(r.c0, S_REP)).expect("Reply");
    kiem(&r.m, "Reply");
    assert_eq!(r.m.tcb_state(r.t[0]), Ok(TcbState::Ready));
    assert!(r.m.ready_contains(0, r.t[0]));
    assert_eq!(
        r.dispatch(0),
        Some(r.t[0]),
        "client mức 5 thắng server mức 0"
    );

    // Suspend/Resume: gỡ khỏi hàng rồi đưa lại đúng mức.
    r.m.tcb_suspend(r.s[0], SuspendMode::Abort)
        .expect("Suspend");
    kiem(&r.m, "Suspend Abort");
    assert_eq!(r.m.tcb_state(r.t[0]), Ok(TcbState::Ready));
    assert!(
        !r.m.ready_contains(0, r.t[0]),
        "suspended không nằm trong hàng"
    );
    r.m.tcb_resume(r.s[0]).expect("Resume");
    kiem(&r.m, "Resume");
    assert!(
        r.m.ready_contains(0, r.t[0]),
        "Resume trên Ready ⇒ enqueue lại"
    );

    // Wait notification / address rồi Wake (model API `[TK]`).
    r.m.tcb_wait_notification(r.s[2], slot(r.c0, S_NTFN))
        .expect("Wait notification");
    kiem(&r.m, "Wait notification");
    assert_eq!(
        r.m.tcb_state(r.t[2]),
        Ok(TcbState::BlockedOnNotification { ntfn: r.ntfn })
    );
    r.m.wake(r.t[2]).expect("wake notification");
    kiem(&r.m, "wake notification");
    assert_eq!(r.m.tcb_state(r.t[2]), Ok(TcbState::Ready));
    assert!(r.m.ready_contains(0, r.t[2]));

    r.m.tcb_wait_address(r.s[2], r.ep, 16).expect("WaitAddr");
    kiem(&r.m, "WaitAddr");
    assert_eq!(
        r.m.tcb_state(r.t[2]),
        Ok(TcbState::BlockedOnAddress {
            frame: r.ep,
            offset: 16
        })
    );
    assert_eq!(r.m.address_waiter(r.ep, 16), Some(r.t[2]));
    r.m.wake(r.t[2]).expect("wake address");
    kiem(&r.m, "wake address");
    assert_eq!(
        r.m.address_waiter(r.ep, 16),
        None,
        "wake phải gỡ khỏi bucket"
    );

    // Exit: không fault_ep ⇒ Inactive (không nằm trong hàng).
    r.m.exit(r.t[2], 7).expect("Exit");
    kiem(&r.m, "Exit");
    assert_eq!(r.m.tcb_state(r.t[2]), Ok(TcbState::Inactive));
    assert!(!r.m.ready_contains(0, r.t[2]));

    // Finalise.
    r.m.finalise_tcb(r.t[2]).expect("finalise");
    kiem(&r.m, "finalise_tcb");
    assert_eq!(r.m.tcb_state(r.t[2]), Ok(TcbState::Dead));
}

// ---------------------------------------------------------------------------
// 4. Preempt ép tại **mọi** bước: 2 TCB cùng mức chạy xen kẽ, không đói (DoD 4).
// ---------------------------------------------------------------------------

#[test]
fn preempt_moi_buoc_hai_tcb_cung_muc_xen_ke_khong_doi() {
    let mut r = Rig::new();
    r.set_prio(0, 5);
    r.set_prio(1, 5);
    r.ready(0);
    r.ready(1);
    r.m.set_preempt_every_step(true);
    assert!(r.m.preempt_every_step(), "cờ preempt phải bật");
    assert_eq!(r.dispatch(0), Some(r.t[0]));

    let mut luot: Vec<ObjId> = Vec::new();
    for step in 0..40 {
        let cur = r.m.current(0).expect("phải có TCB đang chạy");
        let cur_slot = if cur == r.t[0] { r.s[0] } else { r.s[1] };
        // Op **bất biến** (đặt lại đúng mức) để kích preempt sau **mỗi** bước: mỗi
        // public op kết thúc bằng preempt + `pick` (thiết kế §4.13).
        r.m.tcb_set_priority(cur_slot, 5, 255)
            .expect("set_priority");
        kiem(&r.m, "sau preempt");
        let next = r.m.current(0).expect("preempt xong phải có TCB khác chạy");
        assert_ne!(next, cur, "bước {step}: preempt phải đổi TCB cùng mức");
        assert_eq!(dem_running(&r.m, &r.t).len(), 1);
        luot.push(next);
    }
    let a = luot.iter().filter(|t| **t == r.t[0]).count();
    let b = luot.iter().filter(|t| **t == r.t[1]).count();
    assert_eq!(a + b, 40);
    assert!(
        a >= 19 && b >= 19,
        "không TCB nào bị bỏ đói quá N=2 dispatch (a={a}, b={b})"
    );
}

/// Số seed của test ngẫu nhiên: **1.000** trên stable (DoD 4); dưới Miri chỉ **5** vì Miri
/// chậm hơn ~1000 lần (đo thật: 20 seed = **5 m 46 s**, 1.000 seed sẽ >4 h) — **không** nới
/// ngưỡng của cổng: `gate --fast`/L1 vẫn 1.000 seed, Miri là lệnh kiểm bộ nhớ **rời**
/// (D2-47). Cùng seed ⇒ cùng trace và bất biến sau mỗi op vẫn kiểm **đủ** trên 5 seed dưới
/// Miri; không dùng `#[ignore]`.
#[cfg(miri)]
const N_SEED: u64 = 5;
#[cfg(not(miri))]
const N_SEED: u64 = 1000;

/// 1.000 seed × 64 op ngẫu nhiên (call/recv/reply/suspend/resume/exit/retype):
/// cùng seed ⇒ cùng trace; bất biến scheduler giữ sau **mỗi** op (DoD 4).
#[test]
fn mot_nghin_seed_64_op_cung_seed_cung_trace_bat_bien_giu() {
    use std::collections::BTreeSet;

    // Mỗi op ghi **8 số** (không `format!`/`Debug`: dưới Miri định dạng chuỗi đắt gấp
    // nhiều lần bản thân op). `[u64; 8]` = (step, op, ok, current, len, prio, idle,
    // gói 3 state + 3 cờ suspended) — so sánh tất định, đọc được khi in.
    type Row = [u64; 8];

    fn run_seed(seed: u64) -> Vec<Row> {
        let mut r = RandRig::new();
        let mut rng = Lcg::new(seed);
        let mut trace: Vec<Row> = Vec::with_capacity(64);
        for step in 0..64u64 {
            let op = rng.below(7);
            let ti = rng.below(3) as usize;
            let epi = rng.below(2) as usize;
            let repi = rng.below(2) as usize;
            let co_reply = rng.below(2) == 0;
            let ok = match op {
                0 => r.m.call(r.t[ti], r.eps[epi], step).is_ok(),
                1 => {
                    r.m.recv(
                        r.t[ti],
                        r.eps[epi],
                        if co_reply { Some(r.reps[repi]) } else { None },
                    )
                    .is_ok()
                }
                2 => r.m.reply(r.t[ti], r.reps[repi]).is_ok(),
                3 => r.m.tcb_suspend(r.s[ti], SuspendMode::Abort).is_ok(),
                4 => r.m.tcb_resume(r.s[ti]).is_ok(),
                5 => r.m.exit(r.t[ti], step).is_ok(),
                _ => {
                    let _ = r.m.delete(r.spare);
                    match r.m.retype(r.untyped, ObjType::Tcb, 0, 0) {
                        Ok(cap) => {
                            let _ = r.m.put_cap(r.c0, 32, cap);
                            true
                        }
                        Err(_) => false,
                    }
                }
            };
            kiem(&r.m, "op ngẫu nhiên");
            let mut goi = 0u64;
            for (i, t) in r.t.iter().enumerate() {
                let st = r.m.tcb_state(*t).expect("TCB còn sống");
                goi |= (st.index() as u64) << (4 * i);
            }
            for (i, s) in r.s.iter().enumerate() {
                let sus = r.m.tcb_info(*s).expect("TcbInfo").suspended;
                goi |= (sus as u64) << (12 + i);
            }
            trace.push([
                step,
                op,
                ok as u64,
                r.m.current(0).unwrap_or(0) as u64,
                r.m.ready_len(0) as u64,
                r.m.sched_cur_prio(0) as u64,
                r.m.sched_idle(0) as u64,
                goi,
            ]);
        }
        trace
    }

    fn hash(rows: &[Row]) -> u64 {
        let mut h: u64 = 0xcbf29ce484222325;
        for row in rows {
            for v in row {
                for b in v.to_le_bytes() {
                    h ^= b as u64;
                    h = h.wrapping_mul(0x100000001b3);
                }
            }
        }
        h
    }

    let nguong = (N_SEED * 99 / 100) as usize; // 990/1000 (stable), 4/5 (Miri)
    let mut hashes = BTreeSet::new();
    for seed in 0..N_SEED {
        let a = run_seed(seed);
        let b = run_seed(seed);
        assert_eq!(a, b, "seed {seed}: cùng seed phải cùng trace");
        assert_eq!(a.len(), 64, "seed {seed}: đúng 64 op");
        hashes.insert(hash(&a));
    }
    assert!(
        hashes.len() >= nguong,
        "seed phải tác động quỹ đạo scheduler: chỉ {} trace khác nhau/{N_SEED}",
        hashes.len()
    );
    println!(
        "# KT-19 OK: {N_SEED} seed × 64 op, {} trace khác nhau",
        hashes.len()
    );
}

// ---------------------------------------------------------------------------
// 5. `block_current`/`wake`/`yield_current` qua `transition_tcb`; wake TCB suspended
//    không enqueue; resume TCB Ready enqueue đúng priority (DoD 5).
// ---------------------------------------------------------------------------

#[test]
fn wake_suspended_khong_enqueue_resume_enqueue_dung_priority() {
    let mut r = Rig::new();
    r.set_prio(0, 9);
    r.ready(0);
    assert_eq!(r.dispatch(0), Some(r.t[0]));

    // Suspend khi đang Running ⇒ Ready + suspended, ra khỏi hàng.
    r.m.tcb_suspend(r.s[0], SuspendMode::Abort)
        .expect("Suspend");
    kiem(&r.m, "Suspend");
    assert_eq!(r.m.tcb_state(r.t[0]), Ok(TcbState::Ready));
    assert!(r.m.tcb_info(r.s[0]).expect("info").suspended);
    assert_eq!(r.m.ready_len(0), 0);
    assert_eq!(r.m.current(0), None);

    // Wake TCB đang suspended: state Ready giữ nguyên, **không** enqueue.
    assert_eq!(r.m.wake(r.t[0]), Ok(()));
    kiem(&r.m, "wake suspended");
    assert_eq!(r.m.ready_len(0), 0, "wake TCB suspended không được enqueue");
    assert!(!r.m.ready_contains(0, r.t[0]));

    // Resume TCB Ready ⇒ enqueue đúng priority 9.
    r.m.tcb_resume(r.s[0]).expect("Resume");
    kiem(&r.m, "Resume");
    assert_eq!(r.m.ready_len(0), 1);
    assert!(r.m.ready_contains(0, r.t[0]));
    assert_eq!(r.m.sched_pick(), Some(r.t[0]));
    assert_eq!(r.dispatch(0), Some(r.t[0]));
    assert_eq!(r.m.sched_cur_prio(0), 9);

    // block_current gỡ TCB đang chạy khỏi `current` + gỡ khỏi hàng chờ endpoint.
    let blocked =
        r.m.block_current(
            0,
            TcbState::BlockedOnRecv {
                ep: r.ep,
                reply: None,
            },
        )
        .expect("block_current");
    assert_eq!(blocked, r.t[0]);
    kiem(&r.m, "block_current");
    assert_eq!(r.m.current(0), None);
    assert!(r.m.sched_idle(0));
    assert!(!r.m.ready_contains(0, r.t[0]));

    // Wake một TCB đang Recv: gỡ khỏi hàng chờ endpoint + enqueue Ready.
    r.m.recv(r.t[1], slot(r.c0, S_EP), None).expect("Recv");
    kiem(&r.m, "Recv");
    assert_eq!(r.m.endpoint_recv_waiter(r.ep), Ok(Some(r.t[1])));
    r.m.wake(r.t[1]).expect("wake recv");
    kiem(&r.m, "wake recv");
    assert_eq!(r.m.tcb_state(r.t[1]), Ok(TcbState::Ready));
    assert_eq!(
        r.m.endpoint_recv_waiter(r.ep),
        Ok(None),
        "wake phải gỡ khỏi hàng Recv"
    );
    assert!(r.m.ready_contains(0, r.t[1]));
}

#[test]
fn block_current_va_yield_current_doi_current() {
    let mut r = Rig::new();
    r.set_prio(0, 5);
    r.set_prio(1, 5);
    r.ready(0);
    r.ready(1);
    assert_eq!(r.dispatch(0), Some(r.t[0]));

    let blocked =
        r.m.block_current(
            0,
            TcbState::BlockedOnSend {
                ep: r.ep,
                call: true,
                fault: None,
            },
        )
        .expect("block_current");
    assert_eq!(blocked, r.t[0]);
    kiem(&r.m, "block_current");
    assert_eq!(
        r.m.tcb_state(r.t[0]),
        Ok(TcbState::BlockedOnSend {
            ep: r.ep,
            call: true,
            fault: None
        })
    );
    assert_eq!(r.m.current(0), None);

    assert_eq!(r.dispatch(0), Some(r.t[1]));
    assert_eq!(
        r.m.yield_current(0).expect("yield"),
        Some(r.t[1]),
        "hàng chỉ còn B"
    );
    assert_eq!(r.m.tcb_state(r.t[1]), Ok(TcbState::Running { hart: 0 }));
    assert_eq!(r.m.yield_current(0).expect("yield"), Some(r.t[1]));
    assert_eq!(r.m.wake(r.t[0]), Ok(()));
    kiem(&r.m, "wake A");
    assert!(r.m.ready_contains(0, r.t[0]));
    // A vào hàng trước ⇒ yield B đưa B về sau A; A (cùng mức, FIFO) thắng.
    assert_eq!(r.m.yield_current(0).expect("yield"), Some(r.t[0]));
    assert_eq!(r.m.yield_current(0).expect("yield"), Some(r.t[1]));
    assert_eq!(r.m.tcb_state(r.t[1]), Ok(TcbState::Running { hart: 0 }));

    // `block_current` chỉ nhận trạng thái chặn/ngủ — không cho biến Running thành Ready.
    assert_eq!(
        r.m.block_current(0, TcbState::Ready),
        Err(KError::InvalidArgument)
    );
    kiem(&r.m, "block_current(Ready) bị từ chối");
    // Chặn TCB đang chạy rồi `yield_current` khi hart rảnh ⇒ IllegalOperation.
    assert_eq!(
        r.m.block_current(0, TcbState::BlockedOnNotification { ntfn: r.ntfn }),
        Ok(r.t[1])
    );
    kiem(&r.m, "block_current(Notification)");
    assert_eq!(r.m.yield_current(0), Err(KError::IllegalOperation));
}

// ---------------------------------------------------------------------------
// 6. Finalise/suspend gỡ khỏi hàng; `current` được thay; không ObjId chết trong
//    hàng; `finalise_tcb` hook ready queue (DoD 6).
// ---------------------------------------------------------------------------

#[test]
fn finalise_va_suspend_go_ready_queue() {
    let mut r = Rig::new();
    r.set_prio(0, 5);
    r.set_prio(1, 5);
    r.ready(0);
    r.ready(1);
    assert_eq!(r.m.ready_len(0), 2);
    assert_eq!(r.dispatch(0), Some(r.t[0]));
    assert_eq!(r.m.ready_len(0), 1);

    // Finalise TCB đang Running: `current` được thay (về None) và không còn trong hàng.
    r.m.finalise_tcb(r.t[0]).expect("finalise TCB đang chạy");
    kiem(&r.m, "finalise TCB đang chạy");
    assert_eq!(r.m.tcb_state(r.t[0]), Ok(TcbState::Dead));
    assert_eq!(r.m.current(0), None);
    assert_eq!(r.m.ready_len(0), 1, "chỉ TCB B còn trong hàng");
    assert!(!r.m.ready_contains(0, r.t[0]));
    assert_eq!(r.m.sched_pick(), Some(r.t[1]));
    assert_eq!(
        r.dispatch(0),
        Some(r.t[1]),
        "current được thay bằng TCB kế tiếp"
    );

    // Suspend(Abort) TCB đang Running: hủy hàng + ↔ Ready+suspended.
    r.m.tcb_suspend(r.s[1], SuspendMode::Abort)
        .expect("Suspend");
    kiem(&r.m, "Suspend TCB đang chạy");
    assert_eq!(r.m.ready_len(0), 0);
    assert_eq!(r.m.current(0), None);
    assert_eq!(r.dispatch(0), None);

    // Revoke Untyped: finalise_tcb phải gỡ TCB khỏi ready queue trước khi object chết.
    let mut m = Machine::new();
    let c0 = m.add_cnode(16);
    let u0 = m.add_untyped(0x1000, 65536);
    let ucap = m.root_cap(u0);
    m.put_cap(c0, 0, ucap).expect("cap Untyped");
    let mut t = [0u32; 2];
    for (i, item) in t.iter_mut().enumerate() {
        let cap = m
            .retype(slot(c0, 0), ObjType::Tcb, 0, 0)
            .expect("Retype TCB");
        m.put_cap(c0, 1 + i, cap).expect("cap TCB");
        *item = m.cap_obj(cap).expect("obj TCB");
        m.transition_tcb(*item, TcbState::Ready).expect("Ready");
    }
    kiem(&m, "2 TCB Ready trước Revoke");
    assert_eq!(m.ready_len(0), 2);
    m.revoke(c0, Some(0)).expect("Revoke Untyped");
    kiem(&m, "Revoke Untyped");
    assert_eq!(m.finalise_log().len(), 2, "finalise hook chạy cho cả 2 TCB");
    assert_eq!(m.ready_len(0), 0, "queue giảm đúng sau finalise");
    assert_eq!(m.current(0), None);
    assert_eq!(m.dispatch(0), Ok(None), "không còn TCB sống trong hàng");
    for id in t {
        assert!(
            !m.ready_contains(0, id),
            "ObjId chết không được nằm trong hàng"
        );
    }
}

// ---------------------------------------------------------------------------
// 7. Mặc định affinity/home + đổi priority khi đang ở trong hàng phải rebucket
//    (DoD 7 + bất biến "bucket khớp priority").
// ---------------------------------------------------------------------------

#[test]
fn mac_dinh_affinity_home_va_rebucket_priority() {
    let mut r = Rig::new();
    let info = r.m.tcb_info(r.s[0]).expect("TcbInfo");
    assert_eq!(info.priority, 0);
    assert_eq!(info.affinity, 1, "affinity mặc định full mask của 1 hart");
    assert_eq!(info.home, 0, "home mặc định hart 0");

    r.set_prio(0, 5);
    r.set_prio(1, 7);
    r.ready(0);
    r.ready(1);
    assert_eq!(r.m.sched_pick(), Some(r.t[1]), "mức 7 > 5");

    // Hạ B xuống 3 khi đang trong hàng ⇒ B phải sang bucket 3.
    r.set_prio(1, 3);
    kiem(&r.m, "rebucket B 7→3");
    assert_eq!(r.m.sched_pick(), Some(r.t[0]));
    assert_eq!(r.m.ready_len(0), 2);

    // Đưa A xuống 3 khi đang trong hàng ⇒ cùng mức, FIFO: B (vào trước) thắng.
    r.set_prio(0, 3);
    kiem(&r.m, "rebucket A 5→3");
    assert_eq!(r.m.sched_pick(), Some(r.t[1]));
    assert_eq!(r.dispatch(0), Some(r.t[1]));
    assert_eq!(r.dispatch(0), Some(r.t[0]));

    // Priority của TCB đang Running phải cập nhật `cur_prio`.
    r.set_prio(0, 42);
    kiem(&r.m, "priority TCB đang Running");
    assert_eq!(r.m.current(0), Some(r.t[0]));
    assert_eq!(r.m.sched_cur_prio(0), 42);
}

// ---------------------------------------------------------------------------
// Rig cho test 1.000 seed: 2 Endpoint + 2 Reply + 3 TCB + Untyped (để Retê).
// ---------------------------------------------------------------------------

struct RandRig {
    m: Machine,
    c0: ObjId,
    eps: [SlotRef; 2],
    reps: [SlotRef; 2],
    t: [ObjId; 3],
    s: [SlotRef; 3],
    spare: SlotRef,
    untyped: SlotRef,
}

impl RandRig {
    fn new() -> Self {
        let mut m = Machine::new();
        let c0 = m.add_cnode(64);
        let u0 = m.add_untyped(0x1000, 65536);
        let ep1 = m.add_kernel_object(ObjType::Endpoint);
        let ep2 = m.add_kernel_object(ObjType::Endpoint);
        let rep1 = m.add_kernel_object(ObjType::Reply);
        let rep2 = m.add_kernel_object(ObjType::Reply);
        for (i, obj) in [(0usize, u0), (1, ep1), (2, ep2), (3, rep1), (4, rep2)] {
            let cap = m.root_cap(obj);
            m.put_cap(c0, i, cap).expect("cap object");
        }
        // KT-20 (reviewer mục 1): SC mặc định test-only để TCB thực sự chạy trong 1.000 seed.
        let ctl = m.add_kernel_object(ObjType::SchedControl);
        let cap = m.root_cap(ctl);
        m.put_cap(c0, 5, cap).expect("cap SchedControl");
        let sc = m.add_kernel_object(ObjType::SchedContext);
        let cap = m.root_cap(sc);
        m.put_cap(c0, 6, cap).expect("cap SchedContext");
        m.sched_control_configure(slot(c0, 5), slot(c0, 6), SC_MAC_DINH, SC_MAC_DINH)
            .expect("SchedControl.Configure (test-only)");
        let mut t = [0u32; 3];
        let mut s = [slot(c0, 8), slot(c0, 9), slot(c0, 10)];
        for i in 0..3 {
            let tcb = m.add_kernel_object(ObjType::Tcb);
            let cap = m.root_cap(tcb);
            m.put_cap(c0, 8 + i, cap).expect("cap TCB");
            t[i] = tcb;
            s[i] = slot(c0, 8 + i);
            gan_sc_mac_dinh(&mut m, c0, 8 + i, 6);
        }
        // Bật preempt ép tại mọi bước (thiết kế §4.13) cho quỹ đạo ngẫu nhiên.
        m.set_preempt_every_step(true);
        RandRig {
            m,
            c0,
            eps: [slot(c0, 1), slot(c0, 2)],
            reps: [slot(c0, 3), slot(c0, 4)],
            t,
            s,
            spare: slot(c0, 32),
            untyped: slot(c0, 0),
        }
    }
}
