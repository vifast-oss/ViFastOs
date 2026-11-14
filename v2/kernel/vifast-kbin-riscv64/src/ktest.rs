// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ảnh **test** riscv64 cho K3 (KT-44, KT-45, KT-46) — bin riêng, feature `ktest`, nạp qua
//! OpenSBI. D2-08: ảnh test **không** nằm trong ảnh sản phẩm.
//!
//! KT-48 mở rộng ảnh này lên **15 TAP**:
//!
//! 1. `ok 1 - ktest image` — marker tách ảnh của KT-44.
//! 2. `ok 2 - trap breakpoint` — **trap phần cứng thật** (`ebreak` → `scause=3`, resume đúng
//!    `sepc`) theo D2-73.
//! 3. `ok 3 - trap ecall sbi` — `ecall` S-mode do M-mode/OpenSBI xử lý, resume đúng `sepc`.
//! 4. `ok 4 - trap illegal inject` — `illegal` (scause 2) **tiêm**, ghi rõ "inject" (D2-73).
//! 5. `ok 5 - mmu on` — `satp` mode 8, root từ arena RAM, đọc lại `satp`, walk phần mềm khớp
//!    phần cứng (UART/kernel identity + nửa cao), alias ghi/đọc.
//! 6. `ok 6 - map layout` — guard NONE ở cả 3 alias; `.stack`/`.trap_stack`/arena RW NX; physmap
//!    2 MiB **NX**; UART **4 KiB** + PLIC **2×2 MiB** Device RW NX; kernel nửa cao X.
//! 7. `ok 7 - walk khop` — PTE đọc lại **sau** khi bật `satp` bằng PTE trước; walk phần mềm trả
//!    đúng PTE; alias identity ↔ nửa cao ↔ physmap ghi/đọc khớp (hardware walk thật).
//! 8. `ok 8 - plic window` — KT-48: hai cửa sổ PLIC 2 MiB Device RW NX đã map; `ndev` **đọc từ
//!    register** (probe priority, không giả định 95) phủ IRQ 10; context `2*hart+1`; threshold 0;
//!    priority IRQ 10 = 1; enable đúng context S-mode và **không** context M-mode; claim rỗng.
//! 9. `ok 9 - uart irq` — UART `IER.TX-empty` + `sie.SEIE`: IRQ **thật** qua `stvec`
//!    (`scause=(1<<63)|9`), `uart_irqs ≥ 1`, claim/complete **cân bằng**, đúng line 10 (IIR =
//!    TX-empty), handler tự tắt `IER` + enable sau [`UART_IRQ_LIMIT`] ngắt (không ngập).
//! 10. `ok 10 - plic no fake irq` — **0 IRQ giả**: `spurious = unexpected = 0`, claim sau khi tắt
//!     trả 0, số trap external = số claim, và cổng external tắt **im lặng** (không trap mới).
//! 11. `ok 11 - timer 100 ticks monotonic` — SBI TIME + `sie.STIE`: 100 ngắt timer **thật** (chạy
//!     **sau** PLIC ⇒ hồi quy timer trong cùng boot), mỗi tick `rdtime` tăng.
//! 12. `ok 12 - timer no lost tick` — 1.000 tick liên tiếp, `late = missed = 0` (KT-47).
//! 13. `ok 13 - timer irq` — ngắt **thật** qua `stvec`: `scause = (1<<63) | 5`,
//!     `TRAP_STATS.interrupts ≥ 1.000`, 0 exception phát sinh; `sstatus.SIE` đúng.
//! 14. `ok 14 - timer deadline drift` — Δ tick đo được ∈ [9,11] ms (DoD KT-47), in min/max ns.
//! 15. `ok 15 - trap nested panic` — trap **lồng** vẫn fatal (P2b); boot kết thúc bằng `PANIC:
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
use vifast_plat_common::plic::{PLIC_STATS, UART_IRQ_LIMIT};
use vifast_plat_common::timer::{
    ticks_to_ns, TICK_DELTA_MAX_NS, TICK_DELTA_MIN_NS, TICK_HZ, TICK_NS, TICK_TICKS, TIMEBASE_HZ,
    TIMER_STATS,
};
use vifast_plat_common::trap::{
    INTERRUPT_SUPERVISOR_EXTERNAL, INTERRUPT_SUPERVISOR_TIMER, SCAUSE_INTERRUPT, SSTATUS_SIE,
    TRAP_STATS,
};
use vifast_plat_common::{PanicHandler, Platform, PlatformWriter};
use vifast_plat_riscv64::mmu;
use vifast_plat_riscv64::plic;
use vifast_plat_riscv64::timer;
use vifast_plat_riscv64::{
    probe_breakpoint, probe_ecall_sbi, probe_injected_trap, probe_nested_trap, read_trap_vector,
    trap_vector, Riscv64Virt, UART0_BASE, UART0_IRQ,
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

/// Leaf Device **2 MiB** RW NX tại đúng PA `pa` — cửa sổ PLIC của KT-48 (leaf level 1).
fn device_2m(walk: vifast_plat_common::mmu::Walk, pa: u64) -> bool {
    walk.valid
        && walk.leaf
        && walk.level == 1
        && walk.phys == pa
        && walk.raw() & PTE_EXEC == 0
        && walk.raw() & (PTE_READ | PTE_WRITE) == (PTE_READ | PTE_WRITE)
}

/// `SBI_ERR_NOT_SUPPORTED` (SBI spec: -2) — OpenSBI trả cho EID không tồn tại.
const SBI_ERR_NOT_SUPPORTED: u64 = u64::MAX - 1;

#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    PLATFORM.put_str("TAP version 13\n1..15\n");
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
        // KT-48: PLIC map **hai cửa sổ 2 MiB** Device RW NX (priority/enable + context), không
        // còn 4 KiB như KT-46 — mọi đăng ký PLIC đã dùng phải walk thấy leaf 2 MiB đúng PA.
        let plic_low = mmu::walk_sw(mmu::PLIC_WINDOW_LOW_BASE);
        let plic_high = mmu::walk_sw(mmu::PLIC_WINDOW_HIGH_BASE);
        let plic_low_ok = device_2m(plic_low, mmu::PLIC_WINDOW_LOW_BASE);
        let plic_high_ok = device_2m(plic_high, mmu::PLIC_WINDOW_HIGH_BASE);
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
            && plic_low_ok
            && plic_high_ok
            && kernel_high_2m;
        {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# mmu layout: guard=0x{:x}-0x{:x} none_low={} none_high={} none_physmap={} stack=0x{:x}-0x{:x} rw_nx={} trap=0x{:x}-0x{:x} rw_nx={} arena=0x{:x}-0x{:x} arena_alias={} plic_low_2m={} plic_high_2m={} physmap_2m_nx={} uart_dev={} plic_dev={} kernel_high_2m={}",
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
                plic_low_ok as u8,
                plic_high_ok as u8,
                physmap_ok as u8,
                device_leaf(uart, UART0_BASE as u64) as u8,
                (plic_low_ok && plic_high_ok) as u8,
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
            mmu::PLIC_WINDOW_LOW_BASE,
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

    // -------------------------------------------------- 8..10) PLIC + UART IRQ thật (KT-48)
    // D2-72 Q7/Q8: PLIC cổ điển `0x0c00_0000`, context S-mode `2*hart+1`, threshold 0, priority
    // UART0 (IRQ 10) = 1. `ndev` **đọc từ register** (probe priority) — không giả định 95.
    // UART0 bật `IER.TX-empty` để có ngắt **thật**; handler `claim → xử lý → complete`, đếm và
    // tắt `IER` + enable sau `UART_IRQ_LIMIT` ngắt để không ngập (K3-PLAN §4.1 KT-48).
    let traps_before_plic = TRAP_STATS.snapshot();
    let exceptions_before_plic = traps_before_plic.exceptions;
    let interrupts_before_plic = traps_before_plic.interrupts;
    let plic = plic::init();
    let plic_low = mmu::walk_sw(mmu::PLIC_WINDOW_LOW_BASE);
    let plic_high = mmu::walk_sw(mmu::PLIC_WINDOW_HIGH_BASE);
    let plic_low_2m = plic_low.valid && plic_low.leaf && plic_low.level == 1;
    let plic_high_2m = plic_high.valid && plic_high.leaf && plic_high.level == 1;
    let plic_low_nx = plic_low.raw() & PTE_EXEC == 0;
    let plic_high_nx = plic_high.raw() & PTE_EXEC == 0;
    let plic_low_dev = device_2m(plic_low, mmu::PLIC_WINDOW_LOW_BASE);
    let plic_high_dev = device_2m(plic_high, mmu::PLIC_WINDOW_HIGH_BASE);
    // Wrapper `Platform::enable_irq`/`disable_irq` (không chỉ helper nội bộ) phải chạy thật trên
    // `stvec`/PLIC: tắt → đọc lại thấy tắt; bật lại → đọc lại thấy bật (để bật cho TAP 9).
    let platform_irq_off = PLATFORM.disable_irq(UART0_IRQ) && !plic::irq_enabled(UART0_IRQ);
    let platform_irq_on = PLATFORM.enable_irq(UART0_IRQ) && plic::irq_enabled(UART0_IRQ);
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# plic: base=0x{:x} ndev={} hart={} ctx={} plat_off={} plat_on={} threshold={} priority_uart={} enable_ctx={} enable_mmode={} claim_idle={}",
            plic.base,
            plic.ndev,
            plic.hart,
            plic.context,
            platform_irq_off as u8,
            platform_irq_on as u8,
            plic.threshold,
            plic.priority_uart,
            plic.enable_ctx as u8,
            plic.enable_mmode as u8,
            plic.claim_idle
        );
        let _ = writeln!(
            w,
            "# plic map: low=0x{:x}-0x{:x} high=0x{:x}-0x{:x} low_dev={} high_dev={} low_2m={} high_2m={} low_nx={} high_nx={}",
            plic.low_window.0,
            plic.low_window.1,
            plic.high_window.0,
            plic.high_window.1,
            plic_low_dev as u8,
            plic_high_dev as u8,
            plic_low_2m as u8,
            plic_high_2m as u8,
            plic_low_nx as u8,
            plic_high_nx as u8
        );
    }

    // TAP 8: cửa sổ PLIC + cấu hình (ndev/context/threshold/priority/enable) đọc lại từ register.
    let ok_plic_window = plic.is_configured()
        && plic.ndev > UART0_IRQ as u32
        && plic.context == plic::current_context()
        && plic.enable_address == plic.base + 0x2000 + 0x80 * plic.context as u64
        && plic.context_address == plic.base + 0x20_0000 + 0x1000 * plic.context as u64 + 4
        && plic_low_dev
        && plic_high_dev
        && plic_low_2m
        && plic_high_2m
        && plic_low_nx
        && plic_high_nx
        && platform_irq_off
        && platform_irq_on
        && plic::irq_enabled(UART0_IRQ);
    if ok_plic_window {
        PLATFORM.put_str("ok 8 - plic window\n");
    } else {
        PLATFORM
            .put_str("not ok 8 - plic window: map/ndev/context/threshold/priority khong dung\n");
        fails += 1;
    }

    // TAP 9: UART TX-empty IRQ **thật** qua PLIC + `stvec`: claim/complete cân bằng, đúng line
    // 10, IIR = TX-empty, handler tự tắt IER + enable sau N.
    // Snapshot **trước** khi mở cổng: ngắt đầu tiên có thể được hardware nhận ngay giữa các
    // instruction của `enable_external` (SIE bật là trap ngay) nên đo sau khi mở cổng sẽ hụt.
    let traps_before_uart = TRAP_STATS.snapshot();
    let uart_ier_on = plic::uart_enable_tx_irq();
    let external_gate_on = plic::enable_external();
    let uart_wait_ok = wait_uart_irqs(UART_IRQ_LIMIT, 2 * TIMEBASE_HZ);
    let uart_snap = PLIC_STATS.snapshot();
    let traps_after_uart = TRAP_STATS.snapshot();
    plic::disable_external();
    let uart_ier_off = plic::uart_disable_tx_irq();
    let plic_enable_off = !plic::irq_enabled(UART0_IRQ);
    let traps_delta = traps_after_uart
        .interrupts
        .saturating_sub(traps_before_uart.interrupts);
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# uart irq: irqs={} claims={} completes={} traps_delta={} balanced={} spurious={} unexpected={} last_irq={} last_iir=0x{:x} ier_off={} plic_enable={}",
            uart_snap.uart_irqs,
            uart_snap.claims,
            uart_snap.completions,
            traps_delta,
            uart_snap.balanced() as u8,
            uart_snap.spurious,
            uart_snap.unexpected,
            uart_snap.last_irq,
            uart_snap.last_iir,
            uart_ier_off as u8,
            plic::irq_enabled(UART0_IRQ) as u8
        );
    }
    let ok_uart_irq = uart_ier_on
        && external_gate_on
        && uart_wait_ok
        && uart_snap.uart_irqs >= 1
        && uart_snap.uart_irqs == UART_IRQ_LIMIT
        && uart_snap.claims == uart_snap.completions
        && uart_snap.spurious == 0
        && uart_snap.unexpected == 0
        && uart_snap.last_irq == UART0_IRQ as u64
        && uart_snap.last_iir == 0x2
        && uart_snap.uart_ier_off == 1
        && uart_ier_off
        && plic_enable_off
        && traps_delta == uart_snap.claims
        && traps_after_uart.exceptions == traps_before_uart.exceptions
        && traps_after_uart.last_code == INTERRUPT_SUPERVISOR_EXTERNAL
        && traps_after_uart.last_scause == (SCAUSE_INTERRUPT | INTERRUPT_SUPERVISOR_EXTERNAL);
    if ok_uart_irq {
        PLATFORM.put_str("ok 9 - uart irq\n");
    } else {
        PLATFORM.put_str("not ok 9 - uart irq: irq/claim/complete/IIR/context khong dung\n");
        fails += 1;
    }

    // TAP 10: **0 IRQ giả** — spurious/line lạ = 0, claim sau khi tắt trả 0, số trap external =
    // số claim, và cổng external tắt **im lặng** (không trap mới trong ~1 ms thời gian ảo).
    let claim_idle_after = plic::claim();
    let quiet_before = PLIC_STATS.snapshot();
    let traps_quiet_before = TRAP_STATS.snapshot();
    let quiet_start = PLATFORM.now_ticks();
    while PLATFORM.now_ticks().wrapping_sub(quiet_start) < TICK_TICKS / 10 {
        core::hint::spin_loop();
    }
    let quiet_after = PLIC_STATS.snapshot();
    let traps_quiet_after = TRAP_STATS.snapshot();
    let stats_unchanged = quiet_after == quiet_before;
    let traps_unchanged = traps_quiet_after.traps == traps_quiet_before.traps;
    let external_traps = quiet_after.traps;
    let interrupts_delta = traps_quiet_after
        .interrupts
        .saturating_sub(interrupts_before_plic);
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# plic quiet: claim_idle={} quiet={} stats_unchanged={} external_traps={} interrupts_delta={} spurious={} unexpected={}",
            claim_idle_after,
            (stats_unchanged && traps_unchanged) as u8,
            stats_unchanged as u8,
            external_traps,
            interrupts_delta,
            quiet_after.spurious,
            quiet_after.unexpected
        );
    }
    let ok_plic_quiet = claim_idle_after == 0
        && stats_unchanged
        && traps_unchanged
        && quiet_after.spurious == 0
        && quiet_after.unexpected == 0
        && external_traps >= UART_IRQ_LIMIT
        && external_traps == quiet_after.claims + quiet_after.spurious
        && interrupts_delta == external_traps
        && traps_quiet_after.exceptions == exceptions_before_plic
        && !plic::external_gate_enabled();
    if ok_plic_quiet {
        PLATFORM.put_str("ok 10 - plic no fake irq\n");
    } else {
        PLATFORM.put_str("not ok 10 - plic no fake irq: spurious/line la/claim con treo\n");
        fails += 1;
    }

    // -------------------------------------------------- 11..14) Timer SBI (KT-47)
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

    // TAP 11: 100 tick, mỗi tick `rdtime` tăng (monotonic) — chờ tối đa 3 s.
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
        PLATFORM.put_str("ok 11 - timer 100 ticks monotonic\n");
    } else {
        PLATFORM
            .put_str("not ok 11 - timer 100 ticks monotonic: khong du 100 tick/khong don dieu\n");
        fails += 1;
    }

    // TAP 12: 1.000 tick liên tiếp, KHÔNG mất tick (`late = missed = 0`; "trễ > 1 tick" = mất
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
        PLATFORM.put_str("ok 12 - timer no lost tick\n");
    } else {
        PLATFORM.put_str("not ok 12 - timer no lost tick: co tick tre/mat tick\n");
        fails += 1;
    }

    // TAP 13: ngắt **thật** qua `stvec` — `scause = (1<<63) | 5`, đếm trong TRAP_STATS; không
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
        PLATFORM.put_str("ok 13 - timer irq\n");
    } else {
        PLATFORM.put_str("not ok 13 - timer irq: scause/SIE/exception khong dung\n");
        fails += 1;
    }

    // TAP 14: drift Δ tick ∈ [9,11] ms (DoD KT-47) — min/max delta giữa hai tick liên tiếp.
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
        PLATFORM.put_str("ok 14 - timer deadline drift\n");
    } else {
        PLATFORM.put_str("not ok 14 - timer deadline drift: delta ngoai [9,11] ms\n");
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
    PLATFORM.put_str("ok 15 - trap nested panic\n");
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# nested trap: inject sp trong trap stack + jr trap_entry; kỳ vọng PANIC + halt (P2b)"
        );
    }
    probe_nested_trap()
}

/// Chờ tới khi bộ đếm UART IRQ đạt `target` — `false` nếu quá `timeout_ticks` (timebase).
///
/// Busy-spin (không `wfi`): dưới `-icount` thời gian ảo tiến theo lệnh; ngắt PLIC đến giữa các
/// vòng lặp vì handler re-arm `IER` tạo level transition cho ngắt kế.
fn wait_uart_irqs(target: u64, timeout_ticks: u64) -> bool {
    let start = PLATFORM.now_ticks();
    loop {
        if PLIC_STATS.snapshot().uart_irqs >= target {
            return true;
        }
        if PLATFORM.now_ticks().wrapping_sub(start) > timeout_ticks {
            return false;
        }
        core::hint::spin_loop();
    }
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
