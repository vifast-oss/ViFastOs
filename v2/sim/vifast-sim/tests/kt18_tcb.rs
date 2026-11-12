// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-18 — TCB đầy đủ, **một cửa** `transition_tcb` + bảng `ALLOWED` 11×11 (D2-49(d)),
//! `suspended` trực giao, `Suspend(Abort/Defer)`, `Resume` không phát lại, `ReadState`
//! trả **4 giá trị** (`TcbSnapshot`), finalise TCB trước khi object chết.
//!
//! Nguồn ngữ nghĩa: thiết kế §4.4 (`docs/v2/ViFastOs-v2-thiet-ke.md:256-286`), §4.5 bảng
//! ngân sách (`:302-311`), **D2-33** (enum đầy đủ + `BlockedOnRecv{ep, reply}`),
//! **D2-25** (`Exit`/`Resume` xoá `exit_code`), **D2-49(d)** (bảng `ALLOWED` model-only).
//! Bảng xuất ra [`v2/docs/k1b-tcb-transition.md`](../../../docs/k1b-tcb-transition.md) `[TK]`.

use vifast_sim::kobj::{
    BlockedOnKind, FaultKind, KError, Machine, ObjId, ObjType, SleepReason, SlotRef, SuspendMode,
    TcbSnapshot, TcbState, ALLOWED,
};

const N_STATES: usize = 11;

fn slot(cnode: ObjId, slot: usize) -> SlotRef {
    SlotRef::new(cnode, slot)
}

// Slot của CNode gốc trong `rig()` — chỉ số cố định để test đọc được.
const S_UNTYPED: usize = 0;
const S_EP: usize = 1;
const S_REP: usize = 2;
const S_NTFN: usize = 3;
const S_SC: usize = 4;
const S_CLIENT: usize = 5;
const S_SERVER: usize = 6;
const S_CNODE: usize = 7;

/// Bộ object tối thiểu cho các ca KT-18: Endpoint/Reply/Notification/SchedContext/2 TCB
/// do harness cấp + một Untyped (để ca finalise Revoke được).
struct Rig {
    m: Machine,
    c0: ObjId,
    u0: ObjId,
    ep: ObjId,
    rep: ObjId,
    ntfn: ObjId,
    sc: ObjId,
    client: ObjId,
    server: ObjId,
}

fn rig() -> Rig {
    let mut m = Machine::new();
    let c0 = m.add_cnode(16);
    let u0 = m.add_untyped(0x1000, 65536);
    let ep = m.add_kernel_object(ObjType::Endpoint);
    let rep = m.add_kernel_object(ObjType::Reply);
    let ntfn = m.add_kernel_object(ObjType::Notification);
    let sc = m.add_kernel_object(ObjType::SchedContext);
    let client = m.add_kernel_object(ObjType::Tcb);
    let server = m.add_kernel_object(ObjType::Tcb);
    for (i, obj) in [
        (S_UNTYPED, u0),
        (S_EP, ep),
        (S_REP, rep),
        (S_NTFN, ntfn),
        (S_SC, sc),
        (S_CLIENT, client),
        (S_SERVER, server),
        (S_CNODE, c0),
    ] {
        let cap = m.root_cap(obj);
        m.put_cap(c0, i, cap).expect("cap gốc vào CNode");
    }
    Rig {
        m,
        c0,
        u0,
        ep,
        rep,
        ntfn,
        sc,
        client,
        server,
    }
}

impl Rig {
    fn s(&self, index: usize) -> SlotRef {
        slot(self.c0, index)
    }
    fn client_slot(&self) -> SlotRef {
        self.s(S_CLIENT)
    }
    fn read(&self) -> TcbSnapshot {
        self.m
            .tcb_read_state(self.client_slot())
            .expect("ReadState của client")
    }
}

/// Tham số object giả cho bảng trạng thái — cổng chuyển trạng thái **không** kiểm object
/// (KT-19 mới kiểm hart/queue).
struct Ids {
    ep: ObjId,
    rep: ObjId,
    ntfn: ObjId,
    frame: ObjId,
}

fn ids() -> Ids {
    Ids {
        ep: 1,
        rep: 2,
        ntfn: 3,
        frame: 4,
    }
}

/// `TcbState` đại diện cho **chỉ số** của bảng `ALLOWED`; thứ tự khớp `vifast_abi::TCB_STATES`.
fn state_for(index: usize, ids: &Ids) -> TcbState {
    match index {
        0 => TcbState::Inactive,
        1 => TcbState::Ready,
        2 => TcbState::Running { hart: 0 },
        3 => TcbState::BlockedOnSend {
            ep: ids.ep,
            call: false,
            fault: Some(FaultKind::Exit { code: 1 }),
        },
        4 => TcbState::BlockedOnRecv {
            ep: ids.ep,
            reply: None,
        },
        5 => TcbState::BlockedOnReply { reply: ids.rep },
        6 => TcbState::BlockedOnNotification { ntfn: ids.ntfn },
        7 => TcbState::BlockedOnAddress {
            frame: ids.frame,
            offset: 16,
        },
        8 => TcbState::Sleeping {
            reason: SleepReason::Budget,
        },
        9 => TcbState::Stopping,
        10 => TcbState::Dead,
        _ => panic!("chỉ số trạng thái ngoài 0..11"),
    }
}

