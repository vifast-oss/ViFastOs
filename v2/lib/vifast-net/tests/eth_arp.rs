// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-244 — test host cho `eth` + `arp` + `frame` (D2-05: viết trước, đỏ rồi mới xanh).

use vifast_net::addr::{Ipv4Addr, MacAddr};
use vifast_net::arp::{ArpError, ArpPacket, ARP_LEN, OPER_REPLY, OPER_REQUEST};
use vifast_net::eth::{
    EthError, EthHeader, ETHERTYPE_ARP, ETHERTYPE_IPV4, ETH_HEADER_LEN, ETH_MIN_FRAME,
    ETH_MIN_PAYLOAD,
};
use vifast_net::frame::EthernetFrame;

#[test]
fn eth_roundtrip_va_byte_layout() {
    let h = EthHeader::new(
        MacAddr::BROADCAST,
        MacAddr::new([0x02, 0, 0, 0, 0, 1]),
        ETHERTYPE_ARP,
    );
    let mut buf = [0u8; ETH_HEADER_LEN];
    assert_eq!(h.encode(&mut buf).expect("vừa 14 byte"), ETH_HEADER_LEN);
    assert_eq!(EthHeader::parse(&buf).expect("hợp lệ"), h);
    assert_eq!(&buf[0..6], &[0xff; 6]);
    assert_eq!(&buf[6..12], &[0x02, 0, 0, 0, 0, 1]);
    assert_eq!(&buf[12..14], &[0x08, 0x06]);
}

#[test]
fn eth_ca_am_cat_cut() {
    let buf = [0u8; ETH_HEADER_LEN - 1];
    assert_eq!(EthHeader::parse(&buf), Err(EthError::Truncated));
}

#[test]
fn eth_ca_am_buffer_hep() {
    let h = EthHeader::new(MacAddr::ZERO, MacAddr::ZERO, ETHERTYPE_IPV4);
    let mut buf = [0u8; ETH_HEADER_LEN - 1];
    assert_eq!(h.encode(&mut buf), Err(EthError::BufferTooSmall));
}

#[test]
fn eth_hang_so_va_ten_loi() {
    assert_eq!(ETHERTYPE_IPV4, 0x0800);
    assert_eq!(ETHERTYPE_ARP, 0x0806);
    assert_eq!(ETH_HEADER_LEN, 14);
    assert_eq!(ETH_MIN_PAYLOAD, 46);
    assert_eq!(ETH_MIN_FRAME, 60);
    assert_eq!(EthError::Truncated.name(), "eth-truncated");
    assert_eq!(EthError::BufferTooSmall.name(), "eth-buffer-too-small");
}

#[test]
fn arp_request_roundtrip() {
    let p = ArpPacket::request(
        MacAddr::new([0x02, 0, 0, 0, 0, 1]),
        Ipv4Addr::new(10, 0, 2, 15),
        Ipv4Addr::new(10, 0, 2, 2),
    );
    let mut buf = [0u8; ARP_LEN];
    assert_eq!(p.encode(&mut buf).expect("vừa 28 byte"), ARP_LEN);
    assert_eq!(ArpPacket::parse(&buf).expect("hợp lệ"), p);
    assert!(p.is_request());
    assert!(!p.is_reply());
    assert_eq!(&buf[6..8], &[0x00, 0x01]);
    assert_eq!(&buf[18..24], &[0u8; 6]);
    assert_eq!(OPER_REQUEST, 1);
    assert_eq!(OPER_REPLY, 2);
}

#[test]
fn arp_reply_roundtrip() {
    let p = ArpPacket::reply(
        MacAddr::new([0x02, 0, 0, 0, 0, 2]),
        Ipv4Addr::new(10, 0, 2, 2),
        MacAddr::new([0x02, 0, 0, 0, 0, 1]),
        Ipv4Addr::new(10, 0, 2, 15),
    );
    let mut buf = [0u8; ARP_LEN];
    assert_eq!(p.encode(&mut buf).expect("vừa 28 byte"), ARP_LEN);
    assert_eq!(ArpPacket::parse(&buf).expect("hợp lệ"), p);
    assert!(p.is_reply());
    assert_eq!(&buf[6..8], &[0x00, 0x02]);
    assert_eq!(&buf[8..14], &[0x02, 0, 0, 0, 0, 2]);
}

