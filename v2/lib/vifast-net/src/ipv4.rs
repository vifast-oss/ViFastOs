// SPDX-License-Identifier: MIT OR Apache-2.0
//! Header IPv4 — 20 byte không option (RFC 791).
//!
//! `parse` kiểm: `version=4`, `IHL≥5`, `total length` khớp khung, checksum header (có cờ bỏ
//! kiểm), và **từ chối phân mảnh**: `MF` bật hoặc `frag_offset≠0` ⇒ [`Ipv4Error::Unsupported`].
//! D1 chưa có tầng ghép mảnh; im lặng nhận mảnh là sai.

use crate::addr::Ipv4Addr;
use crate::checksum;

/// Độ dài header IPv4 tối thiểu (không option).
pub const IPV4_MIN_HEADER_LEN: usize = 20;
/// `version` của IPv4.
pub const VERSION: u8 = 4;
/// `IHL` khi không có option (5 word 32-bit).
pub const IHL_NO_OPTIONS: u8 = 5;
/// Protocol của ICMP.
pub const PROTO_ICMP: u8 = 1;
/// Protocol của UDP.
pub const PROTO_UDP: u8 = 17;
/// Protocol của TCP.
pub const PROTO_TCP: u8 = 6;
/// Bit `DF` (don't fragment) trong byte flags.
pub const FLAG_DF: u8 = 0x02;
/// Bit `MF` (more fragments) trong byte flags.
pub const FLAG_MF: u8 = 0x01;
/// TTL mặc định cho gói do ta dựng.
pub const DEFAULT_TTL: u8 = 64;

/// Lỗi của codec IPv4.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ipv4Error {
    /// Lát byte ngắn hơn header hoặc ngắn hơn `total length`.
    Truncated,
    /// `version` khác 4.
    BadVersion,
    /// `IHL` nhỏ hơn 5.
    BadIhl,
    /// `total length` nhỏ hơn độ dài header.
    BadTotalLength,
    /// Checksum header sai.
    BadChecksum,
    /// Phân mảnh — chưa hỗ trợ.
    Unsupported,
    /// Bộ đệm đích không đủ chỗ.
    BufferTooSmall,
}

impl Ipv4Error {
    /// Tên ổn định để in trong evidence.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Truncated => "ipv4-truncated",
            Self::BadVersion => "ipv4-bad-version",
            Self::BadIhl => "ipv4-bad-ihl",
            Self::BadTotalLength => "ipv4-bad-total-length",
            Self::BadChecksum => "ipv4-bad-checksum",
            Self::Unsupported => "ipv4-unsupported",
            Self::BufferTooSmall => "ipv4-buffer-too-small",
        }
    }
}

/// Header IPv4 (không option).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ipv4Header {
    /// DSCP (6 bit).
    pub dscp: u8,
    /// ECN (2 bit).
    pub ecn: u8,
    /// Tổng độ dài gói (header + payload), theo byte.
    pub total_len: u16,
    /// Identification.
    pub id: u16,
    /// Ba bit flags (`DF` = [`FLAG_DF`], `MF` = [`FLAG_MF`]).
    pub flags: u8,
    /// Offset phân mảnh, đơn vị 8 byte.
    pub frag_offset: u16,
    /// Time to live.
    pub ttl: u8,
    /// Protocol tầng trên ([`PROTO_ICMP`], [`PROTO_UDP`]).
    pub protocol: u8,
    /// Checksum header như đọc từ dây (0 trước khi [`Ipv4Header::encode`] tính).
    pub checksum: u16,
    /// Địa chỉ nguồn.
    pub src: Ipv4Addr,
    /// Địa chỉ đích.
    pub dst: Ipv4Addr,
}

/// Gói IPv4 đã parse: header + payload (đã cắt theo `total length`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ipv4Packet<'a> {
    /// Header đã parse.
    pub header: Ipv4Header,
    payload: &'a [u8],
}

impl<'a> Ipv4Packet<'a> {
    /// Payload sau header (độ dài = `total length` − `IHL*4`).
    pub fn payload(&self) -> &'a [u8] {
        self.payload
    }
}

