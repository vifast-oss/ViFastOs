// SPDX-License-Identifier: Apache-2.0 OR MIT
//! KT-60 — phần `impl Machine` của **IPC/Reply/Notification/WaitAddr/fault**: port
//! `no_std` từ `v2/sim/vifast-sim/src/kobj.rs` (oracle), cùng mã lỗi, cùng thứ tự hàng chờ,
//! cùng bất biến.
//!
//! Phạm vi: `Endpoint` (hàng gửi FIFO 8 + **một** Recv waiter), `Reply` (`caller` +
//! `ReplyRecv`), `Notification` (word/badge OR, một waiter, bound TCB), `WaitAddr`
//! (futex không timeout — D2-53 Q4), **donation KT-21** gắn `Call`/`Reply`, và **fault**
//! (`Exit` đi `BlockedOnSend` không Reply; fault khác đi Call + Reply).
//!
//! **Hai hook còn nợ từ KT-59** được nối tại [`Machine::transition_tcb`] (`state_write.rs`):
//! `release_donation_of` khi rời `BlockedOnReply`; xung đột `Reply.caller` ⇒
//! `IllegalOperation`.
//!
//! **KT-63:** chuyển cap qua IPC (receive window 4 ô + `plan_cap_transfer` + `RIGHT_GRANT`)
//! và finalise IPC khi `Revoke`/`Delete` (huỷ `Reply.caller` ⇒ caller `Cancelled + Ready`;
//! đánh thức Recv/Notification waiter của object bị finalise — chống treo; huỷ hàng Endpoint
//! và đếm `ipc_dropped`). Mọi bảng có biên tĩnh; đầy ⇒ `NotEnoughMemory` **nguyên tử** (biên
//! triển khai, như `MAX_TCBS`).

use crate::error::{CapId, KError, ObjId, ObjType, SlotRef};
use crate::ipc::{CapPlan, IpcArea, Message, MAX_CAPS};
use crate::machine::{Machine, BODY_ENDPOINT, BODY_NOTIFICATION, BODY_REPLY, MAX_QUEUED_MESSAGES};
use crate::objstate::{
    EndpointData, NotificationData, QueuedMsg, ReplyData, ENDPOINT_QUEUE_CAP, MAX_WAIT_NODES,
    NODE_NONE,
};
use crate::tcb::{FaultKind, SleepReason, TcbState};
use crate::NonReadyTarget;

/// Giá trị `(word, badge)` của Notification (model API `[TK]`, KT-27) — như sim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NotificationValue {
    /// Word 64 bit (OR của các badge đã `Signal`).
    pub word: u64,
    /// Badge song song (bất biến `badge == word`).
    pub badge: u64,
}

/// Ảnh chụp **hộp thư của Endpoint** (model API `[TK]`, KT-22) — như sim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndpointInfo {
    /// Sức chứa cố định (`ENDPOINT_QUEUE_CAP` = 8).
    pub capacity: usize,
    /// Số message đang chờ.
    pub len: usize,
    /// Tổng đã đẩy.
    pub pushed: u64,
    /// Tổng đã lấy.
    pub popped: u64,
    /// Tổng bị huỷ (TCB chết/treo).
    pub removed: u64,
    /// TCB đang chờ `Recv` (tối đa một).
    pub recv_waiter: Option<ObjId>,
}

/// Ảnh chụp một message đang chờ trong hàng gửi Endpoint — model API `[TK]` (KT-60, đọc-only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueuedView {
    /// Người gửi.
    pub sender: ObjId,
    /// Message người dùng (None = fault).
    pub msg: Option<Message>,
    /// `Call` đang chờ Reply?
    pub call: bool,
    /// Fault mang theo (nếu có).
    pub fault: Option<FaultKind>,
    /// Người gửi có đang `BlockedOnSend` chờ giao không?
    pub blocking: bool,
}

impl Machine {
    // ---------------------------------------------------------------- body + pool

    fn body_slot(&self, obj: ObjId, ty: ObjType, kind: u8) -> Result<usize, KError> {
        let idx = self.object_index(obj)?;
        let m = &self.objects[idx];
        if !m.alive {
            return Err(KError::InvalidCapability);
        }
        if m.ty != ty {
            return Err(KError::WrongObjectType);
        }
        if m.body.kind != kind || !m.body.alive {
            return Err(KError::InvalidCapability);
        }
        Ok(m.body.idx as usize)
    }

    pub(crate) fn endpoint_data(&self, obj: ObjId) -> Result<&EndpointData, KError> {
        let i = self.body_slot(obj, ObjType::Endpoint, BODY_ENDPOINT)?;
        Ok(&self.endpoints[i])
    }

    fn endpoint_data_mut(&mut self, obj: ObjId) -> Result<&mut EndpointData, KError> {
        let i = self.body_slot(obj, ObjType::Endpoint, BODY_ENDPOINT)?;
        Ok(&mut self.endpoints[i])
    }

    pub(crate) fn reply_data(&self, obj: ObjId) -> Result<&ReplyData, KError> {
        let i = self.body_slot(obj, ObjType::Reply, BODY_REPLY)?;
        Ok(&self.replies[i])
    }

    pub(crate) fn reply_data_mut(&mut self, obj: ObjId) -> Result<&mut ReplyData, KError> {
        let i = self.body_slot(obj, ObjType::Reply, BODY_REPLY)?;
        Ok(&mut self.replies[i])
    }

    pub(crate) fn notification_data(&self, obj: ObjId) -> Result<&NotificationData, KError> {
        let i = self.body_slot(obj, ObjType::Notification, BODY_NOTIFICATION)?;
        Ok(&self.notifications[i])
    }

    pub(crate) fn notification_data_mut(
        &mut self,
        obj: ObjId,
    ) -> Result<&mut NotificationData, KError> {
        let i = self.body_slot(obj, ObjType::Notification, BODY_NOTIFICATION)?;
        Ok(&mut self.notifications[i])
    }

    fn ipc_msg_available(&self) -> bool {
        self.n_ipc_msg_free > 0 || self.n_ipc_msgs < MAX_QUEUED_MESSAGES
    }

    fn alloc_ipc_msg(&mut self, q: QueuedMsg) -> Option<u16> {
        let idx = if self.n_ipc_msg_free > 0 {
            self.n_ipc_msg_free -= 1;
            self.ipc_msg_free[self.n_ipc_msg_free] as usize
        } else if self.n_ipc_msgs < MAX_QUEUED_MESSAGES {
            let i = self.n_ipc_msgs;
            self.n_ipc_msgs += 1;
            i
        } else {
            return None;
        };
        self.ipc_msgs[idx] = q;
        Some(idx as u16)
    }

    pub(crate) fn free_ipc_msg(&mut self, idx: u16) {
        if (idx as usize) < MAX_QUEUED_MESSAGES {
            self.ipc_msgs[idx as usize] = QueuedMsg::EMPTY;
            if self.n_ipc_msg_free < MAX_QUEUED_MESSAGES {
                self.ipc_msg_free[self.n_ipc_msg_free] = idx;
                self.n_ipc_msg_free += 1;
            }
        }
    }

