// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-244 — test host cho `dhcp` (D2-05: viết trước, đỏ rồi mới xanh).
//!
//! Gồm một **DHCP DISCOVER byte-exact** tự dựng (không dùng builder của crate để dựng kỳ vọng) và
//! một OFFER dựng tay để kiểm từng getter.

use vifast_net::addr::{Ipv4Addr, MacAddr};
use vifast_net::dhcp::{
    DhcpError, DhcpMessage, DHCPACK, DHCPDISCOVER, DHCPOFFER, DHCPREQUEST, DHCP_FIXED_LEN,
    DHCP_MAX_LEN, FLAG_BROADCAST, HLEN_ETHERNET, HTYPE_ETHERNET, MAGIC, MAGIC_COOKIE, OPT_DNS,
    OPT_END, OPT_HOSTNAME, OPT_LEASE_TIME, OPT_MSG_TYPE, OPT_PAD, OPT_PARAM_REQUEST,
    OPT_REQUESTED_IP, OPT_ROUTER, OPT_SERVER_ID, OPT_SUBNET_MASK, OP_REPLY, OP_REQUEST,
};

const MAC: MacAddr = MacAddr::new([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]);
const XID: u32 = 0x3903_f326;

#[test]
fn dhcp_discover_byte_exact() {
    let m = DhcpMessage::discover(XID, MAC);
    let b = m.as_bytes();
    assert_eq!(b.len(), 250);
    assert_eq!(m.len(), 250);
    assert!(!m.is_empty());

    // 236 byte BOOTP.
    assert_eq!(b[0], OP_REQUEST);
    assert_eq!(b[1], HTYPE_ETHERNET);
    assert_eq!(b[2], HLEN_ETHERNET);
    assert_eq!(b[3], 0);
    assert_eq!(&b[4..8], &XID.to_be_bytes()[..]);
    assert_eq!(&b[8..10], &[0u8, 0][..]);
    assert_eq!(&b[10..12], &FLAG_BROADCAST.to_be_bytes()[..]);
    assert_eq!(&b[12..28], &[0u8; 16][..]);
    assert_eq!(&b[28..34], &MAC.octets()[..]);
    assert_eq!(&b[34..44], &[0u8; 10][..]);
    assert_eq!(&b[44..108], &[0u8; 64][..]);
    assert_eq!(&b[108..236], &[0u8; 128][..]);
    assert_eq!(&b[236..240], &MAGIC[..]);
    assert_eq!(
        &b[240..],
        &[
            OPT_MSG_TYPE,
            1,
            DHCPDISCOVER,
            OPT_PARAM_REQUEST,
            4,
            1,
            3,
            6,
            12,
            OPT_END
        ][..]
    );

    // Getter đọc lại đúng từng trường.
    assert_eq!(m.op(), OP_REQUEST);
    assert_eq!(m.htype(), HTYPE_ETHERNET);
    assert_eq!(m.hlen(), HLEN_ETHERNET);
    assert_eq!(m.hops(), 0);
    assert_eq!(m.xid(), XID);
    assert_eq!(m.secs(), 0);
    assert_eq!(m.flags(), FLAG_BROADCAST);
    assert_eq!(m.ciaddr(), Ipv4Addr::UNSPECIFIED);
    assert_eq!(m.yiaddr(), Ipv4Addr::UNSPECIFIED);
    assert_eq!(m.siaddr(), Ipv4Addr::UNSPECIFIED);
    assert_eq!(m.giaddr(), Ipv4Addr::UNSPECIFIED);
    assert_eq!(m.chaddr(), MAC);
    assert_eq!(m.msg_type(), Some(DHCPDISCOVER));
    assert_eq!(m.option(OPT_PARAM_REQUEST), Some(&[1u8, 3, 6, 12][..]));
}

#[test]
fn dhcp_request_roundtrip() {
    let ip = Ipv4Addr::new(10, 0, 2, 15);
    let server = Ipv4Addr::new(10, 0, 2, 2);
    let m = DhcpMessage::request(XID, MAC, ip, server);
    assert_eq!(m.msg_type(), Some(DHCPREQUEST));
    assert_eq!(m.requested_ip(), Some(ip));
    assert_eq!(m.server_id(), Some(server));
    assert_eq!(m.op(), OP_REQUEST);
    let p = DhcpMessage::parse(m.as_bytes()).expect("hợp lệ");
    assert_eq!(p.msg_type(), Some(DHCPREQUEST));
    assert_eq!(p.requested_ip(), Some(ip));
    assert_eq!(p.server_id(), Some(server));
    assert_eq!(p.xid(), XID);
    assert_eq!(p.chaddr(), MAC);
    assert_eq!(p.as_bytes(), m.as_bytes());
}

