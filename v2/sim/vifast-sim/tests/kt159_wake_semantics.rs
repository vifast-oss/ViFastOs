// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-159 (N7, D2-144) — **ghim hai ngữ nghĩa `wake()` / `wake_from()`**.
//!
//! D2-144 đo được: `wake()` **không có** call site sản phẩm nào — chỉ ảnh test
//! (`vifast-kbin-riscv64/src/ktest_inv.rs:51`, `ktest_user.rs:692`) và test sim. Nhưng nó vẫn
//! là **bẫy** cho người viết sau vì hai hàm **cố ý** khác ngữ nghĩa:
//!
//! | | khác hart | cùng hart |
//! |---|---|---|
//! | `wake()` | **post-only**: gửi `MailboxMsg::Wake` về `home`, **không** đổi trạng thái ngay | `Ready` ngay, **không** message |
//! | `wake_from()` | **post + `Ready` ngay** + gửi `MailboxMsg::Wake` về `home` | `Ready` ngay, **không** message |
//!
//! Vì sao hai ngữ nghĩa (không phải trùng lặp): `wake()` dành cho người gọi **không** giữ
//! bookkeeping nào của TCB (hart `home` tự `wake_step` khi xử lý hộp thư — đúng luật 3: hart
//! này không ghi trạng thái hart khác). `wake_from()` dành cho **đường sản phẩm** đã tự gỡ
//! bookkeeping (waiter khỏi hàng chờ, `Reply.caller`, message khỏi `Endpoint.send`) — trạng
//! thái **phải** nhất quán ngay, hoãn sang hart `home` sẽ vi phạm C2
//! (`Reply.caller ⇔ BlockedOnReply`) và `check_ipc`.
//!
//! Tệp này **ghim** cả ba ca trên **cả hai** tầng (kernel model `vifast-core` + sim oracle):
//!
//! - (a) `wake()` chéo hart ⇒ TCB **không** đổi trạng thái ngay + **đúng một** `Wake` trong
//!   hộp thư `home`; hart `home` xử lý ⇒ `Ready`/`Running` và message được tiêu thụ đúng một lần.
//! - (b) `wake_from()` chéo hart (qua đường sản phẩm `notification_signal`) ⇒ TCB `Ready`
//!   **ngay** + **đúng một** `Wake` trong hộp thư `home`.
//! - (c) cùng hart: **cả hai** ⇒ `Ready` ngay, **không** gửi `Wake`.
//!
//! Bất biến kiểm sau mỗi bước: `check_sched` + `check_mailboxes` + `check_ipc` +
//! `check_notifications`.

use vifast_core as kc;
use vifast_sim as vs;

// ---------------------------------------------------------------------------------------------
// Hằng test (không phải ngưỡng): CNode 16 slot, SC budget = period = 64.
// ---------------------------------------------------------------------------------------------

const S_UNTYPED: usize = 0;
const S_SC: usize = 4;
const S_CTL: usize = 5;
const S_T0: usize = 8;
const S_NTFN: usize = 9;
const SC_BUDGET: u64 = 64;

fn slot(cnode: u32, s: usize) -> kc::SlotRef {
    kc::SlotRef::new(cnode, s)
}

fn slot_sim(cnode: u32, s: usize) -> vs::kobj::SlotRef {
    vs::kobj::SlotRef::new(cnode, s)
}

// ---------------------------------------------------------------------------------------------
// Rig core: 2 hart, 1 TCB (slot 8) + 1 Notification (slot 9).
// ---------------------------------------------------------------------------------------------

struct CoreRig {
    m: kc::Machine,
    t0: kc::ObjId,
    ntfn: kc::ObjId,
    s0: kc::SlotRef,
}

impl CoreRig {
    fn new(harts: u16) -> Self {
        let mut m = kc::Machine::with_harts(harts);
        let c0 = m.add_cnode(16);
        let u0 = m.add_untyped(0x1000, 1 << 16);
        let sc = m.add_kernel_object(kc::ObjType::SchedContext);
        let ctl = m.add_kernel_object(kc::ObjType::SchedControl);
        let ntfn = m.add_kernel_object(kc::ObjType::Notification);
        for (i, obj) in [(S_UNTYPED, u0), (S_SC, sc), (S_CTL, ctl), (S_NTFN, ntfn)] {
            let cap = m.root_cap(obj);
            m.put_cap(c0, i, cap).expect("cap object");
        }
        m.sched_control_configure(slot(c0, S_CTL), slot(c0, S_SC), SC_BUDGET, SC_BUDGET)
            .expect("SchedControl.Configure");
        let tcb = m.add_kernel_object(kc::ObjType::Tcb);
        let cap = m.root_cap(tcb);
        m.put_cap(c0, S_T0, cap).expect("cap TCB");
        m.set_sched_params(slot(c0, S_T0), Some(slot(c0, S_SC)))
            .expect("set_sched_params");
        CoreRig {
            m,
            t0: tcb,
            ntfn,
            s0: slot(c0, S_T0),
        }
    }

