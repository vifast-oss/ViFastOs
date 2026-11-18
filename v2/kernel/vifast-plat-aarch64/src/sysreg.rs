// SPDX-License-Identifier: MIT OR Apache-2.0
//! Thanh ghi hệ thống AArch64 — **encoding thuần dữ liệu**, host-testable (KT-167 / K4' bước 2).
//!
//! Tệp này **không** asm, **không** `unsafe`, **không** `cfg(target_arch)`: nó chỉ mô tả vị trí
//! bit và cách ghép/bóc giá trị của các thanh ghi điều khiển MMU. Nhờ vậy `cargo test` trên host
//! kiểm được **giá trị tuyệt đối** của từng trường (test `tests/sysreg.rs`), đúng yêu cầu bằng
//! chứng của K4' bước 2. Phần `msr`/`mrs` thật nằm ở [`crate::asm`] (sau `cfg(target_arch)`).
//!
//! # Nguồn
//!
//! Vị trí bit lấy từ **Arm Architecture Registers** (DDI 0601, bản 2024-12) — đối chiếu chéo với
//! `arch/arm64/include/asm/sysreg.h` + `pgtable-hwdef.h` của Linux, `sys/arm64/include/armreg.h`
//! của FreeBSD và `target/arm/cpu.h` của QEMU. Mã thao tác `TLBI`/barrier đối chiếu bằng **hợp
//! dịch thật** (`clang --target=aarch64-none-elf` + `llvm-objdump`), xem [`crate::tlb`] và
//! [`crate::barrier`].
//!
//! # Ghi chú đặc tả: `SCTLR_EL1` **không** có `UXN`/`PXN`
//!
//! Đề bài KT-167 liệt kê "SCTLR_EL1 (M, C, I, SA, WXN, UXN…)". Theo DDI 0601 bản 2024-12,
//! `SCTLR_EL1` **không** có trường `UXN`/`PXN`: hai bit đó là bit **descriptor** (bit 54/53 của
//! entry bảng trang, đã cài ở [`crate::format`] là [`DESC_UXN`](crate::format::DESC_UXN) /
//! [`DESC_PXN`](crate::format::DESC_PXN)). Tệp này vì vậy **không** bịa trường không tồn tại;
//! nó cài đúng các trường có thật của `SCTLR_EL1` (kể cả `WXN`, `SPAN`, `E0E`, `EE`, `UCI`,
//! `DZE`, `UCT`, `nTWI`, `nTWE`, `IESB`, `EIS`, `EnDB`, `EnDA`, `nTLSMD`, `LSMAOE`, `EnIB`,
//! `EnIA`, `BT0`, `BT1`, `ITFSB`, `ATA`, `DSSBS`, `EnASR`, `EnAS0`, `EnALS`, `EPAN`, `EnTP2`).
//!
//! # Vì sao builder trả `u64` chứ không phải kiểu riêng cho từng thanh ghi
//!
//! `msr`/`mrs` chỉ nhận `u64`; builder ở đây là **hàm const** nên giá trị dựng được lúc biên dịch
//! (`const TCR_EL1_KERNEL: Tcr = ...`), không cấp phát, không panic. Test so **giá trị tuyệt đối**
//! viết tay từ đặc tả — không lấy hằng của chính crate để so (yêu cầu KT-167).

use vifast_pte::Shareability;

// =====================================================================================
// TCR_EL1 — Translation Control Register (EL1)
// =====================================================================================

