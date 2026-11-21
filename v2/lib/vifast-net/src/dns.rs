// SPDX-License-Identifier: MIT OR Apache-2.0
//! DNS — thông điệp RFC 1035: header 12 byte, question, answer; **giải nén tên** (con trỏ 0xC0)
//! có phát hiện vòng lặp.
//!
//! # Phạm vi (KT-260, N1 phần đầu)
//!
//! Chỉ đủ cho **một** câu hỏi `A`/`IN` và **một** reply: dựng query, parse reply, lấy bản ghi `A`
//! đầu tiên. Không resolver đầy đủ, không đệ quy, không DNSSEC, không `AAAA`/`CNAME` (bản ghi lạ
//! được **bỏ qua** khi tìm `A`, nhưng vẫn kiểm độ dài để không đọc ngoài message).
//!
//! # Bất biến
//!
//! - `#![no_std]`, `#![deny(unsafe_code)]`, không cấp phát: mọi thứ làm việc trên lát byte có sẵn;
//!   [`Name`] sở hữu tối đa [`DNS_MAX_NAME_LEN`] byte dạng **có dấu chấm** (`example.com`).
//! - **Fail-closed**: cắt cụt, label > 63 byte, tên > 255 byte, con trỏ ra ngoài message, con trỏ
//!   **vòng lặp**, `RDLENGTH` vượt message hoặc khác 4 với bản ghi `A`, `qdcount=0` khi cần reply,
//!   `QR=0`, `id` lệch — đều trả [`DnsError`], không đọc rác.
//! - Con trỏ nén chỉ được theo tối đa [`DNS_MAX_POINTER_JUMPS`] lần: một vòng lặp thuần con trỏ
//!   (không tiêu thụ byte nào) vẫn bị chặn, không treo.
//! - Mã lỗi in qua [`DnsError::name`] (D2-102: không in số trần trong evidence).

use core::fmt;

use crate::addr::Ipv4Addr;

/// Độ dài header DNS.
pub const DNS_HEADER_LEN: usize = 12;
/// Độ dài tối đa của một tên (dạng wire, RFC 1035 §2.3.4).
pub const DNS_MAX_NAME_LEN: usize = 255;
/// Độ dài tối đa của một label (RFC 1035 §2.3.4).
pub const DNS_MAX_LABEL_LEN: usize = 63;
/// Số lần theo con trỏ nén tối đa trước khi kết luận **vòng lặp**.
pub const DNS_MAX_POINTER_JUMPS: usize = 64;

/// `TYPE` của bản ghi địa chỉ IPv4.
pub const TYPE_A: u16 = 1;
/// `CLASS` của Internet.
pub const CLASS_IN: u16 = 1;

/// Bit `QR` trong `flags` (1 = response).
pub const FLAG_QR: u16 = 0x8000;
/// Mặt nạ `opcode` trong `flags`.
pub const FLAG_OPCODE_MASK: u16 = 0x7800;
/// Bit `AA` (authoritative answer).
pub const FLAG_AA: u16 = 0x0400;
/// Bit `TC` (truncated).
pub const FLAG_TC: u16 = 0x0200;
/// Bit `RD` (recursion desired).
pub const FLAG_RD: u16 = 0x0100;
/// Bit `RA` (recursion available).
pub const FLAG_RA: u16 = 0x0080;
/// Mặt nạ `rcode` trong `flags`.
pub const FLAG_RCODE_MASK: u16 = 0x000f;

/// `rcode` — không lỗi.
pub const RCODE_NOERROR: u8 = 0;
/// `rcode` — tên không tồn tại.
pub const RCODE_NXDOMAIN: u8 = 3;

/// Lỗi của codec DNS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DnsError {
    /// Lát byte ngắn hơn header/câu hỏi/bản ghi.
    Truncated,
    /// Label rỗng, label > 63 byte, hoặc tên > 255 byte.
    BadName,
    /// Con trỏ nén trỏ ra ngoài message.
    BadPointer,
    /// Con trỏ nén tạo vòng lặp.
    PointerLoop,
    /// `qdcount` khác 1 khi cần một reply cho đúng câu hỏi của ta.
    BadCount,
    /// `RDLENGTH` vượt message, hoặc khác 4 với bản ghi `A`.
    BadRdlength,
    /// `QR=0` khi đang chờ response.
    NotResponse,
    /// `id` của reply khác `id` của query.
    BadId,
    /// Bộ đệm đích không đủ chỗ.
    BufferTooSmall,
}

