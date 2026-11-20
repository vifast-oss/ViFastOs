// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho encoding thanh ghi hệ thống AArch64 — KT-167 (K4' bước 2).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64
//! ```
//!
//! Mọi kỳ vọng dưới đây là **giá trị tuyệt đối viết tay** từ Arm Architecture Registers
//! (DDI 0601, bản 2024-12) — **không** lấy hằng của chính crate để so (yêu cầu KT-167). Nhờ vậy
//! một shift sai hay một slot `MAIR` sai thứ tự đều làm test **ĐỎ** (xem log đột biến ở
//! `v2/agent/JOURNAL/kt167-logs/`).

use vifast_plat_aarch64::sysreg::{
    Daif, ElrEl1, Esr, ExceptionClass, FarEl1, Granule, Ips, Mair, Sctlr, Spsr, SpsrMode, Tcr,
    TcrCache, Ttbr, Vbar, CPACR_EL1_FP_SIMD, DAIF_A, DAIF_ALL, DAIF_D, DAIF_F, DAIF_I,
    MAIR_ATTR_DEVICE_NGNRE, MAIR_ATTR_DEVICE_NGNRNE, MAIR_ATTR_NORMAL_NC, MAIR_ATTR_NORMAL_WB,
    MAIR_ATTR_NORMAL_WT, MAIR_EL1_KERNEL, SCTLR_A, SCTLR_ATA, SCTLR_BT0, SCTLR_BT1, SCTLR_C,
    SCTLR_CP15BEN, SCTLR_DSSBS, SCTLR_DZE, SCTLR_E0E, SCTLR_EE, SCTLR_EIS, SCTLR_ENALS,
    SCTLR_ENAS0, SCTLR_ENASR, SCTLR_ENDA, SCTLR_ENDB, SCTLR_ENIA, SCTLR_ENIB, SCTLR_ENTP2,
    SCTLR_EOS, SCTLR_EPAN, SCTLR_I, SCTLR_IESB, SCTLR_ITD, SCTLR_ITFSB, SCTLR_LSMAOE, SCTLR_M,
    SCTLR_NTLSMD, SCTLR_NTWE, SCTLR_NTWI, SCTLR_SA, SCTLR_SA0, SCTLR_SED, SCTLR_SPAN, SCTLR_TSCXT,
    SCTLR_UCI, SCTLR_UCT, SCTLR_UMA, SCTLR_WXN, SPSR_C, SPSR_M_MASK, SPSR_M_SHIFT, SPSR_N, SPSR_V,
    SPSR_Z, TCR_A1, TCR_AS, TCR_EL1_KERNEL, TCR_EPD0, TCR_EPD1, TCR_IPS_MASK, TCR_IPS_SHIFT,
    TCR_IRGN0_MASK, TCR_IRGN0_SHIFT, TCR_IRGN1_MASK, TCR_IRGN1_SHIFT, TCR_ORGN0_MASK,
    TCR_ORGN0_SHIFT, TCR_ORGN1_MASK, TCR_ORGN1_SHIFT, TCR_SH0_MASK, TCR_SH0_SHIFT, TCR_SH1_MASK,
    TCR_SH1_SHIFT, TCR_T0SZ_MASK, TCR_T0SZ_SHIFT, TCR_T1SZ_MASK, TCR_T1SZ_SHIFT, TCR_TBI0,
    TCR_TBI1, TCR_TG0_MASK, TCR_TG0_SHIFT, TCR_TG1_MASK, TCR_TG1_SHIFT, TTBR_ASID_SHIFT,
    TTBR_BADDR_MASK, TTBR_BADDR_SHIFT, TTBR_CNP, VBAR_BASE_MASK, VBAR_BASE_SHIFT,
};
use vifast_plat_aarch64::Shareability;

// ---------------------------------------------------------------- TCR_EL1

/// `TCR_EL1` mục tiêu của kernel: 39-bit VA, 48-bit PA, 4 KiB granule, WBWA, inner-shareable,
/// ASID 16 bit. Giá trị tuyệt đối tính tay:
/// `25 | 1<<8 | 1<<10 | 3<<12 | 25<<16 | 1<<24 | 1<<26 | 3<<28 | 2<<30 | 5<<32 | 1<<36`.
#[test]
fn tcr_kernel_value() {
    assert_eq!(TCR_EL1_KERNEL.bits(), 0x0000_0015_B519_3519);
    assert_eq!(TCR_EL1_KERNEL.t0sz(), 25, "64 - 39 bit VA");
    assert_eq!(TCR_EL1_KERNEL.t1sz(), 25);
    assert_eq!(TCR_EL1_KERNEL.tg0(), Some(Granule::K4));
    assert_eq!(TCR_EL1_KERNEL.tg1(), Some(Granule::K4));
    assert_eq!(TCR_EL1_KERNEL.ips(), Some(Ips::Bits48));
    assert_eq!(TCR_EL1_KERNEL.ips().unwrap().pa_bits(), 48);
    assert!(!TCR_EL1_KERNEL.epd0(), "TTBR0 bật");
    assert!(!TCR_EL1_KERNEL.epd1(), "TTBR1 bật");
    assert!(TCR_EL1_KERNEL.asid16(), "ASID 16 bit");
    assert!(!TCR_EL1_KERNEL.a1(), "ASID ở TTBR0");
    assert!(!TCR_EL1_KERNEL.tbi0());
    assert!(!TCR_EL1_KERNEL.tbi1());
    assert_eq!(TCR_EL1_KERNEL.sh0(), Some(Shareability::Inner));
    assert_eq!(TCR_EL1_KERNEL.sh1(), Some(Shareability::Inner));
    assert_eq!(TCR_EL1_KERNEL.irgn0(), TcrCache::WriteBackWriteAllocate);
    assert_eq!(TCR_EL1_KERNEL.orgn0(), TcrCache::WriteBackWriteAllocate);
    assert_eq!(TCR_EL1_KERNEL.irgn1(), TcrCache::WriteBackWriteAllocate);
    assert_eq!(TCR_EL1_KERNEL.orgn1(), TcrCache::WriteBackWriteAllocate);
}

