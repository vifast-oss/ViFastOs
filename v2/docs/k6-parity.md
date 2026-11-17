<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# K6 — parity v1 ở mức người dùng (KT-77a): `spec/v1-parity.toml` + runner serial

- **Task:** KT-77a (Agent 2 dòng OS), thi hành **OD-K6-2** của Agent 0 (uỷ quyền **D2-121**).
- **Trạng thái:** **DONE phần KT-77a** (spec + runner + ca âm + log trong repo). **KT-77b** (shell
  trong ảnh sản phẩm) đang chạy ở tab khác; artifact lúc đó đã có chuỗi `# shell: up` (xem §5) nhưng
  **không** phải phép đo của KT-77a.
- **Không chạm:** ABI (`spec/abi.toml`, `spec/kernel.toml`), `v2/kernel/**` (KT-77b đang chạy),
  `v2/test/boot/**` (KT-78), `docs/v1/**`, `docs/v2/**`. `spec/v1-parity.toml` là tệp **mới** duy
  nhất trong `spec/` (D2-119).

## §1. Nguồn sự thật: `spec/v1-parity.toml`

**Hai mươi tám mục** = **hai mươi hai builtin** của shell v1 (bảng sinh tự động ở tag `v1-archive`,
mục SHELL-TABLE) + **sáu applet ngoài** `ps`, `free`, `df`, `uname`, `uptime`, `jobs` (ở v1 là chương
trình `/bin/<tên>`, `v1-archive:crates/vifast-bin/src/main.rs:221-239`; shell gọi qua fallback
exec-from-FS, `v1-archive:crates/vifast-client/src/main.rs:1180-1210`).
**Không** đưa vào: `args` — applet **debug** in argv (`v1-archive:crates/vifast-bin/src/main.rs:236`),
không phải lệnh người dùng; ghi ở đây để không bỏ im lặng (A0 có thể yêu cầu thêm).
Con số "hai mươi lăm/hai mươi sáu lệnh" trong `docs/v1/overnight-log.md:2982` là số builtin mà
generator đếm ở các vòng cũ, **không** phải trạng thái cuối của tag — vì vậy bằng chứng ở đây lấy
theo **tag**, không theo con số lịch sử.

| lệnh | đối số | `source` | trạng thái |
|---|---|---|---|
| `help` | — | `docs/v1/agent2-report-21.md:81`; `docs/v1/agent2-report-62.md:13` | **active** |
| `echo` | `<text>` | `docs/v1/agent2-report-21.md:81`; `docs/v1/agent2-report-62.md:23` | **active** |
| `ps` | — | `docs/v1/overnight-log.md:2633`; `v1-archive:crates/vifast-bin/src/main.rs:230` | **active** |
| `kill` | `<pid>` | `docs/v1/overnight-log.md:2887` | **active** |
| `cd` | `<path>` | `v1-archive:crates/vifast-client/src/main.rs:883`; `v1-archive:docs/11-hien-tai/16-van-hanh.md:109` | deferred-K7 + **cho A0** |
| `pwd` | — | `v1-archive:crates/vifast-client/src/main.rs:878`; `v1-archive:docs/11-hien-tai/16-van-hanh.md:121` | deferred-K7 + **cho A0** |
| `time` | — | `docs/v1/agent2-report-21.md:81` | deferred-K7 |
| `ls` | `[path]` | `docs/v1/agent2-report-21.md:81`; `docs/v1/overnight-log.md:3470` | deferred-K7 |
| `cat` | `<path>` | `docs/v1/agent2-report-21.md:81` | deferred-K7 |
| `mkdir` | `<path>` | `docs/v1/agent2-report-21.md:81` | deferred-K7 |
| `rm` | `<path>` | `docs/v1/agent2-report-21.md:81` | deferred-K7 |
| `mv` | `<src> <dst>` | `docs/v1/agent2-report-21.md:81` | deferred-K7 |
| `ln` | `<src> <dst>` | `docs/v1/agent2-report-21.md:82` | deferred-K7 |
| `stat` | `<path>` | `docs/v1/agent2-report-21.md:82`; `docs/v1/overnight-log.md:2576` | deferred-K7 |
| `sync` | — | `docs/v1/agent2-report-21.md:82` | deferred-K7 |
| `truncate` | `<path> <size>` | `docs/v1/agent2-report-21.md:82` | deferred-K7 |
| `fsck` | — | `docs/v1/agent2-report-21.md:82` | deferred-K7 |
| `free` | — | `docs/v1/overnight-log.md:2641`; `docs/v1/overnight-log.md:3469` | deferred-K7 |
| `df` | — | `docs/v1/overnight-log.md:2387`; `docs/v1/overnight-log.md:3469` | deferred-K7 |
| `uptime` | — | `docs/v1/overnight-log.md:2576`; `docs/v1/overnight-log.md:3470` | deferred-K7 |
| `uname` | — | `docs/v1/overnight-log.md:2424` | deferred-K7 |
| `run` | `<path>` | `docs/v1/overnight-log.md:1582` | deferred-K7 |
| `runfs` | `<path> [args...]` | `docs/v1/overnight-log.md:1598`; `docs/v1/overnight-log.md:1805` | deferred-K7 |
| `exec` | `<path> [args...]` | `docs/v1/overnight-log.md:1843` | deferred-K7 |
| `jobs` | — | `docs/v1/overnight-log.md:2903`; `v1-archive:crates/vifast-bin/src/main.rs:233` | deferred-K7 |
| `loop` | `<n>` | `docs/v1/overnight-log.md:3388` | deferred-K7 |
| `history` | — | `docs/v1/overnight-log.md:2776` | deferred-K7 |
| `!!` | — | `docs/v1/overnight-log.md:2776` | deferred-K7 |

