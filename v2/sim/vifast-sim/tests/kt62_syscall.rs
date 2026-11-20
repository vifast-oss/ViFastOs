// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-62 — **16 syscall `0..15` đi vào `Machine` thật** (`vifast-core::syscall`), test trên host.
//!
//! Trước KT-62 chỉ 3 syscall (`DebugPutStr` 12, `DebugHalt` 13, `Exit` 14) có đường; phần còn
//! lại của dải 0..15 trả `NotSupported` (shim KT-53). Bộ test này kiểm **bảng dispatch**:
//!
//! | # | Test | Điều kiện đo |
//! |---|---|---|
//! | 1 | [`kt62_bang_16_syscall_vao_core`] | từng `nr` 0..15 đi vào API core: Call/Send/NBSend chặn đúng, Recv/NBRecv/ReplyRecv, NBSendRecv/NBSendWait, Yield, WaitAddr/WakeAddr, ClockRead, DebugPutStr, DebugHalt, Exit, DebugGetInfo — **không** nhánh `NotSupported` giả cho 8/9/10/11/15 |
//! | 2 | [`kt62_range_error_19`] | `nr` 16/17/31/99/EID SBI ⇒ **19 RangeError** (D2-79), không panic |
//! | 3 | [`kt62_cap_loi_dung_ma`] | slot rỗng ⇒ 1 `InvalidCapability`; Send trên cap TCB ⇒ 2 `WrongObjectType`; Recv trên cap TCB ⇒ 2; Call trên cap TCB ⇒ 18 `NotSupported` (D2-53 Q11) |
//! | 4 | [`kt62_exit_khong_reply`] | `Exit(42)` trên TCB có `fault_ep` ⇒ `BlockedOnSend{call:false, fault:Exit{42}}`, hàng Endpoint có **fault** (không message), **không** Reply; `Exit` lần hai ⇒ `IllegalOperation` |
//! | 5 | [`kt62_yield_doi_tcb`] | 2 TCB cùng mức: `Yield` chuyển current `t0 → t1`, `t0 Ready`; trả phần budget còn lại |
//! | 6 | [`kt62_ipc_vong_qua_dispatch`] | Call(0) → Recv(3) giao message 2 MR → Send trên Reply cap (1) trả lời; kiểm `MessageInfo`/MR trả về user + `Reply.caller` sạch |
//! | 7 | [`kt62_nb_ipc`] | NBSend(2) không chặn; NBRecv(4) lấy message / `Busy`; NBSendRecv(6); NBSendWait(7) với Notification đã signal |
//! | 8 | [`kt62_waitaddr_wakeaddr_clock_put_halt`] | WaitAddr(9) khớp ⇒ chặn, lệch ⇒ 5; WakeAddr(10) đánh thức đúng 1; ClockRead(11) đơn điệu; DebugPutStr(12); DebugHalt(13) trả outcome (không dừng host) |
//!
//! **Biên:** chưa chuyển cap (`caps > 0` ⇒ 5), MR > 4 ⇒ 5 (IPC buffer là nợ KT-63) — ghi ở
//! `docs/k3b-core.md` §11. Test này chạy trong `gate --fast` (L0 của workspace v2).

use kc::syscall::{self, SyscallEnv, SyscallOutcome};
use kc::{KError, Machine, NonReadyTarget, ObjId, ObjType, SlotRef, TcbState};
use vifast_core as kc;

// ---------------------------------------------------------------------------------------------
// Bộ khung: Machine + CNode + 2 TCB + Endpoint + Reply + Notification + SC (từ Untyped thật)
// ---------------------------------------------------------------------------------------------

const UNTYPED_BASE: u64 = 0x1000_0000;
const UNTYPED_BYTES: usize = 1 << 20;

struct Rig {
    u: ObjId,
    t0s: SlotRef,
    t1s: SlotRef,
    eps: SlotRef,
    rs: SlotRef,
    ns: SlotRef,
    ctl: SlotRef,
    sc0: SlotRef,
    sc1: SlotRef,
    cspace: SlotRef,
    t0: ObjId,
    t1: ObjId,
    ep: ObjId,
    reply: ObjId,
    ntfn: ObjId,
}

fn machine() -> Machine {
    let mut m = Machine::with_harts(1);
    m.add_untyped(UNTYPED_BASE, UNTYPED_BYTES);
    m
}

