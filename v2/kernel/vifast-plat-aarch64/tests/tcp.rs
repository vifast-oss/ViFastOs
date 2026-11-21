// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho `vifast_plat_aarch64::tcp` + hằng số UDP/DNS của `virtio_net` — KT-289.
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64 --test tcp
//! ```
//!
//! Phần **thuần** của TCP/DNS được chốt trước khi chạm phần cứng (D2-05). Máy trạng thái TCP đã
//! có 43 test host ở `vifast-net` (KT-268) — tệp này **không** chép lại; nó ghim những gì thuộc
//! **chính sách của driver aarch64**:
//!
//! 1. **Hằng số khớp runner**: cổng server `45678` (runner dựng `python3` trên
//!    `127.0.0.1:45678`), cổng client `0x4b57`, ISS tất định, cửa sổ nhận, MSS.
//! 2. **Ngân sách retransmit**: `TCP_RETRANSMIT_US = 3 s / 4` ⇒ 3 lần retransmit vẫn nằm trong
//!    **một** bước 3 s (fail-closed, không treo) — cùng bất biến `tcp.rs` riscv64 KT-268.
//! 3. **`past_deadline`**: so sánh an toàn với wrap của `CNTPCT_EL0`; `tcp.rs` dùng **chung**
//!    định nghĩa này (không có bản thứ hai lệch ngữ nghĩa).
//! 4. **Hằng số DNS**: server `10.0.2.3:53` của slirp, payload tối đa 512 B (RFC 1035 §4.2.1),
//!    cổng nguồn UDP cố định.

// Test này **ghim hằng số** của driver (cổng, ngân sách thời gian) — khẳng định trên hằng là mục
// đích, không phải sơ suất (cùng khuôn `tests/virtio_net.rs`).
#![allow(clippy::assertions_on_constants)]

use vifast_net::addr::Ipv4Addr;
use vifast_plat_aarch64::tcp::{
    TCP_CLIENT_PORT, TCP_ISS, TCP_MAX_PAYLOAD, TCP_RCV_WINDOW, TCP_RETRANSMIT_US, TCP_SERVER_PORT,
};
use vifast_plat_aarch64::virtio_net::{
    past_deadline, DNS_PORT, DNS_SERVER_IP, NET_STEP_TIMEOUT_US, UDP_CLIENT_PORT, UDP_MAX_PAYLOAD,
};

// ---------------------------------------------------------------- 1. hằng số khớp runner

#[test]
fn tcp_constants_match_runner() {
    // Runner (`run-aarch64.sh`/`soak-aarch64.sh`) dựng server trên `127.0.0.1:45678`; khách nối
    // `10.0.2.2:45678`. Lệch cổng ⇒ TAP 33 đỏ dù driver đúng — ghim để bắt lệch âm thầm.
    assert_eq!(TCP_SERVER_PORT, 45678);
    assert_eq!(TCP_CLIENT_PORT, 0x4b57);
    // ISS tất định (chưa có RNG) — đổi giá trị là đổi hành vi trên dây, phải là quyết định có ý.
    assert_eq!(TCP_ISS, 0x4b54_0001);
    // Cửa sổ nhận quảng bá (không window scaling) + MSS không option: 1500 − 20 IPv4 − 20 TCP.
    assert_eq!(TCP_RCV_WINDOW, 4096);
    assert_eq!(TCP_MAX_PAYLOAD, 1460);
}

// ---------------------------------------------------------------- 2. ngân sách retransmit

#[test]
fn retransmit_budget_fits_one_step() {
    // Một bước mạng = 3 s; retransmit cách nhau 3 s / 4 ⇒ **ba** lần retransmit vẫn trong bước.
    assert_eq!(NET_STEP_TIMEOUT_US, 3_000_000);
    assert_eq!(TCP_RETRANSMIT_US, NET_STEP_TIMEOUT_US / 4);
    assert_eq!(TCP_RETRANSMIT_US, 750_000);
    assert!(
        TCP_RETRANSMIT_US * 3 <= NET_STEP_TIMEOUT_US,
        "3 lần retransmit ({}) vượt ngân sách một bước ({NET_STEP_TIMEOUT_US})",
        TCP_RETRANSMIT_US * 3
    );
}

// ---------------------------------------------------------------- 3. past_deadline

#[test]
fn past_deadline_semantics() {
    // Chưa tới hạn.
    assert!(!past_deadline(100, 200));
    // Đúng mốc hạn ⇒ coi là **đã** qua (không chờ thêm) — fail-closed.
    assert!(past_deadline(200, 200));
    // Quá hạn.
    assert!(past_deadline(300, 200));
    // Wrap của bộ đếm: `now` nhỏ hơn `deadline` về số học nhưng đã qua **sau** khi wrap.
    assert!(past_deadline(0, u64::MAX));
    // Nửa vòng còn lại: `now` lớn hơn `deadline` nhưng **chưa** qua (deadline ở tương lai xa).
    assert!(!past_deadline(u64::MAX, 0));
}

// ---------------------------------------------------------------- 4. hằng số DNS

#[test]
fn dns_constants_match_slirp() {
    // `-netdev user` của QEMU: DNS server `10.0.2.3:53` (dữ kiện nền tảng, cùng riscv64 KT-260).
    assert_eq!(DNS_SERVER_IP, Ipv4Addr::new(10, 0, 2, 3));
    assert_eq!(DNS_PORT, 53);
    // DNS không EDNS: 512 byte (RFC 1035 §4.2.1).
    assert_eq!(UDP_MAX_PAYLOAD, 512);
    // Cổng nguồn UDP cố định (chưa có RNG; còn kiểm địa chỉ/cổng đích).
    assert_eq!(UDP_CLIENT_PORT, 0x4b54);
}
