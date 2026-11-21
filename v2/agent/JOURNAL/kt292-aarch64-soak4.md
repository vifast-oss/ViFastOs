# KT-292 — soak aarch64 **1.000 trên ảnh hiện tại** (TCP + DNS, TAP 35) — **KHÔNG ĐẠT 0/1000**

Trạng thái: **FAIL — 999/1000** (nhánh `task/kt292-a64soak4`, base `main` @ `36e709ce`), 2026-11-21.
Sub-agent `a64soak4`. Quyết định: **D2-214** (mở KT-292), **D2-204** (KT-265 — bất biến soak),
**D2-213** (KT-289 — TCP/DNS aarch64), **D2-24 / D2-72 Q10 / D2-74 / D2-80** (bất biến soak),
**D2-07** (không retry), **D2-116** (hạ tầng), **D2-198** (uỷ quyền commit trên nhánh task).

**Không** tuyên bố K4' đạt. Soak **KHÔNG đạt** 0/1000: **1 boot đỏ** (boot **138**) vì **DNS timeout**
— lỗi **thật** của client DNS (chỉ gửi **một** truy vấn, không retransmit), **không** phải T-INFRA.
Lead đã phân loại và mở **KT-294** (DNS retransmit 2 lần, giữ nguyên timeout 3 s/lần). Task này
**không** sửa `v2/kernel/**`; chỉ đo và ghi nhận.

## 1. Phạm vi

| Việc | Kết quả |
|---|---|
| `soak-aarch64.sh`: kỳ vọng TAP **35** + TCP/DNS | **ĐÃ CÓ SẴN** — KT-289 (`0f6e244d`) đã đặt `TAP_TESTS=35`, dựng TCP server host, thêm 4 kiểm `# net: tcp …`/`# net tcp data:`/`# net: dns …`; `run-aarch64.sh` đã `EXPECT_TESTS=35`. Task này **không** phải sửa kỳ vọng |
| `--selftest` | **38/38** (1 dương + 37 âm), không xoá ca cũ nào |
| **Soak thật 1.000** trên HEAD `36e709ce` | **999/1000** — **1 đỏ** (boot 138, DNS timeout); hạ tầng **HỢP LỆ** (`invalid=0`) |

**Chỉ thêm kiểm, không bỏ kiểm cũ**: không đổi một dòng nào của `soak-aarch64.sh` (kỳ vọng TAP 35
và bất biến TCP/DNS đã có từ KT-289). Không ngưỡng nào bị nới: N=1.000, 3 luồng, 20 s/boot,
0 lỗi/N, không retry.

## 2. Kết quả soak

```text
$ bash v2/test/boot/soak-aarch64.sh 1000 3
# soak-aarch64 KT-265 — 2026-11-21T07:55:13Z
# qemu 11.1.3 (sàn 9.2, D2-72 Q2)
# cấu hình: -M virt,gic-version=3 -cpu cortex-a72 -m 128M -nographic -smp 4 -no-reboot; KHÔNG icount (D2-74 c)
# N=1000 lanes=3 timeout=20s/boot; p=3/1000 (0,3 %) ⇒ n ≥ 3/p = 1000 (D2-24)
# git HEAD=36e709ce64d18653d755b049823faca33b2a030e dirty-kernel-paths=0
# infra before: qemu_total=0 qemu_ppid1=0 qemu_own=0 qemu_foreign=0 loadavg_1m=4.00 ... verdict=OK
# image=product bytes=256768 sha256=8eedfd0ec3307697218028d63c8020a78bc93bfeb67a64776b238b243e78c68b
# BUILD_ID (D2-80) = build=194ad9604b0a18b0 text=101668
# probe: '# conf image:' khớp ELF (build=194ad9604b0a18b0 text=101668)
# soak: bắt đầu N=1000 lanes=3 07:55:21Z
...
# tcp server: connections=1001 sent13=1001 recv15=1001 (mỗi boot một kết nối)
# infra after: qemu_total=0 qemu_ppid1=0 qemu_own=0 qemu_foreign=0 loadavg_1m=6.53 ... verdict=OK
# infra: invalid=0 isolation_bad=0
# soak: xong wall=1052s (0 boot/s); ok=999 fail=1 tổng=1000/1000
# soak: danh sách boot đỏ (lý do):
#   fail 138 #,có,27,dòng,'ok',,cần,35;#,có,1,dòng,'not,ok';... (DNS timeout)
# soak: BUILD_ID=194ad9604b0a18b0 sha256=8eedfd0e... git=36e709ce...
# soak: 999/1000 lỗi=1 — 0/N chỉ là trần (QD §5.3); không retry (D2-07)
# soak-aarch64.sh: FAIL (1/1000 boot đỏ)
exit=1
```

