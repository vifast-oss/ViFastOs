// SPDX-License-Identifier: MIT OR Apache-2.0
//! Barrier bộ nhớ AArch64 — `dsb`/`dmb`/`isb` (KT-167 / K4' bước 2).
//!
//! Tệp này giữ **encoding** của từng barrier dưới dạng hằng **host-testable** (test
//! `tests/tlb_barrier.rs` so giá trị tuyệt đối viết tay) và các hàm asm mỏng gọi
//! [`crate::asm`] — nơi duy nhất có `unsafe` (D2-17).
//!
//! # Encoding
//!
//! Đối chiếu bằng **hợp dịch thật** (`clang --target=aarch64-none-elf` + `llvm-objdump`):
//!
//! ```text
//!   dsb sy      d5033f9f      dmb sy      d5033fbf
//!   dsb ish     d5033b9f      dmb ish     d5033bbf
//!   dsb ishst   d5033a9f      dmb ishst   d5033abf
//!   dsb ishld   d503399f      dmb ishld   d50339bf
//!   dsb nsh     d503379f      dmb nsh     d50337bf
//!   dsb osh     d503339f      dmb osh     d50333bf
//!   isb         d5033fdf
//! ```
//!
//! # Dùng đúng chỗ
//!
//! - `dsb ish` **trước** `TLBI`/sau khi sửa bảng trang: chờ mọi truy cập bộ nhớ đã phát ra
//!   hoàn tất trước khi xoá TLB;
//! - `isb` **sau** `msr` thanh ghi hệ thống (`TCR_EL1`, `SCTLR_EL1`, `TTBR*`, `VBAR_EL1`):
//!   bảo đảm instruction kế tiếp thấy cấu hình mới (context synchronization event);
//! - `dmb ish` giữa ghi dữ liệu và ghi cờ chia sẻ giữa các PE.

/// `DSB SY` — chờ **mọi** truy cập bộ nhớ hoàn tất (toàn hệ thống).
pub const DSB_SY: u32 = 0xD503_3F9F;
/// `DSB ISH` — chờ truy cập bộ nhớ hoàn tất trong inner-shareable domain.
pub const DSB_ISH: u32 = 0xD503_3B9F;
/// `DSB ISHST` — như `DSB ISH`, chỉ chờ store.
pub const DSB_ISHST: u32 = 0xD503_3A9F;
/// `DSB ISHLD` — như `DSB ISH`, chỉ chờ load.
pub const DSB_ISHLD: u32 = 0xD503_399F;
/// `DSB NSH` — chờ trong non-shareable domain.
pub const DSB_NSH: u32 = 0xD503_379F;
/// `DSB OSH` — chờ trong outer-shareable domain.
pub const DSB_OSH: u32 = 0xD503_339F;
/// `DMB SY` — hàng rào thứ tự **mọi** truy cập bộ nhớ (toàn hệ thống).
pub const DMB_SY: u32 = 0xD503_3FBF;
/// `DMB ISH` — hàng rào thứ tự trong inner-shareable domain.
pub const DMB_ISH: u32 = 0xD503_3BBF;
/// `DMB ISHST` — như `DMB ISH`, chỉ store.
pub const DMB_ISHST: u32 = 0xD503_3ABF;
/// `DMB ISHLD` — như `DMB ISH`, chỉ load.
pub const DMB_ISHLD: u32 = 0xD503_39BF;
/// `DMB NSH` — hàng rào trong non-shareable domain.
pub const DMB_NSH: u32 = 0xD503_37BF;
/// `DMB OSH` — hàng rào trong outer-shareable domain.
pub const DMB_OSH: u32 = 0xD503_33BF;
/// `ISB` — context synchronization event (sau `msr` thanh ghi điều khiển).
pub const ISB: u32 = 0xD503_3FDF;

/// `dsb sy`.
pub fn dsb_sy() {
    crate::asm::dsb_sy();
}

/// `dsb ish` — dùng trước `TLBI` và sau khi sửa bảng trang.
pub fn dsb_ish() {
    crate::asm::dsb_ish();
}

/// `dmb sy`.
pub fn dmb_sy() {
    crate::asm::dmb_sy();
}

/// `dmb ish` — hàng rào dữ liệu giữa các PE inner-shareable.
pub fn dmb_ish() {
    crate::asm::dmb_ish();
}

/// `isb` — bắt buộc sau khi ghi thanh ghi hệ thống điều khiển MMU.
pub fn isb() {
    crate::asm::isb();
}
