// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-64 — **parity sim↔core** cho ba mảng có mô hình ở sim: `SchedContext.Bind`/`Signal`,
//! `tcb_resume_with_regs`, và **`TlbFlush`** (KT-64a, D2-92c: `seq/targets/pending/acked/Busy`).
//!
//! Cùng một rig slot (cùng thứ tự tạo object ⇒ cùng `ObjId`), cùng một dãy op; sau **từng bước**
//! so (a) **mã lỗi ABI** (`KError::id()`) và (b) **digest** trạng thái quan sát được. Mảng
//! `SlotPool`→arena **không** có parity ở đây (sim giữ slot CNode bằng heap riêng, core dùng
//! byte arena của Untyped) — lý do ghi trong `docs/k3b-core.md` §13; mảng đó pin bằng
//! `kt64_core_tlb_bind_resume.rs` và TAP QEMU.
//!
//! **KT-64a** — `TlbFlush` **đã** có parity (kịch bản C): sim có giao thức shootdown
//! (`tlb_shootdown_begin/wait/poll`, `tlb_pending`, mask ack — KT-40) nên `seq/targets/pending/
//! acked/Busy` so được với core (D2-92c). Hai bên dùng **cùng một định nghĩa** cho ảnh chụp
//! (xem `targets_of`/`sim_nut_ack`): `targets` = tập hart **phải** ack (trừ hart gọi, D2-78
//! `tlb::targets`), `pending[hart]` = `Some(seq)` ⇔ hart còn **nợ** ack của `seq` — core giữ
//! một ô `tlb_pending`/hart, sim tách hai vai (caller-side `tlb_pending` + `tlb_pending_ack`).
//!
//! `Unbind` cũng **không** có ở sim K1–K2 (chỉ `Bind`); core cài theo `spec/abi.toml` và pin
//! core-only — ghi rõ trong §13.

use kc::{KError as CoreErr, Machine as CoreMachine, ObjId as CoreObj};
use vifast_core as kc;
use vifast_sim::kobj::{
    FaultKind as SimFault, KError as SimErr, Machine as SimMachine, NonReadyTarget as SimTarget,
    ObjId as SimObj, SlotRef as SimSlot, TcbState as SimState,
};

fn sslot(c: u32, s: usize) -> SimSlot {
    SimSlot::new(c, s)
}
fn cslot(c: u32, s: usize) -> kc::SlotRef {
    kc::SlotRef::new(c, s)
}

fn sim_msg(label: u32, words: &[u64]) -> vifast_sim::ipc::Message {
    let info =
        vifast_sim::ipc::MessageInfo::new(label, 0, false, 0, words.len() as u8).expect("info sim");
    vifast_sim::ipc::Message::new(info, words).expect("message sim")
}
fn core_msg(label: u32, words: &[u64]) -> kc::Message {
    let info = kc::MessageInfo::new(label, 0, false, 0, words.len() as u8).expect("info core");
    kc::Message::new(info, words).expect("message core")
}

/// Nhãn trạng thái TCB ổn định (hai enum khác crate, cùng 11 biến thể).
fn sim_state(s: SimState) -> u8 {
    match s {
        SimState::Inactive => 0,
        SimState::Ready => 1,
        SimState::Running { .. } => 2,
        SimState::BlockedOnSend { .. } => 3,
        SimState::BlockedOnRecv { .. } => 4,
        SimState::BlockedOnReply { .. } => 5,
        SimState::BlockedOnNotification { .. } => 6,
        SimState::BlockedOnAddress { .. } => 7,
        SimState::Sleeping { .. } => 8,
        SimState::Stopping => 9,
        SimState::Dead => 10,
    }
}
fn core_state(s: kc::TcbState) -> u8 {
    match s {
        kc::TcbState::Inactive => 0,
        kc::TcbState::Ready => 1,
        kc::TcbState::Running { .. } => 2,
        kc::TcbState::BlockedOnSend { .. } => 3,
        kc::TcbState::BlockedOnRecv { .. } => 4,
        kc::TcbState::BlockedOnReply { .. } => 5,
        kc::TcbState::BlockedOnNotification { .. } => 6,
        kc::TcbState::BlockedOnAddress { .. } => 7,
        kc::TcbState::Sleeping { .. } => 8,
        kc::TcbState::Stopping => 9,
        kc::TcbState::Dead => 10,
    }
}

fn sim_fault_tag(k: SimFault) -> u8 {
    match k {
        SimFault::Exit { .. } => 0,
        SimFault::Vm { .. } => 1,
        SimFault::UnknownSyscall { .. } => 2,
        SimFault::UserException { .. } => 3,
    }
}
fn core_fault_tag(k: kc::FaultKind) -> u8 {
    match k {
        kc::FaultKind::Exit { .. } => 0,
        kc::FaultKind::Vm { .. } => 1,
        kc::FaultKind::UnknownSyscall { .. } => 2,
        kc::FaultKind::UserException { .. } => 3,
    }
}

/// Mã lỗi ABI (1..19) hoặc 0 khi `Ok` — nguồn so duy nhất của "cùng mã lỗi".
fn rc_sim<T>(r: Result<T, SimErr>) -> u16 {
    match r {
        Ok(_) => 0,
        Err(e) => e.id(),
    }
}
fn rc_core<T>(r: Result<T, CoreErr>) -> u16 {
    match r {
        Ok(_) => 0,
        Err(e) => e.id(),
    }
}

