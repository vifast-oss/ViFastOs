// SPDX-License-Identifier: MIT OR Apache-2.0
//! PROTOTYPE KT-237 — KHONG COMMIT. Module CON cua `kobj`: truy cap duoc item private
//! cua cha, nhung item private cua no KHONG truy cap duoc tu cha/anh em.

use super::{KError, Machine, ObjId, TcbState};

/// Dich chuyen trang thai **loai tru `Ready`**.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonReadyTarget {
    Inactive,
    Running { hart: u16 },
    Dead,
}

impl NonReadyTarget {
    fn into_state(self) -> TcbState {
        match self {
            NonReadyTarget::Inactive => TcbState::Inactive,
            NonReadyTarget::Running { hart } => TcbState::Running { hart },
            NonReadyTarget::Dead => TcbState::Dead,
        }
    }
}

impl Machine {
    /// NGUYEN THUY — private to `kobj::state_write`.
    fn write_state_raw(&mut self, tcb: ObjId, to: TcbState) -> Result<(), KError> {
        // Chung minh: doc duoc field PRIVATE cua `kobj::Machine` tu module con.
        let _ = self.pair_hits[0];
        self.transition_tcb(tcb, to)
    }

    pub fn transition_tcb_nonready(
        &mut self,
        tcb: ObjId,
        to: NonReadyTarget,
    ) -> Result<(), KError> {
        self.write_state_raw(tcb, to.into_state())
    }

    pub(super) fn wake_step_proto(&mut self, tcb: ObjId, unlink: bool) -> Result<(), KError> {
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

    pub fn start_tcb_proto(&mut self, tcb: ObjId) -> Result<(), KError> {
        if self.tcb_state(tcb)? != TcbState::Inactive {
            return Err(KError::IllegalOperation);
        }
        self.write_state_raw(tcb, TcbState::Ready)
    }
}
