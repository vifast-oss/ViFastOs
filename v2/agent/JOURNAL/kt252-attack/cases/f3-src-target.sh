#!/usr/bin/env bash
# KT-252 F3 — thư mục tên `target` trong `src/` (WB-12) + `#[path]`.
chen() {
    mkdir -p v2/kernel/vifast-core/src/target
    printf 'pub fn kt252_f3(m: &mut crate::machine::Machine, t: crate::error::ObjId) { let _ = m.wake(t); }\n' \
        >v2/kernel/vifast-core/src/target/kt252_f3.rs
    cat >>v2/kernel/vifast-core/src/lib.rs <<'RS'

// KT-252 F3 — thư mục tên `target`.
#[path = "target/kt252_f3.rs"]
pub mod kt252_f3;
RS
}
go() { :; }
