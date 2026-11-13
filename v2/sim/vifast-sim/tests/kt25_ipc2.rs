// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-25 — **IPC core 2**: `Call/Reply/ReplyRecv/NBSendRecv/NBSendWait`, **4 cap**
//! (GRANT/bỏ GRANT/unwrap badge/receive window), **thất bại mềm** (D2-53 Q6), vòng đời
//! `Reply` bị xoá/revoke ⇒ caller **`Cancelled`** (D2-53 Q8), `NBSendRecv`/`NBSendWait`
//! theo **Q15**, donation không rò `SchedContext` (KT-21).
//!
//! **Model-only `[TK]`, không phải spec/ABI.** Nguồn ngữ nghĩa: `spec/abi.toml:31-49`
//! (`max_caps = 4`, `extra_caps_unwrapped_bits = 4`, `caps:20..22`), thiết kế §4.6
//! `:316-324` (4 cap/GRANT/unwrap/receive window/ReplyRecv), **D2-53 Q6** (thất bại mềm +
//! `cap_transfer_failed`, không thêm bit MessageInfo), **Q8** (Reply bị xoá ⇒ caller
//! `Cancelled`), **Q15** (`NBSendRecv` = `NBSend` rồi `Recv`; `NBSend` không giao được ⇒
//! **vẫn** `Recv`), K1C-PLAN §5 KT-25 DoD 1..10.
//!
//! Test đỏ trước (D2-05): [`JOURNAL/kt25-red-compile.log`](../../agent/JOURNAL/kt25-red-compile.log)
//! (E0599 cho `reply_recv`/`nb_send_recv`/`nb_send_wait`/`tcb_set_recv_window`/
//! `cap_transfer_failed`/`Message::with_cap_ids`) và
//! [`JOURNAL/kt25-red-semantics.log`](../../agent/JOURNAL/kt25-red-semantics.log) (probe
//! tạm: message có cap bị `NotSupported`; `Delete` Reply đang dùng để caller **treo** ở
//! `BlockedOnReply`).

use vifast_sim::ipc::{Message, MessageInfo, MAX_CAPS, MAX_WORDS};
use vifast_sim::kobj::{
    CapId, KError, Machine, ObjId, ObjType, SlotRef, SuspendMode, TcbState, RIGHTS_ALL,
    RIGHT_GRANT, RIGHT_READ, RIGHT_WRITE,
};
use vifast_sim::scenario::{run_sequential, Msg, ScenarioStep, Step};
use vifast_sim::Lcg;

fn slot(cnode: ObjId, s: usize) -> SlotRef {
    SlotRef::new(cnode, s)
}

/// `Message` **không** cap; `words.len()` phải bằng `info.length()`.
fn msg(label: u32, words: &[u64]) -> Message {
    let info = MessageInfo::new(label, 0, false, 0, words.len() as u8).expect("MessageInfo hợp lệ");
    Message::new(info, words).expect("Message hợp lệ")
}

/// `Message` mang `caps` (≤ 4); 5 cap ⇒ `InvalidArgument` **trước** khi giao (`.scn` 15).
fn msg_caps(label: u32, words: &[u64], caps: &[CapId]) -> Message {
    Message::with_cap_ids(label, words, caps).expect("Message có cap hợp lệ")
}

fn msg_empty() -> Message {
    msg(0, &[])
}

// ---------------------------------------------------------------------------
// Rig chung: CNode 128 slot; 2 Endpoint; 2 Reply; 1 Notification; n TCB (SC + priority
// riêng, test-only D2-50); **receive window** 4 slot cho từng TCB; 4 cap nguồn có GRANT
// (Endpoints khác endpoint nhận ⇒ không unwrap); 1 cap thiếu GRANT; 1 cap badge tới `ep0`
// (unwrap).
// ---------------------------------------------------------------------------

const S_EP: [usize; 2] = [0, 1];
const S_REP: [usize; 2] = [2, 3];
const S_NTFN: usize = 4;
const S_SRC: [usize; 4] = [30, 31, 32, 33];
const S_NOGRANT: usize = 34;
const S_BADGE: usize = 35;
const S_TCB: usize = 10;
const S_SC: usize = 70;
const S_CTL: usize = 120;
/// Ô receive window (để **trống** trong CNode; chỉ bind làm CPTR).
const S_WIN: [[usize; 4]; 3] = [[90, 91, 92, 93], [94, 95, 96, 97], [98, 99, 100, 101]];

struct Rig {
    m: Machine,
    c0: ObjId,
    ep: [ObjId; 2],
    ep_s: [SlotRef; 2],
    rep: [ObjId; 2],
    rep_s: [SlotRef; 2],
    ntfn_s: SlotRef,
    src_s: [SlotRef; 4],
    src_caps: [CapId; 4],
    nogrant_s: SlotRef,
    badge_s: SlotRef,
    badge_cap: CapId,
    win: [[SlotRef; 4]; 3],
    t: Vec<ObjId>,
    t_s: Vec<SlotRef>,
}

impl Rig {
    fn new(n_tcb: usize) -> Self {
        assert!((2..=3).contains(&n_tcb), "rig dựng sẵn 3 receive window");
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
        let mut rep = [0u32; 2];
        let mut rep_s = [slot(c0, 2); 2];
        for (i, s) in S_REP.iter().enumerate() {
            rep[i] = m.add_kernel_object(ObjType::Reply);
            let cap = m.root_cap(rep[i]);
            m.put_cap(c0, *s, cap).expect("cap Reply");
            rep_s[i] = slot(c0, *s);
        }
        let ntfn = m.add_kernel_object(ObjType::Notification);
        let ntfn_cap = m.root_cap(ntfn);
        m.put_cap(c0, S_NTFN, ntfn_cap).expect("cap Notification");
        let ntfn_s = slot(c0, S_NTFN);

        // 4 cap nguồn: hai Endpoint **khác** endpoint nhận (`ep[0]`/`ep[1]`) ⇒ cap thường.
        let src_obj = [
            m.add_kernel_object(ObjType::Endpoint),
            m.add_kernel_object(ObjType::Endpoint),
        ];
        let mut src_s = [slot(c0, 30); 4];
        let mut src_caps = [0u32; 4];
        for (i, s) in S_SRC.iter().enumerate() {
            src_caps[i] = m.root_cap(src_obj[i % 2]);
            m.put_cap(c0, *s, src_caps[i]).expect("cap nguồn");
            src_s[i] = slot(c0, *s);
        }
        // Bản sao **thiếu `RIGHT_GRANT`**: chuyển được mềm-thất-bại (D2-53 Q6).
        m.copy_rights(
            src_s[0],
            slot(c0, S_NOGRANT),
            Some(RIGHT_READ | RIGHT_WRITE),
        )
        .expect("copy_rights không GRANT");
        let nogrant_s = slot(c0, S_NOGRANT);
        // Cap badge tới **chính `ep[0]`** ⇒ unwrap khi gửi tới `ep0` (D2-53 Q6, thiết kế `:319`).
        let badge_cap = m
            .mint(ep_s[0], slot(c0, S_BADGE), RIGHTS_ALL, Some(0xA5A5))
            .expect("mint badge trên ep0");
        let badge_s = slot(c0, S_BADGE);

        let mut t = Vec::with_capacity(n_tcb);
        let mut t_s = Vec::with_capacity(n_tcb);
        let mut win = [[slot(c0, 90); 4]; 3];
        for i in 0..n_tcb {
            let obj = m.add_kernel_object(ObjType::Tcb);
            let cap = m.root_cap(obj);
            m.put_cap(c0, S_TCB + i, cap).expect("cap TCB");
            t.push(obj);
            t_s.push(slot(c0, S_TCB + i));

            let sc = m.add_kernel_object(ObjType::SchedContext);
            let sc_cap = m.root_cap(sc);
            m.put_cap(c0, S_SC + i, sc_cap).expect("cap SchedContext");
            let sc_slot = slot(c0, S_SC + i);
            m.sched_control_configure(ctl_s, sc_slot, 4, 8)
                .expect("SchedControl.Configure");
            m.set_sched_params(t_s[i], Some(sc_slot))
                .expect("set_sched_params (test-only)");
            m.tcb_set_priority(t_s[i], 5, 255).expect("Tcb.SetPriority");
            win[i] = [
                slot(c0, S_WIN[i][0]),
                slot(c0, S_WIN[i][1]),
                slot(c0, S_WIN[i][2]),
                slot(c0, S_WIN[i][3]),
            ];
        }
        let mut r = Self {
            m,
            c0,
            ep,
            ep_s,
            rep,
            rep_s,
            ntfn_s,
            src_s,
            src_caps,
            nogrant_s,
            badge_s,
            badge_cap,
            win,
            t,
            t_s,
        };
        for i in 0..r.t.len() {
            r.m.transition_tcb(r.t[i], TcbState::Ready)
                .expect("Inactive→Ready");
            let w = r.win[i];
            r.m.tcb_set_recv_window(r.t_s[i], w)
                .expect("bind receive window");
        }
        r
    }

    fn kiem(&self, buoc: &str) {
        if let Err(e) = self.m.check_sched() {
            panic!("check_sched sau {buoc}: {e}");
        }
        if let Err(e) = self.m.check_mailboxes() {
            panic!("check_mailboxes sau {buoc}: {e}");
        }
        if let Err(e) = self.m.check_ipc() {
            panic!("check_ipc sau {buoc}: {e}");
        }
        if let Err(e) = self.m.check_caps() {
            panic!("check_caps sau {buoc}: {e}");
        }
        // **KT-34** — bất biến liên kết Reply (C2) sau **mỗi** bước của cả test định hướng lẫn
        // fuzz 1.000 seed.
        if let Err(e) = self.m.check_reply_links() {
            panic!("check_reply_links sau {buoc}: {e}");
        }
    }