#[test]
fn arp_frame_request_that_byte_exact() {
    // Frame ARP request thật: broadcast hỏi 10.0.2.2, nguồn 10.0.2.15 / 52:54:00:12:34:56.
    let mac = MacAddr::new([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]);
    let arp = ArpPacket::request(mac, Ipv4Addr::new(10, 0, 2, 15), Ipv4Addr::new(10, 0, 2, 2));
    let eth = EthHeader::new(MacAddr::BROADCAST, mac, ETHERTYPE_ARP);
    let mut buf = [0u8; ETH_HEADER_LEN + ARP_LEN];
    let n = eth.encode(&mut buf).expect("vừa");
    let m = arp.encode(&mut buf[n..]).expect("vừa");
    assert_eq!(n + m, 42);
    let expected: [u8; 42] = [
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, // dst
        0x52, 0x54, 0x00, 0x12, 0x34, 0x56, // src
        0x08, 0x06, // ethertype ARP
        0x00, 0x01, // htype Ethernet
        0x08, 0x00, // ptype IPv4
        0x06, // hlen
        0x04, // plen
        0x00, 0x01, // oper request
        0x52, 0x54, 0x00, 0x12, 0x34, 0x56, // sha
        10, 0, 2, 15, // spa
        0, 0, 0, 0, 0, 0, // tha
        10, 0, 2, 2, // tpa
    ];
    assert_eq!(buf, expected);
    // Parse lại đúng frame đó.
    let f = EthernetFrame::parse(&buf).expect("hợp lệ");
    assert_eq!(f.header.ethertype, ETHERTYPE_ARP);
    assert_eq!(ArpPacket::parse(f.payload).expect("hợp lệ"), arp);
}

#[test]
fn arp_ca_am_hlen_plen_htype_ptype_oper() {
    let p = ArpPacket::request(
        MacAddr::new([0x02, 0, 0, 0, 0, 1]),
        Ipv4Addr::new(10, 0, 2, 15),
        Ipv4Addr::new(10, 0, 2, 2),
    );
    let mut buf = [0u8; ARP_LEN];
    p.encode(&mut buf).expect("vừa");

    let mut bad = buf;
    bad[4] = 8;
    assert_eq!(ArpPacket::parse(&bad), Err(ArpError::BadHlen));

    let mut bad = buf;
    bad[5] = 6;
    assert_eq!(ArpPacket::parse(&bad), Err(ArpError::BadPlen));

    let mut bad = buf;
    bad[0..2].copy_from_slice(&6u16.to_be_bytes());
    assert_eq!(ArpPacket::parse(&bad), Err(ArpError::BadHtype));

    let mut bad = buf;
    bad[2..4].copy_from_slice(&0x86ddu16.to_be_bytes());
    assert_eq!(ArpPacket::parse(&bad), Err(ArpError::BadPtype));

    let mut bad = buf;
    bad[6..8].copy_from_slice(&3u16.to_be_bytes());
    assert_eq!(ArpPacket::parse(&bad), Err(ArpError::BadOper));
}

#[test]
fn arp_ca_am_cat_cut_va_buffer_hep() {
    let buf = [0u8; ARP_LEN - 1];
    assert_eq!(ArpPacket::parse(&buf), Err(ArpError::Truncated));
    let p = ArpPacket::request(MacAddr::ZERO, Ipv4Addr::UNSPECIFIED, Ipv4Addr::UNSPECIFIED);
    let mut small = [0u8; ARP_LEN - 1];
    assert_eq!(p.encode(&mut small), Err(ArpError::BufferTooSmall));
    assert_eq!(ArpError::BadHlen.name(), "arp-bad-hlen");
}

#[test]
fn frame_roundtrip_va_payload() {
    let payload = [0xde, 0xad, 0xbe, 0xef];
    let f = EthernetFrame {
        header: EthHeader::new(
            MacAddr::BROADCAST,
            MacAddr::new([0x02, 0, 0, 0, 0, 1]),
            ETHERTYPE_IPV4,
        ),
        payload: &payload,
    };
    let mut buf = [0u8; 64];
    let n = f.encode(&mut buf).expect("vừa");
    assert_eq!(n, ETH_HEADER_LEN + payload.len());
    let g = EthernetFrame::parse(&buf[..n]).expect("hợp lệ");
    assert_eq!(g.header, f.header);
    assert_eq!(g.payload, &payload);
    assert_eq!(g.frame_len(), n);
}

#[test]
fn frame_ca_am_cat_cut_va_buffer_hep() {
    let buf = [0u8; ETH_HEADER_LEN - 1];
    assert_eq!(EthernetFrame::parse(&buf).err(), Some(EthError::Truncated));
    let f = EthernetFrame {
        header: EthHeader::new(MacAddr::ZERO, MacAddr::ZERO, ETHERTYPE_IPV4),
        payload: &[1, 2, 3, 4],
    };
    let mut small = [0u8; ETH_HEADER_LEN + 3];
    assert_eq!(f.encode(&mut small), Err(EthError::BufferTooSmall));
}