impl DnsError {
    /// Tên ổn định để in trong evidence.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Truncated => "dns-truncated",
            Self::BadName => "dns-bad-name",
            Self::BadPointer => "dns-bad-pointer",
            Self::PointerLoop => "dns-pointer-loop",
            Self::BadCount => "dns-bad-count",
            Self::BadRdlength => "dns-bad-rdlength",
            Self::NotResponse => "dns-not-response",
            Self::BadId => "dns-bad-id",
            Self::BufferTooSmall => "dns-buffer-too-small",
        }
    }
}

/// Header DNS 12 byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DnsHeader {
    /// Transaction id.
    pub id: u16,
    /// `flags` (QR/opcode/AA/TC/RD/RA/Z/rcode).
    pub flags: u16,
    /// Số câu hỏi.
    pub qdcount: u16,
    /// Số answer.
    pub ancount: u16,
    /// Số authority record.
    pub nscount: u16,
    /// Số additional record.
    pub arcount: u16,
}

impl DnsHeader {
    /// Header của một **query** một câu hỏi: `RD=1`, `qdcount=1`, các đếm khác 0.
    pub const fn query(id: u16) -> Self {
        Self {
            id,
            flags: FLAG_RD,
            qdcount: 1,
            ancount: 0,
            nscount: 0,
            arcount: 0,
        }
    }

    /// Ghi header 12 byte vào `out`; trả số byte đã ghi.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, DnsError> {
        if out.len() < DNS_HEADER_LEN {
            return Err(DnsError::BufferTooSmall);
        }
        out[0..2].copy_from_slice(&self.id.to_be_bytes());
        out[2..4].copy_from_slice(&self.flags.to_be_bytes());
        out[4..6].copy_from_slice(&self.qdcount.to_be_bytes());
        out[6..8].copy_from_slice(&self.ancount.to_be_bytes());
        out[8..10].copy_from_slice(&self.nscount.to_be_bytes());
        out[10..12].copy_from_slice(&self.arcount.to_be_bytes());
        Ok(DNS_HEADER_LEN)
    }

    /// Đọc header từ đầu `buf`; đòi ít nhất 12 byte.
    pub fn parse(buf: &[u8]) -> Result<Self, DnsError> {
        if buf.len() < DNS_HEADER_LEN {
            return Err(DnsError::Truncated);
        }
        Ok(Self {
            id: u16::from_be_bytes([buf[0], buf[1]]),
            flags: u16::from_be_bytes([buf[2], buf[3]]),
            qdcount: u16::from_be_bytes([buf[4], buf[5]]),
            ancount: u16::from_be_bytes([buf[6], buf[7]]),
            nscount: u16::from_be_bytes([buf[8], buf[9]]),
            arcount: u16::from_be_bytes([buf[10], buf[11]]),
        })
    }

    /// `true` ⇔ `QR=1` (response).
    pub const fn is_response(&self) -> bool {
        self.flags & FLAG_QR != 0
    }

    /// `opcode` (4 bit).
    pub const fn opcode(&self) -> u8 {
        ((self.flags & FLAG_OPCODE_MASK) >> 11) as u8
    }

    /// `true` ⇔ `RD=1`.
    pub const fn rd(&self) -> bool {
        self.flags & FLAG_RD != 0
    }

    /// `true` ⇔ `RA=1`.
    pub const fn ra(&self) -> bool {
        self.flags & FLAG_RA != 0
    }

    /// `true` ⇔ `TC=1`.
    pub const fn tc(&self) -> bool {
        self.flags & FLAG_TC != 0
    }

    /// `rcode` (4 bit).
    pub const fn rcode(&self) -> u8 {
        (self.flags & FLAG_RCODE_MASK) as u8
    }
}

