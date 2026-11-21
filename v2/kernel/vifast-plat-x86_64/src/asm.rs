// SPDX-License-Identifier: MIT OR Apache-2.0
//! Asm nguyên thủy x86_64 — **mô-đun duy nhất** của crate có `unsafe` (KT-257, K5' bước 1).
//!
//! # Vì sao tách riêng
//!
//! Crate giữ `#![deny(unsafe_code)]` ở gốc; **chỉ** tệp này `#![allow(unsafe_code)]` (D2-17 cho
//! phép `unsafe` trong `kernel/vifast-plat-*`). Mọi phần dữ liệu ([`crate::serial`],
//! [`crate::boot`], [`crate::format`], [`crate::mmu`], [`crate::control`], [`crate::calling`])
//! vẫn **không** `unsafe`. Ảnh boot `vifast-kbin-x86_64` gọi các hàm **an toàn** ở đây nên bản
//! thân ảnh cũng không có `unsafe` (luật R5).
//!
//! # Host stub
//!
//! Mỗi hàm có hai bản: bản **bare-metal x86_64** (`target_arch = "x86_64"` **và**
//! `target_os = "none"`) là asm thật; bản host **panic** (không phải no-op) để `cargo test` trên
//! host vẫn biên dịch được mà không ai lỡ gọi cổng I/O đặc quyền trên máy thật.
//!
//! **Vì sao có thêm `target_os = "none"`**: máy phát triển của repo là **Apple Silicon**
//! (`aarch64-apple-darwin`), nhưng nếu ai chạy test trên macOS x86_64 thì `target_arch = "x86_64"`
//! vẫn khớp và `in`/`out` sẽ fault ở user mode. Điều kiện đúng là "x86_64 bare-metal", không phải
//! "x86_64 bất kỳ"; `cargo build --target x86_64-unknown-none` thoả `target_os = "none"`.
//!
//! # Vì sao `halt` phân nhánh theo `code`
//!
//! QEMU ACPI shutdown (`outw(0x604, 0x2000)`) **luôn** làm QEMU thoát mã 0 — không có cách truyền
//! mã lỗi qua đường đó. Vì vậy: `code == 0` ⇒ shutdown sạch (runner thấy `qg_boot` rc=0);
//! `code != 0` ⇒ **không** shutdown, chỉ `cli; hlt` ⇒ QEMU treo tới hết ngân sách ⇒ runner ĐỎ.
//! Nhờ vậy một `PANIC` không bao giờ trông giống "boot thành công".

#![allow(unsafe_code)] // D2-17: crate `kernel/vifast-plat-*`; chỉ asm mới cần.

#[cfg(all(target_arch = "x86_64", target_os = "none"))]
use crate::boot::{ACPI_PM1A_CNT, ACPI_SHUTDOWN_VALUE};
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
use crate::serial::{self, COM1_BASE, REG_LSR, TX_WAIT_LIMIT};

#[cfg(all(target_arch = "x86_64", target_os = "none"))]
use core::arch::asm;

// ---------------------------------------------------------------------------------------------
// Cổng I/O
// ---------------------------------------------------------------------------------------------

/// Ghi một byte ra cổng I/O 8-bit (`out dx, al`).
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn outb(port: u16, value: u8) {
    // SAFETY: `out` chỉ ghi cổng I/O; `nomem`/`nostack` đúng vì không đụng bộ nhớ/stack.
    unsafe {
        asm!(
            "out dx, al",
            in("dx") port,
            in("al") value,
            options(nomem, nostack, preserves_flags)
        );
    }
}

/// Ghi một word ra cổng I/O 16-bit (`out dx, ax`).
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn outw(port: u16, value: u16) {
    // SAFETY: như `outb`.
    unsafe {
        asm!(
            "out dx, ax",
            in("dx") port,
            in("ax") value,
            options(nomem, nostack, preserves_flags)
        );
    }
}

