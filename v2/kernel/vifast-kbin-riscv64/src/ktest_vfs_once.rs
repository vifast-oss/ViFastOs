// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-150 (N3) / KT-157** — checker "giao đúng một lần" của pha pager, sổ theo `(client, va)`.
//!
//! Tách khỏi `ktest_vfs.rs` để giữ luật cấu trúc R1 (mỗi tệp kernel/lib ≤ 1500 dòng) — cùng lý
//! do `pager_fault.rs` được tách khỏi `user.rs` ở KT-70.
//!
//! Vì sao cần checker: v1 (T31) in `PAGER EVICT OK` **hai lần** ⇒ nhánh xử lý chạy ≥ 3 lần ⇒ có
//! message bị giao/xử lý lặp. v2 tin là đúng nhưng **thiếu bằng chứng**; checker này biến niềm
//! tin đó thành phép đo **fail-closed** (thiếu dòng / đếm không được ⇒ ĐỎ, không "bỏ qua").
//!
//! **KT-157** mở rộng sổ từ "theo VA" thành "theo `(client, va)`": bốn tiến trình U-mode có thể
//! dùng **cùng** một VA số học trong bốn VSpace khác nhau — gộp theo VA sẽ không phân biệt được
//! client nào chưa được giao. Sổ giữ cả hai khóa; `check_once` đòi **đúng** từng cặp.

/// Số cặp `(client, va)` tối đa — 4 client × 8 VA = 32, dư cho bảng hiện tại (11 cặp).
pub const LEDGER_SLOTS: usize = 32;

/// Sổ giao `pager.fault` **theo từng `(client, va)`**.
pub struct DeliveryLedger {
    /// Tổng số message kernel **đã gửi**.
    pub deliveries: u64,
    /// `(client, va, số lần giao)` — sổ theo cặp.
    pub entries: [(u8, u64, u64); LEDGER_SLOTS],
    /// Số cặp phân biệt đã ghi sổ.
    pub n_entries: usize,
}

impl DeliveryLedger {
    /// Sổ rỗng.
    pub const fn new() -> Self {
        Self {
            deliveries: 0,
            entries: [(0, 0, 0); LEDGER_SLOTS],
            n_entries: 0,
        }
    }

    /// Ghi **một** lần giao cho `(client, va)`.
    pub fn record(&mut self, client: u8, va: u64) {
        self.deliveries += 1;
        let mut index = 0usize;
        while index < self.n_entries {
            if self.entries[index].0 == client && self.entries[index].1 == va {
                self.entries[index].2 += 1;
                return;
            }
            index += 1;
        }
        if self.n_entries < self.entries.len() {
            self.entries[self.n_entries] = (client, va, 1);
            self.n_entries += 1;
        }
    }

    /// Số lần đã giao cho `(client, va)` (`0` = chưa từng giao ⇒ checker đỏ).
    pub fn delivered_to(&self, client: u8, va: u64) -> u64 {
        let mut index = 0usize;
        while index < self.n_entries {
            if self.entries[index].0 == client && self.entries[index].1 == va {
                return self.entries[index].2;
            }
            index += 1;
        }
        0
    }

    /// Số **client phân biệt** đã có ít nhất một lần giao — bằng chứng `clients=4`.
    pub fn distinct_clients(&self) -> u64 {
        let mut seen = [false; 256];
        let mut count = 0u64;
        let mut index = 0usize;
        while index < self.n_entries {
            let client = self.entries[index].0 as usize;
            if !seen[client] {
                seen[client] = true;
                count += 1;
            }
            index += 1;
        }
        count
    }
}

impl Default for DeliveryLedger {
    fn default() -> Self {
        Self::new()
    }
}

/// Checker **fail-closed**: `true` ⇔ mỗi fault được giao **đúng một lần** cho **đúng client** và
/// pager xử lý **đúng một lần** mỗi request.
///
/// `expected` = `(client, va, số lần bảng client yêu cầu)` — trang nạp lại sau trục xuất là 2,
/// còn lại 1; số này **suy từ bảng**, không viết tay. Thiếu dữ liệu (sổ rỗng, cặp không có trong
/// sổ, `faults == 0`, `expected` rỗng) ⇒ `false`. **Không** có nhánh "bỏ qua".
pub fn check_once(
    ledger: &DeliveryLedger,
    expected: &[(u8, u64, u64)],
    faults: u64,
    served: u64,
    rejected: u64,
) -> bool {
    if ledger.deliveries == 0 || ledger.n_entries == 0 || faults == 0 || expected.is_empty() {
        return false;
    }
    let mut planned = 0u64;
    for (client, va, want) in expected {
        planned += *want;
        if ledger.delivered_to(*client, *va) != *want {
            return false;
        }
    }
    // Kernel gửi đúng bằng kế hoạch; pager nhận đúng bằng số đã gửi (message giao lặp ⇒ lớn hơn).
    if ledger.deliveries != planned || faults != planned {
        return false;
    }
    // Mỗi request được xử lý **đúng một** lần: một `served` hoặc một `rejected`.
    served + rejected == faults
}
