// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-24 — **IPC core 1**: `MessageInfo` 35 bit + `Message` 64 MR + `ipc::transfer` (một đường
//! chuyển message), `Send/NBSend/Recv/NBRecv`, `no_reply` **hai chiều** (D2-53 Q7), `Call` trên
//! object **không** phải Endpoint trả `NotSupported` (D2-53 Q11).
//!
//! **Model-only `[TK]`, không phải spec/ABI**; layout `MessageInfo`/64 MR đọc **đúng**
//! `spec/abi.toml:31-49`: `label:0..19, caps:20..22, no_reply:23, extra_caps_unwrapped:24..27,
//! length:28..34`; 4 MR đầu ở "thanh ghi", 60 MR sau ở **IPC buffer model** (hai vùng riêng
//! trong `TcbData`, không gộp).
//!
//! Nguồn ngữ nghĩa: thiết kế §4.6 `:316-324` (MessageInfo/MR/`ipc::transfer`/invocation),
//! `spec/abi.toml:105-121` (syscall), **D2-53 Q7** (`no_reply` hai chiều), **Q11** (Call
//! non-Endpoint `NotSupported`), K1C-PLAN §5 KT-24 DoD 1..8.
//!
//! Test đỏ trước (D2-05): [`JOURNAL/kt24-red-compile.log`](../../agent/JOURNAL/kt24-red-compile.log)
//! (`vifast_sim::ipc` chưa có + `send/nb_send/nb_recv/call_msg` E0599) và
//! [`JOURNAL/kt24-red-semantics.log`](../../agent/JOURNAL/kt24-red-semantics.log) (probe tạm:
//! thiếu Reply trả `IllegalOperation` thay vì `Cancelled`; `last_mr` chỉ giữ **một** word).
//!
//! Bất biến kiểm sau **mỗi** thao tác: `check_sched` + `check_mailboxes` + `check_ipc`
//! (`ipc_sent == ipc_delivered + ipc_pending + ipc_dropped`, `transfer_calls == ipc_delivered`,
//! mỗi `msg_id` nằm đúng một nơi).

use vifast_sim::ipc::{
    Message, MessageInfo, INFO_BITS, INFO_MASK, MAX_WORDS, WORDS_IN_IPC_BUFFER, WORDS_IN_REGISTERS,
};
use vifast_sim::kobj::{KError, Machine, ObjId, ObjType, SlotRef, TcbState};
use vifast_sim::Lcg;

fn slot(cnode: ObjId, s: usize) -> SlotRef {
    SlotRef::new(cnode, s)
}

/// `Message` với `len = words.len()`, không cap (cap thuộc KT-25).
fn msg(label: u32, words: &[u64]) -> Message {
    let info = MessageInfo::new(label, 0, false, 0, words.len() as u8).expect("MessageInfo hợp lệ");
    Message::new(info, words).expect("Message hợp lệ")
}

/// `Message` bật cờ `no_reply` (D2-53 Q7 nhánh (a)).
fn msg_no_reply(label: u32, words: &[u64]) -> Message {
    let info = MessageInfo::new(label, 0, true, 0, words.len() as u8).expect("MessageInfo hợp lệ");
    Message::new(info, words).expect("Message hợp lệ")
}

fn msg_empty() -> Message {
    msg(0, &[])
}

// ---------------------------------------------------------------------------
// Rig: CNode 128 slot; 2 Endpoint; 1 Reply; n TCB; n SchedContext (test-only, D2-50);
// 1 SchedControl. TCB 0 = client; TCB 1 = server; các TCB sau là người gửi phụ.
// ---------------------------------------------------------------------------

const S_EP: [usize; 2] = [0, 1];
const S_REP: usize = 2;
const S_TCB: usize = 10;
const S_SC: usize = 70;
const S_CTL: usize = 120;

struct Rig {
    m: Machine,
    c0: ObjId,
    ep: [ObjId; 2],
    ep_s: [SlotRef; 2],
    rep: ObjId,
    rep_s: SlotRef,
    t: Vec<ObjId>,
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
        for i in 0..n_tcb {
            let obj = m.add_kernel_object(ObjType::Tcb);
            let cap = m.root_cap(obj);
            m.put_cap(c0, S_TCB + i, cap).expect("cap TCB");
            t.push(obj);

            let sc = m.add_kernel_object(ObjType::SchedContext);
            let sc_cap = m.root_cap(sc);
            m.put_cap(c0, S_SC + i, sc_cap).expect("cap SchedContext");
            let sc_slot = slot(c0, S_SC + i);
            m.sched_control_configure(ctl_s, sc_slot, 4, 8)
                .expect("SchedControl.Configure");
            let t_slot = slot(c0, S_TCB + i);
            m.set_sched_params(t_slot, Some(sc_slot))
                .expect("set_sched_params (test-only)");
            m.tcb_set_priority(t_slot, 5, 255).expect("Tcb.SetPriority");
        }
        let mut r = Rig {
            m,
            c0,
            ep,
            ep_s,
            rep,
            rep_s,
            t,
        };
        for i in 0..r.t.len() {
            r.m.transition_tcb(r.t[i], TcbState::Ready)
                .expect("Inactive→Ready");
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
    }

