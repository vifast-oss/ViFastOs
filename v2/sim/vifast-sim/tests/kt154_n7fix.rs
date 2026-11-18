// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-154 (N7-2) — **mọi đường đánh thức TCB phải tôn trọng hart `home`**.
//!
//! KT-151 (N7) dựng máy kiểm `v2/tools/check-wake-paths.py` và phát hiện **16 đường**
//! `blocked/sleeping → Ready` **trực tiếp** trong `v2/kernel` (IPC/Reply/Notification/
//! finalise/refill) — không đi qua `wake()`. Với TCB có `home` khác hart đang chạy, các đường
//! đó đánh thức TCB **mà không đánh thức hart `home`**: TCB vào hàng chờ của `home` (nhờ
//! `sched_sync`) nhưng hart `home` **không** nhận được `MailboxMsg::Wake` ⇒ trên SMP thật nó
//! có thể không bao giờ chạy. Đây là nợ **N7-2**.
//!
//! Tệp này là test **đỏ-trước** của KT-154: mỗi ca đòi, sau một đường **không phải**
//! `wake()` (Reply / Notification signal / Recv giao message):
//! - TCB **không** nằm trong ready queue của hart **hiện tại** (hart 0);
//! - hộp thư của hart `home` (hart 1) có **đúng một** `MailboxMsg::Wake(tcb)`;
//! - hart `home` xử lý hộp thư ⇒ TCB `Ready`/`Running` trên hart `home`.
//!
//! Ca cùng hart và ca `n_harts == 1` giữ nguyên ngữ nghĩa cũ: **không** gửi message.

use vifast_core as kc;
use vifast_core::ipc::{Message, MessageInfo};
use vifast_core::{KError, Machine, ObjId, ObjType, SlotRef, TcbState};

// ---------------------------------------------------------------------------------------------
// Hằng test (không phải ngưỡng): CNode 128 slot, SC budget = period = 64.
// ---------------------------------------------------------------------------------------------

const S_EP: usize = 0;
const S_REP: usize = 1;
const S_NTFN: usize = 2;
const S_TCB: usize = 10;
const S_SC: usize = 70;
const S_CTL: usize = 120;
const SC_BUDGET: u64 = 64;

fn slot(cnode: u32, s: usize) -> SlotRef {
    SlotRef::new(cnode, s)
}

/// `Message` với `len = words.len()`, không cap.
fn msg(label: u32, words: &[u64]) -> Message {
    let info = MessageInfo::new(label, 0, false, 0, words.len() as u8).expect("MessageInfo hợp lệ");
    Message::new(info, words).expect("Message hợp lệ")
}

// ---------------------------------------------------------------------------------------------
// Rig: 2 hart, 2 TCB (t0 = client home 1, t1 = server home 0), Endpoint + Reply + Notification.
// ---------------------------------------------------------------------------------------------

struct Rig {
    m: Machine,
    ep: ObjId,
    ep_s: SlotRef,
    rep: ObjId,
    rep_s: SlotRef,
    ntfn: ObjId,
    ntfn_s: SlotRef,
    t: [ObjId; 2],
    t_s: [SlotRef; 2],
}