/// Vị trí `T0SZ[5:0]` — kích cỡ vùng TTBR0: `2^(64 - T0SZ)` byte.
pub const TCR_T0SZ_SHIFT: u32 = 0;
/// Mặt nạ `T0SZ[5:0]`.
pub const TCR_T0SZ_MASK: u64 = 0x3F << TCR_T0SZ_SHIFT;
/// Bit `EPD0` — `1` ⇒ **tắt** table walk của TTBR0 (mọi truy cập nửa thấp fault).
pub const TCR_EPD0_SHIFT: u32 = 7;
/// Mặt nạ bit `EPD0`.
pub const TCR_EPD0: u64 = 1 << TCR_EPD0_SHIFT;
/// Vị trí `IRGN0[1:0]` — cacheability bảng trang TTBR0 (inner).
pub const TCR_IRGN0_SHIFT: u32 = 8;
/// Mặt nạ `IRGN0[1:0]`.
pub const TCR_IRGN0_MASK: u64 = 0b11 << TCR_IRGN0_SHIFT;
/// Vị trí `ORGN0[1:0]` — cacheability bảng trang TTBR0 (outer).
pub const TCR_ORGN0_SHIFT: u32 = 10;
/// Mặt nạ `ORGN0[1:0]`.
pub const TCR_ORGN0_MASK: u64 = 0b11 << TCR_ORGN0_SHIFT;
/// Vị trí `SH0[1:0]` — shareability bảng trang TTBR0.
pub const TCR_SH0_SHIFT: u32 = 12;
/// Mặt nạ `SH0[1:0]`.
pub const TCR_SH0_MASK: u64 = 0b11 << TCR_SH0_SHIFT;
/// Vị trí `TG0[1:0]` — cỡ granule của TTBR0 (`0b00` 4 KiB, `0b01` 64 KiB, `0b10` 16 KiB).
pub const TCR_TG0_SHIFT: u32 = 14;
/// Mặt nạ `TG0[1:0]`.
pub const TCR_TG0_MASK: u64 = 0b11 << TCR_TG0_SHIFT;
/// Vị trí `T1SZ[5:0]` — kích cỡ vùng TTBR1.
pub const TCR_T1SZ_SHIFT: u32 = 16;
/// Mặt nạ `T1SZ[5:0]`.
pub const TCR_T1SZ_MASK: u64 = 0x3F << TCR_T1SZ_SHIFT;
/// Bit `A1` — `1` ⇒ ASID dùng cho TTBR1 nằm ở `TTBR1_EL1.ASID` (không dùng `TTBR0_EL1.ASID`).
pub const TCR_A1_SHIFT: u32 = 22;
/// Mặt nạ bit `A1`.
pub const TCR_A1: u64 = 1 << TCR_A1_SHIFT;
/// Bit `EPD1` — `1` ⇒ **tắt** table walk của TTBR1.
pub const TCR_EPD1_SHIFT: u32 = 23;
/// Mặt nạ bit `EPD1`.
pub const TCR_EPD1: u64 = 1 << TCR_EPD1_SHIFT;
/// Vị trí `IRGN1[1:0]` — cacheability bảng trang TTBR1 (inner).
pub const TCR_IRGN1_SHIFT: u32 = 24;
/// Mặt nạ `IRGN1[1:0]`.
pub const TCR_IRGN1_MASK: u64 = 0b11 << TCR_IRGN1_SHIFT;
/// Vị trí `ORGN1[1:0]` — cacheability bảng trang TTBR1 (outer).
pub const TCR_ORGN1_SHIFT: u32 = 26;
/// Mặt nạ `ORGN1[1:0]`.
pub const TCR_ORGN1_MASK: u64 = 0b11 << TCR_ORGN1_SHIFT;
/// Vị trí `SH1[1:0]` — shareability bảng trang TTBR1.
pub const TCR_SH1_SHIFT: u32 = 28;
/// Mặt nạ `SH1[1:0]`.
pub const TCR_SH1_MASK: u64 = 0b11 << TCR_SH1_SHIFT;
/// Vị trí `TG1[1:0]` — cỡ granule của TTBR1 (`0b01` 16 KiB, `0b10` 4 KiB, `0b11` 64 KiB).
pub const TCR_TG1_SHIFT: u32 = 30;
/// Mặt nạ `TG1[1:0]`.
pub const TCR_TG1_MASK: u64 = 0b11 << TCR_TG1_SHIFT;
/// Vị trí `IPS[2:0]` — số bit PA mà MMU hỗ trợ (xem [`Ips`]).
pub const TCR_IPS_SHIFT: u32 = 32;
/// Mặt nạ `IPS[2:0]`.
pub const TCR_IPS_MASK: u64 = 0b111 << TCR_IPS_SHIFT;
/// Bit `AS` — `1` ⇒ ASID 16 bit (256 → 65 536 thế hệ).
pub const TCR_AS_SHIFT: u32 = 36;
/// Mặt nạ bit `AS`.
pub const TCR_AS: u64 = 1 << TCR_AS_SHIFT;
/// Bit `TBI0` — `1` ⇒ bỏ qua byte cao (top-byte-ignore) của địa chỉ nửa thấp.
pub const TCR_TBI0_SHIFT: u32 = 37;
/// Mặt nạ bit `TBI0`.
pub const TCR_TBI0: u64 = 1 << TCR_TBI0_SHIFT;
/// Bit `TBI1` — `1` ⇒ bỏ qua byte cao của địa chỉ nửa cao.
pub const TCR_TBI1_SHIFT: u32 = 38;
/// Mặt nạ bit `TBI1`.
pub const TCR_TBI1: u64 = 1 << TCR_TBI1_SHIFT;

/// Cỡ granule của bảng trang (AArch64 hỗ trợ 4/16/64 KiB).
///
/// `TG0` và `TG1` **mã hoá khác nhau** (đặc tả DDI 0601): `TG0`: 4K=`0b00`, 64K=`0b01`,
/// 16K=`0b10`; `TG1`: 16K=`0b01`, 4K=`0b10`, 64K=`0b11`. Vì vậy có hai hàm `tg0_bits`/`tg1_bits`
/// riêng — dùng lẫn là lỗi im lặng (test `tcr_tg_encodings` chốt cả hai bảng).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Granule {
    /// 4 KiB.
    K4,
    /// 16 KiB.
    K16,
    /// 64 KiB.
    K64,
}

impl Granule {
    /// Mã `TG0[1:0]` của granule này.
    pub const fn tg0_bits(self) -> u64 {
        match self {
            Self::K4 => 0b00,
            Self::K64 => 0b01,
            Self::K16 => 0b10,
        }
    }

    /// Mã `TG1[1:0]` của granule này (khác `TG0`!).
    pub const fn tg1_bits(self) -> u64 {
        match self {
            Self::K16 => 0b01,
            Self::K4 => 0b10,
            Self::K64 => 0b11,
        }
    }

    /// Số bit offset trong trang (`log2` cỡ granule).
    pub const fn page_bits(self) -> u32 {
        match self {
            Self::K4 => 12,
            Self::K16 => 14,
            Self::K64 => 16,
        }
    }

    /// Bóc `TG0[1:0]`; `0b11` là reserved nên trả `None`.
    pub const fn from_tg0(bits: u64) -> Option<Self> {
        match bits & 0b11 {
            0b00 => Some(Self::K4),
            0b01 => Some(Self::K64),
            0b10 => Some(Self::K16),
            _ => None,
        }
    }

    /// Bóc `TG1[1:0]`; `0b00` là reserved nên trả `None`.
    pub const fn from_tg1(bits: u64) -> Option<Self> {
        match bits & 0b11 {
            0b01 => Some(Self::K16),
            0b10 => Some(Self::K4),
            0b11 => Some(Self::K64),
            _ => None,
        }
    }
}

/// Cacheability của bảng trang (`IRGN*`/`ORGN*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcrCache {
    /// `0b00` — non-cacheable.
    NonCacheable = 0b00,
    /// `0b01` — write-back, write-allocate (kernel dùng).
    WriteBackWriteAllocate = 0b01,
    /// `0b10` — write-through.
    WriteThrough = 0b10,
    /// `0b11` — write-back, không write-allocate.
    WriteBackNoWriteAllocate = 0b11,
}

impl TcrCache {
    /// Mã 2 bit của cacheability.
    pub const fn bits(self) -> u64 {
        self as u64
    }

    /// Bóc 2 bit; mọi tổ hợp đều hợp lệ nên luôn `Some`.
    pub const fn from_bits(bits: u64) -> Self {
        match bits & 0b11 {
            0b00 => Self::NonCacheable,
            0b01 => Self::WriteBackWriteAllocate,
            0b10 => Self::WriteThrough,
            _ => Self::WriteBackNoWriteAllocate,
        }
    }
}

