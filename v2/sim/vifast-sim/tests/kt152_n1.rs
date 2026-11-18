// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-152 (N1)** — kịch bản sim **"đơn điệu sau migrate"** + **conformance chéo hart**.
//!
//! Nguồn: `docs/v1/KNOWN-ISSUES.md` dòng 1 (**T33a**, `[V1:ĐO]`) — v1 treo x86_64 ~0,3–0,5 %
//! lần boot sau `shell>`, không in `PREEMPT OK`; gốc đã lộ một nửa: worker C thấy
//! `TIME MONOTONIC BAD` → `panic!("GetTime went backwards on hart 1")` → root chờ mãi message
//! `WORKER_LABEL_C`. Quyết định: **D2-132** (Agent 1 quyết thay Agent 0).
//!
//! v2.0 **chỉ riscv64** nên tiêu chí gốc "600 boot x86_64" **không áp dụng được**; tiêu chí N1
//! của v2 là ba vế:
//!
//! - **(a)** kịch bản sim "đơn điệu sau migrate": migrate TCB giữa **hai** hart, đọc đồng hồ
//!   monotonic trước/sau, đòi **không giảm** — [`don_dieu_sau_migrate_hai_hart`] và
//!   [`reset_hart_muon_khong_dua_dong_ho_ve_0`] (vế `reset()` của v1);
//! - **(b)** conformance **chéo hart**: mọi hart đọc **cùng một** thứ tự monotonic —
//!   [`conformance_cheo_hart_mot_thu_tu`];
//! - **(c)** **đột biến** làm (a) hoặc (b) **đỏ** — log đỏ nguyên văn ở
//!   `v2/agent/JOURNAL/kt152-n1.md` §4 (đột biến `read` trả bộ đếm **theo hart** thay vì đồng
//!   hồ hệ thống, và đột biến `reset_hart` đưa đồng hồ hệ thống về 0).
//!
//! **Model-only `[TK]`, không phải spec/ABI.** Đồng hồ wall ở [`vifast_sim::clock`] **tách**
//! khỏi đồng hồ ngân sách MCS `Machine::virtual_now()` (KT-20): cái sau bị `tick` đẩy theo TCB
//! đang chạy, cái trước phải đơn điệu qua migrate **và** qua `reset()` của một hart muộn.
//!
//! Bất biến kiểm **sau mỗi** bước: [`kiem`] (scheduler + SMP + IPC + mailbox + đồng hồ).

use vifast_sim::kobj::{KError, Machine, ObjId, ObjType, SlotRef, TcbState};
use vifast_sim::mailbox::MailboxMsg;

fn slot(cnode: ObjId, s: usize) -> SlotRef {
    SlotRef::new(cnode, s)
}

/// Bất biến **sau mỗi** public op: scheduler + SMP + IPC + mailbox + đồng hồ wall.
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
    if let Err(e) = m.check_smp() {
        panic!("check_smp sau {buoc}: {e}");
    }
    if let Err(e) = m.check_clock() {
        panic!("check_clock sau {buoc}: {e}");
    }
}

// ---------------------------------------------------------------------------
// Rig: CNode 16 slot; 1 Untyped, 1 SchedContext, 1 SchedControl; TCB ở slot 8..;
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

    /// TCB i: priority, affinity, home, rồi `Inactive → Ready`.
    fn dat(&mut self, i: usize, p: u8, mask: u64, h: u16) {
        self.m.tcb_set_priority(self.s[i], p, 255).expect("prio");
        self.m.tcb_set_affinity(self.s[i], mask).expect("affinity");
        self.m.tcb_set_home(self.s[i], h).expect("home");
        self.m
            .transition_tcb(self.t[i], TcbState::Ready)
            .expect("Inactive→Ready");
        kiem(&self.m, "dat TCB");
    }

    fn dispatch(&mut self, h: u16) -> Option<ObjId> {
        let got = self.m.dispatch(h).expect("dispatch");
        kiem(&self.m, "dispatch");
        got
    }

    /// Đọc đồng hồ wall "từ" hart `h` + kiểm bất biến.
    fn doc(&mut self, h: u16) -> u64 {
        let v = self.m.clock_read(h).expect("clock_read");
        kiem(&self.m, "clock_read");
        v
    }

    /// Nâng **nguồn** thời gian của hart `h` (mô hình ngắt timer của hart đó).
    fn tick_hart(&mut self, h: u16, n: u64) {
        self.m.clock_tick_on(h, n).expect("clock_tick_on");
        kiem(&self.m, "clock_tick_on");
    }
}

// ---------------------------------------------------------------------------
// (a) Đơn điệu **sau migrate**: TCB chạy trên hart 0 → bị preempt → push migration sang hart 1
//     → chạy tiếp trên hart 1. Đồng hồ đọc trước và sau **không được giảm**, dù nguồn thời gian
//     của hart 1 đang **thấp hơn** hart 0 (đúng tình huống v1: hart phụ có bộ đếm riêng thấp).
// ---------------------------------------------------------------------------

