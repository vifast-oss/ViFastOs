# KT-347b — thư viện harness dùng chung + `nightly --soak --profile` + dọn tài liệu

- **Task**: KT-347b (D2-245/D2-247/D2-250/D2-252 Pha A), 2 h, hàng đợi `QUEUE.md` Khối F.
- **Worktree**: `/private/tmp/kt347b-s1`, nhánh `task/kt347b`, base `main` @ `68104a04` → merge
  `--ff-only` `origin/main` (nạp `KT-347c` `e1875d53`: `ktest.sh` chờ có hạn 20 s, chống TOCTOU).
- **Ngày**: 2026-11-24 07:10–08:15 JST. **Không** push/merge, **không** chạm `main`.
- **T0/T1**: chỉ `v2 check` + `v2 gate --dev`; **không** `gate --fast`, **không** soak N=1000
  (thuộc `KT-348`, máy đo độc quyền). Mọi phép QEMU chạy sau 07:28 (lead chạy T2 #6 độc quyền
  tới ~07:28); trước đó chỉ đọc/soạn thư viện.

## 1. Ba thư viện mới (`v2/test/boot/lib/`)

| Tệp | Dòng | Gom cái gì | `--selftest` |
|---|---:|---|---|
| `harness-scan.sh` | 256 | **23 tiền tố** D2-08 + 2 cách đếm cũ (occurrences / dòng `strings`), `require_zero` fail-closed, chờ tệp chống TOCTOU | **8/8 PASS** |
| `budget-common.sh` | 364 | cộng `.data+.sdata+.bss+.sbss`, `list_seen`, `machine_of`, `.corespace`, **3 ngưỡng** (65.536 / 262.144 / 184.792), in **một** định dạng `# image=` cho 3 arch, parse dòng + `check_line` (nhãn sai ⇒ ĐỎ) | **12/12 PASS** |
| `soak-common.sh` | 316 | vòng luồng (`N`/`LANES`, mỗi slot **đúng 1 lần**), `wait` đúng PID từng luồng, tổng hợp `ok/fail`, neo sha ảnh, `BUILD_ID`, kết luận (`SMOKE` **luôn** exit 1, không retry D2-07) | **11/11 PASS** |

Chi tiết hợp đồng (nguyên văn đầu tệp):

- `harness-scan.sh`: `HARNESS_SCAN_MARKERS` (23), `harness_scan_count` (`grep -a -o -F`, **lần xuất
  hiện** — cách cũ của `budget-x86_64`/`product-riscv64`), `harness_scan_count_strings`
  (`strings -a | grep -Fc`, **dòng** — cách cũ của `budget-aarch64`/`soak*`), `harness_scan_require_zero`,
  `harness_scan_wait_file` (**20 s**, cùng ngân sách `wait_image` của `KT-347c`). Selftest giữ **cả
  hai** công thức đếm (occurrences=2, strings-lines=1 trên cùng một dòng) để sửa một chỗ không âm
  thầm đổi hành vi arch khác.
- `budget-common.sh`: ngưỡng **một nguồn** — `budget_common_TOTAL=262144`,
  `PRODUCT=65536` (**strict `<`**), `PRODUCT_SHELL=262144` (**`≤`**), `MACHINE=184792` (D2-252).
  `budget_common_check_line` đòi đúng **cả ba** điều kiện: đúng nhãn, đúng trần của nhãn, và với
  `*-product-shell` phải `.corespace ≥ 184.792`.
- `soak-common.sh`: `soak_common_run_lanes` (mỗi slot **một** lời gọi — selftest đếm đúng 7 lời gọi
  cho 7 slot kể cả khi boot giả trả đỏ), `soak_common_finish` (thứ tự giữ nguyên: tổng slot → neo
  ảnh → `BUILD_ID` → **extra check của arch** → `SMOKE` → fail → `N ≥ 3/p` → PASS).

## 2. Số dòng trước/sau (runner **gọi** thư viện, không chép logic)

