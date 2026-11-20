// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Bảng vector AArch64** — hình học + cài đặt `VBAR_EL1` (KT-248, K4' bước 3 / B3.2).
//!
//! # Vì sao tệp này **không** có `unsafe`
//!
//! Crate giữ `#![deny(unsafe_code)]`; `unsafe` chỉ ở [`crate::asm`] (D2-17). Tệp này là **dữ
//! liệu** (16 ô, offset tuyệt đối, loại exception) + một hàm [`install`] gọi ba hàm asm đã có
//! (`write_vbar_el1`, `isb`, `read_vbar_el1`). Nhờ vậy toàn bộ hình học bảng vector chạy được
//! trên host (`tests/vector.rs`) — kể cả **đột biến đặt sai một offset**.
//!
//! # Vì sao offset là **dữ liệu** chứ không phải số trong asm
//!
//! Bảng vector là **hợp đồng giữa phần cứng và asm**: phần cứng nhảy tới `VBAR_EL1 + offset`
//! theo loại exception, còn asm của ảnh đặt từng stub ở offset ấy. Nếu hai bên lệch nhau, mọi
//! exception nhảy vào **giữa** một stub — không lỗi biên dịch, không lỗi lúc chạy, chỉ có hành vi
//! sai khi đã có fault. Vì vậy:
//!
//! - offset/loại nằm ở **đây** (host-testable, `tests/vector.rs` khẳng định từng giá trị tuyệt
//!   đối từ DDI 0487);
//! - asm của ảnh dùng `const` operand [`VECTOR_STRIDE`] cho `.org`, nên **không** có bản thứ hai
//!   của con số `0x80` để lệch;
//! - `v2/test/boot/run-aarch64.sh` (KT-250) đọc **ELF** và khẳng định từng symbol stub nằm đúng
//!   `base + i*0x80` — kiểm cả phía asm, không chỉ phía Rust.
//!
//! # Bất biến: bảng vector **không** nằm trong `.bss`
//!
//! `.bss` là `NOLOAD`: nếu bảng vector rơi vào đó thì nó **không** có trong ảnh đã nạp và
//! `VBAR_EL1` trỏ vào vùng rác (QEMU có thể tình cờ có RAM 0 nên "chạy được" — máy thật thì
//! không). Vì vậy asm đặt bảng trong `.text.vectors` (`"ax"`), linker script `KEEP` nó, và
//! `run-aarch64.sh` kiểm `objdump -h` thấy `.text.vectors` là `TEXT` **không** phải `BSS`.

use crate::asm;
use crate::sysreg::Vbar;

/// Số ô của bảng vector (DDI 0487 §D1.10.4): 4 nhóm × 4 loại exception.
pub const VECTOR_COUNT: usize = 16;

/// Khoảng cách giữa hai ô: **`0x80`** byte (32 instruction) — đặc tả cố định.
pub const VECTOR_STRIDE: u64 = 0x80;

/// Căn của cả bảng: **`0x800`** byte (`VBAR_EL1` chỉ giữ bit `[63:11]`).
pub const VECTOR_ALIGN: u64 = 0x800;

/// Một trong 16 ô của bảng vector, theo đúng thứ tự phần cứng dùng.
///
/// Thứ tự khai báo = thứ tự offset (`0x000`, `0x080`, …, `0x780`); [`Self::ALL`] chốt điều đó
/// bằng test host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VectorKind {
    /// `0x000` — đồng bộ, Current EL with SP0.
    SyncCurrentElSp0,
    /// `0x080` — IRQ, Current EL with SP0.
    IrqCurrentElSp0,
    /// `0x100` — FIQ, Current EL with SP0.
    FiqCurrentElSp0,
    /// `0x180` — SError, Current EL with SP0.
    SErrorCurrentElSp0,
    /// `0x200` — đồng bộ, Current EL with SPx (**`brk #0` từ EL1h đi vào ô này**).
    SyncCurrentElSpX,
    /// `0x280` — IRQ, Current EL with SPx.
    IrqCurrentElSpX,
    /// `0x300` — FIQ, Current EL with SPx.
    FiqCurrentElSpX,
    /// `0x380` — SError, Current EL with SPx.
    SErrorCurrentElSpX,
    /// `0x400` — đồng bộ, Lower EL dùng AArch64.
    SyncLowerElAarch64,
    /// `0x480` — IRQ, Lower EL dùng AArch64.
    IrqLowerElAarch64,
    /// `0x500` — FIQ, Lower EL dùng AArch64.
    FiqLowerElAarch64,
    /// `0x580` — SError, Lower EL dùng AArch64.
    SErrorLowerElAarch64,
    /// `0x600` — đồng bộ, Lower EL dùng AArch32.
    SyncLowerElAarch32,
    /// `0x680` — IRQ, Lower EL dùng AArch32.
    IrqLowerElAarch32,
    /// `0x700` — FIQ, Lower EL dùng AArch32.
    FiqLowerElAarch32,
    /// `0x780` — SError, Lower EL dùng AArch32.
    SErrorLowerElAarch32,
}

