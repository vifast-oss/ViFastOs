// SPDX-License-Identifier: MIT OR Apache-2.0
//! PROTOTYPE KT-237 — module san pham GIA goi nguyen thuy tho. PHAI khong bien dich.
use crate::{KError, Machine, ObjId, TcbState};

pub fn evil(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    m.write_state_raw(t, TcbState::Ready)
}
