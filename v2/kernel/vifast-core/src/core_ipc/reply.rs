// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `core_ipc/reply.rs` — `Reply.caller` + **fault/exit** + **donation KT-21** của
//! [`Machine`] (KT-60): `reply_caller`/`reply_set_caller`, `exit`, `inject_fault`,
//! `unlink_tcb_queues*`, `donate_sc`/`return_donation`/`release_donation_of`; port từ oracle.

use super::*;

use crate::objstate::NODE_NONE;
use crate::store::KernelStore;
use crate::tcb::SleepReason;

impl<S: KernelStore, O: CoreObserver> Machine<S, O> {
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
                        next: crate::objstate::QUEUE_NONE,
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
                    next: crate::objstate::QUEUE_NONE,
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
                        next: crate::objstate::QUEUE_NONE,
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
    ///
    /// # KT-342b + KT-401 — một luật duy nhất cho **mọi** đường
    ///
    /// Message `NBSend` **đã trả `Ok`** (`!QueuedMsg.blocking`) **thuộc về bên nhận**: nó không
    /// còn là "hàng chờ của TCB gửi" và **không** bị rút lại khi TCB gửi đổi trạng thái
    /// (`Suspend`/`block_current`/`wake_step`) hay chết (`exit`/`finalise`). Bất biến §4.4 chỉ
    /// nói về message **chặn** — `Send`/`Call`/fault đang giữ TCB ở `BlockedOnSend` — nên gỡ
    /// chúng là đúng, còn giữ message đã cam kết **không** vi phạm bất biến nào.
    ///
    /// Trước KT-401 luật này chỉ áp cho đường **chết**; đường chuyển trạng thái vẫn dùng bản
    /// "gỡ mọi message" và làm **mất** message `NBSend` đã `Ok` (đỏ-trước ở
    /// `nb_queue_tests::nb_send_song_sot_qua_suspend_cua_sender` /
    /// `..._qua_block_cua_sender`). Nay **một** cửa: không còn biến thể "gỡ tất".
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
            for mi in self.endpoints[i].send.iter(&self.ipc_msgs) {
                slots[ns] = mi;
                ns += 1;
            }
            let mut nk = 0usize;
            for &mi in &slots[..ns] {
                let q = self.ipc_msgs[mi as usize];
                // **KT-342b/KT-401**: chỉ message **chặn** mới là hàng chờ của TCB gửi; message
                // `NBSend` đã trả `Ok` ở lại hàng của người nhận.
                if q.sender == tcb && q.blocking {
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
                self.endpoints[i].send.kept(&keep[..nk], &mut self.ipc_msgs);
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
            let mut keep = [0u16; crate::store::GENERAL_MAX_WAIT_NODES];
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

    pub(super) fn donation_donor_of(&self, donee: ObjId) -> Option<ObjId> {
        let sc = self.tcb_data(donee).ok()?.donated_sc?;
        self.donation_of(sc).ok()?.1
    }

    /// Đặt donation tự động `donor → donee`; trả `false` (không đổi gì) khi không đủ điều
    /// kiện (Call vẫn thành công, D2-50).
    pub(super) fn donate_sc(&mut self, donor: ObjId, donee: ObjId) -> Result<bool, KError> {
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
    pub(super) fn return_donation(&mut self, donee: ObjId) -> Result<Option<ObjId>, KError> {
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
