// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ICMP echo + DNS query** trên x86_64 — KT-328 (K5': ICMP/DNS trên x86_64).
//!
//! Tách khỏi [`crate::virtio_net`] vì **luật cấu trúc R1** (tệp trong `kernel/` ≤ 1.500 dòng):
//! driver virtio-net + ARP + DHCP + phần này trong **một** tệp là 1.965 dòng. `virtio_net`
//! **re-export** mọi tên ở đây nên ảnh (`kbin`) và host test vẫn dùng đường
//! `vifast_plat_x86_64::virtio_net::{ping, dns_query, …}` — **không** có API thứ hai.
//!
//! # Vì sao chỉ **chính sách** nằm ở đây (và không viết lại codec)
//!
//! Ethernet/IPv4/ICMP/UDP/DNS đã có trong `vifast_net` (KT-244, host test, gồm ca âm checksum/
//! `RDLENGTH`/`RCODE`). Tệp này chỉ: (1) ghép/đọc **khung**, (2) chạy **máy trạng thái** có trần,
//! (3) tự kiểm **bộ lọc** ngay trong ảnh. Cùng khuôn `dhcp_*` của KT-326 và `ping`/`dns_query`
//! của khuôn aarch64 (KT-275/KT-289) — **một** codec, hai arch dùng chung.
//!
//! # Ba trần cho **mỗi** bước (khuôn KT-326)
//!
//! Mỗi bước có ba trần **độc lập**: toàn bước ([`ICMP_TOTAL_TIMEOUT_US`]/[`DNS_TOTAL_TIMEOUT_US`],
//! 6 s), mỗi lần gửi/chờ ([`ICMP_ATTEMPT_TIMEOUT_US`] 1 s / [`DNS_ATTEMPT_TIMEOUT_US`] 3 s — giữ
//! đúng ngân sách mỗi lần đã đo và công bố cho DNS ở KT-294/D2-215), và số lần thử
//! ([`ICMP_MAX_ATTEMPTS`] 3 / [`DNS_MAX_ATTEMPTS`] = trần của codec, 2). Ngân sách **0** được hiểu
//! là **hết ngân sách** (trả `Timeout` ngay), **không** phải "chờ mãi".
//!
//! # Đích của DNS: `localhost`/`127.0.0.1`
//!
//! Phép kiểm phải đòi **một địa chỉ cụ thể**, không phải "một địa chỉ bất kỳ": slirp chuyển truy
//! vấn tới resolver của host, nên `example.com` trả địa chỉ **đổi theo thời gian** (đo: hai lần
//! trả hai bộ bản ghi `A` khác nhau) — không dùng làm hằng được. `localhost` là tên **cục bộ**:
//! resolver trả `127.0.0.1` (đo trên máy này: `an=1`, `A 127.0.0.1`, TTL 0) và không phụ thuộc
//! DNS ngoài. Hằng kỳ vọng do **ảnh** giữ (`kbin/src/net.rs`) và **runner** đối chiếu lại trong
//! dòng evidence — hai chỗ, cùng một giá trị, không có chỗ thứ ba.

use vifast_net::addr::{Ipv4Addr, MacAddr};
use vifast_net::dhcp::DHCP_MAX_LEN;
use vifast_net::dns::{
    ARecord, DnsMessage, DnsQuery, DnsQuestion, CLASS_IN, RCODE_NOERROR, RCODE_NXDOMAIN, TYPE_A,
};
use vifast_net::eth::{EthHeader, ETHERTYPE_IPV4, ETH_HEADER_LEN};
use vifast_net::frame::EthernetFrame;
use vifast_net::icmp::{IcmpEcho, IcmpMessage, ICMP_HEADER_LEN};
use vifast_net::ipv4::{Ipv4Header, IPV4_MIN_HEADER_LEN, PROTO_ICMP, PROTO_UDP};
use vifast_net::udp::{UdpHeader, UDP_HEADER_LEN};
use vifast_virtio::net::{NetError, MAX_FRAME_BYTES};

use crate::virtio_net::{attempt_budget, udp_frame, NetIo, VirtioNetIo};

