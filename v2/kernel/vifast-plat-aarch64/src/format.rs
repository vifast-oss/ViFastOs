// SPDX-License-Identifier: MIT OR Apache-2.0
//! Định dạng bảng trang **AArch64 VMSA** (4 KiB granule, 4 mức) — KT-160 / D2-147.
//!
//! Tệp này là phần **thuần dữ liệu**: mô tả descriptor 64-bit của AArch64, hình học bảng trang
//! và cách bóc/ghép bit. **Không** asm, **không** `cfg(target_arch)`, **không** `unsafe` — nhờ vậy
//! chạy được trên host (D2-72 Q12) và là bằng chứng đo được của K4' bước 1. Phần `TTBR0_EL1`/
//! `TTBR1_EL1`/`MAIR_EL1`/`TCR_EL1`/`TLBI`/`dsb`/`isb` thuộc **bước 2** (crate arch, D2-17).
//!
//! # Hình học (D2-147)
//!
//! ```text
//!   granule        4 KiB (12 bit offset)
//!   số mức         4 (level 0 = gốc, level 3 = lá 4 KiB)
//!   chỉ số/mức     9 bit (512 entry × 8 B = 4 KiB/bảng)
//!   VA             39 bit **canonical** (bit 38 chép vào bit 39..63)
//!   PA             48 bit (bit 47:12 của descriptor)
//! ```
//!
//! **Vì sao 39-bit VA mà vẫn 4 mức?** AArch64 4 KiB granule có 4 mức là **48-bit VA** đầy đủ
//! (thiết kế §541: "4 KiB granule, 48 bit, TTBR0/1"). D2-147 chốt cấu hình kernel dùng **39-bit
//! canonical** — giống hình Sv39 của riscv64 và giống cách x86_64 dùng 4 cấp với VA canonical
//! 39-bit (K5'). Với VA 39-bit canonical, chỉ số mức 0 (bit 47:39) chỉ có **hai** giá trị hợp lệ:
//! `0` cho nửa thấp và `0x1FF` cho nửa cao — walk vẫn đi **đủ 4 mức**, đúng yêu cầu "bảng trang
//! 4 mức đi đúng cấp". Format vẫn **có khả năng** 48-bit VA ([`AARCH64_MAX_VA_BITS`]); đổi
//! `VA_BITS` là đổi một hằng số, không đổi hình walk (xem test `format_48_bit_va`).
//!
//! # Descriptor 64-bit (ARMv8-A, stage 1, 4 KiB granule)
//!
//! ```text
//!  63      59 58 55 54 53 52 51      48 47                    12 11 10 9 8 7 6 5 4 3 2 1 0
//! [ PBHA ][RES0][UXN][PXN][C][DBM][   addr[47:12]            ][nG][AF][SH][AP][NS][Attr][1][V]
//! ```
//!
//! - `V` bit 0: entry hợp lệ; `bit1 = 1` là page (mức 3) hoặc table (mức 0..2), `bit1 = 0` +
//!   `bit0 = 1` là **block** (2 MiB ở mức 2, 1 GiB ở mức 1) — bước 1 **không tạo** block nhưng
//!   [`Aarch64::decode`] nhận diện được để walker không nói dối;
//! - `AttrIndx[2:0]` bit 4:2: chỉ số byte trong `MAIR_EL1` ([`MemAttr`]);
//! - `AP[2:1]` bit 7:6: `AP[2] = 1` ⇒ chỉ đọc; `AP[1] = 1` ⇒ EL0 truy cập được ([`Perms`]);
//! - `SH[1:0]` bit 9:8: shareability ([`Shareability`]; `0b01` là reserved);
//! - `AF` bit 10: access flag — **bắt buộc 1** với leaf, thiếu thì mọi truy cập fault;
//! - `nG` bit 11: non-global — leaf **user** phải đặt để `TLBI ASIDE1` xoá được (bài học v1
//!   P0-1/r289: thiếu `nG` thì hai VSpace cùng VA thấy entry của nhau, QEMU xanh mà máy thật sai);
//! - `PXN` bit 53 / `UXN` bit 54: cấm thực thi ở EL1 / EL0. Kernel exec: `PXN=0, UXN=1`; user
//!   exec: `PXN=1, UXN=0`; không exec: cả hai bằng 1.
//!
//! # Quan hệ với `vifast-pte` / `vifast-plat-common`
//!
//! Tại `6b2fdce7` **chưa có** trait `PageTableFormat` trong `vifast-pte` hay
//! `vifast-plat-common` (thiết kế §358/§1101 mới chỉ **đặt chỗ** cho nó ở `vifast-pte`; hiện
//! `vifast-pte` là hình Sv39 3 mức, `vifast-plat-common::mmu` là `PageTableBuilder` Sv39 cụ
//! thể). Bước 1 vì vậy **định nghĩa trait tại đây** ([`PageTableFormat`]) và cài cho [`Aarch64`];
//! khi K4' bước 2 mở, trait chuyển lên `vifast-pte` theo thiết kế và crate này chỉ còn impl —
//! **nợ có tên** ghi ở `v2/agent/JOURNAL/kt160-aarch64.md` §6. Không sửa `vifast-pte`/
//! `vifast-plat-common` đêm nay để không đụng L0 của cổng (D2-147, D2-148).