    fn st(&self, i: usize) -> TcbState {
        self.m.tcb_state(self.t[i]).expect("tcb_state")
    }

    fn call(&mut self, i: usize, epi: usize, m: Message) -> Result<(), KError> {
        self.m.call_msg(self.t[i], self.ep_s[epi], m)
    }

    fn send(&mut self, i: usize, epi: usize, m: Message) -> Result<(), KError> {
        self.m.send(self.t[i], self.ep_s[epi], m)
    }

    fn nb_send(&mut self, i: usize, epi: usize, m: Message) -> Result<(), KError> {
        self.m.nb_send(self.t[i], self.ep_s[epi], m)
    }

    fn recv(&mut self, i: usize, epi: usize, reply: Option<usize>) -> Result<(), KError> {
        let rep = reply.map(|k| self.rep_s[k]);
        self.m.recv(self.t[i], self.ep_s[epi], rep)
    }

    fn reply_recv(
        &mut self,
        i: usize,
        reply: usize,
        epi: usize,
        m: Message,
        next: Option<usize>,
    ) -> Result<(), KError> {
        let next = next.map(|k| self.rep_s[k]);
        self.m
            .reply_recv(self.t[i], self.rep_s[reply], self.ep_s[epi], m, next)
    }

    fn nb_send_recv(
        &mut self,
        i: usize,
        epi: usize,
        m: Message,
        recvi: usize,
        next: Option<usize>,
    ) -> Result<Option<KError>, KError> {
        let next = next.map(|k| self.rep_s[k]);
        self.m
            .nb_send_recv(self.t[i], self.ep_s[epi], m, self.ep_s[recvi], next)
    }

    fn nb_send_wait(&mut self, i: usize, epi: usize, m: Message) -> Result<Option<KError>, KError> {
        self.m
            .nb_send_wait(self.t[i], self.ep_s[epi], m, self.ntfn_s)
    }

    fn win_caps(&self, i: usize) -> [Option<CapInfoRow>; MAX_CAPS] {
        let mut out = [None; MAX_CAPS];
        for (k, s) in self.win[i].iter().enumerate() {
            if let Ok(cap) = self.m.cap_at(s.cnode, s.slot) {
                let info = self.m.cap_info(cap).expect("cap_info");
                out[k] = Some(CapInfoRow {
                    ty: info.ty,
                    rights: info.rights,
                    badge: info.badge,
                });
            }
        }
        out
    }
}

/// Bản rút gọn của `CapInfo` để so trong test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CapInfoRow {
    ty: ObjType,
    rights: u8,
    badge: Option<u64>,
}

// ===========================================================================
// 1. 4 cap: cài đủ, **bỏ `RIGHT_GRANT`**, badge giữ nguyên (DoD 3).
// ===========================================================================

#[test]
fn bon_cap_cai_du_bo_grant_giu_badge() {
    let mut r = Rig::new(2);
    r.recv(1, 0, Some(0)).expect("server Recv + Reply cap");
    let caps = r.src_caps;
    r.send(0, 0, msg_caps(7, &[1, 2], &caps))
        .expect("Send 4 cap");
    r.kiem("4 cap");

    assert_eq!(r.st(1), TcbState::Ready, "message giao thẳng cho waiter");
    let info = r.m.tcb_ipc_info(r.t[1]).expect("ipc").expect("đã nhận");
    assert_eq!(info.caps(), 4, "MessageInfo.caps = số cap cài được");
    assert_eq!(info.extra_caps_unwrapped(), 0);
    assert_eq!(info.length(), 2, "MR vẫn nguyên");
    let w = r.win_caps(1);
    for (k, c) in w.iter().enumerate() {
        let c = c.unwrap_or_else(|| panic!("window slot {k} phải có cap"));
        assert_eq!(c.ty, ObjType::Endpoint);
        assert_eq!(
            c.rights & RIGHT_GRANT,
            0,
            "cap nhận được **bỏ** RIGHT_GRANT (thiết kế :319)"
        );
        assert_eq!(c.rights, RIGHTS_ALL & !RIGHT_GRANT);
    }
    let (installed, unwrapped, failed) = r.m.cap_transfer_stats();
    assert_eq!((installed, unwrapped, failed), (4, 0, 0));
    r.kiem("4 cap sau");
}

#[test]
fn cap_da_nhan_khong_chuyen_lai_duoc_vi_mat_grant() {
    let mut r = Rig::new(2);
    r.recv(1, 0, None).expect("server Recv");
    r.send(0, 0, msg_caps(70, &[], &r.src_caps))
        .expect("chuyển 4 cap cho server");
    let nhan =
        r.m.cap_at(r.win[1][0].cnode, r.win[1][0].slot)
            .expect("cap đã nhận");
    assert_eq!(
        r.m.cap_info(nhan).expect("info").rights & RIGHT_GRANT,
        0,
        "cap nhận đã mất RIGHT_GRANT"
    );
    // Server chuyển lại **chính cap đã nhận** cho client: thiếu GRANT ⇒ thất bại **mềm**,
    // message vẫn giao, không có cap nào cài (D2-53 Q6 + "bỏ GRANT đúng").
    r.recv(0, 0, None).expect("client Recv");
    r.send(1, 0, msg_caps(71, &[5], &[nhan]))
        .expect("thất bại mềm ⇒ không lỗi");
    let info = r.m.tcb_ipc_info(r.t[0]).expect("ipc").expect("đã nhận");
    assert_eq!(info.caps(), 0, "không cap nào cài được");
    assert_eq!(r.m.cap_transfer_failed(), 1);
    assert_eq!(r.m.tcb_ipc_regs(r.t[0]).expect("regs")[0], 5, "MR vẫn giao");
    assert!(r.win_caps(0).iter().all(|c| c.is_none()));
    r.kiem("chuyển lại cap mất GRANT");
}

// ===========================================================================
// 2. Thất bại **mềm** (D2-53 Q6): thiếu GRANT / ô đích đã có cap ⇒ message **vẫn giao**,
//    `caps` = số cài được, `cap_transfer_failed` tăng, **không** lỗi cứng (DoD 4).
// ===========================================================================

#[test]
fn thieu_grant_that_bai_mem_message_van_giao() {
    let mut r = Rig::new(2);
    r.recv(1, 0, None).expect("server Recv");
    // cap 1 thiếu GRANT ⇒ không cài, ba cap còn lại vẫn cài.
    let caps = [
        r.src_caps[0],
        r.nogrant_s_cap(),
        r.src_caps[2],
        r.src_caps[3],
    ];
    r.send(0, 0, msg_caps(8, &[9], &caps))
        .expect("thất bại mềm ⇒ Send **không** lỗi");
    r.kiem("thiếu GRANT");
    let info = r.m.tcb_ipc_info(r.t[1]).expect("ipc").expect("đã nhận");
    assert_eq!(info.caps(), 3, "caps = số cap cài được");
    assert_eq!(info.extra_caps_unwrapped(), 0);
    assert_eq!(r.m.cap_transfer_failed(), 1);
    assert_eq!(r.m.cap_transfer_installed(), 3);
    // Ô số 1 của window **trống** (cap thiếu GRANT không cài), các ô khác có cap.
    let w = r.win_caps(1);
    assert!(w[0].is_some());
    assert!(w[1].is_none(), "cap thiếu GRANT không được cài");
    assert!(w[2].is_some());
    assert!(w[3].is_some());
    assert_eq!(r.m.ipc_delivered(), 1, "message vẫn giao");
    r.kiem("thiếu GRANT sau");
}

#[test]
fn o_dich_da_co_cap_that_bai_mem() {
    let mut r = Rig::new(2);
    // Chiếm sẵn ô 2 của window bằng một cap khác (không qua IPC).
    let spare = r.m.root_cap(r.ep[1]);
    r.m.put_cap(r.c0, S_WIN[1][1], spare)
        .expect("chiếm ô window");
    r.recv(1, 0, None).expect("server Recv");
    r.send(0, 0, msg_caps(9, &[], &r.src_caps))
        .expect("thất bại mềm ⇒ không lỗi");
    r.kiem("ô đích đã có cap");
    let info = r.m.tcb_ipc_info(r.t[1]).expect("ipc").expect("đã nhận");
    assert_eq!(info.caps(), 3, "một cap không cài được (ô đích đã có)");
    assert_eq!(r.m.cap_transfer_failed(), 1);
    // Cài **theo vị trí**: cap thứ 2 ứng ô 2 — ô đã có cap ⇒ cap đó không cài; các cap khác
    // vẫn cài đúng ô của chúng (một ô bận **không** chặn các ô sau).
    let w = r.win_caps(1);
    assert!(w[0].is_some() && w[1].is_some() && w[2].is_some() && w[3].is_some());
    assert_eq!(w[1].expect("ô bị chiếm").badge, None);
    assert_eq!(r.st(1), TcbState::Ready);
    r.kiem("ô đích sau");
}

impl Rig {
    /// Cap nguồn **thiếu** `RIGHT_GRANT` (đã tạo trong `new`).
    fn nogrant_s_cap(&self) -> CapId {
        self.m
            .cap_at(self.nogrant_s.cnode, self.nogrant_s.slot)
            .expect("cap thiếu GRANT")
    }
}

