// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-60 — parity core `no_std` (`vifast-core`, K3b tầng 3: IPC/Reply/Notification/WaitAddr/
//! fault) với **sim** (oracle).
//!
//! Cùng một `Op` chạy trên `vifast_sim::kobj::Machine` và `vifast_core::Machine` qua trait
//! [`View`]/[`Mut`]; sau **mỗi** bước so:
//! - kết quả (`Ok`/`Err` + **tên + số** mã lỗi — cùng bộ 19 mã sinh từ spec);
//! - digest: TCB (state + payload, `TcbInfo`, `Tcb.ReadState`, vùng nhận IPC `info/regs/
//!   buffer/badge/msg_id`, `last_ipc_error`, `last_fault`, `notif`), Endpoint (hàng gửi FIFO
//!   **đầy đủ nội dung message** + `pushed/popped/removed` + Recv waiter + `last_mr`), Reply
//!   (`caller`), Notification (word/badge/waiter/bound), WaitAddr (waiter theo khoá + giá trị),
//!   bộ đếm IPC, hộp thư hart (**nội dung** hàng + nhật ký + stopped), log finalise;
//! - sáu bất biến `check_ipc`/`check_reply_links`/`check_notifications`/`check_waitaddr`/
//!   `check_fault`/`check_sched` + `check_caps`/`check_all_bytes` của **cả hai** phải `Ok`.
//!
//! Kịch bản: KT-24 (hàng FIFO/Busy), KT-25 (Call/Reply/ReplyRecv), KT-27 (Notification badge
//! OR/wait/wake/bound), KT-28 (WaitAddr FIFO/va chạm bucket/**không timeout**), KT-26 + D2-25
//! (fault `Exit` không Reply), KT-34 (Reply tái dùng + hai hook `transition_tcb`), KT-21
//! (donation trả khi rời `BlockedOnReply`).
//!
//! **Biên KT-60:** chưa chuyển cap (receive window) — message có cap không được dựng trong
//! kịch bản (nợ §8 `docs/k3b-core.md`); đây là biên task, không phải nới kỳ vọng.

use vifast_core as kc;
use vifast_sim as vs;
use vs::kobj as ks;

// ---------------------------------------------------------------------------------------------
// Kiểu chung
// ---------------------------------------------------------------------------------------------

