// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-59 — các ca `#[test]` parity scheduler/budget/migration (tách khỏi `main.rs`, KT-407).
use super::*;

#[test]
fn kt59_parity_tcb_va_bang_allowed() {
    let (mut s, mut c, r) = rig(1, 3);
    // Defaults (Inactive, priority 0, mcp 0, affinity 1, home 0, không SC).
    for (i, &t) in r.tcbs.iter().enumerate() {
        let a = View::tcb_info(&s, r.tslots[i]).expect("sim info");
        let b = View::tcb_info(&c, r.tslots[i]).expect("core info");
        assert_eq!(a, b, "TCB {t}: InfoFp lệch");
        assert_eq!(a.state, CS::Inactive);
        assert!(!a.suspended);
        assert_eq!(a.priority, 0);
        assert_eq!(a.max_controlled_priority, 0);
        assert_eq!(a.affinity, 1);
        assert_eq!(a.home, 0);
        assert_eq!(a.sched_context, None);
        assert_eq!(t, r.tcbs[i]);
    }
    // Configure: gắn CSpace + xoá exit_code; lỗi cap sai kiểu.
    let steps = [
        // SC đã Configure cho t0 — bất biến "Running ⇒ còn budget" (KT-20).
        ok(
            Op::ScConfigure {
                ctl: r.ctl,
                sc: r.scs[0],
                budget: 64,
                period: 64,
            },
            0,
        ),
        ok(
            Op::SetSched {
                tcb: r.tslots[0],
                sc: Some(r.scs[0]),
            },
            0,
        ),
        ok(
            Op::Configure {
                tcb: r.tslots[0],
                cspace: Some(r.cspace),
            },
            0,
        ),
        err(
            Op::Configure {
                tcb: r.ep,
                cspace: None,
            },
            kc::KError::WrongObjectType,
        ),
        err(
            Op::Configure {
                tcb: r.tslots[0],
                cspace: Some(r.ep),
            },
            kc::KError::WrongObjectType,
        ),
        ok(
            Op::SetFaultEp {
                tcb: r.tslots[0],
                ep: r.ep,
            },
            0,
        ),
        err(
            Op::SetFaultEp {
                tcb: r.tslots[0],
                ep: r.cspace,
            },
            kc::KError::WrongObjectType,
        ),
        // Inactive → Running cấm (phải qua Ready) — ca âm chuẩn KT-18.
        err(
            Op::Transition {
                tcb: r.tcbs[0],
                st: CS::Running { hart: 0 },
            },
            kc::KError::IllegalOperation,
        ),
        // Chuỗi ALLOWED.
        ok(
            Op::Transition {
                tcb: r.tcbs[0],
                st: CS::Ready,
            },
            0,
        ),
        ok(
            Op::Transition {
                tcb: r.tcbs[0],
                st: CS::BlockedOnSend {
                    ep: r.ep.0,
                    call: true,
                },
            },
            0,
        ),
        ok(
            Op::Transition {
                tcb: r.tcbs[0],
                st: CS::BlockedOnReply { reply: r.ep.0 },
            },
            0,
        ),
        ok(
            Op::Transition {
                tcb: r.tcbs[0],
                st: CS::Ready,
            },
            0,
        ),
        ok(
            Op::Transition {
                tcb: r.tcbs[0],
                st: CS::BlockedOnRecv {
                    ep: r.ep.0,
                    reply: r.ep.0,
                },
            },
            0,
        ),
        ok(
            Op::Transition {
                tcb: r.tcbs[0],
                st: CS::Ready,
            },
            0,
        ),
        ok(
            Op::Transition {
                tcb: r.tcbs[0],
                st: CS::BlockedOnNotification { ntfn: r.ep.0 },
            },
            0,
        ),
        ok(
            Op::Transition {
                tcb: r.tcbs[0],
                st: CS::Ready,
            },
            0,
        ),
        ok(
            Op::Transition {
                tcb: r.tcbs[0],
                st: CS::BlockedOnAddress {
                    frame: r.ep.0,
                    offset: 4,
                },
            },
            0,
        ),
        ok(
            Op::Transition {
                tcb: r.tcbs[0],
                st: CS::Ready,
            },
            0,
        ),
        // Ready → Sleeping bị cấm; phải qua Running (bảng §4.5).
        err(
            Op::Transition {
                tcb: r.tcbs[0],
                st: CS::Sleeping { reason: 1 },
            },
            kc::KError::IllegalOperation,
        ),
        ok(
            Op::Transition {
                tcb: r.tcbs[0],
                st: CS::Running { hart: 0 },
            },
            0,
        ),
        ok(
            Op::Transition {
                tcb: r.tcbs[0],
                st: CS::Sleeping { reason: 1 },
            },
            0,
        ),
        // Sleeping + suspended: Ready không vào hàng (D2-33).
        ok(
            Op::Suspend {
                tcb: r.tslots[0],
                abort: true,
            },
            0,
        ),
        ok(
            Op::Transition {
                tcb: r.tcbs[0],
                st: CS::Ready,
            },
            0,
        ),
        ok(Op::Resume { tcb: r.tslots[0] }, 0),
        // Suspend Defer luôn InvalidArgument; Inactive/Dead ⇒ IllegalOperation.
        err(
            Op::Suspend {
                tcb: r.tslots[1],
                abort: false,
            },
            kc::KError::InvalidArgument,
        ),
        err(
            Op::Suspend {
                tcb: r.tslots[1],
                abort: true,
            },
            kc::KError::IllegalOperation,
        ),
        ok(Op::Finalise { tcb: r.tcbs[0] }, 0),
        err(
            Op::Suspend {
                tcb: r.tslots[0],
                abort: true,
            },
            kc::KError::IllegalOperation,
        ),
        err(
            Op::Resume { tcb: r.tslots[0] },
            kc::KError::IllegalOperation,
        ),
        // Dead không ra được.
        err(
            Op::Transition {
                tcb: r.tcbs[0],
                st: CS::Ready,
            },
            kc::KError::IllegalOperation,
        ),
        // TCB chưa kích hoạt vẫn finalise được; log finalise ghi đúng một dòng.
        ok(Op::Finalise { tcb: r.tcbs[1] }, 0),
    ];
    run_steps(&mut s, &mut c, &r, &steps);
    assert_eq!(s.finalise_log().len(), 2);
    assert_eq!(c.finalise_log().len(), 2);
}

