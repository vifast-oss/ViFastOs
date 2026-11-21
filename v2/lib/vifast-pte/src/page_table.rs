// SPDX-License-Identifier: MIT OR Apache-2.0
//! `Mmu` **thuần dữ liệu** — dựng/đi/xoá bảng trang trên một nguồn frame trừu tượng (KT-160).
//!
//! **KT-186 (D2-163)**: tệp này chuyển từ `vifast-plat-aarch64/src/mmu.rs` về crate dùng chung
//! `vifast-pte` để **ba** định dạng (Sv39, AArch64, x86_64) dùng **một** nguồn frame — đóng nợ
//! "hai bản trùng" mà audit K5' (`v2/agent/reports/K5-X86-READINESS.md` §2.4) đã chỉ ra. Crate
//! arch chỉ còn `impl PageTableFormat` + re-export.
//!
//! Tệp này **không** asm, **không** `unsafe`, **không** `cfg` theo kiến trúc: mọi truy cập bộ nhớ
//! bảng trang đi qua trait [`TableMemory`] (host test cài bằng `Vec`, bước 2 cài bằng physmap).
//! Nhờ vậy toàn bộ logic map/unmap/quyền/đếm frame chạy được trên host — bằng chứng chính của
//! K4' bước 1 (D2-147) và K5' bước M2 (KT-186). `TTBR*`/`MAIR`/`TCR`/`TLBI`/`dsb`/`isb` là việc
//! của bước 2.
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

