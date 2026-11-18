// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-152 (N1)** — đồng hồ **wall monotonic**: một nguồn hệ thống, nhiều nguồn theo hart.
//!
//! Mô hình hoá **đúng cơ chế v1 T33a** ([`docs/v1/KNOWN-ISSUES.md`] dòng 1, `[V1:ĐO]`):
//!
//! - v1 có **bộ đếm theo hart** (`Kernel::ticks: [u64; MAX_HARTS]`) **và** một đồng hồ **hệ
//!   thống** (`system_ticks`) mà `GetTime` đọc — `fast_time()` = **max-update** trên mọi hart
//!   (r294.B, `v1-archive:crates/vifast-kernel/src/lib.rs:6543-6552`);
//! - `Kernel::reset()` (gọi bởi `init_kernel()`) từng đưa `system_ticks` về **0** ⇒ task đang
//!   chạy thấy `GetTime` **nhảy lùi** ⇒ worker C `panic!("GetTime went backwards on hart 1")`
//!   (`v1-archive:crates/vifast-root/src/main.rs:491-492` trước r323) ⇒ root **chờ mãi** message
//!   `WORKER_LABEL_C` ⇒ treo sau `shell>`, không in `PREEMPT OK`.
//!
//! Ở đây:
//!
//! - [`MonotonicClock::tick_on`] nâng bộ đếm của **một** hart rồi cập nhật đồng hồ hệ thống bằng
//!   **max-update** — đơn điệu, không phụ thuộc hart nào đang chạy;
//! - [`MonotonicClock::read`] trả **đồng hồ hệ thống** cho **mọi** hart (một thứ tự duy nhất);
//! - [`MonotonicClock::reset_hart`] mô hình `reset()` trên một hart **muộn** và **không** được
//!   đụng đồng hồ hệ thống.
//!
//! [`MonotonicClock::backwards`] là **bộ đếm an ninh**: đúng thì luôn `0`, và **ca âm** của
//! KT-152 (đột biến `read` trả bộ đếm theo hart) chứng minh nó **bắt được** lỗi — đúng
//! D2-108(B) ("bộ đếm luôn 0 mà không có ca âm thì không chứng minh gì").
//!
//! **Model-only `[TK]`, không phải spec/ABI.** Không đọc `Instant`/wall clock (D2-49(h)); mọi
//! giá trị là số nguyên do kịch bản đẩy vào nên quỹ đạo tất định theo seed.
//!
//! [`docs/v1/KNOWN-ISSUES.md`]: ../../../docs/v1/KNOWN-ISSUES.md

use crate::kobj::MAX_HARTS;

/// Đồng hồ wall monotonic của mô hình — **một** nguồn hệ thống + bộ đếm theo hart.
///
/// Bất biến (kiểm bởi [`MonotonicClock::check`] và bởi test KT-152):
///
/// - `system >= per_hart[h]` với **mọi** hart `h` (max-update: đồng hồ hệ thống không bao giờ
///   thấp hơn nguồn nào);
/// - `system` **không giảm** qua mọi thao tác, kể cả [`MonotonicClock::reset_hart`];
/// - mọi lần [`MonotonicClock::read`] (bất kể hart nào) tạo thành dãy **không giảm**.
#[derive(Debug, Clone)]
pub struct MonotonicClock {
    /// Đồng hồ **hệ thống** — giá trị mà **mọi** hart đọc (v1 `fast_time()`).
    system: u64,
    /// Bộ đếm **theo hart** (v1 `Kernel::ticks`) — **nguồn**, không phải giá trị đọc.
    per_hart: [u64; MAX_HARTS as usize],
    /// Giá trị của lần [`Self::read`] **trước** (để phát hiện lùi trên dãy đọc chéo hart).
    last_read: Option<u64>,
    /// Số lần [`Self::read`] trả giá trị **nhỏ hơn** lần đọc trước — bộ đếm an ninh, phải `0`.
    backwards: u64,
    /// Tổng số lần [`Self::read`] (nguồn của "đã đọc thật, không phải hằng").
    reads: u64,
}

impl Default for MonotonicClock {
    fn default() -> Self {
        Self::new()
    }
}

impl MonotonicClock {
    /// Đồng hồ mới: hệ thống `0`, mọi bộ đếm hart `0`, chưa đọc lần nào.
    pub fn new() -> Self {
        Self {
            system: 0,
            per_hart: [0; MAX_HARTS as usize],
            last_read: None,
            backwards: 0,
            reads: 0,
        }
    }

    /// Hart `hart` có **nguồn** thời gian riêng (như v1 `Kernel::ticks[hart]`).
    ///
    /// `hart >= MAX_HARTS` ⇒ `false` (không panic; caller kiểm trước).
    pub fn has_hart(&self, hart: usize) -> bool {
        hart < MAX_HARTS as usize
    }

