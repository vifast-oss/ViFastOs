// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-09 — end-to-end trên sim: Retype TCB/SchedContext/Endpoint/Reply →
//! `SchedControl.Configure` → `Call`/`Reply`, kèm C5 (D2-41): `expect_states` là
//! **hậu điều kiện sau mỗi bước**, kiểm ngay khi bước trả về; với `exit` kiểm
//! ngay sau `halt`, **trước** khi procsrv `Recv`.
//!
//! Fixture âm kiểm **đúng thông điệp lỗi**, không chỉ "có lỗi".

use vifast_sim::kobj::{FaultKind, KError, Machine, ObjType, SlotRef, TcbState};
use vifast_sim::scenario::{run_sequential, ScenarioStep, Step};

fn slot(cnode: u32, slot: usize) -> SlotRef {
    SlotRef::new(cnode, slot)
}

struct Rig {
    m: Machine,
    c0: u32,
    u0: u32,
    client: u32,
    server: u32,
    ep: u32,
    rep: u32,
}

/// Dựng đủ object cho kịch bản: TCB client/server, SchedContext, Endpoint, Reply,
/// tất cả **cấp từ một Untyped** qua Retype (D2-14: object nằm trong Untyped).
fn rig() -> Rig {
    let mut m = Machine::new();
    let c0 = m.add_cnode(16);
    // Cap tới chính CNode gốc (harness cấp) để `Tcb.Configure cspace=` có cap hợp lệ.
    let c0_cap = m.root_cap(c0);
    m.put_cap(c0, 7, c0_cap).expect("cap CNode gốc");
    let u0 = m.add_untyped(0x1000, 65536);
    let cap = m.root_cap(u0);
    m.put_cap(c0, 0, cap).expect("cap Untyped");
    let sc_ctl = m.add_kernel_object(ObjType::SchedControl);
    let cap = m.root_cap(sc_ctl);
    m.put_cap(c0, 1, cap).expect("cap SchedControl");

    let src = slot(c0, 0);
    let client_cap = m
        .retype(src, ObjType::Tcb, 0, 0)
        .expect("Retype TCB client");
    m.put_cap(c0, 2, client_cap).expect("slot client");
    let server_cap = m
        .retype(src, ObjType::Tcb, 0, 0)
        .expect("Retype TCB server");
    m.put_cap(c0, 3, server_cap).expect("slot server");
    let sc_cap = m
        .retype(src, ObjType::SchedContext, 0, 0)
        .expect("Retype SchedContext");
    m.put_cap(c0, 4, sc_cap).expect("slot schedcontext");
    let ep_cap = m
        .retype(src, ObjType::Endpoint, 0, 0)
        .expect("Retype Endpoint");
    m.put_cap(c0, 5, ep_cap).expect("slot endpoint");
    let rep_cap = m.retype(src, ObjType::Reply, 0, 0).expect("Retype Reply");
    m.put_cap(c0, 6, rep_cap).expect("slot reply");

    let client = m.cap_obj(client_cap).expect("client");
    let server = m.cap_obj(server_cap).expect("server");
    let ep = m.cap_obj(ep_cap).expect("ep");
    let rep = m.cap_obj(rep_cap).expect("rep");
    Rig {
        m,
        c0,
        u0,
        client,
        server,
        ep,
        rep,
    }
}