/// Từng trường nằm đúng **vị trí bit tuyệt đối** của DDI 0601: T0SZ[5:0], EPD0[7], IRGN0[9:8],
/// ORGN0[11:10], SH0[13:12], TG0[15:14], T1SZ[21:16], A1[22], EPD1[23], IRGN1[25:24],
/// ORGN1[27:26], SH1[29:28], TG1[31:30], IPS[34:32], AS[36], TBI0[37], TBI1[38].
#[test]
fn tcr_field_positions() {
    let tcr = Tcr::new(0x2A, Granule::K16, 0x15, Granule::K64, Ips::Bits44)
        .with_epd0(true)
        .with_epd1(true)
        .with_a1(true)
        .with_asid16(true)
        .with_tbi(true, true)
        .with_shareability(Shareability::Outer, Shareability::Inner)
        .with_cacheability(
            TcrCache::WriteThrough,
            TcrCache::WriteBackNoWriteAllocate,
            TcrCache::WriteBackWriteAllocate,
            TcrCache::WriteThrough,
        );
    // Tính tay: 0x2A | 0x80 | 0x200 | 0xC00 | 0x2000 | 0x8000 | 0x15_0000 | 0x40_0000 |
    // 0x80_0000 | 0x0100_0000 | 0x0800_0000 | 0x3000_0000 | 0xC000_0000 |
    // 0x4_0000_0000 | 0x10_0000_0000 | 0x20_0000_0000 | 0x40_0000_0000.
    assert_eq!(tcr.bits(), 0x0000_0074_F9D5_AEAA);
    assert_eq!(tcr.t0sz(), 0x2A);
    assert_eq!(tcr.t1sz(), 0x15);
    assert_eq!(tcr.tg0(), Some(Granule::K16));
    assert_eq!(tcr.tg1(), Some(Granule::K64));
    assert_eq!(tcr.ips(), Some(Ips::Bits44));
    assert!(tcr.epd0());
    assert!(tcr.epd1());
    assert!(tcr.a1());
    assert!(tcr.asid16());
    assert!(tcr.tbi0());
    assert!(tcr.tbi1());
    assert_eq!(tcr.sh0(), Some(Shareability::Outer));
    assert_eq!(tcr.sh1(), Some(Shareability::Inner));
    assert_eq!(tcr.irgn0(), TcrCache::WriteThrough);
    assert_eq!(tcr.orgn0(), TcrCache::WriteBackNoWriteAllocate);
    assert_eq!(tcr.irgn1(), TcrCache::WriteBackWriteAllocate);
    assert_eq!(tcr.orgn1(), TcrCache::WriteThrough);
    // Vị trí bit tuyệt đối của từng trường (không phụ thuộc builder).
    assert_eq!(TCR_T0SZ_SHIFT, 0);
    assert_eq!(TCR_EPD0, 0x80);
    assert_eq!(TCR_IRGN0_SHIFT, 8);
    assert_eq!(TCR_ORGN0_SHIFT, 10);
    assert_eq!(TCR_SH0_SHIFT, 12);
    assert_eq!(TCR_TG0_SHIFT, 14);
    assert_eq!(TCR_T1SZ_SHIFT, 16);
    assert_eq!(TCR_A1, 0x40_0000);
    assert_eq!(TCR_EPD1, 0x80_0000);
    assert_eq!(TCR_IRGN1_SHIFT, 24);
    assert_eq!(TCR_ORGN1_SHIFT, 26);
    assert_eq!(TCR_SH1_SHIFT, 28);
    assert_eq!(TCR_TG1_SHIFT, 30);
    assert_eq!(TCR_IPS_SHIFT, 32);
    assert_eq!(TCR_AS, 0x10_0000_0000);
    assert_eq!(TCR_TBI0, 0x20_0000_0000);
    assert_eq!(TCR_TBI1, 0x40_0000_0000);
}

