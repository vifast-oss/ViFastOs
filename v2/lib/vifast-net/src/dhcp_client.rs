// SPDX-License-Identifier: MIT OR Apache-2.0
//! **DHCP client** dùng chung 3 arch (KT-335b) — DISCOVER → OFFER → REQUEST → ACK với **ba trần**
//! của [`RetryBudget`].
//!
//! # Vì sao tệp này tồn tại
//!
//! Codec BOOTP/DHCP đã ở [`crate::dhcp`] từ KT-244. Phần **chính sách** (ghép khung Ethernet +
//! IPv4 + UDP, lọc reply theo `xid`/`chaddr`/`msg_type`, chạy máy trạng thái, áp ba trần) trước
//! KT-335b nằm **ba bản**:
//!
//! | Arch | Bản cũ | Ba trần |
//! |---|---|---|
//! | riscv64 | `vifast-plat-riscv64/src/virtio_net.rs` (KT-246) | **một** timeout 3 s cho mỗi pha, **không** thử lại |
//! | aarch64 | `vifast-plat-aarch64/src/virtio_net.rs` (KT-275) | như riscv64, thêm `pub` `dhcp_frame*` cho host test |
//! | x86_64 | `vifast-plat-x86_64/src/virtio_net.rs` (KT-326) | **ba trần**: 6 s toàn bộ / 1 s mỗi lần / 3 lần mỗi pha |
//!
//! KT-335b chọn **hợp đồng của x86_64** làm hợp đồng chung (nó là bản đầy đủ nhất và là bản đã
//! có test host đo trần). riscv64/aarch64 **giữ nguyên hành vi cũ** bằng cách truyền
//! `RetryBudget { total_us: 2 × timeout, attempt_us: timeout, max_attempts: 1 }`: mỗi pha vẫn có
//! đúng ngân sách 3 s và **không** gửi lại — không nới ngưỡng nào của hai arch đó.
//!
//! # Fail-closed
//!
//! - Hết ngân sách/số lần ⇒ [`LinkError::Timeout`]; `0` ở bất kỳ trần nào ⇒ `Timeout` **ngay**,
//!   không gửi gì (`0` **không** phải "vô hạn" — đột biến (iii) của KT-326).
//! - Server trả ACK thiếu trường hoặc OFFER/ACK lệch nhau ⇒ [`LinkError::Io`] (không đoán).
//! - Ngân sách **đo lại sau khi gửi**: thời gian gửi cũng tiêu ngân sách (một đồng hồ, một trần).

use crate::addr::{Ipv4Addr, MacAddr, SubnetMask};
use crate::client::{
    attempt_budget, past_deadline, udp_frame, Clock, Link, LinkError, RetryBudget, MAX_FRAME_BYTES,
};
use crate::dhcp::{DhcpMessage, DHCPACK, DHCPOFFER, DHCP_MAX_LEN};
use crate::eth::ETHERTYPE_IPV4;
use crate::frame::EthernetFrame;
use crate::ipv4::{Ipv4Header, PROTO_UDP};
use crate::udp::{UdpHeader, DHCP_CLIENT_PORT, DHCP_SERVER_PORT};

/// Lease DHCP đã kiểm — đủ trường để in bằng chứng và để TAP 31 của x86_64 quyết định.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DhcpLease {
    /// Địa chỉ server cấp (`yiaddr`).
    pub ip: Ipv4Addr,
    /// Mặt nạ mạng (option 1).
    pub mask: Ipv4Addr,
    /// Gateway/router đầu tiên (option 3).
    pub gw: Ipv4Addr,
    /// DNS đầu tiên (option 6).
    pub dns: Ipv4Addr,
    /// Server identifier (option 54).
    pub server: Ipv4Addr,
    /// Thời hạn lease (option 51), giây.
    pub lease_secs: u32,
    /// Tổng số frame đã **gửi** (DISCOVER + REQUEST), kể cả lần gửi lại.
    pub attempts: u32,
    /// `true` ⇔ bộ lọc đã từ chối một ACK bị hỏng **magic cookie** (đột biến (i)).
    pub cookie_rejected: bool,
    /// `true` ⇔ bộ lọc đã từ chối một ACK bị đổi **xid** (đột biến (ii)).
    pub xid_rejected: bool,
}

