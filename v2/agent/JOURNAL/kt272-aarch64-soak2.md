# KT-272 — soak aarch64 **1.000 sau U-mode** (KT-269)

Trạng thái: **DONE** (nhánh `task/kt272-a64soak2`, base `main` @ `75e25462`), 2026-11-21.
Sub-agent `a64soak2`. Quyết định: **D2-204** (KT-265 — bất biến soak), **D2-206** (KT-269 — U-mode),
**D2-24 / D2-72 Q10 / D2-74 / D2-80** (bất biến soak), **D2-07** (không retry), **D2-116** (QEMU lạ ⇒
số thời gian không dùng để kết luận), **D2-198** (uỷ quyền commit trên nhánh task).

**Không** tuyên bố K4' đạt. Task này chỉ **đo lại** soak 1.000 trên ảnh **đã đổi** ở KT-269
(thêm U-mode EL0 + `[calling.aarch64]`, TAP 15 → 18) và **thêm** kiểm U-mode vào bất biến soak.

## 1. Phạm vi

| Việc | Kết quả |
|---|---|
| `soak-aarch64.sh`: kỳ vọng TAP **15 → 18** (ảnh KT-269) | **DONE** — chỉ đổi hằng số kỳ vọng, **không** nới ngưỡng nào khác |
| `soak-aarch64.sh`: **thêm** bất biến `# umode:` mỗi boot | **DONE** — đúng 1 dòng `# umode: el0 ran svc=<n> ret=<v>`, `svc=0`, `ret>0` (cùng bộ kiểm với `run-aarch64.sh`) |
| `--selftest`: log tổng hợp 18 TAP + dòng `# umode:` + **3 ca âm mới** | **DONE** — **17/17** (1 dương + **16** âm), không xoá ca cũ nào |
| **Soak thật 1.000** trên HEAD `75e25462` | **DONE** — **0/1000 lỗi**, wall **239 s**, hạ tầng **HỢP LỆ** |

**Chỉ thêm kiểm, không bỏ kiểm cũ** (đúng nhiệm vụ): 13 ca âm cũ giữ nguyên, thêm 3 ca âm U-mode
(thiếu dòng `# umode:`, `svc=1` sai imm16, `ret=0`). Không ngưỡng nào bị nới: N=1.000, 3 luồng,
20 s/boot, 0 lỗi/N, không retry.

## 2. Vì sao phải sửa kỳ vọng TAP (không phải nới ngưỡng)

KT-269 (`d4f6553f`) thêm 3 ca U-mode vào ảnh: `TAP_TESTS` của kernel = **18**, `EXPECT_TESTS` của
`run-aarch64.sh` = **18**, ảnh in `1..18` + `# umode: el0 ran svc=0 ret=12`. `soak-aarch64.sh` còn
`TAP_TESTS=15` từ KT-265 ⇒ nếu **không** cập nhật, **mọi** boot sẽ đỏ vì "có 18 dòng 'ok', cần 15"
— đỏ **sai** (bộ đọc lệch ảnh), không phải ảnh hỏng. Vì vậy kỳ vọng được cập nhật **theo ảnh**,
đồng thời **thêm** kiểm U-mode để ảnh chỉ chạy EL1 (không `eret`) vẫn **ĐỎ** dù TAP 1..18 xanh.

## 3. Soak thật — **0/1000**, hạ tầng HỢP LỆ

```text
$ bash v2/test/boot/soak-aarch64.sh 1000 3
# qemu 11.1.3 (sàn 9.2, D2-72 Q2)
# cấu hình: -M virt,gic-version=3 -cpu cortex-a72 -m 128M -nographic -smp 4 -no-reboot; KHÔNG icount (D2-74 c)
# N=1000 lanes=3 timeout=20s/boot; p=3/1000 (0,3 %) ⇒ n ≥ 3/p = 1000 (D2-24)
# git HEAD=75e25462f1f481bfb8ceb64423c7f335d737f399 dirty-kernel-paths=0
# infra before: qemu_total=0 qemu_ppid1=0 qemu_own=0 qemu_foreign=0 loadavg_1m=2.39 ... verdict=OK
# image=product bytes=158720 sha256=32f55aae7859e72adb53259a15b7eed0cda471cd7b1a01ee94602b311680fc8d
# BUILD_ID (D2-80) = build=c5d78e81f771359b text=40140
# probe: '# conf image:' khớp ELF (build=c5d78e81f771359b text=40140)
# soak: xong wall=239s (4 boot/s); ok=1000 fail=0 tổng=1000/1000
# infra after: qemu_total=0 qemu_ppid1=0 qemu_own=0 qemu_foreign=0 loadavg_1m=5.55 ... verdict=OK
# infra: invalid=0 isolation_bad=0
# soak: 0/1000 lỗi (PASS)
# soak-aarch64.sh: PASS
exit=0
```