/// `TG0` và `TG1` mã hoá **khác nhau** — bảng tuyệt đối của DDI 0601.
#[test]
fn tcr_tg_encodings() {
    // TG0: 4K=0b00, 64K=0b01, 16K=0b10.
    assert_eq!(Granule::K4.tg0_bits(), 0b00);
    assert_eq!(Granule::K64.tg0_bits(), 0b01);
    assert_eq!(Granule::K16.tg0_bits(), 0b10);
    // TG1: 16K=0b01, 4K=0b10, 64K=0b11.
    assert_eq!(Granule::K16.tg1_bits(), 0b01);
    assert_eq!(Granule::K4.tg1_bits(), 0b10);
    assert_eq!(Granule::K64.tg1_bits(), 0b11);
    // Vị trí bit tuyệt đối trong thanh ghi.
    assert_eq!(
        Tcr::new(25, Granule::K4, 25, Granule::K4, Ips::Bits48).bits() & (0b11 << 14),
        0,
        "TG0 = 4 KiB ở bit 15:14"
    );
    assert_eq!(
        Tcr::new(25, Granule::K16, 25, Granule::K16, Ips::Bits48).bits() & (0b11 << 14),
        0b10 << 14
    );
    assert_eq!(
        Tcr::new(25, Granule::K64, 25, Granule::K64, Ips::Bits48).bits() & (0b11 << 14),
        0b01 << 14
    );
    assert_eq!(
        Tcr::new(25, Granule::K4, 25, Granule::K4, Ips::Bits48).bits() & (0b11 << 30),
        0b10 << 30,
        "TG1 = 4 KiB ở bit 31:30 (khác TG0)"
    );
    assert_eq!(
        Tcr::new(25, Granule::K16, 25, Granule::K16, Ips::Bits48).bits() & (0b11 << 30),
        0b01 << 30
    );
    assert_eq!(
        Tcr::new(25, Granule::K64, 25, Granule::K64, Ips::Bits48).bits() & (0b11 << 30),
        0b11 << 30
    );
    // Bóc ngược: 0b11 ở TG0 và 0b00 ở TG1 là reserved.
    assert_eq!(Granule::from_tg0(0b11), None);
    assert_eq!(Granule::from_tg1(0b00), None);
    assert_eq!(Granule::from_tg0(0b00), Some(Granule::K4));
    assert_eq!(Granule::from_tg1(0b10), Some(Granule::K4));
    assert_eq!(Granule::K4.page_bits(), 12);
    assert_eq!(Granule::K16.page_bits(), 14);
    assert_eq!(Granule::K64.page_bits(), 16);
}

/// `IPS[2:0]` — bảng tuyệt đối (32/36/40/42/44/48/52 bit; `0b111` reserved).
#[test]
fn tcr_ips_encodings() {
    let cases = [
        (Ips::Bits32, 0b000u64, 32u32),
        (Ips::Bits36, 0b001, 36),
        (Ips::Bits40, 0b010, 40),
        (Ips::Bits42, 0b011, 42),
        (Ips::Bits44, 0b100, 44),
        (Ips::Bits48, 0b101, 48),
        (Ips::Bits52, 0b110, 52),
    ];
    for (ips, bits, pa_bits) in cases {
        assert_eq!(ips.bits(), bits, "{ips:?}");
        assert_eq!(ips.pa_bits(), pa_bits, "{ips:?}");
        assert_eq!(
            Tcr::new(25, Granule::K4, 25, Granule::K4, ips).bits() & (0b111 << 32),
            bits << 32,
            "{ips:?} phải nằm ở bit 34:32"
        );
        assert_eq!(Ips::from_bits(bits), Some(ips));
    }
    assert_eq!(Ips::from_bits(0b111), None, "0b111 reserved");
}

/// `T0SZ`/`T1SZ` chỉ 6 bit: giá trị lớn bị cắt, không tràn sang trường khác.
#[test]
fn tcr_sz_masked_to_six_bits() {
    let tcr = Tcr::new(0xFF, Granule::K4, 0x40, Granule::K4, Ips::Bits48);
    assert_eq!(tcr.t0sz(), 0x3F);
    assert_eq!(tcr.t1sz(), 0);
    assert_eq!(tcr.bits() & 0x3F, 0x3F);
    assert_eq!((tcr.bits() >> 16) & 0x3F, 0);
}

/// 11 mặt nạ `TCR_*_MASK` — **giá trị bit tham chiếu viết tay** từ Arm ARM (DDI 0487),
/// bảng `TCR_EL1`: `T0SZ[5:0]`, `IRGN0[9:8]`, `ORGN0[11:10]`, `SH0[13:12]`, `TG0[15:14]`,
/// `T1SZ[21:16]`, `IRGN1[25:24]`, `ORGN1[27:26]`, `SH1[29:28]`, `TG1[31:30]`, `IPS[34:32]`.
///
/// Đây là **API công khai** (`pub use` ở `lib.rs`); bước 3 dùng mask để bóc/ghép trường nên
/// một mask sai phải làm test **ĐỎ** (C1 của KT-175, D2-162). Không so với hằng khác của crate.
#[test]
fn tcr_masks_reference_values() {
    let expected: [(u64, u64, &str); 11] = [
        (TCR_T0SZ_MASK, 0x0000_0000_0000_003F, "T0SZ[5:0]"),
        (TCR_IRGN0_MASK, 0x0000_0000_0000_0300, "IRGN0[9:8]"),
        (TCR_ORGN0_MASK, 0x0000_0000_0000_0C00, "ORGN0[11:10]"),
        (TCR_SH0_MASK, 0x0000_0000_0000_3000, "SH0[13:12]"),
        (TCR_TG0_MASK, 0x0000_0000_0000_C000, "TG0[15:14]"),
        (TCR_T1SZ_MASK, 0x0000_0000_003F_0000, "T1SZ[21:16]"),
        (TCR_IRGN1_MASK, 0x0000_0000_0300_0000, "IRGN1[25:24]"),
        (TCR_ORGN1_MASK, 0x0000_0000_0C00_0000, "ORGN1[27:26]"),
        (TCR_SH1_MASK, 0x0000_0000_3000_0000, "SH1[29:28]"),
        (TCR_TG1_MASK, 0x0000_0000_C000_0000, "TG1[31:30]"),
        (TCR_IPS_MASK, 0x0000_0007_0000_0000, "IPS[34:32]"),
    ];
    for (actual, want, name) in expected {
        assert_eq!(actual, want, "mặt nạ {name} sai");
    }
}

