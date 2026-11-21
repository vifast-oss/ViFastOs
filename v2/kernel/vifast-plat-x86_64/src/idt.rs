// SPDX-License-Identifier: MIT OR Apache-2.0
//! **IDT 256 ô × 16 byte** — KT-261 (K5' bước 2).
//!
//! Tệp này là **dữ liệu + bảng**: mã hoá một ô IDT (SDM Vol 3A §6.14, Hình 6-8 "64-Bit IDT
//! Gate Descriptor"), vùng nhớ [`IDT`] ghi/đọc bằng atomic (an toàn), và hàm [`install`] điền
//! **cả 256 ô** rồi `lidt`. Asm đặc quyền nằm ở [`crate::asm`]; tệp này **không** `unsafe`.
//!
//! # Vì sao điền **cả 256** ô
//!
//! Một ô IDT chưa điền có `P = 0` ⇒ mọi exception rơi vào đó sinh `#GP` **trong lúc đang xử lý
//! exception** ⇒ `#DF` ⇒ treo máy im lặng. Điền đủ 256 ô (mọi vector không có stub riêng trỏ
//! [`crate::exc::STUB_UNKNOWN`]) biến "vector lạ" thành một dòng log `# exc: vec=255 …` rồi halt
//! — **không** bao giờ im lặng.
//!
//! # Bố cục một ô (16 byte, little-endian)
//!
//! ```text
//!   byte 0..1   offset[15:0]
//!   byte 2..3   selector (0x08 — kernel code)
//!   byte 4      IST (bit 0..2) | dự phòng (bit 3..7)
//!   byte 5      type_attr (0x8E = P=1, DPL=0, gate ngắt 64-bit)
//!   byte 6..7   offset[31:16]
//!   byte 8..11  offset[63:32]
//!   byte 12..15 dự phòng (0)
//! ```
//!
//! `type_attr = 0x8E` là **interrupt gate** (CPU tự `cli` khi vào) — đúng cho exception: handler
//! không được bị ngắt giữa lúc đang dựng frame. `0x8F` (trap gate) chỉ dùng cho `#DB` nếu cần
//! bước qua; bước này không cần.

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use crate::asm;
use crate::exc;
use crate::gdt;

/// Số ô IDT — đặc tả: **256** (mọi vector 0..255).
pub const IDT_ENTRIES: usize = 256;
/// Cỡ bảng IDT tính bằng byte (256 × 16 = 4096).
pub const IDT_BYTES: usize = IDT_ENTRIES * 16;
/// `IDTR.limit` = cỡ bảng − 1 = **4095**.
pub const IDT_LIMIT: u16 = (IDT_BYTES - 1) as u16;

/// `type_attr` của **interrupt gate** 64-bit: `P=1`, `DPL=0`, `S=0`, `type=0xE`.
pub const GATE_INTERRUPT: u8 = 0x8E;
/// `type_attr` của **trap gate** 64-bit: `P=1`, `DPL=0`, `S=0`, `type=0xF`.
pub const GATE_TRAP: u8 = 0x8F;
/// `type_attr` của interrupt gate gọi được từ CPL3 (`DPL=3`) — dùng cho vector `0x80` (KT-270).
pub const GATE_INTERRUPT_USER: u8 = 0xEE;

/// `type_attr` của ô IDT cho `vec`: **chỉ** vector syscall `0x80` có `DPL = 3` (ring 3 gọi được);
/// mọi vector khác giữ `DPL = 0` (ring 3 **không** được gọi `int` vào exception handler).
///
/// Tách thành hàm `const` để test host chốt được lựa chọn này mà không cần `lidt` (asm đặc quyền
/// chỉ chạy trên bare-metal).
pub const fn gate_type_attr(vec: u8) -> u8 {
    if vec == crate::exc::VEC_SYSCALL {
        GATE_INTERRUPT_USER
    } else {
        GATE_INTERRUPT
    }
}

/// `IST` gán cho `#DF` — `IST1` (index 1). `0` nghĩa là "không dùng IST".
pub const IST_DOUBLE_FAULT: u8 = 1;

/// Mã hoá một ô IDT thành hai `u64` (little-endian) — **nguồn duy nhất** của phép chia offset.
pub const fn entry(offset: u64, selector: u16, ist: u8, type_attr: u8) -> [u64; 2] {
    let low = (offset & 0xFFFF)
        | ((selector as u64) << 16)
        | (((ist & 0x7) as u64) << 32)
        | ((type_attr as u64) << 40)
        | (((offset >> 16) & 0xFFFF) << 48);
    let high = (offset >> 32) & 0xFFFF_FFFF;
    [low, high]
}

/// Giải mã `offset` từ hai `u64` — dùng cho test host (khứ hồi mã hoá/giải mã).
pub const fn entry_offset(low: u64, high: u64) -> u64 {
    (low & 0xFFFF) | (((low >> 48) & 0xFFFF) << 16) | ((high & 0xFFFF_FFFF) << 32)
}

/// Giải mã `selector` từ `u64` đầu.
pub const fn entry_selector(low: u64) -> u16 {
    ((low >> 16) & 0xFFFF) as u16
}

/// Giải mã `IST` từ `u64` đầu.
pub const fn entry_ist(low: u64) -> u8 {
    ((low >> 32) & 0x7) as u8
}

/// Giải mã `type_attr` từ `u64` đầu.
pub const fn entry_type_attr(low: u64) -> u8 {
    ((low >> 40) & 0xFF) as u8
}

