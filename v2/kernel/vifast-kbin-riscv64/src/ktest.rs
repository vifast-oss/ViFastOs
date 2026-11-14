// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ảnh **test** riscv64 cho K3 (KT-44, KT-45, KT-46) — bin riêng, feature `ktest`, nạp qua
//! OpenSBI. D2-08: ảnh test **không** nằm trong ảnh sản phẩm.
//!
//! KT-46/47 mở rộng ảnh này lên **12 TAP**:
//!
//! 1. `ok 1 - ktest image` — marker tách ảnh của KT-44.
//! 2. `ok 2 - trap breakpoint` — **trap phần cứng thật** (`ebreak` → `scause=3`, resume đúng
//!    `sepc`) theo D2-73.
//! 3. `ok 3 - trap ecall sbi` — `ecall` S-mode do M-mode/OpenSBI xử lý, resume đúng `sepc`.
//! 4. `ok 4 - trap illegal inject` — `illegal` (scause 2) **tiêm**, ghi rõ "inject" (D2-73).
//! 5. `ok 5 - mmu on` — `satp` mode 8, root từ arena RAM, đọc lại `satp`, walk phần mềm khớp
//!    phần cứng (UART/kernel identity + nửa cao), alias ghi/đọc.
//! 6. `ok 6 - map layout` — guard NONE ở cả 3 alias; `.stack`/`.trap_stack`/arena RW NX; physmap
//!    2 MiB **NX**; UART/PLIC Device 4 KiB; kernel nửa cao X.
//! 7. `ok 7 - walk khop` — PTE đọc lại **sau** khi bật `satp` bằng PTE trước; walk phần mềm trả
//!    đúng PTE; alias identity ↔ nửa cao ↔ physmap ghi/đọc khớp (hardware walk thật).
//! 8. `ok 8 - timer 100 ticks monotonic` — SBI TIME + `sie.STIE`: 100 ngắt timer **thật**, mỗi
//!    tick `rdtime` tăng (monotonic), không có tick nào `now <= last` (KT-47).
//! 9. `ok 9 - timer no lost tick` — 1.000 tick liên tiếp, `late = missed = 0` (không mất tick;
//!    "trễ > 1 tick" là định nghĩa mất tick của KT-47).
//! 10. `ok 10 - timer irq` — ngắt **thật** qua `stvec`: `scause = (1<<63) | 5`,
//!     `TRAP_STATS.interrupts ≥ 1.000`, 0 exception phát sinh; `sstatus.SIE` đúng (SIE=1 lúc trap,
//!     SIE=0 trong handler, `sret` trả SIE=1).
//! 11. `ok 11 - timer deadline drift` — Δ tick đo được ∈ [9,11] ms (DoD KT-47), in min/max ns.
//! 12. `ok 12 - trap nested panic` — trap **lồng** vẫn fatal (P2b); boot kết thúc bằng `PANIC:
//!     nested trap …` + `DebugHalt(1)`, không treo/không resume. Runner chấp nhận đúng dòng PANIC
//!     này và **chỉ** dòng này.
//!
//! Guard page fault **thật** (`scause=13` load, `scause=15` store) nằm ở ảnh riêng
//! `vifast-kbin-ktest-guard` vì mỗi boot chỉ kết thúc được một lần bằng PANIC có chủ đích; U-mode/
//! syscall là KT-53.
#![no_std]
#![no_main]

use core::fmt::Write;
use core::sync::atomic::AtomicU64;

use vifast_plat_common::mmu::{satp_mode, satp_root, PTE_EXEC, PTE_READ, PTE_WRITE};
use vifast_plat_common::timer::{
    ticks_to_ns, TICK_DELTA_MAX_NS, TICK_DELTA_MIN_NS, TICK_HZ, TICK_NS, TICK_TICKS, TIMEBASE_HZ,
    TIMER_STATS,
};
use vifast_plat_common::trap::{
    INTERRUPT_SUPERVISOR_TIMER, SCAUSE_INTERRUPT, SSTATUS_SIE, TRAP_STATS,
};
use vifast_plat_common::{PanicHandler, Platform, PlatformWriter};
use vifast_plat_riscv64::mmu;
use vifast_plat_riscv64::timer;
use vifast_plat_riscv64::{
    probe_breakpoint, probe_ecall_sbi, probe_injected_trap, probe_nested_trap, read_trap_vector,
    trap_vector, Riscv64Virt, PLIC_BASE, UART0_BASE,
};

