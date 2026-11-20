// SPDX-License-Identifier: MIT OR Apache-2.0
//! Địa chỉ tầng liên kết và tầng mạng: [`MacAddr`] (Ethernet 48-bit), [`Ipv4Addr`] và
//! [`SubnetMask`] + [`same_subnet`].

use core::fmt;

/// Địa chỉ MAC 48-bit (Ethernet).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MacAddr(pub [u8; 6]);

impl MacAddr {
    /// Địa chỉ quảng bá `ff:ff:ff:ff:ff:ff`.
    pub const BROADCAST: Self = Self([0xff; 6]);
    /// Địa chỉ toàn 0 — "chưa biết", cũng là `tha` của ARP request.
    pub const ZERO: Self = Self([0; 6]);

    /// Dựng từ sáu octet.
    pub const fn new(octets: [u8; 6]) -> Self {
        Self(octets)
    }

    /// Sáu octet theo thứ tự truyền trên dây.
    pub const fn octets(&self) -> [u8; 6] {
        self.0
    }

    /// `true` nếu là địa chỉ quảng bá.
    pub const fn is_broadcast(&self) -> bool {
        self.0[0] == 0xff
            && self.0[1] == 0xff
            && self.0[2] == 0xff
            && self.0[3] == 0xff
            && self.0[4] == 0xff
            && self.0[5] == 0xff
    }

    /// `true` nếu là địa chỉ toàn 0.
    pub const fn is_zero(&self) -> bool {
        self.0[0] == 0
            && self.0[1] == 0
            && self.0[2] == 0
            && self.0[3] == 0
            && self.0[4] == 0
            && self.0[5] == 0
    }

    /// `true` nếu bit nhóm (LSB của octet đầu) bật — multicast hoặc broadcast.
    pub const fn is_group(&self) -> bool {
        self.0[0] & 0x01 != 0
    }

    /// Mười hai ký tự hex thường, không dấu phân cách (dạng dùng trong log/evidence).
    pub fn to_hex(&self) -> [u8; 12] {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut out = [0u8; 12];
        let mut i = 0;
        while i < 6 {
            out[i * 2] = HEX[usize::from(self.0[i] >> 4)];
            out[i * 2 + 1] = HEX[usize::from(self.0[i] & 0x0f)];
            i += 1;
        }
        out
    }
}

impl fmt::Display for MacAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let o = self.0;
        write!(
            f,
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            o[0], o[1], o[2], o[3], o[4], o[5]
        )
    }
}

/// Địa chỉ IPv4 32-bit, lưu theo thứ tự mạng (big-endian).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ipv4Addr(pub [u8; 4]);

impl Ipv4Addr {
    /// `0.0.0.0` — "chưa có địa chỉ".
    pub const UNSPECIFIED: Self = Self([0; 4]);
    /// `255.255.255.255`.
    pub const BROADCAST: Self = Self([0xff; 4]);

    /// Dựng từ bốn octet.
    pub const fn new(a: u8, b: u8, c: u8, d: u8) -> Self {
        Self([a, b, c, d])
    }

    /// Bốn octet theo thứ tự mạng.
    pub const fn octets(&self) -> [u8; 4] {
        self.0
    }

    /// Từ số nguyên big-endian (cùng thứ tự với `to_u32_be`).
    pub const fn from_u32_be(v: u32) -> Self {
        Self(v.to_be_bytes())
    }

    /// Sang số nguyên big-endian.
    pub const fn to_u32_be(&self) -> u32 {
        u32::from_be_bytes(self.0)
    }

    /// `true` nếu là `0.0.0.0`.
    pub const fn is_unspecified(&self) -> bool {
        self.to_u32_be() == 0
    }

    /// `true` nếu là `255.255.255.255`.
    pub const fn is_broadcast(&self) -> bool {
        self.to_u32_be() == u32::MAX
    }

    /// `true` nếu thuộc `127.0.0.0/8`.
    pub const fn is_loopback(&self) -> bool {
        self.0[0] == 127
    }

    /// `true` nếu thuộc `224.0.0.0/4`.
    pub const fn is_multicast(&self) -> bool {
        self.0[0] & 0xf0 == 0xe0
    }
}

impl fmt::Display for Ipv4Addr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let o = self.0;
        write!(f, "{}.{}.{}.{}", o[0], o[1], o[2], o[3])
    }
}

/// Khẩu độ mạng IPv4 dạng tiền tố (`/24`), chỉ nhận mặt nạ **liên tục**.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubnetMask {
    prefix: u8,
}

impl SubnetMask {
    /// Từ tiền tố `0..=32`; ngoài khoảng trả `None`.
    pub const fn new(prefix: u8) -> Option<Self> {
        if prefix <= 32 {
            Some(Self { prefix })
        } else {
            None
        }
    }

    /// Từ mặt nạ dạng địa chỉ; trả `None` nếu các bit 1 **không** liên tục từ trái.
    pub fn from_addr(mask: Ipv4Addr) -> Option<Self> {
        let v = mask.to_u32_be();
        let prefix = v.leading_ones() as u8;
        let expected = if prefix == 0 {
            0
        } else {
            u32::MAX << (32 - prefix)
        };
        if v == expected {
            Some(Self { prefix })
        } else {
            None
        }
    }

    /// Số bit 1 liên tục từ trái.
    pub const fn prefix(&self) -> u8 {
        self.prefix
    }

    /// Mặt nạ dạng địa chỉ (`/24` ⇒ `255.255.255.0`).
    pub const fn addr(&self) -> Ipv4Addr {
        let v = if self.prefix == 0 {
            0
        } else {
            u32::MAX << (32 - self.prefix)
        };
        Ipv4Addr::from_u32_be(v)
    }

    /// Địa chỉ mạng của `ip` theo mặt nạ này.
    pub const fn network(&self, ip: Ipv4Addr) -> Ipv4Addr {
        Ipv4Addr::from_u32_be(ip.to_u32_be() & self.addr().to_u32_be())
    }

    /// `true` nếu `ip` nằm trong mạng của `net`.
    pub const fn contains(&self, net: Ipv4Addr, ip: Ipv4Addr) -> bool {
        self.network(net).to_u32_be() == self.network(ip).to_u32_be()
    }
}

/// `true` nếu `a` và `b` cùng mạng theo `mask`.
pub fn same_subnet(a: Ipv4Addr, b: Ipv4Addr, mask: SubnetMask) -> bool {
    mask.network(a).to_u32_be() == mask.network(b).to_u32_be()
}
