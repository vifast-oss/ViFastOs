# SPDX-License-Identifier: MIT OR Apache-2.0
# S2 — `#[path = "../../../evil.rs"] mod evil;` trong crate **thuộc `V2_SCAN`**
# (`v2/kernel/vifast-core`), đích **ngoài crate** (`v2/evil.rs`). Tệp đích gọi `wake`.
#
# Câu hỏi: **luật cấu trúc** có bắt cạnh `mod`/`#[path]` **ra khỏi crate** không (không chỉ
# "ra ngoài `v2/kernel`+`v2/sim`" như `resolve_include_targets` đang kiểm)?
chen() {
    cat > v2/evil.rs <<'EOF'
// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-264 S2 — đích `#[path]` **ngoài crate** `vifast-core`.
pub fn evil(m: &mut crate::Machine, t: crate::ObjId) {
    let _ = m.wake(t);
}
EOF
    cat >> v2/kernel/vifast-core/src/lib.rs <<'EOF'

// KT-264 S2 — `#[path]` trỏ **ra ngoài crate** (`v2/evil.rs`).
#[path = "../../../evil.rs"]
mod evil;
EOF
}
