// SPDX-License-Identifier: Apache-2.0 OR MIT
//! KT-60 — bất biến IPC/Reply/Notification/WaitAddr/fault của core, port từ
//! `Machine::{check_ipc, check_reply_links, check_notifications, check_waitaddr, check_fault}`
//! của sim (`v2/sim/vifast-sim/src/kobj.rs`, oracle), trả [`CheckFailure`] thay `String`.
//!
//! Chạy sau **mỗi** public op qua `sched_after_op` (cùng chỗ `check_sched`/`check_mailboxes`);
//! mỗi hàm có **cửa sổ nhanh** (chưa từng dùng IPC/không có waiter…) để L1 1.000 seed của
//! cổng không chậm thêm.

use crate::error::{CheckFailure, ObjId, ObjType};
use crate::machine::{Machine, BODY_ENDPOINT, BODY_NOTIFICATION, BODY_REPLY, MAX_QUEUED_MESSAGES};
use crate::objstate::{waitaddr_bucket, ENDPOINT_QUEUE_CAP, NODE_NONE, WAITADDR_BUCKETS};
use crate::tcb::TcbState;

impl Machine {
    /// Bất biến **bảo toàn message** + hàng gửi Endpoint (port `check_ipc` + phần Endpoint của
    /// `check_mailboxes`).
    pub fn check_ipc(&self) -> Result<(), CheckFailure> {
        if self.ipc_sent == 0 {
            if self.ipc_delivered == 0 && self.ipc_dropped == 0 && self.transfer_calls == 0 {
                return Ok(());
            }
            return Err(CheckFailure::one("ipc dem lech khi sent=0", 0));
        }
        let mut ids = [0u64; MAX_QUEUED_MESSAGES];
        let mut n_ids = 0usize;
        let mut pending = 0u64;
        for i in 0..self.n_endpoints {
            let e = &self.endpoints[i];
            if !e.send.invariant() {
                return Err(CheckFailure::one(
                    "hang Endpoint vi pham invariant",
                    i as u32,
                ));
            }
            if e.send.len() > ENDPOINT_QUEUE_CAP {
                return Err(CheckFailure::one("hang Endpoint qua suc chua", i as u32));
            }
            if e.recv.is_some() && !e.send.is_empty() {
                return Err(CheckFailure::one(
                    "co message va Recv waiter cung luc",
                    i as u32,
                ));
            }
            for mi in e.send.iter() {
                let q = &self.ipc_msgs[mi as usize];
                if !q.alive {
                    return Err(CheckFailure::one(
                        "hang Endpoint tro message da chet",
                        mi as u32,
                    ));
                }
                match q.msg {
                    Some(m) => {
                        if m.msg_id == 0 || m.msg_id > self.ipc_sent {
                            return Err(CheckFailure::one(
                                "msg_id ngoai dai 1..=ipc_sent",
                                m.msg_id as u32,
                            ));
                        }
                        if ids[..n_ids].contains(&m.msg_id) {
                            return Err(CheckFailure::one("msg_id trung", m.msg_id as u32));
                        }
                        if n_ids < MAX_QUEUED_MESSAGES {
                            ids[n_ids] = m.msg_id;
                            n_ids += 1;
                        }
                        pending += 1;
                    }
                    None => {
                        if q.fault.is_none() {
                            return Err(CheckFailure::one(
                                "message khong noi dung va khong phai fault",
                                q.sender,
                            ));
                        }
                    }
                }
                // **KT-342b** — message **chặn** ⇒ sender phải **còn sống** và đúng
                // `BlockedOnSend{ep, call, fault}`. Message **không chặn** (`NBSend` đã trả về)
                // thuộc về bên nhận: sender **được phép đã chết** (đó là ngữ nghĩa bảo toàn
                // message; trước KT-342b bất biến này đòi sender sống nên buộc `unlink_tcb_queues`
                // phải huỷ message `NBSend` — tức che mất lỗi mất message).
                if q.blocking && !self.object_alive(q.sender) {
                    return Err(CheckFailure::one("sender da chet", q.sender));
                }
                let ep = self.endpoint_object(i as u32);
                if q.blocking {
                    match self.tcb_state(q.sender) {
                        Ok(TcbState::BlockedOnSend {
                            ep: e2,
                            call,
                            fault,
                        }) if e2 == ep && call == q.call && fault == q.fault => {}
                        _ => {
                            return Err(CheckFailure::two(
                                "sender chan nhung trang thai khong khop",
                                q.sender,
                                mi as u32,
                            ));
                        }
                    }
                }
            }
            if let Some((tcb, _)) = e.recv {
                if !self.object_alive(tcb) {
                    return Err(CheckFailure::one("Recv waiter da chet", tcb));
                }
                match self.tcb_state(tcb) {
                    Ok(TcbState::BlockedOnRecv { ep, .. })
                        if ep == self.endpoint_object(i as u32) => {}
                    _ => {
                        return Err(CheckFailure::one(
                            "Recv waiter khong o BlockedOnRecv dung Endpoint",
                            tcb,
                        ));
                    }
                }
            }
        }
        for tcb in self.live_tcb_ids() {
            let t = match self.tcb_data(tcb) {
                Ok(t) => t,
                Err(_) => continue,
            };
            let Some(id) = t.ipc.msg_id else { continue };
            if id == 0 || id > self.ipc_sent {
                return Err(CheckFailure::one("TCB: msg_id ngoai dai", tcb));
            }
            if ids[..n_ids].contains(&id) {
                return Err(CheckFailure::one(
                    "msg_id vua o vung nhan vua trong hang",
                    tcb,
                ));
            }
        }
        if self.transfer_calls != self.ipc_delivered {
            return Err(CheckFailure::two(
                "transfer_calls != ipc_delivered",
                self.transfer_calls as u32,
                self.ipc_delivered as u32,
            ));
        }
        if self.ipc_delivered > self.ipc_sent {
            return Err(CheckFailure::two(
                "ipc_delivered > ipc_sent",
                self.ipc_delivered as u32,
                self.ipc_sent as u32,
            ));
        }
        if self.ipc_sent != self.ipc_delivered + pending + self.ipc_dropped {
            return Err(CheckFailure::two(
                "message mat/them",
                self.ipc_sent as u32,
                (self.ipc_delivered + pending + self.ipc_dropped) as u32,
            ));
        }
        Ok(())
    }