// ---------------------------------------------------------------------------------------------
// 2. KT-19: priority/FIFO/round-robin/dispatch/yield + preempt
// ---------------------------------------------------------------------------------------------

#[test]
fn kt59_parity_scheduler_fifo_round_robin() {
    let (mut s, mut c, r) = rig(1, 3);
    arm_rig(&mut s, &mut c, &r, 64, 64);
    let steps = [
        // FIFO cùng mức: t0 chạy trước, yield xoay vòng t1, t2, t0.
        ok(Op::Dispatch { h: 0 }, r.tcbs[0] as u64),
        ok(Op::Yield { h: 0 }, r.tcbs[1] as u64),
        ok(Op::Dispatch { h: 0 }, r.tcbs[2] as u64),
        ok(Op::Yield { h: 0 }, r.tcbs[0] as u64),
        // Đổi priority: MCP < priority ⇒ InvalidArgument; đổi hợp lệ rebucket.
        err(
            Op::SetPriority {
                tcb: r.tslots[1],
                p: 9,
                mcp: 3,
            },
            kc::KError::InvalidArgument,
        ),
        ok(
            Op::SetPriority {
                tcb: r.tslots[1],
                p: 9,
                mcp: 255,
            },
            0,
        ),
        ok(Op::Dispatch { h: 0 }, r.tcbs[1] as u64),
        // block_current: chỉ nhận trạng thái chặn/ngủ.
        err(
            // **KT-241** — `Ready` **không** biểu diễn được ở kiểu đích nữa; dùng
            // `Running` để giữ đúng phép kiểm **runtime** "không phải đích block hợp lệ"
            // (D2-194 §1(d)) — vẫn kỳ vọng `InvalidArgument`.
            Op::BlockCurrent {
                h: 0,
                st: CS::Running { hart: 0 },
            },
            kc::KError::InvalidArgument,
        ),
        ok(
            Op::BlockCurrent {
                h: 0,
                st: CS::Sleeping { reason: 0 },
            },
            r.tcbs[1] as u64,
        ),
        ok(Op::Wake { tcb: r.tcbs[1] }, 0),
        // Preempt mỗi bước: hai TCB cùng mức xen kẽ, không đổi ai.
        plain(Op::SetPreempt { on: true }),
        ok(Op::Dispatch { h: 0 }, r.tcbs[1] as u64),
        ok(Op::Yield { h: 0 }, r.tcbs[1] as u64),
        plain(Op::SetPreempt { on: false }),
        // Affinity/home: mask 0 / bit lạ / home ngoài affinity ⇒ InvalidArgument.
        err(
            Op::SetAffinity {
                tcb: r.tslots[0],
                mask: 0,
            },
            kc::KError::InvalidArgument,
        ),
        err(
            Op::SetAffinity {
                tcb: r.tslots[0],
                mask: 0x3,
            },
            kc::KError::InvalidArgument,
        ),
        err(
            Op::SetHome {
                tcb: r.tslots[0],
                home: 1,
            },
            kc::KError::InvalidArgument,
        ),
        // Đưa TCB qua Ready rồi mới đổi affinity/home (Running ⇒ home ≠ hart bị chặn khi
        // n=1 không chặn — sim giữ hành vi cũ; ở đây home chỉ có 0 nên ok).
        ok(
            Op::SetAffinity {
                tcb: r.tslots[0],
                mask: 1,
            },
            0,
        ),
    ];
    run_steps(&mut s, &mut c, &r, &steps);
}

