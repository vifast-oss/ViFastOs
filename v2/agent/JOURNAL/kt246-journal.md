# KT-246 — D1: DHCP client + ARP + ICMP echo trên virtio-net thật (QEMU slirp)

Ngày: 2026-11-21. Nhánh: `task/kt243-netdrv` (worktree `/tmp/kt243-s4`), base `main` @ `43a054e3`.
Quyết định mở: **D2-196** mục (1) bước 3 + mục (3) (đo được: `net: dhcp ip=…` + `net: ping … ok` +
ca âm "không có `-netdev` ⇒ fail-closed"). Uỷ quyền commit: **D2-198**.

Tiền đề: KT-245 (commit `c639e820`) — driver virtio-net 2 queue, MAC thật, tx/rx frame thô.

## 0. NỢ KT-246 — codec **tạm** trong `virtio_net.rs` (lead đã chốt cách xử lý)

Crate `vifast-net` (KT-244, sub-agent `netlib`, worktree `/tmp/kt243-s2`) **chưa có** trong worktree
này lúc làm KT-246 (đã kiểm `ls /tmp/kt243-s2/v2/lib/vifast-net` ⇒ không tồn tại). Theo chỉ đạo của
lead (tin nhắn 2026-11-21 01:1x), làm **hai bước**:

1. **Bước 1 — commit này (`[KT-246]`)**: codec Ethernet/IPv4/ICMP/UDP/DHCP nằm **tạm** trong
   `v2/kernel/vifast-plat-riscv64/src/virtio_net.rs`, đánh dấu bằng khối
   `// KT-246 — DHCP client + ARP + ICMP echo` + ghi chú **NỢ KT-246** ngay trên đầu khối. Mục đích:
   chứng minh **E2E** (DHCP + ARP + ping) trên thiết bị thật **trước**, không chặn tiến độ.
2. **Bước 2 — commit riêng `[KT-246b]`** (ngay khi `vifast-net` sẵn sàng): thêm
   `vifast-net = { path = "../../lib/vifast-net" }` vào `v2/kernel/vifast-plat-riscv64/Cargo.toml`,
   **xoá** codec trùng, dùng `vifast_net::{eth,arp,ipv4,icmp,udp,dhcp}`, rồi **chạy lại đúng runner
   E2E này** (cùng log kỳ vọng) để chứng minh **không đổi hành vi**. Nếu `vifast-net` thiếu hàm cần
   ⇒ **báo lead**, **không** vá codec tạm.

Mọi hàm codec ở đây là **thuần** (không `unsafe`, không cấp phát, không đọc trạng thái driver) nên
việc chuyển sang `vifast-net` là máy móc. Phần **không** chuyển được (gọi `send_frame`/
`recv_frame_wait`): `dhcp_acquire`, `arp_resolve`, `ping`.

## 1. Đỏ trước (D2-05)

### 1.1 Runner mới từ chối log của ảnh KT-245 (4 TAP)

Viết validator 7 TAP + ba dòng `net:` **trước** khi viết code, rồi cho nó kiểm log thật của KT-245:

```
$ bash v2/test/boot/net.sh --check-log v2/agent/JOURNAL/kt245-boot-positive.log positive
not ok - ca dương: dòng kế hoạch '1..7' xuất hiện 0 lần (đòi 1)
not ok - dòng 'ok 3 - net dhcp' xuất hiện 0 lần (đòi 1)
not ok - dòng 'ok 4 - net arp' xuất hiện 0 lần (đòi 1)
not ok - dòng 'ok 5 - net ping' xuất hiện 0 lần (đòi 1)
not ok - không thấy dòng 'net: dhcp ip=10.0.2.15 gw=10.0.2.2 mask=255.255.255.0 server=10.0.2.2'
…
rc=10
```

Log: `kt246-red-runner.log`. Chạy đủ ba ca trên ảnh cũ: **12 lỗi** (chỉ ca "không thiết bị" đạt).

### 1.2 Lỗi **thật** tự bắt bằng boot: `ar_tip` của ARP reply

Bản đầu của `arp_parse_reply` kiểm `ar_tip == target_ip`. **Sai**: trong ARP **reply**, `ar_tip` là
địa chỉ **người hỏi** (`our_ip`), `ar_spa` mới là `target_ip`. Hậu quả đo được: DHCP **xanh**,
ARP **timeout 3 s** dù thiết bị đã trả lời (TAP `net rx` thô vẫn nhận đúng frame):

```
# net: dhcp ip=10.0.2.15 gw=10.0.2.2 mask=255.255.255.0 server=10.0.2.2
ok 3 - net dhcp
# net arp: elapsed_ms=3000 err=Timeout
# net: no link
not ok 4 - net arp: không nhận được ARP reply của gateway
```

Log: `kt246-red-arp-bug.log`. Đã sửa (`arp_parse_reply(frame, our_ip, target_ip, our_mac)`) và ghi
lại bẫy này trong doc-comment của hàm để không tái diễn.

## 2. Phần làm

