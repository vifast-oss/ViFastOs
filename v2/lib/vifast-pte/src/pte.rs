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
