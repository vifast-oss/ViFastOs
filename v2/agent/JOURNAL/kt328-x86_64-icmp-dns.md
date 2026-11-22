# KT-328 — ICMP echo + DNS trên x86_64: TAP 32/33

Trạng thái: **DONE** (KT-328, 2026-11-22, nhánh `task/kt328-x64net`, base `main` @ `60a9e594`).
Quyết định: **D2-216** (sửa phép đo, không nới ngưỡng), **D2-226** (vòng 24 — KT-326 DHCP x86_64;
KT-328 là nợ kế tiếp cùng loại), **D2-215** (DNS retransmit 2 lần × 3 s là **ngữ nghĩa giao thức**
theo RFC 1035 §4.2.1, không phải nới ngưỡng), **D2-198** (uỷ quyền commit trên nhánh task),
**D2-05** (test đỏ trước), **D2-07** (không retry để lấy xanh), **D2-17** (`unsafe` chỉ trong
`kernel/vifast-plat-*`), **D2-21** (không dependency ngoài), **D2-110A/B** (không viết lại codec),
**D2-116** (đo hợp lệ). Luật viết phép kiểm: [`k6-test-invariants.md`](../../docs/k6-test-invariants.md).

> **D2-227 không tồn tại** trong `v2/agent/DECISIONS.md` tại base `60a9e594` (mục cuối là
> **D2-226**). Nhiệm vụ yêu cầu đọc **D2-216**, **D2-226**, **D2-227** ⇒ đã đọc D2-216/D2-226/D2-215
> và **ghi lại việc thiếu D2-227** thay vì tự suy ra nội dung (cùng khuôn KT-326 với D2-226).

**Không** tuyên bố K5' đạt. **Không** push, **không** merge, **không** chạm `main`. **Không** sửa
`v2/agent/{STATE,QUEUE,DECISIONS}.md`, `v2/bin/v2`, `spec/`.

## 1. Khảo sát

### 1.1 aarch64 làm ICMP thế nào (khuôn đã chạy)

`v2/kernel/vifast-plat-aarch64/src/virtio_net.rs:1007-1117` + `vifast-kbin-aarch64/src/net.rs:244-280`
(TAP 25):

| Bước | Mã | Ghi chú |
|---|---|---|
| dựng frame | `icmp_echo_frame` → `IcmpEcho::request(id, seq, payload).encode` + `Ipv4Header::new(PROTO_ICMP)` + `EthernetFrame` | payload 32 byte (`ICMP_PAYLOAD_BYTES`), `id = 0x4b54` **hằng**, `seq` bắt đầu **1** và tăng mỗi vòng |
| ai gọi | `net.rs::run` **sau** ARP (TAP 24): `virtio_net::ping(cfg.ip, our_mac, cfg.gw, gw_mac, PING_COUNT=3, NET_STEP_TIMEOUT_US=3 s)` | một lần trong boot; **chờ từng echo** rồi mới gửi echo kế |
| chờ reply | vòng `recv_frame_wait` tới `deadline = started + timeout_us`; `past_deadline` ⇒ bỏ | frame không phải IPv4/ICMP/`src_ip == target` bị **bỏ qua** |
| kiểm ở reply | `IcmpEcho::parse` (**checksum** trên toàn lát đã cắt `total length`) → `EchoReply` ∧ `echo.id == id` ∧ `echo.seq == seq` | **không** so `payload`; không kiểm `dst`/MAC |
| trần | **3 s mỗi echo**, và `ping` trả `Timeout` ngay nếu **chưa từng** có reply nào; mất gói giữa loạt ⇒ `Io` | **không** có trần "toàn bước" riêng, **không** retry, **không** đếm lần gửi |
| TAP | TAP 25 `net: ping` = `stats.received == PING_COUNT` | dòng `# net: ping 10.0.2.2 ok n=3 rtt=<ms>` |

### 1.2 aarch64 làm DNS thế nào

`virtio_net.rs:1136-1382` + `net.rs:418-513` (TAP 35):

