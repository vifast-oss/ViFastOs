// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-63/KT-64a — parity chuyển cap qua IPC + finalise khi Revoke/Delete (tách khỏi `main.rs`, KT-407).
use super::*;

// =============================================================================================
// KT-63 — chuyển cap qua IPC + finalise IPC khi Revoke/Delete (parity sim↔core)
// =============================================================================================

/// Slot tham chiếu trong `Rig63` (mọi slot nằm trong CNode gốc `c0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum S63 {
    Ep(usize),
    Ntfn,
    /// **KT-64a** — Notification **trong** cây Untyped 2 (`ntnA`) để Revoke finalise đúng nó.
    NtnA,
    Reps,
    U2,
    EpA,
    EpB,
    RepA,
    Slot(usize),
}

/// Rig KT-63 — TCB **kernel object** (sống qua revoke) + IPC object **retype từ Untyped**
/// (revoke được); 2 Endpoint/Reply/Notification ngoài Untyped để kiểm đường sống; 4 cap nguồn
/// (`src`), 1 cap thiếu `RIGHT_GRANT`, 2 cap badge, 1 cap sẽ chết; 3 receive window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Rig63 {
    c0: u32,
    u2: (u32, usize),
    t: [u32; 3],
    ts: [(u32, usize); 3],
    ep: [u32; 2],
    eps: [(u32, usize); 2],
    rep: u32,
    reps: (u32, usize),
    ntfn: u32,
    ntfns: (u32, usize),
    ep_a: (u32, usize),
    ep_b: (u32, usize),
    rep_a: (u32, usize),
    ntn_a: (u32, usize),
    rep_a_obj: u32,
    /// 0..3 cap nguồn (Endpoint 2, đủ quyền), 4 cap thiếu GRANT, 5 badge ep0, 6 badge ep1,
    /// 7 cap chết (đã Delete trước khi gửi).
    cap_table: [u32; 8],
    win: [[(u32, usize); 4]; 3],
}

impl Rig63 {
    fn slot(&self, s: S63) -> (u32, usize) {
        match s {
            S63::Ep(i) => self.eps[i],
            S63::Ntfn => self.ntfns,
            S63::NtnA => self.ntn_a,
            S63::Reps => self.reps,
            S63::U2 => self.u2,
            S63::EpA => self.ep_a,
            S63::EpB => self.ep_b,
            S63::RepA => self.rep_a,
            S63::Slot(k) => (self.c0, k),
        }
    }
}

trait Mut63: Mut {
    fn add_tcb_kernel(&mut self) -> u32;
    fn ready(&mut self, tcb: u32) -> Res;
    fn bind_window(&mut self, tcb: (u32, usize), win: [(u32, usize); 4]) -> Res;
    fn copy_slot(&mut self, src: (u32, usize), dst: (u32, usize)) -> Res;
    fn copy_rights_slot(&mut self, src: (u32, usize), dst: (u32, usize), rights: u8) -> Res;
    fn mint_slot(
        &mut self,
        src: (u32, usize),
        dst: (u32, usize),
        rights: u8,
        badge: Option<u64>,
    ) -> Res;
    fn recv_slot(&mut self, tcb: u32, ep: (u32, usize), reply: Option<(u32, usize)>) -> Res;
    fn send_caps(&mut self, tcb: u32, cap: (u32, usize), label: u32, len: u8, caps: &[u32]) -> Res;
    fn call_caps(&mut self, tcb: u32, ep: (u32, usize), label: u32, len: u8, caps: &[u32]) -> Res;
    fn reply_caps(
        &mut self,
        tcb: u32,
        reply: (u32, usize),
        label: u32,
        len: u8,
        caps: &[u32],
    ) -> Res;
    fn delete_slot(&mut self, slot: (u32, usize)) -> Res;
    fn revoke_slot(&mut self, cnode: u32, slot: usize) -> Res;
}

