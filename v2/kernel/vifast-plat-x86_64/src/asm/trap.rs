// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `asm/trap.rs` — **trap/IDT/U-mode**: stub exception + probe gây lỗi + `stub_addr`,
//! `lgdt`/`lidt`/`sgdt`/`sidt`/`ltr`/`str_reg` + `read_rsp`, và `enter_user` (`iretq`
//! xuống ring 3). Tách cơ học từ `asm.rs` (KT-401); `vifast_isr_common` dùng chung với
//! stub syscall ở `asm/syscall.rs`.

#[cfg(all(target_arch = "x86_64", target_os = "none"))]
use core::arch::asm;

// `super::*` chỉ cần cho **stub host** (`host_only`); trên target x86_64 mọi stub bị
// `cfg`-gate ra nên glob import là thừa (clippy `unused_imports`).
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
use super::*;

// ---------------------------------------------------------------------------------------------
// GDT/IDT/TSS — KT-261 (K5' bước 2)
// ---------------------------------------------------------------------------------------------

/// `lgdt [ptr]` — nạp `GDTR` từ pseudo-descriptor 10 byte tại `ptr`.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn lgdt(ptr: u64) {
    // SAFETY: `ptr` trỏ pseudo-descriptor 10 byte hợp lệ (dựng bởi `crate::gdt`).
    unsafe {
        asm!("lgdt [{}]", in(reg) ptr, options(nostack));
    }
}

/// `lidt [ptr]` — nạp `IDTR` từ pseudo-descriptor 10 byte tại `ptr`.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn lidt(ptr: u64) {
    // SAFETY: `ptr` trỏ pseudo-descriptor 10 byte hợp lệ (dựng bởi `crate::idt`).
    unsafe {
        asm!("lidt [{}]", in(reg) ptr, options(nostack));
    }
}

/// `sgdt [ptr]` — ghi `GDTR` hiện tại (10 byte) ra `ptr`.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn sgdt(ptr: u64) {
    // SAFETY: `ptr` trỏ vùng 10 byte ghi được (bộ đệm `GDTR` của `crate::gdt`).
    unsafe {
        asm!("sgdt [{}]", in(reg) ptr, options(nostack));
    }
}

/// `sidt [ptr]` — ghi `IDTR` hiện tại (10 byte) ra `ptr`.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn sidt(ptr: u64) {
    // SAFETY: `ptr` trỏ vùng 10 byte ghi được (bộ đệm `IDTR` của `crate::idt`).
    unsafe {
        asm!("sidt [{}]", in(reg) ptr, options(nostack));
    }
}

/// `ltr ax` — nạp Task Register bằng selector TSS (`0x28`).
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn ltr(selector: u16) {
    // SAFETY: `selector` trỏ descriptor TSS hợp lệ trong GDT đã nạp.
    unsafe {
        asm!("ltr ax", in("ax") selector, options(nostack, preserves_flags));
    }
}

/// Đọc `RSP` hiện tại — **KT-276**: hart phụ kiểm `sp` của chính nó nằm trong slot arena của
/// mình (đột biến "stack per-hart chồng nhau").
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn read_rsp() -> u64 {
    let rsp: u64;
    // SAFETY: chỉ đọc thanh ghi `rsp`; không đụng bộ nhớ, không đổi stack.
    unsafe {
        asm!("mov {}, rsp", out(reg) rsp, options(nomem, nostack, preserves_flags));
    }
    rsp
}

/// `str ax` — đọc Task Register (selector TSS đã nạp).
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn str_reg() -> u16 {
    let value: u16;
    // SAFETY: `str` chỉ đọc thanh ghi hệ thống ở CPL0; không đụng bộ nhớ.
    unsafe {
        asm!("str ax", out("ax") value, options(nostack, preserves_flags));
    }
    value
}