type E = (&'static str, u16);
type Res = Result<u64, E>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CS {
    Inactive,
    Ready,
    Running { hart: u16 },
    BlockedOnSend { ep: u32, call: bool, fault: u8 },
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
            CS::BlockedOnSend { ep, call, fault } => ks::NonReadyTarget::BlockedOnSend {
                ep,
                call,
                fault: if fault == 0 {
                    None
                } else {
                    Some(ks::FaultKind::Exit { code: fault as u64 })
                },
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
            CS::BlockedOnSend { ep, call, fault } => kc::NonReadyTarget::BlockedOnSend {
                ep,
                call,
                fault: if fault == 0 {
                    None
                } else {
                    Some(kc::FaultKind::Exit { code: fault as u64 })
                },
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

/// Như [`sim_to_ready`] nhưng phía **core**.
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
            call,
            fault: match fault {
                None => 0,
                Some(ks::FaultKind::Exit { code }) => code as u8,
                Some(_) => 99,
            },
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
            call,
            fault: match fault {
                None => 0,
                Some(kc::FaultKind::Exit { code }) => code as u8,
                Some(_) => 99,
            },
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FaultFp {
    tag: u8,
    a: u64,
    b: u64,
}

fn ff_sim(k: ks::FaultKind) -> FaultFp {
    match k {
        ks::FaultKind::Exit { code } => FaultFp {
            tag: 1,
            a: code,
            b: 0,
        },
        ks::FaultKind::Vm { va, .. } => FaultFp {
            tag: 2,
            a: va,
            b: 0,
        },
        ks::FaultKind::UnknownSyscall { nr } => FaultFp {
            tag: 3,
            a: nr,
            b: 0,
        },
        ks::FaultKind::UserException { ip, kind } => FaultFp {
            tag: 4,
            a: ip,
            b: kind,
        },
    }
}

fn ff_core(k: kc::FaultKind) -> FaultFp {
    match k {
        kc::FaultKind::Exit { code } => FaultFp {
            tag: 1,
            a: code,
            b: 0,
        },
        kc::FaultKind::Vm { va, .. } => FaultFp {
            tag: 2,
            a: va,
            b: 0,
        },
        kc::FaultKind::UnknownSyscall { nr } => FaultFp {
            tag: 3,
            a: nr,
            b: 0,
        },
        kc::FaultKind::UserException { ip, kind } => FaultFp {
            tag: 4,
            a: ip,
            b: kind,
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MsgFp {
    info: u64,
    badge: u64,
    msg_id: u64,
    len: u8,
    mr: [u64; 64],
}

fn msg_sim(m: &vs::ipc::Message) -> MsgFp {
    MsgFp {
        info: m.info.pack(),
        badge: m.badge,
        msg_id: m.msg_id,
        len: m.info.length(),
        mr: m.mr,
    }
}

fn msg_core(m: &kc::Message) -> MsgFp {
    MsgFp {
        info: m.info.pack(),
        badge: m.badge,
        msg_id: m.msg_id,
        len: m.info.length(),
        mr: m.mr,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct QFp {
    sender: u32,
    call: bool,
    blocking: bool,
    fault: Option<FaultFp>,
    msg: Option<MsgFp>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct IpcFp {
    info: Option<u64>,
    regs: [u64; 4],
    buffer: [u64; 60],
    badge: u64,
    msg_id: Option<u64>,
    last_err: Option<u16>,
    last_fault: Option<(u32, FaultFp)>,
    notif: Option<(u64, u64)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EpFp {
    capacity: usize,
    len: usize,
    pushed: u64,
    popped: u64,
    removed: u64,
    recv_waiter: Option<u32>,
    last_mr: Option<u64>,
    queued: Vec<QFp>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NtFp {
    word: u64,
    badge: u64,
    waiter: Option<u32>,
    bound_sc: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CtFp {
    sent: u64,
    delivered: u64,
    dropped: u64,
    transfers: u64,
    pending: u64,
}

/// Ảnh chụp một ô receive window (KT-63): quyền **đã lọc** + badge của cap đang nằm trong ô.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CapFp {
    ty: u8,
    rights: u8,
    badge: Option<u64>,
}

/// Ảnh chụp receive window của TCB (KT-63): `slots = None` ⇔ chưa bind; `caps[i]` = cap trong
/// ô thứ `i` (None ⇔ ô rỗng).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WinFp {
    slots: Option<[(u32, usize); 4]>,
    caps: [Option<CapFp>; 4],
}

fn ty_tag_sim(t: ks::ObjType) -> u8 {
    match t {
        ks::ObjType::Untyped => 0,
        ks::ObjType::CNode => 1,
        ks::ObjType::Tcb => 2,
        ks::ObjType::SchedContext => 3,
        ks::ObjType::Endpoint => 4,
        ks::ObjType::Notification => 5,
        ks::ObjType::Reply => 6,
        ks::ObjType::SchedControl => 7,
        ks::ObjType::VSpace => 8,
        ks::ObjType::PageTable => 9,
        ks::ObjType::Frame => 10,
        ks::ObjType::IrqControl => 11,
        ks::ObjType::IrqHandler => 12,
        ks::ObjType::IOSpace => 13,
        ks::ObjType::VCpu => 14,
        ks::ObjType::PowerControl => 15,
    }
}

fn ty_tag_core(t: kc::ObjType) -> u8 {
    match t {
        kc::ObjType::Untyped => 0,
        kc::ObjType::CNode => 1,
        kc::ObjType::Tcb => 2,
        kc::ObjType::SchedContext => 3,
        kc::ObjType::Endpoint => 4,
        kc::ObjType::Notification => 5,
        kc::ObjType::Reply => 6,
        kc::ObjType::SchedControl => 7,
        kc::ObjType::VSpace => 8,
        kc::ObjType::PageTable => 9,
        kc::ObjType::Frame => 10,
        kc::ObjType::IrqControl => 11,
        kc::ObjType::IrqHandler => 12,
        kc::ObjType::IOSpace => 13,
        kc::ObjType::VCpu => 14,
        kc::ObjType::PowerControl => 15,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ChkFp {
    ipc: bool,
    reply: bool,
    ntfn: bool,
    waitaddr: bool,
    fault: bool,
    sched: bool,
    caps: bool,
    bytes: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MFp {
    tag: u8,
    a: u32,
    b: u16,
    c: u64,
    d: u64,
}

fn mf_sim(m: vs::mailbox::MailboxMsg) -> MFp {
    match m {
        vs::mailbox::MailboxMsg::Wake(t) => MFp {
            tag: 1,
            a: t,
            b: 0,
            c: 0,
            d: 0,
        },
        vs::mailbox::MailboxMsg::Reschedule => MFp {
            tag: 2,
            a: 0,
            b: 0,
            c: 0,
            d: 0,
        },
        vs::mailbox::MailboxMsg::Migrate { tcb, to_hart } => MFp {
            tag: 3,
            a: tcb,
            b: to_hart,
            c: 0,
            d: 0,
        },
        vs::mailbox::MailboxMsg::Stop => MFp {
            tag: 4,
            a: 0,
            b: 0,
            c: 0,
            d: 0,
        },
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
        kc::MailboxMsg::Wake(t) => MFp {
            tag: 1,
            a: t,
            b: 0,
            c: 0,
            d: 0,
        },
        kc::MailboxMsg::Reschedule => MFp {
            tag: 2,
            a: 0,
            b: 0,
            c: 0,
            d: 0,
        },
        kc::MailboxMsg::Migrate { tcb, to_hart } => MFp {
            tag: 3,
            a: tcb,
            b: to_hart,
            c: 0,
            d: 0,
        },
        kc::MailboxMsg::Stop => MFp {
            tag: 4,
            a: 0,
            b: 0,
            c: 0,
            d: 0,
        },
        kc::MailboxMsg::TlbFlush(f) => MFp {
            tag: 5,
            a: 0,
            b: f.asid,
            c: f.seq,
            d: f.ack,
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MailFp {
    len: usize,
    items: Vec<MFp>,
    log: Vec<MFp>,
    stopped: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Digest {
    states: Vec<Result<CS, E>>,
    infos: Vec<Result<InfoFp, E>>,
    reads: Vec<Result<SnapFp, E>>,
    ipcs: Vec<Result<IpcFp, E>>,
    eps: Vec<Result<EpFp, E>>,
    replies: Vec<Result<Option<u32>, E>>,
    ntfns: Vec<Result<NtFp, E>>,
    wa: Vec<Result<(Option<u32>, u64), E>>,
    wa_stats: (usize, usize, usize),
    counters: CtFp,
    mail: Vec<Result<MailFp, E>>,
    checks: ChkFp,
    finalise: Vec<(u32, u8, u32)>,
    now: u64,
    /// KT-63: receive window của 3 TCB của rig.
    windows: Vec<Result<WinFp, E>>,
    /// KT-63: `(cài được, unwrap, thất bại mềm)`.
    cap_stats: (u64, u64, u64),
}

// ---------------------------------------------------------------------------------------------
// Trait: cùng logic test chạy trên hai bản
// ---------------------------------------------------------------------------------------------

trait View {
    fn tcb_state(&self, obj: u32) -> Result<CS, E>;
    fn tcb_info(&self, slot: (u32, usize)) -> Result<InfoFp, E>;
    fn tcb_read(&self, slot: (u32, usize)) -> Result<SnapFp, E>;
    fn ipc_area(&self, obj: u32) -> Result<IpcFp, E>;
    fn endpoint(&self, obj: u32) -> Result<EpFp, E>;
    fn reply_caller_fp(&self, obj: u32) -> Result<Option<u32>, E>;
    fn notification(&self, obj: u32) -> Result<NtFp, E>;
    fn waitaddr(&self, frame: u32, offset: u32) -> Result<(Option<u32>, u64), E>;
    fn waitaddr_stats(&self) -> (usize, usize, usize);
    fn counters(&self) -> CtFp;
    fn mail(&self, hart: u16) -> Result<MailFp, E>;
    fn checks(&self) -> ChkFp;
    fn finalise_log(&self) -> Vec<(u32, u8, u32)>;
    fn now(&self) -> u64;
    /// KT-63 — receive window của TCB + cap trong từng ô.
    fn window(&self, tcb: u32) -> Result<WinFp, E>;
    /// KT-63 — `(cài được, unwrap, thất bại mềm)`.
    fn cap_stats(&self) -> (u64, u64, u64);
    /// **KT-64a** — `Tcb.last_ipc_error` (mã ABI) của TCB: `Cancelled` sau khi object bị finalise
    /// (D2-89/D2-90) là **quan sát được** nên phải nằm trong digest parity.
    fn last_ipc_error(&self, tcb: u32) -> Result<Option<u16>, E>;
}

trait Mut: View {
    fn add_untyped(&mut self, base: u64, size: usize) -> u32;
    fn add_cnode(&mut self, slots: usize) -> u32;
    fn root_cap(&mut self, obj: u32) -> u32;
    fn put_cap(&mut self, cnode: u32, slot: usize, cap: u32) -> Res;
    fn retype(&mut self, src: (u32, usize), ty: Ty, bits: u32) -> Res;
    fn mint(&mut self, src: (u32, usize), dst: (u32, usize), badge: u64) -> Res;
    fn set_fault_ep(&mut self, tcb: (u32, usize), ep: (u32, usize)) -> Res;
    fn set_priority(&mut self, tcb: (u32, usize), p: u8, mcp: u8) -> Res;
    fn set_sched(&mut self, tcb: (u32, usize), sc: Option<(u32, usize)>) -> Res;
    fn sc_configure(
        &mut self,
        ctl: (u32, usize),
        sc: (u32, usize),
        budget: u64,
        period: u64,
    ) -> Res;
    fn bind_ntfn(&mut self, tcb: (u32, usize), ntfn: (u32, usize)) -> Res;
    fn unbind_ntfn(&mut self, tcb: (u32, usize)) -> Res;
    fn step(&mut self, op: &Op) -> Res;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ty {
    Tcb,
    SchedContext,
    Endpoint,
    Notification,
    Reply,
    SchedControl,
}

impl Ty {
    fn sim(self) -> ks::ObjType {
        match self {
            Ty::Tcb => ks::ObjType::Tcb,
            Ty::SchedContext => ks::ObjType::SchedContext,
            Ty::Endpoint => ks::ObjType::Endpoint,
            Ty::Notification => ks::ObjType::Notification,
            Ty::Reply => ks::ObjType::Reply,
            Ty::SchedControl => ks::ObjType::SchedControl,
        }
    }

    fn core(self) -> kc::ObjType {
        match self {
            Ty::Tcb => kc::ObjType::Tcb,
            Ty::SchedContext => kc::ObjType::SchedContext,
            Ty::Endpoint => kc::ObjType::Endpoint,
            Ty::Notification => kc::ObjType::Notification,
            Ty::Reply => kc::ObjType::Reply,
            Ty::SchedControl => kc::ObjType::SchedControl,
        }
    }
}

/// Một bước kịch bản — Op đủ tham số cho cả hai bản.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
enum Op {
    SetPriority {
        tcb: (u32, usize),
        p: u8,
        mcp: u8,
    },
    SetSched {
        tcb: (u32, usize),
        sc: Option<(u32, usize)>,
    },
    ScConfigure {
        ctl: (u32, usize),
        sc: (u32, usize),
        budget: u64,
        period: u64,
    },
    BindNtfn {
        tcb: (u32, usize),
        ntfn: (u32, usize),
    },
    UnbindNtfn {
        tcb: (u32, usize),
    },
    Call {
        tcb: u32,
        ep: (u32, usize),
        label: u32,
        len: u8,
    },
    Send {
        tcb: u32,
        cap: (u32, usize),
        label: u32,
        len: u8,
    },
    NbSend {
        tcb: u32,
        cap: (u32, usize),
        label: u32,
        len: u8,
    },
    Recv {
        tcb: u32,
        ep: (u32, usize),
        reply: Option<(u32, usize)>,
    },
    NbRecv {
        tcb: u32,
        ep: (u32, usize),
    },
    Reply {
        tcb: u32,
        reply: (u32, usize),
    },
    ReplyMsg {
        tcb: u32,
        reply: (u32, usize),
        label: u32,
        len: u8,
    },
    ReplyRecv {
        tcb: u32,
        reply: (u32, usize),
        ep: (u32, usize),
        recv_reply: Option<(u32, usize)>,
        label: u32,
        len: u8,
    },
    NbSendRecv {
        tcb: u32,
        send_ep: (u32, usize),
        recv_ep: (u32, usize),
        recv_reply: Option<(u32, usize)>,
        label: u32,
        len: u8,
    },
    NbSendWait {
        tcb: u32,
        send_ep: (u32, usize),
        ntfn: (u32, usize),
        label: u32,
        len: u8,
    },
    Exit {
        tcb: u32,
        code: u64,
    },
    InjectFault {
        tcb: u32,
        kind: u8,
        val: u64,
    },
    Transition {
        tcb: u32,
        to: CS,
    },
    BlockCurrent {
        hart: u16,
        to: CS,
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
    WaitAddr {
        tcb: (u32, usize),
        frame: u32,
        offset: u32,
        expected: u64,
    },
    SetAddr {
        frame: u32,
        offset: u32,
        value: u64,
    },
    WakeAddr {
        frame: u32,
        offset: u32,
        n: u32,
    },
    Tick {
        n: u64,
    },
    Dispatch {
        hart: u16,
    },
    Wake {
        tcb: u32,
    },
    Mail {
        hart: u16,
        msg: u8,
    },
    ProcessMail {
        hart: u16,
    },
    SendMail {
        hart: u16,
        msg: u8,
    },
}

fn make_msg_sim(label: u32, len: u8) -> vs::ipc::Message {
    let info = vs::ipc::MessageInfo::new(label, 0, false, 0, len).expect("info hợp lệ");
    let words: Vec<u64> = (0..len).map(|i| (label as u64) * 100 + i as u64).collect();
    vs::ipc::Message::new(info, &words).expect("message hợp lệ")
}

fn make_msg_core(label: u32, len: u8) -> kc::Message {
    let info = kc::MessageInfo::new(label, 0, false, 0, len).expect("info hợp lệ");
    let words: Vec<u64> = (0..len).map(|i| (label as u64) * 100 + i as u64).collect();
    kc::Message::new(info, &words).expect("message hợp lệ")
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

fn num_sim(r: Result<usize, ks::KError>) -> Res {
    r.map(|n| n as u64).map_err(se)
}

fn num_core(r: Result<usize, kc::KError>) -> Res {
    r.map(|n| n as u64).map_err(ce)
}

fn sim_mail_msg(m: u8) -> vs::mailbox::MailboxMsg {
    match m {
        0 => vs::mailbox::MailboxMsg::Reschedule,
        1 => vs::mailbox::MailboxMsg::Stop,
        2 => vs::mailbox::MailboxMsg::Wake(7),
        _ => vs::mailbox::MailboxMsg::Migrate { tcb: 9, to_hart: 0 },
    }
}

fn core_mail_msg(m: u8) -> kc::MailboxMsg {
    match m {
        0 => kc::MailboxMsg::Reschedule,
        1 => kc::MailboxMsg::Stop,
        2 => kc::MailboxMsg::Wake(7),
        _ => kc::MailboxMsg::Migrate { tcb: 9, to_hart: 0 },
    }
}

// ---------------------------------------------------------------------------------------------
// View/Mut cho sim
// ---------------------------------------------------------------------------------------------

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
            info: self.tcb_ipc_info(obj).map_err(se)?.map(|i| i.pack()),
            regs: self.tcb_regs(obj).map_err(se)?,
            buffer: self.tcb_ipc_buffer(obj).map_err(se)?,
            badge: self.tcb_ipc_badge(obj).map_err(se)?,
            msg_id: self.tcb_ipc_msg_id(obj).map_err(se)?,
            last_err: self.tcb_last_ipc_error(obj).map_err(se)?.map(|e| e.id()),
            last_fault: self
                .tcb_last_fault(obj)
                .map_err(se)?
                .map(|(s, k)| (s, ff_sim(k))),
            notif: self
                .tcb_notification(obj)
                .map_err(se)?
                .map(|v| (v.word, v.badge)),
        })
    }
    fn endpoint(&self, obj: u32) -> Result<EpFp, E> {
        let i = self.endpoint_info(obj).map_err(se)?;
        let queued = self
            .endpoint_queued(obj)
            .map_err(se)?
            .into_iter()
            .map(|q| QFp {
                sender: q.sender,
                call: q.call,
                blocking: q.blocking,
                fault: q.fault.map(ff_sim),
                msg: q.msg.as_ref().map(msg_sim),
            })
            .collect();
        Ok(EpFp {
            capacity: i.capacity,
            len: i.len,
            pushed: i.pushed,
            popped: i.popped,
            removed: i.removed,
            recv_waiter: i.recv_waiter,
            last_mr: self.endpoint_last_mr(obj).map_err(se)?,
            queued,
        })
    }
    fn reply_caller_fp(&self, obj: u32) -> Result<Option<u32>, E> {
        self.reply_caller(obj).map_err(se)
    }
    fn notification(&self, obj: u32) -> Result<NtFp, E> {
        Ok(NtFp {
            word: self.notification_word(obj).map_err(se)?,
            badge: self.notification_badge(obj).map_err(se)?,
            waiter: self.notification_waiter(obj).map_err(se)?,
            bound_sc: self.notification_bound_sc(obj).map_err(se)?,
        })
    }
    fn waitaddr(&self, frame: u32, offset: u32) -> Result<(Option<u32>, u64), E> {
        Ok((
            self.address_waiter(frame, offset),
            self.addr_word(frame, offset),
        ))
    }
    fn waitaddr_stats(&self) -> (usize, usize, usize) {
        (
            self.address_waiters_len(),
            self.waitaddr_nonempty_buckets(),
            self.waitaddr_bucket_len(vs::kobj::waitaddr_bucket(3, 0)),
        )
    }
    fn counters(&self) -> CtFp {
        CtFp {
            sent: self.ipc_sent(),
            delivered: self.ipc_delivered(),
            dropped: self.ipc_dropped(),
            transfers: self.transfer_calls(),
            pending: self.ipc_pending(),
        }
    }
    fn mail(&self, hart: u16) -> Result<MailFp, E> {
        Ok(MailFp {
            len: self.mail_len(hart).map_err(se)?,
            items: self
                .mail_items(hart)
                .map_err(se)?
                .into_iter()
                .map(mf_sim)
                .collect(),
            log: self
                .mail_log(hart)
                .map_err(se)?
                .iter()
                .copied()
                .map(mf_sim)
                .collect(),
            stopped: self.hart_stopped(hart).map_err(se)?,
        })
    }
    fn checks(&self) -> ChkFp {
        ChkFp {
            ipc: self.check_ipc().is_ok(),
            reply: self.check_reply_links().is_ok(),
            ntfn: self.check_notifications().is_ok(),
            waitaddr: self.check_waitaddr().is_ok(),
            fault: self.check_fault().is_ok(),
            sched: self.check_sched().is_ok(),
            caps: self.check_caps().is_ok(),
            bytes: self.check_all_bytes().is_ok(),
        }
    }
    fn finalise_log(&self) -> Vec<(u32, u8, u32)> {
        self.finalise_log()
            .iter()
            .map(|e| {
                (
                    e.tcb,
                    match e.state {
                        ks::TcbState::Dead => 10,
                        _ => 0,
                    },
                    e.released_sched_context.unwrap_or(0),
                )
            })
            .collect()
    }
    fn now(&self) -> u64 {
        self.virtual_now()
    }
    fn window(&self, tcb: u32) -> Result<WinFp, E> {
        let w = self.tcb_recv_window(tcb).map_err(se)?;
        let mut out = WinFp {
            slots: None,
            caps: [None; 4],
        };
        if let Some(ws) = w {
            let mut slots = [(0u32, 0usize); 4];
            for i in 0..4 {
                slots[i] = (ws[i].cnode, ws[i].slot);
                if let Ok(cap) = self.cap_at(ws[i].cnode, ws[i].slot) {
                    let ci = self.cap_info(cap).map_err(se)?;
                    out.caps[i] = Some(CapFp {
                        ty: ty_tag_sim(ci.ty),
                        rights: ci.rights,
                        badge: ci.badge,
                    });
                }
            }
            out.slots = Some(slots);
        }
        Ok(out)
    }
    fn cap_stats(&self) -> (u64, u64, u64) {
        self.cap_transfer_stats()
    }
    fn last_ipc_error(&self, tcb: u32) -> Result<Option<u16>, E> {
        self.tcb_last_ipc_error(tcb)
            .map(|o| o.map(|e| e.id()))
            .map_err(se)
    }
}

impl Mut for ks::Machine {
    fn add_untyped(&mut self, base: u64, size: usize) -> u32 {
        self.add_untyped(base, size)
    }
    fn add_cnode(&mut self, slots: usize) -> u32 {
        self.add_cnode(slots)
    }
    fn root_cap(&mut self, obj: u32) -> u32 {
        self.root_cap(obj)
    }
    fn put_cap(&mut self, cnode: u32, slot: usize, cap: u32) -> Res {
        unit_sim(self.put_cap(cnode, slot, cap))
    }
    fn retype(&mut self, src: (u32, usize), ty: Ty, bits: u32) -> Res {
        self.retype(ks::SlotRef::new(src.0, src.1), ty.sim(), bits, 0)
            .map(|c| c as u64)
            .map_err(se)
    }
    fn mint(&mut self, src: (u32, usize), dst: (u32, usize), badge: u64) -> Res {
        self.mint(
            ks::SlotRef::new(src.0, src.1),
            ks::SlotRef::new(dst.0, dst.1),
            ks::RIGHTS_ALL,
            Some(badge),
        )
        .map(|c| c as u64)
        .map_err(se)
    }
    fn set_fault_ep(&mut self, tcb: (u32, usize), ep: (u32, usize)) -> Res {
        unit_sim(
            self.tcb_set_fault_ep(ks::SlotRef::new(tcb.0, tcb.1), ks::SlotRef::new(ep.0, ep.1)),
        )
    }
    fn set_priority(&mut self, tcb: (u32, usize), p: u8, mcp: u8) -> Res {
        unit_sim(self.tcb_set_priority(ks::SlotRef::new(tcb.0, tcb.1), p, mcp))
    }
    fn set_sched(&mut self, tcb: (u32, usize), sc: Option<(u32, usize)>) -> Res {
        unit_sim(self.set_sched_params(
            ks::SlotRef::new(tcb.0, tcb.1),
            sc.map(|s| ks::SlotRef::new(s.0, s.1)),
        ))
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
    fn bind_ntfn(&mut self, tcb: (u32, usize), ntfn: (u32, usize)) -> Res {
        unit_sim(self.tcb_bind_notification(
            ks::SlotRef::new(tcb.0, tcb.1),
            ks::SlotRef::new(ntfn.0, ntfn.1),
        ))
    }
    fn unbind_ntfn(&mut self, tcb: (u32, usize)) -> Res {
        unit_sim(self.tcb_unbind_notification(ks::SlotRef::new(tcb.0, tcb.1)))
    }
    fn step(&mut self, op: &Op) -> Res {
        match *op {
            Op::SetPriority { tcb, p, mcp } => self.set_priority(tcb, p, mcp),
            Op::SetSched { tcb, sc } => self.set_sched(tcb, sc),
            Op::ScConfigure {
                ctl,
                sc,
                budget,
                period,
            } => self.sc_configure(ctl, sc, budget, period),
            Op::BindNtfn { tcb, ntfn } => self.bind_ntfn(tcb, ntfn),
            Op::UnbindNtfn { tcb } => self.unbind_ntfn(tcb),
            Op::Call {
                tcb,
                ep,
                label,
                len,
            } => {
                unit_sim(self.call_msg(tcb, ks::SlotRef::new(ep.0, ep.1), make_msg_sim(label, len)))
            }
            Op::Send {
                tcb,
                cap,
                label,
                len,
            } => unit_sim(self.send(
                tcb,
                ks::SlotRef::new(cap.0, cap.1),
                make_msg_sim(label, len),
            )),
            Op::NbSend {
                tcb,
                cap,
                label,
                len,
            } => unit_sim(self.nb_send(
                tcb,
                ks::SlotRef::new(cap.0, cap.1),
                make_msg_sim(label, len),
            )),
            Op::Recv { tcb, ep, reply } => unit_sim(self.recv(
                tcb,
                ks::SlotRef::new(ep.0, ep.1),
                reply.map(|r| ks::SlotRef::new(r.0, r.1)),
            )),
            Op::NbRecv { tcb, ep } => {
                let r = self.nb_recv(tcb, ks::SlotRef::new(ep.0, ep.1));
                match r {
                    Ok(m) => Ok(m.map_or(0, |m| m.msg_id)),
                    Err(e) => Err(se(e)),
                }
            }
            Op::Reply { tcb, reply } => {
                unit_sim(self.reply(tcb, ks::SlotRef::new(reply.0, reply.1)))
            }
            Op::ReplyMsg {
                tcb,
                reply,
                label,
                len,
            } => unit_sim(self.reply_msg(
                tcb,
                ks::SlotRef::new(reply.0, reply.1),
                make_msg_sim(label, len),
            )),
            Op::ReplyRecv {
                tcb,
                reply,
                ep,
                recv_reply,
                label,
                len,
            } => unit_sim(self.reply_recv(
                tcb,
                ks::SlotRef::new(reply.0, reply.1),
                ks::SlotRef::new(ep.0, ep.1),
                make_msg_sim(label, len),
                recv_reply.map(|r| ks::SlotRef::new(r.0, r.1)),
            )),
            Op::NbSendRecv {
                tcb,
                send_ep,
                recv_ep,
                recv_reply,
                label,
                len,
            } => {
                let r = self.nb_send_recv(
                    tcb,
                    ks::SlotRef::new(send_ep.0, send_ep.1),
                    make_msg_sim(label, len),
                    ks::SlotRef::new(recv_ep.0, recv_ep.1),
                    recv_reply.map(|r| ks::SlotRef::new(r.0, r.1)),
                );
                match r {
                    Ok(None) => Ok(0),
                    Ok(Some(e)) => Ok(e.id() as u64),
                    Err(e) => Err(se(e)),
                }
            }
            Op::NbSendWait {
                tcb,
                send_ep,
                ntfn,
                label,
                len,
            } => {
                let r = self.nb_send_wait(
                    tcb,
                    ks::SlotRef::new(send_ep.0, send_ep.1),
                    make_msg_sim(label, len),
                    ks::SlotRef::new(ntfn.0, ntfn.1),
                );
                match r {
                    Ok(None) => Ok(0),
                    Ok(Some(e)) => Ok(e.id() as u64),
                    Err(e) => Err(se(e)),
                }
            }
            Op::Exit { tcb, code } => unit_sim(self.exit(tcb, code)),
            Op::InjectFault { tcb, kind, val } => {
                let k = match kind {
                    1 => ks::FaultKind::UnknownSyscall { nr: val },
                    2 => ks::FaultKind::UserException { ip: val, kind: 3 },
                    _ => ks::FaultKind::Vm {
                        vspace: val as u32,
                        va: val,
                        access: ks::FaultAccess::Read,
                        reason: ks::VmFaultReason::NotMapped,
                    },
                };
                unit_sim(self.inject_fault(tcb, k))
            }
            Op::Transition { tcb, to } => match to.sim_nonready() {
                Some(target) => unit_sim(self.transition_tcb(tcb, target)),
                None => sim_to_ready(self, tcb),
            },
            Op::BlockCurrent { hart, to } => match to.sim_nonready() {
                Some(target) => match self.block_current(hart, target) {
                    Ok(o) => Ok(o as u64),
                    Err(e) => Err(se(e)),
                },
                // `Ready` không phải đích block hợp lệ (D2-194 §1(d)).
                None => Err(se(ks::KError::InvalidArgument)),
            },
            Op::Suspend { tcb, abort } => unit_sim(self.tcb_suspend(
                ks::SlotRef::new(tcb.0, tcb.1),
                if abort {
                    ks::SuspendMode::Abort
                } else {
                    ks::SuspendMode::Defer
                },
            )),
            Op::Resume { tcb } => unit_sim(self.tcb_resume(ks::SlotRef::new(tcb.0, tcb.1))),
            Op::Finalise { tcb } => unit_sim(self.finalise_tcb(tcb)),
            Op::WaitAddr {
                tcb,
                frame,
                offset,
                expected,
            } => unit_sim(self.wait_addr(ks::SlotRef::new(tcb.0, tcb.1), frame, offset, expected)),
            Op::SetAddr {
                frame,
                offset,
                value,
            } => {
                self.set_addr_word(frame, offset, value);
                Ok(0)
            }
            Op::WakeAddr { frame, offset, n } => self
                .wake_addr(frame, offset, n)
                .map(|n| n as u64)
                .map_err(se),
            Op::Tick { n } => match self.tick(n) {
                Ok(()) => Ok(0),
                Err(e) => Err(se(e)),
            },
            Op::Dispatch { hart } => match self.dispatch(hart) {
                Ok(o) => Ok(o.unwrap_or(0) as u64),
                Err(e) => Err(se(e)),
            },
            Op::Wake { tcb } => unit_sim(self.wake(tcb)),
            Op::Mail { hart, msg } => unit_sim(self.post_mail(hart, sim_mail_msg(msg))),
            Op::ProcessMail { hart } => num_sim(self.process_mail(hart)),
            Op::SendMail { hart, msg } => unit_sim(self.send_mail(hart, sim_mail_msg(msg))),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// View/Mut cho core
// ---------------------------------------------------------------------------------------------

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
        let area = self.tcb_ipc_area(obj).map_err(ce)?;
        Ok(IpcFp {
            info: area.info.map(|i| i.pack()),
            regs: area.regs,
            buffer: area.buffer,
            badge: area.badge,
            msg_id: area.msg_id,
            last_err: self.tcb_last_ipc_error(obj).map_err(ce)?.map(|e| e.id()),
            last_fault: self
                .tcb_last_fault(obj)
                .map_err(ce)?
                .map(|(s, k)| (s, ff_core(k))),
            notif: self
                .tcb_notification(obj)
                .map_err(ce)?
                .map(|v| (v.word, v.badge)),
        })
    }
    fn endpoint(&self, obj: u32) -> Result<EpFp, E> {
        let i = self.endpoint_info(obj).map_err(ce)?;
        let queued = self
            .endpoint_queued(obj)
            .map_err(ce)?
            .map(|q| QFp {
                sender: q.sender,
                call: q.call,
                blocking: q.blocking,
                fault: q.fault.map(ff_core),
                msg: q.msg.as_ref().map(msg_core),
            })
            .collect();
        Ok(EpFp {
            capacity: i.capacity,
            len: i.len,
            pushed: i.pushed,
            popped: i.popped,
            removed: i.removed,
            recv_waiter: i.recv_waiter,
            last_mr: self.endpoint_last_mr(obj).map_err(ce)?,
            queued,
        })
    }
    fn reply_caller_fp(&self, obj: u32) -> Result<Option<u32>, E> {
        self.reply_caller(obj).map_err(ce)
    }
    fn notification(&self, obj: u32) -> Result<NtFp, E> {
        Ok(NtFp {
            word: self.notification_word(obj).map_err(ce)?,
            badge: self.notification_badge(obj).map_err(ce)?,
            waiter: self.notification_waiter(obj).map_err(ce)?,
            bound_sc: self.notification_bound_sc(obj).map_err(ce)?,
        })
    }
    fn waitaddr(&self, frame: u32, offset: u32) -> Result<(Option<u32>, u64), E> {
        Ok((
            self.address_waiter(frame, offset),
            self.addr_word(frame, offset),
        ))
    }
    fn waitaddr_stats(&self) -> (usize, usize, usize) {
        (
            self.address_waiters_len(),
            self.waitaddr_nonempty_buckets(),
            self.waitaddr_bucket_len(kc::waitaddr_bucket(3, 0)),
        )
    }
    fn counters(&self) -> CtFp {
        CtFp {
            sent: self.ipc_sent(),
            delivered: self.ipc_delivered(),
            dropped: self.ipc_dropped(),
            transfers: self.transfer_calls(),
            pending: self.ipc_pending(),
        }
    }
    fn mail(&self, hart: u16) -> Result<MailFp, E> {
        Ok(MailFp {
            len: self.mail_len(hart).map_err(ce)?,
            items: self.mail_items(hart).map_err(ce)?.map(mf_core).collect(),
            log: self
                .mail_log(hart)
                .map_err(ce)?
                .iter()
                .copied()
                .map(mf_core)
                .collect(),
            stopped: self.hart_stopped(hart).map_err(ce)?,
        })
    }
    fn checks(&self) -> ChkFp {
        ChkFp {
            ipc: self.check_ipc().is_ok(),
            reply: self.check_reply_links().is_ok(),
            ntfn: self.check_notifications().is_ok(),
            waitaddr: self.check_waitaddr().is_ok(),
            fault: self.check_fault().is_ok(),
            sched: self.check_sched().is_ok(),
            caps: self.check_caps().is_ok(),
            bytes: self.check_all_bytes().is_ok(),
        }
    }
    fn finalise_log(&self) -> Vec<(u32, u8, u32)> {
        self.finalise_log()
            .iter()
            .map(|e| {
                (
                    e.tcb,
                    match e.state {
                        kc::TcbState::Dead => 10,
                        _ => 0,
                    },
                    e.released_sched_context.unwrap_or(0),
                )
            })
            .collect()
    }
    fn now(&self) -> u64 {
        self.virtual_now()
    }
    fn window(&self, tcb: u32) -> Result<WinFp, E> {
        let w = self.tcb_recv_window(tcb).map_err(ce)?;
        let mut out = WinFp {
            slots: None,
            caps: [None; 4],
        };
        if let Some(ws) = w {
            let mut slots = [(0u32, 0usize); 4];
            for i in 0..4 {
                slots[i] = (ws[i].cnode, ws[i].slot);
                if let Ok(cap) = self.cap_at(ws[i].cnode, ws[i].slot) {
                    let ci = self.cap_info(cap).map_err(ce)?;
                    out.caps[i] = Some(CapFp {
                        ty: ty_tag_core(ci.ty),
                        rights: ci.rights,
                        badge: ci.badge,
                    });
                }
            }
            out.slots = Some(slots);
        }
        Ok(out)
    }
    fn cap_stats(&self) -> (u64, u64, u64) {
        self.cap_transfer_stats()
    }
    fn last_ipc_error(&self, tcb: u32) -> Result<Option<u16>, E> {
        self.tcb_last_ipc_error(tcb)
            .map(|o| o.map(|e| e.id()))
            .map_err(ce)
    }
}

impl Mut for kc::Machine {
    fn add_untyped(&mut self, base: u64, size: usize) -> u32 {
        self.add_untyped(base, size)
    }
    fn add_cnode(&mut self, slots: usize) -> u32 {
        self.add_cnode(slots)
    }
    fn root_cap(&mut self, obj: u32) -> u32 {
        self.root_cap(obj)
    }
    fn put_cap(&mut self, cnode: u32, slot: usize, cap: u32) -> Res {
        unit_core(self.put_cap(cnode, slot, cap))
    }
    fn retype(&mut self, src: (u32, usize), ty: Ty, bits: u32) -> Res {
        self.retype(kc::SlotRef::new(src.0, src.1), ty.core(), bits, 0)
            .map(|c| c as u64)
            .map_err(ce)
    }
    fn mint(&mut self, src: (u32, usize), dst: (u32, usize), badge: u64) -> Res {
        self.mint(
            kc::SlotRef::new(src.0, src.1),
            kc::SlotRef::new(dst.0, dst.1),
            kc::RIGHTS_ALL,
            Some(badge),
        )
        .map(|c| c as u64)
        .map_err(ce)
    }
    fn set_fault_ep(&mut self, tcb: (u32, usize), ep: (u32, usize)) -> Res {
        unit_core(
            self.tcb_set_fault_ep(kc::SlotRef::new(tcb.0, tcb.1), kc::SlotRef::new(ep.0, ep.1)),
        )
    }
    fn set_priority(&mut self, tcb: (u32, usize), p: u8, mcp: u8) -> Res {
        unit_core(self.tcb_set_priority(kc::SlotRef::new(tcb.0, tcb.1), p, mcp))
    }
    fn set_sched(&mut self, tcb: (u32, usize), sc: Option<(u32, usize)>) -> Res {
        unit_core(self.set_sched_params(
            kc::SlotRef::new(tcb.0, tcb.1),
            sc.map(|s| kc::SlotRef::new(s.0, s.1)),
        ))
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
    fn bind_ntfn(&mut self, tcb: (u32, usize), ntfn: (u32, usize)) -> Res {
        unit_core(self.tcb_bind_notification(
            kc::SlotRef::new(tcb.0, tcb.1),
            kc::SlotRef::new(ntfn.0, ntfn.1),
        ))
    }
    fn unbind_ntfn(&mut self, tcb: (u32, usize)) -> Res {
        unit_core(self.tcb_unbind_notification(kc::SlotRef::new(tcb.0, tcb.1)))
    }
    fn step(&mut self, op: &Op) -> Res {
        match *op {
            Op::SetPriority { tcb, p, mcp } => self.set_priority(tcb, p, mcp),
            Op::SetSched { tcb, sc } => self.set_sched(tcb, sc),
            Op::ScConfigure {
                ctl,
                sc,
                budget,
                period,
            } => self.sc_configure(ctl, sc, budget, period),
            Op::BindNtfn { tcb, ntfn } => self.bind_ntfn(tcb, ntfn),
            Op::UnbindNtfn { tcb } => self.unbind_ntfn(tcb),
            Op::Call {
                tcb,
                ep,
                label,
                len,
            } => unit_core(self.call_msg(
                tcb,
                kc::SlotRef::new(ep.0, ep.1),
                make_msg_core(label, len),
            )),
            Op::Send {
                tcb,
                cap,
                label,
                len,
            } => unit_core(self.send(
                tcb,
                kc::SlotRef::new(cap.0, cap.1),
                make_msg_core(label, len),
            )),
            Op::NbSend {
                tcb,
                cap,
                label,
                len,
            } => unit_core(self.nb_send(
                tcb,
                kc::SlotRef::new(cap.0, cap.1),
                make_msg_core(label, len),
            )),
            Op::Recv { tcb, ep, reply } => unit_core(self.recv(
                tcb,
                kc::SlotRef::new(ep.0, ep.1),
                reply.map(|r| kc::SlotRef::new(r.0, r.1)),
            )),
            Op::NbRecv { tcb, ep } => {
                let r = self.nb_recv(tcb, kc::SlotRef::new(ep.0, ep.1));
                match r {
                    Ok(m) => Ok(m.map_or(0, |m| m.msg_id)),
                    Err(e) => Err(ce(e)),
                }
            }
            Op::Reply { tcb, reply } => {
                unit_core(self.reply(tcb, kc::SlotRef::new(reply.0, reply.1)))
            }
            Op::ReplyMsg {
                tcb,
                reply,
                label,
                len,
            } => unit_core(self.reply_msg(
                tcb,
                kc::SlotRef::new(reply.0, reply.1),
                make_msg_core(label, len),
            )),
            Op::ReplyRecv {
                tcb,
                reply,
                ep,
                recv_reply,
                label,
                len,
            } => unit_core(self.reply_recv(
                tcb,
                kc::SlotRef::new(reply.0, reply.1),
                kc::SlotRef::new(ep.0, ep.1),
                make_msg_core(label, len),
                recv_reply.map(|r| kc::SlotRef::new(r.0, r.1)),
            )),
            Op::NbSendRecv {
                tcb,
                send_ep,
                recv_ep,
                recv_reply,
                label,
                len,
            } => {
                let r = self.nb_send_recv(
                    tcb,
                    kc::SlotRef::new(send_ep.0, send_ep.1),
                    make_msg_core(label, len),
                    kc::SlotRef::new(recv_ep.0, recv_ep.1),
                    recv_reply.map(|r| kc::SlotRef::new(r.0, r.1)),
                );
                match r {
                    Ok(None) => Ok(0),
                    Ok(Some(e)) => Ok(e.id() as u64),
                    Err(e) => Err(ce(e)),
                }
            }
            Op::NbSendWait {
                tcb,
                send_ep,
                ntfn,
                label,
                len,
            } => {
                let r = self.nb_send_wait(
                    tcb,
                    kc::SlotRef::new(send_ep.0, send_ep.1),
                    make_msg_core(label, len),
                    kc::SlotRef::new(ntfn.0, ntfn.1),
                );
                match r {
                    Ok(None) => Ok(0),
                    Ok(Some(e)) => Ok(e.id() as u64),
                    Err(e) => Err(ce(e)),
                }
            }
            Op::Exit { tcb, code } => unit_core(self.exit(tcb, code)),
            Op::InjectFault { tcb, kind, val } => {
                let k = match kind {
                    1 => kc::FaultKind::UnknownSyscall { nr: val },
                    2 => kc::FaultKind::UserException { ip: val, kind: 3 },
                    _ => kc::FaultKind::Vm {
                        vspace: val as u32,
                        va: val,
                        access: kc::FaultAccess::Read,
                        reason: kc::VmFaultReason::NotMapped,
                    },
                };
                unit_core(self.inject_fault(tcb, k))
            }
            Op::Transition { tcb, to } => match to.core_nonready() {
                Some(target) => unit_core(self.transition_tcb(tcb, target)),
                None => core_to_ready(self, tcb),
            },
            Op::BlockCurrent { hart, to } => match to.core_nonready() {
                Some(target) => match self.block_current(hart, target) {
                    Ok(o) => Ok(o as u64),
                    Err(e) => Err(ce(e)),
                },
                None => Err(ce(kc::KError::InvalidArgument)),
            },
            Op::Suspend { tcb, abort } => unit_core(self.tcb_suspend(
                kc::SlotRef::new(tcb.0, tcb.1),
                if abort {
                    kc::SuspendMode::Abort
                } else {
                    kc::SuspendMode::Defer
                },
            )),
            Op::Resume { tcb } => unit_core(self.tcb_resume(kc::SlotRef::new(tcb.0, tcb.1))),
            Op::Finalise { tcb } => unit_core(self.finalise_tcb(tcb)),
            Op::WaitAddr {
                tcb,
                frame,
                offset,
                expected,
            } => unit_core(self.wait_addr(kc::SlotRef::new(tcb.0, tcb.1), frame, offset, expected)),
            Op::SetAddr {
                frame,
                offset,
                value,
            } => unit_core(self.set_addr_word(frame, offset, value)),
            Op::WakeAddr { frame, offset, n } => self
                .wake_addr(frame, offset, n)
                .map(|n| n as u64)
                .map_err(ce),
            Op::Tick { n } => unit_core(self.tick(n)),
            Op::Dispatch { hart } => match self.dispatch(hart) {
                Ok(o) => Ok(o.unwrap_or(0) as u64),
                Err(e) => Err(ce(e)),
            },
            Op::Wake { tcb } => unit_core(self.wake(tcb)),
            Op::Mail { hart, msg } => unit_core(self.post_mail(hart, core_mail_msg(msg))),
            Op::ProcessMail { hart } => num_core(self.process_mail(hart)),
            Op::SendMail { hart, msg } => unit_core(self.send_mail(hart, core_mail_msg(msg))),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Rig + driver
// ---------------------------------------------------------------------------------------------

const ROOT_SLOTS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Rig {
    root: u32,
    t: [u32; 3],
    ts: [(u32, usize); 3],
    ep: [u32; 2],
    eps: [(u32, usize); 2],
    r: [u32; 4],
    rs: [(u32, usize); 4],
    n: [u32; 2],
    ns: [(u32, usize); 2],
    ctl: u32,
    ctls: (u32, usize),
    sc: u32,
    scs: (u32, usize),
    b10: (u32, usize),
    b20: (u32, usize),
    b08: (u32, usize),
    b02: (u32, usize),
}

fn build<M: Mut>(m: &mut M) -> Rig {
    let u = m.add_untyped(0x1000_0000, 1 << 20);
    let root = m.add_cnode(ROOT_SLOTS);
    let uc = m.root_cap(u);
    assert_eq!(m.put_cap(root, 0, uc), Ok(0), "đặt cap Untyped");
    let tys = [
        Ty::Tcb,
        Ty::Tcb,
        Ty::Tcb,
        Ty::Endpoint,
        Ty::Endpoint,
        Ty::Reply,
        Ty::Reply,
        Ty::Reply,
        Ty::Reply,
        Ty::Notification,
        Ty::Notification,
        Ty::SchedControl,
        Ty::SchedContext,
    ];
    for (i, ty) in tys.iter().enumerate() {
        let cap = m.retype((root, 0), *ty, 0).expect("retype") as u32;
        assert_eq!(m.put_cap(root, 1 + i, cap), Ok(0), "đặt cap object {i}");
    }
    let rig = Rig {
        root,
        t: [3, 4, 5],
        ts: [(root, 1), (root, 2), (root, 3)],
        ep: [6, 7],
        eps: [(root, 4), (root, 5)],
        r: [8, 9, 10, 11],
        rs: [(root, 6), (root, 7), (root, 8), (root, 9)],
        n: [12, 13],
        ns: [(root, 10), (root, 11)],
        ctl: 14,
        ctls: (root, 12),
        sc: 15,
        scs: (root, 13),
        b10: (root, 14),
        b20: (root, 15),
        b08: (root, 16),
        b02: (root, 17),
    };
    assert_eq!(m.set_priority(rig.ts[0], 3, 10), Ok(0));
    assert_eq!(m.set_priority(rig.ts[1], 1, 10), Ok(0));
    assert_eq!(m.set_priority(rig.ts[2], 2, 10), Ok(0));
    assert_eq!(m.sc_configure(rig.ctls, rig.scs, 4, 8), Ok(0));
    assert_eq!(m.set_sched(rig.ts[0], Some(rig.scs)), Ok(0));
    assert_eq!(m.set_fault_ep(rig.ts[0], rig.eps[0]), Ok(0));
    assert_eq!(m.set_fault_ep(rig.ts[2], rig.eps[0]), Ok(0));
    assert_eq!(m.bind_ntfn(rig.ts[1], rig.ns[0]), Ok(0));
    assert!(m.mint(rig.ns[0], rig.b10, 0x10).is_ok());
    assert!(m.mint(rig.ns[0], rig.b20, 0x20).is_ok());
    assert!(m.mint(rig.ns[1], rig.b08, 0x08).is_ok());
    assert!(m.mint(rig.ns[0], rig.b02, 0x02).is_ok());
    rig
}

const WA_KEYS: [(u32, u32); 4] = [(3, 0), (3, 64), (4, 0), (0, 0)];

fn digest(v: &dyn View, rig: &Rig) -> Digest {
    let mut states = Vec::new();
    let mut infos = Vec::new();
    let mut reads = Vec::new();
    let mut ipcs = Vec::new();
    for (i, &t) in rig.t.iter().take(3).enumerate() {
        states.push(v.tcb_state(t));
        infos.push(v.tcb_info(rig.ts[i]));
        reads.push(v.tcb_read(rig.ts[i]));
        ipcs.push(v.ipc_area(t));
    }
    let mut eps = Vec::new();
    for &ep in rig.ep.iter().take(2) {
        eps.push(v.endpoint(ep));
    }
    let mut replies = Vec::new();
    for &r in rig.r.iter().take(4) {
        replies.push(v.reply_caller_fp(r));
    }
    let mut ntfns = Vec::new();
    for &n in rig.n.iter().take(2) {
        ntfns.push(v.notification(n));
    }
    let mut wa = Vec::new();
    for &(f, o) in &WA_KEYS {
        wa.push(v.waitaddr(f, o));
    }
    let mut mail = Vec::new();
    for h in 0..1u16 {
        mail.push(v.mail(h));
    }
    let mut windows = Vec::new();
    for &t in rig.t.iter().take(3) {
        windows.push(v.window(t));
    }
    Digest {
        states,
        infos,
        reads,
        ipcs,
        eps,
        replies,
        ntfns,
        wa,
        wa_stats: v.waitaddr_stats(),
        counters: v.counters(),
        mail,
        checks: v.checks(),
        finalise: v.finalise_log(),
        now: v.now(),
        windows,
        cap_stats: v.cap_stats(),
    }
}

fn fix_op(op: &Op, root: u32) -> Op {
    let r = |s: (u32, usize)| -> (u32, usize) {
        if s.0 == 0 {
            (root, s.1)
        } else {
            s
        }
    };
    let ro = |s: Option<(u32, usize)>| s.map(r);
    let mut o = *op;
    match &mut o {
        Op::SetPriority { tcb, .. } => *tcb = r(*tcb),
        Op::SetSched { tcb, sc } => {
            *tcb = r(*tcb);
            *sc = ro(*sc);
        }
        Op::ScConfigure { ctl, sc, .. } => {
            *ctl = r(*ctl);
            *sc = r(*sc);
        }
        Op::BindNtfn { tcb, ntfn } => {
            *tcb = r(*tcb);
            *ntfn = r(*ntfn);
        }
        Op::UnbindNtfn { tcb } => *tcb = r(*tcb),
        Op::Call { ep, .. } => *ep = r(*ep),
        Op::Send { cap, .. } | Op::NbSend { cap, .. } => *cap = r(*cap),
        Op::Recv { ep, reply, .. } => {
            *ep = r(*ep);
            *reply = ro(*reply);
        }
        Op::NbRecv { ep, .. } => *ep = r(*ep),
        Op::Reply { reply, .. } | Op::ReplyMsg { reply, .. } => *reply = r(*reply),
        Op::ReplyRecv {
            reply,
            ep,
            recv_reply,
            ..
        } => {
            *reply = r(*reply);
            *ep = r(*ep);
            *recv_reply = ro(*recv_reply);
        }
        Op::NbSendRecv {
            send_ep,
            recv_ep,
            recv_reply,
            ..
        } => {
            *send_ep = r(*send_ep);
            *recv_ep = r(*recv_ep);
            *recv_reply = ro(*recv_reply);
        }
        Op::NbSendWait { send_ep, ntfn, .. } => {
            *send_ep = r(*send_ep);
            *ntfn = r(*ntfn);
        }
        Op::Suspend { tcb, .. } | Op::Resume { tcb } | Op::WaitAddr { tcb, .. } => *tcb = r(*tcb),
        _ => {}
    }
    o
}

fn run(name: &str, ops: &[(&'static str, Op)]) {
    let mut sm = ks::Machine::new();
    let mut cm = kc::Machine::new();
    let rig_s = build(&mut sm);
    let rig_c = build(&mut cm);
    assert_eq!(rig_s, rig_c, "{name}: rig hai bên lệch");
    for (i, (label, op)) in ops.iter().enumerate() {
        let op = &fix_op(op, rig_s.root);
        let a = sm.step(op);
        let b = cm.step(op);
        assert_eq!(a, b, "{name} buoc {i} `{label}`: ket qua lech");
        let da = digest(&sm, &rig_s);
        let db = digest(&cm, &rig_c);
        assert_eq!(da, db, "{name} buoc {i} `{label}`: digest lech");
        assert!(da.checks.ipc, "{name} buoc {i}: sim check_ipc do");
        assert!(da.checks.reply, "{name} buoc {i}: sim check_reply do");
        assert!(
            da.checks.ntfn,
            "{name} buoc {i}: sim check_notifications do"
        );
        assert!(da.checks.waitaddr, "{name} buoc {i}: sim check_waitaddr do");
        assert!(da.checks.fault, "{name} buoc {i}: sim check_fault do");
    }
}

// ---------------------------------------------------------------------------------------------
// KT-24 — Endpoint queue FIFO, Busy, NBSend/Recv
// ---------------------------------------------------------------------------------------------

#[test]
fn kt60_parity_ipc24_hang_fifo_va_busy() {
    let s = |label: &'static str, op: Op| (label, op);
    let mut ops = vec![
        s(
            "send t0",
            Op::Send {
                tcb: 3,
                cap: (0, 4),
                label: 1,
                len: 3,
            },
        ),
        s(
            "send t1",
            Op::Send {
                tcb: 4,
                cap: (0, 4),
                label: 2,
                len: 2,
            },
        ),
        s(
            "nbsend t2",
            Op::NbSend {
                tcb: 5,
                cap: (0, 4),
                label: 3,
                len: 1,
            },
        ),
        s(
            "recv t2",
            Op::Recv {
                tcb: 5,
                ep: (0, 4),
                reply: None,
            },
        ),
        s(
            "recv t1 dang chan",
            Op::Recv {
                tcb: 4,
                ep: (0, 4),
                reply: None,
            },
        ),
        s("wake t1", Op::Wake { tcb: 4 }),
        s(
            "recv t1",
            Op::Recv {
                tcb: 4,
                ep: (0, 4),
                reply: None,
            },
        ),
        s("nbrecv t0", Op::NbRecv { tcb: 3, ep: (0, 4) }),
        s("nbrecv t0 rong", Op::NbRecv { tcb: 3, ep: (0, 4) }),
    ];
    // Hàng 8: nb_send lấp đầy, cái thứ 9 ⇒ Busy.
    for i in 0..8u32 {
        ops.push(s(
            "nbsend lap hang",
            Op::NbSend {
                tcb: 3,
                cap: (0, 4),
                label: 10 + i,
                len: 1,
            },
        ));
    }
    ops.push(s(
        "nbsend thu 9",
        Op::NbSend {
            tcb: 3,
            cap: (0, 4),
            label: 99,
            len: 1,
        },
    ));
    for _ in 0..8 {
        ops.push(s("nbrecv t1", Op::NbRecv { tcb: 4, ep: (0, 4) }));
    }
    ops.push(s("nbrecv t1 rong", Op::NbRecv { tcb: 4, ep: (0, 4) }));
    ops.push(s(
        "send chan",
        Op::Send {
            tcb: 3,
            cap: (0, 4),
            label: 20,
            len: 2,
        },
    ));
    ops.push(s(
        "recv t2 giao",
        Op::Recv {
            tcb: 5,
            ep: (0, 4),
            reply: None,
        },
    ));
    ops.push(s(
        "send len TCB",
        Op::Send {
            tcb: 3,
            cap: (0, 1),
            label: 30,
            len: 1,
        },
    ));
    ops.push(s(
        "call len TCB",
        Op::Call {
            tcb: 3,
            ep: (0, 1),
            label: 31,
            len: 1,
        },
    ));
    ops.push(s(
        "send len Reply",
        Op::Send {
            tcb: 3,
            cap: (0, 8),
            label: 32,
            len: 1,
        },
    ));
    run("kt24", &ops);
}

// ---------------------------------------------------------------------------------------------
// KT-25 + KT-34 — Call/Reply/ReplyRecv, Reply tái dùng, hai hook transition
// ---------------------------------------------------------------------------------------------

#[test]
fn kt60_parity_ipc25_call_reply_replyrecv_va_kt34() {
    let s = |label: &'static str, op: Op| (label, op);
    let ops = vec![
        s(
            "call t0",
            Op::Call {
                tcb: 3,
                ep: (0, 4),
                label: 1,
                len: 3,
            },
        ),
        s(
            "recv t1 R0",
            Op::Recv {
                tcb: 4,
                ep: (0, 4),
                reply: Some((0, 6)),
            },
        ),
        s(
            "reply t1 R0",
            Op::Reply {
                tcb: 4,
                reply: (0, 6),
            },
        ),
        s(
            "reply t1 R0 lan 2",
            Op::Reply {
                tcb: 4,
                reply: (0, 6),
            },
        ),
        s(
            "recv t1 R0 tai dung",
            Op::Recv {
                tcb: 4,
                ep: (0, 4),
                reply: Some((0, 6)),
            },
        ),
        s(
            "call t0 2",
            Op::Call {
                tcb: 3,
                ep: (0, 4),
                label: 2,
                len: 2,
            },
        ),
        s(
            "replyrecv t1",
            Op::ReplyRecv {
                tcb: 4,
                reply: (0, 6),
                ep: (0, 4),
                recv_reply: Some((0, 7)),
                label: 3,
                len: 2,
            },
        ),
        s(
            "call t2",
            Op::Call {
                tcb: 5,
                ep: (0, 4),
                label: 4,
                len: 1,
            },
        ),
        s(
            "replyrecv t1 cung reply",
            Op::ReplyRecv {
                tcb: 4,
                reply: (0, 7),
                ep: (0, 4),
                recv_reply: Some((0, 7)),
                label: 5,
                len: 2,
            },
        ),
        s(
            "call t0 6",
            Op::Call {
                tcb: 3,
                ep: (0, 4),
                label: 6,
                len: 2,
            },
        ),
        s(
            "recv t1 R1 dang active",
            Op::Recv {
                tcb: 4,
                ep: (0, 4),
                reply: Some((0, 7)),
            },
        ),
        s(
            "recv t1 R2",
            Op::Recv {
                tcb: 4,
                ep: (0, 4),
                reply: Some((0, 8)),
            },
        ),
        s(
            "call t2 7",
            Op::Call {
                tcb: 5,
                ep: (0, 4),
                label: 7,
                len: 1,
            },
        ),
        s(
            "reply t1 R2",
            Op::Reply {
                tcb: 4,
                reply: (0, 8),
            },
        ),
        s(
            "transition t0 Ready (roi Reply R1)",
            Op::Transition {
                tcb: 3,
                to: CS::Ready,
            },
        ),
        s(
            "transition t0 -> Reply R3 (bind)",
            Op::Transition {
                tcb: 3,
                to: CS::BlockedOnReply { reply: 11 },
            },
        ),
        s(
            "transition t2 -> Reply R3 (xung dot)",
            Op::Transition {
                tcb: 5,
                to: CS::BlockedOnReply { reply: 11 },
            },
        ),
        s(
            "transition t0 Ready (go caller R3)",
            Op::Transition {
                tcb: 3,
                to: CS::Ready,
            },
        ),
        s(
            "transition t2 -> Reply R3",
            Op::Transition {
                tcb: 5,
                to: CS::BlockedOnReply { reply: 11 },
            },
        ),
        s(
            "transition t2 Ready",
            Op::Transition {
                tcb: 5,
                to: CS::Ready,
            },
        ),
        s(
            "nbsendrecv t0",
            Op::NbSendRecv {
                tcb: 3,
                send_ep: (0, 4),
                recv_ep: (0, 5),
                recv_reply: Some((0, 9)),
                label: 8,
                len: 1,
            },
        ),
        s(
            "nbsendwait t2",
            Op::NbSendWait {
                tcb: 5,
                send_ep: (0, 4),
                ntfn: (0, 10),
                label: 9,
                len: 1,
            },
        ),
    ];
    run("kt25/34", &ops);
}

// ---------------------------------------------------------------------------------------------
// KT-27 — Notification badge OR, wait/wake, bound TCB
// ---------------------------------------------------------------------------------------------

#[test]
fn kt60_parity_ipc27_notification() {
    let s = |label: &'static str, op: Op| (label, op);
    let ops = vec![
        s(
            "wait t1 N0",
            Op::Recv {
                tcb: 4,
                ep: (0, 10),
                reply: None,
            },
        ),
        s(
            "signal N0 badge 0x10",
            Op::Send {
                tcb: 5,
                cap: (0, 14),
                label: 1,
                len: 0,
            },
        ),
        s(
            "recv t1 N0 kem Reply",
            Op::Recv {
                tcb: 4,
                ep: (0, 10),
                reply: Some((0, 6)),
            },
        ),
        s(
            "wait t1 N0 lan 2",
            Op::Recv {
                tcb: 4,
                ep: (0, 10),
                reply: None,
            },
        ),
        s(
            "signal N0 badge 0x20",
            Op::Send {
                tcb: 5,
                cap: (0, 15),
                label: 2,
                len: 0,
            },
        ),
        s(
            "signal N0 badge 0",
            Op::Send {
                tcb: 5,
                cap: (0, 10),
                label: 3,
                len: 0,
            },
        ),
        s(
            "signal N0 badge 0x10 khong waiter",
            Op::Send {
                tcb: 5,
                cap: (0, 14),
                label: 4,
                len: 0,
            },
        ),
        s(
            "signal N0 badge 0x02 khong waiter (OR)",
            Op::Send {
                tcb: 5,
                cap: (0, 17),
                label: 10,
                len: 0,
            },
        ),
        s(
            "wait t1 N0 an word",
            Op::Recv {
                tcb: 4,
                ep: (0, 10),
                reply: None,
            },
        ),
        s(
            "wait t1 N0 chan",
            Op::Recv {
                tcb: 4,
                ep: (0, 10),
                reply: None,
            },
        ),
        s(
            "wait t2 N0 Busy",
            Op::Recv {
                tcb: 5,
                ep: (0, 10),
                reply: None,
            },
        ),
        s(
            "signal N0 tu t2 dang chan",
            Op::Send {
                tcb: 5,
                cap: (0, 14),
                label: 5,
                len: 0,
            },
        ),
        s(
            "signal N0 wake t1",
            Op::Send {
                tcb: 3,
                cap: (0, 15),
                label: 6,
                len: 0,
            },
        ),
        s(
            "bind t2 N1",
            Op::BindNtfn {
                tcb: (0, 3),
                ntfn: (0, 11),
            },
        ),
        s(
            "recv t2 EP1 chan",
            Op::Recv {
                tcb: 5,
                ep: (0, 5),
                reply: None,
            },
        ),
        s(
            "signal N1 badge 0x08 bound",
            Op::Send {
                tcb: 3,
                cap: (0, 16),
                label: 7,
                len: 0,
            },
        ),
        s(
            "bind t2 N0 Busy",
            Op::BindNtfn {
                tcb: (0, 3),
                ntfn: (0, 10),
            },
        ),
        s("unbind t1 N0", Op::UnbindNtfn { tcb: (0, 2) }),
        s(
            "bind t2 N0",
            Op::BindNtfn {
                tcb: (0, 3),
                ntfn: (0, 10),
            },
        ),
        s(
            "bind t1 N0 Busy",
            Op::BindNtfn {
                tcb: (0, 2),
                ntfn: (0, 10),
            },
        ),
        s("unbind t2 N0", Op::UnbindNtfn { tcb: (0, 3) }),
        // F3 reviewer-kt60: block_current(BlockedOnNotification) phải ghi waiter; TCB thứ hai
        // block cùng notification ⇒ Busy; Signal đánh thức đúng waiter.
        s("dispatch t0", Op::Dispatch { hart: 0 }),
        s(
            "block_current t0 N0 (F3)",
            Op::BlockCurrent {
                hart: 0,
                to: CS::BlockedOnNotification { ntfn: 12 },
            },
        ),
        s("dispatch t2", Op::Dispatch { hart: 0 }),
        s(
            "block_current t2 N0 Busy",
            Op::BlockCurrent {
                hart: 0,
                to: CS::BlockedOnNotification { ntfn: 12 },
            },
        ),
        s(
            "signal N0 wake t0 (F3)",
            Op::Send {
                tcb: 5,
                cap: (0, 14),
                label: 11,
                len: 0,
            },
        ),
        s("dispatch t0 sau wake", Op::Dispatch { hart: 0 }),
        s(
            "signal N0 badge 0x20 poll",
            Op::Send {
                tcb: 3,
                cap: (0, 15),
                label: 8,
                len: 0,
            },
        ),
        s(
            "nbrecv t1 N0",
            Op::NbRecv {
                tcb: 4,
                ep: (0, 10),
            },
        ),
        s(
            "nbrecv t1 N0 rong",
            Op::NbRecv {
                tcb: 4,
                ep: (0, 10),
            },
        ),
    ];
    run("kt27", &ops);
}

// ---------------------------------------------------------------------------------------------
// KT-28 — WaitAddr: FIFO, va chạm bucket, không timeout
// ---------------------------------------------------------------------------------------------

#[test]
fn kt60_parity_ipc28_waitaddr() {
    let s = |label: &'static str, op: Op| (label, op);
    let ops = vec![
        s(
            "set word",
            Op::SetAddr {
                frame: 3,
                offset: 0,
                value: 42,
            },
        ),
        s(
            "wait sai expected",
            Op::WaitAddr {
                tcb: (0, 1),
                frame: 3,
                offset: 0,
                expected: 41,
            },
        ),
        s(
            "wait t0",
            Op::WaitAddr {
                tcb: (0, 1),
                frame: 3,
                offset: 0,
                expected: 42,
            },
        ),
        s(
            "wait t1",
            Op::WaitAddr {
                tcb: (0, 2),
                frame: 3,
                offset: 0,
                expected: 42,
            },
        ),
        s(
            "wait t2",
            Op::WaitAddr {
                tcb: (0, 3),
                frame: 3,
                offset: 0,
                expected: 42,
            },
        ),
        s("tick 100 khong danh thuc", Op::Tick { n: 100 }),
        s(
            "wake 1",
            Op::WakeAddr {
                frame: 3,
                offset: 0,
                n: 1,
            },
        ),
        s(
            "wake 0",
            Op::WakeAddr {
                frame: 3,
                offset: 0,
                n: 0,
            },
        ),
        s(
            "wake tat ca",
            Op::WakeAddr {
                frame: 3,
                offset: 0,
                n: u32::MAX,
            },
        ),
        s(
            "set word va cham",
            Op::SetAddr {
                frame: 3,
                offset: 64,
                value: 7,
            },
        ),
        s(
            "wait t0 va cham",
            Op::WaitAddr {
                tcb: (0, 1),
                frame: 3,
                offset: 64,
                expected: 7,
            },
        ),
        s(
            "wait t1 khoa khac",
            Op::WaitAddr {
                tcb: (0, 2),
                frame: 3,
                offset: 0,
                expected: 42,
            },
        ),
        s(
            "wake va cham",
            Op::WakeAddr {
                frame: 3,
                offset: 64,
                n: 1,
            },
        ),
        s(
            "wake khoa khac",
            Op::WakeAddr {
                frame: 3,
                offset: 0,
                n: 1,
            },
        ),
        s(
            "wake khong ai",
            Op::WakeAddr {
                frame: 4,
                offset: 0,
                n: 5,
            },
        ),
        s(
            "suspend t2 Abort",
            Op::Suspend {
                tcb: (0, 3),
                abort: true,
            },
        ),
        s(
            "wait khi suspended",
            Op::WaitAddr {
                tcb: (0, 3),
                frame: 3,
                offset: 0,
                expected: 42,
            },
        ),
        s("resume t2", Op::Resume { tcb: (0, 3) }),
        s(
            "wait t2 sau resume",
            Op::WaitAddr {
                tcb: (0, 3),
                frame: 3,
                offset: 0,
                expected: 42,
            },
        ),
        s(
            "wake t2",
            Op::WakeAddr {
                frame: 3,
                offset: 0,
                n: u32::MAX,
            },
        ),
        s("dispatch hart0", Op::Dispatch { hart: 0 }),
    ];
    run("kt28", &ops);
}

// ---------------------------------------------------------------------------------------------
// KT-26 + D2-25 — fault Exit không Reply, fault khác Call+Reply
// ---------------------------------------------------------------------------------------------

#[test]
fn kt60_parity_ipc26_fault_exit() {
    let s = |label: &'static str, op: Op| (label, op);
    let mut ops = vec![
        s("exit t0", Op::Exit { tcb: 3, code: 42 }),
        s(
            "recv t1 fault",
            Op::Recv {
                tcb: 4,
                ep: (0, 4),
                reply: None,
            },
        ),
        s("exit t0 lan 2", Op::Exit { tcb: 3, code: 43 }),
        s(
            "recv t1 EP0 chan",
            Op::Recv {
                tcb: 4,
                ep: (0, 4),
                reply: None,
            },
        ),
        s("exit t2 fast path", Op::Exit { tcb: 5, code: 7 }),
        s(
            "inject fault UnknownSyscall",
            Op::InjectFault {
                tcb: 5,
                kind: 1,
                val: 5,
            },
        ),
        s("exit khong fault_ep", Op::Exit { tcb: 3, code: 1 }),
    ];
    // Hàng gửi đầy ⇒ Exit có fault_ep trả Busy trước mọi thay đổi.
    for i in 0..8u32 {
        ops.push(s(
            "nbsend lap hang",
            Op::NbSend {
                tcb: 3,
                cap: (0, 4),
                label: 40 + i,
                len: 1,
            },
        ));
    }
    ops.push(s("exit khi hang day", Op::Exit { tcb: 5, code: 9 }));
    for _ in 0..8 {
        ops.push(s("nbrecv t1", Op::NbRecv { tcb: 4, ep: (0, 4) }));
    }
    ops.push(s("exit sau khi day", Op::Exit { tcb: 5, code: 9 }));
    ops.push(s(
        "recv t1 fault 2",
        Op::Recv {
            tcb: 4,
            ep: (0, 4),
            reply: None,
        },
    ));
    // inject fault ≠ Exit với waiter có Reply ⇒ caller BlockedOnReply, reply resume.
    ops.push(s(
        "recv t1 EP0 + R0",
        Op::Recv {
            tcb: 4,
            ep: (0, 4),
            reply: Some((0, 6)),
        },
    ));
    ops.push(s(
        "inject fault co waiter",
        Op::InjectFault {
            tcb: 5,
            kind: 2,
            val: 0x1000,
        },
    ));
    ops.push(s(
        "reply resume",
        Op::Reply {
            tcb: 4,
            reply: (0, 6),
        },
    ));
    run("kt26", &ops);
}

// ---------------------------------------------------------------------------------------------
// KT-21 — donation tự động + hai hook (rời BlockedOnReply)
// ---------------------------------------------------------------------------------------------

#[test]
fn kt60_parity_ipc21_donation_hook() {
    let s = |label: &'static str, op: Op| (label, op);
    let ops = vec![
        s(
            "call t0",
            Op::Call {
                tcb: 3,
                ep: (0, 4),
                label: 1,
                len: 2,
            },
        ),
        s(
            "recv t1 R0 (donation)",
            Op::Recv {
                tcb: 4,
                ep: (0, 4),
                reply: Some((0, 6)),
            },
        ),
        s(
            "reply R0 tra donation",
            Op::Reply {
                tcb: 4,
                reply: (0, 6),
            },
        ),
        s(
            "call t0 lan 2",
            Op::Call {
                tcb: 3,
                ep: (0, 4),
                label: 2,
                len: 2,
            },
        ),
        s(
            "recv t1 R1 (donation)",
            Op::Recv {
                tcb: 4,
                ep: (0, 4),
                reply: Some((0, 7)),
            },
        ),
        s(
            "transition t0 Ready (hook 1)",
            Op::Transition {
                tcb: 3,
                to: CS::Ready,
            },
        ),
        s(
            "reply R1 sau hook",
            Op::Reply {
                tcb: 4,
                reply: (0, 7),
            },
        ),
        s(
            "call t0 lan 3",
            Op::Call {
                tcb: 3,
                ep: (0, 4),
                label: 3,
                len: 1,
            },
        ),
        s(
            "recv t1 R2 (donation)",
            Op::Recv {
                tcb: 4,
                ep: (0, 4),
                reply: Some((0, 8)),
            },
        ),
        // F2 reviewer-kt60: donor đang donate không đổi priority/SC.
        s(
            "set priority donor (F2)",
            Op::SetPriority {
                tcb: (0, 1),
                p: 4,
                mcp: 10,
            },
        ),
        s(
            "set sched donor (F2)",
            Op::SetSched {
                tcb: (0, 1),
                sc: None,
            },
        ),
        // F1 reviewer-kt60: Suspend(Abort) T1 (donee) phải trả SC + đánh thức T0.
        s(
            "suspend t1 donee (F1)",
            Op::Suspend {
                tcb: (0, 2),
                abort: true,
            },
        ),
        s("resume t1", Op::Resume { tcb: (0, 2) }),
        s(
            "call t0 lan 4",
            Op::Call {
                tcb: 3,
                ep: (0, 4),
                label: 4,
                len: 2,
            },
        ),
        s(
            "recv t1 R3 (donation 2)",
            Op::Recv {
                tcb: 4,
                ep: (0, 4),
                reply: Some((0, 9)),
            },
        ),
        s("exit t0 khi donate", Op::Exit { tcb: 3, code: 55 }),
        s(
            "recv t1 fault sau exit",
            Op::Recv {
                tcb: 4,
                ep: (0, 4),
                reply: None,
            },
        ),
        s(
            "call t0 sau Inactive",
            Op::Call {
                tcb: 3,
                ep: (0, 4),
                label: 5,
                len: 1,
            },
        ),
        s("tick 4 budget", Op::Tick { n: 4 }),
        s("dispatch", Op::Dispatch { hart: 0 }),
    ];
    run("kt21", &ops);
}

/// D2-84: pin core ≤ 196.608 B + bảng phân bố (D2-84 bắt buộc có bảng; in ra để journal).
#[test]
fn kt60_pin_dau_chan_d2_84() {
    let f = kc::Machine::footprint();
    let mut sum = 0usize;
    for i in 0..kc::FOOTPRINT_PARTS {
        println!("{:28} {:>8}", f.names[i], f.bytes[i]);
        sum += f.bytes[i];
    }
    println!("{:28} {:>8}", "scalars", f.scalars);
    println!("{:28} {:>8}", "padding", f.padding);
    println!("{:28} {:>8}", "TOTAL", f.total);
    assert_eq!(sum + f.scalars + f.padding, f.total);
    // F5 reviewer-kt60 (D2-85c): không tautology — padding phải nhỏ (repr(Rust) hiện 1 B);
    // nếu padding phình > 8 B thì bảng phân bố đã bỏ sót trường, không được coi là "đã đo".
    assert!(
        f.padding <= 8,
        "bang phan bo bo sot truong: padding {} B > 8",
        f.padding
    );
    // KT-63: + 3 bộ đếm chuyển cap (`cap_transfer_installed`/`cap_unwrapped`/
    // `cap_transfer_failed`) ⇒ 12 × u64. KT-64: + 4 bộ đếm TLB (`tlb_seq`/`tlb_local_flushes`/
    // `tlb_remote_posts`/`tlb_serviced`) ⇒ **16 × u64**, + `slot_win_top` ⇒ **21 × usize**;
    // `slot_pool` → `slot_mem` + `harness_region`; dải slot theo Region là **hằng**, không field.
    // **KT-309**: cap transfer qua IPC buffer thêm **16** trường vô hướng (4 CPTR nguồn +
    // 4 CPTR đích + `n` + `dst` + 6 bộ đếm) ⇒ `20 × usize` → **`20 × usize + 16 × u64`** giữ
    // nguyên, cộng **16** đơn vị vô hướng. Pin **cố ý** đổi (D2-84): tăng trường phải là quyết
    // định có ghi, không phải trôi.
    assert_eq!(
        f.scalars,
        20 * core::mem::size_of::<usize>() + 16 * core::mem::size_of::<u64>() + 2 + 4 + 1 + 16
    );
    assert!(
        f.total <= 196_608,
        "D2-84: size_of::<Machine>() = {} > 196608",
        f.total
    );
}

/// Biên tĩnh: đầy body Endpoint/Reply/Notification/pool message ⇒ `NotEnoughMemory` **nguyên
/// tử**; revoke trả body về free-list rồi retype lại được (giống test pin KT-59).
#[test]
fn kt60_core_bien_tinh_body_ipc() {
    let mut m = kc::Machine::new();
    let u = m.add_untyped(0x2000_0000, 1 << 20);
    let root = m.add_cnode(128);
    let uc = m.root_cap(u);
    m.put_cap(root, 0, uc).expect("untyped cap");
    // Đầy MAX_ENDPOINTS.
    let mut caps = Vec::new();
    for i in 0..kc::MAX_ENDPOINTS {
        let cap = m
            .retype(kc::SlotRef::new(root, 0), kc::ObjType::Endpoint, 0, 0)
            .expect("endpoint");
        caps.push(cap);
        m.put_cap(root, 1 + i, cap).expect("slot");
    }
    assert_eq!(
        m.retype(kc::SlotRef::new(root, 0), kc::ObjType::Endpoint, 0, 0),
        Err(kc::KError::NotEnoughMemory),
        "Endpoint thứ MAX+1 ⇒ NotEnoughMemory"
    );
    // Reply/Notification cũng có biên.
    for i in 0..kc::MAX_REPLIES {
        let cap = m
            .retype(kc::SlotRef::new(root, 0), kc::ObjType::Reply, 0, 0)
            .expect("reply");
        m.put_cap(root, 32 + i, cap).expect("slot");
    }
    assert_eq!(
        m.retype(kc::SlotRef::new(root, 0), kc::ObjType::Reply, 0, 0),
        Err(kc::KError::NotEnoughMemory)
    );
    for i in 0..kc::MAX_NOTIFICATIONS {
        let cap = m
            .retype(kc::SlotRef::new(root, 0), kc::ObjType::Notification, 0, 0)
            .expect("ntfn");
        m.put_cap(root, 64 + i, cap).expect("slot");
    }
    assert_eq!(
        m.retype(kc::SlotRef::new(root, 0), kc::ObjType::Notification, 0, 0),
        Err(kc::KError::NotEnoughMemory)
    );
    // Pool message đầy (24) ⇒ Call/Send thứ 25 trả Busy/NotEnoughMemory trước khi đổi trạng
    // thái: dùng một Endpoint + 8 ô, 3 Endpoint đầy = 24 message.
    let t = m.add_kernel_object(kc::ObjType::Tcb);
    let tc = m.root_cap(t);
    m.put_cap(root, 100, tc).expect("tcb slot");
    for k in 0..3usize {
        let ep = m.cap_obj_at(root, 1 + k).expect("ep obj");
        for j in 0..8u32 {
            let msg = kc::Message::new(
                kc::MessageInfo::new(100 + j, 0, false, 0, 1).unwrap(),
                &[j as u64],
            )
            .unwrap();
            m.nb_send(t, kc::SlotRef::new(root, 1 + k), msg)
                .expect("nbsend vào hàng");
            let _ = ep;
        }
    }
    assert_eq!(m.ipc_pending(), 24);
    let ep4 = m.cap_obj_at(root, 4).expect("ep 4");
    let msg = kc::Message::new(kc::MessageInfo::new(9, 0, false, 0, 1).unwrap(), &[9]).unwrap();
    assert_eq!(
        m.nb_send(t, kc::SlotRef::new(root, 4), msg),
        Err(kc::KError::Busy),
        "hàng Endpoint thứ 4 đầy ⇒ Busy (không đụng pool)"
    );
    assert_eq!(m.ipc_pending(), 24);
    let _ = ep4;
}

// ---------------------------------------------------------------------------------------------
// Pin ngữ nghĩa cứng (không chỉ so hai bên) — trạng thái/bộ đếm cuối kịch bản
// ---------------------------------------------------------------------------------------------

#[test]
fn kt60_core_waitaddr_khong_timeout_va_wake_fifo() {
    let mut m = kc::Machine::new();
    let u = m.add_untyped(0x3000_0000, 1 << 20);
    let root = m.add_cnode(16);
    let uc = m.root_cap(u);
    m.put_cap(root, 0, uc).unwrap();
    let t0 = m.add_kernel_object(kc::ObjType::Tcb);
    let t1 = m.add_kernel_object(kc::ObjType::Tcb);
    let c0 = m.root_cap(t0);
    let c1 = m.root_cap(t1);
    m.put_cap(root, 1, c0).unwrap();
    m.put_cap(root, 2, c1).unwrap();
    m.set_addr_word(7, 0, 0xAA).unwrap();
    m.wait_addr(kc::SlotRef::new(root, 1), 7, 0, 0xAA).unwrap();
    m.wait_addr(kc::SlotRef::new(root, 2), 7, 0, 0xAA).unwrap();
    // Không timeout: tick ảo rất lớn không đánh thức, không sinh `Sleeping{Timeout}`.
    for _ in 0..10 {
        m.tick(1000).unwrap();
    }
    assert_eq!(
        m.tcb_state(t0).unwrap(),
        kc::TcbState::BlockedOnAddress {
            frame: 7,
            offset: 0
        }
    );
    assert_eq!(m.address_waiters_len(), 2);
    // Wake 1 ⇒ FIFO: T0 ra trước, T1 còn chờ.
    assert_eq!(m.wake_addr(7, 0, 1).unwrap(), 1);
    assert_eq!(m.tcb_state(t0).unwrap(), kc::TcbState::Ready);
    assert_eq!(
        m.tcb_state(t1).unwrap(),
        kc::TcbState::BlockedOnAddress {
            frame: 7,
            offset: 0
        }
    );
    assert_eq!(m.wake_addr(7, 0, u32::MAX).unwrap(), 1);
    assert_eq!(m.tcb_state(t1).unwrap(), kc::TcbState::Ready);
    assert_eq!(m.address_waiters_len(), 0);
}

#[test]
fn kt60_core_endpoint_busy_va_fifo() {
    let mut m = kc::Machine::new();
    let u = m.add_untyped(0x4000_0000, 1 << 20);
    let root = m.add_cnode(32);
    let uc = m.root_cap(u);
    m.put_cap(root, 0, uc).unwrap();
    let t0 = m.add_kernel_object(kc::ObjType::Tcb);
    let t1 = m.add_kernel_object(kc::ObjType::Tcb);
    let ep = m.add_kernel_object(kc::ObjType::Endpoint);
    let c0 = m.root_cap(t0);
    let c1 = m.root_cap(t1);
    let ce = m.root_cap(ep);
    m.put_cap(root, 1, c0).unwrap();
    m.put_cap(root, 2, c1).unwrap();
    m.put_cap(root, 3, ce).unwrap();
    // 8 message ⇒ đầy; đúng thứ tự FIFO; cái thứ 9 ⇒ `Busy` (không mất message).
    for i in 0..8u32 {
        let msg = kc::Message::new(
            kc::MessageInfo::new(10 + i, 0, false, 0, 1).unwrap(),
            &[i as u64],
        )
        .unwrap();
        m.nb_send(t0, kc::SlotRef::new(root, 3), msg).unwrap();
    }
    let msg = kc::Message::new(kc::MessageInfo::new(99, 0, false, 0, 1).unwrap(), &[99]).unwrap();
    assert_eq!(
        m.nb_send(t0, kc::SlotRef::new(root, 3), msg),
        Err(kc::KError::Busy)
    );
    assert_eq!(m.endpoint_send_len(ep).unwrap(), 8);
    assert_eq!(m.ipc_pending(), 8);
    for i in 0..8u64 {
        let got = m
            .nb_recv(t1, kc::SlotRef::new(root, 3))
            .unwrap()
            .expect("còn message");
        assert_eq!(got.msg_id, i + 1, "msg_id FIFO");
        assert_eq!(got.mr[0], i, "nội dung FIFO");
    }
    assert_eq!(
        m.nb_recv(t1, kc::SlotRef::new(root, 3)),
        Err(kc::KError::Busy)
    );
    assert_eq!(m.ipc_pending(), 0);
    assert_eq!(m.ipc_sent(), 8);
    assert_eq!(m.ipc_delivered(), 8);
    assert_eq!(m.transfer_calls(), 8);
}

#[test]
fn kt60_core_donation_tao_va_tra_kt21() {
    let mut m = kc::Machine::new();
    let u = m.add_untyped(0x5000_0000, 1 << 20);
    let root = m.add_cnode(16);
    let uc = m.root_cap(u);
    m.put_cap(root, 0, uc).unwrap();
    let mut objs = Vec::new();
    for ty in [
        kc::ObjType::Tcb,
        kc::ObjType::Tcb,
        kc::ObjType::Endpoint,
        kc::ObjType::Reply,
        kc::ObjType::SchedControl,
        kc::ObjType::SchedContext,
    ] {
        let cap = m.retype(kc::SlotRef::new(root, 0), ty, 0, 0).unwrap();
        m.put_cap(root, 1 + objs.len(), cap).unwrap();
        objs.push(m.cap_obj_at(root, 1 + objs.len()).unwrap());
    }
    let t0 = objs[0];
    let t1 = objs[1];
    let ep = objs[2];
    let r0 = objs[3];
    let sc = objs[5];
    m.tcb_set_priority(kc::SlotRef::new(root, 1), 3, 10)
        .unwrap();
    m.tcb_set_priority(kc::SlotRef::new(root, 2), 1, 10)
        .unwrap();
    m.sched_control_configure(kc::SlotRef::new(root, 5), kc::SlotRef::new(root, 6), 4, 8)
        .unwrap();
    m.set_sched_params(kc::SlotRef::new(root, 1), Some(kc::SlotRef::new(root, 6)))
        .unwrap();
    m.call(t0, kc::SlotRef::new(root, 3), 1).unwrap();
    m.recv(
        t1,
        kc::SlotRef::new(root, 3),
        Some(kc::SlotRef::new(root, 4)),
    )
    .unwrap();
    let i0 = m.tcb_info_obj(t0).unwrap();
    let i1 = m.tcb_info_obj(t1).unwrap();
    assert_eq!(i1.donated_sc, Some(sc), "server nhận donation");
    assert_eq!(i0.donated_sc, None, "client là donor, không nhận donation");
    let sc_info = m.sched_info(kc::SlotRef::new(root, 6)).unwrap();
    assert_eq!(sc_info.donated_to, Some(t1));
    assert_eq!(sc_info.donor, Some(t0));
    assert_eq!(m.effective_priority(t1).unwrap(), 3);
    m.reply(t1, kc::SlotRef::new(root, 4)).unwrap();
    assert_eq!(m.tcb_info_obj(t1).unwrap().donated_sc, None);
    let sc_info = m.sched_info(kc::SlotRef::new(root, 6)).unwrap();
    assert_eq!(sc_info.donated_to, None);
    assert_eq!(sc_info.donor, None);
    let _ = (ep, r0);
}

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