    /// TCB sẵn sàng trên `home`, ưu tiên 5, affinity cả hai hart.
    fn ready(&mut self, home: u16) {
        self.m.tcb_set_priority(self.s0, 5, 255).expect("prio");
        self.m.tcb_set_affinity(self.s0, 0b11).expect("affinity");
        self.m.tcb_set_home(self.s0, home).expect("home");
        self.m.start_tcb(self.t0).expect("Inactive→Ready");
    }

    /// Chặn TCB trên Notification (waiter được ghi trên object — bất biến "waiter ⇔ TcbState").
    fn block_ntfn(&mut self) {
        self.m
            .notification_wait(self.t0, self.ntfn)
            .expect("Wait rỗng ⇒ BlockedOnNotification");
        assert_eq!(
            self.m.tcb_state(self.t0).expect("state"),
            kc::TcbState::BlockedOnNotification { ntfn: self.ntfn }
        );
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
            .check_notifications()
            .unwrap_or_else(|e| panic!("check_notifications sau {buoc}: {e}"));
    }
}

// ---------------------------------------------------------------------------------------------
// Rig sim (oracle): cùng hình dạng.
// ---------------------------------------------------------------------------------------------

struct SimRig {
    m: vs::kobj::Machine,
    t0: vs::kobj::ObjId,
    ntfn: vs::kobj::ObjId,
    s0: vs::kobj::SlotRef,
}

impl SimRig {
    fn new(harts: u16) -> Self {
        let mut m = vs::kobj::Machine::with_harts(harts);
        let c0 = m.add_cnode(16);
        let u0 = m.add_untyped(0x1000, 1 << 16);
        let sc = m.add_kernel_object(vs::kobj::ObjType::SchedContext);
        let ctl = m.add_kernel_object(vs::kobj::ObjType::SchedControl);
        let ntfn = m.add_kernel_object(vs::kobj::ObjType::Notification);
        for (i, obj) in [(S_UNTYPED, u0), (S_SC, sc), (S_CTL, ctl), (S_NTFN, ntfn)] {
            let cap = m.root_cap(obj);
            m.put_cap(c0, i, cap).expect("cap object");
        }
        m.sched_control_configure(
            slot_sim(c0, S_CTL),
            slot_sim(c0, S_SC),
            SC_BUDGET,
            SC_BUDGET,
        )
        .expect("SchedControl.Configure");
        let tcb = m.add_kernel_object(vs::kobj::ObjType::Tcb);
        let cap = m.root_cap(tcb);
        m.put_cap(c0, S_T0, cap).expect("cap TCB");
        m.set_sched_params(slot_sim(c0, S_T0), Some(slot_sim(c0, S_SC)))
            .expect("set_sched_params");
        SimRig {
            m,
            t0: tcb,
            ntfn,
            s0: slot_sim(c0, S_T0),
        }
    }

    fn ready(&mut self, home: u16) {
        self.m.tcb_set_priority(self.s0, 5, 255).expect("prio");
        self.m.tcb_set_affinity(self.s0, 0b11).expect("affinity");
        self.m.tcb_set_home(self.s0, home).expect("home");
        self.m.start_tcb(self.t0).expect("Inactive→Ready");
    }

    fn block_ntfn(&mut self) {
        self.m
            .notification_wait(self.t0, self.ntfn)
            .expect("Wait rỗng ⇒ BlockedOnNotification");
        assert_eq!(
            self.m.tcb_state(self.t0).expect("state"),
            vs::kobj::TcbState::BlockedOnNotification { ntfn: self.ntfn }
        );
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
            .check_notifications()
            .unwrap_or_else(|e| panic!("check_notifications sau {buoc}: {e}"));
    }
}

// =============================================================================================
// (a) `wake()` chéo hart ⇒ **post-only**: TCB không đổi trạng thái ngay, đúng một `Wake`.
// =============================================================================================

