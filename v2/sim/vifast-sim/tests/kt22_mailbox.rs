// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-22 — **hộp thư FIFO cố định** (mailbox) cho endpoint/TCB: sức chứa cố định, đầy ⇒
//! **backpressure** (`Busy`, trả lại message — không panic, không ghi đè, không mất), thứ tự
//! FIFO tất định, nhiều endpoint độc lập; **không** atomic/thread/loom (sim 1 luồng, D2-49(f)).
//!
//! **Model-only `[TK]`, không phải spec/ABI.** Ngữ nghĩa: thiết kế §4.16/§4.17
//! (`docs/v2/ViFastOs-v2-thiet-ke.md:403-428`) — "MPSC kích thước cố định … đầy: người gửi xử lý
//! hộp thư của mình rồi thử lại"; K1B-PLAN §5 KT-22 DoD 1/2/6/7/8; **D2-49(a)** (1 hart, push
//! migration hoãn K1c) + **D2-49(f)** (chỉ hộp thư mô hình tất định; `vifast-sync`/lock-free/
//! loom/stress để K1c/SMP).
//!
//! Quy ước mô hình (ghi rõ để review):
//!
//! 1. **Sức chứa cố định 8 message `[TK]`** ([`MAILBOX_CAP`]) cho **cả** hàng gửi của Endpoint
//!    và hộp thư theo hart (`MailboxMsg`). Message **không** bị ghi đè: `try_push` đầy ⇒ trả
//!    **lại** message; tầng `Machine` quy về `KError::Busy` (backpressure).
//! 2. **Endpoint**: `Call` khi hàng gửi đầy ⇒ `Busy` **trước các thay đổi trạng thái của đường
//!    IPC** (không ghi `last_mr`, không `BlockedOnSend`; TCB giữ `Ready`) — người gọi xử lý rồi
//!    **thử lại**. Lưu ý: `ensure_started` (kích hoạt ngầm `Inactive → Ready`, **pre-existing**
//!    từ KT-18) đã chạy trước như mọi syscall ⇒ `Busy` không hoàn nguyên bước kích hoạt đó; test
//!    dùng TCB đã `Ready` nên không phụ thuộc chi tiết này. `Exit` có `fault_ep` khi hàng đầy
//!    cũng `Busy` và **không** thay đổi gì (không `exit_code`, không đổi trạng thái, không trả
//!    donation) — tránh "mất fault" một nửa. Hàng đầy **không** panic.
//! 3. **Hộp thư theo hart** (§4.17): message tối thiểu `Wake`, `Reschedule`, `Migrate`, `Stop`.
//!    K1b **1 hart** (D2-49(a)) ⇒ `Migrate` bị từ chối **ngay** bằng `NotSupported`, **không**
//!    im lặng bỏ; cài `Migrate`/push migration thật thuộc K1c. `post_mail` là **cạnh gửi** (kiểm
//!    sức chứa/backpressure); `process_mail` là **cuối bước** (drain FIFO, áp dụng đúng một lần);
//!    `send_mail` = gửi + xử lý trong **một** bước ⇒ không có trạng thái "đã pop chưa áp dụng".
//! 4. **Bất biến** (kiểm sau **mỗi** op qua `check_sched` + `check_mailboxes`): mỗi mailbox
//!    `pushed == popped + removed + len`, `len ≤ cap`; mọi TCB đang nằm trong hàng gửi của
//!    endpoint ở đúng `BlockedOnSend{ep, call/fault}`; hàng gửi khác rỗng ⇒ không có `recv`
//!    waiter. Cùng seed ⇒ cùng trace.
//!
//! **Follow-up (ghi ở đây để không quên)**: (a) **K1c** — `check_mailboxes` đang mã hoá
//! `call == fault.is_none()` vì K1b chỉ có `Call` và fault `Exit`; thêm `Send` thường
//! (`call: false, fault: None`) phải nới bất biến; (b) **KT-23** — ca `Exit` `Busy` nên snapshot
//! thêm donation/`SchedContext` + `Reply.caller` (KT-22 chỉ kiểm `exit_code`/state/`len`); (c) ack
//! thế hệ + MPSC lock-free thật + `loom`/stress để K1c/SMP (D2-49(f)).
//!
//! Test đỏ trước (D2-05):
//! - [`JOURNAL/kt22-red-compile.log`](../../agent/JOURNAL/kt22-red-compile.log) — tệp này
//!   **không biên dịch** trước KT-22: module `vifast_sim::mailbox` và các model API
//!   (`post_mail`, `process_mail`, `send_mail`, `mail_len`, `mail_log`, `hart_stopped`,
//!   `endpoint_info`, `endpoint_send_items`) chưa có.
//! - [`JOURNAL/kt22-red-unbounded.log`](../../agent/JOURNAL/kt22-red-unbounded.log) — probe
//!   dùng API **cũ** (`call` × 9 + `endpoint_send_len`) chứng minh hàng gửi `VecDeque` **không
//!   có** sức chứa: 9 message vẫn `Ok`, `len == 9 > 8` — tức **không** có backpressure.

