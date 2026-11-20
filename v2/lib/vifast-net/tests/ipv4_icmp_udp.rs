// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-244 — test host cho `ipv4` + `icmp` + `udp` (D2-05: viết trước, đỏ rồi mới xanh).

use vifast_net::addr::Ipv4Addr;
use vifast_net::checksum::internet_checksum;
use vifast_net::icmp::{IcmpEcho, IcmpError, IcmpMessage, TYPE_ECHO_REPLY, TYPE_ECHO_REQUEST};
use vifast_net::ipv4::{
    Ipv4Error, Ipv4Header, FLAG_DF, FLAG_MF, IPV4_MIN_HEADER_LEN, PROTO_ICMP, PROTO_UDP,
};
use vifast_net::udp::{UdpError, UdpHeader, DHCP_CLIENT_PORT, DHCP_SERVER_PORT, UDP_HEADER_LEN};

#[test]
fn ipv4_roundtrip_va_payload() {
    let payload = [1u8, 2, 3, 4, 5];
    let mut h = Ipv4Header::new(
        PROTO_UDP,
        Ipv4Addr::new(10, 0, 2, 15),
        Ipv4Addr::new(10, 0, 2, 2),
        payload.len(),
    );
    h.id = 0x1234;
    let mut buf = [0u8; 64];
    let n = h.encode(&mut buf).expect("vừa 20 byte");
    assert_eq!(n, IPV4_MIN_HEADER_LEN);
    buf[n..n + payload.len()].copy_from_slice(&payload);
    let p = Ipv4Header::parse(&buf[..n + payload.len()], true).expect("hợp lệ");
    assert_eq!(p.header.protocol, PROTO_UDP);
    assert_eq!(p.header.total_len, 25);
    assert_eq!(p.header.id, 0x1234);
    assert_eq!(p.header.ttl, 64);
    assert_eq!(p.header.src, Ipv4Addr::new(10, 0, 2, 15));
    assert_eq!(p.header.dst, Ipv4Addr::new(10, 0, 2, 2));
    assert_eq!(p.payload(), &payload);
    assert_eq!(buf[0], 0x45);
}

#[test]
fn ipv4_checksum_dung_sau_encode() {
    let h = Ipv4Header::new(
        PROTO_ICMP,
        Ipv4Addr::new(192, 168, 0, 1),
        Ipv4Addr::new(192, 168, 0, 199),
        8,
    );
    let mut buf = [0u8; IPV4_MIN_HEADER_LEN];
    h.encode(&mut buf).expect("vừa");
    assert_eq!(internet_checksum(&buf), 0);
    assert_ne!(u16::from_be_bytes([buf[10], buf[11]]), 0);
}

#[test]
fn ipv4_ca_am_version_sai() {
    let h = Ipv4Header::new(PROTO_UDP, Ipv4Addr::UNSPECIFIED, Ipv4Addr::UNSPECIFIED, 0);
    let mut buf = [0u8; IPV4_MIN_HEADER_LEN];
    h.encode(&mut buf).expect("vừa");
    buf[0] = 0x65;
    assert_eq!(
        Ipv4Header::parse(&buf, false).err(),
        Some(Ipv4Error::BadVersion)
    );
}

#[test]
fn ipv4_ca_am_ihl_sai() {
    let h = Ipv4Header::new(PROTO_UDP, Ipv4Addr::UNSPECIFIED, Ipv4Addr::UNSPECIFIED, 0);
    let mut buf = [0u8; IPV4_MIN_HEADER_LEN];
    h.encode(&mut buf).expect("vừa");
    buf[0] = 0x44;
    assert_eq!(
        Ipv4Header::parse(&buf, false).err(),
        Some(Ipv4Error::BadIhl)
    );
}

