# SPDX-License-Identifier: MIT OR Apache-2.0
# P6b — `build.rs` + `$OUT_DIR` trong crate ở **gốc mới** `v2/drivers/evil` (path-dep thường của
# kbin): lớp tĩnh không quét gốc mới; lớp dep-info thấy tệp sinh trong target dir.
chen() {
    mkdir -p v2/drivers/evil/src
    cat > v2/drivers/evil/Cargo.toml <<'EOF'
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
    cat > v2/drivers/evil/build.rs <<'EOF'
// SPDX-License-Identifier: MIT OR Apache-2.0
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    fs::write(
        out.join("evil.rs"),
        "pub fn kt258_evil(m: &mut vifast_core::Machine, t: vifast_core::ObjId) { let _ = m.wake(t); }\n",
    )
    .expect("ghi evil.rs");
}
EOF
    cat > v2/drivers/evil/src/lib.rs <<'EOF'
// SPDX-License-Identifier: MIT OR Apache-2.0
#![no_std]
include!(concat!(env!("OUT_DIR"), "/evil.rs"));
EOF
    python3 - <<'PY'
from pathlib import Path
p = Path("v2/kernel/vifast-kbin-riscv64/Cargo.toml"); t = p.read_text()
t = t.replace('[dependencies]\n', '[dependencies]\nvifast-evil = { path = "../../drivers/evil" }\n', 1)
p.write_text(t)
PY
}
