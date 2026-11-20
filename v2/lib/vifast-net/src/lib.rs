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
//! - `#![no_std]`, `#![deny(unsafe_code)]`, **không** dependency ngoài (D2-21).
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
//! | [`frame`] | bọc header Ethernet + payload |
#![no_std]
#![deny(unsafe_code)]

pub mod addr;
pub mod arp;
pub mod checksum;
pub mod dhcp;
pub mod dns;
pub mod eth;
pub mod frame;
pub mod icmp;
pub mod ipv4;
pub mod udp;
