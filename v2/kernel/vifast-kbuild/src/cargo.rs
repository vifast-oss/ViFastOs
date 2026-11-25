// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-404c** — những dòng `cargo:` mà **cả ba** `build.rs` của `vifast-kbin-*` đều phải in,
//! gom về một chỗ.
//!
//! # Vì sao
//!
//! `KT-404` đã rút phần *máy móc* (danh tính bản dựng + gọi `$RUSTC`) về crate này, nhưng ba
//! `build.rs` vẫn còn **ba bản sao** của ba khối khai báo:
//!
//! | Khối | Bản sao trước `KT-404c` |
//! |---|---:|
//! | `feature(name)` (`CARGO_FEATURE_<NAME>`) | 2 (aarch64, x86_64) |
//! | ba dòng `rustc-link-arg` + `rerun-if-changed=linker.ld` | 3 |
//! | vòng `rerun-if-changed` cho tệp nguồn | 3 |
//!
//! Ba bản sao của một khối khai báo là lớp lỗi KT-76/KT-346: sửa một bản là hai bản kia vẫn sai.
//! `KT-404c` giữ **nguyên văn từng dòng in ra** (thứ tự giữ nguyên) nên `cargo:rustc-link-arg` và
//! các khai báo `rerun-if-changed` của ảnh **không đổi** — chỉ chỗ viết mã đổi.

/// `true` ⇔ feature `name` của Cargo đang bật (Cargo đặt `CARGO_FEATURE_<NAME>`).
///
/// Tên viết **đúng như Cargo đặt**: `product-fs` → `PRODUCT_FS`.
pub fn feature(name: &str) -> bool {
    std::env::var_os(format!("CARGO_FEATURE_{name}")).is_some()
}

/// Đăng ký linker script của ảnh (`<manifest_dir>/linker.ld`) và `--gc-sections` cho **chính**
/// crate ảnh.
///
/// `extra` là cờ `-C link-arg=…` riêng của arch cho ảnh kernel (x86_64: `-no-pie`; riscv64 /
/// aarch64: rỗng). Cờ `link-arg` của **chương trình U-mode** đi qua
/// [`crate::compile::compile_user_elf`], không phải hàm này.
pub fn link_kernel(manifest_dir: &str, extra: &[&str]) {
    println!("cargo:rustc-link-arg=-T{manifest_dir}/linker.ld");
    println!("cargo:rustc-link-arg=--gc-sections");
    for arg in extra {
        println!("cargo:rustc-link-arg={arg}");
    }
    println!("cargo:rerun-if-changed=linker.ld");
}

/// Khai báo đường dẫn (tương đối với thư mục crate) phải làm build script **chạy lại**.
///
/// Thiếu khai báo ⇒ cargo dùng lại build script cũ ⇒ ảnh nhúng artefact cũ (KT-76).
pub fn rerun_if_changed(paths: &[&str]) {
    for path in paths {
        println!("cargo:rerun-if-changed={path}");
    }
}

/// Khai báo feature (`CARGO_FEATURE_<NAME>`) phải làm build script **chạy lại** — dùng cho các
/// feature chỉ đổi `--cfg` của artefact U-mode hoặc bật/tắt một nhánh build.
pub fn rerun_if_features(names: &[&str]) {
    for name in names {
        println!("cargo:rerun-if-env-changed=CARGO_FEATURE_{name}");
    }
}

/// Cờ `--cfg` của artefact U-mode từ danh sách `(tên, bật)`.
///
/// Luôn phát **đủ** `--check-cfg=cfg(<tên>)` cho mọi mục (bản không bật vẫn không cảnh báo
/// "unexpected cfg"), rồi `--cfg <tên>` cho mục `Some(true)` — `None`/`Some(false)` chỉ khai báo.
/// Thứ tự: **toàn bộ** `--check-cfg` trước, rồi `--cfg` theo thứ tự danh sách (giữ nguyên dòng
/// lệnh `$RUSTC` của ba `build.rs` trước `KT-404c`).
pub fn cfgs(flags: &[(&str, Option<bool>)]) -> Vec<String> {
    let mut out: Vec<String> = flags
        .iter()
        .map(|(name, _)| format!("--check-cfg=cfg({name})"))
        .collect();
    for (name, on) in flags {
        if *on == Some(true) {
            out.push("--cfg".to_string());
            out.push((*name).to_string());
        }
    }
    out
}

/// `true` ⇔ biến môi trường `name` đang có mặt (dùng cho `TARGET`, `OUT_DIR` khi cần giá trị).
pub fn env(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

/// `true` ⇔ `TARGET` của lượt build là `target` — chặn nhánh artefact khi cargo build cho host.
pub fn target_is(target: &str) -> bool {
    env("TARGET").as_deref() == Some(target)
}