// ---------------------------------------------------------------------------------------------
// Stub exception + probe gây lỗi — KT-261
// ---------------------------------------------------------------------------------------------
//
// # Vì sao stub nằm ở **crate này** chứ không ở ảnh
//
// Ảnh `vifast-kbin-x86_64` không được có `unsafe` (R5), mà một stub asm gọi handler Rust thì phải
// khai `extern "C"` + gọi trong `unsafe` nếu viết bằng Rust. Viết stub bằng `global_asm!` ở đây
// (crate `kernel/vifast-plat-*`, D2-17) giữ ảnh **không** `unsafe`; ảnh chỉ cung cấp
// `exc_dispatch` — một `extern "C"` **an toàn** — và gọi các hàm probe **an toàn** dưới đây.
//
// # Hợp đồng frame (khớp `crate::exc::FRAME_*`)
//
// CPU đẩy `rflags/cs/rip` (và `rsp/ss` khi đổi CPL hoặc dùng IST). Stub đẩy `vector` rồi `mã lỗi`
// (vector **có** mã lỗi thì CPU đã đẩy, stub chỉ đẩy `vector`). Stub chung đẩy 15 GPR rồi gọi
// `exc_dispatch(vec, err, rip, cs, rflags, frame_rsp) -> FrameReturn` (`rax` = `rip`, `rdx` =
// `rsp` frame tác vụ kế): `rip = 0` ⇒ halt sạch; `rdx != 0` ⇒ `mov rsp, rdx` rồi `pop`/`iretq`
// trên frame mới; còn lại ⇒ ghi `rax` vào `rip` của frame hiện tại rồi `iretq`.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
core::arch::global_asm!(
    ".section .text",
    // ---- stub KHÔNG có mã lỗi: đẩy 0 (mã lỗi giả) rồi vector.
    ".p2align 4",
    ".globl vifast_isr_unknown",
    "vifast_isr_unknown:",
    "push 0",
    "push {unknown_vec}",
    "jmp vifast_isr_common",
    ".p2align 4",
    ".globl vifast_isr_de",
    "vifast_isr_de:",
    "push 0",
    "push 0",
    "jmp vifast_isr_common",
    ".p2align 4",
    ".globl vifast_isr_db",
    "vifast_isr_db:",
    "push 0",
    "push 1",
    "jmp vifast_isr_common",
    ".p2align 4",
    ".globl vifast_isr_bp",
    "vifast_isr_bp:",
    "push 0",
    "push 3",
    "jmp vifast_isr_common",
    ".p2align 4",
    ".globl vifast_isr_ud",
    "vifast_isr_ud:",
    "push 0",
    "push 6",
    "jmp vifast_isr_common",
    // ---- stub CÓ mã lỗi: CPU đã đẩy mã lỗi, chỉ đẩy vector.
    ".p2align 4",
    ".globl vifast_isr_df",
    "vifast_isr_df:",
    "push 8",
    "jmp vifast_isr_common",
    ".p2align 4",
    ".globl vifast_isr_gp",
    "vifast_isr_gp:",
    "push 13",
    "jmp vifast_isr_common",
    ".p2align 4",
    ".globl vifast_isr_pf",
    "vifast_isr_pf:",
    "push 14",
    "jmp vifast_isr_common",
    ".p2align 4",
    ".globl vifast_isr_ts",
    "vifast_isr_ts:",
    "push 10",
    "jmp vifast_isr_common",
    ".p2align 4",
    ".globl vifast_isr_np",
    "vifast_isr_np:",
    "push 11",
    "jmp vifast_isr_common",
    ".p2align 4",
    ".globl vifast_isr_ss",
    "vifast_isr_ss:",
    "push 12",
    "jmp vifast_isr_common",
    ".p2align 4",
    ".globl vifast_isr_ac",
    "vifast_isr_ac:",
    "push 17",
    "jmp vifast_isr_common",
    ".p2align 4",
    ".globl vifast_isr_cp",
    "vifast_isr_cp:",
    "push 21",
    "jmp vifast_isr_common",
    ".p2align 4",
    ".globl vifast_isr_vc",
    "vifast_isr_vc:",
    "push 29",
    "jmp vifast_isr_common",
    ".p2align 4",
    ".globl vifast_isr_sx",
    "vifast_isr_sx:",
    "push 30",
    "jmp vifast_isr_common",
    // ---- KT-266: hai vector **ngắt** (không mã lỗi) — timer 32 và spurious 255.
    ".p2align 4",
    ".globl vifast_isr_timer",
    "vifast_isr_timer:",
    "push 0",
    "push {timer_vec}",
    "jmp vifast_isr_common",
    ".p2align 4",
    ".globl vifast_isr_spurious",
    "vifast_isr_spurious:",
    "push 0",
    "push {spurious_vec}",
    "jmp vifast_isr_common",
    // ---- **KT-278**: stub **IPI** (vector 0x30) — ngắt liên-hart do BSP gửi qua `ICR`
    // (fixed delivery). Không mã lỗi, không đổi CPL ⇒ cùng bố cục frame với timer.
    ".p2align 4",
    ".globl vifast_isr_ipi",
    "vifast_isr_ipi:",
    "push 0",
    "push {ipi_vec}",
    "jmp vifast_isr_common",
    // ---- **KT-406**: stub **COM1 RX** (vector 0x24) — IRQ 4 qua IOAPIC → LAPIC. Không mã lỗi,
    // không đổi CPL ⇒ cùng bố cục frame với timer. Handler đọc `RBR` tới `LSR.DR = 0`, rồi
    // `EOI`; không `EOI` thì LAPIC không bao giờ chuyển ngắt kế tiếp.
    ".p2align 4",
    ".globl vifast_isr_serial",
    "vifast_isr_serial:",
    "push 0",
    "push {serial_vec}",
    "jmp vifast_isr_common",
    // ---- **KT-413** (nợ `N-3`): stub **watchdog LAPIC** (vector 0x32) — nguồn đánh thức dự phòng
    // cho `hlt` khi mất IRQ0. Không mã lỗi, không đổi CPL ⇒ cùng bố cục frame với timer. Handler
    // **chỉ** tăng `WATCHDOG_WAKES` + `EOI`: không đụng `TICK_COUNT` (nếu đụng, một ngắt watchdog
    // sẽ giả một tick và phép đo `hz` mất nghĩa).
    ".p2align 4",
    ".globl vifast_isr_watchdog",
    "vifast_isr_watchdog:",
    "push 0",
    "push {watchdog_vec}",
    "jmp vifast_isr_common",
    // ---- stub chung: giữ 15 GPR, gọi handler Rust, `iretq` hoặc halt.
    ".p2align 4",
    "vifast_isr_common:",
    "push rax",
    "push rcx",
    "push rdx",
    "push rbx",
    "push rbp",
    "push rsi",
    "push rdi",
    "push r8",
    "push r9",
    "push r10",
    "push r11",
    "push r12",
    "push r13",
    "push r14",
    "push r15",
    "mov rdi, [rsp + {vec_off}]",
    "mov rsi, [rsp + {err_off}]",
    "mov rdx, [rsp + {rip_off}]",
    "mov rcx, [rsp + {cs_off}]",
    "mov r8,  [rsp + {rflags_off}]",
    // Căn stack 16 byte trước `call` (ABI SysV). `rbx` đã được đẩy ở trên nên dùng làm con trỏ
    // frame là an toàn; `x86_64-unknown-none` **tắt red zone** (đo được: hàm lá vẫn `sub rsp`),
    // nên vùng dưới `rsp` không chứa dữ liệu sống của mã bị ngắt.
    "mov rbx, rsp",
    // **KT-286**: tham số thứ 6 = con trỏ frame (rsp trỏ ô `r15`). Handler cần nó để **lưu
    // `rsp`** của tác vụ đang chạy khi preempt.
    "mov r9, rbx",
    "and rsp, -16",
    "call exc_dispatch",
    "mov rsp, rbx",
    // **KT-298**: handler trả `FrameReturn` (`#[repr(C)]` hai `u64`) trong `rax`/`rdx` —
    // `rax` = `rip` để `iretq` về, `rdx` = `rsp` frame tác vụ kế (`0` = giữ frame hiện tại).
    //
    // Vì sao **thanh ghi** chứ không phải ô `static` như KT-286: `rax`/`rdx` là **của hart đang
    // chạy**, nên hart phụ **không thể** đọc/ghi giá trị bàn giao của BSP. Ô `static` cũ dùng
    // chung mọi hart ⇒ hart phụ lấy trộm ⇒ hai hart chạy cùng ngữ cảnh trên cùng stack ⇒ `#GP`
    // storm (boot 410/600 của KT-293; bản vá kéo dài cửa sổ tái hiện 3/3 — xem
    // `v2/docs/k5-x86_64-boot.md` §15.3).
    "test rdx, rdx",
    "jnz 7f",
    "test rax, rax",
    "jz 9f",
    "mov [rsp + {rip_off}], rax",
    "jmp 8f",
    // Có `rsp` đích: frame đích đã có `rip`/`cs`/`rflags` đúng, nên **không** ghi `rax` vào
    // frame cũ — chỉ chuyển `rsp` rồi `pop`/`iretq` trên frame mới.
    "7:",
    "mov rsp, rdx",
    "8:",
    "pop r15",
    "pop r14",
    "pop r13",
    "pop r12",
    "pop r11",
    "pop r10",
    "pop r9",
    "pop r8",
    "pop rdi",
    "pop rsi",
    "pop rbp",
    "pop rbx",
    "pop rdx",
    "pop rcx",
    "pop rax",
    "add rsp, 16",
    "iretq",
    // Handler trả 0 ⇒ **halt sạch**: không quay lại chỗ lỗi, không triple-fault.
    "9:",
    "cli",
    "hlt",
    "jmp 9b",
    // ---- probe gây exception **có chủ đích** (KT-261). Mỗi probe là một hàm lá không mở stack
    // frame, nên nhãn `*_resume` nằm đúng trên đỉnh stack lúc fault: `ret` ở đó trả về thẳng chỗ
    // gọi — đó là toàn bộ cơ chế "quay lại điểm tiếp theo" của `#PF`/`#GP`.
    ".p2align 4",
    ".globl vifast_probe_int3",
    "vifast_probe_int3:",
    "int3",
    // `INT3` là **software interrupt**: CPU lưu RIP của lệnh **kế tiếp** (SDM Vol 3A Bảng 6-1),
    // khác fault (`#UD`/`#PF`/`#GP` lưu RIP của chính lệnh gây lỗi). Nhãn này là điểm resume.
    ".globl vifast_probe_int3_next",
    "vifast_probe_int3_next:",
    "ret",
    ".p2align 4",
    ".globl vifast_probe_ud2",
    "vifast_probe_ud2:",
    "ud2",
    "ret",
    ".p2align 4",
    ".globl vifast_probe_pf",
    "vifast_probe_pf:",
    "mov rax, [rdi]",
    ".globl vifast_probe_pf_resume",
    "vifast_probe_pf_resume:",
    "ret",
    ".p2align 4",
    ".globl vifast_probe_gp",
    "vifast_probe_gp:",
    "mov ax, 0x38",
    ".globl vifast_probe_gp_insn",
    "vifast_probe_gp_insn:",
    "mov ds, ax",
    ".globl vifast_probe_gp_resume",
    "vifast_probe_gp_resume:",
    "ret",
    ".p2align 4",
    // **KT-340a** — W^X: ghi **một byte** vào địa chỉ trong `rdi`. Trang chỉ-đọc ⇒ `#PF`
    // (P=1, W=1); trang RW ⇒ ghi thành công. Nhãn resume để handler `iretq` về **sau** lệnh ghi.
    ".globl vifast_probe_write8",
    "vifast_probe_write8:",
    "mov byte ptr [rdi], 0x5a",
    ".globl vifast_probe_write8_resume",
    "vifast_probe_write8_resume:",
    "ret",
    ".p2align 4",
    // **KT-418** — SMAP: **đọc** một byte từ địa chỉ trong `rdi`. Trang `US=1` với `RFLAGS.AC=0`
    // ⇒ `#PF` (P=1, W=0, U/S=1); trang supervisor ⇒ đọc thành công. Nhãn resume để handler
    // `iretq` về **sau** lệnh đọc.
    ".globl vifast_probe_read8",
    "vifast_probe_read8:",
    "mov al, byte ptr [rdi]",
    ".globl vifast_probe_read8_resume",
    "vifast_probe_read8_resume:",
    "ret",
    ".p2align 4",
    // **KT-418** — SMEP: **nhảy** tới địa chỉ trong `rdi` (thực thi). Trang `US=1` ⇒ `#PF`
    // (P=1, I/D=1, U/S=1) **trước khi** instruction ở đó chạy; trang supervisor ⇒ chạy tiếp.
    // Nhãn resume để handler `iretq` về **sau** lệnh nhảy.
    ".globl vifast_probe_exec",
    "vifast_probe_exec:",
    "jmp rdi",
    ".globl vifast_probe_exec_resume",
    "vifast_probe_exec_resume:",
    "ret",
    ".globl vifast_probe_unknown",
    "vifast_probe_unknown:",
    // **KT-278**: vector probe dời từ `0x30` sang `0x31` — `0x30` giờ là vector **IPI** có stub
    // riêng, nên `int 0x30` sẽ chạy vào handler IPI thay vì `isr_unknown` và TAP 7 mất bằng chứng.
    "int {unknown_probe_vec}",
    ".globl vifast_probe_unknown_resume",
    "vifast_probe_unknown_resume:",
    "ret",
    vec_off = const crate::exc::FRAME_VECTOR_OFFSET,
    err_off = const crate::exc::FRAME_ERROR_OFFSET,
    rip_off = const crate::exc::FRAME_RIP_OFFSET,
    cs_off = const crate::exc::FRAME_CS_OFFSET,
    rflags_off = const crate::exc::FRAME_RFLAGS_OFFSET,
    // KT-266: vector **thật** của hai stub mới + sentinel mới của `isr_unknown` (255 đã là
    // vector thật của spurious nên sentinel dời sang 254 — xem `crate::exc::VEC_UNKNOWN`).
    timer_vec = const crate::exc::VEC_TIMER as u32,
    spurious_vec = const crate::exc::VEC_SPURIOUS as u32,
    unknown_vec = const crate::exc::VEC_UNKNOWN as u32,
    // KT-278: vector IPI + vector của probe `isr_unknown` (0x31, **khác** 0x30).
    ipi_vec = const crate::exc::VEC_IPI as u32,
    // KT-406: vector COM1 RX (0x24) — IRQ 4 qua IOAPIC → LAPIC.
    serial_vec = const crate::exc::VEC_SERIAL as u32,
    // KT-413: vector watchdog LAPIC (0x32) — nguồn đánh thức dự phòng cho `hlt` (nợ N-3).
    watchdog_vec = const crate::exc::VEC_WATCHDOG as u32,
    unknown_probe_vec = const crate::exc::VEC_UNKNOWN_PROBE as u32,
    // KT-270: vector `int 0x80` + offset frame của stub syscall (khớp `crate::umode`).
);
// Khai báo stub asm — định nghĩa trong `global_asm!` ở trên.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
extern "C" {
    fn vifast_isr_unknown();
    fn vifast_isr_de();
    fn vifast_isr_db();
    fn vifast_isr_bp();
    fn vifast_isr_ud();
    fn vifast_isr_df();
    fn vifast_isr_gp();
    fn vifast_isr_pf();
    fn vifast_isr_ts();
    fn vifast_isr_np();
    fn vifast_isr_ss();
    fn vifast_isr_ac();
    fn vifast_isr_cp();
    fn vifast_isr_vc();
    fn vifast_isr_sx();
    fn vifast_isr_timer();
    fn vifast_isr_spurious();
    fn vifast_isr_syscall();
    fn vifast_isr_ipi();
    fn vifast_isr_serial();
    fn vifast_isr_watchdog();
    fn vifast_probe_int3();
    fn vifast_probe_int3_next();
    fn vifast_probe_ud2();
    fn vifast_probe_pf(addr: u64);
    fn vifast_probe_pf_resume();
    fn vifast_probe_gp();
    fn vifast_probe_gp_insn();
    fn vifast_probe_gp_resume();
    fn vifast_probe_unknown();
    fn vifast_probe_unknown_resume();
    fn vifast_probe_write8(addr: u64);
    fn vifast_probe_write8_resume();
    fn vifast_probe_read8(addr: u64);
    fn vifast_probe_read8_resume();
    fn vifast_probe_exec(addr: u64);
    fn vifast_probe_exec_resume();
}

