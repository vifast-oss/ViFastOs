// SPDX-License-Identifier: MIT OR Apache-2.0
//! **TSS 64-bit per-hart** (Task State Segment) — KT-261 (K5' bước 2) + **KT-278** (per-hart).
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
//! # KT-278 — vì sao TSS là **per-hart**
//!
//! KT-276 để nợ: hart phụ dùng **chung** GDT tạm 3 entry + IDT của BSP, **không** `ltr`, nên `#DF`
//! trên AP sẽ dùng chung `IST1` của BSP. Với **4 hart** nợ đó thành nguy hiểm thật: hai hart cùng
//! `#DF` sẽ ghi vào **một** stack IST1 và phá nhau. KT-278 vì thế cấp **mỗi hart một TSS riêng**
//! ([`TSS`] là `[[AtomicU32; TSS_WORDS]; MAX_HARTS]`), mỗi hart `ltr` descriptor trỏ **TSS của
//! chính nó** ([`crate::gdt::install`]), và `IST1` của mỗi hart là một stack riêng.
//!
//! [`check_distinct`] là phép kiểm **thuần số học** của địa chỉ: bốn vùng TSS phải **rời nhau**
//! (không trùng, không chồng lấn). Đó là đột biến (iii) của KT-278 — "TSS per-hart trùng địa chỉ"
//! bị phát hiện bằng **kiểm địa chỉ**, không bằng "hy vọng nó không xảy ra".
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

use crate::smp::MAX_HARTS;

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

/// Vùng nhớ TSS **per-hart** — mỗi hart một bản 104 byte, ghi/đọc bằng atomic (an toàn).
///
/// `MAX_HARTS = 8` (D2-195 Q5) nên vùng này là 832 byte; ảnh KT-278 chỉ dùng 4 bản đầu.
static TSS: [[AtomicU32; TSS_WORDS]; MAX_HARTS] =
    [const { [const { AtomicU32::new(0) }; TSS_WORDS] }; MAX_HARTS];

/// Lỗi **kiểm địa chỉ** TSS per-hart — fail-closed: mọi biến thể đều làm boot PANIC.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TssError {
    /// Hai hart có **cùng** địa chỉ TSS — đột biến (iii) của KT-278.
    Duplicate {
        /// Hart có địa chỉ thấp hơn (xuất hiện trước).
        a: usize,
        /// Hart trùng địa chỉ với `a`.
        b: usize,
    },
    /// Hai vùng TSS **chồng lấn** (cách nhau < [`TSS_BYTES`]) nhưng không trùng tuyệt đối.
    Overlap {
        /// Hart có địa chỉ thấp hơn.
        a: usize,
        /// Hart có địa chỉ cao hơn nhưng bắt đầu **trước** khi vùng `a` kết thúc.
        b: usize,
    },
}

/// Kiểm **thuần số học**: các địa chỉ TSS phải **rời nhau** (không trùng, không chồng lấn).
///
/// Tách khỏi [`base`] để test host gọi được với mảng **cố ý sai** (đột biến iii) mà không phải
/// thêm API "đặt địa chỉ tuỳ ý" vào đường sản phẩm. Thứ tự kiểm: `Duplicate` **trước** `Overlap`
/// vì trùng tuyệt đối là triệu chứng cụ thể hơn (hai hart dùng chung một TSS).
pub fn check_distinct(bases: &[u64]) -> Result<(), TssError> {
    let mut a = 0;
    while a < bases.len() {
        let mut b = a + 1;
        while b < bases.len() {
            if bases[a] == bases[b] {
                return Err(TssError::Duplicate { a, b });
            }
            let (low, high) = if bases[a] < bases[b] {
                (bases[a], bases[b])
            } else {
                (bases[b], bases[a])
            };
            if high - low < TSS_BYTES as u64 {
                return Err(TssError::Overlap { a, b });
            }
            b += 1;
        }
        a += 1;
    }
    Ok(())
}

/// Địa chỉ (VA = PA) vùng TSS của `hart` — descriptor trong GDT của hart đó phải trỏ đúng giá trị
/// này.
///
/// `hart >= MAX_HARTS` là lỗi lập trình (caller đã kiểm [`crate::smp::valid_hart`]) ⇒ panic
/// fail-loud thay vì trả một địa chỉ rác.
pub fn base(hart: usize) -> u64 {
    assert!(hart < MAX_HARTS, "hart {hart} ngoài MAX_HARTS");
    TSS[hart].as_ptr() as u64
}

