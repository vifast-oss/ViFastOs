// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-25 — **runner `.scn`** cho IPC core 2: đọc **(không sửa)** năm tệp
//! `v2/test/scn/{05-replyrecv-server,07-reply-deleted-before-reply,
//! 08-reply-revoked-while-blocked,15-cap-transfer-four,16-cap-transfer-no-grant}.scn`,
//! chạy đúng chuỗi bước của từng tệp trên mô hình KT-25 và so **kết quả từng bước** với kỳ
//! vọng ghi trong tệp (`=> ok` / `=> <mã lỗi>`), 0 lệch (K1C-PLAN §5 KT-25 DoD 8).
//!
//! Quy ước (đọc cùng `kt24_scn.rs` + **D2-54**):
//!
//! 1. `ep0` (bên gọi) và `ep1` (bên nhận) trỏ **cùng một** Endpoint.
//! 2. `recv`/`reply_recv` không nêu `reply=` ⇒ runner cấp `rep0` làm Reply window.
//! 3. 08 cần `Revoke` xoá được `rep0` ⇒ `rep0` là **cap con** của cap Reply gốc ở slot 1.
//! 4. 15: `c0,...` là tên cap; runner dựng 4 cap thật ở slot 30..33. **5 cap** ⇒
//!    `InvalidArgument` **trước** khi message vào đường IPC (`spec/abi.toml` `max_caps = 4`).
//! 5. 16: `c1` là cap **thiếu** `RIGHT_GRANT` (copy với `rights=READ|WRITE`) ⇒ `Mint` vượt
//!    quyền trả `InsufficientRights`.
//!
//! Tệp này **đọc đĩa** nên **không** chạy dưới Miri (isolation); Miri của KT-25 chạy
//! `tests/kt25_ipc2.rs` (thuần mô hình) — cùng quy ước `kt24_scn.rs`/`kt24_ipc.rs`.

use std::fs;
use std::path::{Path, PathBuf};

use vifast_sim::ipc::MessageInfo;
use vifast_sim::ipc::{Message, MAX_CAPS};
use vifast_sim::kobj::{
    KError, Machine, ObjId, ObjType, SlotRef, TcbState, RIGHT_GRANT, RIGHT_READ, RIGHT_WRITE,
};

fn slot(cnode: ObjId, s: usize) -> SlotRef {
    SlotRef::new(cnode, s)
}

/// `Message` **không** cap; `words.len()` phải bằng `info.length()`.
fn msg(label: u32, words: &[u64]) -> Message {
    let info = MessageInfo::new(label, 0, false, 0, words.len() as u8).expect("MessageInfo hợp lệ");
    Message::new(info, words).expect("Message hợp lệ")
}

/// `Message` mang `caps` (≤ 4).
fn msg_caps(label: u32, words: &[u64], caps: &[vifast_sim::kobj::CapId]) -> Message {
    Message::with_cap_ids(label, words, caps).expect("Message có cap hợp lệ")
}

fn msg_empty() -> Message {
    msg(0, &[])
}

// ===========================================================================
// 10. Runner `.scn` 05, 07, 08, 15, 16 — đọc tệp (**không sửa**) và so từng bước (DoD 8).
// ===========================================================================

fn scn_path(ten: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../test/scn")
        .join(ten)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Buoc {
    op: String,
    params: Vec<(String, String)>,
    expect: String,
}

impl Buoc {
    fn param(&self, k: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(key, _)| key == k)
            .map(|(_, v)| v.as_str())
    }
}

