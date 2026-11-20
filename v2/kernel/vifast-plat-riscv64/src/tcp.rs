// SPDX-License-Identifier: MIT OR Apache-2.0
//! **TCP client** trên virtio-net (KT-268, N1 phần 2) — phần **chính sách**: dựng frame
//! Ethernet/IPv4/TCP, gửi, chờ segment khớp kết nối, thi hành `Action` của máy trạng thái, và
//! retransmit có hạn.
//!
//! Máy trạng thái TCP **thuần** nằm ở `vifast_net::tcp` (host test 43 ca, gồm ca âm); tệp này
//! không quyết định seq/ack/flags — chỉ hỏi máy trạng thái rồi ghi ra dây.
//!
//! # Bất biến
//!
//! - **Timeout 3 s mỗi bước** ([`NET_STEP_TIMEOUT_TICKS`]); retransmit tối đa
//!   [`vifast_net::tcp::MAX_RETRANSMITS`] = 3 lần, cách nhau [`TCP_RETRANSMIT_TICKS`] = 3 s / 4
//!   ⇒ cả bước vẫn gói trong 3 s (fail-closed, không treo).
//! - Segment không khớp (ARP/ICMP/UDP, IP/cổng khác, checksum sai, máy trạng thái từ chối) bị **bỏ
//!   qua** — không đổi trạng thái, không PANIC.
//! - `Action::Reset` ⇒ `Err(NetError::Io)`; `Action::Close` ⇒ kết nối đã ở trạng thái cuối.
//! - Không có RNG: ISS/cổng nguồn là hằng số; vẫn kiểm IP/cổng đích + checksum nên đủ phân biệt.
//! - **Server thật trên host**: khách nối `10.0.2.2:<port>`; slirp ánh xạ `10.0.2.2` →
//!   `127.0.0.1` của host (`sotranslate_out4`), nên runner chỉ cần dựng server trên loopback host.
//!   (`hostfwd` của QEMU là chiều host → guest — không dùng cho chiều này.)

use vifast_net::addr::{Ipv4Addr, MacAddr};
use vifast_net::eth::{EthHeader, ETHERTYPE_IPV4, ETH_HEADER_LEN};
use vifast_net::frame::EthernetFrame;
use vifast_net::ipv4::{Ipv4Header, IPV4_MIN_HEADER_LEN, PROTO_TCP};
use vifast_net::tcp::{
    Action, SendKind, State as TcpState, TcpConnection, TcpHeader, TCP_HEADER_LEN,
};
use vifast_plat_common::Platform;
use vifast_virtio::net::{NetError, MAX_FRAME_BYTES};

use crate::virtio_net::{past_deadline, recv_frame_wait, send_frame, NET_STEP_TIMEOUT_TICKS};
use crate::Riscv64Virt;

/// Cổng TCP phía khách (cố định — chưa có RNG; còn kiểm IP/cổng đích nên đủ phân biệt).
pub const TCP_CLIENT_PORT: u16 = 0x4b57;
/// Cổng server TCP trên host. Runner dựng `python3` lắng nghe `127.0.0.1:<port>`; khách nối
/// `10.0.2.2:<port>` — slirp ánh xạ `10.0.2.2` → loopback của host (`sotranslate_out4`).
pub const TCP_SERVER_PORT: u16 = 45678;
/// ISS tất định (chưa có RNG).
pub const TCP_ISS: u32 = 0x4b54_0001;
/// Cửa sổ nhận quảng bá (không window scaling).
pub const TCP_RCV_WINDOW: u16 = 4096;
/// Cỡ payload tối đa một segment TCP (MSS không option: 1500 − 20 IPv4 − 20 TCP).
pub const TCP_MAX_PAYLOAD: usize = 1460;
/// Khoảng retransmit trong **một** bước: 3 s / 4 ⇒ kịp **3** lần retransmit trước hạn 3 s.
pub const TCP_RETRANSMIT_TICKS: u64 = NET_STEP_TIMEOUT_TICKS / 4;