/// Địa chỉ stub của `slot` (theo [`crate::exc::stub_slot`]); slot lạ trả `0`.
///
/// `0` là sentinel an toàn: [`crate::idt::install`] ghi offset 0 vào ô IDT, và một exception rơi
/// vào đó sẽ `#GP`/`#DF` **thấy được** thay vì im lặng.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn stub_addr(slot: usize) -> u64 {
    use crate::exc::{
        STUB_AC, STUB_BP, STUB_CP, STUB_DB, STUB_DE, STUB_DF, STUB_GP, STUB_IPI, STUB_NP, STUB_PF,
        STUB_SERIAL, STUB_SPURIOUS, STUB_SS, STUB_SX, STUB_SYSCALL, STUB_TIMER, STUB_TS, STUB_UD,
        STUB_UNKNOWN, STUB_VC, STUB_WATCHDOG,
    };
    let f: unsafe extern "C" fn() = match slot {
        STUB_UNKNOWN => vifast_isr_unknown,
        STUB_DE => vifast_isr_de,
        STUB_DB => vifast_isr_db,
        STUB_BP => vifast_isr_bp,
        STUB_UD => vifast_isr_ud,
        STUB_DF => vifast_isr_df,
        STUB_GP => vifast_isr_gp,
        STUB_PF => vifast_isr_pf,
        STUB_TS => vifast_isr_ts,
        STUB_NP => vifast_isr_np,
        STUB_SS => vifast_isr_ss,
        STUB_AC => vifast_isr_ac,
        STUB_CP => vifast_isr_cp,
        STUB_VC => vifast_isr_vc,
        STUB_SX => vifast_isr_sx,
        STUB_TIMER => vifast_isr_timer,
        STUB_SPURIOUS => vifast_isr_spurious,
        STUB_SYSCALL => vifast_isr_syscall,
        STUB_IPI => vifast_isr_ipi,
        STUB_SERIAL => vifast_isr_serial,
        STUB_WATCHDOG => vifast_isr_watchdog,
        _ => return 0,
    };
    f as usize as u64
}

