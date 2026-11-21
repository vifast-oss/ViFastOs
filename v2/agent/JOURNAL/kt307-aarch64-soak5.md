# KT-307 — soak aarch64 **1.000 trên ảnh vòng 18** (KT-305, TAP 35) — **PASS 0/1000**

Trạng thái: **PASS — 0/1000** (nhánh `task/kt307-a64soak5`, base `main` @ `00c509d5`), 2026-11-21.
Sub-agent `a64soak5`. Quyết định: **D2-220** (vòng 19 — mở KT-307, đọc trên `main` @ `023b8d87`),
**D2-217** (DNS là bất biến TAP), **D2-218** (vòng 17 — đưa soak về 0/N), **D2-219** (vòng 18 —
KT-305), **D2-204** (KT-265 — bất biến soak), **D2-24 / D2-72 Q10 / D2-74 / D2-80** (bất biến
soak), **D2-07** (không retry), **D2-116** (hạ tầng), **D2-198** (uỷ quyền commit trên nhánh task).

**Không** tuyên bố K4' đạt. Soak là **một** điều kiện của K4' ("như K3": TAP đầy đủ smp 1|2|4 +
soak 1.000); K4' còn timer per-hart/preemption, mỗi hart một address space/ASID, đọc DTB, W^X đầy
đủ (`k4-aarch64-boot.md` §7).

**Ghi chú tài liệu**: worktree này base `main` @ `00c509d5` (đúng đề bài); trong lúc soak, `main`
tiến lên **`023b8d87`** — commit **chỉ tài liệu** (`DECISIONS.md` +2 dòng D2-220, `QUEUE.md` +3
dòng KT-307/308/309; `git show --stat` = 5 insertions, **không** chạm `v2/kernel/**`). D2-220 đã
đọc trên `main` và **khớp** nhiệm vụ này (soak aarch64 1.000 trên ảnh vòng 18, DNS cục bộ KT-300,
0 lỗi/N, `rc_124=0`, hash ảnh khớp ELF, không nới ngưỡng). Vì commit đó **không đổi đầu vào build
ảnh**, bằng chứng soak vẫn neo đúng theo `# conf image: build=a4acf01fc8fe8868` (xem §8).

## 1. Phạm vi

Vòng 18 đổi **ảnh aarch64** ở KT-305 (`translate`/`unmap` block descriptor; `.text` 103.676 →
**105.300 B**, +1.624 B) ⇒ bằng chứng soak vòng 17 (KT-300, ảnh `build=8809a6e159cb02b6`) **không
còn áp dụng** (AGENTS §6.3: khóa đầu vào đổi). Task này **chỉ đo lại** trên ảnh mới:

| Việc | Kết quả |
|---|---|
| `soak-aarch64.sh` (bất biến KT-265 + DNS cục bộ KT-300 + TCP KT-289 + GPU KT-279 + input KT-283 + block descriptor KT-305) | **KHÔNG sửa một dòng** — chạy nguyên trạng |
| `--selftest` | **40/40** (1 dương + 39 âm), gồm ca âm `blocks_split=0` của KT-305 |
| **Soak thật 1.000** trên HEAD `00c509d5`, cây sạch | **1000/1000** — **0 đỏ**; hạ tầng **HỢP LỆ** (`invalid=0`) |

Không nới ngưỡng nào: N=1.000, 3 luồng, 20 s/boot, 0 lỗi/N, không retry (D2-07). Không sửa
`v2/kernel/**`, `v2/test/boot/soak-aarch64.sh`, `run-aarch64.sh`, `v2/agent/{STATE,QUEUE,DECISIONS}.md`,
`v2/bin/v2`, `spec/`.

## 2. Kết quả soak