// ===========================================================================================
// Kịch bản A — SchedContext.Bind + Signal: 1 CNode 64 slot; 2 SC; 2 Notification; 1 Reply;
// 1 Endpoint; 2 TCB. Slot cố định để hai bên tạo object cùng thứ tự.
// ===========================================================================================

const A_SC: [usize; 2] = [2, 3];
const A_N: [usize; 2] = [4, 5];
const A_REP: usize = 6;
const A_EP: usize = 7;
const A_T: [usize; 2] = [10, 11];
const A_EP_RECV: usize = 9;

struct SimBind {
    m: SimMachine,
    c0: SimObj,
    n_obj: [SimObj; 2],
    t: [SimObj; 2],
    t_s: [SimSlot; 2],
    checks: [bool; 3],
}

struct CoreBind {
    m: CoreMachine,
    c0: CoreObj,
    n_obj: [CoreObj; 2],
    t: [CoreObj; 2],
    t_s: [kc::SlotRef; 2],
    checks: [bool; 3],
}

fn sim_bind() -> SimBind {
    let mut m = SimMachine::new();
    let c0 = m.add_cnode(64);
    for (i, s) in A_SC.iter().enumerate() {
        let sc = m.add_kernel_object(vifast_sim::kobj::ObjType::SchedContext);
        let cap = m.root_cap(sc);
        m.put_cap(c0, *s, cap).expect("cap SC");
        let _ = i;
    }
    let mut n_obj = [0u32; 2];
    for (i, s) in A_N.iter().enumerate() {
        let n = m.add_kernel_object(vifast_sim::kobj::ObjType::Notification);
        let cap = m.root_cap(n);
        m.put_cap(c0, *s, cap).expect("cap Notification");
        n_obj[i] = n;
    }
    let rep = m.add_kernel_object(vifast_sim::kobj::ObjType::Reply);
    let rcap = m.root_cap(rep);
    m.put_cap(c0, A_REP, rcap).expect("cap Reply");
    let ep = m.add_kernel_object(vifast_sim::kobj::ObjType::Endpoint);
    let ecap = m.root_cap(ep);
    m.put_cap(c0, A_EP, ecap).expect("cap Endpoint");
    let ep2 = m.add_kernel_object(vifast_sim::kobj::ObjType::Endpoint);
    let ecap2 = m.root_cap(ep2);
    m.put_cap(c0, A_EP_RECV, ecap2).expect("cap Endpoint recv");
    let mut t = [0u32; 2];
    let mut t_s = [sslot(c0, 0); 2];
    for (i, s) in A_T.iter().enumerate() {
        t[i] = m.add_kernel_object(vifast_sim::kobj::ObjType::Tcb);
        let cap = m.root_cap(t[i]);
        m.put_cap(c0, *s, cap).expect("cap TCB");
        t_s[i] = sslot(c0, *s);
        m.start_tcb(t[i]).expect("Ready");
    }
    SimBind {
        m,
        c0,
        n_obj,
        t,
        t_s,
        checks: [false; 3],
    }
}

fn core_bind() -> CoreBind {
    let mut m = CoreMachine::new();
    let c0 = m.add_cnode(64);
    for s in A_SC {
        let sc = m.add_kernel_object(kc::ObjType::SchedContext);
        let cap = m.root_cap(sc);
        m.put_cap(c0, s, cap).expect("cap SC");
    }
    let mut n_obj = [0u32; 2];
    for (i, s) in A_N.iter().enumerate() {
        let n = m.add_kernel_object(kc::ObjType::Notification);
        let cap = m.root_cap(n);
        m.put_cap(c0, *s, cap).expect("cap Notification");
        n_obj[i] = n;
    }
    let rep = m.add_kernel_object(kc::ObjType::Reply);
    let rcap = m.root_cap(rep);
    m.put_cap(c0, A_REP, rcap).expect("cap Reply");
    let ep = m.add_kernel_object(kc::ObjType::Endpoint);
    let ecap = m.root_cap(ep);
    m.put_cap(c0, A_EP, ecap).expect("cap Endpoint");
    let ep2 = m.add_kernel_object(kc::ObjType::Endpoint);
    let ecap2 = m.root_cap(ep2);
    m.put_cap(c0, A_EP_RECV, ecap2).expect("cap Endpoint recv");
    let mut t = [0u32; 2];
    let mut t_s = [cslot(c0, 0); 2];
    for (i, s) in A_T.iter().enumerate() {
        t[i] = m.add_kernel_object(kc::ObjType::Tcb);
        let cap = m.root_cap(t[i]);
        m.put_cap(c0, *s, cap).expect("cap TCB");
        t_s[i] = cslot(c0, *s);
        m.start_tcb(t[i]).expect("Ready");
    }
    CoreBind {
        m,
        c0,
        n_obj,
        t,
        t_s,
        checks: [false; 3],
    }
}

#[derive(Debug, PartialEq, Eq)]
struct BD {
    n_bound: [Option<u32>; 2],
    n_word: [u64; 2],
    n_badge: [u64; 2],
    n_waiter: [Option<u32>; 2],
    st: [u8; 2],
    regs: [[u64; 4]; 2],
    notif: [[u64; 2]; 2],
    sent: u64,
    delivered: u64,
    checks: [bool; 3],
}

