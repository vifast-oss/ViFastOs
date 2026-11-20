// SPDX-License-Identifier: MIT OR Apache-2.0
//! Bọc header Ethernet II với payload — tiện cho driver: một frame vào, một `(header, payload)`
//! ra, không phải tự cắt 14 byte ở mỗi chỗ gọi.

use crate::eth::{EthError, EthHeader, ETH_HEADER_LEN};

/// Frame Ethernet II: header + payload (không gồm FCS — QEMU/virtio không đưa FCS lên driver).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EthernetFrame<'a> {
    /// Header Ethernet.
    pub header: EthHeader,
    /// Payload sau 14 byte header.
    pub payload: &'a [u8],
}

impl<'a> EthernetFrame<'a> {
    /// Đọc frame từ `buf`; đòi ít nhất 14 byte.
    pub fn parse(buf: &'a [u8]) -> Result<Self, EthError> {
        let header = EthHeader::parse(buf)?;
        Ok(Self {
            header,
            payload: &buf[ETH_HEADER_LEN..],
        })
    }

    /// Ghi header + payload vào `out`; trả tổng số byte đã ghi.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, EthError> {
        let n = self.header.encode(out)?;
        let need = n + self.payload.len();
        if out.len() < need {
            return Err(EthError::BufferTooSmall);
        }
        out[n..need].copy_from_slice(self.payload);
        Ok(need)
    }

    /// Tổng độ dài frame (header + payload).
    pub fn frame_len(&self) -> usize {
        ETH_HEADER_LEN + self.payload.len()
    }
}
