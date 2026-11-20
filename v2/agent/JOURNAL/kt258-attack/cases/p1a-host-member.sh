# SPDX-License-Identifier: MIT OR Apache-2.0
# P1a — crate ở **gốc mới** `v2/drivers/evil`, thêm vào `members` của workspace **host**
# (`v2/Cargo.toml`) đúng như đề bài; `src/lib.rs` gọi `wake`.
chen() {
    mkdir -p v2/drivers/evil/src
    cat > v2/drivers/evil/Cargo.toml <<'EOF'
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
    cat > v2/drivers/evil/src/lib.rs <<'EOF'
// SPDX-License-Identifier: MIT OR Apache-2.0
#![no_std]
pub fn evil(m: &mut vifast_core::Machine, t: vifast_core::ObjId) {
    let _ = m.wake(t);
}
EOF
    python3 - <<'PY'
from pathlib import Path
p = Path("v2/Cargo.toml"); t = p.read_text()
t = t.replace('members = [\n', 'members = [\n    "drivers/evil",\n', 1)
p.write_text(t)
PY
}
EXTRA_CARGO='cargo check --manifest-path v2/Cargo.toml -q'
