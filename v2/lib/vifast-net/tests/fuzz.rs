// SPDX-License-Identifier: MIT OR Apache-2.0
//! Fuzz **tất định** cho các **parser** của `vifast-net` — KT-418 (ưu tiên 3).
//!
//! # Vì sao fuzz parser mạng là ca kiểm an ninh, không phải "cho vui"
//!
//! Mọi byte của một frame đến từ **ngoài** kernel (thiết bị virtio-net, hoặc mạng thật). Một
//! parser panic/đọc ngoài mảng là một đường **từ chối dịch vụ** (kernel chết vì một gói rác), và
//! một parser "đọc được nhưng trả về giá trị vô lý" là đường **giả mạo** (ARP/DHCP/DNS đầu độc).
//! Vì vậy fuzz ở đây kiểm **ba** bất biến trên **mọi** chuỗi byte, không chỉ chuỗi hợp lệ:
//!
//! 1. **Không panic** — kể cả `buf` rỗng, 1 byte, dài hơn frame, hoặc toàn `0xFF`.
//! 2. **Round-trip idempotent** — parse thành công ⇒ `encode` lại ⇒ parse lại cho **cùng** kết
//!    quả. Đây là bất biến mạnh: nó bắt "parse chấp nhận dữ liệu mà encode không tái tạo được"
//!    (dấu hiệu của trường bị bỏ qua hoặc độ dài không nhất quán).
//! 3. **Kiểm chặt hơn không bao giờ lỏng hơn** — `parse(buf, true)` thành công ⇒ `parse(buf,
//!    false)` cũng phải thành công (checksum chỉ **thêm** điều kiện).
//!
//! # Tất định, không dependency
//!
//! RNG tự viết (xorshift64* — **không** thêm crate ngoài, D2-21). Cùng seed ⇒ cùng chuỗi ⇒ cùng
//! kết quả trên mọi máy, nên ca kiểm tái lập được và **không** phải `#[ignore]`.
//!
//! Số chuỗi: **20.000** (bản đầy đủ) — dưới Miri **200** (bản rút gọn, cùng lý do như
//! `vifast-objmem/tests/fuzz.rs`: Miri chậm ~100×).

use vifast_net::addr::Ipv4Addr;
use vifast_net::arp::ArpPacket;
use vifast_net::dhcp::DhcpMessage;
use vifast_net::dns::DnsMessage;
use vifast_net::eth::EthHeader;
use vifast_net::frame::EthernetFrame;
use vifast_net::icmp::IcmpEcho;
use vifast_net::ipv4::Ipv4Header;
use vifast_net::tcp::TcpHeader;
use vifast_net::udp::UdpHeader;

/// Số chuỗi của bản đầy đủ (stable).
const CHUOI_STABLE: u64 = 20_000;
/// Số chuỗi của bản rút gọn dưới Miri.
const CHUOI_MIRI: u64 = 200;
/// Độ dài tối đa của một buffer fuzz (đủ vượt mọi header + payload điển hình).
const DAI_TOI_DA: usize = 192;
/// Kích thước bộ đệm `encode` — phải chứa **cả** payload của buffer fuzz dài nhất
/// (`DAI_TOI_DA * 4` ở ca biên) ⇒ 1024 là dư, và chính việc cấp đủ lớn là điều kiện để phép
/// round-trip đo được `encode`, không đo `BufferTooSmall`.
const OUT_TOI_DA: usize = 1024;

/// RNG tất định tự viết — xorshift64* (không dependency ngoài, D2-21).
struct Rng(u64);

impl Rng {
    const fn new(seed: u64) -> Self {
        // Seed 0 là điểm bất động của xorshift ⇒ ép khác 0.
        Self(if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        })
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn next_u8(&mut self) -> u8 {
        (self.next_u64() >> 24) as u8
    }

    fn duoi(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }
}

/// Số chuỗi theo cấu hình build: dưới Miri dùng bản rút gọn.
fn so_chuoi() -> u64 {
    if cfg!(miri) {
        CHUOI_MIRI
    } else {
        CHUOI_STABLE
    }
}

