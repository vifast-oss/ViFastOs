// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test host cho **block descriptor** (superpage) của `vifast-plat-aarch64` — KT-302 + KT-305.
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64 --test block
//! ```
//!
//! - **KT-302**: map ECAM 256 MiB bằng block 2 MiB ở mức 2 (128 block ⇒ 2 frame), kiểm căng
//!   lề/cấp/chồng và format không hỗ trợ block.
//! - **KT-305**: `translate` **hiểu** block (PA = đáy block + offset, `level` của block, quyền
//!   giải mã từ descriptor) và `unmap` **chia nhỏ** block (512 lá 4 KiB) thay vì xoá cả block —
//!   trang kế cận **giữ nguyên** mapping. Format không khai `block_span` (x86_64/Sv39) vẫn từ
//!   chối `BlockUnsupported` như trước.
//!
//! Nguồn frame + hằng số ở `common/mod.rs` (một nguồn cho cả `host.rs` và tệp này).

mod common;

use common::*;
use vifast_plat_aarch64::format::{
    is_canonical_bits, Aarch64, Descriptor, MemAttr, PageTableFormat, Perms, Shareability,
    AARCH64_LEVELS, DESC_ADDR_MASK, DESC_ATTR_SHIFT, DESC_VALID,
};
use vifast_plat_aarch64::mmu::{MapError, PageTable, TableMemory, WalkOutcome};

// ---------------------------------------------------------------- 4b. block descriptor (KT-302)

/// **KT-302**: map ECAM 256 MiB bằng **block 2 MiB** ở mức 2 — 128 block trong **một** bảng mức 2
/// ⇒ chỉ tốn **2 frame** (bảng mức 1 + bảng mức 2), không phải 129 bảng mức 3.
#[test]
fn map_block_2m_covers_256mib_with_two_frames() {
    const BLOCK: u64 = 2 << 20;
    const VA_BASE: u64 = 0x0000_0000_8000_0000; // ECAM_VA_BASE
    const PA_BASE: u64 = 0x0000_0040_1000_0000; // ECAM_BASE (0x40_1000_0000)
    let mut mmu = new_mmu();
    let before = mmu.memory().outstanding();
    let mut i = 0u64;
    while i < 128 {
        mmu.map_block(
            VA_BASE + i * BLOCK,
            PA_BASE + i * BLOCK,
            2,
            Perms::KERNEL_RW,
            MemAttr::Device,
            Shareability::Inner,
        )
        .expect("map block 2 MiB");
        i += 1;
    }
    // Chỉ **2** frame mới: bảng mức 1 (index 2 của gốc) + bảng mức 2 (128 block).
    assert_eq!(
        mmu.memory().outstanding() - before,
        2,
        "128 block 2 MiB chỉ tốn 2 frame (không phải 129 bảng mức 3)"
    );
    // Walk **thấy** block ở mức 2 với PA đúng — bằng chứng bảng trang thật sự dùng block.
    assert_eq!(
        mmu.walk(VA_BASE + 3 * BLOCK).outcome,
        WalkOutcome::Block {
            pa: PA_BASE + 3 * BLOCK,
            level: 2
        }
    );
    // **KT-305**: `translate` **hiểu** block — trả PA đáy block + offset và `level = 2` (trước
    // KT-305 hai dòng này đòi `BlockUnsupported`; hành vi một chiều ấy đã bị đảo có chủ đích).
    let t = mmu.translate(VA_BASE + 3 * BLOCK).expect("translate block");
    assert_eq!(t.pa, PA_BASE + 3 * BLOCK);
    assert_eq!(t.level, 2);
    // Map 4 KiB vào trong block ⇒ `AlreadyMapped` (không ghi đè block).
    assert_eq!(
        mmu.map(VA_BASE + 0x1000, PA, Perms::KERNEL_RW),
        Err(MapError::AlreadyMapped)
    );
}

