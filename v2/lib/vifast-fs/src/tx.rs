// SPDX-License-Identifier: MIT OR Apache-2.0
// PORTED-FROM: crates/vifast-fs/src/lib.rs@ca2eaf5
use crate::consts::*;
use crate::fs::*;

impl<D: BlockDev> Fs<D> {
    pub(crate) fn read_staged(&mut self, lba: u32) -> Result<[u8; BLOCK_SIZE], FsError> {
        for index in 0..self.tx_count {
            if self.targets[index] == lba {
                let start = index * BLOCK_SIZE;
                let mut block = zero_block();
                block.copy_from_slice(&self.journal[start..start + BLOCK_SIZE]);
                return Ok(block);
            }
        }
        let mut block = zero_block();
        self.dev.read(lba, &mut block)?;
        Ok(block)
    }

    pub(crate) fn stage(&mut self, lba: u32, data: &[u8; BLOCK_SIZE]) -> Result<(), FsError> {
        if lba == 0 || lba >= self.dev.block_count() {
            return Err(FsError::Invalid);
        }
        for index in 0..self.tx_count {
            if self.targets[index] == lba {
                let start = index * BLOCK_SIZE;
                self.journal[start..start + BLOCK_SIZE].copy_from_slice(data);
                return Ok(());
            }
        }
        if self.tx_count >= MAX_TX_WRITES {
            return Err(FsError::TooManyWrites);
        }
        let index = self.tx_count;
        self.targets[index] = lba;
        let start = index * BLOCK_SIZE;
        self.journal[start..start + BLOCK_SIZE].copy_from_slice(data);
        self.tx_count += 1;
        Ok(())
    }

    pub(crate) fn commit(&mut self) -> Result<(), FsError> {
        if self.tx_count == 0 {
            return Ok(());
        }
        let count = self.tx_count;
        for index in 0..count {
            let start = index * BLOCK_SIZE;
            let mut data = zero_block();
            data.copy_from_slice(&self.journal[start..start + BLOCK_SIZE]);
            self.dev.write(JOURNAL_DATA_START + index as u32, &data)?;
        }
        let mut header = zero_block();
        header[..4].copy_from_slice(b"VJ01");
        put_u32(&mut header, 4, self.seq);
        header[8] = count as u8;
        for index in 0..count {
            put_u32(&mut header, 16 + index * 4, self.targets[index]);
        }
        let mut crc = crc32(&header[..80]);
        for index in 0..count {
            let start = index * BLOCK_SIZE;
            crc = crc32_continue(crc, &self.journal[start..start + BLOCK_SIZE]);
        }
        put_u32(&mut header, 80, crc);
        self.dev.write(JOURNAL_START, &header)?;
        let mut commit = zero_block();
        commit[..4].copy_from_slice(b"VC01");
        put_u32(&mut commit, 4, self.seq);
        put_u32(&mut commit, 8, crc);
        self.dev.write(JOURNAL_COMMIT_LBA, &commit)?;
        for index in 0..count {
            let start = index * BLOCK_SIZE;
            let mut data = zero_block();
            data.copy_from_slice(&self.journal[start..start + BLOCK_SIZE]);
            self.dev.write(self.targets[index], &data)?;
        }
        // Never clear the journal commit until dirty data blocks and their
        // write-back cache have reached the device.
        self.dev.flush()?;
        // T26 (đèn quyết định cuối): sau khi flush, đọc lại TỪNG target bằng
        // đường BỎ CACHE và so với ảnh journal. Lệch ⇒ lệnh ghi không tới đĩa
        // dù commit đã "thành công". Đây là phép đo phân biệt hai nhánh còn lại.
        for index in 0..count {
            let mut on_disk = zero_block();
            if self
                .dev
                .read_uncached(self.targets[index], &mut on_disk)
                .is_ok()
            {
                let start = index * BLOCK_SIZE;
                if on_disk[..] != self.journal[start..start + BLOCK_SIZE] {
                    COMMIT_LOST.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
                }
                COMMIT_VERIFIED.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
            }
        }
        self.dev.write(JOURNAL_COMMIT_LBA, &zero_block())?;
        self.seq = self.seq.wrapping_add(1);
        self.tx_count = 0;
        Ok(())
    }

    pub(crate) fn stage_inode(&mut self, ino: u16) -> Result<(), FsError> {
        let index = ino as usize - 1;
        if index >= self.inode_limit {
            return Err(FsError::Invalid);
        }
        let table_block = index / 8;
        let mut block = zero_block();
        for slot in 0..8 {
            let entry = table_block * 8 + slot;
            inode_encode(
                &self.inodes[entry],
                &mut block[slot * INODE_SIZE..(slot + 1) * INODE_SIZE],
            );
        }
        self.stage(INODE_START + table_block as u32, &block)
    }

    pub(crate) fn stage_bitmap(&mut self) -> Result<(), FsError> {
        for index in 0..BITMAP_BLOCKS as usize {
            let mut block = zero_block();
            let start = index * BLOCK_SIZE;
            block.copy_from_slice(&self.bitmap[start..start + BLOCK_SIZE]);
            self.stage(BITMAP_START + index as u32, &block)?;
        }
        Ok(())
    }
}

impl<D: BlockDev> Fs<D> {
    /// Flush all staged-but-uncommitted transaction data, then the device.
    /// (#11) This is the real durability barrier: `write()` batches staged
    /// blocks and may return with a partial transaction still open, so
    /// `fsync` must commit before flushing.  Callers that need POSIX-like
    /// "fsync makes it durable" semantics should use this.
    pub fn fsync(&mut self) -> Result<(), FsError> {
        if self.tx_count > 0 {
            self.commit()?;
        }
        self.dev.flush()
    }

