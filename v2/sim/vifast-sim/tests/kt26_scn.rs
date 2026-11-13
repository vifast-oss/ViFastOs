// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-26 — **runner `.scn`** cho Exit/fault: đọc **(không sửa)** ba tệp
//! `v2/test/scn/{13-exit-no-fault-ep,14-exit-with-fault-ep,20-exit-procsrv-chua-recv}.scn`,
//! chạy đúng chuỗi bước của từng tệp trên mô hình KT-26 và so **kết quả từng bước** với kỳ vọng
//! ghi trong tệp (`=> ok` / `=> halt`), 0 lệch (K1C-PLAN §5 KT-26 DoD 6).
//!
//! Quy ước (đọc cùng `kt24_scn.rs`/`kt25_scn.rs` + **D2-54**):
//!
//! 1. `ep0` (bên nhận fault) và `ep1` (cap procsrv `Recv`) trỏ **cùng một** Endpoint;
//!    `procsrv` là thread thứ hai, `recv` không nêu `reply=` ⇒ runner cấp `rep0` làm Reply
//!    window (fault `Exit` không có Reply nhưng `Recv` vẫn hợp lệ).
//! 2. `expect_states` là hậu điều kiện **sau bước `exit`** (D2-41: kiểm ngay sau `halt`,
//!    **trước** khi procsrv `Recv` xử lý xong) — runner so cả tên trạng thái và trường
//!    (`call=false`, `fault=Exit{code}`), `.scn` 20 phủ đúng ca "procsrv chưa Recv".
//! 3. `exit ... => halt` là kỳ vọng của syscall `Exit` (không phải `=> ok`): runner ánh xạ
//!    `Ok(())` → `halt`, lỗi → tên mã lỗi.
//!
//! Tệp này **đọc đĩa** nên **không** chạy dưới Miri (isolation); Miri của KT-26 chạy
//! `tests/kt26_fault.rs` (thuần mô hình) — cùng quy ước `kt24_scn.rs`/`kt25_scn.rs`.

use std::fs;
use std::path::{Path, PathBuf};

use vifast_sim::kobj::{FaultKind, KError, Machine, ObjId, ObjType, SlotRef, TcbState};

fn slot(cnode: ObjId, s: usize) -> SlotRef {
    SlotRef::new(cnode, s)
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

impl Buoc {
    fn param(&self, k: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(key, _)| key == k)
            .map(|(_, v)| v.as_str())
    }
}

fn doc(ten: &str) -> (Vec<Buoc>, Option<String>) {
    let path = scn_path(ten);
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("không đọc được {}: {e}", path.display()));
    assert!(text.contains("version = 1"), "{ten}: thiếu version = 1");

    // `expect_states = ["..."]` (0 hoặc 1 dòng) — giữ nguyên chuỗi trạng thái của tệp.
    let expect_states = text
        .lines()
        .find(|l| l.trim_start().starts_with("expect_states"))
        .map(|l| {
            let start = l
                .find('"')
                .unwrap_or_else(|| panic!("{ten}: expect_states thiếu chuỗi"));
            let rest = &l[start + 1..];
            let end = rest.find('"').expect("expect_states thiếu dấu đóng");
            rest[..end].to_string()
        });

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
                // `invoke Tcb.Configure ...`: token thứ hai là **tên method**, không phải k=v.
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
    (steps, expect_states)
}

fn ten_ket_qua(r: Result<(), KError>) -> String {
    match r {
        Ok(()) => "ok".to_string(),
        Err(e) => e.name().to_string(),
    }
}

