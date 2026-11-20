#!/usr/bin/env bash
# KT-252 A7 — trùng tên qua **trait impl**: `impl Kt252Trait for Kt252Door { fn wake_step }`.
chen() {
    cat >>v2/kernel/vifast-core/src/state_write.rs <<'RS'

// KT-252 A7 — "cửa thứ bảy" qua trùng tên trong **trait impl**.
pub struct Kt252Door;
/// Trait của kẻ tấn công.
pub trait Kt252Trait {
    /// Cùng TÊN `wake_step`.
    fn wake_step(m: &mut Machine, tcb: ObjId) -> Result<(), KError>;
}
impl Kt252Trait for Kt252Door {
    fn wake_step(m: &mut Machine, tcb: ObjId) -> Result<(), KError> {
        m.write_state(tcb, TcbState::Ready)
    }
}
RS
    cat >>v2/kernel/vifast-core/src/core_ipc.rs <<'RS'

/// KT-252 A7 — mã sản phẩm gọi "cửa thứ bảy" (trait impl).
pub fn kt252_a7(m: &mut crate::machine::Machine, t: crate::error::ObjId) {
    let _ = <crate::state_write::Kt252Door as crate::state_write::Kt252Trait>::wake_step(m, t);
}
RS
}
go() { :; }
