# SPDX-License-Identifier: MIT OR Apache-2.0
# S4b — tệp **mồ côi** trong crate **`v2/lib`** (`vifast-net/src/kt264_orphan_bad.rs`) có khai
# báo `mod` **hỏng** (`mod 123;` trong nhánh `#[cfg(any())]`). Tệp **không** được biên dịch
# (không `mod` nào khai báo, không `include!`) ⇒ `cargo` RC=0.
#
# Vì sao là **đường lọt cấu trúc**: `scan_structure` trước KT-264 chỉ chạy `v2/kernel`+`v2/sim`
# ⇒ `E_LEX_STRUCT` trong `v2/lib` **không bao giờ** chạy; tệp cũng không thuộc `mod` graph của
# `vifast-net` ⇒ không luật nào thấy. Đây là dạng "`include!` bên trong `mod`" của S4 nhưng ở
# crate **trong phạm vi mà ngoài `V2_SCAN`** — chỗ `scan_structure` chưa từng chạy.
chen() {
    cat > v2/lib/vifast-net/src/kt264_orphan_bad.rs <<'EOF'
// SPDX-License-Identifier: MIT OR Apache-2.0
// KT-264 S4b — tệp **mồ côi** trong `v2/lib`, có khai báo `mod` **hỏng** (nhánh cfg luôn tắt).
#[cfg(any())]
mod 123;

pub fn not_compiled() -> u8 {
    3
}
EOF
}
EXTRA_CARGO='cargo check --manifest-path v2/Cargo.toml -p vifast-net -q'
