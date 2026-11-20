// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-268 — test host cho `tcp` (D2-05: viết trước, đỏ rồi mới xanh).
//!
//! Gồm ba nhóm:
//!
//! 1. **Codec** — header 20 byte encode/parse, checksum có pseudo-header IPv4, cờ;
//! 2. **Máy trạng thái** — `Closed → SynSent → Established → FinWait1 → FinWait2 → TimeWait`,
//!    nhánh `CloseWait/LastAck/Closing`, dữ liệu trong/ngoài cửa sổ, gói trùng, FIN, RST;
//! 3. **Ca âm** — checksum sai, `data offset < 5`, `data offset` khác 5 khi encode, cờ lạ
//!    (reserved bit, SYN+FIN), seq sai, ack sai, RST giữa pha, cửa sổ 0, sai cổng, dữ liệu sau
//!    FIN, quá MSS, retransmit quá 3 lần.

use vifast_net::addr::Ipv4Addr;
use vifast_net::checksum::Checksum;
use vifast_net::tcp::{
    Action, SendKind, State, TcpConnection, TcpError, TcpHeader, FLAG_ACK, FLAG_FIN, FLAG_PSH,
    FLAG_RST, FLAG_SYN, MAX_RETRANSMITS, TCP_HEADER_LEN,
};

const LOCAL_IP: Ipv4Addr = Ipv4Addr::new(10, 0, 2, 15);
const REMOTE_IP: Ipv4Addr = Ipv4Addr::new(10, 0, 2, 2);
const LOCAL_PORT: u16 = 0x4b57;
const REMOTE_PORT: u16 = 45678;
const ISS: u32 = 0x1000_0000;
const IRS: u32 = 0x2000_0000;

/// Ghi một segment theo hướng `src → dst` vào `buf`, trả số byte.
fn build(src: Ipv4Addr, dst: Ipv4Addr, h: TcpHeader, payload: &[u8], buf: &mut [u8]) -> usize {
    h.encode(src, dst, payload, buf).expect("đệm đủ chỗ")
}

/// Ghi một segment **từ server về khách** (hướng dùng trong hầu hết ca).
fn from_remote(h: TcpHeader, payload: &[u8], buf: &mut [u8]) -> usize {
    build(REMOTE_IP, LOCAL_IP, h, payload, buf)
}

/// Header của một segment từ server: cổng đảo chiều.
fn remote_header(seq: u32, ack: u32, flags: u8, window: u16) -> TcpHeader {
    TcpHeader::new(REMOTE_PORT, LOCAL_PORT, seq, ack, flags, window)
}

/// Kết nối đã ở `Established` qua SYN+ACK thật (ack = ISS+1, seq = IRS, window = 4096).
fn established() -> TcpConnection {
    let mut conn = TcpConnection::connect(LOCAL_IP, LOCAL_PORT, REMOTE_IP, REMOTE_PORT, ISS);
    let mut buf = [0u8; 64];
    let h = remote_header(IRS, ISS.wrapping_add(1), FLAG_SYN | FLAG_ACK, 4096);
    let n = from_remote(h, &[], &mut buf);
    let seg = TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], true).expect("hợp lệ");
    let actions = conn.on_segment(&seg).expect("SYN+ACK hợp lệ");
    assert_eq!(actions.len(), 1);
    assert_eq!(conn.state(), State::Established);
    conn
}