use vifast_sim::kobj::{
    EndpointInfo, FaultKind, KError, Machine, ObjId, ObjType, SlotRef, SuspendMode, TcbInfo,
    TcbState,
};
use vifast_sim::mailbox::{Mailbox, MailboxMsg, MAILBOX_CAP};
use vifast_sim::Lcg;

fn slot(cnode: ObjId, s: usize) -> SlotRef {
    SlotRef::new(cnode, s)
}

/// Bất biến scheduler + mailbox sau **mỗi** thao tác công khai (`check_sched` cũng gọi
/// `check_mailboxes`; vi phạm là panic ở build debug). `eps` = endpoint của rig (test không
/// quét được mọi `ObjId` vì các object khác xen kẽ).
fn kiem_ep(m: &Machine, eps: &[ObjId], buoc: &str) {
    if let Err(e) = m.check_sched() {
        panic!("check_sched sau {buoc}: {e}");
    }
    for &ep in eps {
        let info = m.endpoint_info(ep).expect("endpoint_info");
        assert_eq!(info.capacity, MAILBOX_CAP, "sức chứa Endpoint phải cố định");
        assert!(
            info.len <= info.capacity,
            "Endpoint {ep}: len {} > cap {}",
            info.len,
            info.capacity
        );
        assert_eq!(
            info.pushed,
            info.popped + info.removed + info.len as u64,
            "Endpoint {ep}: message bị mất/thêm (pushed={} popped={} removed={} len={})",
            info.pushed,
            info.popped,
            info.removed,
            info.len
        );
        if info.len > 0 {
            assert_eq!(
                info.recv_waiter, None,
                "Endpoint {ep}: có message trong hàng mà vẫn có Recv waiter"
            );
        }
        for (sender, fault) in m.endpoint_send_items(ep).expect("hàng gửi") {
            match m.tcb_state(sender).expect("sender còn sống") {
                TcbState::BlockedOnSend {
                    ep: hang_ep,
                    call,
                    fault: hang_fault,
                } => {
                    assert_eq!(hang_ep, ep, "sender {sender} xếp hàng ở endpoint khác");
                    assert_eq!(call, fault.is_none(), "cờ call phải khớp loại message");
                    assert_eq!(hang_fault, fault, "fault của message phải khớp trạng thái");
                }
                khac => panic!("sender {sender} trong hàng gửi nhưng ở {khac:?}"),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Rig: CNode 128 slot; 2 Endpoint; 1 Reply; n TCB; n SchedContext (mặc định test-only —
// D2-50: TCB không SC không chạy, nên rig gắn SC cho TCB cần `dispatch`); 1 SchedControl.
// TCB cuối = server; các TCB còn lại = client/người gửi.
// ---------------------------------------------------------------------------

const S_EP: [usize; 2] = [0, 1];
const S_REP: usize = 2;
const S_TCB: usize = 10;
const S_SC: usize = 64;
const S_CTL: usize = 120;

struct Rig {
    m: Machine,
    ep: [ObjId; 2],
    ep_s: [SlotRef; 2],
    rep: ObjId,
    rep_s: SlotRef,
    t: Vec<ObjId>,
    ts: Vec<SlotRef>,
}

impl Rig {
    fn new(n_tcb: usize) -> Self {
        assert!(n_tcb >= 2 && S_TCB + n_tcb <= S_SC, "rig cần chỗ cho n TCB");
        let mut m = Machine::new();
        let c0 = m.add_cnode(128);
        let ctl = m.add_kernel_object(ObjType::SchedControl);
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
        let ctl_cap = m.root_cap(ctl);
        m.put_cap(c0, S_CTL, ctl_cap).expect("cap SchedControl");
        let ctl_s = slot(c0, S_CTL);

        let mut t = Vec::with_capacity(n_tcb);
        let mut ts = Vec::with_capacity(n_tcb);
        for i in 0..n_tcb {
            let obj = m.add_kernel_object(ObjType::Tcb);
            let cap = m.root_cap(obj);
            m.put_cap(c0, S_TCB + i, cap).expect("cap TCB");
            t.push(obj);
            ts.push(slot(c0, S_TCB + i));

            let sc = m.add_kernel_object(ObjType::SchedContext);
            let sc_cap = m.root_cap(sc);
            m.put_cap(c0, S_SC + i, sc_cap).expect("cap SchedContext");
            let sc_slot = slot(c0, S_SC + i);
            m.sched_control_configure(ctl_s, sc_slot, 4, 8)
                .expect("SchedControl.Configure");
            m.set_sched_params(ts[i], Some(sc_slot))
                .expect("set_sched_params (test-only)");
            // Cùng mức ưu tiên để `dispatch`/`pick` tất định FIFO theo thứ tự vào hàng.
            m.tcb_set_priority(ts[i], 5 + (i as u8 % 3), 255)
                .expect("Tcb.SetPriority");
        }
        let mut r = Rig {
            m,
            ep,
            ep_s,
            rep,
            rep_s,
            t,
            ts,
        };
        // Mọi TCB vào `Ready` qua **một cửa** `transition_tcb` (KT-18) — `Inactive → Ready`.
        for i in 0..r.t.len() {
            r.ready(i);
        }
        r
    }

    fn kiem(&self, buoc: &str) {
        kiem_ep(&self.m, &self.ep, buoc);
    }

    fn call(&mut self, i: usize, epi: usize, mr: u64) -> Result<(), KError> {
        self.m.call(self.t[i], self.ep_s[epi], mr)
    }

    fn recv(&mut self, i: usize, epi: usize, reply: bool) -> Result<(), KError> {
        let rep = if reply { Some(self.rep_s) } else { None };
        self.m.recv(self.t[i], self.ep_s[epi], rep)
    }

    fn reply(&mut self, i: usize) -> Result<(), KError> {
        self.m.reply(self.t[i], self.rep_s)
    }

    fn ready(&mut self, i: usize) {
        if self.m.tcb_state(self.t[i]).expect("tcb_state") == TcbState::Inactive {
            self.m
                .transition_tcb(self.t[i], TcbState::Ready)
                .expect("Inactive→Ready");
            self.kiem("transition Ready");
        }
    }

    fn state(&self, i: usize) -> TcbState {
        self.m.tcb_state(self.t[i]).expect("tcb_state")
    }

    fn info(&self, i: usize) -> TcbInfo {
        self.m.tcb_info(self.ts[i]).expect("tcb_info")
    }

    fn ep_info(&self, epi: usize) -> EndpointInfo {
        self.m.endpoint_info(self.ep[epi]).expect("endpoint_info")
    }

    /// `send_mail` = gửi + xử lý trong **một** bước (DoD 6: không lộ trạng thái trung gian).
    fn send_mail(&mut self, msg: MailboxMsg) -> Result<(), KError> {
        self.m.send_mail(0, msg)
    }
}

// ---------------------------------------------------------------------------
// 1. Kiểu `Mailbox` cố định: FIFO, đầy ⇒ trả lại message (không ghi đè/không mất), pop giải
//    phóng chỗ (DoD 1).
// ---------------------------------------------------------------------------

#[test]
fn mailbox_fifo_co_dinh_day_tra_lai_message() {
    assert_eq!(
        MAILBOX_CAP, 8,
        "sức chứa mailbox [TK] = 8 (K1B-PLAN §5 KT-22 DoD 1)"
    );
    let mut mb: Mailbox<u32> = Mailbox::new();
    assert_eq!(mb.capacity(), MAILBOX_CAP);
    assert!(mb.is_empty() && !mb.is_full());
    for i in 0..MAILBOX_CAP as u32 {
        assert_eq!(mb.try_push(i), Ok(()), "còn chỗ ⇒ đẩy được");
        assert_eq!(mb.len(), i as usize + 1);
    }
    assert!(mb.is_full());
    // Đầy ⇒ **trả lại nguyên message** (backpressure), không panic/không ghi đè.
    assert_eq!(mb.try_push(99), Err(99));
    assert_eq!(mb.len(), MAILBOX_CAP, "push khi đầy không đổi số message");
    assert_eq!(mb.front(), Some(&0), "message đầu vẫn là message cũ nhất");
    // Pop **FIFO** giải phóng đúng một chỗ.
    for i in 0..MAILBOX_CAP as u32 {
        assert_eq!(mb.pop_front(), Some(i), "FIFO: {i} phải ra trước");
        assert_eq!(mb.len(), MAILBOX_CAP - i as usize - 1);
    }
    assert!(mb.is_empty());
    assert_eq!(mb.pop_front(), None);
    // Pop xong thì đẩy lại được đúng 1 message.
    assert_eq!(mb.try_push(7), Ok(()));
    assert_eq!(mb.pop_front(), Some(7));
    // 8 push + 1 push sau khi pop = 9; lần push bị từ chối **không** tính (không vào hàng).
    assert_eq!(mb.pushed(), (MAILBOX_CAP + 1) as u64);
    assert_eq!(mb.popped(), (MAILBOX_CAP + 1) as u64);
    assert_eq!(mb.removed(), 0);
    assert!(mb.invariant(), "pushed == popped + removed + len");
}

// ---------------------------------------------------------------------------
// 2. Backpressure 1.000 lần đẩy khi đầy: không mất/thêm message, nội dung không đổi (DoD 2).
// ---------------------------------------------------------------------------

#[test]
fn mailbox_backpressure_1000_lan_khong_mat_message() {
    let mut mb: Mailbox<u64> = Mailbox::new();
    for i in 0..MAILBOX_CAP as u64 {
        mb.try_push(100 + i).expect("còn chỗ");
    }
    let truoc: Vec<u64> = mb.iter().copied().collect();
    for k in 0..1_000u64 {
        let msg = 1_000_000 + k;
        assert_eq!(
            mb.try_push(msg),
            Err(msg),
            "lần {k}: đầy ⇒ trả lại đúng message, không panic"
        );
        assert_eq!(mb.len(), MAILBOX_CAP, "lần {k}: sức chứa không đổi");
    }
    assert_eq!(mb.iter().copied().collect::<Vec<_>>(), truoc);
    assert_eq!(
        mb.pushed(),
        MAILBOX_CAP as u64,
        "push bị từ chối không tính"
    );
    assert_eq!(mb.popped(), 0);
    assert_eq!(mb.removed(), 0);
    assert!(mb.invariant());
    // Pop 1 ⇒ đẩy 1 được; lặp 1.000 lần để chứng minh chỗ được giải phóng đúng, không rò.
    for k in 0..1_000u64 {
        assert!(mb.pop_front().is_some());
        assert_eq!(mb.try_push(2_000_000 + k), Ok(()));
        assert_eq!(mb.len(), MAILBOX_CAP);
    }
    assert_eq!(mb.pushed(), (MAILBOX_CAP + 1_000) as u64);
    assert_eq!(mb.popped(), 1_000);
    assert!(mb.invariant());
}

// ---------------------------------------------------------------------------
// 3. `retain` (đường gỡ message khi TCB chết/treo) đếm đúng `removed`, giữ FIFO phần còn lại.
// ---------------------------------------------------------------------------

#[test]
fn mailbox_retain_dem_removed_va_giu_fifo() {
    let mut mb: Mailbox<u32> = Mailbox::new();
    for i in 0..MAILBOX_CAP as u32 {
        mb.try_push(i).expect("còn chỗ");
    }
    let n = mb.retain(|v| v % 2 == 0);
    assert_eq!(n, MAILBOX_CAP / 2, "gỡ các message lẻ");
    assert_eq!(mb.len(), MAILBOX_CAP / 2);
    assert_eq!(mb.removed(), (MAILBOX_CAP / 2) as u64);
    assert_eq!(
        mb.iter().copied().collect::<Vec<_>>(),
        vec![0, 2, 4, 6],
        "FIFO của phần còn lại không đổi"
    );
    assert!(mb.invariant(), "removed được tính vào bất biến");
    for _ in 0..(MAILBOX_CAP / 2) {
        mb.try_push(0).expect("chỗ đã được giải phóng");
    }
    assert_eq!(mb.len(), MAILBOX_CAP);
    assert!(mb.invariant());
}

// ---------------------------------------------------------------------------
// 4. Hộp thư theo hart: 1 hart ⇒ `Migrate` **bị từ chối** `NotSupported`, không im lặng bỏ
//    (DoD 2, D2-49(a): push migration hoãn K1c).
// ---------------------------------------------------------------------------

#[test]
fn hart_mailbox_migrate_not_supported_tren_1_hart() {
    let mut r = Rig::new(2);
    let migrate = MailboxMsg::Migrate {
        tcb: r.t[0],
        to_hart: 0,
    };
    assert_eq!(
        r.m.post_mail(0, migrate),
        Err(KError::NotSupported),
        "K1b 1 hart: Migrate không được nhận (không im lặng bỏ)"
    );
    assert_eq!(r.m.mail_len(0), Ok(0), "message bị từ chối không vào hàng");
    assert!(
        r.m.mail_log(0).expect("mail_log").is_empty(),
        "không có message nào được xử lý"
    );
    assert_eq!(
        r.m.send_mail(0, migrate),
        Err(KError::NotSupported),
        "send_mail cũng từ chối, không nửa vời"
    );
    assert_eq!(r.m.mail_len(0), Ok(0));
    assert_eq!(
        r.m.post_mail(1, MailboxMsg::Reschedule),
        Err(KError::InvalidArgument)
    );
    assert_eq!(r.m.mail_len(1), Err(KError::InvalidArgument));
    assert_eq!(r.m.process_mail(1), Err(KError::InvalidArgument));
    assert_eq!(r.m.hart_stopped(1), Err(KError::InvalidArgument));
    r.kiem("migrate bị từ chối");
}

// ---------------------------------------------------------------------------
// 5. Hộp thư theo hart: FIFO, đầy ⇒ `Busy` (không ghi đè), xử lý cuối bước, `Wake` đánh thức,
//    `Stop` đặt cờ (DoD 1/2/6).
// ---------------------------------------------------------------------------

#[test]
fn hart_mailbox_fifo_day_busy_va_xu_ly_cuoi_buoc() {
    let mut r = Rig::new(3);
    // Cho t[0] chặn `Recv` trên ep0 (Ready→BlockedOnRecv là cặp ALLOWED) để `Wake` có tác dụng
    // quan sát được mà không cần `Running→Sleeping`.
    r.recv(0, 0, false).expect("Recv chặn");
    assert!(matches!(r.state(0), TcbState::BlockedOnRecv { .. }));

    // Đầy: 8 `Reschedule` vào hàng; message thứ 9 bị **trả lại** (`Busy`) — không ghi đè.
    for i in 0..MAILBOX_CAP {
        assert_eq!(r.m.post_mail(0, MailboxMsg::Reschedule), Ok(()), "slot {i}");
    }
    assert_eq!(r.m.mail_len(0), Ok(MAILBOX_CAP));
    assert_eq!(
        r.m.post_mail(0, MailboxMsg::Reschedule),
        Err(KError::Busy),
        "hộp thư đầy ⇒ Busy (không panic, không mất message)"
    );
    assert_eq!(r.m.mail_len(0), Ok(MAILBOX_CAP), "Busy không ghi đè");
    assert_eq!(r.m.process_mail(0), Ok(MAILBOX_CAP));
    assert_eq!(r.m.mail_len(0), Ok(0));
    assert_eq!(
        r.m.mail_log(0).expect("mail_log").len(),
        MAILBOX_CAP,
        "xử lý đúng số message đã nhận, không xử lý message bị từ chối"
    );

    // FIFO + xử lý cuối bước: Wake, Reschedule, Stop theo đúng thứ tự vào.
    let msg = [
        MailboxMsg::Wake(r.t[0]),
        MailboxMsg::Reschedule,
        MailboxMsg::Stop,
    ];
    for m in msg {
        assert_eq!(r.m.post_mail(0, m), Ok(()), "còn chỗ ⇒ nhận");
    }
    assert_eq!(r.m.mail_len(0), Ok(3), "hộp thư giữ message tới cuối bước");
    assert_eq!(
        r.m.process_mail(0),
        Ok(3),
        "drain đúng 3 message (không dính 8 message Busy trước)"
    );
    assert_eq!(
        &r.m.mail_log(0).expect("mail_log")[MAILBOX_CAP..],
        &msg[..],
        "drain đúng thứ tự FIFO, đúng 1 lần mỗi message"
    );
    assert_eq!(r.m.mail_len(0), Ok(0), "xử lý xong hộp thư rỗng");
    assert_eq!(r.state(0), TcbState::Ready, "Wake đánh thức TCB đang chặn");
    assert!(r.m.hart_stopped(0).expect("hart_stopped"), "Stop đặt cờ");
    assert_eq!(r.m.process_mail(0), Ok(0), "drain lần nữa idempotent");
    r.kiem("hart mailbox FIFO");

    // `send_mail` = gửi + xử lý trong một bước: không quan sát được trạng thái trung gian.
    r.recv(1, 0, false).expect("Recv chặn t[1]");
    assert_eq!(r.send_mail(MailboxMsg::Wake(r.t[1])), Ok(()));
    assert_eq!(r.state(1), TcbState::Ready);
    assert_eq!(r.m.mail_len(0), Ok(0), "cuối bước hộp thư đã xử lý");
    assert_eq!(
        r.m.mail_log(0).expect("mail_log").last(),
        Some(&MailboxMsg::Wake(r.t[1]))
    );
    r.kiem("hart mailbox send_mail");
}

// ---------------------------------------------------------------------------
// 6. Endpoint đầy ⇒ `Call` trả `Busy`, **không** đổi trạng thái TCB, không mất message;
//    `Recv` giải phóng đúng một chỗ ⇒ gửi lại được (DoD 1/2 + backpressure §4.17).
// ---------------------------------------------------------------------------

#[test]
fn endpoint_day_call_tra_busy_va_recv_giai_phong() {
    let n = MAILBOX_CAP + 1; // 8 client + 1 server
    let mut r = Rig::new(n);
    let srv = n - 1;
    for i in 0..MAILBOX_CAP {
        r.call(i, 0, i as u64).expect("còn chỗ ⇒ Call nhận");
    }
    assert_eq!(r.ep_info(0).len, MAILBOX_CAP);
    assert_eq!(r.ep_info(0).pushed, MAILBOX_CAP as u64);
    // Đầy: client thứ 9 gọi ⇒ `Busy`, TCB **vẫn `Ready`**, không bị xếp hàng/chặn.
    assert_eq!(
        r.call(MAILBOX_CAP, 0, 0x99),
        Err(KError::Busy),
        "hàng gửi đầy ⇒ backpressure Busy (không panic, không mất message)"
    );
    assert_eq!(
        r.state(MAILBOX_CAP),
        TcbState::Ready,
        "Busy không đổi trạng thái"
    );
    assert_eq!(
        r.ep_info(0).len,
        MAILBOX_CAP,
        "Busy không đẩy message vào hàng"
    );
    assert_eq!(r.ep_info(0).pushed, MAILBOX_CAP as u64);
    r.kiem("Call Busy");

    // Server Recv lấy đúng message **cũ nhất** (client t[0]) ⇒ giải phóng 1 chỗ.
    r.recv(srv, 0, true).expect("Recv");
    assert_eq!(r.ep_info(0).len, MAILBOX_CAP - 1);
    assert_eq!(r.ep_info(0).popped, 1);
    assert_eq!(
        r.state(0),
        TcbState::BlockedOnReply { reply: r.rep },
        "Caller của message cũ nhất chuyển BlockedOnReply"
    );
    assert_eq!(r.m.reply_caller(r.rep), Ok(Some(r.t[0])));
    // Có chỗ ⇒ client thứ 9 gửi lại **thành công** (backpressure là thử lại, không mất).
    r.call(MAILBOX_CAP, 0, 0x99)
        .expect("có chỗ ⇒ thử lại thành công");
    assert_eq!(r.ep_info(0).len, MAILBOX_CAP);
    r.kiem("Call sau Recv");
}

// ---------------------------------------------------------------------------
// 7. Nhiều endpoint **độc lập**: đầy ep0 không ảnh hưởng ep1; bộ đếm riêng; Recv ep1 không
//    chạm ep0 (DoD 1).
// ---------------------------------------------------------------------------

#[test]
fn nhieu_endpoint_doc_lap() {
    let n = MAILBOX_CAP + 2; // 8 client ep0 + 1 client ep1 + 1 server
    let mut r = Rig::new(n);
    let srv = n - 1;
    for i in 0..MAILBOX_CAP {
        r.call(i, 0, i as u64).expect("ep0 còn chỗ");
    }
    assert_eq!(r.call(MAILBOX_CAP, 0, 77), Err(KError::Busy), "ep0 đầy");
    // ep1 **độc lập**: vẫn nhận.
    r.call(MAILBOX_CAP, 1, 77).expect("ep1 chưa đầy ⇒ nhận");
    let ep0 = r.ep_info(0);
    let ep1 = r.ep_info(1);
    assert_eq!(
        (ep0.len, ep0.pushed, ep0.popped),
        (MAILBOX_CAP, MAILBOX_CAP as u64, 0)
    );
    assert_eq!((ep1.len, ep1.pushed, ep1.popped), (1, 1, 0));
    assert_eq!(ep0.recv_waiter, None);
    // Recv ep1 chỉ lấy message của ep1; ep0 không đổi.
    r.recv(srv, 1, true).expect("Recv ep1");
    let ep0_sau = r.ep_info(0);
    assert_eq!(
        (ep0_sau.len, ep0_sau.pushed, ep0_sau.popped),
        (MAILBOX_CAP, MAILBOX_CAP as u64, 0),
        "ep0 không bị ảnh hưởng bởi Recv ep1"
    );
    assert_eq!(r.ep_info(1).len, 0);
    assert_eq!(r.ep_info(1).popped, 1);
    // Recv ep0 (đang đầy) lấy đúng message cũ nhất của ep0.
    r.recv(srv, 0, true).expect("Recv ep0");
    assert_eq!(r.ep_info(0).len, MAILBOX_CAP - 1);
    assert_eq!(r.ep_info(0).popped, 1);
    r.kiem("nhiều endpoint");
}

// ---------------------------------------------------------------------------
// 8. `Exit` có `fault_ep`: hàng đầy ⇒ `Busy` và **không** thay đổi gì (không ghi exit_code,
//    không đổi trạng thái); có chỗ ⇒ message fault vào **cuối** hàng FIFO.
// ---------------------------------------------------------------------------

#[test]
fn exit_fault_ep_day_tra_busy_va_message_fault_vao_cuoi_hang() {
    let n = MAILBOX_CAP + 2; // 8 client + 1 TCB exit + 1 server
    let mut r = Rig::new(n);
    let srv = n - 1;
    let exit_tcb = n - 2;
    r.m.tcb_set_fault_ep(r.ts[exit_tcb], r.ep_s[0])
        .expect("SetFaultEp");
    for i in 0..MAILBOX_CAP {
        r.call(i, 0, i as u64).expect("ep0 còn chỗ");
    }
    // Đầy: Exit **không** được thực hiện nửa vời.
    assert_eq!(r.m.exit(r.t[exit_tcb], 7), Err(KError::Busy));
    let info = r.info(exit_tcb);
    assert_eq!(info.exit_code, None, "Busy ⇒ chưa ghi exit_code");
    assert_eq!(
        r.state(exit_tcb),
        TcbState::Ready,
        "Busy ⇒ trạng thái không đổi"
    );
    assert_eq!(r.ep_info(0).len, MAILBOX_CAP, "Busy ⇒ không thêm message");
    r.kiem("Exit Busy");

    // Giải phóng 1 chỗ ⇒ Exit thành công; message fault vào **cuối** hàng (FIFO).
    r.recv(srv, 0, true).expect("Recv giải phóng 1 chỗ");
    r.m.exit(r.t[exit_tcb], 7).expect("Exit khi còn chỗ");
    assert_eq!(r.info(exit_tcb).exit_code, Some(7));
    assert_eq!(
        r.state(exit_tcb),
        TcbState::BlockedOnSend {
            ep: r.ep[0],
            call: false,
            fault: Some(FaultKind::Exit { code: 7 }),
        }
    );
    let items = r.m.endpoint_send_items(r.ep[0]).expect("hàng gửi");
    assert_eq!(items.len(), MAILBOX_CAP, "vẫn đúng sức chứa");
    assert_eq!(
        items.last().copied(),
        Some((r.t[exit_tcb], Some(FaultKind::Exit { code: 7 }))),
        "message fault vào cuối hàng"
    );
    assert_eq!(
        items.first().copied(),
        Some((r.t[1], None)),
        "message cũ nhất sau Recv là client thứ hai"
    );
    r.kiem("Exit sau Recv");
}

// ---------------------------------------------------------------------------
// 9. FIFO **bắc cầu** loại message: Call xong tới fault Exit theo đúng thứ tự đã vào hàng;
//    Recv xử lý lần lượt, không nhảy cóc (DoD 1).
// ---------------------------------------------------------------------------

#[test]
fn recv_fault_message_dung_thu_tu_fifo() {
    let mut r = Rig::new(5);
    let srv = 4;
    let exit_tcb = 3;
    r.m.tcb_set_fault_ep(r.ts[exit_tcb], r.ep_s[0])
        .expect("SetFaultEp");
    r.call(0, 0, 0xA).expect("Call 0");
    r.m.exit(r.t[exit_tcb], 9).expect("Exit có fault_ep");
    r.call(1, 0, 0xB).expect("Call 1");
    let items = r.m.endpoint_send_items(r.ep[0]).expect("hàng gửi");
    assert_eq!(
        items,
        vec![
            (r.t[0], None),
            (r.t[exit_tcb], Some(FaultKind::Exit { code: 9 })),
            (r.t[1], None),
        ],
        "hàng gửi phải đúng thứ tự vào: Call, fault Exit, Call"
    );
    // Recv lần 1: client 0 (Call cũ nhất) → BlockedOnReply.
    r.recv(srv, 0, true).expect("Recv #1");
    assert_eq!(r.state(0), TcbState::BlockedOnReply { reply: r.rep });
    assert_eq!(
        r.state(exit_tcb),
        TcbState::BlockedOnSend {
            ep: r.ep[0],
            call: false,
            fault: Some(FaultKind::Exit { code: 9 }),
        }
    );
    // Recv lần 2: fault Exit → TCB exit thành Inactive (D2-25: procsrv Recv xong mới Inactive).
    r.recv(srv, 0, true).expect("Recv #2");
    assert_eq!(r.state(exit_tcb), TcbState::Inactive);
    // Recv lần 3: client 1.
    r.recv(srv, 0, true).expect("Recv #3");
    assert_eq!(r.state(1), TcbState::BlockedOnReply { reply: r.rep });
    let info = r.ep_info(0);
    assert_eq!((info.len, info.popped, info.removed), (0, 3, 0));
    assert_eq!(info.pushed, 3);
    r.kiem("FIFO bắc cầu");
}

// ---------------------------------------------------------------------------
// 10. Invariant: 1.000 seed × 64 op (stable; Miri giảm seed/bước bằng `cfg(miri)`):
//     không mất/thêm message (pushed == popped + removed + len), không vượt sức chứa, mọi
//     message trong hàng gửi có TCB `BlockedOnSend` đúng; `check_sched` xanh sau **mỗi** op;
//     cùng seed ⇒ cùng trace (DoD 7). Không atomic/thread/loom (D2-49(f)).
// ---------------------------------------------------------------------------

#[cfg(miri)]
const N_SEED: u64 = 3;
#[cfg(not(miri))]
const N_SEED: u64 = 1000;
#[cfg(miri)]
const N_STEP: u64 = 8;
#[cfg(not(miri))]
const N_STEP: u64 = 64;

/// Một dòng trace: `[now, step, op, ok, current, ready_len, state gói 5 TCB, suspended gói,
/// len ep0, len ep1, pushed0, popped0, removed0, pushed1, popped1, removed1, mail_len,
/// mail_log_len, stopped, tổng pushed, tổng ra+gỡ, tổng len]`.
type Row = [u64; 22];

fn run_seed(seed: u64) -> Vec<Row> {
    let n = 5;
    let mut r = Rig::new(n);
    let mut rng = Lcg::new(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ 0x22);
    let mut trace = Vec::with_capacity(N_STEP as usize);
    for step in 0..N_STEP {
        let op = rng.below(100);
        let ti = rng.below(n as u64) as usize;
        let epi = rng.below(2) as usize;
        let ok = match op {
            0..=17 => r.call(ti, epi, step).is_ok(),
            18..=33 => r.recv(ti, epi, rng.below(2) == 0).is_ok(),
            34..=41 => r.reply(ti).is_ok(),
            42..=51 => r.m.exit(r.t[ti], step).is_ok(),
            52..=57 => r.m.wake(r.t[ti]).is_ok(),
            58..=63 => r.m.tcb_suspend(r.ts[ti], SuspendMode::Abort).is_ok(),
            64..=69 => r.m.tcb_resume(r.ts[ti]).is_ok(),
            70..=77 => r.m.dispatch(0).map(|x| x.is_some()).unwrap_or(false),
            78..=81 => r.m.yield_current(0).is_ok(),
            82..=85 => r.m.tick(1).is_ok(),
            86..=90 => match r.m.post_mail(0, MailboxMsg::Wake(r.t[ti])) {
                Ok(()) => true,
                Err(KError::Busy) => false,
                Err(_) => false,
            },
            91..=94 => r.m.process_mail(0).is_ok(),
            95..=97 => match r.send_mail(MailboxMsg::Wake(r.t[ti])) {
                Ok(()) => true,
                Err(KError::Busy) => false,
                Err(_) => false,
            },
            _ => r.send_mail(MailboxMsg::Reschedule).is_ok(),
        };
        r.kiem("bước ngẫu nhiên (mailbox)");
        assert!(
            r.m.mail_len(0).expect("mail_len") <= MAILBOX_CAP,
            "hộp thư hart vượt sức chứa (seed {seed})"
        );
        let mut row = [0u64; 22];
        row[0] = r.m.virtual_now();
        row[1] = step;
        row[2] = op;
        row[3] = ok as u64;
        row[4] = r.m.current(0).unwrap_or(0) as u64;
        row[5] = r.m.ready_len(0) as u64;
        for i in 0..n {
            let st = r.m.tcb_state(r.t[i]).expect("TCB còn sống");
            row[6] |= (st.index() as u64) << (4 * i);
            row[7] |= (r.info(i).suspended as u64) << i;
        }
        let a = r.ep_info(0);
        let b = r.ep_info(1);
        row[8] = a.len as u64;
        row[9] = b.len as u64;
        row[10] = a.pushed;
        row[11] = a.popped;
        row[12] = a.removed;
        row[13] = b.pushed;
        row[14] = b.popped;
        row[15] = b.removed;
        row[16] = r.m.mail_len(0).expect("mail_len") as u64;
        row[17] = r.m.mail_log(0).expect("mail_log").len() as u64;
        row[18] = r.m.hart_stopped(0).expect("hart_stopped") as u64;
        // Tổng message đã vào hàng hai endpoint và tổng đã ra + bị gỡ — không âm/không vượt.
        row[19] = a.pushed + b.pushed;
        row[20] = a.popped + a.removed + b.popped + b.removed;
        row[21] = (a.len + b.len) as u64;
        assert_eq!(
            row[19],
            row[20] + row[21],
            "seed {seed} step {step}: message bị mất/thêm"
        );
        trace.push(row);
    }
    trace
}

#[test]
fn invariant_1000_seed_64_op_mailbox() {
    // Cùng seed ⇒ cùng trace (byte-for-byte ở mức `Row`), không phụ thuộc thời gian thực.
    for seed in 0..16 {
        assert_eq!(run_seed(seed), run_seed(seed), "seed {seed} phải tất định");
    }
    let mut tong_message = 0u64;
    let mut tong_xu_ly_mail = 0u64;
    let mut seed_co_mail = 0u64;
    for seed in 0..N_SEED {
        let rows = run_seed(seed);
        assert_eq!(rows.len(), N_STEP as usize);
        let cuoi = rows.last().expect("trace khác rỗng");
        tong_message += cuoi[19];
        let xu_ly = rows.iter().map(|r| r[17]).max().expect("trace khác rỗng");
        tong_xu_ly_mail += xu_ly;
        if xu_ly > 0 {
            seed_co_mail += 1;
        }
    }
    if cfg!(not(miri)) {
        assert!(
            tong_message >= 1_000,
            "kịch bản phải thực sự đẩy message qua mailbox, thực tế {tong_message}"
        );
        // Kịch bản phải thực sự đi qua **đường hộp thư hart** (không phải chỉ endpoint):
        // 1.000 seed × 64 op ⇒ hàng trăm seed có message được xử lý.
        assert!(
            tong_xu_ly_mail >= 100,
            "kịch bản phải xử lý message qua hộp thư hart, thực tế {tong_xu_ly_mail}"
        );
        assert!(
            seed_co_mail >= N_SEED / 2,
            "chỉ {seed_co_mail}/{N_SEED} seed đi qua hộp thư hart"
        );
    }
}
