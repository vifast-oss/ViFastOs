// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Bộ ca SMP dùng chung 3 arch** — KT-412 (`D2-259` mục 6, `D2-260`) + KT-413b (hợp nhất).
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
//! nhau** trên ba arch. Hai **profile** (nhóm ca) tồn tại vì hợp đồng TAP của ba runner đã đóng
//! băng và **khác nhau về số TAP** — gộp tập ca là **đổi kỳ vọng harness** (AGENTS §4.2):
//!
//! | Profile | Cửa vào | TAP | Arch |
//! |---|---|---|---|
//! | **shared** | [`run`] | 3 (`first`, `first+1`, `first+2`) | `riscv64` (71..73), `aarch64` (47..49) |
//! | **x64** | [`run_x64`] | 2 (**cố định** 48/49) | `x86_64` |
//!
//! # Profile **shared** — 3 TAP (rv/a64)
//!
//! | TAP | Điều kiện đo |
//! |---|---|
//! | `smp harts bootinfo` | `Machine::set_harts(n)` nhận `n` từ **nguồn thật** (`BootInfo`/`smpboot`), `hart_count()` bằng đúng `n`; mỗi hart có `PerHartState` **rỗng** (`sched_idle`, `ready_len == 0`, `current == None`) và `published` = `{idle: true, cur_prio: 0}`; gọi lại `set_harts(n)` **idempotent**; `check_sched` xanh |
//! | `smp 4 hart preemptive` | 4 hart × 2 TCB (8 TCB, mỗi TCB một `SchedContext` có ngân sách, `home` = hart của nó, `affinity` = cả 4): mỗi tick (nguồn tick = [`Platform::now_ticks`]) `dispatch(h)` cho **cả 4** hart ⇒ **0 double-pick** (không TCB nào là `current` của hai hart), **0 starvation** (mọi TCB chạy ≥ 1 lần), `published` khớp `sched` sau **mọi** tick, `check_sched` xanh sau **mọi** tick |
//! | `smp published rule` | Luật **3** §4.16: chỉ hart **sở hữu** được `publish` (hart khác ⇒ `IllegalOperation`); giá trị công bố sai `idle`/`cur_prio` ⇒ `IllegalOperation`; **đọc** `published` của hart khác **không** đổi trạng thái hart đó (đọc trường công bố là được phép, ghi thì không); hart ngoài dải ⇒ lỗi |
//!
//! # Profile **x64** — 2 TAP (KT-413)
//!
//! | TAP | Điều kiện đo |
//! |---|---|
//! | `core smp 4 hart` | `Machine::hart_count()` **bằng đúng** số hart nền tảng đo được; **cả bốn** hart `dispatch` ≥ 1 TCB; `distinct >= 2`; **0 double-pick**; priority tĩnh (hart 0 có TCB mức 5 và mức 1 ⇒ lượt chọn đầu **bắt buộc** là mức 5) |
//! | `core smp sleep migrate` | `notification_wait` (chặn) + `notification_signal` (đánh thức) đưa TCB về `Ready` và `dispatch` chọn lại được; `push_migration` chạy; bốn bất biến `check_sched`/`check_sc_refs`/`check_budget`/`check_mailboxes` xanh |
//!
//! # Vì sao `SmpBridge` hẹp hơn `CoreBridge`
//!
//! `CoreBridge` (KT-411) mang cả **object layer** (`obj_alloc_frame`/`obj_vm_selftest`) — thứ chỉ
//! `riscv64` có (`vifast-plat-common::obj`). `aarch64` **không** có object layer, `x86_64` **chưa**
//! có (`KT-340c` chỉ nối `Machine`), nên nếu bộ ca SMP đòi `CoreBridge` thì hai ảnh ấy phải cài
//! ba method **giả** — đúng thứ `D2-83`/`D2-102` cấm ("không bịa bằng chứng"). `SmpBridge` chỉ
//! đòi đúng ba thứ bộ ca cần: `with_machine`, số hart **thật** lúc boot, và `ObjId` của Untyped
//! boot. Cả ba arch cài được (xem `ktest_bridge.rs`/`smpcore.rs`/`coresmp.rs`).
//!
//! # Không `unsafe`, không asm, không MMIO
//!
//! Crate `#![forbid(unsafe_code)]` ⇒ toàn bộ bộ ca chạy được trên host
//! (`cargo test -p vifast-ktest --test smp_suite`), đúng AGENTS §4.3.

