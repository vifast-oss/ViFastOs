// SPDX-License-Identifier: MIT OR Apache-2.0
//! Platform riscv64 cho QEMU `virt` — mốc **K3** (KT-44, mở rộng ở KT-45).
//!
//! Phạm vi: entry `_start` (S-mode, OpenSBI `fw_dynamic` nạp trực tiếp tại `0x8020_0000`), MMIO
//! UART 16550 (`debug_put_str`), SBI SRST (`debug_halt`), cài đặt `Platform` trên phần cứng ảo
//! (`rdtime`, `tp`, `stvec`) và **khung trap direct-mode** của KT-45:
//!
//! - `trap_entry` (asm): đổi sang **trap stack** qua `sscratch`, lưu **32 thanh ghi** + `sepc/
//!   sstatus/scause/stval` vào [`TrapFrame`], gọi [`trap_dispatch`], khôi phục rồi `sret`.
//! - [`trap_dispatch`]: decode `scause`, resume exception recoverable (cộng `sepc` đúng cỡ
//!   instruction 2/4 byte), dump + `DebugHalt(1)` cho fatal/interrupt.
//! - Các probe KT-45 (`probe_*`): TAP phần cứng cho `breakpoint` và `ecall-S` (qua SBI), trap
//!   **tiêm** cho `illegal` — D2-73: OpenSBI v1.8.1 `delegate_traps()` đặt `MEDELEG = 0xf4b509`
//!   (bit 2 illegal và bit 9 ecall-S **không** delegate), nên hai loại đó không vào `stvec`.
//!
//! Trap/timer/PLIC/MMU/SMP **không** thuộc KT-45: MMU (KT-46), timer (KT-47), PLIC (KT-48), SMP
//! (KT-49), U-mode/syscall (KT-53).
//!
//! Địa chỉ UART/PLIC/CLINT là **dữ kiện nền tảng QEMU virt** (D2-72 Q7), không phải hằng số thiết
//! kế `[TK]`; `spec/kernel.toml` **không** mở ở K3 (D2-72 Q1) nên chúng nằm ở đây và ở
//! `v2/docs/k3-riscv64.md`.
//!
//! Crate này **có `unsafe`** vì MMIO và asm không viết "safe" được; nằm trong danh sách đóng
//! `kernel/vifast-plat-*` của D2-17. Mọi phần **logic** dùng chung ở `vifast-plat-common`
//! (`#![forbid(unsafe_code)]`) — KT-45 giữ nguyên nguyên tắc này: trapframe/decode/chính sách ở
//! `plat-common`, chỉ asm/CSR ở đây.
#![no_std]
#![allow(unsafe_code)] // crate thuộc danh sách đóng của D2-17 (kernel/vifast-plat-*)

use core::arch::{asm, global_asm};
use core::fmt::Write;

use vifast_plat_common::trap::{
    decode_trap, handle_trap, report_fatal, report_guard_fault, report_interrupt, tf_reg_offset,
    Event, ExceptionKind, TrapAction, TrapFrame, GUARD_STATS, TF_SCAUSE_OFFSET, TF_SEPC_OFFSET,
    TF_SIZE, TF_SSTATUS_OFFSET, TF_STVAL_OFFSET, TRAP_STATS,
};
use vifast_plat_common::{Platform, PlatformWriter};

pub mod mmu;

/// UART0 của QEMU `virt` (16550A) — dữ kiện nền tảng, D2-72 Q7.
pub const UART0_BASE: usize = 0x1000_0000;
/// Dòng IRQ của UART0 trên PLIC của `virt` (D2-72 Q7).
pub const UART0_IRQ: u32 = 10;
/// PLIC cổ điển của `virt` (D2-72 Q7/Q8); `ndev` đọc từ register, không giả định.
pub const PLIC_BASE: usize = 0x0c00_0000;
/// CLINT/ACLINT của `virt` (`aclint=on`; D2-72 Q7).
pub const CLINT_BASE: usize = 0x0200_0000;
/// Trang test của `virt` (`test@100000`; D2-72 Q7).
pub const TEST_PAGE_BASE: usize = 0x0010_0000;
/// Timebase QEMU `virt`: 10 MHz (DTB `timebase-frequency`; D2-72 Q6).
pub const TIME_HZ: u64 = 10_000_000;

