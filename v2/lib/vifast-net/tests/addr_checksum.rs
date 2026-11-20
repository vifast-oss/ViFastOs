// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-244 — test host cho `addr` + `checksum` (D2-05: viết trước, đỏ rồi mới xanh).

use vifast_net::addr::{same_subnet, Ipv4Addr, MacAddr, SubnetMask};
use vifast_net::checksum::{checksum_parts, internet_checksum, Checksum};

#[test]
fn mac_broadcast_va_zero() {
    assert!(MacAddr::BROADCAST.is_broadcast());
    assert!(MacAddr::BROADCAST.is_group());
    assert!(MacAddr::ZERO.is_zero());
    assert!(!MacAddr::ZERO.is_broadcast());
    assert_eq!(MacAddr::BROADCAST.octets(), [0xff; 6]);
    assert!(!MacAddr::new([0x02, 0, 0, 0, 0, 1]).is_broadcast());
    assert!(!MacAddr::new([0x02, 0, 0, 0, 0, 1]).is_group());
}

#[test]
fn mac_display_dang_hai_cham() {
    let m = MacAddr::new([0x02, 0x11, 0x22, 0x33, 0x44, 0xab]);
    assert_eq!(format!("{m}"), "02:11:22:33:44:ab");
}

#[test]
fn mac_hex_muoi_hai_ky_tu() {
    let m = MacAddr::new([0x00, 0x0a, 0x0b, 0x0c, 0x0d, 0xef]);
    assert_eq!(&m.to_hex(), b"000a0b0c0def");
}

#[test]
fn ipv4_u32_be_khu_hoi() {
    let ip = Ipv4Addr::new(192, 168, 0, 199);
    assert_eq!(ip.to_u32_be(), 0xc0a8_00c7);
    assert_eq!(Ipv4Addr::from_u32_be(0xc0a8_00c7), ip);
    assert_eq!(Ipv4Addr::new(10, 0, 2, 15).octets(), [10, 0, 2, 15]);
}

#[test]
fn ipv4_display_va_thuoc_tinh() {
    assert_eq!(format!("{}", Ipv4Addr::new(10, 0, 2, 15)), "10.0.2.15");
    assert!(Ipv4Addr::new(127, 0, 0, 1).is_loopback());
    assert!(Ipv4Addr::new(224, 0, 0, 1).is_multicast());
    assert!(Ipv4Addr::UNSPECIFIED.is_unspecified());
    assert!(Ipv4Addr::BROADCAST.is_broadcast());
    assert!(!Ipv4Addr::new(10, 0, 2, 15).is_multicast());
    assert!(!Ipv4Addr::new(10, 0, 2, 15).is_loopback());
}

#[test]
fn subnet_mask_tien_to_va_dia_chi() {
    let m = SubnetMask::new(24).expect("/24 hợp lệ");
    assert_eq!(m.prefix(), 24);
    assert_eq!(m.addr(), Ipv4Addr::new(255, 255, 255, 0));
    assert_eq!(
        m.network(Ipv4Addr::new(10, 0, 2, 15)),
        Ipv4Addr::new(10, 0, 2, 0)
    );
    assert!(SubnetMask::new(33).is_none());
    assert_eq!(
        SubnetMask::new(0).expect("/0").addr(),
        Ipv4Addr::UNSPECIFIED
    );
    assert_eq!(
        SubnetMask::new(32).expect("/32").addr(),
        Ipv4Addr::BROADCAST
    );
}

#[test]
fn subnet_mask_tu_dia_chi_chi_nhan_lien_tuc() {
    assert_eq!(
        SubnetMask::from_addr(Ipv4Addr::new(255, 255, 255, 0)).map(|m| m.prefix()),
        Some(24)
    );
    assert_eq!(
        SubnetMask::from_addr(Ipv4Addr::new(255, 255, 0, 0)).map(|m| m.prefix()),
        Some(16)
    );
    assert_eq!(SubnetMask::from_addr(Ipv4Addr::new(255, 0, 255, 0)), None);
    assert_eq!(
        SubnetMask::from_addr(Ipv4Addr::UNSPECIFIED).map(|m| m.prefix()),
        Some(0)
    );
    assert_eq!(
        SubnetMask::from_addr(Ipv4Addr::BROADCAST).map(|m| m.prefix()),
        Some(32)
    );
}

