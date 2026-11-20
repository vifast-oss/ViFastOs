#!/usr/bin/env bash
# KT-252 E2 — `include!("…txt")` (đuôi lạ) chứa `wake`.
chen() {
    printf 'pub fn kt252_e2(m: &mut crate::machine::Machine, t: crate::error::ObjId) { let _ = m.wake(t); }\n' \
        >v2/kernel/vifast-core/src/kt252_e2.txt
    cat >>v2/kernel/vifast-core/src/lib.rs <<'RS'

// KT-252 E2 — đuôi lạ.
include!("kt252_e2.txt");
RS
}
go() { :; }
