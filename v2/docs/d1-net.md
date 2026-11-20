# D1 (phần đầu) — ViFastOs **có mạng**: virtio-net + DHCP + ARP + ICMP echo

Trạng thái: **DONE** — Agent 1 (toàn quyền A0), 2026-11-21. Quyết định: **D2-196**.
Nguồn sự thật: code + log trong `v2/agent/JOURNAL/kt244-*`, `kt245-*`, `kt246-*`, `kt246b-*`.

## 1. Phạm vi

| Task | Nội dung | Trạng thái |
|---|---|---|
| KT-244 | crate `vifast-net` — codec Ethernet/ARP/IPv4/ICMP/UDP/DHCP (thuần, host-testable) | **DONE** |
| KT-245 | driver virtio-net (phần thuần + MMIO hai queue + MAC + quét PCI) | **DONE** |
| KT-246 | DHCP client + ARP + ICMP echo trên QEMU `-netdev user` | **DONE** |
| KT-246b | de-dup: `virtio_net` dùng `vifast-net`, xoá codec trùng | **DONE** |
| KT-260 | UDP echo + DNS query (RFC 1035) — N1 phần đầu | **DONE** |

**Không** làm: TCP, socket API, net server U-mode (N1), virtio-gpu/input (input = KT-255),
`spec/` change, ảnh sản phẩm (driver chỉ trong ảnh test `ktest-net`).

## 2. Kiến trúc

```text
v2/lib/vifast-net/            ← codec THUẦN: no_std, deny(unsafe_code), không dependency
  addr.rs checksum.rs eth.rs arp.rs ipv4.rs icmp.rs udp.rs dhcp.rs frame.rs
        ↑ dùng bởi
v2/kernel/vifast-plat-riscv64/src/virtio_net.rs   ← driver MMIO (unsafe chỉ ở crate arch)
  init / send_frame / recv_frame_wait / mac / stats / dhcp_acquire / arp_resolve / ping
        ↑
v2/kernel/vifast-kbin-riscv64/src/ktest_net.rs    ← ảnh test `ktest-net` (7 TAP)
v2/test/boot/net.sh                                ← runner: 3 ca + --red-selftest
```

**Vì sao tách crate thuần**: `vifast-plat-riscv64` có `unsafe` (MMIO) nên **không** chạy host test.
Phần *quyết định* của mạng — địa chỉ, checksum, encode/decode header, kiểm độ dài — là logic thuần
trên byte ⇒ tách ra để test host (D2-05: đỏ trước rồi mới xanh) và để driver chỉ còn phần chạm
thanh ghi.

## 3. Codec (`vifast-net`)

- `#![no_std]`, `#![deny(unsafe_code)]`, **không** dependency ngoài (D2-21).
- Mọi codec làm việc trên **lát byte có sẵn**: không cấp phát, không biết PA, không tự gửi.
  Ngoại lệ duy nhất: `DhcpMessage` **sở hữu** khung 576 B (RFC 951) vì thông điệp được dựng rồi
  mới gửi.
- **Từ chối phân mảnh**: `ipv4::Ipv4Header::parse` trả `Unsupported` khi `MF` bật hoặc
  `frag_offset ≠ 0` — D1 chưa có tầng ghép mảnh, im lặng nhận mảnh là sai.
- Checksum Internet (RFC 1071) cộng dồn **từng mảnh** ⇒ pseudo-header UDP không phải sao chép
  payload vào bộ đệm tạm.
- **58 test host**: vector chuẩn RFC 1071 (`0x220d`), header IPv4 (`0xb861`), frame ARP request
  byte-exact, DHCP DISCOVER byte-exact 250 B; ca âm từng codec (checksum sai, cắt cụt,
  version/IHL/total-length sai, ARP hlen/plen/htype/ptype/oper sai, DHCP option vượt khung/thiếu
  END/cookie sai, UDP length sai, ICMP type lạ).

## 4. Driver (`virtio_net`)

- `pci::VirtioKind::Net` — device id **`0x1000`** (transitional) và **`0x1041`** (modern-only);
  BAR leaf riêng `0x4040_0000` (mỗi loại thiết bị một trang 2 MiB trong cửa sổ PCIe).
- Feature đàm phán: **`VIRTIO_F_VERSION_1`** + **`VIRTIO_NET_F_MAC`** nếu thiết bị có.
  **Không** đàm phán `MRG_RXBUF` (đổi ngữ nghĩa `virtio_net_hdr`; driver dùng **một** descriptor
  device-writable cho mỗi frame) và **không** đàm phán `STATUS` (phát hiện "không có mạng" bằng
  **timeout DHCP/ARP**, fail-closed).
- Hai queue: **0 = RX**, **1 = TX**, cỡ 16; DMA một trang 4 KiB; `send_frame`/`recv_frame_wait`.
- **Fail-closed**: thiếu thiết bị ⇒ `Err`, không PANIC, không treo.

