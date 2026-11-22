// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Arena bảng trang boot** — nguồn frame cho [`PageTable`] trên đường boot aarch64 (KT-249).
//!
//! # KT-338a — arena **ngoài ảnh**, không còn `static` trong `.bss`
//!
//! Trước KT-338a arena là `static [AtomicU64; 44 × 512]` trong `.bss` (180.224 B): ảnh aarch64
//! dùng **233.704 / 262.144 B** `.data+.bss` (89,2 % trần I7) và **không còn chỗ** cho
//! `vifast-core::Machine` (KT-338b). Từ KT-338a:
//!
//! - arena nằm ở **RAM thật ngay sau ảnh** (`__kernel_end`, căn 4 KiB) — vùng
//!   [`boot_ram_bytes`] byte **ngoài** mọi section của ELF, map **identity** (`VA == PA`) nên
//!   `TableMemory` vẫn đọc/ghi entry bằng atomics như trước;
//! - vị trí do **linker** quyết định lúc chạy (`__kernel_end`), không phải hằng viết tay;
//! - bốn **trang DMA động** của `virtio-blk`/`net`/`gpu`/`input` cấp từ chính arena
//!   ([`alloc_boot_frame`]) — không còn `static` 4 × 4 KiB trong `.bss`.
//!
//! Nhờ vậy `.data+.bss` của ảnh aarch64 hạ xuống **dưới 64 KiB** (đo ở
//! `v2/test/boot/budget-aarch64.sh`, journal `kt338a-bss.md`), trần I7 **không** bị nới.
//!
//! # Vì sao **không** có `unsafe`
//!
//! Cách thông thường là `static mut [u8; N]` (hoặc `UnsafeCell` + `unsafe impl Sync`) rồi truy cập
//! bằng con trỏ thô — cả hai đều cần `unsafe`. Tệp này **không** làm vậy: arena là một mảng
//! [`AtomicU64`] do [`crate::asm::arena_words`] dựng trên **VA đã map** của vùng RAM boot, nên
//!
//! - đọc/ghi entry đi qua `load`/`store` (`Relaxed` — boot một hart, không có tranh chấp);
//! - bộ cấp phát là bộ đếm `static` [`SharedBootMemory`] (không `static mut`);
//! - không có con trỏ thô nào trong tệp này ⇒ crate vẫn `#![deny(unsafe_code)]` ngoài
//!   [`crate::asm`] (D2-17).
//!
//! Đây không phải "né luật": `TableMemory` là hợp đồng **dữ liệu** (đọc/ghi 512 entry 8 B của
//! một frame), và một mảng `AtomicU64` căn trang **là** bộ nhớ đó — không cần `unsafe` để mô tả.
//!
//! # "Frame cấp lần đầu phải sạch"
//!
//! `.bss` được `_start` zero, nhưng vùng RAM sau ảnh thì **không** ai zero hộ. `PageTable::new`
//! và mọi lần cấp bảng mới đều gọi [`TableMemory::zero_frame`] trước khi dùng, nên bảng trang
//! vẫn sạch tuyệt đối; riêng frame DMA (cấp thủ công qua [`alloc_boot_frame`]) được
//! [`crate::asm`] zero **tại chỗ** trước khi giao cho driver.
//!
//! # `free_frame` là no-op
//!
//! Arena boot **không** thu hồi frame: bước 3 chỉ map (không unmap), và `PageTable::destroy`
//! không được gọi trên bảng của kernel. Ghi rõ ở đây để người đọc không tưởng đó là rò rỉ ngầm —
//! `frames_used()` cho biết đã tiêu bao nhiêu.

use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use crate::mmu::TableMemory;

/// Số frame 4 KiB của arena bảng trang boot: **64** (256 KiB).
///
/// Đo được trên ảnh KT-249: identity map kernel + UART + GIC + trang probe dùng **8** frame
/// (gốc mức 0, mức 1, hai bảng mức 2, ba bảng mức 3, một bảng mức 3 cho trang probe) ⇒ 32 là
/// **4×** headroom cho bước sau (B3.4 physmap/nửa cao). Vượt thì `alloc_frame` trả `None` và boot
/// **PANIC** (fail-loud), không ghi ra ngoài arena.
///
/// **KT-273 nâng 32 → 44**: ảnh KT-269 đã dùng **hết 32** frame, PCI ECAM + cửa sổ BAR cần thêm
/// **10** frame; đo trên ảnh KT-273: `frames_used=37` ⇒ 44 frame còn **7** dự phòng.
///
/// **KT-338a nâng 44 → 64** — lý do: con số 44 bị **chặn bởi trần `.data+.bss`**, không phải bởi
/// nhu cầu thật (chính tài liệu cũ ghi "64 frame vẫn không dùng được vì `.bss` sẽ vượt trần").
/// Arena ra khỏi `.bss` ⇒ bỏ được chặn ấy. 64 frame = 44 (nhu cầu đã đo) + **4 trang DMA động**
/// (blk/net/gpu/input, KT-338a) + **16 frame** dự phòng cho các bảng trang của KT-338b (nối
/// `vifast-core` + IPC EL0) — vẫn là RAM **ngoài** ảnh nên **không** đụng trần I7.
pub const BOOT_ARENA_FRAMES: usize = 64;

