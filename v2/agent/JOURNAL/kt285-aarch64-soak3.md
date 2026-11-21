# KT-285 — soak aarch64 **1.000 trên ảnh hiện tại** (GPU + console + virtio-input, TAP 32)

Trạng thái: **DONE** (nhánh `task/kt285-a64soak3`, base `main` @ `3b924e7f`), 2026-11-21.
Sub-agent `a64soak3`. Quyết định: **D2-204** (KT-265 — bất biến soak), **D2-210** (KT-279 — GPU),
**D2-211** (KT-283 — bàn phím), **D2-24 / D2-72 Q10 / D2-74 / D2-80** (bất biến soak), **D2-07**
(không retry), **D2-116** (QEMU lạ ⇒ số thời gian không dùng để kết luận), **D2-198** (uỷ quyền
commit trên nhánh task).

**Không** tuyên bố K4' đạt. Task này chỉ **đo lại** soak 1.000 trên ảnh **đã đổi** ở vòng 10–11
(KT-279 thêm virtio-gpu 2D + console, KT-283 thêm virtio-input, TAP 30 → 32) — bằng chứng KT-272
(ảnh TAP 18) **không còn áp dụng** (AGENTS §6.3).

## 1. Phạm vi

| Việc | Kết quả |
|---|---|
| `soak-aarch64.sh`: kỳ vọng TAP **32** + GPU + bàn phím | **ĐÃ CÓ SẴN** — KT-279/KT-283 đã cập nhật (`TAP_TESTS=32`, `-device virtio-gpu-pci`, `-device virtio-keyboard-pci`, `sendkey a`); task này **không** phải sửa kỳ vọng |
| `soak-aarch64.sh`: **thêm** dòng `# boot_end` cho mỗi boot soak | **DONE** — KT-283 thêm `boot_keys` nhưng bỏ `qg_guard_note boot_end` ⇒ `guard-summary.log` chỉ đếm 1 boot probe; chỉ **thêm** log, **không** đổi bất biến/ngưỡng |
| `--selftest` | **DONE** — **36/36** (1 dương + 35 âm), không xoá ca cũ nào |
| **Soak thật 1.000** trên HEAD `3b924e7f` | **DONE** — **0/1000 lỗi**, wall **972 s**, hạ tầng **HỢP LỆ** (`invalid=0`) |

**Chỉ thêm kiểm, không bỏ kiểm cũ** (đúng nhiệm vụ): 35 ca âm cũ giữ nguyên. Không ngưỡng nào bị
nới: N=1.000, 3 luồng, 20 s/boot, 0 lỗi/N, không retry.

## 2. Vì sao không phải sửa kỳ vọng TAP

KT-279 (`36625285`) và KT-283 (`fa07df31`) đã cập nhật **cả hai** nơi hard-code số ca:
`run-aarch64.sh` (`EXPECT_TESTS=32`) và `soak-aarch64.sh` (`TAP_TESTS=32`), kèm bất biến GPU
(10 dòng `# gpu …` + 2 marker) và bàn phím (8 dòng `# input …`). Vì vậy task này **không** đổi
kỳ vọng nào; chỉ vá **một** lỗ hổng bằng chứng (mục 3).

## 3. Vá bằng chứng: `# boot_end` cho mỗi boot soak (KT-283 bỏ sót)

`boot_keys` (KT-283) gọi `qg_start` trực tiếp rồi tự chờ, nên **không** gọi `qg_guard_note boot_end`
như `qg_boot`. Hệ quả đo được ở lần chạy đầu: `qemu-guard.log` có **1001** dòng `# boot_start`
nhưng chỉ **1** dòng `# boot_end` (của boot probe) ⇒ `guard-summary.log` in
`boot_end=1 isolated_1=1 rc_0=1 rc_124=0`, **không** có bằng chứng `rc_124=0` cho 1.000 boot soak.

Vá: thêm **đúng một** lời gọi `qg_guard_note boot_end` ở **cả hai** nhánh (quá hạn 124 và thoát
bình thường) — 6 dòng, **không** xoá dòng nào, **không** đổi `TIMEOUT_DS`/`READY_DS`/`TAP_TESTS`.
Sau vá, `--selftest` vẫn **36/36**; smoke 3 boot cho `boot_end=4 isolated_1=4 rc_0=4 rc_124=0`;
soak 1.000 cho `boot_end=1001 isolated_1=1001 rc_0=1001 rc_124=0`.

