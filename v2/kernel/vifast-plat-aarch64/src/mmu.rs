// SPDX-License-Identifier: MIT OR Apache-2.0
//! `Mmu` **thuần dữ liệu** — dựng/đi/xoá bảng trang trên một nguồn frame trừu tượng (KT-160).
//!
//! Tệp này **không** asm, **không** `unsafe`, **không** `cfg(target_arch)`: mọi truy cập bộ nhớ
//! bảng trang đi qua trait [`TableMemory`] (host test cài bằng `Vec`, bước 2 cài bằng physmap).
//! Nhờ vậy toàn bộ logic map/unmap/quyền/đếm frame chạy được trên host — bằng chứng chính của
//! K4' bước 1 (D2-147). `TTBR*`/`MAIR`/`TCR`/`TLBI`/`dsb`/`isb` là việc của bước 2.
//!
//! # Bất biến
//!
//! - **Không rò rỉ frame**: `unmap` giải phóng **mọi** bảng trung gian rỗng trên đường đi (dừng ở
//!   bảng còn entry); `destroy` giải phóng cả cây kể cả gốc. Test đếm `alloc − free`:
//!   sau khi unmap hết chỉ còn **đúng một** frame (gốc); sau `destroy` còn **0**.
//! - **Không cấp phát khi từ chối**: mọi kiểm tra (căn lề, canonical, dải PA, quyền) chạy
//!   **trước** bước walk ⇒ map lỗi không đổi bộ đếm frame.
//! - **Không ghi đè**: map vào VA đã có lá (hoặc nằm trong block) trả `AlreadyMapped`; map đi qua
//!   entry reserved trả `CorruptTable` thay vì ghi đè.
//! - **Frame cấp lại phải được zero**: [`TableMemory::alloc_frame`] trả nội dung **không xác
//!   định**; `Mmu` gọi [`TableMemory::zero_frame`] trước khi dùng. Test `stale_frame_is_zeroed`
//!   bơm rác vào frame đã free rồi map lại để chốt bất biến này.

use core::marker::PhantomData;

use crate::format::{Descriptor, MemAttr, PageTableFormat, Perms, Shareability};

// KT-164: `MapError`/`Mmu`/`Translation` là kiểu **dùng chung** ở `vifast-pte` (thiết kế
// §358/§1101); tệp này chỉ còn phần dữ liệu `PageTable`/`TableMemory`/`Walk` và impl `Mmu`.
pub use vifast_pte::{MapError, Mmu, Translation};

/// Số mức tối đa mà [`Walk`] ghi lại (đủ cho mọi format của bước 1; format nhiều mức hơn phải
/// nâng hằng số này — `Walk` là chẩn đoán, không phải đường map).
pub const MAX_LEVELS: usize = 4;

/// Nguồn frame + bộ nhớ bảng trang — **ranh giới duy nhất** giữa logic và phần cứng.
///
/// Hợp đồng:
/// - [`alloc_frame`](Self::alloc_frame) trả PA 4 KiB aligned, nội dung **không xác định**;
/// - [`free_frame`](Self::free_frame) thu hồi frame (nội dung có thể bị bẩn — `Mmu` không đọc lại
///   frame đã free);
/// - [`read_entry`](Self::read_entry)/[`write_entry`](Self::write_entry) truy cập 512 entry 8 B
///   của một bảng; trả `None`/`false` khi PA không phải bảng hợp lệ (dữ liệu hỏng).
pub trait TableMemory {
    /// Cấp một frame 4 KiB (nội dung không xác định); `None` khi hết.
    fn alloc_frame(&mut self) -> Option<u64>;

    /// Thu hồi frame.
    fn free_frame(&mut self, pa: u64);

    /// Đọc entry `index` của bảng ở `pa`.
    fn read_entry(&self, pa: u64, index: usize) -> Option<u64>;

    /// Ghi entry `index` của bảng ở `pa`; `false` nếu không ghi được.
    fn write_entry(&mut self, pa: u64, index: usize, entry: u64) -> bool;

    /// Zero toàn bộ frame; `false` nếu không zero được.
    fn zero_frame(&mut self, pa: u64) -> bool;
}

/// Một bước của walk chẩn đoán.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WalkStep {
    /// Mức (0 = gốc).
    pub level: usize,
    /// Chỉ số entry trong bảng.
    pub index: usize,
    /// PA của bảng đang đọc.
    pub table: u64,
    /// Giá trị entry đọc được.
    pub entry: u64,
}

