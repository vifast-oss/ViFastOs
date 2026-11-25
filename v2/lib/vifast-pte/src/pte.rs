// SPDX-License-Identifier: MIT OR Apache-2.0
//! PTE thô + mảng PTE cố định (KT-35).
//!
//! [`Pte`] là **newtype `u64` thô**: KT-35 **không** gán nghĩa bit (rights/attrs/PPN thuộc
//! KT-37) để không chốt nhầm layout trước khi có task map/unmap. [`PteArray`] là bảng cố định
//! `N` entry, không cấp phát, không `unsafe`; `PteArray<512>` có **đúng** 4 KiB — cỡ một bảng
//! trang chuẩn của hình 3 cấp.

/// Một entry PTE thô 64 bit.
///
/// `Pte` chỉ giữ `u64`; việc giải mã/ghi mã quyền và địa chỉ vật lý là việc của KT-37. Ở K2
/// `Pte::EMPTY` (`0`) nghĩa là entry chưa dùng — KT-35 **không** định nghĩa bit "valid".
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Pte(u64);

impl Pte {
    /// Entry rỗng (`0`) — giá trị khởi tạo của [`PteArray::new`].
    pub const EMPTY: Pte = Pte(0);

    /// Bọc một `u64` thô (không kiểm bit, không chuẩn hoá).
    pub const fn from_raw(raw: u64) -> Self {
        Pte(raw)
    }

    /// Trả `u64` thô đã bọc.
    pub const fn raw(self) -> u64 {
        self.0
    }

    /// `true` ⇔ entry đúng bằng [`Pte::EMPTY`].
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl core::fmt::Debug for Pte {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Pte(0x{:016x})", self.0)
    }
}

impl From<u64> for Pte {
    fn from(raw: u64) -> Self {
        Pte::from_raw(raw)
    }
}

impl From<Pte> for u64 {
    fn from(pte: Pte) -> Self {
        pte.raw()
    }
}

/// Mảng PTE cố định `N` entry: đúng cỡ `N × 8` byte, không cấp phát động.
///
/// `PteArray<512>` = 4 KiB (một bảng trang); `PteArray<0>` là kiểu ZST. Mọi truy cập dùng
/// [`PteArray::get`] / [`PteArray::set`] với `Option`: chỉ số ngoài biên **không** panic và
/// **không** ghi gì.
#[derive(Clone, PartialEq, Eq)]
pub struct PteArray<const N: usize> {
    entries: [Pte; N],
}

impl<const N: usize> PteArray<N> {
    /// Bảng rỗng: mọi entry là [`Pte::EMPTY`].
    pub const fn new() -> Self {
        PteArray {
            entries: [Pte::EMPTY; N],
        }
    }

    /// Số entry (`N`).
    pub const fn len(&self) -> usize {
        N
    }

    /// `true` ⇔ `N == 0`.
    pub const fn is_empty(&self) -> bool {
        N == 0
    }

    /// Cỡ theo byte của cả mảng (`N × 8`).
    pub const fn byte_size(&self) -> usize {
        N * core::mem::size_of::<Pte>()
    }

    /// Toàn bộ entry dưới dạng mảng (không cấp phát, không copy).
    pub const fn as_array(&self) -> &[Pte; N] {
        &self.entries
    }

    /// Entry tại `index`; `None` khi ngoài biên.
    pub fn get(&self, index: usize) -> Option<Pte> {
        self.entries.get(index).copied()
    }

    /// Ghi `pte` vào `index`, trả **entry cũ**; `None` khi ngoài biên (không ghi gì).
    pub fn set(&mut self, index: usize, pte: Pte) -> Option<Pte> {
        self.entries
            .get_mut(index)
            .map(|slot| core::mem::replace(slot, pte))
    }

    /// Duyệt entry theo thứ tự chỉ số.
    pub fn iter(&self) -> core::slice::Iter<'_, Pte> {
        self.entries.iter()
    }
}

impl<const N: usize> Default for PteArray<N> {
    fn default() -> Self {
        PteArray::new()
    }
}

impl<const N: usize> core::fmt::Debug for PteArray<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PteArray")
            .field("len", &N)
            .field("entries", &self.entries)
            .finish()
    }
}

impl<'a, const N: usize> IntoIterator for &'a PteArray<N> {
    type Item = &'a Pte;
    type IntoIter = core::slice::Iter<'a, Pte>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

// ---------------------------------------------------------------- bit PTE Sv39 (KT-409)
//
// **KT-409 (D2-259 nút thắt 10)** — các hằng **bit PTE Sv39** trước đây nằm trong
// `vifast-plat-common::mmu` (crate HAL). Chúng là **định dạng** (Sv39), không phải chi tiết nền
// tảng: `vifast-pte` đã sở hữu `VA_BITS = 39`, `PTE_LEVELS = 3`, `walk_shape`, `Pte`. Vì vậy
// chuyển lên đây để `vifast-plat-common` **không** còn phải biết Sv39 (điều kiện để dời
// `mmu`/`Sv39` sang `vifast-plat-riscv64`), còn `obj`/`fault` (ở lại `plat-common` tới `KT-417`)
// vẫn dùng được **một** nguồn sự thật.
//
// Bit theo RISC-V privileged spec (Sv39):
// ```text
//  63       61 60 54        10 9 8 7 6 5 4 3 2 1 0
// [ N | PBMT | reserved | PPN | RSW | D A G U X W R V ]
// ```

/// Bit `V` — entry hợp lệ.
pub const PTE_VALID: u64 = 1 << 0;
/// Bit `R` — đọc được.
pub const PTE_READ: u64 = 1 << 1;
/// Bit `W` — ghi được (RISC-V cấm `W=1, R=0`).
pub const PTE_WRITE: u64 = 1 << 2;
/// Bit `X` — thực thi được.
pub const PTE_EXEC: u64 = 1 << 3;
/// Bit `U` — truy cập được từ U-mode.
pub const PTE_USER: u64 = 1 << 4;
/// Bit `G` — ánh xạ toàn cục (không bị `sfence.vma` theo ASID xoá).
pub const PTE_GLOBAL: u64 = 1 << 5;
/// Bit `A` — đã truy cập.
pub const PTE_ACCESSED: u64 = 1 << 6;
/// Bit `D` — đã ghi (bẩn).
pub const PTE_DIRTY: u64 = 1 << 7;

/// Vị trí bit đầu của `PPN` trong entry PTE.
pub const PTE_PPN_SHIFT: u32 = 10;

/// Số entry của **một** bảng trang: `2^9 = 512` (4 KiB / 8 B mỗi entry).
///
/// Hình học của định dạng (cùng nguồn với [`PTE_LEVELS`]/[`PAGE_SIZE_BITS`]), không phải
/// chi tiết nền tảng — `vifast-plat-common::obj` (ở lại tới `KT-417`) dùng hằng này.
pub const PTE_ENTRIES: usize = 1 << 9;

/// `true` ⇔ `va` canonical theo Sv39 (`VA_BITS = 39`): bit `38..63` **cùng** dấu bit 38.
///
/// Cùng ngữ nghĩa `vifast_pte::sv39::is_canonical` cũ (KT-46) — nay là hàm của **định
/// dạng**, dùng chung cho mọi crate cần kiểm dải VA.
pub const fn is_canonical(va: u64) -> bool {
    crate::is_canonical_bits(va, crate::VA_BITS as u32)
}
