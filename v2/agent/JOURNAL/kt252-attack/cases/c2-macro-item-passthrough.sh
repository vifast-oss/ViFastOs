#!/usr/bin/env bash
# KT-252 C2 — `macro_rules! pass { ($i:item) => { $i } }` + `pass!(fn wake_step … write_state)`.
chen() {
    cat >>v2/kernel/vifast-core/src/state_write.rs <<'RS'

// KT-252 C2 — passthrough `:item`.
/// KT-252 C2.
#[macro_export]
macro_rules! kt252_pass {
    ($i:item) => {
        $i
    };
}
kt252_pass!(
    /// Sinh bởi macro passthrough.
    pub fn wake_step(m: &mut Machine, tcb: ObjId) -> Result<(), KError> {
        m.write_state(tcb, TcbState::Ready)
    }
);
RS
}
go() { :; }
