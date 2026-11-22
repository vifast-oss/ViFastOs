// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-336b** — miền `ipc_ops`: IPC: Endpoint/Reply, transfer cap, Send/Recv/Call/Reply.
//!
//! Tách nguyên văn từ `kobj.rs` (D2-232/D2-238): **chỉ đổi vị trí** + `pub(super)` cho hàm
//! nội bộ cần gọi chéo module; thân hàm, thứ tự op và API công khai giữ nguyên. Đây là
//! module con của [`crate::kobj`] nên thấy item private của `kobj` (cùng mẫu `state_write`).

use super::*;

impl Machine {
    /// **KT-24** — kiểm bất biến **bảo toàn message** (K1C-PLAN §1.1 C1) sau mỗi public op:
    ///
    /// 1. mỗi message **người dùng** còn trong mô hình có `msg_id` khác 0, `<= ipc_sent`, và
    ///    **không trùng** (một `msg_id` xuất hiện đúng một nơi: hàng gửi **hoặc** vùng nhận TCB);
    /// 2. message fault `Exit` không có `msg_id` (không phải message IPC) và không có message nào
    ///    vừa không có nội dung vừa không phải fault;
    /// 3. `transfer_calls == ipc_delivered` — **mọi** lần giao đi qua `crate::ipc::transfer`
    ///    (một đường copy MR duy nhất, DoD 3);
    /// 4. `ipc_sent == ipc_delivered + ipc_pending + ipc_dropped`; `ipc_pending` đếm lại từ hàng
    ///    thật (không tin bộ đếm), `ipc_dropped` chỉ tăng khi huỷ message (D2-25).
    pub fn check_ipc(&self) -> Result<(), String> {
        // Đường nhanh cho mô hình **chưa từng** dùng IPC (L1 của KT-08..KT-23, hầu hết unit
        // test object): không có message ⇒ mọi bất biến đều đúng khi 3 bộ đếm còn 0. Giữ
        // `check_ipc` sau **mỗi** public op mà không làm chậm L1 1.000 seed (QD §5.2).
        if self.ipc_sent == 0 {
            if self.ipc_delivered == 0 && self.ipc_dropped == 0 && self.transfer_calls == 0 {
                return Ok(());
            }
            return Err("ipc_sent = 0 nhưng bộ đếm giao/huỷ/transfer khác 0".to_string());
        }
        let mut ids: Vec<u64> = Vec::new();
        let mut pending = 0u64;
        for (i, meta) in self.objects.iter().enumerate() {
            if !meta.alive || meta.ty != ObjType::Endpoint {
                continue;
            }
            let ep = (i + 1) as ObjId;
            let items = self
                .body(ep, |k| match k {
                    Kind::Endpoint(e) => Ok(e
                        .send
                        .iter()
                        .map(|q| (q.msg.as_ref().map(|m| m.msg_id), q.fault.is_some()))
                        .collect::<Vec<_>>()),
                    _ => Err(KError::WrongObjectType),
                })
                .map_err(|e| format!("Endpoint {ep}: không đọc được hàng gửi: {e:?}"))?;
            for (id, is_fault) in items {
                match id {
                    Some(0) => {
                        return Err(format!("Endpoint {ep}: message chưa được cấp msg_id"));
                    }
                    Some(id) => {
                        if id > self.ipc_sent {
                            return Err(format!(
                                "Endpoint {ep}: msg_id {id} > ipc_sent {}",
                                self.ipc_sent
                            ));
                        }
                        if ids.contains(&id) {
                            return Err(format!("msg_id {id} xuất hiện nhiều hơn một nơi"));
                        }
                        ids.push(id);
                        pending += 1;
                    }
                    None => {
                        if !is_fault {
                            return Err(format!(
                                "Endpoint {ep}: message không có nội dung và không phải fault"
                            ));
                        }
                    }
                }
            }
        }
        for (i, meta) in self.objects.iter().enumerate() {
            if !meta.alive || meta.ty != ObjType::Tcb {
                continue;
            }
            let tcb = (i + 1) as ObjId;
            let id = self
                .body(tcb, |k| match k {
                    Kind::Tcb(t) => Ok(t.ipc.msg_id),
                    _ => Err(KError::WrongObjectType),
                })
                .map_err(|e| format!("TCB {tcb}: không đọc được vùng IPC: {e:?}"))?;
            let Some(id) = id else { continue };
            if id == 0 || id > self.ipc_sent {
                return Err(format!(
                    "TCB {tcb}: msg_id {id} không nằm trong dải 1..=ipc_sent"
                ));
            }
            if ids.contains(&id) {
                return Err(format!(
                    "TCB {tcb}: msg_id {id} vừa ở vùng nhận vừa còn trong hàng gửi"
                ));
            }
        }
        if self.transfer_calls != self.ipc_delivered {
            return Err(format!(
                "transfer_calls {} != ipc_delivered {}",
                self.transfer_calls, self.ipc_delivered
            ));
        }
        if self.ipc_delivered > self.ipc_sent {
            return Err(format!(
                "ipc_delivered {} > ipc_sent {}",
                self.ipc_delivered, self.ipc_sent
            ));
        }
        if self.ipc_sent != self.ipc_delivered + pending + self.ipc_dropped {
            return Err(format!(
                "message mất/thêm: ipc_sent={} != delivered={} + pending={pan} + dropped={}",
                self.ipc_sent,
                self.ipc_delivered,
                self.ipc_dropped,
                pan = pending
            ));
        }
        Ok(())
    }

    // ------------------------------------------- KT-34: bất biến liên kết Reply (C2)

