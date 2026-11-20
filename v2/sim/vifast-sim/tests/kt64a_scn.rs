// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-64a** — runner `.scn` cho **anti-hang** khi finalise IPC (D2-89 + D2-90): đọc
//! **(không sửa)** tệp `v2/test/scn/27-finalise-ba-loai-cho.scn`, chạy đúng **4 bước** của tệp
//! trên mô hình K1–K2 và so **kết quả từng bước** với kỳ vọng ghi trong tệp (`=> ok`).
//!
//! Đây là ca **đỏ-trước** (D2-05) của KT-64a: trên sim **cũ** (trước bản sửa), sau bước
//! `invoke CNode.Revoke` cả ba TCB vẫn nằm trên object **đã chết**
//! (`BlockedOnRecv{ep chết}`, `BlockedOnNotification{ntfn chết}`, `BlockedOnSend{ep chết}`) nên
//! test đỏ ở chính 3 assert trạng thái — log `JOURNAL/kt64a-red-scn.log`. Sau khi sim sửa theo
//! D2-89/D2-90: cả ba `Ready`, `last_ipc_error = Cancelled`, hàng gửi bị huỷ **có đếm**
//! (`ipc_dropped` +1), bất biến `check_*` xanh sau **mỗi** bước.
//!
//! Quy ước (đọc cùng `kt24_scn.rs`/`kt25_scn.rs`/`kt27_scn.rs`):
//!
//! 1. `u1` (Untyped trong `c1`) chứa **cả ba** object có người chờ: `ep0`/`ntf0` (đang có
//!    `Recv`/`Wait`) và `ep1` (có `Send` đang xếp hàng) ⇒ `Revoke` Untyped đó finalise **đúng**
//!    ba object, không đụng object ngoài cây (`u0`, TCB kernel, CNode).
//! 2. `send cap=ep1 => ok` = **Send chặn** (Endpoint chưa có waiter) ⇒ TCB `BlockedOnSend` +
//!    message vào hàng; `recv cap=ntf0` = **Wait** rỗng ⇒ `BlockedOnNotification`.
//! 3. `expect_states` của tệp (3 × `Ready`) là **hậu điều kiện** kiểm ngay sau `Revoke`.
//!
//! Tệp này **đọc đĩa** nên **không** chạy dưới Miri (cùng quy ước `kt24_scn.rs`/`kt27_scn.rs`).

use std::fs;
use std::path::{Path, PathBuf};

use vifast_sim::ipc::{Message, MessageInfo};
use vifast_sim::kobj::{KError, Machine, ObjId, ObjType, SlotRef, TcbState};

fn slot(cnode: ObjId, s: usize) -> SlotRef {
    SlotRef::new(cnode, s)
}

fn msg_empty() -> Message {
    Message::new(MessageInfo::empty(), &[]).expect("Message rỗng hợp lệ")
}