/// Một kết nối TCP đang mở: máy trạng thái + bốn đầu địa chỉ/MAC cần để dựng frame.
pub struct TcpConn {
    conn: TcpConnection,
    our_ip: Ipv4Addr,
    our_mac: MacAddr,
    dst_ip: Ipv4Addr,
    dst_mac: MacAddr,
}

impl TcpConn {
    /// Trạng thái máy trạng thái.
    pub fn state(&self) -> TcpState {
        self.conn.state()
    }

    /// Số lần retransmit đã dùng trong bước hiện tại.
    pub fn retries(&self) -> u8 {
        self.conn.retries()
    }

    /// Số thứ tự gửi chưa được ACK.
    pub fn snd_una(&self) -> u32 {
        self.conn.snd_una()
    }

    /// Số thứ tự nhận kế tiếp.
    pub fn rcv_nxt(&self) -> u32 {
        self.conn.rcv_nxt()
    }

    /// ISS của ta.
    pub fn iss(&self) -> u32 {
        self.conn.iss()
    }

    /// `true` nếu FIN của ta đã được ACK.
    pub fn fin_acked(&self) -> bool {
        self.conn.fin_acked()
    }
}

/// `true` ⇔ `a` **không** đứng trước `b` trong không gian số thứ tự (so sánh có wrap).
fn seq_ge(a: u32, b: u32) -> bool {
    a.wrapping_sub(b) < u32::MAX / 2
}

/// Dựng và gửi **một** segment TCP (Ethernet + IPv4 + TCP) với `seq`/cờ cho trước.
fn tcp_send_segment(conn: &TcpConn, seq: u32, flags: u8, payload: &[u8]) -> Result<(), NetError> {
    if payload.len() > TCP_MAX_PAYLOAD {
        return Err(NetError::BadFrame);
    }
    let mut tcp = [0u8; TCP_HEADER_LEN + TCP_MAX_PAYLOAD];
    let header = TcpHeader::new(
        conn.conn.local().port,
        conn.conn.remote().port,
        seq,
        conn.conn.rcv_nxt(),
        flags,
        conn.conn.rcv_wnd(),
    );
    let tcp_len = header
        .encode(conn.our_ip, conn.dst_ip, payload, &mut tcp)
        .map_err(|_| NetError::BadFrame)?;
    let mut ip = [0u8; IPV4_MIN_HEADER_LEN + TCP_HEADER_LEN + TCP_MAX_PAYLOAD];
    let ip_header_len = Ipv4Header::new(PROTO_TCP, conn.our_ip, conn.dst_ip, tcp_len)
        .encode(&mut ip)
        .map_err(|_| NetError::BadFrame)?;
    ip[ip_header_len..ip_header_len + tcp_len].copy_from_slice(&tcp[..tcp_len]);
    let ip_len = ip_header_len + tcp_len;
    let mut frame = [0u8; ETH_HEADER_LEN + IPV4_MIN_HEADER_LEN + TCP_HEADER_LEN + TCP_MAX_PAYLOAD];
    let len = EthernetFrame {
        header: EthHeader::new(conn.dst_mac, conn.our_mac, ETHERTYPE_IPV4),
        payload: &ip[..ip_len],
    }
    .encode(&mut frame)
    .map_err(|_| NetError::BadFrame)?;
    send_frame(&frame[..len])
}

/// Gửi segment theo `kind` với số thứ tự máy trạng thái đã cấp (SYN = ISS, FIN = `fin_seq`,
/// còn lại = `snd_nxt`).
fn tcp_emit(conn: &TcpConn, kind: SendKind, payload: &[u8]) -> Result<(), NetError> {
    let seq = match kind {
        SendKind::Syn => conn.conn.iss(),
        SendKind::Fin => conn.conn.fin_seq(),
        SendKind::Ack | SendKind::Data | SendKind::Rst => conn.conn.snd_nxt(),
    };
    tcp_send_segment(conn, seq, kind.flags(), payload)
}

