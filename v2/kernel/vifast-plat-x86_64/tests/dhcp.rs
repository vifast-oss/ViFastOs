// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho DHCP client của `vifast_plat_x86_64::virtio_net` — KT-326 (K5' — DHCP x86_64).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-x86_64 --test dhcp
//! ```
//!
//! # Vì sao DHCP client có test host dù QEMU có server thật
//!
//! Slirp **luôn** trả lời DHCP, nên một client nhận **mọi** frame (không kiểm cookie/xid) vẫn
//! xanh trên QEMU. Ba lớp lỗi dưới đây chỉ bắt được bằng test host:
//!
//! 1. **Khung dựng sai** (broadcast/cổng/checksum/`chaddr`) — QEMU có thể vẫn trả lời, nhưng
//!    khung không đúng RFC 2131. Test đọc lại bằng codec `vifast-net`.
//! 2. **Nhận frame của người khác** — OFFER/ACK có `xid`/`chaddr`/`msg_type` khác **phải** bị
//!    bỏ qua. Đột biến (ii) của KT-326 (bỏ kiểm `xid`) làm test này ĐỎ.
//! 3. **Không có trần thời gian** — client phải trả `Timeout` sau ngân sách **hữu hạn**, kể cả
//!    khi `timeout_us = 0` (0 **không** được hiểu là "chờ mãi"). Đột biến (iii) làm test này
//!    ĐỎ/treo; guard `max_now_calls` biến "treo" thành ĐỎ thay vì treo cả `cargo test`.
//!
//! Ngoài ra `dhcp_filter_selfcheck` là phép kiểm **chạy trong ảnh** (TAP 31): nó bơm lại chính
//! ACK với cookie sai và `xid` sai rồi đòi bộ lọc từ chối — nhờ vậy đột biến (i)/(ii) làm **QEMU
//! ĐỎ**, không chỉ test host.

use std::collections::VecDeque;

use vifast_net::addr::{Ipv4Addr, MacAddr};
use vifast_net::dhcp::{
    DhcpMessage, DHCPACK, DHCPOFFER, HLEN_ETHERNET, HTYPE_ETHERNET, MAGIC, OPT_DNS, OPT_LEASE_TIME,
    OPT_MSG_TYPE, OPT_ROUTER, OPT_SERVER_ID, OPT_SUBNET_MASK, OP_REPLY,
};
use vifast_net::eth::ETHERTYPE_IPV4;
use vifast_net::frame::EthernetFrame;
use vifast_net::ipv4::{Ipv4Header, PROTO_UDP};
use vifast_net::udp::{UdpHeader, DHCP_CLIENT_PORT, DHCP_SERVER_PORT};
use vifast_plat_x86_64::virtio_net::{
    dhcp_acquire_with, dhcp_filter_selfcheck, dhcp_frame, dhcp_frame_parse, dhcp_lease_from_ack,
    DhcpIo, DhcpLease, DHCP_ATTEMPT_TIMEOUT_US, DHCP_MAX_ATTEMPTS, DHCP_TOTAL_TIMEOUT_US, DHCP_XID,
};
use vifast_virtio::net::NetError;

/// MAC của ta (giá trị QEMU cấp cho `-device virtio-net-pci`).
const OUR_MAC: MacAddr = MacAddr::new([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]);
/// MAC gateway slirp (`10.0.2.2`).
const GW_MAC: MacAddr = MacAddr::new([0x52, 0x55, 0x0a, 0x00, 0x02, 0x02]);
/// IP slirp cấp.
const OUR_IP: Ipv4Addr = Ipv4Addr::new(10, 0, 2, 15);
/// Gateway slirp.
const GW_IP: Ipv4Addr = Ipv4Addr::new(10, 0, 2, 2);
/// DNS server của slirp.
const DNS_IP: Ipv4Addr = Ipv4Addr::new(10, 0, 2, 3);
/// `/24` của slirp.
const MASK: Ipv4Addr = Ipv4Addr::new(255, 255, 255, 0);