/// Gom hành động thành `Vec` để so sánh (test host có `std`).
fn acts(actions: vifast_net::tcp::Actions<'_>) -> Vec<Action<'_>> {
    actions.iter().collect()
}

// ================================================================ codec

#[test]
fn tcp_header_encode_byte_exact() {
    let h = TcpHeader::new(0x1234, 0x5678, 0xdead_beef, 0x0000_0001, FLAG_SYN, 0x4000);
    let mut buf = [0u8; TCP_HEADER_LEN];
    let n = h
        .encode(LOCAL_IP, REMOTE_IP, &[], &mut buf)
        .expect("vừa 20");
    assert_eq!(n, TCP_HEADER_LEN);
    assert_eq!(&buf[0..2], &[0x12, 0x34]);
    assert_eq!(&buf[2..4], &[0x56, 0x78]);
    assert_eq!(&buf[4..8], &[0xde, 0xad, 0xbe, 0xef]);
    assert_eq!(&buf[8..12], &[0x00, 0x00, 0x00, 0x01]);
    assert_eq!(buf[12], 5 << 4, "data offset = 5 word, reserved = 0");
    assert_eq!(buf[13], FLAG_SYN);
    assert_eq!(&buf[14..16], &[0x40, 0x00]);
    assert_eq!(&buf[18..20], &[0, 0], "urgent pointer = 0");
    // Checksum có pseudo-header: cộng lại toàn segment (kể cả trường checksum) phải bằng 0.
    let mut c = Checksum::new();
    c.add_bytes(&LOCAL_IP.octets());
    c.add_bytes(&REMOTE_IP.octets());
    c.add_bytes(&[0, 6]);
    c.add_u16(TCP_HEADER_LEN as u16);
    c.add_bytes(&buf);
    assert_eq!(c.finish(), 0);
    assert_ne!(u16::from_be_bytes([buf[16], buf[17]]), 0);
}

#[test]
fn tcp_header_roundtrip_co_payload() {
    let payload = b"vifast-tcp-ok";
    let h = TcpHeader::new(
        REMOTE_PORT,
        LOCAL_PORT,
        0x1122_3344,
        0x5566_7788,
        FLAG_ACK | FLAG_PSH,
        4096,
    );
    let mut buf = [0u8; 64];
    let n = h
        .encode(REMOTE_IP, LOCAL_IP, payload, &mut buf)
        .expect("vừa");
    assert_eq!(n, TCP_HEADER_LEN + payload.len());
    let seg = TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], true).expect("hợp lệ");
    assert_eq!(seg.header.src_port, REMOTE_PORT);
    assert_eq!(seg.header.dst_port, LOCAL_PORT);
    assert_eq!(seg.header.seq, 0x1122_3344);
    assert_eq!(seg.header.ack, 0x5566_7788);
    assert_eq!(seg.header.data_offset, 5);
    assert_eq!(seg.header.flags, FLAG_ACK | FLAG_PSH);
    assert_eq!(seg.header.window, 4096);
    assert_eq!(seg.payload(), payload);
    assert!(seg.header.has(FLAG_ACK));
    assert!(seg.header.has(FLAG_PSH));
    assert!(!seg.header.has(FLAG_SYN));
}

#[test]
fn tcp_checksum_phu_thuoc_pseudo_header() {
    // Cùng header + payload nhưng **địa chỉ nguồn khác** ⇒ checksum khác (pseudo-header thật sự
    // được cộng, không chỉ checksum phần TCP).
    let h = TcpHeader::new(REMOTE_PORT, LOCAL_PORT, 1, 2, FLAG_ACK, 4096);
    let mut a = [0u8; 64];
    let mut b = [0u8; 64];
    let na = h.encode(REMOTE_IP, LOCAL_IP, b"abc", &mut a).expect("vừa");
    let nb = h
        .encode(Ipv4Addr::new(10, 0, 2, 3), LOCAL_IP, b"abc", &mut b)
        .expect("vừa");
    assert_eq!(na, nb);
    assert_ne!(&a[16..18], &b[16..18]);
    // Và checksum của bản đúng phải kiểm được bằng pseudo-header tương ứng.
    let mut c = Checksum::new();
    c.add_bytes(&REMOTE_IP.octets());
    c.add_bytes(&LOCAL_IP.octets());
    c.add_bytes(&[0, 6]);
    c.add_u16(na as u16);
    c.add_bytes(&a[..na]);
    assert_eq!(c.finish(), 0);
}

#[test]
fn tcp_parse_checksum_sai() {
    let h = TcpHeader::new(REMOTE_PORT, LOCAL_PORT, 1, 2, FLAG_ACK, 4096);
    let mut buf = [0u8; 64];
    let n = h
        .encode(REMOTE_IP, LOCAL_IP, b"payload", &mut buf)
        .expect("vừa");
    buf[16] ^= 0xff;
    assert_eq!(
        TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], true).err(),
        Some(TcpError::BadChecksum)
    );
    // Bỏ kiểm checksum thì vẫn parse được (dùng cho chẩn đoán).
    assert!(TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], false).is_ok());
}

#[test]
fn tcp_parse_data_offset_nho_hon_5() {
    let h = TcpHeader::new(REMOTE_PORT, LOCAL_PORT, 1, 2, FLAG_ACK, 4096);
    let mut buf = [0u8; 64];
    let n = h.encode(REMOTE_IP, LOCAL_IP, &[], &mut buf).expect("vừa");
    buf[12] = 4 << 4;
    assert_eq!(
        TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], false).err(),
        Some(TcpError::BadDataOffset)
    );
}

