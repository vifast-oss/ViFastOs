// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-336b** — miền `clock`: đồng hồ wall + tick ngân sách.
//!
//! Tách nguyên văn từ `kobj.rs` (D2-232/D2-238): **chỉ đổi vị trí** + `pub(super)` cho hàm
//! nội bộ cần gọi chéo module; thân hàm, thứ tự op và API công khai giữ nguyên. Đây là
//! module con của [`crate::kobj`] nên thấy item private của `kobj` (cùng mẫu `state_write`).

use super::*;

impl Machine {
    // ------------------------------------------- ngân sách MCS (KT-20) `[TK]`
    //
    // Model-only `[TK]`, **không** phải spec/ABI (D2-49(b,e,h)): `SchedContext` có
    // `budget`/`period`/`consumed`/`refill`; thời gian là đồng hồ ảo `tick(n)`; TCB chỉ chạy
    // khi SC còn budget, hết budget ⇒ `Sleeping{reason = Budget}` qua `transition_tcb`, biên
    // refill ⇒ `Ready` nếu `!suspended`. **Không** làm refill sporadic/timeout fault (D2-16).
    // **Donation** (`Call`/`Reply`, passive server, MCP) là phần KT-21 ở mục riêng ngay dưới:
    // charge/refill vẫn tính trên cùng một `SchedContext`, nên ngân sách của client bị trừ đúng
    // bằng thời gian server phục vụ (D2-49(c)).

    /// Thời gian ảo hiện tại (đơn vị tick) — **không** đọc `Instant`/wall clock (D2-49(h)).
    pub fn virtual_now(&self) -> u64 {
        self.now
    }

    // ------------------------------------------- đồng hồ wall monotonic (KT-152 / N1)
    //
    // **Model-only `[TK]`**, không phải spec/ABI. Mô hình cơ chế v1 T33a: mỗi hart có **nguồn**
    // thời gian riêng, đồng hồ **hệ thống** là max-update trên mọi nguồn, và **mọi** hart đọc
    // cùng một giá trị. `reset()` của một hart muộn (v1 `init_kernel()`) **không** được đụng
    // đồng hồ hệ thống. Xem [`crate::clock`] và `tests/kt152_n1.rs`.

    /// **KT-152** — hart `hart` nâng **nguồn** thời gian của mình thêm `n` (mô hình ngắt timer
    /// của hart đó); đồng hồ hệ thống nhận **max-update** nên **không** hart nào làm nó lùi.
    ///
    /// `hart` ngoài dải ⇒ `InvalidArgument` (không panic).
    pub fn clock_tick_on(&mut self, hart: u16, n: u64) -> Result<(), KError> {
        if hart as usize >= self.sched.len() {
            return Err(KError::InvalidArgument);
        }
        if !self.clock.tick_on(hart as usize, n) {
            return Err(KError::InvalidArgument);
        }
        Ok(())
    }

    /// **KT-152** — đọc đồng hồ **wall** "từ" hart `hart`: **mọi** hart nhận **cùng** một giá
    /// trị tại cùng một điểm của kịch bản (một thứ tự monotonic duy nhất — "conformance chéo
    /// hart"). Mỗi lần đọc được ghi vào bộ đếm an ninh [`Machine::clock_backwards`].
    ///
    /// `hart` ngoài dải ⇒ `InvalidArgument`.
    pub fn clock_read(&mut self, hart: u16) -> Result<u64, KError> {
        if hart as usize >= self.sched.len() {
            return Err(KError::InvalidArgument);
        }
        self.clock
            .read(hart as usize)
            .ok_or(KError::InvalidArgument)
    }

    /// **KT-152** — mô hình `Kernel::reset()` chạy trên **một hart muộn** (v1 `init_kernel()`):
    /// dựng lại scheduler + hộp thư **của hart đó** và đưa **nguồn** thời gian của nó về `0`,
    /// nhưng **giữ nguyên** đồng hồ hệ thống (bản vá r323 của v1).
    ///
    /// Chỉ hợp lệ khi hart **rảnh và rỗng** (không `current`, không TCB trong hàng, không message
    /// chờ) — nếu không trả `IllegalOperation` thay vì phá bất biến scheduler.
    pub fn hart_reinit(&mut self, hart: u16) -> Result<(), KError> {
        let h = hart as usize;
        if h >= self.sched.len() {
            return Err(KError::InvalidArgument);
        }
        if !self.sched[h].idle() || !self.sched[h].is_empty() || !self.mail[h].is_empty() {
            return Err(KError::IllegalOperation);
        }
        self.sched[h] = HartSched::new();
        self.mail[h] = HartMailbox::new();
        if !self.clock.reset_hart(h) {
            return Err(KError::InvalidArgument);
        }
        self.refresh_published();
        Ok(())
    }