/// Bóc trường từ **giá trị thanh ghi viết tay** bằng chính các mask công khai — chứng minh
/// mask dùng được để đọc đúng trường, không chỉ đúng khi so literal.
#[test]
fn tcr_masks_extract_reference_fields() {
    // Cùng giá trị tuyệt đối mà `tcr_field_positions` dựng từ builder (tính tay từ DDI 0487).
    let raw: u64 = 0x0000_0074_F9D5_AEAA;
    assert_eq!(raw & TCR_T0SZ_MASK, 0x2A, "T0SZ");
    assert_eq!((raw & TCR_IRGN0_MASK) >> 8, 0b10, "IRGN0 = WriteThrough");
    assert_eq!(
        (raw & TCR_ORGN0_MASK) >> 10,
        0b11,
        "ORGN0 = WriteBackNoWriteAllocate"
    );
    assert_eq!((raw & TCR_SH0_MASK) >> 12, 0b10, "SH0 = Outer");
    assert_eq!((raw & TCR_TG0_MASK) >> 14, 0b10, "TG0 = 16 KiB");
    assert_eq!((raw & TCR_T1SZ_MASK) >> 16, 0x15, "T1SZ");
    assert_eq!((raw & TCR_IRGN1_MASK) >> 24, 0b01, "IRGN1 = WBWA");
    assert_eq!((raw & TCR_ORGN1_MASK) >> 26, 0b10, "ORGN1 = WriteThrough");
    assert_eq!((raw & TCR_SH1_MASK) >> 28, 0b11, "SH1 = Inner");
    assert_eq!((raw & TCR_TG1_MASK) >> 30, 0b11, "TG1 = 64 KiB");
    assert_eq!((raw & TCR_IPS_MASK) >> 32, 0b100, "IPS = 44 bit");
}

// ---------------------------------------------------------------- MAIR_EL1

/// 8 slot 8 bit: slot `i` ở bit `8*i` — giá trị tuyệt đối tính tay.
#[test]
fn mair_slots() {
    let mair = Mair::new([0x00, 0xFF, 0x04, 0x44, 0xBB, 0xF0, 0x11, 0x22]);
    assert_eq!(mair.bits(), 0x2211_F0BB_4404_FF00);
    assert_eq!(mair.slot(0), 0x00);
    assert_eq!(mair.slot(1), 0xFF);
    assert_eq!(mair.slot(2), 0x04);
    assert_eq!(mair.slot(3), 0x44);
    assert_eq!(mair.slot(4), 0xBB);
    assert_eq!(mair.slot(5), 0xF0);
    assert_eq!(mair.slot(6), 0x11);
    assert_eq!(mair.slot(7), 0x22);
    // `with_slot` chỉ đổi đúng byte của slot, không xê dịch slot khác.
    let changed = mair.with_slot(3, 0xAA);
    assert_eq!(changed.bits(), 0x2211_F0BB_AA04_FF00);
    assert_eq!(changed.slot(3), 0xAA);
    assert_eq!(changed.slot(2), 0x04);
    assert_eq!(changed.slot(4), 0xBB);
    // Slot quay vòng modulo 8 (không panic với chỉ số ngoài dải).
    assert_eq!(mair.slot(8), mair.slot(0));
    assert_eq!(mair.with_slot(9, 0x5A).slot(1), 0x5A);
    // Byte thuộc tính chuẩn của đặc tả.
    assert_eq!(MAIR_ATTR_DEVICE_NGNRNE, 0x00);
    assert_eq!(MAIR_ATTR_DEVICE_NGNRE, 0x04);
    assert_eq!(MAIR_ATTR_NORMAL_NC, 0x44);
    assert_eq!(MAIR_ATTR_NORMAL_WT, 0xBB);
    assert_eq!(MAIR_ATTR_NORMAL_WB, 0xFF);
}

/// `MAIR_EL1` mục tiêu: byte 0 Device-nGnRnE, byte 1 Normal WB — khớp `format::MAIR_EL1`.
#[test]
fn mair_kernel_value() {
    assert_eq!(MAIR_EL1_KERNEL.bits(), 0x0000_0000_0000_FF00);
    assert_eq!(MAIR_EL1_KERNEL.slot(0), 0x00);
    assert_eq!(MAIR_EL1_KERNEL.slot(1), 0xFF);
    for i in 2..8 {
        assert_eq!(MAIR_EL1_KERNEL.slot(i), 0, "slot {i} phải trống");
    }
    // Hai định nghĩa độc lập (format.rs và sysreg.rs) phải trùng — nếu ai đổi một bên thì đỏ.
    assert_eq!(
        MAIR_EL1_KERNEL.bits(),
        vifast_plat_aarch64::format::MAIR_EL1
    );
}