/// Kết cục của walk chẩn đoán.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WalkOutcome {
    /// Entry trống.
    Invalid,
    /// Entry reserved/không dùng được.
    Reserved,
    /// Dừng ở bảng (chưa tới lá).
    Table,
    /// Gặp lá 4 KiB.
    Leaf {
        /// PA của lá.
        pa: u64,
        /// Quyền của lá.
        perms: Perms,
    },
    /// Gặp block.
    Block {
        /// PA của block.
        pa: u64,
        /// Mức của block.
        level: usize,
    },
}

/// Walk chẩn đoán: **mọi** bước đã đi (level/index/bảng/entry) + kết cục.
///
/// Dùng để chốt "bảng trang 4 mức đi đúng cấp": test so `steps[i].index` với chỉ số tính tay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Walk {
    /// Các bước đã đi (tối đa [`MAX_LEVELS`]).
    pub steps: [Option<WalkStep>; MAX_LEVELS],
    /// Số bước thực tế.
    pub len: usize,
    /// Kết cục.
    pub outcome: WalkOutcome,
}

impl Walk {
    /// Walk rỗng (VA không canonical hoặc chưa đi bước nào).
    pub const fn empty() -> Self {
        Self {
            steps: [None; MAX_LEVELS],
            len: 0,
            outcome: WalkOutcome::Invalid,
        }
    }

    /// Bước thứ `i` (nếu có).
    pub const fn step(&self, i: usize) -> Option<WalkStep> {
        if i < MAX_LEVELS {
            self.steps[i]
        } else {
            None
        }
    }
}

/// `Mmu` dữ liệu: bảng trang của format `F` trên nguồn frame `M`.
pub struct PageTable<F: PageTableFormat, M: TableMemory> {
    memory: M,
    root: u64,
    _fmt: PhantomData<F>,
}

impl<F: PageTableFormat, M: TableMemory> PageTable<F, M> {
    /// Cấp frame gốc (đã zero) và trả `Mmu` rỗng.
    pub fn new(mut memory: M) -> Result<Self, MapError> {
        let root = memory.alloc_frame().ok_or(MapError::OutOfFrames)?;
        if !memory.zero_frame(root) {
            memory.free_frame(root);
            return Err(MapError::CorruptTable);
        }
        Ok(Self {
            memory,
            root,
            _fmt: PhantomData,
        })
    }

    /// PA bảng gốc.
    pub const fn root(&self) -> u64 {
        self.root
    }

    /// Nguồn frame (đọc).
    pub const fn memory(&self) -> &M {
        &self.memory
    }

    /// Nguồn frame (ghi) — dùng cho test/chẩn đoán.
    pub fn memory_mut(&mut self) -> &mut M {
        &mut self.memory
    }

    /// Map 4 KiB `va → pa` với thuộc tính mặc định (Normal, Inner shareable).
    pub fn map(&mut self, va: u64, pa: u64, perms: Perms) -> Result<(), MapError> {
        self.map_with(va, pa, perms, MemAttr::Normal, Shareability::Inner)
    }

