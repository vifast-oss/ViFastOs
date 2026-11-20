// SPDX-License-Identifier: MIT OR Apache-2.0
//! PROTOTYPE KT-237 — KHONG COMMIT. Mo phong ranh gioi `state_write` + kieu `NonReadyTarget`.

use crate::{FaultKind, KError, Machine, ObjId, SleepReason, TcbState};

/// Dich chuyen trang thai **loai tru `Ready`**.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonReadyTarget {
    Inactive,
    Running { hart: u16 },
    BlockedOnSend { ep: ObjId, call: bool, fault: Option<FaultKind> },
    BlockedOnRecv { ep: ObjId, reply: Option<ObjId> },
    BlockedOnReply { reply: ObjId },
    BlockedOnNotification { ntfn: ObjId },
    BlockedOnAddress { frame: ObjId, offset: u32 },
    Sleeping { reason: SleepReason },
    Stopping,
    Dead,
}

impl NonReadyTarget {
    fn into_state(self) -> TcbState {
        match self {
            NonReadyTarget::Inactive => TcbState::Inactive,
            NonReadyTarget::Running { hart } => TcbState::Running { hart },
            NonReadyTarget::BlockedOnSend { ep, call, fault } => {
                TcbState::BlockedOnSend { ep, call, fault }
            }
            NonReadyTarget::BlockedOnRecv { ep, reply } => TcbState::BlockedOnRecv { ep, reply },
            NonReadyTarget::BlockedOnReply { reply } => TcbState::BlockedOnReply { reply },
            NonReadyTarget::BlockedOnNotification { ntfn } => {
                TcbState::BlockedOnNotification { ntfn }
            }
            NonReadyTarget::BlockedOnAddress { frame, offset } => {
                TcbState::BlockedOnAddress { frame, offset }
            }
            NonReadyTarget::Sleeping { reason } => TcbState::Sleeping { reason },
            NonReadyTarget::Stopping => TcbState::Stopping,
            NonReadyTarget::Dead => TcbState::Dead,
        }
    }
}

impl Machine {
    /// NGUYEN THUY — private to `state_write`; khong `pub`, khong `pub(crate)`.
    fn write_state_raw(&mut self, tcb: ObjId, to: TcbState) -> Result<(), KError> {
        self.transition_tcb(tcb, to)
    }

    /// Cua cong khai duy nhat cho chuyen trang thai KHONG-Ready.
    pub fn transition_tcb_nonready(
        &mut self,
        tcb: ObjId,
        to: NonReadyTarget,
    ) -> Result<(), KError> {
        self.write_state_raw(tcb, to.into_state())
    }

    // ------------------------------------------------ 5 cua `Ready` co ngu nghia

    /// wake: blocked/sleeping -> Ready, go hang theo `unlink`.
    pub(crate) fn wake_step_proto(&mut self, tcb: ObjId, unlink: bool) -> Result<(), KError> {
        let state = self.tcb_state(tcb)?;
        if state == TcbState::Ready {
            return Ok(());
        }
        if !(state.is_blocked() || matches!(state, TcbState::Sleeping { .. })) {
            return Err(KError::IllegalOperation);
        }
        if unlink {
            self.unlink_tcb_queues(tcb);
        }
        self.write_state_raw(tcb, TcbState::Ready)
    }

    /// preemption: TCB dang `Running{hart}` bi chiem cho -> Ready.
    pub(crate) fn preempt_current_proto(&mut self, tcb: ObjId) -> Result<(), KError> {
        self.write_state_raw(tcb, TcbState::Ready)
    }

    /// yield: TCB tu nhuong `Running -> Ready`.
    pub(crate) fn yield_to_ready_proto(&mut self, tcb: ObjId) -> Result<(), KError> {
        self.write_state_raw(tcb, TcbState::Ready)
    }

    /// start/init: `Inactive -> Ready`.
    pub fn start_tcb_proto(&mut self, tcb: ObjId) -> Result<(), KError> {
        if self.tcb_state(tcb)? != TcbState::Inactive {
            return Err(KError::IllegalOperation);
        }
        self.write_state_raw(tcb, TcbState::Ready)
    }

    /// suspend/abort: `Blocked*`/`Running -> Ready` (co `suspended` do caller dat).
    pub(crate) fn suspend_to_ready_proto(&mut self, tcb: ObjId) -> Result<(), KError> {
        self.write_state_raw(tcb, TcbState::Ready)
    }
}
