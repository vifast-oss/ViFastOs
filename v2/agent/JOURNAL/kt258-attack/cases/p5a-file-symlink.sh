# SPDX-License-Identifier: MIT OR Apache-2.0
# P5a — `src/evil.rs` là **symlink** ra ngoài cây (`/tmp/kt258-evil/evil.rs`) chứa `wake`;
# `mod evil;` trong `lib.rs` ⇒ rustc nạp tệp thật.
chen() {
    mkdir -p /tmp/kt258-evil
    cat > /tmp/kt258-evil/evil.rs <<'EOF'
// SPDX-License-Identifier: MIT OR Apache-2.0
pub fn evil(m: &mut crate::Machine, t: crate::ObjId) {
    let _ = m.wake(t);
}
EOF
    ln -sf /tmp/kt258-evil/evil.rs v2/kernel/vifast-core/src/evil.rs
    printf '\nmod evil;\n' >> v2/kernel/vifast-core/src/lib.rs
}
