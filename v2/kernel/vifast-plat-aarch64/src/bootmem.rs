// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Arena bảng trang boot** — nguồn frame cho [`PageTable`] trên đường boot aarch64 (KT-249).
//!
//! # Vì sao **không** có `unsafe`
//!
//! Cách thông thường là `static mut [u8; N]` (hoặc `UnsafeCell` + `unsafe impl Sync`) rồi truy cập
//! bằng con trỏ thô — cả hai đều cần `unsafe`. Tệp này **không** làm vậy: arena là một mảng
//! [`AtomicU64`] (`#[repr(align(4096))]` để căn trang), nên
//!
//! - đọc/ghi entry đi qua `load`/`store` (`Relaxed` — boot một hart, không có tranh chấp);
//! - bộ cấp phát là hai biến đếm **trong struct** (không `static mut`);
//! - không có con trỏ thô nào ⇒ crate vẫn `#![deny(unsafe_code)]` ngoài [`crate::asm`] (D2-17).
//!
//! Đây không phải "né luật": `TableMemory` là hợp đồng **dữ liệu** (đọc/ghi 512 entry 8 B của
//! một frame), và một mảng `AtomicU64` căn trang **là** bộ nhớ đó — không cần `unsafe` để mô tả.
//!
//! # Vì sao arena tĩnh (không lấy từ allocator)
//!
//! Bước 3 chưa có allocator, và bảng trang phải sống **trước** khi MMU bật (không thể cấp phát
//! bằng thứ cần MMU). Arena tĩnh 64 frame = 256 KiB nằm trong `.bss`; `_start` đã zero `.bss`
//! nên mọi frame bắt đầu bằng 0 (đúng yêu cầu "frame cấp lần đầu phải sạch").
//!
//! # `free_frame` là no-op
//!
//! Arena boot **không** thu hồi frame: bước 3 chỉ map (không unmap), và `PageTable::destroy`
//! không được gọi trên bảng của kernel. Ghi rõ ở đây để người đọc không tưởng đó là rò rỉ ngầm —
//! `frames_used()` cho biết đã tiêu bao nhiêu.

use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use crate::mmu::TableMemory;

/// Số frame 4 KiB của arena bảng trang boot: **32** (128 KiB).
///
/// Đo được trên ảnh KT-249: identity map kernel + UART + GIC + trang probe dùng **8** frame
/// (gốc mức 0, mức 1, hai bảng mức 2, ba bảng mức 3, một bảng mức 3 cho trang probe) ⇒ 32 là
/// **4×** headroom cho bước sau (B3.4 physmap/nửa cao). Vượt thì `alloc_frame` trả `None` và boot
/// **PANIC** (fail-loud), không ghi ra ngoài arena.
///
/// **Vì sao không phải 64**: bất biến I7 của thiết kế (`kt242-k4-buoc3-design.md` §3) áp trần
/// K0.5 `.data + .bss ≤ 256 KiB` cho **mọi** bước. Arena 64 frame (256 KiB) + trang probe (4 KiB)
/// đẩy `.bss` lên **268 KiB** ⇒ vượt trần. Đo lại sau khi giảm: xem journal KT-249 §6.
pub const BOOT_ARENA_FRAMES: usize = 32;

/// Số entry một frame bảng trang (512 × 8 B = 4 KiB).
pub const ENTRIES_PER_FRAME: usize = 512;

/// Số `u64` của arena.
pub const BOOT_ARENA_WORDS: usize = BOOT_ARENA_FRAMES * ENTRIES_PER_FRAME;

/// Arena **tĩnh** của ảnh: `#[repr(align(4096))]` để `TTBR0_EL1.BADDR` hợp lệ (đòi căn 4 KiB).
#[repr(align(4096))]
struct Arena([AtomicU64; BOOT_ARENA_WORDS]);

/// Arena duy nhất của ảnh — `static` (không `mut`), ghi qua `AtomicU64`.
static ARENA: Arena = Arena([const { AtomicU64::new(0) }; BOOT_ARENA_WORDS]);

/// Nguồn frame + bộ nhớ bảng trang trên một mảng `AtomicU64` căn trang.
///
/// `base` là **PA** của `words[0]`; trên đường boot PA == VA (identity) nên `base` cũng là địa
/// chỉ ảo của arena. `TableMemory` chỉ dùng `base` để quy đổi `pa → index`, **không** truy cập
/// bộ nhớ qua con trỏ thô.
pub struct BootMemory {
    words: &'static [AtomicU64],
    base: u64,
    next: usize,
    frames: usize,
}

