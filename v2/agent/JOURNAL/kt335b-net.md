# KT-335b — Hợp nhất chính sách DHCP/ARP/ICMP/UDP/DNS về `vifast-net` (bảo toàn `D2-246`)

**Chu kỳ 2 — Đợt 3.** Người làm: sub-agent `net335b`. Ngày: 2026-11-23 (giờ máy Asia/Tokyo).

| | |
|---|---|
| Worktree | `/tmp/kt335b-s1` (nhánh `task/kt335b-net`) |
| Base | `main` @ `3a8219ac` |
| Máy | macOS (Apple Silicon), Rust/cargo `1.100.0` (ghim `v2/rust-toolchain.toml`) |
| Lệnh lib | `cargo --manifest-path v2/Cargo.toml -p vifast-net` / `-p vifast-virtio` |
| Lệnh kernel | `cargo --manifest-path v2/kernel/Cargo.toml --target <arch>` |
| Nhãn số liệu | `wc -l`/`cargo test`/runner đo trực tiếp; log trong `/tmp/kt335b-*.log` |

---

## 0. TL;DR

| Hạng mục | Trước | Sau |
|---|---:|---:|
| Bốn tệp chính sách trong `kernel/` (3 `virtio_net.rs` + `net_client.rs`) | **3.269 dòng** | **1.656 dòng** (**−49,3 %**) |
| Chính sách dùng chung (6 module mới ở `vifast-net`) | 0 | **1.631 dòng** (host test được) |
| Tổng kernel + shared | 3.269 | 3.287 (**+0,6 %** — xem §8: mỗi hàm chỉ còn **một** bản, nhưng bản chung có tài liệu + hai chế độ reply; **không** tuyên bố giảm tổng) |
| Host test mới | — | **9** `vifast-net::tests::net_client` + **1** netdev RX/TX hai chiều |
| Host test cũ giữ nguyên xanh | — | x86_64 **36/36** (13 DHCP + 14 DNS + 9 ICMP); aarch64 **7/7**; `vifast-net` **137/137**; `vifast-virtio::netdev` **10/10** (1 ca mới) — **190 ca** chạy |
| Đột biến | — | **2/2 ĐỎ** (bỏ checksum UDP; bỏ retry DNS — chứng minh `D2-246` còn răng) |
| TAP mạng giữ nguyên | — | x86_64 **36**, aarch64 **35**, riscv64 matrix **27/27** |
| `./v2/bin/v2 check` | xanh ở base | **PASS** (§9.1) |
| `./v2/bin/v2 gate --dev` (T1) | — | **PASS** (§9.2) |
| Ba runner | — | x86_64 **36/36** (+**32/32** âm); aarch64 **32/32** (+**25/25** âm, `tests=35`); riscv64 matrix **27/27**; thêm `net.sh` **4/4** (§9.3) |

**Kết luận**: chính sách DHCP/ARP/ICMP/UDP/DNS đã rời khỏi ba bản `vifast-plat-*` và nằm **một**
chỗ trong `vifast-net` trên **trait `Clock` + `Link`** (dùng lại nguyên từ KT-335) và
**`RetryBudget`** chung. Ba arch chỉ còn adapter `Tick`/`Micro`/`NetIo` → `Clock`/`Link` + hàm mỏng
giữ **nguyên** chữ ký cũ, nên TAP/runner không đổi. **`D2-246` được port nguyên** (đếm
`bad_replies`, thử lại trong ngân sách, chỉ `BadFrame` khi hết ngân sách mà chỉ thấy gói hỏng;
không thấy gói nào ⇒ `Timeout`) — hai host test của KT-335c vẫn chạy và **đột biến (ii) làm cả
bản chung lẫn bản x86_64 ĐỎ**.

---

## 1. Bối cảnh & phạm vi

`D2-231` đã chốt hợp nhất tầng driver (KT-334/335/337) và **đưa chính sách TCP client** về
`vifast-net` (KT-335). KT-335b là phần còn lại: **DHCP/ARP/ICMP/UDP/DNS** vẫn nằm trong ba tệp
platform, **khác nhau đủ** để không thể `cp` cơ học:

1. **Hợp đồng ba trần thời gian** khác nhau (bảng §2.1);
2. x86_64 có `NetIo` (trait cho host test) còn riscv64/aarch64 gọi thẳng `send_frame`/`recv_frame_wait`;
3. aarch64 `pub` `dhcp_frame*`/`arp_parse_reply` cho host test, riscv64 để `fn` private;
4. `DnsAnswer`/`PingStats`/`NetConfig4` mỗi arch một hình dạng;
5. **`D2-246` (KT-335c)** mới chỉ có ở x86_64: reply DNS hỏng ⇒ đếm `bad_replies` + thử lại trong
   ngân sách, không bỏ cả truy vấn; TAP in `bad=<n>`.

