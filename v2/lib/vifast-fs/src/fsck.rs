// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-71** — `fsck` **sửa** (không chỉ phát hiện): block mồ côi, block đếm sai, inode không
//! tham chiếu, link count sai, entry trỏ inode chết.
//!
//! `FsckReport` đếm **phát hiện/sửa theo từng loại** — mọi số trong bằng chứng đọc từ đây, không
//! phải hằng kỳ vọng (D2-102). Sửa đi qua **journal** (`stage` + `commit`), nên ảnh sửa bền sau
//! khi mở lại; mỗi lần sửa commit khi transaction gần đầy (không vượt `MAX_TX_WRITES`).

use crate::consts::*;
use crate::fs::*;

/// Kết quả một lần `fsck_repair`.
///
/// `found = fixed + unfixed` (bất biến kiểm được): lỗi không sửa an toàn được (ví dụ **một
/// block bị hai inode cùng tham chiếu**) được đếm `unfixed` thay vì sửa mù.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FsckReport {
    /// Tổng lỗi phát hiện.
    pub found: u32,
    /// Tổng lỗi đã sửa.
    pub fixed: u32,
    /// Lỗi phát hiện nhưng **không** sửa (không an toàn để đoán).
    pub unfixed: u32,
    /// Block đã cấp trong bitmap nhưng không inode nào tham chiếu.
    pub orphan_blocks: u32,
    /// Inode đang dùng nhưng không nằm trong cây thư mục.
    pub orphan_inodes: u32,
    /// `nlink` trong inode khác số liên kết thật.
    pub bad_nlink: u32,
    /// Con trỏ/bảng block sai (ngoài `size`, bảng thừa, block ngoài dải).
    pub bad_blocks: u32,
    /// Entry thư mục trỏ inode chết / tên sai.
    pub dangling_entries: u32,
    /// Một block bị nhiều nơi tham chiếu (không sửa mù).
    pub double_blocks: u32,
    /// Size thư mục sai (không bội 512 / quá `dir_block_limit`).
    pub dir_size: u32,
}

impl FsckReport {
    fn note(&mut self, category: fn(&mut Self) -> &mut u32) {
        self.found += 1;
        self.fixed += 1;
        *category(self) += 1;
    }
}

/// Duyệt các block của một inode thường (`kind != INODE_DIR`) và in ra lỗi cần sửa.
///
/// Hàm **không** sửa dữ liệu file; chỉ sửa **bảng** (con trỏ/bảng gián tiếp) và **trả block**
/// thừa về bitmap — dữ liệu đã đọc được giữ nguyên.
impl<D: BlockDev> Fs<D> {
    pub(crate) fn fsck_fix_tables(
        &mut self,
        ino: u16,
        report: &mut FsckReport,
        bitmap_dirty: &mut bool,
    ) -> Result<(), FsError> {
        let mut inode = self.inodes[ino as usize - 1];
        let direct = self.direct_blocks;
        let max_blocks = self.max_file_blocks;
        let mut used = inode.blocks_used();
        let mut changed = false;
        if used > max_blocks {
            inode.size = (max_blocks * BLOCK_SIZE) as u32;
            used = max_blocks;
            report.note(|r| &mut r.bad_blocks);
            changed = true;
        }
        let end = direct.min(MAX_BLOCKS_PER_INODE);
        for block in inode.blocks.iter_mut().take(end).skip(used) {
            if *block != 0 {
                self.mark_free(*block);
                *block = 0;
                *bitmap_dirty = true;
                report.note(|r| &mut r.bad_blocks);
                changed = true;
            }
        }
        if direct >= MAX_BLOCKS_PER_INODE {
            if changed {
                self.inodes[ino as usize - 1] = inode;
                self.stage_inode(ino)?;
            }
            return Ok(());
        }
        // Single-indirect: bảng chỉ cần khi `used > direct`.
        let single = inode.blocks[direct];
        if single != 0 && used <= direct {
            let table = self.read_staged(single)?;
            for entry in 0..INDIRECT_ENTRIES {
                let block = get_u32(&table, entry * 4);
                if block != 0 {
                    self.mark_free(block);
                    *bitmap_dirty = true;
                    report.note(|r| &mut r.bad_blocks);
                }
            }
            self.mark_free(single);
            inode.blocks[direct] = 0;
            *bitmap_dirty = true;
            report.note(|r| &mut r.bad_blocks);
            changed = true;
        } else if single != 0 {
            let keep = used - direct;
            let mut table = self.read_staged(single)?;
            let mut table_changed = false;
            for entry in keep..INDIRECT_ENTRIES {
                let block = get_u32(&table, entry * 4);
                if block != 0 {
                    self.mark_free(block);
                    put_u32(&mut table, entry * 4, 0);
                    *bitmap_dirty = true;
                    report.note(|r| &mut r.bad_blocks);
                    table_changed = true;
                }
            }
            if table_changed {
                self.stage(single, &table)?;
            }
        }
        if self.version >= 4 {
            let double = inode.blocks[direct + 1];
            if double != 0 {
                let needed = used.saturating_sub(direct + INDIRECT_ENTRIES);
                let mut root = self.read_staged(double)?;
                let mut root_changed = false;
                for slot in 0..INDIRECT_ENTRIES {
                    let l2 = get_u32(&root, slot * 4);
                    if l2 == 0 {
                        continue;
                    }
                    let slot_keep = if needed == 0 {
                        0
                    } else if slot < needed / INDIRECT_ENTRIES {
                        INDIRECT_ENTRIES
                    } else if slot == needed / INDIRECT_ENTRIES {
                        needed % INDIRECT_ENTRIES
                    } else {
                        0
                    };
                    if slot_keep == 0 {
                        let table = self.read_staged(l2)?;
                        for entry in 0..INDIRECT_ENTRIES {
                            let block = get_u32(&table, entry * 4);
                            if block != 0 {
                                self.mark_free(block);
                                *bitmap_dirty = true;
                                report.note(|r| &mut r.bad_blocks);
                            }
                        }
                        self.mark_free(l2);
                        put_u32(&mut root, slot * 4, 0);
                        *bitmap_dirty = true;
                        root_changed = true;
                        report.note(|r| &mut r.bad_blocks);
                    } else {
                        let mut table = self.read_staged(l2)?;
                        let mut table_changed = false;
                        for entry in slot_keep..INDIRECT_ENTRIES {
                            let block = get_u32(&table, entry * 4);
                            if block != 0 {
                                self.mark_free(block);
                                put_u32(&mut table, entry * 4, 0);
                                *bitmap_dirty = true;
                                report.note(|r| &mut r.bad_blocks);
                                table_changed = true;
                            }
                        }
                        if table_changed {
                            self.stage(l2, &table)?;
                        }
                    }
                }
                if needed == 0 && inode.blocks[direct + 1] != 0 {
                    self.mark_free(double);
                    inode.blocks[direct + 1] = 0;
                    *bitmap_dirty = true;
                    report.note(|r| &mut r.bad_blocks);
                    root_changed = false;
                }
                if root_changed {
                    self.stage(double, &root)?;
                }
                changed |= root_changed || needed == 0;
            }
        }
        if changed {
            self.inodes[ino as usize - 1] = inode;
            self.stage_inode(ino)?;
        }
        Ok(())
    }

