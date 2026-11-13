// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-21 — **MCS donation**: `SchedContext` của client đi theo `Call` sang server **passive**
//! (server không có SC), thời gian phục vụ tính vào ngân sách **của client**, `Reply` trả SC về;
//! server **có SC riêng giữ SC riêng** (không nhận donation); `max_controlled_priority` (MCP)
//! giới hạn priority khi server chạy bằng SC được donate.
//!
//! **Model-only `[TK]`, không phải spec/ABI** (D2-49(c)): các method chưa có chữ ký trong draft
//! D2-33 nên K1b chỉ cài model API, không tuyên bố ABI.
//!
//! Nguồn ngữ nghĩa: thiết kế §4.5 (`docs/v2/ViFastOs-v2-thiet-ke.md:288-311`), **D2-49(c)**
//! ("donation **tự động** qua Call/Reply; server có SC riêng giữ SC riêng"), **D2-50** ("TCB
//! không SC **không chạy**, trừ khi được donation"), **D2-16** (MCS donation từ K1b),
//! kế hoạch `review/agent0/K1B-PLAN.md` §5 KT-21 (DoD 1–8).
//!
//! Quy ước mô hình chốt ở đây (ghi rõ để review; xem thêm `src/mcs.rs`, `src/kobj.rs`):
//!
//! 1. **Donation tự động, một chiều, không chuỗi** (D2-49(c); K1B-PLAN §5 KT-21 rủi ro (a)):
//!    `Call` của client có SC **đã `Configure`** tới server **không có** SC ⇒ SC được đánh dấu
//!    `donated_to = server`, `donor = client`; server chạy bằng ngân sách của client. Server đang
//!    nhận donation **không** donate tiếp (chuỗi >1 thuộc K1c); server có SC riêng **không** nhận
//!    donation (D2-49(c)).
//! 2. **Đường donate** là hai chỗ giao message của `Call`: (a) `Call` gặp server đang `Recv`;
//!    (b) server `Recv` lấy message `Call` còn nằm trong hàng gửi. Cả hai đều đi qua
//!    `transition_tcb` (một cửa KT-18) rồi mới đặt donation.
//! 3. **Priority khi được donate** `[TK]`: `p_eff = min(client.priority, client.mcp,
//!    server.mcp)`. `client.priority ≤ client.mcp` đã là bất biến của `Tcb.SetPriority`
//!    (`kobj.rs:1397`) nên **chặn thực tế** là MCP của **server**: client mức cao hơn MCP của
//!    server ⇒ server chạy **bị kẹp** xuống `server.mcp` (không chạy vượt MCP, không đảo ưu
//!    tiên); client mức thấp hơn ⇒ server chạy thấp hơn priority gốc của nó (không được nâng).
//!    Priority **gốc** của server (`TcbInfo.priority`) **không** bị đổi — chỉ bucket ready queue
//!    và `cur_prio` dùng `p_eff` (`Machine::effective_priority`).
//! 4. **Charge vào SC của client** chỉ khi server `Running`, mỗi tick 1 đơn vị (`tick` của
//!    KT-20); client đang `BlockedOnReply` **không** bị charge. Hết budget ⇒ **server**
//!    `Sleeping{Budget}` (không phải client), refill ⇒ server `Ready` lại; `Reply` hoàn tất,
//!    không mất reply, không deadlock.
//! 5. **Trả SC**: `Reply` ⇒ `donated_to = None`, server mất ngân sách (passive ⇒ không được
//!    `pick`), client `Ready`, budget còn lại **giữ nguyên** (`consumed` không bị reset).
//! 6. **Huỷ donation** `[TK]` khi một bên chết/treo: `Suspend(Abort)`/`Exit`/finalise server ⇒
//!    SC trả về client và client được **đánh thức** (`Reply.caller = None`; mã kết quả
//!    `Cancelled` đầy đủ thuộc K1c); `Suspend(Abort)`/`Exit` client ⇒ SC **thu hồi**, server
//!    `Running` không còn ngân sách ⇒ `Sleeping{Budget}` ngay (D2-50). Không rò SC.
//! 7. **Bất biến** (kiểm trong `check_sched` sau **mỗi** op): một SC chỉ có **một** TCB dùng tại
//!    một thời điểm; donor và donee **không** cùng `Running`; liên kết donation **hai chiều**
//!    khớp (`SchedContext.donated_to/donor` ↔ `Tcb.donated_sc`); donee không có SC riêng;
//!    `consumed` đơn điệu trong chu kỳ và `≤ budget`.
//!
//! Test đỏ trước (D2-05): tệp này **không biên dịch** trước KT-21 (API `sc_of`,
//! `sched_donated_to`, `sched_donor_of`, `donated_sc`, `effective_priority` chưa có) —
//! `JOURNAL/kt21-red-compile.log`; sau đó `JOURNAL/kt21-red-charge.log` chứng minh hành vi cũ
//! **sai** (server passive không bao giờ chạy, SC của client không bị trừ).

use vifast_sim::kobj::{
    KError, Machine, ObjId, ObjType, ScInfo, SleepReason, SlotRef, SuspendMode, TcbState,
};
use vifast_sim::Lcg;

fn slot(cnode: ObjId, s: usize) -> SlotRef {
    SlotRef::new(cnode, s)
}

/// Bất biến scheduler + donation (`check_sched`) sau **mỗi** thao tác công khai.
fn kiem(m: &Machine, buoc: &str) {
    if let Err(e) = m.check_sched() {
        panic!("check_sched sau {buoc}: {e}");
    }
}

fn sleep_budget() -> TcbState {
    TcbState::Sleeping {
        reason: SleepReason::Budget,
    }
}

// ---------------------------------------------------------------------------
// Rig: CNode 32 slot; 1 SchedControl; 1 Endpoint; 2 Reply; 1 Notification;
// 3 SchedContext; 4 TCB —
//   t[0] client (SC0), t[1] server **passive** (không SC),
//   t[2] server **có SC riêng** (SC1), t[3] client thứ hai (SC2).
// ---------------------------------------------------------------------------

