// SPDX-License-Identifier: MIT OR Apache-2.0
//! PROTOTYPE KT-237 — 7 dang WB-17 tren API moi. PHAI khong bien dich.
use crate::{KError, Machine, ObjId, TcbState};

pub fn f1(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    let tt = Machine::transition_tcb_nonready;
    tt(m, t, TcbState::Ready)
}
pub fn f2(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    let tt: fn(&mut Machine, ObjId, TcbState) -> Result<(), KError> =
        Machine::transition_tcb_nonready;
    tt(m, t, TcbState::Ready)
}
fn call(f: fn(&mut Machine, ObjId, TcbState) -> Result<(), KError>, m: &mut Machine, t: ObjId) -> Result<(), KError> {
    f(m, t, TcbState::Ready)
}
pub fn f3(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    call(Machine::transition_tcb_nonready, m, t)
}
pub fn f5a<F: Fn(&mut Machine, ObjId, TcbState) -> Result<(), KError>>(f: F, m: &mut Machine, t: ObjId) -> Result<(), KError> {
    f(m, t, TcbState::Ready)
}
pub fn f5a_call(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    f5a(Machine::transition_tcb_nonready, m, t)
}
pub fn f5b(f: &dyn Fn(&mut Machine, ObjId, TcbState) -> Result<(), KError>, m: &mut Machine, t: ObjId) -> Result<(), KError> {
    f(m, t, TcbState::Ready)
}
pub fn f6(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    Machine::transition_tcb_nonready(m, t, TcbState::Ready)
}
pub fn f7(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    let f = <Machine>::transition_tcb_nonready;
    f(m, t, TcbState::Ready)
}
pub fn f5b_call(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    f5b(&Machine::transition_tcb_nonready, m, t)
}