fn doc_buoc(ten: &str) -> Vec<Buoc> {
    let path = scn_path(ten);
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("không đọc được {}: {e}", path.display()));
    assert!(text.contains("version = 1"), "{ten}: thiếu version = 1");
    let mut steps = Vec::new();
    let mut rest = text.as_str();
    while let Some(start) = rest.find('"') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('"') else { break };
        let inner = &after[..end];
        rest = &after[end + 1..];
        let Some((body, expect)) = inner.rsplit_once("=>") else {
            continue;
        };
        let mut toks = body.split_whitespace();
        let op = toks.next().expect("bước rỗng").to_string();
        let mut params = Vec::new();
        for tok in toks {
            match tok.split_once('=') {
                Some((k, v)) => params.push((k.to_string(), v.to_string())),
                // `invoke CNode.Delete ...`: token thứ hai là **tên method**, không phải k=v.
                None if op == "invoke" && params.is_empty() => {
                    params.push(("invoke".to_string(), tok.to_string()));
                }
                None => panic!("{ten}: tham số không phải k=v: {tok:?}"),
            }
        }
        steps.push(Buoc {
            op,
            params,
            expect: expect.trim().to_string(),
        });
    }
    assert!(!steps.is_empty(), "{ten}: không trích được bước nào");
    steps
}

fn ten_ket_qua(r: Result<(), KError>) -> String {
    match r {
        Ok(()) => "ok".to_string(),
        Err(e) => e.name().to_string(),
    }
}

/// Rig cho `.scn` 05/07/08: **một** Endpoint chung (`ep0`/`ep1` cùng Endpoint — D2-54),
/// `rep0` là cap Reply (gốc hoặc con của cap ở slot `root` tuỳ kịch bản).
struct RigScn {
    m: Machine,
    c0: ObjId,
    ep_s: SlotRef,
    rep_s: SlotRef,
    t: [ObjId; 2],
}

impl RigScn {
    /// `copy_rep`: nếu `true` thì `rep0` là **con** của cap Reply gốc ở slot 1 (cho `.scn` 08).
    fn new(copy_rep: bool) -> Self {
        let mut m = Machine::new();
        let c0 = m.add_cnode(48);
        let ep = m.add_kernel_object(ObjType::Endpoint);
        let ep_cap = m.root_cap(ep);
        m.put_cap(c0, 0, ep_cap).expect("ep");
        let rep = m.add_kernel_object(ObjType::Reply);
        let rep_cap = m.root_cap(rep);
        m.put_cap(c0, 1, rep_cap).expect("rep gốc");
        let rep_s = if copy_rep {
            let child = m.copy(slot(c0, 1), slot(c0, 2)).expect("copy rep0");
            let _ = child;
            slot(c0, 2)
        } else {
            slot(c0, 1)
        };
        let mut t = [0u32; 2];
        for (i, obj) in t.iter_mut().enumerate() {
            *obj = m.add_kernel_object(ObjType::Tcb);
            let cap = m.root_cap(*obj);
            m.put_cap(c0, 10 + i, cap).expect("tcb");
            m.transition_tcb(*obj, TcbState::Ready).expect("Ready");
        }
        Self {
            m,
            c0,
            ep_s: slot(c0, 0),
            rep_s,
            t,
        }
    }

    fn kiem(&self, ten: &str) {
        self.m
            .check_sched()
            .unwrap_or_else(|e| panic!("{ten}: check_sched: {e}"));
        self.m
            .check_ipc()
            .unwrap_or_else(|e| panic!("{ten}: check_ipc: {e}"));
    }

