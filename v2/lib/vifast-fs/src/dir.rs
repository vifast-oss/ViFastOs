// SPDX-License-Identifier: MIT OR Apache-2.0
// PORTED-FROM: crates/vifast-fs/src/lib.rs@ca2eaf5
use crate::consts::*;
use crate::fs::*;
use crate::tx::*;

impl<D: BlockDev> Fs<D> {
    pub fn create(&mut self, path: &[u8], uid: u16, kind: u8) -> Result<u16, FsError> {
        if kind != INODE_FILE && kind != INODE_DIR {
            return Err(FsError::Invalid);
        }
        self.tx_count = 0;
        let (parent, name, name_len) = self.resolve_parent(path, uid)?;
        if self.lookup_child(parent, &name[..name_len])?.is_some() {
            return Err(FsError::Exists);
        }
        let ino = self.alloc_inode()?;
        let inode = &mut self.inodes[ino as usize - 1];
        inode.kind = kind;
        inode.uid = uid;
        inode.nlink = 1;
        inode.size = 0;
        self.stage_inode(ino)?;
        self.stage_bitmap()?;
        // 291.4 (r291): nếu bước ghi entry thư mục hỏng (thư mục đầy là ca
        // thật), các thay đổi ĐÃ STAGE (inode + bitmap) phải được ROLLBACK.
        // Bản cũ để nguyên: lần commit sau sẽ ghi inode đã cấp mà không có
        // entry ⇒ rò rỉ inode, `fsck` báo lệch.
        if let Err(error) = self.write_dir_entry(parent, &name[..name_len], ino) {
            self.release_inode(ino);
            self.tx_count = 0;
            return Err(error);
        }
        if kind == INODE_DIR {
            let parent_inode = &mut self.inodes[parent as usize - 1];
            parent_inode.nlink = parent_inode.nlink.saturating_add(1);
            self.stage_inode(parent)?;
        }
        self.commit()?;
        Ok(ino)
    }

    pub fn mkdir(&mut self, path: &[u8], uid: u16) -> Result<u16, FsError> {
        self.create(path, uid, INODE_DIR)
    }

    pub fn symlink(&mut self, target: &[u8], path: &[u8], uid: u16) -> Result<u16, FsError> {
        if target.is_empty() || target.len() > MAX_PATH {
            return Err(FsError::Invalid);
        }
        if path.len() < 2 || path[0] != b'/' || path.len() > MAX_PATH {
            return Err(FsError::Invalid);
        }
        self.tx_count = 0;
        let (parent, name, name_len) = self.resolve_parent(path, uid)?;
        if self.lookup_child(parent, &name[..name_len])?.is_some() {
            return Err(FsError::Exists);
        }
        let ino = self.alloc_inode()?;
        let block_no = self.alloc_block()?;
        let mut block = zero_block();
        block[..target.len()].copy_from_slice(target);
        self.stage(block_no, &block)?;
        {
            let inode = &mut self.inodes[ino as usize - 1];
            inode.kind = INODE_SYMLINK;
            inode.mode = 0b0111;
            inode.uid = uid;
            inode.nlink = 1;
            inode.size = target.len() as u32;
            inode.blocks[0] = block_no;
        }
        self.stage_bitmap()?;
        self.stage_inode(ino)?;
        if let Err(error) = self.write_dir_entry(parent, &name[..name_len], ino) {
            // r299: rollback như create() — dir đầy không được để lọt inode
            // + block đã cấp: transaction kế stage bitmap sẽ persist chỗ
            // leak và fsck báo inode mồ côi vĩnh viễn.
            self.release_inode(ino);
            self.mark_free(block_no);
            self.tx_count = 0;
            return Err(error);
        }
        self.commit()?;
        Ok(ino)
    }

    pub fn readlink(&mut self, path: &[u8], uid: u16) -> Result<([u8; MAX_PATH], usize), FsError> {
        let (parent, name, name_len) = self.resolve_parent(path, uid)?;
        let ino = self
            .lookup_child(parent, &name[..name_len])?
            .ok_or(FsError::NotFound)?;
        self.readlink_inode(ino)
    }

