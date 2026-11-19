// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-232 — dạng 5: generic `F: Fn(...)` + trait object `&dyn Fn(...)`.
use crate::{KError, Machine, ObjId, TcbState};

fn call_generic<F>(f: F, m: &mut Machine, t: ObjId) -> Result<(), KError>
where
    F: Fn(&mut Machine, ObjId, TcbState) -> Result<(), KError>,
{
    f(m, t, TcbState::Ready)
}

pub fn kt232_f5(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    call_generic(Machine::transition_tcb, m, t)
}

pub fn kt232_f5b(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    let f: fn(&mut Machine, ObjId, TcbState) -> Result<(), KError> = Machine::transition_tcb;
    let g: &dyn Fn(&mut Machine, ObjId, TcbState) -> Result<(), KError> = &f;
    g(m, t, TcbState::Ready)
}
