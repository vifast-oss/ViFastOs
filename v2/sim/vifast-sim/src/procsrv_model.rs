// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-26 — **mô hình process server** `[TK]`, **không phải procsrv thật** (D2-53 **Q10**:
//! procsrv + shell/user binary là K3.5). Mục tiêu là chứng minh **kernel-side** đủ cho giao thức
//! `Exit`/`wait`/`kill` của thiết kế §6.4 (`docs/v2/ViFastOs-v2-thiet-ke.md:585-600`) và D2-25:
//!
//! - [`ProcsrvModel::recv_exit`] — `Recv` trên `fault_ep` (có Reply cap) và gom cặp
//!   `(tcb, code)` của các fault `Exit`. Fault **khác** Exit cũng đi qua đây (cùng `fault_ep`,
//!   nhưng là **Call + Reply**) — mô hình ghi lại để [`ProcsrvModel::wait`] trả `Faulted` và để
//!   test gọi [`ProcsrvModel::resume_fault`] (trả lời kèm thanh ghi mới).
//! - [`ProcsrvModel::wait`] — `{code, how}` với `how ∈ {Exited, Cancelled, Faulted, Killed}` đúng
//!   nhánh: `Exited` khi `exit` (có hoặc không `fault_ep`), `Cancelled` khi `Suspend(Abort)` huỷ
//!   message Exit **trước** khi giao (D2-25: `exit_code` vẫn đọc được), `Faulted` khi fault ≠ Exit
//!   chưa được sửa (`TCB` còn `BlockedOnReply`), `Killed` khi `kill`.
//! - [`ProcsrvModel::kill`] — `Tcb.Suspend(Abort)` rồi **Revoke** cap TCB (chỉ mô hình); message
//!   Exit đang chờ (nếu có) bị huỷ nhưng `exit_code` vẫn đọc được (D2-25).
//!
//! Bảng tiến trình `gid -> [cap TCB]` của procsrv thật được mô hình bằng [`ProcsrvModel::track`]
//! (một dòng cho mỗi TCB được theo dõi). **Không** có shell, không có ELF, không có SchedContext
//! cấp cho tiến trình con trong K1c.

use crate::ipc::Message;
use crate::kobj::{FaultKind, KError, Machine, ObjId, SlotRef, SuspendMode, TcbState};

/// `how` của `wait` — thiết kế §6.4 `:591`, xác nhận ở **D2-53 Q10**.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitHow {
    Exited,
    Cancelled,
    Faulted,
    Killed,
}

impl WaitHow {
    pub fn name(self) -> &'static str {
        match self {
            WaitHow::Exited => "Exited",
            WaitHow::Cancelled => "Cancelled",
            WaitHow::Faulted => "Faulted",
            WaitHow::Killed => "Killed",
        }
    }
}

/// Kết quả `wait` — `{code, how}` (thiết kế §6.4 `:591`). `code = 0` với `Faulted`/`Killed`
/// khi không có mã thoát (mô hình `[TK]`; procsrv thật sẽ dùng mã lỗi của nó).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaitResult {
    pub code: u64,
    pub how: WaitHow,
}

#[derive(Debug, Clone, Copy)]
struct ProcEntry {
    tcb: ObjId,
    /// Slot chứa cap TCB (model API nhận `SlotRef`; cap này dùng cho `Revoke` khi `kill`).
    cap: SlotRef,
    /// Kết quả đã chốt (`Exited` khi đã nhận fault Exit, `Killed` khi `kill`).
    outcome: Option<WaitResult>,
}

/// Mô hình procsrv tối giản — một `fault_ep`, một Reply window, bảng tiến trình.
#[derive(Debug)]
pub struct ProcsrvModel {
    self_tcb: ObjId,
    ep: SlotRef,
    reply: SlotRef,
    procs: Vec<ProcEntry>,
    /// Lịch sử fault đã nhận `(tcb, kind)` — `wait` đọc ngược để biết `Faulted`.
    faults: Vec<(ObjId, FaultKind)>,
    /// Nhật ký `recv_exit`: các cặp `(tcb, code)` đã gom được (thứ tự nhận).
    exits: Vec<(ObjId, u64)>,
}

