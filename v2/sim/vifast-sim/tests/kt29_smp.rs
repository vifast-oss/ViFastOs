// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-29 — **SMP mô hình tất định 1/2/4/8 hart + push migration** (K1C-PLAN §5 KT-29,
//! **D2-53 Q1/Q2**; thiết kế §4.8 `docs/v2/ViFastOs-v2-thiet-ke.md:336-344`, §4.16 luật 3
//! `:412-419`).
//!
//! **Model-only `[TK]`, không phải spec/ABI; không thread/QEMU.** Mô hình N hart tất định
//! 1/2/4/8 (`MAX_HARTS = 8` `[TK]`): mỗi hart một [`HartSched`] riêng, công bố
//! `Published { idle, cur_prio }`; push **một** TCB cuối hàng mức Ready cao nhất sang hart
//! **rảnh có chỉ số nhỏ nhất** trong affinity, qua hộp thư `MailboxMsg::Migrate`; wake chéo
//! hart qua `MailboxMsg::Wake`, chỉ preempt khi ưu tiên **>** `published.cur_prio` của đích;
//! **không** work stealing; guard luật 3: `publish` chỉ ghi được hart đang chạy.
//!
//! Test đỏ trước (D2-05):
//! - [`JOURNAL/kt29-red-compile.log`](../../agent/JOURNAL/kt29-red-compile.log) — tệp này
//!   **không biên dịch** trước KT-29: `Machine::{with_harts, hart_count > 1, current_hart,
//!   set_current_hart, publish, published, push_migration, migrate_count, mailbox_len,
//!   check_smp, check_harts}`, `Published`, `MAX_HARTS` chưa có ⇒ E0432/E0599.
//! - [`JOURNAL/kt29-red-semantics.log`](../../agent/JOURNAL/kt29-red-semantics.log) — probe
//!   API cũ: `Machine::new().hart_count() == 1`; `dispatch(1)`/`post_mail(1, …)` ⇒
//!   `InvalidArgument`; `post_mail(0, Migrate{..})` ⇒ `NotSupported` — tức mô hình **không
//!   có** hart thứ hai và **không** có Migrate, nên push migration chưa biểu diễn được.
//!
//! Bất biến kiểm **sau mỗi** bước: [`Machine::check_sched`] + [`Machine::check_harts`] +
//! [`Machine::check_ipc`] + [`Machine::check_mailboxes`] (gói trong [`kiem`]).

use vifast_sim::kobj::{KError, Machine, ObjId, ObjType, Published, SlotRef, TcbState, MAX_HARTS};
use vifast_sim::mailbox::MailboxMsg;
use vifast_sim::tcb::SleepReason;
use vifast_sim::Lcg;

fn slot(cnode: ObjId, s: usize) -> SlotRef {
    SlotRef::new(cnode, s)
}

/// Bất biến **sau mỗi** public op: scheduler + SMP (bảo toàn TCB/published/affinity) + IPC +
/// mailbox. Vi phạm là lỗi mô hình, panic ngay (không retry — D2-07).
fn kiem(m: &Machine, buoc: &str) {
    if let Err(e) = m.check_sched() {
        panic!("check_sched sau {buoc}: {e}");
    }
    if let Err(e) = m.check_harts() {
        panic!("check_harts sau {buoc}: {e}");
    }
    if let Err(e) = m.check_ipc() {
        panic!("check_ipc sau {buoc}: {e}");
    }
    if let Err(e) = m.check_mailboxes() {
        panic!("check_mailboxes sau {buoc}: {e}");
    }
}

// ---------------------------------------------------------------------------
// Rig: CNode 16 slot; 1 Untyped, 1 SchedContext, 1 SchedControl; các TCB ở slot 8..;
// SC đã `SchedControl.Configure` (budget = period = 64 — hằng **test**, không phải ngưỡng).
// ---------------------------------------------------------------------------

const S_UNTYPED: usize = 0;
const S_SC: usize = 4;
const S_CTL: usize = 5;
const S_T0: usize = 8;
const SC_MAC_DINH: u64 = 64;

struct Rig {
    m: Machine,
    s: Vec<SlotRef>,
    t: Vec<ObjId>,
}

impl Rig {
    fn new(harts: u16, so_tcb: usize) -> Self {
        let mut m = Machine::with_harts(harts);
        let c0 = m.add_cnode(16);
        let u0 = m.add_untyped(0x1000, 1 << 16);
        let sc = m.add_kernel_object(ObjType::SchedContext);
        let ctl = m.add_kernel_object(ObjType::SchedControl);
        for (i, obj) in [(S_UNTYPED, u0), (S_SC, sc), (S_CTL, ctl)] {
            let cap = m.root_cap(obj);
            m.put_cap(c0, i, cap).expect("cap object");
        }
        m.sched_control_configure(slot(c0, S_CTL), slot(c0, S_SC), SC_MAC_DINH, SC_MAC_DINH)
            .expect("SchedControl.Configure SC mặc định (test-only)");
        let mut s = Vec::new();
        let mut t = Vec::new();
        for i in 0..so_tcb {
            let tcb = m.add_kernel_object(ObjType::Tcb);
            let cap = m.root_cap(tcb);
            m.put_cap(c0, S_T0 + i, cap).expect("cap TCB");
            m.set_sched_params(slot(c0, S_T0 + i), Some(slot(c0, S_SC)))
                .expect("set_sched_params SC mặc định (test-only)");
            s.push(slot(c0, S_T0 + i));
            t.push(tcb);
        }
        Rig { m, s, t }
    }

