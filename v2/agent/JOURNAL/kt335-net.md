# KT-335 — Hợp nhất `virtio_net` 3 arch + TCP client về `v2/lib/vifast-net`

**Chu kỳ 2 — Đợt 1.** Người làm: sub-agent `net335`. Ngày: 2026-11-22 (giờ máy Asia/Tokyo).

| | |
|---|---|
| Worktree | `/tmp/kt335-s1` (nhánh `task/kt335-net`) |
| Base | `main` @ `bfb9a13d` (`[KT-340c]`) |
| Máy | macOS, Rust/cargo `1.100.0` (ghim `v2/rust-toolchain.toml`) |
| Lệnh | `export PATH="$HOME/.cargo/bin:$PATH"`; `cargo --manifest-path v2/Cargo.toml` (lib), `cargo --manifest-path v2/kernel/Cargo.toml` (platform) |
| Nhãn số liệu | `wc -l` đo trực tiếp; log cổng/runner trong `/tmp/kt335-*.log` |

---

## 0. TL;DR

| Hạng mục | Trước | Sau |
|---|---:|---:|
| `virtio_net.rs` × 3 arch | **4.004 dòng** | **2.487 dòng** (**−37,89 %**) |
| `tcp.rs` × 2 arch | **750 dòng** | **264 dòng** (**−64,80 %**) |
| **Tổng 5 tệp driver** | **4.754 dòng** | **2.751 dòng** (**−42,13 %**) |
| Logic dùng chung mới | 0 | `netdev` 794 + `tcp_client` 524 = **1.318 dòng** (có host test) |
| Test host mới | — | **9** `netdev` + **10** `tcp_client` (tổng 19, xanh) |
| Đột biến | — | **2/2 ĐỎ** (bỏ kiểm checksum; bỏ tăng `seq`) |
| `vifast-net` `#![forbid(unsafe_code)]` | `deny` | **`forbid`** |
| TAP mạng giữ nguyên | — | x86_64 `# net:` (ARP/DHCP/ICMP/DNS), riscv64 `ktest-netsrv` 3 TAP, aarch64 `# net:` |
| `./v2/bin/v2 check` | xanh ở base | **PASS** (§7) |
| QEMU riscv64 matrix smp 1/2/4 | — | **27/27 boot sạch** (§6.1) |
| QEMU aarch64 | — | **27/27 kiểm sạch, tests=35** (§6.2, có 1 lần `T-INFRA` DNS) |
| QEMU x86_64 | — | **36/36 kiểm sạch, tests=36** (§6.3) |
| `.data+.bss` ba ảnh sản phẩm | 196.416 / 30.672 / 96.320 (base) | **196.416 / 30.672 / 96.320** (không đổi, §6.4) |

**Kết luận**: ba bản sao của cùng một giao thức virtio-net (status → feature → hai queue → descriptor
→ notify → poll `used` → `virtio_net_config`) đã gộp về **một** `Net` trong crate thuần; ba nền tảng
chỉ còn adapter MMIO/DMA/đồng hồ. Chính sách **TCP client** (bắt tay/gửi/nhận/đóng/retransmit) đã
dời khỏi hai bản `tcp.rs` nhân bản về `vifast_net::tcp_client` trên trait `Clock` + `Link`. TAP/API
của ảnh **không đổi** (§8). **Chưa** dời chính sách DHCP/ARP/ICMP/DNS khỏi driver — ghi rõ ở §9
(không tuyên bố đạt phần đó).

---

## 1. Vì sao (bối cảnh)

`D2-231` chốt: rút logic lặp của `pci`, `virtio_blk`, `virtio_net`, `virtio_gpu`, `virtio_input` về
lớp transport thuần (`MmioBus` + `DmaPool`, `forbid(unsafe_code)`), giữ adapter `unsafe` tối giản ở
`vifast-plat-*` (D2-17), và **đưa state machine TCP/client về `v2/lib/vifast-net`**, xoá hai bản
`tcp.rs` nhân bản. `KT-334` đã làm xong `pci` + `virtio_blk`; `KT-337` xong `virtio_gpu` +
`virtio_input`. `KT-335` là phần còn lại.