impl ProcsrvModel {
    /// `self_tcb` là TCB của procsrv (đã `Ready`); `ep` là cap `fault_ep` dùng chung cho các
    /// tiến trình con; `reply` là cap Reply cấp cho `Recv` (fault ≠ Exit cần Reply để resume).
    pub fn new(self_tcb: ObjId, ep: SlotRef, reply: SlotRef) -> Self {
        Self {
            self_tcb,
            ep,
            reply,
            procs: Vec::new(),
            faults: Vec::new(),
            exits: Vec::new(),
        }
    }

    /// Đưa một TCB vào bảng tiến trình; `cap` là slot cap TCB trong CSpace của mô hình (dùng
    /// `Revoke` khi `kill`). Gọi lại cho cùng TCB là no-op (idempotent).
    pub fn track(&mut self, tcb: ObjId, cap: SlotRef) {
        if self.procs.iter().any(|p| p.tcb == tcb) {
            return;
        }
        self.procs.push(ProcEntry {
            tcb,
            cap,
            outcome: None,
        });
    }

    /// Các cặp `(tcb, code)` đã gom được từ `recv_exit` (thứ tự nhận).
    pub fn exits(&self) -> &[(ObjId, u64)] {
        &self.exits
    }

    /// Fault gần nhất đã nhận của `tcb` (model-only; dùng cho test/`wait`).
    pub fn last_fault(&self, tcb: ObjId) -> Option<FaultKind> {
        self.faults
            .iter()
            .rev()
            .find(|(t, _)| *t == tcb)
            .map(|(_, k)| *k)
    }

    fn entry(&self, tcb: ObjId) -> Result<&ProcEntry, KError> {
        self.procs
            .iter()
            .find(|p| p.tcb == tcb)
            .ok_or(KError::InvalidArgument)
    }

    fn set_outcome(&mut self, tcb: ObjId, outcome: WaitResult) -> Result<(), KError> {
        let e = self
            .procs
            .iter_mut()
            .find(|p| p.tcb == tcb)
            .ok_or(KError::InvalidArgument)?;
        e.outcome = Some(outcome);
        Ok(())
    }

    /// Xử lý một fault vừa giao cho procsrv: ghi lịch sử; `Exit` ⇒ gom `(tcb, code)` + chốt
    /// `Exited`; fault khác ⇒ để `wait` trả `Faulted` (nếu TCB còn chờ Reply).
    fn handle_fault(
        &mut self,
        tcb: ObjId,
        kind: FaultKind,
    ) -> Result<Option<(ObjId, u64)>, KError> {
        self.faults.push((tcb, kind));
        if let FaultKind::Exit { code } = kind {
            self.exits.push((tcb, code));
            // TCB có thể chưa được `track` (procsrv nhận Exit của tiến trình ngoài bảng) — vẫn
            // ghi nhật ký; `wait` chỉ trả cho TCB đã `track`.
            let _ = self.set_outcome(
                tcb,
                WaitResult {
                    code,
                    how: WaitHow::Exited,
                },
            );
            Ok(Some((tcb, code)))
        } else {
            Ok(None)
        }
    }

