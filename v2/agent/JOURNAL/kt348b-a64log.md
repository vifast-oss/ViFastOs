# KT-348b — soak aarch64: 7 boot "không có log" (ENOSPC) + 1 boot `input: key` timeout (harness)

**Task**: KT-348b (3 h) — nhánh `task/kt348b`, worktree `/tmp/kt348b-s1`; base `main` @ `4d4cbb62`,
**đã ff-only lên `e5555e5b`** (D2-257 của Agent 0, chỉ đổi `QUEUE`/`DECISIONS`/kế hoạch).
**Bằng chứng gốc**: phiên nghiệm thu KT-348, bước 5 = soak aarch64 `product-shell` N=1000 × 3 luồng,
`target-task/nightly/nightly-20261123-234735-4d4cbb62/soak-aarch64/run-20261124-111303-86008/`
+ `step-5-soak.log`. Kết quả gốc: **`# soak: 992/1000 lỗi=8`**.

## 1. Kết luận trước (đọc nhanh)

| # | Hiện tượng | Nguyên nhân **đo được** | Loại |
|---|---|---|---|
| 1 | 7 boot 814, 817, 818, 819, 820, 822, 823 **không có tệp log** | **Đĩa đầy (`ENOSPC`)**: redirect `>"$log"` trong `qg_start` thất bại, QEMU **không được chạy**, nhưng `qg_start` vẫn trả 0 ⇒ caller báo `không có log` **không lý do** | **harness** (fail-closed hỏng) |
| 2 | 1 boot 861 có log, `not ok 32 - input: key`, `events=0 elapsed_ms=5076`, `notifies=1` | `send_key` **không kết nối được monitor**: `[Errno 2] No such file or directory` (QEMU đã thoát khi ảnh hết hạn 5 s) — harness cũ `|| true` **nuốt** lỗi, boot đỏ trông như lỗi nhân | **harness** (thứ tự gửi) |
| 3 | `# dns server: chỉ 994 …` | **hệ quả** của 7 boot không chạy: 994 = **probe + 993 boot QEMU thật sự chạy** (1000 − 7); boot 861 vẫn tới DNS (`ok 35`) rồi mới chết ở input | hệ quả, không phải lỗi riêng |
| 4 | 3 `trap … EXIT` chồng nhau (D2-257 mục 1) | `trap` sau **ghi đè** trap trước ⇒ `stop_soak_dns_server`/`stop_soak_tcp_server`/`qg_cleanup` **không chạy** khi script thoát đường lỗi | **harness** (rò rỉ khi lỗi), đã sửa |

Không có lỗi nhân nào trong 8 boot đỏ: **7 boot QEMU không chạy**, **1 boot harness gửi phím trượt**.
Không nới ngưỡng nào; mọi sửa đổi là **fail-closed có tên** + rút ngắn đường gửi phím.

## 2. Bằng chứng gốc — 7 boot không log là `ENOSPC`, không phải QEMU không lên

`step-5-soak.log` (nguyên văn, 7 dòng):

```text
/Users/mps/dvchd/ViFastOs/v2/test/boot/qemu_guard.sh: line 225: …/boot-814.log: No space left on device
boot 814/1000 FAIL: # không có log …/boot-814.log log=…/boot-814.log
tail: …/boot-814.log: No such file or directory
```

Đủ 7 cặp `No space left on device` + `# không có log` cho 814, 817, 818, 819, 820, 822, 823
(`grep -c 'No space left on device' step-5-soak.log` = **7**). Thư mục run có **994** tệp
`boot-*.log` (1000 − 7 thiếu + 0 tệp rỗng; boot 861 **có** log). Đĩa lúc đo (kiểm lại hôm nay):
`460Gi`/`17Gi` trống — áp lực **tạm thời** (cùng lúc 7 lần ENOSPC rồi hết), đúng loại sự cố hạ tầng
không thể "đoán" từ log QEMU vì **không có log QEMU**.

Ba câu hỏi của A0 (D2-257 mục 1), trả lời bằng số:

| Câu hỏi | Trả lời đo được | Bằng chứng |
|---|---|---|
| 7 tệp log **0 byte** hay **không tồn tại**? | **Không tồn tại** (không phải rỗng): `ls` không có tệp, `tail` báo `No such file or directory` | 7 dòng `No such file or directory` trong `step-5-soak.log`; run dir đếm đúng 994 tệp |
| QEMU có được **spawn** không? | **Không**. Lỗi xảy ra ở redirection của shell **trước** `exec` (thông báo là của `bash`/`qemu_guard.sh`, không phải của `qemu-system-aarch64`) | 7 dòng `qemu_guard.sh: line 225: … No space left on device` |
| `qg_start` trả gì? | **0** (đó là lỗi). Không có `QG_START_ERR`; `boot_one` cũ chỉ phân biệt `rc != 0` = "quá 20 s (treo)", còn tệp không có ⇒ `check_boot_log` in `# không có log` trống | mục `fail 814 #,không,có,log,…` của `results.txt` không có trường lý do |

**Vì sao harness nuốt lỗi**:

1. `qg_start` chạy `perl … >"$log" 2>&1 &` — redirection thực hiện **trong tiến trình con**; khi nó
   thất bại, bash in lỗi ra stderr của script rồi con chết, nhưng `qg_start` vẫn thấy `$!` và
   **trả 0**.
2. `boot_keys` chờ `# input: ready`, QEMU chết nên `wait` trả nhanh, hàm trả 0 — **không phân biệt**
   "QEMU chết vì không mở được log" với "boot xong".
3. `check_boot_log` thấy `[ ! -f "$log" ]` ⇒ in `# không có log <tệp>` **không có lý do**.
4. Không có phép **đếm** tệp log ⇒ 7 slot mất bằng chứng vẫn trôi qua như boot đỏ bình thường;
   186 boot sau vẫn chạy tiếp.

### 2b. `trap EXIT` chồng nhau (D2-257 mục 1) — đã tái hiện và sửa

Trước khi sửa, `git show 4d4cbb62:v2/test/boot/soak-aarch64.sh` có **ba** `trap … EXIT` liên tiếp
(đúng như A0 chỉ: dòng 1148, 1512, 1546):

```text
1148:trap 'qg_cleanup; measure_lock_release_if_top' EXIT
1512:trap 'stop_soak_dns_server; stop_soak_tcp_server; qg_cleanup; measure_lock_release_if_top' EXIT
1546:trap 'rm -rf "$SOCKDIR" 2>/dev/null || true; measure_lock_release_if_top' EXIT
```

Đo tối thiểu (bash 3.2, cùng mẫu): ba `trap … EXIT` ⇒ **chỉ trap cuối chạy**:

```text
$ bash x.sh   # trap A; trap B; trap C
chỉ chạy: C
```

⇒ `stop_soak_dns_server`/`stop_soak_tcp_server`/`qg_cleanup` **không bao giờ chạy** khi script
thoát (đường `die`/`exit 3`/lỗi giữa chừng): DNS/TCP server + QEMU group **rò rỉ**. Đây là lỗi
harness thật, **đã sửa** (mục 3). **Đính chính**: lỗi trap **không** phải nguyên nhân của
`# dns server: chỉ 994` — con số 994 khớp chính xác probe + 993 boot có DNS (1000 − 7 boot không
chạy), và trong lần chạy đó server DNS của chính run đã trả lời đủ 994 truy vấn (nếu server cũ rò
rỉ giữ cổng 53 thì log server của run mới sẽ **0** truy vấn, không phải 994). Sự cố DNS là **hệ
quả** của 7 boot không chạy, không phải nguyên nhân độc lập.

## 3. Sửa (1) — fail-closed có tên khi không mở được log + log phải có mặt & khác rỗng

