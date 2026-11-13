// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-27 — **Notification**: word 64 bit + badge, `Signal` **OR** + **một** waiter + xoá word,
//! `Wait`/`Poll`, bound notification với TCB + `SchedContext.Bind`, bind 1:1, unbind khi TCB
//! chết.
//!
//! **Model-only `[TK]`, không phải spec/ABI**; ngữ nghĩa đọc **đúng** thiết kế §4.7 `:328`
//! ("word 64 bit; Signal OR badge, đánh thức **một** waiter, giao word, **xoá word**; Wait/Poll;
//! bound notification với TCB và SchedContext") + **D2-53 Q6** (bind Notification bằng **model
//! API**, **không** thêm chữ ký TCB vào ABI) + K1C-PLAN §5 KT-27 DoD 1..8.
//!
//! Khác v1 **có chủ ý** (QD §3.11): **không** "đánh thức tất cả" — broadcast là việc user space
//! (mỗi bên chờ một notification riêng).
//!
//! Test đỏ trước (D2-05): [`JOURNAL/kt27-red-compile.log`](../../agent/JOURNAL/kt27-red-compile.log)
//! (`NotificationValue`, `Machine::{notification_signal, notification_word, notification_badge,
//! notification_poll, notification_wait, notification_bound_sc, tcb_notification,
//! tcb_unbind_notification, sched_bound_notification, check_notifications}`, `Step::{Signal,
//! WaitNotification, PollNotification}` chưa có ⇒ E0432/E0599) và
//! [`JOURNAL/kt27-red-semantics.log`](../../agent/JOURNAL/kt27-red-semantics.log) (probe tạm:
//! `NotificationState` chỉ có `waiter`, không word/badge/signal; `send` trên Notification trả
//! `NotSupported`; `tcb_bind_notification` không 1:1; `SchedContext.Bind` không ghi `bound_sc`).
//!
//! Bất biến kiểm sau **mỗi** thao tác: `check_sched` + `check_ipc` + `check_notifications` +
//! `check_caps` + `check_all_bytes`.

use vifast_sim::ipc::Message;
use vifast_sim::kobj::{
    KError, Machine, NotificationValue, ObjId, ObjType, SlotRef, TcbState, RIGHTS_ALL,
};
use vifast_sim::scenario::{run_sequential, ScenarioStep, Step};
use vifast_sim::Lcg;

fn slot(cnode: ObjId, s: usize) -> SlotRef {
    SlotRef::new(cnode, s)
}

fn val(word: u64, badge: u64) -> NotificationValue {
    NotificationValue { word, badge }
}

fn msg_empty() -> Message {
    vifast_sim::ipc::Message::new(vifast_sim::ipc::MessageInfo::empty(), &[])
        .expect("Message rỗng hợp lệ")
}

// ---------------------------------------------------------------------------
// Rig: CNode 128 slot; 2 Endpoint; 1 Reply; **2 Notification** (+ 1 cap badge trên ntf0);
// n TCB (mỗi TCB một SchedContext đã Configure — test-only D2-50); 1 SchedControl.
// TCB 0/1/2 là các thread; SC 0/1/2 tương ứng.
// ---------------------------------------------------------------------------

const S_EP: [usize; 2] = [0, 1];
const S_REP: usize = 2;
const S_NTFN: [usize; 2] = [4, 5];
/// Cap `mint` badge `0x0F0F` trên `ntf0` — `Send` trên Notification = **Signal** với badge
/// của cap (seL4; model-only `[TK]`).
const S_NTFN_BADGE: usize = 6;
const S_TCB: usize = 10;
const S_SC: usize = 70;
const S_CTL: usize = 120;

struct Rig {
    m: Machine,
    ep: [ObjId; 2],
    ep_s: [SlotRef; 2],
    rep_s: SlotRef,
    ntfn: [ObjId; 2],
    ntfn_s: [SlotRef; 2],
    ntfn_badge_s: SlotRef,
    sc: Vec<ObjId>,
    sc_s: Vec<SlotRef>,
    t: Vec<ObjId>,
    t_s: Vec<SlotRef>,
}

