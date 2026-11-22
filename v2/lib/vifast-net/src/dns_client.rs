// SPDX-License-Identifier: MIT OR Apache-2.0
//! **DNS client** dùng chung 3 arch (KT-335b) — truy vấn `A` trên UDP với **ba trần**, bộ lọc
//! reply, và **hợp đồng D2-246** (reply hỏng ⇒ đếm rồi thử lại, không bỏ cả truy vấn).
//!
//! # Vì sao tệp này tồn tại
//!
//! Codec DNS (header/name nén/`question`/`A`, phát hiện vòng lặp) đã ở [`crate::dns`] từ KT-244.
//! Phần **chính sách** trước KT-335b nằm ba bản:
//!
//! | Arch | Bản cũ | Hành vi với reply hỏng | Trần |
//! |---|---|---|---|
//! | riscv64 | `virtio_net.rs` (KT-260/KT-294) | dừng ngay (`BadFrame`, **không** thử lại) | 2 lần × 3 s |
//! | aarch64 | `virtio_net.rs` (KT-275/KT-289/KT-294) | như riscv64 | 2 lần × 3 s |
//! | x86_64 | `net_client.rs` (KT-328) + **D2-246/KT-335c** | **đếm `bad_replies` rồi thử lại** trong ngân sách | 6 s / 3 s / 2 lần |
//!
//! KT-335b chọn **hợp đồng D2-246** làm hợp đồng chung: một reply hỏng/không khớp (stale, cắt,
//! trùng, sai `id`/`QR`/`RCODE`) chỉ là **một gói xấu**, không phải kết luận về cả truy vấn. Chỉ
//! khi **hết** ngân sách/lượt mà vẫn chỉ thấy gói hỏng mới trả [`LinkError::BadFrame`]; không thấy
//! gói nào ⇒ [`LinkError::Timeout`]. Vì sao: soak đầy đủ đã bắt lỗi **thật** `1/600` boot x86_64
//! (`# net: dns error=BadFrame elapsed_ms=24`) — xem D2-246; đây **không** phải "flake của máy".
//!
//! riscv64/aarch64 vì vậy **thay đổi hành vi** đúng một chỗ: reply hỏng trước đây kết thúc truy
//! vấn, nay được thử lại trong cùng ngân sách (tối đa 2 lần như cũ). Đường thành công và dòng TAP
//! không đổi; không nới ngưỡng nào.
//!
//! # Kiểu reply: chặt cho x86_64, "đọc được" cho riscv64/aarch64
//!
//! x86_64 đòi `RCODE=0`, đúng câu hỏi, **và** bản ghi `A` bằng hằng kỳ vọng (`localhost` →
//! `127.0.0.1`) — xem [`dns_reply_accept`]. riscv64 dùng reply DNS làm "gương" UDP với tên
//! `vifast-udp-echo` (server có thể trả NXDOMAIN/NODATA nhưng **vẫn** chép question section);
//! aarch64 hỏi `example.com` (địa chỉ **đổi theo thời gian** nên không có hằng kỳ vọng). Vì vậy
//! `expect_a = None` là chế độ **đọc được**: đòi `id`/`QR`/`qdcount`/câu hỏi đúng, nhưng chấp
//! nhận mọi `RCODE` và `A` tuỳ chọn — đúng hành vi cũ của hai arch đó.
//!
//! # Bất biến
//!
//! - `0` ở bất kỳ trần nào ⇒ `Timeout` **ngay**, không gửi gì (D2-246 không nới "0 = vô hạn").
//! - Số lần gửi bị chặn bởi `budget.max_attempts` và bởi trần codec
//!   ([`crate::dns::DNS_MAX_ATTEMPTS`] = 2) — không có vòng lặp vô hạn.
//! - Reply hỏng không bao giờ được trả như thành công; `bad_replies` đếm được để TAP in `bad=<n>`.

use crate::addr::Ipv4Addr;
use crate::client::{
    attempt_budget, past_deadline, udp_frame, udp_recv_until, Clock, Link, LinkError, NetPeer,
    RetryBudget,
};
use crate::dns::{
    ARecord, DnsMessage, DnsQuery, DnsQuestion, DnsRetry, CLASS_IN, RCODE_NOERROR, TYPE_A,
};
use crate::eth::{ETHERTYPE_IPV4, ETH_HEADER_LEN};
use crate::frame::EthernetFrame;
use crate::ipv4::IPV4_MIN_HEADER_LEN;
use crate::udp::UDP_HEADER_LEN;