// ---------------------------------------------------------------------------------------------
// 3. KT-20: budget/refill/enforce/pick có điều kiện
// ---------------------------------------------------------------------------------------------

#[test]
fn kt59_parity_budget_va_refill() {
    let (mut s, mut c, r) = rig(1, 2);
    // Configure lỗi trước, rồi 4/8; gắn SC cho t0; cả hai Ready.
    let steps = [
        err(
            Op::ScConfigure {
                ctl: r.ctl,
                sc: r.scs[0],
                budget: 0,
                period: 8,
            },
            kc::KError::InvalidArgument,
        ),
        err(
            Op::ScConfigure {
                ctl: r.ctl,
                sc: r.scs[0],
                budget: 4,
                period: 0,
            },
            kc::KError::InvalidArgument,
        ),
        err(
            Op::ScConfigure {
                ctl: r.ctl,
                sc: r.scs[0],
                budget: 9,
                period: 8,
            },
            kc::KError::InvalidArgument,
        ),
        ok(
            Op::ScConfigure {
                ctl: r.ctl,
                sc: r.scs[0],
                budget: 4,
                period: 8,
            },
            0,
        ),
        ok(
            Op::SetSched {
                tcb: r.tslots[0],
                sc: Some(r.scs[0]),
            },
            0,
        ),
        ok(
            Op::SetSched {
                tcb: r.tslots[1],
                sc: None,
            },
            0,
        ),
        ok(
            Op::Transition {
                tcb: r.tcbs[0],
                st: CS::Ready,
            },
            0,
        ),
        ok(
            Op::Transition {
                tcb: r.tcbs[1],
                st: CS::Ready,
            },
            0,
        ),
        // t1 không SC ⇒ không được pick; pick nhảy t0.
        ok(Op::Dispatch { h: 0 }, r.tcbs[0] as u64),
        // Charge 4 tick: đúng bằng budget ⇒ Sleeping{Budget}.
        ok(Op::Tick { n: 4 }, 0),
        // Hết budget: pick bỏ qua t0; t1 không budget nên hart rảnh.
        ok(Op::Dispatch { h: 0 }, 0),
        ok(Op::Tick { n: 4 }, 0),
        // now = 8: refill ⇒ t0 Ready lại và dispatch được.
        ok(Op::Dispatch { h: 0 }, r.tcbs[0] as u64),
        // Gỡ SC khi đang Running ⇒ ngủ ngay `Sleeping{Budget}` (enforce_budget).
        ok(
            Op::SetSched {
                tcb: r.tslots[0],
                sc: None,
            },
            0,
        ),
        // Gắn SC đã cạn cho TCB đang Sleeping — vẫn Sleeping tới biên refill.
        ok(Op::Tick { n: 8 }, 0),
        ok(
            Op::SetSched {
                tcb: r.tslots[0],
                sc: Some(r.scs[1]),
            },
            0,
        ),
        ok(
            Op::ScConfigure {
                ctl: r.ctl,
                sc: r.scs[1],
                budget: 2,
                period: 4,
            },
            0,
        ),
        // now = 20 (biên refill của sc1) ⇒ TCB Sleeping{Budget} được đánh thức `Ready`;
        // `pick` chỉ chọn TCB có budget nên t1 (không SC) vẫn bị bỏ qua.
        ok(Op::Tick { n: 4 }, 0),
        ok(Op::Dispatch { h: 0 }, r.tcbs[0] as u64),
        ok(Op::Tick { n: 2 }, 0),
        ok(Op::Tick { n: 2 }, 0),
        ok(Op::Dispatch { h: 0 }, r.tcbs[0] as u64),
        // Configure lại giữa chu kỳ đặt consumed=0 (model-only `[TK]`).
        ok(
            Op::ScConfigure {
                ctl: r.ctl,
                sc: r.scs[1],
                budget: 3,
                period: 6,
            },
            0,
        ),
    ];
    run_steps(&mut s, &mut c, &r, &steps);
}

