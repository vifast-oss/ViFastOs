// SPDX-License-Identifier: MIT OR Apache-2.0
//! **W^X toàn ảnh** trên aarch64 — KT-277 (K4' bước 3).
//!
//! # Vấn đề
//!
//! Trước KT-277, mọi đường map vùng ảnh dùng **một** khối `Perms::KERNEL_RWX`:
//!
//! ```text
//!   map_identity(text_start, kernel_end, KERNEL_RWX)      // TTBR0 boot
//!   map_high_alias(text_start, kernel_end, KERNEL_RWX)    // TTBR1 boot
//!   map_physmap(text_start, kernel_end, KERNEL_RW)        // physmap
//!   VSpace: map_kernel_high / map_identity_into / map_physmap — cùng khuôn
//! ```
//!
//! Nghĩa là `.text` **ghi được** từ EL1 — W^X không có. Đó là nợ §7.6 của
//! `v2/docs/k4-aarch64-boot.md`.
//!
//! # Cách tách
//!
//! Ranh giới lấy từ **symbol linker** (không hard-code địa chỉ):
//!
//! ```text
//!   [__text_start, __data_start)   RX   .text + .text.vectors + .el0 + .rodata
//!   [__data_start, __kernel_end)   RW   .data + .bss + .smp_stacks (NOLOAD)
//! ```
//!
//! `.rodata` nằm trong vùng **RX** vì nó là dữ liệu **chỉ đọc** (vtable `core::fmt`, bảng
//! chuỗi…): đọc được, không ghi được, và `exec` không hại (không ai nhảy vào giữa `.rodata`).
//! `.data.rel.ro` **không** nằm trong `.rodata` ở ảnh này — linker script gom nó vào `.data`
//! (`*(.data .data.*)`) nên nó **RW**, đúng bản chất "dữ liệu" của nó.
//!
//! # Trang **guard** của stack per-hart: **không map**
//!
//! Arena `.smp_stacks` là `MAX_HARTS` record `guard 4 KiB | stack 64 KiB` (xem [`crate::smp`]).
//! [`ImageLayout::rw_ranges`] trả các khoảng RW **đã trừ** 8 trang guard ⇒ guard **không có
//! mapping** nào (không phải "map RW rồi hy vọng"). Tràn stack đi xuống chạm guard ⇒ fault; KT-277
//! **không** kiểm fault đó (phạm vi: chỉ cần không map), nhưng bất biến "guard không được map"
//! được test host khẳng định trên chính các khoảng trả về.
//!
//! # Một định nghĩa, bốn đường map
//!
//! [`map_low`]/[`map_high`]/[`map_phys`] cho bảng **boot** ([`crate::split_mmu::SplitMmu`]) và
//! [`map_vspace_low`]/[`map_vspace_high`]/[`map_vspace_phys`] cho [`VSpace`] — cả sáu đều đi qua
//! [`ImageLayout::rx`]/[`ImageLayout::rw_ranges`], nên không có đường map thứ hai lệch quyền.
//!
//! # Phạm vi
//!
//! Chỉ **dữ liệu + vòng lặp map**. `TTBR*`/`TCR`/`MAIR`/`TLBI` là việc của
//! [`crate::boot_mmu`]/[`crate::asm`] (D2-17). **Không** KPTI, **không** PAN/UAO (D2-209).

use crate::format::{Aarch64, MemAttr, Perms};
use crate::mmu::{MapError, PageTable, TableMemory};
use crate::split_mmu::{map_high_alias_from, map_physmap, SplitMmu, KERNEL_HIGH_BASE};
use crate::vspace::{Frame, VSpace};

/// Số khoảng RW tối đa: `[data_start, stacks_start)` + `MAX_HARTS` khoảng stack +
/// `[stacks_end, kernel_end)` (khoảng cuối hiện **rỗng** vì `.smp_stacks` là section cuối, nhưng
/// vẫn dành chỗ để bố cục khác không làm tràn mảng).
pub const MAX_RW_RANGES: usize = 2 + crate::smp::MAX_HARTS;

