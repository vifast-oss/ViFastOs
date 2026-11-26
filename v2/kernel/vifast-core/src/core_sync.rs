// SPDX-License-Identifier: Apache-2.0 OR MIT
//! KT-60 — `impl Machine` cho **Notification** (KT-27) và **WaitOnAddress** (KT-28): port
//! `no_std` từ `v2/sim/vifast-sim/src/kobj.rs` (oracle), tách khỏi `core_ipc.rs` để giữ
//! giới hạn dòng mỗi tệp (QD §3.15).
//!
//! - `Signal`: `word |= badge`, `badge |= badge`, đánh thức **một** waiter (hoặc TCB **bound**
//!   đang `BlockedOnRecv`), giao `(word, badge)` rồi xoá cả hai; badge 0 hợp lệ.
//! - `Wait`/`Poll` + **một** waiter (TCB thứ hai ⇒ `Busy`), bound TCB 1:1 (`Busy` phía
//!   Notification).
//! - `WaitAddr`/`WakeAddr`: bảng băm 64 bucket, khoá đầy đủ `(frame, offset)`, FIFO, **không
//!   timeout** (D2-53 Q4 — `tick` ảo không đánh thức waiter).
//!
//! Model-only `[TK]`, không phải ABI. Bảng có biên tĩnh; đầy ⇒ `NotEnoughMemory` nguyên tử.

use crate::error::{CapId, KError, ObjId, ObjType, SlotRef};
use crate::machine::Machine;
use crate::objstate::{waitaddr_bucket, AddrWord, WaitNode, NODE_NONE, WAITADDR_BUCKETS};
use crate::observer::CoreObserver;
use crate::observer::TestObserver;
use crate::tcb::TcbState;
use crate::NonReadyTarget;

use crate::core_ipc::NotificationValue;
use crate::store::{KernelStore, StaticStore};

impl<S: KernelStore, O: CoreObserver> Machine<S, O> {
    // ---------------------------------------------------------------- Notification

    /// `Signal`: `word |= badge`, `badge |= badge`; đánh thức **một** waiter (hoặc TCB bound
    /// đang `BlockedOnRecv`), giao `(word, badge)` rồi xoá cả hai. Badge 0 hợp lệ.
    pub fn notification_signal(&mut self, ntfn: ObjId, badge: u64) -> Result<(), KError> {
        let (word, acc_badge, waiter) = {
            let n = self.notification_data(ntfn)?;
            (n.word, n.badge, n.waiter)
        };
        let new_word = word | badge;
        let new_badge = acc_badge | badge;
        let target = match waiter {
            Some(t) => {
                if self.tcb_state(t)? != (TcbState::BlockedOnNotification { ntfn }) {
                    return Err(KError::IllegalOperation);
                }
                Some(t)
            }
            None => self.bound_tcb_blocked_on_recv(ntfn)?,
        };
        let Some(target) = target else {
            let n = self.notification_data_mut(ntfn)?;
            n.word = new_word;
            n.badge = new_badge;
            self.sched_after_op(false);
            return Ok(());
        };
        {
            let n = self.notification_data_mut(ntfn)?;
            n.word = new_word;
            n.badge = new_badge;
            n.waiter = None;
        }
        self.deliver_notification(target, new_word, new_badge)?;
        self.unlink_tcb_queues(target);
        self.wake_from(target)?;
        {
            let n = self.notification_data_mut(ntfn)?;
            n.word = 0;
            n.badge = 0;
        }
        self.sched_after_op(true);
        Ok(())
    }

    /// `Send`/`NBSend` trên Notification = `Signal` với badge của cap.
    pub(crate) fn notification_signal_via_cap(
        &mut self,
        tcb: ObjId,
        cap: SlotRef,
    ) -> Result<(), KError> {
        let cap_id = self.ipc_slot_cap(cap)?;
        let (ntfn_obj, badge) = {
            let c = self.cap(cap_id)?;
            if !c.alive() {
                return Err(KError::InvalidCapability);
            }
            if c.ty != ObjType::Notification {
                return Err(KError::WrongObjectType);
            }
            (c.obj, c.badge().unwrap_or(0))
        };
        let state = self.ensure_started(tcb)?;
        if state != TcbState::Ready && !matches!(state, TcbState::Running { .. }) {
            return Err(KError::IllegalOperation);
        }
        self.reset_ipc_error(tcb)?;
        self.notification_signal(ntfn_obj, badge)
    }