// ===========================================================================
// 3. Lỗi **cứng**: cap nguồn chết/không tồn tại ⇒ `InvalidCapability` **trước** khi giao;
//    5 cap ⇒ `InvalidArgument` **trước** khi giao (DoD 3/4).
// ===========================================================================

#[test]
fn cap_nguon_chet_loi_cung_truoc_khi_giao() {
    let mut r = Rig::new(2);
    let chet = r.m.copy(r.src_s[0], slot(r.c0, 60)).expect("copy");
    r.m.delete(slot(r.c0, 60)).expect("delete cap nguồn");
    r.recv(1, 0, None).expect("server Recv");
    assert_eq!(
        r.send(0, 0, msg_caps(10, &[1], &[chet])),
        Err(KError::InvalidCapability),
        "cap nguồn chết ⇒ lỗi cứng"
    );
    assert_eq!(r.m.ipc_sent(), 0, "message **không** vào đường IPC");
    assert_eq!(r.m.ipc_pending(), 0);
    assert_eq!(r.st(0), TcbState::Ready, "không đổi trạng thái");
    assert_eq!(
        r.st(1),
        TcbState::BlockedOnRecv {
            ep: r.ep[0],
            reply: None
        },
        "waiter vẫn chờ, không nhận rác"
    );
    r.kiem("cap chết");
}

#[test]
fn nam_cap_bi_tu_choi_truoc_khi_giao() {
    // `spec/abi.toml` `max_caps = 4`: 5 cap không biểu diễn được ⇒ `InvalidArgument` **trước**
    // khi message vào đường IPC (`.scn` 15).
    let r = Rig::new(2);
    let nam = [
        r.src_caps[0],
        r.src_caps[1],
        r.src_caps[2],
        r.src_caps[3],
        r.badge_cap,
    ];
    assert_eq!(
        Message::with_cap_ids(11, &[], &nam),
        Err(KError::InvalidArgument)
    );
    // 4 cap vẫn hợp lệ.
    assert!(Message::with_cap_ids(11, &[], &nam[..4]).is_ok());
    r.kiem("5 cap");
}

// ===========================================================================
// 4. Unwrap badge: cap tới **chính endpoint** đang gọi + badge ⇒ không cài, badge OR vào
//    message, `extra_caps_unwrapped` tăng, receive window **không** bị chiếm (DoD 5).
// ===========================================================================

#[test]
fn unwrap_badge_khong_chiem_receive_window() {
    let mut r = Rig::new(2);
    // Cap badge nằm đúng slot 35 (nguồn của cap unwrap) — cap còn sống, rights đầy đủ.
    assert_eq!(r.m.cap_at(r.badge_s.cnode, r.badge_s.slot), Ok(r.badge_cap));
    r.recv(1, 0, None).expect("server Recv");
    // [cap badge tới ep0, cap thường tới ep khác] ⇒ 1 unwrap + 1 cài.
    let caps = [r.badge_cap, r.src_caps[1]];
    r.send(0, 0, msg_caps(12, &[0xAB], &caps))
        .expect("Send unwrap");
    r.kiem("unwrap");
    let info = r.m.tcb_ipc_info(r.t[1]).expect("ipc").expect("đã nhận");
    assert_eq!(info.caps(), 1, "chỉ cap thường được cài");
    assert_eq!(info.extra_caps_unwrapped(), 1, "một cap unwrap");
    assert_eq!(r.m.tcb_ipc_badge(r.t[1]).expect("badge"), 0xA5A5);
    assert_eq!(r.m.cap_unwrapped(), 1);
    assert_eq!(r.m.cap_transfer_installed(), 1);
    // Cài **theo vị trí**: cap unwrap ở index 0 **không** chiếm ô 0; cap thường ở index 1
    // cài vào ô 1 ⇒ ô 0 vẫn trống (window không bị chiếm bởi cap unwrap).
    let w = r.win_caps(1);
    assert!(w[1].is_some(), "cap thường cài vào ô index 1");
    assert!(
        w[0].is_none(),
        "cap unwrap KHÔNG chiếm receive window (thiết kế :319)"
    );
    // `badge` reset mỗi message: message sau không có cap ⇒ badge 0.
    r.recv(1, 0, None).expect("Recv lại");
    r.send(0, 0, msg(13, &[1])).expect("Send không cap");
    assert_eq!(r.m.tcb_ipc_badge(r.t[1]).expect("badge"), 0);
    r.kiem("unwrap sau");
}

#[test]
fn hai_badge_or_vao_message_badge() {
    let mut r = Rig::new(2);
    let badge_fn =
        r.m.mint(r.ep_s[0], slot(r.c0, 36), RIGHTS_ALL, Some(0x0F00));
    let badge2 = badge_fn.expect("mint badge 2");
    r.recv(1, 0, None).expect("server Recv");
    r.send(0, 0, msg_caps(14, &[], &[r.badge_cap, badge2]))
        .expect("Send 2 unwrap");
    r.kiem("2 unwrap");
    let info = r.m.tcb_ipc_info(r.t[1]).expect("ipc").expect("đã nhận");
    assert_eq!(info.caps(), 0);
    assert_eq!(info.extra_caps_unwrapped(), 2);
    assert_eq!(
        r.m.tcb_ipc_badge(r.t[1]).expect("badge"),
        0xA5A5 | 0x0F00,
        "Message.badge = OR các badge (model-only)"
    );
}

// ===========================================================================
// 5. Receive window **chưa bind** ⇒ `InvalidArgument` (khác "bind rỗng"), message không mất.
// ===========================================================================

#[test]
fn receive_window_chua_bind_tra_invalid_argument() {
    let mut r = Rig::new(2);
    // TCB 2 là TCB mới **chưa** bind window.
    let t2 = r.m.add_kernel_object(ObjType::Tcb);
    let t2_cap = r.m.root_cap(t2);
    r.m.put_cap(r.c0, 50, t2_cap).expect("cap TCB 2");
    r.m.transition_tcb(t2, TcbState::Ready).expect("Ready");
    let t2_s = slot(r.c0, 50);
    r.m.recv(t2, r.ep_s[1], None).expect("TCB 2 Recv");
    assert_eq!(
        r.m.send(r.t[0], r.ep_s[1], msg_caps(15, &[], &r.src_caps)),
        Err(KError::InvalidArgument),
        "window chưa bind ⇒ lỗi cấu hình rõ ràng"
    );
    assert_eq!(r.m.ipc_sent(), 0, "message không vào đường IPC");
    assert_eq!(t2_s, slot(r.c0, 50));
    r.kiem("window chưa bind");
    // Message **chỉ** mang cap unwrap (badge tới chính endpoint đang gửi) ⇒ không cần window:
    // không có cap nào cần cài, badge OR vào message (DoD 5).
    let badge_ep1 =
        r.m.mint(r.ep_s[1], slot(r.c0, 51), RIGHTS_ALL, Some(0x33))
            .expect("mint badge ep1");
    r.m.send(r.t[0], r.ep_s[1], msg_caps(16, &[], &[badge_ep1]))
        .expect("unwrap-only không cần window");
    assert_eq!(r.m.ipc_sent(), 1);
    let info = r.m.tcb_ipc_info(t2).expect("ipc").expect("đã nhận");
    assert_eq!(info.caps(), 0);
    assert_eq!(info.extra_caps_unwrapped(), 1);
    assert_eq!(r.m.tcb_ipc_badge(t2).expect("badge"), 0x33);
    r.kiem("unwrap-only không window");
    // Bind window 4 ô trống ⇒ message có cap cài được bình thường.
    let w = [
        slot(r.c0, 60),
        slot(r.c0, 61),
        slot(r.c0, 62),
        slot(r.c0, 63),
    ];
    r.m.tcb_set_recv_window(t2_s, w).expect("bind window");
    assert_eq!(
        r.m.tcb_recv_window(t2).expect("window"),
        Some(w),
        "đọc lại receive window"
    );
    r.m.recv(t2, r.ep_s[1], None).expect("TCB 2 Recv lại");
    r.m.send(r.t[0], r.ep_s[1], msg_caps(17, &[], &r.src_caps))
        .expect("sau khi bind ⇒ giao được");
    assert_eq!(
        r.m.tcb_ipc_info(t2).expect("ipc").expect("đã nhận").caps(),
        4
    );
    r.kiem("window bind");
}

// ===========================================================================
// 6. `ReplyRecv`: giao reply **trước**, rồi `recv`; không message ⇒ `BlockedOnRecv` mang
//    Reply **mới**; không mất reply/message (DoD 1).
// ===========================================================================

#[test]
fn reply_recv_giao_reply_truoc_roi_recv() {
    let mut r = Rig::new(2);
    r.recv(1, 0, Some(0)).expect("server Recv + rep0");
    r.call(0, 0, msg(20, &[0xC1])).expect("client Call");
    assert_eq!(
        r.st(0),
        TcbState::BlockedOnReply { reply: r.rep[0] },
        "caller BlockedOnReply"
    );
    assert_eq!(r.st(1), TcbState::Ready, "server nhận message");

    // Server trả lời rồi Recv trên ep1, mang Reply **mới** `rep1`.
    r.reply_recv(1, 0, 1, msg(21, &[0xD2]), Some(1))
        .expect("ReplyRecv");
    r.kiem("reply_recv");
    assert_eq!(r.st(0), TcbState::Ready, "caller được đánh thức");
    assert_eq!(r.m.tcb_last_ipc_error(r.t[0]).expect("err"), None);
    let got = r.m.tcb_ipc_regs(r.t[0]).expect("regs");
    assert_eq!(got[0], 0xD2, "caller nhận message của Reply");
    assert_eq!(
        r.st(1),
        TcbState::BlockedOnRecv {
            ep: r.ep[1],
            reply: Some(r.rep[1])
        },
        "server chờ message kế với Reply mới"
    );
    assert_eq!(
        r.m.endpoint_recv_waiter(r.ep[1]).expect("waiter"),
        Some(r.t[1])
    );
    // Reply cũ đã dùng ⇒ không còn caller; reply mới chưa có caller.
    assert_eq!(r.m.reply_caller(r.rep[0]).expect("rep0"), None);
    assert_eq!(r.m.reply_caller(r.rep[1]).expect("rep1"), None);
    // `nb_recv` trên ep1 ⇒ Busy (không có message).
    assert_eq!(r.m.nb_recv(r.t[1], r.ep_s[1]), Err(KError::Busy));
    r.kiem("reply_recv sau");
}

