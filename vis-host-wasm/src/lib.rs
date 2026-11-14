// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-74 (VD-104)** — khung host `wasm32` cho ViScript.
//!
//! **Chưa có runner wasm**: crate chỉ khẳng định **mức bằng chứng** — biên dịch chéo xanh. Trên
//! `wasm32-unknown-unknown` **không** có sandbox OS, không tiến trình, không socket; lời gọi
//! `std::process`/`std::net`/`std::fs` chỉ biên dịch được, **không** chạy được. Vì vậy `probe()`
//! trả đúng câu mô tả mức bảo đảm, không giả vờ có cách ly.

#![cfg(target_arch = "wasm32")]

/// Câu mô tả **mức bảo đảm** trên wasm: chỉ runtime kiểm, không có sandbox OS.
pub const MO_TA: &str = "wasm: runtime kiểm, không có sandbox OS";

/// Probe tĩnh: trả câu mô tả mức bảo đảm (không truy cập OS).
pub fn probe() -> &'static str {
    MO_TA
}