    /// Bất biến liên kết **Reply** (C2, KT-34): `caller ⇔ BlockedOnReply`.
    pub fn check_reply_links(&self) -> Result<(), CheckFailure> {
        for i in 0..self.n_replies {
            let Some(rep) = self.reply_object(i as u32) else {
                continue;
            };
            let Some(t) = self.replies[i].caller else {
                continue;
            };
            if !self.object_alive(t) {
                return Err(CheckFailure::two("Reply caller da chet", rep, t));
            }
            match self.tcb_state(t) {
                Ok(TcbState::BlockedOnReply { reply }) if reply == rep => {}
                _ => {
                    return Err(CheckFailure::two(
                        "Reply caller khong o BlockedOnReply tren Reply nay",
                        rep,
                        t,
                    ));
                }
            }
        }
        for tcb in self.live_tcb_ids() {
            let Ok(TcbState::BlockedOnReply { reply }) = self.tcb_state(tcb) else {
                continue;
            };
            if !self.object_alive(reply) || self.object_type(reply) != Some(ObjType::Reply) {
                return Err(CheckFailure::two(
                    "BlockedOnReply tren Reply chet/sai kieu",
                    tcb,
                    reply,
                ));
            }
            if self
                .reply_caller(reply)
                .map_err(|_| CheckFailure::two("Reply caller loi", tcb, reply))?
                != Some(tcb)
            {
                return Err(CheckFailure::two(
                    "BlockedOnReply nhung Reply.caller lech",
                    tcb,
                    reply,
                ));
            }
        }
        Ok(())
    }

