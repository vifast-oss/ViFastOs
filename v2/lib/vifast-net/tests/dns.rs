// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-260 — test host cho `dns` (D2-05: viết trước, đỏ rồi mới xanh).
//!
//! Gồm: truy vấn A **byte-exact**, reply dựng tay (một A; CNAME + A), nén tên, và **ca âm**:
//! cắt cụt, `qdcount=0`, con trỏ nén **vòng lặp**, con trỏ ra ngoài message, `RDLENGTH` sai,
//! label quá 63 byte, tên quá 255 byte, buffer hẹp, id sai, không phải response.

use vifast_net::addr::Ipv4Addr;
use vifast_net::dns::{
    ARecord, DnsError, DnsHeader, DnsMessage, DnsQuery, DnsQuestion, Name, CLASS_IN,
    DNS_HEADER_LEN, DNS_MAX_LABEL_LEN, FLAG_RD, TYPE_A,
};

/// Header reply tối thiểu: `id`, QR=1, RD=1, RA=1, `qd`, `an`.
fn reply_header(id: u16, qd: u16, an: u16) -> [u8; DNS_HEADER_LEN] {
    let flags: u16 = 0x8000 | 0x0100 | 0x0080;
    let mut out = [0u8; DNS_HEADER_LEN];
    out[0..2].copy_from_slice(&id.to_be_bytes());
    out[2..4].copy_from_slice(&flags.to_be_bytes());
    out[4..6].copy_from_slice(&qd.to_be_bytes());
    out[6..8].copy_from_slice(&an.to_be_bytes());
    out
}

/// Question `example.com` A IN (17 byte).
const QUESTION_EXAMPLE: [u8; 17] = [
    7, b'e', b'x', b'a', b'm', b'p', b'l', b'e', 3, b'c', b'o', b'm', 0, 0, 1, 0, 1,
];

#[test]
fn dns_query_example_com_byte_exact() {
    let q = DnsQuery::new(0x1234, b"example.com", TYPE_A).expect("tên hợp lệ");
    let mut buf = [0u8; 64];
    let n = q.encode(&mut buf).expect("vừa 29 byte");
    assert_eq!(n, 29);
    assert_eq!(
        &buf[..n],
        &[
            0x12, 0x34, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 7, b'e', b'x',
            b'a', b'm', b'p', b'l', b'e', 3, b'c', b'o', b'm', 0, 0x00, 0x01, 0x00, 0x01,
        ][..]
    );
}

#[test]
fn dns_query_parse_roundtrip() {
    let q = DnsQuery::new(0xbeef, b"example.com", TYPE_A).expect("hợp lệ");
    let mut buf = [0u8; 64];
    let n = q.encode(&mut buf).expect("vừa");
    let m = DnsMessage::parse(&buf[..n]).expect("parse được");
    assert_eq!(m.header().id, 0xbeef);
    assert!(m.header().rd());
    assert!(!m.header().is_response());
    assert_eq!(m.header().qdcount, 1);
    assert_eq!(m.header().ancount, 0);
    let question = m.question().expect("parse câu hỏi").expect("có câu hỏi");
    assert_eq!(question.name.as_bytes(), b"example.com");
    assert_eq!(question.qtype, TYPE_A);
    assert_eq!(question.qclass, CLASS_IN);
    assert!(m.first_a().expect("không lỗi").is_none());
}

#[test]
fn dns_header_parse_fields() {
    let raw = [
        0x12, 0x34, 0x81, 0x83, 0x00, 0x01, 0x00, 0x02, 0x00, 0x03, 0x00, 0x04,
    ];
    let h = DnsHeader::parse(&raw).expect("12 byte");
    assert_eq!(h.id, 0x1234);
    assert!(h.is_response());
    assert_eq!(h.opcode(), 0);
    assert!(h.rd());
    assert!(h.ra());
    assert_eq!(h.rcode(), 3);
    assert_eq!(h.qdcount, 1);
    assert_eq!(h.ancount, 2);
    assert_eq!(h.nscount, 3);
    assert_eq!(h.arcount, 4);
    let query = DnsHeader::query(7);
    assert_eq!(query.id, 7);
    assert_eq!(query.flags, FLAG_RD);
    assert_eq!(query.qdcount, 1);
    assert_eq!(query.ancount, 0);
}

