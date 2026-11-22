// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-336b** — miền `tcb`: TCB: configure, priority/affinity/home, suspend/resume, hàng chờ.
//!
//! Tách nguyên văn từ `kobj.rs` (D2-232/D2-238): **chỉ đổi vị trí** + `pub(super)` cho hàm
//! nội bộ cần gọi chéo module; thân hàm, thứ tự op và API công khai giữ nguyên. Đây là
//! module con của [`crate::kobj`] nên thấy item private của `kobj` (cùng mẫu `state_write`).

use super::*;

impl Machine {
    // ---------------------------------------------------------------- cấu hình

    /// `Tcb.Configure`: gắn CSpace; **xoá `exit_code`** (D2-25: `Configure`/`Resume` xoá).
    pub fn tcb_configure(&mut self, tcb: SlotRef, cspace: Option<SlotRef>) -> Result<(), KError> {
        let tcb_obj = self.require_type(tcb, ObjType::Tcb)?;
        let cspace_obj = match cspace {
            Some(s) => Some(self.require_type(s, ObjType::CNode)?),
            None => None,
        };
        self.body_mut(tcb_obj, |k| match k {
            Kind::Tcb(t) => {
                t.cspace = cspace_obj;
                t.exit_code = None;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        self.sched_after_op(false);
        Ok(())
    }

    pub fn tcb_set_fault_ep(&mut self, tcb: SlotRef, ep: SlotRef) -> Result<(), KError> {
        let tcb_obj = self.require_type(tcb, ObjType::Tcb)?;
        let ep_cap = self.cap_at(ep.cnode, ep.slot)?;
        if self.cap(ep_cap)?.ty != ObjType::Endpoint {
            return Err(KError::WrongObjectType);
        }
        self.body_mut(tcb_obj, |k| match k {
            Kind::Tcb(t) => {
                t.fault_ep = Some(ep_cap);
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        self.sched_after_op(false);
        Ok(())
    }

    // ------------------------------------------- trường TCB qua model API `[TK]` (KT-18 DoD 1)
    //
    // Các method dưới đây **không** có chữ ký trong draft D2-33 (trừ `Tcb.ReadState`), nên
    // K1b chỉ cài **model API**, không tuyên bố ABI (D2-49(b,c)). KT-21 sẽ dùng chúng cho
    // donation/MCP.

    /// `Tcb.SetPriority`/`Tcb.SetMcp` `[TK]`: `max_controlled_priority ≥ priority`
    /// (thiết kế §4.5 `:311`: MCP ngăn tự nâng ưu tiên).
    pub fn tcb_set_priority(
        &mut self,
        tcb: SlotRef,
        priority: u8,
        max_controlled_priority: u8,
    ) -> Result<(), KError> {
        let tcb_obj = self.require_type(tcb, ObjType::Tcb)?;
        if max_controlled_priority < priority {
            return Err(KError::InvalidArgument);
        }
        // KT-21: priority của **donor** đang được donate không đổi được (sẽ đổi priority hiệu
        // lực của donee giữa chừng) — phải `Reply`/huỷ donation trước. Donee thì đổi được
        // `priority` gốc (không ảnh hưởng khi đang donate) và **MCP** (đổi priority hiệu lực).
        let info = self.tcb_info_obj(tcb_obj)?;
        if self.is_donation_donor(tcb_obj) {
            return Err(KError::IllegalOperation);
        }
        // KT-19: priority đổi ⇒ gỡ khỏi bucket cũ **trước** khi sửa (bucket == priority hiệu
        // lực), rồi `sched_sync` đưa vào bucket mới nếu đang `Ready`. Donee phải gỡ cả khi
        // không `Ready` (MCP đổi ⇒ priority hiệu lực đổi).
        if info.state == TcbState::Ready || info.donated_sc.is_some() {
            self.sched_remove_all(tcb_obj);
        }
        self.body_mut(tcb_obj, |k| match k {
            Kind::Tcb(t) => {
                t.priority = priority;
                t.max_controlled_priority = max_controlled_priority;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        self.sched_sync(tcb_obj)?;
        self.sched_after_op(true);
        Ok(())
    }

    /// `Tcb.SetAffinity` `[TK]`: `HartMask` ≤ 64 bit, khác 0, chỉ bit của hart có thật, và phải
    /// chứa hart `home` hiện tại (KT-19: `home` của TCB phải nằm trong affinity — nếu không
    /// `Ready` không thể vào hàng nào; KT-29 giữ bất biến này khi có nhiều hart).
    pub fn tcb_set_affinity(&mut self, tcb: SlotRef, affinity: u64) -> Result<(), KError> {
        let tcb_obj = self.require_type(tcb, ObjType::Tcb)?;
        let hart_mask = (1u64 << self.sched.len()) - 1;
        if affinity == 0 || affinity & !hart_mask != 0 {
            return Err(KError::InvalidArgument);
        }
        let home = self.tcb_info_obj(tcb_obj)?.home;
        if affinity & (1u64 << home) == 0 {
            return Err(KError::InvalidArgument);
        }
        if self.tcb_state(tcb_obj)? == TcbState::Ready {
            self.sched_remove_all(tcb_obj);
        }
        self.body_mut(tcb_obj, |k| match k {
            Kind::Tcb(t) => {
                t.affinity = affinity;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        self.sched_sync(tcb_obj)?;
        self.sched_after_op(true);
        Ok(())
    }

    /// `Tcb.SetHome` `[TK]`: home phải là hart có thật và **nằm trong affinity** của TCB.
    /// **KT-29**: không đổi `home` của TCB đang `Running` (`Running{hart}` ⇒ `home == hart` là
    /// bất biến `check_smp`; muốn đổi phải preempt/`Ready` trước).
    pub fn tcb_set_home(&mut self, tcb: SlotRef, home: u16) -> Result<(), KError> {
        let tcb_obj = self.require_type(tcb, ObjType::Tcb)?;
        if home as usize >= self.sched.len() {
            return Err(KError::InvalidArgument);
        }
        let info = self.tcb_info_obj(tcb_obj)?;
        // **KT-29**: với `n > 1` bất biến `Running{hart} ⇒ home == hart`; với `n = 1` giữ
        // nguyên hành vi cũ (chỉ hart 0 nên đổi home vô hại) — reviewer-kt29 mục 5.
        if self.sched.len() > 1 && matches!(info.state, TcbState::Running { .. }) {
            return Err(KError::IllegalOperation);
        }
        if info.affinity & (1u64 << home) == 0 {
            return Err(KError::InvalidArgument);
        }
        if info.state == TcbState::Ready {
            self.sched_remove_all(tcb_obj);
        }
        self.body_mut(tcb_obj, |k| match k {
            Kind::Tcb(t) => {
                t.home = home;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        self.sched_sync(tcb_obj)?;
        self.sched_after_op(true);
        Ok(())
    }

    /// Bind notification vào TCB (thiết kế §4.7): model API `[TK]`, **chưa** có chữ ký ABI
    /// (D2-53 Q6: **không** thêm `Tcb.BindNotification` vào `spec/`).
    ///
    /// **KT-27 — 1:1**: một TCB chỉ có **một** bound notification (bind notification **khác** ⇒
    /// `IllegalOperation`) và một notification chỉ bind **một** TCB (TCB khác đã bind ⇒ `Busy`).
    /// Bind lại **cùng** notification là no-op `Ok`.
    pub fn tcb_bind_notification(&mut self, tcb: SlotRef, ntfn: SlotRef) -> Result<(), KError> {
        let tcb_obj = self.require_type(tcb, ObjType::Tcb)?;
        let ntfn_obj = self.require_type(ntfn, ObjType::Notification)?;
        let current = self.body(tcb_obj, |k| match k {
            Kind::Tcb(t) => Ok(t.bound_notification),
            _ => Err(KError::WrongObjectType),
        })?;
        if let Some(cur) = current {
            if cur == ntfn_obj {
                return Ok(());
            }
            return Err(KError::IllegalOperation);
        }
        // 1:1 phía notification: không cho hai TCB cùng bind một notification.
        for (i, meta) in self.objects.iter().enumerate() {
            if !meta.alive || meta.ty != ObjType::Tcb {
                continue;
            }
            let other = (i as u32) + 1;
            if other == tcb_obj {
                continue;
            }
            let bound = self.body(other, |k| match k {
                Kind::Tcb(t) => Ok(t.bound_notification),
                _ => Err(KError::WrongObjectType),
            })?;
            if bound == Some(ntfn_obj) {
                return Err(KError::Busy);
            }
        }
        self.body_mut(tcb_obj, |k| match k {
            Kind::Tcb(t) => {
                t.bound_notification = Some(ntfn_obj);
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        self.sched_after_op(false);
        Ok(())
    }

    /// **KT-27** — `Tcb.UnbindNotification` (method 14 trong danh sách `spec/abi.toml:143`,
    /// **chưa** có chữ ký nên đây là **model API** `[TK]`, D2-53 Q6). Idempotent: chưa bind ⇒
    /// `Ok`. TCB chết cũng unbind qua [`Machine::finalise_tcb`].
    pub fn tcb_unbind_notification(&mut self, tcb: SlotRef) -> Result<(), KError> {
        let tcb_obj = self.require_type(tcb, ObjType::Tcb)?;
        self.body_mut(tcb_obj, |k| match k {
            Kind::Tcb(t) => {
                t.bound_notification = None;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        self.sched_after_op(false);
        Ok(())
    }

    /// Ảnh chụp toàn bộ trường TCB của mô hình (đọc/ghi qua model API `[TK]`).
    pub fn tcb_info(&self, tcb: SlotRef) -> Result<TcbInfo, KError> {
        let obj = self.require_type(tcb, ObjType::Tcb)?;
        self.tcb_info_obj(obj)
    }

    /// **KT-25** — bind **receive window** (CPTR 4 ô CSpace) cho TCB — model API `[TK]`,
    /// **không** thêm chữ ký ABI (`Tcb.Configure` của `spec/abi.toml` chỉ có CSpace; D2-33).
    ///
    /// Mỗi ô phải là slot **có thật** của một CNode còn sống; các ô phải **khác nhau** (hai cap
    /// cài vào cùng một slot sẽ đè nhau ⇒ `InvalidArgument`). Bind `[s0..s3]` với các ô **rỗng**
    /// là hợp lệ — khác "chưa bind" (`None`: message có cap ⇒ `InvalidArgument` vì không có chỗ
    /// cài, KT-25 rủi ro (b) của K1C-PLAN).
    pub fn tcb_set_recv_window(
        &mut self,
        tcb: SlotRef,
        window: [SlotRef; ipc::MAX_CAPS],
    ) -> Result<(), KError> {
        let tcb_obj = self.require_type(tcb, ObjType::Tcb)?;
        for (i, w) in window.iter().enumerate() {
            self.check_cnode_slot(w.cnode, w.slot)?;
            if window[..i].contains(w) {
                return Err(KError::InvalidArgument);
            }
        }
        self.body_mut(tcb_obj, |k| match k {
            Kind::Tcb(t) => {
                t.recv_window = Some(window);
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        self.sched_after_op(false);
        Ok(())
    }

    /// **KT-25** — receive window hiện tại của TCB (`None` = chưa bind).
    pub fn tcb_recv_window(&self, tcb: ObjId) -> Result<Option<[SlotRef; ipc::MAX_CAPS]>, KError> {
        self.body(tcb, |k| match k {
            Kind::Tcb(t) => Ok(t.recv_window),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// Như [`Machine::tcb_info`] nhưng nhận thẳng `ObjId` — cho scheduler (KT-19) đọc
    /// `state`/`suspended`/`priority`/`affinity`/`home` không cần cap.
    pub(super) fn tcb_info_obj(&self, tcb: ObjId) -> Result<TcbInfo, KError> {
        self.body(tcb, |k| match k {
            Kind::Tcb(t) => Ok(t.snapshot()),
            _ => Err(KError::WrongObjectType),
        })
    }

    // ------------------------------------------- một cửa chuyển trạng thái (KT-18 DoD 3+4)

    /// `Tcb.ReadState` — trả **4 giá trị** `{state, suspended, blocked_on_kind, exit_code}`
    /// (thiết kế §4.4 `:282`, D2-33). Trước KT-18 hàm này trả `TcbState` (drift đã sửa).
    pub fn tcb_read_state(&self, tcb: SlotRef) -> Result<TcbSnapshot, KError> {
        let obj = self.require_type(tcb, ObjType::Tcb)?;
        self.body(obj, |k| match k {
            Kind::Tcb(t) => Ok(TcbSnapshot {
                state: t.state,
                suspended: t.suspended,
                blocked_on_kind: t.state.blocked_on_kind(),
                exit_code: t.exit_code,
            }),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// Số lần cặp `(from, to)` (chỉ số `TcbState::index`) đã đi qua cổng.
    pub fn pair_hit_count(&self, from: usize, to: usize) -> u32 {
        self.pair_hits[from * TCB_STATE_COUNT + to]
    }

    /// `(số cặp ALLOWED đã có ít nhất một hit, tổng số cặp ALLOWED)` — KT-18 DoD 4.
    pub fn transition_coverage(&self) -> (usize, usize) {
        crate::tcb::coverage(&self.pair_hits)
    }

    /// **KT-23** — số lần cặp **cấm** `(from, to)` bị thử (mọi lần trả `IllegalOperation`, state
    /// không đổi). Dùng cho DoD 3: **mọi** cặp cấm phải được thử ít nhất một lần.
    pub fn forbidden_attempt_count(&self, from: usize, to: usize) -> u32 {
        self.forbidden_hits[from * TCB_STATE_COUNT + to]
    }

    /// **KT-23** — `(số cặp thiết kế đã có hit, 46)`; bỏ 2 cặp `Stopping` reserved (K1c).
    pub fn design_coverage(&self) -> (usize, usize) {
        crate::tcb::coverage_design(&self.pair_hits)
    }

    /// Log finalise TCB (theo thứ tự finalise) — KT-18 DoD 9.
    pub fn finalise_log(&self) -> &[FinaliseEntry] {
        &self.finalise_log
    }

    pub fn tcb_state(&self, tcb: ObjId) -> Result<TcbState, KError> {
        self.body(tcb, |k| match k {
            Kind::Tcb(t) => Ok(t.state),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// `Tcb.Suspend(mode)` (thiết kế §4.4 `:278-283`).
    ///
    /// - `Abort`: gỡ TCB khỏi **mọi** hàng chờ **trước** khi bật cờ `suspended`; `BlockedOn*`
    ///   và `Running` → `Ready`; `Ready` giữ `Ready`; `Sleeping` **giữ** `Sleeping` (bảng §4.5
    ///   `:302-311`). `exit_code` **giữ nguyên** (D2-25, DoD 8).
    /// - `Defer`: K1b chưa có thao tác dài có `Restart` (§4.4 `:282`) ⇒ **luôn**
    ///   `InvalidArgument`, trạng thái không đổi (DoD 6).
    /// - `Inactive`/`Stopping`/`Dead`: không có gì để dừng ⇒ `IllegalOperation`.
    pub fn tcb_suspend(&mut self, tcb: SlotRef, mode: SuspendMode) -> Result<(), KError> {
        let obj = self.require_type(tcb, ObjType::Tcb)?;
        if mode == SuspendMode::Defer {
            return Err(KError::InvalidArgument);
        }
        let state = self.tcb_state(obj)?;
        if !state.is_live() || state == TcbState::Inactive {
            return Err(KError::IllegalOperation);
        }
        // KT-21 DoD 5: huỷ donation **trước** khi gỡ hàng/đổi trạng thái — TCB là donor thì thu
        // hồi SC (donee đang `Running` mất ngân sách ⇒ `Sleeping{Budget}` ngay, D2-50); TCB là
        // donee thì trả SC về client và đánh thức client (reply sẽ không tới).
        self.release_donation_of(obj)?;
        // Invariant §4.4: gỡ khỏi hàng **trước** khi bật cờ để bất biến kiểm được sau event.
        self.unlink_tcb_queues(obj);
        if state.is_blocked() || matches!(state, TcbState::Running { .. }) {
            self.suspend_to_ready(obj)?;
        }
        self.set_tcb_suspended(obj, true)?;
        self.sched_after_op(true);
        Ok(())
    }

    /// `Tcb.Resume`: chỉ xoá cờ `suspended` + `exit_code` (D2-25); **không bao giờ** phát lại
    /// Call/Send/Recv/ReplyRecv (thiết kế §4.4 `:283`, DoD 7). Idempotent khi TCB không treo.
    pub fn tcb_resume(&mut self, tcb: SlotRef) -> Result<(), KError> {
        let obj = self.require_type(tcb, ObjType::Tcb)?;
        if self.tcb_state(obj)? == TcbState::Dead {
            return Err(KError::IllegalOperation);
        }
        let dang_treo = self.tcb_info_obj(obj)?.suspended;
        self.body_mut(obj, |k| match k {
            Kind::Tcb(t) => {
                t.suspended = false;
                t.exit_code = None;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        // KT-19 DoD 5: chỉ khi thực sự gỡ cờ `suspended` mới enqueue lại **đúng priority**;
        // `Resume` trên TCB đang chạy không được làm nó nhảy về đuôi hàng FIFO.
        if dang_treo {
            self.sched_sync(obj)?;
        }
        self.sched_after_op(true);
        Ok(())
    }

    /// **KT-26** — model API `[TK]` `tcb_resume_with_regs(tcb, regs)`: cho TCB **chạy tiếp** sau
    /// fault với **thanh ghi/MR mới**, đúng đường thiết kế §4.4 `:279` — *"message ghi thẳng vào
    /// context + IPC buffer rồi mới chuyển `Ready`. Đường mất message kiểu r319 không biểu diễn
    /// được"*. Đây là **một cửa** cho fault-resume:
    ///
    /// - Ghi `regs` qua **cùng** [`crate::ipc::transfer`] (một message thanh ghi được `admit` +
    ///   giao như message thường: `ipc_sent`/`ipc_delivered` cùng tăng, `check_ipc` giữ nguyên)
    ///   **trước** khi đổi trạng thái ⇒ không có cửa sổ "Ready nhưng message chưa tới" (r319).
    /// - Nhận các trạng thái: `BlockedOnReply{reply}` (fault đã giao, procsrv đang giữ Reply),
    ///   `BlockedOnSend{call:true, fault:Some(k≠Exit)}` (fault còn trong hàng — huỷ message đó),
    ///   `Ready` (bỏ cờ `suspended` — `Tcb.Resume` + thanh ghi) và `Sleeping` (giữ **nguyên**
    ///   `Sleeping{reason}`, bảng §4.5 `:309-310`). Trạng thái khác ⇒ `IllegalOperation`.
    /// - **Không phát lại** Call/Send/Recv/Reply cũ: trả TCB về `Ready` (hoặc giữ `Sleeping`)
    ///   **đúng priority** của nó; không đụng `exit_code` (khác `Tcb.Resume` của KT-18 — D2-25
    ///   chỉ buộc `Tcb.Resume`/`Configure` xoá `exit_code`; đường fault-resume phải **giữ** mã
    ///   để procsrv còn đọc được).
    /// - Message **không** mang cap (model API cho thanh ghi; chuyển cap thuộc Call/Reply).
    pub fn tcb_resume_with_regs(&mut self, tcb: SlotRef, msg: Message) -> Result<(), KError> {
        let obj = self.require_type(tcb, ObjType::Tcb)?;
        if !msg.caps.is_empty() || msg.caps.len() != msg.info.caps() as usize {
            return Err(KError::InvalidArgument);
        }
        let state = self.tcb_state(obj)?;
        let suspended = self.tcb_info_obj(obj)?.suspended;
        // Trạng thái đích — kiểm **trước** khi gỡ hàng/giao thanh ghi (lỗi không đổi gì).
        match state {
            TcbState::BlockedOnReply { reply } => {
                if self.reply_caller(reply)? != Some(obj) {
                    return Err(KError::IllegalOperation);
                }
            }
            TcbState::BlockedOnSend {
                call: true,
                fault: Some(kind),
                ..
            } if !kind.is_exit() => {}
            TcbState::Ready => {}
            TcbState::Sleeping { .. } => {}
            _ => return Err(KError::IllegalOperation),
        }
        // Gỡ khỏi **mọi** hàng chờ/Reply.caller **trước** khi giao (bất biến §4.4); với
        // `BlockedOnSend` đây là chỗ huỷ message fault đang chờ (fault message không có `msg_id`
        // nên không tăng `ipc_dropped`).
        self.unlink_tcb_queues(obj);
        // Ghi thanh ghi **trước** khi `Ready` — qua đúng một hàm `ipc::transfer`.
        let snaps = self.resolve_caps(&msg)?;
        let mut m = self.admit_message(msg, snaps.clone())?;
        self.deliver_message(obj, &mut m, &snaps, None)?;
        // Bỏ cờ `suspended` (nếu có) rồi về trạng thái đích qua **một** cửa `transition_tcb`.
        if suspended {
            self.set_tcb_suspended(obj, false)?;
        }
        match state {
            // `Sleeping` giữ nguyên (bảng §4.5 `:309-310`; không có cặp danh tính trong bảng).
            TcbState::Sleeping { .. } => {}
            // Đã `Ready`: nếu vừa bỏ `suspended`, `set_tcb_suspended(false)` đã sync đúng bucket;
            // nếu không, TCB vẫn ở đúng chỗ — **không** gọi `transition_tcb` (cặp danh tính cấm).
            TcbState::Ready => {}
            _ => self.wake_from(obj)?,
        }
        self.sched_after_op(true);
        Ok(())
    }

    /// Kích hoạt ngầm của **mô hình K1b**: mô hình chưa có method `Activate`, nên TCB
    /// `Inactive` tự chuyển `Inactive → Ready` khi nó phát syscall IPC đầu tiên — cặp
    /// `(Inactive, Ready)` của bảng ALLOWED. KHÔNG phải ABI; KT-19 thêm `pick`/`Ready`.
    pub(super) fn ensure_started(&mut self, tcb: ObjId) -> Result<TcbState, KError> {
        let state = self.tcb_state(tcb)?;
        if state == TcbState::Inactive {
            self.start_tcb(tcb)?;
            return Ok(TcbState::Ready);
        }
        Ok(state)
    }

    pub(super) fn set_tcb_suspended(&mut self, tcb: ObjId, suspended: bool) -> Result<(), KError> {
        self.body_mut(tcb, |k| match k {
            Kind::Tcb(t) => {
                t.suspended = suspended;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        // `suspended` trực giao với state: bật ⇒ gỡ khỏi ready queue; tắt ⇒ sched_sync
        // của caller đưa lại đúng mức (KT-19 DoD 5).
        self.sched_sync(tcb)
    }

    /// Gỡ `tcb` khỏi **mọi** hàng chờ nó đang nằm trong: hàng gửi/nhận của Endpoint,
    /// `Reply.caller`, waiter của Notification, bucket WaitOnAddress. Trả **số hàng** đã gỡ.
    ///
    /// Gọi **trước** khi bật `suspended`/chuyển trạng thái (thiết kế §4.4 invariant: TCB nằm
    /// trong hàng của X **khi và chỉ khi** trạng thái mang X **và** `!suspended`).
    pub(super) fn unlink_tcb_queues(&mut self, tcb: ObjId) -> usize {
        let live: Vec<(ObjId, ObjType)> = self
            .objects
            .iter()
            .enumerate()
            .filter(|(_, m)| m.alive)
            .map(|(i, m)| ((i as u32) + 1, m.ty))
            .collect();
        let mut removed = 0usize;
        // KT-24: message **người dùng** bị huỷ theo TCB (D2-25) phải đếm vào `ipc_dropped`
        // để bất biến `ipc_sent == ipc_delivered + ipc_pending + ipc_dropped` giữ nguyên.
        let mut msg_dropped = 0usize;
        for (obj, ty) in live {
            match ty {
                ObjType::Endpoint => {
                    let _ = self.body_mut(obj, |k| match k {
                        Kind::Endpoint(e) => {
                            if let Some((waiter, _)) = e.recv {
                                if waiter == tcb {
                                    e.recv = None;
                                    removed += 1;
                                }
                            }
                            // KT-22: gỡ message của TCB khỏi **mailbox cố định**; `retain`
                            // đếm `removed` nên bất biến `pushed == popped + removed + len`
                            // vẫn đúng sau khi huỷ message (D2-25).
                            removed += e.send.retain(|q| {
                                if q.sender != tcb {
                                    return true;
                                }
                                if q.msg.is_some() {
                                    msg_dropped += 1;
                                }
                                false
                            });
                            Ok(())
                        }
                        _ => Err(KError::WrongObjectType),
                    });
                }
                ObjType::Reply => {
                    let _ = self.body_mut(obj, |k| match k {
                        Kind::Reply(r) => {
                            if r.caller == Some(tcb) {
                                r.caller = None;
                                removed += 1;
                            }
                            Ok(())
                        }
                        _ => Err(KError::WrongObjectType),
                    });
                }
                ObjType::Notification => {
                    let _ = self.body_mut(obj, |k| match k {
                        Kind::Notification(n) => {
                            if n.waiter == Some(tcb) {
                                n.waiter = None;
                                removed += 1;
                            }
                            Ok(())
                        }
                        _ => Err(KError::WrongObjectType),
                    });
                }
                _ => {}
            }
        }
        // KT-28: bảng băm — gỡ theo **đúng bucket** suy từ trạng thái của TCB (khoá nằm trong
        // `BlockedOnAddress{frame, offset}`), không quét tuyến tính toàn bảng. Nếu trạng thái
        // đã bị đổi trước (đường hỏng), `debug_assert` bắt phần còn sót; `check_waitaddr` bắt
        // trên mọi build sau public op.
        let mut da_go = 0usize;
        if let Ok(TcbState::BlockedOnAddress { frame, offset }) = self.tcb_state(tcb) {
            let bucket = waitaddr_bucket(frame, offset);
            let before = self.address_waiters[bucket].len();
            self.address_waiters[bucket].retain(|(_, _, t)| *t != tcb);
            da_go = before - self.address_waiters[bucket].len();
        }
        debug_assert!(
            self.address_waiters
                .iter()
                .all(|v| v.iter().all(|(_, _, t)| *t != tcb)),
            "KT-28: còn nút waiter của TCB {tcb} sau khi gỡ khỏi bucket"
        );
        removed += da_go;
        self.ipc_dropped += msg_dropped as u64;
        removed
    }

    pub(super) fn reply_set_caller(
        &mut self,
        reply: ObjId,
        caller: Option<ObjId>,
    ) -> Result<(), KError> {
        self.body_mut(reply, |k| match k {
            Kind::Reply(r) => {
                r.caller = caller;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })
    }

    /// Đánh thức **một** TCB đang chờ object bị finalise: chỉ khi trạng thái hiện tại khớp
    /// (`khop`), đặt `last_ipc_error = Cancelled` rồi `→ Ready` qua **một** cửa
    /// [`Machine::transition_tcb`] (hook KT-21 tự trả donation; `sched_sync` đưa vào ready queue).
    pub(super) fn wake_cancelled<F: Fn(&TcbState) -> bool>(&mut self, tcb: ObjId, khop: F) {
        let Ok(st) = self.tcb_state(tcb) else { return };
        if !khop(&st) {
            return;
        }
        let _ = self.set_last_ipc_error(tcb, KError::Cancelled);
        let _ = self.wake_from(tcb);
    }

    /// Bỏ tham chiếu tới object/cap chết (thiết kế §4.2: không trường nào trỏ tới object chết).
    pub(super) fn purge_dead_refs(&mut self) {
        let alive_objs: Vec<bool> = self.objects.iter().map(|m| m.alive).collect();
        let alive_caps: Vec<bool> = self.caps.iter().map(|c| c.alive).collect();
        let n = alive_objs.len();
        let n_caps = alive_caps.len();
        let is_alive =
            |id: ObjId| -> bool { id > 0 && (id as usize) <= n && alive_objs[id as usize - 1] };
        let cap_alive = |cap: CapId| -> bool {
            cap > 0 && (cap as usize) <= n_caps && alive_caps[cap as usize - 1]
        };
        // **KT-27** — binding SC↔Notification còn hiệu lực: `sc_bind[sc]` = Notification mà SC
        // đang bind (cap Notification còn sống). Tính **trước** vòng `body_mut` để dọn
        // `Notification.bound_sc` mồ côi trong cùng lượt purge.
        let mut sc_bind: Vec<Option<ObjId>> = vec![None; n + 1];
        for (i, alive) in alive_objs.iter().enumerate() {
            if !*alive {
                continue;
            }
            let obj = (i + 1) as ObjId;
            let is_sc = self.objects[i].ty == ObjType::SchedContext;
            if !is_sc {
                continue;
            }
            let bound = self
                .body(obj, |k| match k {
                    Kind::SchedContext(s) => Ok(s.bound),
                    _ => Err(KError::WrongObjectType),
                })
                .ok()
                .flatten();
            if let Some(cap) = bound {
                if cap_alive(cap) {
                    let c = &self.caps[cap as usize - 1];
                    if c.ty == ObjType::Notification {
                        sc_bind[obj as usize] = Some(c.obj);
                    }
                }
            }
        }
        let mut tong_dropped = 0u64;
        for (i, alive) in alive_objs.iter().enumerate() {
            if !*alive {
                continue;
            }
            let obj = (i + 1) as ObjId;
            // KT-24: message người dùng bị gỡ ở đây (sender đã chết) cũng phải đếm `ipc_dropped`.
            let mut dropped = 0u64;
            let _ = self.body_mut(obj, |k| {
                match k {
                    Kind::Endpoint(e) => {
                        if let Some((tcb, reply_cap)) = e.recv {
                            if !is_alive(tcb) {
                                e.recv = None;
                            } else if let Some(cap) = reply_cap {
                                // **KT-25**: Reply cap của waiter đã bị xoá ⇒ coi như `Recv`
                                // không cấp Reply (Q7 nhánh (b) khi Call tới) — không giữ cap chết.
                                if !cap_alive(cap) {
                                    e.recv = Some((tcb, None));
                                }
                            }
                        }
                        e.send.retain(|q| {
                            if is_alive(q.sender) {
                                return true;
                            }
                            if q.msg.is_some() {
                                dropped += 1;
                            }
                            false
                        });
                    }
                    Kind::Reply(r) => {
                        if let Some(c) = r.caller {
                            if !is_alive(c) {
                                r.caller = None;
                            }
                        }
                    }
                    Kind::Notification(n) => {
                        if let Some(w) = n.waiter {
                            if !is_alive(w) {
                                n.waiter = None;
                            }
                        }
                        // **KT-27**: binding SC mồ côi (SC chết/không còn bind notification
                        // này) phải gỡ — giữ bất biến 1:1 hai chiều của `check_notifications`.
                        if let Some(sc) = n.bound_sc {
                            if sc_bind.get(sc as usize).copied().flatten() != Some(obj) {
                                n.bound_sc = None;
                            }
                        }
                    }
                    Kind::Tcb(t) => {
                        if let Some(cap) = t.fault_ep {
                            if !cap_alive(cap) {
                                t.fault_ep = None;
                            }
                        }
                        if let Some(c) = t.cspace {
                            if !is_alive(c) {
                                t.cspace = None;
                            }
                        }
                        if let Some(c) = t.sched_context {
                            if !is_alive(c) {
                                t.sched_context = None;
                            }
                        }
                        if let Some(c) = t.bound_notification {
                            if !is_alive(c) {
                                t.bound_notification = None;
                            }
                        }
                        if let Some(c) = t.donated_sc {
                            if !is_alive(c) {
                                t.donated_sc = None;
                            }
                        }
                    }
                    Kind::SchedContext(sd) => {
                        // KT-21: donation chỉ còn hiệu lực khi **cả hai** TCB sống.
                        let con = match (sd.donated_to, sd.donor) {
                            (Some(d), Some(o)) => is_alive(d) && is_alive(o),
                            (None, None) => true,
                            _ => false,
                        };
                        if !con {
                            sd.donated_to = None;
                            sd.donor = None;
                        }
                        // **KT-27**: `SchedData.bound` trỏ cap đã chết ⇒ gỡ (bất biến
                        // `check_sc_refs` "bound cap còn sống"; Notification chết theo Untyped
                        // cũng đi qua đây).
                        if let Some(cap) = sd.bound {
                            if !cap_alive(cap) {
                                sd.bound = None;
                            }
                        }
                    }
                    _ => {}
                }
                Ok(())
            });
            tong_dropped += dropped;
        }
        self.ipc_dropped += tong_dropped;
        // Bucket WaitOnAddress: bỏ waiter/nút có frame hoặc TCB đã chết (§4.7). KT-28: quét
        // **từng bucket** của bảng băm (không còn `Vec` tuyến tính).
        for bucket in self.address_waiters.iter_mut() {
            bucket.retain(|(frame, _, tcb)| is_alive(*frame) && is_alive(*tcb));
        }
    }
}