```text
$ SOAK_AARCH64_LOGDIR=/tmp/kt307/soak-1000 bash v2/test/boot/soak-aarch64.sh 1000 3
# soak-aarch64 KT-265 — 2026-11-21T13:58:43Z
# qemu 11.1.3 (sàn 9.2, D2-72 Q2)
# cấu hình: -M virt,gic-version=3 -cpu cortex-a72 -m 128M -nographic -smp 4 -no-reboot; KHÔNG icount (D2-74 c)
# N=1000 lanes=3 timeout=20s/boot; p=3/1000 (0,3 %) ⇒ n ≥ 3/p = 1000 (D2-24)
# git HEAD=00c509d53cefb3b097ef0d81caf2fc01ec707ea5 dirty-kernel-paths=0
# infra before: qemu_total=0 qemu_ppid1=0 qemu_own=0 qemu_foreign=0 loadavg_1m=3.11 loadavg_5m=3.53 loadavg_15m=4.42 ncpu=12 boot_done=0 verdict=OK
# image=product bytes=266280 sha256=c2e7317e89a9d25c85133e718e671649b1b66100329956fead08a3240e162de9
# BUILD_ID (D2-80) = build=a4acf01fc8fe8868 text=105300
# dns server: local=0.0.0.0:53 answer=192.0.2.123 ttl=3600 mutation=none dylib=…/kt300-dns-interpose.dylib
# probe: '# conf image:' khớp ELF (build=a4acf01fc8fe8868 text=105300)
# soak: bắt đầu N=1000 lanes=3 13:59:03Z
… (1000 dòng `boot i/1000 ok`)
# tcp server: connections=1001 sent13=1001 recv15=1001 (mỗi boot một kết nối)
# dns server: queries=1001 replies=1001 local_hits=1001 attempts1=1001 (cần ≥ 1001 truy vấn/trả lời và đúng 1001 local_hits)
# infra after: qemu_total=0 qemu_ppid1=0 qemu_own=0 qemu_foreign=0 loadavg_1m=7.16 loadavg_5m=7.59 loadavg_15m=7.46 ncpu=12 boot_done=1 verdict=OK
# infra: invalid=0 isolation_bad=0 guard_log=/tmp/kt307/soak-1000/qemu-guard.log
# soak: xong wall=1062s (0 boot/s); ok=1000 fail=0 tổng=1000/1000
# soak: BUILD_ID=a4acf01fc8fe8868 sha256=c2e7317e89a9d25c85133e718e671649b1b66100329956fead08a3240e162de9 git=00c509d53cefb3b097ef0d81caf2fc01ec707ea5
# soak: 1000/1000 lỗi=0 — 0/N chỉ là trần (QD §5.3); không retry (D2-07)
# soak: 0/1000 lỗi (PASS)
# soak-aarch64.sh: PASS
exit=0
```

| Phép đo | Kết quả |
|---|---|
| N / luồng / timeout | **1.000** / **3** / **20 s** mỗi boot (không nới) |
| boot đỏ | **0** — `results.txt`: 1000 dòng `ok`, **0** dòng `fail` |
| wall | **1062 s** (≈ 0,94 boot/s) — **HỢP LỆ**: `qemu_foreign=0` ở **cả hai** mẫu, `invalid=0` |
| guard (`qemu_guard.sh`) | `boot_end=1001 isolated_1=1001 isolated_0=0 rc_0=1001 rc_124=0` (1000 soak + 1 probe) |
| `rc_124` (quá 20 s) | **0** — không boot nào chạm timeout |
| `qemu_own` / `qemu_foreign` | **0** / **0** sau khi xong; **0** trước khi bắt đầu |
| ảnh đổi giữa soak | **không** (sha256 đầu == cuối, script tự so; `IMG_CHANGED=0`) |
| hash ảnh khớp ELF | **1000/1000** boot + probe (D2-80) |
| TCP server (bằng chứng hai chiều) | **1001/1001/1001** — `accept`/`sent=13`/`recv=15` (1000 soak + 1 probe) |
| DNS server cục bộ (KT-300) | **queries=1001 replies=1001** (probe + 1000 boot), **1001** câu trả lời `rcode=0 an=1` |
| DNS `local_hits` | **1001/1001** log boot có `# net: dns example.com a=192.0.2.123 ttl=3600` |
| DNS `attempts` | **1000/1000** = **1** (không retransmit) |
| retry | **0** (D2-07) |

## 3. Kiểm độc lập trên 1.000 log (không chỉ dựa vào `check_boot_log`)

Script `check_logs.py` (chạy ngoài repo, đọc **từng** log) đếm lại **mọi** bất biến của
`soak-aarch64.sh`; kết quả **1000/1000** cho **tất cả** các mục, **không** log nào lệch
(`kt307-independent-check.log`):

