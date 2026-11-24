// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-232 — dạng 7: `let f = <Machine>::transition_tcb;`
use crate::{KError, Machine, ObjId, TcbState};

pub fn kt232_f7(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    let f = <Machine>::transition_tcb;
    f(m, t, TcbState::Ready)
}