    fn prio(&mut self, i: usize, p: u8) {
        self.m
            .tcb_set_priority(self.s[i], p, 255)
            .expect("Tcb.SetPriority");
        kiem(&self.m, "Tcb.SetPriority");
    }

    fn affinity(&mut self, i: usize, mask: u64) {
        self.m
            .tcb_set_affinity(self.s[i], mask)
            .expect("Tcb.SetAffinity");
        kiem(&self.m, "Tcb.SetAffinity");
    }

    fn home(&mut self, i: usize, h: u16) {
        self.m.tcb_set_home(self.s[i], h).expect("Tcb.SetHome");
        kiem(&self.m, "Tcb.SetHome");
    }

    /// `Inactive → Ready` (một cửa `transition_tcb`).
    fn ready(&mut self, i: usize) {
        self.m
            .transition_tcb(self.t[i], TcbState::Ready)
            .expect("Inactive→Ready");
        kiem(&self.m, "transition Ready");
    }

    /// TCB i: priority, affinity (phải chứa home), home, rồi `Ready`.
    fn dat(&mut self, i: usize, p: u8, mask: u64, h: u16) {
        self.prio(i, p);
        self.affinity(i, mask);
        self.home(i, h);
        self.ready(i);
    }

    fn dispatch(&mut self, h: u16) -> Option<ObjId> {
        let got = self.m.dispatch(h).expect("dispatch");
        kiem(&self.m, "dispatch");
        got
    }
}

fn dem_ready(m: &Machine, h: u16) -> usize {
    m.ready_len(h)
}

// ---------------------------------------------------------------------------
// 1. `with_harts(1/2/4/8)`; `Machine::new()` vẫn **1 hart** (không đổi 263 test cũ);
//    `n = 0`/`> 8` ⇒ `assert!` như `with_arena_bytes` (DoD 1).
// ---------------------------------------------------------------------------

#[test]
fn with_harts_1_2_4_8_va_mac_dinh_mot_hart() {
    assert_eq!(MAX_HARTS, 8);
    for n in [1u16, 2, 4, 8] {
        let m = Machine::with_harts(n);
        assert_eq!(m.hart_count(), n, "hart_count phải trả đúng n");
        for h in 0..n {
            assert!(m.sched_idle(h), "hart {h} mới phải rảnh");
            assert_eq!(m.sched_cur_prio(h), 0);
            assert_eq!(
                m.published(h).expect("published"),
                Published {
                    idle: true,
                    cur_prio: 0
                }
            );
            assert_eq!(m.mailbox_len(h).expect("mailbox_len"), 0);
        }
    }
    assert_eq!(Machine::new().hart_count(), 1, "mặc định vẫn 1 hart");
    for bad in [0u16, (MAX_HARTS + 1)] {
        let r = std::panic::catch_unwind(|| Machine::with_harts(bad));
        assert!(r.is_err(), "with_harts({bad}) phải assert");
    }
}

// ---------------------------------------------------------------------------
// 2. `Published { idle, cur_prio }` theo hart: tự khớp trạng thái thật sau mỗi op; `publish`
//    chỉ ghi được hart đang chạy (`current_hart()`), `published` **đọc** được từ mọi hart
//    (luật 3) — DoD 2.
// ---------------------------------------------------------------------------

#[test]
fn published_idle_cur_prio_va_guard_luat_3() {
    let mut r = Rig::new(2, 2);
    assert_eq!(r.m.current_hart(), 0, "hart hiện tại mặc định 0");
    assert_eq!(
        r.m.published(0).expect("published 0"),
        Published {
            idle: true,
            cur_prio: 0
        }
    );

    // t0 chạy trên hart 0 mức 7 ⇒ published(0) = busy/mức 7; hart 1 vẫn rảnh.
    r.dat(0, 7, 0b01, 0);
    assert_eq!(r.dispatch(0), Some(r.t[0]));
    assert_eq!(
        r.m.published(0).expect("published 0"),
        Published {
            idle: false,
            cur_prio: 7
        }
    );
    assert_eq!(
        r.m.published(1).expect("published 1"),
        Published {
            idle: true,
            cur_prio: 0
        }
    );

    // TCB ngủ ⇒ hart 0 rảnh lại, published cập nhật.
    r.m.block_current(
        0,
        TcbState::Sleeping {
            reason: SleepReason::Timeout,
        },
    )
    .expect("block_current");
    kiem(&r.m, "block_current");
    assert_eq!(
        r.m.published(0).expect("published 0"),
        Published {
            idle: true,
            cur_prio: 0
        }
    );

    // Tự công bố đúng trạng thái thật của chính hart đang chạy ⇒ Ok.
    r.m.publish(0, true, 0).expect("publish hart hiện tại");

    // Luật 3: ghi `idle`/`cur_prio` của hart **khác** ⇒ `Err(IllegalOperation)`, không đổi.
    r.m.set_current_hart(1).expect("set_current_hart");
    assert_eq!(
        r.m.publish(0, false, 9),
        Err(KError::IllegalOperation),
        "không được ghi trạng thái hart khác"
    );
    assert_eq!(
        r.m.published(0).expect("published 0"),
        Published {
            idle: true,
            cur_prio: 0
        },
        "guard từ chối thì không đổi published"
    );
    // Đọc trường công bố từ hart khác là được phép (luật 3).
    assert_eq!(
        r.m.published(0),
        Ok(Published {
            idle: true,
            cur_prio: 0
        })
    );

    // Hart không tồn tại ⇒ InvalidArgument.
    assert_eq!(
        r.m.set_current_hart(2),
        Err(KError::InvalidArgument),
        "hart ngoài mô hình"
    );
    assert_eq!(r.m.published(2), Err(KError::InvalidArgument));
    assert_eq!(r.m.publish(2, true, 0), Err(KError::InvalidArgument));
}

