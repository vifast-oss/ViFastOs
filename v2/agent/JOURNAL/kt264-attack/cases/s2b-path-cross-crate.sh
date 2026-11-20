# SPDX-License-Identifier: MIT OR Apache-2.0
# S2b — `#[path = "../../vifast-plat-common/src/evil.rs"] mod evil;` trong crate **thuộc
# `V2_SCAN`** (`v2/kernel/vifast-core`): đích **trong `v2/kernel`** (nên
# `resolve_include_targets` **cho phép**) nhưng **ngoài crate** `vifast-core` — nó thuộc crate
# `vifast-plat-common`.
#
# Vì sao là **đường lọt cấu trúc**: cùng một tệp được biên dịch vào **hai** crate với hai phân
# loại test/sản phẩm khác nhau ⇒ `is_test` của luật token có thể sai; và `mod` graph của crate
# không còn khép kín. Luật hiện có chỉ chặn đích ra ngoài `v2/kernel`+`v2/sim`, **không** chặn
# đích ra ngoài **crate**. Lớp dep-info **không** bắt: tệp thuộc crate `vifast-plat-common`
# trong metadata.
chen() {
    cat > v2/kernel/vifast-plat-common/src/evil.rs <<'EOF'
// SPDX-License-Identifier: MIT OR Apache-2.0
// KT-264 S2b — tệp **ngoài crate** `vifast-core`, nạp bằng `#[path]`.
pub fn evil() -> u8 {
    0
}
EOF
    cat >> v2/kernel/vifast-core/src/lib.rs <<'EOF'

// KT-264 S2b — `#[path]` trỏ **ra ngoài crate** nhưng vẫn trong `v2/kernel`.
#[path = "../../vifast-plat-common/src/evil.rs"]
mod evil;
EOF
}
