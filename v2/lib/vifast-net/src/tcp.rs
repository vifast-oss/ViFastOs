// SPDX-License-Identifier: MIT OR Apache-2.0
//! TCP — header 20 byte (RFC 793) + **máy trạng thái client** thuần.
//!
//! # Phạm vi (KT-268, N1 phần 2)
//!
//! Đủ cho **một** kết nối client: mở (SYN → SYN+ACK), gửi/nhận dữ liệu trong cửa sổ, đóng chủ
//! động (FIN) và bị đóng (FIN/RST). **Không** có: option TCP (MSS/window scale/SACK/timestamp),
//! retransmit tự động theo RTO (driver quyết định), congestion control, TIME_WAIT 2MSL, đồng thời
//! mở (simultaneous open), half-close phức tạp.
//!
//! # Bất biến
//!
//! - `#![no_std]`, `#![deny(unsafe_code)]`, **không** cấp phát: mọi thứ làm việc trên lát byte có
//!   sẵn; [`Actions`] là danh sách **cỡ cố định** ([`MAX_ACTIONS`]).
//! - Checksum **có pseudo-header IPv4** (RFC 793 §3.1) qua [`crate::checksum::Checksum`] — cộng
//!   từng mảnh, không sao chép payload.
//! - **Fail-closed**: cắt cụt, `data offset < 5`, `data offset` khác 5 khi encode (không option),
//!   reserved bit khác 0, tổ hợp cờ vô nghĩa (SYN+FIN, SYN+RST, FIN+RST), checksum sai, `ack` vượt
//!   `snd_nxt`, sai cổng, dữ liệu sau FIN, cửa sổ 0 — đều trả [`TcpError`], không đoán.
//! - **Gói ngoài cửa sổ / trùng / lỗ hổng** không phải lỗi: bỏ dữ liệu và **ACK lại** (RFC 793
//!   §3.9) — người gọi thấy [`Action::Send`]`(`[`SendKind::Ack`]`)`, không thấy `Deliver`.
//! - Mã lỗi in qua [`TcpError::name`] (D2-102: không in số trần trong evidence).
//!
//! # Máy trạng thái
//!
//! ```text
//! Closed ──connect──▶ SynSent ──SYN+ACK──▶ Established ──build_fin──▶ FinWait1
//!                                              │  ▲                      │
//!                                     FIN peer │  │ data                 │ ACK FIN
//!                                              ▼  │                      ▼
//!                                           LastAck                    FinWait2
//!                                              │                          │
//!                                     ACK FIN  ▼                 FIN peer ▼
//!                                           Closed ◀──Close── TimeWait ◀───┘
//! ```
//!
//! `FinWait1` + FIN của peer **chưa** ACK FIN của ta ⇒ `Closing`; ACK tới ⇒ `TimeWait` + `Close`.
//! `Established` + FIN của peer ⇒ ACK + FIN của ta (`LastAck`); ACK tới ⇒ `Close`.
//!
//! [`Action::Close`] nghĩa là "kết nối đã ở trạng thái cuối, tầng trên giải phóng"; [`Action::Reset`]
//! nghĩa là "huỷ: gửi RST và báo lỗi". Không có bộ đếm thời gian trong crate này — driver quyết
//! định khi nào gọi [`TcpConnection::on_timeout`] (tối đa [`MAX_RETRANSMITS`] lần).

use crate::addr::Ipv4Addr;
use crate::checksum::Checksum;
use crate::ipv4::PROTO_TCP;

/// Độ dài header TCP tối thiểu (không option).
pub const TCP_HEADER_LEN: usize = 20;
/// `data offset` nhỏ nhất hợp lệ (5 word 32-bit).
pub const TCP_MIN_DATA_OFFSET: u8 = 5;
/// `data offset` lớn nhất biểu diễn được (4 bit).
pub const TCP_MAX_DATA_OFFSET: u8 = 15;
/// MSS mặc định khi không đàm phán option (RFC 879: 536; Ethernet 1500 − 40 = 1460).
pub const DEFAULT_MSS: usize = 1460;
/// Cửa sổ nhận mặc định quảng bá (không window scaling).
pub const DEFAULT_WINDOW: u16 = 4096;
/// Số lần **retransmit** tối đa trước khi huỷ (KT-268: "không retransmit quá 3 lần").
pub const MAX_RETRANSMITS: u8 = 3;
/// Số hành động tối đa một segment sinh ra (dữ liệu + FIN + ACK = 3; chừa một chỗ).
pub const MAX_ACTIONS: usize = 4;

