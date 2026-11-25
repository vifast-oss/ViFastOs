// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho bộ ca kiểm dùng chung 3 arch — KT-411 (`B5`, D2-259 mục 2).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-ktest
//! ```
//!
//! Đây là **ca kiểm đỏ-trước** (D2-05): khi tệp này được viết, crate `vifast-ktest` chỉ có khung
//! rỗng — [`vifast_ktest::core_suite::run`] và `vifast_plat_common::core_bridge::CoreBridge`
//! **chưa tồn tại**, nên lượt chạy đầu **không biên dịch được** (đỏ đúng lý do). Sau khi hiện
//! thực, các khẳng định dưới đây chốt bốn bất biến **host-đo-được** của bộ ca dùng chung:
//!
//! 1. **một bộ ca, ba arch**: `core_suite::run` chỉ phụ thuộc `Platform + CoreBridge` — không
//!    một dòng nào của nó nhắc tới CSR/asm/MMIO của arch nào. Test này chạy nó trên một nền tảng
//!    **giả** (host) và đòi **0 ca đỏ** + **đủ 10 TAP** (43..52).
//! 2. **TAP ổn định**: chuỗi TAP phát ra đúng thứ tự 43..52, mỗi dòng `ok <n> - <tên>` — runner
//!    grep theo đúng chuỗi này nên bộ chung **không** được đổi nhãn.
//! 3. **bất biến dùng chung**: `invariants::check_all` gom đúng **12** nhóm `check_*` của core
//!    (core/IPC/VM/SMP) và `all_ok()` chỉ đúng khi cả 12 xanh.
//! 4. **không nuốt lỗi**: một `Machine` bị làm hỏng (revoke sai) phải làm `all_ok()` **sai** —
//!    bộ kiểm có răng, không phải hàm luôn trả `true`.

use std::cell::RefCell;
use std::string::String;

use vifast_core::{Machine, ObjId};
use vifast_ktest::core_suite::{self, Tap};
use vifast_ktest::invariants;
use vifast_ktest::{CoreBridge, FrameLabel, LinkReport, ObjVmReport};
use vifast_plat_common::Platform;

/// Cửa `Ready` của nền tảng giả — trên host `wake()` hợp lệ (tệp test cấp crate).
fn ready(m: &mut Machine, tcb: vifast_core::ObjId) -> Result<(), vifast_core::KError> {
    match m.tcb_state(tcb)? {
        vifast_core::TcbState::Inactive => m.start_tcb(tcb),
        vifast_core::TcbState::Ready => Ok(()),
        vifast_core::TcbState::Running { .. } => m.tcb_yield(tcb),
        _ => m.wake(tcb),
    }
}

/// Nền tảng **giả** trên host: ghi ra `String`, đồng hồ đếm giả, mọi thứ khác no-op.
///
/// Đây chính là bằng chứng "một bộ ca dùng chung": nó chỉ cần [`Platform`], không cần CSR nào.
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

/// Cầu nối **giả**: `Machine` thật của `vifast-core` trên host + object layer giả.
///
/// Object layer giả cấp Frame ở một cửa sổ PA cố định nằm trong dải RAM giả — đủ để TAP 50/51
/// chạy đúng nhánh "có object layer thật" mà không cần MMIO.
struct FakeBridge {
    machine: RefCell<Machine>,
    report: LinkReport,
    frames: RefCell<u32>,
}

const FAKE_RAM_BASE: u64 = 0x8000_0000;
const FAKE_RAM_BYTES: u64 = 128 * 1024 * 1024;
const FAKE_FRAME_BASE: u64 = 0x8020_0000;

