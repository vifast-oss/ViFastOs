// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-59 — parity core `no_std` (`vifast-core`, K3b tầng 2: TCB/transition/scheduler/ngân sách
//! MCS/hộp thư) với **sim** (oracle).
//!
//! Cùng một `Op` chạy trên `vifast_sim::kobj::Machine` và `vifast_core::Machine` qua trait
//! [`View`]/[`Mut`]; sau **mỗi** bước so:
//! - kết quả (`Ok`/`Err` + **tên + số** mã lỗi — cùng bộ 19 mã sinh từ spec);
//! - digest trạng thái TCB (state + payload, `suspended`, `exit_code`, `fault_ep`, `cspace`,
//!   priority/MCP, affinity/home, `sched_context`, `donated_sc`, `bound_notification`),
//!   `Tcb.ReadState`, ngân sách mọi `SchedContext`, ready queue từng hart (bitmap/len/FIFO
//!   `pick`/bucket/`current`/`published`), hộp thư từng hart (len/log/stopped), log finalise,
//!   bộ đếm cặp chuyển trạng thái;
//! - bốn bất biến `check_sched` + `check_mailboxes` + `check_caps` + `check_all_bytes` của
//!   **cả hai** phải `Ok`.
//!
//! Kịch bản (`scripts`): port các ca KT-18 (bảng ALLOWED, suspend/resume, finalise), KT-19
//! (priority/FIFO/round-robin/preempt), KT-20 (budget/refill/pick có điều kiện), KT-22 (hộp thư
//! FIFO/backpressure/`Stop`/`Migrate` 1 hart) và KT-29 (nhiều hart, `published`, push migration
//! cơ bản). Differential 16 seed × 32 op tất định (LCG) phủ tổ hợp.
//!
//! **Biên KT-59:** không có IPC/Endpoint/Reply/Notification (KT-60) nên kịch bản **không** đi
//! qua `Call`/`Recv`/`Reply`/`Signal`/`Bind`; donation KT-21 (gắn `Call`/`Reply`) chỉ được pin
//! bằng công thức + unit test của core, không có bước tạo donation. Đây là biên task đã ghi
//! trong `docs/k3b-core.md` §7, không phải nới kỳ vọng.

use std::collections::BTreeSet;

use vifast_core as kc;
use vifast_sim as vs;
use vs::kobj as ks;

// **KT-407** — tách theo miền (tệp gốc 2262 dòng).
mod tests;
use vs::Lcg;

// ---------------------------------------------------------------------------------------------
// Kiểu chung
// ---------------------------------------------------------------------------------------------

type E = (&'static str, u16);
type Res = Result<u64, E>;

/// Loại object trong kịch bản (ánh xạ sang enum của sim/core).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ty {
    Tcb,
    SchedContext,
    Endpoint,
    SchedControl,
}

impl Ty {
    fn sim(self) -> ks::ObjType {
        match self {
            Ty::Tcb => ks::ObjType::Tcb,
            Ty::SchedContext => ks::ObjType::SchedContext,
            Ty::Endpoint => ks::ObjType::Endpoint,
            Ty::SchedControl => ks::ObjType::SchedControl,
        }
    }

    fn core(self) -> kc::ObjType {
        match self {
            Ty::Tcb => kc::ObjType::Tcb,
            Ty::SchedContext => kc::ObjType::SchedContext,
            Ty::Endpoint => kc::ObjType::Endpoint,
            Ty::SchedControl => kc::ObjType::SchedControl,
        }
    }
}

/// Trạng thái TCB **chuẩn hoá** (payload đủ để digest; fault chỉ hỗ trợ `None`/`Exit`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CS {
    Inactive,
    Ready,
    Running { hart: u16 },
    BlockedOnSend { ep: u32, call: bool },
    BlockedOnRecv { ep: u32, reply: u32 },
    BlockedOnReply { reply: u32 },
    BlockedOnNotification { ntfn: u32 },
    BlockedOnAddress { frame: u32, offset: u32 },
    Sleeping { reason: u8 },
    Stopping,
    Dead,
}

impl CS {
    /// Đích **không-`Ready`** phía sim (D2-187/D2-194). `None` khi `CS::Ready`: `Ready`
    /// **không** biểu diễn được bằng [`ks::NonReadyTarget`] — phải đi qua **cửa ngữ nghĩa**
    /// ([`sim_to_ready`]).
    fn sim_nonready(self) -> Option<ks::NonReadyTarget> {
        Some(match self {
            CS::Ready => return None,
            CS::Inactive => ks::NonReadyTarget::Inactive,
            CS::Running { hart } => ks::NonReadyTarget::Running { hart },
            CS::BlockedOnSend { ep, call } => ks::NonReadyTarget::BlockedOnSend {
                ep,
                call,
                fault: None,
            },
            CS::BlockedOnRecv { ep, reply } => ks::NonReadyTarget::BlockedOnRecv {
                ep,
                reply: if reply == 0 { None } else { Some(reply) },
            },
            CS::BlockedOnReply { reply } => ks::NonReadyTarget::BlockedOnReply { reply },
            CS::BlockedOnNotification { ntfn } => {
                ks::NonReadyTarget::BlockedOnNotification { ntfn }
            }
            CS::BlockedOnAddress { frame, offset } => {
                ks::NonReadyTarget::BlockedOnAddress { frame, offset }
            }
            CS::Sleeping { reason } => ks::NonReadyTarget::Sleeping {
                reason: if reason == 0 {
                    ks::SleepReason::Timeout
                } else {
                    ks::SleepReason::Budget
                },
            },
            CS::Stopping => ks::NonReadyTarget::Stopping,
            CS::Dead => ks::NonReadyTarget::Dead,
        })
    }

    /// Như [`CS::sim_nonready`] nhưng phía core.
    fn core_nonready(self) -> Option<kc::NonReadyTarget> {
        Some(match self {
            CS::Ready => return None,
            CS::Inactive => kc::NonReadyTarget::Inactive,
            CS::Running { hart } => kc::NonReadyTarget::Running { hart },
            CS::BlockedOnSend { ep, call } => kc::NonReadyTarget::BlockedOnSend {
                ep,
                call,
                fault: None,
            },
            CS::BlockedOnRecv { ep, reply } => kc::NonReadyTarget::BlockedOnRecv {
                ep,
                reply: if reply == 0 { None } else { Some(reply) },
            },
            CS::BlockedOnReply { reply } => kc::NonReadyTarget::BlockedOnReply { reply },
            CS::BlockedOnNotification { ntfn } => {
                kc::NonReadyTarget::BlockedOnNotification { ntfn }
            }
            CS::BlockedOnAddress { frame, offset } => {
                kc::NonReadyTarget::BlockedOnAddress { frame, offset }
            }
            CS::Sleeping { reason } => kc::NonReadyTarget::Sleeping {
                reason: if reason == 0 {
                    kc::SleepReason::Timeout
                } else {
                    kc::SleepReason::Budget
                },
            },
            CS::Stopping => kc::NonReadyTarget::Stopping,
            CS::Dead => kc::NonReadyTarget::Dead,
        })
    }
}

