// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-20 — **ngân sách MCS** (`SchedContext` budget/period/consumed/refill) cho mô hình TCB.
//!
//! **Model-only `[TK]`, không phải spec/ABI**: D2-49(b) (gắn SC bằng model API
//! `set_sched_params`), D2-49(e) (**chu kỳ thẳng hàng**, cửa sổ trượt/refill sporadic hoãn sau K6
//! — D2-16), D2-49(h) (tick ảo, không timer HW/QEMU). Ngữ nghĩa thiết kế §4.5
//! (`docs/v2/ViFastOs-v2-thiet-ke.md:288-311`), bảng chuyển trạng thái ngân sách `:302-311`:
//! `Running` hết budget ⇒ `Sleeping{reason = Budget}`; biên refill ⇒ `Sleeping{Budget}` +
//! `!suspended` ⇒ `Ready`.
//!
//! Bốn quy ước mô hình (chốt ở đây, test `kt20_budget` kiểm):
//!
//! 1. **Chu kỳ thẳng hàng toàn cục**: [`ScBudget::configure`] đặt `refill` = **bội số kế tiếp**
//!    của `period` tính từ `now`; không có pha riêng từng SC, không cửa sổ trượt.
//! 2. **Charge trước, refill sau**: `Machine::tick` charge TCB đang `Running` cho đơn vị
//!    `[now, now+1)` vào `consumed` **của thời điểm `now`**, rồi mới xử lý biên refill tại thời
//!    điểm mới. Vì thế tổng charge trong `[k·period, (k+1)·period)` ≤ `budget`.
//! 3. **Không dồn budget**: refill đặt `consumed = 0`; phần chưa dùng của chu kỳ cũ mất.
//! 4. **TCB không SC hoặc SC chưa `SchedControl.Configure` KHÔNG có budget** ⇒
//!    `has_budget() == false` ⇒ `pick`/`dispatch` **không chọn** (ngữ nghĩa §4.5: "TCB chỉ chạy
//!    khi có SchedContext còn budget", K1B-PLAN §5 KT-20 DoD 4). TCB muốn chạy phải có SC **đã
//!    Configure**; passive server sẽ chạy bằng donation (KT-21) chứ không chạy vô hạn. Các rig
//!    test KT-18/19/20 gắn một SC mặc định **test-only** cho TCB cần chạy (không đổi production,
//!    không nới kỳ vọng test cũ) — sửa theo verdict reviewer-kt20 (DRIFT mục 1).
//! 5. **Cấu hình lại giữa chu kỳ** (`SchedControl.Configure` lần nữa) đặt `consumed = 0` và
//!    tính lại `refill` theo `period` mới (model-only `[TK]`): bất biến "mỗi chu kỳ thẳng hàng
//!    ≤ budget" áp dụng cho một **cấu hình giữ nguyên trong chu kỳ** (test invariant không
//!    reconfigure giữa chừng; ca reconfigure có test riêng).
//!
//! Mọi phép cộng thời gian dùng `saturating_add`/`saturating_mul` để **không wrap** ở `u64`
//! (K1B-PLAN §5 KT-20 rủi ro (b)); có unit test ở cuối tệp.

/// Số liệu ngân sách của một `SchedContext` — model-only `[TK]`.
///
/// `refill` là **mốc tuyệt đối** (đơn vị tick ảo) của biên nạp lại kế tiếp; `charged` là tổng
/// số tick đã charge từ đầu vòng đời SC (đơn điệu, **không** reset khi refill) — nguồn để test
/// invariant "tổng chạy trong một chu kỳ thẳng hàng ≤ budget".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScBudget {
    budget: u64,
    period: u64,
    consumed: u64,
    refill: u64,
    charged: u64,
}

/// Ảnh chụp `SchedContext` cho test/model API `[TK]` (`Machine::sched_info`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScInfo {
    pub budget: u64,
    pub period: u64,
    pub consumed: u64,
    /// Mốc tuyệt đối của biên nạp lại kế tiếp (`0` khi SC chưa cấu hình).
    pub refill: u64,
    /// Tổng tick đã charge từ đầu vòng đời SC (đơn điệu; không reset khi refill).
    pub charged: u64,
    /// `budget > 0 && period > 0` — chỉ SC đã `SchedControl.Configure` mới bị giới hạn.
    pub configured: bool,
}

impl ScBudget {
    pub fn new() -> Self {
        Self::default()
    }

    /// `SchedControl.Configure` **hợp lệ** (người gọi đã kiểm `budget > 0`, `period > 0`,
    /// `budget ≤ period`): đặt tham số, `consumed = 0`, `refill` = biên toàn cục kế tiếp.
    pub fn configure(&mut self, now: u64, budget: u64, period: u64) {
        debug_assert!(period > 0, "configure: period phải dương");
        self.budget = budget;
        self.period = period;
        self.consumed = 0;
        self.refill = next_boundary(now, period.max(1));
    }