fn sim_digest(r: &SimBind) -> BD {
    BD {
        n_bound: [
            r.m.notification_bound_sc(r.n_obj[0]).unwrap(),
            r.m.notification_bound_sc(r.n_obj[1]).unwrap(),
        ],
        n_word: [
            r.m.notification_word(r.n_obj[0]).unwrap(),
            r.m.notification_word(r.n_obj[1]).unwrap(),
        ],
        n_badge: [
            r.m.notification_badge(r.n_obj[0]).unwrap(),
            r.m.notification_badge(r.n_obj[1]).unwrap(),
        ],
        n_waiter: [
            r.m.notification_waiter(r.n_obj[0]).unwrap(),
            r.m.notification_waiter(r.n_obj[1]).unwrap(),
        ],
        st: [
            sim_state(r.m.tcb_state(r.t[0]).unwrap()),
            sim_state(r.m.tcb_state(r.t[1]).unwrap()),
        ],
        regs: [r.m.tcb_regs(r.t[0]).unwrap(), r.m.tcb_regs(r.t[1]).unwrap()],
        notif: [
            match r.m.tcb_notification(r.t[0]) {
                Ok(Some(v)) => [v.word, v.badge],
                _ => [0, 0],
            },
            match r.m.tcb_notification(r.t[1]) {
                Ok(Some(v)) => [v.word, v.badge],
                _ => [0, 0],
            },
        ],
        sent: r.m.ipc_sent(),
        delivered: r.m.ipc_delivered(),
        checks: r.checks,
    }
}

fn core_digest(r: &CoreBind) -> BD {
    BD {
        n_bound: [
            r.m.notification_bound_sc(r.n_obj[0]).unwrap(),
            r.m.notification_bound_sc(r.n_obj[1]).unwrap(),
        ],
        n_word: [
            r.m.notification_word(r.n_obj[0]).unwrap(),
            r.m.notification_word(r.n_obj[1]).unwrap(),
        ],
        n_badge: [
            r.m.notification_badge(r.n_obj[0]).unwrap(),
            r.m.notification_badge(r.n_obj[1]).unwrap(),
        ],
        n_waiter: [
            r.m.notification_waiter(r.n_obj[0]).unwrap(),
            r.m.notification_waiter(r.n_obj[1]).unwrap(),
        ],
        st: [
            core_state(r.m.tcb_state(r.t[0]).unwrap()),
            core_state(r.m.tcb_state(r.t[1]).unwrap()),
        ],
        regs: [r.m.tcb_regs(r.t[0]).unwrap(), r.m.tcb_regs(r.t[1]).unwrap()],
        notif: [
            match r.m.tcb_notification(r.t[0]) {
                Ok(Some(v)) => [v.word, v.badge],
                _ => [0, 0],
            },
            match r.m.tcb_notification(r.t[1]) {
                Ok(Some(v)) => [v.word, v.badge],
                _ => [0, 0],
            },
        ],
        sent: r.m.ipc_sent(),
        delivered: r.m.ipc_delivered(),
        checks: r.checks,
    }
}

fn buoc_a(s: &mut SimBind, c: &mut CoreBind, i: usize, rs: u16, rc: u16, what: &str) {
    assert_eq!(rs, rc, "bước {i} ({what}): mã lỗi sim {rs} vs core {rc}");
    s.checks = [
        s.m.check_notifications().is_ok(),
        s.m.check_ipc().is_ok(),
        s.m.check_sched().is_ok(),
    ];
    c.checks = [
        c.m.check_notifications().is_ok(),
        c.m.check_ipc().is_ok(),
        c.m.check_sched().is_ok(),
    ];
    assert_eq!(
        sim_digest(s),
        core_digest(c),
        "bước {i} ({what}): digest lệch"
    );
}

