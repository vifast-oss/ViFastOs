// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ICMP echo** dùng chung 3 arch (KT-335b) — dựng/đọc khung, bộ lọc tự kiểm, và bước ping có
//! **ba trần** ([`RetryBudget`]).
//!
//! # Vì sao tệp này tồn tại
//!
//! Trước KT-335b có **ba** bản chính sách ICMP: riscv64 (KT-246) và aarch64 (KT-275) gần y hệt
//! (`ping(count)` + `icmp_echo_frame`), x86_64 (KT-328) có thêm bộ lọc tự kiểm và `RetryBudget`
//! (`ping_with`). Codec ICMP (`IcmpEcho::parse` kiểm checksum thật) đã ở [`crate::icmp`] từ
//! KT-244; tệp này chỉ còn **chính sách**.
//!
//! # Hai hình dạng gọi, **một** hợp đồng
//!
//! - x86_64: `count = 1`, ngân sách **ba trần** (6 s / 1 s / 3 lần) — TAP 32 in `attempts`.
//! - riscv64/aarch64: `count = 3`, mỗi echo có ngân sách riêng 3 s, **1** lần gửi — đúng hành vi
//!   cũ (mất echo đầu ⇒ `Timeout` ngay; mất giữa loạt ⇒ `Io` sau khi đủ `count`).
//!
//! Nhờ vậy ba arch dùng **cùng** máy trạng thái, cùng bộ lọc `id`/`seq`/payload, cùng cách bỏ
//! reply lạ trong **cùng** ngân sách lần thử — không arch nào có đường "chờ vô hạn".
//!
//! # Bất biến
//!
//! - Reply phải khớp **cả bốn lớp**: Ethernet/IPv4 + địa chỉ/MAC hai đầu; `payload()` IPv4 đã cắt
//!   đúng `total length`; [`IcmpEcho::parse`] kiểm **checksum ICMP**; `id`/`seq`/payload khớp.
//! - Reply của người khác bị **bỏ qua** trong cùng hạn (không tính là "mất gói", không đổi trạng
//!   thái) — nếu chỉ thấy reply lạ thì hết hạn ⇒ `Timeout`.
//! - Bộ lọc tự kiểm (`icmp_filter_selfcheck`) dựng lại reply hợp lệ với **một** trường bị đổi
//!   (checksum vẫn đúng) và đòi bị từ chối — đột biến (i)/(ii) của KT-328 làm TAP 32 ĐỎ.

use crate::addr::{Ipv4Addr, MacAddr};
use crate::client::{
    attempt_budget, past_deadline, Clock, Link, LinkError, NetPeer, RetryBudget, MAX_FRAME_BYTES,
};
use crate::eth::{EthHeader, ETHERTYPE_IPV4, ETH_HEADER_LEN};
use crate::frame::EthernetFrame;
use crate::icmp::{IcmpEcho, IcmpMessage, ICMP_HEADER_LEN};
use crate::ipv4::{Ipv4Header, IPV4_MIN_HEADER_LEN, PROTO_ICMP};

/// Cỡ payload ICMP echo (byte đệm) — cùng giá trị ba bản cũ.
pub const ICMP_PAYLOAD_BYTES: usize = 32;
/// `id` của echo request (chưa có RNG); reply **phải** khớp `id` này.
pub const ICMP_ECHO_ID: u16 = 0x4b54;
/// `seq` của echo request đầu tiên (một request mỗi lần thử).
pub const ICMP_ECHO_SEQ: u16 = 1;
/// Cỡ tối đa một frame ICMP echo (Ethernet + IPv4 + ICMP + payload) — **tính** từ hằng codec.
pub const ICMP_FRAME_BYTES: usize =
    ETH_HEADER_LEN + IPV4_MIN_HEADER_LEN + ICMP_HEADER_LEN + ICMP_PAYLOAD_BYTES;

