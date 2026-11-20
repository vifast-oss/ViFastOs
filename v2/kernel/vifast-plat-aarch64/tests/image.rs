// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho `vifast_plat_aarch64::image` — KT-265 (D2-80, khuôn riscv64).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64
//! ```
//!
//! Hai điều phải chứng minh được **trên host** (không cần QEMU):
//!
//! 1. **Thuật toán băm đúng FNV-1a 64** — cùng hằng và cùng thứ tự với
//!    `test/conformance/image_digest.py` (bên băm tệp ELF). Nếu lệch, kernel in một hash mà script
//!    không bao giờ khớp ⇒ soak từ chối **mọi** boot (đỏ giả) hoặc tệ hơn: hai bên "khớp" vì cùng
//!    sai. Vector lấy từ đặc tả FNV, không lấy từ chính code.
//! 2. **Stub host fail-loud** — trên host không có bản đồ của kernel và không có symbol linker
//!    `__text_start`/`__text_end`, nên `image_digest()` phải **panic**, không được trả hash giả
//!    (một hash giả sẽ khiến người đọc tin nhầm là đã đo được ảnh).

use vifast_plat_aarch64::image::{fnv1a64, fnv1a64_update, FNV_OFFSET, FNV_PRIME};

/// Vector chuẩn của FNV-1a 64 (đặc tả FNV; đối chiếu độc lập bằng Python trong journal KT-265).
#[test]
fn fnv1a64_matches_reference_vectors() {
    let cases: &[(&[u8], u64)] = &[
        (b"", 0xcbf2_9ce4_8422_2325),
        (b"a", 0xaf63_dc4c_8601_ec8c),
        (b"foobar", 0x8594_4171_f739_67e8),
        (b"hello", 0xa430_d846_80aa_bd0b),
        (b"ViFastOs", 0x0d5f_38e5_80a0_3340),
    ];
    for (bytes, want) in cases {
        assert_eq!(
            fnv1a64(bytes),
            *want,
            "FNV-1a 64 của {:?} lệch vector chuẩn",
            core::str::from_utf8(bytes).unwrap_or("<bytes>")
        );
    }
}

/// Hằng số phải đúng đặc tả — `image_digest.py` dùng **cùng** hai số này.
#[test]
fn fnv_constants_are_the_spec_ones() {
    assert_eq!(FNV_OFFSET, 0xcbf2_9ce4_8422_2325);
    assert_eq!(FNV_PRIME, 0x0000_0100_0000_01b3);
}

/// `fnv1a64` phải **đúng bằng** chuỗi `fnv1a64_update` từng byte — vòng đọc bộ nhớ của
/// `image_digest` dùng `update`, nên hai đường không được lệch nhau.
#[test]
fn incremental_update_equals_one_shot() {
    let bytes = b"ViFastOs aarch64: boot ok\n";
    let mut hash = FNV_OFFSET;
    for &byte in bytes {
        hash = fnv1a64_update(hash, byte);
    }
    assert_eq!(hash, fnv1a64(bytes));
}

/// Độ dài 0 và độ dài 1 là hai biên của vòng `while p < end` — phải băm được, không panic.
#[test]
fn empty_and_single_byte_are_well_defined() {
    assert_eq!(fnv1a64(b""), FNV_OFFSET);
    assert_eq!(fnv1a64(b"\0"), FNV_OFFSET.wrapping_mul(FNV_PRIME));
}

/// Trên host **không** có symbol linker ⇒ `image_digest()` phải panic (fail-loud), không trả 0.
#[test]
fn host_stub_is_fail_loud() {
    let result = std::panic::catch_unwind(vifast_plat_aarch64::image::image_digest);
    assert!(
        result.is_err(),
        "image_digest() trên host phải panic — trả một hash giả là bằng chứng giả (KT-265)"
    );
}
