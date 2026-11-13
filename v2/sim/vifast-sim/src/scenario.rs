// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-09 — bộ chạy kịch bản tối giản trên sim + ngữ nghĩa thời điểm `expect_states` (C5, D2-41).
//!
//! `expect_states` là **hậu điều kiện sau mỗi bước** của thread tương ứng, kiểm **ngay sau khi
//! bước trả về**; với `exit`/`debug_halt` kiểm ngay sau `halt`, **trước** khi procsrv `Recv`
//! xử lý xong. Bộ chạy ở đây kiểm đúng thứ tự đó: `execute` → so kết quả → so trạng thái →
//! kiểm bất biến byte → mới sang bước kế.
//!
//! Kịch bản là **dữ liệu** (thiết kế §9.3) nhưng ở mức Rust cho KT-09; `.scn` văn bản vẫn do
//! `v2/test/scn/check_scn.py` kiểm tĩnh (KT-04c) và `vifast-scenario` đầy đủ để sau (KT-11).
//!
//! Hai chế độ:
//! - `run_sequential`: thứ tự cố định — dùng cho end-to-end và kiểm C5.
//! - `run_interleaved`: scheduler tối thiểu chen **giữa các bước**, chọn thread theo seed
//!   (cùng seed ⇒ cùng trace); kiểm bất biến byte sau **mỗi** event. **KT-23**: chỉ chọn
//!   thread có bước kế **chạy được dưới scheduler** (xem [`Step::dispatchable`]) — tôn trọng
//!   `Running`/ready queue/`SchedContext` budget; cùng seed ⇒ cùng trace (DoD 1).

use std::collections::VecDeque;
use std::fmt;

use crate::ipc::{Message, MessageInfo, MAX_CAPS, MAX_WORDS};
use crate::kobj::{
    CapId, FaultKind, KError, Machine, ObjId, ObjType, SlotRef, SuspendMode, TcbState,
};
use crate::Lcg;

/// **KT-24** — message đầy đủ cho bước kịch bản `Send`/`NBSend` (model-only `[TK]`):
/// `MessageInfo` 35 bit + **64 MR** (`spec/abi.toml:31-49`). Cap/badge thuộc KT-25 nên `Msg`
/// của KT-24 không mang cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Msg {
    pub info: MessageInfo,
    pub mr: [u64; MAX_WORDS],
}

impl Msg {
    /// Dựng `Msg` từ `info` + `words`; `words.len()` phải bằng `info.length()` (không tự sửa
    /// `info`, không cắt bớt MR).
    pub fn new(info: MessageInfo, words: &[u64]) -> Result<Self, KError> {
        if words.len() != info.length() as usize || words.len() > MAX_WORDS {
            return Err(KError::InvalidArgument);
        }
        let mut mr = [0u64; MAX_WORDS];
        mr[..words.len()].copy_from_slice(words);
        Ok(Self { info, mr })
    }

    /// Các MR **thật** của `Msg` (`mr[..length]`) — dùng cho bước `SendCaps` (KT-25).
    pub fn words(&self) -> &[u64] {
        &self.mr[..self.info.length() as usize]
    }

    /// `Message` tương ứng (chưa có `msg_id` — kernel cấp khi nhận vào đường IPC).
    pub fn to_message(self) -> Result<Message, KError> {
        Message::new(self.info, &self.mr[..self.info.length() as usize])
    }
}

