// SPDX-License-Identifier: MIT OR Apache-2.0
//! BOOTP/DHCP — dựng DISCOVER/REQUEST và đọc OFFER/ACK (RFC 951 + RFC 2131/2132).
//!
//! Khung cố định 236 byte + magic cookie 4 byte + options TLV, tổng khung tối đa
//! [`DHCP_MAX_LEN`] (576, sàn tái hợp của RFC 951). [`DhcpMessage`] **sở hữu** khung đó vì thông
//! điệp được dựng rồi mới gửi; mọi getter đọc trên khung đã kiểm.
//!
//! `parse` kiểm: độ dài khung, magic cookie, `htype=1`/`hlen=6`, `op` hợp lệ, và **từng option
//! TLV** phải nằm trong khung (option dài quá khung ⇒ [`DhcpError::OptionOverrun`]); thiếu byte
//! `END` ⇒ [`DhcpError::MissingEnd`]. Nhờ vậy getter không bao giờ đọc ngoài khung.

use crate::addr::{Ipv4Addr, MacAddr};

/// Độ dài phần cố định BOOTP + magic cookie.
pub const DHCP_FIXED_LEN: usize = 240;
/// Khung DHCP tối đa (sàn tái hợp RFC 951).
pub const DHCP_MAX_LEN: usize = 576;
/// Magic cookie `99.130.83.99` (0x63825363).
pub const MAGIC: [u8; 4] = [99, 130, 83, 99];
/// Magic cookie dạng số nguyên big-endian.
pub const MAGIC_COOKIE: u32 = u32::from_be_bytes(MAGIC);
/// `op` của thông điệp do client gửi.
pub const OP_REQUEST: u8 = 1;
/// `op` của thông điệp do server gửi.
pub const OP_REPLY: u8 = 2;
/// `htype` của Ethernet.
pub const HTYPE_ETHERNET: u8 = 1;
/// `hlen` của địa chỉ MAC Ethernet.
pub const HLEN_ETHERNET: u8 = 6;
/// Cờ "broadcast" trong trường `flags`.
pub const FLAG_BROADCAST: u16 = 0x8000;
/// Option pad (bỏ qua).
pub const OPT_PAD: u8 = 0;
/// Option end (kết thúc danh sách).
pub const OPT_END: u8 = 255;
/// Option 1 — subnet mask.
pub const OPT_SUBNET_MASK: u8 = 1;
/// Option 3 — router.
pub const OPT_ROUTER: u8 = 3;
/// Option 6 — DNS.
pub const OPT_DNS: u8 = 6;
/// Option 12 — hostname.
pub const OPT_HOSTNAME: u8 = 12;
/// Option 50 — requested IP address.
pub const OPT_REQUESTED_IP: u8 = 50;
/// Option 51 — lease time (giây).
pub const OPT_LEASE_TIME: u8 = 51;
/// Option 53 — DHCP message type.
pub const OPT_MSG_TYPE: u8 = 53;
/// Option 54 — server identifier.
pub const OPT_SERVER_ID: u8 = 54;
/// Option 55 — parameter request list.
pub const OPT_PARAM_REQUEST: u8 = 55;
/// Message type DISCOVER.
pub const DHCPDISCOVER: u8 = 1;
/// Message type OFFER.
pub const DHCPOFFER: u8 = 2;
/// Message type REQUEST.
pub const DHCPREQUEST: u8 = 3;
/// Message type ACK.
pub const DHCPACK: u8 = 5;
/// Message type NAK.
pub const DHCPNAK: u8 = 6;

// Offset các trường trong khung BOOTP.
const OFF_OP: usize = 0;
const OFF_HTYPE: usize = 1;
const OFF_HLEN: usize = 2;
const OFF_HOPS: usize = 3;
const OFF_XID: usize = 4;
const OFF_SECS: usize = 8;
const OFF_FLAGS: usize = 10;
const OFF_CIADDR: usize = 12;
const OFF_YIADDR: usize = 16;
const OFF_SIADDR: usize = 20;
const OFF_GIADDR: usize = 24;
const OFF_CHADDR: usize = 28;
const OFF_COOKIE: usize = 236;