| Tệp | Nội dung |
|---|---|
| `v2/kernel/vifast-plat-riscv64/src/virtio_net.rs` | khối **KT-246**: `ones_complement_sum`/`checksum`, `eth_build`/`eth_parse`, `ipv4_build`/`ipv4_parse` (kiểm version/IHL/`total_length`/checksum), `icmp_build_echo`/`icmp_parse_echo_reply` (kiểm type/code/checksum/id/seq), `udp_build`/`udp_parse`, `dhcp_build`/`dhcp_parse` (BOOTP + magic + option 53/1/3/50/54/55), `arp_build_request`/`arp_parse_reply`, `dhcp_acquire` (DISCOVER→OFFER→REQUEST→ACK), `arp_resolve`, `ping` (3 echo), `NetConfig4`, `PingStats`, `NET_STEP_TIMEOUT_TICKS = 3 s` |
| `v2/kernel/vifast-kbin-riscv64/src/ktest_net.rs` | 7 TAP (thứ tự mới: device, mac, **dhcp, arp, ping**, tx, rx) + ba dòng cố định `net: dhcp …` / `net: arp gw=…` / `net: ping … ok n=3 rtt=…` + `net: no link` + `net: no device` |
| `v2/test/boot/net.sh` | validator 7 TAP + ba dòng `net:` + **ca thứ ba** "không mạng" (`-netdev socket,id=n0,listen=127.0.0.1:0`, không peer) + `--red-selftest` **12 kiểm** |

**Thứ tự TAP đổi** (tx/rx từ 3/4 → 6/7) để ca "không mạng" đỏ đúng ở **TAP 3 (dhcp)** thay vì chết
ở TAP rx — nếu giữ thứ tự cũ, ca "không mạng" sẽ đỏ ở `net rx` và **không bao giờ** in
`net: no link`, tức là không chứng minh được DHCP fail-closed.

## 3. Đo được — QEMU `virt` + slirp (log nguyên văn)

Dòng QEMU: `-machine virt,aia=none,aclint=on -cpu rv64 -m 128M -smp 1 -nographic -bios
opensbi-riscv64-generic-fw_dynamic.bin -kernel vifast-kbin-ktest-net -netdev user,id=n0 -device
virtio-net-pci,netdev=n0` (+ `-icount shift=6` trong runner, D2-74).

```
TAP version 13
1..7
# net image: hart=0 stvec=0x802000a0 direct=1 satp=0x80000000000804e8 mode=8 tables=10 used=40960 const:gateway=10.0.2.2 const:ping_count=3
# net pci: functions=2 bus=0 dev=1 func=0 vendor=0x1af4 device=0x1000 bar=4 kind=mem64 size=16384 assigned=0x40400000 readback=0x40400000 command=0x6
# net cap: common=0x0 notify=0x3000 device=0x2000 isr=0x1000 mul=4 cap_bar=4 ecam=0x30008000 ecam_identity=1 bar_identity=1 cap_ranges_ok=1 overlap_checked=1 other_bars=0 reserved_ranges=5
ok 1 - net device
# net dev: mac=52:54:00:12:34:56 status=0x1 mtu_raw=65535 max_virtqueue_pairs=65535 num_queues=3 queue_max_rx=256 queue_max_tx=256 queue_size_rx=16 queue_size_tx=16 device_features=0x10130bf8024 driver_features=0x100000020 mac_feature=1 status_feature=0 mrg_rxbuf_feature=0 device_status=0xf config_generation=0
# net dma: desc_rx=0x8020b000 avail_rx=0x8020b100 used_rx=0x8020b128 desc_tx=0x8020b200 avail_tx=0x8020b300 used_tx=0x8020b328 rx_buf=0x8020b400 tx_buf=0x8020ba00 identity=1 version1=1 notify_off_rx=0 notify_off_tx=1 notify_addr_rx=0x40403000 notify_addr_tx=0x40403004
ok 2 - net mac
# net: dhcp ip=10.0.2.15 gw=10.0.2.2 mask=255.255.255.0 server=10.0.2.2
# net dhcp: elapsed_ms=0 xid=0x4b543234 const:timeout_ms=3000
ok 3 - net dhcp
# net: arp gw=52:55:0a:00:02:02
# net arp: elapsed_ms=0 gw=10.0.2.2 mac_khong_broadcast=1
ok 4 - net arp
# net: ping 10.0.2.2 ok n=3 rtt=0
# net ping: sent=3 received=3 rtt_min_ms=0 rtt_max_ms=0 elapsed_ms=0 const:count=3
ok 5 - net ping
# net tx: frame=42 (=14 ethernet + 28 arp) result=ok tx_frames=7 tx_bytes=902 notifies=14 errors=0 timeouts=0 id_mismatch=0 queue=1
ok 6 - net tx
# net rx: frame=64 oper=2 ethertype=0x806 sender_ip=10.0.2.2 sender_mac=52:55:0a:00:02:02 target_mac_khop=1 rx_frames=7 rx_bytes=1530 errors=0 timeouts=0 id_mismatch=0 queue=0
ok 7 - net rx
# net stats: tx_frames=7 tx_bytes=902 rx_frames=7 rx_bytes=1530 notifies=15 errors=0 timeouts=0 id_mismatch=0 device_status=0xf
PANIC: KT-246 net xong (7 TAP, 0 fail)
```