/// Tên DNS đã giải nén, dạng **có dấu chấm** (`example.com`, không có dấu chấm cuối).
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Name {
    bytes: [u8; DNS_MAX_NAME_LEN],
    len: usize,
}

impl Name {
    /// Dựng từ tên dạng có dấu chấm; từ chối tên rỗng, label rỗng, label > 63 byte, tên > 255 byte
    /// (dạng wire).
    pub fn from_dotted(name: &[u8]) -> Result<Self, DnsError> {
        if name.is_empty() || name.len() > DNS_MAX_NAME_LEN {
            return Err(DnsError::BadName);
        }
        // Dấu chấm cuối tạo label rỗng — không nhận (tên trong crate này không có dấu chấm cuối).
        if name[name.len() - 1] == b'.' {
            return Err(DnsError::BadName);
        }
        let mut wire = 1usize; // byte root
        let mut start = 0usize;
        while start < name.len() {
            let end = match name[start..].iter().position(|&b| b == b'.') {
                Some(index) => start + index,
                None => name.len(),
            };
            let label_len = end - start;
            if label_len == 0 || label_len > DNS_MAX_LABEL_LEN {
                return Err(DnsError::BadName);
            }
            wire += 1 + label_len;
            if wire > DNS_MAX_NAME_LEN {
                return Err(DnsError::BadName);
            }
            start = end + 1;
        }
        let mut bytes = [0u8; DNS_MAX_NAME_LEN];
        bytes[..name.len()].copy_from_slice(name);
        Ok(Self {
            bytes,
            len: name.len(),
        })
    }

    /// Tên dạng có dấu chấm.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }

    /// Độ dài tên dạng có dấu chấm.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Luôn `false`: [`Name::from_dotted`] từ chối tên rỗng.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// So sánh **không phân biệt hoa/thường** (DNS không phân biệt hoa/thường, RFC 1035 §2.3.3).
    pub fn eq_ignore_ascii_case(&self, other: &[u8]) -> bool {
        self.len == other.len()
            && self.bytes[..self.len]
                .iter()
                .zip(other.iter())
                .all(|(a, b)| a.eq_ignore_ascii_case(b))
    }

    /// Ghi tên dạng wire (label + byte root) vào `out`; trả số byte đã ghi.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, DnsError> {
        let mut pos = 0usize;
        let mut start = 0usize;
        while start < self.len {
            let end = match self.bytes[start..self.len].iter().position(|&b| b == b'.') {
                Some(index) => start + index,
                None => self.len,
            };
            let label = &self.bytes[start..end];
            if label.is_empty() || label.len() > DNS_MAX_LABEL_LEN {
                return Err(DnsError::BadName);
            }
            if out.len() < pos + 1 + label.len() {
                return Err(DnsError::BufferTooSmall);
            }
            out[pos] = label.len() as u8;
            out[pos + 1..pos + 1 + label.len()].copy_from_slice(label);
            pos += 1 + label.len();
            start = end + 1;
        }
        if out.len() < pos + 1 {
            return Err(DnsError::BufferTooSmall);
        }
        out[pos] = 0;
        Ok(pos + 1)
    }

    /// Giải tên tại `offset` trong `msg`, theo con trỏ nén; trả `(tên, offset sau tên)`.
    ///
    /// `offset sau tên` là vị trí **trong chuỗi gốc** (sau byte root, hoặc sau con trỏ đầu tiên) —
    /// đúng chỗ để đọc `QTYPE`/`QCLASS` hoặc `TYPE`/`CLASS` tiếp theo.
    pub fn parse_at(msg: &[u8], offset: usize) -> Result<(Self, usize), DnsError> {
        let mut bytes = [0u8; DNS_MAX_NAME_LEN];
        let mut len = 0usize;
        let mut wire = 1usize; // byte root
        let mut pos = offset;
        let mut next = None;
        let mut jumps = 0usize;
        loop {
            if pos >= msg.len() {
                return Err(DnsError::Truncated);
            }
            let byte = msg[pos];
            if byte == 0 {
                if next.is_none() {
                    next = Some(pos + 1);
                }
                break;
            }
            if byte & 0xc0 == 0xc0 {
                if pos + 1 >= msg.len() {
                    return Err(DnsError::Truncated);
                }
                let target = (usize::from(byte & 0x3f) << 8) | usize::from(msg[pos + 1]);
                if target >= msg.len() {
                    return Err(DnsError::BadPointer);
                }
                if next.is_none() {
                    next = Some(pos + 2);
                }
                jumps += 1;
                if jumps > DNS_MAX_POINTER_JUMPS {
                    return Err(DnsError::PointerLoop);
                }
                pos = target;
                continue;
            }
            if byte & 0xc0 != 0 {
                return Err(DnsError::BadName);
            }
            let label_len = usize::from(byte);
            if label_len > DNS_MAX_LABEL_LEN {
                return Err(DnsError::BadName);
            }
            if pos + 1 + label_len > msg.len() {
                return Err(DnsError::Truncated);
            }
            wire += 1 + label_len;
            if wire > DNS_MAX_NAME_LEN {
                return Err(DnsError::BadName);
            }
            if len > 0 {
                if len + 1 > DNS_MAX_NAME_LEN {
                    return Err(DnsError::BadName);
                }
                bytes[len] = b'.';
                len += 1;
            }
            if len + label_len > DNS_MAX_NAME_LEN {
                return Err(DnsError::BadName);
            }
            bytes[len..len + label_len].copy_from_slice(&msg[pos + 1..pos + 1 + label_len]);
            len += label_len;
            pos += 1 + label_len;
        }
        Ok((Self { bytes, len }, next.unwrap_or(pos.saturating_add(1))))
    }
}

