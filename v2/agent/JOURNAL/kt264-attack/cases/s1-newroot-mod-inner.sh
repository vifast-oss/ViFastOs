# SPDX-License-Identifier: MIT OR Apache-2.0
# S1 — crate ở **gốc mới** `v2/drivers/evil` (không phải thành viên workspace nào) là
# **path-dependency tuỳ chọn** của `vifast-kbin-riscv64`, chỉ bật bởi feature `product-fs`
# (feature của **ảnh sản phẩm**). `src/lib.rs` khai `mod inner;`, `src/inner.rs` gọi `wake`.
#
# Câu hỏi: **luật cấu trúc** (`scan_structure` / `mod` graph) có phủ crate ngoài `V2_SCAN`
# không — và nếu không, `wake` trong tệp con của `mod` graph có lọt không?
chen() {
    mkdir -p v2/drivers/evil/src
    cat > v2/drivers/evil/Cargo.toml <<'EOF'
# SPDX-License-Identifier: MIT OR Apache-2.0
[package]
name = "vifast-evil"
version = "0.1.0"
edition = "2021"
license = "MIT OR Apache-2.0"

# Crate riêng (không thuộc workspace host `v2/` dù nằm dưới nó).
[workspace]

[dependencies]
vifast-core = { path = "../../kernel/vifast-core" }
EOF
    cat > v2/drivers/evil/src/lib.rs <<'EOF'
// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-264 S1 — crate ở gốc mới, chỉ được biên dịch khi bật `product-fs`.
#![no_std]

mod inner;

pub use inner::evil;
EOF
    cat > v2/drivers/evil/src/inner.rs <<'EOF'
// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-264 S1 — tệp **con của `mod` graph**, gọi `wake`.

pub fn evil(m: &mut vifast_core::Machine, t: vifast_core::ObjId) {
    let _ = m.wake(t);
}
EOF
    python3 - <<'PY'
from pathlib import Path
p = Path("v2/kernel/vifast-kbin-riscv64/Cargo.toml")
t = p.read_text()
t = t.replace(
    'product-fs = ["product-shell", "vifast-plat-riscv64/block"]',
    'product-fs = ["product-shell", "vifast-plat-riscv64/block", "vifast-evil"]',
    1,
)
t = t.replace(
    '[dependencies]\n',
    '[dependencies]\n# KT-264 S1 — path-dependency tuỳ chọn, chỉ bật bởi `product-fs`.\nvifast-evil = { path = "../../drivers/evil", optional = true }\n',
    1,
)
p.write_text(t)
m = Path("v2/kernel/vifast-kbin-riscv64/src/main.rs")
s = m.read_text()
s += """
// KT-264 S1 — đầu mối gọi crate ở gốc mới (chỉ biên dịch khi bật `product-fs`).
#[cfg(feature = "product-fs")]
#[allow(dead_code)]
fn kt264_probe(m: &mut vifast_core::Machine, t: vifast_core::ObjId) {
    vifast_evil::evil(m, t);
}
"""
m.write_text(s)
PY
}
EXTRA_CARGO='cargo check --manifest-path v2/kernel/Cargo.toml -p vifast-kbin-riscv64 --bin vifast-kbin-riscv64 --target riscv64gc-unknown-none-elf --features product-fs -q'
