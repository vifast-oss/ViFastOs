// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-335b — test **host** cho **chính sách** mạng dùng chung 3 arch (`vifast_net::client` /
//! `dhcp_client` / `dns_client`), D2-05: viết trước, đỏ rồi mới xanh.
//!
//! ```text
//! cargo test --manifest-path v2/Cargo.toml -p vifast-net --test net_client
//! ```
//!
//! # Vì sao test ở đây
//!
//! Trước KT-335b, chính sách DHCP/ARP/ICMP/UDP/DNS nằm **ba bản** trong
//! `vifast-plat-{riscv64,aarch64,x86_64}/src/virtio_net.rs` (+ `net_client.rs` của x86_64) và chỉ
//! chạy được trên QEMU. Sau KT-335b chính sách là hàm thuần trên hai trait
//! [`Clock`]/[`Link`] (đã có từ KT-335) + [`RetryBudget`] chung; test cài bằng **đường dây giả
//! trên byte thật** (encode/parse qua chính codec của crate) nên mỗi ca dưới đây chứng minh đúng
//! một tính chất giao thức.
//!
//! # Hai ca giữ **hợp đồng D2-246** (bắt buộc)
//!
//! - [`dns_query_skips_bad_reply_then_accepts_good_one`]: một reply hỏng/không khớp **không** bỏ
//!   cả truy vấn — đếm `bad_replies` rồi **thử lại trong ngân sách**.
//! - [`dns_query_all_replies_bad_is_badframe`]: hết ngân sách/lượt mà **vẫn chỉ** thấy gói hỏng
//!   ⇒ `BadFrame`; không thấy gói nào ⇒ `Timeout`.
//!
//! Đây là hợp đồng đã vá flake soak `1/600` boot x86_64 (D2-246, KT-335c). Ca đột biến (ii) của
//! KT-335b (bỏ retry, quay lại `?` như bản cũ) **phải** làm ca thứ nhất ĐỎ — nếu không, hợp đồng
//! chỉ còn trên giấy.
//!
//! # Ca đột biến (i) — checksum UDP
//!
//! [`udp_recv_ignores_bad_checksum`] bơm frame UDP **có checksum sai** đứng trước frame đúng;
//! client phải **bỏ** frame sai (không nhận "gần đúng"). Đột biến "bỏ kiểm checksum"
//! (`UdpHeader::parse(…, true)` → `false`) làm ca này ĐỎ vì payload nhận được là của gói độc.

use std::cell::Cell;
use std::collections::VecDeque;
use std::rc::Rc;

use vifast_net::addr::{Ipv4Addr, MacAddr};
use vifast_net::client::{udp_frame, udp_recv_until, Clock, Link, LinkError, NetPeer, RetryBudget};
use vifast_net::dhcp_client::dhcp_acquire_with;
use vifast_net::dns::{DnsQuery, DnsRetry, CLASS_IN, TYPE_A};
use vifast_net::dns_client::{dns_query_with, DNS_CLIENT_PORT, DNS_MAX_LEN, DNS_PORT};
use vifast_net::eth::ETHERTYPE_IPV4;
use vifast_net::frame::EthernetFrame;
use vifast_net::icmp_client::ping_with;
use vifast_net::ipv4::{Ipv4Header, PROTO_UDP};
use vifast_net::udp::UdpHeader;

/// MAC của ta (giá trị QEMU cấp cho `-device virtio-net-pci`).
const OUR_MAC: MacAddr = MacAddr::new([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]);
/// MAC server (slirp suy MAC từ địa chỉ).
const SRV_MAC: MacAddr = MacAddr::new([0x52, 0x55, 0x0a, 0x00, 0x02, 0x03]);
/// IP của ta (slirp cấp qua DHCP).
const OUR_IP: Ipv4Addr = Ipv4Addr::new(10, 0, 2, 15);
/// IP server (DNS của slirp).
const SRV_IP: Ipv4Addr = Ipv4Addr::new(10, 0, 2, 3);
/// Tên **cục bộ** mà resolver luôn trả cùng một địa chỉ.
const NAME: &[u8] = b"localhost";
/// Địa chỉ **kỳ vọng** cho [`NAME`] — hằng của phép kiểm, không phải "địa chỉ bất kỳ".
const EXPECT_A: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 1);
/// `id` của truy vấn DNS.
const DNS_ID: u16 = 0x4b55;
/// Hai đầu của bước mạng.
const PEER: NetPeer = NetPeer::new(OUR_IP, OUR_MAC, SRV_IP, SRV_MAC);

// ---------------------------------------------------------------- đường dây giả