    // ---------------------------------------------------------------- cap helpers

    /// Cap trong slot của CSpace, ánh xạ `SlotEmpty` ⇒ `InvalidCapability` **như sim**
    /// (`ipc_slot_cap`).
    pub(crate) fn ipc_slot_cap(&self, slot: SlotRef) -> Result<CapId, KError> {
        self.cap_at(slot.cnode, slot.slot).map_err(|e| match e {
            KError::SlotEmpty => KError::InvalidCapability,
            other => other,
        })
    }

    /// Endpoint đích của `Call`: cap **không** phải Endpoint ⇒ `NotSupported` (D2-53 Q11).
    fn ipc_call_endpoint(&self, slot: SlotRef) -> Result<ObjId, KError> {
        let cap = self.ipc_slot_cap(slot)?;
        let c = self.cap(cap)?;
        if c.ty != ObjType::Endpoint {
            return Err(KError::NotSupported);
        }
        Ok(c.obj)
    }

    fn ipc_cap_type(&self, slot: SlotRef) -> Result<ObjType, KError> {
        let cap = self.ipc_slot_cap(slot)?;
        Ok(self.cap(cap)?.ty)
    }

    fn require_cap_type(&self, cap: CapId, ty: ObjType) -> Result<ObjId, KError> {
        let c = self.cap(cap)?;
        if !c.alive {
            return Err(KError::InvalidCapability);
        }
        if c.ty != ty {
            return Err(KError::WrongObjectType);
        }
        Ok(c.obj)
    }

    /// `ensure_started` của sim: `Inactive` ⇒ `Ready` (qua một cửa), trả trạng thái hiện tại.
    pub(crate) fn ensure_started(&mut self, tcb: ObjId) -> Result<TcbState, KError> {
        let state = self.tcb_state(tcb)?;
        if state == TcbState::Inactive {
            self.start_tcb(tcb)?;
            return Ok(TcbState::Ready);
        }
        Ok(state)
    }

    pub(crate) fn reset_ipc_error(&mut self, tcb: ObjId) -> Result<(), KError> {
        let t = self.tcb_data_mut(tcb)?;
        t.last_ipc_error = None;
        t.last_fault = None;
        Ok(())
    }

    pub(crate) fn set_last_ipc_error(&mut self, tcb: ObjId, e: KError) -> Result<(), KError> {
        self.tcb_data_mut(tcb)?.last_ipc_error = Some(e);
        Ok(())
    }

    fn record_last_fault(
        &mut self,
        receiver: ObjId,
        sender: ObjId,
        kind: FaultKind,
    ) -> Result<(), KError> {
        self.tcb_data_mut(receiver)?.last_fault = Some((sender, kind));
        Ok(())
    }

    // ---------------------------------------------------------------- Endpoint queue

    fn endpoint_recv_slot(&self, ep: ObjId) -> Result<(bool, Option<CapId>), KError> {
        let e = self.endpoint_data(ep)?;
        Ok((e.recv.is_some(), e.recv.and_then(|(_, cap)| cap)))
    }

    fn endpoint_take_waiter(&mut self, ep: ObjId) -> Result<Option<ObjId>, KError> {
        Ok(self.endpoint_data_mut(ep)?.recv.take().map(|(t, _)| t))
    }

    fn endpoint_full(&self, ep: ObjId) -> Result<bool, KError> {
        Ok(self.endpoint_data(ep)?.send.is_full())
    }

    /// TCB đang chờ `Recv` trên Endpoint (chỉ **đọc**, không gỡ hàng) — KT-63 dùng để kiểm
    /// receive window của bên nhận **trước** khi message vào đường IPC (như sim).
    fn endpoint_recv_waiter_obj(&self, ep: ObjId) -> Result<Option<ObjId>, KError> {
        Ok(self.endpoint_data(ep)?.recv.map(|(t, _)| t))
    }

    /// Message **đầu** hàng gửi của Endpoint (chỉ **đọc**, không pop) — KT-63 peek ảnh chụp
    /// cap để kiểm receive window **trước** khi pop (không giao nửa chừng, như sim).
    fn endpoint_front_msg(&self, ep: ObjId) -> Result<Option<Message>, KError> {
        let i = self.body_slot(ep, ObjType::Endpoint, BODY_ENDPOINT)?;
        let Some(mi) = self.endpoints[i].send.iter().next() else {
            return Ok(None);
        };
        Ok(self.ipc_msgs[mi as usize].msg)
    }

    fn endpoint_enqueue(&mut self, ep: ObjId, q: QueuedMsg) -> Result<(), KError> {
        let i = self.body_slot(ep, ObjType::Endpoint, BODY_ENDPOINT)?;
        if self.endpoints[i].send.is_full() {
            return Err(KError::Busy);
        }
        let Some(mi) = self.alloc_ipc_msg(q) else {
            return Err(KError::NotEnoughMemory);
        };
        match self.endpoints[i].send.try_push(mi) {
            Ok(()) => Ok(()),
            Err(mi) => {
                self.free_ipc_msg(mi);
                Err(KError::Busy)
            }
        }
    }

    fn endpoint_pop(&mut self, ep: ObjId) -> Result<Option<QueuedMsg>, KError> {
        let i = self.body_slot(ep, ObjType::Endpoint, BODY_ENDPOINT)?;
        let Some(mi) = self.endpoints[i].send.pop_front() else {
            return Ok(None);
        };
        let q = self.ipc_msgs[mi as usize];
        self.free_ipc_msg(mi);
        Ok(Some(q))
    }

    fn set_endpoint_last_mr(&mut self, ep: ObjId, mr: u64) -> Result<(), KError> {
        self.endpoint_data_mut(ep)?.last_mr = Some(mr);
        Ok(())
    }

    // ---------------------------------------------------------------- delivery

    /// Nhận message vào đường IPC: biên cap (`≤ 4`, khớp `MessageInfo.caps` + **đủ** ảnh
    /// chụp), cấp `msg_id` từ `ipc_sent` (không tái dùng) — như `admit_message` của sim.
    pub(crate) fn admit_message(&mut self, mut msg: Message) -> Result<Message, KError> {
        if msg.n_caps as usize > MAX_CAPS
            || msg.n_caps as usize != msg.info.caps() as usize
            || msg.n_snaps != msg.n_caps
        {
            return Err(KError::InvalidArgument);
        }
        self.ipc_sent += 1;
        msg.msg_id = self.ipc_sent;
        Ok(msg)
    }

