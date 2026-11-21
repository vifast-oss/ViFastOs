# SPDX-License-Identifier: MIT OR Apache-2.0
# C1 — nợ §7.3(c): `wake` trong `vifast-kbin-riscv64/src/proc_fs.rs` — tệp **chỉ** biên dịch
# dưới `product-fs` (`#[cfg(feature = "product-fs")] mod proc_fs;` ở cuối `main.rs`).
#
# Đo hai điều:
#   (i) **lớp tĩnh** phủ đủ (tệp trong phạm vi, luật token **không** phụ thuộc feature) ⇒ ĐỎ;
#   (ii) **lớp dep-info** — `cargo check --workspace` mặc định **không** nạp `proc_fs.rs` ⇒ XANH
#        (hở); `--product-fs` nạp ⇒ ĐỎ `E_DEPINFO_WAKE`.
chen() {
    cat >> v2/kernel/vifast-kbin-riscv64/src/proc_fs.rs <<'EOF'

// KT-267 C1 — mã **chỉ** tồn tại dưới `product-fs`.
pub fn kt267_c1(m: &mut vifast_core::Machine, t: vifast_core::ObjId) {
    let _ = m.wake(t);
}
EOF
}
EXTRA_CARGO='cargo check --manifest-path v2/kernel/Cargo.toml --workspace --target riscv64gc-unknown-none-elf --features product-fs -q'
