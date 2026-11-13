// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-35 — test tích hợp của `vifast-pte` (K2-PLAN §3, DoD KT-35; D2-65 Q1/Q2/Q8).
//!
//! Phạm vi **chỉ** hình/layout: `FrameSize` ↔ `size_bits`, `Pte`, `PteArray`, `walk_shape`
//! cho VA 39 bit. **Không** có PageTable walker/map thật (KT-37).
//!
//! Test biên bắt buộc theo KT-35: `size_bits` lạ ⇒ `InvalidArgument`; `va = 1 << 38` cắt đúng
//! 3 chỉ số; `PteArray` đúng cỡ/không panic; `walk_shape` khớp 3 chỉ số; property 10.000 VA.
//! Tệp dùng `std` của **crate test** (crate sản phẩm vẫn `#![no_std]` — D2-42).
//!
//! Miri (`./v2/bin/v2 miri -p vifast-pte`): property giảm còn 256 vòng bằng `cfg!(miri)`; số
//! vòng 10.000 của test thường **không** đổi (D2-47(a): Miri ngoài `gate --fast`).

use vifast_pte::{
    walk_shape, FrameSize, Pte, PteArray, PteError, WalkShape, ASID_BITS, MAX_FRAMES_PER_VSPACE,
    MAX_MAP_DEPTH, PAGE_SIZE_BITS, PTE_LEVELS, TLB_ENTRIES_PER_HART, VA_BITS,
};

/// Số vòng property của test thường; Miri chậm hơn nhiều nên giảm (tiền lệ `cfg(miri)` của K1c).
const PROPERTY_ITERS: usize = if cfg!(miri) { 256 } else { 10_000 };

/// LCG 64 bit cố định — tất định, không dependency (D2-21), cùng dãy trên mọi lần chạy.
fn next_lcg(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *state
}

// ---------------------------------------------------------------- hằng số Q1

#[test]
fn hang_so_q1_dung_gia_tri_chot() {
    assert_eq!(
        PAGE_SIZE_BITS,
        [12, 21, 30],
        "cỡ trang: 4 KiB / 2 MiB / 1 GiB"
    );
    assert_eq!(PAGE_SIZE_BITS.len(), PTE_LEVELS, "một cỡ cho mỗi cấp lá");
    assert_eq!(PTE_LEVELS, 3, "Q2: ba cấp kiểu Sv39");
    assert_eq!(VA_BITS, 39, "Q2: VA 39 bit");
    assert_eq!(ASID_BITS, 8, "Q4: 256 ASID mỗi thế hệ");
    assert_eq!(1u32 << ASID_BITS, 256, "ASID 8 bit ⇒ đúng 256");
    assert_eq!(TLB_ENTRIES_PER_HART, 16, "Q4: 16 entry TLB mỗi hart");
    assert_eq!(MAX_MAP_DEPTH, PTE_LEVELS, "độ sâu tối đa = số cấp");
    assert_eq!(MAX_FRAMES_PER_VSPACE, 4096, "biên số Frame mỗi VSpace [TK]");
    assert!(
        MAX_FRAMES_PER_VSPACE.is_power_of_two(),
        "biên phải là luỹ thừa 2 để chia được bảng 512 entry"
    );
}

// ---------------------------------------------------------------- FrameSize

#[test]
fn frame_size_bits_va_byte_roundtrip() {
    assert_eq!(
        FrameSize::ALL,
        [FrameSize::K4, FrameSize::M2, FrameSize::G1],
        "thứ tự ALL khớp PAGE_SIZE_BITS"
    );
    for (i, size) in FrameSize::ALL.iter().enumerate() {
        assert_eq!(size.size_bits(), PAGE_SIZE_BITS[i]);
        assert_eq!(
            FrameSize::from_size_bits(size.size_bits()),
            Ok(*size),
            "size_bits -> FrameSize -> size_bits phải khép kín"
        );
        assert_eq!(size.size_bytes(), 1u64 << size.size_bits());
        assert_eq!(FrameSize::try_from(size.size_bits()), Ok(*size));
    }
    assert_eq!(FrameSize::K4.size_bytes(), 4 * 1024);
    assert_eq!(FrameSize::M2.size_bytes(), 2 * 1024 * 1024);
    assert_eq!(FrameSize::G1.size_bytes(), 1024 * 1024 * 1024);
    assert_eq!(
        PteArray::<512>::new().byte_size(),
        FrameSize::K4.size_bytes() as usize
    );
}