    /// `Wait` trên Notification: word ≠ 0 ⇒ giao ngay; rỗng ⇒ chặn (một waiter, thứ hai
    /// `Busy`).
    pub fn notification_wait(&mut self, tcb: ObjId, ntfn: ObjId) -> Result<(), KError> {
        let state = self.ensure_started(tcb)?;
        if state != TcbState::Ready && !matches!(state, TcbState::Running { .. }) {
            return Err(KError::IllegalOperation);
        }
        let (word, badge, busy) = {
            let n = self.notification_data(ntfn)?;
            (n.word, n.badge, n.waiter.is_some())
        };
        if word != 0 {
            {
                let n = self.notification_data_mut(ntfn)?;
                n.word = 0;
                n.badge = 0;
            }
            self.deliver_notification(tcb, word, badge)?;
            self.sched_after_op(false);
            return Ok(());
        }
        if busy {
            return Err(KError::Busy);
        }
        self.notification_data_mut(ntfn)?.waiter = Some(tcb);
        self.transition_tcb(tcb, NonReadyTarget::BlockedOnNotification { ntfn })?;
        self.sched_after_op(true);
        Ok(())
    }

    /// `Poll` trên Notification.
    pub fn notification_poll(&mut self, ntfn: ObjId) -> Result<Option<NotificationValue>, KError> {
        let (word, badge) = {
            let n = self.notification_data(ntfn)?;
            (n.word, n.badge)
        };
        if word == 0 {
            return Ok(None);
        }
        {
            let n = self.notification_data_mut(ntfn)?;
            n.word = 0;
            n.badge = 0;
        }
        self.sched_after_op(false);
        Ok(Some(NotificationValue { word, badge }))
    }

    pub(crate) fn deliver_notification(
        &mut self,
        tcb: ObjId,
        word: u64,
        badge: u64,
    ) -> Result<(), KError> {
        self.tcb_data_mut(tcb)?.notif = Some((word, badge));
        Ok(())
    }

    fn bound_tcb_blocked_on_recv(&self, ntfn: ObjId) -> Result<Option<ObjId>, KError> {
        for i in 0..self.n_objects {
            let (alive, ty) = {
                let m = &self.objects[i];
                (m.alive, m.ty)
            };
            if !alive || ty != ObjType::Tcb {
                continue;
            }
            let tcb = (i as u32) + 1;
            let t = self.tcb_data(tcb)?;
            if t.bound_notification == Some(ntfn)
                && matches!(t.state, TcbState::BlockedOnRecv { .. })
            {
                return Ok(Some(tcb));
            }
        }
        Ok(None)
    }

    /// `Tcb.BindNotification` (model API `[TK]`): 1:1 phía TCB + phía Notification.
    pub fn tcb_bind_notification(&mut self, tcb: SlotRef, ntfn: SlotRef) -> Result<(), KError> {
        let tcb_obj = self.require_type(tcb, ObjType::Tcb)?;
        let ntfn_obj = self.require_type(ntfn, ObjType::Notification)?;
        let current = self.tcb_data(tcb_obj)?.bound_notification;
        if let Some(cur) = current {
            if cur == ntfn_obj {
                return Ok(());
            }
            return Err(KError::IllegalOperation);
        }
        for i in 0..self.n_objects {
            let (alive, ty) = {
                let m = &self.objects[i];
                (m.alive, m.ty)
            };
            if !alive || ty != ObjType::Tcb {
                continue;
            }
            let other = (i as u32) + 1;
            if other == tcb_obj {
                continue;
            }
            if self.tcb_data(other)?.bound_notification == Some(ntfn_obj) {
                return Err(KError::Busy);
            }
        }
        self.tcb_data_mut(tcb_obj)?.bound_notification = Some(ntfn_obj);
        self.sched_after_op(false);
        Ok(())
    }

    /// `Tcb.UnbindNotification` — idempotent.
    pub fn tcb_unbind_notification(&mut self, tcb: SlotRef) -> Result<(), KError> {
        let tcb_obj = self.require_type(tcb, ObjType::Tcb)?;
        self.tcb_data_mut(tcb_obj)?.bound_notification = None;
        self.sched_after_op(false);
        Ok(())
    }