    pub fn lookup(&mut self, path: &[u8], uid: u16) -> Result<u16, FsError> {
        self.lookup_path(path, uid)
    }

    /// Vòng 139: dung lượng dữ liệu còn dùng được — trả (đã dùng, tổng).
    /// Chỉ tính vùng DATA (bỏ superblock/bitmap/inode/journal) đúng như
    /// `alloc_block`, để con số khớp với khả năng cấp phát thật.
    ///
    /// O(1): `used_blocks` được duy trì bởi `mark_used`/`mark_free` và dựng
    /// lại trong `load_metadata`, nên không cần quét bitmap 8162 bit nữa.
    pub fn usage(&self) -> (u32, u32) {
        let total = self.capacity.saturating_sub(DATA_START);
        (core::cmp::min(self.used_blocks, total), total)
    }

    pub fn info(&self, ino: u16) -> Result<InodeInfo, FsError> {
        let inode = *self.inodes.get(ino as usize - 1).ok_or(FsError::BadFd)?;
        if inode.kind == INODE_FREE {
            return Err(FsError::NotFound);
        }
        Ok(InodeInfo {
            kind: inode.kind,
            mode: inode.mode,
            uid: inode.uid,
            nlink: inode.nlink,
            size: inode.size,
        })
    }

    pub fn read(
        &mut self,
        ino: u16,
        offset: usize,
        out: &mut [u8],
        uid: u16,
    ) -> Result<usize, FsError> {
        let inode = *self.inodes.get(ino as usize - 1).ok_or(FsError::BadFd)?;
        if inode.kind == INODE_FREE {
            return Err(FsError::BadFd);
        }
        if inode.kind == INODE_DIR {
            return Err(FsError::IsDir);
        }
        if !self.can_read(&inode, uid) {
            return Err(FsError::NoAccess);
        }
        if offset >= inode.size as usize {
            return Ok(0);
        }
        let count = core::cmp::min(out.len(), inode.size as usize - offset);
        let mut done = 0usize;
        while done < count {
            let block_index = (offset + done) / BLOCK_SIZE;
            let in_block = (offset + done) % BLOCK_SIZE;
            let block_no = self.file_block(&inode, block_index)?;
            let chunk = core::cmp::min(BLOCK_SIZE - in_block, count - done);
            // 291.3 (r291): block 0 trong phạm vi `size` là LỖ của file thưa
            // (ghi quá EOF tạo lỗ) — đọc ra 0, KHÔNG phải Corrupt. Bản cũ trả
            // Corrupt nên ghi ở offset 1 MiB làm cả file không đọc được.
            if block_no == 0 {
                out[done..done + chunk].fill(0);
                done += chunk;
                continue;
            }
            let mut block = zero_block();
            self.dev.read(block_no, &mut block)?;
            out[done..done + chunk].copy_from_slice(&block[in_block..in_block + chunk]);
            done += chunk;
        }
        Ok(count)
    }

