# SPDX-License-Identifier: MIT OR Apache-2.0
# S5 — tệp nguồn **mồ côi** (không được khai báo bằng `mod`) trong crate ở **gốc mới**
# (`v2/drivers/evil/src/orphan.rs`). Tệp **không** được biên dịch ⇒ `cargo` RC=0; nó cũng
# **không** nằm trong `mod` graph ⇒ luật cấu trúc phải bắt "tệp `.rs` không thuộc `mod` graph".
#
# Vì sao đây là **đường lọt cấu trúc**: luật token quét **mọi** tệp `.rs` trong phạm vi nên
# `wake` trong tệp mồ côi vẫn bị bắt; nhưng **cấu trúc** thì không — tệp mồ côi là chỗ để
# `mod` graph của checker và của rustc **lệch nhau** (một `mod` thêm sau sẽ biên dịch nó mà
# không ai kiểm cạnh nguồn). Đây đúng nợ §7.3: "`mod` graph lạ không bị lớp tĩnh bắt".
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

pub fn ok() -> u8 {
    0
}
EOF
    cat > v2/drivers/evil/src/orphan.rs <<'EOF'
// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-264 S5 — tệp **mồ côi**: không `mod` nào khai báo nó.
pub fn not_compiled() -> u8 {
    1
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
    '[dependencies]\n# KT-264 S5 — path-dependency tuỳ chọn, chỉ bật bởi `product-fs`.\nvifast-evil = { path = "../../drivers/evil", optional = true }\n',
    1,
)
p.write_text(t)
PY
}
EXTRA_CARGO='cargo check --manifest-path v2/kernel/Cargo.toml -p vifast-kbin-riscv64 --bin vifast-kbin-riscv64 --target riscv64gc-unknown-none-elf --features product-fs -q'