const S_CTL: usize = 0;
const S_UNTYPED: usize = 1;
const S_EP: usize = 2;
const S_REP: [usize; 2] = [4, 5];
const S_NTFN: usize = 6;
const S_SC: [usize; 3] = [8, 9, 10];
const S_TCB: [usize; 4] = [16, 17, 18, 19];

struct Rig {
    m: Machine,
    ctl: SlotRef,
    ep: SlotRef,
    rep: [SlotRef; 2],
    rep_obj: [ObjId; 2],
    ntfn: SlotRef,
    sc: [SlotRef; 3],
    sc_obj: [ObjId; 3],
    ts: [SlotRef; 4],
    t: [ObjId; 4],
}

fn rig() -> Rig {
    let mut m = Machine::new();
    let c0 = m.add_cnode(32);
    let ctl = m.add_kernel_object(ObjType::SchedControl);
    let u0 = m.add_untyped(0x1000, 65536);
    let ep = m.add_kernel_object(ObjType::Endpoint);
    let rep0 = m.add_kernel_object(ObjType::Reply);
    let rep1 = m.add_kernel_object(ObjType::Reply);
    let ntfn = m.add_kernel_object(ObjType::Notification);
    for (s, obj) in [
        (S_CTL, ctl),
        (S_UNTYPED, u0),
        (S_EP, ep),
        (S_REP[0], rep0),
        (S_REP[1], rep1),
        (S_NTFN, ntfn),
    ] {
        let cap = m.root_cap(obj);
        m.put_cap(c0, s, cap).expect("cap object vào CNode");
    }
    let mut sc = [slot(c0, 0); 3];
    let mut sc_obj = [0u32; 3];
    for (i, s) in S_SC.iter().enumerate() {
        let obj = m.add_kernel_object(ObjType::SchedContext);
        let cap = m.root_cap(obj);
        m.put_cap(c0, *s, cap).expect("cap SchedContext");
        sc[i] = slot(c0, *s);
        sc_obj[i] = obj;
    }
    let mut ts = [slot(c0, 0); 4];
    let mut t = [0u32; 4];
    for (i, s) in S_TCB.iter().enumerate() {
        let obj = m.add_kernel_object(ObjType::Tcb);
        let cap = m.root_cap(obj);
        m.put_cap(c0, *s, cap).expect("cap TCB");
        ts[i] = slot(c0, *s);
        t[i] = obj;
    }
    Rig {
        m,
        ctl: slot(c0, S_CTL),
        ep: slot(c0, S_EP),
        rep: [slot(c0, S_REP[0]), slot(c0, S_REP[1])],
        rep_obj: [rep0, rep1],
        ntfn: slot(c0, S_NTFN),
        sc,
        sc_obj,
        ts,
        t,
    }
}

impl Rig {
    fn configure(&mut self, i: usize, budget: u64, period: u64) {
        self.m
            .sched_control_configure(self.ctl, self.sc[i], budget, period)
            .expect("SchedControl.Configure hợp lệ");
        kiem(&self.m, "SchedControl.Configure");
    }

    fn attach(&mut self, tcb: usize, sched: usize) {
        self.m
            .set_sched_params(self.ts[tcb], Some(self.sc[sched]))
            .expect("set_sched_params");
        kiem(&self.m, "set_sched_params");
    }

    /// `Tcb.SetPriority` + `Tcb.SetMcp` (model API `[TK]`, KT-18 DoD 1).
    fn prio(&mut self, tcb: usize, p: u8, mcp: u8) {
        self.m
            .tcb_set_priority(self.ts[tcb], p, mcp)
            .expect("Tcb.SetPriority");
        kiem(&self.m, "Tcb.SetPriority");
    }

    /// `Inactive → Ready` (cặp ALLOWED của bảng KT-18, vào hàng qua một cửa KT-19).
    fn ready(&mut self, tcb: usize) {
        self.m
            .transition_tcb(self.t[tcb], TcbState::Ready)
            .expect("Inactive→Ready");
        kiem(&self.m, "transition Ready");
    }

    fn dispatch(&mut self) -> Option<ObjId> {
        let got = self.m.dispatch(0).expect("dispatch");
        kiem(&self.m, "dispatch");
        got
    }

    fn tick(&mut self, n: u64) {
        self.m.tick(n).expect("tick");
        kiem(&self.m, "tick");
    }

    fn info(&self, i: usize) -> ScInfo {
        self.m.sched_info(self.sc[i]).expect("sched_info")
    }

    fn state(&self, i: usize) -> TcbState {
        self.m.tcb_state(self.t[i]).expect("tcb_state")
    }

    fn eff(&self, i: usize) -> u8 {
        self.m
            .effective_priority(self.t[i])
            .expect("effective_priority")
    }
}

// ---------------------------------------------------------------------------
// 1. Server **passive** chỉ chạy khi được donate: trước `Call` `pick` không chọn;
//    sau `Call` chạy bằng SC của client (charge vào SC client); `Reply` xong không chạy nữa
//    (DoD 1 + 2, K1B-PLAN §5 KT-21 #1/#2/#3).
// ---------------------------------------------------------------------------