fn build63<M: Mut63>(m: &mut M) -> Rig63 {
    let u = m.add_untyped(0x1000_0000, 1 << 20);
    let c0 = m.add_cnode(128);
    let uc = m.root_cap(u);
    assert_eq!(m.put_cap(c0, 0, uc), Ok(0), "cap Untyped");
    let mk = |m: &mut M, ty: Ty, s: usize| -> u32 {
        let cap = m.retype((c0, 0), ty, 0).expect("retype") as u32;
        assert_eq!(m.put_cap(c0, s, cap), Ok(0), "put cap {s}");
        cap
    };
    let ep0 = mk(m, Ty::Endpoint, 1);
    let ep1 = mk(m, Ty::Endpoint, 2);
    let rep = mk(m, Ty::Reply, 3);
    let ntfn = mk(m, Ty::Notification, 4);
    let _ = ep0;
    let _ = ep1;
    let _ = rep;
    let _ = ntfn;
    let mut t = [0u32; 3];
    let mut ts = [(c0, 10usize); 3];
    for i in 0..3 {
        let obj = m.add_tcb_kernel();
        let cap = m.root_cap(obj);
        assert_eq!(m.put_cap(c0, 10 + i, cap), Ok(0));
        t[i] = obj;
        ts[i] = (c0, 10 + i);
        assert_eq!(m.ready(obj), Ok(0));
    }
    // 4 cap nguồn (Endpoint thứ 2 — khác endpoint nhận ⇒ không unwrap).
    let mut cap_table = [0u32; 8];
    for (i, dst) in cap_table.iter_mut().take(4).enumerate() {
        *dst = m.copy_slot((c0, 2), (c0, 30 + i)).expect("copy cap nguồn") as u32;
    }
    cap_table[4] = m
        .copy_rights_slot((c0, 2), (c0, 34), ks_rights_rw())
        .expect("cap thiếu GRANT") as u32;
    cap_table[5] = m
        .mint_slot((c0, 1), (c0, 35), ks_rights_all(), Some(0xA5A5))
        .expect("cap badge ep0") as u32;
    cap_table[6] = m
        .mint_slot((c0, 2), (c0, 36), ks_rights_all(), Some(0x0F0F))
        .expect("cap badge ep1") as u32;
    // Cap sẽ chết: copy rồi Delete trước khi gửi (lỗi cứng `InvalidCapability`).
    cap_table[7] = m.copy_slot((c0, 2), (c0, 50)).expect("cap chết") as u32;
    // Untyped thứ hai + cây IPC retype được (revoke).
    let u2 = m.add_untyped(0x2000_0000, 1 << 20);
    let uc2 = m.root_cap(u2);
    assert_eq!(m.put_cap(c0, 20, uc2), Ok(0), "cap Untyped 2");
    let u2s = (c0, 20usize);
    let a = m.retype(u2s, Ty::Endpoint, 0).expect("retype epA") as u32;
    assert_eq!(m.put_cap(c0, 21, a), Ok(0));
    let b = m.retype(u2s, Ty::Endpoint, 0).expect("retype epB") as u32;
    assert_eq!(m.put_cap(c0, 22, b), Ok(0));
    let ra = m.retype(u2s, Ty::Reply, 0).expect("retype repA") as u32;
    assert_eq!(m.put_cap(c0, 23, ra), Ok(0));
    let na = m.retype(u2s, Ty::Notification, 0).expect("retype ntnA") as u32;
    assert_eq!(m.put_cap(c0, 24, na), Ok(0));
    let mut win = [[(c0, 90usize); 4]; 3];
    for (i, base) in [90usize, 94, 98].iter().enumerate() {
        win[i] = [
            (c0, *base),
            (c0, *base + 1),
            (c0, *base + 2),
            (c0, *base + 3),
        ];
    }
    Rig63 {
        c0,
        u2: u2s,
        t,
        ts,
        ep: [ep0, ep1],
        eps: [(c0, 1), (c0, 2)],
        rep,
        reps: (c0, 3),
        ntfn,
        ntfns: (c0, 4),
        ep_a: (c0, 21),
        ep_b: (c0, 22),
        rep_a: (c0, 23),
        ntn_a: (c0, 24),
        rep_a_obj: ra,
        cap_table,
        win,
    }
}

/// Quyền toàn phần của cap (4 bit) — hằng thiết kế §4.3, giống nhau hai phía.
fn ks_rights_all() -> u8 {
    0xF
}

fn ks_rights_rw() -> u8 {
    0x3
}

fn msg_words(label: u32, len: u8) -> Vec<u64> {
    (0..len).map(|i| (label as u64) * 100 + i as u64).collect()
}

