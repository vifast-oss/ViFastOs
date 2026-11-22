// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Chính sách mạng** dùng chung 3 arch — phần khung/thời gian nền (KT-335b, D2-231).
//!
//! # Vì sao tệp này tồn tại
//!
//! KT-335 đã gộp transport virtio-net (`vifast-virtio::netdev`) và chính sách **TCP client**
//! (`vifast_net::tcp_client`). KT-335b làm nốt nửa sau: **DHCP/ARP/ICMP/UDP/DNS** — trước đó ba
//! bản sao nằm trong `vifast-plat-{riscv64,aarch64,x86_64}/src/virtio_net.rs` (x86_64 còn thêm
//! `net_client.rs`), mỗi bản chỉ chạy được trên QEMU.
//!
//! Tệp này giữ phần **nền** của cả năm chính sách:
//!
//! - hai trait biên [`Clock`] + [`Link`] — **dùng lại nguyên** từ [`crate::tcp_client`] (một biên
//!   frame/đồng hồ cho cả TCP lẫn UDP/DHCP/ICMP/DNS; không có bản thứ hai);
//! - [`RetryBudget`] — **hợp đồng ba trần** (toàn bộ / mỗi lần / số lần) đã có ở x86_64 từ
//!   KT-326; KT-335b nâng thành hợp đồng chung cho DHCP/ICMP/DNS (xem `dhcp_client`/`icmp_client`/
//!   `dns_client`);
//! - [`NetPeer`] — hai đầu của một bước mạng (ta ↔ đích) để call-site không truyền bốn tham số
//!   rời và không hoán vị nhầm "ta" với "đích";
//! - [`udp_frame`]/[`udp_send`]/[`udp_recv_until`] — ghép/đọc **một** datagram UDP trên Ethernet
//!   (dùng chung cho DHCP, DNS và "gương" UDP echo).
//!
//! # Ba trần (hợp đồng, không phải hằng số)
//!
//! [`RetryBudget`] là **hợp đồng**, còn **số** thì do từng nền tảng quyết định — vì ba nền tảng đã
//! đo được ba ngân sách khác nhau và KT-335b **không** nới ngưỡng nào:
//!
//! | Bước | x86_64 (KT-326/KT-328) | riscv64/aarch64 (KT-246/KT-275) |
//! |---|---|---|
//! | DHCP | 6 s toàn bộ, 1 s/lần, 3 lần/pha | 3 s/lần cho **mỗi pha**, **1** lần (đúng như cũ) |
//! | ICMP | 6 s toàn bộ, 1 s/lần, 3 lần | mỗi echo 3 s, **1** lần mỗi echo |
//! | DNS | 6 s toàn bộ, 3 s/lần, 2 lần | mỗi lần 3 s, tối đa 2 lần (KT-294) |
//!
//! Đơn vị là **đơn vị của [`Clock`]** — tick timebase (riscv64), µs `CNTPCT` (aarch64), µs PIT
//! (x86_64). Nhờ vậy cùng một hàm chạy đúng trên cả ba mà không phải quy đổi trong driver.
//!
//! # Bất biến
//!
//! - **Fail-closed**: hết hạn ⇒ [`LinkError::Timeout`]; frame hỏng ⇒ [`LinkError::BadFrame`];
//!   không PANIC, không treo, không đoán.
//! - **Bỏ qua frame không khớp** (ARP/ICMP, IP/cổng khác, checksum sai) rồi chờ tiếp.
//! - `#![no_std]`, **không** cấp phát: mọi bộ đệm là mảng trên stack của caller.
//! - `0` **không** phải "vô hạn": [`RetryBudget::is_zero`] ⇒ trả `Timeout` **trước** khi gửi.

use crate::addr::{Ipv4Addr, MacAddr};
use crate::dhcp::DHCP_MAX_LEN;
use crate::eth::{EthHeader, ETHERTYPE_IPV4, ETH_HEADER_LEN};
use crate::frame::EthernetFrame;
use crate::ipv4::{Ipv4Header, IPV4_MIN_HEADER_LEN, PROTO_UDP};
use crate::udp::{UdpHeader, UDP_HEADER_LEN};

