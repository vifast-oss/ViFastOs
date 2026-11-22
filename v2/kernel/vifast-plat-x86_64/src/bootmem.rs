// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-340b** — nguồn frame **động** cho bảng trang x86_64 ngoài `.bss` (khuôn `KT-338a`).
//!
//! # Vì sao
//!
//! `vifast-pte::PageTable` cần một nguồn frame ([`vifast_pte::TableMemory`]) để cấp bảng trang.
//! Trước KT-340b, ảnh x86_64 chỉ có **một** cây bảng trang tĩnh trong `.bss` — không có chỗ cấp
//! frame thứ hai, tức không có `VSpace` động cho tác vụ ring 3 (D2-238/D2-244). Nếu cấp bằng
//! `static` mới trong `.bss` thì `.data+.bss` lại phình, đúng thứ `KT-340b` phải hạ.
//!
//! Arena là **RAM thật**, do linker đặt trong section NOLOAD `.bootpt` (`bootpt.rs` của ảnh):
//! `VA == PA` nhờ identity map `0..2 MiB` mà `_start` dựng — cùng điều kiện mà `KT-338a` cần cho
//! `DmaPool`/`dc civac` bên aarch64. Nhờ vậy [`TableMemory`] đọc/ghi entry bằng **PA** trực tiếp,
//! không cần alias physmap (x86_64 chưa có physmap — nợ của `KT-340c`).
//!
//! # Hợp đồng
//!
//! - `alloc_frame` **nguyên tử** trên một bộ đếm `&'static AtomicU64` **chia sẻ** giữa mọi
//!   `VSpace` (hai tác vụ cùng cấp frame từ một arena ⇒ không bao giờ trùng frame);
//! - `free_frame` **no-op có chủ đích**: arena boot không thu hồi (cùng quyết định `KT-338a` —
//!   `VSpace::destroy` chưa có đường dùng, và thu hồi nửa vời còn tệ hơn không thu hồi);
//! - `read_entry`/`write_entry` trả `None`/`false` khi `pa` **ngoài** arena hoặc không căn 4 KiB
//!   (fail-closed: một entry hỏng không được biến thành ghi bừa vào RAM khác);
//! - mọi truy cập bộ nhớ đi qua `asm::read_u64`/`asm::write_u64` (`volatile`) — tệp này **không**
//!   có `unsafe` (D2-17: `unsafe` chỉ trong [`crate::asm`]).
//!
//! # Bất biến "frame cấp lại phải zero"
//!
//! [`TableMemory::alloc_frame`] trả frame **không xác định**; `PageTable` gọi
//! [`TableMemory::zero_frame`] trước khi dùng (hợp đồng của `vifast-pte`). Arena đã được `_start`
//! zero một lần lúc boot, nhưng bất biến **không** dựa vào đó — `zero_frame` vẫn xoá thật.

use core::sync::atomic::{AtomicU64, Ordering};

use vifast_pte::TableMemory;

use crate::asm;

/// Cỡ một frame/bảng trang x86_64: 4 KiB.
pub const FRAME_BYTES: u64 = 4096;
/// Số entry một bảng trang: 512.
pub const ENTRIES_PER_FRAME: usize = 512;

/// Nguồn frame của **arena bảng trang động** (RAM thật, identity map, ngoài `.bss`).
///
/// `Copy` để mỗi `VSpace` giữ một handle **chia sẻ** cùng arena/bộ đếm; frame không bao giờ bị
/// cấp trùng vì bộ đếm là `fetch_add`/CAS nguyên tử trên `&'static AtomicU64`.
#[derive(Clone, Copy)]
pub struct BootArena {
    base: u64,
    frames: usize,
    next: &'static AtomicU64,
}

impl BootArena {
    /// Dựng handle trên arena `[base, base + frames*4 KiB)`; `None` nếu `base` lệch 4 KiB hoặc
    /// `frames == 0` (fail-closed thay vì cấp PA rác).
    pub const fn new(base: u64, frames: usize, next: &'static AtomicU64) -> Option<Self> {
        if base & (FRAME_BYTES - 1) != 0 || frames == 0 {
            return None;
        }
        Some(Self { base, frames, next })
    }

    /// PA frame đầu arena (== VA, identity map).
    pub const fn base(&self) -> u64 {
        self.base
    }

    /// Tổng số frame của arena.
    pub const fn frames(&self) -> usize {
        self.frames
    }

    /// PA của frame thứ `frame` — `None` nếu ngoài biên.
    pub fn frame_pa(&self, frame: usize) -> Option<u64> {
        if frame >= self.frames {
            return None;
        }
        Some(self.base + (frame as u64) * FRAME_BYTES)
    }

    /// Số frame đã cấp (bằng chứng `# vspace: frames_used=…`).
    pub fn used(&self) -> usize {
        self.next.load(Ordering::Acquire) as usize
    }

    /// `true` ⇔ `pa` là một frame **của arena** (căn 4 KiB, trong `[base, base+frames*4K)`).
    pub fn contains_frame(&self, pa: u64) -> bool {
        pa & (FRAME_BYTES - 1) == 0
            && pa >= self.base
            && pa < self.base + (self.frames as u64) * FRAME_BYTES
    }
}

impl TableMemory for BootArena {
    /// Cấp frame kế tiếp (CAS trên bộ đếm **chia sẻ**); `None` khi arena cạn.
    ///
    /// `compare_exchange_weak` trong vòng lặp: hai `VSpace` dựng **tuần tự** ở BSP nên tranh chấp
    /// thực tế bằng 0, nhưng hợp đồng "không bao giờ cấp trùng frame" phải đúng cả khi sau này có
    /// hart thứ hai dựng VSpace.
    fn alloc_frame(&mut self) -> Option<u64> {
        let mut n = self.next.load(Ordering::Acquire);
        loop {
            if n as usize >= self.frames {
                return None;
            }
            match self
                .next
                .compare_exchange_weak(n, n + 1, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => return Some(self.base + n * FRAME_BYTES),
                Err(actual) => n = actual,
            }
        }
    }

    /// **No-op có chủ đích** — arena boot không thu hồi (xem tài liệu đầu tệp).
    fn free_frame(&mut self, _pa: u64) {}

    fn read_entry(&self, pa: u64, index: usize) -> Option<u64> {
        if !self.contains_frame(pa) || index >= ENTRIES_PER_FRAME {
            return None;
        }
        Some(asm::read_u64(pa + (index as u64) * 8))
    }

    fn write_entry(&mut self, pa: u64, index: usize, entry: u64) -> bool {
        if !self.contains_frame(pa) || index >= ENTRIES_PER_FRAME {
            return false;
        }
        asm::write_u64(pa + (index as u64) * 8, entry);
        true
    }

    fn zero_frame(&mut self, pa: u64) -> bool {
        if !self.contains_frame(pa) {
            return false;
        }
        let mut i = 0usize;
        while i < ENTRIES_PER_FRAME {
            asm::write_u64(pa + (i as u64) * 8, 0);
            i += 1;
        }
        true
    }
}