use crate::mmu::{
    Descriptor, MapError, MemAttr, Mmu, PageTableFormat, Perms, Shareability, Translation,
};

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

    /// Map **block** (superpage) `va → pa` ở `level` (0 = gốc, `LEVELS - 1` = lá).
    ///
    /// Block chỉ hợp lệ ở mức **giữa** (`1..LEVELS-1`): AArch64 4 KiB granule có block 1 GiB ở
    /// mức 1 và 2 MiB ở mức 2; mức 0 và mức lá không có block. Cỡ block suy từ hình học:
    /// `PAGE_BYTES << (INDEX_BITS * (LEVELS - 1 - level))`.
    ///
    /// # Vì sao cần (KT-302)
    ///
    /// ECAM PCIe của QEMU `virt` aarch64 phủ **256 MiB**. Map bằng lá 4 KiB cần **129** bảng
    /// trang (mỗi 2 MiB một bảng mức 3) — vượt arena boot và trần `.data+.bss` (I7). Block 2 MiB
    /// ở mức 2 gói 256 MiB vào **một** bảng mức 2 (128 entry) ⇒ chỉ tốn 2 frame.
    ///
    /// # Ngữ nghĩa
    ///
    /// - `translate` **hiểu** block (KT-305): trả PA đáy block **+ offset trong block** và
    ///   `level` của block; `unmap` **chia nhỏ** block (xem [`Self::unmap`]).
    /// - Map lá 4 KiB vào trong block ⇒ [`MapError::AlreadyMapped`]; map block phủ lên lá/bảng
    ///   đã có ⇒ `AlreadyMapped`.
    /// - Mọi kiểm tra (cấp, căn lề, canonical, dải PA, quyền, format có hỗ trợ block) chạy
    ///   **trước** bước walk ⇒ từ chối không cấp frame.
    pub fn map_block(
        &mut self,
        va: u64,
        pa: u64,
        level: usize,
        perms: Perms,
        attr: MemAttr,
        share: Shareability,
    ) -> Result<(), MapError> {
        if level >= F::LEVELS {
            return Err(MapError::LevelOutOfRange);
        }
        // Năng lực format + cỡ block: mức 0 (gốc) và mức lá **không** có block descriptor, và
        // format không khai `block_span` cũng không có ⇒ `BlockUnsupported` (KT-305 gom hình học
        // về **một** nguồn: `block_span`).
        let span = F::block_span(level).ok_or(MapError::BlockUnsupported)?;
        if va & (span - 1) != 0 || pa & (span - 1) != 0 {
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
        // Năng lực format kiểm **trước** khi cấp frame: format không hỗ trợ block ⇒ từ chối.
        let block =
            F::encode_block(level, pa, perms, attr, share).ok_or(MapError::BlockUnsupported)?;

        let mut table = self.root;
        let mut current = 0;
        while current < level {
            let index = F::index(va, current);
            let entry = self
                .memory
                .read_entry(table, index)
                .ok_or(MapError::CorruptTable)?;
            match F::decode(entry, current) {
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
            current += 1;
        }

        let index = F::index(va, level);
        let old = self
            .memory
            .read_entry(table, index)
            .ok_or(MapError::CorruptTable)?;
        match F::decode(old, level) {
            Descriptor::Invalid => {}
            Descriptor::Leaf { .. } | Descriptor::Block { .. } | Descriptor::Table { .. } => {
                return Err(MapError::AlreadyMapped)
            }
            Descriptor::Reserved => return Err(MapError::CorruptTable),
        }
        if !self.memory.write_entry(table, index, block) {
            return Err(MapError::CorruptTable);
        }
        Ok(())
    }

    /// Bỏ mapping 4 KiB tại `va`, trả PA đã map; giải phóng bảng trung gian rỗng.
    ///
    /// Không đụng TLB (bước 2 mới có `TLBI`). VA chưa map ⇒ `NotMapped`; bảng hỏng ⇒
    /// `CorruptTable`.
    ///
    /// # Block descriptor (KT-305)
    ///
    /// Gặp block phủ `va` mà format khai [`PageTableFormat::block_span`] ⇒ **chia nhỏ** block
    /// xuống mức lá rồi bỏ **một** lá: 2 MiB → 512 lá 4 KiB (một bảng mức 3); 1 GiB → 512 block
    /// 2 MiB → 512 lá (hai bảng). Nhờ vậy **511 trang kế cận giữ nguyên mapping** — xoá cả block
    /// là mất mapping của chúng (đột biến (i) của KT-305).
    ///
    /// Chia nhỏ cần **một frame mỗi mức**; hết frame ⇒ [`MapError::OutOfFrames`] và block **còn
    /// nguyên** (không có bảng nửa vời nào được gắn vào cây). Format **không** khai `block_span`
    /// (x86_64, Sv39) vẫn trả [`MapError::BlockUnsupported`] như trước.
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
                // KT-305: block phủ `va` ⇒ chia **một** mức rồi đi tiếp; vòng lặp chia tiếp nếu
                // mức kế vẫn là block (1 GiB → 2 MiB → lá).
                Descriptor::Block { .. } => {
                    table = self.split_block_once(table, index, level)?;
                }
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

    /// **Chia nhỏ một mức**: thay block ở `(table, index, level)` bằng **một bảng mới** 512 entry
    /// ở mức `level + 1`, trả PA bảng mới.
    ///
    /// Chỉ chia **một** mức — `unmap` gọi lặp khi mức kế vẫn là block (1 GiB → 2 MiB → lá). Mọi
    /// entry con do [`PageTableFormat::split_block`] sinh (format quyết định block hay lá).
    ///
    /// # Fail-closed
    ///
    /// - Format không khai `block_span`/`split_block` ⇒ [`MapError::BlockUnsupported`] **trước**
    ///   khi cấp frame.
    /// - Block lệch căn theo cỡ block ⇒ [`MapError::CorruptTable`] (chia nhỏ block lệch sẽ sinh
    ///   PA sai lặng lẽ).
    /// - Hết frame ⇒ [`MapError::OutOfFrames`]; bảng mới **chưa** được gắn vào cây nên block còn
    ///   nguyên (không có bảng nửa vời).
    fn split_block_once(
        &mut self,
        table: u64,
        index: usize,
        level: usize,
    ) -> Result<u64, MapError> {
        let entry = self
            .memory
            .read_entry(table, index)
            .ok_or(MapError::CorruptTable)?;
        let Descriptor::Block { pa, .. } = F::decode(entry, level) else {
            return Err(MapError::CorruptTable);
        };
        let span = F::block_span(level).ok_or(MapError::BlockUnsupported)?;
        if pa & (span - 1) != 0 {
            return Err(MapError::CorruptTable);
        }
        let new_table = self.memory.alloc_frame().ok_or(MapError::OutOfFrames)?;
        if !self.memory.zero_frame(new_table) {
            self.memory.free_frame(new_table);
            return Err(MapError::CorruptTable);
        }
        for child_index in 0..F::ENTRIES {
            let Some(child) = F::split_block(entry, level, child_index) else {
                // Format khai `block_span` mà không chia được ⇒ không đoán entry con.
                self.memory.free_frame(new_table);
                return Err(MapError::BlockUnsupported);
            };
            if !self.memory.write_entry(new_table, child_index, child) {
                self.memory.free_frame(new_table);
                return Err(MapError::CorruptTable);
            }
        }
        if !self
            .memory
            .write_entry(table, index, F::encode_table(new_table))
        {
            self.memory.free_frame(new_table);
            return Err(MapError::CorruptTable);
        }
        Ok(new_table)
    }

    /// Dịch `va`; `NotMapped` nếu trống, `CorruptTable` nếu bảng hỏng.
    ///
    /// # Block descriptor (KT-305)
    ///
    /// Gặp block ở mức `level` mà format khai [`PageTableFormat::block_span`] ⇒ trả
    /// `Translation { pa: đáy block + (va & (span - 1)), perms, level }` — PA **đúng địa chỉ
    /// được dịch**, không phải đáy block (đột biến (ii) của KT-305). Format **không** khai
    /// `block_span` (x86_64, Sv39) vẫn trả [`MapError::BlockUnsupported`] như trước.
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
                Descriptor::Block { pa, level, perms } => {
                    let span = F::block_span(level).ok_or(MapError::BlockUnsupported)?;
                    // Block do `map_block` tạo luôn căn theo cỡ block; lệch ⇒ dữ liệu hỏng,
                    // không đoán PA (cộng offset vào đáy lệch sẽ ra PA sai lặng lẽ).
                    if pa & (span - 1) != 0 {
                        return Err(MapError::CorruptTable);
                    }
                    return Ok(Translation {
                        pa: pa + (va & (span - 1)),
                        perms,
                        level,
                    });
                }
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
                Descriptor::Block { pa, level, .. } => {
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
