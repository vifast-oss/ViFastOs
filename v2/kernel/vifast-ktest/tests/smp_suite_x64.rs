// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho bộ ca **SMP dùng chung** — KT-413 (`B5`/SMP, D2-259 mục 2).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-ktest
//! ```
//!
//! Đây là **ca kiểm đỏ-trước** (D2-05): khi tệp này được viết, `vifast_ktest::smp_suite` **chưa
//! tồn tại** nên lượt chạy đầu **không biên dịch được** (đỏ đúng lý do "API chưa có"), không phải
//! đỏ vì lỗi khác. Sau khi hiện thực, các khẳng định dưới đây chốt bốn bất biến **host-đo-được**:
//!
//! 1. **một bộ ca, ba arch**: `smp_suite::run` chỉ phụ thuộc `Platform + CoreBridge` — không một
//!    dòng nào nhắc CSR/asm/MMIO của arch nào. Test chạy nó trên nền tảng **giả** (host) với
//!    `Machine` **4 hart** và đòi **0 ca đỏ** + **đủ 2 TAP** (49..50).
//! 2. **TAP ổn định**: đúng nhãn `core smp 4 hart` / `core smp sleep migrate` — runner grep theo
//!    đúng chuỗi này nên bộ chung **không** được đổi nhãn.
//! 3. **bốn hart thật**: `picks[h] >= 1` cho **cả bốn** hart, `distinct >= 2`, `double == 0`
//!    (không TCB nào bị hai hart cùng giữ) — nếu bộ ca chỉ chạy hart 0 thì ca này đỏ.
//! 4. **có răng**: `Machine` 1 hart ⇒ `run` trả `fails > 0` (bộ ca **không** phải hàm luôn xanh).

use std::cell::RefCell;
use std::string::String;

use vifast_core::{Machine, ObjId};
use vifast_ktest::smp_suite_x64::{self, SMP_HARTS};
use vifast_ktest::{CoreBridge, FrameLabel, LinkReport, ObjVmReport};
use vifast_plat_common::Platform;

/// Nền tảng **giả** trên host: ghi ra `String`, đồng hồ đếm giả, mọi thứ khác no-op.
struct FakePlatform {
    out: RefCell<String>,
    ticks: RefCell<u64>,
}

impl FakePlatform {
    fn new() -> Self {
        Self {
            out: RefCell::new(String::new()),
            ticks: RefCell::new(0),
        }
    }

    fn take(&self) -> String {
        core::mem::take(&mut *self.out.borrow_mut())
    }
}

impl Platform for FakePlatform {
    fn put_str(&self, s: &str) {
        self.out.borrow_mut().push_str(s);
    }

    fn halt(&self, code: usize) -> ! {
        std::process::exit(code as i32)
    }

    fn now_ticks(&self) -> u64 {
        let mut t = self.ticks.borrow_mut();
        *t += 1;
        *t
    }

    fn timebase_hz(&self) -> u64 {
        10_000_000
    }

    fn hart_id(&self) -> usize {
        0
    }

    fn set_trap_vector(&self, _vector: usize) {}

    fn enable_irq(&self, _irq: u32) -> bool {
        true
    }

    fn disable_irq(&self, _irq: u32) -> bool {
        true
    }

    fn set_timer_deadline(&self, _deadline: u64) {}

    fn enable_timer(&self) -> bool {
        true
    }

    fn disable_timer(&self) {}
}

/// Cầu nối **giả**: `Machine` thật của `vifast-core` trên host, `harts` cấu hình được.
struct FakeBridge {
    machine: RefCell<Machine>,
    report: LinkReport,
}

const FAKE_RAM_BASE: u64 = 0x8000_0000;
const FAKE_RAM_BYTES: u64 = 128 * 1024 * 1024;
const FAKE_FRAME_BASE: u64 = 0x8020_0000;

impl FakeBridge {
    fn new(harts: u16) -> Self {
        let mut machine = Machine::new();
        machine.init_zeroed(harts);
        let untyped = machine.add_untyped(FAKE_FRAME_BASE, 64 * 1024);
        let report = LinkReport {
            addr: 0x8020_0000,
            static_bytes: core::mem::size_of::<Machine>(),
            pin: vifast_ktest::CORE_PIN_BYTES,
            base: FAKE_FRAME_BASE,
            arena_bytes: 64 * 1024,
            untyped,
            harts: harts as usize,
            objects: machine.object_count(),
            caps: machine.live_cap_count(),
            check_caps: machine.check_caps().is_ok(),
            check_bytes: machine.check_bytes(untyped).is_ok(),
            check_all_bytes: machine.check_all_bytes().is_ok(),
            footprint: Machine::footprint(),
            in_image: true,
        };
        Self {
            machine: RefCell::new(machine),
            report,
        }
    }
}

