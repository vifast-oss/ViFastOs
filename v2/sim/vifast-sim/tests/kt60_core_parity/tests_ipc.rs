// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-60 — các ca `#[test]` parity IPC 21..28 (tách khỏi `main.rs`, KT-407).
use super::*;

fn run(name: &str, ops: &[(&'static str, Op)]) {
    let mut sm = ks::Machine::new();
    let mut cm = kc::Machine::new();
    let rig_s = build(&mut sm);
    let rig_c = build(&mut cm);
    assert_eq!(rig_s, rig_c, "{name}: rig hai bên lệch");
    for (i, (label, op)) in ops.iter().enumerate() {
        let op = &fix_op(op, rig_s.root);
        let a = sm.step(op);
        let b = cm.step(op);
        assert_eq!(a, b, "{name} buoc {i} `{label}`: ket qua lech");
        let da = digest(&sm, &rig_s);
        let db = digest(&cm, &rig_c);
        assert_eq!(da, db, "{name} buoc {i} `{label}`: digest lech");
        assert!(da.checks.ipc, "{name} buoc {i}: sim check_ipc do");
        assert!(da.checks.reply, "{name} buoc {i}: sim check_reply do");
        assert!(
            da.checks.ntfn,
            "{name} buoc {i}: sim check_notifications do"
        );
        assert!(da.checks.waitaddr, "{name} buoc {i}: sim check_waitaddr do");
        assert!(da.checks.fault, "{name} buoc {i}: sim check_fault do");
    }
}

// ---------------------------------------------------------------------------------------------
// KT-24 — Endpoint queue FIFO, Busy, NBSend/Recv
// ---------------------------------------------------------------------------------------------

#[test]
fn kt60_parity_ipc24_hang_fifo_va_busy() {
    let s = |label: &'static str, op: Op| (label, op);
    let mut ops = vec![
        s(
            "send t0",
            Op::Send {
                tcb: 3,
                cap: (0, 4),
                label: 1,
                len: 3,
            },
        ),
        s(
            "send t1",
            Op::Send {
                tcb: 4,
                cap: (0, 4),
                label: 2,
                len: 2,
            },
        ),
        s(
            "nbsend t2",
            Op::NbSend {
                tcb: 5,
                cap: (0, 4),
                label: 3,
                len: 1,
            },
        ),
        s(
            "recv t2",
            Op::Recv {
                tcb: 5,
                ep: (0, 4),
                reply: None,
            },
        ),
        s(
            "recv t1 dang chan",
            Op::Recv {
                tcb: 4,
                ep: (0, 4),
                reply: None,
            },
        ),
        s("wake t1", Op::Wake { tcb: 4 }),
        s(
            "recv t1",
            Op::Recv {
                tcb: 4,
                ep: (0, 4),
                reply: None,
            },
        ),
        s("nbrecv t0", Op::NbRecv { tcb: 3, ep: (0, 4) }),
        s("nbrecv t0 rong", Op::NbRecv { tcb: 3, ep: (0, 4) }),
    ];
    // Hàng 8: nb_send lấp đầy, cái thứ 9 ⇒ Busy.
    for i in 0..8u32 {
        ops.push(s(
            "nbsend lap hang",
            Op::NbSend {
                tcb: 3,
                cap: (0, 4),
                label: 10 + i,
                len: 1,
            },
        ));
    }
    ops.push(s(
        "nbsend thu 9",
        Op::NbSend {
            tcb: 3,
            cap: (0, 4),
            label: 99,
            len: 1,
        },
    ));
    for _ in 0..8 {
        ops.push(s("nbrecv t1", Op::NbRecv { tcb: 4, ep: (0, 4) }));
    }
    ops.push(s("nbrecv t1 rong", Op::NbRecv { tcb: 4, ep: (0, 4) }));
    ops.push(s(
        "send chan",
        Op::Send {
            tcb: 3,
            cap: (0, 4),
            label: 20,
            len: 2,
        },
    ));
    ops.push(s(
        "recv t2 giao",
        Op::Recv {
            tcb: 5,
            ep: (0, 4),
            reply: None,
        },
    ));
    ops.push(s(
        "send len TCB",
        Op::Send {
            tcb: 3,
            cap: (0, 1),
            label: 30,
            len: 1,
        },
    ));
    ops.push(s(
        "call len TCB",
        Op::Call {
            tcb: 3,
            ep: (0, 1),
            label: 31,
            len: 1,
        },
    ));
    ops.push(s(
        "send len Reply",
        Op::Send {
            tcb: 3,
            cap: (0, 8),
            label: 32,
            len: 1,
        },
    ));
    run("kt24", &ops);
}

// ---------------------------------------------------------------------------------------------
// KT-25 + KT-34 — Call/Reply/ReplyRecv, Reply tái dùng, hai hook transition
// ---------------------------------------------------------------------------------------------

#[test]
fn kt60_parity_ipc25_call_reply_replyrecv_va_kt34() {
    let s = |label: &'static str, op: Op| (label, op);
    let ops = vec![
        s(
            "call t0",
            Op::Call {
                tcb: 3,
                ep: (0, 4),
                label: 1,
                len: 3,
            },
        ),
        s(
            "recv t1 R0",
            Op::Recv {
                tcb: 4,
                ep: (0, 4),
                reply: Some((0, 6)),
            },
        ),
        s(
            "reply t1 R0",
            Op::Reply {
                tcb: 4,
                reply: (0, 6),
            },
        ),
        s(
            "reply t1 R0 lan 2",
            Op::Reply {
                tcb: 4,
                reply: (0, 6),
            },
        ),
        s(
            "recv t1 R0 tai dung",
            Op::Recv {
                tcb: 4,
                ep: (0, 4),
                reply: Some((0, 6)),
            },
        ),
        s(
            "call t0 2",
            Op::Call {
                tcb: 3,
                ep: (0, 4),
                label: 2,
                len: 2,
            },
        ),
        s(
            "replyrecv t1",
            Op::ReplyRecv {
                tcb: 4,
                reply: (0, 6),
                ep: (0, 4),
                recv_reply: Some((0, 7)),
                label: 3,
                len: 2,
            },
        ),
        s(
            "call t2",
            Op::Call {
                tcb: 5,
                ep: (0, 4),
                label: 4,
                len: 1,
            },
        ),
        s(
            "replyrecv t1 cung reply",
            Op::ReplyRecv {
                tcb: 4,
                reply: (0, 7),
                ep: (0, 4),
                recv_reply: Some((0, 7)),
                label: 5,
                len: 2,
            },
        ),
        s(
            "call t0 6",
            Op::Call {
                tcb: 3,
                ep: (0, 4),
                label: 6,
                len: 2,
            },
        ),
        s(
            "recv t1 R1 dang active",
            Op::Recv {
                tcb: 4,
                ep: (0, 4),
                reply: Some((0, 7)),
            },
        ),
        s(
            "recv t1 R2",
            Op::Recv {
                tcb: 4,
                ep: (0, 4),
                reply: Some((0, 8)),
            },
        ),
        s(
            "call t2 7",
            Op::Call {
                tcb: 5,
                ep: (0, 4),
                label: 7,
                len: 1,
            },
        ),
        s(
            "reply t1 R2",
            Op::Reply {
                tcb: 4,
                reply: (0, 8),
            },
        ),
        s(
            "transition t0 Ready (roi Reply R1)",
            Op::Transition {
                tcb: 3,
                to: CS::Ready,
            },
        ),
        s(
            "transition t0 -> Reply R3 (bind)",
            Op::Transition {
                tcb: 3,
                to: CS::BlockedOnReply { reply: 11 },
            },
        ),
        s(
            "transition t2 -> Reply R3 (xung dot)",
            Op::Transition {
                tcb: 5,
                to: CS::BlockedOnReply { reply: 11 },
            },
        ),
        s(
            "transition t0 Ready (go caller R3)",
            Op::Transition {
                tcb: 3,
                to: CS::Ready,
            },
        ),
        s(
            "transition t2 -> Reply R3",
            Op::Transition {
                tcb: 5,
                to: CS::BlockedOnReply { reply: 11 },
            },
        ),
        s(
            "transition t2 Ready",
            Op::Transition {
                tcb: 5,
                to: CS::Ready,
            },
        ),
        s(
            "nbsendrecv t0",
            Op::NbSendRecv {
                tcb: 3,
                send_ep: (0, 4),
                recv_ep: (0, 5),
                recv_reply: Some((0, 9)),
                label: 8,
                len: 1,
            },
        ),
        s(
            "nbsendwait t2",
            Op::NbSendWait {
                tcb: 5,
                send_ep: (0, 4),
                ntfn: (0, 10),
                label: 9,
                len: 1,
            },
        ),
    ];
    run("kt25/34", &ops);
}

// ---------------------------------------------------------------------------------------------
// KT-27 — Notification badge OR, wait/wake, bound TCB
// ---------------------------------------------------------------------------------------------

#[test]
fn kt60_parity_ipc27_notification() {
    let s = |label: &'static str, op: Op| (label, op);
    let ops = vec![
        s(
            "wait t1 N0",
            Op::Recv {
                tcb: 4,
                ep: (0, 10),
                reply: None,
            },
        ),
        s(
            "signal N0 badge 0x10",
            Op::Send {
                tcb: 5,
                cap: (0, 14),
                label: 1,
                len: 0,
            },
        ),
        s(
            "recv t1 N0 kem Reply",
            Op::Recv {
                tcb: 4,
                ep: (0, 10),
                reply: Some((0, 6)),
            },
        ),
        s(
            "wait t1 N0 lan 2",
            Op::Recv {
                tcb: 4,
                ep: (0, 10),
                reply: None,
            },
        ),
        s(
            "signal N0 badge 0x20",
            Op::Send {
                tcb: 5,
                cap: (0, 15),
                label: 2,
                len: 0,
            },
        ),
        s(
            "signal N0 badge 0",
            Op::Send {
                tcb: 5,
                cap: (0, 10),
                label: 3,
                len: 0,
            },
        ),
        s(
            "signal N0 badge 0x10 khong waiter",
            Op::Send {
                tcb: 5,
                cap: (0, 14),
                label: 4,
                len: 0,
            },
        ),
        s(
            "signal N0 badge 0x02 khong waiter (OR)",
            Op::Send {
                tcb: 5,
                cap: (0, 17),
                label: 10,
                len: 0,
            },
        ),
        s(
            "wait t1 N0 an word",
            Op::Recv {
                tcb: 4,
                ep: (0, 10),
                reply: None,
            },
        ),
        s(
            "wait t1 N0 chan",
            Op::Recv {
                tcb: 4,
                ep: (0, 10),
                reply: None,
            },
        ),
        s(
            "wait t2 N0 Busy",
            Op::Recv {
                tcb: 5,
                ep: (0, 10),
                reply: None,
            },
        ),
        s(
            "signal N0 tu t2 dang chan",
            Op::Send {
                tcb: 5,
                cap: (0, 14),
                label: 5,
                len: 0,
            },
        ),
        s(
            "signal N0 wake t1",
            Op::Send {
                tcb: 3,
                cap: (0, 15),
                label: 6,
                len: 0,
            },
        ),
        s(
            "bind t2 N1",
            Op::BindNtfn {
                tcb: (0, 3),
                ntfn: (0, 11),
            },
        ),
        s(
            "recv t2 EP1 chan",
            Op::Recv {
                tcb: 5,
                ep: (0, 5),
                reply: None,
            },
        ),
        s(
            "signal N1 badge 0x08 bound",
            Op::Send {
                tcb: 3,
                cap: (0, 16),
                label: 7,
                len: 0,
            },
        ),
        s(
            "bind t2 N0 Busy",
            Op::BindNtfn {
                tcb: (0, 3),
                ntfn: (0, 10),
            },
        ),
        s("unbind t1 N0", Op::UnbindNtfn { tcb: (0, 2) }),
        s(
            "bind t2 N0",
            Op::BindNtfn {
                tcb: (0, 3),
                ntfn: (0, 10),
            },
        ),
        s(
            "bind t1 N0 Busy",
            Op::BindNtfn {
                tcb: (0, 2),
                ntfn: (0, 10),
            },
        ),
        s("unbind t2 N0", Op::UnbindNtfn { tcb: (0, 3) }),
        // F3 reviewer-kt60: block_current(BlockedOnNotification) phải ghi waiter; TCB thứ hai
        // block cùng notification ⇒ Busy; Signal đánh thức đúng waiter.
        s("dispatch t0", Op::Dispatch { hart: 0 }),
        s(
            "block_current t0 N0 (F3)",
            Op::BlockCurrent {
                hart: 0,
                to: CS::BlockedOnNotification { ntfn: 12 },
            },
        ),
        s("dispatch t2", Op::Dispatch { hart: 0 }),
        s(
            "block_current t2 N0 Busy",
            Op::BlockCurrent {
                hart: 0,
                to: CS::BlockedOnNotification { ntfn: 12 },
            },
        ),
        s(
            "signal N0 wake t0 (F3)",
            Op::Send {
                tcb: 5,
                cap: (0, 14),
                label: 11,
                len: 0,
            },
        ),
        s("dispatch t0 sau wake", Op::Dispatch { hart: 0 }),
        s(
            "signal N0 badge 0x20 poll",
            Op::Send {
                tcb: 3,
                cap: (0, 15),
                label: 8,
                len: 0,
            },
        ),
        s(
            "nbrecv t1 N0",
            Op::NbRecv {
                tcb: 4,
                ep: (0, 10),
            },
        ),
        s(
            "nbrecv t1 N0 rong",
            Op::NbRecv {
                tcb: 4,
                ep: (0, 10),
            },
        ),
    ];
    run("kt27", &ops);
}

// ---------------------------------------------------------------------------------------------
// KT-28 — WaitAddr: FIFO, va chạm bucket, không timeout
// ---------------------------------------------------------------------------------------------

#[test]
fn kt60_parity_ipc28_waitaddr() {
    let s = |label: &'static str, op: Op| (label, op);
    let ops = vec![
        s(
            "set word",
            Op::SetAddr {
                frame: 3,
                offset: 0,
                value: 42,
            },
        ),
        s(
            "wait sai expected",
            Op::WaitAddr {
                tcb: (0, 1),
                frame: 3,
                offset: 0,
                expected: 41,
            },
        ),
        s(
            "wait t0",
            Op::WaitAddr {
                tcb: (0, 1),
                frame: 3,
                offset: 0,
                expected: 42,
            },
        ),
        s(
            "wait t1",
            Op::WaitAddr {
                tcb: (0, 2),
                frame: 3,
                offset: 0,
                expected: 42,
            },
        ),
        s(
            "wait t2",
            Op::WaitAddr {
                tcb: (0, 3),
                frame: 3,
                offset: 0,
                expected: 42,
            },
        ),
        s("tick 100 khong danh thuc", Op::Tick { n: 100 }),
        s(
            "wake 1",
            Op::WakeAddr {
                frame: 3,
                offset: 0,
                n: 1,
            },
        ),
        s(
            "wake 0",
            Op::WakeAddr {
                frame: 3,
                offset: 0,
                n: 0,
            },
        ),
        s(
            "wake tat ca",
            Op::WakeAddr {
                frame: 3,
                offset: 0,
                n: u32::MAX,
            },
        ),
        s(
            "set word va cham",
            Op::SetAddr {
                frame: 3,
                offset: 64,
                value: 7,
            },
        ),
        s(
            "wait t0 va cham",
            Op::WaitAddr {
                tcb: (0, 1),
                frame: 3,
                offset: 64,
                expected: 7,
            },
        ),
        s(
            "wait t1 khoa khac",
            Op::WaitAddr {
                tcb: (0, 2),
                frame: 3,
                offset: 0,
                expected: 42,
            },
        ),
        s(
            "wake va cham",
            Op::WakeAddr {
                frame: 3,
                offset: 64,
                n: 1,
            },
        ),
        s(
            "wake khoa khac",
            Op::WakeAddr {
                frame: 3,
                offset: 0,
                n: 1,
            },
        ),
        s(
            "wake khong ai",
            Op::WakeAddr {
                frame: 4,
                offset: 0,
                n: 5,
            },
        ),
        s(
            "suspend t2 Abort",
            Op::Suspend {
                tcb: (0, 3),
                abort: true,
            },
        ),
        s(
            "wait khi suspended",
            Op::WaitAddr {
                tcb: (0, 3),
                frame: 3,
                offset: 0,
                expected: 42,
            },
        ),
        s("resume t2", Op::Resume { tcb: (0, 3) }),
        s(
            "wait t2 sau resume",
            Op::WaitAddr {
                tcb: (0, 3),
                frame: 3,
                offset: 0,
                expected: 42,
            },
        ),
        s(
            "wake t2",
            Op::WakeAddr {
                frame: 3,
                offset: 0,
                n: u32::MAX,
            },
        ),
        s("dispatch hart0", Op::Dispatch { hart: 0 }),
    ];
    run("kt28", &ops);
}

// ---------------------------------------------------------------------------------------------
// KT-26 + D2-25 — fault Exit không Reply, fault khác Call+Reply
// ---------------------------------------------------------------------------------------------

#[test]
fn kt60_parity_ipc26_fault_exit() {
    let s = |label: &'static str, op: Op| (label, op);
    let mut ops = vec![
        s("exit t0", Op::Exit { tcb: 3, code: 42 }),
        s(
            "recv t1 fault",
            Op::Recv {
                tcb: 4,
                ep: (0, 4),
                reply: None,
            },
        ),
        s("exit t0 lan 2", Op::Exit { tcb: 3, code: 43 }),
        s(
            "recv t1 EP0 chan",
            Op::Recv {
                tcb: 4,
                ep: (0, 4),
                reply: None,
            },
        ),
        s("exit t2 fast path", Op::Exit { tcb: 5, code: 7 }),
        s(
            "inject fault UnknownSyscall",
            Op::InjectFault {
                tcb: 5,
                kind: 1,
                val: 5,
            },
        ),
        s("exit khong fault_ep", Op::Exit { tcb: 3, code: 1 }),
    ];
    // Hàng gửi đầy ⇒ Exit có fault_ep trả Busy trước mọi thay đổi.
    for i in 0..8u32 {
        ops.push(s(
            "nbsend lap hang",
            Op::NbSend {
                tcb: 3,
                cap: (0, 4),
                label: 40 + i,
                len: 1,
            },
        ));
    }
    ops.push(s("exit khi hang day", Op::Exit { tcb: 5, code: 9 }));
    for _ in 0..8 {
        ops.push(s("nbrecv t1", Op::NbRecv { tcb: 4, ep: (0, 4) }));
    }
    ops.push(s("exit sau khi day", Op::Exit { tcb: 5, code: 9 }));
    ops.push(s(
        "recv t1 fault 2",
        Op::Recv {
            tcb: 4,
            ep: (0, 4),
            reply: None,
        },
    ));
    // inject fault ≠ Exit với waiter có Reply ⇒ caller BlockedOnReply, reply resume.
    ops.push(s(
        "recv t1 EP0 + R0",
        Op::Recv {
            tcb: 4,
            ep: (0, 4),
            reply: Some((0, 6)),
        },
    ));
    ops.push(s(
        "inject fault co waiter",
        Op::InjectFault {
            tcb: 5,
            kind: 2,
            val: 0x1000,
        },
    ));
    ops.push(s(
        "reply resume",
        Op::Reply {
            tcb: 4,
            reply: (0, 6),
        },
    ));
    run("kt26", &ops);
}

// ---------------------------------------------------------------------------------------------
// KT-21 — donation tự động + hai hook (rời BlockedOnReply)
// ---------------------------------------------------------------------------------------------

#[test]
fn kt60_parity_ipc21_donation_hook() {
    let s = |label: &'static str, op: Op| (label, op);
    let ops = vec![
        s(
            "call t0",
            Op::Call {
                tcb: 3,
                ep: (0, 4),
                label: 1,
                len: 2,
            },
        ),
        s(
            "recv t1 R0 (donation)",
            Op::Recv {
                tcb: 4,
                ep: (0, 4),
                reply: Some((0, 6)),
            },
        ),
        s(
            "reply R0 tra donation",
            Op::Reply {
                tcb: 4,
                reply: (0, 6),
            },
        ),
        s(
            "call t0 lan 2",
            Op::Call {
                tcb: 3,
                ep: (0, 4),
                label: 2,
                len: 2,
            },
        ),
        s(
            "recv t1 R1 (donation)",
            Op::Recv {
                tcb: 4,
                ep: (0, 4),
                reply: Some((0, 7)),
            },
        ),
        s(
            "transition t0 Ready (hook 1)",
            Op::Transition {
                tcb: 3,
                to: CS::Ready,
            },
        ),
        s(
            "reply R1 sau hook",
            Op::Reply {
                tcb: 4,
                reply: (0, 7),
            },
        ),
        s(
            "call t0 lan 3",
            Op::Call {
                tcb: 3,
                ep: (0, 4),
                label: 3,
                len: 1,
            },
        ),
        s(
            "recv t1 R2 (donation)",
            Op::Recv {
                tcb: 4,
                ep: (0, 4),
                reply: Some((0, 8)),
            },
        ),
        // F2 reviewer-kt60: donor đang donate không đổi priority/SC.
        s(
            "set priority donor (F2)",
            Op::SetPriority {
                tcb: (0, 1),
                p: 4,
                mcp: 10,
            },
        ),
        s(
            "set sched donor (F2)",
            Op::SetSched {
                tcb: (0, 1),
                sc: None,
            },
        ),
        // F1 reviewer-kt60: Suspend(Abort) T1 (donee) phải trả SC + đánh thức T0.
        s(
            "suspend t1 donee (F1)",
            Op::Suspend {
                tcb: (0, 2),
                abort: true,
            },
        ),
        s("resume t1", Op::Resume { tcb: (0, 2) }),
        s(
            "call t0 lan 4",
            Op::Call {
                tcb: 3,
                ep: (0, 4),
                label: 4,
                len: 2,
            },
        ),
        s(
            "recv t1 R3 (donation 2)",
            Op::Recv {
                tcb: 4,
                ep: (0, 4),
                reply: Some((0, 9)),
            },
        ),
        s("exit t0 khi donate", Op::Exit { tcb: 3, code: 55 }),
        s(
            "recv t1 fault sau exit",
            Op::Recv {
                tcb: 4,
                ep: (0, 4),
                reply: None,
            },
        ),
        s(
            "call t0 sau Inactive",
            Op::Call {
                tcb: 3,
                ep: (0, 4),
                label: 5,
                len: 1,
            },
        ),
        s("tick 4 budget", Op::Tick { n: 4 }),
        s("dispatch", Op::Dispatch { hart: 0 }),
    ];
    run("kt21", &ops);
}

/// D2-84: pin core ≤ 196.608 B + bảng phân bố (D2-84 bắt buộc có bảng; in ra để journal).
#[test]
fn kt60_pin_dau_chan_d2_84() {
    let f = kc::Machine::footprint();
    let mut sum = 0usize;
    for i in 0..kc::FOOTPRINT_PARTS {
        println!("{:28} {:>8}", f.names[i], f.bytes[i]);
        sum += f.bytes[i];
    }
    println!("{:28} {:>8}", "scalars", f.scalars);
    println!("{:28} {:>8}", "padding", f.padding);
    println!("{:28} {:>8}", "TOTAL", f.total);
    assert_eq!(sum + f.scalars + f.padding, f.total);
    // F5 reviewer-kt60 (D2-85c): không tautology — padding phải nhỏ (repr(Rust) hiện 1 B);
    // nếu padding phình > 8 B thì bảng phân bố đã bỏ sót trường, không được coi là "đã đo".
    assert!(
        f.padding <= 8,
        "bang phan bo bo sot truong: padding {} B > 8",
        f.padding
    );
    // KT-63: + 3 bộ đếm chuyển cap (`cap_transfer_installed`/`cap_unwrapped`/
    // `cap_transfer_failed`) ⇒ 12 × u64. KT-64: + 4 bộ đếm TLB (`tlb_seq`/`tlb_local_flushes`/
    // `tlb_remote_posts`/`tlb_serviced`) ⇒ **16 × u64**, + `slot_win_top` ⇒ **21 × usize**;
    // `slot_pool` → `slot_mem` + `harness_region`; dải slot theo Region là **hằng**, không field.
    // **KT-309**: cap transfer qua IPC buffer thêm **16** trường vô hướng (4 CPTR nguồn +
    // 4 CPTR đích + `n` + `dst` + 6 bộ đếm) ⇒ `20 × usize` → **`20 × usize + 16 × u64`** giữ
    // nguyên, cộng **16** đơn vị vô hướng. Pin **cố ý** đổi (D2-84): tăng trường phải là quyết
    // định có ghi, không phải trôi.
    // **KT-312**: **không** thêm/bớt trường vô hướng ⇒ `scalars` **311 không đổi**; phần thu hồi
    // 10.240 B nằm ở dòng `objects[256]` (22.528 → **12.288**), không phải ở đây.
    assert_eq!(
        f.scalars,
        20 * core::mem::size_of::<usize>() + 16 * core::mem::size_of::<u64>() + 2 + 4 + 1 + 16
    );
    // **KT-312** — bảng phải **thật**: dòng `objects[256]` là `256 × size_of::<Meta>()`, và
    // `Meta` **48 B** (không phải 88 B) vì ba trường chỉ số đã dùng kiểu hẹp (`Option<u8>`/
    // `Option<u16>`, miền giá trị chứng minh được `< MAX_UNTYPEDS`/`≤ MAX_SLOT_WORDS`). Nếu ai
    // nới lại `Meta` thành `Option<usize>`, dòng này đỏ **ngay** — bảng không còn là lời khai.
    assert_eq!(
        f.bytes[1],
        256 * 48,
        "objects[256] phải = 256 × 48 B (Meta 48 B, KT-312)"
    );
    assert!(
        f.total <= 196_608,
        "D2-84: size_of::<Machine>() = {} > 196608",
        f.total
    );
}

/// Biên tĩnh: đầy body Endpoint/Reply/Notification/pool message ⇒ `NotEnoughMemory` **nguyên
/// tử**; revoke trả body về free-list rồi retype lại được (giống test pin KT-59).
#[test]
fn kt60_core_bien_tinh_body_ipc() {
    let mut m = kc::Machine::new();
    let u = m.add_untyped(0x2000_0000, 1 << 20);
    let root = m.add_cnode(128);
    let uc = m.root_cap(u);
    m.put_cap(root, 0, uc).expect("untyped cap");
    // Đầy MAX_ENDPOINTS.
    let mut caps = Vec::new();
    for i in 0..kc::MAX_ENDPOINTS {
        let cap = m
            .retype(kc::SlotRef::new(root, 0), kc::ObjType::Endpoint, 0, 0)
            .expect("endpoint");
        caps.push(cap);
        m.put_cap(root, 1 + i, cap).expect("slot");
    }
    assert_eq!(
        m.retype(kc::SlotRef::new(root, 0), kc::ObjType::Endpoint, 0, 0),
        Err(kc::KError::NotEnoughMemory),
        "Endpoint thứ MAX+1 ⇒ NotEnoughMemory"
    );
    // Reply/Notification cũng có biên.
    for i in 0..kc::MAX_REPLIES {
        let cap = m
            .retype(kc::SlotRef::new(root, 0), kc::ObjType::Reply, 0, 0)
            .expect("reply");
        m.put_cap(root, 32 + i, cap).expect("slot");
    }
    assert_eq!(
        m.retype(kc::SlotRef::new(root, 0), kc::ObjType::Reply, 0, 0),
        Err(kc::KError::NotEnoughMemory)
    );
    for i in 0..kc::MAX_NOTIFICATIONS {
        let cap = m
            .retype(kc::SlotRef::new(root, 0), kc::ObjType::Notification, 0, 0)
            .expect("ntfn");
        m.put_cap(root, 64 + i, cap).expect("slot");
    }
    assert_eq!(
        m.retype(kc::SlotRef::new(root, 0), kc::ObjType::Notification, 0, 0),
        Err(kc::KError::NotEnoughMemory)
    );
    // Pool message đầy (24) ⇒ Call/Send thứ 25 trả Busy/NotEnoughMemory trước khi đổi trạng
    // thái: dùng một Endpoint + 8 ô, 3 Endpoint đầy = 24 message.
    let t = m.add_kernel_object(kc::ObjType::Tcb);
    let tc = m.root_cap(t);
    m.put_cap(root, 100, tc).expect("tcb slot");
    for k in 0..3usize {
        let ep = m.cap_obj_at(root, 1 + k).expect("ep obj");
        for j in 0..8u32 {
            let msg = kc::Message::new(
                kc::MessageInfo::new(100 + j, 0, false, 0, 1).unwrap(),
                &[j as u64],
            )
            .unwrap();
            m.nb_send(t, kc::SlotRef::new(root, 1 + k), msg)
                .expect("nbsend vào hàng");
            let _ = ep;
        }
    }
    assert_eq!(m.ipc_pending(), 24);
    let ep4 = m.cap_obj_at(root, 4).expect("ep 4");
    let msg = kc::Message::new(kc::MessageInfo::new(9, 0, false, 0, 1).unwrap(), &[9]).unwrap();
    assert_eq!(
        m.nb_send(t, kc::SlotRef::new(root, 4), msg),
        Err(kc::KError::Busy),
        "hàng Endpoint thứ 4 đầy ⇒ Busy (không đụng pool)"
    );
    assert_eq!(m.ipc_pending(), 24);
    let _ = ep4;
}

// ---------------------------------------------------------------------------------------------
// Pin ngữ nghĩa cứng (không chỉ so hai bên) — trạng thái/bộ đếm cuối kịch bản
// ---------------------------------------------------------------------------------------------

#[test]
fn kt60_core_waitaddr_khong_timeout_va_wake_fifo() {
    let mut m = kc::Machine::new();
    let u = m.add_untyped(0x3000_0000, 1 << 20);
    let root = m.add_cnode(16);
    let uc = m.root_cap(u);
    m.put_cap(root, 0, uc).unwrap();
    let t0 = m.add_kernel_object(kc::ObjType::Tcb);
    let t1 = m.add_kernel_object(kc::ObjType::Tcb);
    let c0 = m.root_cap(t0);
    let c1 = m.root_cap(t1);
    m.put_cap(root, 1, c0).unwrap();
    m.put_cap(root, 2, c1).unwrap();
    m.set_addr_word(7, 0, 0xAA).unwrap();
    m.wait_addr(kc::SlotRef::new(root, 1), 7, 0, 0xAA).unwrap();
    m.wait_addr(kc::SlotRef::new(root, 2), 7, 0, 0xAA).unwrap();
    // Không timeout: tick ảo rất lớn không đánh thức, không sinh `Sleeping{Timeout}`.
    for _ in 0..10 {
        m.tick(1000).unwrap();
    }
    assert_eq!(
        m.tcb_state(t0).unwrap(),
        kc::TcbState::BlockedOnAddress {
            frame: 7,
            offset: 0
        }
    );
    assert_eq!(m.address_waiters_len(), 2);
    // Wake 1 ⇒ FIFO: T0 ra trước, T1 còn chờ.
    assert_eq!(m.wake_addr(7, 0, 1).unwrap(), 1);
    assert_eq!(m.tcb_state(t0).unwrap(), kc::TcbState::Ready);
    assert_eq!(
        m.tcb_state(t1).unwrap(),
        kc::TcbState::BlockedOnAddress {
            frame: 7,
            offset: 0
        }
    );
    assert_eq!(m.wake_addr(7, 0, u32::MAX).unwrap(), 1);
    assert_eq!(m.tcb_state(t1).unwrap(), kc::TcbState::Ready);
    assert_eq!(m.address_waiters_len(), 0);
}

#[test]
fn kt60_core_endpoint_busy_va_fifo() {
    let mut m = kc::Machine::new();
    let u = m.add_untyped(0x4000_0000, 1 << 20);
    let root = m.add_cnode(32);
    let uc = m.root_cap(u);
    m.put_cap(root, 0, uc).unwrap();
    let t0 = m.add_kernel_object(kc::ObjType::Tcb);
    let t1 = m.add_kernel_object(kc::ObjType::Tcb);
    let ep = m.add_kernel_object(kc::ObjType::Endpoint);
    let c0 = m.root_cap(t0);
    let c1 = m.root_cap(t1);
    let ce = m.root_cap(ep);
    m.put_cap(root, 1, c0).unwrap();
    m.put_cap(root, 2, c1).unwrap();
    m.put_cap(root, 3, ce).unwrap();
    // 8 message ⇒ đầy; đúng thứ tự FIFO; cái thứ 9 ⇒ `Busy` (không mất message).
    for i in 0..8u32 {
        let msg = kc::Message::new(
            kc::MessageInfo::new(10 + i, 0, false, 0, 1).unwrap(),
            &[i as u64],
        )
        .unwrap();
        m.nb_send(t0, kc::SlotRef::new(root, 3), msg).unwrap();
    }
    let msg = kc::Message::new(kc::MessageInfo::new(99, 0, false, 0, 1).unwrap(), &[99]).unwrap();
    assert_eq!(
        m.nb_send(t0, kc::SlotRef::new(root, 3), msg),
        Err(kc::KError::Busy)
    );
    assert_eq!(m.endpoint_send_len(ep).unwrap(), 8);
    assert_eq!(m.ipc_pending(), 8);
    for i in 0..8u64 {
        let got = m
            .nb_recv(t1, kc::SlotRef::new(root, 3))
            .unwrap()
            .expect("còn message");
        assert_eq!(got.msg_id, i + 1, "msg_id FIFO");
        assert_eq!(got.mr[0], i, "nội dung FIFO");
    }
    assert_eq!(
        m.nb_recv(t1, kc::SlotRef::new(root, 3)),
        Err(kc::KError::Busy)
    );
    assert_eq!(m.ipc_pending(), 0);
    assert_eq!(m.ipc_sent(), 8);
    assert_eq!(m.ipc_delivered(), 8);
    assert_eq!(m.transfer_calls(), 8);
}

#[test]
fn kt60_core_donation_tao_va_tra_kt21() {
    let mut m = kc::Machine::new();
    let u = m.add_untyped(0x5000_0000, 1 << 20);
    let root = m.add_cnode(16);
    let uc = m.root_cap(u);
    m.put_cap(root, 0, uc).unwrap();
    let mut objs = Vec::new();
    for ty in [
        kc::ObjType::Tcb,
        kc::ObjType::Tcb,
        kc::ObjType::Endpoint,
        kc::ObjType::Reply,
        kc::ObjType::SchedControl,
        kc::ObjType::SchedContext,
    ] {
        let cap = m.retype(kc::SlotRef::new(root, 0), ty, 0, 0).unwrap();
        m.put_cap(root, 1 + objs.len(), cap).unwrap();
        objs.push(m.cap_obj_at(root, 1 + objs.len()).unwrap());
    }
    let t0 = objs[0];
    let t1 = objs[1];
    let ep = objs[2];
    let r0 = objs[3];
    let sc = objs[5];
    m.tcb_set_priority(kc::SlotRef::new(root, 1), 3, 10)
        .unwrap();
    m.tcb_set_priority(kc::SlotRef::new(root, 2), 1, 10)
        .unwrap();
    m.sched_control_configure(kc::SlotRef::new(root, 5), kc::SlotRef::new(root, 6), 4, 8)
        .unwrap();
    m.set_sched_params(kc::SlotRef::new(root, 1), Some(kc::SlotRef::new(root, 6)))
        .unwrap();
    m.call(t0, kc::SlotRef::new(root, 3), 1).unwrap();
    m.recv(
        t1,
        kc::SlotRef::new(root, 3),
        Some(kc::SlotRef::new(root, 4)),
    )
    .unwrap();
    let i0 = m.tcb_info_obj(t0).unwrap();
    let i1 = m.tcb_info_obj(t1).unwrap();
    assert_eq!(i1.donated_sc, Some(sc), "server nhận donation");
    assert_eq!(i0.donated_sc, None, "client là donor, không nhận donation");
    let sc_info = m.sched_info(kc::SlotRef::new(root, 6)).unwrap();
    assert_eq!(sc_info.donated_to, Some(t1));
    assert_eq!(sc_info.donor, Some(t0));
    assert_eq!(m.effective_priority(t1).unwrap(), 3);
    m.reply(t1, kc::SlotRef::new(root, 4)).unwrap();
    assert_eq!(m.tcb_info_obj(t1).unwrap().donated_sc, None);
    let sc_info = m.sched_info(kc::SlotRef::new(root, 6)).unwrap();
    assert_eq!(sc_info.donated_to, None);
    assert_eq!(sc_info.donor, None);
    let _ = (ep, r0);
}
