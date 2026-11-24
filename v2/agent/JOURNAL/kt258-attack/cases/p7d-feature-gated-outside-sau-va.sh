# SPDX-License-Identifier: MIT OR Apache-2.0
# P7d — crate ở **ngoài repo** (`/tmp/kt258-evil/crate`) là path-dependency **tuỳ chọn** của
# `vifast-kbin-riscv64`, chỉ bật bởi `product-fs`: `cargo metadata` **bỏ** nó (ngoài gốc) nên
# phạm vi không có nó; lớp dep-info không build (feature tắt) ⇒ đo xem có lọt không.
chen() {
    mkdir -p /tmp/kt258-evil/crate/src
    cat > /tmp/kt258-evil/crate/Cargo.toml <<EOF
# SPDX-License-Identifier: MIT OR Apache-2.0
[package]
name = "vifast-evil"
version = "0.1.0"
edition = "2021"
license = "MIT OR Apache-2.0"

[workspace]

[dependencies]
vifast-core = { path = "$ROOT/v2/kernel/vifast-core" }
EOF
    cat > /tmp/kt258-evil/crate/src/lib.rs <<'EOF'
// SPDX-License-Identifier: MIT OR Apache-2.0
#![no_std]
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
    '[dependencies]\nvifast-evil = { path = "/tmp/kt258-evil/crate", optional = true }\n',
    1,
)
p.write_text(t)
PY
}
EXTRA_CARGO='cargo check --manifest-path v2/kernel/Cargo.toml -p vifast-kbin-riscv64 --bin vifast-kbin-riscv64 --target riscv64gc-unknown-none-elf --features product-fs -q'