// Entry point: OpenSBI nhảy vào S-mode với `a0 = hartid`, `a1 = địa chỉ DTB` (SBI spec).
// Giữ hartid ở `tp` để `Riscv64Virt::hart_id` đọc lại mà không cần `static mut`; đặt `sp` từ
// linker script (`.stack` NOLOAD), đặt `sscratch = __trap_stack_top` để **mọi** trap đổi sang
// trap stack, rồi vào `rust_main` do crate ảnh cung cấp.
//
// `trap_entry` là **direct mode** (`stvec.MODE = 0`), 4-byte aligned. Bố cục trapframe lấy từ
// `vifast-plat-common::trap` bằng `const` operand — không hardcode offset trong asm (rủi ro
// "sai offset trapframe" của K3-PLAN §4.1), `const` assert ở `trap.rs` chặn lệch bố cục.
global_asm!(
    ".section .text.entry",
    ".globl _start",
    "_start:",
    "mv tp, a0", // hartid (a0) — OpenSBI truyền; `a1` là DTB (KT-45 trở đi mới dùng)
    "la sp, __stack_top",
    // KT-45 P4: zero `.bss` tường minh (không dựa vào "QEMU/RAM tình cờ bằng 0"). Chỉ dùng
    // t0/t1, không đụng a0 (hartid)/a1 (DTB). `.bss` luôn 8-byte aligned theo linker script.
    "la t0, __bss_start",
    "la t1, __bss_end",
    "1:",
    "bgeu t0, t1, 2f",
    "sd zero, 0(t0)",
    "addi t0, t0, 8",
    "j 1b",
    "2:",
    "la t0, __trap_stack_top", // KT-45: trap stack riêng, đổi bằng sscratch
    "csrw sscratch, t0",
    "call rust_main",
    "j .",
    // ------------------------------------------------------------------ trap entry (KT-45)
    ".balign 4",
    ".globl trap_entry",
    "trap_entry:",
    // sp = trap stack top, sscratch = sp bị ngắt. Cấp phát trapframe 288 B NGAY TRONG
    // `.trap_stack` (`addi sp, sp, -TF_SIZE`; 288 chia hết 16 nên ABI vẫn aligned) — thiếu bước
    // này thì frame ghi NGOÀI vùng stack (và ngoài `filesz` của LOAD) dù QEMU RAM phẳng che mất.
    // Lưu x5 (t0) TRƯỚC khi dùng nó.
    "csrrw sp, sscratch, sp",
    "addi sp, sp, -{tf_size}",
    "sd zero, {x0}(sp)",
    "sd x1, {x1}(sp)",
    "sd x3, {x3}(sp)",
    "sd x4, {x4}(sp)",
    "sd x5, {x5}(sp)",
    "csrr t0, sscratch",
    "sd t0, {x2}(sp)",
    "la t0, __trap_stack_top",
    "csrw sscratch, t0",
    "sd x6, {x6}(sp)",
    "sd x7, {x7}(sp)",
    "sd x8, {x8}(sp)",
    "sd x9, {x9}(sp)",
    "sd x10, {x10}(sp)",
    "sd x11, {x11}(sp)",
    "sd x12, {x12}(sp)",
    "sd x13, {x13}(sp)",
    "sd x14, {x14}(sp)",
    "sd x15, {x15}(sp)",
    "sd x16, {x16}(sp)",
    "sd x17, {x17}(sp)",
    "sd x18, {x18}(sp)",
    "sd x19, {x19}(sp)",
    "sd x20, {x20}(sp)",
    "sd x21, {x21}(sp)",
    "sd x22, {x22}(sp)",
    "sd x23, {x23}(sp)",
    "sd x24, {x24}(sp)",
    "sd x25, {x25}(sp)",
    "sd x26, {x26}(sp)",
    "sd x27, {x27}(sp)",
    "sd x28, {x28}(sp)",
    "sd x29, {x29}(sp)",
    "sd x30, {x30}(sp)",
    "sd x31, {x31}(sp)",
    // KT-45 P2b: phát hiện trap LỒNG (trap xảy ra khi đang chạy trên trap stack). Đọc sp bị ngắt
    // từ **frame đã lưu** (`{x2}(sp)`) — KHÔNG đọc `sscratch` vì nó đã được đặt lại thành
    // `__trap_stack_top` ở trên. Nếu sp đó nằm trong [__trap_stack_bottom, __trap_stack_top) thì
    // đây là trap trong handler ⇒ **fatal** (PANIC + halt), không khôi phục frame cũ. Frame lồng
    // đè đúng slot frame ngoài (`top-288`) — vô hại vì halt ngay, không resume. t0/t1 đã lưu ở
    // trên nên dùng tự do. Normal: sp bị ngắt ≤ __stack_top = __trap_stack_bottom.
    "ld t0, {x2}(sp)",
    "la t1, __trap_stack_bottom",
    "bgeu t1, t0, .Ltrap_normal", // old_sp <= bottom ⇒ trap từ boot stack, bình thường
    "la t1, __trap_stack_top",
    "bltu t0, t1, .Ltrap_nested", // bottom < old_sp < top ⇒ trap lồng trong handler
    ".Ltrap_normal:",
    "csrr t0, sepc",
    "sd t0, {sepc}(sp)",
    "csrr t0, sstatus",
    "sd t0, {sstatus}(sp)",
    "csrr t0, scause",
    "sd t0, {scause}(sp)",
    "csrr t0, stval",
    "sd t0, {stval}(sp)",
    "mv a0, sp",
    "call trap_dispatch",
    "j .Ltrap_resume",
    ".Ltrap_nested:",
    "call nested_trap_halt", // P2b: không quay lại
    "j .Ltrap_nested",
    ".Ltrap_resume:",
    // Khôi phục: CSR trước, thanh ghi sau, `sp` CUỐI CÙNG (đang dùng sp làm base).
    // P2a: **không** ghi `sstatus` — `sret` tự khôi phục SIE từ SPIE và privilege từ SPP; ghi
    // sstatus sớm có thể bật interrupt khi frame/handler chưa thoát (KT-47). KHÔNG ghi đè
    // sstatus cũng tránh làm hỏng trạng thái mà `sret` cần.
    "ld t0, {sepc}(sp)",
    "csrw sepc, t0",
    "la t0, __trap_stack_top",
    "csrw sscratch, t0",
    "ld x1, {x1}(sp)",
    "ld x3, {x3}(sp)",
    "ld x4, {x4}(sp)",
    "ld x6, {x6}(sp)",
    "ld x7, {x7}(sp)",
    "ld x8, {x8}(sp)",
    "ld x9, {x9}(sp)",
    "ld x10, {x10}(sp)",
    "ld x11, {x11}(sp)",
    "ld x12, {x12}(sp)",
    "ld x13, {x13}(sp)",
    "ld x14, {x14}(sp)",
    "ld x15, {x15}(sp)",
    "ld x16, {x16}(sp)",
    "ld x17, {x17}(sp)",
    "ld x18, {x18}(sp)",
    "ld x19, {x19}(sp)",
    "ld x20, {x20}(sp)",
    "ld x21, {x21}(sp)",
    "ld x22, {x22}(sp)",
    "ld x23, {x23}(sp)",
    "ld x24, {x24}(sp)",
    "ld x25, {x25}(sp)",
    "ld x26, {x26}(sp)",
    "ld x27, {x27}(sp)",
    "ld x28, {x28}(sp)",
    "ld x29, {x29}(sp)",
    "ld x30, {x30}(sp)",
    "ld x31, {x31}(sp)",
    "ld x5, {x5}(sp)",
    "ld sp, {x2}(sp)",
    "sret",
    x0 = const tf_reg_offset(0),
    x1 = const tf_reg_offset(1),
    x2 = const tf_reg_offset(2),
    x3 = const tf_reg_offset(3),
    x4 = const tf_reg_offset(4),
    x5 = const tf_reg_offset(5),
    x6 = const tf_reg_offset(6),
    x7 = const tf_reg_offset(7),
    x8 = const tf_reg_offset(8),
    x9 = const tf_reg_offset(9),
    x10 = const tf_reg_offset(10),
    x11 = const tf_reg_offset(11),
    x12 = const tf_reg_offset(12),
    x13 = const tf_reg_offset(13),
    x14 = const tf_reg_offset(14),
    x15 = const tf_reg_offset(15),
    x16 = const tf_reg_offset(16),
    x17 = const tf_reg_offset(17),
    x18 = const tf_reg_offset(18),
    x19 = const tf_reg_offset(19),
    x20 = const tf_reg_offset(20),
    x21 = const tf_reg_offset(21),
    x22 = const tf_reg_offset(22),
    x23 = const tf_reg_offset(23),
    x24 = const tf_reg_offset(24),
    x25 = const tf_reg_offset(25),
    x26 = const tf_reg_offset(26),
    x27 = const tf_reg_offset(27),
    x28 = const tf_reg_offset(28),
    x29 = const tf_reg_offset(29),
    x30 = const tf_reg_offset(30),
    x31 = const tf_reg_offset(31),
    sepc = const TF_SEPC_OFFSET,
    sstatus = const TF_SSTATUS_OFFSET,
    scause = const TF_SCAUSE_OFFSET,
    stval = const TF_STVAL_OFFSET,
    tf_size = const TF_SIZE,
);

