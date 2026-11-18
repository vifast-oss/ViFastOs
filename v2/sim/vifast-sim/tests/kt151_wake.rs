// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-151 (N7, D2-131) — **đánh thức TCB chéo hart đúng một lần qua hộp thư**.
//!
//! N7 là nợ **cấu trúc** của v1: ba port có ba kênh đánh thức khác nhau (riscv64 doorbell
//! riêng cho reschedule; arm/x86 dùng mailbox chung). v2 thống nhất qua hộp thư theo hart
//! (`MailboxMsg::Wake`/`Reschedule`/`Migrate`), nhưng trước KT-151 **chưa có test chạy** cho
//! đường chéo hart. Tệp này là test đó, chạy trên **mô hình kernel** (`vifast-core`) và
//! **sim** (oracle) — không QEMU, không thread.
//!
//! Mỗi ca đòi **đúng một** lần giao:
//! - `wake(tcb)` từ hart khác **không** đổi trạng thái TCB ngay; nó để **đúng một**
//!   `MailboxMsg::Wake` trong hộp thư của `home`;
//! - hart đích `process_mail` ⇒ TCB `Ready`/`Running` và **chạy thật** (`current(home)`);
//! - `process_mail` lần hai **không** giao lại (nhật ký vẫn đúng một `Wake`);
//! - `Reschedule`/`Migrate` đi cùng kênh hộp thư, mỗi message xử lý đúng một lần.
//!
//! Bất biến kiểm sau mỗi bước: `check_sched` + `check_mailboxes` (+ `check_ipc` cho core).

use vifast_core as kc;
use vifast_sim as vs;

// ---------------------------------------------------------------------------------------------
// Hằng test (không phải ngưỡng): CNode 16 slot, SC budget = period = 64.
// ---------------------------------------------------------------------------------------------

const S_UNTYPED: usize = 0;
const S_SC: usize = 4;
const S_CTL: usize = 5;
const S_T0: usize = 8;
const SC_BUDGET: u64 = 64;

fn slot(cnode: u32, s: usize) -> kc::SlotRef {
    kc::SlotRef::new(cnode, s)
}

fn slot_sim(cnode: u32, s: usize) -> vs::kobj::SlotRef {
    vs::kobj::SlotRef::new(cnode, s)
}

// ---------------------------------------------------------------------------------------------
// Rig core: 2 hart, 1 TCB ở slot 8.
// ---------------------------------------------------------------------------------------------

struct CoreRig {
    m: kc::Machine,
    t0: kc::ObjId,
    s0: kc::SlotRef,
}

impl CoreRig {
    fn new(harts: u16) -> Self {
        let mut m = kc::Machine::with_harts(harts);
        let c0 = m.add_cnode(16);
        let u0 = m.add_untyped(0x1000, 1 << 16);
        let sc = m.add_kernel_object(kc::ObjType::SchedContext);
        let ctl = m.add_kernel_object(kc::ObjType::SchedControl);
        for (i, obj) in [(S_UNTYPED, u0), (S_SC, sc), (S_CTL, ctl)] {
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
            s0: slot(c0, S_T0),
        }
    }

    /// TCB sẵn sàng trên `home`, ưu tiên 5, affinity cả hai hart.
    fn ready(&mut self, home: u16) {
        self.m.tcb_set_priority(self.s0, 5, 255).expect("prio");
        self.m.tcb_set_affinity(self.s0, 0b11).expect("affinity");
        self.m.tcb_set_home(self.s0, home).expect("home");
        self.m
            .transition_tcb(self.t0, kc::TcbState::Ready)
            .expect("Inactive→Ready");
    }

    fn block(&mut self) {
        self.m
            .transition_tcb(
                self.t0,
                kc::TcbState::BlockedOnAddress {
                    frame: 1,
                    offset: 0,
                },
            )
            .expect("Ready→BlockedOnAddress");
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
    }
}

// ---------------------------------------------------------------------------------------------
// Rig sim (oracle): cùng hình dạng.
// ---------------------------------------------------------------------------------------------

struct SimRig {
    m: vs::kobj::Machine,
    t0: vs::kobj::ObjId,
    s0: vs::kobj::SlotRef,
}

