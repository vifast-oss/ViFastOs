# KT-78 — harness QEMU: process group riêng, timeout cứng, dọn dẹp (OD-K6-6 / D2-116)

Tệp này là **hợp đồng** của `v2/test/boot/qemu_guard.sh`: mọi lần mở QEMU trong harness dòng OS
đều đi qua nó. Bối cảnh: **D2-116** — ngày `2026-11-17` lúc `03:35` có hai `qemu-system-riscv64`
**PPID** `1` chạy `2h58`/`2h55` ở `100 %` CPU mỗi cái, đốt hai core đúng lúc gate/matrix/soak/persist đang
đo ⇒ mọi số **thời gian** trong khoảng đó không dùng được. Nguyên nhân: script cha chết
(SIGINT/SIGKILL) thì `kill $pid` trong script không bao giờ chạy, QEMU bị `init` nhận nuôi.

## 1. Hợp đồng (`qemu_guard.sh`)

| Hàm | Việc |
|---|---|
| `qg_init` | bật trap `EXIT/INT/TERM/HUP`, dựng registry PGID + **watchdog** bám PID script. Gọi **một lần** ngay sau khi `source`. |
| `qg_boot <log> <tds> <bin> [args…]` | chạy QEMU trong **process group riêng**; `0` = QEMU tự thoát, `124` = quá `<tds>` (đơn vị `0,1 s`) ⇒ đã kill **cả group**, `125` = không khởi động được. |
| `qg_tmp <tệp>…` | đăng ký tệp tạm để trap `rm` khi thoát. |
| `qg_infra <tag> [tệp]` | in (và ghi thêm vào `[tệp]`) dòng hạ tầng `qemu_total/qemu_ppid1/qemu_own/qemu_foreign/loadavg_*/ncpu/boot_done/verdict`. Trả `1` khi phép đo **KHÔNG HỢP LỆ**. |
| `qg_own` | số QEMU **của chính script** còn sống (theo PGID đã đăng ký). |
| `qg_cleanup` | kill mọi group còn sống + `rm` tệp tạm; trap gọi tự động. |

**Không đổi ngữ nghĩa cổng**: `qg_boot` giữ đúng chữ ký/ngữ nghĩa cũ của `boot_image`/`boot_one`/
`run_boot` (timeout `20 s/boot` = `TIMEOUT_DS=200`, `-icount` chỉ trong `ktest.sh`, `-smp` `1/2/4`, log
vẫn là `<ảnh>-smp<N>-<boot>.log`). Số boot, số seed, ngưỡng, `expected.txt` **không đổi**; dòng log
QEMU **không bị chèn thêm** (bằng chứng process group ghi ra tệp riêng `qemu-guard.log`), nên mọi
validator của cổng chạy nguyên như trước.

## 2. Cơ chế (macOS không có `setsid`, không có `timeout`, `bash 3.2`)

1. **Process group riêng** — `perl -e 'setpgrp(0,0); exec @ARGV'`: QEMU **là leader** của group
   riêng, `PGID == PID == $!`. Dọn bằng `kill -TERM -- -$pgid` rồi `-KILL` sau `2 s`. **Không**
   `pkill -f qemu-system` (AGENTS §9); mỗi lần kill còn kiểm `qg_group_has_qemu` (group phải **có**
   QEMU) nên không bao giờ giết nhầm group khác.
2. **Bằng chứng PGID đo được** — trước `exec`, perl ghi `pid`/`pgid` bằng `getpgrp(0)` ra
   `$QG_DIR/qg-$$-<seq>.binfo`; `qg_boot` đọc tệp đó ⇒ boot thoát trong `0,1 s` vẫn chứng minh được
   `isolated=1` (không phụ thuộc `ps` kịp nhìn thấy tiến trình — đó là lỗi đo đã gặp: bản đầu đọc
   `ps` ngay sau `&` nên thấy **group của cha**, báo động giả).
3. **Timeout cứng** — vòng `sleep 0,1` đếm đủ `tds` bước (đúng như bản cũ) **và** trần tuyệt đối
   theo `SECONDS` = `2×` danh nghĩa `+ 5 s`, để boot không thể treo vô hạn kể cả khi `sleep` bị kéo
   dài. Hết hạn ⇒ kill **cả group** (QEMU + con nó sinh), trả `124`.
4. **Cha chết, kể cả SIGKILL** — watchdog là subshell `disown` bám PID của script (`ps -o stat=`):
   script biến mất **hoặc thành zombie** ⇒ đọc registry PGID ⇒ kill từng group ⇒ `0` QEMU còn sống.
   Đây là đường duy nhất SIGKILL cha không kịp chạy trap. Registry là tệp **dùng chung** của các
   luồng trong cùng script (soak ba lane); chỉ thân script chính/watchdog được xoá tệp
   (`BASH_SUBSHELL`), lane không xoá.
