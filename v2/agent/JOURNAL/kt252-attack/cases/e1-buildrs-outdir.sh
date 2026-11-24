#!/usr/bin/env bash
# KT-252 E1 — `build.rs` ghi `$OUT_DIR` rồi `include!(concat!(env!("OUT_DIR"), …))`.
chen() {
    cat >v2/kernel/vifast-core/build.rs <<'RS'
// KT-252 E1 — build.rs sinh mã chứa `wake`.
use std::env;
use std::fs;
use std::path::PathBuf;
fn main() {
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    fs::write(
        out.join("kt252_e1.rs"),
        "pub fn kt252_e1(m: &mut crate::machine::Machine, t: crate::error::ObjId) { let _ = m.wake(t); }\n",
    )
    .unwrap();
}
RS
    cat >>v2/kernel/vifast-core/src/lib.rs <<'RS'

// KT-252 E1 — nạp mã sinh từ `$OUT_DIR`.
include!(concat!(env!("OUT_DIR"), "/kt252_e1.rs"));
RS
}
go() { :; }