Phạm vi KT-335b: gộp **chính sách** (không gộp codec — codec đã ở `vifast-net` từ KT-244/260/268/
289/294), giữ nguyên TAP/API ảnh, giữ `#![forbid(unsafe_code)]` cho `vifast-net`/`vifast-virtio`,
không nới ngưỡng nào, không sửa `spec/`/19 tệp legacy/`STATE`/`QUEUE`/`DECISIONS`/`v2/bin/v2`.

---

## 2. Khảo sát trước khi gộp

### 2.1 Ba trần thời gian của ba bản (đơn vị gốc của từng arch)

| Bước | riscv64 (KT-246/260/268/294) | aarch64 (KT-275/289/294) | x86_64 (KT-326/328 + KT-335c) |
|---|---|---|---|
| Đồng hồ | tick timebase (`Riscv64Virt::now_ticks`), hằng `NET_STEP_TIMEOUT_TICKS = 3 × TIMEBASE_HZ` | µs `CNTPCT_EL0` (`Micro`), hằng `NET_STEP_TIMEOUT_US = 3_000_000` | µs PIT **do ảnh truyền vào** (`now: &mut dyn FnMut() -> u64`) |
| DHCP `TOTAL` | **không có**; mỗi pha một ngân sách `timeout_ticks` | **không có**; mỗi pha `timeout_us` | `DHCP_TOTAL_TIMEOUT_US = 6_000_000` |
| DHCP `ATTEMPT` | `timeout_ticks` (= 3 s ảo) mỗi pha | `timeout_us` (= 3 s) mỗi pha | `DHCP_ATTEMPT_TIMEOUT_US = 1_000_000`, **3** lần/pha (`DHCP_MAX_ATTEMPTS`) |
| DHCP `MAX_ATTEMPTS` | **1** (không gửi lại) | **1** | **3** mỗi pha; `attempts` in ra bị chặn `≤ 2×3` |
| ARP | một request, `timeout_ticks` | một request, `timeout_us` | một request, `timeout_us` (gọi `arp_resolve(..., 3 s, now)`) |
| ICMP | `ping(count=3)`, **mỗi echo** `timeout_ticks`, 1 lần gửi | như riscv64, µs | `ping` **một** echo; `ICMP_TOTAL=6 s`/`ATTEMPT=1 s`/`MAX=3`; TAP in `attempts` |
| UDP | `udp_echo(..., timeout_ticks)`; ngân sách tính **cả bước** (KT-268) | như riscv64, µs | không có bước UDP riêng; `udp_frame` dùng cho DHCP/DNS |
| DNS | `timeout_ticks`/lần, `DnsRetry` tối đa `DNS_MAX_ATTEMPTS = 2`; **chỉ timeout** mới thử lại | như riscv64, µs | `DNS_TOTAL=6 s`/`ATTEMPT=3 s`/`MAX=2`; **`D2-246`**: gói hỏng cũng thử lại, đếm `bad_replies`, TAP in `bad=<n>` |
| Ai gọi | `vifast-kbin-riscv64/src/ktest_net.rs` (TAP 3–8 + chẩn đoán), `ktest_netsrv.rs` (netsrv) | `vifast-kbin-aarch64/src/net.rs` (TAP 22–35) | `vifast-kbin-x86_64/src/net.rs` (TAP 15–17, 31–33) |

### 2.2 `NetIo` (x86_64) vs `dhcp_frame*` (aarch64) vs riscv64

