# KT-246b — de-dup codec: `virtio_net` dùng `vifast-net` (KT-244), xoá bản sao trong kernel

Ngày: 2026-11-21. Nhánh: `task/kt243-netdrv`, base `main` @ `d0ef2d35` (đã có `25320063` = KT-244
`vifast-net` và `a4541103` = KT-246 của chính task này). Quyết định: **D2-196**; chỉ đạo hai bước
của lead (tin nhắn 2026-11-21 01:1x + 01:2x).

## 1. Vì sao có commit này

`[KT-246]` (commit `a4541103` trên `main`) đã dựng **codec tạm** Ethernet/IPv4/ICMP/UDP/DHCP/ARP
**trong** `v2/kernel/vifast-plat-riscv64/src/virtio_net.rs` vì crate `vifast-net` (KT-244) chưa có
lúc đó. Crate ấy nay đã xong (`25320063`, 58 test host XANH) ⇒ xoá bản sao, dùng crate.

## 2. Đã làm

| Tệp | Thay đổi |
|---|---|
| `v2/kernel/vifast-plat-riscv64/Cargo.toml` | `+ vifast-net = { path = "../../lib/vifast-net" }` |
| `v2/kernel/vifast-kbin-riscv64/Cargo.toml` | `+ vifast-net = { path = "../../lib/vifast-net" }` (ảnh `ktest-net` dùng `Ipv4Addr`/`MacAddr` — **cùng** định nghĩa với driver) |
| `v2/kernel/vifast-plat-riscv64/src/virtio_net.rs` | **xoá** toàn bộ codec tạm; dùng `vifast_net::{addr, arp, dhcp, eth, frame, icmp, ipv4, udp}` |
| `v2/kernel/vifast-kbin-riscv64/src/ktest_net.rs` | `NetConfig4`/`PingStats` nay dùng `Ipv4Addr`/`MacAddr` (in bằng `Display`, bỏ hàm `mac_str` + in từng octet) |

### Đã xoá khỏi `virtio_net.rs` (bản sao, không còn dòng nào)

`ones_complement_sum`, `checksum`, `EthFrame`/`eth_build`/`eth_parse`, `Ipv4Packet`/`ipv4_build`/
`ipv4_parse`, `icmp_build_echo`/`icmp_parse_echo_reply`, `UdpDatagram`/`udp_build`/`udp_parse`,
`DhcpMessage`/`dhcp_build`/`dhcp_parse`, `arp_parse_reply` (bản tự viết), và **toàn bộ hằng trùng**:
`ETH_HDR_BYTES`, `ETHERTYPE_IPV4`, `ETHERTYPE_ARP`, `IPV4_HDR_BYTES`, `IP_PROTO_ICMP`,
`IP_PROTO_UDP`, `UDP_HDR_BYTES`, `DHCP_CLIENT_PORT`, `DHCP_SERVER_PORT`, `DHCP_FIXED_BYTES`,
`DHCP_MAGIC`, `DHCP_OPT_*` (9 hằng), `DHCP_DISCOVER/OFFER/REQUEST/ACK`, `BROADCAST_MAC`,
`BROADCAST_IP`, `ZERO_IP`.

### Giữ lại (phần **driver** + **chính sách**, không thuộc crate thuần)

`init`/`send_frame`/`recv_frame`/`recv_frame_wait`/`stats`/`mac`/`device_status`/`arm_rx`/
`queue_with` (MMIO, queue, DMA, MAC, bộ đếm) + `dhcp_acquire`/`arp_resolve`/`ping`/
`arp_build_request`/`dhcp_frame`/`dhcp_frame_parse`/`dhcp_wait`/`arp_parse_reply` (bản dùng
`ArpPacket`) + `NetConfig4`/`PingStats`/`NET_STEP_TIMEOUT_TICKS`/`ICMP_PAYLOAD_BYTES`.
Lý do: chúng gọi `send_frame`/`recv_frame_wait` (MMIO) nên **không** chuyển sang crate `no_std`
thuần được; `arp_build_request`/`dhcp_frame` chỉ là lớp **ghép khung** (codec nằm ở crate).

### Số dòng

| Tệp | Trước | Sau | Δ |
|---|---:|---:|---:|
| `virtio_net.rs` | 1.494 | **1.074** | **−420** |
| `ktest_net.rs` | 618 | 604 | −14 |