/// Số bit địa chỉ vật lý mà MMU hỗ trợ — trường `IPS[2:0]` của `TCR_EL1`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ips {
    /// `0b000` — 32 bit PA.
    Bits32 = 0b000,
    /// `0b001` — 36 bit PA.
    Bits36 = 0b001,
    /// `0b010` — 40 bit PA.
    Bits40 = 0b010,
    /// `0b011` — 42 bit PA.
    Bits42 = 0b011,
    /// `0b100` — 44 bit PA.
    Bits44 = 0b100,
    /// `0b101` — 48 bit PA (kernel dùng; khớp `AARCH64_PA_BITS`).
    Bits48 = 0b101,
    /// `0b110` — 52 bit PA (cần FEAT_LPA).
    Bits52 = 0b110,
}

impl Ips {
    /// Mã `IPS[2:0]`.
    pub const fn bits(self) -> u64 {
        self as u64
    }

    /// Số bit PA tương ứng.
    pub const fn pa_bits(self) -> u32 {
        match self {
            Self::Bits32 => 32,
            Self::Bits36 => 36,
            Self::Bits40 => 40,
            Self::Bits42 => 42,
            Self::Bits44 => 44,
            Self::Bits48 => 48,
            Self::Bits52 => 52,
        }
    }

    /// Bóc `IPS[2:0]`; `0b111` là reserved nên trả `None`.
    pub const fn from_bits(bits: u64) -> Option<Self> {
        match bits & 0b111 {
            0b000 => Some(Self::Bits32),
            0b001 => Some(Self::Bits36),
            0b010 => Some(Self::Bits40),
            0b011 => Some(Self::Bits42),
            0b100 => Some(Self::Bits44),
            0b101 => Some(Self::Bits48),
            0b110 => Some(Self::Bits52),
            _ => None,
        }
    }
}

/// Giá trị `TCR_EL1` — builder **const** đóng gói bitfield.
///
/// `Tcr::new` đặt 5 trường bắt buộc (T0SZ/TG0/T1SZ/TG1/IPS); các trường còn lại đặt bằng
/// `with_*`. Mọi hàm là `const fn` nên dựng được `const` lúc biên dịch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tcr(u64);

impl Tcr {
    /// Dựng `TCR_EL1` với 5 trường bắt buộc; các trường khác bằng 0.
    ///
    /// `t0sz`/`t1sz` bị cắt còn 6 bit (đặc tả chỉ có `[5:0]`); caller truyền `64 - VA_BITS`
    /// (39-bit VA ⇒ 25).
    pub const fn new(t0sz: u32, tg0: Granule, t1sz: u32, tg1: Granule, ips: Ips) -> Self {
        let mut bits = 0u64;
        bits |= ((t0sz as u64) & 0x3F) << TCR_T0SZ_SHIFT;
        bits |= tg0.tg0_bits() << TCR_TG0_SHIFT;
        bits |= ((t1sz as u64) & 0x3F) << TCR_T1SZ_SHIFT;
        bits |= tg1.tg1_bits() << TCR_TG1_SHIFT;
        bits |= ips.bits() << TCR_IPS_SHIFT;
        Self(bits)
    }

    /// Giá trị 64 bit để ghi `TCR_EL1`.
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// Đặt/xoá một bit theo `mask` (dùng nội bộ cho các `with_*`).
    const fn set(self, mask: u64, on: bool) -> Self {
        Self(if on { self.0 | mask } else { self.0 & !mask })
    }

    /// Đặt một trường 2 bit theo `shift`.
    const fn set2(self, shift: u32, value: u64) -> Self {
        Self((self.0 & !(0b11 << shift)) | ((value & 0b11) << shift))
    }

    /// `EPD0 = 1` khi `disabled` — tắt table walk TTBR0.
    pub const fn with_epd0(self, disabled: bool) -> Self {
        self.set(TCR_EPD0, disabled)
    }

    /// `EPD1 = 1` khi `disabled` — tắt table walk TTBR1.
    pub const fn with_epd1(self, disabled: bool) -> Self {
        self.set(TCR_EPD1, disabled)
    }

    /// `A1 = 1` khi `on` — ASID của TTBR1 nằm ở `TTBR1_EL1`.
    pub const fn with_a1(self, on: bool) -> Self {
        self.set(TCR_A1, on)
    }

    /// `AS = 1` khi `on` — ASID 16 bit.
    pub const fn with_asid16(self, on: bool) -> Self {
        self.set(TCR_AS, on)
    }

    /// `TBI0`/`TBI1` — top-byte-ignore cho nửa thấp/cao.
    pub const fn with_tbi(self, tbi0: bool, tbi1: bool) -> Self {
        self.set(TCR_TBI0, tbi0).set(TCR_TBI1, tbi1)
    }

    /// `SH0`/`SH1` — shareability bảng trang TTBR0/TTBR1.
    pub const fn with_shareability(self, sh0: Shareability, sh1: Shareability) -> Self {
        self.set2(TCR_SH0_SHIFT, sh0.bits())
            .set2(TCR_SH1_SHIFT, sh1.bits())
    }

    /// `IRGN0`/`ORGN0`/`IRGN1`/`ORGN1` — cacheability bảng trang.
    pub const fn with_cacheability(
        self,
        irgn0: TcrCache,
        orgn0: TcrCache,
        irgn1: TcrCache,
        orgn1: TcrCache,
    ) -> Self {
        self.set2(TCR_IRGN0_SHIFT, irgn0.bits())
            .set2(TCR_ORGN0_SHIFT, orgn0.bits())
            .set2(TCR_IRGN1_SHIFT, irgn1.bits())
            .set2(TCR_ORGN1_SHIFT, orgn1.bits())
    }

    /// `T0SZ[5:0]`.
    pub const fn t0sz(self) -> u32 {
        ((self.0 >> TCR_T0SZ_SHIFT) & 0x3F) as u32
    }

    /// `T1SZ[5:0]`.
    pub const fn t1sz(self) -> u32 {
        ((self.0 >> TCR_T1SZ_SHIFT) & 0x3F) as u32
    }

    /// `TG0[1:0]` đã bóc; `0b11` (reserved) ⇒ `None`.
    pub const fn tg0(self) -> Option<Granule> {
        Granule::from_tg0(self.0 >> TCR_TG0_SHIFT)
    }