/// Cờ `FIN`.
pub const FLAG_FIN: u8 = 0x01;
/// Cờ `SYN`.
pub const FLAG_SYN: u8 = 0x02;
/// Cờ `RST`.
pub const FLAG_RST: u8 = 0x04;
/// Cờ `PSH`.
pub const FLAG_PSH: u8 = 0x08;
/// Cờ `ACK`.
pub const FLAG_ACK: u8 = 0x10;
/// Cờ `URG`.
pub const FLAG_URG: u8 = 0x20;
/// Cờ `ECE`.
pub const FLAG_ECE: u8 = 0x40;
/// Cờ `CWR`.
pub const FLAG_CWR: u8 = 0x80;

/// Lỗi của codec và của máy trạng thái TCP.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcpError {
    /// Lát byte ngắn hơn header (hoặc ngắn hơn `data offset` khai báo).
    Truncated,
    /// `data offset < 5`, hoặc khác 5 khi encode (crate không hỗ trợ option).
    BadDataOffset,
    /// Checksum sai (khi bật kiểm).
    BadChecksum,
    /// Bit reserved (kể cả `NS`) khác 0 — RFC 793 §3.1.
    BadReserved,
    /// Tổ hợp cờ vô nghĩa: SYN+FIN, SYN+RST, FIN+RST.
    BadFlags,
    /// Bộ đệm đích không đủ chỗ.
    BufferTooSmall,
    /// Cổng nguồn/đích không khớp kết nối.
    WrongPort,
    /// `ack` vượt `snd_nxt` (ACK tương lai).
    BadAck,
    /// Segment không hợp lệ ở trạng thái hiện tại (ví dụ dữ liệu sau FIN).
    BadState,
    /// Không gửi được: cửa sổ của peer bằng 0.
    ZeroWindow,
    /// Không gửi được: dữ liệu dài hơn cửa sổ của peer.
    WindowTooSmall,
    /// Chưa `Established` nên không gửi/đóng được.
    NotEstablished,
    /// Payload rỗng hoặc dài hơn [`DEFAULT_MSS`].
    PayloadTooLarge,
}

impl TcpError {
    /// Tên ổn định để in trong evidence.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Truncated => "tcp-truncated",
            Self::BadDataOffset => "tcp-bad-data-offset",
            Self::BadChecksum => "tcp-bad-checksum",
            Self::BadReserved => "tcp-bad-reserved",
            Self::BadFlags => "tcp-bad-flags",
            Self::BufferTooSmall => "tcp-buffer-too-small",
            Self::WrongPort => "tcp-wrong-port",
            Self::BadAck => "tcp-bad-ack",
            Self::BadState => "tcp-bad-state",
            Self::ZeroWindow => "tcp-zero-window",
            Self::WindowTooSmall => "tcp-window-too-small",
            Self::NotEstablished => "tcp-not-established",
            Self::PayloadTooLarge => "tcp-payload-too-large",
        }
    }
}

/// Header TCP (không option).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TcpHeader {
    /// Cổng nguồn.
    pub src_port: u16,
    /// Cổng đích.
    pub dst_port: u16,
    /// Số thứ tự.
    pub seq: u32,
    /// Số xác nhận.
    pub ack: u32,
    /// `data offset` theo word 32-bit (≥ 5).
    pub data_offset: u8,
    /// Cờ (byte 13).
    pub flags: u8,
    /// Cửa sổ quảng bá.
    pub window: u16,
    /// Checksum (0 trước khi [`TcpHeader::encode`] tính).
    pub checksum: u16,
    /// Urgent pointer.
    pub urgent: u16,
}

/// Segment TCP đã parse: header + payload (đã cắt theo `data offset`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TcpSegment<'a> {
    /// Header đã parse.
    pub header: TcpHeader,
    payload: &'a [u8],
}

