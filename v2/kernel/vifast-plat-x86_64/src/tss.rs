// SPDX-License-Identifier: MIT OR Apache-2.0
//! **TSS 64-bit** (Task State Segment) — KT-261 (K5' bước 2).
//!
//! Tệp này là **dữ liệu + bảng**: bố cục TSS 104 byte theo SDM Vol 3A §7.2.1 (Hình 7-11 "64-Bit
//! TSS"), vùng nhớ [`TSS`] ghi/đọc bằng atomic (an toàn), và hàm [`init`] điền `RSP0` + `IST1`.
//! Asm đặc quyền (`ltr`) nằm ở [`crate::asm`]; tệp này **không** `unsafe`.
//!
//! # Vì sao cần TSS ở bước này
//!
//! 1. **`ltr` là một phần của "GDT/TSS"** của tiêu chí KT-261: `str` phải trả selector TSS.
//! 2. **`IST` cho `#DF`**: double fault là lỗi **không thể xử lý trên stack hiện tại** (stack có
//!    thể đang hỏng — đó chính là nguyên nhân #DF). `IST1` cho vector 8 là cách duy nhất để có
//!    một stack **sạch** cho handler #DF. Bước này **cấu hình** IST1 và **kiểm** nó qua descriptor
//!    IDT + trường `IST1` của TSS; **không** kích hoạt #DF (không cần cho tiêu chí, và một #DF
//!    thật thì không "thu hồi" được).
//! 3. `RSP0` là stack kernel dùng khi chuyển từ CPL3 → CPL0. Bước này **chưa** có U-mode, nhưng
//!    TSS phải có `RSP0` hợp lệ để không phải sửa lại bố cục khi mở U-mode.
//!
//! # Bố cục (SDM Hình 7-11, offset tính bằng byte)
//!
//! | Offset | Cỡ | Trường |
//! |---|---|---|
//! | `0x00` | 4 | dự phòng |
//! | `0x04` | 8 | `RSP0` |
//! | `0x0C` | 8 | `RSP1` |
//! | `0x14` | 8 | `RSP2` |
//! | `0x1C` | 8 | dự phòng |
//! | `0x24` | 8 | `IST1` |
//! | `0x2C`..`0x54` | 8 × 6 | `IST2`..`IST7` |
//! | `0x5C` | 8 | dự phòng |
//! | `0x64` | 2 | dự phòng |
//! | `0x66` | 2 | `I/O map base` |
//!
//! Tổng **104** byte (`0x68`). `I/O map base = 104` nghĩa là **không có** bitmap I/O (giá trị
//! bằng limit + 1 ⇒ mọi truy cập I/O từ CPL3 đều #GP; bước này không có U-mode nên chỉ là hợp
//! đồng dữ liệu).
//!
//! # Vì sao `[AtomicU32; 26]` chứ không `#[repr(C)] struct`
//!
//! `RSP0` nằm ở offset **4** — không căn 8. Một `#[repr(C)]` với `AtomicU64` sẽ bị chèn đệm và
//! lệch layout; `#[repr(C, packed)]` + atomic là **UB** (truy cập atomic không căn). Mọi trường
//! của TSS đều căn **4** (4, 12, 20, 36, 44, …, 102), nên `[AtomicU32; 26]` phủ đúng layout và
//! mỗi trường 64-bit ghi bằng **hai** `u32` (thứ tự little-endian của x86_64).

use core::sync::atomic::{AtomicU32, Ordering};

/// Cỡ TSS 64-bit theo đặc tả (SDM §7.2.1): **104** byte.
pub const TSS_BYTES: usize = 104;
/// `limit` của descriptor TSS = byte cuối = 103.
pub const TSS_LIMIT: u16 = (TSS_BYTES - 1) as u16;
/// Số `u32` của vùng nhớ TSS (104 / 4).
pub const TSS_WORDS: usize = TSS_BYTES / 4;

