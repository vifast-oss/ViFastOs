# SPDX-License-Identifier: MIT OR Apache-2.0
# B2 — như B1 nhưng `doorbell` chỉ trong **chú thích**. Trước KT-267: miễn theo tên tệp ⇒ XANH.
# Sau KT-267: `doorbell` vẫn khớp (luật giữ nguyên độ rộng), miễn theo **ngữ nghĩa** ⇒ ĐỎ.
chen() {
    cat > v2/kernel/vifast-plat-riscv64/src/virtio_evil.rs <<'EOF'
// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-267 B2 — tệp tên `virtio_*` nhưng **không** phải driver virtio.
// doorbell riêng (chú thích)
pub fn kt267_b2() -> u32 {
    1
}
EOF
    python3 - <<'PY'
from pathlib import Path
p = Path("v2/kernel/vifast-plat-riscv64/src/lib.rs")
t = p.read_text()
t = t.replace("pub mod static_slot;\n", "pub mod static_slot;\nmod virtio_evil;\n", 1)
p.write_text(t)
PY
}