//
// # Vì sao chỉ **chính sách** nằm ở đây (và không viết lại codec)
//
// Ethernet/IPv4/ICMP/UDP/DNS đã có trong `vifast_net` (KT-244, host test, gồm ca âm checksum/
// `RDLENGTH`/`RCODE`). Tệp này chỉ: (1) ghép/đọc **khung**, (2) chạy **máy trạng thái** có trần,
// (3) tự kiểm **bộ lọc** ngay trong ảnh. Cùng khuôn `dhcp_*` của KT-326 và `ping`/`dns_query` của
// khuôn aarch64 (KT-275/KT-289) — **một** codec, hai arch dùng chung.
//
// # Ba trần cho **mỗi** bước (khuôn KT-326)
//
// Mỗi bước có ba trần **độc lập**: toàn bước ([`ICMP_TOTAL_TIMEOUT_US`]/[`DNS_TOTAL_TIMEOUT_US`],
// 6 s), mỗi lần gửi/chờ ([`ICMP_ATTEMPT_TIMEOUT_US`] 1 s / [`DNS_ATTEMPT_TIMEOUT_US`] 3 s — giữ
// đúng ngân sách mỗi lần đã đo và công bố cho DNS ở KT-294/D2-215), và số lần thử
// ([`ICMP_MAX_ATTEMPTS`] 3 / [`DNS_MAX_ATTEMPTS`] = trần của codec, 2). Ngân sách **0** được hiểu
// là **hết ngân sách** (trả `Timeout` ngay), **không** phải "chờ mãi".
//
// # Đích của DNS: `localhost`/`127.0.0.1`
//
// Phép kiểm phải đòi **một địa chỉ cụ thể**, không phải "một địa chỉ bất kỳ": slirp chuyển truy
// vấn tới resolver của host, nên `example.com` trả địa chỉ **đổi theo thời gian** (đo: 2 bản ghi
// `A` khác nhau giữa hai lần) — không dùng làm hằng được. `localhost` là tên **cục bộ**: resolver
// trả `127.0.0.1` (đo trên máy này: `an=1`, `A 127.0.0.1`, TTL 0) và không phụ thuộc DNS ngoài.
// Hằng kỳ vọng do **ảnh** giữ (`kbin/src/net.rs`) và **runner** đối chiếu lại trong dòng evidence
// — hai chỗ, cùng một giá trị, không có chỗ thứ ba.

/// Cỡ payload ICMP echo (byte đệm) — cùng giá trị khuôn aarch64 [`vifast_plat_aarch64`].
pub const ICMP_PAYLOAD_BYTES: usize = 32;
/// `id` của echo request (chưa có RNG); reply **phải** khớp `id` này.
pub const ICMP_ECHO_ID: u16 = 0x4b54;
/// `seq` của echo request (một request mỗi bước).
pub const ICMP_ECHO_SEQ: u16 = 1;
/// Trần **toàn bộ** bước ICMP (µs): 6 s (khuôn KT-326).
pub const ICMP_TOTAL_TIMEOUT_US: u64 = 6_000_000;
/// Trần **mỗi lần** gửi/chờ của ICMP (µs): 1 s.
pub const ICMP_ATTEMPT_TIMEOUT_US: u64 = 1_000_000;
/// Số lần thử của bước ICMP — trần, không retry vô hạn.
pub const ICMP_MAX_ATTEMPTS: u32 = 3;

/// Cổng UDP của DNS.
pub const DNS_PORT: u16 = 53;
/// Cổng nguồn UDP cố định của client (chưa có RNG; còn kiểm cổng/địa chỉ đích).
pub const DNS_CLIENT_PORT: u16 = 0x4b54;
/// Cỡ payload DNS tối đa không EDNS (RFC 1035 §4.2.1) — trần đệm reply.
pub const DNS_MAX_LEN: usize = 512;
/// `id` của truy vấn DNS (chưa có RNG); reply **phải** khớp `id` này (codec kiểm trong
/// [`dns_reply_accept`]).
pub const DNS_ID: u16 = 0x4b55;
/// Trần **toàn bộ** bước DNS (µs): 6 s — bằng `2 × [`DNS_ATTEMPT_TIMEOUT_US`]`.
pub const DNS_TOTAL_TIMEOUT_US: u64 = 6_000_000;
/// Trần **mỗi lần** gửi/chờ của DNS (µs): 3 s — **không** đổi so với KT-294/D2-215 (ngân sách đã
/// đo: p99 150 ms, max thành công 286 ms dưới slirp).
pub const DNS_ATTEMPT_TIMEOUT_US: u64 = 3_000_000;
/// Số lần thử của DNS — **suy ra** từ trần của codec ([`vifast_net::dns::DNS_MAX_ATTEMPTS`], 2),
/// không chép số thứ hai.
pub const DNS_MAX_ATTEMPTS: u32 = vifast_net::dns::DNS_MAX_ATTEMPTS as u32;

/// Cỡ tối đa một frame ICMP echo (Ethernet + IPv4 + ICMP + payload) — **tính** từ hằng codec.
pub const ICMP_FRAME_BYTES: usize =
    ETH_HEADER_LEN + IPV4_MIN_HEADER_LEN + ICMP_HEADER_LEN + ICMP_PAYLOAD_BYTES;
/// Cỡ tối đa một frame UDP mang DNS (Ethernet + IPv4 + UDP + payload 512) — **tính** từ hằng codec.
pub const DNS_FRAME_BYTES: usize =
    ETH_HEADER_LEN + IPV4_MIN_HEADER_LEN + UDP_HEADER_LEN + DNS_MAX_LEN;