#[test]
fn tcp_parse_data_offset_vuot_lat_byte() {
    let h = TcpHeader::new(REMOTE_PORT, LOCAL_PORT, 1, 2, FLAG_ACK, 4096);
    let mut buf = [0u8; 64];
    let n = h.encode(REMOTE_IP, LOCAL_IP, &[], &mut buf).expect("vừa");
    buf[12] = 6 << 4; // 24 byte header nhưng chỉ có 20 byte
    assert_eq!(
        TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], false).err(),
        Some(TcpError::Truncated)
    );
}

#[test]
fn tcp_parse_cat_cut() {
    let h = TcpHeader::new(REMOTE_PORT, LOCAL_PORT, 1, 2, FLAG_ACK, 4096);
    let mut buf = [0u8; 64];
    let n = h.encode(REMOTE_IP, LOCAL_IP, &[], &mut buf).expect("vừa");
    assert_eq!(
        TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n - 1], false).err(),
        Some(TcpError::Truncated)
    );
}

#[test]
fn tcp_parse_co_la_reserved() {
    let h = TcpHeader::new(REMOTE_PORT, LOCAL_PORT, 1, 2, FLAG_ACK, 4096);
    let mut buf = [0u8; 64];
    let n = h.encode(REMOTE_IP, LOCAL_IP, &[], &mut buf).expect("vừa");
    buf[12] |= 0x02; // một bit reserved (RFC 793 §3.1: phải 0)
    assert_eq!(
        TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], false).err(),
        Some(TcpError::BadReserved)
    );
}

#[test]
fn tcp_parse_co_la_syn_fin() {
    let h = TcpHeader::new(REMOTE_PORT, LOCAL_PORT, 1, 2, FLAG_SYN | FLAG_FIN, 4096);
    let mut buf = [0u8; 64];
    let n = h.encode(REMOTE_IP, LOCAL_IP, &[], &mut buf).expect("vừa");
    assert_eq!(
        TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], false).err(),
        Some(TcpError::BadFlags)
    );
}

#[test]
fn tcp_encode_dem_hep() {
    let h = TcpHeader::new(REMOTE_PORT, LOCAL_PORT, 1, 2, FLAG_ACK, 4096);
    let mut buf = [0u8; TCP_HEADER_LEN - 1];
    assert_eq!(
        h.encode(REMOTE_IP, LOCAL_IP, &[], &mut buf).err(),
        Some(TcpError::BufferTooSmall)
    );
    let mut buf = [0u8; TCP_HEADER_LEN];
    assert_eq!(
        h.encode(REMOTE_IP, LOCAL_IP, b"x", &mut buf).err(),
        Some(TcpError::BufferTooSmall)
    );
}

#[test]
fn tcp_encode_data_offset_khac_5() {
    let mut h = TcpHeader::new(REMOTE_PORT, LOCAL_PORT, 1, 2, FLAG_ACK, 4096);
    h.data_offset = 6;
    let mut buf = [0u8; 64];
    assert_eq!(
        h.encode(REMOTE_IP, LOCAL_IP, &[], &mut buf).err(),
        Some(TcpError::BadDataOffset)
    );
}

#[test]
fn tcp_co_va_send_kind() {
    assert_eq!(FLAG_FIN, 0x01);
    assert_eq!(FLAG_SYN, 0x02);
    assert_eq!(FLAG_RST, 0x04);
    assert_eq!(FLAG_PSH, 0x08);
    assert_eq!(FLAG_ACK, 0x10);
    assert_eq!(SendKind::Syn.flags(), FLAG_SYN);
    assert_eq!(SendKind::Ack.flags(), FLAG_ACK);
    assert_eq!(SendKind::Data.flags(), FLAG_ACK | FLAG_PSH);
    assert_eq!(SendKind::Fin.flags(), FLAG_FIN | FLAG_ACK);
    assert_eq!(SendKind::Rst.flags(), FLAG_RST | FLAG_ACK);
}

// ================================================================ máy trạng thái

#[test]
fn tcp_connect_vao_syn_sent() {
    let conn = TcpConnection::connect(LOCAL_IP, LOCAL_PORT, REMOTE_IP, REMOTE_PORT, ISS);
    assert_eq!(conn.state(), State::SynSent);
    assert_eq!(conn.iss(), ISS);
    assert_eq!(conn.snd_nxt(), ISS.wrapping_add(1));
    assert_eq!(conn.snd_una(), ISS);
    assert_eq!(conn.rcv_nxt(), 0);
    assert_eq!(conn.local().port, LOCAL_PORT);
    assert_eq!(conn.remote().port, REMOTE_PORT);
    assert_eq!(conn.remote().ip, REMOTE_IP);
}