#[test]
fn same_subnet_theo_khau_do() {
    let m24 = SubnetMask::new(24).expect("/24");
    assert!(same_subnet(
        Ipv4Addr::new(10, 0, 2, 15),
        Ipv4Addr::new(10, 0, 2, 2),
        m24
    ));
    assert!(!same_subnet(
        Ipv4Addr::new(10, 0, 2, 15),
        Ipv4Addr::new(10, 0, 3, 2),
        m24
    ));
    let m16 = SubnetMask::new(16).expect("/16");
    assert!(same_subnet(
        Ipv4Addr::new(10, 0, 2, 15),
        Ipv4Addr::new(10, 0, 3, 2),
        m16
    ));
    assert!(m24.contains(Ipv4Addr::new(10, 0, 2, 0), Ipv4Addr::new(10, 0, 2, 99)));
}

#[test]
fn checksum_vector_rfc1071() {
    // RFC 1071 §3: 00 01 f2 03 f4 f5 f6 f7 ⇒ 0x220d.
    let data = [0x00u8, 0x01, 0xf2, 0x03, 0xf4, 0xf5, 0xf6, 0xf7];
    assert_eq!(internet_checksum(&data), 0x220d);
}

#[test]
fn checksum_do_dai_le_dem_byte_cao() {
    // "hello" (5 byte): byte cuối là byte CAO của word cuối.
    let padded = *b"hello\0";
    assert_eq!(internet_checksum(b"hello"), internet_checksum(&padded));
    assert_ne!(internet_checksum(b"hello"), 0);
}

#[test]
fn checksum_cong_don_tung_manh_bang_nhau() {
    let all = [0x00u8, 0x01, 0xf2, 0x03, 0xf4, 0xf5, 0xf6, 0xf7];
    let mut c = Checksum::new();
    c.add_bytes(&all[..3]);
    c.add_bytes(&all[3..]);
    assert_eq!(c.finish(), internet_checksum(&all));
    assert_eq!(
        checksum_parts(&[&all[..1], &all[1..5], &all[5..]]),
        internet_checksum(&all)
    );
    // Mảnh rỗng không đổi kết quả.
    assert_eq!(
        checksum_parts(&[&all[..4], &[], &all[4..]]),
        internet_checksum(&all)
    );
}

#[test]
fn checksum_add_u16_bang_add_bytes() {
    let mut a = Checksum::new();
    a.add_u16(0x1234);
    a.add_u16(0xabcd);
    let mut b = Checksum::new();
    b.add_bytes(&[0x12, 0x34, 0xab, 0xcd]);
    assert_eq!(a.finish(), b.finish());
}

#[test]
fn checksum_header_ipv4_vector() {
    // Header IPv4 thật (20 byte) với checksum = 0 ⇒ 0xb861; điền vào ⇒ checksum toàn header = 0.
    let mut hdr = [
        0x45u8, 0x00, 0x00, 0x73, 0x00, 0x00, 0x40, 0x00, 0x40, 0x11, 0x00, 0x00, 0xc0, 0xa8, 0x00,
        0x01, 0xc0, 0xa8, 0x00, 0xc7,
    ];
    assert_eq!(internet_checksum(&hdr), 0xb861);
    hdr[10] = 0xb8;
    hdr[11] = 0x61;
    assert_eq!(internet_checksum(&hdr), 0);
}

#[test]
fn checksum_rong_bang_ffff() {
    assert_eq!(internet_checksum(&[]), 0xffff);
    assert_eq!(Checksum::new().finish(), 0xffff);
}