impl SimRig {
    fn new(harts: u16) -> Self {
        let mut m = vs::kobj::Machine::with_harts(harts);
        let c0 = m.add_cnode(16);
        let u0 = m.add_untyped(0x1000, 1 << 16);
        let sc = m.add_kernel_object(vs::kobj::ObjType::SchedContext);
        let ctl = m.add_kernel_object(vs::kobj::ObjType::SchedControl);
        for (i, obj) in [(S_UNTYPED, u0), (S_SC, sc), (S_CTL, ctl)] {
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
            s0: slot_sim(c0, S_T0),
        }
    }

    fn ready(&mut self, home: u16) {
        self.m.tcb_set_priority(self.s0, 5, 255).expect("prio");
        self.m.tcb_set_affinity(self.s0, 0b11).expect("affinity");
        self.m.tcb_set_home(self.s0, home).expect("home");
        self.m
            .transition_tcb(self.t0, vs::kobj::TcbState::Ready)
            .expect("Inactive→Ready");
    }

    fn block(&mut self) {
        self.m
            .transition_tcb(
                self.t0,
                vs::kobj::TcbState::BlockedOnAddress {
                    frame: 1,
                    offset: 0,
                },
            )
            .expect("Ready→BlockedOnAddress");
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
    }
}

// ---------------------------------------------------------------------------------------------
// (b) Ca chính — wake chéo hart: đúng một lần giao, TCB thật sự chạy.
// ---------------------------------------------------------------------------------------------

#[test]
fn kt151_core_wake_cheo_hart_dung_mot_lan_va_tcb_chay() {
    let mut r = CoreRig::new(2);
    r.ready(1);
    r.block();
    r.kiem("chuẩn bị");

    // Hart 0 đánh thức TCB có `home = 1`: **không** ghi trạng thái hart khác (luật 3 §4.16).
    r.m.set_current_hart(0).expect("hart 0");
    r.m.wake(r.t0).expect("wake chéo hart");
    assert_eq!(
        r.m.mail_len(1).expect("mail_len"),
        1,
        "phải có đúng một message trong hộp thư hart 1"
    );
    assert!(
        r.m.mail_log(1).expect("mail_log").is_empty(),
        "message chưa được xử lý thì nhật ký phải rỗng"
    );
    assert_eq!(
        r.m.tcb_state(r.t0).expect("state"),
        kc::TcbState::BlockedOnAddress {
            frame: 1,
            offset: 0
        },
        "wake chéo hart KHÔNG được đổi trạng thái TCB ngay (phải qua hộp thư)"
    );
    r.kiem("wake chéo hart");

    // Hart đích xử lý hộp thư: TCB `Ready` rồi **chạy thật** (ưu tiên 5 > cur_prio 0).
    assert_eq!(r.m.process_mail(1).expect("process_mail"), 1);
    assert_eq!(r.m.mail_len(1).expect("mail_len"), 0);
    assert_eq!(
        r.m.mail_log(1).expect("mail_log"),
        &[kc::MailboxMsg::Wake(r.t0)],
        "đúng một lần giao `Wake`"
    );
    assert_eq!(
        r.m.current(1),
        Some(r.t0),
        "TCB phải thực sự chạy trên hart 1"
    );
    assert_eq!(
        r.m.tcb_state(r.t0).expect("state"),
        kc::TcbState::Running { hart: 1 }
    );
    r.kiem("process_mail");

    // Xử lý lần hai: **không** giao lại, không đổi trạng thái.
    assert_eq!(r.m.process_mail(1).expect("process_mail lần hai"), 0);
    assert_eq!(
        r.m.mail_log(1).expect("mail_log"),
        &[kc::MailboxMsg::Wake(r.t0)],
        "nhật ký vẫn đúng một `Wake` (không giao lại)"
    );
    assert_eq!(r.m.current(1), Some(r.t0));
    r.kiem("process_mail lần hai");
}