impl Rig {
    fn new(n_tcb: usize) -> Self {
        assert!((1..=3).contains(&n_tcb), "rig dựng sẵn 3 TCB");
        let mut m = Machine::new();
        let c0 = m.add_cnode(128);
        let ctl = m.add_kernel_object(ObjType::SchedControl);
        let ctl_cap = m.root_cap(ctl);
        m.put_cap(c0, S_CTL, ctl_cap).expect("cap SchedControl");
        let ctl_s = slot(c0, S_CTL);

        let mut ep = [0u32; 2];
        let mut ep_s = [slot(c0, 0); 2];
        for (i, s) in S_EP.iter().enumerate() {
            ep[i] = m.add_kernel_object(ObjType::Endpoint);
            let cap = m.root_cap(ep[i]);
            m.put_cap(c0, *s, cap).expect("cap Endpoint");
            ep_s[i] = slot(c0, *s);
        }
        let rep = m.add_kernel_object(ObjType::Reply);
        let rep_cap = m.root_cap(rep);
        m.put_cap(c0, S_REP, rep_cap).expect("cap Reply");
        let rep_s = slot(c0, S_REP);

        let mut ntfn = [0u32; 2];
        let mut ntfn_s = [slot(c0, 0); 2];
        for (i, s) in S_NTFN.iter().enumerate() {
            ntfn[i] = m.add_kernel_object(ObjType::Notification);
            let cap = m.root_cap(ntfn[i]);
            m.put_cap(c0, *s, cap).expect("cap Notification");
            ntfn_s[i] = slot(c0, *s);
        }
        m.mint(ntfn_s[0], slot(c0, S_NTFN_BADGE), RIGHTS_ALL, Some(0x0F0F))
            .expect("mint badge trên ntf0");
        let ntfn_badge_s = slot(c0, S_NTFN_BADGE);

        let mut t = Vec::with_capacity(n_tcb);
        let mut t_s = Vec::with_capacity(n_tcb);
        let mut sc = Vec::with_capacity(n_tcb);
        let mut sc_s = Vec::with_capacity(n_tcb);
        for i in 0..n_tcb {
            let obj = m.add_kernel_object(ObjType::Tcb);
            let cap = m.root_cap(obj);
            m.put_cap(c0, S_TCB + i, cap).expect("cap TCB");
            t.push(obj);
            t_s.push(slot(c0, S_TCB + i));

            let sc_obj = m.add_kernel_object(ObjType::SchedContext);
            let sc_cap = m.root_cap(sc_obj);
            m.put_cap(c0, S_SC + i, sc_cap).expect("cap SchedContext");
            let sc_slot = slot(c0, S_SC + i);
            m.sched_control_configure(ctl_s, sc_slot, 4, 8)
                .expect("SchedControl.Configure");
            m.set_sched_params(t_s[i], Some(sc_slot))
                .expect("set_sched_params (test-only)");
            m.tcb_set_priority(t_s[i], 5, 255).expect("Tcb.SetPriority");
            sc.push(sc_obj);
            sc_s.push(sc_slot);
        }
        let mut r = Self {
            m,
            ep,
            ep_s,
            rep_s,
            ntfn,
            ntfn_s,
            ntfn_badge_s,
            sc,
            sc_s,
            t,
            t_s,
        };
        for i in 0..r.t.len() {
            r.m.transition_tcb(r.t[i], TcbState::Ready)
                .expect("Inactive→Ready");
        }
        r
    }

    /// Bất biến của KT-27 sau **mỗi** bước: scheduler + IPC + Notification + cap + byte.
    fn kiem(&self, buoc: &str) {
        if let Err(e) = self.m.check_sched() {
            panic!("check_sched sau {buoc}: {e}");
        }
        if let Err(e) = self.m.check_ipc() {
            panic!("check_ipc sau {buoc}: {e}");
        }
        if let Err(e) = self.m.check_notifications() {
            panic!("check_notifications sau {buoc}: {e}");
        }
        if let Err(e) = self.m.check_caps() {
            panic!("check_caps sau {buoc}: {e}");
        }
        if let Err(e) = self.m.check_all_bytes() {
            panic!("check_all_bytes sau {buoc}: {e}");
        }
    }

    fn st(&self, i: usize) -> TcbState {
        self.m.tcb_state(self.t[i]).expect("tcb_state")
    }

    fn word(&self, i: usize) -> u64 {
        self.m
            .notification_word(self.ntfn[i])
            .expect("notification_word")
    }

    fn badge(&self, i: usize) -> u64 {
        self.m
            .notification_badge(self.ntfn[i])
            .expect("notification_badge")
    }

    fn tval(&self, i: usize) -> Option<NotificationValue> {
        self.m
            .tcb_notification(self.t[i])
            .expect("tcb_notification")
    }

    fn recv_ntfn(&mut self, ti: usize, ni: usize) -> Result<(), KError> {
        self.m.recv(self.t[ti], self.ntfn_s[ni], None)
    }

    fn nb_recv_ntfn(&mut self, ti: usize, ni: usize) -> Result<Option<Message>, KError> {
        self.m.nb_recv(self.t[ti], self.ntfn_s[ni])
    }

    fn send_ntfn(&mut self, ti: usize, ni: usize) -> Result<(), KError> {
        self.m.send(self.t[ti], self.ntfn_s[ni], msg_empty())
    }
}

// ===========================================================================
// 1. Word 64 bit + OR + badge.
// ===========================================================================

