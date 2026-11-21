// SPDX-License-Identifier: MIT OR Apache-2.0
//! Trait + kiểu **dùng chung** cho bảng trang của mọi định dạng — KT-164 (thiết kế §358/§1101).
//!
//! Trước KT-164, `PageTableFormat`/`Mmu` được định nghĩa **cục bộ** trong
//! `v2/kernel/vifast-plat-aarch64` (KT-160, nợ N1) vì crate dùng chung chưa có trait. Tệp này
//! chuyển chúng về đây — nơi đã giữ hằng số hình học chung (`PTE_LEVELS`, `VA_BITS`,
//! `PAGE_SIZE_BITS`, `walk_shape`) — để riscv64 (Sv39) và aarch64 **thật sự** thay thế được cho
//! nhau: cùng một trait, hai impl ở hai crate arch, **một** thân test generic chạy cho cả hai.
//!
//! # Nội dung
//!
//! - [`PageTableFormat`]: hình học + cách mã hoá descriptor của **một** định dạng;
//! - [`Mmu`]: giao diện map/unmap/translate mà tầng trên dùng, không biết định dạng cụ thể;
//! - kiểu đi kèm: [`Perms`], [`MemAttr`], [`Shareability`], [`Descriptor`], [`Translation`],
//!   [`MapError`].
//!
//! # Vì sao ở `vifast-pte` mà không phải `vifast-plat-common`
//!
//! `vifast-pte` là crate `no_std`, **không dependency**, `#![forbid(unsafe_code)]` và đã là nhà
//! của hình học PTE dùng chung; `vifast-plat-common` lại phụ thuộc `vifast-core`/`vifast-alloc`
//! (tầng object), kéo trait xuống đó sẽ buộc crate arch phải kéo cả tầng object. Thiết kế
//! §358/§1101 cũng chỉ định `vifast-pte`.
//!
//! # Trung lập kiến trúc
//!
//! Không `cfg` theo kiến trúc (luật R2), không asm, không `unsafe`: đây là **hợp đồng dữ liệu**. Phần
//! `TTBR*`/`MAIR`/`TCR`/`TLBI` (aarch64) hay `satp`/`sfence.vma` (riscv64) nằm ở crate arch
//! (D2-17). `MemAttr`/`Shareability` ở đây là **khái niệm trung lập** (Device/Normal,
//! non/outer/inner shareable); cách mã hoá chúng thành bit là việc của từng format.

/// Quyền truy cập của một lá — dạng **độc lập arch**.
///
/// `read`/`write`/`exec` là quyền hiệu dụng; `user` là mức đặc quyền (EL0/U-mode so với
/// kernel). Ràng buộc **riêng của từng format** (AArch64 không có lá "không đọc"; Sv39 cấm
/// `W=1, R=0`) nằm ở [`PageTableFormat::perms_valid`], không nằm ở đây.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Perms {
    /// Đọc được.
    pub read: bool,
    /// Ghi được.
    pub write: bool,
    /// Thực thi được.
    pub exec: bool,
    /// Truy cập được từ mức người dùng (EL0/U-mode).
    pub user: bool,
}

impl Perms {
    /// Không quyền nào — **không hợp lệ** với mọi format (giữ để test ca âm).
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
    /// Kernel RWX (image — W^X đầy đủ là việc của tầng object/VSpace).
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

    /// `true` ⇔ có **ít nhất một** quyền `R/W/X`.
    ///
    /// Đây là điều kiện **chung** (một lá không cấp quyền nào thì vô nghĩa với mọi format).
    /// Điều kiện đủ thì tuỳ format — dùng [`PageTableFormat::perms_valid`].
    pub const fn is_valid(self) -> bool {
        self.read || self.write || self.exec
    }
}

/// Thuộc tính bộ nhớ của một lá — khái niệm **trung lập** (Device/Normal).
///
/// AArch64 dùng chỉ số này làm `AttrIndx` (byte trong `MAIR_EL1`); Sv39 chưa có khái niệm tương
/// ứng (PBMT không dùng) nên bỏ qua. Cách ánh xạ sang bit là việc của từng format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum MemAttr {
    /// Device (MMIO) — AArch64: `Device-nGnRnE`.
    Device = 0,
    /// Normal (RAM) — AArch64: write-back, read/write-allocate.
    Normal = 1,
}

impl MemAttr {
    /// Chỉ số thuộc tính (AArch64: `AttrIndx`/byte `MAIR_EL1`).
    pub const fn index(self) -> u64 {
        self as u64
    }

    /// Bóc chỉ số thuộc tính; `None` nếu chỉ số chưa được định nghĩa.
    pub const fn from_index(index: u64) -> Option<Self> {
        match index {
            0 => Some(Self::Device),
            1 => Some(Self::Normal),
            _ => None,
        }
    }
}

/// Shareability của một lá — khái niệm **trung lập** (AArch64: `SH[1:0]`).
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
    /// Bit `SH[1:0]` (AArch64).
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