Ba `virtio_net.rs` ở base khác nhau **đúng bốn thứ** (đều do kiến trúc, không do phong cách):

1. **cửa sổ MMIO** — `read_volatile`/`write_volatile` (riscv64), `asm::mmio_*` + `dsb sy` (aarch64),
   `asm::read_u8/…` + `fence(SeqCst)` (x86_64);
2. **PA của trang DMA** — identity map `VA == PA` (riscv64/x86_64) vs PA **truyền vào** `init` vì
   trang nằm trong `.bss` dưới `KERNEL_HIGH_BASE` (aarch64, KT-273/KT-338a);
3. **nguồn thời gian** — timebase SBI (riscv64, tick) vs `CNTPCT_EL0` (aarch64, µs) vs PIT
   **do ảnh truyền vào** qua `now: &mut dyn FnMut() -> u64` (x86_64, µs);
4. **bộ trường `NetReport`** — mỗi bản một tập con, **không** bản nào là superset.

Hai `tcp.rs` khác nhau **đúng 66 dòng** trên 750 (`diff` đếm): tên hằng `*_TICKS` vs `*_US`, ba chỗ
`now_ticks()`/`now_us()`, một tham số `send_frame`, và `wrapping_` vs `saturating_` ở phép trừ hạn.

---

## 2. Khảo sát trước khi gộp — bảng theo hàm

Ký hiệu: `≈` = giống nhau về **quyết định**, chỉ khác cách chạm phần cứng.

| Khu vực / hàm | riscv64 (1.338 dòng) | aarch64 (1.395 dòng) | x86_64 (1.271 dòng) |
|---|---|---|---|
| Trạng thái driver | `struct State` trong `UnsafeCell` + `unsafe impl Sync` | 14 `static Atomic*` (không `unsafe`) | `struct State` trong `UnsafeCell` + `unsafe impl Sync` |
| MMIO | `read_volatile`/`write_volatile`, `fence rw, rw` | `asm::mmio_read_*`/`asm::mmio_write_*`, `dsb sy` | `asm::read_u8/…`, `fence(SeqCst)` |
| Trang DMA | `static DmaPage` trong `.bss`, `dma_page_addr()`, `VA == PA` | `asm::dma_page_net()` + `DMA_PA` **từ `init`**, PA ≠ VA | `static DmaPage` trong `.bss`, `VA == PA` |
| `init(pci …)` | `init(pci)` | `init(pci, dma_pa)` | `init(pci)` |
| Đồng hồ | `Riscv64Virt.now_ticks()` (tick) | `now_us()` = `CNTPCT/CNTFRQ` (µs) | tham số `now: &mut dyn FnMut() -> u64` (PIT của ảnh) |
| Trần poll `used` | `POLL_TIMEOUT_TICKS = 2 × TIMEBASE_HZ` | `POLL_TIMEOUT_US = 2_000_000` | `POLL_TIMEOUT_US = 2_000_000` |
| Queue setup (rx=0, tx=1) | ≈ giống | ≈ giống | ≈ giống |
| `arm_rx` | ≈ (1 descriptor `WRITE`, notify ngay) | ≈ | ≈ |
| `send_frame` | `(frame)` | `(frame, timeout_us)` | `(frame, timeout_us, now)` |
| `recv_frame`/`_wait` | `(out)` / `(out, timeout_ticks)` | `(out)` / `(out, timeout_us)` | `(out)` / `(out, timeout_us, now)` |
| `NetReport` | 30 trường | 25 trường (thiếu `bus/dev/func`, `notify_addr_*`) | 33 trường (thêm `bus/dev/func`) |
| ARP | `arp_build_request`, `arp_parse_reply` (**private**), `arp_resolve` | `arp_build_request`, `arp_parse_reply` (**pub**), `arp_resolve` | `arp_build_request`, `arp_parse_reply` (**private**), `arp_resolve` |
| DHCP | `dhcp_acquire`; `dhcp_frame`/`dhcp_frame_parse`/`dhcp_wait` **private** | `dhcp_acquire`; `dhcp_frame`/`dhcp_frame_parse` **pub** | `dhcp_acquire`, `dhcp_acquire_with(io)`; **`DhcpLease`** + `dhcp_lease_from_ack` + `dhcp_filter_selfcheck` + `udp_frame` |
| ICMP echo | `ping` | `ping`, `icmp_echo_frame` | `ping_with` + bộ lọc trong `net_client.rs` (KT-328) |
| DNS | `dns_query` | `dns_query` | `dns_query_with` trong `net_client.rs` (KT-328) |
| UDP | `udp_send`/`udp_recv`/`udp_echo` | như riscv64 | `udp_frame` + `NetIo` trait (KT-326) |
| TCP | `tcp.rs` 374 dòng: **máy trạng thái ở `vifast_net::tcp`**, tệp này là **chính sách** | `tcp.rs` 376 dòng: cùng khuôn, khác đồng hồ | **không có** (`ktest-net` x86_64 không có TAP TCP) |

