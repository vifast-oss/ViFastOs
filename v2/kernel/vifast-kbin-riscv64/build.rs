// SPDX-License-Identifier: MIT OR Apache-2.0
//! Đăng ký linker script và các cờ liên kết cho ảnh boot riscv64 (K0.5).

fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    println!("cargo:rustc-link-arg=-T{dir}/linker.ld");
    println!("cargo:rustc-link-arg=--gc-sections");
    println!("cargo:rerun-if-changed=linker.ld");
}
