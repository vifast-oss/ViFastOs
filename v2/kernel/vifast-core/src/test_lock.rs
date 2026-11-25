// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-408** — khoá **dùng chung** cho mọi ca test đọc **bộ đếm toàn cục** `invoke::stats()`.
//!
//! Trước KT-408 mỗi mô-đun test tự khai một `Mutex` riêng (`SUSPEND_COUNTER_LOCK`,
//! `CONFIGURE_COUNTER_LOCK`, `RESUME_COUNTER_LOCK`, `TLS_COUNTER_LOCK`) nhưng **cùng** đọc một bộ
//! đếm `static` toàn cục ⇒ hai mô-đun khác nhau vẫn chạy **song song** và làm lệch khẳng định
//! `== base + n` của nhau. Đo được trên nhánh KT-408: `cargo test -p vifast-core --lib` lặp 25
//! lượt có **1 lượt ĐỎ** `resume_tests::resume_inactive_chuyen_running_va_giao_entry`
//! (`stats().resume == base + 1`) — race **có sẵn**, không phải do KT-408 thêm, nhưng làm cổng
//! T1/T2 chập chờn.
//!
//! Sửa **không nới ngưỡng**: giữ nguyên khẳng định `==`, chỉ **nối tiếp** các ca với nhau bằng
//! **một** khoá. Đây là khoá test, không vào ảnh sản phẩm (`#[cfg(test)]`).

extern crate std;

/// Khoá nối tiếp cho mọi ca đọc/khẳng định `invoke::stats()`.
pub(crate) static STATS_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
