# SPDX-License-Identifier: MIT OR Apache-2.0
# P2a — crate ở `v2/foo/evil` (ngoài ba gốc), **không** là thành viên workspace nào; là
# path-dependency **thường** của `vifast-plat-common` (workspace kernel) ⇒ vào **bao đóng**
# sản phẩm và được biên dịch mỗi lần build kernel.
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
    python3 - <<'PY'
from pathlib import Path
p = Path("v2/kernel/vifast-plat-common/Cargo.toml"); t = p.read_text()
t = t.replace('[dependencies]\n', '[dependencies]\nvifast-evil = { path = "../../foo/evil" }\n', 1)
p.write_text(t)
PY
}
