// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Adapter TCP** của aarch64 — KT-335. Trước: 376 dòng chính sách **nhân bản** với
//! `vifast-plat-riscv64/src/tcp.rs` (khác **đúng** đồng hồ và chữ ký `send_frame`).
//!
//! Toàn bộ chính sách client (bắt tay 3 bước, gửi/nhận trong cửa sổ, retransmit có hạn, FIN/RST)
//! đã dời về [`vifast_net::tcp_client`] — crate thuần, host test 10 ca
//! (`v2/lib/vifast-net/tests/tcp_client.rs`). Tệp này chỉ cài hai thứ thuộc phần cứng:
//!
//! 1. [`Micro`] — [`Clock`] trên `CNTPCT_EL0 / CNTFRQ_EL0` ([`crate::virtio_net::now_us`], µs);
//! 2. [`Wire`] — [`Link`] trên `send_frame`/`recv_frame_wait` của [`crate::virtio_net`] (MMIO +
//!    DMA + cache maintenance nằm ở đó, D2-17/D2-239).
//!
//! Chữ ký hàm và hằng số **giữ nguyên** để `kbin` aarch64 không phải đổi một dòng nào; mã lỗi mà
//! TAP in ra cũng không đổi (`LinkError` → `NetError` là ánh xạ 1-1 theo tên).

use vifast_net::addr::{Ipv4Addr, MacAddr};
use vifast_net::tcp_client::{self, ClientConfig, Clock, Link, LinkError};
use vifast_virtio::net::NetError;

use crate::virtio_net::{
    now_us, recv_frame_wait, send_frame, NET_STEP_TIMEOUT_US, POLL_TIMEOUT_US,
};

/// Một kết nối TCP đang mở — **cùng** kiểu với hai nền tảng kia (không còn bản sao).
pub use vifast_net::tcp_client::TcpConn;

/// Cổng TCP phía khách (cố định — chưa có RNG; còn kiểm IP/cổng đích nên đủ phân biệt).
pub const TCP_CLIENT_PORT: u16 = 0x4b57;
/// Cổng server TCP trên host. Runner dựng `python3` lắng nghe `127.0.0.1:<port>`; khách nối
/// `10.0.2.2:<port>` — slirp ánh xạ `10.0.2.2` → loopback của host (`sotranslate_out4`).
pub const TCP_SERVER_PORT: u16 = 45678;
/// `ISS` tất định (chưa có RNG).
pub const TCP_ISS: u32 = 0x4b54_0001;
/// Cửa sổ nhận quảng bá (không window scaling).
pub const TCP_RCV_WINDOW: u16 = tcp_client::RCV_WINDOW;
/// Cỡ payload tối đa một segment TCP (MSS không option: 1500 − 20 IPv4 − 20 TCP).
pub const TCP_MAX_PAYLOAD: usize = tcp_client::MAX_PAYLOAD;
/// Khoảng retransmit trong **một** bước: 3 s / 4 ⇒ kịp **3** lần retransmit trước hạn 3 s.
pub const TCP_RETRANSMIT_US: u64 = NET_STEP_TIMEOUT_US / 4;

/// Đồng hồ driver: `CNTPCT_EL0 / CNTFRQ_EL0` (µs) — như bản cũ, không đổi đơn vị.
struct Micro;

impl Clock for Micro {
    fn now(&self) -> u64 {
        now_us()
    }
}

/// Đường frame: `send_frame`/`recv_frame_wait` của driver virtio-net aarch64.
struct Wire;

impl Link for Wire {
    fn send_frame(&mut self, frame: &[u8]) -> Result<(), LinkError> {
        send_frame(frame, POLL_TIMEOUT_US).map_err(from_net_error)
    }

    fn recv_frame(&mut self, out: &mut [u8], timeout: u64) -> Option<usize> {
        recv_frame_wait(out, timeout)
    }
}

/// `NetError` của driver → `LinkError` của crate thuần (ánh xạ 1-1 theo **tên**, D2-102).
const fn from_net_error(error: NetError) -> LinkError {
    match error {
        NetError::NotReady | NetError::NoDevice | NetError::BadFrame => LinkError::BadFrame,
        NetError::Timeout => LinkError::Timeout,
        NetError::Io | NetError::QueueFull => LinkError::Io,
    }
}

/// `LinkError` của crate thuần → `NetError` mà TAP đang in (giữ **nguyên** mã lỗi cũ).
const fn to_net_error(error: LinkError) -> NetError {
    match error {
        LinkError::BadFrame => NetError::BadFrame,
        LinkError::Timeout => NetError::Timeout,
        LinkError::Io => NetError::Io,
    }
}

/// Cấu hình kết nối cho một lần `tcp_connect`.
fn config(
    our_ip: Ipv4Addr,
    our_mac: MacAddr,
    dst_ip: Ipv4Addr,
    dst_mac: MacAddr,
    dst_port: u16,
) -> ClientConfig {
    ClientConfig::new(
        our_ip,
        our_mac,
        dst_ip,
        dst_mac,
        dst_port,
        TCP_CLIENT_PORT,
        TCP_ISS,
        TCP_RETRANSMIT_US,
    )
}

/// **Mở** kết nối TCP tới `dst_ip:dst_port`: gửi SYN, chờ SYN+ACK, ACK lại.
///
/// `Err(NetError::Timeout)` nếu không có SYN+ACK trong `timeout_us` (đã retransmit tối đa 3 lần);
/// `Err(NetError::Io)` nếu peer trả RST (từ chối kết nối).
pub fn tcp_connect(
    our_ip: Ipv4Addr,
    our_mac: MacAddr,
    dst_ip: Ipv4Addr,
    dst_mac: MacAddr,
    dst_port: u16,
    timeout_us: u64,
) -> Result<TcpConn, NetError> {
    let cfg = config(our_ip, our_mac, dst_ip, dst_mac, dst_port);
    tcp_client::connect(&Micro, &mut Wire, &cfg, timeout_us).map_err(to_net_error)
}

/// **Gửi** một segment dữ liệu và chờ ACK bao hết nó (retransmit tối đa 3 lần trong `timeout_us`).
pub fn tcp_send(conn: &mut TcpConn, payload: &[u8], timeout_us: u64) -> Result<usize, NetError> {
    tcp_client::send(&Micro, &mut Wire, conn, payload, timeout_us).map_err(to_net_error)
}

/// **Nhận** tối đa một segment dữ liệu trong `timeout_us`; trả số byte đã copy vào `out`.
///
/// Trả `Ok(0)` khi peer đã đóng (FIN) mà không còn dữ liệu; `Err(NetError::Timeout)` khi hết hạn.
pub fn tcp_recv(conn: &mut TcpConn, out: &mut [u8], timeout_us: u64) -> Result<usize, NetError> {
    tcp_client::recv(&Micro, &mut Wire, conn, out, timeout_us).map_err(to_net_error)
}

/// **Đóng** kết nối: gửi FIN và chờ ACK (retransmit tối đa 3 lần trong `timeout_us`).
pub fn tcp_close(conn: &mut TcpConn, timeout_us: u64) -> Result<(), NetError> {
    tcp_client::close(&Micro, &mut Wire, conn, timeout_us).map_err(to_net_error)
}