// ---------------------------------------------------------------------------
// 3. Push đúng **1** TCB/hart **nguồn**/vòng, TCB **cuối hàng** mức Ready cao nhất, sang hart
//    rảnh **nhỏ nhất** trong affinity, qua `Mailbox::Migrate`; `migrate_count` + log hộp thư
//    (DoD 3).
// ---------------------------------------------------------------------------

#[test]
fn push_dung_mot_tcb_cuoi_hang_qua_migrate() {
    let mut r = Rig::new(2, 3);
    for i in 0..3 {
        r.dat(i, 5, 0b11, 0);
    }
    assert_eq!(dem_ready(&r.m, 0), 3);
    assert_eq!(dem_ready(&r.m, 1), 0);

    // Hart 0 có 3 TCB cùng mức 5; hart 1 rảnh ⇒ đẩy **một** TCB **cuối hàng** (t2) sang hart 1.
    assert_eq!(r.m.push_migration().expect("push_migration"), 1);
    assert_eq!(r.m.migrate_count(), 1);
    assert_eq!(r.m.tcb_info(r.s[2]).expect("tcb_info").home, 1);
    assert!(!r.m.ready_contains(0, r.t[2]));
    assert!(r.m.ready_contains(1, r.t[2]), "TCB phải vào hàng hart 1");
    assert_eq!(dem_ready(&r.m, 0), 2);
    assert_eq!(dem_ready(&r.m, 1), 1);
    kiem(&r.m, "push_migration");

    // Đi qua hộp thư: message `Migrate` đã xử lý trong **cùng bước** (không có in-flight),
    // hộp thư rỗng sau drain, log ghi đúng thứ tự FIFO.
    assert_eq!(r.m.mailbox_len(1).expect("mailbox_len"), 0);
    assert_eq!(
        r.m.mail_log(1).expect("mail_log"),
        &[MailboxMsg::Migrate {
            tcb: r.t[2],
            to_hart: 1
        }]
    );

    // Không nhân bản/không mất TCB.
    assert_ne!(r.t[2], r.t[1]);
    assert_ne!(r.t[2], r.t[0]);
}

// ---------------------------------------------------------------------------
// 3b. Drain **FIFO**: `Wake` đến trước `Migrate` trong cùng hộp thư ⇒ log/xử lý đúng thứ tự
//     KT-22 (reviewer-kt29 mục 1); "≤1 TCB/hart/bước" = **mỗi hart nguồn**, một hart đích có
//     thể nhận từ nhiều nguồn trong cùng vòng (reviewer-kt29 mục 4).
// ---------------------------------------------------------------------------