#[test]
fn server_passive_chi_chay_khi_duoc_donate() {
    let mut r = rig();
    r.configure(0, 4, 8);
    r.attach(0, 0); // client t[0] có SC0
    r.prio(0, 100, 255);
    r.prio(1, 10, 200); // server passive t[1]

    // Server chờ `Recv`; đánh thức nó để chứng minh TCB **Ready mà không có SC vẫn không được
    // pick** (D2-50), rồi cho nó `Recv` lại.
    r.m.recv(r.t[1], r.ep, Some(r.rep[0])).expect("Recv");
    kiem(&r.m, "Recv");
    assert!(matches!(r.state(1), TcbState::BlockedOnRecv { .. }));
    r.m.wake(r.t[1]).expect("wake");
    kiem(&r.m, "wake");
    assert_eq!(r.state(1), TcbState::Ready);
    assert!(r.m.ready_contains(0, r.t[1]), "Ready phải nằm trong hàng");
    assert!(!r.m.tcb_has_budget(r.t[1]), "không SC ⇒ không có ngân sách");
    assert_eq!(
        r.m.sched_pick(),
        None,
        "SC-less Ready KHÔNG được pick (D2-50)"
    );
    assert_eq!(r.dispatch(), None);
    r.m.recv(r.t[1], r.ep, Some(r.rep[0])).expect("Recv lại");
    kiem(&r.m, "Recv lại");

    // Client `Call` ⇒ SC0 được donate sang server.
    r.m.call(r.t[0], r.ep, 0x42).expect("Call");
    kiem(&r.m, "Call");
    assert!(matches!(r.state(0), TcbState::BlockedOnReply { .. }));
    assert_eq!(r.state(1), TcbState::Ready, "server được đánh thức");
    assert_eq!(
        r.m.donated_sc(r.t[1]),
        Ok(Some(r.sc_obj[0])),
        "server passive nhận donation"
    );
    assert_eq!(
        r.m.sc_of(r.t[1]),
        Ok(Some(r.sc_obj[0])),
        "SC đang cấp ngân sách cho server là SC của client"
    );
    assert_eq!(
        r.m.sc_of(r.t[0]),
        Ok(None),
        "SC đã rời client (đi theo Call)"
    );
    assert_eq!(r.m.sched_donated_to(r.sc[0]), Ok(Some(r.t[1])));
    assert_eq!(r.m.sched_donor_of(r.sc[0]), Ok(Some(r.t[0])));
    assert!(
        r.m.tcb_has_budget(r.t[1]),
        "server chạy bằng ngân sách client"
    );
    assert_eq!(r.m.sched_pick(), Some(r.t[1]));
    assert_eq!(r.dispatch(), Some(r.t[1]));
    assert_eq!(r.state(1), TcbState::Running { hart: 0 });

    // Thời gian phục vụ tính vào SC **của client**; client đang chờ **không** bị charge.
    r.tick(2);
    assert_eq!(r.info(0).consumed, 2, "charge vào SC client");
    assert_eq!(r.info(0).charged, 2);
    assert_eq!(r.state(1), TcbState::Running { hart: 0 });
    assert!(matches!(r.state(0), TcbState::BlockedOnReply { .. }));

    // `Reply` trả SC về client; server passive mất ngân sách ⇒ không được pick nữa.
    r.m.reply(r.t[1], r.rep[0]).expect("Reply");
    kiem(&r.m, "Reply");
    assert_eq!(r.m.sched_donated_to(r.sc[0]), Ok(None));
    assert_eq!(r.m.sched_donor_of(r.sc[0]), Ok(None));
    assert_eq!(r.m.donated_sc(r.t[1]), Ok(None));
    assert_eq!(
        r.m.sc_of(r.t[1]),
        Ok(None),
        "server trở lại trạng thái không SC"
    );
    assert!(!r.m.tcb_has_budget(r.t[1]));
    assert_eq!(r.m.sc_of(r.t[0]), Ok(Some(r.sc_obj[0])));
    assert_eq!(r.state(0), TcbState::Ready);
    assert_eq!(r.state(1), TcbState::Ready);
    assert_eq!(r.m.ready_len_at(0, 100), 1, "client ở bucket 100");
    assert_eq!(r.m.ready_len_at(0, 10), 1, "server về lại bucket gốc 10");
    assert_eq!(r.m.sched_pick(), Some(r.t[0]), "chỉ client còn ngân sách");
    assert_eq!(r.dispatch(), Some(r.t[0]));
    r.tick(1);
    assert_eq!(r.info(0).consumed, 3, "budget còn lại giữ nguyên + 1 tick");
}

// ---------------------------------------------------------------------------
// 2. Charge đúng budget khi server chạy; hết budget ⇒ server `Sleeping{Budget}` rồi refill;
//    `Reply` hoàn tất sau refill; client cạn budget ⇒ `Sleeping{Budget}` (DoD 2 + 4).
// ---------------------------------------------------------------------------

#[test]
fn charge_budget_het_can_refill_va_reply_khong_mat() {
    let mut r = rig();
    r.configure(0, 2, 8);
    r.attach(0, 0);
    r.prio(0, 100, 255);
    r.prio(1, 10, 200);
    r.m.recv(r.t[1], r.ep, Some(r.rep[0])).expect("Recv");
    r.m.call(r.t[0], r.ep, 1).expect("Call");
    kiem(&r.m, "Call");
    assert_eq!(r.dispatch(), Some(r.t[1]));
    r.tick(1);
    assert_eq!(r.info(0).consumed, 1);
    assert_eq!(r.info(0).charged, 1);

    // Tick thứ 2 làm cạn budget: **server** ngủ, client vẫn `BlockedOnReply` (không chết cứng).
    r.tick(1);
    assert_eq!(r.info(0).consumed, 2, "charge đúng biên, không budget + 1");
    assert_eq!(r.state(1), sleep_budget(), "server ngủ vì SC client cạn");
    assert!(matches!(r.state(0), TcbState::BlockedOnReply { .. }));
    assert_eq!(r.m.current(0), None);
    assert_eq!(r.m.sched_pick(), None);
    r.tick(5); // tới now = 7, chưa chạm biên 8
    assert_eq!(r.m.virtual_now(), 7);
    assert_eq!(r.state(1), sleep_budget());
    r.tick(1); // now = 8 ⇒ refill
    assert_eq!(r.info(0).consumed, 0);
    assert_eq!(r.state(1), TcbState::Ready, "refill đánh thức server");
    assert_eq!(r.m.sched_pick(), Some(r.t[1]));
    assert_eq!(r.dispatch(), Some(r.t[1]));
    r.tick(1);
    assert_eq!(r.info(0).consumed, 1);

    // `Reply` hoàn tất: client `Ready` (không mất reply), budget còn 1.
    assert_eq!(r.m.reply_caller(r.rep_obj[0]), Ok(Some(r.t[0])));
    r.m.reply(r.t[1], r.rep[0]).expect("Reply");
    kiem(&r.m, "Reply sau refill");
    assert_eq!(r.m.reply_caller(r.rep_obj[0]), Ok(None));
    assert_eq!(r.state(0), TcbState::Ready);
    assert_eq!(r.m.sc_of(r.t[0]), Ok(Some(r.sc_obj[0])));
    assert_eq!(r.info(0).consumed, 1, "Reply không reset consumed");
    assert_eq!(r.dispatch(), Some(r.t[0]));
    r.tick(1);
    assert_eq!(r.info(0).consumed, 2);
    assert_eq!(
        r.state(0),
        sleep_budget(),
        "client cạn budget ⇒ Sleeping{{Budget}}"
    );
    r.tick(8); // now = 17… biên kế tiếp sau 9 là 16 ⇒ đã refill
    assert_eq!(r.state(0), TcbState::Ready);
}