/// Ghi `value` (64-bit) vào `offset` (phải căn 4) của TSS **hart** `hart`, little-endian.
fn write_u64(hart: usize, offset: usize, value: u64) {
    assert!(hart < MAX_HARTS, "hart {hart} ngoài MAX_HARTS");
    assert!(offset.is_multiple_of(4), "offset TSS phải căn 4");
    TSS[hart][offset / 4].store(value as u32, Ordering::Relaxed);
    TSS[hart][offset / 4 + 1].store((value >> 32) as u32, Ordering::Relaxed);
}

/// Đọc trường 64-bit ở `offset` (phải căn 4) của TSS hart `hart` — dùng cho bằng chứng
/// `IST1`/`RSP0`.
pub fn read_u64(hart: usize, offset: usize) -> u64 {
    assert!(hart < MAX_HARTS, "hart {hart} ngoài MAX_HARTS");
    assert!(offset.is_multiple_of(4), "offset TSS phải căn 4");
    let low = TSS[hart][offset / 4].load(Ordering::Relaxed) as u64;
    let high = TSS[hart][offset / 4 + 1].load(Ordering::Relaxed) as u64;
    (high << 32) | low
}

/// Ghi trường 16-bit ở `offset` (phải căn 2) của TSS hart `hart`.
fn write_u16(hart: usize, offset: usize, value: u16) {
    assert!(hart < MAX_HARTS, "hart {hart} ngoài MAX_HARTS");
    assert!(offset.is_multiple_of(2), "offset TSS phải căn 2");
    let word = TSS[hart][offset / 4].load(Ordering::Relaxed);
    let shift = (offset % 4) * 8;
    let mask = !(0xFFFFu32 << shift);
    TSS[hart][offset / 4].store((word & mask) | ((value as u32) << shift), Ordering::Relaxed);
}

/// Đọc trường 16-bit ở `offset` (phải căn 2) của TSS hart `hart`.
pub fn read_u16(hart: usize, offset: usize) -> u16 {
    assert!(hart < MAX_HARTS, "hart {hart} ngoài MAX_HARTS");
    assert!(offset.is_multiple_of(2), "offset TSS phải căn 2");
    let word = TSS[hart][offset / 4].load(Ordering::Relaxed);
    ((word >> ((offset % 4) * 8)) & 0xFFFF) as u16
}

/// Điền TSS **của hart `hart`**: `RSP0 = rsp0`, `IST1 = ist1`, `I/O map base = 104`.
///
/// Gọi **trước** [`crate::gdt::install`] để descriptor TSS trỏ vào một TSS đã hợp lệ.
pub fn init(hart: usize, rsp0: u64, ist1: u64) {
    write_u64(hart, TSS_RSP0_OFFSET, rsp0);
    write_u64(hart, TSS_IST1_OFFSET, ist1);
    write_u16(hart, TSS_IO_MAP_OFFSET, TSS_IO_MAP_BASE);
}

/// `RSP0` đã nạp của hart `hart`.
pub fn rsp0(hart: usize) -> u64 {
    read_u64(hart, TSS_RSP0_OFFSET)
}

/// Đổi `RSP0` của hart `hart` — stack kernel dùng khi vào từ CPL3 (KT-270).
///
/// Ảnh gọi hàm này **ngay trước** `iretq` xuống ring 3 để chuyển từ stack boot sang một stack
/// riêng cho đường vào kernel từ ring 3 (`int 0x80` + ngắt). Không đổi descriptor TSS: chỉ đổi
/// giá trị `RSP0` trong vùng nhớ TSS mà descriptor đã trỏ.
pub fn set_rsp0(hart: usize, value: u64) {
    write_u64(hart, TSS_RSP0_OFFSET, value);
}

/// `ISTn` (`n` từ 1 tới 7) đã nạp của hart `hart`; ngoài phạm vi trả 0.
pub fn ist(hart: usize, n: usize) -> u64 {
    if n == 0 || n > TSS_IST_COUNT {
        return 0;
    }
    read_u64(hart, TSS_IST1_OFFSET + (n - 1) * TSS_IST_STRIDE)
}

/// `I/O map base` đã nạp của hart `hart`.
pub fn io_map_base(hart: usize) -> u16 {
    read_u16(hart, TSS_IO_MAP_OFFSET)
}