    /// `TG1[1:0]` đã bóc; `0b00` (reserved) ⇒ `None`.
    pub const fn tg1(self) -> Option<Granule> {
        Granule::from_tg1(self.0 >> TCR_TG1_SHIFT)
    }

    /// `IPS[2:0]` đã bóc; `0b111` (reserved) ⇒ `None`.
    pub const fn ips(self) -> Option<Ips> {
        Ips::from_bits(self.0 >> TCR_IPS_SHIFT)
    }

    /// `EPD0`.
    pub const fn epd0(self) -> bool {
        self.0 & TCR_EPD0 != 0
    }

    /// `EPD1`.
    pub const fn epd1(self) -> bool {
        self.0 & TCR_EPD1 != 0
    }

    /// `A1`.
    pub const fn a1(self) -> bool {
        self.0 & TCR_A1 != 0
    }

    /// `AS` (ASID 16 bit).
    pub const fn asid16(self) -> bool {
        self.0 & TCR_AS != 0
    }

    /// `TBI0`.
    pub const fn tbi0(self) -> bool {
        self.0 & TCR_TBI0 != 0
    }

    /// `TBI1`.
    pub const fn tbi1(self) -> bool {
        self.0 & TCR_TBI1 != 0
    }

    /// `SH0[1:0]`; `0b01` (reserved) ⇒ `None`.
    pub const fn sh0(self) -> Option<Shareability> {
        Shareability::from_bits(self.0 >> TCR_SH0_SHIFT)
    }

    /// `SH1[1:0]`; `0b01` (reserved) ⇒ `None`.
    pub const fn sh1(self) -> Option<Shareability> {
        Shareability::from_bits(self.0 >> TCR_SH1_SHIFT)
    }

    /// `IRGN0[1:0]`.
    pub const fn irgn0(self) -> TcrCache {
        TcrCache::from_bits(self.0 >> TCR_IRGN0_SHIFT)
    }

    /// `ORGN0[1:0]`.
    pub const fn orgn0(self) -> TcrCache {
        TcrCache::from_bits(self.0 >> TCR_ORGN0_SHIFT)
    }

    /// `IRGN1[1:0]`.
    pub const fn irgn1(self) -> TcrCache {
        TcrCache::from_bits(self.0 >> TCR_IRGN1_SHIFT)
    }

    /// `ORGN1[1:0]`.
    pub const fn orgn1(self) -> TcrCache {
        TcrCache::from_bits(self.0 >> TCR_ORGN1_SHIFT)
    }
}

/// `TCR_EL1` mục tiêu của kernel: 39-bit VA canonical (D2-147), 48-bit PA, 4 KiB granule,
/// bảng trang Normal write-back write-allocate, inner-shareable, ASID 16 bit.
///
/// `T0SZ = T1SZ = 64 - 39 = 25`; `TG0 = TG1 = 4 KiB`; `IPS = 48 bit`. Cả TTBR0 (nửa thấp) và
/// TTBR1 (nửa cao) **bật** (`EPD0 = EPD1 = 0`); bước 3 nối `TTBR1_EL1` — bước 2 chỉ chốt
/// encoding. Giá trị tuyệt đối: `0x0000_0015_B519_3519` (test `tcr_kernel_value`).
pub const TCR_EL1_KERNEL: Tcr = Tcr::new(25, Granule::K4, 25, Granule::K4, Ips::Bits48)
    .with_shareability(Shareability::Inner, Shareability::Inner)
    .with_cacheability(
        TcrCache::WriteBackWriteAllocate,
        TcrCache::WriteBackWriteAllocate,
        TcrCache::WriteBackWriteAllocate,
        TcrCache::WriteBackWriteAllocate,
    )
    .with_asid16(true);

// =====================================================================================
// MAIR_EL1 — Memory Attribute Indirection Register (EL1)
// =====================================================================================

/// Số byte thuộc tính trong `MAIR_EL1` (8 slot).
pub const MAIR_SLOTS: usize = 8;
/// Số bit dịch giữa hai slot.
pub const MAIR_SLOT_SHIFT: u32 = 8;
/// Mặt nạ một slot.
pub const MAIR_SLOT_MASK: u64 = 0xFF;

/// Byte `MAIR` cho Device-nGnRnE (không gộp, không sắp xếp lại, không early-write ack).
pub const MAIR_ATTR_DEVICE_NGNRNE: u8 = 0x00;
/// Byte `MAIR` cho Device-nGnRE.
pub const MAIR_ATTR_DEVICE_NGNRE: u8 = 0x04;
/// Byte `MAIR` cho Normal, non-cacheable.
pub const MAIR_ATTR_NORMAL_NC: u8 = 0x44;
/// Byte `MAIR` cho Normal, write-through.
pub const MAIR_ATTR_NORMAL_WT: u8 = 0xBB;
/// Byte `MAIR` cho Normal, write-back, read/write-allocate (kernel dùng cho RAM).
pub const MAIR_ATTR_NORMAL_WB: u8 = 0xFF;

/// Giá trị `MAIR_EL1` — builder **const** cho 8 slot 8 bit.
///
/// Slot `i` nằm ở bit `8*i`; slot 0/1 khớp [`crate::format::MemAttr`] (`Device` = 0,
/// `Normal` = 1) nên leaf dùng `AttrIndx` 0/1 đọc đúng byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mair(u64);

impl Mair {
    /// Dựng từ 8 byte thuộc tính; slot `i` = `attrs[i]`.
    pub const fn new(attrs: [u8; MAIR_SLOTS]) -> Self {
        let mut bits = 0u64;
        let mut i = 0;
        while i < MAIR_SLOTS {
            bits |= (attrs[i] as u64) << (MAIR_SLOT_SHIFT * i as u32);
            i += 1;
        }
        Self(bits)
    }

    /// Giá trị 64 bit để ghi `MAIR_EL1`.
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// Byte thuộc tính của slot `i` (`i` lấy modulo 8).
    pub const fn slot(self, i: usize) -> u8 {
        ((self.0 >> (MAIR_SLOT_SHIFT * (i % MAIR_SLOTS) as u32)) & MAIR_SLOT_MASK) as u8
    }