| Phép đo | Kết quả |
|---|---|
| N / luồng / timeout | **1.000** / **3** / **20 s** mỗi boot (không nới) |
| boot đỏ | **0** — `results.txt`: 1000 dòng `ok`, **0** dòng `fail` |
| wall | **239 s** (≈ 4,2 boot/s) — **HỢP LỆ**: `qemu_foreign=0` ở **cả hai** mẫu, `invalid=0` |
| guard (`qemu_guard.sh`) | `boot_end=1001 isolated_1=1001 isolated_0=0 rc_0=1001 rc_124=0` (1000 soak + 1 probe) |
| `rc_124` (quá 20 s) | **0** — không boot nào chạm timeout |
| `qemu_own` | **0** sau khi xong — không QEMU mồ côi |
| `qemu_foreign` | **0** trước **và** sau — không QEMU lạ (khác KT-265) |
| ảnh đổi giữa soak | **không** (sha256 đầu == cuối) |
| hash ảnh khớp ELF | **1000/1000** boot + probe (D2-80) |
| retry | **0** (D2-07) |

**Hạ tầng lần này HỢP LỆ** (khác KT-265): trước khi bắt đầu đã kiểm `ps -ax | grep qemu` ⇒ rỗng;
mẫu `before`/`after` của soak đều `qemu_total=0 qemu_ppid1=0 qemu_own=0 qemu_foreign=0 verdict=OK`
⇒ `invalid=0`. Vì vậy **số thời gian** (239 s) **dùng được** để kết luận (D2-116 không kích hoạt).
Ghi chú trung thực: lúc **09:31** có một `qemu-system-riscv64` của sub-agent khác (worktree
`/private/tmp/kt270-s2`, KT-263 GPU) sống ~20 s; nó **đã tắt trước** mẫu `before` của soak
(09:32:45) nên **không** xuất hiện ở mẫu nào của lần đo này. Dòng `qemu_foreign=1` trong
`kt272-infra.log` (dòng 5) là **lịch sử** của lần KT-265 (06:58–07:07, QEMU x86_64 của KT-266 —
đúng journal KT-265 §3), **không** phải lần này; dòng 6–7 là smoke 3 boot, dòng 8–9 là soak này.

Bất biến **mỗi** boot (fail-closed, `check_boot_log`) — **giữ nguyên 6 mục cũ, thêm mục 5**:

1. đúng 1 `TAP version 13`, đúng 1 `1..18`, đúng **18** dòng `ok N - ` (đủ số 1..18), **0** `not ok`;
2. đúng 1 `# viFastOs aarch64: boot ok`;
3. đúng 1 `# conf image: build=<16 hex> text=<len>` **khớp ELF** (D2-80);
4. `# smp: online mask=0xf alive=0xf expect=0xf harts=4 mpidr_ok=1` + `# smp: hart=N online|alive`
   cho cả bốn hart + `# smp: ipi recv=1` đúng một lần + ba dòng `# tlbi:` (read1 `match=1`,
   shootdown `acks=3 ok=1`, read2 `fault=1`);
5. **mới (KT-269)** — đúng **một** `# umode: el0 ran svc=<n> ret=<v>` với `svc=0`, `ret>0`;
6. **0** dòng `PANIC` (tính cả dòng thụt đầu dòng);
7. quá 20 s ⇒ `kill` **cả process group**, boot đó đỏ, **không retry**.

