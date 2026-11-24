# SPDX-License-Identifier: MIT OR Apache-2.0
# P7b — `#[path]` tới tệp **không thuộc crate nào** (`v2/kernel/evil/lib.rs`, không có
# `Cargo.toml`) ⇒ tệp được biên dịch nhưng **không** có trong `cargo metadata`.
chen() {
    mkdir -p v2/kernel/evil
    cat > v2/kernel/evil/lib.rs <<'EOF'
// SPDX-License-Identifier: MIT OR Apache-2.0
pub fn evil(m: &mut crate::Machine, t: crate::ObjId) {
    let _ = m.wake(t);
}
EOF
    printf '\n#[path = "../../evil/lib.rs"]\nmod evil;\n' >> v2/kernel/vifast-core/src/lib.rs
}