/// Hai đầu của **một** bước mạng trên Ethernet: IP/MAC của ta và của đích.
///
/// Gói bốn giá trị vào một struct để call-site (`ping`, `dns_query`, host test) không phải truyền
/// bốn tham số rời — và để "đích" không thể bị hoán vị nhầm với "ta".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NetPeer {
    /// IP của ta (lease DHCP).
    pub our_ip: Ipv4Addr,
    /// MAC của ta (đọc từ thiết bị).
    pub our_mac: MacAddr,
    /// IP của đích (gateway hoặc DNS server).
    pub peer_ip: Ipv4Addr,
    /// MAC của đích (đã resolve bằng ARP).
    pub peer_mac: MacAddr,
}

impl NetPeer {
    /// Hai đầu của một bước mạng.
    pub const fn new(
        our_ip: Ipv4Addr,
        our_mac: MacAddr,
        peer_ip: Ipv4Addr,
        peer_mac: MacAddr,
    ) -> Self {
        Self {
            our_ip,
            our_mac,
            peer_ip,
            peer_mac,
        }
    }
}

/// Ba trần của **một** bước mạng có thử lại — truyền vào hàm để host test đo được trần mà
/// **không** cần QEMU, và để ảnh không có đường "chờ vô hạn" nào khác.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetryBudget {
    /// Ngân sách **toàn bộ** bước (µs) — deadline tuyệt đối.
    pub total_us: u64,
    /// Ngân sách **mỗi lần** gửi/chờ (µs).
    pub attempt_us: u64,
    /// Số lần thử tối đa.
    pub max_attempts: u32,
}

impl RetryBudget {
    /// Ba trần của một bước.
    pub const fn new(total_us: u64, attempt_us: u64, max_attempts: u32) -> Self {
        Self {
            total_us,
            attempt_us,
            max_attempts,
        }
    }

    /// `true` ⇔ **không** có ngân sách nào ⇒ phải `Timeout` **ngay**, không gửi/không chờ.
    ///
    /// `0` **không** phải "vô hạn": đây là hợp đồng mà đột biến (iii) của KT-326 đã kiểm cho
    /// DHCP và KT-328 kiểm lại cho ICMP/DNS.
    pub const fn is_zero(&self) -> bool {
        self.total_us == 0 || self.attempt_us == 0 || self.max_attempts == 0
    }
}

/// Kết quả **một** echo request/reply đã kiểm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PingStats {
    /// `id` đã gửi và đã khớp ở reply.
    pub id: u16,
    /// `seq` đã gửi và đã khớp ở reply.
    pub seq: u16,
    /// Số frame echo request đã gửi (≥ 1).
    pub sent: u32,
    /// Số reply khớp đã nhận (bước này đòi **đúng 1**).
    pub received: u32,
    /// Cỡ payload đã gửi (reply phải chép **nguyên** — RFC 792).
    pub payload_bytes: usize,
    /// RTT của reply (µs).
    pub rtt_us: u64,
    /// `true` ⇔ bộ lọc **đã** từ chối một reply sai `id` (đột biến (i) của KT-328).
    pub filter_id: bool,
    /// `true` ⇔ bộ lọc **đã** từ chối một reply sai `seq` (đột biến (ii)).
    pub filter_seq: bool,
    /// `true` ⇔ bộ lọc **đã** từ chối một reply sai payload.
    pub filter_payload: bool,
}

impl PingStats {
    /// `true` ⇔ **cả ba** phép tự kiểm của bộ lọc đều bắt được reply bị đột biến.
    pub const fn filter_ok(&self) -> bool {
        self.filter_id && self.filter_seq && self.filter_payload
    }
}

/// Reply DNS đã kiểm (một câu hỏi + một bản ghi `A` đúng hằng kỳ vọng).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DnsReply {
    /// `id` của reply — trùng `id` của truy vấn (codec kiểm).
    pub id: u16,
    /// `RCODE` — bước này đòi [`RCODE_NOERROR`].
    pub rcode: u8,
    /// Câu hỏi trong reply (server **chép nguyên**, RFC 1035 §4.1.2).
    pub question: DnsQuestion,
    /// Bản ghi `A` đầu tiên — phải bằng hằng kỳ vọng.
    pub a: ARecord,
}

