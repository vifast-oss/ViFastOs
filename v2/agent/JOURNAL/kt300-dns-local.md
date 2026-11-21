# KT-300 — DNS **cục bộ** cho soak aarch64 (0/1000)

Trạng thái: **PASS — soak 0/1000** (nhánh `task/kt300-dnslocal`, base `main` @ `a78e67a9`),
2026-11-21. Sub-agent `dnslocal`. Quyết định: **D2-217** (mở KT-300), **D2-215** (KT-294 —
retransmit DNS), **D2-204** (KT-265 — bất biến soak), **D2-24 / D2-72 Q10 / D2-74 / D2-80**
(bất biến soak), **D2-07** (không retry), **D2-116** (hạ tầng), **D2-198** (uỷ quyền commit trên
nhánh task).

**Không** tuyên bố K4' đạt. Mục tiêu của task: soak aarch64 **0/1000** — DNS không còn phụ thuộc
resolver của host.

## 1. Khảo sát trước (đo trên máy này: QEMU 11.1.3, libslirp 4.9.7, macOS 27.0.1 arm64)

### 1.1 `-netdev user,dns=<ip>` hoạt động thế nào?

Đọc mã QEMU 11.1.3 (`net/slirp.c`) + libslirp 4.9.7 (`src/slirp.c`, `src/socket.c`):

- `dns=<ip>` chỉ gán `SlirpConfig.vnameserver` — **địa chỉ DNS ảo** mà khách thấy (DHCP option 6)
  và là địa chỉ slirp **chặn** ở cổng 53. Nó **không** đổi resolver thượng nguồn.
- `sotranslate_out4()` của libslirp: mọi datagram tới `vnameserver:53` (mặc định `10.0.2.3:53`)
  được chuyển tới `get_dns_addr()` — **resolver của host**. Trên macOS `get_dns_addr()` dùng
  `res_ninit()`/`res_getservers()` (libresolv) ⇒ đọc `/etc/resolv.conf` của host.
