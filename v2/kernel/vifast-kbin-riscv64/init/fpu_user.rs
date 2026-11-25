// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Nguồn Rust tối thiểu của chương trình U-mode **dấu phẩy động** (KT-418) — chỉ chứa
//! `global_asm!` của [`fpu_user.S`] và panic handler. Cùng đường build với `init` (build.rs gọi
//! `$RUSTC` + [`init.ld`]), nạp bằng cùng loader `vifast-elf` tại `0x4000_0000`.
#![no_std]
#![no_main]

core::arch::global_asm!(include_str!("fpu_user.S"));

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