    /// Trả bản sao có slot `i` = `attr` (`i` lấy modulo 8).
    pub const fn with_slot(self, i: usize, attr: u8) -> Self {
        let shift = MAIR_SLOT_SHIFT * (i % MAIR_SLOTS) as u32;
        Self((self.0 & !(MAIR_SLOT_MASK << shift)) | ((attr as u64) << shift))
    }
}

/// `MAIR_EL1` mục tiêu của kernel: slot 0 = Device-nGnRnE, slot 1 = Normal write-back
/// read/write-allocate, 6 slot còn lại 0.
///
/// Khớp [`crate::format::MAIR_EL1`] (byte 0 `0x00`, byte 1 `0xFF`); giá trị tuyệt đối
/// `0x0000_0000_0000_FF00` (test `mair_kernel_value`).
pub const MAIR_EL1_KERNEL: Mair = Mair::new([
    MAIR_ATTR_DEVICE_NGNRNE,
    MAIR_ATTR_NORMAL_WB,
    0,
    0,
    0,
    0,
    0,
    0,
]);

// =====================================================================================
// SCTLR_EL1 — System Control Register (EL1)
// =====================================================================================

/// Bit `M` — bật MMU (stage 1).
pub const SCTLR_M: u64 = 1 << 0;
/// Bit `A` — bật kiểm tra alignment khi truy cập dữ liệu.
pub const SCTLR_A: u64 = 1 << 1;
/// Bit `C` — bật data cache.
pub const SCTLR_C: u64 = 1 << 2;
/// Bit `SA` — bật kiểm tra alignment của stack pointer (EL1).
pub const SCTLR_SA: u64 = 1 << 3;
/// Bit `SA0` — bật kiểm tra alignment của stack pointer (EL0).
pub const SCTLR_SA0: u64 = 1 << 4;
/// Bit `CP15BEN` — cho phép lệnh CP15 barrier (AArch32).
pub const SCTLR_CP15BEN: u64 = 1 << 5;
/// Bit `ITD` — tắt IT instruction (AArch32).
pub const SCTLR_ITD: u64 = 1 << 7;
/// Bit `SED` — tắt SETEND (AArch32).
pub const SCTLR_SED: u64 = 1 << 8;
/// Bit `UMA` — cho phép EL0 dùng PSTATE.PAN.
pub const SCTLR_UMA: u64 = 1 << 9;
/// Bit `EOS` — bật tính năng ExS (ARMv8.5).
pub const SCTLR_EOS: u64 = 1 << 11;
/// Bit `I` — bật instruction cache.
pub const SCTLR_I: u64 = 1 << 12;
/// Bit `EnDB` — bật breakpoint debug (ARMv8.3).
pub const SCTLR_ENDB: u64 = 1 << 13;
/// Bit `DZE` — cho phép EL0 thực thi DC ZVA.
pub const SCTLR_DZE: u64 = 1 << 14;
/// Bit `UCT` — cho phép EL0 truy cập CTR_EL0.
pub const SCTLR_UCT: u64 = 1 << 15;
/// Bit `nTWI` — `1` ⇒ không trap WFI của EL0.
pub const SCTLR_NTWI: u64 = 1 << 16;
/// Bit `nTWE` — `1` ⇒ không trap WFE của EL0.
pub const SCTLR_NTWE: u64 = 1 << 18;
/// Bit `WXN` — vùng ghi được coi là execute-never.
pub const SCTLR_WXN: u64 = 1 << 19;
/// Bit `TSCXT` — trap SVC khi PSTATE.SCXT (ARMv8.7).
pub const SCTLR_TSCXT: u64 = 1 << 20;
/// Bit `IESB` — chèn implicit error synchronization barrier.
pub const SCTLR_IESB: u64 = 1 << 21;
/// Bit `EIS` — bật exception injection synchronization (ARMv8.5).
pub const SCTLR_EIS: u64 = 1 << 22;
/// Bit `SPAN` — cho phép PSTATE.PAN mặc định 1 khi vào EL1.
pub const SCTLR_SPAN: u64 = 1 << 23;
/// Bit `E0E` — endianness của truy cập dữ liệu ở EL0.
pub const SCTLR_E0E: u64 = 1 << 24;
/// Bit `EE` — endianness của truy cập dữ liệu ở EL1.
pub const SCTLR_EE: u64 = 1 << 25;
/// Bit `UCI` — cho phép EL0 thực thi cache invalidate.
pub const SCTLR_UCI: u64 = 1 << 26;
/// Bit `EnDA` — bật disable-trapping của DAIF (ARMv8.3).
pub const SCTLR_ENDA: u64 = 1 << 27;
/// Bit `nTLSMD` — tắt trap load/store multiple (AArch32).
pub const SCTLR_NTLSMD: u64 = 1 << 28;
/// Bit `LSMAOE` — load/store multiple atomicity (AArch32).
pub const SCTLR_LSMAOE: u64 = 1 << 29;
/// Bit `EnIB` — bật implicit branch target identification (ARMv8.5).
pub const SCTLR_ENIB: u64 = 1 << 30;
/// Bit `EnIA` — bật implicit BTI cho instruction (ARMv8.5).
pub const SCTLR_ENIA: u64 = 1 << 31;
/// Bit `BT0` — bật BTI ở EL0.
pub const SCTLR_BT0: u64 = 1 << 35;
/// Bit `BT1` — bật BTI ở EL1.
pub const SCTLR_BT1: u64 = 1 << 36;
/// Bit `ITFSB` — implicit tag check fault synchronization (ARMv8.5).
pub const SCTLR_ITFSB: u64 = 1 << 37;
/// Bit `ATA` — allocation tag access ở EL1 (ARMv8.5).
pub const SCTLR_ATA: u64 = 1 << 43;
/// Bit `DSSBS` — giá trị mặc định của PSTATE.DSSBS (ARMv8.5).
pub const SCTLR_DSSBS: u64 = 1 << 44;
/// Bit `EnASR` — bật atomicity/synchronization cho LS64 (ARMv8.7).
pub const SCTLR_ENASR: u64 = 1 << 54;
/// Bit `EnAS0` — bật atomicity/synchronization cho LS64 ở EL0 (ARMv8.7).
pub const SCTLR_ENAS0: u64 = 1 << 55;
/// Bit `EnALS` — bật atomicity/synchronization cho LS64 (ARMv8.7).
pub const SCTLR_ENALS: u64 = 1 << 56;
/// Bit `EPAN` — bật PAN3 (ARMv8.7).
pub const SCTLR_EPAN: u64 = 1 << 57;
/// Bit `EnTP2` — bật SME trap (ARMv9).
pub const SCTLR_ENTP2: u64 = 1 << 60;

