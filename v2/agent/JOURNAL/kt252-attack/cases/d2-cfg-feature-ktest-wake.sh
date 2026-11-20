#!/usr/bin/env bash
# KT-252 D2 — `#[cfg(feature = "ktest")]` trên hàm gọi `wake` (hợp đồng: `feature="ktest"` là
# **sản phẩm**, fail-closed — chỉ `test`/`all(test)` là test).
chen() {
    cat >>v2/kernel/vifast-core/src/core_ipc.rs <<'RS'

// KT-252 D2 — `feature="ktest"` phải là SẢN PHẨM (fail-closed).
/// KT-252 D2.
#[cfg(feature = "ktest")]
pub fn kt252_d2(m: &mut crate::machine::Machine, t: crate::error::ObjId) {
    let _ = m.wake(t);
}
RS
}
go() { :; }