#[test]
fn map_block_unaligned_rejected_without_allocating() {
    let mut mmu = new_mmu();
    let before = mmu.memory().outstanding();
    // VA lệch 4 KiB (không căn 2 MiB).
    assert_eq!(
        mmu.map_block(
            0x8000_0000 + 0x1000,
            0x40_1000_0000,
            2,
            Perms::KERNEL_RW,
            MemAttr::Device,
            Shareability::Inner
        ),
        Err(MapError::Unaligned)
    );
    // PA lệch 4 KiB.
    assert_eq!(
        mmu.map_block(
            0x8000_0000,
            0x40_1000_0000 + 0x1000,
            2,
            Perms::KERNEL_RW,
            MemAttr::Device,
            Shareability::Inner
        ),
        Err(MapError::Unaligned)
    );
    // Block 1 GiB ở mức 1: VA/PA phải căn 1 GiB — PA lệch 2 MiB ⇒ Unaligned.
    assert_eq!(
        mmu.map_block(
            0x8000_0000,
            0x4000_0000 + 0x20_0000,
            1,
            Perms::KERNEL_RW,
            MemAttr::Device,
            Shareability::Inner
        ),
        Err(MapError::Unaligned)
    );
    assert_eq!(
        mmu.memory().outstanding(),
        before,
        "từ chối trước khi cấp frame"
    );
}

#[test]
fn map_block_level_out_of_range_rejected() {
    let mut mmu = new_mmu();
    let before = mmu.memory().outstanding();
    // Mức lá (3) không có block; mức 0 (gốc) cũng không.
    for level in [0usize, 3] {
        assert_eq!(
            mmu.map_block(
                0x8000_0000,
                0x40_1000_0000,
                level,
                Perms::KERNEL_RW,
                MemAttr::Device,
                Shareability::Inner
            ),
            Err(MapError::BlockUnsupported),
            "mức {level} không có block descriptor"
        );
    }
    // Mức ngoài `0..LEVELS` là lỗi cấp, không phải "format không hỗ trợ".
    assert_eq!(
        mmu.map_block(
            0x8000_0000,
            0x40_1000_0000,
            4,
            Perms::KERNEL_RW,
            MemAttr::Device,
            Shareability::Inner
        ),
        Err(MapError::LevelOutOfRange)
    );
    assert_eq!(mmu.memory().outstanding(), before);
}

#[test]
fn map_block_overlap_rejected() {
    const BLOCK: u64 = 2 << 20;
    let mut mmu = new_mmu();
    // Lá 4 KiB đã có ⇒ block phủ lên phải bị từ chối.
    mmu.map(0x8000_0000, PA, Perms::KERNEL_RW).expect("leaf");
    assert_eq!(
        mmu.map_block(
            0x8000_0000,
            0x40_1000_0000,
            2,
            Perms::KERNEL_RW,
            MemAttr::Device,
            Shareability::Inner
        ),
        Err(MapError::AlreadyMapped)
    );
    // Block đã có ⇒ map lại cùng VA cũng bị từ chối.
    let mut mmu = new_mmu();
    mmu.map_block(
        0x8000_0000,
        0x40_1000_0000,
        2,
        Perms::KERNEL_RW,
        MemAttr::Device,
        Shareability::Inner,
    )
    .expect("block");
    assert_eq!(
        mmu.map_block(
            0x8000_0000,
            0x40_1000_0000 + BLOCK,
            2,
            Perms::KERNEL_RW,
            MemAttr::Device,
            Shareability::Inner
        ),
        Err(MapError::AlreadyMapped)
    );
}