/// Parity `Bind` + `Signal`: bind 1:1, vi phạm 1:1, rebind gỡ phía cũ, target sai kiểu, xoá cap
/// Notification, signal chỉ đánh thức TCB **đúng** bound.
#[test]
fn kt64_parity_bind_va_signal() {
    let mut s = sim_bind();
    let mut c = core_bind();

    let r =
        s.m.sched_context_bind(sslot(s.c0, A_SC[0]), sslot(s.c0, A_N[0]));
    let rs = rc_sim(r);
    let rc = rc_core(c.m.sched_context_bind(cslot(c.c0, A_SC[0]), cslot(c.c0, A_N[0])));
    buoc_a(&mut s, &mut c, 1, rs, rc, "bind sc0↔n0");

    let rs = rc_sim(s.m.sched_context_bind(sslot(s.c0, A_SC[1]), sslot(s.c0, A_N[0])));
    let rc = rc_core(c.m.sched_context_bind(cslot(c.c0, A_SC[1]), cslot(c.c0, A_N[0])));
    buoc_a(&mut s, &mut c, 2, rs, rc, "vi phạm 1:1 phía notification");

    let rs = rc_sim(s.m.sched_context_bind(sslot(s.c0, A_SC[0]), sslot(s.c0, A_N[1])));
    let rc = rc_core(c.m.sched_context_bind(cslot(c.c0, A_SC[0]), cslot(c.c0, A_N[1])));
    buoc_a(&mut s, &mut c, 3, rs, rc, "SC đổi sang notification khác");

    let rs = rc_sim(s.m.sched_context_bind(sslot(s.c0, A_SC[0]), sslot(s.c0, A_REP)));
    let rc = rc_core(c.m.sched_context_bind(cslot(c.c0, A_SC[0]), cslot(c.c0, A_REP)));
    buoc_a(&mut s, &mut c, 4, rs, rc, "SC bind Reply");

    let rs = rc_sim(s.m.sched_context_bind(sslot(s.c0, A_SC[0]), sslot(s.c0, A_EP)));
    let rc = rc_core(c.m.sched_context_bind(cslot(c.c0, A_SC[0]), cslot(c.c0, A_EP)));
    buoc_a(&mut s, &mut c, 5, rs, rc, "target sai kiểu");

    // Bind lại Notification + Tcb.BindNotification hai TCB; Recv trên Endpoint riêng.
    let rs = rc_sim(s.m.sched_context_bind(sslot(s.c0, A_SC[0]), sslot(s.c0, A_N[0])));
    let rc = rc_core(c.m.sched_context_bind(cslot(c.c0, A_SC[0]), cslot(c.c0, A_N[0])));
    assert_eq!(rs, rc, "bind lại n0");
    let rs = rc_sim(s.m.tcb_bind_notification(s.t_s[0], sslot(s.c0, A_N[0])));
    let rc = rc_core(c.m.tcb_bind_notification(c.t_s[0], cslot(c.c0, A_N[0])));
    assert_eq!(rs, rc, "t0 bind n0");
    let rs = rc_sim(s.m.tcb_bind_notification(s.t_s[1], sslot(s.c0, A_N[1])));
    let rc = rc_core(c.m.tcb_bind_notification(c.t_s[1], cslot(c.c0, A_N[1])));
    assert_eq!(rs, rc, "t1 bind n1");
    let ep_s = sslot(s.c0, A_EP_RECV);
    let rs = rc_sim(s.m.recv(s.t[0], ep_s, None));
    let rc = rc_core(c.m.recv(c.t[0], cslot(c.c0, A_EP_RECV), None));
    assert_eq!(rs, rc, "t0 recv chặn");
    let rs = rc_sim(s.m.recv(s.t[1], ep_s, None));
    let rc = rc_core(c.m.recv(c.t[1], cslot(c.c0, A_EP_RECV), None));
    buoc_a(&mut s, &mut c, 6, rs, rc, "hai TCB chờ Recv + bound");

    let rs = rc_sim(s.m.notification_signal(s.n_obj[0], 0x42));
    let rc = rc_core(c.m.notification_signal(c.n_obj[0], 0x42));
    buoc_a(
        &mut s,
        &mut c,
        7,
        rs,
        rc,
        "signal n0 chỉ đánh thức đúng bound",
    );
    assert_eq!(sim_state(s.m.tcb_state(s.t[0]).unwrap()), 1, "sim: t0 thức");
    assert_eq!(
        core_state(c.m.tcb_state(c.t[0]).unwrap()),
        1,
        "core: t0 thức"
    );
}

// ===========================================================================================
// Kịch bản B — tcb_resume_with_regs: 1 CNode 64; 1 Endpoint; 1 Reply; 2 TCB.
// ===========================================================================================

const B_EP: usize = 1;
const B_REP: usize = 2;
const B_T: [usize; 2] = [10, 11];

struct SimRes {
    m: SimMachine,
    c0: SimObj,
    rep: SimObj,
    t: [SimObj; 2],
    t_s: [SimSlot; 2],
}

struct CoreRes {
    m: CoreMachine,
    c0: CoreObj,
    rep: CoreObj,
    t: [CoreObj; 2],
    t_s: [kc::SlotRef; 2],
}

fn sim_res() -> SimRes {
    let mut m = SimMachine::new();
    let c0 = m.add_cnode(64);
    let ep = m.add_kernel_object(vifast_sim::kobj::ObjType::Endpoint);
    let epc = m.root_cap(ep);
    m.put_cap(c0, B_EP, epc).expect("cap ep");
    let rep = m.add_kernel_object(vifast_sim::kobj::ObjType::Reply);
    let rpc = m.root_cap(rep);
    m.put_cap(c0, B_REP, rpc).expect("cap rep");
    let mut t = [0u32; 2];
    let mut t_s = [sslot(c0, 0); 2];
    for (i, s) in B_T.iter().enumerate() {
        t[i] = m.add_kernel_object(vifast_sim::kobj::ObjType::Tcb);
        let cap = m.root_cap(t[i]);
        m.put_cap(c0, *s, cap).expect("cap tcb");
        t_s[i] = sslot(c0, *s);
        m.start_tcb(t[i]).expect("Ready");
    }
    m.tcb_set_fault_ep(t_s[0], sslot(c0, B_EP))
        .expect("fault_ep");
    SimRes { m, c0, rep, t, t_s }
}

fn core_res() -> CoreRes {
    let mut m = CoreMachine::new();
    let c0 = m.add_cnode(64);
    let ep = m.add_kernel_object(kc::ObjType::Endpoint);
    let epc = m.root_cap(ep);
    m.put_cap(c0, B_EP, epc).expect("cap ep");
    let rep = m.add_kernel_object(kc::ObjType::Reply);
    let rpc = m.root_cap(rep);
    m.put_cap(c0, B_REP, rpc).expect("cap rep");
    let mut t = [0u32; 2];
    let mut t_s = [cslot(c0, 0); 2];
    for (i, s) in B_T.iter().enumerate() {
        t[i] = m.add_kernel_object(kc::ObjType::Tcb);
        let cap = m.root_cap(t[i]);
        m.put_cap(c0, *s, cap).expect("cap tcb");
        t_s[i] = cslot(c0, *s);
        m.start_tcb(t[i]).expect("Ready");
    }
    m.tcb_set_fault_ep(t_s[0], cslot(c0, B_EP))
        .expect("fault_ep");
    CoreRes { m, c0, rep, t, t_s }
}

