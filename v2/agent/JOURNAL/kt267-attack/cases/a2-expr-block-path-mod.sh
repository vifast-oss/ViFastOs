# SPDX-License-Identifier: MIT OR Apache-2.0
# A2 — `:expr` chở `mod` + `#[path]` **qua biểu thức block** trong **đối số lời gọi**:
# `kt267_pass!({ #[path = "…"] mod evil; evil::f() })`. Đây là dạng **mạnh nhất** của
# `:expr` (block là biểu thức, chứa được item `mod` + thuộc tính `#[path]`).
# Tệp đích `kt267_evil_mod.rs` gọi `m.wake(t)` thật.
chen() {
    cat > v2/kernel/vifast-core/src/kt267_expr.rs <<'EOF'
// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-267 A2 — macro `:expr` passthrough (định nghĩa **trong phạm vi**).
macro_rules! kt267_pass {
    ($e:expr) => {
        $e
    };
}
pub(crate) use kt267_pass;
EOF
    cat > v2/kernel/vifast-core/src/kt267_call.rs <<'EOF'
// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-267 A2 — `:expr` chở `mod` + `#[path]` qua block trong **đối số lời gọi**.
use crate::kt267_expr::kt267_pass;

pub fn kt267_evil(m: &mut crate::Machine, t: crate::ObjId) {
    let _ = kt267_pass!({
        #[path = "kt267_evil_mod.rs"]
        mod evil;
        evil::kt267_evil(m, t)
    });
}
EOF
    cat > v2/kernel/vifast-core/src/kt267_evil_mod.rs <<'EOF'
// SPDX-License-Identifier: MIT OR Apache-2.0
// KT-267 A2 — module đích của `#[path]` trong block; gọi `wake` thật.
pub fn kt267_evil(m: &mut crate::Machine, t: crate::ObjId) {
    let _ = m.wake(t);
}
EOF
    python3 - <<'PY'
from pathlib import Path
p = Path("v2/kernel/vifast-core/src/lib.rs")
t = p.read_text()
t = t.replace("mod tlb;\n", "mod tlb;\nmod kt267_call;\nmod kt267_expr;\n", 1)
p.write_text(t)
PY
}
