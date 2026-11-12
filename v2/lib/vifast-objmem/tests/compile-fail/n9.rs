// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ca **compile-fail** của KT-15: `with_many_mut` với `N = 9` **phải không build**.
//!
//! `ObjStore::with_many_mut` cưỡng chế trần `N ≤ 8` **lúc biên dịch** bằng
//! `const { assert!(N <= MAX_MANY, ...) }` (thiết kế `:208`; K1A-PLAN §4/KT-15).
//! Tệp này **không** nằm trong cây test tự động của cargo (thư mục con
//! `tests/compile-fail/`); nó được biên dịch bởi `tests/compile_fail.rs`, test đó
//! khẳng định `rustc` thoát khác 0 và stderr chứa thông điệp trần `N`.
//!
//! Chạy tay:
//! `rustc --edition 2021 --emit=metadata --extern vifast_objmem=<rlib> \
//!   tests/compile-fail/n9.rs` → phải đỏ.

use core::any::Any;

use vifast_objmem::{KObj, ObjMem, ObjRef, ObjStore};

struct Be(u64);

impl KObj for Be {
    fn kind(&self) -> u8 {
        // Đọc field để tệp không sinh warning `dead_code` (log đỏ phải sạch nhiễu).
        self.0 as u8
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }
}

fn main() {
    let mut vung = [0u8; 4096];
    // SAFETY: vùng nhớ hợp lệ và chỉ dùng trong `main`.
    let mut s = unsafe { ObjMem::new(vung.as_mut_ptr(), vung.len()) };
    let pa = s.alloc::<Be>().expect("còn chỗ");
    // SAFETY: `pa` vừa cấp cho `Be`.
    let r: ObjRef<dyn KObj> = unsafe { s.place(pa, Be(1)) }.into_dyn();

    // N = 9 > 8 ⇒ `const { assert!(N <= 8) }` phải chặn **lúc biên dịch**.
    let _ = s.with_many_mut([r; 9], |_| {});
}
