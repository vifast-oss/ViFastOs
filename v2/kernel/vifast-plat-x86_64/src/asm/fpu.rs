// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `asm/fpu.rs` — **ngữ cảnh FPU/SIMD**: truy cập `CR0` (bit `MP`/`EM`/`TS` — bật/tắt lazy
//! switch) và `CR4` (`OSFXSR`/`OSXMMEXCPT` — bật `fxsave`/`xmm`). Vùng lưu **theo TCB**
//! (`fxsave`/`xsave`, KT-418) dựng trên các hàm này; tách cơ học từ `asm.rs`.
//! Stub host (`target_os != "none"`) panic to — không giả lập asm đặc quyền.

#[cfg(all(target_arch = "x86_64", target_os = "none"))]
use core::arch::asm;

// `super::*` chỉ cần cho **stub host** (`host_only`); trên target x86_64 mọi stub bị
// `cfg`-gate ra nên glob import là thừa (clippy `unused_imports`).
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
use super::*;

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

/// Ghi `CR0` — **KT-340a**: bật `CR0.WP` (bit 16) để ring 0 **không** ghi được trang chỉ-đọc.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub fn write_cr0(value: u64) {
    // SAFETY: `mov cr0, r64` ở CPL0; giữ nguyên các bit khác do caller quyết định.
    unsafe {
        asm!("mov cr0, {}", in(reg) value, options(nomem, nostack, preserves_flags));
    }
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

/// Stub host của [`read_cr0`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn read_cr0() -> u64 {
    host_only("read_cr0")
}

/// Stub host của [`read_cr4`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn read_cr4() -> u64 {
    host_only("read_cr4")
}

/// Stub host của [`write_cr0`].
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub fn write_cr0(_value: u64) {
    host_only("write_cr0")
}