#[test]
fn push_drain_fifo_va_mot_tcb_moi_hart_nguon() {
    // (a) Wake đến trước Migrate trong hộp thư hart 1: drain FIFO ⇒ Wake xử lý trước.
    let mut r = Rig::new(2, 3);
    r.dat(0, 5, 0b11, 1);
    r.m.transition_tcb(
        r.t[0],
        TcbState::BlockedOnAddress {
            frame: 1,
            offset: 0,
        },
    )
    .expect("t0 chặn");
    r.dat(1, 5, 0b11, 0);
    r.dat(2, 5, 0b11, 0);
    r.m.post_mail(1, MailboxMsg::Wake(r.t[0]))
        .expect("Wake vào hộp thư hart 1");
    assert_eq!(r.m.mailbox_len(1).expect("mailbox_len"), 1);
    assert_eq!(r.m.push_migration().expect("push"), 1);
    assert_eq!(
        r.m.mail_log(1).expect("mail_log"),
        &[
            MailboxMsg::Wake(r.t[0]),
            MailboxMsg::Migrate {
                tcb: r.t[2],
                to_hart: 1
            }
        ],
        "Wake đến trước phải được xử lý/ghi log trước Migrate (FIFO)"
    );
    // Wake xử lý trước: t0 hết chặn; ưu tiên 5 > cur_prio 0 của hart 1 rảnh ⇒ preempt luôn.
    assert!(matches!(
        r.m.tcb_state(r.t[0]).expect("state"),
        TcbState::Ready | TcbState::Running { hart: 1 }
    ));
    assert_eq!(
        r.m.current(1),
        Some(r.t[0]),
        "Wake được xử lý trước Migrate"
    );
    assert_eq!(r.m.tcb_info(r.s[2]).expect("tcb_info").home, 1);
    kiem(&r.m, "drain FIFO");

    // (b) Hai hart **nguồn** cùng đẩy sang một hart đích rảnh: đúng 1 TCB/nguồn/vòng, đích
    //     nhận 2 (đây là "≤1 TCB/hart nguồn", không phải "≤1 TCB/đích").
    let mut r = Rig::new(3, 6);
    r.dat(0, 1, 0b111, 0);
    r.m.transition_tcb(r.t[0], TcbState::Running { hart: 0 })
        .expect("t0 chạy hart 0");
    r.dat(1, 5, 0b111, 0);
    r.dat(2, 5, 0b111, 0);
    r.dat(3, 1, 0b111, 1);
    r.m.transition_tcb(r.t[3], TcbState::Running { hart: 1 })
        .expect("t3 chạy hart 1");
    r.dat(4, 5, 0b111, 1);
    r.dat(5, 5, 0b111, 1);
    assert_eq!(r.m.push_migration().expect("push"), 2);
    assert_eq!(r.m.migrate_count(), 2);
    assert_eq!(r.m.ready_len(0), 1, "hart 0 mất đúng 1 TCB");
    assert_eq!(r.m.ready_len(1), 1, "hart 1 mất đúng 1 TCB");
    assert_eq!(r.m.ready_len(2), 2, "hart 2 (đích) nhận 1 từ mỗi nguồn");
    kiem(&r.m, "push 2 nguồn cùng đích");
}

// ---------------------------------------------------------------------------
// 4. `cuối hàng`: hart đích "rảnh" (chưa có `current`) nhưng **đã có** TCB trong hàng ⇒ TCB
//    di trú vào **cuối** bucket mức của nó, không chen đầu (Q2).
// ---------------------------------------------------------------------------

#[test]
fn push_vao_cuoi_hang_cua_dich() {
    let mut r = Rig::new(2, 3);
    // t0 (hart 1, mức 5) đã ở hàng đích trước; t1,t2 (hart 0, mức 5) là nguồn đẩy.
    r.dat(0, 5, 0b11, 1);
    r.dat(1, 5, 0b11, 0);
    r.dat(2, 5, 0b11, 0);
    assert!(
        r.m.sched_idle(1),
        "hart 1 chưa chạy TCB nào ⇒ published idle"
    );
    assert_eq!(dem_ready(&r.m, 1), 1);

    assert_eq!(r.m.push_migration().expect("push"), 1);
    assert_eq!(r.m.ready_len_at(1, 5), 2, "hai TCB cùng mức 5 trên hart 1");
    assert_eq!(
        r.m.sched_pick_on(1),
        Some(r.t[0]),
        "TCB cũ giữ đầu hàng, TCB di trú vào cuối"
    );
    assert_eq!(r.m.tcb_info(r.s[2]).expect("tcb_info").home, 1);
    kiem(&r.m, "push vào cuối hàng");
}

// ---------------------------------------------------------------------------
// 5. Ca âm: hart đích **không** idle ⇒ 0 migrate; ngoài affinity ⇒ 0; nguồn chỉ **1** Ready
//    mức cao nhất ⇒ 0; **không** work stealing (đích rảnh không tự kéo) (DoD 3/4).
// ---------------------------------------------------------------------------

#[test]
fn push_chi_khi_dich_idle_va_trong_affinity() {
    // (a) đích bận: hart 1 đang chạy t0 ⇒ 0 migrate.
    let mut r = Rig::new(2, 3);
    r.dat(0, 3, 0b11, 1);
    assert_eq!(r.dispatch(1), Some(r.t[0]), "hart 1 bận");
    r.dat(1, 5, 0b11, 0);
    r.dat(2, 5, 0b11, 0);
    assert_eq!(r.m.push_migration().expect("push"), 0, "đích bận ⇒ 0");
    assert_eq!(r.m.migrate_count(), 0);
    assert_eq!(dem_ready(&r.m, 0), 2);

    // (b) ngoài affinity: chỉ hart 0 trong affinity ⇒ 0 migrate dù hart 1 rảnh.
    let mut r = Rig::new(2, 2);
    r.dat(0, 5, 0b01, 0);
    r.dat(1, 5, 0b01, 0);
    assert_eq!(r.m.push_migration().expect("push"), 0, "ngoài affinity ⇒ 0");
    assert_eq!(r.m.migrate_count(), 0);

    // (c) nguồn chỉ 1 Ready mức cao nhất ⇒ 0.
    let mut r = Rig::new(2, 2);
    r.dat(0, 9, 0b11, 0);
    r.dat(1, 3, 0b11, 0); // mức thấp hơn: mức cao nhất (9) chỉ có 1 TCB
    assert_eq!(dem_ready(&r.m, 0), 2);
    assert_eq!(r.m.push_migration().expect("push"), 0, "top chỉ 1 ⇒ 0");

    // (d) không work stealing: hart 1 rảnh **không** tự kéo TCB của hart 0.
    let mut r = Rig::new(2, 2);
    r.dat(0, 5, 0b11, 0);
    assert_eq!(dem_ready(&r.m, 0), 1);
    assert_eq!(r.dispatch(1), None, "hart 1 rảnh không kéo việc");
    assert_eq!(r.m.push_migration().expect("push"), 0);
    assert_eq!(r.m.migrate_count(), 0);
    assert!(r.m.ready_contains(0, r.t[0]));
}