// ---------------------------------------------------------------- dựng reply tổng hợp

/// Dựng payload BOOTP/DHCP **server → client** (op=2, cookie đúng) với danh sách option cho trước.
fn reply_payload(xid: u32, mac: MacAddr, yiaddr: Ipv4Addr, opts: &[(u8, &[u8])]) -> Vec<u8> {
    let mut b = vec![0u8; 240];
    b[0] = OP_REPLY;
    b[1] = HTYPE_ETHERNET;
    b[2] = HLEN_ETHERNET;
    b[4..8].copy_from_slice(&xid.to_be_bytes());
    b[16..20].copy_from_slice(&yiaddr.octets());
    b[28..34].copy_from_slice(&mac.octets());
    b[236..240].copy_from_slice(&MAGIC);
    for (code, data) in opts {
        b.push(*code);
        b.push(data.len() as u8);
        b.extend_from_slice(data);
    }
    b.push(255);
    b
}

fn lease_u32(secs: u32) -> [u8; 4] {
    secs.to_be_bytes()
}

/// ACK đầy đủ option (53/1/3/6/51/54) như slirp trả.
fn ack_payload(xid: u32, mac: MacAddr) -> Vec<u8> {
    reply_payload(
        xid,
        mac,
        OUR_IP,
        &[
            (OPT_MSG_TYPE, &[DHCPACK]),
            (OPT_SUBNET_MASK, &MASK.octets()),
            (OPT_ROUTER, &GW_IP.octets()),
            (OPT_DNS, &DNS_IP.octets()),
            (OPT_LEASE_TIME, &lease_u32(86_400)),
            (OPT_SERVER_ID, &GW_IP.octets()),
        ],
    )
}

/// OFFER tối thiểu mà client cần: `yiaddr` + server id.
fn offer_payload(xid: u32, mac: MacAddr, yiaddr: Ipv4Addr) -> Vec<u8> {
    reply_payload(
        xid,
        mac,
        yiaddr,
        &[
            (OPT_MSG_TYPE, &[DHCPOFFER]),
            (OPT_SERVER_ID, &GW_IP.octets()),
        ],
    )
}

/// Bọc payload BOOTP vào Ethernet + IPv4 + UDP **có checksum thật** (chiều server → client).
fn server_frame(payload: &[u8]) -> Vec<u8> {
    let mut frame = vec![0u8; 720];
    let len = vifast_plat_x86_64::virtio_net::udp_frame(
        GW_MAC,
        OUR_MAC,
        GW_IP,
        OUR_IP,
        DHCP_SERVER_PORT,
        DHCP_CLIENT_PORT,
        payload,
        &mut frame,
    )
    .expect("frame server vừa đệm");
    frame.truncate(len);
    frame
}

// ---------------------------------------------------------------- fake IO

/// IO giả: phát frame theo hàng đợi, đồng hồ nhảy `step_us` mỗi lần `recv` không có frame.
struct FakeIo {
    rx: VecDeque<Vec<u8>>,
    now: u64,
    step_us: u64,
    now_calls: u32,
    recv_calls: u32,
    max_now_calls: u32,
    sent: Vec<Vec<u8>>,
    /// Timeout nhỏ nhất mà client đã truyền xuống `recv` (phải > 0 — 0 là "chờ mãi").
    min_recv_timeout: u64,
}

impl FakeIo {
    fn new(frames: Vec<Vec<u8>>) -> Self {
        Self {
            rx: frames.into(),
            now: 0,
            step_us: 10_000,
            now_calls: 0,
            recv_calls: 0,
            max_now_calls: 4096,
            sent: Vec::new(),
            min_recv_timeout: u64::MAX,
        }
    }
}

