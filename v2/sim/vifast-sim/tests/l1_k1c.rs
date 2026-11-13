// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-29** — driver **L1 theo `smp`** (K1C-PLAN §5 KT-29 DoD 8): đọc `VIFAST_SMP`
//! (mặc định `1`) và `VIFAST_SEEDS` (mặc định `1000`), chạy **cùng seed ⇒ cùng trace** trên
//! mô hình N hart tất định (1/2/4/8), kiểm `check_sched`/`check_harts`/`check_ipc` sau **mỗi**
//! op.
//!
//! `gate --fast` vẫn chạy **1.000 seed / smp 1** (mặc định env, không nới ngưỡng); muốn quét
//! 1/2/4/8 dùng `./v2/bin/v2 nightly --l1 --smp 1,2,4,8 [--seeds N]` (100k seed thuộc cổng
//! cuối phase §7.1, **không** chạy trong vòng task).
//!
//! **Model-only `[TK]`**, không phải spec/ABI; không thread/QEMU.

use vifast_sim::kobj::{KError, Machine, ObjId, ObjType, SlotRef, TcbState};
use vifast_sim::tcb::SleepReason;
use vifast_sim::Lcg;

fn slot(cnode: ObjId, s: usize) -> SlotRef {
    SlotRef::new(cnode, s)
}

/// `VIFAST_SMP` (mặc định `1`): danh sách hart, ví dụ `1,2,4,8`. Mỗi phần tử phải trong
/// `1..=MAX_HARTS` (8) — sai ⇒ panic rõ (không im lặng bỏ).
fn smp_tu_env() -> Vec<u16> {
    let raw = std::env::var("VIFAST_SMP").unwrap_or_else(|_| "1".to_string());
    let mut ds = Vec::new();
    for phan in raw.split(',') {
        let n: u16 = phan
            .trim()
            .parse()
            .unwrap_or_else(|_| panic!("VIFAST_SMP: '{phan}' không phải số hart"));
        assert!(
            (1..=vifast_sim::kobj::MAX_HARTS).contains(&n),
            "VIFAST_SMP: hart {n} ngoài 1..={}",
            vifast_sim::kobj::MAX_HARTS
        );
        ds.push(n);
    }
    ds
}

/// `VIFAST_SEEDS` (mặc định `1000`); `0` ⇒ panic (không chạy rỗng để "xanh").
fn seeds_tu_env() -> u64 {
    let raw = std::env::var("VIFAST_SEEDS").unwrap_or_else(|_| "1000".to_string());
    let n: u64 = raw
        .trim()
        .parse()
        .unwrap_or_else(|_| panic!("VIFAST_SEEDS: '{raw}' không phải số"));
    assert!(n > 0, "VIFAST_SEEDS phải dương");
    n
}