/// Đọc một byte từ cổng I/O 8-bit (`in al, dx`).
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn inb(port: u16) -> u8 {
    let value: u8;
    // SAFETY: `in` chỉ đọc cổng I/O; không đụng bộ nhớ.
    unsafe {
        asm!(
            "in al, dx",
            in("dx") port,
            out("al") value,
            options(nomem, nostack, preserves_flags)
        );
    }
    value
}

/// Ghi một dword ra cổng I/O 32-bit (`out dx, eax`) — **KT-274**: `CONFIG_ADDRESS` `0xCF8`.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn outl(port: u16, value: u32) {
    // SAFETY: `out` chỉ ghi cổng I/O; `nomem`/`nostack` đúng vì không đụng bộ nhớ/stack.
    unsafe {
        asm!(
            "out dx, eax",
            in("dx") port,
            in("eax") value,
            options(nomem, nostack, preserves_flags)
        );
    }
}

/// Đọc một dword từ cổng I/O 32-bit (`in eax, dx`) — **KT-274**: `CONFIG_DATA` `0xCFC`.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn inl(port: u16) -> u32 {
    let value: u32;
    // SAFETY: `in` chỉ đọc cổng I/O; không đụng bộ nhớ.
    unsafe {
        asm!(
            "in eax, dx",
            in("dx") port,
            out("eax") value,
            options(nomem, nostack, preserves_flags)
        );
    }
    value
}

// ---------------------------------------------------------------------------------------------
// Thanh ghi điều khiển + MSR
// ---------------------------------------------------------------------------------------------

