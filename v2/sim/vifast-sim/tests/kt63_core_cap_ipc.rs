// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-63 — test **core-only** cho chuyển cap qua IPC + finalise IPC khi Revoke/Delete.
//!
//! Đây là phần *hành vi core* không so trực tiếp được với sim (sim không định nghĩa đường
//! đánh thức Recv waiter khi object nhận bị finalise) — parity sim↔core nằm ở
//! `kt60_core_parity.rs` (mở rộng KT-63). Tệp này pin:
//!
//! 1. receive window + `plan_cap_transfer`: 4 cap cài đủ, **bỏ `RIGHT_GRANT`**, giữ badge;
//!    thiếu GRANT/ô bận ⇒ thất bại **mềm**; window chưa bind/cap nguồn chết/5 cap ⇒ lỗi
//!    **cứng trước khi giao** (message không vào đường IPC); unwrap badge không chiếm window.
//! 2. finalise IPC khi `Revoke`/`Delete`: caller `BlockedOnReply` ⇒ `Cancelled + Ready`;
//!    Recv/Notification waiter của object bị finalise ⇒ `Cancelled + Ready` (chống treo);
//!    hàng Endpoint bị huỷ và `ipc_dropped` cộng đủ (bất biến bảo toàn message).

use kc::{
    KError, Machine, Message, MessageInfo, ObjId, ObjType, SlotRef, TcbState, RIGHTS_ALL,
    RIGHT_GRANT,
};
use vifast_core as kc;

fn slot(c: u32, s: usize) -> SlotRef {
    SlotRef::new(c, s)
}

fn msg(label: u32, words: &[u64]) -> Message {
    let info = MessageInfo::new(label, 0, false, 0, words.len() as u8).expect("info hợp lệ");
    Message::new(info, words).expect("message hợp lệ")
}

fn msg_caps(label: u32, words: &[u64], caps: &[kc::CapId]) -> Message {
    Message::with_cap_ids(label, words, caps).expect("message cap hợp lệ")
}

/// Rig core: 1 CNode 64 slot; 2 Endpoint + Reply + Notification **retype từ Untyped**
/// (revoke được); 3 TCB **kernel object** (sống qua revoke); 4 cap nguồn, 1 cap thiếu
/// GRANT, 1 cap badge; window t1 = slot 40..43, t2 = 44..47.
struct Rig {
    m: Machine,
    c0: ObjId,
    t: [ObjId; 3],
    ts: [SlotRef; 3],
    ep: [ObjId; 2],
    eps: [SlotRef; 2],
    rep: ObjId,
    reps: SlotRef,
    src: [kc::CapId; 4],
    nogrant: kc::CapId,
    badge: kc::CapId,
    win: [[SlotRef; 4]; 2],
}

const SRC: [usize; 4] = [30, 31, 32, 33];
const NOGRANT: usize = 34;
const BADGE: usize = 35;
const WIN: [[usize; 4]; 2] = [[40, 41, 42, 43], [44, 45, 46, 47]];