    /// Trả **mọi** block mà một inode (mồ côi) có thể đang giữ: 12 con trỏ trực tiếp, bảng
    /// single-indirect + entry của nó, bảng double-indirect + các bảng L2 + entry của chúng.
    pub(crate) fn fsck_free_inode(&mut self, inode: &Inode, bitmap_dirty: &mut bool) {
        let direct = self.direct_blocks;
        if inode.kind == INODE_DIR {
            for &block in inode.blocks.iter().take(MAX_DIR_BLOCKS) {
                if block != 0 {
                    self.mark_free(block);
                    *bitmap_dirty = true;
                }
            }
            return;
        }
        for &block in inode.blocks.iter().take(direct.min(MAX_BLOCKS_PER_INODE)) {
            if block != 0 {
                self.mark_free(block);
                *bitmap_dirty = true;
            }
        }
        if direct >= MAX_BLOCKS_PER_INODE {
            return;
        }
        let single = inode.blocks[direct];
        if single != 0 {
            if let Ok(table) = self.read_staged(single) {
                for entry in 0..INDIRECT_ENTRIES {
                    let block = get_u32(&table, entry * 4);
                    if block != 0 {
                        self.mark_free(block);
                        *bitmap_dirty = true;
                    }
                }
            }
            self.mark_free(single);
            *bitmap_dirty = true;
        }
        if self.version >= 4 {
            let double = inode.blocks[direct + 1];
            if double != 0 {
                if let Ok(root) = self.read_staged(double) {
                    for slot in 0..INDIRECT_ENTRIES {
                        let l2 = get_u32(&root, slot * 4);
                        if l2 == 0 {
                            continue;
                        }
                        if let Ok(table) = self.read_staged(l2) {
                            for entry in 0..INDIRECT_ENTRIES {
                                let block = get_u32(&table, entry * 4);
                                if block != 0 {
                                    self.mark_free(block);
                                    *bitmap_dirty = true;
                                }
                            }
                        }
                        self.mark_free(l2);
                        *bitmap_dirty = true;
                    }
                }
                self.mark_free(double);
                *bitmap_dirty = true;
            }
        }
    }

