# SPDX-License-Identifier: MIT OR Apache-2.0
# B1 — nợ §7.3(b): tệp tên `virtio_evil.rs` **không** phải driver virtio (không `SplitQueue`/
# `VqLayout`, không kiểu `Virtio*`, không `impl … VirtioKind`) mà dùng `doorbell` trong **mã**.
# Trước KT-267: miễn theo **tên tệp** ⇒ XANH cả ba lượt. Sau KT-267: kiểm **ngữ nghĩa** ⇒ ĐỎ
# `E_ARCH_DOORBELL`.
chen() {
    cat > v2/kernel/vifast-plat-riscv64/src/virtio_evil.rs <<'EOF'
// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-267 B1 — tệp tên `virtio_*` nhưng **không** phải driver virtio.
pub fn kt267_b1() -> u32 {
    let doorbell = 1u32;
    doorbell
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