/// Một khoảng `[start, end)` theo byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Range {
    /// Đầu khoảng (byte).
    pub start: u64,
    /// Cuối khoảng (không tính).
    pub end: u64,
}

impl Range {
    /// Dựng khoảng.
    pub const fn new(start: u64, end: u64) -> Self {
        Self { start, end }
    }

    /// Số byte của khoảng (`0` nếu `end <= start`).
    pub const fn bytes(self) -> u64 {
        self.end.saturating_sub(self.start)
    }

    /// `true` ⇔ khoảng rỗng.
    pub const fn is_empty(self) -> bool {
        self.end <= self.start
    }

    /// `true` ⇔ hai đầu đều căn `align` (luỹ thừa 2).
    pub const fn is_aligned(self, align: u64) -> bool {
        align != 0 && self.start & (align - 1) == 0 && self.end & (align - 1) == 0
    }
}

/// Bố cục ảnh theo **symbol linker** — đầu vào duy nhất của mọi đường map W^X.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageLayout {
    /// `__text_start` — đầu `.text` (PA liên kết).
    pub text_start: u64,
    /// `__data_start` — đầu `.data`; cũng là **cuối vùng RX**.
    pub data_start: u64,
    /// `__kernel_end` — cuối vùng ảnh (kể cả `.smp_stacks` NOLOAD).
    pub kernel_end: u64,
    /// `__smp_stacks_start` — đầu arena stack per-hart (NOLOAD).
    pub stacks_start: u64,
    /// `__smp_stacks_end` — cuối arena stack per-hart.
    pub stacks_end: u64,
}

impl ImageLayout {
    /// Dựng bố cục từ năm symbol linker.
    pub const fn new(
        text_start: u64,
        data_start: u64,
        kernel_end: u64,
        stacks_start: u64,
        stacks_end: u64,
    ) -> Self {
        Self {
            text_start,
            data_start,
            kernel_end,
            stacks_start,
            stacks_end,
        }
    }

    /// Vùng **RX**: `[__text_start, __data_start)` — `.text` + `.text.vectors` + `.el0` +
    /// `.rodata`.
    pub const fn rx(&self) -> Range {
        Range::new(self.text_start, self.data_start)
    }

    /// Số byte vùng RX (`text_bytes=` của dòng `# wx:`).
    pub const fn text_bytes(&self) -> u64 {
        self.data_start.saturating_sub(self.text_start)
    }

    /// Số byte vùng RW **trước khi trừ** guard (`data_bytes=` của dòng `# wx:`).
    pub const fn data_bytes(&self) -> u64 {
        self.kernel_end.saturating_sub(self.data_start)
    }

    /// Số trang guard **không map** — `MAX_HARTS` nếu arena stack nằm trong vùng ảnh, ngược lại 0.
    pub const fn guard_pages(&self) -> usize {
        if !self.stacks_inside() {
            return 0;
        }
        let mut pages = 0;
        let mut hart = 0;
        while hart < crate::smp::MAX_HARTS {
            let base = self.stacks_start + hart as u64 * crate::smp::HART_STRIDE;
            if base + crate::smp::HART_STRIDE > self.stacks_end {
                break;
            }
            pages += (crate::smp::HART_GUARD_BYTES / 0x1000) as usize;
            hart += 1;
        }
        pages
    }

    /// Số byte guard **không map**.
    pub const fn guard_bytes(&self) -> u64 {
        self.guard_pages() as u64 * 0x1000
    }

    /// `true` ⇔ arena stack nằm **gọn** trong `[data_start, kernel_end)` và có ít nhất một record.
    const fn stacks_inside(&self) -> bool {
        self.stacks_start >= self.data_start
            && self.stacks_end > self.stacks_start
            && self.stacks_end <= self.kernel_end
    }