#[test]
fn tcp_syn_ack_vao_established() {
    let mut conn = TcpConnection::connect(LOCAL_IP, LOCAL_PORT, REMOTE_IP, REMOTE_PORT, ISS);
    let mut buf = [0u8; 64];
    let h = remote_header(IRS, ISS.wrapping_add(1), FLAG_SYN | FLAG_ACK, 4096);
    let n = from_remote(h, &[], &mut buf);
    let seg = TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], true).expect("hợp lệ");
    let actions = conn.on_segment(&seg).expect("hợp lệ");
    assert_eq!(acts(actions), vec![Action::Send(SendKind::Ack)]);
    assert_eq!(conn.state(), State::Established);
    assert_eq!(conn.irs(), IRS);
    assert_eq!(conn.rcv_nxt(), IRS.wrapping_add(1));
    assert_eq!(conn.snd_una(), ISS.wrapping_add(1));
    assert_eq!(conn.snd_wnd(), 4096);
}

#[test]
fn tcp_syn_ack_ack_sai() {
    let mut conn = TcpConnection::connect(LOCAL_IP, LOCAL_PORT, REMOTE_IP, REMOTE_PORT, ISS);
    let mut buf = [0u8; 64];
    let h = remote_header(IRS, ISS.wrapping_add(2), FLAG_SYN | FLAG_ACK, 4096);
    let n = from_remote(h, &[], &mut buf);
    let seg = TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], true).expect("hợp lệ");
    assert_eq!(conn.on_segment(&seg).err(), Some(TcpError::BadAck));
    assert_eq!(
        conn.state(),
        State::SynSent,
        "ack sai không được đổi trạng thái"
    );
}

#[test]
fn tcp_syn_khong_ack_bi_tu_choi() {
    let mut conn = TcpConnection::connect(LOCAL_IP, LOCAL_PORT, REMOTE_IP, REMOTE_PORT, ISS);
    let mut buf = [0u8; 64];
    let h = remote_header(IRS, 0, FLAG_SYN, 4096);
    let n = from_remote(h, &[], &mut buf);
    let seg = TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], true).expect("hợp lệ");
    let actions = conn.on_segment(&seg).expect("không phải lỗi codec");
    assert_eq!(acts(actions), vec![Action::Reset]);
    assert_eq!(conn.state(), State::Closed);
}

#[test]
fn tcp_du_lieu_trong_cua_so() {
    let mut conn = established();
    let mut buf = [0u8; 64];
    let h = remote_header(
        IRS.wrapping_add(1),
        ISS.wrapping_add(1),
        FLAG_ACK | FLAG_PSH,
        4096,
    );
    let n = from_remote(h, b"hello", &mut buf);
    let seg = TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], true).expect("hợp lệ");
    let actions = conn.on_segment(&seg).expect("hợp lệ");
    assert_eq!(
        acts(actions),
        vec![Action::Deliver(b"hello"), Action::Send(SendKind::Ack)]
    );
    assert_eq!(conn.rcv_nxt(), IRS.wrapping_add(6));
}

#[test]
fn tcp_du_lieu_ngoai_cua_so() {
    let mut conn = established();
    let mut buf = [0u8; 64];
    let h = remote_header(IRS.wrapping_add(9000), ISS.wrapping_add(1), FLAG_ACK, 4096);
    let n = from_remote(h, b"late", &mut buf);
    let seg = TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], true).expect("hợp lệ");
    let actions = conn.on_segment(&seg).expect("bỏ + ACK lại, không phải lỗi");
    assert_eq!(acts(actions), vec![Action::Send(SendKind::Ack)]);
    assert_eq!(conn.rcv_nxt(), IRS.wrapping_add(1), "không nhận byte nào");
}

#[test]
fn tcp_du_lieu_trung_bi_ack_lai() {
    let mut conn = established();
    let mut buf = [0u8; 64];
    let h = remote_header(IRS.wrapping_add(1), ISS.wrapping_add(1), FLAG_ACK, 4096);
    let n = from_remote(h, b"abc", &mut buf);
    let seg = TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], true).expect("hợp lệ");
    let first = conn.on_segment(&seg).expect("hợp lệ");
    assert_eq!(first.len(), 2);
    // Gửi lại **đúng** segment cũ (retransmit của peer): phải ACK lại, **không** giao lần hai.
    let again = conn.on_segment(&seg).expect("hợp lệ");
    assert_eq!(acts(again), vec![Action::Send(SendKind::Ack)]);
    assert_eq!(conn.rcv_nxt(), IRS.wrapping_add(4));
}

