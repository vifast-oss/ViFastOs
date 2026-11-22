// SPDX-License-Identifier: MIT OR Apache-2.0
//! **UDP echo** dùng chung 3 arch (KT-335b) — gửi một datagram, chờ **đúng** payload quay lại.
//!
//! Trước KT-335b khối này nằm hai bản gần y hệt trong `vifast-plat-{riscv64,aarch64}/src/
//! virtio_net.rs` (KT-260/KT-275) và chỉ khác đồng hồ (tick timebase vs `CNTPCT`). Bước này là
//! "gương" UDP của bằng chứng mạng: libslirp **không** có echo service ở `10.0.2.2:7`, nên
//! `udp_echo` tới cổng đó là **ca âm fail-closed 3 s** có bằng chứng đo được; vòng UDP hai chiều
//! thật lấy từ **reply DNS** (server chép nguyên question section — RFC 1035 §4.1.2), xem
//! `vifast-kbin-riscv64/src/ktest_net.rs`.
//!
//! # Bất biến
//!
//! - Ngân sách `timeout` tính cho **cả bước** (gửi + chờ) — KT-268: nếu hạn chờ bắt đầu **sau**
//!   khi gửi thì ca timeout in `elapsed_ms = 3000 + thời gian gửi`, thành flake dưới tải.
//! - Reply khác payload ⇒ [`LinkError::BadFrame`] (không nhận "gần đúng").
//! - Hết hạn ⇒ [`LinkError::Timeout`]; payload vượt trần ⇒ `BadFrame` **trước** khi chạm dây.

use crate::addr::{Ipv4Addr, MacAddr};
use crate::client::{past_deadline, udp_recv_until, udp_send, Clock, Link, LinkError};
use crate::udp::UdpHeader;

/// Cổng UDP của slirp echo service (`10.0.2.2:7`) — dữ kiện nền tảng QEMU.
pub const UDP_ECHO_PORT: u16 = 7;
/// Cổng nguồn UDP cố định của client (chưa có RNG; còn kiểm địa chỉ/cổng đích nên đủ phân biệt).
pub const UDP_CLIENT_PORT: u16 = 0x4b54;
/// Payload UDP echo theo KT-260.
pub const UDP_ECHO_PAYLOAD: &[u8] = b"vifast-udp-echo";
/// Cỡ payload UDP tối đa của bước UDP echo (DNS không EDNS: 512 byte, RFC 1035 §4.2.1).
pub const UDP_MAX_PAYLOAD: usize = 512;

/// Kết quả một lần UDP echo.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UdpEcho {
    /// Số byte payload nhận lại (bằng payload đã gửi).
    pub len: usize,
    /// RTT (ms).
    pub rtt_ms: u64,
}

/// Gửi **một** datagram UDP (Ethernet + IPv4 + UDP) tới `dst_ip:dst_port`.
#[allow(clippy::too_many_arguments)] // hai đầu cuối (IP + MAC) + hai cổng + payload + link
pub fn send<L: Link>(
    link: &mut L,
    our_ip: Ipv4Addr,
    our_mac: MacAddr,
    dst_ip: Ipv4Addr,
    dst_mac: MacAddr,
    src_port: u16,
    dst_port: u16,
    payload: &[u8],
) -> Result<(), LinkError> {
    if payload.len() > UDP_MAX_PAYLOAD {
        return Err(LinkError::BadFrame);
    }
    udp_send(
        link, our_mac, dst_mac, our_ip, dst_ip, src_port, dst_port, payload,
    )
}

/// Chờ **một** datagram UDP khớp `src_ip:src_port` → `our_ip:dst_port` trong `timeout`; trả độ dài
/// payload và copy payload vào `out`.
///
/// Frame không khớp (ARP, ICMP, cổng khác, checksum sai) bị bỏ qua — cùng cách `dhcp_wait`/`ping`
/// đang làm. Hết hạn ⇒ [`LinkError::Timeout`] (fail-closed).
#[allow(clippy::too_many_arguments)] // địa chỉ/cổng hai đầu + đệm ra + timeout
pub fn recv<C: Clock, L: Link>(
    clock: &C,
    link: &mut L,
    our_ip: Ipv4Addr,
    src_ip: Ipv4Addr,
    src_port: u16,
    dst_port: u16,
    out: &mut [u8],
    timeout: u64,
) -> Result<usize, LinkError> {
    let deadline = clock.now().wrapping_add(timeout);
    udp_recv_until(
        clock, link, our_ip, src_ip, src_port, dst_port, out, deadline,
    )
}

/// Gửi `payload` tới `dst_ip:dst_port` và chờ **đúng** payload đó quay lại (echo), trong
/// `timeout` cho **cả bước**.
///
/// `Err(LinkError::Timeout)` nếu không có reply; `Err(LinkError::BadFrame)` nếu reply khác payload
/// (không nhận "gần đúng").
#[allow(clippy::too_many_arguments)] // hai đầu cuối (IP + MAC) + hai cổng + payload + timeout
pub fn echo<C: Clock, L: Link>(
    clock: &C,
    link: &mut L,
    our_ip: Ipv4Addr,
    our_mac: MacAddr,
    dst_ip: Ipv4Addr,
    dst_mac: MacAddr,
    src_port: u16,
    dst_port: u16,
    payload: &[u8],
    timeout: u64,
) -> Result<UdpEcho, LinkError> {
    let started = clock.now();
    send(
        link, our_ip, our_mac, dst_ip, dst_mac, src_port, dst_port, payload,
    )?;
    // Ngân sách `timeout` tính cho **cả bước** (gửi + chờ) — cùng lý do KT-268 bên riscv64.
    let spent = clock.now().wrapping_sub(started);
    let remaining = if past_deadline(spent, timeout) {
        0
    } else {
        timeout.wrapping_sub(spent)
    };
    let mut reply = [0u8; UDP_MAX_PAYLOAD];
    let len = recv(
        clock, link, our_ip, dst_ip, dst_port, src_port, &mut reply, remaining,
    )?;
    if &reply[..len] != payload {
        return Err(LinkError::BadFrame);
    }
    Ok(UdpEcho {
        len,
        rtt_ms: clock.now().wrapping_sub(started) / 1_000,
    })
}

/// Số byte payload UDP mà [`UdpHeader`] đọc được từ một frame (dùng cho chẩn đoán của ảnh).
pub fn payload_len(frame: &[u8]) -> Option<usize> {
    let eth = crate::frame::EthernetFrame::parse(frame).ok()?;
    let ip = crate::ipv4::Ipv4Header::parse(eth.payload, true).ok()?;
    let udp = UdpHeader::parse(ip.header.src, ip.header.dst, ip.payload(), true).ok()?;
    Some(udp.payload().len())
}