/// Đưa một TCB **mới** từ `Inactive` tới `index` chỉ bằng các cặp `ALLOWED` (đường dẫn của
/// bảng, không có cửa sau). `index = 10` đi thẳng `Inactive → Dead` (finalise).
fn drive_to(m: &mut Machine, tcb: ObjId, index: usize, ids: &Ids) {
    let path: &[usize] = match index {
        0 => &[],
        1 => &[1],
        2 => &[1, 2],
        3 => &[1, 2, 3],
        4 => &[1, 2, 4],
        5 => &[1, 2, 5],
        6 => &[1, 2, 6],
        7 => &[1, 2, 7],
        8 => &[1, 2, 8],
        9 => &[1, 9],
        10 => &[10],
        _ => panic!("chỉ số trạng thái ngoài 0..11"),
    };
    for step in path {
        m.transition_tcb(tcb, state_for(*step, ids))
            .expect("đường dẫn trạng thái của bảng phải ALLOWED");
    }
}

// ---------------------------------------------------------------------------
// 1. TcbData đủ trường (DoD 1) + model API nội bộ `[TK]` (D2-49(b,c)).
// ---------------------------------------------------------------------------

#[test]
fn tcb_data_du_truong_va_model_api() {
    let mut r = rig();
    // cspace + fault_ep
    r.m.tcb_configure(r.client_slot(), Some(r.s(S_CNODE)))
        .expect("Tcb.Configure cspace");
    r.m.tcb_set_fault_ep(r.client_slot(), r.s(S_EP))
        .expect("Tcb.SetFaultEp");
    // priority / max_controlled_priority (D2-49(c): model API, không tuyên bố ABI)
    r.m.tcb_set_priority(r.client_slot(), 5, 9)
        .expect("SetPriority/SetMcp");
    assert_eq!(
        r.m.tcb_set_priority(r.client_slot(), 9, 5),
        Err(KError::InvalidArgument),
        "MCP phải ≥ priority"
    );
    // affinity (HartMask ≤ 64; K1b 1 hart ⇒ chỉ bit 0)
    r.m.tcb_set_affinity(r.client_slot(), 1)
        .expect("SetAffinity");
    assert_eq!(
        r.m.tcb_set_affinity(r.client_slot(), 0),
        Err(KError::InvalidArgument),
        "affinity rỗng vô nghĩa"
    );
    assert_eq!(
        r.m.tcb_set_affinity(r.client_slot(), 2),
        Err(KError::InvalidArgument),
        "K1b 1 hart ⇒ bit 1 ngoài mặt nạ"
    );
    // home
    r.m.tcb_set_home(r.client_slot(), 0).expect("SetHome");
    assert_eq!(
        r.m.tcb_set_home(r.client_slot(), 1),
        Err(KError::InvalidArgument),
        "home phải < số hart"
    );
    // sched_context: model API `set_sched_params(tcb, sc)` — D2-49(b), không đụng ABI
    r.m.set_sched_params(r.client_slot(), Some(r.s(S_SC)))
        .expect("set_sched_params");
    assert_eq!(
        r.m.tcb_info(r.client_slot()).expect("info").sched_context,
        Some(r.sc)
    );
    assert_eq!(
        r.m.set_sched_params(r.client_slot(), Some(r.s(S_EP))),
        Err(KError::WrongObjectType),
        "chỉ SchedContext mới gắn được"
    );
    // bound_notification
    r.m.tcb_bind_notification(r.client_slot(), r.s(S_NTFN))
        .expect("bind notification");
    assert_eq!(
        r.m.tcb_bind_notification(r.client_slot(), r.s(S_EP)),
        Err(KError::WrongObjectType)
    );

    let info = r.m.tcb_info(r.client_slot()).expect("TcbInfo");
    assert_eq!(info.priority, 5);
    assert_eq!(info.max_controlled_priority, 9);
    assert_eq!(info.affinity, 1);
    assert_eq!(info.home, 0);
    assert_eq!(info.sched_context, Some(r.sc));
    assert_eq!(info.bound_notification, Some(r.ntfn));
    assert_eq!(info.cspace, Some(r.c0));
    assert!(info.fault_ep.is_some());
    assert!(!info.suspended);
    assert_eq!(info.exit_code, None);

    // gỡ SchedContext cũng đi qua cùng model API
    r.m.set_sched_params(r.client_slot(), None)
        .expect("gỡ sched params");
    assert_eq!(
        r.m.tcb_info(r.client_slot()).expect("info").sched_context,
        None
    );
}

// ---------------------------------------------------------------------------
// 2. `TcbState` khớp D2-33 (DoD 2): `BlockedOnRecv{ep, reply}`, `offset: u32`, render.
// ---------------------------------------------------------------------------

#[test]
fn tcb_state_khop_d2_33_va_render() {
    let states = [
        TcbState::Inactive,
        TcbState::Ready,
        TcbState::Running { hart: 0 },
        TcbState::BlockedOnSend {
            ep: 1,
            call: true,
            fault: None,
        },
        TcbState::BlockedOnRecv { ep: 1, reply: None },
        TcbState::BlockedOnReply { reply: 1 },
        TcbState::BlockedOnNotification { ntfn: 1 },
        TcbState::BlockedOnAddress {
            frame: 1,
            offset: 16,
        },
        TcbState::Sleeping {
            reason: SleepReason::Timeout,
        },
        TcbState::Stopping,
        TcbState::Dead,
    ];
    assert_eq!(states.len(), N_STATES);
    for (i, st) in states.iter().enumerate() {
        assert_eq!(st.index(), i, "chỉ số trạng thái phải khớp TCB_STATES");
        assert_eq!(st.name(), vifast_abi::TCB_STATES[i]);
    }
    // D2-33: `BlockedOnRecv` mang thêm `reply`; `BlockedOnAddress.offset` là `u32`.
    assert_eq!(
        TcbState::BlockedOnRecv {
            ep: 7,
            reply: Some(3)
        }
        .render(),
        "BlockedOnRecv{ep=7, reply=3}"
    );
    assert_eq!(
        TcbState::BlockedOnRecv { ep: 7, reply: None }.render(),
        "BlockedOnRecv{ep=7, reply=none}"
    );
    let offset: u32 = 16;
    assert_eq!(
        TcbState::BlockedOnAddress { frame: 3, offset }.render(),
        "BlockedOnAddress{frame=3, offset=16}"
    );
}