// ---------------------------------------------------------------------------------------------
// 4. KT-22: hộp thư FIFO/backpressure/Stop/Migrate 1 hart
// ---------------------------------------------------------------------------------------------

#[test]
fn kt59_parity_hop_thu() {
    let (mut s, mut c, r) = rig(1, 2);
    arm_rig(&mut s, &mut c, &r, 64, 64);
    let mut steps = Vec::new();
    // 8 message đầy hàng; thứ 9 ⇒ Busy (backpressure), message không bị ghi đè.
    for i in 0..8 {
        steps.push(ok(
            Op::PostMail {
                h: 0,
                msg: CMsg::Wake(r.tcbs[i % 2]),
            },
            0,
        ));
    }
    steps.push(err(
        Op::PostMail {
            h: 0,
            msg: CMsg::Reschedule,
        },
        kc::KError::Busy,
    ));
    steps.push(err(
        Op::PostMail {
            h: 1,
            msg: CMsg::Stop,
        },
        kc::KError::InvalidArgument,
    ));
    steps.push(err(
        Op::PostMail {
            h: 0,
            msg: CMsg::Migrate {
                tcb: r.tcbs[0],
                to_hart: 1,
            },
        },
        kc::KError::NotSupported,
    ));
    // Drain đúng FIFO một lần mỗi message.
    steps.push(ok(Op::ProcessMail { h: 0 }, 8));
    // send_mail = post + process trong một bước; Stop đặt cờ.
    steps.push(ok(
        Op::SendMail {
            h: 0,
            msg: CMsg::Stop,
        },
        0,
    ));
    run_steps(&mut s, &mut c, &r, &steps);
    assert!(s.hart_stopped(0).unwrap());
    assert!(c.hart_stopped(0).unwrap());
    assert_eq!(s.mail_log(0).unwrap().len(), 9);
    assert_eq!(c.mail_log(0).unwrap().len(), 9);
}

// ---------------------------------------------------------------------------------------------
// 5. KT-29: nhiều hart, published, push migration cơ bản
// ---------------------------------------------------------------------------------------------