| Bước | Mã | Ghi chú |
|---|---|---|
| resolve MAC server | `arp_resolve(cfg.ip, DNS_SERVER_IP=10.0.2.3, 3 s)` | in `# net: arp dns=<mac> ip=10.0.2.3` — **không** dùng MAC gateway cho host cùng dạng |
| dựng query | `DnsQuery::new(id, name, TYPE_A).encode` → `udp_send(…, UDP_CLIENT_PORT=0x4b54, DNS_PORT=53, …)` | `id = 0x4b55` hằng, tên `example.com` |
| chờ reply | `udp_recv(our_ip, dns_ip, 53, client_port, out, remaining)` | lọc `PROTO_UDP` + `src_ip`/`dst_ip` + hai cổng; frame khác bỏ qua |
| kiểm ở reply | `DnsMessage::parse_response(reply, id)` (**QR=1**, **`id` khớp**, `qdcount=1`), `question()`, `first_a()`, `rcode` | **địa chỉ KHÔNG được kiểm** — TAP 35 chỉ đòi `rcode == 0` **và** có bản ghi `A` |
| trần | `DnsRetry` (codec): **2 lần**, mỗi lần **3 s**; chỉ **timeout** mới gửi lại (reply hỏng = lỗi giao thức) | KT-294/D2-215: ngân sách 6 s **chỉ khi lần đầu timeout** |
| TAP | TAP 35 `net: dns` + dòng `# net: dns example.com a=<ip> ttl=<n>` | TAP 35 đỏ **không** dừng ảnh (cờ `DNS_RED`) vì còn TAP sau |

### 1.3 x86_64 thiếu gì (đối chiếu)

| Thành phần | Trước KT-328 | Kết luận |
|---|---|---|
| driver gửi/nhận frame | `virtio_net::{send_frame, recv_frame_wait}` (KT-274) | dùng được, **không** sửa driver |
| đồng hồ timeout | `timebase::PitClock` (PIT kênh 2, không ngắt) | dùng được, cùng nguồn với DHCP |
| ghép Ethernet+IPv4+UDP | `udp_frame` (KT-326) | **dùng lại**; chỉ thiếu đường **nhận** UDP tổng quát |
| codec ICMP / DNS | `vifast_net::{icmp, dns}` đã có + host test | **dùng lại**, không viết bản thứ hai |
| ICMP echo client | **không có** | **thiếu thật** ⇒ thêm `ping_with`/`ping` + bộ lọc tự kiểm |
| DNS client | **không có** (x86_64 mới có ARP + DHCP) | **thiếu thật** ⇒ thêm `dns_query_with`/`dns_query` + bộ lọc tự kiểm |
| trần thời gian | DHCP x86_64 có ba trần (KT-326) | áp **cùng khuôn** cho ICMP/DNS |
| chỗ gọi | `net::run` chạy sau `Exit` ring 3; TAP 1..31 đã có | TAP 32/33 nối **cuối** để 1..31 **không đổi** |

**Trả lời câu hỏi của nhiệm vụ:** phần thiếu trên x86_64 **chỉ là** ghép/đọc khung + máy trạng thái
+ trần thời gian; codec, driver, đồng hồ và vị trí TAP đều đã có.

### 1.4 slirp có trả lời ping `10.0.2.2` không? — **CÓ** (ba nguồn)

1. **Đo được trong repo**: riscv64 `# net: ping 10.0.2.2 ok n=3 rtt=0`
   (`v2/agent/JOURNAL/kt246-journal.md:99`) và aarch64 TAP 25 cùng khuôn.
2. **Đo hôm nay trên QEMU 11.1.3 + libslirp**: boot KT-328 in
   `# net: icmp target=10.0.2.2 reply=1 id=0x4b54 seq=1 payload=32 rtt_ms=0 …` ⇒ slirp trả echo
   reply **thật** cho địa chỉ gateway ảo.
3. Không cần dịch vụ host nào: reply đến từ chính slirp (`icmp_input`), nên **không** phụ thuộc
   mạng ngoài — khác DNS (mục sau).

### 1.5 DNS: vì sao hằng kỳ vọng là `localhost → 127.0.0.1`

Nhiệm vụ đòi "**địa chỉ trả về khớp hằng kỳ vọng**". Đo trực tiếp resolver mà slirp dùng
(`nameserver 192.168.2.1`, cùng đường `res_9_*` mà libslirp gọi) hôm nay:

```text
localhost   -> rcode=0 an=1  A 127.0.0.1 ttl=0
example.com -> rcode=0 an=2  A 172.66.147.243 ttl=290, A 104.20.23.154 ttl=290
vifast-udp-echo -> rcode=0 an=0   (NODATA)
```

- `example.com` trả **hai** bản ghi `A` và **đổi theo thời gian** (KT-289/KT-294 đã vì thế mà chỉ
  kiểm `rcode == 0`, không kiểm địa chỉ) ⇒ **không** dùng làm hằng được.
- `localhost` là tên **cục bộ**: mọi resolver (dnsmasq/router/mDNSResponder) trả `127.0.0.1` cho
  `A` mà **không** cần mạng ngoài ⇒ dùng được làm hằng, và **bớt** phụ thuộc upstream so với
  `example.com` (bài học D2-217: 8/1000 boot aarch64 đỏ vì resolver ngoài chậm dưới tải).
- TTL của `localhost` là **0** ⇒ TTL **không** được là điều kiện của TAP 33 (chỉ in ra).

## 2. Thiết kế

### 2.1 Tách module (luật cấu trúc R1)

Driver + ARP + DHCP + ICMP + DNS trong **một** tệp là **1.965 dòng** > trần R1 **1.500**
(`v2/bin/v2` R1) ⇒ `v2 check` **ĐỎ** ở lần chạy đầu. Đã tách phần ICMP/DNS sang
`v2/kernel/vifast-plat-x86_64/src/net_client.rs` (763 dòng); `virtio_net.rs` còn **1.270** dòng và
**re-export** mọi tên của module mới ⇒ ảnh/host test vẫn dùng **một** đường
(`vifast_plat_x86_64::virtio_net::{ping, dns_query, …}`), **không** có API thứ hai.

Ba thứ dùng chung giữa DHCP và ICMP/DNS nằm lại `virtio_net.rs` (một bản, không nhân đôi):
`NetIo` (trait nguồn khung/đồng hồ — bí danh `DhcpIo` giữ cho call-site KT-326),
`VirtioNetIo` (`pub(crate)`, thêm `new(now)`), `attempt_budget` (`pub(crate)`).

### 2.2 Ba trần cho **mỗi** bước (khuôn KT-326)

```rust
// ICMP
pub const ICMP_TOTAL_TIMEOUT_US: u64 = 6_000_000;   // toàn bước
pub const ICMP_ATTEMPT_TIMEOUT_US: u64 = 1_000_000; // mỗi lần gửi/chờ
pub const ICMP_MAX_ATTEMPTS: u32 = 3;               // số lần thử
// DNS
pub const DNS_TOTAL_TIMEOUT_US: u64 = 6_000_000;    // toàn bước = 2 × 3 s
pub const DNS_ATTEMPT_TIMEOUT_US: u64 = 3_000_000;  // KT-294/D2-215: **không** đổi
pub const DNS_MAX_ATTEMPTS: u32 = vifast_net::dns::DNS_MAX_ATTEMPTS as u32; // = 2, suy ra
```

- `RetryBudget { total_us, attempt_us, max_attempts }` truyền vào `ping_with`/`dns_query_with` để
  **host test đo được trần** mà không cần QEMU; `is_zero()` ⇒ `Timeout` **ngay**, không gửi.
- **DNS vì sao 3 s/lần chứ không 1 s**: 3 s là ngân sách **đã công bố và đã đo** cho DNS ở
  KT-294/D2-215 (p99 150 ms, max thành công 286 ms dưới slirp). Hạ xuống 1 s là **siết** ngưỡng đã
  có bằng chứng theo hướng ngược lại và sẽ tạo đỏ-oan dưới tải; trần **toàn bước** vẫn **6 s** như
  khuôn KT-326, và `0` vẫn là "hết ngân sách".
- Mọi vòng lặp so **deadline tuyệt đối**; sau `send` **đo lại** đồng hồ (thời gian gửi cũng tiêu
  ngân sách) — cùng khuôn `dhcp_acquire_with`.

### 2.3 Bộ lọc + phép tự kiểm **trong ảnh** (để đột biến làm QEMU ĐỎ)

