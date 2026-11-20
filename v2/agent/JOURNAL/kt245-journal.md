# KT-245 — D1: driver **virtio-net** thật trên QEMU `virt` (chỉ trong ảnh test `ktest-net`)

Ngày: 2026-11-21. Nhánh: `task/kt243-netdrv` (worktree `/tmp/kt243-s4`), base `main` @ `43a054e3`.
Quyết định mở: **D2-196** (D1 phạm vi virtio-net + codec + DHCP + ICMP echo; **không** TCP, **không**
socket API, **không** đổi `spec/`). Uỷ quyền commit: **D2-198**.

## 1. Vào — baseline

```
$ cargo test -p vifast-virtio          # 10 test strict + 17 logic + 6 driver_request: XANH
$ ./v2/bin/v2 check                    # PASS (trước khi sửa)
```

Cây sạch ngoài các tệp của task. Không chạm `v2/agent/{STATE,QUEUE,DECISIONS}.md`.

## 2. Đỏ trước (D2-05)

### 2.1 Host — test mới trên cây **chưa có** module

Viết `v2/lib/vifast-virtio/tests/net.rs` (23 test) **trước**, chạy khi `net.rs` chưa có và
`VirtioKind::Net` chưa có ⇒ **không biên dịch**:

```
error[E0432]: unresolved import `vifast_virtio::net`
error[E0599]: no variant, associated function, or constant named `Net` found for enum `VirtioKind`
error: could not compile `vifast-virtio` (test "net") due to 6 previous errors
```

Log: `kt245-red-host.log`.

### 2.2 Đột biến (chứng minh test **không** tautology)

| Đột biến | Test bắt | Log |
|---|---|---|
| `negotiated_features` đàm phán luôn `MRG_RXBUF` | `feature_khong_bao_gio_dam_phan_mrg_rxbuf` | `kt245-mutation-host.log` |
| hoán vị offset `status` ↔ `max_virtqueue_pairs` trong `decode_config` | `config_doc_dung_offset_spec` (`left: 2 / right: 1`) | `kt245-mutation2-host.log` |

**Đột biến thứ hai ban đầu KHÔNG bị bắt** — fixture để `status = max_virtqueue_pairs = 1` nên hoán
vị hai trường vẫn ra cùng giá trị. Đã sửa fixture (`max_virtqueue_pairs = 2`) rồi chạy lại: **ĐỎ**.
Đây đúng lớp lỗi "test xanh nhưng giả" (KT-163) — ghi lại vì nó **suýt** lọt.

## 3. Phần làm

| Tệp | Nội dung |
|---|---|
| `v2/lib/vifast-virtio/src/net.rs` (mới) | `virtio_net_config` (§5.1.4: mac 0..6, status 6..8, max_virtqueue_pairs 8..10, mtu 10..12), feature `F_MAC=5`/`F_MRG_RXBUF=15`/`F_STATUS=16`/`F_VERSION_1=32`, `negotiated_features` (luôn VERSION_1 + MAC nếu có; **không** MRG_RXBUF/STATUS), `NetHdr` 10/12 byte `encode`/`parse`, `frame_len`, `frame_ok`, `NetError` có tên, `DEVICE_ID_NET=0x1000`, `Q_RX=0`/`Q_TX=1`, `QUEUE_SIZE=16` |
| `v2/lib/vifast-virtio/src/pci.rs` | `VirtioKind::Net` (0x1000/0x1041) + `is_virtio_net`; id lấy từ `net::` (**một** nguồn) |
| `v2/lib/vifast-virtio/src/lib.rs` | `pub mod net;` |
| `v2/lib/vifast-virtio/tests/net.rs` (mới) | 23 test host: config (5), feature (5), header (5), frame/hằng số (8) |
| `v2/kernel/vifast-plat-riscv64/src/virtio_net.rs` (mới) | driver MMIO: `init` (reset→ACK/DRIVER→feature→FEATURES_OK→2 queue→DRIVER_OK→config→arm rx), `send_frame`, `recv_frame`/`recv_frame_wait`, `stats`, `mac`, `device_status` |
| `v2/kernel/vifast-plat-riscv64/src/pci.rs` | `BAR_ASSIGN_BASE_NET = 0x4040_0000` (leaf 3), `PCI_DRIVER_LEAVES` (net ⇒ 3 leaf liền dải), `init_for` nhánh `Net` |
| `v2/kernel/vifast-plat-riscv64/src/mmu.rs` | map leaf 2 + 3 của cửa sổ PCIe khi `feature = "net"` (leaf 1 map luôn cho liền dải) |
| `v2/kernel/vifast-plat-riscv64/src/lib.rs` | `pub mod virtio_net;` sau `feature = "net"`; `pci` mở thêm `feature = "net"` |
| `v2/kernel/vifast-plat-riscv64/Cargo.toml` | `net = ["ktest"]` |
| `v2/kernel/vifast-kbin-riscv64/Cargo.toml` | `net = ["ktest", "vifast-plat-riscv64/net"]` + bin `vifast-kbin-ktest-net` |
| `v2/kernel/vifast-kbin-riscv64/src/ktest_net.rs` (mới) | ảnh TAP 4 ca + dòng `# net: no device` cho ca âm |
| `v2/test/boot/net.sh` (mới) | runner: ca dương (4 TAP + MAC thật + ARP reply + bộ đếm sạch) + ca âm (không `-netdev` ⇒ đỏ) + `--red-selftest` |
| `v2/bin/v2` | bước **55** làn B: `selftest validator net (KT-245)` (KT-134: script tự kiểm phải có cổng gọi) |

