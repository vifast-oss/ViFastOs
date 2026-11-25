// SPDX-License-Identifier: Apache-2.0 OR MIT
//! **KT-418** — pha **cứng hóa `SMEP`/`SMAP`** trên x86_64 (TAP 50): bật `CR4.SMEP`/`CR4.SMAP`
//! **chỉ khi** `CPUID.07H:EBX` báo CPU có, rồi **kiểm ca âm bằng trap phần cứng thật**.
//!
//! # Hai ca âm
//!
//! 1. **SMAP** — `probe_read8(user_va)`: đọc **một byte** từ trang `US=1` khi `RFLAGS.AC = 0`
//!    ⇒ `#PF` với `err` có `P=1` (bảo vệ), `W=0` (đọc), `U/S=1` (trang user), `CR2 = user_va`.
//! 2. **SMEP** — `probe_exec(user_va)`: **nhảy** tới trang `US=1` ⇒ `#PF` với `err` có `P=1`,
//!    `I/D=1` (instruction fetch), `U/S=1`, `CR2 = user_va` — fault xảy ra **trước khi**
//!    instruction ở trang user chạy.
//!
//! # Vì sao phải kiểm `CPUID` trước
//!
//! Model mặc định của runner là `qemu64` (KT-323 giữ nguyên để số đo cũ tái lập được) và **đo
//! được** là nó **không** có SMEP/SMAP (`cpuid_smep=0 cpuid_smap=0`). Ghi `CR4` với bit CPU
//! không hỗ trợ ⇒ `#GP` ⇒ boot chết. Vì vậy:
//!
//! - CPU **có** ⇒ bật + chạy **cả hai** ca âm; TAP xanh ⇔ cả hai fault đúng `err`/`CR2`.
//! - CPU **không có** ⇒ không bật (không có gì để bật); TAP xanh ⇔ `hardened()` (không bit nào
//!   "hỗ trợ mà chưa bật") — log ghi `cpuid_smep=0` để **không** ai đọc thành "đã kiểm SMEP".
//!
//! Bằng chứng đầy đủ của ca âm (khi CPU có) đo được bằng `V2_X64_CPU=max` — xem journal
//! `kt418-sec1.md` §7.

use core::fmt::Write;

use vifast_plat_x86_64::asm;
use vifast_plat_x86_64::exc;

use crate::{check, Console, EXC_CR2, EXC_ERR, EXC_VEC};

/// Bit `#PF` `err`: `P` (bảo vệ, không phải "không có trang").
const PF_ERR_PROTECTION: u64 = 1 << 0;
/// Bit `#PF` `err`: `U/S` — **chế độ truy cập** (1 = từ CPL3), **không** phải `U/S` của trang.
/// Vi phạm `SMEP`/`SMAP` xảy ra ở CPL0 ⇒ bit này **0** (đo được: `err=0x11` cho `SMEP`).
const PF_ERR_USER: u64 = 1 << 2;
/// Bit `#PF` `err`: `I/D` (instruction fetch).
const PF_ERR_INSN: u64 = 1 << 4;

/// Chạy pha SMEP/SMAP; `tap` = số hiệu TAP (50), `user_va` = VA **đã map `US=1`** trong bảng
/// trang đang hoạt động (trang mã ring 3 của `VSpace` động).
pub(crate) fn run(console: &mut Console, tap: usize, user_va: u64) {
    let rep = asm::fpu::enable_smep_smap();
    let mut smap_ok = true;
    let mut smep_ok = true;
    let mut smap_err = 0u64;
    let mut smep_err = 0u64;
    let mut smap_cr2 = 0u64;
    let mut smep_cr2 = 0u64;

    if rep.smap {
        crate::exc_reset();
        crate::exc_arm(exc::VEC_PF as u64, asm::probe_read8_resume());
        asm::probe_read8(user_va);
        smap_err = EXC_ERR.load(core::sync::atomic::Ordering::Relaxed);
        smap_cr2 = EXC_CR2.load(core::sync::atomic::Ordering::Relaxed);
        smap_ok = EXC_VEC.load(core::sync::atomic::Ordering::Relaxed) == exc::VEC_PF as u64
            && smap_err & PF_ERR_PROTECTION != 0
            && smap_err & PF_ERR_USER == 0
            && smap_err & 0x2 == 0
            && smap_err & PF_ERR_INSN == 0
            && smap_cr2 == user_va;
    }
    if rep.smep {
        crate::exc_reset();
        crate::exc_arm(exc::VEC_PF as u64, asm::probe_exec_resume());
        asm::probe_exec(user_va);
        smep_err = EXC_ERR.load(core::sync::atomic::Ordering::Relaxed);
        smep_cr2 = EXC_CR2.load(core::sync::atomic::Ordering::Relaxed);
        smep_ok = EXC_VEC.load(core::sync::atomic::Ordering::Relaxed) == exc::VEC_PF as u64
            && smep_err & PF_ERR_PROTECTION != 0
            && smep_err & PF_ERR_USER == 0
            && smep_err & PF_ERR_INSN != 0
            && smep_cr2 == user_va;
    }

    let _ = writeln!(
        console,
        "# smep/smap: va=0x{user_va:x} cpuid_smep={} cpuid_smap={} smep={} smap={} smap_deferred={} cr4=0x{:016x} smap_err=0x{smap_err:x} smap_cr2=0x{smap_cr2:x} smap_ok={} smep_err=0x{smep_err:x} smep_cr2=0x{smep_cr2:x} smep_ok={} hardened={}",
        rep.cpuid_smep as u8,
        rep.cpuid_smap as u8,
        rep.smep as u8,
        rep.smap as u8,
        rep.smap_deferred() as u8,
        rep.cr4,
        smap_ok as u8,
        smep_ok as u8,
        rep.hardened() as u8,
    );
    // Xanh ⇔ (mọi bit CPU **có** đều đã bật) **và** (ca âm của bit nào bật thì fault đúng).
    let ok = rep.hardened() && smap_ok && smep_ok;
    check(console, tap, "smep/smap: bat va ca am", ok);
}
