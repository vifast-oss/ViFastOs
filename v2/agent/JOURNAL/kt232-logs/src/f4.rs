// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-232 — dạng 4: closure gọi thẳng `m.transition_tcb(t, TcbState::Ready)`.
use crate::{KError, Machine, ObjId, TcbState};

pub fn kt232_f4(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    let f = |m: &mut Machine, t: ObjId| m.transition_tcb(t, TcbState::Ready);
    f(m, t)
}