/// Sinh một buffer: độ dài ngẫu nhiên 0..=`DAI_TOI_DA`, nội dung ngẫu nhiên.
///
/// Một phần ba số lần "mồi" các byte đầu bằng giá trị **hợp lệ** của header tương ứng để fuzz đi
/// sâu vào nhánh parse thay vì thoát ngay ở byte đầu — fuzz thuần ngẫu nhiên gần như luôn trượt
/// `BadVersion`/`BadHtype` và không chạm phần thân.
fn sinh_buf(rng: &mut Rng, out: &mut [u8; DAI_TOI_DA]) -> usize {
    let len = rng.duoi(DAI_TOI_DA + 1);
    for byte in out.iter_mut().take(len) {
        *byte = rng.next_u8();
    }
    // Mồi: 1/3 số lần đặt các byte "cổng vào" thành giá trị hợp lệ.
    if len >= 4 && rng.next_u64().is_multiple_of(3) {
        match rng.next_u64() % 4 {
            // IPv4: version 4, IHL 5.
            0 => out[0] = 0x45,
            // ARP: htype=1, ptype=0x0800.
            1 => {
                out[0] = 0x00;
                out[1] = 0x01;
                out[2] = 0x08;
                out[3] = 0x00;
            }
            // TCP: data offset 5.
            2 => out[12] = 0x50,
            // UDP: length = len (đúng bằng buffer).
            _ => {
                let l = len as u16;
                out[4..6].copy_from_slice(&l.to_be_bytes());
            }
        }
    }
    len
}

/// Hai địa chỉ IPv4 cố định dùng cho pseudo-header của UDP/TCP.
const SRC: Ipv4Addr = Ipv4Addr::new(10, 0, 2, 15);
const DST: Ipv4Addr = Ipv4Addr::new(10, 0, 2, 2);

/// Fuzz một buffer qua **mọi** parser; trả `true` ⇔ mọi bất biến giữ.
fn fuzz_mot_buf(buf: &[u8]) -> u32 {
    let mut ok = 0u32;

    // ---- Ethernet header + frame.
    if let Ok(h) = EthHeader::parse(buf) {
        let mut out = [0u8; 14];
        if let Ok(n) = h.encode(&mut out) {
            if !(n == 14 && EthHeader::parse(&out[..n]).map(|r| r == h).unwrap_or(false)) {
                ok |= 1;
            }
        } else {
            ok |= 1 << 31;
        }
    }
    if let Ok(f) = EthernetFrame::parse(buf) {
        let mut out = [0u8; OUT_TOI_DA];
        if let Ok(n) = f.encode(&mut out) {
            if !EthernetFrame::parse(&out[..n])
                .map(|r| r == f)
                .unwrap_or(false)
            {
                ok |= 2;
            }
        } else {
            ok |= 1 << 31;
        }
    }

    // ---- ARP.
    if let Ok(a) = ArpPacket::parse(buf) {
        let mut out = [0u8; 28];
        if let Ok(n) = a.encode(&mut out) {
            if !(n == 28 && ArpPacket::parse(&out[..n]).map(|r| r == a).unwrap_or(false)) {
                ok |= 4;
            }
        } else {
            ok |= 1 << 31;
        }
    }

    // ---- IPv4: kiểm chặt ⇒ kiểm lỏng cũng phải qua; round-trip giữ nguyên header.
    let ipv4_lax = Ipv4Header::parse(buf, false);
    if Ipv4Header::parse(buf, true).is_ok() && ipv4_lax.is_err() {
        ok |= 8;
    }
    if let Ok(p) = ipv4_lax {
        // Gói có option (`IHL > 5`): `Ipv4Header::encode` chỉ ghi 20 byte header nên **không** tái
        // tạo được — bỏ qua **riêng** phép round-trip này, các parser sau vẫn phải chạy.
        if p.header.total_len as usize == 20 + p.payload().len() {
            // `Ipv4Header::encode` chỉ ghi **20 byte header**; `parse` đòi **cả** payload theo
            // `total_len` ⇒ phải nối payload vào trước khi parse lại.
            let mut out = [0u8; OUT_TOI_DA];
            if let Ok(n) = p.header.encode(&mut out) {
                let end = n + p.payload().len();
                if end <= out.len() {
                    out[n..end].copy_from_slice(p.payload());
                }
                if !Ipv4Header::parse(&out[..end], true)
                    .map(|r| {
                        r.header.src == p.header.src
                            && r.header.dst == p.header.dst
                            && r.header.protocol == p.header.protocol
                            && r.payload() == p.payload()
                    })
                    .unwrap_or(false)
                {
                    ok |= 16;
                }
            } else {
                ok |= 1 << 31;
            }
        }
    }

    // ---- UDP.
    let udp_lax = UdpHeader::parse(SRC, DST, buf, false);
    if UdpHeader::parse(SRC, DST, buf, true).is_ok() && udp_lax.is_err() {
        ok |= 32;
    }
    if let Ok(d) = udp_lax {
        let mut out = [0u8; OUT_TOI_DA];
        if let Ok(n) = UdpHeader::encode(
            SRC,
            DST,
            d.header.src_port,
            d.header.dst_port,
            d.payload(),
            &mut out,
        ) {
            if !UdpHeader::parse(SRC, DST, &out[..n], true)
                .map(|r| {
                    r.header.src_port == d.header.src_port
                        && r.header.dst_port == d.header.dst_port
                        && r.payload() == d.payload()
                })
                .unwrap_or(false)
            {
                ok |= 64;
            }
        } else {
            ok |= 1 << 31;
        }
    }

    // ---- TCP.
    let tcp_lax = TcpHeader::parse(SRC, DST, buf, false);
    if TcpHeader::parse(SRC, DST, buf, true).is_ok() && tcp_lax.is_err() {
        ok |= 128;
    }
    if let Ok(s) = tcp_lax {
        // Cùng lý do: `TcpHeader::encode` chỉ hỗ trợ `data_offset = 5` (không option).
        if s.header.data_offset == 5 {
            let mut out = [0u8; OUT_TOI_DA];
            if let Ok(n) = s.header.encode(SRC, DST, s.payload(), &mut out) {
                if !TcpHeader::parse(SRC, DST, &out[..n], true)
                    .map(|r| {
                        r.header.src_port == s.header.src_port
                            && r.header.dst_port == s.header.dst_port
                            && r.header.seq == s.header.seq
                            && r.header.ack == s.header.ack
                            && r.header.flags == s.header.flags
                            && r.payload() == s.payload()
                    })
                    .unwrap_or(false)
                {
                    ok |= 256;
                }
            } else {
                ok |= 1 << 31;
            }
        }
    }

    // ---- ICMP (checksum bắt buộc ⇒ phần lớn buffer trượt; bất biến là **không panic**).
    let _ = IcmpEcho::parse(buf);

    // ---- DHCP: `as_bytes()` là **cùng** vùng đã parse ⇒ parse lại phải thành công.
    if let Ok(m) = DhcpMessage::parse(buf) {
        if DhcpMessage::parse(m.as_bytes()).is_err() {
            ok |= 512;
        }
    }

    // ---- DNS (message + response): bất biến là **không panic** và không đọc ngoài `buf`.
    let _ = DnsMessage::parse(buf);
    let _ = DnsMessage::parse_response(buf, 0x1234);

    ok
}