impl Mut63 for ks::Machine {
    fn add_tcb_kernel(&mut self) -> u32 {
        self.add_kernel_object(ks::ObjType::Tcb)
    }
    fn ready(&mut self, tcb: u32) -> Res {
        unit_sim(self.start_tcb(tcb))
    }
    fn bind_window(&mut self, tcb: (u32, usize), win: [(u32, usize); 4]) -> Res {
        let mut w = [ks::SlotRef::new(0, 0); 4];
        for i in 0..4 {
            w[i] = ks::SlotRef::new(win[i].0, win[i].1);
        }
        unit_sim(self.tcb_set_recv_window(ks::SlotRef::new(tcb.0, tcb.1), w))
    }
    fn copy_slot(&mut self, src: (u32, usize), dst: (u32, usize)) -> Res {
        self.copy(
            ks::SlotRef::new(src.0, src.1),
            ks::SlotRef::new(dst.0, dst.1),
        )
        .map(|c| c as u64)
        .map_err(se)
    }
    fn copy_rights_slot(&mut self, src: (u32, usize), dst: (u32, usize), rights: u8) -> Res {
        self.copy_rights(
            ks::SlotRef::new(src.0, src.1),
            ks::SlotRef::new(dst.0, dst.1),
            Some(rights),
        )
        .map(|c| c as u64)
        .map_err(se)
    }
    fn mint_slot(
        &mut self,
        src: (u32, usize),
        dst: (u32, usize),
        rights: u8,
        badge: Option<u64>,
    ) -> Res {
        self.mint(
            ks::SlotRef::new(src.0, src.1),
            ks::SlotRef::new(dst.0, dst.1),
            rights,
            badge,
        )
        .map(|c| c as u64)
        .map_err(se)
    }
    fn recv_slot(&mut self, tcb: u32, ep: (u32, usize), reply: Option<(u32, usize)>) -> Res {
        let r = reply.map(|x| ks::SlotRef::new(x.0, x.1));
        unit_sim(self.recv(tcb, ks::SlotRef::new(ep.0, ep.1), r))
    }
    fn send_caps(&mut self, tcb: u32, cap: (u32, usize), label: u32, len: u8, caps: &[u32]) -> Res {
        let ids = caps.iter().copied().map(|c| c as ks::CapId).collect();
        let info = vs::ipc::MessageInfo::new(label, caps.len() as u8, false, 0, len).expect("info");
        let msg =
            vs::ipc::Message::with_caps(info, &msg_words(label, len), ids, 0).expect("message cap");
        unit_sim(self.send(tcb, ks::SlotRef::new(cap.0, cap.1), msg))
    }
    fn call_caps(&mut self, tcb: u32, ep: (u32, usize), label: u32, len: u8, caps: &[u32]) -> Res {
        let ids = caps.iter().copied().map(|c| c as ks::CapId).collect();
        let info = vs::ipc::MessageInfo::new(label, caps.len() as u8, false, 0, len).expect("info");
        let msg =
            vs::ipc::Message::with_caps(info, &msg_words(label, len), ids, 0).expect("message cap");
        unit_sim(self.call_msg(tcb, ks::SlotRef::new(ep.0, ep.1), msg))
    }
    fn reply_caps(
        &mut self,
        tcb: u32,
        reply: (u32, usize),
        label: u32,
        len: u8,
        caps: &[u32],
    ) -> Res {
        let ids = caps.iter().copied().map(|c| c as ks::CapId).collect();
        let info = vs::ipc::MessageInfo::new(label, caps.len() as u8, false, 0, len).expect("info");
        let msg =
            vs::ipc::Message::with_caps(info, &msg_words(label, len), ids, 0).expect("message cap");
        unit_sim(self.reply_msg(tcb, ks::SlotRef::new(reply.0, reply.1), msg))
    }
    fn delete_slot(&mut self, slot: (u32, usize)) -> Res {
        unit_sim(self.delete(ks::SlotRef::new(slot.0, slot.1)))
    }
    fn revoke_slot(&mut self, cnode: u32, slot: usize) -> Res {
        num_sim(self.revoke(cnode, Some(slot)))
    }
}