    pub fn write(
        &mut self,
        ino: u16,
        offset: usize,
        data: &[u8],
        uid: u16,
    ) -> Result<usize, FsError> {
        let inode = *self.inodes.get(ino as usize - 1).ok_or(FsError::BadFd)?;
        if inode.kind == INODE_FREE {
            return Err(FsError::BadFd);
        }
        if inode.kind == INODE_DIR {
            return Err(FsError::IsDir);
        }
        if !self.can_write(&inode, uid) {
            return Err(FsError::NoAccess);
        }
        let max_file_size = self.max_file_size;
        let end = offset.checked_add(data.len()).ok_or(FsError::Invalid)?;
        if end > max_file_size {
            return Err(FsError::NoSpace);
        }
        let mut done = 0usize;
        // TẠM REVERT batch (#8) để bisect x86 EIO: quay về 1 commit mỗi
        // 512B chunk như trước, giữ nguyên mọi cải tiến khác (CRC bảng,
        // cache 32, counter, cursor, fsync barrier).
        let staged: usize = 0;
        while done < data.len() {
            self.tx_count = 0;
            let mut current = *self.inodes.get(ino as usize - 1).ok_or(FsError::BadFd)?;
            let absolute = offset + done;
            let block_index = absolute / BLOCK_SIZE;
            if block_index >= self.max_file_blocks {
                if staged > 0 {
                    self.commit()?;
                }
                return Err(FsError::NoSpace);
            }
            let in_block = absolute % BLOCK_SIZE;
            let chunk = core::cmp::min(BLOCK_SIZE - in_block, data.len() - done);
            let mut block_no = self.file_block(&current, block_index)?;
            let mut block = if block_no != 0 {
                let mut block = zero_block();
                self.dev.read(block_no, &mut block)?;
                block
            } else {
                block_no = self.alloc_block()?;
                self.set_file_block(&mut current, block_index, block_no)?;
                self.stage_bitmap()?;
                zero_block()
            };
            block[in_block..in_block + chunk].copy_from_slice(&data[done..done + chunk]);
            // TẠM REVERT: stage thẳng + commit ngay (bản cũ).
            self.stage(block_no, &block)?;
            current.size = core::cmp::max(current.size, (absolute + chunk) as u32);
            self.inodes[ino as usize - 1] = current;
            self.stage_inode(ino)?;
            self.commit()?;
            let _ = staged;
            done += chunk;
        }
        Ok(data.len())
    }

    pub(crate) fn free_double_tables(&mut self, inode: &mut Inode) -> Result<(), FsError> {
        if self.version < 4 {
            return Ok(());
        }
        let double_index = self.direct_blocks + 1;
        let double = inode.blocks[double_index];
        if double == 0 {
            return Ok(());
        }
        let table = self.read_staged(double)?;
        for slot in 0..INDIRECT_ENTRIES {
            let second = get_u32(&table, slot * 4);
            if second != 0 {
                self.mark_free(second);
            }
        }
        self.mark_free(double);
        inode.blocks[double_index] = 0;
        Ok(())
    }

    /// Drop indirect-table entries and table blocks that are beyond the new
    /// file length.  Data blocks are freed by `truncate` before this runs.
    pub(crate) fn trim_indirect(
        &mut self,
        inode: &mut Inode,
        new_blocks: usize,
    ) -> Result<(), FsError> {
        let direct = self.direct_blocks;
        // r299: VF02 không có slot gián tiếp (direct = 12 = kích thước mảng
        // blocks) — trim trên ảnh v2 sẽ index quá mảng.
        if direct >= MAX_BLOCKS_PER_INODE {
            return Ok(());
        }
        let single = inode.blocks[direct];
        if self.version < 4 {
            if new_blocks <= direct {
                if single != 0 {
                    self.mark_free(single);
                    inode.blocks[direct] = 0;
                }
            } else if single != 0 {
                let mut table = self.read_staged(single)?;
                for slot in (new_blocks - direct)..INDIRECT_ENTRIES {
                    put_u32(&mut table, slot * 4, 0);
                }
                self.stage(single, &table)?;
            }
            return Ok(());
        }

        if new_blocks <= direct {
            if single != 0 {
                self.mark_free(single);
                inode.blocks[direct] = 0;
            }
            return self.free_double_tables(inode);
        }

        if single != 0 {
            let mut table = self.read_staged(single)?;
            let keep = core::cmp::min(new_blocks - direct, INDIRECT_ENTRIES);
            for slot in keep..INDIRECT_ENTRIES {
                put_u32(&mut table, slot * 4, 0);
            }
            self.stage(single, &table)?;
        }
        if new_blocks <= direct + INDIRECT_ENTRIES {
            return self.free_double_tables(inode);
        }

        let double_index = direct + 1;
        let double = inode.blocks[double_index];
        if double == 0 {
            return Ok(());
        }
        let double_used = new_blocks - direct - INDIRECT_ENTRIES;
        let keep_slots = double_used.div_ceil(INDIRECT_ENTRIES);
        let mut table = self.read_staged(double)?;
        for slot in 0..keep_slots {
            let second = get_u32(&table, slot * 4);
            if second == 0 {
                continue;
            }
            let mut second_table = self.read_staged(second)?;
            let keep_entries = if slot + 1 == keep_slots {
                double_used - slot * INDIRECT_ENTRIES
            } else {
                INDIRECT_ENTRIES
            };
            for entry in keep_entries..INDIRECT_ENTRIES {
                put_u32(&mut second_table, entry * 4, 0);
            }
            self.stage(second, &second_table)?;
        }
        for slot in keep_slots..INDIRECT_ENTRIES {
            let second = get_u32(&table, slot * 4);
            if second != 0 {
                self.mark_free(second);
                put_u32(&mut table, slot * 4, 0);
            }
        }
        self.stage(double, &table)
    }