static PLATFORM: Riscv64Virt = Riscv64Virt;
static PANIC: PanicHandler<Riscv64Virt> = PanicHandler::new(Riscv64Virt);
/// Biến trong `.bss` để TAP ghi/đọc qua identity ↔ nửa cao ↔ physmap (hardware walk thật).
static MMU_PROBE: AtomicU64 = AtomicU64::new(0);

/// Leaf Device 4 KiB RW NX tại đúng PA `pa`.
fn device_leaf(walk: vifast_plat_common::mmu::Walk, pa: u64) -> bool {
    walk.valid
        && walk.leaf
        && walk.level == 0
        && walk.phys == pa
        && walk.raw() & PTE_EXEC == 0
        && walk.raw() & (PTE_READ | PTE_WRITE) == (PTE_READ | PTE_WRITE)
}

/// `SBI_ERR_NOT_SUPPORTED` (SBI spec: -2) — OpenSBI trả cho EID không tồn tại.
const SBI_ERR_NOT_SUPPORTED: u64 = u64::MAX - 1;

#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    PLATFORM.put_str("TAP version 13\n1..12\n");
    // KT-45: vector direct-mode trước mọi test trap.
    PLATFORM.set_trap_vector(trap_vector());
    let stvec = read_trap_vector();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ktest image: hart={} timebase_hz={} time={} tick_hz={} stvec=0x{:x} trap_vector=0x{:x} direct={}",
            PLATFORM.hart_id(),
            PLATFORM.timebase_hz(),
            PLATFORM.now_ticks(),
            TICK_HZ,
            stvec,
            trap_vector(),
            stvec & 0b11 == 0
        );
    }
    if stvec != (trap_vector() & !0b11) {
        PLATFORM.put_str("not ok 1 - ktest image: stvec không khớp trap_vector\n");
        PLATFORM.put_str("# ktest halt\n");
        PLATFORM.halt(1)
    }
    PLATFORM.put_str("ok 1 - ktest image\n");

    let mut fails = 0usize;

    // ---------------------------------------------------------------- 2) breakpoint (thật)
    {
        let before = TRAP_STATS.snapshot();
        let after = probe_breakpoint();
        let st = TRAP_STATS.snapshot();
        let ok = st.traps == before.traps + 1
            && st.exceptions == before.exceptions + 1
            && st.recovered == before.recovered + 1
            && st.last_scause == 3
            && st.last_code == 3
            && st.last_sepc + 4 == after as u64;
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# breakpoint: scause=0x{:x} kind=0x{:x} sepc=0x{:x} after=0x{:x} \
                 traps+{} recovered+{}",
                st.last_scause,
                st.last_code,
                st.last_sepc,
                after,
                st.traps - before.traps,
                st.recovered - before.recovered
            );
        }
        if ok {
            PLATFORM.put_str("ok 2 - trap breakpoint\n");
        } else {
            PLATFORM.put_str("not ok 2 - trap breakpoint: scause/sepc không đúng\n");
            fails += 1;
        }
    }

    // ---------------------------------------------------------------- 3) ecall-S qua SBI
    {
        let before = TRAP_STATS.snapshot();
        let (ret, ecall_addr, after) = probe_ecall_sbi();
        let st = TRAP_STATS.snapshot();
        let ok =
            ret == SBI_ERR_NOT_SUPPORTED && after == ecall_addr + 4 && st.traps == before.traps; // KHÔNG vào stvec S-mode (D2-73)
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# ecall sbi: a0=0x{:x} (NOT_SUPPORTED=0x{:x}) ecall=0x{:x} after=0x{:x} \
                 stvec-traps+{})",
                ret,
                SBI_ERR_NOT_SUPPORTED,
                ecall_addr,
                after,
                st.traps - before.traps
            );
        }
        if ok {
            PLATFORM.put_str("ok 3 - trap ecall sbi\n");
        } else {
            PLATFORM.put_str("not ok 3 - trap ecall sbi: SBI/sepc không đúng\n");
            fails += 1;
        }
    }

    // ---------------------------------------------------------------- 4) illegal (TIÊM)
    {
        let before = TRAP_STATS.snapshot();
        let after = probe_injected_trap(2, 0xffff_ffff);
        let st = TRAP_STATS.snapshot();
        let ok = st.traps == before.traps + 1
            && st.exceptions == before.exceptions + 1
            && st.recovered == before.recovered + 1
            && st.last_scause == 2
            && st.last_code == 2
            && st.last_sepc + 4 == after as u64;
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# illegal inject (KHÔNG phải trap phần cứng — MEDELEG bit 2 = 0, D2-73): \
                 scause=0x{:x} kind=0x{:x} sepc=0x{:x} after=0x{:x}",
                st.last_scause, st.last_code, st.last_sepc, after
            );
        }
        if ok {
            PLATFORM.put_str("ok 4 - trap illegal inject\n");
        } else {
            PLATFORM.put_str("not ok 4 - trap illegal inject: decode/resume không đúng\n");
            fails += 1;
        }
    }

    // ------------------------------------------------- 5) MMU Sv39 thật (KT-46)
    let mmu = mmu::init();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# mmu: satp=0x{:x} mode={} root=0x{:x} tables={} used={} arena=0x{:x}-0x{:x} kernel_end=0x{:x}",
            mmu.satp,
            satp_mode(mmu.satp),
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
    }

    // TAP 5: satp mode 8 + đọc lại + root từ arena + walk UART/kernel identity/nửa cao.
    {
        let uart = mmu::walk_sw(UART0_BASE as u64);
        let kernel_low = mmu::walk_sw(mmu::KERNEL_PHYS_BASE);
        let kernel_high = mmu::walk_sw(mmu::KERNEL_HIGH_BASE);
        let ok = mmu::mmu_enabled()
            && satp_mode(mmu.satp) == 8
            && mmu::satp_now() == mmu.satp
            && mmu.root == mmu.arena_start
            && mmu::root_phys() == mmu.root
            && mmu.alias_ok
            && device_leaf(uart, UART0_BASE as u64)
            && kernel_low.valid
            && kernel_high.valid;
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# mmu on: satp=0x{:x} mode={} readback=0x{:x} root=0x{:x} arena=0x{:x} alias={} uart={} kernel_low={} kernel_high={}",
                mmu.satp,
                satp_mode(mmu.satp),
                mmu::satp_now(),
                mmu.root,
                mmu.arena_start,
                mmu.alias_ok as u8,
                uart.valid as u8,
                kernel_low.valid as u8,
                kernel_high.valid as u8
            );
        }
        if ok {
            PLATFORM.put_str("ok 5 - mmu on\n");
        } else {
            PLATFORM.put_str("not ok 5 - mmu on: satp/root/walk khong dung\n");
            fails += 1;
        }
    }

    // TAP 6: guard NONE cả 3 alias; stack/trap/arena RW NX; physmap 2 MiB NX; MMIO Device.
    {
        let (guard_start, guard_end) = mmu::guard_range();
        let (stack_start, stack_end) = mmu::stack_range();
        let (trap_start, trap_end) = mmu::trap_stack_range();
        let (arena_start, arena_end) = mmu::arena_range();
        let guard_none_low = !mmu::walk_sw(guard_start).valid;
        let guard_none_high = mmu::high_alias(guard_start)
            .map(|va| !mmu::walk_sw(va).valid)
            .unwrap_or(false);
        let guard_none_physmap = mmu::physmap_alias(guard_start)
            .map(|va| !mmu::walk_sw(va).valid)
            .unwrap_or(false);
        let stack_walk = mmu::walk_sw(stack_start);
        let trap_walk = mmu::walk_sw(trap_start);
        let stack_ok = stack_walk.valid
            && stack_walk.leaf
            && stack_walk.level == 0
            && stack_walk.raw() & PTE_EXEC == 0
            && stack_walk.raw() & (PTE_READ | PTE_WRITE) == (PTE_READ | PTE_WRITE);
        let trap_ok = trap_walk.valid
            && trap_walk.leaf
            && trap_walk.level == 0
            && trap_walk.raw() & PTE_EXEC == 0
            && trap_walk.raw() & (PTE_READ | PTE_WRITE) == (PTE_READ | PTE_WRITE);
        let stack_high = mmu::high_alias(stack_start).unwrap_or(0);
        let trap_high = mmu::high_alias(trap_start).unwrap_or(0);
        let stack_marker = 0x5117_0000_0000_0001u64;
        mmu::write_u64(stack_high, stack_marker);
        let stack_alias = mmu::read_u64(stack_start) == stack_marker;
        let trap_marker = 0x5117_0000_0000_0002u64;
        mmu::write_u64(trap_high, trap_marker);
        let trap_alias = mmu::read_u64(trap_start) == trap_marker;
        let arena_probe = arena_start + mmu.used as u64;
        let arena_ok = arena_probe + 8 <= arena_end
            && mmu::physmap_alias(arena_probe)
                .map(|va| {
                    mmu::write_u64(va, stack_marker);
                    mmu::read_u64(arena_probe) == stack_marker
                })
                .unwrap_or(false);
        let probe_low = core::ptr::addr_of!(MMU_PROBE) as u64;
        let probe_high = mmu::high_alias(probe_low).unwrap_or(0);
        let probe_pm = mmu::physmap_alias(probe_low).unwrap_or(0);
        let alias_marker = 0x5117_0000_0000_0003u64;
        mmu::write_u64(probe_high, alias_marker);
        let alias_high = mmu::read_u64(probe_low) == alias_marker;
        mmu::write_u64(probe_low, alias_marker ^ 0xffff);
        let alias_low = mmu::read_u64(probe_high) == (alias_marker ^ 0xffff)
            && mmu::read_u64(probe_pm) == (alias_marker ^ 0xffff);
        let physmap_ram = mmu::walk_sw(mmu::PHYSMAP_BASE + mmu::RAM_PHYS_BASE);
        let physmap_ok = physmap_ram.valid
            && physmap_ram.leaf
            && physmap_ram.level == 1
            && physmap_ram.raw() & PTE_EXEC == 0
            && physmap_ram.raw() & PTE_WRITE != 0;
        let uart = mmu::walk_sw(UART0_BASE as u64);
        let plic = mmu::walk_sw(PLIC_BASE as u64);
        // Image nửa cao phải là **leaf 2 MiB** (level 1, X=1) — yêu cầu "kernel nửa cao 2 MiB";
        // vùng NOLOAD phía sau mới là 4 KiB (guard NONE, stack/trap/arena RW NX).
        let kernel_low_walk = mmu::walk_sw(mmu::KERNEL_PHYS_BASE);
        let kernel_high_walk = mmu::walk_sw(mmu::KERNEL_HIGH_BASE);
        let kernel_high_2m = kernel_low_walk.valid
            && kernel_low_walk.level == 1
            && kernel_high_walk.valid
            && kernel_high_walk.level == 1
            && kernel_high_walk.raw() & PTE_EXEC != 0;
        let ok = guard_none_low
            && guard_none_high
            && guard_none_physmap
            && stack_ok
            && trap_ok
            && stack_alias
            && trap_alias
            && arena_ok
            && alias_high
            && alias_low
            && physmap_ok
            && device_leaf(uart, UART0_BASE as u64)
            && device_leaf(plic, PLIC_BASE as u64)
            && kernel_high_2m;
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# mmu layout: guard=0x{:x}-0x{:x} none_low={} none_high={} none_physmap={} stack=0x{:x}-0x{:x} rw_nx={} trap=0x{:x}-0x{:x} rw_nx={} arena=0x{:x}-0x{:x} arena_alias={} physmap_2m_nx={} uart_dev={} plic_dev={} kernel_high_2m={}",
                guard_start,
                guard_end,
                guard_none_low as u8,
                guard_none_high as u8,
                guard_none_physmap as u8,
                stack_start,
                stack_end,
                stack_ok as u8,
                trap_start,
                trap_end,
                trap_ok as u8,
                arena_start,
                arena_end,
                arena_ok as u8,
                physmap_ok as u8,
                device_leaf(uart, UART0_BASE as u64) as u8,
                device_leaf(plic, PLIC_BASE as u64) as u8,
                kernel_high_2m as u8
            );
        }
        if ok {
            PLATFORM.put_str("ok 6 - map layout\n");
        } else {
            PLATFORM.put_str("not ok 6 - map layout: guard/physmap/mmio/stack khong dung\n");
            fails += 1;
        }
    }

    // TAP 7: PTE đọc lại sau `satp` bằng PTE trước; walk phần mềm = phần cứng (alias).
    {
        let samples = [
            mmu::KERNEL_PHYS_BASE,
            mmu::KERNEL_HIGH_BASE,
            mmu::PHYSMAP_BASE + mmu::RAM_PHYS_BASE,
            UART0_BASE as u64,
            PLIC_BASE as u64,
            mmu.stack_start,
            mmu.trap_stack_start,
            mmu.arena_start,
        ];
        let pre_post = mmu.pre_ptes == mmu.post_ptes;
        let mut software = 0usize;
        for (index, va) in samples.iter().enumerate() {
            if mmu::walk_sw(*va).raw() == mmu.post_ptes[index] {
                software += 1;
            }
        }
        let probe_low = core::ptr::addr_of!(MMU_PROBE) as u64;
        let probe_high = mmu::high_alias(probe_low).unwrap_or(0);
        let probe_pm = mmu::physmap_alias(probe_low).unwrap_or(0);
        let marker = 0x5117_0000_0000_0004u64;
        mmu::write_u64(probe_pm, marker);
        let hardware_a = mmu::read_u64(probe_low) == marker && mmu::read_u64(probe_high) == marker;
        mmu::write_u64(probe_high, marker ^ 0x5a);
        let hardware_b = mmu::read_u64(probe_low) == (marker ^ 0x5a)
            && mmu::read_u64(probe_pm) == (marker ^ 0x5a);
        let ok = pre_post
            && software == samples.len()
            && hardware_a
            && hardware_b
            && mmu.alias_ok
            && satp_mode(mmu.satp) == 8
            && satp_root(mmu.satp) == mmu.root;
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# walk khop: pre=post={} software={}/{} hardware_a={} hardware_b={} alias={} satp_root=0x{:x}",
                pre_post as u8,
                software,
                samples.len(),
                hardware_a as u8,
                hardware_b as u8,
                mmu.alias_ok as u8,
                satp_root(mmu.satp)
            );
        }
        if ok {
            PLATFORM.put_str("ok 7 - walk khop\n");
        } else {
            PLATFORM.put_str("not ok 7 - walk khop: PTE/alias khong khop\n");
            fails += 1;
        }
    }

    // -------------------------------------------------- 8..11) Timer SBI (KT-47)
    // SBI TIME (EID 0x54494D45) + `sie.STIE`: deadline tuyệt đối mỗi hart, tick 100 Hz trên
    // timebase 10 MHz (D2-72 Q6). `timer::init` arm deadline đầu + bật SIE **sau** khi deadline
    // đã nạp (P2a: trap entry/handler không ghi `sstatus`; `sret` khôi phục SIE từ SPIE).
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
    let exceptions_before = TRAP_STATS.snapshot().exceptions;

    // TAP 8: 100 tick, mỗi tick `rdtime` tăng (monotonic) — chờ tối đa 3 s.
    let ok100 = wait_ticks(100, 3 * TIMEBASE_HZ);
    let snap100 = TIMER_STATS.snapshot();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# timer ticks: ticks={} min_delta_ticks={} max_delta_ticks={} non_monotonic={} late={} missed={} max_late_ticks={} first={} last={} deadline={}",
            snap100.ticks,
            snap100.min_delta,
            snap100.max_delta,
            snap100.non_monotonic,
            snap100.late,
            snap100.missed,
            snap100.max_late,
            snap100.first,
            snap100.last,
            snap100.deadline
        );
    }
    let ok8 = ok100
        && snap100.ticks >= 100
        && snap100.non_monotonic == 0
        && PLATFORM.now_ticks() > snap100.last;
    if ok8 {
        PLATFORM.put_str("ok 8 - timer 100 ticks monotonic\n");
    } else {
        PLATFORM
            .put_str("not ok 8 - timer 100 ticks monotonic: khong du 100 tick/khong don dieu\n");
        fails += 1;
    }

    // TAP 9: 1.000 tick liên tiếp, KHÔNG mất tick (`late = missed = 0`; "trễ > 1 tick" = mất
    // tick). Chờ tối đa thêm 12 s (1.000 tick = 10 s ở 100 Hz).
    let ok1000 = ok100 && wait_ticks(1000, 12 * TIMEBASE_HZ);
    let snap = TIMER_STATS.snapshot();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# timer ticks: ticks={} min_delta_ticks={} max_delta_ticks={} non_monotonic={} late={} missed={} max_late_ticks={} first={} last={} deadline={}",
            snap.ticks,
            snap.min_delta,
            snap.max_delta,
            snap.non_monotonic,
            snap.late,
            snap.missed,
            snap.max_late,
            snap.first,
            snap.last,
            snap.deadline
        );
    }
    let ok9 = ok1000
        && snap.ticks >= 1000
        && snap.late == 0
        && snap.missed == 0
        && snap.non_monotonic == 0;
    if ok9 {
        PLATFORM.put_str("ok 9 - timer no lost tick\n");
    } else {
        PLATFORM.put_str("not ok 9 - timer no lost tick: co tick tre/mat tick\n");
        fails += 1;
    }

    // TAP 10: ngắt **thật** qua `stvec` — `scause = (1<<63) | 5`, đếm trong TRAP_STATS; không
    // phát sinh exception nào trong lúc timer chạy; `sstatus.SIE` đúng (trap SIE=1, handler SIE=0,
    // `sret` trả SIE=1).
    let traps = TRAP_STATS.snapshot();
    let sie_stie_after = timer::read_sie() & timer::SIE_STIE != 0;
    let sret_sie = timer::read_sstatus() & SSTATUS_SIE != 0;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# timer irq: interrupts={} scause=0x{:x} code={} exceptions={} exceptions_before={} sie_stie={}",
            traps.interrupts,
            traps.last_scause,
            traps.last_code,
            traps.exceptions,
            exceptions_before,
            sie_stie_after as u8
        );
        let _ = writeln!(
            w,
            "# timer sie: trap_spie_on={} handler_sie_off={} ticks={} sie_stie=1 sret_sie={}",
            snap.trap_spie_on, snap.handler_sie_off, snap.ticks, sret_sie as u8
        );
    }
    let ok10 = traps.interrupts >= 1000
        && traps.last_scause == (SCAUSE_INTERRUPT | INTERRUPT_SUPERVISOR_TIMER)
        && traps.last_code == INTERRUPT_SUPERVISOR_TIMER
        && traps.exceptions == exceptions_before
        && sie_stie_after
        && sret_sie
        && snap.trap_spie_on == snap.ticks
        && snap.handler_sie_off == snap.ticks;
    if ok10 {
        PLATFORM.put_str("ok 10 - timer irq\n");
    } else {
        PLATFORM.put_str("not ok 10 - timer irq: scause/SIE/exception khong dung\n");
        fails += 1;
    }

    // TAP 11: drift Δ tick ∈ [9,11] ms (DoD KT-47) — min/max delta giữa hai tick liên tiếp.
    let min_ns = ticks_to_ns(snap.min_delta);
    let max_ns = ticks_to_ns(snap.max_delta);
    let drift_in_range = min_ns >= TICK_DELTA_MIN_NS && max_ns <= TICK_DELTA_MAX_NS;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# timer drift: min_ns={} max_ns={} min_us={} max_us={} tick_ns={} min_bound_ns={} max_bound_ns={} in_range={}",
            min_ns,
            max_ns,
            min_ns / 1000,
            max_ns / 1000,
            TICK_NS,
            TICK_DELTA_MIN_NS,
            TICK_DELTA_MAX_NS,
            drift_in_range as u8
        );
    }
    let ok11 = ok9 && drift_in_range && snap.late == 0;
    if ok11 {
        PLATFORM.put_str("ok 11 - timer deadline drift\n");
    } else {
        PLATFORM.put_str("not ok 11 - timer deadline drift: delta ngoai [9,11] ms\n");
        fails += 1;
    }

    // Dừng timer trước TAP trap lồng: nạp deadline xa (dập ngắt đang chờ) rồi tắt SIE/STIE.
    // `trap_entry`/handler không ghi `sstatus`; chỉ `disable_timer` tắt cổng (đảo thứ tự enable).
    PLATFORM.set_timer_deadline(u64::MAX);
    PLATFORM.disable_timer();
    let disabled_stie = timer::read_sie() & timer::SIE_STIE == 0;
    let disabled_sie = timer::read_sstatus() & SSTATUS_SIE == 0;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# timer disable: sie_stie={} sstatus_sie={}",
            (!disabled_stie) as u8,
            (!disabled_sie) as u8
        );
    }
    if !(disabled_stie && disabled_sie) {
        fails += 1;
    }

    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ktest: fails={} traps total={} recovered={} fatal={} interrupts={}",
            fails,
            TRAP_STATS.snapshot().traps,
            TRAP_STATS.snapshot().recovered,
            TRAP_STATS.snapshot().fatal,
            TRAP_STATS.snapshot().interrupts
        );
    }
    if fails > 0 {
        PLATFORM.put_str("# ktest halt\n");
        PLATFORM.halt(1)
    }

    // ------------------------------------------------ 12) trap LỒNG (fatal, có chủ đích)
    // In `ok 12` TRƯỚC khi kích hoạt: boot này kết thúc bằng `PANIC: nested trap …` + SBI SRST,
    // không có `# ktest halt`. ktest.sh đòi đúng dòng PANIC này và **không** có PANIC nào khác.
    PLATFORM.put_str("ok 12 - trap nested panic\n");
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# nested trap: inject sp trong trap stack + jr trap_entry; kỳ vọng PANIC + halt (P2b)"
        );
    }
    probe_nested_trap()
}

/// Chờ tới khi bộ đếm timer đạt `target` tick — `false` nếu quá `timeout_ticks` (timebase).
///
/// Vòng chờ đọc `rdtime` (asm, không bị hoist) và bộ đếm atomic; ngắt timer chen vào giữa các
/// vòng lặp bình thường vì `sstatus.SIE = 1` sau `sret`.
fn wait_ticks(target: u64, timeout_ticks: u64) -> bool {
    let start = PLATFORM.now_ticks();
    loop {
        if TIMER_STATS.snapshot().ticks >= target {
            return true;
        }
        if PLATFORM.now_ticks().wrapping_sub(start) > timeout_ticks {
            return false;
        }
        // Busy-spin (không `wfi`): dưới `-icount shift=6` thời gian ảo tiến theo lệnh đã chạy,
        // nên vCPU phải chạy để deadline tới hạn đúng hạn; `wfi` làm thời gian ảo đứng ⇒ tick trễ.
        core::hint::spin_loop();
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    PANIC.handle(info)
}