/// Offset `RSP0` — stack kernel khi vào từ CPL3.
pub const TSS_RSP0_OFFSET: usize = 0x04;
/// Offset `IST1` — stack riêng cho `#DF` (vector 8).
pub const TSS_IST1_OFFSET: usize = 0x24;
/// Khoảng cách giữa hai `ISTn`.
pub const TSS_IST_STRIDE: usize = 8;
/// Số `IST` (SDM: `IST1`..`IST7`).
pub const TSS_IST_COUNT: usize = 7;
/// Offset trường `I/O map base`.
pub const TSS_IO_MAP_OFFSET: usize = 0x66;
/// Giá trị `I/O map base` khi **không** có bitmap: bằng `limit + 1`.
pub const TSS_IO_MAP_BASE: u16 = TSS_BYTES as u16;

/// Vùng nhớ TSS — ghi/đọc bằng atomic (an toàn, không `unsafe`).
static TSS: [AtomicU32; TSS_WORDS] = [const { AtomicU32::new(0) }; TSS_WORDS];

/// Địa chỉ (VA = PA) của TSS — descriptor trong GDT phải trỏ đúng giá trị này.
pub fn base() -> u64 {
    TSS.as_ptr() as u64
}

/// Ghi `value` (64-bit) vào `offset` (phải căn 4) của TSS, little-endian.
fn write_u64(offset: usize, value: u64) {
    assert!(offset.is_multiple_of(4), "offset TSS phải căn 4");
    TSS[offset / 4].store(value as u32, Ordering::Relaxed);
    TSS[offset / 4 + 1].store((value >> 32) as u32, Ordering::Relaxed);
}

/// Đọc trường 64-bit ở `offset` (phải căn 4) — dùng cho bằng chứng `IST1`/`RSP0`.
pub fn read_u64(offset: usize) -> u64 {
    assert!(offset.is_multiple_of(4), "offset TSS phải căn 4");
    let low = TSS[offset / 4].load(Ordering::Relaxed) as u64;
    let high = TSS[offset / 4 + 1].load(Ordering::Relaxed) as u64;
    (high << 32) | low
}

/// Ghi trường 16-bit ở `offset` (phải căn 2).
fn write_u16(offset: usize, value: u16) {
    assert!(offset.is_multiple_of(2), "offset TSS phải căn 2");
    let word = TSS[offset / 4].load(Ordering::Relaxed);
    let shift = (offset % 4) * 8;
    let mask = !(0xFFFFu32 << shift);
    TSS[offset / 4].store((word & mask) | ((value as u32) << shift), Ordering::Relaxed);
}

/// Đọc trường 16-bit ở `offset` (phải căn 2).
pub fn read_u16(offset: usize) -> u16 {
    assert!(offset.is_multiple_of(2), "offset TSS phải căn 2");
    let word = TSS[offset / 4].load(Ordering::Relaxed);
    ((word >> ((offset % 4) * 8)) & 0xFFFF) as u16
}

/// Điền TSS: `RSP0 = rsp0`, `IST1 = ist1`, `I/O map base = 104` (không bitmap).
///
/// Gọi **trước** [`crate::gdt::install`] để descriptor TSS trỏ vào một TSS đã hợp lệ.
pub fn init(rsp0: u64, ist1: u64) {
    write_u64(TSS_RSP0_OFFSET, rsp0);
    write_u64(TSS_IST1_OFFSET, ist1);
    write_u16(TSS_IO_MAP_OFFSET, TSS_IO_MAP_BASE);
}

/// `RSP0` đã nạp.
pub fn rsp0() -> u64 {
    read_u64(TSS_RSP0_OFFSET)
}

/// `ISTn` (`n` từ 1 tới 7) đã nạp; ngoài phạm vi trả 0.
pub fn ist(n: usize) -> u64 {
    if n == 0 || n > TSS_IST_COUNT {
        return 0;
    }
    read_u64(TSS_IST1_OFFSET + (n - 1) * TSS_IST_STRIDE)
}

/// `I/O map base` đã nạp.
pub fn io_map_base() -> u16 {
    read_u16(TSS_IO_MAP_OFFSET)
}
