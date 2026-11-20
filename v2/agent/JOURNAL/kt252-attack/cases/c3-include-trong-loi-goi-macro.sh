#!/usr/bin/env bash
# KT-252 C3 — `include!` trong **đối số lời gọi macro** ở mã sản phẩm.
chen() {
    printf 'pub fn kt252_c3(m: &mut crate::machine::Machine, t: crate::error::ObjId) { let _ = m.wake(t); }\n' \
        >v2/kernel/vifast-core/src/kt252_evil_inc.rs
    cat >>v2/kernel/vifast-core/src/core_ipc.rs <<'RS'

// KT-252 C3 — `include!` trong đối số lời gọi macro.
macro_rules! kt252_ident {
    ($x:expr) => {
        $x
    };
}
kt252_ident! {
    include!("kt252_evil_inc.rs");
}
RS
}
go() { :; }