/// Danh sách tham số client xin server trả (option 55).
const PARAM_REQUEST_LIST: [u8; 4] = [OPT_SUBNET_MASK, OPT_ROUTER, OPT_DNS, OPT_HOSTNAME];

/// Lỗi của codec DHCP.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DhcpError {
    /// Khung ngắn hơn [`DHCP_FIXED_LEN`].
    Truncated,
    /// Khung dài hơn [`DHCP_MAX_LEN`].
    TooLong,
    /// Magic cookie sai.
    BadCookie,
    /// `htype` khác Ethernet.
    BadHtype,
    /// `hlen` khác 6.
    BadHlen,
    /// `op` không phải request/reply.
    BadOp,
    /// Option dài quá khung.
    OptionOverrun,
    /// Thiếu byte `END` (255).
    MissingEnd,
    /// Không còn chỗ trong khung để thêm option.
    BufferTooSmall,
    /// Dữ liệu option dài hơn 255 byte (không biểu diễn được bằng TLV một byte).
    OptionTooLong,
}

impl DhcpError {
    /// Tên ổn định để in trong evidence.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Truncated => "dhcp-truncated",
            Self::TooLong => "dhcp-too-long",
            Self::BadCookie => "dhcp-bad-cookie",
            Self::BadHtype => "dhcp-bad-htype",
            Self::BadHlen => "dhcp-bad-hlen",
            Self::BadOp => "dhcp-bad-op",
            Self::OptionOverrun => "dhcp-option-overrun",
            Self::MissingEnd => "dhcp-missing-end",
            Self::BufferTooSmall => "dhcp-buffer-too-small",
            Self::OptionTooLong => "dhcp-option-too-long",
        }
    }
}

/// Thông điệp DHCP sở hữu khung của nó.
#[derive(Clone)]
pub struct DhcpMessage {
    buf: [u8; DHCP_MAX_LEN],
    len: usize,
}

impl DhcpMessage {
    /// Khung client rỗng: `op=1`, Ethernet, `flags` broadcast, `chaddr=mac`, magic cookie.
    fn new_client(xid: u32, mac: MacAddr) -> Self {
        let mut buf = [0u8; DHCP_MAX_LEN];
        buf[OFF_OP] = OP_REQUEST;
        buf[OFF_HTYPE] = HTYPE_ETHERNET;
        buf[OFF_HLEN] = HLEN_ETHERNET;
        buf[OFF_XID..OFF_XID + 4].copy_from_slice(&xid.to_be_bytes());
        buf[OFF_FLAGS..OFF_FLAGS + 2].copy_from_slice(&FLAG_BROADCAST.to_be_bytes());
        buf[OFF_CHADDR..OFF_CHADDR + 6].copy_from_slice(&mac.octets());
        buf[OFF_COOKIE..OFF_COOKIE + 4].copy_from_slice(&MAGIC);
        Self {
            buf,
            len: DHCP_FIXED_LEN,
        }
    }

    /// Thêm một option TLV vào cuối khung.
    fn push_option(&mut self, code: u8, data: &[u8]) -> Result<(), DhcpError> {
        let l = u8::try_from(data.len()).map_err(|_| DhcpError::OptionTooLong)?;
        let need = 2 + data.len();
        if self.len + need > DHCP_MAX_LEN {
            return Err(DhcpError::BufferTooSmall);
        }
        self.buf[self.len] = code;
        self.buf[self.len + 1] = l;
        self.buf[self.len + 2..self.len + 2 + data.len()].copy_from_slice(data);
        self.len += need;
        Ok(())
    }

    /// Đóng danh sách option bằng byte `END`.
    fn finish_options(&mut self) {
        if self.len < DHCP_MAX_LEN {
            self.buf[self.len] = OPT_END;
            self.len += 1;
        }
    }

    /// DHCPDISCOVER: option 53 = 1, option 55 = danh sách tham số.
    ///
    /// Không thể tràn khung: 240 + 3 + 6 + 1 = 250 ≤ 576.
    pub fn discover(xid: u32, mac: MacAddr) -> Self {
        let mut m = Self::new_client(xid, mac);
        m.push_option(OPT_MSG_TYPE, &[DHCPDISCOVER])
            .expect("discover: option 53 vừa khung");
        m.push_option(OPT_PARAM_REQUEST, &PARAM_REQUEST_LIST)
            .expect("discover: option 55 vừa khung");
        m.finish_options();
        m
    }