5. **Trap `EXIT/INT/TERM/HUP`** — `qg_cleanup` kill mọi group đã đăng ký + `rm` tệp tạm (ví dụ FIFO
   `con-smp<N>-boot<I>.in/.out` của `ktest.sh`).

## 3. Nối vào đâu

| Tệp | Thay đổi |
|---|---|
| `v2/test/boot/ktest.sh` | `boot_image` → `qg_boot`; `qg_infra before/after` + `# infra: invalid=… isolation_bad=…`; FIFO console vào `qg_tmp`. |
| `v2/test/boot/soak.sh` | `boot_one` → `qg_boot`; `qg_infra before/after` (ba lane dùng chung registry). |
| `v2/test/boot/persist.sh` | `run_boot` → `qg_boot`; `pid` lấy từ `QG_LAST_PID` (bằng chứng “mười boot, mười PID khác nhau” của KT-75 **giữ nguyên**); `qg_infra before/after`. |
| `v2/test/boot/run.sh` | vòng boot → `qg_boot`; thêm `LOGDIR` (`KT49_LOGDIR`, mặc định `/tmp/k3-run`) cho log guard/infra; `qg_infra before/after`. |
| `v2/test/boot/infra.sh` | chụp hạ tầng quanh phép đo **không** do script boot sinh ra (`gate`/`L1`). |
| `v2/bin/v2` | **không sửa** — tệp này **không** mở QEMU (`grep -c 'qemu-system' v2/bin/v2` = `0`); `gate`/`L1` chụp hạ tầng bằng `infra.sh` (hai dòng dán vào log bằng chứng của task). |

## 4. Dòng `# infra` và luật “KHÔNG HỢP LỆ”

Mẫu (mọi giá trị lấy từ **biến đo**, D2-102) — trích **nguyên văn** từ log matrix của KT-78:

```text scope=kt78-ktest-matrix.log
# infra before: qemu_total=0 qemu_ppid1=0 qemu_own=0 qemu_foreign=0 loadavg_1m=2.98 loadavg_5m=2.72 loadavg_15m=3.52 ncpu=12 boot_done=0 verdict=OK
```

- `qemu_total` = số tiến trình `qemu-system*` **toàn máy**; `qemu_ppid1` = trong đó có `PPID` `1`
  (mồ côi); `qemu_own` = số do **script này** đang chạy; `qemu_foreign = qemu_total − qemu_own`.
- **`verdict=KHONG_HOP_LE`** khi `qemu_ppid1 > 0`, hoặc `qemu_foreign > 0` (QEMU của task khác),
  hoặc `loadavg_1m > ncpu`; in thêm `WARNING` ra stderr và `# MEASUREMENT_INVALID tag=… ly_do=…`.
  Đúng OD-K6-6: “có QEMU lạ thì phép đo bị đánh dấu không hợp lệ”, không chỉ ghi chú.
- Đánh dấu **không** làm đổi exit code của cổng (cổng vẫn xanh/đỏ theo đúng tiêu chí cũ); nó chỉ
  nói số **thời gian** của lần đo đó không được dùng để kết luận hiệu năng (D2-116c, D2-100).
- Ca âm của bộ đếm (D2-108B): `guard_selftest.sh` **tạo** một QEMU mồ côi có kiểm soát rồi chạy
  `infra.sh`; kết quả phải là `verdict=KHONG_HOP_LE … ly_do=qemu_ppid1=1` (log trong repo).

## 5. Ca âm bắt buộc (a)(b) — `guard_selftest.sh`

Lệnh: `bash v2/test/boot/guard_selftest.sh --mode both` — ghi log vào `v2/agent/JOURNAL/kt78-guard/`
(mỗi lần chạy một tệp `selftest-<dấu-thời-gian>.log`, **không** ghi đè bằng chứng cũ).

