// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-418b** — test host cho **API entropy của `vifast-core`** (`crate::entropy`).
//!
//! Vì sao test host: đây là **hợp đồng** giữa driver `virtio-rng` (phần cứng) và phần còn lại của
//! kernel — "khi nào được coi là có entropy, và byte ra có tái lập/lặp không". Sai ở đây không
//! hiện ra ở boot mà hiện ra ở **khoá trùng** giữa hai lần chạy, rất khó truy.
//!
//! Bốn lớp được chốt:
//!
//! 1. **fail-closed**: chưa `seed` ⇒ `fill` trả `NotSeeded` và **không** ghi byte nào vào `out`;
//! 2. **seed rác bị từ chối**: quá ngắn, hoặc toàn **một** giá trị (thiết bị trả toàn 0);
//! 3. **không lặp**: hai lần `fill` liên tiếp khác nhau; 4 KiB ra **không** có khối 8 byte lặp;
//!    64 KiB ra phủ **đủ 256** giá trị byte (không kẹt ở một dải);
//! 4. **tất định theo seed**: cùng seed ⇒ cùng dòng ra (test tái lập được), khác seed ⇒ khác.
#![cfg(test)]

extern crate std;

use crate::entropy::{EntropyError, EntropyPool, ENTROPY_MIN_SEED_BYTES, ENTROPY_STATE_WORDS};

/// Seed "tử tế" cho test: dãy đếm 0..N (đa dạng, tất định, không phải entropy thật).
fn seed_bytes(len: usize) -> std::vec::Vec<u8> {
    (0..len)
        .map(|index| (index as u8).wrapping_mul(37))
        .collect()
}

#[test]
fn chua_seed_thi_fill_do_va_khong_ghi_byte_nao() {
    let mut pool = EntropyPool::new();
    assert!(!pool.is_seeded());
    let mut out = [0x5au8; 64];
    assert_eq!(pool.fill(&mut out), Err(EntropyError::NotSeeded));
    assert_eq!(
        out, [0x5au8; 64],
        "fill thất bại **không** được ghi vào out"
    );
    assert_eq!(pool.stats().bytes_out, 0);
}

#[test]
fn seed_qua_ngan_hoac_mot_gia_tri_thi_do() {
    let mut pool = EntropyPool::new();
    assert_eq!(
        pool.seed(&[0u8; 8]),
        Err(EntropyError::SeedTooShort),
        "seed dưới {ENTROPY_MIN_SEED_BYTES} byte phải bị từ chối"
    );
    assert!(!pool.is_seeded());
    // Thiết bị trả toàn 0 / toàn 0xff ⇒ **không** được coi là seed.
    assert_eq!(
        pool.seed(&[0u8; 64]),
        Err(EntropyError::SeedDegenerate),
        "seed toàn 0 phải bị từ chối"
    );
    assert_eq!(
        pool.seed(&[0xffu8; 64]),
        Err(EntropyError::SeedDegenerate),
        "seed toàn 0xff phải bị từ chối"
    );
    assert!(!pool.is_seeded());
    assert_eq!(pool.stats().seeded_bytes, 0);
    // Seed hợp lệ ⇒ nhận.
    assert_eq!(pool.seed(&seed_bytes(64)), Ok(()));
    assert!(pool.is_seeded());
    assert_eq!(pool.stats().seeded_bytes, 64);
}

#[test]
fn hai_lan_fill_lien_tiep_khac_nhau() {
    let mut pool = EntropyPool::new();
    pool.seed(&seed_bytes(64)).expect("seed hợp lệ");
    let mut first = [0u8; 64];
    let mut second = [0u8; 64];
    pool.fill(&mut first).expect("fill 1");
    pool.fill(&mut second).expect("fill 2");
    assert_ne!(first, second, "hai lần fill phải khác nhau");
    assert_eq!(pool.stats().bytes_out, 128);
}

#[test]
fn bon_kib_khong_lap_khoi_8_byte() {
    let mut pool = EntropyPool::new();
    pool.seed(&seed_bytes(64)).expect("seed hợp lệ");
    let mut out = [0u8; 4096];
    pool.fill(&mut out).expect("fill");
    // 512 khối 8 byte: đếm số khối **khác nhau** (không cấp phát: so từng cặp là 130k phép so —
    // rẻ hơn nhiều so với việc thêm `std::collections` vào test của crate `no_std`).
    let blocks = out.len() / 8;
    let mut duplicates = 0usize;
    for left in 0..blocks {
        for right in (left + 1)..blocks {
            if out[left * 8..left * 8 + 8] == out[right * 8..right * 8 + 8] {
                duplicates += 1;
            }
        }
    }
    assert_eq!(
        duplicates, 0,
        "4 KiB ra không được có hai khối 8 byte giống nhau"
    );
}

#[test]
fn sau_muoi_sau_kib_phu_du_256_gia_tri_byte() {
    let mut pool = EntropyPool::new();
    pool.seed(&seed_bytes(64)).expect("seed hợp lệ");
    let mut out = [0u8; 65536];
    pool.fill(&mut out).expect("fill");
    let mut seen = [false; 256];
    for byte in out {
        seen[usize::from(byte)] = true;
    }
    let missing = seen.iter().filter(|present| !**present).count();
    assert_eq!(missing, 0, "64 KiB ra phải phủ đủ 256 giá trị byte");
}

#[test]
fn cung_seed_thi_cung_dong_ra_khac_seed_thi_khac() {
    let mut left = EntropyPool::new();
    let mut right = EntropyPool::new();
    let mut other = EntropyPool::new();
    left.seed(&seed_bytes(64)).expect("seed");
    right.seed(&seed_bytes(64)).expect("seed");
    other.seed(&seed_bytes(65)).expect("seed");
    let mut a = [0u8; 64];
    let mut b = [0u8; 64];
    let mut c = [0u8; 64];
    left.fill(&mut a).expect("fill");
    right.fill(&mut b).expect("fill");
    other.fill(&mut c).expect("fill");
    assert_eq!(a, b, "cùng seed ⇒ cùng dòng ra (test tái lập được)");
    assert_ne!(a, c, "seed khác ⇒ dòng ra khác");
}

#[test]
fn seed_lai_thi_dong_ra_doi_va_dem_reseed() {
    let mut pool = EntropyPool::new();
    pool.seed(&seed_bytes(64)).expect("seed 1");
    let mut before = [0u8; 64];
    pool.fill(&mut before).expect("fill 1");
    pool.seed(&seed_bytes(96)).expect("seed 2");
    let mut after = [0u8; 64];
    pool.fill(&mut after).expect("fill 2");
    assert_ne!(before, after, "seed lại phải đổi dòng ra");
    let stats = pool.stats();
    assert_eq!(stats.reseeds, 1);
    assert_eq!(stats.seeded_bytes, 96, "seed sau cùng thay thế seed trước");
    assert_eq!(stats.bytes_out, 128);
    assert_eq!(ENTROPY_STATE_WORDS, 4, "trạng thái 256 bit");
}

#[test]
fn fill_do_dai_bat_ky_ke_ca_le() {
    let mut pool = EntropyPool::new();
    pool.seed(&seed_bytes(64)).expect("seed");
    for len in [1usize, 3, 7, 8, 9, 31, 32, 33, 255] {
        let mut out = std::vec![0u8; len];
        pool.fill(&mut out).expect("fill độ dài bất kỳ");
        assert_eq!(out.len(), len);
    }
    assert_eq!(
        pool.stats().bytes_out,
        1 + 3 + 7 + 8 + 9 + 31 + 32 + 33 + 255
    );
}