## 4. Đo được — QEMU `virt` (log nguyên văn)

Dòng QEMU: `-machine virt,aia=none,aclint=on -cpu rv64 -m 128M -smp 1 -nographic -bios
opensbi-riscv64-generic-fw_dynamic.bin -kernel vifast-kbin-ktest-net -netdev user,id=n0 -device
virtio-net-pci,netdev=n0` (thêm `-icount shift=6` trong runner, D2-74).

```
TAP version 13
1..4
# net image: hart=0 stvec=0x802000a0 direct=1 satp=0x80000000000804e8 mode=8 tables=10 used=40960 const:gateway=10.0.2.2 const:guest=10.0.2.15
# net pci: functions=2 bus=0 dev=1 func=0 vendor=0x1af4 device=0x1000 bar=4 kind=mem64 size=16384 assigned=0x40400000 readback=0x40400000 command=0x6
# net cap: common=0x0 notify=0x3000 device=0x2000 isr=0x1000 mul=4 cap_bar=4 ecam=0x30008000 ecam_identity=1 bar_identity=1 cap_ranges_ok=1 overlap_checked=1 other_bars=0 reserved_ranges=5
ok 1 - net device
# net dev: mac=52:54:00:12:34:56 status=0x1 mtu_raw=65535 max_virtqueue_pairs=65535 num_queues=3 queue_max_rx=256 queue_max_tx=256 queue_size_rx=16 queue_size_tx=16 device_features=0x10130bf8024 driver_features=0x100000020 mac_feature=1 status_feature=0 mrg_rxbuf_feature=0 device_status=0xf config_generation=0
# net dma: desc_rx=0x8020a000 avail_rx=0x8020a100 used_rx=0x8020a128 desc_tx=0x8020a200 avail_tx=0x8020a300 used_tx=0x8020a328 rx_buf=0x8020a400 tx_buf=0x8020aa00 identity=1 version1=1 notify_off_rx=0 notify_off_tx=1 notify_addr_rx=0x40403000 notify_addr_tx=0x40403004
ok 2 - net mac
# net tx: frame=42 (=14 ethernet + 28 arp) result=ok tx_frames=1 tx_bytes=42 notifies=2 errors=0 timeouts=0 id_mismatch=0 queue=1
ok 3 - net tx
# net rx: frame=64 oper=2 ethertype=0x806 sender_ip=10.0.2.2 sender_mac=52:55:0a:00:02:02 target_mac_khop=1 rx_frames=1 rx_bytes=64 errors=0 timeouts=0 id_mismatch=0 queue=0
ok 4 - net rx
# net stats: tx_frames=1 tx_bytes=42 rx_frames=1 rx_bytes=64 notifies=3 errors=0 timeouts=0 id_mismatch=0 device_status=0xf
PANIC: KT-245 net xong (4 TAP, 0 fail)
```

Log: `kt245-boot-positive.log`. **MAC thật** `52:54:00:12:34:56` (OUI QEMU) đọc từ
`virtio_net_config` — không hardcode trong ảnh. **RX là frame thật của slirp**: ARP reply
`sender_ip=10.0.2.2`, `sender_mac=52:55:0a:00:02:02`, `target_mac` = MAC của ta ⇒ một vòng
tx→rx **qua thiết bị**, không phải loopback phần mềm.

