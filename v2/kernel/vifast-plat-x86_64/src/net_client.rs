// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Adapter ICMP + DNS** của x86_64 — KT-328 (K5': ICMP/DNS) → **KT-335b** (chính sách về
//! `vifast-net`).
//!
//! Tách khỏi [`crate::virtio_net`] vì **luật cấu trúc R1** (tệp trong `kernel/` ≤ 1.500 dòng):
//! driver virtio-net + ARP + DHCP + phần này trong **một** tệp từng là 1.965 dòng. `virtio_net`
//! **re-export** mọi tên ở đây nên ảnh (`kbin`) và host test vẫn dùng đường
//! `vifast_plat_x86_64::virtio_net::{ping, dns_query, …}` — **không** có API thứ hai.
//!
//! # Vì sao tệp này chỉ còn **adapter**
//!
//! Trước KT-335b, ICMP/DNS của x86_64 nằm ở đây (782 dòng). KT-335b dời **toàn bộ** chính sách
//! về [`vifast_net::icmp_client`]/[`vifast_net::dns_client`] — nơi host test chạy được (D2-05) và
//! nơi **D2-246** (reply hỏng ⇒ đếm `bad_replies` rồi thử lại trong ngân sách) là hợp đồng chung
//! cho cả ba arch. Tệp này giữ **nguyên** chữ ký cũ trên [`crate::virtio_net::NetIo`] để ảnh và
//! host test không phải đổi, và để ba trần của x86_64 **không đổi số**:
//!
//! | Bước | Toàn bộ | Mỗi lần | Số lần |
//! |---|---:|---:|---:|
//! | ICMP | [`ICMP_TOTAL_TIMEOUT_US`] 6 s | [`ICMP_ATTEMPT_TIMEOUT_US`] 1 s | [`ICMP_MAX_ATTEMPTS`] 3 |
//! | DNS | [`DNS_TOTAL_TIMEOUT_US`] 6 s | [`DNS_ATTEMPT_TIMEOUT_US`] 3 s | [`DNS_MAX_ATTEMPTS`] 2 |
//!
//! Nhờ vậy TAP 32/33 in `attempts`/`elapsed` trong cùng khoảng đã đo, và `dns_query_with` vẫn
//! **giữ D2-246**: gói xấu không bỏ cả truy vấn — xem [`vifast_net::dns_client::dns_query_with`].

use vifast_net::dns::DnsRetry;
use vifast_net::dns_client;
use vifast_net::icmp_client;

use vifast_virtio::net::NetError;

use crate::virtio_net::{to_net_error, Bridge, NetIo, VirtioNetIo};

// Ba trần ICMP của x86_64 (giữ nguyên số từ KT-328).
/// Trần **toàn bộ** bước ICMP (µs): 6 s.
pub const ICMP_TOTAL_TIMEOUT_US: u64 = 6_000_000;
/// Trần **mỗi lần** gửi/chờ của ICMP (µs): 1 s.
pub const ICMP_ATTEMPT_TIMEOUT_US: u64 = 1_000_000;
/// Số lần thử của bước ICMP — trần, không retry vô hạn.
pub const ICMP_MAX_ATTEMPTS: u32 = 3;
/// `id` của truy vấn DNS của ảnh x86_64 (chưa có RNG); reply **phải** khớp `id` này.
pub const DNS_ID: u16 = 0x4b55;
/// Trần **toàn bộ** bước DNS (µs): 6 s — bằng `2 × [`DNS_ATTEMPT_TIMEOUT_US`]`.
pub const DNS_TOTAL_TIMEOUT_US: u64 = 6_000_000;
/// Trần **mỗi lần** gửi/chờ của DNS (µs): 3 s — **không** đổi so với KT-294/D2-215.
pub const DNS_ATTEMPT_TIMEOUT_US: u64 = 3_000_000;
/// Số lần thử của DNS — **suy ra** từ trần của codec ([`vifast_net::dns::DNS_MAX_ATTEMPTS`], 2).
pub const DNS_MAX_ATTEMPTS: u32 = vifast_net::dns::DNS_MAX_ATTEMPTS as u32;

