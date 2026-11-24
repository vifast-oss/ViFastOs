# SPDX-License-Identifier: MIT OR Apache-2.0
# P1d — như P1c nhưng `wake` nằm trong tệp **được `include!`** từ crate ở gốc mới (path-dep
# tuỳ chọn của kbin, chỉ bật bởi `product-fs`): đo **luật cấu trúc** (`include!`/`#[path]`) có
# phủ crate trong bao đóng sản phẩm không.
chen() {
    mkdir -p v2/drivers/evil/src
    cat > v2/drivers/evil/Cargo.toml <<'EOF'
# SPDX-License-Identifier: MIT OR Apache-2.0
[package]
name = "vifast-evil"
version = "0.1.0"
edition = "2021"
license = "MIT OR Apache-2.0"

[workspace]

[dependencies]
vifast-core = { path = "../../kernel/vifast-core" }
EOF
    cat > v2/drivers/evil/src/lib.rs <<'EOF'
// SPDX-License-Identifier: MIT OR Apache-2.0
#![no_std]
include!("evil_body.rs");
EOF
    cat > v2/drivers/evil/src/evil_body.rs <<'EOF'
// SPDX-License-Identifier: MIT OR Apache-2.0
pub fn evil(m: &mut vifast_core::Machine, t: vifast_core::ObjId) {
    let _ = m.wake(t);
}
EOF
    python3 - <<'PY'
from pathlib import Path
p = Path("v2/kernel/vifast-kbin-riscv64/Cargo.toml"); t = p.read_text()
t = t.replace(
    'product-fs = ["product-shell", "vifast-plat-riscv64/block"]',
    'product-fs = ["product-shell", "vifast-plat-riscv64/block", "vifast-evil"]',
    1,
)
t = t.replace(
    '[dependencies]\n',
    '[dependencies]\nvifast-evil = { path = "../../drivers/evil", optional = true }\n',
    1,
)
p.write_text(t)
PY
}
EXTRA_CARGO='cargo check --manifest-path v2/kernel/Cargo.toml -p vifast-kbin-riscv64 --bin vifast-kbin-riscv64 --target riscv64gc-unknown-none-elf --features product-fs -q'
