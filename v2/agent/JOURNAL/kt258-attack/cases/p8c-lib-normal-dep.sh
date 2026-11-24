# SPDX-License-Identifier: MIT OR Apache-2.0
# P8c — **đối chứng dương**: `v2/lib/vifast-net` có **normal dependency** `vifast-core` + mã
# sản phẩm gọi `wake` ⇒ `E_SCOPE_LIB_CORE` **phải** đỏ (ca canh DEBT-KT227-1 có răng).
chen() {
    python3 - <<'PY'
from pathlib import Path
p = Path("v2/lib/vifast-net/Cargo.toml"); t = p.read_text()
t = t.replace('[dependencies]\n', '[dependencies]\nvifast-core = { path = "../../kernel/vifast-core" }\n', 1)
p.write_text(t)
PY
    cat >> v2/lib/vifast-net/src/lib.rs <<'EOF'

// KT-258 P8c — mã sản phẩm `v2/lib` gọi `wake` qua normal dependency.
pub fn kt258_evil(m: &mut vifast_core::Machine, t: vifast_core::ObjId) {
    let _ = m.wake(t);
}
EOF
}
EXTRA_CARGO='cargo check --manifest-path v2/Cargo.toml -q'