    pub fn truncate(&mut self, ino: u16, size: usize, uid: u16) -> Result<(), FsError> {
        let inode = *self.inodes.get(ino as usize - 1).ok_or(FsError::BadFd)?;
        // r299: thư mục không được truncate — free_inode_blocks sẽ quét TOÀN
        // BỘ block của thư mục: mọi entry biến mất trong khi inode con vẫn
        // giữ nlink cũ (fsck Corrupt, dữ liệu mất qua namespace). write/
        // read đều đã chặn IsDir; truncate phải chặn như nhau.
        if inode.kind == INODE_DIR {
            return Err(FsError::IsDir);
        }
        if inode.kind == INODE_FILE && !self.can_write(&inode, uid) {
            return Err(FsError::NoAccess);
        }
        if size > self.max_file_size {
            return Err(FsError::NoSpace);
        }
        let mut current = inode;
        let old_size = current.size as usize;
        let old_blocks = current.blocks_used();
        let new_blocks = size.div_ceil(BLOCK_SIZE);
        if new_blocks > self.max_file_blocks {
            return Err(FsError::NoSpace);
        }
        self.tx_count = 0;
        if size < old_size && !size.is_multiple_of(BLOCK_SIZE) {
            // r299: điều kiện cũ `old_size % BLOCK_SIZE != 0` bỏ sót cặp
            // old_size align block (vd 1024 -> 100): đuôi block giữ dữ liệu
            // cũ, grow lại sau đó đọc được byte cũ (POSIX yêu cầu đọc 0).
            // Chỉ cần kiểm `size % BLOCK_SIZE != 0`.
            let block_index = size / BLOCK_SIZE;
            let block_no = self.file_block(&current, block_index)?;
            if block_no != 0 {
                let mut block = zero_block();
                self.dev.read(block_no, &mut block)?;
                for byte in block[size % BLOCK_SIZE..].iter_mut() {
                    *byte = 0;
                }
                self.stage(block_no, &block)?;
            }
        } else if size > old_size && !old_size.is_multiple_of(BLOCK_SIZE) {
            let block_index = old_size / BLOCK_SIZE;
            let block_no = self.file_block(&current, block_index)?;
            if block_no != 0 {
                let mut block = zero_block();
                self.dev.read(block_no, &mut block)?;
                for byte in block[old_size % BLOCK_SIZE..].iter_mut() {
                    *byte = 0;
                }
                self.stage(block_no, &block)?;
            }
        }
        if new_blocks < old_blocks {
            for index in new_blocks..old_blocks {
                let block = self.file_block(&current, index)?;
                if block != 0 {
                    self.mark_free(block);
                }
                if index < self.direct_blocks {
                    current.blocks[index] = 0;
                }
            }
            self.trim_indirect(&mut current, new_blocks)?;
            current.size = size as u32;
            self.inodes[ino as usize - 1] = current;
            self.stage_inode(ino)?;
            self.stage_bitmap()?;
            return self.commit();
        }
        if new_blocks > old_blocks {
            for index in old_blocks..new_blocks {
                self.tx_count = 0;
                let mut current = self.inodes[ino as usize - 1];
                let block = self.alloc_block()?;
                self.set_file_block(&mut current, index, block)?;
                self.stage(block, &zero_block())?;
                self.stage_bitmap()?;
                current.size = core::cmp::max(current.size, ((index + 1) * BLOCK_SIZE) as u32);
                self.inodes[ino as usize - 1] = current;
                self.stage_inode(ino)?;
                self.commit()?;
            }
        }
        let mut current = self.inodes[ino as usize - 1];
        current.size = size as u32;
        self.inodes[ino as usize - 1] = current;
        self.stage_inode(ino)?;
        self.stage_bitmap()?;
        self.commit()
    }