**TCP state machine đang ở đâu**: máy trạng thái RFC 793 (`ESTABLISHED`/`SYN-SENT`/cửa sổ/
`on_timeout`/checksum) **đã** ở `v2/lib/vifast-net/src/tcp.rs` (764 dòng, KT-268, 43 host test) từ
trước KT-335. Cái **còn nhân bản** là **chính sách client** — dựng frame Ethernet/IPv4/TCP, gửi,
bơm frame tới `on_segment`, thi hành `Action`, retransmit có hạn, FIN/RST — ở
`vifast-plat-{riscv64,aarch64}/src/tcp.rs` (374 + 376 dòng). Ai gọi: `vifast-kbin-riscv64/src/ktest_net.rs`
(`tcp::tcp_connect` dòng 921, `tcp_recv` 992, `tcp_send` 1016, `tcp_close` 1021) và
`vifast-kbin-aarch64/src/net.rs` (289, 348, 365, 366). Đó là phần KT-335 dời đi.

`vifast-net` ở base **không rỗng**: 11 module codec thuần (addr/checksum/eth/arp/ipv4/icmp/udp/
dhcp/dns/tcp/frame), 127 host test (14+13+26+11+20+43 = 127), **không** dependency, `deny(unsafe_code)`.
Thiếu đúng một thứ: **chính sách** (thứ cần một đường frame + đồng hồ).

---

## 3. Thiết kế sau khi gộp

### 3.1 Transport (`v2/lib/vifast-virtio/src/netdev.rs`, 794 dòng)

- `Net` — 24 trường `Atomic*`/`AtomicBool` để platform khai `static NET: Net = Net::new()` mà
  **không** `UnsafeCell`/`unsafe` (cùng khuôn `blkdev::Blk` của KT-334);
- `Clock` — **một** hàm `now(&mut self) -> u64`, đơn vị do nền tảng quy ước; vì sao **không** dùng
  `blkdev::Clock`: đồng hồ của blk mang `deadline()`/`spin_limit()` cho mô hình trần **tuyệt đối**,
  còn net nhận **timeout tương đối** ở từng lời gọi (`recv_frame_wait(out, timeout)` là API đã có
  của cả ba nền tảng) — hai hợp đồng khác nhau thì để hai trait khác nhau;
- `init<B: MmioBus, D: DmaPool>(…)` giữ **đúng** chuỗi chặng cũ và **đúng tên** chặng:
  `layout, dma_align, reset, version1, features, features_ok, queues, queue_size, queue_enable,
  driver_ok, config_generation, config, mac, arm`;
- `send_frame`/`recv_frame`/`recv_frame_wait` nhận `timeout` từ người gọi (đơn vị `Clock`);
- `NetReport` = **hợp** của ba bộ trường cũ (không bản nào là superset) — không ảnh nào phải bỏ
  trường đang in;