#[test]
fn size_bits_la_tra_invalid_argument() {
    for bits in 0u8..=u8::MAX {
        match FrameSize::from_size_bits(bits) {
            Ok(size) => {
                assert!(
                    bits == 12 || bits == 21 || bits == 30,
                    "chỉ 12/21/30 hợp lệ, nhận {bits}"
                );
                assert_eq!(size.size_bits(), bits);
            }
            Err(err) => {
                assert!(
                    bits != 12 && bits != 21 && bits != 30,
                    "size_bits hợp lệ {bits} bị từ chối"
                );
                assert_eq!(err, PteError::InvalidArgument);
                assert!(err.to_string().contains("InvalidArgument"));
            }
        }
    }
    // Biên cụ thể của K2: 11/13/20/22/29/31 và 0/255 đều là lỗi model/local.
    for bits in [0u8, 1, 11, 13, 20, 22, 29, 31, 63, 255] {
        assert_eq!(FrameSize::try_from(bits), Err(PteError::InvalidArgument));
    }
    // Không đổi ABI: lỗi chỉ là kiểu cục bộ của crate, không map sang `spec/abi.toml`.
    assert_eq!(
        FrameSize::from_size_bits(31).unwrap_err().code_name(),
        "InvalidArgument"
    );
}

// ---------------------------------------------------------------- walk_shape

#[test]
fn walk_shape_bien_va_39_bit() {
    // Ba chỉ số = bit 30/21/12, mỗi chỉ số 9 bit.
    assert_eq!(walk_shape(0).indices(), [0, 0, 0]);
    assert_eq!(
        walk_shape(0xfff).indices(),
        [0, 0, 0],
        "offset trong trang không vào chỉ số"
    );
    assert_eq!(walk_shape(0x1000).indices(), [0, 0, 1]);
    assert_eq!(walk_shape(1u64 << 21).indices(), [0, 1, 0]);
    assert_eq!(walk_shape(1u64 << 30).indices(), [1, 0, 0]);

    // BIÊN KT-35: va = 1 << 38 cắt đúng 3 chỉ số, không panic (bit 38 là bit cao nhất của VA 39 bit).
    let shape = walk_shape(1u64 << 38);
    assert_eq!(shape.len(), PTE_LEVELS);
    assert_eq!(shape.indices(), [256, 0, 0], "bit 38 -> chỉ số cấp gốc 256");
    assert_eq!(shape.level2(), 256);
    assert_eq!(shape.level1(), 0);
    assert_eq!(shape.level0(), 0);
    assert_eq!(shape.indices()[0], 256);

    // Ba chỉ số lớn nhất cùng lúc; u64::MAX bị cắt phần trên VA_BITS, không panic.
    let max_va = (511u64 << 30) | (511u64 << 21) | (511u64 << 12) | 0xfff;
    assert_eq!(walk_shape(max_va).indices(), [511, 511, 511]);
    assert_eq!(walk_shape(u64::MAX).indices(), [511, 511, 511]);
    assert_eq!(walk_shape(u64::MAX).indices(), walk_shape(max_va).indices());

    // Bit trên VA_BITS bị cắt (không phải kiểm canonical — việc của KT-37).
    assert_eq!(walk_shape(1u64 << 39).indices(), [0, 0, 0]);
    assert_eq!(walk_shape((1u64 << 39) | (1u64 << 12)).indices(), [0, 0, 1]);
    assert_eq!(walk_shape((1u64 << 40) | (1u64 << 21)).indices(), [0, 1, 0]);
    assert_eq!(walk_shape(1u64 << 63).indices(), [0, 0, 0]);
}

#[test]
fn walk_shape_khop_ba_chi_so_va_len_dung_3() {
    for shift in [0u32, 11, 12, 20, 21, 29, 30, 38] {
        let receives = walk_shape(1u64 << shift);
        assert_eq!(receives.len(), PTE_LEVELS);
        let expected = [
            ((1u64 << shift) >> 30 & 0x1ff) as u16,
            ((1u64 << shift) >> 21 & 0x1ff) as u16,
            ((1u64 << shift) >> 12 & 0x1ff) as u16,
        ];
        assert_eq!(receives.indices(), expected, "shift {shift}");
    }
}

