// SPDX-License-Identifier: MIT OR Apache-2.0
// PORTED-FROM: crates/vifast-fs/src/lib.rs@ca2eaf5
use crate::consts::*;
use crate::fs::*;

impl<D: BlockDev> Fs<D> {
    pub(crate) fn alloc_inode(&mut self) -> Result<u16, FsError> {
        for index in 1..self.inode_limit {
            if self.inodes[index].kind == INODE_FREE {
                let ino = (index + 1) as u16;
                self.inodes[index] = Inode {
                    kind: INODE_FILE,
                    mode: 0b0111,
                    uid: 0,
                    nlink: 1,
                    size: 0,
                    generation: self.seq ^ index as u32,
                    blocks: [0; MAX_BLOCKS_PER_INODE],
                };
                return Ok(ino);
            }
        }
        Err(FsError::NoInode)
    }

    pub(crate) fn can_read(&self, inode: &Inode, uid: u16) -> bool {
        uid == 0
            || (uid == inode.uid && inode.mode & 1 != 0)
            || (uid != inode.uid && inode.mode & 4 != 0)
    }
    pub(crate) fn can_write(&self, inode: &Inode, uid: u16) -> bool {
        uid == 0
            || (uid == inode.uid && inode.mode & 2 != 0)
            || (uid != inode.uid && inode.mode & 8 != 0)
    }

