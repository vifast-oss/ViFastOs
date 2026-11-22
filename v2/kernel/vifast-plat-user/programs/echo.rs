// SPDX-License-Identifier: MIT OR Apache-2.0
//! **echo** — chương trình U-mode kiểm **IPC buffer** của KT-339 (TAP 44, aarch64).
//!
//! Cùng nguồn Rust với `procsrv`/`shell`: chỉ `use vifast_user_abi::…`, **không** `cfg` kiến
//! trúc. Chương trình:
//!
//! 1. đọc **MR4..MR7** mà kernel đã ghi sẵn vào IPC buffer (`mrs()` — trang do loader map);
//! 2. ghi lại 4 word đó với một **biến đổi biết trước** (`set_mr()`), chứng minh chiều **ghi**
//!    buffer của U-mode;
//! 3. `send(EP_ECHO, label, 4 MR thanh ghi)` với `length = 8` ⇒ kernel phải đọc **thêm**
//!    MR4..MR7 từ buffer vừa ghi (chiều **đọc** buffer của kernel qua hook `SyscallEnv`);
//! 4. `Exit(0)`.
//!
//! Kernel khẳng định cả bốn word + `length` + nội dung trang thật (TAP 44) — chương trình chỉ
//! gọi syscall, không tự kết luận "ok".
#![no_std]
#![no_main]

extern crate vifast_user_abi;
use vifast_user_abi::{exit, mrs, put, send, set_mr};

/// Ô cap Endpoint echo trong CSpace chung của ảnh (khớp `k5user::SLOT_EP_ECHO`).
const EP_ECHO: u64 = 9;
/// Label của message echo (khớp `k5user::ECHO_LABEL`).
const ECHO_LABEL: u64 = 0x660;
/// Dịch bit `length` trong `MessageInfo` (`spec/abi.toml`).
const INFO_LEN_SHIFT: u64 = 28;
/// Mask biến đổi: mỗi word ghi lại = `word đọc ^ MASK` — kernel biết trước kết quả.
pub const ECHO_MASK: u64 = 0x0000_00FF_0000_00FF;
/// MR0..MR3 của message echo (chỉ đi trong **thanh ghi**, khác MR4..MR7 trong buffer).
const REG_MRS: [u64; 4] = [0xE0A0, 0xE0A1, 0xE0A2, 0xE0A3];

/// In hex 16 chữ số (`put` không format được; không `alloc`).
fn put_hex(mut value: u64) {
    let digits = b"0123456789abcdef";
    let mut buf = [0u8; 16];
    let mut index = buf.len();
    if value == 0 {
        index -= 1;
        buf[index] = b'0';
    }
    while value > 0 {
        index -= 1;
        buf[index] = digits[(value & 0xF) as usize];
        value >>= 4;
    }
    put(&buf[index..]);
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    put(b"# echo: up\n");
    let before = mrs();
    let mut after = [0u64; 4];
    let mut index = 0;
    while index < 4 {
        after[index] = before[index] ^ ECHO_MASK;
        let _ = set_mr(index, after[index]);
        index += 1;
    }
    put(b"# echo: read mr4=0x");
    put_hex(before[0]);
    put(b" write mr4=0x");
    put_hex(after[0]);
    put(b"\n");
    let info = ECHO_LABEL | (8u64 << INFO_LEN_SHIFT);
    let _ = send(EP_ECHO, info, REG_MRS);
    exit(0)
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    // Panic U-mode là lỗi không phục hồi; thoát mã 99 để kernel đỏ.
    exit(99)
}