#[test]
fn map_block_unsupported_format_rejected() {
    // Format **không** cài `encode_block` ⇒ mặc định `None` ⇒ `BlockUnsupported`, **trước** khi
    // cấp frame nào (kiểm tra năng lực format chạy trước bước walk).
    struct NoBlock3;
    impl PageTableFormat for NoBlock3 {
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
        fn encode_leaf(pa: u64, _perms: Perms, _attr: MemAttr, _share: Shareability) -> u64 {
            (pa & !0xFFF) | 1 | 2
        }
        fn decode(entry: u64, level: usize) -> Descriptor {
            if entry & 1 == 0 {
                return Descriptor::Invalid;
            }
            let pa = entry & !0xFFF;
            if level + 1 == 3 {
                Descriptor::Leaf {
                    pa,
                    perms: Perms::KERNEL_RW,
                }
            } else if entry & 2 != 0 {
                Descriptor::Reserved
            } else {
                Descriptor::Table { pa }
            }
        }
    }

    let mut mmu = PageTable::<NoBlock3, _>::new(TestMemory::new(FRAME_BASE)).expect("gốc");
    let before = mmu.memory().outstanding();
    assert_eq!(
        mmu.map_block(
            0,
            0,
            1,
            Perms::KERNEL_RW,
            MemAttr::Normal,
            Shareability::Inner
        ),
        Err(MapError::BlockUnsupported)
    );
    assert_eq!(mmu.memory().outstanding(), before, "không cấp frame");
}

// ---------------------------------------------------------------- 4c. block: translate/unmap (KT-305)
//
// KT-302 map ECAM 256 MiB bằng **block 2 MiB** nhưng `translate`/`unmap` vẫn trả
// `BlockUnsupported` ⇒ mapping **một chiều**. KT-305 đóng nửa còn lại: `translate` trả PA **đã
// cộng offset trong block** + `level` của block; `unmap` **chia nhỏ** block thành 512 lá 4 KiB
// rồi bỏ **một** lá (không xoá cả block — xoá cả block là mất mapping kế cận).

/// **KT-305**: `translate` **hiểu** block mức 2 (2 MiB) — trả PA đáy block **+ offset trong
/// block**, `level = 2`, quyền giải mã từ descriptor. Trước KT-305: `BlockUnsupported`.
#[test]
fn translate_block_level2_returns_pa_with_offset() {
    const BLOCK: u64 = 2 << 20;
    const VA_BASE: u64 = 0x0000_0000_8000_0000; // ECAM_VA_BASE
    const PA_BASE: u64 = 0x0000_0040_1000_0000; // ECAM_BASE
    let mut mmu = new_mmu();
    mmu.map_block(
        VA_BASE,
        PA_BASE,
        2,
        Perms::KERNEL_RW,
        MemAttr::Device,
        Shareability::Inner,
    )
    .expect("map block 2 MiB");
    for offset in [0u64, 0x1000, BLOCK - 0x1000] {
        let t = mmu
            .translate(VA_BASE + offset)
            .expect("translate trong block 2 MiB");
        assert_eq!(
            t.pa,
            PA_BASE + offset,
            "PA = đáy block + offset trong block"
        );
        assert_eq!(t.level, 2, "level = mức của block (2 MiB ở mức 2)");
        assert_eq!(t.perms, Perms::KERNEL_RW, "quyền giải mã từ descriptor");
    }
    // Block kế tiếp **chưa** map ⇒ NotMapped (không trả PA của block bên cạnh).
    assert_eq!(mmu.translate(VA_BASE + BLOCK), Err(MapError::NotMapped));
}

/// **KT-305**: `translate` hiểu cả block mức 1 (1 GiB) — `level = 1`, quyền RX giữ nguyên.
#[test]
fn translate_block_level1_returns_pa_with_offset() {
    const GIB: u64 = 1 << 30;
    const VA_BASE: u64 = 0x0000_0000_4000_0000; // căn 1 GiB
    const PA_BASE: u64 = 0x0000_0000_4000_0000;
    let mut mmu = new_mmu();
    mmu.map_block(
        VA_BASE,
        PA_BASE,
        1,
        Perms::KERNEL_RX,
        MemAttr::Normal,
        Shareability::Inner,
    )
    .expect("map block 1 GiB");
    let t = mmu
        .translate(VA_BASE + 0x1234_5000)
        .expect("translate trong block 1 GiB");
    assert_eq!(t.pa, PA_BASE + 0x1234_5000);
    assert_eq!(t.level, 1, "level = mức của block (1 GiB ở mức 1)");
    assert_eq!(t.perms, Perms::KERNEL_RX, "quyền của block, không bịa");
    assert_eq!(mmu.translate(VA_BASE + GIB), Err(MapError::NotMapped));
}