- `DmaPool` cấp **một** trang 4 KiB; bố cục ring/đệm giữ nguyên
  (`RX_RING_OFF=0x000`, `TX_RING_OFF=0x200`, `RX_BUF_OFF=0x400`, `TX_BUF_OFF=0xA00`) nên TAP/ảnh
  không đổi; hai hook `sync_for_device`/`sync_for_cpu` được gọi đúng chỗ (bàn giao ring `rx`/`tx`,
  nhận lại đệm `rx`) — riscv64/x86_64 là no-op, aarch64 giữ **nguyên** hành vi cũ (bản KT-275
  **không** có `dc civac` cho virtio-net; thêm vào là đổi hành vi DMA chưa đo lại);
- **fence** đi qua `MmioBus::fence` (riscv64 `fence rw,rw`, aarch64 `dsb sy`, x86_64 `mfence`) —
  không còn ba bản sao thứ tự spec §2.7.13.

### 3.2 TCP client (`v2/lib/vifast-net/src/tcp_client.rs`, 520 dòng)

- hai trait biên: `Clock { now }` và `Link { send_frame, recv_frame }`;
- `ClientConfig` (địa chỉ/cổng/ISS/MSS/cửa sổ/khoảng retransmit) + `TcpConn` (máy trạng thái +
  cấu hình) + `connect`/`send`/`recv`/`close`;
- `LinkError { BadFrame, Timeout, Io }` — **tên khớp** `NetError` để adapter ánh xạ 1-1, TAP in mã
  lỗi y như trước;
- **giữ nguyên** bất biến cũ: mỗi bước có trần, retransmit ≤ `MAX_RETRANSMITS` = 3, fail-closed,
  bỏ qua frame không khớp (kể cả checksum sai), không cấp phát.

### 3.3 Adapter (mỗi nền tảng)

| Nền tảng | Adapter | Dòng |
|---|---|---:|
| riscv64 | `Mmio` (con trỏ thô + `fence rw,rw`) / `Dma` (trang `.bss`, identity) / `Tick` (timebase) + chính sách DHCP/ARP/ICMP/UDP/DNS | 840 |
| aarch64 | `Mmio` (`asm::mmio_*` + `dsb sy`) / `Dma` (`asm::dma_page_net` + PA từ `init`) / `Micro` (`CNTPCT`) + chính sách | 860 |
| x86_64 | `Mmio` (`asm::read_u8/…` + `fence(SeqCst)`) / `Dma` (trang `.bss`) / `Pit` (đồng hồ PIT **của ảnh**) + chính sách (ARP/DHCP/ICMP/DNS) | 787 |
| riscv64 | `tcp.rs` → **adapter** `Tick`+`Wire` trên `tcp_client` | 374 → **132** |
| aarch64 | `tcp.rs` → **adapter** `Micro`+`Wire` trên `tcp_client` | 376 → **132** |

---

## 4. Bảng dòng trước/sau từng tệp

| Tệp | Trước | Sau | Δ |
|---|---:|---:|---:|
| `vifast-plat-riscv64/src/virtio_net.rs` | 1.338 | 840 | **−498 (−37,22 %)** |
| `vifast-plat-aarch64/src/virtio_net.rs` | 1.395 | 860 | **−535 (−38,35 %)** |
| `vifast-plat-x86_64/src/virtio_net.rs` | 1.271 | 787 | **−484 (−38,08 %)** |
| `vifast-plat-riscv64/src/tcp.rs` | 374 | 132 | **−242 (−64,71 %)** |
| `vifast-plat-aarch64/src/tcp.rs` | 376 | 132 | **−244 (−64,89 %)** |
| **Tổng 5 tệp driver** | **4.754** | **2.751** | **−2.003 (−42,13 %)** |
| `vifast-virtio/src/netdev.rs` (mới) | 0 | 794 | +794 |
| `vifast-net/src/tcp_client.rs` (mới) | 0 | 524 | +524 |
| `vifast-virtio/tests/netdev.rs` (mới) | 0 | 531 | +531 (test) |
| `vifast-net/tests/tcp_client.rs` (mới) | 0 | 474 | +474 (test) |
| **Tổng mã nguồn phải bảo trì** | **4.754** | **4.069** | **−685 (−14,41 %)** |