/// `expect_states` của `.scn` 20 → `TcbState` (chỉ hỗ trợ dạng dùng trong 3 tệp này).
fn parse_state(ten: &str, s: &str, ep: ObjId) -> TcbState {
    let (name, body) = match s.split_once('{') {
        Some((n, rest)) => (n.trim(), rest.rsplit_once('}').expect("thiếu }").0),
        None => (s.trim(), ""),
    };
    assert_eq!(
        name, "BlockedOnSend",
        "{ten}: expect_states {name:?} chưa hỗ trợ"
    );
    let mut call = None;
    let mut fault = None;
    for field in body.split(',') {
        let field = field.trim();
        if field.is_empty() {
            continue;
        }
        // Trường trần (`ep`) không có giá trị: bỏ qua (đúng Endpoint của rig).
        let Some((k, v)) = field.split_once('=') else {
            assert_eq!(field, "ep", "{ten}: trường expect_states lạ: {field:?}");
            continue;
        };
        match k.trim() {
            "ep" => {} // `ep` trần = đúng Endpoint của rig
            "call" => call = Some(v.trim() == "true"),
            "fault" => {
                let code = v
                    .trim()
                    .strip_prefix("Exit{")
                    .and_then(|r| r.strip_suffix('}'))
                    .and_then(|c| c.parse::<u64>().ok())
                    .unwrap_or_else(|| panic!("{ten}: fault {v:?} chưa hỗ trợ"));
                fault = Some(FaultKind::Exit { code });
            }
            other => panic!("{ten}: trường {other:?} chưa hỗ trợ"),
        }
    }
    TcbState::BlockedOnSend {
        ep,
        call: call.expect("thiếu call="),
        fault,
    }
}

struct Rig {
    m: Machine,
    ep: ObjId,
    ep_s: SlotRef,
    rep_s: SlotRef,
    app: ObjId,
    app_s: SlotRef,
    procsrv: ObjId,
}

impl Rig {
    fn new() -> Self {
        let mut m = Machine::new();
        let c0 = m.add_cnode(32);
        let ep = m.add_kernel_object(ObjType::Endpoint);
        let ep_cap = m.root_cap(ep);
        m.put_cap(c0, 0, ep_cap).expect("cap Endpoint (ep0/ep1)");
        let rep = m.add_kernel_object(ObjType::Reply);
        let rep_cap = m.root_cap(rep);
        m.put_cap(c0, 1, rep_cap).expect("cap Reply (rep0)");
        let mut tcb = [0u32; 2];
        for (i, obj) in tcb.iter_mut().enumerate() {
            *obj = m.add_kernel_object(ObjType::Tcb);
            let cap = m.root_cap(*obj);
            m.put_cap(c0, 10 + i, cap).expect("cap TCB");
            m.transition_tcb(*obj, TcbState::Ready).expect("Ready");
        }
        Self {
            m,
            ep,
            ep_s: slot(c0, 0),
            rep_s: slot(c0, 1),
            app: tcb[0],
            app_s: slot(c0, 10),
            procsrv: tcb[1],
        }
    }

    fn kiem(&self, ten: &str, i: usize) {
        self.m
            .check_sched()
            .unwrap_or_else(|e| panic!("{ten} bước {i}: check_sched: {e}"));
        self.m
            .check_ipc()
            .unwrap_or_else(|e| panic!("{ten} bước {i}: check_ipc: {e}"));
        self.m
            .check_mailboxes()
            .unwrap_or_else(|e| panic!("{ten} bước {i}: check_mailboxes: {e}"));
    }