#[test]
fn kt59_parity_nhieu_hart_va_push_migration() {
    let (mut s, mut c, r) = rig(2, 2);
    arm_rig(&mut s, &mut c, &r, 64, 64);
    // Đưa t1 sang hart 1, dùng publish/dispatch/yield/push migration.
    let steps = [
        ok(
            Op::SetAffinity {
                tcb: r.tslots[0],
                mask: 3,
            },
            0,
        ),
        ok(
            Op::SetAffinity {
                tcb: r.tslots[1],
                mask: 3,
            },
            0,
        ),
        ok(
            Op::SetHome {
                tcb: r.tslots[1],
                home: 1,
            },
            0,
        ),
        // publish luật 3: hart khác current_hart ⇒ IllegalOperation.
        err(
            Op::Publish {
                h: 1,
                idle: true,
                prio: 0,
            },
            kc::KError::IllegalOperation,
        ),
        err(Op::SetCurrentHart { h: 2 }, kc::KError::InvalidArgument),
        ok(Op::SetCurrentHart { h: 1 }, 0),
        // Hart 1 rảnh ⇒ published phải là (idle = true, cur_prio = 0); "nói dối" bị chặn.
        err(
            Op::Publish {
                h: 1,
                idle: false,
                prio: 0,
            },
            kc::KError::IllegalOperation,
        ),
        ok(
            Op::Publish {
                h: 1,
                idle: true,
                prio: 0,
            },
            0,
        ),
        // Dispatch hart1 chọn t1 (hàng FIFO).
        ok(Op::Dispatch { h: 1 }, r.tcbs[1] as u64),
        // Running ⇒ Ready (cặp ALLOWED) không đổi current, chỉ đưa lại hàng.
        ok(
            Op::Transition {
                tcb: r.tcbs[1],
                st: CS::Ready,
            },
            0,
        ),
        ok(
            Op::SetHome {
                tcb: r.tslots[0],
                home: 1,
            },
            0,
        ),
        // Hai TCB Ready mức cao nhất ở hart1, hart0 idle ⇒ push đúng MỘT TCB (cuối hàng).
        ok(Op::PushMigration, 1),
        // TCB bị đẩy là t0 (cuối hàng) ⇒ hart0 dispatch ra t0.
        ok(Op::Dispatch { h: 0 }, r.tcbs[0] as u64),
    ];
    run_steps(&mut s, &mut c, &r, &steps);
    assert_eq!(s.migrate_count(), 1);
    assert_eq!(c.migrate_count(), 1);
}

// ---------------------------------------------------------------------------------------------
// 6. Differential 16 seed × 32 op (LCG) — cùng seed cùng trace
// ---------------------------------------------------------------------------------------------