fn build() -> Rig {
    let mut m = Machine::new();
    let u = m.add_untyped(0x8000_0000, 1 << 18);
    let c0 = m.add_cnode(64);
    let uc = m.root_cap(u);
    m.put_cap(c0, 0, uc).expect("cap Untyped");
    let mk = |m: &mut Machine, ty: ObjType, s: usize| -> ObjId {
        let cap = m.retype(slot(c0, 0), ty, 0, 0).expect("retype");
        let obj = m.cap_obj(cap).expect("cap_obj");
        m.put_cap(c0, s, cap).expect("put_cap");
        obj
    };
    let ep0 = mk(&mut m, ObjType::Endpoint, 1);
    let ep1 = mk(&mut m, ObjType::Endpoint, 2);
    let rep = mk(&mut m, ObjType::Reply, 3);
    let mut t = [0u32; 3];
    let mut ts = [slot(c0, 10); 3];
    for i in 0..3 {
        let obj = m.add_kernel_object(ObjType::Tcb);
        let cap = m.root_cap(obj);
        m.put_cap(c0, 10 + i, cap).expect("cap TCB");
        t[i] = obj;
        ts[i] = slot(c0, 10 + i);
        m.start_tcb(obj).expect("Ready");
    }
    let mut src = [0u32; 4];
    for (i, s) in SRC.iter().enumerate() {
        src[i] = m.copy(slot(c0, 2), slot(c0, *s)).expect("copy cap nguồn");
    }
    let nogrant = m
        .copy_rights(
            slot(c0, 2),
            slot(c0, NOGRANT),
            Some(kc::RIGHT_READ | kc::RIGHT_WRITE),
        )
        .expect("cap thiếu GRANT");
    let badge = m
        .mint(slot(c0, 1), slot(c0, BADGE), RIGHTS_ALL, Some(0xA5A5))
        .expect("cap badge ep0");
    let win = [
        [
            slot(c0, WIN[0][0]),
            slot(c0, WIN[0][1]),
            slot(c0, WIN[0][2]),
            slot(c0, WIN[0][3]),
        ],
        [
            slot(c0, WIN[1][0]),
            slot(c0, WIN[1][1]),
            slot(c0, WIN[1][2]),
            slot(c0, WIN[1][3]),
        ],
    ];
    Rig {
        m,
        c0,
        t,
        ts,
        ep: [ep0, ep1],
        eps: [slot(c0, 1), slot(c0, 2)],
        rep,
        reps: slot(c0, 3),
        src,
        nogrant,
        badge,
        win,
    }
}

impl Rig {
    fn st(&self, i: usize) -> TcbState {
        self.m.tcb_state(self.t[i]).expect("tcb_state")
    }
    fn info(&self, i: usize) -> Option<MessageInfo> {
        self.m.tcb_ipc_info(self.t[i]).expect("ipc_info")
    }
    fn win_cap(&self, i: usize, k: usize) -> Option<kc::CapId> {
        self.m
            .cap_at(self.win[i][k].cnode, self.win[i][k].slot)
            .ok()
    }
    fn kiem(&self, buoc: &str) {
        assert!(
            self.m.check_ipc().is_ok(),
            "{buoc}: check_ipc: {:?}",
            self.m.check_ipc()
        );
        assert!(
            self.m.check_reply_links().is_ok(),
            "{buoc}: check_reply_links: {:?}",
            self.m.check_reply_links()
        );
        assert!(
            self.m.check_notifications().is_ok(),
            "{buoc}: check_notifications: {:?}",
            self.m.check_notifications()
        );
        assert!(
            self.m.check_caps().is_ok(),
            "{buoc}: check_caps: {:?}",
            self.m.check_caps()
        );
        assert!(
            self.m.check_all_bytes().is_ok(),
            "{buoc}: check_all_bytes: {:?}",
            self.m.check_all_bytes()
        );
    }
}

// =============================================================================================
// 1. Chuyển cap qua IPC — 4 cap, bỏ GRANT, badge, unwrap, thất bại mềm, lỗi cứng.
// =============================================================================================

#[test]
fn kt63_core_chuyen_bon_cap_bo_grant_va_giu_badge() {
    let mut r = build();
    r.m.tcb_set_recv_window(r.ts[1], r.win[0]).expect("bind");
    assert_eq!(r.m.tcb_recv_window(r.t[1]).expect("window"), Some(r.win[0]));
    r.m.recv(r.t[1], r.eps[0], Some(r.reps)).expect("Recv");
    let caps = r.src;
    r.m.send(r.t[0], r.eps[0], msg_caps(7, &[1, 2], &caps))
        .expect("Send 4 cap");
    r.kiem("4 cap");
    assert_eq!(r.st(1), TcbState::Ready, "waiter được đánh thức");
    let info = r.info(1).expect("đã nhận");
    assert_eq!(info.caps(), 4);
    assert_eq!(info.extra_caps_unwrapped(), 0);
    assert_eq!(info.length(), 2, "MR vẫn nguyên");
    for k in 0..4 {
        let cap = r.win_cap(0, k).expect("ô window phải có cap");
        let ci = r.m.cap_info(cap).expect("cap_info");
        assert_eq!(ci.ty, ObjType::Endpoint);
        assert_eq!(ci.rights & RIGHT_GRANT, 0, "bỏ RIGHT_GRANT");
        assert_eq!(ci.rights, RIGHTS_ALL & !RIGHT_GRANT);
    }
    assert_eq!(r.m.cap_transfer_stats(), (4, 0, 0));
    r.kiem("4 cap sau");
}

