// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-232 — dạng 6: alias **kiểu** `Machine as M` + gọi UFCS `M::transition_tcb(m, t, Ready)`.
use crate::Machine as M;
use crate::{KError, ObjId, TcbState};

pub fn kt232_f6(m: &mut M, t: ObjId) -> Result<(), KError> {
    M::transition_tcb(m, t, TcbState::Ready)
}
