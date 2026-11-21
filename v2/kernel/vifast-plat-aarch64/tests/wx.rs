// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho **W^X toàn ảnh** — KT-277 (K4' bước 3).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64 --test wx
//! ```
//!
//! # Bố cục dùng trong test là bố cục **thật** của một ảnh đã đo
//!
//! Năm hằng dưới đây lấy từ `llvm-objdump -h` + `-t` của ảnh `vifast-kbin-aarch64` (KT-273):
//!
//! ```text
//!   .text 0x40080000 .. 0x4008bf10   .text.vectors 0x4008c000   .el0 0x4008d000
//!   .rodata 0x4008e000 .. 0x40092000  ⇒ __data_start = 0x40092000
//!   .data 0x40092000   .bss 0x40093000 .. 0x400c9000
//!   .smp_stacks 0x400c9000 .. 0x40151000 (NOLOAD, 8 record × 0x1_1000)
//! ```
//!
//! Các con số **sẽ đổi** khi ảnh đổi (KT-277 làm `.text` lớn thêm); test chỉ cần **một** bố cục
//! thật để kiểm logic, nên nó giữ bố cục KT-273 và **không** đọc ELF lúc chạy. Nhờ vậy test khẳng
//! định **đúng** ranh giới mà ảnh dùng, không phải một bố cục bịa.
//!
//! # Điều test này chứng minh
//!
//! 1. vùng RX là `[__text_start, __data_start)` — **gồm** `.text.vectors`, `.el0`, `.rodata`;
//! 2. các khoảng RW **đã trừ** 8 trang guard, phủ **đúng** phần còn lại (không hở, không chồng);
//! 3. `map_low`/`map_high`/`map_phys`/`map_vspace_*` cho **đúng** quyền: text RX (không ghi),
//!    data RW (không exec), guard **không map**;
//! 4. alias nửa cao dùng **một** gốc (`__text_start`) cho mọi khoảng — nếu mỗi khoảng lấy đầu
//!    của chính nó, alias của `.data` lệch đúng bằng kích thước vùng RX.

use core::cell::Cell;
use core::sync::atomic::{AtomicU64, Ordering};
use std::rc::Rc;

use vifast_plat_aarch64::format::{Aarch64, MemAttr, Perms};
use vifast_plat_aarch64::mmu::{MapError, PageTable, TableMemory};
use vifast_plat_aarch64::split_mmu::{high_alias_va, physmap_va, SplitMmu, KERNEL_HIGH_BASE};
use vifast_plat_aarch64::vspace::VSpace;
use vifast_plat_aarch64::wx::{
    map_high, map_low, map_phys, map_vspace_high, map_vspace_low, map_vspace_phys, ImageLayout,
    Range,
};

// ---------------------------------------------------------------- bố cục ảnh thật (KT-273)

/// `__text_start` — đầu `.text`.
const TEXT_START: u64 = 0x4008_0000;
/// `__data_start` — đầu `.data`, cũng là cuối vùng RX.
const DATA_START: u64 = 0x4009_2000;
/// `__smp_stacks_start` — đầu arena stack per-hart (NOLOAD).
const STACKS_START: u64 = 0x400C_9000;
/// `__smp_stacks_end` = `__kernel_end` — cuối vùng ảnh.
const KERNEL_END: u64 = 0x4015_1000;

/// Bố cục ảnh thật của KT-273.
const LAYOUT: ImageLayout =
    ImageLayout::new(TEXT_START, DATA_START, KERNEL_END, STACKS_START, KERNEL_END);

/// PA gốc của nguồn frame test (4 KiB aligned).
const FRAME_BASE: u64 = 0x5000_0000;
/// Số frame của nguồn test — đủ cho bảng 4 mức của ~1 MiB vùng ảnh + physmap.
const FRAMES: usize = 128;

// ---------------------------------------------------------------- nguồn frame host chia sẻ

/// Nguồn frame + bộ nhớ bảng trang cho host, **chia sẻ** giữa hai `PageTable` (như
/// `bootmem::SharedBootMemory` của ảnh).
struct SharedTestMemory {
    words: Rc<Vec<AtomicU64>>,
    base: u64,
    frames: usize,
    next: Rc<Cell<usize>>,
}