/// Giá trị `SCTLR_EL1` — builder **const** trên các bit ở trên.
///
/// **Không** có `UXN`/`PXN`: hai bit đó là bit descriptor (xem ghi chú đầu tệp).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sctlr(u64);

impl Sctlr {
    /// Giá trị 0 (MMU/cache tắt).
    pub const fn new() -> Self {
        Self(0)
    }

    /// Giá trị 64 bit để ghi `SCTLR_EL1`.
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// Đặt/xoá bit `bit` theo `on`.
    pub const fn with(self, bit: u64, on: bool) -> Self {
        Self(if on { self.0 | bit } else { self.0 & !bit })
    }

    /// Đọc một bit.
    pub const fn bit(self, bit: u64) -> bool {
        self.0 & bit != 0
    }
}

/// Sinh các setter có tên cho từng bit `SCTLR_EL1` (tránh 40 hàm viết tay trùng lặp).
macro_rules! sctlr_setters {
    ($($name:ident => $bit:ident),* $(,)?) => {
        impl Sctlr {
            $(
                #[doc = concat!("Bật/tắt `", stringify!($bit), "`.")]
                pub const fn $name(self, on: bool) -> Self {
                    self.with($bit, on)
                }
            )*
        }
    };
}

sctlr_setters! {
    with_mmu => SCTLR_M,
    with_alignment => SCTLR_A,
    with_cache => SCTLR_C,
    with_stack_alignment => SCTLR_SA,
    with_el0_stack_alignment => SCTLR_SA0,
    with_cp15ben => SCTLR_CP15BEN,
    with_itd => SCTLR_ITD,
    with_sed => SCTLR_SED,
    with_uma => SCTLR_UMA,
    with_eos => SCTLR_EOS,
    with_icache => SCTLR_I,
    with_endb => SCTLR_ENDB,
    with_dze => SCTLR_DZE,
    with_uct => SCTLR_UCT,
    with_ntwi => SCTLR_NTWI,
    with_ntwe => SCTLR_NTWE,
    with_wxn => SCTLR_WXN,
    with_tscxt => SCTLR_TSCXT,
    with_iesb => SCTLR_IESB,
    with_eis => SCTLR_EIS,
    with_span => SCTLR_SPAN,
    with_el0_big_endian => SCTLR_E0E,
    with_el1_big_endian => SCTLR_EE,
    with_uci => SCTLR_UCI,
    with_enda => SCTLR_ENDA,
    with_ntlsmd => SCTLR_NTLSMD,
    with_lsmaoe => SCTLR_LSMAOE,
    with_enib => SCTLR_ENIB,
    with_enia => SCTLR_ENIA,
    with_bt0 => SCTLR_BT0,
    with_bt1 => SCTLR_BT1,
    with_itfsb => SCTLR_ITFSB,
    with_ata => SCTLR_ATA,
    with_dssbs => SCTLR_DSSBS,
    with_enasr => SCTLR_ENASR,
    with_enas0 => SCTLR_ENAS0,
    with_enals => SCTLR_ENALS,
    with_epan => SCTLR_EPAN,
    with_entp2 => SCTLR_ENTP2,
}

// =====================================================================================
// TTBR0_EL1 / TTBR1_EL1 — Translation Table Base Register
// =====================================================================================

/// Vị trí `ASID[15:0]` trong TTBR.
pub const TTBR_ASID_SHIFT: u32 = 48;
/// Mặt nạ `ASID[15:0]`.
pub const TTBR_ASID_MASK: u64 = 0xFFFF << TTBR_ASID_SHIFT;
/// Vị trí bit thấp của `BADDR` (bit 1 của thanh ghi).
pub const TTBR_BADDR_SHIFT: u32 = 1;
/// Mặt nạ `BADDR` cho granule 4 KiB: bit `[47:12]` (đặc tả: trường `[47:1]`, `[11:1]` RES0).
pub const TTBR_BADDR_MASK: u64 = ((1u64 << 48) - 1) & !0xFFF;
/// Bit `CnP` — copy-on-PTE (chia sẻ bảng trang giữa các PE).
pub const TTBR_CNP: u64 = 1 << 0;

/// Giá trị `TTBR0_EL1`/`TTBR1_EL1` — builder **const** (ASID + BADDR + CnP).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ttbr(u64);

impl Ttbr {
    /// Dựng TTBR với bảng gốc `baddr` (4 KiB aligned) và `asid` 16 bit.
    ///
    /// `baddr` bị cắt còn bit `[47:12]` — đúng trường `BADDR` của granule 4 KiB.
    pub const fn new(baddr: u64, asid: u16) -> Self {
        Self((baddr & TTBR_BADDR_MASK) | ((asid as u64) << TTBR_ASID_SHIFT))
    }

    /// Giá trị 64 bit để ghi TTBR.
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// Địa chỉ bảng gốc (bit `[47:12]`).
    pub const fn baddr(self) -> u64 {
        self.0 & TTBR_BADDR_MASK
    }

    /// ASID 16 bit.
    pub const fn asid(self) -> u16 {
        ((self.0 & TTBR_ASID_MASK) >> TTBR_ASID_SHIFT) as u16
    }

    /// Bit `CnP`.
    pub const fn cnp(self) -> bool {
        self.0 & TTBR_CNP != 0
    }

    /// Đặt/xoá `CnP`.
    pub const fn with_cnp(self, on: bool) -> Self {
        Self(if on {
            self.0 | TTBR_CNP
        } else {
            self.0 & !TTBR_CNP
        })
    }
}