#[test]
fn word_64_bit_cao_va_thap() {
    let mut r = Rig::new(1);
    let hi = 1u64 << 63;
    r.m.notification_signal(r.ntfn[0], 1)
        .expect("signal bit thấp");
    r.kiem("signal bit thấp");
    assert_eq!(r.word(0), 1);
    assert_eq!(r.badge(0), 1);
    r.m.notification_signal(r.ntfn[0], hi)
        .expect("signal bit cao");
    r.kiem("signal bit cao");
    assert_eq!(r.word(0), 1 | hi, "word phải giữ đủ 64 bit");
    assert_eq!(r.badge(0), 1 | hi);
    let got =
        r.m.notification_poll(r.ntfn[0])
            .expect("poll")
            .expect("phải có word");
    assert_eq!(got, val(1 | hi, 1 | hi));
    assert_eq!(r.word(0), 0, "poll lấy xong phải xoá word");
    assert_eq!(r.badge(0), 0, "poll lấy xong phải xoá badge");
    r.kiem("poll xoá word");
}

#[test]
fn signal_or_ba_lan_truoc_khi_wait() {
    let mut r = Rig::new(1);
    for b in [0b001u64, 0b010, 0b100] {
        r.m.notification_signal(r.ntfn[0], b).expect("signal OR");
        r.kiem("signal OR");
    }
    assert_eq!(r.word(0), 0b111, "Signal phải OR vào word");
    assert_eq!(r.badge(0), 0b111, "badge cũng OR");
    r.recv_ntfn(0, 0).expect("Wait");
    r.kiem("Wait sau OR");
    assert_eq!(
        r.tval(0),
        Some(val(0b111, 0b111)),
        "Wait trả word + badge đã OR"
    );
    assert_eq!(r.word(0), 0, "Wait xong xoá word");
    assert_eq!(r.badge(0), 0, "Wait xong xoá badge");
    assert_eq!(r.st(0), TcbState::Ready, "word có sẵn ⇒ Wait trả ngay");
}

#[test]
fn badge_0_hop_le_va_van_danh_thuc_waiter() {
    // D2/K1C-PLAN rủi ro (b): badge 0 **hợp lệ** — không dùng 0 làm "chưa có tín hiệu".
    let mut r = Rig::new(2);
    r.recv_ntfn(0, 0).expect("Wait rỗng");
    assert_eq!(r.st(0), TcbState::BlockedOnNotification { ntfn: r.ntfn[0] });
    r.m.notification_signal(r.ntfn[0], 0)
        .expect("signal badge 0");
    r.kiem("signal badge 0");
    assert_eq!(r.st(0), TcbState::Ready, "signal badge 0 vẫn đánh thức");
    assert_eq!(r.tval(0), Some(val(0, 0)), "nhận word/badge = 0");
    assert_eq!(r.m.notification_waiter(r.ntfn[0]), Ok(None));
    assert_eq!(r.word(0), 0);
    assert_eq!(r.st(1), TcbState::Ready, "TCB khác không bị đánh thức");
    assert_eq!(r.tval(1), None);
}

#[test]
fn badge_or_nhieu_signal_va_wait_tra_badge() {
    let mut r = Rig::new(1);
    for b in [0x10u64, 0x20, 0x40] {
        r.m.notification_signal(r.ntfn[0], b).expect("signal badge");
    }
    assert_eq!(r.badge(0), 0x70);
    r.m.notification_signal(r.ntfn[0], 0x0F)
        .expect("signal badge 2");
    assert_eq!(r.badge(0), 0x7F, "badge OR dồn");
    r.recv_ntfn(0, 0).expect("Wait");
    assert_eq!(r.tval(0), Some(val(0x7F, 0x7F)));
    assert_eq!(r.badge(0), 0);
    r.kiem("Wait badge");
}

// ===========================================================================
// 2. Wait / Poll và "một waiter".
// ===========================================================================

#[test]
fn wait_tren_rong_blocked_on_notification() {
    let mut r = Rig::new(1);
    r.recv_ntfn(0, 0).expect("Wait rỗng");
    r.kiem("Wait rỗng");
    assert_eq!(
        r.st(0),
        TcbState::BlockedOnNotification { ntfn: r.ntfn[0] },
        "Wait rỗng ⇒ BlockedOnNotification"
    );
    assert_eq!(r.m.notification_waiter(r.ntfn[0]), Ok(Some(r.t[0])));
    assert_eq!(r.word(0), 0);
    assert_eq!(r.tval(0), None, "chưa có gì để giao");
}