impl DhcpLease {
    /// `true` ⇔ lease **nhất quán**: địa chỉ/mặt nạ/gateway/DNS/server hợp lệ, cùng mạng, lease
    /// dương. Đây là điều kiện **của TAP 31** (x86_64), không phải "đọc được một dòng".
    pub fn valid(&self) -> bool {
        if self.ip.is_unspecified()
            || self.ip.is_broadcast()
            || self.gw.is_unspecified()
            || self.gw.is_broadcast()
            || self.dns.is_unspecified()
            || self.dns.is_broadcast()
            || self.server.is_unspecified()
            || self.server.is_broadcast()
            || self.lease_secs == 0
            || self.ip == self.gw
            || self.ip == self.dns
        {
            return false;
        }
        let Some(mask) = SubnetMask::from_addr(self.mask) else {
            return false;
        };
        // Mặt nạ /0 hoặc /32 không phải cấu hình mà một DHCP server lành gửi cho client.
        if mask.prefix() == 0 || mask.prefix() == 32 {
            return false;
        }
        crate::addr::same_subnet(self.ip, self.gw, mask)
            && crate::addr::same_subnet(self.ip, self.dns, mask)
    }

    /// `true` ⇔ **cả hai** phép tự kiểm của bộ lọc đều bắt được gói độc (xem
    /// [`dhcp_filter_selfcheck`]).
    pub const fn filter_ok(&self) -> bool {
        self.cookie_rejected && self.xid_rejected
    }
}

/// Dựng frame DHCP **client → server** (broadcast, cổng 68 → 67) vào `out`; trả tổng số byte.
pub fn dhcp_frame(message: &DhcpMessage, mac: MacAddr, out: &mut [u8]) -> Option<usize> {
    udp_frame(
        mac,
        MacAddr::BROADCAST,
        Ipv4Addr::UNSPECIFIED,
        Ipv4Addr::BROADCAST,
        DHCP_CLIENT_PORT,
        DHCP_SERVER_PORT,
        message.as_bytes(),
        out,
    )
}

/// Đọc frame DHCP **server → client** và chỉ nhận khi khớp **cả ba**: `xid`, `chaddr`,
/// `msg_type`. Trả `None` cho frame khác (không PANIC, không đoán).
pub fn dhcp_frame_parse(
    frame: &[u8],
    xid: u32,
    mac: MacAddr,
    want_type: u8,
) -> Option<DhcpMessage> {
    let eth = EthernetFrame::parse(frame).ok()?;
    if eth.header.ethertype != ETHERTYPE_IPV4 {
        return None;
    }
    let ip = Ipv4Header::parse(eth.payload, true).ok()?;
    if ip.header.protocol != PROTO_UDP {
        return None;
    }
    let udp = UdpHeader::parse(ip.header.src, ip.header.dst, ip.payload(), true).ok()?;
    if udp.header.src_port != DHCP_SERVER_PORT || udp.header.dst_port != DHCP_CLIENT_PORT {
        return None;
    }
    let message = DhcpMessage::parse(udp.payload()).ok()?;
    if message.xid() != xid || message.chaddr() != mac || message.msg_type() != Some(want_type) {
        return None;
    }
    Some(message)
}

/// Đọc lease từ ACK; **từ chối** ACK thiếu mặt nạ/router/DNS/lease/server hoặc lease không nhất
/// quán (không đoán giá trị thiếu).
pub fn dhcp_lease_from_ack(ack: &DhcpMessage, attempts: u32) -> Result<DhcpLease, LinkError> {
    let lease = DhcpLease {
        ip: ack.yiaddr(),
        mask: ack.subnet_mask().ok_or(LinkError::Io)?,
        gw: ack.router().ok_or(LinkError::Io)?,
        dns: ack.dns().ok_or(LinkError::Io)?,
        server: ack.server_id().ok_or(LinkError::Io)?,
        lease_secs: ack.lease_secs().ok_or(LinkError::Io)?,
        attempts,
        cookie_rejected: false,
        xid_rejected: false,
    };
    if !lease.valid() {
        return Err(LinkError::Io);
    }
    Ok(lease)
}

