// SPDX-License-Identifier: MIT OR Apache-2.0
// PORTED-FROM: crates/vifast-fs/src/lib.rs@ca2eaf5

pub const BLOCK_SIZE: usize = 512;
pub const TOTAL_BLOCKS: u32 = 8192;
pub const BITMAP_START: u32 = 2;
pub const BITMAP_BLOCKS: u32 = 2;
pub const INODE_START: u32 = 4;
pub const INODE_BLOCKS: u32 = 8;
pub const JOURNAL_START: u32 = 12;
pub const JOURNAL_BLOCKS: u32 = 18;
pub const JOURNAL_DATA_START: u32 = 13;
pub const JOURNAL_COMMIT_LBA: u32 = 29;
pub const DATA_START: u32 = 30;
pub const MAX_INODES: usize = 64;
pub const INODE_SIZE: usize = 64;
pub const MAX_BLOCKS_PER_INODE: usize = 12;
/// Legacy `VF03` geometry: 11 direct blocks + one single-indirect block.
pub const MAX_DIRECT_BLOCKS: usize = 11;
pub const INDIRECT_ENTRIES: usize = BLOCK_SIZE / 4;
pub const MAX_FILE_BLOCKS_V3: usize = MAX_DIRECT_BLOCKS + INDIRECT_ENTRIES;
pub const MAX_FILE_SIZE_V3: usize = MAX_FILE_BLOCKS_V3 * BLOCK_SIZE;
/// `VF04` geometry: 10 direct + one single-indirect + one double-indirect.
pub const V4_DIRECT_BLOCKS: usize = 10;
pub const V4_SINGLE_INDIRECT_INDEX: usize = V4_DIRECT_BLOCKS;
pub const V4_DOUBLE_INDIRECT_INDEX: usize = V4_DIRECT_BLOCKS + 1;
pub const MAX_FILE_BLOCKS: usize =
    V4_DIRECT_BLOCKS + INDIRECT_ENTRIES + INDIRECT_ENTRIES * INDIRECT_ENTRIES;
pub const MAX_FILE_SIZE: usize = MAX_FILE_BLOCKS * BLOCK_SIZE;
pub const MAX_DIR_BLOCKS: usize = MAX_DIRECT_BLOCKS;
pub const DIR_ENTRY_SIZE: usize = 32;
pub const DIR_ENTRIES_PER_BLOCK: usize = 16;
pub const MAX_NAME: usize = 28;
pub const MAX_PATH: usize = 128;
pub const ROOT_INO: u16 = 1;
pub const INODE_FREE: u8 = 0;
pub const INODE_FILE: u8 = 1;
pub const INODE_DIR: u8 = 2;
pub const INODE_SYMLINK: u8 = 3;
pub const MAX_SYMLINK_DEPTH: usize = 8;