#[test]
fn kt63_core_thieu_grant_va_o_ban_that_bai_mem() {
    let mut r = build();
    r.m.tcb_set_recv_window(r.ts[1], r.win[0]).expect("bind");
    r.m.recv(r.t[1], r.eps[0], None).expect("Recv");
    let caps = [r.src[0], r.nogrant, r.src[2], r.src[3]];
    r.m.send(r.t[0], r.eps[0], msg_caps(8, &[9], &caps))
        .expect("thất bại mềm ⇒ message vẫn giao");
    r.kiem("thiếu GRANT");
    let info = r.info(1).expect("đã nhận");
    assert_eq!(info.caps(), 3, "caps = số cap cài được");
    assert_eq!(r.m.cap_transfer_failed(), 1);
    assert_eq!(r.m.cap_transfer_installed(), 3);
    assert!(r.win_cap(0, 1).is_none(), "ô của cap thiếu GRANT để trống");
    assert_eq!(r.m.ipc_delivered(), 1, "message vẫn giao");

    // Ô đích đã có cap (chiếm sẵn ô 2) ⇒ cap đó thất bại mềm, các cap khác vẫn cài.
    let mut r = build();
    let spare = r.m.root_cap(r.ep[1]);
    r.m.put_cap(r.win[0][1].cnode, r.win[0][1].slot, spare)
        .expect("chiếm ô window");
    r.m.tcb_set_recv_window(r.ts[1], r.win[0]).expect("bind");
    r.m.recv(r.t[1], r.eps[0], None).expect("Recv");
    let caps = r.src;
    r.m.send(r.t[0], r.eps[0], msg_caps(9, &[], &caps))
        .expect("thất bại mềm");
    r.kiem("ô bận");
    assert_eq!(r.info(1).expect("đã nhận").caps(), 3);
    assert_eq!(r.m.cap_transfer_failed(), 1);
}

#[test]
fn kt63_core_window_chua_bind_va_cap_nguon_chet_loi_cung() {
    let mut r = build();
    // t2 chưa bind receive window.
    r.m.recv(r.t[2], r.eps[1], None).expect("Recv t2");
    let caps = r.src;
    assert_eq!(
        r.m.send(r.t[0], r.eps[1], msg_caps(15, &[], &caps)),
        Err(KError::InvalidArgument)
    );
    assert_eq!(r.m.ipc_sent(), 0, "message không vào đường IPC");
    r.kiem("window chưa bind");
    // Message **chỉ** mang cap unwrap ⇒ không cần window: badge OR, extra = 1.
    let badge_ep1 =
        r.m.mint(r.eps[1], slot(r.c0, 51), RIGHTS_ALL, Some(0x33))
            .expect("mint badge ep1");
    r.m.send(r.t[0], r.eps[1], msg_caps(16, &[], &[badge_ep1]))
        .expect("unwrap-only không cần window");
    assert_eq!(r.m.ipc_sent(), 1);
    let info = r.info(2).expect("đã nhận");
    assert_eq!(info.caps(), 0);
    assert_eq!(info.extra_caps_unwrapped(), 1);
    assert_eq!(r.m.tcb_ipc_badge(r.t[2]).expect("badge"), 0x33);
    r.kiem("unwrap-only");

    // Cap nguồn chết ⇒ lỗi cứng **trước** khi cấp msg_id.
    let mut r = build();
    let chet = r.m.copy(r.eps[1], slot(r.c0, 50)).expect("copy");
    r.m.delete(slot(r.c0, 50)).expect("delete cap nguồn");
    r.m.recv(r.t[1], r.eps[0], None).expect("Recv");
    assert_eq!(
        r.m.send(r.t[0], r.eps[0], msg_caps(10, &[1], &[chet])),
        Err(KError::InvalidCapability)
    );
    assert_eq!(r.m.ipc_sent(), 0);
    assert_eq!(r.m.ipc_pending(), 0);
    assert!(matches!(r.st(1), TcbState::BlockedOnRecv { .. }));
    r.kiem("cap chết");
}