impl SharedTestMemory {
    fn index(&self, pa: u64, index: usize) -> Option<usize> {
        if pa < self.base || pa & 0xFFF != 0 || index >= 512 {
            return None;
        }
        let frame = ((pa - self.base) / 4096) as usize;
        if frame >= self.frames {
            return None;
        }
        Some(frame * 512 + index)
    }
}

impl TableMemory for SharedTestMemory {
    fn alloc_frame(&mut self) -> Option<u64> {
        let n = self.next.get();
        if n >= self.frames {
            return None;
        }
        self.next.set(n + 1);
        Some(self.base + (n as u64) * 4096)
    }

    fn free_frame(&mut self, _pa: u64) {}

    fn read_entry(&self, pa: u64, index: usize) -> Option<u64> {
        self.index(pa, index)
            .map(|i| self.words[i].load(Ordering::Relaxed))
    }

    fn write_entry(&mut self, pa: u64, index: usize, entry: u64) -> bool {
        match self.index(pa, index) {
            Some(i) => {
                self.words[i].store(entry, Ordering::Relaxed);
                true
            }
            None => false,
        }
    }

    fn zero_frame(&mut self, pa: u64) -> bool {
        for index in 0..512 {
            if !self.write_entry(pa, index, 0) {
                return false;
            }
        }
        true
    }
}

/// Hai nguồn frame **chia sẻ** một arena.
fn pair() -> (SharedTestMemory, SharedTestMemory) {
    let words = Rc::new((0..FRAMES * 512).map(|_| AtomicU64::new(0)).collect());
    let next = Rc::new(Cell::new(0usize));
    let low = SharedTestMemory {
        words: Rc::clone(&words),
        base: FRAME_BASE,
        frames: FRAMES,
        next: Rc::clone(&next),
    };
    let high = SharedTestMemory {
        words,
        base: FRAME_BASE,
        frames: FRAMES,
        next,
    };
    (low, high)
}

/// `SplitMmu` rỗng trên hai nguồn frame chia sẻ.
fn empty_mmu() -> SplitMmu<SharedTestMemory> {
    let (low, high) = pair();
    SplitMmu::new(low, high).expect("dựng SplitMmu")
}

/// `VSpace` rỗng trên hai nguồn frame chia sẻ.
fn empty_vspace() -> VSpace<SharedTestMemory> {
    let (low, high) = pair();
    VSpace::create(1, low, high).expect("dựng VSpace")
}

/// `PageTable` nửa thấp rỗng (cho `map_low`).
fn empty_table() -> PageTable<Aarch64, SharedTestMemory> {
    let (low, _high) = pair();
    PageTable::new(low).expect("dựng PageTable")
}

// ---------------------------------------------------------------- (1) bố cục

/// Vùng RX là `[__text_start, __data_start)` — **gồm** `.text.vectors`, `.el0`, `.rodata`.
#[test]
fn layout_rx_and_bytes() {
    assert_eq!(LAYOUT.rx(), Range::new(TEXT_START, DATA_START));
    assert_eq!(LAYOUT.text_bytes(), DATA_START - TEXT_START);
    assert_eq!(LAYOUT.text_bytes(), 0x1_2000);
    assert_eq!(LAYOUT.data_bytes(), KERNEL_END - DATA_START);
    assert_eq!(LAYOUT.data_bytes(), 0xB_F000);
    // 8 record × 1 trang guard 4 KiB.
    assert_eq!(LAYOUT.guard_pages(), 8);
    assert_eq!(LAYOUT.guard_bytes(), 0x8000);
    assert!(LAYOUT.is_page_aligned());
    // Vùng RX **không** chứa `.data`/`.bss`/stack.
    assert!(LAYOUT.rx().end <= DATA_START);
}

