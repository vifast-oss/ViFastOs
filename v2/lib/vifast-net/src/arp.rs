// SPDX-License-Identifier: MIT OR Apache-2.0
//! ARP cho IPv4 trên Ethernet — 28 byte (RFC 826).
//!
//! Chỉ nhận `htype=1` (Ethernet), `ptype=0x0800` (IPv4), `hlen=6`, `plen=4`; `oper` chỉ nhận
//! request (1) hoặc reply (2). Mọi trường khác là lỗi tường minh, không đoán.

use crate::addr::{Ipv4Addr, MacAddr};
use crate::eth::ETHERTYPE_IPV4;

/// Độ dài gói ARP cho IPv4/Ethernet.
pub const ARP_LEN: usize = 28;
/// `htype` của Ethernet.
pub const HTYPE_ETHERNET: u16 = 1;
/// `ptype` của IPv4.
pub const PTYPE_IPV4: u16 = ETHERTYPE_IPV4;
/// `hlen` của địa chỉ phần cứng Ethernet.
pub const HLEN_ETHERNET: u8 = 6;
/// `plen` của địa chỉ giao thức IPv4.
pub const PLEN_IPV4: u8 = 4;
/// `oper` của ARP request.
pub const OPER_REQUEST: u16 = 1;
/// `oper` của ARP reply.
pub const OPER_REPLY: u16 = 2;

/// Lỗi của codec ARP.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArpError {
    /// Lát byte ngắn hơn 28.
    Truncated,
    /// Bộ đệm đích không đủ chỗ.
    BufferTooSmall,
    /// `htype` khác Ethernet.
    BadHtype,
    /// `ptype` khác IPv4.
    BadPtype,
    /// `hlen` khác 6.
    BadHlen,
    /// `plen` khác 4.
    BadPlen,
    /// `oper` không phải request/reply.
    BadOper,
}

impl ArpError {
    /// Tên ổn định để in trong evidence.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Truncated => "arp-truncated",
            Self::BufferTooSmall => "arp-buffer-too-small",
            Self::BadHtype => "arp-bad-htype",
            Self::BadPtype => "arp-bad-ptype",
            Self::BadHlen => "arp-bad-hlen",
            Self::BadPlen => "arp-bad-plen",
            Self::BadOper => "arp-bad-oper",
        }
    }
}

/// Gói ARP IPv4/Ethernet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArpPacket {
    /// Loại thao tác ([`OPER_REQUEST`] hoặc [`OPER_REPLY`]).
    pub oper: u16,
    /// Địa chỉ MAC nguồn.
    pub sha: MacAddr,
    /// Địa chỉ IPv4 nguồn.
    pub spa: Ipv4Addr,
    /// Địa chỉ MAC đích (`00:00:00:00:00:00` khi hỏi).
    pub tha: MacAddr,
    /// Địa chỉ IPv4 đích.
    pub tpa: Ipv4Addr,
}

impl ArpPacket {
    /// ARP request: `tha` để 0, `oper` = 1.
    pub const fn request(sha: MacAddr, spa: Ipv4Addr, tpa: Ipv4Addr) -> Self {
        Self {
            oper: OPER_REQUEST,
            sha,
            spa,
            tha: MacAddr::ZERO,
            tpa,
        }
    }

    /// ARP reply: `oper` = 2.
    pub const fn reply(sha: MacAddr, spa: Ipv4Addr, tha: MacAddr, tpa: Ipv4Addr) -> Self {
        Self {
            oper: OPER_REPLY,
            sha,
            spa,
            tha,
            tpa,
        }
    }

    /// `true` nếu là request.
    pub const fn is_request(&self) -> bool {
        self.oper == OPER_REQUEST
    }

    /// `true` nếu là reply.
    pub const fn is_reply(&self) -> bool {
        self.oper == OPER_REPLY
    }

    /// Ghi gói vào `out`, trả số byte đã ghi (28).
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, ArpError> {
        if out.len() < ARP_LEN {
            return Err(ArpError::BufferTooSmall);
        }
        out[0..2].copy_from_slice(&HTYPE_ETHERNET.to_be_bytes());
        out[2..4].copy_from_slice(&PTYPE_IPV4.to_be_bytes());
        out[4] = HLEN_ETHERNET;
        out[5] = PLEN_IPV4;
        out[6..8].copy_from_slice(&self.oper.to_be_bytes());
        out[8..14].copy_from_slice(&self.sha.octets());
        out[14..18].copy_from_slice(&self.spa.octets());
        out[18..24].copy_from_slice(&self.tha.octets());
        out[24..28].copy_from_slice(&self.tpa.octets());
        Ok(ARP_LEN)
    }

    /// Đọc gói từ đầu `buf`; kiểm `htype`/`ptype`/`hlen`/`plen`/`oper`.
    pub fn parse(buf: &[u8]) -> Result<Self, ArpError> {
        if buf.len() < ARP_LEN {
            return Err(ArpError::Truncated);
        }
        if u16::from_be_bytes([buf[0], buf[1]]) != HTYPE_ETHERNET {
            return Err(ArpError::BadHtype);
        }
        if u16::from_be_bytes([buf[2], buf[3]]) != PTYPE_IPV4 {
            return Err(ArpError::BadPtype);
        }
        if buf[4] != HLEN_ETHERNET {
            return Err(ArpError::BadHlen);
        }
        if buf[5] != PLEN_IPV4 {
            return Err(ArpError::BadPlen);
        }
        let oper = u16::from_be_bytes([buf[6], buf[7]]);
        if oper != OPER_REQUEST && oper != OPER_REPLY {
            return Err(ArpError::BadOper);
        }
        let mut sha = [0u8; 6];
        sha.copy_from_slice(&buf[8..14]);
        let mut tha = [0u8; 6];
        tha.copy_from_slice(&buf[18..24]);
        Ok(Self {
            oper,
            sha: MacAddr(sha),
            spa: Ipv4Addr([buf[14], buf[15], buf[16], buf[17]]),
            tha: MacAddr(tha),
            tpa: Ipv4Addr([buf[24], buf[25], buf[26], buf[27]]),
        })
    }
}