    /// Giao message **đã xếp hàng** cho TCB nhận (`recv`/`nb_recv`), xử lý người gửi và
    /// nhánh `no_reply` (D2-53 Q7). Trả message đã giao (nếu là message người dùng).
    fn deliver_queued(
        &mut self,
        receiver: ObjId,
        q: QueuedMsg,
        reply_obj: Option<ObjId>,
        plan: Option<CapPlan>,
    ) -> Result<Option<Message>, KError> {
        let mut out = q.msg;
        let mut kernel_no_reply = false;
        if let Some(m) = out.as_mut() {
            if q.call && q.blocking && reply_obj.is_none() && !m.info.no_reply() {
                m.info = m.info.with_no_reply(true);
                kernel_no_reply = true;
            }
            self.deliver_message(receiver, m, plan.as_ref())?;
        }
        match q.fault {
            Some(kind) => {
                self.record_last_fault(receiver, q.sender, kind)?;
                if kind.is_exit() {
                    // Fault Exit: procsrv Recv xong mới Inactive (D2-25).
                    self.transition_tcb(q.sender, NonReadyTarget::Inactive)?;
                } else {
                    match (q.call, reply_obj) {
                        (true, Some(rep)) if q.blocking => {
                            self.reply_set_caller(rep, Some(q.sender))?;
                            self.transition_tcb(
                                q.sender,
                                NonReadyTarget::BlockedOnReply { reply: rep },
                            )?;
                            self.donate_sc(q.sender, receiver)?;
                        }
                        _ => {
                            if q.blocking {
                                self.set_last_ipc_error(q.sender, KError::Cancelled)?;
                                self.wake_from(q.sender)?;
                            }
                        }
                    }
                }
            }
            None => match (q.call, reply_obj) {
                (true, Some(rep)) if q.blocking => {
                    self.reply_set_caller(rep, Some(q.sender))?;
                    self.transition_tcb(q.sender, NonReadyTarget::BlockedOnReply { reply: rep })?;
                    self.donate_sc(q.sender, receiver)?;
                }
                _ => {
                    if q.blocking {
                        self.wake_from(q.sender)?;
                    }
                }
            },
        }
        if kernel_no_reply {
            self.set_last_ipc_error(q.sender, KError::Cancelled)?;
        }
        Ok(out)
    }

    // ---------------------------------------------------------------- Reply caller

    /// `Reply.caller` của object (lỗi body như sim).
    pub fn reply_caller(&self, reply: ObjId) -> Result<Option<ObjId>, KError> {
        Ok(self.reply_data(reply)?.caller)
    }

    pub(crate) fn reply_set_caller(
        &mut self,
        reply: ObjId,
        caller: Option<ObjId>,
    ) -> Result<(), KError> {
        self.reply_data_mut(reply)?.caller = caller;
        Ok(())
    }

    // ---------------------------------------------------------------- Call / Send

    /// `Call` (message đầy đủ) — port `Machine::call_msg` của sim.
    pub fn call_msg(&mut self, tcb: ObjId, ep: SlotRef, msg: Message) -> Result<(), KError> {
        let ep_obj = self.ipc_call_endpoint(ep)?;
        let state = self.ensure_started(tcb)?;
        if state != TcbState::Ready && !matches!(state, TcbState::Running { .. }) {
            return Err(KError::IllegalOperation);
        }
        self.reset_ipc_error(tcb)?;
        let (has_waiter, reply_cap) = self.endpoint_recv_slot(ep_obj)?;
        let reply_obj = match reply_cap {
            Some(cap) => Some(self.require_cap_type(cap, ObjType::Reply)?),
            None => None,
        };
        if !has_waiter && (self.endpoint_full(ep_obj)? || !self.ipc_msg_available()) {
            return Err(KError::Busy);
        }
        let user_no_reply = msg.info.no_reply();
        if !user_no_reply {
            if let Some(rep) = reply_obj {
                if self.reply_caller(rep)?.is_some() {
                    return Err(KError::Busy);
                }
            }
        }
        let mut msg = msg;
        self.resolve_caps(&mut msg)?;
        let plan = match self.endpoint_recv_waiter_obj(ep_obj)? {
            Some(w) => self.prepare_caps(w, &msg.snaps[..msg.n_snaps as usize], Some(ep_obj))?,
            None => None,
        };
        let mut msg = self.admit_message(msg)?;
        self.set_endpoint_last_mr(ep_obj, msg.mr[0])?;
        if has_waiter {
            let waiter = self
                .endpoint_take_waiter(ep_obj)?
                .expect("has_waiter vừa kiểm");
            let kernel_no_reply = !user_no_reply && reply_obj.is_none();
            if kernel_no_reply {
                msg.info = msg.info.with_no_reply(true);
            }
            self.deliver_message(waiter, &mut msg, plan.as_ref())?;
            self.wake_from(waiter)?;
            if user_no_reply {
                self.sched_after_op(true);
                return Ok(());
            }
            match reply_obj {
                Some(rep) => {
                    self.reply_set_caller(rep, Some(tcb))?;
                    self.transition_tcb(tcb, NonReadyTarget::BlockedOnReply { reply: rep })?;
                    self.donate_sc(tcb, waiter)?;
                }
                None => {
                    self.set_last_ipc_error(tcb, KError::Cancelled)?;
                    self.sched_after_op(true);
                    return Err(KError::Cancelled);
                }
            }
        } else {
            let blocking = !user_no_reply;
            self.endpoint_enqueue(
                ep_obj,
                QueuedMsg {
                    msg: Some(msg),
                    sender: tcb,
                    call: blocking,
                    fault: None,
                    blocking,
                    alive: true,
                },
            )?;
            if blocking {
                self.transition_tcb(
                    tcb,
                    NonReadyTarget::BlockedOnSend {
                        ep: ep_obj,
                        call: true,
                        fault: None,
                    },
                )?;
            }
        }
        self.sched_after_op(true);
        Ok(())
    }

    /// `Call` bản một MR (API K1b giữ nguyên).
    pub fn call(&mut self, tcb: ObjId, ep: SlotRef, mr: u64) -> Result<(), KError> {
        let info = crate::ipc::MessageInfo::new(0, 0, false, 0, 1)?;
        let msg = Message::new(info, &[mr])?;
        self.call_msg(tcb, ep, msg)
    }

    /// `Send` trên Endpoint/Reply/Notification — dispatch theo loại cap (spec `:107`).
    pub fn send(&mut self, tcb: ObjId, cap: SlotRef, msg: Message) -> Result<(), KError> {
        match self.ipc_cap_type(cap)? {
            ObjType::Endpoint => self.endpoint_send(tcb, cap, msg, true),
            ObjType::Reply => self.reply_msg(tcb, cap, msg),
            ObjType::Notification => self.notification_signal_via_cap(tcb, cap),
            _ => Err(KError::WrongObjectType),
        }
    }

    /// `NBSend` — không chặn; trên Notification cũng là `Signal`.
    pub fn nb_send(&mut self, tcb: ObjId, ep: SlotRef, msg: Message) -> Result<(), KError> {
        match self.ipc_cap_type(ep)? {
            ObjType::Endpoint => self.endpoint_send(tcb, ep, msg, false),
            ObjType::Notification => self.notification_signal_via_cap(tcb, ep),
            _ => Err(KError::WrongObjectType),
        }
    }