/// `int3` — probe `#BP` (vector 3, không mã lỗi). Trả về **chỉ khi** handler đã `iretq`.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn probe_int3() {
    // SAFETY: `int3` ở CPL0 với IDT đã nạp; handler của ảnh quyết định resume hay halt.
    unsafe { vifast_probe_int3() }
}

/// `ud2` — probe `#UD` (vector 6, không mã lỗi).
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn probe_ud2() {
    // SAFETY: như `probe_int3`.
    unsafe { vifast_probe_ud2() }
}

/// Đọc `[addr]` — probe `#PF` (vector 14) khi `addr` chưa map.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn probe_pf(addr: u64) {
    // SAFETY: `addr` **cố ý** chưa map; handler của ảnh resume ở nhãn sau lệnh đọc.
    unsafe { vifast_probe_pf(addr) }
}

/// `mov ds, ax` với selector **ngoài** GDT — probe `#GP` (vector 13, mã lỗi = selector).
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn probe_gp() {
    // SAFETY: lệnh cố ý fault; DS **không** đổi khi #GP, handler resume ở nhãn sau lệnh.
    unsafe { vifast_probe_gp() }
}

/// `int 0x30` — probe đường `isr_unknown` (vector sentinel [`crate::exc::VEC_UNKNOWN`]).
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn probe_unknown() {
    // SAFETY: `int` tới ô IDT 0x30 (stub unknown) ở CPL0; handler resume sau lệnh `int`.
    unsafe { vifast_probe_unknown() }
}