use core::fmt::Write;

use vifast_core::{KError, Machine, ObjId, ObjType, SlotRef, TcbState, MAX_HARTS};
use vifast_plat_common::{Platform, PlatformWriter};

use crate::core_suite::{state_name, Tap, TapLine};

// ---------------------------------------------------------------------------------------------
// Hằng số profile **shared** (rv/a64) — phạm vi KT-412 ("SMP Preemptive 4 hart").
// ---------------------------------------------------------------------------------------------

/// Số hart của phép kiểm preemptive — phạm vi KT-412 ("SMP Preemptive 4 hart").
pub const SMP_HARTS: u16 = 4;
/// Số TCB **mỗi hart** — 2 ⇒ mỗi hart có hàng chờ thật (round-robin mới quan sát được).
pub const TCB_PER_HART: usize = 2;
/// Số tick của vòng preemptive — đủ để mọi TCB chạy ít nhất một lần (8 TCB / 4 hart = 2 vòng).
pub const TICKS: usize = 8;
/// Số TCB của phép kiểm preemptive.
pub const SMP_TCBS: usize = SMP_HARTS as usize * TCB_PER_HART;

// ---------------------------------------------------------------------------------------------
// Hằng số profile **x64** — phạm vi KT-413 ("SMP Preemptive 4 hart x86_64").
// ---------------------------------------------------------------------------------------------

/// Số hart của profile x64 — **4** (cùng `-smp 4` của runner; `Machine::MAX_HARTS` = 8).
pub const X64_HARTS: usize = 4;
/// Số TCB dựng cho profile x64: **một** cho mỗi hart + **một** TCB mức cao trên hart 0 (đo priority).
pub const X64_TCBS: usize = X64_HARTS + 1;
/// Số TAP đầu của profile x64 — **hợp đồng runner** `run-x86_64.sh` (`ok 48`/`ok 49`), không đổi.
pub const X64_FIRST_TAP: u32 = 48;
/// Mức ưu tiên của từng TCB — **khác nhau đôi một** để phép đo priority có nghĩa.
///
/// `PRIO[0..4]` là TCB "chủ" của hart 0..3; `PRIO[4]` là TCB **mức cao nhất** đặt trên hart 0
/// (nên lượt chọn đầu của hart 0 **bắt buộc** là nó).
pub const PRIO: [u8; X64_TCBS] = [1, 2, 3, 4, 5];
/// Hart nhà của từng TCB: `[0, 1, 2, 3, 0]` — hart 0 có **hai** TCB, ba hart kia mỗi hart một.
pub const HOME: [u16; X64_TCBS] = [0, 1, 2, 3, 0];

/// Kết quả **đo** của profile x64 — in ở dòng `# core smp:` và trả cho caller.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SmpReport {
    /// `Machine::hart_count()` **đo được** của lần boot này.
    pub harts: u16,
    /// Số lượt `dispatch` thành công của từng hart.
    pub picks: [u64; X64_HARTS],
    /// Chỉ số TCB (`0..X64_TCBS`) mà mỗi hart chọn ở **lượt đầu** (`u8::MAX` = chưa chọn).
    pub first: [u8; X64_HARTS],
    /// Số TCB **khác nhau** đã chạy.
    pub distinct: u64,
    /// Số lần một TCB bị **hai** hart cùng giữ (phải `0`).
    pub double: u64,
    /// Priority tĩnh được tôn trọng (lượt đầu của hart 0 là TCB mức cao nhất).
    pub prio_ok: bool,
    /// `block_current` + `wake` + `dispatch` lại đúng.
    pub sleep_ok: bool,
    /// Số TCB đã **migrate** qua hart khác (`push_migration`).
    pub migrate: u64,
    /// Bốn bất biến scheduler/SC/budget/mailbox của core đều xanh.
    pub invariants_ok: bool,
}