fn build_rig(m: &mut Machine) -> Rig {
    let u = m.add_untyped(UNTYPED_BASE + (1 << 19), UNTYPED_BYTES);
    let root = m.add_cnode(32);
    let us = SlotRef::new(root, 0);
    let uc = m.root_cap(u);
    m.put_cap(root, 0, uc).expect("cap untyped");
    let tys = [
        ObjType::Tcb,
        ObjType::Tcb,
        ObjType::Endpoint,
        ObjType::Reply,
        ObjType::Notification,
        ObjType::SchedControl,
        ObjType::SchedContext,
        ObjType::SchedContext,
    ];
    let mut obj = [0u32; 8];
    for (i, ty) in tys.iter().enumerate() {
        let cap = m.retype(us, *ty, 0, 0).expect("retype");
        m.put_cap(root, 1 + i, cap).expect("put cap");
        obj[i] = m.cap_obj(cap).expect("cap_obj");
    }
    let rc = m.root_cap(root);
    m.put_cap(root, 10, rc).expect("cap cspace");
    Rig {
        u,
        t0s: SlotRef::new(root, 1),
        t1s: SlotRef::new(root, 2),
        eps: SlotRef::new(root, 3),
        rs: SlotRef::new(root, 4),
        ns: SlotRef::new(root, 5),
        ctl: SlotRef::new(root, 6),
        sc0: SlotRef::new(root, 7),
        sc1: SlotRef::new(root, 8),
        cspace: SlotRef::new(root, 10),
        t0: obj[0],
        t1: obj[1],
        ep: obj[2],
        reply: obj[3],
        ntfn: obj[4],
    }
}

/// Cấu hình 1 TCB: cspace + SC + priority; đưa về `Running{hart 0}`.
fn setup_tcb(m: &mut Machine, rig: &Rig, slot: SlotRef, tcb: ObjId, sc: SlotRef) {
    m.tcb_configure(slot, Some(rig.cspace)).expect("cspace");
    m.sched_control_configure(rig.ctl, sc, 100, 1_000)
        .expect("sc");
    m.set_sched_params(slot, Some(sc)).expect("sched params");
    m.tcb_set_priority(slot, 3, 10).expect("priority");
    m.start_tcb(tcb).expect("ready");
    m.transition_tcb(tcb, NonReadyTarget::Running { hart: 0 })
        .expect("running");
}

fn setup(m: &mut Machine) -> Rig {
    let rig = build_rig(m);
    setup_tcb(m, &rig, rig.t0s, rig.t0, rig.sc0);
    rig
}