/// Cổng UDP của DNS.
pub const DNS_PORT: u16 = 53;
/// Cổng nguồn UDP cố định của client (chưa có RNG; còn kiểm cổng/địa chỉ đích).
pub const DNS_CLIENT_PORT: u16 = 0x4b54;
/// Cỡ payload DNS tối đa không EDNS (RFC 1035 §4.2.1) — trần đệm reply.
pub const DNS_MAX_LEN: usize = 512;
/// Cỡ tối đa một frame UDP mang DNS (Ethernet + IPv4 + UDP + payload 512) — **tính** từ hằng codec.
pub const DNS_FRAME_BYTES: usize =
    ETH_HEADER_LEN + IPV4_MIN_HEADER_LEN + UDP_HEADER_LEN + DNS_MAX_LEN;

/// Reply DNS đã kiểm (một câu hỏi + một bản ghi `A` đúng hằng kỳ vọng).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DnsReply {
    /// `id` của reply — trùng `id` của truy vấn (codec kiểm).
    pub id: u16,
    /// `RCODE` — bước này đòi [`RCODE_NOERROR`].
    pub rcode: u8,
    /// Câu hỏi trong reply (server **chép nguyên**, RFC 1035 §4.1.2).
    pub question: DnsQuestion,
    /// Bản ghi `A` đầu tiên — phải bằng hằng kỳ vọng.
    pub a: ARecord,
}

/// Kết quả **một** truy vấn DNS `A` đã kiểm.
///
/// Là **hợp** của hai bản cũ: x86_64 dùng `attempts`/`bad_replies`/`rtt_us`/`filter_*`; riscv64/
/// aarch64 dùng `a: Option<ARecord>` (reply NXDOMAIN/NODATA vẫn đọc được) và `rtt_ms`.
#[derive(Clone, Copy, Debug)]
pub struct DnsAnswer {
    /// `id` của reply — trùng `id` của truy vấn (codec kiểm).
    pub id: u16,
    /// `RCODE` của reply (`0` ở chế độ chặt; chế độ "đọc được" có thể khác 0).
    pub rcode: u8,
    /// Câu hỏi trong reply (server **chép nguyên**, RFC 1035 §4.1.2).
    pub question: DnsQuestion,
    /// Bản ghi `A` đầu tiên; `None` khi reply không có `A` (chỉ ở chế độ "đọc được").
    pub a: Option<ARecord>,
    /// RTT của reply (đơn vị [`Clock`]).
    pub rtt_us: u64,
    /// RTT của reply (ms) — dòng evidence của riscv64/aarch64 in số này.
    pub rtt_ms: u64,
    /// Độ dài message reply (byte).
    pub reply_len: usize,
    /// Số lần đã **gửi** truy vấn (1..=`budget.max_attempts`).
    pub attempts: u32,
    /// Số reply **hỏng/không khớp** đã bị bỏ rồi **thử lại** (D2-246); `0` ở đường bình thường.
    pub bad_replies: u32,
    /// `true` ⇔ bộ lọc **đã** từ chối một reply sai `id` (đột biến (iii) của KT-328).
    pub filter_id: bool,
    /// `true` ⇔ bộ lọc **đã** từ chối một reply có `QR=0`.
    pub filter_qr: bool,
    /// `true` ⇔ bộ lọc **đã** từ chối một reply có `RCODE≠0` (đột biến (iv)).
    pub filter_rcode: bool,
}

impl DnsAnswer {
    /// `true` ⇔ **cả ba** phép tự kiểm của bộ lọc đều bắt được reply bị đột biến.
    pub const fn filter_ok(&self) -> bool {
        self.filter_id && self.filter_qr && self.filter_rcode
    }
}

/// Quyết định nhận/không nhận một reply DNS.
struct Checked {
    rcode: u8,
    question: DnsQuestion,
    a: Option<ARecord>,
}

