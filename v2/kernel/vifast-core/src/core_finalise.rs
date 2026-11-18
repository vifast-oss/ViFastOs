// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-63 — **finalise IPC** khi `Revoke`/`Delete`: huỷ `Reply.caller` (caller `Cancelled +
//! Ready`), đánh thức Recv/Notification waiter của object bị finalise (chống treo), gỡ Reply
//! cap đã chết khỏi waiter, và đếm message bị huỷ (`ipc_dropped`) — port ngữ nghĩa sim
//! (`cancel_reply`/`clear_dead_reply_in_waiters`/`purge_dead_refs`), tách khỏi `core_ipc.rs`
//! để giữ R1 ≤ 1.500 dòng.
//!
//! **KT-64a (D2-89 + D2-90)** — **một nguồn sự thật**: sim đã sửa theo cùng chuẩn (KT-64a) nên
//! **không còn** khác biệt có chủ đích ở đường này. Finalise Endpoint đánh thức **cả** Recv
//! waiter **và** mọi sender `BlockedOnSend` đang xếp hàng trên Endpoint đó (`Cancelled +
//! Ready`); finalise Notification đánh thức waiter. Message bị huỷ đếm vào `ipc_dropped`
//! (D2-90: drop **có đếm**). Parity `kt60_core_parity.rs` phủ trọn ba loại chờ
//! (`kt63_parity_revoke_finalise_caller_va_sender`, `kt64a_parity_finalise_recv_waiter`,
//! `kt64a_parity_finalise_notification_waiter`).

use crate::error::{KError, ObjId};
use crate::machine::Machine;
use crate::tcb::TcbState;

impl Machine {
    // ------------------------------------------------- finalise IPC (KT-63)

    /// **KT-63** — `Reply` bị xoá/revoke khi caller đang `BlockedOnReply` (D2-53 **Q8**):
    /// lấy `Reply.caller`, đặt lỗi IPC `Cancelled` cho caller rồi đánh thức caller `Ready`
    /// (như `cancel_reply` của sim). Không deadlock, `Reply.caller == None` sau khi huỷ; đi
    /// qua **một** cửa [`Machine::transition_tcb`] ⇒ hook KT-21 tự trả donation (không rò SC).
    /// Trả `Some(caller)` nếu đã đánh thức ai. Idempotent (`caller = None` sau lần đầu).
    pub(crate) fn cancel_reply_object(&mut self, reply_obj: ObjId) -> Option<ObjId> {
        let caller = match self.reply_data_mut(reply_obj) {
            Ok(r) => r.caller.take(),
            Err(_) => return None,
        };
        if let Some(c) = caller {
            let _ = self.set_last_ipc_error(c, KError::Cancelled);
            if matches!(
                self.tcb_state(c),
                Ok(TcbState::BlockedOnReply { reply }) if reply == reply_obj
            ) {
                let _ = self.wake_from(c);
            }
        }
        caller
    }

    /// **KT-63** — gỡ **Reply cap đã chết** khỏi Recv waiter của mọi Endpoint (như sim): waiter
    /// vẫn chờ nhưng coi như **không cấp Reply** ⇒ `Call` tới đi nhánh Q7(b) (message vẫn giao
    /// + caller `Cancelled`) thay vì lỗi cứng `InvalidCapability`.
    pub(crate) fn clear_dead_reply_in_waiters(&mut self) {
        for i in 0..self.n_endpoints {
            let Some((tcb, Some(cap))) = self.endpoints[i].recv else {
                continue;
            };
            let alive = self
                .caps
                .get(cap as usize - 1)
                .map(|c| c.alive)
                .unwrap_or(false);
            if !alive {
                self.endpoints[i].recv = Some((tcb, None));
            }
        }
    }

    /// **KT-63 / KT-64a** — finalise **body Endpoint** trước khi trả free-list: đánh thức Recv
    /// waiter **và mọi sender `BlockedOnSend` đang xếp hàng** (`Cancelled + Ready` — anti-hang,
    /// D2-89/D2-90) và đếm message người dùng bị huỷ vào `ipc_dropped` (bất biến bảo toàn
    /// message, D2-90: drop **có đếm**).
    ///
    /// **KT-64a**: trước đây core giữ hành vi cũ của sim (sender **treo** — nợ D2-90 tường
    /// minh); nay sim đã theo chuẩn nên core đóng nốt nợ: cả ba loại chờ trên object bị finalise
    /// đều `Cancelled + Ready`. Không phải "nới" ngữ nghĩa — đây **là** chuẩn D2-89/D2-90.
    pub(crate) fn finalise_endpoint_body(&mut self, ep: ObjId) {
        let (recv, senders, n_senders, dropped) = match self.endpoint_data(ep) {
            Ok(e) => {
                let dropped = e
                    .send
                    .iter()
                    .filter(|mi| self.ipc_msgs[*mi as usize].msg.is_some())
                    .count();
                // Chỉ **sender đang chờ giao** (`blocking`) mới cần đánh thức; `NBSend` xếp hàng
                // nhưng vẫn `Ready` ⇒ không phải waiter.
                let mut senders = [0u32; crate::objstate::ENDPOINT_QUEUE_CAP];
                let mut n = 0usize;
                for mi in e.send.iter() {
                    let q = &self.ipc_msgs[mi as usize];
                    if q.blocking && !senders[..n].contains(&q.sender) {
                        senders[n] = q.sender;
                        n += 1;
                    }
                }
                (e.recv, senders, n, dropped)
            }
            Err(_) => return,
        };
        self.ipc_dropped += dropped as u64;
        if let Some((w, _)) = recv {
            if matches!(self.tcb_state(w), Ok(TcbState::BlockedOnRecv { ep: e2, .. }) if e2 == ep) {
                let _ = self.set_last_ipc_error(w, KError::Cancelled);
                let _ = self.wake_from(w);
            }
        }
        // **D2-90** — sender bị finalise: `Cancelled + Ready`, không treo trên Endpoint đã chết.
        for &s in &senders[..n_senders] {
            if matches!(self.tcb_state(s), Ok(TcbState::BlockedOnSend { ep: e2, .. }) if e2 == ep) {
                let _ = self.set_last_ipc_error(s, KError::Cancelled);
                let _ = self.wake_from(s);
            }
        }
    }

    /// **KT-63** — finalise **body Notification**: đánh thức waiter (`Cancelled + Ready`) trước
    /// khi body về free-list — cùng lý do chống treo với [`Machine::finalise_endpoint_body`].
    pub(crate) fn finalise_notification_body(&mut self, ntfn: ObjId) {
        let waiter = match self.notification_data(ntfn) {
            Ok(n) => n.waiter,
            Err(_) => return,
        };
        if let Some(w) = waiter {
            if matches!(
                self.tcb_state(w),
                Ok(TcbState::BlockedOnNotification { ntfn: n2 }) if n2 == ntfn
            ) {
                let _ = self.set_last_ipc_error(w, KError::Cancelled);
                let _ = self.wake_from(w);
            }
        }
    }
}
