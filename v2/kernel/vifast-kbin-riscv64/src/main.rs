// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ảnh **sản phẩm** riscv64 cho K3 (KT-44, KT-45, KT-46, KT-47).
//!
//! Không chứa chuỗi/đường dẫn `ktest` (D2-08); ảnh test là bin `vifast-kbin-ktest`
//! (`src/ktest.rs`, feature `ktest`). KT-45 cài `stvec` **direct mode** ngay đầu boot và in lại
//! `stvec` để chứng minh vector thật trên phần cứng; trap thật được kiểm trong ảnh test.
//!
//! KT-47: sau MMU, ảnh sản phẩm **arm timer 100 Hz** bằng SBI TIME (`sbi_set_timer`) + `sie.STIE`,
//! chờ **≥1 tick** thật (timeout 1 s) rồi in `# timer …` bằng chứng. Timer hỏng ⇒ `PANIC` (runner
//! đỏ), không giả xanh. PLIC/UART IRQ (KT-48), SMP (KT-49), U-mode (KT-53) **không** thuộc ảnh này.
#![no_std]
#![no_main]

use core::fmt::Write;

use vifast_plat_common::timer::{ticks_to_ns, TICK_HZ, TICK_TICKS, TIMEBASE_HZ, TIMER_STATS};
use vifast_plat_common::{PanicHandler, Platform, PlatformWriter};
use vifast_plat_riscv64::mmu;
use vifast_plat_riscv64::timer;
use vifast_plat_riscv64::{read_trap_vector, Riscv64Virt};

/// Nền tảng QEMU `virt` — zero-sized, dựng được trong `static`.
static PLATFORM: Riscv64Virt = Riscv64Virt;
/// Một handler panic dùng chung cho cả ảnh sản phẩm và ảnh test (`vifast-plat-common`).
static PANIC: PanicHandler<Riscv64Virt> = PanicHandler::new(Riscv64Virt);

/// Được `_start` (trong `vifast-plat-riscv64`) gọi sau khi đặt `sp`/`tp`/`sscratch`.
#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    PLATFORM.put_str("TAP version 13\n1..1\n");
    PLATFORM.put_str("# k3 image=product (kernel thật trên QEMU virt, không phải model)\n");
    // KT-45: cài vector trap direct-mode trước mọi việc khác; in lại stvec để runner thấy giá trị
    // thật (không phải hằng số viết tay trong tài liệu — QD §9 Q12). Trap sống trước MMU để lỗi
    // mapping lúc bật `satp` được dump thay vì treo im lặng.
    PLATFORM.set_trap_vector(vifast_plat_riscv64::trap_vector());
    let stvec = read_trap_vector();
    // KT-46: dựng bảng Sv39 từ arena RAM **xong** mới bật `satp` + `sfence.vma`, rồi in địa chỉ
    // thật (root/arena/guard) do lần boot này sinh ra.
    let mmu = mmu::init();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# mmu: satp=0x{:x} mode={} root=0x{:x} tables={} used={} arena=0x{:x}-0x{:x} kernel_end=0x{:x}",
            mmu.satp,
            mmu.satp >> 60,
            mmu.root,
            mmu.tables,
            mmu.used,
            mmu.arena_start,
            mmu.arena_end,
            mmu.kernel_end
        );
        let _ = writeln!(
            w,
            "# mmu map: high=0x{:x} physmap=0x{:x} guard=0x{:x}-0x{:x} guard_none=1 alias={}",
            mmu.high_base, mmu.physmap_base, mmu.guard_start, mmu.guard_end, mmu.alias_ok as u8
        );
        let _ = writeln!(
            w,
            "# hart={} timebase_hz={} tick_hz={} time={} stvec=0x{:x} direct={}",
            PLATFORM.hart_id(),
            PLATFORM.timebase_hz(),
            TICK_HZ,
            PLATFORM.now_ticks(),
            stvec,
            stvec & 0b11 == 0
        );
    }

    // KT-47: **timer SBI** — arm deadline đầu (`now + TICK_TICKS`) rồi mới bật `sie.STIE` +
    // `sstatus.SIE` (P2a: trap entry/handler không ghi `sstatus`; `sret` khôi phục SIE từ SPIE).
    let timer_arm = timer::init();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# timer arm: now={} first_deadline={} tick_ticks={} tick_hz={} timebase_hz={} sie_stie={} sstatus_sie={}",
            timer_arm.now,
            timer_arm.first_deadline,
            TICK_TICKS,
            TICK_HZ,
            TIMEBASE_HZ,
            timer_arm.sie_stie as u8,
            timer_arm.sstatus_sie as u8
        );
    }
    // Chờ **≥1 tick thật**; timeout 1 s (100 tick) để boot hỏng không treo im lặng. Busy-spin để
    // không phụ thuộc `wfi` (QEMU chỉ phục vụ timer ảo khi vCPU chạy — xem `ktest.sh` về icount).
    let start = PLATFORM.now_ticks();
    let mut snap = TIMER_STATS.snapshot();
    while snap.ticks == 0 {
        if PLATFORM.now_ticks().wrapping_sub(start) > PLATFORM.timebase_hz() {
            PLATFORM.put_str("PANIC: timer khong tick trong 1 s\n");
            PLATFORM.halt(1)
        }
        core::hint::spin_loop();
        snap = TIMER_STATS.snapshot();
    }
    let sret_sie = timer::read_sstatus() & timer::SSTATUS_SIE != 0;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# timer: ticks={} delta_ns={} late={} missed={} non_monotonic={} last={} deadline={} sret_sie={}",
            snap.ticks,
            ticks_to_ns(snap.max_delta),
            snap.late,
            snap.missed,
            snap.non_monotonic,
            snap.last,
            snap.deadline,
            sret_sie as u8
        );
    }
    // Dừng timer: deadline xa dập ngắt đang chờ rồi tắt cổng (sstatus.SIE → sie.STIE).
    PLATFORM.set_timer_deadline(u64::MAX);
    PLATFORM.disable_timer();
    let timer_ok = timer_arm.sie_stie
        && timer_arm.sstatus_sie
        && sret_sie
        && snap.ticks >= 1
        && snap.non_monotonic == 0
        && timer::read_sie() & timer::SIE_STIE == 0
        && timer::read_sstatus() & timer::SSTATUS_SIE == 0;
    // Ảnh sản phẩm chỉ TAP 1 dòng, nhưng MMU + timer phải thật: satp mode 8, đọc lại được, alias
    // ghi/đọc khớp, timer nạp deadline + bật SIE + có ≥1 ngắt thật qua `stvec`. Sai ⇒ PANIC
    // (run.sh coi là đỏ), không giả xanh.
    if mmu.satp >> 60 != 8 || !mmu::mmu_enabled() || !mmu.alias_ok {
        PLATFORM.put_str("PANIC: mmu init product\n");
        PLATFORM.halt(1)
    }
    if !timer_ok {
        PLATFORM.put_str("PANIC: timer init product\n");
        PLATFORM.halt(1)
    }
    PLATFORM.put_str("ok 1 - k3 boot\n");
    PLATFORM.put_str("# k3 halt\n");
    PLATFORM.halt(0)
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    PANIC.handle(info)
}
