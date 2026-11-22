// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho đường bật MMU + identity map — KT-249 (K4' bước 3, B3.3).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64
//! ```
//!
//! Kỳ vọng viết tay từ Arm Architecture Reference Manual (DDI 0487):
//!
//! - **thứ tự** nạp thanh ghi điều khiển MMU: `TCR_EL1 → MAIR_EL1 → TTBR0_EL1 → SCTLR_EL1 → ISB`
//!   (`kt167-a64step2.md` §5; đảo thứ tự ⇒ walk dùng `TCR`/`MAIR` cũ);
//! - **mức bắt đầu walk** theo `T0SZ` (bảng D5-5, granule 4 KiB): `T0SZ = 25` (39-bit VA) ⇒ bắt
//!   đầu ở **mức 1** ⇒ `TTBR0_EL1` phải trỏ vào **bảng mức 1**, không phải bảng gốc mức 0;
//! - `MAIR_EL1` slot 1 = `0xFF` (Normal write-back) — slot mà leaf `MemAttr::Normal` dùng.
//!
//! Đây là ca kiểm **đỏ trước** của KT-249 (log: `v2/agent/JOURNAL/kt249-red.log`).
//! **Ba đột biến bắt buộc** (log: `v2/agent/JOURNAL/kt249-mutation.log`):
//! (i) bỏ `ISB` khỏi chuỗi ⇒ `mmu_enable_sequence_order` ĐỎ;
//! (ii) `T0SZ` 25→24 ⇒ `tcr_start_level` + `ttbr0_is_level1_table` ĐỎ (và trên QEMU: fault);
//! (iii) `MAIR` slot 1 sai ⇒ `mair_slot1_is_normal_wb` ĐỎ.

use core::sync::atomic::{AtomicU64, Ordering};

use vifast_plat_aarch64::boot_mmu::{
    map_identity, sctlr_for_mmu_on, start_level, ttbr0_base, MmuStep, MMU_ENABLE_SEQUENCE,
};
use vifast_plat_aarch64::bootmem::{BootMemory, BOOT_ARENA_FRAMES};
use vifast_plat_aarch64::format::{Aarch64, MemAttr, Perms};
use vifast_plat_aarch64::mmu::{PageTable, TableMemory};
use vifast_plat_aarch64::sysreg::{
    Mair, Sctlr, MAIR_EL1_KERNEL, SCTLR_C, SCTLR_I, SCTLR_M, TCR_EL1_KERNEL,
};

/// PA gốc của nguồn frame test (4 KiB aligned).
const FRAME_BASE: u64 = 0x4000_0000;
/// Số frame của nguồn test.
const FRAMES: usize = 32;

// ---------------------------------------------------------------- nguồn frame host

/// Nguồn frame + bộ nhớ bảng trang cho host: `Vec<u64>` phẳng, bump allocator, **không** zero ở
/// `alloc_frame` (đúng hợp đồng [`TableMemory`]).
struct TestMemory {
    words: Vec<AtomicU64>,
    base: u64,
    next: usize,
    frames: usize,
}