/// Cửa `Ready` phía **sim** theo trạng thái nguồn (D2-187/D2-194): `Inactive` ⇒ `start_tcb`;
/// `Running` ⇒ `tcb_yield`; `BlockedOn*`/`Sleeping` ⇒ `wake` (test-only, D2-144); còn lại
/// (`Ready`/`Stopping`/`Dead`) ⇒ không cửa nào nhận ⇒ `IllegalOperation`.
fn sim_to_ready(m: &mut ks::Machine, tcb: u32) -> Res {
    match m.tcb_state(tcb) {
        Ok(ks::TcbState::Inactive) => unit_sim(m.start_tcb(tcb)),
        Ok(ks::TcbState::Running { .. }) => unit_sim(m.tcb_yield(tcb)),
        Ok(st) if st.is_blocked() || matches!(st, ks::TcbState::Sleeping { .. }) => {
            unit_sim(m.wake(tcb))
        }
        Ok(_) => Err(se(ks::KError::IllegalOperation)),
        Err(e) => Err(se(e)),
    }
}

/// Như [`sim_to_ready] nhưng phía **core**.
fn core_to_ready(m: &mut kc::Machine, tcb: u32) -> Res {
    match m.tcb_state(tcb) {
        Ok(kc::TcbState::Inactive) => unit_core(m.start_tcb(tcb)),
        Ok(kc::TcbState::Running { .. }) => unit_core(m.tcb_yield(tcb)),
        Ok(st) if st.is_blocked() || matches!(st, kc::TcbState::Sleeping { .. }) => {
            unit_core(m.wake(tcb))
        }
        Ok(_) => Err(ce(kc::KError::IllegalOperation)),
        Err(e) => Err(ce(e)),
    }
}

fn cs_of_sim(st: ks::TcbState) -> CS {
    match st {
        ks::TcbState::Inactive => CS::Inactive,
        ks::TcbState::Ready => CS::Ready,
        ks::TcbState::Running { hart } => CS::Running { hart },
        ks::TcbState::BlockedOnSend { ep, call, fault } => CS::BlockedOnSend {
            ep,
            call: call && fault.is_none(),
        },
        ks::TcbState::BlockedOnRecv { ep, reply } => CS::BlockedOnRecv {
            ep,
            reply: reply.unwrap_or(0),
        },
        ks::TcbState::BlockedOnReply { reply } => CS::BlockedOnReply { reply },
        ks::TcbState::BlockedOnNotification { ntfn } => CS::BlockedOnNotification { ntfn },
        ks::TcbState::BlockedOnAddress { frame, offset } => CS::BlockedOnAddress { frame, offset },
        ks::TcbState::Sleeping { reason } => CS::Sleeping {
            reason: match reason {
                ks::SleepReason::Timeout => 0,
                ks::SleepReason::Budget => 1,
            },
        },
        ks::TcbState::Stopping => CS::Stopping,
        ks::TcbState::Dead => CS::Dead,
    }
}

fn cs_of_core(st: kc::TcbState) -> CS {
    match st {
        kc::TcbState::Inactive => CS::Inactive,
        kc::TcbState::Ready => CS::Ready,
        kc::TcbState::Running { hart } => CS::Running { hart },
        kc::TcbState::BlockedOnSend { ep, call, fault } => CS::BlockedOnSend {
            ep,
            call: call && fault.is_none(),
        },
        kc::TcbState::BlockedOnRecv { ep, reply } => CS::BlockedOnRecv {
            ep,
            reply: reply.unwrap_or(0),
        },
        kc::TcbState::BlockedOnReply { reply } => CS::BlockedOnReply { reply },
        kc::TcbState::BlockedOnNotification { ntfn } => CS::BlockedOnNotification { ntfn },
        kc::TcbState::BlockedOnAddress { frame, offset } => CS::BlockedOnAddress { frame, offset },
        kc::TcbState::Sleeping { reason } => CS::Sleeping {
            reason: match reason {
                kc::SleepReason::Timeout => 0,
                kc::SleepReason::Budget => 1,
            },
        },
        kc::TcbState::Stopping => CS::Stopping,
        kc::TcbState::Dead => CS::Dead,
    }
}

/// Message hộp thư chuẩn hoá.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CMsg {
    Wake(u32),
    Reschedule,
    Migrate { tcb: u32, to_hart: u16 },
    Stop,
}

impl CMsg {
    fn sim(self) -> vs::mailbox::MailboxMsg {
        match self {
            CMsg::Wake(t) => vs::mailbox::MailboxMsg::Wake(t),
            CMsg::Reschedule => vs::mailbox::MailboxMsg::Reschedule,
            CMsg::Migrate { tcb, to_hart } => vs::mailbox::MailboxMsg::Migrate { tcb, to_hart },
            CMsg::Stop => vs::mailbox::MailboxMsg::Stop,
        }
    }

    fn core(self) -> kc::MailboxMsg {
        match self {
            CMsg::Wake(t) => kc::MailboxMsg::Wake(t),
            CMsg::Reschedule => kc::MailboxMsg::Reschedule,
            CMsg::Migrate { tcb, to_hart } => kc::MailboxMsg::Migrate { tcb, to_hart },
            CMsg::Stop => kc::MailboxMsg::Stop,
        }
    }

