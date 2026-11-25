// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-413** — pha **SMP của `vifast-core`**: bốn hart `x86_64` vào `Machine` qua `SmpBridge` +
//! bộ ca SMP **dùng chung** `vifast_ktest::smp_suite` (TAP 48/49).
//!
//! # Vì sao pha này tồn tại
//!
//! `KT-340c` nối `vifast-core::Machine` vào ảnh nhưng `core_bridge::init` chạy **trước** SMP với
//! `harts = 1` (nợ tự ghi của `KT-340c`), và pha core chỉ chạy trên BSP. `KT-413` đóng nợ ấy:
//! sau khi `smp::run` đưa **bốn** hart lên, model được dựng lại với **số hart đo được**
//! (`D2-88(a)`: không hardcode 1), rồi bộ ca SMP dùng chung chạy trên `Machine` **thật** của ảnh.
//!
//! # Vì sao dựng lại model (không tái dùng model của pha core)
//!
//! `Machine::init_zeroed(harts)` chỉ nhận số hart **một lần**; đổi `n_harts` giữa chừng sẽ làm
//! `PerHartState`/hàng đợi của hart mới không được khởi tạo. Pha core (TAP 38..43) và pha shell
//! (TAP 44..47) đã **xong** trước `smp::run` (chúng chạy trong `umode::run`), nên dựng lại model
//! với `harts = 4` là đường duy nhất đúng — và nó **không** làm mất bằng chứng nào đã in.
//!
//! # Bằng chứng
//!
//! - `# core smp: harts=4 picks=[…] first=[…] distinct=… double=0 …` — **một** dòng, giá trị **đo**
//!   từ `Machine` (không hardcode), do chính bộ ca chung in.
//! - TAP **48** `core smp 4 hart` — cả bốn hart `dispatch` được ≥ 1 TCB, không double-pick,
//!   priority tĩnh đúng.
//! - TAP **49** `core smp sleep migrate` — `block_current` + `wake` + `dispatch` lại đúng,
//!   migration chạy, bốn bất biến `check_sched`/`check_sc_refs`/`check_budget`/`check_mailboxes`
//!   xanh.
//!
//! # Vì sao TAP 48/49 (không phải 46/47)
//!
//! Shell sản phẩm (`KT-341b`) đã chiếm **44..47** và `gpu` chiếm **48**; 49/50 là số **kế tiếp**
//! tự do, nên TAP 1..48 **không đổi** nghĩa (bất biến "không đổi hợp đồng cũ" của runner).

use core::fmt::Write;
use core::sync::atomic::{AtomicU32, Ordering};

use vifast_core::Machine;
use vifast_ktest::smp_suite::{self, SmpBridge, X64_HARTS};
use vifast_plat_common::Platform;
use vifast_plat_x86_64::bootmem::BootArena;
use vifast_plat_x86_64::core_bridge;
use vifast_plat_x86_64::mmu::TableMemory;

use crate::{asm, Console};

/// Số frame 4 KiB cấp cho Untyped boot của model SMP (nhãn PA — `D2-83`: core chỉ kế toán byte).
const SMP_ARENA_FRAMES: usize = 4;

/// `ObjId` của Untyped boot — bộ ca chung cần nó để `retype`.
static UNTYPED_ID: AtomicU32 = AtomicU32::new(0);

/// Nền tảng của ảnh cho bộ ca chung — chỉ dùng `put_str`/`halt`/`now_ticks`/`hart_id`.
struct ImagePlatform;

impl Platform for ImagePlatform {
    fn put_str(&self, s: &str) {
        let mut console = Console;
        let _ = console.write_str(s);
    }

    fn halt(&self, code: usize) -> ! {
        asm::halt(code as u32)
    }

    fn now_ticks(&self) -> u64 {
        crate::TICK_COUNT.load(Ordering::Relaxed)
    }

    fn timebase_hz(&self) -> u64 {
        // PIT kênh 0 chạy 100 Hz (`apic::PIT_FREQUENCY_HZ / divisor`); bộ ca SMP không dùng
        // `now_ticks` cho phép đo thời gian, nên giá trị này chỉ để hợp đồng trait đủ.
        100
    }