## 4. Soak thật — **0/1000**, hạ tầng HỢP LỆ

```text
$ bash v2/test/boot/soak-aarch64.sh 1000 3
# qemu 11.1.3 (sàn 9.2, D2-72 Q2)
# cấu hình: -M virt,gic-version=3 -cpu cortex-a72 -m 128M -nographic -smp 4 -no-reboot; KHÔNG icount (D2-74 c)
# N=1000 lanes=3 timeout=20s/boot; p=3/1000 (0,3 %) ⇒ n ≥ 3/p = 1000 (D2-24)
# git HEAD=3b924e7fd743c4220c8bb4533ab21f7fbad92ee7 dirty-kernel-paths=0
# infra before: qemu_total=0 qemu_ppid1=0 qemu_own=0 qemu_foreign=0 loadavg_1m=4.81 ... verdict=OK
# image=product bytes=241744 sha256=b9f98b44026f3fa103720374182098411b2afe90b0315135319fd5f604f8fd02
# BUILD_ID (D2-80) = build=f451e5c084e7858e text=89092
# probe: '# conf image:' khớp ELF (build=f451e5c084e7858e text=89092)
# soak: bắt đầu N=1000 lanes=3 05:54:02Z
...
# infra after: qemu_total=0 qemu_ppid1=0 qemu_own=0 qemu_foreign=0 loadavg_1m=9.80 ... verdict=OK
# infra: invalid=0 isolation_bad=0
# soak: xong wall=972s (1 boot/s); ok=1000 fail=0 tổng=1000/1000
# soak: BUILD_ID=f451e5c084e7858e sha256=b9f98b44... git=3b924e7f...
# soak: 1000/1000 lỗi=0 — 0/N chỉ là trần (QD §5.3); không retry (D2-07)
# soak: 0/1000 lỗi (PASS)
# soak-aarch64.sh: PASS
exit=0
```

| Phép đo | Kết quả |
|---|---|
| N / luồng / timeout | **1.000** / **3** / **20 s** mỗi boot (không nới) |
| boot đỏ | **0** — `results.txt`: 1000 dòng `ok`, **0** dòng `fail` |
| wall | **972 s** (≈ 1,03 boot/s) — **HỢP LỆ**: `qemu_foreign=0` ở **cả hai** mẫu, `invalid=0` |
| guard (`qemu_guard.sh`) | `boot_end=1001 isolated_1=1001 isolated_0=0 rc_0=1001 rc_124=0` (1000 soak + 1 probe) |
| `rc_124` (quá 20 s) | **0** — không boot nào chạm timeout |
| `qemu_own` | **0** sau khi xong — không QEMU mồ côi |
| `qemu_foreign` | **0** trước **và** sau |
| ảnh đổi giữa soak | **không** (sha256 đầu == cuối) |
| hash ảnh khớp ELF | **1000/1000** boot + probe (D2-80) |
| retry | **0** (D2-07) |

**Kiểm độc lập trên 1.000 log** (không chỉ dựa vào `check_boot_log`):

| Kiểm | Kết quả |
|---|---|
| log có `1..32` | **1000/1000** |
| log có đúng 32 dòng `ok N - ` | **1000/1000** |
| dòng `not ok` | **0** |
| dòng `PANIC` | **0** |
| `# conf image: build=f451e5c084e7858e text=89092` | **1000/1000** |
| `# gpu: drawn` | **1000/1000** |
| `# input: key a code=30 ascii=0x61 shift=0` | **1000/1000** |

## 5. Hai lần chạy trước (ghi trung thực, D2-116)

Máy dùng chung với sub-agent khác (D2-198: tối đa 4 sub-agent). Hai lần chạy đầu **cũng 0/1000**
nhưng mẫu hạ tầng **KHÔNG HỢP LỆ** vì QEMU lạ của sub-agent khác (x86_64, `/tmp/kt286-manual/boot.sh`):