#[test]
fn dns_a_reply_parse() {
    let mut buf = Vec::new();
    buf.extend_from_slice(&reply_header(0x1234, 1, 1));
    buf.extend_from_slice(&QUESTION_EXAMPLE);
    // Answer: name = con trỏ 0x0c, type A, class IN, ttl 300, rdlength 4, 93.184.216.34.
    buf.extend_from_slice(&[
        0xc0, 0x0c, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x01, 0x2c, 0x00, 0x04, 93, 184, 216, 34,
    ]);
    let m = DnsMessage::parse(&buf).expect("parse");
    assert_eq!(m.header().ancount, 1);
    let a = m.first_a().expect("không lỗi").expect("có A");
    assert_eq!(a.addr, Ipv4Addr::new(93, 184, 216, 34));
    assert_eq!(a.ttl, 300);
    let question = m.question().expect("parse").expect("có câu hỏi");
    assert_eq!(question.name.as_bytes(), b"example.com");
}

#[test]
fn dns_a_reply_bo_qua_cname() {
    let mut buf = Vec::new();
    buf.extend_from_slice(&reply_header(0x1234, 1, 2));
    buf.extend_from_slice(&QUESTION_EXAMPLE);
    // CNAME: name = con trỏ 0x0c, type 5, class IN, ttl 60, rdlength 2, rdata = con trỏ 0x0c.
    buf.extend_from_slice(&[
        0xc0, 0x0c, 0x00, 0x05, 0x00, 0x01, 0x00, 0x00, 0x00, 0x3c, 0x00, 0x02, 0xc0, 0x0c,
    ]);
    // A: name = con trỏ 0x0c, type A, class IN, ttl 60, rdlength 4, 1.2.3.4.
    buf.extend_from_slice(&[
        0xc0, 0x0c, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x3c, 0x00, 0x04, 1, 2, 3, 4,
    ]);
    let m = DnsMessage::parse(&buf).expect("parse");
    let a = m.first_a().expect("không lỗi").expect("có A");
    assert_eq!(a.addr, Ipv4Addr::new(1, 2, 3, 4));
    assert_eq!(a.ttl, 60);
}

#[test]
fn dns_ca_am_cat_cut_header() {
    assert_eq!(
        DnsHeader::parse(&[0u8; DNS_HEADER_LEN - 1]).err(),
        Some(DnsError::Truncated)
    );
    assert_eq!(
        DnsMessage::parse(&[0u8; DNS_HEADER_LEN - 1]).err(),
        Some(DnsError::Truncated)
    );
}

#[test]
fn dns_ca_am_cat_cut_question() {
    // Header qd=1 nhưng **không** có byte câu hỏi.
    let raw = [0x12, 0x34, 0x01, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0];
    assert_eq!(DnsMessage::parse(&raw).err(), Some(DnsError::Truncated));
    // Tên đủ nhưng thiếu 2 byte của qtype/qclass.
    let mut raw = Vec::from(&[0x12, 0x34, 0x01, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0][..]);
    raw.extend_from_slice(&[1, b'a', 0, 0, 1]);
    assert_eq!(DnsMessage::parse(&raw).err(), Some(DnsError::Truncated));
}