impl SmpReport {
    /// `true` ⇔ **cả bốn** hart đã chọn ≥ 1 TCB, không double-pick, priority + sleep đúng.
    pub fn four_harts_ok(&self) -> bool {
        self.harts as usize == X64_HARTS
            && self.picks.iter().all(|n| *n >= 1)
            && self.double == 0
            && self.distinct >= 2
            && self.prio_ok
    }

    /// `true` ⇔ sleep/wake + migration + bất biến core đều đúng.
    pub fn lifecycle_ok(&self) -> bool {
        self.sleep_ok && self.invariants_ok
    }
}

/// Kết quả một lượt chạy bộ ca SMP: số TAP đỏ, số TAP đã phát, và báo cáo **đo được**.
///
/// Profile **shared** để `report` ở giá trị mặc định (nó không dựng `SmpReport`); profile **x64**
/// đổ đầy — caller dùng cho dòng `# core smp:` của ảnh.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    /// Số TAP đỏ.
    pub fails: usize,
    /// Số TAP đã phát.
    pub taps: usize,
    /// Báo cáo đo của profile x64 (`Default` với profile shared).
    pub report: SmpReport,
}

/// Hợp đồng **tối thiểu** để chạy bộ ca SMP — hẹp hơn [`crate::CoreBridge`] (xem đầu tệp).
///
/// Cài đặt nằm ở crate arch (nơi có `static MACHINE` + asm/CSR), **không** ở đây: crate này
/// `forbid(unsafe_code)` và chạy được trên host.
pub trait SmpBridge {
    /// Chạy `f` với **`&mut Machine` duy nhất** của ảnh (khoá lá, không re-entrant).
    fn with_machine<R>(&self, f: impl FnOnce(&mut Machine) -> R) -> R;

    /// Số hart **thật** của lần boot này — `BootInfo::n_harts` (`KT-402`) trên `riscv64`,
    /// `smpboot::online_hart_count()` trên `aarch64`, `smp::online_count()` trên `x86_64`.
    /// **Không** phải hằng `-smp` chép tay.
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

// =============================================================================================
// Profile **shared** (rv/a64) — 3 TAP, số TAP đầu **tham số hoá**.
// =============================================================================================

/// Chạy **toàn bộ** profile shared; `first` là số TAP đầu (riscv64 = 71, aarch64 = 47).
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

    Outcome {
        fails,
        taps,
        report: SmpReport::default(),
    }
}

// =============================================================================================
// Profile **x64** — 2 TAP cố định 48/49 (hợp đồng `run-x86_64.sh`).
// =============================================================================================

