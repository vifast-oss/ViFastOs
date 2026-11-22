// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho ICMP echo của `vifast_plat_x86_64::virtio_net` — KT-328 (K5' — ICMP x86_64).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-x86_64 --test icmp
//! ```
//!
//! # Vì sao ICMP echo cần test host dù slirp **có** trả lời ping
//!
//! Slirp trả lời echo request tới `10.0.2.2`, nên một client **không kiểm gì** vẫn xanh trên
//! QEMU. Ba lớp lỗi dưới đây chỉ bắt được bằng test host:
//!
//! 1. **Khung dựng sai** (ethertype/địa chỉ/protocol/checksum/`id`/`seq`) — QEMU vẫn trả lời một
//!    khung gần đúng, nhưng khung không đúng RFC 792. Test đọc lại bằng codec `vifast-net`
//!    (`IcmpEcho::parse` kiểm checksum **thật**).
//! 2. **Nhận reply của người khác** — `id`/`seq` lệch **phải** bị bỏ qua, và **payload** khác
//!    cũng phải bị bỏ qua (RFC 792: dữ liệu được chép nguyên). Đột biến (i)/(ii) của KT-328 (bỏ
//!    kiểm `id`/`seq`) làm test này ĐỎ.
//! 3. **Không có trần thời gian** — `ping` phải trả `Timeout` sau ngân sách **hữu hạn**, kể cả
//!    khi ngân sách bằng 0 (0 **không** được hiểu là "chờ mãi"). Guard `max_now_calls` biến
//!    "treo" thành ĐỎ thay vì treo cả `cargo test`.
//!
//! Ngoài ra `icmp_filter_selfcheck` là phép kiểm **chạy trong ảnh** (TAP 32): nó dựng reply hợp
//! lệ rồi dựng lại với `id`/`seq`/`payload` sai (checksum **đúng**) và đòi bộ lọc từ chối — nhờ
//! vậy đột biến (i)/(ii) làm **QEMU ĐỎ**, không chỉ test host.

use std::collections::VecDeque;

use vifast_net::addr::{Ipv4Addr, MacAddr};
use vifast_net::frame::EthernetFrame;
use vifast_net::icmp::{IcmpEcho, IcmpMessage};
use vifast_net::ipv4::{Ipv4Header, PROTO_ICMP};
use vifast_plat_x86_64::virtio_net::{
    icmp_echo_frame, icmp_filter_selfcheck, icmp_reply_accept, icmp_reply_frame, ping_with, NetIo,
    NetPeer, RetryBudget, ICMP_ATTEMPT_TIMEOUT_US, ICMP_ECHO_ID, ICMP_ECHO_SEQ, ICMP_MAX_ATTEMPTS,
    ICMP_PAYLOAD_BYTES, ICMP_TOTAL_TIMEOUT_US,
};
use vifast_virtio::net::NetError;

/// MAC của ta (giá trị QEMU cấp cho `-device virtio-net-pci`).
const OUR_MAC: MacAddr = MacAddr::new([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]);
/// MAC gateway slirp (`10.0.2.2`).
const GW_MAC: MacAddr = MacAddr::new([0x52, 0x55, 0x0a, 0x00, 0x02, 0x02]);
/// IP slirp cấp.
const OUR_IP: Ipv4Addr = Ipv4Addr::new(10, 0, 2, 15);
/// Gateway slirp — đích của echo request.
const GW_IP: Ipv4Addr = Ipv4Addr::new(10, 0, 2, 2);

/// Hai đầu của bước mạng dùng trong cả tệp (ta ↔ gateway).
const PEER: NetPeer = NetPeer::new(OUR_IP, OUR_MAC, GW_IP, GW_MAC);

/// Payload của echo request — **giống hệt** payload mà driver dựng (`payload[i] = i`).
fn payload() -> [u8; ICMP_PAYLOAD_BYTES] {
    let mut payload = [0u8; ICMP_PAYLOAD_BYTES];
    for (index, byte) in payload.iter_mut().enumerate() {
        *byte = index as u8;
    }
    payload
}