#[test]
fn tcp_du_lieu_lo_hong_bi_ack_lai() {
    let mut conn = established();
    let mut buf = [0u8; 64];
    let h = remote_header(IRS.wrapping_add(3), ISS.wrapping_add(1), FLAG_ACK, 4096);
    let n = from_remote(h, b"gap", &mut buf);
    let seg = TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], true).expect("hợp lệ");
    let actions = conn.on_segment(&seg).expect("hợp lệ");
    assert_eq!(acts(actions), vec![Action::Send(SendKind::Ack)]);
    assert_eq!(conn.rcv_nxt(), IRS.wrapping_add(1));
}

#[test]
fn tcp_cua_so_0_nhan_du_lieu() {
    let mut conn = established();
    conn.set_rcv_window(0);
    let mut buf = [0u8; 64];
    let h = remote_header(IRS.wrapping_add(1), ISS.wrapping_add(1), FLAG_ACK, 4096);
    let n = from_remote(h, b"x", &mut buf);
    let seg = TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], true).expect("hợp lệ");
    let actions = conn.on_segment(&seg).expect("hợp lệ");
    assert_eq!(acts(actions), vec![Action::Send(SendKind::Ack)]);
    assert_eq!(conn.rcv_nxt(), IRS.wrapping_add(1));
}

#[test]
fn tcp_cua_so_0_khong_gui_duoc() {
    let mut conn = TcpConnection::connect(LOCAL_IP, LOCAL_PORT, REMOTE_IP, REMOTE_PORT, ISS);
    let mut buf = [0u8; 64];
    let h = remote_header(IRS, ISS.wrapping_add(1), FLAG_SYN | FLAG_ACK, 0);
    let n = from_remote(h, &[], &mut buf);
    let seg = TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], true).expect("hợp lệ");
    conn.on_segment(&seg).expect("hợp lệ");
    assert_eq!(conn.snd_wnd(), 0);
    assert_eq!(conn.build_data(1).err(), Some(TcpError::ZeroWindow));
}

#[test]
fn tcp_rst_giua_pha() {
    let mut conn = established();
    let mut buf = [0u8; 64];
    let h = remote_header(
        IRS.wrapping_add(1),
        ISS.wrapping_add(1),
        FLAG_RST | FLAG_ACK,
        0,
    );
    let n = from_remote(h, &[], &mut buf);
    let seg = TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], true).expect("hợp lệ");
    let actions = conn
        .on_segment(&seg)
        .expect("RST là hành động, không phải lỗi codec");
    assert_eq!(acts(actions), vec![Action::Reset]);
    assert_eq!(conn.state(), State::Closed);
}

#[test]
fn tcp_rst_syn_sent_ack_dung() {
    let mut conn = TcpConnection::connect(LOCAL_IP, LOCAL_PORT, REMOTE_IP, REMOTE_PORT, ISS);
    let mut buf = [0u8; 64];
    let h = remote_header(0, ISS.wrapping_add(1), FLAG_RST | FLAG_ACK, 0);
    let n = from_remote(h, &[], &mut buf);
    let seg = TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], true).expect("hợp lệ");
    let actions = conn.on_segment(&seg).expect("hợp lệ");
    assert_eq!(acts(actions), vec![Action::Reset]);
    assert_eq!(conn.state(), State::Closed);
}

#[test]
fn tcp_rst_syn_sent_ack_sai_bi_bo_qua() {
    let mut conn = TcpConnection::connect(LOCAL_IP, LOCAL_PORT, REMOTE_IP, REMOTE_PORT, ISS);
    let mut buf = [0u8; 64];
    let h = remote_header(0, 0, FLAG_RST, 0);
    let n = from_remote(h, &[], &mut buf);
    let seg = TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], true).expect("hợp lệ");
    let actions = conn.on_segment(&seg).expect("hợp lệ");
    assert!(acts(actions).is_empty(), "RST không ACK đúng thì bỏ qua");
    assert_eq!(conn.state(), State::SynSent);
}

#[test]
fn tcp_fin_established_vao_last_ack() {
    let mut conn = established();
    let mut buf = [0u8; 64];
    let h = remote_header(
        IRS.wrapping_add(1),
        ISS.wrapping_add(1),
        FLAG_FIN | FLAG_ACK,
        4096,
    );
    let n = from_remote(h, &[], &mut buf);
    let seg = TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], true).expect("hợp lệ");
    let actions = conn.on_segment(&seg).expect("hợp lệ");
    assert_eq!(
        acts(actions),
        vec![Action::Send(SendKind::Ack), Action::Send(SendKind::Fin)]
    );
    assert_eq!(conn.state(), State::LastAck);
    assert_eq!(
        conn.rcv_nxt(),
        IRS.wrapping_add(2),
        "FIN chiếm một số thứ tự"
    );
}

