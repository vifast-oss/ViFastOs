// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-155 (N7-4) — **kernel**: `wake_addr` chéo hart phải đánh thức hart `home`.
//!
//! Trước KT-155, `Machine::wake_addr` (`core_sync.rs`) gọi `wake_step(tcb, true)` **trực
//! tiếp** cho waiter bất kể `home`: waiter `BlockedOnAddress` ở hart khác được đưa về `Ready`
//! (vào hàng của `home` nhờ `sched_sync`) nhưng hart `home` **không** nhận
//! `MailboxMsg::Wake` ⇒ trên SMP thật nó có thể không bao giờ chạy. Đây là nợ **N7-4**.
//!
//! KT-155 thêm `Machine::wake_from_unlink` (như `wake_from` nhưng `unlink = true` — cần gỡ
//! bucket WaitAddr) và cho `wake_addr` đi qua cửa đó. Tệp này là test **đỏ-trước** của vế
//! kernel: trên mã chưa vá, hộp thư hart `home` **rỗng** (`left: 0 right: 1`).

use vifast_core::mailbox::MailboxMsg;
use vifast_core::{Machine, ObjId, ObjType, SlotRef, TcbState};

const S_TCB: usize = 10;
const S_SC: usize = 70;
const S_CTL: usize = 120;
const SC_BUDGET: u64 = 64;

fn slot(cnode: u32, s: usize) -> SlotRef {
    SlotRef::new(cnode, s)
}

struct Rig {
    m: Machine,
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
            let mask = if harts >= 64 {
                u64::MAX
            } else {
                (1u64 << harts) - 1
            };
            m.tcb_set_affinity(t_s[i], mask).expect("Tcb.SetAffinity");
            m.start_tcb(obj).expect("Inactive→Ready");
        }
        Rig { m, t, t_s }
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

#[test]
fn kt155_core_wake_addr_cheo_hart_phai_gui_wake() {
    let mut r = Rig::new(2);
    r.m.tcb_set_home(r.t_s[0], 0).expect("home 0");
    r.m.tcb_set_home(r.t_s[1], 1).expect("home 1");
    // Khoá WaitAddr chỉ là `ObjId` (không cần object Frame); `addr_word` trả 0 cho khoá lạ.
    let frame = r.t[0];
    r.m.set_current_hart(1).expect("hart 1");
    r.m.wait_addr(r.t_s[1], frame, 0, 0).expect("wait_addr");
    r.kiem("wait_addr");
    assert!(
        matches!(
            r.m.tcb_state(r.t[1]).expect("state"),
            TcbState::BlockedOnAddress { .. }
        ),
        "waiter phải `BlockedOnAddress`"
    );

    // Đánh thức từ hart 0 (khác `home` = 1) ⇒ phải gửi `Wake` về hart 1.
    r.m.set_current_hart(0).expect("hart 0");
    assert_eq!(r.m.wake_addr(frame, 0, u32::MAX).expect("wake_addr"), 1);
    r.kiem("wake_addr");
    assert_eq!(
        r.m.tcb_state(r.t[1]).expect("state"),
        TcbState::Ready,
        "wake_addr phải đưa waiter về Ready"
    );
    assert_eq!(
        r.m.mail_len(1).expect("mail_len"),
        1,
        "wake_addr chéo hart: hộp thư hart 1 phải có đúng một Wake"
    );
    assert_eq!(
        r.m.mail_items(1).expect("mail_items").next(),
        Some(MailboxMsg::Wake(r.t[1])),
        "wake_addr chéo hart: message phải là Wake(waiter)"
    );
}

#[test]
fn kt155_core_wake_addr_cung_hart_khong_gui_wake() {
    let mut r = Rig::new(2);
    r.m.tcb_set_home(r.t_s[0], 0).expect("home 0");
    r.m.tcb_set_home(r.t_s[1], 0).expect("home 0");
    let frame = r.t[0];
    r.m.set_current_hart(0).expect("hart 0");
    r.m.wait_addr(r.t_s[1], frame, 0, 0).expect("wait_addr");
    assert_eq!(r.m.wake_addr(frame, 0, u32::MAX).expect("wake_addr"), 1);
    r.kiem("wake_addr cùng hart");
    assert_eq!(r.m.tcb_state(r.t[1]).expect("state"), TcbState::Ready);
    assert_eq!(
        r.m.mail_len(0).expect("mail_len"),
        0,
        "cùng hart ⇒ không gửi message"
    );
}