/// Kịch bản end-to-end đầy đủ: cấu hình → Call (chặn) → Recv → Reply → Ready.
fn e2e_steps(r: &Rig) -> Vec<ScenarioStep> {
    let c = r.c0;
    vec![
        ScenarioStep {
            thread: "client",
            step: Step::TcbConfigure {
                tcb: slot(c, 2),
                cspace: Some(slot(c, 7)),
            },
            expect: Ok(()),
            expect_state: Some(TcbState::Inactive),
        },
        ScenarioStep {
            thread: "server",
            step: Step::TcbConfigure {
                tcb: slot(c, 3),
                cspace: Some(slot(c, 7)),
            },
            expect: Ok(()),
            expect_state: Some(TcbState::Inactive),
        },
        ScenarioStep {
            thread: "init",
            step: Step::SchedControlConfigure {
                sched_control: slot(c, 1),
                sched: slot(c, 4),
                budget: 1000,
                period: 10_000,
            },
            expect: Ok(()),
            expect_state: None,
        },
        ScenarioStep {
            thread: "server",
            step: Step::SchedContextBind {
                sched: slot(c, 4),
                target: slot(c, 6),
            },
            expect: Ok(()),
            expect_state: None,
        },
        ScenarioStep {
            thread: "client",
            step: Step::Call {
                tcb: r.client,
                ep: slot(c, 5),
                mr: 42,
            },
            expect: Ok(()),
            expect_state: Some(TcbState::BlockedOnSend {
                ep: r.ep,
                call: true,
                fault: None,
            }),
        },
        ScenarioStep {
            thread: "server",
            step: Step::Recv {
                tcb: r.server,
                ep: slot(c, 5),
                reply: Some(slot(c, 6)),
            },
            expect: Ok(()),
            expect_state: Some(TcbState::Ready),
        },
        // C5: sau `Recv`, người gọi đã chuyển sang chờ Reply — kiểm ngay sau bước.
        ScenarioStep {
            thread: "client",
            step: Step::TcbReadState { tcb: slot(c, 2) },
            expect: Ok(()),
            expect_state: Some(TcbState::BlockedOnReply { reply: r.rep }),
        },
        ScenarioStep {
            thread: "server",
            step: Step::Reply {
                tcb: r.server,
                reply: slot(c, 6),
            },
            expect: Ok(()),
            expect_state: Some(TcbState::Ready),
        },
        ScenarioStep {
            thread: "client",
            step: Step::TcbReadState { tcb: slot(c, 2) },
            expect: Ok(()),
            expect_state: Some(TcbState::Ready),
        },
    ]
}

#[test]
fn e2e_retype_configure_call_reply() {
    let mut r = rig();
    let steps = e2e_steps(&r);
    let report = run_sequential(&mut r.m, &steps).expect("kịch bản end-to-end phải chạy");
    assert_eq!(report.events, steps.len(), "mỗi bước đúng một event");
    assert_eq!(r.m.tcb_state(r.client), Ok(TcbState::Ready));
    assert_eq!(r.m.tcb_state(r.server), Ok(TcbState::Ready));
    // 2 TCB 512 + SchedContext 256 + Endpoint 64 + Reply 64.
    assert_eq!(r.m.untyped_used(r.u0), 1408);
    assert_eq!(r.m.untyped_free(r.u0), 65536 - 1408);
    r.m.check_bytes(r.u0).expect("bất biến tổng byte");
    assert!(report.trace.iter().any(|l| l.contains("op=call")));
    assert!(report.trace.iter().any(|l| l.contains("op=reply")));
}

#[test]
fn cung_kich_ban_cung_trace() {
    let mut a = rig();
    let mut b = rig();
    let sa = e2e_steps(&a);
    let sb = e2e_steps(&b);
    let ra = run_sequential(&mut a.m, &sa).expect("lần 1");
    let rb = run_sequential(&mut b.m, &sb).expect("lần 2");
    assert_eq!(ra.trace, rb.trace, "cùng kịch bản ⇒ cùng trace");
}

#[test]
fn expect_states_lech_tra_dung_thong_diep() {
    let mut r = rig();
    let steps = vec![ScenarioStep {
        thread: "client",
        step: Step::Call {
            tcb: r.client,
            ep: slot(r.c0, 5),
            mr: 1,
        },
        expect: Ok(()),
        expect_state: Some(TcbState::Ready), // cố ý sai: Call chưa có người nhận ⇒ chặn
    }];
    let err = run_sequential(&mut r.m, &steps).expect_err("phải báo lệch trạng thái");
    assert_eq!(
        err.to_string(),
        format!(
            "bước 0 (client): expect_states lệch — mong đợi Ready, thực tế BlockedOnSend{{ep={}, call=true, fault=none}}",
            r.ep
        ),
        "thông điệp lỗi phải nêu bước, thread, mong đợi và thực tế"
    );
}

