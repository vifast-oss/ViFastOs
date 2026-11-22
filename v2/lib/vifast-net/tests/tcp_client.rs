// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-335 — test host cho **TCP client policy** (`vifast_net::tcp_client`), D2-05: viết trước,
//! đỏ rồi mới xanh.
//!
//! # Vì sao test ở đây mà không ở driver
//!
//! Chính sách client (bắt tay, gửi/nhận, retransmit, đóng) trước KT-335 nằm **hai bản** trong
//! `vifast-plat-{riscv64,aarch64}/src/tcp.rs` — hai bản chỉ khác đồng hồ (tick vs µs) và chữ ký
//! `send_frame`, nên **không** chạy host được (crate plat có `unsafe` MMIO). Sau KT-335 chính sách
//! là hàm thuần trên hai trait [`Clock`]/[`Link`]; test cài bằng **server TCP giả** trên dây byte
//! thật (không mock tầng TCP) nên mỗi ca dưới đây chứng minh đúng một tính chất của giao thức.
//!
//! # Chín ca (đúng danh sách DoD của KT-335)
//!
//! Một dòng, đúng thứ tự: (1) bắt tay 3 bước; (2) gửi/nhận dữ liệu; (3) cửa sổ đầy (server
//! quảng bá `wnd = 0`); (4) mất gói + retransmit; (5) checksum sai (frame bị **bỏ**, không nhận
//! bừa); (6) thứ tự sai (ngoài cửa sổ ⇒ ACK lại, **không** giao dữ liệu); (7) FIN; (8) RST;
//! (9) `seq` **tăng** sau mỗi lần gửi (đột biến "bỏ tăng `seq`" phải làm ca này ĐỎ).
//!
//! Ca 9 tồn tại vì một đột biến chỉ bị bắt nếu có phép kiểm **khẳng định** nó: nếu chỉ kiểm "gửi
//! xong không lỗi" thì bỏ `snd_nxt += len` vẫn xanh — đúng lớp lỗi "phép kiểm xanh oan" của
//! `v2/docs/k6-test-invariants.md` §1.

use std::cell::Cell;
use std::collections::VecDeque;
use std::rc::Rc;

use vifast_net::addr::{Ipv4Addr, MacAddr};
use vifast_net::eth::{EthHeader, ETHERTYPE_IPV4, ETH_HEADER_LEN};
use vifast_net::frame::EthernetFrame;
use vifast_net::ipv4::{Ipv4Header, PROTO_TCP};
use vifast_net::tcp::{SendKind, State, TcpHeader, FLAG_ACK, FLAG_FIN, FLAG_RST, FLAG_SYN};
use vifast_net::tcp_client::{self, ClientConfig, Clock, Link, LinkError, TcpConn};

// ---------------------------------------------------------------- bốn đầu địa chỉ cố định

/// IP của khách (slirp cấp qua DHCP).
const OUR_IP: Ipv4Addr = Ipv4Addr::new(10, 0, 2, 15);
/// Gateway slirp — đóng vai server TCP trong test.
const GW_IP: Ipv4Addr = Ipv4Addr::new(10, 0, 2, 2);
/// MAC của khách.
const OUR_MAC: MacAddr = MacAddr::new([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]);
/// MAC của gateway.
const GW_MAC: MacAddr = MacAddr::new([0x52, 0x55, 0x0a, 0x00, 0x02, 0x02]);
/// Cổng client (hằng số của driver — chưa có RNG).
const OUR_PORT: u16 = 0x4b57;
/// Cổng server — **phải** khớp `run-*.sh`/`soak-*.sh` (server python lắng nghe cổng này).
const SRV_PORT: u16 = 45678;
/// MSS không option: 1500 − 20 IPv4 − 20 TCP.
const MSS: usize = 1460;

/// Một bước mạng 3 s; đổi theo đơn vị **tick** cho gọn số (test không phụ thuộc đơn vị thật).
const STEP: u64 = 3_000;
/// Khoảng retransmit = 3 s / 4 ⇒ 3 lần vẫn trong một bước.
const RETX: u64 = STEP / 4;

// ---------------------------------------------------------------- đồng hồ giả

/// Đồng hồ giả chia sẻ với [`Wire`] — `recv_frame` **tiến thời gian**, đúng như driver thật (nó
/// chờ trong hạn rồi mới trả `None`). Nhờ vậy vòng retransmit của client chạy được mà test không
/// ngủ thật.
#[derive(Clone)]
struct FakeClock(Rc<Cell<u64>>);

impl Clock for FakeClock {
    fn now(&self) -> u64 {
        self.0.get()
    }
}