/// **KT-305**: `unmap` một trang **trong** block phải **chia nhỏ** block thành 512 lá 4 KiB rồi
/// bỏ **một** lá — trang kế cận **vẫn** map. Đây là ca mà "xoá cả block" làm ĐỎ.
#[test]
fn unmap_inside_block_splits_and_keeps_neighbours() {
    const BLOCK: u64 = 2 << 20;
    const VA_BASE: u64 = 0x0000_0000_8000_0000;
    const PA_BASE: u64 = 0x0000_0040_1000_0000;
    let mut mmu = new_mmu();
    mmu.map_block(
        VA_BASE,
        PA_BASE,
        2,
        Perms::KERNEL_RW,
        MemAttr::Device,
        Shareability::Inner,
    )
    .expect("map block");
    let before = mmu.memory().outstanding();
    assert_eq!(
        mmu.unmap(VA_BASE + 0x1000),
        Ok(PA_BASE + 0x1000),
        "unmap trả PA của trang trong block"
    );
    assert_eq!(
        mmu.translate(VA_BASE + 0x1000),
        Err(MapError::NotMapped),
        "trang vừa bỏ phải NotMapped"
    );
    // Ba trang kế cận **vẫn** map — nếu `unmap` xoá cả block thì cả ba NotMapped. Kiểm **trước**
    // phép đếm frame để đột biến (i) của KT-305 đỏ đúng lý do "mất mapping kế cận".
    for offset in [0u64, 0x2000, BLOCK - 0x1000] {
        assert_eq!(
            mmu.translate(VA_BASE + offset)
                .expect("trang kế cận còn map")
                .pa,
            PA_BASE + offset,
            "trang kế cận offset {offset:#x} mất mapping"
        );
    }
    assert_eq!(
        mmu.memory().outstanding(),
        before + 1,
        "chia nhỏ cần **đúng một** bảng mức 3"
    );
    // Lá vừa bỏ map lại được (đúng đường ảnh dùng để tự kiểm rồi khôi phục ECAM).
    mmu.map(VA_BASE + 0x1000, PA_BASE + 0x1000, Perms::KERNEL_RW)
        .expect("map lại lá đã bỏ");
    assert_eq!(
        mmu.translate(VA_BASE + 0x1000).unwrap().pa,
        PA_BASE + 0x1000
    );
}

/// **KT-305**: unmap **cả** block (512 lá một lượt) ⇒ bảng chia nhỏ + bảng mức 2 + mức 1 đều
/// được giải phóng, chỉ còn gốc — không rò rỉ frame.
#[test]
fn unmap_all_pages_of_block_frees_split_table() {
    const BLOCK: u64 = 2 << 20;
    const VA_BASE: u64 = 0x0000_0000_8000_0000;
    const PA_BASE: u64 = 0x0000_0040_1000_0000;
    let mut mmu = new_mmu();
    mmu.map_block(
        VA_BASE,
        PA_BASE,
        2,
        Perms::KERNEL_RW,
        MemAttr::Device,
        Shareability::Inner,
    )
    .expect("map block");
    for i in 0..512u64 {
        assert_eq!(
            mmu.unmap(VA_BASE + i * 0x1000),
            Ok(PA_BASE + i * 0x1000),
            "unmap lá {i}"
        );
    }
    assert_eq!(
        mmu.memory().outstanding(),
        1,
        "unmap hết 512 lá ⇒ giải phóng bảng chia nhỏ + bảng mức 2 + mức 1"
    );
    assert_eq!(mmu.memory().frees(), mmu.memory().allocs() - 1);
    assert_eq!(mmu.translate(VA_BASE), Err(MapError::NotMapped));
    assert_eq!(
        mmu.translate(VA_BASE + BLOCK - 0x1000),
        Err(MapError::NotMapped)
    );
}