/// Kiểm **một** message reply DNS cho truy vấn `(id, name)`.
///
/// `expect_a = Some(addr)` ⇒ chế độ **chặt** (x86_64): `RCODE=0`, câu hỏi đúng `name`/`A`/`IN`,
/// **có** bản ghi `A` và **bằng** `addr`. `expect_a = None` ⇒ chế độ **đọc được** (riscv64/
/// aarch64): vẫn đòi `id`/`QR`/`qdcount`/câu hỏi đúng, nhưng chấp nhận `RCODE` bất kỳ và `A` tuỳ
/// chọn — đúng hành vi cũ của hai arch đó (reply NXDOMAIN của "gương" UDP vẫn phải đọc được).
fn check_reply(
    reply: &[u8],
    id: u16,
    name: &[u8],
    expect_a: Option<Ipv4Addr>,
) -> Result<Checked, LinkError> {
    let message = DnsMessage::parse_response(reply, id).map_err(|_| LinkError::BadFrame)?;
    let header = message.header();
    let question = message
        .question()
        .map_err(|_| LinkError::BadFrame)?
        .ok_or(LinkError::BadFrame)?;
    if question.qtype != TYPE_A || question.qclass != CLASS_IN {
        return Err(LinkError::BadFrame);
    }
    if !question.name.eq_ignore_ascii_case(name) {
        return Err(LinkError::BadFrame);
    }
    let a = message.first_a().map_err(|_| LinkError::BadFrame)?;
    // Chế độ **chặt** (`Some`): `RCODE=0` và bản ghi `A` **bằng** hằng kỳ vọng. Chế độ "đọc
    // được" (`None`): `RCODE`/`A` tuỳ ý (riscv64 "gương" UDP nhận cả NXDOMAIN/NODATA).
    if let Some(expected) = expect_a {
        if header.rcode() != RCODE_NOERROR {
            return Err(LinkError::BadFrame);
        }
        let record = a.ok_or(LinkError::BadFrame)?;
        if record.addr != expected {
            return Err(LinkError::BadFrame);
        }
    }
    Ok(Checked {
        rcode: header.rcode(),
        question,
        a,
    })
}

/// Kiểm **một** message reply DNS **chặt** cho truy vấn `(id, name)` và đòi bản ghi `A` đúng
/// `expect_a` — API của x86_64 (KT-328) giữ nguyên chữ ký.
///
/// Không viết lại codec: [`DnsMessage::parse_response`] đã đòi `QR=1`, `id` khớp, `qdcount=1`;
/// ở đây thêm bốn điều kiện **của bước này**: `RCODE=0`, câu hỏi đúng `name`/`A`/`IN`, có bản ghi
/// `A`, và địa chỉ **bằng hằng kỳ vọng**.
pub fn dns_reply_accept(
    reply: &[u8],
    id: u16,
    name: &[u8],
    expect_a: Ipv4Addr,
) -> Result<DnsReply, LinkError> {
    let checked = check_reply(reply, id, name, Some(expect_a))?;
    Ok(DnsReply {
        id,
        rcode: checked.rcode,
        question: checked.question,
        a: checked.a.ok_or(LinkError::BadFrame)?,
    })
}