#[test]
fn kt59_differential_seed() {
    const N_SEED: u64 = 16;
    const N_OP: usize = 32;

    /// KT-59 **không** có Endpoint/Reply/Notification (KT-60) nên differential chỉ dùng
    /// trạng thái scheduler thuần: Inactive/Ready/Running/Sleeping/Stopping. Các trạng thái
    /// chặn IPC đã có kịch bản tường minh qua `transition_tcb` (không đi qua `sched_after_op`
    /// nên không chạm `check_ipc` của sim).
    fn st(theo: u8, h: u16) -> CS {
        match theo % 5 {
            0 => CS::Inactive,
            1 => CS::Ready,
            2 => CS::Running { hart: h },
            3 => CS::Sleeping { reason: 1 },
            _ => CS::Stopping,
        }
    }

    fn run_seed(seed: u64) -> Vec<(Res, Digest)> {
        let (mut s, mut c, r) = rig(2, 4);
        arm_rig(&mut s, &mut c, &r, 8, 8);
        let mut rng = Lcg::new(seed);
        let mut trace = Vec::new();
        for _ in 0..N_OP {
            let t = rng.below(4) as usize;
            let h = rng.below(2) as u16;
            let op = match rng.below(20) {
                0 => Op::SetPriority {
                    tcb: r.tslots[t],
                    p: rng.below(12) as u8,
                    mcp: 255,
                },
                1 => Op::SetPriority {
                    tcb: r.tslots[t],
                    p: 9,
                    mcp: rng.below(12) as u8,
                },
                2 => Op::Transition {
                    tcb: r.tcbs[t],
                    st: st(rng.below(5) as u8, h),
                },
                3 => Op::Suspend {
                    tcb: r.tslots[t],
                    abort: rng.below(4) != 0,
                },
                4 => Op::Resume { tcb: r.tslots[t] },
                5 => Op::Tick { n: rng.below(4) },
                6 => Op::Dispatch { h },
                7 => Op::Yield { h },
                8 => Op::Wake { tcb: r.tcbs[t] },
                9 => Op::BlockCurrent {
                    h,
                    st: CS::Sleeping { reason: 1 },
                },
                10 => Op::PostMail {
                    h,
                    msg: CMsg::Wake(r.tcbs[t]),
                },
                11 => Op::PostMail {
                    h,
                    msg: CMsg::Reschedule,
                },
                12 => Op::PostMail { h, msg: CMsg::Stop },
                13 => Op::ProcessMail { h },
                14 => Op::SendMail {
                    h,
                    msg: CMsg::Wake(r.tcbs[t]),
                },
                15 => Op::SetCurrentHart { h },
                16 => Op::PushMigration,
                17 => Op::Publish {
                    h,
                    idle: rng.below(2) == 0,
                    prio: rng.below(10) as u8,
                },
                18 => Op::SetAffinity {
                    tcb: r.tslots[t],
                    mask: 1 + rng.below(3),
                },
                _ => Op::ScConfigure {
                    ctl: r.ctl,
                    sc: r.scs[rng.below(3) as usize],
                    budget: 1 + rng.below(8),
                    period: 8,
                },
            };
            let a = apply(&mut s, op);
            let b = apply(&mut c, op);
            assert_eq!(a, b, "seed {seed}: kết quả lệch ở {op:?}");
            let da = digest(&s, &r);
            let db = digest(&c, &r);
            assert_eq!(
                da, db,
                "seed {seed}: digest lệch ở {op:?}\nsim={da:?}\ncore={db:?}"
            );
            assert_eq!(
                da.checks,
                (true, true, true, true),
                "seed {seed}: bất biến vỡ ở {op:?}"
            );
            trace.push((a, da));
        }
        trace
    }

    let mut hashes = BTreeSet::new();
    for seed in 0..N_SEED {
        let a = run_seed(seed);
        let b = run_seed(seed);
        assert_eq!(a, b, "seed {seed}: cùng seed phải cùng trace");
        assert_eq!(a.len(), N_OP);
        hashes.insert(format!("{:?}", a));
    }
    println!(
        "# KT-59 differential OK: {N_SEED} seed × {N_OP} op, {} trace khác nhau",
        hashes.len()
    );
}

// ---------------------------------------------------------------------------------------------
// 7. Core-only: biên tĩnh (MAX_TCBS/MAX_SCHED_CONTEXTS) + dấu chân `Machine`
// ---------------------------------------------------------------------------------------------