/// Một bước kịch bản — chiếu tối giản của `enum Step` thiết kế §9.3 sang ABI v2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Retype {
        src: SlotRef,
        ty: ObjType,
        size_bits: u32,
        flags: u32,
        dst: SlotRef,
    },
    Copy {
        src: SlotRef,
        dst: SlotRef,
    },
    Delete {
        slot: SlotRef,
    },
    Revoke {
        cnode: ObjId,
        slot: Option<usize>,
    },
    TcbConfigure {
        tcb: SlotRef,
        cspace: Option<SlotRef>,
    },
    TcbSetFaultEp {
        tcb: SlotRef,
        ep: SlotRef,
    },
    SchedControlConfigure {
        sched_control: SlotRef,
        sched: SlotRef,
        budget: u64,
        period: u64,
    },
    SchedContextBind {
        sched: SlotRef,
        target: SlotRef,
    },
    Call {
        tcb: ObjId,
        ep: SlotRef,
        mr: u64,
    },
    Recv {
        tcb: ObjId,
        ep: SlotRef,
        reply: Option<SlotRef>,
    },
    Reply {
        tcb: ObjId,
        reply: SlotRef,
    },
    Exit {
        tcb: ObjId,
        code: u64,
    },
    // ------------------------------------------- KT-24: IPC message 64 MR (model-only `[TK]`)
    /// `Send` trên Endpoint/Reply (`Step::Call` cũ vẫn là wrapper 1 MR của `kt09_e2e`).
    Send {
        tcb: ObjId,
        cap: SlotRef,
        msg: Msg,
    },
    /// `NBSend` — gửi **không chặn** (hàng gửi của Endpoint hoặc giao thẳng cho waiter).
    NBSend {
        tcb: ObjId,
        cap: SlotRef,
        msg: Msg,
    },
    /// `NBRecv` — poll Endpoint; rỗng ⇒ `Busy`, không chặn.
    NBRecv {
        tcb: ObjId,
        cap: SlotRef,
    },
    // ------------------------------------------- KT-25: IPC core 2 (4 cap/ReplyRecv/NB*)
    /// `ReplyRecv` — giao Reply (`reply`) rồi `Recv` trên `ep`; `recv_reply` là Reply **mới**
    /// cấp cho Call kế (`None` = không cấp).
    ReplyRecv {
        tcb: ObjId,
        reply: SlotRef,
        ep: SlotRef,
        msg: Msg,
        recv_reply: Option<SlotRef>,
    },
    /// `NBSendRecv` (D2-53 Q15a) — `NBSend` không chặn rồi `Recv`; lỗi của `NBSend` không
    /// chặn `Recv` (bước vẫn `Ok` nếu `Recv` chạy được).
    NBSendRecv {
        tcb: ObjId,
        send: SlotRef,
        msg: Msg,
        recv: SlotRef,
        recv_reply: Option<SlotRef>,
    },
    /// `NBSendWait` (D2-53 Q15a) — `NBSend` không chặn rồi `Wait` trên Notification.
    NBSendWait {
        tcb: ObjId,
        send: SlotRef,
        msg: Msg,
        ntfn: SlotRef,
    },
    /// `Send` mang **cap** (≤ 4, `caps[..n_caps]`) tới Endpoint/Reply — đường chuyển cap của
    /// KT-25 (`MessageInfo.caps` ≤ 4, `spec/abi.toml:34`).
    SendCaps {
        tcb: ObjId,
        cap: SlotRef,
        msg: Msg,
        caps: [CapId; MAX_CAPS],
        n_caps: u8,
    },
    TcbReadState {
        tcb: SlotRef,
    },
    // ------------------------------------------- KT-26: fault + Exit (model-only `[TK]`)
    /// Fault **khác** Exit (model API [`Machine::inject_fault`]) — fault đi **Call + Reply** tới
    /// `fault_ep` (thiết kế §4.6 `:321`; `Exit` giữ đường `Send` của D2-25 qua bước `Exit`).
    Fault {
        tcb: ObjId,
        kind: FaultKind,
    },
    /// Cho TCB chạy tiếp sau fault với **thanh ghi mới** (model API
    /// [`Machine::tcb_resume_with_regs`]) — cửa duy nhất "ghi message rồi mới `Ready`" (r319).
    ResumeWithRegs {
        tcb: SlotRef,
        msg: Msg,
    },
    /// procsrv `Reply` cho fault đang chờ (`Call` + `Reply`) kèm thanh ghi mới — tương đương
    /// `Send` trên cap Reply nhưng nêu rõ ngữ nghĩa fault-resume của KT-26.
    ReplyFault {
        tcb: ObjId,
        reply: SlotRef,
        msg: Msg,
    },
    // ------------------------------------------- KT-23: bước TCB/scheduler của K1b
    //
    // Các bước dưới gọi **model API** của K1b (D2-49): `Tcb.Suspend/Resume` (KT-18),
    // `Tcb.ReadState` (KT-18), `set_sched_params` (D2-49(b), KT-20) và scheduler `tick`/
    // `dispatch` (KT-19/20). Không thêm method ABI nào; tên bước là chiếu của method đã có
    // trong draft chữ ký (`v2/docs/abi-call-signatures.toml`) — riêng `set_sched_params` là
    // model API `[TK]` (D2-49(b)).
    TcbSuspend {
        tcb: SlotRef,
        mode: SuspendMode,
    },
    TcbResume {
        tcb: SlotRef,
    },
    /// Gắn/gỡ `SchedContext` của TCB bằng model API `set_sched_params` (D2-49(b)).
    TcbSetSchedParams {
        tcb: SlotRef,
        sched: Option<SlotRef>,
    },
    /// `tick(n)` của đồng hồ ảo (D2-49(h)) — charge `Running` rồi xử lý biên refill.
    SchedTick {
        n: u64,
    },
    /// `pick` + chuyển ngữ cảnh = `dispatch(hart)` (KT-19): `Ready` cao nhất → `Running`.
    /// `pick` thuần chỉ **đọc** nên không tạo event; bước này là "một lát cắt scheduler".
    SchedPick {
        hart: u16,
    },
}