#[test]
fn kt63_core_unwrap_badge_khong_chiem_window_va_5_cap_bi_tu_choi() {
    let mut r = build();
    r.m.tcb_set_recv_window(r.ts[1], r.win[0]).expect("bind");
    r.m.recv(r.t[1], r.eps[0], None).expect("Recv");
    // [cap badge tới chính ep0, cap thường] ⇒ 1 unwrap + 1 cài vào **ô index 1**.
    let caps = [r.badge, r.src[1]];
    r.m.send(r.t[0], r.eps[0], msg_caps(12, &[0xAB], &caps))
        .expect("Send unwrap");
    r.kiem("unwrap");
    let info = r.info(1).expect("đã nhận");
    assert_eq!(info.caps(), 1);
    assert_eq!(info.extra_caps_unwrapped(), 1);
    assert_eq!(r.m.tcb_ipc_badge(r.t[1]).expect("badge"), 0xA5A5);
    assert_eq!(r.m.cap_transfer_installed(), 1);
    assert_eq!(r.m.cap_unwrapped(), 1);
    assert!(r.win_cap(0, 0).is_none(), "unwrap không chiếm ô 0");
    assert!(r.win_cap(0, 1).is_some(), "cap thường cài ô 1");
    // 5 cap không biểu diễn được (spec max_caps = 4) — từ chối ngay khi dựng message.
    let nam = [r.src[0], r.src[1], r.src[2], r.src[3], r.badge];
    assert_eq!(
        Message::with_cap_ids(11, &[], &nam),
        Err(KError::InvalidArgument)
    );
    assert!(Message::with_cap_ids(11, &[], &nam[..4]).is_ok());
    r.kiem("5 cap");
}

#[test]
fn kt63_core_call_va_reply_mang_cap() {
    let mut r = build();
    r.m.tcb_set_recv_window(r.ts[1], r.win[0]).expect("bind");
    r.m.recv(r.t[1], r.eps[0], Some(r.reps))
        .expect("server Recv");
    // Call mang 2 cap ⇒ giao cho waiter, caller BlockedOnReply.
    let caps = [r.src[0], r.src[1]];
    r.m.call_msg(r.t[0], r.eps[0], msg_caps(20, &[0x11], &caps))
        .expect("Call 2 cap");
    r.kiem("Call 2 cap");
    assert_eq!(r.m.reply_caller(r.rep).expect("caller"), Some(r.t[0]));
    assert_eq!(r.info(1).expect("server nhận").caps(), 2);
    assert_eq!(r.st(0), TcbState::BlockedOnReply { reply: r.rep });
    // Reply mang 2 cap ⇒ caller nhận cap trong window của nó (bind t0).
    r.m.tcb_set_recv_window(r.ts[0], r.win[1]).expect("bind t0");
    let caps = [r.src[2], r.src[3]];
    r.m.reply_msg(r.t[1], r.reps, msg_caps(21, &[0x22], &caps))
        .expect("Reply 2 cap");
    r.kiem("Reply 2 cap");
    assert_eq!(r.st(0), TcbState::Ready);
    assert_eq!(r.info(0).expect("caller nhận").caps(), 2);
    assert_eq!(r.m.cap_transfer_stats(), (4, 0, 0));
}

// =============================================================================================
// 2. Finalise IPC khi Revoke/Delete.
// =============================================================================================

/// Dựng thêm một Untyped thứ hai với Endpoint/Reply/Notification **retype được** (slot 20..23),
/// trả về slot cap Untyped (để Revoke cả cây).
fn them_cay(m: &mut Machine, c0: ObjId) -> SlotRef {
    let u = m.add_untyped(0x9000_0000, 1 << 18);
    let uc = m.root_cap(u);
    m.put_cap(c0, 20, uc).expect("cap Untyped 2");
    let us = slot(c0, 20);
    let tys = [
        ObjType::Endpoint,
        ObjType::Endpoint,
        ObjType::Reply,
        ObjType::Notification,
    ];
    for (i, ty) in tys.iter().enumerate() {
        let cap = m.retype(us, *ty, 0, 0).expect("retype cây 2");
        m.put_cap(c0, 21 + i, cap).expect("put cap cây 2");
    }
    us
}

