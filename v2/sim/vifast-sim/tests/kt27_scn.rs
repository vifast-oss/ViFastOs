// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-27 — **runner `.scn`** cho Notification: đọc **(không sửa)** tệp
//! `v2/test/scn/21-notification-signal-wait.scn`, chạy đúng **4 bước** của tệp trên mô hình
//! KT-27 và so **kết quả từng bước** với kỳ vọng ghi trong tệp (`=> ok` / `=> <mã lỗi>`),
//! 0 lệch (K1C-PLAN §5 KT-27 DoD 4/8).
//!
//! Quy ước (đọc cùng `kt24_scn.rs`/`kt25_scn.rs` + **D2-54**):
//!
//! 1. `ntf0` là **một** Notification; `tcb0` là TCB `t0` (đưa lên `Ready` trước bước đầu).
//! 2. `send cap=ntf0` = **Signal** với badge của cap (cap gốc không badge ⇒ badge 0 — hợp lệ);
//!    `recv cap=ntf0` = **Wait**; `nb_recv cap=ntf0` = **Poll** (`Busy` khi rỗng/TCB không chạy).
//! 3. Tệp **không** nêu badge, nên sau `send` word vẫn 0; `recv` vì thế **chặn**
//!    `BlockedOnNotification` (trả `Ok` theo ABI) rồi `nb_recv` ⇒ `Busy`.
//!
//! Tệp này **đọc đĩa** nên **không** chạy dưới Miri (isolation); Miri của KT-27 chạy
//! `tests/kt27_notification.rs` (thuần mô hình) — cùng quy ước `kt24_scn.rs`/`kt25_scn.rs`.

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
        let op = toks.next().expect("bước rỗng").to_string();
        let mut params = Vec::new();
        for tok in toks {
            match tok.split_once('=') {
                Some((k, v)) => params.push((k.to_string(), v.to_string())),
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

/// Ô cap của rig `.scn` 21 — chỉ cần Notification + TCB (các cap khác của tệp không dùng).
const S_NTFN: usize = 0;
const S_TCB: usize = 1;

#[test]
fn runner_scn_21_notification_signal_wait() {
    let steps = doc_buoc("21-notification-signal-wait.scn");
    assert_eq!(steps.len(), 4, "tệp 21 phải có đúng 4 bước");

    let mut m = Machine::new();
    let c0 = m.add_cnode(64);
    let ntfn = m.add_kernel_object(ObjType::Notification);
    let ntfn_cap = m.root_cap(ntfn);
    m.put_cap(c0, S_NTFN, ntfn_cap).expect("cap ntf0");
    let tcb = m.add_kernel_object(ObjType::Tcb);
    let tcb_cap = m.root_cap(tcb);
    m.put_cap(c0, S_TCB, tcb_cap).expect("cap tcb0");
    m.transition_tcb(tcb, TcbState::Ready).expect("tcb0 Ready");
    let ntfn_s = slot(c0, S_NTFN);

    for (i, b) in steps.iter().enumerate() {
        assert!(
            b.params.iter().any(|(k, v)| k == "cap" && v == "ntf0"),
            "bước {i}: chỉ hỗ trợ cap=ntf0, gặp {:?}",
            b.params
        );
        let got = match b.op.as_str() {
            "nb_recv" => m.nb_recv(tcb, ntfn_s).map(|_| ()),
            "send" => m.send(tcb, ntfn_s, msg_empty()),
            "recv" => m.recv(tcb, ntfn_s, None),
            other => panic!("bước {i}: op lạ {other}"),
        };
        assert_eq!(
            ten_ket_qua(got),
            b.expect,
            "bước {i} ({}): kết quả lệch kỳ vọng của tệp 21",
            b.op
        );
        // Bất biến sau mỗi bước (KT-27 DoD 7).
        if let Err(e) = m.check_sched() {
            panic!("bước {i}: check_sched: {e}");
        }
        if let Err(e) = m.check_ipc() {
            panic!("bước {i}: check_ipc: {e}");
        }
        if let Err(e) = m.check_notifications() {
            panic!("bước {i}: check_notifications: {e}");
        }
        if let Err(e) = m.check_caps() {
            panic!("bước {i}: check_caps: {e}");
        }
    }

    // Ngữ nghĩa sau chuỗi: `send` badge 0 ⇒ word 0; `recv` chặn ⇒ tcb0
    // `BlockedOnNotification`; `nb_recv` trên TCB không chạy được ⇒ `Busy`.
    assert_eq!(m.notification_word(ntfn), Ok(0));
    assert_eq!(
        m.tcb_state(tcb),
        Ok(TcbState::BlockedOnNotification { ntfn }),
        "recv trên notification rỗng phải chặn"
    );
    assert_eq!(m.notification_waiter(ntfn), Ok(Some(tcb)));
    assert_eq!(m.tcb_notification(tcb), Ok(None), "chưa giao gì");
}