#[test]
fn tcp_dong_chu_dong_fin_wait1_roi_fin_wait2() {
    let mut conn = established();
    let seq = conn.build_fin().expect("đóng được");
    assert_eq!(seq, ISS.wrapping_add(1));
    assert_eq!(conn.state(), State::FinWait1);
    assert_eq!(conn.snd_nxt(), ISS.wrapping_add(2));
    // ACK của peer cho FIN của ta.
    let mut buf = [0u8; 64];
    let h = remote_header(IRS.wrapping_add(1), ISS.wrapping_add(2), FLAG_ACK, 4096);
    let n = from_remote(h, &[], &mut buf);
    let seg = TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], true).expect("hợp lệ");
    let actions = conn.on_segment(&seg).expect("hợp lệ");
    assert!(acts(actions).is_empty());
    assert_eq!(conn.state(), State::FinWait2);
}

#[test]
fn tcp_fin_wait1_nhan_fin_va_ack_cung_luc() {
    let mut conn = established();
    conn.build_fin().expect("đóng được");
    let mut buf = [0u8; 64];
    let h = remote_header(
        IRS.wrapping_add(1),
        ISS.wrapping_add(2),
        FLAG_FIN | FLAG_ACK,
        4096,
    );
    let n = from_remote(h, &[], &mut buf);
    let seg = TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], true).expect("hợp lệ");
    let actions = conn.on_segment(&seg).expect("hợp lệ");
    assert_eq!(
        acts(actions),
        vec![Action::Send(SendKind::Ack), Action::Close]
    );
    assert_eq!(conn.state(), State::TimeWait);
}

#[test]
fn tcp_fin_wait1_nhan_fin_chua_ack_vao_closing() {
    let mut conn = established();
    conn.build_fin().expect("đóng được");
    let mut buf = [0u8; 64];
    // ACK cũ (chưa ack FIN của ta) + FIN của peer.
    let h = remote_header(
        IRS.wrapping_add(1),
        ISS.wrapping_add(1),
        FLAG_FIN | FLAG_ACK,
        4096,
    );
    let n = from_remote(h, &[], &mut buf);
    let seg = TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], true).expect("hợp lệ");
    let actions = conn.on_segment(&seg).expect("hợp lệ");
    assert_eq!(acts(actions), vec![Action::Send(SendKind::Ack)]);
    assert_eq!(conn.state(), State::Closing);
    // Rồi ACK cho FIN của ta ⇒ TimeWait + Close.
    let h = remote_header(IRS.wrapping_add(2), ISS.wrapping_add(2), FLAG_ACK, 4096);
    let n = from_remote(h, &[], &mut buf);
    let seg = TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], true).expect("hợp lệ");
    let actions = conn.on_segment(&seg).expect("hợp lệ");
    assert_eq!(acts(actions), vec![Action::Close]);
    assert_eq!(conn.state(), State::TimeWait);
}

#[test]
fn tcp_fin_wait2_nhan_fin() {
    let mut conn = established();
    conn.build_fin().expect("đóng được");
    let mut buf = [0u8; 64];
    let h = remote_header(IRS.wrapping_add(1), ISS.wrapping_add(2), FLAG_ACK, 4096);
    let n = from_remote(h, &[], &mut buf);
    let seg = TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], true).expect("hợp lệ");
    conn.on_segment(&seg).expect("hợp lệ");
    assert_eq!(conn.state(), State::FinWait2);
    let h = remote_header(
        IRS.wrapping_add(1),
        ISS.wrapping_add(2),
        FLAG_FIN | FLAG_ACK,
        4096,
    );
    let n = from_remote(h, &[], &mut buf);
    let seg = TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], true).expect("hợp lệ");
    let actions = conn.on_segment(&seg).expect("hợp lệ");
    assert_eq!(
        acts(actions),
        vec![Action::Send(SendKind::Ack), Action::Close]
    );
    assert_eq!(conn.state(), State::TimeWait);
}