impl fmt::Debug for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Name").field(&self.as_bytes()).finish()
    }
}

/// Một câu hỏi DNS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DnsQuestion {
    /// Tên được hỏi.
    pub name: Name,
    /// `QTYPE` (ví dụ [`TYPE_A`]).
    pub qtype: u16,
    /// `QCLASS` (ví dụ [`CLASS_IN`]).
    pub qclass: u16,
}

impl DnsQuestion {
    /// Ghi câu hỏi (tên + `QTYPE` + `QCLASS`) vào `out`; trả số byte đã ghi.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, DnsError> {
        let name_len = self.name.encode(out)?;
        if out.len() < name_len + 4 {
            return Err(DnsError::BufferTooSmall);
        }
        out[name_len..name_len + 2].copy_from_slice(&self.qtype.to_be_bytes());
        out[name_len + 2..name_len + 4].copy_from_slice(&self.qclass.to_be_bytes());
        Ok(name_len + 4)
    }

    /// Đọc câu hỏi tại `offset`; trả `(câu hỏi, offset sau câu hỏi)`.
    pub fn parse(msg: &[u8], offset: usize) -> Result<(Self, usize), DnsError> {
        let (name, mut pos) = Name::parse_at(msg, offset)?;
        if pos + 4 > msg.len() {
            return Err(DnsError::Truncated);
        }
        let qtype = u16::from_be_bytes([msg[pos], msg[pos + 1]]);
        let qclass = u16::from_be_bytes([msg[pos + 2], msg[pos + 3]]);
        pos += 4;
        Ok((
            Self {
                name,
                qtype,
                qclass,
            },
            pos,
        ))
    }
}

/// Truy vấn DNS một câu hỏi (header + question).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DnsQuery {
    /// Header (`RD=1`, `qdcount=1`).
    pub header: DnsHeader,
    /// Câu hỏi.
    pub question: DnsQuestion,
}

impl DnsQuery {
    /// Dựng query `A`/`IN` cho `name` với `id` cho trước.
    pub fn new(id: u16, name: &[u8], qtype: u16) -> Result<Self, DnsError> {
        Ok(Self {
            header: DnsHeader::query(id),
            question: DnsQuestion {
                name: Name::from_dotted(name)?,
                qtype,
                qclass: CLASS_IN,
            },
        })
    }