#[test]
fn core_bien_tinh_tcb_va_sched_context() {
    use kc::{KError, Machine, ObjType, SlotRef};
    let mut m = Machine::new();
    let c0 = m.add_cnode(256);
    let u0 = m.add_untyped(0x1000, 1 << 20);
    let cap = m.root_cap(u0);
    m.put_cap(c0, 0, cap).expect("cap Untyped");
    // Retype đủ MAX_TCBS TCB — mỗi TCB 512 B, arena 1 MiB.
    for i in 0..kc::MAX_TCBS {
        let cap = m
            .retype(SlotRef::new(c0, 0), ObjType::Tcb, 0, 0)
            .expect("retype TCB trong biên");
        m.put_cap(c0, i + 1, cap).expect("dat cap TCB");
    }
    let used = m.untyped_used(u0);
    assert_eq!(used, kc::MAX_TCBS * kc::spec::OBJ_TCB_BYTES);
    // TCB thứ MAX_TCBS+1 ⇒ NotEnoughMemory nguyên tử, watermark không đổi.
    assert_eq!(
        m.retype(SlotRef::new(c0, 0), ObjType::Tcb, 0, 0),
        Err(KError::NotEnoughMemory)
    );
    assert_eq!(m.untyped_used(u0), used, "lỗi bảng không được đốt byte");
    // Revoke Untyped finalise cả 64 TCB + trả body về free-list ⇒ retype lại được.
    let deleted = m.revoke(c0, Some(0)).expect("revoke Untyped");
    assert_eq!(deleted, kc::MAX_TCBS);
    assert_eq!(m.untyped_used(u0), 0, "revoke trả hết byte");
    let cap = m
        .retype(SlotRef::new(c0, 0), ObjType::Tcb, 0, 0)
        .expect("retype lại sau khi body về free-list");
    m.put_cap(c0, 1, cap).expect("dat cap TCB lai");
    // MAX_SCHED_CONTEXTS: đầy ⇒ NotEnoughMemory, byte không đổi.
    for i in 0..kc::MAX_SCHED_CONTEXTS {
        let cap = m
            .retype(SlotRef::new(c0, 0), ObjType::SchedContext, 0, 0)
            .expect("retype SC trong biên");
        m.put_cap(c0, 100 + i, cap).expect("dat cap SC");
    }
    let used_sc = m.untyped_used(u0);
    assert_eq!(
        m.retype(SlotRef::new(c0, 0), ObjType::SchedContext, 0, 0),
        Err(KError::NotEnoughMemory)
    );
    assert_eq!(m.untyped_used(u0), used_sc);
}

#[test]
fn core_dau_chan_tinh_va_bang_phan_bo_d2_84() {
    let sz = core::mem::size_of::<kc::Machine>();
    let fp = kc::Machine::footprint();
    println!("# KT-59 core: size_of::<Machine>() = {sz} B (D2-84 pin <= 196.608 B)");
    for (name, bytes) in fp.names.iter().zip(fp.bytes.iter()) {
        println!("#   {name:<20} {bytes:>7} B");
    }
    println!("#   {:<20} {:>7} B", "scalars", fp.scalars);
    println!("#   {:<20} {:>7} B", "padding", fp.padding);
    assert_eq!(
        fp.total, sz,
        "footprint.total phải bằng size_of::<Machine>()"
    );
    let sum: usize = fp.bytes.iter().sum::<usize>() + fp.scalars + fp.padding;
    assert_eq!(
        sum, sz,
        "tổng bảng phân bố phải khớp size_of (kể cả padding)"
    );
    assert!(
        fp.padding < 128,
        "padding {} B bất thường (layout đổi?)",
        fp.padding
    );
    // **D2-84**: trần core 192 KiB = 262.144 B trừ 64 KiB headroom; vượt ⇒ tối ưu, không nâng.
    assert!(
        sz <= 196_608,
        "Machine {sz} B vượt pin D2-84 192 KiB (196.608 B) — phải tối ưu"
    );
    println!(
        "# KT-59 core: {sz} B = {:.1}% trần ảnh 262.144 B, headroom còn {} B",
        sz as f64 * 100.0 / 262_144.0,
        196_608usize.saturating_sub(sz)
    );
    assert_eq!(kc::MAX_TCBS, 64);
    assert_eq!(kc::MAX_SCHED_CONTEXTS, 32);
    assert_eq!(kc::MAX_HARTS, 8);
    assert_eq!(kc::MAILBOX_CAP, 8);
    assert_eq!(kc::MAX_MAIL_LOG, 32);
    assert_eq!(kc::tcb::allowed_count(), 48);
    assert_eq!(kc::tcb::reserved_count(), 2);
    assert_eq!(kc::tcb::forbidden_count(), 73);
}