    /// `recv_exit()` — `Recv` trên `fault_ep` rồi gom kết quả. Trả `Some((tcb, code))` khi nhận
    /// **fault Exit**, `None` khi nhận fault khác (đã ghi lịch sử) hoặc khi message không phải
    /// fault. Mô hình dùng `Recv` **chặn** như procsrv thật: hàng rỗng ⇒ TCB procsrv
    /// `BlockedOnRecv` (test cần đảm bảo có fault, hoặc để Exit tới đánh thức).
    ///
    /// Fault đã được giao **trước** lời gọi (procsrv đang `Recv` lúc tiến trình fault/exit) được
    /// tiêu thụ qua `take_last_fault` — không gọi `Recv` lần hai.
    pub fn recv_exit(&mut self, m: &mut Machine) -> Result<Option<(ObjId, u64)>, KError> {
        if let Some((tcb, kind)) = m.take_last_fault(self.self_tcb)? {
            return self.handle_fault(tcb, kind);
        }
        m.recv(self.self_tcb, self.ep, Some(self.reply))?;
        if let Some((tcb, kind)) = m.take_last_fault(self.self_tcb)? {
            return self.handle_fault(tcb, kind);
        }
        Ok(None)
    }

    /// `wait(tcb)` → `{code, how}` (thiết kế §6.4). `Busy` khi tiến trình **chưa** kết thúc
    /// (còn chạy/đang chờ Exit chưa được `Recv`/fault đã được sửa và chạy tiếp).
    pub fn wait(&self, m: &Machine, tcb: ObjId) -> Result<WaitResult, KError> {
        let e = self.entry(tcb)?;
        if let Some(outcome) = e.outcome {
            return Ok(outcome);
        }
        let snap = m.tcb_read_state(e.cap)?;
        if let Some(code) = snap.exit_code {
            return match snap.state {
                // `Exit` đã giao xong (hoặc exit không `fault_ep` ⇒ Inactive ngay) — D2-25.
                TcbState::Inactive => Ok(WaitResult {
                    code,
                    how: WaitHow::Exited,
                }),
                // Còn `BlockedOnSend` trên `fault_ep`: procsrv **chưa** `Recv` ⇒ chưa xong.
                TcbState::BlockedOnSend { .. } => Err(KError::Busy),
                // `Suspend(Abort)` huỷ message Exit trước khi giao nhưng **giữ** `exit_code`.
                _ if snap.suspended => Ok(WaitResult {
                    code,
                    how: WaitHow::Cancelled,
                }),
                _ => Err(KError::Busy),
            };
        }
        if let Some(kind) = self.last_fault(tcb) {
            if !kind.is_exit() && matches!(m.tcb_state(tcb)?, TcbState::BlockedOnReply { .. }) {
                return Ok(WaitResult {
                    code: 0,
                    how: WaitHow::Faulted,
                });
            }
        }
        Err(KError::Busy)
    }

    /// `kill(tcb)` = `Tcb.Suspend(Abort)` + **Revoke** cap TCB (thiết kế §6.4; D2-53 Q10 — chỉ
    /// mô hình). Ghi `how = Killed`; message Exit đang chờ bị huỷ nhưng `exit_code` giữ nguyên
    /// (D2-25).
    pub fn kill(&mut self, m: &mut Machine, tcb: ObjId) -> Result<(), KError> {
        let e = *self.entry(tcb)?;
        m.tcb_suspend(e.cap, SuspendMode::Abort)?;
        // `Revoke` trên cap TCB: xoá các cap **dẫn xuất** (không finalise object) — đúng câu chữ
        // "Revoke cap TCB" của §6.4; `CNode.Delete` + finalise thuộc K3.5.
        m.revoke(e.cap.cnode, Some(e.cap.slot))?;
        let code = m
            .tcb_read_state(e.cap)
            .map(|s| s.exit_code.unwrap_or(0))
            .unwrap_or(0);
        self.set_outcome(
            tcb,
            WaitResult {
                code,
                how: WaitHow::Killed,
            },
        )
    }

    /// Cho tiến trình đang bị fault chạy tiếp bằng **thanh ghi mới** — gọi
    /// [`Machine::tcb_resume_with_regs`] (fault-resume một cửa, không phát lại IPC cũ).
    pub fn resume_fault(
        &mut self,
        m: &mut Machine,
        tcb: ObjId,
        regs: Message,
    ) -> Result<(), KError> {
        let e = *self.entry(tcb)?;
        m.tcb_resume_with_regs(e.cap, regs)
    }
}