| Bước | Predicate (một chỗ) | Tự kiểm in ra | Đột biến làm `0` |
|---|---|---|---|
| ICMP | `icmp_reply_accept`: Ethernet/IPv4 hợp lệ + **checksum IPv4** + `src/dst` MAC+IP đúng + `IcmpEcho::parse` (**checksum ICMP**) = `EchoReply` ∧ `id` ∧ `seq` ∧ **`payload`** | `filter=id:1 seq:1 payload:1` | (i) bỏ `id`, (ii) bỏ `seq` |
| DNS | `dns_reply_accept`: `DnsMessage::parse_response` (**QR=1**, **`id`**, `qdcount=1`) ∧ **`RCODE=0`** ∧ câu hỏi đúng tên/`A`/`IN` ∧ có bản ghi `A` ∧ **`a == hằng`** | `filter=id:1 qr:1 rcode:1` | (iii) bỏ `id`, (iv) bỏ `RCODE` |

Frame độc được dựng **lại** bằng codec (`IcmpEcho::reply` / `DnsHeader` sửa đúng trường) nên
**checksum luôn đúng** — nếu bộ lọc nhận nó thì nguyên nhân **chỉ** có thể là thiếu phép kiểm
tương ứng, không phải lỗi checksum (bẫy "đột biến bị bắt vì lý do khác"). Nếu reply **hợp lệ**
không được nhận, self-check trả `(false,false,false)` — "từ chối tất cả" **không** phải bằng chứng.

### 2.4 Vị trí TAP + dòng bằng chứng

TAP 32/33 chạy **cuối** `net::run` (sau TAP 31 DHCP) ⇒ số hiệu TAP 1..31 **không đổi**. `check(...)`
đỏ ⇒ `halt(3)`, **không** marker (fail-fast hiện có; TAP này là cuối nên không che TAP nào).

```text
# net: icmp target=10.0.2.2 reply=1 id=0x4b54 seq=1 payload=32 rtt_ms=<n> attempts=<n> elapsed_ms=<n> filter=id:1 seq:1 payload:1
ok 32 - net: icmp echo reply target=10.0.2.2 id=0x4b54 seq=1
# net: dns name=localhost a=127.0.0.1 ttl=<n> id=0x4b55 rcode=0 an=1 reply_len=<n> rtt_ms=<n> elapsed_ms=<n> attempts=<n> const:timeout_ms=3000 filter=id:1 qr:1 rcode:1
ok 33 - net: dns a=127.0.0.1 rcode=0
```

`rtt_ms`/`elapsed_ms` **chỉ in**, không phải điều kiện (bài học KT-308/KT-313: không đòi cửa sổ
thời gian). `attempts` chỉ đòi `1..=trần` (số đo, không phải hằng thời gian). Hằng `127.0.0.1` nằm
ở **hai** chỗ (ảnh: `DNS_EXPECT_A`; runner: mẫu dòng) và **không** có chỗ thứ ba.

## 3. Test đỏ trước (D2-05)

Viết **trước** khi cài: `v2/kernel/vifast-plat-x86_64/tests/icmp.rs` (9 test) +
`tests/dns.rs` (13 test). Chạy trên mã HEAD:

```text
error[E0432]: unresolved import `vifast_plat_x86_64::virtio_net::icmp_echo_frame`
   |     no `icmp_echo_frame` in `virtio_net`
   |     no `icmp_filter_selfcheck` in `virtio_net`
   |     no `icmp_reply_accept` in `virtio_net`
   |     no `icmp_reply_frame` in `virtio_net`
   |     no `ping_with` in `virtio_net`
   |     no `NetIo` in `virtio_net`
error: could not compile `vifast-plat-x86_64` (test "icmp") due to 1 previous error
```

```text
error[E0432]: unresolved imports `…::dns_filter_selfcheck`, `…::dns_query_with`,
`…::dns_reply_accept`, `…::NetIo`, `…::NetPeer`, `…::RetryBudget`, `…::DNS_ATTEMPT_TIMEOUT_US`,
`…::DNS_CLIENT_PORT`, `…::DNS_ID`, `…::DNS_MAX_LEN`, `…::DNS_PORT`, `…::DNS_TOTAL_TIMEOUT_US`
error: could not compile `vifast-plat-x86_64` (test "dns") due to 1 previous error
```