#[test]
fn don_dieu_sau_migrate_hai_hart() {
    let mut r = Rig::new(2, 1);
    r.dat(0, 5, 0b11, 0);
    assert_eq!(r.dispatch(0), Some(r.t[0]), "t0 chạy trên hart 0");
    assert_eq!(r.m.current(0), Some(r.t[0]));

    // Hart 0 chạy được 5 tick ⇒ nguồn hart 0 = 5, đồng hồ hệ thống = 5.
    r.tick_hart(0, 5);
    assert_eq!(r.m.clock_per_hart(0), Some(5));
    assert_eq!(r.m.clock_system(), 5);
    let truoc = r.doc(0);
    assert_eq!(truoc, 5, "đọc trước migrate");

    // Preempt rồi migrate sang hart 1 qua **hộp thư** (`MailboxMsg::Migrate` — đúng đường
    // migrate của mô hình; `push_migration` là **chính sách** push chỉ chạy khi nguồn có > 1 TCB
    // cùng mức cao nhất, nên không dùng cho ca 1 TCB). `transition_tcb` chỉ đồng bộ scheduler;
    // `published` được làm tươi ở public op kế tiếp (`send_mail`), nên `kiem` đặt **sau** nó.
    r.m.transition_tcb(r.t[0], TcbState::Ready)
        .expect("Running→Ready (preempt)");
    r.m.send_mail(
        1,
        MailboxMsg::Migrate {
            tcb: r.t[0],
            to_hart: 1,
        },
    )
    .expect("Migrate sang hart 1");
    assert_eq!(r.m.migrate_count(), 1);
    assert_eq!(r.m.tcb_info(r.s[0]).expect("tcb_info").home, 1);
    assert!(r.m.ready_contains(1, r.t[0]), "t0 phải ở hàng hart 1");
    kiem(&r.m, "push_migration");

    // Hart 1 mới chạy 1 tick ⇒ **nguồn** hart 1 = 1 < 5. Nếu đọc bộ đếm theo hart thì đồng hồ
    // **lùi** (đúng lỗi v1 T33a); đọc đồng hồ hệ thống thì phải **không** lùi.
    r.tick_hart(1, 1);
    assert_eq!(
        r.m.clock_per_hart(1),
        Some(1),
        "nguồn hart 1 thấp hơn hart 0"
    );
    let sau = r.doc(1);
    assert!(
        sau >= truoc,
        "đồng hồ wall giảm sau migrate: trước={truoc} sau={sau} (hart 1 nguồn=1)"
    );
    assert_eq!(
        sau, 5,
        "đồng hồ hệ thống giữ max-update, không theo hart đích"
    );

    // Chạy tiếp trên hart 1: nguồn hart 1 vượt hart 0 ⇒ đồng hồ hệ thống **tăng** theo.
    assert_eq!(r.dispatch(1), Some(r.t[0]), "t0 chạy tiếp trên hart 1");
    r.tick_hart(1, 10);
    assert_eq!(r.m.clock_per_hart(1), Some(11));
    let sau2 = r.doc(1);
    assert_eq!(sau2, 11, "hart 1 vượt ⇒ max-update nâng đồng hồ hệ thống");
    assert!(sau2 >= sau);

    // Migrate **ngược** về hart 0 (nguồn hart 0 = 5 < 11) — vẫn không được lùi.
    r.m.transition_tcb(r.t[0], TcbState::Ready)
        .expect("Running→Ready lần hai");
    r.m.send_mail(
        0,
        MailboxMsg::Migrate {
            tcb: r.t[0],
            to_hart: 0,
        },
    )
    .expect("Migrate về hart 0");
    assert_eq!(r.m.migrate_count(), 2);
    assert_eq!(r.m.tcb_info(r.s[0]).expect("tcb_info").home, 0);
    let sau3 = r.doc(0);
    assert!(
        sau3 >= sau2,
        "đồng hồ wall giảm khi migrate ngược: trước={sau2} sau={sau3}"
    );

    assert_eq!(r.m.clock_backwards(), 0, "không lần đọc nào lùi");
    assert_eq!(r.m.clock_reads(), 4, "đã đọc thật 4 lần");
    kiem(&r.m, "cuối (a)");
}

// ---------------------------------------------------------------------------
// (a') Vế `reset()` của v1: `Kernel::reset()` (gọi bởi `init_kernel()`) từng đưa `system_ticks`
//      về 0 khi một hart **muộn** khởi tạo lại ⇒ task đang chạy thấy `GetTime` nhảy lùi. Ở đây
//      `hart_reinit(1)` mô hình đúng việc đó và **không** được đụng đồng hồ hệ thống.
// ---------------------------------------------------------------------------