/// Dựng frame echo **reply** hợp lệ (checksum thật) vào đệm mới.
fn reply_frame() -> Vec<u8> {
    let payload = payload();
    let mut frame = vec![0u8; 96];
    let len = icmp_reply_frame(
        OUR_IP,
        OUR_MAC,
        GW_IP,
        GW_MAC,
        ICMP_ECHO_ID,
        ICMP_ECHO_SEQ,
        &payload,
        &mut frame,
    )
    .expect("dựng được frame reply");
    frame.truncate(len);
    frame
}

// ---------------------------------------------------------------- fake IO

/// IO giả: phát frame theo hàng đợi, đồng hồ nhảy `step_us` mỗi lần `recv` không có frame.
struct FakeIo {
    rx: VecDeque<Vec<u8>>,
    /// Trả **lặp lại** frame này khi hàng đợi rỗng (mô phỏng "chỉ có reply của người khác").
    forever: Option<Vec<u8>>,
    now: u64,
    step_us: u64,
    now_calls: u32,
    max_now_calls: u32,
    sent: Vec<Vec<u8>>,
    /// Timeout nhỏ nhất mà client đã truyền xuống `recv` (phải > 0 — 0 là "chờ mãi").
    min_recv_timeout: u64,
}

impl FakeIo {
    fn new(frames: Vec<Vec<u8>>) -> Self {
        Self {
            rx: frames.into(),
            forever: None,
            now: 0,
            step_us: 2_000,
            now_calls: 0,
            max_now_calls: 4096,
            sent: Vec::new(),
            min_recv_timeout: u64::MAX,
        }
    }

    fn always(frame: Vec<u8>) -> Self {
        let mut io = Self::new(Vec::new());
        io.forever = Some(frame);
        io
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
        if let Some(frame) = self.rx.pop_front().or_else(|| self.forever.clone()) {
            assert!(frame.len() <= out.len());
            out[..frame.len()].copy_from_slice(&frame);
            self.now += self.step_us;
            return Some(frame.len());
        }
        self.now += self.step_us.min(timeout_us);
        None
    }
}

// ---------------------------------------------------------------- 1. khung request

#[test]
fn icmp_echo_frame_round_trips() {
    let payload = payload();
    let mut frame = [0u8; 96];
    let len = icmp_echo_frame(
        OUR_IP,
        OUR_MAC,
        GW_IP,
        GW_MAC,
        ICMP_ECHO_ID,
        ICMP_ECHO_SEQ,
        &payload,
        &mut frame,
    )
    .expect("dựng được frame echo request");

    let eth = EthernetFrame::parse(&frame[..len]).expect("frame hợp lệ");
    assert_eq!(eth.header.src, OUR_MAC);
    assert_eq!(eth.header.dst, GW_MAC);
    assert_eq!(eth.header.ethertype, vifast_net::eth::ETHERTYPE_IPV4);

    let ip = Ipv4Header::parse(eth.payload, true).expect("IPv4 hợp lệ (checksum thật)");
    assert_eq!(ip.header.protocol, PROTO_ICMP);
    assert_eq!(ip.header.src, OUR_IP);
    assert_eq!(ip.header.dst, GW_IP);

    // `IcmpEcho::parse` kiểm checksum trên **toàn** lát byte ⇒ checksum do driver tính là đúng.
    match IcmpEcho::parse(ip.payload()).expect("ICMP hợp lệ (checksum thật)") {
        IcmpMessage::EchoRequest(echo) => {
            assert_eq!(echo.id, ICMP_ECHO_ID);
            assert_eq!(echo.seq, ICMP_ECHO_SEQ);
            assert_eq!(
                echo.payload,
                &payload[..],
                "payload được chép nguyên vào frame"
            );
        }
        other => panic!("phải là echo request, nhận {other:?}"),
    }
}

// ---------------------------------------------------------------- 2. bộ lọc reply