// ---------------------------------------------------------------- server TCP giả trên dây

/// Server TCP tối giản: đọc segment của client rồi trả lời **bằng byte thật** (Ethernet/IPv4/TCP
/// encode qua chính codec của crate), nên phép kiểm không mock tầng TCP.
struct Wire {
    time: Rc<Cell<u64>>,
    /// Frame server đã gửi, chờ client nhận.
    inbox: VecDeque<Vec<u8>>,
    /// Frame client đã gửi (theo thứ tự) — dùng để kiểm cờ/`seq`.
    sent: Vec<Vec<u8>>,
    /// Số frame client gửi bị **rơi** trên đường ra (mô phỏng mất gói).
    drop_out: usize,
    /// Server sẽ trả lời hay **im** (ca "mất gói" ở chiều vào).
    silent: bool,
    /// Cửa sổ server quảng bá (0 = cửa sổ đầy).
    window: u16,
    /// `ISS` của server.
    irs: u32,
    /// Số thứ tự kế tiếp server mong nhận.
    rcv_nxt: u32,
}

impl Wire {
    fn new() -> (Self, FakeClock) {
        let time = Rc::new(Cell::new(1_000));
        let wire = Self {
            time: Rc::clone(&time),
            inbox: VecDeque::new(),
            sent: Vec::new(),
            drop_out: 0,
            silent: false,
            window: 4096,
            irs: 0x2000_0000,
            rcv_nxt: 0,
        };
        (wire, FakeClock(time))
    }

    /// Đẩy một frame thô vào hàng đợi **vào** (server chủ động gửi trước khi client chờ).
    fn push(&mut self, frame: Vec<u8>) {
        self.inbox.push_back(frame);
    }

    /// Dựng frame server → khách.
    fn server_frame(&self, seq: u32, ack: u32, flags: u8, payload: &[u8]) -> Vec<u8> {
        frame_from(
            GW_MAC,
            OUR_MAC,
            GW_IP,
            OUR_IP,
            SRV_PORT,
            OUR_PORT,
            seq,
            ack,
            flags,
            self.window,
            payload,
        )
    }

    /// Segment client vừa gửi đã parse (nếu đúng hình dạng) — `(seq, ack, flags, payload)`.
    fn parse_client(frame: &[u8]) -> Option<(u32, u8, Vec<u8>)> {
        let eth = EthernetFrame::parse(frame).ok()?;
        if eth.header.ethertype != ETHERTYPE_IPV4 {
            return None;
        }
        let ip = Ipv4Header::parse(eth.payload, true).ok()?;
        if ip.header.protocol != PROTO_TCP {
            return None;
        }
        let seg = TcpHeader::parse(ip.header.src, ip.header.dst, ip.payload(), true).ok()?;
        Some((seg.header.seq, seg.header.flags, seg.payload().to_vec()))
    }

    /// Phản hồi **đúng giao thức** cho frame client vừa gửi.
    fn respond(&mut self, frame: &[u8]) {
        let Some((seq, flags, payload)) = Self::parse_client(frame) else {
            return;
        };
        if flags & FLAG_SYN != 0 {
            self.rcv_nxt = seq.wrapping_add(1);
            let reply = self.server_frame(self.irs, self.rcv_nxt, FLAG_SYN | FLAG_ACK, &[]);
            self.inbox.push_back(reply);
            return;
        }
        if flags & FLAG_RST != 0 {
            return;
        }
        if !payload.is_empty() {
            self.rcv_nxt = seq.wrapping_add(payload.len() as u32);
            self.inbox
                .push_back(self.server_frame(self.irs, self.rcv_nxt, FLAG_ACK, &[]));
            return;
        }
        if flags & FLAG_FIN != 0 {
            self.rcv_nxt = seq.wrapping_add(1);
            self.inbox
                .push_back(self.server_frame(self.irs, self.rcv_nxt, FLAG_ACK, &[]));
        }
        // ACK thuần: không trả lời (đúng TCP).
    }
}

impl Link for Wire {
    fn send_frame(&mut self, frame: &[u8]) -> Result<(), LinkError> {
        self.sent.push(frame.to_vec());
        if self.drop_out > 0 {
            self.drop_out -= 1;
            return Ok(());
        }
        if self.silent {
            return Ok(());
        }
        self.respond(frame);
        Ok(())
    }

