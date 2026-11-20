// SPDX-License-Identifier: MIT OR Apache-2.0
//! PROTOTYPE KT-237 — helper o TE KHAC cho WB-16 f2.
use crate::{KError, Machine, ObjId, TcbState};

pub(crate) fn helper(m: &mut Machine, t: ObjId, s: TcbState) -> Result<(), KError> {
    m.transition_tcb_nonready(t, s)
}