#[test]
fn ipv4_ca_am_total_length_sai() {
    let h = Ipv4Header::new(PROTO_UDP, Ipv4Addr::UNSPECIFIED, Ipv4Addr::UNSPECIFIED, 0);
    let mut buf = [0u8; IPV4_MIN_HEADER_LEN];
    h.encode(&mut buf).expect("vừa");

    let mut bad = buf;
    bad[2..4].copy_from_slice(&12u16.to_be_bytes());
    assert_eq!(
        Ipv4Header::parse(&bad, false).err(),
        Some(Ipv4Error::BadTotalLength)
    );

    let mut bad = buf;
    bad[2..4].copy_from_slice(&100u16.to_be_bytes());
    assert_eq!(
        Ipv4Header::parse(&bad, false).err(),
        Some(Ipv4Error::Truncated)
    );
}

#[test]
fn ipv4_ca_am_checksum_sai_va_co_bo_kiem() {
    let h = Ipv4Header::new(PROTO_UDP, Ipv4Addr::UNSPECIFIED, Ipv4Addr::UNSPECIFIED, 0);
    let mut buf = [0u8; IPV4_MIN_HEADER_LEN];
    h.encode(&mut buf).expect("vừa");
    buf[10] ^= 0xff;
    assert_eq!(
        Ipv4Header::parse(&buf, true).err(),
        Some(Ipv4Error::BadChecksum)
    );
    // Cờ cho phép bỏ kiểm: cùng lát byte, parse vẫn đọc được header.
    assert!(Ipv4Header::parse(&buf, false).is_ok());
}

#[test]
fn ipv4_tu_choi_phan_manh() {
    let h = Ipv4Header::new(PROTO_UDP, Ipv4Addr::UNSPECIFIED, Ipv4Addr::UNSPECIFIED, 0);
    let mut buf = [0u8; IPV4_MIN_HEADER_LEN];
    h.encode(&mut buf).expect("vừa");

    let mut mf = buf;
    mf[6] |= 0x20; // MF
    assert_eq!(
        Ipv4Header::parse(&mf, false).err(),
        Some(Ipv4Error::Unsupported)
    );

    let mut off = buf;
    off[7] = 1; // frag_offset = 1
    assert_eq!(
        Ipv4Header::parse(&off, false).err(),
        Some(Ipv4Error::Unsupported)
    );

    let mut df = buf;
    df[6] |= 0x40; // DF — không phải phân mảnh
    let p = Ipv4Header::parse(&df, false).expect("DF hợp lệ");
    assert_eq!(p.header.flags & FLAG_DF, FLAG_DF);
    assert_eq!(p.header.flags & FLAG_MF, 0);
}

#[test]
fn ipv4_ca_am_cat_cut() {
    let buf = [0u8; IPV4_MIN_HEADER_LEN - 1];
    assert_eq!(
        Ipv4Header::parse(&buf, false).err(),
        Some(Ipv4Error::Truncated)
    );
    let h = Ipv4Header::new(PROTO_UDP, Ipv4Addr::UNSPECIFIED, Ipv4Addr::UNSPECIFIED, 0);
    let mut small = [0u8; IPV4_MIN_HEADER_LEN - 1];
    assert_eq!(h.encode(&mut small), Err(Ipv4Error::BufferTooSmall));
    assert_eq!(Ipv4Error::Unsupported.name(), "ipv4-unsupported");
}

#[test]
fn icmp_echo_request_roundtrip() {
    let e = IcmpEcho::request(0x1234, 1, b"abcd");
    let mut buf = [0u8; 12];
    let n = e.encode(&mut buf).expect("vừa 12 byte");
    assert_eq!(n, 12);
    assert_eq!(buf[0], TYPE_ECHO_REQUEST);
    assert_eq!(buf[1], 0);
    assert_eq!(u16::from_be_bytes([buf[2], buf[3]]), 0x2104);
    match IcmpEcho::parse(&buf[..n]).expect("hợp lệ") {
        IcmpMessage::EchoRequest(g) => {
            assert_eq!(g.id, 0x1234);
            assert_eq!(g.seq, 1);
            assert_eq!(g.payload, &b"abcd"[..]);
        }
        other => panic!("phải là EchoRequest, nhận {other:?}"),
    }
}