/// Số entry một frame bảng trang (512 × 8 B = 4 KiB).
pub const ENTRIES_PER_FRAME: usize = 512;

/// Số `u64` của arena.
pub const BOOT_ARENA_WORDS: usize = BOOT_ARENA_FRAMES * ENTRIES_PER_FRAME;

/// Số byte arena (bội của 4 KiB).
pub const BOOT_RAM_BYTES: usize = BOOT_ARENA_WORDS * 8;

/// Số **trang DMA động** cấp từ arena: một trang cho mỗi driver virtio một-hart
/// (`virtio-blk`, `virtio-net`, `virtio-gpu`, `virtio-input`).
///
/// Trước KT-338a bốn trang này là `static` 4 × 4 KiB trong `.bss` (`asm::DmaPage*`).
pub const BOOT_DMA_FRAMES: usize = 4;

/// PA 4 KiB của arena — công bố **một lần** ở `mmu_bringup` bằng [`publish_arena`].
///
/// `0` nghĩa là "chưa công bố"; [`alloc_boot_frame`] từ chối cấp trong trạng thái đó thay vì trả
/// địa chỉ rác.
static ARENA_BASE: AtomicU64 = AtomicU64::new(0);

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

/// Công bố **PA** của arena (KT-338a) — gọi **một lần** trong `mmu_bringup`, trước
/// [`shared_boot_memory`] đầu tiên.
///
/// `base` phải căn 4 KiB và là đầu vùng [`BOOT_RAM_BYTES`] byte RAM **ngoài ảnh** mà `mmu_bringup`
/// sẽ map **identity** (`VA == PA`) vào cả bảng boot lẫn `VSpace`. Arena **không** còn là `static`
/// trong `.bss` — vị trí do linker (`__kernel_end`) quyết định lúc chạy.
pub fn publish_arena(base: u64) {
    ARENA_BASE.store(base, Ordering::Release);
}

/// PA arena đã công bố; `0` ⇔ chưa công bố.
pub fn arena_base() -> u64 {
    ARENA_BASE.load(Ordering::Acquire)
}

/// Số byte vùng RAM boot (arena + mọi thứ cấp từ nó) — bội của 4 KiB.
pub const fn boot_ram_bytes() -> usize {
    BOOT_RAM_BYTES
}

/// Cấp **một** frame 4 KiB từ arena cho **vùng DMA động** (KT-338a) — `None` khi chưa công bố
/// arena hoặc đã cạn.
///
/// Vì sao dùng `fetch_add`: đây là đường cấp phát **duy nhất** có thể chạy sau khi các hart phụ
/// đã online (driver virtio chạy ở `el0_done`), nên bộ đếm phải nguyên tử; `SharedBootMemory`
/// vẫn dùng `load`/`store` vì nó chỉ chạy trên hart 0 lúc dựng bảng trang. Hai đường **nối tiếp**
/// nhau trên cùng bộ đếm nên không frame nào bị cấp hai lần.
///
/// Caller **phải** zero frame trước khi dùng (`asm::dma_page_*` làm việc đó) — vùng RAM sau ảnh
/// không được `_start` zero hộ.
pub fn alloc_boot_frame() -> Option<u64> {
    let base = arena_base();
    if base == 0 {
        return None;
    }
    let n = SHARED_NEXT.fetch_add(1, Ordering::Relaxed);
    if n >= BOOT_ARENA_FRAMES {
        return None;
    }
    Some(base + (n as u64) * 0x1000)
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
/// **KT-338a**: arena nằm ngoài ảnh, PA do [`publish_arena`] công bố; `words` là lát
/// [`crate::asm::arena_words`] trên **VA identity** của arena (`VA == PA`). Nhờ identity, cùng
/// một lát ấy dùng được **trước** khi bật MMU (PA), **sau** khi nhảy nửa cao (TTBR0 vẫn map VA
/// thấp) và **sau** `VSpace::activate` (`VSpace` map identity vùng RAM boot) — không cần alias
/// nửa cao riêng cho arena.
///
/// Gọi **một lần** trong `kmain` (bộ đếm `SHARED_NEXT` là `static`, gọi lần hai sẽ cấp tiếp từ
/// chỗ cũ chứ không reset — vẫn an toàn, chỉ tốn frame).
pub fn shared_boot_memory() -> (SharedBootMemory, SharedBootMemory) {
    let base = arena_base();
    shared_boot_memory_at(crate::asm::arena_words(base), base)
}

/// Như [`shared_boot_memory`] nhưng **chỉ định** lát `words` và PA arena.
///
/// Tách ra để test host dựng arena trên `Vec`/`Box` (không cần RAM thật) và để ảnh truyền **cùng**
/// PA đã công bố. `words.len()` phải là bội của [`ENTRIES_PER_FRAME`].
pub fn shared_boot_memory_at(
    words: &'static [AtomicU64],
    base: u64,
) -> (SharedBootMemory, SharedBootMemory) {
    let low = SharedBootMemory::new(words, base, &SHARED_NEXT);
    let high = SharedBootMemory::new(words, base, &SHARED_NEXT);
    (low, high)
}
