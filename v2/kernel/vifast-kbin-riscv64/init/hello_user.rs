// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-342** — chương trình U-mode `/hello.elf` nhỏ nhất của **ảnh sản phẩm** riscv64.
//!
//! Vì sao cần: tiêu chí 3 của `KT-342` đòi **`exec` chạy một tệp rồi thoát sạch** trên **một**
//! boot sản phẩm (`run /hello.elf`). Chương trình này là ảnh **đích** của lần `exec` ấy: nó in
//! dấu chuẩn `RUN OK` (chuỗi mà v1 in, xem `spec/v1-parity.toml` mục `run`) + một dòng evidence
//! riêng `# hello: run ok`, rồi `Exit(0)` — để kernel thấy `shell_exit=0` và halt sạch.
//!
//! Không `unsafe`, không `asm!`, không cấp phát: chỉ ba syscall đã có (`init_ipc_buffer`,
//! `DebugPutStr`, `Exit`).

#![no_std]
#![no_main]

extern crate vifast_user_abi;

use vifast_user_abi::{exit, init_ipc_buffer, put};

#[no_mangle]
pub extern "C" fn _start() -> ! {
    init_ipc_buffer();
    // Chuỗi `RUN OK` là kỳ vọng của parity v1 (`spec/v1-parity.toml` mục `run`) — in nguyên.
    put(b"RUN OK\n");
    // Dòng evidence của KT-342: **ảnh đích của `exec`** đã chạy **thật** trong VSpace mới.
    put(b"# hello: run ok\n");
    exit(0)
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    // `panic` trong chương trình U-mode ⇒ mã thoát khác 0 (kernel coi là shell_exit != 0 ⇒ đỏ).
    exit(99)
}