/// Số mức bảng trang của AArch64 4 KiB granule.
pub const AARCH64_LEVELS: usize = 4;
/// Số bit offset trong trang (4 KiB).
pub const AARCH64_PAGE_BITS: u32 = 12;
/// Số bit chỉ số mỗi mức (512 entry).
pub const AARCH64_INDEX_BITS: u32 = 9;
/// Số bit VA **canonical** mà kernel dùng (D2-147).
pub const AARCH64_VA_BITS: u32 = 39;
/// Số bit VA **tối đa** của format 4 mức 4 KiB (thiết kế §541: 48 bit).
pub const AARCH64_MAX_VA_BITS: u32 = 48;
/// Số bit địa chỉ vật lý (PA) trong descriptor.
pub const AARCH64_PA_BITS: u32 = 48;
/// Cỡ một trang/bảng: 4 KiB.
pub const AARCH64_PAGE_BYTES: u64 = 1 << AARCH64_PAGE_BITS;
/// Số entry một bảng trang (`2^9 = 512`).
pub const AARCH64_ENTRIES: usize = 1 << AARCH64_INDEX_BITS;

/// Bit `V` — descriptor hợp lệ.
pub const DESC_VALID: u64 = 1 << 0;
/// Bit 1 — `1` với table (mức 0..2) và page (mức 3); `0` với block.
pub const DESC_TABLE_OR_PAGE: u64 = 1 << 1;
/// Vị trí `AttrIndx[2:0]` (bit 4:2).
pub const DESC_ATTR_SHIFT: u32 = 2;
/// Bit `NS` (non-secure) — bước 1 luôn 0 (EL1 non-secure, không dùng Secure world).
pub const DESC_NS: u64 = 1 << 5;
/// Bit `AP[1]` — `1` ⇒ EL0 (user) truy cập được.
pub const DESC_AP_USER: u64 = 1 << 6;
/// Bit `AP[2]` — `1` ⇒ chỉ đọc (read-only).
pub const DESC_AP_RO: u64 = 1 << 7;
/// Vị trí `SH[1:0]` (bit 9:8).
pub const DESC_SH_SHIFT: u32 = 8;
/// Bit `AF` — access flag, **bắt buộc 1** với leaf.
pub const DESC_AF: u64 = 1 << 10;
/// Bit `nG` — non-global (leaf user).
pub const DESC_NG: u64 = 1 << 11;
/// Vị trí địa chỉ vật lý trong descriptor (bit 12).
pub const DESC_ADDR_SHIFT: u32 = 12;
/// Mặt nạ địa chỉ vật lý 48-bit đã bỏ 12 bit offset.
pub const DESC_ADDR_MASK: u64 = ((1u64 << AARCH64_PA_BITS) - 1) & !(AARCH64_PAGE_BYTES - 1);
/// Bit `PXN` — privileged execute-never (EL1).
pub const DESC_PXN: u64 = 1 << 53;
/// Bit `UXN` — unprivileged execute-never (EL0).
pub const DESC_UXN: u64 = 1 << 54;