// ---------------------------------------------------------------- SCTLR_EL1

/// Từng bit `SCTLR_EL1` đúng vị trí tuyệt đối của DDI 0601 (bản 2024-12).
///
/// Ghi chú: `SCTLR_EL1` **không** có `UXN`/`PXN` — hai bit đó là bit descriptor (bit 54/53),
/// đã có ở `format::DESC_UXN`/`DESC_PXN`; test này không bịa trường không tồn tại.
#[test]
fn sctlr_bit_positions() {
    let expected: [(u64, u64); 39] = [
        (SCTLR_M, 1 << 0),
        (SCTLR_A, 1 << 1),
        (SCTLR_C, 1 << 2),
        (SCTLR_SA, 1 << 3),
        (SCTLR_SA0, 1 << 4),
        (SCTLR_CP15BEN, 1 << 5),
        (SCTLR_ITD, 1 << 7),
        (SCTLR_SED, 1 << 8),
        (SCTLR_UMA, 1 << 9),
        (SCTLR_EOS, 1 << 11),
        (SCTLR_I, 1 << 12),
        (SCTLR_ENDB, 1 << 13),
        (SCTLR_DZE, 1 << 14),
        (SCTLR_UCT, 1 << 15),
        (SCTLR_NTWI, 1 << 16),
        (SCTLR_NTWE, 1 << 18),
        (SCTLR_WXN, 1 << 19),
        (SCTLR_TSCXT, 1 << 20),
        (SCTLR_IESB, 1 << 21),
        (SCTLR_EIS, 1 << 22),
        (SCTLR_SPAN, 1 << 23),
        (SCTLR_E0E, 1 << 24),
        (SCTLR_EE, 1 << 25),
        (SCTLR_UCI, 1 << 26),
        (SCTLR_ENDA, 1 << 27),
        (SCTLR_NTLSMD, 1 << 28),
        (SCTLR_LSMAOE, 1 << 29),
        (SCTLR_ENIB, 1 << 30),
        (SCTLR_ENIA, 1 << 31),
        (SCTLR_BT0, 1 << 35),
        (SCTLR_BT1, 1 << 36),
        (SCTLR_ITFSB, 1 << 37),
        (SCTLR_ATA, 1 << 43),
        (SCTLR_DSSBS, 1 << 44),
        (SCTLR_ENASR, 1 << 54),
        (SCTLR_ENAS0, 1 << 55),
        (SCTLR_ENALS, 1 << 56),
        (SCTLR_EPAN, 1 << 57),
        (SCTLR_ENTP2, 1 << 60),
    ];
    for (actual, want) in expected {
        assert_eq!(actual, want, "bit SCTLR sai vị trí");
    }
}

/// Builder `Sctlr` ghép/xoá bit đúng — giá trị tuyệt đối tính tay.
#[test]
fn sctlr_builder() {
    let on = Sctlr::new()
        .with_mmu(true)
        .with_cache(true)
        .with_icache(true)
        .with_stack_alignment(true)
        .with_wxn(true);
    // M(1) | C(4) | I(0x1000) | SA(8) | WXN(0x8_0000) = 0x8_100D.
    assert_eq!(on.bits(), 0x8_100D);
    assert!(on.bit(SCTLR_M));
    assert!(on.bit(SCTLR_C));
    assert!(on.bit(SCTLR_I));
    assert!(on.bit(SCTLR_SA));
    assert!(on.bit(SCTLR_WXN));
    assert!(!on.bit(SCTLR_A));
    // Xoá lại đúng bit đã bật, không đụng bit khác.
    let off = on.with_mmu(false).with_wxn(false);
    assert_eq!(off.bits(), 0x100C);
    assert!(!off.bit(SCTLR_M));
    assert!(!off.bit(SCTLR_WXN));
    assert!(off.bit(SCTLR_I));
    // Tổ hợp 34 setter còn lại — **giá trị tuyệt đối viết tay** (không OR hằng của crate):
    // bit 23,24,25,26,14,15,16,18,21,22,13,27,28,29,30,31,35,36,37,43,44,54,55,56,57,60,
    // 11,20,5,7,8,9,1,4.
    let misc = Sctlr::new()
        .with_span(true)
        .with_el0_big_endian(true)
        .with_el1_big_endian(true)
        .with_uci(true)
        .with_dze(true)
        .with_uct(true)
        .with_ntwi(true)
        .with_ntwe(true)
        .with_iesb(true)
        .with_eis(true)
        .with_endb(true)
        .with_enda(true)
        .with_ntlsmd(true)
        .with_lsmaoe(true)
        .with_enib(true)
        .with_enia(true)
        .with_bt0(true)
        .with_bt1(true)
        .with_itfsb(true)
        .with_ata(true)
        .with_dssbs(true)
        .with_enasr(true)
        .with_enas0(true)
        .with_enals(true)
        .with_epan(true)
        .with_entp2(true)
        .with_eos(true)
        .with_tscxt(true)
        .with_cp15ben(true)
        .with_itd(true)
        .with_sed(true)
        .with_uma(true)
        .with_alignment(true)
        .with_el0_stack_alignment(true);
    assert_eq!(misc.bits(), 0x13C0_1838_FFF5_EBB2);
}