    /// r296: chuyển tiếp số liệu eviction của thiết bị (chỉ để chẩn đoán).
    pub fn evict_stats(&self) -> (u64, u64) {
        self.dev.evict_stats()
    }

    /// r296 (DIRSNAP): chụp lại ĐÚNG LÚC readdir trả "hết" — ba đường đọc
    /// (cache, RAW bỏ cache, ảnh journal đang chờ) trên cùng một block, cộng
    /// `isize` của inode và kết quả `lookup` ngay tại chỗ.
    ///
    /// Vì sao cần: `SLO`/`LOOK` là đèn "lần gần nhất" **không có `seq`**, nên
    /// không được dùng để khẳng định hai lần đọc cùng thời điểm. Đèn này có
    /// `seq` (server in kèm) và đọc cả ba đường trong CÙNG một lời gọi.
    pub fn dir_snapshot(&mut self, dir: u16, index: usize, probe: &[u8]) -> Option<DirSnap> {
        let inode = *self.inodes.get(dir as usize - 1)?;
        if inode.kind != INODE_DIR {
            return None;
        }
        let mut snap = DirSnap {
            blocks: inode.blocks_used() as u32,
            blk0: inode.blocks.first().copied().unwrap_or(0),
            isize: inode.size,
            index: index as u32,
            cached: false,
            dirty: false,
            n_cache: 0,
            n_raw: 0,
            n_jrnl: 0,
            lookup: 0,
            lookup_ino: 0,
            tx: self.tx_count as u32,
            names: [[0u8; MAX_NAME]; DIR_ENTRIES_PER_BLOCK],
            name_len: [0u8; DIR_ENTRIES_PER_BLOCK],
            inos: [0u16; DIR_ENTRIES_PER_BLOCK],
        };
        for block_index in 0..inode.blocks_used() {
            let lba = inode.blocks[block_index];
            if block_index == 0 {
                let (cached, dirty) = self.dev.slot_state(lba);
                snap.cached = cached;
                snap.dirty = dirty;
            }
            let mut block = zero_block();
            if self.dev.read(lba, &mut block).is_ok() {
                snap.n_cache += count_dir_entries(&block);
                if block_index == 0 {
                    for slot in 0..DIR_ENTRIES_PER_BLOCK {
                        let off = slot * DIR_ENTRY_SIZE;
                        let ino = get_u16(&block, off);
                        let len = block[off + 2] as usize;
                        snap.inos[slot] = ino;
                        if ino == 0 || len == 0 || len > MAX_NAME {
                            continue;
                        }
                        snap.name_len[slot] = len as u8;
                        snap.names[slot][..len].copy_from_slice(&block[off + 3..off + 3 + len]);
                    }
                }
            }
            let mut raw = zero_block();
            if self.dev.read_uncached(lba, &mut raw).is_ok() {
                snap.n_raw += count_dir_entries(&raw);
            }
            // Ảnh "nếu áp journal đang chờ": block có đang nằm trong vùng
            // staging (`self.journal` + `self.targets`) hay không.
            let mut journal_hit = false;
            for entry in 0..self.tx_count {
                if self.targets[entry] == lba {
                    let start = entry * BLOCK_SIZE;
                    let mut staged = zero_block();
                    staged.copy_from_slice(&self.journal[start..start + BLOCK_SIZE]);
                    snap.n_jrnl += count_dir_entries(&staged);
                    journal_hit = true;
                }
            }
            if !journal_hit {
                snap.n_jrnl += count_dir_entries(&block);
            }
        }
        // `lookup` cùng tên ngay tại đây, để so trực tiếp với đường liệt kê.
        snap.lookup = match self.lookup_child(dir, probe) {
            Ok(Some(ino)) => {
                snap.lookup_ino = ino;
                1
            }
            Ok(None) => 0,
            Err(_) => 2,
        };
        Some(snap)
    }

