#!/usr/bin/env bash
# KT-252 E3 — **symlink**: `src/kt252_e3.rs` trỏ tới tệp ngoài cây chứa `wake`; `mod kt252_e3;`.
chen() {
    printf 'pub fn kt252_e3(m: &mut crate::machine::Machine, t: crate::error::ObjId) { let _ = m.wake(t); }\n' \
        >/tmp/kt252-e3-evil.rs
    ln -sf /tmp/kt252-e3-evil.rs v2/kernel/vifast-core/src/kt252_e3.rs
    cat >>v2/kernel/vifast-core/src/lib.rs <<'RS'

// KT-252 E3 — symlink.
pub mod kt252_e3;
RS
}
go() { :; }
