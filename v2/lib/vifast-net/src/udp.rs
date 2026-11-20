// SPDX-License-Identifier: MIT OR Apache-2.0
//! UDP — header 8 byte + checksum có pseudo-header IPv4 (RFC 768).
//!
//! Checksum 0 nghĩa là "không dùng checksum" (RFC 768 cho phép với IPv4) và được chấp nhận khi
//! parse. Checksum tính ra 0 thì truyền `0xffff` (RFC 768 §"Checksum").

use crate::addr::Ipv4Addr;
use crate::checksum::Checksum;
use crate::ipv4::PROTO_UDP;

/// Độ dài header UDP.
pub const UDP_HEADER_LEN: usize = 8;
/// Cổng UDP của DHCP client (RFC 2131).
pub const DHCP_CLIENT_PORT: u16 = 68;
/// Cổng UDP của DHCP server (RFC 2131).
pub const DHCP_SERVER_PORT: u16 = 67;

/// Lỗi của codec UDP.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UdpError {
    /// Lát byte ngắn hơn header hoặc ngắn hơn trường `length`.
    Truncated,
    /// Trường `length` nhỏ hơn 8 hoặc vượt 65535.
    BadLength,
    /// Checksum sai (khi bật kiểm và checksum khác 0).
    BadChecksum,
    /// Bộ đệm đích không đủ chỗ.
    BufferTooSmall,
}

impl UdpError {
    /// Tên ổn định để in trong evidence.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Truncated => "udp-truncated",
            Self::BadLength => "udp-bad-length",
            Self::BadChecksum => "udp-bad-checksum",
            Self::BufferTooSmall => "udp-buffer-too-small",
        }
    }
}

/// Header UDP.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UdpHeader {
    /// Cổng nguồn.
    pub src_port: u16,
    /// Cổng đích.
    pub dst_port: u16,
    /// Độ dài datagram (header + payload).
    pub len: u16,
    /// Checksum (0 = không dùng).
    pub checksum: u16,
}

/// Datagram UDP đã parse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UdpDatagram<'a> {
    /// Header đã parse.
    pub header: UdpHeader,
    payload: &'a [u8],
}

impl<'a> UdpDatagram<'a> {
    /// Payload sau header (độ dài = `len` − 8).
    pub fn payload(&self) -> &'a [u8] {
        self.payload
    }
}

/// Checksum UDP trên pseudo-header IPv4 + datagram. `udp` phải bắt đầu bằng header UDP; khi
/// **tính** thì trường checksum phải là 0, khi **kiểm** thì giữ nguyên giá trị nhận được (kết quả
/// đúng phải bằng 0).
fn checksum_v4(src: Ipv4Addr, dst: Ipv4Addr, udp: &[u8]) -> u16 {
    let mut c = Checksum::new();
    c.add_bytes(&src.octets());
    c.add_bytes(&dst.octets());
    c.add_bytes(&[0, PROTO_UDP]);
    c.add_u16(udp.len() as u16);
    c.add_bytes(udp);
    c.finish()
}

impl UdpHeader {
    /// Ghi header + payload vào `out`, tính checksum có pseudo-header; trả số byte đã ghi.
    pub fn encode(
        src: Ipv4Addr,
        dst: Ipv4Addr,
        sport: u16,
        dport: u16,
        payload: &[u8],
        out: &mut [u8],
    ) -> Result<usize, UdpError> {
        let udp_len = UDP_HEADER_LEN + payload.len();
        if out.len() < udp_len {
            return Err(UdpError::BufferTooSmall);
        }
        let len16 = u16::try_from(udp_len).map_err(|_| UdpError::BadLength)?;
        out[0..2].copy_from_slice(&sport.to_be_bytes());
        out[2..4].copy_from_slice(&dport.to_be_bytes());
        out[4..6].copy_from_slice(&len16.to_be_bytes());
        out[6] = 0;
        out[7] = 0;
        out[UDP_HEADER_LEN..udp_len].copy_from_slice(payload);
        let c = checksum_v4(src, dst, &out[..udp_len]);
        let c = if c == 0 { 0xffff } else { c };
        out[6..8].copy_from_slice(&c.to_be_bytes());
        Ok(udp_len)
    }

    /// Đọc datagram từ `buf`; kiểm `length` và (khi `verify_checksum` và checksum khác 0)
    /// checksum có pseudo-header.
    pub fn parse(
        src: Ipv4Addr,
        dst: Ipv4Addr,
        buf: &[u8],
        verify_checksum: bool,
    ) -> Result<UdpDatagram<'_>, UdpError> {
        if buf.len() < UDP_HEADER_LEN {
            return Err(UdpError::Truncated);
        }
        let len = u16::from_be_bytes([buf[4], buf[5]]);
        if usize::from(len) < UDP_HEADER_LEN {
            return Err(UdpError::BadLength);
        }
        if usize::from(len) > buf.len() {
            return Err(UdpError::Truncated);
        }
        let checksum = u16::from_be_bytes([buf[6], buf[7]]);
        if verify_checksum && checksum != 0 && checksum_v4(src, dst, &buf[..usize::from(len)]) != 0
        {
            return Err(UdpError::BadChecksum);
        }
        Ok(UdpDatagram {
            header: UdpHeader {
                src_port: u16::from_be_bytes([buf[0], buf[1]]),
                dst_port: u16::from_be_bytes([buf[2], buf[3]]),
                len,
                checksum,
            },
            payload: &buf[UDP_HEADER_LEN..usize::from(len)],
        })
    }
}