/// Bộ lọc tự kiểm **chạy trong ảnh** (TAP 33 của x86_64): bơm lại **chính reply đã nhận** với `id`
/// sai, `QR` bị xoá và `RCODE=3`, rồi đòi [`dns_reply_accept`] từ chối — trả `(id_bị_từ_chối,
/// qr_bị_từ_chối, rcode_bị_từ_chối)`.
///
/// Ba đột biến **chỉ** đổi đúng trường của nó (không đụng độ dài/RDLENGTH), nên "bị từ chối" chỉ
/// có thể do phép kiểm tương ứng. Reply hỏng sẵn ⇒ `(false, false, false)` (không tự khen).
pub fn dns_filter_selfcheck(
    reply: &[u8],
    id: u16,
    name: &[u8],
    expect_a: Ipv4Addr,
) -> (bool, bool, bool) {
    if reply.len() < crate::dns::DNS_HEADER_LEN || reply.len() > DNS_MAX_LEN {
        return (false, false, false);
    }
    if dns_reply_accept(reply, id, name, expect_a).is_err() {
        return (false, false, false);
    }
    let mut poisoned = [0u8; DNS_MAX_LEN];
    let len = reply.len();
    poisoned[..len].copy_from_slice(reply);
    // Byte 0..2 = `id`; byte 2..4 = `flags` (QR ở bit 15, `rcode` ở 4 bit thấp).
    poisoned[0] ^= 0xff;
    let id_ok = dns_reply_accept(&poisoned[..len], id, name, expect_a).is_err();
    poisoned[0] ^= 0xff;
    poisoned[2] &= 0x7f;
    let qr_ok = dns_reply_accept(&poisoned[..len], id, name, expect_a).is_err();
    poisoned[2] |= 0x80;
    poisoned[3] = (poisoned[3] & 0xf0) | crate::dns::RCODE_NXDOMAIN;
    let rcode_ok = dns_reply_accept(&poisoned[..len], id, name, expect_a).is_err();
    (id_ok, qr_ok, rcode_ok)
}