Đọc bảng cho đúng: **logic** không còn nhân bản (một `netdev`, một `tcp_client`), nhưng tổng dòng
chỉ giảm 14,4 % vì (a) tài liệu trong crate thuần dày hơn bản driver, (b) chính sách
DHCP/ARP/ICMP/DNS **vẫn** nằm ở ba tệp platform (§9). Nếu chỉ nhìn "dòng driver phải sửa mỗi lần
đổi giao thức virtio" thì con số là **4.004 → 794** (−80,2 %) — đó mới là thứ KT-335 nhắm.

---

## 5. Test host + đột biến

### 5.1 Test host mới

| Bộ | Số ca | Nội dung |
|---|---:|---|
| `v2/lib/vifast-virtio/tests/netdev.rs` | **9** | init đọc **mọi** trường từ thiết bị; `version1`; thiết bị từ chối `FEATURES_OK`; queue < 2; `config_generation` **xé**; MAC toàn 0; gọi trước `init`; frame > 1514 ⇒ `BadFrame` **trước** ring; thiết bị không hoàn tất ⇒ `Timeout` + đếm |
| `v2/lib/vifast-net/tests/tcp_client.rs` | **10** | bắt tay 3 bước; gửi/nhận; **cửa sổ 0**; **mất gói + retransmit** (SYN rơi 2 lần); **checksum sai**; **thứ tự sai** (ngoài cửa sổ ⇒ ACK lại, **không** giao dữ liệu); FIN; RST; **`seq` tăng sau khi gửi**; hằng cờ |

Test TCP dựng **server TCP giả trên byte thật** (encode/parse qua chính codec của crate) nên phép
kiểm không mock tầng TCP. `cargo test -p vifast-virtio --test netdev`: **9/9 xanh**.
`cargo test -p vifast-net --test tcp_client`: **10/10 xanh**.

**Đỏ trước (D2-05)**: chạy `cargo test -p vifast-net --test tcp_client` **trước** khi viết module:
`error[E0432]: unresolved import vifast_net::tcp_client` ⇒ `could not compile`. Sau đó mới viết
`tcp_client.rs`.

### 5.2 Đột biến — **cả hai ĐỎ**

**(i) Bỏ kiểm checksum** — `tcp_client::step_until`: `TcpHeader::parse(…, true)` → `false`.

```text
test bad_checksum_is_ignored ... FAILED
test result: FAILED. 9 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
panicked at lib/vifast-net/tests/tcp_client.rs:382:10
```

**(ii) Bỏ tăng `seq` sau khi gửi** — `vifast_net::tcp::TcpConnection::build_data`: xoá dòng
`self.snd_nxt = self.snd_nxt.wrapping_add(len as u32);`.

```text
test seq_advances_after_send ... FAILED
test send_and_receive_payload ... FAILED
test result: FAILED. 8 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out
panicked at lib/vifast-net/tests/tcp_client.rs:453:66
panicked at lib/vifast-net/tests/tcp_client.rs:303:78
```

Cả hai đột biến đã **phục hồi** ngay sau khi ghi log; cây làm việc không còn dấu vết đột biến.

---

## 6. Ba runner (QEMU)

Đo trên commit `495eb3ad` (inputs `5e5f6baa9dfca229`, `head_rev=495eb3ad7b7d`, cây **sạch**,
`REV-GATE: ACCEPT`). Lệnh theo đúng thứ tự, **một QEMU một lúc**.

### 6.1 riscv64 — `bash v2/test/boot/ktest.sh 1 matrix`