/// **KT-305**: block 1 GiB bị chia **hai** mức (1 GiB → 512 block 2 MiB → 512 lá) khi unmap một
/// trang; chỉ nhánh chứa VA bị chia, các block 2 MiB khác **giữ nguyên** ở mức 2.
#[test]
fn unmap_level1_block_splits_twice() {
    const GIB: u64 = 1 << 30;
    const VA_BASE: u64 = 0x0000_0000_4000_0000;
    const PA_BASE: u64 = 0x0000_0000_4000_0000;
    let mut mmu = new_mmu();
    mmu.map_block(
        VA_BASE,
        PA_BASE,
        1,
        Perms::KERNEL_RW,
        MemAttr::Normal,
        Shareability::Inner,
    )
    .expect("map block 1 GiB");
    let before = mmu.memory().outstanding();
    assert_eq!(mmu.unmap(VA_BASE + 0x1000), Ok(PA_BASE + 0x1000));
    assert_eq!(
        mmu.memory().outstanding(),
        before + 2,
        "1 GiB → 512 block 2 MiB → 512 lá: **hai** bảng"
    );
    assert_eq!(mmu.translate(VA_BASE + 0x1000), Err(MapError::NotMapped));
    assert_eq!(mmu.translate(VA_BASE).unwrap().pa, PA_BASE);
    let next = mmu
        .translate(VA_BASE + 0x20_0000)
        .expect("block 2 MiB kế tiếp");
    assert_eq!(next.pa, PA_BASE + 0x20_0000);
    assert_eq!(next.level, 2, "block 2 MiB kế tiếp **vẫn** ở mức 2");
    assert_eq!(
        mmu.translate(VA_BASE + GIB - 0x1000).unwrap().pa,
        PA_BASE + GIB - 0x1000,
        "block 2 MiB cuối dải vẫn dịch được"
    );
}

/// **KT-305**: chia nhỏ cần **một** frame; hết frame ⇒ `OutOfFrames` và block **còn nguyên**
/// (không mất mapping nào — fail-closed).
#[test]
fn unmap_block_split_out_of_frames_keeps_block() {
    const VA_BASE: u64 = 0x0000_0000_8000_0000;
    const PA_BASE: u64 = 0x0000_0040_1000_0000;
    let mut mmu = new_mmu();
    mmu.map_block(
        VA_BASE,
        PA_BASE,
        2,
        Perms::KERNEL_RW,
        MemAttr::Device,
        Shareability::Inner,
    )
    .expect("map block");
    let allocs = mmu.memory().allocs();
    mmu.memory_mut().fail_after = Some(allocs);
    assert_eq!(
        mmu.unmap(VA_BASE + 0x1000),
        Err(MapError::OutOfFrames),
        "hết frame khi chia nhỏ"
    );
    assert_eq!(
        mmu.translate(VA_BASE + 0x1000).unwrap().pa,
        PA_BASE + 0x1000,
        "block phải còn nguyên sau khi chia nhỏ thất bại"
    );
    assert_eq!(mmu.translate(VA_BASE).unwrap().level, 2);
}

/// **KT-305**: `block_span` là **một** nguồn hình học — phải khớp `encode_block` và `span(level)`
/// ở **mọi** mức (lệch là `map_block` nhận cỡ này còn `translate`/`unmap` hiểu cỡ khác).
#[test]
fn block_span_matches_encode_block() {
    for level in 0..AARCH64_LEVELS {
        let span = Aarch64::block_span(level);
        let encoded = Aarch64::encode_block(
            level,
            0,
            Perms::KERNEL_RW,
            MemAttr::Normal,
            Shareability::Inner,
        );
        assert_eq!(
            span.is_some(),
            encoded.is_some(),
            "mức {level}: block_span và encode_block phải cùng năng lực"
        );
        if let Some(span) = span {
            assert_eq!(
                span,
                Aarch64::span(level),
                "mức {level}: cỡ block = span(level)"
            );
            assert_eq!(
                span,
                1u64 << (12 + 9 * (3 - level)),
                "mức {level}: cỡ block theo hình học 4 KiB/9 bit"
            );
        }
    }
    assert_eq!(Aarch64::block_span(1), Some(1 << 30), "mức 1 = 1 GiB");
    assert_eq!(Aarch64::block_span(2), Some(2 << 20), "mức 2 = 2 MiB");
    assert_eq!(Aarch64::block_span(0), None, "mức gốc không có block");
    assert_eq!(Aarch64::block_span(3), None, "mức lá không có block");
}