| Runner | Trước | Sau | Δ |
|---|---:|---:|---:|
| `v2/test/boot/budget.sh` (riscv64) | 147 | 75 | **−72** |
| `v2/test/boot/budget-aarch64.sh` | 387 | 333 | **−54** |
| `v2/test/boot/budget-x86_64.sh` | 452 | 372 | **−80** |
| `v2/test/boot/product-riscv64.sh` | 486 | 485 | **−1** (chỉ thay vòng quét; phần còn lại là kịch bản boot riêng) |
| `v2/test/boot/soak.sh` | 591 | 583 | **−8** (bỏ ~38 dòng lặp, **thêm** `--disk` mới ~20 dòng) |
| `v2/test/boot/soak-x86_64.sh` | 1469 | 1414 | **−55** |
| `v2/test/boot/soak-aarch64.sh` | 1803 | 1747 | **−56** |
| **Tổng 7 runner** | **5335** | **5009** | **−326** |

`--selftest` của từng runner vẫn **nguyên số ca và kết quả**: `budget.sh` 3 ca (nay gọi
`budget_common_selftest` 12 ca), `budget-aarch64` **13/13**, `budget-x86_64` **17/17**,
`soak-x86_64` **54/54**, `soak-aarch64` **40/40**, `soak.sh` không có selftest (giữ nguyên).

## 3. `--profile` của `v2 nightly --soak` (chỉ 2 tệp `v2/bin/lib/*` được sửa, đúng ngoại lệ)

**`v2/bin/lib/steps_soak.sh`** (thêm, không đổi bước cũ):

- `nightly_soak_kinds` — **một** nguồn cho bộ bước theo profile:
  - *(không nêu)* = 4 bước cũ (giữ **nguyên** hành vi trước KT-347b): `soak-riscv64`,
    `soak-aarch64`, `soak-x86_64`, `soak-x86_64-max`;
  - `product-shell` = `soak-riscv64-product-shell` (`soak.sh --features product-shell`),
    `soak-aarch64`, `soak-x86_64`;
  - `soak-x86_64-max` = đúng nhánh `V2_X64_CPU=max`;
  - `virtio-blk` = `soak-riscv64-virtio-blk` (`--features product-shell,product-fs --disk`);
  - `all` = **5** bước (3 sản phẩm-shell + `-cpu max` + đĩa thật).
- `nightly_soak_kind_args` — tham số riêng đặt **trước** N/LANES và trước `--smoke`; in đúng vào
  dòng kế hoạch.
- `nightly_soak_full_n` giữ N: rv/a64 **1.000**, x64 (cả `-cpu max`) **600**; thêm 2 bước mới cũng
  **1.000** — **không** nới/hạ ngưỡng nào.
- `nightly_soak_step` truyền `kargs` cho runner, giữ nguyên kiểm `TỪ CHỐI ĐO` (rc 3) ≠ smoke.

**`v2/bin/lib/nightly.sh`**:

- parse `--profile <tên>` + `--smoke` (lối tắt `--n 1 --runs 1`); `NIGHTLY_SOAK_RUNS_SET` để
  `--smoke` **không** đi cùng `--n`/`--runs` (một nguồn N).
- **fail-closed**: tên lạ ⇒ exit 2 + in **tên**; `--profile` chỉ dùng với `--soak`/`--all`;
  `--smoke` cũng vậy. Selftest thêm 9 ca (tổng **30/30**).
- **in profile đã chọn** ở cả `--dry-run` và lượt chạy thật (dòng `profile soak: … — bước: …`),
  thêm manifest `soak_profile`/`soak_kinds`/`soak_n_*` (không xoá khoá cũ).
- `nightly_plan_scope` in `profile=` cho `--all`/`--soak`.

Chứng minh chọn profile + từ chối tên lạ (log rút gọn):

```
$ ./v2/bin/v2 nightly --soak --profile product-shell --dry-run
   phạm vi: --soak (profile=product-shell; N=1000/1000/600/600; runs=3)
   profile soak: product-shell — bước: soak-riscv64-product-shell soak-aarch64 soak-x86_64
   1  soak  Soak riscv64 product-shell …  $ bash v2/test/boot/soak.sh --features product-shell 1000 3

$ ./v2/bin/v2 nightly --soak --profile virtio-blk --dry-run
   profile soak: virtio-blk — bước: soak-riscv64-virtio-blk
   $ bash v2/test/boot/soak.sh --features product-shell,product-fs --disk 1000 3

$ ./v2/bin/v2 nightly --soak --profile virto-blk --dry-run
v2 nightly --profile: tên lạ 'virto-blk' (hợp lệ: product-shell|soak-x86_64-max|virtio-blk|all)   [exit 2]
```

