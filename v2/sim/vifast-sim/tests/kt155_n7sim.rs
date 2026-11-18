// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-155 (N7-2b) — **sim**: mọi đường đánh thức TCB phải tôn trọng hart `home`.
//!
//! KT-154 vá **kernel** (15/16 đường qua `Machine::wake_from`); sim — **oracle** — còn **13**
//! đường `blocked/sleeping → Ready` gọi `transition_tcb` trực tiếp, nên `--strict` còn 2 chỗ
//! (`tcb_resume_with_regs`, `process_refills`) và 11 đường IPC/Reply/Notification/finalise
//! **không** gửi `MailboxMsg::Wake` về `home`. KT-155 vá sim **đối xứng** kernel: thêm
//! `Machine::wake_from` (sim) + `wake_step(tcb, unlink)`; 13 đường đi qua cửa đó.
//!
//! Tệp này là test **đỏ-trước** của vế sim: mỗi ca đòi, sau một đường **không phải** `wake()`
//! (Reply / fault-resume / refill), TCB có `home` khác hart hiện tại:
//! - **không** nằm trong ready queue của hart hiện tại;
//! - hộp thư của hart `home` có **đúng một** `MailboxMsg::Wake(tcb)`;
//! - hart `home` xử lý hộp thư ⇒ TCB `Ready`/`Running` trên hart `home`.
//!
//! Ca cùng hart giữ nguyên ngữ nghĩa cũ: **không** gửi message.

use vifast_sim::ipc::{Message, MessageInfo};
use vifast_sim::kobj::{Machine, ObjId, ObjType, SlotRef, TcbState};
use vifast_sim::mailbox::MailboxMsg;

// ---------------------------------------------------------------------------------------------
// Hằng test (không phải ngưỡng): CNode 128 slot, SC budget = 2, period = 8.
// ---------------------------------------------------------------------------------------------

const S_EP: usize = 0;
const S_REP: usize = 1;
const S_TCB: usize = 10;
const S_SC: usize = 70;
const S_CTL: usize = 120;
const SC_BUDGET: u64 = 2;
const SC_PERIOD: u64 = 8;

fn slot(cnode: ObjId, s: usize) -> SlotRef {
    SlotRef::new(cnode, s)
}

/// `Message` với `len = words.len()`, không cap.
fn msg(label: u32, words: &[u64]) -> Message {
    let info = MessageInfo::new(label, 0, false, 0, words.len() as u8).expect("MessageInfo hợp lệ");
    Message::new(info, words).expect("Message hợp lệ")
}

// ---------------------------------------------------------------------------------------------
// Rig: 2 hart, 2 TCB (t0 = server, t1 = client), Endpoint + Reply + SC riêng cho mỗi TCB.
// ---------------------------------------------------------------------------------------------

struct Rig {
    m: Machine,
    ep: SlotRef,
    rep: SlotRef,
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
            m.sched_control_configure(ctl_s, sc_s, SC_BUDGET, SC_PERIOD)
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
            ep: slot(c0, S_EP),
            rep: slot(c0, S_REP),
            t,
            t_s,
        }
    }

    fn home(&mut self, i: usize, h: u16) {
        self.m.tcb_set_home(self.t_s[i], h).expect("Tcb.SetHome");
    }

    fn state(&self, i: usize) -> TcbState {
        self.m.tcb_state(self.t[i]).expect("tcb_state")
    }

    fn kiem(&self, buoc: &str) {
        self.m
            .check_sched()
            .unwrap_or_else(|e| panic!("check_sched sau {buoc}: {e}"));
        self.m
            .check_smp()
            .unwrap_or_else(|e| panic!("check_smp sau {buoc}: {e}"));
        self.m
            .check_mailboxes()
            .unwrap_or_else(|e| panic!("check_mailboxes sau {buoc}: {e}"));
        self.m
            .check_ipc()
            .unwrap_or_else(|e| panic!("check_ipc sau {buoc}: {e}"));
    }
}

// ---------------------------------------------------------------------------------------------
// (1) `reply_with` — server trả lời caller có `home` khác hart ⇒ `Wake` về `home`.
// ---------------------------------------------------------------------------------------------

