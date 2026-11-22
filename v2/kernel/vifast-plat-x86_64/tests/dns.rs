// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho DNS client của `vifast_plat_x86_64::virtio_net` — KT-328 (K5' — DNS x86_64).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-x86_64 --test dns
//! ```
//!
//! # Vì sao DNS cần test host dù slirp **có** trả lời truy vấn
//!
//! Slirp luôn trả lời `A` cho tên ta hỏi, nên client **không kiểm gì** vẫn xanh trên QEMU. Bốn
//! lớp lỗi dưới đây chỉ bắt được bằng test host:
//!
//! 1. **Khung truy vấn sai** (cổng/địa chỉ/`id`/`qtype`/`qclass`) — QEMU có thể vẫn trả lời một
//!    truy vấn gần đúng. Test đọc lại frame đã gửi bằng codec `vifast-net`.
//! 2. **Nhận reply của người khác** (`id` lệch) hoặc reply **không phải response** (`QR=0`).
//! 3. **Nhận reply có lỗi giao thức** (`RCODE≠0`, không có bản ghi `A`) hoặc **địa chỉ khác hằng
//!    kỳ vọng** — đột biến (iii)/(iv) của KT-328 (bỏ so `id` / bỏ kiểm `RCODE`) làm test này ĐỎ.
//! 4. **Không có trần thời gian** — `dns_query_with` phải trả `Timeout` sau ngân sách **hữu hạn**
//!    (tối đa `DNS_MAX_ATTEMPTS` lần gửi, mỗi lần có ngân sách riêng), kể cả khi ngân sách = 0.
//!
//! `dns_filter_selfcheck` là phép kiểm **chạy trong ảnh** (TAP 33): nó bơm lại **chính** reply
//! nhận được với `id` sai, `QR` bị xoá và `RCODE=3`, rồi đòi bộ lọc từ chối — nhờ vậy đột biến
//! (iii)/(iv) làm **QEMU ĐỎ**, không chỉ test host.

use std::collections::VecDeque;

use vifast_net::addr::{Ipv4Addr, MacAddr};
use vifast_net::dns::{DnsMessage, DnsQuery, CLASS_IN, TYPE_A};
use vifast_net::frame::EthernetFrame;
use vifast_net::ipv4::{Ipv4Header, PROTO_UDP};
use vifast_net::udp::UdpHeader;
use vifast_plat_x86_64::virtio_net::{
    dns_filter_selfcheck, dns_query_with, dns_reply_accept, udp_frame, NetIo, NetPeer, RetryBudget,
    DNS_ATTEMPT_TIMEOUT_US, DNS_CLIENT_PORT, DNS_ID, DNS_MAX_LEN, DNS_PORT, DNS_TOTAL_TIMEOUT_US,
};
use vifast_virtio::net::NetError;

/// MAC của ta (giá trị QEMU cấp cho `-device virtio-net-pci`).
const OUR_MAC: MacAddr = MacAddr::new([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]);
/// MAC DNS server slirp (`10.0.2.3`) — slirp dùng MAC suy từ địa chỉ.
const DNS_MAC: MacAddr = MacAddr::new([0x52, 0x55, 0x0a, 0x00, 0x02, 0x03]);
/// IP slirp cấp.
const OUR_IP: Ipv4Addr = Ipv4Addr::new(10, 0, 2, 15);
/// DNS server của slirp.
const DNS_IP: Ipv4Addr = Ipv4Addr::new(10, 0, 2, 3);
/// Tên được hỏi — tên **cục bộ** mà resolver của host luôn trả cùng một địa chỉ.
const NAME: &[u8] = b"localhost";
/// Địa chỉ kỳ vọng cho [`NAME`] (hằng của phép kiểm, không phải "địa chỉ bất kỳ").
const EXPECT_A: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 1);

const PEER: NetPeer = NetPeer::new(OUR_IP, OUR_MAC, DNS_IP, DNS_MAC);
const BUDGET: RetryBudget = RetryBudget::new(DNS_TOTAL_TIMEOUT_US, DNS_ATTEMPT_TIMEOUT_US, 2);

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

/// Bọc message DNS vào Ethernet + IPv4 + UDP **có checksum thật** (chiều server → client).
fn server_frame(message: &[u8]) -> Vec<u8> {
    let mut frame = vec![0u8; DNS_MAX_LEN + 64];
    let len = udp_frame(
        DNS_MAC,
        OUR_MAC,
        DNS_IP,
        OUR_IP,
        DNS_PORT,
        DNS_CLIENT_PORT,
        message,
        &mut frame,
    )
    .expect("frame server vừa đệm");
    frame.truncate(len);
    frame
}

// ---------------------------------------------------------------- fake IO

/// IO giả: phát frame theo hàng đợi, đồng hồ nhảy `step_us` mỗi lần `recv` có/không có frame.
struct FakeIo {
    rx: VecDeque<Vec<u8>>,
    now: u64,
    step_us: u64,
    now_calls: u32,
    max_now_calls: u32,
    sent: Vec<Vec<u8>>,
    min_recv_timeout: u64,
}