    fn fp(self) -> MFp {
        match self {
            CMsg::Wake(t) => MFp {
                tag: 1,
                a: t,
                b: 0,
                c: 0,
                d: 0,
            },
            CMsg::Reschedule => MFp {
                tag: 2,
                a: 0,
                b: 0,
                c: 0,
                d: 0,
            },
            CMsg::Migrate { tcb, to_hart } => MFp {
                tag: 3,
                a: tcb,
                b: to_hart,
                c: 0,
                d: 0,
            },
            CMsg::Stop => MFp {
                tag: 4,
                a: 0,
                b: 0,
                c: 0,
                d: 0,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MFp {
    tag: u8,
    a: u32,
    b: u16,
    c: u64,
    d: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InfoFp {
    state: CS,
    suspended: bool,
    exit_code: Option<u64>,
    fault_ep: Option<u32>,
    cspace: Option<u32>,
    priority: u8,
    max_controlled_priority: u8,
    affinity: u64,
    home: u16,
    sched_context: Option<u32>,
    donated_sc: Option<u32>,
    bound_notification: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SnapFp {
    state: CS,
    suspended: bool,
    blocked_on_kind: Option<u8>,
    exit_code: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IpcFp {
    buffer: [u64; 60],
    regs: [u64; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScFp {
    budget: u64,
    period: u64,
    consumed: u64,
    refill: u64,
    charged: u64,
    configured: bool,
    donated_to: Option<u32>,
    donor: Option<u32>,
}

// ---------------------------------------------------------------------------------------------
// Trait: cùng logic test chạy trên hai bản
// ---------------------------------------------------------------------------------------------

trait View {
    fn tcb_state(&self, obj: u32) -> Result<CS, E>;
    fn tcb_info(&self, slot: (u32, usize)) -> Result<InfoFp, E>;
    fn tcb_read(&self, slot: (u32, usize)) -> Result<SnapFp, E>;
    fn ipc_area(&self, obj: u32) -> Result<IpcFp, E>;
    fn sc_info(&self, slot: (u32, usize)) -> Result<ScFp, E>;
    fn sc_remaining(&self, slot: (u32, usize)) -> Result<u64, E>;
    fn has_budget(&self, obj: u32) -> bool;
    fn effective_priority(&self, obj: u32) -> Result<u8, E>;
    fn published(&self, h: u16) -> Result<(bool, u8), E>;
    fn current(&self, h: u16) -> Option<u32>;
    fn idle(&self, h: u16) -> bool;
    fn cur_prio(&self, h: u16) -> u8;
    fn ready_len(&self, h: u16) -> usize;
    fn ready_len_at(&self, h: u16, p: u8) -> usize;
    fn ready_contains(&self, h: u16, obj: u32) -> bool;
    fn sched_pick(&self, h: u16) -> Option<u32>;
    fn now(&self) -> u64;
    fn migrate_count(&self) -> u64;
    fn mail_len(&self, h: u16) -> Result<u64, E>;
    fn mail_log(&self, h: u16) -> Result<Vec<MFp>, E>;
    fn stopped(&self, h: u16) -> Result<bool, E>;
    fn finalise_log(&self) -> Vec<(u32, CS, u32)>;
    fn pair_hits(&self, from: usize, to: usize) -> u32;
    fn cov(&self) -> (usize, usize);
    fn check_sched(&self) -> bool;
    fn check_mailboxes(&self) -> bool;
    fn check_caps(&self) -> bool;
    fn check_bytes(&self) -> bool;
    fn live_caps(&self) -> usize;
}

trait Mut: View {
    fn add_untyped(&mut self, base: u64, size: usize) -> u32;
    fn add_cnode(&mut self, slots: usize) -> u32;
    fn add_kernel(&mut self, ty: Ty) -> u32;
    fn root_cap(&mut self, obj: u32) -> u32;
    fn put_cap(&mut self, cnode: u32, slot: usize, cap: u32) -> Res;
    fn configure(&mut self, tcb: (u32, usize), cspace: Option<(u32, usize)>) -> Res;
    fn set_fault_ep(&mut self, tcb: (u32, usize), ep: (u32, usize)) -> Res;
    fn sc_configure(
        &mut self,
        ctl: (u32, usize),
        sc: (u32, usize),
        budget: u64,
        period: u64,
    ) -> Res;
    fn set_priority(&mut self, tcb: (u32, usize), p: u8, mcp: u8) -> Res;
    fn set_affinity(&mut self, tcb: (u32, usize), mask: u64) -> Res;
    fn set_home(&mut self, tcb: (u32, usize), home: u16) -> Res;
    fn set_sched(&mut self, tcb: (u32, usize), sc: Option<(u32, usize)>) -> Res;
    fn transition(&mut self, tcb: u32, st: CS) -> Res;
    fn suspend(&mut self, tcb: (u32, usize), abort: bool) -> Res;
    fn resume(&mut self, tcb: (u32, usize)) -> Res;
    fn finalise(&mut self, tcb: u32) -> Res;
    fn tick(&mut self, n: u64) -> Res;
    fn dispatch(&mut self, h: u16) -> Res;
    fn block_current(&mut self, h: u16, st: CS) -> Res;
    fn wake(&mut self, tcb: u32) -> Res;
    fn yield_current(&mut self, h: u16) -> Res;
    fn post_mail(&mut self, h: u16, msg: CMsg) -> Res;
    fn process_mail(&mut self, h: u16) -> Res;
    fn send_mail(&mut self, h: u16, msg: CMsg) -> Res;
    fn push_migration(&mut self) -> Res;
    fn publish(&mut self, h: u16, idle: bool, prio: u8) -> Res;
    fn set_current_hart(&mut self, h: u16) -> Res;
    fn set_preempt(&mut self, on: bool);
}

fn se(e: ks::KError) -> E {
    (e.name(), e.id())
}

fn ce(e: kc::KError) -> E {
    (e.name(), e.id())
}

fn unit_sim(r: Result<(), ks::KError>) -> Res {
    r.map(|_| 0).map_err(se)
}

fn unit_core(r: Result<(), kc::KError>) -> Res {
    r.map(|_| 0).map_err(ce)
}

fn opt_sim(r: Result<Option<u32>, ks::KError>) -> Res {
    r.map(|o| o.unwrap_or(0) as u64).map_err(se)
}

fn opt_core(r: Result<Option<u32>, kc::KError>) -> Res {
    r.map(|o| o.unwrap_or(0) as u64).map_err(ce)
}

fn num_sim(r: Result<usize, ks::KError>) -> Res {
    r.map(|n| n as u64).map_err(se)
}

fn num_core(r: Result<usize, kc::KError>) -> Res {
    r.map(|n| n as u64).map_err(ce)
}

fn mf_sim(m: vs::mailbox::MailboxMsg) -> MFp {
    match m {
        vs::mailbox::MailboxMsg::Wake(t) => CMsg::Wake(t).fp(),
        vs::mailbox::MailboxMsg::Reschedule => CMsg::Reschedule.fp(),
        vs::mailbox::MailboxMsg::Migrate { tcb, to_hart } => CMsg::Migrate { tcb, to_hart }.fp(),
        vs::mailbox::MailboxMsg::Stop => CMsg::Stop.fp(),
        vs::mailbox::MailboxMsg::TlbFlush(f) => MFp {
            tag: 5,
            a: 0,
            b: f.asid,
            c: f.seq,
            d: f.ack,
        },
    }
}

fn mf_core(m: kc::MailboxMsg) -> MFp {
    match m {
        kc::MailboxMsg::Wake(t) => CMsg::Wake(t).fp(),
        kc::MailboxMsg::Reschedule => CMsg::Reschedule.fp(),
        kc::MailboxMsg::Migrate { tcb, to_hart } => CMsg::Migrate { tcb, to_hart }.fp(),
        kc::MailboxMsg::Stop => CMsg::Stop.fp(),
        kc::MailboxMsg::TlbFlush(f) => MFp {
            tag: 5,
            a: 0,
            b: f.asid,
            c: f.seq,
            d: f.ack,
        },
    }
}

impl View for ks::Machine {
    fn tcb_state(&self, obj: u32) -> Result<CS, E> {
        self.tcb_state(obj).map(cs_of_sim).map_err(se)
    }
    fn tcb_info(&self, slot: (u32, usize)) -> Result<InfoFp, E> {
        let i = self
            .tcb_info(ks::SlotRef::new(slot.0, slot.1))
            .map_err(se)?;
        Ok(InfoFp {
            state: cs_of_sim(i.state),
            suspended: i.suspended,
            exit_code: i.exit_code,
            fault_ep: i.fault_ep,
            cspace: i.cspace,
            priority: i.priority,
            max_controlled_priority: i.max_controlled_priority,
            affinity: i.affinity,
            home: i.home,
            sched_context: i.sched_context,
            donated_sc: i.donated_sc,
            bound_notification: i.bound_notification,
        })
    }
    fn tcb_read(&self, slot: (u32, usize)) -> Result<SnapFp, E> {
        let s = self
            .tcb_read_state(ks::SlotRef::new(slot.0, slot.1))
            .map_err(se)?;
        Ok(SnapFp {
            state: cs_of_sim(s.state),
            suspended: s.suspended,
            blocked_on_kind: s.blocked_on_kind.map(|k| match k {
                ks::BlockedOnKind::Send => 0,
                ks::BlockedOnKind::Recv => 1,
                ks::BlockedOnKind::Reply => 2,
                ks::BlockedOnKind::Notification => 3,
                ks::BlockedOnKind::Address => 4,
            }),
            exit_code: s.exit_code,
        })
    }
    fn ipc_area(&self, obj: u32) -> Result<IpcFp, E> {
        Ok(IpcFp {
            buffer: self.tcb_ipc_buffer(obj).map_err(se)?,
            regs: self.tcb_regs(obj).map_err(se)?,
        })
    }
    fn sc_info(&self, slot: (u32, usize)) -> Result<ScFp, E> {
        let s = self
            .sched_info(ks::SlotRef::new(slot.0, slot.1))
            .map_err(se)?;
        Ok(ScFp {
            budget: s.budget,
            period: s.period,
            consumed: s.consumed,
            refill: s.refill,
            charged: s.charged,
            configured: s.configured,
            donated_to: s.donated_to,
            donor: s.donor,
        })
    }
    fn sc_remaining(&self, slot: (u32, usize)) -> Result<u64, E> {
        self.sched_remaining(ks::SlotRef::new(slot.0, slot.1))
            .map_err(se)
    }
    fn has_budget(&self, obj: u32) -> bool {
        self.tcb_has_budget(obj)
    }
    fn effective_priority(&self, obj: u32) -> Result<u8, E> {
        self.effective_priority(obj).map_err(se)
    }
    fn published(&self, h: u16) -> Result<(bool, u8), E> {
        self.published(h).map(|p| (p.idle, p.cur_prio)).map_err(se)
    }
    fn current(&self, h: u16) -> Option<u32> {
        self.current(h)
    }
    fn idle(&self, h: u16) -> bool {
        self.sched_idle(h)
    }
    fn cur_prio(&self, h: u16) -> u8 {
        self.sched_cur_prio(h)
    }
    fn ready_len(&self, h: u16) -> usize {
        self.ready_len(h)
    }
    fn ready_len_at(&self, h: u16, p: u8) -> usize {
        self.ready_len_at(h, p)
    }
    fn ready_contains(&self, h: u16, obj: u32) -> bool {
        self.ready_contains(h, obj)
    }
    fn sched_pick(&self, h: u16) -> Option<u32> {
        self.sched_pick_on(h)
    }
    fn now(&self) -> u64 {
        self.virtual_now()
    }
    fn migrate_count(&self) -> u64 {
        self.migrate_count()
    }
    fn mail_len(&self, h: u16) -> Result<u64, E> {
        self.mail_len(h).map(|n| n as u64).map_err(se)
    }
    fn mail_log(&self, h: u16) -> Result<Vec<MFp>, E> {
        self.mail_log(h)
            .map(|l| l.iter().copied().map(mf_sim).collect())
            .map_err(se)
    }
    fn stopped(&self, h: u16) -> Result<bool, E> {
        self.hart_stopped(h).map_err(se)
    }
    fn finalise_log(&self) -> Vec<(u32, CS, u32)> {
        self.finalise_log()
            .iter()
            .map(|e| {
                (
                    e.tcb,
                    cs_of_sim(e.state),
                    e.released_sched_context.unwrap_or(0),
                )
            })
            .collect()
    }
    fn pair_hits(&self, from: usize, to: usize) -> u32 {
        self.pair_hit_count(from, to)
    }
    fn cov(&self) -> (usize, usize) {
        self.transition_coverage()
    }
    fn check_sched(&self) -> bool {
        self.check_sched().is_ok()
    }
    fn check_mailboxes(&self) -> bool {
        self.check_mailboxes().is_ok()
    }
    fn check_caps(&self) -> bool {
        self.check_caps().is_ok()
    }
    fn check_bytes(&self) -> bool {
        self.check_all_bytes().is_ok()
    }
    fn live_caps(&self) -> usize {
        self.live_cap_count()
    }
}

impl Mut for ks::Machine {
    fn add_untyped(&mut self, base: u64, size: usize) -> u32 {
        self.add_untyped(base, size)
    }
    fn add_cnode(&mut self, slots: usize) -> u32 {
        self.add_cnode(slots)
    }
    fn add_kernel(&mut self, ty: Ty) -> u32 {
        self.add_kernel_object(ty.sim())
    }
    fn root_cap(&mut self, obj: u32) -> u32 {
        self.root_cap(obj)
    }
    fn put_cap(&mut self, cnode: u32, slot: usize, cap: u32) -> Res {
        unit_sim(self.put_cap(cnode, slot, cap))
    }
    fn configure(&mut self, tcb: (u32, usize), cspace: Option<(u32, usize)>) -> Res {
        unit_sim(self.tcb_configure(
            ks::SlotRef::new(tcb.0, tcb.1),
            cspace.map(|c| ks::SlotRef::new(c.0, c.1)),
        ))
    }
    fn set_fault_ep(&mut self, tcb: (u32, usize), ep: (u32, usize)) -> Res {
        unit_sim(
            self.tcb_set_fault_ep(ks::SlotRef::new(tcb.0, tcb.1), ks::SlotRef::new(ep.0, ep.1)),
        )
    }
    fn sc_configure(
        &mut self,
        ctl: (u32, usize),
        sc: (u32, usize),
        budget: u64,
        period: u64,
    ) -> Res {
        unit_sim(self.sched_control_configure(
            ks::SlotRef::new(ctl.0, ctl.1),
            ks::SlotRef::new(sc.0, sc.1),
            budget,
            period,
        ))
    }
    fn set_priority(&mut self, tcb: (u32, usize), p: u8, mcp: u8) -> Res {
        unit_sim(self.tcb_set_priority(ks::SlotRef::new(tcb.0, tcb.1), p, mcp))
    }
    fn set_affinity(&mut self, tcb: (u32, usize), mask: u64) -> Res {
        unit_sim(self.tcb_set_affinity(ks::SlotRef::new(tcb.0, tcb.1), mask))
    }
    fn set_home(&mut self, tcb: (u32, usize), home: u16) -> Res {
        unit_sim(self.tcb_set_home(ks::SlotRef::new(tcb.0, tcb.1), home))
    }
    fn set_sched(&mut self, tcb: (u32, usize), sc: Option<(u32, usize)>) -> Res {
        unit_sim(self.set_sched_params(
            ks::SlotRef::new(tcb.0, tcb.1),
            sc.map(|s| ks::SlotRef::new(s.0, s.1)),
        ))
    }
    fn transition(&mut self, tcb: u32, st: CS) -> Res {
        match st.sim_nonready() {
            Some(to) => unit_sim(self.transition_tcb(tcb, to)),
            None => sim_to_ready(self, tcb),
        }
    }
    fn suspend(&mut self, tcb: (u32, usize), abort: bool) -> Res {
        unit_sim(self.tcb_suspend(
            ks::SlotRef::new(tcb.0, tcb.1),
            if abort {
                ks::SuspendMode::Abort
            } else {
                ks::SuspendMode::Defer
            },
        ))
    }
    fn resume(&mut self, tcb: (u32, usize)) -> Res {
        unit_sim(self.tcb_resume(ks::SlotRef::new(tcb.0, tcb.1)))
    }
    fn finalise(&mut self, tcb: u32) -> Res {
        unit_sim(self.finalise_tcb(tcb))
    }
    fn tick(&mut self, n: u64) -> Res {
        unit_sim(self.tick(n))
    }
    fn dispatch(&mut self, h: u16) -> Res {
        opt_sim(self.dispatch(h))
    }
    fn block_current(&mut self, h: u16, st: CS) -> Res {
        match st.sim_nonready() {
            Some(to) => self.block_current(h, to).map(|o| o as u64).map_err(se),
            // `Ready` không phải đích block hợp lệ (D2-194 §1(d)) — giữ nguyên mã lỗi cũ.
            None => Err(se(ks::KError::InvalidArgument)),
        }
    }
    fn wake(&mut self, tcb: u32) -> Res {
        unit_sim(self.wake(tcb))
    }
    fn yield_current(&mut self, h: u16) -> Res {
        opt_sim(self.yield_current(h))
    }
    fn post_mail(&mut self, h: u16, msg: CMsg) -> Res {
        unit_sim(self.post_mail(h, msg.sim()))
    }
    fn process_mail(&mut self, h: u16) -> Res {
        num_sim(self.process_mail(h))
    }
    fn send_mail(&mut self, h: u16, msg: CMsg) -> Res {
        unit_sim(self.send_mail(h, msg.sim()))
    }
    fn push_migration(&mut self) -> Res {
        num_sim(self.push_migration())
    }
    fn publish(&mut self, h: u16, idle: bool, prio: u8) -> Res {
        unit_sim(self.publish(h, idle, prio))
    }
    fn set_current_hart(&mut self, h: u16) -> Res {
        unit_sim(self.set_current_hart(h))
    }
    fn set_preempt(&mut self, on: bool) {
        self.set_preempt_every_step(on);
    }
}

impl View for kc::Machine {
    fn tcb_state(&self, obj: u32) -> Result<CS, E> {
        self.tcb_state(obj).map(cs_of_core).map_err(ce)
    }
    fn tcb_info(&self, slot: (u32, usize)) -> Result<InfoFp, E> {
        let i = self
            .tcb_info(kc::SlotRef::new(slot.0, slot.1))
            .map_err(ce)?;
        Ok(InfoFp {
            state: cs_of_core(i.state),
            suspended: i.suspended,
            exit_code: i.exit_code,
            fault_ep: i.fault_ep,
            cspace: i.cspace,
            priority: i.priority,
            max_controlled_priority: i.max_controlled_priority,
            affinity: i.affinity,
            home: i.home,
            sched_context: i.sched_context,
            donated_sc: i.donated_sc,
            bound_notification: i.bound_notification,
        })
    }
    fn tcb_read(&self, slot: (u32, usize)) -> Result<SnapFp, E> {
        let s = self
            .tcb_read_state(kc::SlotRef::new(slot.0, slot.1))
            .map_err(ce)?;
        Ok(SnapFp {
            state: cs_of_core(s.state),
            suspended: s.suspended,
            blocked_on_kind: s.blocked_on_kind.map(|k| match k {
                kc::BlockedOnKind::Send => 0,
                kc::BlockedOnKind::Recv => 1,
                kc::BlockedOnKind::Reply => 2,
                kc::BlockedOnKind::Notification => 3,
                kc::BlockedOnKind::Address => 4,
            }),
            exit_code: s.exit_code,
        })
    }
    fn ipc_area(&self, obj: u32) -> Result<IpcFp, E> {
        Ok(IpcFp {
            buffer: self.tcb_ipc_buffer(obj).map_err(ce)?,
            regs: self.tcb_regs(obj).map_err(ce)?,
        })
    }
    fn sc_info(&self, slot: (u32, usize)) -> Result<ScFp, E> {
        let s = self
            .sched_info(kc::SlotRef::new(slot.0, slot.1))
            .map_err(ce)?;
        Ok(ScFp {
            budget: s.budget,
            period: s.period,
            consumed: s.consumed,
            refill: s.refill,
            charged: s.charged,
            configured: s.configured,
            donated_to: s.donated_to,
            donor: s.donor,
        })
    }
    fn sc_remaining(&self, slot: (u32, usize)) -> Result<u64, E> {
        self.sched_remaining(kc::SlotRef::new(slot.0, slot.1))
            .map_err(ce)
    }
    fn has_budget(&self, obj: u32) -> bool {
        self.tcb_has_budget(obj)
    }
    fn effective_priority(&self, obj: u32) -> Result<u8, E> {
        self.effective_priority(obj).map_err(ce)
    }
    fn published(&self, h: u16) -> Result<(bool, u8), E> {
        self.published(h).map(|p| (p.idle, p.cur_prio)).map_err(ce)
    }
    fn current(&self, h: u16) -> Option<u32> {
        self.current(h)
    }
    fn idle(&self, h: u16) -> bool {
        self.sched_idle(h)
    }
    fn cur_prio(&self, h: u16) -> u8 {
        self.sched_cur_prio(h)
    }
    fn ready_len(&self, h: u16) -> usize {
        self.ready_len(h)
    }
    fn ready_len_at(&self, h: u16, p: u8) -> usize {
        self.ready_len_at(h, p)
    }
    fn ready_contains(&self, h: u16, obj: u32) -> bool {
        self.ready_contains(h, obj)
    }
    fn sched_pick(&self, h: u16) -> Option<u32> {
        self.sched_pick_on(h)
    }
    fn now(&self) -> u64 {
        self.virtual_now()
    }
    fn migrate_count(&self) -> u64 {
        self.migrate_count()
    }
    fn mail_len(&self, h: u16) -> Result<u64, E> {
        self.mail_len(h).map(|n| n as u64).map_err(ce)
    }
    fn mail_log(&self, h: u16) -> Result<Vec<MFp>, E> {
        self.mail_log(h)
            .map(|l| l.iter().copied().map(mf_core).collect())
            .map_err(ce)
    }
    fn stopped(&self, h: u16) -> Result<bool, E> {
        self.hart_stopped(h).map_err(ce)
    }
    fn finalise_log(&self) -> Vec<(u32, CS, u32)> {
        self.finalise_log()
            .iter()
            .map(|e| {
                (
                    e.tcb,
                    cs_of_core(e.state),
                    e.released_sched_context.unwrap_or(0),
                )
            })
            .collect()
    }
    fn pair_hits(&self, from: usize, to: usize) -> u32 {
        self.pair_hit_count(from, to)
    }
    fn cov(&self) -> (usize, usize) {
        self.transition_coverage()
    }
    fn check_sched(&self) -> bool {
        self.check_sched().is_ok()
    }
    fn check_mailboxes(&self) -> bool {
        self.check_mailboxes().is_ok()
    }
    fn check_caps(&self) -> bool {
        self.check_caps().is_ok()
    }
    fn check_bytes(&self) -> bool {
        self.check_all_bytes().is_ok()
    }
    fn live_caps(&self) -> usize {
        self.live_cap_count()
    }
}

impl Mut for kc::Machine {
    fn add_untyped(&mut self, base: u64, size: usize) -> u32 {
        self.add_untyped(base, size)
    }
    fn add_cnode(&mut self, slots: usize) -> u32 {
        self.add_cnode(slots)
    }
    fn add_kernel(&mut self, ty: Ty) -> u32 {
        self.add_kernel_object(ty.core())
    }
    fn root_cap(&mut self, obj: u32) -> u32 {
        self.root_cap(obj)
    }
    fn put_cap(&mut self, cnode: u32, slot: usize, cap: u32) -> Res {
        unit_core(self.put_cap(cnode, slot, cap))
    }
    fn configure(&mut self, tcb: (u32, usize), cspace: Option<(u32, usize)>) -> Res {
        unit_core(self.tcb_configure(
            kc::SlotRef::new(tcb.0, tcb.1),
            cspace.map(|c| kc::SlotRef::new(c.0, c.1)),
        ))
    }
    fn set_fault_ep(&mut self, tcb: (u32, usize), ep: (u32, usize)) -> Res {
        unit_core(
            self.tcb_set_fault_ep(kc::SlotRef::new(tcb.0, tcb.1), kc::SlotRef::new(ep.0, ep.1)),
        )
    }
    fn sc_configure(
        &mut self,
        ctl: (u32, usize),
        sc: (u32, usize),
        budget: u64,
        period: u64,
    ) -> Res {
        unit_core(self.sched_control_configure(
            kc::SlotRef::new(ctl.0, ctl.1),
            kc::SlotRef::new(sc.0, sc.1),
            budget,
            period,
        ))
    }
    fn set_priority(&mut self, tcb: (u32, usize), p: u8, mcp: u8) -> Res {
        unit_core(self.tcb_set_priority(kc::SlotRef::new(tcb.0, tcb.1), p, mcp))
    }
    fn set_affinity(&mut self, tcb: (u32, usize), mask: u64) -> Res {
        unit_core(self.tcb_set_affinity(kc::SlotRef::new(tcb.0, tcb.1), mask))
    }
    fn set_home(&mut self, tcb: (u32, usize), home: u16) -> Res {
        unit_core(self.tcb_set_home(kc::SlotRef::new(tcb.0, tcb.1), home))
    }
    fn set_sched(&mut self, tcb: (u32, usize), sc: Option<(u32, usize)>) -> Res {
        unit_core(self.set_sched_params(
            kc::SlotRef::new(tcb.0, tcb.1),
            sc.map(|s| kc::SlotRef::new(s.0, s.1)),
        ))
    }
    fn transition(&mut self, tcb: u32, st: CS) -> Res {
        match st.core_nonready() {
            Some(to) => unit_core(self.transition_tcb(tcb, to)),
            None => core_to_ready(self, tcb),
        }
    }
    fn suspend(&mut self, tcb: (u32, usize), abort: bool) -> Res {
        unit_core(self.tcb_suspend(
            kc::SlotRef::new(tcb.0, tcb.1),
            if abort {
                kc::SuspendMode::Abort
            } else {
                kc::SuspendMode::Defer
            },
        ))
    }
    fn resume(&mut self, tcb: (u32, usize)) -> Res {
        unit_core(self.tcb_resume(kc::SlotRef::new(tcb.0, tcb.1)))
    }
    fn finalise(&mut self, tcb: u32) -> Res {
        unit_core(self.finalise_tcb(tcb))
    }
    fn tick(&mut self, n: u64) -> Res {
        unit_core(self.tick(n))
    }
    fn dispatch(&mut self, h: u16) -> Res {
        opt_core(self.dispatch(h))
    }
    fn block_current(&mut self, h: u16, st: CS) -> Res {
        match st.core_nonready() {
            Some(to) => self.block_current(h, to).map(|o| o as u64).map_err(ce),
            None => Err(ce(kc::KError::InvalidArgument)),
        }
    }
    fn wake(&mut self, tcb: u32) -> Res {
        unit_core(self.wake(tcb))
    }
    fn yield_current(&mut self, h: u16) -> Res {
        opt_core(self.yield_current(h))
    }
    fn post_mail(&mut self, h: u16, msg: CMsg) -> Res {
        unit_core(self.post_mail(h, msg.core()))
    }
    fn process_mail(&mut self, h: u16) -> Res {
        num_core(self.process_mail(h))
    }
    fn send_mail(&mut self, h: u16, msg: CMsg) -> Res {
        unit_core(self.send_mail(h, msg.core()))
    }
    fn push_migration(&mut self) -> Res {
        num_core(self.push_migration())
    }
    fn publish(&mut self, h: u16, idle: bool, prio: u8) -> Res {
        unit_core(self.publish(h, idle, prio))
    }
    fn set_current_hart(&mut self, h: u16) -> Res {
        unit_core(self.set_current_hart(h))
    }
    fn set_preempt(&mut self, on: bool) {
        self.set_preempt_every_step(on);
    }
}

// ---------------------------------------------------------------------------------------------
// Digest
// ---------------------------------------------------------------------------------------------

type TcbRow = Result<(CS, InfoFp, SnapFp, Option<ScFp>, u64, bool, u8, IpcFp), E>;
type HartRow = (
    u16,
    Result<(bool, u8), E>,
    Option<u32>,
    bool,
    u8,
    u64,
    Option<u32>,
    [u16; 256],
    Vec<bool>,
);
type MailRow = (Result<u64, E>, Result<Vec<MFp>, E>, Result<bool, E>);

#[derive(Debug, Clone, PartialEq, Eq)]
struct Digest {
    now: u64,
    migrate: u64,
    live_caps: usize,
    tcbs: Vec<(u32, TcbRow)>,
    scs: Vec<(u32, Result<ScFp, E>)>,
    finalise: Vec<(u32, CS, u32)>,
    harts: Vec<HartRow>,
    mails: Vec<MailRow>,
    pairs: Vec<u32>,
    cov: (usize, usize),
    checks: (bool, bool, bool, bool),
}

/// Rig cố định của kịch bản: TCB, slot SlotRef, SchedContext, hart.
#[derive(Debug, Clone)]
struct Rig {
    tcbs: Vec<u32>,
    tslots: Vec<(u32, usize)>,
    scs: Vec<(u32, usize)>,
    sc_objs: Vec<u32>,
    ctl: (u32, usize),
    ep: (u32, usize),
    cspace: (u32, usize),
    n_harts: u16,
}

fn digest(v: &impl View, rig: &Rig) -> Digest {
    let mut tcbs = Vec::new();
    for (i, &t) in rig.tcbs.iter().enumerate() {
        let slot = rig.tslots[i];
        let st = v.tcb_state(t);
        let info = v.tcb_info(slot);
        let read = v.tcb_read(slot);
        let sc = info
            .as_ref()
            .ok()
            .and_then(|i| i.sched_context)
            .and_then(|sc_obj| {
                rig.scs
                    .iter()
                    .zip(rig.sc_objs.iter())
                    .find(|(_, o)| **o == sc_obj)
                    .and_then(|(slot, _)| v.sc_info(*slot).ok())
            });
        let rem = match rig
            .scs
            .iter()
            .zip(rig.sc_objs.iter())
            .find(|(_, o)| Some(**o) == info.as_ref().ok().and_then(|i| i.sched_context))
        {
            Some((slot, _)) => v.sc_remaining(*slot).unwrap_or(u64::MAX),
            None => u64::MAX,
        };
        let has = v.has_budget(t);
        let eff = v.effective_priority(t);
        let ipc = v.ipc_area(t);
        let row = match (st, info, read, ipc) {
            (Ok(st), Ok(info), Ok(read), Ok(ipc)) => {
                Ok((st, info, read, sc, rem, has, eff.unwrap_or(255), ipc))
            }
            (st, info, read, ipc) => Err(st
                .err()
                .or_else(|| info.err())
                .or_else(|| read.err())
                .or_else(|| ipc.err())
                .unwrap()),
        };
        tcbs.push((t, row));
    }
    let scs: Vec<_> = rig
        .scs
        .iter()
        .zip(rig.sc_objs.iter())
        .map(|(slot, &obj)| (obj, v.sc_info(*slot)))
        .collect();
    let mut harts = Vec::new();
    for h in 0..rig.n_harts {
        let mut len_at = [0u16; 256];
        for (p, slot) in len_at.iter_mut().enumerate() {
            *slot = v.ready_len_at(h, p as u8) as u16;
        }
        let mut contains = Vec::new();
        for &t in &rig.tcbs {
            contains.push(v.ready_contains(h, t));
        }
        harts.push((
            h,
            v.published(h),
            v.current(h),
            v.idle(h),
            v.cur_prio(h),
            v.ready_len(h) as u64,
            v.sched_pick(h),
            len_at,
            contains,
        ));
    }
    let mails = (0..rig.n_harts)
        .map(|h| (v.mail_len(h), v.mail_log(h), v.stopped(h)))
        .collect();
    let mut pairs = Vec::new();
    for from in 0..11 {
        for to in 0..11 {
            pairs.push(v.pair_hits(from, to));
        }
    }
    Digest {
        now: v.now(),
        migrate: v.migrate_count(),
        live_caps: v.live_caps(),
        tcbs,
        scs,
        finalise: v.finalise_log(),
        harts,
        mails,
        pairs,
        cov: v.cov(),
        checks: (
            v.check_sched(),
            v.check_mailboxes(),
            v.check_caps(),
            v.check_bytes(),
        ),
    }
}

// ---------------------------------------------------------------------------------------------
// Ops + Step
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
enum Op {
    Configure {
        tcb: (u32, usize),
        cspace: Option<(u32, usize)>,
    },
    SetFaultEp {
        tcb: (u32, usize),
        ep: (u32, usize),
    },
    ScConfigure {
        ctl: (u32, usize),
        sc: (u32, usize),
        budget: u64,
        period: u64,
    },
    SetPriority {
        tcb: (u32, usize),
        p: u8,
        mcp: u8,
    },
    SetAffinity {
        tcb: (u32, usize),
        mask: u64,
    },
    SetHome {
        tcb: (u32, usize),
        home: u16,
    },
    SetSched {
        tcb: (u32, usize),
        sc: Option<(u32, usize)>,
    },
    Transition {
        tcb: u32,
        st: CS,
    },
    Suspend {
        tcb: (u32, usize),
        abort: bool,
    },
    Resume {
        tcb: (u32, usize),
    },
    Finalise {
        tcb: u32,
    },
    Tick {
        n: u64,
    },
    Dispatch {
        h: u16,
    },
    BlockCurrent {
        h: u16,
        st: CS,
    },
    Wake {
        tcb: u32,
    },
    Yield {
        h: u16,
    },
    PostMail {
        h: u16,
        msg: CMsg,
    },
    ProcessMail {
        h: u16,
    },
    SendMail {
        h: u16,
        msg: CMsg,
    },
    PushMigration,
    Publish {
        h: u16,
        idle: bool,
        prio: u8,
    },
    SetCurrentHart {
        h: u16,
    },
    SetPreempt {
        on: bool,
    },
}

fn apply(m: &mut dyn Mut, op: Op) -> Res {
    match op {
        Op::Configure { tcb, cspace } => m.configure(tcb, cspace),
        Op::SetFaultEp { tcb, ep } => m.set_fault_ep(tcb, ep),
        Op::ScConfigure {
            ctl,
            sc,
            budget,
            period,
        } => m.sc_configure(ctl, sc, budget, period),
        Op::SetPriority { tcb, p, mcp } => m.set_priority(tcb, p, mcp),
        Op::SetAffinity { tcb, mask } => m.set_affinity(tcb, mask),
        Op::SetHome { tcb, home } => m.set_home(tcb, home),
        Op::SetSched { tcb, sc } => m.set_sched(tcb, sc),
        Op::Transition { tcb, st } => m.transition(tcb, st),
        Op::Suspend { tcb, abort } => m.suspend(tcb, abort),
        Op::Resume { tcb } => m.resume(tcb),
        Op::Finalise { tcb } => m.finalise(tcb),
        Op::Tick { n } => m.tick(n),
        Op::Dispatch { h } => m.dispatch(h),
        Op::BlockCurrent { h, st } => m.block_current(h, st),
        Op::Wake { tcb } => m.wake(tcb),
        Op::Yield { h } => m.yield_current(h),
        Op::PostMail { h, msg } => m.post_mail(h, msg),
        Op::ProcessMail { h } => m.process_mail(h),
        Op::SendMail { h, msg } => m.send_mail(h, msg),
        Op::PushMigration => m.push_migration(),
        Op::Publish { h, idle, prio } => m.publish(h, idle, prio),
        Op::SetCurrentHart { h } => m.set_current_hart(h),
        Op::SetPreempt { on } => {
            m.set_preempt(on);
            Ok(0)
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Step {
    op: Op,
    expect: Option<Res>,
}

fn plain(op: Op) -> Step {
    Step { op, expect: None }
}

fn ok(op: Op, v: u64) -> Step {
    Step {
        op,
        expect: Some(Ok(v)),
    }
}

fn err(op: Op, e: kc::KError) -> Step {
    Step {
        op,
        expect: Some(Err((e.name(), e.id()))),
    }
}

fn run_steps(s: &mut ks::Machine, c: &mut kc::Machine, rig: &Rig, steps: &[Step]) {
    for (i, st) in steps.iter().enumerate() {
        let a = apply(s, st.op);
        let b = apply(c, st.op);
        assert_eq!(a, b, "bước {i} {:?}: kết quả sim/core lệch", st.op);
        if let Some(exp) = st.expect {
            assert_eq!(a, exp, "bước {i} {:?}: kết quả khác kỳ vọng cứng", st.op);
        }
        let da = digest(s, rig);
        let db = digest(c, rig);
        assert_eq!(
            da, db,
            "bước {i} {:?}: digest sim/core lệch\nsim={da:?}\ncore={db:?}",
            st.op
        );
        assert_eq!(
            da.checks,
            (true, true, true, true),
            "bước {i} {:?}: bất biến vỡ (sim={:?})",
            st.op,
            da.checks
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Rig builder
// ---------------------------------------------------------------------------------------------

/// Gọi cùng một closure trên hai máy, trả id (đã so khớp).
fn both_id(s: &mut ks::Machine, c: &mut kc::Machine, f: impl Fn(&mut dyn Mut) -> u32) -> u32 {
    let a = f(s);
    let b = f(c);
    assert_eq!(a, b, "id sim/core lệch");
    a
}

fn both_res(s: &mut ks::Machine, c: &mut kc::Machine, f: impl Fn(&mut dyn Mut) -> Res) -> Res {
    let a = f(s);
    let b = f(c);
    assert_eq!(a, b, "kết quả sim/core lệch khi dựng rig");
    a
}

fn rig(harts: u16, n_tcb: usize) -> (ks::Machine, kc::Machine, Rig) {
    let mut s = ks::Machine::with_harts(harts);
    let mut c = kc::Machine::with_harts(harts);
    // CNode 128 slot, Untyped (để test Retype ở test core-only dùng chung id), CNode CSpace,
    // Endpoint (fault_ep), SchedControl, 3 SchedContext, n TCB.
    let c0 = both_id(&mut s, &mut c, |m| m.add_cnode(128));
    let _u0 = both_id(&mut s, &mut c, |m| m.add_untyped(0x1000, 1 << 20));
    let c1 = both_id(&mut s, &mut c, |m| m.add_cnode(8));
    let ep = both_id(&mut s, &mut c, |m| m.add_kernel(Ty::Endpoint));
    let ctl = both_id(&mut s, &mut c, |m| m.add_kernel(Ty::SchedControl));
    let mut sc_objs = Vec::new();
    for _ in 0..3 {
        sc_objs.push(both_id(&mut s, &mut c, |m| m.add_kernel(Ty::SchedContext)));
    }
    let mut tcbs = Vec::new();
    for _ in 0..n_tcb {
        tcbs.push(both_id(&mut s, &mut c, |m| m.add_kernel(Ty::Tcb)));
    }
    // Cap gốc + đặt slot: 0 = CNode CSpace, 1 = Endpoint, 2 = SchedControl, 3..6 = SC,
    // 10.. = TCB.
    let put = |s: &mut ks::Machine, c: &mut kc::Machine, obj: u32, slot: usize| {
        let cap = both_id(s, c, |m| m.root_cap(obj));
        assert_eq!(
            both_res(s, c, |m| m.put_cap(c0, slot, cap)),
            Ok(0),
            "put_cap slot {slot}"
        );
    };
    put(&mut s, &mut c, c1, 0);
    put(&mut s, &mut c, ep, 1);
    put(&mut s, &mut c, ctl, 2);
    for (i, &sc) in sc_objs.iter().enumerate() {
        put(&mut s, &mut c, sc, 3 + i);
    }
    for (i, &t) in tcbs.iter().enumerate() {
        put(&mut s, &mut c, t, 10 + i);
    }
    let r = Rig {
        tslots: (0..n_tcb).map(|i| (c0, 10 + i)).collect(),
        tcbs,
        scs: (0..3).map(|i| (c0, 3 + i)).collect(),
        sc_objs,
        ctl: (c0, 2),
        ep: (c0, 1),
        cspace: (c0, 0),
        n_harts: harts,
    };
    (s, c, r)
}

/// Gắn SC đã Configure cho từng TCB + đưa `Ready` — như rig KT-19/20 (test-only).
fn arm_rig(s: &mut ks::Machine, c: &mut kc::Machine, r: &Rig, budget: u64, period: u64) {
    for i in 0..r.tcbs.len() {
        let sc = r.scs[i % r.scs.len()];
        assert_eq!(
            both_res(s, c, |m| { m.sc_configure(r.ctl, sc, budget, period) }),
            Ok(0),
            "arm_rig: Configure SC {i}"
        );
        assert_eq!(
            both_res(s, c, |m| m.set_sched(r.tslots[i], Some(sc))),
            Ok(0),
            "arm_rig: set_sched_params {i}"
        );
        assert_eq!(
            both_res(s, c, |m| m.transition(r.tcbs[i], CS::Ready)),
            Ok(0),
            "arm_rig: Inactive → Ready {i}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// 1. Kịch bản tường minh — KT-18: defaults, configure, bảng ALLOWED, suspend/resume/finalise
// ---------------------------------------------------------------------------------------------