impl Step {
    pub fn name(self) -> &'static str {
        match self {
            Step::Retype { .. } => "retype",
            Step::Copy { .. } => "copy",
            Step::Delete { .. } => "delete",
            Step::Revoke { .. } => "revoke",
            Step::TcbConfigure { .. } => "tcb_configure",
            Step::TcbSetFaultEp { .. } => "tcb_set_fault_ep",
            Step::SchedControlConfigure { .. } => "sched_control_configure",
            Step::SchedContextBind { .. } => "sched_context_bind",
            Step::Call { .. } => "call",
            Step::Recv { .. } => "recv",
            Step::Reply { .. } => "reply",
            Step::Exit { .. } => "exit",
            Step::Send { .. } => "send",
            Step::NBSend { .. } => "nb_send",
            Step::NBRecv { .. } => "nb_recv",
            Step::ReplyRecv { .. } => "reply_recv",
            Step::NBSendRecv { .. } => "nb_send_recv",
            Step::NBSendWait { .. } => "nb_send_wait",
            Step::SendCaps { .. } => "send_caps",
            Step::TcbReadState { .. } => "tcb_read_state",
            Step::Fault { .. } => "fault",
            Step::ResumeWithRegs { .. } => "resume_with_regs",
            Step::ReplyFault { .. } => "reply_fault",
            Step::TcbSuspend { .. } => "tcb_suspend",
            Step::TcbResume { .. } => "tcb_resume",
            Step::TcbSetSchedParams { .. } => "tcb_set_sched_params",
            Step::SchedTick { .. } => "sched_tick",
            Step::SchedPick { .. } => "sched_pick",
        }
    }

    /// TCB mà `expect_states` nói về (nếu bước có actor). `ReadState`/`Tcb*` tra qua slot.
    pub fn actor(self, m: &Machine) -> Option<ObjId> {
        match self {
            Step::TcbConfigure { tcb, .. }
            | Step::TcbSetFaultEp { tcb, .. }
            | Step::TcbReadState { tcb }
            | Step::TcbSuspend { tcb, .. }
            | Step::TcbResume { tcb }
            | Step::TcbSetSchedParams { tcb, .. }
            | Step::ResumeWithRegs { tcb, .. } => m.cap_obj_at(tcb.cnode, tcb.slot).ok(),
            Step::Call { tcb, .. }
            | Step::Recv { tcb, .. }
            | Step::Reply { tcb, .. }
            | Step::Exit { tcb, .. }
            | Step::Send { tcb, .. }
            | Step::NBSend { tcb, .. }
            | Step::NBRecv { tcb, .. }
            | Step::ReplyRecv { tcb, .. }
            | Step::NBSendRecv { tcb, .. }
            | Step::NBSendWait { tcb, .. }
            | Step::SendCaps { tcb, .. }
            | Step::Fault { tcb, .. }
            | Step::ReplyFault { tcb, .. } => Some(tcb),
            _ => None,
        }
    }
}