/// Kết quả **một** bước ICMP echo đã kiểm.
///
/// Là **hợp** của hai bản cũ: x86_64 in `id`/`seq`/`payload_bytes`/`rtt_us`/`filter_*`; riscv64/
/// aarch64 in `sent`/`received`/`rtt_min_ms`/`rtt_max_ms`. Không bản nào mất trường đang in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PingStats {
    /// `id` đã gửi và đã khớp ở reply.
    pub id: u16,
    /// `seq` của echo **thành công gần nhất** (x86_64 đòi bằng [`ICMP_ECHO_SEQ`]).
    pub seq: u16,
    /// Tổng số frame echo request đã gửi, kể cả lần gửi lại.
    pub sent: u32,
    /// Số reply khớp đã nhận.
    pub received: u32,
    /// Cỡ payload đã gửi (reply phải chép **nguyên** — RFC 792).
    pub payload_bytes: usize,
    /// RTT của reply **thành công gần nhất** (đơn vị [`Clock`]).
    pub rtt_us: u64,
    /// RTT nhỏ nhất (ms) trên các reply đã nhận.
    pub rtt_min_ms: u64,
    /// RTT lớn nhất (ms) trên các reply đã nhận.
    pub rtt_max_ms: u64,
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

impl Default for PingStats {
    fn default() -> Self {
        Self {
            id: ICMP_ECHO_ID,
            seq: ICMP_ECHO_SEQ,
            sent: 0,
            received: 0,
            payload_bytes: ICMP_PAYLOAD_BYTES,
            rtt_us: 0,
            rtt_min_ms: 0,
            rtt_max_ms: 0,
            filter_id: false,
            filter_seq: false,
            filter_payload: false,
        }
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

/// Bộ lọc tự kiểm **chạy trong ảnh** (TAP 32 của x86_64): dựng reply hợp lệ rồi dựng lại với `id`,
/// `seq`, payload bị đổi — **checksum vẫn đúng** — và trả `(id_bị_từ_chối, seq_bị_từ_chối,
/// payload_bị_từ_chối)`.
///
/// Nếu reply hợp lệ **không** được nhận thì trả `(false, false, false)`: "từ chối tất cả" không
/// phải bằng chứng bộ lọc kiểm đúng chỗ. Đột biến (i)/(ii) của KT-328 làm một cờ thành `0` ⇒
/// TAP 32 **ĐỎ ngay trên QEMU**.
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

/// Hạn của **một lần thử**: `now + min(attempt, còn lại)`, không vượt hạn tổng.
fn attempt_deadline(now: u64, total_deadline: u64, attempt_us: u64) -> u64 {
    let candidate = now.wrapping_add(attempt_budget(now, total_deadline, attempt_us));
    if past_deadline(candidate, total_deadline) {
        total_deadline
    } else {
        candidate
    }
}

/// Một echo request + chờ reply khớp, thử lại trong `budget`; trả `(rtt, bộ lọc tự kiểm)`.
///
/// `sent` được cộng **mỗi** frame thực sự gửi (kể cả lần gửi lại) — nhờ vậy `PingStats::sent`
/// vừa là "số lần thử" của x86_64 vừa là "số echo" của riscv64/aarch64.
fn echo_once<C: Clock, L: Link>(
    clock: &C,
    link: &mut L,
    peer: NetPeer,
    seq: u16,
    payload: &[u8],
    budget: RetryBudget,
    sent: &mut u32,
) -> Result<(u64, (bool, bool, bool)), LinkError> {
    let mut frame = [0u8; ICMP_FRAME_BYTES];
    let frame_len = icmp_echo_frame(
        peer.our_ip,
        peer.our_mac,
        peer.peer_ip,
        peer.peer_mac,
        ICMP_ECHO_ID,
        seq,
        payload,
        &mut frame,
    )
    .ok_or(LinkError::BadFrame)?;
    let total_deadline = clock.now().wrapping_add(budget.total_us);
    let mut attempts = 0u32;
    while attempts < budget.max_attempts {
        attempts = attempts.saturating_add(1);
        *sent = sent.saturating_add(1);
        let now = clock.now();
        if past_deadline(now, total_deadline) {
            return Err(LinkError::Timeout);
        }
        let _send_budget = attempt_budget(now, total_deadline, budget.attempt_us);
        link.send_frame(&frame[..frame_len])?;
        // Đo lại sau khi gửi: thời gian gửi cũng tiêu ngân sách (một đồng hồ, một trần).
        let sent_at = clock.now();
        if past_deadline(sent_at, total_deadline) {
            return Err(LinkError::Timeout);
        }
        let deadline = attempt_deadline(sent_at, total_deadline, budget.attempt_us);
        let mut reply = [0u8; MAX_FRAME_BYTES];
        loop {
            let now = clock.now();
            if past_deadline(now, deadline) {
                break;
            }
            let remaining = deadline.wrapping_sub(now);
            // `remaining > 0` (đã kiểm ở trên) — **không bao giờ** truyền 0 xuống như "chờ mãi".
            let Some(got) = link.recv_frame(&mut reply, remaining) else {
                continue;
            };
            if !icmp_reply_accept(&reply[..got], peer, ICMP_ECHO_ID, seq, payload) {
                continue;
            }
            let rtt_us = clock.now().wrapping_sub(sent_at);
            let filters = icmp_filter_selfcheck(peer, ICMP_ECHO_ID, seq, payload);
            return Ok((rtt_us, filters));
        }
    }
    Err(LinkError::Timeout)
}

/// **ICMP echo** `count` lần tới `peer.peer_ip` trên **một** đường frame bất kỳ, với **ba trần**
/// của `budget` cho **mỗi** echo.
///
/// - `count = 1` ⇒ hành vi x86_64 (KT-328): ngân sách ba trần cho **một** echo; TAP 32 in
///   `attempts = stats.sent`.
/// - `count > 1` ⇒ hành vi riscv64/aarch64 (KT-246/KT-275): mỗi echo có ngân sách riêng; không
///   reply nào ở echo **đầu** ⇒ `Timeout` ngay; mất echo giữa loạt ⇒ [`LinkError::Io`] sau khi đủ
///   `count` (không giấu mất gói).
pub fn ping_with<C: Clock, L: Link>(
    clock: &C,
    link: &mut L,
    peer: NetPeer,
    count: u32,
    budget: RetryBudget,
) -> Result<PingStats, LinkError> {
    if budget.is_zero() {
        return Err(LinkError::Timeout);
    }
    let mut payload = [0u8; ICMP_PAYLOAD_BYTES];
    for (index, byte) in payload.iter_mut().enumerate() {
        *byte = index as u8;
    }
    let mut stats = PingStats::default();
    let mut seq = ICMP_ECHO_SEQ;
    let mut echoes = 0u32;
    while echoes < count {
        match echo_once(clock, link, peer, seq, &payload, budget, &mut stats.sent) {
            Ok((rtt_us, filters)) => {
                stats.received = stats.received.saturating_add(1);
                stats.seq = seq;
                stats.rtt_us = rtt_us;
                let rtt_ms = rtt_us / 1_000;
                if stats.received == 1 || rtt_ms < stats.rtt_min_ms {
                    stats.rtt_min_ms = rtt_ms;
                }
                if rtt_ms > stats.rtt_max_ms {
                    stats.rtt_max_ms = rtt_ms;
                }
                stats.filter_id |= filters.0;
                stats.filter_seq |= filters.1;
                stats.filter_payload |= filters.2;
            }
            Err(LinkError::Timeout) => {
                // Chưa từng có reply nào ⇒ coi như không có mạng (fail-closed, không chờ đủ loạt).
                if stats.received == 0 {
                    return Err(LinkError::Timeout);
                }
            }
            Err(error) => return Err(error),
        }
        echoes += 1;
        seq = seq.wrapping_add(1);
    }
    if stats.received != count {
        return Err(LinkError::Io);
    }
    Ok(stats)
}

const _: () = {
    // Một frame ICMP echo phải vừa đệm nhận một frame của driver.
    assert!(ICMP_FRAME_BYTES <= crate::client::MAX_FRAME_BYTES);
    assert!(ICMP_PAYLOAD_BYTES > 0 && ICMP_PAYLOAD_BYTES <= 64);
};
