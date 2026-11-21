# SPDX-License-Identifier: MIT OR Apache-2.0
# A1 — nợ §7.3(a): `:expr` passthrough. Macro `($e:expr) => { $e }` **định nghĩa** trong
# `vifast-core/src/kt267_expr.rs` (tệp **trong phạm vi**), **gọi** từ
# `vifast-core/src/kt267_call.rs` (tệp **trong phạm vi**) với `include!` trong **đối số lời
# gọi**; tệp được include gọi `m.wake(t)` thật.
#
# Câu hỏi của lead: `:expr` có chở được `include!` từ **tệp trong phạm vi** không?
# Chỉ tính là **đường lọt** khi `cargo check` RC=0 **và** cả ba lượt kiểm `errors=0`.
chen() {
    cat > v2/kernel/vifast-core/src/kt267_expr.rs <<'EOF'
// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-267 A1 — macro `:expr` passthrough (định nghĩa **trong phạm vi**).
macro_rules! kt267_pass {
    ($e:expr) => {
        $e
    };
}
pub(crate) use kt267_pass;
EOF
    cat > v2/kernel/vifast-core/src/kt267_call.rs <<'EOF'
// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-267 A1 — **lời gọi** `:expr` với `include!` trong đối số (tệp **trong phạm vi**).
use crate::kt267_expr::kt267_pass;

pub fn kt267_evil(m: &mut crate::Machine, t: crate::ObjId) {
    let _ = kt267_pass!(include!("kt267_evil_expr.rs"));
}
EOF
    cat > v2/kernel/vifast-core/src/kt267_evil_expr.rs <<'EOF'
// SPDX-License-Identifier: MIT OR Apache-2.0
// KT-267 A1 — nội dung được `include!` ở **vị trí biểu thức**; gọi `wake` thật.
m.wake(t)
EOF
    python3 - <<'PY'
from pathlib import Path
p = Path("v2/kernel/vifast-core/src/lib.rs")
t = p.read_text()
t = t.replace("mod tlb;\n", "mod tlb;\nmod kt267_call;\nmod kt267_expr;\n", 1)
p.write_text(t)
PY
}