| Lần | Kết quả | wall | `invalid` | Lý do |
|---|---|---:|---:|---|
| 1 | 0/1000 | 1042 s | **2** | `before qemu_foreign=2`, `after qemu_foreign=1` |
| 2 | 0/1000 | 790 s | **1** | `before qemu_foreign=1`, `after` OK |
| **3 (chính)** | **0/1000** | **972 s** | **0** | `before`/`after` đều `qemu_foreign=0` |

Theo **D2-116**, số **thời gian** của lần 1–2 **không** dùng để kết luận; số **0/1000** vẫn là
bằng chứng đúng (mỗi boot là một QEMU riêng, ảnh không đổi). Lần 3 là phép đo **hợp lệ** và là
bằng chứng chính. Không lần nào retry boot đỏ (D2-07).

## 6. Log một boot (mẫu `kt285-boot-1.log`)

```text
1..32
# conf image: build=f451e5c084e7858e text=89092
...
# smp: online mask=0xf alive=0xf expect=0xf harts=4 mpidr_ok=1 mpidr=0x80000000,...
# umode: el0 ran svc=0 ret=12
...
ok 28 - gpu: device
# gpu: drawn
ok 29 - gpu: draw
ok 30 - gpu: console
# gpu: console done lines=3
ok 31 - input: device
# input: ready
# input: key a code=30 ascii=0x61 shift=0 elapsed_ms=307
ok 32 - input: key
# viFastOs aarch64: boot ok
# viFastOs aarch64: halt
```

## 7. Phát hiện

**Không có lỗi mới.** GPU (virtio-gpu 2D + console) và bàn phím (virtio-input, `sendkey a`) **ổn
định dưới soak 1.000**: 1.000/1.000 boot có `# gpu: drawn`, `# gpu: console done lines=3`,
`# input: key a code=30 ascii=0x61 shift=0`, TAP 28–32 xanh, không PANIC, không boot nào chạm 20 s.
Không thấy dấu hiệu race/ô nhiễm giữa các boot.

Hai điểm **quan sát** (không phải lỗi ảnh):

1. **Chi phí GPU + bàn phím (đo được)**: wall **972 s** so với **239 s** của KT-272 (ảnh TAP 18,
   không GPU/input) ⇒ **+733 s** (~12,2 phút) cho 1.000 boot. Trong đó: giữ khung GPU
   `GPU_HOLD_MS=500` ms/boot (KT-279 dự tính +2,8 phút) + chờ phím của khách
   (`elapsed_ms` trung bình **274 ms**, tối đa **2.627 ms**) + khởi tạo GPU/input mỗi boot.
   **Không** bỏ GPU/input để nhanh hơn (sẽ phải nới bất biến TAP — không được phép).
2. **`boot_keys` (KT-283) chưa tính 5 s chờ `# input: ready` vào ngân sách 20 s**: vòng chờ
   marker dùng `READY_DS=100` bước × 0,05 s = **5 s** riêng, rồi vòng chờ thoát mới đếm
   `waited` tới `TIMEOUT_DS=200` (20 s) ⇒ boot treo **trước** `# input: ready` bị kill ở
   **~25 s**, không phải 20 s. **Không** ảnh hưởng phép đo này (không boot nào gần 20 s: max
   `elapsed_ms` 2.627 ms; tổng thời gian boot ~2,9 s/boot theo wall), nhưng là **nới ngưỡng
   20 s/boot** so với bất biến D2-72 Q10 ⇒ **đề nghị lead** mở task vá riêng (trừ thời gian chờ
   marker vào ngân sách, hoặc đặt `t0` trước vòng chờ marker). Task này **không** sửa (ngoài
   phạm vi; sửa sẽ đổi fingerprint bằng chứng đã đo).

## 8. Tệp đã đổi