    fn st(&self, i: usize) -> TcbState {
        self.m.tcb_state(self.t[i]).expect("tcb_state")
    }

    fn call_msg(&mut self, i: usize, epi: usize, m: Message) -> Result<(), KError> {
        self.m.call_msg(self.t[i], self.ep_s[epi], m)
    }

    fn send(&mut self, i: usize, epi: usize, m: Message) -> Result<(), KError> {
        self.m.send(self.t[i], self.ep_s[epi], m)
    }

    fn nb_send(&mut self, i: usize, epi: usize, m: Message) -> Result<(), KError> {
        self.m.nb_send(self.t[i], self.ep_s[epi], m)
    }

    fn recv(&mut self, i: usize, epi: usize, reply: bool) -> Result<(), KError> {
        let rep = if reply { Some(self.rep_s) } else { None };
        self.m.recv(self.t[i], self.ep_s[epi], rep)
    }

    fn nb_recv(&mut self, i: usize, epi: usize) -> Result<Option<Message>, KError> {
        self.m.nb_recv(self.t[i], self.ep_s[epi])
    }

    fn reply(&mut self, i: usize) -> Result<(), KError> {
        self.m.reply(self.t[i], self.rep_s)
    }

    /// `Send` tới **Reply** cap (đường `send cap=rep0` của `.scn` 04) — đi qua `send`.
    fn send_reply(&mut self, i: usize) -> Result<(), KError> {
        self.m.send(self.t[i], self.rep_s, msg_empty())
    }

    fn regs(&self, i: usize) -> [u64; WORDS_IN_REGISTERS] {
        self.m.tcb_ipc_regs(self.t[i]).expect("tcb_ipc_regs")
    }

    fn buffer(&self, i: usize) -> [u64; WORDS_IN_IPC_BUFFER] {
        self.m.tcb_ipc_buffer(self.t[i]).expect("tcb_ipc_buffer")
    }

    fn ipc_info(&self, i: usize) -> MessageInfo {
        self.m
            .tcb_ipc_info(self.t[i])
            .expect("tcb_ipc_info")
            .expect("TCB đã nhận message")
    }

    /// FIFO `msg_id` của hàng gửi Endpoint (nilai 0 = message fault, không có `msg_id`).
    fn ep_ids(&self, epi: usize) -> Vec<u64> {
        self.m.endpoint_msg_ids(self.ep[epi]).expect("hàng gửi")
    }

    fn identity(&self) -> (u64, u64, u64, u64, u64) {
        (
            self.m.ipc_sent(),
            self.m.ipc_delivered(),
            self.m.ipc_pending(),
            self.m.ipc_dropped(),
            self.m.transfer_calls(),
        )
    }
}

// ===========================================================================
// 1. `MessageInfo` 35 bit — layout **chính xác** theo `spec/abi.toml:46` (DoD 1).
// ===========================================================================

#[test]
fn message_info_dung_layout_35_bit() {
    assert_eq!(INFO_BITS, 35);
    assert_eq!(INFO_MASK, (1u64 << 35) - 1);
    let mi = MessageInfo::new(0xABCDE, 4, true, 0xF, 64).expect("hợp lệ");
    // label:0..19, caps:20..22, no_reply:23, extra_caps_unwrapped:24..27, length:28..34.
    let want = 0xABCDEu64 | (4u64 << 20) | (1u64 << 23) | (0xFu64 << 24) | (64u64 << 28);
    assert_eq!(mi.pack(), want, "bit đúng thứ tự spec/abi.toml");
    assert_eq!(mi.bits(), want);
    assert_eq!(mi.bits() & !INFO_MASK, 0, "35 bit, không bit thừa");
    assert_eq!(mi.label(), 0xABCDE);
    assert_eq!(mi.caps(), 4);
    assert!(mi.no_reply());
    assert_eq!(mi.extra_caps_unwrapped(), 0xF);
    assert_eq!(mi.length(), 64);

    // Kiểm từng dải bit **độc lập** (không suy từ pack của chính nó).
    let mi2 = MessageInfo::new(0x12345, 2, false, 3, 7).expect("hợp lệ");
    let bits = mi2.pack();
    assert_eq!(bits & 0xF_FFFF, 0x12345, "label:0..19");
    assert_eq!((bits >> 20) & 0x7, 2, "caps:20..22");
    assert_eq!((bits >> 23) & 1, 0, "no_reply:23");
    assert_eq!((bits >> 24) & 0xF, 3, "extra_caps_unwrapped:24..27");
    assert_eq!((bits >> 28) & 0x7F, 7, "length:28..34");
    assert_eq!(MessageInfo::empty().pack(), 0);
    assert_eq!(
        MessageInfo::unpack(0).expect("0 hợp lệ"),
        MessageInfo::empty()
    );
    assert_eq!(
        MessageInfo::unpack(want).expect("unpack"),
        mi,
        "unpack(pack(x)) == x"
    );
}

