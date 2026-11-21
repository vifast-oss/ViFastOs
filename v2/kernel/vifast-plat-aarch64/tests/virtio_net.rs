// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho `vifast_plat_aarch64::virtio_net` — KT-275 (K4' bước 3, D1 trên aarch64).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64 --test virtio_net
//! ```
//!
//! Phần **thuần** của driver được chốt trước khi chạm phần cứng (D2-05). Mọi hàm dưới đây là hàm
//! **không** chạm thanh ghi: chúng dựng/đọc khung bằng codec `vifast-net` và là thứ ảnh in ra
//! evidence, nên sai ở đây là sai **âm thầm** trên QEMU (log vẫn "trông đúng").
//!
//! 1. **Bố cục trang DMA**: hai ring + hai đệm phải nằm gọn trong **một** trang 4 KiB — cùng bất
//!    biến `const _: () = assert!` của driver (test này là bản chạy được, in số đo khi lệch).
//! 2. **ARP request byte-exact**: 42 byte, Ethernet broadcast, `oper=1`, `tha` toàn 0.
//! 3. **ARP reply đọc `ar_spa`, không phải `ar_tip`** — bẫy thật của KT-246: bản đầu kiểm
//!    `ar_tip == target_ip` nên **mọi** reply hợp lệ bị từ chối và ARP timeout dù thiết bị đã trả
//!    lời. Test này khoá đúng hành vi đã sửa.
//! 4. **DHCP DISCOVER**: khung Ethernet+IPv4+UDP+BOOTP đúng trường (broadcast, cổng 68→67,
//!    `op=1`, `chaddr` = MAC của ta, option 53 = 1).
//! 5. **DHCP parse**: OFFER tổng hợp khớp `xid`/`chaddr`/`msg_type` ⇒ đọc được `yiaddr` +
//!    `server_id`; lệch một trong ba ⇒ `None` (không nhận nhầm reply của máy khác).
//! 6. **ICMP echo**: khung đúng ethertype/protocol, checksum hợp lệ, `id`/`seq`/payload tròn.
//! 7. **Hằng số**: ngân sách bước mạng 3 s, poll 2 s, queue luỹ thừa 2 ≥ 2, offset DMA rời nhau.

// Test này **ghim hằng số** của driver (bố cục trang DMA, ngân sách thời gian) — khẳng định trên
// hằng là mục đích, không phải sơ suất (cùng khuôn `tests/virtio_blk.rs`).
#![allow(clippy::assertions_on_constants)]

use vifast_net::addr::{Ipv4Addr, MacAddr};
use vifast_net::arp::ArpPacket;
use vifast_net::dhcp::{DhcpMessage, DHCPOFFER, OPT_END};
use vifast_net::eth::{EthHeader, ETHERTYPE_ARP, ETHERTYPE_IPV4};
use vifast_net::frame::EthernetFrame;
use vifast_net::icmp::{IcmpEcho, IcmpMessage};
use vifast_net::ipv4::{Ipv4Header, PROTO_ICMP, PROTO_UDP};
use vifast_net::udp::{UdpHeader, DHCP_CLIENT_PORT, DHCP_SERVER_PORT};
use vifast_plat_aarch64::virtio_net::{
    arp_build_request, arp_parse_reply, dhcp_frame, dhcp_frame_parse, icmp_echo_frame,
    DMA_PAGE_BYTES, ICMP_PAYLOAD_BYTES, NET_STEP_TIMEOUT_US, POLL_TIMEOUT_US, RX_BUF_OFF,
    RX_RING_OFF, TX_BUF_OFF, TX_RING_OFF,
};
use vifast_virtio::net::{QUEUE_SIZE, RX_BUF_BYTES, TX_BUF_BYTES};
use vifast_virtio::queue::VqLayout;

/// MAC của ta trong test (giá trị QEMU cấp cho `-device virtio-net-pci`).
const OUR_MAC: MacAddr = MacAddr::new([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]);
/// MAC gateway slirp (`10.0.2.2`).
const GW_MAC: MacAddr = MacAddr::new([0x52, 0x55, 0x0a, 0x00, 0x02, 0x02]);
/// IP của ta (slirp cấp qua DHCP).
const OUR_IP: Ipv4Addr = Ipv4Addr::new(10, 0, 2, 15);
/// Gateway slirp.
const GW_IP: Ipv4Addr = Ipv4Addr::new(10, 0, 2, 2);
/// `xid` cố định của driver (không có RNG ở bước này).
const XID: u32 = 0x4b54_3235;

// ---------------------------------------------------------------- 1. bố cục trang DMA

