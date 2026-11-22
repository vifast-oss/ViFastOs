// SPDX-License-Identifier: MIT OR Apache-2.0
//! TCP **client** — chính sách mở/gửi/nhận/đóng trên một đường frame trừu tượng.
//!
//! # Vì sao tệp này tồn tại (KT-335, D2-231)
//!
//! Trước KT-335, chính sách client nằm **hai bản** trong `vifast-plat-riscv64/src/tcp.rs`
//! (KT-268) và `vifast-plat-aarch64/src/tcp.rs` (KT-289): 374 + 376 dòng, khác nhau **đúng**
//! ba thứ thuộc về nền tảng — đồng hồ (timebase tick vs `CNTPCT_EL0`), tên hằng thời gian
//! (`*_TICKS` vs `*_US`), và chữ ký `send_frame` (có/không tham số timeout). Toàn bộ **quyết
//! định** giao thức (segment nào, `seq`/`ack`/cờ nào, retransmit khi nào, bỏ frame nào) là **một**
//! đoạn mã, nên nó thuộc về crate thuần — nơi host test chạy được (D2-05) và nơi driver chỉ còn
//! phần chạm thanh ghi (D2-17).
//!
//! Máy trạng thái **thuần** (RFC 793) vẫn ở [`crate::tcp`]; tệp này **không** quyết định
//! `seq`/`ack`/flags — nó hỏi máy trạng thái rồi ghi ra dây, đúng như hai bản cũ.
//!
//! # Hai trait biên
//!
//! ```text
//! Clock ── now() ────────────────────────────────┐
//! Link  ── send_frame(frame) / recv_frame(..) ───┤─► connect / send / recv / close
//! ```
//!
//! - [`Clock`]: đồng hồ **đơn điệu**, đơn vị tuỳ ý (tick timebase, µs `CNTPCT`, hay bộ đếm giả
//!   của test). Mọi so sánh hạn dùng [`past_deadline`] nên **an toàn với wrap** của `rdtime`.
//! - [`Link`]: gửi/nhận **một** frame Ethernet. `recv_frame` trả `None` khi hết hạn — đúng hợp
//!   đồng `recv_frame_wait` của driver; `timeout = 0` là "hỏi không chờ".
//!
//! Nhờ vậy test host cài `Link` bằng **server TCP giả trên byte thật**
//! (`tests/tcp_client.rs`), còn driver cài bằng `send_frame`/`recv_frame_wait` của virtio-net
//! (adapter `vifast-plat-*/src/tcp.rs` chỉ còn ~60 dòng).
//!
//! # Bất biến giữ nguyên từ KT-268/KT-289
//!
//! - **Mỗi bước có trần**: `timeout` của lần gọi; retransmit tối đa
//!   [`crate::tcp::MAX_RETRANSMITS`] = 3 lần, cách nhau `cfg.retransmit` (driver truyền
//!   `bước / 4` ⇒ ba lần vẫn gói trong một bước). Không có retransmit vô hạn.
//! - **Fail-closed**: RST ⇒ [`LinkError::Io`]; hết hạn ⇒ [`LinkError::Timeout`]; frame sai độ dài
//!   ⇒ [`LinkError::BadFrame`]. Không PANIC, không treo, không đoán.
//! - **Bỏ qua frame không khớp**: ARP/ICMP/UDP, IP/cổng khác, checksum sai, segment bị máy trạng
//!   thái từ chối — bỏ **im lặng** rồi chờ tiếp (không đổi trạng thái).
//! - `#![no_std]`, `#![deny(unsafe_code)]`, **không** cấp phát: mọi bộ đệm là mảng trên stack.

use crate::addr::{Ipv4Addr, MacAddr};
use crate::eth::{EthHeader, ETHERTYPE_IPV4, ETH_HEADER_LEN};
use crate::frame::EthernetFrame;
use crate::ipv4::{Ipv4Header, IPV4_MIN_HEADER_LEN, PROTO_TCP};
use crate::tcp::{Action, SendKind, State as TcpState, TcpConnection, TcpHeader, TCP_HEADER_LEN};