    /// DHCPREQUEST: option 53 = 3, 50 = IP xin, 54 = server id, 55 = danh sách tham số.
    ///
    /// Không thể tràn khung: 240 + 3 + 6 + 6 + 6 + 1 = 262 ≤ 576.
    pub fn request(xid: u32, mac: MacAddr, requested: Ipv4Addr, server: Ipv4Addr) -> Self {
        let mut m = Self::new_client(xid, mac);
        m.push_option(OPT_MSG_TYPE, &[DHCPREQUEST])
            .expect("request: option 53 vừa khung");
        m.push_option(OPT_REQUESTED_IP, &requested.octets())
            .expect("request: option 50 vừa khung");
        m.push_option(OPT_SERVER_ID, &server.octets())
            .expect("request: option 54 vừa khung");
        m.push_option(OPT_PARAM_REQUEST, &PARAM_REQUEST_LIST)
            .expect("request: option 55 vừa khung");
        m.finish_options();
        m
    }

    /// Đọc thông điệp từ `src`; kiểm khung, cookie, `htype`/`hlen`/`op` và **toàn bộ** option TLV.
    pub fn parse(src: &[u8]) -> Result<Self, DhcpError> {
        if src.len() < DHCP_FIXED_LEN {
            return Err(DhcpError::Truncated);
        }
        if src.len() > DHCP_MAX_LEN {
            return Err(DhcpError::TooLong);
        }
        if src[OFF_COOKIE..OFF_COOKIE + 4] != MAGIC {
            return Err(DhcpError::BadCookie);
        }
        if src[OFF_HTYPE] != HTYPE_ETHERNET {
            return Err(DhcpError::BadHtype);
        }
        if src[OFF_HLEN] != HLEN_ETHERNET {
            return Err(DhcpError::BadHlen);
        }
        if src[OFF_OP] != OP_REQUEST && src[OFF_OP] != OP_REPLY {
            return Err(DhcpError::BadOp);
        }
        let mut i = DHCP_FIXED_LEN;
        let mut end = None;
        while i < src.len() {
            let code = src[i];
            if code == OPT_PAD {
                i += 1;
                continue;
            }
            if code == OPT_END {
                end = Some(i);
                break;
            }
            if i + 1 >= src.len() {
                return Err(DhcpError::OptionOverrun);
            }
            let l = usize::from(src[i + 1]);
            if i + 2 + l > src.len() {
                return Err(DhcpError::OptionOverrun);
            }
            i += 2 + l;
        }
        let end = end.ok_or(DhcpError::MissingEnd)?;
        let len = end + 1;
        let mut buf = [0u8; DHCP_MAX_LEN];
        buf[..len].copy_from_slice(&src[..len]);
        Ok(Self { buf, len })
    }

    /// Khung thông điệp (gồm byte `END`).
    pub fn as_bytes(&self) -> &[u8] {
        &self.buf[..self.len]
    }

    /// Độ dài khung.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Luôn `false`: thông điệp DHCP hợp lệ tối thiểu [`DHCP_FIXED_LEN`] byte.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// `op` (1 = request, 2 = reply).
    pub fn op(&self) -> u8 {
        self.buf[OFF_OP]
    }

    /// `htype`.
    pub fn htype(&self) -> u8 {
        self.buf[OFF_HTYPE]
    }

    /// `hlen`.
    pub fn hlen(&self) -> u8 {
        self.buf[OFF_HLEN]
    }

    /// `hops`.
    pub fn hops(&self) -> u8 {
        self.buf[OFF_HOPS]
    }

    /// Transaction id.
    pub fn xid(&self) -> u32 {
        u32::from_be_bytes([
            self.buf[OFF_XID],
            self.buf[OFF_XID + 1],
            self.buf[OFF_XID + 2],
            self.buf[OFF_XID + 3],
        ])
    }

    /// `secs`.
    pub fn secs(&self) -> u16 {
        u16::from_be_bytes([self.buf[OFF_SECS], self.buf[OFF_SECS + 1]])
    }

