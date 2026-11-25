// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-412** (`D2-259` mục 6, `D2-260`) — adapter chạy bộ ca **SMP dùng chung**
//! (`vifast_ktest::smp_suite`, TAP **47..49**) trên ảnh kiểm `aarch64`.
//!
//! # Vì sao adapter ở **ảnh** mà không ở `vifast-plat-aarch64`
//!
//! `vifast_ktest::smp_suite` cần hai hợp đồng: [`vifast_ktest::smp_suite::SmpBridge`] (khoá
//! `Machine` + số hart thật + Untyped) và [`vifast_plat_common::Platform`] (in + đồng hồ). Crate
//! `vifast-plat-aarch64` **không** cài `Platform` (nó là crate thuần dữ liệu/asm, `Platform` của
//! `riscv64` nằm ở `vifast-plat-riscv64` vì lịch sử K3). Ảnh kiểm đã có `Console` + `debug_halt`
//! + `asm::read_cntpct_el0` nên cài **tại đây** là đường ngắn nhất, và giữ ảnh **sản phẩm** 0
//! byte harness (mô-đun này chỉ biên dịch dưới `feature = "ktest"`, D2-08).
//!
//! # `Platform` ở đây là **hẹp**, không phải nền tảng sản phẩm
//!
//! Bộ ca SMP chỉ dùng `put_str` + `now_ticks` (+ `timebase_hz` cho quy đổi). Bốn hàm IRQ/timer
//! **không** được bộ ca gọi; chúng trả `false` (fail-closed: "chưa làm") thay vì `true` giả —
//! nếu sau này bộ ca gọi tới, phép kiểm sẽ **đỏ** chứ không im lặng tin một no-op.

use core::fmt::Write;

use vifast_core::{Machine, ObjId};
use vifast_ktest::smp_suite::{self, SmpBridge};
use vifast_plat_common::Platform;

use vifast_plat_aarch64::{asm, core_bridge, debug_halt, pl011};

use crate::Console;

/// Số TAP đầu của bộ ca SMP trên ảnh `aarch64` (36..41 core, 42..44 k5, 45..46 shell).
pub const FIRST_TAP: u32 = 47;

/// Nền tảng **hẹp** cho bộ ca SMP — xem đầu tệp.
pub struct SmpPlat;

impl Platform for SmpPlat {
    fn put_str(&self, s: &str) {
        let _ = pl011::write_str(s);
    }

    fn halt(&self, code: usize) -> ! {
        debug_halt(code)
    }

    fn now_ticks(&self) -> u64 {
        asm::read_cntpct_el0()
    }

    fn timebase_hz(&self) -> u64 {
        asm::read_cntfrq_el0()
    }

    fn hart_id(&self) -> usize {
        // `MPIDR_EL1.Aff0` — hart id của QEMU `virt` (cùng nguồn `smpboot` dùng).
        (asm::read_mpidr_el1() & 0xFF) as usize
    }

    fn set_trap_vector(&self, _vector: usize) {
        // Vector đã cài từ `kmain`; bộ ca SMP không đổi vector.
    }

    /// **Chưa cài** — trả `false` (fail-closed), không tuyên bố đã bật.
    fn enable_irq(&self, _irq: u32) -> bool {
        false
    }

    /// **Chưa cài** — trả `false` (fail-closed).
    fn disable_irq(&self, _irq: u32) -> bool {
        false
    }

    fn set_timer_deadline(&self, _deadline: u64) {
        // Bộ ca dùng `now_ticks` làm nguồn tick; không arm timer phần cứng.
    }

    /// **Chưa cài** — trả `false` (fail-closed).
    fn enable_timer(&self) -> bool {
        false
    }

    fn disable_timer(&self) {}
}

/// Cầu nối cho bộ ca SMP — `Machine` **static trong ảnh** + số hart **thật** từ `BootInfo`.
pub struct SmpBridgeImpl;

impl SmpBridge for SmpBridgeImpl {
    fn with_machine<R>(&self, f: impl FnOnce(&mut Machine) -> R) -> R {
        core_bridge::with_machine(f)
    }

    fn boot_harts(&self) -> usize {
        // `core_bridge::harts()` = số hart `init` đã đặt qua `Machine::set_harts(n)`, lấy từ
        // `BootInfo::n_harts` (`KT-402`) khi bảng boot parse được.
        core_bridge::harts()
    }

    fn untyped(&self) -> ObjId {
        crate::coretest::untyped()
    }
}

/// Chạy bộ ca SMP dùng chung (TAP 47..49); trả số TAP đỏ.
pub fn smp_phase(console: &mut Console) -> usize {
    let outcome = smp_suite::run(&SmpPlat, &SmpBridgeImpl, FIRST_TAP);
    let _ = writeln!(
        console,
        "# smp core: taps={} fails={} boot_harts={}",
        outcome.taps,
        outcome.fails,
        core_bridge::harts()
    );
    outcome.fails
}