/// **Từng** setter `Sctlr` phải nối đúng **một** bit tuyệt đối viết tay từ DDI 0487.
///
/// Thay tautology cũ (`misc.bits() == OR hằng của chính crate`): phép OR giao hoán nên hoán vị
/// hai setter vẫn xanh. Ở đây mỗi setter được kiểm **riêng** với literal ⇒ hoán vị
/// `span↔E0E`, `ata↔dssbs`, `ntwi↔ntwe` đều **ĐỎ** (C2 của KT-175, D2-162).
#[test]
fn sctlr_setters_reference_bits() {
    let expected: [(u64, u64, &str); 39] = [
        (Sctlr::new().with_mmu(true).bits(), 1 << 0, "M"),
        (Sctlr::new().with_alignment(true).bits(), 1 << 1, "A"),
        (Sctlr::new().with_cache(true).bits(), 1 << 2, "C"),
        (Sctlr::new().with_stack_alignment(true).bits(), 1 << 3, "SA"),
        (
            Sctlr::new().with_el0_stack_alignment(true).bits(),
            1 << 4,
            "SA0",
        ),
        (Sctlr::new().with_cp15ben(true).bits(), 1 << 5, "CP15BEN"),
        (Sctlr::new().with_itd(true).bits(), 1 << 7, "ITD"),
        (Sctlr::new().with_sed(true).bits(), 1 << 8, "SED"),
        (Sctlr::new().with_uma(true).bits(), 1 << 9, "UMA"),
        (Sctlr::new().with_eos(true).bits(), 1 << 11, "EOS"),
        (Sctlr::new().with_icache(true).bits(), 1 << 12, "I"),
        (Sctlr::new().with_endb(true).bits(), 1 << 13, "EnDB"),
        (Sctlr::new().with_dze(true).bits(), 1 << 14, "DZE"),
        (Sctlr::new().with_uct(true).bits(), 1 << 15, "UCT"),
        (Sctlr::new().with_ntwi(true).bits(), 1 << 16, "nTWI"),
        (Sctlr::new().with_ntwe(true).bits(), 1 << 18, "nTWE"),
        (Sctlr::new().with_wxn(true).bits(), 1 << 19, "WXN"),
        (Sctlr::new().with_tscxt(true).bits(), 1 << 20, "TSCXT"),
        (Sctlr::new().with_iesb(true).bits(), 1 << 21, "IESB"),
        (Sctlr::new().with_eis(true).bits(), 1 << 22, "EIS"),
        (Sctlr::new().with_span(true).bits(), 1 << 23, "SPAN"),
        (
            Sctlr::new().with_el0_big_endian(true).bits(),
            1 << 24,
            "E0E",
        ),
        (Sctlr::new().with_el1_big_endian(true).bits(), 1 << 25, "EE"),
        (Sctlr::new().with_uci(true).bits(), 1 << 26, "UCI"),
        (Sctlr::new().with_enda(true).bits(), 1 << 27, "EnDA"),
        (Sctlr::new().with_ntlsmd(true).bits(), 1 << 28, "nTLSMD"),
        (Sctlr::new().with_lsmaoe(true).bits(), 1 << 29, "LSMAOE"),
        (Sctlr::new().with_enib(true).bits(), 1 << 30, "EnIB"),
        (Sctlr::new().with_enia(true).bits(), 1 << 31, "EnIA"),
        (Sctlr::new().with_bt0(true).bits(), 1 << 35, "BT0"),
        (Sctlr::new().with_bt1(true).bits(), 1 << 36, "BT1"),
        (Sctlr::new().with_itfsb(true).bits(), 1 << 37, "ITFSB"),
        (Sctlr::new().with_ata(true).bits(), 1 << 43, "ATA"),
        (Sctlr::new().with_dssbs(true).bits(), 1 << 44, "DSSBS"),
        (Sctlr::new().with_enasr(true).bits(), 1 << 54, "EnASR"),
        (Sctlr::new().with_enas0(true).bits(), 1 << 55, "EnAS0"),
        (Sctlr::new().with_enals(true).bits(), 1 << 56, "EnALS"),
        (Sctlr::new().with_epan(true).bits(), 1 << 57, "EPAN"),
        (Sctlr::new().with_entp2(true).bits(), 1 << 60, "EnTP2"),
    ];
    for (actual, want, name) in expected {
        assert_eq!(actual, want, "setter {name} nối sai bit");
    }
}

// ---------------------------------------------------------------- TTBR0/1_EL1