```text
# infra before: qemu_total=0 qemu_ppid1=0 qemu_own=0 qemu_foreign=0 loadavg_1m=5.44 loadavg_5m=7.12 loadavg_15m=6.64 ncpu=12 boot_done=0 verdict=OK
REV-GATE: ACCEPT inputs=5e5f6baa9dfca229 current_inputs=5e5f6baa9dfca229 head_rev=495eb3ad7b7d count=354 log=/var/folders/jp/05hgrzxj65bgh355l2zs2zz80000gn/T//k3-ktest/kt86-rev-gate.log
# infra after: qemu_total=0 qemu_ppid1=0 qemu_own=0 qemu_foreign=0 loadavg_1m=2.99 loadavg_5m=5.23 loadavg_15m=5.93 ncpu=12 boot_done=28 verdict=OK
# KTEST: 27/27 boot sạch (smp=matrix, NBOOT=1, ảnh/boot=ktest ktest-guard ktest-user ktest-proc ktest-vblk ktest-con ktest-exec ktest-ipc ktest-netsrv)
# ktest.sh: PASS
```

Log: `/tmp/kt335-rv-matrix2.log`. `qemu_foreign=0` cả hai đầu ⇒ phép đo hợp lệ (D2-116).

### 6.2 aarch64 — `bash v2/test/boot/run-aarch64.sh`

```text
# ca âm: 24/24 ĐỎ đúng như phải
# 27/27 kiểm sạch (boot=1, tests=35)
# infra: guard_log=/var/folders/jp/05hgrzxj65bgh355l2zs2zz80000gn/T//k4-aarch64/qemu-guard.log qemu_own=0
```

Log: `/tmp/kt335-aarch643.log`.

**Một lần đỏ trước đó — `T-INFRA`, đã phân loại (D2-07)**: lần chạy `/tmp/kt335-aarch642.log` ra
`# 26/27 kiểm sạch (boot=1, tests=35)`; kiểm đỏ là **TAP 35 `net: dns`**:

```text
not ok 35 - net: dns
# net dns: elapsed_ms=6000 attempts=2 err=Timeout
# net stats: tx_frames=14 tx_bytes=1329 rx_frames=13 rx_bytes=1897 notifies=28 errors=0 timeouts=0 id_mismatch=0
```

Đây **không** phải hồi quy của driver: cùng ảnh đó, ARP + ICMP + **TCP (ok 33/34)** xanh, `net stats`
`errors=0 timeouts=0`, và TAP DNS chỉ phụ thuộc **resolver của host** qua slirp (`example.com`).
Chạy lại **một lần** (đúng D2-07: chỉ để phân loại) ⇒ 27/27. Ghi tên nợ: **T-INFRA — DNS ngoài
timeout 6 s dưới tải** (`loadavg_1m ≈ 6,9` trên 12 core vì một task khác đang chạy QEMU).

### 6.3 x86_64 — `bash v2/test/boot/run-x86_64.sh`

```text
# ca âm: 32/32 ĐỎ đúng như phải (+ 1 ca dương KT-313)
# 36/36 kiểm sạch (boot=1, tests=36, exc=6)
# infra: cpu=qemu64 guard_log=/var/folders/jp/05hgrzxj65bgh355l2zs2zz80000gn/T//k5-x86_64/qemu-guard.log qemu_own=0
```

Log: `/tmp/kt335-x86_642.log`.

### 6.4 `.data+.bss` ba ảnh **sản phẩm** (trần 262.144 B, **không** nới)

| Ảnh | `.data` | `.bss` | `.data+.bss` | Trần | Kết luận |
|---|---:|---:|---:|---:|---|
| riscv64 `vifast-kbin-riscv64` | 8.136 | 188.280 | **196.416** | 262.144 | OK (−65.728) |
| aarch64 `vifast-kbin-aarch64` | 232 | 30.440 | **30.672** | 262.144 | OK (−231.472) |
| x86_64 `vifast-kbin-x86_64-product` | 1.168 | 95.152 | **96.320** | 262.144 | OK (−165.824) |

Lệnh: `budget.sh` / `budget-aarch64.sh` / `budget-x86_64.sh`; log `/tmp/kt335-*.log` và
`/var/folders/…/k5-x86_64/`.

### 6.5 `size_of::<Machine>()` và R1

- `size_of::<Machine>()` **không** đổi: KT-335 không chạm `vifast-core`/`Machine` (chỉ thêm `Net`
  phía driver, không nằm trong `Machine`).