impl<'a> TcpSegment<'a> {
    /// Ghép header + payload có sẵn (dùng khi đã có segment trong bộ đệm).
    pub const fn new(header: TcpHeader, payload: &'a [u8]) -> Self {
        Self { header, payload }
    }

    /// Payload sau header.
    pub fn payload(&self) -> &'a [u8] {
        self.payload
    }
}

/// Checksum TCP trên pseudo-header IPv4 + segment. Khi **tính** thì trường checksum phải là 0;
/// khi **kiểm** thì giữ nguyên giá trị nhận được (kết quả đúng phải bằng 0).
fn checksum_v4(src: Ipv4Addr, dst: Ipv4Addr, tcp: &[u8]) -> u16 {
    let mut c = Checksum::new();
    c.add_bytes(&src.octets());
    c.add_bytes(&dst.octets());
    c.add_bytes(&[0, PROTO_TCP]);
    c.add_u16(tcp.len() as u16);
    c.add_bytes(tcp);
    c.finish()
}

impl TcpHeader {
    /// Header không option, checksum 0, urgent 0.
    pub const fn new(
        src_port: u16,
        dst_port: u16,
        seq: u32,
        ack: u32,
        flags: u8,
        window: u16,
    ) -> Self {
        Self {
            src_port,
            dst_port,
            seq,
            ack,
            data_offset: TCP_MIN_DATA_OFFSET,
            flags,
            window,
            checksum: 0,
            urgent: 0,
        }
    }

    /// Độ dài header theo byte (`data_offset * 4`).
    pub const fn header_len(&self) -> usize {
        self.data_offset as usize * 4
    }

    /// `true` nếu cờ `flag` bật.
    pub const fn has(&self, flag: u8) -> bool {
        self.flags & flag != 0
    }

    /// Ghi header + payload vào `out`, tính checksum có pseudo-header; trả số byte đã ghi.
    ///
    /// Chỉ hỗ trợ **không option** (`data_offset = 5`): header khác 5 trả
    /// [`TcpError::BadDataOffset`] thay vì ghi ra segment sai.
    pub fn encode(
        &self,
        src: Ipv4Addr,
        dst: Ipv4Addr,
        payload: &[u8],
        out: &mut [u8],
    ) -> Result<usize, TcpError> {
        if self.data_offset != TCP_MIN_DATA_OFFSET {
            return Err(TcpError::BadDataOffset);
        }
        let total = TCP_HEADER_LEN + payload.len();
        if out.len() < total {
            return Err(TcpError::BufferTooSmall);
        }
        if total > usize::from(u16::MAX) {
            return Err(TcpError::PayloadTooLarge);
        }
        out[0..2].copy_from_slice(&self.src_port.to_be_bytes());
        out[2..4].copy_from_slice(&self.dst_port.to_be_bytes());
        out[4..8].copy_from_slice(&self.seq.to_be_bytes());
        out[8..12].copy_from_slice(&self.ack.to_be_bytes());
        out[12] = self.data_offset << 4; // reserved + NS = 0
        out[13] = self.flags;
        out[14..16].copy_from_slice(&self.window.to_be_bytes());
        out[16] = 0;
        out[17] = 0;
        out[18..20].copy_from_slice(&self.urgent.to_be_bytes());
        out[TCP_HEADER_LEN..total].copy_from_slice(payload);
        let c = checksum_v4(src, dst, &out[..total]);
        // TCP: checksum tính ra 0 thì truyền 0xffff (0 là giá trị "không có checksum" không hợp lệ).
        let c = if c == 0 { 0xffff } else { c };
        out[16..18].copy_from_slice(&c.to_be_bytes());
        Ok(total)
    }