    pub fn read_dir_entry(
        &mut self,
        dir: u16,
        index: usize,
    ) -> Result<Option<(u16, [u8; MAX_NAME], usize)>, FsError> {
        Ok(self
            .read_dir_entry_full(dir, index)?
            .map(|(ino, name, len, _)| (ino, name, len)))
    }

    /// Directory entry with the child kind inline.  Powers the
    /// `READDIR_BULK` path (#10): the server packs `(ino, kind, len, name)`
    /// records without an extra `STAT_PATH` IPC per entry, so `ls` no longer
    /// pays N stats for N entries.
    pub fn read_dir_entry_full(
        &mut self,
        dir: u16,
        index: usize,
    ) -> Result<Option<DirEntryFull>, FsError> {
        let inode = *self.inodes.get(dir as usize - 1).ok_or(FsError::BadFd)?;
        if inode.kind != INODE_DIR {
            return Err(FsError::NotDir);
        }
        // r296b (T26, sửa gốc): `index` là **CHỈ SỐ SLOT** — không phải "entry
        // thứ n". Bản cũ đếm entry không rỗng, nên khi một entry PHÍA TRƯỚC bị
        // xoá giữa hai lời gọi thì mọi entry sau tụt xuống một bậc và client
        // (đi tuần tự 0,1,2,…) **bỏ sót** đúng một entry. Đó là cơ chế T26:
        // root tạo rồi xoá `/rm-test` trong `/` đúng lúc client đang liệt kê.
        // Với chỉ số slot, xoá một entry không làm xê dịch entry nào khác.
        //
        // Ô TRỐNG trả về bản ghi `ino == 0` (KHÔNG phải `None`) để client đi
        // tiếp; `None` chỉ khi đã vượt quá slot cuối của thư mục.
        let per_block = DIR_ENTRIES_PER_BLOCK;
        let total_slots = inode.blocks_used() * per_block;
        if index < total_slots {
            let block_index = index / per_block;
            let slot = index % per_block;
            let mut block = zero_block();
            self.dev.read(inode.blocks[block_index], &mut block)?;
            let off = slot * DIR_ENTRY_SIZE;
            let ino = get_u16(&block, off);
            if ino == 0 {
                return Ok(Some((0, [0u8; MAX_NAME], 0, 0)));
            }
            let len = block[off + 2] as usize;
            if len == 0 || len > MAX_NAME || off + 3 + len > BLOCK_SIZE {
                return Err(FsError::Corrupt);
            }
            let mut name = [0u8; MAX_NAME];
            name[..len].copy_from_slice(&block[off + 3..off + 3 + len]);
            let kind = self
                .inodes
                .get(ino as usize - 1)
                .map(|child| child.kind)
                .unwrap_or(INODE_FILE);
            return Ok(Some((ino, name, len, kind)));
        }
        // T26: tới đây là "hết" — đo lại bằng đường RAW (bỏ cache) để biết
        // ảnh nào đúng. Lệch ⇒ bug cache/inode, không phải hết thật.
        let mut cached_seen = 0u32;
        for block_index in 0..inode.blocks_used() {
            let mut cached = zero_block();
            if self
                .dev
                .read(inode.blocks[block_index], &mut cached)
                .is_ok()
            {
                for slot in 0..DIR_ENTRIES_PER_BLOCK {
                    if get_u16(&cached, slot * DIR_ENTRY_SIZE) != 0 {
                        cached_seen += 1;
                    }
                }
            }
        }
        let mut raw_seen = 0u32;
        for block_index in 0..inode.blocks_used() {
            let mut raw = zero_block();
            if self.dev.read(inode.blocks[block_index], &mut raw).is_ok() {
                for slot in 0..DIR_ENTRIES_PER_BLOCK {
                    if get_u16(&raw, slot * DIR_ENTRY_SIZE) != 0 {
                        raw_seen += 1;
                    }
                }
            }
        }
        // Hình dạng thư mục tại đúng thời điểm "hết" (T26): size, blocks_used,
        // và số entry không rỗng của từng block.
        let mut per_block = [0u32; 2];
        let used_blocks = inode.blocks_used();
        for (block_index, count) in per_block.iter_mut().enumerate() {
            if block_index >= used_blocks {
                break;
            }
            let mut raw = zero_block();
            if self.dev.read(inode.blocks[block_index], &mut raw).is_ok() {
                for slot in 0..DIR_ENTRIES_PER_BLOCK {
                    if get_u16(&raw, slot * DIR_ENTRY_SIZE) != 0 {
                        *count += 1;
                    }
                }
            }
        }
        // T26: so ảnh CACHE với ảnh ĐĨA THẬT (bỏ hẳn cache) của block 0.
        // Đèn `DIR` trước đây dùng `dev.read` cho cả hai phía nên không chứng
        // minh được gì (subagent chỉ ra). Nay một phía đi `read_uncached`.
        let mut disk_mismatch = 0u32;
        if inode.blocks_used() > 0 {
            let mut cached = zero_block();
            let mut on_disk = zero_block();
            let cached_ok = self.dev.read(inode.blocks[0], &mut cached).is_ok();
            let disk_ok = self
                .dev
                .read_uncached(inode.blocks[0], &mut on_disk)
                .is_ok();
            if cached_ok && disk_ok && cached != on_disk {
                disk_mismatch = 1;
            }
        }
        DIR_SHAPE[3].store(disk_mismatch, core::sync::atomic::Ordering::Relaxed);
        // Ghi 8 ino đầu của block 0 để so với những gì `lookup_child` đã thấy.
        if inode.blocks_used() > 0 {
            let mut probe = zero_block();
            if self.dev.read(inode.blocks[0], &mut probe).is_ok() {
                for (slot, cell) in SLOT_INOS.iter().enumerate().take(8) {
                    cell.store(
                        get_u16(&probe, slot * DIR_ENTRY_SIZE) as u32,
                        core::sync::atomic::Ordering::Relaxed,
                    );
                }
            }
        }
        DIR_SHAPE[0].store(inode.size, core::sync::atomic::Ordering::Relaxed);
        DIR_SHAPE[1].store(
            inode.blocks_used() as u32,
            core::sync::atomic::Ordering::Relaxed,
        );
        DIR_SHAPE[2].store(per_block[0], core::sync::atomic::Ordering::Relaxed);
        DIR_SHAPE[3].store(per_block[1], core::sync::atomic::Ordering::Relaxed);
        DIR_PROBE[0].store(index as u32, core::sync::atomic::Ordering::Relaxed);
        DIR_PROBE[1].store(cached_seen, core::sync::atomic::Ordering::Relaxed);
        DIR_PROBE[2].store(raw_seen, core::sync::atomic::Ordering::Relaxed);
        Ok(None)
    }