/// MSS mặc định khi không đàm phán option: 1500 − 20 IPv4 − 20 TCP.
pub const MAX_PAYLOAD: usize = 1460;
/// Cửa sổ nhận quảng bá mặc định (không window scaling).
pub const RCV_WINDOW: u16 = 4096;
/// Cỡ frame Ethernet lớn nhất trên dây (14 + 1500) — cùng giá trị `MAX_FRAME_BYTES` của
/// `vifast-virtio`; crate này **không** phụ thuộc crate kia nên hằng được khai lại ở đây.
pub const MAX_FRAME_BYTES: usize = 1514;

/// Lỗi của tầng chính sách.
///
/// Ba tên **khớp** `vifast_virtio::net::NetError::{BadFrame, Timeout, Io}` để adapter của mỗi
/// nền tảng ánh xạ 1-1 (không đổi mã lỗi mà TAP in ra).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkError {
    /// Frame không dựng/không parse được, hoặc yêu cầu vượt MSS/cửa sổ.
    BadFrame,
    /// Hết hạn chờ (không có segment khớp trong ngân sách).
    Timeout,
    /// Peer huỷ kết nối (RST) hoặc kết nối đã đóng khi đang gửi.
    Io,
}

impl LinkError {
    /// Tên để in evidence (D2-102: không in số trần).
    pub const fn name(self) -> &'static str {
        match self {
            Self::BadFrame => "BadFrame",
            Self::Timeout => "Timeout",
            Self::Io => "Io",
        }
    }
}

/// Đồng hồ đơn điệu — driver cài bằng đồng hồ thật, test cài bằng bộ đếm giả.
pub trait Clock {
    /// Thời điểm hiện tại, cùng đơn vị với mọi mốc hạn truyền vào crate này.
    fn now(&self) -> u64;
}

/// Đường frame: gửi **một** frame Ethernet, nhận **một** frame trong hạn.
pub trait Link {
    /// Gửi một frame Ethernet hoàn chỉnh (không kể header virtio).
    fn send_frame(&mut self, frame: &[u8]) -> Result<(), LinkError>;

    /// Nhận một frame, chờ tối đa `timeout` (cùng đơn vị [`Clock`]); `None` = hết hạn.
    fn recv_frame(&mut self, out: &mut [u8], timeout: u64) -> Option<usize>;
}

/// Tham số một kết nối client — gói lại để chữ ký hàm không phải mười tham số.
#[derive(Clone, Copy, Debug)]
pub struct ClientConfig {
    /// IP của ta (DHCP cấp).
    pub our_ip: Ipv4Addr,
    /// MAC của ta (thiết bị cấp).
    pub our_mac: MacAddr,
    /// IP peer.
    pub dst_ip: Ipv4Addr,
    /// MAC peer (đã resolve bằng ARP).
    pub dst_mac: MacAddr,
    /// Cổng peer.
    pub dst_port: u16,
    /// Cổng nguồn của ta (chưa có RNG ⇒ hằng số; còn kiểm IP/cổng đích nên đủ phân biệt).
    pub client_port: u16,
    /// `ISS` (chưa có RNG ⇒ hằng số tất định).
    pub iss: u32,
    /// Cửa sổ nhận quảng bá.
    pub rcv_window: u16,
    /// MSS dùng cho một segment dữ liệu.
    pub max_payload: usize,
    /// Khoảng retransmit, **cùng đơn vị** với [`Clock`].
    pub retransmit: u64,
}

impl ClientConfig {
    /// Cấu hình mặc định từ bốn đầu địa chỉ + `ISS` (các trường còn lại lấy hằng của crate).
    // 8 tham số: bốn đầu địa chỉ/MAC + cổng đích + cổng nguồn + `ISS` + khoảng retransmit. Gói
    // thành struct sẽ làm call-site khó đọc hơn ở adapter ba arch; cùng khuôn `#[allow]` đã dùng
    // ở `udp_echo`/`dns_query` của driver.
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        our_ip: Ipv4Addr,
        our_mac: MacAddr,
        dst_ip: Ipv4Addr,
        dst_mac: MacAddr,
        dst_port: u16,
        client_port: u16,
        iss: u32,
        retransmit: u64,
    ) -> Self {
        Self {
            our_ip,
            our_mac,
            dst_ip,
            dst_mac,
            dst_port,
            client_port,
            iss,
            rcv_window: RCV_WINDOW,
            max_payload: MAX_PAYLOAD,
            retransmit,
        }
    }
}