#[test]
fn tcp_last_ack_nhan_ack_dong() {
    let mut conn = established();
    let mut buf = [0u8; 64];
    let h = remote_header(
        IRS.wrapping_add(1),
        ISS.wrapping_add(1),
        FLAG_FIN | FLAG_ACK,
        4096,
    );
    let n = from_remote(h, &[], &mut buf);
    let seg = TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], true).expect("hợp lệ");
    conn.on_segment(&seg).expect("hợp lệ");
    assert_eq!(conn.state(), State::LastAck);
    let h = remote_header(IRS.wrapping_add(2), ISS.wrapping_add(2), FLAG_ACK, 4096);
    let n = from_remote(h, &[], &mut buf);
    let seg = TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], true).expect("hợp lệ");
    let actions = conn.on_segment(&seg).expect("hợp lệ");
    assert_eq!(acts(actions), vec![Action::Close]);
    assert_eq!(conn.state(), State::Closed);
}

#[test]
fn tcp_ack_tuong_lai_bi_tu_choi() {
    let mut conn = established();
    let mut buf = [0u8; 64];
    let h = remote_header(IRS.wrapping_add(1), ISS.wrapping_add(9), FLAG_ACK, 4096);
    let n = from_remote(h, &[], &mut buf);
    let seg = TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], true).expect("hợp lệ");
    assert_eq!(conn.on_segment(&seg).err(), Some(TcpError::BadAck));
    assert_eq!(conn.state(), State::Established);
    assert_eq!(conn.snd_una(), ISS.wrapping_add(1));
}

#[test]
fn tcp_ack_cu_khong_doi_trang_thai() {
    let mut conn = established();
    let mut buf = [0u8; 64];
    let h = remote_header(IRS.wrapping_add(1), ISS, FLAG_ACK, 4096);
    let n = from_remote(h, &[], &mut buf);
    let seg = TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], true).expect("hợp lệ");
    let actions = conn.on_segment(&seg).expect("hợp lệ");
    assert!(acts(actions).is_empty());
    assert_eq!(
        conn.snd_una(),
        ISS.wrapping_add(1),
        "ACK cũ không lùi snd_una"
    );
}

#[test]
fn tcp_cap_nhat_cua_so_gui() {
    let mut conn = established();
    let mut buf = [0u8; 64];
    let h = remote_header(IRS.wrapping_add(1), ISS.wrapping_add(1), FLAG_ACK, 0);
    let n = from_remote(h, &[], &mut buf);
    let seg = TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], true).expect("hợp lệ");
    conn.on_segment(&seg).expect("hợp lệ");
    assert_eq!(conn.snd_wnd(), 0);
    let h = remote_header(IRS.wrapping_add(1), ISS.wrapping_add(1), FLAG_ACK, 100);
    let n = from_remote(h, &[], &mut buf);
    let seg = TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], true).expect("hợp lệ");
    conn.on_segment(&seg).expect("hợp lệ");
    assert_eq!(conn.snd_wnd(), 100);
}

#[test]
fn tcp_send_du_lieu_qua_mss() {
    let mut conn = established();
    assert_eq!(
        conn.build_data(vifast_net::tcp::DEFAULT_MSS + 1).err(),
        Some(TcpError::PayloadTooLarge)
    );
    assert_eq!(conn.build_data(0).err(), Some(TcpError::PayloadTooLarge));
}

#[test]
fn tcp_send_khi_chua_established() {
    let mut conn = TcpConnection::connect(LOCAL_IP, LOCAL_PORT, REMOTE_IP, REMOTE_PORT, ISS);
    assert_eq!(conn.build_data(1).err(), Some(TcpError::NotEstablished));
    assert_eq!(conn.build_fin().err(), Some(TcpError::NotEstablished));
}

#[test]
fn tcp_sai_cong_bi_tu_choi() {
    let mut conn = established();
    let mut buf = [0u8; 64];
    let h = TcpHeader::new(
        REMOTE_PORT + 1,
        LOCAL_PORT,
        IRS.wrapping_add(1),
        ISS.wrapping_add(1),
        FLAG_ACK,
        4096,
    );
    let n = from_remote(h, &[], &mut buf);
    let seg = TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], true).expect("hợp lệ");
    assert_eq!(conn.on_segment(&seg).err(), Some(TcpError::WrongPort));
}

#[test]
fn tcp_du_lieu_sau_fin_bi_tu_choi() {
    let mut conn = established();
    let mut buf = [0u8; 64];
    let h = remote_header(
        IRS.wrapping_add(1),
        ISS.wrapping_add(1),
        FLAG_FIN | FLAG_ACK,
        4096,
    );
    let n = from_remote(h, &[], &mut buf);
    let seg = TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], true).expect("hợp lệ");
    conn.on_segment(&seg).expect("hợp lệ");
    assert_eq!(conn.state(), State::LastAck);
    let h = remote_header(IRS.wrapping_add(2), ISS.wrapping_add(1), FLAG_ACK, 4096);
    let n = from_remote(h, b"after-fin", &mut buf);
    let seg = TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], true).expect("hợp lệ");
    assert_eq!(conn.on_segment(&seg).err(), Some(TcpError::BadState));
}