## 5. Ca âm (fail-closed)

Cùng ảnh, **không** `-netdev`/`-device`:

```
TAP version 13
1..4
# net pci: err=NoDevice stage=nodevice
# net: no device
not ok 1 - net device: không tìm/khởi tạo được PCI
# ktest halt
```

Log: `kt245-boot-negative.log`. Không PANIC, không treo; runner coi đây là **đạt** (ảnh đỏ đúng
cách) và **đỏ** nếu ảnh PASS giả.

## 6. Runner

```
$ bash v2/test/boot/net.sh
# ca dương: -netdev user,id=n0 -device virtio-net-pci,netdev=n0
ok 1 - ca dương: 4/4 TAP + MAC thật + ARP reply + bộ đếm sạch (…/kt245-positive.log)
# ca âm: cùng ảnh, KHÔNG -netdev/-device
ok 2 - ca âm: ảnh in 'net: no device' + TAP 1 đỏ, không PASS giả (…/kt245-negative.log)
# net.sh: 2 kiểm đạt, 0 lỗi
```

`--red-selftest` (8 kiểm): log tốt dựng tay **được** chấp nhận; 5 đột biến (`xoa_ok3`, `doi_mac`,
`doi_oper`, `them_not_ok`, `xoa_stats`) **bị** từ chối; log ca âm đỏ đúng cách **được** chấp nhận;
ca âm "PASS giả" (có `ok 1`) **bị** từ chối. Log: `kt245-runner.log`.

**Lưu ý hạ tầng**: dòng `# infra kt245-net: … qemu_foreign=1 verdict=KHONG_HOP_LE` là QEMU của
worktree `/tmp/kt243-s3` (sub-agent `infra`) đang chạy — **không** phải QEMU của task này, và
**không** bị kill (AGENTS §9). Phép đo này là kiểm **đúng/sai**, không dùng số thời gian.

## 7. Ngân sách ảnh (D2-72 Q5)

```
# image=ktest-net file=89376 data+bss=10976 budget=262144 stack=65536 guard=4096 trap_stack=16384 syscall_stack=32768
```

10.976 B / 262.144 B — dư nhiều (ảnh `ktest` cũ ở 92,6 % trần, đây là lý do tách ảnh).

## 8. Cổng

```
$ ./v2/bin/v2 check
v2 check: PASS (L1 chạy trong gate --fast)
```

Log: `kt245-check.log`. `cargo fmt` đã chạy cho **cả hai** workspace (`v2/` và `v2/kernel/`).

## 9. Nợ / rủi ro

1. **`mtu_raw=65535`, `max_virtqueue_pairs=65535`** — QEMU trả hai trường này vô nghĩa khi driver
   **không** đàm phán `VIRTIO_NET_F_MTU`/`MQ`. Driver **không** dùng chúng; evidence in `mtu_raw`
   (không gọi là "MTU") để không ai đọc thành số thật.
2. **Chưa có ngắt**: `recv_frame` poll `used` (cùng KT-72/73). `recv_frame_wait` có timeout tick
   nên không treo; KT-246 dùng nó cho DHCP/ARP/ICMP.
3. **`PCI_DRIVER_LEAVES`**: ảnh net map **3** leaf 2 MiB của cửa sổ PCIe (leaf 1 để trống) vì
   `mmu_range_conflict` kiểm bằng **một** khoảng liền. Ảnh blk (1 leaf) và console (2 leaf)
   **không đổi** — số đo `assigned=0x40200000` của KT-73 giữ nguyên.
4. **`v2/bin/v2` bị sửa** (thêm bước 55) — tệp này thuộc ranh giới của sub-agent `infra` theo
   `KE-HOACH-CUOI-TUAN-2026-11-21.md` §3. Cần thiết vì `check-selftest-wiring.py` (KT-134) đòi mọi
   script có `--selftest`/`--red-selftest` phải có cổng gọi. Thay đổi **một** dòng hàm + **một**
   dòng `lane_step` + `lane_fails 54 → 55`; lead kiểm khi merge.
5. **`mmu.rs` bị sửa** (thêm leaf net) — cùng crate `vifast-plat-riscv64`, không ai khác trong vòng
   này chạm (a64 dùng `vifast-plat-aarch64`).