    fn endpoint_send(
        &mut self,
        tcb: ObjId,
        ep: SlotRef,
        msg: Message,
        blocking: bool,
    ) -> Result<(), KError> {
        let cap = self.ipc_slot_cap(ep)?;
        let ep_obj = self.require_cap_type(cap, ObjType::Endpoint)?;
        let state = self.ensure_started(tcb)?;
        if state != TcbState::Ready && !matches!(state, TcbState::Running { .. }) {
            return Err(KError::IllegalOperation);
        }
        self.reset_ipc_error(tcb)?;
        let (has_waiter, _) = self.endpoint_recv_slot(ep_obj)?;
        if !has_waiter && (self.endpoint_full(ep_obj)? || !self.ipc_msg_available()) {
            return Err(KError::Busy);
        }
        let mut msg = msg;
        self.resolve_caps(&mut msg)?;
        let plan = match self.endpoint_recv_waiter_obj(ep_obj)? {
            Some(w) => self.prepare_caps(w, &msg.snaps[..msg.n_snaps as usize], Some(ep_obj))?,
            None => None,
        };
        let mut msg = self.admit_message(msg)?;
        self.set_endpoint_last_mr(ep_obj, msg.mr[0])?;
        if has_waiter {
            let waiter = self
                .endpoint_take_waiter(ep_obj)?
                .expect("has_waiter vừa kiểm");
            self.deliver_message(waiter, &mut msg, plan.as_ref())?;
            self.wake_from(waiter)?;
        } else {
            self.endpoint_enqueue(
                ep_obj,
                QueuedMsg {
                    msg: Some(msg),
                    sender: tcb,
                    call: false,
                    fault: None,
                    blocking,
                    alive: true,
                },
            )?;
            if blocking {
                self.transition_tcb(
                    tcb,
                    NonReadyTarget::BlockedOnSend {
                        ep: ep_obj,
                        call: false,
                        fault: None,
                    },
                )?;
            }
        }
        self.sched_after_op(true);
        Ok(())
    }

    // ---------------------------------------------------------------- Recv / Reply

    /// `Recv` trên Endpoint hoặc `Wait` trên Notification (spec `:109`).
    pub fn recv(&mut self, tcb: ObjId, ep: SlotRef, reply: Option<SlotRef>) -> Result<(), KError> {
        let state = self.ensure_started(tcb)?;
        if state != TcbState::Ready && !matches!(state, TcbState::Running { .. }) {
            return Err(KError::IllegalOperation);
        }
        let cap = self.ipc_slot_cap(ep)?;
        match self.cap(cap)?.ty {
            ObjType::Notification => {
                if reply.is_some() {
                    return Err(KError::InvalidArgument);
                }
                let ntfn_obj = self.require_cap_type(cap, ObjType::Notification)?;
                self.reset_ipc_error(tcb)?;
                return self.notification_wait(tcb, ntfn_obj);
            }
            ObjType::Endpoint => {}
            _ => return Err(KError::WrongObjectType),
        }
        let ep_obj = self.require_cap_type(cap, ObjType::Endpoint)?;
        let reply_cap = match reply {
            Some(r) => Some(self.cap_at(r.cnode, r.slot)?),
            None => None,
        };
        let reply_obj = match reply_cap {
            Some(cap) => Some(self.require_cap_type(cap, ObjType::Reply)?),
            None => None,
        };
        if let Some(rep) = reply_obj {
            if self.reply_caller(rep)?.is_some() {
                return Err(KError::Busy);
            }
        }
        self.reset_ipc_error(tcb)?;
        // **KT-63**: kiểm receive window **trước** khi pop (message có cap mà window chưa bind
        // ⇒ `InvalidArgument`, message **ở lại** hàng — không mất, không giao nửa chừng).
        let plan = match self.endpoint_front_msg(ep_obj)? {
            Some(m) if m.n_snaps > 0 => {
                self.prepare_caps(tcb, &m.snaps[..m.n_snaps as usize], Some(ep_obj))?
            }
            _ => None,
        };
        match self.endpoint_pop(ep_obj)? {
            Some(q) => {
                let _ = self.deliver_queued(tcb, q, reply_obj, plan)?;
            }
            None => {
                self.endpoint_data_mut(ep_obj)?.recv = Some((tcb, reply_cap));
                self.transition_tcb(
                    tcb,
                    NonReadyTarget::BlockedOnRecv {
                        ep: ep_obj,
                        reply: reply_obj,
                    },
                )?;
            }
        }
        self.sched_after_op(true);
        Ok(())
    }

    /// `NBRecv` — Poll: `Busy` khi hàng rỗng; TCB không chạy được ⇒ `Busy`.
    pub fn nb_recv(&mut self, tcb: ObjId, ep: SlotRef) -> Result<Option<Message>, KError> {
        let state = self.ensure_started(tcb)?;
        if state != TcbState::Ready && !matches!(state, TcbState::Running { .. }) {
            return Err(KError::Busy);
        }
        let cap = self.ipc_slot_cap(ep)?;
        if self.cap(cap)?.ty == ObjType::Notification {
            let ntfn_obj = self.require_cap_type(cap, ObjType::Notification)?;
            self.reset_ipc_error(tcb)?;
            let Some(v) = self.notification_poll(ntfn_obj)? else {
                return Err(KError::Busy);
            };
            self.deliver_notification(tcb, v.word, v.badge)?;
            return Ok(Some(notification_message(v)));
        }
        let ep_obj = self.require_cap_type(cap, ObjType::Endpoint)?;
        self.reset_ipc_error(tcb)?;
        let plan = match self.endpoint_front_msg(ep_obj)? {
            Some(m) if m.n_snaps > 0 => {
                self.prepare_caps(tcb, &m.snaps[..m.n_snaps as usize], Some(ep_obj))?
            }
            _ => None,
        };
        match self.endpoint_pop(ep_obj)? {
            Some(q) => {
                let got = self.deliver_queued(tcb, q, None, plan)?;
                self.sched_after_op(true);
                Ok(got)
            }
            None => Err(KError::Busy),
        }
    }

    /// `Send` trên Reply — message rỗng (API K1b).
    pub fn reply(&mut self, tcb: ObjId, reply: SlotRef) -> Result<(), KError> {
        self.reply_with(tcb, reply, None)
    }

    /// `Send` trên Reply với message đầy đủ.
    pub fn reply_msg(&mut self, tcb: ObjId, reply: SlotRef, msg: Message) -> Result<(), KError> {
        self.reply_with(tcb, reply, Some(msg))
    }