/// Đưa `tcb` trở lại `Running{0}` (sau khi một syscall làm nó chặn/ngủ) — chỉ dùng cho test.
fn make_running(m: &mut Machine, tcb: ObjId) {
    if m.tcb_state(tcb) != Ok(TcbState::Running { hart: 0 }) {
        let _ = m.wake(tcb);
        if m.tcb_state(tcb) == Ok(TcbState::Ready) {
            m.transition_tcb(tcb, NonReadyTarget::Running { hart: 0 })
                .expect("running lại");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Hook arch giả — cùng contract với bản QEMU (`vifast-plat-riscv64::user`).
// ---------------------------------------------------------------------------------------------

struct FakeEnv {
    now: u64,
    addr: Option<(u32, u32)>,
    puts: usize,
    bytes: usize,
    features: u64,
    harts: u64,
}

impl FakeEnv {
    fn new() -> Self {
        Self {
            now: 1_000,
            addr: Some((7, 64)),
            puts: 0,
            bytes: 0,
            features: 0b1111,
            harts: 3,
        }
    }
}

impl SyscallEnv for FakeEnv {
    fn put_str(&mut self, _ptr: u64, len: u64) -> Result<u64, u64> {
        self.puts += 1;
        self.bytes += len as usize;
        Ok(len)
    }
    fn resolve_addr(&mut self, _va: u64) -> Result<(u32, u32), u64> {
        self.addr.ok_or(11)
    }
    fn now_ns(&mut self) -> u64 {
        self.now += 10;
        self.now
    }
    fn features(&mut self) -> u64 {
        self.features
    }
    fn num_harts(&mut self) -> u64 {
        self.harts
    }
}

fn args(a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64) -> [u64; 6] {
    [a0, a1, a2, a3, a4, a5]
}

fn call(m: &mut Machine, tcb: ObjId, nr: u64, a: [u64; 6], env: &mut FakeEnv) -> SyscallOutcome {
    syscall::dispatch(m, tcb, 0, nr, a, env)
}

fn err(outcome: SyscallOutcome) -> u64 {
    match outcome {
        SyscallOutcome::Return(r) => r.error,
        other => panic!("cần Return, thấy {other:?}"),
    }
}

/// `info` 35 bit: label 0, caps 0, no_reply 0, extra 0, length `len`.
fn info(len: u8) -> u64 {
    u64::from(len) << 28
}

// ---------------------------------------------------------------------------------------------
// 1) Bảng 16 syscall
// ---------------------------------------------------------------------------------------------

#[test]
fn kt62_bang_16_syscall_vao_core() {
    let mut m = machine();
    let rig = setup(&mut m);
    let mut env = FakeEnv::new();

    // --- 3 Recv: hàng trống ⇒ chặn trên Endpoint (trước mọi lần gửi).
    let out = call(
        &mut m,
        rig.t0,
        3,
        args(rig.eps.slot as u64, syscall::REPLY_NONE, 0, 0, 0, 0),
        &mut env,
    );
    assert!(matches!(
        out,
        SyscallOutcome::Switched {
            state: TcbState::BlockedOnRecv { .. }
        }
    ));
    make_running(&mut m, rig.t0);

    // --- 4 NBRecv: hàng trống ⇒ 14 `Busy` (Poll, spec `[syscall]`).
    let out = call(
        &mut m,
        rig.t0,
        4,
        args(rig.eps.slot as u64, 0, 0, 0, 0, 0),
        &mut env,
    );
    assert_eq!(err(out), KError::Busy.id() as u64);

    // --- 0 Call: không có người nhận ⇒ chặn trên hàng gửi (đúng core `call_msg`).
    let out = call(
        &mut m,
        rig.t0,
        0,
        args(rig.eps.slot as u64, info(1), 0xaa, 0, 0, 0),
        &mut env,
    );
    assert_eq!(
        out,
        SyscallOutcome::Switched {
            state: TcbState::BlockedOnSend {
                ep: rig.ep,
                call: true,
                fault: None
            }
        },
        "Call phải vào core và chặn trên Endpoint"
    );
    assert_eq!(m.endpoint_send_len(rig.ep).unwrap(), 1);
    make_running(&mut m, rig.t0);
    assert_eq!(m.endpoint_send_len(rig.ep).unwrap(), 0, "wake gỡ message");

    // --- 1 Send: cũng chặn (không Reply).
    let out = call(
        &mut m,
        rig.t0,
        1,
        args(rig.eps.slot as u64, info(0), 0, 0, 0, 0),
        &mut env,
    );
    assert!(matches!(
        out,
        SyscallOutcome::Switched {
            state: TcbState::BlockedOnSend { call: false, .. }
        }
    ));
    make_running(&mut m, rig.t0);

    // --- 2 NBSend: không chặn, message nằm trong hàng + không Reply.
    let out = call(
        &mut m,
        rig.t0,
        2,
        args(rig.eps.slot as u64, info(1), 0xbb, 0, 0, 0),
        &mut env,
    );
    assert_eq!(err(out), 0, "NBSend hợp lệ");
    assert!(matches!(m.tcb_state(rig.t0), Ok(TcbState::Running { .. })));
    assert_eq!(m.endpoint_send_len(rig.ep).unwrap(), 1);

    // --- 3 Recv (lần hai): lấy message vừa NBSend — trả info + MR cho user.
    match call(
        &mut m,
        rig.t0,
        3,
        args(rig.eps.slot as u64, syscall::REPLY_NONE, 0, 0, 0, 0),
        &mut env,
    ) {
        SyscallOutcome::Return(r) => {
            assert_eq!((r.error, r.value), (0, 1));
            assert_eq!(r.info >> 28, 1, "a2 = MessageInfo (length 1)");
            assert_eq!(r.mr[0], 0xbb);
        }
        other => panic!("Recv lấy message phải trả về, thấy {other:?}"),
    }

    // --- 5 ReplyRecv: Reply chưa có caller ⇒ reply bị bỏ, Recv chặn.
    let out = call(
        &mut m,
        rig.t0,
        5,
        args(rig.rs.slot as u64, rig.eps.slot as u64, info(0), 0, 0, 0),
        &mut env,
    );
    assert!(matches!(
        out,
        SyscallOutcome::Switched {
            state: TcbState::BlockedOnRecv { .. }
        }
    ));
    make_running(&mut m, rig.t0);

    // --- 6 NBSendRecv: tự gửi rồi tự nhận được message trên cùng Endpoint.
    let out = call(
        &mut m,
        rig.t0,
        6,
        args(
            rig.eps.slot as u64,
            rig.eps.slot as u64,
            syscall::REPLY_NONE,
            info(1),
            0xcc,
            0,
        ),
        &mut env,
    );
    match out {
        SyscallOutcome::Return(r) => {
            assert_eq!(r.error, 0);
            assert_eq!(r.mr[0], 0xcc);
        }
        other => panic!("NBSendRecv phải trả message, thấy {other:?}"),
    }
    make_running(&mut m, rig.t0);

    // --- 7 NBSendWait: Notification đã signal ⇒ Wait không chặn.
    m.notification_signal(rig.ntfn, 0x2).expect("signal");
    let out = call(
        &mut m,
        rig.t0,
        7,
        args(rig.eps.slot as u64, rig.ns.slot as u64, info(0), 0, 0, 0),
        &mut env,
    );
    assert_eq!(err(out), 0, "NBSendWait với word sẵn sàng");
    make_running(&mut m, rig.t0);

    // --- 8 Yield: một TCB ⇒ round-trip Running→Ready→Running, trả budget còn lại.
    let out = call(&mut m, rig.t0, 8, args(0, 0, 0, 0, 0, 0), &mut env);
    match out {
        SyscallOutcome::Return(r) => {
            assert_eq!(r.error, 0);
            assert!(r.value <= 100, "budget còn lại ≤ budget (100)");
        }
        other => panic!("Yield phải trả về user, thấy {other:?}"),
    }
    assert_eq!(m.tcb_state(rig.t0), Ok(TcbState::Running { hart: 0 }));

    // --- 9 WaitAddr: giá trị khớp ⇒ chặn; lệch ⇒ 5.
    m.set_addr_word(7, 64, 0x1234).expect("addr word");
    let out = call(
        &mut m,
        rig.t0,
        9,
        args(0x4000_0000, 0x1234, 0, 0, 0, 0),
        &mut env,
    );
    assert_eq!(
        out,
        SyscallOutcome::Switched {
            state: TcbState::BlockedOnAddress {
                frame: 7,
                offset: 64
            }
        },
        "WaitAddr khớp phải chặn đúng khoá (frame, offset)"
    );
    make_running(&mut m, rig.t0);
    let out = call(
        &mut m,
        rig.t0,
        9,
        args(0x4000_0000, 0x9999, 0, 0, 0, 0),
        &mut env,
    );
    assert_eq!(err(out), KError::InvalidArgument.id() as u64);

    // --- 10 WakeAddr: không waiter ⇒ 0.
    let out = call(
        &mut m,
        rig.t0,
        10,
        args(0x4000_0000, 1, 0, 0, 0, 0),
        &mut env,
    );
    match out {
        SyscallOutcome::Return(r) => assert_eq!((r.error, r.value), (0, 0)),
        other => panic!("WakeAddr phải trả về, thấy {other:?}"),
    }

    // --- 11 ClockRead: đồng hồ từ hook arch.
    let out = call(&mut m, rig.t0, 11, args(0, 0, 0, 0, 0, 0), &mut env);
    match out {
        SyscallOutcome::Return(r) => assert!(r.error == 0 && r.value > 0),
        other => panic!("ClockRead phải trả về, thấy {other:?}"),
    }

    // --- 12 DebugPutStr: đi qua hook `put_str` (arch đọc bộ nhớ user).
    let out = call(
        &mut m,
        rig.t0,
        12,
        args(0x4000_0000, 26, 0, 0, 0, 0),
        &mut env,
    );
    match out {
        SyscallOutcome::Return(r) => assert_eq!((r.error, r.value), (0, 26)),
        other => panic!("DebugPutStr phải trả về, thấy {other:?}"),
    }
    assert_eq!(env.puts, 1);
    // Quá trần `MAX_DEBUG_PUT` ⇒ 5.
    assert_eq!(
        err(call(
            &mut m,
            rig.t0,
            12,
            args(0, 4097, 0, 0, 0, 0),
            &mut env
        )),
        KError::InvalidArgument.id() as u64
    );

    // --- 13 DebugHalt: chỉ trả outcome; host **không** dừng.
    assert_eq!(
        call(&mut m, rig.t0, 13, args(7, 0, 0, 0, 0, 0), &mut env),
        SyscallOutcome::Halt { code: 7 }
    );

    // --- 15 DebugGetInfo: a1=abi_version, a3=min_compat, a4=features, a5=num_harts.
    match call(&mut m, rig.t0, 15, args(0, 0, 0, 0, 0, 0), &mut env) {
        SyscallOutcome::Return(r) => {
            assert_eq!(r.error, 0);
            assert_eq!(r.value, u64::from(vifast_abi::ABI_VERSION));
            assert_eq!(r.mr[0], u64::from(vifast_abi::MIN_COMPAT));
            assert_eq!(r.mr[1], 0b1111);
            assert_eq!(r.mr[2], env.harts, "num_harts = số hart nền tảng (D2-88a)");
            assert_eq!(r.mr[3], 0);
        }
        other => panic!("DebugGetInfo phải trả về, thấy {other:?}"),
    }

    // --- 14 Exit: kiểm riêng ở test 4 (fault_ep + không Reply).
    assert!(m.check_caps().is_ok() && m.check_ipc().is_ok() && m.check_sched().is_ok());
}

// ---------------------------------------------------------------------------------------------
// 2) Ngoài dải 0..15 ⇒ 19
// ---------------------------------------------------------------------------------------------

#[test]
fn kt62_range_error_19() {
    let mut m = machine();
    let rig = setup(&mut m);
    let mut env = FakeEnv::new();
    for nr in [16u64, 17, 31, 99, 0x5352_5354, u64::MAX] {
        let out = call(&mut m, rig.t0, nr, args(0, 0, 0, 0, 0, 0), &mut env);
        assert_eq!(
            err(out),
            19,
            "nr={nr} ngoài 0..15 phải là RangeError 19 (D2-79)"
        );
        // TCB không đổi trạng thái, không panic.
        assert_eq!(m.tcb_state(rig.t0), Ok(TcbState::Running { hart: 0 }));
    }
    // Dải trong ABI không bị nhầm thành RangeError: 15 trả Ok.
    match call(&mut m, rig.t0, 15, args(0, 0, 0, 0, 0, 0), &mut env) {
        SyscallOutcome::Return(r) => assert_eq!(r.error, 0),
        other => panic!("15 phải trả về, thấy {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------------
// 3) Cap sai ⇒ đúng mã
// ---------------------------------------------------------------------------------------------

#[test]
fn kt62_cap_loi_dung_ma() {
    let mut m = machine();
    let rig = setup(&mut m);
    let mut env = FakeEnv::new();

    // Slot rỗng (31 chưa cấp) ⇒ 1 `InvalidCapability`.
    assert_eq!(
        err(call(
            &mut m,
            rig.t0,
            1,
            args(31, info(0), 0, 0, 0, 0),
            &mut env
        )),
        KError::InvalidCapability.id() as u64
    );
    // Cap index quá lớn (không đổi được sang usize slot) ⇒ 1.
    assert_eq!(
        err(call(
            &mut m,
            rig.t0,
            1,
            args(u64::MAX, info(0), 0, 0, 0, 0),
            &mut env
        )),
        KError::InvalidCapability.id() as u64
    );
    // Send trên cap TCB ⇒ 2 `WrongObjectType`.
    assert_eq!(
        err(call(
            &mut m,
            rig.t0,
            1,
            args(rig.t0s.slot as u64, info(0), 0, 0, 0, 0),
            &mut env
        )),
        KError::WrongObjectType.id() as u64
    );
    // Recv trên cap TCB ⇒ 2.
    assert_eq!(
        err(call(
            &mut m,
            rig.t0,
            3,
            args(rig.t0s.slot as u64, syscall::REPLY_NONE, 0, 0, 0, 0),
            &mut env
        )),
        KError::WrongObjectType.id() as u64
    );
    // Call trên cap không phải Endpoint ⇒ 18 `NotSupported` (D2-53 Q11).
    assert_eq!(
        err(call(
            &mut m,
            rig.t0,
            0,
            args(rig.t0s.slot as u64, info(0), 0, 0, 0, 0),
            &mut env
        )),
        KError::NotSupported.id() as u64
    );
    // TCB chưa cấu hình cspace ⇒ mọi cap là 1.
    let rig2 = build_rig(&mut m);
    assert_eq!(
        err(call(
            &mut m,
            rig2.t0,
            1,
            args(rig2.eps.slot as u64, info(0), 0, 0, 0, 0),
            &mut env
        )),
        KError::InvalidCapability.id() as u64
    );
    // Cùng lúc: `check_*` vẫn xanh sau các mã lỗi.
    assert!(m.check_caps().is_ok());
    assert!(m.check_ipc().is_ok());
    // Untyped của rig vẫn kế toán được (không rò cap/byte).
    assert!(m.untyped_used(rig.u) > 0);
    assert!(m.check_bytes(rig.u).is_ok());
}

// ---------------------------------------------------------------------------------------------
// 4) Exit không Reply + fault `FaultKind::Exit`
// ---------------------------------------------------------------------------------------------

#[test]
fn kt62_exit_khong_reply() {
    let mut m = machine();
    let rig = setup(&mut m);
    let mut env = FakeEnv::new();
    m.tcb_set_fault_ep(rig.t0s, rig.eps).expect("fault_ep");

    let out = call(&mut m, rig.t0, 14, args(42, 0, 0, 0, 0, 0), &mut env);
    assert_eq!(out, SyscallOutcome::Exit { code: 42 });
    assert_eq!(
        m.tcb_state(rig.t0),
        Ok(TcbState::BlockedOnSend {
            ep: rig.ep,
            call: false,
            fault: Some(kc::FaultKind::Exit { code: 42 })
        }),
        "Exit phải chặn trên fault_ep mang FaultKind::Exit (D2-25)"
    );
    // Hàng gửi có **fault** (không message), không Reply.
    let items: Vec<_> = m.endpoint_send_items(rig.ep).expect("hàng").collect();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].0, rig.t0, "người gửi fault là TCB thoát");
    assert_eq!(
        items[0].1,
        Some(kc::FaultKind::Exit { code: 42 }),
        "hàng gửi mang FaultKind::Exit, không phải message"
    );
    assert_eq!(m.reply_caller(rig.reply).unwrap_or(None), None);
    assert_eq!(m.tcb_read_state(rig.t0s).unwrap().exit_code, Some(42));
    // Exit trên TCB **không** có fault_ep ⇒ Inactive ngay, không sinh fault/Reply.
    setup_tcb(&mut m, &rig, rig.t1s, rig.t1, rig.sc1);
    assert_eq!(
        call(&mut m, rig.t1, 14, args(7, 0, 0, 0, 0, 0), &mut env),
        SyscallOutcome::Exit { code: 7 }
    );
    assert_eq!(m.tcb_state(rig.t1), Ok(TcbState::Inactive));
    assert_eq!(m.endpoint_send_len(rig.ep).unwrap(), 1, "chỉ fault của t0");
    assert!(m.check_fault().is_ok());
    assert!(m.check_sched().is_ok());
}

// ---------------------------------------------------------------------------------------------
// 5) Yield đổi TCB
// ---------------------------------------------------------------------------------------------

#[test]
fn kt62_yield_doi_tcb() {
    let mut m = machine();
    let rig = setup(&mut m);
    setup_tcb(&mut m, &rig, rig.t1s, rig.t1, rig.sc1);
    let mut env = FakeEnv::new();
    // `setup_tcb` đưa t1 Running ⇒ current = t1; Yield của t1 ⇒ quay lại t0? Cùng mức FIFO:
    // t0 đang Running trước đó bị t1 thay; để kiểm tường minh, đặt t0 làm current lại.
    m.tcb_yield(rig.t1).expect("t1 ready");
    m.transition_tcb(rig.t0, NonReadyTarget::Running { hart: 0 })
        .expect("t0 running");
    let before = m.current(0);
    assert_eq!(before, Some(rig.t0));

    let out = call(&mut m, rig.t0, 8, args(0, 0, 0, 0, 0, 0), &mut env);
    assert_eq!(
        out,
        SyscallOutcome::Switched {
            state: TcbState::Ready
        },
        "Yield phải nhường cho TCB kế (t0 về Ready)"
    );
    assert_eq!(m.current(0), Some(rig.t1), "current phải là t1 sau Yield");
    assert_eq!(m.tcb_state(rig.t0), Ok(TcbState::Ready));
    assert_eq!(m.tcb_state(rig.t1), Ok(TcbState::Running { hart: 0 }));
    assert!(m.tcb_remaining(rig.t0) <= 100);
    assert!(m.check_sched().is_ok());
}

// ---------------------------------------------------------------------------------------------
// 6) Vòng IPC qua dispatch
// ---------------------------------------------------------------------------------------------

#[test]
fn kt62_ipc_vong_qua_dispatch() {
    let mut m = machine();
    let rig = setup(&mut m);
    setup_tcb(&mut m, &rig, rig.t1s, rig.t1, rig.sc1);
    let mut env = FakeEnv::new();

    // Server t1 Recv (chặn), rồi client t0 Call 2 MR.
    let out = call(
        &mut m,
        rig.t1,
        3,
        args(rig.eps.slot as u64, rig.rs.slot as u64, 0, 0, 0, 0),
        &mut env,
    );
    assert!(matches!(
        out,
        SyscallOutcome::Switched {
            state: TcbState::BlockedOnRecv { .. }
        }
    ));
    m.transition_tcb(rig.t0, NonReadyTarget::Running { hart: 0 })
        .expect("t0 running");
    let out = call(
        &mut m,
        rig.t0,
        0,
        args(rig.eps.slot as u64, info(2), 0x11, 0x22, 0, 0),
        &mut env,
    );
    assert!(matches!(
        out,
        SyscallOutcome::Switched {
            state: TcbState::BlockedOnReply { .. }
        }
    ));
    // t1 đã Ready + nhận message 2 MR.
    assert_eq!(m.tcb_state(rig.t1), Ok(TcbState::Ready));
    let area = m.tcb_ipc_area(rig.t1).expect("ipc area");
    assert_eq!(area.info.map(|i| i.length()), Some(2));
    assert_eq!((area.mr(0), area.mr(1)), (Some(0x11), Some(0x22)));

    // Server t1 Reply: Send trên Reply cap (1) — caller t0 về Ready, Reply.caller sạch.
    m.transition_tcb(rig.t1, NonReadyTarget::Running { hart: 0 })
        .expect("t1 running");
    let out = call(
        &mut m,
        rig.t1,
        1,
        args(rig.rs.slot as u64, info(1), 0x33, 0, 0, 0),
        &mut env,
    );
    // `Send` trên Reply: reply xong TCB server về `Ready`; dispatcher có thể chọn caller ⇒
    // chấp nhận cả `Return` (server được chọn lại) và `Switched` (caller chạy trước).
    match out {
        SyscallOutcome::Return(r) => assert_eq!(r.error, 0),
        SyscallOutcome::Switched { .. } => {}
        other => panic!("Send Reply, thấy {other:?}"),
    }
    assert_eq!(m.tcb_state(rig.t1), Ok(TcbState::Ready));
    let area0 = m.tcb_ipc_area(rig.t0).expect("ipc area t0");
    assert_eq!(area0.mr(0), Some(0x33), "Reply chuyển MR cho caller");
    assert_eq!(m.reply_caller(rig.reply).unwrap_or(None), None);
    assert!(m.check_ipc().is_ok());
    assert!(m.check_reply_links().is_ok());
}

// ---------------------------------------------------------------------------------------------
// 7) Nhóm NB
// ---------------------------------------------------------------------------------------------

#[test]
fn kt62_nb_ipc() {
    let mut m = machine();
    let rig = setup(&mut m);
    let mut env = FakeEnv::new();

    // NBSend (2) không chặn, message nằm trong hàng.
    assert_eq!(
        err(call(
            &mut m,
            rig.t0,
            2,
            args(rig.eps.slot as u64, info(1), 0x55, 0, 0, 0),
            &mut env
        )),
        0
    );
    assert_eq!(m.endpoint_send_len(rig.ep).unwrap(), 1);
    // NBRecv (4) lấy được message + info/MR trả về user.
    match call(
        &mut m,
        rig.t0,
        4,
        args(rig.eps.slot as u64, 0, 0, 0, 0, 0),
        &mut env,
    ) {
        SyscallOutcome::Return(r) => {
            assert_eq!(r.error, 0);
            assert_eq!(r.value, 1, "value = length");
            assert_eq!(r.info >> 28, 1, "MessageInfo trả ở a2");
            assert_eq!(r.mr[0], 0x55);
        }
        other => panic!("NBRecv phải trả message, thấy {other:?}"),
    }
    // Hàng rỗng ⇒ 14 Busy.
    assert_eq!(
        err(call(
            &mut m,
            rig.t0,
            4,
            args(rig.eps.slot as u64, 0, 0, 0, 0, 0),
            &mut env
        )),
        KError::Busy.id() as u64
    );
    // NBSendRecv (6): gửi rồi nhận được chính message đó.
    match call(
        &mut m,
        rig.t0,
        6,
        args(
            rig.eps.slot as u64,
            rig.eps.slot as u64,
            syscall::REPLY_NONE,
            info(1),
            0x66,
            0,
        ),
        &mut env,
    ) {
        SyscallOutcome::Return(r) => {
            assert_eq!(r.error, 0);
            assert_eq!(r.mr[0], 0x66);
        }
        other => panic!("NBSendRecv phải trả message, thấy {other:?}"),
    }
    // NBSendWait (7): Notification chưa signal ⇒ chặn trên Notification.
    let out = call(
        &mut m,
        rig.t0,
        7,
        args(rig.eps.slot as u64, rig.ns.slot as u64, info(0), 0, 0, 0),
        &mut env,
    );
    assert!(matches!(
        out,
        SyscallOutcome::Switched {
            state: TcbState::BlockedOnNotification { .. }
        }
    ));
    // Signal đánh thức đúng TCB.
    m.notification_signal(rig.ntfn, 0x1).expect("signal");
    assert_eq!(m.tcb_state(rig.t0), Ok(TcbState::Ready));
    assert!(m.check_notifications().is_ok());
}

// ---------------------------------------------------------------------------------------------
// 8) WaitAddr/WakeAddr/Clock/Put/Halt
// ---------------------------------------------------------------------------------------------

#[test]
fn kt62_waitaddr_wakeaddr_clock_put_halt() {
    let mut m = machine();
    let rig = setup(&mut m);
    let mut env = FakeEnv::new();
    m.set_addr_word(7, 64, 0x1234).expect("addr word");

    // WaitAddr khớp ⇒ chặn đúng khoá.
    let out = call(
        &mut m,
        rig.t0,
        9,
        args(0x4000_0000, 0x1234, 0, 0, 0, 0),
        &mut env,
    );
    assert_eq!(
        out,
        SyscallOutcome::Switched {
            state: TcbState::BlockedOnAddress {
                frame: 7,
                offset: 64
            }
        }
    );
    // WakeAddr(1) đánh thức đúng 1 waiter cùng khoá.
    match call(
        &mut m,
        rig.t0,
        10,
        args(0x4000_0000, 1, 0, 0, 0, 0),
        &mut env,
    ) {
        // TCB đang chặn: dispatcher vẫn vào core `wake_addr` (không cần TCB Running).
        SyscallOutcome::Return(r) => assert_eq!((r.error, r.value), (0, 1)),
        other => panic!("WakeAddr phải trả số đã đánh thức, thấy {other:?}"),
    }
    assert_eq!(m.tcb_state(rig.t0), Ok(TcbState::Ready));
    // Địa chỉ không dịch được (hook trả 11) ⇒ 11 `NotMapped`, không panic.
    env.addr = None;
    assert_eq!(
        err(call(
            &mut m,
            rig.t0,
            9,
            args(0xdead_0000, 0, 0, 0, 0, 0),
            &mut env
        )),
        KError::NotMapped.id() as u64
    );
    env.addr = Some((7, 64));
    // ClockRead đơn điệu (hook tăng 10 ns mỗi lần).
    let a = match call(&mut m, rig.t0, 11, args(0, 0, 0, 0, 0, 0), &mut env) {
        SyscallOutcome::Return(r) => r.value,
        other => panic!("clock, thấy {other:?}"),
    };
    let b = match call(&mut m, rig.t0, 11, args(0, 0, 0, 0, 0, 0), &mut env) {
        SyscallOutcome::Return(r) => r.value,
        other => panic!("clock, thấy {other:?}"),
    };
    assert!(b > a, "ClockRead phải đơn điệu: {a} → {b}");
    // DebugPutStr quá trần ⇒ 5; hợp lệ ⇒ số byte.
    assert_eq!(
        err(call(
            &mut m,
            rig.t0,
            12,
            args(0, 4097, 0, 0, 0, 0),
            &mut env
        )),
        KError::InvalidArgument.id() as u64
    );
    match call(&mut m, rig.t0, 12, args(0, 3, 0, 0, 0, 0), &mut env) {
        SyscallOutcome::Return(r) => assert_eq!((r.error, r.value), (0, 3)),
        other => panic!("put_str, thấy {other:?}"),
    }
    // DebugHalt chỉ trả outcome.
    assert_eq!(
        call(&mut m, rig.t0, 13, args(9, 0, 0, 0, 0, 0), &mut env),
        SyscallOutcome::Halt { code: 9 }
    );
    assert!(m.check_waitaddr().is_ok());
}

// ---------------------------------------------------------------------------------------------
// 9) Payload Notification của Recv/NBRecv/NBSendWait (F1 reviewer-kt62)
// ---------------------------------------------------------------------------------------------

#[test]
fn kt62_payload_notification() {
    let mut m = machine();
    let rig = setup(&mut m);
    let mut env = FakeEnv::new();

    // NBRecv (4) trên cap Notification: `word` (153) + `badge` (153) phải tới user, không phải
    // stale `ipc_area` của Endpoint.
    m.notification_signal(rig.ntfn, 153).expect("signal 153");
    match call(
        &mut m,
        rig.t0,
        4,
        args(rig.ns.slot as u64, 0, 0, 0, 0, 0),
        &mut env,
    ) {
        SyscallOutcome::Return(r) => {
            assert_eq!(r.error, 0, "NBRecv Notification");
            assert_eq!(r.value, 153, "a1 = word");
            assert_eq!(r.mr[0], 153, "MR0 = word");
            assert_eq!(r.mr[1], 153, "MR1 = badge");
        }
        other => panic!("NBRecv Notification phải trả về, thấy {other:?}"),
    }

    // Recv (3) trên Notification: word khác 0 ⇒ giao ngay (không chặn) + payload.
    m.notification_signal(rig.ntfn, 171).expect("signal 171");
    match call(
        &mut m,
        rig.t0,
        3,
        args(rig.ns.slot as u64, syscall::REPLY_NONE, 0, 0, 0, 0),
        &mut env,
    ) {
        SyscallOutcome::Return(r) => {
            assert_eq!(r.error, 0, "Recv Notification");
            assert_eq!((r.value, r.mr[0], r.mr[1]), (171, 171, 171));
        }
        other => panic!("Recv Notification phải trả về, thấy {other:?}"),
    }

    // NBSendWait (7): gửi Endpoint rồi Wait Notification đã signal ⇒ payload, không `value=0`.
    m.notification_signal(rig.ntfn, 205).expect("signal 205");
    match call(
        &mut m,
        rig.t0,
        7,
        args(rig.eps.slot as u64, rig.ns.slot as u64, info(0), 0, 0, 0),
        &mut env,
    ) {
        SyscallOutcome::Return(r) => {
            assert_eq!(r.error, 0, "NBSendWait");
            assert_eq!((r.value, r.mr[0], r.mr[1]), (205, 205, 205));
        }
        other => panic!("NBSendWait phải trả payload, thấy {other:?}"),
    }
    assert!(m.check_notifications().is_ok());
}
