// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ảnh **test** riscv64 cho K3 (KT-44, KT-45) — bin riêng, feature `ktest`, nạp qua OpenSBI.
//!
//! D2-08: ảnh test **không** nằm trong ảnh sản phẩm. KT-54 dựng harness TAP đầy đủ (nhiều test,
//! `expected.txt`, smp 1/2/4); ở KT-45 ảnh này chạy **5 TAP** theo **D2-73** + P2b:
//!
//! 1. `ok 1 - ktest image` — marker tách ảnh của KT-44.
//! 2. `ok 2 - trap breakpoint` — **trap phần cứng thật**: `ebreak` → `scause=3`, handler resume
//!    đúng `sepc` (kiểm bằng `TRAP_STATS.last_sepc + 4 ==` địa chỉ sau `ebreak`).
//! 3. `ok 3 - trap ecall sbi` — `ecall` **từ S-mode**: D2-73 ghi rõ OpenSBI v1.8.1 không delegate
//!    ecall-S (`MEDELEG` bit 9 = 0), nên đây **không phải** trap S-mode; TAP chỉ chứng minh SBI trả
//!    `SBI_ERR_NOT_SUPPORTED` và resume đúng `sepc` (không treo).
//! 4. `ok 4 - trap illegal inject` — `illegal` (scause 2) **tiêm** bằng CSR + `jr trap_entry`, ghi
//!    rõ "inject": OpenSBI không delegate illegal (`MEDELEG` bit 2 = 0) nên không thể là trap phần
//!    cứng. Handler dùng **cùng đường asm** với `breakpoint` thật; decode + resume được kiểm.
//! 5. `ok 5 - trap nested panic` — trap **lồng** (tiêm sp giả trong trap stack) phải là **fatal**:
//!    `PANIC: nested trap …` + `DebugHalt(1)`, không treo/không resume. TAP 5 in `ok` **trước**
//!    khi kích hoạt vì boot kết thúc bằng PANIC có chủ đích; `ktest.sh` chấp nhận đúng dòng PANIC
//!    này (và **chỉ** dòng này) thay cho `# ktest halt`.
//!
//! Page fault thật (`scause` 13/15, guard page) chuyển **KT-46**; U-mode/syscall là KT-53.
#![no_std]
#![no_main]

use core::fmt::Write;

use vifast_plat_common::trap::TRAP_STATS;
use vifast_plat_common::{PanicHandler, Platform, PlatformWriter};
use vifast_plat_riscv64::{
    probe_breakpoint, probe_ecall_sbi, probe_injected_trap, probe_nested_trap, read_trap_vector,
    trap_vector, Riscv64Virt,
};

static PLATFORM: Riscv64Virt = Riscv64Virt;
static PANIC: PanicHandler<Riscv64Virt> = PanicHandler::new(Riscv64Virt);

/// `SBI_ERR_NOT_SUPPORTED` (SBI spec: -2) — OpenSBI trả cho EID không tồn tại.
const SBI_ERR_NOT_SUPPORTED: u64 = u64::MAX - 1;

#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    PLATFORM.put_str("TAP version 13\n1..5\n");
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

    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ktest trap: fails={} traps total={} recovered={} fatal={} interrupts={}",
            fails,
            TRAP_STATS.snapshot().traps,
            TRAP_STATS.snapshot().recovered,
            TRAP_STATS.snapshot().fatal,
            TRAP_STATS.snapshot().interrupts
        );
    }

    // ------------------------------------------------- 5) trap LỒNG (fatal, có chủ đích)
    // In `ok 5` TRƯỚC khi kích hoạt: boot này kết thúc bằng `PANIC: nested trap …` + SBI SRST,
    // không có `# ktest halt`. ktest.sh đòi đúng dòng PANIC này và **không** có PANIC nào khác.
    PLATFORM.put_str("ok 5 - trap nested panic\n");
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