#[test]
fn kt63_core_revoke_huy_caller_waiter_hang_va_tra_cancelled() {
    let mut r = build();
    let us = them_cay(&mut r.m, r.c0);
    let ep_a = slot(r.c0, 21);
    let ep_b = slot(r.c0, 22);
    let rep_a = slot(r.c0, 23);
    let ntn_a = slot(r.c0, 24);
    // t1 Recv ep_a kèm Reply ⇒ t0 Call ⇒ t0 BlockedOnReply, t1 Ready.
    r.m.recv(r.t[1], ep_a, Some(rep_a)).expect("waiter ep_a");
    r.m.call_msg(r.t[0], ep_a, msg(30, &[0xA0]))
        .expect("Call ep_a");
    assert_eq!(
        r.st(0),
        TcbState::BlockedOnReply {
            reply: r.m.cap_obj_at(rep_a.cnode, rep_a.slot).expect("rep obj")
        }
    );
    // Hàng Endpoint: t2 Send ep_b (chặn, không waiter) ⇒ message trong hàng.
    r.m.send(r.t[2], ep_b, msg(31, &[0xB0])).expect("Send ep_b");
    assert_eq!(
        r.m.endpoint_send_len(r.m.cap_obj_at(ep_b.cnode, ep_b.slot).expect("ep"))
            .expect("len"),
        1
    );
    // Waiter Notification: t1 Wait ntn_a ⇒ BlockedOnNotification.
    r.m.recv(r.t[1], ntn_a, None).expect("Wait ntn_a");
    assert!(matches!(r.st(1), TcbState::BlockedOnNotification { .. }));
    let sent_truoc = r.m.ipc_sent();
    // Revoke cả cây: ep_a/ep_b/rep_a/ntn_a chết.
    let _ = r.m.revoke(r.c0, Some(us.slot)).expect("revoke cây");
    // Caller: Cancelled + Ready (không treo).
    assert_eq!(r.st(0), TcbState::Ready, "caller phải được đánh thức");
    assert_eq!(
        r.m.tcb_last_ipc_error(r.t[0]).expect("err"),
        Some(KError::Cancelled)
    );
    // Waiter Notification của object bị finalise: Cancelled + Ready (chống treo).
    assert_eq!(
        r.st(1),
        TcbState::Ready,
        "waiter Notification phải thức khi object chết"
    );
    assert_eq!(
        r.m.tcb_last_ipc_error(r.t[1]).expect("err"),
        Some(KError::Cancelled)
    );
    // Hàng rỗng, message bị huỷ được đếm `ipc_dropped` (bất biến bảo toàn message).
    assert_eq!(r.m.ipc_pending(), 0);
    assert_eq!(r.m.ipc_dropped(), 1);
    assert_eq!(
        r.m.ipc_sent(),
        r.m.ipc_delivered() + r.m.ipc_pending() + r.m.ipc_dropped()
    );
    assert!(r.m.ipc_sent() >= sent_truoc);
    r.kiem("revoke cây IPC");
}

#[test]
fn kt63_core_recv_waiter_thuc_khi_endpoint_bi_finalise() {
    let mut r = build();
    let us = them_cay(&mut r.m, r.c0);
    let ep_a = slot(r.c0, 21);
    // t1 chờ Recv trên Endpoint sắp chết (không Reply).
    r.m.recv(r.t[1], ep_a, None).expect("Recv ep_a");
    assert!(matches!(r.st(1), TcbState::BlockedOnRecv { .. }));
    let _ = r.m.revoke(r.c0, Some(us.slot)).expect("revoke cây");
    assert_eq!(
        r.st(1),
        TcbState::Ready,
        "Recv waiter phải thức khi Endpoint chết (không treo)"
    );
    assert_eq!(
        r.m.tcb_last_ipc_error(r.t[1]).expect("err"),
        Some(KError::Cancelled)
    );
    r.kiem("revoke ep có Recv waiter");
}