/// Kết quả **một** segment đã xử lý.
#[derive(Clone, Copy, Debug, Default)]
struct TcpStep {
    /// Số byte đã giao cho tầng trên (0 nếu segment không mang dữ liệu mới).
    delivered: usize,
    /// Máy trạng thái báo kết nối đã ở trạng thái cuối.
    closed: bool,
    /// Máy trạng thái báo huỷ (RST).
    reset: bool,
}

/// Chờ **một** segment TCP khớp kết nối tới `deadline`, thi hành hành động của máy trạng thái.
///
/// Segment không khớp (ethertype/protocol/IP/cổng/checksum) hoặc bị máy trạng thái từ chối bị bỏ
/// qua — hàm chỉ trả khi có segment **khớp** hoặc hết hạn ([`NetError::Timeout`]).
fn tcp_step_until(conn: &mut TcpConn, deadline: u64, out: &mut [u8]) -> Result<TcpStep, NetError> {
    let mut frame = [0u8; MAX_FRAME_BYTES];
    loop {
        let now = Riscv64Virt.now_ticks();
        if past_deadline(now, deadline) {
            return Err(NetError::Timeout);
        }
        let remaining = deadline.wrapping_sub(now);
        let Some(got) = recv_frame_wait(&mut frame, remaining) else {
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
        if ip.header.protocol != PROTO_TCP
            || ip.header.src != conn.dst_ip
            || ip.header.dst != conn.our_ip
        {
            continue;
        }
        let Ok(seg) = TcpHeader::parse(ip.header.src, ip.header.dst, ip.payload(), true) else {
            continue;
        };
        if seg.header.src_port != conn.conn.remote().port
            || seg.header.dst_port != conn.conn.local().port
        {
            continue;
        }
        let Ok(actions) = conn.conn.on_segment(&seg) else {
            continue;
        };
        let mut step = TcpStep::default();
        for action in actions.iter() {
            match action {
                Action::Send(kind) => tcp_emit(conn, kind, &[])?,
                Action::Deliver(data) => {
                    if data.len() > out.len() {
                        return Err(NetError::BadFrame);
                    }
                    out[..data.len()].copy_from_slice(data);
                    step.delivered = data.len();
                }
                Action::Close => step.closed = true,
                Action::Reset => step.reset = true,
            }
        }
        return Ok(step);
    }
}

/// Mốc retransmit kế tiếp, không vượt `deadline`.
fn next_retransmit(now: u64, deadline: u64) -> u64 {
    let candidate = now.wrapping_add(TCP_RETRANSMIT_TICKS);
    if past_deadline(candidate, deadline) {
        deadline
    } else {
        candidate
    }
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
    let mut conn = TcpConn {
        conn: TcpConnection::connect(our_ip, TCP_CLIENT_PORT, dst_ip, dst_port, TCP_ISS),
        our_ip,
        our_mac,
        dst_ip,
        dst_mac,
    };
    conn.conn.set_rcv_window(TCP_RCV_WINDOW);
    tcp_emit(&conn, SendKind::Syn, &[])?;
    let deadline = Riscv64Virt.now_ticks().wrapping_add(timeout_ticks);
    let mut next_retx = next_retransmit(Riscv64Virt.now_ticks(), deadline);
    let mut out = [0u8; 1];
    loop {
        match tcp_step_until(&mut conn, next_retx, &mut out) {
            Ok(step) => {
                if step.reset {
                    return Err(NetError::Io);
                }
                if conn.conn.state() == TcpState::Established {
                    return Ok(conn);
                }
            }
            Err(NetError::Timeout) => {
                let now = Riscv64Virt.now_ticks();
                if past_deadline(now, deadline) {
                    return Err(NetError::Timeout);
                }
                match conn.conn.on_timeout() {
                    Action::Send(SendKind::Syn) => tcp_emit(&conn, SendKind::Syn, &[])?,
                    _ => return Err(NetError::Timeout),
                }
                next_retx = next_retransmit(now, deadline);
            }
            Err(error) => return Err(error),
        }
    }
}

/// **Gửi** một segment dữ liệu và chờ ACK bao hết nó (retransmit tối đa 3 lần trong `timeout_ticks`).
pub fn tcp_send(conn: &mut TcpConn, payload: &[u8], timeout_ticks: u64) -> Result<usize, NetError> {
    let seq = conn
        .conn
        .build_data(payload.len())
        .map_err(|_| NetError::BadFrame)?;
    let target = seq.wrapping_add(payload.len() as u32);
    tcp_send_segment(conn, seq, SendKind::Data.flags(), payload)?;
    let deadline = Riscv64Virt.now_ticks().wrapping_add(timeout_ticks);
    let mut next_retx = next_retransmit(Riscv64Virt.now_ticks(), deadline);
    let mut out = [0u8; 1];
    loop {
        if seq_ge(conn.conn.snd_una(), target) {
            return Ok(payload.len());
        }
        match tcp_step_until(conn, next_retx, &mut out) {
            Ok(step) => {
                if step.reset || step.closed {
                    return Err(NetError::Io);
                }
            }
            Err(NetError::Timeout) => {
                let now = Riscv64Virt.now_ticks();
                if past_deadline(now, deadline) {
                    return Err(NetError::Timeout);
                }
                match conn.conn.on_timeout() {
                    Action::Send(SendKind::Data) => {
                        tcp_send_segment(conn, seq, SendKind::Data.flags(), payload)?;
                    }
                    _ => return Err(NetError::Timeout),
                }
                next_retx = next_retransmit(now, deadline);
            }
            Err(error) => return Err(error),
        }
    }
}

/// **Nhận** tối đa một segment dữ liệu trong `timeout_ticks`; trả số byte đã copy vào `out`.
///
/// Trả `Ok(0)` khi peer đã đóng (FIN) mà không còn dữ liệu; `Err(NetError::Timeout)` khi hết hạn.
pub fn tcp_recv(conn: &mut TcpConn, out: &mut [u8], timeout_ticks: u64) -> Result<usize, NetError> {
    let deadline = Riscv64Virt.now_ticks().wrapping_add(timeout_ticks);
    let mut buf = [0u8; TCP_MAX_PAYLOAD];
    loop {
        let step = tcp_step_until(conn, deadline, &mut buf)?;
        if step.reset {
            return Err(NetError::Io);
        }
        if step.delivered > 0 {
            if step.delivered > out.len() {
                return Err(NetError::BadFrame);
            }
            out[..step.delivered].copy_from_slice(&buf[..step.delivered]);
            return Ok(step.delivered);
        }
        if step.closed {
            return Ok(0);
        }
    }
}

/// **Đóng** kết nối: gửi FIN và chờ ACK (retransmit tối đa 3 lần trong `timeout_ticks`).
///
/// Kết nối đã đóng/đang đóng (không còn gì để gửi) ⇒ `Ok(())` — không phải lỗi.
pub fn tcp_close(conn: &mut TcpConn, timeout_ticks: u64) -> Result<(), NetError> {
    let seq = match conn.conn.build_fin() {
        Ok(seq) => seq,
        Err(_) => return Ok(()),
    };
    tcp_send_segment(conn, seq, SendKind::Fin.flags(), &[])?;
    let deadline = Riscv64Virt.now_ticks().wrapping_add(timeout_ticks);
    let mut next_retx = next_retransmit(Riscv64Virt.now_ticks(), deadline);
    let mut out = [0u8; 1];
    loop {
        if conn.conn.fin_acked() {
            return Ok(());
        }
        match tcp_step_until(conn, next_retx, &mut out) {
            Ok(step) => {
                if step.reset {
                    return Err(NetError::Io);
                }
                if conn.conn.fin_acked() {
                    return Ok(());
                }
            }
            Err(NetError::Timeout) => {
                let now = Riscv64Virt.now_ticks();
                if past_deadline(now, deadline) {
                    return Err(NetError::Timeout);
                }
                match conn.conn.on_timeout() {
                    Action::Send(SendKind::Fin) => {
                        tcp_send_segment(conn, seq, SendKind::Fin.flags(), &[])?;
                    }
                    _ => return Err(NetError::Timeout),
                }
                next_retx = next_retransmit(now, deadline);
            }
            Err(error) => return Err(error),
        }
    }
}