| Nhóm | Kiểm | Kết quả |
|---|---|---|
| TAP | đúng 1 `TAP version 13`, đúng 1 `1..35`, đúng **35** dòng `ok N - ` đủ 1..35, **0** `not ok` | **1000/1000** |
| Ảnh | `# conf image: build=a4acf01fc8fe8868 text=105300` đúng 1 (khớp ELF) | **1000/1000** |
| Marker | `# viFastOs aarch64: boot ok` + `# viFastOs aarch64: halt` đúng 1 | **1000/1000** |
| PANIC | **0** dòng `PANIC` (kể cả thụt đầu dòng) | **1000/1000** |
| **KT-305** | `# pci: unmap … level=2 translate_ok=1 split=1 neighbor_ok=1 unmapped_ok=1 remap_ok=1 blocks_split=1` đúng 1 | **1000/1000** |
| PCI | `# pci: ecam … buses=256`; `# pci: map … frames_used=33` | **1000/1000** |
| blk | `# blk: sector0 hex=5649464153544f532d4b543237332d53 magic=1 capacity=8 status=0x0f` | **1000/1000** |
| SMP | `# smp: online mask=0xf alive=0xf expect=0xf harts=4 mpidr_ok=1`; `# smp: ipi recv=1` | **1000/1000** |
| TLBI | 4 dòng `# tlbi:` (remap + read1 `match=1` + shootdown `acks=3 ok=1` + read2 `fault=1`) | **1000/1000** |
| U-mode | `# umode: el0 ran svc=0 ret=<n>`, `n > 0` | **1000/1000** |
| Mạng | device (vendor=0x1af4, assigned/readback), dhcp, arp, ping `n=3`, tcp connect/data, dns `attempts=(1\|2)` + `a=192.0.2.123`, `# net stats:` `errors=0 timeouts=0 id_mismatch=0` | **1000/1000** |
| W^X/DMA | `# wx: … text=RX data=RW ok=1 fault_ec=0x25`; `# dma: … ok=1 lines×line==bytes` | **1000/1000** |
| GPU | 10 dòng `# gpu …` + 2 marker; `fb identity=1 bytes=1228800 pages=300 pa=0x42000000 walk=0x42000000`; `scanout bogus=InvalidResourceId`; `console commands=2 matched=29/29`; `stats errors=0 timeouts=0 id_mismatch=0 rejects=1 fb_clean_lines>0` | **1000/1000** |
| Input | 8 dòng `# input …`; `pci vendor=0x1af4 device=0x1052 assigned=0x10600000`; `name QEMU Virtio Keyboard`; `dev version1=1 rep_feature=0 device_status=0x0f num_queues=2 queue_size=16 status_queue_size=16 key_a_bit=1`; `dma identity=1 armed=8 status_queue_idle=1`; `key a code=30 ascii=0x61 shift=0`; `stats errors=0 timeouts=0 id_mismatch=0 clean_lines>0 inval_lines>0` | **1000/1000** |

Log boot đầu và boot cuối (nguyên văn, phần KT-305 + DNS):

```text
# boot-1.log
# pci: unmap va=0x8fe01000 pa=0x401fe01000 unmap_pa=0x401fe01000 level=2 translate_ok=1 split=1 neighbor_ok=1 unmapped_ok=1 remap_ok=1 blocks_split=1
# net dns: name=example.com id=0x4b55 rcode=0 an=1 reply_len=45 rtt_ms=4 elapsed_ms=4 attempts=1 const:timeout_ms=3000
# net: dns example.com a=192.0.2.123 ttl=3600
ok 35 - net: dns

# boot-1000.log
# pci: unmap va=0x8fe01000 pa=0x401fe01000 unmap_pa=0x401fe01000 level=2 translate_ok=1 split=1 neighbor_ok=1 unmapped_ok=1 remap_ok=1 blocks_split=1
# net dns: name=example.com id=0x4b55 rcode=0 an=1 reply_len=45 rtt_ms=2 elapsed_ms=3 attempts=1 const:timeout_ms=3000
# net: dns example.com a=192.0.2.123 ttl=3600
ok 35 - net: dns
```

## 4. Phân bố thời gian (1000 boot)