    fn recv_frame(&mut self, out: &mut [u8], timeout: u64) -> Option<usize> {
        // Trễ một chiều giả định 10 đơn vị: frame đã nằm trong hàng đợi thì tới ngay.
        if let Some(frame) = self.inbox.pop_front() {
            self.time.set(self.time.get() + 10);
            let n = frame.len().min(out.len());
            out[..n].copy_from_slice(&frame[..n]);
            return Some(n);
        }
        // Không có frame: **tiến thời gian** theo hạn rồi trả `None` — đúng hành vi của
        // `recv_frame_wait` trong driver (chờ hết hạn chờ rồi mới trả `None`).
        self.time.set(self.time.get() + timeout.max(1));
        None
    }
}

// ---------------------------------------------------------------- helper dựng frame

/// Ghép frame Ethernet/IPv4/TCP hoàn chỉnh (checksum thật qua codec của crate).
#[allow(clippy::too_many_arguments)]
fn frame_from(
    src_mac: MacAddr,
    dst_mac: MacAddr,
    src_ip: Ipv4Addr,
    dst_ip: Ipv4Addr,
    src_port: u16,
    dst_port: u16,
    seq: u32,
    ack: u32,
    flags: u8,
    window: u16,
    payload: &[u8],
) -> Vec<u8> {
    let mut tcp = [0u8; 128];
    let header = TcpHeader::new(src_port, dst_port, seq, ack, flags, window);
    let tcp_len = header
        .encode(src_ip, dst_ip, payload, &mut tcp)
        .expect("đệm TCP đủ chỗ");
    let mut ip = [0u8; 160];
    let ip_header_len = Ipv4Header::new(PROTO_TCP, src_ip, dst_ip, tcp_len)
        .encode(&mut ip)
        .expect("đệm IPv4 đủ chỗ");
    ip[ip_header_len..ip_header_len + tcp_len].copy_from_slice(&tcp[..tcp_len]);
    let ip_len = ip_header_len + tcp_len;
    let mut frame = [0u8; 256];
    let len = EthernetFrame {
        header: EthHeader::new(dst_mac, src_mac, ETHERTYPE_IPV4),
        payload: &ip[..ip_len],
    }
    .encode(&mut frame)
    .expect("đệm Ethernet đủ chỗ");
    frame[..len].to_vec()
}

/// Cấu hình client dùng chung cho mọi ca.
fn config() -> ClientConfig {
    ClientConfig {
        our_ip: OUR_IP,
        our_mac: OUR_MAC,
        dst_ip: GW_IP,
        dst_mac: GW_MAC,
        dst_port: SRV_PORT,
        client_port: OUR_PORT,
        iss: 0x4b54_0001,
        rcv_window: 4096,
        max_payload: MSS,
        retransmit: RETX,
    }
}

/// `(seq, flags, payload)` của frame thứ `index` client đã gửi.
fn sent_at(wire: &Wire, index: usize) -> (u32, u8, Vec<u8>) {
    let frame = wire.sent.get(index).expect("đã gửi frame này");
    let eth = EthernetFrame::parse(frame).expect("frame hợp lệ");
    let ip = Ipv4Header::parse(eth.payload, true).expect("IPv4 hợp lệ");
    let seg =
        TcpHeader::parse(ip.header.src, ip.header.dst, ip.payload(), true).expect("TCP hợp lệ");
    (seg.header.seq, seg.header.flags, seg.payload().to_vec())
}

/// Mở kết nối tới trạng thái `Established` (bắt tay thật qua [`Wire`]).
fn connect(wire: &mut Wire, clock: &FakeClock) -> TcpConn {
    tcp_client::connect(clock, wire, &config(), STEP).expect("bắt tay thành công")
}

// ================================================================ 1. bắt tay 3 bước

#[test]
fn handshake_three_way() {
    let (mut wire, clock) = Wire::new();
    let conn = connect(&mut wire, &clock);

    // Bước 1: SYN ra dây với `seq = ISS` và **không** có ACK.
    let (seq, flags, payload) = sent_at(&wire, 0);
    assert_eq!(seq, config().iss, "SYN phải mang ISS");
    assert_eq!(flags & FLAG_SYN, FLAG_SYN, "SYN phải bật cờ SYN");
    assert_eq!(flags & FLAG_ACK, 0, "SYN đầu tiên không có ACK");
    assert!(payload.is_empty());

    // Bước 3: ACK lại SYN+ACK với `ack = IRS + 1` — đây là bước thứ ba của bắt tay.
    let (seq, flags, _) = sent_at(&wire, 1);
    assert_eq!(flags & FLAG_ACK, FLAG_ACK, "bước 3 phải là ACK");
    assert_eq!(seq, config().iss.wrapping_add(1), "seq của ACK = ISS + 1");
    assert_eq!(conn.state(), State::Established);
}