| Phép đo | Kết quả |
|---|---|
| N / luồng / timeout | **1.000** / **3** / **20 s** mỗi boot (không nới) |
| boot đỏ | **1** — `results.txt`: 999 dòng `ok`, **1** dòng `fail` (boot **138**) |
| wall | **1052 s** (≈ 0,95 boot/s) — **HỢP LỆ**: `qemu_foreign=0` ở **cả hai** mẫu, `invalid=0` |
| guard (`qemu_guard.sh`) | `boot_end=1001 isolated_1=1001 isolated_0=0 rc_0=1001 rc_124=0` (1000 soak + 1 probe) |
| `rc_124` (quá 20 s) | **0** — không boot nào chạm timeout |
| `qemu_own` | **0** sau khi xong — không QEMU mồ côi |
| `qemu_foreign` | **0** trước **và** sau |
| ảnh đổi giữa soak | **không** (sha256 đầu == cuối) |
| hash ảnh khớp ELF | **1000/1000** boot + probe (D2-80) |
| TCP server (bằng chứng hai chiều) | **1001/1001/1001** — `accept`/`sent=13`/`recv=15` (1000 soak + 1 probe) |
| retry | **0** (D2-07) |

## 3. Boot đỏ duy nhất — boot 138, **DNS timeout**

`boot-138.log` (log đầy đủ: `kt292-boot-138.log`):

```text
# net: tcp connect 10.0.2.2:45678 ok rtt=12 state=established
# net tcp connect: iss=0x4b540001 snd_una=0x4b540002 rcv_nxt=0xfa02 retransmits=0 elapsed_ms=12 const:timeout_ms=3000 const:port=45678
ok 33 - net: tcp connect
# net: tcp recv=13 first=vifast-tcp-ok
# net tcp data: recv=13 sent=15 send=ok close=ok state=fin-wait-2 retransmits=0 elapsed_ms=18 const:timeout_ms=3000
ok 34 - net: tcp data
# net: arp dns=52:55:0a:00:02:03 ip=10.0.2.3
# net dns: elapsed_ms=3000 err=Timeout
# net: no dns
not ok 35 - net: dns
# net stats: tx_frames=13 tx_bytes=1258 rx_frames=13 rx_bytes=1897 notifies=27 errors=0 timeouts=0 id_mismatch=0
```

- **TCP cùng boot xanh** (bắt tay, nhận 13 byte, gửi 15 byte, đóng FIN) ⇒ đường mạng/virtio-net
  **không** hỏng; chỉ **DNS** timeout.
- Ảnh **halt sau TAP đỏ** ⇒ log thiếu `ok 26..32` (W^X/DMA/GPU/input) và marker `boot ok`; đó là
  hệ quả của TAP 35 đỏ, không phải lỗi thêm.
- **Nguyên nhân gốc** (lead xác nhận): `dns_query` (`v2/kernel/vifast-plat-aarch64/src/virtio_net.rs`)
  gửi **đúng một** truy vấn UDP rồi chờ `NET_STEP_TIMEOUT_US` = 3000 ms; **không retransmit**
  (RFC 1035 §4.2.1 khuyến nghị resolver phải thử lại). Một lần resolver/slirp chậm > 3 s là boot đỏ.
  Đây là **lỗi thật của client DNS**, không phải T-INFRA ⇒ lead mở **KT-294** (retransmit 2 lần,
  **giữ nguyên** timeout 3 s mỗi lần — không nới ngưỡng).
