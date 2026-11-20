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

/// Đọc 32-bit tại địa chỉ **vật lý** (identity map ⇒ VA == PA).
///
/// Dùng để đọc `hvm_start_info.magic` mà `%ebx` trỏ tới — bằng chứng giao thức PVH. Địa chỉ nằm
/// trong identity map 1 GiB nên không fault kể cả khi `%ebx` rác.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn read_u32(addr: u64) -> u32 {
    // SAFETY: caller bảo đảm `addr` nằm trong vùng đã map (identity map 1 GiB của ảnh boot).
    unsafe { core::ptr::read_volatile(addr as *const u32) }
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