/// Các khoảng RW **đã trừ** guard: phủ đúng phần còn lại, không hở, không chồng, tăng dần.
#[test]
fn rw_ranges_exclude_guard() {
    let (ranges, count) = LAYOUT.rw_ranges();
    assert_eq!(count, 1 + 8, "một khoảng .data/.bss + tám khoảng stack");
    // Khoảng đầu: [data_start, stacks_start).
    assert_eq!(ranges[0], Range::new(DATA_START, STACKS_START));
    // Tám khoảng stack: bỏ trang guard ở đầu mỗi record.
    for hart in 0..8usize {
        let base = STACKS_START + hart as u64 * 0x1_1000;
        assert_eq!(
            ranges[1 + hart],
            Range::new(base + 0x1000, base + 0x1_1000),
            "hart {hart}"
        );
    }
    // Tăng dần, không chồng, không rỗng.
    let mut previous_end = 0u64;
    let mut total = 0u64;
    for range in ranges.iter().take(count) {
        assert!(!range.is_empty());
        assert!(range.start >= previous_end, "khoảng phải tăng dần");
        assert_eq!(range.start & 0xFFF, 0);
        assert_eq!(range.end & 0xFFF, 0);
        previous_end = range.end;
        total += range.bytes();
    }
    // Tổng byte = toàn vùng RW **trừ** guard.
    assert_eq!(total, LAYOUT.data_bytes() - LAYOUT.guard_bytes());
    // Mọi trang guard **không** nằm trong khoảng nào.
    for hart in 0..8usize {
        let guard = STACKS_START + hart as u64 * 0x1_1000;
        for range in ranges.iter().take(count) {
            assert!(
                guard < range.start || guard >= range.end,
                "trang guard 0x{guard:x} bị map bởi {range:?}"
            );
        }
    }
    // Mọi trang **không** phải guard trong [data_start, kernel_end) đều được phủ đúng một lần.
    let mut page = DATA_START;
    while page < KERNEL_END {
        let is_guard = (0..8usize).any(|hart| page == STACKS_START + hart as u64 * 0x1_1000);
        let hits = ranges
            .iter()
            .take(count)
            .filter(|range| page >= range.start && page < range.end)
            .count();
        assert_eq!(hits, if is_guard { 0 } else { 1 }, "trang 0x{page:x}");
        page += 0x1000;
    }
}

/// Bố cục không có arena stack (hoặc arena ngoài vùng ảnh) ⇒ **một** khoảng RW, không guard.
#[test]
fn layout_without_stacks_is_single_range() {
    let layout = ImageLayout::new(TEXT_START, DATA_START, DATA_START + 0x3000, 0, 0);
    let (ranges, count) = layout.rw_ranges();
    assert_eq!(count, 1);
    assert_eq!(ranges[0], Range::new(DATA_START, DATA_START + 0x3000));
    assert_eq!(layout.guard_pages(), 0);
    assert_eq!(layout.guard_bytes(), 0);
    // Khoảng rỗng ⇒ không khoảng nào.
    let empty = ImageLayout::new(TEXT_START, DATA_START, DATA_START, 0, 0);
    let (_, count) = empty.rw_ranges();
    assert_eq!(count, 0);
}

// ---------------------------------------------------------------- (2) quyền trên bảng thật

/// `map_low`: text RX (không ghi), data RW (không exec), guard **không map**.
#[test]
fn map_low_perms() {
    let mut table = empty_table();
    let pages = map_low(&mut table, &LAYOUT).expect("map_low");
    let expected = (LAYOUT.text_bytes() + LAYOUT.data_bytes() - LAYOUT.guard_bytes()) / 0x1000;
    assert_eq!(pages as u64, expected);

    let text = table.translate(TEXT_START).expect("text phải map");
    assert_eq!(text.pa, TEXT_START);
    assert!(text.perms.read && !text.perms.write && text.perms.exec && !text.perms.user);
    // `.rodata` nằm trong vùng RX ⇒ cũng không ghi được.
    let rodata = table
        .translate(DATA_START - 0x1000)
        .expect("rodata phải map");
    assert!(!rodata.perms.write, ".rodata phải read-only");

    let data = table.translate(DATA_START).expect("data phải map");
    assert_eq!(data.pa, DATA_START);
    assert!(data.perms.read && data.perms.write && !data.perms.exec && !data.perms.user);

    // Stack của hart 0 (ngay trên guard) map RW; guard **không** map.
    let stack = table
        .translate(STACKS_START + 0x1000)
        .expect("stack phải map");
    assert!(stack.perms.write && !stack.perms.exec);
    assert_eq!(
        table.translate(STACKS_START),
        Err(MapError::NotMapped),
        "trang guard của hart 0 KHÔNG được map"
    );
    assert_eq!(
        table.translate(STACKS_START + 7 * 0x1_1000),
        Err(MapError::NotMapped),
        "trang guard của hart 7 KHÔNG được map"
    );
}

