#!/usr/bin/env bash
# KT-252 B2 — helper **mức module** trong `state_write.rs` gọi `write_state` (không phải cửa).
chen() {
    cat >>v2/kernel/vifast-core/src/state_write.rs <<'RS'

// KT-252 B2 — helper mức module (tên KHÔNG thuộc sáu cửa).
/// KT-252 B2 — ghi `Ready` từ hàm không phải cửa.
pub fn kt252_b2_helper(m: &mut Machine, tcb: ObjId) -> Result<(), KError> {
    m.write_state(tcb, TcbState::Ready)
}
RS
}
go() { :; }
