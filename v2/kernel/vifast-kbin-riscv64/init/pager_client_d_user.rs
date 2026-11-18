// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-157** — `pager_client_d` U-mode: tiến trình thứ tư, CSpace + bảng trang riêng, fault trên
//! VA riêng `D_BASE` (2 trang file 0). Pager phải giao **đúng một lần** cho từng VA.

#![no_std]
#![no_main]

extern crate vifast_abi;
extern crate vifast_user_abi;
extern crate vifast_vfs;

#[path = "pager_common.rs"]
mod common;
#[path = "pager_client_cd_common.rs"]
mod cd;

use common::{d_va, D_TABLE};

/// Quy đổi VA của client D: mọi trang đều thuộc file 0 nên chỉ phụ thuộc `page`.
fn va_of(file: u32, page: u32) -> u64 {
    debug_assert_eq!(file, 0);
    let _ = file;
    d_va(page)
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    cd::cd_main(&D_TABLE, va_of)
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    vifast_user_abi::exit(99)
}