#[test]
fn mot_waiter_duy_nhat_ba_tcb() {
    let mut r = Rig::new(3);
    r.recv_ntfn(0, 0).expect("t0 Wait");
    assert_eq!(r.st(0), TcbState::BlockedOnNotification { ntfn: r.ntfn[0] });
    assert_eq!(
        r.recv_ntfn(1, 0),
        Err(KError::Busy),
        "waiter thứ hai ⇒ Busy"
    );
    assert_eq!(r.recv_ntfn(2, 0), Err(KError::Busy), "waiter thứ ba ⇒ Busy");
    assert_eq!(r.st(1), TcbState::Ready, "Busy không đổi trạng thái");
    assert_eq!(r.st(2), TcbState::Ready);
    assert_eq!(r.m.notification_waiter(r.ntfn[0]), Ok(Some(r.t[0])));
    r.m.notification_signal(r.ntfn[0], 0xA5)
        .expect("signal đánh thức một");
    r.kiem("signal một waiter");
    assert_eq!(r.st(0), TcbState::Ready);
    assert_eq!(r.st(1), TcbState::Ready);
    assert_eq!(r.st(2), TcbState::Ready);
    assert_eq!(r.tval(0), Some(val(0xA5, 0xA5)));
    assert_eq!(r.tval(1), None, "chỉ **một** waiter được đánh thức");
    assert_eq!(r.tval(2), None);
    assert_eq!(r.word(0), 0, "giao cho waiter xong word = 0");
    assert_eq!(r.m.notification_waiter(r.ntfn[0]), Ok(None));
}

#[test]
fn word_chi_xoa_khi_giao_cho_waiter() {
    let mut r = Rig::new(1);
    r.m.notification_signal(r.ntfn[0], 0xAA)
        .expect("signal không waiter");
    r.kiem("signal không waiter");
    assert_eq!(r.word(0), 0xAA, "không waiter ⇒ word giữ cho Wait sau");
    r.m.notification_signal(r.ntfn[0], 0x55)
        .expect("signal OR tiếp");
    assert_eq!(r.word(0), 0xFF);
    assert_eq!(r.m.notification_waiter(r.ntfn[0]), Ok(None));
    r.recv_ntfn(0, 0).expect("Wait thấy word");
    assert_eq!(r.tval(0), Some(val(0xFF, 0xFF)));
    assert_eq!(r.word(0), 0, "chỉ xoá khi giao");
}

#[test]
fn poll_rong_model_api_va_nb_recv_busy() {
    let mut r = Rig::new(1);
    assert_eq!(r.m.notification_poll(r.ntfn[0]), Ok(None));
    assert_eq!(r.nb_recv_ntfn(0, 0), Err(KError::Busy), "Poll rỗng ⇒ Busy");
    assert_eq!(r.st(0), TcbState::Ready, "Poll không chặn");
    r.kiem("poll rỗng");
}

#[test]
fn nb_recv_tra_word_badge_va_xoa() {
    let mut r = Rig::new(1);
    let w = 0x8000_0000_0000_0001u64;
    r.m.notification_signal(r.ntfn[0], w).expect("signal");
    let got = r
        .nb_recv_ntfn(0, 0)
        .expect("NBRecv Notification")
        .expect("phải có word");
    assert_eq!(got.mr[0], w, "word nằm ở MR0");
    assert_eq!(got.badge, w, "badge đi cùng");
    assert_eq!(r.word(0), 0);
    assert_eq!(r.badge(0), 0);
    assert_eq!(r.tval(0), Some(val(w, w)));
    assert_eq!(r.st(0), TcbState::Ready);
    r.kiem("nb_recv notification");
}

#[test]
fn poll_tren_notification_cap_khi_tcb_khong_chay_busy() {
    let mut r = Rig::new(2);
    r.recv_ntfn(0, 1).expect("t0 chờ ntf1");
    assert_eq!(r.st(0), TcbState::BlockedOnNotification { ntfn: r.ntfn[1] });
    assert_eq!(
        r.nb_recv_ntfn(0, 0),
        Err(KError::Busy),
        "TCB không chạy được ⇒ Busy (KT-25 giữ nguyên)"
    );
    assert_eq!(r.word(0), 0);
}

// ===========================================================================
// 3. `Send`/`NBSend` trên Notification = Signal theo badge của cap.
// ===========================================================================

#[test]
fn send_tren_notification_la_signal_theo_badge_cap() {
    let mut r = Rig::new(1);
    // Cap gốc `ntf0` không badge ⇒ Signal(0): word vẫn 0 (badge 0 hợp lệ, không "bật" gì).
    r.send_ntfn(0, 0).expect("Send = Signal");
    r.kiem("send ntf badge 0");
    assert_eq!(r.word(0), 0);
    // Cap mint badge `0x0F0F` ⇒ word/badge = 0x0F0F.
    r.m.send(r.t[0], r.ntfn_badge_s, msg_empty())
        .expect("Send cap badge");
    r.kiem("send ntf badge");
    assert_eq!(r.word(0), 0x0F0F);
    assert_eq!(r.badge(0), 0x0F0F);
}