impl DhcpIo for FakeIo {
    fn now_us(&mut self) -> u64 {
        self.now_calls += 1;
        assert!(
            self.now_calls <= self.max_now_calls,
            "client đọc đồng hồ {} lần — không có trần thời gian?",
            self.now_calls
        );
        self.now
    }

    fn send(&mut self, frame: &[u8], timeout_us: u64) -> Result<(), NetError> {
        assert!(timeout_us > 0, "send với timeout 0 (chờ mãi)");
        self.sent.push(frame.to_vec());
        Ok(())
    }

    fn recv(&mut self, out: &mut [u8], timeout_us: u64) -> Option<usize> {
        self.recv_calls += 1;
        assert!(timeout_us > 0, "recv với timeout 0 (chờ mãi)");
        self.min_recv_timeout = self.min_recv_timeout.min(timeout_us);
        if let Some(frame) = self.rx.pop_front() {
            assert!(frame.len() <= out.len());
            out[..frame.len()].copy_from_slice(&frame);
            self.now += 1_000;
            return Some(frame.len());
        }
        self.now += self.step_us.min(timeout_us);
        None
    }
}

// ---------------------------------------------------------------- 1. khung DISCOVER

#[test]
fn dhcp_discover_frame_shape() {
    let discover = DhcpMessage::discover(DHCP_XID, OUR_MAC);
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
    assert_eq!(message.xid(), DHCP_XID);
    assert_eq!(message.chaddr(), OUR_MAC);
    assert_eq!(message.msg_type(), Some(1), "option 53 = DHCPDISCOVER");
}

// ---------------------------------------------------------------- 2. bộ lọc OFFER/ACK

#[test]
fn dhcp_frame_parse_matches_xid_mac_and_type() {
    let payload = offer_payload(DHCP_XID, OUR_MAC, OUR_IP);
    let frame = server_frame(&payload);

    let parsed = dhcp_frame_parse(&frame, DHCP_XID, OUR_MAC, DHCPOFFER).expect("phải khớp");
    assert_eq!(parsed.yiaddr(), OUR_IP);

    assert!(
        dhcp_frame_parse(&frame, DHCP_XID ^ 1, OUR_MAC, DHCPOFFER).is_none(),
        "xid khác không phải phiên của ta"
    );
    assert!(
        dhcp_frame_parse(&frame, DHCP_XID, GW_MAC, DHCPOFFER).is_none(),
        "chaddr khác là reply của máy khác"
    );
    assert!(
        dhcp_frame_parse(&frame, DHCP_XID, OUR_MAC, DHCPACK).is_none(),
        "msg_type khác chưa phải ACK"
    );
}

#[test]
fn dhcp_frame_parse_rejects_wrong_ports_and_short_frame() {
    // Frame ARP/ngắn: trả `None`, không PANIC.
    assert!(dhcp_frame_parse(&[0u8; 13], DHCP_XID, OUR_MAC, DHCPOFFER).is_none());
    // UDP sai cổng: dựng tay frame Ethernet + IPv4 + UDP 1234 → 68.
    let mut frame = vec![0u8; 720];
    let len = vifast_plat_x86_64::virtio_net::udp_frame(
        GW_MAC,
        OUR_MAC,
        GW_IP,
        OUR_IP,
        1234,
        DHCP_CLIENT_PORT,
        b"khong-phai-dhcp",
        &mut frame,
    )
    .expect("frame vừa đệm");
    assert!(dhcp_frame_parse(&frame[..len], DHCP_XID, OUR_MAC, DHCPOFFER).is_none());
}

// ---------------------------------------------------------------- 3. lease

#[test]
fn dhcp_lease_reads_ip_mask_gw_dns_and_lease() {
    let ack = DhcpMessage::parse(&ack_payload(DHCP_XID, OUR_MAC)).expect("ACK hợp lệ");
    let lease = dhcp_lease_from_ack(&ack, 2).expect("ACK đủ trường");
    assert_eq!(lease.ip, OUR_IP);
    assert_eq!(lease.mask, MASK);
    assert_eq!(lease.gw, GW_IP);
    assert_eq!(lease.dns, DNS_IP);
    assert_eq!(lease.server, GW_IP);
    assert_eq!(lease.lease_secs, 86_400);
    assert_eq!(lease.attempts, 2);
    assert!(lease.valid(), "lease slirp phải hợp lệ");
}