/// Bộ lọc tự kiểm **chạy trong ảnh** (TAP 31 của x86_64): bơm lại chính payload ACK với
/// (1) magic cookie sai, (2) `xid` lạ; trả `(cookie_bị_từ_chối, xid_bị_từ_chối)`.
///
/// Frame độc được dựng qua [`udp_frame`] nên **checksum đúng** — nếu bộ lọc nhận nó, nguyên nhân
/// **chỉ** có thể là thiếu phép kiểm cookie/xid, không phải lỗi checksum.
pub fn dhcp_filter_selfcheck(mac: MacAddr, xid: u32, ack_payload: &[u8]) -> (bool, bool) {
    let Ok(ack) = DhcpMessage::parse(ack_payload) else {
        return (false, false);
    };
    let src_ip = ack.server_id().unwrap_or(Ipv4Addr::UNSPECIFIED);
    let dst_ip = ack.yiaddr();
    let mut payload = [0u8; DHCP_MAX_LEN];
    let len = ack_payload.len().min(DHCP_MAX_LEN);
    payload[..len].copy_from_slice(&ack_payload[..len]);

    // (1) magic cookie sai — mọi byte còn lại giữ nguyên.
    payload[236] ^= 0xff;
    let cookie_rejected = dhcp_poison_rejected(&payload[..len], mac, xid, src_ip, dst_ip);
    payload[236] ^= 0xff;

    // (2) `xid` lạ — cookie và checksum vẫn đúng.
    payload[4] ^= 0xff;
    let xid_rejected = dhcp_poison_rejected(&payload[..len], mac, xid, src_ip, dst_ip);
    (cookie_rejected, xid_rejected)
}

/// `true` ⇔ frame chứa `payload` bị [`dhcp_frame_parse`] **từ chối**.
fn dhcp_poison_rejected(
    payload: &[u8],
    mac: MacAddr,
    xid: u32,
    src_ip: Ipv4Addr,
    dst_ip: Ipv4Addr,
) -> bool {
    let mut frame = [0u8; DHCP_MAX_LEN + 64];
    let Some(len) = udp_frame(
        mac,
        mac,
        src_ip,
        dst_ip,
        DHCP_SERVER_PORT,
        DHCP_CLIENT_PORT,
        payload,
        &mut frame,
    ) else {
        return false;
    };
    dhcp_frame_parse(&frame[..len], xid, mac, DHCPACK).is_none()
}

/// Chờ một frame DHCP khớp `xid`/`mac`/`want_type` tới **deadline tuyệt đối** `deadline`.
fn dhcp_wait<C: Clock, L: Link>(
    clock: &C,
    link: &mut L,
    xid: u32,
    mac: MacAddr,
    want_type: u8,
    deadline: u64,
) -> Result<DhcpMessage, LinkError> {
    let mut frame = [0u8; MAX_FRAME_BYTES];
    loop {
        let now = clock.now();
        if past_deadline(now, deadline) {
            return Err(LinkError::Timeout);
        }
        let remaining = deadline.wrapping_sub(now);
        // `remaining > 0` (đã kiểm ở trên) — **không bao giờ** truyền 0 xuống như "chờ mãi".
        if let Some(len) = link.recv_frame(&mut frame, remaining) {
            if let Some(message) = dhcp_frame_parse(&frame[..len], xid, mac, want_type) {
                return Ok(message);
            }
        }
    }
}