impl VectorKind {
    /// Cả 16 ô theo đúng thứ tự offset — nguồn duy nhất cho vòng lặp kiểm.
    pub const ALL: [Self; VECTOR_COUNT] = [
        Self::SyncCurrentElSp0,
        Self::IrqCurrentElSp0,
        Self::FiqCurrentElSp0,
        Self::SErrorCurrentElSp0,
        Self::SyncCurrentElSpX,
        Self::IrqCurrentElSpX,
        Self::FiqCurrentElSpX,
        Self::SErrorCurrentElSpX,
        Self::SyncLowerElAarch64,
        Self::IrqLowerElAarch64,
        Self::FiqLowerElAarch64,
        Self::SErrorLowerElAarch64,
        Self::SyncLowerElAarch32,
        Self::IrqLowerElAarch32,
        Self::FiqLowerElAarch32,
        Self::SErrorLowerElAarch32,
    ];

    /// Chỉ số ô (`0..16`) — cũng là bội số của [`VECTOR_STRIDE`] trong offset.
    pub const fn index(self) -> usize {
        self as usize
    }

    /// Offset tuyệt đối từ `VBAR_EL1` (`index * 0x80`).
    pub const fn offset(self) -> u64 {
        self.index() as u64 * VECTOR_STRIDE
    }

    /// Tên ngắn để in log (D2-102: in từ dữ liệu, không chép chuỗi tại chỗ gọi).
    pub const fn name(self) -> &'static str {
        match self {
            Self::SyncCurrentElSp0 => "sync-cur-sp0",
            Self::IrqCurrentElSp0 => "irq-cur-sp0",
            Self::FiqCurrentElSp0 => "fiq-cur-sp0",
            Self::SErrorCurrentElSp0 => "serror-cur-sp0",
            Self::SyncCurrentElSpX => "sync-cur-spx",
            Self::IrqCurrentElSpX => "irq-cur-spx",
            Self::FiqCurrentElSpX => "fiq-cur-spx",
            Self::SErrorCurrentElSpX => "serror-cur-spx",
            Self::SyncLowerElAarch64 => "sync-low-a64",
            Self::IrqLowerElAarch64 => "irq-low-a64",
            Self::FiqLowerElAarch64 => "fiq-low-a64",
            Self::SErrorLowerElAarch64 => "serror-low-a64",
            Self::SyncLowerElAarch32 => "sync-low-a32",
            Self::IrqLowerElAarch32 => "irq-low-a32",
            Self::FiqLowerElAarch32 => "fiq-low-a32",
            Self::SErrorLowerElAarch32 => "serror-low-a32",
        }
    }

    /// Bóc chỉ số ô (`0..16`) — `None` ngoài dải (không panic).
    pub const fn from_index(index: usize) -> Option<Self> {
        if index < VECTOR_COUNT {
            Some(Self::ALL[index])
        } else {
            None
        }
    }
}

/// Offset tuyệt đối của ô thứ `index`; `None` nếu `index >= 16`.
///
/// Đây là hàm mà asm của ảnh dùng (qua `const` operand) và test host khẳng định **từng giá trị
/// tuyệt đối** — một offset lệch `0x80` làm test ĐỎ.
pub const fn vector_offset(index: usize) -> Option<u64> {
    if index < VECTOR_COUNT {
        Some(index as u64 * VECTOR_STRIDE)
    } else {
        None
    }
}

/// Cài bảng vector: `VBAR_EL1 ← base`, `ISB`, rồi **đọc lại** `VBAR_EL1`.
///
/// Trả giá trị **đọc lại** (đã qua [`Vbar`] cắt bit thấp) để caller in bằng chứng "thanh ghi
/// thật sự giữ địa chỉ này", không phải "ta vừa ghi số này".
///
/// `base` phải căn [`VECTOR_ALIGN`]; hàm **không** tự sửa căn — `Vbar::new` cắt bit thấp nên
/// địa chỉ lệch sẽ **im lặng** trỏ vào chỗ khác. Caller (ảnh) lấy `base` từ symbol linker
/// `__vector_base` đã căn `0x800` bằng `.p2align 11` + `ALIGN(0x800)`.
pub fn install(base: u64) -> u64 {
    asm::write_vbar_el1(Vbar::new(base).bits());
    // `VBAR_EL1` là thanh ghi điều khiển: cần context synchronization trước khi exception kế
    // tiếp dùng giá trị mới (DDI 0487 §D1.10.4 + yêu cầu `ISB` của đặc tả).
    asm::isb();
    Vbar::new(asm::read_vbar_el1()).bits()
}

/// `true` ⇔ `base` căn đúng [`VECTOR_ALIGN`] (dùng để fail-loud trước khi cài).
pub const fn is_aligned(base: u64) -> bool {
    base & (VECTOR_ALIGN - 1) == 0
}