Sau khi cài: **9/9 + 13/13 PASS** (log `/tmp/kt328-logs/green-icmp-try1.log`,
`green-dns-try1.log`); `cargo test -p vifast-plat-x86_64` (toàn bộ, gồm `dhcp.rs` 13/13 qua bí danh
`DhcpIo`) **PASS**.

Test có răng ở tầng host (không chỉ QEMU): `dns_reply_accept_rejects_wrong_id`,
`…_rejects_not_response`, `…_rejects_nonzero_rcode`, `…_rejects_wrong_address`,
`…_rejects_other_question`, `icmp_reply_accept_matches_id_seq_and_payload`,
`…_rejects_bad_checksum_and_foreign_source`, `ping_zero_budget_does_not_send`,
`dns_query_zero_budget_does_not_send`, `*_total_budget_is_bounded`.

## 4. Log QEMU nguyên văn

Boot: `-M q35 -cpu qemu64 -m 128M -nographic -no-reboot -smp 4 -kernel <ELF>` +
`-netdev user,id=n0 -device virtio-net-pci,netdev=n0` + đĩa tạm virtio-blk. Trích từ
`/tmp/kt328-logs/run2/boot-1.log` (lần chạy cổng cuối, ảnh sau khi tách module):

```text
# net: mac=52:54:00:12:34:56 mtu=65535 status=0x0001 queues=3 rx=16 tx=16 features=0x0000010130bf8024 driver=0x0000000100000020
# net: arp gw=10.0.2.2 mac=52:55:0a:00:02:02 sent=1 recv=1 elapsed_us=1856 errors=0 timeouts=0
# net: dhcp lease=86400 ip=10.0.2.15 mask=255.255.255.0 gw=10.0.2.2 dns=10.0.2.3 server=10.0.2.2 attempts=2 elapsed_us=8282 filter=cookie:1 xid:1
ok 31 - net: dhcp lease=86400 ip=10.0.2.15 gw=10.0.2.2 dns=10.0.2.3
# net: icmp target=10.0.2.2 reply=1 id=0x4b54 seq=1 payload=32 rtt_ms=0 attempts=1 elapsed_ms=4 filter=id:1 seq:1 payload:1
ok 32 - net: icmp echo reply target=10.0.2.2 id=0x4b54 seq=1
# net: arp dns=52:55:0a:00:02:03 ip=10.0.2.3
# net: dns name=localhost a=127.0.0.1 ttl=0 id=0x4b55 rcode=0 an=1 reply_len=43 rtt_ms=18 elapsed_ms=24 attempts=1 const:timeout_ms=3000 filter=id:1 qr:1 rcode:1
ok 33 - net: dns a=127.0.0.1 rcode=0
# viFastOs x86_64: boot ok
# viFastOs x86_64: halt
```

Đọc dòng theo đúng nghĩa:

- `reply=1` — **một** reply khớp `id`/`seq`/payload của ta (slirp trả lời ping `10.0.2.2`).
- `filter=id:1 seq:1 payload:1` — bộ lọc **trong ảnh** đã từ chối reply bị đổi `id`, đổi `seq`, và
  đổi payload (cả ba frame đều có **checksum đúng**).
- `a=127.0.0.1 ttl=0 an=1` — bản ghi `A` **thật** qua `10.0.2.3:53`; `ttl=0` là giá trị của
  resolver cho `localhost` nên TTL **không** là điều kiện.
- `attempts=1` (cả hai) — lần đầu được, không retransmit; trần vẫn là 3 (ICMP) / 2 (DNS).
- `reply_len=43` — khớp độ dài reply đo được **trên host** cho cùng truy vấn (`len=43`), tức hai
  đường đo độc lập ra cùng một message.

## 5. Đột biến (đo trên cây **cuối**, mỗi lần build lại + boot lại)

### (i) ICMP: bỏ kiểm `id` ⇒ nhận reply của người khác (ĐỎ)

Sửa `net_client.rs::icmp_reply_accept`: `echo.id == id && echo.seq == seq && …` →
`echo.seq == seq && …`. Build + boot:

```text
# net: icmp target=10.0.2.2 reply=1 id=0x4b54 seq=1 payload=32 rtt_ms=4 attempts=1 elapsed_ms=5 filter=id:0 seq:1 payload:1
not ok 32 - net: icmp echo reply target=10.0.2.2 id=0x4b54 seq=1
# TAP 32 FAIL — dừng (không in marker)
```

