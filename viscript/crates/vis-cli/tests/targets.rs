// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-74 (VD-104)** — smoke test cho ma trận `cargo check --target`: **một** ô
//! (`vis-types` × `wasm32-unknown-unknown`) để bắt hồi quy target trong `cargo test`.
//!
//! Ma trận **đầy đủ** (8 crate × 2 target + `vis-host-*`) do `bin/vis-targets` đo và in bảng; test
//! này cố ý chỉ gọi **một** ô (~giây) để không làm cổng chậm. Mức bằng chứng: **chỉ CHECK biên
//! dịch chéo**, **không** chạy trên wasm.

use std::path::PathBuf;
use std::process::Command;

#[test]
fn targets_wasm_vis_types() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml");
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let out = Command::new(cargo)
        .args([
            "check",
            "--manifest-path",
            manifest.to_str().expect("đường dẫn UTF-8"),
            "-p",
            "vis-types",
            "--target",
            "wasm32-unknown-unknown",
        ])
        .output()
        .expect("chạy `cargo check`");
    assert!(
        out.status.success(),
        "vis-types × wasm32 phải CHECK xanh; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