// Hằng/khung/đọc reply: **re-export** từ crate chung — ảnh và host test không có bản thứ hai.
use vifast_net::addr::Ipv4Addr;
pub use vifast_net::client::{NetPeer, RetryBudget};
pub use vifast_net::dns_client::{
    dns_filter_selfcheck, DnsAnswer, DnsReply, DNS_CLIENT_PORT, DNS_FRAME_BYTES, DNS_MAX_LEN,
    DNS_PORT,
};

/// Kiểm **một** message reply DNS **chặt** — **API cũ** của x86_64 (KT-328): lỗi trả `NetError`
/// để ảnh/host test không phải đổi kiểu lỗi.
pub fn dns_reply_accept(
    reply: &[u8],
    id: u16,
    name: &[u8],
    expect_a: Ipv4Addr,
) -> Result<DnsReply, NetError> {
    dns_client::dns_reply_accept(reply, id, name, expect_a).map_err(to_net_error)
}
pub use vifast_net::icmp_client::{
    icmp_echo_frame, icmp_filter_selfcheck, icmp_reply_accept, icmp_reply_frame, PingStats,
    ICMP_ECHO_ID, ICMP_ECHO_SEQ, ICMP_FRAME_BYTES, ICMP_PAYLOAD_BYTES,
};

/// **ICMP echo** một request tới `peer.peer_ip` trên **một** [`NetIo`] bất kỳ (driver thật hoặc
/// fake trong host test), với **ba trần** của `budget`.
///
/// ADAPTER của [`vifast_net::icmp_client::ping_with`] với `count = 1` (đúng hành vi x86_64:
/// TAP 32 in `attempts = stats.sent`); `Err(NetError::Timeout)` khi hết ngân sách/số lần thử,
/// `Err(NetError::BadFrame)` khi không dựng được frame.
pub fn ping_with(
    io: &mut dyn NetIo,
    peer: NetPeer,
    budget: RetryBudget,
) -> Result<PingStats, NetError> {
    let bridge = Bridge::new(io);
    let clock = bridge.clock();
    let mut link = bridge.link();
    icmp_client::ping_with(&clock, &mut link, peer, 1, budget).map_err(to_net_error)
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

/// **DNS query** trên **một** [`NetIo`] bất kỳ: truy vấn `A` cho `name`, kiểm reply bằng bộ lọc
/// chặt (`RCODE=0`, đúng câu hỏi, bản ghi `A` **bằng** `expect_a`), với **ba trần** của `budget`.
///
/// **D2-246**: reply hỏng/không khớp được **đếm** rồi **thử lại** trong cùng ngân sách; chỉ khi
/// hết ngân sách/lượt mà vẫn chỉ thấy gói hỏng mới trả `BadFrame` (không thấy gói nào ⇒
/// `Timeout`). Đây là hợp đồng đã vá flake soak `1/600` boot x86_64 (KT-335c) — **không** được
/// quay lại `?` như bản trước KT-335c.
#[allow(clippy::too_many_arguments)] // chữ ký cũ của KT-328 (ảnh + host test đang dùng)
pub fn dns_query_with(
    io: &mut dyn NetIo,
    peer: NetPeer,
    id: u16,
    name: &[u8],
    expect_a: Ipv4Addr,
    budget: RetryBudget,
    reply_out: &mut [u8],
) -> Result<DnsAnswer, NetError> {
    let bridge = Bridge::new(io);
    let clock = bridge.clock();
    let mut link = bridge.link();
    let mut retry = DnsRetry::new();
    dns_client::dns_query_with(
        &clock,
        &mut link,
        peer,
        id,
        name,
        Some(expect_a),
        budget,
        reply_out,
        &mut retry,
    )
    .map_err(to_net_error)
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
