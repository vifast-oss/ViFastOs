// SPDX-License-Identifier: MIT OR Apache-2.0
//! `vifast-net` — **codec mạng thuần** cho D1 (KT-244): Ethernet II, ARP, IPv4, ICMP echo, UDP,
//! DHCP. Không MMIO, không `unsafe`, không dependency ngoài (D2-21).
//!
//! # Vì sao tách crate này
//!
//! `kernel/vifast-plat-riscv64` chứa `unsafe` (MMIO/asm) nên **không** chạy host test. Phần
//! *quyết định* của mạng — địa chỉ, checksum Internet, encode/decode từng header, kiểm độ dài và
//! trường bắt buộc — là logic **thuần** trên byte. Tách ở đây để test host chạy được (D2-05: đỏ
//! trước rồi mới xanh) và để driver virtio-net (KT-245) chỉ còn phần chạm thanh ghi.
//!
//! # Nguyên tắc
//!
//! - `#![no_std]`, `#![forbid(unsafe_code)]` (KT-335: nâng từ `deny` — không có đường
//!   `allow(unsafe_code)` nào lọt vào crate logic), **không** dependency ngoài (D2-21).
//! - Mọi codec làm việc trên **lát byte có sẵn**: crate này không cấp phát, không biết địa chỉ
//!   vật lý, không tự gửi. `DhcpMessage` là ngoại lệ duy nhất: nó **sở hữu** một khung 576 byte
//!   (RFC 951) vì thông điệp DHCP được dựng rồi mới gửi.
//! - Mã lỗi in qua `name()` (D2-102: không in số trần trong evidence).
//! - **Từ chối phân mảnh**: `ipv4::Ipv4Header::parse` trả `Unsupported` khi `MF` bật hoặc
//!   `frag_offset` khác không — D1 chưa có tầng ghép mảnh, im lặng nhận mảnh là sai.
//! - Checksum Internet (RFC 1071) cộng dồn **từng mảnh** được, nên pseudo-header UDP/TCP không
//!   phải sao chép payload vào bộ đệm tạm.
//!
//! # Bản đồ module
//!
//! | Module | Nội dung |
//! |---|---|
//! | [`addr`] | `MacAddr`, `Ipv4Addr`, `SubnetMask`, `same_subnet` |
//! | [`checksum`] | checksum Internet 16-bit một bù, cộng dồn từng mảnh |
//! | [`eth`] | header Ethernet II 14 byte |
//! | [`arp`] | ARP cho IPv4/Ethernet 28 byte |
//! | [`ipv4`] | header IPv4 20 byte, từ chối phân mảnh |
//! | [`icmp`] | echo request/reply (type 8/0) |
//! | [`udp`] | header UDP 8 byte + checksum pseudo-header IPv4 |
//! | [`dhcp`] | BOOTP/DHCP: DISCOVER/REQUEST + đọc OFFER/ACK |
//! | [`dns`] | DNS RFC 1035: query `A`, parse reply, giải nén tên có phát hiện vòng lặp |
//! | [`tcp`] | header TCP 20 byte + checksum pseudo-header + máy trạng thái client (RFC 793) |
//! | [`tcp_client`] | **KT-335** chính sách client TCP (bắt tay/gửi/nhận/đóng/retransmit) trên trait `Clock` + `Link` |
//! | [`client`] | **KT-335b** nền chính sách mạng: trait `Clock`/`Link` (dùng lại), `RetryBudget` ba trần, `NetPeer`, `udp_frame`/`udp_send`/`udp_recv_until` |
//! | [`dhcp_client`] | **KT-335b** DHCP client (DISCOVER/OFFER/REQUEST/ACK) trên `RetryBudget`, `DhcpLease` |
//! | [`arp_client`] | **KT-335b** ARP request/reply + resolve (một request, fail-closed) |
//! | [`icmp_client`] | **KT-335b** ICMP echo + bộ lọc `id`/`seq`/payload + ping có trần |
//! | [`udp_client`] | **KT-335b** UDP send/recv/echo ("gương" UDP của bằng chứng mạng) |
//! | [`dns_client`] | **KT-335b** DNS query `A` + **D2-246** (reply hỏng ⇒ đếm & thử lại) + bộ lọc reply |
//! | [`frame`] | bọc header Ethernet + payload |
#![no_std]
#![forbid(unsafe_code)]

pub mod addr;
pub mod arp;
/// **KT-335b** chính sách ARP dùng chung 3 arch (request/reply/resolve).
pub mod arp_client;
pub mod checksum;
/// **KT-335b** nền chính sách mạng: `Clock`/`Link` (dùng lại từ `tcp_client`), `RetryBudget`,
/// `NetPeer`, ghép/đọc datagram UDP.
pub mod client;
pub mod dhcp;
/// **KT-335b** DHCP client dùng chung 3 arch (ba trần `RetryBudget`).
pub mod dhcp_client;
pub mod dns;
/// **KT-335b** DNS client dùng chung 3 arch — **giữ hợp đồng D2-246** (reply hỏng ⇒ đếm & thử
/// lại trong ngân sách; hết ngân sách mà chỉ thấy gói hỏng ⇒ `BadFrame`, không thấy gói nào ⇒
/// `Timeout`).
pub mod dns_client;
pub mod eth;
pub mod frame;
pub mod icmp;
/// **KT-335b** ICMP echo dùng chung 3 arch (bộ lọc `id`/`seq`/payload + ping có trần).
pub mod icmp_client;
pub mod ipv4;
pub mod tcp;
/// **KT-335** chính sách **client TCP** — bắt tay/gửi/nhận/đóng + retransmit trên trait
/// [`tcp_client::Clock`] + [`tcp_client::Link`] (D2-231: hết hai bản sao ở `vifast-plat-*`).
pub mod tcp_client;
pub mod udp;
/// **KT-335b** UDP send/recv/echo dùng chung 3 arch.
pub mod udp_client;