extern "C" {
    fn trap_entry();
}

/// Địa chỉ vector trap (`trap_entry`) — dùng cho [`Platform::set_trap_vector`].
pub fn trap_vector() -> usize {
    trap_entry as *const () as usize
}

/// Cài vector trap direct-mode vào `stvec` (tiện cho ảnh kernel).
pub fn install_trap_vector() {
    Riscv64Virt.set_trap_vector(trap_vector());
}

/// Đọc lại `stvec` (4 byte đầu) — ktest xác nhận đã cài đúng direct-mode.
pub fn read_trap_vector() -> usize {
    let value: usize;
    // SAFETY: chỉ đọc CSR `stvec`, không tác dụng phụ.
    unsafe { asm!("csrr {}, stvec", out(reg) value, options(nomem, nostack)) };
    value
}

/// Handler C mà asm `trap_entry` gọi sau khi đã lưu [`TrapFrame`] trên trap stack.
///
/// - Exception recoverable (`breakpoint`, `illegal`, `ecall`) ⇒ cộng `sepc` đúng cỡ instruction,
///   ghi [`TRAP_STATS`], in một dòng `# trap …`, rồi asm `sret` về instruction kế.
/// - Exception fatal ⇒ ghi stats, dump `PANIC` + `scause/sepc/stval` + thanh ghi chính, halt(1).
/// - Interrupt ⇒ ghi stats, dump `PANIC: unexpected interrupt` (timer/PLIC là KT-47/48), halt(1).
#[no_mangle]
pub extern "C" fn trap_dispatch(tf: &mut TrapFrame) {
    let saved = *tf;
    match decode_trap(&saved) {
        Event::Exception(exc) => {
            // KT-46: guard page fault là page fault **thật** (scause 12/13/15, stval trong guard
            // page). Chính sách mặc định fatal dump+halt; probe test load-guard được resume một
            // lần để kiểm cả scause 13 và 15 trong một boot (xem `mmu::handle_guard_fault`).
            if mmu::is_guard_fault(&exc) {
                match mmu::handle_guard_fault(tf) {
                    mmu::GuardOutcome::Recovered => {
                        TRAP_STATS.record_exception(&saved, exc.kind, true);
                        GUARD_STATS.record(&saved, true);
                    }
                    mmu::GuardOutcome::Fatal => {
                        TRAP_STATS.record_exception(&saved, exc.kind, false);
                        GUARD_STATS.record(&saved, false);
                        report_guard_fault(&Riscv64Virt, &exc, &saved)
                    }
                }
                return;
            }
            let insn_len = if exc.kind.is_recoverable() {
                instruction_len(exc.kind, saved.sepc as usize)
            } else {
                4 // không dùng: fatal không cộng sepc; tránh đọc instruction lỗi
            };
            match handle_trap(tf, insn_len) {
                TrapAction::Resume => {
                    TRAP_STATS.record_exception(&saved, exc.kind, true);
                    // `frame` = địa chỉ trapframe trên trap stack: bằng chứng P1 (frame nằm trong
                    // `.trap_stack`, KHÔNG ngoài vùng stack) xuất hiện ngay trong log TAP.
                    let frame = tf as *const TrapFrame as usize;
                    let mut w = PlatformWriter(&Riscv64Virt);
                    let _ = writeln!(
                        w,
                        "# trap {} scause=0x{:x} sepc=0x{:x} stval=0x{:x} insn={} resume=0x{:x} frame=0x{:x}",
                        exc.name(),
                        saved.scause,
                        saved.sepc,
                        saved.stval,
                        insn_len,
                        tf.sepc,
                        frame
                    );
                }
                TrapAction::Fatal(fatal) => {
                    TRAP_STATS.record_exception(&saved, fatal.kind, false);
                    report_fatal(&Riscv64Virt, &fatal, &saved)
                }
                TrapAction::Interrupt { .. } => unreachable!("decode_trap đã trả Exception"),
            }
        }
        Event::Interrupt { code } => {
            TRAP_STATS.record_interrupt(&saved, code);
            report_interrupt(&Riscv64Virt, code, &saved)
        }
    }
}

