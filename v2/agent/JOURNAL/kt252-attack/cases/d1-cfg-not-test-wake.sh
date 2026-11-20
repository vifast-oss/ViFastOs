#!/usr/bin/env bash
# KT-252 D1 — `#[cfg(not(test))]` trên hàm gọi `wake` (nhánh **sản phẩm**).
chen() {
    cat >>v2/kernel/vifast-core/src/core_ipc.rs <<'RS'

// KT-252 D1 — `cfg(not(test))` = sản phẩm.
/// KT-252 D1.
#[cfg(not(test))]
pub fn kt252_d1(m: &mut crate::machine::Machine, t: crate::error::ObjId) {
    let _ = m.wake(t);
}
RS
}
go() { :; }
