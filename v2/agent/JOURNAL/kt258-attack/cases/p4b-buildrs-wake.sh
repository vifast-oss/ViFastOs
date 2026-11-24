# SPDX-License-Identifier: MIT OR Apache-2.0
# P4b — `build.rs` của crate kernel chứa `wake` (build script **được biên dịch thật** cho host
# trong mỗi lần build kernel) ⇒ phải bị bắt.
chen() {
    cat > v2/kernel/vifast-plat-common/build.rs <<'EOF'
// SPDX-License-Identifier: MIT OR Apache-2.0
fn main() {
    let mut m = vifast_core::Machine::new();
    let _ = m.wake(0);
}
EOF
    python3 - <<'PY'
from pathlib import Path
p = Path("v2/kernel/vifast-plat-common/Cargo.toml"); t = p.read_text()
t = t.replace('[dependencies]\n', '[build-dependencies]\nvifast-core = { path = "../vifast-core" }\n\n[dependencies]\n', 1)
p.write_text(t)
PY
}