// ---------------------------------------------------------------------------
// 3. Server **có SC riêng** giữ SC riêng — không nhận donation, không bị đổi budget; client
//    không bị trừ cho thời gian server chạy (D2-49(c)).
// ---------------------------------------------------------------------------

#[test]
fn server_co_sc_rieng_giu_sc_rieng() {
    let mut r = rig();
    r.configure(0, 4, 8);
    r.attach(0, 0);
    r.configure(1, 3, 8);
    r.attach(2, 1); // server t[2] có SC1
    r.prio(0, 100, 255);
    r.prio(2, 20, 200);
    let truoc = r.info(1);

    r.m.recv(r.t[2], r.ep, Some(r.rep[0])).expect("Recv");
    r.m.call(r.t[0], r.ep, 7).expect("Call");
    kiem(&r.m, "Call tới server có SC riêng");
    assert_eq!(
        r.m.sched_donated_to(r.sc[0]),
        Ok(None),
        "server có SC riêng KHÔNG nhận donation (D2-49c)"
    );
    assert_eq!(r.m.sched_donated_to(r.sc[1]), Ok(None));
    assert_eq!(r.m.sc_of(r.t[2]), Ok(Some(r.sc_obj[1])));
    assert_eq!(r.m.sc_of(r.t[0]), Ok(Some(r.sc_obj[0])));
    assert_eq!(r.eff(2), 20, "chạy ở priority gốc, không bị donation đổi");
    assert_eq!(r.m.sched_pick(), Some(r.t[2]));
    assert_eq!(r.dispatch(), Some(r.t[2]));
    r.tick(2);
    assert_eq!(r.info(1).consumed, 2, "charge vào SC riêng của server");
    assert_eq!(r.info(0).consumed, 0, "client không bị trừ");
    assert_eq!(r.info(1).budget - r.info(1).consumed, 1);
    assert!(r.m.tcb_has_budget(r.t[2]));

    r.m.reply(r.t[2], r.rep[0]).expect("Reply");
    kiem(&r.m, "Reply server có SC riêng");
    assert_eq!(r.m.sc_of(r.t[2]), Ok(Some(r.sc_obj[1])), "vẫn giữ SC riêng");
    assert!(r.m.tcb_has_budget(r.t[2]));
    assert_eq!(r.state(0), TcbState::Ready);
    // Cả hai `Ready`: client mức 100 thắng server mức 20 nhưng SC riêng vẫn còn.
    assert_eq!(r.m.sched_pick(), Some(r.t[0]));
    assert_eq!(r.info(1).budget, truoc.budget);
    assert_eq!(r.m.sched_donated_to(r.sc[0]), Ok(None));
}

// ---------------------------------------------------------------------------
// 4. MCP: server không chạy vượt MCP khi được donate; priority cao/thấp đều đúng, không đảo
//    ưu tiên (DoD 6).
// ---------------------------------------------------------------------------

#[test]
fn mcp_kep_priority_khi_duoc_donate() {
    // (a) Client 100, server gốc 10, MCP server 200 ⇒ server chạy ở 100 (kế thừa).
    let mut r = rig();
    r.configure(0, 4, 8);
    r.attach(0, 0);
    r.prio(0, 100, 255);
    r.prio(1, 10, 200);
    r.m.recv(r.t[1], r.ep, Some(r.rep[0])).expect("Recv");
    r.m.call(r.t[0], r.ep, 1).expect("Call");
    kiem(&r.m, "Call (a)");
    assert_eq!(r.eff(1), 100, "server chạy ở priority client");
    assert_eq!(r.m.ready_len_at(0, 100), 1);
    assert_eq!(r.m.ready_len_at(0, 10), 0, "không còn ở bucket gốc");
    assert_eq!(r.dispatch(), Some(r.t[1]));
    assert_eq!(r.m.sched_cur_prio(0), 100);

    // (b) Priority client 100 **vượt** MCP 40 của server ⇒ bị **kẹp** xuống 40, không chạy 100.
    let mut r = rig();
    r.configure(0, 4, 8);
    r.attach(0, 0);
    r.prio(0, 100, 255);
    r.prio(1, 10, 40);
    r.m.recv(r.t[1], r.ep, Some(r.rep[0])).expect("Recv");
    r.m.call(r.t[0], r.ep, 1).expect("Call");
    kiem(&r.m, "Call (b)");
    assert_eq!(r.eff(1), 40, "kẹp theo MCP của server");
    assert!(r.eff(1) <= 40, "không vượt MCP");
    assert_eq!(r.m.ready_len_at(0, 40), 1);
    assert_eq!(r.m.ready_len_at(0, 100), 0);
    assert_eq!(r.dispatch(), Some(r.t[1]));
    assert_eq!(r.m.sched_cur_prio(0), 40, "chạy ở 40, KHÔNG 100");

    // (c) Client 20 thấp hơn priority gốc 50 của server ⇒ server chạy 20 khi được donate
    //     (không được nâng), trả SC xong về lại 50 (không đảo ưu tiên).
    let mut r = rig();
    r.configure(0, 4, 8);
    r.attach(0, 0);
    r.prio(0, 20, 255);
    r.prio(1, 50, 200);
    r.m.recv(r.t[1], r.ep, Some(r.rep[0])).expect("Recv");
    r.m.call(r.t[0], r.ep, 1).expect("Call");
    kiem(&r.m, "Call (c)");
    assert_eq!(r.eff(1), 20, "không vượt priority client");
    assert_eq!(r.m.ready_len_at(0, 20), 1);
    assert_eq!(r.dispatch(), Some(r.t[1]));
    assert_eq!(r.m.sched_cur_prio(0), 20);
    r.m.reply(r.t[1], r.rep[0]).expect("Reply");
    kiem(&r.m, "Reply (c)");
    assert_eq!(r.eff(1), 50, "trả SC ⇒ về priority gốc");
    assert_eq!(r.m.ready_len_at(0, 50), 1);
    assert_eq!(
        r.m.sched_cur_prio(0),
        0,
        "hart rảnh sau Reply (không dispatch)"
    );
}

