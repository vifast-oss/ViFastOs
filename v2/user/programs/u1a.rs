// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-347 (U1a)** — chương trình PoC **`u1a.elf`**: chứng minh `vifast-rt` (`no_std + alloc`,
//! toolchain stable 1.100.0) build được thành ELF U-mode và **chạy thật** trên ảnh sản phẩm
//! riscv64 khi được nạp bằng `run /u1a.elf` — tức **exec đọc ELF từ FS** (nợ KT-346 mục 1).
//!
//! # Bằng chứng (ảnh sản phẩm không có TAP — D2-08)
//!
//! ```text
//! RUN OK
//! # u1a: rt up heap_bytes=8192
//! # u1a: vec n=16 sum=376
//! # u1a: alloc-string:376
//! # u1a: heap used=NNN/8192
//! # u1a: rt done
//! ```
//!
//! `Vec` + `String` + `format!` đều đi qua `#[global_allocator]` (arena tĩnh 8 KiB) ⇒ nếu alloc
//! hỏng, `# u1a: alloc fail …` (Exit 95) hoặc `# u1a: panic …` (Exit 98) xuất hiện thay cho
//! `rt done` — hai đột biến đỏ của KT-347 dựa vào đúng hai dòng này.
//!
//! Không `unsafe`, không `asm!`: mọi thứ đi qua `vifast_rt` (API an toàn).

#![no_std]
#![no_main]

extern crate alloc;
extern crate vifast_rt;
extern crate vifast_user_abi;

use alloc::string::String;
use alloc::vec::Vec;
use vifast_rt::{entry, heap_stats, println, BumpAlloc};

/// `#[global_allocator]` — đặt ở **chương trình** (không phải rlib) để linker luôn thấy; kiểu
/// `BumpAlloc` là unit struct `Sync`, phần `unsafe` của `GlobalAlloc` nằm ở shim (D2-17).
#[global_allocator]
static ALLOC: BumpAlloc = BumpAlloc;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    entry(rt_main)
}

/// `main` của PoC: in, cấp phát thật qua `Vec`/`String`, in số đo, thoát 0.
fn rt_main() {
    // `RUN OK` là chuỗi parity v1 của ảnh đích `exec` (`spec/v1-parity.toml` mục `run`).
    println!("RUN OK");
    println!("# u1a: rt up heap_bytes={}", vifast_rt::HEAP_BYTES);

    // 1) `Vec<u64>` — 16 phần tử, cấp phát + lớn dần (realloc của bump).
    let mut values: Vec<u64> = Vec::new();
    for index in 0..16u64 {
        values.push(index * 3 + 1);
    }
    let sum: u64 = values.iter().sum();
    println!("# u1a: vec n={} sum={}", values.len(), sum);

    // 2) `String` + `format!` — đường `core::fmt` + alloc.
    let text: String = alloc::format!("alloc-string:{}", sum);
    println!("# u1a: {}", text);

    // 3) Số đo thật của arena (không phải hằng): `used` > 0 chứng minh alloc đã cấp.
    let (used, cap) = heap_stats();
    println!("# u1a: heap used={}/{}", used, cap);
    println!("# u1a: rt done");
}