| Tệp · dòng | Sửa |
|---|---|
| `v2/test/boot/qemu_guard.sh` `qg_start` (sau kiểm tham số, trước `perl`) | `QG_START_ERR=""`; `: >"$log"` **trước** khi exec, bắt stderr: `_log_err="$({ : >"$log"; } 2>&1)"`; thất bại ⇒ `QG_START_ERR="log-open: <errno>"`, in `qemu_guard: KHÔNG mở được log …`, **trả 125** |
| `soak-aarch64.sh` `boot_one` | `rc=125` ⇒ `fail i start-log-open:…` + `#   hạ tầng: boot i: QEMU không khởi động được: …` + **`: >"$INFRA_STOP"`** (dừng soak) — không còn bị báo `timeout-20s`/`không có log` trống |
| `soak-aarch64.sh` `check_boot_log` | tham số thứ 4 `miss_reason`; thiếu tệp ⇒ `# không có log <tệp> (lý do: …)` |
| `soak-aarch64.sh` `soak_a64_log_inventory` | đếm **từng slot**: **có mặt + khác rỗng**; thiếu/rỗng ⇒ in `# không có log … (lý do: …)` / `# log rỗng … (lý do: …)` và soak ĐỎ qua `SOAK_EXTRA_CHECK` |
| `soak-aarch64.sh` **trap** | **một** `_soak_a64_cleanup` + **một** `trap EXIT` (và INT/TERM) gọi đủ: `stop_soak_dns_server`, `stop_soak_tcp_server`, `qg_cleanup`, `rm -rf SOCKDIR`, `measure_lock_release_if_top`; xoá hai trap bị ghi đè (D2-257 mục 1) |
| `soak-aarch64.sh` `boot_one_or_stop` + `SOAK_ABORTED` | lane thấy `INFRA-STOP` thì không chạy boot mới; DNS không bị kết luận sai "interpose hỏng" khi soak dừng sớm |

## 4. Bằng chứng gốc — boot 861: ai gửi phím, khi nào, vì sao `events=0`

Ảnh (`v2/kernel/vifast-kbin-aarch64/src/input.rs`): `init` **arm 8 đệm** rồi in `# input: ready`,
sau đó poll tối đa `INPUT_WAIT_SECS = 5`; hết hạn ⇒ `a_code=0`, `elapsed_ms≈5000`, TAP 32 đỏ.

Harness cũ (`soak-aarch64.sh`): chờ `# input: ready` bằng `grep` (mỗi 0,05 s), **rồi mới**
`send_key` = spawn `python3` → `connect()` monitor → `sendkey a`. Cả đường đó **nằm trong** cửa sổ
5 s của ảnh.

`step-5-soak.log` dòng 896 (nguyên văn) — **bằng chứng quyết định**:

```text
# soak-aarch64.sh: không nối được monitor /tmp/v2s-kt.LMcZ60/mon-861.sock: [Errno 2] No such file or directory
```

Tức `sendkey a` **chưa bao giờ được gửi**: `connect()` nhận `ENOENT` vì QEMU đã **thoát** (QEMU xoá
socket monitor khi thoát; mtime `boot-861.log` = `11:26:40`, đúng lúc ảnh in `not ok 32` sau
5.076 s). `send_key … || true` **nuốt** lỗi ⇒ boot đỏ đội lốt `TAP 32` của nhân.

Vì sao trượt: cửa sổ 5 s bị tiêu bởi **tải máy đo**, không phải do ảnh. Đo trên 993 log có log:

| chỉ số | min | p50 | p99 | max |
|---|---:|---:|---:|---:|
| `# input: key a … elapsed_ms` (KT-348) | 37 | 204 | 1.081 | **5.076** (boot 861, hết hạn) |

Các boot lân cận cùng cửa sổ đều trượt mạnh: 851 = **2.129**, 855 = **2.253**, 856 = **2.324**,
857 = **3.827**, 862 = **1.578** ms — cùng lúc với chùm `ENOSPC` 814..823, tức máy đo bị **stall**
(tranh CPU/IO) chứ không phải ảnh chậm. Harness cũ không có ngưỡng nào cho "đã gửi hay chưa" và
không phân loại lỗi gửi.

**Đính chính một giả thuyết trong đề bài**: `notifies=1` **không** phải "thiết bị đã notify mà không
có sự kiện". Trong `vifast-virtio::inputdev`, `notifies` đếm **cửa thông báo do driver ghi**
(`handoff_and_notify`), và `init` arm 8 đệm rồi notify **một lần** ⇒ boot sạch có `events=1
notifies=2`; boot 861 có `notifies=1` = **chỉ** lần notify lúc init, thiết bị **chưa từng** nhận
phím. Vì vậy đây là lỗi **harness**, không phải driver/kernel.

## 5. Sửa (2) — rút đường gửi phím ra khỏi cửa sổ 5 s + fail-closed