impl CoreBridge for FakeBridge {
    fn with_machine<R>(&self, f: impl FnOnce(&mut Machine) -> R) -> R {
        f(&mut self.machine.borrow_mut())
    }

    fn link(&self) -> LinkReport {
        self.report
    }

    fn ram_window(&self) -> (u64, u64) {
        (FAKE_RAM_BASE, FAKE_RAM_BYTES)
    }

    fn obj_alloc_frame(&self) -> Option<FrameLabel> {
        Some(FrameLabel {
            pa: FAKE_FRAME_BASE,
            label: (FAKE_FRAME_BASE >> 12) as u32,
            window_base: FAKE_FRAME_BASE,
            window_used: 64 * 1024,
        })
    }

    fn obj_free_frame(&self, _label: u32) {}

    fn obj_vm_selftest(&self) -> ObjVmReport {
        ObjVmReport {
            vspace: true,
            frame: true,
            health: true,
            frames: 1,
            vspaces: 1,
            clean: true,
        }
    }
}

/// **Ca 1** — bộ ca SMP chung chạy trên nền tảng giả 4 hart: 0 đỏ, đủ 2 TAP 48..49.
#[test]
fn shared_smp_suite_runs_on_fake_platform() {
    let plat = FakePlatform::new();
    let bridge = FakeBridge::new(SMP_HARTS as u16);
    let outcome = smp_suite_x64::run(&plat, &bridge, SMP_HARTS as u16);
    let out = plat.take();
    assert_eq!(
        outcome.fails, 0,
        "bộ ca SMP dùng chung phải 0 đỏ trên nền tảng giả; log:\n{out}"
    );
    assert_eq!(outcome.taps, 2, "đủ 2 TAP (48..49); log:\n{out}");
    for n in 48..=49u32 {
        assert!(
            out.contains(&format!("ok {n} - ")),
            "thiếu TAP {n} trong log:\n{out}"
        );
    }
}

/// **Ca 2** — nhãn TAP ổn định + dòng bằng chứng `# core smp:` đúng một lần.
#[test]
fn shared_smp_suite_tap_labels_are_stable() {
    let plat = FakePlatform::new();
    let bridge = FakeBridge::new(SMP_HARTS as u16);
    let outcome = smp_suite_x64::run(&plat, &bridge, SMP_HARTS as u16);
    let out = plat.take();
    assert_eq!(outcome.taps, 2);
    for (n, name) in [(48u32, "core smp 4 hart"), (49, "core smp sleep migrate")] {
        assert!(
            out.contains(&format!("ok {n} - {name}\n")),
            "TAP {n} phải là `ok {n} - {name}`; log:\n{out}"
        );
        assert_eq!(
            out.matches(&format!("ok {n} - ")).count(),
            1,
            "TAP {n} phát đúng một lần; log:\n{out}"
        );
    }
    assert_eq!(
        out.matches("# core smp: ").count(),
        1,
        "dòng `# core smp:` đúng một lần; log:\n{out}"
    );
}

/// **Ca 3** — bốn hart **thật**: mỗi hart chọn ≥ 1, `distinct >= 2`, `double == 0`.
#[test]
fn shared_smp_suite_uses_all_four_harts() {
    let plat = FakePlatform::new();
    let bridge = FakeBridge::new(SMP_HARTS as u16);
    let report = smp_suite_x64::run(&plat, &bridge, SMP_HARTS as u16).report;
    for (h, n) in report.picks.iter().enumerate() {
        assert!(*n >= 1, "hart {h} chưa chọn tác vụ nào: {report:?}");
    }
    assert!(
        report.distinct >= 2,
        "phải có ≥ 2 tác vụ khác nhau: {report:?}"
    );
    assert_eq!(report.double, 0, "không được double-pick: {report:?}");
    assert!(report.prio_ok, "priority phải được tôn trọng: {report:?}");
    assert!(report.sleep_ok, "sleep/wake phải đúng: {report:?}");
    assert!(report.invariants_ok, "bất biến core phải xanh: {report:?}");
}

/// **Ca 4** — bộ ca **có răng**: `Machine` 1 hart ⇒ `fails > 0` (không phải hàm luôn xanh).
#[test]
fn shared_smp_suite_fails_on_single_hart_machine() {
    let plat = FakePlatform::new();
    let bridge = FakeBridge::new(1);
    let outcome = smp_suite_x64::run(&plat, &bridge, SMP_HARTS as u16);
    let out = plat.take();
    assert!(
        outcome.fails > 0,
        "Machine 1 hart phải làm bộ ca SMP đỏ; log:\n{out}"
    );
    assert!(
        out.contains("not ok 48 - core smp 4 hart"),
        "TAP 48 phải đỏ khi chỉ có 1 hart; log:\n{out}"
    );
    let _: ObjId = bridge.report.untyped;
}