/// **KT-305**: entry con do `split_block` sinh phải **giữ quyền/thuộc tính** của block và **PA
/// đúng** (`đáy block + index × cỡ con`); mức 1 → block mức 2, mức 2 → lá mức 3.
#[test]
fn split_block_entries_keep_attrs_and_pa() {
    const PA_BASE: u64 = 0x0000_0040_1000_0000;
    let block = Aarch64::encode_block(
        2,
        PA_BASE,
        Perms::KERNEL_RW,
        MemAttr::Device,
        Shareability::Inner,
    )
    .expect("encode block 2 MiB");
    for index in [0usize, 1, 511] {
        let leaf = Aarch64::split_block(block, 2, index).expect("chia block 2 MiB");
        assert_eq!(leaf & 0b11, 0b11, "mức lá: page descriptor");
        assert_eq!(
            leaf & DESC_ADDR_MASK,
            PA_BASE + (index as u64) * 0x1000,
            "PA con = đáy block + index × 4 KiB"
        );
        assert_eq!(
            Aarch64::decode(leaf, 3),
            Descriptor::Leaf {
                pa: PA_BASE + (index as u64) * 0x1000,
                perms: Perms::KERNEL_RW
            },
            "quyền của block được giữ nguyên"
        );
        assert_eq!(
            (leaf >> DESC_ATTR_SHIFT) & 0b111,
            MemAttr::Device.index(),
            "AttrIndx (Device) được giữ nguyên"
        );
    }
    let gib = Aarch64::encode_block(
        1,
        0x0000_0000_4000_0000,
        Perms::KERNEL_RW,
        MemAttr::Normal,
        Shareability::Inner,
    )
    .expect("encode block 1 GiB");
    let child = Aarch64::split_block(gib, 1, 3).expect("chia block 1 GiB");
    assert_eq!(child & 0b11, 0b01, "mức 2: block descriptor");
    assert_eq!(
        Aarch64::decode(child, 2),
        Descriptor::Block {
            pa: 0x0000_0000_4000_0000 + 3 * (2 << 20),
            level: 2,
            perms: Perms::KERNEL_RW
        }
    );
    // Mức gốc / mức lá không có block ⇒ không chia được.
    assert_eq!(Aarch64::split_block(block, 0, 0), None);
    assert_eq!(Aarch64::split_block(block, 3, 0), None);
}

/// Format **giải mã** block nhưng **không** khai năng lực (`block_span` mặc định `None`) — chốt
/// rằng `translate`/`unmap` vẫn trả `BlockUnsupported` cho format chỉ map lá. Đây là hành vi cũ
/// của **x86_64** và **Sv39** sau KT-305 (chúng không phải sửa một dòng nào).
struct BlockDecodedNoSupport4;

impl PageTableFormat for BlockDecodedNoSupport4 {
    const LEVELS: usize = 4;
    const PAGE_BITS: u32 = 12;
    const INDEX_BITS: u32 = 9;
    const VA_BITS: u32 = 39;
    const PA_BITS: u32 = 48;
    const PAGE_BYTES: u64 = 4096;
    const ENTRIES: usize = 512;

    fn index(va: u64, level: usize) -> usize {
        ((va >> (12 + 9 * (3 - level))) & 0x1FF) as usize
    }
    fn is_canonical(va: u64) -> bool {
        is_canonical_bits(va, 39)
    }
    fn encode_table(pa: u64) -> u64 {
        (pa & !0xFFF) | 0b11
    }
    fn encode_leaf(pa: u64, _perms: Perms, _attr: MemAttr, _share: Shareability) -> u64 {
        (pa & !0xFFF) | 0b11
    }
    fn decode(entry: u64, level: usize) -> Descriptor {
        if entry & 1 == 0 {
            return Descriptor::Invalid;
        }
        let pa = entry & !0xFFF;
        if level + 1 == 4 {
            Descriptor::Leaf {
                pa,
                perms: Perms::KERNEL_RW,
            }
        } else if entry & 0b11 == 0b01 {
            Descriptor::Block {
                pa,
                level,
                perms: Perms::KERNEL_RW,
            }
        } else {
            Descriptor::Table { pa }
        }
    }
}

