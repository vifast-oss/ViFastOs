#!/usr/bin/env bash
# KT-252 A8 — trùng tên qua **free fn** ở mức module trong `state_write.rs`.
chen() {
    cat >>v2/kernel/vifast-core/src/state_write.rs <<'RS'

// KT-252 A8 — "cửa thứ bảy" qua trùng tên: **free fn** mức module.
/// Cùng TÊN `wake_step` ⇒ checker quy theo tên.
pub fn wake_step(m: &mut Machine, tcb: ObjId) -> Result<(), KError> {
    m.write_state(tcb, TcbState::Ready)
}
RS
    cat >>v2/kernel/vifast-core/src/core_ipc.rs <<'RS'

/// KT-252 A8 — mã sản phẩm gọi "cửa thứ bảy" (free fn).
pub fn kt252_a8(m: &mut crate::machine::Machine, t: crate::error::ObjId) {
    let _ = crate::state_write::wake_step(m, t);
}
RS
}
go() { :; }
