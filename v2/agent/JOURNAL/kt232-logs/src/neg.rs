// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-232 — đối chứng âm: gọi thẳng `m.transition_tcb(t, TcbState::Ready)`.
use crate::{KError, Machine, ObjId, TcbState};

pub fn kt232_neg(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    m.transition_tcb(t, TcbState::Ready)
}