// ---------------------------------------------------------------------------
// 3. Bảng `ALLOWED` 11×11 (DoD 3+4): cặp ALLOWED `Ok`, cặp cấm `IllegalOperation` và
//    **không** đổi trạng thái; 100% cặp ALLOWED có hit (`pair_hits`/`transition_coverage`).
// ---------------------------------------------------------------------------

#[test]
fn bang_allowed_11x11_du_cap_va_cam() {
    let ids = ids();
    let mut m = Machine::new();
    let mut allowed = 0usize;
    for (from, row) in ALLOWED.iter().enumerate() {
        for (to, is_allowed) in row.iter().enumerate() {
            // Mỗi cặp một TCB mới trên **cùng** một `Machine` ⇒ `pair_hits` cộng dồn đủ 121 cặp.
            let tcb = m.add_kernel_object(ObjType::Tcb);
            drive_to(&mut m, tcb, from, &ids);
            let before = m.tcb_state(tcb).expect("trạng thái nguồn");
            let target = state_for(to, &ids);
            let result = m.transition_tcb(tcb, target);
            if *is_allowed {
                allowed += 1;
                assert!(
                    result.is_ok(),
                    "cặp ALLOWED ({from}->{to}) phải Ok, nhận {result:?}"
                );
                assert_eq!(
                    m.tcb_state(tcb),
                    Ok(target),
                    "cặp ALLOWED ({from}->{to}) phải tới đúng trạng thái đích"
                );
                assert!(
                    m.pair_hit_count(from, to) >= 1,
                    "cặp ALLOWED ({from}->{to}) phải có hit"
                );
            } else {
                assert_eq!(
                    result,
                    Err(KError::IllegalOperation),
                    "cặp cấm ({from}->{to}) phải IllegalOperation"
                );
                assert_eq!(
                    m.tcb_state(tcb),
                    Ok(before),
                    "cặp cấm ({from}->{to}) **không** được đổi trạng thái"
                );
                assert_eq!(
                    m.pair_hit_count(from, to),
                    0,
                    "cặp cấm ({from}->{to}) không được đếm hit"
                );
            }
        }
    }
    // Reviewer-kt18 mục 2: **không** dùng `(48, 48)` như "coverage thao tác". 48 cặp của bảng =
    // **46** cặp suy ra từ thiết kế §4.4/§4.5/D2-25/D2-33 + **2 cặp `Stopping` reserved** cho
    // KT-19/K1c (`Ready/Running → Stopping`; K1b 1 hart chưa có đường nhập `Stopping`). Con số
    // này là coverage của **bảng qua chính cổng**; coverage **thao tác** nằm ở test
    // `moi_duong_dung_mot_cua_transition_tcb` (đường `call/recv/reply/exit/suspend/finalise`).
    let reserved_stopping = ALLOWED[TcbState::Ready.index()][TcbState::Stopping.index()] as usize
        + ALLOWED[TcbState::Running { hart: 0 }.index()][TcbState::Stopping.index()] as usize;
    assert_eq!(
        reserved_stopping, 2,
        "đúng 2 cặp `Stopping` reserved cho KT-19/K1c"
    );
    assert_eq!(
        allowed,
        46 + reserved_stopping,
        "46 cặp suy ra từ thiết kế + 2 cặp reserved"
    );
    assert_eq!(
        m.transition_coverage(),
        (allowed, allowed),
        "mọi cặp ALLOWED của bảng phải có ít nhất một hit qua cổng"
    );
}

/// Ca cấm viết **từ lý do thiết kế** (không đọc bảng ra kỳ vọng) — reviewer-kt18 mục 2:
/// bảng tự chứng minh vòng tròn nếu mọi kỳ vọng đều lấy từ chính `ALLOWED`.
///
/// - `Inactive → Running`: phải qua `Ready` (kích hoạt) rồi `pick` (§4.4/§4.8).
/// - `Sleeping → Running`: hết ngủ ⇒ `Ready` rồi mới `pick` (bảng §4.5 `:307-308`).
/// - `BlockedOnReply → BlockedOnRecv`: không có nguồn thiết kế (phải qua `Ready` sau `Send(reply)`).
/// - `Dead → Ready`: `Dead` là trạng thái cuối (§4.4 `:270`).
/// - `Ready → Sleeping`: chỉ `Running` hết budget mới ngủ (bảng §4.5 `:302`).
#[test]
fn cap_cam_theo_thiet_ke_bi_tu_choi() {
    let ids = ids();
    let mut m = Machine::new();
    for (from, to, ly_do) in [
        (0usize, 2usize, "Inactive→Running phải qua Ready"),
        (8, 2, "Sleeping→Running phải qua Ready"),
        (5, 4, "BlockedOnReply→BlockedOnRecv không có nguồn"),
        (10, 1, "Dead không ra được"),
        (1, 8, "chỉ Running hết budget mới Sleeping"),
    ] {
        let tcb = m.add_kernel_object(ObjType::Tcb);
        drive_to(&mut m, tcb, from, &ids);
        let before = m.tcb_state(tcb).expect("trạng thái nguồn");
        assert_eq!(before, state_for(from, &ids), "{ly_do}");
        assert_eq!(
            m.transition_tcb(tcb, state_for(to, &ids)),
            Err(KError::IllegalOperation),
            "cặp ({from}->{to}) phải bị từ chối: {ly_do}"
        );
        assert_eq!(m.tcb_state(tcb), Ok(before), "cặp cấm không đổi trạng thái");
    }
}