#[test]
fn reply_recv_khong_co_message_thi_blocked_on_recv() {
    let mut r = Rig::new(2);
    // Server chưa Recv: ReplyRecv phải trả lời (không có caller ⇒ reply bị bỏ) rồi Recv.
    r.reply_recv(1, 0, 1, msg_empty(), Some(0))
        .expect("ReplyRecv không caller");
    r.kiem("reply_recv không caller");
    assert_eq!(
        r.st(1),
        TcbState::BlockedOnRecv {
            ep: r.ep[1],
            reply: Some(r.rep[0])
        }
    );
    // Message rỗng của reply bị bỏ ⇒ đếm `ipc_dropped` (không im lặng "mất").
    assert_eq!(r.m.ipc_sent(), 1);
    assert_eq!(r.m.ipc_dropped(), 1);
    // Client gọi vào ep1 ⇒ server nhận, client BlockedOnReply{rep1}.
    r.call(0, 1, msg(22, &[7])).expect("client Call sau");
    assert_eq!(r.st(1), TcbState::Ready);
    assert_eq!(r.m.tcb_ipc_regs(r.t[1]).expect("regs")[0], 7);
    r.kiem("reply_recv không caller sau");
}

// ===========================================================================
// 7. `NBSendRecv` / `NBSendWait` (D2-53 Q15): NBSend **không chặn** rồi Recv/Wait;
//    NBSend **không giao được** ⇒ **vẫn** Recv/Wait, message không bị kernel coi là mất.
// ===========================================================================

#[test]
fn nb_send_recv_nhanh_gui_duoc_va_nhanh_khong_gui_duoc() {
    let mut r = Rig::new(3);
    // Nhánh A: người nhận đang chờ ⇒ NBSend giao ngay; Recv chưa có message ⇒ chặn.
    r.recv(1, 0, Some(0)).expect("server Recv chờ");
    assert_eq!(
        r.nb_send_recv(2, 0, msg(30, &[0x31]), 1, Some(1)),
        Ok(None),
        "NBSend giao được, Recv chặn"
    );
    r.kiem("NB* nhánh A");
    assert_eq!(r.st(1), TcbState::Ready, "server nhận được message");
    assert_eq!(r.m.tcb_ipc_regs(r.t[1]).expect("regs")[0], 0x31);
    assert_eq!(
        r.st(2),
        TcbState::BlockedOnRecv {
            ep: r.ep[1],
            reply: Some(r.rep[1])
        },
        "Recv của NBSendRecv chặn với Reply mới"
    );
    assert_eq!(r.m.ipc_sent(), 1);
    assert_eq!(r.m.ipc_delivered(), 1);
    // Server gọi lại vào ep1 ⇒ TCB 2 nhận và server BlockedOnReply{rep1}.
    r.call(1, 1, msg(31, &[0x41])).expect("server Call ep1");
    assert_eq!(r.st(2), TcbState::Ready);
    assert_eq!(r.m.tcb_ipc_regs(r.t[2]).expect("regs")[0], 0x41);
    r.m.reply(r.t[2], r.rep_s[1]).expect("server t2 Reply");
    r.kiem("NB* nhánh A sau");

    // Nhánh B: hàng gửi đầy ⇒ NBSend `Busy` nhưng **vẫn** Recv (chặn);
    // message **không** bị kernel nhận ⇒ `ipc_sent` không đổi (Q15a).
    let mut r2 = Rig::new(3);
    for i in 0..8u64 {
        r2.nb_send(2, 0, msg(40 + i as u32, &[i]))
            .expect("làm đầy hàng");
    }
    assert_eq!(
        r2.nb_send_recv(2, 0, msg(50, &[0x51]), 1, None),
        Ok(Some(KError::Busy))
    );
    r2.kiem("NB* nhánh B");
    assert_eq!(
        r2.m.ipc_sent(),
        8,
        "NBSend lỗi ⇒ message **không** vào đường IPC"
    );
    assert_eq!(
        r2.st(2),
        TcbState::BlockedOnRecv {
            ep: r2.ep[1],
            reply: None
        },
        "NBSend lỗi ⇒ **vẫn** Recv (Q15a)"
    );
    // Message vẫn của bên gọi: pop 1 chỗ rồi gửi lại được (từ TCB khác đang Ready — TCB 2
    // đang `BlockedOnRecv`) ⇒ message **không** bị coi là mất.
    r2.m.nb_recv(r2.t[1], r2.ep_s[0]).expect("pop 1");
    r2.nb_send(0, 0, msg(50, &[0x51])).expect("gửi lại được");
    assert_eq!(r2.m.ipc_sent(), 9);
    r2.kiem("NB* nhánh B sau");
}

#[test]
fn nb_send_wait_hai_nhanh() {
    let mut r = Rig::new(3);
    // Nhánh A: server Recv chờ; NBSend giao; sau đó Wait trên Notification ⇒ chặn.
    r.recv(1, 0, Some(0)).expect("server Recv chờ");
    assert_eq!(r.nb_send_wait(2, 0, msg(60, &[0x61])), Ok(None));
    r.kiem("NBSendWait A");
    assert_eq!(r.st(1), TcbState::Ready);
    assert_eq!(
        r.st(2),
        TcbState::BlockedOnNotification { ntfn: r.ntfn_obj() }
    );
    r.kiem("NBSendWait A sau");

    // Nhánh B: hàng đầy ⇒ NBSend `Busy` nhưng **vẫn** Wait (chặn).
    let mut r2 = Rig::new(3);
    for i in 0..8u64 {
        r2.nb_send(2, 0, msg(70 + i as u32, &[i]))
            .expect("đầy hàng");
    }
    assert_eq!(
        r2.nb_send_wait(2, 0, msg(80, &[0x81])),
        Ok(Some(KError::Busy))
    );
    r2.kiem("NBSendWait B");
    assert_eq!(r2.m.ipc_sent(), 8, "NBSend lỗi ⇒ không vào IPC");
    assert_eq!(
        r2.m.notification_waiter(r2.ntfn_obj()).expect("waiter"),
        Some(r2.t[2])
    );
    assert!(matches!(r2.st(2), TcbState::BlockedOnNotification { .. }));
}

impl Rig {
    fn ntfn_obj(&self) -> ObjId {
        self.m
            .cap_obj_at(self.ntfn_s.cnode, self.ntfn_s.slot)
            .expect("ntfn obj")
    }
}

#[test]
fn reply_loi_cung_khong_lam_mat_caller() {
    let mut r = Rig::new(2);
    r.recv(1, 0, Some(0)).expect("server Recv");
    r.call(0, 0, msg(80, &[1])).expect("client Call");
    // Cap nguồn chết trong message của Reply ⇒ lỗi cứng, **trước** khi lấy `caller`
    // (nếu lấy trước, `Reply.caller = None` mà caller vẫn chờ ⇒ treo).
    let chet = r.m.copy(r.src_s[0], slot(r.c0, 60)).expect("copy");
    r.m.delete(slot(r.c0, 60)).expect("delete");
    assert_eq!(
        r.m.reply_msg(r.t[1], r.rep_s[0], msg_caps(81, &[], &[chet])),
        Err(KError::InvalidCapability)
    );
    r.kiem("reply lỗi cứng");
    assert_eq!(
        r.m.reply_caller(r.rep[0]).expect("rep0"),
        Some(r.t[0]),
        "caller vẫn bind (không mất binding)"
    );
    assert_eq!(r.st(0), TcbState::BlockedOnReply { reply: r.rep[0] });
    // Reply lại (không cap) ⇒ trả lời bình thường, caller `Ready` + nhận message.
    r.m.reply_msg(r.t[1], r.rep_s[0], msg(82, &[9]))
        .expect("reply lại");
    assert_eq!(r.st(0), TcbState::Ready);
    assert_eq!(r.m.tcb_ipc_regs(r.t[0]).expect("regs")[0], 9);
    r.kiem("reply lỗi cứng sau");
}

// ===========================================================================
// 8. Vòng đời Reply (D2-53 Q8): `Delete`/`Revoke` khi caller `BlockedOnReply` ⇒ caller
//    **`Cancelled`** (không deadlock); `Reply.caller == None`; `Send(rep0)` ⇒
//    `InvalidCapability` (`.scn` 07); `.scn` 08 ⇒ `Delete` sau `Revoke` = `SlotEmpty` (DoD 6).
// ===========================================================================