#[test]
fn icmp_reply_accept_matches_id_seq_and_payload() {
    let payload = payload();
    let frame = reply_frame();
    assert!(
        icmp_reply_accept(&frame, PEER, ICMP_ECHO_ID, ICMP_ECHO_SEQ, &payload),
        "reply đúng id/seq/payload phải được nhận"
    );

    let mut wrong = vec![0u8; 96];
    let len = icmp_reply_frame(
        OUR_IP,
        OUR_MAC,
        GW_IP,
        GW_MAC,
        ICMP_ECHO_ID ^ 1,
        ICMP_ECHO_SEQ,
        &payload,
        &mut wrong,
    )
    .expect("dựng reply id sai");
    assert!(
        !icmp_reply_accept(&wrong[..len], PEER, ICMP_ECHO_ID, ICMP_ECHO_SEQ, &payload),
        "reply của người khác (id khác) phải bị bỏ qua"
    );

    let mut wrong = vec![0u8; 96];
    let len = icmp_reply_frame(
        OUR_IP,
        OUR_MAC,
        GW_IP,
        GW_MAC,
        ICMP_ECHO_ID,
        ICMP_ECHO_SEQ ^ 1,
        &payload,
        &mut wrong,
    )
    .expect("dựng reply seq sai");
    assert!(
        !icmp_reply_accept(&wrong[..len], PEER, ICMP_ECHO_ID, ICMP_ECHO_SEQ, &payload),
        "reply của phiên khác (seq khác) phải bị bỏ qua"
    );

    let mut other_payload = payload;
    other_payload[0] ^= 0xff;
    let mut wrong = vec![0u8; 96];
    let len = icmp_reply_frame(
        OUR_IP,
        OUR_MAC,
        GW_IP,
        GW_MAC,
        ICMP_ECHO_ID,
        ICMP_ECHO_SEQ,
        &other_payload,
        &mut wrong,
    )
    .expect("dựng reply payload sai");
    assert!(
        !icmp_reply_accept(&wrong[..len], PEER, ICMP_ECHO_ID, ICMP_ECHO_SEQ, &payload),
        "payload khác payload đã gửi phải bị bỏ qua"
    );
}

#[test]
fn icmp_reply_accept_rejects_bad_checksum_and_foreign_source() {
    let payload = payload();
    let mut frame = reply_frame();
    // Checksum ICMP ở byte 2..4 của header ICMP (sau Ethernet 14 + IPv4 20).
    frame[14 + 20 + 2] ^= 0xff;
    assert!(
        !icmp_reply_accept(&frame, PEER, ICMP_ECHO_ID, ICMP_ECHO_SEQ, &payload),
        "checksum sai phải bị từ chối (codec kiểm)"
    );

    // Reply từ **host khác** (nguồn IP/MAC khác) không phải reply của gateway.
    let other_ip = Ipv4Addr::new(10, 0, 2, 99);
    let other_mac = MacAddr::new([0x52, 0x55, 0x0a, 0x00, 0x02, 0x63]);
    let mut frame = vec![0u8; 96];
    let len = icmp_reply_frame(
        OUR_IP,
        OUR_MAC,
        other_ip,
        other_mac,
        ICMP_ECHO_ID,
        ICMP_ECHO_SEQ,
        &payload,
        &mut frame,
    )
    .expect("dựng reply nguồn lạ");
    assert!(
        !icmp_reply_accept(&frame[..len], PEER, ICMP_ECHO_ID, ICMP_ECHO_SEQ, &payload),
        "reply từ host khác phải bị bỏ qua"
    );
}

#[test]
fn icmp_filter_selfcheck_detects_id_seq_payload() {
    assert_eq!(
        icmp_filter_selfcheck(PEER, ICMP_ECHO_ID, ICMP_ECHO_SEQ, &payload()),
        (true, true, true),
        "cả ba đột biến (id/seq/payload) phải bị bộ lọc từ chối"
    );
}

// ---------------------------------------------------------------- 3. bước ping + trần