/// Địa chỉ lệnh `int3` của [`probe_int3`] — để ảnh khẳng định `rip` bắt được **đúng** lệnh.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn probe_int3_insn() -> u64 {
    vifast_probe_int3 as *const () as usize as u64
}

/// Địa chỉ lệnh **kế tiếp** `int3` — cũng là RIP mà CPU lưu cho `#BP`.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn probe_int3_next() -> u64 {
    vifast_probe_int3_next as *const () as usize as u64
}

/// Địa chỉ lệnh `ud2` của [`probe_ud2`].
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn probe_ud2_insn() -> u64 {
    vifast_probe_ud2 as *const () as usize as u64
}

/// Địa chỉ lệnh `mov rax, [rdi]` của [`probe_pf`].
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn probe_pf_insn() -> u64 {
    vifast_probe_pf as *const () as usize as u64
}

/// Địa chỉ nhãn resume của [`probe_pf`] (ngay sau lệnh đọc).
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn probe_pf_resume() -> u64 {
    vifast_probe_pf_resume as *const () as usize as u64
}

/// Địa chỉ lệnh `mov ds, ax` của [`probe_gp`].
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn probe_gp_insn() -> u64 {
    vifast_probe_gp_insn as *const () as usize as u64
}

/// Địa chỉ nhãn resume của [`probe_gp`].
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn probe_gp_resume() -> u64 {
    vifast_probe_gp_resume as *const () as usize as u64
}

