// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho bộ ca **SMP dùng chung 3 arch** — KT-412 (`D2-259` mục 6) + KT-413b.
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-ktest --test smp_suite
//! ```
//!
//! # KT-413b — một tệp test cho **một** bộ ca
//!
//! Trước KT-413b có **hai** tệp (`tests/smp_suite.rs` cho rv/a64 + `tests/smp_suite_x64.rs` cho
//! x64) và **hai** trait `SmpBridge` khác nhau. KT-413b gộp về **một** mô-đun `smp_suite` với
//! **một** trait; tệp này giữ **đủ 9 ca** của cả hai tệp cũ (không bỏ ca nào — AGENTS §4.3) và
//! thêm ca **hợp nhất**: hai cửa vào `run` (profile rv/a64, 3 TAP) và `run_x64` (profile x64,
//! 2 TAP) nằm **cùng** một mô-đun, cùng một trait, cùng một `Machine` thật.
//!
//! Đây là **ca kiểm đỏ-trước** (D2-05) của phần hợp nhất: khi tệp này được viết, `run_x64` **chưa**
//! tồn tại ⇒ lượt chạy đầu **không biên dịch được** (đỏ đúng lý do "API chưa có").
//!
//! # Bất biến host-đo-được
//!
//! 1. **một bộ ca, ba arch**: cả hai cửa vào chỉ phụ thuộc `Platform + SmpBridge` — không một
//!    dòng nào nhắc CSR/asm/MMIO của arch nào.
//! 2. **TAP ổn định**: profile rv/a64 phát `ok 72..74 - smp …`; profile x64 phát
//!    `ok 48/49 - core smp …` — runner `ktest.sh`/`run-aarch64.sh`/`run-x86_64.sh` grep đúng chuỗi
//!    này, nên **không** được đổi nhãn/số.
//! 3. **số hart lấy từ nguồn thật**: `SmpBridge::boot_harts()` quyết định `Machine::hart_count()`
//!    sau `set_harts(n)` — không hardcode 1, không hardcode 4.
//! 4. **bộ kiểm có răng**: `Machine` lệch nguồn thật (rv/a64) hoặc 1 hart (x64) phải làm bộ ca
//!    **ĐỎ** — không phải hàm luôn trả `0`.

use std::cell::RefCell;
use std::string::String;

use vifast_core::{Machine, ObjId};
use vifast_ktest::smp_suite::{self, SmpBridge, SMP_HARTS};
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

/// Cầu nối **giả** cho profile rv/a64: `Machine` thật của `vifast-core` trên host + số hart
/// "BootInfo" giả.
struct FakeBridge {
    machine: RefCell<Machine>,
    boot_harts: usize,
    untyped: ObjId,
}

const FAKE_ARENA_BASE: u64 = 0x8020_0000;
const FAKE_ARENA_BYTES: usize = 64 * 1024;

impl FakeBridge {
    fn new(boot_harts: usize) -> Self {
        let mut machine = Machine::new();
        machine.init_zeroed(1);
        let untyped = machine.add_untyped(FAKE_ARENA_BASE, FAKE_ARENA_BYTES);
        Self {
            machine: RefCell::new(machine),
            boot_harts,
            untyped,
        }
    }
}

impl SmpBridge for FakeBridge {
    fn with_machine<R>(&self, f: impl FnOnce(&mut Machine) -> R) -> R {
        f(&mut self.machine.borrow_mut())
    }

    fn boot_harts(&self) -> usize {
        self.boot_harts
    }

    fn untyped(&self) -> ObjId {
        self.untyped
    }
}

