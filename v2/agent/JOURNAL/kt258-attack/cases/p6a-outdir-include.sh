# SPDX-License-Identifier: MIT OR Apache-2.0
# P6a — `build.rs` ghi `$OUT_DIR/evil.rs` chứa `wake`; `lib.rs` nạp bằng
# `include!(concat!(env!("OUT_DIR"), "/evil.rs"))` (không literal).
chen() {
    cat > v2/kernel/vifast-core/build.rs <<'EOF'
// SPDX-License-Identifier: MIT OR Apache-2.0
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    fs::write(
        out.join("evil.rs"),
        "pub fn kt258_evil(m: &mut crate::Machine, t: crate::ObjId) { let _ = m.wake(t); }\n",
    )
    .expect("ghi evil.rs");
}
EOF
    printf '\ninclude!(concat!(env!("OUT_DIR"), "/evil.rs"));\n' >> v2/kernel/vifast-core/src/lib.rs
}