/// `TTBR`: ASID[63:48] + BADDR[47:12] + CnP[0] — giá trị tuyệt đối tính tay.
#[test]
fn ttbr_fields() {
    let ttbr = Ttbr::new(0x4000_0000, 0x1234);
    assert_eq!(ttbr.bits(), 0x1234_0000_4000_0000);
    assert_eq!(ttbr.baddr(), 0x4000_0000);
    assert_eq!(ttbr.asid(), 0x1234);
    assert!(!ttbr.cnp());
    assert_eq!(TTBR_ASID_SHIFT, 48);
    assert_eq!(TTBR_CNP, 1);
    assert_eq!(TTBR_BADDR_MASK, 0x0000_FFFF_FFFF_F000);
    // CnP là bit 0.
    assert_eq!(ttbr.with_cnp(true).bits(), 0x1234_0000_4000_0001);
    assert!(ttbr.with_cnp(true).cnp());
    // BADDR bị cắt còn bit [47:12] (granule 4 KiB).
    assert_eq!(Ttbr::new(0x4000_0ABC, 0).bits(), 0x4000_0000);
    assert_eq!(
        Ttbr::new(0xFFFF_FFFF_FFFF_FFFF, 0xFFFF).bits(),
        0xFFFF_FFFF_FFFF_F000
    );
    // ASID 16 bit: giá trị lớn nhất vẫn giữ nguyên.
    assert_eq!(Ttbr::new(0, u16::MAX).asid(), 0xFFFF);
}

// ---------------------------------------------------------------- VBAR_EL1

/// `VBAR_EL1`: VBA[63:11], 11 bit thấp RES0 — giá trị tuyệt đối tính tay.
#[test]
fn vbar_fields() {
    let vbar = Vbar::new(0xFFFF_0000_0000_0800);
    assert_eq!(vbar.bits(), 0xFFFF_0000_0000_0800);
    assert_eq!(vbar.base(), 0xFFFF_0000_0000_0800);
    assert_eq!(VBAR_BASE_MASK, !0x7FF);
    // Địa chỉ lệch 2 KiB bị cắt: 0xDEF & !0x7FF = 0x800.
    assert_eq!(
        Vbar::new(0x1234_5678_9ABC_0DEF).bits(),
        0x1234_5678_9ABC_0800
    );
}

// ---------------------------------------------------------------- ESR_EL1

/// `ESR_EL1`: EC[31:26] + IL[25] + ISS[24:0] — giá trị tuyệt đối tính tay.
#[test]
fn esr_fields() {
    let esr = Esr::new(ExceptionClass::DataAbortSameEl, true, 0x1234);
    // 0x25 << 26 = 0x9400_0000; IL = 0x0200_0000; ISS = 0x1234.
    assert_eq!(esr.bits(), 0x9600_1234);
    assert_eq!(esr.ec_raw(), 0x25);
    assert_eq!(esr.ec(), Some(ExceptionClass::DataAbortSameEl));
    assert!(esr.il());
    assert_eq!(esr.iss(), 0x1234);
    // Bảng EC tuyệt đối (DDI 0601).
    assert_eq!(ExceptionClass::Unknown.bits(), 0x00);
    assert_eq!(ExceptionClass::Svc64.bits(), 0x15);
    assert_eq!(ExceptionClass::InstructionAbortLowerEl.bits(), 0x20);
    assert_eq!(ExceptionClass::InstructionAbortSameEl.bits(), 0x21);
    assert_eq!(ExceptionClass::PcAlignment.bits(), 0x22);
    assert_eq!(ExceptionClass::DataAbortLowerEl.bits(), 0x24);
    assert_eq!(ExceptionClass::DataAbortSameEl.bits(), 0x25);
    assert_eq!(ExceptionClass::SpAlignment.bits(), 0x26);
    assert_eq!(ExceptionClass::BreakpointLowerEl.bits(), 0x30);
    assert_eq!(ExceptionClass::BreakpointSameEl.bits(), 0x31);
    assert_eq!(
        ExceptionClass::from_bits(0x24),
        Some(ExceptionClass::DataAbortLowerEl)
    );
    assert_eq!(ExceptionClass::from_bits(0x3F), None, "EC lạ ⇒ None");
    // ISS bị cắt còn 25 bit.
    assert_eq!(
        Esr::new(ExceptionClass::Unknown, false, 0xFFFF_FFFF).iss(),
        0x1FF_FFFF
    );
    assert!(!Esr::new(ExceptionClass::Unknown, false, 0).il());
}

// ---------------------------------------------------------------- SPSR_EL1 / DAIF

/// `SPSR_EL1`: NZCV[31:28] + DAIF[9:6] + M[3:0] — giá trị tuyệt đối tính tay.
#[test]
fn spsr_fields() {
    let spsr = Spsr::new(SpsrMode::El1h).with_daif(Daif::all());
    // M = 0b0101 = 5; DAIF = 0b1111 << 6 = 0x3C0.
    assert_eq!(spsr.bits(), 0x3C5);
    assert_eq!(spsr.mode(), Some(SpsrMode::El1h));
    assert_eq!(spsr.mode_raw(), 5);
    assert!(spsr.daif().is_all_masked());
    assert!(!spsr.n() && !spsr.z() && !spsr.c() && !spsr.v());
    // NZCV: N(31) | C(29) = 0xA000_0000.
    let nzcv = spsr.with_nzcv(true, false, true, false);
    assert_eq!(nzcv.bits(), 0xA000_03C5);
    assert!(nzcv.n());
    assert!(!nzcv.z());
    assert!(nzcv.c());
    assert!(!nzcv.v());
    // Bảng mode tuyệt đối.
    assert_eq!(SpsrMode::El0t.bits(), 0b0000);
    assert_eq!(SpsrMode::El1t.bits(), 0b0100);
    assert_eq!(SpsrMode::El1h.bits(), 0b0101);
    assert_eq!(SpsrMode::El2t.bits(), 0b1000);
    assert_eq!(SpsrMode::El2h.bits(), 0b1001);
    assert_eq!(SpsrMode::El3t.bits(), 0b1100);
    assert_eq!(SpsrMode::El3h.bits(), 0b1101);
    assert_eq!(SpsrMode::from_bits(0b0101), Some(SpsrMode::El1h));
    assert_eq!(SpsrMode::from_bits(0b0110), None, "mode lạ ⇒ None");
    assert_eq!(Spsr::new(SpsrMode::El0t).bits(), 0);
    assert_eq!(SPSR_N, 0x8000_0000);
    assert_eq!(SPSR_Z, 0x4000_0000);
    assert_eq!(SPSR_C, 0x2000_0000);
    assert_eq!(SPSR_V, 0x1000_0000);
}