impl Rig {
    fn new(harts: u16) -> Self {
        let mut m = Machine::with_harts(harts);
        let c0 = m.add_cnode(128);
        let ctl = m.add_kernel_object(ObjType::SchedControl);
        let ctl_cap = m.root_cap(ctl);
        m.put_cap(c0, S_CTL, ctl_cap).expect("cap SchedControl");
        let ctl_s = slot(c0, S_CTL);

        let ep = m.add_kernel_object(ObjType::Endpoint);
        let ep_cap = m.root_cap(ep);
        m.put_cap(c0, S_EP, ep_cap).expect("cap Endpoint");
        let rep = m.add_kernel_object(ObjType::Reply);
        let rep_cap = m.root_cap(rep);
        m.put_cap(c0, S_REP, rep_cap).expect("cap Reply");
        let ntfn = m.add_kernel_object(ObjType::Notification);
        let ntfn_cap = m.root_cap(ntfn);
        m.put_cap(c0, S_NTFN, ntfn_cap).expect("cap Notification");

        let mut t = [0u32; 2];
        let mut t_s = [slot(c0, S_TCB); 2];
        for i in 0..2 {
            let obj = m.add_kernel_object(ObjType::Tcb);
            let cap = m.root_cap(obj);
            m.put_cap(c0, S_TCB + i, cap).expect("cap TCB");
            t[i] = obj;
            t_s[i] = slot(c0, S_TCB + i);

            let sc = m.add_kernel_object(ObjType::SchedContext);
            let sc_cap = m.root_cap(sc);
            m.put_cap(c0, S_SC + i, sc_cap).expect("cap SchedContext");
            let sc_s = slot(c0, S_SC + i);
            m.sched_control_configure(ctl_s, sc_s, SC_BUDGET, SC_BUDGET)
                .expect("SchedControl.Configure");
            m.set_sched_params(t_s[i], Some(sc_s))
                .expect("set_sched_params (test-only)");
            m.tcb_set_priority(t_s[i], 5, 255).expect("Tcb.SetPriority");
            // Affinity phủ **mọi** hart của mô hình (1 hart ⇒ mask 1, 2 hart ⇒ mask 3).
            let mask = if harts >= 64 {
                u64::MAX
            } else {
                (1u64 << harts) - 1
            };
            m.tcb_set_affinity(t_s[i], mask).expect("Tcb.SetAffinity");
            m.transition_tcb(obj, TcbState::Ready)
                .expect("Inactive→Ready");
        }
        Rig {
            m,
            ep,
            ep_s: slot(c0, S_EP),
            rep,
            rep_s: slot(c0, S_REP),
            ntfn,
            ntfn_s: slot(c0, S_NTFN),
            t,
            t_s,
        }
    }

    fn set_home(&mut self, i: usize, home: u16) {
        self.m.tcb_set_home(self.t_s[i], home).expect("Tcb.SetHome");
    }

    fn st(&self, i: usize) -> TcbState {
        self.m.tcb_state(self.t[i]).expect("tcb_state")
    }

    /// Chạy `t[i]` trên hart `h` (TCB phải `Ready` và `home == h`).
    fn run_on(&mut self, i: usize, h: u16) {
        self.m.set_current_hart(h).expect("set_current_hart");
        let got = self.m.dispatch(h).expect("dispatch");
        assert_eq!(got, Some(self.t[i]), "hart {h} phải chạy t{i}");
    }

    fn kiem(&self, buoc: &str) {
        self.m
            .check_sched()
            .unwrap_or_else(|e| panic!("check_sched sau {buoc}: {e}"));
        self.m
            .check_mailboxes()
            .unwrap_or_else(|e| panic!("check_mailboxes sau {buoc}: {e}"));
        self.m
            .check_ipc()
            .unwrap_or_else(|e| panic!("check_ipc sau {buoc}: {e}"));
        self.m
            .check_reply_links()
            .unwrap_or_else(|e| panic!("check_reply_links sau {buoc}: {e}"));
        self.m
            .check_notifications()
            .unwrap_or_else(|e| panic!("check_notifications sau {buoc}: {e}"));
    }