    /// Các khoảng **RW** của ảnh, **đã trừ** trang guard của từng hart.
    ///
    /// Trả `(mảng, số phần tử dùng)`. Thứ tự **tăng dần**, không chồng nhau; mọi trang trong
    /// `[data_start, kernel_end)` hoặc thuộc một khoảng trả về, hoặc là trang guard (test host
    /// khẳng định cả hai chiều).
    pub const fn rw_ranges(&self) -> ([Range; MAX_RW_RANGES], usize) {
        let mut out = [Range::new(0, 0); MAX_RW_RANGES];
        let mut n = 0;
        if self.kernel_end <= self.data_start {
            return (out, 0);
        }
        // Không có arena stack trong vùng ảnh (bố cục khác) ⇒ một khoảng duy nhất.
        if !self.stacks_inside() {
            out[0] = Range::new(self.data_start, self.kernel_end);
            return (out, 1);
        }
        // `.data` + `.bss` (trước arena stack).
        if self.stacks_start > self.data_start {
            out[n] = Range::new(self.data_start, self.stacks_start);
            n += 1;
        }
        // Từng hart: bỏ trang guard ở **đầu** record, map phần stack.
        let mut hart = 0;
        while hart < crate::smp::MAX_HARTS {
            let base = self.stacks_start + hart as u64 * crate::smp::HART_STRIDE;
            if base + crate::smp::HART_STRIDE > self.stacks_end {
                break;
            }
            out[n] = Range::new(
                base + crate::smp::HART_GUARD_BYTES,
                base + crate::smp::HART_STRIDE,
            );
            n += 1;
            hart += 1;
        }
        // Phần sau arena stack (hiện rỗng vì `.smp_stacks` là section cuối).
        if self.kernel_end > self.stacks_end {
            out[n] = Range::new(self.stacks_end, self.kernel_end);
            n += 1;
        }
        (out, n)
    }

    /// `true` ⇔ mọi khoảng RX/RW đều căn 4 KiB (điều kiện của mọi đường map trang 4 KiB).
    pub const fn is_page_aligned(&self) -> bool {
        if !self.rx().is_aligned(0x1000) {
            return false;
        }
        let (ranges, n) = self.rw_ranges();
        let mut i = 0;
        while i < n {
            if !ranges[i].is_aligned(0x1000) {
                return false;
            }
            i += 1;
        }
        true
    }
}

/// Map vùng ảnh vào bảng **nửa thấp** (identity) của bảng boot: RX + các khoảng RW.
pub fn map_low<M: TableMemory>(
    table: &mut PageTable<Aarch64, M>,
    layout: &ImageLayout,
) -> Result<usize, MapError> {
    let rx = layout.rx();
    let mut pages =
        crate::boot_mmu::map_identity(table, rx.start, rx.end, Perms::KERNEL_RX, MemAttr::Normal)?;
    pages += map_rw_ranges(
        &mut |start, end| {
            crate::boot_mmu::map_identity(table, start, end, Perms::KERNEL_RW, MemAttr::Normal)
        },
        layout,
    )?;
    Ok(pages)
}

/// Map **alias nửa cao** của vùng ảnh vào bảng nửa cao của bảng boot: RX + các khoảng RW.
///
/// Gốc alias là `layout.text_start` cho **mọi** khoảng (không phải đầu từng khoảng) — nếu không,
/// alias của `.data` sẽ lệch đúng bằng kích thước vùng RX.
pub fn map_high<M: TableMemory>(
    mmu: &mut SplitMmu<M>,
    layout: &ImageLayout,
) -> Result<usize, MapError> {
    let rx = layout.rx();
    let mut pages = mmu.map_high_alias(
        rx.start,
        rx.end,
        KERNEL_HIGH_BASE,
        Perms::KERNEL_RX,
        MemAttr::Normal,
    )?;
    pages += map_rw_ranges(
        &mut |start, end| {
            map_high_alias_from(
                mmu.high_mut(),
                start,
                end,
                layout.text_start,
                KERNEL_HIGH_BASE,
                Perms::KERNEL_RW,
                MemAttr::Normal,
            )
        },
        layout,
    )?;
    Ok(pages)
}

