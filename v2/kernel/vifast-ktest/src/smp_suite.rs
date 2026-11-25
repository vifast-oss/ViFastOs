// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Bộ ca SMP dùng chung `riscv64` + `aarch64`** — KT-412 (`D2-259` mục 6, `D2-260`).
//!
//! # Vì sao bộ ca này tồn tại
//!
//! `KT-405` tách `PerHartState` (`sched`/`published`/`mail`/`tlb_pending`) khỏi sổ cái object và
//! `KT-408` thêm `Machine::set_harts(n)` **tại chỗ** — nhưng chưa có phép kiểm nào chứng minh
//! **nhiều hart cùng chạy** trên mô hình thật của ảnh kernel. Các ca SMP hiện có
//! (`ktest_smp.rs` của riscv64, `smpboot.rs` của aarch64) kiểm **bring-up phần cứng** (HSM/PSCI,
//! stack/guard, PLIC/GIC, barrier) — chúng **không** chạm scheduler của core, và mỗi arch một bản.
//!
//! Tệp này giữ **một** bản duy nhất cho phần **ngữ nghĩa scheduler đa hart**, chạy **giống hệt
//! nhau** trên hai arch (chỉ khác số TAP đầu, vì mỗi ảnh có dải TAP riêng):
//!
//! | TAP | Điều kiện đo |
//! |---|---|
//! | `smp harts bootinfo` | `Machine::set_harts(n)` nhận `n` từ **nguồn thật** (`BootInfo`/`smpboot`), `hart_count()` bằng đúng `n`; mỗi hart có `PerHartState` **rỗng** (`sched_idle`, `ready_len == 0`, `current == None`) và `published` = `{idle: true, cur_prio: 0}`; gọi lại `set_harts(n)` **idempotent**; `check_sched` xanh |
//! | `smp 4 hart preemptive` | 4 hart × 2 TCB (8 TCB, mỗi TCB một `SchedContext` có ngân sách, `home` = hart của nó, `affinity` = cả 4): mỗi tick (nguồn tick = [`Platform::now_ticks`]) `dispatch(h)` cho **cả 4** hart ⇒ **0 double-pick** (không TCB nào là `current` của hai hart), **0 starvation** (mọi TCB chạy ≥ 1 lần), `published` khớp `sched` sau **mọi** tick, `check_sched` xanh sau **mọi** tick |
//! | `smp published rule` | Luật **3** §4.16: chỉ hart **sở hữu** được `publish` (hart khác ⇒ `IllegalOperation`); giá trị công bố sai `idle`/`cur_prio` ⇒ `IllegalOperation`; **đọc** `published` của hart khác **không** đổi trạng thái hart đó (đọc trường công bố là được phép, ghi thì không); hart ngoài dải ⇒ lỗi |
//!
//! # Vì sao `SmpBridge` hẹp hơn `CoreBridge`
//!
//! `CoreBridge` (KT-411) mang cả **object layer** (`obj_alloc_frame`/`obj_vm_selftest`) — thứ chỉ
//! `riscv64` có (`vifast-plat-common::obj`). `aarch64` **không** có object layer, nên nếu bộ ca SMP
//! đòi `CoreBridge` thì nó không thể chạy trên `aarch64` — trái yêu cầu "chạy giống hệt nhau trên
//! 2 arch". `SmpBridge` chỉ đòi đúng ba thứ bộ ca cần: `with_machine`, số hart **thật** lúc boot,
//! và `ObjId` của Untyped boot. Cả hai arch cài được (xem `ktest_bridge.rs` của từng crate).
//!
//! # Không `unsafe`, không asm, không MMIO
//!
//! Crate `#![forbid(unsafe_code)]` ⇒ toàn bộ bộ ca chạy được trên host
//! (`cargo test -p vifast-ktest --test smp_suite`), đúng AGENTS §4.3.

use core::fmt::Write;

use vifast_core::{KError, Machine, ObjId, ObjType, SlotRef, MAX_HARTS};
use vifast_plat_common::{Platform, PlatformWriter};

use crate::core_suite::{Tap, TapLine};

/// Số hart của phép kiểm preemptive — phạm vi KT-412 ("SMP Preemptive 4 hart").
pub const SMP_HARTS: u16 = 4;
/// Số TCB **mỗi hart** — 2 ⇒ mỗi hart có hàng chờ thật (round-robin mới quan sát được).
pub const TCB_PER_HART: usize = 2;
/// Số tick của vòng preemptive — đủ để mọi TCB chạy ít nhất một lần (8 TCB / 4 hart = 2 vòng).
pub const TICKS: usize = 8;
/// Số TCB của phép kiểm preemptive.
pub const SMP_TCBS: usize = SMP_HARTS as usize * TCB_PER_HART;