/// Đồng hồ giả chia sẻ với [`Wire`] — `recv_frame` **tiến thời gian** khi hàng đợi rỗng, đúng như
/// driver thật (nó chờ trong hạn rồi mới trả `None`).
#[derive(Clone)]
struct FakeClock(Rc<Cell<u64>>);

impl Clock for FakeClock {
    fn now(&self) -> u64 {
        self.0.get()
    }
}

/// Đường dây giả: phát frame theo hàng đợi, ghi lại frame đã gửi, và tiến đồng hồ `step` mỗi lần
/// chờ hết hạn. Không mock codec: frame được dựng/đọc bằng chính `vifast-net`.
struct Wire {
    time: Rc<Cell<u64>>,
    rx: VecDeque<Vec<u8>>,
    sent: Vec<Vec<u8>>,
    step: u64,
}

impl Wire {
    fn new(frames: Vec<Vec<u8>>) -> (Self, FakeClock) {
        let time = Rc::new(Cell::new(0));
        let wire = Self {
            time: Rc::clone(&time),
            rx: frames.into(),
            sent: Vec::new(),
            step: 1_000,
        };
        (wire, FakeClock(time))
    }
}

impl Link for Wire {
    fn send_frame(&mut self, frame: &[u8]) -> Result<(), LinkError> {
        self.sent.push(frame.to_vec());
        Ok(())
    }

    fn recv_frame(&mut self, out: &mut [u8], timeout: u64) -> Option<usize> {
        if let Some(frame) = self.rx.pop_front() {
            let len = frame.len().min(out.len());
            out[..len].copy_from_slice(&frame[..len]);
            return Some(len);
        }
        // Hết hàng đợi: chờ đúng `min(step, timeout)` rồi báo "chưa có" — không bao giờ trả 0.
        let advance = self.step.min(timeout);
        self.time.set(self.time.get() + advance);
        None
    }
}

// ---------------------------------------------------------------- dựng frame

/// Bọc payload UDP vào Ethernet + IPv4 + UDP **có checksum thật** (chiều server → client).
fn server_udp_frame(
    src_mac: MacAddr,
    src_ip: Ipv4Addr,
    src_port: u16,
    dst_port: u16,
    payload: &[u8],
) -> Vec<u8> {
    let mut frame = vec![0u8; 256];
    let len = udp_frame(
        src_mac, OUR_MAC, src_ip, OUR_IP, src_port, dst_port, payload, &mut frame,
    )
    .expect("frame server vừa đệm");
    frame.truncate(len);
    frame
}

/// Dựng **một** message reply DNS `A` từ chính query (RFC 1035 §4.1.2: server chép question).
fn reply_message(id: u16, name: &[u8], flags: u16, answer: Option<Ipv4Addr>) -> Vec<u8> {
    let query = DnsQuery::new(id, name, TYPE_A).expect("query hợp lệ");
    let mut message = vec![0u8; DNS_MAX_LEN];
    let len = query.encode(&mut message).expect("query vừa đệm");
    // Header: biến query thành response (`QR=1`) với `flags` cho trước, `ANCOUNT` theo `answer`.
    message[2..4].copy_from_slice(&flags.to_be_bytes());
    let ancount: u16 = answer.is_some() as u16;
    message[6..8].copy_from_slice(&ancount.to_be_bytes());
    message.truncate(len);
    if let Some(addr) = answer {
        message.extend_from_slice(&[0xc0, 0x0c]); // trỏ về name của question
        message.extend_from_slice(&TYPE_A.to_be_bytes());
        message.extend_from_slice(&CLASS_IN.to_be_bytes());
        message.extend_from_slice(&3600u32.to_be_bytes());
        message.extend_from_slice(&4u16.to_be_bytes());
        message.extend_from_slice(&addr.octets());
    }
    message
}

/// Reply hợp lệ: `id` đúng, `QR=1`, `RCODE=0`, một bản ghi `A` = [`EXPECT_A`].
fn good_reply() -> Vec<u8> {
    reply_message(DNS_ID, NAME, 0x8180, Some(EXPECT_A))
}

// ================================================================ 1. UDP + checksum

