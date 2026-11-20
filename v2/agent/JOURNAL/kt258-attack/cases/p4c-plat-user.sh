# SPDX-License-Identifier: MIT OR Apache-2.0
# P4c — shim ABI U-mode `v2/kernel/vifast-plat-user/lib.rs` (**không phải crate**, biên dịch bởi
# `build.rs` của kbin) chứa `wake` ⇒ phải bị bắt qua `NON_CRATE_SOURCE_TREES`.
chen() {
    cat >> v2/kernel/vifast-plat-user/lib.rs <<'EOF'

// KT-258 P4c — token `wake` trong shim ABI U-mode.
pub fn kt258_evil(m: &mut vifast_core::Machine, t: vifast_core::ObjId) {
    let _ = m.wake(t);
}
EOF
}