| Tệp | Sửa |
|---|---|
| `lib/monitor-sendkey.py` (**mới**) | nối monitor **trước** (chạy song song lúc khách boot), poll log bằng `read(2)` (không spawn `grep`), gửi `sendkey a` trên **kết nối đã mở sẵn** ngay khi thấy `# input: ready` (in **sau** khi đã arm 8 đệm), đọc prompt, kiểm `Error`; in/ghi `i=… ok ready_ms=… send_ms=…` hoặc `i=… fail reason=<tên>` |
| `soak-aarch64.sh` `boot_keys` | spawn helper ngay sau `qg_start` (trước ready); chờ **tệp kết quả** (không dựa `kill -0` với zombie); nhánh lỗi ⇒ `QG_KEYS_ERR` có tên, kill group, **trả 126** |
| `soak-aarch64.sh` `boot_one` | `rc=126` ⇒ `fail i monitor-<lý do>` + `#   hạ tầng: boot i: không gửi được phím: …` + stderr `… KHÔNG phải lỗi nhân; T-INFRA` |
| `soak-aarch64.sh` sau lanes | `# kbd: sendkey ok=… fail=… ready_ms_max=… send_ms_max=…`; boot lỗi in kèm lý do |

Ngưỡng **giữ nguyên**: `READY_DS=100` (5 s) và `INPUT_WAIT_SECS=5` của ảnh không đổi; chỉ **thứ tự**
(mở monitor trước) và **điều kiện** (gửi khi ready, lỗi phải có tên) thay đổi.

**Hiệu quả đo được** (cùng máy, cùng 1000 boot): `# input: key a … elapsed_ms` (thời gian ảnh chờ
tính từ `ready`) **p50 204 → ~75 ms**, **p99 1.081 → ~200 ms**, **max 5.076 → ~312 ms**;
`send_ms` (từ lúc helper thấy ready đến khi QEMU trả prompt) **p50 ~1 ms**, max ~0,2 s; 0/1000 lần
gửi hỏng.

## 6. Kiểm chứng — số đo thực tế và **trạng thái trung thực**

| Phép kiểm | Kết quả |
|---|---|
| `bash v2/test/boot/soak-aarch64.sh --selftest` (code cuối) | **48/48 ca** (1 dương + 47 âm), PASS |
| Đột biến "ca kiểm có răng" | **3/3**: bỏ pre-create log ⇒ ca `qg_start log-open` ĐỎ; nuốt lỗi connect ⇒ ca `helper ca âm` ĐỎ; inventory luôn xanh ⇒ 2 ca đếm log ĐỎ (tệp khôi phục đúng sha256) |
| `--smoke 1 1` (đường mới, code trước khi gộp trap/inventory) | `boot 1/1 ok`; `# kbd: sendkey ok=1 fail=0 ready_ms_max=1050 send_ms_max=3`; `# log: có đủ 1/1` |
| **`bash v2/test/boot/soak-aarch64.sh 1000 3` — lượt 1** | **`# soak: 1000/1000 lỗi=0` → `# soak-aarch64.sh: PASS`**, wall=**944 s**; DNS `queries=1001 replies=1001 local_hits=1001`; TCP `connections=1001`; `# kbd: sendkey ok=1000 fail=0` |
| `run-aarch64.sh` 37/37 | **CHƯA CHẠY trong commit này** (máy đo bận — xem dưới) |
| `./v2/bin/v2 t0` | **PASS** (fmt; 0 crate bị chạm — thay đổi chỉ ở harness/`.py`/tài liệu) |
| `./v2/bin/v2 gate --dev` (T1) | **CHƯA CHẠY trong commit này** (cùng lý do) |
| Soak lượt 2 trên **code cuối** (trap + inventory) | **CHƯA CHẠY** — sẽ do lead chạy trong khung Cao điểm 2 (D2-256) sau khi tuần tự hoá máy đo |

**Trung thực về fingerprint**: lượt soak 1000/1000 ở trên chạy trên code **trước** khi gộp
`trap`+`inventory` (sha256 `soak-aarch64.sh` = `d0a601b7c4c822063aafd248c3a89b592ffb071147eaf0f7846b079b938cbf89`;
`qemu_guard.sh` = `c636c3b5…`; `monitor-sendkey.py` = `d95aa6b5…`). Hai thay đổi sau đó **không đụng
đường boot** (chỉ dọn ở `EXIT` và đếm log ở tổng kết) nhưng **commit này không tuyên bố** chúng đã
được soak 1.000 — lượt cuối trên code cuối (sha256 `soak-aarch64.sh` = `9f5df2d9…`) do lead chạy
trong khung Cao điểm. Nếu lượt cuối ĐỎ thì đó là **dữ liệu thật** (không phải nhiễu của lượt 1).