fn scn_path(ten: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../test/scn")
        .join(ten)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Buoc {
    op: String,
    invoke: Option<String>,
    params: Vec<(String, String)>,
    expect: String,
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
        let first = toks.next().expect("bước rỗng").to_string();
        let (op, invoke) = if first == "invoke" {
            (
                "invoke".to_string(),
                Some(toks.next().expect("invoke thiếu tên").to_string()),
            )
        } else {
            (first, None)
        };
        let mut params = Vec::new();
        for tok in toks {
            match tok.split_once('=') {
                Some((k, v)) => params.push((k.to_string(), v.to_string())),
                None => panic!("{ten}: tham số không phải k=v: {tok:?}"),
            }
        }
        steps.push(Buoc {
            op,
            invoke,
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

const U1: usize = 0;
const EP0: usize = 1;
const EP1: usize = 2;
const NTF0: usize = 3;

/// Rig `.scn` 27: CNode gốc `c0` + CNode `c1` giữ Untyped `u1` và **ba** object bị finalise.
struct Rig {
    m: Machine,
    c1: ObjId,
    /// `SlotRef` của `u1`/`ep0`/`ep1`/`ntf0` (đúng thứ tự `caps` trong tệp).
    s: [SlotRef; 4],
    /// `ObjId` của `u1`/`ep0`/`ep1`/`ntf0` — đọc **trước** Revoke để assert sau khi cap chết.
    obj: [ObjId; 4],
    t: [ObjId; 3],
}

impl Rig {
    fn slot_ten(&self, ten: &str) -> SlotRef {
        match ten {
            "u1" => self.s[U1],
            "ep0" => self.s[EP0],
            "ep1" => self.s[EP1],
            "ntf0" => self.s[NTF0],
            other => panic!("cap lạ trong tệp 27: {other:?}"),
        }
    }
}

fn dung_rig() -> Rig {
    let mut m = Machine::new();
    let c0 = m.add_cnode(64);
    let c1 = m.add_cnode(16);
    // u0: Untyped của CNode gốc (giữ rig giống `.scn`: mọi cap khai đều tồn tại).
    let u0 = m.add_untyped(0x1000_0000, 1 << 16);
    let u0c = m.root_cap(u0);
    m.put_cap(c0, 0, u0c).expect("cap u0");
    // u1: Untyped **bị Revoke** — chứa Endpoint/Endpoint/Notification.
    let u1 = m.add_untyped(0x2000_0000, 1 << 16);
    let u1c = m.root_cap(u1);
    m.put_cap(c1, U1, u1c).expect("cap u1");
    let mut s = [slot(c1, U1); 4];
    let mut obj = [0u32; 4];
    obj[U1] = u1;
    for (i, ty) in [ObjType::Endpoint, ObjType::Endpoint, ObjType::Notification]
        .iter()
        .enumerate()
    {
        let cap = m
            .retype(slot(c1, U1), *ty, 0, 0)
            .unwrap_or_else(|e| panic!("retype {ty:?}: {}", e.name()));
        let idx = U1 + 1 + i;
        let o = m.cap_info(cap).expect("cap vừa cấp").obj;
        m.put_cap(c1, idx, cap).expect("cap object trong u1");
        s[idx] = slot(c1, idx);
        obj[idx] = o;
    }
    // tcb0..tcb2: TCB **kernel object** (sống qua Revoke — không nằm trong u1).
    let mut t = [0u32; 3];
    for (i, tcb) in t.iter_mut().enumerate() {
        *tcb = m.add_kernel_object(ObjType::Tcb);
        let cap = m.root_cap(*tcb);
        m.put_cap(c0, 10 + i, cap).expect("cap tcb");
        m.start_tcb(*tcb).expect("tcb Ready");
    }
    Rig { m, c1, s, obj, t }
}

#[test]
fn runner_scn_27_finalise_ba_loai_cho() {
    let steps = doc_buoc("27-finalise-ba-loai-cho.scn");
    assert_eq!(steps.len(), 4, "tệp 27 phải có đúng 4 bước");
    let mut r = dung_rig();
    for (i, b) in steps.iter().enumerate() {
        let p = |k: &str| -> Option<&str> {
            b.params
                .iter()
                .find(|(kk, _)| kk == k)
                .map(|(_, v)| v.as_str())
        };
        let got = match b.op.as_str() {
            "recv" => {
                let cap = p("cap").unwrap_or_else(|| panic!("bước {i}: recv thiếu cap"));
                match cap {
                    "ntf0" => r.m.recv(r.t[1], r.slot_ten(cap), None),
                    "ep0" => r.m.recv(r.t[0], r.slot_ten(cap), None),
                    other => panic!("bước {i}: recv cap lạ {other}"),
                }
            }
            "send" => {
                let cap = p("cap").expect("send thiếu cap");
                r.m.send(r.t[2], r.slot_ten(cap), msg_empty())
            }
            "invoke" => {
                assert_eq!(
                    b.invoke.as_deref(),
                    Some("CNode.Revoke"),
                    "bước {i}: invoke lạ"
                );
                assert_eq!(p("cnode"), Some("c1"), "bước {i}: cnode lạ");
                let s = p("slot").expect("Revoke thiếu slot");
                r.m.revoke(r.c1, Some(r.slot_ten(s).slot)).map(|_| ())
            }
            other => panic!("bước {i}: op lạ {other}"),
        };
        assert_eq!(
            ten_ket_qua(got),
            b.expect,
            "bước {i} ({}): kết quả lệch kỳ vọng của tệp 27",
            b.op
        );
        // Hậu điều kiện **sau mỗi bước** (D2-41 / KT-27 DoD 7).
        for (ten, kq) in [
            ("check_ipc", r.m.check_ipc()),
            ("check_sched", r.m.check_sched()),
            ("check_notifications", r.m.check_notifications()),
            ("check_caps", r.m.check_caps()),
            ("check_fault", r.m.check_fault()),
            ("check_mailboxes", r.m.check_mailboxes()),
        ] {
            if let Err(e) = kq {
                panic!("bước {i} ({}): {ten}: {e}", b.op);
            }
        }
    }

    // --- Ngữ nghĩa D2-89/D2-90: **cả ba** loại chờ được đánh thức `Cancelled` + `Ready` ---
    for (i, ten) in [
        "t0 (Recv waiter)",
        "t1 (Notification waiter)",
        "t2 (sender)",
    ]
    .iter()
    .enumerate()
    {
        let st = r.m.tcb_state(r.t[i]).expect("state");
        assert_eq!(
            st,
            TcbState::Ready,
            "{ten}: phải `Ready` sau khi object bị finalise (D2-89/D2-90), đang {st:?}"
        );
        assert_eq!(
            r.m.tcb_last_ipc_error(r.t[i]).expect("last_ipc_error"),
            Some(KError::Cancelled),
            "{ten}: phải nhận `Cancelled`"
        );
    }
    // Message của sender bị huỷ **có đếm** (D2-90: `ipc_dropped`).
    assert_eq!(r.m.ipc_dropped(), 1, "message trong hàng gửi phải được đếm");
    assert_eq!(r.m.ipc_pending(), 0, "hàng gửi đã huỷ");
    // Ba object trong `u1` **đã chết** (Revoke Untyped finalise cả cây).
    for (i, ten) in ["ep0", "ep1", "ntf0"].iter().enumerate() {
        assert!(
            !r.m.object_alive(r.obj[EP0 + i]),
            "{ten}: phải chết sau Revoke Untyped"
        );
    }
    // Cap con của `u1` bị xoá theo cây (không cap nào trỏ object chết).
    for cr in r.s.iter().skip(1) {
        assert!(
            r.m.cap_at(cr.cnode, cr.slot).is_err(),
            "slot {:?} của cây u1 phải rỗng sau Revoke",
            cr
        );
    }
}
