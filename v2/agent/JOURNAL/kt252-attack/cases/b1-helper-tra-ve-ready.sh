#!/usr/bin/env bash
# KT-252 B1 — helper trả về `TcbState::Ready` rồi đưa vào `transition_tcb`.
chen() {
    cat >>v2/kernel/vifast-core/src/core_ipc.rs <<'RS'

/// KT-252 B1 — helper trả `Ready`; API mới (`NonReadyTarget`) phải chặn ở **kiểu**.
fn kt252_b1_ready() -> crate::tcb::TcbState {
    crate::tcb::TcbState::Ready
}
/// KT-252 B1 — điểm gọi.
pub fn kt252_b1(m: &mut crate::machine::Machine, t: crate::error::ObjId) -> Result<(), crate::error::KError> {
    m.transition_tcb(t, kt252_b1_ready())
}
RS
}
go() { :; }
