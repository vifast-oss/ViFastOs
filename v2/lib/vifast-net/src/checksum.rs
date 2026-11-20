// SPDX-License-Identifier: MIT OR Apache-2.0
//! Checksum Internet 16-bit (RFC 1071): tổng bù một của các word 16-bit big-endian.
//!
//! [`Checksum`] cộng dồn **từng mảnh** nên pseudo-header UDP/TCP (RFC 768/793) không phải sao
//! chép payload vào bộ đệm tạm: cộng địa chỉ nguồn/đích, byte 0 + protocol, độ dài, rồi payload.

/// Bộ cộng dồn checksum Internet.
///
/// Byte lẻ ở cuối một mảnh được **giữ lại** và ghép với byte đầu của mảnh kế tiếp, nên
/// `add_bytes(&[a]); add_bytes(&[b, c])` cho cùng kết quả với `add_bytes(&[a, b, c])`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Checksum {
    sum: u32,
    pending: Option<u8>,
}

impl Checksum {
    /// Bộ cộng dồn rỗng.
    pub const fn new() -> Self {
        Self {
            sum: 0,
            pending: None,
        }
    }

    /// Cộng thêm một mảnh byte.
    pub fn add_bytes(&mut self, data: &[u8]) {
        let mut i = 0;
        if let Some(hi) = self.pending.take() {
            if data.is_empty() {
                self.pending = Some(hi);
                return;
            }
            self.sum += u32::from(u16::from_be_bytes([hi, data[0]]));
            i = 1;
        }
        while i + 1 < data.len() {
            self.sum += u32::from(u16::from_be_bytes([data[i], data[i + 1]]));
            i += 2;
        }
        if i < data.len() {
            self.pending = Some(data[i]);
        }
    }

    /// Cộng thêm một word 16-bit (theo thứ tự big-endian).
    pub fn add_u16(&mut self, v: u16) {
        self.add_bytes(&v.to_be_bytes());
    }

    /// Kết quả: bù một của tổng đã gấp tràn.
    pub fn finish(&self) -> u16 {
        let mut sum = self.sum;
        if let Some(hi) = self.pending {
            sum += u32::from(hi) << 8;
        }
        while sum >> 16 != 0 {
            sum = (sum & 0xffff) + (sum >> 16);
        }
        !(sum as u16)
    }
}

/// Checksum Internet của một lát byte.
pub fn internet_checksum(data: &[u8]) -> u16 {
    let mut c = Checksum::new();
    c.add_bytes(data);
    c.finish()
}

/// Checksum Internet của nhiều mảnh nối tiếp (tương đương nối byte rồi tính một lần).
pub fn checksum_parts(parts: &[&[u8]]) -> u16 {
    let mut c = Checksum::new();
    for p in parts {
        c.add_bytes(p);
    }
    c.finish()
}
