# SPDX-License-Identifier: MIT OR Apache-2.0
# P8a — `v2/lib/vifast-net` (crate lib **không** nằm trong bao đóng `vifast-core`) chỉ có
# **dev-dependency** `vifast-core`; mã **sản phẩm** gọi `wake` ⇒ phải **không biên dịch được**.
chen() {
    python3 - <<'PY'
from pathlib import Path
p = Path("v2/lib/vifast-net/Cargo.toml"); t = p.read_text()
t += '\n[dev-dependencies]\nvifast-core = { path = "../../kernel/vifast-core" }\n'
p.write_text(t)
PY
    cat >> v2/lib/vifast-net/src/lib.rs <<'EOF'

// KT-258 P8a — mã **sản phẩm** gọi `wake` qua dev-dependency (phải bị cargo từ chối).
pub fn kt258_evil(m: &mut vifast_core::Machine, t: vifast_core::ObjId) {
    let _ = m.wake(t);
}
EOF
}
