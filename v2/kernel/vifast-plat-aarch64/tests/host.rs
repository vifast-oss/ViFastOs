// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho `vifast-plat-aarch64` — KT-160 / D2-147 (K4' bước 1).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64
//! ```
//!
//! Bằng chứng đo được của bước 1 (không boot aarch64, không QEMU):
//!
//! 1. **Descriptor**: từng bit `V/bit1/AF/AP[2:1]/UXN/PXN/nG/SH/AttrIndx` đúng đặc tả ARMv8-A;
//!    `MAIR_EL1` có byte cho mọi chỉ số mà leaf dùng.
//! 2. **Map/translate 4 mức**: walk đi **đủ 4 cấp**, chỉ số từng cấp tính tay; nửa cao dùng
//!    `root[511]`; nửa thấp dùng `root[0]`.
//! 3. **Quyền**: R/W/X + user/kernel round-trip qua descriptor; `read = false` bị từ chối vì
//!    AArch64 không biểu diễn được leaf "không đọc".
//! 4. **Ca âm**: VA/PA lệch 4 KiB, VA không canonical, PA vượt 48 bit, map chồng, unmap VA trống,
//!    block descriptor — tất cả trả lỗi **và không đổi bộ đếm frame**.
//! 5. **Không rò rỉ frame**: `alloc − free` sau unmap hết = 1 (gốc); sau `destroy` = 0; frame cấp
//!    lại bị bẩn vẫn được zero trước khi dùng.
//! 6. **Format không hardcode AArch64**: cùng `Mmu` chạy với format 3 mức thứ hai, và format
//!    AArch64 48-bit VA (thiết kế §541) dùng lại đúng cách mã hoá.

use vifast_plat_aarch64::format::{
    is_canonical_bits, mair_byte, mair_index, Aarch64, Descriptor, MemAttr, PageTableFormat, Perms,
    Shareability, AARCH64_ENTRIES, AARCH64_LEVELS, AARCH64_MAX_VA_BITS, AARCH64_PAGE_BYTES,
    AARCH64_PA_BITS, AARCH64_VA_BITS, DESC_ADDR_MASK, DESC_AF, DESC_AP_RO, DESC_AP_USER,
    DESC_ATTR_SHIFT, DESC_NG, DESC_NS, DESC_PXN, DESC_SH_SHIFT, DESC_UXN, DESC_VALID, MAIR_EL1,
};
use vifast_plat_aarch64::mmu::{MapError, Mmu, PageTable, TableMemory, Translation, WalkOutcome};

/// Số entry một bảng trang của test (4 KiB / 8 B).
const ENTRIES: usize = 512;
/// Cỡ frame của test.
const FRAME_BYTES: u64 = 4096;
/// Rác bơm vào frame lúc free — để chốt "frame cấp lại phải được zero".
const GARBAGE: u64 = 0xDEAD_BEEF_DEAD_BEEF;
/// PA gốc của nguồn frame test (4 KiB aligned).
const FRAME_BASE: u64 = 0x4000_0000;
/// VA mẫu nửa thấp (canonical 39-bit).
const VA: u64 = 0x0000_0000_0000_1000;
/// PA mẫu.
const PA: u64 = 0x0000_0000_8000_1000;

// ---------------------------------------------------------------- nguồn frame test

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
    fail_after: Option<usize>,
}

impl TestMemory {
    fn new(base: u64) -> Self {
        Self {
            base,
            frames: Vec::new(),
            free: Vec::new(),
            allocs: 0,
            frees: 0,
            fail_after: None,
        }
    }

    fn outstanding(&self) -> usize {
        self.allocs - self.frees
    }

    fn allocs(&self) -> usize {
        self.allocs
    }