impl FakeBridge {
    fn new() -> Self {
        // `Machine` là `static` trong ảnh kernel; trên host nó là giá trị 184 KB trên stack của
        // luồng test (8 MB) — cùng bit-valid vì `new()` là `const fn` rồi `init_zeroed` đặt các
        // sentinel khác 0 tại chỗ, đúng đường `core_bridge::init` của ảnh thật.
        let mut machine = Machine::new();
        machine.init_zeroed(1);
        let untyped = machine.add_untyped(FAKE_FRAME_BASE, 64 * 1024);
        let report = LinkReport {
            addr: 0x8020_0000,
            static_bytes: core::mem::size_of::<Machine>(),
            pin: vifast_ktest::CORE_PIN_BYTES,
            base: FAKE_FRAME_BASE,
            arena_bytes: 64 * 1024,
            untyped,
            harts: 1,
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
            frames: RefCell::new(0),
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
        let mut n = self.frames.borrow_mut();
        let pa = FAKE_FRAME_BASE + u64::from(*n) * 4096;
        *n += 1;
        Some(FrameLabel {
            pa,
            label: (pa >> 12) as u32,
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

/// **Ca 1** — bộ ca dùng chung chạy trên nền tảng giả: 0 đỏ, đủ 10 TAP 43..52.
#[test]
fn shared_core_suite_runs_on_fake_platform() {
    let plat = FakePlatform::new();
    let bridge = FakeBridge::new();
    let outcome = core_suite::run(&plat, &bridge, ready);
    let out = plat.take();
    assert_eq!(
        outcome.fails, 0,
        "bộ ca dùng chung phải 0 đỏ trên nền tảng giả; log:\n{out}"
    );
    assert_eq!(outcome.taps, 10, "đủ 10 TAP (43..52); log:\n{out}");
    for n in 43..=52u32 {
        assert!(
            out.contains(&format!("ok {n} - ")),
            "thiếu TAP {n} trong log:\n{out}"
        );
    }
}

/// **Ca 2** — thứ tự + nhãn TAP ổn định (runner grep theo đúng chuỗi này).
#[test]
fn shared_core_suite_tap_labels_are_stable() {
    let plat = FakePlatform::new();
    let bridge = FakeBridge::new();
    let outcome = core_suite::run(&plat, &bridge, ready);
    let out = plat.take();
    let want = [
        (43u32, "core link"),
        (44, "core untyped boot"),
        (45, "core tcb retype"),
        (46, "core configure resume suspend"),
        (47, "core sched 2 tcb"),
        (48, "core ipc round"),
        (49, "core notification signal"),
        (50, "core waitaddr"),
        (51, "core d2-83 boundary"),
        (52, "core invariants"),
    ];
    assert_eq!(outcome.taps, want.len());
    for (n, name) in want {
        assert!(
            out.contains(&format!("ok {n} - {name}\n")),
            "TAP {n} phải là `ok {n} - {name}`; log:\n{out}"
        );
    }
    // Mỗi TAP đúng **một** lần (không phát lặp).
    for (n, _) in want {
        assert_eq!(
            out.matches(&format!("ok {n} - ")).count(),
            1,
            "TAP {n} phát đúng một lần; log:\n{out}"
        );
    }
}

/// **Ca 3** — `invariants::check_all` gom đúng 12 nhóm và xanh trên `Machine` mới.
#[test]
fn shared_invariants_cover_twelve_groups() {
    let bridge = FakeBridge::new();
    let untyped = bridge.report.untyped;
    let inv = bridge.with_machine(|m| invariants::check_all(m, untyped));
    assert_eq!(inv.groups(), 12, "12 nhóm check_* của core");
    assert!(inv.all_ok(), "Machine mới phải xanh 12/12: {inv:?}");
}

/// **Ca 4** — bộ kiểm có răng: một nhóm sai ⇒ `all_ok()` **sai**, `failures()` đếm đúng.
///
/// `check_bytes` nhận `ObjId` **không** phải Untyped ⇒ nhóm `bytes` đỏ. Đây là phép đo "hàm
/// không phải hằng `true`": nếu `all_ok()` bị viết cứng, ca này đỏ.
#[test]
fn shared_invariants_catch_a_broken_machine() {
    let bridge = FakeBridge::new();
    let untyped = bridge.report.untyped;
    let good = bridge.with_machine(|m| invariants::check_all(m, untyped));
    assert!(good.all_ok(), "Machine mới phải xanh 12/12: {good:?}");
    assert_eq!(good.failures(), 0);

    // ObjId 9999 không tồn tại ⇒ `check_bytes` phải ĐỎ (không phải "không kiểm gì").
    let bad = bridge.with_machine(|m| invariants::check_all(m, 9999));
    assert!(!bad.bytes, "check_bytes(ObjId rác) phải đỏ: {bad:?}");
    assert!(!bad.all_ok(), "một nhóm đỏ phải làm all_ok() sai: {bad:?}");
    assert_eq!(bad.failures(), 1, "đúng một nhóm đỏ: {bad:?}");
    // Các nhóm còn lại vẫn xanh — chứng minh phép đo **định vị** được nhóm hỏng.
    assert!(bad.caps && bad.sched && bad.ipc && bad.fault);
}

/// **Ca 5** — `Tap` là kiểu dữ liệu của bộ chung, không phải chuỗi viết tay trong từng arch.
#[test]
fn tap_type_is_shared() {
    let t = Tap::new(43, "core link");
    assert_eq!(t.number, 43);
    assert_eq!(t.name, "core link");
    assert_eq!(format!("{}", t.ok_line()), "ok 43 - core link\n");
    assert_eq!(
        format!("{}", t.not_ok_line("lý do")),
        "not ok 43 - core link: lý do\n"
    );
}

/// **Ca 6** — `ObjId` của báo cáo link trỏ đúng Untyped vừa thêm (không phải hằng chép tay).
#[test]
fn link_report_points_at_the_boot_untyped() {
    let bridge = FakeBridge::new();
    let report = bridge.link();
    let size = bridge.with_machine(|m| m.untyped_size(report.untyped));
    assert_eq!(size, report.arena_bytes);
    assert!(report.check_caps && report.check_bytes && report.check_all_bytes);
    assert!(report.static_bytes <= report.pin);
    let _: ObjId = report.untyped;
}