| Chỉ số | min | p50 | p90 | p99 | max | mean |
|---|---:|---:|---:|---:|---:|---:|
| DNS `rtt_ms` | 0 | **3** | 7 | 12 | **69** | 4,0 |
| DNS `elapsed_ms` | 1 | 4 | 8 | 14 | **70** | 4,8 |
| input key `elapsed_ms` | 58 | 216 | 331 | 505 | 592 | 226,6 |
| TCP connect `rtt` | 0 | 3 | 7 | 15 | 286 | 4,4 |
| ping `rtt` | 0 | 0 | 3 | 9 | 254 | 1,4 |

So với KT-300 (cùng cơ chế DNS cục bộ, ảnh cũ): `rtt_ms` p50 **4 → 3**, p99 **24 → 12**, max
**272 → 69**; `elapsed_ms` max **2529 → 70**. Không boot nào chạm timeout 3 s/lần
(`attempts=1` 1000/1000). Khác biệt chủ yếu do tải máy (KT-300 đo lúc loadavg 13,8; lần này
3,11 → 7,16) — **không** dùng để kết luận hiệu năng (D2-116).

## 5. Phát hiện

1. **Ảnh vòng 18 (KT-305) soak 0/1000** — dòng `# pci: unmap … blocks_split=1` có mặt
   **1000/1000** boot ⇒ `translate`/`unmap` block descriptor chạy **dưới soak** trên bảng trang
   thật, không hồi quy, không PANIC, không boot nào chạm 20 s. Đây là bằng chứng soak **mới** cho
   ảnh `build=a4acf01fc8fe8868` (thay bằng chứng KT-300 `build=8809a6e159cb02b6`).
2. **Hạ tầng hợp lệ**: `qemu_foreign=0` **trước và sau**, `qemu_own=0`, `rc_124=0`,
   `invalid=0` (loadavg 3,11 → 7,16 < ncpu=12). Không có `MEASUREMENT_INVALID` (D2-116).
3. **`secs=` trong guard log của boot soak KHÔNG phải wall time boot** — `boot_keys` (KT-283) đặt
   `QG_LAST_SECS` sau vòng poll `# input: ready` (0,05 s × tối đa 10 s) + `sendkey`; QEMU thường
   đã thoát trước vòng chờ đuôi ⇒ 923 boot `secs=0.1`, 77 boot `secs=0.0`, chỉ probe (`qg_boot`)
   `secs=3.7`. Đây là **hành vi có sẵn từ KT-283/285**, không phải hồi quy; **không** dùng `secs`
   để kết luận hiệu năng. Bằng chứng timeout là `rc_124=0` + wall 1062 s. Không sửa (ngoài phạm
   vi, đổi fingerprint).
4. **DNS cục bộ (KT-300) vẫn hoạt động đúng**: 1001 truy vấn/1001 trả lời, `local_hits=1001`,
   `attempts=1` 1000/1000, không timeout. Interpose `DYLD` + server `0.0.0.0:53` không bị macOS
   chặn trong lần đo này.
5. **Probe boot là boot đỏ không tính** (TAP 32 `input: key` timeout vì probe dùng `qg_boot`,
   không mở HMP monitor/`sendkey`) — hành vi **có sẵn từ KT-283**; probe **chỉ** kiểm
   `# conf image:` (D2-80) nên vẫn đạt. Không tính vào 1000 boot soak.
6. **Console in `0 boot/s`** khi `wall > N` (chia nguyên) — lỗi hiển thị có sẵn của script, không
   ảnh hưởng phép đo; **không** sửa.
7. **Wall 1062 s** so với 1100 s của KT-300 (ảnh cũ) — chênh lệch nằm trong nhiễu tải máy dùng
   chung; **không** dùng để kết luận hiệu năng (D2-116).

## 6. Tệp đã đổi