| Khái niệm | x86_64 | aarch64 | riscv64 |
|---|---|---|---|
| Đường frame/đồng hồ cho host test | **`trait NetIo { now_us(&mut); send(&mut, frame, timeout_us); recv(&mut, out, timeout_us) }`** + `VirtioNetIo` bọc PIT | không có trait; gọi thẳng `send_frame(frame, timeout_us)`/`recv_frame_wait(out, timeout_us)` (asm) | không có trait; gọi thẳng `send_frame(frame)`/`recv_frame_wait(out, timeout_ticks)` |
| `dhcp_frame`/`dhcp_frame_parse` | private (dùng trong driver) | **`pub`** cho `tests/virtio_net.rs` | private |
| `arp_build_request`/`arp_parse_reply` | `arp_build_request` pub, `arp_parse_reply` private | cả hai **`pub`** (host test khoá bẫy `ar_spa` vs `ar_tip`) | private |
| `DnsAnswer` | `{ reply: DnsReply { id, rcode, question, a }, reply_len, rtt_us, attempts, bad_replies, filter_id/qr/rcode }` | `{ id, rcode, question, a: Option<ARecord>, reply_len, rtt_ms }` | như aarch64 |
| `PingStats` | `{ id, seq, sent, received, payload_bytes, rtt_us, filter_id/seq/payload }` | `{ sent, received, rtt_min_ms, rtt_max_ms }` | như aarch64 |
| `NetConfig4` | không có (dùng `DhcpLease` trực tiếp) | `{ ip, gw, mask, server }` | `{ ip, gw, mask, server }` |
| Thời gian chờ | tham số `now` + `timeout_us` truyền từ ảnh | `now_us()` trong crate arch | `Riscv64Virt::now_ticks()` |

### 2.3 Chính sách **giống nhau về ngữ nghĩa** (gộp được, khác đúng đơn vị/đồng hồ)

| Chính sách | Kết luận khảo sát |
|---|---|
| ARP request/reply | **Giống**: cùng codec `ArpPacket`, cùng điều kiện nhận (`oper=2`, `ar_spa=target`, `ar_tpa=our`, `ar_tha=our_mac`, MAC nguồn ≠ 0/broadcast), cùng bẫy `ar_spa` vs `ar_tip` (KT-246). Chỉ khác tên hằng timeout. |
| ICMP echo | **Giống**: payload 32 byte `0..31`, `id=0x4b54`, `seq` từ 1, cùng bộ lọc `id`/`seq`/payload, cùng checksum codec; khác cách đếm (`count=3` vs một echo + `attempts`). |
| UDP frame | **Giống**: cùng `udp_frame` (Ethernet+IPv4+UDP, checksum pseudo-header), khác cỡ đệm và tên `UDP_MAX_PAYLOAD` (512) vs `DHCP_MAX_LEN` (576). |
| DHCP DISCOVER/REQUEST + parse | **Giống**: cùng codec `DhcpMessage`, cùng lọc `xid`/`chaddr`/`msg_type`; khác chiến lược thử lại và điều kiện lease. |
| DNS query/parse/filter | **Giống**: cùng `DnsQuery`/`DnsMessage::parse_response`/`first_a`; khác **chế độ** reply (x86_64 đòi `RCODE=0` + `A` = hằng; riscv64/aarch64 "đọc được" mọi `RCODE`/`A` tuỳ chọn) và **D2-246** (chỉ x86_64 có). |

---

## 3. Hợp đồng chọn được và **arch nào đổi hành vi**

**Hợp đồng chung**: **ba trần `RetryBudget { total, attempt, max_attempts }` của x86_64** (bản đầy
đủ nhất, đã có host test đo trần từ KT-326/328) cho **DHCP/ICMP/DNS**; `ARP`/`UDP` nhận **một**
`timeout` (đúng bản cũ, không có thử lại). Kiểu/ngữ nghĩa hợp nhất:

| Hạng mục | Hợp đồng mới | riscv64 | aarch64 | x86_64 |
|---|---|---|---|---|
| `Clock`/`Link` | dùng **lại** `vifast_net::tcp_client::{Clock, Link, LinkError}` (KT-335) | adapter `Tick`/`Wire` (tick) | adapter `Micro`/`Wire` (µs) | adapter `NetIo` → `Bridge` (`RefCell`, µs) |
| `RetryBudget` | `client::RetryBudget` (3 trường, `is_zero` = "không ngân sách") | DHCP/ICMP: `{2×timeout, timeout, 1}`; DNS: `{2×timeout, timeout, DNS_MAX_ATTEMPTS}` | như riscv64 (µs) | **số cũ**: DHCP 6 s/1 s/3; ICMP 6 s/1 s/3; DNS 6 s/3 s/2 |
| `DnsAnswer` | hợp của ba bản: `id, rcode, question, a: Option<ARecord>, rtt_us, rtt_ms, reply_len, attempts, bad_replies, filter_*` | `None` = chế độ "đọc được" | `None` | `Some(expect_a)` = chế độ chặt + selfcheck |
| `PingStats` | hợp: `id, seq, sent, received, payload_bytes, rtt_us, rtt_min_ms, rtt_max_ms, filter_*` | `count=3`, mỗi echo 3 s, 1 lần | như riscv64 | `count=1`, ba trần |
| `DhcpLease`/`NetConfig4` | `DhcpLease` chung (đủ 6 trường + `attempts` + 2 cờ selfcheck); `NetConfig4` giữ ở adapter aarch64/riscv64 | convert | convert | dùng trực tiếp |