    /// Chạy các bước theo **đúng thứ tự trong tệp** (thread ghi trong tệp = client/server).
    fn chay(&mut self, ten: &str, steps: &[Buoc]) -> usize {
        let mut khop = 0;
        for (i, b) in steps.iter().enumerate() {
            let kq = match b.op.as_str() {
                "call" => {
                    assert_eq!(b.param("cap"), Some("ep0"), "{ten} bước {i}");
                    if b.param("caps").is_some() {
                        let caps = vec![
                            self.m.cap_at(self.c0, 30).expect("cap 30"),
                            self.m.cap_at(self.c0, 31).expect("cap 31"),
                            self.m.cap_at(self.c0, 32).expect("cap 32"),
                            self.m.cap_at(self.c0, 33).expect("cap 33"),
                        ];
                        let words = parse_mr(b);
                        self.m
                            .call_msg(self.t[0], self.ep_s, msg_caps(0, &words, &caps))
                    } else {
                        let words = parse_mr(b);
                        self.m.call_msg(self.t[0], self.ep_s, msg(0, &words))
                    }
                }
                "recv" => {
                    assert_eq!(b.param("cap"), Some("ep1"), "{ten} bước {i}");
                    self.m.recv(self.t[1], self.ep_s, Some(self.rep_s))
                }
                "reply_recv" => {
                    assert_eq!(b.param("reply"), Some("rep0"), "{ten} bước {i}");
                    assert_eq!(b.param("cap"), Some("ep1"), "{ten} bước {i}");
                    let words = parse_mr(b);
                    self.m
                        .reply_recv(self.t[1], self.rep_s, self.ep_s, msg(0, &words), None)
                }
                "nb_recv" => {
                    assert_eq!(b.param("cap"), Some("ep0"), "{ten} bước {i}");
                    self.m.nb_recv(self.t[0], self.ep_s).map(|_| ())
                }
                "send" => {
                    assert_eq!(b.param("cap"), Some("rep0"), "{ten} bước {i}");
                    self.m.send(self.t[1], self.rep_s, msg_empty())
                }
                "invoke" => match b.param("invoke") {
                    Some("CNode.Delete") => match b.param("cnode") {
                        Some("c0") => match b.param("slot") {
                            Some("rep0") => self.m.delete(self.rep_s),
                            other => panic!("{ten} bước {i}: slot {other:?} chưa dựng"),
                        },
                        other => panic!("{ten} bước {i}: cnode {other:?} chưa dựng"),
                    },
                    other => panic!("{ten} bước {i}: invoke {other:?} chưa hỗ trợ"),
                },
                "invoke_old" => match b.param("cnode") {
                    Some("c0") => match b.param("slot") {
                        Some("rep0") => self.m.delete(self.rep_s),
                        other => panic!("{ten} bước {i}: slot {other:?} chưa dựng"),
                    },
                    other => panic!("{ten} bước {i}: cnode {other:?} chưa dựng"),
                },
                other => panic!("{ten} bước {i}: op {other:?} chưa hỗ trợ"),
            };
            assert_eq!(
                ten_ket_qua(kq),
                b.expect,
                "{ten} bước {i} ({}): kết quả lệch so với tệp .scn",
                b.op
            );
            self.kiem(ten);
            khop += 1;
        }
        khop
    }
}

/// Slot nguồn cho cap thứ `k` của `.scn` 15 (chỉ 4 cap thật ở slot 30..33).
fn caps_cho_phep(k: usize) -> usize {
    k % MAX_CAPS
}

fn parse_mr(b: &Buoc) -> Vec<u64> {
    match b.param("mr") {
        None => Vec::new(),
        Some(s) => s
            .split(',')
            .map(|w| w.parse::<u64>().expect("mr u64"))
            .collect(),
    }
}