/// Kết quả **một** truy vấn DNS `A` đã kiểm.
#[derive(Clone, Copy, Debug)]
pub struct DnsAnswer {
    /// Reply đã kiểm.
    pub reply: DnsReply,
    /// RTT của reply (µs).
    pub rtt_us: u64,
    /// Độ dài message reply (byte).
    pub reply_len: usize,
    /// Số lần đã **gửi** truy vấn (1..=[`DNS_MAX_ATTEMPTS`]).
    pub attempts: u32,
    /// Số reply **hỏng/không khớp** đã bị bỏ rồi **thử lại** (D2-246); `0` ở đường bình thường.
    pub bad_replies: u32,
    /// `true` ⇔ bộ lọc **đã** từ chối một reply sai `id` (đột biến (iii) của KT-328).
    pub filter_id: bool,
    /// `true` ⇔ bộ lọc **đã** từ chối một reply có `QR=0`.
    pub filter_qr: bool,
    /// `true` ⇔ bộ lọc **đã** từ chối một reply có `RCODE≠0` (đột biến (iv)).
    pub filter_rcode: bool,
}

impl DnsAnswer {
    /// `true` ⇔ **cả ba** phép tự kiểm của bộ lọc đều bắt được reply bị đột biến.
    pub const fn filter_ok(&self) -> bool {
        self.filter_id && self.filter_qr && self.filter_rcode
    }
}

/// Ghép **một** frame ICMP echo vào `out`; trả tổng số byte.
///
/// `reply = false` ⇒ echo **request** (ta → đích); `reply = true` ⇒ echo **reply** (đích → ta,
/// dùng cho phép tự kiểm bộ lọc và host test). Cả hai chiều dùng **cùng** codec
/// [`IcmpEcho::encode`] nên checksum trong frame luôn **đúng** — frame bị đột biến chỉ khác đúng
/// trường bị đổi, nhờ vậy phép tự kiểm không "đúng vì lý do sai" (checksum hỏng).
#[allow(clippy::too_many_arguments)]
fn icmp_message_frame(
    src_mac: MacAddr,
    dst_mac: MacAddr,
    src_ip: Ipv4Addr,
    dst_ip: Ipv4Addr,
    id: u16,
    seq: u16,
    payload: &[u8],
    reply: bool,
    out: &mut [u8],
) -> Option<usize> {
    if payload.len() > ICMP_PAYLOAD_BYTES {
        return None;
    }
    let mut icmp = [0u8; ICMP_HEADER_LEN + ICMP_PAYLOAD_BYTES];
    let message = if reply {
        IcmpEcho::reply(id, seq, payload)
    } else {
        IcmpEcho::request(id, seq, payload)
    };
    let icmp_len = message.encode(&mut icmp).ok()?;
    let mut ip = [0u8; IPV4_MIN_HEADER_LEN + ICMP_HEADER_LEN + ICMP_PAYLOAD_BYTES];
    let header_len = Ipv4Header::new(PROTO_ICMP, src_ip, dst_ip, icmp_len)
        .encode(&mut ip)
        .ok()?;
    ip[header_len..header_len + icmp_len].copy_from_slice(&icmp[..icmp_len]);
    let ip_len = header_len + icmp_len;
    EthernetFrame {
        header: EthHeader::new(dst_mac, src_mac, ETHERTYPE_IPV4),
        payload: &ip[..ip_len],
    }
    .encode(out)
    .ok()
}

/// Dựng **một** frame ICMP echo **request** (ta → đích) vào `out`; trả tổng số byte.
///
/// Tách khỏi [`ping_with`] để **host test** kiểm được khung thật (checksum/độ dài/ethertype) mà
/// không cần thiết bị — cùng lý do `icmp_echo_frame` của khuôn aarch64.
#[allow(clippy::too_many_arguments)]
pub fn icmp_echo_frame(
    our_ip: Ipv4Addr,
    our_mac: MacAddr,
    target_ip: Ipv4Addr,
    target_mac: MacAddr,
    id: u16,
    seq: u16,
    payload: &[u8],
    out: &mut [u8],
) -> Option<usize> {
    icmp_message_frame(
        our_mac, target_mac, our_ip, target_ip, id, seq, payload, false, out,
    )
}

/// Dựng **một** frame ICMP echo **reply** (đích → ta) vào `out`; trả tổng số byte.
///
/// Chỉ dùng cho **phép tự kiểm bộ lọc** và host test: checksum hợp lệ, nên lý do duy nhất để
/// [`icmp_reply_accept`] từ chối một frame như vậy là trường bị đổi (`id`/`seq`/payload).
#[allow(clippy::too_many_arguments)]
pub fn icmp_reply_frame(
    our_ip: Ipv4Addr,
    our_mac: MacAddr,
    target_ip: Ipv4Addr,
    target_mac: MacAddr,
    id: u16,
    seq: u16,
    payload: &[u8],
    out: &mut [u8],
) -> Option<usize> {
    icmp_message_frame(
        target_mac, our_mac, target_ip, our_ip, id, seq, payload, true, out,
    )
}