#[test]
fn kt159_core_wake_cheo_hart_post_only() {
    let mut r = CoreRig::new(2);
    r.ready(1);
    r.block_ntfn();
    r.kiem("chuẩn bị");

    r.m.set_current_hart(0).expect("hart 0");
    r.m.wake(r.t0).expect("wake chéo hart");

    // (a1) **Không** đổi trạng thái ngay — `wake()` chỉ post.
    assert_eq!(
        r.m.tcb_state(r.t0).expect("state"),
        kc::TcbState::BlockedOnNotification { ntfn: r.ntfn },
        "wake() chéo hart KHÔNG được đổi trạng thái TCB ngay (post-only, D2-144)"
    );
    // (a2) **Đúng một** `Wake` trong hộp thư `home`, chưa xử lý.
    assert_eq!(
        r.m.mail_len(1).expect("mail_len"),
        1,
        "phải có đúng một message trong hộp thư hart 1"
    );
    assert!(
        r.m.mail_log(1).expect("mail_log").is_empty(),
        "message chưa xử lý thì nhật ký phải rỗng"
    );
    // (a3) Post-only **không** gỡ bookkeeping: waiter vẫn trên Notification.
    assert_eq!(
        r.m.notification_waiter(r.ntfn).expect("waiter"),
        Some(r.t0),
        "wake() post-only không được gỡ waiter khỏi Notification"
    );
    r.kiem("wake chéo hart");

    // Hart `home` xử lý hộp thư ⇒ TCB `Ready` rồi chạy (ưu tiên 5 > cur_prio 0).
    assert_eq!(r.m.process_mail(1).expect("process_mail"), 1);
    assert_eq!(
        r.m.mail_log(1).expect("mail_log"),
        &[kc::MailboxMsg::Wake(r.t0)],
        "đúng một lần giao `Wake`"
    );
    assert_eq!(
        r.m.tcb_state(r.t0).expect("state"),
        kc::TcbState::Running { hart: 1 },
        "hart home xử lý ⇒ TCB chạy trên hart 1"
    );
    r.kiem("process_mail");
}

#[test]
fn kt159_sim_wake_cheo_hart_post_only() {
    let mut r = SimRig::new(2);
    r.ready(1);
    r.block_ntfn();
    r.kiem("chuẩn bị");

    r.m.set_current_hart(0).expect("hart 0");
    r.m.wake(r.t0).expect("wake chéo hart");

    assert_eq!(
        r.m.tcb_state(r.t0).expect("state"),
        vs::kobj::TcbState::BlockedOnNotification { ntfn: r.ntfn },
        "wake() chéo hart KHÔNG được đổi trạng thái TCB ngay (post-only, D2-144)"
    );
    assert_eq!(r.m.mailbox_len(1).expect("mailbox_len"), 1);
    assert!(r.m.mail_log(1).expect("mail_log").is_empty());
    assert_eq!(r.m.notification_waiter(r.ntfn).expect("waiter"), Some(r.t0));
    r.kiem("wake chéo hart");

    assert_eq!(r.m.process_mail(1).expect("process_mail"), 1);
    assert_eq!(
        r.m.mail_log(1).expect("mail_log"),
        &[vs::mailbox::MailboxMsg::Wake(r.t0)]
    );
    assert_eq!(
        r.m.tcb_state(r.t0).expect("state"),
        vs::kobj::TcbState::Running { hart: 1 }
    );
    r.kiem("process_mail");
}

// =============================================================================================
// (b) `wake_from()` chéo hart ⇒ **post + `Ready` ngay**, đúng một `Wake`.
//     Đường sản phẩm: `notification_signal` gọi `wake_from` (core_sync.rs / kobj.rs).
// =============================================================================================

#[test]
fn kt159_core_wake_from_cheo_hart_ready_ngay() {
    let mut r = CoreRig::new(2);
    r.ready(1);
    r.block_ntfn();
    r.kiem("chuẩn bị");

    r.m.set_current_hart(0).expect("hart 0");
    r.m.notification_signal(r.ntfn, 1).expect("Signal");

    // (b1) `Ready` **ngay** — không chờ hart `home` xử lý hộp thư.
    assert_eq!(
        r.m.tcb_state(r.t0).expect("state"),
        kc::TcbState::Ready,
        "wake_from() chéo hart phải đổi `Ready` NGAY (post + Ready, D2-144)"
    );
    // (b2) **Đúng một** `Wake` trong hộp thư `home` (đánh thức hart đích).
    assert_eq!(
        r.m.mail_len(1).expect("mail_len"),
        1,
        "wake_from() chéo hart vẫn phải gửi đúng một `Wake` về hart home"
    );
    assert!(r.m.mail_log(1).expect("mail_log").is_empty());
    r.kiem("wake_from chéo hart");

    // Hart `home` xử lý: message tiêu thụ đúng một lần, TCB chạy.
    assert_eq!(r.m.process_mail(1).expect("process_mail"), 1);
    assert_eq!(
        r.m.mail_log(1).expect("mail_log"),
        &[kc::MailboxMsg::Wake(r.t0)]
    );
    assert_eq!(
        r.m.tcb_state(r.t0).expect("state"),
        kc::TcbState::Running { hart: 1 }
    );
    r.kiem("process_mail");
}

