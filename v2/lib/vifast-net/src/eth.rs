// SPDX-License-Identifier: MIT OR Apache-2.0
//! Header Ethernet II — 14 byte: đích (6), nguồn (6), ethertype (2).

use crate::addr::MacAddr;

/// Độ dài header Ethernet II.
pub const ETH_HEADER_LEN: usize = 14;
/// Ethertype của IPv4.
pub const ETHERTYPE_IPV4: u16 = 0x0800;
/// Ethertype của ARP.
pub const ETHERTYPE_ARP: u16 = 0x0806;
/// Payload tối thiểu của một frame Ethernet hợp lệ (không tính FCS).
pub const ETH_MIN_PAYLOAD: usize = 46;
/// Frame Ethernet tối thiểu trên dây (không tính FCS): 14 + 46.
pub const ETH_MIN_FRAME: usize = 60;

/// Lỗi của codec Ethernet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EthError {
    /// Lát byte ngắn hơn header.
    Truncated,
    /// Bộ đệm đích không đủ chỗ.
    BufferTooSmall,
}

impl EthError {
    /// Tên ổn định để in trong evidence (D2-102: không in số trần).
    pub const fn name(self) -> &'static str {
        match self {
            Self::Truncated => "eth-truncated",
            Self::BufferTooSmall => "eth-buffer-too-small",
        }
    }
}

/// Header Ethernet II.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EthHeader {
    /// Địa chỉ đích.
    pub dst: MacAddr,
    /// Địa chỉ nguồn.
    pub src: MacAddr,
    /// Loại payload (xem [`ETHERTYPE_IPV4`], [`ETHERTYPE_ARP`]).
    pub ethertype: u16,
}

impl EthHeader {
    /// Độ dài header (14).
    pub const LEN: usize = ETH_HEADER_LEN;

    /// Dựng header.
    pub const fn new(dst: MacAddr, src: MacAddr, ethertype: u16) -> Self {
        Self {
            dst,
            src,
            ethertype,
        }
    }

    /// Ghi header vào `out`, trả số byte đã ghi (14).
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, EthError> {
        if out.len() < ETH_HEADER_LEN {
            return Err(EthError::BufferTooSmall);
        }
        out[0..6].copy_from_slice(&self.dst.octets());
        out[6..12].copy_from_slice(&self.src.octets());
        out[12..14].copy_from_slice(&self.ethertype.to_be_bytes());
        Ok(ETH_HEADER_LEN)
    }

    /// Đọc header từ đầu `buf`; đòi ít nhất 14 byte.
    pub fn parse(buf: &[u8]) -> Result<Self, EthError> {
        if buf.len() < ETH_HEADER_LEN {
            return Err(EthError::Truncated);
        }
        let mut dst = [0u8; 6];
        dst.copy_from_slice(&buf[0..6]);
        let mut src = [0u8; 6];
        src.copy_from_slice(&buf[6..12]);
        Ok(Self {
            dst: MacAddr(dst),
            src: MacAddr(src),
            ethertype: u16::from_be_bytes([buf[12], buf[13]]),
        })
    }
}