#[test]
fn tcp_retransmit_toi_da_3_lan() {
    let mut conn = TcpConnection::connect(LOCAL_IP, LOCAL_PORT, REMOTE_IP, REMOTE_PORT, ISS);
    for _ in 0..MAX_RETRANSMITS {
        assert_eq!(conn.on_timeout(), Action::Send(SendKind::Syn));
        assert_eq!(conn.state(), State::SynSent);
    }
    assert_eq!(conn.retries(), MAX_RETRANSMITS);
    // Lần thứ tư: **không** retransmit nữa — huỷ.
    assert_eq!(conn.on_timeout(), Action::Reset);
    assert_eq!(conn.state(), State::Closed);
}

#[test]
fn tcp_retransmit_fin_va_reset_lai_khi_tien_trien() {
    let mut conn = established();
    conn.build_fin().expect("đóng được");
    assert_eq!(conn.on_timeout(), Action::Send(SendKind::Fin));
    assert_eq!(conn.retries(), 1);
    // ACK của peer ⇒ tiến triển ⇒ đếm retransmit về 0.
    let mut buf = [0u8; 64];
    let h = remote_header(IRS.wrapping_add(1), ISS.wrapping_add(2), FLAG_ACK, 4096);
    let n = from_remote(h, &[], &mut buf);
    let seg = TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], true).expect("hợp lệ");
    conn.on_segment(&seg).expect("hợp lệ");
    assert_eq!(conn.state(), State::FinWait2);
    assert_eq!(conn.retries(), 0);
}

#[test]
fn tcp_seq_wrap_quanh_u32() {
    let mut conn =
        TcpConnection::connect(LOCAL_IP, LOCAL_PORT, REMOTE_IP, REMOTE_PORT, u32::MAX - 1);
    let mut buf = [0u8; 64];
    let h = remote_header(u32::MAX, u32::MAX, FLAG_SYN | FLAG_ACK, 4096);
    let n = from_remote(h, &[], &mut buf);
    let seg = TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], true).expect("hợp lệ");
    conn.on_segment(&seg).expect("hợp lệ");
    assert_eq!(conn.state(), State::Established);
    assert_eq!(conn.rcv_nxt(), 0, "seq 0xffffffff + 1 = 0");
    let h = remote_header(0, 0, FLAG_ACK, 4096);
    let n = from_remote(h, b"wrap", &mut buf);
    let seg = TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], true).expect("hợp lệ");
    let actions = conn.on_segment(&seg).expect("hợp lệ");
    assert_eq!(
        acts(actions),
        vec![Action::Deliver(b"wrap"), Action::Send(SendKind::Ack)]
    );
    assert_eq!(conn.rcv_nxt(), 4);
}

#[test]
fn tcp_build_data_tra_seq_va_tang_snd_nxt() {
    let mut conn = established();
    let seq = conn.build_data(5).expect("gửi được");
    assert_eq!(seq, ISS.wrapping_add(1));
    assert_eq!(conn.snd_nxt(), ISS.wrapping_add(6));
    // ACK bao đúng 5 byte ⇒ snd_una tiến, retransmit về 0.
    let mut buf = [0u8; 64];
    let h = remote_header(IRS.wrapping_add(1), ISS.wrapping_add(6), FLAG_ACK, 4096);
    let n = from_remote(h, &[], &mut buf);
    let seg = TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], true).expect("hợp lệ");
    conn.on_segment(&seg).expect("hợp lệ");
    assert_eq!(conn.snd_una(), ISS.wrapping_add(6));
    assert_eq!(conn.retries(), 0);
}

#[test]
fn tcp_syn_ack_trung_sau_established_chi_ack_lai() {
    // Peer retransmit SYN+ACK (vì ACK của ta tới muộn) ⇒ ACK lại, **không** reset kết nối.
    let mut conn = established();
    let mut buf = [0u8; 64];
    let h = remote_header(IRS, ISS.wrapping_add(1), FLAG_SYN | FLAG_ACK, 4096);
    let n = from_remote(h, &[], &mut buf);
    let seg = TcpHeader::parse(REMOTE_IP, LOCAL_IP, &buf[..n], true).expect("hợp lệ");
    let actions = conn.on_segment(&seg).expect("hợp lệ");
    assert_eq!(acts(actions), vec![Action::Send(SendKind::Ack)]);
    assert_eq!(conn.state(), State::Established);
}