| Tệp | Việc |
|---|---|
| `v2/agent/JOURNAL/kt307-aarch64-soak5.md` | **mới** — journal này |
| `v2/agent/JOURNAL/kt307-soak-1000.log` | **mới** — toàn bộ console soak (header + 1000 dòng `boot i/1000 ok` + tổng) |
| `v2/agent/JOURNAL/kt307-soak-results.log` | **mới** — `results.txt`: 1000 dòng `ok`, 0 dòng `fail` |
| `v2/agent/JOURNAL/kt307-guard-summary.log` | **mới** — `boot_end=1001 isolated_1=1001 rc_0=1001 rc_124=0` |
| `v2/agent/JOURNAL/kt307-infra.log` | **mới** — mẫu `qg_infra` trước/sau (cả hai `verdict=OK`) |
| `v2/agent/JOURNAL/kt307-boot-1.log`, `kt307-boot-1000.log` | **mới** — log boot đầu/cuối (có dòng KT-305 + DNS cục bộ) |
| `v2/agent/JOURNAL/kt307-dns-server.log` | **mới** — 1001 truy vấn + 1001 câu trả lời của server DNS cục bộ |
| `v2/agent/JOURNAL/kt307-dns-rtt.txt` | **mới** — 1000 giá trị `rtt_ms` của DNS (đã sắp xếp) |
| `v2/agent/JOURNAL/kt307-independent-check.log` | **mới** — kiểm độc lập 1000 log (mọi mục 1000/1000) |
| `v2/agent/JOURNAL/kt307-selftest.log` | **mới** — `--selftest` 40/40 |
| `v2/agent/JOURNAL/kt307-v2check.log` | **mới** — log `./v2/bin/v2 check` |

**Không** sửa: `v2/kernel/**`, `v2/test/boot/soak-aarch64.sh`, `v2/test/boot/run-aarch64.sh`,
`v2/agent/{STATE,QUEUE,DECISIONS}.md`, `v2/bin/v2`, `spec/`, `docs/`.

## 7. Cổng cuối

```text
$ ./v2/bin/v2 check
…
# step-time-total   138813 ms  TỔNG check
exit=0
```

Log đầy đủ: `kt307-v2check.log`.

## 8. Ràng buộc bằng chứng với commit

Soak chạy trên cây **sạch** (`dirty-kernel-paths=0`, `git HEAD=00c509d5`) — chỉ có các tệp
`v2/agent/JOURNAL/kt307-*` là thay đổi **ngoài** `v2/kernel`, nên **không** đổi đầu vào build ảnh.
Ảnh đã soak:

```text
# BUILD_ID (D2-80) = build=a4acf01fc8fe8868 text=105300
sha256 = c2e7317e89a9d25c85133e718e671649b1b66100329956fead08a3240e162de9
bytes  = 266280
data+bss = 233704 / 262144 (89,1 %)
```

Commit chứa **đúng** nội dung cây đã soak (journal + log). **Không** chạy lại soak sau commit
(cùng fingerprint — AGENTS §6.3 cấm chạy lại soak trên cùng fingerprint chỉ để tăng số lần PASS).

**`main` tiến lên trong lúc soak**: `00c509d5` → **`023b8d87`** (chỉ `DECISIONS.md` + `QUEUE.md`,
5 insertions — D2-220 + KT-307/308/309). Commit đó **không** chạm `v2/kernel/**` ⇒ đầu vào build
ảnh **không đổi**; danh tính ảnh đã soak vẫn là `# conf image: build=a4acf01fc8fe8868 text=105300`
+ sha256 `c2e7317e…`. Nhánh task **giữ base `00c509d5`** (không rebase/merge — lead merge).

## 9. Còn thiếu / rủi ro

1. **Không** tuyên bố K4' đạt — soak chỉ là một điều kiện; K4' còn timer per-hart/preemption, mỗi
   hart một address space/ASID, đọc DTB, W^X đầy đủ (`k4-aarch64-boot.md` §7).
2. **D2-220** đọc trên `main` @ `023b8d87` (vòng 19) — **khớp** nhiệm vụ; worktree base
   `00c509d5` chưa có mục này (chỉ tài liệu, không ảnh hưởng ảnh). Không sửa `DECISIONS.md`.
3. **Ảnh aarch64 chưa in `# build:` (KT-86)** — cổng danh tính vẫn là `# conf image:` + sha256 +
   `git HEAD` (nợ P20: sha ảnh phụ thuộc thư mục build).
4. **`secs=` của guard log không dùng được làm wall time boot** (mục 5.3) — nếu sau này cần phân
   bố thời gian boot thật thì phải mở task riêng (đổi `boot_keys`, đổi fingerprint).
5. **Máy dùng chung**: wall 1062 s không dùng để so hiệu năng; hai mẫu `qg_infra` đều
   `qemu_foreign=0` nên phép đo **HỢP LỆ**.
