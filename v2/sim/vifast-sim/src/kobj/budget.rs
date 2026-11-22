// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-336b** — miền `budget`: MCS: SchedContext, budget/refill, donation, K1b report.
//!
//! Tách nguyên văn từ `kobj.rs` (D2-232/D2-238): **chỉ đổi vị trí** + `pub(super)` cho hàm
//! nội bộ cần gọi chéo module; thân hàm, thứ tự op và API công khai giữ nguyên. Đây là
//! module con của [`crate::kobj`] nên thấy item private của `kobj` (cùng mẫu `state_write`).

use super::*;

impl Machine {
    pub fn sched_control_configure(
        &mut self,
        sched_control: SlotRef,
        sched: SlotRef,
        budget: u64,
        period: u64,
    ) -> Result<(), KError> {
        self.require_type(sched_control, ObjType::SchedControl)?;
        let sched_obj = self.require_type(sched, ObjType::SchedContext)?;
        if budget == 0 || period == 0 || budget > period {
            return Err(KError::InvalidArgument);
        }
        let now = self.now;
        self.body_mut(sched_obj, |k| match k {
            Kind::SchedContext(s) => {
                s.budget.configure(now, budget, period);
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        // KT-20: `Configure` hợp lệ luôn đặt `consumed = 0` (cấu hình lại giữa chu kỳ = bắt
        // đầu chu kỳ mới theo `period` mới — quy ước model-only `[TK]`, xem `mcs`) nên TCB
        // đang gắn (kể cả `Running`) chắc chắn còn budget; chỉ cần kiểm bất biến sau op.
        self.sched_after_op(false);
        Ok(())
    }

    /// `SchedContext.Bind(sc, reply|notification)` — chỉ **gắn/kiểm** quan hệ cấu trúc
    /// (chữ ký đã duyệt, D2-33). **KT-21 DoD 3**: `Bind` **không** tạo/xoá donation (donation tự
    /// động qua `Call`/`Reply`, D2-49(c)). `Bind` không đổi ngân sách.
    ///
    /// **KT-27** — khi `target` là **Notification**: ghi quan hệ **1:1** vào
    /// [`NotificationState::bound_sc`] (model-only `[TK]`, D2-53 Q6 — không thêm chữ ký TCB):
    /// notification đã bind **SC khác** ⇒ `IllegalOperation`; `SC` đổi target ⇒ binding cũ trên
    /// notification được gỡ (không để `bound_sc` mồ côi).
    pub fn sched_context_bind(&mut self, sched: SlotRef, target: SlotRef) -> Result<(), KError> {
        let sched_obj = self.require_type(sched, ObjType::SchedContext)?;
        let target_cap = self.cap_at(target.cnode, target.slot)?;
        let target_ty = self.cap(target_cap)?.ty;
        if !matches!(target_ty, ObjType::Reply | ObjType::Notification) {
            return Err(KError::WrongObjectType);
        }
        // **KT-27**: kiểm 1:1 phía notification **trước** khi đổi bất cứ trạng thái nào.
        if target_ty == ObjType::Notification {
            let ntfn_obj = self.cap(target_cap)?.obj;
            let bound = self.body(ntfn_obj, |k| match k {
                Kind::Notification(n) => Ok(n.bound_sc),
                _ => Err(KError::WrongObjectType),
            })?;
            if let Some(other) = bound {
                if other != sched_obj {
                    return Err(KError::IllegalOperation);
                }
            }
        }
        // Gỡ binding cũ của SC (nếu đang trỏ một Notification khác) — giữ 1:1 hai chiều.
        let old_bound = self.body(sched_obj, |k| match k {
            Kind::SchedContext(s) => Ok(s.bound),
            _ => Err(KError::WrongObjectType),
        })?;
        if let Some(old_cap) = old_bound {
            if old_cap != target_cap {
                let old = self.cap(old_cap).ok().map(|c| (c.obj, c.ty, c.alive));
                if let Some((old_obj, ObjType::Notification, true)) = old {
                    self.body_mut(old_obj, |k| match k {
                        Kind::Notification(n) => {
                            if n.bound_sc == Some(sched_obj) {
                                n.bound_sc = None;
                            }
                            Ok(())
                        }
                        _ => Err(KError::WrongObjectType),
                    })?;
                }
            }
        }
        self.body_mut(sched_obj, |k| match k {
            Kind::SchedContext(s) => {
                s.bound = Some(target_cap);
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        if target_ty == ObjType::Notification {
            let ntfn_obj = self.cap(target_cap)?.obj;
            self.body_mut(ntfn_obj, |k| match k {
                Kind::Notification(n) => {
                    n.bound_sc = Some(sched_obj);
                    Ok(())
                }
                _ => Err(KError::WrongObjectType),
            })?;
        }
        self.sched_after_op(false);
        Ok(())
    }

    /// **KT-27** — Notification mà `SchedContext` đang bind (nếu `SchedData.bound` là cap
    /// Notification còn sống); `None` khi chưa bind hoặc đang bind Reply. Model API `[TK]`.
    pub fn sched_bound_notification(&self, sched: SlotRef) -> Result<Option<ObjId>, KError> {
        let sched_obj = self.require_type(sched, ObjType::SchedContext)?;
        let bound = self.body(sched_obj, |k| match k {
            Kind::SchedContext(s) => Ok(s.bound),
            _ => Err(KError::WrongObjectType),
        })?;
        let Some(cap) = bound else { return Ok(None) };
        let c = self.cap(cap)?;
        if !c.alive || c.ty != ObjType::Notification {
            return Ok(None);
        }
        Ok(Some(c.obj))
    }

    /// Model API `set_sched_params(tcb, sc)` — **D2-49(b)**: gắn/gỡ `SchedContext` của TCB,
    /// không sửa `spec/`/ABI ở K1b (`Tcb.SetSchedParams` để K1c/K3).
    pub fn set_sched_params(&mut self, tcb: SlotRef, sched: Option<SlotRef>) -> Result<(), KError> {
        let tcb_obj = self.require_type(tcb, ObjType::Tcb)?;
        let sched_obj = match sched {
            Some(s) => Some(self.require_type(s, ObjType::SchedContext)?),
            None => None,
        };
        // KT-21: không đổi ngân sách của TCB đang nhận donation / đang cho mượn SC (phải
        // `Reply` hoặc huỷ donation trước); không lấy SC đang bị donate cho TCB khác.
        let info = self.tcb_info_obj(tcb_obj)?;
        if info.donated_sc.is_some() || self.is_donation_donor(tcb_obj) {
            return Err(KError::IllegalOperation);
        }
        if let Some(sc) = sched_obj {
            if self.donation_of(sc)?.0.is_some() {
                return Err(KError::Busy);
            }
        }
        self.body_mut(tcb_obj, |k| match k {
            Kind::Tcb(t) => {
                t.sched_context = sched_obj;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        // KT-20: gắn SC **đã cạn** cho TCB đang `Running` ⇒ ngủ ngay (`Running → Sleeping`
        // là cặp ALLOWED; không để `Running` mà hết budget). TCB `Ready` giữ nguyên trong
        // hàng — `pick` bỏ qua theo điều kiện budget, tới biên refill lại chọn được.
        self.enforce_budget(tcb_obj)?;
        self.sched_after_op(false);
        Ok(())
    }

    /// **KT-23** — miền bất biến **tham chiếu `SchedContext` + donation** (tách nguyên văn từ
    /// `check_sched` của KT-21, không đổi ngữ nghĩa):
    ///
    /// 1. donation **hai chiều** khớp (`SchedContext.donated_to/donor` ↔ `Tcb.donated_sc`), mỗi
    ///    SC ↔ đúng một donee, donee **không** có SC riêng và **không** phải donor (không chuỗi
    ///    donation nhiều tầng), donor đang `BlockedOnReply`, donor ≠ donee, donee còn sống,
    ///    donor/donee **không** cùng `Running`, priority hiệu lực không vượt MCP của một bên;
    /// 2. mọi `Tcb.sched_context`/`Tcb.donated_sc` trỏ tới **object còn sống đúng kiểu**
    ///    `SchedContext` (bắt SC đã bị Revoke/finalise mà TCB còn giữ tham chiếu);
    /// 3. `SchedContext.bound` (nếu có) trỏ tới **cap còn sống** thuộc Reply/Notification.
    pub fn check_sc_refs(&self) -> Result<(), String> {
        let scs = self.live_sched_contexts();
        let mut donee_cua: Vec<Option<ObjId>> = vec![None; self.objects.len() + 1];
        for sc in &scs {
            let (donated_to, donor) = self
                .donation_of(*sc)
                .map_err(|e| format!("SC {sc}: donation không đọc được: {e:?}"))?;
            match (donated_to, donor) {
                (None, None) => {}
                (Some(donee), Some(donor)) => {
                    if donee == donor {
                        return Err(format!("SC {sc}: donor trùng donee"));
                    }
                    let di = self
                        .tcb_info_obj(donee)
                        .map_err(|e| format!("SC {sc}: donee {donee} không đọc được: {e:?}"))?;
                    let oi = self
                        .tcb_info_obj(donor)
                        .map_err(|e| format!("SC {sc}: donor {donor} không đọc được: {e:?}"))?;
                    if di.donated_sc != Some(*sc) {
                        return Err(format!(
                            "SC {sc} donate cho {donee} nhưng donee không trỏ lại (donated_sc={:?})",
                            di.donated_sc
                        ));
                    }
                    if di.sched_context.is_some() {
                        return Err(format!(
                            "donee {donee} vừa có SC riêng vừa nhận donation (D2-49c)"
                        ));
                    }
                    if oi.sched_context != Some(*sc) {
                        return Err(format!("donor {donor} không còn gắn SC {sc}"));
                    }
                    if oi.donated_sc.is_some() {
                        return Err(format!("TCB {donor} vừa donor vừa donee (chuỗi >1)"));
                    }
                    if !matches!(oi.state, TcbState::BlockedOnReply { .. }) {
                        return Err(format!(
                            "donor {donor} state {} nhưng phải BlockedOnReply",
                            oi.state.name()
                        ));
                    }
                    if matches!(di.state, TcbState::Inactive | TcbState::Dead) {
                        return Err(format!("donee {donee} state {}", di.state.name()));
                    }
                    if matches!(di.state, TcbState::Running { .. })
                        && matches!(oi.state, TcbState::Running { .. })
                    {
                        return Err(format!("donor {donor} và donee {donee} cùng Running"));
                    }
                    let eff = crate::mcs::donated_priority(
                        oi.priority,
                        oi.max_controlled_priority,
                        di.max_controlled_priority,
                    );
                    if eff > oi.max_controlled_priority || eff > di.max_controlled_priority {
                        return Err(format!(
                            "SC {sc}: priority donation {eff} vượt MCP (donor {} / donee {})",
                            oi.max_controlled_priority, di.max_controlled_priority
                        ));
                    }
                    if let Some(slot) = donee_cua.get_mut(donee as usize) {
                        if slot.replace(*sc).is_some() {
                            return Err(format!("donee {donee} nhận donation từ >1 SC"));
                        }
                    }
                }
                other => return Err(format!("SC {sc}: donation nửa vời {other:?}")),
            }
        }
        // Chiều ngược lại: `Tcb.donated_sc` phải khớp `SchedContext.donated_to`, và donee
        // **không** được có SC riêng (một SC một TCB tại một thời điểm).
        for (idx, meta) in self.objects.iter().enumerate() {
            if !meta.alive || meta.ty != ObjType::Tcb {
                continue;
            }
            let obj = (idx as u32) + 1;
            let Some(sc) = self
                .tcb_info_obj(obj)
                .map_err(|e| format!("TCB {obj} không đọc được: {e:?}"))?
                .donated_sc
            else {
                continue;
            };
            let (donated_to, _) = self
                .donation_of(sc)
                .map_err(|e| format!("TCB {obj}: donated_sc {sc} không đọc được: {e:?}"))?;
            if donated_to != Some(obj) {
                return Err(format!(
                    "TCB {obj} giữ donated_sc {sc} nhưng SC không trỏ lại (donated_to={donated_to:?})"
                ));
            }
            if self
                .tcb_info_obj(obj)
                .map_err(|e| format!("TCB {obj} không đọc được: {e:?}"))?
                .sched_context
                .is_some()
            {
                return Err(format!("donee {obj} vừa có SC riêng vừa nhận donation"));
            }
        }
        // KT-20: bất biến ngân sách của **mọi** `SchedContext` sống — `consumed ≤ budget`,
        // `budget`/`period` dương khi đã cấu hình, `refill` ở tương lai; `Sleeping{Budget}`
        // không nằm ready queue đã được nhánh "trạng thái khác" ở trên kiểm.
        for sc in scs {
            let budget = self
                .body(sc, |k| match k {
                    Kind::SchedContext(s) => Ok(s.budget),
                    _ => Err(KError::WrongObjectType),
                })
                .map_err(|e| format!("SchedContext {sc} không đọc được: {e:?}"))?;
            budget
                .check(self.now)
                .map_err(|e| format!("SchedContext {sc}: {e}"))?;
        }

        // KT-23 mục 2: tham chiếu SC của TCB phải trỏ tới object **sống** đúng kiểu
        // `SchedContext` (không chỉ kiểm donation — bắt cả SC bị Revoke mà TCB còn giữ).
        for (idx, meta) in self.objects.iter().enumerate() {
            if !meta.alive || meta.ty != ObjType::Tcb {
                continue;
            }
            let obj = (idx as u32) + 1;
            let info = self
                .tcb_info_obj(obj)
                .map_err(|e| format!("TCB {obj} không đọc được: {e:?}"))?;
            if let Some(sc) = info.sched_context {
                self.check_sc_alive(obj, sc, "sched_context")?;
            }
            if let Some(sc) = info.donated_sc {
                self.check_sc_alive(obj, sc, "donated_sc")?;
            }
        }
        // KT-23 mục 3: `SchedContext.bound` (Reply/Notification) còn sống, đúng kiểu.
        for sc in self.live_sched_contexts() {
            let bound = self
                .body(sc, |k| match k {
                    Kind::SchedContext(s) => Ok(s.bound),
                    _ => Err(KError::WrongObjectType),
                })
                .map_err(|e| format!("SchedContext {sc} không đọc được: {e:?}"))?;
            let Some(cap) = bound else { continue };
            let c = self
                .cap(cap)
                .map_err(|e| format!("SchedContext {sc}: bound cap {cap} không đọc được: {e:?}"))?;
            if !c.alive {
                return Err(format!("SchedContext {sc}: bound cap {cap} đã chết"));
            }
            if !matches!(c.ty, ObjType::Reply | ObjType::Notification) {
                return Err(format!(
                    "SchedContext {sc}: bound cap {cap} kiểu {:?}, phải Reply/Notification",
                    c.ty
                ));
            }
        }
        Ok(())
    }

    /// Kiểm một tham chiếu `SchedContext` của TCB trỏ tới object sống đúng kiểu.
    pub(super) fn check_sc_alive(&self, tcb: ObjId, sc: ObjId, truong: &str) -> Result<(), String> {
        let meta = self
            .objects
            .get(sc as usize - 1)
            .ok_or_else(|| format!("TCB {tcb}: {truong} trỏ tới {sc} ngoài bảng object"))?;
        if !meta.alive {
            return Err(format!("TCB {tcb}: {truong} trỏ tới {sc} đã chết"));
        }
        if meta.ty != ObjType::SchedContext {
            return Err(format!(
                "TCB {tcb}: {truong} trỏ tới {sc} kiểu {:?}, phải SchedContext",
                meta.ty
            ));
        }
        Ok(())
    }

    /// **KT-23** — miền bất biến **ngân sách** (KT-20, giữ nguyên ngữ nghĩa):
    ///
    /// 1. `Running` ⇒ SC cấp ngân sách **đã `Configure` và còn budget** (bất biến "TCB chỉ chạy
    ///    khi SC còn budget" — mọi đường rời `Running` hết budget đi qua `Sleeping{Budget}`);
    /// 2. mọi `SchedContext` sống: `consumed ≤ budget`, `budget`/`period` dương khi đã cấu hình,
    ///    `refill` ở tương lai ([`ScBudget::check`]).
    pub fn check_budget(&self) -> Result<(), String> {
        for (idx, meta) in self.objects.iter().enumerate() {
            if !meta.alive || meta.ty != ObjType::Tcb {
                continue;
            }
            let obj = (idx as u32) + 1;
            let info = self
                .tcb_info_obj(obj)
                .map_err(|e| format!("TCB {obj} không đọc được: {e:?}"))?;
            if matches!(info.state, TcbState::Running { .. }) && !self.tcb_has_budget(obj) {
                return Err(format!(
                    "TCB {obj} Running nhưng SchedContext đã cạn budget (consumed == budget)"
                ));
            }
        }
        // KT-20: bất biến ngân sách của **mọi** `SchedContext` sống — `consumed ≤ budget`,
        // `budget`/`period` dương khi đã cấu hình, `refill` ở tương lai.
        for sc in self.live_sched_contexts() {
            let budget = self
                .body(sc, |k| match k {
                    Kind::SchedContext(s) => Ok(s.budget),
                    _ => Err(KError::WrongObjectType),
                })
                .map_err(|e| format!("SchedContext {sc} không đọc được: {e:?}"))?;
            budget
                .check(self.now)
                .map_err(|e| format!("SchedContext {sc}: {e}"))?;
        }
        Ok(())
    }

    // ------------------------------------------- tổng kết K1b cho test/harness (KT-23) `[TK]`
    //
    // **Model-only, không phải ABI**: `k1b_report` là ảnh chụp bộ đếm của mô hình để KT-23
    // in `transitions hit/allowed`, `forbidden_attempts`, `budget_violations` (K1B-PLAN §5
    // KT-23 DoD 2/3) — **không** thêm method ABI nào, không đổi ngữ nghĩa.

    /// Bất biến K1b tại **một** thời điểm, gộp cả 5 miền — trả lỗi **đầu tiên** kèm tên miền
    /// (test KT-23 gọi sau **mỗi** bước; `sched_after_op` vẫn kiểm sau mỗi public op).
    pub fn k1b_audit(&self) -> Result<(), String> {
        if let Err(e) = self.check_sched() {
            return Err(format!("sched: {e}"));
        }
        if let Err(e) = self.check_sc_refs() {
            return Err(format!("sc_refs: {e}"));
        }
        if let Err(e) = self.check_budget() {
            return Err(format!("budget: {e}"));
        }
        if let Err(e) = self.check_mailboxes() {
            return Err(format!("mailbox: {e}"));
        }
        self.check_all_bytes().map_err(|e| format!("bytes: {e}"))?;
        Ok(())
    }

    /// Dựng [`K1bReport`] từ bộ đếm của mô hình (KT-23 DoD 2/3). Chạy các `check_*` để điền
    /// `*_violations` (0 = sạch tại thời điểm gọi).
    pub fn k1b_report(&self) -> K1bReport {
        let (hit, allowed) = crate::tcb::coverage_design(&self.pair_hits);
        let (hit_table, table_allowed) = crate::tcb::coverage(&self.pair_hits);
        let mut forbidden_attempts = 0u64;
        let mut forbidden_pairs_attempted = 0usize;
        for from in 0..TCB_STATE_COUNT {
            for to in 0..TCB_STATE_COUNT {
                if crate::tcb::ALLOWED[from][to] {
                    continue;
                }
                let n = self.forbidden_hits[from * TCB_STATE_COUNT + to] as u64;
                if n > 0 {
                    forbidden_attempts += n;
                    forbidden_pairs_attempted += 1;
                }
            }
        }
        let mut q = K1bQueueReport {
            live_tcbs: 0,
            ready: 0,
            running: 0,
            blocked: 0,
            sleeping: 0,
            inactive: 0,
            dead: 0,
            queued: 0,
            current: self.sched[0].current(),
            idle: self.sched[0].idle(),
        };
        for (idx, meta) in self.objects.iter().enumerate() {
            if !meta.alive || meta.ty != ObjType::Tcb {
                continue;
            }
            let obj = (idx as u32) + 1;
            let Ok(info) = self.tcb_info_obj(obj) else {
                continue;
            };
            q.live_tcbs += 1;
            match info.state {
                TcbState::Ready => q.ready += 1,
                TcbState::Running { .. } => q.running += 1,
                TcbState::Sleeping { .. } => q.sleeping += 1,
                TcbState::Inactive => q.inactive += 1,
                TcbState::Dead => q.dead += 1,
                other => {
                    if other.is_blocked() {
                        q.blocked += 1;
                    }
                }
            }
        }
        q.queued = self.sched[0].len();
        let mut b = K1bBudgetReport {
            ticks: self.now,
            budget_sleeps: self.budget_sleeps,
            refills: self.budget_refills,
            live_sched_contexts: 0,
            configured: 0,
            charged: 0,
            consumed_now: 0,
        };
        for sc in self.live_sched_contexts() {
            let Ok(budget) = self.body(sc, |k| match k {
                Kind::SchedContext(s) => Ok(s.budget),
                _ => Err(KError::WrongObjectType),
            }) else {
                continue;
            };
            b.live_sched_contexts += 1;
            if budget.is_configured() {
                b.configured += 1;
            }
            b.charged = b.charged.saturating_add(budget.charged());
            b.consumed_now = b.consumed_now.saturating_add(budget.consumed());
        }
        K1bReport {
            transition: K1bTransitionReport {
                hit,
                allowed,
                hit_table,
                table_allowed,
                reserved: crate::tcb::reserved_count(),
                forbidden_attempts,
                forbidden_pairs_attempted,
                forbidden_pairs_total: crate::tcb::forbidden_count(),
            },
            queue: q,
            budget: b,
            sched_violations: u64::from(self.check_sched().is_err()),
            sc_ref_violations: u64::from(self.check_sc_refs().is_err()),
            budget_violations: u64::from(self.check_budget().is_err()),
            mailbox_violations: u64::from(self.check_mailboxes().is_err()),
            byte_violations: u64::from(self.check_all_bytes().is_err()),
        }
    }

    /// Ảnh chụp ngân sách của `SchedContext` — model API `[TK]` (D2-49(b)).
    pub fn sched_info(&self, sched: SlotRef) -> Result<ScInfo, KError> {
        let obj = self.require_type(sched, ObjType::SchedContext)?;
        self.body(obj, |k| match k {
            Kind::SchedContext(s) => {
                let mut info = s.budget.info();
                info.donated_to = s.donated_to;
                info.donor = s.donor;
                Ok(info)
            }
            _ => Err(KError::WrongObjectType),
        })
    }

    /// `SchedContext.Consumed` `[TK]`: số tick đã charge trong chu kỳ hiện tại.
    pub fn sched_consumed(&self, sched: SlotRef) -> Result<u64, KError> {
        Ok(self.sched_info(sched)?.consumed)
    }

    /// Budget còn lại của chu kỳ hiện tại; `0` khi SC chưa cấu hình (không có budget).
    pub fn sched_remaining(&self, sched: SlotRef) -> Result<u64, KError> {
        let info = self.sched_info(sched)?;
        Ok(if info.configured {
            info.budget - info.consumed
        } else {
            0
        })
    }

    /// TCB có được phép chạy theo ngân sách không: **phải có** SC cấp ngân sách **đã**
    /// `SchedControl.Configure` và `consumed < budget`. SC cấp ngân sách là SC **riêng**
    /// (không bị donate) hoặc SC **được donate** cho TCB (KT-21, D2-49(c)); không SC / SC chưa
    /// cấu hình ⇒ `false` — TCB không được `pick` (ngữ nghĩa §4.5, K1B-PLAN §5 KT-20 DoD 4,
    /// D2-50: không SC **không phải** "không giới hạn"). Dùng bởi `pick`/`dispatch`,
    /// `enforce_budget`, `charge_tick` và test bất biến.
    pub fn tcb_has_budget(&self, tcb: ObjId) -> bool {
        let Some(sc) = self.funding_sc(tcb) else {
            return false;
        };
        self.body(sc, |k| match k {
            Kind::SchedContext(s) => Ok(s.budget.has_budget()),
            _ => Err(KError::WrongObjectType),
        })
        .unwrap_or(false)
    }

    // ------------------------------------------- MCS donation (KT-21) `[TK]`
    //
    // Nguồn: **D2-49(c)** ("donation **tự động** qua Call/Reply; server có SC riêng giữ SC
    // riêng; các method chưa có chữ ký chỉ cài model API"), **D2-50** ("TCB không SC **không
    // chạy**, trừ khi được donation"), thiết kế §4.5 (`:288-311`), K1B-PLAN §5 KT-21.
    // Model-only `[TK]` — **không** mở chữ ký ABI nào.
    //
    // Liên kết **hai chiều** để tra O(1) và kiểm bất biến:
    //   `SchedData { donated_to: Option<donee>, donor: Option<donor> }`
    //   `TcbData  { donated_sc: Option<sc> }`
    // Bất biến (kiểm trong [`Machine::check_sched`] sau **mỗi** public op): hai chiều khớp;
    // donee **không** có SC riêng; donor đang `BlockedOnReply`; donor ≠ donee; mỗi SC một donee
    // và mỗi donee một SC; donor/donee **không** cùng `Running`; priority hiệu lực không vượt
    // MCP của một trong hai bên.
    //
    // **Không chuỗi >1** (K1B-PLAN §5 KT-21 rủi ro (a)): chỉ TCB có `sched_context` **và**
    // không phải donee mới donate được. K1b cũng **không** mô hình hoá một server phục vụ nhiều
    // client cùng lúc (server chỉ nhận donation khi đang không có donation khác).

    /// `(donated_to, donor)` của một `SchedContext` — `(None, None)` khi SC không bị donate.
    pub(super) fn donation_of(&self, sc: ObjId) -> Result<(Option<ObjId>, Option<ObjId>), KError> {
        self.body(sc, |k| match k {
            Kind::SchedContext(s) => Ok((s.donated_to, s.donor)),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// Danh sách `SchedContext` còn sống (mô hình nhỏ; dùng cho donation/bất biến).
    pub(super) fn live_sched_contexts(&self) -> Vec<ObjId> {
        self.objects
            .iter()
            .enumerate()
            .filter(|(_, m)| m.alive && m.ty == ObjType::SchedContext)
            .map(|(i, _)| (i as u32) + 1)
            .collect()
    }

    /// SC đang **cấp ngân sách** cho `tcb`: SC riêng (khi **không** bị donate) hoặc SC được
    /// donate cho TCB. `None` ⇔ không có ngân sách ⇒ không được `pick` (D2-50).
    pub(super) fn funding_sc(&self, tcb: ObjId) -> Option<ObjId> {
        let info = self.tcb_info_obj(tcb).ok()?;
        if let Some(sc) = info.donated_sc {
            return Some(sc);
        }
        let own = info.sched_context?;
        match self.donation_of(own) {
            Ok((None, _)) => Some(own),
            // SC riêng đang cho người khác mượn (hoặc SC chết) ⇒ chủ sở hữu **không** có budget.
            _ => None,
        }
    }

    /// `tcb` có phải **donor** của một donation đang hiệu lực không.
    pub(super) fn is_donation_donor(&self, tcb: ObjId) -> bool {
        self.live_sched_contexts()
            .into_iter()
            .any(|sc| matches!(self.donation_of(sc), Ok((_, Some(o))) if o == tcb))
    }

    /// Model API `[TK]` (K1B-PLAN §5 KT-21 #1): SC đang cấp ngân sách cho `tcb` — SC được
    /// donate nếu `tcb` là donee, SC riêng nếu không bị donate, `None` nếu không có.
    pub fn sc_of(&self, tcb: ObjId) -> Result<Option<ObjId>, KError> {
        let info = self.tcb_info_obj(tcb)?;
        if let Some(sc) = info.donated_sc {
            return Ok(Some(sc));
        }
        let Some(own) = info.sched_context else {
            return Ok(None);
        };
        Ok(match self.donation_of(own)? {
            (None, _) => Some(own),
            _ => None,
        })
    }

    /// Model API `[TK]`: SC **được donate cho** `tcb` (`None` nếu không phải donee).
    pub fn donated_sc(&self, tcb: ObjId) -> Result<Option<ObjId>, KError> {
        Ok(self.tcb_info_obj(tcb)?.donated_sc)
    }

    /// `SchedContext.DonatedTo` `[TK]`: TCB đang nhận donation của `sched` (`None` nếu không).
    pub fn sched_donated_to(&self, sched: SlotRef) -> Result<Option<ObjId>, KError> {
        let obj = self.require_type(sched, ObjType::SchedContext)?;
        Ok(self.donation_of(obj)?.0)
    }

    /// `SchedContext.Donor` `[TK]`: TCB đang cho mượn `sched` (`None` nếu không).
    pub fn sched_donor_of(&self, sched: SlotRef) -> Result<Option<ObjId>, KError> {
        let obj = self.require_type(sched, ObjType::SchedContext)?;
        Ok(self.donation_of(obj)?.1)
    }

    /// Priority **hiệu lực** của `tcb` (KT-21 DoD 6): khi nhận donation =
    /// [`crate::mcs::donated_priority`] (kẹp theo MCP của donor và donee); ngược lại priority
    /// gốc. Priority gốc **không** bị đổi — chỉ bucket ready queue/`cur_prio` dùng giá trị này.
    pub fn effective_priority(&self, tcb: ObjId) -> Result<u8, KError> {
        let info = self.tcb_info_obj(tcb)?;
        let Some(sc) = info.donated_sc else {
            return Ok(info.priority);
        };
        let (_, donor) = self.donation_of(sc)?;
        let Some(donor) = donor else {
            return Ok(info.priority);
        };
        let oi = self.tcb_info_obj(donor)?;
        Ok(crate::mcs::donated_priority(
            oi.priority,
            oi.max_controlled_priority,
            info.max_controlled_priority,
        ))
    }

    /// Đặt donation tự động `donor → donee` (KT-21 DoD 1). Trả `false` (**không đổi gì**) khi
    /// không đủ điều kiện — quan trọng: `Call` vẫn thành công, chỉ là server không có ngân sách
    /// nên không chạy (D2-50); **không** từ chối IPC vì thiếu SC (giữ KT-20 DoD 4).
    ///
    /// Điều kiện: donor có SC riêng **đã** `SchedControl.Configure`, donor **không** đang là
    /// donee (không chuỗi >1), donee **không** có SC riêng và **không** đang nhận donation khác
    /// (D2-49(c): "server có SC riêng giữ SC riêng"), SC chưa bị donate.
    pub(super) fn donate_sc(&mut self, donor: ObjId, donee: ObjId) -> Result<bool, KError> {
        if donor == donee {
            return Ok(false);
        }
        let dinfo = self.tcb_info_obj(donor)?;
        if dinfo.donated_sc.is_some() {
            return Ok(false);
        }
        let Some(sc) = dinfo.sched_context else {
            return Ok(false);
        };
        let configured = self.body(sc, |k| match k {
            Kind::SchedContext(s) => Ok(s.budget.is_configured()),
            _ => Err(KError::WrongObjectType),
        })?;
        if !configured {
            return Ok(false);
        }
        // KT-21 **siết** "một SC một TCB tại một thời điểm" cho donation: mô hình KT-20 chưa
        // cấm hai TCB chia một SC; nếu SC đang gắn cho TCB khác thì **không donate** (nếu vẫn
        // donate, TCB kia đang `Running` sẽ mất ngân sách giữa chừng — vỡ bất biến "Running ⇒
        // còn budget"). SC dùng chung vẫn chạy bình thường theo KT-20, chỉ không donate.
        let mut chu_so_huu = 0usize;
        for (idx, meta) in self.objects.iter().enumerate() {
            if !meta.alive || meta.ty != ObjType::Tcb {
                continue;
            }
            if let Ok(info) = self.tcb_info_obj((idx as u32) + 1) {
                if info.sched_context == Some(sc) {
                    chu_so_huu += 1;
                }
            }
        }
        if chu_so_huu != 1 {
            return Ok(false);
        }
        let donee_info = self.tcb_info_obj(donee)?;
        if donee_info.sched_context.is_some() || donee_info.donated_sc.is_some() {
            return Ok(false);
        }
        if self.donation_of(sc)?.0.is_some() {
            return Ok(false);
        }
        // Liên kết hai chiều **trước** khi sync bucket (priority hiệu lực đọc từ đây).
        self.body_mut(sc, |k| match k {
            Kind::SchedContext(s) => {
                s.donated_to = Some(donee);
                s.donor = Some(donor);
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        self.body_mut(donee, |k| match k {
            Kind::Tcb(t) => {
                t.donated_sc = Some(sc);
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        // Donee đang `Ready` ⇒ chuyển sang bucket priority hiệu lực (một SC một TCB một lúc).
        self.sched_remove_all(donee);
        self.sched_sync(donee)?;
        Ok(true)
    }

    /// Trả SC donation của `donee` về donor (KT-21 DoD 2). Gỡ bucket **cũ** (priority donate)
    /// trước khi đổi priority hiệu lực, rồi sync lại theo priority gốc. Trả SC đã trả (nếu có).
    pub(super) fn return_donation(&mut self, donee: ObjId) -> Result<Option<ObjId>, KError> {
        let Some(sc) = self.tcb_info_obj(donee)?.donated_sc else {
            return Ok(None);
        };
        self.sched_remove_all(donee);
        self.body_mut(donee, |k| match k {
            Kind::Tcb(t) => {
                t.donated_sc = None;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        self.body_mut(sc, |k| match k {
            Kind::SchedContext(s) => {
                s.donated_to = None;
                s.donor = None;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        self.sched_sync(donee)?;
        Ok(Some(sc))
    }

    /// Donor của SC đang donate cho `donee` (`None` nếu `donee` không nhận donation).
    pub(super) fn donation_donor_of(&self, donee: ObjId) -> Option<ObjId> {
        let sc = self.tcb_info_obj(donee).ok()?.donated_sc?;
        self.donation_of(sc).ok()?.1
    }

    /// Huỷ reply đang chờ của `donor`: gỡ `Reply.caller` rồi đánh thức về `Ready`.
    ///
    /// Model-only `[TK]` (KT-21 DoD 5): server chết/treo khi đang được donate thì reply **không
    /// bao giờ** tới, nên client không được treo vĩnh viễn. Mã kết quả `Cancelled` đầy đủ (mang
    /// giá trị trả về của `Call`) thuộc K1c — K1b chỉ mô hình hoá việc đánh thức.
    pub(super) fn cancel_reply_of(&mut self, donor: ObjId) -> Result<(), KError> {
        if let TcbState::BlockedOnReply { reply } = self.tcb_state(donor)? {
            self.reply_set_caller(reply, None)?;
            self.wake_from(donor)?;
        }
        Ok(())
    }

    /// **KT-25** — `Reply` **bị xoá/revoke** khi caller đang `BlockedOnReply` (D2-53 **Q8**):
    /// lấy `Reply.caller`, đặt lỗi IPC `Cancelled` cho caller và đánh thức caller `Ready`.
    /// Không deadlock, `Reply.caller == None` sau khi huỷ. Đi qua **một** cửa
    /// [`Machine::transition_tcb`] ⇒ hook KT-21 tự trả `SchedContext` donation về donor (không
    /// rò SC). Trả `Some(caller)` nếu đã đánh thức ai.
    pub(super) fn cancel_reply(&mut self, reply_obj: ObjId) -> Result<Option<ObjId>, KError> {
        let caller = self.body_mut(reply_obj, |k| match k {
            Kind::Reply(r) => Ok(r.caller.take()),
            _ => Err(KError::WrongObjectType),
        })?;
        if let Some(c) = caller {
            self.set_last_ipc_error(c, KError::Cancelled)?;
            if matches!(
                self.tcb_state(c)?,
                TcbState::BlockedOnReply { reply } if reply == reply_obj
            ) {
                self.wake_from(c)?;
            }
        }
        Ok(caller)
    }

    /// **KT-25** — một **cap Reply** vừa bị `Delete`/`Revoke` ⇒ coi như đường reply bị xoá:
    /// caller đang chờ được đánh thức `Cancelled` (D2-53 Q8, `.scn` 07/08).
    ///
    /// Model-only `[TK]`: seL4 chỉ mất object khi Untyped bị revoke, nhưng hai kịch bản `.scn`
    /// của K1c xoá **cap** Reply (`CNode.Delete`/`CNode.Revoke`) rồi kỳ vọng caller không treo;
    /// Q8 chốt caller `Cancelled`. Object **không** bị finalise (chỉ huỷ **đường reply**), nên
    /// gọi nhiều lần là idempotent (`caller = None` sau lần đầu).
    pub(super) fn cancel_reply_of_cap(&mut self, obj: ObjId) -> Result<(), KError> {
        if self.object_alive(obj) && self.object_type(obj) == Some(ObjType::Reply) {
            self.cancel_reply(obj)?;
        }
        Ok(())
    }

    /// **KT-25** — gỡ **Reply cap đã chết** khỏi Recv waiter của mọi Endpoint: waiter vẫn chờ
    /// nhưng coi như **không cấp Reply** ⇒ `Call` tới đi nhánh Q7(b) (message vẫn giao + caller
    /// `Cancelled`) thay vì lỗi cứng `InvalidCapability` (`.scn` 07: `Delete(rep0)` sau `Recv`).
    pub(super) fn clear_dead_reply_in_waiters(&mut self) {
        let alive: Vec<bool> = self.caps.iter().map(|c| c.alive).collect();
        let cap_alive = |cap: CapId| -> bool {
            cap > 0 && (cap as usize) <= alive.len() && alive[cap as usize - 1]
        };
        let eps: Vec<ObjId> = self
            .objects
            .iter()
            .enumerate()
            .filter(|(_, m)| m.alive && m.ty == ObjType::Endpoint)
            .map(|(i, _)| (i as u32) + 1)
            .collect();
        for ep in eps {
            let _ = self.body_mut(ep, |k| {
                if let Kind::Endpoint(e) = k {
                    if let Some((tcb, Some(cap))) = e.recv {
                        if !cap_alive(cap) {
                            e.recv = Some((tcb, None));
                        }
                    }
                }
                Ok::<(), KError>(())
            });
        }
    }

    /// Huỷ donation khi `tcb` là **donee** hoặc **donor** (suspend/exit/finalise — DoD 5).
    ///
    /// - `tcb` là donee ⇒ trả SC về donor + đánh thức donor (reply sẽ không tới).
    /// - `tcb` là donor ⇒ thu hồi SC khỏi donee; donee đang `Running` mất ngân sách ⇒
    ///   `Sleeping{Budget}` ngay (D2-50), `Ready` thì ở lại hàng và bị `pick` bỏ qua.
    ///
    /// Không rò SC: sau hàm này không còn `donated_to`/`donor`/`donated_sc` nào trỏ tới `tcb`.
    pub(super) fn release_donation_of(&mut self, tcb: ObjId) -> Result<(), KError> {
        // (1) `tcb` là donee.
        if let Some(sc) = self.tcb_info_obj(tcb)?.donated_sc {
            let (_, donor) = self.donation_of(sc)?;
            self.return_donation(tcb)?;
            if let Some(donor) = donor {
                self.cancel_reply_of(donor)?;
            }
        }
        // (2) `tcb` là donor (SC của nó đang cho mượn).
        for sc in self.live_sched_contexts() {
            let (donated_to, donor) = self.donation_of(sc)?;
            if donor != Some(tcb) {
                continue;
            }
            let Some(donee) = donated_to else {
                continue;
            };
            self.sched_remove_all(donee);
            self.body_mut(donee, |k| match k {
                Kind::Tcb(t) => {
                    t.donated_sc = None;
                    Ok(())
                }
                _ => Err(KError::WrongObjectType),
            })?;
            self.body_mut(sc, |k| match k {
                Kind::SchedContext(s) => {
                    s.donated_to = None;
                    s.donor = None;
                    Ok(())
                }
                _ => Err(KError::WrongObjectType),
            })?;
            if matches!(self.tcb_state(donee)?, TcbState::Running { .. }) {
                self.transition_tcb(
                    donee,
                    NonReadyTarget::Sleeping {
                        reason: SleepReason::Budget,
                    },
                )?;
            } else {
                self.sched_sync(donee)?;
            }
        }
        Ok(())
    }
}