pub use crate::tcp_client::{Clock, Link, LinkError};

/// Cỡ frame Ethernet lớn nhất trên dây (14 + 1500) — cùng giá trị `MAX_FRAME_BYTES` của
/// `vifast-virtio`; crate này **không** phụ thuộc crate kia nên hằng được khai lại ở đây.
pub const MAX_FRAME_BYTES: usize = 1514;
/// Cỡ payload UDP lớn nhất mà bộ ghép khung nhận: BOOTP/DHCP là 576 byte (RFC 951) — trần rộng
/// hơn DNS không EDNS (512) nên **một** hàm ghép khung dùng được cho cả ba bước.
pub const MAX_UDP_PAYLOAD: usize = DHCP_MAX_LEN;

/// Ba trần của **một** bước mạng có thử lại.
///
/// Đây là **hợp đồng chung** giữa ba arch (KT-335b): DHCP/ICMP/DNS đều nhận cùng struct, mỗi
/// nền tảng truyền **số đã đo** của mình (bảng ở đầu tệp) — không arch nào được tự do bỏ một
/// trần. `0` ở **bất kỳ** trường nào nghĩa là "không có ngân sách" ⇒ `Timeout` **ngay**, không
/// gửi gì; `0` **không** phải "vô hạn" (đột biến (iii) của KT-326 giữ nguyên hiệu lực).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetryBudget {
    /// Ngân sách **toàn bộ** bước — deadline tuyệt đối, cùng đơn vị [`Clock`].
    pub total_us: u64,
    /// Ngân sách **mỗi lần** gửi/chờ — cùng đơn vị [`Clock`].
    pub attempt_us: u64,
    /// Số lần gửi tối đa (mỗi pha với DHCP).
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
    pub const fn is_zero(&self) -> bool {
        self.total_us == 0 || self.attempt_us == 0 || self.max_attempts == 0
    }
}

/// Ngân sách còn lại cho **một lần thử**: `min(attempt_timeout, còn lại tới deadline tổng)`.
///
/// Caller phải bảo đảm `now` **chưa** qua `total_deadline` (mọi vòng lặp đều kiểm trước); hàm
/// dùng phép trừ có wrap để an toàn với bộ đếm `rdtime`/`CNTPCT` chạy vòng.
pub const fn attempt_budget(now: u64, total_deadline: u64, attempt_timeout: u64) -> u64 {
    let remaining = total_deadline.wrapping_sub(now);
    if remaining < attempt_timeout {
        remaining
    } else {
        attempt_timeout
    }
}

/// `true` ⇔ `deadline` **đã** qua so với `now`; an toàn với wrap (cùng `tcp_client::past_deadline`).
pub use crate::tcp_client::past_deadline;

/// Hai đầu của **một** bước mạng trên Ethernet: IP/MAC của ta và của đích.
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

/// Ghép **một** frame Ethernet + IPv4 + UDP (checksum đầy đủ) vào `out`; trả tổng số byte.
///
/// Dùng cho **cả ba** bước có UDP: DHCP DISCOVER/REQUEST (68 → 67), DNS query (cổng cao → 53), và
/// frame "độc" của các phép tự kiểm bộ lọc — nhờ vậy frame độc có checksum **đúng**, và lý do
/// duy nhất bị từ chối là trường bị đổi (cookie/`id`/`xid`/…), không phải checksum.
///
/// `None` nếu `payload` vượt [`MAX_UDP_PAYLOAD`] hoặc `out` không đủ chỗ (không cắt im lặng).
#[allow(clippy::too_many_arguments)] // một frame = các trường của nó; gói lại chỉ thêm lớp
pub fn udp_frame(
    src_mac: MacAddr,
    dst_mac: MacAddr,
    src_ip: Ipv4Addr,
    dst_ip: Ipv4Addr,
    src_port: u16,
    dst_port: u16,
    payload: &[u8],
    out: &mut [u8],
) -> Option<usize> {
    if payload.len() > MAX_UDP_PAYLOAD {
        return None;
    }
    let mut udp = [0u8; MAX_UDP_PAYLOAD + UDP_HEADER_LEN];
    let udp_len = UdpHeader::encode(src_ip, dst_ip, src_port, dst_port, payload, &mut udp).ok()?;
    let mut ip = [0u8; MAX_UDP_PAYLOAD + UDP_HEADER_LEN + IPV4_MIN_HEADER_LEN];
    let header_len = Ipv4Header::new(PROTO_UDP, src_ip, dst_ip, udp_len)
        .encode(&mut ip)
        .ok()?;
    ip[header_len..header_len + udp_len].copy_from_slice(&udp[..udp_len]);
    let ip_len = header_len + udp_len;
    EthernetFrame {
        header: EthHeader::new(dst_mac, src_mac, ETHERTYPE_IPV4),
        payload: &ip[..ip_len],
    }
    .encode(out)
    .ok()
}