/// `true` ⇔ `frame` là echo **reply** cho **đúng** request `(id, seq, payload)` của ta.
///
/// Bốn lớp kiểm, mỗi lớp bắt một lỗi khác nhau: (1) Ethernet/IPv4 hợp lệ + checksum IPv4
/// (`Ipv4Header::parse(…, true)`) + địa chỉ/MAC đúng hai đầu; (2) `payload` IPv4 đã cắt đúng
/// `total length` nên checksum ICMP không lẫn padding Ethernet; (3) [`IcmpEcho::parse`] kiểm
/// **checksum ICMP** và phân loại `EchoReply`; (4) `id`/`seq`/`payload` phải khớp — reply của
/// người khác (đột biến (i)/(ii)) bị **bỏ qua**, không nhận "gần đúng".
pub fn icmp_reply_accept(frame: &[u8], peer: NetPeer, id: u16, seq: u16, payload: &[u8]) -> bool {
    let Ok(eth) = EthernetFrame::parse(frame) else {
        return false;
    };
    if eth.header.ethertype != ETHERTYPE_IPV4 {
        return false;
    }
    if eth.header.src != peer.peer_mac || eth.header.dst != peer.our_mac {
        return false;
    }
    let Ok(ip) = Ipv4Header::parse(eth.payload, true) else {
        return false;
    };
    if ip.header.protocol != PROTO_ICMP
        || ip.header.src != peer.peer_ip
        || ip.header.dst != peer.our_ip
    {
        return false;
    }
    match IcmpEcho::parse(ip.payload()) {
        Ok(IcmpMessage::EchoReply(echo)) => {
            echo.id == id && echo.seq == seq && echo.payload == payload
        }
        _ => false,
    }
}

/// `true` ⇔ frame dựng với `(frame_id, frame_seq, frame_payload)` bị [`icmp_reply_accept`] **từ
/// chối** khi kỳ vọng là `(id, seq, payload)`.
fn icmp_poisoned_rejected(
    peer: NetPeer,
    frame_id: u16,
    frame_seq: u16,
    frame_payload: &[u8],
    id: u16,
    seq: u16,
    payload: &[u8],
) -> bool {
    let mut frame = [0u8; ICMP_FRAME_BYTES];
    match icmp_reply_frame(
        peer.our_ip,
        peer.our_mac,
        peer.peer_ip,
        peer.peer_mac,
        frame_id,
        frame_seq,
        frame_payload,
        &mut frame,
    ) {
        Some(len) => !icmp_reply_accept(&frame[..len], peer, id, seq, payload),
        None => false,
    }
}

/// Bộ lọc tự kiểm **chạy trong ảnh** (TAP 32): dựng reply hợp lệ rồi dựng lại với `id`, `seq`,
/// payload bị đổi — **checksum vẫn đúng** — và trả `(id_bị_từ_chối, seq_bị_từ_chối,
/// payload_bị_từ_chối)`.
///
/// Nếu reply hợp lệ **không** được nhận thì trả `(false, false, false)`: "từ chối tất cả" không
/// phải bằng chứng bộ lọc kiểm đúng chỗ. Đột biến (i)/(ii) làm một cờ thành `0` ⇒ TAP 32 **ĐỎ
/// ngay trên QEMU**.
pub fn icmp_filter_selfcheck(
    peer: NetPeer,
    id: u16,
    seq: u16,
    payload: &[u8],
) -> (bool, bool, bool) {
    let mut good = [0u8; ICMP_FRAME_BYTES];
    let Some(len) = icmp_reply_frame(
        peer.our_ip,
        peer.our_mac,
        peer.peer_ip,
        peer.peer_mac,
        id,
        seq,
        payload,
        &mut good,
    ) else {
        return (false, false, false);
    };
    if !icmp_reply_accept(&good[..len], peer, id, seq, payload) {
        return (false, false, false);
    }
    let id_ok = icmp_poisoned_rejected(peer, id ^ 1, seq, payload, id, seq, payload);
    let seq_ok = icmp_poisoned_rejected(peer, id, seq ^ 1, payload, id, seq, payload);
    // Payload khác **cùng** độ dài (một byte đổi) — reply "gần đúng" vẫn phải bị từ chối.
    let n = payload.len();
    let mut other = [0u8; ICMP_PAYLOAD_BYTES];
    other[..n].copy_from_slice(payload);
    other[0] ^= 0xff;
    let payload_ok = icmp_poisoned_rejected(peer, id, seq, &other[..n], id, seq, payload);
    (id_ok, seq_ok, payload_ok)
}