/// Byte `MAIR_EL1` cho Device-nGnRnE (chỉ số 0).
pub const MAIR_DEVICE_NGNRNE: u64 = 0x00;
/// Byte `MAIR_EL1` cho Normal, write-back, read/write-allocate (chỉ số 1).
pub const MAIR_NORMAL_WB_RW_ALLOC: u64 = 0xFF;
/// Giá trị `MAIR_EL1` mà bước 2 sẽ nạp: byte 0 Device-nGnRnE, byte 1 Normal WB.
///
/// Bước 1 chỉ **dữ liệu**: hằng số này để test chốt "chỉ số MAIR mà leaf dùng có mặt trong
/// `MAIR_EL1` đã định nghĩa" — nếu ai đổi [`MemAttr`] mà quên `MAIR_EL1` thì test đỏ.
pub const MAIR_EL1: u64 = MAIR_DEVICE_NGNRNE | (MAIR_NORMAL_WB_RW_ALLOC << 8);

/// Thuộc tính bộ nhớ của một leaf — ánh xạ 1-1 sang chỉ số byte trong `MAIR_EL1`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum MemAttr {
    /// Device-nGnRnE (MMIO): byte 0 của [`MAIR_EL1`].
    Device = 0,
    /// Normal write-back RW-allocate (RAM): byte 1 của [`MAIR_EL1`].
    Normal = 1,
}

impl MemAttr {
    /// Chỉ số `AttrIndx` (bit 4:2 của descriptor).
    pub const fn mair_index(self) -> u64 {
        self as u64
    }

    /// Bóc `AttrIndx` từ một descriptor leaf; `None` nếu chỉ số chưa được định nghĩa.
    pub const fn from_index(index: u64) -> Option<Self> {
        match index {
            0 => Some(Self::Device),
            1 => Some(Self::Normal),
            _ => None,
        }
    }

    /// Byte `MAIR_EL1` tương ứng (để test đối chiếu).
    pub const fn mair_byte(self) -> u64 {
        (MAIR_EL1 >> (8 * self.mair_index())) & 0xFF
    }
}

/// Shareability của một leaf (`SH[1:0]`, bit 9:8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Shareability {
    /// `0b00` — non-shareable.
    NonShareable = 0b00,
    /// `0b10` — outer shareable.
    Outer = 0b10,
    /// `0b11` — inner shareable (kernel dùng mặc định).
    Inner = 0b11,
}

impl Shareability {
    /// Bit `SH[1:0]`.
    pub const fn bits(self) -> u64 {
        self as u64
    }

    /// Bóc `SH[1:0]`; `0b01` là reserved nên trả `None`.
    pub const fn from_bits(bits: u64) -> Option<Self> {
        match bits & 0b11 {
            0b00 => Some(Self::NonShareable),
            0b10 => Some(Self::Outer),
            0b11 => Some(Self::Inner),
            _ => None,
        }
    }
}