// ---------------------------------------------------------------------------
// 6. TCB đang `Running` mức cao hơn **không** bị đẩy; chỉ TCB `Ready` cùng mức Ready cao
//    nhất mới di trú (Q2).
// ---------------------------------------------------------------------------

#[test]
fn push_khong_day_tcb_running_va_dung_muc_cao_nhat() {
    let mut r = Rig::new(2, 3);
    r.dat(0, 255, 0b11, 0); // chạy trên hart 0
    r.dat(1, 9, 0b11, 0);
    r.dat(2, 9, 0b11, 0);
    assert_eq!(r.dispatch(0), Some(r.t[0]), "TCB mức 255 chạy");
    assert_eq!(
        r.m.tcb_state(r.t[0]).expect("state"),
        TcbState::Running { hart: 0 }
    );

    // `dispatch` tự chạy một vòng push (KT-29) ⇒ đúng 1 TCB Ready cuối mức 9 di trú.
    assert_eq!(r.m.migrate_count(), 1, "đúng 1 TCB Ready");
    assert_eq!(
        r.m.current(0),
        Some(r.t[0]),
        "TCB Running không bị đẩy — vẫn current hart 0"
    );
    assert!(matches!(
        r.m.tcb_state(r.t[0]).expect("state"),
        TcbState::Running { hart: 0 }
    ));
    // TCB cuối mức 9 (t2) di trú; t1 (cùng mức) ở lại.
    assert_eq!(r.m.tcb_info(r.s[2]).expect("tcb_info").home, 1);
    assert!(r.m.ready_contains(0, r.t[1]));
    assert!(!r.m.ready_contains(0, r.t[2]));
    kiem(&r.m, "push không đụng Running");
}

// ---------------------------------------------------------------------------
// 7. Push tự động cuối bước khi `preempt_every_step` bật (DoD 3), vẫn đúng một TCB/bước.
// ---------------------------------------------------------------------------

#[test]
fn push_tu_dong_khi_preempt_moi_buoc() {
    let mut r = Rig::new(2, 3);
    for i in 0..3 {
        r.prio(i, 5);
        r.affinity(i, 0b11);
        r.ready(i);
    }
    r.m.set_preempt_every_step(true);
    // Một public op có preempt: dispatch mọi hart rồi push cuối bước.
    r.m.tcb_set_priority(r.s[0], 5, 255).expect("priority");
    kiem(&r.m, "preempt + push");
    assert_eq!(r.m.migrate_count(), 1, "đúng 1 migrate trong bước");
    // `SetPriority(t0)` gỡ t0 rồi đặt lại **cuối** hàng mức 5 ⇒ t0 là TCB cuối hàng được đẩy;
    // preempt chọn t1 (đầu hàng) chạy trên hart 0.
    assert_eq!(
        r.m.tcb_info(r.s[0]).expect("tcb_info").home,
        1,
        "TCB cuối hàng được đẩy"
    );
    assert!(matches!(
        r.m.tcb_state(r.t[1]).expect("state"),
        TcbState::Running { hart: 0 }
    ));
    assert_eq!(dem_ready(&r.m, 0), 1, "hart 0 còn current + 1 Ready");
}

// ---------------------------------------------------------------------------
// 8. `Migrate` qua hộp thư: `post_mail` + `process_mail` di trú thật; 1 hart vẫn
//    `NotSupported` (không đổi KT-22); yêu cầu sai (hart/affinity/home) bị từ chối.
// ---------------------------------------------------------------------------

