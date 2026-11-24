// SPDX-License-Identifier: MIT OR Apache-2.0
//! PROTOTYPE KT-237 — DOI CHUNG DUONG: API moi dung cach PHAI bien dich.
use crate::kobj::{KError, Machine, NonReadyTarget, ObjId};

pub fn ok_nonready(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    m.transition_tcb_nonready(t, NonReadyTarget::Inactive)
}
pub fn ok_start(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    m.start_tcb_proto(t)
}