#[test]
fn reset_hart_muon_khong_dua_dong_ho_ve_0() {
    let mut r = Rig::new(2, 1);
    r.dat(0, 5, 0b11, 0);
    assert_eq!(r.dispatch(0), Some(r.t[0]));
    r.tick_hart(0, 7);
    let truoc = r.doc(0);
    assert_eq!(truoc, 7);

    // Hart 1 **rảnh và rỗng** ⇒ `init_kernel()` muộn trên hart 1 hợp lệ.
    assert!(r.m.sched_idle(1));
    r.m.hart_reinit(1).expect("hart_reinit(1)");
    kiem(&r.m, "hart_reinit");

    assert_eq!(
        r.m.clock_per_hart(1),
        Some(0),
        "nguồn hart 1 về 0 (như reset)"
    );
    assert_eq!(
        r.m.clock_system(),
        7,
        "reset hart muộn KHÔNG được đưa đồng hồ hệ thống về 0 (bản vá r323 của v1)"
    );
    let sau = r.doc(0);
    assert!(
        sau >= truoc,
        "đồng hồ wall lùi sau reset hart muộn: trước={truoc} sau={sau}"
    );
    // Hart 1 đọc **cùng** giá trị — một nguồn hệ thống cho mọi hart.
    assert_eq!(r.doc(1), sau);
    assert_eq!(r.m.clock_backwards(), 0);
    kiem(&r.m, "cuối (a')");
}

// ---------------------------------------------------------------------------
// (b) Conformance **chéo hart**: mọi hart đọc **cùng một** thứ tự monotonic. Xen kẽ đọc từ hai
//     hart, giữa các lần đọc có tick của hart này hoặc hart kia; dãy đọc (bất kể hart) phải
//     **không giảm**, và tại **cùng một điểm** của kịch bản hai hart phải đọc **cùng** giá trị.
// ---------------------------------------------------------------------------

#[test]
fn conformance_cheo_hart_mot_thu_tu() {
    let mut r = Rig::new(2, 2);
    r.dat(0, 5, 0b11, 0);
    r.dat(1, 5, 0b11, 1);
    assert_eq!(r.dispatch(0), Some(r.t[0]));
    assert_eq!(r.dispatch(1), Some(r.t[1]));

    let mut truoc = 0u64;
    let mut lan = 0u32;
    // 12 lần đọc xen kẽ: hart 0, hart 1, hart 0, …; nguồn của hart đọc **không** quyết định
    // giá trị (đó là nội dung của conformance chéo hart).
    for buoc in 0..12u32 {
        let h = (buoc % 2) as u16;
        // Nửa số bước có tick của hart **kia** để hai nguồn lệch nhau thật.
        if buoc % 3 == 0 {
            r.tick_hart(1 - h, 1 + u64::from(buoc % 4));
        }
        let v = r.doc(h);
        assert!(
            v >= truoc,
            "dãy đọc chéo hart giảm ở bước {buoc} (hart {h}): trước={truoc} sau={v}"
        );
        // Tại **cùng** điểm, hart kia đọc **cùng** giá trị.
        let w = r.doc(1 - h);
        assert_eq!(w, v, "hai hart đọc khác nhau tại bước {buoc}");
        assert!(w >= v);
        truoc = w;
        lan += 2;
    }
    assert_eq!(r.m.clock_reads(), u64::from(lan), "đã đọc thật {lan} lần");
    assert_eq!(r.m.clock_backwards(), 0, "không lần đọc nào lùi");
    assert!(
        r.m.clock_system() > 0,
        "đồng hồ đã chạy thật, không phải hằng 0"
    );
    kiem(&r.m, "cuối (b)");
}

// ---------------------------------------------------------------------------
// Ca âm của chính API (D2-108(B): bộ đếm/luật phải có ca âm, không chỉ "luôn 0"):
// hart ngoài dải bị **từ chối** (không panic), và `hart_reinit` trên hart **bận** bị từ chối
// thay vì phá bất biến scheduler.
// ---------------------------------------------------------------------------

#[test]
fn ca_am_api_dong_ho() {
    let mut r = Rig::new(2, 1);
    assert_eq!(r.m.clock_read(2), Err(KError::InvalidArgument));
    assert_eq!(r.m.clock_tick_on(2, 1), Err(KError::InvalidArgument));
    assert_eq!(r.m.hart_reinit(2), Err(KError::InvalidArgument));
    assert_eq!(r.m.clock_per_hart(2), None);
    assert_eq!(r.m.clock_reads(), 0, "lần đọc bị từ chối không được tính");

    // Hart 0 **bận** ⇒ `hart_reinit(0)` bị từ chối, không phá bất biến.
    r.dat(0, 5, 0b11, 0);
    assert_eq!(r.dispatch(0), Some(r.t[0]));
    assert_eq!(r.m.hart_reinit(0), Err(KError::IllegalOperation));
    kiem(&r.m, "ca âm hart_reinit bận");

    // Hart 1 rảnh nhưng **có message chờ** ⇒ cũng bị từ chối (không nuốt message).
    r.m.post_mail(1, MailboxMsg::Wake(r.t[0]))
        .expect("post_mail Wake");
    assert_eq!(r.m.hart_reinit(1), Err(KError::IllegalOperation));
    assert_eq!(r.m.mailbox_len(1).expect("mailbox_len"), 1);
    kiem(&r.m, "ca âm hart_reinit có message");
}