#[test]
fn delete_reply_dang_trong_recv_waiter_khong_lam_hong_call() {
    let mut r = Rig::new(2);
    // `.scn` 07: server Recv (D2-54 cấp `rep0` làm Reply window) rồi `CNode.Delete` cap `rep0`.
    r.recv(1, 0, Some(0)).expect("server Recv");
    r.m.delete(r.rep_s[0])
        .expect("Delete Reply cap đang trong waiter");
    r.kiem("delete reply trong waiter");
    // `Call` sau đó **không** được trả cứng `InvalidCapability`: Reply cap chết trong waiter
    // phải được coi như "không cấp Reply" ⇒ message **vẫn giao**, caller `Cancelled` (Q7b).
    let kq = r.call(0, 0, msg(95, &[0x5A]));
    assert_eq!(
        kq,
        Err(KError::Cancelled),
        "Q7(b): caller Cancelled, không lỗi cứng"
    );
    r.kiem("call sau delete reply trong waiter");
    assert_eq!(r.st(1), TcbState::Ready, "waiter nhận message");
    assert_eq!(
        r.m.tcb_ipc_regs(r.t[1]).expect("regs")[0],
        0x5A,
        "message vẫn giao (không mất)"
    );
    assert_eq!(
        r.m.tcb_last_ipc_error(r.t[0]).expect("err"),
        Some(KError::Cancelled)
    );
    assert_eq!(r.m.ipc_sent(), 1);
    assert_eq!(r.m.ipc_delivered(), 1);
}

#[test]
fn reply_bi_xoa_caller_cancelled() {
    let mut r = Rig::new(2);
    r.recv(1, 0, Some(0)).expect("server Recv");
    r.call(0, 0, msg(90, &[1])).expect("client Call");
    assert_eq!(r.st(0), TcbState::BlockedOnReply { reply: r.rep[0] });
    r.m.delete(r.rep_s[0]).expect("Delete Reply cap");
    r.kiem("delete reply");
    assert_eq!(
        r.st(0),
        TcbState::Ready,
        "caller được đánh thức (không deadlock)"
    );
    assert_eq!(
        r.m.tcb_last_ipc_error(r.t[0]).expect("err"),
        Some(KError::Cancelled),
        "Q8: caller `Cancelled`"
    );
    assert_eq!(r.m.reply_caller(r.rep[0]).expect("rep0"), None);
    // Server gửi vào Reply đã bị xoá ⇒ `InvalidCapability` (`.scn` 07).
    assert_eq!(
        r.m.send(r.t[1], r.rep_s[0], msg_empty()),
        Err(KError::InvalidCapability)
    );
    r.kiem("delete reply sau");
}

#[test]
fn reply_bi_revoke_caller_cancelled_va_delete_slot_empty() {
    let mut r = Rig::new(2);
    // `rep0` là **con** của cap Reply gốc ở slot 2 ⇒ `Revoke` xoá được (`.scn` 08).
    let child =
        r.m.copy(slot(r.c0, S_REP[0]), slot(r.c0, 40))
            .expect("copy Reply");
    let child_s = slot(r.c0, 40);
    r.m.tcb_set_recv_window(
        r.t_s[1],
        [
            slot(r.c0, 90),
            slot(r.c0, 91),
            slot(r.c0, 92),
            slot(r.c0, 93),
        ],
    )
    .expect("window");
    r.m.recv(r.t[1], r.ep_s[0], Some(child_s))
        .expect("server Recv");
    r.call(0, 0, msg(91, &[2])).expect("client Call");
    assert_eq!(r.st(0), TcbState::BlockedOnReply { reply: r.rep[0] });
    assert!(child != 0);
    // `Revoke` toàn CNode: cap con ở slot 40 bị xoá ⇒ caller `Cancelled`.
    r.m.revoke(r.c0, None).expect("Revoke");
    r.kiem("revoke reply");
    assert_eq!(r.st(0), TcbState::Ready, "caller được đánh thức");
    assert_eq!(
        r.m.tcb_last_ipc_error(r.t[0]).expect("err"),
        Some(KError::Cancelled)
    );
    assert_eq!(r.m.reply_caller(r.rep[0]).expect("rep0"), None);
    assert_eq!(
        r.m.delete(child_s),
        Err(KError::SlotEmpty),
        "Revoke đã xoá cap con ⇒ Delete = SlotEmpty"
    );
    r.kiem("revoke reply sau");
}

#[test]
fn revoke_cap_con_reply_de_slot_empty() {
    // `.scn` 08 đúng nghĩa: Revoke **cap của CNode** (không phải Revoke Untyped) xoá cap con,
    // rồi `Delete` slot đó ⇒ `SlotEmpty`; CNode còn sống.
    let mut r = Rig::new(2);
    let child =
        r.m.copy(slot(r.c0, S_REP[0]), slot(r.c0, 40))
            .expect("copy Reply");
    let child_s = slot(r.c0, 40);
    r.m.tcb_set_recv_window(
        r.t_s[1],
        [
            slot(r.c0, 90),
            slot(r.c0, 91),
            slot(r.c0, 92),
            slot(r.c0, 93),
        ],
    )
    .expect("window");
    r.m.recv(r.t[1], r.ep_s[0], Some(child_s))
        .expect("server Recv");
    r.call(0, 0, msg(92, &[3])).expect("client Call");
    assert!(child != 0);
    r.m.revoke(r.c0, Some(S_REP[0]))
        .expect("Revoke cap Reply gốc");
    r.kiem("revoke con");
    assert_eq!(r.m.cap_at(r.c0, 40), Err(KError::SlotEmpty));
    assert_eq!(
        r.m.delete(child_s),
        Err(KError::SlotEmpty),
        ".scn` 08: Delete sau Revoke ⇒ SlotEmpty"
    );
    assert_eq!(r.st(0), TcbState::Ready);
    assert_eq!(
        r.m.tcb_last_ipc_error(r.t[0]).expect("err"),
        Some(KError::Cancelled)
    );
}

// ===========================================================================
// 9. Donation không rò khi Reply bị xoá/revoke/`Suspend(Abort)` caller (DoD 7).
// ===========================================================================

/// Rig donation: client **có** SC cấu hình, server **không** có SC riêng (điều kiện donate
/// của KT-21); cả hai `Ready`.
struct RigDon {
    m: Machine,
    c0: ObjId,
    ep_s: SlotRef,
    rep_s: SlotRef,
    sc_s: SlotRef,
    client_s: SlotRef,
    server: ObjId,
    client: ObjId,
}

impl RigDon {
    fn new() -> Self {
        let mut m = Machine::new();
        let c0 = m.add_cnode(64);
        let ctl = m.add_kernel_object(ObjType::SchedControl);
        let ctl_cap = m.root_cap(ctl);
        m.put_cap(c0, 60, ctl_cap).expect("ctl");
        let ctl_s = slot(c0, 60);

        let ep = m.add_kernel_object(ObjType::Endpoint);
        let ep_cap = m.root_cap(ep);
        m.put_cap(c0, 0, ep_cap).expect("ep");
        let rep = m.add_kernel_object(ObjType::Reply);
        let rep_cap = m.root_cap(rep);
        m.put_cap(c0, 1, rep_cap).expect("rep");

        let client = m.add_kernel_object(ObjType::Tcb);
        let client_cap = m.root_cap(client);
        m.put_cap(c0, 10, client_cap).expect("client");
        let server = m.add_kernel_object(ObjType::Tcb);
        let server_cap = m.root_cap(server);
        m.put_cap(c0, 11, server_cap).expect("server");

        // Client: SC cấu hình; server: **không** SC (D2-49(c): server passive).
        let sc = m.add_kernel_object(ObjType::SchedContext);
        let sc_cap = m.root_cap(sc);
        m.put_cap(c0, 20, sc_cap).expect("sc");
        let sc_s = slot(c0, 20);
        m.sched_control_configure(ctl_s, sc_s, 4, 8)
            .expect("configure");
        let client_s = slot(c0, 10);
        m.set_sched_params(client_s, Some(sc_s))
            .expect("set_sched_params");
        m.tcb_set_priority(client_s, 5, 255).expect("prio client");
        let server_s = slot(c0, 11);
        m.tcb_set_priority(server_s, 1, 255).expect("prio server");
        m.transition_tcb(client, TcbState::Ready).expect("Ready");
        m.transition_tcb(server, TcbState::Ready).expect("Ready");
        Self {
            m,
            c0,
            ep_s: slot(c0, 0),
            rep_s: slot(c0, 1),
            sc_s,
            client_s,
            server,
            client,
        }
    }

    fn kiem(&self, buoc: &str) {
        self.m
            .check_sched()
            .unwrap_or_else(|e| panic!("check_sched sau {buoc}: {e}"));
        self.m
            .check_sc_refs()
            .unwrap_or_else(|e| panic!("check_sc_refs sau {buoc}: {e}"));
        self.m
            .check_ipc()
            .unwrap_or_else(|e| panic!("check_ipc sau {buoc}: {e}"));
    }

    fn call_va_cho_donation(&mut self) {
        self.m
            .recv(self.server, self.ep_s, Some(self.rep_s))
            .expect("server Recv");
        self.m
            .call_msg(self.client, self.ep_s, msg(1, &[0x55]))
            .expect("client Call");
        assert_eq!(
            self.m.tcb_state(self.client).expect("state"),
            TcbState::BlockedOnReply {
                reply: self
                    .m
                    .cap_obj_at(self.rep_s.cnode, self.rep_s.slot)
                    .expect("rep")
            }
        );
        assert!(
            self.m.donated_sc(self.server).expect("donated").is_some(),
            "KT-21: SC của client đi theo Call sang server passive"
        );
        assert_eq!(
            self.m.sched_donor_of(self.sc_s).expect("donor"),
            Some(self.client)
        );
        // `sc_of(donor)` = `None` **trong lúc** SC đang cho mượn (KT-21: một SC một TCB).
        assert_eq!(self.m.sc_of(self.client).expect("sc"), None);
    }

