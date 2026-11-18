// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-157** — thân chung của hai client C/D: chạm từng trang theo bảng, kiểm **toàn bộ** 4096
//! byte so với `vifast_vfs::byte_at`, rồi `Exit(0)`.
//!
//! Khác client A/B (KT-70): **không** map khối kết quả và **không** dùng receive window — mục
//! đích của C/D là chứng minh **bốn tiến trình U-mode** (mỗi cái CSpace + bảng trang riêng) cùng
//! fault trên VA riêng và pager giao **đúng một lần** cho từng VA, nên chỉ cần mã Exit làm bằng
//! chứng dữ liệu: `0` = mọi byte đúng, `0x70 + index` = trang thứ `index` có byte sai.
//!
//! Không `unsafe`; mọi truy cập bộ nhớ qua shim (D2-101).

use vifast_user_abi::{exit, peek_u64, yield_now};
use vifast_vfs::{byte_at, PAGE_SIZE};

/// Chạy client: `table` = thứ tự `(file, page)`, `va_of` quy đổi VA của client.
pub fn cd_main(table: &[(u32, u32)], va_of: fn(u32, u32) -> u64) -> ! {
    let mut index = 0usize;
    while index < table.len() {
        let (file, page) = table[index];
        // Nhường CPU trước mỗi lần chạm: kernel có nhịp chạy pager giữa các client.
        let _ = yield_now();
        let va = va_of(file, page);
        let mut offset = 0usize;
        while offset < PAGE_SIZE {
            let word = peek_u64(va + offset as u64);
            let bytes = word.to_le_bytes();
            let mut inner = 0usize;
            while inner < 8 {
                if bytes[inner] != byte_at(file, page, offset + inner) {
                    exit(0x70 + index as u64);
                }
                inner += 1;
            }
            offset += 8;
        }
        index += 1;
    }
    exit(0)
}