#[test]
fn nb_send_tren_notification_cung_la_signal() {
    let mut r = Rig::new(1);
    r.m.nb_send(r.t[0], r.ntfn_badge_s, msg_empty())
        .expect("NBSend = Signal");
    r.kiem("nb_send ntf");
    assert_eq!(r.word(0), 0x0F0F);
    assert_eq!(r.st(0), TcbState::Ready, "NBSend không chặn");
}

#[test]
fn send_notification_khi_tcb_khong_chay_loi() {
    let mut r = Rig::new(2);
    r.recv_ntfn(0, 1).expect("t0 chờ ntf1");
    assert_eq!(
        r.send_ntfn(0, 0),
        Err(KError::IllegalOperation),
        "TCB đang chờ không phát Signal được"
    );
    assert_eq!(r.word(0), 0, "lỗi không đổi word");
}

#[test]
fn recv_notification_xoa_last_ipc_error_cu() {
    let mut r = Rig::new(2);
    // t1 `Recv` Endpoint **không** cấp Reply ⇒ `Call` của t0 bị `Cancelled` (D2-53 Q7b) và
    // `last_ipc_error` còn trên TCB.
    r.m.recv(r.t[1], r.ep_s[0], None).expect("server Recv");
    assert_eq!(
        r.m.call(r.t[0], r.ep_s[0], 1),
        Err(KError::Cancelled),
        "kernel thiếu Reply ⇒ caller Cancelled"
    );
    assert_eq!(
        r.m.tcb_last_ipc_error(r.t[0]),
        Ok(Some(KError::Cancelled)),
        "lỗi IPC cũ còn trên TCB"
    );
    // Signal trước để Wait trả ngay (không chặn).
    r.m.notification_signal(r.ntfn[0], 0x42).expect("signal");
    r.recv_ntfn(0, 0).expect("Wait Notification");
    assert_eq!(
        r.m.tcb_last_ipc_error(r.t[0]),
        Ok(None),
        "Recv Notification phải reset last_ipc_error như nhánh Endpoint (KT-26)"
    );
    assert_eq!(r.tval(0), Some(val(0x42, 0x42)));
    r.kiem("recv notification reset lỗi IPC");
}

#[test]
fn recv_notification_co_reply_cap_loi() {
    let mut r = Rig::new(1);
    assert_eq!(
        r.m.recv(r.t[0], r.ntfn_s[0], Some(r.rep_s)),
        Err(KError::InvalidArgument),
        "Recv Notification không nhận Reply cap"
    );
    assert_eq!(r.st(0), TcbState::Ready);
}

// ===========================================================================
// 4. Bound notification: TCB 1:1, SchedContext.Bind, unbind.
// ===========================================================================

#[test]
fn bind_tcb_notification_1_1() {
    let mut r = Rig::new(2);
    r.m.tcb_bind_notification(r.t_s[0], r.ntfn_s[0])
        .expect("bind TCB–ntf0");
    r.kiem("bind TCB–ntf0");
    assert_eq!(
        r.m.tcb_info(r.t_s[0]).expect("TcbInfo").bound_notification,
        Some(r.ntfn[0])
    );
    assert_eq!(
        r.m.tcb_bind_notification(r.t_s[0], r.ntfn_s[1]),
        Err(KError::IllegalOperation),
        "TCB đã có bound notification khác"
    );
    assert_eq!(
        r.m.tcb_bind_notification(r.t_s[1], r.ntfn_s[0]),
        Err(KError::Busy),
        "notification đã bind TCB khác (1:1)"
    );
    r.m.tcb_unbind_notification(r.t_s[0]).expect("unbind TCB");
    assert_eq!(
        r.m.tcb_info(r.t_s[0]).expect("TcbInfo").bound_notification,
        None
    );
    r.m.tcb_bind_notification(r.t_s[1], r.ntfn_s[0])
        .expect("bind lại cho TCB khác");
    r.kiem("bind 1:1");
}

