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

// **KT-407** — tách theo miền (tệp gốc 4279 dòng).
mod tests_ipc;
mod tests_kt63;

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
