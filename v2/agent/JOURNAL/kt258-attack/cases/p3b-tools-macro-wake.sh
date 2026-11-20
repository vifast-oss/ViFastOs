# SPDX-License-Identifier: MIT OR Apache-2.0
# P3b — `v2/tools/evil` (được nâng) chứa token `wake` trong **thân `macro_rules!`** — luật
# macro của lớp tĩnh phải quét crate được nâng (KT-256 nói "nâng ở cả lớp tĩnh").
chen() {
    mkdir -p v2/tools/evil/src
    cat > v2/tools/evil/Cargo.toml <<'EOF'
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
    cat > v2/tools/evil/src/lib.rs <<'EOF'
// SPDX-License-Identifier: MIT OR Apache-2.0
#![no_std]
macro_rules! kt258_m {
    ($m:expr, $t:expr) => {
        $m.wake($t)
    };
}
pub fn evil(m: &mut vifast_core::Machine, t: vifast_core::ObjId) {
    let _ = kt258_m!(m, t);
}
EOF
    python3 - <<'PY'
from pathlib import Path
p = Path("v2/Cargo.toml"); t = p.read_text()
t = t.replace('members = [\n', 'members = [\n    "tools/evil",\n', 1)
p.write_text(t)
PY
}
EXTRA_CARGO='cargo check --manifest-path v2/Cargo.toml -q'