⇒ `filter=id:0` ⇒ TAP 32 **ĐỎ đúng lý do**, không marker. Host test
`icmp_reply_accept_matches_id_seq_and_payload` cũng đỏ với đột biến này.

### (ii) ICMP: bỏ kiểm `seq` (ĐỎ)

```text
# net: icmp target=10.0.2.2 reply=1 id=0x4b54 seq=1 payload=32 rtt_ms=0 attempts=1 elapsed_ms=2 filter=id:1 seq:0 payload:1
not ok 32 - net: icmp echo reply target=10.0.2.2 id=0x4b54 seq=1
# TAP 32 FAIL — dừng (không in marker)
```

### (iii) DNS: bỏ so `id` (ĐỎ)

Sửa **codec** `v2/lib/vifast-net/src/dns.rs::parse_response`: bỏ khối
`if message.header.id != id { return Err(DnsError::BadId) }`:

```text
# net: dns name=localhost a=127.0.0.1 ttl=0 id=0x4b55 rcode=0 an=1 reply_len=43 rtt_ms=27 elapsed_ms=31 attempts=1 const:timeout_ms=3000 filter=id:0 qr:1 rcode:1
not ok 33 - net: dns a=127.0.0.1 rcode=0
# TAP 33 FAIL — dừng (không in marker)
```

Host test với cùng đột biến: `test result: FAILED. 9 passed; 4 failed` —
`dns_reply_accept_rejects_wrong_id`, `dns_filter_selfcheck_detects_id_qr_rcode`,
`dns_query_wrong_id_reply_is_a_protocol_error_not_a_retry`, `dns_query_accepts_reply`.

### (iv) DNS: bỏ kiểm `RCODE` (ĐỎ)

Sửa `net_client.rs::dns_reply_accept`: bỏ `if header.rcode() != RCODE_NOERROR { … }`:

```text
# net: dns name=localhost a=127.0.0.1 ttl=0 id=0x4b55 rcode=0 an=1 reply_len=43 rtt_ms=19 elapsed_ms=21 attempts=1 const:timeout_ms=3000 filter=id:1 qr:1 rcode:0
not ok 33 - net: dns a=127.0.0.1 rcode=0
# TAP 33 FAIL — dừng (không in marker)
```

**Khôi phục**: `cp` từ bản sao đã lưu; xác nhận `git status` **không** còn `v2/lib/vifast-net/src/dns.rs`
(không có diff) và build lại ảnh sạch trước khi chạy cổng.

## 6. Cổng

| Cổng | Kết quả | Ghi chú |
|---|---|---|
| `cargo test -p vifast-plat-x86_64 --test icmp` | **9/9 PASS** | host test |
| `cargo test -p vifast-plat-x86_64 --test dns` | **13/13 PASS** | host test |
| `cargo test -p vifast-plat-x86_64` (toàn bộ) | **PASS** | gồm `dhcp.rs` 13/13 qua bí danh `DhcpIo` |
| `cargo build -p vifast-kbin-x86_64 --target x86_64-unknown-none --release` | **OK** | ảnh có TAP 32/33 |
| `./v2/bin/v2 check` | **PASS (rc 0)** | fmt + clippy + 5 luật; log `/tmp/kt328-logs/v2-check2.log` |
| `bash v2/test/boot/run-x86_64.sh` | **32/32 kiểm sạch**; TAP **33/33**; **29 ca âm + 1 ca dương** | log `/tmp/kt328-logs/run2.log`; boot thật + 3 boot ca âm thật |
| `bash v2/test/boot/soak-x86_64.sh --smoke 60 3` | **60/60, lỗi=0** (xem §6.1) | log `/tmp/kt328-logs/soak.log` |
| `bash v2/test/boot/soak-x86_64.sh --selftest` | **47/47 ca** (2 dương + 45 âm) | không mở QEMU |