/// Map **physmap** của vùng ảnh vào bảng nửa cao của bảng boot: RX + các khoảng RW.
pub fn map_phys<M: TableMemory>(
    mmu: &mut SplitMmu<M>,
    layout: &ImageLayout,
) -> Result<usize, MapError> {
    let rx = layout.rx();
    let mut pages = mmu.map_physmap(rx.start, rx.end, Perms::KERNEL_RX, MemAttr::Normal)?;
    pages += map_rw_ranges(
        &mut |start, end| {
            map_physmap(
                mmu.high_mut(),
                start,
                end,
                Perms::KERNEL_RW,
                MemAttr::Normal,
            )
        },
        layout,
    )?;
    Ok(pages)
}

/// Map vùng ảnh **identity** vào bảng nửa thấp của [`VSpace`]: RX + các khoảng RW.
pub fn map_vspace_low<M: TableMemory>(
    vs: &mut VSpace<M>,
    layout: &ImageLayout,
) -> Result<usize, MapError> {
    let rx = layout.rx();
    let mut pages = map_vspace_identity(vs, rx.start, rx.end, Perms::KERNEL_RX)?;
    pages += map_rw_ranges(
        &mut |start, end| map_vspace_identity(vs, start, end, Perms::KERNEL_RW),
        layout,
    )?;
    Ok(pages)
}

/// Map **alias nửa cao** của vùng ảnh vào bảng nửa cao của [`VSpace`]: RX + các khoảng RW.
pub fn map_vspace_high<M: TableMemory>(
    vs: &mut VSpace<M>,
    layout: &ImageLayout,
) -> Result<usize, MapError> {
    let rx = layout.rx();
    let mut pages = vs.map_kernel_high(
        rx.start,
        rx.end,
        KERNEL_HIGH_BASE,
        Perms::KERNEL_RX,
        MemAttr::Normal,
    )?;
    pages += map_rw_ranges(
        &mut |start, end| {
            map_high_alias_from(
                vs.mmu_mut().high_mut(),
                start,
                end,
                layout.text_start,
                KERNEL_HIGH_BASE,
                Perms::KERNEL_RW,
                MemAttr::Normal,
            )
        },
        layout,
    )?;
    Ok(pages)
}

/// Map **physmap** của vùng ảnh vào bảng nửa cao của [`VSpace`]: RX + các khoảng RW.
pub fn map_vspace_phys<M: TableMemory>(
    vs: &mut VSpace<M>,
    layout: &ImageLayout,
) -> Result<usize, MapError> {
    let rx = layout.rx();
    let mut pages = vs.map_physmap(rx.start, rx.end, Perms::KERNEL_RX, MemAttr::Normal)?;
    pages += map_rw_ranges(
        &mut |start, end| vs.map_physmap(start, end, Perms::KERNEL_RW, MemAttr::Normal),
        layout,
    )?;
    Ok(pages)
}

/// Lặp các khoảng RW của `layout` và cộng dồn số trang đã map.
fn map_rw_ranges<F>(mut map: F, layout: &ImageLayout) -> Result<usize, MapError>
where
    F: FnMut(u64, u64) -> Result<usize, MapError>,
{
    let (ranges, count) = layout.rw_ranges();
    let mut pages = 0;
    let mut index = 0;
    while index < count {
        let range = ranges[index];
        if !range.is_empty() {
            pages += map(range.start, range.end)?;
        }
        index += 1;
    }
    Ok(pages)
}

/// Map identity `[start, end)` vào bảng nửa thấp của `vs` bằng trang 4 KiB.
fn map_vspace_identity<M: TableMemory>(
    vs: &mut VSpace<M>,
    start: u64,
    end: u64,
    perms: Perms,
) -> Result<usize, MapError> {
    if end <= start {
        return Ok(0);
    }
    if start & 0xFFF != 0 || end & 0xFFF != 0 {
        return Err(MapError::Unaligned);
    }
    let mut pages = 0;
    let mut pa = start;
    while pa < end {
        let frame = Frame::new(pa).ok_or(MapError::Unaligned)?;
        vs.map_obj_with(pa, frame, perms, MemAttr::Normal)?;
        pages += 1;
        pa += 0x1000;
    }
    Ok(pages)
}