**Sáu lệnh `active`** (`help`, `echo`, `ps`, `kill`, `ls`, `cat` — shell v2 đã có) ·
**hai mươi hai mục `deferred-K7`**. (Đính chính KT-122: bản trước ghi "bốn" và "hai mươi bốn" —
đúng là sáu và hai mươi hai, khớp `spec/v1-parity.toml` và bảng KT-109.)

**Lý do `deferred` đã sửa cho đúng hiện trạng (review KT-77a):**
- `time`, `uptime`: **không** phải "cần syscall đồng hồ" — `ClockRead` = syscall mười một đã có
  (`spec/abi.toml:117`, `v2/kernel/vifast-plat-riscv64/src/user.rs:1229`, TAP
  `v2/kernel/vifast-kbin-riscv64/src/ktest_user.rs:1105`). Thiếu chỉ là **builtin shell**.
- `jobs`: **không** phải "cần `TcbInfo`" — shell v2 **đã** nhận state từng TCB trong reply `PS_REQ`
  và tự in (`shell_user.rs`: `pid=entry>>5`, `state=(entry>>1)&0x0f`). Thiếu chỉ là builtin
  đếm Ready/Running. **Đính chính KT-122**: bản trước ghi `&0x1f` — đó là **lỗi thật** đã vá ở
  KT-105 (trạng thái chiếm **bốn** bit, không phải năm; `&0x1f` làm `ps` in `state=Unknown`).
- `cd`/`pwd`: **hai dữ kiện mâu thuẫn**, ghi cả hai chiều trong mục (`source` + `counter_evidence`)
  và `decision = 'cho Agent 0'`: mã/tài liệu ở tag `v1-archive` **có** cd/pwd
  (`v1-archive:crates/vifast-client/src/main.rs:878`) nhưng `docs/v1/overnight-log.md:3664-3669`
  ghi **đã revert** vì vượt ngân sách trang. KT-77a **không** tự chọn; lead ghi `BLOCKED-CHO-A0`.

Mỗi mục có **kịch bản serial**: `send`, `expect`, `timeout_ms`, `serial_min_count`,
`transcript_expect`. **Khác biệt newline:** v1 nhận `\r` (`docs/v1/agent2-report-62.md:24`), shell v2
nhận `\n` (`shell_user.rs:299`).

## §2. Runner: `v2/test/parity/run.sh`

```
bash v2/test/parity/run.sh [--mode transcript|serial] [--image ktest-proc|product|<path>]
                           [--negative wrong-expect|no-send] [--smp N] [--send-delay-ms N]
                           [--logdir DIR] [--log-suffix S]
```

- **`--mode transcript`** (mặc định): boot **một** lần, đối chiếu đầu ra shell với `expect` của từng
  lệnh `active`. Nguồn byte vào shell là **script nội bộ của kernel**
  (`v2/kernel/vifast-kbin-riscv64/src/proc_demo.rs:270-280`) ⇒ đo "shell có lệnh + in đúng dạng",
  **không** đo đường serial ngoài.
