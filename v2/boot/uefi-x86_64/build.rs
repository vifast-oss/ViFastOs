// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-414 — cờ **đột biến/tiêm lỗi lúc build** cho ca âm của boot stub UEFI.
//!
//! Hai biến môi trường (chỉ dùng trong `run.sh --selftest`, KHÔNG dùng ở ảnh sản phẩm):
//!
//! - `VIFAST_UEFI_FAULT=<tên>`: làm hỏng bảng nhớ ngay sau `GetMemoryMap` (trước khi kiểm) để
//!   chứng minh đường fail-closed **có tên** thật sự chạy. Danh sách đóng, sai tên ⇒ build đỏ
//!   (không âm thầm bỏ qua).
//! - `VIFAST_UEFI_NAIVE=1`: **tái hiện hành vi trước khi vá** — bỏ bước kiểm bảng nhớ rồi vẫn
//!   `ExitBootServices`. `run.sh --selftest` dùng nó để chứng minh ca âm **đỏ trên bản chưa vá**
//!   (nếu ca âm vẫn xanh trên bản naive thì phép kiểm là giả).
//!
//! Không có mạng, không dependency ngoài (D2-21).

use std::env;

/// Danh sách đóng của tên lỗi tiêm được.
const FAULTS: &[&str] = &[
    "",
    "desc-size",
    "desc-version",
    "map-size",
    "map-empty",
    "bad-type",
    "bad-page",
    "bad-attr",
];

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=VIFAST_UEFI_FAULT");
    println!("cargo:rerun-if-env-changed=VIFAST_UEFI_NAIVE");
    println!("cargo:rustc-check-cfg=cfg(uefi_naive)");

    let fault = env::var("VIFAST_UEFI_FAULT").unwrap_or_default();
    if !FAULTS.contains(&fault.as_str()) {
        panic!(
            "VIFAST_UEFI_FAULT='{fault}' không nằm trong danh sách đóng {FAULTS:?} — từ chối build"
        );
    }
    println!("cargo:rustc-env=VIFAST_UEFI_FAULT={fault}");

    let naive = env::var("VIFAST_UEFI_NAIVE").unwrap_or_default();
    if naive == "1" {
        println!("cargo:rustc-cfg=uefi_naive");
    } else if !naive.is_empty() && naive != "0" {
        panic!("VIFAST_UEFI_NAIVE='{naive}' không hợp lệ (chỉ nhận 0/1)");
    }
}