    pub(crate) fn dir_empty(&mut self, dir: u16) -> Result<bool, FsError> {
        let inode = self.inodes[dir as usize - 1];
        for block_index in 0..inode.blocks_used() {
            let mut block = zero_block();
            self.dev.read(inode.blocks[block_index], &mut block)?;
            for slot in 0..DIR_ENTRIES_PER_BLOCK {
                if get_u16(&block, slot * DIR_ENTRY_SIZE) != 0 {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    /// Create a second directory entry for an existing regular file.  The
    /// inode link count is journaled together with the new directory entry.
    pub fn link(&mut self, target: &[u8], new_path: &[u8], uid: u16) -> Result<(), FsError> {
        if target.len() > MAX_PATH || new_path.len() > MAX_PATH {
            return Err(FsError::Invalid);
        }
        self.tx_count = 0;
        let target_ino = self.lookup_path(target, uid)?;
        let target_inode = self.inodes[target_ino as usize - 1];
        if target_inode.kind == INODE_DIR {
            return Err(FsError::IsDir);
        }
        if target_inode.kind == INODE_FREE {
            return Err(FsError::NotFound);
        }
        if target_inode.nlink == u16::MAX {
            return Err(FsError::NoSpace);
        }
        let (parent, name, name_len) = self.resolve_parent(new_path, uid)?;
        if self.lookup_child(parent, &name[..name_len])?.is_some() {
            return Err(FsError::Exists);
        }
        let old_nlink = target_inode.nlink;
        {
            let inode = &mut self.inodes[target_ino as usize - 1];
            inode.nlink = inode.nlink.saturating_add(1);
        }
        self.stage_inode(target_ino)?;
        if let Err(error) = self.write_dir_entry(parent, &name[..name_len], target_ino) {
            let inode = &mut self.inodes[target_ino as usize - 1];
            inode.nlink = old_nlink;
            self.tx_count = 0;
            return Err(error);
        }
        self.commit()
    }

    pub fn unlink(&mut self, path: &[u8], uid: u16) -> Result<(), FsError> {
        self.tx_count = 0;
        let (parent, name, name_len) = self.resolve_parent(path, uid)?;
        let ino = self
            .lookup_child(parent, &name[..name_len])?
            .ok_or(FsError::NotFound)?;
        let inode = self.inodes[ino as usize - 1];
        if inode.kind == INODE_DIR && !self.dir_empty(ino)? {
            return Err(FsError::NotEmpty);
        }
        self.remove_dir_entry(parent, &name[..name_len])?;
        if inode.kind == INODE_DIR {
            let parent_inode = &mut self.inodes[parent as usize - 1];
            parent_inode.nlink = parent_inode.nlink.saturating_sub(1);
            self.stage_inode(parent)?;
        }
        let mut inode = self.inodes[ino as usize - 1];
        inode.nlink = inode.nlink.saturating_sub(1);
        if inode.nlink == 0 {
            self.free_inode_blocks(&mut inode);
            inode = Inode::default();
            self.inodes[ino as usize - 1] = inode;
            self.stage_inode(ino)?;
            self.stage_bitmap()?;
        } else {
            self.inodes[ino as usize - 1] = inode;
            self.stage_inode(ino)?;
        }
        self.commit()
    }

    pub fn rename(&mut self, old: &[u8], new: &[u8], uid: u16) -> Result<(), FsError> {
        if old.len() > MAX_PATH || new.len() > MAX_PATH {
            return Err(FsError::Invalid);
        }
        if new.len() > old.len() && &new[..old.len()] == old && new[old.len()] == b'/' {
            return Err(FsError::Invalid);
        }
        self.tx_count = 0;
        let (old_parent, old_name, old_len) = self.resolve_parent(old, uid)?;
        let (new_parent, new_name, new_len) = self.resolve_parent(new, uid)?;
        let ino = self
            .lookup_child(old_parent, &old_name[..old_len])?
            .ok_or(FsError::NotFound)?;
        if self
            .lookup_child(new_parent, &new_name[..new_len])?
            .is_some()
        {
            return Err(FsError::Exists);
        }
        let inode = self.inodes[ino as usize - 1];
        self.remove_dir_entry(old_parent, &old_name[..old_len])?;
        if let Err(error) = self.write_dir_entry(new_parent, &new_name[..new_len], ino) {
            // r299: rollback như create/link/symlink — bỏ tx_count=0 thì
            // removal đã stage sẽ bị transaction KẾ (hoặc fsync) commit
            // theo, entry cũ biến mất dù rename đã báo lỗi.
            self.tx_count = 0;
            return Err(error);
        }
        if inode.kind == INODE_DIR && old_parent != new_parent {
            let old_parent_inode = &mut self.inodes[old_parent as usize - 1];
            old_parent_inode.nlink = old_parent_inode.nlink.saturating_sub(1);
            self.stage_inode(old_parent)?;
            let new_parent_inode = &mut self.inodes[new_parent as usize - 1];
            new_parent_inode.nlink = new_parent_inode.nlink.saturating_add(1);
            self.stage_inode(new_parent)?;
        }
        self.commit()
    }
}