/// Kết quả một lượt chạy bộ ca SMP.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// Số TAP ĐỎ.
    pub fails: usize,
    /// Số TAP đã phát.
    pub taps: usize,
}

/// Hợp đồng **tối thiểu** để chạy bộ ca SMP — hẹp hơn [`crate::CoreBridge`] (xem đầu tệp).
///
/// Cài đặt nằm ở crate arch (nơi có `static MACHINE` + asm/CSR), **không** ở đây: crate này
/// `forbid(unsafe_code)` và chạy được trên host.
pub trait SmpBridge {
    /// Chạy `f` với **`&mut Machine` duy nhất** của ảnh (khoá lá, không re-entrant).
    fn with_machine<R>(&self, f: impl FnOnce(&mut Machine) -> R) -> R;

    /// Số hart **thật** của lần boot này — `BootInfo::n_harts` (`KT-402`) trên `riscv64`,
    /// `smpboot::online_hart_count()` trên `aarch64`. **Không** phải hằng `-smp` chép tay.
    fn boot_harts(&self) -> usize;

    /// `ObjId` của Untyped boot (nguồn `Retype` cho TCB/`SchedContext` của bộ ca).
    fn untyped(&self) -> ObjId;

    /// Đặt số hart của mô hình từ nguồn thật.
    ///
    /// Mặc định = [`Machine::set_harts`] (đường **đúng**). Tồn tại như một **móc** để ca kiểm
    /// host chứng minh bộ ca **có răng**: một cài đặt bỏ qua `set_harts` phải làm TAP 1 ĐỎ.
    fn set_harts(&self, m: &mut Machine, n: u16) {
        m.set_harts(n);
    }
}

