// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ảnh **test** riscv64 cho K3 (KT-44, KT-45, KT-46) — bin riêng, feature `ktest`, nạp qua
//! OpenSBI. D2-08: ảnh test **không** nằm trong ảnh sản phẩm.
//!
//! KT-46 mở rộng ảnh này từ 5 lên **8 TAP**:
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
//! 8. `ok 8 - trap nested panic` — trap **lồng** vẫn fatal (P2b); boot kết thúc bằng `PANIC:
//!    nested trap …` + `DebugHalt(1)`, không treo/không resume. Runner chấp nhận đúng dòng PANIC
//!    này và **chỉ** dòng này.
//!
//! Guard page fault **thật** (`scause=13` load, `scause=15` store) nằm ở ảnh riêng
//! `vifast-kbin-ktest-guard` vì mỗi boot chỉ kết thúc được một lần bằng PANIC có chủ đích; U-mode/
//! syscall là KT-53.
#![no_std]
#![no_main]

use core::fmt::Write;
use core::sync::atomic::AtomicU64;

use vifast_plat_common::mmu::{satp_mode, satp_root, PTE_EXEC, PTE_READ, PTE_WRITE};
use vifast_plat_common::trap::TRAP_STATS;
use vifast_plat_common::{PanicHandler, Platform, PlatformWriter};
use vifast_plat_riscv64::mmu;
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
    PLATFORM.put_str("TAP version 13\n1..8\n");
    // KT-45: vector direct-mode trước mọi test trap.
    PLATFORM.set_trap_vector(trap_vector());
    let stvec = read_trap_vector();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ktest image: hart={} tick_hz={} time={} stvec=0x{:x} trap_vector=0x{:x} direct={}",
            PLATFORM.hart_id(),
            PLATFORM.tick_hz(),
            PLATFORM.now(),
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

    // ------------------------------------------------- 8) trap LỒNG (fatal, có chủ đích)
    // In `ok 8` TRƯỚC khi kích hoạt: boot này kết thúc bằng `PANIC: nested trap …` + SBI SRST,
    // không có `# ktest halt`. ktest.sh đòi đúng dòng PANIC này và **không** có PANIC nào khác.
    PLATFORM.put_str("ok 8 - trap nested panic\n");
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# nested trap: inject sp trong trap stack + jr trap_entry; kỳ vọng PANIC + halt (P2b)"
        );
    }
    probe_nested_trap()
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    PANIC.handle(info)
}