/// Một bước + hậu điều kiện mong đợi của nó (C5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScenarioStep {
    pub thread: &'static str,
    pub step: Step,
    pub expect: Result<(), KError>,
    pub expect_state: Option<TcbState>,
}

/// Kết quả một lần chạy: số event, số lần kiểm bất biến byte, trace tất định.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub events: usize,
    pub checked_bytes: usize,
    pub trace: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScenarioError {
    Result {
        index: usize,
        thread: &'static str,
        expected: Result<(), KError>,
        actual: Result<(), KError>,
    },
    State {
        index: usize,
        thread: &'static str,
        expected: TcbState,
        actual: TcbState,
    },
    Bytes {
        index: usize,
        thread: &'static str,
        detail: String,
    },
    Deadlock {
        remaining: usize,
    },
    NoActor {
        index: usize,
        thread: &'static str,
    },
    NotTcb {
        index: usize,
        thread: &'static str,
        error: KError,
    },
}

fn render_result(r: Result<(), KError>) -> &'static str {
    match r {
        Ok(()) => "ok",
        Err(e) => e.name(),
    }
}

impl fmt::Display for ScenarioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScenarioError::Result {
                index,
                thread,
                expected,
                actual,
            } => write!(
                f,
                "bước {index} ({thread}): kết quả lệch — mong đợi {}, thực tế {}",
                render_result(*expected),
                render_result(*actual)
            ),
            ScenarioError::State {
                index,
                thread,
                expected,
                actual,
            } => write!(
                f,
                "bước {index} ({thread}): expect_states lệch — mong đợi {}, thực tế {}",
                expected.render(),
                actual.render()
            ),
            ScenarioError::Bytes {
                index,
                thread,
                detail,
            } => write!(f, "bước {index} ({thread}): bất biến byte vỡ — {detail}"),
            ScenarioError::Deadlock { remaining } => write!(
                f,
                "bế tắc: còn {remaining} bước nhưng không thread nào chạy được"
            ),
            ScenarioError::NoActor { index, thread } => write!(
                f,
                "bước {index} ({thread}): không xác định được TCB để kiểm expect_states"
            ),
            ScenarioError::NotTcb {
                index,
                thread,
                error,
            } => write!(
                f,
                "bước {index} ({thread}): actor không phải TCB ({})",
                error.name()
            ),
        }
    }
}

impl std::error::Error for ScenarioError {}

/// Thực thi một bước trên mô hình.
pub fn execute(m: &mut Machine, step: &Step) -> Result<(), KError> {
    match *step {
        Step::Retype {
            src,
            ty,
            size_bits,
            flags,
            dst,
        } => {
            let cap = m.retype(src, ty, size_bits, flags)?;
            m.put_cap(dst.cnode, dst.slot, cap)
        }
        Step::Copy { src, dst } => m.copy(src, dst).map(|_| ()),
        Step::Delete { slot } => m.delete(slot),
        Step::Revoke { cnode, slot } => m.revoke(cnode, slot).map(|_| ()),
        Step::TcbConfigure { tcb, cspace } => m.tcb_configure(tcb, cspace),
        Step::TcbSetFaultEp { tcb, ep } => m.tcb_set_fault_ep(tcb, ep),
        Step::SchedControlConfigure {
            sched_control,
            sched,
            budget,
            period,
        } => m.sched_control_configure(sched_control, sched, budget, period),
        Step::SchedContextBind { sched, target } => m.sched_context_bind(sched, target),
        Step::Call { tcb, ep, mr } => m.call(tcb, ep, mr),
        Step::Recv { tcb, ep, reply } => m.recv(tcb, ep, reply),
        Step::Reply { tcb, reply } => m.reply(tcb, reply),
        Step::Exit { tcb, code } => m.exit(tcb, code),
        Step::Send { tcb, cap, msg } => m.send(tcb, cap, msg.to_message()?),
        Step::NBSend { tcb, cap, msg } => m.nb_send(tcb, cap, msg.to_message()?),
        Step::NBRecv { tcb, cap } => m.nb_recv(tcb, cap).map(|_| ()),
        Step::ReplyRecv {
            tcb,
            reply,
            ep,
            msg,
            recv_reply,
        } => m.reply_recv(tcb, reply, ep, msg.to_message()?, recv_reply),
        Step::NBSendRecv {
            tcb,
            send,
            msg,
            recv,
            recv_reply,
        } => m
            .nb_send_recv(tcb, send, msg.to_message()?, recv, recv_reply)
            .map(|_| ()),
        Step::NBSendWait {
            tcb,
            send,
            msg,
            ntfn,
        } => m
            .nb_send_wait(tcb, send, msg.to_message()?, ntfn)
            .map(|_| ()),
        Step::SendCaps {
            tcb,
            cap,
            msg,
            caps,
            n_caps,
        } => {
            let n = n_caps as usize;
            if n > MAX_CAPS {
                return Err(KError::InvalidArgument);
            }
            let message = Message::with_cap_ids(msg.info.label(), msg.words(), &caps[..n])?;
            m.send(tcb, cap, message)
        }
        Step::TcbReadState { tcb } => m.tcb_read_state(tcb).map(|_| ()),
        Step::Fault { tcb, kind } => m.inject_fault(tcb, kind),
        Step::ResumeWithRegs { tcb, msg } => m.tcb_resume_with_regs(tcb, msg.to_message()?),
        Step::ReplyFault { tcb, reply, msg } => m.reply_msg(tcb, reply, msg.to_message()?),
        Step::TcbSuspend { tcb, mode } => m.tcb_suspend(tcb, mode),
        Step::TcbResume { tcb } => m.tcb_resume(tcb),
        Step::TcbSetSchedParams { tcb, sched } => m.set_sched_params(tcb, sched),
        Step::SchedTick { n } => m.tick(n),
        Step::SchedPick { hart } => m.dispatch(hart).map(|_| ()),
    }
}

