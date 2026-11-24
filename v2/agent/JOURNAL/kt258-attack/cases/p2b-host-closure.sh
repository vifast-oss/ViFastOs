# SPDX-License-Identifier: MIT OR Apache-2.0
# P2b — crate ở `v2/foo/evil`, path-dependency **thường** của `vifast-sim` (workspace host).
# Crate nằm trong cây thư mục workspace host ⇒ phải `exclude` khỏi workspace host (không dùng
# `[workspace]` riêng: cargo báo "multiple workspace roots").
chen() {
    mkdir -p v2/foo/evil/src
    cat > v2/foo/evil/Cargo.toml <<'EOF'
# SPDX-License-Identifier: MIT OR Apache-2.0
[package]
name = "vifast-evil"
version = "0.1.0"
edition = "2021"
license = "MIT OR Apache-2.0"

[dependencies]
vifast-core = { path = "../../kernel/vifast-core" }
EOF
    cat > v2/foo/evil/src/lib.rs <<'EOF'
// SPDX-License-Identifier: MIT OR Apache-2.0
#![no_std]
pub fn evil(m: &mut vifast_core::Machine, t: vifast_core::ObjId) {
    let _ = m.wake(t);
}
EOF
    python3 - <<'PY'
from pathlib import Path
p = Path("v2/Cargo.toml"); t = p.read_text()
t = t.replace('exclude = ["kernel"]', 'exclude = ["kernel", "foo"]', 1)
p.write_text(t)
s = Path("v2/sim/vifast-sim/Cargo.toml"); u = s.read_text()
u = u.replace('[dependencies]\n', '[dependencies]\nvifast-evil = { path = "../../foo/evil" }\n', 1)
s.write_text(u)
PY
}
EXTRA_CARGO='cargo check --manifest-path v2/Cargo.toml -q'