impl BootMemory {
    /// Dựng nguồn frame trên `words` (phải dài bội số của [`ENTRIES_PER_FRAME`]) ở PA `base`.
    ///
    /// `base` phải căn 4 KiB — arena tĩnh đã căn bằng `#[repr(align(4096))]`; test host dùng
    /// `FRAME_BASE` căn sẵn.
    pub fn new(words: &'static [AtomicU64], base: u64) -> Self {
        let frames = words.len() / ENTRIES_PER_FRAME;
        Self {
            words,
            base,
            next: 0,
            frames,
        }
    }

    /// PA của frame đầu tiên (địa chỉ arena) — để log bằng chứng.
    pub const fn base_hint(&self) -> u64 {
        self.base
    }

    /// Số frame **đã cấp** (bằng chứng "tiêu bao nhiêu frame" cho log boot).
    pub fn frames_used(&self) -> usize {
        self.next
    }

    /// Tổng số frame của arena.
    pub fn frames_total(&self) -> usize {
        self.frames
    }

    /// PA của frame thứ `frame` (không kiểm biên — dùng nội bộ sau khi đã kiểm).
    const fn frame_pa(&self, frame: usize) -> u64 {
        self.base + (frame as u64) * 4096
    }

    /// `pa` → chỉ số từ đầu arena; `None` nếu ngoài arena hoặc không căn 4 KiB.
    fn word_index(&self, pa: u64, index: usize) -> Option<usize> {
        if index >= ENTRIES_PER_FRAME || pa & 0xFFF != 0 || pa < self.base {
            return None;
        }
        let offset = pa - self.base;
        let frame = (offset / 4096) as usize;
        if frame >= self.frames {
            return None;
        }
        Some(frame * ENTRIES_PER_FRAME + index)
    }
}

impl TableMemory for BootMemory {
    fn alloc_frame(&mut self) -> Option<u64> {
        if self.next >= self.frames {
            return None;
        }
        let pa = self.frame_pa(self.next);
        self.next += 1;
        Some(pa)
    }

    /// No-op **có chủ đích**: arena boot không thu hồi (xem tài liệu đầu tệp).
    fn free_frame(&mut self, _pa: u64) {}

    fn read_entry(&self, pa: u64, index: usize) -> Option<u64> {
        self.word_index(pa, index)
            .map(|i| self.words[i].load(Ordering::Relaxed))
    }

    fn write_entry(&mut self, pa: u64, index: usize, entry: u64) -> bool {
        match self.word_index(pa, index) {
            Some(i) => {
                self.words[i].store(entry, Ordering::Relaxed);
                true
            }
            None => false,
        }
    }

    fn zero_frame(&mut self, pa: u64) -> bool {
        for index in 0..ENTRIES_PER_FRAME {
            if !self.write_entry(pa, index, 0) {
                return false;
            }
        }
        true
    }
}

/// Nguồn frame của **ảnh** — arena tĩnh [`ARENA`], PA = địa chỉ ảo (identity trước khi bật MMU).
///
/// Gọi **một lần** trong `kmain`; gọi lần hai sẽ cấp lại từ frame 0 (arena dùng chung) nên ảnh
/// chỉ gọi một lần.
pub fn boot_memory() -> BootMemory {
    let base = core::ptr::addr_of!(ARENA.0) as u64;
    BootMemory::new(&ARENA.0, base)
}

// =====================================================================================
// KT-253 (B3.4) — arena **chia sẻ** cho hai bảng trang (nửa thấp + nửa cao)
// =====================================================================================

/// Con trỏ bump **chia sẻ** giữa hai bảng trang của ảnh (một arena, một bộ đếm).
///
/// Vì sao cần: [`crate::split_mmu::SplitMmu`] giữ **hai** `PageTable` (một cho `TTBR0_EL1`, một
/// cho `TTBR1_EL1`). Nếu mỗi bảng có bộ đếm riêng thì cả hai bắt đầu từ frame 0 và **ghi đè**
/// lên nhau — bảng trang hỏng ngay sau frame đầu. Một bộ đếm `static` chia sẻ giải quyết đúng
/// vấn đề, không cần `unsafe`, không cần allocator.
static SHARED_NEXT: AtomicUsize = AtomicUsize::new(0);

