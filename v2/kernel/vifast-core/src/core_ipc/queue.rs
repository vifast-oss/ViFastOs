// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `core_ipc/queue.rs` — **truy vấn hàng đợi/Endpoint** của [`Machine`] (KT-60):
//! các bộ đếm IPC, `endpoint_*`, iterator `QueuedIter`/`MsgIds`/`SendItems`; port từ oracle.

use super::*;

impl<O: CoreObserver> Machine<O> {
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
            for mi in e.send.iter(&self.ipc_msgs) {
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

    /// **KT-309** — số **extra cap** caller khai qua IPC buffer (tổng `n` của mọi message).
    pub fn extra_caps_sent(&self) -> u64 {
        self.extra_caps_sent
    }

    /// **KT-309** — số **extra cap** cài được vào CSpace receiver.
    pub fn extra_caps_installed(&self) -> u64 {
        self.extra_caps_installed
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
    pub fn endpoint_queued(&self, ep: ObjId) -> Result<QueuedIter<'_, O>, KError> {
        let i = self.body_slot(ep, ObjType::Endpoint, BODY_ENDPOINT)?;
        let mut slots = [0u16; ENDPOINT_QUEUE_CAP];
        let mut n = 0usize;
        for mi in self.endpoints[i].send.iter(&self.ipc_msgs) {
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

impl<O: CoreObserver> Machine<O> {
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
        Ok(self.endpoint_data(ep)?.model.last_mr)
    }

    /// `msg_id` các message người dùng đang chờ (FIFO; bỏ fault).
    pub fn endpoint_msg_ids(&self, ep: ObjId) -> Result<MsgIds<'_, O>, KError> {
        let i = self.body_slot(ep, ObjType::Endpoint, BODY_ENDPOINT)?;
        Ok(MsgIds {
            m: self,
            ep: i,
            idx: 0,
        })
    }

    /// `(sender, fault)` các mục trong hàng gửi Endpoint (FIFO).
    pub fn endpoint_send_items(&self, ep: ObjId) -> Result<SendItems<'_, O>, KError> {
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
pub struct QueuedIter<'a, O: CoreObserver = TestObserver> {
    m: &'a Machine<O>,
    slots: [u16; ENDPOINT_QUEUE_CAP],
    n_slots: usize,
    idx: usize,
}

impl<O: CoreObserver> Iterator for QueuedIter<'_, O> {
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
pub struct MsgIds<'a, O: CoreObserver = TestObserver> {
    m: &'a Machine<O>,
    ep: usize,
    idx: usize,
}

impl<O: CoreObserver> Iterator for MsgIds<'_, O> {
    type Item = u64;

    fn next(&mut self) -> Option<u64> {
        let mut slots = [0u16; ENDPOINT_QUEUE_CAP];
        let mut n = 0usize;
        for mi in self.m.endpoints[self.ep].send.iter(&self.m.ipc_msgs) {
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
pub struct SendItems<'a, O: CoreObserver = TestObserver> {
    m: &'a Machine<O>,
    ep: usize,
    idx: usize,
}

impl<O: CoreObserver> Iterator for SendItems<'_, O> {
    type Item = (ObjId, Option<FaultKind>);

    fn next(&mut self) -> Option<(ObjId, Option<FaultKind>)> {
        let mut slots = [0u16; ENDPOINT_QUEUE_CAP];
        let mut n = 0usize;
        for mi in self.m.endpoints[self.ep].send.iter(&self.m.ipc_msgs) {
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

impl<O: CoreObserver> Machine<O> {
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
