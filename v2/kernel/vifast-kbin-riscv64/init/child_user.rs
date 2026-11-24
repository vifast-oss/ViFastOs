// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-65 — tiến trình **con** chạy U-mode (Rust, ELF riêng nạp như `init`).
//!
//! In marker qua `DebugPutStr` (chứng minh đã chạy thật trong U-mode) rồi `Exit(42)`; kernel/core
//! ghi `FaultKind::Exit{42}` vào `fault_ep` của procsrv (QD §1 Q-A), procsrv `Recv` rồi core đưa
//! TCB về `Inactive` (D2-25). Đây là mắt xích "có vòng đời" của KT-65.
#![no_std]
#![no_main]

extern crate vifast_user_abi;
use vifast_user_abi::{exit, put};

/// **KT-401 — fixture "vùng nạp > 24 KiB"**: mảng tĩnh 27 KiB nằm trong `.rodata` của ELF con.
///
/// Trước KT-401 biên user riscv64 là guard `0x4000_7000` (ảnh chỉ được tới ~28 KiB kể từ
/// `0x4000_0000`) + `MAX_INIT_PAGES = 16` ⇒ ảnh này **không nạp được** (`StackOverlap`/`Image`).
/// Sau KT-401: guard `0x400f_7000`, 32 trang ⇒ nạp + chạy thật. Đọc qua `black_box` để linker
/// **không** lược mảng (nếu lược, bằng chứng biên chỉ còn là hằng số).
static BIG: [u8; 27 * 1024] = [0xA5; 27 * 1024];

/// In số thập phân vào cuối `out`, trả chỉ số byte bắt đầu (không dùng alloc).
fn put_dec(mut n: u32, out: &mut [u8; 10]) -> usize {
    let mut i = 10usize;
    loop {
        i -= 1;
        out[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    i
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    // KT-401: chạm **thật** mảng lớn rồi in tổng (bằng chứng ảnh > 24 KiB đã nạp và chạy).
    let big = core::hint::black_box(&BIG);
    let sum = big.iter().fold(0u32, |acc, b| acc.wrapping_add(u32::from(*b)));
    let mut line = [0u8; 64];
    let head = b"# child: big bytes=27648 sum=";
    line[..head.len()].copy_from_slice(head);
    let mut num = [0u8; 10];
    let at = put_dec(sum, &mut num);
    let n = head.len() + (10 - at);
    line[head.len()..n].copy_from_slice(&num[at..]);
    line[n] = b'\n';
    put(&line[..n + 1]);
    let msg = b"# child: up then exit 42\n";
    put(msg);
    // Core ghi fault Exit{42} tới fault_ep (procsrv) — QD §1 Q-A.
    exit(42);
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    exit(99)
}