#[derive(Debug, PartialEq, Eq)]
struct RD {
    st: [u8; 2],
    regs: [[u64; 4]; 2],
    fault: [Option<(u32, u8)>; 2],
    reply_caller: Option<u32>,
    sent: u64,
    delivered: u64,
    dropped: u64,
    ok: [bool; 4],
}

fn sim_rd(r: &SimRes) -> RD {
    RD {
        st: [
            sim_state(r.m.tcb_state(r.t[0]).unwrap()),
            sim_state(r.m.tcb_state(r.t[1]).unwrap()),
        ],
        regs: [r.m.tcb_regs(r.t[0]).unwrap(), r.m.tcb_regs(r.t[1]).unwrap()],
        fault: [
            r.m.tcb_last_fault(r.t[0])
                .unwrap()
                .map(|(o, k)| (o, sim_fault_tag(k))),
            r.m.tcb_last_fault(r.t[1])
                .unwrap()
                .map(|(o, k)| (o, sim_fault_tag(k))),
        ],
        reply_caller: r.m.reply_caller(r.rep).unwrap(),
        sent: r.m.ipc_sent(),
        delivered: r.m.ipc_delivered(),
        dropped: r.m.ipc_dropped(),
        ok: [
            r.m.check_ipc().is_ok(),
            r.m.check_sched().is_ok(),
            r.m.check_caps().is_ok(),
            r.m.check_notifications().is_ok(),
        ],
    }
}

fn core_rd(r: &CoreRes) -> RD {
    RD {
        st: [
            core_state(r.m.tcb_state(r.t[0]).unwrap()),
            core_state(r.m.tcb_state(r.t[1]).unwrap()),
        ],
        regs: [r.m.tcb_regs(r.t[0]).unwrap(), r.m.tcb_regs(r.t[1]).unwrap()],
        fault: [
            r.m.tcb_last_fault(r.t[0])
                .unwrap()
                .map(|(o, k)| (o, core_fault_tag(k))),
            r.m.tcb_last_fault(r.t[1])
                .unwrap()
                .map(|(o, k)| (o, core_fault_tag(k))),
        ],
        reply_caller: r.m.reply_caller(r.rep).unwrap(),
        sent: r.m.ipc_sent(),
        delivered: r.m.ipc_delivered(),
        dropped: r.m.ipc_dropped(),
        ok: [
            r.m.check_ipc().is_ok(),
            r.m.check_sched().is_ok(),
            r.m.check_caps().is_ok(),
            r.m.check_notifications().is_ok(),
        ],
    }
}

fn buoc_b(s: &mut SimRes, c: &mut CoreRes, i: usize, rs: u16, rc: u16, what: &str) {
    assert_eq!(rs, rc, "bước {i} ({what}): mã lỗi sim {rs} vs core {rc}");
    assert_eq!(sim_rd(s), core_rd(c), "bước {i} ({what}): digest lệch");
}