/// Kết quả bóc một descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Descriptor {
    /// Entry trống (`V = 0`).
    Invalid,
    /// Hợp lệ nhưng mã hoá không dùng được (reserved/không nhất quán với mức).
    Reserved,
    /// Con trỏ tới bảng mức kế tiếp (chỉ mức `0..LEVELS-1`).
    Table {
        /// Địa chỉ vật lý bảng con (đã bỏ bit offset).
        pa: u64,
    },
    /// Lá ở mức cuối.
    Leaf {
        /// Địa chỉ vật lý trang (đã bỏ bit offset).
        pa: u64,
        /// Quyền đọc/ghi/thực thi/user.
        perms: Perms,
    },
    /// Block/superpage (AArch64 block 2 MiB/1 GiB; Sv39 superpage 2 MiB/1 GiB).
    Block {
        /// Địa chỉ vật lý khối (đáy block, đã căn theo cỡ block).
        pa: u64,
        /// Mức của block (theo quy ước của [`PageTableFormat`]: 0 = gốc).
        level: usize,
        /// Quyền đọc/ghi/thực thi/user của **cả** block.
        ///
        /// **KT-305**: block descriptor mang **cùng** bộ bit quyền với lá (AArch64 `AP`/`UXN`/
        /// `PXN`, x86_64 `RW`/`US`/`NX`, Sv39 `R/W/X/U`), nên `translate` trả được quyền thật
        /// thay vì bịa. Trước KT-305 trường này không có vì `translate` từ chối block.
        perms: Perms,
    },
}

/// Lỗi của `Mmu` — lỗi **nội bộ task** (không phải mã lỗi ABI; `spec/` không mở — D2-72 Q1).
///
/// Đây là **hợp** của các lỗi mà hai định dạng cần; mỗi format chỉ dùng một tập con và tài liệu
/// của nó nói rõ tập nào. Gộp một enum để tầng trên (và test generic) so sánh lỗi mà không phải
/// biết định dạng — đúng mục tiêu "hai arch thay thế được cho nhau" của KT-164.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapError {
    /// Hết frame cho bảng trung gian/gốc (nguồn frame động — AArch64).
    OutOfFrames,
    /// Hết chỗ trong arena (nguồn bảng tĩnh — Sv39 `PageTableBuilder`).
    OutOfArena,
    /// VA/PA không căn lề theo cỡ trang.
    Unaligned,
    /// Cấp leaf ngoài `0..LEVELS` (Sv39 map nhiều cỡ lá).
    LevelOutOfRange,
    /// VA không canonical theo `VA_BITS`.
    NotCanonical,
    /// PA vượt `PA_BITS`.
    PaOutOfRange,
    /// Quyền không biểu diễn được bằng descriptor của format.
    InvalidPerms,
    /// Cờ PTE không hợp lệ (`V=0`, không quyền nào, hoặc `W=1` mà `R=0`).
    InvalidFlags,
    /// VA đã có mapping (lá hoặc block phủ lên).
    AlreadyMapped,
    /// VA chưa có mapping.
    NotMapped,
    /// Gặp block/superpage — tầng này chỉ tạo/nhận lá ở mức cuối.
    BlockUnsupported,
    /// Bảng trang hỏng (entry reserved, PA ngoài nguồn, ghi thất bại).
    CorruptTable,
}

/// Kết quả dịch VA → PA.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Translation {
    /// Địa chỉ vật lý (đáy trang).
    pub pa: u64,
    /// Quyền của lá.
    pub perms: Perms,
    /// Mức của lá theo quy ước của [`PageTableFormat`] (0 = gốc, `LEVELS - 1` = lá).
    pub level: usize,
}