    /// Map 4 KiB `va → pa` với thuộc tính/shareability chỉ định.
    ///
    /// Từ chối: VA/PA lệch 4 KiB (`Unaligned`), VA không canonical (`NotCanonical`), PA vượt
    /// `PA_BITS` (`PaOutOfRange`), quyền không biểu diễn được (`InvalidPerms`), VA đã có mapping
    /// (`AlreadyMapped`). Mọi từ chối xảy ra **trước** khi cấp frame.
    pub fn map_with(
        &mut self,
        va: u64,
        pa: u64,
        perms: Perms,
        attr: MemAttr,
        share: Shareability,
    ) -> Result<(), MapError> {
        if va & (F::PAGE_BYTES - 1) != 0 || pa & (F::PAGE_BYTES - 1) != 0 {
            return Err(MapError::Unaligned);
        }
        if !F::is_canonical(va) {
            return Err(MapError::NotCanonical);
        }
        if pa >> F::PA_BITS != 0 {
            return Err(MapError::PaOutOfRange);
        }
        if !F::perms_valid(perms) {
            return Err(MapError::InvalidPerms);
        }

        let mut table = self.root;
        let mut level = 0;
        while level + 1 < F::LEVELS {
            let index = F::index(va, level);
            let entry = self
                .memory
                .read_entry(table, index)
                .ok_or(MapError::CorruptTable)?;
            match F::decode(entry, level) {
                Descriptor::Invalid => {
                    let new_table = self.memory.alloc_frame().ok_or(MapError::OutOfFrames)?;
                    if !self.memory.zero_frame(new_table) {
                        self.memory.free_frame(new_table);
                        return Err(MapError::CorruptTable);
                    }
                    if !self
                        .memory
                        .write_entry(table, index, F::encode_table(new_table))
                    {
                        self.memory.free_frame(new_table);
                        return Err(MapError::CorruptTable);
                    }
                    table = new_table;
                }
                Descriptor::Table { pa: next } => table = next,
                Descriptor::Leaf { .. } | Descriptor::Block { .. } => {
                    return Err(MapError::AlreadyMapped)
                }
                Descriptor::Reserved => return Err(MapError::CorruptTable),
            }
            level += 1;
        }

        let leaf_index = F::index(va, F::LEVELS - 1);
        let old = self
            .memory
            .read_entry(table, leaf_index)
            .ok_or(MapError::CorruptTable)?;
        match F::decode(old, F::LEVELS - 1) {
            Descriptor::Invalid => {}
            Descriptor::Leaf { .. } | Descriptor::Block { .. } => {
                return Err(MapError::AlreadyMapped)
            }
            Descriptor::Table { .. } | Descriptor::Reserved => return Err(MapError::CorruptTable),
        }
        let leaf = F::encode_leaf(pa, perms, attr, share);
        if !self.memory.write_entry(table, leaf_index, leaf) {
            return Err(MapError::CorruptTable);
        }
        Ok(())
    }

    /// Bỏ mapping 4 KiB tại `va`, trả PA đã map; giải phóng bảng trung gian rỗng.
    ///
    /// Không đụng TLB (bước 2 mới có `TLBI`). VA chưa map ⇒ `NotMapped`; gặp block ⇒
    /// `BlockUnsupported`; bảng hỏng ⇒ `CorruptTable`.
    pub fn unmap(&mut self, va: u64) -> Result<u64, MapError> {
        if va & (F::PAGE_BYTES - 1) != 0 {
            return Err(MapError::Unaligned);
        }
        if !F::is_canonical(va) {
            return Err(MapError::NotCanonical);
        }

        // Đường đi: `path[level] = (bảng tại level, chỉ số dùng để xuống level+1)`.
        let mut path = [(0u64, 0usize); MAX_LEVELS];
        let mut table = self.root;
        let mut level = 0;
        while level + 1 < F::LEVELS {
            let index = F::index(va, level);
            if level >= MAX_LEVELS {
                return Err(MapError::CorruptTable);
            }
            path[level] = (table, index);
            let entry = self
                .memory
                .read_entry(table, index)
                .ok_or(MapError::CorruptTable)?;
            match F::decode(entry, level) {
                Descriptor::Table { pa: next } => table = next,
                Descriptor::Block { .. } => return Err(MapError::BlockUnsupported),
                Descriptor::Invalid => return Err(MapError::NotMapped),
                Descriptor::Leaf { .. } | Descriptor::Reserved => {
                    return Err(MapError::CorruptTable)
                }
            }
            level += 1;
        }

        let leaf_level = F::LEVELS - 1;
        if leaf_level >= MAX_LEVELS {
            return Err(MapError::CorruptTable);
        }
        let leaf_index = F::index(va, leaf_level);
        path[leaf_level] = (table, leaf_index);
        let entry = self
            .memory
            .read_entry(table, leaf_index)
            .ok_or(MapError::CorruptTable)?;
        let pa = match F::decode(entry, leaf_level) {
            Descriptor::Leaf { pa, .. } => pa,
            Descriptor::Block { .. } => return Err(MapError::BlockUnsupported),
            Descriptor::Invalid => return Err(MapError::NotMapped),
            Descriptor::Table { .. } | Descriptor::Reserved => return Err(MapError::CorruptTable),
        };
        if !self.memory.write_entry(table, leaf_index, 0) {
            return Err(MapError::CorruptTable);
        }

        // Giải phóng bảng rỗng từ dưới lên; dừng ngay khi gặp bảng còn entry.
        let mut level = F::LEVELS - 1;
        while level >= 1 {
            let (parent, parent_index) = path[level - 1];
            let child = path[level].0;
            if !self.table_is_empty(child) {
                break;
            }
            if !self.memory.write_entry(parent, parent_index, 0) {
                return Err(MapError::CorruptTable);
            }
            self.memory.free_frame(child);
            level -= 1;
        }
        Ok(pa)
    }