    /// **KT-152** — đồng hồ wall hệ thống hiện tại (không ghi nhận lần đọc).
    pub fn clock_system(&self) -> u64 {
        self.clock.system()
    }

    /// **KT-152** — **nguồn** thời gian của hart `hart` (`None` nếu ngoài dải **của mô hình**,
    /// tức `hart >= hart_count()` — không phải `MAX_HARTS`).
    pub fn clock_per_hart(&self, hart: u16) -> Option<u64> {
        if hart as usize >= self.sched.len() {
            return None;
        }
        self.clock.per_hart(hart as usize)
    }

    /// **KT-152** — số lần đọc đã trả giá trị **nhỏ hơn** lần đọc trước (bộ đếm an ninh; đúng
    /// thì `0`, và có **ca âm** chứng minh nó bắt được lỗi — D2-108(B)).
    pub fn clock_backwards(&self) -> u64 {
        self.clock.backwards()
    }

    /// **KT-152** — tổng số lần [`Machine::clock_read`] (chứng minh phép kiểm không rỗng).
    pub fn clock_reads(&self) -> u64 {
        self.clock.reads()
    }

    /// **KT-152** — bất biến đồng hồ: `system >= per_hart[h]` với mọi hart (max-update đúng).
    pub fn check_clock(&self) -> Result<(), String> {
        self.clock.check()
    }

    /// `tick(n)` — chạy đồng hồ ảo `n` đơn vị; tất định, cùng kịch bản ⇒ cùng mốc refill.
    ///
    /// Mỗi đơn vị: charge TCB đang `Running` cho `[now, now+1)`, `now += 1`, rồi xử lý biên
    /// refill tại `now` mới (charge trước — refill sau, xem `mcs`). **Không** tự `dispatch`:
    /// chọn TCB vẫn là thao tác tường minh như KT-19.
    pub fn tick(&mut self, n: u64) -> Result<(), KError> {
        for _ in 0..n {
            self.tick_one()?;
        }
        self.sched_after_op(false);
        Ok(())
    }

    pub(super) fn tick_one(&mut self) -> Result<(), KError> {
        // 1) Charge TCB đang chạy của từng hart (K1b: 1 hart — D2-49(a)); hết budget ⇒
        //    `Running → Sleeping{Budget}` ngay, gỡ `current` qua `sched_sync`.
        for h in 0..self.sched.len() {
            if let Some(cur) = self.sched[h].current() {
                self.charge_tick(cur)?;
            }
        }
        // 2) Thời gian ảo tiến 1 (bão hoà ở trần `u64`, không wrap).
        self.now = self.now.saturating_add(1);
        // 3) Biên refill tại `now` mới: `Sleeping{Budget} + !suspended → Ready`.
        self.process_refills()
    }

    /// Charge **một** tick cho TCB đang chạy; hết budget ⇒ `Sleeping{Budget}` (một cửa
    /// [`Machine::transition_tcb`]). TCB không SC cấp ngân sách/SC chưa cấu hình không bị charge.
    /// **KT-21**: SC cấp ngân sách có thể là SC **được donate** — charge vào SC của client.
    pub(super) fn charge_tick(&mut self, tcb: ObjId) -> Result<(), KError> {
        let info = self.tcb_info_obj(tcb)?;
        if !matches!(info.state, TcbState::Running { .. }) {
            return Ok(());
        }
        // KT-21: SC cấp ngân sách = SC **riêng** (không bị donate) hoặc SC **được donate** cho
        // TCB (server passive). Charge vào chính SC đó ⇒ thời gian server chạy trừ ngân sách của
        // client (D2-49(c)); SC của client có thể đã cạn ⇒ server `Sleeping{Budget}`.
        let Some(sc) = self.funding_sc(tcb) else {
            // Không SC = không có budget ⇒ không chạy (chỉ tới `Running` bằng transition trực
            // tiếp; `pick` đã chặn từ đầu, D2-50).
            self.transition_tcb(
                tcb,
                NonReadyTarget::Sleeping {
                    reason: SleepReason::Budget,
                },
            )?;
            return Ok(());
        };
        // SC chưa `SchedControl.Configure` = **không có budget** ⇒ không charge; nếu TCB đang
        // `Running` (chỉ tới được bằng `transition_tcb` trực tiếp, không qua `pick`), đưa về
        // `Sleeping{Budget}` để giữ "không có budget ⇒ không chạy".
        let configured = self.body(sc, |k| match k {
            Kind::SchedContext(s) => Ok(s.budget.is_configured()),
            _ => Err(KError::WrongObjectType),
        })?;
        if !configured {
            self.transition_tcb(
                tcb,
                NonReadyTarget::Sleeping {
                    reason: SleepReason::Budget,
                },
            )?;
            return Ok(());
        }
        let charged = self.body_mut(sc, |k| match k {
            Kind::SchedContext(s) => Ok(s.budget.charge()),
            _ => Err(KError::WrongObjectType),
        })?;
        let can = self.tcb_has_budget(tcb);
        if !charged || !can {
            self.transition_tcb(
                tcb,
                NonReadyTarget::Sleeping {
                    reason: SleepReason::Budget,
                },
            )?;
        }
        Ok(())
    }