#[test]
fn dma_layout_fits_one_page() {
    let layout = VqLayout::new(QUEUE_SIZE).expect("queue size hợp lệ");
    // Ring `rx` phải kết thúc **trước** ring `tx`, ring `tx` trước đệm `rx`, đệm `rx` trước `tx`.
    assert!(
        RX_RING_OFF + layout.end <= TX_RING_OFF,
        "ring rx kết thúc ở {} > TX_RING_OFF {TX_RING_OFF}",
        RX_RING_OFF + layout.end
    );
    assert!(
        TX_RING_OFF + layout.end <= RX_BUF_OFF,
        "ring tx kết thúc ở {} > RX_BUF_OFF {RX_BUF_OFF}",
        TX_RING_OFF + layout.end
    );
    assert!(RX_BUF_OFF + RX_BUF_BYTES <= TX_BUF_OFF);
    assert!(TX_BUF_OFF + TX_BUF_BYTES <= DMA_PAGE_BYTES);
    // Đệm `rx`/`tx` phải chứa được header modern + frame lớn nhất (hằng của `vifast-virtio`,
    // không viết lại số 1514 ở đây).
    assert!(RX_BUF_BYTES >= 12 + 1514);
    assert!(TX_BUF_BYTES >= 12 + 1514);
    assert!(QUEUE_SIZE >= 2 && QUEUE_SIZE.is_power_of_two());
    assert_eq!(DMA_PAGE_BYTES, 4096);
}

// ---------------------------------------------------------------- 2. ARP request

#[test]
fn arp_request_is_byte_exact() {
    let mut frame = [0u8; 64];
    let len = arp_build_request(OUR_MAC, OUR_IP, GW_IP, &mut frame).expect("dựng được ARP request");
    // 14 (Ethernet) + 28 (ARP) = 42.
    assert_eq!(len, 42);

    let eth = EthernetFrame::parse(&frame[..len]).expect("frame hợp lệ");
    assert_eq!(eth.header.dst, MacAddr::BROADCAST);
    assert_eq!(eth.header.src, OUR_MAC);
    assert_eq!(eth.header.ethertype, ETHERTYPE_ARP);

    let arp = ArpPacket::parse(eth.payload).expect("ARP hợp lệ");
    assert_eq!(arp.oper, 1, "ARP request phải có oper=1");
    assert_eq!(arp.sha, OUR_MAC);
    assert_eq!(arp.spa, OUR_IP);
    assert_eq!(arp.tha, MacAddr::ZERO, "tha của request phải toàn 0");
    assert_eq!(arp.tpa, GW_IP);
}

// ---------------------------------------------------------------- 3. ARP reply (bẫy KT-246)

/// Dựng một ARP **reply** từ `sha`/`spa` tới `tha`/`tpa` (Ethernet unicast tới `tha`).
fn arp_reply_frame(sha: MacAddr, spa: Ipv4Addr, tha: MacAddr, tpa: Ipv4Addr) -> ([u8; 64], usize) {
    let mut arp = [0u8; 28];
    let arp_len = ArpPacket {
        oper: 2,
        sha,
        spa,
        tha,
        tpa,
    }
    .encode(&mut arp)
    .expect("ARP reply vừa đệm");
    let mut frame = [0u8; 64];
    let len = EthernetFrame {
        header: EthHeader::new(tha, sha, ETHERTYPE_ARP),
        payload: &arp[..arp_len],
    }
    .encode(&mut frame)
    .expect("frame vừa đệm");
    (frame, len)
}

#[test]
fn arp_reply_uses_spa_not_tip() {
    // Reply **hợp lệ**: `ar_spa` = IP được hỏi (gateway), `ar_tip` = IP của **ta** (người hỏi).
    // Đây đúng là hình dạng mà bản đầu của KT-246 từ chối vì kiểm `ar_tip == target_ip`.
    let (frame, len) = arp_reply_frame(GW_MAC, GW_IP, OUR_MAC, OUR_IP);
    let found = arp_parse_reply(&frame[..len], OUR_IP, GW_IP, OUR_MAC);
    assert_eq!(found, Some(GW_MAC), "reply hợp lệ phải được nhận");

    // `ar_spa` khác IP được hỏi ⇒ không phải câu trả lời cho ta.
    let (frame, len) = arp_reply_frame(GW_MAC, Ipv4Addr::new(10, 0, 2, 99), OUR_MAC, OUR_IP);
    assert_eq!(arp_parse_reply(&frame[..len], OUR_IP, GW_IP, OUR_MAC), None);

    // `ar_tip` khác IP của ta ⇒ reply của phiên khác.
    let (frame, len) = arp_reply_frame(GW_MAC, GW_IP, OUR_MAC, Ipv4Addr::new(10, 0, 2, 77));
    assert_eq!(arp_parse_reply(&frame[..len], OUR_IP, GW_IP, OUR_MAC), None);

    // MAC toàn 0 / broadcast là giá trị vô nghĩa cho một host.
    let (frame, len) = arp_reply_frame(MacAddr::ZERO, GW_IP, OUR_MAC, OUR_IP);
    assert_eq!(arp_parse_reply(&frame[..len], OUR_IP, GW_IP, OUR_MAC), None);
    let (frame, len) = arp_reply_frame(MacAddr::BROADCAST, GW_IP, OUR_MAC, OUR_IP);
    assert_eq!(arp_parse_reply(&frame[..len], OUR_IP, GW_IP, OUR_MAC), None);

    // Request (oper=1) **không** phải reply.
    let mut frame = [0u8; 64];
    let len = arp_build_request(OUR_MAC, OUR_IP, GW_IP, &mut frame).expect("ARP request");
    assert_eq!(arp_parse_reply(&frame[..len], OUR_IP, GW_IP, OUR_MAC), None);
}

