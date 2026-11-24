# SPDX-License-Identifier: MIT OR Apache-2.0
# P2c — crate ở `v2/foo/evil` là **build-dependency** của `vifast-plat-common`; build script
# **dùng** nó (nếu không, cargo chỉ cảnh báo "unused build dependency" và **không** biên dịch).
chen() {
    mkdir -p v2/foo/evil/src
    cat > v2/foo/evil/Cargo.toml <<'EOF'
# SPDX-License-Identifier: MIT OR Apache-2.0
[package]
name = "vifast-evil"
version = "0.1.0"
edition = "2021"
license = "MIT OR Apache-2.0"

[workspace]

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
    cat > v2/kernel/vifast-plat-common/build.rs <<'EOF'
// SPDX-License-Identifier: MIT OR Apache-2.0
// Build script rỗng: cargo vẫn **biên dịch** mọi build-dependency (đo: "Compiling vifast-evil").
fn main() {}
EOF
    python3 - <<'PY'
from pathlib import Path
p = Path("v2/kernel/vifast-plat-common/Cargo.toml"); t = p.read_text()
t = t.replace('[dependencies]\n', '[build-dependencies]\nvifast-evil = { path = "../../foo/evil" }\n\n[dependencies]\n', 1)
p.write_text(t)
PY
}