- **R1 ≤ 1.500 dòng/tệp**: tệp lớn nhất của KT-335 là `netdev.rs` **794** dòng; ba `virtio_net.rs`
  còn 840/860/787 — đều dưới trần. `v2 check` xanh ⇒ luật cấu trúc đạt.

### 6.6 TAP mạng giữ nguyên — bằng chứng đo được

aarch64 (`boot-1.log`):

```text
# net: dhcp ip=10.0.2.15 gw=10.0.2.2 mask=255.255.255.0 server=10.0.2.2
# net: arp gw=52:55:0a:00:02:02
# net: ping 10.0.2.2 ok n=3 rtt=0
# net: tcp connect 10.0.2.2:45678 ok rtt=2 state=established
# net tcp data: recv=13 sent=15 send=ok close=ok state=fin-wait-2 retransmits=0 elapsed_ms=2 const:timeout_ms=3000
# net: arp dns=52:55:0a:00:02:03 ip=10.0.2.3
# net: dns example.com a=172.66.147.243 ttl=274
# net stats: tx_frames=13 tx_bytes=1258 rx_frames=13 rx_bytes=1940 notifies=27 errors=0 timeouts=0 id_mismatch=0
```

x86_64 (`/var/folders/…/k5-x86_64/`):

```text
# net: arp gw=10.0.2.2 mac=52:55:0a:00:02:02 sent=1 recv=1 elapsed_us=4750 errors=0 timeouts=0
# net: dhcp lease=86400 ip=10.0.2.15 mask=255.255.255.0 gw=10.0.2.2 dns=10.0.2.3 server=10.0.2.2 attempts=2 elapsed_us=41585 filter=cookie:1 xid:1
# net: icmp target=10.0.2.2 reply=1 id=0x4b54 seq=1 payload=32 rtt_ms=0 attempts=1 elapsed_ms=7 filter=id:1 seq:1 payload:1
# net: dns name=localhost a=127.0.0.1 ttl=0 id=0x4b55 rcode=0 an=1 reply_len=43 rtt_ms=41 elapsed_ms=58 attempts=1 const:timeout_ms=3000 filter=id:1 qr:1 rcode:1
```

riscv64: `ktest-netsrv` **3 TAP** (`plan=3 ok=3`) + `ktest-ipc` 11 TAP — đều xanh trong log matrix.



---

## 7. `./v2/bin/v2 check`

```text
v2 check: PASS (L1 chạy trong gate --fast)
# step-time-total    86401 ms  TỔNG check
```

Log: `/tmp/kt335-check3.log` (chạy trên cây **sạch** tại `495eb3ad`). Lượt đầu đỏ hai bước và đã
sửa **trước** khi commit: `fmt` (rustfmt trên tệp test mới) và `clippy`
`this function has too many arguments (8/7)` ở `ClientConfig::new` ⇒ thêm
`#[allow(clippy::too_many_arguments)]` + lý do (cùng khuôn `udp_echo`/`dns_query` sẵn có), **không**
nới ngưỡng nào của cổng.


---

## 8. TAP/API giữ nguyên

- **riscv64**: `ktest-netsrv` **3 TAP** (`# netsrv:` + `not ok 1` cho ca "không thiết bị"), ảnh
  `ktest`/`ktest-vblk`/`ktest-ipc`… không đổi; `ktest.sh` in đúng `expected[...]: plan=N ok=N`.
- **aarch64**: `# net:` (ARP/DHCP/ICMP/DNS) + `# net: tcp …` — chữ ký `virtio_net::*` và
  `tcp::*` giữ nguyên nên `vifast-kbin-aarch64/src/net.rs` **không** phải sửa một dòng.
- **x86_64**: `# net:` ARP/DHCP/ICMP/DNS (KT-326/KT-328) — `NetIo`/`udp_frame`/`dhcp_*` giữ nguyên,
  chỉ thân các hàm `send_frame`/`recv_frame_wait` đổi sang gọi `netdev` (chữ ký có `now` **không**
  đổi).