#[test]
fn dhcp_parse_offer_va_getter() {
    // OFFER dựng tay: op=2, yiaddr=10.0.2.15, options 53/1/3/6/51/54.
    let mut b = [0u8; 300];
    b[0] = OP_REPLY;
    b[1] = HTYPE_ETHERNET;
    b[2] = HLEN_ETHERNET;
    b[4..8].copy_from_slice(&XID.to_be_bytes());
    b[16..20].copy_from_slice(&[10, 0, 2, 15]);
    b[28..34].copy_from_slice(&MAC.octets());
    b[236..240].copy_from_slice(&MAGIC);
    let mut i = DHCP_FIXED_LEN;
    let opts: [(u8, &[u8]); 6] = [
        (OPT_MSG_TYPE, &[DHCPOFFER]),
        (OPT_SUBNET_MASK, &[255, 255, 255, 0]),
        (OPT_ROUTER, &[10, 0, 2, 2]),
        (OPT_DNS, &[10, 0, 2, 3]),
        (OPT_LEASE_TIME, &3600u32.to_be_bytes()),
        (OPT_SERVER_ID, &[10, 0, 2, 2]),
    ];
    for (code, data) in opts {
        b[i] = code;
        b[i + 1] = data.len() as u8;
        b[i + 2..i + 2 + data.len()].copy_from_slice(data);
        i += 2 + data.len();
    }
    b[i] = OPT_END;
    i += 1;

    let m = DhcpMessage::parse(&b[..i]).expect("hợp lệ");
    assert_eq!(m.op(), OP_REPLY);
    assert_eq!(m.msg_type(), Some(DHCPOFFER));
    assert_eq!(m.yiaddr(), Ipv4Addr::new(10, 0, 2, 15));
    assert_eq!(m.subnet_mask(), Some(Ipv4Addr::new(255, 255, 255, 0)));
    assert_eq!(m.router(), Some(Ipv4Addr::new(10, 0, 2, 2)));
    assert_eq!(m.dns(), Some(Ipv4Addr::new(10, 0, 2, 3)));
    assert_eq!(m.lease_secs(), Some(3600));
    assert_eq!(m.server_id(), Some(Ipv4Addr::new(10, 0, 2, 2)));
    assert_eq!(m.len(), i);
    assert_eq!(m.as_bytes().len(), i);
}

#[test]
fn dhcp_parse_ack_va_hostname() {
    let mut b = [0u8; 300];
    b[0] = OP_REPLY;
    b[1] = HTYPE_ETHERNET;
    b[2] = HLEN_ETHERNET;
    b[4..8].copy_from_slice(&XID.to_be_bytes());
    b[16..20].copy_from_slice(&[10, 0, 2, 15]);
    b[28..34].copy_from_slice(&MAC.octets());
    b[236..240].copy_from_slice(&MAGIC);
    let mut i = DHCP_FIXED_LEN;
    let opts: [(u8, &[u8]); 3] = [
        (OPT_MSG_TYPE, &[DHCPACK]),
        (OPT_HOSTNAME, b"vifast"),
        (OPT_LEASE_TIME, &86400u32.to_be_bytes()),
    ];
    for (code, data) in opts {
        b[i] = code;
        b[i + 1] = data.len() as u8;
        b[i + 2..i + 2 + data.len()].copy_from_slice(data);
        i += 2 + data.len();
    }
    b[i] = OPT_END;
    i += 1;
    let m = DhcpMessage::parse(&b[..i]).expect("hợp lệ");
    assert_eq!(m.msg_type(), Some(DHCPACK));
    assert_eq!(m.hostname(), Some(&b"vifast"[..]));
    assert_eq!(m.lease_secs(), Some(86400));
    assert_eq!(m.subnet_mask(), None);
}

#[test]
fn dhcp_pad_bi_bo_qua() {
    let mut b = [0u8; 300];
    b[0] = OP_REQUEST;
    b[1] = HTYPE_ETHERNET;
    b[2] = HLEN_ETHERNET;
    b[4..8].copy_from_slice(&XID.to_be_bytes());
    b[28..34].copy_from_slice(&MAC.octets());
    b[236..240].copy_from_slice(&MAGIC);
    let mut i = DHCP_FIXED_LEN;
    b[i] = OPT_PAD;
    i += 1;
    b[i] = OPT_PAD;
    i += 1;
    b[i] = OPT_MSG_TYPE;
    b[i + 1] = 1;
    b[i + 2] = DHCPDISCOVER;
    i += 3;
    b[i] = OPT_END;
    i += 1;
    let m = DhcpMessage::parse(&b[..i]).expect("hợp lệ");
    assert_eq!(m.msg_type(), Some(DHCPDISCOVER));
    assert_eq!(m.len(), i);
}