    /// **KT-417** — Notification đang gắn với TCB (`None` = chưa gắn). Đọc cho test host/TAP.
    pub fn tcb_bound_notification(&self, tcb: ObjId) -> Result<Option<ObjId>, KError> {
        Ok(self.tcb_data(tcb)?.bound_notification)
    }

    /// `(word, badge)` của lần `Wait`/`Poll` gần nhất giao cho TCB.
    pub fn tcb_notification(&self, tcb: ObjId) -> Result<Option<NotificationValue>, KError> {
        Ok(self
            .tcb_data(tcb)?
            .notif
            .map(|(word, badge)| NotificationValue { word, badge }))
    }

    /// Như [`Machine::tcb_notification`] nhưng **tiêu thụ** bản ghi.
    pub fn take_tcb_notification(
        &mut self,
        tcb: ObjId,
    ) -> Result<Option<NotificationValue>, KError> {
        Ok(self
            .tcb_data_mut(tcb)?
            .notif
            .take()
            .map(|(word, badge)| NotificationValue { word, badge }))
    }

    /// Word hiện tại của Notification.
    pub fn notification_word(&self, ntfn: ObjId) -> Result<u64, KError> {
        Ok(self.notification_data(ntfn)?.word)
    }

    /// Badge hiện tại của Notification.
    pub fn notification_badge(&self, ntfn: ObjId) -> Result<u64, KError> {
        Ok(self.notification_data(ntfn)?.badge)
    }

    /// Waiter của Notification.
    pub fn notification_waiter(&self, ntfn: ObjId) -> Result<Option<ObjId>, KError> {
        Ok(self.notification_data(ntfn)?.waiter)
    }

    /// SC bound với Notification.
    pub fn notification_bound_sc(&self, ntfn: ObjId) -> Result<Option<ObjId>, KError> {
        Ok(self.notification_data(ntfn)?.bound_sc)
    }

    // ------------------------------------------------------------- SchedContext.Bind (KT-64)

    /// `SchedContext.Bind(sched, target)` — port `sched_context_bind` của sim (KT-27/KT-64):
    /// chỉ **gắn/kiểm** quan hệ cấu trúc, **không** tạo/xoá donation, **không** đổi ngân sách.
    ///
    /// - `target` phải là cap **Reply** hoặc **Notification** (spec `[method_signature.
    ///   "SchedContext.Bind"]`); loại khác ⇒ `WrongObjectType`; cap chết ⇒ `InvalidCapability`.
    /// - **1:1 phía Notification**: notification đã bind **SC khác** ⇒ `IllegalOperation` và
    ///   **không** đổi gì (kiểm trước mọi mutation).
    /// - SC đổi target ⇒ binding cũ trên notification được **gỡ** (không để `bound_sc` mồ côi).
    pub fn sched_context_bind(&mut self, sched: SlotRef, target: SlotRef) -> Result<(), KError> {
        let sched_obj = self.require_type(sched, ObjType::SchedContext)?;
        let target_cap = self.cap_at(target.cnode, target.slot)?;
        let target_ty = {
            let c = self.cap(target_cap)?;
            if !c.alive() {
                return Err(KError::InvalidCapability);
            }
            c.ty
        };
        if !matches!(target_ty, ObjType::Reply | ObjType::Notification) {
            return Err(KError::WrongObjectType);
        }
        // Kiểm 1:1 phía Notification **trước** khi đổi bất cứ trạng thái nào.
        if target_ty == ObjType::Notification {
            let ntfn_obj = self.cap(target_cap)?.obj;
            if let Some(other) = self.notification_bound_sc(ntfn_obj)? {
                if other != sched_obj {
                    return Err(KError::IllegalOperation);
                }
            }
        }
        // Gỡ binding cũ của SC (nếu đang trỏ Notification khác) — giữ 1:1 hai chiều.
        let old_bound = self.sc_data(sched_obj)?.bound;
        if let Some(old_cap) = old_bound {
            if old_cap != target_cap {
                let old = self
                    .cap(old_cap)
                    .ok()
                    .map(|c| (c.obj, c.ty, c.alive(), c.badge()));
                if let Some((old_obj, ObjType::Notification, true, _)) = old {
                    let n = self.notification_data_mut(old_obj)?;
                    if n.bound_sc == Some(sched_obj) {
                        n.bound_sc = None;
                    }
                }
            }
        }
        self.sc_data_mut(sched_obj)?.bound = Some(target_cap);
        if target_ty == ObjType::Notification {
            let ntfn_obj = self.cap(target_cap)?.obj;
            self.notification_data_mut(ntfn_obj)?.bound_sc = Some(sched_obj);
        }
        self.sched_after_op(false);
        Ok(())
    }