    /// Đánh dấu block đã tham chiếu; block ngoài dải hoặc **đã bị tham chiếu** ⇒ lỗi.
    fn fsck_claim(
        &self,
        block: u32,
        block_used: &mut [u8; (BITMAP_BLOCKS as usize) * BLOCK_SIZE],
        report: &mut FsckReport,
    ) {
        let capacity = self.capacity;
        if block < DATA_START || block >= capacity {
            report.note(|r| &mut r.bad_blocks);
            return;
        }
        let byte = block as usize / 8;
        let bit = 1u8 << (block as usize % 8);
        if block_used[byte] & bit != 0 {
            // Hai inode cùng tham chiếu một block: KHÔNG sửa mù (không biết ai đúng).
            report.found += 1;
            report.unfixed += 1;
            report.double_blocks += 1;
            return;
        }
        block_used[byte] |= bit;
    }

    /// **Quét và sửa** FS; trả số lỗi phát hiện/đã sửa theo từng loại.
    ///
    /// Chạy được trên ảnh **hỏng do crash** (sau `replay`) lẫn ảnh bị sửa tay có chủ đích; sau
    /// khi sửa, `fsck()` (kiểm) phải sạch và lần `fsck_repair` kế tiếp trả `found = 0`.
    #[inline(never)]
    pub fn fsck_repair(&mut self) -> Result<FsckReport, FsError> {
        let mut report = FsckReport::default();
        let mut bitmap_dirty = false;
        let inode_limit = self.inode_limit;
        let dir_limit_bytes = self.dir_block_limit as u32 * BLOCK_SIZE as u32;
        let mut reachable = [false; MAX_INODES];
        let mut nlink = [0u16; MAX_INODES];
        let mut block_used = [0u8; (BITMAP_BLOCKS as usize) * BLOCK_SIZE];
        let mut stack = [0u16; MAX_INODES];
        let mut stack_len = 1usize;
        let mut reach_dirs = [false; MAX_INODES];
        stack[0] = ROOT_INO;
        reachable[ROOT_INO as usize - 1] = true;
        reach_dirs[ROOT_INO as usize - 1] = true;
        nlink[ROOT_INO as usize - 1] = 1;
        while stack_len != 0 {
            stack_len -= 1;
            let ino = stack[stack_len];
            let mut inode = self.inodes[ino as usize - 1];
            if inode.kind != INODE_DIR {
                continue;
            }
            let mut used = inode.blocks_used();
            if !inode.size.is_multiple_of(BLOCK_SIZE as u32) || inode.size > dir_limit_bytes {
                report.note(|r| &mut r.dir_size);
                let blocks = core::cmp::min(
                    (inode.size as usize).div_ceil(BLOCK_SIZE),
                    self.dir_block_limit,
                );
                inode.size = core::cmp::max(blocks, 1) as u32 * BLOCK_SIZE as u32;
                used = inode.blocks_used();
                self.inodes[ino as usize - 1] = inode;
                self.stage_inode(ino)?;
            }
            for block_index in 0..used.min(MAX_DIR_BLOCKS) {
                let block_no = inode.blocks[block_index];
                if block_no < DATA_START || block_no >= self.capacity {
                    report.note(|r| &mut r.bad_blocks);
                    inode.blocks[block_index] = 0;
                    self.inodes[ino as usize - 1] = inode;
                    self.stage_inode(ino)?;
                    continue;
                }
                self.fsck_claim(block_no, &mut block_used, &mut report);
                let mut data = self.read_staged(block_no)?;
                let mut block_changed = false;
                for slot in 0..DIR_ENTRIES_PER_BLOCK {
                    let off = slot * DIR_ENTRY_SIZE;
                    let child = get_u16(&data, off);
                    if child == 0 {
                        continue;
                    }
                    let name_len = data[off + 2] as usize;
                    let mut drop = name_len == 0
                        || name_len > MAX_NAME
                        || child as usize > inode_limit
                        || child == 0;
                    if !drop {
                        let name = &data[off + 3..off + 3 + name_len];
                        drop = name == b"."
                            || name == b".."
                            || name.contains(&b'/')
                            || name.contains(&0)
                            || self.inodes[child as usize - 1].kind == INODE_FREE;
                    }
                    if drop {
                        for byte in data[off..off + DIR_ENTRY_SIZE].iter_mut() {
                            *byte = 0;
                        }
                        block_changed = true;
                        report.note(|r| &mut r.dangling_entries);
                        continue;
                    }
                    let child_dir = self.inodes[child as usize - 1].kind == INODE_DIR;
                    if !reachable[child as usize - 1] {
                        reachable[child as usize - 1] = true;
                        if child_dir {
                            if stack_len >= MAX_INODES {
                                report.found += 1;
                                report.unfixed += 1;
                                continue;
                            }
                            stack[stack_len] = child;
                            stack_len += 1;
                            reach_dirs[child as usize - 1] = true;
                        }
                    }
                    nlink[child as usize - 1] = nlink[child as usize - 1].saturating_add(1);
                    if child_dir {
                        nlink[ino as usize - 1] = nlink[ino as usize - 1].saturating_add(1);
                    }
                }
                if block_changed {
                    self.stage(block_no, &data)?;
                    if self.tx_count + 4 >= MAX_TX_WRITES {
                        self.commit()?;
                    }
                }
            }
        }
        if self.tx_count != 0 {
            self.commit()?;
        }
        // Inode không tham chiếu + bảng block sai + nlink.
        for index in 0..inode_limit {
            let inode = self.inodes[index];
            if inode.kind == INODE_FREE {
                continue;
            }
            if !reachable[index] {
                self.fsck_free_inode(&inode, &mut bitmap_dirty);
                self.inodes[index] = Inode::default();
                bitmap_dirty = true;
                self.stage_inode((index + 1) as u16)?;
                report.note(|r| &mut r.orphan_inodes);
                if self.tx_count + 4 >= MAX_TX_WRITES {
                    self.stage_bitmap()?;
                    self.commit()?;
                    bitmap_dirty = false;
                }
                continue;
            }
            if inode.kind != INODE_FILE && inode.kind != INODE_DIR && inode.kind != INODE_SYMLINK {
                self.fsck_free_inode(&inode, &mut bitmap_dirty);
                self.inodes[index] = Inode::default();
                bitmap_dirty = true;
                self.stage_inode((index + 1) as u16)?;
                report.note(|r| &mut r.orphan_inodes);
                continue;
            }
            let mut current = self.inodes[index];
            let mut inode_changed = false;
            if reach_dirs[index] && current.kind != INODE_DIR {
                // Chỉ xảy ra khi ảnh bị sửa tay: kiểu inode vs kiểu tham chiếu không khớp.
                report.found += 1;
                report.unfixed += 1;
                continue;
            }
            if current.nlink != nlink[index] {
                current.nlink = nlink[index];
                inode_changed = true;
                report.note(|r| &mut r.bad_nlink);
            }
            if inode_changed {
                self.inodes[index] = current;
                self.stage_inode((index + 1) as u16)?;
            }
            if current.kind != INODE_DIR && current.kind != INODE_FREE {
                self.fsck_fix_tables((index + 1) as u16, &mut report, &mut bitmap_dirty)?;
                if self.tx_count + 4 >= MAX_TX_WRITES {
                    self.stage_bitmap()?;
                    self.commit()?;
                    bitmap_dirty = false;
                }
            }
        }
        if self.tx_count != 0 {
            if bitmap_dirty {
                self.stage_bitmap()?;
                bitmap_dirty = false;
            }
            self.commit()?;
        }
        // Đánh dấu block tham chiếu của inode **đã sửa bảng** (đọc lại từ in-memory), rồi
        // trả mọi block đã cấp mà không ai tham chiếu.
        let mut block_used = [0u8; (BITMAP_BLOCKS as usize) * BLOCK_SIZE];
        let mut report_claim = FsckReport::default();
        for (index, reach) in reachable.iter().take(inode_limit).enumerate() {
            let inode = self.inodes[index];
            if inode.kind == INODE_FREE || !*reach {
                continue;
            }
            if inode.kind == INODE_DIR {
                for &block in inode
                    .blocks
                    .iter()
                    .take(inode.blocks_used().min(MAX_DIR_BLOCKS))
                {
                    if block != 0 {
                        self.fsck_claim(block, &mut block_used, &mut report_claim);
                    }
                }
                continue;
            }
            let used = inode.blocks_used();
            for block_index in 0..used {
                let block = match self.file_block(&inode, block_index) {
                    Ok(block) => block,
                    Err(_) => {
                        report.note(|r| &mut r.bad_blocks);
                        continue;
                    }
                };
                if block == 0 {
                    continue;
                }
                self.fsck_claim(block, &mut block_used, &mut report_claim);
            }
            report.found += report_claim.found;
            report.fixed += report_claim.fixed;
            report.unfixed += report_claim.unfixed;
            report.bad_blocks += report_claim.bad_blocks;
            report.double_blocks += report_claim.double_blocks;
            report_claim = FsckReport::default();
        }
        for block in DATA_START..self.capacity {
            if self.block_is_used(block) {
                let byte = block as usize / 8;
                let bit = 1u8 << (block as usize % 8);
                if block_used[byte] & bit == 0 {
                    self.mark_free(block);
                    bitmap_dirty = true;
                    report.note(|r| &mut r.orphan_blocks);
                }
            }
        }
        if bitmap_dirty {
            self.stage_bitmap()?;
        }
        if self.tx_count != 0 {
            self.commit()?;
        }
        Ok(report)
    }
}