`--selftest` (không mở QEMU): **17/17** — 1 ca dương + **16** ca âm (13 cũ + 3 mới), tất cả đúng
như phải ⇒ bộ đọc **không** phải dấu cao su.

## 4. Log một boot (mẫu `kt272-boot-1.log`)

```text
1..18
# conf image: build=c5d78e81f771359b text=40140
...
ok 15 - smp: 4 harts online
# umode: eret entry=0x70000000 sp=0x70002000 spsr=0x3c0 mode=EL0t code_va=0x70000000 code_pa=0x4008b000 code_bytes=52 stack_va=0x70001000 stack_pa=0x40097000
ok 16 - umode: el0 entered
el0 says hi
ok 17 - umode: svc handled
# umode: el0 ran svc=0 ret=12
# umode: svc nr=12 imm=0 elr=0x70000010 ptr=0x70000028 len=12 printed=12 error=0 ret=12
ok 18 - umode: returned to el0
# umode: exit code=0 nr=14 imm=0
# umode: done
# viFastOs aarch64: boot ok
# viFastOs aarch64: halt
```

## 5. Phát hiện mới

**Không có lỗi mới.** U-mode (EL0 + `svc` + `eret` về EL1h) **ổn định dưới soak 1.000**: 1.000/1.000
boot có đúng một `# umode: el0 ran svc=0 ret=12`, TAP 16/17/18 xanh, `# umode: done`, marker in sau
cùng, không PANIC, không boot nào chạm 20 s. Không thấy dấu hiệu race/ô nhiễm giữa các boot (mỗi
boot là một QEMU riêng, ảnh không đổi).

Hai điểm **quan sát** (không phải lỗi):

1. **Kỳ vọng TAP lệch giữa ảnh và soak script** là **nợ quy trình** đã biết: KT-269 đổi ảnh nhưng
   không cập nhật `soak-aarch64.sh` (KT-265 không nằm trong diff của KT-269). Đã vá trong task này.
   Đề xuất cho lead: mọi task đổi số ca TAP phải grep `TAP_TESTS`/`EXPECT_TESTS` trong
   `v2/test/boot/` — hiện có **hai** nơi hard-code (`run-aarch64.sh`, `soak-aarch64.sh`).
2. `# umode: el0 ran svc=0 ret=12` là **một** dòng; các dòng `# umode: eret/svc/exit/done` khác
   cũng xuất hiện đúng một lần nhưng task này **chỉ** đưa dòng `el0 ran` vào bất biến (đúng phạm vi
   nhiệm vụ: "thêm dòng `# umode:`"), để tránh ràng buộc quá chặt vào định dạng nội bộ.

## 6. Tệp đã đổi

| Tệp | Việc |
|---|---|
| `v2/test/boot/soak-aarch64.sh` | `TAP_TESTS` 15 → 18; **thêm** kiểm `# umode:` trong `check_boot_log`; log tổng hợp 18 TAP + dòng `# umode:`; **thêm** 3 ca âm (n/o/p); cập nhật chú thích + `usage()` range |
| `v2/agent/JOURNAL/kt272-aarch64-soak2.md` | **mới** — journal này |
| `v2/agent/JOURNAL/kt272-soak-1000.log` | **mới** — toàn bộ console soak (header + 1000 dòng `boot i/1000 ok` + tổng) |
| `v2/agent/JOURNAL/kt272-soak-results.log` | **mới** — `results.txt`: 1000 dòng `ok i`, **0** `fail` |
| `v2/agent/JOURNAL/kt272-guard-summary.log` | **mới** — `boot_end=1001 isolated_1=1001 rc_0=1001 rc_124=0` |
| `v2/agent/JOURNAL/kt272-infra.log` | **mới** — mẫu `qg_infra` (dòng 8–9 = soak này, cả hai `verdict=OK`) |
| `v2/agent/JOURNAL/kt272-boot-1.log`, `kt272-boot-1000.log` | **mới** — log một boot đầu và boot cuối |
| `v2/agent/JOURNAL/kt272-selftest.log` | **mới** — `--selftest` 17/17 |