    /// `SchedContext.Unbind` — gỡ binding của SC (cả hai chiều), **idempotent**.
    ///
    /// Sim K1–K2 **không** mô hình hoá method này (chỉ có `Bind`); core cài theo
    /// `spec/abi.toml` (method 1 của `SchedContext`) và pin bằng test core-only. `Unbind` trên
    /// SC chưa bind ⇒ `Ok` (không lỗi).
    pub fn sched_context_unbind(&mut self, sched: SlotRef) -> Result<(), KError> {
        let sched_obj = self.require_type(sched, ObjType::SchedContext)?;
        let bound = self.sc_data(sched_obj)?.bound;
        if let Some(cap) = bound {
            let target = self.cap(cap).ok().map(|c| (c.obj, c.ty, c.alive()));
            if let Some((obj, ObjType::Notification, true)) = target {
                let n = self.notification_data_mut(obj)?;
                if n.bound_sc == Some(sched_obj) {
                    n.bound_sc = None;
                }
            }
        }
        self.sc_data_mut(sched_obj)?.bound = None;
        self.sched_after_op(false);
        Ok(())
    }

    /// Cap mà `SchedContext` đang bind (`None` = chưa bind) — ảnh chụp cho test/parity.
    pub fn sched_context_bound(&self, sc: ObjId) -> Result<Option<CapId>, KError> {
        Ok(self.sc_data(sc)?.bound)
    }

    /// **KT-64** — cap Notification `cap` (trỏ `obj`) bị xoá: gỡ `SchedContext.bound` đang trỏ
    /// cap đó + `Notification.bound_sc` phía đối diện (1:1 hai chiều). Gọi từ `Machine::delete`.
    pub(crate) fn drop_sched_binding_to_cap(&mut self, cap: CapId, obj: ObjId) {
        let mut idx = 0usize;
        while idx < self.n_objects {
            let (alive, ty, body) = {
                let m = &self.objects[idx];
                (m.alive, m.ty, m.body)
            };
            if alive && ty == ObjType::SchedContext && body.alive {
                let sc = idx as u32 + 1;
                let mut clear = false;
                if let Some(d) = self.scs.get(body.idx as usize) {
                    clear = d.bound == Some(cap);
                }
                if clear {
                    if let Some(d) = self.scs.get_mut(body.idx as usize) {
                        d.bound = None;
                    }
                    if let Ok(n) = self.notification_data_mut(obj) {
                        if n.bound_sc == Some(sc) {
                            n.bound_sc = None;
                        }
                    }
                }
            }
            idx += 1;
        }
    }

    /// **KT-64** — object `obj` (Notification) bị finalise: gỡ mọi `SchedContext.bound` trỏ nó
    /// (không để `bound` trỏ object chết). Gọi từ `Machine::revoke_children`.
    pub(crate) fn drop_sched_binding_to_obj(&mut self, obj: ObjId) {
        let mut idx = 0usize;
        while idx < self.n_objects {
            let (alive, ty, body) = {
                let m = &self.objects[idx];
                (m.alive, m.ty, m.body)
            };
            if alive && ty == ObjType::SchedContext && body.alive {
                let mut clear = false;
                if let Some(cap) = self.scs.get(body.idx as usize).and_then(|d| d.bound) {
                    if let Ok(c) = self.cap(cap) {
                        clear = c.obj == obj || !c.alive();
                    }
                }
                if clear {
                    if let Some(d) = self.scs.get_mut(body.idx as usize) {
                        d.bound = None;
                    }
                }
            }
            idx += 1;
        }
        if let Ok(n) = self.notification_data_mut(obj) {
            n.bound_sc = None;
        }
    }

    // ---------------------------------------------------------------- WaitAddr

