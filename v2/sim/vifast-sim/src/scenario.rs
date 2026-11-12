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
//!   (cùng seed ⇒ cùng trace); kiểm bất biến byte sau **mỗi** event.

use std::collections::VecDeque;
use std::fmt;

use crate::kobj::{KError, Machine, ObjId, ObjType, SlotRef, TcbState};
use crate::Lcg;

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
    ReadState {
        tcb: SlotRef,
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
            Step::ReadState { .. } => "read_state",
        }
    }

    /// TCB mà `expect_states` nói về (nếu bước có actor). `ReadState`/`Tcb*` tra qua slot.
    pub fn actor(self, m: &Machine) -> Option<ObjId> {
        match self {
            Step::TcbConfigure { tcb, .. }
            | Step::TcbSetFaultEp { tcb, .. }
            | Step::ReadState { tcb } => m.cap_obj_at(tcb.cnode, tcb.slot).ok(),
            Step::Call { tcb, .. }
            | Step::Recv { tcb, .. }
            | Step::Reply { tcb, .. }
            | Step::Exit { tcb, .. } => Some(tcb),
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
        Step::ReadState { tcb } => m.tcb_read_state(tcb).map(|_| ()),
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
        let runnable: Vec<usize> = (0..queues.len())
            .filter(|t| {
                let Some(&idx) = queues[*t].front() else {
                    return false;
                };
                match steps[idx].step.actor(m) {
                    Some(actor) => !matches!(m.tcb_state(actor), Ok(st) if st.is_runnable()),
                    None => true,
                }
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