    fn reply_with(
        &mut self,
        tcb: ObjId,
        reply: SlotRef,
        msg: Option<Message>,
    ) -> Result<(), KError> {
        let state = self.tcb_state(tcb)?;
        if state != TcbState::Ready && !matches!(state, TcbState::Running { .. }) {
            return Err(KError::IllegalOperation);
        }
        let reply_obj = self.require_type(reply, ObjType::Reply)?;
        // **KT-63** (như sim): kiểm biên cap + cap nguồn + receive window của **caller**
        // **trước** khi lấy `caller` — lỗi cứng không được để `Reply.caller = None` trong khi
        // caller vẫn `BlockedOnReply` (mất đường reply = treo).
        let caller_peek = self.reply_data(reply_obj)?.caller;
        let (mut msg, plan) = match msg {
            Some(mut m) => {
                if m.n_caps as usize > MAX_CAPS || m.n_caps as usize != m.info.caps() as usize {
                    return Err(KError::InvalidArgument);
                }
                self.resolve_caps(&mut m)?;
                let plan = match caller_peek {
                    Some(c) => self.prepare_caps(c, &m.snaps[..m.n_snaps as usize], None)?,
                    None => None,
                };
                (Some(m), plan)
            }
            None => (None, None),
        };
        let caller = self.reply_data_mut(reply_obj)?.caller.take();
        if let Some(donor) = self.donation_donor_of(tcb) {
            if caller == Some(donor) {
                self.return_donation(tcb)?;
            }
        }
        if let Some(m) = msg.as_mut() {
            let mut m = self.admit_message(*m)?;
            match caller {
                Some(c) => self.deliver_message(c, &mut m, plan.as_ref())?,
                None => self.ipc_dropped += 1,
            }
        }
        if let Some(caller) = caller {
            self.wake_from(caller)?;
        }
        if matches!(state, TcbState::Running { .. }) {
            self.yield_to_ready(tcb)?;
        }
        self.tcb_data_mut(tcb)?.last_fault = None;
        self.sched_after_op(true);
        Ok(())
    }

    /// `ReplyRecv` — đường nóng server: reply **trước**, rồi `Recv` với Reply **mới**.
    pub fn reply_recv(
        &mut self,
        tcb: ObjId,
        reply: SlotRef,
        ep: SlotRef,
        msg: Message,
        recv_reply: Option<SlotRef>,
    ) -> Result<(), KError> {
        let _ = self.require_type(reply, ObjType::Reply)?;
        let _ = self.require_type(ep, ObjType::Endpoint)?;
        if let Some(r) = recv_reply {
            let rep = self.require_type(r, ObjType::Reply)?;
            let dang = self.require_type(reply, ObjType::Reply)?;
            if rep != dang && self.reply_caller(rep)?.is_some() {
                return Err(KError::Busy);
            }
        }
        self.reply_with(tcb, reply, Some(msg))?;
        self.recv(tcb, ep, recv_reply)
    }

    /// `NBSendRecv` (D2-53 Q15a) — NBSend rồi Recv; lỗi NBSend trả dạng `Ok(Some(err))`.
    pub fn nb_send_recv(
        &mut self,
        tcb: ObjId,
        send_ep: SlotRef,
        msg: Message,
        recv_ep: SlotRef,
        recv_reply: Option<SlotRef>,
    ) -> Result<Option<KError>, KError> {
        let _ = self.require_type(recv_ep, ObjType::Endpoint)?;
        if let Some(r) = recv_reply {
            let rep = self.require_type(r, ObjType::Reply)?;
            if self.reply_caller(rep)?.is_some() {
                return Err(KError::Busy);
            }
        }
        let send_err = self.nb_send(tcb, send_ep, msg).err();
        match self.recv(tcb, recv_ep, recv_reply) {
            Ok(()) => Ok(send_err),
            Err(e) => Err(e),
        }
    }

    /// `NBSendWait` (D2-53 Q15a) — NBSend rồi `Wait` Notification.
    pub fn nb_send_wait(
        &mut self,
        tcb: ObjId,
        send_ep: SlotRef,
        msg: Message,
        ntfn: SlotRef,
    ) -> Result<Option<KError>, KError> {
        let ntfn_obj = self.require_type(ntfn, ObjType::Notification)?;
        let send_err = self.nb_send(tcb, send_ep, msg).err();
        match self.notification_wait(tcb, ntfn_obj) {
            Ok(()) => Ok(send_err),
            Err(e) => Err(e),
        }
    }

    // ---------------------------------------------------------------- fault / exit

    /// `Exit(code)`: có `fault_ep` ⇒ `BlockedOnSend{call=false, fault=Exit{code}}` (không
    /// Reply); không ⇒ `Inactive` ngay. Hàng chờ cũ bị huỷ trước khi giao fault (D2-25).
    pub fn exit(&mut self, tcb: ObjId, code: u64) -> Result<(), KError> {
        let state = self.tcb_state(tcb)?;
        if !state.is_live() {
            return Err(KError::IllegalOperation);
        }
        let fault_ep = self.tcb_data(tcb)?.fault_ep;
        let fault_ep_obj = match fault_ep {
            Some(cap) => Some(self.require_cap_type(cap, ObjType::Endpoint)?),
            None => None,
        };
        let nguoi_nhan = match fault_ep_obj {
            Some(ep_obj) => self.endpoint_data(ep_obj)?.recv.is_some(),
            None => false,
        };
        if fault_ep_obj.is_some() {
            self.fault_events += 1;
        }
        if let Some(ep_obj) = fault_ep_obj {
            if !nguoi_nhan && (self.endpoint_full(ep_obj)? || !self.ipc_msg_available()) {
                return Err(KError::Busy);
            }
        }
        self.release_donation_of(tcb)?;
        self.tcb_data_mut(tcb)?.exit_code = Some(code);
        match fault_ep_obj {
            Some(ep_obj) if nguoi_nhan => {
                self.unlink_tcb_queues(tcb);
                let waiter = self.endpoint_data_mut(ep_obj)?.recv.take();
                if let Some((w, _)) = waiter {
                    if w != tcb {
                        self.record_last_fault(w, tcb, FaultKind::Exit { code })?;
                        self.wake_from(w)?;
                    }
                }
                if state != TcbState::Inactive {
                    self.transition_tcb(tcb, NonReadyTarget::Inactive)?;
                }
            }
            Some(ep_obj) => {
                self.unlink_tcb_queues(tcb);
                self.endpoint_enqueue(
                    ep_obj,
                    QueuedMsg {
                        msg: None,
                        sender: tcb,
                        call: false,
                        fault: Some(FaultKind::Exit { code }),
                        blocking: true,
                        alive: true,
                    },
                )?;
                self.transition_tcb(
                    tcb,
                    NonReadyTarget::BlockedOnSend {
                        ep: ep_obj,
                        call: false,
                        fault: Some(FaultKind::Exit { code }),
                    },
                )?;
            }
            None => {
                if state == TcbState::Inactive {
                    self.sched_after_op(true);
                    return Ok(());
                }
                self.unlink_tcb_queues(tcb);
                self.transition_tcb(tcb, NonReadyTarget::Inactive)?;
            }
        }
        self.sched_after_op(true);
        Ok(())
    }