#[test]
fn walk_shape_property_10k_va() {
    let mut state = 0x1234_5678_9abc_def0u64;
    for _ in 0..PROPERTY_ITERS {
        let va = next_lcg(&mut state);
        let shape: WalkShape = walk_shape(va);
        let idx = shape.indices();
        assert_eq!(idx.len(), PTE_LEVELS);
        assert_eq!(idx[0], ((va >> 30) & 0x1ff) as u16);
        assert_eq!(idx[1], ((va >> 21) & 0x1ff) as u16);
        assert_eq!(idx[2], ((va >> 12) & 0x1ff) as u16);
        assert!(
            idx.iter().all(|&i| (i as usize) < 512),
            "chỉ số phải nằm trong bảng 512 entry: {idx:?} từ VA {va:#018x}"
        );

        // VA 39 bit hợp lệ: dựng lại bit 12..38 từ ba chỉ số phải khớp phần VA đã cắt.
        let canonical = va & ((1u64 << VA_BITS) - 1);
        let rebuilt = ((idx[0] as u64) << 30) | ((idx[1] as u64) << 21) | ((idx[2] as u64) << 12);
        assert_eq!(rebuilt, canonical & !0xfff);
    }
}

// ---------------------------------------------------------------- Pte

#[test]
fn pte_la_newtype_u64_tho() {
    assert_eq!(core::mem::size_of::<Pte>(), 8);
    assert_eq!(Pte::default(), Pte::EMPTY);
    assert!(Pte::default().is_empty());
    let pte = Pte::from_raw(0x8000_0000_0000_0fff);
    assert_eq!(pte.raw(), 0x8000_0000_0000_0fff);
    assert_eq!(u64::from(pte), 0x8000_0000_0000_0fff);
    assert_eq!(Pte::from(0x8000_0000_0000_0fff), pte);
    assert!(!pte.is_empty());
    assert_eq!(Pte::EMPTY.raw(), 0);
    // Layout là u64 thô: KT-35 **không** gán nghĩa bit (rights/attrs thuộc KT-37).
    assert_eq!(Pte::from_raw(u64::MAX).raw(), u64::MAX);
    assert!(format!("{pte:?}").contains("Pte"));
    assert_eq!(pte, pte);
    assert_ne!(pte, Pte::EMPTY);
}

// ---------------------------------------------------------------- PteArray

#[test]
fn pte_array_dung_co_512_entry_4k() {
    // Bảng chuẩn: 512 PTE × 8 B = 4096 B = một trang 4 KiB.
    assert_eq!(core::mem::size_of::<PteArray<512>>(), 4096);
    assert_eq!(core::mem::size_of::<PteArray<1>>(), 8);
    assert_eq!(core::mem::size_of::<PteArray<0>>(), 0);
    let a = PteArray::<512>::new();
    assert_eq!(a.len(), 512);
    assert!(!a.is_empty());
    assert_eq!(a.byte_size(), 4096);
    assert!(a.iter().all(|pte| *pte == Pte::EMPTY));
    assert_eq!(a.get(0), Some(Pte::EMPTY));
    assert_eq!(a.get(511), Some(Pte::EMPTY));
    assert_eq!(a.get(512), None);
    assert_eq!(a.get(usize::MAX), None);
    assert_eq!(a.as_array().len(), 512);
    assert_eq!(PteArray::<0>::new().len(), 0);
    assert!(PteArray::<0>::new().is_empty());
}

#[test]
fn pte_array_khong_panic_moi_chi_so_va_set_tra_cu() {
    let mut array = PteArray::<4>::new();
    assert_eq!(array.set(0, Pte::from_raw(0xdead)), Some(Pte::EMPTY));
    assert_eq!(
        array.set(0, Pte::from_raw(0xbeef)),
        Some(Pte::from_raw(0xdead))
    );
    assert_eq!(array.get(0), Some(Pte::from_raw(0xbeef)));
    assert_eq!(array.set(3, Pte::from_raw(3)), Some(Pte::EMPTY));
    assert_eq!(array.get(3), Some(Pte::from_raw(3)));
    // Ngoài biên: trả None, **không** panic, **không** ghi.
    assert_eq!(array.set(4, Pte::from_raw(4)), None);
    assert_eq!(array.set(usize::MAX, Pte::from_raw(5)), None);
    assert_eq!(array.get(4), None);
    assert_eq!(array.get(usize::MAX), None);
    assert_eq!(
        array.as_array(),
        &[
            Pte::from_raw(0xbeef),
            Pte::EMPTY,
            Pte::EMPTY,
            Pte::from_raw(3)
        ]
    );

    // Quét toàn dải chỉ số (và hai biên) không panic.
    for index in 0..=4 {
        let _ = array.get(index);
        let _ = array.set(index, Pte::from_raw(index as u64));
    }
    assert_eq!(array.get(4), None);

    // Clone độc lập; Default = rỗng.
    let copy = array.clone();
    assert_eq!(copy, array);
    assert_eq!(PteArray::<4>::default(), PteArray::<4>::new());
    assert!(format!("{array:?}").contains("PteArray"));
}