/// `map_high`: alias nửa cao dùng **một** gốc (`__text_start`) cho **mọi** khoảng.
///
/// Nếu `.data` lấy đầu của chính nó làm gốc alias, `translate` sẽ trả PA lệch đúng bằng
/// `text_bytes` — test này bắt lỗi ấy.
#[test]
fn map_high_alias_origin() {
    let mut mmu = empty_mmu();
    let pages = map_high(&mut mmu, &LAYOUT).expect("map_high");
    let expected = (LAYOUT.text_bytes() + LAYOUT.data_bytes() - LAYOUT.guard_bytes()) / 0x1000;
    assert_eq!(pages as u64, expected);

    let text_va = high_alias_va(TEXT_START, TEXT_START, KERNEL_HIGH_BASE);
    let text = mmu.translate(text_va).expect("alias text phải map");
    assert_eq!(text.pa, TEXT_START);
    assert!(!text.perms.write && text.perms.exec);

    let data_va = high_alias_va(DATA_START, TEXT_START, KERNEL_HIGH_BASE);
    let data = mmu.translate(data_va).expect("alias data phải map");
    assert_eq!(
        data.pa, DATA_START,
        "alias .data phải trỏ đúng PA của .data"
    );
    assert!(data.perms.write && !data.perms.exec);

    // Guard không map ở alias nửa cao.
    let guard_va = high_alias_va(STACKS_START, TEXT_START, KERNEL_HIGH_BASE);
    assert_eq!(mmu.translate(guard_va), Err(MapError::NotMapped));
}

/// `map_phys`: physmap của text RX, của data RW, guard không map.
#[test]
fn map_phys_perms() {
    let mut mmu = empty_mmu();
    map_phys(&mut mmu, &LAYOUT).expect("map_phys");
    let text = mmu.translate(physmap_va(TEXT_START)).expect("physmap text");
    assert_eq!(text.pa, TEXT_START);
    assert!(!text.perms.write && text.perms.exec);
    let data = mmu.translate(physmap_va(DATA_START)).expect("physmap data");
    assert_eq!(data.pa, DATA_START);
    assert!(data.perms.write && !data.perms.exec);
    assert_eq!(
        mmu.translate(physmap_va(STACKS_START)),
        Err(MapError::NotMapped),
        "guard không được map qua physmap"
    );
}

/// `VSpace`: cả ba đường (identity nửa thấp, alias nửa cao, physmap) đều W^X.
#[test]
fn map_vspace_perms() {
    let mut vs = empty_vspace();
    map_vspace_low(&mut vs, &LAYOUT).expect("map_vspace_low");
    map_vspace_high(&mut vs, &LAYOUT).expect("map_vspace_high");
    map_vspace_phys(&mut vs, &LAYOUT).expect("map_vspace_phys");

    let text = vs.walk(TEXT_START).expect("identity text");
    assert_eq!(text.pa, TEXT_START);
    assert!(!text.perms.write && text.perms.exec);
    let data = vs.walk(DATA_START).expect("identity data");
    assert!(data.perms.write && !data.perms.exec);
    assert_eq!(vs.walk(STACKS_START), Err(MapError::NotMapped));

    let high_data = vs
        .walk(high_alias_va(DATA_START, TEXT_START, KERNEL_HIGH_BASE))
        .expect("alias data");
    assert_eq!(high_data.pa, DATA_START);
    assert!(high_data.perms.write);

    let phys_text = vs.walk(physmap_va(TEXT_START)).expect("physmap text");
    assert!(!phys_text.perms.write);
}

/// `Perms` dùng cho hai vùng là **hằng của crate dùng chung**, không phải giá trị dựng tay ở
/// ảnh: RX = `KERNEL_RX`, RW = `KERNEL_RW`. Test chốt để một lần đổi hằng không lặng lẽ đổi W^X.
#[test]
fn perms_constants_are_wx() {
    // Gán vào biến cục bộ để phép khẳng định là **giá trị chạy được**, không phải hằng biên dịch
    // (clippy `assertions_on_constants`); ý nghĩa không đổi: hai hằng này **phải** là W^X.
    let rx = Perms::KERNEL_RX;
    let rw = Perms::KERNEL_RW;
    assert!(
        rx.read && !rx.write && rx.exec,
        "KERNEL_RX phải là read+exec"
    );
    assert!(
        rw.read && rw.write && !rw.exec,
        "KERNEL_RW phải là read+write"
    );
    // Vùng ảnh dùng `MemAttr::Normal` (RAM), không phải Device.
    let normal = MemAttr::Normal;
    assert_eq!(normal.index(), 1);
}