#[test]
fn icmp_echo_reply_roundtrip() {
    let e = IcmpEcho::reply(7, 9, &[]);
    let mut buf = [0u8; 8];
    let n = e.encode(&mut buf).expect("vừa 8 byte");
    assert_eq!(n, 8);
    assert_eq!(buf[0], TYPE_ECHO_REPLY);
    match IcmpEcho::parse(&buf[..n]).expect("hợp lệ") {
        IcmpMessage::EchoReply(g) => {
            assert_eq!(g.id, 7);
            assert_eq!(g.seq, 9);
            assert!(g.payload.is_empty());
        }
        other => panic!("phải là EchoReply, nhận {other:?}"),
    }
}

#[test]
fn icmp_ca_am_checksum_sai() {
    let e = IcmpEcho::request(1, 2, b"xyz");
    let mut buf = [0u8; 11];
    let n = e.encode(&mut buf).expect("vừa");
    buf[2] ^= 0xff;
    assert_eq!(
        IcmpEcho::parse(&buf[..n]).err(),
        Some(IcmpError::BadChecksum)
    );
}

#[test]
fn icmp_type_la_tra_other() {
    // Type 3 (destination unreachable), code 1, checksum đúng.
    let mut buf = [0u8; 8];
    buf[0] = 3;
    buf[1] = 1;
    let c = internet_checksum(&buf);
    buf[2..4].copy_from_slice(&c.to_be_bytes());
    match IcmpEcho::parse(&buf).expect("hợp lệ") {
        IcmpMessage::Other { typ, code } => {
            assert_eq!(typ, 3);
            assert_eq!(code, 1);
        }
        other => panic!("phải là Other, nhận {other:?}"),
    }
}

#[test]
fn icmp_echo_code_khac_khong_tra_other() {
    let mut buf = [0u8; 8];
    buf[0] = TYPE_ECHO_REQUEST;
    buf[1] = 5;
    let c = internet_checksum(&buf);
    buf[2..4].copy_from_slice(&c.to_be_bytes());
    match IcmpEcho::parse(&buf).expect("hợp lệ") {
        IcmpMessage::Other { typ, code } => {
            assert_eq!(typ, TYPE_ECHO_REQUEST);
            assert_eq!(code, 5);
        }
        other => panic!("echo code≠0 phải là Other, nhận {other:?}"),
    }
}

#[test]
fn icmp_ca_am_cat_cut_va_buffer_hep() {
    let buf = [0u8; 7];
    assert_eq!(IcmpEcho::parse(&buf).err(), Some(IcmpError::Truncated));
    let e = IcmpEcho::request(1, 1, b"abcd");
    let mut small = [0u8; 11];
    assert_eq!(e.encode(&mut small), Err(IcmpError::BufferTooSmall));
    assert_eq!(IcmpError::BadChecksum.name(), "icmp-bad-checksum");
}

#[test]
fn udp_roundtrip_va_checksum_vector() {
    let src = Ipv4Addr::new(192, 168, 0, 1);
    let dst = Ipv4Addr::new(192, 168, 0, 199);
    let mut buf = [0u8; 64];
    let n = UdpHeader::encode(src, dst, 5000, 5001, b"hello", &mut buf).expect("vừa");
    assert_eq!(n, 13);
    assert_eq!(u16::from_be_bytes([buf[4], buf[5]]), 13);
    assert_eq!(u16::from_be_bytes([buf[6], buf[7]]), 0x12d8);
    let p = UdpHeader::parse(src, dst, &buf[..n], true).expect("hợp lệ");
    assert_eq!(p.header.src_port, 5000);
    assert_eq!(p.header.dst_port, 5001);
    assert_eq!(p.header.len, 13);
    assert_eq!(p.payload(), &b"hello"[..]);
    assert_eq!(UDP_HEADER_LEN, 8);
    assert_eq!(DHCP_CLIENT_PORT, 68);
    assert_eq!(DHCP_SERVER_PORT, 67);
}