// ---------------------------------------------------------------------------
// 4-8. `Suspend(Abort)` gỡ khỏi hàng chờ **trước** khi bật cờ (DoD 5).
// ---------------------------------------------------------------------------

#[test]
fn suspend_abort_blocked_on_send() {
    let mut r = rig();
    r.m.call(r.client, r.s(S_EP), 42).expect("Call");
    assert_eq!(r.m.endpoint_send_len(r.ep), Ok(1));
    assert_eq!(
        r.m.tcb_state(r.client),
        Ok(TcbState::BlockedOnSend {
            ep: r.ep,
            call: true,
            fault: None
        })
    );
    r.m.tcb_suspend(r.client_slot(), SuspendMode::Abort)
        .expect("Tcb.Suspend(Abort)");
    assert_eq!(r.m.endpoint_send_len(r.ep), Ok(0), "hàng gửi giảm đúng 1");
    let snap = r.read();
    assert_eq!(snap.state, TcbState::Ready);
    assert!(snap.suspended);
    assert_eq!(snap.blocked_on_kind, None);
}

#[test]
fn suspend_abort_blocked_on_recv() {
    let mut r = rig();
    r.m.recv(r.server, r.s(S_EP), Some(r.s(S_REP)))
        .expect("Recv");
    assert_eq!(r.m.endpoint_recv_waiter(r.ep), Ok(Some(r.server)));
    r.m.tcb_suspend(r.s(S_SERVER), SuspendMode::Abort)
        .expect("Tcb.Suspend(Abort)");
    assert_eq!(
        r.m.endpoint_recv_waiter(r.ep),
        Ok(None),
        "hàng nhận giảm đúng 1"
    );
    let snap = r.m.tcb_read_state(r.s(S_SERVER)).expect("read");
    assert_eq!(snap.state, TcbState::Ready);
    assert!(snap.suspended);
    // Không còn tham chiếu TCB trong object chờ: Call sau đó **không** đánh thức ai.
    r.m.call(r.client, r.s(S_EP), 1).expect("Call");
    assert_eq!(
        r.m.tcb_state(r.server),
        Ok(TcbState::Ready),
        "server đã bị Abort không được nhận message"
    );
}

#[test]
fn suspend_abort_blocked_on_reply_va_reply_bi_bo() {
    let mut r = rig();
    r.m.call(r.client, r.s(S_EP), 42).expect("Call");
    r.m.recv(r.server, r.s(S_EP), Some(r.s(S_REP)))
        .expect("Recv");
    assert_eq!(
        r.m.tcb_state(r.client),
        Ok(TcbState::BlockedOnReply { reply: r.rep })
    );
    assert_eq!(r.m.reply_caller(r.rep), Ok(Some(r.client)));
    r.m.tcb_suspend(r.client_slot(), SuspendMode::Abort)
        .expect("Tcb.Suspend(Abort)");
    assert_eq!(r.m.reply_caller(r.rep), Ok(None), "Reply.caller bị tách");
    let snap = r.read();
    assert_eq!(snap.state, TcbState::Ready);
    assert!(snap.suspended);
    // Thiết kế §4.4: `Send(reply)` sau đó **thành công nhưng reply bị bỏ**.
    r.m.reply(r.server, r.s(S_REP))
        .expect("Send(reply) sau Abort phải thành công (bị bỏ)");
    assert_eq!(
        r.m.tcb_state(r.client),
        Ok(TcbState::Ready),
        "client đã dừng không bị đánh thức"
    );
}

#[test]
fn suspend_abort_notification_va_address() {
    // Notification waiter.
    let mut r = rig();
    r.m.tcb_wait_notification(r.client_slot(), r.s(S_NTFN))
        .expect("Wait notification");
    assert_eq!(r.m.notification_waiter(r.ntfn), Ok(Some(r.client)));
    assert_eq!(
        r.m.tcb_state(r.client),
        Ok(TcbState::BlockedOnNotification { ntfn: r.ntfn })
    );
    r.m.tcb_suspend(r.client_slot(), SuspendMode::Abort)
        .expect("Tcb.Suspend(Abort)");
    assert_eq!(
        r.m.notification_waiter(r.ntfn),
        Ok(None),
        "waiter giảm đúng 1"
    );
    let snap = r.read();
    assert_eq!(snap.state, TcbState::Ready);
    assert!(snap.suspended);

    // Address bucket (frame thô — K1b chưa có object Frame, K2; dùng Untyped của rig).
    let mut r = rig();
    r.m.tcb_wait_address(r.client_slot(), r.u0, 16)
        .expect("WaitAddr");
    assert_eq!(r.m.address_waiter(r.u0, 16), Some(r.client));
    assert_eq!(r.m.address_waiters_len(), 1);
    assert_eq!(
        r.m.tcb_state(r.client),
        Ok(TcbState::BlockedOnAddress {
            frame: r.u0,
            offset: 16
        })
    );
    r.m.tcb_suspend(r.client_slot(), SuspendMode::Abort)
        .expect("Tcb.Suspend(Abort)");
    assert_eq!(r.m.address_waiter(r.u0, 16), None, "bucket giảm đúng 1");
    assert_eq!(r.m.address_waiters_len(), 0);
    assert_eq!(r.read().state, TcbState::Ready);
}