impl Ipv4Header {
    /// Header cho gói không option, `ttl` mặc định, `id=0`, không flags.
    pub fn new(protocol: u8, src: Ipv4Addr, dst: Ipv4Addr, payload_len: usize) -> Self {
        Self {
            dscp: 0,
            ecn: 0,
            total_len: (IPV4_MIN_HEADER_LEN + payload_len) as u16,
            id: 0,
            flags: 0,
            frag_offset: 0,
            ttl: DEFAULT_TTL,
            protocol,
            checksum: 0,
            src,
            dst,
        }
    }

    /// Ghi header 20 byte vào `out` và **tính checksum**; trả số byte đã ghi (20).
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, Ipv4Error> {
        if out.len() < IPV4_MIN_HEADER_LEN {
            return Err(Ipv4Error::BufferTooSmall);
        }
        out[0] = (VERSION << 4) | IHL_NO_OPTIONS;
        out[1] = ((self.dscp & 0x3f) << 2) | (self.ecn & 0x03);
        out[2..4].copy_from_slice(&self.total_len.to_be_bytes());
        out[4..6].copy_from_slice(&self.id.to_be_bytes());
        let flags_frag = (u16::from(self.flags & 0x07) << 13) | (self.frag_offset & 0x1fff);
        out[6..8].copy_from_slice(&flags_frag.to_be_bytes());
        out[8] = self.ttl;
        out[9] = self.protocol;
        out[10] = 0;
        out[11] = 0;
        out[12..16].copy_from_slice(&self.src.octets());
        out[16..20].copy_from_slice(&self.dst.octets());
        let c = checksum::internet_checksum(&out[..IPV4_MIN_HEADER_LEN]);
        out[10..12].copy_from_slice(&c.to_be_bytes());
        Ok(IPV4_MIN_HEADER_LEN)
    }

    /// Đọc gói từ `buf`; `verify_checksum=false` bỏ kiểm checksum header (dùng khi tầng dưới đã
    /// kiểm, hoặc khi cần chẩn đoán gói hỏng).
    pub fn parse(buf: &[u8], verify_checksum: bool) -> Result<Ipv4Packet<'_>, Ipv4Error> {
        if buf.len() < IPV4_MIN_HEADER_LEN {
            return Err(Ipv4Error::Truncated);
        }
        if buf[0] >> 4 != VERSION {
            return Err(Ipv4Error::BadVersion);
        }
        let ihl = buf[0] & 0x0f;
        if ihl < IHL_NO_OPTIONS {
            return Err(Ipv4Error::BadIhl);
        }
        let header_len = usize::from(ihl) * 4;
        if buf.len() < header_len {
            return Err(Ipv4Error::Truncated);
        }
        let total_len = u16::from_be_bytes([buf[2], buf[3]]);
        if usize::from(total_len) < header_len {
            return Err(Ipv4Error::BadTotalLength);
        }
        if usize::from(total_len) > buf.len() {
            return Err(Ipv4Error::Truncated);
        }
        let flags = buf[6] >> 5;
        let frag_offset = u16::from_be_bytes([buf[6], buf[7]]) & 0x1fff;
        if flags & FLAG_MF != 0 || frag_offset != 0 {
            return Err(Ipv4Error::Unsupported);
        }
        if verify_checksum && checksum::internet_checksum(&buf[..header_len]) != 0 {
            return Err(Ipv4Error::BadChecksum);
        }
        let header = Self {
            dscp: buf[1] >> 2,
            ecn: buf[1] & 0x03,
            total_len,
            id: u16::from_be_bytes([buf[4], buf[5]]),
            flags,
            frag_offset,
            ttl: buf[8],
            protocol: buf[9],
            checksum: u16::from_be_bytes([buf[10], buf[11]]),
            src: Ipv4Addr([buf[12], buf[13], buf[14], buf[15]]),
            dst: Ipv4Addr([buf[16], buf[17], buf[18], buf[19]]),
        };
        Ok(Ipv4Packet {
            header,
            payload: &buf[header_len..usize::from(total_len)],
        })
    }
}