#[test]
fn udp_checksum_khong_dung_bang_khong() {
    // RFC 768: checksum 0 = không dùng checksum ⇒ parse chấp nhận dù bật kiểm.
    let src = Ipv4Addr::new(10, 0, 2, 15);
    let dst = Ipv4Addr::new(10, 0, 2, 2);
    let mut buf = [0u8; 16];
    let n = UdpHeader::encode(src, dst, 1234, 53, b"abcd", &mut buf).expect("vừa");
    buf[6] = 0;
    buf[7] = 0;
    let p = UdpHeader::parse(src, dst, &buf[..n], true).expect("checksum 0 hợp lệ");
    assert_eq!(p.payload(), &b"abcd"[..]);
    assert_eq!(p.header.checksum, 0);
}

#[test]
fn udp_ca_am_checksum_sai() {
    let src = Ipv4Addr::new(10, 0, 2, 15);
    let dst = Ipv4Addr::new(10, 0, 2, 2);
    let mut buf = [0u8; 16];
    let n = UdpHeader::encode(src, dst, 1234, 53, b"abcd", &mut buf).expect("vừa");
    buf[6] ^= 0xff;
    assert_eq!(
        UdpHeader::parse(src, dst, &buf[..n], true).err(),
        Some(UdpError::BadChecksum)
    );
    assert!(UdpHeader::parse(src, dst, &buf[..n], false).is_ok());
}

#[test]
fn udp_ca_am_length_sai() {
    let src = Ipv4Addr::new(10, 0, 2, 15);
    let dst = Ipv4Addr::new(10, 0, 2, 2);
    let mut buf = [0u8; 16];
    let n = UdpHeader::encode(src, dst, 1, 2, b"abcd", &mut buf).expect("vừa");

    let mut bad = buf;
    bad[4..6].copy_from_slice(&4u16.to_be_bytes());
    assert_eq!(
        UdpHeader::parse(src, dst, &bad[..n], true).err(),
        Some(UdpError::BadLength)
    );

    let mut bad = buf;
    bad[4..6].copy_from_slice(&100u16.to_be_bytes());
    assert_eq!(
        UdpHeader::parse(src, dst, &bad[..n], true).err(),
        Some(UdpError::Truncated)
    );
}

#[test]
fn udp_ca_am_cat_cut_va_buffer_hep() {
    let src = Ipv4Addr::new(10, 0, 2, 15);
    let dst = Ipv4Addr::new(10, 0, 2, 2);
    let buf = [0u8; UDP_HEADER_LEN - 1];
    assert_eq!(
        UdpHeader::parse(src, dst, &buf, true).err(),
        Some(UdpError::Truncated)
    );
    let mut small = [0u8; 12];
    assert_eq!(
        UdpHeader::encode(src, dst, 1, 2, b"hello", &mut small),
        Err(UdpError::BufferTooSmall)
    );
    assert_eq!(UdpError::BadLength.name(), "udp-bad-length");
}

#[test]
fn udp_checksum_phu_thuoc_pseudo_header() {
    // Cùng cổng + payload nhưng src khác ⇒ checksum khác: pseudo-header có tham gia.
    let mut a = [0u8; 32];
    let mut b = [0u8; 32];
    UdpHeader::encode(
        Ipv4Addr::new(10, 0, 2, 15),
        Ipv4Addr::new(10, 0, 2, 2),
        1,
        2,
        b"x",
        &mut a,
    )
    .expect("vừa");
    UdpHeader::encode(
        Ipv4Addr::new(10, 0, 2, 16),
        Ipv4Addr::new(10, 0, 2, 2),
        1,
        2,
        b"x",
        &mut b,
    )
    .expect("vừa");
    assert_ne!(a[6..8], b[6..8]);
}