/// **Đột biến (i) của KT-335b** — frame UDP có checksum **sai** phải bị bỏ, kể cả khi nó tới
/// **trước** frame đúng trong cùng hạn. Nếu bỏ phép kiểm checksum (`UdpHeader::parse(…, false)`),
/// client nhận gói độc và ca này ĐỎ (payload khác `vifast-udp-ok`).
#[test]
fn udp_recv_ignores_bad_checksum() {
    let good = b"vifast-udp-ok";
    let poison = b"poison-payload";
    // Gói độc: payload **khác**, checksum bị đổi một byte (byte thứ 7 của header UDP).
    let mut bad = server_udp_frame(SRV_MAC, SRV_IP, 7, 9, poison);
    bad[14 + 20 + 6] ^= 0xff;
    let good_frame = server_udp_frame(SRV_MAC, SRV_IP, 7, 9, good);
    let (mut wire, clock) = Wire::new(vec![bad.clone(), good_frame]);

    let mut out = [0u8; 32];
    let deadline = clock.now() + 1_000_000;
    let n = udp_recv_until(&clock, &mut wire, OUR_IP, SRV_IP, 7, 9, &mut out, deadline)
        .expect("frame đúng phía sau frame hỏng phải được nhận");
    assert_eq!(
        &out[..n],
        good,
        "đã nhận payload của gói checksum-sai (bỏ kiểm checksum?)"
    );

    // Chỉ có gói độc ⇒ hết hạn, **không** nhận bừa.
    let (mut wire, clock) = Wire::new(vec![bad]);
    let deadline = clock.now() + 50_000;
    assert_eq!(
        udp_recv_until(&clock, &mut wire, OUR_IP, SRV_IP, 7, 9, &mut out, deadline),
        Err(LinkError::Timeout)
    );
}

/// `udp_recv_until` trả `BadFrame` khi payload dài hơn đệm người gọi (fail-closed, không cắt).
#[test]
fn udp_recv_rejects_payload_larger_than_buffer() {
    let frame = server_udp_frame(SRV_MAC, SRV_IP, 7, 9, b"0123456789");
    let (mut wire, clock) = Wire::new(vec![frame]);
    let mut out = [0u8; 4];
    let deadline = clock.now() + 1_000_000;
    assert_eq!(
        udp_recv_until(&clock, &mut wire, OUR_IP, SRV_IP, 7, 9, &mut out, deadline),
        Err(LinkError::BadFrame)
    );
}

// ================================================================ 2. DNS — D2-246

/// **D2-246 (KT-335c), port sang `vifast-net`** — một gói reply **sai/không khớp** không kết luận
/// gì về cả truy vấn; client **bỏ gói đó rồi thử lại** trong cùng ngân sách.
///
/// Đây là ca mà đột biến (ii) của KT-335b (bỏ retry, quay lại `?` như bản cũ) **phải** làm ĐỎ.
#[test]
fn dns_query_skips_bad_reply_then_accepts_good_one() {
    let stale = reply_message(DNS_ID ^ 1, NAME, 0x8180, Some(EXPECT_A));
    let (mut wire, clock) = Wire::new(vec![
        server_udp_frame(SRV_MAC, SRV_IP, DNS_PORT, DNS_CLIENT_PORT, &stale),
        server_udp_frame(SRV_MAC, SRV_IP, DNS_PORT, DNS_CLIENT_PORT, &good_reply()),
    ]);
    let budget = RetryBudget::new(6_000_000, 3_000_000, 2);
    let mut retry = DnsRetry::new();
    let mut reply = [0u8; DNS_MAX_LEN];
    let answer = dns_query_with(
        &clock,
        &mut wire,
        PEER,
        DNS_ID,
        NAME,
        Some(EXPECT_A),
        budget,
        &mut reply,
        &mut retry,
    )
    .expect("gói xấu đầu tiên KHÔNG được bỏ cả truy vấn (D2-246)");
    assert_eq!(answer.a.expect("reply phải có bản ghi A").addr, EXPECT_A);
    assert_eq!(answer.bad_replies, 1, "đếm đúng một gói bị bỏ");
    assert_eq!(answer.attempts, 2, "gói xấu ⇒ gửi lại, tốn đúng một lượt");
    assert_eq!(wire.sent.len(), 2, "đúng hai lần gửi");
}

/// **D2-246** — nếu **mọi** reply đều hỏng thì vẫn là `BadFrame` (không im lặng trả `Timeout`),
/// và bị chặn bởi trần số lần gửi (không quay vô hạn).
#[test]
fn dns_query_all_replies_bad_is_badframe() {
    let bad_a = reply_message(DNS_ID ^ 1, NAME, 0x8180, Some(EXPECT_A));
    let bad_b = reply_message(DNS_ID, NAME, 0x8183, Some(EXPECT_A));
    let frames = [&bad_a, &bad_b, &bad_a, &bad_b]
        .iter()
        .map(|message| server_udp_frame(SRV_MAC, SRV_IP, DNS_PORT, DNS_CLIENT_PORT, message))
        .collect();
    let (mut wire, clock) = Wire::new(frames);
    let budget = RetryBudget::new(6_000_000, 3_000_000, 2);
    let mut retry = DnsRetry::new();
    let mut reply = [0u8; DNS_MAX_LEN];
    let error = dns_query_with(
        &clock,
        &mut wire,
        PEER,
        DNS_ID,
        NAME,
        Some(EXPECT_A),
        budget,
        &mut reply,
        &mut retry,
    )
    .expect_err("mọi reply đều hỏng ⇒ BadFrame");
    assert_eq!(error, LinkError::BadFrame);
    assert!(
        wire.sent.len() <= 2,
        "bị chặn bởi trần số lần gửi, không quay vô hạn (gửi {} lần)",
        wire.sent.len()
    );
}