// ---------------------------------------------------------------------------
// 5. `Suspend(Abort)`/`Exit` server đang được donate ⇒ SC trả về client, client được đánh
//    thức, không rò SC (DoD 5 nhánh 1).
// ---------------------------------------------------------------------------

#[test]
fn suspend_abort_server_dang_donate_tra_sc_va_danh_thuc_client() {
    let mut r = rig();
    r.configure(0, 4, 8);
    r.attach(0, 0);
    r.prio(0, 100, 255);
    r.prio(1, 10, 200);
    r.m.recv(r.t[1], r.ep, Some(r.rep[0])).expect("Recv");
    r.m.call(r.t[0], r.ep, 1).expect("Call");
    assert_eq!(r.dispatch(), Some(r.t[1]));
    assert!(matches!(r.state(1), TcbState::Running { .. }));

    r.m.tcb_suspend(r.ts[1], SuspendMode::Abort)
        .expect("Suspend(Abort) server");
    kiem(&r.m, "Suspend(Abort) server");
    assert_eq!(r.state(0), TcbState::Ready, "client được đánh thức");
    assert_eq!(
        r.m.reply_caller(r.rep_obj[0]),
        Ok(None),
        "reply bị huỷ, không treo client"
    );
    assert_eq!(r.m.sched_donated_to(r.sc[0]), Ok(None));
    assert_eq!(r.m.donated_sc(r.t[1]), Ok(None));
    assert_eq!(r.m.sc_of(r.t[1]), Ok(None), "không rò SC về server");
    assert_eq!(r.m.sc_of(r.t[0]), Ok(Some(r.sc_obj[0])));
    assert!(r.m.tcb_has_budget(r.t[0]));
    let info = r.m.tcb_info(r.ts[1]).expect("TcbInfo");
    assert!(info.suspended);
    assert_eq!(info.state, TcbState::Ready);
    assert_eq!(r.m.sched_pick(), Some(r.t[0]));

    // Nhánh `Exit`: server đang `Running` với SC được donate.
    let mut r = rig();
    r.configure(0, 4, 8);
    r.attach(0, 0);
    r.prio(0, 100, 255);
    r.prio(1, 10, 200);
    r.m.recv(r.t[1], r.ep, Some(r.rep[0])).expect("Recv");
    r.m.call(r.t[0], r.ep, 1).expect("Call");
    assert_eq!(r.dispatch(), Some(r.t[1]));
    r.m.exit(r.t[1], 9).expect("Exit server");
    kiem(&r.m, "Exit server");
    assert_eq!(r.state(1), TcbState::Inactive);
    assert_eq!(r.m.sched_donated_to(r.sc[0]), Ok(None));
    assert_eq!(r.m.donated_sc(r.t[1]), Ok(None));
    assert_eq!(r.state(0), TcbState::Ready);
    assert_eq!(r.m.reply_caller(r.rep_obj[0]), Ok(None));
    assert_eq!(r.m.sc_of(r.t[0]), Ok(Some(r.sc_obj[0])));

    // Nhánh **finalise** (hook `Revoke`/finalise của KT-18): SC cũng phải trả về client và client
    // được đánh thức trước khi TCB chết.
    let mut r = rig();
    r.configure(0, 4, 8);
    r.attach(0, 0);
    r.prio(0, 100, 255);
    r.prio(1, 10, 200);
    r.m.recv(r.t[1], r.ep, Some(r.rep[0])).expect("Recv");
    r.m.call(r.t[0], r.ep, 1).expect("Call");
    assert_eq!(r.dispatch(), Some(r.t[1]));
    r.m.finalise_tcb(r.t[1]).expect("finalise server");
    kiem(&r.m, "finalise server");
    assert_eq!(r.state(1), TcbState::Dead);
    assert_eq!(r.m.sched_donated_to(r.sc[0]), Ok(None));
    assert_eq!(r.m.donated_sc(r.t[1]), Ok(None));
    assert_eq!(r.state(0), TcbState::Ready);
    assert_eq!(r.m.reply_caller(r.rep_obj[0]), Ok(None));
    assert_eq!(r.m.sc_of(r.t[0]), Ok(Some(r.sc_obj[0])));
    assert!(r.m.tcb_has_budget(r.t[0]));
}

// ---------------------------------------------------------------------------
// 6. Huỷ donation từ phía **client**: SC thu hồi, server đang chạy mất ngân sách ⇒
//    `Sleeping{Budget}` ngay (D2-50), không rò SC (DoD 5 nhánh 2).
// ---------------------------------------------------------------------------