/// Cỡ instruction tại `pc` (2 hoặc 4 byte) — chỉ gọi với exception **fetch được** instruction
/// (`breakpoint`, `illegal`), nên đọc bộ nhớ tại `sepc` an toàn.
///
/// `ecall` theo spec RISC-V luôn 4 byte (không có dạng nén), nên không cần đọc.
pub(crate) fn instruction_len(kind: ExceptionKind, pc: usize) -> usize {
    match kind {
        ExceptionKind::EnvironmentCallFromU | ExceptionKind::EnvironmentCallFromS => 4,
        _ => {
            // SAFETY: `pc` là địa chỉ instruction vừa gây exception (đã fetch được). Trước KT-46
            // PA == VA; sau KT-46 image được identity-map RWX nên vẫn đọc trực tiếp được. Chỉ
            // đọc 2 byte, không ghi.
            let half = unsafe { core::ptr::read_volatile(pc as *const u16) };
            if half & 0b11 == 0b11 {
                4
            } else {
                2
            }
        }
    }
}

/// Nền tảng QEMU `virt` riscv64 — zero-sized, dựng được trong `static`/`const`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Riscv64Virt;

impl Platform for Riscv64Virt {
    fn put_str(&self, s: &str) {
        debug_put_str(s);
    }

    fn halt(&self, code: usize) -> ! {
        debug_halt(code)
    }