    /// `inject_fault(tcb, kind)`: fault ≠ Exit đi **Call + Reply** tới `fault_ep` (KT-26).
    pub fn inject_fault(&mut self, tcb: ObjId, kind: FaultKind) -> Result<(), KError> {
        if kind.is_exit() {
            return Err(KError::InvalidArgument);
        }
        let state = self.tcb_state(tcb)?;
        if !matches!(state, TcbState::Ready | TcbState::Running { .. }) {
            return Err(KError::IllegalOperation);
        }
        let fault_ep = self.tcb_data(tcb)?.fault_ep;
        let fault_ep_cap = fault_ep.ok_or(KError::InvalidArgument)?;
        let ep_obj = self.require_cap_type(fault_ep_cap, ObjType::Endpoint)?;
        let waiter = self.endpoint_data(ep_obj)?.recv;
        let reply_obj = match waiter {
            Some((_, Some(cap))) => Some(self.require_cap_type(cap, ObjType::Reply)?),
            _ => None,
        };
        if waiter.is_none() && (self.endpoint_full(ep_obj)? || !self.ipc_msg_available()) {
            return Err(KError::Busy);
        }
        self.fault_events += 1;
        match waiter {
            Some((w, _)) => {
                self.transition_tcb(
                    tcb,
                    NonReadyTarget::BlockedOnSend {
                        ep: ep_obj,
                        call: true,
                        fault: Some(kind),
                    },
                )?;
                let taken = self.endpoint_data_mut(ep_obj)?.recv.take();
                debug_assert!(matches!(taken, Some((w2, _)) if w2 == w));
                let q = QueuedMsg {
                    msg: None,
                    sender: tcb,
                    call: true,
                    fault: Some(kind),
                    blocking: true,
                    alive: true,
                };
                let _ = self.deliver_queued(w, q, reply_obj, None)?;
                if w != tcb {
                    self.wake_from(w)?;
                }
            }
            None => {
                self.endpoint_enqueue(
                    ep_obj,
                    QueuedMsg {
                        msg: None,
                        sender: tcb,
                        call: true,
                        fault: Some(kind),
                        blocking: true,
                        alive: true,
                    },
                )?;
                self.transition_tcb(
                    tcb,
                    NonReadyTarget::BlockedOnSend {
                        ep: ep_obj,
                        call: true,
                        fault: Some(kind),
                    },
                )?;
            }
        }
        self.sched_after_op(true);
        Ok(())
    }

    /// Gỡ `tcb` khỏi **mọi** hàng chờ: Recv waiter + hàng gửi Endpoint, `Reply.caller`,
    /// waiter Notification, bucket WaitOnAddress. Trả **số hàng** đã gỡ (như sim).
    ///
    /// Gọi **trước** khi bật `suspended`/chuyển trạng thái (bất biến §4.4: TCB nằm trong hàng
    /// của X ⇔ trạng thái mang X **và** `!suspended`).
    pub(crate) fn unlink_tcb_queues(&mut self, tcb: ObjId) -> usize {
        let mut removed = 0usize;
        let mut msg_dropped = 0usize;
        for i in 0..self.n_endpoints {
            if let Some((w, _)) = self.endpoints[i].recv {
                if w == tcb {
                    self.endpoints[i].recv = None;
                    removed += 1;
                }
            }
            let mut keep = [0u16; ENDPOINT_QUEUE_CAP];
            let mut slots = [0u16; ENDPOINT_QUEUE_CAP];
            let mut ns = 0usize;
            for mi in self.endpoints[i].send.iter() {
                slots[ns] = mi;
                ns += 1;
            }
            let mut nk = 0usize;
            for &mi in &slots[..ns] {
                let q = self.ipc_msgs[mi as usize];
                if q.sender == tcb {
                    if q.msg.is_some() {
                        msg_dropped += 1;
                    }
                    self.free_ipc_msg(mi);
                } else {
                    keep[nk] = mi;
                    nk += 1;
                }
            }
            if nk != self.endpoints[i].send.len() {
                self.endpoints[i].send.kept(&keep[..nk]);
                removed += 1;
            }
        }
        for i in 0..self.n_replies {
            if self.replies[i].caller == Some(tcb) {
                self.replies[i].caller = None;
                removed += 1;
            }
        }
        for i in 0..self.n_notifications {
            if self.notifications[i].waiter == Some(tcb) {
                self.notifications[i].waiter = None;
                removed += 1;
            }
        }
        // WaitAddr: gỡ theo **đúng bucket** suy từ trạng thái TCB.
        if let Ok(TcbState::BlockedOnAddress { frame, offset }) = self.tcb_state(tcb) {
            let b = crate::objstate::waitaddr_bucket(frame, offset);
            let mut keep = [0u16; MAX_WAIT_NODES];
            let mut nk = 0usize;
            let mut cur = self.wait_buckets[b];
            while cur != NODE_NONE {
                let node = self.wait_nodes[cur as usize];
                if node.alive && node.tcb == tcb {
                    self.free_wait_node(cur);
                    removed += 1;
                } else {
                    keep[nk] = cur;
                    nk += 1;
                }
                cur = node.next;
            }
            self.wait_buckets[b] = NODE_NONE;
            self.wait_buckets_tail[b] = NODE_NONE;
            for k in 0..nk {
                let prev = if k == 0 { NODE_NONE } else { keep[k - 1] };
                self.wait_nodes[keep[k] as usize].next = NODE_NONE;
                if prev == NODE_NONE {
                    self.wait_buckets[b] = keep[k];
                } else {
                    self.wait_nodes[prev as usize].next = keep[k];
                }
                self.wait_buckets_tail[b] = keep[k];
            }
        }
        self.ipc_dropped += msg_dropped as u64;
        removed
    }

    // ---------------------------------------------------------------- donation KT-21

    fn donation_of(&self, sc: ObjId) -> Result<(Option<ObjId>, Option<ObjId>), KError> {
        let s = self.sc_data(sc)?;
        Ok((s.donated_to, s.donor))
    }

    pub(crate) fn is_donation_donor(&self, tcb: ObjId) -> bool {
        for i in 0..self.n_objects {
            let (alive, ty, body) = {
                let m = &self.objects[i];
                (m.alive, m.ty, m.body)
            };
            if !alive || ty != ObjType::SchedContext || body.kind != crate::machine::BODY_SC {
                continue;
            }
            if self.scs[body.idx as usize].donor == Some(tcb) {
                return true;
            }
        }
        false
    }

    fn donation_donor_of(&self, donee: ObjId) -> Option<ObjId> {
        let sc = self.tcb_data(donee).ok()?.donated_sc?;
        self.donation_of(sc).ok()?.1
    }