#[test]
fn ket_qua_lech_tra_dung_thong_diep() {
    let mut r = rig();
    let steps = vec![ScenarioStep {
        thread: "client",
        step: Step::Delete {
            slot: slot(r.c0, 0),
        },
        expect: Err(KError::SlotEmpty), // cố ý sai: slot 0 đang giữ cap Untyped
        expect_state: None,
    }];
    let err = run_sequential(&mut r.m, &steps).expect_err("phải báo lệch kết quả");
    assert_eq!(
        err.to_string(),
        "bước 0 (client): kết quả lệch — mong đợi SlotEmpty, thực tế ok"
    );
}

/// C5 (D2-41): `exit` là `BlockedOnSend{fault_ep, call=false, fault=Exit{code}}` được
/// kiểm **ngay sau halt**, trước khi procsrv `Recv` chuyển TCB sang `Inactive`.
#[test]
fn exit_kiem_ngay_sau_halt_truoc_procsrv_recv() {
    let mut r = rig();
    let c = r.c0;
    let steps = vec![
        ScenarioStep {
            thread: "app",
            step: Step::TcbSetFaultEp {
                tcb: slot(c, 2),
                ep: slot(c, 5),
            },
            expect: Ok(()),
            expect_state: Some(TcbState::Inactive),
        },
        ScenarioStep {
            thread: "app",
            step: Step::Exit {
                tcb: r.client,
                code: 7,
            },
            expect: Ok(()),
            expect_state: Some(TcbState::BlockedOnSend {
                ep: r.ep,
                call: false,
                fault: Some(FaultKind::Exit { code: 7 }),
            }),
        },
        ScenarioStep {
            thread: "procsrv",
            step: Step::Recv {
                tcb: r.server,
                ep: slot(c, 5),
                reply: None,
            },
            expect: Ok(()),
            expect_state: Some(TcbState::Ready),
        },
        ScenarioStep {
            thread: "app",
            step: Step::TcbReadState { tcb: slot(c, 2) },
            expect: Ok(()),
            expect_state: Some(TcbState::Inactive),
        },
    ];
    run_sequential(&mut r.m, &steps).expect("exit phải kiểm trước khi procsrv Recv");
    assert_eq!(r.m.tcb_state(r.client), Ok(TcbState::Inactive));
}

/// Cùng kịch bản nhưng kỳ vọng `Inactive` **ngay sau halt** — nếu bộ chạy kiểm sau
/// `Recv` thì sẽ không bắt được; thông điệp phải nêu `BlockedOnSend{... fault=Exit{7}}`.
#[test]
fn exit_ky_vong_sai_bi_bat_truoc_procsrv_recv() {
    let mut r = rig();
    let c = r.c0;
    let steps = vec![
        ScenarioStep {
            thread: "app",
            step: Step::TcbSetFaultEp {
                tcb: slot(c, 2),
                ep: slot(c, 5),
            },
            expect: Ok(()),
            expect_state: None,
        },
        ScenarioStep {
            thread: "app",
            step: Step::Exit {
                tcb: r.client,
                code: 7,
            },
            expect: Ok(()),
            expect_state: Some(TcbState::Inactive),
        },
        ScenarioStep {
            thread: "procsrv",
            step: Step::Recv {
                tcb: r.server,
                ep: slot(c, 5),
                reply: None,
            },
            expect: Ok(()),
            expect_state: None,
        },
    ];
    let err = run_sequential(&mut r.m, &steps).expect_err("phải bắt lệch ngay sau halt");
    assert_eq!(
        err.to_string(),
        format!(
            "bước 1 (app): expect_states lệch — mong đợi Inactive, thực tế BlockedOnSend{{ep={}, call=false, fault=Exit{{7}}}}",
            r.ep
        )
    );
}

/// Không có `fault_ep` ⇒ `Exit` chuyển `Inactive` ngay (QD §1 Q-A).
#[test]
fn exit_khong_fault_ep_thanh_inactive() {
    let mut r = rig();
    let steps = vec![ScenarioStep {
        thread: "app",
        step: Step::Exit {
            tcb: r.client,
            code: 3,
        },
        expect: Ok(()),
        expect_state: Some(TcbState::Inactive),
    }];
    run_sequential(&mut r.m, &steps).expect("exit không fault_ep");
    assert_eq!(r.m.tcb_state(r.client), Ok(TcbState::Inactive));
}