### 3.1 Hành vi **đổi** ở arch nào (ghi rõ, không nới ngưỡng)

1. **riscv64 + aarch64 — DNS reply hỏng**: trước đây một reply sai/không khớp ⇒ trả `BadFrame`
   **ngay** (bỏ cả truy vấn); nay **đếm `bad_replies` + thử lại** trong cùng ngân sách (tối đa 2
   lần như cũ). Đây là **chủ đích**: `D2-246` là hợp đồng chung sau sự cố soak `1/600` boot
   x86_64; áp cho hai arch kia chỉ **giảm** khả năng đỏ oan, không đổi đường thành công/TAP (runner
   aarch64 chấp nhận `attempts=(1|2)`), không tăng trần (tổng vẫn `2 × 3 s`).
2. **riscv64 + aarch64 — DHCP lease chặt hơn**: `dhcp_lease_from_ack` chung đòi đủ
   mask/router/DNS/lease/server, `valid()` (cùng mạng, không /0-/32, lease > 0) — đúng hợp đồng
   x86_64 (TAP 31). Đường thành công của slirp như cũ; chỉ ACK **thiếu trường** mới khác (trước:
   aarch64/riscv64 chấp nhận thiếu DNS/lease, nay `Io`). Không nới ngưỡng.
3. **riscv64 + aarch64 — DNS reply phải chép đúng câu hỏi**: policy chung đòi
   `question.name == name`/`qtype=A`/`qclass=IN` (x86_64 đã đòi từ KT-328). Với "gương" UDP của
   riscv64 (QNAME = payload) đây chính là điều TAP 8 vốn kiểm ở call-site — nay policy giữ hộ,
   nên nếu server không chép question thì truy vấn thành `BadFrame` thay vì trả `Ok` rồi TAP đỏ.
4. **x86_64 — timeout gửi**: `Link::send_frame` không có tham số timeout (hợp đồng KT-335), nên
   adapter dùng trần poll `used` của driver (`POLL_TIMEOUT_US = 2 s`) thay cho `attempt_budget`
   truyền vào `io.send` trước đây. Đường thành công không đổi (gửi xong ~µs); **số TAP
   (`attempts`, `elapsed_us`) không đổi** vì ngân sách **nhận** vẫn đo sau khi gửi, và bản thân
   phép gửi trước đây cũng chỉ poll `used` (trần 2 s của driver).
5. **x86_64 — DHCP `attempts`**: giữ nguyên công thức cũ (2 pha × tối đa 3 lần); host test
   `dhcp_retry_budget_is_finite_and_attempts_bounded` vẫn xanh.

**Không đổi**: chữ ký hàm công khai của ba adapter (`dhcp_acquire`, `arp_resolve`, `ping`,
`udp_send`/`udp_recv`/`udp_echo`, `dns_query`, các hằng số), mã lỗi in ra (`BadFrame`/`Timeout`/
`Io`), dòng TAP, và `DnsRetry` (vẫn là bộ đếm của **lần gọi**, caller in `attempts`).

---

## 4. Bản đồ gộp

### 4.1 Module mới trong `vifast-net` (đều `#![forbid(unsafe_code)]`, không dependency ngoài)

| Tệp | Dòng | Nội dung |
|---|---:|---|
| `src/client.rs` | 257 | `pub use` `Clock`/`Link`/`LinkError` (từ `tcp_client`), `RetryBudget` (ba trần), `NetPeer`, `attempt_budget`/`past_deadline`, `udp_frame`/`udp_send`/`udp_recv_until` (checksum **thật** ở `UdpHeader::parse(..., true)`) |
| `src/arp_client.rs` | 103 | `arp_build_request`/`arp_parse_reply` (bẫy `ar_spa`)/`arp_resolve` |
| `src/dhcp_client.rs` | 339 | `DhcpLease` + `valid`/`filter_ok`, `dhcp_frame`/`dhcp_frame_parse`/`dhcp_lease_from_ack`, `dhcp_filter_selfcheck`, `dhcp_acquire_with` |
| `src/icmp_client.rs` | 418 | `PingStats`, `icmp_echo_frame`/`icmp_reply_frame`/`icmp_reply_accept`/`icmp_filter_selfcheck`, `ping_with` (một echo có ba trần, hoặc `count` echo mỗi echo một ngân sách) |
| `src/udp_client.rs` | 130 | `UdpEcho`, `send`/`recv`/`echo` (ngân sách **cả bước** theo KT-268) |
| `src/dns_client.rs` | 384 | `DnsReply`/`DnsAnswer`, `dns_reply_accept` (chặt), `check_reply` (chặt/"đọc được"), `dns_filter_selfcheck`, `dns_query_with` **giữ D2-246** |
| **Tổng** | **1.631** | (chưa kể 9 host test + `lib.rs` +21 dòng bản đồ) |