    /// Ghi giá trị "từ" tại `(frame, offset)`; chưa có thì thêm (biên `MAX_ADDR_WORDS`).
    pub fn set_addr_word(&mut self, frame: ObjId, offset: u32, value: u64) -> Result<(), KError> {
        for w in &mut self.addr_words[..self.n_addr_words] {
            if w.alive && w.frame == frame && w.offset == offset {
                w.value = value;
                return Ok(());
            }
        }
        if self.n_addr_words >= self.addr_words.len() {
            return Err(KError::NotEnoughMemory);
        }
        let i = self.n_addr_words;
        self.addr_words[i] = AddrWord {
            frame,
            offset,
            value,
            alive: true,
        };
        self.n_addr_words += 1;
        Ok(())
    }

    /// Đọc giá trị "từ"; chưa ghi ⇒ 0.
    pub fn addr_word(&self, frame: ObjId, offset: u32) -> u64 {
        for w in &self.addr_words[..self.n_addr_words] {
            if w.alive && w.frame == frame && w.offset == offset {
                return w.value;
            }
        }
        0
    }

    fn wait_node_available(&self) -> bool {
        self.n_wait_free > 0 || self.n_wait_nodes < self.wait_nodes.len()
    }

    fn alloc_wait_node(&mut self, node: WaitNode) -> Option<u16> {
        let idx = if self.n_wait_free > 0 {
            self.n_wait_free -= 1;
            self.wait_free[self.n_wait_free] as usize
        } else if self.n_wait_nodes < self.wait_nodes.len() {
            let i = self.n_wait_nodes;
            self.n_wait_nodes += 1;
            i
        } else {
            return None;
        };
        self.wait_nodes[idx] = node;
        Some(idx as u16)
    }

    pub(crate) fn free_wait_node(&mut self, idx: u16) {
        if (idx as usize) < self.wait_nodes.len() {
            self.wait_nodes[idx as usize] = WaitNode::EMPTY;
            if self.n_wait_free < self.wait_free.len() {
                self.wait_free[self.n_wait_free] = idx;
                self.n_wait_free += 1;
            }
        }
    }

    /// `WaitAddr(frame, offset, expected)` — đọc giá trị và xếp hàng trong **cùng một hàm**
    /// (mô hình một luồng; không timeout — D2-53 Q4).
    pub fn wait_addr(
        &mut self,
        tcb: SlotRef,
        frame: ObjId,
        offset: u32,
        expected: u64,
    ) -> Result<(), KError> {
        let tcb_obj = self.require_type(tcb, ObjType::Tcb)?;
        self.wait_addr_steps(tcb_obj, frame, offset, expected)
    }

    /// `WaitAddr` nhận thẳng `ObjId` của TCB (KT-62: syscall dispatch giữ `ObjId`, không có slot
    /// cap của chính TCB). Cùng ngữ nghĩa [`Machine::wait_addr`] — **một** thân hàm.
    pub fn wait_addr_obj(
        &mut self,
        tcb: ObjId,
        frame: ObjId,
        offset: u32,
        expected: u64,
    ) -> Result<(), KError> {
        // `tcb_data` kiểm object sống + đúng kiểu TCB (cùng mã lỗi như `require_type`).
        self.tcb_data(tcb)?;
        self.wait_addr_steps(tcb, frame, offset, expected)
    }

    fn wait_addr_steps(
        &mut self,
        tcb_obj: ObjId,
        frame: ObjId,
        offset: u32,
        expected: u64,
    ) -> Result<(), KError> {
        let state = self.ensure_started(tcb_obj)?;
        if state != TcbState::Ready && !matches!(state, TcbState::Running { .. }) {
            return Err(KError::IllegalOperation);
        }
        if self.tcb_info_obj(tcb_obj)?.suspended {
            return Err(KError::IllegalOperation);
        }
        if self.addr_word(frame, offset) != expected {
            return Err(KError::InvalidArgument);
        }
        if !self.wait_node_available() {
            return Err(KError::NotEnoughMemory);
        }
        self.transition_tcb(tcb_obj, NonReadyTarget::BlockedOnAddress { frame, offset })?;
        let idx = self
            .alloc_wait_node(WaitNode {
                frame,
                offset,
                tcb: tcb_obj,
                next: NODE_NONE,
                alive: true,
            })
            .ok_or(KError::NotEnoughMemory)?;
        let b = waitaddr_bucket(frame, offset);
        let tail = self.wait_buckets_tail[b];
        if tail == NODE_NONE {
            self.wait_buckets[b] = idx;
        } else {
            self.wait_nodes[tail as usize].next = idx;
        }
        self.wait_buckets_tail[b] = idx;
        self.sched_after_op(true);
        Ok(())
    }

