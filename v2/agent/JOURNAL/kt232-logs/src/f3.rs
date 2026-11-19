// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-232 — dạng 3: truyền fn-pointer qua tham số hàm trợ giúp.
use crate::{KError, Machine, ObjId, TcbState};

fn call(
    f: fn(&mut Machine, ObjId, TcbState) -> Result<(), KError>,
    m: &mut Machine,
    t: ObjId,
) -> Result<(), KError> {
    f(m, t, TcbState::Ready)
}

pub fn kt232_f3(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    call(Machine::transition_tcb, m, t)
}