    /// Đặt donation tự động `donor → donee`; trả `false` (không đổi gì) khi không đủ điều
    /// kiện (Call vẫn thành công, D2-50).
    fn donate_sc(&mut self, donor: ObjId, donee: ObjId) -> Result<bool, KError> {
        if donor == donee {
            return Ok(false);
        }
        let dinfo = self.tcb_data(donor)?.snapshot();
        if dinfo.donated_sc.is_some() {
            return Ok(false);
        }
        let Some(sc) = dinfo.sched_context else {
            return Ok(false);
        };
        let configured = match self.sc_data(sc) {
            Ok(s) => s.budget.is_configured(),
            Err(_) => return Ok(false),
        };
        if !configured {
            return Ok(false);
        }
        let mut chu_so_huu = 0usize;
        for i in 0..self.n_objects {
            let (alive, ty, body) = {
                let m = &self.objects[i];
                (m.alive, m.ty, m.body)
            };
            if !alive || ty != ObjType::Tcb || body.kind != crate::machine::BODY_TCB {
                continue;
            }
            if self.tcbs[body.idx as usize].sched_context == Some(sc) {
                chu_so_huu += 1;
            }
        }
        if chu_so_huu != 1 {
            return Ok(false);
        }
        let donee_info = self.tcb_data(donee)?.snapshot();
        if donee_info.sched_context.is_some() || donee_info.donated_sc.is_some() {
            return Ok(false);
        }
        if self.donation_of(sc)?.0.is_some() {
            return Ok(false);
        }
        {
            let s = self.sc_data_mut(sc)?;
            s.donated_to = Some(donee);
            s.donor = Some(donor);
        }
        self.tcb_data_mut(donee)?.donated_sc = Some(sc);
        self.sched_remove_all(donee);
        self.sched_sync(donee)?;
        Ok(true)
    }

    /// Trả SC donation của `donee` về donor (KT-21 DoD 2).
    fn return_donation(&mut self, donee: ObjId) -> Result<Option<ObjId>, KError> {
        let Some(sc) = self.tcb_data(donee)?.donated_sc else {
            return Ok(None);
        };
        self.sched_remove_all(donee);
        self.tcb_data_mut(donee)?.donated_sc = None;
        {
            let s = self.sc_data_mut(sc)?;
            s.donated_to = None;
            s.donor = None;
        }
        self.sched_sync(donee)?;
        Ok(Some(sc))
    }

    /// Huỷ reply đang chờ của `donor`: gỡ `Reply.caller` rồi đánh thức về `Ready`.
    fn cancel_reply_of(&mut self, donor: ObjId) -> Result<(), KError> {
        if let TcbState::BlockedOnReply { reply } = self.tcb_state(donor)? {
            self.reply_set_caller(reply, None)?;
            self.wake_from(donor)?;
        }
        Ok(())
    }

    /// Huỷ donation khi `tcb` là **donee** hoặc **donor** (suspend/exit/finalise — KT-21
    /// DoD 5). Không rò SC: sau hàm này không còn `donated_to`/`donor`/`donated_sc` trỏ tới
    /// `tcb`.
    pub(crate) fn release_donation_of(&mut self, tcb: ObjId) -> Result<(), KError> {
        if let Some(sc) = self.tcb_data(tcb)?.donated_sc {
            let (_, donor) = self.donation_of(sc)?;
            self.return_donation(tcb)?;
            if let Some(donor) = donor {
                self.cancel_reply_of(donor)?;
            }
        }
        loop {
            let mut found: Option<(ObjId, ObjId)> = None;
            for i in 0..self.n_objects {
                let (alive, ty, body) = {
                    let m = &self.objects[i];
                    (m.alive, m.ty, m.body)
                };
                if !alive || ty != ObjType::SchedContext || body.kind != crate::machine::BODY_SC {
                    continue;
                }
                let s = &self.scs[body.idx as usize];
                if s.donor == Some(tcb) {
                    if let Some(donee) = s.donated_to {
                        found = Some(((i as u32) + 1, donee));
                        break;
                    }
                }
            }
            let Some((sc, donee)) = found else { break };
            self.sched_remove_all(donee);
            self.tcb_data_mut(donee)?.donated_sc = None;
            {
                let s = self.sc_data_mut(sc)?;
                s.donated_to = None;
                s.donor = None;
            }
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

/// Message của `NBRecv` trên Notification: `MR0 = word`, `badge = badge`, `len = 1`.
fn notification_message(v: NotificationValue) -> Message {
    let info = crate::ipc::MessageInfo::new(0, 0, false, 0, 1).expect("hằng số hợp lệ");
    let mut msg = Message::new(info, &[v.word]).expect("Message 1 MR hợp lệ");
    msg.badge = v.badge;
    msg
}

impl Machine {
    /// Số message đã nhận vào đường IPC (kernel cấp `msg_id`).
    pub fn ipc_sent(&self) -> u64 {
        self.ipc_sent
    }

    /// Số message đã giao vào vùng nhận TCB.
    pub fn ipc_delivered(&self) -> u64 {
        self.ipc_delivered
    }

    /// Số message bị huỷ (TCB chết/treo hoặc Reply mất người nhận).
    pub fn ipc_dropped(&self) -> u64 {
        self.ipc_dropped
    }

    /// Số lần `ipc::transfer` (luôn bằng `ipc_delivered`).
    pub fn transfer_calls(&self) -> u64 {
        self.transfer_calls
    }

    /// Số message người dùng đang nằm trong các hàng gửi Endpoint.
    pub fn ipc_pending(&self) -> u64 {
        let mut n = 0u64;
        for e in &self.endpoints[..self.n_endpoints] {
            for mi in e.send.iter() {
                if self.ipc_msgs[mi as usize].msg.is_some() {
                    n += 1;
                }
            }
        }
        n
    }

    /// **KT-63** — số cap **cài được** vào receive window qua đường IPC (`RIGHT_GRANT` đã bỏ).
    pub fn cap_transfer_installed(&self) -> u64 {
        self.cap_transfer_installed
    }

    /// **KT-63** — số cap **thất bại mềm** khi chuyển (thiếu `RIGHT_GRANT`, ô đích đã có cap,
    /// cap nguồn chết giữa hàng và giao).
    pub fn cap_transfer_failed(&self) -> u64 {
        self.cap_transfer_failed
    }

    /// **KT-63** — số cap **unwrap** thành badge (cap trỏ chính Endpoint đang gọi + badge).
    pub fn cap_unwrapped(&self) -> u64 {
        self.cap_unwrapped
    }

    /// **KT-63** — `(cài được, unwrap, thất bại mềm)`, cùng bộ ba `cap_transfer_stats` của sim.
    pub fn cap_transfer_stats(&self) -> (u64, u64, u64) {
        (
            self.cap_transfer_installed,
            self.cap_unwrapped,
            self.cap_transfer_failed,
        )
    }

    /// Số lần fault đi vào đường fault (cửa sổ nhanh của `check_fault`).
    pub fn fault_events(&self) -> u64 {
        self.fault_events
    }

    /// Hàng gửi Endpoint — thứ tự FIFO, ảnh chụp đọc-only.
    pub fn endpoint_queued(&self, ep: ObjId) -> Result<QueuedIter<'_>, KError> {
        let i = self.body_slot(ep, ObjType::Endpoint, BODY_ENDPOINT)?;
        let mut slots = [0u16; ENDPOINT_QUEUE_CAP];
        let mut n = 0usize;
        for mi in self.endpoints[i].send.iter() {
            slots[n] = mi;
            n += 1;
        }
        Ok(QueuedIter {
            m: self,
            slots,
            n_slots: n,
            idx: 0,
        })
    }
}

impl Machine {
    /// Waiter `Recv` của Endpoint (`None` nếu không có).
    pub fn endpoint_recv_waiter(&self, ep: ObjId) -> Result<Option<ObjId>, KError> {
        Ok(self.endpoint_data(ep)?.recv.map(|(t, _)| t))
    }

    /// Ảnh chụp hộp thư Endpoint (model API `[TK]`).
    pub fn endpoint_info(&self, ep: ObjId) -> Result<EndpointInfo, KError> {
        let e = self.endpoint_data(ep)?;
        Ok(EndpointInfo {
            capacity: e.send.capacity(),
            len: e.send.len(),
            pushed: e.send.pushed,
            popped: e.send.popped,
            removed: e.send.removed,
            recv_waiter: e.recv.map(|(t, _)| t),
        })
    }

    /// Số message đang chờ của Endpoint.
    pub fn endpoint_send_len(&self, ep: ObjId) -> Result<usize, KError> {
        Ok(self.endpoint_data(ep)?.send.len())
    }

    /// MR0 của message gần nhất vào Endpoint.
    pub fn endpoint_last_mr(&self, ep: ObjId) -> Result<Option<u64>, KError> {
        Ok(self.endpoint_data(ep)?.last_mr)
    }

    /// `msg_id` các message người dùng đang chờ (FIFO; bỏ fault).
    pub fn endpoint_msg_ids(&self, ep: ObjId) -> Result<MsgIds<'_>, KError> {
        let i = self.body_slot(ep, ObjType::Endpoint, BODY_ENDPOINT)?;
        Ok(MsgIds {
            m: self,
            ep: i,
            idx: 0,
        })
    }

    /// `(sender, fault)` các mục trong hàng gửi Endpoint (FIFO).
    pub fn endpoint_send_items(&self, ep: ObjId) -> Result<SendItems<'_>, KError> {
        let i = self.body_slot(ep, ObjType::Endpoint, BODY_ENDPOINT)?;
        Ok(SendItems {
            m: self,
            ep: i,
            idx: 0,
        })
    }
}

