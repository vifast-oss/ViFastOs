// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `core_ipc/send.rs` — `Call`/`Send` của [`Machine`] (KT-60): `call_msg`, `call`,
//! `send`, `nb_send` và đường giao message `endpoint_send`; port từ `vifast-sim` (oracle).

use super::*;

impl<O: CoreObserver> Machine<O> {
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
                    next: crate::objstate::QUEUE_NONE,
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
        // **KT-342b — đột biến (ii)** `mut-state-before-cap` (ca 4 đỏ: trả 7 thay vì 1).
        #[cfg(feature = "mut-state-before-cap")]
        {
            let state = self.ensure_started(tcb)?;
            if state != TcbState::Ready && !matches!(state, TcbState::Running { .. }) {
                return Err(KError::IllegalOperation);
            }
        }
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
            // **KT-342b — đột biến (i)** `mut-drop-nb-no-waiter` (ca 1 + 2 đỏ).
            #[cfg(feature = "mut-drop-nb-no-waiter")]
            if !blocking {
                self.sched_after_op(true);
                return Ok(());
            }
            self.endpoint_enqueue(
                ep_obj,
                QueuedMsg {
                    msg: Some(msg),
                    sender: tcb,
                    call: false,
                    fault: None,
                    blocking,
                    alive: true,
                    next: crate::objstate::QUEUE_NONE,
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
}
