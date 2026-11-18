// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **generic** chứng minh `PageTableFormat`/`Mmu` **dùng chung** thật — KT-164.
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common
//! ```
//!
//! Trước KT-164, `PageTableFormat`/`Mmu` được định nghĩa **cục bộ** trong
//! `vifast-plat-aarch64` (nợ N1 của KT-160) nên "hai arch thay thế được cho nhau" chỉ là tuyên
//! bố. Tệp này chốt bằng **một** thân test generic [`kiem_tung_dinh_dang`] chạy cho **hai** định
//! dạng thật:
//!
//! - [`Aarch64`] — 4 mức, 4 KiB, 39-bit VA, 48-bit PA (`vifast-plat-aarch64`);
//! - [`Sv39`] — 3 mức, 4 KiB, 39-bit VA, 56-bit PA (`vifast-plat-common`).
//!
//! Cùng thân test ⇒ cùng map/unmap/translate/quyền/đếm frame; chỉ khác `F::LEVELS` (số bảng
//! trung gian), đúng bản chất hình học. Nếu ai định nghĩa lại trait ở một crate (hai bản trùng)
//! thì test này không biên dịch được cho cả hai — đó là bất biến của KT-164.
//!
//! `PageTableBuilder` (Sv39, arena) cài **cùng** trait `Mmu`; test
//! [`builder_sv39_implements_mmu`] gọi nó **qua trait** để chốt.

use vifast_plat_aarch64::format::Aarch64;
use vifast_plat_aarch64::mmu::{PageTable, TableMemory};
use vifast_plat_common::mmu::{PageTableBuilder, Sv39};
use vifast_pte::{MapError, Mmu, PageTableFormat, Perms};

/// Số entry một bảng trang của test (4 KiB / 8 B).
const ENTRIES: usize = 512;
/// Cỡ một frame/bảng.
const FRAME_BYTES: u64 = 4096;
/// PA của frame đầu tiên mà [`TestMemory`] cấp.
const FRAME_BASE: u64 = 0x4000_0000;
/// Rác bơm vào frame đã free — `Mmu` phải zero trước khi dùng lại.
const GARBAGE: u64 = 0xDEAD_BEEF_DEAD_BEEF;

/// Nguồn frame + bộ nhớ bảng trang cho host: `Vec` frame 4 KiB, free-list LIFO, đếm alloc/free.
///
/// Cố ý **không** zero ở `alloc_frame` (đúng hợp đồng [`TableMemory`]): frame tái sử dụng còn
/// rác [`GARBAGE`] từ `free_frame`, nên nếu `Mmu` quên `zero_frame` thì test đỏ ngay.
struct TestMemory {
    base: u64,
    frames: Vec<Option<Box<[u64; ENTRIES]>>>,
    free: Vec<usize>,
    allocs: usize,
    frees: usize,
}

impl TestMemory {
    fn new(base: u64) -> Self {
        Self {
            base,
            frames: Vec::new(),
            free: Vec::new(),
            allocs: 0,
            frees: 0,
        }
    }