/// Đọc `CR0`.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn read_cr0() -> u64 {
    let value: u64;
    // SAFETY: đọc thanh ghi điều khiển ở CPL0; không đụng bộ nhớ.
    unsafe {
        asm!("mov {}, cr0", out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

/// Đọc `CR3`.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn read_cr3() -> u64 {
    let value: u64;
    // SAFETY: như `read_cr0`.
    unsafe {
        asm!("mov {}, cr3", out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

/// Đọc `CR4`.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn read_cr4() -> u64 {
    let value: u64;
    // SAFETY: như `read_cr0`.
    unsafe {
        asm!("mov {}, cr4", out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

/// Đọc MSR 64-bit (`rdmsr`): `EDX:EAX` → `u64`.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn read_msr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;
    // SAFETY: `rdmsr` ở CPL0 với MSR hợp lệ; không đụng bộ nhớ.
    unsafe {
        asm!(
            "rdmsr",
            in("ecx") msr,
            out("eax") low,
            out("edx") high,
            options(nomem, nostack, preserves_flags)
        );
    }
    ((high as u64) << 32) | (low as u64)
}

/// Ghi MSR 64-bit (`wrmsr`): `u64` → `EDX:EAX`.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn write_msr(msr: u32, value: u64) {
    let low = value as u32;
    let high = (value >> 32) as u32;
    // SAFETY: `wrmsr` ở CPL0 với MSR hợp lệ; không đụng bộ nhớ.
    unsafe {
        asm!(
            "wrmsr",
            in("ecx") msr,
            in("eax") low,
            in("edx") high,
            options(nomem, nostack, preserves_flags)
        );
    }
}

/// Đọc 32-bit tại địa chỉ **vật lý** (identity map ⇒ VA == PA).
///
/// Dùng để đọc `hvm_start_info.magic` mà `%ebx` trỏ tới — bằng chứng giao thức PVH. Địa chỉ nằm
/// trong identity map 1 GiB nên không fault kể cả khi `%ebx` rác.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn read_u32(addr: u64) -> u32 {
    // SAFETY: caller bảo đảm `addr` nằm trong vùng đã map (identity map 1 GiB của ảnh boot).
    unsafe { core::ptr::read_volatile(addr as *const u32) }
}

/// Ghi 32-bit tại địa chỉ **vật lý** (identity map ⇒ VA == PA).
///
/// Dùng cho **MMIO** của LAPIC/IOAPIC (KT-266): địa chỉ nằm trong MMIO map 3..4 GiB mà ảnh dựng
/// ở `_start`; `volatile` để compiler không gộp/bỏ lần ghi.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn write_u32(addr: u64, value: u32) {
    // SAFETY: caller bảo đảm `addr` là MMIO đã map và ghi được (LAPIC/IOAPIC).
    unsafe { core::ptr::write_volatile(addr as *mut u32, value) }
}

/// Đọc 64-bit tại địa chỉ **vật lý** (identity map ⇒ VA == PA).
///
/// Dùng cho bảng stub/`GDTR`/`IDTR` — mọi địa chỉ ấy nằm trong identity map 1 GiB của ảnh.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn read_u64(addr: u64) -> u64 {
    // SAFETY: caller bảo đảm `addr` nằm trong vùng đã map (identity map 1 GiB của ảnh boot).
    unsafe { core::ptr::read_volatile(addr as *const u64) }
}

/// Đọc 16-bit tại địa chỉ **vật lý** — **KT-274**: thanh ghi cấu hình PCI (ECAM) và `used->idx`
/// của virtqueue (đọc `volatile` để compiler **không** hoist khỏi vòng poll DMA).
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn read_u16(addr: u64) -> u16 {
    // SAFETY: caller bảo đảm `addr` nằm trong vùng đã map (identity map/MMIO map của ảnh).
    unsafe { core::ptr::read_volatile(addr as *const u16) }
}

/// Ghi 8-bit tại địa chỉ **vật lý** — **KT-274**: thanh ghi `device_status`/`queue_enable` của
/// virtio-pci (MMIO).
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn write_u8(addr: u64, value: u8) {
    // SAFETY: caller bảo đảm `addr` là MMIO đã map và ghi được (BAR virtio).
    unsafe { core::ptr::write_volatile(addr as *mut u8, value) }
}

/// Ghi 16-bit tại địa chỉ **vật lý** — **KT-274**: `queue_select`/`queue_size`/cửa thông báo
/// (notify) của virtio-pci.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn write_u16(addr: u64, value: u16) {
    // SAFETY: caller bảo đảm `addr` là MMIO đã map và ghi được (BAR virtio).
    unsafe { core::ptr::write_volatile(addr as *mut u16, value) }
}

/// Ghi 64-bit tại địa chỉ **vật lý** — **KT-274**: `queue_desc`/`queue_driver`/`queue_device`
/// của virtio-pci (địa chỉ 64 bit).
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn write_u64(addr: u64, value: u64) {
    // SAFETY: caller bảo đảm `addr` là MMIO đã map và ghi được (BAR virtio).
    unsafe { core::ptr::write_volatile(addr as *mut u64, value) }
}