/// Parity `tcb_resume_with_regs`: sau fault (`BlockedOnSend`), sau `BlockedOnReply`, `Ready`,
/// `Running` (cấm), message mang cap (InvalidArgument) — so mã lỗi + digest (kể cả thanh ghi,
/// `last_fault`, `reply_caller`, bộ đếm IPC).
#[test]
fn kt64_parity_resume_with_regs() {
    let mut s = sim_res();
    let mut c = core_res();

    // procsrv Recv giữ Reply, rồi fault t0 ⇒ t0 BlockedOnSend (chưa giao).
    let rs = rc_sim(s.m.recv(s.t[1], sslot(s.c0, B_EP), Some(sslot(s.c0, B_REP))));
    let rc = rc_core(c.m.recv(c.t[1], cslot(c.c0, B_EP), Some(cslot(c.c0, B_REP))));
    buoc_b(&mut s, &mut c, 1, rs, rc, "t1 recv + Reply");

    let rs = rc_sim(s.m.inject_fault(s.t[0], SimFault::UnknownSyscall { nr: 7 }));
    let rc = rc_core(c.m.inject_fault(c.t[0], kc::FaultKind::UnknownSyscall { nr: 7 }));
    buoc_b(&mut s, &mut c, 2, rs, rc, "fault t0");

    let rs = rc_sim(s.m.tcb_resume_with_regs(s.t_s[0], sim_msg(0, &[1, 2, 3, 4])));
    let rc = rc_core(c.m.tcb_resume_with_regs(c.t_s[0], core_msg(0, &[1, 2, 3, 4])));
    buoc_b(&mut s, &mut c, 3, rs, rc, "resume sau fault ghi regs");

    // Message mang cap ⇒ `InvalidArgument` (cả hai).
    let sim_cap = s.m.root_cap(s.t[0]);
    let sim_bad = vifast_sim::ipc::Message::with_cap_ids(1, &[9], &[sim_cap]).expect("msg cap");
    let rs = rc_sim(s.m.tcb_resume_with_regs(s.t_s[0], sim_bad));
    let core_cap = c.m.root_cap(c.t[0]);
    let core_bad = kc::Message::with_cap_ids(1, &[9], &[core_cap]).expect("msg cap");
    let rc = rc_core(c.m.tcb_resume_with_regs(c.t_s[0], core_bad));
    buoc_b(&mut s, &mut c, 4, rs, rc, "message mang cap");

    // `BlockedOnReply`: t1 Recv(Reply) rồi t0 Call ⇒ t0 chờ reply; resume ⇒ Ready.
    let rs = rc_sim(s.m.recv(s.t[1], sslot(s.c0, B_EP), Some(sslot(s.c0, B_REP))));
    let rc = rc_core(c.m.recv(c.t[1], cslot(c.c0, B_EP), Some(cslot(c.c0, B_REP))));
    assert_eq!(rs, rc, "t1 recv lần 2");
    let rs = rc_sim(s.m.call_msg(s.t[0], sslot(s.c0, B_EP), sim_msg(2, &[0x55])));
    let rc = rc_core(c.m.call_msg(c.t[0], cslot(c.c0, B_EP), core_msg(2, &[0x55])));
    buoc_b(&mut s, &mut c, 5, rs, rc, "call ⇒ BlockedOnReply");

    let rs = rc_sim(s.m.tcb_resume_with_regs(s.t_s[0], sim_msg(3, &[0x66])));
    let rc = rc_core(c.m.tcb_resume_with_regs(c.t_s[0], core_msg(3, &[0x66])));
    buoc_b(&mut s, &mut c, 6, rs, rc, "resume caller BlockedOnReply");

    // `Running` ⇒ cấm (`IllegalOperation`), digest không đổi (thanh ghi giữ 0x66).
    let rs = rc_sim(s.m.transition_tcb(s.t[0], SimTarget::Running { hart: 0 }));
    let rc = rc_core(c.m.transition_tcb(c.t[0], kc::NonReadyTarget::Running { hart: 0 }));
    assert_eq!(rs, rc, "t0 Running");
    let rs = rc_sim(s.m.tcb_resume_with_regs(s.t_s[0], sim_msg(4, &[0x77])));
    let rc = rc_core(c.m.tcb_resume_with_regs(c.t_s[0], core_msg(4, &[0x77])));
    buoc_b(&mut s, &mut c, 7, rs, rc, "resume khi Running (cấm)");
    assert_eq!(
        rc,
        CoreErr::IllegalOperation.id(),
        "core phải IllegalOperation"
    );
}

// ===========================================================================================
// Kịch bản C — **parity `TlbFlush`** (KT-64a / D2-92c): `seq`, `targets`, `pending`, `acked`,
// `Busy`. Hai hart, cùng một `active_on`, cùng một `asid`/`va`.
// ===========================================================================================

const C_VA: u64 = 0x4000;

/// Hook [`kc::TlbEnv`] giả cho core: `active_on`/`generation` điều khiển được, đếm flush **theo
/// hart** (cùng nghĩa `env.flushes` của `kt64_core_tlb_bind_resume.rs`).
#[derive(Default)]
struct TestTlb {
    active: u64,
    generation: u64,
    flushes: std::vec::Vec<(u16, kc::TlbScope, u16, u64)>,
}

impl kc::TlbEnv for TestTlb {
    fn active_on(&self) -> u64 {
        self.active
    }
    fn generation(&self) -> u64 {
        self.generation
    }
    fn flush_local(&mut self, hart: u16, scope: kc::TlbScope, asid: u16, va: u64) {
        self.flushes.push((hart, scope, asid, va));
    }
}

/// Bit hart **phải** ack — **đúng** công thức `D2-78` (`vifast_plat_common::tlb::targets`), dùng
/// chung cho hai phía để `targets` của sim (`= active_on`, quy ước `TlbShootdownInfo` của KT-40 và
/// `tlb::last_shootdown` của kernel) so được với `targets` của core (`TlbFlushReport`).
fn targets_of(active_on: u64, caller: usize) -> u64 {
    active_on & !(1u64 << caller)
}

fn sim_nut_ack(m: &SimMachine, seq: u64) -> [Option<u64>; 2] {
    // Vai **hart gọi**: `tlb_pending` của sim (một shootdown/hart). Vai **hart đích**: phần
    // `tlb_pending_ack` (KT-64a — chi tiết trước đây sim thiếu, nay có để so `pending`).
    [
        m.tlb_pending(0)
            .expect("pending hart 0")
            .map(|p| p.seq)
            .filter(|s| *s == seq)
            .or_else(|| m.tlb_pending_ack(0).expect("ack hart 0")),
        m.tlb_pending(1)
            .expect("pending hart 1")
            .map(|p| p.seq)
            .filter(|s| *s == seq)
            .or_else(|| m.tlb_pending_ack(1).expect("ack hart 1")),
    ]
}

struct SimTlbRig {
    m: SimMachine,
    vs: SimObj,
}