/// Hình học + cách mã hoá descriptor của **một** định dạng bảng trang.
///
/// Cài đặt ở crate arch: `vifast-plat-aarch64` cho AArch64 VMSA 4 KiB/4 mức,
/// `vifast-plat-common` cho Sv39 3 mức. `Mmu` ([`Mmu`]) chỉ dùng trait này nên **không** biết
/// định dạng cụ thể — đó là bằng chứng "dùng chung" của KT-164.
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

    /// Mã hoá descriptor **lá** ở mức cuối cho `pa` với quyền/thuộc tính/shareability.
    fn encode_leaf(pa: u64, perms: Perms, attr: MemAttr, share: Shareability) -> u64;

    /// Cỡ **block** (byte) mà format hỗ trợ ở `level` — `None` nếu format **không** có block ở
    /// mức ấy.
    ///
    /// Đây là **năng lực** mà `translate`/`unmap` dùng để quyết định: gặp `Descriptor::Block` ở
    /// mức `level` mà `block_span(level)` là `Some` ⇒ dịch/chia nhỏ được; `None` ⇒
    /// [`MapError::BlockUnsupported`] (giữ nguyên hành vi cũ cho format chỉ map lá).
    ///
    /// Mặc định `None`: ba format hiện có chỉ map lá 4 KiB; KT-302 mở block cho AArch64 (2 MiB ở
    /// mức 2, 1 GiB ở mức 1) và KT-305 mở `translate`/`unmap` cho đúng những mức ấy. Nhờ mặc định
    /// này, x86_64/Sv39 **không** phải sửa mà vẫn từ chối block như trước.
    ///
    /// Hợp đồng: `Some(span)` ⇔ [`Self::encode_block`] mã hoá được ở `level` **và**
    /// [`Self::split_block`] chia được block ở `level` (test `block_span_matches_encode_block`
    /// chốt điều này cho AArch64).
    fn block_span(level: usize) -> Option<u64> {
        let _ = level;
        None
    }

    /// Mã hoá descriptor **block** (superpage) ở `level` cho `pa` — `None` nếu format **không**
    /// hỗ trợ block ở mức ấy.
    ///
    /// Mặc định trả `None`: ba format hiện có chỉ map lá 4 KiB, nên thêm block là **tuỳ chọn**
    /// của từng format (KT-302 mở cho AArch64: block 2 MiB ở mức 2, 1 GiB ở mức 1). Nhờ mặc định
    /// này, `PageTable::map_block` **không** cần sửa impl nào khác và trả
    /// [`MapError::BlockUnsupported`] thay vì bịa descriptor.
    ///
    /// Hợp đồng: `pa` phải căn theo cỡ block của `level` — [`PageTable::map_block`] kiểm trước
    /// khi gọi; hàm này chỉ mã hoá bit.
    fn encode_block(
        level: usize,
        pa: u64,
        perms: Perms,
        attr: MemAttr,
        share: Shareability,
    ) -> Option<u64> {
        let _ = (level, pa, perms, attr, share);
        None
    }

    /// Mã hoá **entry con thứ `index`** khi **chia nhỏ** một block ở `level` — `None` nếu format
    /// không chia được.
    ///
    /// `entry` là **nguyên** descriptor block đọc từ bảng (đã giải mã thành `Descriptor::Block`);
    /// `index` là chỉ số con trong bảng mới (`0..ENTRIES`). Kết quả là descriptor của **mức
    /// `level + 1`**: block (nếu `level + 1` vẫn là mức block) hoặc lá 4 KiB (nếu `level + 1` là
    /// mức lá). PA con = PA block + `index × span(level + 1)`.
    ///
    /// **KT-305**: `PageTable::unmap` dùng hàm này để **chia nhỏ** block thay vì xoá cả block —
    /// xoá cả block là mất mapping của 511 trang kế cận. Mặc định `None` ⇒ `unmap` trả
    /// [`MapError::BlockUnsupported`] (hành vi cũ của x86_64/Sv39).
    fn split_block(entry: u64, level: usize, index: usize) -> Option<u64> {
        let _ = (entry, level, index);
        None
    }

    /// Bóc descriptor tại `level`.
    fn decode(entry: u64, level: usize) -> Descriptor;

    /// `perms` có biểu diễn được bằng descriptor của format này không.
    ///
    /// Mặc định: phải có ít nhất một quyền ([`Perms::is_valid`]). Format có ràng buộc riêng
    /// **override**: AArch64 không có lá "không đọc" (`read` bắt buộc); Sv39 cấm `W=1, R=0`.
    fn perms_valid(perms: Perms) -> bool {
        perms.is_valid()
    }
}

/// Giao diện `Mmu` của thiết kế §183 (`type Fmt: PageTableFormat`) — phần **dữ liệu**.
///
/// Tầng trên (VSpace, test generic) gọi qua trait này; crate arch cài cho cấu trúc dữ liệu của
/// mình (`PageTable<F, M>` ở `vifast-plat-aarch64`, `PageTableBuilder` ở `vifast-plat-common`).
pub trait Mmu {
    /// Định dạng bảng trang.
    type Fmt: PageTableFormat;

    /// PA bảng gốc.
    fn root(&self) -> u64;

    /// Map một trang ở mức cuối với thuộc tính mặc định.
    fn map(&mut self, va: u64, pa: u64, perms: Perms) -> Result<(), MapError>;

    /// Bỏ mapping một trang ở mức cuối, trả PA.
    fn unmap(&mut self, va: u64) -> Result<u64, MapError>;

    /// Dịch VA → PA.
    fn translate(&self, va: u64) -> Result<Translation, MapError>;
}

/// `va` canonical theo `va_bits` bit: bit `va_bits - 1` chép vào mọi bit cao hơn.
///
/// Hàm **thuần**, không panic với mọi `u64`; `va_bits` ngoài `1..64` coi như không ràng buộc.
/// Dùng chung cho AArch64 (39/48 bit) và Sv39 (39 bit) — test `format_48_bit_va` chốt điều này.
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