    /// Đọc segment từ `buf` (phải là **đúng** phần TCP, không lẫn padding của tầng dưới).
    ///
    /// Kiểm: độ dài ≥ 20, `data offset ≥ 5`, header nằm trong `buf`, reserved bit = 0, tổ hợp cờ
    /// hợp lệ, và (khi `verify_checksum`) checksum có pseudo-header.
    pub fn parse(
        src: Ipv4Addr,
        dst: Ipv4Addr,
        buf: &[u8],
        verify_checksum: bool,
    ) -> Result<TcpSegment<'_>, TcpError> {
        if buf.len() < TCP_HEADER_LEN {
            return Err(TcpError::Truncated);
        }
        let data_offset = buf[12] >> 4;
        if data_offset < TCP_MIN_DATA_OFFSET {
            return Err(TcpError::BadDataOffset);
        }
        let header_len = usize::from(data_offset) * 4;
        if buf.len() < header_len {
            return Err(TcpError::Truncated);
        }
        if buf[12] & 0x0f != 0 {
            return Err(TcpError::BadReserved);
        }
        let flags = buf[13];
        let syn_fin = FLAG_SYN | FLAG_FIN;
        let syn_rst = FLAG_SYN | FLAG_RST;
        let fin_rst = FLAG_FIN | FLAG_RST;
        if flags & syn_fin == syn_fin || flags & syn_rst == syn_rst || flags & fin_rst == fin_rst {
            return Err(TcpError::BadFlags);
        }
        if verify_checksum && checksum_v4(src, dst, buf) != 0 {
            return Err(TcpError::BadChecksum);
        }
        Ok(TcpSegment {
            header: Self {
                src_port: u16::from_be_bytes([buf[0], buf[1]]),
                dst_port: u16::from_be_bytes([buf[2], buf[3]]),
                seq: u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]),
                ack: u32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]),
                data_offset,
                flags,
                window: u16::from_be_bytes([buf[14], buf[15]]),
                checksum: u16::from_be_bytes([buf[16], buf[17]]),
                urgent: u16::from_be_bytes([buf[18], buf[19]]),
            },
            payload: &buf[header_len..],
        })
    }
}

/// Trạng thái kết nối TCP phía client.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// Chưa mở / đã đóng.
    Closed,
    /// Đã gửi SYN, chờ SYN+ACK.
    SynSent,
    /// Hai chiều mở, trao đổi dữ liệu được.
    Established,
    /// Ta đã gửi FIN, chờ ACK.
    FinWait1,
    /// FIN của ta đã được ACK, chờ FIN của peer.
    FinWait2,
    /// FIN của peer tới trước khi FIN của ta được ACK.
    Closing,
    /// Hai FIN đã trao đổi xong (không có bộ đếm 2MSL trong phạm vi này).
    TimeWait,
    /// FIN của peer đã nhận, FIN của ta đã gửi, chờ ACK cuối.
    LastAck,
}

/// Một đầu kết nối.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Endpoint {
    /// Địa chỉ IPv4.
    pub ip: Ipv4Addr,
    /// Cổng TCP.
    pub port: u16,
}

/// Loại segment cần gửi.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendKind {
    /// SYN (mở kết nối).
    Syn,
    /// ACK thuần (không payload).
    Ack,
    /// Dữ liệu (`ACK|PSH`).
    Data,
    /// FIN (`FIN|ACK`).
    Fin,
    /// RST (`RST|ACK`).
    Rst,
}

impl SendKind {
    /// Cờ TCP của segment loại này.
    pub const fn flags(self) -> u8 {
        match self {
            Self::Syn => FLAG_SYN,
            Self::Ack => FLAG_ACK,
            Self::Data => FLAG_ACK | FLAG_PSH,
            Self::Fin => FLAG_FIN | FLAG_ACK,
            Self::Rst => FLAG_RST | FLAG_ACK,
        }
    }
}

/// Hành động máy trạng thái trả cho tầng trên.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action<'a> {
    /// Gửi một segment (seq/ack do máy trạng thái đặt).
    Send(SendKind),
    /// Giao dữ liệu đã nhận cho tầng trên.
    Deliver(&'a [u8]),
    /// Kết nối đã ở trạng thái cuối — giải phóng.
    Close,
    /// Huỷ: gửi RST và báo lỗi.
    Reset,
}

/// Danh sách hành động **cỡ cố định** (không cấp phát).
#[derive(Clone, Copy, Debug)]
pub struct Actions<'a> {
    items: [Option<Action<'a>>; MAX_ACTIONS],
    len: usize,
}