    /// Bất biến Notification (KT-27): `badge == word`, waiter đúng trạng thái, bound 1:1.
    pub fn check_notifications(&self) -> Result<(), CheckFailure> {
        let mut bound_by = [0u32; 257];
        for tcb in self.live_tcb_ids() {
            let t = self
                .tcb_data(tcb)
                .map_err(|_| CheckFailure::one("TCB khong doc duoc", tcb))?;
            let Some(ntfn) = t.bound_notification else {
                continue;
            };
            if !self.object_alive(ntfn) || self.object_type(ntfn) != Some(ObjType::Notification) {
                return Err(CheckFailure::two(
                    "bound_notification chet/sai kieu",
                    tcb,
                    ntfn,
                ));
            }
            let k = ntfn as usize;
            if k < bound_by.len() {
                if bound_by[k] != 0 {
                    return Err(CheckFailure::two("Notification bind hai TCB", ntfn, tcb));
                }
                bound_by[k] = tcb;
            }
        }
        for i in 0..self.n_notifications {
            let n = &self.notifications[i];
            let Some(ntfn) = self.notification_object(i as u32) else {
                continue;
            };
            if n.badge != n.word {
                return Err(CheckFailure::two(
                    "Notification badge != word",
                    ntfn,
                    n.word as u32,
                ));
            }
            if let Some(t) = n.waiter {
                if !self.object_alive(t) {
                    return Err(CheckFailure::two("Notification waiter da chet", ntfn, t));
                }
                match self.tcb_state(t) {
                    Ok(TcbState::BlockedOnNotification { ntfn: n2 }) if n2 == ntfn => {}
                    _ => {
                        return Err(CheckFailure::two(
                            "Notification waiter khong dung trang thai",
                            ntfn,
                            t,
                        ));
                    }
                }
            }
            if let Some(sc) = n.bound_sc {
                if !self.object_alive(sc) || self.object_type(sc) != Some(ObjType::SchedContext) {
                    return Err(CheckFailure::two(
                        "Notification bound_sc chet/sai kieu",
                        ntfn,
                        sc,
                    ));
                }
                // **KT-64 (D2-92d)** — chiều **ngược**: SC phải trỏ lại **đúng** notification này
                // (cap còn sống, đúng object) ⇒ không còn "binding một chiều" mồ côi.
                let back_ok = self
                    .sc_data(sc)
                    .ok()
                    .and_then(|d| d.bound)
                    .and_then(|cap| self.cap(cap).ok().map(|c| (c.obj, c.alive)))
                    .map(|(obj, alive)| alive && obj == ntfn)
                    .unwrap_or(false);
                if !back_ok {
                    return Err(CheckFailure::two(
                        "Notification bound_sc nhung SC.bound khong tro lai",
                        ntfn,
                        sc,
                    ));
                }
            }
        }
        // **KT-64 (D2-92d)** — `ScData.bound` 1:1 hai chiều: mỗi SC sống có `bound` phải trỏ cap
        // Reply/Notification **còn sống**; nếu là Notification thì `bound_sc` phải trỏ lại SC đó.
        // Binding mồ côi (một chiều) ⇒ lỗi bất biến (đi kèm hook `delete`/`Revoke`).
        for idx in 0..self.n_objects {
            let meta = &self.objects[idx];
            if !meta.alive || meta.ty != ObjType::SchedContext {
                continue;
            }
            let sc = (idx + 1) as u32;
            let Some(cap) = self.sc_data(sc).ok().and_then(|d| d.bound) else {
                continue;
            };
            let Ok(c) = self.cap(cap) else {
                return Err(CheckFailure::one("SC.bound cap khong hop le", sc));
            };
            if !c.alive {
                return Err(CheckFailure::two("SC.bound cap da chet", sc, c.obj));
            }
            match c.ty {
                ObjType::Notification => {
                    if self.notification_bound_sc(c.obj).ok().flatten() != Some(sc) {
                        return Err(CheckFailure::two(
                            "SC.bound nhung Notification.bound_sc khong tro lai",
                            sc,
                            c.obj,
                        ));
                    }
                }
                ObjType::Reply => {}
                _ => {
                    return Err(CheckFailure::two(
                        "SC.bound sai kieu (phai Reply/Notification)",
                        sc,
                        c.obj,
                    ));
                }
            }
        }
        Ok(())
    }

    /// Bất biến WaitOnAddress (KT-28): nút đúng bucket, TCB sống/không treo/đúng trạng thái,
    /// mỗi TCB tối đa một nút.
    pub fn check_waitaddr(&self) -> Result<(), CheckFailure> {
        if self.address_waiters_len() == 0 {
            return Ok(());
        }
        let mut seen = [false; 257];
        for b in 0..WAITADDR_BUCKETS {
            let mut cur = self.wait_buckets[b];
            while cur != NODE_NONE {
                let node = &self.wait_nodes[cur as usize];
                if !node.alive {
                    return Err(CheckFailure::one(
                        "nut waiter da chet trong bucket",
                        cur as u32,
                    ));
                }
                if waitaddr_bucket(node.frame, node.offset) != b {
                    return Err(CheckFailure::one("nut waiter sai bucket", node.tcb));
                }
                if self.tcb_state(node.tcb)
                    != Ok(TcbState::BlockedOnAddress {
                        frame: node.frame,
                        offset: node.offset,
                    })
                {
                    return Err(CheckFailure::one(
                        "nut waiter khong dung trang thai",
                        node.tcb,
                    ));
                }
                if self
                    .tcb_info_obj(node.tcb)
                    .map_err(|_| CheckFailure::one("TCB info loi", node.tcb))?
                    .suspended
                {
                    return Err(CheckFailure::one("nut waiter cua TCB suspended", node.tcb));
                }
                if (node.tcb as usize) < seen.len() {
                    if seen[node.tcb as usize] {
                        return Err(CheckFailure::one("TCB co hai nut waiter", node.tcb));
                    }
                    seen[node.tcb as usize] = true;
                }
                cur = node.next;
            }
        }
        Ok(())
    }

