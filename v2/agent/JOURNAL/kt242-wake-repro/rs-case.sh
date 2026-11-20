chen() {
  cat > v2/kernel/vifast-core/src/kt242_rs.RS <<'RS'
// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-242 — tệp `.RS` (hoa) được `mod` nạp trên FS không phân biệt hoa/thường (macOS).

use crate::error::ObjId;
use crate::machine::Machine;

/// Đường sản phẩm gọi `wake()`.
pub fn kt242_rs(m: &mut Machine, t: ObjId) {
    let _ = m.wake(t);
}
RS
  printf '\nmod kt242_rs;\n' >> v2/kernel/vifast-core/src/lib.rs
}
go() { :; }