// ===========================================================================
// 2. Round-trip 1.000 giá trị ngẫu nhiên (LCG) + biên `caps > 4`, `len > 64` (DoD 1).
// ===========================================================================

#[test]
fn message_info_roundtrip_1000_va_bien() {
    let mut rng = Lcg::new(24);
    for k in 0..1_000u64 {
        let label = (rng.next_u64() & 0xF_FFFF) as u32;
        let caps = (rng.below(5)) as u8;
        let no_reply = rng.below(2) == 1;
        let extra = (rng.below(16)) as u8;
        let len = (rng.below(65)) as u8;
        let mi = MessageInfo::new(label, caps, no_reply, extra, len)
            .unwrap_or_else(|e| panic!("lần {k}: MessageInfo hợp lệ nhưng lỗi {e:?}"));
        assert_eq!(
            MessageInfo::unpack(mi.pack()).expect("unpack"),
            mi,
            "round-trip lần {k}"
        );
        assert_eq!(mi.pack() & !INFO_MASK, 0);
    }
    // Biên: `caps` chỉ 3 bit nhưng ABI `max_caps = 4` ⇒ 5/6/7 là lỗi.
    for caps in 5..=7u8 {
        assert_eq!(
            MessageInfo::new(0, caps, false, 0, 0),
            Err(KError::InvalidArgument),
            "caps={caps} > max_caps=4"
        );
    }
    // `length` 7 bit nhưng ABI `max_words = 64` ⇒ 65..127 là lỗi.
    for len in 65..=127u8 {
        assert_eq!(
            MessageInfo::new(0, 0, false, 0, len),
            Err(KError::InvalidArgument),
            "len={len} > max_words=64"
        );
    }
    // `extra_caps_unwrapped` 4 bit ⇒ 16 là lỗi.
    assert_eq!(
        MessageInfo::new(0, 0, false, 16, 0),
        Err(KError::InvalidArgument)
    );
    // `label` 20 bit ⇒ 1<<20 là lỗi.
    assert_eq!(
        MessageInfo::new(1 << 20, 0, false, 0, 0),
        Err(KError::InvalidArgument)
    );
    // Bit ngoài 35 là lỗi khi unpack (không cắt im lặng).
    assert_eq!(
        MessageInfo::unpack(INFO_MASK + 1),
        Err(KError::InvalidArgument)
    );
    assert_eq!(MessageInfo::unpack(u64::MAX), Err(KError::InvalidArgument));
    // Giá trị 35 bit có `caps` 5..7 / `length` 65..127 không phải MessageInfo hợp lệ.
    assert_eq!(
        MessageInfo::unpack(5u64 << 20),
        Err(KError::InvalidArgument),
        "caps ngoài max_caps"
    );
    assert_eq!(
        MessageInfo::unpack(65u64 << 28),
        Err(KError::InvalidArgument),
        "length ngoài max_words"
    );
}

// ===========================================================================
// 3. `Message` 64 MR: 4 word đầu ở thanh ghi, 60 word sau ở IPC buffer (DoD 2).
// ===========================================================================