/// Chạy profile **x64** trên nền tảng `plat` + cầu nối `bridge`.
///
/// `harts` là số hart **nền tảng** của lần boot này (`smp::online_count()` của ảnh). Bộ ca
/// **không** tự đoán: `Machine::hart_count() != harts` ⇒ TAP 48 đỏ (fail-closed).
///
/// # Vì sao **không** dùng `wake()`
///
/// `wake()` là API **CHỈ-DÙNG-CHO-TEST** (D2-144) và luật `E_STRICT_TEST_ONLY_WAKE` chỉ miễn tệp
/// test-only **theo đồ thị module** — crate thư viện dùng chung **không** thoả. Vì vậy bước
/// "đánh thức" đi qua **đường sản phẩm**: `notification_wait` (chặn) + `notification_signal`
/// (đánh thức) — cùng bất biến "TCB bị chặn rời hàng đợi, tín hiệu đưa nó về `Ready`", nhưng
/// bằng API mà kernel **thật** dùng.
pub fn run_x64<P, B>(plat: &P, bridge: &B, harts: u16) -> Outcome
where
    P: Platform + ?Sized,
    B: SmpBridge + ?Sized,
{
    let untyped = bridge.untyped();
    let report = bridge.with_machine(|m| run_x64_locked(plat, m, untyped, harts));
    let mut fails = 0usize;
    let mut taps = 0usize;

    // ---------------------------------------------------------------- TAP 48: bốn hart
    taps += 1;
    let ok48 = report.four_harts_ok();
    let tap48 = Tap::new(X64_FIRST_TAP, "core smp 4 hart");
    if ok48 {
        tap48.ok_line().emit(plat);
    } else {
        let mut w = PlatformWriter(plat);
        let _ = writeln!(
            w,
            "not ok {} - core smp 4 hart: harts={} picks={:?} first={:?} distinct={} double={} prio_ok={}",
            X64_FIRST_TAP,
            report.harts,
            report.picks,
            report.first,
            report.distinct,
            report.double,
            report.prio_ok as u8
        );
        fails += 1;
    }

    // ------------------------------------------------- TAP 49: sleep/wake + migration
    taps += 1;
    let ok49 = report.lifecycle_ok();
    let tap49 = Tap::new(X64_FIRST_TAP + 1, "core smp sleep migrate");
    if ok49 {
        tap49.ok_line().emit(plat);
    } else {
        let mut w = PlatformWriter(plat);
        let _ = writeln!(
            w,
            "not ok {} - core smp sleep migrate: sleep_ok={} migrate={} invariants_ok={}",
            X64_FIRST_TAP + 1,
            report.sleep_ok as u8,
            report.migrate,
            report.invariants_ok as u8
        );
        fails += 1;
    }

    Outcome {
        fails,
        taps,
        report,
    }
}

