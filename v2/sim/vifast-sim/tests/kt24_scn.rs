// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-24 — **runner `.scn`** cho IPC core 1: đọc **(không sửa)** ba tệp
//! `v2/test/scn/{04-call-reply-basic,06-call-invalid-cap,17-call-no-reply-bit}.scn`, chạy đúng
//! chuỗi bước của tệp trên mô hình KT-24 và so **kết quả từng bước** với kỳ vọng ghi trong tệp
//! (`=> ok` / `=> <mã lỗi>`), 0 lệch (K1C-PLAN §5 KT-24 DoD 8).
//!
//! Quy ước của fixture (ghi rõ để review):
//!
//! 1. `caps` là **khai báo tên**; runner dựng rig theo kịch bản: `ep0` (bên gọi) và `ep1`
//!    (bên nhận) trỏ **cùng một** Endpoint — đúng cách các tệp 04/05/07/12/17 dùng cặp này.
//! 2. `recv` **không** nêu `reply=` ⇒ runner cấp `rep0` làm Reply window — đúng giả định của
//!    tệp 07 (`CNode.Delete ... slot=rep0` rồi `send cap=rep0 => InvalidCapability` chứng minh
//!    `rep0` là Reply của `Call`).
//! 3. Tệp 06 (`call-invalid-cap`): `c1` là slot **rỗng** (call ⇒ `InvalidCapability`) và `c0` là
//!    cap CNode (send ⇒ `WrongObjectType`) — đúng tên/tags của kịch bản.
//!
//! Cũng kiểm `Step::{Send, NBSend, NBRecv}` + `Msg` mới của `scenario.rs` chạy qua
//! `run_sequential` (cùng đường `Machine` mà runner dùng).

use std::fs;
use std::path::{Path, PathBuf};

use vifast_sim::ipc::{Message, MessageInfo};
use vifast_sim::kobj::{KError, Machine, ObjId, ObjType, SlotRef, TcbState};
use vifast_sim::scenario::{run_sequential, Msg, ScenarioStep, Step};

fn slot(cnode: ObjId, s: usize) -> SlotRef {
    SlotRef::new(cnode, s)
}

fn scn_path(ten: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../test/scn")
        .join(ten)
}

/// Một bước `.scn` đã tách: `op`, các `k=v`, và tên kết quả kỳ vọng.
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

/// Trích mọi chuỗi trong tệp `.scn` có chứa `=>` (đó là các bước), **đọc tệp** là nguồn kỳ
/// vọng duy nhất — không chép tay kết quả.
fn doc_buoc(ten: &str) -> Vec<Buoc> {
    let path = scn_path(ten);
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("không đọc được {}: {e}", path.display()));
    assert!(
        text.contains("version = 1"),
        "{ten}: fixture phải có version = 1"
    );
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
            let (k, v) = tok
                .split_once('=')
                .unwrap_or_else(|| panic!("{ten}: tham số không phải k=v: {tok:?}"));
            params.push((k.to_string(), v.to_string()));
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

fn ket_qua(r: Result<(), KError>) -> String {
    match r {
        Ok(()) => "ok".to_string(),
        Err(e) => e.name().to_string(),
    }
}

/// Parse `mr=1,2` thành danh sách word; vắng mặt ⇒ message rỗng (`length = 0`).
fn parse_mr(b: &Buoc) -> Vec<u64> {
    match b.param("mr") {
        None => Vec::new(),
        Some(s) => s
            .split(',')
            .map(|w| {
                w.parse::<u64>()
                    .unwrap_or_else(|e| panic!("mr {w:?} không phải u64: {e}"))
            })
            .collect(),
    }
}

fn message_tu_buoc(b: &Buoc) -> Message {
    let words = parse_mr(b);
    let no_reply = matches!(b.param("no_reply"), Some("1"));
    let info = MessageInfo::new(0, 0, no_reply, 0, words.len() as u8).expect("MessageInfo");
    Message::new(info, &words).expect("Message")
}

// ---------------------------------------------------------------------------
// Rig cho 04/17: một Endpoint chung + một Reply; client t0, server t1.
// ---------------------------------------------------------------------------

struct RigIpc {
    m: Machine,
    ep_s: SlotRef,
    rep_s: SlotRef,
    client: ObjId,
    server: ObjId,
}