const BUDGET: RetryBudget = RetryBudget::new(
    ICMP_TOTAL_TIMEOUT_US,
    ICMP_ATTEMPT_TIMEOUT_US,
    ICMP_MAX_ATTEMPTS,
);

#[test]
fn ping_accepts_matching_reply() {
    let mut io = FakeIo::new(vec![reply_frame()]);
    let stats = ping_with(&mut io, PEER, BUDGET).expect("phải nhận được reply");
    assert_eq!(stats.received, 1);
    assert_eq!(stats.sent, 1, "không phải gửi lại khi lần đầu được");
    assert_eq!((stats.id, stats.seq), (ICMP_ECHO_ID, ICMP_ECHO_SEQ));
    assert_eq!(stats.payload_bytes, ICMP_PAYLOAD_BYTES);
    assert!(stats.filter_id && stats.filter_seq && stats.filter_payload);
    assert_eq!(io.sent.len(), 1);
}

#[test]
fn ping_skips_foreign_reply_then_accepts_own() {
    let payload = payload();
    let mut foreign = vec![0u8; 96];
    let len = icmp_reply_frame(
        OUR_IP,
        OUR_MAC,
        GW_IP,
        GW_MAC,
        ICMP_ECHO_ID ^ 0x1234,
        ICMP_ECHO_SEQ,
        &payload,
        &mut foreign,
    )
    .expect("dựng reply lạ");
    let mut io = FakeIo::new(vec![foreign[..len].to_vec(), reply_frame()]);
    let stats = ping_with(&mut io, PEER, BUDGET).expect("reply của ta vẫn phải được nhận");
    assert_eq!(stats.received, 1);
    // Chỉ **một** lần gửi: reply lạ bị bỏ qua trong **cùng** ngân sách lần thử.
    assert_eq!(stats.sent, 1);
}

#[test]
fn ping_with_only_foreign_replies_times_out_bounded() {
    let payload = payload();
    let mut foreign = vec![0u8; 96];
    let len = icmp_reply_frame(
        OUR_IP,
        OUR_MAC,
        GW_IP,
        GW_MAC,
        ICMP_ECHO_ID ^ 0x1234,
        ICMP_ECHO_SEQ,
        &payload,
        &mut foreign,
    )
    .expect("dựng reply lạ");
    let mut io = FakeIo::always(foreign[..len].to_vec());
    let error = ping_with(&mut io, PEER, BUDGET).expect_err("không có reply của ta ⇒ Timeout");
    assert_eq!(error, NetError::Timeout);
    assert_eq!(
        io.sent.len() as u32,
        ICMP_MAX_ATTEMPTS,
        "số lần gửi có trần"
    );
    assert!(io.min_recv_timeout > 0 && io.min_recv_timeout != u64::MAX);
}

#[test]
fn ping_zero_budget_does_not_send() {
    let mut io = FakeIo::new(vec![reply_frame()]);
    let error = ping_with(&mut io, PEER, RetryBudget::new(0, 0, 0))
        .expect_err("ngân sách 0 ⇒ Timeout ngay");
    assert_eq!(error, NetError::Timeout);
    assert!(
        io.sent.is_empty(),
        "0 không phải 'vô hạn' — không được gửi gì"
    );
    assert!(
        io.now_calls <= 2,
        "không được đọc đồng hồ nhiều lần khi ngân sách 0"
    );
}

#[test]
fn ping_total_budget_is_bounded() {
    let mut io = FakeIo::new(Vec::new());
    let error = ping_with(&mut io, PEER, BUDGET).expect_err("không có reply ⇒ Timeout");
    assert_eq!(error, NetError::Timeout);
    assert!(
        io.now <= ICMP_TOTAL_TIMEOUT_US + ICMP_ATTEMPT_TIMEOUT_US,
        "tổng thời gian chờ {} µs vượt trần toàn bước {} µs",
        io.now,
        ICMP_TOTAL_TIMEOUT_US
    );
    assert!(io.min_recv_timeout > 0 && io.min_recv_timeout != u64::MAX);
}
