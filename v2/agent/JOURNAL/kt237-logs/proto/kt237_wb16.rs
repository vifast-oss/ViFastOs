// SPDX-License-Identifier: MIT OR Apache-2.0
//! PROTOTYPE KT-237 — 6 dang WB-16 tren API moi. PHAI khong bien dich.
use crate::{KError, Machine, ObjId, TcbState};

fn helper(m: &mut Machine, t: ObjId, s: TcbState) -> Result<(), KError> {
    m.transition_tcb_nonready(t, s)
}
pub fn f1(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    helper(m, t, TcbState::Ready)
}
pub fn f2(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    crate::kt237_wb16_helper::helper(m, t, TcbState::Ready)
}
fn ready_state() -> TcbState {
    TcbState::Ready
}
pub fn f3(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    m.transition_tcb_nonready(t, ready_state())
}
const R: TcbState = TcbState::Ready;
pub fn f4(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    m.transition_tcb_nonready(t, R)
}
static S: TcbState = TcbState::Ready;
pub fn f5(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    m.transition_tcb_nonready(t, S)
}
pub fn f6(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    (|s: TcbState| m.transition_tcb_nonready(t, s))(TcbState::Ready)
}