    /// **KT-34** — bất biến **liên kết Reply** (C2 của K1C-DOD; D2-63), chạy sau **mỗi** public
    /// op qua [`Machine::sched_after_op`] (cùng chỗ `check_ipc`/`check_notifications`; gom một
    /// hàm vì mỗi op chỉ thêm **một** lượt quét object, xem `v2/docs/k1c-ipc-semantics.md` §15):
    ///
    /// 1. `Reply.caller = Some(t)` ⇒ `t` là TCB **sống** và **đang** `BlockedOnReply{reply}`
    ///    với `reply` **đúng** object này. Chiều này bắt đường "đổi trạng thái trực tiếp"
    ///    (`transition_tcb`) làm TCB rời `BlockedOnReply` mà quên gỡ `caller`, và bắt **ghi đè**
    ///    caller (hai TCB cùng chờ một Reply thì một trong hai sai).
    /// 2. `TcbState::BlockedOnReply{reply}` ⇒ `reply` là object `Reply` **sống** và
    ///    `caller == Some(tcb)` (chiều ngược). Thiếu chiều này, caller bị "treo im lặng": chờ
    ///    một Reply không còn trỏ về mình nên `Send(reply)` không bao giờ đánh thức.
    ///
    /// Không có "trạng thái transition hợp lệ" nào ở **cuối** op: `call_msg`/`deliver_queued`/
    /// `reply_with`/`cancel_reply` đều đặt `caller` và trạng thái trong **cùng** một op
    /// (đường ghi duy nhất [`Machine::transition_tcb`]); trạng thái nửa vời chỉ tồn tại **bên
    /// trong** op nên không bị kiểm. `Reply.caller = Some(t)` mà `t` không `BlockedOnReply`
    /// (kể cả `BlockedOnRecv{reply: rep}` — dùng lại cap active) là **lỗi mô hình**.
    pub fn check_reply_links(&self) -> Result<(), String> {
        let n = self.objects.len();
        // 1. Mọi Reply **sống**: `caller` phải là TCB sống đang chờ **đúng** Reply này.
        for (i, meta) in self.objects.iter().enumerate() {
            if !meta.alive || meta.ty != ObjType::Reply {
                continue;
            }
            let rep = (i + 1) as ObjId;
            let caller = self
                .reply_caller(rep)
                .map_err(|e| format!("Reply {rep}: caller không đọc được: {e:?}"))?;
            let Some(t) = caller else { continue };
            if t == 0 || t as usize > n || !self.objects[t as usize - 1].alive {
                return Err(format!("Reply {rep}: caller {t} không phải TCB sống"));
            }
            if self.objects[t as usize - 1].ty != ObjType::Tcb {
                return Err(format!("Reply {rep}: caller {t} không phải TCB"));
            }
            match self.tcb_state(t) {
                Ok(TcbState::BlockedOnReply { reply }) if reply == rep => {}
                other => {
                    return Err(format!(
                        "Reply {rep}: caller {t} không ở BlockedOnReply trên Reply này ({other:?})"
                    ));
                }
            }
        }
        // 2. Mọi TCB **sống** đang `BlockedOnReply{reply}`: Reply đó phải sống và trỏ về TCB.
        for (i, meta) in self.objects.iter().enumerate() {
            if !meta.alive || meta.ty != ObjType::Tcb {
                continue;
            }
            let tcb = (i + 1) as ObjId;
            let Ok(TcbState::BlockedOnReply { reply }) = self.tcb_state(tcb) else {
                continue;
            };
            if reply == 0 || reply as usize > n || !self.objects[reply as usize - 1].alive {
                return Err(format!(
                    "TCB {tcb}: BlockedOnReply trên Reply {reply} đã chết/không tồn tại"
                ));
            }
            if self.objects[reply as usize - 1].ty != ObjType::Reply {
                return Err(format!(
                    "TCB {tcb}: BlockedOnReply trên object {reply} không phải Reply"
                ));
            }
            let caller = self
                .reply_caller(reply)
                .map_err(|e| format!("TCB {tcb}: Reply {reply} không đọc được: {e:?}"))?;
            if caller != Some(tcb) {
                return Err(format!(
                    "TCB {tcb}: BlockedOnReply{{reply {reply}}} nhưng Reply.caller = {caller:?}"
                ));
            }
        }
        Ok(())
    }

    // ------------------------------------------- KT-24: bộ đếm/đọc vùng nhận IPC

    /// Số message **người dùng** đã nhận vào đường IPC (message fault `Exit` không tính).
    pub fn ipc_sent(&self) -> u64 {
        self.ipc_sent
    }

    /// Số message đã giao vào vùng nhận của TCB (mỗi lần giao đúng một `ipc::transfer`).
    pub fn ipc_delivered(&self) -> u64 {
        self.ipc_delivered
    }

    /// Số message bị huỷ khi TCB/Endpoint chết hoặc bị `Suspend(Abort)` (D2-25).
    pub fn ipc_dropped(&self) -> u64 {
        self.ipc_dropped
    }

    /// Số lần gọi `crate::ipc::transfer` — **test-only**, luôn bằng [`Machine::ipc_delivered`].
    pub fn transfer_calls(&self) -> u64 {
        self.transfer_calls
    }

    // ------------------------------------------- KT-25: bộ đếm chuyển cap (mềm, D2-53 Q6)

    /// Số cap **cài được** vào receive window qua đường IPC (đã bỏ `RIGHT_GRANT`).
    pub fn cap_transfer_installed(&self) -> u64 {
        self.cap_transfer_installed
    }

    /// Số cap **thất bại mềm** khi chuyển (thiếu `RIGHT_GRANT`, ô đích đã có cap, cap nguồn
    /// chết giữa chừng) — **không** phải bit MessageInfo (D2-53 Q6).
    pub fn cap_transfer_failed(&self) -> u64 {
        self.cap_transfer_failed
    }

    /// Số cap **unwrap** thành badge (cap tới chính Endpoint đang gọi + có badge).
    pub fn cap_unwrapped(&self) -> u64 {
        self.cap_unwrapped
    }

    /// `(cài được, unwrap, thất bại mềm)` — ảnh chụp bộ đếm chuyển cap.
    pub fn cap_transfer_stats(&self) -> (u64, u64, u64) {
        (
            self.cap_transfer_installed,
            self.cap_unwrapped,
            self.cap_transfer_failed,
        )
    }

    /// Số message **người dùng** đang nằm trong các hàng gửi Endpoint (đếm lại từ hàng thật).
    pub fn ipc_pending(&self) -> u64 {
        let mut n = 0u64;
        for (i, meta) in self.objects.iter().enumerate() {
            if !meta.alive || meta.ty != ObjType::Endpoint {
                continue;
            }
            let ep = (i + 1) as ObjId;
            if let Ok(v) = self.body(ep, |k| match k {
                Kind::Endpoint(e) => Ok(e.send.iter().filter(|q| q.msg.is_some()).count() as u64),
                _ => Err(KError::WrongObjectType),
            }) {
                n += v;
            }
        }
        n
    }