/// Iterator ảnh chụp message đang chờ của một Endpoint (FIFO).
#[derive(Debug, Clone)]
pub struct QueuedIter<'a> {
    m: &'a Machine,
    slots: [u16; ENDPOINT_QUEUE_CAP],
    n_slots: usize,
    idx: usize,
}

impl Iterator for QueuedIter<'_> {
    type Item = QueuedView;

    fn next(&mut self) -> Option<QueuedView> {
        while self.idx < self.n_slots {
            let i = self.idx;
            self.idx += 1;
            let q = &self.m.ipc_msgs[self.slots[i] as usize];
            if !q.alive {
                continue;
            }
            return Some(QueuedView {
                sender: q.sender,
                msg: q.msg,
                call: q.call,
                fault: q.fault,
                blocking: q.blocking,
            });
        }
        None
    }
}

/// Iterator `msg_id` message người dùng trong hàng gửi Endpoint.
#[derive(Debug, Clone)]
pub struct MsgIds<'a> {
    m: &'a Machine,
    ep: usize,
    idx: usize,
}

impl Iterator for MsgIds<'_> {
    type Item = u64;

    fn next(&mut self) -> Option<u64> {
        let mut slots = [0u16; ENDPOINT_QUEUE_CAP];
        let mut n = 0usize;
        for mi in self.m.endpoints[self.ep].send.iter() {
            slots[n] = mi;
            n += 1;
        }
        while self.idx < n {
            let q = &self.m.ipc_msgs[slots[self.idx] as usize];
            self.idx += 1;
            if let Some(msg) = q.msg {
                return Some(msg.msg_id);
            }
        }
        None
    }
}

/// Iterator `(sender, fault)` trong hàng gửi Endpoint.
#[derive(Debug, Clone)]
pub struct SendItems<'a> {
    m: &'a Machine,
    ep: usize,
    idx: usize,
}

impl Iterator for SendItems<'_> {
    type Item = (ObjId, Option<FaultKind>);

    fn next(&mut self) -> Option<(ObjId, Option<FaultKind>)> {
        let mut slots = [0u16; ENDPOINT_QUEUE_CAP];
        let mut n = 0usize;
        for mi in self.m.endpoints[self.ep].send.iter() {
            slots[n] = mi;
            n += 1;
        }
        if self.idx >= n {
            return None;
        }
        let q = &self.m.ipc_msgs[slots[self.idx] as usize];
        self.idx += 1;
        Some((q.sender, q.fault))
    }
}

impl Machine {
    /// **KT-63** — bind **receive window** (CPTR 4 ô CSpace) cho TCB — model API `[TK]` như
    /// sim, **không** thêm chữ ký ABI (`Tcb.Configure` của `spec/abi.toml` chỉ có CSpace).
    ///
    /// Mỗi ô phải là slot **có thật** của một CNode còn sống; các ô phải **khác nhau** (hai cap
    /// cài vào cùng một slot sẽ đè nhau ⇒ `InvalidArgument`). Bind `[s0..s3]` với các ô **rỗng**
    /// là hợp lệ — khác "chưa bind" (`None`: message có cap cần cài ⇒ `InvalidArgument`).
    pub fn tcb_set_recv_window(
        &mut self,
        tcb: SlotRef,
        window: [SlotRef; MAX_CAPS],
    ) -> Result<(), KError> {
        let tcb_obj = self.require_type(tcb, ObjType::Tcb)?;
        self.set_recv_window_for(tcb_obj, window)
    }

    /// **KT-67b** — như [`Machine::tcb_set_recv_window`] nhưng nhận **`ObjId`** của TCB, dùng cho
    /// đường syscall: `build_message` lấy receive window từ **header IPC buffer** của chính TCB
    /// gọi (D2-98) — core không có (và không cần) một slot cap TCB của caller.
    pub fn tcb_set_recv_window_obj(
        &mut self,
        tcb: ObjId,
        window: [SlotRef; MAX_CAPS],
    ) -> Result<(), KError> {
        // `tcb_data` kiểm đúng loại TCB (cùng ngữ nghĩa `require_type`).
        let _ = self.tcb_data(tcb)?;
        self.set_recv_window_for(tcb, window)
    }

    /// Phần chung: kiểm 4 ô window (CNode còn sống, ô khác nhau) rồi gán.
    fn set_recv_window_for(
        &mut self,
        tcb_obj: ObjId,
        window: [SlotRef; MAX_CAPS],
    ) -> Result<(), KError> {
        for (i, w) in window.iter().enumerate() {
            self.check_cnode_slot(w.cnode, w.slot)?;
            if window[..i].contains(w) {
                return Err(KError::InvalidArgument);
            }
        }
        self.tcb_data_mut(tcb_obj)?.recv_window = Some(window);
        self.sched_after_op(false);
        Ok(())
    }

    /// **KT-63** — receive window hiện tại của TCB (`None` = chưa bind).
    pub fn tcb_recv_window(&self, tcb: ObjId) -> Result<Option<[SlotRef; MAX_CAPS]>, KError> {
        Ok(self.tcb_data(tcb)?.recv_window)
    }
}

/// `IpcArea` của message gần nhất — bí danh tiện cho parity.
pub type TcbIpcArea = IpcArea;