    /// Dịch `va`; `NotMapped` nếu trống, `BlockUnsupported` nếu gặp block.
    pub fn translate(&self, va: u64) -> Result<Translation, MapError> {
        if !F::is_canonical(va) {
            return Err(MapError::NotCanonical);
        }
        let mut table = self.root;
        let mut level = 0;
        while level < F::LEVELS {
            let index = F::index(va, level);
            let entry = self
                .memory
                .read_entry(table, index)
                .ok_or(MapError::CorruptTable)?;
            match F::decode(entry, level) {
                Descriptor::Invalid => return Err(MapError::NotMapped),
                Descriptor::Table { pa } => {
                    if level + 1 == F::LEVELS {
                        return Err(MapError::CorruptTable);
                    }
                    table = pa;
                }
                Descriptor::Leaf { pa, perms } => {
                    return Ok(Translation { pa, perms, level });
                }
                Descriptor::Block { .. } => return Err(MapError::BlockUnsupported),
                Descriptor::Reserved => return Err(MapError::CorruptTable),
            }
            level += 1;
        }
        Err(MapError::NotMapped)
    }

    /// Walk chẩn đoán — ghi **mọi** bước đã đi (kể cả bước dừng).
    pub fn walk(&self, va: u64) -> Walk {
        let mut walk = Walk::empty();
        if !F::is_canonical(va) {
            return walk;
        }
        let mut table = self.root;
        let mut level = 0;
        while level < F::LEVELS {
            if walk.len >= MAX_LEVELS {
                return walk;
            }
            let index = F::index(va, level);
            let Some(entry) = self.memory.read_entry(table, index) else {
                return walk;
            };
            walk.steps[walk.len] = Some(WalkStep {
                level,
                index,
                table,
                entry,
            });
            walk.len += 1;
            match F::decode(entry, level) {
                Descriptor::Invalid => {
                    walk.outcome = WalkOutcome::Invalid;
                    return walk;
                }
                Descriptor::Table { pa } => {
                    walk.outcome = WalkOutcome::Table;
                    table = pa;
                }
                Descriptor::Leaf { pa, perms } => {
                    walk.outcome = WalkOutcome::Leaf { pa, perms };
                    return walk;
                }
                Descriptor::Block { pa, level } => {
                    walk.outcome = WalkOutcome::Block { pa, level };
                    return walk;
                }
                Descriptor::Reserved => {
                    walk.outcome = WalkOutcome::Reserved;
                    return walk;
                }
            }
            level += 1;
        }
        walk
    }

    /// Giải phóng **toàn bộ** cây bảng trang kể cả gốc (dùng khi huỷ VSpace).
    ///
    /// Trả lại nguồn frame để caller kiểm `alloc == free` (test "không rò rỉ frame").
    pub fn destroy(mut self) -> Result<M, MapError> {
        self.free_subtree(self.root, 0)?;
        Ok(self.memory)
    }

    fn table_is_empty(&self, table: u64) -> bool {
        for index in 0..F::ENTRIES {
            match self.memory.read_entry(table, index) {
                Some(0) => {}
                _ => return false,
            }
        }
        true
    }

    fn free_subtree(&mut self, table: u64, level: usize) -> Result<(), MapError> {
        if level + 1 < F::LEVELS {
            for index in 0..F::ENTRIES {
                let entry = self
                    .memory
                    .read_entry(table, index)
                    .ok_or(MapError::CorruptTable)?;
                if let Descriptor::Table { pa } = F::decode(entry, level) {
                    self.free_subtree(pa, level + 1)?;
                }
            }
        }
        self.memory.free_frame(table);
        Ok(())
    }
}

impl<F: PageTableFormat, M: TableMemory> Mmu for PageTable<F, M> {
    type Fmt = F;

    fn root(&self) -> u64 {
        self.root
    }

    fn map(&mut self, va: u64, pa: u64, perms: Perms) -> Result<(), MapError> {
        PageTable::map(self, va, pa, perms)
    }

    fn unmap(&mut self, va: u64) -> Result<u64, MapError> {
        PageTable::unmap(self, va)
    }

    fn translate(&self, va: u64) -> Result<Translation, MapError> {
        PageTable::translate(self, va)
    }
}