#[test]
fn bind_sched_context_notification_1_1() {
    // Cần **hai** SchedContext để kiểm 1:1 phía notification.
    let mut r = Rig::new(2);
    r.m.sched_context_bind(r.sc_s[0], r.ntfn_s[0])
        .expect("Bind(sc0, ntf0)");
    r.kiem("Bind(sc0, ntf0)");
    assert_eq!(r.m.notification_bound_sc(r.ntfn[0]), Ok(Some(r.sc[0])));
    assert_eq!(
        r.m.sched_bound_notification(r.sc_s[0]),
        Ok(Some(r.ntfn[0])),
        "SchedContext.bound trỏ Notification"
    );
    assert_eq!(
        r.m.sched_context_bind(r.sc_s[1], r.ntfn_s[0]),
        Err(KError::IllegalOperation),
        "notification đã bind SC khác (1:1)"
    );
    // Đổi target: SC0 sang ntf1 ⇒ ntf0 mất binding.
    r.m.sched_context_bind(r.sc_s[0], r.ntfn_s[1])
        .expect("Bind(sc0, ntf1)");
    r.kiem("Bind(sc0, ntf1)");
    assert_eq!(r.m.notification_bound_sc(r.ntfn[0]), Ok(None));
    assert_eq!(r.m.notification_bound_sc(r.ntfn[1]), Ok(Some(r.sc[0])));
    // Bind sang Reply ⇒ notification không còn bound SC.
    r.m.sched_context_bind(r.sc_s[0], r.rep_s)
        .expect("Bind(sc0, reply)");
    r.kiem("Bind(sc0, reply)");
    assert_eq!(r.m.notification_bound_sc(r.ntfn[1]), Ok(None));
    assert_eq!(r.m.sched_bound_notification(r.sc_s[0]), Ok(None));
}

#[test]
fn signal_danh_thuc_bound_tcb_dang_blocked_on_recv() {
    let mut r = Rig::new(2);
    r.m.tcb_bind_notification(r.t_s[0], r.ntfn_s[0])
        .expect("bind TCB–ntf0");
    r.m.recv(r.t[0], r.ep_s[0], None).expect("Recv Endpoint");
    assert_eq!(
        r.st(0),
        TcbState::BlockedOnRecv {
            ep: r.ep[0],
            reply: None
        }
    );
    assert_eq!(r.m.endpoint_recv_waiter(r.ep[0]), Ok(Some(r.t[0])));
    r.m.notification_signal(r.ntfn[0], 0xBEEF)
        .expect("signal bound");
    r.kiem("signal bound BlockedOnRecv");
    assert_eq!(r.st(0), TcbState::Ready, "signal đánh thức TCB bound");
    assert_eq!(
        r.m.endpoint_recv_waiter(r.ep[0]),
        Ok(None),
        "phải gỡ khỏi hàng Recv của Endpoint"
    );
    assert_eq!(r.tval(0), Some(val(0xBEEF, 0xBEEF)), "trả badge");
    assert_eq!(r.word(0), 0, "giao xong xoá word");
    assert_eq!(r.st(1), TcbState::Ready);
    assert_eq!(r.tval(1), None);
}

#[test]
fn signal_bound_khong_danh_thuc_tcb_bound_khac() {
    let mut r = Rig::new(3);
    r.m.tcb_bind_notification(r.t_s[0], r.ntfn_s[0])
        .expect("bind t0–ntf0");
    r.m.tcb_bind_notification(r.t_s[1], r.ntfn_s[1])
        .expect("bind t1–ntf1");
    r.m.recv(r.t[0], r.ep_s[0], None).expect("t0 BlockedOnRecv");
    r.recv_ntfn(2, 1).expect("t2 chờ ntf1");
    assert_eq!(r.st(2), TcbState::BlockedOnNotification { ntfn: r.ntfn[1] });
    // ntf1 có waiter t2 **và** TCB bound t1 (đang Ready) ⇒ đánh thức đúng t2.
    r.m.notification_signal(r.ntfn[1], 0x22)
        .expect("signal ntf1");
    r.kiem("signal ntf1");
    assert_eq!(r.st(2), TcbState::Ready, "waiter của ntf1 được đánh thức");
    assert_eq!(r.tval(2), Some(val(0x22, 0x22)));
    assert_eq!(
        r.st(1),
        TcbState::Ready,
        "TCB bound nhưng không chờ không bị đổi"
    );
    assert_eq!(r.tval(1), None);
    assert_eq!(
        r.st(0),
        TcbState::BlockedOnRecv {
            ep: r.ep[0],
            reply: None
        },
        "signal ntf1 không đánh thức TCB bound ntf0"
    );
    r.m.notification_signal(r.ntfn[0], 0x11)
        .expect("signal ntf0");
    r.kiem("signal ntf0");
    assert_eq!(r.st(0), TcbState::Ready);
    assert_eq!(r.tval(0), Some(val(0x11, 0x11)));
}

#[test]
fn tcb_chet_go_binding_va_waiter() {
    let mut r = Rig::new(2);
    r.m.tcb_bind_notification(r.t_s[0], r.ntfn_s[0])
        .expect("bind TCB–ntf0");
    r.recv_ntfn(0, 0).expect("Wait rỗng");
    assert_eq!(r.m.notification_waiter(r.ntfn[0]), Ok(Some(r.t[0])));
    r.m.finalise_tcb(r.t[0]).expect("finalise TCB");
    r.kiem("finalise TCB");
    assert_eq!(
        r.m.tcb_info(r.t_s[0]).expect("TcbInfo").bound_notification,
        None,
        "TCB chết phải unbind notification"
    );
    assert_eq!(
        r.m.notification_waiter(r.ntfn[0]),
        Ok(None),
        "waiter của TCB chết phải gỡ"
    );
    r.m.notification_signal(r.ntfn[0], 0x77)
        .expect("signal sau khi TCB chết");
    assert_eq!(r.word(0), 0x77, "không còn ai chờ ⇒ word giữ");
}