impl FakeIo {
    fn new(frames: Vec<Vec<u8>>) -> Self {
        Self {
            rx: frames.into(),
            now: 0,
            step_us: 2_000,
            now_calls: 0,
            max_now_calls: 4096,
            sent: Vec::new(),
            min_recv_timeout: u64::MAX,
        }
    }
}

impl NetIo for FakeIo {
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
        assert!(timeout_us > 0, "recv với timeout 0 (chờ mãi)");
        self.min_recv_timeout = self.min_recv_timeout.min(timeout_us);
        if let Some(frame) = self.rx.pop_front() {
            assert!(frame.len() <= out.len());
            out[..frame.len()].copy_from_slice(&frame);
            self.now += 100;
            return Some(frame.len());
        }
        self.now += self.step_us.min(timeout_us);
        None
    }
}

// ---------------------------------------------------------------- 1. khung truy vấn

#[test]
fn dns_query_frame_shape() {
    let mut io = FakeIo::new(vec![server_frame(&good_reply())]);
    let mut reply = [0u8; DNS_MAX_LEN];
    let answer = dns_query_with(&mut io, PEER, DNS_ID, NAME, EXPECT_A, BUDGET, &mut reply)
        .expect("phải nhận được reply");

    let frame = io.sent.first().expect("đã gửi một truy vấn");
    let eth = EthernetFrame::parse(frame).expect("frame hợp lệ");
    assert_eq!(eth.header.src, OUR_MAC);
    assert_eq!(eth.header.dst, DNS_MAC);
    assert_eq!(eth.header.ethertype, vifast_net::eth::ETHERTYPE_IPV4);

    let ip = Ipv4Header::parse(eth.payload, true).expect("IPv4 hợp lệ (checksum thật)");
    assert_eq!(ip.header.protocol, PROTO_UDP);
    assert_eq!(ip.header.src, OUR_IP);
    assert_eq!(ip.header.dst, DNS_IP);

    let udp = UdpHeader::parse(ip.header.src, ip.header.dst, ip.payload(), true)
        .expect("UDP hợp lệ (checksum có pseudo-header)");
    assert_eq!(udp.header.src_port, DNS_CLIENT_PORT);
    assert_eq!(udp.header.dst_port, DNS_PORT);

    let message = DnsMessage::parse(udp.payload()).expect("DNS message hợp lệ");
    assert_eq!(message.header().id, DNS_ID, "id trong truy vấn");
    assert!(
        !message.header().is_response(),
        "truy vấn không phải response"
    );
    assert_eq!(message.header().qdcount, 1);
    assert_eq!(message.header().ancount, 0);
    let question = message.question().expect("parse được").expect("có câu hỏi");
    assert_eq!(question.qtype, TYPE_A);
    assert_eq!(question.qclass, CLASS_IN);
    assert!(question.name.eq_ignore_ascii_case(NAME));
    assert_eq!(answer.attempts, 1);
}

// ---------------------------------------------------------------- 2. bộ lọc reply

#[test]
fn dns_reply_accept_returns_noerror_a() {
    let reply = good_reply();
    let parsed = dns_reply_accept(&reply, DNS_ID, NAME, EXPECT_A).expect("reply hợp lệ");
    assert_eq!(parsed.id, DNS_ID);
    assert_eq!(parsed.rcode, 0);
    assert_eq!(parsed.a.addr, EXPECT_A);
    assert_eq!(parsed.question.qtype, TYPE_A);
    assert!(parsed.question.name.eq_ignore_ascii_case(NAME));
}

#[test]
fn dns_reply_accept_rejects_wrong_id() {
    // **Đột biến (iii)** — bỏ so `id` thì reply của người khác được nhận.
    let reply = reply_message(DNS_ID ^ 1, NAME, 0x8180, Some(EXPECT_A));
    assert_eq!(
        dns_reply_accept(&reply, DNS_ID, NAME, EXPECT_A),
        Err(NetError::BadFrame)
    );
}

#[test]
fn dns_reply_accept_rejects_not_response() {
    // `QR=0`: một truy vấn (của ta, bị phản chiếu) không phải reply.
    let reply = reply_message(DNS_ID, NAME, 0x0100, None);
    assert_eq!(
        dns_reply_accept(&reply, DNS_ID, NAME, EXPECT_A),
        Err(NetError::BadFrame)
    );
}

#[test]
fn dns_reply_accept_rejects_nonzero_rcode() {
    // **Đột biến (iv)** — bỏ kiểm `RCODE` thì NXDOMAIN được nhận như thành công.
    let reply = reply_message(DNS_ID, NAME, 0x8183, None);
    assert_eq!(
        dns_reply_accept(&reply, DNS_ID, NAME, EXPECT_A),
        Err(NetError::BadFrame)
    );
}