#[test]
fn dhcp_ca_am_option_vuot_khung() {
    let m = DhcpMessage::discover(XID, MAC);
    let n = m.len();
    let mut b = [0u8; 300];
    b[..n].copy_from_slice(m.as_bytes());
    // Option 53 khai độ dài 200 nhưng khung chỉ còn 10 byte.
    b[240] = OPT_MSG_TYPE;
    b[241] = 200;
    assert_eq!(
        DhcpMessage::parse(&b[..n]).err(),
        Some(DhcpError::OptionOverrun)
    );
}

#[test]
fn dhcp_ca_am_option_cut_duoi() {
    // Option có mã nhưng thiếu byte độ dài ở cuối khung.
    let m = DhcpMessage::discover(XID, MAC);
    let n = m.len();
    let mut b = [0u8; 300];
    b[..n].copy_from_slice(m.as_bytes());
    b[n - 1] = OPT_MSG_TYPE; // thay END bằng mã option, hết khung
    assert_eq!(
        DhcpMessage::parse(&b[..n]).err(),
        Some(DhcpError::OptionOverrun)
    );
}

#[test]
fn dhcp_ca_am_thieu_end() {
    let m = DhcpMessage::discover(XID, MAC);
    let n = m.len();
    let mut b = [0u8; 300];
    b[..n].copy_from_slice(m.as_bytes());
    assert_eq!(
        DhcpMessage::parse(&b[..n - 1]).err(),
        Some(DhcpError::MissingEnd)
    );
}

#[test]
fn dhcp_ca_am_cookie_sai() {
    let m = DhcpMessage::discover(XID, MAC);
    let n = m.len();
    let mut b = [0u8; 300];
    b[..n].copy_from_slice(m.as_bytes());
    b[236] ^= 0xff;
    assert_eq!(
        DhcpMessage::parse(&b[..n]).err(),
        Some(DhcpError::BadCookie)
    );
}

#[test]
fn dhcp_ca_am_htype_hlen_op() {
    let m = DhcpMessage::discover(XID, MAC);
    let n = m.len();
    let mut b = [0u8; 300];
    b[..n].copy_from_slice(m.as_bytes());

    let mut bad = b;
    bad[1] = 6;
    assert_eq!(
        DhcpMessage::parse(&bad[..n]).err(),
        Some(DhcpError::BadHtype)
    );

    let mut bad = b;
    bad[2] = 8;
    assert_eq!(
        DhcpMessage::parse(&bad[..n]).err(),
        Some(DhcpError::BadHlen)
    );

    let mut bad = b;
    bad[0] = 9;
    assert_eq!(DhcpMessage::parse(&bad[..n]).err(), Some(DhcpError::BadOp));
}

#[test]
fn dhcp_ca_am_cat_cut_va_qua_dai() {
    let b = [0u8; DHCP_FIXED_LEN - 1];
    assert_eq!(DhcpMessage::parse(&b).err(), Some(DhcpError::Truncated));
    let big = [0u8; DHCP_MAX_LEN + 1];
    assert_eq!(DhcpMessage::parse(&big).err(), Some(DhcpError::TooLong));
    assert_eq!(DhcpError::OptionOverrun.name(), "dhcp-option-overrun");
}

#[test]
fn dhcp_option_khong_co_tra_none() {
    let m = DhcpMessage::discover(XID, MAC);
    assert_eq!(m.subnet_mask(), None);
    assert_eq!(m.router(), None);
    assert_eq!(m.dns(), None);
    assert_eq!(m.server_id(), None);
    assert_eq!(m.lease_secs(), None);
    assert_eq!(m.requested_ip(), None);
    assert_eq!(m.hostname(), None);
    assert_eq!(m.option(OPT_HOSTNAME), None);
}

#[test]
fn dhcp_hang_so_va_magic() {
    assert_eq!(MAGIC, [99, 130, 83, 99]);
    assert_eq!(MAGIC_COOKIE, 0x6382_5363);
    assert_eq!(DHCP_FIXED_LEN, 240);
    assert_eq!(DHCP_MAX_LEN, 576);
    assert_eq!(DHCPDISCOVER, 1);
    assert_eq!(DHCPOFFER, 2);
    assert_eq!(DHCPREQUEST, 3);
    assert_eq!(DHCPACK, 5);
    assert_eq!(OPT_SUBNET_MASK, 1);
    assert_eq!(OPT_ROUTER, 3);
    assert_eq!(OPT_DNS, 6);
    assert_eq!(OPT_HOSTNAME, 12);
    assert_eq!(OPT_REQUESTED_IP, 50);
    assert_eq!(OPT_LEASE_TIME, 51);
    assert_eq!(OPT_MSG_TYPE, 53);
    assert_eq!(OPT_SERVER_ID, 54);
    assert_eq!(OPT_PARAM_REQUEST, 55);
    assert_eq!(OPT_PAD, 0);
    assert_eq!(OPT_END, 255);
}
