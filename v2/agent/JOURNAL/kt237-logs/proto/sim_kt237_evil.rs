// SPDX-License-Identifier: MIT OR Apache-2.0
//! PROTOTYPE KT-237 — module san pham GIA (anh em cua `kobj`). PHAI khong bien dich.
use crate::kobj::{KError, Machine, ObjId, TcbState};

pub fn evil(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    m.write_state_raw(t, TcbState::Ready)
}