Trần R1 = 1.500 ⇒ `virtio_net.rs` từ **99,6 %** xuống **71,6 %** trần; không cần tách
`net_stack.rs`.

## 3. Bằng chứng **không đổi hành vi** (cùng runner, cùng log kỳ vọng)

`bash v2/test/boot/net.sh` sau de-dup: **3/3 ca đạt, 0 lỗi**; `--red-selftest` **12 kiểm, 0 lỗi**.

```
ok 1 - ca dương: 7/7 TAP + MAC thật + DHCP + ARP + ping + ARP reply + bộ đếm sạch
ok 2 - ca âm: ảnh in 'net: no device' + TAP 1 đỏ, không PASS giả
ok 3 - ca không mạng: ảnh in 'net: no link' + TAP 3 đỏ, không treo, không PASS giả
```

Log dương (nguyên văn, `kt246b-boot-positive.log`):

```
TAP version 13
1..7
# net: dhcp ip=10.0.2.15 gw=10.0.2.2 mask=255.255.255.0 server=10.0.2.2
ok 3 - net dhcp
# net: arp gw=52:55:0a:00:02:02
ok 4 - net arp
# net: ping 10.0.2.2 ok n=3 rtt=0
ok 5 - net ping
ok 6 - net tx
ok 7 - net rx
# net stats: tx_frames=7 tx_bytes=902 rx_frames=7 rx_bytes=1530 notifies=15 errors=0 timeouts=0 id_mismatch=0 device_status=0xf
PANIC: KT-246 net xong (7 TAP, 0 fail)
```

### Diff **trước / sau** (`kt246b-diff-before-after.log`) — chỉ 2 dòng khác

```
8c8
< # net dma: desc_rx=0x8020b000 … rx_buf=0x8020b400 tx_buf=0x8020ba00 …
---
> # net dma: desc_rx=0x8020c000 … rx_buf=0x8020c400 tx_buf=0x8020ca00 …
11c11
< # net dhcp: elapsed_ms=0 xid=0x4b543234 const:timeout_ms=3000
---
> # net dhcp: elapsed_ms=2 xid=0x4b543234 const:timeout_ms=3000
```

- Dòng 8: địa chỉ DMA **dịch 0x1000** vì `.text` của ảnh đổi (codec chuyển từ kernel sang crate
  được link vào) ⇒ vùng `.bss` của driver nằm sau `.text` dài hơn. **Không** phải đổi hành vi;
  `identity=1` vẫn đúng.
- Dòng 11: `elapsed_ms` 0 → 2 là **nhiễu thời gian** dưới `-icount`, không phải hằng.

**Mọi** dòng `ok`, `not ok`, `PANIC`, `# net: dhcp/arp/ping`, `# net tx/rx`, `# net stats` **giống
từng byte**. Hai ca âm cũng giống (`kt246b-boot-negative.log`, `kt246b-boot-nolink.log`).

## 4. Cổng

```
$ ./v2/bin/v2 check
v2 check: PASS (L1 chạy trong gate --fast)
```

Log: `kt246b-check.log`. `cargo fmt` đã chạy cho **cả hai** workspace.

## 5. Rủi ro / ghi chú

1. **`.text` ảnh tăng** 99.152 → 114.368 B (crate `vifast-net` link vào, codec không còn inline
   trong kernel). `.data+.bss` **không đổi** (10.976 B / 262.144 B) ⇒ ngân sách D2-72 Q5 vẫn dư.
2. **`vifast-net` phải merge trước** nhánh này (đã có trên `main` tại `25320063`) — nếu không,
   `v2/kernel` build sẽ hỏng vì `path` dependency. Đây là **phụ thuộc một chiều** đã thoả.
3. `v2/kernel` là workspace **riêng**; `path` từ `v2/kernel/vifast-plat-riscv64` tới
   `v2/lib/vifast-net` hợp lệ (đã kiểm bằng `cargo build`), và `vifast-net` là `no_std` +
   `deny(unsafe_code)` + **không** dependency ngoài ⇒ không kéo `std` vào kernel.
4. **Không** còn bản sao codec nào trong kernel: `grep -rn "fn ipv4_build\|fn dhcp_build\|
   fn icmp_build_echo\|fn eth_build" v2/kernel/` ⇒ rỗng (đã kiểm).
