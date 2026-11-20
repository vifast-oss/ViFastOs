#!/usr/bin/env bash
# KT-252 C1 — `macro_rules!` trong `state_write.rs` mở rộng thành `fn wake_step` (free fn) gọi
# `write_state`; gọi macro để nó vào cây.
chen() {
    cat >>v2/kernel/vifast-core/src/state_write.rs <<'RS'

// KT-252 C1 — macro sinh ra "cửa" trùng tên.
/// KT-252 C1.
#[macro_export]
macro_rules! kt252_mkdoor {
    () => {
        /// Sinh bởi macro.
        pub fn wake_step(m: &mut Machine, tcb: ObjId) -> Result<(), KError> {
            m.write_state(tcb, TcbState::Ready)
        }
    };
}
kt252_mkdoor!();
RS
    cat >>v2/kernel/vifast-core/src/core_ipc.rs <<'RS'

/// KT-252 C1 — điểm gọi.
pub fn kt252_c1(m: &mut crate::machine::Machine, t: crate::error::ObjId) {
    let _ = crate::state_write::wake_step(m, t);
}
RS
}
go() { :; }