// =====================================================================================
// VBAR_EL1 — Vector Base Address Register
// =====================================================================================

/// Vị trí `VBA[63:11]` — vector base phải 2 KiB aligned.
pub const VBAR_BASE_SHIFT: u32 = 11;
/// Mặt nạ `VBA[63:11]`.
pub const VBAR_BASE_MASK: u64 = !0x7FF;

/// Giá trị `VBAR_EL1` — builder **const**.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Vbar(u64);

impl Vbar {
    /// Dựng từ địa chỉ bảng vector (bị cắt còn bit `[63:11]`).
    pub const fn new(base: u64) -> Self {
        Self(base & VBAR_BASE_MASK)
    }

    /// Giá trị 64 bit để ghi `VBAR_EL1`.
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// Địa chỉ bảng vector (bit `[63:11]`).
    pub const fn base(self) -> u64 {
        self.0 & VBAR_BASE_MASK
    }
}

// =====================================================================================
// ESR_EL1 — Exception Syndrome Register
// =====================================================================================

/// Vị trí `EC[5:0]` — Exception Class.
pub const ESR_EC_SHIFT: u32 = 26;
/// Mặt nạ `EC[5:0]`.
pub const ESR_EC_MASK: u64 = 0x3F << ESR_EC_SHIFT;
/// Bit `IL` — độ dài instruction bị lỗi (32 bit).
pub const ESR_IL: u64 = 1 << 25;
/// Mặt nạ `ISS[24:0]`.
pub const ESR_ISS_MASK: u64 = (1 << 25) - 1;

/// Một số `EC` (Exception Class) mà kernel gặp — giá trị theo DDI 0601.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExceptionClass {
    /// `0x00` — unknown reason.
    Unknown = 0x00,
    /// `0x15` — SVC 64-bit từ AArch64.
    Svc64 = 0x15,
    /// `0x20` — instruction abort từ EL0.
    InstructionAbortLowerEl = 0x20,
    /// `0x21` — instruction abort từ EL1.
    InstructionAbortSameEl = 0x21,
    /// `0x22` — PC alignment fault.
    PcAlignment = 0x22,
    /// `0x24` — data abort từ EL0.
    DataAbortLowerEl = 0x24,
    /// `0x25` — data abort từ EL1.
    DataAbortSameEl = 0x25,
    /// `0x26` — SP alignment fault.
    SpAlignment = 0x26,
    /// `0x30` — breakpoint từ EL0.
    BreakpointLowerEl = 0x30,
    /// `0x31` — breakpoint từ EL1.
    BreakpointSameEl = 0x31,
}

impl ExceptionClass {
    /// Mã `EC[5:0]`.
    pub const fn bits(self) -> u8 {
        self as u8
    }

    /// Bóc `EC`; mã lạ trả `None` (kernel chỉ cần nhận diện các mã trên).
    pub const fn from_bits(bits: u8) -> Option<Self> {
        match bits & 0x3F {
            0x00 => Some(Self::Unknown),
            0x15 => Some(Self::Svc64),
            0x20 => Some(Self::InstructionAbortLowerEl),
            0x21 => Some(Self::InstructionAbortSameEl),
            0x22 => Some(Self::PcAlignment),
            0x24 => Some(Self::DataAbortLowerEl),
            0x25 => Some(Self::DataAbortSameEl),
            0x26 => Some(Self::SpAlignment),
            0x30 => Some(Self::BreakpointLowerEl),
            0x31 => Some(Self::BreakpointSameEl),
            _ => None,
        }
    }
}

/// Giá trị `ESR_EL1` — builder **const** (EC + IL + ISS).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Esr(u64);

impl Esr {
    /// Dựng từ `EC`, `IL` và `ISS` (25 bit thấp).
    pub const fn new(ec: ExceptionClass, il: bool, iss: u32) -> Self {
        let mut bits = ((ec.bits() as u64) << ESR_EC_SHIFT) | ((iss as u64) & ESR_ISS_MASK);
        if il {
            bits |= ESR_IL;
        }
        Self(bits)
    }

    /// Giá trị 64 bit đọc từ `ESR_EL1`.
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// `EC[5:0]` thô.
    pub const fn ec_raw(self) -> u8 {
        ((self.0 & ESR_EC_MASK) >> ESR_EC_SHIFT) as u8
    }

    /// `EC` đã bóc; mã lạ ⇒ `None`.
    pub const fn ec(self) -> Option<ExceptionClass> {
        ExceptionClass::from_bits(self.ec_raw())
    }

    /// `IL`.
    pub const fn il(self) -> bool {
        self.0 & ESR_IL != 0
    }

    /// `ISS[24:0]`.
    pub const fn iss(self) -> u32 {
        (self.0 & ESR_ISS_MASK) as u32
    }
}

// =====================================================================================
// FAR_EL1 / ELR_EL1 — địa chỉ 64 bit đầy đủ
// =====================================================================================

/// Giá trị `FAR_EL1` — Fault Address Register (địa chỉ 64 bit đầy đủ, không bitfield).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FarEl1(u64);

impl FarEl1 {
    /// Dựng từ địa chỉ lỗi.
    pub const fn new(address: u64) -> Self {
        Self(address)
    }

    /// Giá trị 64 bit đọc từ `FAR_EL1`.
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// Địa chỉ lỗi.
    pub const fn address(self) -> u64 {
        self.0
    }
}

/// Giá trị `ELR_EL1` — Exception Link Register (địa chỉ quay về sau `eret`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ElrEl1(u64);

impl ElrEl1 {
    /// Dựng từ địa chỉ quay về.
    pub const fn new(address: u64) -> Self {
        Self(address)
    }

    /// Giá trị 64 bit để ghi `ELR_EL1`.
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// Địa chỉ quay về.
    pub const fn address(self) -> u64 {
        self.0
    }
}

// =====================================================================================
// DAIF — PSTATE interrupt mask
// =====================================================================================

