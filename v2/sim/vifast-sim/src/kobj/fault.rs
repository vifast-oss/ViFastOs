// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-336b** — miền `fault`: fault (Exit/Inject) + kiểm fault.
//!
//! Tách nguyên văn từ `kobj.rs` (D2-232/D2-238): **chỉ đổi vị trí** + `pub(super)` cho hàm
//! nội bộ cần gọi chéo module; thân hàm, thứ tự op và API công khai giữ nguyên. Đây là
//! module con của [`crate::kobj`] nên thấy item private của `kobj` (cùng mẫu `state_write`).

use super::*;

impl Machine {
    // ------------------------------------------- KT-41: bất biến "fault không mất"

    /// **KT-41** — bất biến **fault không mất/không nhân bản** (chạy sau **mỗi** public op qua
    /// [`Machine::sched_after_op`], cùng chỗ với `check_ipc`/`check_reply_links`):
    ///
    /// 1. Mỗi message **fault** trong hàng gửi của Endpoint phải khớp **đúng** trạng thái người
    ///    gửi: `BlockedOnSend{ep, call, fault: Some(kind)}` cùng `ep`/`call`/`kind`; `Exit` là
    ///    Send không Reply (`call = false`), fault khác là Call (`call = true`, D2-25). Lệch ⇒
    ///    fault đã bị mất/ghi đè giữa hàng và trạng thái.
    /// 2. Mỗi TCB `BlockedOnSend` mang fault phải có **đúng một** message fault khớp
    ///    `(sender, kind)` trong hàng của `ep` — đây là chốt của C2/D2-63 cho đường fault: Reply
    ///    đang giữ caller không được làm fault thứ hai biến mất.
    ///
    /// Cửa sổ nhanh `fault_events == 0` giữ chi phí per-op của mọi test K1a..K1c/KT-36..40 như
    /// trước KT-41. `Vm` là **model-only** nên bất biến kiểm cả `Exit`/`UnknownSyscall`/… (một
    /// cửa fault duy nhất — không có đường riêng cho `Vm`).
    pub fn check_fault(&self) -> Result<(), String> {
        if self.fault_events == 0 {
            return Ok(());
        }
        // 1. Hàng gửi của Endpoint ⇔ trạng thái sender.
        for (i, meta) in self.objects.iter().enumerate() {
            if !meta.alive || meta.ty != ObjType::Endpoint {
                continue;
            }
            let ep = (i + 1) as ObjId;
            let hang: Vec<(ObjId, bool, Option<FaultKind>)> = self
                .body(ep, |k| match k {
                    Kind::Endpoint(e) => {
                        Ok(e.send.iter().map(|q| (q.sender, q.call, q.fault)).collect())
                    }
                    _ => Err(KError::WrongObjectType),
                })
                .map_err(|e| format!("Endpoint {ep}: không đọc được hàng gửi: {e:?}"))?;
            for (sender, call, fault) in hang {
                let Some(kind) = fault else { continue };
                let dung_call = !kind.is_exit();
                if call != dung_call {
                    return Err(format!(
                        "Endpoint {ep}: fault {kind:?} của TCB {sender} có call={call} (luật D2-25: Exit=false, fault khác=true)"
                    ));
                }
                let st = self
                    .tcb_state(sender)
                    .map_err(|e| format!("Endpoint {ep}: sender {sender} không đọc được: {e:?}"))?;
                match st {
                    TcbState::BlockedOnSend {
                        ep: ep2,
                        call: call2,
                        fault: Some(kind2),
                    } if ep2 == ep && call2 == call && kind2 == kind => {}
                    other => {
                        return Err(format!(
                            "Endpoint {ep}: fault {kind:?} của TCB {sender} nằm trong hàng nhưng trạng thái là {other:?} (fault có thể bị mất/ghi đè)"
                        ));
                    }
                }
            }
        }
        // 2. TCB `BlockedOnSend` mang fault ⇔ **đúng một** message fault trong hàng.
        for (i, meta) in self.objects.iter().enumerate() {
            if !meta.alive || meta.ty != ObjType::Tcb {
                continue;
            }
            let tcb = (i + 1) as ObjId;
            let st = self
                .tcb_state(tcb)
                .map_err(|e| format!("TCB {tcb}: không đọc được trạng thái: {e:?}"))?;
            let TcbState::BlockedOnSend {
                ep,
                call,
                fault: Some(kind),
            } = st
            else {
                continue;
            };
            if !self.object_alive(ep) {
                return Err(format!(
                    "TCB {tcb}: BlockedOnSend mang fault {kind:?} trên Endpoint {ep} đã chết"
                ));
            }
            let mut n = 0usize;
            self.body(ep, |k| match k {
                Kind::Endpoint(e) => {
                    n = e
                        .send
                        .iter()
                        .filter(|q| q.sender == tcb && q.call == call && q.fault == Some(kind))
                        .count();
                    Ok(())
                }
                _ => Err(KError::WrongObjectType),
            })
            .map_err(|e| format!("TCB {tcb}: Endpoint {ep} không đọc được: {e:?}"))?;
            if n != 1 {
                return Err(format!(
                    "TCB {tcb}: BlockedOnSend mang fault {kind:?} nhưng có {n} message fault trong hàng Endpoint {ep} (phải đúng 1 — fault mất/nhân bản)"
                ));
            }
        }
        Ok(())
    }