/// **DHCP client** trên **một** đường frame bất kỳ (driver thật hoặc fake trong test host):
/// DISCOVER → OFFER → REQUEST → ACK, với **ba trần** của `budget`.
///
/// `budget.is_zero()` ⇒ `Timeout` **ngay**, không gửi/không chờ. Trả [`LinkError::Timeout`] khi
/// hết ngân sách/tổng số lần thử, [`LinkError::Io`] khi server trả ACK thiếu trường hoặc
/// OFFER/ACK lệch nhau (fail-closed, không đoán).
pub fn dhcp_acquire_with<C: Clock, L: Link>(
    clock: &C,
    link: &mut L,
    mac: MacAddr,
    xid: u32,
    budget: RetryBudget,
) -> Result<DhcpLease, LinkError> {
    if budget.is_zero() {
        return Err(LinkError::Timeout);
    }
    let total_deadline = clock.now().wrapping_add(budget.total_us);
    let mut frame = [0u8; DHCP_MAX_LEN + 64];
    let mut attempts = 0u32;

    // ---------------------------------------------------------------- DISCOVER → OFFER
    let discover = DhcpMessage::discover(xid, mac);
    let discover_len = dhcp_frame(&discover, mac, &mut frame).ok_or(LinkError::BadFrame)?;
    let mut offer: Option<DhcpMessage> = None;
    let mut tried = 0u32;
    while tried < budget.max_attempts {
        tried += 1;
        attempts = attempts.saturating_add(1);
        let now = clock.now();
        if past_deadline(now, total_deadline) {
            return Err(LinkError::Timeout);
        }
        // Ngân sách gửi nằm trong trần chung; `Link::send_frame` chỉ có trần poll của driver nên
        // phép tính này giữ đúng hợp đồng ba trần (và được đo lại ngay sau khi gửi bên dưới).
        let _send_budget = attempt_budget(now, total_deadline, budget.attempt_us);
        link.send_frame(&frame[..discover_len])?;
        // Đo lại sau khi gửi: thời gian gửi cũng tiêu ngân sách (một đồng hồ, một trần).
        let now = clock.now();
        if past_deadline(now, total_deadline) {
            return Err(LinkError::Timeout);
        }
        let attempt_deadline = budget_deadline(now, total_deadline, budget.attempt_us);
        match dhcp_wait(clock, link, xid, mac, DHCPOFFER, attempt_deadline) {
            Ok(message) => {
                offer = Some(message);
                break;
            }
            Err(LinkError::Timeout) => continue,
            Err(error) => return Err(error),
        }
    }
    let offer = offer.ok_or(LinkError::Timeout)?;
    let yiaddr = offer.yiaddr();
    if yiaddr.is_unspecified() || yiaddr.is_broadcast() {
        return Err(LinkError::Io);
    }
    let server = offer.server_id().ok_or(LinkError::Io)?;

    // ---------------------------------------------------------------- REQUEST → ACK
    let request = DhcpMessage::request(xid, mac, yiaddr, server);
    let request_len = dhcp_frame(&request, mac, &mut frame).ok_or(LinkError::BadFrame)?;
    let mut ack: Option<DhcpMessage> = None;
    let mut tried = 0u32;
    while tried < budget.max_attempts {
        tried += 1;
        attempts = attempts.saturating_add(1);
        let now = clock.now();
        if past_deadline(now, total_deadline) {
            return Err(LinkError::Timeout);
        }
        let _send_budget = attempt_budget(now, total_deadline, budget.attempt_us);
        link.send_frame(&frame[..request_len])?;
        let now = clock.now();
        if past_deadline(now, total_deadline) {
            return Err(LinkError::Timeout);
        }
        let attempt_deadline = budget_deadline(now, total_deadline, budget.attempt_us);
        match dhcp_wait(clock, link, xid, mac, DHCPACK, attempt_deadline) {
            Ok(message) => {
                ack = Some(message);
                break;
            }
            Err(LinkError::Timeout) => continue,
            Err(error) => return Err(error),
        }
    }
    let ack = ack.ok_or(LinkError::Timeout)?;
    if ack.yiaddr() != yiaddr {
        return Err(LinkError::Io);
    }
    let (cookie_rejected, xid_rejected) = dhcp_filter_selfcheck(mac, xid, ack.as_bytes());
    let mut lease = dhcp_lease_from_ack(&ack, attempts)?;
    lease.cookie_rejected = cookie_rejected;
    lease.xid_rejected = xid_rejected;
    Ok(lease)
}

/// Hạn của **một lần thử**: `now + min(attempt, còn lại)`, không vượt hạn tổng.
fn budget_deadline(now: u64, total_deadline: u64, attempt_us: u64) -> u64 {
    let candidate = now.wrapping_add(attempt_budget(now, total_deadline, attempt_us));
    if past_deadline(candidate, total_deadline) {
        total_deadline
    } else {
        candidate
    }
}