/// Bit `D` — debug exception mask.
pub const DAIF_D: u64 = 1 << 9;
/// Bit `A` — SError mask.
pub const DAIF_A: u64 = 1 << 8;
/// Bit `I` — IRQ mask.
pub const DAIF_I: u64 = 1 << 7;
/// Bit `F` — FIQ mask.
pub const DAIF_F: u64 = 1 << 6;
/// Cả bốn bit `D/A/I/F`.
pub const DAIF_ALL: u64 = DAIF_D | DAIF_A | DAIF_I | DAIF_F;

/// Giá trị `DAIF` — builder **const** (bit 9:6 của PSTATE).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Daif(u64);

impl Daif {
    /// Không mask gì.
    pub const fn new() -> Self {
        Self(0)
    }

    /// Mask cả `D/A/I/F`.
    pub const fn all() -> Self {
        Self(DAIF_ALL)
    }

    /// Giá trị 64 bit để ghi `DAIF`.
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// Đặt/xoá một bit mask.
    pub const fn with(self, bit: u64, masked: bool) -> Self {
        Self(if masked { self.0 | bit } else { self.0 & !bit })
    }

    /// Mask/bỏ mask `D`.
    pub const fn with_d(self, masked: bool) -> Self {
        self.with(DAIF_D, masked)
    }

    /// Mask/bỏ mask `A`.
    pub const fn with_a(self, masked: bool) -> Self {
        self.with(DAIF_A, masked)
    }

    /// Mask/bỏ mask `I`.
    pub const fn with_i(self, masked: bool) -> Self {
        self.with(DAIF_I, masked)
    }

    /// Mask/bỏ mask `F`.
    pub const fn with_f(self, masked: bool) -> Self {
        self.with(DAIF_F, masked)
    }

    /// `D` đang mask?
    pub const fn d(self) -> bool {
        self.0 & DAIF_D != 0
    }

    /// `A` đang mask?
    pub const fn a(self) -> bool {
        self.0 & DAIF_A != 0
    }

    /// `I` đang mask?
    pub const fn i(self) -> bool {
        self.0 & DAIF_I != 0
    }

    /// `F` đang mask?
    pub const fn f(self) -> bool {
        self.0 & DAIF_F != 0
    }

    /// Cả bốn bit đang mask?
    pub const fn is_all_masked(self) -> bool {
        self.0 & DAIF_ALL == DAIF_ALL
    }
}

// =====================================================================================
// SPSR_EL1 — Saved Program Status Register
// =====================================================================================

/// Bit `N` (bit 31).
pub const SPSR_N: u64 = 1 << 31;
/// Bit `Z` (bit 30).
pub const SPSR_Z: u64 = 1 << 30;
/// Bit `C` (bit 29).
pub const SPSR_C: u64 = 1 << 29;
/// Bit `V` (bit 28).
pub const SPSR_V: u64 = 1 << 28;
/// Vị trí `M[3:0]` — chế độ quay về.
pub const SPSR_M_SHIFT: u32 = 0;
/// Mặt nạ `M[3:0]`.
pub const SPSR_M_MASK: u64 = 0xF;

/// Chế độ (Exception level + stack pointer) trong `SPSR_EL1.M[3:0]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpsrMode {
    /// `0b0000` — EL0t.
    El0t = 0b0000,
    /// `0b0100` — EL1t.
    El1t = 0b0100,
    /// `0b0101` — EL1h.
    El1h = 0b0101,
    /// `0b1000` — EL2t.
    El2t = 0b1000,
    /// `0b1001` — EL2h.
    El2h = 0b1001,
    /// `0b1100` — EL3t.
    El3t = 0b1100,
    /// `0b1101` — EL3h.
    El3h = 0b1101,
}

impl SpsrMode {
    /// Mã `M[3:0]`.
    pub const fn bits(self) -> u64 {
        self as u64
    }

    /// Bóc `M[3:0]`; mã lạ ⇒ `None`.
    pub const fn from_bits(bits: u64) -> Option<Self> {
        match bits & SPSR_M_MASK {
            0b0000 => Some(Self::El0t),
            0b0100 => Some(Self::El1t),
            0b0101 => Some(Self::El1h),
            0b1000 => Some(Self::El2t),
            0b1001 => Some(Self::El2h),
            0b1100 => Some(Self::El3t),
            0b1101 => Some(Self::El3h),
            _ => None,
        }
    }
}

/// Giá trị `SPSR_EL1` — builder **const** (mode + NZCV + DAIF).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spsr(u64);

impl Spsr {
    /// Dựng từ chế độ quay về; NZCV/DAIF = 0.
    pub const fn new(mode: SpsrMode) -> Self {
        Self(mode.bits() & SPSR_M_MASK)
    }

    /// Giá trị 64 bit để ghi `SPSR_EL1`.
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// Đặt `N/Z/C/V`.
    pub const fn with_nzcv(self, n: bool, z: bool, c: bool, v: bool) -> Self {
        let mut bits = self.0;
        bits = if n { bits | SPSR_N } else { bits & !SPSR_N };
        bits = if z { bits | SPSR_Z } else { bits & !SPSR_Z };
        bits = if c { bits | SPSR_C } else { bits & !SPSR_C };
        bits = if v { bits | SPSR_V } else { bits & !SPSR_V };
        Self(bits)
    }

    /// Đặt `D/A/I/F` từ [`Daif`].
    pub const fn with_daif(self, daif: Daif) -> Self {
        Self((self.0 & !DAIF_ALL) | daif.bits())
    }

    /// `M[3:0]` thô.
    pub const fn mode_raw(self) -> u64 {
        self.0 & SPSR_M_MASK
    }

    /// Chế độ đã bóc; mã lạ ⇒ `None`.
    pub const fn mode(self) -> Option<SpsrMode> {
        SpsrMode::from_bits(self.0)
    }

    /// `N`.
    pub const fn n(self) -> bool {
        self.0 & SPSR_N != 0
    }

    /// `Z`.
    pub const fn z(self) -> bool {
        self.0 & SPSR_Z != 0
    }

    /// `C`.
    pub const fn c(self) -> bool {
        self.0 & SPSR_C != 0
    }

    /// `V`.
    pub const fn v(self) -> bool {
        self.0 & SPSR_V != 0
    }

    /// `D/A/I/F` đã bóc.
    pub const fn daif(self) -> Daif {
        Daif(self.0 & DAIF_ALL)
    }
}
