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
use crate::machine::{Machine, BODY_ENDPOINT, BODY_NOTIFICATION, BODY_REPLY};
use crate::objstate::{EndpointData, NotificationData, QueuedMsg, ReplyData, ENDPOINT_QUEUE_CAP};
use crate::observer::CoreObserver;
use crate::observer::TestObserver;
use crate::store::KernelStore;
use crate::tcb::{FaultKind, TcbState};
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

impl<S: KernelStore, O: CoreObserver> Machine<S, O> {
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
        self.n_ipc_msg_free > 0 || self.n_ipc_msgs < self.ipc_msgs.len()
    }

    fn alloc_ipc_msg(&mut self, q: QueuedMsg) -> Option<u16> {
        let idx = if self.n_ipc_msg_free > 0 {
            self.n_ipc_msg_free -= 1;
            self.ipc_msg_free[self.n_ipc_msg_free] as usize
        } else if self.n_ipc_msgs < self.ipc_msgs.len() {
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
        if (idx as usize) < self.ipc_msgs.len() {
            self.ipc_msgs[idx as usize] = QueuedMsg::EMPTY;
            if self.n_ipc_msg_free < self.ipc_msg_free.len() {
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
        let Some(mi) = self.endpoints[i].send.iter(&self.ipc_msgs).next() else {
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
        match self.endpoints[i].send.try_push(mi, &mut self.ipc_msgs) {
            Ok(()) => Ok(()),
            Err(mi) => {
                self.free_ipc_msg(mi);
                Err(KError::Busy)
            }
        }
    }

    fn endpoint_pop(&mut self, ep: ObjId) -> Result<Option<QueuedMsg>, KError> {
        let i = self.body_slot(ep, ObjType::Endpoint, BODY_ENDPOINT)?;
        let Some(mi) = self.endpoints[i].send.pop_front(&self.ipc_msgs) else {
            return Ok(None);
        };
        let q = self.ipc_msgs[mi as usize];
        self.free_ipc_msg(mi);
        Ok(Some(q))
    }

    fn set_endpoint_last_mr(&mut self, ep: ObjId, mr: u64) -> Result<(), KError> {
        self.endpoint_data_mut(ep)?.model.last_mr = Some(mr);
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
}

mod queue;
mod recv;
mod reply;
mod send;

pub use queue::{MsgIds, QueuedIter, SendItems};

/// `IpcArea` của message gần nhất — bí danh tiện cho parity.
pub type TcbIpcArea = IpcArea;