    pub(crate) fn next_component<'a>(
        path: &'a [u8],
        pos: &mut usize,
    ) -> Result<Option<&'a [u8]>, FsError> {
        while *pos < path.len() && path[*pos] == b'/' {
            *pos += 1;
        }
        if *pos >= path.len() {
            return Ok(None);
        }
        let start = *pos;
        while *pos < path.len() && path[*pos] != b'/' {
            *pos += 1;
        }
        let name = &path[start..*pos];
        if name.len() > MAX_NAME {
            return Err(FsError::NameTooLong);
        }
        if name == b"." || name == b".." {
            return Err(FsError::Invalid);
        }
        // r299: fsck từ chối tên chứa NUL (name.contains(&0) → Corrupt) —
        // writer phải chặn ngay từ next_component, nếu không một lần tạo
        // là đủ báo ảnh dirty vĩnh viễn.
        if name.contains(&0) {
            return Err(FsError::Invalid);
        }
        Ok(Some(name))
    }

    pub(crate) fn lookup_child(&mut self, dir: u16, name: &[u8]) -> Result<Option<u16>, FsError> {
        let inode = self.inodes[dir as usize - 1];
        if inode.kind != INODE_DIR {
            return Err(FsError::NotDir);
        }
        for block_index in 0..inode.blocks_used() {
            let mut block = zero_block();
            self.dev.read(inode.blocks[block_index], &mut block)?;
            for slot in 0..DIR_ENTRIES_PER_BLOCK {
                let off = slot * DIR_ENTRY_SIZE;
                let ino = get_u16(&block, off);
                if ino == 0 {
                    continue;
                }
                let len = block[off + 2] as usize;
                // Vòng 83 (audit #2 P0-1): độ dài tên đến từ đĩa phải được
                // kiểm tra trước khi cắt slice, nếu không một byte hỏng là đủ
                // để panic trong VFS server.
                if len == 0 || len > MAX_NAME || off + 3 + len > BLOCK_SIZE {
                    continue;
                }
                if len == name.len() && &block[off + 3..off + 3 + len] == name {
                    LOOK_HIT[0].store(inode.size, core::sync::atomic::Ordering::Relaxed);
                    LOOK_HIT[1].store(
                        inode.blocks_used() as u32,
                        core::sync::atomic::Ordering::Relaxed,
                    );
                    LOOK_HIT[2].store(inode.blocks[0], core::sync::atomic::Ordering::Relaxed);
                    LOOK_HIT[3].store(ino as u32, core::sync::atomic::Ordering::Relaxed);
                    return Ok(Some(ino));
                }
            }
        }
        Ok(None)
    }

    pub(crate) fn readlink_inode(&mut self, ino: u16) -> Result<([u8; MAX_PATH], usize), FsError> {
        let inode = *self.inodes.get(ino as usize - 1).ok_or(FsError::BadFd)?;
        if inode.kind != INODE_SYMLINK {
            return Err(FsError::Invalid);
        }
        let len = core::cmp::min(inode.size as usize, MAX_PATH);
        let mut target = [0u8; MAX_PATH];
        if len != 0 {
            self.read(ino, 0, &mut target[..len], 0)?;
        }
        Ok((target, len))
    }

    pub(crate) fn lookup_path_follow(
        &mut self,
        path: &[u8],
        uid: u16,
        follow_final: bool,
    ) -> Result<u16, FsError> {
        let _ = uid;
        if path.is_empty() || path[0] != b'/' || path.len() > MAX_PATH {
            return Err(FsError::Invalid);
        }
        let mut buf = [0u8; MAX_PATH];
        buf[..path.len()].copy_from_slice(path);
        let mut path_len = path.len();
        let mut pos = 0usize;
        let mut ino = ROOT_INO;
        let mut symlinks = 0usize;
        loop {
            let current = self.inodes[ino as usize - 1];
            if current.kind != INODE_DIR {
                return Err(FsError::NotDir);
            }
            while pos < path_len && buf[pos] == b'/' {
                pos += 1;
            }
            if pos >= path_len {
                return Ok(ino);
            }
            let name_start = pos;
            while pos < path_len && buf[pos] != b'/' {
                pos += 1;
            }
            let name = &buf[name_start..pos];
            if name.len() > MAX_NAME || name == b"." || name == b".." {
                return Err(if name.len() > MAX_NAME {
                    FsError::NameTooLong
                } else {
                    FsError::Invalid
                });
            }
            let parent = ino;
            let child = self.lookup_child(parent, name)?.ok_or(FsError::NotFound)?;
            let child_inode = self.inodes[child as usize - 1];
            let mut probe = pos;
            while probe < path_len && buf[probe] == b'/' {
                probe += 1;
            }
            let has_more = probe < path_len;
            if child_inode.kind == INODE_SYMLINK && (follow_final || has_more) {
                symlinks += 1;
                if symlinks > MAX_SYMLINK_DEPTH {
                    return Err(FsError::Invalid);
                }
                let (target, target_len) = self.readlink_inode(child)?;
                if target_len == 0 || target[0] != b'/' && name_start == 0 {
                    return Err(FsError::Invalid);
                }
                let suffix = &buf[pos..path_len];
                let mut next = [0u8; MAX_PATH];
                let prefix_len = if target[0] == b'/' { 0 } else { name_start };
                if prefix_len != 0 {
                    next[..prefix_len].copy_from_slice(&buf[..prefix_len]);
                }
                if prefix_len + target_len + suffix.len() > MAX_PATH {
                    return Err(FsError::Invalid);
                }
                next[prefix_len..prefix_len + target_len].copy_from_slice(&target[..target_len]);
                next[prefix_len + target_len..prefix_len + target_len + suffix.len()]
                    .copy_from_slice(suffix);
                path_len = prefix_len + target_len + suffix.len();
                buf = next;
                pos = 0;
                ino = if target[0] == b'/' { ROOT_INO } else { parent };
                continue;
            }
            ino = child;
            if !has_more {
                return Ok(ino);
            }
        }
    }

    pub(crate) fn lookup_path(&mut self, path: &[u8], uid: u16) -> Result<u16, FsError> {
        self.lookup_path_follow(path, uid, true)
    }

    pub(crate) fn resolve_parent(
        &mut self,
        path: &[u8],
        uid: u16,
    ) -> Result<(u16, [u8; MAX_NAME], usize), FsError> {
        if path.len() < 2 || path[0] != b'/' || path.len() > MAX_PATH {
            return Err(FsError::Invalid);
        }
        let mut pos = 0usize;
        let mut last_start = 0usize;
        let mut last_len = 0usize;
        while let Some(name) = Self::next_component(path, &mut pos)? {
            last_start = pos.saturating_sub(name.len());
            last_len = name.len();
        }
        if last_len == 0 {
            return Err(FsError::Invalid);
        }
        let parent = if last_start == 0 {
            ROOT_INO
        } else {
            self.lookup_path(&path[..last_start], uid)?
        };
        let parent_inode = self.inodes[parent as usize - 1];
        if parent_inode.kind != INODE_DIR {
            return Err(FsError::NotDir);
        }
        if !self.can_write(&parent_inode, uid) {
            return Err(FsError::NoAccess);
        }
        let mut last = [0u8; MAX_NAME];
        last[..last_len].copy_from_slice(&path[last_start..last_start + last_len]);
        Ok((parent, last, last_len))
    }

    pub(crate) fn write_dir_entry(
        &mut self,
        dir: u16,
        name: &[u8],
        child: u16,
    ) -> Result<(), FsError> {
        let mut inode = self.inodes[dir as usize - 1];
        for block_index in 0..inode.blocks_used() {
            let mut block = self.read_staged(inode.blocks[block_index])?;
            for slot in 0..DIR_ENTRIES_PER_BLOCK {
                let off = slot * DIR_ENTRY_SIZE;
                if get_u16(&block, off) == 0 {
                    put_u16(&mut block, off, child);
                    block[off + 2] = name.len() as u8;
                    block[off + 3..off + 3 + name.len()].copy_from_slice(name);
                    self.stage(inode.blocks[block_index], &block)?;
                    return Ok(());
                }
            }
        }
        if inode.blocks_used() >= self.dir_block_limit {
            return Err(FsError::NoSpace);
        }
        let block_no = self.alloc_block()?;
        let mut block = zero_block();
        put_u16(&mut block, 0, child);
        block[2] = name.len() as u8;
        block[3..3 + name.len()].copy_from_slice(name);
        inode.blocks[inode.blocks_used()] = block_no;
        inode.size = inode.size.saturating_add(BLOCK_SIZE as u32);
        self.inodes[dir as usize - 1] = inode;
        self.stage(block_no, &block)?;
        self.stage_inode(dir)?;
        self.stage_bitmap()?;
        Ok(())
    }

    pub(crate) fn remove_dir_entry(&mut self, dir: u16, name: &[u8]) -> Result<u16, FsError> {
        let inode = self.inodes[dir as usize - 1];
        for block_index in 0..inode.blocks_used() {
            let mut block = self.read_staged(inode.blocks[block_index])?;
            for slot in 0..DIR_ENTRIES_PER_BLOCK {
                let off = slot * DIR_ENTRY_SIZE;
                let ino = get_u16(&block, off);
                if ino == 0 {
                    continue;
                }
                let len = block[off + 2] as usize;
                // Vòng 83 (audit #2 P0-1): độ dài tên đến từ đĩa phải được
                // kiểm tra trước khi cắt slice, nếu không một byte hỏng là đủ
                // để panic trong VFS server.
                if len == 0 || len > MAX_NAME || off + 3 + len > BLOCK_SIZE {
                    continue;
                }
                if len == name.len() && &block[off + 3..off + 3 + len] == name {
                    put_u16(&mut block, off, 0);
                    block[off + 2] = 0;
                    for byte in block[off + 3..off + 3 + MAX_NAME].iter_mut() {
                        *byte = 0;
                    }
                    self.stage(inode.blocks[block_index], &block)?;
                    return Ok(ino);
                }
            }
        }
        Err(FsError::NotFound)
    }
}
