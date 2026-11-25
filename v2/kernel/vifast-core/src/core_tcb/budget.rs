// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `core_tcb/budget.rs` — **ngân sách MCS** (KT-20): `virtual_now`/`tick`/refill/charge,
//! `enforce_budget`, truy vấn `SchedContext`/ưu tiên hiệu dụng; port từ oracle.

use super::*;
use crate::store::KernelStore;

impl<S: KernelStore, O: CoreObserver> Machine<S, O> {
    // ------------------------------------------------------------- ngân sách MCS (KT-20)

    /// Thời gian ảo hiện tại (tick) — **không** đọc wall clock (D2-49(h)).
    pub fn virtual_now(&self) -> u64 {
        self.now
    }

    /// `tick(n)` — chạy đồng hồ ảo `n` đơn vị; tất định. Mỗi đơn vị: charge TCB đang `Running`
    /// cho `[now, now+1)`, `now += 1`, rồi xử lý biên refill (charge trước — refill sau).
    /// **Không** tự `dispatch`.
    pub fn tick(&mut self, n: u64) -> Result<(), KError> {
        for _ in 0..n {
            self.tick_one()?;
        }
        self.sched_after_op(false);
        Ok(())
    }

    fn tick_one(&mut self) -> Result<(), KError> {
        for h in 0..self.n_harts {
            if let Some(cur) = self.harts[h].sched.current() {
                self.charge_tick(cur)?;
            }
        }
        self.now = self.now.saturating_add(1);
        self.process_refills()
    }

    /// Charge **một** tick cho TCB đang chạy; hết budget ⇒ `Running → Sleeping{Budget}`.
    fn charge_tick(&mut self, tcb: ObjId) -> Result<(), KError> {
        let info = self.tcb_data(tcb)?.snapshot();
        if !matches!(info.state, TcbState::Running { .. }) {
            return Ok(());
        }
        let Some(sc) = self.funding_sc(tcb) else {
            self.transition_tcb(
                tcb,
                NonReadyTarget::Sleeping {
                    reason: SleepReason::Budget,
                },
            )?;
            return Ok(());
        };
        if !self.sc_data(sc)?.budget.is_configured() {
            self.transition_tcb(
                tcb,
                NonReadyTarget::Sleeping {
                    reason: SleepReason::Budget,
                },
            )?;
            return Ok(());
        }
        let charged = self.sc_data_mut(sc)?.budget.charge();
        if !charged || !self.tcb_has_budget(tcb) {
            self.transition_tcb(
                tcb,
                NonReadyTarget::Sleeping {
                    reason: SleepReason::Budget,
                },
            )?;
        }
        Ok(())
    }

    /// Xử lý biên nạp lại tại `now` cho mọi `SchedContext` sống; `Sleeping{Budget}` +
    /// `!suspended` được đánh thức `Ready`.
    fn process_refills(&mut self) -> Result<(), KError> {
        let now = self.now;
        let mut refilled = [0u32; MAX_SCHED_CONTEXTS];
        let mut n_refilled = 0usize;
        for i in 0..self.n_objects {
            let obj = (i as u32) + 1;
            if !self.objects[i].alive || self.objects[i].ty != ObjType::SchedContext {
                continue;
            }
            let body = self.objects[i].body;
            if body.kind != BODY_SC || !body.alive {
                continue;
            }
            if self.scs[body.idx as usize].budget.refill_due(now) {
                self.budget_refills += 1;
                if n_refilled < MAX_SCHED_CONTEXTS {
                    refilled[n_refilled] = obj;
                    n_refilled += 1;
                }
            }
        }
        if n_refilled == 0 {
            return Ok(());
        }
        let mut wake = [0u32; MAX_TCBS];
        let mut n_wake = 0usize;
        for i in 0..self.n_objects {
            let obj = (i as u32) + 1;
            if !self.objects[i].alive || self.objects[i].ty != ObjType::Tcb {
                continue;
            }
            let info = self.tcb_data(obj)?.snapshot();
            if matches!(
                info.state,
                TcbState::Sleeping {
                    reason: SleepReason::Budget
                }
            ) && !info.suspended
            {
                if let Some(sc) = self.funding_sc(obj) {
                    if refilled[..n_refilled].contains(&sc) && n_wake < MAX_TCBS {
                        wake[n_wake] = obj;
                        n_wake += 1;
                    }
                }
            }
        }
        for &tcb in &wake[..n_wake] {
            self.wake_from(tcb)?;
        }
        Ok(())
    }