#[test]
fn notification_khong_bind_sc_van_chay() {
    let mut r = Rig::new(1);
    assert_eq!(r.m.notification_bound_sc(r.ntfn[0]), Ok(None));
    r.m.notification_signal(r.ntfn[0], 0x31).expect("signal");
    r.recv_ntfn(0, 0).expect("Wait");
    assert_eq!(r.tval(0), Some(val(0x31, 0x31)));
    r.kiem("notification không bind SC");
}

// ===========================================================================
// 5. Không mất đánh thức (C2): hai thứ tự.
// ===========================================================================

#[test]
fn khong_mat_danh_thuc_signal_truoc_va_sau_wait() {
    // (a) Signal **trước** Wait ⇒ word còn để lần Wait sau thấy.
    let mut r = Rig::new(1);
    r.m.notification_signal(r.ntfn[0], 0x1234)
        .expect("signal trước");
    r.kiem("signal trước wait");
    r.recv_ntfn(0, 0).expect("Wait thấy word còn");
    assert_eq!(r.tval(0), Some(val(0x1234, 0x1234)));
    assert_eq!(r.st(0), TcbState::Ready);

    // (b) Wait **trước** Signal ⇒ waiter được đánh thức, không mất.
    let mut r2 = Rig::new(1);
    r2.recv_ntfn(0, 0).expect("Wait trước");
    assert_eq!(
        r2.st(0),
        TcbState::BlockedOnNotification { ntfn: r2.ntfn[0] }
    );
    r2.m.notification_signal(r2.ntfn[0], 0x5678)
        .expect("signal sau");
    r2.kiem("signal sau wait");
    assert_eq!(r2.st(0), TcbState::Ready);
    assert_eq!(r2.tval(0), Some(val(0x5678, 0x5678)));
    assert_eq!(r2.word(0), 0);
}

#[test]
fn nbsend_wait_voi_word_co_san_khong_chan() {
    let mut r = Rig::new(1);
    r.m.notification_signal(r.ntfn[0], 0x99)
        .expect("signal sẵn");
    r.m.nb_send_wait(r.t[0], r.ep_s[0], msg_empty(), r.ntfn_s[0])
        .expect("NBSendWait");
    r.kiem("NBSendWait word sẵn");
    assert_eq!(
        r.st(0),
        TcbState::Ready,
        "Wait thấy word ⇒ trả ngay, không BlockedOnNotification"
    );
    assert_eq!(r.tval(0), Some(val(0x99, 0x99)));
    assert_eq!(r.word(0), 0);
}

// ===========================================================================
// 6. Bất biến: `check_notifications` bắt waiter lệch.
// ===========================================================================

#[test]
fn check_notifications_bat_waiter_lech() {
    let mut r = Rig::new(1);
    r.recv_ntfn(0, 0).expect("Wait rỗng");
    // Đường tắt **sai** (không qua unlink): đổi trạng thái trực tiếp ⇒ waiter còn treo.
    r.m.transition_tcb(r.t[0], TcbState::Ready)
        .expect("transition trực tiếp");
    assert_eq!(r.m.notification_waiter(r.ntfn[0]), Ok(Some(r.t[0])));
    let e =
        r.m.check_notifications()
            .expect_err("phải bắt waiter lệch trạng thái");
    assert!(
        e.contains("waiter") || e.contains("BlockedOnNotification"),
        "thông điệp phải nêu waiter/BlockedOnNotification: {e}"
    );
}

// ===========================================================================
// 7. Scenario: bước `Signal`/`WaitNotification`/`PollNotification`.
// ===========================================================================