impl RigIpc {
    fn new() -> Self {
        let mut m = Machine::new();
        let c0 = m.add_cnode(32);
        let ep = m.add_kernel_object(ObjType::Endpoint);
        let ep_cap = m.root_cap(ep);
        m.put_cap(c0, 0, ep_cap).expect("cap Endpoint");
        let rep = m.add_kernel_object(ObjType::Reply);
        let rep_cap = m.root_cap(rep);
        m.put_cap(c0, 1, rep_cap).expect("cap Reply");
        let mut tcb = [0u32; 2];
        for (i, obj) in tcb.iter_mut().enumerate() {
            *obj = m.add_kernel_object(ObjType::Tcb);
            let cap = m.root_cap(*obj);
            m.put_cap(c0, 10 + i, cap).expect("cap TCB");
            m.transition_tcb(*obj, TcbState::Ready).expect("Ready");
        }
        Self {
            m,
            ep_s: slot(c0, 0),
            rep_s: slot(c0, 1),
            client: tcb[0],
            server: tcb[1],
        }
    }

    /// Chạy các bước theo **đúng thứ tự trong tệp**; trả số bước đã khớp.
    fn chay(&mut self, ten: &str, steps: &[Buoc]) -> usize {
        let mut khop = 0;
        for (i, b) in steps.iter().enumerate() {
            let kq = match b.op.as_str() {
                "call" => {
                    let cap = b.param("cap").expect("call cần cap");
                    assert_eq!(cap, "ep0", "{ten} bước {i}: runner chỉ dựng ep0 cho call");
                    self.m.call_msg(self.client, self.ep_s, message_tu_buoc(b))
                }
                "recv" => {
                    let cap = b.param("cap").expect("recv cần cap");
                    assert_eq!(cap, "ep1", "{ten} bước {i}: runner chỉ dựng ep1 cho recv");
                    // Quy ước fixture: `recv` không nêu `reply=` ⇒ cấp `rep0`.
                    self.m.recv(self.server, self.ep_s, Some(self.rep_s))
                }
                "send" => {
                    let cap = b.param("cap").expect("send cần cap");
                    match cap {
                        "rep0" => self.m.send(self.server, self.rep_s, message_tu_buoc(b)),
                        "ep0" | "ep1" => self.m.send(self.client, self.ep_s, message_tu_buoc(b)),
                        other => panic!("{ten} bước {i}: send cap {other} chưa dựng trong runner"),
                    }
                }
                other => panic!("{ten} bước {i}: op {other:?} chưa hỗ trợ trong runner"),
            };
            let got = ket_qua(kq);
            assert_eq!(
                got, b.expect,
                "{ten} bước {i} ({}): kết quả lệch so với tệp .scn",
                b.op
            );
            khop += 1;
        }
        khop
    }
}

// ---------------------------------------------------------------------------
// Rig cho 06: c0 = cap CNode (send ⇒ WrongObjectType), c1 = slot rỗng (call ⇒
// InvalidCapability).
// ---------------------------------------------------------------------------

struct RigLoi {
    m: Machine,
    t0: ObjId,
    c0_slot: SlotRef,
    c1_slot: SlotRef,
}

impl RigLoi {
    fn new() -> Self {
        let mut m = Machine::new();
        let c0 = m.add_cnode(16);
        let self_cap = m.root_cap(c0);
        m.put_cap(c0, 2, self_cap).expect("cap CNode gốc");
        let t0 = m.add_kernel_object(ObjType::Tcb);
        let cap = m.root_cap(t0);
        m.put_cap(c0, 3, cap).expect("cap TCB");
        m.transition_tcb(t0, TcbState::Ready).expect("Ready");
        Self {
            m,
            t0,
            c0_slot: slot(c0, 2),
            c1_slot: slot(c0, 5),
        }
    }

    fn chay(&mut self, steps: &[Buoc]) -> usize {
        let mut khop = 0;
        for (i, b) in steps.iter().enumerate() {
            let kq = match b.op.as_str() {
                "call" => {
                    let cap = b.param("cap").expect("call cần cap");
                    assert_eq!(cap, "c1", "06 bước {i}: kịch bản gọi cap c1 (rỗng)");
                    self.m.call_msg(self.t0, self.c1_slot, message_tu_buoc(b))
                }
                "send" => {
                    let cap = b.param("cap").expect("send cần cap");
                    assert_eq!(cap, "c0", "06 bước {i}: kịch bản gửi cap c0 (CNode)");
                    self.m.send(self.t0, self.c0_slot, message_tu_buoc(b))
                }
                other => panic!("06 bước {i}: op {other:?} chưa hỗ trợ"),
            };
            assert_eq!(
                ket_qua(kq),
                b.expect,
                "06 bước {i} ({}): kết quả lệch so với tệp .scn",
                b.op
            );
            assert_eq!(
                self.m.tcb_state(self.t0),
                Ok(TcbState::Ready),
                "06 bước {i}: Call/Send lỗi không được đổi TcbState"
            );
            khop += 1;
        }
        khop
    }
}

