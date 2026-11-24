chen() {
  cat > v2/kernel/vifast-core/src/kt242_a1.rs <<'RS'
// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-242 A1 — repro: `wake` lấy làm **giá trị** (`self.wake`) + binding cục bộ `wake` làm mồi.

use crate::error::ObjId;
use crate::machine::Machine;

impl Machine {
    /// Đường sản phẩm gọi `wake()` qua method-reference.
    pub fn kt242_a1(&mut self, t: ObjId) {
        let wake = 0u32;
        let _ = wake;
        let f = self.wake;
        let _ = f(t);
    }
}
RS
  printf '\nmod kt242_a1;\n' >> v2/kernel/vifast-core/src/lib.rs
}
go() { :; }