    /// **KT-26** — `(sender, FaultKind)` của **fault vừa giao** cho TCB làm bên nhận (procsrv);
    /// `None` nếu lần nhận gần nhất không phải fault. Model-only `[TK]`: message fault không có
    /// nội dung MR nên procsrv đọc sender + loại fault qua đây.
    pub fn tcb_last_fault(&self, tcb: ObjId) -> Result<Option<(ObjId, FaultKind)>, KError> {
        self.body(tcb, |k| match k {
            Kind::Tcb(t) => Ok(t.last_fault),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// **KT-26** — như [`Machine::tcb_last_fault`] nhưng **tiêu thụ** bản ghi (đặt `None`).
    /// `ProcsrvModel::recv_exit` dùng để không xử lý hai lần cùng một fault.
    pub fn take_last_fault(&mut self, tcb: ObjId) -> Result<Option<(ObjId, FaultKind)>, KError> {
        self.body_mut(tcb, |k| match k {
            Kind::Tcb(t) => Ok(t.last_fault.take()),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// Ghi `(sender, fault)` vừa giao cho **bên nhận** (procsrv) — model-only `[TK]`, KT-26.
    pub(super) fn record_last_fault(
        &mut self,
        receiver: ObjId,
        sender: ObjId,
        kind: FaultKind,
    ) -> Result<(), KError> {
        self.body_mut(receiver, |k| match k {
            Kind::Tcb(t) => {
                t.last_fault = Some((sender, kind));
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })
    }

    /// `Exit(code)`: có `fault_ep` ⇒ `BlockedOnSend{call=false, fault=Exit{code}}`; không ⇒
    /// `Inactive` ngay; TCB đang `Inactive` + không `fault_ep` thì **không** chuyển trạng thái
    /// (đặt `exit_code` thôi). Message/hàng chờ cũ bị huỷ **trước** khi giao fault (D2-25).
    pub fn exit(&mut self, tcb: ObjId, code: u64) -> Result<(), KError> {
        let state = self.tcb_state(tcb)?;
        if !state.is_live() {
            return Err(KError::IllegalOperation);
        }
        // Reviewer-kt18 mục 1: **kiểm `fault_ep` trước** khi ghi `exit_code`/gỡ hàng chờ.
        // Cap `fault_ep` có thể đã bị Delete/Revoke ⇒ `Exit` lỗi mà TCB **không** bị đổi
        // trạng thái/hàng chờ (thiết kế §4.4 invariant; trước sửa: state rác).
        let fault_ep = self.body(tcb, |k| match k {
            Kind::Tcb(t) => Ok(t.fault_ep),
            _ => Err(KError::WrongObjectType),
        })?;
        let fault_ep_obj = match fault_ep {
            Some(cap) => Some(self.require_cap_type(cap, ObjType::Endpoint)?),
            None => None,
        };
        // KT-23 (bug fuzz tìm ra, xem `kt23_k1b` + JOURNAL): nếu `fault_ep` đang có **người nhận
        // chờ** (`Recv` waiter) thì giao fault **trực tiếp** như đường nhanh của `call` — message
        // **không** vào hàng đợi, nên bất biến "có message ⇒ không `Recv` waiter" (KT-22) giữ
        // nguyên. Procsrv đã chờ sẵn ⇒ coi như `Recv` xong tại thời điểm giao: TCB thoát đi
        // `Inactive` ngay (D2-25 nói `Inactive` sau khi procsrv `Recv`) và **không** bị
        // `BlockedOnSend` trên một message không còn trong hàng.
        // Không có waiter ⇒ giữ nguyên D2-25: hàng gửi cố định, đầy ⇒ `Busy` **trước** mọi thay
        // đổi (không ghi `exit_code`, không đổi trạng thái, không trả donation) — message fault
        // không được "mất nửa chừng"; người gọi thử lại sau khi procsrv `Recv` (§4.17).
        let nguoi_nhan = match fault_ep_obj {
            Some(ep_obj) => self.body(ep_obj, |k| match k {
                Kind::Endpoint(e) => Ok(e.recv.is_some()),
                _ => Err(KError::WrongObjectType),
            })?,
            None => false,
        };
        // **KT-41** — cửa sổ nhanh cho `check_fault`: chỉ khi đã từng có fault mới quét.
        if fault_ep_obj.is_some() {
            self.fault_events += 1;
        }
        if let Some(ep_obj) = fault_ep_obj {
            if !nguoi_nhan {
                let con_cho = self.body(ep_obj, |k| match k {
                    Kind::Endpoint(e) => Ok(!e.send.is_full()),
                    _ => Err(KError::WrongObjectType),
                })?;
                if !con_cho {
                    return Err(KError::Busy);
                }
            }
        }
        // KT-21 DoD 5: `Exit` khi đang donate ⇒ trả SC (server chết) / thu hồi SC (client chết)
        // trước khi ghi `exit_code` + đổi trạng thái; không rò SC/không bỏ treo client.
        self.release_donation_of(tcb)?;
        self.body_mut(tcb, |k| match k {
            Kind::Tcb(t) => {
                t.exit_code = Some(code);
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        match fault_ep_obj {
            // Có `Recv` waiter: giao **thẳng** (không vào hàng) — xem chú thích phía trên.
            Some(ep_obj) if nguoi_nhan => {
                self.unlink_tcb_queues_keep_committed(tcb);
                let waiter = self.body_mut(ep_obj, |k| match k {
                    Kind::Endpoint(e) => Ok(e.recv.take()),
                    _ => Err(KError::WrongObjectType),
                })?;
                if let Some((w, _)) = waiter {
                    // Waiter khác TCB đang thoát (procsrv) ⇒ đánh thức; nếu chính nó là waiter
                    // (procsrv tự Exit khi đang chờ `Recv` cùng endpoint) thì đi thẳng `Inactive`.
                    if w != tcb {
                        // **KT-26**: procsrv phải biết `(sender, Exit{code})` vừa nhận.
                        self.record_last_fault(w, tcb, FaultKind::Exit { code })?;
                        self.wake_from(w)?;
                    }
                }
                // KT-23: `Exit` từ `Inactive` giữ `Inactive` (cặp danh tính `(Inactive, Inactive)`
                // bị cấm — cùng quy ước với nhánh không `fault_ep`, D2-25/KT-18).
                if state != TcbState::Inactive {
                    self.transition_tcb(tcb, NonReadyTarget::Inactive)?;
                }
            }
            Some(ep_obj) => {
                self.unlink_tcb_queues_keep_committed(tcb);
                self.body_mut(ep_obj, |k| match k {
                    Kind::Endpoint(e) => {
                        // Đã kiểm chỗ phía trên ⇒ chỉ có thể thành công; map về `Busy` để
                        // không `panic` nếu bất biến đổi (không dùng `expect` trong mô hình).
                        // KT-24: fault `Exit` là **message fault** (không phải IPC message) nên
                        // không có `msg_id`, không tính vào `ipc_sent/delivered/pending`.
                        e.send
                            .try_push(QueuedMessage {
                                msg: None,
                                sender: tcb,
                                call: false,
                                fault: Some(FaultKind::Exit { code }),
                                blocking: true,
                                snaps: Vec::new(),
                            })
                            .map_err(|_| KError::Busy)
                    }
                    _ => Err(KError::WrongObjectType),
                })?;
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
                self.unlink_tcb_queues_keep_committed(tcb);
                self.transition_tcb(tcb, NonReadyTarget::Inactive)?;
            }
        }
        self.sched_after_op(true);
        Ok(())
    }

    /// **KT-26** — `inject_fault(tcb, kind)`: mô phỏng một **trap/fault** xảy ra khi `tcb` đang
    /// chạy (`Ready`/`Running`), theo thiết kế §4.6 `:321`: fault ≠ Exit đi **Call tới
    /// `fault_ep`** (caller chờ `BlockedOnReply`, procsrv trả lời để chạy tiếp); `Exit` **không**
    /// đi đường này — dùng [`Machine::exit`] (D2-25, Send không Reply).
    ///
    /// **Model-only `[TK]`, không phải ABI** (D2-53 Q5): `tcb.fault_ep` **phải** có (KT-26 chỉ mô
    /// hình đường có fault endpoint; fault không handler để K3 chốt — trả `InvalidArgument` và
    /// **không** đổi trạng thái). Cap `fault_ep` chết ⇒ `InvalidCapability` (như `Exit`), hàng
    /// gửi của `fault_ep` đầy và không có `Recv` waiter ⇒ `Busy` — **trước** mọi thay đổi, không
    /// giao nửa chừng.
    ///
    /// Có `Recv` waiter (procsrv đang chờ) ⇒ giao **thẳng** như đường nhanh của `call`/`exit`
    /// (message không vào hàng, bất biến "có message ⇒ không waiter" giữ nguyên); waiter **không**
    /// cấp Reply cap ⇒ caller `Cancelled` (không treo vĩnh viễn — cùng khuôn D2-53 Q7(b)).
    pub fn inject_fault(&mut self, tcb: ObjId, kind: FaultKind) -> Result<(), KError> {
        if kind.is_exit() {
            return Err(KError::InvalidArgument);
        }
        let state = self.tcb_state(tcb)?;
        if !matches!(state, TcbState::Ready | TcbState::Running { .. }) {
            return Err(KError::IllegalOperation);
        }
        let fault_ep = self.body(tcb, |k| match k {
            Kind::Tcb(t) => Ok(t.fault_ep),
            _ => Err(KError::WrongObjectType),
        })?;
        let fault_ep_cap = fault_ep.ok_or(KError::InvalidArgument)?;
        let ep_obj = self.require_cap_type(fault_ep_cap, ObjType::Endpoint)?;
        // Đọc waiter + Reply cap của nó **trước** khi đổi bất cứ thứ gì (lỗi cap cứng không để
        // fault mất nửa chừng).
        let waiter = self.body(ep_obj, |k| match k {
            Kind::Endpoint(e) => Ok(e.recv),
            _ => Err(KError::WrongObjectType),
        })?;
        let reply_obj = match waiter {
            Some((_, Some(cap))) => Some(self.require_cap_type(cap, ObjType::Reply)?),
            _ => None,
        };
        if waiter.is_none() && self.endpoint_full(ep_obj)? {
            return Err(KError::Busy);
        }
        // **KT-41** — cửa sổ nhanh cho `check_fault` (đếm **mọi** fault đi vào đường fault,
        // kể cả `Vm`); đếm trước khi giao để `sched_after_op` bên dưới kiểm được ngay.
        self.fault_events += 1;
        match waiter {
            Some((w, _)) => {
                // Giao thẳng: đưa TCB vào trạng thái chặn **trước** (mọi chuyển trạng thái bên
                // trong `deliver_queued` khi đó nằm trong bảng `ALLOWED`: `3→5` Reply, `3→1`
                // Cancelled), rồi lấy waiter khỏi Endpoint và dùng **cùng** đường
                // `deliver_queued` (một chỗ xử lý fault: `last_fault` + Reply/Cancelled).
                self.transition_tcb(
                    tcb,
                    NonReadyTarget::BlockedOnSend {
                        ep: ep_obj,
                        call: true,
                        fault: Some(kind),
                    },
                )?;
                let taken = self.body_mut(ep_obj, |k| match k {
                    Kind::Endpoint(e) => Ok(e.recv.take()),
                    _ => Err(KError::WrongObjectType),
                })?;
                debug_assert!(matches!(taken, Some((w2, _)) if w2 == w));
                let q = QueuedMessage {
                    msg: None,
                    sender: tcb,
                    call: true,
                    fault: Some(kind),
                    blocking: true,
                    snaps: Vec::new(),
                };
                let _ = self.deliver_queued(w, q, reply_obj, None)?;
                if w != tcb {
                    self.wake_from(w)?;
                }
            }
            None => {
                self.endpoint_enqueue(
                    ep_obj,
                    QueuedMessage {
                        msg: None,
                        sender: tcb,
                        call: true,
                        fault: Some(kind),
                        blocking: true,
                        snaps: Vec::new(),
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
}