#[test]
fn message_64_mr_hai_vung_thanh_ghi_va_ipc_buffer() {
    assert_eq!(MAX_WORDS, 64);
    assert_eq!(WORDS_IN_REGISTERS, 4);
    assert_eq!(WORDS_IN_IPC_BUFFER, 60);
    let mut r = Rig::new(2);
    let words: Vec<u64> = (0..64u64).map(|i| 0xF000_0000 + i).collect();
    let m = msg(7, &words);
    assert_eq!(m.len(), 64);
    assert_eq!(m.words(), &words[..], "Message giữ byte-for-byte");

    r.call_msg(0, 0, m).expect("Call xếp hàng");
    r.kiem("Call 64 MR");
    assert_eq!(r.ep_ids(0), vec![1], "một message trong hàng");
    r.recv(1, 0, true).expect("Recv lấy message");
    r.kiem("Recv 64 MR");

    let info = r.ipc_info(1);
    assert_eq!(info.length(), 64);
    assert_eq!(info.label(), 7);
    let regs = r.regs(1);
    let buffer = r.buffer(1);
    assert_eq!(regs, [words[0], words[1], words[2], words[3]], "4 MR đầu");
    for i in 0..WORDS_IN_IPC_BUFFER {
        assert_eq!(buffer[i], words[4 + i], "MR {} ở IPC buffer", 4 + i);
    }
    // `mr[4]` **chỉ** có trong buffer — không bị gộp vào vùng thanh ghi.
    assert_eq!(buffer[0], words[4]);
    assert!(
        !regs.contains(&words[4]),
        "mr[4] không được nằm trong vùng thanh ghi"
    );
    assert_eq!(r.m.tcb_ipc_msg_id(r.t[1]), Ok(Some(1)));
    assert_eq!(r.st(0), TcbState::BlockedOnReply { reply: r.rep });
    assert_eq!(r.st(1), TcbState::Ready);
    assert_eq!(
        r.identity(),
        (1, 1, 0, 0, 1),
        "sent/delivered/pending/dropped/transfer"
    );
}

// ===========================================================================
// 4. Message ngắn không để lại rác trong hai vùng; `transfer` là chỗ duy nhất copy (DoD 2/3).
// ===========================================================================

#[test]
fn transfer_mot_duong_va_xoa_du_lieu_cu() {
    let words: Vec<u64> = (0..64u64).map(|i| 0x1000 + i).collect();
    let mut area = vifast_sim::ipc::IpcArea::new();
    let n = vifast_sim::ipc::transfer(&msg(1, &words), &mut area);
    assert_eq!(n, 64);
    assert_eq!(area.regs, [0x1000, 0x1001, 0x1002, 0x1003]);
    assert_eq!(area.buffer[59], 0x1000 + 63);
    assert_eq!(
        area.msg_id,
        Some(0),
        "transfer không đổi msg_id do kernel cấp"
    );
    // Message ngắn hơn phải **xoá sạch** phần còn lại (không lộ dữ liệu message trước).
    let n2 = vifast_sim::ipc::transfer(&msg(2, &[9, 8]), &mut area);
    assert_eq!(n2, 2);
    assert_eq!(area.regs, [9, 8, 0, 0]);
    assert!(area.buffer.iter().all(|w| *w == 0), "buffer cũ bị xoá");
    assert_eq!(area.info.expect("info").label(), 2);
    // Message rỗng: cả hai vùng rỗng.
    assert_eq!(vifast_sim::ipc::transfer(&msg(3, &[]), &mut area), 0);
    assert_eq!(area.regs, [0; WORDS_IN_REGISTERS]);
    assert!(area.buffer.iter().all(|w| *w == 0));
}

// ===========================================================================
// 5. `send` (chặn) + `nb_send` (không chặn) + FIFO; `nb_recv` Busy khi rỗng (DoD 4).
// ===========================================================================

#[test]
fn send_nb_send_fifo_va_nb_recv_busy() {
    let mut r = Rig::new(3);
    assert_eq!(r.nb_recv(1, 0), Err(KError::Busy), "rỗng ⇒ Busy");
    assert_eq!(r.st(1), TcbState::Ready, "NBRecv không chặn");

    // `send` (chặn): chưa có người nhận ⇒ xếp hàng, người gửi BlockedOnSend.
    r.send(0, 0, msg(10, &[100])).expect("Send xếp hàng");
    r.kiem("send chặn");
    assert_eq!(
        r.st(0),
        TcbState::BlockedOnSend {
            ep: r.ep[0],
            call: false,
            fault: None
        }
    );
    // `nb_send` (không chặn): xếp hàng nhưng người gửi **vẫn Ready**.
    r.nb_send(2, 0, msg(11, &[200])).expect("NBSend xếp hàng");
    r.kiem("nb_send không chặn");
    assert_eq!(r.st(2), TcbState::Ready);
    assert_eq!(r.ep_ids(0), vec![1, 2], "FIFO theo msg_id");
    assert_eq!(r.m.ipc_sent(), 2);
    assert_eq!(r.m.ipc_pending(), 2);
    assert_eq!(r.m.ipc_delivered(), 0);

    // `recv` lấy message của `send` ⇒ người gửi chặn được đánh thức, không Reply.
    r.recv(1, 0, false).expect("Recv 1");
    r.kiem("recv 1");
    assert_eq!(r.st(0), TcbState::Ready, "sender của Send được đánh thức");
    let got = r.ipc_info(1);
    assert_eq!(got.label(), 10);
    assert_eq!(r.regs(1)[..1], [100]);
    // `nb_recv` lấy message của `nb_send`; người gửi vốn Ready nên không đổi.
    let m2 = r.nb_recv(1, 0).expect("NBRecv").expect("có message");
    assert_eq!(m2.info.label(), 11);
    assert_eq!(m2.msg_id, 2);
    assert_eq!(r.st(2), TcbState::Ready);
    assert_eq!(r.nb_recv(1, 0), Err(KError::Busy));
    r.kiem("nb_recv");
    assert_eq!(r.identity(), (2, 2, 0, 0, 2));
}