    /// `WaitAddr` bản K1b (không `expected`).
    pub fn tcb_wait_address(
        &mut self,
        tcb: SlotRef,
        frame: ObjId,
        offset: u32,
    ) -> Result<(), KError> {
        let current = self.addr_word(frame, offset);
        self.wait_addr(tcb, frame, offset, current)
    }

    /// `WakeAddr(frame, offset, n)`: đánh thức tối đa `n` waiter **cùng khoá đầy đủ** theo
    /// FIFO; trả số đã đánh thức. `n = u32::MAX` = tất cả.
    pub fn wake_addr(&mut self, frame: ObjId, offset: u32, n: u32) -> Result<u32, KError> {
        let b = waitaddr_bucket(frame, offset);
        let mut chon = [0u32; crate::store::GENERAL_MAX_WAIT_NODES];
        let mut nchon = 0usize;
        let mut cur = self.wait_buckets[b];
        while cur != NODE_NONE && nchon < n as usize && nchon < chon.len() {
            let node = self.wait_nodes[cur as usize];
            if node.alive && node.frame == frame && node.offset == offset {
                chon[nchon] = node.tcb;
                nchon += 1;
            }
            cur = node.next;
        }
        for &tcb in &chon[..nchon] {
            // **KT-155 (N7-4)**: gỡ bucket WaitAddr (`unlink = true`) **và** gửi `Wake` về
            // `home` khi waiter ở hart khác — trước đây gọi `wake_step` trực tiếp nên hart
            // `home` không được đánh thức.
            self.wake_from_unlink(tcb)?;
        }
        self.sched_after_op(false);
        Ok(nchon as u32)
    }

    /// TCB đang chờ ở `(frame, offset)` (đầu hàng FIFO).
    pub fn address_waiter(&self, frame: ObjId, offset: u32) -> Option<ObjId> {
        let b = waitaddr_bucket(frame, offset);
        let mut cur = self.wait_buckets[b];
        while cur != NODE_NONE {
            let node = self.wait_nodes[cur as usize];
            if node.alive && node.frame == frame && node.offset == offset {
                return Some(node.tcb);
            }
            cur = node.next;
        }
        None
    }

    /// Tổng số nút waiter WaitOnAddress còn sống.
    pub fn address_waiters_len(&self) -> usize {
        self.wait_nodes[..self.n_wait_nodes]
            .iter()
            .filter(|n| n.alive)
            .count()
    }

    /// Các waiter của đúng khoá `(frame, offset)` theo FIFO.
    pub fn address_waiters_for(&self, frame: ObjId, offset: u32) -> Waiters<'_, S, O> {
        Waiters {
            m: self,
            frame,
            offset,
            idx: self.wait_buckets[waitaddr_bucket(frame, offset)],
        }
    }

    /// Số waiter trong bucket.
    pub fn waitaddr_bucket_len(&self, bucket: usize) -> usize {
        let mut n = 0usize;
        let mut cur = self.wait_buckets[bucket];
        while cur != NODE_NONE {
            let node = self.wait_nodes[cur as usize];
            if node.alive {
                n += 1;
            }
            cur = node.next;
        }
        n
    }

    /// Số bucket khác rỗng.
    pub fn waitaddr_nonempty_buckets(&self) -> usize {
        (0..WAITADDR_BUCKETS)
            .filter(|&b| self.waitaddr_bucket_len(b) > 0)
            .count()
    }
}

/// Iterator waiter WaitOnAddress của đúng khoá `(frame, offset)`, theo FIFO.
#[derive(Debug, Clone)]
pub struct Waiters<'a, S: KernelStore = StaticStore, O: CoreObserver = TestObserver> {
    m: &'a Machine<S, O>,
    frame: ObjId,
    offset: u32,
    idx: u16,
}

impl<S: KernelStore, O: CoreObserver> Iterator for Waiters<'_, S, O> {
    type Item = ObjId;

    fn next(&mut self) -> Option<ObjId> {
        while self.idx != NODE_NONE {
            let node = self.m.wait_nodes[self.idx as usize];
            self.idx = node.next;
            if node.alive && node.frame == self.frame && node.offset == self.offset {
                return Some(node.tcb);
            }
        }
        None
    }
}
