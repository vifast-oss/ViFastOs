// SPDX-License-Identifier: MIT OR Apache-2.0
// PORTED-FROM: crates/vifast-fs/src/lib.rs@ca2eaf5
extern crate std;

use crate::consts::*;
use crate::fs::*;

use std::vec;
use std::vec::Vec;

#[derive(Clone)]
pub(crate) struct MemDev {
    data: Vec<u8>,
}
impl MemDev {
    pub(crate) fn new() -> Self {
        Self {
            data: vec![0u8; TOTAL_BLOCKS as usize * BLOCK_SIZE],
        }
    }
    pub(crate) fn with_blocks(blocks: u32) -> Self {
        Self {
            data: vec![0u8; blocks as usize * BLOCK_SIZE],
        }
    }
}
impl BlockDev for MemDev {
    fn read(&mut self, lba: u32, out: &mut [u8; BLOCK_SIZE]) -> Result<(), FsError> {
        let start = lba as usize * BLOCK_SIZE;
        out.copy_from_slice(&self.data[start..start + BLOCK_SIZE]);
        Ok(())
    }
    fn write(&mut self, lba: u32, input: &[u8; BLOCK_SIZE]) -> Result<(), FsError> {
        let start = lba as usize * BLOCK_SIZE;
        self.data[start..start + BLOCK_SIZE].copy_from_slice(input);
        Ok(())
    }
    fn block_count(&self) -> u32 {
        (self.data.len() / BLOCK_SIZE) as u32
    }
}

pub(crate) struct CountingDev {
    inner: MemDev,
    reads: usize,
    writes: usize,
    flushes: usize,
}
impl CountingDev {
    pub(crate) fn new() -> Self {
        Self {
            inner: MemDev::new(),
            reads: 0,
            writes: 0,
            flushes: 0,
        }
    }
}
impl BlockDev for CountingDev {
    fn read(&mut self, lba: u32, out: &mut [u8; BLOCK_SIZE]) -> Result<(), FsError> {
        self.reads += 1;
        self.inner.read(lba, out)
    }
    fn write(&mut self, lba: u32, input: &[u8; BLOCK_SIZE]) -> Result<(), FsError> {
        self.writes += 1;
        self.inner.write(lba, input)
    }
    fn block_count(&self) -> u32 {
        self.inner.block_count()
    }
    fn flush(&mut self) -> Result<(), FsError> {
        self.flushes += 1;
        Ok(())
    }
}

struct CrashDev {
    data: Vec<u8>,
    writes: usize,
    fail_at: usize,
    failed: bool,
}
impl BlockDev for CrashDev {
    fn read(&mut self, lba: u32, out: &mut [u8; BLOCK_SIZE]) -> Result<(), FsError> {
        let start = lba as usize * BLOCK_SIZE;
        out.copy_from_slice(&self.data[start..start + BLOCK_SIZE]);
        Ok(())
    }
    fn write(&mut self, lba: u32, input: &[u8; BLOCK_SIZE]) -> Result<(), FsError> {
        if self.writes == self.fail_at {
            self.failed = true;
            return Err(FsError::Io);
        }
        self.writes += 1;
        let start = lba as usize * BLOCK_SIZE;
        self.data[start..start + BLOCK_SIZE].copy_from_slice(input);
        Ok(())
    }
    fn block_count(&self) -> u32 {
        (self.data.len() / BLOCK_SIZE) as u32
    }
}

pub(crate) fn fresh() -> Fs<MemDev> {
    let mut dev = MemDev::new();
    format(&mut dev).unwrap();
    Fs::mount(dev).unwrap()
}
pub(crate) fn reopen(fs: &Fs<MemDev>) -> Fs<MemDev> {
    Fs::mount(fs.dev.clone()).unwrap()
}

/// `use super::*` che mất macro `format!` (crate có hàm `format` riêng)
/// nên tên sinh bằng tay.
pub(crate) fn index_name(out: &mut [u8], index: u32) -> usize {
    let mut digits = [0u8; 10];
    let mut len = 0usize;
    let mut value = index;
    if value == 0 {
        digits[0] = b'0';
        len = 1;
    }
    while value > 0 {
        digits[len] = b'0' + (value % 10) as u8;
        value /= 10;
        len += 1;
    }
    digits[..len].reverse();
    out[..len].copy_from_slice(&digits[..len]);
    len
}

/// Lấp đầy thư mục `dir` tới mức dir_block_limit bằng hard-link tới một
/// file (không tốn inode — chỉ có 64 inode nhưng dir chứa được 11*16
/// entry); trả số entry lấp được.
pub(crate) fn fill_dir_with_links(fs: &mut Fs<MemDev>, dir: &[u8], prefix: u8) -> u32 {
    fs.create(b"/src", 1, INODE_FILE).unwrap();
    let mut index = 0u32;
    loop {
        let mut path = [0u8; 40];
        path[..dir.len()].copy_from_slice(dir);
        path[dir.len()] = b'/';
        path[dir.len() + 1] = prefix;
        let len = index_name(&mut path[dir.len() + 2..], index);
        let total = dir.len() + 2 + len;
        if fs.link(b"/src", &path[..total], 1).is_err() {
            return index;
        }
        index += 1;
    }
}

mod part1;
mod part2;