/// **ICMP echo** một request tới `peer.peer_ip` trên **một** [`NetIo`] bất kỳ (driver thật hoặc
/// fake trong host test), với **ba trần** của `budget`.
///
/// `Err(NetError::Timeout)` khi hết ngân sách/số lần thử; `Err(NetError::BadFrame)` khi không
/// dựng được frame. **Không** có đường "chờ mãi": mọi vòng lặp so với deadline **tuyệt đối**.
pub fn ping_with(
    io: &mut dyn NetIo,
    peer: NetPeer,
    budget: RetryBudget,
) -> Result<PingStats, NetError> {
    if budget.is_zero() {
        return Err(NetError::Timeout);
    }
    let mut payload = [0u8; ICMP_PAYLOAD_BYTES];
    for (index, byte) in payload.iter_mut().enumerate() {
        *byte = index as u8;
    }
    let (id, seq) = (ICMP_ECHO_ID, ICMP_ECHO_SEQ);
    let mut frame = [0u8; ICMP_FRAME_BYTES];
    let frame_len = icmp_echo_frame(
        peer.our_ip,
        peer.our_mac,
        peer.peer_ip,
        peer.peer_mac,
        id,
        seq,
        &payload,
        &mut frame,
    )
    .ok_or(NetError::BadFrame)?;
    let total_deadline = io.now_us().saturating_add(budget.total_us);
    let mut reply = [0u8; MAX_FRAME_BYTES];
    let mut attempts = 0u32;
    while attempts < budget.max_attempts {
        attempts = attempts.saturating_add(1);
        let now = io.now_us();
        if now >= total_deadline {
            return Err(NetError::Timeout);
        }
        let send_budget = attempt_budget(now, total_deadline, budget.attempt_us);
        io.send(&frame[..frame_len], send_budget)?;
        // Đo lại sau khi gửi: thời gian gửi cũng tiêu ngân sách (một đồng hồ, một trần).
        let sent_at = io.now_us();
        if sent_at >= total_deadline {
            return Err(NetError::Timeout);
        }
        let attempt_deadline = sent_at
            .saturating_add(attempt_budget(sent_at, total_deadline, budget.attempt_us))
            .min(total_deadline);
        loop {
            let now = io.now_us();
            if now >= attempt_deadline {
                break;
            }
            let remaining = attempt_deadline.saturating_sub(now);
            // `remaining > 0` (đã kiểm ở trên) — **không bao giờ** truyền 0 xuống như "chờ mãi".
            let Some(got) = io.recv(&mut reply, remaining) else {
                continue;
            };
            if !icmp_reply_accept(&reply[..got], peer, id, seq, &payload) {
                continue;
            }
            let rtt_us = io.now_us().saturating_sub(sent_at);
            let (filter_id, filter_seq, filter_payload) =
                icmp_filter_selfcheck(peer, id, seq, &payload);
            return Ok(PingStats {
                id,
                seq,
                sent: attempts,
                received: 1,
                payload_bytes: payload.len(),
                rtt_us,
                filter_id,
                filter_seq,
                filter_payload,
            });
        }
    }
    Err(NetError::Timeout)
}

/// **ICMP echo** trên driver thật: một request tới `peer.peer_ip`, ba trần của module.
pub fn ping(peer: NetPeer, now: &mut dyn FnMut() -> u64) -> Result<PingStats, NetError> {
    let mut io = VirtioNetIo::new(now);
    ping_with(
        &mut io,
        peer,
        RetryBudget::new(
            ICMP_TOTAL_TIMEOUT_US,
            ICMP_ATTEMPT_TIMEOUT_US,
            ICMP_MAX_ATTEMPTS,
        ),
    )
}

/// Đọc **một** datagram UDP khớp `src_ip:src_port` → `our_ip:dst_port` cho tới deadline **tuyệt
/// đối** `deadline_us`; copy payload vào `out`.
///
/// Frame không khớp (ARP, ICMP, cổng khác) bị **bỏ qua** — cùng cách `dhcp_wait`/`ping_with`
/// đang làm. Hết hạn ⇒ [`NetError::Timeout`] (fail-closed, không treo).
fn udp_recv_until(
    io: &mut dyn NetIo,
    our_ip: Ipv4Addr,
    src_ip: Ipv4Addr,
    src_port: u16,
    dst_port: u16,
    out: &mut [u8],
    deadline_us: u64,
) -> Result<usize, NetError> {
    let mut frame = [0u8; MAX_FRAME_BYTES];
    loop {
        let now = io.now_us();
        if now >= deadline_us {
            return Err(NetError::Timeout);
        }
        let remaining = deadline_us.saturating_sub(now);
        let Some(got) = io.recv(&mut frame, remaining) else {
            continue;
        };
        let Ok(eth) = EthernetFrame::parse(&frame[..got]) else {
            continue;
        };
        if eth.header.ethertype != ETHERTYPE_IPV4 {
            continue;
        }
        let Ok(ip) = Ipv4Header::parse(eth.payload, true) else {
            continue;
        };
        if ip.header.protocol != PROTO_UDP || ip.header.src != src_ip || ip.header.dst != our_ip {
            continue;
        }
        let Ok(udp) = UdpHeader::parse(ip.header.src, ip.header.dst, ip.payload(), true) else {
            continue;
        };
        if udp.header.src_port != src_port || udp.header.dst_port != dst_port {
            continue;
        }
        let payload = udp.payload();
        if payload.len() > out.len() {
            return Err(NetError::BadFrame);
        }
        out[..payload.len()].copy_from_slice(payload);
        return Ok(payload.len());
    }
}