- Host DNS lúc đo lại **bình thường** (`dig example.com` 17 ms) ⇒ sự cố là **thoáng qua**, nhưng
  client không có cơ chế thử lại nên vẫn đỏ.

**Phân bố thời gian DNS** (1000 boot, `kt292-dns-elapsed.txt`):

| Chỉ số | Giá trị |
|---|---|
| `elapsed_ms` (cả 1000, gồm ca timeout) | min **19**, p50 **27**, p90 **42**, p99 **199**, max **3000** |
| `rtt_ms` (999 ca thành công) | min **19**, p50 **26**, p99 **189**, max **512** |
| TCP connect `rtt` (1000) | p50 **3**, p99 **22**, max **154** |
| input key `elapsed_ms` (999) | p50 **256**, p99 **584**, max **731** |

Chỉ **1/1000** boot chạm timeout; 999 boot còn lại DNS trả trong **≤ 512 ms** (p99 = 199 ms) ⇒
không phải suy giảm dần, mà là **một lần trượt** bị client không retry biến thành đỏ.

## 4. Kiểm độc lập trên 1.000 log (không chỉ dựa vào `check_boot_log`)

| Kiểm | Kết quả |
|---|---|
| log có `1..35` | **1000/1000** |
| log có đúng 35 dòng `ok N - ` | **999/1000** (boot 138: 27 dòng) |
| dòng `not ok` | **1** (chỉ boot 138) |
| dòng `PANIC` | **0** |
| `# conf image: build=194ad9604b0a18b0 text=101668` | **1000/1000** |
| `# viFastOs aarch64: boot ok` | **999/1000** (thiếu ở boot 138) |
| `# net: no tcp` | **0** |
| `# net: no dns` | **1** (boot 138) |
| `# net stats:` có `errors=0 timeouts=0 id_mismatch=0` | **1000/1000** |
| `# gpu stats:` có `errors=0 timeouts=0 id_mismatch=0` | **999/999** (boot 138 halt trước GPU) |
| `# input stats:` có `errors=0 timeouts=0 id_mismatch=0` | **999/999** |

## 5. Phát hiện

1. **DNS không retransmit — lỗi thật, đã báo lead ngay khi boot 138 đỏ** (đúng nhiệm vụ §4). Lead
   phân loại **không phải T-INFRA** và mở **KT-294**. Soak này **KHÔNG** được gọi là PASS.
2. **Boot dò danh tính (probe) in `not ok 32`** — probe dùng `qg_boot` (không mở HMP monitor, không
   `sendkey a`) nên TAP 32 `input: key` timeout 5 s; probe **chỉ** kiểm `# conf image:` (D2-80) nên
   vẫn đạt. Đây là hành vi **có sẵn từ KT-283**, không phải hồi quy; **không** tính vào 1000 boot
   soak. Ghi nhận để người sau không nhầm log probe là log soak.
3. **Console in `0 boot/s`** khi `wall > N` (`1000/1052` chia nguyên) — lỗi hiển thị của script,
   không ảnh hưởng phép đo; **không** sửa (ngoài phạm vi, sẽ đổi fingerprint).
4. **Wall 1052 s** so với **972 s** của KT-285 (ảnh TAP 32, chưa TCP/DNS): +80 s cho 1000 boot
   (≈ +80 ms/boot) — hợp lý với một kết nối TCP + một truy vấn DNS mỗi boot. Máy dùng chung
   (loadavg 1 m đỉnh **11,48** trong lúc soak, có QEMU của sub-agent khác chạy trước/sau) nên
   **không** dùng wall để kết luận hiệu năng (D2-116).
5. **Không** thấy dấu hiệu race/ô nhiễm giữa các boot: 999 boot xanh liên tục, `net/gpu/input stats`
   đều `errors=0 timeouts=0 id_mismatch=0`, không PANIC, không boot nào chạm 20 s.

## 6. Tệp đã đổi

