// SPDX-License-Identifier: MIT OR Apache-2.0
// PORTED-FROM: crates/vifast-fs/src/lib.rs@ca2eaf5
use crate::consts::*;

/// Format-time geometry.  `total_blocks == 0` uses the device capacity.
#[derive(Clone, Copy, Debug)]
pub struct FormatOptions {
    pub total_blocks: u32,
    pub inode_limit: u16,
    pub dir_block_limit: u8,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            total_blocks: 0,
            inode_limit: MAX_INODES as u16,
            dir_block_limit: MAX_DIR_BLOCKS as u8,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SuperInfo {
    version: u8,
    inode_limit: usize,
    dir_block_limit: usize,
    direct_blocks: usize,
    max_file_blocks: usize,
    max_file_size: usize,
    /// P1-1 (r290): số block của FS theo SUPERBLOCK, không theo thiết bị.
    /// Đĩa lớn hơn vùng đã format là hợp lệ; dùng `dev.block_count()` ở đây
    /// từng làm `load_metadata` quét quá biên bitmap (panic out of bounds).
    total: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FsError {
    Io,
    Invalid,
    NoSpace,
    NoInode,
    NotFound,
    Exists,
    NotDir,
    IsDir,
    NotEmpty,
    NoAccess,
    NameTooLong,
    BadFd,
    TooManyWrites,
    Corrupt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Inode {
    pub kind: u8,
    pub mode: u8,
    pub uid: u16,
    pub nlink: u16,
    pub size: u32,
    pub generation: u32,
    pub blocks: [u32; MAX_BLOCKS_PER_INODE],
}

#[derive(Clone, Copy, Debug)]
pub struct InodeInfo {
    pub kind: u8,
    pub mode: u8,
    pub uid: u16,
    pub nlink: u16,
    pub size: u32,
}

pub trait BlockDev {
    fn read(&mut self, lba: u32, out: &mut [u8; BLOCK_SIZE]) -> Result<(), FsError>;
    fn write(&mut self, lba: u32, data: &[u8; BLOCK_SIZE]) -> Result<(), FsError>;
    fn block_count(&self) -> u32 {
        TOTAL_BLOCKS
    }
    /// Flush the device's volatile write cache.  Backends without a cache can
    /// keep the default no-op; real block drivers should override it.
    fn flush(&mut self) -> Result<(), FsError> {
        Ok(())
    }

    /// Đọc **bỏ qua cache** (T26: đèn phân biệt "cache khác đĩa" với "đĩa thật
    /// sự cũ"). Thiết bị không có cache giữ mặc định = `read`.
    fn read_uncached(&mut self, lba: u32, out: &mut [u8; BLOCK_SIZE]) -> Result<(), FsError> {
        self.read(lba, out)
    }

    /// r296 (DIRSNAP): `(có trong cache, đang bẩn)`. Mặc định "không có cache";
    /// `CachedDev` ghi đè. Chỉ dùng cho chẩn đoán.
    fn slot_state(&self, lba: u32) -> (bool, bool) {
        let _ = lba;
        (false, false)
    }

    /// r296: `(evict_lost, evict_dirty)`. Mặc định 0 cho thiết bị không cache.
    fn evict_stats(&self) -> (u64, u64) {
        (0, 0)
    }
}

/// Number of 512-byte blocks kept in the write-through cache.
///
/// Eight slots cost about 4 KiB and fit comfortably in the VFS server's
/// static storage.  Keeping the cache small also makes eviction predictable:
/// the wrapper uses first-free, then round-robin replacement.
///
/// NOTE (#9a REVERTED): raising this to 32 broke x86 QEMU deterministically
/// (`PROG SIZE BAD` / `FS V2 BAD` via ATA `EIO`): with 32 slots, `flush()`
/// pushes many dirty blocks back-to-back and QEMU's ATA emulation drops the
/// burst (`ata_wait_bsy` timeout).  Virtio ports (riscv64/aarch64) tolerated
/// it, but correctness on all three ports comes first.  Revisit with a
/// larger cache only alongside an ATA driver that paces burst writes.
pub const CACHE_SLOTS: usize = 8;

/// T26: ba số đo của lần `read_dir_entry_full` trả `Ok(None)` gần nhất:
/// `(index, số entry đếm được qua cache, số entry đếm được qua đường RAW)`.
/// Lệch nhau ⇒ ảnh cache khác ảnh đĩa (đèn quyết định), không phải "hết thật".
pub(crate) static DIR_PROBE: [core::sync::atomic::AtomicU32; 3] = [
    core::sync::atomic::AtomicU32::new(0),
    core::sync::atomic::AtomicU32::new(0),
    core::sync::atomic::AtomicU32::new(0),
];

/// T26: hình dạng thư mục tại lần `Ok(None)` gần nhất:
/// `(size, blocks_used, số entry khối 0, số entry khối 1)`.
pub(crate) static DIR_SHAPE: [core::sync::atomic::AtomicU32; 4] = [
    core::sync::atomic::AtomicU32::new(0),
    core::sync::atomic::AtomicU32::new(0),
    core::sync::atomic::AtomicU32::new(0),
    core::sync::atomic::AtomicU32::new(0),
];

/// Số lần `commit()` ghi xong mà đọc lại (bỏ cache) thấy KHÁC ảnh journal.
pub(crate) static COMMIT_LOST: core::sync::atomic::AtomicU32 =
    core::sync::atomic::AtomicU32::new(0);
/// Tổng số target đã kiểm (mẫu số của đèn trên).
pub(crate) static COMMIT_VERIFIED: core::sync::atomic::AtomicU32 =
    core::sync::atomic::AtomicU32::new(0);

/// T26: hình dạng inode của lần `lookup_child` TÌM THẤY gần nhất:
/// `(size, blocks_used, blocks[0], ino)`. So với lần `read_dir_entry_full` trả
/// `Ok(None)` để biết hai hàm có thấy cùng một phiên bản inode không.
pub(crate) static LOOK_HIT: [core::sync::atomic::AtomicU32; 4] = [
    core::sync::atomic::AtomicU32::new(0),
    core::sync::atomic::AtomicU32::new(0),
    core::sync::atomic::AtomicU32::new(0),
    core::sync::atomic::AtomicU32::new(0),
];

/// T26: `ino` của 8 slot đầu trong block 0 thư mục, tại lần `Ok(None)` gần nhất.
pub(crate) static SLOT_INOS: [core::sync::atomic::AtomicU32; 8] = [
    core::sync::atomic::AtomicU32::new(0),
    core::sync::atomic::AtomicU32::new(0),
    core::sync::atomic::AtomicU32::new(0),
    core::sync::atomic::AtomicU32::new(0),
    core::sync::atomic::AtomicU32::new(0),
    core::sync::atomic::AtomicU32::new(0),
    core::sync::atomic::AtomicU32::new(0),
    core::sync::atomic::AtomicU32::new(0),
];

pub fn slot_inos() -> [u32; 8] {
    [
        SLOT_INOS[0].load(core::sync::atomic::Ordering::Relaxed),
        SLOT_INOS[1].load(core::sync::atomic::Ordering::Relaxed),
        SLOT_INOS[2].load(core::sync::atomic::Ordering::Relaxed),
        SLOT_INOS[3].load(core::sync::atomic::Ordering::Relaxed),
        SLOT_INOS[4].load(core::sync::atomic::Ordering::Relaxed),
        SLOT_INOS[5].load(core::sync::atomic::Ordering::Relaxed),
        SLOT_INOS[6].load(core::sync::atomic::Ordering::Relaxed),
        SLOT_INOS[7].load(core::sync::atomic::Ordering::Relaxed),
    ]
}

pub fn look_hit() -> (u32, u32, u32, u32) {
    (
        LOOK_HIT[0].load(core::sync::atomic::Ordering::Relaxed),
        LOOK_HIT[1].load(core::sync::atomic::Ordering::Relaxed),
        LOOK_HIT[2].load(core::sync::atomic::Ordering::Relaxed),
        LOOK_HIT[3].load(core::sync::atomic::Ordering::Relaxed),
    )
}

pub fn commit_lost() -> (u32, u32) {
    (
        COMMIT_LOST.load(core::sync::atomic::Ordering::Relaxed),
        COMMIT_VERIFIED.load(core::sync::atomic::Ordering::Relaxed),
    )
}

pub fn dir_shape() -> (u32, u32, u32, u32) {
    (
        DIR_SHAPE[0].load(core::sync::atomic::Ordering::Relaxed),
        DIR_SHAPE[1].load(core::sync::atomic::Ordering::Relaxed),
        DIR_SHAPE[2].load(core::sync::atomic::Ordering::Relaxed),
        DIR_SHAPE[3].load(core::sync::atomic::Ordering::Relaxed),
    )
}

pub fn dir_probe() -> (u32, u32, u32) {
    (
        DIR_PROBE[0].load(core::sync::atomic::Ordering::Relaxed),
        DIR_PROBE[1].load(core::sync::atomic::Ordering::Relaxed),
        DIR_PROBE[2].load(core::sync::atomic::Ordering::Relaxed),
    )
}

#[derive(Clone, Copy)]
struct CacheSlot {
    lba: u32,
    data: [u8; BLOCK_SIZE],
    dirty: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub writes: u64,
}

/// A small block cache with write-back for file data and write-through for
/// metadata/journal blocks.
///
/// Journal data, journal headers and commit records live below `DATA_START`
/// and are written through immediately, preserving write-ahead ordering.
/// File data blocks are cached dirty; `commit()` flushes them before clearing
/// the journal commit record, and `Fs::fsync()` flushes them explicitly.
pub struct CachedDev<D> {
    inner: D,
    slots: [Option<CacheSlot>; CACHE_SLOTS],
    next: usize,
    hits: u64,
    misses: u64,
    writes: u64,
    /// Số lần "ghi xong mà đọc lại thấy khác" — bằng chứng trực tiếp của việc
    /// mất lệnh ghi (T26).
    lost_writes: u64,
    /// r296: số lần ĐẨY MỘT BLOCK BẨN RA KHỎI CACHE (eviction) xong đọc lại thì
    /// thấy ảnh CŨ. Đây là đường ghi duy nhất trước đây KHÔNG được kiểm: `flush()`
    /// có kiểm, còn `insert()` thì không. Giả thuyết T26 mới nhất: eviction ghi
    /// xuống thiết bị nhưng lần đọc sau (cache miss) vẫn thấy ảnh trước đó.
    evict_lost: u64,
    /// r296: tổng số lần evict một block bẩn (mẫu số của `evict_lost`).
    evict_dirty: u64,
}

impl<D: BlockDev> CachedDev<D> {
    pub const fn new(inner: D) -> Self {
        Self {
            inner,
            slots: [None; CACHE_SLOTS],
            next: 0,
            hits: 0,
            misses: 0,
            writes: 0,
            lost_writes: 0,
            evict_lost: 0,
            evict_dirty: 0,
        }
    }

    pub fn stats(&self) -> CacheStats {
        CacheStats {
            hits: self.hits,
            misses: self.misses,
            writes: self.writes,
        }
    }

    /// Số lần ghi-xong-mà-đọc-lại-khác (T26). Chỉ dùng cho chẩn đoán.
    pub fn lost_writes(&self) -> u64 {
        self.lost_writes
    }

    /// r296: `(evict_lost, evict_dirty)` — số lần đẩy block bẩn ra khỏi cache
    /// mà đọc lại thấy ảnh cũ / tổng số lần đẩy block bẩn.
    pub fn evict_stats(&self) -> (u64, u64) {
        (self.evict_lost, self.evict_dirty)
    }

    pub fn inner(&self) -> &D {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut D {
        &mut self.inner
    }

    /// r296 (DIRSNAP): bản ghi này có đang nằm trong cache không, và có bẩn
    /// (chưa xuống đĩa) không. Chỉ để chẩn đoán.
    pub fn slot_state(&self, lba: u32) -> (bool, bool) {
        for slot in self.slots.iter().flatten() {
            if slot.lba == lba {
                return (true, slot.dirty);
            }
        }
        (false, false)
    }

    /// Drop all cached blocks.  Used after formatting a device outside the
    /// cache or after an external writer changes the disk.
    pub fn clear(&mut self) {
        self.slots = [None; CACHE_SLOTS];
        self.next = 0;
    }

    pub(crate) fn insert(
        &mut self,
        lba: u32,
        data: &[u8; BLOCK_SIZE],
        dirty: bool,
    ) -> Result<(), FsError> {
        for slot in self.slots.iter_mut().flatten() {
            if slot.lba == lba {
                slot.data.copy_from_slice(data);
                slot.dirty = dirty;
                return Ok(());
            }
        }
        let index = self
            .slots
            .iter()
            .position(|slot| slot.is_none())
            .unwrap_or_else(|| {
                let index = self.next;
                self.next = (self.next + 1) % CACHE_SLOTS;
                index
            });
        if let Some(old) = self.slots[index] {
            if old.dirty {
                self.inner.write(old.lba, &old.data)?;
                self.evict_dirty = self.evict_dirty.wrapping_add(1);
                // r296: ĐỌC LẠI ngay sau khi đẩy ra — đường ghi này trước đây
                // không có nhân chứng nào.
                let mut verify = [0u8; BLOCK_SIZE];
                if self.inner.read(old.lba, &mut verify).is_ok() && verify != old.data {
                    self.evict_lost = self.evict_lost.wrapping_add(1);
                }
            }
        }
        self.slots[index] = Some(CacheSlot {
            lba,
            data: *data,
            dirty,
        });
        Ok(())
    }
}

impl<D: BlockDev> BlockDev for CachedDev<D> {
    fn read(&mut self, lba: u32, out: &mut [u8; BLOCK_SIZE]) -> Result<(), FsError> {
        for slot in self.slots.iter().flatten() {
            if slot.lba == lba {
                out.copy_from_slice(&slot.data);
                self.hits = self.hits.wrapping_add(1);
                return Ok(());
            }
        }
        let mut data = [0u8; BLOCK_SIZE];
        self.inner.read(lba, &mut data)?;
        out.copy_from_slice(&data);
        self.insert(lba, &data, false)?;
        self.misses = self.misses.wrapping_add(1);
        Ok(())
    }

    fn write(&mut self, lba: u32, data: &[u8; BLOCK_SIZE]) -> Result<(), FsError> {
        if lba < DATA_START {
            self.inner.write(lba, data)?;
            self.insert(lba, data, false)?;
        } else {
            self.insert(lba, data, true)?;
        }
        self.writes = self.writes.wrapping_add(1);
        Ok(())
    }

    fn block_count(&self) -> u32 {
        self.inner.block_count()
    }

    /// Đi thẳng xuống thiết bị, **không** qua slot cache.
    fn read_uncached(&mut self, lba: u32, out: &mut [u8; BLOCK_SIZE]) -> Result<(), FsError> {
        self.inner.read(lba, out)
    }

    fn slot_state(&self, lba: u32) -> (bool, bool) {
        for slot in self.slots.iter().flatten() {
            if slot.lba == lba {
                return (true, slot.dirty);
            }
        }
        (false, false)
    }

    fn evict_stats(&self) -> (u64, u64) {
        (self.evict_lost, self.evict_dirty)
    }

    fn flush(&mut self) -> Result<(), FsError> {
        // T26 (đèn quyết định): ghi xong thì ĐỌC LẠI và so. Nếu lệnh ghi bị
        // thiết bị bỏ im lặng (giả thuyết "burst flush bị QEMU đánh rơi" ghi
        // trong chú thích `CACHE_SLOTS` ở đầu tệp), thì đọc lại sẽ thấy ảnh
        // cũ — đây là cách chứng minh trực tiếp thay vì suy từ status.
        for slot in self.slots.iter_mut().flatten() {
            if slot.dirty {
                self.inner.write(slot.lba, &slot.data)?;
                slot.dirty = false;
                let mut verify = [0u8; BLOCK_SIZE];
                if self.inner.read(slot.lba, &mut verify).is_ok() && verify != slot.data {
                    self.lost_writes = self.lost_writes.wrapping_add(1);
                }
            }
        }
        self.inner.flush()
    }
}

pub(crate) fn get_u16(data: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([data[off], data[off + 1]])
}
pub(crate) fn get_u32(data: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}
pub(crate) fn put_u16(data: &mut [u8], off: usize, value: u16) {
    data[off..off + 2].copy_from_slice(&value.to_le_bytes());
}
pub(crate) fn put_u32(data: &mut [u8], off: usize, value: u32) {
    data[off..off + 4].copy_from_slice(&value.to_le_bytes());
}

/// Precomputed CRC-32 (IEEE, polynomial `0xedb88320`) lookup table.
///
/// Generated at compile time by `build_crc32_table()` so the source carries
/// no 256 hand-typed constants that could hide a typo.  `crc32()` and
/// `crc32_continue()` consult this table instead of shifting one bit at a
/// time (~8 table lookups per byte vs 64 bit-steps).
pub(crate) const fn build_crc32_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0usize;
    while i < 256 {
        let mut crc = i as u32;
        let mut j = 0u32;
        while j < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xedb8_8320;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

pub const CRC32_TABLE: [u32; 256] = build_crc32_table();

pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in data {
        let index = ((crc ^ (*byte as u32)) & 0xff) as usize;
        crc = CRC32_TABLE[index] ^ (crc >> 8);
    }
    !crc
}

pub(crate) fn inode_encode(inode: &Inode, out: &mut [u8]) {
    for byte in out.iter_mut() {
        *byte = 0;
    }
    out[0] = inode.kind;
    out[1] = inode.mode;
    put_u16(out, 2, inode.uid);
    put_u16(out, 4, inode.nlink);
    put_u32(out, 8, inode.size);
    put_u32(out, 12, inode.generation);
    for (index, block) in inode.blocks.iter().enumerate() {
        put_u32(out, 16 + index * 4, *block);
    }
}
pub(crate) fn inode_decode(input: &[u8]) -> Inode {
    let mut inode = Inode {
        kind: input[0],
        mode: input[1],
        uid: get_u16(input, 2),
        nlink: get_u16(input, 4),
        size: get_u32(input, 8),
        generation: get_u32(input, 12),
        ..Default::default()
    };
    for index in 0..MAX_BLOCKS_PER_INODE {
        inode.blocks[index] = get_u32(input, 16 + index * 4);
    }
    inode
}

pub(crate) fn encode_super(
    total_blocks: u32,
    root: u16,
    inode_limit: u16,
    dir_block_limit: u8,
) -> [u8; BLOCK_SIZE] {
    let mut block = [0u8; BLOCK_SIZE];
    block[..4].copy_from_slice(b"VF04");
    put_u32(&mut block, 4, 4);
    put_u32(&mut block, 8, BLOCK_SIZE as u32);
    put_u32(&mut block, 12, total_blocks);
    put_u32(&mut block, 16, inode_limit as u32);
    put_u32(&mut block, 20, BITMAP_START);
    put_u32(&mut block, 24, BITMAP_BLOCKS);
    put_u32(&mut block, 28, INODE_START);
    put_u32(&mut block, 32, INODE_BLOCKS);
    put_u32(&mut block, 36, JOURNAL_START);
    put_u32(&mut block, 40, JOURNAL_BLOCKS);
    put_u32(&mut block, 44, DATA_START);
    put_u32(&mut block, 48, root as u32);
    put_u32(&mut block, 52, dir_block_limit as u32);
    let crc = crc32(&block[..56]);
    put_u32(&mut block, 56, crc);
    block
}

pub(crate) fn validate_super(
    block: &[u8; BLOCK_SIZE],
    capacity: u32,
) -> Result<SuperInfo, FsError> {
    let (version, direct_blocks, max_file_blocks, max_dir_blocks, size_limit) =
        if &block[..4] == b"VF04" && get_u32(block, 4) == 4 {
            (
                4u8,
                V4_DIRECT_BLOCKS,
                MAX_FILE_BLOCKS,
                MAX_DIR_BLOCKS,
                MAX_FILE_SIZE,
            )
        } else if &block[..4] == b"VF03" && get_u32(block, 4) == 3 {
            (
                3u8,
                MAX_DIRECT_BLOCKS,
                MAX_FILE_BLOCKS_V3,
                MAX_DIR_BLOCKS,
                MAX_FILE_SIZE_V3,
            )
        } else if &block[..4] == b"VF02" && get_u32(block, 4) == 2 {
            (
                2u8,
                MAX_BLOCKS_PER_INODE,
                MAX_BLOCKS_PER_INODE,
                MAX_BLOCKS_PER_INODE,
                MAX_BLOCKS_PER_INODE * BLOCK_SIZE,
            )
        } else {
            return Err(FsError::Invalid);
        };
    if get_u32(block, 8) != BLOCK_SIZE as u32 {
        return Err(FsError::Invalid);
    }
    let total = get_u32(block, 12);
    if !(DATA_START + 2..=TOTAL_BLOCKS).contains(&total) || total > capacity {
        return Err(FsError::Invalid);
    }
    let inode_limit = get_u32(block, 16) as usize;
    let dir_block_limit = get_u32(block, 52) as usize;
    if inode_limit == 0
        || inode_limit > MAX_INODES
        || dir_block_limit == 0
        || dir_block_limit > max_dir_blocks
        || get_u32(block, 20) != BITMAP_START
        || get_u32(block, 24) != BITMAP_BLOCKS
        || get_u32(block, 28) != INODE_START
        || get_u32(block, 32) != INODE_BLOCKS
        || get_u32(block, 36) != JOURNAL_START
        || get_u32(block, 40) != JOURNAL_BLOCKS
        || get_u32(block, 44) != DATA_START
        || get_u32(block, 48) != ROOT_INO as u32
    {
        return Err(FsError::Invalid);
    }
    let mut copy = *block;
    let expected = get_u32(block, 56);
    put_u32(&mut copy, 56, 0);
    if crc32(&copy[..56]) != expected {
        return Err(FsError::Corrupt);
    }
    Ok(SuperInfo {
        version,
        inode_limit,
        dir_block_limit,
        direct_blocks,
        max_file_blocks,
        max_file_size: size_limit,
        total,
    })
}

pub(crate) fn zero_block() -> [u8; BLOCK_SIZE] {
    [0u8; BLOCK_SIZE]
}

/// Format a block device with an empty root directory.  Formatting is not
/// journaled (there is no old filesystem to preserve); every call is
/// deterministic and writes all metadata.
pub fn format<D: BlockDev>(dev: &mut D) -> Result<(), FsError> {
    format_with_options(dev, FormatOptions::default())
}

pub fn format_with_options<D: BlockDev>(
    dev: &mut D,
    options: FormatOptions,
) -> Result<(), FsError> {
    if dev.block_count() < DATA_START + 2 {
        return Err(FsError::NoSpace);
    }
    if options.inode_limit == 0 || options.inode_limit as usize > MAX_INODES {
        return Err(FsError::Invalid);
    }
    if options.dir_block_limit == 0 || options.dir_block_limit as usize > MAX_DIR_BLOCKS {
        return Err(FsError::Invalid);
    }
    let capacity = dev.block_count();
    let total = if options.total_blocks == 0 {
        capacity.min(TOTAL_BLOCKS)
    } else {
        options.total_blocks.min(capacity).min(TOTAL_BLOCKS)
    };
    if total < DATA_START + 2 {
        return Err(FsError::NoSpace);
    }
    let mut zero = zero_block();
    dev.write(0, &zero)?;
    let superblock = encode_super(
        total,
        ROOT_INO,
        options.inode_limit,
        options.dir_block_limit,
    );
    dev.write(1, &superblock)?;
    zero = zero_block();
    for lba in BITMAP_START..BITMAP_START + BITMAP_BLOCKS {
        dev.write(lba, &zero)?;
    }
    let mut inode_block = zero_block();
    let root = Inode {
        kind: INODE_DIR,
        mode: 0b0111,
        uid: 1,
        nlink: 1,
        size: 0,
        generation: 1,
        blocks: [0; MAX_BLOCKS_PER_INODE],
    };
    inode_encode(&root, &mut inode_block[..INODE_SIZE]);
    dev.write(INODE_START, &inode_block)?;
    zero = zero_block();
    for lba in INODE_START + 1..INODE_START + INODE_BLOCKS {
        dev.write(lba, &zero)?;
    }
    for lba in JOURNAL_START..JOURNAL_START + JOURNAL_BLOCKS {
        dev.write(lba, &zero)?;
    }
    Ok(())
}

/// Maximum blocks staged in one journal transaction.  The on-disk journal
/// holds `JOURNAL_BLOCKS - 2` data blocks and the commit header reserves
/// exactly this many target slots (`header[16..80]`).
pub const MAX_TX_WRITES: usize = 16;

/// r296 (DIRSNAP): ảnh chụp ba đường đọc của một block thư mục.
#[derive(Clone, Copy, Default)]
pub struct DirSnap {
    pub blocks: u32,
    pub blk0: u32,
    pub isize: u32,
    pub index: u32,
    pub cached: bool,
    pub dirty: bool,
    pub n_cache: u32,
    pub n_raw: u32,
    pub n_jrnl: u32,
    pub lookup: u8,
    pub lookup_ino: u16,
    pub tx: u32,
    /// Tên của từng slot trong block 0 (để chứng minh `d` có/không có mặt).
    pub names: [[u8; MAX_NAME]; DIR_ENTRIES_PER_BLOCK],
    pub name_len: [u8; DIR_ENTRIES_PER_BLOCK],
    pub inos: [u16; DIR_ENTRIES_PER_BLOCK],
}

/// Đếm entry hợp lệ (ino != 0) trong một block thư mục.
pub(crate) fn count_dir_entries(block: &[u8; BLOCK_SIZE]) -> u32 {
    let mut count = 0u32;
    for slot in 0..DIR_ENTRIES_PER_BLOCK {
        let off = slot * DIR_ENTRY_SIZE;
        if get_u16(block, off) != 0 {
            count += 1;
        }
    }
    count
}

pub struct Fs<D: BlockDev> {
    pub(crate) dev: D,
    pub(crate) inodes: [Inode; MAX_INODES],
    pub(crate) bitmap: [u8; (BITMAP_BLOCKS as usize) * BLOCK_SIZE],
    pub(crate) journal: [u8; (JOURNAL_BLOCKS as usize - 2) * BLOCK_SIZE],
    pub(crate) targets: [u32; MAX_TX_WRITES],
    pub(crate) tx_count: usize,
    pub(crate) seq: u32,
    pub(crate) replay_status: u8,
    pub(crate) migrated: bool,
    pub(crate) inode_limit: usize,
    pub(crate) dir_block_limit: usize,
    pub(crate) version: u8,
    pub(crate) direct_blocks: usize,
    pub(crate) max_file_blocks: usize,
    pub(crate) max_file_size: usize,
    /// Running count of allocated blocks in `DATA_START..block_count`.
    /// Maintained by `mark_used`/`mark_free` (bit flips only) and rebuilt by
    /// `load_metadata`, so `usage()` is O(1) instead of scanning the bitmap.
    pub(crate) used_blocks: u32,
    /// P1-1 (r290): số block của FS theo superblock — biên THẬT của bitmap và
    /// của vùng cấp phát. Thiết bị có thể lớn hơn (đĩa gắn thêm) nhưng FS
    /// không được quét/ghi ra ngoài `capacity`.
    pub(crate) capacity: u32,
    /// Hint where the next `alloc_block()` scan starts.  Freed blocks below
    /// the cursor pull it back, so steady-state allocation is O(1) instead
    /// of re-scanning from `DATA_START` every time.
    pub(crate) next_free: u32,
}

pub(crate) const EMPTY_INODE: Inode = Inode {
    kind: INODE_FREE,
    mode: 0,
    uid: 0,
    nlink: 0,
    size: 0,
    generation: 0,
    blocks: [0; MAX_BLOCKS_PER_INODE],
};

impl<D: BlockDev> Fs<D> {
    pub const fn empty(dev: D) -> Self {
        Self {
            dev,
            inodes: [EMPTY_INODE; MAX_INODES],
            bitmap: [0u8; (BITMAP_BLOCKS as usize) * BLOCK_SIZE],
            journal: [0u8; (JOURNAL_BLOCKS as usize - 2) * BLOCK_SIZE],
            targets: [0; MAX_TX_WRITES],
            tx_count: 0,
            seq: 1,
            replay_status: 0,
            migrated: false,
            inode_limit: MAX_INODES,
            dir_block_limit: MAX_DIR_BLOCKS,
            version: 4,
            direct_blocks: V4_DIRECT_BLOCKS,
            max_file_blocks: MAX_FILE_BLOCKS,
            max_file_size: MAX_FILE_SIZE,
            used_blocks: 0,
            capacity: 0,
            next_free: DATA_START,
        }
    }

    /// Initialize a preallocated filesystem object in place.  Server uses
    /// this so the 13 KiB FS state lives in `.bss`, not on its one-page stack.
    pub fn initialize(&mut self) -> Result<(), FsError> {
        self.inodes = [EMPTY_INODE; MAX_INODES];
        self.bitmap = [0u8; (BITMAP_BLOCKS as usize) * BLOCK_SIZE];
        self.journal = [0u8; (JOURNAL_BLOCKS as usize - 2) * BLOCK_SIZE];
        self.targets = [0; MAX_TX_WRITES];
        self.tx_count = 0;
        self.seq = 1;
        self.replay_status = 0;
        self.migrated = false;
        self.inode_limit = MAX_INODES;
        self.dir_block_limit = MAX_DIR_BLOCKS;
        self.version = 4;
        self.direct_blocks = V4_DIRECT_BLOCKS;
        self.max_file_blocks = MAX_FILE_BLOCKS;
        self.max_file_size = MAX_FILE_SIZE;
        self.used_blocks = 0;
        self.next_free = DATA_START;
        let capacity = self.dev.block_count();
        self.capacity = capacity;
        let mut superblock = zero_block();
        self.dev.read(1, &mut superblock)?;
        let mut legacy = zero_block();
        let mut is_legacy = false;
        if validate_super(&superblock, capacity).is_err() {
            self.dev.read(0, &mut legacy)?;
            if &legacy[..4] != b"VF01" {
                return Err(FsError::Invalid);
            }
            is_legacy = true;
            format(&mut self.dev)?;
            self.dev.read(1, &mut superblock)?;
        }
        let info = validate_super(&superblock, capacity)?;
        self.capacity = info.total;
        self.inode_limit = info.inode_limit;
        self.dir_block_limit = info.dir_block_limit;
        self.version = info.version;
        self.direct_blocks = info.direct_blocks;
        self.max_file_blocks = info.max_file_blocks;
        self.max_file_size = info.max_file_size;
        self.load_metadata()?;
        self.replay()?;
        if self.replay_status != 0 {
            self.load_metadata()?;
        }
        if is_legacy {
            let len = get_u32(&legacy, 4) as usize;
            let len = core::cmp::min(len, 64);
            self.migrated = true;
            let ino = self.create(b"/scratch", 0, INODE_FILE)?;
            self.write(ino, 0, &legacy[8..8 + len], 0)?;
            let zero = zero_block();
            self.dev.write(0, &zero)?;
        }
        Ok(())
    }

    pub fn mount(dev: D) -> Result<Self, FsError> {
        let mut fs = Self::empty(dev);
        fs.initialize()?;
        Ok(fs)
    }

    pub fn migrated(&self) -> bool {
        self.migrated
    }

    pub fn replayed(&self) -> u8 {
        self.replay_status
    }

    /// Access the underlying device for cache statistics, explicit format, or
    /// future flush operations.
    pub fn dev(&self) -> &D {
        &self.dev
    }

    pub fn dev_mut(&mut self) -> &mut D {
        &mut self.dev
    }

    pub(crate) fn fsck_mark_block(
        &self,
        block: u32,
        block_used: &mut [u8; (BITMAP_BLOCKS as usize) * BLOCK_SIZE],
    ) -> Result<(), FsError> {
        // P1-1/291.6: biên là FS (capacity), không phải dung lượng thiết bị.
        if block < DATA_START || block >= self.capacity {
            return Err(FsError::Corrupt);
        }
        let byte = block as usize / 8;
        let bit = 1u8 << (block as usize % 8);
        if block_used[byte] & bit != 0 {
            return Err(FsError::Corrupt);
        }
        block_used[byte] |= bit;
        Ok(())
    }

    pub(crate) fn fsck_file_tables(
        &mut self,
        inode: &Inode,
        used_blocks: usize,
        block_used: &mut [u8; (BITMAP_BLOCKS as usize) * BLOCK_SIZE],
    ) -> Result<(), FsError> {
        let direct = self.direct_blocks;
        // r299: VF02 có direct = MAX_BLOCKS_PER_INODE (= 12) nên không còn
        // slot nào cho bảng gián tiếp — `blocks[direct]` là index quá mảng
        // [u32; 12], panic ngay khi mount+fsck ảnh v2 chứa file. File v2
        // không vượt 12 block nên không có gì để kiểm thêm.
        if direct >= MAX_BLOCKS_PER_INODE {
            return Ok(());
        }
        let single = inode.blocks[direct];
        if self.version < 4 {
            if used_blocks <= direct {
                if single != 0 {
                    return Err(FsError::Corrupt);
                }
                return Ok(());
            }
            self.fsck_mark_block(single, block_used)?;
            let mut table = zero_block();
            self.dev.read(single, &mut table)?;
            let keep = used_blocks - direct;
            for slot in 0..INDIRECT_ENTRIES {
                if slot >= keep && get_u32(&table, slot * 4) != 0 {
                    return Err(FsError::Corrupt);
                }
            }
            return Ok(());
        }

        if used_blocks > direct && single != 0 {
            // r299: single == 0 khi used_blocks > direct là LỖ của file thưa
            // (dữ liệu nằm hẳn ở vùng double) — hợp lệ, chỉ bỏ qua đánh dấu.
            // Bản cũ trả Corrupt cho chính image do writer tạo ra.
            self.fsck_mark_block(single, block_used)?;
            let mut table = zero_block();
            self.dev.read(single, &mut table)?;
            let keep = core::cmp::min(used_blocks - direct, INDIRECT_ENTRIES);
            for slot in 0..INDIRECT_ENTRIES {
                if slot >= keep && get_u32(&table, slot * 4) != 0 {
                    return Err(FsError::Corrupt);
                }
            }
        } else if used_blocks <= direct && single != 0 {
            return Err(FsError::Corrupt);
        }

        let double_index = direct + 1;
        let double = inode.blocks[double_index];
        if used_blocks > direct + INDIRECT_ENTRIES {
            if double == 0 {
                return Err(FsError::Corrupt);
            }
            self.fsck_mark_block(double, block_used)?;
            let mut table = zero_block();
            self.dev.read(double, &mut table)?;
            let double_used = used_blocks - direct - INDIRECT_ENTRIES;
            let keep_tables = double_used.div_ceil(INDIRECT_ENTRIES);
            for slot in 0..INDIRECT_ENTRIES {
                let second = get_u32(&table, slot * 4);
                if slot < keep_tables && second != 0 {
                    // r299: second == 0 trong phạm vi keep là lỗ thưa giữa
                    // chừng — bỏ qua như single ở trên; chỉ đánh dấu bảng có
                    // thật.
                    self.fsck_mark_block(second, block_used)?;
                    let mut second_table = zero_block();
                    self.dev.read(second, &mut second_table)?;
                    let keep = if slot + 1 == keep_tables {
                        double_used - slot * INDIRECT_ENTRIES
                    } else {
                        INDIRECT_ENTRIES
                    };
                    for entry in 0..INDIRECT_ENTRIES {
                        if entry >= keep && get_u32(&second_table, entry * 4) != 0 {
                            return Err(FsError::Corrupt);
                        }
                    }
                } else if slot >= keep_tables && second != 0 {
                    return Err(FsError::Corrupt);
                }
            }
        } else if double != 0 {
            return Err(FsError::Corrupt);
        }
        Ok(())
    }

    /// Vòng 163: thư mục gốc có TRỐNG không (không tính "." và "..").
    /// Dùng để quyết định có được phép format khi fsck phát hiện bất nhất
    /// hay không: chỉ sửa khi chắc chắn không mất dữ liệu người dùng.
    pub fn root_is_empty(&mut self) -> bool {
        let mut index = 0usize;
        while let Ok(Some((ino, name, len))) = self.read_dir_entry(ROOT_INO, index) {
            index += 1;
            if ino == 0 {
                // Ô trống (r296b: chỉ số là SLOT) — đi tiếp, không phải "hết".
                continue;
            }
            let entry = &name[..len.min(MAX_NAME)];
            if entry != b"." && entry != b".." {
                return false;
            }
        }
        true
    }

    pub fn fsck(&mut self) -> Result<(), FsError> {
        let mut superblock = zero_block();
        self.dev.read(1, &mut superblock)?;
        let info = validate_super(&superblock, self.dev.block_count())?;
        let inode_limit = info.inode_limit;
        let dir_limit_bytes = info.dir_block_limit as u32 * BLOCK_SIZE as u32;
        let mut used = [false; MAX_INODES];
        let mut nlink = [0u16; MAX_INODES];
        let mut block_used = [0u8; (BITMAP_BLOCKS as usize) * BLOCK_SIZE];
        let mut stack = [0u16; MAX_INODES];
        let mut stack_len = 1usize;
        stack[0] = ROOT_INO;
        used[ROOT_INO as usize - 1] = true;
        nlink[ROOT_INO as usize - 1] = 1;
        while stack_len != 0 {
            stack_len -= 1;
            let ino = stack[stack_len];
            let inode = self.inodes[ino as usize - 1];
            if inode.kind == INODE_DIR {
                if !inode.size.is_multiple_of(BLOCK_SIZE as u32)
                    || inode.size > dir_limit_bytes
                    || inode.blocks_used() != (inode.size / BLOCK_SIZE as u32) as usize
                {
                    return Err(FsError::Corrupt);
                }
                for (block_index, block) in inode.blocks.iter().enumerate() {
                    if block_index >= inode.blocks_used() {
                        if *block != 0 {
                            return Err(FsError::Corrupt);
                        }
                        continue;
                    }
                    if *block < DATA_START || *block >= self.capacity {
                        return Err(FsError::Corrupt);
                    }
                    let mut data = zero_block();
                    self.dev.read(*block, &mut data)?;
                    for slot in 0..DIR_ENTRIES_PER_BLOCK {
                        let off = slot * DIR_ENTRY_SIZE;
                        let child = get_u16(&data, off);
                        if child == 0 {
                            continue;
                        }
                        let name_len = data[off + 2] as usize;
                        if name_len == 0
                            || name_len > MAX_NAME
                            || child as usize > inode_limit
                            || child == 0
                        {
                            return Err(FsError::Corrupt);
                        }
                        let name = &data[off + 3..off + 3 + name_len];
                        if name == b"."
                            || name == b".."
                            || name.contains(&b'/')
                            || name.contains(&0)
                        {
                            return Err(FsError::Corrupt);
                        }
                        let child_inode = self.inodes[child as usize - 1];
                        if child_inode.kind == INODE_FREE {
                            return Err(FsError::Corrupt);
                        }
                        if !used[child as usize - 1] {
                            used[child as usize - 1] = true;
                            if stack_len >= MAX_INODES {
                                return Err(FsError::Corrupt);
                            }
                            stack[stack_len] = child;
                            stack_len += 1;
                        }
                        nlink[child as usize - 1] = nlink[child as usize - 1].saturating_add(1);
                        if child_inode.kind == INODE_DIR {
                            nlink[ino as usize - 1] = nlink[ino as usize - 1].saturating_add(1);
                        }
                    }
                }
            }
        }
        for index in 0..inode_limit {
            let inode = self.inodes[index];
            if inode.kind == INODE_FREE {
                continue;
            }
            if !used[index] {
                return Err(FsError::Corrupt);
            }
            if inode.kind != INODE_FILE && inode.kind != INODE_DIR && inode.kind != INODE_SYMLINK {
                return Err(FsError::Corrupt);
            }
            if inode.size as usize > self.max_file_size {
                return Err(FsError::Corrupt);
            }
            if (inode.kind == INODE_FILE || inode.kind == INODE_SYMLINK)
                && inode.blocks_used() != (inode.size as usize).div_ceil(BLOCK_SIZE)
            {
                return Err(FsError::Corrupt);
            }
            if inode.nlink != nlink[index] {
                return Err(FsError::Corrupt);
            }
            let used_blocks = inode.blocks_used();
            if inode.kind == INODE_FILE || inode.kind == INODE_SYMLINK {
                if used_blocks > self.max_file_blocks {
                    return Err(FsError::Corrupt);
                }
                for block_index in 0..used_blocks {
                    let block = self.file_block(&inode, block_index)?;
                    // 291.3 (r291): block 0 trong phạm vi `size` là LỖ của file
                    // thưa — hợp lệ, không có gì để đánh dấu. Bản cũ gọi
                    // `fsck_mark_block(0)` ⇒ `0 < DATA_START` ⇒ Corrupt.
                    if block == 0 {
                        continue;
                    }
                    self.fsck_mark_block(block, &mut block_used)?;
                }
                self.fsck_file_tables(&inode, used_blocks, &mut block_used)?;
            } else {
                let mut count = 0usize;
                for (block_index, block) in inode.blocks.iter().enumerate() {
                    if *block == 0 {
                        continue;
                    }
                    if block_index >= used_blocks {
                        return Err(FsError::Corrupt);
                    }
                    self.fsck_mark_block(*block, &mut block_used)?;
                    count += 1;
                }
                if count != used_blocks {
                    return Err(FsError::Corrupt);
                }
            }
        }
        // P1-1: chỉ kiểm trong vùng FS; ngoài `capacity` là đĩa thừa, không
        // phải lỗi của FS.
        for block in DATA_START..self.capacity {
            let bit = self.bitmap[block as usize / 8] & (1 << (block as usize % 8)) != 0;
            let expected = block_used[block as usize / 8] & (1 << (block as usize % 8)) != 0;
            if bit != expected {
                return Err(FsError::Corrupt);
            }
        }
        for block in 0..DATA_START {
            if self.bitmap[block as usize / 8] & (1 << (block as usize % 8)) != 0 {
                return Err(FsError::Corrupt);
            }
        }
        Ok(())
    }

    pub(crate) fn load_metadata(&mut self) -> Result<(), FsError> {
        let mut block = zero_block();
        for index in 0..BITMAP_BLOCKS as usize {
            self.dev.read(BITMAP_START + index as u32, &mut block)?;
            self.bitmap[index * BLOCK_SIZE..(index + 1) * BLOCK_SIZE].copy_from_slice(&block);
        }
        // Rebuild the O(1) usage counter and the allocation cursor from the
        // on-disk bitmap (mount / replay path).  `used_blocks` only counts
        // the DATA region, matching `alloc_block`'s domain; `next_free`
        // points at the first hole so the first allocation is cheap.
        // P1-1: quét theo biên của FS, không theo dung lượng thiết bị.
        let capacity = self.capacity;
        self.used_blocks = 0;
        self.next_free = capacity;
        for block_no in DATA_START..capacity {
            if self.block_is_used(block_no) {
                self.used_blocks = self.used_blocks.saturating_add(1);
            } else if self.next_free == capacity {
                self.next_free = block_no;
            }
        }
        for index in 0..INODE_BLOCKS as usize {
            self.dev.read(INODE_START + index as u32, &mut block)?;
            for slot in 0..8 {
                let ino = index * 8 + slot;
                self.inodes[ino] = inode_decode(&block[slot * INODE_SIZE..(slot + 1) * INODE_SIZE]);
            }
        }
        Ok(())
    }

    pub(crate) fn replay(&mut self) -> Result<(), FsError> {
        let mut commit = zero_block();
        if self.dev.read(JOURNAL_COMMIT_LBA, &mut commit).is_err() {
            return Ok(());
        }
        if &commit[..4] != b"VC01" {
            return Ok(());
        }
        let seq = get_u32(&commit, 4);
        let expected = get_u32(&commit, 8);
        let mut header = zero_block();
        if self.dev.read(JOURNAL_START, &mut header).is_err() {
            self.replay_status = 2;
            return Ok(());
        }
        if &header[..4] != b"VJ01" || get_u32(&header, 4) != seq {
            self.replay_status = 2;
            let zero = zero_block();
            let _ = self.dev.write(JOURNAL_COMMIT_LBA, &zero);
            return Ok(());
        }
        let count = header[8] as usize;
        if count == 0 || count > 16 {
            self.replay_status = 2;
            let zero = zero_block();
            let _ = self.dev.write(JOURNAL_COMMIT_LBA, &zero);
            return Ok(());
        }
        for index in 0..count {
            let mut data = zero_block();
            if self
                .dev
                .read(JOURNAL_DATA_START + index as u32, &mut data)
                .is_err()
            {
                self.replay_status = 2;
                return Ok(());
            }
            let start = index * BLOCK_SIZE;
            self.journal[start..start + BLOCK_SIZE].copy_from_slice(&data);
        }
        let mut crc = crc32(&header[..80]);
        for index in 0..count {
            crc = crc32_continue(
                crc,
                &self.journal[index * BLOCK_SIZE..(index + 1) * BLOCK_SIZE],
            );
        }
        if crc != expected {
            self.replay_status = 2;
            let zero = zero_block();
            let _ = self.dev.write(JOURNAL_COMMIT_LBA, &zero);
            return Ok(());
        }
        for index in 0..count {
            let target = get_u32(&header, 16 + index * 4);
            if target == 0 || target >= self.dev.block_count() {
                self.replay_status = 2;
                return Ok(());
            }
            let mut data = zero_block();
            data.copy_from_slice(&self.journal[index * BLOCK_SIZE..(index + 1) * BLOCK_SIZE]);
            if self.dev.write(target, &data).is_err() {
                return Err(FsError::Io);
            }
        }
        self.replay_status = 1;
        self.dev.flush()?;
        let zero = zero_block();
        let _ = self.dev.write(JOURNAL_COMMIT_LBA, &zero);
        Ok(())
    }
}

pub(crate) fn crc32_continue(mut crc: u32, data: &[u8]) -> u32 {
    crc = !crc;
    for byte in data {
        let index = ((crc ^ (*byte as u32)) & 0xff) as usize;
        crc = CRC32_TABLE[index] ^ (crc >> 8);
    }
    !crc
}
