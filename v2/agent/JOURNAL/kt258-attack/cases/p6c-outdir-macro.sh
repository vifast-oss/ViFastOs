# SPDX-License-Identifier: MIT OR Apache-2.0
# P6c — `include!(concat!(env!("OUT_DIR"), …))` nằm trong **thân `macro_rules!`** ⇒ phải bị từ
# chối riêng (`E_STRICT_MACRO_INCLUDE`), không phụ thuộc đích.
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
    cat >> v2/kernel/vifast-core/src/lib.rs <<'EOF'

macro_rules! kt258_gen {
    () => {
        include!(concat!(env!("OUT_DIR"), "/evil.rs"));
    };
}
kt258_gen!();
EOF
}
