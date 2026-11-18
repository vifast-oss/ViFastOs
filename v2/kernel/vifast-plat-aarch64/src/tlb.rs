// SPDX-License-Identifier: MIT OR Apache-2.0
//! `TLBI` — xoá TLB theo địa chỉ/ASID (KT-167 / K4' bước 2).
//!
//! Tệp này giữ **mã thao tác** dưới dạng hằng encoding **host-testable** (test
//! `tests/tlb_barrier.rs` so giá trị tuyệt đối viết tay) và các hàm asm mỏng gọi
//! [`crate::asm`] — nơi duy nhất có `unsafe` (D2-17).
//!
//! # Encoding
//!
//! `TLBI` là system instruction: `SYS #0, C8, C<CRm>, #<op2>, Xt`. Công thức:
//!
//! ```text
//!   0xD508_8000 | (CRm << 8) | (op2 << 5) | Rt
//! ```
//!
//! Bảng dưới đây đối chiếu bằng **hợp dịch thật** (`clang --target=aarch64-none-elf` +
//! `llvm-objdump`), không chép từ trí nhớ:
//!
//! ```text
//!   tlbi vmalle1        d508871f   (CRm=7, op2=0, Rt=31)
//!   tlbi vmalle1is      d508831f   (CRm=3, op2=0, Rt=31)
//!   tlbi vae1, x0       d5088720   (CRm=7, op2=1, Rt=0)
//!   tlbi vae1is, x0     d5088320   (CRm=3, op2=1, Rt=0)
//!   tlbi aside1, x0     d5088740   (CRm=7, op2=2, Rt=0)
//!   tlbi aside1is, x0   d5088340   (CRm=3, op2=2, Rt=0)
//! ```
//!
//! # Vì sao `vmalle1`/`vmalle1is` không có tham số
//!
//! Hai lệnh này xoá **toàn bộ** TLB của EL1&0 (không theo VA/ASID) nên `Rt` **bắt buộc** là
//! `31` (`XZR`); dùng thanh ghi khác là encoding không hợp lệ. Vì vậy chúng là hằng số đầy đủ,
//! còn `vae1`/`aside1` là `base | Rt` với `Rt = 0` (asm đặt tham số vào `x0`).

/// Base encoding của `TLBI`: `SYS #0, C8, C<CRm>, #<op2>, Xt`.
pub const TLBI_SYS_BASE: u32 = 0xD508_8000;

/// Ghép encoding `TLBI` từ `CRm`, `op2`, `Rt` (mỗi trường bị cắt đúng số bit của đặc tả).
pub const fn tlbi_encoding(crm: u32, op2: u32, rt: u32) -> u32 {
    TLBI_SYS_BASE | ((crm & 0xF) << 8) | ((op2 & 0x7) << 5) | (rt & 0x1F)
}

/// `TLBI VMALLE1` — xoá toàn bộ TLB EL1&0 của PE hiện tại (không broadcast).
pub const TLBI_VMALLE1: u32 = tlbi_encoding(7, 0, 31);
/// `TLBI VMALLE1IS` — như trên, broadcast tới mọi PE trong inner-shareable domain.
pub const TLBI_VMALLE1IS: u32 = tlbi_encoding(3, 0, 31);
/// `TLBI VAE1` — xoá entry của một VA (mọi ASID) ở EL1&0; tham số ở `x0`.
pub const TLBI_VAE1: u32 = tlbi_encoding(7, 1, 0);
/// `TLBI VAE1IS` — như `VAE1`, broadcast inner-shareable.
pub const TLBI_VAE1IS: u32 = tlbi_encoding(3, 1, 0);
/// `TLBI ASIDE1` — xoá entry của một ASID (mọi VA) ở EL1&0; tham số ở `x0`.
pub const TLBI_ASIDE1: u32 = tlbi_encoding(7, 2, 0);
/// `TLBI ASIDE1IS` — như `ASIDE1`, broadcast inner-shareable.
pub const TLBI_ASIDE1IS: u32 = tlbi_encoding(3, 2, 0);

/// Tham số `Xt` của `TLBI VAE1*`: `VA[55:12]` nằm ở bit `[43:0]` của thanh ghi.
///
/// Với VA 39-bit canonical (D2-147), `va >> 12` luôn vừa 44 bit.
pub const fn tlbi_va_arg(va: u64) -> u64 {
    va >> 12
}

/// Xoá toàn bộ TLB EL1&0 của PE hiện tại.
pub fn vmalle1() {
    crate::asm::tlbi_vmalle1();
}

/// Xoá toàn bộ TLB EL1&0, broadcast inner-shareable.
pub fn vmalle1is() {
    crate::asm::tlbi_vmalle1is();
}

/// Xoá entry TLB của `va` (mọi ASID) — gọi sau khi sửa/xoá lá bảng trang.
pub fn vae1(va: u64) {
    crate::asm::tlbi_vae1(tlbi_va_arg(va));
}

/// Xoá entry TLB của `va`, broadcast inner-shareable.
pub fn vae1is(va: u64) {
    crate::asm::tlbi_vae1is(tlbi_va_arg(va));
}

/// Xoá mọi entry TLB của `asid` (dùng khi huỷ cả VSpace).
pub fn aside1(asid: u16) {
    crate::asm::tlbi_aside1(asid as u64);
}

/// Xoá mọi entry TLB của `asid`, broadcast inner-shareable.
pub fn aside1is(asid: u16) {
    crate::asm::tlbi_aside1is(asid as u64);
}