#[test]
fn abort_client_dang_donate_thu_hoi_sc_khong_ro() {
    let mut r = rig();
    r.configure(0, 4, 8);
    r.attach(0, 0);
    r.prio(0, 100, 255);
    r.prio(1, 10, 200);
    r.m.recv(r.t[1], r.ep, Some(r.rep[0])).expect("Recv");
    r.m.call(r.t[0], r.ep, 1).expect("Call");
    assert_eq!(r.dispatch(), Some(r.t[1]));
    assert_eq!(r.info(0).consumed, 0);

    r.m.tcb_suspend(r.ts[0], SuspendMode::Abort)
        .expect("Suspend(Abort) client");
    kiem(&r.m, "Suspend(Abort) client");
    assert_eq!(r.m.sched_donated_to(r.sc[0]), Ok(None));
    assert_eq!(r.m.donated_sc(r.t[1]), Ok(None));
    assert_eq!(r.m.sc_of(r.t[1]), Ok(None));
    assert!(!r.m.tcb_has_budget(r.t[1]));
    assert_eq!(
        r.state(1),
        sleep_budget(),
        "server Running mất SC ⇒ Sleeping{{Budget}} (D2-50)"
    );
    assert_eq!(r.m.current(0), None);
    assert_eq!(r.m.sc_of(r.t[0]), Ok(Some(r.sc_obj[0])), "SC về client");
    let info = r.m.tcb_info(r.ts[0]).expect("TcbInfo");
    assert!(info.suspended);
    assert_eq!(info.state, TcbState::Ready);
    assert_eq!(r.m.reply_caller(r.rep_obj[0]), Ok(None));

    // Nhánh finalise **client**: thu hồi SC khỏi server trước khi TCB chết (không rò SC).
    let mut r = rig();
    r.configure(0, 4, 8);
    r.attach(0, 0);
    r.prio(0, 100, 255);
    r.prio(1, 10, 200);
    r.m.recv(r.t[1], r.ep, Some(r.rep[0])).expect("Recv");
    r.m.call(r.t[0], r.ep, 1).expect("Call");
    assert_eq!(r.dispatch(), Some(r.t[1]));
    r.m.finalise_tcb(r.t[0]).expect("finalise client");
    kiem(&r.m, "finalise client");
    assert_eq!(r.state(0), TcbState::Dead);
    assert_eq!(r.m.sched_donated_to(r.sc[0]), Ok(None));
    assert_eq!(r.m.donated_sc(r.t[1]), Ok(None));
    assert_eq!(r.state(1), sleep_budget());
}

// ---------------------------------------------------------------------------
// 7. `SchedContext.Bind` chỉ gắn/kiểm quan hệ SC–Reply/Notification, **không** tạo donation
//    thứ hai (DoD 3).
// ---------------------------------------------------------------------------

#[test]
fn sched_context_bind_khong_tao_donation() {
    let mut r = rig();
    r.configure(0, 4, 8);
    r.attach(0, 0);
    r.prio(0, 100, 255);
    r.prio(1, 10, 200);
    r.m.sched_context_bind(r.sc[0], r.rep[0])
        .expect("Bind(sc, reply)");
    kiem(&r.m, "Bind(sc, reply)");
    r.m.sched_context_bind(r.sc[0], r.ntfn)
        .expect("Bind(sc, notification)");
    kiem(&r.m, "Bind(sc, notification)");
    assert_eq!(
        r.m.sched_donated_to(r.sc[0]),
        Ok(None),
        "Bind KHÔNG tạo donation"
    );
    assert_eq!(r.info(0).consumed, 0);

    r.m.recv(r.t[1], r.ep, Some(r.rep[0])).expect("Recv");
    r.m.call(r.t[0], r.ep, 1).expect("Call");
    kiem(&r.m, "Call sau Bind");
    assert_eq!(
        r.m.sched_donated_to(r.sc[0]),
        Ok(Some(r.t[1])),
        "chỉ Call tạo donation"
    );
    r.m.sched_context_bind(r.sc[0], r.rep[0])
        .expect("Bind lại khi đang donate");
    kiem(&r.m, "Bind khi đang donate");
    assert_eq!(
        r.m.sched_donated_to(r.sc[0]),
        Ok(Some(r.t[1])),
        "Bind không tạo donation thứ hai/không đổi donee"
    );
    assert_eq!(r.m.sched_donor_of(r.sc[0]), Ok(Some(r.t[0])));
    // `Bind` sai loại object vẫn bị từ chối như trước.
    assert_eq!(
        r.m.sched_context_bind(r.sc[0], r.ts[0]),
        Err(KError::WrongObjectType)
    );
}

// ---------------------------------------------------------------------------
// 8. Model API: donate cần SC **đã Configure**; không chuỗi donation >1; SC đang donate
//    không bị `set_sched_params` lấy lại (D2-49(c), K1B-PLAN §5 KT-21 rủi ro (a)).
// ---------------------------------------------------------------------------

