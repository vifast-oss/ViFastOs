# SPDX-License-Identifier: MIT OR Apache-2.0
# P7a — crate khai `path` dependency trỏ **ra ngoài repo** (`/tmp/kt258-evil/crate`) ⇒
# `cargo metadata` **bỏ** nó khỏi phạm vi; nó vẫn được biên dịch (path-dep của crate kernel).
chen() {
    mkdir -p /tmp/kt258-evil/crate/src
    cat > /tmp/kt258-evil/crate/Cargo.toml <<EOF
# SPDX-License-Identifier: MIT OR Apache-2.0
[package]
name = "vifast-evil"
version = "0.1.0"
edition = "2021"
license = "MIT OR Apache-2.0"

[workspace]

[dependencies]
vifast-core = { path = "$ROOT/v2/kernel/vifast-core" }
EOF
    cat > /tmp/kt258-evil/crate/src/lib.rs <<'EOF'
// SPDX-License-Identifier: MIT OR Apache-2.0
#![no_std]
pub fn evil(m: &mut vifast_core::Machine, t: vifast_core::ObjId) {
    let _ = m.wake(t);
}
EOF
    python3 - <<'PY'
from pathlib import Path
p = Path("v2/kernel/vifast-plat-common/Cargo.toml"); t = p.read_text()
t = t.replace('[dependencies]\n', '[dependencies]\nvifast-evil = { path = "/tmp/kt258-evil/crate" }\n', 1)
p.write_text(t)
PY
}