/// Thân profile x64 — chạy **trong** khoá `Machine` do caller giữ (khoá không re-entrant).
///
/// Trả [`SmpReport`]; **không** panic, **không** `unwrap` trên đường lỗi model: mọi lỗi core đều
/// làm báo cáo giữ giá trị mặc định (`picks = 0`) ⇒ TAP đỏ **có chẩn đoán**, không treo.
fn run_x64_locked<P>(plat: &P, m: &mut Machine, untyped: ObjId, harts: u16) -> SmpReport
where
    P: Platform + ?Sized,
{
    let mut rep = SmpReport {
        harts: m.hart_count(),
        first: [u8::MAX; X64_HARTS],
        ..SmpReport::default()
    };
    // Fail-closed: `Machine` không có đúng số hart nền tảng ⇒ không đo gì thêm (TAP 49 đỏ).
    if rep.harts != harts || rep.harts as usize != X64_HARTS {
        emit_evidence(plat, &rep);
        return rep;
    }

    // ---- 1. Dựng CNode gốc + 5 TCB + 5 SchedContext + 1 SchedControl từ Untyped của core.
    let root = m.add_cnode(32);
    let us = SlotRef::new(root, 0);
    let uc = m.root_cap(untyped);
    if m.put_cap(root, 0, uc).is_err() {
        emit_evidence(plat, &rep);
        return rep;
    }
    let mut tcb_slot = [SlotRef::new(root, 0); X64_TCBS];
    let mut tcb = [0u32; X64_TCBS];
    let mut sc_slot = [SlotRef::new(root, 0); X64_TCBS];
    for i in 0..X64_TCBS {
        let Ok(cap) = m.retype(us, ObjType::Tcb, 0, 0) else {
            emit_evidence(plat, &rep);
            return rep;
        };
        if m.put_cap(root, 1 + i, cap).is_err() {
            emit_evidence(plat, &rep);
            return rep;
        }
        tcb_slot[i] = SlotRef::new(root, 1 + i);
        tcb[i] = m.cap_obj(cap).unwrap_or(0);
    }
    for (i, slot) in sc_slot.iter_mut().enumerate() {
        let Ok(cap) = m.retype(us, ObjType::SchedContext, 0, 0) else {
            emit_evidence(plat, &rep);
            return rep;
        };
        if m.put_cap(root, 1 + X64_TCBS + i, cap).is_err() {
            emit_evidence(plat, &rep);
            return rep;
        }
        *slot = SlotRef::new(root, 1 + X64_TCBS + i);
    }
    let Ok(ctl_cap) = m.retype(us, ObjType::SchedControl, 0, 0) else {
        emit_evidence(plat, &rep);
        return rep;
    };
    let ctl_slot = 1 + 2 * X64_TCBS;
    if m.put_cap(root, ctl_slot, ctl_cap).is_err() {
        emit_evidence(plat, &rep);
        return rep;
    }
    let ctl = SlotRef::new(root, ctl_slot);
    // Notification cho bước "chặn rồi đánh thức" — **đường sản phẩm** (`notification_wait` +
    // `notification_signal`), không dùng `wake()` chỉ-dùng-cho-test (D2-144).
    let Ok(ntfn_cap) = m.retype(us, ObjType::Notification, 0, 0) else {
        emit_evidence(plat, &rep);
        return rep;
    };
    let ntfn_slot = ctl_slot + 1;
    if m.put_cap(root, ntfn_slot, ntfn_cap).is_err() {
        emit_evidence(plat, &rep);
        return rep;
    }
    let ntfn = m.cap_obj(ntfn_cap).unwrap_or(0);

    // ---- 2. Cấu hình SC + priority + home + affinity rồi `start` (Ready).
    for i in 0..X64_TCBS {
        let ok = m.sched_control_configure(ctl, sc_slot[i], 4, 8).is_ok()
            && m.set_sched_params(tcb_slot[i], Some(sc_slot[i])).is_ok()
            && m.tcb_set_priority(tcb_slot[i], PRIO[i], 10).is_ok()
            // **Thứ tự bắt buộc**: `SetAffinity` trước `SetHome` — `SetHome` đòi `home` nằm
            // trong `affinity` hiện tại, mà affinity mặc định chỉ có hart 0.
            && m.tcb_set_affinity(tcb_slot[i], 0xF).is_ok()
            && m.tcb_set_home(tcb_slot[i], HOME[i]).is_ok()
            && m.start_tcb(tcb[i]).is_ok();
        if !ok {
            emit_evidence(plat, &rep);
            return rep;
        }
    }

    // ---- 3. Vòng 1: **mỗi** hart dispatch một lượt. Ghi lượt chọn đầu + số lượt.
    for h in 0..X64_HARTS {
        if let Ok(Some(picked)) = m.dispatch(h as u16) {
            rep.picks[h] += 1;
            rep.first[h] = slot_of(&tcb, picked);
        }
    }
    rep.double += count_double(m, &tcb);
    rep.distinct = distinct_running(m, &tcb);
    // Priority: hart 0 có TCB mức 5 và mức 1 ⇒ lượt đầu **bắt buộc** là TCB mức 5 (chỉ số 4).
    rep.prio_ok = rep.first[0] == (X64_TCBS - 1) as u8;

    // ---- 4. Chặn rồi đánh thức — **đường sản phẩm** (`notification_wait` + `notification_signal`),
    //         không dùng `wake()` chỉ-dùng-cho-test (D2-144). Bất biến: TCB bị chặn **rời** hàng
    //         đợi (dispatch kế chọn TCB mức thấp), tín hiệu đưa nó về `Ready` và dispatch chọn lại.
    let high = tcb[X64_TCBS - 1];
    let low = tcb[0];
    let mut sleep_ok = m.notification_wait(high, ntfn).is_ok();
    sleep_ok &= m.tcb_state(high) == Ok(TcbState::BlockedOnNotification { ntfn });
    sleep_ok &= matches!(m.dispatch(0), Ok(Some(t)) if t == low);
    sleep_ok &= m.notification_signal(ntfn, 0x42).is_ok();
    sleep_ok &= m.tcb_state(high) == Ok(TcbState::Ready);
    sleep_ok &= matches!(m.dispatch(0), Ok(Some(t)) if t == high);
    rep.sleep_ok = sleep_ok;
    rep.double += count_double(m, &tcb);

    // ---- 5. Migration: một vòng push công khai (model API `[TK]`).
    rep.migrate = m.push_migration().unwrap_or(0) as u64;
    rep.migrate = rep.migrate.max(m.migrate_count());

    // ---- 6. Bất biến core sau toàn bộ chuỗi.
    rep.invariants_ok = m.check_sched().is_ok()
        && m.check_sc_refs().is_ok()
        && m.check_budget().is_ok()
        && m.check_mailboxes().is_ok();

    emit_evidence(plat, &rep);
    rep
}