#[test]
fn dhcp_lease_rejects_missing_option_and_inconsistent_subnet() {
    // Thiếu router ⇒ từ chối (không đoán gateway).
    let missing_router = reply_payload(
        DHCP_XID,
        OUR_MAC,
        OUR_IP,
        &[
            (OPT_MSG_TYPE, &[DHCPACK]),
            (OPT_SUBNET_MASK, &MASK.octets()),
            (OPT_DNS, &DNS_IP.octets()),
        ],
    );
    let ack = DhcpMessage::parse(&missing_router).expect("khung hợp lệ");
    assert_eq!(dhcp_lease_from_ack(&ack, 1).err(), Some(NetError::Io));

    // Gateway ngoài mạng của `ip`/`mask` ⇒ lease không nhất quán.
    let bad_gw = reply_payload(
        DHCP_XID,
        OUR_MAC,
        OUR_IP,
        &[
            (OPT_MSG_TYPE, &[DHCPACK]),
            (OPT_SUBNET_MASK, &MASK.octets()),
            (OPT_ROUTER, &[10, 0, 3, 1]),
            (OPT_DNS, &DNS_IP.octets()),
            (OPT_LEASE_TIME, &lease_u32(3600)),
            (OPT_SERVER_ID, &GW_IP.octets()),
        ],
    );
    let ack = DhcpMessage::parse(&bad_gw).expect("khung hợp lệ");
    assert_eq!(dhcp_lease_from_ack(&ack, 1).err(), Some(NetError::Io));

    // `yiaddr = 0.0.0.0` ⇒ chưa được cấp địa chỉ.
    let zero = reply_payload(
        DHCP_XID,
        OUR_MAC,
        Ipv4Addr::UNSPECIFIED,
        &[
            (OPT_MSG_TYPE, &[DHCPACK]),
            (OPT_SUBNET_MASK, &MASK.octets()),
            (OPT_ROUTER, &GW_IP.octets()),
            (OPT_DNS, &DNS_IP.octets()),
            (OPT_LEASE_TIME, &lease_u32(3600)),
            (OPT_SERVER_ID, &GW_IP.octets()),
        ],
    );
    let ack = DhcpMessage::parse(&zero).expect("khung hợp lệ");
    assert_eq!(dhcp_lease_from_ack(&ack, 1).err(), Some(NetError::Io));
}

// ---------------------------------------------------------------- 4. self-check filter

#[test]
fn dhcp_selfcheck_rejects_bad_cookie_and_bad_xid() {
    let payload = ack_payload(DHCP_XID, OUR_MAC);
    let (cookie_ok, xid_ok) = dhcp_filter_selfcheck(OUR_MAC, DHCP_XID, &payload);
    assert!(cookie_ok, "bộ lọc phải từ chối ACK có magic cookie sai");
    assert!(xid_ok, "bộ lọc phải từ chối ACK có xid lạ");
}

// ---------------------------------------------------------------- 5. state machine