#[test]
fn kt155_sim_reply_cheo_hart_phai_gui_wake() {
    let mut r = Rig::new(2);
    r.home(0, 0); // server ở hart 0
    r.home(1, 1); // client ở hart 1
    r.m.set_current_hart(0).expect("hart 0");
    r.m.recv(r.t[0], r.ep, Some(r.rep)).expect("Recv");
    r.kiem("Recv");

    // Client gọi từ hart 1 ⇒ server (hart 0) được đánh thức qua `call_msg` (cũng là
    // `wake_from`): hộp thư hart 0 có `Wake(t0)`. Xử lý ngay để cô lập phép đo sau.
    r.m.set_current_hart(1).expect("hart 1");
    r.m.call_msg(r.t[1], r.ep, msg(1, &[7])).expect("Call");
    r.kiem("Call");
    assert!(
        matches!(r.state(1), TcbState::BlockedOnReply { .. }),
        "client phải `BlockedOnReply` sau Call"
    );
    assert_eq!(
        r.m.mail_len(0).expect("mail_len"),
        1,
        "Call chéo hart: hộp thư hart 0 phải có Wake(server)"
    );
    assert_eq!(r.m.process_mail(0).expect("process_mail"), 1);
    assert_eq!(r.m.mail_len(0).expect("mail_len"), 0);

    // Server trả lời từ hart 0 ⇒ caller (home 1) phải nhận `Wake` ở hộp thư hart 1.
    r.m.set_current_hart(0).expect("hart 0");
    r.m.reply(r.t[0], r.rep).expect("Reply");
    r.kiem("Reply");
    assert_eq!(r.state(1), TcbState::Ready, "Reply phải đánh thức caller");
    assert_eq!(
        r.m.mail_len(1).expect("mail_len"),
        1,
        "Reply chéo hart: hộp thư hart 1 phải có đúng một Wake"
    );
    assert_eq!(
        r.m.mail_items(1).expect("mail_items"),
        vec![MailboxMsg::Wake(r.t[1])],
        "Reply chéo hart: message phải là Wake(client)"
    );

    // Hart `home` xử lý hộp thư ⇒ client **chạy thật** trên hart 1.
    assert_eq!(r.m.process_mail(1).expect("process_mail"), 1);
    assert_eq!(r.m.current(1), Some(r.t[1]), "client phải chạy trên hart 1");
    r.kiem("process_mail");
}

// ---------------------------------------------------------------------------------------------
// (2) `tcb_resume_with_regs` — fault-resume caller có `home` khác hart ⇒ `Wake` về `home`.
// ---------------------------------------------------------------------------------------------

#[test]
fn kt155_sim_resume_cheo_hart_phai_gui_wake() {
    let mut r = Rig::new(2);
    r.home(0, 0);
    r.home(1, 1);
    r.m.set_current_hart(0).expect("hart 0");
    r.m.recv(r.t[0], r.ep, Some(r.rep)).expect("Recv");
    r.m.set_current_hart(1).expect("hart 1");
    r.m.call_msg(r.t[1], r.ep, msg(1, &[7])).expect("Call");
    assert_eq!(r.m.process_mail(0).expect("process_mail"), 1);
    r.kiem("Call");

    // Fault-resume caller từ hart 0 (khác `home` = 1).
    r.m.set_current_hart(0).expect("hart 0");
    r.m.tcb_resume_with_regs(r.t_s[1], msg(0, &[0xAA, 0xBB]))
        .expect("tcb_resume_with_regs");
    r.kiem("tcb_resume_with_regs");
    assert_eq!(
        r.state(1),
        TcbState::Ready,
        "resume phải đưa caller về Ready"
    );
    assert_eq!(
        r.m.mail_len(1).expect("mail_len"),
        1,
        "resume chéo hart: hộp thư hart 1 phải có đúng một Wake"
    );
    assert_eq!(
        r.m.mail_items(1).expect("mail_items"),
        vec![MailboxMsg::Wake(r.t[1])]
    );
}

// ---------------------------------------------------------------------------------------------
// (3) `process_refills` — TCB `Sleeping{Budget}` có `home` khác hart ⇒ `Wake` về `home`.
// ---------------------------------------------------------------------------------------------