impl TestMemory {
    fn new() -> Self {
        Self {
            words: (0..FRAMES * 512).map(|_| AtomicU64::new(0)).collect(),
            base: FRAME_BASE,
            next: 0,
            frames: FRAMES,
        }
    }

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

impl TableMemory for TestMemory {
    fn alloc_frame(&mut self) -> Option<u64> {
        if self.next >= self.frames {
            return None;
        }
        let pa = self.base + (self.next as u64) * 4096;
        self.next += 1;
        Some(pa)
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

// ---------------------------------------------------------------- thứ tự chuỗi bật MMU

/// **Thứ tự bắt buộc**: `TCR → MAIR → TTBR0 → TTBR1 → SCTLR → ISB` (6 bước, không thừa không
/// thiếu). `TTBR1` được KT-253 (B3.4) thêm vào giữa `TTBR0` và `SCTLR`: nửa cao phải có gốc
/// **trước** khi `SCTLR_EL1.M = 1`.
///
/// Đột biến (i) — bỏ `ISB` — làm test này **ĐỎ**.
#[test]
fn mmu_enable_sequence_order() {
    // Dùng `.get()` chứ không `[i]`: nếu ai bỏ một bước, test phải **chạy** và ĐỎ với thông điệp
    // rõ, không phải lỗi biên dịch "index out of bounds" (lỗi biên dịch dễ bị nhầm là hỏng không
    // liên quan — bài học từ chính đột biến (i) của KT-249).
    assert_eq!(MMU_ENABLE_SEQUENCE.len(), 6, "chuỗi phải đủ 6 bước");
    assert_eq!(MMU_ENABLE_SEQUENCE.first(), Some(&MmuStep::WriteTcr));
    assert_eq!(MMU_ENABLE_SEQUENCE.get(1), Some(&MmuStep::WriteMair));
    assert_eq!(MMU_ENABLE_SEQUENCE.get(2), Some(&MmuStep::WriteTtbr0));
    assert_eq!(
        MMU_ENABLE_SEQUENCE.get(3),
        Some(&MmuStep::WriteTtbr1),
        "TTBR1 phải có trước SCTLR — nửa cao phải map xong trước khi bật MMU"
    );
    assert_eq!(MMU_ENABLE_SEQUENCE.get(4), Some(&MmuStep::WriteSctlr));
    assert_eq!(
        MMU_ENABLE_SEQUENCE.get(5),
        Some(&MmuStep::Isb),
        "ISB phải là bước cuối — thiếu nó thì MMU có thể chưa có hiệu lực"
    );
}

/// `SCTLR_EL1` sau khi bật: bit `M` = 1, các bit khác **giữ nguyên** (không ghi đè cấu hình
/// reset của firmware/QEMU).
#[test]
fn sctlr_sets_mmu_bit() {
    let off = sctlr_for_mmu_on(0);
    assert!(Sctlr::from_bits(off).bit(SCTLR_M), "M phải bật");
    assert_eq!(off, SCTLR_M, "từ 0 chỉ thêm M");

    let with_caches = sctlr_for_mmu_on(SCTLR_C | SCTLR_I);
    assert!(Sctlr::from_bits(with_caches).bit(SCTLR_M));
    assert!(Sctlr::from_bits(with_caches).bit(SCTLR_C), "giữ C");
    assert!(Sctlr::from_bits(with_caches).bit(SCTLR_I), "giữ I");
    assert_eq!(with_caches, SCTLR_C | SCTLR_I | SCTLR_M);
}

// ---------------------------------------------------------------- mức bắt đầu walk theo T0SZ

/// Bảng D5-5 (granule 4 KiB): `T0SZ = 25` ⇒ VA 39 bit ⇒ walk bắt đầu ở **mức 1**.
///
/// Đột biến (ii) — `T0SZ` 25→24 — làm test này **ĐỎ** (mức bắt đầu thành 0).
#[test]
fn tcr_start_level() {
    assert_eq!(TCR_EL1_KERNEL.t0sz(), 25, "64 - 39 bit VA");
    assert_eq!(start_level(25), Some(1), "39-bit VA ⇒ mức 1");
    assert_eq!(start_level(24), Some(0), "40-bit VA ⇒ mức 0");
    assert_eq!(start_level(16), Some(0), "48-bit VA ⇒ mức 0");
    assert_eq!(start_level(34), Some(2), "30-bit VA ⇒ mức 2");
    assert_eq!(start_level(43), Some(3), "21-bit VA ⇒ mức 3");
    // Ngoài dải biểu diễn được ⇒ `None` (không panic, không đoán).
    assert_eq!(start_level(15), None);
    assert_eq!(start_level(52), None);
}

/// `TTBR0_EL1` phải trỏ vào **bảng mức 1** (con của gốc mức 0) vì walk bắt đầu ở mức 1.
///
/// Nếu ai đưa thẳng bảng gốc mức 0 vào `TTBR0_EL1` thì phần cứng đọc nó như bảng mức 1 ⇒ mọi
/// địa chỉ dịch sai. Test này chốt **hành vi**, không chỉ chốt hằng số.
#[test]
fn ttbr0_is_level1_table() {
    let mut table = PageTable::<Aarch64, TestMemory>::new(TestMemory::new()).expect("tạo bảng");
    // Bảng rỗng: chưa có bảng mức 1 ⇒ `None` (không bịa địa chỉ).
    assert_eq!(ttbr0_base(&table), None, "bảng rỗng chưa có mức 1");

    table
        .map(0x4008_0000, 0x4008_0000, Perms::KERNEL_RW)
        .expect("map");
    // Bump allocator: frame 0 = gốc (mức 0), frame 1 = bảng mức 1.
    assert_eq!(
        ttbr0_base(&table),
        Some(FRAME_BASE + 0x1000),
        "TTBR0 = bảng mức 1, KHÔNG phải gốc mức 0"
    );
    assert_ne!(ttbr0_base(&table), Some(table.root()), "không được là gốc");
}

// ---------------------------------------------------------------- identity map

/// Identity map một khoảng: mỗi trang 4 KiB dịch về **chính nó**, đếm đúng số trang.
#[test]
fn map_identity_range() {
    let mut table = PageTable::<Aarch64, TestMemory>::new(TestMemory::new()).expect("tạo bảng");
    let start = 0x4008_0000u64;
    let end = 0x4008_3000u64; // 3 trang
    let pages = map_identity(&mut table, start, end, Perms::KERNEL_RW, MemAttr::Normal)
        .expect("map identity");
    assert_eq!(pages, 3);
    for page in 0..3u64 {
        let va = start + page * 0x1000;
        let t = table.translate(va).expect("dịch được");
        assert_eq!(t.pa, va, "identity: PA == VA");
    }
    // Ngoài khoảng đã map ⇒ lỗi, không im lặng trả PA rác.
    assert!(table.translate(end).is_err());
}

/// Đầu/cuối **không** căn 4 KiB được làm tròn ra ngoài (map đủ trang chứa chúng) — boot map theo
/// symbol linker nên hai đầu thường lệch.
#[test]
fn map_identity_aligns_outward() {
    let mut table = PageTable::<Aarch64, TestMemory>::new(TestMemory::new()).expect("tạo bảng");
    let pages = map_identity(
        &mut table,
        0x4008_0100,
        0x4008_2100,
        Perms::KERNEL_RW,
        MemAttr::Normal,
    )
    .expect("map identity");
    assert_eq!(pages, 3, "0x4008_0000..0x4008_3000");
    assert!(table.translate(0x4008_0000).is_ok(), "trang đầu được map");
    assert!(table.translate(0x4008_2000).is_ok(), "trang cuối được map");
    assert!(table.translate(0x4008_3000).is_err(), "không map thừa");
}

/// Khoảng rỗng (`start == end`) map 0 trang, không lỗi.
#[test]
fn map_identity_empty() {
    let mut table = PageTable::<Aarch64, TestMemory>::new(TestMemory::new()).expect("tạo bảng");
    assert_eq!(
        map_identity(
            &mut table,
            0x4008_0000,
            0x4008_0000,
            Perms::KERNEL_RW,
            MemAttr::Normal
        ),
        Ok(0)
    );
}

// ---------------------------------------------------------------- arena bảng trang boot

/// **KT-338a** — arena bảng trang phải đủ chỗ cho **cả** nhu cầu đã đo của ảnh **và** bốn trang
/// DMA **động** (blk/net/gpu/input) sẽ cấp từ chính arena thay vì `static` trong `.bss`.
///
/// Vế phải là **phép tính có ghi** (khuôn `k6-test-invariants.md` §4): `44` là số frame lớn nhất
/// đã đo trên ảnh KT-273 (không phải số chép tay của một bố cục mới) và `4` là bốn driver virtio
/// một-hart, mỗi driver một trang DMA.
///
/// Trước KT-338a `BOOT_ARENA_FRAMES = 44` ⇒ 44 < 48 ⇒ **ĐỎ** (log `v2/agent/JOURNAL/kt338a-red.log`).
#[test]
fn boot_arena_has_room_for_dynamic_dma_pages() {
    const MEASURED_KT273_FRAMES: usize = 44; // `frames_used` lớn nhất đã đo trên ảnh KT-273
    const DMA_PAGES: usize = 4; // blk, net, gpu, input — một trang mỗi driver
    assert!(
        BOOT_ARENA_FRAMES >= MEASURED_KT273_FRAMES + DMA_PAGES,
        "arena {} frame < {} frame đã đo + {} trang DMA động",
        BOOT_ARENA_FRAMES,
        MEASURED_KT273_FRAMES,
        DMA_PAGES
    );
}

/// Arena boot cấp frame tuần tự từ `base`, đọc/ghi entry đúng chỗ, `zero_frame` xoá sạch.
#[test]
fn boot_memory_round_trip() {
    let words: &'static [AtomicU64] = Box::leak(
        (0..BOOT_ARENA_FRAMES * 512)
            .map(|_| AtomicU64::new(0))
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );
    let mut mem = BootMemory::new(words, FRAME_BASE);
    let f0 = mem.alloc_frame().expect("frame 0");
    let f1 = mem.alloc_frame().expect("frame 1");
    assert_eq!(f0, FRAME_BASE);
    assert_eq!(f1, FRAME_BASE + 0x1000);

    assert!(mem.write_entry(f0, 7, 0xDEAD_BEEF));
    assert_eq!(mem.read_entry(f0, 7), Some(0xDEAD_BEEF));
    assert_eq!(mem.read_entry(f1, 7), Some(0), "frame khác không bị ghi");
    assert!(mem.zero_frame(f0));
    assert_eq!(mem.read_entry(f0, 7), Some(0), "zero_frame xoá sạch");

    // PA ngoài arena ⇒ `None`/`false`, không đọc bừa.
    assert_eq!(mem.read_entry(0xDEAD_0000, 0), None);
    assert!(!mem.write_entry(0xDEAD_0000, 0, 1));
}

/// Cạn arena ⇒ `alloc_frame` trả `None` (không trả địa chỉ ngoài arena).
#[test]
fn boot_memory_exhausts() {
    let words: &'static [AtomicU64] = Box::leak(
        (0..BOOT_ARENA_FRAMES * 512)
            .map(|_| AtomicU64::new(0))
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );
    let mut mem = BootMemory::new(words, FRAME_BASE);
    for i in 0..BOOT_ARENA_FRAMES {
        assert_eq!(mem.alloc_frame(), Some(FRAME_BASE + (i as u64) * 0x1000));
    }
    assert_eq!(mem.alloc_frame(), None, "hết frame ⇒ None");
}

// ---------------------------------------------------------------- MAIR

/// `MAIR_EL1` slot 1 = `0xFF` (Normal write-back RW-allocate) — slot mà leaf `MemAttr::Normal`
/// dùng; slot 0 = `0x00` (Device-nGnRnE). Đột biến (iii) làm test này **ĐỎ**.
#[test]
fn mair_slot1_is_normal_wb() {
    assert_eq!(MAIR_EL1_KERNEL.bits(), 0x0000_0000_0000_FF00);
    assert_eq!(MAIR_EL1_KERNEL.slot(0), 0x00, "Device-nGnRnE");
    assert_eq!(MAIR_EL1_KERNEL.slot(1), 0xFF, "Normal WB");
    assert_eq!(MemAttr::Device.index(), 0);
    assert_eq!(MemAttr::Normal.index(), 1);
    // Slot 1 sai (ví dụ 0x00) thì leaf Normal đọc như Device ⇒ test này phải bắt được.
    let wrong = Mair::new([0x00, 0x00, 0, 0, 0, 0, 0, 0]);
    assert_ne!(wrong.slot(1), MAIR_EL1_KERNEL.slot(1));
}