fn sim_tlb_rig(harts: u16, active: u64) -> SimTlbRig {
    let mut m = SimMachine::with_harts(harts);
    let c = m.add_cnode(1024);
    let uva = m.add_untyped(0, 2 * 1024 * 1024);
    let ufr = m.add_untyped(1 << 32, 4096);
    for (slot, u) in [(0usize, uva), (1, ufr)] {
        let cap = m.root_cap(u);
        m.put_cap(c, slot, cap).expect("cap Untyped");
    }
    let cap = m.vspace_create(sslot(c, 0)).expect("vspace_create");
    let vs = m.cap_info(cap).expect("cap vspace").obj;
    m.put_cap(c, 2, cap).expect("cap vspace");
    for h in 0..harts {
        if active & (1u64 << h) != 0 {
            m.vspace_activate(vs, h).expect("vspace_activate");
        }
    }
    assert_eq!(
        m.vspace_active_on(vs).expect("active_on").bits(),
        active,
        "sim: active_on phải đúng mask của rig"
    );
    SimTlbRig { m, vs }
}

/// `(số flush theo hart của sim, số flush theo hart của core)` — hai bên phải bằng nhau.
fn so_flush(s: &SimTlbRig, c: &CoreTlb, harts: u16) {
    for h in 0..harts {
        let a = s.m.tlb_info(h).expect("tlb_info").flushes;
        let b = c.env.flushes.iter().filter(|f| f.0 == h).count() as u64;
        assert_eq!(a, b, "hart {h}: số flush thật lệch ({a} vs {b})");
    }
}

struct CoreTlb {
    m: CoreMachine,
    env: TestTlb,
}

fn core_tlb(active: u64, generation: u64) -> CoreTlb {
    let m = CoreMachine::with_harts(2);
    CoreTlb {
        m,
        env: TestTlb {
            active,
            generation,
            flushes: std::vec::Vec::new(),
        },
    }
}

/// **C1 — 1 hart** (`active_on = {0}`): `seq`, `targets`, `acked` đọc **literal** từ
/// `TlbShootdownInfo` của sim và `TlbFlushReport` của core (cùng phép chiếu `targets_of`).
#[test]
fn kt64a_parity_tlb_flush_mot_hart() {
    let mut s = sim_tlb_rig(1, 0b1);
    let asid = s.m.vspace_get_asid(s.vs).expect("asid");
    let generation = s.m.vspace_info(s.vs).expect("vspace_info").generation;
    let mut c = core_tlb(0b1, generation);
    c.m.set_current_hart(0).expect("hart 0");

    // Sim: `begin` + `wait` (hai cửa của mô hình KT-40). Core: `tlb_flush` **một cửa**
    // (= `tlb_shootdown_begin` + `tlb_shootdown_wait`, trả `TlbFlushReport`) — cùng một shootdown
    // mỗi bên nên số flush thật so được **trực tiếp** sau bước này.
    let seq_s = s.m.tlb_shootdown_begin(0, s.vs, C_VA).expect("sim begin");
    s.m.tlb_shootdown_wait(0).expect("sim wait");
    let r =
        c.m.tlb_flush(&mut c.env, kc::TlbScope::Page { va: C_VA }, asid, C_VA)
            .expect("core tlb_flush 1 hart");
    assert_eq!(seq_s, r.seq, "`seq` phải giống nhau (1-based)");
    let inf = s.m.last_tlb_shootdown().expect("shootdown vừa xong");
    assert_eq!(
        r.active_on,
        inf.targets.bits(),
        "`active_on` của core = `targets` của sim (= active_on)"
    );
    assert_eq!(
        r.targets,
        targets_of(inf.targets.bits(), 0),
        "`targets` của core = `tlb::targets(active_on, caller)` (D2-78)"
    );
    assert_eq!(
        r.acked,
        targets_of(inf.acked.bits(), 0),
        "`acked` của core = ack **trừ** hart gọi (ack ngầm, D2-68)"
    );
    assert!(r.local, "core: `local` = true (luôn flush cục bộ)");
    assert!(inf.completed, "sim: `completed` = true");
    assert_eq!(inf.acked, inf.targets, "sim: `Ok ⇒ acked == targets`");
    assert_eq!(inf.messages, 0, "sim: không có message remote");
    assert_eq!(
        c.m.tlb_remote_posts(),
        0,
        "core: không post cho hart ngoài `active_on`"
    );
    assert!(
        s.m.tlb_pending(0).expect("pending").is_none(),
        "sim: hết chờ"
    );
    assert!(
        c.m.tlb_pending(0).expect("pending core").is_none(),
        "core: hết chờ"
    );
    so_flush(&s, &c, 1);
}

