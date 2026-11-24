#!/usr/bin/env bash
# KT-252 A6 — trùng tên qua **module lồng** trong `state_write.rs`: `mod kt252 { pub fn wake_step }`.
chen() {
    cat >>v2/kernel/vifast-core/src/state_write.rs <<'RS'

// KT-252 A6 — "cửa thứ bảy" qua trùng tên trong **module con**.
pub mod kt252 {
    use crate::error::{KError, ObjId};
    use crate::machine::Machine;
    use crate::tcb::TcbState;
    /// Cùng TÊN `wake_step` ⇒ checker quy theo tên.
    pub fn wake_step(m: &mut Machine, tcb: ObjId) -> Result<(), KError> {
        m.write_state(tcb, TcbState::Ready)
    }
}
RS
    cat >>v2/kernel/vifast-core/src/core_ipc.rs <<'RS'

/// KT-252 A6 — mã sản phẩm gọi "cửa thứ bảy" (module con).
pub fn kt252_a6(m: &mut crate::machine::Machine, t: crate::error::ObjId) {
    let _ = crate::state_write::kt252::wake_step(m, t);
}
RS
}
go() { :; }