    /// `flags`.
    pub fn flags(&self) -> u16 {
        u16::from_be_bytes([self.buf[OFF_FLAGS], self.buf[OFF_FLAGS + 1]])
    }

    /// `ciaddr`.
    pub fn ciaddr(&self) -> Ipv4Addr {
        self.ipv4_at(OFF_CIADDR)
    }

    /// `yiaddr` — địa chỉ server cấp cho client.
    pub fn yiaddr(&self) -> Ipv4Addr {
        self.ipv4_at(OFF_YIADDR)
    }

    /// `siaddr`.
    pub fn siaddr(&self) -> Ipv4Addr {
        self.ipv4_at(OFF_SIADDR)
    }

    /// `giaddr`.
    pub fn giaddr(&self) -> Ipv4Addr {
        self.ipv4_at(OFF_GIADDR)
    }

    /// `chaddr` — MAC client (6 byte đầu của trường 16 byte).
    pub fn chaddr(&self) -> MacAddr {
        let mut mac = [0u8; 6];
        mac.copy_from_slice(&self.buf[OFF_CHADDR..OFF_CHADDR + 6]);
        MacAddr(mac)
    }

    /// Option 53 — DHCP message type.
    pub fn msg_type(&self) -> Option<u8> {
        let d = self.option(OPT_MSG_TYPE)?;
        if d.len() == 1 {
            Some(d[0])
        } else {
            None
        }
    }

    /// Option 1 — subnet mask.
    pub fn subnet_mask(&self) -> Option<Ipv4Addr> {
        self.ipv4_option(OPT_SUBNET_MASK)
    }

    /// Option 3 — router (địa chỉ đầu tiên).
    pub fn router(&self) -> Option<Ipv4Addr> {
        self.ipv4_option(OPT_ROUTER)
    }

    /// Option 6 — DNS (địa chỉ đầu tiên).
    pub fn dns(&self) -> Option<Ipv4Addr> {
        self.ipv4_option(OPT_DNS)
    }

    /// Option 54 — server identifier.
    pub fn server_id(&self) -> Option<Ipv4Addr> {
        self.ipv4_option(OPT_SERVER_ID)
    }

    /// Option 50 — requested IP address.
    pub fn requested_ip(&self) -> Option<Ipv4Addr> {
        self.ipv4_option(OPT_REQUESTED_IP)
    }

    /// Option 51 — lease time (giây).
    pub fn lease_secs(&self) -> Option<u32> {
        let d = self.option(OPT_LEASE_TIME)?;
        if d.len() == 4 {
            Some(u32::from_be_bytes([d[0], d[1], d[2], d[3]]))
        } else {
            None
        }
    }

    /// Option 12 — hostname (byte thô, không giả định UTF-8).
    pub fn hostname(&self) -> Option<&[u8]> {
        self.option(OPT_HOSTNAME)
    }

    /// Option bất kỳ theo mã; `None` nếu không có hoặc danh sách hỏng.
    pub fn option(&self, code: u8) -> Option<&[u8]> {
        let opts = &self.buf[DHCP_FIXED_LEN..self.len];
        let mut i = 0;
        while i < opts.len() {
            let c = opts[i];
            if c == OPT_PAD {
                i += 1;
                continue;
            }
            if c == OPT_END {
                return None;
            }
            if i + 1 >= opts.len() {
                return None;
            }
            let l = usize::from(opts[i + 1]);
            if i + 2 + l > opts.len() {
                return None;
            }
            if c == code {
                return Some(&opts[i + 2..i + 2 + l]);
            }
            i += 2 + l;
        }
        None
    }

    fn ipv4_at(&self, off: usize) -> Ipv4Addr {
        Ipv4Addr([
            self.buf[off],
            self.buf[off + 1],
            self.buf[off + 2],
            self.buf[off + 3],
        ])
    }

    fn ipv4_option(&self, code: u8) -> Option<Ipv4Addr> {
        let d = self.option(code)?;
        if d.len() >= 4 {
            Some(Ipv4Addr([d[0], d[1], d[2], d[3]]))
        } else {
            None
        }
    }
}
