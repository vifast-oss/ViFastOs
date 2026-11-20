# SPDX-License-Identifier: MIT OR Apache-2.0
# P8b — `v2/lib/vifast-net` có **build-dependency** `vifast-core`; `build.rs` gọi `wake`.
# Build script **được biên dịch thật** cho host trong mỗi lần build host.
chen() {
    python3 - <<'PY'
from pathlib import Path
p = Path("v2/lib/vifast-net/Cargo.toml"); t = p.read_text()
t += '\n[build-dependencies]\nvifast-core = { path = "../../kernel/vifast-core" }\n'
p.write_text(t)
PY
    cat > v2/lib/vifast-net/build.rs <<'EOF'
// SPDX-License-Identifier: MIT OR Apache-2.0
fn main() {
    let mut m = vifast_core::Machine::new();
    let _ = m.wake(0);
}
EOF
}
EXTRA_CARGO='cargo check --manifest-path v2/Cargo.toml -q'
