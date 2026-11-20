#!/usr/bin/env bash
# KT-252 F2 — `#[path = "src/../src/evil.rs"]` (chuẩn hoá `..`).
chen() {
    printf 'pub fn kt252_f2(m: &mut crate::machine::Machine, t: crate::error::ObjId) { let _ = m.wake(t); }\n' \
        >v2/kernel/vifast-core/src/kt252_f2.rs
    cat >>v2/kernel/vifast-core/src/lib.rs <<'RS'

// KT-252 F2 — `#[path]` có `..` chuẩn hoá về trong cây.
#[path = "src/../src/kt252_f2.rs"]
pub mod kt252_f2;
RS
}
go() { :; }