| Ca | Kịch bản | Kỳ vọng |
|---|---|---|
| `old-sigkill` | **pattern cũ** (không group, không trap, không watchdog) + `kill -9` cha | QEMU **còn sống**, `PPID` `1` ⇒ tái hiện D2-116 (đỏ-trước) |
| `old-sigint` | pattern cũ + `kill -INT` cha (SIGINT đặt lại DEFAULT trước `exec`, vì bash đặt SIGINT=ignore cho job nền) | QEMU **còn sống**, `PPID` `1` |
| `guard-sigkill` | `qemu_guard.sh` + `kill -9` cha | **không còn** QEMU trong `≤ 5 s` (watchdog dọn), `wrapper_rc=137` |
| `guard-sigint` | `qemu_guard.sh` + `kill -INT` cha | **không còn** QEMU trong `≤ 5 s` (trap dọn), `wrapper_rc=130` |
| `guard-sigterm` | `qemu_guard.sh` + `kill -TERM` cha | **không còn** QEMU trong `≤ 5 s`, `wrapper_rc=143` |
| `old-timeout` / `guard-timeout` | khách **không bao giờ thoát**: bốn byte `6f 00 00 00` = `j .`, nạp bằng `-bios` (raw) | `exit=124`, không treo vô hạn, `0` QEMU còn |
| `clean-boot` | ảnh **sản phẩm** boot thật tới `# k3 halt` | `exit=0`, guard log `isolated=1` |
| `infra-orphan` | tạo QEMU `PPID` `1` có kiểm soát rồi chạy `infra.sh` | `verdict=KHONG_HOP_LE`, `qemu_ppid1=1` |
| `iso-bad` | chạy QEMU **không** qua `setpgrp` (`QG_FORCE_NO_SETPGRP=1`, hook **chỉ** cho ca âm) | guard ghi `isolated=0`, `QG_ISO_BAD` tăng, `qg_infra` ⇒ `KHONG_HOP_LE ly_do=isolation_bad=…` (ca âm `D2-108B` cho ba bộ đếm `isolated_*`/`isolation_bad`) |

Mỗi ca in `alive_before=[pid ppid pgid stat]` **trước** khi gửi signal: ca chỉ có giá trị khi QEMU
**thật sự còn sống** lúc đó (chống ca âm “rỗng” — D2-108B). Ca `old-*` chứng minh bộ đếm
`qemu_orphan=1` **có thể** khác `0`, nên `qemu_ppid1=0` ở ca `guard-*` không phải số vô nghĩa.

**Tái lập từ ngoài (đúng tiêu chí của reviewer)**: để `QG_FORCE_NO_SETPGRP` **không** đặt ⇒ ca
`iso-bad` xanh; đặt `QG_FORCE_NO_SETPGRP=0` ⇒ ca `iso-bad` **ĐỎ** (hook mặc định `1` nhưng cho
override từ môi trường, nên không phải sửa script trên bản copy):

```text scope=kt78-guard/selftest-guard-default.log
# guard_selftest: cases=8 ok=8 logdir=/tmp/kt78-proof-default/run-20261117T022304Z
```

```text scope=kt78-guard/selftest-guard-override0.log
not ok 8 - iso-bad: guard KHÔNG phát hiện QEMU ngoài process group riêng
# guard_selftest: cases=8 ok=7 logdir=/tmp/kt78-proof-zero/run-20261117T022320Z
```

**Chạy lúc máy có QEMU của tab khác**: các ca `*-timeout` và `infra-orphan` đếm **toàn máy**, nên
sẽ **ĐỎ** (`qemu_total` lệch baseline, `qemu_foreign>0`) — đúng `OD-K6-6` (“có QEMU lạ thì phép đo
không hợp lệ”) chứ **không** phải harness hỏng. Chạy lại khi máy rảnh; con số trong tài liệu này lấy
từ lần chạy **không** có QEMU lạ (log có `baseline: qemu_total=0`).

## 6. Chạy lại (thứ tự không chạy song song — AGENTS §6, tối đa `3` QEMU)

Theo thứ tự (không chạy song song), mỗi lệnh một dòng:

- ca âm (a)(b) + đỏ-trước: `bash v2/test/boot/guard_selftest.sh --mode both`
- hạ tầng **trước** cổng: `bash v2/test/boot/infra.sh before-gate v2/agent/JOURNAL/kt78-gate-infra.log`
- cổng: `./v2/bin/v2 gate --fast`
- hạ tầng **sau** cổng: `bash v2/test/boot/infra.sh after-gate v2/agent/JOURNAL/kt78-gate-infra.log`
- matrix: `bash v2/test/boot/ktest.sh 1 matrix`
- soak: `bash v2/test/boot/soak.sh 1000 3`
- persist: `bash v2/test/boot/persist.sh`

Số của từng lần chạy **chỉ** nằm trong khối `text scope=` (log trong repo); sau mỗi lần chạy,
`qg_guard_summary` sinh `guard-summary.log` cạnh log chính (nguồn + công thức in kèm).

## 7. Giới hạn / nợ (nói rõ, không giấu)

- Phụ thuộc **`perl`** (có sẵn trên macOS) cho `setpgrp` + `exec`: thiếu perl ⇒ `qg_init` trả `2`,
  harness **dừng** (fail-closed) chứ không chạy QEMU thiếu process group.