/// Một kết nối TCP đang mở: máy trạng thái + cấu hình cần để dựng frame.
#[derive(Clone, Copy, Debug)]
pub struct TcpConn {
    conn: TcpConnection,
    cfg: ClientConfig,
}

impl TcpConn {
    /// Trạng thái máy trạng thái.
    pub const fn state(&self) -> TcpState {
        self.conn.state()
    }

    /// Số lần retransmit đã dùng trong bước hiện tại.
    pub const fn retries(&self) -> u8 {
        self.conn.retries()
    }

    /// Số thứ tự gửi chưa được ACK.
    pub const fn snd_una(&self) -> u32 {
        self.conn.snd_una()
    }

    /// Số thứ tự nhận kế tiếp.
    pub const fn rcv_nxt(&self) -> u32 {
        self.conn.rcv_nxt()
    }

    /// `ISS` của ta.
    pub const fn iss(&self) -> u32 {
        self.conn.iss()
    }

    /// `true` nếu FIN của ta đã được ACK.
    pub const fn fin_acked(&self) -> bool {
        self.conn.fin_acked()
    }

    /// Cấu hình của kết nối (địa chỉ/cổng) — dùng khi cần dựng frame ngoài crate.
    pub const fn config(&self) -> ClientConfig {
        self.cfg
    }
}

/// `true` ⇔ `now` **đã** qua `deadline`; an toàn với wrap của bộ đếm (kt334/`rdtime`).
pub const fn past_deadline(now: u64, deadline: u64) -> bool {
    now.wrapping_sub(deadline) < u64::MAX / 2
}

/// `true` ⇔ `a` **không** đứng trước `b` trong không gian số thứ tự (so sánh có wrap).
const fn seq_ge(a: u32, b: u32) -> bool {
    a.wrapping_sub(b) < u32::MAX / 2
}

/// Dựng và gửi **một** segment TCP (Ethernet + IPv4 + TCP) với `seq`/cờ cho trước.
fn send_segment<L: Link>(
    link: &mut L,
    conn: &TcpConn,
    seq: u32,
    flags: u8,
    payload: &[u8],
) -> Result<(), LinkError> {
    if payload.len() > conn.cfg.max_payload {
        return Err(LinkError::BadFrame);
    }
    let mut tcp = [0u8; TCP_HEADER_LEN + MAX_PAYLOAD];
    let header = TcpHeader::new(
        conn.conn.local().port,
        conn.conn.remote().port,
        seq,
        conn.conn.rcv_nxt(),
        flags,
        conn.conn.rcv_wnd(),
    );
    let tcp_len = header
        .encode(conn.cfg.our_ip, conn.cfg.dst_ip, payload, &mut tcp)
        .map_err(|_| LinkError::BadFrame)?;
    let mut ip = [0u8; IPV4_MIN_HEADER_LEN + TCP_HEADER_LEN + MAX_PAYLOAD];
    let ip_header_len = Ipv4Header::new(PROTO_TCP, conn.cfg.our_ip, conn.cfg.dst_ip, tcp_len)
        .encode(&mut ip)
        .map_err(|_| LinkError::BadFrame)?;
    ip[ip_header_len..ip_header_len + tcp_len].copy_from_slice(&tcp[..tcp_len]);
    let ip_len = ip_header_len + tcp_len;
    let mut frame = [0u8; ETH_HEADER_LEN + IPV4_MIN_HEADER_LEN + TCP_HEADER_LEN + MAX_PAYLOAD];
    let len = EthernetFrame {
        header: EthHeader::new(conn.cfg.dst_mac, conn.cfg.our_mac, ETHERTYPE_IPV4),
        payload: &ip[..ip_len],
    }
    .encode(&mut frame)
    .map_err(|_| LinkError::BadFrame)?;
    link.send_frame(&frame[..len])
}