// ===========================================================================
// 6. `recv` chặn (`BlockedOnRecv`) + giao **thẳng** khi người gửi tới; `Send` không Reply.
// ===========================================================================

#[test]
fn recv_chan_va_giao_thang_cho_waiter() {
    let mut r = Rig::new(2);
    r.recv(1, 0, false).expect("Recv chặn");
    r.kiem("recv chặn");
    assert_eq!(
        r.st(1),
        TcbState::BlockedOnRecv {
            ep: r.ep[0],
            reply: None
        }
    );
    assert_eq!(r.m.endpoint_info(r.ep[0]).expect("info").len, 0);

    let words: Vec<u64> = (0..64u64).map(|i| 0x2000 + i).collect();
    r.send(0, 0, msg(20, &words)).expect("Send giao thẳng");
    r.kiem("send giao thẳng");
    assert_eq!(r.st(1), TcbState::Ready, "waiter được đánh thức");
    assert_eq!(r.st(0), TcbState::Ready, "Send giao được ⇒ không chặn");
    assert_eq!(r.m.endpoint_info(r.ep[0]).expect("info").len, 0);
    assert_eq!(r.identity(), (1, 1, 0, 0, 1));
    assert_eq!(r.ipc_info(1).length(), 64);
    for i in 0..60 {
        assert_eq!(r.buffer(1)[i], words[4 + i]);
    }
}

// ===========================================================================
// 7. `Call` + `Reply` giữ nguyên (chuyển message qua `transfer`), `send cap=Reply`.
// ===========================================================================

#[test]
fn call_reply_va_send_tren_reply() {
    let mut r = Rig::new(2);
    // `call` cũ (một MR) vẫn là wrapper của Message 1 word: tương thích K1b.
    r.m.call(r.t[0], r.ep_s[0], 0x42).expect("Call cũ");
    r.kiem("call cũ");
    assert_eq!(r.m.endpoint_last_mr(r.ep[0]), Ok(Some(0x42)));
    r.recv(1, 0, true).expect("Recv + Reply cap");
    r.kiem("recv call");
    assert_eq!(r.regs(1), [0x42, 0, 0, 0]);
    assert_eq!(r.ipc_info(1).length(), 1);
    assert_eq!(r.st(0), TcbState::BlockedOnReply { reply: r.rep });
    assert_eq!(r.m.reply_caller(r.rep), Ok(Some(r.t[0])));
    // `.scn` 04: server trả lời bằng `Send` trên **Reply cap** (không phải `recv`).
    r.send_reply(1).expect("Send trên Reply");
    r.kiem("send reply");
    assert_eq!(r.st(0), TcbState::Ready, "Reply đánh thức caller");
    assert_eq!(r.m.reply_caller(r.rep), Ok(None));
}

// ===========================================================================
// 8. `no_reply` **hai chiều** (D2-53 Q7) — (a) caller đặt cờ ⇒ Call = Send, Ok ngay.
// ===========================================================================