/// Kiểm **một** message reply DNS cho truy vấn `(id, name)` và đòi bản ghi `A` **đúng** `expect_a`.
///
/// Không viết lại codec: [`DnsMessage::parse_response`] đã đòi `QR=1`, `id` khớp, `qdcount=1`
/// (đột biến (iii) làm nó ĐỎ); ở đây thêm bốn điều kiện **của bước này**: `RCODE=0` (đột biến
/// (iv)), câu hỏi đúng `name`/`A`/`IN`, có bản ghi `A`, và địa chỉ **bằng hằng kỳ vọng**.
pub fn dns_reply_accept(
    reply: &[u8],
    id: u16,
    name: &[u8],
    expect_a: Ipv4Addr,
) -> Result<DnsReply, NetError> {
    let message = DnsMessage::parse_response(reply, id).map_err(|_| NetError::BadFrame)?;
    let header = message.header();
    if header.rcode() != RCODE_NOERROR {
        return Err(NetError::BadFrame);
    }
    let question = message
        .question()
        .map_err(|_| NetError::BadFrame)?
        .ok_or(NetError::BadFrame)?;
    if question.qtype != TYPE_A || question.qclass != CLASS_IN {
        return Err(NetError::BadFrame);
    }
    if !question.name.eq_ignore_ascii_case(name) {
        return Err(NetError::BadFrame);
    }
    let a = message
        .first_a()
        .map_err(|_| NetError::BadFrame)?
        .ok_or(NetError::BadFrame)?;
    if a.addr != expect_a {
        return Err(NetError::BadFrame);
    }
    Ok(DnsReply {
        id: header.id,
        rcode: header.rcode(),
        question,
        a,
    })
}

/// Bộ lọc tự kiểm **chạy trong ảnh** (TAP 33): bơm lại **chính reply đã nhận** với `id` sai, `QR`
/// bị xoá và `RCODE=3`, rồi đòi [`dns_reply_accept`] từ chối — trả `(id_bị_từ_chối,
/// qr_bị_từ_chối, rcode_bị_từ_chối)`.
///
/// Ba đột biến **chỉ** đổi đúng trường của nó (không đụng độ dài/RDLENGTH), nên "bị từ chối" chỉ
/// có thể do phép kiểm tương ứng. Reply hỏng sẵn ⇒ `(false, false, false)` (không tự khen).
pub fn dns_filter_selfcheck(
    reply: &[u8],
    id: u16,
    name: &[u8],
    expect_a: Ipv4Addr,
) -> (bool, bool, bool) {
    if reply.len() < vifast_net::dns::DNS_HEADER_LEN || reply.len() > DNS_MAX_LEN {
        return (false, false, false);
    }
    if dns_reply_accept(reply, id, name, expect_a).is_err() {
        return (false, false, false);
    }
    let mut poisoned = [0u8; DNS_MAX_LEN];
    let len = reply.len();
    poisoned[..len].copy_from_slice(reply);
    // Byte 0..2 = `id`; byte 2..4 = `flags` (QR ở bit 15, `rcode` ở 4 bit thấp).
    poisoned[0] ^= 0xff;
    let id_ok = dns_reply_accept(&poisoned[..len], id, name, expect_a).is_err();
    poisoned[0] ^= 0xff;
    poisoned[2] &= 0x7f;
    let qr_ok = dns_reply_accept(&poisoned[..len], id, name, expect_a).is_err();
    poisoned[2] |= 0x80;
    poisoned[3] = (poisoned[3] & 0xf0) | RCODE_NXDOMAIN;
    let rcode_ok = dns_reply_accept(&poisoned[..len], id, name, expect_a).is_err();
    (id_ok, qr_ok, rcode_ok)
}