/// Gửi segment theo `kind` với số thứ tự máy trạng thái đã cấp (SYN = `iss`, FIN = `fin_seq`,
/// còn lại = `snd_nxt`).
fn emit<L: Link>(
    link: &mut L,
    conn: &TcpConn,
    kind: SendKind,
    payload: &[u8],
) -> Result<(), LinkError> {
    let seq = match kind {
        SendKind::Syn => conn.conn.iss(),
        SendKind::Fin => conn.conn.fin_seq(),
        SendKind::Ack | SendKind::Data | SendKind::Rst => conn.conn.snd_nxt(),
    };
    send_segment(link, conn, seq, kind.flags(), payload)
}

/// Kết quả **một** segment đã xử lý.
#[derive(Clone, Copy, Debug, Default)]
struct Step {
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
/// qua — hàm chỉ trả khi có segment **khớp** hoặc hết hạn ([`LinkError::Timeout`]).
fn step_until<C: Clock, L: Link>(
    clock: &C,
    link: &mut L,
    conn: &mut TcpConn,
    deadline: u64,
    out: &mut [u8],
) -> Result<Step, LinkError> {
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
        if ip.header.protocol != PROTO_TCP
            || ip.header.src != conn.cfg.dst_ip
            || ip.header.dst != conn.cfg.our_ip
        {
            continue;
        }
        // `true` = **kiểm** checksum. Đột biến KT-335 "bỏ kiểm checksum" đổi đúng tham số này
        // sang `false` và phải làm `tests/tcp_client.rs::bad_checksum_is_ignored` ĐỎ.
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
        let mut step = Step::default();
        for action in actions.iter() {
            match action {
                Action::Send(kind) => emit(link, conn, kind, &[])?,
                Action::Deliver(data) => {
                    if data.len() > out.len() {
                        return Err(LinkError::BadFrame);
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
fn next_retransmit(now: u64, deadline: u64, interval: u64) -> u64 {
    let candidate = now.wrapping_add(interval);
    if past_deadline(candidate, deadline) {
        deadline
    } else {
        candidate
    }
}

/// **Mở** kết nối TCP tới `cfg.dst_ip:cfg.dst_port`: gửi SYN, chờ SYN+ACK, ACK lại.
///
/// `Err(LinkError::Timeout)` nếu không có SYN+ACK trong `timeout` (đã retransmit tối đa 3 lần);
/// `Err(LinkError::Io)` nếu peer trả RST (từ chối kết nối).
pub fn connect<C: Clock, L: Link>(
    clock: &C,
    link: &mut L,
    cfg: &ClientConfig,
    timeout: u64,
) -> Result<TcpConn, LinkError> {
    let mut conn = TcpConn {
        conn: TcpConnection::connect(
            cfg.our_ip,
            cfg.client_port,
            cfg.dst_ip,
            cfg.dst_port,
            cfg.iss,
        ),
        cfg: *cfg,
    };
    conn.conn.set_rcv_window(cfg.rcv_window);
    emit(link, &conn, SendKind::Syn, &[])?;
    let deadline = clock.now().wrapping_add(timeout);
    let mut next_retx = next_retransmit(clock.now(), deadline, cfg.retransmit);
    let mut out = [0u8; 1];
    loop {
        match step_until(clock, link, &mut conn, next_retx, &mut out) {
            Ok(step) => {
                if step.reset {
                    return Err(LinkError::Io);
                }
                if conn.conn.state() == TcpState::Established {
                    return Ok(conn);
                }
            }
            Err(LinkError::Timeout) => {
                let now = clock.now();
                if past_deadline(now, deadline) {
                    return Err(LinkError::Timeout);
                }
                match conn.conn.on_timeout() {
                    Action::Send(SendKind::Syn) => emit(link, &conn, SendKind::Syn, &[])?,
                    _ => return Err(LinkError::Timeout),
                }
                next_retx = next_retransmit(now, deadline, cfg.retransmit);
            }
            Err(error) => return Err(error),
        }
    }
}

/// **Gửi** một segment dữ liệu và chờ ACK bao hết nó (retransmit tối đa 3 lần trong `timeout`).
pub fn send<C: Clock, L: Link>(
    clock: &C,
    link: &mut L,
    conn: &mut TcpConn,
    payload: &[u8],
    timeout: u64,
) -> Result<usize, LinkError> {
    let seq = conn
        .conn
        .build_data(payload.len())
        .map_err(|_| LinkError::BadFrame)?;
    let target = seq.wrapping_add(payload.len() as u32);
    send_segment(link, conn, seq, SendKind::Data.flags(), payload)?;
    let deadline = clock.now().wrapping_add(timeout);
    let mut next_retx = next_retransmit(clock.now(), deadline, conn.cfg.retransmit);
    let mut out = [0u8; 1];
    loop {
        if seq_ge(conn.conn.snd_una(), target) {
            return Ok(payload.len());
        }
        match step_until(clock, link, conn, next_retx, &mut out) {
            Ok(step) => {
                if step.reset || step.closed {
                    return Err(LinkError::Io);
                }
            }
            Err(LinkError::Timeout) => {
                let now = clock.now();
                if past_deadline(now, deadline) {
                    return Err(LinkError::Timeout);
                }
                match conn.conn.on_timeout() {
                    Action::Send(SendKind::Data) => {
                        send_segment(link, conn, seq, SendKind::Data.flags(), payload)?;
                    }
                    _ => return Err(LinkError::Timeout),
                }
                next_retx = next_retransmit(now, deadline, conn.cfg.retransmit);
            }
            Err(error) => return Err(error),
        }
    }
}

/// **Nhận** tối đa một segment dữ liệu trong `timeout`; trả số byte đã copy vào `out`.
///
/// Trả `Ok(0)` khi peer đã đóng (FIN) mà không còn dữ liệu; `Err(LinkError::Timeout)` khi hết hạn.
pub fn recv<C: Clock, L: Link>(
    clock: &C,
    link: &mut L,
    conn: &mut TcpConn,
    out: &mut [u8],
    timeout: u64,
) -> Result<usize, LinkError> {
    let deadline = clock.now().wrapping_add(timeout);
    let mut buf = [0u8; MAX_PAYLOAD];
    loop {
        let step = step_until(clock, link, conn, deadline, &mut buf)?;
        if step.reset {
            return Err(LinkError::Io);
        }
        if step.delivered > 0 {
            if step.delivered > out.len() {
                return Err(LinkError::BadFrame);
            }
            out[..step.delivered].copy_from_slice(&buf[..step.delivered]);
            return Ok(step.delivered);
        }
        if step.closed {
            return Ok(0);
        }
    }
}

/// **Đóng** kết nối: gửi FIN và chờ ACK (retransmit tối đa 3 lần trong `timeout`).
///
/// Kết nối đã đóng/đang đóng (không còn gì để gửi) ⇒ `Ok(())` — không phải lỗi.
pub fn close<C: Clock, L: Link>(
    clock: &C,
    link: &mut L,
    conn: &mut TcpConn,
    timeout: u64,
) -> Result<(), LinkError> {
    let seq = match conn.conn.build_fin() {
        Ok(seq) => seq,
        Err(_) => return Ok(()),
    };
    send_segment(link, conn, seq, SendKind::Fin.flags(), &[])?;
    let deadline = clock.now().wrapping_add(timeout);
    let mut next_retx = next_retransmit(clock.now(), deadline, conn.cfg.retransmit);
    let mut out = [0u8; 1];
    loop {
        if conn.conn.fin_acked() {
            return Ok(());
        }
        match step_until(clock, link, conn, next_retx, &mut out) {
            Ok(step) => {
                if step.reset {
                    return Err(LinkError::Io);
                }
                if conn.conn.fin_acked() {
                    return Ok(());
                }
            }
            Err(LinkError::Timeout) => {
                let now = clock.now();
                if past_deadline(now, deadline) {
                    return Err(LinkError::Timeout);
                }
                match conn.conn.on_timeout() {
                    Action::Send(SendKind::Fin) => {
                        send_segment(link, conn, seq, SendKind::Fin.flags(), &[])?;
                    }
                    _ => return Err(LinkError::Timeout),
                }
                next_retx = next_retransmit(now, deadline, conn.cfg.retransmit);
            }
            Err(error) => return Err(error),
        }
    }
}