| Tệp | Việc |
|---|---|
| `v2/agent/JOURNAL/kt292-aarch64-soak4.md` | **mới** — journal này |
| `v2/agent/JOURNAL/kt292-soak-1000.log` | **mới** — toàn bộ console soak (header + 1000 dòng `boot i/1000` + tổng) |
| `v2/agent/JOURNAL/kt292-soak-results.log` | **mới** — `results.txt`: 999 dòng `ok i`, **1** dòng `fail 138` |
| `v2/agent/JOURNAL/kt292-guard-summary.log` | **mới** — `boot_end=1001 isolated_1=1001 rc_0=1001 rc_124=0` |
| `v2/agent/JOURNAL/kt292-infra.log` | **mới** — mẫu `qg_infra` trước/sau (cả hai `verdict=OK`) |
| `v2/agent/JOURNAL/kt292-boot-138.log` | **mới** — log boot đỏ (DNS timeout) |
| `v2/agent/JOURNAL/kt292-boot-1.log`, `kt292-boot-1000.log` | **mới** — log boot đầu và boot cuối |
| `v2/agent/JOURNAL/kt292-selftest.log` | **mới** — `--selftest` 38/38 |
| `v2/agent/JOURNAL/kt292-dns-elapsed.txt` | **mới** — 1000 giá trị `elapsed_ms` của DNS (đã sắp xếp) |

**Không** sửa: `v2/agent/{STATE,QUEUE,DECISIONS}.md`, `v2/bin/v2`, `spec/`, `v2/kernel/**`,
`v2/test/boot/soak-aarch64.sh` (không đổi một dòng).

## 7. Cổng cuối

```text
$ ./v2/bin/v2 check
# step-time-total   138336 ms  TỔNG check
exit=0

$ bash v2/test/boot/run-aarch64.sh
# ca âm: 19/19 ĐỎ đúng như phải
# 22/22 kiểm sạch (boot=1, tests=35)
# infra: guard_log=… qemu_own=0
exit=0
```

## 8. Ràng buộc bằng chứng với commit

Soak chạy trên cây **sạch** (`dirty-kernel-paths=0`, `git HEAD=36e709ce`) — chỉ có các tệp
`v2/agent/JOURNAL/kt292-*` là thay đổi **ngoài** `v2/kernel`, nên **không** đổi đầu vào build ảnh.
Ảnh đã soak:

```text
# BUILD_ID (D2-80) = build=194ad9604b0a18b0 text=101668
sha256 = 8eedfd0ec3307697218028d63c8020a78bc93bfeb67a64776b238b243e78c68b
bytes  = 256768
data+bss = 233704 / 262144 (89,1 %)
```

Commit chứa **đúng** nội dung cây đã soak (journal + log). **Không** chạy lại soak sau commit
(cùng fingerprint — AGENTS §6.3 cấm chạy lại soak trên cùng fingerprint chỉ để tăng số lần PASS).

## 9. Còn thiếu / rủi ro

1. **Soak KHÔNG đạt 0/1000** — 1 boot đỏ vì DNS timeout. Cần **KT-294** (DNS retransmit) rồi soak
   lại 1.000 trên ảnh đã vá (lead sẽ làm sau khi merge KT-294).
2. **K4' chưa đạt**: soak là một điều kiện; K4' còn timer per-hart / preemption, mỗi hart một
   address space/ASID, đọc DTB, W^X đầy đủ (`k4-aarch64-boot.md` §7).
3. **Ảnh aarch64 chưa in `# build:` (KT-86)** — cổng danh tính vẫn là `# conf image:` + sha256 +
   `git HEAD` (nợ P20: sha ảnh phụ thuộc thư mục build).
4. **Probe boot là boot đỏ không tính** (TAP 32) — xem §5.2; nếu sau này muốn probe cũng kiểm đủ
   bất biến thì phải cho probe dùng `boot_keys` (thay đổi script, ngoài phạm vi task này).
5. **Máy dùng chung**: trong lúc soak có QEMU của sub-agent khác (KT-291 riscv64 chạy trước, KT-293
   x86_64 chạy sau); hai mẫu `qg_infra` của soak đều `qemu_foreign=0` nên phép đo **HỢP LỆ**, nhưng
   wall **không** dùng để so hiệu năng.
