// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-232 — dạng 1: `let tt = Machine::transition_tcb; tt(m, t, TcbState::Ready);`
use crate::{KError, Machine, ObjId, TcbState};

pub fn kt232_f1(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    let tt = Machine::transition_tcb;
    tt(m, t, TcbState::Ready)
}