/// Địa chỉ lệnh `int 0x30` của [`probe_unknown`].
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn probe_unknown_insn() -> u64 {
    vifast_probe_unknown as *const () as usize as u64
}

/// Địa chỉ nhãn resume của [`probe_unknown`].
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn probe_unknown_resume() -> u64 {
    vifast_probe_unknown_resume as *const () as usize as u64
}

// ---------------------------------------------------------------------------------------------
// U-mode (ring 3) — KT-270 (K5' bước 4)
// ---------------------------------------------------------------------------------------------

/// `iretq` xuống **ring 3** tại `entry` với `RSP = stack_top` và `RFLAGS = rflags`. **Không trả về.**
///
/// Dựng frame `iretq` theo đúng thứ tự CPU đọc (SDM Vol 3A §5.5, Hình 5-9 "Stack Usage on
/// Transitions to Inner Privilege Levels"): `SS`, `RSP`, `RFLAGS`, `CS`, `RIP`. `CS = USER_CS`
/// (`RPL 3`) và `SS = USER_SS` (`RPL 3`) là điều kiện **bắt buộc** để hạ đặc quyền; `DS`/`ES`
/// được đặt về selector user data trước `iretq` (ring 3 không dùng phân đoạn ở long mode, nhưng
/// giữ đúng trạng thái).
///
/// `rflags` do caller dựng ([`crate::umode::USER_RFLAGS`] = `0x202`: `IF = 1`). Hàm **không** trả
/// về: đường về duy nhất là `int 0x80`/`syscall` → handler → `iretq`/`sysretq` (hoặc halt khi lỗi).
///
/// # **Hotfix audit X2** — chuẩn hoá cặp `swapgs` trước `iretq`
///
/// Đây là nơi **duy nhất** trong **ba** đường `iretq`/`sysretq` về CPL3 mà mã kernel **chủ động**
/// đi qua (`umode::run`, `coretest::start_core_phase` của nhánh `Exit`, `shellpath`). Hai cửa còn
/// lại — `iretq` của stub `int 0x80` (`asm/syscall.rs`) và `iretq` chung của `vifast_isr_common`
/// (`asm/trap.rs`) — trả về **nguyên trạng** ngắt đã vào; chúng **không** lật GS khi vào (đường
/// `int 0x80`/exception từ CPL3 không `swapgs`), nên trước `iretq` `GS.base` đã là 0 theo bất
/// biến; vì vậy **không** cần chuẩn hoá ở đó (thêm `swapgs` sẽ lật sai chiều). Điều này được
/// kiểm bằng chốt ở **cửa vào** handler: `int 0x80` đòi `GS.base == 0` (halt(6)), `syscall` đòi
/// `GS.base == gs_scratch(hart)` (halt(6)) — nếu một đường ra để GS lệch, lần vào kế tiếp chết
/// **có tên** thay vì `#PF` khó đọc.
///
/// Stub `syscall` `swapgs` **một** lần khi vào và chỉ `swapgs` trả lại ở **đuôi stub** — mà
/// `Exit` **không** quay lại đuôi stub ⇒ `GS.base` ở lại scratch của kernel, và `syscall` kế tiếp
/// `swapgs` lần nữa rồi ghi `gs:[8]` vào **VA 8** (`#PF` CPL0 trên stack user; dưới `SMAP` ⇒
/// `#DF`). Vì vậy trước `iretq`:
///
/// 1. đọc `IA32_GS_BASE` (**đo**, không đoán trạng thái);
/// 2. nếu khác [`crate::syscall_fast::USER_GS_BASE`] ⇒ đúng **một** `swapgs` (đường `int 0x80`
///    không lật GS ⇒ điều kiện này **không** swap, nếu không sẽ lật sai chiều);
/// 3. đọc lại **cả hai** MSR và **halt có tên** nếu cặp không đúng
///    ([`crate::syscall_fast::user_gs_pair_ok`]) — `halt(5)` (runner đỏ vì thiếu marker/halt
///    sạch), không để ring 3 chạy với `GS.base`/`KERNEL_GS_BASE` của kernel hay của hart khác.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn enter_user(entry: u64, stack_top: u64, rflags: u64) -> ! {
    // (1)+(2) chuẩn hoá cặp GS **trước** khi dựng frame `iretq`.
    let gs_base = crate::asm::read_msr(crate::control::MSR_IA32_GS_BASE);
    if crate::syscall_fast::needs_swap_before_user(gs_base) {
        // SAFETY: `swapgs` chỉ đảo `IA32_GS_BASE` ↔ `IA32_KERNEL_GS_BASE`; ở CPL0 luôn hợp lệ.
        unsafe { asm!("swapgs", options(nostack, preserves_flags)) };
    }
    // (3) bằng chứng + chốt: sau bước trên cặp `GS.base`/`KERNEL_GS_BASE` **phải** đúng của
    // ring 3 trên **hart này** (không chỉ `GS.base == 0`).
    let gs_base = crate::asm::read_msr(crate::control::MSR_IA32_GS_BASE);
    let kernel_gs_base = crate::asm::read_msr(crate::control::MSR_IA32_KERNEL_GS_BASE);
    let hart = crate::ipi::current_hart().unwrap_or(0);
    if !crate::syscall_fast::user_gs_pair_ok(gs_base, kernel_gs_base, hart) {
        // Không có `Console` ở tầng asm ⇒ halt mã riêng: runner thấy **không** có dòng halt sạch.
        crate::asm::halt(5)
    }
    // SAFETY: GDT/IDT/TSS đã nạp; `entry`/`stack_top` trỏ trang user đã map `US = 1`; `rflags`
    // dựng bởi `crate::umode::USER_RFLAGS`. `iretq` hạ đặc quyền và **không** trả về.
    unsafe {
        asm!(
            // Giữ `rax` (không khai `out("rax")` vì `noreturn` cấm output): `mov ax` chỉ để nạp
            // `DS`/`ES` bằng selector user data.
            "push rax",
            "mov ax, {user_ds}",
            "mov ds, ax",
            "mov es, ax",
            "pop rax",
            "push {user_ss}",
            "push {stack_top}",
            "push {rflags}",
            "push {user_cs}",
            "push {entry}",
            "iretq",
            user_ds = const crate::umode::USER_SS,
            user_ss = in(reg) crate::umode::USER_SS as u64,
            stack_top = in(reg) stack_top,
            rflags = in(reg) rflags,
            user_cs = in(reg) crate::umode::USER_CS as u64,
            entry = in(reg) entry,
            options(noreturn),
        )
    }
}