#[test]
fn no_reply_caller_dat_co_khong_sinh_reply() {
    // (a1) chưa có waiter: Ok **ngay**, message xếp hàng, không Reply, caller vẫn Ready.
    let mut r = Rig::new(2);
    r.call_msg(0, 0, msg_no_reply(30, &[1, 2, 3]))
        .expect("no_reply=1 ⇒ Ok ngay");
    r.kiem("no_reply chưa waiter");
    assert_eq!(
        r.st(0),
        TcbState::Ready,
        "Call = Send, không chặn chờ Reply"
    );
    assert_eq!(r.st(1), TcbState::Ready);
    assert_eq!(r.ep_ids(0), vec![1]);
    r.recv(1, 0, true).expect("Recv");
    r.kiem("no_reply recv");
    let info = r.ipc_info(1);
    assert!(info.no_reply(), "bên nhận **thấy** cờ no_reply");
    assert_eq!(info.label(), 30);
    assert_eq!(
        r.st(0),
        TcbState::Ready,
        "không sinh Reply/không BlockedOnReply"
    );
    assert_eq!(r.m.reply_caller(r.rep), Ok(None), "không bind Reply");
    assert_eq!(r.m.tcb_last_ipc_error(r.t[0]), Ok(None));

    // (a2) waiter đã chờ: giao thẳng, Ok, không Reply, bên nhận thấy cờ.
    let mut r = Rig::new(2);
    r.recv(1, 0, true).expect("Recv chặn (có Reply cap)");
    r.call_msg(0, 0, msg_no_reply(31, &[9]))
        .expect("no_reply=1 giao thẳng");
    r.kiem("no_reply giao thẳng");
    assert_eq!(r.st(0), TcbState::Ready);
    assert_eq!(r.st(1), TcbState::Ready);
    assert!(r.ipc_info(1).no_reply());
    assert_eq!(r.regs(1), [9, 0, 0, 0]);
    assert_eq!(r.m.reply_caller(r.rep), Ok(None));
}

// ===========================================================================
// 9. `no_reply` nhánh (b): kernel thiếu Reply ⇒ message **vẫn giao**, caller `Cancelled`.
// ===========================================================================

#[test]
fn no_reply_kernel_thieu_reply_tra_cancelled() {
    // (b1) server đã `Recv` **không** kèm Reply cap: caller nhận `Cancelled` ngay, message
    // vẫn tới server + cờ `no_reply` bật (thiết kế :322, D2-53 Q7).
    let mut r = Rig::new(2);
    r.recv(1, 0, false).expect("Recv không Reply cap");
    let kq = r.call_msg(0, 0, msg(40, &[5, 6]));
    assert_eq!(
        kq,
        Err(KError::Cancelled),
        "kernel thiếu Reply ⇒ caller Cancelled (hiện/thiết kế :322)"
    );
    r.kiem("call thiếu reply (b1)");
    assert_eq!(r.st(1), TcbState::Ready, "message vẫn giao cho server");
    assert!(r.ipc_info(1).no_reply(), "cờ do kernel bật, server thấy");
    assert_eq!(r.regs(1), [5, 6, 0, 0]);
    assert_eq!(r.st(0), TcbState::Ready, "caller không bị treo");
    assert_eq!(r.m.tcb_last_ipc_error(r.t[0]), Ok(Some(KError::Cancelled)));
    assert_eq!(r.identity(), (1, 1, 0, 0, 1));

    // (b2) caller `Call` xếp hàng trước, server `Recv` sau **không** Reply cap: khi giao,
    // caller được đánh thức và thấy `Cancelled` qua `tcb_last_ipc_error`.
    let mut r = Rig::new(2);
    r.call_msg(0, 0, msg(41, &[7])).expect("Call chặn");
    assert_eq!(
        r.st(0),
        TcbState::BlockedOnSend {
            ep: r.ep[0],
            call: true,
            fault: None
        }
    );
    r.recv(1, 0, false).expect("Recv không Reply cap");
    r.kiem("call thiếu reply (b2)");
    assert_eq!(
        r.st(0),
        TcbState::Ready,
        "caller được đánh thức, không treo"
    );
    assert_eq!(r.m.tcb_last_ipc_error(r.t[0]), Ok(Some(KError::Cancelled)));
    assert!(r.ipc_info(1).no_reply());
    assert_eq!(r.m.reply_caller(r.rep), Ok(None));
}

// ===========================================================================
// 10. `Call` trên object **không** phải Endpoint: `NotSupported`, không chặn/không Reply,
//     `TcbState` không đổi (DoD 6, D2-53 Q11); `Send` sai kiểu ⇒ `WrongObjectType`.
// ===========================================================================