## 5. Đo được (QEMU `virt`, `-netdev user,id=n0 -device virtio-net-pci,netdev=n0`)

```text
# net pci: functions=2 bus=0 dev=1 func=0 vendor=0x1af4 device=0x1000 bar=4 kind=mem64 size=16384 assigned=0x40400000
# net dev: mac=52:54:00:12:34:56 … device_features=0x10130bf8024 driver_features=0x100000020
# net: dhcp ip=10.0.2.15 gw=10.0.2.2 mask=255.255.255.0 server=10.0.2.2
# net: arp gw=52:55:0a:00:02:02
# net: ping 10.0.2.2 ok n=3 rtt=0
ok 1..7 - net device/mac/dhcp/arp/ping/tx/rx
# net: udp echo ok len=15 rtt=6
ok 8 - net udp echo
# net: dns example.com a=172.66.147.243 ttl=205
ok 9 - net dns a
# net stats: tx_frames=12 tx_bytes=1218 rx_frames=10 rx_bytes=1772 notifies=23 errors=0 timeouts=0 id_mismatch=0
PANIC: KT-260 net xong (9 TAP, 0 fail)
```

- MAC `52:54:00:12:34:56` **đọc từ `virtio_net_config`** (không hardcode).
- RX là **ARP reply thật của slirp** (`oper=2`, `sender_ip=10.0.2.2`), không phải loopback phần mềm.
- `rtt=0` là số đo thật dưới `-icount shift=6` (đồng hồ ảo).

## 6. Ca âm (fail-closed)

| Ca | Cách dựng | Kỳ vọng |
|---|---|---|
| **không thiết bị** | cùng ảnh, **không** `-netdev`/`-device` | `net: no device` + `not ok 1 - net device`; runner coi là **đạt** khi ảnh đỏ đúng cách |
| **không mạng** | `-netdev socket,id=n0,listen=127.0.0.1:0` (không peer) | `net dhcp: elapsed_ms=3000 err=Timeout` + `net: no link` + `not ok 3`; QEMU **tự thoát**, không treo |

Runner `v2/test/boot/net.sh`: 3/3 ca + `--red-selftest` **12 kiểm** (7 đột biến log + 2 ca "PASS
giả" đều bị từ chối). Nối vào cổng: bước **55** `selftest validator net` (T1+T2) và bước **57**
`QEMU virtio-net` (chỉ T2, ~40 s).

## 7. Lỗi thật tự bắt được

`ar_tip` của ARP reply là IP **người hỏi**, không phải target. Bản đầu đặt sai ⇒ ARP timeout 3 s dù
thiết bị đã trả lời (`JOURNAL/kt246-red-arp-bug.log`). Đây là lý do ca âm "không mạng" phải có
**timeout thật** chứ không chỉ kiểm chuỗi.

## 7b. UDP + DNS (KT-260)

- **`vifast-net::dns`** — codec RFC 1035: header 12 B, question, answer, **giải nén tên** (con trỏ
  `0xC0`) có phát hiện **vòng lặp**; 21 test host gồm ca âm (cắt cụt, `qdcount=0`, con trỏ vòng
  lặp, con trỏ ra ngoài message, `RDLENGTH` sai, `ancount` lớn hơn số record, label > 63, tên > 255,
  buffer hẹp, `id` sai, không phải response).
- **UDP echo**: `10.0.2.2:7` **không** có service trong libslirp (`udp_input` chỉ xử lý BOOTP/TFTP)
  ⇒ bằng chứng UDP hai chiều lấy từ **DNS question echo**: QNAME `vifast-udp-echo`, server chép
  nguyên question section (RFC 1035 §4.1.2) ⇒ `echoed=1`, `reply_len=33`. `udp_echo` tới
  `10.0.2.2:7` vẫn hiện thực đầy đủ và **timeout 3 s** đúng (chẩn đoán trong log).
- **DNS**: `example.com` → `A 172.66.147.243`, `rcode=0`, `an=1`, `rtt_ms=6`.
- **Ca âm**: DNS tới `192.0.2.1:53` (TEST-NET-1) ⇒ timeout 3 s, không treo.
- Runner: **9 TAP** + 2 ca âm + `--red-selftest` **21 kiểm**.

## 8. Còn thiếu

1. **TCP** và **socket API** (N1) — chưa có (UDP + DNS đã có ở KT-260).
2. **Net server U-mode** — chưa có; hiện toàn bộ stack chạy trong kernel ảnh test.
3. **virtio-gpu / virtio-input** — input đang mở ở KT-255.
4. **`ping`/`ifconfig` trong shell sản phẩm** — chưa có (driver chỉ trong ảnh test).
5. **IPv6**, **phân mảnh**, **ICMP lỗi** (destination unreachable) — chưa có.
