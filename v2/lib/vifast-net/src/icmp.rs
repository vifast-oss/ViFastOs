// SPDX-License-Identifier: MIT OR Apache-2.0
//! ICMP echo — request (type 8) / reply (type 0), code 0, id, seq, payload (RFC 792).
//!
//! `parse` kiểm checksum trên **toàn** thông điệp (ICMP không có trường độ dài; độ dài thông điệp
//! chính là độ dài lát byte được đưa vào), rồi phân loại: echo request, echo reply, hay `Other`
//! (type khác, hoặc echo có `code≠0`).

use crate::checksum;

/// Độ dài header ICMP echo (type, code, checksum, id, seq).
pub const ICMP_HEADER_LEN: usize = 8;
/// Type của echo reply.
pub const TYPE_ECHO_REPLY: u8 = 0;
/// Type của echo request.
pub const TYPE_ECHO_REQUEST: u8 = 8;
/// Code của echo (luôn 0).
pub const CODE_ECHO: u8 = 0;

/// Lỗi của codec ICMP.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IcmpError {
    /// Lát byte ngắn hơn 8.
    Truncated,
    /// Checksum sai.
    BadChecksum,
    /// Bộ đệm đích không đủ chỗ.
    BufferTooSmall,
}

impl IcmpError {
    /// Tên ổn định để in trong evidence.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Truncated => "icmp-truncated",
            Self::BadChecksum => "icmp-bad-checksum",
            Self::BufferTooSmall => "icmp-buffer-too-small",
        }
    }
}

/// Thông điệp ICMP echo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IcmpEcho<'a> {
    /// Type ([`TYPE_ECHO_REQUEST`] hoặc [`TYPE_ECHO_REPLY`]).
    pub typ: u8,
    /// Identifier.
    pub id: u16,
    /// Sequence number.
    pub seq: u16,
    /// Payload sau 8 byte header.
    pub payload: &'a [u8],
}

/// Thông điệp ICMP đã phân loại.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IcmpMessage<'a> {
    /// Echo request (type 8, code 0).
    EchoRequest(IcmpEcho<'a>),
    /// Echo reply (type 0, code 0).
    EchoReply(IcmpEcho<'a>),
    /// Type khác, hoặc echo có `code≠0`.
    Other {
        /// Type nhận được.
        typ: u8,
        /// Code nhận được.
        code: u8,
    },
}

impl<'a> IcmpEcho<'a> {
    /// Echo request (type 8).
    pub const fn request(id: u16, seq: u16, payload: &'a [u8]) -> Self {
        Self {
            typ: TYPE_ECHO_REQUEST,
            id,
            seq,
            payload,
        }
    }

    /// Echo reply (type 0).
    pub const fn reply(id: u16, seq: u16, payload: &'a [u8]) -> Self {
        Self {
            typ: TYPE_ECHO_REPLY,
            id,
            seq,
            payload,
        }
    }

    /// Ghi thông điệp vào `out` và tính checksum; trả số byte đã ghi.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, IcmpError> {
        let need = ICMP_HEADER_LEN + self.payload.len();
        if out.len() < need {
            return Err(IcmpError::BufferTooSmall);
        }
        out[0] = self.typ;
        out[1] = CODE_ECHO;
        out[2] = 0;
        out[3] = 0;
        out[4..6].copy_from_slice(&self.id.to_be_bytes());
        out[6..8].copy_from_slice(&self.seq.to_be_bytes());
        out[ICMP_HEADER_LEN..need].copy_from_slice(self.payload);
        let c = checksum::internet_checksum(&out[..need]);
        out[2..4].copy_from_slice(&c.to_be_bytes());
        Ok(need)
    }

    /// Đọc thông điệp từ `buf` (kiểm checksum trên toàn `buf`) và phân loại.
    pub fn parse(buf: &[u8]) -> Result<IcmpMessage<'_>, IcmpError> {
        if buf.len() < ICMP_HEADER_LEN {
            return Err(IcmpError::Truncated);
        }
        if checksum::internet_checksum(buf) != 0 {
            return Err(IcmpError::BadChecksum);
        }
        let typ = buf[0];
        let code = buf[1];
        let id = u16::from_be_bytes([buf[4], buf[5]]);
        let seq = u16::from_be_bytes([buf[6], buf[7]]);
        let payload = &buf[ICMP_HEADER_LEN..];
        match (typ, code) {
            (TYPE_ECHO_REQUEST, CODE_ECHO) => Ok(IcmpMessage::EchoRequest(IcmpEcho {
                typ,
                id,
                seq,
                payload,
            })),
            (TYPE_ECHO_REPLY, CODE_ECHO) => Ok(IcmpMessage::EchoReply(IcmpEcho {
                typ,
                id,
                seq,
                payload,
            })),
            _ => Ok(IcmpMessage::Other { typ, code }),
        }
    }
}
