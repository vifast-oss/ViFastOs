#!/usr/bin/env bash
# KT-252 D3 — `#[cfg_attr(not(test), path = "…")]` trỏ ra ngoài cây.
chen() {
    printf 'pub fn kt252_d3(m: &mut crate::machine::Machine, t: crate::error::ObjId) { let _ = m.wake(t); }\n' \
        >/tmp/kt252-d3-evil.rs
    cat >>v2/kernel/vifast-core/src/lib.rs <<'RS'

// KT-252 D3 — `cfg_attr` lồng trỏ ra ngoài cây.
#[cfg_attr(not(test), path = "/tmp/kt252-d3-evil.rs")]
pub mod kt252_d3;
RS
}
go() { :; }
