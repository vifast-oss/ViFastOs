// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho bộ ca **SMP dùng chung** — KT-412 (`D2-259` mục 6, `D2-260`).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-ktest --test smp_suite
//! ```
//!
//! Đây là **ca kiểm đỏ-trước** (D2-05): khi tệp này được viết, `vifast_ktest::smp_suite` **chưa
//! tồn tại** ⇒ lượt chạy đầu **không biên dịch được** (đỏ đúng lý do, không phải đỏ do assert).
//! Sau khi hiện thực, các khẳng định dưới đây chốt bốn bất biến **host-đo-được**:
//!
//! 1. **một bộ ca, hai arch**: `smp_suite::run` chỉ phụ thuộc `Platform + SmpBridge` — không một
//!    dòng nào nhắc CSR/asm/MMIO của arch nào. Test chạy nó trên nền tảng **giả** (host) và đòi
//!    **0 ca đỏ** + **đủ 3 TAP**.
//! 2. **TAP ổn định**: chuỗi `ok <n> - <tên>` đúng thứ tự, mỗi dòng đúng **một** lần — runner
//!    `ktest.sh`/`run-aarch64.sh` grep theo đúng chuỗi này.
//! 3. **số hart lấy từ nguồn thật**: `SmpBridge::boot_harts()` (BootInfo/`smpboot`) quyết định
//!    `Machine::hart_count()` sau `set_harts(n)` — không hardcode 1, không hardcode 4.
//! 4. **bộ kiểm có răng**: một `Machine` bị làm hỏng (TCB `Running` trên hart không có trong mô
//!    hình) phải làm bộ ca **ĐỎ** — không phải hàm luôn trả `0`.

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

/// Cầu nối **giả**: `Machine` thật của `vifast-core` trên host + số hart "BootInfo" giả.
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

/// **Ca 1** — bộ ca SMP dùng chung chạy trên nền tảng giả: 0 đỏ, đủ 3 TAP.
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

/// **Ca 5** — hằng số của bộ ca là **một** nguồn: 4 hart, 2 TCB/hart, ≥ 8 tick.
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
