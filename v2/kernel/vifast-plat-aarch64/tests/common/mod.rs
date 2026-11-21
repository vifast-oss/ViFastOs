// SPDX-License-Identifier: MIT OR Apache-2.0
//! Nguồn frame + hằng số **dùng chung** cho các test host của `vifast-plat-aarch64`.
//!
//! Tách khỏi `host.rs` khi KT-305 thêm bộ test block (`block.rs`): luật **R1** giới hạn 1.500
//! dòng/tệp trong `kernel/`, nên hai tệp test không thể mỗi tệp giữ một bản `TestMemory` — đó
//! cũng là "hai bản trùng" mà D2-163 đã đóng. `mod common;` được biên dịch riêng cho **mỗi**
//! test crate nhưng chỉ có **một** nguồn.

#![allow(dead_code)]

use vifast_plat_aarch64::format::Aarch64;
use vifast_plat_aarch64::mmu::{PageTable, TableMemory};

pub const ENTRIES: usize = 512;
/// Cỡ frame của test.
pub const FRAME_BYTES: u64 = 4096;
/// Rác bơm vào frame lúc free — để chốt "frame cấp lại phải được zero".
pub const GARBAGE: u64 = 0xDEAD_BEEF_DEAD_BEEF;
/// PA gốc của nguồn frame test (4 KiB aligned).
pub const FRAME_BASE: u64 = 0x4000_0000;
/// VA mẫu nửa thấp (canonical 39-bit).
pub const VA: u64 = 0x0000_0000_0000_1000;
/// PA mẫu.
pub const PA: u64 = 0x0000_0000_8000_1000;

// ---------------------------------------------------------------- nguồn frame test

/// Nguồn frame + bộ nhớ bảng trang cho host: `Vec` frame 4 KiB, free-list LIFO, đếm alloc/free.
///
/// Cố ý **không** zero ở `alloc_frame` (đúng hợp đồng [`TableMemory`]): frame tái sử dụng còn
/// rác [`GARBAGE`] từ `free_frame`, nên nếu `Mmu` quên `zero_frame` thì test đỏ ngay.
pub struct TestMemory {
    base: u64,
    frames: Vec<Option<Box<[u64; ENTRIES]>>>,
    free: Vec<usize>,
    allocs: usize,
    frees: usize,
    pub fail_after: Option<usize>,
}

impl TestMemory {
    pub fn new(base: u64) -> Self {
        Self {
            base,
            frames: Vec::new(),
            free: Vec::new(),
            allocs: 0,
            frees: 0,
            fail_after: None,
        }
    }

    pub fn outstanding(&self) -> usize {
        self.allocs - self.frees
    }

    pub fn allocs(&self) -> usize {
        self.allocs
    }

    pub fn frees(&self) -> usize {
        self.frees
    }

    fn index_of(&self, pa: u64) -> Option<usize> {
        let off = pa.checked_sub(self.base)?;
        if off % FRAME_BYTES != 0 {
            return None;
        }
        let index = (off / FRAME_BYTES) as usize;
        if index < self.frames.len() {
            Some(index)
        } else {
            None
        }
    }
}

impl TableMemory for TestMemory {
    fn alloc_frame(&mut self) -> Option<u64> {
        if let Some(limit) = self.fail_after {
            if self.allocs >= limit {
                return None;
            }
        }
        let index = match self.free.pop() {
            // Tái sử dụng: giữ nguyên rác — `Mmu` phải tự zero.
            Some(index) => index,
            None => {
                self.frames.push(Some(Box::new([0u64; ENTRIES])));
                self.frames.len() - 1
            }
        };
        self.allocs += 1;
        Some(self.base + index as u64 * FRAME_BYTES)
    }

    fn free_frame(&mut self, pa: u64) {
        let index = self.index_of(pa).expect("free_frame: PA lạ");
        assert!(
            !self.free.contains(&index),
            "free_frame: double free frame {pa:#x}"
        );
        if let Some(frame) = self.frames[index].as_mut() {
            for entry in frame.iter_mut() {
                *entry = GARBAGE;
            }
        }
        self.frees += 1;
        self.free.push(index);
    }

    fn read_entry(&self, pa: u64, index: usize) -> Option<u64> {
        if index >= ENTRIES {
            return None;
        }
        let frame = self.index_of(pa)?;
        self.frames
            .get(frame)?
            .as_ref()
            .map(|entries| entries[index])
    }

    fn write_entry(&mut self, pa: u64, index: usize, entry: u64) -> bool {
        if index >= ENTRIES {
            return false;
        }
        let Some(frame) = self.index_of(pa) else {
            return false;
        };
        match self.frames.get_mut(frame) {
            Some(Some(entries)) => {
                entries[index] = entry;
                true
            }
            _ => false,
        }
    }

    fn zero_frame(&mut self, pa: u64) -> bool {
        let Some(frame) = self.index_of(pa) else {
            return false;
        };
        match self.frames.get_mut(frame) {
            Some(Some(entries)) => {
                for entry in entries.iter_mut() {
                    *entry = 0;
                }
                true
            }
            _ => false,
        }
    }
}

pub fn new_mmu() -> PageTable<Aarch64, TestMemory> {
    PageTable::new(TestMemory::new(FRAME_BASE)).expect("cấp frame gốc")
}