impl Step {
    /// Bước **syscall của một TCB** (cần actor chạy được dưới scheduler) — `Call`/`Recv`/
    /// `Reply`/`Exit`. Các bước quản trị (`TcbSuspend/Resume/ReadState/SetSchedParams`,
    /// `SchedTick/Pick`, retype/copy/...) do **người điều khiển** phát nên luôn chạy được.
    pub fn is_tcb_syscall(self) -> bool {
        matches!(
            self,
            Step::Call { .. }
                | Step::Recv { .. }
                | Step::Reply { .. }
                | Step::Exit { .. }
                | Step::Send { .. }
                | Step::NBSend { .. }
                | Step::NBRecv { .. }
                | Step::ReplyRecv { .. }
                | Step::NBSendRecv { .. }
                | Step::NBSendWait { .. }
                | Step::SendCaps { .. }
                | Step::Fault { .. }
                | Step::ReplyFault { .. }
        )
    }

    /// KT-23 DoD 1 — bước này có **chạy được ngay** dưới scheduler 1 hart không?
    ///
    /// - bước không có TCB actor (retype/copy/delete/revoke/sched configure/tick/pick...) ⇒ có;
    /// - bước quản trị trên TCB (`Suspend/Resume/ReadState/SetSchedParams`) ⇒ có (người điều
    ///   khiển gọi, không phải TCB tự chạy);
    /// - bước **syscall** ⇒ actor phải `Running` **hoặc** `Ready` **đang nằm trong ready queue**
    ///   (không `suspended`) và **còn budget** (`tcb_has_budget`, D2-50: không SC/SC chưa
    ///   `Configure` **không** chạy), hoặc `Inactive` còn budget (kích hoạt ngầm `Inactive→Ready`
    ///   của KT-18). TCB chặn/ngủ/chết/treo ⇒ bước của thread đó **chờ**.
    pub fn dispatchable(self, m: &Machine) -> bool {
        let Some(actor) = self.actor(m) else {
            return true;
        };
        if !self.is_tcb_syscall() {
            return true;
        }
        match m.tcb_state(actor) {
            Ok(TcbState::Running { .. }) => m.tcb_has_budget(actor),
            Ok(TcbState::Ready) => {
                let trong_hang = (0..m.hart_count()).any(|h| m.ready_contains(h, actor));
                trong_hang && m.tcb_has_budget(actor)
            }
            Ok(TcbState::Inactive) => m.tcb_has_budget(actor),
            _ => false,
        }
    }
}