#[test]
fn kt159_sim_wake_from_cheo_hart_ready_ngay() {
    let mut r = SimRig::new(2);
    r.ready(1);
    r.block_ntfn();
    r.kiem("chuẩn bị");

    r.m.set_current_hart(0).expect("hart 0");
    r.m.notification_signal(r.ntfn, 1).expect("Signal");

    assert_eq!(
        r.m.tcb_state(r.t0).expect("state"),
        vs::kobj::TcbState::Ready,
        "wake_from() chéo hart phải đổi `Ready` NGAY (post + Ready, D2-144)"
    );
    assert_eq!(r.m.mailbox_len(1).expect("mailbox_len"), 1);
    assert!(r.m.mail_log(1).expect("mail_log").is_empty());
    r.kiem("wake_from chéo hart");

    assert_eq!(r.m.process_mail(1).expect("process_mail"), 1);
    assert_eq!(
        r.m.mail_log(1).expect("mail_log"),
        &[vs::mailbox::MailboxMsg::Wake(r.t0)]
    );
    assert_eq!(
        r.m.tcb_state(r.t0).expect("state"),
        vs::kobj::TcbState::Running { hart: 1 }
    );
    r.kiem("process_mail");
}

// =============================================================================================
// (c) Cùng hart: **cả hai** ⇒ `Ready` ngay, **không** gửi `Wake`.
// =============================================================================================

#[test]
fn kt159_core_cung_hart_ca_hai_ready_ngay_khong_wake() {
    let mut r = CoreRig::new(2);
    r.ready(1);
    r.block_ntfn();
    r.m.set_current_hart(1).expect("hart 1");

    // (c1) `wake()` cùng hart ⇒ `Ready` ngay, không message.
    r.m.wake(r.t0).expect("wake cùng hart");
    assert_eq!(
        r.m.tcb_state(r.t0).expect("state"),
        kc::TcbState::Ready,
        "wake() cùng hart ⇒ Ready ngay"
    );
    assert_eq!(
        r.m.mail_len(1).expect("mail_len"),
        0,
        "cùng hart ⇒ KHÔNG gửi `Wake`"
    );
    r.kiem("wake cùng hart");

    // (c2) `wake_from()` cùng hart (qua `notification_signal`) ⇒ `Ready` ngay, không message.
    r.block_ntfn();
    r.m.notification_signal(r.ntfn, 1).expect("Signal");
    assert_eq!(
        r.m.tcb_state(r.t0).expect("state"),
        kc::TcbState::Ready,
        "wake_from() cùng hart ⇒ Ready ngay"
    );
    assert_eq!(
        r.m.mail_len(1).expect("mail_len"),
        0,
        "cùng hart ⇒ KHÔNG gửi `Wake`"
    );
    r.kiem("wake_from cùng hart");
}

#[test]
fn kt159_sim_cung_hart_ca_hai_ready_ngay_khong_wake() {
    let mut r = SimRig::new(2);
    r.ready(1);
    r.block_ntfn();
    r.m.set_current_hart(1).expect("hart 1");

    r.m.wake(r.t0).expect("wake cùng hart");
    assert_eq!(
        r.m.tcb_state(r.t0).expect("state"),
        vs::kobj::TcbState::Ready,
        "wake() cùng hart ⇒ Ready ngay"
    );
    assert_eq!(
        r.m.mailbox_len(1).expect("mailbox_len"),
        0,
        "cùng hart ⇒ KHÔNG gửi `Wake`"
    );
    r.kiem("wake cùng hart");

    r.block_ntfn();
    r.m.notification_signal(r.ntfn, 1).expect("Signal");
    assert_eq!(
        r.m.tcb_state(r.t0).expect("state"),
        vs::kobj::TcbState::Ready,
        "wake_from() cùng hart ⇒ Ready ngay"
    );
    assert_eq!(
        r.m.mailbox_len(1).expect("mailbox_len"),
        0,
        "cùng hart ⇒ KHÔNG gửi `Wake`"
    );
    r.kiem("wake_from cùng hart");
}