#[test]
fn call_non_endpoint_not_supported_khong_doi_trang_thai() {
    let mut r = Rig::new(2);
    // Thêm cap tới chính CNode gốc, tới TCB, tới Reply, tới Untyped ở các slot riêng.
    let c0_cap = r.m.root_cap(r.c0);
    r.m.put_cap(r.c0, 40, c0_cap).expect("cap CNode");
    r.m.copy(slot(r.c0, S_TCB + 1), slot(r.c0, 41))
        .expect("cap TCB (Copy)");
    let u0 = r.m.add_untyped(0x5000, 8192);
    let u0_cap = r.m.root_cap(u0);
    r.m.put_cap(r.c0, 42, u0_cap).expect("cap Untyped");
    let ntfn = r.m.add_kernel_object(ObjType::Notification);
    let ntfn_cap = r.m.root_cap(ntfn);
    r.m.put_cap(r.c0, 43, ntfn_cap).expect("cap Notification");
    let rep_cap2 = r.m.root_cap(r.rep);
    r.m.put_cap(r.c0, 44, rep_cap2).expect("cap Reply 2");

    for (slot_i, ten) in [
        (40usize, "CNode"),
        (41, "Tcb"),
        (44usize, "Reply"),
        (42, "Untyped"),
    ] {
        let kq = r.m.call_msg(r.t[0], slot(r.c0, slot_i), msg(50, &[1]));
        assert_eq!(
            kq,
            Err(KError::NotSupported),
            "Call trên {ten} chưa cài method ⇒ NotSupported"
        );
        assert_eq!(r.st(0), TcbState::Ready, "Call non-Endpoint không chặn");
        assert_eq!(r.m.ipc_sent(), 0, "không sinh message");
        assert_eq!(r.m.ipc_pending(), 0);
        r.kiem("call non-endpoint");
    }
    // Cap rỗng ⇒ InvalidCapability (không phải NotSupported).
    assert_eq!(
        r.m.call_msg(r.t[0], slot(r.c0, 45), msg(50, &[1])),
        Err(KError::InvalidCapability)
    );
    // `Send` trên object không phải Endpoint/Reply ⇒ WrongObjectType (`.scn` 06).
    assert_eq!(
        r.m.send(r.t[0], slot(r.c0, 40), msg(51, &[2])),
        Err(KError::WrongObjectType)
    );
    assert_eq!(
        r.m.send(r.t[0], slot(r.c0, 42), msg(51, &[2])),
        Err(KError::WrongObjectType)
    );
    assert_eq!(r.st(0), TcbState::Ready);
    assert_eq!(r.m.ipc_sent(), 0);
    // Notification: KT-27 mới cài Signal ⇒ `NotSupported` (không im lặng bỏ).
    assert_eq!(
        r.m.send(r.t[0], slot(r.c0, 43), msg(52, &[3])),
        Err(KError::NotSupported)
    );
    r.kiem("send sai kiểu");
}

// ===========================================================================
// 11. Hàng gửi đầy (`MAILBOX_CAP = 8`) ⇒ `Busy`, **không** mất message/không đổi trạng thái.
// ===========================================================================

#[test]
fn hang_gui_day_tra_busy_khong_mat_message() {
    let mut r = Rig::new(3);
    for i in 0..8u64 {
        r.nb_send(2, 0, msg(60 + i as u32, &[i]))
            .unwrap_or_else(|e| panic!("NBSend {i}: {e:?}"));
    }
    r.kiem("đầy hàng");
    assert_eq!(r.m.endpoint_info(r.ep[0]).expect("info").len, 8);
    assert_eq!(r.m.ipc_sent(), 8);
    assert_eq!(r.m.ipc_pending(), 8);
    // Cả `nb_send` và `send` (chặn) đều `Busy` **trước** thay đổi trạng thái.
    assert_eq!(
        r.nb_send(2, 0, msg(99, &[99])),
        Err(KError::Busy),
        "NBSend khi đầy ⇒ Busy"
    );
    assert_eq!(
        r.send(0, 0, msg(99, &[99])),
        Err(KError::Busy),
        "Send khi đầy ⇒ Busy"
    );
    assert_eq!(r.st(0), TcbState::Ready, "Busy không đổi trạng thái");
    assert_eq!(r.m.ipc_sent(), 8, "message bị từ chối không tính");
    assert_eq!(r.ep_ids(0), (1..=8).collect::<Vec<_>>());
    // Pop 1 ⇒ đẩy lại được; FIFO giữ nguyên.
    r.nb_recv(1, 0).expect("NBRecv").expect("message");
    r.nb_send(2, 0, msg(70, &[70])).expect("còn chỗ");
    r.kiem("sau pop");
    assert_eq!(r.m.ipc_sent(), 9);
    assert_eq!(r.m.ipc_delivered(), 1);
    assert_eq!(r.m.ipc_pending(), 8);
}

// ===========================================================================
// 12. Bất biến `ipc_sent == ipc_delivered + ipc_pending` (không abort ⇒ dropped = 0)
//     trên 1.000 seed × 64 op (stable; Miri giảm seed/bước bằng `cfg(miri)`).
// ===========================================================================

#[cfg(miri)]
const N_SEED: u64 = 3;
#[cfg(not(miri))]
const N_SEED: u64 = 1000;
#[cfg(miri)]
const N_STEP: u64 = 8;
#[cfg(not(miri))]
const N_STEP: u64 = 64;

/// Một dòng trace: `[now, step, op, ok, state client, state server, sent, delivered, pending,
/// dropped, transfer, len, label]`.
type Row = [u64; 13];