**`soak.sh` nhận `--disk`** (mới, mặc định **không** đĩa ⇒ hành vi cũ không đổi): tạo đĩa raw
4 MiB trong `LOGDIR`, thêm `-drive …,snapshot=on -device virtio-blk-pci` vào **mọi** boot (kể cả
boot dò `# build:`) và mỗi boot shell phải có `# product fs: medium=virtio-blk ` — **không** chỉ
"có gắn `-drive`". Đĩa thật chỉ được soak ở profile `virtio-blk` (N đầy 1.000 cho `KT-348`).

**Smoke bắt được một lỗi thật (đã sửa)**: `nightly --soak --profile product-shell --smoke` lượt
đầu ĐỎ ở bước x86_64: `soak-x86_64.sh: TỪ CHỐI ĐO — probe boot … '# net: dns name=localhost …'
xuất hiện 0 lần`. Nguyên nhân: **KT-335b (D2-246) thêm trường `bad=<n>`** vào dòng DNS, còn regex
của `soak-x86_64.sh` vẫn đòi `attempts=[1-2] const:timeout_ms=3000` **liền nhau** ⇒ không bao giờ
khớp log thật (soak không nằm trong T2 nên lỗi thời này chưa ai bắt). Đã sửa **regex khớp bằng
chứng** (`attempts=[1-2] bad=[0-9]+ const:…`) + fixture `--selftest` theo đúng dòng log thật;
`soak-x86_64.sh --selftest` vẫn **54/54**. Không nới ngưỡng: vẫn đòi **đúng 1** dòng, `rcode=0`,
`filter` đủ.

**Nói thẳng (không giấu)**: `soak-x86_64.sh`/`soak-aarch64.sh` vẫn soak **ảnh ktest** theo hợp
đồng cũ (TAP là hợp đồng của soak); profile `product-shell` chỉ **thêm** bước riscv64 soak **đúng
ảnh sản phẩm có shell**, còn hai arch kia mới dừng ở việc step **build + kiểm ngân sách/marker**
ảnh `product-shell` rồi soak ảnh ktest. Việc lái shell trên a64/x64 để soak product-shell thật
**chưa thuộc** KT-347b (không có runner lái shell cho hai arch) — ghi nợ cho `KT-348`/task kế.

## 4. Dọn tài liệu

- `git mv` (3 tệp, `git status` ghi `R`):
  - `KE-HOACH-CUOI-TUAN-2026-11-20.md` → `v2/agent/archive/reports/`
  - `KE-HOACH-CUOI-TUAN-2026-11-21.md` → `v2/agent/archive/reports/`
  - `GIAI-THICH-VIFASTOS-CHO-SINH-VIEN.md` → `v2/docs/guides/`
- Liên kết đã sửa: `README.md` §8 thêm 2 dòng — `v2/docs/guides/GIAI-THICH-VIFASTOS-CHO-SINH-VIEN.md`
  và `v2/agent/archive/reports/`; `BAO-CAO-CUOI-TUAN-KT243-2026-11-22.md` trỏ kế hoạch sang
  `v2/agent/archive/reports/KE-HOACH-CUOI-TUAN-2026-11-21.md`. `grep` toàn repo: **không còn**
  liên kết `](KE-HOACH-CUOI-TUAN-2026-11-2[01].md)` / `](GIAI-THICH-…)` cũ. Hai JOURNAL lịch sử
  (`kt242-wake-patch.md`, `kt245-journal.md`) nhắc **tên tệp trong văn xuôi** — giữ nguyên (nhật ký
  là bản ghi thời điểm, không phải liên kết sống); `QUEUE.md`/`DECISIONS.md` là tệp **cấm sửa**.

## 5. `BLOCKED.md` — dọn mục đã đóng (và mục **giữ nguyên**)

Đã **xoá** (đóng rõ ràng, có bằng chứng trong `QUEUE.md`): nợ **P20** (`KT-346` DONE, `VBREV_ALGO`
`kt134-v5`); **`.data+.bss` x86_64 96.320 B** (`KT-340b` DONE → 43.072 B); **chính sách
DHCP/ARP/ICMP/UDP/DNS** (`KT-335b` DONE — gộp vào `vifast-net`); **sản phẩm aarch64 chưa MMU/VSpace/
U-mode** (`KT-338b`/`KT-339a` DONE). Mục `KT-345a` ở §4 (chờ Agent 0) cũng đã đóng → sửa lại.

