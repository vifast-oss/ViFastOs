// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho bảng vector AArch64 — KT-248 (K4' bước 3, B3.2).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64
//! ```
//!
//! Kỳ vọng là **offset tuyệt đối viết tay** từ Arm Architecture Reference Manual (DDI 0487,
//! `VBAR_EL1` — bảng vector 16 ô × `0x80`, căn `0x800`), **không** lấy hằng của crate để so:
//!
//! ```text
//!   0x000  Synchronous  Current EL with SP0        0x400  Synchronous  Lower EL using AArch64
//!   0x080  IRQ          Current EL with SP0        0x480  IRQ          Lower EL using AArch64
//!   0x100  FIQ          Current EL with SP0        0x500  FIQ          Lower EL using AArch64
//!   0x180  SError       Current EL with SP0        0x580  SError       Lower EL using AArch64
//!   0x200  Synchronous  Current EL with SPx        0x600  Synchronous  Lower EL using AArch32
//!   0x280  IRQ          Current EL with SPx        0x680  IRQ          Lower EL using AArch32
//!   0x300  FIQ          Current EL with SPx        0x700  FIQ          Lower EL using AArch32
//!   0x380  SError       Current EL with SPx        0x780  SError       Lower EL using AArch32
//! ```
//!
//! Đây là ca kiểm **đỏ trước** của KT-248: viết trước khi `vector` tồn tại
//! (log: `v2/agent/JOURNAL/kt248-red.log`). **Đột biến bắt buộc**: đặt sai một offset (ô thứ 3
//! lệch `0x80`) ⇒ test này **ĐỎ** — log ở `v2/agent/JOURNAL/kt248-mutation.log`.

use vifast_plat_aarch64::sysreg::{Esr, ExceptionClass, Vbar, VBAR_BASE_MASK, VBAR_BASE_SHIFT};
use vifast_plat_aarch64::vector::{
    vector_offset, VectorKind, VECTOR_ALIGN, VECTOR_COUNT, VECTOR_STRIDE,
};

/// Bảng offset tuyệt đối của 16 ô — viết tay từ DDI 0487 (không sinh từ `vector_offset`).
const EXPECTED_OFFSETS: [u64; 16] = [
    0x000, 0x080, 0x100, 0x180, 0x200, 0x280, 0x300, 0x380, 0x400, 0x480, 0x500, 0x580, 0x600,
    0x680, 0x700, 0x780,
];

/// Hình học bảng vector: 16 ô, mỗi ô `0x80`, cả bảng căn `0x800` (DDI 0487 §D1.10.4).
#[test]
fn vector_geometry() {
    assert_eq!(VECTOR_COUNT, 16);
    assert_eq!(VECTOR_STRIDE, 0x80);
    assert_eq!(VECTOR_ALIGN, 0x800);
    assert_eq!(
        VECTOR_COUNT as u64 * VECTOR_STRIDE,
        VECTOR_ALIGN,
        "16 ô × 0x80 phải vừa đúng một khối căn 0x800"
    );
}

/// **Offset tuyệt đối** từng ô — sai một ô là nhảy vào ô khác khi có exception.
#[test]
fn vector_offsets_are_absolute() {
    for (i, expected) in EXPECTED_OFFSETS.iter().enumerate() {
        assert_eq!(vector_offset(i), Some(*expected), "ô thứ {i}");
    }
}

/// `vector_offset` ngoài dải trả `None` (không panic, không trả offset rác).
#[test]
fn vector_offset_out_of_range() {
    assert_eq!(vector_offset(16), None);
    assert_eq!(vector_offset(usize::MAX), None);
}