/// Quyền truy cập của một leaf — dạng **độc lập arch** để `Mmu` dùng chung.
///
/// AArch64 **không có** bit `R` riêng: `AP[2]` chọn RW/RO, `AP[1]` chọn EL0/EL1, nên mọi leaf
/// hợp lệ đều **đọc được**. Vì vậy `read = false` là quyền **không biểu diễn được** ⇒
/// [`Perms::is_valid`] trả `false` (map trả `InvalidPerms`), thay vì lặng lẽ cấp quyền đọc.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Perms {
    /// Đọc được (luôn `true` với leaf hợp lệ của AArch64).
    pub read: bool,
    /// Ghi được (`AP[2] = 0`).
    pub write: bool,
    /// Thực thi được (EL0 nếu `user`, EL1 nếu kernel).
    pub exec: bool,
    /// EL0 truy cập được (`AP[1] = 1`).
    pub user: bool,
}

impl Perms {
    /// Không quyền nào — **không hợp lệ** (giữ để test ca âm).
    pub const NONE: Self = Self {
        read: false,
        write: false,
        exec: false,
        user: false,
    };
    /// Kernel RW, NX (data/stack/physmap).
    pub const KERNEL_RW: Self = Self {
        read: true,
        write: true,
        exec: false,
        user: false,
    };
    /// Kernel RX (text).
    pub const KERNEL_RX: Self = Self {
        read: true,
        write: false,
        exec: true,
        user: false,
    };
    /// Kernel RWX (image — W^X đầy đủ là bước sau).
    pub const KERNEL_RWX: Self = Self {
        read: true,
        write: true,
        exec: true,
        user: false,
    };
    /// User RW, NX.
    pub const USER_RW: Self = Self {
        read: true,
        write: true,
        exec: false,
        user: true,
    };
    /// User RX.
    pub const USER_RX: Self = Self {
        read: true,
        write: false,
        exec: true,
        user: true,
    };
    /// User RO, NX.
    pub const USER_RO: Self = Self {
        read: true,
        write: false,
        exec: false,
        user: true,
    };

    /// Tạo từ bốn cờ.
    pub const fn new(read: bool, write: bool, exec: bool, user: bool) -> Self {
        Self {
            read,
            write,
            exec,
            user,
        }
    }

    /// `true` ⇔ biểu diễn được bằng descriptor AArch64 (xem doc của [`Perms`]).
    pub const fn is_valid(self) -> bool {
        self.read
    }
}

/// Kết quả bóc một descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Descriptor {
    /// `V = 0` — entry trống.
    Invalid,
    /// Hợp lệ nhưng mã hoá không dùng được (reserved/không nhất quán với mức).
    Reserved,
    /// Con trỏ tới bảng mức kế tiếp (chỉ mức 0..2).
    Table {
        /// Địa chỉ vật lý bảng con (đã bỏ 12 bit offset).
        pa: u64,
    },
    /// Lá 4 KiB (chỉ mức 3).
    Leaf {
        /// Địa chỉ vật lý trang (đã bỏ 12 bit offset).
        pa: u64,
        /// Quyền đọc/ghi/thực thi/user.
        perms: Perms,
    },
    /// Block descriptor (2 MiB ở mức 2, 1 GiB ở mức 1) — bước 1 **không tạo**, chỉ nhận diện.
    Block {
        /// Địa chỉ vật lý khối.
        pa: u64,
        /// Mức của block.
        level: usize,
    },
}

/// Hình học + cách mã hoá descriptor của một định dạng bảng trang.
///
/// Đây là trait mà thiết kế §358/§1101 đặt ở `vifast-pte`; bước 1 định nghĩa **tại crate này**
/// (xem doc đầu tệp). `Mmu` ([`crate::mmu::PageTable`]) chỉ dùng trait này, không biết AArch64 —
/// test `mmu_generic_over_second_format` chứng minh bằng một format thứ hai.
pub trait PageTableFormat {
    /// Số mức (gốc = 0, lá = `LEVELS - 1`).
    const LEVELS: usize;
    /// Số bit offset trong trang.
    const PAGE_BITS: u32;
    /// Số bit chỉ số mỗi mức.
    const INDEX_BITS: u32;
    /// Số bit VA canonical.
    const VA_BITS: u32;
    /// Số bit địa chỉ vật lý.
    const PA_BITS: u32;
    /// Cỡ trang (byte).
    const PAGE_BYTES: u64;
    /// Số entry mỗi bảng.
    const ENTRIES: usize;