    fn frees(&self) -> usize {
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

fn new_mmu() -> PageTable<Aarch64, TestMemory> {
    PageTable::new(TestMemory::new(FRAME_BASE)).expect("cấp frame gốc")
}

// ---------------------------------------------------------------- 1. descriptor

#[test]
fn descriptor_bits_aarch64() {
    let pa = 0x0000_0000_8000_1000u64;

    // Kernel RW, NX, Normal, Inner.
    let d = Aarch64::encode_leaf(pa, Perms::KERNEL_RW, MemAttr::Normal, Shareability::Inner);
    assert_eq!(
        d & 0b11,
        0b11,
        "leaf phải là page descriptor (bit1:0 = 0b11)"
    );
    assert_ne!(
        d & DESC_AF,
        0,
        "AF bắt buộc 1 — thiếu là fault mọi truy cập"
    );
    assert_eq!(d & DESC_AP_RO, 0, "kernel RW: AP[2] = 0");
    assert_eq!(d & DESC_AP_USER, 0, "kernel: AP[1] = 0 (EL0 không vào)");
    assert_ne!(d & DESC_UXN, 0, "kernel: UXN = 1 (EL0 không thực thi)");
    assert_ne!(d & DESC_PXN, 0, "kernel NX: PXN = 1 (EL1 không thực thi)");
    assert_eq!(d & DESC_NG, 0, "kernel leaf: global (nG = 0)");
    assert_eq!(d & DESC_NS, 0, "bước 1: non-secure luôn 0");
    assert_eq!(
        (d >> DESC_ATTR_SHIFT) & 0b111,
        mair_index(MemAttr::Normal),
        "AttrIndx = chỉ số MAIR của Normal"
    );
    assert_eq!(
        (d >> DESC_SH_SHIFT) & 0b11,
        Shareability::Inner.bits(),
        "SH = inner shareable"
    );
    assert_eq!(d & DESC_ADDR_MASK, pa, "PA nằm ở bit 47:12");
    assert_eq!(
        Aarch64::leaf_mair_index(d),
        mair_index(MemAttr::Normal),
        "helper bóc AttrIndx"
    );
    assert_eq!(
        Aarch64::leaf_shareability(d),
        Some(Shareability::Inner),
        "helper bóc SH"
    );

    // User RX: AP[1]=1, AP[2]=1 (RO), UXN=0, PXN=1, nG=1.
    let u = Aarch64::encode_leaf(pa, Perms::USER_RX, MemAttr::Normal, Shareability::Inner);
    assert_ne!(u & DESC_AP_USER, 0, "user: AP[1] = 1");
    assert_ne!(u & DESC_AP_RO, 0, "user RX: AP[2] = 1 (chỉ đọc)");
    assert_eq!(u & DESC_UXN, 0, "user exec: UXN = 0");
    assert_ne!(
        u & DESC_PXN,
        0,
        "user: PXN = 1 (EL1 không thực thi trang user)"
    );
    assert_ne!(u & DESC_NG, 0, "leaf user phải nG (bài học v1 P0-1)");

    // Device, non-shareable.
    let dev = Aarch64::encode_leaf(
        pa,
        Perms::KERNEL_RW,
        MemAttr::Device,
        Shareability::NonShareable,
    );
    assert_eq!((dev >> DESC_ATTR_SHIFT) & 0b111, 0, "Device = MAIR[0]");
    assert_eq!((dev >> DESC_SH_SHIFT) & 0b11, 0, "non-shareable");

    // Table descriptor: V|bit1|PA, không AF, không AP.
    let t = Aarch64::encode_table(pa);
    assert_eq!(t & 0b11, 0b11, "table descriptor = 0b11");
    assert_eq!(t & DESC_AF, 0, "table không mang AF");
    assert_eq!(t & DESC_ADDR_MASK, pa);
    assert_eq!(Aarch64::decode(t, 0), Descriptor::Table { pa });
    assert_eq!(Aarch64::decode(t, 1), Descriptor::Table { pa });
    assert_eq!(Aarch64::decode(t, 2), Descriptor::Table { pa });
    // Ở mức lá, 0b11 là page descriptor — không phải table.
    assert!(matches!(Aarch64::decode(t, 3), Descriptor::Leaf { pa: p, .. } if p == pa));

    // Entry trống.
    assert_eq!(Aarch64::decode(0, 0), Descriptor::Invalid);
    assert_eq!(Aarch64::decode(0, 3), Descriptor::Invalid);
}

#[test]
fn mair_index_matches_programmed_mair() {
    for attr in [MemAttr::Device, MemAttr::Normal] {
        let index = mair_index(attr);
        assert!(index < 8, "AttrIndx chỉ 3 bit");
        assert_eq!(
            (MAIR_EL1 >> (8 * index)) & 0xFF,
            mair_byte(attr),
            "byte MAIR của {attr:?} phải khớp MAIR_EL1"
        );
        assert_eq!(MemAttr::from_index(index), Some(attr));
    }
    assert_eq!(mair_byte(MemAttr::Device), 0x00);
    assert_eq!(mair_byte(MemAttr::Normal), 0xFF);
    assert_eq!(MemAttr::from_index(2), None, "chỉ số MAIR chưa định nghĩa");
    assert_eq!(Shareability::from_bits(0b01), None, "SH = 0b01 là reserved");
    assert_eq!(Shareability::from_bits(0b11), Some(Shareability::Inner));
}

#[test]
fn permissions_roundtrip() {
    let pa = 0x0000_0000_9000_0000u64;
    let cases = [
        Perms::KERNEL_RW,
        Perms::KERNEL_RX,
        Perms::KERNEL_RWX,
        Perms::USER_RW,
        Perms::USER_RX,
        Perms::USER_RO,
    ];
    for perms in cases {
        let d = Aarch64::encode_leaf(pa, perms, MemAttr::Normal, Shareability::Inner);
        match Aarch64::decode(d, AARCH64_LEVELS - 1) {
            Descriptor::Leaf {
                pa: got,
                perms: got_perms,
            } => {
                assert_eq!(got, pa, "PA round-trip");
                assert_eq!(got_perms, perms, "quyền round-trip cho {perms:?}");
            }
            other => panic!("phải là Leaf, nhận {other:?}"),
        }
        // Bit mức thấp: AP[2] = !write, AP[1] = user, UXN/PXN theo exec+user, nG = user.
        assert_eq!(d & DESC_AP_RO == 0, perms.write, "AP[2] cho {perms:?}");
        assert_eq!(d & DESC_AP_USER != 0, perms.user, "AP[1] cho {perms:?}");
        assert_eq!(
            d & DESC_UXN == 0,
            perms.exec && perms.user,
            "UXN cho {perms:?}"
        );
        assert_eq!(
            d & DESC_PXN == 0,
            perms.exec && !perms.user,
            "PXN cho {perms:?}"
        );
        assert_eq!(d & DESC_NG != 0, perms.user, "nG cho {perms:?}");
        assert_ne!(d & DESC_AF, 0, "AF cho {perms:?}");
    }
}

// ---------------------------------------------------------------- 2. map/translate 4 mức

#[test]
fn map_translate_4_levels() {
    let mut mmu = new_mmu();
    assert_eq!(mmu.memory().outstanding(), 1, "chỉ frame gốc");

    mmu.map(VA, PA, Perms::KERNEL_RW).expect("map 4 KiB");
    assert_eq!(
        mmu.memory().outstanding(),
        4,
        "gốc + 3 bảng trung gian cho một lá 4 KiB"
    );

    let t = mmu.translate(VA).expect("translate");
    assert_eq!(t.pa, PA);
    assert_eq!(t.perms, Perms::KERNEL_RW);
    assert_eq!(t.level, AARCH64_LEVELS - 1, "lá ở mức cuối");

    let walk = mmu.walk(VA);
    assert_eq!(walk.len, 4, "walk phải đi đủ 4 mức");
    assert_eq!(
        walk.outcome,
        WalkOutcome::Leaf {
            pa: PA,
            perms: Perms::KERNEL_RW
        }
    );
    let indices: Vec<usize> = (0..4).map(|i| walk.step(i).unwrap().index).collect();
    assert_eq!(indices, vec![0, 0, 0, 1], "chỉ số từng cấp tính tay");
    let levels: Vec<usize> = (0..4).map(|i| walk.step(i).unwrap().level).collect();
    assert_eq!(levels, vec![0, 1, 2, 3]);
    assert_eq!(
        walk.step(0).unwrap().table,
        mmu.root(),
        "bước 0 đọc bảng gốc"
    );
    for i in 1..4 {
        let prev = walk.step(i - 1).unwrap();
        assert_eq!(
            Aarch64::decode(prev.entry, prev.level),
            Descriptor::Table {
                pa: walk.step(i).unwrap().table
            },
            "entry mức {i} phải trỏ đúng bảng mức {i}"
        );
    }
    assert_eq!(walk.step(3).unwrap().entry & DESC_ADDR_MASK, PA);
}

#[test]
fn map_high_half_uses_root_511() {
    let mut mmu = new_mmu();
    // Nửa cao canonical 39-bit: bit 38 = 1, bit 63:39 = 1.
    let high = 0xFFFF_FFC0_0000_0000u64;
    assert!(Aarch64::is_canonical(high), "nửa cao 39-bit phải canonical");
    mmu.map(high, PA, Perms::KERNEL_RW).expect("map nửa cao");
    let walk = mmu.walk(high);
    assert_eq!(walk.len, 4);
    assert_eq!(walk.step(0).unwrap().index, 511, "nửa cao dùng root[511]");
    assert_eq!(
        walk.step(1).unwrap().index,
        256,
        "bit 38 là MSB của chỉ số mức 1"
    );
    assert_eq!(walk.step(2).unwrap().index, 0);
    assert_eq!(walk.step(3).unwrap().index, 0);
    assert_eq!(mmu.translate(high).unwrap().pa, PA);

    // Trang cao nhất của không gian 39-bit: mọi chỉ số = 511.
    let top = 0xFFFF_FFFF_FFFF_F000u64;
    assert!(Aarch64::is_canonical(top));
    mmu.map(top, PA + 0x1000, Perms::KERNEL_RX)
        .expect("map trang cao nhất");
    let walk = mmu.walk(top);
    let indices: Vec<usize> = (0..4).map(|i| walk.step(i).unwrap().index).collect();
    assert_eq!(indices, vec![511, 511, 511, 511]);
}

#[test]
fn format_48_bit_va() {
    // Thiết kế §541: format 4 KiB granule 4 mức **có khả năng** 48-bit VA. D2-147 chốt kernel
    // dùng 39-bit canonical; test này chứng minh đổi `VA_BITS` là đủ, không đổi hình walk.
    struct Aarch64Full48;
    impl PageTableFormat for Aarch64Full48 {
        const LEVELS: usize = AARCH64_LEVELS;
        const PAGE_BITS: u32 = 12;
        const INDEX_BITS: u32 = 9;
        const VA_BITS: u32 = AARCH64_MAX_VA_BITS;
        const PA_BITS: u32 = AARCH64_PA_BITS;
        const PAGE_BYTES: u64 = AARCH64_PAGE_BYTES;
        const ENTRIES: usize = AARCH64_ENTRIES;

        fn index(va: u64, level: usize) -> usize {
            Aarch64::index(va, level)
        }
        fn is_canonical(va: u64) -> bool {
            is_canonical_bits(va, AARCH64_MAX_VA_BITS)
        }
        fn encode_table(pa: u64) -> u64 {
            Aarch64::encode_table(pa)
        }
        fn encode_leaf(pa: u64, perms: Perms, attr: MemAttr, share: Shareability) -> u64 {
            Aarch64::encode_leaf(pa, perms, attr, share)
        }
        fn decode(entry: u64, level: usize) -> Descriptor {
            Aarch64::decode(entry, level)
        }
    }

    let mut mmu = PageTable::<Aarch64Full48, _>::new(TestMemory::new(FRAME_BASE)).unwrap();
    // VA canonical 48-bit nhưng KHÔNG canonical 39-bit: bit 39 = 1, bit 47:40 = 0.
    let va48 = 0x0000_0080_0000_0000u64;
    assert!(!Aarch64::is_canonical(va48), "39-bit phải từ chối VA này");
    assert!(Aarch64Full48::is_canonical(va48));
    mmu.map(va48, PA, Perms::KERNEL_RW).expect("map VA 48-bit");
    let walk = mmu.walk(va48);
    assert_eq!(walk.len, 4);
    assert_eq!(
        walk.step(0).unwrap().index,
        1,
        "bit 39 là LSB của chỉ số mức 0"
    );
    assert_eq!(mmu.translate(va48).unwrap().pa, PA);
}

// ---------------------------------------------------------------- 3. ca âm

#[test]
fn map_unaligned_rejected() {
    let mut mmu = new_mmu();
    let before = (mmu.memory().allocs(), mmu.memory().frees());
    assert_eq!(
        mmu.map(VA + 1, PA, Perms::KERNEL_RW),
        Err(MapError::Unaligned),
        "VA lệch 4 KiB"
    );
    assert_eq!(
        mmu.map(VA, PA + 0x800, Perms::KERNEL_RW),
        Err(MapError::Unaligned),
        "PA lệch 4 KiB"
    );
    assert_eq!(
        (mmu.memory().allocs(), mmu.memory().frees()),
        before,
        "map lỗi không được cấp frame"
    );
}

#[test]
fn map_non_canonical_rejected() {
    let mut mmu = new_mmu();
    let before = (mmu.memory().allocs(), mmu.memory().frees());
    // bit 39 = 1 nhưng bit 38 = 0 ⇒ không canonical 39-bit.
    assert_eq!(
        mmu.map(0x0000_0080_0000_0000, PA, Perms::KERNEL_RW),
        Err(MapError::NotCanonical)
    );
    // bit 38 = 0 nhưng bit 63:39 = 1 ⇒ không canonical.
    assert_eq!(
        mmu.map(0xFFFF_FF80_0000_0000, PA, Perms::KERNEL_RW),
        Err(MapError::NotCanonical)
    );
    assert_eq!(
        mmu.translate(0x0000_0080_0000_0000),
        Err(MapError::NotCanonical)
    );
    assert_eq!(
        (mmu.memory().allocs(), mmu.memory().frees()),
        before,
        "map lỗi không được cấp frame"
    );
}

#[test]
fn map_pa_out_of_range_rejected() {
    let mut mmu = new_mmu();
    let before = (mmu.memory().allocs(), mmu.memory().frees());
    assert_eq!(
        mmu.map(VA, 1u64 << AARCH64_PA_BITS, Perms::KERNEL_RW),
        Err(MapError::PaOutOfRange),
        "PA = 2^48 vượt 48 bit"
    );
    assert_eq!(
        mmu.map(VA, 0x0001_0000_0000_0000, Perms::KERNEL_RW),
        Err(MapError::PaOutOfRange)
    );
    assert_eq!((mmu.memory().allocs(), mmu.memory().frees()), before);
}

#[test]
fn map_overlap_rejected() {
    let mut mmu = new_mmu();
    mmu.map(VA, PA, Perms::KERNEL_RW).expect("map lần đầu");
    let before = (mmu.memory().allocs(), mmu.memory().frees());
    assert_eq!(
        mmu.map(VA, PA + 0x1000, Perms::KERNEL_RX),
        Err(MapError::AlreadyMapped),
        "map chồng phải bị từ chối"
    );
    assert_eq!(
        (mmu.memory().allocs(), mmu.memory().frees()),
        before,
        "map chồng không được cấp frame"
    );
    assert_eq!(mmu.translate(VA).unwrap().pa, PA, "mapping cũ giữ nguyên");
}

#[test]
fn map_invalid_perms_rejected() {
    let mut mmu = new_mmu();
    let before = (mmu.memory().allocs(), mmu.memory().frees());
    assert_eq!(
        mmu.map(VA, PA, Perms::NONE),
        Err(MapError::InvalidPerms),
        "không quyền nào"
    );
    assert_eq!(
        mmu.map(VA, PA, Perms::new(false, true, false, false)),
        Err(MapError::InvalidPerms),
        "AArch64 không có leaf ghi-mà-không-đọc"
    );
    assert_eq!((mmu.memory().allocs(), mmu.memory().frees()), before);
}

#[test]
fn unmap_unmapped_rejected() {
    let mut mmu = new_mmu();
    let before = (mmu.memory().allocs(), mmu.memory().frees());
    assert_eq!(mmu.unmap(VA), Err(MapError::NotMapped));
    assert_eq!(mmu.unmap(VA + 1), Err(MapError::Unaligned));
    assert_eq!(
        mmu.unmap(0x0000_0080_0000_0000),
        Err(MapError::NotCanonical)
    );
    assert_eq!(
        (mmu.memory().allocs(), mmu.memory().frees()),
        before,
        "unmap lỗi không được free frame"
    );
}

#[test]
fn block_descriptor_rejected() {
    // Block 1 GiB hợp lệ ở mức 1: kind = 0b01 (bit1 = 0, bit0 = 1).
    let block = DESC_VALID | (0x4000_0000 & DESC_ADDR_MASK);
    assert_eq!(block & 0b11, 0b01);
    let mut mmu = new_mmu();
    mmu.map(VA, PA, Perms::KERNEL_RW).expect("map");
    let l1 = mmu.walk(VA).step(1).unwrap().table;
    assert!(mmu.memory_mut().write_entry(l1, 0, block));
    assert_eq!(
        mmu.translate(VA),
        Err(MapError::BlockUnsupported),
        "translate gặp block phải báo, không nói dối"
    );
    assert_eq!(
        mmu.map(VA + 0x1000, PA, Perms::KERNEL_RW),
        Err(MapError::AlreadyMapped),
        "VA nằm trong block ⇒ đã map"
    );
    assert_eq!(mmu.unmap(VA), Err(MapError::BlockUnsupported));

    // Block ở mức 0 là **reserved** theo đặc tả ⇒ CorruptTable, không lặng lẽ nhận.
    let mut mmu = new_mmu();
    let root = mmu.root();
    assert!(mmu.memory_mut().write_entry(root, 0, block));
    assert_eq!(mmu.translate(VA), Err(MapError::CorruptTable));
}

// ---------------------------------------------------------------- 4. không rò rỉ frame

#[test]
fn unmap_frees_frames_no_leak() {
    let mut mmu = new_mmu();
    assert_eq!(mmu.memory().outstanding(), 1, "gốc");

    mmu.map(VA, PA, Perms::KERNEL_RW).expect("map");
    assert_eq!(mmu.memory().outstanding(), 4, "gốc + 3 bảng");

    assert_eq!(mmu.unmap(VA), Ok(PA), "unmap trả PA đã map");
    assert_eq!(
        mmu.memory().outstanding(),
        1,
        "unmap phải giải phóng đủ 3 bảng trung gian, chỉ còn gốc"
    );
    assert_eq!(mmu.memory().frees(), mmu.memory().allocs() - 1);
    assert_eq!(mmu.translate(VA), Err(MapError::NotMapped));

    let before = (mmu.memory().allocs(), mmu.memory().frees());
    assert_eq!(mmu.unmap(VA), Err(MapError::NotMapped));
    assert_eq!(
        (mmu.memory().allocs(), mmu.memory().frees()),
        before,
        "unmap lần hai không được free thêm"
    );
}

#[test]
fn unmap_512_pages_frees_all_tables() {
    let mut mmu = new_mmu();
    // 512 trang = 2 MiB, cùng một bảng lá.
    let va_base = 0x0000_0000_0040_0000u64;
    let pa_base = 0x0000_0000_8000_0000u64;
    for i in 0..512u64 {
        mmu.map(va_base + i * 0x1000, pa_base + i * 0x1000, Perms::KERNEL_RW)
            .expect("map trang");
    }
    assert_eq!(
        mmu.memory().outstanding(),
        4,
        "512 trang cùng 2 MiB chỉ cần gốc + 3 bảng"
    );
    for i in 0..512u64 {
        assert_eq!(mmu.unmap(va_base + i * 0x1000), Ok(pa_base + i * 0x1000));
    }
    assert_eq!(
        mmu.memory().outstanding(),
        1,
        "unmap hết phải giải phóng cả 3 bảng, chỉ còn gốc"
    );

    // Map lại ở vùng 2 MiB khác: cấp lại đúng 3 bảng.
    mmu.map(va_base + 0x20_0000, pa_base, Perms::KERNEL_RW)
        .expect("map vùng khác");
    assert_eq!(mmu.memory().outstanding(), 4);
}

#[test]
fn frame_accounting_alloc_eq_free_after_unmap() {
    for n in 1..=64u64 {
        let mut mmu = new_mmu();
        let va_base = 0x0000_0000_0010_0000u64;
        let pa_base = 0x0000_0000_8000_0000u64;
        for i in 0..n {
            mmu.map(va_base + i * 0x1000, pa_base + i * 0x1000, Perms::KERNEL_RW)
                .expect("map");
        }
        for i in 0..n {
            mmu.unmap(va_base + i * 0x1000).expect("unmap");
        }
        assert_eq!(
            mmu.memory().outstanding(),
            1,
            "n={n}: sau unmap hết chỉ còn frame gốc"
        );
        assert_eq!(
            mmu.memory().frees(),
            mmu.memory().allocs() - 1,
            "n={n}: số frame giải phóng = số frame cấp phát − gốc"
        );
        let memory = mmu.destroy().expect("destroy");
        assert_eq!(
            memory.allocs(),
            memory.frees(),
            "n={n}: destroy phải giải phóng nốt gốc — alloc = free"
        );
        assert_eq!(memory.outstanding(), 0);
    }
}

#[test]
fn destroy_frees_everything() {
    let mut mmu = new_mmu();
    mmu.map(VA, PA, Perms::KERNEL_RW).expect("map 1");
    mmu.map(0x0000_0000_0040_0000, PA, Perms::KERNEL_RW)
        .expect("map 2");
    mmu.map(0xFFFF_FFC0_0000_0000, PA, Perms::KERNEL_RX)
        .expect("map 3");
    assert!(mmu.memory().outstanding() > 1);
    let memory = mmu.destroy().expect("destroy");
    assert_eq!(memory.outstanding(), 0, "destroy phải giải phóng cả cây");
    assert_eq!(memory.allocs(), memory.frees());
}

#[test]
fn stale_frame_is_zeroed() {
    let mut mmu = new_mmu();
    mmu.map(VA, PA, Perms::KERNEL_RW).expect("map");
    mmu.unmap(VA).expect("unmap");
    // Ba frame vừa free đầy rác GARBAGE; map lại buộc phải zero trước khi dùng.
    let va2 = 0x0000_0000_0000_2000u64;
    mmu.map(va2, PA + 0x1000, Perms::KERNEL_RW)
        .expect("map lại trên frame rác");
    assert_eq!(mmu.translate(va2).unwrap().pa, PA + 0x1000);
    assert_eq!(mmu.translate(VA), Err(MapError::NotMapped));
}

#[test]
fn out_of_frames() {
    let mut memory = TestMemory::new(FRAME_BASE);
    memory.fail_after = Some(1); // chỉ đủ frame gốc
    let mut mmu = PageTable::<Aarch64, _>::new(memory).expect("gốc");
    assert_eq!(
        mmu.map(VA, PA, Perms::KERNEL_RW),
        Err(MapError::OutOfFrames),
        "hết frame cho bảng trung gian"
    );
    assert_eq!(mmu.memory().outstanding(), 1, "không rò rỉ frame khi hết");
}

// ---------------------------------------------------------------- 5. format không hardcode

#[test]
fn mmu_generic_over_second_format() {
    // Format 3 mức 4 KiB, 30-bit VA, descriptor tự chế — chứng minh `Mmu` không phụ thuộc AArch64.
    struct Toy3;
    impl PageTableFormat for Toy3 {
        const LEVELS: usize = 3;
        const PAGE_BITS: u32 = 12;
        const INDEX_BITS: u32 = 9;
        const VA_BITS: u32 = 30;
        const PA_BITS: u32 = 32;
        const PAGE_BYTES: u64 = 4096;
        const ENTRIES: usize = 512;

        fn index(va: u64, level: usize) -> usize {
            ((va >> (12 + 9 * (2 - level))) & 0x1FF) as usize
        }
        fn is_canonical(va: u64) -> bool {
            va >> 30 == 0
        }
        fn encode_table(pa: u64) -> u64 {
            (pa & !0xFFF) | 1
        }
        fn encode_leaf(pa: u64, perms: Perms, _attr: MemAttr, _share: Shareability) -> u64 {
            let mut d = (pa & !0xFFF) | 1 | 2;
            if perms.write {
                d |= 4;
            }
            if perms.exec {
                d |= 8;
            }
            if perms.user {
                d |= 16;
            }
            d
        }
        fn decode(entry: u64, level: usize) -> Descriptor {
            if entry & 1 == 0 {
                return Descriptor::Invalid;
            }
            let pa = entry & !0xFFF;
            if level + 1 == 3 {
                Descriptor::Leaf {
                    pa,
                    perms: Perms {
                        read: true,
                        write: entry & 4 != 0,
                        exec: entry & 8 != 0,
                        user: entry & 16 != 0,
                    },
                }
            } else if entry & 2 != 0 {
                Descriptor::Reserved
            } else {
                Descriptor::Table { pa }
            }
        }
    }

    fn map_three<M: Mmu>(mmu: &mut M, base: u64) -> Result<(), MapError> {
        for i in 0..3u64 {
            mmu.map(
                base + i * 0x1000,
                0x8000_0000 + i * 0x1000,
                Perms::KERNEL_RW,
            )?;
        }
        Ok(())
    }

    let mut mmu = PageTable::<Toy3, _>::new(TestMemory::new(FRAME_BASE)).expect("gốc");
    map_three(&mut mmu, 0x1000).expect("map qua trait Mmu");
    assert_eq!(mmu.translate(0x2000).unwrap().pa, 0x8000_1000);
    assert_eq!(mmu.memory().outstanding(), 3, "gốc + 2 bảng của 3 mức");
    for i in 0..3u64 {
        mmu.unmap(0x1000 + i * 0x1000).expect("unmap");
    }
    assert_eq!(mmu.memory().outstanding(), 1);
    let memory = mmu.destroy().expect("destroy");
    assert_eq!(memory.outstanding(), 0);
}

// ---------------------------------------------------------------- 6. walk chẩn đoán

#[test]
fn walk_reports_invalid_and_canonical() {
    let mmu = new_mmu();
    let walk = mmu.walk(0x0000_0080_0000_0000);
    assert_eq!(walk.len, 0, "VA không canonical không đọc bộ nhớ");
    assert_eq!(walk.outcome, WalkOutcome::Invalid);

    let walk = mmu.walk(VA);
    assert_eq!(walk.len, 1, "dừng ngay entry gốc trống");
    assert_eq!(walk.outcome, WalkOutcome::Invalid);
    assert_eq!(walk.step(0).unwrap().index, 0);
    assert_eq!(walk.step(1), None);
}

#[test]
fn translation_is_copy_and_eq() {
    let t = Translation {
        pa: PA,
        perms: Perms::KERNEL_RW,
        level: 3,
    };
    let copy = t;
    assert_eq!(t, copy);
    assert_eq!(AARCH64_VA_BITS, 39);
    assert_eq!(AARCH64_PA_BITS, 48);
    assert_eq!(AARCH64_PAGE_BYTES, 4096);
    assert_eq!(AARCH64_ENTRIES, 512);
    // `span`: vùng VA một entry phủ ở từng mức.
    assert_eq!(Aarch64::span(3), 4096);
    assert_eq!(Aarch64::span(2), 2 << 20);
    assert_eq!(Aarch64::span(1), 1 << 30);
    assert_eq!(Aarch64::span(0), 1 << 39);
    assert_eq!(Aarch64::span(4), 0, "mức ngoài dải ⇒ 0");
}