impl<'a> Actions<'a> {
    /// Danh sách rỗng.
    pub const fn new() -> Self {
        Self {
            items: [None; MAX_ACTIONS],
            len: 0,
        }
    }

    /// Thêm một hành động; đầy thì bỏ (không xảy ra với [`MAX_ACTIONS`] = 4 — xem `debug_assert`).
    fn push(&mut self, action: Action<'a>) {
        debug_assert!(self.len < MAX_ACTIONS, "Actions đầy");
        if self.len < MAX_ACTIONS {
            self.items[self.len] = Some(action);
            self.len += 1;
        }
    }

    /// Số hành động.
    pub const fn len(&self) -> usize {
        self.len
    }

    /// `true` nếu không có hành động nào.
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Hành động thứ `index`.
    pub fn get(&self, index: usize) -> Option<Action<'a>> {
        if index < self.len {
            self.items[index]
        } else {
            None
        }
    }

    /// Duyệt các hành động theo thứ tự.
    pub fn iter(&self) -> impl Iterator<Item = Action<'a>> + '_ {
        self.items[..self.len].iter().filter_map(|item| *item)
    }
}

impl Default for Actions<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for Actions<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len && self.iter().eq(other.iter())
    }
}

impl Eq for Actions<'_> {}

/// Máy trạng thái một kết nối TCP phía client.
#[derive(Clone, Copy, Debug)]
pub struct TcpConnection {
    state: State,
    local: Endpoint,
    remote: Endpoint,
    iss: u32,
    snd_una: u32,
    snd_nxt: u32,
    snd_wnd: u16,
    irs: u32,
    rcv_nxt: u32,
    rcv_wnd: u16,
    fin_seq: u32,
    fin_sent: bool,
    fin_acked: bool,
    retries: u8,
}

impl TcpConnection {
    /// Mở kết nối: trạng thái [`State::SynSent`], `snd_nxt = iss + 1`.
    pub const fn connect(
        local_ip: Ipv4Addr,
        local_port: u16,
        remote_ip: Ipv4Addr,
        remote_port: u16,
        iss: u32,
    ) -> Self {
        Self {
            state: State::SynSent,
            local: Endpoint {
                ip: local_ip,
                port: local_port,
            },
            remote: Endpoint {
                ip: remote_ip,
                port: remote_port,
            },
            iss,
            snd_una: iss,
            snd_nxt: iss.wrapping_add(1),
            snd_wnd: 0,
            irs: 0,
            rcv_nxt: 0,
            rcv_wnd: DEFAULT_WINDOW,
            fin_seq: 0,
            fin_sent: false,
            fin_acked: false,
            retries: 0,
        }
    }

    /// Trạng thái hiện tại.
    pub const fn state(&self) -> State {
        self.state
    }

    /// Đầu ta.
    pub const fn local(&self) -> Endpoint {
        self.local
    }

    /// Đầu peer.
    pub const fn remote(&self) -> Endpoint {
        self.remote
    }

    /// ISS (số thứ tự đầu của ta).
    pub const fn iss(&self) -> u32 {
        self.iss
    }

    /// IRS (số thứ tự đầu của peer).
    pub const fn irs(&self) -> u32 {
        self.irs
    }

    /// Số thứ tự gửi chưa được ACK.
    pub const fn snd_una(&self) -> u32 {
        self.snd_una
    }

    /// Số thứ tự gửi kế tiếp.
    pub const fn snd_nxt(&self) -> u32 {
        self.snd_nxt
    }

    /// Cửa sổ gửi mà peer quảng bá.
    pub const fn snd_wnd(&self) -> u16 {
        self.snd_wnd
    }

    /// Số thứ tự nhận kế tiếp.
    pub const fn rcv_nxt(&self) -> u32 {
        self.rcv_nxt
    }

    /// Cửa sổ nhận ta quảng bá.
    pub const fn rcv_wnd(&self) -> u16 {
        self.rcv_wnd
    }

    /// Số thứ tự của FIN ta đã gửi (chỉ có nghĩa khi [`TcpConnection::fin_sent`]).
    pub const fn fin_seq(&self) -> u32 {
        self.fin_seq
    }

    /// `true` nếu FIN của ta đã gửi.
    pub const fn fin_sent(&self) -> bool {
        self.fin_sent
    }