    /// Số frame đang sống (`alloc − free`) — bất biến "không rò rỉ frame".
    fn outstanding(&self) -> usize {
        self.allocs - self.frees
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

/// **Một** thân test cho **mọi** định dạng bảng trang — trái tim của bằng chứng KT-164.
///
/// Chạy đúng các phép mà tầng trên cần: map 4 KiB, translate (PA + quyền + mức), quyền user,
/// ca âm (không cấp frame), unmap trả PA + giải phóng bảng rỗng, destroy giải phóng cả gốc.
/// Không chỗ nào nhắc tới AArch64 hay Sv39 — chỉ `F::LEVELS`/`F::PAGE_BYTES` của trait.
fn kiem_tung_dinh_dang<F: PageTableFormat>() {
    let mut mmu = PageTable::<F, _>::new(TestMemory::new(FRAME_BASE)).expect("cấp frame gốc");
    let va = 0x1000u64;
    let pa = 0x8000_0000u64;

    // 1. Map 3 trang liên tiếp trong cùng một vùng 2 MiB + 1 trang user ở index 4.
    for i in 0..3u64 {
        mmu.map(va + i * 0x1000, pa + i * 0x1000, Perms::KERNEL_RW)
            .expect("map 4 KiB");
    }
    mmu.map(va + 0x4000, pa + 0x4000, Perms::USER_RX)
        .expect("map user RX");

    // 2. Translate: PA, quyền và mức lá round-trip qua descriptor của **chính** format này.
    for i in 0..3u64 {
        let t = mmu.translate(va + i * 0x1000).expect("translate");
        assert_eq!(t.pa, pa + i * 0x1000, "PA round-trip");
        assert_eq!(t.perms, Perms::KERNEL_RW, "quyền round-trip");
        assert_eq!(t.level, F::LEVELS - 1, "lá ở mức cuối");
    }
    assert_eq!(
        mmu.translate(va + 0x4000).expect("translate user").perms,
        Perms::USER_RX,
        "quyền user round-trip"
    );

    // 3. Không rò rỉ frame: gốc + (LEVELS - 1) bảng trung gian, không hơn.
    assert_eq!(
        mmu.memory().outstanding(),
        F::LEVELS,
        "gốc + bảng trung gian = LEVELS frame"
    );

    // 4. Ca âm: từ chối **trước** khi cấp frame, mapping cũ giữ nguyên.
    let before = mmu.memory().outstanding();
    assert_eq!(
        mmu.map(va, pa, Perms::KERNEL_RW),
        Err(MapError::AlreadyMapped),
        "map chồng"
    );
    assert_eq!(
        mmu.map(va + 1, pa, Perms::KERNEL_RW),
        Err(MapError::Unaligned),
        "VA lệch trang"
    );
    assert_eq!(
        mmu.map(0x0000_0080_0000_0000, pa, Perms::KERNEL_RW),
        Err(MapError::NotCanonical),
        "VA ngoài VA_BITS"
    );
    assert_eq!(
        mmu.map(va + 0x5000, pa + 0x5000, Perms::NONE),
        Err(MapError::InvalidPerms),
        "không quyền nào"
    );
    assert_eq!(
        mmu.memory().outstanding(),
        before,
        "từ chối không được cấp frame"
    );
    assert_eq!(
        mmu.translate(va).expect("mapping cũ").pa,
        pa,
        "mapping cũ giữ nguyên"
    );

    // 5. Unmap trả PA, giải phóng bảng rỗng; hết mapping chỉ còn frame gốc.
    for i in [0u64, 1, 2, 4] {
        assert_eq!(
            mmu.unmap(va + i * 0x1000),
            Ok(pa + i * 0x1000),
            "unmap trả PA"
        );
    }
    assert_eq!(mmu.memory().outstanding(), 1, "chỉ còn frame gốc");
    assert_eq!(mmu.translate(va), Err(MapError::NotMapped));
    assert_eq!(mmu.unmap(va), Err(MapError::NotMapped), "unmap hai lần");

    // 6. Destroy giải phóng cả gốc — `alloc == free`.
    let memory = mmu.destroy().expect("destroy");
    assert_eq!(memory.outstanding(), 0, "không rò rỉ frame");
}

#[test]
fn dinh_dang_aarch64() {
    kiem_tung_dinh_dang::<Aarch64>();
}

#[test]
fn dinh_dang_sv39() {
    kiem_tung_dinh_dang::<Sv39>();
}

#[test]
fn builder_sv39_implements_mmu() {
    // Chốt lúc biên dịch: `PageTableBuilder` cài **cùng** trait `Mmu` với `PageTable`.
    fn assert_mmu<M: Mmu>() {}
    assert_mmu::<PageTableBuilder<'static>>();

    let arena: &'static mut [u8] = Box::leak(vec![0u8; 64 * 1024].into_boxed_slice());
    let mut builder = PageTableBuilder::new(arena, 0x8021_9000).expect("arena hợp lệ");
    assert_eq!(Mmu::root(&builder), 0x8021_9000);

    Mmu::map(&mut builder, 0x1000, 0x8000_1000, Perms::KERNEL_RW).expect("map qua trait Mmu");
    let t = Mmu::translate(&builder, 0x1000).expect("translate qua trait Mmu");
    assert_eq!(t.pa, 0x8000_1000);
    assert_eq!(t.perms, Perms::KERNEL_RW);
    assert_eq!(t.level, 2, "lá 4 KiB ở mức cuối của Sv39");

    assert_eq!(Mmu::unmap(&mut builder, 0x1000), Ok(0x8000_1000));
    assert_eq!(Mmu::translate(&builder, 0x1000), Err(MapError::NotMapped));
    assert_eq!(Mmu::unmap(&mut builder, 0x1000), Err(MapError::NotMapped));
    assert_eq!(
        Mmu::map(&mut builder, 0x1000, 0x8000_1000, Perms::NONE),
        Err(MapError::InvalidPerms),
        "Sv39: không quyền nào"
    );
    assert_eq!(
        Mmu::map(
            &mut builder,
            0x2000,
            0x8000_2000,
            Perms::new(false, true, false, false)
        ),
        Err(MapError::InvalidPerms),
        "Sv39: W=1 mà R=0 là reserved"
    );
}