#[test]
fn kt155_sim_refill_cheo_hart_phai_gui_wake() {
    let mut r = Rig::new(2);
    r.home(0, 0);
    r.home(1, 1);
    // TCB 1 chạy trên hart 1 (home = 1) tới khi cạn budget ⇒ `Sleeping{Budget}`.
    r.m.set_current_hart(1).expect("hart 1");
    assert_eq!(r.m.dispatch(1).expect("dispatch"), Some(r.t[1]));
    r.m.tick(SC_BUDGET).expect("tick cạn budget");
    r.kiem("tick cạn budget");
    assert_eq!(
        r.state(1),
        TcbState::Sleeping {
            reason: vifast_sim::kobj::SleepReason::Budget
        },
        "hết budget ⇒ Sleeping{{Budget}}"
    );

    // Refill ở hart 0 (khác `home` = 1) ⇒ phải gửi `Wake` về hart 1.
    r.m.set_current_hart(0).expect("hart 0");
    r.m.tick(SC_PERIOD - SC_BUDGET)
        .expect("tick tới biên refill");
    r.kiem("refill");
    assert_eq!(r.state(1), TcbState::Ready, "refill phải đánh thức TCB");
    assert_eq!(
        r.m.mail_len(1).expect("mail_len"),
        1,
        "refill chéo hart: hộp thư hart 1 phải có đúng một Wake"
    );
    assert_eq!(
        r.m.mail_items(1).expect("mail_items"),
        vec![MailboxMsg::Wake(r.t[1])]
    );
}

// ---------------------------------------------------------------------------------------------
// (4) Cùng hart — **không** gửi message (giữ nguyên ngữ nghĩa cũ).
// ---------------------------------------------------------------------------------------------

#[test]
fn kt155_sim_cung_hart_khong_gui_wake() {
    let mut r = Rig::new(2);
    r.home(0, 0);
    r.home(1, 0); // caller cùng hart 0 với server
    r.m.set_current_hart(0).expect("hart 0");
    r.m.recv(r.t[0], r.ep, Some(r.rep)).expect("Recv");
    r.m.call_msg(r.t[1], r.ep, msg(1, &[7])).expect("Call");
    r.m.reply(r.t[0], r.rep).expect("Reply");
    r.kiem("Reply cùng hart");
    assert_eq!(r.state(1), TcbState::Ready);
    assert_eq!(
        r.m.mail_len(0).expect("mail_len"),
        0,
        "cùng hart ⇒ không gửi message"
    );
}

// ---------------------------------------------------------------------------------------------
// (5) `wake_addr` — waiter `BlockedOnAddress` có `home` khác hart ⇒ `Wake` về `home` (N7-4).
// ---------------------------------------------------------------------------------------------

#[test]
fn kt155_sim_wake_addr_cheo_hart_phai_gui_wake() {
    let mut r = Rig::new(2);
    r.home(0, 0);
    r.home(1, 1);
    // Khoá WaitAddr chỉ là `ObjId` (không cần object Frame); `addr_word` trả 0 cho khoá lạ.
    let frame = r.t[0];
    r.m.set_current_hart(1).expect("hart 1");
    r.m.wait_addr(r.t_s[1], frame, 0, 0).expect("wait_addr");
    r.kiem("wait_addr");
    assert!(
        matches!(r.state(1), TcbState::BlockedOnAddress { .. }),
        "waiter phải `BlockedOnAddress`"
    );

    // Đánh thức từ hart 0 (khác `home` = 1) ⇒ phải gửi `Wake` về hart 1.
    r.m.set_current_hart(0).expect("hart 0");
    assert_eq!(r.m.wake_addr(frame, 0, u32::MAX).expect("wake_addr"), 1);
    r.kiem("wake_addr");
    assert_eq!(
        r.state(1),
        TcbState::Ready,
        "wake_addr phải đưa waiter về Ready"
    );
    assert_eq!(
        r.m.mail_len(1).expect("mail_len"),
        1,
        "wake_addr chéo hart: hộp thư hart 1 phải có đúng một Wake"
    );
    assert_eq!(
        r.m.mail_items(1).expect("mail_items"),
        vec![MailboxMsg::Wake(r.t[1])]
    );
}