#[test]
fn donation_can_sc_configure_va_khong_chuoi_hai_tang() {
    // (a) Client có SC **chưa** `Configure` ⇒ không có donation: server passive vẫn không chạy.
    let mut r = rig();
    r.attach(0, 0); // SC0 chưa configure
    r.prio(0, 100, 255);
    r.prio(1, 10, 200);
    r.m.recv(r.t[1], r.ep, Some(r.rep[0])).expect("Recv");
    r.m.call(r.t[0], r.ep, 1).expect("Call");
    kiem(&r.m, "Call với SC chưa Configure");
    assert_eq!(r.m.sched_donated_to(r.sc[0]), Ok(None));
    assert!(!r.m.tcb_has_budget(r.t[1]));
    assert_eq!(r.m.sched_pick(), None);
    assert_eq!(r.dispatch(), None);

    // (b) Không chuỗi donation: server đang được donate gọi tiếp endpoint khác ⇒ không donate SC
    //     của client cho tầng thứ hai (K1b không làm chuỗi >1).
    let mut r = rig();
    r.configure(0, 4, 8);
    r.attach(0, 0);
    r.prio(0, 100, 255);
    r.prio(1, 10, 200);
    r.m.recv(r.t[1], r.ep, Some(r.rep[0])).expect("Recv");
    r.m.call(r.t[0], r.ep, 1).expect("Call");
    kiem(&r.m, "Call tầng 1");
    assert_eq!(r.m.donated_sc(r.t[1]), Ok(Some(r.sc_obj[0])));
    // t[2] (passive thứ hai) `Recv` trên cùng endpoint trước… nhưng endpoint đang có sender đã
    // giao; dùng endpoint thứ hai không có ⇒ `Call` của server vào hàng gửi, không có donee.
    r.m.call(r.t[1], r.ep, 2).expect("Call lồng từ server");
    kiem(&r.m, "Call lồng");
    assert_eq!(
        r.m.sched_donated_to(r.sc[0]),
        Ok(Some(r.t[1])),
        "SC vẫn chỉ donate cho tầng 1, không donate tiếp"
    );
    assert_eq!(r.m.donated_sc(r.t[2]), Ok(None));

    // (c) `set_sched_params` không được lấy SC đang donate / đổi SC của TCB đang nhận donation.
    assert_eq!(
        r.m.set_sched_params(r.ts[3], Some(r.sc[0])),
        Err(KError::Busy),
        "SC đang donate không gắn được cho TCB khác"
    );
    assert_eq!(
        r.m.set_sched_params(r.ts[1], None),
        Err(KError::IllegalOperation),
        "donee không được gỡ/đổi SC khi đang nhận donation"
    );
    assert_eq!(
        r.m.set_sched_params(r.ts[0], None),
        Err(KError::IllegalOperation),
        "donor không được gỡ SC khi đang donate"
    );
    kiem(&r.m, "set_sched_params bị từ chối");
    assert_eq!(r.m.sched_donated_to(r.sc[0]), Ok(Some(r.t[1])));
}

// ---------------------------------------------------------------------------
// 9. Invariant: 1.000 seed × 64 op (stable; Miri giảm seed/bước bằng `cfg(miri)`):
//    không TCB nào chạy mà không có SC cấp (own/donation); budget không âm/không vượt;
//    donation hai chiều khớp; SC trả đúng; cùng seed cùng trace (DoD 7).
// ---------------------------------------------------------------------------

#[cfg(miri)]
const N_SEED: u64 = 3;
#[cfg(not(miri))]
const N_SEED: u64 = 1000;
#[cfg(miri)]
const N_STEP: u64 = 8;
#[cfg(not(miri))]
const N_STEP: u64 = 64;

