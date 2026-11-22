// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Adapter TCP** của riscv64 — KT-335. Trước: 374 dòng chính sách **nhân bản** với
//! `vifast-plat-aarch64/src/tcp.rs` (chỉ khác đồng hồ).
//!
//! Toàn bộ chính sách client (bắt tay 3 bước, gửi/nhận trong cửa sổ, retransmit có hạn, FIN/RST)
//! đã dời về [`vifast_net::tcp_client`] — crate thuần, host test 10 ca
//! (`v2/lib/vifast-net/tests/tcp_client.rs`). Tệp này chỉ cài hai thứ thuộc phần cứng:
//!
//! 1. [`Tick`] — [`Clock`] trên timebase của `Riscv64Virt` (tick);
//! 2. [`Wire`] — [`Link`] trên `send_frame`/`recv_frame_wait` của [`crate::virtio_net`] (MMIO +
//!    DMA + `unsafe` nằm ở đó, D2-17).
//!
//! Chữ ký hàm và hằng số **giữ nguyên** để `ktest_net` của ảnh không phải đổi một dòng nào; mã lỗi
//! mà TAP in ra cũng không đổi (`LinkError` → `NetError` là ánh xạ 1-1 theo tên).

use vifast_net::addr::{Ipv4Addr, MacAddr};
use vifast_net::tcp_client::{self, ClientConfig, Clock, Link, LinkError};
use vifast_plat_common::Platform;
use vifast_virtio::net::NetError;

use crate::virtio_net::{recv_frame_wait, send_frame, NET_STEP_TIMEOUT_TICKS};
use crate::Riscv64Virt;

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
pub const TCP_RETRANSMIT_TICKS: u64 = NET_STEP_TIMEOUT_TICKS / 4;

/// Đồng hồ driver: timebase của `Riscv64Virt` (tick).
struct Tick;

impl Clock for Tick {
    fn now(&self) -> u64 {
        Riscv64Virt.now_ticks()
    }
}

/// Đường frame: `send_frame`/`recv_frame_wait` của driver virtio-net.
struct Wire;

impl Link for Wire {
    fn send_frame(&mut self, frame: &[u8]) -> Result<(), LinkError> {
        send_frame(frame).map_err(from_net_error)
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
        TCP_RETRANSMIT_TICKS,
    )
}

/// **Mở** kết nối TCP tới `dst_ip:dst_port`: gửi SYN, chờ SYN+ACK, ACK lại.
///
/// `Err(NetError::Timeout)` nếu không có SYN+ACK trong `timeout_ticks` (đã retransmit tối đa 3
/// lần); `Err(NetError::Io)` nếu peer trả RST (từ chối kết nối).
pub fn tcp_connect(
    our_ip: Ipv4Addr,
    our_mac: MacAddr,
    dst_ip: Ipv4Addr,
    dst_mac: MacAddr,
    dst_port: u16,
    timeout_ticks: u64,
) -> Result<TcpConn, NetError> {
    let cfg = config(our_ip, our_mac, dst_ip, dst_mac, dst_port);
    tcp_client::connect(&Tick, &mut Wire, &cfg, timeout_ticks).map_err(to_net_error)
}

/// **Gửi** một segment dữ liệu và chờ ACK bao hết nó (retransmit tối đa 3 lần trong `timeout_ticks`).
pub fn tcp_send(conn: &mut TcpConn, payload: &[u8], timeout_ticks: u64) -> Result<usize, NetError> {
    tcp_client::send(&Tick, &mut Wire, conn, payload, timeout_ticks).map_err(to_net_error)
}

/// **Nhận** tối đa một segment dữ liệu trong `timeout_ticks`; trả số byte đã copy vào `out`.
///
/// Trả `Ok(0)` khi peer đã đóng (FIN) mà không còn dữ liệu; `Err(NetError::Timeout)` khi hết hạn.
pub fn tcp_recv(conn: &mut TcpConn, out: &mut [u8], timeout_ticks: u64) -> Result<usize, NetError> {
    tcp_client::recv(&Tick, &mut Wire, conn, out, timeout_ticks).map_err(to_net_error)
}

/// **Đóng** kết nối: gửi FIN và chờ ACK (retransmit tối đa 3 lần trong `timeout_ticks`).
pub fn tcp_close(conn: &mut TcpConn, timeout_ticks: u64) -> Result<(), NetError> {
    tcp_client::close(&Tick, &mut Wire, conn, timeout_ticks).map_err(to_net_error)
}