    /// Xử lý biên nạp lại tại `now` cho mọi `SchedContext` sống: `consumed = 0`, rồi đánh
    /// thức TCB `Sleeping{Budget}` **không** `suspended` đang gắn SC đó.
    pub(super) fn process_refills(&mut self) -> Result<(), KError> {
        let now = self.now;
        let scs: Vec<ObjId> = self
            .objects
            .iter()
            .enumerate()
            .filter(|(_, m)| m.alive && m.ty == ObjType::SchedContext)
            .map(|(i, _)| (i as u32) + 1)
            .collect();
        let mut refilled: Vec<ObjId> = Vec::new();
        for sc in scs {
            let did = self.body_mut(sc, |k| match k {
                Kind::SchedContext(s) => Ok(s.budget.refill_due(now)),
                _ => Err(KError::WrongObjectType),
            })?;
            if did {
                // KT-23: đếm lần nạp lại (một SC một lần/chu kỳ) cho `k1b_report`.
                self.budget_refills += 1;
                refilled.push(sc);
            }
        }
        if refilled.is_empty() {
            return Ok(());
        }
        let tcbs: Vec<ObjId> = self
            .objects
            .iter()
            .enumerate()
            .filter(|(_, m)| m.alive && m.ty == ObjType::Tcb)
            .map(|(i, _)| (i as u32) + 1)
            .collect();
        let mut wake: Vec<ObjId> = Vec::new();
        for tcb in tcbs {
            let info = self.tcb_info_obj(tcb)?;
            if matches!(
                info.state,
                TcbState::Sleeping {
                    reason: SleepReason::Budget
                }
            ) && !info.suspended
            {
                // KT-21: SC cấp ngân sách có thể là SC riêng **hoặc** SC được donate — cả hai
                // đều phải được đánh thức ở biên refill (DoD 4: server ngủ, refill, Reply).
                if let Some(sc) = self.funding_sc(tcb) {
                    if refilled.contains(&sc) {
                        wake.push(tcb);
                    }
                }
            }
        }
        for tcb in wake {
            self.wake_from(tcb)?;
        }
        Ok(())
    }

    /// Gắn/gỡ SC khiến TCB đang `Running` **không còn budget** (SC đã cạn, SC chưa cấu hình,
    /// hoặc gỡ SC) ⇒ ngủ ngay `Sleeping{Budget}` (giữ bất biến "Running ⇒ còn budget", và
    /// "không SC/SC chưa cấu hình ⇒ không chạy" — reviewer-kt20 mục 1). TCB `Ready` không đổi
    /// trạng thái (bảng ALLOWED cấm `Ready → Sleeping`) — `pick` bỏ qua nó cho tới khi có SC
    /// đã cấu hình còn budget.
    pub(super) fn enforce_budget(&mut self, tcb: ObjId) -> Result<(), KError> {
        let info = self.tcb_info_obj(tcb)?;
        if matches!(info.state, TcbState::Running { .. }) && !self.tcb_has_budget(tcb) {
            self.transition_tcb(
                tcb,
                NonReadyTarget::Sleeping {
                    reason: SleepReason::Budget,
                },
            )?;
        }
        Ok(())
    }
}