### 4.2 Adapter còn lại (chỉ phần thuộc nền tảng)

| Tệp | Dòng sau | Vai trò |
|---|---:|---|
| `vifast-plat-riscv64/src/virtio_net.rs` | 509 | `Tick`/`Wire` (tick), `NetConfig4`, hàm mỏng giữ chữ ký cũ, `DHCP_XID=0x4b54_3234`, `DNS_SERVER_IP` |
| `vifast-plat-aarch64/src/virtio_net.rs` | 488 | `Micro`/`Wire` (µs), `NetConfig4`, `dhcp_frame*`/`arp_parse_reply` **vẫn pub** (host test), `DHCP_XID=0x4b54_3235`, `DNS_SERVER_IP` |
| `vifast-plat-x86_64/src/virtio_net.rs` | 496 | `NetIo`/`DhcpIo`/`VirtioNetIo`, `Bridge` (`RefCell`) → `Clock`/`Link`, ARP/DHCP adapter, ba trần DHCP, re-export `net_client` |
| `vifast-plat-x86_64/src/net_client.rs` | 163 | adapter ICMP/DNS trên `NetIo` (`ping_with`, `dns_query_with`), ba trần ICMP/DNS, re-export khung/type chung |
| `vifast-kbin-x86_64/src/net.rs` | 554→**560** | chỉ đổi cách đọc `DnsAnswer` (`answer.a` thay `answer.reply.a`) — **chuỗi TAP giữ nguyên 100 %** |

> Vì sao `net_client.rs` giữ lại: `kernel/` có trần R1 ≤ 1.500 dòng/tệp; tách ICMP/DNS adapter khỏi
> driver giữ hai tệp đều ~500 dòng và giữ đúng **một** đường API (`virtio_net` re-export).

### 4.3 Cầu `NetIo` → `Clock`/`Link` (x86_64)

`Clock::now(&self)` + `Link::send_frame(&mut self)` là **hai** trait (KT-335), còn `NetIo` gộp cả
hai trong **một** `&mut dyn`. Adapter dùng `RefCell<&mut dyn NetIo>` với hai wrapper
(`BridgeClock`/`BridgeLink`): mỗi lời gọi mượn ngắn rồi nhả, **không lồng** nên không thể
`BorrowMutError`; chính sách gọi `clock.now()` **rồi** `link.recv_frame()` tuần tự. Không `unsafe`.

---

## 5. Giữ `D2-246` (bắt buộc)

`dns_client::dns_query_with` port **nguyên** vòng lặp của `net_client.rs` (KT-335c):

- reply **hỏng/không khớp** (`id` lệch, `QR=0`, `RCODE≠0`, địa chỉ khác hằng, cắt/trùng/stale) ⇒
  `bad_replies += 1` rồi **`continue`** (thử lại, cùng `id`/message);