    /// `MessageInfo` của message **vừa nhận** ở vùng nhận của TCB (`None` = chưa nhận).
    pub fn tcb_ipc_info(&self, tcb: ObjId) -> Result<Option<MessageInfo>, KError> {
        self.body(tcb, |k| match k {
            Kind::Tcb(t) => Ok(t.ipc.info),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// 4 MR "thanh ghi" (MR0..MR3) của message vừa nhận ở TCB.
    pub fn tcb_ipc_regs(&self, tcb: ObjId) -> Result<[u64; ipc::WORDS_IN_REGISTERS], KError> {
        self.body(tcb, |k| match k {
            Kind::Tcb(t) => Ok(t.ipc.regs),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// **KT-26** — alias model API `[TK]` của [`Machine::tcb_ipc_regs`] cho đường **fault-resume**:
    /// "thanh ghi" của TCB sau khi được `Reply`/`tcb_resume_with_regs` ghi **trước** `Ready`
    /// (thiết kế §4.4 `:279`, chống đường mất message r319).
    pub fn tcb_regs(&self, tcb: ObjId) -> Result<[u64; ipc::WORDS_IN_REGISTERS], KError> {
        self.tcb_ipc_regs(tcb)
    }

    /// 60 MR "IPC buffer model" (MR4..MR63) của message vừa nhận ở TCB — **vùng riêng**, không
    /// gộp với thanh ghi (KT-24 DoD 2).
    pub fn tcb_ipc_buffer(&self, tcb: ObjId) -> Result<[u64; ipc::WORDS_IN_IPC_BUFFER], KError> {
        self.body(tcb, |k| match k {
            Kind::Tcb(t) => Ok(t.ipc.buffer),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// `msg_id` của message vừa nhận ở TCB (kernel cấp; `None` = chưa nhận).
    pub fn tcb_ipc_msg_id(&self, tcb: ObjId) -> Result<Option<u64>, KError> {
        self.body(tcb, |k| match k {
            Kind::Tcb(t) => Ok(t.ipc.msg_id),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// Badge của message vừa nhận ở TCB (KT-25 sẽ chuyển badge thật; KT-24 giữ 0).
    pub fn tcb_ipc_badge(&self, tcb: ObjId) -> Result<u64, KError> {
        self.body(tcb, |k| match k {
            Kind::Tcb(t) => Ok(t.ipc.badge),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// Lỗi IPC kernel gán cho TCB ở lần giao gần nhất (`Cancelled` cho nhánh `no_reply` (b) —
    /// D2-53 Q7); `None` nếu không có.
    pub fn tcb_last_ipc_error(&self, tcb: ObjId) -> Result<Option<KError>, KError> {
        self.body(tcb, |k| match k {
            Kind::Tcb(t) => Ok(t.last_ipc_error),
            _ => Err(KError::WrongObjectType),
        })
    }

    // ------------------------------------------- đọc hàng chờ (test/đối chứng)

    pub fn endpoint_send_len(&self, ep: ObjId) -> Result<usize, KError> {
        self.body(ep, |k| match k {
            Kind::Endpoint(e) => Ok(e.send.len()),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// KT-22 — ảnh chụp hộp thư (mailbox) của Endpoint: sức chứa, số đang chờ, bộ đếm
    /// `pushed/popped/removed` (bất biến `pushed == popped + removed + len`) + Recv waiter.
    pub fn endpoint_info(&self, ep: ObjId) -> Result<EndpointInfo, KError> {
        self.body(ep, |k| match k {
            Kind::Endpoint(e) => Ok(EndpointInfo {
                capacity: e.send.capacity(),
                len: e.send.len(),
                pushed: e.send.pushed(),
                popped: e.send.popped(),
                removed: e.send.removed(),
                recv_waiter: e.recv.map(|(tcb, _)| tcb),
            }),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// Hàng gửi của Endpoint theo đúng **thứ tự FIFO** — API K1b (KT-22) giữ nguyên: trả
    /// `(sender, fault)` để `kt22_mailbox`/`kt23_k1b` không phải đổi. Nội dung message đọc qua
    /// [`Machine::endpoint_msg_ids`] (KT-24).
    pub fn endpoint_send_items(
        &self,
        ep: ObjId,
    ) -> Result<Vec<(ObjId, Option<FaultKind>)>, KError> {
        self.body(ep, |k| match k {
            Kind::Endpoint(e) => Ok(e.send.iter().map(|q| (q.sender, q.fault)).collect()),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// **KT-24** — `msg_id` của các message **người dùng** trong hàng gửi, đúng thứ tự FIFO
    /// (message fault `Exit` không có `msg_id` nên **không** xuất hiện).
    pub fn endpoint_msg_ids(&self, ep: ObjId) -> Result<Vec<u64>, KError> {
        self.body(ep, |k| match k {
            Kind::Endpoint(e) => Ok(e
                .send
                .iter()
                .filter_map(|q| q.msg.as_ref().map(|m| m.msg_id))
                .collect()),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// **KT-60** — ảnh chụp message **đang chờ** trong hàng gửi Endpoint (chỉ đọc; accessor
    /// parity, **không** đổi semantics): người gửi + message + cờ `call`/`blocking` + fault.
    ///
    /// Trước KT-60 oracle chỉ có `endpoint_send_items` (sender/fault) và `endpoint_msg_ids`;
    /// nội dung message đang treo chưa đọc được nên parity KT-59 không so được — nợ §7.5.
    pub fn endpoint_queued(&self, ep: ObjId) -> Result<Vec<QueuedView>, KError> {
        self.body(ep, |k| match k {
            Kind::Endpoint(e) => Ok(e
                .send
                .iter()
                .map(|q| QueuedView {
                    sender: q.sender,
                    msg: q.msg.as_deref().cloned(),
                    call: q.call,
                    fault: q.fault,
                    blocking: q.blocking,
                })
                .collect()),
            _ => Err(KError::WrongObjectType),
        })
    }

    pub fn endpoint_recv_waiter(&self, ep: ObjId) -> Result<Option<ObjId>, KError> {
        self.body(ep, |k| match k {
            Kind::Endpoint(e) => Ok(e.recv.map(|(tcb, _)| tcb)),
            _ => Err(KError::WrongObjectType),
        })
    }

    pub fn endpoint_last_mr(&self, ep: ObjId) -> Result<Option<u64>, KError> {
        self.body(ep, |k| match k {
            Kind::Endpoint(e) => Ok(e.last_mr),
            _ => Err(KError::WrongObjectType),
        })
    }

    pub fn reply_caller(&self, reply: ObjId) -> Result<Option<ObjId>, KError> {
        self.body(reply, |k| match k {
            Kind::Reply(r) => Ok(r.caller),
            _ => Err(KError::WrongObjectType),
        })
    }

    // ---------------------------------------------------------------- IPC (KT-24)

    /// Resolve cap cho entry IPC: `SlotEmpty` quy về `InvalidCapability` đúng nghĩa ABI
    /// ("cap không hợp lệ" cho tham số syscall — `SlotEmpty` là mã của thao tác CNode, không
    /// phải của `Call`/`Send`; `.scn` 06 `call cap=c1 => InvalidCapability`).
    pub(super) fn ipc_slot_cap(&self, slot: SlotRef) -> Result<CapId, KError> {
        self.cap_at(slot.cnode, slot.slot).map_err(|e| match e {
            KError::SlotEmpty => KError::InvalidCapability,
            other => other,
        })
    }

    /// Endpoint đích của `Call`. Object **không** phải Endpoint = invocation chưa cài ⇒
    /// `NotSupported`, **không** chặn, **không** Reply, `TcbState` không đổi (D2-53 Q11).
    pub(super) fn ipc_call_endpoint(&self, slot: SlotRef) -> Result<ObjId, KError> {
        let cap = self.ipc_slot_cap(slot)?;
        let c = self.cap(cap)?;
        if c.ty != ObjType::Endpoint {
            return Err(KError::NotSupported);
        }
        Ok(c.obj)
    }

    /// Loại object của cap cho `Send` (dispatch `Endpoint`/`Reply`/`Notification`).
    pub(super) fn ipc_cap_type(&self, slot: SlotRef) -> Result<ObjType, KError> {
        let cap = self.ipc_slot_cap(slot)?;
        Ok(self.cap(cap)?.ty)
    }

    /// Peek waiter `Recv` của Endpoint: `(có waiter, Reply cap người nhận đã cấp)`.
    pub(super) fn endpoint_recv_slot(&self, ep: ObjId) -> Result<(bool, Option<CapId>), KError> {
        self.body(ep, |k| match k {
            Kind::Endpoint(e) => Ok((e.recv.is_some(), e.recv.as_ref().and_then(|(_, cap)| *cap))),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// TCB đang chờ `Recv` trên Endpoint (chỉ **đọc**, không gỡ hàng) — KT-25 dùng để kiểm
    /// receive window của bên nhận **trước** khi message vào đường IPC.
    pub(super) fn endpoint_recv_waiter_obj(&self, ep: ObjId) -> Result<Option<ObjId>, KError> {
        self.body(ep, |k| match k {
            Kind::Endpoint(e) => Ok(e.recv.map(|(t, _)| t)),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// Lấy (và gỡ) TCB đang chờ `Recv` trên Endpoint — **sau khi** đã peek/kiểm Reply cap.
    pub(super) fn endpoint_take_waiter(&mut self, ep: ObjId) -> Result<Option<ObjId>, KError> {
        self.body_mut(ep, |k| match k {
            Kind::Endpoint(e) => Ok(e.recv.take().map(|(t, _)| t)),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// Hàng gửi đầy? (chỉ kiểm khi **không** có waiter — có waiter thì giao thẳng, không dùng hàng.)
    pub(super) fn endpoint_full(&self, ep: ObjId) -> Result<bool, KError> {
        self.body(ep, |k| match k {
            Kind::Endpoint(e) => Ok(e.send.is_full()),
            _ => Err(KError::WrongObjectType),
        })
    }

    /// **KT-25** — ảnh chụp cap nguồn của message **đầu** hàng gửi (`None` = hàng rỗng) —
    /// để kiểm receive window trước khi pop (không giao nửa chừng).
    pub(super) fn endpoint_front_snaps(
        &self,
        ep: ObjId,
    ) -> Result<Option<Vec<ipc::CapSnapshot>>, KError> {
        self.body(ep, |k| match k {
            Kind::Endpoint(e) => Ok(e.send.front().map(|q| q.snaps.clone())),
            _ => Err(KError::WrongObjectType),
        })
    }

    pub(super) fn endpoint_pop(&mut self, ep: ObjId) -> Result<Option<QueuedMessage>, KError> {
        self.body_mut(ep, |k| match k {
            Kind::Endpoint(e) => Ok(e.send.pop_front()),
            _ => Err(KError::WrongObjectType),
        })
    }

    pub(super) fn endpoint_enqueue(&mut self, ep: ObjId, q: QueuedMessage) -> Result<(), KError> {
        self.body_mut(ep, |k| match k {
            Kind::Endpoint(e) => e.send.try_push(q).map_err(|_| KError::Busy),
            _ => Err(KError::WrongObjectType),
        })
    }

    pub(super) fn set_endpoint_last_mr(&mut self, ep: ObjId, mr: u64) -> Result<(), KError> {
        self.body_mut(ep, |k| match k {
            Kind::Endpoint(e) => {
                e.last_mr = Some(mr);
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })
    }

    /// Nhận message vào đường IPC: kiểm biên cap (`≤ 4`, khớp `MessageInfo.caps`) và danh sách
    /// ảnh chụp cap đã resolve (KT-25) rồi **cấp `msg_id`** từ bộ đếm `ipc_sent` (không bao giờ
    /// tái dùng). Cap **không** bị bỏ im lặng: cap chết ⇒ `InvalidCapability` ở `resolve_caps`;
    /// thiếu `RIGHT_GRANT`/ô đích bận ⇒ **thất bại mềm** khi giao (`deliver_caps`, D2-53 Q6).
    pub(super) fn admit_message(
        &mut self,
        mut msg: Message,
        snaps: Vec<ipc::CapSnapshot>,
    ) -> Result<Message, KError> {
        // **KT-25**: biên ABI `max_caps = 4` (không nới) + `info.caps` phải khớp danh sách.
        if msg.caps.len() > ipc::MAX_CAPS
            || msg.caps.len() != msg.info.caps() as usize
            || snaps.len() != msg.caps.len()
        {
            return Err(KError::InvalidArgument);
        }
        self.ipc_sent += 1;
        msg.msg_id = self.ipc_sent;
        Ok(msg)
    }

    /// **KT-25** — chụp ảnh cap nguồn của message: **lỗi cứng** `InvalidCapability` khi cap
    /// chết/không tồn tại (`cap = 0`/ngoài bảng), **trước** khi cấp `msg_id` (message không bị
    /// giao nửa chừng — KT-25 DoD 4).
    pub(super) fn resolve_caps(&self, msg: &Message) -> Result<Vec<ipc::CapSnapshot>, KError> {
        let mut snaps = Vec::with_capacity(msg.caps.len());
        for cap in &msg.caps {
            let c = self.cap(*cap)?;
            if !c.alive {
                return Err(KError::InvalidCapability);
            }
            snaps.push(ipc::CapSnapshot {
                cap: *cap,
                obj: c.obj,
                ty: c.ty,
                rights: c.rights,
                badge: c.badge,
            });
        }
        Ok(snaps)
    }

    /// **KT-25** — lập kế hoạch chuyển cap vào receive window của `receiver`.
    ///
    /// - message **không** cap ⇒ `None` (không cần window);
    /// - window **chưa bind** mà message có cap ⇒ `InvalidArgument` (lỗi cấu hình, **trước** khi
    ///   message được giao — khác "bind 4 ô rỗng" hợp lệ);
    /// - ngược lại ⇒ [`ipc::plan_cap_transfer`] với ô nào đang có cap (thất bại **mềm**, Q6).
    pub(super) fn prepare_caps(
        &self,
        receiver: ObjId,
        snaps: &[ipc::CapSnapshot],
        endpoint: Option<ObjId>,
    ) -> Result<Option<ipc::CapPlan>, KError> {
        if snaps.is_empty() {
            return Ok(None);
        }
        let window = self.tcb_recv_window(receiver)?;
        let mut occupied = [false; ipc::MAX_CAPS];
        if let Some(window) = window.as_ref() {
            for (i, w) in window.iter().enumerate() {
                occupied[i] = self.cnode_slot(w.cnode, w.slot)?.is_some();
            }
        }
        Ok(Some(ipc::plan_cap_transfer(
            snaps,
            window.as_ref(),
            &occupied,
            endpoint,
        )?))
    }

    /// **KT-25** — cài cap theo kế hoạch vào receive window và cập nhật `MessageInfo` của
    /// message **nhận**: `caps` = số cap **cài được**, `extra_caps_unwrapped` = số cap unwrap,
    /// `badge` = OR các badge unwrap. Cap nguồn chết giữa lúc xếp hàng và lúc giao ⇒ thất bại
    /// **mềm** (không tạo cap mồ côi, không mất message — D2-53 Q6).
    pub(super) fn deliver_caps(
        &mut self,
        msg: &mut Message,
        snaps: &[ipc::CapSnapshot],
        plan: Option<&ipc::CapPlan>,
    ) -> Result<(), KError> {
        if snaps.is_empty() {
            return Ok(());
        }
        let Some(plan) = plan else {
            return Err(KError::InvalidArgument);
        };
        let mut installed = 0u8;
        for (idx, dst) in &plan.install {
            let snap = &snaps[*idx];
            let alive = snap.cap > 0
                && self
                    .caps
                    .get(snap.cap as usize - 1)
                    .map(|c| c.alive)
                    .unwrap_or(false);
            if !alive {
                self.cap_transfer_failed += 1;
                continue;
            }
            // Cap nhận **mất** `RIGHT_GRANT` (thiết kế `:319`), giữ badge nguồn.
            let rights = snap.rights & !RIGHT_GRANT;
            let cap = self.push_cap(snap.obj, snap.ty, Some(snap.cap), rights, snap.badge);
            self.cnode_set_slot(dst.cnode, dst.slot, Some(cap))?;
            self.caps[cap as usize - 1].slot = Some(*dst);
            installed += 1;
        }
        self.cap_transfer_failed += plan.skipped as u64;
        self.cap_transfer_installed += installed as u64;
        self.cap_unwrapped += plan.unwrapped() as u64;
        msg.info = msg
            .info
            .with_caps(installed)?
            .with_extra_caps_unwrapped(plan.unwrapped() as u8)?;
        msg.badge |= plan.badge();
        // Cap đã cài vào window của bên nhận; message nhận **không** còn `CapId` nguồn.
        msg.caps.clear();
        Ok(())
    }

    /// Giao message vào vùng nhận của TCB qua **`crate::ipc::transfer`** (đường copy **duy nhất**);
    /// cộng `ipc_delivered` + `transfer_calls` (hai bộ đếm phải luôn bằng nhau).
    pub(super) fn deliver_to(&mut self, receiver: ObjId, msg: &Message) -> Result<(), KError> {
        self.body_mut(receiver, |k| match k {
            Kind::Tcb(t) => {
                ipc::transfer(msg, &mut t.ipc);
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        self.ipc_delivered += 1;
        self.transfer_calls += 1;
        Ok(())
    }

    /// **KT-25** — giao message: chuyển cap theo kế hoạch rồi copy message vào vùng nhận.
    pub(super) fn deliver_message(
        &mut self,
        receiver: ObjId,
        msg: &mut Message,
        snaps: &[ipc::CapSnapshot],
        plan: Option<&ipc::CapPlan>,
    ) -> Result<(), KError> {
        self.deliver_caps(msg, snaps, plan)?;
        self.deliver_to(receiver, msg)
    }

    pub(super) fn reset_ipc_error(&mut self, tcb: ObjId) -> Result<(), KError> {
        self.body_mut(tcb, |k| match k {
            Kind::Tcb(t) => {
                t.last_ipc_error = None;
                // **KT-26**: fault vừa giao thuộc về lần nhận **trước**; một op IPC mới của
                // bên nhận bắt đầu ⇒ bản ghi "fault vừa nhận" hết hiệu lực (tránh procsrv
                // tiêu thụ nhầm fault cũ).
                t.last_fault = None;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })
    }

    pub(super) fn set_last_ipc_error(&mut self, tcb: ObjId, e: KError) -> Result<(), KError> {
        self.body_mut(tcb, |k| match k {
            Kind::Tcb(t) => {
                t.last_ipc_error = Some(e);
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })
    }

    /// Giao một message **đã xếp hàng** cho TCB nhận (`recv`/`nb_recv`): chép qua `transfer`,
    /// xử lý người gửi (`Send` ⇒ `Ready`; `Call` + Reply ⇒ `BlockedOnReply` + donation;
    /// fault `Exit` ⇒ `Inactive`), và **nhánh (b)** của `no_reply`: thiếu Reply ⇒ kernel bật cờ
    /// trên message + người gọi `Cancelled` (D2-53 Q7). Trả message đã giao (nếu là message
    /// người dùng) để `nb_recv` trả lại cho test.
    pub(super) fn deliver_queued(
        &mut self,
        receiver: ObjId,
        q: QueuedMessage,
        reply_obj: Option<ObjId>,
        plan: Option<ipc::CapPlan>,
    ) -> Result<Option<Message>, KError> {
        let queued_snaps = q.snaps;
        let mut out = q.msg.map(|m| *m);
        let mut kernel_no_reply = false;
        if let Some(m) = out.as_mut() {
            if q.call && q.blocking && reply_obj.is_none() && !m.info.no_reply() {
                // Kernel thiếu Reply: message **vẫn** giao, cờ bật phía nhận (thiết kế `:322`).
                m.info = m.info.with_no_reply(true);
                kernel_no_reply = true;
            }
            self.deliver_message(receiver, m, &queued_snaps, plan.as_ref())?;
        }
        match q.fault {
            // **KT-26** — fault được ghi nhận cho bên nhận (procsrv) **trước** mọi chuyển
            // trạng thái; message fault không mang MR nên đây là đường duy nhất procsrv biết
            // `(sender, kind)`.
            Some(kind) => {
                self.record_last_fault(receiver, q.sender, kind)?;
                if kind.is_exit() {
                    // Fault Exit: procsrv Recv xong mới Inactive (QD §1 Q-A, D2-25).
                    self.transition_tcb(q.sender, NonReadyTarget::Inactive)?;
                } else {
                    // Fault ≠ Exit đi **Call + Reply** tới `fault_ep` (thiết kế §4.6 `:321`):
                    // caller chờ `BlockedOnReply` để procsrv trả lời kèm thanh ghi mới.
                    // Bên nhận **không** cấp Reply ⇒ không có đường resume: caller
                    // `Cancelled` (cùng khuôn Q7(b) cho `Call`), không treo vĩnh viễn.
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
                    // KT-21: `Call` xếp hàng nay mới giao được ⇒ SC của client đi theo.
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

    /// `Call` trên Endpoint — bản **message đầy đủ** (KT-24). Ngữ nghĩa:
    ///
    /// - Đích **không** phải Endpoint ⇒ `NotSupported`, không chặn/không Reply (D2-53 Q11).
    /// - Có waiter + Reply cap ⇒ giao thẳng, caller `BlockedOnReply` (như K1b).
    /// - Có waiter nhưng **không** Reply cap ⇒ giao thẳng, kernel bật `no_reply`, caller
    ///   `Cancelled` (D2-53 Q7 nhánh (b)).
    /// - Caller đặt `no_reply=1` ⇒ **Call = Send**, `Ok` ngay, **không** sinh Reply (Q7 nhánh (a)).
    /// - Không có waiter ⇒ xếp hàng + `BlockedOnSend{call:true}` (Call thường) hoặc xếp hàng
    ///   không chặn (no_reply).
    pub fn call_msg(&mut self, tcb: ObjId, ep: SlotRef, msg: Message) -> Result<(), KError> {
        let ep_obj = self.ipc_call_endpoint(ep)?;
        let state = self.ensure_started(tcb)?;
        if state != TcbState::Ready && !matches!(state, TcbState::Running { .. }) {
            return Err(KError::IllegalOperation);
        }
        self.reset_ipc_error(tcb)?;
        let (has_waiter, reply_cap) = self.endpoint_recv_slot(ep_obj)?;
        // Reply cap của người nhận phải hợp lệ **trước** khi nhận message (không mất message).
        let reply_obj = match reply_cap {
            Some(cap) => Some(self.require_cap_type(cap, ObjType::Reply)?),
            None => None,
        };
        // Hàng gửi cố định: đầy ⇒ `Busy` **trước** mọi thay đổi (không cấp `msg_id`, không
        // `last_mr`, không đổi trạng thái) — người gọi xử lý hộp thư rồi thử lại (KT-22 §4.17).
        if !has_waiter && self.endpoint_full(ep_obj)? {
            return Err(KError::Busy);
        }
        // **KT-34** (C2) — lớp chặn **thứ hai** cho lỗi "ghi đè `caller`": Reply cap mà waiter
        // `Recv` đang giữ có thể **đã** bị bind `caller` qua **một cap khác** cùng object (hai
        // cap cùng trỏ một `Reply`) trong lúc waiter chờ. Call này mà giao thì
        // `reply_set_caller` sẽ **ghi đè** caller cũ ⇒ TCB cũ mất đường Reply. Kiểm **trước**
        // `admit_message` (chưa cấp `msg_id`, chưa `take_waiter`, chưa đổi trạng thái), và chỉ
        // khi Call này **sẽ** bind (`!no_reply` + có Reply cap): `Call no_reply=1` = Send.
        let user_no_reply = msg.info.no_reply();
        if !user_no_reply {
            if let Some(rep) = reply_obj {
                if self.reply_caller(rep)?.is_some() {
                    return Err(KError::Busy);
                }
            }
        }
        // **KT-25**: chụp ảnh cap nguồn + kiểm receive window bên nhận **trước** khi cấp
        // `msg_id` (lỗi cứng ⇒ message không bị giao nửa chừng, không mất).
        let snaps = self.resolve_caps(&msg)?;
        let plan = match self.endpoint_recv_waiter_obj(ep_obj)? {
            Some(w) => self.prepare_caps(w, &snaps, Some(ep_obj))?,
            None => None,
        };
        let mut msg = self.admit_message(msg, snaps.clone())?;
        self.set_endpoint_last_mr(ep_obj, msg.mr[0])?;
        if has_waiter {
            let waiter = self
                .endpoint_take_waiter(ep_obj)?
                .expect("has_waiter vừa kiểm");
            let kernel_no_reply = !user_no_reply && reply_obj.is_none();
            if kernel_no_reply {
                msg.info = msg.info.with_no_reply(true);
            }
            self.deliver_message(waiter, &mut msg, &snaps, plan.as_ref())?;
            self.wake_from(waiter)?;
            if user_no_reply {
                // (a) Caller đặt cờ: Call = Send, **không** sinh Reply.
                self.sched_after_op(true);
                return Ok(());
            }
            match reply_obj {
                Some(rep) => {
                    self.reply_set_caller(rep, Some(tcb))?;
                    self.transition_tcb(tcb, NonReadyTarget::BlockedOnReply { reply: rep })?;
                    // KT-21 (D2-49(c)): SC của client đi theo `Call` sang server passive.
                    self.donate_sc(tcb, waiter)?;
                }
                None => {
                    // (b) Kernel thiếu Reply: message vẫn giao, caller `Cancelled`.
                    self.set_last_ipc_error(tcb, KError::Cancelled)?;
                    self.sched_after_op(true);
                    return Err(KError::Cancelled);
                }
            }
        } else {
            let blocking = !user_no_reply;
            self.endpoint_enqueue(
                ep_obj,
                QueuedMessage {
                    msg: Some(Box::new(msg)),
                    sender: tcb,
                    call: blocking,
                    fault: None,
                    blocking,
                    snaps,
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

    /// `Call` bản **một MR** của K1b (giữ API cũ cho `kt09_e2e`/`kt18_tcb`/`kt21_donation`) —
    /// wrapper mỏng của [`Machine::call_msg`] với `Message` 1 word, `label = 0`.
    pub fn call(&mut self, tcb: ObjId, ep: SlotRef, mr: u64) -> Result<(), KError> {
        let info = MessageInfo::new(0, 0, false, 0, 1)?;
        let msg = Message::new(info, &[mr])?;
        self.call_msg(tcb, ep, msg)
    }

    /// `Send` trên Endpoint/Reply: dispatch theo loại cap đúng ABI (`spec/abi.toml:107`):
    /// `Endpoint` ⇒ gửi message (chặn: `BlockedOnSend` khi phải xếp hàng), `Reply` ⇒ trả lời,
    /// `Notification` ⇒ KT-27 (`NotSupported`), loại khác ⇒ `WrongObjectType`.
    pub fn send(&mut self, tcb: ObjId, cap: SlotRef, msg: Message) -> Result<(), KError> {
        match self.ipc_cap_type(cap)? {
            ObjType::Endpoint => self.endpoint_send(tcb, cap, msg, true),
            ObjType::Reply => self.reply_msg(tcb, cap, msg),
            // **KT-27** — spec `:107`: `Send` trên Notification = **Signal** (nội dung message
            // không dùng; badge lấy từ cap, seL4).
            ObjType::Notification => self.notification_signal_via_cap(tcb, cap),
            _ => Err(KError::WrongObjectType),
        }
    }

    /// **KT-27** — `Send`/`NBSend` trên **Notification** = **Signal** với badge của cap
    /// (seL4: badge của cap signal được OR vào word; `None` = 0 — **hợp lệ**). Kiểm TCB chạy
    /// được + `reset_ipc_error` như mọi syscall IPC khác; nội dung `Message` **không** dùng.
    pub(super) fn notification_signal_via_cap(
        &mut self,
        tcb: ObjId,
        cap: SlotRef,
    ) -> Result<(), KError> {
        let cap_id = self.ipc_slot_cap(cap)?;
        let (ntfn_obj, badge) = {
            let c = self.cap(cap_id)?;
            if !c.alive {
                return Err(KError::InvalidCapability);
            }
            if c.ty != ObjType::Notification {
                return Err(KError::WrongObjectType);
            }
            (c.obj, c.badge.unwrap_or(0))
        };
        let state = self.ensure_started(tcb)?;
        if state != TcbState::Ready && !matches!(state, TcbState::Running { .. }) {
            return Err(KError::IllegalOperation);
        }
        self.reset_ipc_error(tcb)?;
        self.notification_signal(ntfn_obj, badge)
    }

    /// `NBSend` — **không chặn**: bên nhận đang `Recv` ⇒ giao ngay; không ⇒ vào hàng gửi của
    /// Endpoint và người gửi **vẫn** `Ready`/`Running` (`spec/abi.toml:108` "không chờ").
    /// **KT-27** — trên Notification, `NBSend` cũng là **Signal** (không chặn theo bản chất).
    pub fn nb_send(&mut self, tcb: ObjId, ep: SlotRef, msg: Message) -> Result<(), KError> {
        match self.ipc_cap_type(ep)? {
            ObjType::Endpoint => self.endpoint_send(tcb, ep, msg, false),
            ObjType::Notification => self.notification_signal_via_cap(tcb, ep),
            _ => Err(KError::WrongObjectType),
        }
    }

    /// Đường gửi chung của `Send` (blocking) / `NBSend` (non-blocking) trên Endpoint.
    pub(super) fn endpoint_send(
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
        if !has_waiter && self.endpoint_full(ep_obj)? {
            return Err(KError::Busy);
        }
        // **KT-25**: cap nguồn + receive window của bên nhận kiểm **trước** khi cấp `msg_id`.
        let snaps = self.resolve_caps(&msg)?;
        let plan = match self.endpoint_recv_waiter_obj(ep_obj)? {
            Some(w) => self.prepare_caps(w, &snaps, Some(ep_obj))?,
            None => None,
        };
        let mut msg = self.admit_message(msg, snaps.clone())?;
        self.set_endpoint_last_mr(ep_obj, msg.mr[0])?;
        if has_waiter {
            let waiter = self
                .endpoint_take_waiter(ep_obj)?
                .expect("has_waiter vừa kiểm");
            self.deliver_message(waiter, &mut msg, &snaps, plan.as_ref())?;
            self.wake_from(waiter)?;
        } else {
            self.endpoint_enqueue(
                ep_obj,
                QueuedMessage {
                    msg: Some(Box::new(msg)),
                    sender: tcb,
                    call: false,
                    fault: None,
                    blocking,
                    snaps,
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

    /// `Recv` trên Endpoint: lấy message đang chờ (chép qua `ipc::transfer`), hoặc chặn
    /// `BlockedOnRecv{ep, reply}`. `reply` là Reply cap của **người nhận** (nếu cấp); thiếu nó
    /// khi nhận `Call` ⇒ kernel bật `no_reply` + caller `Cancelled` (D2-53 Q7).
    pub fn recv(&mut self, tcb: ObjId, ep: SlotRef, reply: Option<SlotRef>) -> Result<(), KError> {
        let state = self.ensure_started(tcb)?;
        if state != TcbState::Ready && !matches!(state, TcbState::Running { .. }) {
            return Err(KError::IllegalOperation);
        }
        let cap = self.ipc_slot_cap(ep)?;
        // **KT-27** — cap Notification ⇒ `Recv` = **Wait** (spec `:109` "Endpoint hoặc
        // Notification"), phân nhánh theo `ObjType` của cap, không nhầm
        // `BlockedOnRecv`/`BlockedOnNotification`.
        match self.cap(cap)?.ty {
            ObjType::Notification => {
                if reply.is_some() {
                    return Err(KError::InvalidArgument);
                }
                let ntfn_obj = self.require_cap_type(cap, ObjType::Notification)?;
                // Cùng hợp đồng KT-26 với nhánh Endpoint/`nb_recv`: một op IPC mới **xoá**
                // `last_ipc_error`/`last_fault` cũ trước khi Wait/Poll chạy.
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
        // **KT-34** (C2, D2-63) — cap `Reply` **đang active** (`caller = Some`) **không** được
        // dùng lại để `Recv`: nếu nhận, `Call` kế tiếp sẽ `reply_set_caller(rep, Some(new))` và
        // **ghi đè** `caller` cũ ⇒ TCB cũ đang `BlockedOnReply` **mất đường Reply/đánh thức**
        // (treo vĩnh viễn). Từ chối `Busy` **trước** khi xoá lỗi IPC cũ, trước khi pop/đổi
        // trạng thái ⇒ không mất message, không đổi `caller` (xem `v2/docs/k1c-ipc-semantics.md`
        // §15). `Reply` **không** còn caller (chưa dùng hoặc vừa `Reply` xong) dùng lại **được**.
        if let Some(rep) = reply_obj {
            if self.reply_caller(rep)?.is_some() {
                return Err(KError::Busy);
            }
        }
        self.reset_ipc_error(tcb)?;
        // **KT-25**: kiểm receive window **trước** khi pop (message có cap mà window chưa bind
        // ⇒ `InvalidArgument`, message **ở lại** hàng — không mất, không giao nửa chừng).
        let plan = match self.endpoint_front_snaps(ep_obj)? {
            Some(snaps) => self.prepare_caps(tcb, &snaps, Some(ep_obj))?,
            None => None,
        };
        match self.endpoint_pop(ep_obj)? {
            Some(q) => {
                let _ = self.deliver_queued(tcb, q, reply_obj, plan)?;
            }
            None => {
                self.body_mut(ep_obj, |k| match k {
                    Kind::Endpoint(e) => {
                        e.recv = Some((tcb, reply_cap));
                        Ok(())
                    }
                    _ => Err(KError::WrongObjectType),
                })?;
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

    /// `NBRecv` — **Poll**: `Busy` khi hàng rỗng (`spec/abi.toml:110`), không chặn. Trả message
    /// vừa nhận (đã chép vào `TcbData.ipc`) để test kiểm 64 MR/label/len.
    pub fn nb_recv(&mut self, tcb: ObjId, ep: SlotRef) -> Result<Option<Message>, KError> {
        let state = self.ensure_started(tcb)?;
        if state != TcbState::Ready && !matches!(state, TcbState::Running { .. }) {
            // **KT-25** — `NBRecv` là **poll** (`spec/abi.toml:110` "không chặn"): TCB không
            // chạy được (đang `BlockedOnReply`/`BlockedOnSend`/`BlockedOnRecv`…) thì "không có
            // message ngay" ⇒ `Busy`, **không** đổi trạng thái. Poll không chặn nên không thể
            // tự nó tạo chuyển trạng thái trái `ALLOWED`; `.scn` 05 dựa vào đúng ngữ nghĩa này
            // (client poll Endpoint sau khi `ReplyRecv` của server đã lấy message của nó).
            return Err(KError::Busy);
        }
        let cap = self.ipc_slot_cap(ep)?;
        // **KT-27** — cap Notification ⇒ `NBRecv` = **Poll**: word ≠ 0 trả `(word, badge)` và
        // xoá; rỗng ⇒ `Busy` (spec `:110` "Poll: Busy khi không có message").
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
        // **KT-25**: như `recv` — kiểm receive window trước khi pop.
        let plan = match self.endpoint_front_snaps(ep_obj)? {
            Some(snaps) => self.prepare_caps(tcb, &snaps, Some(ep_obj))?,
            None => None,
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

    /// `Send` trên Reply: đánh thức người gọi `BlockedOnReply → Ready` (qua một cửa).
    ///
    /// Nếu `Reply.caller` đã bị `Suspend(Abort)` tách ra: thiết kế §4.4 `:281` — Send **thành
    /// công nhưng reply bị bỏ** (không lỗi, không đánh thức ai).
    pub fn reply(&mut self, tcb: ObjId, reply: SlotRef) -> Result<(), KError> {
        // API K1b không có tham số message ⇒ reply mang message **rỗng** (0 MR) — tương đương
        // `Send(reply)` với `length = 0` của ABI (`spec/abi.toml:107`).
        self.reply_with(tcb, reply, None)
    }

    /// `Send` trên **Reply** với message đầy đủ (ABI `Send` trên Reply = trả lời) — message đi
    /// qua `ipc::transfer` tới **caller** trước khi caller `Ready` (thiết kế §4.6 `:324`).
    pub fn reply_msg(&mut self, tcb: ObjId, reply: SlotRef, msg: Message) -> Result<(), KError> {
        self.reply_with(tcb, reply, Some(msg))
    }

    /// Thân chung của `reply`: trả donation (KT-21), giao message (nếu có) cho caller, rồi đánh
    /// thức caller `BlockedOnReply → Ready` qua **một** cửa `transition_tcb`.
    pub(super) fn reply_with(
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
        // **KT-25**: kiểm cap của message reply + receive window của caller **trước** khi lấy
        // `caller` — lỗi cứng (cap chết/window chưa bind) **không** được để `Reply.caller = None`
        // trong khi caller vẫn `BlockedOnReply` (mất binding = treo).
        let caller_peek = self.body(reply_obj, |k| match k {
            Kind::Reply(r) => Ok(r.caller),
            _ => Err(KError::WrongObjectType),
        })?;
        let (snaps, plan) = match msg.as_ref() {
            Some(m) => {
                if m.caps.len() > ipc::MAX_CAPS || m.caps.len() != m.info.caps() as usize {
                    return Err(KError::InvalidArgument);
                }
                let snaps = self.resolve_caps(m)?;
                let plan = match caller_peek {
                    Some(c) => self.prepare_caps(c, &snaps, None)?,
                    None => None,
                };
                (snaps, plan)
            }
            None => (Vec::new(), None),
        };
        let caller = self.body_mut(reply_obj, |k| match k {
            Kind::Reply(r) => Ok(r.caller.take()),
            _ => Err(KError::WrongObjectType),
        })?;
        // KT-21 (D2-49(c)): **chỉ** trả SC donation khi Reply đúng Reply của **donor** (người
        // cho mượn SC). Reply nhầm Reply object (`caller = None` hoặc người khác) **không** trả
        // SC và không đánh thức nhầm ai — xem `JOURNAL/kt21-reply-owner.md` (kiểm chủ sở hữu
        // cap/`ReplyRecv`/4 cap đầy đủ thuộc K1c). Khi trả: bucket server về priority gốc,
        // client giữ nguyên `consumed` của chu kỳ (DoD 2).
        if let Some(donor) = self.donation_donor_of(tcb) {
            if caller == Some(donor) {
                self.return_donation(tcb)?;
            }
        }
        if let Some(m) = msg {
            // Message của Reply cũng đi qua **cùng** `admit_message` + `ipc::transfer`; không có
            // caller (reply đã bị bỏ — thiết kế §4.4 `:281`) ⇒ đếm `ipc_dropped` để bất biến
            // bảo toàn message vẫn đúng, không im lặng "mất" message. **KT-25**: cap của Reply
            // cũng đi theo message (đã chụp ảnh + kiểm window **trước** khi lấy `caller`).
            let mut m = self.admit_message(m, snaps.clone())?;
            match caller {
                Some(c) => self.deliver_message(c, &mut m, &snaps, plan.as_ref())?,
                None => self.ipc_dropped += 1,
            }
        }
        if let Some(caller) = caller {
            self.wake_from(caller)?;
        }
        if matches!(state, TcbState::Running { .. }) {
            self.yield_to_ready(tcb)?;
        }
        // **KT-26**: server vừa trả lời (Reply/ReplyRecv) ⇒ bản ghi "fault vừa nhận" đã xử lý
        // xong; xoá để `recv_exit` kế tiếp không tiêu thụ nhầm fault cũ.
        self.body_mut(tcb, |k| match k {
            Kind::Tcb(t) => {
                t.last_fault = None;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
        self.sched_after_op(true);
        Ok(())
    }

    /// **KT-25** — `ReplyRecv` (đường nóng server; thiết kế §4.6 `:320`): giao **reply trước**
    /// (`Send` trên Reply của server), rồi `Recv` trên Endpoint với Reply **mới**.
    ///
    /// Thứ tự bắt buộc: reply được giao **xong** trước khi `Recv` có thể chặn — nếu làm ngược,
    /// `Recv` chặn sẽ bỏ mất reply (K1C-PLAN §5 KT-25 rủi ro (a)). Không có message ⇒
    /// `BlockedOnRecv{ep, reply: Some(reply mới)}` (mang Reply **mới**, không mang Reply vừa
    /// dùng).
    ///
    /// Mọi tham số được kiểm **trước** khi giao reply (cap Reply/Endpoint + Reply mới) ⇒ lỗi
    /// cấu hình không để lại reply đã gửi nửa chừng. `recv_reply = None` ⇒ `Recv` không cấp
    /// Reply cho Call kế (Q7 nhánh (b)).
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
            // **KT-34** (C2) — kiểm Reply **mới** *trước* khi giao reply: nếu nó đang active thì
            // trả `Busy` mà **không** gửi reply nửa chừng (cùng hợp đồng "kiểm mọi tham số
            // trước khi giao" của KT-25). `recv` cũng kiểm, nhưng lúc đó reply đã gửi rồi.
            // Ngoại lệ hợp lệ: `recv_reply` **chính là** Reply vừa trả lời (`rep == dang`) —
            // `reply_with` ở dưới tiêu thụ `caller` trước, nên lúc `recv` chạy nó đã inactive.
            if rep != dang && self.reply_caller(rep)?.is_some() {
                return Err(KError::Busy);
            }
        }
        self.reply_with(tcb, reply, Some(msg))?;
        self.recv(tcb, ep, recv_reply)
    }

    /// **KT-25** — `NBSendRecv` (D2-53 **Q15a**): `NBSend` **không chặn** rồi `Recv` **chặn**.
    ///
    /// Nếu `NBSend` **không giao được** (hàng đầy ⇒ `Busy`, cap sai ⇒ lỗi…) thì **vẫn** `Recv`;
    /// message **không** bị kernel coi là mất (nó chưa hề vào đường IPC — `ipc_sent` không đổi,
    /// người gọi vẫn giữ message để thử lại). Lỗi của `NBSend` được **trả về** sau khi `Recv`
    /// đã chạy: `Ok(Some(err))`; `Ok(None)` = NBSend giao được. `Err` = lỗi cứng của `Recv`
    /// (chưa/không chặn được — vd cap nhận sai) sau khi NBSend đã chạy.
    pub fn nb_send_recv(
        &mut self,
        tcb: ObjId,
        send_ep: SlotRef,
        msg: Message,
        recv_ep: SlotRef,
        recv_reply: Option<SlotRef>,
    ) -> Result<Option<KError>, KError> {
        // Kiểm phía nhận **trước** để lỗi cấu hình không làm message gửi đi rồi mới báo lỗi.
        let _ = self.require_type(recv_ep, ObjType::Endpoint)?;
        if let Some(r) = recv_reply {
            let rep = self.require_type(r, ObjType::Reply)?;
            // **KT-34** (C2) — như `reply_recv`: Reply **mới** đang active ⇒ `Busy` **trước** khi
            // `NBSend`/`Recv` chạy (không gửi message rồi mới phát hiện cấu hình sai).
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

    /// **KT-25** — `NBSendWait` (D2-53 **Q15a**): `NBSend` **không chặn** rồi `Wait` trên
    /// Notification. `NBSend` không giao được ⇒ **vẫn** `Wait`; lỗi send trả về dạng
    /// `Ok(Some(err))` như [`Machine::nb_send_recv`]. Ngữ nghĩa `Wait` (word/badge/`Signal`) đầy
    /// đủ thuộc KT-27 — KT-25 dùng `BlockedOnNotification` + một waiter của mô hình K1b.
    pub fn nb_send_wait(
        &mut self,
        tcb: ObjId,
        send_ep: SlotRef,
        msg: Message,
        ntfn: SlotRef,
    ) -> Result<Option<KError>, KError> {
        let ntfn_obj = self.require_type(ntfn, ObjType::Notification)?;
        let send_err = self.nb_send(tcb, send_ep, msg).err();
        match self.wait_notification_obj(tcb, ntfn_obj) {
            Ok(()) => Ok(send_err),
            Err(e) => Err(e),
        }
    }

    /// **KT-64a (D2-89 + D2-90)** — finalise IPC **body** trước khi trả về free-list: đánh thức
    /// **mọi** TCB đang chờ object sắp chết bằng `Cancelled` + `Ready` (anti-hang), đúng chuẩn
    /// mà core đã cài từ KT-63. Ba loại chờ trên object:
    ///
    /// - `BlockedOnRecv{ep}` — Recv waiter của Endpoint (`Endpoint.recv`);
    /// - `BlockedOnNotification{ntfn}` — waiter của Notification;
    /// - `BlockedOnSend{ep, ..}` — **sender đang xếp hàng** (`Endpoint.send`). D2-90: message của
    ///   nó bị huỷ **có đếm** (`ipc_dropped`, đếm ở vòng trước của [`Machine::revoke_children`]);
    ///   ở đây chỉ đánh thức + trạng thái, **không** đụng hàng (body sắp bị `free`).
    ///
    /// Caller `BlockedOnReply` đã đúng từ KT-25 (`cancel_reply`), nên không nằm ở đây. Chỉ đánh
    /// thức TCB còn **đúng** trạng thái trỏ object này: TCB đã `Dead`/đã được đánh thức, hay
    /// sender **không chặn** (`NBSend` xếp hàng nhưng vẫn `Ready`), đều là no-op.
    ///
    /// **Trước KT-64a** sim để cả ba **treo** trên object đã chết (lỗi mô hình — D2-89); hàm này
    /// xoá lệch đó nên parity KT-63/KT-64 không còn phải loại trừ đường finalise-có-waiter.
    pub(super) fn finalise_ipc_waiters(&mut self, obj: ObjId) {
        match self.objects[obj as usize - 1].ty {
            ObjType::Endpoint => {
                let (recv, senders) = match self.body(obj, |k| match k {
                    Kind::Endpoint(e) => Ok((
                        e.recv.map(|(t, _)| t),
                        e.send.iter().map(|q| q.sender).collect::<Vec<_>>(),
                    )),
                    _ => Err(KError::WrongObjectType),
                }) {
                    Ok(v) => v,
                    Err(_) => return,
                };
                if let Some(w) = recv {
                    self.wake_cancelled(
                        w,
                        |st| matches!(st, TcbState::BlockedOnRecv { ep, .. } if *ep == obj),
                    );
                }
                for s in senders {
                    self.wake_cancelled(
                        s,
                        |st| matches!(st, TcbState::BlockedOnSend { ep, .. } if *ep == obj),
                    );
                }
            }
            ObjType::Notification => {
                let waiter = match self.body(obj, |k| match k {
                    Kind::Notification(n) => Ok(n.waiter),
                    _ => Err(KError::WrongObjectType),
                }) {
                    Ok(v) => v,
                    Err(_) => return,
                };
                if let Some(w) = waiter {
                    self.wake_cancelled(
                        w,
                        |st| matches!(st, TcbState::BlockedOnNotification { ntfn } if *ntfn == obj),
                    );
                }
            }
            _ => {}
        }
    }
}
