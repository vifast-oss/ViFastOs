// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-229 A2 đối chứng âm — dùng thẳng `TcbState::Ready`.
use crate::{Machine, ObjId, TcbState};

pub fn kt229_a2(m: &mut Machine, t: ObjId) {
    let _ = m.transition_tcb(t, TcbState::Ready);
}