    /// Nâng bộ đếm của **một** hart thêm `n` rồi cập nhật đồng hồ hệ thống bằng **max-update**
    /// (v1 r294.B: "AP ticks advance system time by max-update").
    ///
    /// Trả `false` (không đổi gì) nếu `hart` ngoài dải — fail rõ, không panic.
    pub fn tick_on(&mut self, hart: usize, n: u64) -> bool {
        if !self.has_hart(hart) {
            return false;
        }
        let slot = &mut self.per_hart[hart];
        *slot = slot.saturating_add(n);
        let now = *slot;
        if now > self.system {
            self.system = now;
        }
        true
    }

    /// Đọc đồng hồ **hệ thống** "từ" hart `hart` — **mọi** hart nhận **cùng** một giá trị tại
    /// cùng một điểm của kịch bản (đó là nội dung của "conformance chéo hart").
    ///
    /// Ghi nhận vi phạm đơn điệu vào [`Self::backwards`] nếu giá trị **nhỏ hơn** lần đọc trước
    /// (bất kể hart nào đọc) — dãy đọc chéo hart phải **không giảm**.
    ///
    /// Trả `None` nếu `hart` ngoài dải.
    pub fn read(&mut self, hart: usize) -> Option<u64> {
        if !self.has_hart(hart) {
            return None;
        }
        let v = self.system;
        if let Some(last) = self.last_read {
            if v < last {
                self.backwards = self.backwards.saturating_add(1);
            }
        }
        self.last_read = Some(v);
        self.reads = self.reads.saturating_add(1);
        Some(v)
    }

    /// Mô hình `Kernel::reset()` chạy trên **một hart muộn** (v1 `init_kernel()`): đưa **bộ đếm
    /// của hart đó** về `0` nhưng **giữ nguyên** đồng hồ hệ thống.
    ///
    /// Đây đúng là bản vá r323 của v1 (`v1-archive:crates/vifast-kernel/src/lib.rs:415-427`:
    /// "KHÔNG đưa đồng hồ hệ thống về 0 trong `reset()`"). Trả `false` nếu `hart` ngoài dải.
    pub fn reset_hart(&mut self, hart: usize) -> bool {
        if !self.has_hart(hart) {
            return false;
        }
        self.per_hart[hart] = 0;
        true
    }

    /// Đồng hồ hệ thống hiện tại (không ghi nhận lần đọc).
    pub fn system(&self) -> u64 {
        self.system
    }

    /// Bộ đếm **nguồn** của hart `hart` (`None` nếu ngoài dải).
    pub fn per_hart(&self, hart: usize) -> Option<u64> {
        if self.has_hart(hart) {
            Some(self.per_hart[hart])
        } else {
            None
        }
    }

    /// Số lần đọc đã trả giá trị **nhỏ hơn** lần trước — bộ đếm an ninh, đúng thì `0`.
    pub fn backwards(&self) -> u64 {
        self.backwards
    }

    /// Tổng số lần đọc (chứng minh phép kiểm không rỗng).
    pub fn reads(&self) -> u64 {
        self.reads
    }

    /// Bất biến cấu trúc: `system >= per_hart[h]` với mọi hart (max-update đúng).
    pub fn check(&self) -> Result<(), String> {
        for (h, v) in self.per_hart.iter().enumerate() {
            if *v > self.system {
                return Err(format!(
                    "hart {h}: nguồn {v} > đồng hồ hệ thống {} (max-update sai)",
                    self.system
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_update_va_doc_cheo_hart() {
        let mut c = MonotonicClock::new();
        assert!(c.tick_on(0, 5));
        assert_eq!(c.system(), 5);
        assert!(c.tick_on(1, 2));
        // max-update: hart 1 chỉ tới 2 nên hệ thống **vẫn** 5.
        assert_eq!(c.system(), 5);
        assert_eq!(c.read(0), Some(5));
        assert_eq!(c.read(1), Some(5));
        assert_eq!(c.backwards(), 0);
        assert_eq!(c.reads(), 2);
        c.check().expect("max-update đúng");
    }

    #[test]
    fn reset_hart_khong_dua_dong_ho_ve_0() {
        let mut c = MonotonicClock::new();
        assert!(c.tick_on(0, 7));
        assert!(c.reset_hart(1));
        assert_eq!(
            c.system(),
            7,
            "reset hart muộn không được đụng đồng hồ hệ thống"
        );
        assert_eq!(c.per_hart(1), Some(0));
        assert_eq!(c.read(0), Some(7));
        assert_eq!(c.backwards(), 0);
    }

    #[test]
    fn hart_ngoai_dai_bi_tu_choi_khong_panic() {
        let mut c = MonotonicClock::new();
        assert!(!c.tick_on(MAX_HARTS as usize, 1));
        assert!(!c.reset_hart(MAX_HARTS as usize));
        assert_eq!(c.read(MAX_HARTS as usize), None);
        assert_eq!(c.system(), 0);
        assert_eq!(c.reads(), 0);
    }
}
