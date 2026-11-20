# SPDX-License-Identifier: MIT OR Apache-2.0
# S3b — `mod` lồng **3 tầng** trong crate ở **gốc mới**, tầng **lá** có cạnh `mod` **không phân
# giải được** (`#[cfg(any())] mod c;`, không có `c.rs`). `cargo` RC=0 (nhánh cfg tắt).
#
# Vì sao là **đường lọt cấu trúc**: luật cấu trúc trước KT-264 chỉ chạy `V2_SCAN` ⇒ **không**
# đi vào `mod` graph của crate ở gốc mới; và `_mods_of` bỏ im lặng cạnh không phân giải được.
# Ca này đo **độ sâu**: luật phải đi hết 3 tầng mới thấy cạnh ở tầng lá.
chen() {
    mkdir -p v2/drivers/evil/src/a/b
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
mod a;
pub fn ok() -> u8 {
    0
}
EOF
    cat > v2/drivers/evil/src/a.rs <<'EOF'
// SPDX-License-Identifier: MIT OR Apache-2.0
pub mod b;
EOF
    cat > v2/drivers/evil/src/a/b.rs <<'EOF'
// SPDX-License-Identifier: MIT OR Apache-2.0
// KT-264 S3b — tầng **lá** có cạnh `mod` không phân giải được (nhánh cfg luôn tắt).
#[cfg(any())]
pub mod c;
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
    '[dependencies]\n# KT-264 S3b — path-dependency tuỳ chọn, chỉ bật bởi `product-fs`.\nvifast-evil = { path = "../../drivers/evil", optional = true }\n',
    1,
)
p.write_text(t)
PY
}
EXTRA_CARGO='cargo check --manifest-path v2/kernel/Cargo.toml -p vifast-kbin-riscv64 --bin vifast-kbin-riscv64 --target riscv64gc-unknown-none-elf --features product-fs -q'