/// Một lượt L1 tất định trên `harts` hart: trace là chuỗi sự kiện + ảnh chụp scheduler.
fn mot_luot(seed: u64, harts: u16, so_op: usize) -> Vec<String> {
    let mut m = Machine::with_harts(harts);
    let c0 = m.add_cnode(16);
    let u0 = m.add_untyped(0x1000, 1 << 16);
    let sc = m.add_kernel_object(ObjType::SchedContext);
    let ctl = m.add_kernel_object(ObjType::SchedControl);
    for (i, obj) in [(0usize, u0), (4, sc), (5, ctl)] {
        let cap = m.root_cap(obj);
        m.put_cap(c0, i, cap).expect("cap");
    }
    m.sched_control_configure(slot(c0, 5), slot(c0, 4), 64, 64)
        .expect("SC mặc định (test-only)");
    let mut s = Vec::new();
    let mut t = Vec::new();
    for i in 0..4 {
        let tcb = m.add_kernel_object(ObjType::Tcb);
        let cap = m.root_cap(tcb);
        m.put_cap(c0, 8 + i, cap).expect("cap TCB");
        m.set_sched_params(slot(c0, 8 + i), Some(slot(c0, 4)))
            .expect("SC cho TCB");
        m.tcb_set_priority(slot(c0, 8 + i), 3 + i as u8, 255)
            .expect("prio");
        m.tcb_set_affinity(slot(c0, 8 + i), (1u64 << harts) - 1)
            .expect("affinity");
        m.transition_tcb(tcb, TcbState::Ready).expect("Ready");
        s.push(slot(c0, 8 + i));
        t.push(tcb);
    }
    let mut rng = Lcg::new(seed ^ ((harts as u64) << 32));
    let mut trace = Vec::new();
    for step in 0..so_op {
        let op = rng.below(9);
        let mut dong = format!("s{step} op{op}");
        match op {
            0 => {
                let i = rng.below(4) as usize;
                let p = rng.below(16) as u8;
                if m.tcb_set_priority(s[i], p, 255).is_ok() {
                    dong.push_str(&format!(" prio{i}={p}"));
                }
            }
            1 => {
                let i = rng.below(4) as usize;
                let home = m.tcb_info(s[i]).expect("info").home;
                let mask = (1u64 << home) | (rng.below(1u64 << harts) & !(1u64 << home));
                if m.tcb_set_affinity(s[i], mask).is_ok() {
                    dong.push_str(&format!(" aff{i}={mask:#x}"));
                }
            }
            2 => {
                let h = rng.below(harts as u64) as u16;
                let got = m.dispatch(h).expect("dispatch");
                dong.push_str(&format!(" disp{h}={got:?}"));
            }
            3 => {
                let h = rng.below(harts as u64) as u16;
                if m.current(h).is_some() {
                    let _ = m.block_current(
                        h,
                        TcbState::Sleeping {
                            reason: SleepReason::Timeout,
                        },
                    );
                    dong.push_str(&format!(" block{h}"));
                }
            }
            4 => {
                let i = rng.below(4) as usize;
                if m.wake(t[i]).is_ok() {
                    dong.push_str(&format!(" wake{i}"));
                }
            }
            5 => {
                let h = rng.below(harts as u64) as u16;
                let n = m.process_mail(h).expect("process_mail");
                dong.push_str(&format!(" mail{h}={n}"));
            }
            6 => {
                let n = m.push_migration().expect("push_migration");
                dong.push_str(&format!(" push={n}"));
            }
            7 => {
                let i = rng.below(4) as usize;
                let st = m.tcb_state(t[i]).expect("state");
                if matches!(st, TcbState::Inactive) {
                    m.transition_tcb(t[i], TcbState::Ready).expect("Ready");
                    dong.push_str(&format!(" ready{i}"));
                } else if !matches!(st, TcbState::Ready | TcbState::Running { .. }) {
                    let _ = m.wake(t[i]);
                    dong.push_str(&format!(" wake{i}"));
                }
            }
            _ => {
                let h = rng.below(harts as u64) as u16;
                if m.current(h).is_some() {
                    let _ = m.yield_current(h).expect("yield");
                    dong.push_str(&format!(" yield{h}"));
                }
            }
        }
        if let Err(e) = m.check_sched() {
            panic!("check_sched sau {dong}: {e}");
        }
        if let Err(e) = m.check_harts() {
            panic!("check_harts sau {dong}: {e}");
        }
        if let Err(e) = m.check_ipc() {
            panic!("check_ipc sau {dong}: {e}");
        }
        let mut anh = String::new();
        for h in 0..harts {
            let p = m.published(h).expect("published");
            anh.push_str(&format!(
                "h{h}:{}/{}:{};",
                m.current(h).is_some(),
                p.cur_prio,
                m.ready_len(h)
            ));
        }
        dong.push_str(&format!(" | {anh} mig={}", m.migrate_count()));
        trace.push(dong);
    }
    trace
}

/// **L1 theo smp**: cùng seed ⇒ **cùng trace** byte-for-byte, 0 vi phạm bất biến sau mỗi op.
///
/// Tên **không** bắt đầu bằng `l1_` để `gate --fast` (bộ lọc `l1_` của L1 cũ) **không** chạy
/// driver này: gate giữ L1 1.000 seed cũ ≤ 60 s (K1C-PLAN §5 KT-29 DoD 8); `nightly --l1` đặt
/// env `VIFAST_SMP`/`VIFAST_SEEDS` và chạy theo bộ lọc `driver_smp`.
#[test]
fn driver_smp_cung_seed_cung_trace() {
    let smps = smp_tu_env();
    let seeds = seeds_tu_env();
    let so_op: usize = 64;
    let mut tong = 0u64;
    for &harts in &smps {
        for seed in 0..seeds {
            let a = mot_luot(seed, harts, so_op);
            let b = mot_luot(seed, harts, so_op);
            assert_eq!(
                a, b,
                "smp {harts} seed {seed}: cùng seed phải cùng trace (tất định)"
            );
            assert_eq!(a.len(), so_op);
            tong += 1;
        }
        println!("# L1 OK: smp {harts}, {seeds} seed × {so_op} op");
    }
    println!("# L1 OK: {} lượt (smp {smps:?}, seeds {seeds})", tong);
    let _ = KError::InvalidArgument;
}