// ---------------------------------------------------------------- 4. DHCP DISCOVER

#[test]
fn dhcp_discover_frame_shape() {
    let discover = DhcpMessage::discover(XID, OUR_MAC);
    let mut frame = [0u8; 720];
    let len = dhcp_frame(&discover, OUR_MAC, &mut frame).expect("dựng được frame DHCP");

    let eth = EthernetFrame::parse(&frame[..len]).expect("frame hợp lệ");
    assert_eq!(
        eth.header.dst,
        MacAddr::BROADCAST,
        "DISCOVER phải broadcast"
    );
    assert_eq!(eth.header.src, OUR_MAC);
    assert_eq!(eth.header.ethertype, ETHERTYPE_IPV4);

    let ip = Ipv4Header::parse(eth.payload, true).expect("IPv4 hợp lệ");
    assert_eq!(ip.header.protocol, PROTO_UDP);
    assert_eq!(ip.header.src, Ipv4Addr::UNSPECIFIED);
    assert_eq!(ip.header.dst, Ipv4Addr::BROADCAST);

    let udp = UdpHeader::parse(ip.header.src, ip.header.dst, ip.payload(), true)
        .expect("UDP hợp lệ (checksum có pseudo-header)");
    assert_eq!(udp.header.src_port, DHCP_CLIENT_PORT);
    assert_eq!(udp.header.dst_port, DHCP_SERVER_PORT);

    let message = DhcpMessage::parse(udp.payload()).expect("BOOTP hợp lệ");
    assert_eq!(message.op(), 1, "DISCOVER là request");
    assert_eq!(message.xid(), XID);
    assert_eq!(message.chaddr(), OUR_MAC);
    assert_eq!(message.msg_type(), Some(1), "option 53 = DHCPDISCOVER");
    assert_eq!(message.yiaddr(), Ipv4Addr::UNSPECIFIED);
}

// ---------------------------------------------------------------- 5. DHCP parse (OFFER)

/// Dựng một **OFFER** tổng hợp từ khung DISCOVER: `op=2`, `yiaddr`, option 53 = 2, thêm
/// option 54 (server id) trước byte `END`.
fn synthetic_offer(yiaddr: Ipv4Addr, server: Ipv4Addr) -> [u8; 576] {
    let discover = DhcpMessage::discover(XID, OUR_MAC);
    let src = discover.as_bytes();
    let mut out = [0u8; 576];
    out[..src.len()].copy_from_slice(src);
    // `op` = reply, `yiaddr` (offset 16..20), option 53 (offset 240+2) = DHCPOFFER.
    out[0] = 2;
    out[16..20].copy_from_slice(&yiaddr.octets());
    out[242] = DHCPOFFER;
    // Byte `END` là byte cuối của khung DISCOVER; chèn option 54 ngay trước nó.
    let end = src.len() - 1;
    assert_eq!(out[end], OPT_END, "khung DISCOVER phải kết thúc bằng END");
    out[end] = 54;
    out[end + 1] = 4;
    out[end + 2..end + 6].copy_from_slice(&server.octets());
    out[end + 6] = OPT_END;
    out
}

/// Dựng frame **server → client** (cổng 67 → 68, IP nguồn = server) — đúng hình dạng slirp gửi.
///
/// `dhcp_frame` của driver chỉ dựng chiều **client → server** (68 → 67), nên phép kiểm
/// `dhcp_frame_parse` phải có khung chiều ngược lại; nếu dùng nhầm `dhcp_frame`, bộ parse **từ
/// chối đúng** (đó là hành vi đúng, không phải lỗi — đo được khi dựng test này).
fn server_frame(message: &DhcpMessage, server: Ipv4Addr, out: &mut [u8]) -> usize {
    let mut udp = [0u8; 640];
    let udp_len = UdpHeader::encode(
        server,
        OUR_IP,
        DHCP_SERVER_PORT,
        DHCP_CLIENT_PORT,
        message.as_bytes(),
        &mut udp,
    )
    .expect("UDP server vừa đệm");
    let mut ip = [0u8; 680];
    let header_len = Ipv4Header::new(PROTO_UDP, server, OUR_IP, udp_len)
        .encode(&mut ip)
        .expect("IPv4 vừa đệm");
    ip[header_len..header_len + udp_len].copy_from_slice(&udp[..udp_len]);
    let ip_len = header_len + udp_len;
    EthernetFrame {
        header: EthHeader::new(OUR_MAC, GW_MAC, ETHERTYPE_IPV4),
        payload: &ip[..ip_len],
    }
    .encode(out)
    .expect("frame vừa đệm")
}