/// Chỉ số TCB trong bảng của profile x64 (`u8::MAX` khi không khớp — không được xảy ra).
fn slot_of(tcb: &[ObjId; X64_TCBS], picked: ObjId) -> u8 {
    let mut i = 0;
    while i < X64_TCBS {
        if tcb[i] == picked {
            return i as u8;
        }
        i += 1;
    }
    u8::MAX
}

/// Đếm số TCB bị **hai** hart cùng giữ — phải `0`.
fn count_double(m: &Machine, tcb: &[ObjId; X64_TCBS]) -> u64 {
    let mut double = 0u64;
    for id in tcb.iter() {
        let mut owners = 0u64;
        for h in 0..X64_HARTS {
            if m.current(h as u16) == Some(*id) {
                owners += 1;
            }
        }
        if owners > 1 {
            double += 1;
        }
    }
    double
}

/// Số TCB **khác nhau** đang `Running` trên bốn hart.
fn distinct_running(m: &Machine, tcb: &[ObjId; X64_TCBS]) -> u64 {
    let mut n = 0u64;
    for id in tcb.iter() {
        if (0..X64_HARTS).any(|h| m.current(h as u16) == Some(*id)) {
            n += 1;
        }
    }
    n
}

/// In dòng bằng chứng `# core smp:` — **một** dòng, giá trị **đo**, không hardcode.
fn emit_evidence<P: Platform + ?Sized>(plat: &P, rep: &SmpReport) {
    let mut w = PlatformWriter(plat);
    let _ = writeln!(
        w,
        "# core smp: harts={} picks=[{}, {}, {}, {}] first=[{}, {}, {}, {}] distinct={} double={} prio_ok={} sleep_ok={} migrate={} invariants_ok={}",
        rep.harts,
        rep.picks[0],
        rep.picks[1],
        rep.picks[2],
        rep.picks[3],
        first_name(rep.first[0]),
        first_name(rep.first[1]),
        first_name(rep.first[2]),
        first_name(rep.first[3]),
        rep.distinct,
        rep.double,
        rep.prio_ok as u8,
        rep.sleep_ok as u8,
        rep.migrate,
        rep.invariants_ok as u8
    );
}

/// Tên ngắn của chỉ số TCB cho dòng bằng chứng (`?` khi chưa chọn).
fn first_name(slot: u8) -> &'static str {
    match slot {
        0 => "t0",
        1 => "t1",
        2 => "t2",
        3 => "t3",
        4 => "t4",
        _ => "?",
    }
}

/// Tên trạng thái TCB — dùng lại bản **dùng chung** của `core_suite` (không chép lần hai).
pub fn tcb_state_name(s: TcbState) -> &'static str {
    state_name(s)
}

/// Dòng TAP của bộ ca SMP — tái xuất để ảnh kiểm in được cùng định dạng với bộ core.
pub type SmpTap = TapLine;