#[test]
fn suspend_abort_khi_cho_giao_exit_giu_exit_code() {
    let mut r = rig();
    r.m.tcb_set_fault_ep(r.client_slot(), r.s(S_EP))
        .expect("SetFaultEp");
    r.m.exit(r.client, 7).expect("Exit");
    assert_eq!(
        r.m.tcb_state(r.client),
        Ok(TcbState::BlockedOnSend {
            ep: r.ep,
            call: false,
            fault: Some(FaultKind::Exit { code: 7 })
        })
    );
    assert_eq!(r.m.endpoint_send_len(r.ep), Ok(1));
    r.m.tcb_suspend(r.client_slot(), SuspendMode::Abort)
        .expect("Tcb.Suspend(Abort)");
    assert_eq!(r.m.endpoint_send_len(r.ep), Ok(0), "message Exit bị huỷ");
    let snap = r.read();
    assert_eq!(snap.state, TcbState::Ready);
    assert!(snap.suspended);
    assert_eq!(
        snap.exit_code,
        Some(7),
        "Suspend(Abort) khi chờ giao Exit **giữ** exit_code (D2-25)"
    );
}

/// Reviewer-kt18 mục 3 (bảng §4.5 `:309-310`): `Suspend(Abort)` trên
/// `Sleeping{Budget}` **giữ** `Sleeping` (không phải chuyển trạng thái), bật cờ, giữ `exit_code`,
/// không đụng hàng chờ nào.
#[test]
fn suspend_abort_tren_sleeping_giu_trang_thai() {
    let ids = ids();
    let mut m = Machine::new();
    let tcb = m.add_kernel_object(ObjType::Tcb);
    drive_to(&mut m, tcb, 8, &ids); // Inactive → Ready → Running → Sleeping{Budget}
    let c0 = m.add_cnode(4);
    let cap = m.root_cap(tcb);
    m.put_cap(c0, 0, cap).expect("cap TCB");
    // `exit_code` phải còn sau khi ngủ lại: Exit (không fault_ep) → Inactive, rồi đi lại đường
    // Inactive → Ready → Running → Sleeping (không `Resume`/`Configure` — hai method đó xoá mã).
    m.exit(tcb, 9).expect("Exit không fault_ep");
    assert_eq!(
        m.tcb_read_state(slot(c0, 0)).expect("read").exit_code,
        Some(9)
    );
    drive_to(&mut m, tcb, 1, &ids);
    m.transition_tcb(tcb, state_for(2, &ids))
        .expect("Ready→Running");
    m.transition_tcb(tcb, state_for(8, &ids))
        .expect("Running→Sleeping");
    m.tcb_suspend(slot(c0, 0), SuspendMode::Abort)
        .expect("Abort trên Sleeping");
    let snap = m.tcb_read_state(slot(c0, 0)).expect("read");
    assert_eq!(
        snap.state,
        TcbState::Sleeping {
            reason: SleepReason::Budget
        },
        "Sleeping + Abort **giữ** Sleeping (bảng §4.5)"
    );
    assert!(snap.suspended, "Abort bật cờ suspended");
    assert_eq!(snap.blocked_on_kind, None);
    assert_eq!(snap.exit_code, Some(9), "Abort giữ exit_code");
    assert_eq!(
        m.address_waiters_len(),
        0,
        "không có hàng WaitOnAddress nào"
    );
    assert_eq!(
        m.pair_hit_count(8, 8),
        0,
        "Sleeping→Sleeping không phải chuyển trạng thái ⇒ không có cặp danh tính"
    );
}

// ---------------------------------------------------------------------------
// 6. `Suspend(Defer)` (DoD 6): chưa có thao tác dài ⇒ `InvalidArgument`, state không đổi.
// ---------------------------------------------------------------------------

#[test]
fn suspend_defer_invalid_argument_state_khong_doi() {
    let ids = ids();
    // Trạng thái nguồn: Ready, BlockedOnSend, BlockedOnRecv, Sleeping.
    for index in [1usize, 3, 4, 8] {
        let mut m = Machine::new();
        let tcb = m.add_kernel_object(ObjType::Tcb);
        drive_to(&mut m, tcb, index, &ids);
        let before = m.tcb_state(tcb).expect("nguồn");
        let hit_before = m.pair_hit_count(index, index);
        // `tcb_suspend` nhận SlotRef ⇒ dựng cap gốc cho TCB.
        let c0 = m.add_cnode(4);
        let cap = m.root_cap(tcb);
        m.put_cap(c0, 0, cap).expect("cap TCB");
        assert_eq!(
            m.tcb_suspend(slot(c0, 0), SuspendMode::Defer),
            Err(KError::InvalidArgument),
            "Defer chưa có thao tác dài ⇒ InvalidArgument (state {index})"
        );
        assert_eq!(m.tcb_state(tcb), Ok(before), "Defer không đổi trạng thái");
        let snap = m.tcb_read_state(slot(c0, 0)).expect("read");
        assert!(!snap.suspended, "Defer không bật cờ suspended");
        assert_eq!(
            m.pair_hit_count(index, index),
            hit_before,
            "Defer không phát sinh chuyển trạng thái"
        );
    }
}

// ---------------------------------------------------------------------------
// 7. `Resume` chỉ xoá cờ; **không** phát lại Call/Send/Recv (DoD 7) + xoá `exit_code`.
// ---------------------------------------------------------------------------