    /// `true` nếu FIN của ta đã được ACK.
    pub const fn fin_acked(&self) -> bool {
        self.fin_acked
    }

    /// Số lần retransmit đã dùng trong bước hiện tại.
    pub const fn retries(&self) -> u8 {
        self.retries
    }

    /// Đổi cửa sổ nhận quảng bá (driver dùng để thu nhỏ cửa sổ; test dùng cho ca cửa sổ 0).
    pub fn set_rcv_window(&mut self, window: u16) {
        self.rcv_wnd = window;
    }

    /// `true` nếu `seq` nằm trong cửa sổ nhận `[rcv_nxt, rcv_nxt + rcv_wnd)`.
    fn seq_in_window(&self, seq: u32) -> bool {
        if self.rcv_wnd == 0 {
            return false;
        }
        seq.wrapping_sub(self.rcv_nxt) < u32::from(self.rcv_wnd)
    }

    /// Xử lý **một** segment từ peer; trả danh sách hành động theo thứ tự phải thi hành.
    ///
    /// Lỗi codec/trạng thái ⇒ [`TcpError`] và **không** đổi trạng thái (trừ RST/SYN bất ngờ, vốn
    /// là hành động [`Action::Reset`] chứ không phải lỗi).
    pub fn on_segment<'a>(&mut self, seg: &'a TcpSegment<'a>) -> Result<Actions<'a>, TcpError> {
        let header = seg.header;
        if header.src_port != self.remote.port || header.dst_port != self.local.port {
            return Err(TcpError::WrongPort);
        }
        let mut out = Actions::new();

        // ---- 1) RST: huỷ (RFC 793 §3.4). Ở SynSent chỉ nhận RST+ACK đúng số thứ tự.
        if header.has(FLAG_RST) {
            if self.state == State::SynSent && !(header.has(FLAG_ACK) && header.ack == self.snd_nxt)
            {
                return Ok(out);
            }
            self.state = State::Closed;
            out.push(Action::Reset);
            return Ok(out);
        }

        // ---- 2) SYN: chỉ hợp lệ khi đang SynSent và có ACK đúng (SYN+ACK).
        if header.has(FLAG_SYN) {
            // SYN+ACK **trùng** sau khi đã Established (peer retransmit vì ACK của ta tới muộn):
            // ACK lại, **không** reset — nếu reset, một lần retransmit SYN của ta sẽ giết kết nối.
            if self.state == State::Established
                && header.has(FLAG_ACK)
                && header.seq == self.irs
                && header.ack == self.snd_una
            {
                out.push(Action::Send(SendKind::Ack));
                return Ok(out);
            }
            if self.state != State::SynSent || !header.has(FLAG_ACK) {
                self.state = State::Closed;
                out.push(Action::Reset);
                return Ok(out);
            }
            if header.ack != self.snd_nxt {
                return Err(TcpError::BadAck);
            }
            self.irs = header.seq;
            self.rcv_nxt = header.seq.wrapping_add(1);
            self.snd_una = header.ack;
            self.snd_wnd = header.window;
            self.state = State::Established;
            self.retries = 0;
            out.push(Action::Send(SendKind::Ack));
            return Ok(out);
        }

        // ---- 3) ACK: kiểm và cập nhật; chuyển trạng thái do FIN của ta được ACK.
        if header.has(FLAG_ACK) {
            if header.ack > self.snd_nxt {
                return Err(TcpError::BadAck);
            }
            if header.ack > self.snd_una {
                self.snd_una = header.ack;
                self.retries = 0;
            }
            self.snd_wnd = header.window;
            if self.fin_sent && self.snd_una == self.snd_nxt {
                self.fin_acked = true;
            }
            match self.state {
                State::FinWait1 if self.fin_acked => self.state = State::FinWait2,
                State::Closing if self.fin_acked => {
                    self.state = State::TimeWait;
                    out.push(Action::Close);
                }
                State::LastAck if self.fin_acked => {
                    self.state = State::Closed;
                    out.push(Action::Close);
                }
                _ => {}
            }
        }

        // ---- 4) Dữ liệu / FIN.
        let payload = seg.payload();
        if payload.is_empty() && !header.has(FLAG_FIN) {
            return Ok(out);
        }
        if !matches!(
            self.state,
            State::Established | State::FinWait1 | State::FinWait2
        ) {
            return Err(TcpError::BadState);
        }
        if !self.seq_in_window(header.seq) || header.seq != self.rcv_nxt {
            // Ngoài cửa sổ / gói trùng / lỗ hổng: bỏ dữ liệu và ACK lại (RFC 793 §3.9).
            out.push(Action::Send(SendKind::Ack));
            return Ok(out);
        }
        if !payload.is_empty() {
            self.rcv_nxt = self.rcv_nxt.wrapping_add(payload.len() as u32);
            out.push(Action::Deliver(payload));
        }
        if header.has(FLAG_FIN) {
            self.rcv_nxt = self.rcv_nxt.wrapping_add(1);
            out.push(Action::Send(SendKind::Ack));
            match self.state {
                State::Established => {
                    // Peer đóng trước: ACK + FIN của ta ngay (client không có half-close).
                    self.fin_seq = self.snd_nxt;
                    self.snd_nxt = self.snd_nxt.wrapping_add(1);
                    self.fin_sent = true;
                    self.state = State::LastAck;
                    out.push(Action::Send(SendKind::Fin));
                }
                State::FinWait1 => {
                    if self.fin_acked {
                        self.state = State::TimeWait;
                        out.push(Action::Close);
                    } else {
                        self.state = State::Closing;
                    }
                }
                State::FinWait2 => {
                    self.state = State::TimeWait;
                    out.push(Action::Close);
                }
                _ => {}
            }
        } else if !payload.is_empty() {
            out.push(Action::Send(SendKind::Ack));
        }
        Ok(out)
    }