/// Nguồn frame **chia sẻ**: hai handle cùng trỏ một arena + một con trỏ bump.
///
/// Khác [`BootMemory`] ở đúng một điểm: `next` là `&'static AtomicUsize` (chia sẻ) thay vì
/// `usize` (riêng). Mọi thứ còn lại — `TableMemory`, `free_frame` no-op, kiểm biên — giữ nguyên
/// ngữ nghĩa.
pub struct SharedBootMemory {
    words: &'static [AtomicU64],
    base: u64,
    frames: usize,
    next: &'static AtomicUsize,
}

impl SharedBootMemory {
    /// Dựng handle trên `words` (bội số của [`ENTRIES_PER_FRAME`]) ở PA `base`, dùng chung
    /// `next` với handle còn lại.
    pub fn new(words: &'static [AtomicU64], base: u64, next: &'static AtomicUsize) -> Self {
        Self {
            words,
            base,
            frames: words.len() / ENTRIES_PER_FRAME,
            next,
        }
    }

    /// PA của frame đầu tiên (địa chỉ arena) — để log bằng chứng.
    pub const fn base_hint(&self) -> u64 {
        self.base
    }

    /// Số frame **đã cấp** trên toàn arena (cả hai bảng) — bằng chứng "tiêu bao nhiêu frame".
    pub fn frames_used(&self) -> usize {
        self.next.load(Ordering::Relaxed)
    }

    /// Tổng số frame của arena.
    pub const fn frames_total(&self) -> usize {
        self.frames
    }

    /// `pa` → chỉ số từ đầu arena; `None` nếu ngoài arena hoặc không căn 4 KiB.
    fn word_index(&self, pa: u64, index: usize) -> Option<usize> {
        if index >= ENTRIES_PER_FRAME || pa & 0xFFF != 0 || pa < self.base {
            return None;
        }
        let frame = ((pa - self.base) / 4096) as usize;
        if frame >= self.frames {
            return None;
        }
        Some(frame * ENTRIES_PER_FRAME + index)
    }
}

impl TableMemory for SharedBootMemory {
    fn alloc_frame(&mut self) -> Option<u64> {
        // Boot một hart ⇒ `load`/`store` đủ; không có tranh chấp nào để `fetch_add` phải lo.
        let n = self.next.load(Ordering::Relaxed);
        if n >= self.frames {
            return None;
        }
        self.next.store(n + 1, Ordering::Relaxed);
        Some(self.base + (n as u64) * 4096)
    }

    /// No-op **có chủ đích** như [`BootMemory`]: arena boot không thu hồi frame.
    fn free_frame(&mut self, _pa: u64) {}

    fn read_entry(&self, pa: u64, index: usize) -> Option<u64> {
        self.word_index(pa, index)
            .map(|i| self.words[i].load(Ordering::Relaxed))
    }

    fn write_entry(&mut self, pa: u64, index: usize, entry: u64) -> bool {
        match self.word_index(pa, index) {
            Some(i) => {
                self.words[i].store(entry, Ordering::Relaxed);
                true
            }
            None => false,
        }
    }

    fn zero_frame(&mut self, pa: u64) -> bool {
        for index in 0..ENTRIES_PER_FRAME {
            if !self.write_entry(pa, index, 0) {
                return false;
            }
        }
        true
    }
}

/// Hai handle **chia sẻ** arena của ảnh: `(nửa thấp, nửa cao)` — KT-253.
///
/// Gọi **một lần** trong `kmain` (bộ đếm `SHARED_NEXT` là `static`, gọi lần hai sẽ cấp tiếp từ
/// chỗ cũ chứ không reset — vẫn an toàn, chỉ tốn frame).
pub fn shared_boot_memory() -> (SharedBootMemory, SharedBootMemory) {
    shared_boot_memory_at(core::ptr::addr_of!(ARENA.0) as u64)
}

/// Như [`shared_boot_memory`] nhưng **chỉ định PA** của arena.
///
/// **Vì sao cần**: sau khi nhảy sang nửa cao (KT-253), `addr_of!(ARENA.0)` là **PC-relative**
/// nên trả **alias nửa cao** của arena, không phải PA. `TableMemory` phải dùng **PA** trong
/// descriptor (`BADDR`), nên caller đọc PA **trước** khi nhảy rồi truyền vào đây; còn `words`
/// vẫn là `&ARENA.0` (alias nửa cao — cùng bộ nhớ vật lý, đã map).
pub fn shared_boot_memory_at(base: u64) -> (SharedBootMemory, SharedBootMemory) {
    let low = SharedBootMemory::new(&ARENA.0, base, &SHARED_NEXT);
    let high = SharedBootMemory::new(&ARENA.0, base, &SHARED_NEXT);
    (low, high)
}