Đã **giữ** (còn hiệu lực/không chắc — ghi lý do): `CNode.Mutate/Rotate`, `CNode.Insert`,
`Tcb.Exec(path)`, `exec` ca lỗi-muộn/SMP thật, `ping`/`ifconfig` trong shell, **đọc DTB aarch64**,
**`Mailbox SMP-1`** (hoãn Chu kỳ 3 **có văn bản**), **`v2 mutants`** (Chu kỳ 3), số ca TAP hard-code.
Mục `x86_64: VSpace động + W^X toàn ảnh + TLB shootdown` **giữ** nhưng sửa nguồn: VSpace/TLB đã đóng
(`KT-340b`/`KT-340d`), **W^X toàn ảnh chưa xác nhận** ⇒ không xoá khi chưa đo.

## 6. Cổng (T0/T1) — log nguyên văn

- `./v2/bin/v2 check`: xem §6.0 (lượt 2 sau khi nối selftest vào cổng).
- `./v2/bin/v2 gate --dev`: **PASS** — `# gate-steps registered=76 ran=62 skipped=14 failed=0`;
  `TỔNG gate --dev (T1)` **797,0 s**, xem §6.1 (thời gian **không dùng để kết luận**: lúc chạy có
  worktree anh em `/private/tmp/kt347-s1` chạy `ktest.sh 1 matrix` ⇒ `qemu_foreign>0`, D2-116).
- `bash v2/test/boot/soak.sh --smoke 20 1`: xem §6.2.
- `bash v2/bin/v2 nightly --soak --profile product-shell --smoke`: xem §6.3.
- `bash v2/bin/v2 nightly --soak --profile virtio-blk --smoke`: xem §6.4.
- `bash v2/test/boot/budget-x86_64.sh --selftest`: **17/17**.
- 3 thư viện: **8/8**, **12/12**, **11/11**.

### 6.0 `v2 check`

```
== v2 check (HEAD e1875d53) ==
v2 check: PASS (L1 chạy trong gate --fast)
...
# step-time-total    86089 ms  TỔNG check
CHECK_RC=0
```

Trong đó: `check-selftest-wiring: PASS (36 script tự kiểm: 36 có cổng, 0 ngoại lệ, 0 mồ côi)`;
`v2 nightly --selftest: PASS (30/30)` (17 ca cũ + 13 ca mới, có 9 ca `--profile`/`--smoke`).

### 6.1 `gate --dev`

Lượt 1 (trước khi nối 3 selftest vào `selftests.sh`) **ĐỎ đúng 1 bước** — đúng lỗi cần chặn:

```
check-selftest-wiring: FAIL (3 script tự kiểm không cổng nào gọi: v2/test/boot/lib/budget-common.sh, v2/test/boot/lib/harness-scan.sh, v2/test/boot/lib/soak-common.sh)
--- cổng selftest có cổng gọi (KT-134): FAIL
v2 gate --dev: FAIL (T1 — kiểm tích hợp nhanh)
```

Lead duyệt (D2-254) thêm **đúng 3 dòng** `_t0_tool` vào `run_t0_tooling()`; lượt 2 **PASS**
nguyên văn:

```
# gate-steps registered=76 ran=62 skipped=14 failed=0
# step-time-total   797032 ms  TỔNG gate --dev (T1)
v2 gate --dev: PASS (T1 — kiểm tích hợp nhanh, **KHÔNG** phải full gate; full gate = 'gate --fast')
```

### 6.2 `soak.sh --smoke 20 1`

```
# soak: xong wall=15s (1 boot/s); ok=20 fail=0 tổng=20/20
# soak: BUILD_ID=aeb06d0cb78c8cc1 sha256=d6a8068afe70a0f3cbc1bac527390d82fb26cfb09f6c5f60a86fb038aa4f0f17 git=e1875d5382f21126125c54259111428e9da825dc
# soak: 20/20 lỗi=0 — 0/N chỉ là trần (QD §5.3); không retry (D2-07)
# soak: SMOKE N=20 < 1000 — KHÔNG phải bằng chứng cổng K3
SOAK_SMOKE_RC=1
```

(`# infra: invalid=2 … qemu_foreign=1` — worktree anh em đang chạy QEMU ⇒ **thời gian** không
dùng; 20/20 boot vẫn đúng bất biến. `--smoke` **luôn** exit 1 — hợp đồng cũ giữ nguyên.)