```text
ok 2 - boot 1: TAP 33/33 + 5 dòng '# exc:' + marker 1 lần + halt sạch
ok 30 - ca âm (bb): mất dòng '# net: icmp' ⇒ ĐỎ (# thiếu '# net: icmp target=10.0.2.2 reply=1 id=0x4b54 seq=1 … filter=id:1 seq:1 payload:1' (ICMP echo thật))
ok 31 - ca âm (cc): mất dòng '# net: dns' ⇒ ĐỎ (# thiếu '# net: dns name=localhost a=127.0.0.1 … rcode=0 … filter=id:1 qr:1 rcode:1' (DNS thật))
ok 32 - ca dương (KT-313): picks=[2, 4, 1, 1] first=[a, a, u, c] diff=1 ⇒ XANH
# ca âm: 29/29 ĐỎ đúng như phải (+ 1 ca dương KT-313)
# 32/32 kiểm sạch (boot=1, tests=33, exc=5)
# infra: cpu=qemu64 guard_log=/tmp/kt328-logs/run2/qemu-guard.log qemu_own=0
```

### 6.1 Soak (số đo cuối)

```text
# infra after: qemu_total=0 qemu_ppid1=0 qemu_own=0 qemu_foreign=0 loadavg_1m=8.25 … ncpu=12 boot_done=1 verdict=OK
# infra: cpu=qemu64 invalid=0 isolation_bad=0 guard_log=/tmp/kt328-logs/soak/qemu-guard.log
# guard: boot_end=61 rc_124=0 qemu_own=0
# soak: xong wall=118s (0 boot/s); ok=60 fail=0 tổng=60/60
# soak: BUILD_ID=9ef82f9cbce97fdf sha256=8e8f03b106668ebeb8591521a4d91779c07297949e740d30901c9855ce3470f5 git=60a9e594968d372d174815deca7d66d3e7a481c9
# soak: 60/60 lỗi=0 — 0/N chỉ là trần (QD §5.3); không retry (D2-07)
# soak: SMOKE N=60 < 600 — KHÔNG phải bằng chứng cổng K5'
```

- `rc_124=0`, `qemu_foreign=0`, `qemu_ppid1=0`, `invalid=0`, `isolation_bad=0`, `qemu_own=0`.
- **Mã thoát của `--smoke` là 1 theo thiết kế** (dòng `SMOKE N=60 < 600 — KHÔNG phải bằng chứng
  cổng K5'` rồi `exit 1`); **không** phải soak đỏ: `fail=0`, `60/60`.
- Ảnh **không** đổi giữa soak (`sha256` đầu = cuối, cùng `BUILD_ID=9ef82f9cbce97fdf`).

Thay đổi runner/soak (**không** phải nới ngưỡng):

- `run-x86_64.sh`: `EXPECT_TESTS` 31 → **33**; thêm 4 mẫu dòng (`# net: icmp …`, `ok 32 …`,
  `# net: dns …`, `ok 33 …`); thêm ca âm **(bb)** "mất dòng `# net: icmp`" + **(cc)** "mất dòng
  `# net: dns`" — **cả hai tự khẳng định đã phá** bằng `grep -c` trước/sau (luật 2); số hiển thị
  của dòng tổng kết `total + 28` → `total + 30`.
- `soak-x86_64.sh`: `TAP_TESTS` 31 → **33**; 4 mẫu dòng; 2 ca âm mới (43 → **45** ca âm); dòng
  `1..31` trong log tổng hợp của `--selftest` → `1..33`.
- Không đổi timeout, không đổi seed, không `#[ignore]`, không retry.

## 7. Rủi ro / nợ / cổng chưa chạy

1. **Chưa chạy T2 `gate --fast`**: thay đổi chỉ ở ảnh x86_64 + runner/soak + tài liệu (không chạm
   `spec/`, không chạm ABI, không chạm scheduler/memory ordering). `v2 check` xanh, runner + soak
   x86_64 xanh; lead quyết định T2 trên ứng viên tích hợp sau merge.
2. **Hằng `127.0.0.1` phụ thuộc resolver của host cho tên `localhost`** (đo hôm nay:
   `an=1`, `A 127.0.0.1`, TTL 0). Nếu một resolver trả NXDOMAIN/địa chỉ khác thì TAP 33 **ĐỎ** —
   đúng fail-closed, và dòng `# net: dns …` cho biết ngay `rcode`/`a`. **Không** dùng `example.com`
   vì địa chỉ đổi theo thời gian (đo: hai bản ghi, TTL khác nhau) ⇒ sẽ là đỏ-oan.
   Phương án "DNS cục bộ" của KT-300 (dylib `DYLD_INTERPOSE` + server python bind `0.0.0.0:53`)
   **không** dùng ở đây: nó chiếm **cổng 53 toàn máy** (đụng soak-aarch64 đang chạy song song trên
   cùng host) và cần `clang` — nặng hơn lợi ích, trong khi `localhost` đã là hằng **cục bộ**.