#[test]
fn resume_chi_xoa_co_khong_phat_lai() {
    let mut r = rig();
    r.m.call(r.client, r.s(S_EP), 42).expect("Call");
    assert_eq!(r.m.endpoint_last_mr(r.ep), Ok(Some(42)));
    r.m.tcb_suspend(r.client_slot(), SuspendMode::Abort)
        .expect("Suspend(Abort)");
    assert!(r.read().suspended);
    let send_len = r.m.endpoint_send_len(r.ep).expect("hàng gửi");
    let recv_waiter = r.m.endpoint_recv_waiter(r.ep).expect("hàng nhận");
    let last_mr = r.m.endpoint_last_mr(r.ep).expect("last_mr");
    let coverage = r.m.transition_coverage();
    r.m.tcb_resume(r.client_slot()).expect("Tcb.Resume");
    let snap = r.read();
    assert!(!snap.suspended, "Resume xoá cờ");
    assert_eq!(snap.state, TcbState::Ready, "Resume không đổi trạng thái");
    assert_eq!(
        r.m.endpoint_send_len(r.ep),
        Ok(send_len),
        "không phát lại Send"
    );
    assert_eq!(
        r.m.endpoint_recv_waiter(r.ep),
        Ok(recv_waiter),
        "không phát sinh Recv"
    );
    assert_eq!(
        r.m.endpoint_last_mr(r.ep),
        Ok(last_mr),
        "message không bị chép lại"
    );
    assert_eq!(
        r.m.transition_coverage(),
        coverage,
        "Resume không phát sinh chuyển trạng thái"
    );
    // Resume khi không suspended là no-op thành công (seL4: idempotent).
    r.m.tcb_resume(r.client_slot()).expect("Resume lần hai");

    // D2-25: `Resume` xoá `exit_code`; `Suspend(Abort)` thì **giữ**.
    let mut r2 = rig();
    r2.m.tcb_set_fault_ep(r2.client_slot(), r2.s(S_EP))
        .expect("SetFaultEp");
    r2.m.exit(r2.client, 9).expect("Exit");
    r2.m.tcb_suspend(r2.client_slot(), SuspendMode::Abort)
        .expect("Suspend(Abort)");
    assert_eq!(r2.read().exit_code, Some(9));
    r2.m.tcb_resume(r2.client_slot()).expect("Resume");
    assert_eq!(r2.read().exit_code, None, "Resume xoá exit_code (D2-25)");
}

// ---------------------------------------------------------------------------
// 8. `Exit` giữ D2-25 (DoD 8).
// ---------------------------------------------------------------------------

#[test]
fn exit_giu_d2_25() {
    // (a) có fault_ep → BlockedOnSend{call=false, fault=Exit}; procsrv Recv xong → Inactive.
    let mut r = rig();
    r.m.tcb_set_fault_ep(r.client_slot(), r.s(S_EP))
        .expect("SetFaultEp");
    r.m.exit(r.client, 7).expect("Exit");
    assert_eq!(
        r.m.tcb_state(r.client),
        Ok(TcbState::BlockedOnSend {
            ep: r.ep,
            call: false,
            fault: Some(FaultKind::Exit { code: 7 })
        })
    );
    assert_eq!(r.m.endpoint_send_len(r.ep), Ok(1));
    r.m.recv(r.server, r.s(S_EP), None).expect("procsrv Recv");
    assert_eq!(r.m.tcb_state(r.client), Ok(TcbState::Inactive));
    assert_eq!(r.m.pair_hit_count(3, 0), 1, "fault Exit tiêu thụ: 3→0");

    // (b) không fault_ep → Inactive ngay.
    let mut r = rig();
    r.m.exit(r.client, 3).expect("Exit");
    assert_eq!(r.m.tcb_state(r.client), Ok(TcbState::Inactive));
    assert_eq!(
        r.m.tcb_read_state(r.client_slot()).expect("read").exit_code,
        Some(3)
    );

    // (c) Exit khi đang chờ gửi: gỡ message cũ, giao fault qua `fault_ep` (cặp danh tính 3→3).
    let mut r = rig();
    r.m.tcb_set_fault_ep(r.client_slot(), r.s(S_EP))
        .expect("SetFaultEp");
    r.m.call(r.client, r.s(S_EP), 1).expect("Call");
    assert_eq!(r.m.endpoint_send_len(r.ep), Ok(1));
    r.m.exit(r.client, 11).expect("Exit khi BlockedOnSend");
    assert_eq!(
        r.m.endpoint_send_len(r.ep),
        Ok(1),
        "message Call cũ bị huỷ, message Exit thay chỗ"
    );
    assert_eq!(
        r.m.tcb_state(r.client),
        Ok(TcbState::BlockedOnSend {
            ep: r.ep,
            call: false,
            fault: Some(FaultKind::Exit { code: 11 })
        })
    );
    assert_eq!(
        r.m.pair_hit_count(3, 3),
        1,
        "Exit khi đang chờ gửi dùng cặp danh tính (3,3) của bảng"
    );
}

/// Reviewer-kt18 mục 1: `Exit` phải kiểm `fault_ep` **trước** khi ghi `exit_code`/gỡ hàng chờ.
/// Cap `fault_ep` bị Delete ⇒ `Exit` trả `InvalidCapability`, TCB **không** đổi trạng thái,
/// hàng gửi **không** mất message, `exit_code` **không** bị ghi (thiết kế §4.4 invariant).
#[test]
fn exit_cap_fault_ep_chet_khong_doi_trang_thai() {
    let mut r = rig();
    r.m.tcb_set_fault_ep(r.client_slot(), r.s(S_EP))
        .expect("SetFaultEp");
    r.m.call(r.client, r.s(S_EP), 1).expect("Call");
    let before = r.m.tcb_state(r.client).expect("state");
    assert!(matches!(before, TcbState::BlockedOnSend { call: true, .. }));
    assert_eq!(r.m.endpoint_send_len(r.ep), Ok(1));
    r.m.delete(r.s(S_EP)).expect("Delete cap fault_ep");
    assert_eq!(
        r.m.exit(r.client, 7),
        Err(KError::InvalidCapability),
        "cap fault_ep đã chết ⇒ InvalidCapability"
    );
    assert_eq!(
        r.m.tcb_state(r.client),
        Ok(before),
        "Exit lỗi **không** được đổi trạng thái"
    );
    assert_eq!(
        r.m.endpoint_send_len(r.ep),
        Ok(1),
        "Exit lỗi **không** được gỡ hàng chờ"
    );
    assert_eq!(
        r.m.tcb_read_state(r.client_slot()).expect("read").exit_code,
        None,
        "Exit lỗi **không** được ghi exit_code"
    );
}