/// Stub host của [`lgdt`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn lgdt(_ptr: u64) {
    host_only("lgdt")
}

/// Stub host của [`lidt`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn lidt(_ptr: u64) {
    host_only("lidt")
}

/// Stub host của [`sgdt`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn sgdt(_ptr: u64) {
    host_only("sgdt")
}

/// Stub host của [`sidt`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn sidt(_ptr: u64) {
    host_only("sidt")
}

/// Stub host của [`ltr`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn ltr(_selector: u16) {
    host_only("ltr")
}

/// Stub host của [`str_reg`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn str_reg() -> u16 {
    host_only("str_reg")
}

/// Stub host của [`read_rsp`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn read_rsp() -> u64 {
    host_only("read_rsp")
}

/// Stub host của [`enter_user`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn enter_user(_entry: u64, _stack_top: u64, _rflags: u64) -> ! {
    host_only("enter_user")
}

/// Thử **ghi** vào `addr` bằng stub asm — **KT-340a**: trang chỉ-đọc ⇒ `#PF` (bắt bằng
/// [`crate::exc`]); trang RW ⇒ byte thành `0x5A`. Stub có nhãn resume để handler quay lại **sau**
/// lệnh ghi ([`probe_write8_resume`]).
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn probe_write8(addr: u64) {
    // SAFETY: stub asm chỉ ghi 1 byte vào `addr`; fault do quyền trang quyết định — ca kiểm arm
    // `#PF` trước khi gọi.
    unsafe { vifast_probe_write8(addr) }
}

