# SPDX-License-Identifier: MIT OR Apache-2.0
# S1b — như S1 nhưng cạnh `mod` **không phân giải được**: `#[cfg(any())] mod ghost;` trong
# crate ở **gốc mới** (không có `ghost.rs`). `cargo` RC=0 vì nhánh cfg tắt.
#
# Vì sao là **đường lọt cấu trúc**: `_mods_of` chỉ thêm cạnh khi `is_file()` ⇒ cạnh này **bỏ
# im lặng** với mọi luật; `resolve_include_targets` chỉ phân giải `#[path]`, không phân giải
# `mod NAME;` trơn. Tệp `ghost.rs` (nếu được thêm sau) sẽ biên dịch mà không ai kiểm cạnh nguồn.
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

// KT-264 S1b — cạnh `mod` **không phân giải được** (nhánh cfg luôn tắt).
#[cfg(any())]
mod ghost;

pub fn ok() -> u8 {
    0
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
    '[dependencies]\n# KT-264 S1b — path-dependency tuỳ chọn, chỉ bật bởi `product-fs`.\nvifast-evil = { path = "../../drivers/evil", optional = true }\n',
    1,
)
p.write_text(t)
PY
}
EXTRA_CARGO='cargo check --manifest-path v2/kernel/Cargo.toml -p vifast-kbin-riscv64 --bin vifast-kbin-riscv64 --target riscv64gc-unknown-none-elf --features product-fs -q'