**Không** sửa: `v2/agent/{STATE,QUEUE,DECISIONS}.md`, `v2/bin/v2`, `spec/`, `v2/kernel/**`.

## 7. Cổng cuối

```text
$ ./v2/bin/v2 check
# step-time-total    85537 ms  TỔNG check
exit=0

$ bash v2/test/boot/run-aarch64.sh
ok 2 - boot 1: TAP 18/18 + marker 1 lần + hash ảnh khớp ELF
ok 13 - ca âm (k): thiếu dòng '# umode:' ⇒ ĐỎ
# ca âm: 11/11 ĐỎ đúng như phải
# 13/13 kiểm sạch (boot=1, tests=18)
exit=0
```

## 8. Ràng buộc bằng chứng với commit

Soak chạy trên cây **sạch** (`dirty-kernel-paths=0`, `git HEAD=75e25462`) — chỉ có
`v2/test/boot/soak-aarch64.sh` + các tệp `v2/agent/JOURNAL/kt272-*` là thay đổi **ngoài**
`v2/kernel`, nên **không** đổi đầu vào build ảnh. Ảnh đã soak:

```text
# BUILD_ID (D2-80) = build=c5d78e81f771359b text=40140
sha256 = 32f55aae7859e72adb53259a15b7eed0cda471cd7b1a01ee94602b311680fc8d
bytes  = 158720
```

Commit chứa **đúng** nội dung cây đã soak (script + journal). **Không** chạy lại soak sau commit
(cùng fingerprint — AGENTS §6.3 cấm chạy lại soak trên cùng fingerprint chỉ để tăng số lần PASS).

**Kiểm lại sau commit** (đúng invocation của script — đường dẫn **lô-gíc** `/tmp/...`):

```text
$ cargo build --manifest-path /tmp/kt270-s3/v2/kernel/Cargo.toml -p vifast-kbin-aarch64 \
      --target aarch64-unknown-none --release
$ python3.12 v2/test/conformance/image_digest.py .../vifast-kbin-aarch64
build=c5d78e81f771359b text=40140
$ shasum -a 256 .../vifast-kbin-aarch64
32f55aae7859e72adb53259a15b7eed0cda471cd7b1a01ee94602b311680fc8d
```

**Trùng byte** với ảnh đã soak ⇒ bằng chứng **0/1000** áp đúng cho commit này.

**Ghi chú P20 (đo được, không sửa trong task này):** build **cùng nguồn** nhưng gọi
`--manifest-path /private/tmp/kt270-s3/...` cho `build=7b77329364b3d7d1` / sha `3e0fe415…`, còn
`/tmp/kt270-s3/...` cho `build=c5d78e81f771359b` / sha `32f55aae…` — **cùng cỡ 158.720 B**, khác
chuỗi đường dẫn trong metadata cargo (đúng nợ P20 ở KT-265 §6). Vì vậy **không** so sha ảnh giữa
hai cách gọi; danh tính bền là `# conf image:` + `git HEAD` + invocation ghi trong log soak.

## 9. Còn thiếu / rủi ro

1. **K4' chưa đạt**: soak 1.000 **đã xong** (lần 2, sau U-mode), nhưng K4' còn timer per-hart /
   preemption, mỗi hart một address space/ASID, đọc DTB, W^X đầy đủ (`k4-aarch64-boot.md` §7).
2. **Ảnh aarch64 chưa in `# build:` (KT-86)** — cổng danh tính vẫn là `# conf image:` + sha256 +
   `git HEAD` (nợ P20: sha ảnh phụ thuộc thư mục build).
3. **Hai nơi hard-code số ca TAP** (`run-aarch64.sh`, `soak-aarch64.sh`) — xem §5.1; task này
   **không** gộp (ngoài phạm vi), chỉ đồng bộ giá trị.
4. **Soak phải chạy độc quyền** (AGENTS §6): lần này không có QEMU lạ ở mẫu nào của soak, nhưng
   máy vẫn có sub-agent khác chạy (loadavg 5,5 lúc kết thúc) — wall 239 s **hợp lệ** theo D2-116
   (không có `qemu_foreign`), chỉ là **không** nên so trực tiếp với 247 s của KT-265.