    /// Bất biến **fault không mất/không nhân bản** (KT-41, phần fault của Endpoint).
    pub fn check_fault(&self) -> Result<(), CheckFailure> {
        if self.fault_events == 0 {
            return Ok(());
        }
        // 1. Message fault trong hàng phải khớp trạng thái người gửi.
        for i in 0..self.n_endpoints {
            let ep = self.endpoint_object(i as u32);
            for mi in self.endpoints[i].send.iter() {
                let q = &self.ipc_msgs[mi as usize];
                let Some(kind) = q.fault else { continue };
                match self.tcb_state(q.sender) {
                    Ok(TcbState::BlockedOnSend {
                        ep: e2,
                        call,
                        fault,
                    }) if e2 == ep && call == q.call && fault == Some(kind) => {}
                    _ => {
                        return Err(CheckFailure::two(
                            "message fault khong khop trang thai sender",
                            q.sender,
                            ep,
                        ));
                    }
                }
            }
        }
        // 2. Mỗi TCB `BlockedOnSend` mang fault có **đúng một** message fault khớp.
        for tcb in self.live_tcb_ids() {
            let Ok(TcbState::BlockedOnSend { ep, call, fault }) = self.tcb_state(tcb) else {
                continue;
            };
            let Some(kind) = fault else { continue };
            let mut found = 0usize;
            if let Ok(e) = self.endpoint_data(ep) {
                for mi in e.send.iter() {
                    let q = &self.ipc_msgs[mi as usize];
                    if q.sender == tcb && q.fault == Some(kind) && q.call == call {
                        found += 1;
                    }
                }
            }
            if found != 1 {
                return Err(CheckFailure::two(
                    "so message fault khong dung 1",
                    tcb,
                    found as u32,
                ));
            }
        }
        Ok(())
    }

    // ---------------------------------------------------------------- tiện ích nội bộ

    fn reply_object(&self, i: u32) -> Option<ObjId> {
        for j in 0..self.n_objects {
            let m = &self.objects[j];
            if m.alive
                && m.ty == ObjType::Reply
                && m.body.kind == BODY_REPLY
                && m.body.idx as u32 == i
            {
                return Some((j as u32) + 1);
            }
        }
        None
    }

    fn notification_object(&self, i: u32) -> Option<ObjId> {
        for j in 0..self.n_objects {
            let m = &self.objects[j];
            if m.alive
                && m.ty == ObjType::Notification
                && m.body.kind == BODY_NOTIFICATION
                && m.body.idx as u32 == i
            {
                return Some((j as u32) + 1);
            }
        }
        None
    }

    fn live_tcb_ids(&self) -> TcbIds<'_> {
        TcbIds { m: self, idx: 0 }
    }

    /// `ObjId` của Endpoint body thứ `i` — dùng ở `check_ipc` (map qua `Meta.body.idx`).
    fn endpoint_object(&self, i: u32) -> ObjId {
        for j in 0..self.n_objects {
            let m = &self.objects[j];
            if m.alive
                && m.ty == ObjType::Endpoint
                && m.body.kind == BODY_ENDPOINT
                && m.body.idx as u32 == i
            {
                return (j as u32) + 1;
            }
        }
        0
    }
}

/// Iterator `ObjId` TCB sống theo thứ tự cấp.
#[derive(Debug, Clone)]
pub struct TcbIds<'a> {
    m: &'a Machine,
    idx: usize,
}

impl Iterator for TcbIds<'_> {
    type Item = ObjId;

    fn next(&mut self) -> Option<ObjId> {
        while self.idx < self.m.n_objects {
            let i = self.idx;
            self.idx += 1;
            let m = &self.m.objects[i];
            if m.alive && m.ty == ObjType::Tcb {
                return Some((i as u32) + 1);
            }
        }
        None
    }
}