#[test]
fn post_migrate_qua_hop_thu_di_tru_that() {
    let mut r = Rig::new(2, 1);
    r.dat(0, 5, 0b11, 0);
    assert_eq!(
        r.m.post_mail(
            1,
            MailboxMsg::Migrate {
                tcb: r.t[0],
                to_hart: 1
            }
        ),
        Ok(())
    );
    assert_eq!(r.m.mailbox_len(1).expect("mailbox_len"), 1);
    assert_eq!(r.m.migrate_count(), 0, "chưa xử lý hộp thư thì chưa di trú");
    assert_eq!(r.m.process_mail(1).expect("process_mail"), 1);
    assert_eq!(r.m.migrate_count(), 1);
    assert_eq!(r.m.tcb_info(r.s[0]).expect("tcb_info").home, 1);
    assert!(r.m.ready_contains(1, r.t[0]));
    assert_eq!(
        r.m.mail_log(1).expect("mail_log"),
        &[MailboxMsg::Migrate {
            tcb: r.t[0],
            to_hart: 1
        }]
    );
    kiem(&r.m, "process Migrate");

    // Yêu cầu sai: hart đích ngoài mô hình / trùng home / TCB không còn trong affinity.
    let mut r = Rig::new(2, 1);
    r.dat(0, 5, 0b01, 0);
    assert_eq!(
        r.m.post_mail(
            1,
            MailboxMsg::Migrate {
                tcb: r.t[0],
                to_hart: 2
            }
        ),
        Err(KError::InvalidArgument)
    );
    assert_eq!(
        r.m.post_mail(
            0,
            MailboxMsg::Migrate {
                tcb: r.t[0],
                to_hart: 0
            }
        ),
        Err(KError::InvalidArgument),
        "to_hart == home"
    );
    assert_eq!(
        r.m.post_mail(
            1,
            MailboxMsg::Migrate {
                tcb: r.t[0],
                to_hart: 1
            }
        ),
        Err(KError::InvalidArgument),
        "đích ngoài affinity"
    );

    // 1 hart: `Migrate` vẫn `NotSupported` (giữ nguyên ngữ nghĩa KT-22).
    let mut m1 = Machine::new();
    let tcb = m1.add_kernel_object(ObjType::Tcb);
    assert_eq!(
        m1.post_mail(0, MailboxMsg::Migrate { tcb, to_hart: 1 }),
        Err(KError::NotSupported)
    );
}

// ---------------------------------------------------------------------------
// 8b. **Chỉ một** yêu cầu `Migrate`/TCB đang chờ (reviewer-kt29b): `post_mail` lần hai cho
//     **cùng TCB** ⇒ `InvalidArgument`, **không panic** (`check_smp` bất biến (ii) xanh);
//     xử lý xong yêu cầu mới lại hợp lệ.
// ---------------------------------------------------------------------------

#[test]
fn post_hai_migrate_cung_tcb_bi_tu_choi() {
    let mut r = Rig::new(2, 1);
    r.dat(0, 5, 0b11, 0);
    let yeu_cau = MailboxMsg::Migrate {
        tcb: r.t[0],
        to_hart: 1,
    };
    r.m.post_mail(1, yeu_cau).expect("yêu cầu đầu hợp lệ");
    assert_eq!(r.m.mailbox_len(1).expect("mailbox_len"), 1);
    assert_eq!(
        r.m.post_mail(1, yeu_cau),
        Err(KError::InvalidArgument),
        "yêu cầu thứ hai cho cùng TCB phải bị từ chối"
    );
    assert_eq!(r.m.mailbox_len(1).expect("mailbox_len"), 1);
    kiem(&r.m, "post trùng Migrate"); // không panic, bất biến (ii) vẫn xanh

    // Xử lý xong ⇒ yêu cầu mới hợp lệ (TCB đã home 1 nên gửi về hart 0).
    assert_eq!(r.m.process_mail(1).expect("process_mail"), 1);
    assert_eq!(r.m.migrate_count(), 1);
    r.m.post_mail(
        0,
        MailboxMsg::Migrate {
            tcb: r.t[0],
            to_hart: 0,
        },
    )
    .expect("sau khi xử lý, yêu cầu mới hợp lệ");
    assert_eq!(r.m.process_mail(0).expect("process_mail"), 1);
    assert_eq!(r.m.tcb_info(r.s[0]).expect("info").home, 0);
    kiem(&r.m, "Migrate về hart 0");
}

// ---------------------------------------------------------------------------
// 9. Wake chéo hart: `wake(tcb home=Y)` từ hart X ⇒ `Wake` vào hộp thư Y; Y chỉ preempt nếu
//    ưu tiên **>** `published(Y).cur_prio`; hai nhánh cao hơn/không cao hơn (DoD 5).
// ---------------------------------------------------------------------------

