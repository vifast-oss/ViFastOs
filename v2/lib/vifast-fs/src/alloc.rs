// SPDX-License-Identifier: MIT OR Apache-2.0
// PORTED-FROM: crates/vifast-fs/src/lib.rs@ca2eaf5
use crate::consts::*;
use crate::fs::*;

impl<D: BlockDev> Fs<D> {
    pub(crate) fn block_is_used(&self, block: u32) -> bool {
        // P1-1: chặn theo CẢ capacity của FS và độ dài bitmap (phòng thủ).
        block < self.capacity
            && (block as usize) / 8 < self.bitmap.len()
            && self.bitmap[block as usize / 8] & (1 << (block as usize % 8)) != 0
    }

    pub(crate) fn mark_used(&mut self, block: u32) {
        if block >= self.capacity || block as usize / 8 >= self.bitmap.len() {
            return;
        }
        let byte = block as usize / 8;
        let bit = 1u8 << (block as usize % 8);
        if self.bitmap[byte] & bit == 0 {
            self.bitmap[byte] |= bit;
            // Only the DATA region feeds the usage counter.
            if block >= DATA_START {
                self.used_blocks = self.used_blocks.saturating_add(1);
            }
        }
    }

    pub(crate) fn mark_free(&mut self, block: u32) {
        // Vòng 83: block number đọc từ đĩa (free_inode_blocks) có thể rác.
        // Không kiểm tra biên thì bitmap[block/8] ghi ra ngoài mảng.
        if block >= self.capacity || block as usize / 8 >= self.bitmap.len() {
            return;
        }
        let byte = block as usize / 8;
        let bit = 1u8 << (block as usize % 8);
        if self.bitmap[byte] & bit != 0 {
            self.bitmap[byte] &= !bit;
            if block >= DATA_START {
                self.used_blocks = self.used_blocks.saturating_sub(1);
                // Pull the cursor back so the freed block is reused soon.
                if block < self.next_free {
                    self.next_free = block;
                }
            }
        }
    }

    pub(crate) fn alloc_block(&mut self) -> Result<u32, FsError> {
        // P1-1: cấp phát trong biên FS, không trong biên thiết bị.
        let capacity = self.capacity;
        // Scan from the cursor, wrapping once, instead of always restarting
        // at DATA_START.  The common case (cursor at a free block) is O(1).
        let start = self.next_free.clamp(DATA_START, capacity);
        for pass in 0..2 {
            let from = if pass == 0 { start } else { DATA_START };
            let to = if pass == 0 { capacity } else { start };
            let mut block = from;
            while block < to {
                if !self.block_is_used(block) {
                    self.mark_used(block);
                    // Advance past the block just handed out.
                    self.next_free = if block + 1 < capacity {
                        block + 1
                    } else {
                        DATA_START
                    };
                    return Ok(block);
                }
                block += 1;
            }
        }
        // Keep the cursor in range even when the disk is full.
        self.next_free = DATA_START;
        Err(FsError::NoSpace)
    }

    pub(crate) fn file_block(&mut self, inode: &Inode, block_index: usize) -> Result<u32, FsError> {
        if inode.kind == INODE_DIR {
            return Ok(inode.blocks.get(block_index).copied().unwrap_or(0));
        }
        if block_index < self.direct_blocks {
            return Ok(inode.blocks[block_index]);
        }
        // P1-2 (r290): chỉ số block đến từ `inode.size` trên đĩa, tức dữ liệu
        // không tin được. Vượt sức chứa tối đa của FS ⇒ `Corrupt`, KHÔNG được
        // để `get_u32` đọc ngoài mảng (bản cũ panic và giết kernel).
        if block_index >= self.max_file_blocks {
            return Err(FsError::Corrupt);
        }
        let relative = block_index - self.direct_blocks;
        if self.version < 4 && relative >= INDIRECT_ENTRIES {
            // Không có double-indirect ở định dạng cũ ⇒ chỉ số này bịa.
            return Err(FsError::Corrupt);
        }
        if self.version >= 4 && relative >= INDIRECT_ENTRIES {
            // r299: phải nhìn bảng DOUBLE TRƯỚC khi đoán theo single — bản
            // cũ `single == 0 → return 0` đứng trước nhánh này nên file thưa
            // có dữ liệu đầu tiên ở vùng double bị coi là lỗ hoàn toàn:
            // read mất dữ liệu, rewrite cấp block mới và bỏ lọt block cũ
            // (bitmap không khớp, fsck Corrupt).
            let double_index = self.direct_blocks + 1;
            let double = inode.blocks[double_index];
            if double == 0 {
                return Ok(0);
            }
            let double_relative = relative - INDIRECT_ENTRIES;
            let slot = double_relative / INDIRECT_ENTRIES;
            // P1-2: bảng double-indirect chỉ có `INDIRECT_ENTRIES` ô.
            if slot >= INDIRECT_ENTRIES {
                return Err(FsError::Corrupt);
            }
            let double_table = self.read_staged(double)?;
            let table = get_u32(&double_table, slot * 4);
            if table == 0 {
                return Ok(0);
            }
            let table = self.read_staged(table)?;
            return Ok(get_u32(&table, (double_relative % INDIRECT_ENTRIES) * 4));
        }
        let indirect = inode.blocks[self.direct_blocks];
        if indirect == 0 {
            return Ok(0);
        }
        let block = self.read_staged(indirect)?;
        Ok(get_u32(&block, relative * 4))
    }