    fn now(&self) -> u64 {
        read_time()
    }

    fn tick_hz(&self) -> u64 {
        TIME_HZ
    }

    fn hart_id(&self) -> usize {
        read_hart_id()
    }

    fn set_trap_vector(&self, vector: usize) {
        // Direct mode: MODE = low 2 bit = 0; địa chỉ phải 4-byte aligned. Symbol `trap_entry`
        // luôn aligned (`.balign 4`), `debug_assert` chặn cấu hình sai khi test.
        debug_assert_eq!(
            vector & 0b11,
            0,
            "stvec direct mode cần vector 4-byte aligned"
        );
        let value = vector & !0b11;
        // SAFETY: ghi CSR `stvec` của hart hiện tại; giá trị đã kiểm aligned, mode direct.
        unsafe { asm!("csrw stvec, {}", in(reg) value, options(nomem, nostack)) };
    }

    fn enable_irq(&self, _irq: u32) -> bool {
        // KT-45 **stub**: PLIC/claim/complete là KT-48 (D2-72 Q7). Trả false = chưa bật thật.
        false
    }

    fn disable_irq(&self, _irq: u32) -> bool {
        // KT-45 **stub**: PLIC là KT-48.
        false
    }
}

/// `rdtime` (Zicntr): S-mode đọc được vì OpenSBI đặt `mcounteren.TM`; tick 10 MHz.
/// Đây là **stub** của K3 — timer thật (`set_timer`, tick 100 Hz) là KT-47 (D2-72 Q6).
fn read_time() -> u64 {
    let ticks: u64;
    // SAFETY: `rdtime` chỉ đọc CSR `time`, không tác dụng phụ, không chạm bộ nhớ.
    unsafe { asm!("rdtime {}", out(reg) ticks, options(nomem, nostack)) };
    ticks
}

/// Hart id đọc từ `tp` — `_start` đã chép `a0` (hartid OpenSBI truyền) vào đó.
fn read_hart_id() -> usize {
    let id: usize;
    // SAFETY: `tp` do chính `_start` đặt từ `a0`; kernel `no_std` chưa dùng TLS/`tp`.
    unsafe { asm!("mv {}, tp", out(reg) id, options(nomem, nostack, preserves_flags)) };
    id
}