/// RIP **sau** lệnh ghi của [`probe_write8`] — để `#PF` resume đúng chỗ.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn probe_write8_resume() -> u64 {
    // Không cần `unsafe`: lấy **địa chỉ** một nhãn trong `.text` của ảnh, không dereference.
    // Cast qua `*const ()` trước (clippy `fn_to_numeric_cast`).
    vifast_probe_write8_resume as *const () as u64
}

/// Stub host của [`probe_write8`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn probe_write8(_addr: u64) {
    host_only("probe_write8")
}

/// Stub host của [`probe_write8_resume`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn probe_write8_resume() -> u64 {
    host_only("probe_write8_resume")
}

/// **KT-418** — thử **đọc** một byte từ `addr` (stub asm) — ca âm `SMAP`: trang `US=1` với
/// `RFLAGS.AC = 0` ⇒ `#PF` (bắt bằng [`crate::exc`]).
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn probe_read8(addr: u64) {
    // SAFETY: stub asm chỉ đọc 1 byte từ `addr`; fault do quyền trang + SMAP quyết định — ca
    // kiểm arm `#PF` trước khi gọi.
    unsafe { vifast_probe_read8(addr) }
}

/// RIP **sau** lệnh đọc của [`probe_read8`].
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn probe_read8_resume() -> u64 {
    vifast_probe_read8_resume as *const () as u64
}

/// Stub host của [`probe_read8`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn probe_read8(_addr: u64) {
    host_only("probe_read8")
}

/// Stub host của [`probe_read8_resume`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn probe_read8_resume() -> u64 {
    host_only("probe_read8_resume")
}

/// **KT-418** — thử **thực thi** tại `addr` (stub asm) — ca âm `SMEP`: trang `US=1` ⇒ `#PF`
/// (P=1, I/D=1) **trước khi** instruction ở đó chạy.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn probe_exec(addr: u64) {
    // SAFETY: stub asm chỉ `jmp` tới `addr`; với SMEP bật, trang user fault **trước** khi chạy
    // mã ở đó; ca kiểm arm `#PF` trước khi gọi.
    unsafe { vifast_probe_exec(addr) }
}

/// RIP **sau** lệnh nhảy của [`probe_exec`].
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn probe_exec_resume() -> u64 {
    vifast_probe_exec_resume as *const () as u64
}

/// Stub host của [`probe_exec`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn probe_exec(_addr: u64) {
    host_only("probe_exec")
}

/// Stub host của [`probe_exec_resume`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn probe_exec_resume() -> u64 {
    host_only("probe_exec_resume")
}

/// Stub host của [`stub_addr`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn stub_addr(_slot: usize) -> u64 {
    host_only("stub_addr")
}

/// Stub host của [`probe_int3`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn probe_int3() {
    host_only("probe_int3")
}

/// Stub host của [`probe_int3_next`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn probe_int3_next() -> u64 {
    host_only("probe_int3_next")
}

/// Stub host của [`probe_ud2`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn probe_ud2() {
    host_only("probe_ud2")
}

/// Stub host của [`probe_pf`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn probe_pf(_addr: u64) {
    host_only("probe_pf")
}

/// Stub host của [`probe_gp`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn probe_gp() {
    host_only("probe_gp")
}

/// Stub host của [`probe_unknown`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn probe_unknown() {
    host_only("probe_unknown")
}

/// Stub host của [`probe_int3_insn`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn probe_int3_insn() -> u64 {
    host_only("probe_int3_insn")
}

/// Stub host của [`probe_ud2_insn`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn probe_ud2_insn() -> u64 {
    host_only("probe_ud2_insn")
}

/// Stub host của [`probe_pf_insn`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn probe_pf_insn() -> u64 {
    host_only("probe_pf_insn")
}

/// Stub host của [`probe_pf_resume`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn probe_pf_resume() -> u64 {
    host_only("probe_pf_resume")
}

/// Stub host của [`probe_gp_insn`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn probe_gp_insn() -> u64 {
    host_only("probe_gp_insn")
}

/// Stub host của [`probe_gp_resume`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn probe_gp_resume() -> u64 {
    host_only("probe_gp_resume")
}

/// Stub host của [`probe_unknown_insn`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn probe_unknown_insn() -> u64 {
    host_only("probe_unknown_insn")
}

/// Stub host của [`probe_unknown_resume`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn probe_unknown_resume() -> u64 {
    host_only("probe_unknown_resume")
}