    /// Chạy đúng thứ tự bước trong tệp, kiểm kết quả từng bước + `expect_states` sau `exit`.
    fn chay(&mut self, ten: &str, steps: &[Buoc], expect_states: Option<&str>) -> usize {
        assert!(!steps.is_empty(), "{ten}: rỗng");
        let mut kiem_state_chua = expect_states.is_some();
        for (i, b) in steps.iter().enumerate() {
            let kq: Result<(), KError> = match b.op.as_str() {
                "invoke" => match b.param("invoke") {
                    Some("Tcb.Configure") => {
                        assert_eq!(b.param("tcb"), Some("tcb0"), "{ten} bước {i}");
                        self.m.tcb_configure(self.app_s, None)
                    }
                    Some("Tcb.SetFaultEp") => {
                        assert_eq!(b.param("tcb"), Some("tcb0"), "{ten} bước {i}");
                        assert_eq!(
                            b.param("cap"),
                            Some("ep0"),
                            "{ten} bước {i}: fault_ep phải là ep0"
                        );
                        self.m.tcb_set_fault_ep(self.app_s, self.ep_s)
                    }
                    other => panic!("{ten} bước {i}: invoke {other:?} chưa hỗ trợ"),
                },
                "exit" => {
                    let code: u64 = b
                        .param("code")
                        .unwrap_or_else(|| panic!("{ten} bước {i}: exit thiếu code"))
                        .parse()
                        .expect("code u64");
                    self.m.exit(self.app, code)
                }
                "recv" => {
                    assert_eq!(b.param("cap"), Some("ep1"), "{ten} bước {i}");
                    self.m.recv(self.procsrv, self.ep_s, Some(self.rep_s))
                }
                other => panic!("{ten} bước {i}: op {other:?} chưa hỗ trợ"),
            };
            let got = if b.op == "exit" {
                // `.scn` ghi `=> halt` cho syscall Exit.
                match kq {
                    Ok(()) => "halt".to_string(),
                    Err(e) => e.name().to_string(),
                }
            } else {
                ten_ket_qua(kq)
            };
            assert_eq!(
                got, b.expect,
                "{ten} bước {i} ({}): kết quả lệch so với tệp .scn",
                b.op
            );
            if b.op == "exit" {
                if let Some(want) = expect_states {
                    let want = parse_state(ten, want, self.ep);
                    let actual = self.m.tcb_state(self.app).expect("tcb_state");
                    assert_eq!(
                        actual, want,
                        "{ten}: expect_states phải khớp **ngay sau** exit, trước procsrv Recv (D2-41)"
                    );
                    kiem_state_chua = false;
                }
            }
            self.kiem(ten, i);
        }
        assert!(
            !kiem_state_chua,
            "{ten}: tệp có expect_states nhưng không có bước exit để kiểm"
        );
        steps.len()
    }
}

// ===========================================================================
// 1. Runner `.scn` 13/14/20 — đọc tệp (**không sửa**) và chạy, 0 lệch (DoD 6).
// ===========================================================================

#[test]
fn runner_scn_13_14_20_khong_lech() {
    // 13 — `halt` **không** fault_ep ⇒ TCB `Inactive` ngay.
    let (s13, st13) = doc("13-exit-no-fault-ep.scn");
    assert_eq!(s13.len(), 2, "13: Configure + exit");
    assert_eq!(st13, None, "13 không có expect_states");
    let mut r = Rig::new();
    assert_eq!(r.chay("13", &s13, st13.as_deref()), 2);
    assert_eq!(
        r.m.tcb_state(r.app),
        Ok(TcbState::Inactive),
        "13: exit không fault_ep ⇒ Inactive ngay"
    );
    assert_eq!(
        r.m.tcb_read_state(r.app_s).expect("read").exit_code,
        Some(0)
    );

    // 14 — `SetFaultEp` rồi `exit` ⇒ TCB `BlockedOnSend`; procsrv `Recv` `ok` ⇒ `Inactive`.
    let (s14, st14) = doc("14-exit-with-fault-ep.scn");
    assert_eq!(s14.len(), 3, "14: SetFaultEp + exit + recv");
    assert_eq!(st14, None);
    let mut r = Rig::new();
    assert_eq!(r.chay("14", &s14, st14.as_deref()), 3);
    assert_eq!(
        r.m.tcb_state(r.app),
        Ok(TcbState::Inactive),
        "14: procsrv Recv xong ⇒ Inactive"
    );
    assert_eq!(
        r.m.tcb_read_state(r.app_s).expect("read").exit_code,
        Some(3)
    );

    // 20 — expect_states `BlockedOnSend{ep, call=false, fault=Exit{7}}` khớp **trước** Recv.
    let (s20, st20) = doc("20-exit-procsrv-chua-recv.scn");
    assert_eq!(s20.len(), 3, "20: SetFaultEp + exit + recv");
    let st20 = st20.expect("20 phải có expect_states");
    assert!(
        st20.contains("call=false") && st20.contains("fault=Exit{7}"),
        "20: expect_states phải khớp cả trường (D2-41): {st20}"
    );
    let mut r = Rig::new();
    assert_eq!(r.chay("20", &s20, Some(&st20)), 3);
    assert_eq!(
        r.m.tcb_state(r.app),
        Ok(TcbState::Inactive),
        "20: sau Recv của procsrv ⇒ Inactive"
    );
    assert_eq!(
        r.m.tcb_read_state(r.app_s).expect("read").exit_code,
        Some(7)
    );
}