- `DMA_PAGE_BYTES`, `RX_*`/`TX_*_OFF`, `POLL_TIMEOUT_*`, `NET_STEP_TIMEOUT_*`, `ICMP_PAYLOAD_BYTES`,
  `NetReport`, `Stats`, `past_deadline`, `now_us`/`now_ticks`, `high_to_pa` đều **re-export** —
  host test của platform (`vifast-plat-aarch64/tests/{virtio_net,tcp}.rs`) không đổi import.

---

## 9. Nợ còn mở (ghi rõ, **không** tuyên bố đạt)

`D2-231` nói "chuyển **toàn bộ** state machine TCP/**DHCP**/**ARP**/**ICMP**/**DNS** dùng trait
`Clock` vào `vifast-net`". KT-335 làm xong:

- ✔ **transport** của virtio-net (một `netdev`, ba adapter) — hết nhân bản;
- ✔ **TCP client** (một `tcp_client`) — xoá hai bản `tcp.rs` nhân bản, hai tệp còn lại là adapter
  132 dòng;
- ✔ máy trạng thái TCP thuần đã ở `vifast-net` từ KT-268 (không phải việc của task này).

**Chưa** làm (cần task riêng, ước lượng 2–3 h):

- **chính sách DHCP/ARP/ICMP/UDP/DNS** vẫn ở ba `virtio_net.rs`. Chúng **không** giống nhau đủ để
  gộp cơ học: x86_64 dùng trait `NetIo` + ba trần thời gian (`DHCP_TOTAL/ATTEMPT/MAX_ATTEMPTS`,
  KT-326) và có `DhcpLease`/`dhcp_filter_selfcheck` mà hai arch kia không có; aarch64 `pub`
  `dhcp_frame`/`dhcp_frame_parse` cho host test; riscv64 giữ chúng `private`. Gộp đúng cần **hợp
  nhất hợp đồng ba trần** trước (quyết định thiết kế, không phải di chuyển mã), nếu không sẽ đổi
  hành vi timeout của TAP x86_64 — trái D2-05.
- `netdev` **chưa** có test host dựng thiết bị giả **hoàn tất** `tx`/`rx` thật (chỉ có 9 ca
  fail-closed + đường init + timeout). Test "thiết bị giả bơm frame vào `rx`" cần một thiết bị mô
  phỏng ring hai chiều — để lại thành nợ có tên, **không** giả vờ đã phủ.

Đề xuất: mở **KT-335b** (gộp chính sách DHCP/ARP/ICMP/DNS + hợp nhất hợp đồng ba trần thời gian),
và ghi nợ test thiết bị giả hai chiều vào đó.

---

## 10. Bài học

1. **"Cùng một giao thức" không có nghĩa "cùng một API"**: ba `NetReport` khác nhau khiến gộp phải
   lấy **hợp**, không lấy **giao** — nếu lấy giao thì hai ảnh mất trường đang in và TAP đổi âm thầm.
2. **Hook `sync_for_*` phải gọi đúng chỗ, không gọi cho có**: bản aarch64 của virtio-net **không**
   có `dc civac` (khác virtio-blk). Thêm vào lúc gộp là "cải tiến" không có phép đo — đúng lớp lỗi
   D2-05 (đột biến hành vi không có test đỏ trước). Giữ no-op + ghi lý do.
3. **Test phải khẳng định đúng thứ đột biến phá**: đột biến "bỏ tăng `seq`" chỉ bị bắt vì có ca
   `seq_advances_after_send` **so sánh hai `seq`**; nếu chỉ kiểm "gửi không lỗi" thì đột biến vẫn
   xanh — đúng `k6-test-invariants.md` §1 (phép kiểm xanh oan).
4. **Cache của `cargo check -p` đánh lừa**: `cargo check -p vifast-plat-*` báo `Finished` 0,02 s dù
   tệp vừa sửa; lỗi biên dịch thật chỉ hiện khi **build ảnh** (`--features ktest`). Từ nay với
   `v2/kernel` phải build đúng ảnh, không tin `check -p`.