#[test]
fn dns_reply_accept_rejects_wrong_address() {
    // Địa chỉ **khác** hằng kỳ vọng: đúng `RCODE=0` nhưng không phải câu trả lời ta kiểm.
    let reply = reply_message(DNS_ID, NAME, 0x8180, Some(Ipv4Addr::new(192, 0, 2, 99)));
    assert_eq!(
        dns_reply_accept(&reply, DNS_ID, NAME, EXPECT_A),
        Err(NetError::BadFrame)
    );
    // Không có bản ghi `A` (NODATA) cũng không phải thành công.
    let reply = reply_message(DNS_ID, NAME, 0x8180, None);
    assert_eq!(
        dns_reply_accept(&reply, DNS_ID, NAME, EXPECT_A),
        Err(NetError::BadFrame)
    );
}

#[test]
fn dns_reply_accept_rejects_other_question() {
    let reply = reply_message(DNS_ID, b"example.com", 0x8180, Some(EXPECT_A));
    assert_eq!(
        dns_reply_accept(&reply, DNS_ID, NAME, EXPECT_A),
        Err(NetError::BadFrame)
    );
}

#[test]
fn dns_filter_selfcheck_detects_id_qr_rcode() {
    let reply = good_reply();
    assert_eq!(
        dns_filter_selfcheck(&reply, DNS_ID, NAME, EXPECT_A),
        (true, true, true),
        "cả ba đột biến (id/QR/RCODE) phải bị bộ lọc từ chối"
    );
}

// ---------------------------------------------------------------- 3. bước query + trần

#[test]
fn dns_query_accepts_reply() {
    let mut io = FakeIo::new(vec![server_frame(&good_reply())]);
    let mut reply = [0u8; DNS_MAX_LEN];
    let answer = dns_query_with(&mut io, PEER, DNS_ID, NAME, EXPECT_A, BUDGET, &mut reply)
        .expect("phải nhận được reply");
    assert_eq!(answer.reply.id, DNS_ID);
    assert_eq!(answer.reply.rcode, 0);
    assert_eq!(answer.reply.a.addr, EXPECT_A);
    assert_eq!(answer.attempts, 1);
    assert!(answer.reply_len > 0 && answer.reply_len <= DNS_MAX_LEN);
    assert!(answer.filter_id && answer.filter_qr && answer.filter_rcode);
    assert_eq!(io.sent.len(), 1);
}

#[test]
fn dns_query_wrong_id_reply_is_a_protocol_error_not_a_retry() {
    let message = reply_message(DNS_ID ^ 1, NAME, 0x8180, Some(EXPECT_A));
    let mut io = FakeIo::new(vec![server_frame(&message)]);
    let mut reply = [0u8; DNS_MAX_LEN];
    let error = dns_query_with(&mut io, PEER, DNS_ID, NAME, EXPECT_A, BUDGET, &mut reply)
        .expect_err("reply id lạ là lỗi giao thức");
    assert_eq!(error, NetError::BadFrame);
    assert_eq!(io.sent.len(), 1, "reply hỏng **không** được thử lại");
}

#[test]
fn dns_query_retries_on_timeout_then_fails_bounded() {
    let mut io = FakeIo::new(Vec::new());
    let mut reply = [0u8; DNS_MAX_LEN];
    let error = dns_query_with(&mut io, PEER, DNS_ID, NAME, EXPECT_A, BUDGET, &mut reply)
        .expect_err("không có reply ⇒ Timeout");
    assert_eq!(error, NetError::Timeout);
    assert_eq!(io.sent.len(), 2, "tối đa DNS_MAX_ATTEMPTS lần gửi");
    assert!(io.min_recv_timeout > 0 && io.min_recv_timeout != u64::MAX);
}

#[test]
fn dns_query_zero_budget_does_not_send() {
    let mut io = FakeIo::new(vec![server_frame(&good_reply())]);
    let mut reply = [0u8; DNS_MAX_LEN];
    let error = dns_query_with(
        &mut io,
        PEER,
        DNS_ID,
        NAME,
        EXPECT_A,
        RetryBudget::new(0, 0, 0),
        &mut reply,
    )
    .expect_err("ngân sách 0 ⇒ Timeout ngay");
    assert_eq!(error, NetError::Timeout);
    assert!(
        io.sent.is_empty(),
        "0 không phải 'vô hạn' — không được gửi gì"
    );
    assert!(io.now_calls <= 2);
}

#[test]
fn dns_query_total_budget_is_bounded() {
    let mut io = FakeIo::new(Vec::new());
    let mut reply = [0u8; DNS_MAX_LEN];
    let error = dns_query_with(&mut io, PEER, DNS_ID, NAME, EXPECT_A, BUDGET, &mut reply)
        .expect_err("không có reply ⇒ Timeout");
    assert_eq!(error, NetError::Timeout);
    assert_eq!(
        io.now,
        DNS_ATTEMPT_TIMEOUT_US * 2,
        "hai lần thử, mỗi lần đúng ngân sách {} µs (không cộng dồn quá trần)",
        DNS_ATTEMPT_TIMEOUT_US
    );
    assert!(
        io.now <= DNS_TOTAL_TIMEOUT_US,
        "tổng {} µs vượt trần toàn bước {} µs",
        io.now,
        DNS_TOTAL_TIMEOUT_US
    );
}