// ================================================================ 2. gửi / nhận dữ liệu

#[test]
fn send_and_receive_payload() {
    let (mut wire, clock) = Wire::new();
    let mut conn = connect(&mut wire, &clock);

    let sent = tcp_client::send(&clock, &mut wire, &mut conn, b"ping", STEP).expect("gửi được");
    assert_eq!(sent, 4);
    // Frame dữ liệu (index 2) mang payload + cờ PSH.
    let (_, flags, payload) = sent_at(&wire, 2);
    assert_eq!(payload, b"ping");
    assert_eq!(flags & FLAG_ACK, FLAG_ACK);

    // Server đẩy "pong" vào; `recv` phải giao đúng 4 byte.
    let rcv_nxt = conn.rcv_nxt();
    let frame = wire.server_frame(wire.irs.wrapping_add(1), rcv_nxt, FLAG_ACK, b"pong");
    wire.push(frame);
    let mut out = [0u8; 16];
    let got = tcp_client::recv(&clock, &mut wire, &mut conn, &mut out, STEP).expect("nhận được");
    assert_eq!(&out[..got], b"pong");
    // Nhận xong phải ACK lại (TCP không có ACK chậm ở đây — máy trạng thái trả `Send(Ack)`).
    let (_, flags, _) = sent_at(&wire, 3);
    assert_eq!(flags & FLAG_ACK, FLAG_ACK);
}

// ================================================================ 3. cửa sổ đầy

#[test]
fn zero_window_blocks_send() {
    let (mut wire, clock) = Wire::new();
    // Server quảng bá cửa sổ 0 ngay từ SYN+ACK ⇒ không được gửi byte nào.
    wire.window = 0;
    let mut conn = connect(&mut wire, &clock);
    assert_eq!(conn.state(), State::Established);

    let sent_before = wire.sent.len();
    let error = tcp_client::send(&clock, &mut wire, &mut conn, b"x", STEP)
        .expect_err("cửa sổ 0 phải từ chối");
    assert_eq!(
        error,
        LinkError::BadFrame,
        "cửa sổ 0 ⇒ BadFrame (fail-closed)"
    );
    assert_eq!(wire.sent.len(), sent_before, "không frame nào được gửi");
}

// ================================================================ 4. mất gói + retransmit

#[test]
fn lost_syn_is_retransmitted() {
    let (mut wire, clock) = Wire::new();
    // Bỏ **hai** SYN đầu ⇒ phải retransmit tới lần thứ ba rồi mới bắt tay xong.
    wire.drop_out = 2;
    let conn =
        tcp_client::connect(&clock, &mut wire, &config(), STEP).expect("retransmit cứu được");

    // Ba frame SYN đã ra dây, cùng `seq`, cùng cờ — retransmit **nguyên văn** (RFC 793).
    assert!(wire.sent.len() >= 4, "SYN gửi lại ít nhất 3 lần");
    for index in 0..3 {
        let (seq, flags, _) = sent_at(&wire, index);
        assert_eq!(seq, config().iss, "SYN lần {index} giữ nguyên ISS");
        assert_eq!(flags & FLAG_SYN, FLAG_SYN);
    }
    assert_eq!(conn.state(), State::Established);
    // Hai frame bị rơi đã được "tiêu" ⇒ đúng hai lần retransmit xảy ra (không phải may mắn).
    assert_eq!(wire.drop_out, 0, "cả hai SYN bị rơi đã được gửi lại");
}

// ================================================================ 5. checksum sai

#[test]
fn bad_checksum_is_ignored() {
    let (mut wire, clock) = Wire::new();
    let mut conn = connect(&mut wire, &clock);

    // Segment "dữ liệu" của server với checksum TCP bị hỏng (lật byte cuối vùng TCP).
    let rcv_nxt = conn.rcv_nxt();
    let mut frame = wire.server_frame(wire.irs.wrapping_add(1), rcv_nxt, FLAG_ACK, b"hacked");
    let tcp_checksum_off = ETH_HEADER_LEN + 20 + 16;
    frame[tcp_checksum_off] ^= 0xff;
    wire.push(frame);

    let mut out = [0u8; 16];
    let error = tcp_client::recv(&clock, &mut wire, &mut conn, &mut out, STEP)
        .expect_err("frame checksum sai phải bị bỏ ⇒ hết hạn");
    assert_eq!(
        error,
        LinkError::Timeout,
        "checksum sai không được giao dữ liệu"
    );
    assert_eq!(conn.state(), State::Established, "trạng thái không đổi");
}

// ================================================================ 6. thứ tự sai