/// 16 loại exception theo đúng thứ tự bảng: 4 nhóm × (sync, irq, fiq, serror).
#[test]
fn vector_kinds_in_order() {
    let expected = [
        VectorKind::SyncCurrentElSp0,
        VectorKind::IrqCurrentElSp0,
        VectorKind::FiqCurrentElSp0,
        VectorKind::SErrorCurrentElSp0,
        VectorKind::SyncCurrentElSpX,
        VectorKind::IrqCurrentElSpX,
        VectorKind::FiqCurrentElSpX,
        VectorKind::SErrorCurrentElSpX,
        VectorKind::SyncLowerElAarch64,
        VectorKind::IrqLowerElAarch64,
        VectorKind::FiqLowerElAarch64,
        VectorKind::SErrorLowerElAarch64,
        VectorKind::SyncLowerElAarch32,
        VectorKind::IrqLowerElAarch32,
        VectorKind::FiqLowerElAarch32,
        VectorKind::SErrorLowerElAarch32,
    ];
    assert_eq!(VectorKind::ALL.len(), 16);
    for (i, kind) in expected.iter().enumerate() {
        assert_eq!(VectorKind::ALL[i], *kind, "ô thứ {i}");
        assert_eq!(kind.index(), i, "index của ô thứ {i}");
        assert_eq!(kind.offset(), EXPECTED_OFFSETS[i], "offset của ô thứ {i}");
    }
}

/// `brk #0` ở EL1h đi vào ô **Current EL with SPx / Synchronous** = `0x200` (DDI 0487 §D1.10.4:
/// exception từ EL1 dùng `SP_EL1` ⇒ nhóm "Current EL with SPx").
#[test]
fn brk_from_el1_uses_spx_sync_slot() {
    assert_eq!(VectorKind::SyncCurrentElSpX.offset(), 0x200);
    assert_eq!(VectorKind::SyncCurrentElSpX.index(), 4);
}

/// `VBAR_EL1` chỉ giữ bit `[63:11]` — địa chỉ bảng phải căn `0x800`; `Vbar` **cắt** bit thấp
/// thay vì giữ giá trị sai (nếu không, `msr vbar_el1` giữ rác và mọi exception nhảy sai).
#[test]
fn vbar_masks_low_bits() {
    assert_eq!(VBAR_BASE_SHIFT, 11);
    assert_eq!(VBAR_BASE_MASK, !0x7FFu64);
    assert_eq!(Vbar::new(0x4008_0000).bits(), 0x4008_0000);
    assert_eq!(
        Vbar::new(0x4008_07FF).bits(),
        0x4008_0000,
        "bit thấp bị cắt"
    );
    assert_eq!(Vbar::new(0x4008_0800).bits(), 0x4008_0800);
    assert_eq!(Vbar::new(0x4008_0FFF).bits(), 0x4008_0800);
    // Căn của bảng vector thoả yêu cầu của VBAR: `VECTOR_ALIGN & VBAR_BASE_MASK == VECTOR_ALIGN`.
    assert_eq!(VECTOR_ALIGN & VBAR_BASE_MASK, VECTOR_ALIGN);
}

/// `EC` của `BRK` là **0x3C** ("BRK instruction execution in AArch64 state", DDI 0487 §D17.2.37)
/// — **không** phải `0x30`/`0x31` (hai mã đó là *breakpoint* phần cứng). Thiếu mã này thì handler
/// đồng bộ không nhận ra `brk #0` và boot dừng ở "EC lạ".
#[test]
fn brk_exception_class() {
    assert_eq!(ExceptionClass::Brk64.bits(), 0x3C);
    assert_eq!(ExceptionClass::from_bits(0x3C), Some(ExceptionClass::Brk64));
    assert_ne!(
        ExceptionClass::from_bits(0x3C),
        Some(ExceptionClass::BreakpointSameEl),
        "0x31 là breakpoint phần cứng, không phải BRK"
    );
}

/// `Esr::from_bits` bóc lại đúng giá trị thô mà asm đọc từ `ESR_EL1` (EC/IL/ISS) — handler dùng
/// đường này thay vì tự dịch bit tại chỗ.
#[test]
fn esr_from_bits_round_trip() {
    let raw = Esr::new(ExceptionClass::Brk64, true, 0x1234).bits();
    let esr = Esr::from_bits(raw);
    assert_eq!(esr.bits(), raw);
    assert_eq!(esr.ec(), Some(ExceptionClass::Brk64));
    assert_eq!(esr.ec_raw(), 0x3C);
    assert!(esr.il(), "BRK là instruction 32 bit ⇒ IL = 1");
    assert_eq!(esr.iss(), 0x1234);
    // Ca âm: EC lạ vẫn bóc được `ec_raw`, `ec()` trả `None` (không panic).
    let unknown = Esr::from_bits(0x3F << 26);
    assert_eq!(unknown.ec_raw(), 0x3F);
    assert_eq!(unknown.ec(), None);
}