#[test]
fn wake_cheo_hart_qua_hop_thu_va_nguong_preempt() {
    // (a) ưu tiên cao hơn cur_prio công bố ⇒ preempt ngay khi xử lý hộp thư.
    let mut r = Rig::new(2, 2);
    r.dat(0, 3, 0b11, 1); // t0 chạy trên hart 1 mức 3
    assert_eq!(r.dispatch(1), Some(r.t[0]));
    r.dat(1, 7, 0b11, 1); // t1 chờ trên hart 1 mức 7 (vào hàng **sau** khi t0 chạy)
    r.m.transition_tcb(
        r.t[1],
        TcbState::BlockedOnAddress {
            frame: 1,
            offset: 0,
        },
    )
    .expect("t1 chặn");
    kiem(&r.m, "t1 BlockedOnAddress");

    assert_eq!(r.m.current_hart(), 0);
    r.m.wake(r.t[1]).expect("wake chéo hart");
    // Wake đi qua hộp thư của hart đích, **chưa** đánh thức cho tới khi hart Y xử lý.
    assert_eq!(r.m.mailbox_len(1).expect("mailbox_len"), 1);
    assert!(matches!(
        r.m.tcb_state(r.t[1]).expect("state"),
        TcbState::BlockedOnAddress { .. }
    ));
    assert_eq!(r.m.process_mail(1).expect("process_mail"), 1);
    assert_eq!(
        r.m.current(1),
        Some(r.t[1]),
        "ưu tiên 7 > cur_prio 3 ⇒ preempt"
    );
    assert_eq!(
        r.m.tcb_state(r.t[0]).expect("state"),
        TcbState::Ready,
        "TCB cũ về Ready"
    );
    assert_eq!(
        r.m.mail_log(1).expect("mail_log"),
        &[MailboxMsg::Wake(r.t[1])]
    );

    // (b) ưu tiên **không** cao hơn cur_prio công bố ⇒ chỉ Ready, không preempt.
    let mut r = Rig::new(2, 2);
    r.dat(0, 5, 0b11, 1);
    assert_eq!(r.dispatch(1), Some(r.t[0]));
    r.dat(1, 5, 0b11, 1);
    r.m.transition_tcb(
        r.t[1],
        TcbState::BlockedOnAddress {
            frame: 1,
            offset: 0,
        },
    )
    .expect("t1 chặn");
    r.m.wake(r.t[1]).expect("wake chéo hart");
    assert_eq!(r.m.process_mail(1).expect("process_mail"), 1);
    assert_eq!(r.m.current(1), Some(r.t[0]), "cùng mức 5 ⇒ không preempt");
    assert_eq!(r.m.tcb_state(r.t[1]).expect("state"), TcbState::Ready);
    assert!(r.m.ready_contains(1, r.t[1]));

    // (c) wake trên **cùng** hart (current_hart == home) đi thẳng, không qua hộp thư.
    let mut r = Rig::new(2, 1);
    r.dat(0, 5, 0b11, 0);
    r.m.transition_tcb(
        r.t[0],
        TcbState::BlockedOnAddress {
            frame: 2,
            offset: 8,
        },
    )
    .expect("chặn");
    r.m.wake(r.t[0]).expect("wake cùng hart");
    assert_eq!(r.m.mailbox_len(0).expect("mailbox_len"), 0);
    assert_eq!(r.m.tcb_state(r.t[0]).expect("state"), TcbState::Ready);
}

// ---------------------------------------------------------------------------
// 10. Fuzz SMP: 1.000 seed × 64 op × n ∈ {1,2,4,8}; cùng seed ⇒ **cùng trace**; bất biến
//     `check_sched`/`check_harts`/`check_ipc` sau **mỗi** bước; 0 vi phạm (DoD 6/7). Dưới
//     Miri giảm còn 3 seed × 8 op (`cfg(miri)`), stable vẫn 1.000 × 64.
// ---------------------------------------------------------------------------

fn so_seed() -> u64 {
    if cfg!(miri) {
        3
    } else {
        1_000
    }
}

fn so_op() -> usize {
    if cfg!(miri) {
        8
    } else {
        64
    }
}