#[test]
fn block_translate_unmap_unsupported_format_rejected() {
    let mut mmu =
        PageTable::<BlockDecodedNoSupport4, _>::new(TestMemory::new(FRAME_BASE)).expect("gốc");
    mmu.map(VA, PA, Perms::KERNEL_RW)
        .expect("map lá dựng đường");
    let l2 = mmu.walk(VA).step(2).unwrap().table;
    let block = 0x4000_0000u64 | 0b01; // block 2 MiB ở mức 2
    assert!(mmu
        .memory_mut()
        .write_entry(l2, BlockDecodedNoSupport4::index(VA, 2), block));
    assert_eq!(
        mmu.translate(VA),
        Err(MapError::BlockUnsupported),
        "format không khai block_span ⇒ translate từ chối như trước KT-305"
    );
    assert_eq!(
        mmu.unmap(VA),
        Err(MapError::BlockUnsupported),
        "format không khai block_span ⇒ unmap từ chối như trước KT-305"
    );
}

#[test]
fn block_descriptor_translate_and_unmap() {
    // Block 1 GiB hợp lệ ở mức 1: kind = 0b01 (bit1 = 0, bit0 = 1). Entry **tự chế** (không qua
    // `map_block`) để chốt rằng `translate`/`unmap` đọc **descriptor thật**, không chỉ đường map.
    let block = DESC_VALID | (0x4000_0000 & DESC_ADDR_MASK);
    assert_eq!(block & 0b11, 0b01);
    let mut mmu = new_mmu();
    mmu.map(VA, PA, Perms::KERNEL_RW).expect("map");
    let l1 = mmu.walk(VA).step(1).unwrap().table;
    assert!(mmu.memory_mut().write_entry(l1, 0, block));
    // **KT-305** — test này trước đây chốt `BlockUnsupported` (hành vi một chiều của KT-302).
    // Nay `translate` **hiểu** block: PA = đáy block + offset trong block, `level = 1`, quyền
    // giải mã từ descriptor (entry tự chế không đặt `AP[2]`/`UXN`/`PXN` ⇒ RWX kernel).
    let t = mmu.translate(VA).expect("translate block 1 GiB");
    assert_eq!(t.pa, 0x4000_0000 + (VA & ((1 << 30) - 1)));
    assert_eq!(t.level, 1);
    assert_eq!(t.perms, Perms::KERNEL_RWX);
    assert_eq!(
        mmu.map(VA + 0x1000, PA, Perms::KERNEL_RW),
        Err(MapError::AlreadyMapped),
        "VA nằm trong block ⇒ đã map"
    );
    // `unmap` **chia nhỏ** block (1 GiB → 512 block 2 MiB → 512 lá) rồi bỏ **một** lá; trang kế
    // cận **vẫn** map (xoá cả block là mất mapping của 511 trang).
    let before = mmu.memory().outstanding();
    assert_eq!(mmu.unmap(VA), Ok(0x4000_0000 + (VA & ((1 << 30) - 1))));
    assert_eq!(
        mmu.memory().outstanding(),
        before + 2,
        "chia 1 GiB xuống lá cần hai bảng"
    );
    assert_eq!(mmu.translate(VA), Err(MapError::NotMapped));
    assert_eq!(
        mmu.translate(VA + 0x1000).unwrap().pa,
        0x4000_0000 + ((VA + 0x1000) & ((1 << 30) - 1))
    );

    // Block ở mức 0 là **reserved** theo đặc tả ⇒ CorruptTable, không lặng lẽ nhận.
    let mut mmu = new_mmu();
    let root = mmu.root();
    assert!(mmu.memory_mut().write_entry(root, 0, block));
    assert_eq!(mmu.translate(VA), Err(MapError::CorruptTable));
}