    /// Ghi query hoàn chỉnh vào `out`; trả tổng số byte.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, DnsError> {
        let header_len = self.header.encode(out)?;
        let question_len = self.question.encode(&mut out[header_len..])?;
        Ok(header_len + question_len)
    }
}

/// Bản ghi `A` (địa chỉ IPv4 + TTL).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ARecord {
    /// Địa chỉ IPv4.
    pub addr: Ipv4Addr,
    /// TTL (giây).
    pub ttl: u32,
}

/// Thông điệp DNS đã parse (giữ lát byte gốc; mọi getter đọc lại trên đó).
#[derive(Clone, Copy, Debug)]
pub struct DnsMessage<'a> {
    buf: &'a [u8],
    header: DnsHeader,
    /// Offset bắt đầu phần answer (ngay sau question section).
    answer_start: usize,
}

impl<'a> DnsMessage<'a> {
    /// Parse **toàn bộ** message: header, `qdcount` câu hỏi, rồi `ancount+nscount+arcount` bản ghi.
    ///
    /// Mọi bản ghi (kể cả authority/additional) đều được kiểm `RDLENGTH` nằm trong message — nhờ
    /// vậy getter không bao giờ đọc ngoài `buf`.
    pub fn parse(buf: &'a [u8]) -> Result<Self, DnsError> {
        let header = DnsHeader::parse(buf)?;
        let mut pos = DNS_HEADER_LEN;
        for _ in 0..header.qdcount {
            let (_, next) = DnsQuestion::parse(buf, pos)?;
            pos = next;
        }
        let answer_start = pos;
        let records =
            usize::from(header.ancount) + usize::from(header.nscount) + usize::from(header.arcount);
        for _ in 0..records {
            let (_, next) = Name::parse_at(buf, pos)?;
            pos = next;
            if pos + 10 > buf.len() {
                return Err(DnsError::Truncated);
            }
            let rdlength = usize::from(u16::from_be_bytes([buf[pos + 8], buf[pos + 9]]));
            pos += 10;
            if pos + rdlength > buf.len() {
                return Err(DnsError::BadRdlength);
            }
            pos += rdlength;
        }
        Ok(Self {
            buf,
            header,
            answer_start,
        })
    }

    /// Parse một **response** cho query `id`: đòi `QR=1`, `id` khớp, `qdcount=1`.
    pub fn parse_response(buf: &'a [u8], id: u16) -> Result<Self, DnsError> {
        let message = Self::parse(buf)?;
        if !message.header.is_response() {
            return Err(DnsError::NotResponse);
        }
        if message.header.id != id {
            return Err(DnsError::BadId);
        }
        if message.header.qdcount != 1 {
            return Err(DnsError::BadCount);
        }
        Ok(message)
    }

    /// Header đã parse.
    pub fn header(&self) -> DnsHeader {
        self.header
    }

    /// Câu hỏi đầu tiên; `None` khi `qdcount=0`.
    pub fn question(&self) -> Result<Option<DnsQuestion>, DnsError> {
        if self.header.qdcount == 0 {
            return Ok(None);
        }
        let (question, _) = DnsQuestion::parse(self.buf, DNS_HEADER_LEN)?;
        Ok(Some(question))
    }