#[test]
fn runner_scn_05_07_08_15_16_khong_lech() {
    // 05 — ReplyRecv (server nóng): call xếp hàng rồi ReplyRecv trả lời + Recv.
    let s05 = doc_buoc("05-replyrecv-server.scn");
    assert_eq!(s05.len(), 3, "05: call + reply_recv + nb_recv");
    let mut r = RigScn::new(false);
    assert_eq!(r.chay("05", &s05), 3);
    assert_eq!(
        r.m.reply_caller(r.m.cap_obj_at(r.c0, 1).expect("rep")),
        Ok(None)
    );

    // 07 — Delete Reply đang dùng ⇒ caller `Cancelled`, `send rep0` ⇒ InvalidCapability.
    let s07 = doc_buoc("07-reply-deleted-before-reply.scn");
    assert_eq!(s07.len(), 4, "07: call + recv + delete + send");
    let mut r = RigScn::new(false);
    assert_eq!(r.chay("07", &s07), s07.len());

    // 08 — Revoke rồi Delete ⇒ `SlotEmpty` (rep0 là cap con).
    let s08 = doc_buoc("08-reply-revoked-while-blocked.scn");
    let mut r = RigScn::new(true);
    // Runner riêng cho 08: `Revoke` cần `Step::Revoke` chứ không phải Delete.
    let mut khop = 0;
    for (i, b) in s08.iter().enumerate() {
        let kq = match b.op.as_str() {
            "call" => r.m.call_msg(r.t[0], r.ep_s, msg(0, &parse_mr(b))),
            "recv" => r.m.recv(r.t[1], r.ep_s, Some(r.rep_s)),
            "invoke" => match (b.param("cnode"), b.param("slot")) {
                (Some("c0"), None) => r.m.revoke(r.c0, None).map(|_| ()),
                (Some("c0"), Some("rep0")) => r.m.delete(r.rep_s),
                other => panic!("08 bước {i}: {other:?}"),
            },
            other => panic!("08 bước {i}: op {other:?}"),
        };
        assert_eq!(ten_ket_qua(kq), b.expect, "08 bước {i} ({}): lệch", b.op);
        r.kiem("08");
        khop += 1;
    }
    assert_eq!(khop, s08.len());
    assert_eq!(
        r.m.tcb_last_ipc_error(r.t[0]).expect("err"),
        Some(KError::Cancelled)
    );

    // 15 — 4 cap ⇒ ok; 5 cap ⇒ InvalidArgument trước khi giao.
    let s15 = doc_buoc("15-cap-transfer-four.scn");
    assert_eq!(s15.len(), 2);
    let mut r = RigScn::new(false);
    for s in [30usize, 31, 32, 33] {
        let obj = r.m.add_kernel_object(ObjType::Endpoint);
        let cap = r.m.root_cap(obj);
        r.m.put_cap(r.c0, s, cap).expect("cap nguồn 15");
    }
    for (i, b) in s15.iter().enumerate() {
        let words = parse_mr(b);
        // `.scn` khai theo **tên cap** (`c0,...`); runner dựng 4 cap thật ở slot 30..33 và
        // tái dùng cho tới đủ số tên trong bước (5 tên ⇒ 5 cap ⇒ `InvalidArgument`).
        let mut caps = Vec::new();
        for _ in b.param("caps").expect("caps=").split(',') {
            caps.push(
                r.m.cap_at(r.c0, 30 + caps_cho_phep(caps.len()))
                    .expect("cap nguồn 15"),
            );
        }
        let kq = match b.op.as_str() {
            "call" | "send" => {
                if caps.len() > MAX_CAPS {
                    // Bước 5 cap: dựng message có 5 cap ⇒ `InvalidArgument` **trước** khi giao.
                    Message::with_cap_ids(0, &words, &caps).map(|_| ())
                } else if b.op == "call" {
                    r.m.call_msg(r.t[0], r.ep_s, msg_caps(0, &words, &caps))
                } else {
                    r.m.send(r.t[0], r.ep_s, msg_caps(0, &words, &caps))
                }
            }
            other => panic!("15 bước {i}: op {other:?}"),
        };
        assert_eq!(ten_ket_qua(kq), b.expect, "15 bước {i} lệch");
        r.kiem("15");
    }
    assert_eq!(r.m.ipc_sent(), 1, "chỉ bước đầu vào đường IPC");

    // 16 — Mint vượt quyền ⇒ InsufficientRights.
    let s16 = doc_buoc("16-cap-transfer-no-grant.scn");
    assert_eq!(s16.len(), 1);
    let mut r = RigScn::new(false);
    let nguon =
        r.m.copy_rights(slot(r.c0, 0), slot(r.c0, 5), Some(RIGHT_READ | RIGHT_WRITE))
            .expect("cap nguồn thiếu GRANT");
    let _ = nguon;
    let b = &s16[0];
    assert_eq!(b.op, "invoke");
    let kq = r.m.mint(
        slot(r.c0, 5),
        slot(r.c0, 0),
        RIGHT_WRITE | RIGHT_GRANT,
        None,
    );
    assert_eq!(ten_ket_qua(kq.map(|_| ())), b.expect, "16: Mint vượt quyền");
    r.kiem("16");
}