| Tệp | Việc |
|---|---|
| `v2/test/boot/soak-aarch64.sh` | **+6 dòng**: `qg_guard_note boot_end` ở hai nhánh của `boot_keys` (bằng chứng `rc_124`/`isolated` cho 1.000 boot). Không đổi bất biến/ngưỡng |
| `v2/agent/JOURNAL/kt285-aarch64-soak3.md` | **mới** — journal này |
| `v2/agent/JOURNAL/kt285-soak-1000.log` | **mới** — toàn bộ console soak lần 3 (header + 1000 dòng `boot i/1000 ok` + tổng) |
| `v2/agent/JOURNAL/kt285-soak-results.log` | **mới** — `results.txt`: 1000 dòng `ok i`, **0** `fail` |
| `v2/agent/JOURNAL/kt285-guard-summary.log` | **mới** — `boot_end=1001 isolated_1=1001 rc_0=1001 rc_124=0` |
| `v2/agent/JOURNAL/kt285-infra.log` | **mới** — mẫu `qg_infra` lần 3 (cả hai `verdict=OK`) |
| `v2/agent/JOURNAL/kt285-boot-1.log`, `kt285-boot-1000.log` | **mới** — log một boot đầu và boot cuối |
| `v2/agent/JOURNAL/kt285-selftest.log` | **mới** — `--selftest` 36/36 |

**Không** sửa: `v2/agent/{STATE,QUEUE,DECISIONS}.md`, `v2/bin/v2`, `spec/`, `v2/kernel/**`.

## 9. Cổng cuối

```text
$ ./v2/bin/v2 check
# step-time-total    85038 ms  TỔNG check
exit=0

$ bash v2/test/boot/run-aarch64.sh
ok 2 - boot 1: TAP 32/32 + marker 1 lần + hash ảnh khớp ELF
ok 3 - ppm: 23 kiểm PPM xanh (màu rect + 29/29 ký tự console)
ok 19 - ca âm (p): không có GPU ⇒ '# gpu: no device' + TAP ĐỎ
ok 20 - ca âm (q): không có bàn phím ⇒ '# input: no device' + TAP ĐỎ
# ca âm: 17/17 ĐỎ đúng như phải
# 20/20 kiểm sạch (boot=1, tests=32)
exit=0
```

## 10. Ràng buộc bằng chứng với commit

Soak chạy trên cây **sạch** (`dirty-kernel-paths=0`, `git HEAD=3b924e7f`) — chỉ có
`v2/test/boot/soak-aarch64.sh` + các tệp `v2/agent/JOURNAL/kt285-*` là thay đổi **ngoài**
`v2/kernel`, nên **không** đổi đầu vào build ảnh. Ảnh đã soak:

```text
# BUILD_ID (D2-80) = build=f451e5c084e7858e text=89092
sha256 = b9f98b44026f3fa103720374182098411b2afe90b0315135319fd5f604f8fd02
bytes  = 241744
```

Commit chứa **đúng** nội dung cây đã soak (script + journal). **Không** chạy lại soak sau commit
(cùng fingerprint — AGENTS §6.3 cấm chạy lại soak trên cùng fingerprint chỉ để tăng số lần PASS).

## 11. Còn thiếu / rủi ro

1. **K4' chưa đạt**: soak 1.000 **đã xong** (lần 3, ảnh GPU + input), nhưng K4' còn timer per-hart /
   preemption, mỗi hart một address space/ASID, đọc DTB, W^X đầy đủ (`k4-aarch64-boot.md` §7).
2. **Ảnh aarch64 chưa in `# build:` (KT-86)** — cổng danh tính vẫn là `# conf image:` + sha256 +
   `git HEAD` (nợ P20: sha ảnh phụ thuộc thư mục build).
3. **`boot_keys` chưa tính 5 s chờ marker vào ngân sách 20 s** — xem §7.2; đề nghị task vá riêng.
4. **Máy dùng chung**: lần 1–2 có QEMU lạ của sub-agent khác ⇒ `invalid=2`/`1` (D2-116); lần 3 sạch.
   Wall 972 s **hợp lệ** cho lần 3, nhưng **không** nên so trực tiếp với 239 s của KT-272 (ảnh khác,
   có GPU/input).
5. **D2-212 không có trong `v2/agent/DECISIONS.md`** (tệp dừng ở D2-211) — nhiệm vụ có nhắc đọc
   D2-212; đã đọc D2-204 (KT-265 — bất biến soak) và các D2 liên quan. Nếu D2-212 được thêm sau,
   cần đối chiếu lại.