Bằng chứng thô (máy này, giữ trong `/var/folders/…/T/`):

```text
lượt 1: /var/folders/jp/05hgrzxj65bgh355l2zs2zz80000gn/T//k4-aarch64-soak.87624/
        run-20261124-134441-87624/{results.txt,kbd.log,boot-1.log,…}
        # soak: BUILD_ID=e2d8b6f19d2e1f1f sha256=5b755b04f3a6cacc23e54df6df7fefbea333a21d0f1fcc5952c19cafd7dbfc94 git=4d4cbb6223c8845ec42f886c201fb9ec9912e14c
        # soak: xong wall=944s (1 boot/s); ok=1000 fail=0 tổng=1000/1000
        # soak: 1000/1000 lỗi=0 — 0/N chỉ là trần (QD §5.3); không retry (D2-07)
        # soak: 0/1000 lỗi (PASS)
        # soak-aarch64.sh: PASS
```

Hiệu quả của bản sửa gửi phím, đo trên **cùng 1000 boot** (so sánh với KT-348):

| chỉ số | KT-348 (trước) | lượt 1 (sau) |
|---|---:|---:|
| `# input: key a … elapsed_ms` p50 | 204 ms | **~75 ms** |
| p99 | 1.081 ms | **~200 ms** |
| max | **5.076 ms** (hết hạn, boot 861) | **~312 ms** |
| boot gửi phím hỏng | 1 (bị nuốt, thành TAP 32 đỏ) | **0/1000** (`# kbd: ok=1000 fail=0`) |

## 7. Bàn giao cho lead (trạng thái thật)

- Mã + selftest + T0 xanh; **soak 1.000 đã PASS** trên code gần cuối (khác đúng phần dọn `EXIT` +
  đếm log).
- `run-aarch64.sh` (37/37, TAP 46), `gate --dev` và **soak lượt 2 trên code cuối** chưa chạy vì
  `measure-lock` đang do soak `KT-348a` (`soak-x86_64.sh 600 3`) giữ; lead tuần tự hoá sau 15:00 JST.
- Cách chạy lại (máy độc quyền, `qemu_foreign=0`):
  `bash v2/test/boot/run-aarch64.sh` → `bash v2/test/boot/soak-aarch64.sh 1000 3` →
  `./v2/bin/v2 gate --dev`.

## 8. Tệp đã đổi

| Tệp | Việc |
|---|---|
| `v2/test/boot/qemu_guard.sh` | `QG_START_ERR`, pre-create log, rc 125 (fail-closed có tên) |
| `v2/test/boot/lib/monitor-sendkey.py` | **mới**: helper nối monitor trước, gửi `sendkey a` khi ready |
| `v2/test/boot/soak-aarch64.sh` | `boot_keys`/`boot_one` phân loại 125/126 có tên; inventory log **có mặt + khác rỗng**; **một** trap EXIT (D2-257); `# kbd`; +11 ca selftest |
| `v2/docs/k6-harness.md` | §8: hợp đồng fail-closed mới + thứ tự gửi phím |
| `v2/agent/JOURNAL/kt348b-a64log.md` | journal này |

## 9. Nợ / ghi nhận

- `run-aarch64.sh` vẫn dùng đường cũ (chờ ready → spawn `send_key`): cùng rủi ro lý thuyết nhưng
  37 boot/lần (không phải 1.000); **không sửa** trong task này để giữ phạm vi (nếu tái diễn ⇒ task
  riêng, dùng lại `lib/monitor-sendkey.py`).
- `lib/soak-common.sh` (dùng chung 3 arch) chưa có inventory log — chỉ aarch64 có trong task này;
  nếu soak riscv64/x86_64 gặp lại ENOSPC thì nên nâng lên lib.
- Đĩa đầy là sự cố **hạ tầng** ngoài harness: harness nay **dừng có tên**, nhưng vẫn cần lead kiểm
  dung lượng trước `v2 nightly --all`.
- Trap `EXIT` chồng nhau là lỗi có từ trước (không riêng KT-348); nay đã gộp một chỗ. Các runner
  khác (`soak.sh`, `soak-x86_64.sh`, `run-*.sh`) cần rà cùng mẫu (ngoài phạm vi task).