    /// SC của client (obj) — dùng cho assert sau khi donation được trả.
    fn sc_obj(&self) -> ObjId {
        self.m
            .cap_obj_at(self.sc_s.cnode, self.sc_s.slot)
            .expect("sc obj")
    }
}

#[test]
fn donation_khong_ro_khi_reply_bi_xoa() {
    let mut r = RigDon::new();
    r.call_va_cho_donation();
    r.m.delete(r.rep_s).expect("Delete Reply đang dùng");
    r.kiem("delete donation");
    assert_eq!(r.m.tcb_state(r.client).expect("state"), TcbState::Ready);
    assert_eq!(
        r.m.tcb_last_ipc_error(r.client).expect("err"),
        Some(KError::Cancelled)
    );
    assert_eq!(
        r.m.donated_sc(r.server).expect("donated"),
        None,
        "SC trả về đúng chủ"
    );
    assert_eq!(r.m.sched_donor_of(r.sc_s).expect("donor"), None);
    assert_eq!(r.m.sched_donated_to(r.sc_s).expect("donee"), None);
    assert_eq!(
        r.m.sc_of(r.client).expect("sc"),
        Some(r.sc_obj()),
        "SC trả về đúng chủ (client)"
    );
    assert_eq!(
        r.m.sc_of(r.server).expect("sc"),
        None,
        "server không giữ SC"
    );
    r.kiem("delete donation sau");
}

#[test]
fn donation_khong_ro_khi_revoke_reply() {
    let mut r = RigDon::new();
    // `rep0` là con của cap Reply ở slot 1 ⇒ Revoke cap slot 1 xoá nó.
    let child = r.m.copy(slot(r.c0, 1), slot(r.c0, 30)).expect("copy Reply");
    let child_s = slot(r.c0, 30);
    r.m.recv(r.server, r.ep_s, Some(child_s))
        .expect("server Recv");
    r.m.call_msg(r.client, r.ep_s, msg(2, &[0x66]))
        .expect("Call");
    assert!(r.m.donated_sc(r.server).expect("donated").is_some());
    assert!(child != 0);
    r.m.revoke(r.c0, Some(1)).expect("Revoke cap Reply gốc");
    r.kiem("revoke donation");
    assert_eq!(r.m.tcb_state(r.client).expect("state"), TcbState::Ready);
    assert_eq!(r.m.donated_sc(r.server).expect("donated"), None);
    assert_eq!(r.m.sched_donor_of(r.sc_s).expect("donor"), None);
    assert_eq!(
        r.m.tcb_last_ipc_error(r.client).expect("err"),
        Some(KError::Cancelled)
    );
    r.kiem("revoke donation sau");
}

#[test]
fn donation_khong_ro_khi_suspend_abort_caller() {
    let mut r = RigDon::new();
    r.call_va_cho_donation();
    r.m.tcb_suspend(r.client_s, SuspendMode::Abort)
        .expect("Suspend(Abort) caller");
    r.kiem("suspend donation");
    assert_eq!(r.m.donated_sc(r.server).expect("donated"), None);
    assert_eq!(r.m.sched_donor_of(r.sc_s).expect("donor"), None);
    assert_eq!(
        r.m.sc_of(r.client).expect("sc"),
        Some(r.sc_obj()),
        "SC về client"
    );
    assert_eq!(r.m.sc_of(r.server).expect("sc"), None);
    r.kiem("suspend donation sau");
}

// ===========================================================================
// 11. `scenario.rs` có `Step::{ReplyRecv, NBSendRecv, NBSendWait, SendCaps}` và chạy được
//     qua `run_sequential` (DoD 8).
// ===========================================================================

#[test]
fn scenario_step_reply_recv_nb_cap() {
    let mut r = Rig::new(2);
    let ep0 = r.ep_s[0];
    let ep1 = r.ep_s[1];
    let rep0 = r.rep_s[0];
    let rep1 = r.rep_s[1];
    let t0 = r.t[0];
    let t1 = r.t[1];
    let caps = [r.src_caps[0], r.src_caps[1], 0, 0];
    let info = MessageInfo::new(3, 0, false, 0, 1).expect("info");
    let msg = Msg::new(info, &[0x77]).expect("Msg");

    let steps = vec![
        // 1. Server chờ message với Reply `rep0`.
        ScenarioStep {
            thread: "server",
            step: Step::Recv {
                tcb: t1,
                ep: ep0,
                reply: Some(rep0),
            },
            expect: Ok(()),
            expect_state: Some(TcbState::BlockedOnRecv {
                ep: r.ep[0],
                reply: Some(r.rep[0]),
            }),
        },
        // 2. Client Call ⇒ server Ready, client BlockedOnReply{rep0}.
        ScenarioStep {
            thread: "client",
            step: Step::Call {
                tcb: t0,
                ep: ep0,
                mr: 0x99,
            },
            expect: Ok(()),
            expect_state: Some(TcbState::BlockedOnReply { reply: r.rep[0] }),
        },
        // 3. `ReplyRecv`: trả lời client rồi chờ message kế với Reply mới `rep1`.
        ScenarioStep {
            thread: "server",
            step: Step::ReplyRecv {
                tcb: t1,
                reply: rep0,
                ep: ep1,
                msg,
                recv_reply: Some(rep1),
            },
            expect: Ok(()),
            expect_state: Some(TcbState::BlockedOnRecv {
                ep: r.ep[1],
                reply: Some(r.rep[1]),
            }),
        },
        // 4. `SendCaps`: client gửi 2 cap tới ô receive window của server.
        ScenarioStep {
            thread: "client",
            step: Step::SendCaps {
                tcb: t0,
                cap: ep1,
                msg,
                caps,
                n_caps: 2,
            },
            expect: Ok(()),
            expect_state: Some(TcbState::Ready),
        },
        // 5. `NBSendRecv` tự gửi rồi tự nhận trên `ep0` (hàng có message ngay) ⇒ `Ok`.
        ScenarioStep {
            thread: "server",
            step: Step::NBSendRecv {
                tcb: t1,
                send: ep0,
                msg,
                recv: ep0,
                recv_reply: None,
            },
            expect: Ok(()),
            expect_state: Some(TcbState::Ready),
        },
        // 6. `NBSendWait`: gửi rồi Wait Notification ⇒ `BlockedOnNotification`.
        ScenarioStep {
            thread: "server2",
            step: Step::NBSendWait {
                tcb: t1,
                send: ep1,
                msg,
                ntfn: slot(r.c0, S_NTFN),
            },
            expect: Ok(()),
            expect_state: Some(TcbState::BlockedOnNotification { ntfn: r.ntfn_obj() }),
        },
    ];
    let report = run_sequential(&mut r.m, &steps).expect("scenario KT-25 phải chạy được");
    assert_eq!(report.events, steps.len());
    assert_eq!(report.checked_bytes, steps.len());
    r.kiem("scenario KT-25");
    // `Step::name` của bốn bước mới.
    assert_eq!(steps[2].step.name(), "reply_recv");
    assert_eq!(steps[3].step.name(), "send_caps");
    assert_eq!(steps[4].step.name(), "nb_send_recv");
    assert_eq!(steps[5].step.name(), "nb_send_wait");
}

// ===========================================================================
// 12. 1.000 seed × 64 op hỗn hợp (call/reply/reply_recv/NB*/cap lỗi) — 0 vi phạm, cùng
//     seed cùng trace (DoD 9, Miri giảm seed bằng `cfg(miri)`).
// ===========================================================================

#[cfg(miri)]
const N_SEED: u64 = 3;
#[cfg(not(miri))]
const N_SEED: u64 = 1000;
#[cfg(miri)]
const N_STEP: u64 = 8;
#[cfg(not(miri))]
const N_STEP: u64 = 64;

/// Một dòng trace: `[step, op, ok, st0, st1, st2, sent, delivered, pending, dropped,
/// installed, unwrapped, failed]`.
type Row = [u64; 13];

fn op_caps(r: &Rig, sel: u64) -> Vec<CapId> {
    let mut caps = Vec::new();
    let n = 1 + (sel % MAX_CAPS as u64) as usize; // 1..=4
    for k in 0..n {
        caps.push(match (sel >> 2) as usize % 3 {
            0 => r.src_caps[k % 4],
            1 => {
                if k == 0 {
                    r.badge_cap
                } else {
                    r.src_caps[k % 4]
                }
            }
            _ => {
                if k == 0 {
                    r.nogrant_s_cap()
                } else {
                    r.src_caps[k % 4]
                }
            }
        });
    }
    caps
}