- `qemu_own` đếm theo **PGID đã đăng ký** trong tiến trình hiện tại; luồng phụ (lane soak) có
  danh sách riêng nên `qemu_own` của thân script chính không thấy QEMU của lane đang chạy. Vì
  `qg_infra` chỉ gọi **trước/sau** phép đo (không có boot đang chạy) nên điều này không ảnh hưởng
  kết luận; nếu sau này gọi giữa lúc có lane thì phải cộng registry (nợ nhỏ, có ghi).
- Ba bộ đếm `isolated_0`/`isolation_bad`/`isolated_unknown` **đã có ca âm** (`iso-bad`): chạy QEMU
  không qua `setpgrp` ⇒ `isolated=0`, `isolation_bad=1`, `qg_infra` kết luận `KHONG_HOP_LE`. Không
  còn bộ đếm “luôn `0` mà không chứng minh”.
- Trần `loadavg_1m > ncpu` là ngưỡng **thô** (máy mười hai nhân bị hai QEMU mồ côi chỉ thêm khoảng hai đơn vị tải nên **không**
  bắt được bằng load); bộ đếm QEMU là detector chính, load chỉ là lưới phụ. Ghi rõ để không ai
  đọc “verdict=OK” thành “máy chắc chắn rảnh”.
- Không có `setsid`/`timeout` trên macOS ⇒ dùng perl + vòng `sleep`; `setsid`/`timeout` của Linux
  không được giả định ở đâu trong harness.

## 8. KT-348b — mất log phải có tên; gửi phím không nằm trong cửa sổ 5 s của ảnh

Hai lỗi harness đo được ở vòng nghiệm thu `KT-348` (soak aarch64 `992/1000`):

1. **7 boot không có tệp log vì đĩa đầy** (`qemu_guard.sh: line 225: …: No space left on device`).
   Lỗi xảy ra ở redirection `>"$log"` **của tiến trình con**, nên `qg_start` vẫn trả `0` và boot
   thành `# không có log` **không lý do**. Nay `qg_start` **mở/tỉa log trước** khi exec; thất bại
   ⇒ `QG_START_ERR="log-open: <errno>"` + trả **125**. `soak-aarch64.sh` xếp slot đó là
   `fail i start-log-open:…`, in `#   hạ tầng: …`, bật `INFRA-STOP` (**dừng soak** — chạy tiếp chỉ
   sinh thêm boot không bằng chứng) và **đếm** tệp log (`soak_a64_log_inventory`): thiếu tệp ⇒
   `# không có log … (lý do: …)` + soak ĐỎ. Hợp đồng mới: **không có tệp log = ĐỎ có tên**, không
   phải boot đỏ bình thường.
2. **1 boot `not ok 32 - input: key` với `events=0`, `elapsed_ms=5076`** nhưng log harness có
   `# soak-aarch64.sh: không nối được monitor …: [Errno 2] No such file or directory`: `sendkey a`
   **chưa bao giờ được gửi**. Đường cũ (chờ `# input: ready` bằng `grep` → spawn `python3` →
   `connect`) nằm trong cửa sổ 5 s của ảnh; dưới tải máy đo, chính đường đó mất 1,5–3,8 s (và một
   lần > 5 s). Nay `lib/monitor-sendkey.py` nối monitor **trước** (song song lúc khách boot) và gửi
   `sendkey a` trên kết nối **đã mở sẵn** ngay khi thấy ready; lỗi gửi ⇒ `fail i monitor-<lý do>`
   (T-INFRA), **không** đội lốt TAP 32. Ngưỡng `READY_DS` (5 s) và `INPUT_WAIT_SECS` (5 s) của ảnh
   **không đổi**. Bằng chứng mỗi lần soak: `# kbd: sendkey ok=… fail=… ready_ms_max=… send_ms_max=…`.

3. **Ba `trap … EXIT` chồng nhau** (từ trước KT-348, D2-257 mục 1): `trap` sau **ghi đè** trap
   trước nên `stop_soak_dns_server`/`stop_soak_tcp_server`/`qg_cleanup` **không chạy** khi script
   thoát bằng đường lỗi (`die`/`exit 3`/tín hiệu). Nay **một** hàm `_soak_a64_cleanup` + **một**
   `trap EXIT` (INT/TERM dùng cùng hàm) gọi đủ bốn việc: dừng DNS server, dừng TCP server,
   `qg_cleanup`, `rm -rf SOCKDIR` và nhả `measure-lock`; hai trap bị ghi đè đã xoá.

Lưu ý đọc log: `notifies` trong `# input stats:` là số **cửa thông báo do driver ghi** (init notify
một lần cho 8 đệm), **không** phải số ngắt từ thiết bị; boot sạch có `events=1 notifies=2`, còn
`notifies=1 events=0` nghĩa là **thiết bị chưa từng nhận phím**.