/// Một ô IDT — hai `u64` (16 byte), ghi/đọc bằng atomic.
#[repr(C)]
pub struct IdtEntry {
    low: AtomicU64,
    high: AtomicU64,
}

impl IdtEntry {
    /// Ô rỗng (`P = 0`) — trạng thái trước khi [`install`] điền.
    pub const fn new() -> Self {
        Self {
            low: AtomicU64::new(0),
            high: AtomicU64::new(0),
        }
    }

    /// Ghi một ô đã mã hoá.
    pub fn set(&self, entry: [u64; 2]) {
        self.low.store(entry[0], Ordering::Relaxed);
        self.high.store(entry[1], Ordering::Relaxed);
    }

    /// Đọc một ô đã mã hoá.
    pub fn get(&self) -> [u64; 2] {
        [
            self.low.load(Ordering::Relaxed),
            self.high.load(Ordering::Relaxed),
        ]
    }
}

impl Default for IdtEntry {
    fn default() -> Self {
        Self::new()
    }
}

/// Bảng IDT **thật** của ảnh — 256 ô, ghi/đọc bằng atomic (an toàn, không `unsafe`).
static IDT: [IdtEntry; IDT_ENTRIES] = [const { IdtEntry::new() }; IDT_ENTRIES];

/// Pseudo-descriptor `IDTR` — cùng layout 10 byte như `GDTR` (xem [`crate::gdt`]).
static IDTR: [AtomicU32; 3] = [AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0)];

/// Địa chỉ (VA = PA) của bảng IDT.
pub fn base() -> u64 {
    IDT.as_ptr() as u64
}

/// Đọc ô `vec` (0..256); ngoài phạm vi trả `[0, 0]`.
pub fn entry_at(vec: usize) -> [u64; 2] {
    match IDT.get(vec) {
        Some(slot) => slot.get(),
        None => [0, 0],
    }
}

/// `offset` (địa chỉ stub) của ô `vec`.
pub fn offset_of(vec: usize) -> u64 {
    let [low, high] = entry_at(vec);
    entry_offset(low, high)
}

/// `selector` của ô `vec`.
pub fn selector_of(vec: usize) -> u16 {
    entry_selector(entry_at(vec)[0])
}

/// `IST` của ô `vec`.
pub fn ist_of(vec: usize) -> u8 {
    entry_ist(entry_at(vec)[0])
}

/// `type_attr` của ô `vec`.
pub fn type_attr_of(vec: usize) -> u8 {
    entry_type_attr(entry_at(vec)[0])
}

/// Ghi `limit`/`base` vào pseudo-descriptor `IDTR` (chưa nạp — [`install`] mới `lidt`).
fn set_idtr(base: u64, limit: u16) {
    let word0 = (limit as u32) | (((base & 0xFFFF) as u32) << 16);
    let word1 = ((base >> 16) & 0xFFFF_FFFF) as u32;
    let word2 = ((base >> 48) & 0xFFFF) as u32;
    IDTR[0].store(word0, Ordering::Relaxed);
    IDTR[1].store(word1, Ordering::Relaxed);
    IDTR[2].store(word2, Ordering::Relaxed);
}

/// Đọc lại `IDTR` bằng `sidt` — bằng chứng "IDT đã nạp thật".
pub fn read_idtr() -> (u64, u16) {
    asm::sidt(IDTR.as_ptr() as u64);
    let word0 = IDTR[0].load(Ordering::Relaxed);
    let word1 = IDTR[1].load(Ordering::Relaxed);
    let word2 = IDTR[2].load(Ordering::Relaxed);
    let limit = (word0 & 0xFFFF) as u16;
    let base = (((word0 >> 16) & 0xFFFF) as u64)
        | ((word1 as u64) << 16)
        | (((word2 & 0xFFFF) as u64) << 48);
    (base, limit)
}

/// Nạp lại **cùng** bảng IDT đã cài — dùng cho hart phụ (KT-276).
///
/// Hart phụ thừa hưởng bảng IDT **dùng chung** (mọi ô đã điền từ trước, `P = 1`), nên nó chỉ cần
/// `lidt` lại pseudo-descriptor; điền lại 256 ô sẽ là ghi tranh với hart 0 mà không thêm nghĩa.
pub fn reload() {
    set_idtr(base(), IDT_LIMIT);
    asm::lidt(IDTR.as_ptr() as u64);
}

/// Điền **cả 256 ô** rồi nạp `IDTR`.
///
/// Mỗi ô lấy địa chỉ stub từ [`asm::stub_addr`] theo [`exc::stub_slot`]; `#DF` được gán `IST1`
/// (stack riêng), mọi vector khác `IST = 0` (stack hiện tại). Vector `0x80` (syscall) dùng gate
/// `DPL = 3` ([`gate_type_attr`]) để ring 3 gọi được; mọi vector khác `DPL = 0`.
pub fn install() {
    let mut vec = 0;
    while vec < IDT_ENTRIES {
        let slot = exc::stub_slot(vec as u8);
        let offset = asm::stub_addr(slot);
        let ist = if vec as u8 == exc::VEC_DF {
            IST_DOUBLE_FAULT
        } else {
            0
        };
        IDT[vec].set(entry(
            offset,
            gdt::SELECTOR_KERNEL_CODE,
            ist,
            gate_type_attr(vec as u8),
        ));
        vec += 1;
    }
    set_idtr(base(), IDT_LIMIT);
    asm::lidt(IDTR.as_ptr() as u64);
}