### 6.3 `nightly --soak --profile product-shell --smoke`

```
   phạm vi: --soak (profile=product-shell; N=1/1/1/1; runs=1)
   profile soak: product-shell — bước: soak-riscv64-product-shell soak-aarch64 soak-x86_64
--- nightly 1: PASS (74s; log …/step-1-soak.log)      # riscv64 --features product-shell
--- nightly 2: PASS (20s; log …/step-2-soak.log)      # aarch64
--- nightly 3: PASS (25s; log …/step-3-soak.log)      # x86_64
v2 nightly: PASS (3 bước; bằng chứng /private/tmp/kt347b-s1/target-task/nightly/nightly-20261123-223916-e1875d53; STATUS.md đã sinh)
NIGHTLY_PS2_RC=0
```

### 6.4 `nightly --soak --profile virtio-blk --smoke`

```
   phạm vi: --soak (profile=virtio-blk; N=1/1/1/1; runs=1)
   profile soak: virtio-blk — bước: soak-riscv64-virtio-blk
--- nightly 1: Soak riscv64 + đĩa thật virtio-blk ảnh sản phẩm (N=1 × 1 luồng; SMOKE …)
--- nightly 1: PASS (100s; log …/step-1-soak.log)
v2 nightly: PASS (1 bước; bằng chứng /private/tmp/kt347b-s1/target-task/nightly/nightly-20261123-224145-e1875d53; STATUS.md đã sinh)
NIGHTLY_VB_RC=0
```

## 7. Nợ / giới hạn

1. **Xác nhận chưa làm**: a64/x64 **chưa** soak product-shell thật (chưa có runner lái shell hai
   arch) — profile `product-shell` mới nối đúng nghĩa cho riscv64; a64/x64 dừng ở build+kiểm
   product-shell + soak ktest (hợp đồng cũ). Đề xuất: task riêng trước/trong `KT-348`.
2. `--smoke` là **lối tắt mới** của nightly (`--n 1 --runs 1`); nó **không** phải bằng chứng — in
   cảnh báo như `--n` thấp.
3. `harness-scan.sh` giữ **nguyên** hai công thức đếm khác nhau (occurrences vs dòng) thay vì hợp
   nhất, vì hợp nhất sẽ đổi số hit của arch khác — selftest khoá cả hai.
4. `soak-x86_64.sh` nay quét **23** tiền tố (trước 14) cho ảnh sản phẩm/ktest; đây là **siết cho
   khớp** bộ `budget-x86_64.sh` đã áp cùng ảnh (sản phẩm 0, ktest > 0) — không nới.
5. `budget.sh --selftest` nay gọi `budget_common_selftest` (12 ca dùng chung) thay vì 3 fixture
   riêng — phép kiểm **không mất** (biến thể, 300 KiB vượt trần, ngưỡng nhãn); số ca đổi.
6. Mọi phép đo QEMU trong journal chỉ là **smoke/selftest**, **không** phải bằng chứng nghiệm thu;
   soak N đầy thuộc `KT-348` trên máy đo độc quyền. Các lượt chạy của tôi trùng thời gian với
   worktree anh em `/private/tmp/kt347-s1` (`ktest.sh 1 matrix`) ⇒ `qemu_foreign>0`; **thời gian**
   trong log không dùng để kết luận (D2-116), chỉ dùng **kết quả đúng/đỏ**.
7. **Ngoại lệ D2-254 (lead duyệt)**: thêm **đúng 3 dòng** `_t0_tool` vào
   `v2/bin/lib/selftests.sh` (ngoài 3 tệp soak/nightly) để `check-selftest-wiring.py` có lời gọi
   thật cho 3 thư viện; không sửa dòng nào khác trong `v2/bin/**`. `git diff --stat
   v2/bin/lib/selftests.sh` = **1 tệp, +3 dòng**.
8. Lượt `nightly --soak --profile … --smoke` (chạy thật theo DoD) có ghi `STATUS.md` +
   `JOURNAL/nightly/<run-id>/`; đây là **smoke**, không phải bằng chứng nghiệm thu, nên tôi đã
   **khôi phục** `STATUS.md` về HEAD và **xoá** 3 thư mục journal smoke để cây sạch; log đầy đủ
   giữ ở `/tmp/kt347b-nightly-*.log` + `target-task/nightly/` (gitignored).