    /// Chỉ số entry của `va` tại `level` (level ngoài dải ⇒ 0).
    fn index(va: u64, level: usize) -> usize;

    /// `va` có canonical theo [`Self::VA_BITS`] không.
    fn is_canonical(va: u64) -> bool;

    /// Mã hoá descriptor **bảng** trỏ tới `pa`.
    fn encode_table(pa: u64) -> u64;

    /// Mã hoá descriptor **lá** 4 KiB cho `pa` với quyền/thuộc tính/shareability.
    fn encode_leaf(pa: u64, perms: Perms, attr: MemAttr, share: Shareability) -> u64;

    /// Bóc descriptor tại `level`.
    fn decode(entry: u64, level: usize) -> Descriptor;
}

/// `va` canonical theo `va_bits` bit: bit `va_bits - 1` chép vào mọi bit cao hơn.
///
/// Hàm **thuần**, không panic với mọi `u64`; `va_bits` ngoài `1..64` coi như không ràng buộc.
/// Tách khỏi [`Aarch64`] để format 48-bit (thiết kế §541) dùng lại được — test
/// `format_48_bit_va` chốt điều này.
pub const fn is_canonical_bits(va: u64, va_bits: u32) -> bool {
    if va_bits == 0 || va_bits >= 64 {
        return true;
    }
    let sign = (va >> (va_bits - 1)) & 1;
    let upper = va >> va_bits;
    if sign == 1 {
        upper == (1u64 << (64 - va_bits)) - 1
    } else {
        upper == 0
    }
}

/// Định dạng AArch64 VMSA 4 KiB granule, 4 mức, 39-bit VA canonical, 48-bit PA (D2-147).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Aarch64;

impl Aarch64 {
    /// Chỉ số entry của `va` tại `level` — `level` ngoài `0..4` trả `0` (hàm thuần, không panic).
    pub const fn index(va: u64, level: usize) -> usize {
        if level >= AARCH64_LEVELS {
            return 0;
        }
        let shift =
            AARCH64_PAGE_BITS + AARCH64_INDEX_BITS * (AARCH64_LEVELS as u32 - 1 - level as u32);
        ((va >> shift) & (AARCH64_ENTRIES as u64 - 1)) as usize
    }

    /// `va` canonical theo 39 bit: bit 38 chép vào bit 39..63.
    pub const fn is_canonical(va: u64) -> bool {
        is_canonical_bits(va, AARCH64_VA_BITS)
    }

    /// Cỡ vùng VA mà một entry ở `level` phủ (`level` ngoài dải ⇒ 0).
    pub const fn span(level: usize) -> u64 {
        if level >= AARCH64_LEVELS {
            return 0;
        }
        1u64 << (AARCH64_PAGE_BITS
            + AARCH64_INDEX_BITS * (AARCH64_LEVELS as u32 - 1 - level as u32))
    }

    /// Descriptor bảng trỏ tới `pa` (`V | bit1 | addr`).
    pub const fn encode_table(pa: u64) -> u64 {
        DESC_VALID | DESC_TABLE_OR_PAGE | (pa & DESC_ADDR_MASK)
    }