- **`--mode serial`**: mỗi lệnh `active` một lần boot; `stdin_wrap.sh` bơm byte vào **stdin QEMU** qua
  FIFO (bơm lặp trong khoảng bốn mươi đến một trăm chín mươi mili giây, vì một lần bơm có thể rơi
  lúc QEMU chưa vào vòng lặp chính ⇒ mất byte). Attribution theo `serial_min_count`: `help`/`ps` cần
  xuất hiện ít nhất hai lần (kernel cũng tự chạy một lần), `echo` (`K77A`) và `kill` với pid lạ dùng
  chuỗi đặc trưng.
- **Không sửa** `v2/test/boot/qemu_guard.sh` (KT-78): `source` tệp đó; mọi boot qua `qg_boot`
  (process group riêng, timeout cứng) + `qg_infra` trước/sau.
- **Nguồn gốc ảnh (D2-109/D2-122):** đầu mỗi lần chạy, runner in `image_path`, `image_sha256`,
  `image_bytes` và `artifacts_bin=artifacts/<sha>.bin co=…` (xem §3). Runner đọc ảnh từ
  `v2/kernel/target/.../release/`; **đóng băng `artifacts/<sha>.bin` thuộc KT-84**, không phải KT-77a.
- **Exit code:** khác nhau cho bốn trường hợp (đạt, lỗi cấu hình, có mục không khớp, timeout, không
  thấy shell) — số cụ thể nằm trong dòng `KET_LUAN` của log, không viết tay ở đây.
- TAP: `ok N - <lệnh>` / `not ok N - <lệnh>: <lý do>`; dòng tổng hợp
  `# parity: total=… ok=… deferred=… active=…` (số do runner sinh).

## §3. Kết quả đo (log trong repo; vòng đo cuối `-r2b-20261117T1305`)

**Ảnh đo** (in bởi runner, không phải viết tay) — trích từ `v2/agent/JOURNAL/kt77a-transcript-ktest-proc-smp1-r2b-20261117T1305.log:5`:

```text scope=v2/agent/JOURNAL/kt77a-transcript-ktest-proc-smp1-r2b-20261117T1305.log
# parity: image_path=/Users/mps/dvchd/ViFastOs/v2/kernel/target/riscv64gc-unknown-none-elf/release/vifast-kbin-ktest-proc image_sha256=f7c6b4a360d54e0edb04fd302e3d38a1e56680ae231cca73d2759388c281cfc1 image_bytes=437672
# parity: artifacts_bin=artifacts/f7c6b4a360d54e0edb04fd302e3d38a1e56680ae231cca73d2759388c281cfc1.bin co=0 (dong bang artifacts/<sha>.bin thuoc KT-84)
```

**Transcript trên ảnh test** (smp một hart): mọi lệnh `active` đạt — trích từ
`v2/agent/JOURNAL/kt77a-transcript-ktest-proc-smp1-r2b-20261117T1305.log:150`:

```text scope=v2/agent/JOURNAL/kt77a-transcript-ktest-proc-smp1-r2b-20261117T1305.log
ok 1 - help
ok 2 - echo
ok 3 - ps
ok 4 - kill
# parity: total=28 ok=4 deferred=24 active=4 (active_da_chay=4)
# parity: KET_LUAN=ok (exit 0)
```

**Serial trên ảnh test**: mọi lệnh `active` đạt; bộ đếm khớp `serial_min_count` — trích từ
`v2/agent/JOURNAL/kt77a-serial-ktest-proc-smp1-r2b-20261117T1305.log:153`:

```text scope=v2/agent/JOURNAL/kt77a-serial-ktest-proc-smp1-r2b-20261117T1305.log
ok 1 - help (khop=3 >= 2, waited_ms=0)
ok 2 - echo (khop=1 >= 1, waited_ms=0)
ok 3 - ps (khop=2 >= 2, waited_ms=0)
ok 4 - kill (khop=2 >= 1, waited_ms=0)
# parity: total=28 ok=4 deferred=24 active=4 (active_da_chay=4)
# parity: KET_LUAN=ok (exit 0)
```

**Độ ổn định (không giấu lần đỏ).** Serial là **đo chuyển động**: byte ngoài tranh cửa sổ RX với
script nội bộ kernel. Đã có lần **không đạt** một lệnh với bản wrapper **gửi một lần** — trích từ
`v2/agent/JOURNAL/kt77a-serial-ktest-proc-smp1-r2-20261117T1245.log:292`:

```text scope=v2/agent/JOURNAL/kt77a-serial-ktest-proc-smp1-r2-20261117T1245.log
not ok 2 - echo: timeout sau 2000 ms — expect khong xuat hien du 1 lan (^# shell: echo K77A$)
# parity: KET_LUAN=timeout (exit 4)
```