// ---------------------------------------------------------------------------
// 9. `ReadState` trả **4 giá trị** (`TcbSnapshot`) — DoD 1/ABI `Tcb.ReadState`.
// ---------------------------------------------------------------------------

#[test]
fn read_state_tra_bon_gia_tri() {
    let mut r = rig();
    assert_eq!(
        r.read(),
        TcbSnapshot {
            state: TcbState::Inactive,
            suspended: false,
            blocked_on_kind: None,
            exit_code: None,
        }
    );
    // Send
    r.m.call(r.client, r.s(S_EP), 1).expect("Call");
    assert_eq!(r.read().blocked_on_kind, Some(BlockedOnKind::Send));
    // Reply (client) + Recv (server)
    r.m.recv(r.server, r.s(S_EP), Some(r.s(S_REP)))
        .expect("Recv");
    assert_eq!(r.read().blocked_on_kind, Some(BlockedOnKind::Reply));
    assert_eq!(
        r.m.tcb_read_state(r.s(S_SERVER))
            .expect("read server")
            .blocked_on_kind,
        None,
        "server Ready ⇒ không blocked"
    );
    // Recv
    let mut r2 = rig();
    r2.m.recv(r.server, r2.s(S_EP), None).expect("Recv");
    let snap = r2.m.tcb_read_state(r2.s(S_SERVER)).expect("read");
    assert_eq!(snap.blocked_on_kind, Some(BlockedOnKind::Recv));
    assert_eq!(
        snap.state,
        TcbState::BlockedOnRecv {
            ep: r2.ep,
            reply: None
        }
    );
    // Notification
    let mut r3 = rig();
    r3.m.tcb_wait_notification(r3.client_slot(), r3.s(S_NTFN))
        .expect("Wait");
    assert_eq!(r3.read().blocked_on_kind, Some(BlockedOnKind::Notification));
    // Address
    let mut r4 = rig();
    r4.m.tcb_wait_address(r4.client_slot(), r4.ep, 32)
        .expect("WaitAddr");
    assert_eq!(r4.read().blocked_on_kind, Some(BlockedOnKind::Address));
    assert_eq!(
        r4.read().state,
        TcbState::BlockedOnAddress {
            frame: r4.ep,
            offset: 32
        }
    );
}

// ---------------------------------------------------------------------------
// 10. Finalise TCB **trước** khi object chết (DoD 9, thiết kế §4.3 bảng Finalise).
// ---------------------------------------------------------------------------

#[test]
fn finalise_tcb_truoc_khi_revoke_untyped() {
    let mut m = Machine::new();
    let c0 = m.add_cnode(16);
    let u0 = m.add_untyped(0x1000, 65536);
    let ep = m.add_kernel_object(ObjType::Endpoint);
    let ep2 = m.add_kernel_object(ObjType::Endpoint);
    let ep3 = m.add_kernel_object(ObjType::Endpoint);
    let rep = m.add_kernel_object(ObjType::Reply);
    let ntfn = m.add_kernel_object(ObjType::Notification);
    let sc = m.add_kernel_object(ObjType::SchedContext);
    let u0_cap = m.root_cap(u0);
    m.put_cap(c0, 0, u0_cap).expect("cap Untyped");
    for (i, obj) in [
        (1usize, ep),
        (2, ep2),
        (3, ep3),
        (4, rep),
        (5, ntfn),
        (6, sc),
    ] {
        let cap = m.root_cap(obj);
        m.put_cap(c0, i, cap).expect("cap object ngoài Untyped");
    }
    // 6 TCB đều cấp từ Untyped ⇒ Revoke Untyped finalise cả 6.
    let mut tcb_caps = [0u32; 6];
    for (i, item) in tcb_caps.iter_mut().enumerate() {
        let cap = m
            .retype(slot(c0, 0), ObjType::Tcb, 0, 0)
            .expect("Retype TCB");
        m.put_cap(c0, 8 + i, cap).expect("slot TCB");
        *item = cap;
    }
    let tcb: Vec<ObjId> = tcb_caps
        .iter()
        .map(|c| m.cap_obj(*c).expect("obj TCB"))
        .collect();
    // a: BlockedOnSend (Call không có server).
    m.call(tcb[0], slot(c0, 1), 1).expect("Call");
    // b: BlockedOnRecv.
    m.recv(tcb[1], slot(c0, 2), None).expect("Recv");
    // c: BlockedOnReply (client tcb[2] Call, server tcb[3] Recv có Reply cap).
    m.call(tcb[2], slot(c0, 3), 2).expect("Call");
    m.recv(tcb[3], slot(c0, 3), Some(slot(c0, 4)))
        .expect("Recv");
    // d: BlockedOnNotification.
    m.tcb_wait_notification(slot(c0, 8 + 4), slot(c0, 5))
        .expect("Wait notification");
    // e: BlockedOnAddress (frame thô — K1b chưa có object Frame).
    m.tcb_wait_address(slot(c0, 8 + 5), ep, 16)
        .expect("WaitAddr");
    // f: sched_context gắn cho tcb[0] để finalise phải trả lại.
    m.set_sched_params(slot(c0, 8), Some(slot(c0, 6)))
        .expect("set_sched_params");

    assert_eq!(m.endpoint_send_len(ep), Ok(1));
    assert_eq!(m.endpoint_recv_waiter(ep2), Ok(Some(tcb[1])));
    assert_eq!(m.reply_caller(rep), Ok(Some(tcb[2])));
    assert_eq!(m.notification_waiter(ntfn), Ok(Some(tcb[4])));
    assert_eq!(m.address_waiter(ep, 16), Some(tcb[5]));

    // Revoke Untyped ⇒ finalise 5 TCB **trước** khi destroy rồi mới xoá cap.
    m.revoke(c0, Some(0)).expect("Revoke Untyped");

    let log = m.finalise_log();
    assert_eq!(log.len(), 6, "mọi TCB trong Untyped phải được finalise");
    for entry in log {
        assert_eq!(entry.state, TcbState::Dead, "finalise đặt state Dead");
        assert!(tcb.contains(&entry.tcb), "log chỉ chứa TCB của Untyped");
    }
    let first = log
        .iter()
        .find(|e| e.tcb == tcb[0])
        .expect("entry của tcb[0]");
    assert_eq!(
        first.released_sched_context,
        Some(sc),
        "hook finalise trả lại sched_context"
    );
    for id in &tcb {
        assert!(!m.object_alive(*id), "sau finalise object phải chết");
    }
    // Không còn tham chiếu tới TCB đã chết trong object sống.
    assert_eq!(m.endpoint_send_len(ep), Ok(0));
    assert_eq!(m.endpoint_recv_waiter(ep2), Ok(None));
    assert_eq!(m.reply_caller(rep), Ok(None));
    assert_eq!(m.notification_waiter(ntfn), Ok(None));
    assert_eq!(m.address_waiter(ep, 16), None);
    assert_eq!(m.address_waiters_len(), 0);
    m.check_caps().expect("bất biến cây cap sau finalise");
    // TCB đã chết: đọc trạng thái trả lỗi, không panic.
    assert!(m.tcb_state(tcb[0]).is_err());
}