#[test]
fn kt151_sim_wake_cheo_hart_dung_mot_lan_va_tcb_chay() {
    let mut r = SimRig::new(2);
    r.ready(1);
    r.block();
    r.kiem("chuẩn bị");

    r.m.set_current_hart(0).expect("hart 0");
    r.m.wake(r.t0).expect("wake chéo hart");
    assert_eq!(r.m.mailbox_len(1).expect("mailbox_len"), 1);
    assert!(r.m.mail_log(1).expect("mail_log").is_empty());
    assert_eq!(
        r.m.tcb_state(r.t0).expect("state"),
        vs::kobj::TcbState::BlockedOnAddress {
            frame: 1,
            offset: 0
        }
    );
    r.kiem("wake chéo hart");

    assert_eq!(r.m.process_mail(1).expect("process_mail"), 1);
    assert_eq!(r.m.mailbox_len(1).expect("mailbox_len"), 0);
    assert_eq!(
        r.m.mail_log(1).expect("mail_log"),
        &[vs::mailbox::MailboxMsg::Wake(r.t0)]
    );
    assert_eq!(r.m.current(1), Some(r.t0));
    assert_eq!(
        r.m.tcb_state(r.t0).expect("state"),
        vs::kobj::TcbState::Running { hart: 1 }
    );
    r.kiem("process_mail");

    assert_eq!(r.m.process_mail(1).expect("process_mail lần hai"), 0);
    assert_eq!(
        r.m.mail_log(1).expect("mail_log"),
        &[vs::mailbox::MailboxMsg::Wake(r.t0)]
    );
    r.kiem("process_mail lần hai");
}

// ---------------------------------------------------------------------------------------------
// Cùng hart: đánh thức trực tiếp, **không** qua hộp thư (thiết kế §4.8).
// ---------------------------------------------------------------------------------------------

#[test]
fn kt151_core_wake_cung_hart_khong_qua_hop_thu() {
    let mut r = CoreRig::new(2);
    r.ready(1);
    r.block();
    r.m.set_current_hart(1).expect("hart 1");
    r.m.wake(r.t0).expect("wake cùng hart");
    assert_eq!(
        r.m.mail_len(1).expect("mail_len"),
        0,
        "cùng hart ⇒ không gửi message"
    );
    assert_eq!(r.m.tcb_state(r.t0).expect("state"), kc::TcbState::Ready);
    r.kiem("wake cùng hart");
}

// ---------------------------------------------------------------------------------------------
// `Reschedule` và `Migrate` đi cùng kênh hộp thư, mỗi message đúng một lần.
// ---------------------------------------------------------------------------------------------

#[test]
fn kt151_core_reschedule_qua_hop_thu_chay_tcb() {
    let mut r = CoreRig::new(2);
    r.ready(1);
    r.m.set_current_hart(0).expect("hart 0");
    r.m.post_mail(1, kc::MailboxMsg::Reschedule)
        .expect("Reschedule vào hộp thư hart 1");
    assert_eq!(r.m.mail_len(1).expect("mail_len"), 1);
    assert_eq!(r.m.process_mail(1).expect("process_mail"), 1);
    assert_eq!(
        r.m.mail_log(1).expect("mail_log"),
        &[kc::MailboxMsg::Reschedule]
    );
    assert_eq!(r.m.current(1), Some(r.t0), "Reschedule phải dispatch TCB");
    r.kiem("Reschedule");
}

#[test]
fn kt151_core_migrate_qua_hop_thu_dung_mot_lan() {
    let mut r = CoreRig::new(2);
    r.ready(0);
    r.m.set_current_hart(0).expect("hart 0");
    r.m.post_mail(
        1,
        kc::MailboxMsg::Migrate {
            tcb: r.t0,
            to_hart: 1,
        },
    )
    .expect("Migrate vào hộp thư hart 1");
    assert_eq!(r.m.mail_len(1).expect("mail_len"), 1);
    assert_eq!(r.m.process_mail(1).expect("process_mail"), 1);
    assert_eq!(
        r.m.mail_log(1).expect("mail_log"),
        &[kc::MailboxMsg::Migrate {
            tcb: r.t0,
            to_hart: 1
        }]
    );
    assert_eq!(r.m.tcb_info_obj(r.t0).expect("info").home, 1);
    assert!(r.m.ready_contains(1, r.t0), "TCB phải vào hàng hart 1");
    assert_eq!(r.m.migrate_count(), 1, "di trú đúng một lần");
    r.kiem("Migrate");
}
