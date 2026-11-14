// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ảnh **test guard page** riscv64 (KT-46) — bin riêng, feature `ktest`, nạp qua OpenSBI.
//!
//! Vì sao ảnh riêng: mỗi boot chỉ kết thúc được **một** lần bằng PANIC có chủ đích. Ảnh
//! `vifast-kbin-ktest` đã dùng lần kết thúc đó cho trap lồng (KT-45 P2b), nên guard page-fault
//! **thật** (`scause=13` load, `scause=15` store) cần một boot riêng — vẫn trong cùng crate,
//! cùng linker, cùng cổng budget, **không** nằm trong ảnh sản phẩm (D2-08).
//!
//! TAP (3 test):
//!
//! 1. `ok 1 - guard image` — `satp` mode 8 (Sv39) + guard page **NONE** ở identity, nửa cao và
//!    physmap; alias ghi/đọc khớp (MMU thật, không model).
//! 2. `ok 2 - guard load fault` — đọc guard page ⇒ **load page fault thật** `scause=13`,
//!    `stval` trong guard, handler in `# guard fault load …` rồi resume **đúng một instruction**
//!    (chỉ probe test; chính sách mặc định vẫn fatal) — để kiểm được cả 13 và 15 trong một boot.
//! 3. `ok 3 - guard store fault` — ghi guard page ⇒ **store page fault thật** `scause=15`;
//!    handler in `PANIC: guard page fault scause=0xf …` + thanh ghi rồi `DebugHalt(1)` sạch.
//!    Không có `# ktest halt`: boot kết thúc bằng PANIC có chủ đích, runner chỉ chấp nhận đúng
//!    dòng PANIC này.
#![no_std]
#![no_main]

use core::fmt::Write;

use vifast_plat_common::trap::{GUARD_STATS, TRAP_STATS};
use vifast_plat_common::{PanicHandler, Platform, PlatformWriter};
use vifast_plat_riscv64::mmu;
use vifast_plat_riscv64::{read_trap_vector, trap_vector, Riscv64Virt};

static PLATFORM: Riscv64Virt = Riscv64Virt;
static PANIC: PanicHandler<Riscv64Virt> = PanicHandler::new(Riscv64Virt);

