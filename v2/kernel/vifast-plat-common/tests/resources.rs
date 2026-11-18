// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-81** — bảng tài nguyên nền tảng: một nguồn, và phép kiểm **chồng lấn** phải thật sự bắt.
//!
//! Hai vế, đúng luật đóng lỗi của Agent 0:
//!
//! 1. **Xanh trên bảng thật**: [`REGIONS`] không có cặp nào chồng lấn và mọi vùng hợp lệ.
//! 2. **Đỏ trên bảng hỏng**: một bảng cố ý đè nhau (và một bảng có vùng kích thước 0) **phải** bị
//!    bắt. Thiếu vế này thì một `first_overlap` luôn trả `None` cũng qua được vế 1.

use vifast_plat_common::resources::{
    first_invalid, first_overlap, overlaps, Region, CLINT, PCI_ECAM, PCI_MMIO32, PLIC, REGIONS,
    TEST_PAGE, UART0,
};

#[test]
fn bang_that_khong_chong_lan() {
    assert_eq!(
        first_overlap(REGIONS),
        None,
        "bảng tài nguyên nền tảng có hai vùng đè nhau — MMU map một đằng, driver đọc một nẻo"
    );
}

#[test]
fn bang_that_moi_vung_hop_le() {
    assert_eq!(
        first_invalid(REGIONS),
        None,
        "có vùng kích thước 0 hoặc tràn địa chỉ"
    );
    for r in REGIONS {
        assert!(r.size > 0, "vùng {} có kích thước 0", r.name);
        assert!(
            r.base % 0x1000 == 0,
            "vùng {} không căn trang 4 KiB",
            r.name
        );
    }
}

#[test]
fn bang_that_du_vung_bat_buoc() {
    // Bảng phải chứa **đủ** các vùng mà kernel map/đọc trực tiếp; thiếu một vùng là hằng số ấy
    // quay lại làm nguồn thứ hai.
    for want in [UART0, PLIC, CLINT, TEST_PAGE, PCI_ECAM, PCI_MMIO32] {
        assert!(
            REGIONS
                .iter()
                .any(|r| r.name == want.name && r.base == want.base),
            "bảng thiếu vùng {}",
            want.name
        );
    }
}

#[test]
fn chong_lan_bi_bat() {
    let a = Region {
        name: "a",
        base: 0x1000,
        size: 0x1000,
    };
    // Đè một phần.
    let b = Region {
        name: "b",
        base: 0x1800,
        size: 0x1000,
    };
    assert!(overlaps(a, b));
    assert_eq!(first_overlap(&[a, b]), Some((a, b)));
    // Chứa nhau.
    let c = Region {
        name: "c",
        base: 0x1000,
        size: 0x4000,
    };
    assert!(overlaps(a, c));
    // Kề nhau = an toàn (nửa mở).
    let d = Region {
        name: "d",
        base: 0x2000,
        size: 0x1000,
    };
    assert!(!overlaps(a, d));
    assert_eq!(first_overlap(&[a, d]), None);
    // Trùng đúng một byte ở biên cũng là chồng.
    let e = Region {
        name: "e",
        base: 0x1fff,
        size: 0x1000,
    };
    assert!(overlaps(a, e));
}

#[test]
fn vung_kich_thuoc_khong_bi_bat() {
    let zero = Region {
        name: "zero",
        base: 0x1000,
        size: 0,
    };
    assert!(!zero.is_valid());
    assert_eq!(first_invalid(&[zero]), Some(zero));
    // Kích thước 0 không được coi là "chồng lấn" (khoảng rỗng) — nhưng `first_invalid` bắt nó.
    let ok = Region {
        name: "ok",
        base: 0x1000,
        size: 0x1000,
    };
    assert!(!overlaps(zero, ok));
    assert_eq!(first_overlap(&[zero, ok]), None);
}

#[test]
fn tran_dia_chi_bi_bat() {
    let over = Region {
        name: "over",
        base: u64::MAX - 0x10,
        size: 0x1000,
    };
    assert!(!over.is_valid());
    assert_eq!(first_invalid(&[over]), Some(over));
    // `end()` bão hoà, không quay vòng thành địa chỉ nhỏ.
    assert_eq!(over.end(), u64::MAX);
}

#[test]
fn bang_rong_khong_bao_dong() {
    assert_eq!(first_overlap(&[]), None);
    assert_eq!(first_invalid(&[]), None);
}