    /// `budget > 0 && period > 0` (SC đã được cấu hình ⇒ có giới hạn).
    pub fn is_configured(&self) -> bool {
        self.budget > 0 && self.period > 0
    }

    pub fn budget(&self) -> u64 {
        self.budget
    }

    pub fn period(&self) -> u64 {
        self.period
    }

    pub fn consumed(&self) -> u64 {
        self.consumed
    }

    pub fn refill(&self) -> u64 {
        self.refill
    }

    pub fn charged(&self) -> u64 {
        self.charged
    }

    /// Budget còn lại của chu kỳ hiện tại; `0` khi SC chưa cấu hình (không có budget).
    pub fn remaining(&self) -> u64 {
        if self.is_configured() {
            self.budget.saturating_sub(self.consumed)
        } else {
            0
        }
    }

    /// TCB dùng SC này có được phép chạy không: SC **phải đã cấu hình** (`budget > 0`,
    /// `period > 0`) **và** `consumed < budget`. SC chưa cấu hình ⇒ không có budget ⇒ `false`
    /// (quy ước 4; không phải "không giới hạn").
    pub fn has_budget(&self) -> bool {
        self.is_configured() && self.consumed < self.budget
    }

    /// Charge **một** tick chạy. `false` nếu không charge (SC chưa cấu hình hoặc đã cạn).
    pub fn charge(&mut self) -> bool {
        if !self.is_configured() || self.consumed >= self.budget {
            return false;
        }
        self.consumed += 1;
        self.charged += 1;
        true
    }

    /// Xử lý biên nạp lại tại `now`: nếu `refill ≤ now` ⇒ `consumed = 0`, đẩy `refill` qua mọi
    /// biên đã trôi (thường đúng một bước vì `tick` đi từng đơn vị), trả `true`.
    pub fn refill_due(&mut self, now: u64) -> bool {
        if !self.is_configured() || self.refill > now {
            return false;
        }
        self.consumed = 0;
        let steps = (now - self.refill) / self.period + 1;
        self.refill = self
            .refill
            .saturating_add(self.period.saturating_mul(steps));
        if self.refill <= now {
            // Bão hoà ở trần `u64`: không thể biểu diễn biên kế tiếp — chốt `u64::MAX`
            // (không wrap, không lặp vô hạn; chỉ chạm được ở now = u64::MAX).
            self.refill = u64::MAX;
        }
        true
    }

    pub fn info(&self) -> ScInfo {
        ScInfo {
            budget: self.budget,
            period: self.period,
            consumed: self.consumed,
            refill: self.refill,
            charged: self.charged,
            configured: self.is_configured(),
        }
    }

    /// Bất biến ngân sách (gọi từ `Machine::check_sched`): `consumed ≤ budget`, `budget`/
    /// `period` dương khi đã cấu hình, `refill` ở tương lai, SC chưa cấu hình thì mọi số 0.
    pub fn check(&self, now: u64) -> Result<(), String> {
        if !self.is_configured() {
            if self.budget == 0 && self.period == 0 && self.consumed == 0 && self.refill == 0 {
                return Ok(());
            }
            return Err(format!(
                "SchedContext chưa cấu hình nhưng budget={} period={} consumed={} refill={}",
                self.budget, self.period, self.consumed, self.refill
            ));
        }
        if self.period == 0 {
            return Err("SchedContext đã cấu hình nhưng period = 0".to_string());
        }
        if self.budget == 0 {
            return Err("SchedContext đã cấu hình nhưng budget = 0".to_string());
        }
        if self.budget > self.period {
            return Err(format!(
                "SchedContext budget {} > period {}",
                self.budget, self.period
            ));
        }
        if self.consumed > self.budget {
            return Err(format!(
                "SchedContext consumed {} > budget {}",
                self.consumed, self.budget
            ));
        }
        if self.refill <= now {
            return Err(format!(
                "SchedContext refill {} ≤ now {} (biên chưa được xử lý)",
                self.refill, now
            ));
        }
        Ok(())
    }
}