// ===========================================================================
// 1. Runner đọc tệp `.scn` 04, 06, 17 và chạy — 0 lệch.
// ===========================================================================

#[test]
fn runner_scn_04_06_17_khong_lech() {
    let s04 = doc_buoc("04-call-reply-basic.scn");
    assert_eq!(s04.len(), 3, "04: call + recv + send");
    assert_eq!((s04[0].op.as_str(), s04[0].expect.as_str()), ("call", "ok"));
    let mut r = RigIpc::new();
    assert_eq!(r.chay("04", &s04), 3);
    assert_eq!(
        r.m.reply_caller(r.m.cap_obj_at(r.rep_s.cnode, r.rep_s.slot).expect("rep")),
        Ok(None),
        "04: Reply đã được gửi ⇒ không còn caller"
    );

    let s17 = doc_buoc("17-call-no-reply-bit.scn");
    assert_eq!(s17.len(), 2, "17: call no_reply + recv");
    assert!(s17[0].param("no_reply") == Some("1"));
    let mut r = RigIpc::new();
    assert_eq!(r.chay("17", &s17), 2);
    // Bên nhận **thấy** cờ no_reply của Caller.
    assert!(
        r.m.tcb_ipc_info(r.server)
            .expect("ipc")
            .expect("đã nhận")
            .no_reply(),
        "17: server phải thấy no_reply=1"
    );
    assert_eq!(
        r.m.tcb_state(r.client),
        Ok(TcbState::Ready),
        "17: call no_reply=1 không chờ Reply"
    );

    let s06 = doc_buoc("06-call-invalid-cap.scn");
    assert_eq!(s06.len(), 2, "06: call + send");
    let mut r = RigLoi::new();
    assert_eq!(r.chay(&s06), 2);
    assert_eq!(r.m.ipc_sent(), 0, "06: không message nào được nhận");
}

// ===========================================================================
// 2. `Step::{Send, NBSend, NBRecv}` + `Msg` của `scenario.rs` chạy qua `run_sequential`.
// ===========================================================================

#[test]
fn scenario_step_send_nbsend_nbrecv() {
    let mut m = Machine::new();
    let c0 = m.add_cnode(32);
    let ep = m.add_kernel_object(ObjType::Endpoint);
    let ep_cap = m.root_cap(ep);
    m.put_cap(c0, 0, ep_cap).expect("cap Endpoint");
    let mut tcb = [0u32; 2];
    for (i, obj) in tcb.iter_mut().enumerate() {
        *obj = m.add_kernel_object(ObjType::Tcb);
        let cap = m.root_cap(*obj);
        m.put_cap(c0, 10 + i, cap).expect("cap TCB");
        m.transition_tcb(*obj, TcbState::Ready).expect("Ready");
    }
    let ep_s = slot(c0, 0);
    let client = tcb[0];
    let server = tcb[1];
    let info = MessageInfo::new(9, 0, false, 0, 2).expect("info");
    let msg = Msg::new(info, &[0xAA, 0xBB]).expect("Msg");
    let steps = vec![
        ScenarioStep {
            thread: "client",
            step: Step::Send {
                tcb: client,
                cap: ep_s,
                msg,
            },
            expect: Ok(()),
            expect_state: Some(TcbState::BlockedOnSend {
                ep,
                call: false,
                fault: None,
            }),
        },
        ScenarioStep {
            thread: "server",
            step: Step::NBRecv {
                tcb: server,
                cap: ep_s,
            },
            expect: Ok(()),
            expect_state: Some(TcbState::Ready),
        },
        ScenarioStep {
            thread: "client",
            step: Step::NBSend {
                tcb: client,
                cap: ep_s,
                msg,
            },
            expect: Ok(()),
            expect_state: Some(TcbState::Ready),
        },
        ScenarioStep {
            thread: "server",
            step: Step::NBRecv {
                tcb: server,
                cap: ep_s,
            },
            expect: Ok(()),
            expect_state: Some(TcbState::Ready),
        },
    ];
    let report = run_sequential(&mut m, &steps).expect("scenario Step mới phải chạy được");
    assert_eq!(report.events, 4);
    assert_eq!(report.checked_bytes, 4);
    assert_eq!(m.ipc_sent(), 2);
    assert_eq!(m.ipc_delivered(), 2);
    assert_eq!(m.ipc_pending(), 0);
    assert_eq!(m.transfer_calls(), 2);
}