Log: `kt246-boot-positive.log`.

**`rtt=0` là số đo thật, không phải hằng**: runner chạy `-icount shift=6` (D2-74) nên đồng hồ **ảo**
tiến rất chậm; cả DHCP + ARP + 3 ping xong trong **0 ms ảo**. Giá trị lấy từ `rdtime` của kernel
(`PLATFORM.now_ticks()`), không viết tay. Muốn RTT khác 0 phải bỏ `-icount` — đổi ngưỡng đo, không
làm trong task này.

## 4. Ca âm (fail-closed) — hai ca

### 4.1 Không có thiết bị (`net: no device`)

Cùng ảnh, **không** `-netdev`/`-device`:

```
TAP version 13
1..7
# net pci: err=NoDevice stage=nodevice
# net: no device
not ok 1 - net device: không tìm/khởi tạo được PCI
# ktest halt
```

Log: `kt246-boot-negative.log`.

### 4.2 Có thiết bị, **không có mạng** (`net: no link`)

`-netdev socket,id=n0,listen=127.0.0.1:0` (thiết bị có mặt, **không** peer nào trả lời):

```
TAP version 13
1..7
…
ok 1 - net device
…
ok 2 - net mac
# net dhcp: elapsed_ms=3000 err=Timeout
# net: no link
not ok 3 - net dhcp: không nhận được DHCP OFFER/ACK trong 3 s
# ktest halt
```

Log: `kt246-boot-nolink.log`. `elapsed_ms=3000` chứng minh timeout **thật** 3 s và **không treo**
(QEMU tự thoát, runner không phải kill).

## 5. Runner

```
$ bash v2/test/boot/net.sh
# ca dương: -netdev user,id=n0 -device virtio-net-pci,netdev=n0
ok 1 - ca dương: 7/7 TAP + MAC thật + DHCP + ARP + ping + ARP reply + bộ đếm sạch (…)
# ca âm: cùng ảnh, KHÔNG -netdev/-device
ok 2 - ca âm: ảnh in 'net: no device' + TAP 1 đỏ, không PASS giả (…)
# ca không mạng: -netdev socket,id=n0,listen=127.0.0.1:0 (không peer)
ok 3 - ca không mạng: ảnh in 'net: no link' + TAP 3 đỏ, không treo, không PASS giả (…)
# net.sh: 3 kiểm đạt, 0 lỗi
```

`--red-selftest` (**12 kiểm**): log tốt dựng tay **được** chấp nhận; **7** đột biến (`xoa_ok3`,
`doi_mac`, `doi_oper`, `doi_ip`, `them_not_ok`, `xoa_stats`, `doi_ping_n`) **bị** từ chối; log ca
"không thiết bị" đỏ đúng cách **được** chấp nhận, bản "PASS giả" **bị** từ chối; log ca "không mạng"
đỏ đúng cách **được** chấp nhận, bản "PASS giả" **bị** từ chối. Log: `kt246-runner.log`.

## 6. Ngân sách ảnh (D2-72 Q5)

```
# image=ktest-net file=99152 data+bss=10976 budget=262144 stack=65536 guard=4096 trap_stack=16384 syscall_stack=32768
```

10.976 B / 262.144 B — **không đổi** so với KT-245 (codec là `.text`, không thêm `.bss`).

## 7. Cổng

```
$ ./v2/bin/v2 check
v2 check: PASS (L1 chạy trong gate --fast)
```

Log: `kt246-check.log` (40 bước OK). `cargo fmt` đã chạy cho **cả hai** workspace.

## 8. Nợ / rủi ro

1. **Codec tạm** — xem §0. Commit `[KT-246b]` sẽ thay bằng `vifast-net` và chạy lại **đúng** runner
   này. `virtio_net.rs` hiện **1.494 dòng** (trần R1 = 1.500) ⇒ **không** thêm gì vào tệp này trước
   khi `[KT-246b]` xoá codec (sau đó còn ~900 dòng).
2. **`rtt=0`** dưới `-icount` — số đo thật của đồng hồ ảo, không phải hằng (xem §3).
3. **`xid` DHCP là hằng** (`0x4b543234`) vì K6 chưa có RNG; vẫn phân biệt được reply của ta nhờ
   kiểm thêm `chaddr` = MAC của ta.
4. **Không phân mảnh IP**: `ipv4_build` từ chối gói > 1500 B; DHCP/ARP/ICMP của task này đều nhỏ.
5. **`v2/bin/v2`**: bước **55** (`selftest validator net`) đã thêm ở commit KT-245 vì
   `check-selftest-wiring.py` (KT-134) **đỏ** nếu script có `--red-selftest` mà không cổng nào gọi.
   Lead đã nói giữ việc nối cổng — bước này **đã có**, lead có thể giữ/đổi số khi merge; KT-246
   **không** thêm bước nào.