#[test]
fn out_of_order_is_acked_not_delivered() {
    let (mut wire, clock) = Wire::new();
    let mut conn = connect(&mut wire, &clock);

    // Segment nhảy **quá** cửa sổ nhận (seq = rcv_nxt + 2000 > wnd 4096? chọn 3000 để ngoài
    // cửa sổ 4096 là sai — nên dùng 2 cửa sổ: +9000).
    let rcv_nxt = conn.rcv_nxt();
    let frame = wire.server_frame(rcv_nxt.wrapping_add(9_000), rcv_nxt, FLAG_ACK, b"future");
    wire.push(frame);

    let mut out = [0u8; 16];
    let error = tcp_client::recv(&clock, &mut wire, &mut conn, &mut out, STEP)
        .expect_err("ngoài cửa sổ ⇒ không giao dữ liệu");
    assert_eq!(error, LinkError::Timeout);
    // Máy trạng thái **vẫn** ACK lại (RFC 793 §3.9): ít nhất một ACK thuần đã ra dây.
    let last = sent_at(&wire, wire.sent.len() - 1);
    assert_eq!(
        last.1 & FLAG_ACK,
        FLAG_ACK,
        "phải ACK lại segment ngoài cửa sổ"
    );
    assert!(last.2.is_empty(), "ACK lại không mang payload");
    assert_eq!(conn.state(), State::Established);
}

// ================================================================ 7. đóng bằng FIN

#[test]
fn fin_closes_connection() {
    let (mut wire, clock) = Wire::new();
    let mut conn = connect(&mut wire, &clock);
    tcp_client::close(&clock, &mut wire, &mut conn, STEP).expect("đóng được");

    let (_, flags, payload) = sent_at(&wire, wire.sent.len() - 1);
    assert_eq!(flags & FLAG_FIN, FLAG_FIN, "phải gửi FIN");
    assert!(payload.is_empty());
    assert!(conn.fin_acked(), "FIN của ta đã được ACK");
    // RFC 793: FIN của **ta** được ACK ⇒ `FinWait2` (chờ FIN của peer). `TimeWait` chỉ tới khi
    // **cả hai** FIN đã trao đổi — đúng bất biến của `crate::tcp`, không nới ở đây.
    assert_eq!(conn.state(), State::FinWait2, "FIN được ACK ⇒ FinWait2");
}

// ================================================================ 8. RST

#[test]
fn rst_aborts_connect() {
    let (mut wire, clock) = Wire::new();
    // Server trả RST (+ACK đúng số thứ tự) thay vì SYN+ACK.
    let rst = wire.server_frame(0, config().iss.wrapping_add(1), FLAG_RST | FLAG_ACK, &[]);
    wire.push(rst);
    let error = tcp_client::connect(&clock, &mut wire, &config(), STEP)
        .expect_err("RST phải làm hỏng bắt tay");
    assert_eq!(error, LinkError::Io, "RST ⇒ Io (huỷ), không phải Timeout");
}

// ================================================================ 9. seq tăng sau khi gửi

#[test]
fn seq_advances_after_send() {
    let (mut wire, clock) = Wire::new();
    let mut conn = connect(&mut wire, &clock);

    tcp_client::send(&clock, &mut wire, &mut conn, b"abc", STEP).expect("gửi lần 1");
    let (seq1, _, payload1) = sent_at(&wire, 2);
    tcp_client::send(&clock, &mut wire, &mut conn, b"de", STEP).expect("gửi lần 2");
    let (seq2, _, payload2) = sent_at(&wire, 3);
    assert_eq!(payload1, b"abc");
    assert_eq!(payload2, b"de");
    // **Đột biến "bỏ tăng seq" phải làm dòng này ĐỎ**: segment thứ hai phải bắt đầu đúng sau
    // segment thứ nhất, không dùng lại số thứ tự cũ.
    assert_eq!(
        seq2,
        seq1.wrapping_add(payload1.len() as u32),
        "seq phải tăng đúng bằng số byte đã gửi"
    );
    // Và `snd_una` của máy trạng thái cũng đã tiến (ACK của server bao cả hai segment).
    assert_eq!(seq2.wrapping_add(2), conn.snd_una());
}

/// `SendKind` được dùng để giữ liên kết với hằng cờ của máy trạng thái (tránh import thừa).
#[test]
fn send_kinds_match_wire_flags() {
    assert_eq!(SendKind::Data.flags() & FLAG_ACK, FLAG_ACK);
    assert_eq!(SendKind::Fin.flags() & FLAG_FIN, FLAG_FIN);
}