fn run_seed(seed: u64) -> Vec<Row> {
    let mut r = Rig::new(3);
    let mut rng = Lcg::new(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ 0x25);
    let mut trace = Vec::with_capacity(N_STEP as usize);
    for step in 0..N_STEP {
        let op = rng.below(100);
        let ti = rng.below(3) as usize;
        let epi = rng.below(2) as usize;
        let rep = rng.below(2) as usize;
        let len = match rng.below(4) {
            0 => 0usize,
            1 => 1,
            2 => 4,
            _ => 60,
        };
        let words: Vec<u64> = (0..len as u64)
            .map(|i| seed.wrapping_mul(1_000_003) ^ (step << 8) ^ i)
            .collect();
        let label = (rng.next_u64() & 0xF_FFFF) as u32;
        let sel = rng.next_u64();
        let ok = match op {
            0..=14 => {
                let caps = op_caps(&r, sel);
                r.call(ti, epi, msg_caps(label, &words, &caps)).is_ok()
            }
            15..=29 => r.send(ti, epi, msg(label, &words)).is_ok(),
            30..=44 => {
                let caps = op_caps(&r, sel);
                r.send(ti, epi, msg_caps(label, &words, &caps)).is_ok()
            }
            45..=54 => r.nb_send(ti, epi, msg(label, &words)).is_ok(),
            55..=69 => r.recv(ti, epi, Some(rep)).is_ok(),
            70..=79 => {
                let next = if rng.below(2) == 0 { Some(rep) } else { None };
                r.reply_recv(ti, rep, epi, msg(label, &words), next).is_ok()
            }
            80..=89 => {
                let recvi = rng.below(2) as usize;
                let next = if rng.below(2) == 0 { Some(rep) } else { None };
                r.nb_send_recv(ti, epi, msg(label, &words), recvi, next)
                    .is_ok()
            }
            90..=94 => r.nb_send_wait(ti, epi, msg(label, &words)).is_ok(),
            95..=97 => r.m.reply(r.t[ti], r.rep_s[rep]).is_ok(),
            _ => r.m.nb_recv(r.t[ti], r.ep_s[epi]).is_ok(),
        };
        let st = |i: usize| r.st(i).index() as u64;
        trace.push([
            step,
            op,
            u64::from(ok),
            st(0),
            st(1),
            st(2),
            r.m.ipc_sent(),
            r.m.ipc_delivered(),
            r.m.ipc_pending(),
            r.m.ipc_dropped(),
            r.m.cap_transfer_installed(),
            r.m.cap_unwrapped(),
            r.m.cap_transfer_failed(),
        ]);
        r.kiem("fuzz ipc2");
        assert_eq!(
            r.m.ipc_sent(),
            r.m.ipc_delivered() + r.m.ipc_pending() + r.m.ipc_dropped(),
            "seed {seed} bước {step}: message mất/thêm"
        );
        assert_eq!(r.m.transfer_calls(), r.m.ipc_delivered());
        assert_eq!(
            r.m.cap_transfer_installed() + r.m.cap_unwrapped(),
            r.m.cap_transfer_installed() + r.m.cap_unwrapped(),
            "bộ đếm cap đơn điệu"
        );
    }
    trace
}

#[test]
fn fuzz_1000_seed_64_op_ipc2() {
    let mut khac = 0usize;
    let mut truoc: Option<Vec<Row>> = None;
    for seed in 0..N_SEED {
        let a = run_seed(seed);
        let b = run_seed(seed);
        assert_eq!(a, b, "seed {seed}: cùng seed ⇒ cùng trace (tất định)");
        if let Some(p) = truoc.as_ref() {
            if *p != a {
                khac += 1;
            }
        }
        truoc = Some(a);
    }
    assert!(khac > 0, "các seed phải cho trace khác nhau");
}

// ===========================================================================
// 13. `MessageInfo.caps` ≤ 4 / `extra_caps_unwrapped` ≤ 0xF — không bit mới (D2-53 Q6).
// ===========================================================================

#[test]
fn message_info_giu_nguyen_35_bit_khi_co_cap() {
    assert_eq!(MAX_CAPS, 4);
    let info = MessageInfo::new(0x1234, 4, false, 0xF, 64).expect("hợp lệ");
    assert_eq!(info.caps(), 4);
    assert_eq!(info.pack() & (0x7 << 20), 4 << 20, "caps vẫn ở bit 20..22");
    assert!(
        MessageInfo::new(0x1234, 5, false, 0, 0).is_err(),
        "5 cap không biểu diễn được (không nới max_caps)"
    );
    assert!(MessageInfo::new(0, 0, false, 0x10, 0).is_err());
    assert_eq!(MAX_WORDS, 64);
}

// ===========================================================================
// 14. KT-34 (C2, D2-63) — cap `Reply` **đang active** (`caller = Some`) không được dùng lại
//     để `Recv`/`Call`: ghi đè `caller` làm TCB cũ `BlockedOnReply` **mất đường Reply**.
// ===========================================================================

/// `Recv` bằng cap `Reply` **đang** giữ `caller` ⇒ `Busy` **trước** khi nhận message nào:
/// caller cũ **vẫn** `BlockedOnReply` và **vẫn** là `caller` của Reply (không ghi đè); `Reply`
/// đúng ⇒ caller `Ready` + `caller = None`; sau đó cap **dùng lại được** (cả hai chiều).
#[test]
fn recv_cap_reply_dang_active_tra_busy_roi_dung_lai_duoc() {
    let mut r = Rig::new(3);
    // Server t[1] chờ trên ep0 với Reply `rep0` chưa dùng ⇒ `caller = None`: Recv **bình
    // thường** (đúng ca `.scn` 04/05) — không bị chặn oan.
    r.recv(1, 0, Some(0)).expect("Recv thường (caller None)");
    assert!(matches!(r.st(1), TcbState::BlockedOnRecv { .. }));
    // Client t[0] `Call` ⇒ message giao, `rep0.caller = Some(t[0])`, t[0] `BlockedOnReply`.
    r.call(0, 0, msg(0x34, &[0xA])).expect("Call");
    assert_eq!(r.st(0), TcbState::BlockedOnReply { reply: r.rep[0] });
    assert_eq!(r.m.reply_caller(r.rep[0]), Ok(Some(r.t[0])));
    assert_eq!(r.st(1), TcbState::Ready, "server được đánh thức");
    r.kiem("Call");
    // Server `Recv` **lại** bằng đúng cap Reply đang active ⇒ `Busy`, **không** đổi gì.
    assert_eq!(
        r.recv(1, 0, Some(0)),
        Err(KError::Busy),
        "cap Reply đang caller=Some ⇒ Busy (không ghi đè caller)"
    );
    assert_eq!(
        r.m.reply_caller(r.rep[0]),
        Ok(Some(r.t[0])),
        "caller cũ phải giữ nguyên sau Busy"
    );
    assert_eq!(
        r.st(0),
        TcbState::BlockedOnReply { reply: r.rep[0] },
        "caller cũ vẫn BlockedOnReply — không mất đường Reply"
    );
    assert_eq!(
        r.st(1),
        TcbState::Ready,
        "Busy không đổi trạng thái người nhận"
    );
    r.kiem("Recv cap active ⇒ Busy");
    // Reply **đúng** ⇒ caller `Ready`, `caller = None`.
    r.m.reply(r.t[1], r.rep_s[0]).expect("Reply");
    assert_eq!(r.m.reply_caller(r.rep[0]), Ok(None));
    assert_eq!(r.st(0), TcbState::Ready, "Reply đánh thức caller");
    r.kiem("Reply");
    // Chiều ngược: sau Reply, cap **dùng lại được** (caller None ⇒ không bị chặn).
    r.recv(1, 0, Some(0))
        .expect("Recv lại sau Reply (caller None)");
    assert!(matches!(r.st(1), TcbState::BlockedOnRecv { .. }));
    r.call(2, 0, msg(0x35, &[0xB])).expect("Call thứ hai");
    assert_eq!(r.st(2), TcbState::BlockedOnReply { reply: r.rep[0] });
    assert_eq!(r.m.reply_caller(r.rep[0]), Ok(Some(r.t[2])));
    r.kiem("Call thứ hai");
}

/// Lớp chặn **thứ hai** ở `Call`: hai cap cùng trỏ **một** `Reply` object, waiter `Recv` đã
/// nhận cap lúc `caller = None` nhưng Reply bị bind **qua cap kia** ⇒ `Call` tới waiter đó phải
/// `Busy` (không giao message, không ghi đè caller). Sau `Reply` (caller None) thì `Call`
/// **thành công** — không chặn vĩnh viễn.
#[test]
fn call_vao_waiter_co_reply_da_active_qua_cap_khac_tra_busy() {
    let mut r = Rig::new(3);
    // Bản sao cap `Reply rep0` (cùng object, khác slot) — đường "cap khác" của mô hình.
    let rep2_s = slot(r.c0, 110);
    r.m.copy(r.rep_s[0], rep2_s).expect("copy cap Reply");
    // t[1] Recv ep0 bằng cap **gốc** (caller None lúc nhận ⇒ hợp lệ).
    r.recv(1, 0, Some(0)).expect("Recv t1");
    // t[2] Recv ep1 bằng cap **copy** (cùng object rep0).
    r.m.recv(r.t[2], r.ep_s[1], Some(rep2_s)).expect("Recv t2");
    // t[0] Call ep1 ⇒ bind `rep0.caller = Some(t[0])` **qua cap copy**.
    r.call(0, 1, msg(0x36, &[0xC])).expect("Call ep1");
    assert_eq!(r.m.reply_caller(r.rep[0]), Ok(Some(r.t[0])));
    r.kiem("Call ep1 bind caller");
    // t[2] (đã Ready) Call ep0: waiter t[1] giữ Reply **đã active** ⇒ `Busy`; message **không**
    // vào IPC (không cấp `msg_id`), waiter vẫn chờ, caller cũ giữ nguyên.
    assert_eq!(r.call(2, 0, msg(0x37, &[0xD])), Err(KError::Busy));
    assert_eq!(r.m.ipc_sent(), 1, "Call bị từ chối không cấp msg_id");
    assert!(
        matches!(r.st(1), TcbState::BlockedOnRecv { .. }),
        "waiter ep0 vẫn chờ"
    );
    assert_eq!(
        r.m.reply_caller(r.rep[0]),
        Ok(Some(r.t[0])),
        "caller cũ giữ nguyên"
    );
    r.kiem("Call vào waiter có Reply active ⇒ Busy");
    // Reply ⇒ caller None; `Call` lại **thành công** (không chặn vĩnh viễn).
    r.m.reply(r.t[2], rep2_s).expect("Reply");
    assert_eq!(r.m.reply_caller(r.rep[0]), Ok(None));
    assert_eq!(r.st(0), TcbState::Ready);
    r.call(2, 0, msg(0x38, &[0xE])).expect("Call lại sau Reply");
    assert_eq!(r.st(1), TcbState::Ready);
    assert_eq!(r.st(2), TcbState::BlockedOnReply { reply: r.rep[0] });
    r.kiem("Call lại sau Reply");
}