#[test]
fn dhcp_frame_parse_matches_xid_mac_and_type() {
    let offer = synthetic_offer(OUR_IP, GW_IP);
    let message = DhcpMessage::parse(&offer).expect("OFFER tổng hợp hợp lệ");
    let mut frame = [0u8; 720];
    let len = server_frame(&message, GW_IP, &mut frame);

    // Khớp đủ ba điều kiện ⇒ đọc được `yiaddr` + `server_id`.
    let parsed = dhcp_frame_parse(&frame[..len], XID, OUR_MAC, DHCPOFFER).expect("phải khớp");
    assert_eq!(parsed.yiaddr(), OUR_IP);
    assert_eq!(parsed.server_id(), Some(GW_IP));

    // `xid` khác ⇒ không phải phiên của ta.
    assert!(dhcp_frame_parse(&frame[..len], XID ^ 1, OUR_MAC, DHCPOFFER).is_none());
    // `chaddr` khác ⇒ reply của máy khác.
    assert!(dhcp_frame_parse(&frame[..len], XID, GW_MAC, DHCPOFFER).is_none());
    // `msg_type` khác ⇒ chưa phải OFFER (ví dụ đang chờ ACK).
    assert!(dhcp_frame_parse(&frame[..len], XID, OUR_MAC, 5).is_none());
    // Không phải IPv4 (ARP) ⇒ bỏ qua, không hoảng.
    let mut arp_frame = [0u8; 64];
    let arp_len = arp_build_request(OUR_MAC, OUR_IP, GW_IP, &mut arp_frame).expect("ARP");
    assert!(dhcp_frame_parse(&arp_frame[..arp_len], XID, OUR_MAC, DHCPOFFER).is_none());
}

// ---------------------------------------------------------------- 6. ICMP echo

#[test]
fn icmp_echo_frame_round_trips() {
    let payload: [u8; ICMP_PAYLOAD_BYTES] = core::array::from_fn(|i| i as u8);
    let mut frame = [0u8; 128];
    let len = icmp_echo_frame(
        OUR_IP, OUR_MAC, GW_IP, GW_MAC, 0x4b54, 7, &payload, &mut frame,
    )
    .expect("dựng được frame ICMP");

    let eth = EthernetFrame::parse(&frame[..len]).expect("frame hợp lệ");
    assert_eq!(eth.header.dst, GW_MAC);
    assert_eq!(eth.header.src, OUR_MAC);
    assert_eq!(eth.header.ethertype, ETHERTYPE_IPV4);

    let ip = Ipv4Header::parse(eth.payload, true).expect("IPv4 hợp lệ");
    assert_eq!(ip.header.protocol, PROTO_ICMP);
    assert_eq!(ip.header.src, OUR_IP);
    assert_eq!(ip.header.dst, GW_IP);

    // `IcmpEcho::parse` kiểm checksum trên **toàn** lát byte ⇒ đây là phép kiểm checksum thật.
    match IcmpEcho::parse(ip.payload()).expect("ICMP hợp lệ (checksum đúng)") {
        IcmpMessage::EchoRequest(echo) => {
            assert_eq!(echo.id, 0x4b54);
            assert_eq!(echo.seq, 7);
            assert_eq!(echo.payload, &payload[..]);
        }
        other => panic!("phải là EchoRequest, nhận {other:?}"),
    }
}

// ---------------------------------------------------------------- 7. hằng số

#[test]
fn timeouts_and_offsets_are_sane() {
    // Ngân sách bước mạng 3 s, poll `used` 2 s — cùng khuôn riscv64 KT-245/246, không nới.
    assert_eq!(NET_STEP_TIMEOUT_US, 3_000_000);
    assert_eq!(POLL_TIMEOUT_US, 2_000_000);
    assert!(ICMP_PAYLOAD_BYTES > 0 && ICMP_PAYLOAD_BYTES <= 64);
    // Bốn vùng trong trang DMA phải **rời nhau** (không chồng), nếu không driver ghi đè ring.
    assert!(RX_RING_OFF < TX_RING_OFF);
    assert!(TX_RING_OFF < RX_BUF_OFF);
    assert!(RX_BUF_OFF < TX_BUF_OFF);
    assert!(TX_BUF_OFF < DMA_PAGE_BYTES);
}
