// SPDX-License-Identifier: MIT OR Apache-2.0
//! Đăng ký linker script cho ảnh x86_64 (KT-257).
//!
//! Cùng khuôn `vifast-kbin-aarch64/build.rs` nhưng thêm `-no-pie`: target `x86_64-unknown-none`
//! của rustc 1.100.0 mặc định liên kết **PIE** (đo được: ELF có `DYNAMIC` + entry `0x1274`), mà
//! ảnh boot phải nằm ở **địa chỉ tuyệt đối** `0x0010_0000` để QEMU `-kernel` nạp đúng và để
//! `_start` 32-bit dùng được địa chỉ tĩnh.
//!
//! `--gc-sections` cần thiết vì `global_asm!` đặt mỗi đoạn trong section riêng; linker script
//! `KEEP` đúng ba thứ không ai tham chiếu từ Rust: `.multiboot`, `.note.Xen`, `.text.entry`.

fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    println!("cargo:rustc-link-arg=-T{dir}/linker.ld");
    println!("cargo:rustc-link-arg=--gc-sections");
    println!("cargo:rustc-link-arg=-no-pie");
    println!("cargo:rerun-if-changed=linker.ld");
}