#[test]
fn kt63_core_delete_cap_reply_huy_caller() {
    let mut r = build();
    // t1 Recv ep0 kèm Reply; t0 Call ⇒ BlockedOnReply.
    r.m.recv(r.t[1], r.eps[0], Some(r.reps)).expect("Recv");
    r.m.call_msg(r.t[0], r.eps[0], msg(40, &[0x40]))
        .expect("Call");
    assert_eq!(r.st(0), TcbState::BlockedOnReply { reply: r.rep });
    // Delete **cap** Reply ⇒ caller Cancelled + Ready, object Reply vẫn sống.
    r.m.delete(r.reps).expect("Delete cap Reply");
    assert_eq!(r.st(0), TcbState::Ready);
    assert_eq!(
        r.m.tcb_last_ipc_error(r.t[0]).expect("err"),
        Some(KError::Cancelled)
    );
    assert_eq!(r.m.reply_caller(r.rep).expect("caller"), None);
    assert!(r.m.object_alive(r.rep));
    r.kiem("delete cap Reply");
}

#[test]
fn kt63_core_delete_reply_cap_trong_recv_waiter_khong_gay_loi_cung() {
    let mut r = build();
    // Waiter giữ cap Reply; Delete cap đó ⇒ waiter coi như **không cấp Reply** (Q7b):
    // Call kế tiếp vẫn giao message + caller `Cancelled`, không lỗi cứng `InvalidCapability`.
    r.m.recv(r.t[1], r.eps[0], Some(r.reps)).expect("Recv");
    r.m.delete(r.reps).expect("Delete cap Reply của waiter");
    assert_eq!(
        r.m.call_msg(r.t[0], r.eps[0], msg(41, &[0x41])),
        Err(KError::Cancelled),
        "Q7(b): message vẫn giao nhưng Call trả Cancelled (thiếu Reply)"
    );
    assert_eq!(r.st(0), TcbState::Ready);
    assert_eq!(
        r.m.tcb_last_ipc_error(r.t[0]).expect("err"),
        Some(KError::Cancelled)
    );
    assert_eq!(r.st(1), TcbState::Ready, "waiter được đánh thức");
    assert_eq!(r.info(1).expect("nhận").length(), 1);
    r.kiem("dead reply trong waiter");
}

#[test]
fn kt63_core_nb_send_xep_hang_roi_cai_cap() {
    // Message mang 4 cap vào **hàng** (không waiter) rồi Recv ⇒ plan tính lúc Recv, cài đủ;
    // message không bị mất (bất biến bảo toàn message).
    let mut r = build();
    r.m.tcb_set_recv_window(r.ts[1], r.win[0]).expect("bind");
    let caps = r.src;
    r.m.nb_send(r.t[0], r.eps[0], msg_caps(50, &[1], &caps))
        .expect("NBSend xếp hàng");
    assert_eq!(r.m.ipc_pending(), 1);
    r.m.recv(r.t[1], r.eps[0], None).expect("Recv pop");
    assert_eq!(r.info(1).expect("nhận").caps(), 4);
    assert_eq!(r.m.ipc_pending(), 0);
    assert_eq!(r.m.cap_transfer_stats(), (4, 0, 0));
    r.kiem("xếp hàng rồi cài cap");
}