/// Gửi **một** datagram UDP (Ethernet + IPv4 + UDP) tới `dst_ip:dst_port`.
///
/// `BadFrame` nếu payload vượt trần hoặc không dựng được khung; lỗi của đường dây được chuyển
/// nguyên từ [`Link`].
#[allow(clippy::too_many_arguments)] // hai đầu cuối (IP+MAC) + hai cổng + payload + link
pub fn udp_send<L: Link>(
    link: &mut L,
    src_mac: MacAddr,
    dst_mac: MacAddr,
    src_ip: Ipv4Addr,
    dst_ip: Ipv4Addr,
    src_port: u16,
    dst_port: u16,
    payload: &[u8],
) -> Result<(), LinkError> {
    if payload.len() > MAX_UDP_PAYLOAD {
        return Err(LinkError::BadFrame);
    }
    let mut frame = [0u8; ETH_HEADER_LEN + IPV4_MIN_HEADER_LEN + UDP_HEADER_LEN + MAX_UDP_PAYLOAD];
    let len = udp_frame(
        src_mac, dst_mac, src_ip, dst_ip, src_port, dst_port, payload, &mut frame,
    )
    .ok_or(LinkError::BadFrame)?;
    link.send_frame(&frame[..len])
}

/// Chờ **một** datagram UDP khớp `src_ip:src_port` → `our_ip:dst_port` tới deadline **tuyệt đối**
/// `deadline`; copy payload vào `out` và trả số byte.
///
/// Frame không khớp (ARP, ICMP, cổng khác, **checksum sai**) bị **bỏ qua** rồi chờ tiếp — cùng
/// cách `dhcp_wait`/`ping` đang làm. Hết hạn ⇒ [`LinkError::Timeout`]; payload dài hơn `out` ⇒
/// [`LinkError::BadFrame`] (không cắt im lặng).
///
/// Tham số `true` của [`UdpHeader::parse`] là **phép kiểm checksum** — đột biến (i) của KT-335b
/// đổi nó sang `false` và **phải** làm `tests/net_client.rs::udp_recv_ignores_bad_checksum` ĐỎ.
#[allow(clippy::too_many_arguments)] // địa chỉ/cổng hai đầu + đệm ra + deadline
pub fn udp_recv_until<C: Clock, L: Link>(
    clock: &C,
    link: &mut L,
    our_ip: Ipv4Addr,
    src_ip: Ipv4Addr,
    src_port: u16,
    dst_port: u16,
    out: &mut [u8],
    deadline: u64,
) -> Result<usize, LinkError> {
    let mut frame = [0u8; MAX_FRAME_BYTES];
    loop {
        let now = clock.now();
        if past_deadline(now, deadline) {
            return Err(LinkError::Timeout);
        }
        let remaining = deadline.wrapping_sub(now);
        let Some(got) = link.recv_frame(&mut frame, remaining) else {
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
            return Err(LinkError::BadFrame);
        }
        out[..payload.len()].copy_from_slice(payload);
        return Ok(payload.len());
    }
}