/// Ba hằng **shift** công khai — **giá trị tham chiếu viết tay** từ DDI 0487.
///
/// `TTBR`: `BADDR` bắt đầu bit 1 (bit `[11:1]` RES0 với granule 4 KiB); `VBAR`: `VBA[63:11]`
/// (11 bit thấp RES0, 2 KiB aligned); `SPSR`: `M[3:0]` ở bit 0. Đột biến từng hằng phải làm
/// test **ĐỎ** (C1 của KT-175, D2-162).
#[test]
fn shift_constants_reference_values() {
    assert_eq!(TTBR_BADDR_SHIFT, 1, "TTBR BADDR bắt đầu bit 1");
    assert_eq!(1u64 << TTBR_BADDR_SHIFT, 0x2, "bit 1");
    assert_eq!(VBAR_BASE_SHIFT, 11, "VBAR VBA[63:11]");
    assert_eq!(1u64 << VBAR_BASE_SHIFT, 0x800, "2 KiB alignment");
    assert_eq!(SPSR_M_SHIFT, 0, "SPSR M[3:0] ở bit 0");
    assert_eq!(SPSR_M_MASK, 0xF, "M[3:0] = 4 bit");
}

/// `DAIF`: D[9] A[8] I[7] F[6] — giá trị tuyệt đối tính tay.
#[test]
fn daif_fields() {
    assert_eq!(DAIF_D, 0x200);
    assert_eq!(DAIF_A, 0x100);
    assert_eq!(DAIF_I, 0x80);
    assert_eq!(DAIF_F, 0x40);
    assert_eq!(DAIF_ALL, 0x3C0);
    assert_eq!(Daif::all().bits(), 0x3C0);
    assert_eq!(Daif::new().bits(), 0);
    assert_eq!(Daif::new().with_i(true).bits(), 0x80);
    assert_eq!(Daif::new().with_d(true).bits(), 0x200);
    assert_eq!(Daif::new().with_a(true).bits(), 0x100);
    assert_eq!(Daif::new().with_f(true).bits(), 0x40);
    let all = Daif::all();
    assert!(all.d() && all.a() && all.i() && all.f());
    assert!(all.is_all_masked());
    assert!(!Daif::new().is_all_masked());
    assert_eq!(Daif::all().with_i(false).bits(), 0x340);
}

// ---------------------------------------------------------------- FAR_EL1 / ELR_EL1

/// `FAR_EL1`/`ELR_EL1` là địa chỉ 64 bit đầy đủ — round-trip tuyệt đối.
#[test]
fn far_elr_roundtrip() {
    let far = FarEl1::new(0xFFFF_FFC0_0000_1000);
    assert_eq!(far.bits(), 0xFFFF_FFC0_0000_1000);
    assert_eq!(far.address(), 0xFFFF_FFC0_0000_1000);
    let elr = ElrEl1::new(0x0000_0000_0000_2000);
    assert_eq!(elr.bits(), 0x0000_0000_0000_2000);
    assert_eq!(elr.address(), 0x0000_0000_0000_2000);
    // Không cắt bit nào, kể cả địa chỉ cao nhất.
    assert_eq!(FarEl1::new(u64::MAX).bits(), u64::MAX);
    assert_eq!(ElrEl1::new(u64::MAX).bits(), u64::MAX);
}

/// **KT-254** — `CPACR_EL1` phải bật **FP/SIMD** (`FPEN[21:20] = 0b11`) và **SVE**
/// (`ZEN[17:16] = 0b11`): trình biên dịch phát NEON cho hàm lớn (`PageTable::unmap` zero 176 B
/// stack); `FPEN = 0` ⇒ `Undefined Instruction` (`EC = 0x07`) — đo được trên QEMU.
#[test]
fn cpacr_enables_fp_simd() {
    assert_eq!(
        CPACR_EL1_FP_SIMD & (0b11 << 20),
        0b11 << 20,
        "FPEN phải là 0b11 (không trap FP/SIMD ở EL0/EL1)"
    );
    assert_eq!(
        CPACR_EL1_FP_SIMD & (0b11 << 16),
        0b11 << 16,
        "ZEN phải là 0b11 (không trap SVE)"
    );
    assert_eq!(CPACR_EL1_FP_SIMD, 0x0033_0000, "giá trị tuyệt đối");
}
