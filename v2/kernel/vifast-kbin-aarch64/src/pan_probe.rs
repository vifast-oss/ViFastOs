// SPDX-License-Identifier: Apache-2.0 OR MIT
//! **KT-418** — pha **cứng hóa `PAN`** trên aarch64 (TAP 50): bật `PSTATE.PAN = 1` cho EL1 rồi
//! **kiểm ca âm bằng data abort thật**.
//!
//! # Vì sao `PAN` là bản tương đương `SMAP` của aarch64
//!
//! `PSTATE.PAN = 1` ⇒ EL1 **không** đọc/ghi được trang `EL0` (trừ khi tạm `PAN = 0` quanh vùng
//! `copy_from_user`). Cùng vai trò với `CR4.SMAP` (x86_64) và `sstatus.SUM = 0` (riscv64): kernel
//! **không** chạm được bộ nhớ user bằng một phép đọc/ghi thường.
//!
//! # Ca âm (trap phần cứng thật, không tiêm)
//!
//! 1. `PAN = 1` ⇒ `ldr` từ `EL0_CODE_VA` (trang `USER_RX`, `EL0`) ⇒ **data abort cùng EL**
//!    (`EC = 0x25`) với `FAR` = đúng VA và `Access::Read`; handler của ảnh ghi `ESR/FAR/ELR` rồi
//!    resume `ELR + 4`.
//! 2. `PAN = 0` ⇒ **cùng** `ldr` **không** fault ⇒ chứng minh fault trên là do `PAN`, không phải
//!    do trang không map.
//! 3. `PAN` trả về `1` sau phép đo ⇒ trạng thái cứng hóa **bền**, không phải bật tạm rồi quên.
//!
//! # Vì sao bật được **toàn cục** (khác `SMAP` của x86_64)
//!
//! Kernel aarch64 **không** đọc/ghi trang user bằng VA user: loader ELF ghi nội dung qua **PA**
//! (identity map, supervisor) rồi mới `map_obj_with` sang VA `USER_*`; trang `.el0` lấy thẳng từ
//! ảnh. Vì vậy `PAN = 1` toàn cục **không** phá đường nạp — đo được ở lần boot này.

use core::fmt::Write;

use vifast_plat_aarch64::asm;
use vifast_plat_aarch64::fault::{classify_esr, Access, EC_DATA_ABORT_SAME_EL};
use vifast_plat_aarch64::sysreg::Esr;
use vifast_plat_aarch64::umode::EL0_CODE_VA;

use crate::{faults, tap, Console};

/// Chạy pha `PAN`; `tap` = số hiệu TAP (50). Trả `true` ⇔ xanh.
pub fn run(console: &mut Console, tap_no: usize) -> bool {
    let hart = 0usize;
    // ---- 0) CPU **có** PAN không (`ID_AA64MMFR1_EL1.PAN`). Runner dùng `-cpu cortex-a72`
    // (ARMv8.0) ⇒ **không** có; `msr pan` khi đó là `#trap ec=0x00` và boot chết. Vì vậy phép đo
    // chỉ chạy khi CPU có, và log ghi rõ `pan_supported=0` để **không** ai đọc thành "đã kiểm PAN".
    if !asm::pan_supported() {
        let _ = writeln!(
            console,
            "# pan: va=0x{EL0_CODE_VA:x} pan_supported=0 (CPU khong co PAN — ARMv8.0) ok=1"
        );
        return tap(console, tap_no, "pan: el1 khong doc trang el0", true);
    }
    // ---- 1) PAN = 1: đọc trang EL0 từ EL1 phải data abort.
    faults::reset_raw(hart);
    faults::expect_raw(hart);
    asm::pan_set(true);
    let _ = asm::probe_read64(EL0_CODE_VA);
    let raw = faults::raw(hart);
    let _ = faults::take_expect_raw(hart);
    let (fault_ok, ec, far, elr) = match raw {
        Some(raw) => {
            let esr = Esr::from_bits(raw.esr);
            let abort = classify_esr(raw.esr, raw.far);
            let ok = matches!(
                abort,
                Some(abort) if abort.access == Access::Read
                    && abort.va == EL0_CODE_VA
                    && abort.ec == EC_DATA_ABORT_SAME_EL
            );
            (ok, esr.ec_raw(), raw.far, raw.elr)
        }
        None => (false, 0, 0, 0),
    };

    // ---- 2) PAN = 0: cùng phép đọc **không** fault (đối chứng dương).
    faults::reset_raw(hart);
    asm::pan_set(false);
    let value = asm::probe_read64(EL0_CODE_VA);
    let no_fault = faults::raw(hart).is_none();

    // ---- 3) Trả PAN về 1 (trạng thái cứng hóa) rồi đọc **lại**: phải fault lại ⇒ chứng minh
    // `pan_set(true)` có hiệu lực **bền**, không phải "bật tạm rồi quên".
    faults::reset_raw(hart);
    faults::expect_raw(hart);
    asm::pan_set(true);
    let _ = asm::probe_read64(EL0_CODE_VA);
    let restored = faults::raw(hart).is_some();
    let _ = faults::take_expect_raw(hart);

    let ok = fault_ok && no_fault && restored;
    let _ = writeln!(
        console,
        "# pan: va=0x{EL0_CODE_VA:x} pan_supported=1 ec=0x{ec:02x} far=0x{far:x} elr=0x{elr:x} fault_ok={} pan_off_read=0x{value:016x} no_fault={} restored={} ok={}",
        fault_ok as u8,
        no_fault as u8,
        restored as u8,
        ok as u8,
    );
    tap(console, tap_no, "pan: el1 khong doc trang el0", ok)
}