/// Gửi truy vấn DNS `A` cho `name` tới `peer.peer_ip:53` trên **một** [`NetIo`] bất kỳ và kiểm
/// reply bằng [`dns_reply_accept`], với **ba trần** của `budget`.
///
/// Reply **hỏng** (`id` lệch, `QR=0`, `RCODE≠0`, địa chỉ khác hằng) là lỗi **giao thức**: trả
/// `BadFrame` **không** thử lại (thử lại chỉ che lỗi). Chỉ **timeout** mới gửi lại — cùng `id`,
/// cùng message (RFC 1035 §4.2.1, khuôn KT-294).
pub fn dns_query_with(
    io: &mut dyn NetIo,
    peer: NetPeer,
    id: u16,
    name: &[u8],
    expect_a: Ipv4Addr,
    budget: RetryBudget,
    reply_out: &mut [u8],
) -> Result<DnsAnswer, NetError> {
    if budget.is_zero() {
        return Err(NetError::Timeout);
    }
    let query = DnsQuery::new(id, name, TYPE_A).map_err(|_| NetError::BadFrame)?;
    let mut request = [0u8; DNS_MAX_LEN];
    let request_len = query.encode(&mut request).map_err(|_| NetError::BadFrame)?;
    let mut frame = [0u8; DNS_FRAME_BYTES];
    let frame_len = udp_frame(
        peer.our_mac,
        peer.peer_mac,
        peer.our_ip,
        peer.peer_ip,
        DNS_CLIENT_PORT,
        DNS_PORT,
        &request[..request_len],
        &mut frame,
    )
    .ok_or(NetError::BadFrame)?;
    let total_deadline = io.now_us().saturating_add(budget.total_us);
    let mut attempts = 0u32;
    // **D2-246** — reply **hỏng/không khớp** chỉ là **một gói xấu**, không phải kết luận về cả
    // truy vấn: DNS/UDP có thể trả gói cắt/trùng/stale (soak 1/600 boot x86_64). Vì vậy đếm vào
    // `bad_replies` rồi **thử lại** trong cùng ngân sách; chỉ khi **hết** ngân sách mà vẫn chỉ
    // thấy gói xấu mới trả `BadFrame` (không thấy gói nào ⇒ `Timeout` như cũ).
    let mut bad_replies = 0u32;
    loop {
        if attempts >= budget.max_attempts {
            return Err(if bad_replies > 0 {
                NetError::BadFrame
            } else {
                NetError::Timeout
            });
        }
        attempts = attempts.saturating_add(1);
        let now = io.now_us();
        if now >= total_deadline {
            return Err(NetError::Timeout);
        }
        let send_budget = attempt_budget(now, total_deadline, budget.attempt_us);
        io.send(&frame[..frame_len], send_budget)?;
        let sent_at = io.now_us();
        if sent_at >= total_deadline {
            return Err(NetError::Timeout);
        }
        let attempt_deadline = sent_at
            .saturating_add(attempt_budget(sent_at, total_deadline, budget.attempt_us))
            .min(total_deadline);
        match udp_recv_until(
            io,
            peer.our_ip,
            peer.peer_ip,
            DNS_PORT,
            DNS_CLIENT_PORT,
            reply_out,
            attempt_deadline,
        ) {
            Ok(len) => {
                let reply = match dns_reply_accept(&reply_out[..len], id, name, expect_a) {
                    Ok(reply) => reply,
                    Err(NetError::BadFrame) => {
                        bad_replies = bad_replies.saturating_add(1);
                        continue;
                    }
                    Err(error) => return Err(error),
                };
                let rtt_us = io.now_us().saturating_sub(sent_at);
                let (filter_id, filter_qr, filter_rcode) =
                    dns_filter_selfcheck(&reply_out[..len], id, name, expect_a);
                return Ok(DnsAnswer {
                    reply,
                    rtt_us,
                    reply_len: len,
                    attempts,
                    bad_replies,
                    filter_id,
                    filter_qr,
                    filter_rcode,
                });
            }
            // Chỉ **timeout** mới thử lại; reply hỏng đã trả `BadFrame` ở trên.
            Err(NetError::Timeout) => continue,
            Err(error) => return Err(error),
        }
    }
}

/// **DNS query** trên driver thật: truy vấn `A` cho `name`, ba trần của module, reply vào
/// `reply_out` (caller giữ đệm — nhờ vậy phép tự kiểm bộ lọc dùng lại **chính** reply đã nhận).
pub fn dns_query(
    peer: NetPeer,
    id: u16,
    name: &[u8],
    expect_a: Ipv4Addr,
    reply_out: &mut [u8],
    now: &mut dyn FnMut() -> u64,
) -> Result<DnsAnswer, NetError> {
    let mut io = VirtioNetIo::new(now);
    dns_query_with(
        &mut io,
        peer,
        id,
        name,
        expect_a,
        RetryBudget::new(
            DNS_TOTAL_TIMEOUT_US,
            DNS_ATTEMPT_TIMEOUT_US,
            DNS_MAX_ATTEMPTS,
        ),
        reply_out,
    )
}

const _: () = {
    // `udp_frame` (ở [`crate::virtio_net`]) dựng frame UDP trong đệm `DHCP_MAX_LEN`; payload DNS
    // không EDNS là 512 byte ⇒ phải vừa, nếu không [`dns_query_with`] sẽ dựng frame bằng một đệm
    // hẹp hơn trần mà nó công bố.
    assert!(DNS_MAX_LEN <= DHCP_MAX_LEN);
    // Frame DNS lớn nhất phải vừa đệm nhận một frame của driver.
    assert!(DNS_FRAME_BYTES <= MAX_FRAME_BYTES);
};