    /// Cấp số thứ tự cho một segment dữ liệu `len` byte và đẩy `snd_nxt`.
    ///
    /// Lỗi: chưa `Established`, `len` rỗng hoặc quá [`DEFAULT_MSS`], cửa sổ peer bằng 0 / quá hẹp.
    pub fn build_data(&mut self, len: usize) -> Result<u32, TcpError> {
        if self.state != State::Established {
            return Err(TcpError::NotEstablished);
        }
        if len == 0 || len > DEFAULT_MSS {
            return Err(TcpError::PayloadTooLarge);
        }
        if self.snd_wnd == 0 {
            return Err(TcpError::ZeroWindow);
        }
        if len > usize::from(self.snd_wnd) {
            return Err(TcpError::WindowTooSmall);
        }
        let seq = self.snd_nxt;
        self.snd_nxt = self.snd_nxt.wrapping_add(len as u32);
        Ok(seq)
    }

    /// Cấp số thứ tự cho FIN và chuyển `Established → FinWait1`.
    pub fn build_fin(&mut self) -> Result<u32, TcpError> {
        if self.state != State::Established {
            return Err(TcpError::NotEstablished);
        }
        let seq = self.snd_nxt;
        self.snd_nxt = self.snd_nxt.wrapping_add(1);
        self.fin_seq = seq;
        self.fin_sent = true;
        self.state = State::FinWait1;
        Ok(seq)
    }

    /// Hết hạn chờ: retransmit (tối đa [`MAX_RETRANSMITS`] lần) hoặc huỷ.
    ///
    /// Driver chỉ gọi khi đang chờ **một** bước cụ thể (SYN+ACK, ACK dữ liệu, ACK FIN); ở trạng
    /// thái không có gì để gửi lại thì trả [`Action::Close`] (chờ FIN của peer) hoặc
    /// [`Action::Reset`] (đã đóng).
    pub fn on_timeout(&mut self) -> Action<'static> {
        if self.retries >= MAX_RETRANSMITS {
            self.state = State::Closed;
            return Action::Reset;
        }
        self.retries += 1;
        match self.state {
            State::SynSent => Action::Send(SendKind::Syn),
            State::Established => Action::Send(SendKind::Data),
            State::FinWait1 | State::Closing | State::LastAck => Action::Send(SendKind::Fin),
            State::FinWait2 | State::TimeWait => Action::Close,
            State::Closed => Action::Reset,
        }
    }
}