#[test]
fn dhcp_acquire_full_flow_offer_then_ack() {
    let frames = vec![
        server_frame(&offer_payload(DHCP_XID, OUR_MAC, OUR_IP)),
        server_frame(&ack_payload(DHCP_XID, OUR_MAC)),
    ];
    let mut io = FakeIo::new(frames);
    let lease = dhcp_acquire_with(
        &mut io,
        OUR_MAC,
        DHCP_XID,
        DHCP_TOTAL_TIMEOUT_US,
        DHCP_ATTEMPT_TIMEOUT_US,
        DHCP_MAX_ATTEMPTS,
    )
    .expect("phải lấy được lease");

    assert_eq!(lease.ip, OUR_IP);
    assert_eq!(lease.gw, GW_IP);
    assert_eq!(lease.dns, DNS_IP);
    assert!(lease.filter_ok());
    assert!(lease.attempts >= 2, "phải gửi DISCOVER và REQUEST");
    assert_eq!(io.sent.len(), 2, "mỗi pha một frame: DISCOVER + REQUEST");
    // Frame REQUEST phải xin đúng IP trong OFFER và đúng server id.
    let request = {
        let eth = EthernetFrame::parse(&io.sent[1]).expect("REQUEST hợp lệ");
        let ip = Ipv4Header::parse(eth.payload, true).expect("IPv4");
        let udp = UdpHeader::parse(ip.header.src, ip.header.dst, ip.payload(), true).expect("UDP");
        DhcpMessage::parse(udp.payload()).expect("BOOTP")
    };
    assert_eq!(request.msg_type(), Some(3), "REQUEST");
    assert_eq!(request.requested_ip(), Some(OUR_IP));
    assert_eq!(request.server_id(), Some(GW_IP));
}

#[test]
fn dhcp_acquire_skips_wrong_xid_offer() {
    // OFFER của **phiên khác** (xid+1, IP khác) đứng trước OFFER thật: client phải bỏ qua.
    let frames = vec![
        server_frame(&offer_payload(
            DHCP_XID ^ 1,
            OUR_MAC,
            Ipv4Addr::new(10, 0, 2, 99),
        )),
        server_frame(&offer_payload(DHCP_XID, OUR_MAC, OUR_IP)),
        server_frame(&ack_payload(DHCP_XID, OUR_MAC)),
    ];
    let mut io = FakeIo::new(frames);
    let lease = dhcp_acquire_with(
        &mut io,
        OUR_MAC,
        DHCP_XID,
        DHCP_TOTAL_TIMEOUT_US,
        DHCP_ATTEMPT_TIMEOUT_US,
        DHCP_MAX_ATTEMPTS,
    )
    .expect("phải bỏ qua OFFER lạ và nhận OFFER thật");
    assert_eq!(lease.ip, OUR_IP);
}

#[test]
fn dhcp_acquire_skips_bad_cookie_offer() {
    // OFFER có magic cookie sai (gói rác) đứng trước OFFER thật: client phải bỏ qua.
    let mut bad = offer_payload(DHCP_XID, OUR_MAC, Ipv4Addr::new(10, 0, 2, 99));
    bad[236] ^= 0xff;
    let frames = vec![
        server_frame(&bad),
        server_frame(&offer_payload(DHCP_XID, OUR_MAC, OUR_IP)),
        server_frame(&ack_payload(DHCP_XID, OUR_MAC)),
    ];
    let mut io = FakeIo::new(frames);
    let lease = dhcp_acquire_with(
        &mut io,
        OUR_MAC,
        DHCP_XID,
        DHCP_TOTAL_TIMEOUT_US,
        DHCP_ATTEMPT_TIMEOUT_US,
        DHCP_MAX_ATTEMPTS,
    )
    .expect("phải bỏ qua gói rác và nhận OFFER thật");
    assert_eq!(lease.ip, OUR_IP);
}

// ---------------------------------------------------------------- 6. trần thời gian

#[test]
fn dhcp_acquire_timeout_is_bounded() {
    // Không có frame nào: phải trả `Timeout` trong ngân sách **hữu hạn**.
    let mut io = FakeIo::new(Vec::new());
    let err = dhcp_acquire_with(&mut io, OUR_MAC, DHCP_XID, 300_000, 100_000, 3)
        .err()
        .expect("không có server phải lỗi");
    assert_eq!(err, NetError::Timeout);
    // Guard của `FakeIo` (4096 lần đọc đồng hồ) không được chạm: chứng minh có trần.
    assert!(
        io.now_calls < 100,
        "client đọc đồng hồ {} lần cho ngân sách 300 ms — có vòng lặp không trần?",
        io.now_calls
    );
    // Mỗi lần chờ đều có timeout > 0 (không truyền 0 = "chờ mãi").
    assert!(io.min_recv_timeout > 0);
}

