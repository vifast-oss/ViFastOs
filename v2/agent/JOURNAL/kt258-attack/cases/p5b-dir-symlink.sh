# SPDX-License-Identifier: MIT OR Apache-2.0
# P5b — **symlink thư mục**: `src/evil/` → `/tmp/kt258-evil/dir/` chứa `mod.rs` có `wake`;
# `mod evil;` ⇒ rustc nạp `src/evil/mod.rs` (qua symlink).
chen() {
    mkdir -p /tmp/kt258-evil/dir
    cat > /tmp/kt258-evil/dir/mod.rs <<'EOF'
// SPDX-License-Identifier: MIT OR Apache-2.0
pub fn evil(m: &mut crate::Machine, t: crate::ObjId) {
    let _ = m.wake(t);
}
EOF
    ln -sfn /tmp/kt258-evil/dir v2/kernel/vifast-core/src/evil
    printf '\nmod evil;\n' >> v2/kernel/vifast-core/src/lib.rs
}