3. **DNS có retransmit** (2 lần × 3 s, cùng `id`) nên `attempts=2` là hợp lệ; TAP 33 chỉ đòi
   `1..=2`. `elapsed_ms` tối đa ~6 s < ngân sách boot 20 s của runner.
4. **ICMP kiểm payload** (chặt hơn khuôn aarch64: chỉ kiểm `id`/`seq`) — đúng yêu cầu nhiệm vụ
   ("kiểm id/seq/checksum/payload"), và nhờ vậy bộ lọc có ba cờ thay vì hai.
5. **TAP 32/33 chạy cuối**: nếu một pha trước đỏ thì chúng không chạy — đúng fail-fast hiện có.
   Thứ tự TAP 1..31 **không đổi**.
6. **TAP 32/33 ghim giá trị slirp** (`10.0.2.2` trả echo; `10.0.2.3` trả `A` cho `localhost`) — cùng
   hợp đồng `-netdev user` mà ARP/DHCP đang dùng; không phải hằng của kernel.
7. **Khi TAP 32/33 đỏ**, ảnh `halt` không ACPI-shutdown ⇒ `qemu_guard` giữ QEMU tới trần rồi giết
   (`rc_124`); runner/soak đã coi là boot đỏ (không phải treo sản phẩm). Boot xanh vẫn thoát sạch.
8. **Smoke 60 KHÔNG phải bằng chứng soak 600** của K5' (QD §5.3); soak 600 phải chạy lại trên ứng
   viên tích hợp sau merge.
9. **`-cpu max`** (KT-323) chưa đo cho TAP 32/33; mặc định vẫn `qemu64`.
10. **Nợ còn lại của K5'**: `syscall`/`sysret`, virtio-gpu x86_64, MSI-X, hotplug — **không** tuyên
    bố K5' đạt.

## 8. Tệp đã sửa

| Tệp | Thay đổi |
|---|---|
| `v2/kernel/vifast-plat-x86_64/src/net_client.rs` | **mới (763 dòng)**: `NetPeer`, `RetryBudget`, `PingStats`, `DnsReply`, `DnsAnswer`, `icmp_*frame`, `icmp_reply_accept`, `icmp_filter_selfcheck`, `ping{,_with}`, `udp_recv_until`, `dns_reply_accept`, `dns_filter_selfcheck`, `dns_query{,_with}`, sáu trần |
| `v2/kernel/vifast-plat-x86_64/src/virtio_net.rs` | trait `DhcpIo` → `NetIo` (+ bí danh `DhcpIo`), `VirtioNetIo::new`, `attempt_budget` `pub(crate)`, **re-export** `net_client` (R1: 1.965 → 1.270 dòng) |
| `v2/kernel/vifast-plat-x86_64/src/lib.rs` | `pub mod net_client;` |
| `v2/kernel/vifast-plat-x86_64/tests/icmp.rs` | **mới**: 9 test host |
| `v2/kernel/vifast-plat-x86_64/tests/dns.rs` | **mới**: 13 test host |
| `v2/kernel/vifast-kbin-x86_64/src/net.rs` | giữ MAC gateway; TAP 32 (ICMP) + TAP 33 (DNS); `DNS_NAME_A`/`DNS_EXPECT_A`/`DNS_ID_A` |
| `v2/kernel/vifast-kbin-x86_64/src/main.rs` | `TAP_TESTS` 31 → **33** |
| `v2/test/boot/run-x86_64.sh` | `EXPECT_TESTS` 33, 4 mẫu dòng, ca âm (bb)/(cc), số hiển thị selftest |
| `v2/test/boot/soak-x86_64.sh` | `TAP_TESTS` 33, 4 mẫu dòng, 2 ca âm, `1..33` trong log selftest |
| `v2/docs/k5-x86_64-boot.md` | ICMP/DNS DONE (§11.8), bỏ khỏi nợ, cập nhật runner/ca âm |