    /// Descriptor lá 4 KiB.
    ///
    /// Bất biến **bắt buộc** (test `descriptor_bits_aarch64` chốt từng bit):
    /// - `AF = 1` — thiếu thì mọi truy cập fault;
    /// - `AP[2] = !write`, `AP[1] = user`;
    /// - `nG = user` — leaf user phải non-global;
    /// - `UXN = !(exec && user)`, `PXN = !(exec && !user)` — kernel exec chỉ ở EL1, user exec chỉ
    ///   ở EL0, không exec thì cấm cả hai;
    /// - `AttrIndx = attr`, `SH = share`.
    pub const fn encode_leaf(pa: u64, perms: Perms, attr: MemAttr, share: Shareability) -> u64 {
        let mut d = DESC_VALID | DESC_TABLE_OR_PAGE | DESC_AF;
        d |= attr.mair_index() << DESC_ATTR_SHIFT;
        d |= share.bits() << DESC_SH_SHIFT;
        if !perms.write {
            d |= DESC_AP_RO;
        }
        if perms.user {
            d |= DESC_AP_USER | DESC_NG;
        }
        if !(perms.exec && perms.user) {
            d |= DESC_UXN;
        }
        if !(perms.exec && !perms.user) {
            d |= DESC_PXN;
        }
        d | (pa & DESC_ADDR_MASK)
    }

    /// Bóc descriptor tại `level` (0 = gốc, 3 = lá).
    pub const fn decode(entry: u64, level: usize) -> Descriptor {
        if entry & DESC_VALID == 0 {
            return Descriptor::Invalid;
        }
        let pa = entry & DESC_ADDR_MASK;
        let kind = entry & 0b11;
        if level >= AARCH64_LEVELS {
            return Descriptor::Reserved;
        }
        if level + 1 == AARCH64_LEVELS {
            // Mức lá: chỉ page descriptor (0b11) hợp lệ; 0b01/0b10 reserved.
            if kind == 0b11 {
                Descriptor::Leaf {
                    pa,
                    perms: decode_perms(entry),
                }
            } else {
                Descriptor::Reserved
            }
        } else if kind == 0b11 {
            Descriptor::Table { pa }
        } else if kind == 0b01 && level >= 1 {
            // Block 1 GiB (mức 1) / 2 MiB (mức 2); mức 0 không có block.
            Descriptor::Block { pa, level }
        } else {
            Descriptor::Reserved
        }
    }

    /// Chỉ số `MAIR` của một descriptor leaf.
    pub const fn leaf_mair_index(entry: u64) -> u64 {
        (entry >> DESC_ATTR_SHIFT) & 0b111
    }

    /// Shareability của một descriptor leaf (`None` nếu reserved).
    pub const fn leaf_shareability(entry: u64) -> Option<Shareability> {
        Shareability::from_bits(entry >> DESC_SH_SHIFT)
    }
}

/// Bóc quyền từ descriptor leaf (dùng chung cho [`Aarch64::decode`]).
const fn decode_perms(entry: u64) -> Perms {
    let user = entry & DESC_AP_USER != 0;
    let write = entry & DESC_AP_RO == 0;
    let exec = if user {
        entry & DESC_UXN == 0
    } else {
        entry & DESC_PXN == 0
    };
    Perms {
        read: true,
        write,
        exec,
        user,
    }
}

impl PageTableFormat for Aarch64 {
    const LEVELS: usize = AARCH64_LEVELS;
    const PAGE_BITS: u32 = AARCH64_PAGE_BITS;
    const INDEX_BITS: u32 = AARCH64_INDEX_BITS;
    const VA_BITS: u32 = AARCH64_VA_BITS;
    const PA_BITS: u32 = AARCH64_PA_BITS;
    const PAGE_BYTES: u64 = AARCH64_PAGE_BYTES;
    const ENTRIES: usize = AARCH64_ENTRIES;

    fn index(va: u64, level: usize) -> usize {
        Aarch64::index(va, level)
    }

    fn is_canonical(va: u64) -> bool {
        Aarch64::is_canonical(va)
    }

    fn encode_table(pa: u64) -> u64 {
        Aarch64::encode_table(pa)
    }

    fn encode_leaf(pa: u64, perms: Perms, attr: MemAttr, share: Shareability) -> u64 {
        Aarch64::encode_leaf(pa, perms, attr, share)
    }

    fn decode(entry: u64, level: usize) -> Descriptor {
        Aarch64::decode(entry, level)
    }
}