/// **Ca 1** — profile rv/a64 chạy trên nền tảng giả: 0 đỏ, đủ 3 TAP.
#[test]
fn smp_suite_runs_on_fake_platform() {
    let plat = FakePlatform::new();
    let bridge = FakeBridge::new(4);
    let outcome = smp_suite::run(&plat, &bridge, 72);
    let out = plat.take();
    assert_eq!(
        outcome.fails, 0,
        "bộ ca SMP phải 0 đỏ trên nền tảng giả; log:\n{out}"
    );
    assert_eq!(outcome.taps, 3, "đủ 3 TAP SMP; log:\n{out}");
    for n in 72..=74u32 {
        assert!(
            out.contains(&format!("ok {n} - ")),
            "thiếu TAP {n} trong log:\n{out}"
        );
    }
}

/// **Ca 2** — thứ tự + nhãn TAP ổn định (runner grep theo đúng chuỗi này).
#[test]
fn smp_suite_tap_labels_are_stable() {
    let plat = FakePlatform::new();
    let bridge = FakeBridge::new(4);
    let outcome = smp_suite::run(&plat, &bridge, 72);
    let out = plat.take();
    let want = [
        (72u32, "smp harts bootinfo"),
        (73, "smp 4 hart preemptive"),
        (74, "smp published rule"),
    ];
    assert_eq!(outcome.taps, want.len());
    for (n, name) in want {
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
}

/// **Ca 3** — số hart lấy từ `boot_harts()` (nguồn thật), không hardcode.
#[test]
fn smp_suite_uses_boot_hart_count() {
    for boot in [1usize, 2, 4] {
        let plat = FakePlatform::new();
        let bridge = FakeBridge::new(boot);
        let outcome = smp_suite::run(&plat, &bridge, 72);
        let out = plat.take();
        assert_eq!(outcome.fails, 0, "boot_harts={boot} phải xanh; log:\n{out}");
        assert!(
            out.contains(&format!("# smp: boot_harts={boot} model_harts={boot} ")),
            "TAP 72 phải báo đúng số hart thật ({boot}); log:\n{out}"
        );
    }
}

/// **Ca 4** — bộ kiểm có răng: `Machine` hỏng (hart count lệch nguồn thật) ⇒ TAP 72 **ĐỎ**.
///
/// `boot_harts()` trả 4 nhưng `Machine` bị khoá ở 1 hart (mô phỏng "quên `set_harts` từ
/// `BootInfo`") ⇒ bộ ca phải bắt. Nếu `run` bỏ qua phép so, ca này đỏ.
#[test]
fn smp_suite_catches_hart_count_mismatch() {
    let plat = FakePlatform::new();
    let bridge = FakeBridge::new(4);
    // Làm hỏng: `set_harts` bị vô hiệu bằng cách ghim mô hình ở 1 hart **sau** khi suite chạy
    // xong phần A — mô phỏng đúng lỗi "BootInfo nói 4, model chạy 1".
    let out = {
        let outcome = smp_suite::run(&plat, &bridge, 72);
        assert_eq!(outcome.fails, 0, "lượt đầu phải xanh");
        plat.take()
    };
    assert!(out.contains("ok 72 - smp harts bootinfo\n"), "log:\n{out}");

    // Đối chứng âm: nguồn thật nói 4 nhưng mô hình chỉ có 1 ⇒ `run` phải ĐỎ.
    let plat2 = FakePlatform::new();
    let broken = BrokenBridge::new(4);
    let outcome = smp_suite::run(&plat2, &broken, 72);
    let out2 = plat2.take();
    assert!(
        outcome.fails >= 1,
        "hart count lệch nguồn thật phải làm bộ ca ĐỎ; log:\n{out2}"
    );
    assert!(
        out2.contains("not ok 72 - smp harts bootinfo"),
        "TAP 72 phải ĐỎ khi model_harts != boot_harts; log:\n{out2}"
    );
}

/// Cầu nối **hỏng** — `boot_harts()` nói 4 nhưng lời gọi `set_harts` **đầu tiên** bị bỏ qua
/// (mô phỏng đúng lỗi "quên `set_harts` từ `BootInfo` lúc init"): mô hình giữ 1 hart trong khi
/// nguồn thật nói 4. Các lời gọi sau vẫn thi hành để TAP 2/3 chạy được (đo **đúng** TAP 1 đỏ).
struct BrokenBridge {
    machine: RefCell<Machine>,
    boot_harts: usize,
    untyped: ObjId,
    calls: std::cell::Cell<u32>,
}

impl BrokenBridge {
    fn new(boot_harts: usize) -> Self {
        let mut machine = Machine::new();
        machine.init_zeroed(1);
        let untyped = machine.add_untyped(FAKE_ARENA_BASE, FAKE_ARENA_BYTES);
        Self {
            machine: RefCell::new(machine),
            boot_harts,
            untyped,
            calls: std::cell::Cell::new(0),
        }
    }
}

impl SmpBridge for BrokenBridge {
    fn with_machine<R>(&self, f: impl FnOnce(&mut Machine) -> R) -> R {
        f(&mut self.machine.borrow_mut())
    }

    fn boot_harts(&self) -> usize {
        self.boot_harts
    }

    fn untyped(&self) -> ObjId {
        self.untyped
    }

    /// **Cố ý hỏng**: bỏ qua lời gọi **đầu tiên** ⇒ mô hình giữ 1 hart dù nguồn thật nói 4.
    fn set_harts(&self, m: &mut Machine, n: u16) {
        let c = self.calls.get();
        self.calls.set(c + 1);
        if c == 0 {
            return;
        }
        m.set_harts(n);
    }
}

/// **Ca 5** — hằng số của profile rv/a64 là **một** nguồn: 4 hart, 2 TCB/hart, ≥ 8 tick.
///
/// Kiểm ở **biên dịch** (`const _`), không phải `assert!` runtime: đây là hằng số của phạm vi
/// KT-412, đổi chúng là đổi phạm vi task — phải là lỗi biên dịch, không phải test đỏ lúc chạy.
const _: () = assert!(SMP_HARTS == 4, "SMP 4 hart la pham vi KT-412");
const _: () = assert!(
    smp_suite::TCB_PER_HART == 2,
    "2 TCB/hart => moi hart co hang cho that"
);
const _: () = assert!(
    smp_suite::TICKS >= 8,
    "du tick de moi TCB chay it nhat mot lan"
);
const _: () = assert!(
    smp_suite::SMP_TCBS == SMP_HARTS as usize * smp_suite::TCB_PER_HART,
    "SMP_TCBS phai bang harts x TCB/hart"
);

/// **Ca 5** — chạy được (giữ một `#[test]` để `cargo test` báo ca này có mặt).
#[test]
fn smp_suite_constants_are_shared() {
    assert_eq!(smp_suite::SMP_TCBS, 8);
}

// =============================================================================================
// Profile **x86_64** — 2 TAP 48/49 (`core smp 4 hart`, `core smp sleep migrate`).
// =============================================================================================

/// Cầu nối **giả** cho profile x64: `Machine` thật của `vifast-core` trên host, `harts` cấu hình
/// được. Cài **trực tiếp** [`SmpBridge`] (không qua `CoreBridge`) — đúng hợp đồng **hẹp** mà bộ
/// ca cần, và chứng minh bộ ca **không** đòi object layer.
struct FakeBridgeX64 {
    machine: RefCell<Machine>,
    harts: usize,
    untyped: ObjId,
}

impl FakeBridgeX64 {
    fn new(harts: u16) -> Self {
        let mut machine = Machine::new();
        machine.init_zeroed(harts);
        let untyped = machine.add_untyped(FAKE_ARENA_BASE, FAKE_ARENA_BYTES);
        Self {
            machine: RefCell::new(machine),
            harts: harts as usize,
            untyped,
        }
    }
}

impl SmpBridge for FakeBridgeX64 {
    fn with_machine<R>(&self, f: impl FnOnce(&mut Machine) -> R) -> R {
        f(&mut self.machine.borrow_mut())
    }

    fn boot_harts(&self) -> usize {
        self.harts
    }

    fn untyped(&self) -> ObjId {
        self.untyped
    }
}

/// **Ca 6** — profile x64 chạy trên nền tảng giả 4 hart: 0 đỏ, đủ 2 TAP 48..49.
#[test]
fn shared_smp_suite_runs_on_fake_platform() {
    let plat = FakePlatform::new();
    let bridge = FakeBridgeX64::new(SMP_HARTS);
    let outcome = smp_suite::run_x64(&plat, &bridge, SMP_HARTS);
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

/// **Ca 7** — nhãn TAP ổn định + dòng bằng chứng `# core smp:` đúng một lần.
#[test]
fn shared_smp_suite_tap_labels_are_stable() {
    let plat = FakePlatform::new();
    let bridge = FakeBridgeX64::new(SMP_HARTS);
    let outcome = smp_suite::run_x64(&plat, &bridge, SMP_HARTS);
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

/// **Ca 8** — bốn hart **thật**: mỗi hart chọn ≥ 1, `distinct >= 2`, `double == 0`.
#[test]
fn shared_smp_suite_uses_all_four_harts() {
    let plat = FakePlatform::new();
    let bridge = FakeBridgeX64::new(SMP_HARTS);
    let report = smp_suite::run_x64(&plat, &bridge, SMP_HARTS).report;
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

/// **Ca 9** — bộ ca **có răng**: `Machine` 1 hart ⇒ `fails > 0` (không phải hàm luôn xanh).
#[test]
fn shared_smp_suite_fails_on_single_hart_machine() {
    let plat = FakePlatform::new();
    let bridge = FakeBridgeX64::new(1);
    let outcome = smp_suite::run_x64(&plat, &bridge, SMP_HARTS);
    let out = plat.take();
    assert!(
        outcome.fails > 0,
        "Machine 1 hart phải làm bộ ca SMP đỏ; log:\n{out}"
    );
    assert!(
        out.contains("not ok 48 - core smp 4 hart"),
        "TAP 48 phải đỏ khi chỉ có 1 hart; log:\n{out}"
    );
    let _: ObjId = bridge.untyped;
}

/// **Ca 10** — **hợp nhất**: hai profile nằm **cùng** một mô-đun, cùng một trait, và **không**
/// giẫm số TAP của nhau (rv/a64 dùng `first` tham số; x64 cố định 48/49 theo hợp đồng runner).
#[test]
fn both_profiles_share_one_module_and_one_bridge() {
    // Cùng một kiểu `SmpBridge` phục vụ cả hai cửa vào (biên dịch = bằng chứng).
    fn takes_bridge<B: SmpBridge>(_: &B) {}
    let shared = FakeBridge::new(4);
    let x64 = FakeBridgeX64::new(SMP_HARTS);
    takes_bridge(&shared);
    takes_bridge(&x64);

    // Profile rv/a64 đánh số theo `first`; profile x64 cố định 48/49.
    let plat = FakePlatform::new();
    let a = smp_suite::run(&plat, &shared, 10);
    let out_a = plat.take();
    assert_eq!(a.taps, 3);
    assert!(out_a.contains("ok 10 - smp harts bootinfo\n"), "{out_a}");
    assert!(out_a.contains("ok 12 - smp published rule\n"), "{out_a}");

    let plat = FakePlatform::new();
    let b = smp_suite::run_x64(&plat, &x64, SMP_HARTS);
    let out_b = plat.take();
    assert_eq!(b.taps, 2);
    assert!(out_b.contains("ok 48 - core smp 4 hart\n"), "{out_b}");
    assert!(
        out_b.contains("ok 49 - core smp sleep migrate\n"),
        "{out_b}"
    );
    // Hai profile **không** phát cùng nhãn TAP.
    assert!(!out_b.contains("smp harts bootinfo"), "{out_b}");
    assert!(!out_a.contains("core smp 4 hart"), "{out_a}");
}