    /// Đòi: TCB `t[i]` **không** ở hàng hart hiện tại, và hộp thư hart `home` có đúng một
    /// `Wake(t[i])`.
    fn doi_wake_ve_home(&self, i: usize, home: u16, buoc: &str) {
        assert!(
            !self.m.ready_contains(0, self.t[i]),
            "{buoc}: TCB home={home} KHÔNG được nằm trong ready queue hart 0"
        );
        assert_eq!(
            self.m.mail_len(home).expect("mail_len"),
            1,
            "{buoc}: hộp thư hart {home} phải có đúng một message `Wake`"
        );
        assert_eq!(
            self.m.mail_items(home).expect("mail_items").next(),
            Some(kc::MailboxMsg::Wake(self.t[i])),
            "{buoc}: message phải là `Wake(t{i})`"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// (1) Đường **Reply** — `reply_with` đánh thức caller `BlockedOnReply` có `home` khác hart.
// ---------------------------------------------------------------------------------------------

#[test]
fn kt154_core_reply_cheo_hart_phai_gui_wake() {
    let mut r = Rig::new(2);
    r.set_home(0, 1); // client home 1
    r.set_home(1, 0); // server home 0

    // Client chạy trên hart 1, `Call` vào Endpoint không có waiter ⇒ `BlockedOnSend`.
    r.run_on(0, 1);
    r.m.call_msg(r.t[0], r.ep_s, msg(1, &[7]))
        .expect("Call trên hart 1");
    assert_eq!(
        r.st(0),
        TcbState::BlockedOnSend {
            ep: r.ep,
            call: true,
            fault: None
        },
        "Call không waiter ⇒ BlockedOnSend"
    );

    // Server chạy trên hart 0, `Recv` lấy message ⇒ client `BlockedOnReply`.
    r.run_on(1, 0);
    r.m.recv(r.t[1], r.ep_s, Some(r.rep_s))
        .expect("Recv trên hart 0");
    assert_eq!(
        r.st(0),
        TcbState::BlockedOnReply { reply: r.rep },
        "Recv có Reply cap ⇒ caller BlockedOnReply"
    );
    r.kiem("chuẩn bị");

    // Đường **không phải** `wake()`: `Reply` từ hart 0 đánh thức caller home=1.
    r.m.reply_msg(r.t[1], r.rep_s, msg(2, &[9]))
        .expect("Reply trên hart 0");

    r.doi_wake_ve_home(0, 1, "Reply chéo hart");
    r.kiem("Reply chéo hart");

    // Hart `home` xử lý hộp thư ⇒ TCB thực sự chạy trên hart 1.
    assert_eq!(r.m.process_mail(1).expect("process_mail"), 1);
    assert_eq!(
        r.m.mail_log(1).expect("mail_log"),
        &[kc::MailboxMsg::Wake(r.t[0])],
        "đúng một lần giao `Wake`"
    );
    assert_eq!(r.m.current(1), Some(r.t[0]), "TCB phải chạy trên hart 1");
    assert_eq!(r.st(0), TcbState::Running { hart: 1 });
    r.kiem("process_mail");
}

// ---------------------------------------------------------------------------------------------
// (2) Đường **Notification signal** — `notification_signal` đánh thức waiter `home` khác hart.
// ---------------------------------------------------------------------------------------------

#[test]
fn kt154_core_notification_cheo_hart_phai_gui_wake() {
    let mut r = Rig::new(2);
    r.set_home(0, 1);
    r.set_home(1, 0);

    // Client chạy trên hart 1, `Wait` trên Notification rỗng ⇒ `BlockedOnNotification`.
    r.run_on(0, 1);
    r.m.recv(r.t[0], r.ntfn_s, None).expect("Wait Notification");
    assert_eq!(
        r.st(0),
        TcbState::BlockedOnNotification { ntfn: r.ntfn },
        "Wait trên Notification rỗng ⇒ BlockedOnNotification"
    );

    // Hart 0 signal — đường **không phải** `wake()`.
    r.m.set_current_hart(0).expect("hart 0");
    r.m.notification_signal(r.ntfn, 0b1010)
        .expect("Signal trên hart 0");

    r.doi_wake_ve_home(0, 1, "Signal chéo hart");
    r.kiem("Signal chéo hart");

    assert_eq!(r.m.process_mail(1).expect("process_mail"), 1);
    assert_eq!(
        r.m.mail_log(1).expect("mail_log"),
        &[kc::MailboxMsg::Wake(r.t[0])]
    );
    assert_eq!(r.m.current(1), Some(r.t[0]));
    assert_eq!(r.st(0), TcbState::Running { hart: 1 });
    r.kiem("process_mail");
}

// ---------------------------------------------------------------------------------------------
// (3) Đường **Recv giao message** — `deliver_queued` đánh thức sender `BlockedOnSend`.
// ---------------------------------------------------------------------------------------------

#[test]
fn kt154_core_recv_giao_message_cheo_hart_phai_gui_wake() {
    let mut r = Rig::new(2);
    r.set_home(0, 1); // sender home 1
    r.set_home(1, 0); // receiver home 0

    // Sender chạy trên hart 1, `Send` chặn vào Endpoint không waiter ⇒ `BlockedOnSend`.
    r.run_on(0, 1);
    r.m.send(r.t[0], r.ep_s, msg(3, &[11]))
        .expect("Send trên hart 1");
    assert_eq!(
        r.st(0),
        TcbState::BlockedOnSend {
            ep: r.ep,
            call: false,
            fault: None
        },
        "Send chặn ⇒ BlockedOnSend"
    );

    // Receiver chạy trên hart 0, `Recv` lấy message ⇒ sender được đánh thức (deliver_queued).
    r.run_on(1, 0);
    r.m.recv(r.t[1], r.ep_s, None).expect("Recv trên hart 0");

    r.doi_wake_ve_home(0, 1, "Recv giao message chéo hart");
    r.kiem("Recv giao message chéo hart");

    assert_eq!(r.m.process_mail(1).expect("process_mail"), 1);
    assert_eq!(
        r.m.mail_log(1).expect("mail_log"),
        &[kc::MailboxMsg::Wake(r.t[0])]
    );
    assert_eq!(r.m.current(1), Some(r.t[0]));
    r.kiem("process_mail");
}

// ---------------------------------------------------------------------------------------------
// (4) Cùng hart — **không** gửi message (thiết kế §4.8), trạng thái `Ready` ngay.
// ---------------------------------------------------------------------------------------------

#[test]
fn kt154_core_cung_hart_khong_gui_wake() {
    let mut r = Rig::new(2);
    r.set_home(0, 1);
    r.set_home(1, 0);

    r.run_on(0, 1);
    r.m.recv(r.t[0], r.ntfn_s, None).expect("Wait Notification");
    assert_eq!(r.st(0), TcbState::BlockedOnNotification { ntfn: r.ntfn });

    // Signal **trên đúng hart home** ⇒ đánh thức trực tiếp, không qua hộp thư.
    r.m.set_current_hart(1).expect("hart 1");
    r.m.notification_signal(r.ntfn, 1)
        .expect("Signal cùng hart");
    assert_eq!(r.st(0), TcbState::Ready, "cùng hart ⇒ Ready ngay");
    assert!(r.m.ready_contains(1, r.t[0]), "phải vào hàng hart home");
    assert_eq!(
        r.m.mail_len(1).expect("mail_len"),
        0,
        "cùng hart ⇒ KHÔNG gửi message"
    );
    r.kiem("Signal cùng hart");
}

// ---------------------------------------------------------------------------------------------
// (5) `n_harts == 1` — **không** gửi message (giữ nguyên ngữ nghĩa cũ).
// ---------------------------------------------------------------------------------------------

#[test]
fn kt154_core_mot_hart_khong_gui_wake() {
    let mut r = Rig::new(1);
    r.set_home(0, 0);
    r.set_home(1, 0);

    r.run_on(0, 0);
    r.m.recv(r.t[0], r.ntfn_s, None).expect("Wait Notification");
    assert_eq!(r.st(0), TcbState::BlockedOnNotification { ntfn: r.ntfn });

    r.m.notification_signal(r.ntfn, 1).expect("Signal");
    assert_eq!(r.st(0), TcbState::Ready);
    assert!(r.m.ready_contains(0, r.t[0]));
    assert_eq!(
        r.m.mail_len(0).expect("mail_len"),
        0,
        "một hart ⇒ KHÔNG gửi message"
    );
    r.kiem("một hart");
}

// ---------------------------------------------------------------------------------------------
// (6) `Reply` cùng hart — không message, `Ready` ngay (đối chứng cho ca (1)).
// ---------------------------------------------------------------------------------------------

#[test]
fn kt154_core_reply_cung_hart_khong_gui_wake() {
    let mut r = Rig::new(2);
    r.set_home(0, 0);
    r.set_home(1, 0);

    r.run_on(0, 0);
    r.m.call_msg(r.t[0], r.ep_s, msg(1, &[7])).expect("Call");
    r.run_on(1, 0);
    r.m.recv(r.t[1], r.ep_s, Some(r.rep_s)).expect("Recv");
    assert_eq!(r.st(0), TcbState::BlockedOnReply { reply: r.rep });

    r.m.reply_msg(r.t[1], r.rep_s, msg(2, &[9])).expect("Reply");
    assert_eq!(r.st(0), TcbState::Ready, "cùng hart ⇒ Ready ngay");
    assert!(r.m.ready_contains(0, r.t[0]));
    assert_eq!(
        r.m.mail_len(0).expect("mail_len"),
        0,
        "cùng hart ⇒ KHÔNG gửi message"
    );
    r.kiem("Reply cùng hart");
}

// ---------------------------------------------------------------------------------------------
// (7) `KError` không đổi: `wake` trên TCB `Inactive` vẫn `IllegalOperation`.
// ---------------------------------------------------------------------------------------------

#[test]
fn kt154_core_wake_inactive_van_loi() {
    let mut r = Rig::new(2);
    let t2 = r.m.add_kernel_object(ObjType::Tcb);
    r.m.set_current_hart(0).expect("hart 0");
    assert_eq!(r.m.wake(t2), Err(KError::IllegalOperation));
}