/// **Bất biến C2** (`Machine::check_reply_links`) và **cổng** `transition_tcb` cùng giữ liên
/// kết `Reply.caller ⇔ BlockedOnReply`: rời `BlockedOnReply` qua cổng thấp thì `caller` được
/// gỡ; vào `BlockedOnReply` thì `caller` được bind; **ghi đè** caller đang active bị cổng từ
/// chối (`IllegalOperation`) — cùng lỗi C2, chặn ở mọi đường.
#[test]
fn cong_transition_giu_lien_ket_reply_va_khong_ghi_de_caller() {
    let mut r = Rig::new(3);
    r.recv(1, 0, Some(0)).expect("Recv");
    r.call(0, 0, msg(0x39, &[0xF])).expect("Call");
    assert_eq!(r.m.reply_caller(r.rep[0]), Ok(Some(r.t[0])));
    // Trạng thái **đúng** ⇒ `Ok`.
    r.m.check_reply_links().expect("liên kết Reply đúng");
    // Cổng thấp: t[2] thử vào `BlockedOnReply` trên **cùng** Reply đang active ⇒ từ chối,
    // không đổi trạng thái, **không** ghi đè caller (lỗi C2 bị chặn ngay ở cổng).
    let st2 = r.st(2);
    assert_eq!(
        r.m.transition_tcb(r.t[2], TcbState::BlockedOnReply { reply: r.rep[0] }),
        Err(KError::IllegalOperation),
        "Reply đang bind caller khác ⇒ cổng không cho bind đè"
    );
    assert_eq!(r.st(2), st2, "cổng từ chối ⇒ trạng thái không đổi");
    assert_eq!(r.m.reply_caller(r.rep[0]), Ok(Some(r.t[0])));
    r.m.check_reply_links().expect("liên kết Reply vẫn đúng");
    // Vào `BlockedOnReply` bằng cổng thấp trên Reply **rảnh** ⇒ tự bind `caller = Some(t[2])`.
    assert_eq!(r.m.reply_caller(r.rep[1]), Ok(None));
    r.m.transition_tcb(r.t[2], TcbState::BlockedOnReply { reply: r.rep[1] })
        .expect("vào BlockedOnReply trên Reply rảnh");
    assert_eq!(
        r.m.reply_caller(r.rep[1]),
        Ok(Some(r.t[2])),
        "cổng vào BlockedOnReply phải bind caller"
    );
    r.m.check_reply_links().expect("liên kết Reply sau khi vào");
    // Rời `BlockedOnReply` bằng cổng thấp ⇒ tự gỡ `caller` (không còn "caller ma").
    r.m.transition_tcb(r.t[2], TcbState::Ready)
        .expect("rời BlockedOnReply");
    assert_eq!(
        r.m.reply_caller(r.rep[1]),
        Ok(None),
        "cổng rời BlockedOnReply phải gỡ caller"
    );
    r.m.check_reply_links().expect("liên kết Reply sau khi rời");
}

/// **Phân biệt then chốt của KT-34** (đúng câu hỏi reviewer): `Call` xảy ra **trước** `Recv` đầu
/// tiên ⇒ message nằm ở **hàng gửi**, `Reply` **chưa** bind (`caller = None`) ⇒ `Recv` với cap
/// Reply **vẫn hợp lệ** (kernel mới bind `caller` lúc giao). Chỉ **lần Recv thứ hai** trên cùng
/// `Reply` đang active mới `Busy`. Tức quy tắc là "Reply **đã** dùng cho một Call đang chờ", không
/// phải "cứ có Call trước là cấm Recv".
#[test]
fn call_truoc_recv_lan_dau_van_hop_le_chi_chan_lan_hai() {
    let mut r = Rig::new(3);
    // Client `Call` khi **chưa** ai `Recv` ⇒ xếp hàng gửi, chưa bind Reply.
    r.call(0, 0, msg(0x3A, &[0x11]))
        .expect("Call khi chưa có waiter");
    assert!(matches!(r.st(0), TcbState::BlockedOnSend { .. }));
    assert_eq!(r.m.reply_caller(r.rep[0]), Ok(None), "Reply chưa bị bind");
    r.kiem("Call xếp hàng");
    // `Recv` **đầu tiên** với cap Reply **rảnh** ⇒ Ok (đường `.scn` 04/05).
    r.recv(1, 0, Some(0)).expect("Recv đầu tiên sau Call");
    assert_eq!(r.st(0), TcbState::BlockedOnReply { reply: r.rep[0] });
    assert_eq!(r.m.reply_caller(r.rep[0]), Ok(Some(r.t[0])));
    assert_eq!(r.st(1), TcbState::Ready);
    r.kiem("Recv đầu tiên");
    // `Recv` **thứ hai** với cùng cap **đang active** ⇒ Busy (mới là lỗi C2).
    assert_eq!(
        r.recv(1, 0, Some(0)),
        Err(KError::Busy),
        "chỉ lần Recv thứ hai trên Reply đã bind mới bị chặn"
    );
    assert_eq!(r.m.reply_caller(r.rep[0]), Ok(Some(r.t[0])));
    r.kiem("Recv thứ hai ⇒ Busy");
}

/// `ReplyRecv` với Reply **mới** đang active ⇒ `Busy` **trước** khi gửi reply; còn dùng **chính**
/// `Reply` vừa trả lời (`reply == recv_reply`) ⇒ hợp lệ (reply tiêu thụ `caller` trước, lúc `recv`
/// chạy Reply đã rảnh) và vẫn bind được cho `Call` kế — không bị `Busy` oan.
#[test]
fn reply_recv_kiem_reply_moi_truoc_khi_gui_va_dung_lai_chinh_reply_hop_le() {
    let mut r = Rig::new(3);
    // t[1] server ep0 (rep0); t[0] Call ⇒ rep0.caller = Some(t[0]); t[1] Ready (có message).
    r.recv(1, 0, Some(0)).expect("Recv ep0");
    r.call(0, 0, msg(0x3B, &[0x12])).expect("Call ep0");
    assert_eq!(r.m.reply_caller(r.rep[0]), Ok(Some(r.t[0])));
    // t[2] server ep1 (rep1); t[1] Call ep1 ⇒ rep1.caller = Some(t[1]) (**active**), t[2] Ready.
    r.m.recv(r.t[2], r.ep_s[1], Some(r.rep_s[1]))
        .expect("Recv ep1 (rep1)");
    r.call(1, 1, msg(0x3C, &[0x13])).expect("Call ep1");
    assert_eq!(r.m.reply_caller(r.rep[1]), Ok(Some(r.t[1])));
    r.kiem("hai Reply đều active");
    // t[2] `reply_recv` với Reply **mới** = rep0 (đang active) ⇒ Busy **trước** khi gửi reply trên
    // rep1: không reply nửa chừng, caller hai bên giữ nguyên.
    assert_eq!(
        r.reply_recv(2, 1, 0, msg(0x3D, &[0x14]), Some(0)),
        Err(KError::Busy),
        "Reply mới (rep0) đang active ⇒ Busy trước khi gửi reply"
    );
    assert_eq!(
        r.m.reply_caller(r.rep[1]),
        Ok(Some(r.t[1])),
        "chưa gửi reply nào (rep1 vẫn bind caller)"
    );
    assert_eq!(r.m.reply_caller(r.rep[0]), Ok(Some(r.t[0])));
    r.kiem("ReplyRecv bị chặn trước khi gửi");
    // t[2] `reply_recv` dùng **chính** rep1 (vừa trả lời) cho lượt Recv kế ⇒ hợp lệ.
    r.reply_recv(2, 1, 0, msg(0x3E, &[0x15]), Some(1))
        .expect("reply_recv reply=recv_reply=rep1");
    assert_eq!(
        r.m.reply_caller(r.rep[1]),
        Ok(None),
        "reply tiêu thụ caller"
    );
    assert_eq!(r.st(1), TcbState::Ready, "caller t[1] được đánh thức");
    assert_eq!(
        r.st(2),
        TcbState::BlockedOnRecv {
            ep: r.ep[0],
            reply: Some(r.rep[1])
        },
        "server chờ Call kế trên ep0 với Reply vừa dùng (đã rảnh)"
    );
    r.kiem("ReplyRecv dùng lại chính Reply");
    // `Call` kế bind lại bình thường (rep1 đã rảnh).
    r.call(1, 0, msg(0x3F, &[0x16])).expect("Call kế");
    assert_eq!(r.st(1), TcbState::BlockedOnReply { reply: r.rep[1] });
    assert_eq!(r.m.reply_caller(r.rep[1]), Ok(Some(r.t[1])));
    r.kiem("Call kế bind lại");
}