- **hết** `budget.max_attempts` mà `bad_replies > 0` ⇒ `BadFrame`; **không** thấy gói nào ⇒
  `Timeout` (đúng cả nhánh "hết ngân sách thời gian" như bản KT-335c — **giữ nguyên**, không "cải
  tiến");
- `DnsAnswer.bad_replies` + `attempts` được trả về; TAP x86_64 in `bad=<n>` y như cũ;
- `DnsRetry` được ghi một lượt **mỗi lần gửi** ⇒ `attempts` mà aarch64/riscv64 in vẫn đúng, **kể
  cả khi lỗi**.

Hai host test của KT-335c vẫn tồn tại ở **cả hai** nơi: `vifast-net/tests/net_client.rs` (bản
chung) và `vifast-plat-x86_64/tests/dns.rs` (bản adapter). Đột biến (ii) §7 chứng minh **cả hai**
ĐỎ khi bỏ retry.

---

## 6. Host test

| Bộ | Số ca | Ghi chú |
|---|---:|---|
| `vifast-net --test net_client` (mới) | **9** | UDP checksum (đột biến i), UDP tràn đệm, **2 ca D2-246**, DNS timeout/zero-budget, DHCP zero-budget, ICMP zero-budget, hằng codec |
| `vifast-virtio --test netdev` | **10** (9 cũ + **1 mới**) | `two_way_rx_tx_on_fake_device`: thiết bị giả bơm `rx`, hoàn tất `tx` sau doorbell, `rx` arm lại, đúng byte hai chiều, `errors/timeouts/id_mismatch = 0` |
| `vifast-net` toàn bộ | **137** | 14 addr/checksum + 13 dhcp + 26 dns + 11 eth/arp + 20 ipv4/icmp/udp + 9 net_client + 43 tcp + 10 tcp_client |
| `vifast-plat-x86_64` DHCP/DNS/ICMP | **36/36** | 13 + 14 + 9 — **không đổi một ca nào**, chỉ đổi 4 dòng truy cập `answer.reply.*` → `answer.*` trong `tests/dns.rs` |
| `vifast-plat-aarch64 --test virtio_net` | **7/7** | `dhcp_frame*`/`arp_parse_reply` vẫn `pub` qua re-export |

Đỏ trước (D2-05): `cargo test -p vifast-net --test net_client` **trước** khi viết module ⇒
`E0432` cho **5** import (`client`, `dhcp_client`, `dns_client`, `icmp_client`, `udp_client`).

---

## 7. Đột biến — cả hai ĐỎ

### (i) Bỏ kiểm checksum UDP — `client.rs::udp_recv_until`

`UdpHeader::parse(ip.header.src, ip.header.dst, ip.payload(), true)` → `false`.

```text
assertion `left == right` failed: đã nhận payload của gói checksum-sai (bỏ kiểm checksum?)
  left: [112, 111, 105, 115, 111, 110, 45, 112, 97, 121, 108, 111, 97, 100]
 right: [118, 105, 102, 97, 115, 116, 45, 117, 100, 112, 45, 111, 107]
failures:
    udp_recv_ignores_bad_checksum
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 8 filtered out
```

### (ii) Bỏ retry DNS — quay lại `?` như bản trước KT-335c

`dns_client::dns_query_with`: thay nhánh `Err(LinkError::BadFrame) => { bad_replies += 1; continue; }`
bằng `let checked = check_reply(...)?;` (một dòng `?` — đúng bản cũ).

```text
# vifast-net (bản chung) — ca D2-246
thread 'dns_query_skips_bad_reply_then_accepts_good_one' panicked at lib/vifast-net/tests/net_client.rs:240:6:
gói xấu đầu tiên KHÔNG được bỏ cả truy vấn (D2-246): BadFrame
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 8 filtered out

# vifast-plat-x86_64 (adapter) — CÙNG tên ca, chạy qua adapter
thread 'dns_query_skips_bad_reply_then_accepts_good_one' panicked at vifast-plat-x86_64/tests/dns.rs:302:10:
gói xấu đầu tiên KHÔNG được bỏ cả truy vấn (D2-246): BadFrame
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 13 filtered out
```

⇒ `D2-246` **còn răng**: bỏ retry ở crate chung làm **cả** test bản chung **và** test adapter x86_64
ĐỎ. Cả hai đột biến đã **phục hồi** ngay sau khi ghi log; cây làm việc không còn dấu vết (kiểm
bằng `git diff` trước khi commit).

---

## 8. Đo dòng trước/sau

Lệnh: `wc -l` trên tệp hiện tại; "trước" = `git show 3a8219ac:<tệp> | wc -l`.

| Tệp | Trước | Sau | Δ |
|---|---:|---:|---:|
| `vifast-plat-riscv64/src/virtio_net.rs` | 840 | **509** | **−331 (−39,4 %)** |
| `vifast-plat-aarch64/src/virtio_net.rs` | 860 | **488** | **−372 (−43,3 %)** |
| `vifast-plat-x86_64/src/virtio_net.rs` | 787 | **496** | **−291 (−37,0 %)** |
| `vifast-plat-x86_64/src/net_client.rs` | 782 | **163** | **−619 (−79,2 %)** |
| **Tổng 4 tệp kernel** | **3.269** | **1.656** | **−1.613 (−49,3 %)** |
| Chính sách dùng chung mới (`vifast-net/src/*_client.rs` + `client.rs`) | 0 | **1.631** | +1.631 |
| **Tổng kernel + shared** | **3.269** | **3.287** | **+18 (+0,6 %)** |
| `vifast-kbin-x86_64/src/net.rs` | 554 | **561** | +7 (đọc `DnsAnswer` hợp nhất) |

**Đọc số cho đúng**: số **tệp platform giảm 49,3 %**; tổng dòng (kể cả module chung) **không
giảm** — mỗi hàm chỉ còn **một** bản nhưng bản chung có tài liệu dài hơn và gánh thêm hai chế độ
reply (chặt/đọc-được) + `count` echo. Giá trị của KT-335b là **một nguồn chính sách + host test**,
không phải giảm LOC. Không tuyên bố "giảm tổng dòng".

---

## 9. Cổng

### 9.1 `./v2/bin/v2 check`

```text
== v2 check (HEAD 3a8219ac) ==
# làn F: fmt … / làn main: clippy … / làn F: luật cấu trúc R1..R5 + tên tệp NTFS (KT-331) …
v2 check: PASS (L1 chạy trong gate --fast)
# step-time-total   108330 ms  TỔNG check
```

Log: `/tmp/kt335b-check2.log` (lượt `check` đầu **FAIL clippy** `single_match` ở `dns_client.rs`
— đã sửa `match` → `if let`, đúng một lỗi, không nới lint).

### 9.2 `./v2/bin/v2 gate --dev` (T1)

**PASS** trên cây đã commit (`cb61c91b`):

```text
--- fmt: OK
--- clippy: OK
--- luật cấu trúc R1..R5 + tên tệp NTFS (KT-331): OK
--- L1 (sim 1.000 seed): OK
--- QEMU aarch64 (K4' bước 3): OK
--- QEMU x86_64 (K5' bước 1): OK
# step-time-total   628713 ms  TỔNG gate --dev (T1)
v2 gate --dev: PASS (T1 — kiểm tích hợp nhanh, **KHÔNG** phải full gate; full gate = 'gate --fast')
```

Log: `/tmp/kt335b-gatedev4.log`. **Không** chạy `gate --fast` (D2-245/D2-248); mọi lượt đều chờ
`measure-lock` trống, không dùng `V2_MEASURE_LOCK=off`.

Ba lượt trước **không phải lỗi mã** — ghi lại đúng để không bán "PASS giả":

1. **Lượt đầu** (cây **chưa commit**): `selftest pre-measure (KT-95)` ĐỎ `exit 7` vì
   `vbrev_dirty=1` — pre-measure cố ý từ chối đo trên cây bẩn. Commit xong thì ca này xanh.
2. **Lượt hai**: `QEMU x86_64` ĐỎ ở **ca âm (b2)** — ảnh `--features ktest,no-wx` mất **cả hai**
   gói DNS (`# net: dns error=Timeout elapsed_ms=6000`), nên ảnh dừng ở TAP 33 **trước** dòng
   `# wx:`, runner báo "đột biến không vào ảnh". Chạy **cùng runner một mình** ngay sau:
   `# 36/36 kiểm sạch (boot=1, tests=36, exc=6)` + `# ca âm: 32/32 ĐỎ đúng như phải`, (b2) xanh
   ⇒ phân loại **T-INFRA** (slirp rớt gói dưới nhiều QEMU song song của gate/agent khác), **không**
   phải hồi quy — không đổi mã, không nới ngưỡng.
3. **Lượt ba**: `QEMU aarch64` ĐỎ vì **host port 45678** đang bị một agent khác giữ (`lsof` thấy
   listener); chờ port trống rồi chạy lại ⇒ lượt bốn PASS. Cùng lý do **T-INFRA**, không sửa mã.

### 9.3 Ba runner

Mỗi runner chạy **một mình** (kiểm `pgrep qemu-system` trước lượt).

| Runner | Dòng tổng kết (nguyên văn) |
|---|---|
| x86_64 | `# 36/36 kiểm sạch (boot=1, tests=36, exc=6)`<br>`# ca âm: 32/32 ĐỎ đúng như phải (+ 1 ca dương KT-313)` |
| aarch64 | `# 32/32 kiểm sạch (boot=1, tests=35)`<br>`# ca âm: 25/25 ĐỎ đúng như phải` |
| riscv64 matrix | `# KTEST: 27/27 boot sạch (smp=matrix, NBOOT=1, ảnh/boot=ktest ktest-guard ktest-user ktest-proc ktest-vblk ktest-con ktest-exec ktest-ipc ktest-netsrv)`<br>`# ktest.sh: PASS` |

Bằng chứng mạng **nguyên văn** (cùng đường bị đổi):

```text
# x86_64 (TAP 31/32/33 — giữ nguyên định dạng cũ)
# net: dhcp lease=86400 ip=10.0.2.15 mask=255.255.255.0 gw=10.0.2.2 dns=10.0.2.3 server=10.0.2.2 attempts=2 elapsed_us=11835 filter=cookie:1 xid:1
# net: icmp target=10.0.2.2 reply=1 id=0x4b54 seq=1 payload=32 rtt_ms=0 attempts=1 elapsed_ms=2 filter=id:1 seq:1 payload:1
# net: dns name=localhost a=127.0.0.1 ttl=0 id=0x4b55 rcode=0 an=1 reply_len=43 rtt_ms=31 elapsed_ms=34 attempts=1 bad=0 const:timeout_ms=3000 filter=id:1 qr:1 rcode:1

# aarch64 (TAP 35)
# net: dhcp ip=10.0.2.15 gw=10.0.2.2 mask=255.255.255.0 server=10.0.2.2
# net: ping 10.0.2.2 ok n=3 rtt=0
# net dns: name=example.com id=0x4b55 rcode=0 an=1 reply_len=61 rtt_ms=20 elapsed_ms=3024 attempts=2 const:timeout_ms=3000

# riscv64 netsrv
# netsrv: ping 10.0.2.2 ok rtt=1
```

Matrix **không** gồm ảnh `ktest-net` (TAP 3–8 UDP/DNS "gương"), nên chạy thêm
`bash v2/test/boot/net.sh` (bước **T2**, chỉ lấy bằng chứng cho đúng đường bị đổi; **không** thay
T1/T2, không đổi ngưỡng): `# net.sh: 4 kiểm đạt, 0 lỗi`.

```text
# net udp echo: target=10.0.2.3:53 via=dns-question-echo payload_len=15 reply_len=33 rcode=0 echoed=1 rtt_ms=489 elapsed_ms=3049 const:timeout_ms=3000
# net dns: name=example.com id=0x4b55 rcode=0 an=1 reply_len=61 rtt_ms=598 elapsed_ms=3061 attempts=2 const:timeout_ms=3000
# net dns: target=192.0.2.1:53 result=Timeout elapsed_ms=6000 attempts=2 const:timeout_ms=3000
```

Dòng thứ nhất chứng minh chế độ "đọc được" (NXDOMAIN/`RCODE≠0` vẫn trả câu hỏi + `echoed`) **giữ
nguyên**; dòng thứ hai chứng minh thử lại **thật** (lần một rớt); dòng thứ ba chứng minh trần
**2 × 3 s** không đổi (6000 ms, `attempts=2`).

---

## 10. Bài học

1. **Ba trần là hợp đồng, không phải hằng số**: gộp bằng `RetryBudget` + **số của từng arch** giữ
   đúng hành vi cũ (riscv64/aarch64 truyền `max_attempts = 1`), chỉ x86_64 giữ 3/2. Nếu ép cùng
   số thì hoặc nới ngưỡng riscv64/aarch64, hoặc làm chậm x86_64 — cả hai đều trái luật.
2. **`?` là chỗ `D2-246` từng chết**: đột biến (ii) chỉ đổi **một** nhánh `match` thành `?` và
   làm ĐỎ đúng ca đã sinh ra từ soak `1/600`; test adapter một mình là chưa đủ nếu policy nằm
   crate khác — phải có **cả** test bản chung **và** test adapter.
3. **Cầu hai trait cần `RefCell`, không cần `unsafe`**: `Clock::now(&self)` + `Link::send_frame(&mut
   self)` trên một `NetIo` gộp được bằng hai wrapper mượn ngắn; thử `Rc::get_mut`/`Weak` cho thiết
   bị giả thì vấp luật "weak cũng chặn `get_mut`" — chuyển vai thiết bị sang `FakeDma` (nơi có
   `&mut`) là lời giải không `unsafe`.
4. **`cargo check -p` trên host đánh lừa với crate kernel**: phải `--target` đúng arch và
   `--features ktest` mới biên dịch module mạng; ba ảnh đều đã typecheck trước khi chạy QEMU.

---

## 11. Trạng thái

- `vifast-net`: **137/137** host test xanh (9 ca mới), clippy sạch, không `unsafe`.
- `vifast-plat-x86_64` **36/36**; `vifast-plat-aarch64` **7/7**; `vifast-virtio::netdev` **10/10**
  (1 ca mới) ⇒ **190 ca** host đã chạy ở lượt này.
- `v2 check` **PASS**; `v2 gate --dev` **PASS**; ba runner xanh (§9); `net.sh` 4/4.
- Hai đột biến **ĐỎ** rồi phục hồi; cây sạch sau commit `cb61c91b` (amend chỉ đổi journal).
- **Không** push/merge/main; **không** `gate --fast` (D2-245/D2-248); **không** nới ngưỡng nào.