/// Đọc `CR2` — địa chỉ gây `#PF` (SDM Vol 3A §6.15).
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn read_cr2() -> u64 {
    let value: u64;
    // SAFETY: đọc thanh ghi điều khiển ở CPL0; không đụng bộ nhớ.
    unsafe {
        asm!("mov {}, cr2", out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

// ---------------------------------------------------------------------------------------------
// Ngắt — KT-266 (K5' bước 3)
// ---------------------------------------------------------------------------------------------

/// `sti` — bật ngắt che được (`RFLAGS.IF = 1`).
///
/// Sau lệnh này ngắt (PIT qua IOAPIC → LAPIC → vector 32) bắt đầu được CPU nhận. Ảnh gọi **một
/// lần** ngay trước khi đo tick; mọi phần còn lại của boot chạy với ngắt tắt (trạng thái từ
/// `_start`).
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn sti() {
    // SAFETY: `sti` ở CPL0; không đụng bộ nhớ/stack.
    unsafe {
        asm!("sti", options(nomem, nostack));
    }
}

/// `hlt` — dừng CPU tới **ngắt kế tiếp** (không đụng `RFLAGS.IF`).
///
/// Dùng trong vòng chờ khi ngắt đã bật: nhường BQL để QEMU xử lý timer, thay vì busy-wait giữ
/// BQL và làm ngắt bị trễ hàng trăm ms (đo được ở KT-266).
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn hlt() {
    // SAFETY: `hlt` ở CPL0; không đụng bộ nhớ/stack.
    unsafe {
        asm!("hlt", options(nomem, nostack));
    }
}

/// `cli` — tắt ngắt che được (`RFLAGS.IF = 0`).
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn cli() {
    // SAFETY: `cli` ở CPL0; không đụng bộ nhớ/stack.
    unsafe {
        asm!("cli", options(nomem, nostack));
    }
}

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
// `exc_dispatch(vec, err, rip, cs, rflags) -> u64`: trả `0` ⇒ halt, trả `rip mới` ⇒ `iretq`.
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
    // ---- KT-270: stub `int 0x80` (vector 0x80) — đường syscall từ ring 3.
    //
    // `int 0x80` từ CPL3 đẩy `SS/RSP/RFLAGS/CS/RIP` (5 qword); stub đẩy `err = 0`,
    // `vector = 0x80`, rồi **15 GPR** — cùng bố cục frame với exception chung. Handler
    // `syscall_dispatch` nhận `(nr, a0, a1, mi, mr0, mr1, cs, rip)` theo SysV (6 thanh ghi + 2
    // trên stack) và trả `SyscallRet` (`rax:rdx`); stub ghi hai giá trị vào ô `rax`/`rdx` của
    // frame rồi `iretq` — ring 3 đọc đúng `[calling.x86_64].return_error`/`return_value`.
    ".p2align 4",
    ".globl vifast_isr_syscall",
    "vifast_isr_syscall:",
    "push 0",
    "push {syscall_vec}",
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
    // `rbx` = con trỏ frame; mọi tham số đọc **từ bộ nhớ** nên việc ghi đè thanh ghi không ảnh
    // hưởng thứ tự đọc.
    "mov rbx, rsp",
    "mov rdi, [rbx + {sc_rax}]", // nr
    "mov rsi, [rbx + {sc_rdi}]", // a0
    "mov rdx, [rbx + {sc_rsi}]", // a1
    "mov rcx, [rbx + {sc_rdx}]", // mi
    "mov r8,  [rbx + {sc_r10}]", // mr0
    "mov r9,  [rbx + {sc_r8}]",  // mr1
    "mov r12, [rbx + {sc_rip}]", // rip (tham số thứ 8)
    "mov r13, [rbx + {sc_cs}]",  // cs (tham số thứ 7)
    // Căn stack 16 byte trước `call` (SysV); hai tham số cuối đẩy lên stack.
    "and rsp, -16",
    "push r12", // 8: rip
    "push r13", // 7: cs
    "call syscall_dispatch",
    "add rsp, 16",
    "mov rsp, rbx",
    // Trả kết quả theo `[calling.x86_64]`: `rax` = error, `rdx` = value.
    "mov [rsp + {sc_rax}], rax",
    "mov [rsp + {sc_rdx}], rdx",
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
    "and rsp, -16",
    "call exc_dispatch",
    "mov rsp, rbx",
    "test rax, rax",
    "jz 9f",
    "mov [rsp + {rip_off}], rax",
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
    ".globl vifast_probe_unknown",
    "vifast_probe_unknown:",
    "int 0x30",
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
    // KT-270: vector `int 0x80` + offset frame của stub syscall (khớp `crate::umode`).
    syscall_vec = const crate::exc::VEC_SYSCALL as u32,
    sc_rax = const crate::umode::SYSCALL_FRAME_RAX,
    sc_rdx = const crate::umode::SYSCALL_FRAME_RDX,
    sc_rsi = const crate::umode::SYSCALL_FRAME_RSI,
    sc_rdi = const crate::umode::SYSCALL_FRAME_RDI,
    sc_r8 = const crate::umode::SYSCALL_FRAME_R8,
    sc_r10 = const crate::umode::SYSCALL_FRAME_R10,
    sc_rip = const crate::umode::SYSCALL_FRAME_RIP,
    sc_cs = const crate::umode::SYSCALL_FRAME_CS,
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
}

// Handler **syscall** của ảnh — định nghĩa ở `vifast-kbin-x86_64` (`#[no_mangle]`).
//
// Stub `vifast_isr_syscall` gọi hàm này với 8 tham số SysV (6 thanh ghi + 2 trên stack) và nhận
// `SyscallRet` ở `rax:rdx`. Khai báo ở đây (crate plat) để stub asm gọi được mà ảnh **không**
// cần `unsafe` (R5).
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
#[allow(dead_code)] // chỉ được gọi từ stub asm (`vifast_isr_syscall`), không từ Rust.
extern "C" {
    fn syscall_dispatch(
        nr: u64,
        a0: u64,
        a1: u64,
        mi: u64,
        mr0: u64,
        mr1: u64,
        cs: u64,
        rip: u64,
    ) -> crate::umode::SyscallRet;
}

/// Địa chỉ stub của `slot` (theo [`crate::exc::stub_slot`]); slot lạ trả `0`.
///
/// `0` là sentinel an toàn: [`crate::idt::install`] ghi offset 0 vào ô IDT, và một exception rơi
/// vào đó sẽ `#GP`/`#DF` **thấy được** thay vì im lặng.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn stub_addr(slot: usize) -> u64 {
    use crate::exc::{
        STUB_AC, STUB_BP, STUB_CP, STUB_DB, STUB_DE, STUB_DF, STUB_GP, STUB_NP, STUB_PF,
        STUB_SPURIOUS, STUB_SS, STUB_SX, STUB_SYSCALL, STUB_TIMER, STUB_TS, STUB_UD, STUB_UNKNOWN,
        STUB_VC,
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
/// Transitions to Inner Privilege Levels"): `SS`, `RSP`, `RFLAGS`, `CS`, `RIP`. `CS = 0x1B`
/// (`RPL 3`) và `SS = 0x23` (`RPL 3`) là điều kiện **bắt buộc** để hạ đặc quyền; `DS`/`ES` được
/// đặt về selector user data trước `iretq` (ring 3 không dùng phân đoạn ở long mode, nhưng giữ
/// đúng trạng thái).
///
/// `rflags` do caller dựng ([`crate::umode::USER_RFLAGS`] = `0x202`: `IF = 1`). Hàm **không** trả
/// về: đường về duy nhất là `int 0x80` → handler → `iretq` (hoặc halt khi lỗi).
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn enter_user(entry: u64, stack_top: u64, rflags: u64) -> ! {
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

/// Đọc một byte tại địa chỉ **user** (identity map ⇒ VA == PA) — dùng cho `DebugPutStr` từ ring 3.
///
/// Kernel đọc được trang user vì `CR4.SMAP = 0` (chưa bật SMAP); nếu bật SMAP, lần đọc đầu fault
/// và handler in `# exc: vec=14` thay vì im lặng.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn read_u8(addr: u64) -> u8 {
    // SAFETY: caller bảo đảm `addr` nằm trong vùng đã map (trang user của ring 3).
    unsafe { core::ptr::read_volatile(addr as *const u8) }
}

// ---------------------------------------------------------------------------------------------
// Dừng máy
// ---------------------------------------------------------------------------------------------

/// Chờ bộ truyền UART rỗng hẳn (`LSR.TEMT`) — **có trần** [`TX_WAIT_LIMIT`] — rồi:
///
/// - `code == 0`: ACPI shutdown `outw(0x604, 0x2000)` ⇒ QEMU thoát mã 0;
/// - `code != 0`: **không** shutdown, `cli; hlt` ⇒ QEMU treo ⇒ runner ĐỎ.
///
/// Không bao giờ trả về.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn halt(code: u32) -> ! {
    let mut spins = 0u32;
    while spins < TX_WAIT_LIMIT {
        if inb(COM1_BASE + REG_LSR) & serial::LSR_TEMT != 0 {
            break;
        }
        spins += 1;
    }
    if code == 0 {
        outw(ACPI_PM1A_CNT, ACPI_SHUTDOWN_VALUE);
    }
    loop {
        // SAFETY: `cli; hlt` ở CPL0; `nomem`/`nostack` đúng.
        unsafe {
            asm!("cli", "hlt", options(nomem, nostack));
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Host stub — **panic**, không im lặng (cùng khuôn `vifast-plat-aarch64::asm`)
// ---------------------------------------------------------------------------------------------

/// Stub host: gọi asm đặc quyền trên máy thật là lỗi lập trình, phải **kêu to**.
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
fn host_only(name: &str) -> ! {
    panic!("vifast_plat_x86_64::asm::{name}: chỉ chạy trên x86_64 bare-metal (target_os = none)")
}

/// Stub host của [`outb`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn outb(_port: u16, _value: u8) {
    host_only("outb")
}

/// Stub host của [`outw`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn outw(_port: u16, _value: u16) {
    host_only("outw")
}

/// Stub host của [`inb`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn inb(_port: u16) -> u8 {
    host_only("inb")
}

/// Stub host của [`outl`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn outl(_port: u16, _value: u32) {
    host_only("outl")
}

/// Stub host của [`inl`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn inl(_port: u16) -> u32 {
    host_only("inl")
}