- `10.0.2.2:53` (gateway) **cũng** bị chặn làm DNS (đường tương thích VirtualBox/virtualbox#447)
  ⇒ cũng đi resolver host. Chỉ `10.0.2.2:<cổng ≠ 53>` mới map về loopback host
  (`loopback_map` mặc định `10.0.2.2 → 127.0.0.1`).
- Đo trên máy: `/etc/resolv.conf` = `nameserver 192.168.2.1`; `res_getservers()` trả
  `192.168.2.1:53`; `RESOLV_CONF` **không** được libresolv macOS tôn trọng (đo: vẫn
  `192.168.2.1`).

⇒ **Guest nhận DNS server nào qua DHCP?** DHCP option 6 = `vnameserver` (mặc định `10.0.2.3`);
ảnh aarch64 **không** đọc option 6 — nó hardcode `DNS_SERVER_IP = 10.0.2.3` trong
`vifast-plat-aarch64/src/virtio_net.rs`. Vì vậy `dns=` **một mình** không đổi được gì.

### 1.2 Có thể trỏ tới server DNS do runner dựng trên host không (qua `10.0.2.2`)?

**Không** qua đường QEMU-native:

- `guestfwd` của QEMU **chỉ TCP** — `slirp_guestfwd()` (`net/slirp.c`):
  `if (strcmp(buf, "tcp") && buf[0] != '\0') goto fail_syntax;`. Không có UDP.
- `hostfwd` là chiều **host → guest** (`hostfwd=[tcp|udp]:[hostaddr]:hostport-[guestaddr]:guestport`),
  không phải guest → host.
- `10.0.2.2:53` bị chặn làm DNS (mục 1.1) ⇒ không tới được server trên loopback host ở cổng 53.
- `-netdev user,dns=10.0.2.2` chỉ đổi **địa chỉ ảo**; slirp vẫn chuyển tới resolver host.

⇒ Phương án ưu tiên của đề bài ("`dns=10.0.2.2` + `guestfwd`/`hostfwd`") **bất khả thi** trên
QEMU 11.1.3 + libslirp 4.9.7. Ghi rõ ở đây và trong header `soak-aarch64.sh`.

### 1.3 Đường thay thế đã chọn (chỉ soak; ảnh và T2 giữ nguyên)

1. **Server DNS cục bộ**: `python3` UDP bind `0.0.0.0:53`, trả A cố định `192.0.2.123`
   (TEST-NET-1) TTL 3600 cho `example.com`; tên khác NXDOMAIN. Đo: macOS cho bind `0.0.0.0:53`
   **không cần root**; `127.0.0.1:53` trả `EACCES` (đo được, nên dùng wildcard).
2. **Chuyển resolver thượng nguồn của tiến trình QEMU**: dylib nhỏ `DYLD_INTERPOSE`
   `res_9_ninit`/`res_9_getservers`/`res_9_ndestroy` trả `127.0.0.1:53` ⇒ `get_dns_addr()` của
   libslirp trả loopback ⇒ slirp gửi truy vấn DNS của khách tới server cục bộ. **Chỉ** tiến trình
   QEMU của soak nạp dylib.
3. **Wrapper QEMU**: `qemu_guard.sh` chạy QEMU qua `perl` (`/usr/bin/perl`, binary SIP) — dyld
   **xoá** `DYLD_*` khỏi môi trường trước `exec` (đo được: `perl -e 'print $ENV{DYLD_INSERT_LIBRARIES}'`
   in `unset`). Wrapper bash đặt lại `DYLD_INSERT_LIBRARIES` rồi `exec` QEMU thật.
4. **Không** đổi ảnh (D2-08: không có nhánh "chế độ soak"), **không** đổi `run-aarch64.sh`
   (T2 vẫn đòi DNS **thật** qua slirp → resolver host), **không** đổi ngưỡng, **không** retry.

### 1.4 Vì sao không chọn các đường khác

| Đường | Lý do loại |
|---|---|
| Đổi ảnh sang `10.0.2.2:<cổng cao>` (loopback map) | T2 phải đổi theo (thêm forwarder) — trái yêu cầu "T2 giữ nguyên"; ảnh có nhánh chỉ dùng cho test (D2-08) |
| `sudo` + `pf` chuyển UDP 53 → 127.0.0.1:53 | Ảnh hưởng **toàn máy** (mọi tiến trình, kể cả agent khác); cần root; rủi ro rò rỉ rule |
| `sudo` + sửa `/etc/resolv.conf` | Ảnh hưởng toàn máy; cần root |
| Bind `192.168.2.1:53` (resolver host) | Không bind được IP của router |
| `guestfwd` UDP | QEMU không hỗ trợ (chỉ TCP) |
| DNS over TCP | Đề bài loại trừ; ảnh không có client TCP-DNS |

## 2. Thay đổi trong `v2/test/boot/soak-aarch64.sh` (chỉ tệp này)

- Header: thêm mục **KT-300** ghi khảo sát + lựa chọn + bằng chứng fail-closed + đột biến.
- Thêm server DNS cục bộ (`write_soak_dns_server`/`start_soak_dns_server`/`stop_soak_dns_server`),
  dylib interpose (`write_soak_dns_interpose`/`build_soak_dns_interpose`), wrapper QEMU
  (`write_soak_qemu_wrapper`).
- Trap `EXIT/INT/TERM` dọn thêm server DNS (chuỗi với `qg_cleanup`).
- Trước boot dò: dựng interpose + wrapper, `start_soak_dns_server`, `QEMU="$DNS_WRAPPER"`.
- Sau soak: dừng server DNS, đếm `queries`/`replies`/`local_hits`/`attempts1`; **fail-closed**
  nếu `queries < N+1` hoặc `replies < N+1` hoặc `local_hits ≠ N+1` (N+1 = probe + N boot).
- Đột biến qua `SOAK_DNS_MUTATION` (`off`/`nxdomain`/`wrongip`) — **chỉ** để kiểm chứng cơ chế,
  không phải cổng.

**Không** sửa: `v2/kernel/**`, `run-aarch64.sh`, `v2/agent/{STATE,QUEUE,DECISIONS}.md`,
`v2/bin/v2`, `spec/`. Không đổi ngưỡng nào (N=1.000, 3 luồng, 20 s/boot, timeout 3 s/lần,
`attempts=(1|2)`, TAP 35, `0 lỗi/N`, không retry).

## 3. Đo

### 3.1 Smoke (kiểm cơ chế, không phải bằng chứng cổng)

```text
$ SOAK_AARCH64_LOGDIR=/tmp/kt300/soak-smoke bash v2/test/boot/soak-aarch64.sh --smoke 6 3
# dns server: local=0.0.0.0:53 answer=192.0.2.123 ttl=3600 mutation=none dylib=…/kt300-dns-interpose.dylib
# probe: '# conf image:' khớp ELF (build=8809a6e159cb02b6 text=101920)
boot 1/6 ok … boot 6/6 ok
# tcp server: connections=7 sent13=7 recv15=7
# dns server: queries=7 replies=7 local_hits=7 attempts1=7 (cần ≥ 7 truy vấn/trả lời và đúng 7 local_hits)
# soak: xong wall=5s; ok=6 fail=0 tổng=6/6
```

Log boot (nguyên văn):

```text
# net dns: name=example.com id=0x4b55 rcode=0 an=1 reply_len=45 rtt_ms=2 elapsed_ms=3 attempts=1 const:timeout_ms=3000
# net: dns example.com a=192.0.2.123 ttl=3600
ok 35 - net: dns
```

Log server DNS (nguyên văn):

```text
listen port=53 mode= answer=192.0.2.123 ttl=3600
query id=0x4b55 name=example.com type=1 class=1 from=127.0.0.1:63542
reply id=0x4b55 rcode=0 an=1 len=45
```

### 3.2 Soak 1.000 (cổng) — **PASS 0/1000**

```text
$ SOAK_AARCH64_LOGDIR=/tmp/kt300/soak-1000 bash v2/test/boot/soak-aarch64.sh 1000 3
# soak-aarch64 KT-265 — 2026-11-21T11:32:20Z
# qemu 11.1.3 (sàn 9.2, D2-72 Q2)
# cấu hình: -M virt,gic-version=3 -cpu cortex-a72 -m 128M -nographic -smp 4 -no-reboot; KHÔNG icount (D2-74 c)
# N=1000 lanes=3 timeout=20s/boot; p=3/1000 (0,3 %) ⇒ n ≥ 3/p = 1000 (D2-24)
# git HEAD=a78e67a90ccb4b8943f59bb0883210c9ef6f204c dirty-kernel-paths=0
# infra before: qemu_total=0 qemu_ppid1=0 qemu_own=0 qemu_foreign=0 loadavg_1m=8.31 … verdict=OK
# image=product bytes=256768 sha256=0deb7fc6a908f8377a74d453cd65be46f8cfd12a6729492dbad0556d0fb85b22
# BUILD_ID (D2-80) = build=8809a6e159cb02b6 text=101920
# dns server: local=0.0.0.0:53 answer=192.0.2.123 ttl=3600 mutation=none dylib=…/kt300-dns-interpose.dylib
# probe: '# conf image:' khớp ELF (build=8809a6e159cb02b6 text=101920)
# soak: bắt đầu N=1000 lanes=3 11:32:30Z
… (1000 dòng `boot i/1000 ok`)
# tcp server: connections=1001 sent13=1001 recv15=1001 (mỗi boot một kết nối)
# dns server: queries=1001 replies=1001 local_hits=1001 attempts1=1001 (cần ≥ 1001 truy vấn/trả lời và đúng 1001 local_hits)
# infra after: qemu_total=0 qemu_ppid1=0 qemu_own=0 qemu_foreign=0 loadavg_1m=13.80 … verdict=KHONG_HOP_LE ly_do=loadavg_1m=13.80>ncpu=12
# infra: invalid=1 isolation_bad=0
# soak: xong wall=1100s; ok=1000 fail=0 tổng=1000/1000
# soak: BUILD_ID=8809a6e159cb02b6 sha256=0deb7fc6… git=a78e67a9…
# soak: 1000/1000 lỗi=0 — 0/N chỉ là trần (QD §5.3); không retry (D2-07)
# soak: 0/1000 lỗi (PASS)
# soak-aarch64.sh: PASS
exit=0
```

| Phép đo | Kết quả |
|---|---|
| N / luồng / timeout | **1.000** / **3** / **20 s** mỗi boot (không nới) |
| boot đỏ | **0** — `results.txt`: 1000 dòng `ok`, **0** dòng `fail` |
| wall | **1100 s** — **không** dùng để kết luận hiệu năng: `infra: invalid=1` do `loadavg_1m=13.80 > ncpu=12` ở mẫu **after** (tải của **chính** soak: 3 QEMU × 4 vCPU; `qemu_foreign=0` ở **cả hai** mẫu) |
| guard (`qemu_guard.sh`) | `boot_end=1001 isolated_1=1001 isolated_0=0 rc_0=1001 rc_124=0` (1000 soak + 1 probe) |
| `rc_124` (quá 20 s) | **0** |
| `qemu_own` / `qemu_foreign` | **0** / **0** sau khi xong; **0** trước khi bắt đầu |
| ảnh đổi giữa soak | **không** (sha256 đầu == cuối) |
| hash ảnh khớp ELF | **1000/1000** boot + probe (D2-80) |
| TCP server | **1001/1001/1001** — `accept`/`sent=13`/`recv=15` |
| **DNS server cục bộ** | **queries=1001 replies=1001** (probe + 1000 boot), **1001** câu trả lời `rcode=0 an=1` |
| **DNS local_hits** | **1001/1001** log boot có `# net: dns example.com a=192.0.2.123 ttl=3600` |
| **DNS attempts** | **1001/1001** dòng `# net dns:` có `attempts=1` (không retransmit) |
| retry | **0** (D2-07) |

Log boot đầu và boot cuối (nguyên văn):

```text
# boot-1.log
# net dns: name=example.com id=0x4b55 rcode=0 an=1 reply_len=45 rtt_ms=24 elapsed_ms=36 attempts=1 const:timeout_ms=3000
# net: dns example.com a=192.0.2.123 ttl=3600
ok 35 - net: dns

# boot-1000.log
# net dns: name=example.com id=0x4b55 rcode=0 an=1 reply_len=45 rtt_ms=17 elapsed_ms=18 attempts=1 const:timeout_ms=3000
# net: dns example.com a=192.0.2.123 ttl=3600
ok 35 - net: dns
```

Phân bố thời gian DNS (1000 boot, `rtt_ms` trong dòng `# net dns:`):

| Chỉ số | Giá trị |
|---|---|
| `rtt_ms` | min **1**, p50 **4**, p99 **24**, max **272**, mean **6,0** |
| `elapsed_ms` | min **0**, p50 **6**, p99 **497**, max **2529** (vẫn < 3000 ⇒ `attempts=1`) |
| `attempts` | **1000/1000** = 1 |

So sánh với vòng 16 (KT-292, resolver host): `rtt_ms` p50 **26**, p99 **189**, max **512**;
`elapsed_ms` p50 **27**, p99 **199**, max **3000** (1 boot timeout). DNS cục bộ hạ p50
**26 → 4 ms**; đuôi `elapsed_ms` còn lại là **đường virtio-net của khách dưới TCG** (máy chạy
soak có loadavg 13,8), không phải resolver.

### 3.3 Đột biến (chứng minh server cục bộ **thật sự** được dùng)

Chạy `--smoke 3 3` (3 boot) cho mỗi đột biến; `SOAK_DNS_MUTATION` chỉ để kiểm chứng cơ chế,
**không** phải cổng. Cả ba đều **ĐỎ** đúng như phải:

| Đột biến | Kết quả | Bằng chứng |
|---|---|---|
| `off` — **không dựng** server DNS | **ĐỎ** 3/3 boot | `# net dns: elapsed_ms=6001 attempts=2 err=Timeout` + `# net: no dns` + `not ok 35 - net: dns`; `# có 27 dòng 'ok', cần 35` |
| `nxdomain` — server trả **NXDOMAIN** | **ĐỎ** 3/3 boot | `# net dns: … rcode=3 an=0 … attempts=1` + `# net: no dns` + `not ok 35`; `local_hits=0` |
| `wrongip` — server trả **203.0.113.9** | **ĐỎ** (TAP 35 vẫn xanh) | `# net: dns example.com a=203.0.113.9 ttl=3600` + `ok 35` **nhưng** `local_hits=0` ⇒ `# soak-aarch64.sh: FAIL — DNS cục bộ không được dùng đủ (KT-300)` |

Đột biến `off` chứng minh: nếu server cục bộ không chạy, DNS **timeout** (không có resolver host
nào trả lời) ⇒ đường DNS của soak **thật sự** đi qua server cục bộ. Đột biến `nxdomain`/`wrongip`
chứng minh câu trả lời của khách **lấy từ** server cục bộ (không phải resolver host).

## 4. Còn thiếu / rủi ro

1. **`infra: invalid=1`** ở mẫu **after** (`loadavg_1m=13.80 > ncpu=12`) — tải của **chính**
   soak (3 QEMU × 4 vCPU dưới TCG) + nền máy; `qemu_foreign=0` ở **cả hai** mẫu, `rc_124=0`.
   Kết quả **đúng/sai** (0/1000) hợp lệ; **wall 1100 s không** dùng để kết luận hiệu năng
   (D2-116). Muốn `invalid=0` phải đo lại lúc máy rảnh — không đổi bất biến nào.
2. **Interpose là cơ chế macOS-specific** (`DYLD_INTERPOSE` + `res_9_*` + `perl` SIP): nếu macOS
   đổi hành vi dyld/libresolv, soak sẽ **ĐỎ** ở kiểm `local_hits`/`queries` (fail-closed), không
   âm thầm quay lại resolver host. Đã ghi rõ trong header script.
3. **Bind `0.0.0.0:53`** (không phải `127.0.0.1:53` — EACCES): server nghe trên mọi giao diện
   trong lúc soak; chỉ trả lời `example.com` ⇒ A `192.0.2.123`, tên khác NXDOMAIN. Nếu máy có
   tiến trình khác giữ cổng 53, soak **die** ngay (fail-closed).
4. **`attempts=1` chưa phải bất biến cứng**: kiểm hiện tại vẫn nhận `attempts=(1|2)` (giữ nguyên
   bất biến KT-294); `attempts1=1001` chỉ là **bằng chứng đo**. Nếu muốn siết thành bất biến
   (`attempts=1` bắt buộc) phải mở task riêng — không tự đổi trong KT-300.
5. **T2 (`run-aarch64.sh`) giữ nguyên** — vẫn đòi DNS **thật** qua slirp → resolver host; đã
   chạy lại xanh (xem §5).
6. **Không** tuyên bố K4' đạt: soak là một điều kiện; K4' còn timer per-hart/preemption, mỗi hart
   một address space/ASID, đọc DTB, W^X đầy đủ (`k4-aarch64-boot.md` §7).


## 5. Cổng cuối

```text
$ ./v2/bin/v2 check
…
# step-time-total   199960 ms  TỔNG check
exit=0

$ bash v2/test/boot/run-aarch64.sh
…
# ca âm: 21/21 ĐỎ đúng như phải
# 24/24 kiểm sạch (boot=1, tests=35)
# infra: guard_log=… qemu_own=0
exit=0
```

`run-aarch64.sh` **giữ nguyên** (không sửa một dòng): T2 vẫn đòi DNS **thật** qua slirp →
resolver host (không nạp interpose, không có server cục bộ) — 24/24 kiểm sạch, 21/21 ca âm ĐỎ.

## 6. Tệp đã đổi

| Tệp | Việc |
|---|---|
| `v2/test/boot/soak-aarch64.sh` | **sửa** — thêm DNS cục bộ (server + interpose + wrapper + kiểm fail-closed + đột biến); **không** đổi bất biến/ngưỡng nào |
| `v2/agent/JOURNAL/kt300-dns-local.md` | **mới** — journal này |
| `v2/agent/JOURNAL/kt300-soak-1000.log` | **mới** — toàn bộ console soak (header + 1000 dòng `boot i/1000 ok` + tổng) |
| `v2/agent/JOURNAL/kt300-soak-results.log` | **mới** — `results.txt`: 1000 dòng `ok`, 0 dòng `fail` |
| `v2/agent/JOURNAL/kt300-guard-summary.log` | **mới** — `boot_end=1001 isolated_1=1001 rc_0=1001 rc_124=0` |
| `v2/agent/JOURNAL/kt300-infra.log` | **mới** — mẫu `qg_infra` trước/sau |
| `v2/agent/JOURNAL/kt300-boot-1.log`, `kt300-boot-1000.log` | **mới** — log boot đầu/cuối (có dòng DNS cục bộ) |
| `v2/agent/JOURNAL/kt300-dns-server.log` | **mới** — 1001 truy vấn + 1001 câu trả lời của server cục bộ |
| `v2/agent/JOURNAL/kt300-dns-rtt.txt` | **mới** — 1000 giá trị `rtt_ms` của DNS (đã sắp xếp) |
| `v2/agent/JOURNAL/kt300-mut-off-boot-1.log` | **mới** — đột biến `off`: DNS timeout ⇒ ĐỎ |
| `v2/agent/JOURNAL/kt300-mut-nxdomain-boot-1.log` | **mới** — đột biến `nxdomain`: `rcode=3` ⇒ TAP 35 ĐỎ |
| `v2/agent/JOURNAL/kt300-mut-wrongip-boot-1.log` | **mới** — đột biến `wrongip`: `a=203.0.113.9` ⇒ `local_hits=0` ⇒ ĐỎ |
| `v2/agent/JOURNAL/kt300-run-aarch64.log` | **mới** — log T2 `run-aarch64.sh` (24/24, 21/21) |
| `v2/agent/JOURNAL/kt300-v2check.log` | **mới** — log `./v2/bin/v2 check` |

**Không** sửa: `v2/kernel/**`, `v2/test/boot/run-aarch64.sh`, `v2/agent/{STATE,QUEUE,DECISIONS}.md`,
`v2/bin/v2`, `spec/`, `docs/`.

## 7. Ràng buộc bằng chứng với commit

Soak chạy trên cây **sạch** (`dirty-kernel-paths=0`, `git HEAD=a78e67a9`) — chỉ có
`v2/test/boot/soak-aarch64.sh` + `v2/agent/JOURNAL/kt300-*` là thay đổi **ngoài** `v2/kernel`,
nên **không** đổi đầu vào build ảnh. Ảnh đã soak:

```text
# BUILD_ID (D2-80) = build=8809a6e159cb02b6 text=101920
sha256 = 0deb7fc6a908f8377a74d453cd65be46f8cfd12a6729492dbad0556d0fb85b22
bytes  = 256768
data+bss = 233704 / 262144 (89,1 %)
```

Commit chứa **đúng** nội dung cây đã soak (script + journal + log). **Không** chạy lại soak sau
commit (cùng fingerprint — AGENTS §6.3 cấm chạy lại soak trên cùng fingerprint chỉ để tăng số lần
PASS).