/// Không thấy reply nào ⇒ `Timeout` (không phải `BadFrame`), số lần gửi bị chặn bởi ngân sách.
#[test]
fn dns_query_without_reply_times_out_bounded() {
    let (mut wire, clock) = Wire::new(Vec::new());
    let budget = RetryBudget::new(6_000_000, 3_000_000, 2);
    let mut retry = DnsRetry::new();
    let mut reply = [0u8; DNS_MAX_LEN];
    let error = dns_query_with(
        &clock,
        &mut wire,
        PEER,
        DNS_ID,
        NAME,
        Some(EXPECT_A),
        budget,
        &mut reply,
        &mut retry,
    )
    .expect_err("không có reply ⇒ Timeout");
    assert_eq!(error, LinkError::Timeout);
    assert_eq!(wire.sent.len(), 2, "tối đa hai lần gửi");
    assert!(clock.now() <= 6_000_000, "không vượt trần toàn bước");
}

/// Ngân sách **0** ⇒ `Timeout` **ngay**, không gửi gì (0 **không** phải "vô hạn").
#[test]
fn dns_query_zero_budget_does_not_send() {
    let (mut wire, clock) = Wire::new(vec![server_udp_frame(
        SRV_MAC,
        SRV_IP,
        DNS_PORT,
        DNS_CLIENT_PORT,
        &good_reply(),
    )]);
    let mut retry = DnsRetry::new();
    let mut reply = [0u8; DNS_MAX_LEN];
    let error = dns_query_with(
        &clock,
        &mut wire,
        PEER,
        DNS_ID,
        NAME,
        Some(EXPECT_A),
        RetryBudget::new(0, 0, 0),
        &mut reply,
        &mut retry,
    )
    .expect_err("ngân sách 0 ⇒ Timeout ngay");
    assert_eq!(error, LinkError::Timeout);
    assert!(wire.sent.is_empty(), "không được gửi gì");
}

// ================================================================ 3. trần chung

/// DHCP với ngân sách **0** cũng phải `Timeout` ngay, không gửi DISCOVER.
#[test]
fn dhcp_zero_budget_does_not_send() {
    let (mut wire, clock) = Wire::new(Vec::new());
    let error = dhcp_acquire_with(
        &clock,
        &mut wire,
        OUR_MAC,
        0x4b54_3332,
        RetryBudget::new(0, 0, 0),
    )
    .expect_err("ngân sách 0 ⇒ Timeout ngay");
    assert_eq!(error, LinkError::Timeout);
    assert!(wire.sent.is_empty(), "0 không phải 'vô hạn'");
}

/// ICMP với ngân sách **0** cũng phải `Timeout` ngay, không gửi echo request.
#[test]
fn ping_zero_budget_does_not_send() {
    let (mut wire, clock) = Wire::new(Vec::new());
    let error = ping_with(&clock, &mut wire, PEER, 1, RetryBudget::new(0, 0, 0))
        .expect_err("ngân sách 0 ⇒ Timeout ngay");
    assert_eq!(error, LinkError::Timeout);
    assert!(wire.sent.is_empty(), "0 không phải 'vô hạn'");
}

/// ETHERTYPE_IPV4 phải là hằng của codec chứ không phải số chép tay trong ca UDP ở trên.
#[test]
fn constants_come_from_codec() {
    let frame = server_udp_frame(SRV_MAC, SRV_IP, 7, 9, b"x");
    let eth = EthernetFrame::parse(&frame).expect("frame hợp lệ");
    assert_eq!(eth.header.ethertype, ETHERTYPE_IPV4);
    let ip = Ipv4Header::parse(eth.payload, true).expect("IPv4 hợp lệ");
    assert_eq!(ip.header.protocol, PROTO_UDP);
    let udp = UdpHeader::parse(ip.header.src, ip.header.dst, ip.payload(), true)
        .expect("UDP hợp lệ (checksum đúng)");
    assert_eq!(udp.payload(), b"x");
}