/// Một lượt chạy tất định: trả **trace** (op + ảnh chụp trạng thái sau op). Cùng seed cùng
/// hart ⇒ trace phải byte-for-byte giống nhau.
fn mot_luot(seed: u64, harts: u16) -> Vec<String> {
    let mut r = Rig::new(harts, 4);
    for i in 0..4 {
        r.prio(i, 3 + i as u8);
        r.affinity(i, (1u64 << harts) - 1);
        r.ready(i);
    }
    let mut rng = Lcg::new(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ harts as u64);
    let mut trace: Vec<String> = Vec::new();
    for step in 0..so_op() {
        let op = rng.below(10);
        let mut mo_ta = format!("step {step} op {op}");
        match op {
            0 => {
                let i = rng.below(4) as usize;
                let p = rng.below(16) as u8;
                match r.m.tcb_set_priority(r.s[i], p, 255) {
                    Ok(()) => mo_ta.push_str(&format!(" prio {i}={p}")),
                    Err(e) => mo_ta.push_str(&format!(" prio {i} err {e:?}")),
                }
            }
            1 => {
                let i = rng.below(4) as usize;
                let home = r.m.tcb_info(r.s[i]).expect("info").home;
                let them = rng.below(1u64 << harts) & !(1u64 << home);
                let mask = (1u64 << home) | them;
                match r.m.tcb_set_affinity(r.s[i], mask) {
                    Ok(()) => mo_ta.push_str(&format!(" aff {i}={mask:#x}")),
                    Err(e) => mo_ta.push_str(&format!(" aff {i} err {e:?}")),
                }
            }
            2 => {
                let i = rng.below(4) as usize;
                let h = rng.below(harts as u64) as u16;
                let info = r.m.tcb_info(r.s[i]).expect("info");
                if info.affinity & (1u64 << h) == 0 {
                    mo_ta.push_str(" home ngoài affinity (bỏ)");
                } else {
                    match r.m.tcb_set_home(r.s[i], h) {
                        Ok(()) => mo_ta.push_str(&format!(" home {i}={h}")),
                        Err(e) => mo_ta.push_str(&format!(" home {i} err {e:?}")),
                    }
                }
            }
            3 => {
                let h = rng.below(harts as u64) as u16;
                let got = r.m.dispatch(h).expect("dispatch");
                mo_ta.push_str(&format!(" dispatch {h} -> {got:?}"));
            }
            4 => {
                let h = rng.below(harts as u64) as u16;
                if r.m.current(h).is_some() {
                    let got =
                        r.m.block_current(
                            h,
                            TcbState::Sleeping {
                                reason: SleepReason::Timeout,
                            },
                        )
                        .expect("block_current");
                    mo_ta.push_str(&format!(" block {got}"));
                }
            }
            5 => {
                let i = rng.below(4) as usize;
                match r.m.wake(r.t[i]) {
                    Ok(()) => mo_ta.push_str(&format!(" wake {i}")),
                    Err(e) => mo_ta.push_str(&format!(" wake {i} err {e:?}")),
                }
            }
            6 => {
                let h = rng.below(harts as u64) as u16;
                match r.m.process_mail(h) {
                    Ok(n) => mo_ta.push_str(&format!(" mail {h} {n}")),
                    Err(e) => mo_ta.push_str(&format!(" mail {h} err {e:?}")),
                }
            }
            7 => {
                let n = r.m.push_migration().expect("push_migration");
                mo_ta.push_str(&format!(" push {n}"));
            }
            8 => {
                let i = rng.below(4) as usize;
                let st = r.m.tcb_state(r.t[i]).expect("state");
                if matches!(st, TcbState::Inactive) {
                    r.m.transition_tcb(r.t[i], TcbState::Ready)
                        .expect("Inactive→Ready");
                    mo_ta.push_str(&format!(" ready {i}"));
                } else if !matches!(st, TcbState::Ready | TcbState::Running { .. }) {
                    let _ = r.m.wake(r.t[i]);
                    mo_ta.push_str(&format!(" ready(wake) {i}"));
                }
            }
            _ => {
                let i = rng.below(4) as usize;
                match r.m.tcb_state(r.t[i]).expect("state") {
                    TcbState::Ready => {
                        r.m.transition_tcb(
                            r.t[i],
                            TcbState::BlockedOnAddress {
                                frame: 1 + i as ObjId,
                                offset: 0,
                            },
                        )
                        .expect("Ready→BlockedOnAddress");
                        mo_ta.push_str(&format!(" addr {i}"));
                    }
                    TcbState::Inactive => {
                        r.m.transition_tcb(r.t[i], TcbState::Ready)
                            .expect("Inactive→Ready");
                        mo_ta.push_str(&format!(" ready {i}"));
                    }
                    _ => mo_ta.push_str(" addr (bỏ)"),
                }
            }
        }
        kiem(&r.m, &mo_ta);
        // Ảnh chụp gọn: current/published/hàng/migrate của mọi hart — đổi một bit là trace khác.
        let mut anh = String::new();
        for h in 0..harts {
            let p = r.m.published(h).expect("published");
            anh.push_str(&format!(
                "h{h}[cur={:?},idle={},prio={},len={}]",
                r.m.current(h),
                p.idle,
                p.cur_prio,
                r.m.ready_len(h)
            ));
        }
        mo_ta.push_str(&format!(" | {anh} mig={}", r.m.migrate_count()));
        trace.push(mo_ta);
    }
    trace
}

#[test]
fn fuzz_smp_tat_dinh_bat_bien() {
    let seeds = so_seed();
    for harts in [1u16, 2, 4, 8] {
        for seed in 0..seeds {
            let a = mot_luot(seed, harts);
            let b = mot_luot(seed, harts);
            assert_eq!(a, b, "smp {harts} seed {seed}: cùng seed phải cùng trace");
            assert_eq!(a.len(), so_op(), "đúng số op");
        }
        println!(
            "# KT-29 fuzz OK: smp {harts}, {seeds} seed × {} op",
            so_op()
        );
    }
}

// ---------------------------------------------------------------------------
// 11. `check_smp`/`check_harts` chạy được ở mọi cấu hình hart và `publish` không cho hart
//     "nói dối" trạng thái thật (bất biến (iv) của DoD 6) — checker không phải hàm rỗng.
// ---------------------------------------------------------------------------

#[test]
fn check_harts_chay_moi_cau_hinh_va_publish_trung_thuc() {
    for n in [1u16, 2, 4, 8] {
        let m = Machine::with_harts(n);
        assert!(m.check_harts().is_ok(), "mô hình rỗng {n} hart phải sạch");
        assert!(m.check_smp().is_ok());
    }
    let mut r = Rig::new(2, 1);
    r.dat(0, 5, 0b11, 0);
    assert!(r.m.check_harts().is_ok());
    // Hart 0 đang rảnh: công bố "bận" là sai trạng thái thật ⇒ bị từ chối (không cho nói dối).
    assert_eq!(
        r.m.publish(0, false, 5),
        Err(KError::IllegalOperation),
        "published phải là ảnh trung thực của scheduler"
    );
    r.m.publish(0, true, 0).expect("công bố đúng");
    assert!(r.m.check_harts().is_ok());
    // Bật hart 0 chạy ⇒ phải công bố bận/mức thật.
    assert_eq!(r.dispatch(0), Some(r.t[0]));
    assert_eq!(r.m.publish(0, true, 0), Err(KError::IllegalOperation));
    r.m.publish(0, false, 5).expect("công bố đúng sau dispatch");
    assert!(r.m.check_harts().is_ok());
}
