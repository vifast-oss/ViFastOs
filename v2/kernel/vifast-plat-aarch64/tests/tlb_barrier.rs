// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho `TLBI` + barrier AArch64 — KT-167 (K4' bước 2).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64
//! ```
//!
//! Kỳ vọng là **giá trị tuyệt đối viết tay**, đối chiếu bằng hợp dịch thật
//! (`clang --target=aarch64-none-elf` + `llvm-objdump`) — xem bảng trong `src/tlb.rs` và
//! `src/barrier.rs`. Một opcode `TLBI vae1` sai hay một barrier sai mã đều làm test **ĐỎ**.

use vifast_plat_aarch64::barrier::{
    DMB_ISH, DMB_ISHLD, DMB_ISHST, DMB_NSH, DMB_OSH, DMB_SY, DSB_ISH, DSB_ISHLD, DSB_ISHST,
    DSB_NSH, DSB_OSH, DSB_SY, ISB,
};
use vifast_plat_aarch64::tlb::{
    tlbi_encoding, tlbi_va_arg, TLBI_ASIDE1, TLBI_ASIDE1IS, TLBI_SYS_BASE, TLBI_VAE1, TLBI_VAE1IS,
    TLBI_VMALLE1, TLBI_VMALLE1IS,
};

// ---------------------------------------------------------------- TLBI

/// Mã thao tác `TLBI` — giá trị tuyệt đối của `llvm-objdump`:
/// `vmalle1 d508871f`, `vmalle1is d508831f`, `vae1 d5088720`, `vae1is d5088320`,
/// `aside1 d5088740`, `aside1is d5088340`.
#[test]
fn tlbi_encodings() {
    assert_eq!(TLBI_SYS_BASE, 0xD508_8000);
    assert_eq!(TLBI_VMALLE1, 0xD508_871F);
    assert_eq!(TLBI_VMALLE1IS, 0xD508_831F);
    assert_eq!(TLBI_VAE1, 0xD508_8720);
    assert_eq!(TLBI_VAE1IS, 0xD508_8320);
    assert_eq!(TLBI_ASIDE1, 0xD508_8740);
    assert_eq!(TLBI_ASIDE1IS, 0xD508_8340);
    // Công thức ghép: base | CRm<<8 | op2<<5 | Rt.
    assert_eq!(tlbi_encoding(7, 0, 31), 0xD508_871F);
    assert_eq!(tlbi_encoding(3, 0, 31), 0xD508_831F);
    assert_eq!(tlbi_encoding(7, 1, 0), 0xD508_8720);
    assert_eq!(tlbi_encoding(3, 1, 0), 0xD508_8320);
    assert_eq!(tlbi_encoding(7, 2, 0), 0xD508_8740);
    assert_eq!(tlbi_encoding(3, 2, 0), 0xD508_8340);
    // Trường bị cắt đúng số bit: CRm 4 bit, op2 3 bit, Rt 5 bit.
    assert_eq!(tlbi_encoding(0x1F, 0x9, 0x3F), 0xD508_8F3F);
    // `vmalle1`/`vmalle1is` bắt buộc Rt = 31 (XZR) — encoding đầy đủ, không có tham số.
    assert_eq!(TLBI_VMALLE1 & 0x1F, 31);
    assert_eq!(TLBI_VMALLE1IS & 0x1F, 31);
    // `vae1`/`aside1` dùng Rt = 0 (asm đặt tham số vào x0).
    assert_eq!(TLBI_VAE1 & 0x1F, 0);
    assert_eq!(TLBI_ASIDE1 & 0x1F, 0);
}

/// Tham số `VA[55:12]` của `TLBI VAE1*` — `va >> 12`, tuyệt đối.
#[test]
fn tlbi_va_arg_values() {
    assert_eq!(tlbi_va_arg(0x0000_0000_0000_1000), 1);
    assert_eq!(tlbi_va_arg(0x0000_0000_0040_0000), 0x400);
    assert_eq!(tlbi_va_arg(0xFFFF_FFFF_FFFF_F000), 0x000F_FFFF_FFFF_FFFF);
    assert_eq!(tlbi_va_arg(0), 0);
}

// ---------------------------------------------------------------- barrier

/// Mã barrier — giá trị tuyệt đối của `llvm-objdump`.
#[test]
fn barrier_encodings() {
    assert_eq!(DSB_SY, 0xD503_3F9F);
    assert_eq!(DSB_ISH, 0xD503_3B9F);
    assert_eq!(DSB_ISHST, 0xD503_3A9F);
    assert_eq!(DSB_ISHLD, 0xD503_399F);
    assert_eq!(DSB_NSH, 0xD503_379F);
    assert_eq!(DSB_OSH, 0xD503_339F);
    assert_eq!(DMB_SY, 0xD503_3FBF);
    assert_eq!(DMB_ISH, 0xD503_3BBF);
    assert_eq!(DMB_ISHST, 0xD503_3ABF);
    assert_eq!(DMB_ISHLD, 0xD503_39BF);
    assert_eq!(DMB_NSH, 0xD503_37BF);
    assert_eq!(DMB_OSH, 0xD503_33BF);
    assert_eq!(ISB, 0xD503_3FDF);
    // Quan hệ cấu trúc: DSB/DMB khác nhau ở bit 5; ISB khác DSB ở bit 6.
    assert_eq!(DSB_ISH ^ DMB_ISH, 1 << 5);
    assert_eq!(DSB_SY ^ ISB, 1 << 6);
}

// ---------------------------------------------------------------- stub host

/// Stub host phải **panic**, không được là no-op im lặng: gọi asm đặc quyền trên host là lỗi
/// lập trình, và một no-op sẽ khiến test "xanh" trong khi chẳng có barrier/TLBI nào chạy.
#[test]
fn host_stub_is_fail_loud() {
    let barrier = std::panic::catch_unwind(vifast_plat_aarch64::barrier::isb);
    assert!(barrier.is_err(), "stub host của `isb` phải panic");
    let tlb = std::panic::catch_unwind(vifast_plat_aarch64::tlb::vmalle1);
    assert!(tlb.is_err(), "stub host của `tlbi vmalle1` phải panic");
    let sysreg = std::panic::catch_unwind(vifast_plat_aarch64::asm::read_tcr_el1);
    assert!(sysreg.is_err(), "stub host của `mrs tcr_el1` phải panic");
}