#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    PLATFORM.put_str("TAP version 13\n1..3\n");
    PLATFORM.set_trap_vector(trap_vector());
    let stvec = read_trap_vector();
    // Dựng bảng xong mới bật `satp` (hai bước — `mmu::init`), trap đã sống để fault lúc map được
    // dump thay vì treo.
    let report = mmu::init();
    // KT-49: ảnh guard dùng guard **của hart boot thật** (`hart_id()`), không hardcode hart 0 —
    // OpenSBI có thể chọn boot hart khác 0 (đo được ở ảnh sản phẩm `-smp 4` không icount:
    // `Domain0 Boot HART: 2`). `handle_guard_fault` chỉ resume khi `guard_owner(stval)` = hart này.
    let hart = PLATFORM.hart_id();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# guard image: hart={} stvec=0x{:x} direct={} satp=0x{:x} mode={} root=0x{:x} tables={} used={} arena=0x{:x}-0x{:x}",
            hart,
            stvec,
            (stvec & 0b11 == 0) as u8,
            report.satp,
            report.satp >> 60,
            report.root,
            report.tables,
            report.used,
            report.arena_start,
            report.arena_end
        );
        let _ = writeln!(
            w,
            "# mmu map: high=0x{:x} physmap=0x{:x} guard=0x{:x}-0x{:x} guard_none=1 alias={}",
            report.high_base,
            report.physmap_base,
            report.guard_start,
            report.guard_end,
            report.alias_ok as u8
        );
    }

    // ---------------------------------------------------------------- 1) guard NONE cả 3 alias
    let (guard_start, guard_end) = mmu::guard_range_of(hart);
    let guard_none_low = !mmu::walk_sw(guard_start).valid;
    let guard_none_high = mmu::high_alias(guard_start)
        .map(|va| !mmu::walk_sw(va).valid)
        .unwrap_or(false);
    let guard_none_physmap = mmu::physmap_alias(guard_start)
        .map(|va| !mmu::walk_sw(va).valid)
        .unwrap_or(false);
    let ok = mmu::mmu_enabled()
        && report.satp >> 60 == 8
        && report.root == report.arena_start
        && mmu::satp_now() == report.satp
        && report.alias_ok
        && mmu::guard_owner(guard_start) == Some(hart)
        && guard_end == guard_start + 0x1000
        && guard_none_low
        && guard_none_high
        && guard_none_physmap;
    if ok {
        PLATFORM.put_str("ok 1 - guard image\n");
    } else {
        PLATFORM.put_str("not ok 1 - guard image: satp/guard/alias khong dung\n");
        PLATFORM.put_str("# ktest halt\n");
        PLATFORM.halt(1)
    }

    // ---------------------------------------------------------------- 2) load fault (scause=13)
    let before = GUARD_STATS.snapshot();
    let traps_before = TRAP_STATS.snapshot();
    PLATFORM.put_str("ok 2 - guard load fault\n");
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# guard load: hart={} cham 0x{:x} (guard 0x{:x}-0x{:x}) — kỳ vọng scause=0xd, handler in + resume 1 instruction",
            hart, guard_start, guard_start, guard_end
        );
    }
    let _ = mmu::probe_guard_load(guard_start);
    let after = GUARD_STATS.snapshot();
    let traps_after = TRAP_STATS.snapshot();
    let load_ok = mmu::guard_owner(guard_start) == Some(hart)
        && after.load_faults == before.load_faults + 1
        && after.store_faults == before.store_faults
        && after.recovered == before.recovered + 1
        && after.fatal == before.fatal
        && traps_after.traps == traps_before.traps + 1
        && traps_after.last_scause == 13
        && traps_after.last_stval == guard_start
        && !mmu::guard_recovery_armed();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# guard load stats: faults+{} load+{} store+{} recovered+{} fatal+{} last_scause=0x{:x} last_stval=0x{:x} armed={}",
            after.faults - before.faults,
            after.load_faults - before.load_faults,
            after.store_faults - before.store_faults,
            after.recovered - before.recovered,
            after.fatal - before.fatal,
            traps_after.last_scause,
            traps_after.last_stval,
            mmu::guard_recovery_armed() as u8
        );
    }
    if !load_ok {
        PLATFORM.put_str("not ok 2 - guard load fault: khong phai page fault 13\n");
        PLATFORM.put_str("# ktest halt\n");
        PLATFORM.halt(1)
    }

    // ----------------------------------------------- 3) store fault (scause=15, fatal có chủ đích)
    // In `ok 3` TRƯỚC khi kích hoạt: boot kết thúc bằng `PANIC: guard page fault scause=0xf …` +
    // SBI SRST, không có `# ktest halt`. Runner đòi đúng dòng PANIC này và **chỉ** dòng này.
    PLATFORM.put_str("ok 3 - guard store fault\n");
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# guard store: hart={} ghi 0x{:x} — kỳ vọng scause=0xf + PANIC guard page fault + halt",
            hart,
            guard_start + 8
        );
    }
    mmu::probe_guard_store(guard_start + 8);
    // Không được tới đây: guard không fault nghĩa là trang bảo vệ bị map sai.
    PLATFORM.put_str("not ok 3 - guard store fault: khong fault (guard bi map)\n");
    PLATFORM.put_str("# ktest halt\n");
    PLATFORM.halt(1)
}

/// Entry hart **thứ cấp** (KT-49): ảnh guard chỉ cần symbol để link; ảnh này **không** gọi
/// `smp::init` nên không hart thứ cấp nào chạy (nếu có, vào đây rồi park — không panic).
#[no_mangle]
pub extern "C" fn rust_secondary_main(_hart: usize, opaque: usize) -> ! {
    let _ = vifast_plat_riscv64::smp::secondary_setup(opaque);
    vifast_plat_riscv64::park_forever()
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    PANIC.handle(info)
}