#[test]
fn fuzz_parser_khong_panic_va_round_trip() {
    let mut rng = Rng::new(0x4B54_3431_385F_4E45); // "KT418_NE"
    let mut buf = [0u8; DAI_TOI_DA];
    let n = so_chuoi();
    let mut hong = 0u64;
    let mut dau_tien = None;
    for i in 0..n {
        let len = sinh_buf(&mut rng, &mut buf);
        if fuzz_mot_buf(&buf[..len]) != 0 {
            hong += 1;
            if dau_tien.is_none() {
                dau_tien = Some((i, len, fuzz_mot_buf(&buf[..len]), buf[..len].to_vec()));
            }
        }
    }
    assert_eq!(
        hong, 0,
        "fuzz vifast-net: {hong}/{n} chuỗi vi phạm bất biến; chuỗi đầu tiên: {dau_tien:?}"
    );
}

#[test]
fn fuzz_buf_dac_biet() {
    // Các buffer biên mà fuzz ngẫu nhiên hiếm khi sinh ra: rỗng, 1 byte, toàn 0x00/0xFF, đúng
    // độ dài header của từng giao thức, và dài hơn `DAI_TOI_DA`.
    let mut dai = vec![0u8; DAI_TOI_DA * 4];
    for (i, byte) in dai.iter_mut().enumerate() {
        *byte = (i & 0xFF) as u8;
    }
    let dai_ff = vec![0xFFu8; DAI_TOI_DA * 4];
    let dai_00 = vec![0x00u8; DAI_TOI_DA * 4];
    let mut ca: Vec<&[u8]> = vec![
        &[],
        &[0x00],
        &[0xFF],
        &[0x45],
        &[0x45, 0x00],
        &[0x00; 14],
        &[0xFF; 14],
        &[0x00; 20],
        &[0xFF; 20],
        &[0x00; 28],
        &[0xFF; 28],
        &[0x00; 42],
        &[0xFF; 42],
        &[0x00; 240],
        &[0xFF; 240],
        &dai,
        &dai_ff,
        &dai_00,
    ];
    // Thêm mọi tiền tố của một buffer "trông hợp lệ" — bắt lỗi cắt ngắn ở từng offset.
    let mut hop_le = [0u8; 64];
    hop_le[0] = 0x45; // IPv4, IHL 5
    hop_le[2..4].copy_from_slice(&40u16.to_be_bytes()); // total length
    hop_le[8] = 64; // TTL
    hop_le[9] = 17; // UDP
    for end in 0..=hop_le.len() {
        ca.push(&hop_le[..end]);
    }
    for buf in ca {
        let mask = fuzz_mot_buf(buf);
        assert!(
            mask == 0,
            "buffer đặc biệt vi phạm bất biến (mask=0x{mask:x}): {buf:?}"
        );
    }
}
