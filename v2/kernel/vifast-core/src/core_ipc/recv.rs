// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `core_ipc/recv.rs` — `Recv`/`Reply` của [`Machine`] (KT-60): `recv`, `nb_recv`,
//! `reply`, `reply_msg`, `reply_with`, `reply_recv`, `nb_send_recv`, `nb_send_wait`; port từ
//! `vifast-sim` (oracle).

use super::*;
use crate::store::KernelStore;

impl<S: KernelStore, O: CoreObserver> Machine<S, O> {
    // ---------------------------------------------------------------- Recv / Reply

    /// `Recv` trên Endpoint hoặc `Wait` trên Notification (spec `:109`).
    pub fn recv(&mut self, tcb: ObjId, ep: SlotRef, reply: Option<SlotRef>) -> Result<(), KError> {
        // **KT-408** — ring trace 32 B **luôn bật** (`D2-09`): một bản ghi `ipc` mỗi lần nhận.
        self.trace_push(crate::trace::TraceKind::Ipc, tcb, 0, 0);
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
        // **KT-309** — kiểm slot đích của extra cap **trước** khi pop: `SlotOccupied` không được
        // làm mất message (cùng lý do `prepare_caps` chạy trước `endpoint_pop`).
        if let Some(m) = self.endpoint_front_msg(ep_obj)? {
            if m.n_extra > 0 {
                let _ = self.extra_caps_slots(tcb, m.n_extra as usize)?;
            }
        }
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
        // **KT-309** — như `recv`: kiểm slot đích extra cap **trước** khi pop.
        if let Some(m) = self.endpoint_front_msg(ep_obj)? {
            if m.n_extra > 0 {
                let _ = self.extra_caps_slots(tcb, m.n_extra as usize)?;
            }
        }
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
                if m.n_caps as usize > self.caps.len()
                    || m.n_caps as usize != m.info.caps() as usize
                {
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
}

/// Message của `NBRecv` trên Notification: `MR0 = word`, `badge = badge`, `len = 1`.
fn notification_message(v: NotificationValue) -> Message {
    let info = crate::ipc::MessageInfo::new(0, 0, false, 0, 1).expect("hằng số hợp lệ");
    let mut msg = Message::new(info, &[v.word]).expect("Message 1 MR hợp lệ");
    msg.badge = v.badge;
    msg
}