#[test]
fn scenario_buoc_signal_wait_poll() {
    let mut r = Rig::new(1);
    let steps = [
        ScenarioStep {
            thread: "t0",
            step: Step::WaitNotification {
                tcb: r.t[0],
                ntfn: r.ntfn_s[0],
            },
            expect: Ok(()),
            expect_state: Some(TcbState::BlockedOnNotification { ntfn: r.ntfn[0] }),
        },
        ScenarioStep {
            thread: "ctl",
            step: Step::Signal {
                ntfn: r.ntfn[0],
                badge: 0x11,
            },
            expect: Ok(()),
            expect_state: None,
        },
        ScenarioStep {
            thread: "t0",
            step: Step::PollNotification {
                tcb: r.t[0],
                ntfn: r.ntfn_s[0],
            },
            expect: Err(KError::Busy),
            expect_state: Some(TcbState::Ready),
        },
        ScenarioStep {
            thread: "t0",
            step: Step::PollNotification {
                tcb: r.t[0],
                ntfn: r.ntfn_s[0],
            },
            expect: Err(KError::Busy),
            expect_state: Some(TcbState::Ready),
        },
    ];
    // `Signal` mang badge 0x11 ⇒ sau bước 2 waiter đã Ready và word đã giao; bước 3 và 4
    // Poll trên notification rỗng ⇒ `Busy` (không đổi trạng thái).
    run_sequential(&mut r.m, &steps).expect("scenario Notification");
    r.kiem("scenario");
    assert_eq!(r.tval(0), Some(val(0x11, 0x11)));
}

// ===========================================================================
// 8. Fuzz 1.000 seed × 64 op: bất biến sau mỗi bước + cùng seed cùng trace.
// ===========================================================================

/// Một bước fuzz: `(op, kết quả, trạng thái 3 TCB, word/badge 2 notification)` — so hai lượt
/// cùng seed ⇒ cùng trace (DoD 6).
type BuocFuzz = (u8, String, [String; 3], [u64; 2], [u64; 2]);

fn ten_ket_qua(r: Result<(), KError>) -> String {
    match r {
        Ok(()) => "ok".to_string(),
        Err(e) => e.name().to_string(),
    }
}

fn mot_buoc(r: &mut Rig, seed: u64, i: u64, trace: &mut Vec<BuocFuzz>) {
    let mut rng = Lcg::new(seed.wrapping_mul(64).wrapping_add(i));
    let op = rng.below(10) as u8;
    let ti = rng.below(3) as usize;
    let ni = rng.below(2) as usize;
    let si = rng.below(3) as usize;
    let badge = rng.next_u64();
    let res = match op {
        // Signal (model API) — badge ngẫu nhiên, có cả bit cao.
        0 => r.m.notification_signal(r.ntfn[ni], badge),
        // Wait qua cap Notification (recv).
        1 => r.m.recv(r.t[ti], r.ntfn_s[ni], None),
        // Poll qua cap Notification (nb_recv).
        2 => r.m.nb_recv(r.t[ti], r.ntfn_s[ni]).map(|_| ()),
        // Signal qua Send + badge của cap (0x0F0F).
        3 => r.m.send(r.t[ti], r.ntfn_badge_s, msg_empty()),
        // Signal qua NBSend.
        4 => r.m.nb_send(r.t[ti], r.ntfn_badge_s, msg_empty()),
        // Bind TCB–Notification (1:1).
        5 => r.m.tcb_bind_notification(r.t_s[ti], r.ntfn_s[ni]),
        // Unbind TCB.
        6 => r.m.tcb_unbind_notification(r.t_s[ti]),
        // Bind SchedContext–Notification (1:1).
        7 => r.m.sched_context_bind(r.sc_s[si], r.ntfn_s[ni]),
        // Poll model API trên object.
        8 => r.m.notification_poll(r.ntfn[ni]).map(|_| ()),
        // Wake (model API của KT-19) — đường khác đưa TCB khỏi hàng chờ.
        _ => r.m.wake(r.t[ti]),
    };
    let mut states = [String::new(), String::new(), String::new()];
    for (k, s) in states.iter_mut().enumerate() {
        *s = r.st(k).render();
    }
    trace.push((
        op,
        ten_ket_qua(res),
        states,
        [r.word(0), r.word(1)],
        [r.badge(0), r.badge(1)],
    ));
}

#[test]
fn fuzz_1000_seed_64_op_bat_bien_va_tat_dinh() {
    #[cfg(miri)]
    let seeds: u64 = 3;
    #[cfg(not(miri))]
    let seeds: u64 = 1_000;
    #[cfg(miri)]
    let ops: u64 = 8;
    #[cfg(not(miri))]
    let ops: u64 = 64;

    for seed in 0..seeds {
        let mut trace = Vec::with_capacity(ops as usize);
        {
            let mut r = Rig::new(3);
            for i in 0..ops {
                mot_buoc(&mut r, seed, i, &mut trace);
                // Bất biến sau **mỗi** bước (DoD 6/7).
                r.kiem(&format!("seed {seed} bước {i}"));
            }
        }
        // Cùng seed ⇒ cùng trace.
        let mut trace2 = Vec::with_capacity(ops as usize);
        {
            let mut r2 = Rig::new(3);
            for i in 0..ops {
                mot_buoc(&mut r2, seed, i, &mut trace2);
            }
        }
        assert_eq!(trace, trace2, "seed {seed}: hai lượt phải cùng trace");
    }
}