    fn hart_id(&self) -> usize {
        crate::smp::hart_of_lapic(crate::lapic_id()).unwrap_or(0)
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

/// Cầu nối **hẹp** của ảnh: `with_machine` + số hart **đo được** + `ObjId` Untyped boot — đúng
/// ba thứ [`vifast_ktest::smp_suite::SmpBridge`] cần.
///
/// Không cài `CoreBridge` (đòi object layer `Frame`/`VSpace` mà kernel `x86_64` **chưa** có —
/// `KT-340c` chỉ nối `Machine`): bịa ba method giả sẽ là bằng chứng sai (`D2-83`/`D2-102`).
///
/// **KT-413b**: trước đây cài trait `vifast_ktest::SmpBridge` (bản `with_machine` + `link` của
/// `lib.rs`); hợp nhất về **một** trait `smp_suite::SmpBridge` cho cả ba arch nên `link()` —
/// vốn chỉ để lấy `untyped` — thay bằng đúng hai method hẹp.
struct ImageBridge;

impl SmpBridge for ImageBridge {
    fn with_machine<R>(&self, f: impl FnOnce(&mut Machine) -> R) -> R {
        core_bridge::with_machine(f)
    }

    fn boot_harts(&self) -> usize {
        core_bridge::harts()
    }

    fn untyped(&self) -> vifast_core::ObjId {
        UNTYPED_ID.load(Ordering::Acquire)
    }
}

/// Chạy pha SMP của core: dựng lại model với **số hart đo được**, rồi chạy bộ ca chung.
///
/// Gọi từ [`crate::net::run`] **ngay sau** `smp::run` (TAP 18..21) — lúc ấy cả bốn hart đã online
/// và chưa hart nào vào pha preemption của `sched::run`.
pub fn run(console: &mut Console, tap_first: usize) {
    let harts = crate::smp::online_count();
    let _ = writeln!(
        console,
        "# core smp: online mask=0x{:x} harts={harts} expect={X64_HARTS}",
        crate::smp::online_mask(),
    );
    if harts != X64_HARTS {
        // Fail-closed: bộ ca chung đòi **đúng** bốn hart; thiếu hart là TAP 18 đã đỏ trước đó,
        // nhưng vẫn phải in TAP 48/49 đỏ thay vì im lặng bỏ qua pha.
        let _ = writeln!(
            console,
            "not ok 48 - core smp 4 hart: harts={harts} (cần {X64_HARTS})"
        );
        let _ = writeln!(
            console,
            "not ok 49 - core smp sleep migrate: harts={harts} (cần {X64_HARTS})"
        );
        let _ = writeln!(console, "# TAP 48 FAIL — dừng (không in marker)");
        asm::halt(3)
    }

    // ---- 1. Untyped boot **nhãn PA** từ arena bảng trang (D2-83: core không dereference).
    let Some(mut arena) = BootArena::new(
        crate::bootpt::arena_base(),
        crate::bootpt::arena_frames(),
        crate::bootpt::arena_next(),
    ) else {
        let _ = writeln!(console, "PANIC: coresmp: arena không hợp lệ");
        asm::halt(3)
    };
    let mut base = 0u64;
    for index in 0..SMP_ARENA_FRAMES {
        let Some(pa) = arena.alloc_frame() else {
            let _ = writeln!(
                console,
                "PANIC: coresmp: arena hết frame (đã dùng {} / {})",
                arena.used(),
                arena.frames()
            );
            asm::halt(3)
        };
        if index == 0 {
            base = pa;
        }
    }
    let bytes = SMP_ARENA_FRAMES * 0x1000;

    // ---- 2. Dựng lại model với **số hart đo được** (D2-88(a)) — pha core/shell đã xong.
    let report = core_bridge::init(base, bytes, harts);
    UNTYPED_ID.store(report.untyped, Ordering::Release);
    let _ = writeln!(
        console,
        "# core smp: init addr=0x{:x} size={} pin={} harts={} untyped={} base=0x{base:x} bytes={bytes} healthy={}",
        report.addr,
        report.static_bytes,
        report.pin,
        report.harts,
        report.untyped,
        u8::from(report.healthy()),
    );
    if !report.healthy() {
        let _ = writeln!(console, "PANIC: coresmp: model không healthy");
        asm::halt(3)
    }

    // ---- 3. Bộ ca SMP **dùng chung** (TAP 48/49 + dòng `# core smp:` bằng chứng).
    let outcome = smp_suite::run_x64(&ImagePlatform, &ImageBridge, harts as u16);
    let _ = writeln!(
        console,
        "# core smp: taps={} fails={} ok={}",
        outcome.taps,
        outcome.fails,
        u8::from(outcome.fails == 0),
    );
    if outcome.fails != 0 {
        let _ = writeln!(console, "# TAP {} FAIL — dừng (không in marker)", tap_first);
        asm::halt(3)
    }
}