Sau khi đổi wrapper sang **bơm lặp**, lần đo cuối đạt tất cả (log `-r2b`). Đây **không** phải cổng
chặt — chỉ là phép đo trên ảnh test; cổng phải là ảnh sản phẩm có shell (KT-77b).

**Hạ tầng khi đo:** vòng `-r2b` có **QEMU của tab khác** (KT-77b) đang chạy nên `qg_infra` kết luận
phép đo **không hợp lệ** (dẫn nguyên văn ở khối dưới); vì vậy **không** dùng số **thời gian** của
vòng này để kết luận (D2-116/OD-K6-6). Kết quả TAP là nội dung log, không phải phép đo thời gian.

Hạ tầng của vòng đo (trích từ `v2/agent/JOURNAL/kt77a-transcript-ktest-proc-smp1-r2b-20261117T1305.log:7`):

```text scope=v2/agent/JOURNAL/kt77a-transcript-ktest-proc-smp1-r2b-20261117T1305.log
# infra before: qemu_total=1 qemu_ppid1=0 qemu_own=0 qemu_foreign=1 loadavg_1m=3.03 loadavg_5m=3.89 loadavg_15m=3.53 ncpu=12 boot_done=0 verdict=KHONG_HOP_LE ly_do=qemu_foreign=1
```

## §4. Ca âm (D2-108B) — ba ca, ba exit code khác nhau (log `-r2b`)

- **Gửi lệnh sai** (`zzz`) với `expect` không thể khớp ⇒ shell in `# shell: ?`, TAP `not ok`, có hạn
  thời gian (không treo) — trích từ `v2/agent/JOURNAL/kt77a-serial-ktest-proc-smp1-neg-wrong-expect-r2b-20261117T1305.log:151`:

```text scope-red=v2/agent/JOURNAL/kt77a-serial-ktest-proc-smp1-neg-wrong-expect-r2b-20261117T1305.log
not ok 1 - help: byte da vao shell (co 2 dong 'shell: ?') nhung expect khong khop sau 2000 ms (^# shell: echo KHONG-BAO-GIO$)
# parity: KET_LUAN=not_ok (exit 3)
```

- **Không gửi gì** ⇒ `expect` không xuất hiện đủ số lần ⇒ `timeout` — trích từ `v2/agent/JOURNAL/kt77a-serial-ktest-proc-smp1-neg-no-send-r2b-20261117T1305.log:149`:

```text scope-red=v2/agent/JOURNAL/kt77a-serial-ktest-proc-smp1-neg-no-send-r2b-20261117T1305.log
not ok 1 - help: timeout sau 2000 ms — expect khong xuat hien du 2 lan (^# shell: help cmds=[0-9]+$)
# parity: KET_LUAN=timeout (exit 4)
```

- **Shell không in gì** — ảnh sản phẩm **cũ trong `target`** tại thời điểm đo (`fd202276…`, chưa
  canonical, xem §5): **không** có dòng `# shell: up`, mọi mục `not ok` — trích từ `v2/agent/JOURNAL/kt77a-serial-product-smp1-r2b-20261117T1305.log:92`.
  Lưu ý: ảnh target đã bị tab KT-77b **rebuild lại** sau phép đo này (giờ có shell), nên ca âm
  chỉ có giá trị lịch sử trên đúng sha đã đo — không chạy lại được trên target hiện tại:

```text scope=v2/agent/JOURNAL/kt77a-serial-product-smp1-r2b-20261117T1305.log
not ok 1 - help: khong thay shell trong log (qemu_rc=0)
# parity: KET_LUAN=khong_co_shell (exit 5)
```

## §5. Giới hạn đã đo (phải đọc trước khi tuyên bố "tương đương v1")

- **Ảnh test không có shell "tự do"**: shell trong `ktest-proc` đọc **script sáu dòng do kernel bơm**
   (`v2/kernel/vifast-kbin-riscv64/src/proc_demo.rs:270-280`), rồi `exit`; byte serial ngoài **vào
   được** shell nhưng **gộp cửa sổ RX** với dòng script kernel (khối dẫn nguồn dưới) và TAP nội
   bộ của ảnh test đỏ — runner parity chỉ chấm `expect` của kịch bản.