    pub(crate) fn set_file_block(
        &mut self,
        inode: &mut Inode,
        block_index: usize,
        block_no: u32,
    ) -> Result<(), FsError> {
        if inode.kind == INODE_DIR || block_index < self.direct_blocks {
            inode.blocks[block_index] = block_no;
            return Ok(());
        }
        let relative = block_index - self.direct_blocks;
        if self.version >= 4 && relative >= INDIRECT_ENTRIES {
            let double_index = self.direct_blocks + 1;
            let double_relative = relative - INDIRECT_ENTRIES;
            let slot = double_relative / INDIRECT_ENTRIES;
            let double = inode.blocks[double_index];
            let double = if double == 0 {
                if block_no == 0 {
                    return Ok(());
                }
                let allocated = self.alloc_block()?;
                self.stage(allocated, &zero_block())?;
                self.stage_bitmap()?;
                inode.blocks[double_index] = allocated;
                allocated
            } else {
                double
            };
            let mut double_table = self.read_staged(double)?;
            let table = get_u32(&double_table, slot * 4);
            let table = if table == 0 {
                if block_no == 0 {
                    return Ok(());
                }
                let allocated = self.alloc_block()?;
                self.stage(allocated, &zero_block())?;
                self.stage_bitmap()?;
                put_u32(&mut double_table, slot * 4, allocated);
                self.stage(double, &double_table)?;
                allocated
            } else {
                table
            };
            let mut block = self.read_staged(table)?;
            put_u32(
                &mut block,
                (double_relative % INDIRECT_ENTRIES) * 4,
                block_no,
            );
            return self.stage(table, &block);
        }
        let indirect = inode.blocks[self.direct_blocks];
        let indirect = if indirect == 0 {
            if block_no == 0 {
                return Ok(());
            }
            let allocated = self.alloc_block()?;
            self.stage(allocated, &zero_block())?;
            self.stage_bitmap()?;
            inode.blocks[self.direct_blocks] = allocated;
            allocated
        } else {
            indirect
        };
        let mut table = self.read_staged(indirect)?;
        put_u32(&mut table, relative * 4, block_no);
        self.stage(indirect, &table)
    }

    pub(crate) fn free_inode_blocks(&mut self, inode: &mut Inode) {
        let used = inode.blocks_used();
        for index in 0..used {
            if let Ok(block) = self.file_block(inode, index) {
                if block != 0 {
                    self.mark_free(block);
                }
            }
        }
        // r299: VF02 (direct = 12 = MAX_BLOCKS_PER_INODE) không có slot gián
        // tiếp — blocks[direct] quá mảng, panic khi unlink file trên ảnh v2.
        if inode.kind == INODE_FILE && self.direct_blocks < MAX_BLOCKS_PER_INODE {
            let indirect = inode.blocks[self.direct_blocks];
            if indirect != 0 {
                self.mark_free(indirect);
                inode.blocks[self.direct_blocks] = 0;
            }
            if self.version >= 4 {
                let double_index = self.direct_blocks + 1;
                let double = inode.blocks[double_index];
                if double != 0 {
                    if let Ok(table) = self.read_staged(double) {
                        for slot in 0..INDIRECT_ENTRIES {
                            let second = get_u32(&table, slot * 4);
                            if second != 0 {
                                self.mark_free(second);
                            }
                        }
                    }
                    self.mark_free(double);
                    inode.blocks[double_index] = 0;
                }
            }
        }
    }

    /// 291.4 (r291): trả một inode vừa cấp về trạng thái tự do (rollback).
    pub(crate) fn release_inode(&mut self, ino: u16) {
        if let Some(inode) = self.inodes.get_mut(ino as usize - 1) {
            inode.kind = INODE_FREE;
            inode.nlink = 0;
            inode.size = 0;
            inode.blocks = [0; MAX_BLOCKS_PER_INODE];
        }
    }
}