/// **C2 — 2 hart** (`active_on = {0,1}`): begin ⇒ Busy khi còn pending ⇒ hart đích tự phục vụ ⇒
/// wait. So `seq`, `pending` **cả hai vai** (hart gọi + hart nợ ack), `acked`, `Busy`, số
/// `TlbFlush` giao và số flush **theo hart**.
#[test]
fn kt64a_parity_tlb_flush_hai_hart_busy_va_pending() {
    let mut s = sim_tlb_rig(2, 0b11);
    let asid = s.m.vspace_get_asid(s.vs).expect("asid");
    let generation = s.m.vspace_info(s.vs).expect("vspace_info").generation;
    let mut c = core_tlb(0b11, generation);
    c.m.set_current_hart(0).expect("hart 0");

    // --- bước 1: begin từ hart 0 ---
    let seq_s = s.m.tlb_shootdown_begin(0, s.vs, C_VA).expect("sim begin");
    let seq_c =
        c.m.tlb_shootdown_begin(&mut c.env, kc::TlbScope::Page { va: C_VA }, asid, C_VA)
            .expect("core begin");
    assert_eq!(seq_s, seq_c, "`seq`");
    let inf = s.m.tlb_pending(0).expect("pending sim").expect("đang chờ");
    assert_eq!(inf.asid, asid, "sim: `asid` mang theo lệnh flush");
    assert_eq!(inf.generation, generation, "sim: `generation`");
    assert_eq!(inf.targets.bits(), 0b11, "sim: `targets` = active_on");
    assert_eq!(
        targets_of(inf.targets.bits(), 0),
        0b10,
        "`targets` sau phép chiếu D2-78 = hart 1"
    );
    assert_eq!(inf.acked.bits(), 0b1, "sim: ack ngầm của hart gọi");
    assert_eq!(
        targets_of(inf.acked.bits(), 0),
        0,
        "core: chưa hart đích nào ack"
    );
    // `targets` **≠ 0**: so **gián tiếp** qua tập hart **bị post** — hộp thư từng hart (dưới) và
    // số message; `TlbFlushReport.targets` của core chỉ đọc được ở ca `targets == 0` (C1).
    assert!(inf.messages == 1, "sim: đúng 1 `TlbFlush` remote");
    assert_eq!(c.m.tlb_remote_posts(), 1, "core: đúng 1 `TlbFlush` remote");
    // `pending` — sim: hart 0 (vai gọi) + hart 1 (nợ ack); core: một ô/hart, cùng seq.
    assert_eq!(
        sim_nut_ack(&s.m, seq_s)[0],
        c.m.tlb_pending(0).expect("core p0")
    );
    assert_eq!(
        sim_nut_ack(&s.m, seq_s)[1],
        c.m.tlb_pending(1).expect("core p1")
    );
    assert_eq!(
        s.m.tlb_pending_ack(1).expect("ack 1"),
        Some(seq_s),
        "sim: hart 1 **nợ** ack (chi tiết KT-64a thêm vào sim)"
    );
    assert_eq!(
        s.m.tlb_pending_ack(0).expect("ack 0"),
        None,
        "hart gọi ack ngầm"
    );
    // Hộp thư: chỉ hart 1 nhận `TlbFlush` — giống nhau.
    for h in 0..2 {
        assert_eq!(
            s.m.mail_len(h).expect("mail sim"),
            c.m.mail_len(h).expect("mail core"),
            "hộp thư hart {h}"
        );
    }
    assert_eq!(c.m.mail_len(1).expect("mail core"), 1, "hart 1 nhận 1 lệnh");

    // --- bước 2: `Busy` khi còn shootdown chưa ack (cả hai) ---
    let rs = s.m.tlb_shootdown_begin(0, s.vs, C_VA).err();
    let rc =
        c.m.tlb_shootdown_begin(&mut c.env, kc::TlbScope::Page { va: C_VA }, asid, C_VA)
            .err();
    assert_eq!(
        rs.map(|e| e.id()),
        rc.map(|e| e.id()),
        "`Busy` phải giống nhau (cùng mã ABI)"
    );
    assert_eq!(rs, Some(SimErr::Busy), "sim: Busy");
    assert_eq!(rs.map(|e| e.id() == CoreErr::Busy.id()), Some(true));
    assert_eq!(
        s.m.tlb_shootdown_count(),
        c.m.tlb_seq(),
        "seq không tăng khi Busy"
    );

    // --- bước 3: hart đích tự phục vụ (không ai ack hộ) ---
    let n_s = s.m.process_mail(1).expect("sim: hart 1 xử lý hộp thư");
    let n_c =
        c.m.tlb_service(&mut c.env, 1)
            .expect("core: hart 1 phục vụ");
    assert_eq!(n_s, n_c, "cùng một lệnh `TlbFlush` được phục vụ");
    assert_eq!(n_c, 1);
    assert_eq!(
        s.m.tlb_ack_count(),
        c.m.tlb_serviced(),
        "bộ đếm ack thật (sim `ack_total` vs core `serviced`)"
    );
    assert_eq!(sim_nut_ack(&s.m, seq_s)[1], None, "sim: hart 1 hết nợ ack");
    assert_eq!(
        c.m.tlb_pending(1).expect("core p1"),
        None,
        "core: hart 1 hết nợ ack"
    );
    so_flush(&s, &c, 2);

    // --- bước 4: wait đủ ack ---
    s.m.tlb_shootdown_wait(0).expect("sim wait");
    c.m.tlb_shootdown_wait(&mut c.env, 0).expect("core wait");
    let inf = s.m.last_tlb_shootdown().expect("shootdown vừa xong");
    assert!(inf.completed, "sim: completed");
    assert_eq!(inf.acked, inf.targets, "sim: `Ok ⇒ acked == targets` (Q5)");
    assert_eq!(
        targets_of(inf.acked.bits(), 0),
        targets_of(inf.targets.bits(), 0),
        "core: `acked` (trừ ack ngầm) == `targets`"
    );
    assert!(
        sim_nut_ack(&s.m, seq_s).iter().all(|p| p.is_none()),
        "sim: hết pending"
    );
    for h in 0..2 {
        assert!(
            c.m.tlb_pending(h).expect("core pending").is_none(),
            "core: hart {h} hết pending"
        );
        assert_eq!(
            s.m.mail_len(h).expect("mail sim"),
            0,
            "sim: hộp thư {h} rỗng"
        );
        assert_eq!(
            c.m.mail_len(h).expect("mail core"),
            0,
            "core: hộp thư {h} rỗng"
        );
    }
    so_flush(&s, &c, 2);
}