#[test]
fn dhcp_acquire_zero_budget_does_not_wait() {
    // `timeout = 0` **không** được hiểu là "chờ mãi": phải fail-closed ngay (đột biến (iii)).
    let mut io = FakeIo::new(Vec::new());
    let err = dhcp_acquire_with(&mut io, OUR_MAC, DHCP_XID, 0, 0, 3)
        .err()
        .expect("ngân sách 0 phải lỗi ngay");
    assert_eq!(err, NetError::Timeout);
    assert_eq!(io.recv_calls, 0, "không được chờ frame khi ngân sách 0");
    assert_eq!(io.sent.len(), 0, "không được gửi frame khi ngân sách 0");
}

#[test]
fn dhcp_retry_budget_is_finite_and_attempts_bounded() {
    // Server **không** trả lời: số lần gửi bị chặn bởi `max_attempts` (không retry vô hạn).
    let mut io = FakeIo::new(Vec::new());
    let err = dhcp_acquire_with(&mut io, OUR_MAC, DHCP_XID, 600_000, 100_000, 2)
        .err()
        .expect("không có server phải lỗi");
    assert_eq!(err, NetError::Timeout);
    // 2 pha (DISCOVER + REQUEST) × 2 lần thử = tối đa 4 frame; dừng sớm cũng chấp nhận.
    assert!(
        io.sent.len() <= 4 && !io.sent.is_empty(),
        "số frame gửi {} vượt trần 2 pha × 2 lần thử",
        io.sent.len()
    );
}

// ---------------------------------------------------------------- 7. hằng số & lease
#[test]
fn dhcp_budget_constants_are_bounded() {
    assert!(DHCP_TOTAL_TIMEOUT_US > 0, "trần tổng phải > 0");
    assert!(
        DHCP_TOTAL_TIMEOUT_US <= 10_000_000,
        "trần tổng {} µs vượt ngân sách boot 20 s",
        DHCP_TOTAL_TIMEOUT_US
    );
    assert!(DHCP_ATTEMPT_TIMEOUT_US > 0);
    assert!(
        DHCP_ATTEMPT_TIMEOUT_US <= DHCP_TOTAL_TIMEOUT_US,
        "một lần thử không được dài hơn cả bước"
    );
    assert!(DHCP_MAX_ATTEMPTS >= 1 && DHCP_MAX_ATTEMPTS <= 4);

    // `valid()` là cổng của TAP 31: lease rác phải bị từ chối.
    let good = DhcpLease {
        ip: OUR_IP,
        mask: MASK,
        gw: GW_IP,
        dns: DNS_IP,
        server: GW_IP,
        lease_secs: 86_400,
        attempts: 2,
        cookie_rejected: true,
        xid_rejected: true,
    };
    assert!(good.valid() && good.filter_ok());

    let mut bad = good;
    bad.dns = Ipv4Addr::UNSPECIFIED;
    assert!(!bad.valid());
    let mut bad = good;
    bad.lease_secs = 0;
    assert!(!bad.valid());
    let mut bad = good;
    bad.mask = Ipv4Addr::new(255, 0, 255, 0); // mặt nạ không liên tục
    assert!(!bad.valid());
    let mut bad = good;
    bad.gw = Ipv4Addr::new(10, 0, 3, 1); // ngoài mạng /24
    assert!(!bad.valid());
    let mut bad = good;
    bad.cookie_rejected = false;
    assert!(!bad.filter_ok());
    let mut bad = good;
    bad.xid_rejected = false;
    assert!(!bad.filter_ok());
}