```text scope=v2/agent/JOURNAL/kt77a-serial-1-help-serial-ktest-proc-smp1-r2b-20261117T1305.log
# con uart: irqs=41 bytes=55 const:script_bytes=45 lines=6 inject_ok=0 drops=0 timeouts=0 head=42 tail=42 used=0 empty_polls=0 const:va=0x43000000 ring_ok=1
```

- **`ps` phụ thuộc ảnh test (OD-K6-2 tiêu chí sáu)** — cảnh báo bắt buộc: `ps` được đánh `active`
   **nhưng** bên phục vụ `PS_REQ` chỉ tồn tại trong ảnh test
   (`v2/kernel/vifast-kbin-riscv64/src/proc_demo.rs:338`); đường **sản phẩm** **không** có procsrv.
   Muốn `ps` chạy trên ảnh sản phẩm thì **KT-77b** phải đưa procsrv vào đường sản phẩm. Đây đúng
   dạng "user mode phụ thuộc test harness" mà OD-K6-2 tiêu chí sáu bắt phải báo.
- **`help` in hằng không nhãn `const:` (D2-102a):** kernel in số lệnh từ hằng `CMDS`
   (`v2/kernel/vifast-kbin-riscv64/init/shell_user.rs:45`), **không** có nhãn `const:`. KT-77a
   **không** sửa `v2/kernel/**`; runner đã bỏ khoá regex vào số cụ thể (`expect` chỉ đòi
   `cmds=` theo sau là chữ số bất kỳ). Việc thêm nhãn `const:cmds=` (hoặc in từ biến) thuộc
   **KT-77b** khi sửa shell.
- **Nguồn gốc ảnh (D2-122):** runner đọc ảnh từ `v2/kernel/target/.../release/` và ghi sha+size vào
   log (§3); `artifacts/<sha>.bin` cho ảnh đó **chưa có**. Ảnh sản phẩm `fd202276…` là **ảnh trước
   canonical — KHÔNG dùng làm bằng chứng v2.0** (D2-122(2)). Việc đóng băng `artifacts/<sha>.bin`
   thuộc **KT-84**. Lúc đo, tab KT-77b đã tạo một artifact có `# shell: up` — **không** phải phép đo
   của KT-77a.
- **Đo trên máy có QEMU của tab khác** ở vòng `-r2b` ⇒ `qg_infra` đánh dấu phép đo **không hợp lệ**
   (khối dẫn nguồn ở §3); không dùng số thời gian; **không** có QEMU mồ côi.
- **Chỉ bốn lệnh chạy được**; hai mươi bốn mục `deferred-K7` (bảng §1), trong đó `cd`/`pwd` **chờ
   Agent 0** quyết vì hai dữ kiện mâu thuẫn.

## §6. D2-116 / OD-K6-6 — QEMU

- Mọi boot đi qua `qemu_guard.sh` (không sửa): `qg_init` + `qg_boot` + `qg_infra`.
- Cuối mỗi lần đo có dòng `# parity: qemu_con_sot_total=… qemu_cua_script=…` — ví dụ
  `v2/agent/JOURNAL/kt77a-transcript-ktest-proc-smp1-r2b-20261117T1305.log:158` và `v2/agent/JOURNAL/kt77a-serial-ktest-proc-smp1-r2b-20261117T1305.log:588`. Số khác không ở vòng `-r2b`
  là QEMU **của tab khác** (KT-77b), không phải của runner.
- **D2-108B:** bộ đếm "QEMU còn sót của runner" **không** có ca âm riêng trong KT-77a (dùng chung
  `qg_infra`, vốn có ca âm `QG_FORCE_NO_SETPGRP` ⇒ `isolation_bad` khác không ở
  `v2/docs/k6-harness.md`); ghi rõ để không đọc "không" thành "đã chứng minh". Các bộ đếm `not_ok`,
  `timeout`, `khong_co_shell` **đều có** lần khác không trong ba ca âm §4.

## §7. Việc chưa làm

- **KT-77b** (tab khác): shell + U-mode + **procsrv** trên đường sản phẩm; chạy
  `run.sh --mode serial --image v2/artifacts/<sha>.bin`; smp một/hai/bốn hart; ca âm "shell bị tắt";
  thêm nhãn `const:` cho `cmds=`.
- **Hai mươi bốn lệnh `deferred-K7`**: phần lớn cần VFS client trong shell hoặc builtin mới (`time`,
  `uptime`, `jobs` chỉ cần builtin — syscall đã có); `cd`/`pwd` chờ **Agent 0** quyết.
- **KT-84**: đóng băng `artifacts/<sha>.bin` cho ảnh đo + gắn nhãn canonical.