    /// Queue one complete write transaction without applying it.  This is a
    /// test/host-tool hook used to create a valid or torn journal image.
    pub fn inject_pending_write(
        &mut self,
        path: &[u8],
        data: &[u8],
        uid: u16,
        torn: bool,
    ) -> Result<(), FsError> {
        let ino = match self.lookup(path, uid) {
            Ok(ino) => ino,
            Err(FsError::NotFound) => self.create(path, uid, INODE_FILE)?,
            Err(error) => return Err(error),
        };
        // Build the transaction but write only journal/commit blocks, leaving
        // target blocks untouched so mount must replay.
        self.tx_count = 0;
        let inode = *self.inodes.get(ino as usize - 1).ok_or(FsError::BadFd)?;
        let mut current = inode;
        let end = data.len().min(MAX_FILE_SIZE);
        let first = 0usize;
        let last = end.saturating_sub(1) / BLOCK_SIZE;
        for block_index in first..=last {
            // r299: đi qua file_block/set_file_block như writer thật — index
            // thẳng current.blocks[] chỉ đúng cho 10 block đầu; quá 12 là
            // index ngoài mảng [u32; 12] và bỏ luôn sơ đồ gián tiếp.
            let existing = self.file_block(&current, block_index)?;
            let block_no = if existing != 0 {
                existing
            } else {
                let block_no = self.alloc_block()?;
                self.stage(block_no, &zero_block())?;
                self.stage_bitmap()?;
                self.set_file_block(&mut current, block_index, block_no)?;
                block_no
            };
            let mut block = zero_block();
            self.dev.read(block_no, &mut block)?;
            let start = block_index * BLOCK_SIZE;
            let chunk = core::cmp::min(BLOCK_SIZE, end - start);
            block[..chunk].copy_from_slice(&data[start..start + chunk]);
            self.stage(block_no, &block)?;
        }
        current.size = end as u32;
        self.inodes[ino as usize - 1] = current;
        self.stage_inode(ino)?;
        self.stage_bitmap()?;
        self.write_journal_only(torn)
    }

    pub(crate) fn write_journal_only(&mut self, torn: bool) -> Result<(), FsError> {
        let count = self.tx_count;
        for index in 0..count {
            let start = index * BLOCK_SIZE;
            let mut data = zero_block();
            data.copy_from_slice(&self.journal[start..start + BLOCK_SIZE]);
            self.dev.write(JOURNAL_DATA_START + index as u32, &data)?;
        }
        let mut header = zero_block();
        header[..4].copy_from_slice(b"VJ01");
        put_u32(&mut header, 4, self.seq);
        header[8] = count as u8;
        for index in 0..count {
            put_u32(&mut header, 16 + index * 4, self.targets[index]);
        }
        let mut crc = crc32(&header[..80]);
        for index in 0..count {
            let start = index * BLOCK_SIZE;
            crc = crc32_continue(crc, &self.journal[start..start + BLOCK_SIZE]);
        }
        put_u32(&mut header, 80, crc);
        self.dev.write(JOURNAL_START, &header)?;
        let mut commit = zero_block();
        commit[..4].copy_from_slice(b"VC01");
        put_u32(&mut commit, 4, self.seq);
        put_u32(&mut commit, 8, if torn { crc ^ 0xdead_beef } else { crc });
        self.dev.write(JOURNAL_COMMIT_LBA, &commit)?;
        self.tx_count = 0;
        self.seq = self.seq.wrapping_add(1);
        Ok(())
    }
}
/// Reference bitwise CRC-32 kept only as a test oracle: it recomputes the
/// digest one bit at a time so the table-driven `crc32()`/`crc32_continue()`
/// above can be cross-checked against an independent implementation.
#[cfg(test)]
pub(crate) fn crc32_bitwise(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in data {
        crc ^= *byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

/// Reference bitwise CRC-32 continuation, mirroring `crc32_continue()`
/// without the lookup table.
#[cfg(test)]
pub(crate) fn crc32_continue_bitwise(mut crc: u32, data: &[u8]) -> u32 {
    crc = !crc;
    for byte in data {
        crc ^= *byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

impl Inode {
    pub fn blocks_used(&self) -> usize {
        if self.kind == INODE_FREE {
            return 0;
        }
        // Vòng 83 (audit #2 P0-2): size trên đĩa có thể bị hỏng (ví dụ dir
        // size > 6144). Nếu không kẹp theo số slot thật của `blocks`, các
        // vòng lặp bên dưới sẽ index `[u32; MAX_BLOCKS_PER_INODE]` quá biên
        // và panic trong VFS server (mọi IPC sau đó treo).
        let blocks = (self.size as usize).div_ceil(BLOCK_SIZE);
        if self.kind == INODE_DIR {
            // Chỉ thư mục mới index trực tiếp `blocks[]`; file lớn dùng
            // `file_block()` nên có thể vượt 12 slot một cách hợp lệ.
            return core::cmp::min(blocks, MAX_BLOCKS_PER_INODE);
        }
        blocks
    }
}

/// Bản ghi thư mục đầy đủ (ino, tên, độ dài tên, loại inode) — dùng cho
/// `READDIR_BULK` để client khỏi tốn thêm một `STAT_PATH` mỗi entry.
pub type DirEntryFull = (u16, [u8; MAX_NAME], usize, u8);