/// Stub host của [`read_cr0`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn read_cr0() -> u64 {
    host_only("read_cr0")
}

/// Stub host của [`read_cr3`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn read_cr3() -> u64 {
    host_only("read_cr3")
}

/// Stub host của [`read_cr4`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn read_cr4() -> u64 {
    host_only("read_cr4")
}

/// Stub host của [`read_msr`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn read_msr(_msr: u32) -> u64 {
    host_only("read_msr")
}

/// Stub host của [`write_msr`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn write_msr(_msr: u32, _value: u64) {
    host_only("write_msr")
}

/// Stub host của [`write_u32`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn write_u32(_addr: u64, _value: u32) {
    host_only("write_u32")
}

/// Stub host của [`sti`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn sti() {
    host_only("sti")
}

/// Stub host của [`cli`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn cli() {
    host_only("cli")
}

/// Stub host của [`hlt`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn hlt() {
    host_only("hlt")
}

/// Stub host của [`read_u32`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn read_u32(_addr: u64) -> u32 {
    host_only("read_u32")
}

/// Stub host của [`halt`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn halt(_code: u32) -> ! {
    host_only("halt")
}

/// Stub host của [`read_u64`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn read_u64(_addr: u64) -> u64 {
    host_only("read_u64")
}

/// Stub host của [`read_u16`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn read_u16(_addr: u64) -> u16 {
    host_only("read_u16")
}

/// Stub host của [`write_u8`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn write_u8(_addr: u64, _value: u8) {
    host_only("write_u8")
}

/// Stub host của [`write_u16`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn write_u16(_addr: u64, _value: u16) {
    host_only("write_u16")
}

/// Stub host của [`write_u64`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn write_u64(_addr: u64, _value: u64) {
    host_only("write_u64")
}

/// Stub host của [`read_cr2`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn read_cr2() -> u64 {
    host_only("read_cr2")
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

/// Stub host của [`enter_user`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn enter_user(_entry: u64, _stack_top: u64, _rflags: u64) -> ! {
    host_only("enter_user")
}

/// Stub host của [`read_u8`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn read_u8(_addr: u64) -> u8 {
    host_only("read_u8")
}