fn run_seed(seed: u64) -> Vec<Row> {
    let mut r = Rig::new(3);
    let mut rng = Lcg::new(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ 0x24);
    let mut trace = Vec::with_capacity(N_STEP as usize);
    for step in 0..N_STEP {
        let op = rng.below(100);
        let ti = rng.below(2) as usize; // chỉ client/server để dễ kiểm trạng thái
        let epi = rng.below(2) as usize;
        let len = match rng.below(5) {
            0 => 0usize,
            1 => 1,
            2 => 4,
            3 => 60,
            _ => 64,
        };
        let words: Vec<u64> = (0..len as u64)
            .map(|i| seed.wrapping_mul(1_000_003) ^ (step << 8) ^ i)
            .collect();
        let label = (rng.next_u64() & 0xF_FFFF) as u32;
        let no_reply = rng.below(2) == 1;
        let m = if no_reply {
            msg_no_reply(label, &words)
        } else {
            msg(label, &words)
        };
        let ok = match op {
            0..=19 => r.call_msg(ti, epi, m).is_ok(),
            20..=39 => r.send(ti, epi, m).is_ok(),
            40..=54 => r.nb_send(ti, epi, m).is_ok(),
            55..=74 => r.recv(ti, epi, rng.below(2) == 0).is_ok(),
            75..=89 => r.nb_recv(ti, epi).is_ok(),
            90..=94 => r.reply(ti).is_ok(),
            _ => r.m.exit(r.t[ti], seed ^ step).is_ok(),
        };
        let st = |x: TcbState| x.index() as u64;
        trace.push([
            r.m.virtual_now(),
            step,
            op,
            u64::from(ok),
            st(r.st(0)),
            st(r.st(1)),
            r.m.ipc_sent(),
            r.m.ipc_delivered(),
            r.m.ipc_pending(),
            r.m.ipc_dropped(),
            r.m.transfer_calls(),
            len as u64,
            label as u64,
        ]);
        r.kiem("fuzz op");
        // Bất biến đầy đủ (kể cả message bị huỷ bởi Exit — D2-25).
        assert_eq!(
            r.m.ipc_sent(),
            r.m.ipc_delivered() + r.m.ipc_pending() + r.m.ipc_dropped(),
            "seed {seed} bước {step}: ipc_sent != delivered + pending + dropped"
        );
        assert_eq!(
            r.m.transfer_calls(),
            r.m.ipc_delivered(),
            "seed {seed} bước {step}: transfer_calls != delivered"
        );
    }
    trace
}

#[test]
fn fuzz_1000_seed_64_op_bat_bien_ipc() {
    let mut khac = 0usize;
    for seed in 0..N_SEED {
        let a = run_seed(seed);
        let b = run_seed(seed);
        assert_eq!(a, b, "seed {seed}: cùng seed ⇒ cùng trace (tất định)");
        if seed > 0 {
            let prev = run_seed(seed - 1);
            if prev != a {
                khac += 1;
            }
        }
    }
    assert!(
        khac > 0,
        "các seed phải cho trace khác nhau (không bị dồn về một hành vi)"
    );
}

/// Không `Exit`/`Suspend` ⇒ **không** message nào bị huỷ (`dropped = 0`) và bất biến đúng
/// nguyên văn K1C-PLAN §1.1 C1: `ipc_sent == ipc_delivered + ipc_pending`.
#[test]
fn bao_toan_message_khi_khong_abort() {
    let mut r = Rig::new(3);
    let mut rng = Lcg::new(0x24_24);
    for step in 0..N_STEP {
        let op = rng.below(100);
        let ti = rng.below(2) as usize;
        let epi = rng.below(2) as usize;
        let len = (rng.below(65)) as usize;
        let words: Vec<u64> = (0..len as u64).map(|i| (step << 16) | i).collect();
        let m = msg((rng.next_u64() & 0xF_FFFF) as u32, &words);
        let _ = match op {
            0..=24 => r.call_msg(ti, epi, m),
            25..=49 => r.send(ti, epi, m),
            50..=69 => r.nb_send(ti, epi, m),
            70..=89 => r.recv(ti, epi, rng.below(2) == 0),
            _ => r.nb_recv(ti, epi).map(|_| ()),
        };
        r.kiem("không abort");
        assert_eq!(r.m.ipc_dropped(), 0, "không abort ⇒ không message bị huỷ");
        assert_eq!(
            r.m.ipc_sent(),
            r.m.ipc_delivered() + r.m.ipc_pending(),
            "bước {step}: ipc_sent == delivered + pending"
        );
    }
}