/// Chạy một kịch bản donation tất định; trả `(trace, số bước có ít nhất một SC đang được
/// donate)` — trace để so cùng seed cùng trace, bộ đếm để chứng minh kịch bản **thực sự** đi qua
/// đường donation (không phải test rỗng).
fn run_seed(seed: u64) -> (Vec<u64>, u64) {
    use std::collections::HashMap;

    let mut r = rig();
    let mut rng = Lcg::new(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ 0x21);
    // SC0 cho client t[0] (budget 3/period 6), SC1 cho server active t[2] (2/6),
    // SC2 cho client t[3] (2/4). Không reconfigure trong lúc chạy.
    let budget = [3u64, 2, 2];
    let period = [6u64, 6, 4];
    for i in 0..3 {
        r.configure(i, budget[i], period[i]);
    }
    r.attach(0, 0);
    r.attach(2, 1);
    r.attach(3, 2);
    r.prio(0, 60, 120);
    r.prio(1, 10, 90);
    r.prio(2, 30, 60);
    r.prio(3, 45, 200);
    for i in 0..4 {
        r.ready(i);
    }

    // (SC, cửa sổ thẳng hàng) → số tick đã charge — bất biến "mỗi cửa sổ ≤ budget" (KT-20).
    let mut cua_so: HashMap<(usize, u64), u64> = HashMap::new();
    let mut trace: Vec<u64> = Vec::new();
    let mut so_buoc_donate = 0u64;

    for _ in 0..N_STEP {
        let op = rng.below(100);
        match op {
            0..=7 => {
                let _ = r.m.call(r.t[0], r.ep, op);
            }
            8..=14 => {
                let _ = r.m.call(r.t[3], r.ep, op);
            }
            15..=23 => {
                let rep = if rng.below(2) == 0 {
                    r.rep[0]
                } else {
                    r.rep[1]
                };
                let _ = r.m.recv(r.t[1], r.ep, Some(rep));
            }
            24..=31 => {
                let rep = if rng.below(2) == 0 {
                    r.rep[0]
                } else {
                    r.rep[1]
                };
                let _ = r.m.recv(r.t[2], r.ep, Some(rep));
            }
            32..=41 => {
                let (tcb, rep) = if rng.below(2) == 0 {
                    (r.t[1], r.rep[0])
                } else {
                    (r.t[2], r.rep[1])
                };
                let _ = r.m.reply(tcb, rep);
            }
            42..=51 => {
                let _ = r.m.dispatch(0);
            }
            52..=59 => {
                // Tick từng đơn vị để quy charge về đúng cửa sổ thẳng hàng.
                let n = 1 + rng.below(3);
                for _ in 0..n {
                    let now = r.m.virtual_now();
                    let truoc: [u64; 3] = [r.info(0).charged, r.info(1).charged, r.info(2).charged];
                    r.m.tick(1).expect("tick");
                    for j in 0..3 {
                        let sau = r.info(j).charged;
                        let delta = sau - truoc[j];
                        assert!(delta <= 1, "một tick charge tối đa 1 (SC {j} +{delta})");
                        if delta == 1 {
                            let w = now / period[j];
                            let cu = *cua_so.get(&(j, w)).unwrap_or(&0);
                            assert!(
                                cu < budget[j],
                                "SC {j} cửa sổ {w}: {cu}+1 > budget {} (seed {seed})",
                                budget[j]
                            );
                            cua_so.insert((j, w), cu + 1);
                        }
                    }
                }
            }
            60..=67 => {
                let i = rng.below(4) as usize;
                let _ = r.m.tcb_suspend(r.ts[i], SuspendMode::Abort);
            }
            68..=74 => {
                let i = rng.below(4) as usize;
                let _ = r.m.tcb_resume(r.ts[i]);
            }
            75..=81 => {
                let i = rng.below(4) as usize;
                let _ = r.m.wake(r.t[i]);
            }
            82..=87 => {
                let _ = r.m.yield_current(0);
            }
            88..=93 => {
                let i = rng.below(4) as usize;
                let j = rng.below(3) as usize;
                if rng.below(2) == 0 {
                    let _ = r.m.set_sched_params(r.ts[i], Some(r.sc[j]));
                } else {
                    let _ = r.m.set_sched_params(r.ts[i], None);
                }
            }
            _ => {
                let i = rng.below(4) as usize;
                let p = rng.below(256) as u8;
                let mcp = p.saturating_add(rng.below(2) as u8);
                let _ = r.m.tcb_set_priority(r.ts[i], p, mcp);
            }
        }
        kiem(&r.m, "bước ngẫu nhiên (donation)");

        // (1) **Không TCB nào chạy mà không có SC cấp**: `Running ⇒ sc_of(..).is_some()` và còn
        //     budget (D2-50 + DoD 7).
        for i in 0..4 {
            let state = r.m.tcb_state(r.t[i]).expect("tcb_state");
            let cap_sc = r.m.sc_of(r.t[i]).expect("sc_of");
            if matches!(state, TcbState::Running { .. }) {
                assert!(
                    cap_sc.is_some(),
                    "TCB {i} Running mà không có SC cấp (seed {seed})"
                );
                assert!(
                    r.m.tcb_has_budget(r.t[i]),
                    "TCB {i} Running mà hết budget (seed {seed})"
                );
            }
            if cap_sc.is_none() {
                assert!(
                    !r.m.tcb_has_budget(r.t[i]),
                    "không SC cấp ⇒ không có budget (seed {seed})"
                );
            }
        }

        // (2) Donation **hai chiều** khớp; một SC một TCB; donor/donee khác nhau; donee không có
        //     SC riêng; donor đang `BlockedOnReply`.
        let mut donee: [Option<ObjId>; 3] = [None; 3];
        for (j, (donee_slot, sc_obj)) in donee.iter_mut().zip(r.sc_obj.iter()).enumerate() {
            let info = r.info(j);
            assert!(
                info.consumed <= info.budget,
                "SC {j}: consumed {} > budget {} (seed {seed})",
                info.consumed,
                info.budget
            );
            assert!(info.refill > r.m.virtual_now());
            match (info.donated_to, info.donor) {
                (Some(d), Some(o)) => {
                    assert_ne!(d, o, "donor == donee (seed {seed})");
                    assert_eq!(
                        r.m.donated_sc(d),
                        Ok(Some(*sc_obj)),
                        "SC {j} donate cho {d} nhưng donee không trỏ lại (seed {seed})"
                    );
                    assert_eq!(
                        r.m.sc_of(d),
                        Ok(Some(*sc_obj)),
                        "donee phải được cấp ngân sách bằng SC được donate"
                    );
                    let dstate = r.m.tcb_state(d).expect("donee state");
                    assert!(
                        !matches!(dstate, TcbState::Inactive | TcbState::Dead),
                        "donee {d} ở trạng thái {dstate:?} (seed {seed})"
                    );
                    assert_eq!(
                        r.m.tcb_state(o).expect("donor state").blocked_on_kind(),
                        Some(vifast_sim::kobj::BlockedOnKind::Reply),
                        "donor phải đang chờ Reply (seed {seed})"
                    );
                    assert!(
                        !matches!(r.m.tcb_state(d).expect("donee"), TcbState::Running { .. })
                            || !matches!(
                                r.m.tcb_state(o).expect("donor"),
                                TcbState::Running { .. }
                            ),
                        "donor và donee không cùng Running (seed {seed})"
                    );
                    *donee_slot = Some(d);
                }
                (None, None) => {}
                other => panic!("SC {j}: donation nửa vời {other:?} (seed {seed})"),
            }
        }
        // Mỗi donee chỉ nhận donation từ **một** SC.
        for (j, &dj) in donee.iter().enumerate() {
            if let Some(d) = dj {
                for (k, &dk) in donee.iter().enumerate() {
                    if k != j {
                        assert_ne!(dk, Some(d), "donee {d} nhận 2 SC (seed {seed})");
                    }
                }
            }
        }

        if (0..3).any(|j| r.info(j).donated_to.is_some()) {
            so_buoc_donate += 1;
        }
        // Trace tất định: (now, state/suspended/donated của 4 TCB, consumed/charged của 3 SC).
        trace.push(r.m.virtual_now());
        for i in 0..4 {
            let info = r.m.tcb_info(r.ts[i]).expect("TcbInfo");
            trace.push(info.state.index() as u64);
            trace.push(info.suspended as u64);
            trace.push(info.donated_sc.is_some() as u64);
            trace.push(r.m.effective_priority(r.t[i]).expect("effective") as u64);
        }
        for j in 0..3 {
            let info = r.info(j);
            trace.push(info.consumed);
            trace.push(info.charged);
            trace.push(info.donated_to.unwrap_or(0) as u64);
            trace.push(info.donor.unwrap_or(0) as u64);
        }
    }
    (trace, so_buoc_donate)
}

#[test]
fn invariant_1000_seed_64_op_donation() {
    // Cùng seed ⇒ cùng trace (tất định, không thời gian thực/HashMap trong đường chạy).
    for seed in 0..16 {
        assert_eq!(run_seed(seed), run_seed(seed), "seed {seed} phải tất định");
    }
    let mut tong_buoc_donate = 0u64;
    for seed in 0..N_SEED {
        let (_, d) = run_seed(seed);
        tong_buoc_donate += d;
    }
    assert!(
        tong_buoc_donate > 0,
        "kịch bản invariant phải đi qua đường donation ít nhất một lần"
    );
}