impl Mut63 for kc::Machine {
    fn add_tcb_kernel(&mut self) -> u32 {
        self.add_kernel_object(kc::ObjType::Tcb)
    }
    fn ready(&mut self, tcb: u32) -> Res {
        unit_core(self.start_tcb(tcb))
    }
    fn bind_window(&mut self, tcb: (u32, usize), win: [(u32, usize); 4]) -> Res {
        let mut w = [kc::SlotRef::new(0, 0); 4];
        for i in 0..4 {
            w[i] = kc::SlotRef::new(win[i].0, win[i].1);
        }
        unit_core(self.tcb_set_recv_window(kc::SlotRef::new(tcb.0, tcb.1), w))
    }
    fn copy_slot(&mut self, src: (u32, usize), dst: (u32, usize)) -> Res {
        self.copy(
            kc::SlotRef::new(src.0, src.1),
            kc::SlotRef::new(dst.0, dst.1),
        )
        .map(|c| c as u64)
        .map_err(ce)
    }
    fn copy_rights_slot(&mut self, src: (u32, usize), dst: (u32, usize), rights: u8) -> Res {
        self.copy_rights(
            kc::SlotRef::new(src.0, src.1),
            kc::SlotRef::new(dst.0, dst.1),
            Some(rights),
        )
        .map(|c| c as u64)
        .map_err(ce)
    }
    fn mint_slot(
        &mut self,
        src: (u32, usize),
        dst: (u32, usize),
        rights: u8,
        badge: Option<u64>,
    ) -> Res {
        self.mint(
            kc::SlotRef::new(src.0, src.1),
            kc::SlotRef::new(dst.0, dst.1),
            rights,
            badge,
        )
        .map(|c| c as u64)
        .map_err(ce)
    }
    fn recv_slot(&mut self, tcb: u32, ep: (u32, usize), reply: Option<(u32, usize)>) -> Res {
        let r = reply.map(|x| kc::SlotRef::new(x.0, x.1));
        unit_core(self.recv(tcb, kc::SlotRef::new(ep.0, ep.1), r))
    }
    fn send_caps(&mut self, tcb: u32, cap: (u32, usize), label: u32, len: u8, caps: &[u32]) -> Res {
        let ids: Vec<kc::CapId> = caps.to_vec();
        let info = kc::MessageInfo::new(label, caps.len() as u8, false, 0, len).expect("info");
        let msg =
            kc::Message::with_caps(info, &msg_words(label, len), &ids, 0).expect("message cap");
        unit_core(self.send(tcb, kc::SlotRef::new(cap.0, cap.1), msg))
    }
    fn call_caps(&mut self, tcb: u32, ep: (u32, usize), label: u32, len: u8, caps: &[u32]) -> Res {
        let ids: Vec<kc::CapId> = caps.to_vec();
        let info = kc::MessageInfo::new(label, caps.len() as u8, false, 0, len).expect("info");
        let msg =
            kc::Message::with_caps(info, &msg_words(label, len), &ids, 0).expect("message cap");
        unit_core(self.call_msg(tcb, kc::SlotRef::new(ep.0, ep.1), msg))
    }
    fn reply_caps(
        &mut self,
        tcb: u32,
        reply: (u32, usize),
        label: u32,
        len: u8,
        caps: &[u32],
    ) -> Res {
        let ids: Vec<kc::CapId> = caps.to_vec();
        let info = kc::MessageInfo::new(label, caps.len() as u8, false, 0, len).expect("info");
        let msg =
            kc::Message::with_caps(info, &msg_words(label, len), &ids, 0).expect("message cap");
        unit_core(self.reply_msg(tcb, kc::SlotRef::new(reply.0, reply.1), msg))
    }
    fn delete_slot(&mut self, slot: (u32, usize)) -> Res {
        unit_core(self.delete(kc::SlotRef::new(slot.0, slot.1)))
    }
    fn revoke_slot(&mut self, cnode: u32, slot: usize) -> Res {
        num_core(self.revoke(cnode, Some(slot)))
    }
}

#[derive(Debug, Clone, Copy)]
enum Op63 {
    BindWindow {
        tcb: usize,
        win: usize,
    },
    Recv {
        tcb: usize,
        slot: S63,
        reply: Option<S63>,
    },
    SendCaps {
        tcb: usize,
        slot: S63,
        label: u32,
        len: u8,
        caps: [u8; 4],
        n: u8,
    },
    CallCaps {
        tcb: usize,
        slot: S63,
        label: u32,
        len: u8,
        caps: [u8; 4],
        n: u8,
    },
    ReplyCaps {
        tcb: usize,
        slot: S63,
        label: u32,
        len: u8,
        caps: [u8; 4],
        n: u8,
    },
    MintWindow {
        win: usize,
        slot: S63,
    },
    Delete {
        slot: S63,
    },
    Copy {
        src: S63,
        dst: S63,
    },
    RevokeSlot {
        slot: S63,
    },
}

fn step63<M: Mut63>(m: &mut M, r: &Rig63, op: &Op63) -> Res {
    match *op {
        Op63::BindWindow { tcb, win } => m.bind_window(r.ts[tcb], r.win[win]),
        Op63::Recv { tcb, slot, reply } => {
            let ep = r.slot(slot);
            m.recv_slot(r.t[tcb], ep, reply.map(|s| r.slot(s)))
        }
        Op63::SendCaps {
            tcb,
            slot,
            label,
            len,
            caps,
            n,
        } => {
            let ids: Vec<u32> = caps[..n as usize]
                .iter()
                .map(|i| r.cap_table[*i as usize])
                .collect();
            m.send_caps(r.t[tcb], r.slot(slot), label, len, &ids)
        }
        Op63::CallCaps {
            tcb,
            slot,
            label,
            len,
            caps,
            n,
        } => {
            let ids: Vec<u32> = caps[..n as usize]
                .iter()
                .map(|i| r.cap_table[*i as usize])
                .collect();
            m.call_caps(r.t[tcb], r.slot(slot), label, len, &ids)
        }
        Op63::ReplyCaps {
            tcb,
            slot,
            label,
            len,
            caps,
            n,
        } => {
            let ids: Vec<u32> = caps[..n as usize]
                .iter()
                .map(|i| r.cap_table[*i as usize])
                .collect();
            m.reply_caps(r.t[tcb], r.slot(slot), label, len, &ids)
        }
        Op63::MintWindow { win, slot } => {
            let dst = r.win[win][1];
            m.mint_slot(r.slot(slot), dst, ks_rights_all(), None)
        }
        Op63::Delete { slot } => m.delete_slot(r.slot(slot)),
        Op63::Copy { src, dst } => m.copy_slot(r.slot(src), r.slot(dst)),
        Op63::RevokeSlot { slot } => {
            let s = r.slot(slot);
            m.revoke_slot(s.0, s.1)
        }
    }
}