/// In một byte ra UART — ghi thẳng THR; QEMU không cần chờ LSR.
pub fn debug_put_byte(byte: u8) {
    // SAFETY: MMIO của thiết bị 16550 trên QEMU virt; ghi một byte vào THR.
    unsafe {
        core::ptr::write_volatile(UART0_BASE as *mut u8, byte);
    }
}

/// `DebugPutStr` mức platform (syscall 12 dùng lại hàm này ở K3 trở đi).
pub fn debug_put_str(s: &str) {
    for byte in s.bytes() {
        debug_put_byte(byte);
    }
}

/// `DebugHalt` mức platform (syscall 13). Thoát QEMU qua SBI SRST:
/// `EID=0x53525354`, `FID=0` (system_reset), `a0=0` (shutdown), `a1=0` (no reason).
///
/// **Không** in marker: mỗi ảnh tự in dòng `# k3 halt` / `# ktest halt` của mình trước khi gọi,
/// để TAP sạch và runner phân biệt được ảnh nào (D2-08).
pub fn debug_halt(_code: usize) -> ! {
    // SAFETY: ecall SBI SRST theo SBI v2.0; OpenSBI đã nạp trước khi vào S-mode.
    unsafe {
        asm!(
            "ecall",
            in("a7") 0x5352_5354usize, // SRST
            in("a6") 0usize,          // FID 0 = system_reset
            in("a0") 0usize,          // reset_type = shutdown
            in("a1") 0usize,          // reset_reason = none
            options(noreturn),
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Probe KT-45 (chỉ dùng trong ảnh ktest; API an toàn để `vifast-kbin-riscv64` không cần `unsafe` —
// R5/D2-17 chỉ cho `unsafe` trong `kernel/vifast-plat-*`).
// ---------------------------------------------------------------------------------------------

/// Chạy `ebreak` và trả địa chỉ instruction **ngay sau** `ebreak` (đã resume).
///
/// TAP: `scause=3`, `TRAP_STATS.last_sepc + 4 ==` kết quả (ebreak 4 byte), và trả về bình thường
/// ⇒ chứng minh trap S-mode thật + resume đúng `sepc`.
pub fn probe_breakpoint() -> usize {
    let after: usize;
    // SAFETY: `ebreak` 4 byte do `.4byte` cưỡng bức; handler của chính kernel resume qua nó.
    // Inline asm khai báo output `after`; mọi thanh ghi khác được `trap_entry` lưu/khôi phục.
    unsafe {
        asm!(
            "2:",
            ".4byte 0x00100073", // ebreak (4 byte, không phụ thuộc C extension)
            "3:",
            "la {after}, 3b",
            after = out(reg) after,
        );
    }
    after
}

/// Gọi SBI bằng `ecall` **từ S-mode** với EID không tồn tại, trả `(a0, địa chỉ ecall, địa chỉ sau
/// ecall)`.
///
/// D2-73: `MEDELEG` bit 9 = 0 ⇒ ecall-S do **M-mode/OpenSBI** xử lý, không vào `stvec`; TAP chỉ
/// chứng minh `a0 = SBI_ERR_NOT_SUPPORTED (-2)` và resume đúng `sepc` (không treo), **không** claim
/// là trap S-mode.
pub fn probe_ecall_sbi() -> (u64, usize, usize) {
    let ret: u64;
    let ecall_addr: usize;
    let after: usize;
    // SAFETY: ecall SBI hợp lệ; OpenSBI trả NOT_SUPPORTED cho EID lạ và quay lại `sepc + 4`.
    // a0 = mã trả về; t3/t4 giữ địa chỉ (SBI bảo toàn mọi thanh ghi trừ a0/a1).
    unsafe {
        asm!(
            "la t3, 2f",
            "2:",
            "ecall",
            "3:",
            "la t4, 3b",
            in("a7") 0xdead_beefu64, // EID không tồn tại
            lateout("a0") ret,
            lateout("a1") _, // SBI spec: a0/a1 là hai thanh ghi có thể bị clobber
            out("t3") ecall_addr,
            out("t4") after,
        );
    }
    (ret, ecall_addr, after)
}

/// Trap **tiêm** (D2-73): đặt `scause`/`stval`/`sepc` bằng CSR rồi nhảy thẳng vào `trap_entry`.
///
/// - `scause=2` (`illegal`) ⇒ handler resume qua `nop` 4 byte và trả địa chỉ sau `nop`.
///   **Không phải trap phần cứng**: OpenSBI v1.8.1 không delegate illegal (`MEDELEG` bit 2 = 0).
/// - `scause` fatal (vd 13/15) ⇒ handler dump `PANIC` rồi `DebugHalt(1)` — **không quay lại**.
///
/// Mục đích: kiểm **cùng đường asm/handler** với trap thật (`breakpoint`) cho các `scause` mà
/// firmware không delegate; TAP phải ghi rõ "inject".
pub fn probe_injected_trap(scause: u64, stval: u64) -> usize {
    let after: usize;
    let vector = trap_vector();
    // SAFETY: ghi các CSR trap (`sepc/scause/stval/sstatus.SPP`) rồi nhảy vào vector trap của
    // chính kernel; handler lưu/khôi phục mọi thanh ghi nên inline asm vẫn đúng ABI.
    unsafe {
        asm!(
            "csrr t3, sstatus", // giữ sstatus cũ để khôi phục sau trap tiêm
            "la t4, 3f",
            "csrw sepc, t4",
            "csrw scause, a0",
            "csrw stval, a1",
            "li t4, 0x100", // sstatus.SPP = 1: sret quay lại S-mode
            "csrs sstatus, t4",
            "jr t5",
            "3:",
            ".4byte 0x00000013", // nop 4 byte — nguồn độ dài instruction cho handler
            "4:",
            "csrw sstatus, t3", // khôi phục nguyên sstatus trước khi tiêm
            "la t6, 4b",
            in("a0") scause,
            in("a1") stval,
            in("t5") vector,
            out("t3") _,
            out("t4") _,
            lateout("t6") after,
        );
    }
    after
}

/// Xử lý trap **lồng** (trap xảy ra khi đang chạy trên trap stack) — KT-45 coi là **fatal**.
///
/// Được `trap_entry` gọi khi phát hiện sp bị ngắt nằm trong trap stack. In `PANIC` cố định rồi
/// `DebugHalt(1)`; **không** khôi phục/không resume frame cũ. Lưu ý bố cục: `sscratch` đã được
/// đặt lại về `__trap_stack_top` trước khi phát hiện, nên frame lồng được cấp phát ở **đúng slot**
/// `top-288` và **đè** frame ngoài — vô hại vì halt ngay, nhưng không được mô tả là "nằm dưới
/// frame ngoài".
#[no_mangle]
pub extern "C" fn nested_trap_halt() -> ! {
    debug_put_str("PANIC: nested trap (KT-45: trap trong handler = fatal, khong resume)\n");
    debug_halt(1)
}

/// Trap **lồng** để test P2b: đặt sp GIẢ vào trong trap stack (`__trap_stack_top - 0x100`) rồi
/// nhảy `trap_entry` ⇒ handler phải phát hiện lồng, in `PANIC: nested trap` và halt, **không**
/// treo/không resume. Dùng cho TAP 5 của ảnh test (boot dừng có chủ đích).
#[allow(unreachable_code)] // asm noreturn ⇒ `loop {}` chỉ để thoả kiểu `!`
pub fn probe_nested_trap() -> ! {
    let vector = trap_vector();
    // SAFETY: đổi sp sang vùng trap stack, đặt CSR trap rồi nhảy vào vector của chính kernel;
    // `options(noreturn)` vì đường này luôn kết thúc bằng PANIC + SBI SRST.
    unsafe {
        asm!(
            "la t3, 3f",
            "csrw sepc, t3",
            "li t4, 3", // scause = breakpoint (nếu không bị chặn là lồng, handler sẽ resume)
            "csrw scause, t4",
            "li t4, 0",
            "csrw stval, t4",
            "li t4, 0x100", // sstatus.SPP = 1
            "csrs sstatus, t4",
            "la t5, __trap_stack_top",
            "addi sp, t5, -0x100", // sp GIẢ nằm trong trap stack
            "jr t6",
            "3:",
            ".4byte 0x00000013", // nop 4 byte
            "4:",
            "j 4b",
            in("t6") vector,
            options(noreturn),
        );
    }
    loop {
        core::hint::spin_loop();
    }
}