fn check_state(m: &Machine, index: usize, s: &ScenarioStep) -> Result<(), ScenarioError> {
    let Some(want) = &s.expect_state else {
        return Ok(());
    };
    let actor = s.step.actor(m).ok_or(ScenarioError::NoActor {
        index,
        thread: s.thread,
    })?;
    let actual = m.tcb_state(actor).map_err(|error| ScenarioError::NotTcb {
        index,
        thread: s.thread,
        error,
    })?;
    if &actual != want {
        return Err(ScenarioError::State {
            index,
            thread: s.thread,
            expected: *want,
            actual,
        });
    }
    Ok(())
}

fn trace_line(index: usize, s: &ScenarioStep, m: &Machine) -> String {
    format!(
        "event={index} thread={} op={} used={}",
        s.thread,
        s.step.name(),
        m.total_used()
    )
}

/// Chạy theo thứ tự cố định, kiểm C5 sau **từng** bước.
pub fn run_sequential(m: &mut Machine, steps: &[ScenarioStep]) -> Result<Report, ScenarioError> {
    let mut trace = Vec::with_capacity(steps.len());
    let mut checked = 0;
    for (index, s) in steps.iter().enumerate() {
        let got = execute(m, &s.step);
        if got != s.expect {
            return Err(ScenarioError::Result {
                index,
                thread: s.thread,
                expected: s.expect,
                actual: got,
            });
        }
        check_state(m, index, s)?;
        m.check_all_bytes().map_err(|detail| ScenarioError::Bytes {
            index,
            thread: s.thread,
            detail,
        })?;
        checked += 1;
        trace.push(trace_line(index, s, m));
    }
    Ok(Report {
        events: steps.len(),
        checked_bytes: checked,
        trace,
    })
}

/// Chạy chen giữa các bước: mỗi event chọn một thread chạy được theo seed.
///
/// Cùng seed ⇒ cùng trace. Bất biến byte kiểm sau **mỗi** event (`checked_bytes == events`).
pub fn run_interleaved(
    m: &mut Machine,
    steps: &[ScenarioStep],
    seed: u64,
) -> Result<Report, ScenarioError> {
    let mut threads: Vec<&'static str> = Vec::new();
    let mut queues: Vec<VecDeque<usize>> = Vec::new();
    for (i, s) in steps.iter().enumerate() {
        match threads.iter().position(|t| *t == s.thread) {
            Some(p) => queues[p].push_back(i),
            None => {
                threads.push(s.thread);
                queues.push(VecDeque::from([i]));
            }
        }
    }

    let mut rng = Lcg::new(seed);
    let mut trace = Vec::with_capacity(steps.len());
    let mut checked = 0;
    let mut events = 0;
    loop {
        let remaining: usize = queues.iter().map(VecDeque::len).sum();
        if remaining == 0 {
            break;
        }
        // KT-23 DoD 1: chỉ chọn thread có bước kế **chạy được dưới scheduler** (Running/ready
        // queue + budget; xem [`Step::dispatchable`]) — không còn "chọn theo trạng thái TCB"
        // của bản KT-09. Bước không có actor (retype/copy/tick/pick...) luôn chọn được.
        let runnable: Vec<usize> = (0..queues.len())
            .filter(|t| {
                let Some(&idx) = queues[*t].front() else {
                    return false;
                };
                steps[idx].step.dispatchable(m)
            })
            .collect();
        if runnable.is_empty() {
            return Err(ScenarioError::Deadlock { remaining });
        }
        let pick = runnable[rng.below(runnable.len() as u64) as usize];
        let idx = queues[pick].pop_front().expect("đã kiểm còn bước");
        let s = &steps[idx];
        let got = execute(m, &s.step);
        if got != s.expect {
            return Err(ScenarioError::Result {
                index: idx,
                thread: s.thread,
                expected: s.expect,
                actual: got,
            });
        }
        check_state(m, idx, s)?;
        m.check_all_bytes().map_err(|detail| ScenarioError::Bytes {
            index: idx,
            thread: s.thread,
            detail,
        })?;
        checked += 1;
        events += 1;
        trace.push(trace_line(idx, s, m));
    }
    Ok(Report {
        events,
        checked_bytes: checked,
        trace,
    })
}
