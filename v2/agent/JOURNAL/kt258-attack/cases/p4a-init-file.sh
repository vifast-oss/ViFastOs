# SPDX-License-Identifier: MIT OR Apache-2.0
# P4a — tệp nguồn **ngoài `src/`** của crate ảnh sản phẩm: `init/evil.rs` (chương trình U-mode
# do `build.rs` biên dịch bằng rustc trực tiếp). Không thêm vào danh sách của `build.rs` ⇒
# **không** được biên dịch; ca này đo **phạm vi** (KT-256 T1 nói đã bắt).
chen() {
    cat > v2/kernel/vifast-kbin-riscv64/init/evil.rs <<'EOF'
// SPDX-License-Identifier: MIT OR Apache-2.0
#![no_std]
pub fn evil(m: &mut vifast_core::Machine, t: vifast_core::ObjId) {
    let _ = m.wake(t);
}
EOF
}
