// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-229 B2 đối chứng âm — BỎ binding cục bộ `wake`.
use crate::{Machine, ObjId};

pub fn kt229_b2(m: &mut Machine, t: ObjId) {
    let _ = vifast_abi::matches!(m, wake, t);
}
