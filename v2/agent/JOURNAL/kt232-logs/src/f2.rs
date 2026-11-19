// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-232 — dạng 2: fn-pointer có kiểu tường minh.
use crate::{KError, Machine, ObjId, TcbState};

pub fn kt232_f2(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    let tt: fn(&mut Machine, ObjId, TcbState) -> Result<(), KError> = Machine::transition_tcb;
    tt(m, t, TcbState::Ready)
}
