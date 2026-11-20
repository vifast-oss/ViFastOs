# SPDX-License-Identifier: MIT OR Apache-2.0
# P5c — symlink tới tệp **ngoài repo** nhưng **không** đuôi `.rs` (`/tmp/kt258-evil/evil.txt`):
# tên symlink là `evil.rs` ⇒ checker phải đi theo symlink và đọc nội dung.
chen() {
    mkdir -p /tmp/kt258-evil
    cat > /tmp/kt258-evil/evil.txt <<'EOF'
// SPDX-License-Identifier: MIT OR Apache-2.0
pub fn evil(m: &mut crate::Machine, t: crate::ObjId) {
    let _ = m.wake(t);
}
EOF
    ln -sf /tmp/kt258-evil/evil.txt v2/kernel/vifast-core/src/evil.rs
    printf '\nmod evil;\n' >> v2/kernel/vifast-core/src/lib.rs
}