/// Gửi truy vấn DNS `A` cho `name` tới `peer.peer_ip:53` trên **một** đường frame bất kỳ, kiểm
/// reply, với **ba trần** của `budget` — và **hợp đồng D2-246**.
///
/// # D2-246 (bắt buộc giữ)
///
/// Reply **hỏng/không khớp** (`id` lệch, `QR=0`, `RCODE≠0`, địa chỉ khác hằng, cắt/trùng/stale)
/// **không** kết luận gì về cả truy vấn: đếm vào `bad_replies` rồi **thử lại** (cùng `id`, cùng
/// message — RFC 1035 §4.2.1) trong **cùng** ngân sách. Chỉ khi **hết** ngân sách/lượt mà vẫn
/// chỉ thấy gói hỏng mới trả [`LinkError::BadFrame`]; không thấy gói nào ⇒ [`LinkError::Timeout`].
/// Lý do: soak đầy đủ bắt lỗi thật `1/600` boot x86_64 — xem D2-246.
///
/// `reply_out` do **caller** giữ: nhờ vậy phép tự kiểm bộ lọc dùng lại **chính** reply đã nhận,
/// không phải dựng lại (không thể "tự khen").
///
/// `retry` là bộ đếm của **lần gọi này** (tạo mới cho mỗi truy vấn): mỗi lần gửi ghi một lượt;
/// sau khi trả về, `retry.attempts()` là số lần đã gửi — riscv64/aarch64 in nó vào evidence,
/// **kể cả** khi lỗi.
#[allow(clippy::too_many_arguments)] // endpoint + id/tên + kỳ vọng + trần + đệm + bộ đếm
pub fn dns_query_with<C: Clock, L: Link>(
    clock: &C,
    link: &mut L,
    peer: NetPeer,
    id: u16,
    name: &[u8],
    expect_a: Option<Ipv4Addr>,
    budget: RetryBudget,
    reply_out: &mut [u8],
    retry: &mut DnsRetry,
) -> Result<DnsAnswer, LinkError> {
    if budget.is_zero() {
        return Err(LinkError::Timeout);
    }
    let query = DnsQuery::new(id, name, TYPE_A).map_err(|_| LinkError::BadFrame)?;
    let mut request = [0u8; DNS_MAX_LEN];
    let request_len = query
        .encode(&mut request)
        .map_err(|_| LinkError::BadFrame)?;
    let mut frame = [0u8; DNS_FRAME_BYTES];
    let frame_len = udp_frame(
        peer.our_mac,
        peer.peer_mac,
        peer.our_ip,
        peer.peer_ip,
        DNS_CLIENT_PORT,
        DNS_PORT,
        &request[..request_len],
        &mut frame,
    )
    .ok_or(LinkError::BadFrame)?;
    let total_deadline = clock.now().wrapping_add(budget.total_us);
    let mut attempts = 0u32;
    // **D2-246** — reply hỏng/không khớp chỉ là **một gói xấu**, không phải kết luận về cả truy
    // vấn: đếm vào `bad_replies` rồi thử lại trong cùng ngân sách; chỉ khi **hết** ngân sách/lượt
    // mà vẫn chỉ thấy gói xấu mới trả `BadFrame` (không thấy gói nào ⇒ `Timeout` như cũ).
    let mut bad_replies = 0u32;
    loop {
        if attempts >= budget.max_attempts {
            return Err(if bad_replies > 0 {
                LinkError::BadFrame
            } else {
                LinkError::Timeout
            });
        }
        attempts = attempts.saturating_add(1);
        retry.record_attempt();
        let now = clock.now();
        if past_deadline(now, total_deadline) {
            return Err(LinkError::Timeout);
        }
        let _send_budget = attempt_budget(now, total_deadline, budget.attempt_us);
        link.send_frame(&frame[..frame_len])?;
        let sent_at = clock.now();
        if past_deadline(sent_at, total_deadline) {
            return Err(LinkError::Timeout);
        }
        let attempt_deadline = {
            let candidate =
                sent_at.wrapping_add(attempt_budget(sent_at, total_deadline, budget.attempt_us));
            if past_deadline(candidate, total_deadline) {
                total_deadline
            } else {
                candidate
            }
        };
        match udp_recv_until(
            clock,
            link,
            peer.our_ip,
            peer.peer_ip,
            DNS_PORT,
            DNS_CLIENT_PORT,
            reply_out,
            attempt_deadline,
        ) {
            Ok(len) => {
                let checked = match check_reply(&reply_out[..len], id, name, expect_a) {
                    Ok(checked) => checked,
                    Err(LinkError::BadFrame) => {
                        // D2-246: gói xấu ⇒ đếm, **không** bỏ cả truy vấn.
                        bad_replies = bad_replies.saturating_add(1);
                        continue;
                    }
                    Err(error) => return Err(error),
                };
                let a = checked.a;
                let rtt_us = clock.now().wrapping_sub(sent_at);
                // Bộ lọc tự kiểm chỉ có nghĩa ở chế độ chặt (có hằng kỳ vọng) — đúng phạm vi
                // TAP 33 của x86_64; arch khác không in các cờ này.
                let (filter_id, filter_qr, filter_rcode) = match (expect_a, a) {
                    (Some(expected), _) => {
                        dns_filter_selfcheck(&reply_out[..len], id, name, expected)
                    }
                    (None, Some(record)) => {
                        dns_filter_selfcheck(&reply_out[..len], id, name, record.addr)
                    }
                    (None, None) => (false, false, false),
                };
                return Ok(DnsAnswer {
                    id,
                    rcode: checked.rcode,
                    question: checked.question,
                    a,
                    rtt_us,
                    rtt_ms: rtt_us / 1_000,
                    reply_len: len,
                    attempts,
                    bad_replies,
                    filter_id,
                    filter_qr,
                    filter_rcode,
                });
            }
            // Chỉ **timeout** mới thử lại ở đây; reply hỏng đã đếm + `continue` ở trên (D2-246).
            Err(LinkError::Timeout) => continue,
            Err(error) => return Err(error),
        }
    }
}

/// `true` ⇔ frame đọc được thành Ethernet/IPv4/UDP và có cổng nguồn [`DNS_PORT`] — chẩn đoán của
/// ảnh (không phải đường quyết định).
pub fn is_dns_frame(frame: &[u8]) -> bool {
    let Ok(eth) = EthernetFrame::parse(frame) else {
        return false;
    };
    if eth.header.ethertype != ETHERTYPE_IPV4 {
        return false;
    }
    let Ok(ip) = crate::ipv4::Ipv4Header::parse(eth.payload, true) else {
        return false;
    };
    let Ok(udp) = crate::udp::UdpHeader::parse(ip.header.src, ip.header.dst, ip.payload(), true)
    else {
        return false;
    };
    udp.header.src_port == DNS_PORT
}

const _: () = {
    // Payload DNS không EDNS phải vừa trần ghép khung UDP, và frame DNS lớn nhất phải vừa đệm
    // nhận một frame của driver — hai bất biến của KT-328, giữ **một** chỗ.
    assert!(DNS_MAX_LEN <= crate::client::MAX_UDP_PAYLOAD);
    assert!(DNS_FRAME_BYTES <= crate::client::MAX_FRAME_BYTES);
};