/// Loại trạng thái kỳ vọng (bỏ ObjId để literal kịch bản không phụ thuộc id).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StKind {
    Ready,
    BlockedOnSend,
    BlockedOnRecv,
    BlockedOnNotification,
}

impl StKind {
    fn sim(self, st: CS) -> bool {
        matches!(
            (self, st),
            (StKind::Ready, CS::Ready)
                | (StKind::BlockedOnSend, CS::BlockedOnSend { .. })
                | (StKind::BlockedOnRecv, CS::BlockedOnRecv { .. })
                | (
                    StKind::BlockedOnNotification,
                    CS::BlockedOnNotification { .. }
                )
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Digest63 {
    states: Vec<Result<CS, E>>,
    /// **KT-64a** — `Tcb.last_ipc_error` của 3 TCB (D2-89/D2-90: phải là `Cancelled` sau finalise).
    last_errs: Vec<Result<Option<u16>, E>>,
    windows: Vec<Result<WinFp, E>>,
    ipcs: Vec<Result<IpcFp, E>>,
    eps: Vec<Result<EpFp, E>>,
    rep: Result<Option<u32>, E>,
    rep_a: Result<Option<u32>, E>,
    ntfn: Result<NtFp, E>,
    counters: CtFp,
    cap_stats: (u64, u64, u64),
    checks: ChkFp,
}

fn digest63(v: &dyn View, r: &Rig63) -> Digest63 {
    Digest63 {
        states: (0..3).map(|i| v.tcb_state(r.t[i])).collect(),
        last_errs: (0..3).map(|i| v.last_ipc_error(r.t[i])).collect(),
        windows: (0..3).map(|i| v.window(r.t[i])).collect(),
        ipcs: (0..3).map(|i| v.ipc_area(r.t[i])).collect(),
        eps: (0..2).map(|i| v.endpoint(r.ep[i])).collect(),
        rep: v.reply_caller_fp(r.rep),
        rep_a: v.reply_caller_fp(r.rep_a_obj),
        ntfn: v.notification(r.ntfn),
        counters: v.counters(),
        cap_stats: v.cap_stats(),
        checks: v.checks(),
    }
}

fn run63(name: &str, ops: &[(&'static str, Op63)], expect_states: &[(usize, StKind)]) {
    run63_full(name, ops, expect_states, &[]);
}

/// Như [`run63`] + pin `Tcb.last_ipc_error` kỳ vọng của từng TCB (**KT-64a**: D2-89/D2-90 —
/// `Cancelled` cho waiter/caller/sender sau khi object bị finalise). Không truyền ⇒ chỉ so
/// parity (digest đã chứa `last_errs`).
fn run63_full(
    name: &str,
    ops: &[(&'static str, Op63)],
    expect_states: &[(usize, StKind)],
    expect_errs: &[(usize, u16)],
) {
    let mut sm = ks::Machine::new();
    let mut cm = kc::Machine::new();
    let rig_s = build63(&mut sm);
    let rig_c = build63(&mut cm);
    assert_eq!(rig_s, rig_c, "{name}: rig hai bên lệch");
    for (i, (label, op)) in ops.iter().enumerate() {
        let a = step63(&mut sm, &rig_s, op);
        let b = step63(&mut cm, &rig_c, op);
        assert_eq!(a, b, "{name} buoc {i} `{label}`: ket qua lech");
        let da = digest63(&sm, &rig_s);
        let db = digest63(&cm, &rig_c);
        assert_eq!(da, db, "{name} buoc {i} `{label}`: digest lech");
        assert!(da.checks.ipc, "{name} buoc {i}: sim check_ipc do");
        assert!(da.checks.reply, "{name} buoc {i}: sim check_reply do");
        assert!(
            da.checks.ntfn,
            "{name} buoc {i}: sim check_notifications do"
        );
        assert!(da.checks.caps, "{name} buoc {i}: sim check_caps do");
        assert!(da.checks.sched, "{name} buoc {i}: sim check_sched do");
        assert!(db.checks.ipc, "{name} buoc {i}: core check_ipc do");
        assert!(db.checks.reply, "{name} buoc {i}: core check_reply do");
        assert!(
            db.checks.ntfn,
            "{name} buoc {i}: core check_notifications do"
        );
        assert!(db.checks.caps, "{name} buoc {i}: core check_caps do");
        assert!(db.checks.sched, "{name} buoc {i}: core check_sched do");
    }
    for (i, kind) in expect_states {
        let a = cs_of_sim(sm.tcb_state(rig_s.t[*i]).expect("sim state"));
        let b = cs_of_core(cm.tcb_state(rig_c.t[*i]).expect("core state"));
        assert!(
            kind.sim(a),
            "{name}: sim t{i} state {a:?} khong dung loai {kind:?}"
        );
        assert!(
            kind.sim(b),
            "{name}: core t{i} state {b:?} khong dung loai {kind:?}"
        );
    }
    for (i, id) in expect_errs {
        let a = sm
            .tcb_last_ipc_error(rig_s.t[*i])
            .expect("sim last_ipc_error")
            .map(|e| e.id());
        let b = cm
            .tcb_last_ipc_error(rig_c.t[*i])
            .expect("core last_ipc_error")
            .map(|e| e.id());
        assert_eq!(a, Some(*id), "{name}: sim t{i} last_ipc_error {a:?}");
        assert_eq!(b, Some(*id), "{name}: core t{i} last_ipc_error {b:?}");
    }
}

#[test]
fn kt63_parity_chuyen_cap_qua_ipc() {
    let s = |label: &'static str, op: Op63| (label, op);
    let ops = vec![
        s("bind window t1", Op63::BindWindow { tcb: 1, win: 0 }),
        s(
            "recv t1 ep0 + reply",
            Op63::Recv {
                tcb: 1,
                slot: S63::Ep(0),
                reply: Some(S63::Reps),
            },
        ),
        // Ô window 1 của t1 **bị chiếm** trước khi giao ⇒ cap thứ 2 thất bại mềm.
        s(
            "chiem o window 1 cua t1",
            Op63::MintWindow {
                win: 0,
                slot: S63::Ep(1),
            },
        ),
        s(
            "gui 4 cap vao o ban",
            Op63::SendCaps {
                tcb: 0,
                slot: S63::Ep(0),
                label: 7,
                len: 2,
                caps: [0, 1, 2, 3],
                n: 4,
            },
        ),
        s(
            "recv t1 lai",
            Op63::Recv {
                tcb: 1,
                slot: S63::Ep(0),
                reply: None,
            },
        ),
        s(
            "gui 3 cap + 1 thieu GRANT",
            Op63::SendCaps {
                tcb: 0,
                slot: S63::Ep(0),
                label: 8,
                len: 1,
                caps: [0, 4, 2, 3],
                n: 4,
            },
        ),
        s(
            "recv t1 lai 2",
            Op63::Recv {
                tcb: 1,
                slot: S63::Ep(0),
                reply: None,
            },
        ),
        s(
            "gui unwrap + 1 cap",
            Op63::SendCaps {
                tcb: 0,
                slot: S63::Ep(0),
                label: 12,
                len: 1,
                caps: [5, 1, 0, 0],
                n: 2,
            },
        ),
        // Window **chưa bind** (t2) ⇒ lỗi cứng trước khi giao; message chỉ unwrap vẫn giao.
        s(
            "recv t2 ep1 (chua bind)",
            Op63::Recv {
                tcb: 2,
                slot: S63::Ep(1),
                reply: None,
            },
        ),
        s(
            "gui 4 cap vao window chua bind",
            Op63::SendCaps {
                tcb: 0,
                slot: S63::Ep(1),
                label: 15,
                len: 0,
                caps: [0, 1, 2, 3],
                n: 4,
            },
        ),
        s(
            "gui unwrap-only khi chua bind",
            Op63::SendCaps {
                tcb: 0,
                slot: S63::Ep(1),
                label: 16,
                len: 0,
                caps: [6, 0, 0, 0],
                n: 1,
            },
        ),
        s(
            "recv t2 ep1 lai",
            Op63::Recv {
                tcb: 2,
                slot: S63::Ep(1),
                reply: None,
            },
        ),
        // Bind window t2 rồi giao **đủ 4 cap** (window sạch) cho waiter ep1.
        s("bind window t2", Op63::BindWindow { tcb: 2, win: 1 }),
        s(
            "gui 4 cap du",
            Op63::SendCaps {
                tcb: 0,
                slot: S63::Ep(1),
                label: 17,
                len: 1,
                caps: [0, 1, 2, 3],
                n: 4,
            },
        ),
        // Call/Reply mang cap (caller bind window riêng).
        s(
            "recv t1 + reply",
            Op63::Recv {
                tcb: 1,
                slot: S63::Ep(0),
                reply: Some(S63::Reps),
            },
        ),
        s(
            "call 2 cap",
            Op63::CallCaps {
                tcb: 0,
                slot: S63::Ep(0),
                label: 20,
                len: 1,
                caps: [0, 1, 0, 0],
                n: 2,
            },
        ),
        s("bind window t0", Op63::BindWindow { tcb: 0, win: 2 }),
        s(
            "reply 2 cap",
            Op63::ReplyCaps {
                tcb: 1,
                slot: S63::Reps,
                label: 21,
                len: 1,
                caps: [2, 3, 0, 0],
                n: 2,
            },
        ),
        // Cap nguồn chết ⇒ lỗi cứng `InvalidCapability` trước khi cấp `msg_id`.
        s(
            "delete cap nguon",
            Op63::Delete {
                slot: S63::Slot(50),
            },
        ),
        s(
            "recv t2 ep1 lai 2",
            Op63::Recv {
                tcb: 2,
                slot: S63::Ep(1),
                reply: None,
            },
        ),
        s(
            "gui cap nguon chet",
            Op63::SendCaps {
                tcb: 0,
                slot: S63::Ep(1),
                label: 10,
                len: 1,
                caps: [7, 0, 0, 0],
                n: 1,
            },
        ),
    ];
    run63(
        "kt63-cap-transfer",
        &ops,
        &[
            (0, StKind::Ready),
            (1, StKind::Ready),
            (2, StKind::BlockedOnRecv),
        ],
    );
    // Pin bộ đếm của cả kịch bản (**cùng cửa sổ được tái dùng**, nên các lần sau thấy ô đã có
    // cap ⇒ thất bại mềm — đúng ngữ nghĩa sim, không phải lỗi):
    //   cài = 3 (ô 1 bị chiếm) + 4 (window t2 sạch) + 2 (Reply → window t0 sạch) = 9;
    //   unwrap = 1 (ów 0/1 đã bận, chỉ unwrap) + 1 (unwrap-only, window chưa bind) = 2;
    //   mềm = 1 + 4 + 1 + 2 = 8.
    let mut cm = kc::Machine::new();
    let rig = build63(&mut cm);
    for (_, op) in ops.iter() {
        let _ = step63(&mut cm, &rig, op);
    }
    assert_eq!(cm.cap_transfer_stats(), (9, 2, 8));
}

/// **KT-63 → KT-64a** — `Revoke` Untyped khi **có caller** (`BlockedOnReply`) **và sender**
/// (`BlockedOnSend` trên Endpoint trong cây). D2-89/D2-90: **cả hai** phải được đánh thức
/// `Cancelled + Ready` (trước KT-64a sim để sender **treo** trên Endpoint đã chết — khi đó
/// kịch bản này là **loại trừ** parity; nay phủ trọn, không còn `skip`).
#[test]
fn kt63_parity_revoke_finalise_caller_va_sender() {
    let s = |label: &'static str, op: Op63| (label, op);
    let ops = vec![
        s("bind window t1", Op63::BindWindow { tcb: 1, win: 0 }),
        // Waiter epA (cây 2) + Call mang cap ⇒ caller BlockedOnReply(repA).
        s(
            "recv t1 epA + reply A",
            Op63::Recv {
                tcb: 1,
                slot: S63::EpA,
                reply: Some(S63::RepA),
            },
        ),
        s(
            "call epA 1 cap",
            Op63::CallCaps {
                tcb: 0,
                slot: S63::EpA,
                label: 30,
                len: 1,
                caps: [0, 0, 0, 0],
                n: 1,
            },
        ),
        // Hàng epB: t2 Send 1 cap (chặn, không waiter).
        s(
            "send epB 1 cap",
            Op63::SendCaps {
                tcb: 2,
                slot: S63::EpB,
                label: 31,
                len: 0,
                caps: [1, 0, 0, 0],
                n: 1,
            },
        ),
        // t1 chờ Notification **sống** (ngoài cây 2) — không bị Revoke.
        s(
            "wait notification song",
            Op63::Recv {
                tcb: 1,
                slot: S63::Ntfn,
                reply: None,
            },
        ),
        s("revoke cay Untyped 2", Op63::RevokeSlot { slot: S63::U2 }),
    ];
    // Pha **trước** finalise (5 bước đầu): sender còn `BlockedOnSend` — chốt rằng ca
    // `StKind::BlockedOnSend` ở đây là trạng thái **thật** trước `Revoke`, không phải kịch bản né.
    run63_full(
        "kt63-revoke (truoc finalise)",
        &ops[..5],
        &[
            // t0 đang `BlockedOnReply` (không có biến thể `StKind`), t1 chờ Notification **sống**.
            (1, StKind::BlockedOnNotification),
            (2, StKind::BlockedOnSend),
        ],
        &[],
    );
    run63_full(
        "kt63-revoke",
        &ops,
        &[
            (0, StKind::Ready),
            (1, StKind::BlockedOnNotification),
            // D2-90: sender trên Endpoint bị finalise được đánh thức (không treo).
            (2, StKind::Ready),
        ],
        &[
            (0, ks::KError::Cancelled.id()),
            (2, ks::KError::Cancelled.id()),
        ],
    );
}

/// **KT-64a (D2-89)** — finalise Endpoint **đang có Recv waiter**: waiter phải `Cancelled +
/// Ready` thay vì treo trên Endpoint đã chết. Đây là đường mà parity KT-63 **loại trừ**; nay
/// phủ trực tiếp (`epA` nằm trong cây Untyped 2, `recv` trước khi Revoke).
#[test]
fn kt64a_parity_finalise_recv_waiter() {
    let s = |label: &'static str, op: Op63| (label, op);
    let ops = vec![
        // t1 chờ **chính** Endpoint sẽ bị finalise (không có message ⇒ BlockedOnRecv).
        s(
            "recv t1 epA",
            Op63::Recv {
                tcb: 1,
                slot: S63::EpA,
                reply: Some(S63::RepA),
            },
        ),
        s("revoke cay Untyped 2", Op63::RevokeSlot { slot: S63::U2 }),
    ];
    run63_full(
        "kt64a-finalise-recv-waiter",
        &ops,
        &[(0, StKind::Ready), (1, StKind::Ready), (2, StKind::Ready)],
        &[(1, ks::KError::Cancelled.id())],
    );
}

/// **KT-64a (D2-89)** — finalise Notification **đang có waiter**: waiter `Cancelled + Ready`
/// (sim cũ để `BlockedOnNotification{ntfn chết}` — lỗi mô hình).
#[test]
fn kt64a_parity_finalise_notification_waiter() {
    let s = |label: &'static str, op: Op63| (label, op);
    let ops = vec![
        s(
            "wait t1 ntnA",
            Op63::Recv {
                tcb: 1,
                slot: S63::NtnA,
                reply: None,
            },
        ),
        s("revoke cay Untyped 2", Op63::RevokeSlot { slot: S63::U2 }),
    ];
    run63_full(
        "kt64a-finalise-notification-waiter",
        &ops,
        &[(0, StKind::Ready), (1, StKind::Ready), (2, StKind::Ready)],
        &[(1, ks::KError::Cancelled.id())],
    );
}

#[test]
fn kt63_parity_delete_cap_reply_huy_caller() {
    let s = |label: &'static str, op: Op63| (label, op);
    let ops = vec![
        s("bind window t1", Op63::BindWindow { tcb: 1, win: 0 }),
        s(
            "recv t1 ep0 + reply",
            Op63::Recv {
                tcb: 1,
                slot: S63::Ep(0),
                reply: Some(S63::Reps),
            },
        ),
        s(
            "call 2 cap",
            Op63::CallCaps {
                tcb: 0,
                slot: S63::Ep(0),
                label: 40,
                len: 1,
                caps: [0, 1, 0, 0],
                n: 2,
            },
        ),
        s("delete cap Reply", Op63::Delete { slot: S63::Reps }),
        s(
            "recv t2 ep0",
            Op63::Recv {
                tcb: 2,
                slot: S63::Ep(0),
                reply: None,
            },
        ),
    ];
    run63(
        "kt63-delete-reply",
        &ops,
        &[
            (0, StKind::Ready),
            (1, StKind::Ready),
            (2, StKind::BlockedOnRecv),
        ],
    );
}

#[test]
fn kt63_parity_revoke_cap_reply_con_huy_caller() {
    let s = |label: &'static str, op: Op63| (label, op);
    let ops = vec![
        s("bind window t1", Op63::BindWindow { tcb: 1, win: 0 }),
        s(
            "copy cap Reply -> slot 51",
            Op63::Copy {
                src: S63::Reps,
                dst: S63::Slot(51),
            },
        ),
        s(
            "recv t1 ep0 + reply con",
            Op63::Recv {
                tcb: 1,
                slot: S63::Ep(0),
                reply: Some(S63::Slot(51)),
            },
        ),
        s(
            "call 1 cap",
            Op63::CallCaps {
                tcb: 0,
                slot: S63::Ep(0),
                label: 41,
                len: 0,
                caps: [0, 0, 0, 0],
                n: 1,
            },
        ),
        s("revoke cap Reply goc", Op63::RevokeSlot { slot: S63::Reps }),
        s(
            "delete cap con sau revoke",
            Op63::Delete {
                slot: S63::Slot(51),
            },
        ),
    ];
    run63(
        "kt63-revoke-reply-con",
        &ops,
        &[(0, StKind::Ready), (1, StKind::Ready), (2, StKind::Ready)],
    );
}

#[test]
fn kt63_parity_reply_cap_chet_trong_waiter_q7b() {
    let s = |label: &'static str, op: Op63| (label, op);
    let ops = vec![
        s("bind window t1", Op63::BindWindow { tcb: 1, win: 0 }),
        s(
            "recv t1 ep0 + reply",
            Op63::Recv {
                tcb: 1,
                slot: S63::Ep(0),
                reply: Some(S63::Reps),
            },
        ),
        s(
            "delete reply cap cua waiter",
            Op63::Delete { slot: S63::Reps },
        ),
        s(
            "call 1 cap (Q7b)",
            Op63::CallCaps {
                tcb: 0,
                slot: S63::Ep(0),
                label: 42,
                len: 1,
                caps: [0, 0, 0, 0],
                n: 1,
            },
        ),
    ];
    run63(
        "kt63-q7b",
        &ops,
        &[(0, StKind::Ready), (1, StKind::Ready), (2, StKind::Ready)],
    );
}
