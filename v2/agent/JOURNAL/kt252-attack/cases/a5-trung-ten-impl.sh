#!/usr/bin/env bash
# KT-252 A5 — "cửa thứ bảy" qua **TRÙNG TÊN**: `impl Kt252Door { fn wake_step }` (cùng tên cửa
# thật, khác kiểu) trong `state_write.rs`; mã sản phẩm gọi nó. KT-242 vá **hàm lồng**, không vá
# **trùng tên** ⇒ `_innermost_fn_tree` trả `wake_step` ⇒ thuộc `STATE_WRITE_CALLERS`.
chen() {
    cat >>v2/kernel/vifast-core/src/state_write.rs <<'RS'

// KT-252 A5 — "cửa thứ bảy" qua **TRÙNG TÊN** (không lồng trong thân cửa).
/// KT-252 A5 — ghi `Ready` **không** qua điều kiện nguồn nào.
pub struct Kt252Door;
impl Kt252Door {
    /// Cùng TÊN `wake_step` với cửa thật ⇒ checker quy theo **tên**, không theo kiểu.
    pub fn wake_step(m: &mut Machine, tcb: ObjId) -> Result<(), KError> {
        m.write_state(tcb, TcbState::Ready)
    }
}
RS
    cat >>v2/kernel/vifast-core/src/core_ipc.rs <<'RS'

/// KT-252 A5 — mã sản phẩm gọi "cửa thứ bảy" (đường lọt).
pub fn kt252_a5(m: &mut crate::machine::Machine, t: crate::error::ObjId) {
    let _ = crate::state_write::Kt252Door::wake_step(m, t);
}
RS
}
go() { :; }