// ---------------------------------------------------------------------------
// 11. Mọi đường (call/recv/reply/exit/suspend/finalise) đi qua **một** cửa.
// ---------------------------------------------------------------------------

#[test]
fn moi_duong_dung_mot_cua_transition_tcb() {
    let mut r = rig();
    // Server Recv trước: Inactive → Ready (kích hoạt mô hình) rồi Ready → BlockedOnRecv.
    r.m.recv(r.server, r.s(S_EP), Some(r.s(S_REP)))
        .expect("Recv");
    assert_eq!(r.m.pair_hit_count(0, 1), 1, "Recv kích hoạt: 0→1");
    assert_eq!(r.m.pair_hit_count(1, 4), 1, "Recv chặn: 1→4");
    // Client Call có server chờ: Inactive → Ready, Ready → BlockedOnReply; server 4→1.
    r.m.call(r.client, r.s(S_EP), 7).expect("Call");
    assert_eq!(r.m.pair_hit_count(1, 5), 1, "Call có người nhận: 1→5");
    assert_eq!(r.m.pair_hit_count(4, 1), 1, "server được đánh thức: 4→1");
    // Reply: client 5→1.
    r.m.reply(r.server, r.s(S_REP)).expect("Reply");
    assert_eq!(r.m.pair_hit_count(5, 1), 1, "Reply: 5→1");
    // Call không người nhận: Ready → BlockedOnSend; Suspend(Abort): 3→1.
    r.m.call(r.client, r.s(S_EP), 8).expect("Call");
    assert_eq!(r.m.pair_hit_count(1, 3), 1, "Call chặn: 1→3");
    r.m.tcb_suspend(r.client_slot(), SuspendMode::Abort)
        .expect("Suspend");
    assert_eq!(r.m.pair_hit_count(3, 1), 1, "Suspend(Abort): 3→1");
    // Resume không chuyển trạng thái ⇒ không hit mới.
    let coverage = r.m.transition_coverage();
    r.m.tcb_resume(r.client_slot()).expect("Resume");
    assert_eq!(r.m.transition_coverage(), coverage);
    // Exit không fault_ep: Ready → Inactive.
    r.m.exit(r.client, 3).expect("Exit");
    assert_eq!(r.m.pair_hit_count(1, 0), 1, "Exit không fault_ep: 1→0");
    // Finalise (Revoke Untyped): TCB chết đi qua cặp → Dead.
    let tcb2 = r.m.add_kernel_object(ObjType::Tcb);
    let c1 = r.m.add_cnode(2);
    let cap = r.m.root_cap(tcb2);
    r.m.put_cap(c1, 0, cap).expect("cap TCB");
    r.m.recv(tcb2, r.s(S_EP), None).expect("Recv TCB phụ");
    r.m.finalise_tcb(tcb2).expect("finalise trực tiếp");
    assert_eq!(r.m.tcb_state(tcb2), Ok(TcbState::Dead));
    assert_eq!(r.m.pair_hit_count(4, 10), 1, "finalise: 4→10");
}

// ---------------------------------------------------------------------------
// 12. `Suspend` trên TCB `Inactive`/`Dead` là `IllegalOperation`.
// ---------------------------------------------------------------------------

#[test]
fn suspend_tcb_inactive_va_dead_illegal() {
    let mut r = rig();
    assert_eq!(
        r.m.tcb_suspend(r.client_slot(), SuspendMode::Abort),
        Err(KError::IllegalOperation),
        "không có gì để dừng ở Inactive"
    );
    assert_eq!(r.m.tcb_state(r.client), Ok(TcbState::Inactive));
    assert!(!r.read().suspended);

    r.m.exit(r.client, 1).expect("Exit");
    r.m.transition_tcb(r.client, TcbState::Dead)
        .expect("1→10 khi finalise");
    assert_eq!(
        r.m.tcb_suspend(r.client_slot(), SuspendMode::Abort),
        Err(KError::IllegalOperation)
    );
    assert_eq!(r.m.tcb_state(r.client), Ok(TcbState::Dead));
}
