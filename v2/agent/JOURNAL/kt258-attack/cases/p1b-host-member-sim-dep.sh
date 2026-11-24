# SPDX-License-Identifier: MIT OR Apache-2.0
# P1b — crate ở **gốc mới** `v2/net/evil`, thành viên workspace host **và** path-dep của
# `vifast-sim` ⇒ được biên dịch vào **oracle** (sim), không chỉ đứng riêng.
chen() {
    mkdir -p v2/net/evil/src
    cat > v2/net/evil/Cargo.toml <<'EOF'
# SPDX-License-Identifier: MIT OR Apache-2.0
[package]
name = "vifast-evil"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
vifast-core = { path = "../../kernel/vifast-core" }
EOF
    cat > v2/net/evil/src/lib.rs <<'EOF'
// SPDX-License-Identifier: MIT OR Apache-2.0
#![no_std]
pub fn evil(m: &mut vifast_core::Machine, t: vifast_core::ObjId) {
    let _ = m.wake(t);
}
EOF
    python3 - <<'PY'
from pathlib import Path
p = Path("v2/Cargo.toml"); t = p.read_text()
t = t.replace('members = [\n', 'members = [\n    "net/evil",\n', 1)
p.write_text(t)
s = Path("v2/sim/vifast-sim/Cargo.toml"); u = s.read_text()
u = u.replace('[dependencies]\n', '[dependencies]\nvifast-evil = { path = "../../net/evil" }\n', 1)
s.write_text(u)
PY
}
EXTRA_CARGO='cargo check --manifest-path v2/Cargo.toml -q'
