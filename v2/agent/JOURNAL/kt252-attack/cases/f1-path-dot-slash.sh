#!/usr/bin/env bash
# KT-252 F1 — `#[path = "./evil.rs"]` (đích TRONG cây, chứa `wake`).
chen() {
    printf 'pub fn kt252_f1(m: &mut crate::machine::Machine, t: crate::error::ObjId) { let _ = m.wake(t); }\n' \
        >v2/kernel/vifast-core/src/kt252_f1.rs
    cat >>v2/kernel/vifast-core/src/lib.rs <<'RS'

// KT-252 F1 — `#[path]` có `./`.
#[path = "./kt252_f1.rs"]
pub mod kt252_f1;
RS
}
go() { :; }