#[test]
fn dns_ca_am_qdcount_0() {
    let raw = [
        0x12, 0x34, 0x81, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    let m = DnsMessage::parse(&raw).expect("cấu trúc hợp lệ");
    assert!(m.question().expect("không lỗi").is_none());
    assert_eq!(
        DnsMessage::parse_response(&raw, 0x1234).err(),
        Some(DnsError::BadCount)
    );
}

#[test]
fn dns_ca_am_pointer_vong_lap() {
    // Con trỏ trỏ vào chính nó (offset 12).
    let mut raw = Vec::from(&[0x12, 0x34, 0x01, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0][..]);
    raw.extend_from_slice(&[0xc0, 0x0c]);
    assert_eq!(DnsMessage::parse(&raw).err(), Some(DnsError::PointerLoop));
    // Hai con trỏ trỏ vào nhau: 0x0c → 0x0e, 0x0e → 0x0c.
    let mut raw = Vec::from(&[0x12, 0x34, 0x01, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0][..]);
    raw.extend_from_slice(&[0xc0, 0x0e, 0xc0, 0x0c]);
    assert_eq!(DnsMessage::parse(&raw).err(), Some(DnsError::PointerLoop));
}

#[test]
fn dns_ca_am_pointer_ngoai_message() {
    let mut raw = Vec::from(&[0x12, 0x34, 0x01, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0][..]);
    raw.extend_from_slice(&[0xc0, 0xff]); // target 255 > độ dài message
    assert_eq!(DnsMessage::parse(&raw).err(), Some(DnsError::BadPointer));
}

#[test]
fn dns_ca_am_rdlength_sai() {
    // A record với rdlength=3 (phải 4).
    let mut buf = Vec::new();
    buf.extend_from_slice(&reply_header(0x1234, 1, 1));
    buf.extend_from_slice(&QUESTION_EXAMPLE);
    buf.extend_from_slice(&[
        0xc0, 0x0c, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x3c, 0x00, 0x03, 1, 2, 3,
    ]);
    let m = DnsMessage::parse(&buf).expect("cấu trúc hợp lệ");
    assert_eq!(m.first_a().err(), Some(DnsError::BadRdlength));
}

#[test]
fn dns_ca_am_rdlength_vuot_message() {
    // rdlength=100 nhưng chỉ có 4 byte RDATA.
    let mut buf = Vec::new();
    buf.extend_from_slice(&reply_header(0x1234, 1, 1));
    buf.extend_from_slice(&QUESTION_EXAMPLE);
    buf.extend_from_slice(&[
        0xc0, 0x0c, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x3c, 0x00, 0x64, 1, 2, 3, 4,
    ]);
    assert_eq!(DnsMessage::parse(&buf).err(), Some(DnsError::BadRdlength));
}

#[test]
fn dns_ca_am_ancount_lon_hon_so_record() {
    // ancount=2 nhưng chỉ có **một** answer.
    let mut buf = Vec::new();
    buf.extend_from_slice(&reply_header(0x1234, 1, 2));
    buf.extend_from_slice(&QUESTION_EXAMPLE);
    buf.extend_from_slice(&[
        0xc0, 0x0c, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x3c, 0x00, 0x04, 1, 2, 3, 4,
    ]);
    assert_eq!(DnsMessage::parse(&buf).err(), Some(DnsError::Truncated));
}

#[test]
fn dns_ca_am_khong_phai_response_va_id_sai() {
    let q = DnsQuery::new(0x1234, b"example.com", TYPE_A).expect("hợp lệ");
    let mut buf = [0u8; 64];
    let n = q.encode(&mut buf).expect("vừa");
    assert_eq!(
        DnsMessage::parse_response(&buf[..n], 0x1234).err(),
        Some(DnsError::NotResponse)
    );
    let mut reply = Vec::from(&buf[..n]);
    reply[2] |= 0x80; // QR=1
    assert_eq!(
        DnsMessage::parse_response(&reply, 0x9999).err(),
        Some(DnsError::BadId)
    );
    assert!(DnsMessage::parse_response(&reply, 0x1234).is_ok());
}

#[test]
fn dns_ca_am_label_qua_dai_va_ten_qua_255() {
    let label = [b'a'; DNS_MAX_LABEL_LEN + 1];
    assert_eq!(Name::from_dotted(&label).err(), Some(DnsError::BadName));
    // Bốn label 63 byte: wire = 4 × (1 + 63) + 1 = 257 > 255.
    let mut name = Vec::new();
    for index in 0..4 {
        if index > 0 {
            name.push(b'.');
        }
        name.extend_from_slice(&[b'a'; 63]);
    }
    assert_eq!(Name::from_dotted(&name).err(), Some(DnsError::BadName));
    // Label 63 byte vẫn hợp lệ.
    let ok = Name::from_dotted(&[b'a'; 63]).expect("63 byte hợp lệ");
    assert_eq!(ok.as_bytes().len(), 63);
}

#[test]
fn dns_ca_am_label_rong() {
    assert_eq!(Name::from_dotted(b"").err(), Some(DnsError::BadName));
    assert_eq!(Name::from_dotted(b"a..b").err(), Some(DnsError::BadName));
    assert_eq!(Name::from_dotted(b".a").err(), Some(DnsError::BadName));
    assert_eq!(Name::from_dotted(b"a.").err(), Some(DnsError::BadName));
}

#[test]
fn dns_ca_am_buffer_hep() {
    let q = DnsQuery::new(1, b"example.com", TYPE_A).expect("hợp lệ");
    let mut small = [0u8; 20];
    assert_eq!(q.encode(&mut small), Err(DnsError::BufferTooSmall));
    let mut tiny = [0u8; DNS_HEADER_LEN - 1];
    assert_eq!(
        DnsHeader::query(1).encode(&mut tiny),
        Err(DnsError::BufferTooSmall)
    );
}

#[test]
fn dns_nen_ten_doc_lai() {
    let mut buf = Vec::new();
    buf.extend_from_slice(&reply_header(0x1234, 1, 1));
    buf.extend_from_slice(&QUESTION_EXAMPLE);
    buf.extend_from_slice(&[
        0xc0, 0x0c, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x3c, 0x00, 0x04, 1, 2, 3, 4,
    ]);
    let (name, next) = Name::parse_at(&buf, 29).expect("giải nén");
    assert_eq!(name.as_bytes(), b"example.com");
    assert_eq!(next, 31);
}

#[test]
fn dns_question_roundtrip_va_name_encode() {
    let q = DnsQuestion {
        name: Name::from_dotted(b"a.bc").expect("hợp lệ"),
        qtype: TYPE_A,
        qclass: CLASS_IN,
    };
    let mut buf = [0u8; 32];
    let n = q.encode(&mut buf).expect("vừa");
    assert_eq!(n, 10);
    assert_eq!(&buf[..n], &[1, b'a', 2, b'b', b'c', 0, 0, 1, 0, 1]);
    let (parsed, next) = DnsQuestion::parse(&buf[..n], 0).expect("parse");
    assert_eq!(parsed.name.as_bytes(), b"a.bc");
    assert_eq!(next, n);
}

#[test]
fn dns_ten_loi_on_dinh() {
    assert_eq!(DnsError::Truncated.name(), "dns-truncated");
    assert_eq!(DnsError::BadName.name(), "dns-bad-name");
    assert_eq!(DnsError::BadPointer.name(), "dns-bad-pointer");
    assert_eq!(DnsError::PointerLoop.name(), "dns-pointer-loop");
    assert_eq!(DnsError::BadCount.name(), "dns-bad-count");
    assert_eq!(DnsError::BadRdlength.name(), "dns-bad-rdlength");
    assert_eq!(DnsError::NotResponse.name(), "dns-not-response");
    assert_eq!(DnsError::BadId.name(), "dns-bad-id");
    assert_eq!(DnsError::BufferTooSmall.name(), "dns-buffer-too-small");
}

#[test]
fn dns_arcord_khop_va_khong_khop() {
    let a = ARecord {
        addr: Ipv4Addr::new(10, 0, 2, 3),
        ttl: 7,
    };
    assert_eq!(a.addr, Ipv4Addr::new(10, 0, 2, 3));
    assert_eq!(a.ttl, 7);
    assert_ne!(a.addr, Ipv4Addr::new(10, 0, 2, 4));
}
