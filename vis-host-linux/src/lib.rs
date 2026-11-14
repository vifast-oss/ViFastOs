// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-73 (VD-107)** — khung host Linux cho ViScript: **chỉ probe Landlock bằng đọc tệp**.
//!
//! **Không** gọi syscall, **không** `unsafe`, **không** dependency. Trên Linux đọc
//! `/sys/kernel/security/lsm`; tập LSM chứa `landlock` ⇒ [`Probe::Co`], không ⇒ [`Probe::Khong`],
//! lỗi đọc ⇒ [`Probe::KhongRo`].
//!
//! **Giới hạn bằng chứng**: đây là **khung** + probe; crate **chưa** cưỡng chế quyền bằng kernel.
//! Muốn Landlock thật cần một trong hai (VD-107 (4)): (a) crate `landlock` được duyệt vào allowlist,
//! hoặc (b) `unsafe` **đóng** trong crate này + runner Linux để đo.

#![cfg(target_os = "linux")]

use std::fs;

/// Đường dẫn LSM của kernel Linux (chỉ đọc).
pub const DUONG_LSM: &str = "/sys/kernel/security/lsm";

/// Kết quả probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Probe {
    /// Có `landlock` trong danh sách LSM đang bật.
    Co { lsm: Vec<String> },
    /// Không có `landlock` (LSM đọc được nhưng thiếu).
    Khong { lsm: Vec<String> },
    /// Không đọc được `/sys/kernel/security/lsm`.
    KhongRo { ly_do: String },
}

impl Probe {
    /// Dòng chẩn đoán **ổn định** (cùng định dạng `vis run --sandbox-check`).
    pub fn dong(&self) -> String {
        match self {
            Probe::Co { lsm } => format!("landlock: có (LSM: {})", lsm.join(",")),
            Probe::Khong { lsm } => format!("landlock: không (LSM: {})", lsm.join(",")),
            Probe::KhongRo { ly_do } => format!("landlock: không rõ ({ly_do})"),
        }
    }

    /// Mã thoát gợi ý: có ⇒ 0, không ⇒ 3, không rõ ⇒ 4.
    pub fn ma(&self) -> u8 {
        match self {
            Probe::Co { .. } => 0,
            Probe::Khong { .. } => 3,
            Probe::KhongRo { .. } => 4,
        }
    }
}

/// Đọc `/sys/kernel/security/lsm` và phân loại — **chỉ đọc**, không syscall.
pub fn probe() -> Probe {
    match fs::read_to_string(DUONG_LSM) {
        Ok(raw) => {
            let lsm: Vec<String> = raw
                .trim()
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if lsm.iter().any(|x| x == "landlock") {
                Probe::Co { lsm }
            } else {
                Probe::Khong { lsm }
            }
        }
        Err(e) => Probe::KhongRo {
            ly_do: e.to_string(),
        },
    }
}