#[test]
fn kt63_core_bang_cap_day_tra_not_enough_memory_nguyen_tu() {
    // **F1 (reviewer-kt63)** — bảng cap đầy ⇒ `NotEnoughMemory` **trước** `admit`/pop: message
    // không vào đường IPC, waiter còn `BlockedOnRecv`, không panic giữa message.
    let mut r = build();
    let mut cn = [0u32; 6];
    for c in cn.iter_mut() {
        *c = r.m.add_cnode(96);
    }
    let src = r.m.cap_at(r.c0, 2).expect("cap nguồn");
    'fill: for c in cn.iter().copied() {
        for s in 0..96 {
            if r.m.cap_count() >= kc::MAX_CAPS {
                break 'fill;
            }
            r.m.copy(slot(r.c0, 2), slot(c, s)).expect("copy");
        }
    }
    assert_eq!(r.m.cap_count(), kc::MAX_CAPS, "bảng cap đầy đúng biên");
    let wcn = r.m.add_cnode(64);
    r.m.tcb_set_recv_window(
        r.ts[1],
        [slot(wcn, 0), slot(wcn, 1), slot(wcn, 2), slot(wcn, 3)],
    )
    .expect("bind window");
    r.m.recv(r.t[1], r.eps[0], None).expect("Recv");
    let sent = r.m.ipc_sent();
    let pending = r.m.ipc_pending();
    assert_eq!(
        r.m.send(r.t[0], r.eps[0], msg_caps(60, &[1], &[src])),
        Err(KError::NotEnoughMemory)
    );
    assert_eq!(r.m.ipc_sent(), sent, "không cấp msg_id");
    assert_eq!(r.m.ipc_pending(), pending, "message không vào đường IPC");
    assert!(matches!(r.st(1), TcbState::BlockedOnRecv { .. }));
    r.kiem("bảng cap đầy");
}

#[test]
fn kt63_core_window_tro_cnode_chet_tra_invalid_capability() {
    // **F2 (reviewer-kt63)** — receive window trỏ CNode đã finalise ⇒ `InvalidCapability`
    // **trước** `admit`; không tạo cap mồ côi, message không kẹt nửa đường.
    let mut r = build();
    let u2 = r.m.add_untyped(0x8100_0000, 1 << 16);
    let uc2 = r.m.root_cap(u2);
    r.m.put_cap(r.c0, 53, uc2).expect("cap Untyped 2");
    let wcap =
        r.m.retype(slot(r.c0, 53), ObjType::CNode, 6, 0)
            .expect("retype CNode");
    let wcn = r.m.cap_obj(wcap).expect("obj CNode");
    r.m.put_cap(r.c0, 52, wcap).expect("giữ cap CNode");
    r.m.tcb_set_recv_window(
        r.ts[1],
        [slot(wcn, 0), slot(wcn, 1), slot(wcn, 2), slot(wcn, 3)],
    )
    .expect("bind window");
    r.m.recv(r.t[1], r.eps[0], None).expect("Recv");
    r.m.revoke(r.c0, Some(53))
        .expect("revoke Untyped chứa CNode");
    assert!(!r.m.object_alive(wcn), "CNode window đã chết");
    let sent = r.m.ipc_sent();
    assert_eq!(
        r.m.send(r.t[0], r.eps[0], msg_caps(61, &[1], &[r.src[0]])),
        Err(KError::InvalidCapability)
    );
    assert_eq!(r.m.ipc_sent(), sent);
    assert!(matches!(r.st(1), TcbState::BlockedOnRecv { .. }));
    r.kiem("window trỏ CNode chết");
}

#[test]
fn kt63_core_cap_nguon_chet_giua_hang_va_giao_thi_that_bai_mem() {
    // **F3 (reviewer-kt63)** — cap nguồn chết **giữa lúc xếp hàng và lúc giao**: message vẫn
    // được giao, `info.caps = 0`, `cap_transfer_failed += 1`, **không** mất message.
    let mut r = build();
    r.m.tcb_set_recv_window(r.ts[1], r.win[0]).expect("bind");
    let src = r.m.copy(r.eps[1], slot(r.c0, 50)).expect("copy cap nguồn");
    r.m.nb_send(r.t[0], r.eps[0], msg_caps(62, &[7], &[src]))
        .expect("NBSend xếp hàng");
    r.m.delete(slot(r.c0, 50)).expect("delete cap nguồn");
    r.m.recv(r.t[1], r.eps[0], None).expect("Recv");
    let info = r.info(1).expect("message vẫn được giao");
    assert_eq!(info.caps(), 0, "cap chết ⇒ thất bại mềm");
    assert_eq!(info.length(), 1);
    assert_eq!(r.m.cap_transfer_stats(), (0, 0, 1));
    assert_eq!(r.m.ipc_dropped(), 0);
    r.kiem("cap nguồn chết giữa hàng");
}