/// Biên thẳng hàng kế tiếp sau `now` cho `period` (bội số toàn cục của `period`).
///
/// `period = 0` không xảy ra (người gọi đã kiểm); bão hoà ở `u64::MAX` thay vì wrap.
fn next_boundary(now: u64, period: u64) -> u64 {
    debug_assert!(period > 0, "next_boundary: period phải dương");
    let base = now - now % period;
    base.saturating_add(period)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bien_thang_hang_toan_cuc() {
        assert_eq!(next_boundary(0, 4), 4);
        assert_eq!(next_boundary(1, 4), 4);
        assert_eq!(next_boundary(3, 4), 4);
        assert_eq!(next_boundary(4, 4), 8);
        assert_eq!(next_boundary(5, 4), 8);
        assert_eq!(next_boundary(0, 1), 1);
        assert_eq!(next_boundary(7, 1), 8);
        // now = 3, period 5 ⇒ biên kế tiếp là 5 (không phải 8).
        assert_eq!(next_boundary(3, 5), 5);
    }

    #[test]
    fn bien_bao_hoa_o_tran_khong_wrap() {
        assert_eq!(next_boundary(u64::MAX, 7), u64::MAX);
        assert_eq!(next_boundary(u64::MAX - 1, 2), u64::MAX);
        assert_eq!(next_boundary(0, u64::MAX), u64::MAX);
    }

    #[test]
    fn configure_dat_consumed_0_va_refill_toan_cuc() {
        let mut s = ScBudget::new();
        assert!(!s.is_configured());
        assert!(!s.has_budget(), "chưa cấu hình ⇒ không có budget");
        assert_eq!(s.remaining(), 0);
        s.configure(3, 2, 5);
        assert!(s.is_configured());
        assert_eq!((s.budget(), s.period()), (2, 5));
        assert_eq!((s.consumed(), s.refill()), (0, 5));
        assert_eq!(s.remaining(), 2);
        // Cấu hình lại ở now = 5 ⇒ biên 10; consumed đặt lại 0.
        assert!(s.charge());
        assert_eq!(s.consumed(), 1);
        s.configure(5, 3, 5);
        assert_eq!((s.consumed(), s.refill(), s.charged()), (0, 10, 1));
    }

    #[test]
    fn sc_chua_cau_hinh_khong_co_budget() {
        let mut s = ScBudget::new();
        assert!(!s.has_budget());
        assert!(!s.charge(), "chưa cấu hình ⇒ không charge được");
        assert_eq!((s.consumed(), s.charged()), (0, 0));
        assert!(!s.refill_due(100), "chưa cấu hình ⇒ không có biên refill");
        s.check(0).expect("trạng thái 0 hợp lệ");
    }

    #[test]
    fn charge_dung_bien_budget() {
        let mut s = ScBudget::new();
        s.configure(0, 2, 4);
        assert!(s.has_budget());
        assert!(s.charge());
        assert!(s.has_budget());
        assert!(s.charge());
        assert_eq!((s.consumed(), s.charged()), (2, 2));
        assert!(!s.has_budget());
        assert!(!s.charge(), "đã cạn ⇒ không charge (không budget+1)");
        assert_eq!(s.consumed(), 2);
        assert_eq!(s.remaining(), 0);
    }

    #[test]
    fn refill_due_tai_bien_va_nhay_nhieu_bien() {
        let mut s = ScBudget::new();
        s.configure(0, 2, 4);
        assert!(!s.refill_due(3));
        assert_eq!(s.refill(), 4);
        assert!(s.charge());
        assert!(s.refill_due(4));
        assert_eq!((s.consumed(), s.refill()), (0, 8));
        // Nhảy qua nhiều biên (now rất xa): 8 → 16.
        assert!(s.refill_due(12));
        assert_eq!(s.refill(), 16);
        assert!(!s.refill_due(15));
    }

    #[test]
    fn refill_due_khi_now_o_tran_khong_lap() {
        let mut s = ScBudget::new();
        s.configure(u64::MAX - 10, 2, 4);
        let r0 = s.refill();
        assert!(r0 > u64::MAX - 10 && r0 < u64::MAX);
        // now = trần: đẩy refill bão hoà u64::MAX, không lặp vô hạn/không wrap.
        assert!(s.refill_due(u64::MAX));
        assert_eq!(s.refill(), u64::MAX);
        assert_eq!(s.consumed(), 0);
    }

    #[test]
    fn check_bat_budget_period_va_consumed_sai() {
        let mut s = ScBudget::new();
        s.check(0).expect("chưa cấu hình hợp lệ");
        s.configure(0, 2, 4);
        s.check(0).expect("cấu hình hợp lệ");
        // Các trạng thái sai (chỉ dựng được trong unit test vì field private).
        let mut sai = s;
        sai.budget = 0;
        assert!(sai.check(0).is_err(), "budget 0");
        let mut sai = s;
        sai.period = 0;
        assert!(sai.check(0).is_err(), "period 0");
        let mut sai = s;
        sai.budget = 5;
        assert!(sai.check(0).is_err(), "budget > period");
        let mut sai = s;
        sai.consumed = 3;
        assert!(sai.check(0).is_err(), "consumed > budget");
        let mut sai = s;
        sai.refill = 0;
        assert!(sai.check(0).is_err(), "refill ≤ now");
    }
}
