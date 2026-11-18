// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-150 (N3)** — checker "giao đúng một lần" của pha pager.
//!
//! Tách khỏi `ktest_vfs.rs` để giữ luật cấu trúc R1 (mỗi tệp kernel/lib ≤ 1500 dòng) — cùng lý
//! do `pager_fault.rs` được tách khỏi `user.rs` ở KT-70.
//!
//! Vì sao cần checker: v1 (T31) in `PAGER EVICT OK` **hai lần** ⇒ nhánh xử lý chạy ≥ 3 lần ⇒ có
//! message bị giao/xử lý lặp. v2 tin là đúng nhưng **thiếu bằng chứng**; checker này biến niềm
//! tin đó thành phép đo **fail-closed** (thiếu dòng / đếm không được ⇒ ĐỎ, không "bỏ qua").

/// Sổ giao `pager.fault` **theo từng địa chỉ**.
pub struct DeliveryLedger {
    /// Tổng số message kernel **đã gửi**.
    pub deliveries: u64,
    /// `(va, số lần giao)` — sổ theo địa chỉ.
    pub va_counts: [(u64, u64); 16],
    /// Số địa chỉ phân biệt đã ghi sổ.
    pub n_va: usize,
}

impl DeliveryLedger {
    /// Sổ rỗng.
    pub const fn new() -> Self {
        Self {
            deliveries: 0,
            va_counts: [(0, 0); 16],
            n_va: 0,
        }
    }

    /// Ghi **một** lần giao cho `va`.
    pub fn record(&mut self, va: u64) {
        self.deliveries += 1;
        let mut index = 0usize;
        while index < self.n_va {
            if self.va_counts[index].0 == va {
                self.va_counts[index].1 += 1;
                return;
            }
            index += 1;
        }
        if self.n_va < self.va_counts.len() {
            self.va_counts[self.n_va] = (va, 1);
            self.n_va += 1;
        }
    }

    /// Số lần đã giao cho `va` (`0` = chưa từng giao ⇒ checker đỏ).
    pub fn delivered_to(&self, va: u64) -> u64 {
        let mut index = 0usize;
        while index < self.n_va {
            if self.va_counts[index].0 == va {
                return self.va_counts[index].1;
            }
            index += 1;
        }
        0
    }
}

impl Default for DeliveryLedger {
    fn default() -> Self {
        Self::new()
    }
}

/// Checker **fail-closed**: `true` ⇔ mỗi fault được giao **đúng một lần** và pager xử lý **đúng
/// một lần** mỗi request.
///
/// `expected` = `(va, số lần bảng client yêu cầu)` — trang nạp lại sau trục xuất là 2, còn lại 1;
/// số này **suy từ bảng** (`A_TABLE`/`B_TABLE`), không viết tay. Thiếu dữ liệu (sổ rỗng, VA không
/// có trong sổ, `faults == 0`) ⇒ `false`.
pub fn check_once(
    ledger: &DeliveryLedger,
    expected: &[(u64, u64)],
    faults: u64,
    served: u64,
    rejected: u64,
) -> bool {
    if ledger.deliveries == 0 || ledger.n_va == 0 || faults == 0 || expected.is_empty() {
        return false;
    }
    let mut planned = 0u64;
    for (va, want) in expected {
        planned += *want;
        if ledger.delivered_to(*va) != *want {
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
