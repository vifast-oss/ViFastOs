// SPDX-License-Identifier: MIT OR Apache-2.0
//! Đăng ký linker script cho ảnh aarch64 (KT-247).
//!
//! Cùng khuôn `vifast-kbin-riscv64/build.rs` nhưng **tối giản**: bước 3 mới có một ảnh, chưa có
//! ELF user nào cần biên dịch lồng, nên build script chỉ đăng ký `linker.ld` + `--gc-sections`.
//!
//! `--gc-sections` cần thiết vì `global_asm!` của bảng vector (KT-248) đặt mỗi ô trong section
//! riêng; không có cờ này thì linker giữ tất cả, có thì giữ đúng ô được tham chiếu — nhưng bảng
//! vector được `KEEP` trong linker script nên vẫn còn đủ 16 ô.

fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    println!("cargo:rustc-link-arg=-T{dir}/linker.ld");
    println!("cargo:rustc-link-arg=--gc-sections");
    println!("cargo:rerun-if-changed=linker.ld");
}
