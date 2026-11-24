chen() {
  python3 - <<'PY'
from pathlib import Path
p = Path("v2/kernel/vifast-core/src/state_write.rs")
t = p.read_text(encoding="utf-8")
old = """    pub(crate) fn wake_step(&mut self, tcb: ObjId, unlink: bool) -> Result<(), KError> {
        let state = self.tcb_state(tcb)?;"""
new = """    pub(crate) fn wake_step(&mut self, tcb: ObjId, unlink: bool) -> Result<(), KError> {
        #[allow(non_local_definitions)]
        impl Machine {
            /// KT-242 R3-a — cửa thứ bảy: gọi thẳng `write_state`, bỏ mọi điều kiện nguồn.
            pub fn kt242_door(&mut self, tcb: ObjId) -> Result<(), KError> {
                self.write_state(tcb, TcbState::Ready)
            }
        }
        let state = self.tcb_state(tcb)?;"""
assert old in t, "khong tim thay wake_step"
p.write_text(t.replace(old, new, 1), encoding="utf-8")
PY
  cat > v2/kernel/vifast-core/src/kt242_r3.rs <<'RS'
// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-242 R3-a — mã sản phẩm gọi "cửa thứ bảy" để đưa TCB về `Ready` không qua điều kiện nào.

use crate::error::{KError, ObjId};
use crate::machine::Machine;

/// Đường sản phẩm: đưa TCB bất kỳ về `Ready` mà không kiểm điều kiện nguồn.
pub fn kt242_r3(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    m.kt242_door(t)
}
RS
  printf '\nmod kt242_r3;\n' >> v2/kernel/vifast-core/src/lib.rs
}
go() { :; }