    /// Gắn/gỡ SC khiến TCB đang `Running` **không còn budget** ⇒ ngủ ngay `Sleeping{Budget}`.
    pub(super) fn enforce_budget(&mut self, tcb: ObjId) -> Result<(), KError> {
        let info = self.tcb_data(tcb)?.snapshot();
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

    /// Ảnh chụp ngân sách của `SchedContext`.
    pub fn sched_info(&self, sched: SlotRef) -> Result<ScInfo, KError> {
        let obj = self.require_type(sched, ObjType::SchedContext)?;
        let s = self.sc_data(obj)?;
        let mut info = s.budget.info();
        info.donated_to = s.donated_to;
        info.donor = s.donor;
        Ok(info)
    }

    /// `SchedContext.Consumed` `[TK]`: số tick đã charge trong chu kỳ hiện tại.
    pub fn sched_consumed(&self, sched: SlotRef) -> Result<u64, KError> {
        Ok(self.sched_info(sched)?.consumed)
    }

    /// Budget còn lại của chu kỳ hiện tại; `0` khi SC chưa cấu hình.
    pub fn sched_remaining(&self, sched: SlotRef) -> Result<u64, KError> {
        let info = self.sched_info(sched)?;
        Ok(if info.configured {
            info.budget - info.consumed
        } else {
            0
        })
    }

    /// SC đang **cấp ngân sách** cho `tcb`: SC được donate (KT-21, chưa có ở KT-59) hoặc SC
    /// riêng không bị donate; `None` ⇒ không có budget ⇒ không được `pick` (D2-50).
    fn funding_sc(&self, tcb: ObjId) -> Option<ObjId> {
        let info = self.tcb_data(tcb).ok()?.snapshot();
        if let Some(sc) = info.donated_sc {
            return Some(sc);
        }
        let own = info.sched_context?;
        match self.sc_data(own) {
            Ok(s) if s.donated_to.is_none() => Some(own),
            _ => None,
        }
    }

    /// Model API `[TK]`: SC đang cấp ngân sách cho `tcb`.
    pub fn sc_of(&self, tcb: ObjId) -> Result<Option<ObjId>, KError> {
        Ok(self.funding_sc(tcb))
    }

    /// Ngân sách **còn lại** của SC đang cấp cho `tcb` (0 khi không có SC/chưa cấu hình).
    ///
    /// KT-62: `Yield` (8) trả phần budget còn lại cho user (`spec/abi.toml` `[syscall]`) mà
    /// không cần slot cap của SC; cùng nguồn với [`Machine::sched_remaining`].
    pub fn tcb_remaining(&self, tcb: ObjId) -> u64 {
        self.funding_sc(tcb)
            .and_then(|sc| self.sc_data(sc).ok())
            .map(|s| s.budget.remaining())
            .unwrap_or(0)
    }

    /// TCB có được phép chạy theo ngân sách không (SC đã cấu hình và `consumed < budget`).
    pub fn tcb_has_budget(&self, tcb: ObjId) -> bool {
        let Some(sc) = self.funding_sc(tcb) else {
            return false;
        };
        self.sc_data(sc)
            .map(|s| s.budget.has_budget())
            .unwrap_or(false)
    }

    /// Priority **hiệu lực** của `tcb` (KT-21 DoD 6): khi nhận donation =
    /// [`donated_priority`]; ngược lại priority gốc. KT-59 chưa tạo donation nên luôn là
    /// priority gốc; công thức + trường đã sẵn cho KT-60.
    pub fn effective_priority(&self, tcb: ObjId) -> Result<u8, KError> {
        let info = self.tcb_data(tcb)?.snapshot();
        let Some(sc) = info.donated_sc else {
            return Ok(info.priority);
        };
        let donor = self.sc_data(sc)?.donor;
        let Some(donor) = donor else {
            return Ok(info.priority);
        };
        let oi = self.tcb_data(donor)?.snapshot();
        Ok(donated_priority(
            oi.priority,
            oi.max_controlled_priority,
            info.max_controlled_priority,
        ))
    }
}