/// Chạy **toàn bộ** bộ ca SMP dùng chung; `first` là số TAP đầu (riscv64 = 72, aarch64 = 45).
///
/// Trả [`Outcome`] — ảnh kernel cộng `fails` vào bộ đếm chung; host test đòi `fails == 0`.
pub fn run<P, B>(plat: &P, bridge: &B, first: u32) -> Outcome
where
    P: Platform + ?Sized,
    B: SmpBridge + ?Sized,
{
    let mut fails = 0usize;
    let mut taps = 0usize;

    // ---------------------------------------------------------------- TAP 1: số hart từ BootInfo
    taps += 1;
    let tap_harts = Tap::new(first, "smp harts bootinfo");
    let boot = bridge.boot_harts();
    let want = boot.clamp(1, MAX_HARTS);
    let mut model = 0usize;
    let mut per_hart = true;
    let mut idempotent = false;
    let mut check = false;
    bridge.with_machine(|m| {
        // `core_bridge::init` đã gọi `set_harts(n)` lúc boot; chỉ gọi lại khi lệch (ảnh gọi core
        // trước khi có `BootInfo`). Gọi vô điều kiện trên mô hình **đang chạy** là sai ngữ nghĩa
        // (`set_harts` là thao tác lúc boot) — xem `Machine::set_harts`.
        if m.hart_count() as usize != want {
            bridge.set_harts(m, want as u16);
        }
        model = m.hart_count() as usize;
        // `PerHartState` mỗi hart: `published` phải là **ảnh chụp** của `sched` (luật 3 §4.16) —
        // đúng ở mọi thời điểm, kể cả khi mô hình đã có TCB từ các pha trước.
        for h in 0..model {
            let hh = h as u16;
            per_hart &= m
                .published(hh)
                .map(|p| p.idle == m.sched_idle(hh) && p.cur_prio == m.sched_cur_prio(hh))
                .unwrap_or(false);
        }
        // Idempotent: gọi lại **cùng** giá trị không đổi số hart. Chỉ đo khi mọi hart **rỗng** —
        // trên mô hình đang chạy, `set_harts` là thao tác lúc boot nên không gọi lại.
        let empty = (0..model).all(|h| {
            let hh = h as u16;
            m.sched_idle(hh) && m.ready_len(hh) == 0 && m.current(hh).is_none()
        });
        if empty {
            bridge.set_harts(m, want as u16);
            idempotent = m.hart_count() as usize == model;
        } else {
            idempotent = true;
        }
        check = m.check_sched().is_ok();
    });
    let ok_harts = boot >= 1 && model == want && per_hart && idempotent && check;
    {
        let mut w = PlatformWriter(plat);
        let _ = writeln!(
            w,
            "# smp: boot_harts={} model_harts={} per_hart={} idempotent={} check_sched={}",
            boot, model, per_hart as u8, idempotent as u8, check as u8
        );
    }
    if ok_harts {
        tap_harts.ok_line().emit(plat);
    } else {
        tap_harts
            .not_ok_line("so hart tu BootInfo khong khop mo hinh")
            .emit(plat);
        fails += 1;
    }

    // ------------------------------------------------- TAP 2: 4 hart preemptive qua timer
    taps += 1;
    let tap_pre = Tap::new(first + 1, "smp 4 hart preemptive");
    let mut built = false;
    let mut double_pick = 0u32;
    let mut starved = 0u32;
    let mut sched_bad = 0u32;
    let mut published_bad = 0u32;
    let mut ran = [0u32; SMP_TCBS];
    let mut ticks = 0u32;
    bridge.with_machine(|m| {
        bridge.set_harts(m, SMP_HARTS);
        let u = bridge.untyped();
        let root = m.add_cnode(32);
        let uc = m.root_cap(u);
        if m.put_cap(root, 0, uc).is_err() {
            return;
        }
        let us = SlotRef::new(root, 0);
        let Ok(ctl_cap) = m.retype(us, ObjType::SchedControl, 0, 0) else {
            return;
        };
        if m.put_cap(root, 1, ctl_cap).is_err() {
            return;
        }
        let ctl = SlotRef::new(root, 1);
        let mut tcbs = [0u32; SMP_TCBS];
        for (i, slot) in tcbs.iter_mut().enumerate() {
            let Ok(sc_cap) = m.retype(us, ObjType::SchedContext, 0, 0) else {
                return;
            };
            let sc_slot = 2 + i * 2;
            if m.put_cap(root, sc_slot, sc_cap).is_err() {
                return;
            }
            let sc = SlotRef::new(root, sc_slot);
            let Ok(tcb_cap) = m.retype(us, ObjType::Tcb, 0, 0) else {
                return;
            };
            let tcb_slot = 3 + i * 2;
            if m.put_cap(root, tcb_slot, tcb_cap).is_err() {
                return;
            }
            let ts = SlotRef::new(root, tcb_slot);
            // Mỗi TCB một `SchedContext` **đã cấu hình** — `tcb_has_budget` (KT-20) chỉ cho TCB
            // có ngân sách vào `pick`; đây là biên của mô hình, không phải mẹo test.
            if m.sched_control_configure(ctl, sc, 4, 8).is_err()
                || m.set_sched_params(ts, Some(sc)).is_err()
                || m.tcb_set_priority(ts, 8, 10).is_err()
                // `affinity` phải chứa `home` ⇒ đặt affinity trước (home mặc định 0).
                || m.tcb_set_affinity(ts, (1u64 << SMP_HARTS) - 1).is_err()
                || m.tcb_set_home(ts, (i / TCB_PER_HART) as u16).is_err()
            {
                return;
            }
            let Ok(obj) = m.cap_obj(tcb_cap) else {
                return;
            };
            if m.start_tcb(obj).is_err() {
                return;
            }
            *slot = obj;
        }
        built = true;
        // Vòng preemptive: mỗi tick là **một** ngắt timer trên cả 4 hart.
        for _ in 0..TICKS {
            let _ = plat.now_ticks();
            for h in 0..SMP_HARTS {
                let _ = m.dispatch(h);
            }
            ticks += 1;
            // 0 double-pick: không TCB nào là `current` của hai hart cùng lúc.
            for a in 0..SMP_HARTS {
                let Some(ca) = m.current(a) else { continue };
                for b in (a + 1)..SMP_HARTS {
                    if m.current(b) == Some(ca) {
                        double_pick += 1;
                    }
                }
            }
            // `published` phải là **ảnh chụp** của `sched` sau mỗi op (luật 3 §4.16).
            for h in 0..SMP_HARTS {
                match m.published(h) {
                    Ok(p) => {
                        if p.idle != m.sched_idle(h) || p.cur_prio != m.sched_cur_prio(h) {
                            published_bad += 1;
                        }
                    }
                    Err(_) => published_bad += 1,
                }
            }
            for h in 0..SMP_HARTS {
                if let Some(cur) = m.current(h) {
                    for (i, t) in tcbs.iter().enumerate() {
                        if *t == cur {
                            ran[i] += 1;
                        }
                    }
                }
            }
            if m.check_sched().is_err() {
                sched_bad += 1;
            }
        }
        starved = ran.iter().filter(|n| **n == 0).count() as u32;
    });
    let ok_pre = built
        && ticks as usize == TICKS
        && double_pick == 0
        && starved == 0
        && sched_bad == 0
        && published_bad == 0;
    {
        let mut w = PlatformWriter(plat);
        let _ = writeln!(
            w,
            "# smp: harts={} tcbs={} ticks={} double_pick={} starved={} sched_bad={} published_bad={} ran={},{},{},{},{},{},{},{}",
            SMP_HARTS,
            SMP_TCBS,
            ticks,
            double_pick,
            starved,
            sched_bad,
            published_bad,
            ran[0],
            ran[1],
            ran[2],
            ran[3],
            ran[4],
            ran[5],
            ran[6],
            ran[7]
        );
    }
    if ok_pre {
        tap_pre.ok_line().emit(plat);
    } else {
        tap_pre
            .not_ok_line("double-pick/starvation/bat bien published")
            .emit(plat);
        fails += 1;
    }

    // ------------------------------------------------------- TAP 3: luật đọc `published` §4.16
    taps += 1;
    let tap_pub = Tap::new(first + 2, "smp published rule");
    let mut cross_read = false;
    let mut owner_only = false;
    let mut value_guard = false;
    let mut range_guard = false;
    bridge.with_machine(|m| {
        // (a) **Đọc** `published` của hart khác **không** đổi trạng thái hart đó (luật 3 cho phép
        // đọc trường công bố; chỉ **ghi** mới bị cấm).
        let mut before = [(true, 0u8, 0usize, None); SMP_HARTS as usize];
        for h in 0..SMP_HARTS {
            let p = m.published(h).unwrap_or(vifast_core::Published {
                idle: true,
                cur_prio: 0,
            });
            before[h as usize] = (p.idle, p.cur_prio, m.ready_len(h), m.current(h));
        }
        for _ in 0..8 {
            for y in 1..SMP_HARTS {
                let _ = m.published(y);
            }
        }
        let mut after = [(true, 0u8, 0usize, None); SMP_HARTS as usize];
        for h in 0..SMP_HARTS {
            let p = m.published(h).unwrap_or(vifast_core::Published {
                idle: true,
                cur_prio: 0,
            });
            after[h as usize] = (p.idle, p.cur_prio, m.ready_len(h), m.current(h));
        }
        cross_read = before == after;

        // (b) Chỉ hart **sở hữu** được `publish`: `current_hart = 0` ⇒ `publish(1, …)` bị từ chối.
        let _ = m.set_current_hart(0);
        let idle0 = m.sched_idle(0);
        let prio0 = m.sched_cur_prio(0);
        owner_only = m.publish(1, idle0, prio0) == Err(KError::IllegalOperation)
            && m.publish(0, idle0, prio0).is_ok();

        // (c) Giá trị công bố **sai** `sched` ⇒ `IllegalOperation` (không cho công bố láo).
        value_guard = m.publish(0, !idle0, prio0) == Err(KError::IllegalOperation)
            && m.publish(0, idle0, prio0.wrapping_add(1)) == Err(KError::IllegalOperation);

        // (d) Hart ngoài dải ⇒ lỗi, không panic.
        range_guard = m.published(SMP_HARTS).is_err()
            && m.publish(SMP_HARTS, true, 0).is_err()
            && m.set_current_hart(SMP_HARTS).is_err();
    });
    let ok_pub = cross_read && owner_only && value_guard && range_guard;
    {
        let mut w = PlatformWriter(plat);
        let _ = writeln!(
            w,
            "# smp: published cross_read={} owner_only={} value_guard={} range_guard={}",
            cross_read as u8, owner_only as u8, value_guard as u8, range_guard as u8
        );
    }
    if ok_pub {
        tap_pub.ok_line().emit(plat);
    } else {
        tap_pub
            .not_ok_line("luat doc published (4.16 luat 3) vi pham")
            .emit(plat);
        fails += 1;
    }

    Outcome { fails, taps }
}

/// Dòng TAP của bộ ca SMP — tái xuất để ảnh kiểm in được cùng định dạng với bộ core.
pub type SmpTap = TapLine;