    /// Bản ghi `A`/`IN` **đầu tiên** trong answer section; `None` nếu không có.
    ///
    /// Bản ghi loại khác (ví dụ `CNAME`) được bỏ qua nhưng vẫn kiểm `RDLENGTH`; bản ghi `A` có
    /// `RDLENGTH` khác 4 là **lỗi** (không đoán địa chỉ từ dữ liệu sai độ dài).
    pub fn first_a(&self) -> Result<Option<ARecord>, DnsError> {
        let mut pos = self.answer_start;
        for _ in 0..self.header.ancount {
            let (_, next) = Name::parse_at(self.buf, pos)?;
            pos = next;
            if pos + 10 > self.buf.len() {
                return Err(DnsError::Truncated);
            }
            let typ = u16::from_be_bytes([self.buf[pos], self.buf[pos + 1]]);
            let class = u16::from_be_bytes([self.buf[pos + 2], self.buf[pos + 3]]);
            let ttl = u32::from_be_bytes([
                self.buf[pos + 4],
                self.buf[pos + 5],
                self.buf[pos + 6],
                self.buf[pos + 7],
            ]);
            let rdlength = usize::from(u16::from_be_bytes([self.buf[pos + 8], self.buf[pos + 9]]));
            pos += 10;
            if pos + rdlength > self.buf.len() {
                return Err(DnsError::BadRdlength);
            }
            if typ == TYPE_A {
                if rdlength != 4 {
                    return Err(DnsError::BadRdlength);
                }
                if class == CLASS_IN {
                    return Ok(Some(ARecord {
                        addr: Ipv4Addr([
                            self.buf[pos],
                            self.buf[pos + 1],
                            self.buf[pos + 2],
                            self.buf[pos + 3],
                        ]),
                        ttl,
                    }));
                }
            }
            pos += rdlength;
        }
        Ok(None)
    }
}

// ================================================================================
// KT-294 — **thử lại** một truy vấn DNS (RFC 1035 §4.2.1)
//
// Soak aarch64 vòng 14 bắt `# net dns: elapsed_ms=3000 err=Timeout` trong khi TCP cùng boot
// **xanh**: 3000 ms là **outlier** của slirp dưới tải (p50 = 31 ms, p99 = 150 ms, max thành công
// = 286 ms), không phải ngưỡng quá chặt. RFC 1035 §4.2.1: "the resolver should retry" — driver
// cũ chỉ gửi **một** truy vấn nên mọi outlier thành lỗi boot. Chính sách ở đây là **thuần** (test
// host, D2-05) và **dùng chung** cho hai kiến trúc: không có bản sao lệch nhau.
// ================================================================================

/// Số lần thử tối đa của **một** truy vấn DNS.
///
/// **Hai** lần, mỗi lần có ngân sách timeout **riêng** (3 s — không đổi, không nới). Đây là ngữ
/// nghĩa giao thức DNS (RFC 1035 §4.2.1), **không** phải nới ngưỡng thời gian.
pub const DNS_MAX_ATTEMPTS: u8 = 2;

/// Chính sách **thử lại** của một truy vấn DNS: tối đa [`DNS_MAX_ATTEMPTS`] lần, **cùng `id`**.
///
/// Tạo **mới** cho mỗi truy vấn; driver ghi nhận từng lần gửi rồi hỏi [`DnsRetry::should_retry`].
/// Bộ đếm **bão hoà** ở trần nên "retransmit vô hạn" không biểu diễn được qua API này.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DnsRetry {
    attempts: u8,
}

impl DnsRetry {
    /// Chưa gửi lần nào.
    pub const fn new() -> Self {
        Self { attempts: 0 }
    }

    /// Số lần đã gửi (`0..=`[`DNS_MAX_ATTEMPTS`]).
    pub const fn attempts(&self) -> u8 {
        self.attempts
    }

    /// Ghi nhận **một** lần gửi; trả số thứ tự lần gửi (`1..=`[`DNS_MAX_ATTEMPTS`]).
    ///
    /// Bão hoà ở [`DNS_MAX_ATTEMPTS`]: gọi thêm không làm bộ đếm vượt trần.
    pub fn record_attempt(&mut self) -> u8 {
        self.attempts = self.attempts.saturating_add(1).min(DNS_MAX_ATTEMPTS);
        self.attempts
    }

    /// `true` ⇔ lần vừa rồi **timeout** và còn lượt ⇒ gửi lại **đúng** message cũ (cùng `id`).
    ///
    /// Reply hỏng (`BadFrame`, `id` lệch, parse lỗi) **không** thử lại: đó là lỗi giao thức, thử
    /// lại chỉ che lỗi.
    pub const fn should_retry(&self, timed_out: bool) -> bool {
        timed_out && self.attempts < DNS_MAX_ATTEMPTS
    }

    /// `true` ⇔ đã dùng hết lượt.
    pub const fn exhausted(&self) -> bool {
        self.attempts >= DNS_MAX_ATTEMPTS
    }
}
