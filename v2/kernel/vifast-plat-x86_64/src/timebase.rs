// SPDX-License-Identifier: MIT OR Apache-2.0
//! Đồng hồ **PIT kênh 2** — nguồn thời gian không cần ngắt cho bước mạng — KT-274 (K5' bước 5).
//!
//! # Vì sao không dùng `TICK_COUNT`
//!
//! Nguồn tick của ảnh là PIT kênh 0 → IOAPIC → LAPIC (KT-266), nhưng đường đó cần **ngắt bật**.
//! Bước PCI/virtio-net chạy sau `cli` (và trước U-mode) nên không dựa vào nó được. PIT **kênh 2**
//! đã được `measure_timer` lập trình mode 2 divisor [`PIT_CH2_DIVISOR`] và **không** phát ngắt ⇒
//! dùng làm đồng hồ đếm-lùi tự do: mỗi lần đọc là một ảnh 16 bit, cộng dồn chênh lệch (xử lý bọc)
//! ra số count đã trôi qua, rồi đổi sang µs bằng tần số PIT thật.
//!
//! # Vì sao tách khỏi `asm`
//!
//! Phần **số học** ([`PitClock`], [`elapsed_us`]) là thuần ⇒ test host chốt được (D2-05). Phần
//! chạm cổng I/O (`outb`/`inb`) nằm ở ảnh (`vifast-kbin-x86_64::pit2_latch_read`) và đi qua
//! [`crate::asm`] — tệp này **không** có `unsafe`.

pub use crate::apic::{PIT_FREQUENCY_HZ, PIT_MAX_DIVISOR as PIT_CH2_DIVISOR};

/// Số count của một chu kỳ đầy kênh 2 (`divisor + 1` — counter đếm `divisor..0` rồi nạp lại).
pub const PIT_CH2_WRAP_COUNTS: u64 = 0x1_0000;

/// Đổi số count PIT sang µs (làm tròn xuống, bão hoà khi tràn).
pub const fn elapsed_us(counts: u64) -> u64 {
    counts.saturating_mul(1_000_000) / PIT_FREQUENCY_HZ as u64
}

/// Bộ cộng dồn count của PIT kênh 2 — xử lý **bọc** (counter nạp lại sau mỗi chu kỳ).
///
/// Người gọi đưa **giá trị thô** đọc từ counter (đếm lùi); `update` trả tổng số count đã trôi qua
/// kể từ [`PitClock::reset`]. Một lần đọc cách nhau **ít hơn** một chu kỳ (54,9 ms) là điều kiện
/// đúng — vòng poll của driver cách nhau vài µs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PitClock {
    last: u16,
    counts: u64,
    primed: bool,
}

impl PitClock {
    /// Đồng hồ chưa có mốc — lần [`PitClock::update`] đầu tiên chỉ đặt mốc, trả 0.
    pub const fn new() -> Self {
        Self {
            last: 0,
            counts: 0,
            primed: false,
        }
    }

    /// Đặt mốc hiện tại = `raw` và xoá bộ đếm.
    pub fn reset(&mut self, raw: u16) {
        self.last = raw;
        self.counts = 0;
        self.primed = true;
    }

    /// Nạp ảnh counter mới; trả **tổng** count đã trôi qua kể từ mốc.
    ///
    /// Counter đếm **lùi**: `raw <= last` ⇒ `last - raw`; `raw > last` ⇒ đã bọc ⇒
    /// `last + 0x1_0000 - raw`. Lần gọi đầu (chưa có mốc) đặt mốc và trả 0.
    pub fn update(&mut self, raw: u16) -> u64 {
        if !self.primed {
            self.reset(raw);
            return 0;
        }
        let delta = if raw <= self.last {
            (self.last - raw) as u64
        } else {
            (self.last as u64) + PIT_CH2_WRAP_COUNTS - (raw as u64)
        };
        self.last = raw;
        self.counts = self.counts.saturating_add(delta);
        self.counts
    }

    /// Tổng count đã trôi qua (không nạp ảnh mới).
    pub const fn counts(&self) -> u64 {
        self.counts
    }

    /// Nạp ảnh mới và trả thời gian đã trôi qua tính bằng µs.
    pub fn now_us(&mut self, raw: u16) -> u64 {
        elapsed_us(self.update(raw))
    }
}
