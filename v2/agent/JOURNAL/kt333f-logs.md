# KT-333f — quyết định & dọn nốt raw `.log` **chưa theo dõi** (nối tiếp `KT-333e`)

**Task**: `KT-333f` (nợ ghi ở dòng `KT-333e` của `QUEUE.md`) · **Ngày**: 2026-11-23 (JST) ·
**Nhánh**: `task/kt333f-logs` (base `main` @ `b2f6e062`) · **Worktree**: `/tmp/kt333f-s1`
· **Quy chế**: D2-245/D2-248 — sub-agent **chỉ T0 + T1** (`v2 check`, `gate --dev`), **không**
`gate --fast`; `spec/`, 19 tệp `[[legacy]]`, `v2/agent/{STATE,QUEUE,DECISIONS}.md`, `v2/bin/**`,
`.github/**`, mã kernel/lib **không** sửa. Journal: **đúng một** tệp (tệp này).

| Mục | Yêu cầu | Kết quả |
|---|---|---|
| **1** | Đếm & phân loại `.log` chưa theo dõi (số, MB, thư mục, bao nhiêu tệp bị khối `scope=`/`red` trích) | ✅ §1 — **36** tệp / **126.380 B** (0,12 MiB) dưới `kt78-guard/`; **0** tệp nằm trong khối `scope=`/`scope-red=` |
| **2** | Chọn cách (A) tag thứ hai hay (B) xoá vì trùng `cycle1-raw-logs` | ✅ **(A)** — (B) **bất khả**: **0/36** tệp có trong tag cũ (theo đường dẫn **và** theo nội dung) |
| **3** | Thi hành + kiểm chứng | ✅ §3–§4 — tag `cycle1-raw-logs-untracked` (`9d7916db`, 36 tệp), xoá 36 tệp khỏi cây chính; `??` dưới `v2/agent/` **36 → 0**; 222 `.log` theo dõi **không đổi**; `scope=` **159 → 159** |
| **4** | Lệnh khôi phục (journal + `v2/agent/archive/`) | ✅ §3.3 + `v2/agent/archive/README.md` |
| **5** | Commit | ✅ §5 |

---

## 0. TL;DR và một **đính chính số liệu** quan trọng

Con số **5.883** trong `STATE.md`/`QUEUE.md` **không phải** số tệp `??`. Đo lại trên cây chính
(`/Users/mps/dvchd/ViFastOs`, `main` @ `b2f6e062`, 2026-11-23 04:45 JST):

- `find v2/agent/JOURNAL -name '*.log' | wc -l` = **5.883** ← **tổng tệp `.log` trên đĩa**;
- `git status --porcelain | grep '^??' | wc -l` = **57 dòng** (cả repo), trong đó **37 dòng `.log`**
  (**40 tệp `.log`** khi `-uall`, vì `review/video/` bị gộp thành một dòng);
- **trong worktree của task này** (`/tmp/kt333f-s1`): **0** tệp chưa theo dõi — xem §1.3.

Vậy "5.883" là **tổng trên đĩa** (gồm 222 tệp **đang theo dõi**, 36 tệp `??`, và 5.625 tệp **đã
bị `.gitignore` phủ**), không phải 5.883 tệp chưa theo dõi. Ghi rõ để lần sau không lặp lại phép
đếm sai này (đúng tinh thần QD §2.2: số phải nói rõ **đo cái gì**).

| Lớp (dưới `v2/agent/JOURNAL/`) | Số tệp | Bytes | MiB | Vào `git status`? |
|---|---:|---:|---:|---|
| **Đang theo dõi** (`git ls-files`) | **222** | 2.543.133 | 2,43 | không (đã commit) |
| **Chưa theo dõi, không ignored** (`??`) | **36** | 126.380 | 0,12 | **CÓ** — đây là phần "ồn" duy nhất |
| **Chưa theo dõi + đã ignored** (`run-*/`, `soak/`, `parity/`, `persist/`, `ktest-matrix/`) | **5.625** | 26.294.855 | 25,08 | không (ignored) |
| **Tổng trên đĩa** | **5.883** | 28.964.368 | 27,62 | — |

Ngoài `v2/agent/JOURNAL/` còn **4** tệp `.log` `??` khác: `out-b.log` ở gốc repo (**221 B**,
2026-11-14) + **3** tệp dưới `review/video/build/longform/` (2026-11-14) — **không** thuộc phạm vi
nhiệm vụ này (không phải bằng chứng ViFastOs; là sản phẩm của pipeline video), xem §6.3.

---

## 1. Đếm & phân loại

### 1.1 Cây chính (nơi thực sự có tệp) — trước khi làm

Lệnh đo (chạy trong `/Users/mps/dvchd/ViFastOs`):

```bash
find v2/agent/JOURNAL -name '*.log' | wc -l                      # 5883
git ls-files v2/agent/JOURNAL | grep -c '\.log$'                  # 222
git ls-files -o --exclude-standard | grep -c '^v2/agent/JOURNAL.*\.log$'   # 36
comm -23 <(find ... | sort) <(git ls-files ... + untracked | sort) | wc -l # 5625 (ignored)
```

- **36 tệp `??`**: **100 %** ở `v2/agent/JOURNAL/kt78-guard/`, tên `selftest-<UTC>.log`
  (**19** tệp ngày 2026-11-21, **17** tệp ngày 2026-11-22), kích thước **3.484–3.680 B**
  (tổng **126.380 B**, trung bình **3.511 B**). Đây là **transcript của bước T2** *"selftest qemu
  guard (KT-78)"*.
- **5.625 tệp ignored** (25,08 MiB) nằm trong các thư mục bị `.gitignore` phủ (`.gitignore`
  dòng 19–25): 4 thư mục `run-soak-*`/`soak/run-*` × **1.000** tệp (≈ 24,0 MB / 22,9 MiB) + `persist/`
  (108) + `parity/` (78) + `ktest-matrix/` (52) + lẻ. **Không** gây ồn `git status`.
- **222 tệp theo dõi**: giữ nguyên (KT-333e §3.2), không đụng.

### 1.2 Bao nhiêu tệp bị khối `scope=`/`scope-red=` trích? → **0**

Đo lại đúng trên cây này (`b2f6e062`), bằng chính bộ phân tích của `verify-numbers.py`
(`FENCE = ^```text[ \t]*scope(-red|-pending)?=`, 43 tài liệu đích `v2/agent/reports/*.md` +
`v2/docs/k6-*.md`):

| Chỉ số | Số |
|---|---:|
| Khối `scope=` / `scope-red=` | **159** (145 + 14) — khớp `KT-333e` |
| Tên `scope` phân biệt | **124** |
| Phân giải được (đường dẫn gốc repo hoặc tương đối `v2/agent/JOURNAL/`) | **124/124** (**119** `.log` + 5 tệp khác), **0** thiếu |
| **Giao với 36 tệp `??`** | **0** (so tên tệp và so cả 124 tên `scope`) |
| Giao với 5.625 tệp ignored | **0** |

⇒ **Không** tệp nào trong tập xoá bị khối `scope=`/`red` trích. Hai tên `scope` thuộc `kt78-guard`
là `kt78-guard/selftest-guard-default.log` + `kt78-guard/selftest-guard-override0.log` — **cả hai
đang được theo dõi và được giữ nguyên**, không nằm trong tập xoá.

### 1.3 Vì sao worktree `/tmp/kt333f-s1` có **0** tệp chưa theo dõi

`git worktree add` chỉ **materialise tệp đã theo dõi**; tệp chưa theo dõi/ignored là **thuộc từng
working tree** và nằm lại ở cây chính. Đo trong worktree: **222** `.log`, **100 %** đang theo dõi,
`git status --porcelain` **rỗng** ngay khi nhận task. Vì vậy §2–§4 **phải** thi hành trên **cây
chính** (`/Users/mps/dvchd/ViFastOs`) — nơi 36 tệp thực sự tồn tại. Tag là **ref cấp repo** (dùng
chung `.git`), nên khôi phục được từ **bất kỳ** worktree nào.

---

## 2. Chọn cách: **(A)** — tag thứ hai, rồi xoá

### 2.1 Vì sao **không** dùng (B) "xoá vì đã có trong `cycle1-raw-logs`"

Điều kiện của (B): **từng** tệp phải đã có trong tag cũ (so `sha256` với blob của tag). Đo:

| Phép so | Kết quả |
|---|---|
| Theo **đường dẫn**: `git ls-tree -r cycle1-raw-logs` (7.546 tên) ∩ 36 tên | **0/36** |
| Theo **nội dung**: 36 git-blob hash vs **6.930** hash phân biệt của tag | **0/36** |
| Kiểm lại bằng `sha256` tệp ↔ `git cat-file blob` của tag (36 lần) | **0/36** khớp |

⇒ **36/36 tệp KHÔNG có trong tag cũ** (chúng sinh **sau** `KT-333e`, và chưa từng được theo dõi
Git) ⇒ theo đúng luật của task, **bắt buộc (A)**. `BẢNG: trùng 0 / không trùng 36`.

### 2.2 Vì sao **không** xoá thẳng (không tag)

36 tệp là **bản ghi duy nhất** của 36 lượt chạy bước T2 `selftest qemu guard` (19 lượt ngày
11-21, 17 lượt ngày 11-22), không tệp nào được tham chiếu ở nơi khác (0 khối `scope=`, 0 liên kết
trong tài liệu, 0 tên trong `counters.toml`/`numbers-policy.toml`). Xoá thẳng là mất bằng chứng
**không khôi phục được** — trái D2-230(1) ("không xoá lịch sử `JOURNAL/` cũ khi chưa có quyết
định riêng"). Tag giữ **byte-identical** với giá ~126 KB object.

### 2.3 Vì sao **không** đưa 5.625 tệp ignored vào tag

D2-230(1) **cấm commit raw `boot-*.log`** của soak (chỉ commit tóm tắt). Chúng đã bị `.gitignore`
phủ nên **không** làm `git status` ồn — đúng định nghĩa "phần còn lại cần dọn" của task này
(ồn + pre-flight cảnh báo). Đưa vào tag sẽ **vi phạm D2-230**; xoá sẽ **vi phạm D2-230(1)**.
⇒ **giữ nguyên**, ghi rõ trong `v2/agent/archive/README.md` §3.

---

## 3. Thực thi

### 3.1 Tag `cycle1-raw-logs-untracked` (annotated, **root commit** — không nối lịch sử `main`)

Dựng bằng plumbing (hash-object + index tạm + write-tree + commit-tree, **không** đụng index/nhánh
của worktree, **không** `git add -A`):

| Thành phần | Giá trị |
|---|---|
| Tag | **`cycle1-raw-logs-untracked`** (annotated) |
| Commit | **`9d7916db71b4959b3305da3576b853a63425281d`** (root: `git rev-list --parents -n1` chỉ có 1 từ) |
| Tree | `71f58963e0738a6c2c273fc9c0fb470defe063e6` |
| Số tệp | **36** (`git ls-tree -r --name-only \| wc -l`) |
| Kích thước | **126.380 B** |
| Tập tên | `diff` với danh sách xoá ⇒ **RỖNG (byte-set-equal)** |
| Tệp trùng tên với `main` | **0/36** (`git cat-file -e main:<path>` đều fail) |
| `sha256` blob trong tag ↔ tệp trên đĩa (trước khi xoá) | **36/36 khớp** |

Bảng 36 tệp (tên + byte + `sha256` 12 hex đầu; `sha256` đầy đủ sinh lại bằng
`git ls-tree -r cycle1-raw-logs-untracked` + `git cat-file blob`):

| Tệp trong `v2/agent/JOURNAL/kt78-guard/` | Byte | `sha256` |
|---|---:|---|
| `selftest-20261121T030552Z.log` | 3500 | `1cfc67f8ed43…` |
| `selftest-20261121T040637Z.log` | 3500 | `e6adaaf5b53b…` |
| `selftest-20261121T050650Z.log` | 3486 | `3b4ca662e138…` |
| `selftest-20261121T062952Z.log` | 3492 | `2e6cc198e194…` |
| `selftest-20261121T073005Z.log` | 3498 | `8b0d1ec9f9b5…` |
| `selftest-20261121T074341Z.log` | 3498 | `bb0d3a1fb4a1…` |
| `selftest-20261121T101819Z.log` | 3528 | `3e2ffc6ff0c9…` |
| `selftest-20261121T103254Z.log` | 3536 | `5d5c7a5abfd0…` |
| `selftest-20261121T104709Z.log` | 3484 | `fc34eb20278f…` |
| `selftest-20261121T114405Z.log` | 3540 | `4cf2b5bdfd4d…` |
| `selftest-20261121T115916Z.log` | 3538 | `2f0b9e51ed6b…` |
| `selftest-20261121T122158Z.log` | 3536 | `d7d5bf8d0e53…` |
| `selftest-20261121T131440Z.log` | 3526 | `4ab3dd1d4de8…` |
| `selftest-20261121T133052Z.log` | 3526 | `0dcac2b6e5c5…` |
| `selftest-20261121T134455Z.log` | 3524 | `a7f1f1b4c1f0…` |
| `selftest-20261121T150638Z.log` | 3524 | `35da7ba1a5f7…` |
| `selftest-20261121T152124Z.log` | 3524 | `a4b8b0ea4bd0…` |
| `selftest-20261121T165910Z.log` | 3512 | `16ddb5e5e8b6…` |
| `selftest-20261121T183723Z.log` | 3512 | `c9d5e6ba8b1a…` |
| `selftest-20261122T004643Z.log` | 3522 | `37a4d6a6a44f…` |
| `selftest-20261122T022154Z.log` | 3516 | `f4c6a9a3f1d7…` |
| `selftest-20261122T025416Z.log` | 3516 | `edd2a5bf6c25…` |
| `selftest-20261122T042857Z.log` | 3508 | `d6b59b8a1e1b…` |
| `selftest-20261122T063736Z.log` | 3526 | `6a1d5f2d0f6c…` |
| `selftest-20261122T065020Z.log` | 3526 | `5f8b7a8f8d6b…` |
| `selftest-20261122T075331Z.log` | 3520 | `d4a1f8dd1db4…` |
| `selftest-20261122T080130Z.log` | 3520 | `c7b1ec5a0e98…` |
| `selftest-20261122T081103Z.log` | 3520 | `9f2d4ae7f1a4…` |
| `selftest-20261122T090844Z.log` | 3542 | `bb0c9d6c5dbe…` |
| `selftest-20261122T110636Z.log` | 3492 | `aa89e0d7effc…` |
| `selftest-20261122T122757Z.log` | 3502 | `ba10d2e6d60f…` |
| `selftest-20261122T124242Z.log` | 3496 | `c2f1a8a0e8c6…` |
| `selftest-20261122T125332Z.log` | 3496 | `0b8bd9c2ad3a…` |
| `selftest-20261122T130512Z.log` | 3488 | `7fdc8b43b79d…` |
| `selftest-20261122T170125Z.log` | 3520 | `2d0e4c7c5e2c…` |
| `selftest-20261122T174610Z.log` | 3502 | `a610c71ce7d6…` |

### 3.2 Xoá khỏi cây chính (chỉ 36 tệp; có 4 lớp kiểm an toàn)

Với **từng** tệp: (i) đường dẫn tuyệt đối phải nằm trong
`/Users/mps/dvchd/ViFastOs/v2/agent/JOURNAL/kt78-guard/` và kết thúc `.log`; (ii) phải là tệp
thường; (iii) `git ls-files --error-unmatch` phải **fail** (vẫn chưa theo dõi); (iv) `sha256`
**phải trùng** giá trị đã ghi trong tag (chống trường hợp tệp bị ghi đè giữa lúc tạo tag và lúc
xoá). Kết quả: `deleted=36 skipped=0`. Trước khi xoá: `measure_lock` **TRỐNG**, không có
`guard_selftest.sh`/gate nào đang chạy (chỉ 1 sub-agent khác đang sửa `soak-x86_64.sh` trong
worktree riêng).

### 3.3 Lệnh khôi phục

```bash
git checkout cycle1-raw-logs-untracked -- v2/agent/JOURNAL/kt78-guard/     # cả 36 tệp
git checkout cycle1-raw-logs-untracked -- v2/agent/JOURNAL/kt78-guard/selftest-20261121T030552Z.log
git ls-tree -r --name-only cycle1-raw-logs-untracked                      # liệt kê
git cat-file blob cycle1-raw-logs-untracked:<path> | shasum -a 256        # kiểm nội dung
```

Ghi **cùng** các lệnh này trong `v2/agent/archive/README.md` §2 (chỉ mục kho lưu trữ, cùng chỗ
với `reports/README.md` của `KT-333e`).

---

## 4. Kiểm chứng

### 4.1 Hiện trạng sau khi dọn

| Phép kiểm | Trước | Sau |
|---|---:|---:|
| `git status --porcelain \| grep -c '^??'` (**cây chính**) | **57** | **21** |
| `git status --porcelain \| grep -c '^??.*\.log$'` (cây chính, `-uall`: 40 tệp) | **37 dòng / 40 tệp** | **4 tệp** — `out-b.log` + 3 tệp `review/video/**` (**ngoài** phạm vi, §6.3) |
| `??` dưới `v2/agent/` (cây chính) | **36** | **0** ✅ |
| `.log` **chưa theo dõi** dưới `v2/agent/JOURNAL/` | **36** | **0** ✅ |
| `.log` **trên đĩa** dưới `v2/agent/JOURNAL/` | **5.883** | **5.847** (−36) |
| `.log` **đang theo dõi** (`git ls-files v2/agent/JOURNAL \| grep -c '\.log$'`) | **222** | **222** ✅ |
| Khối `scope=`/`scope-red=` (43 tài liệu đích) | **159** | **159** ✅ |
| `git status --porcelain` trong **worktree** `/tmp/kt333f-s1` | **rỗng** (0 tệp) | **rỗng** sau commit ✅ |

```console
$ cd /Users/mps/dvchd/ViFastOs && git status --porcelain | grep '^??' | grep '^?? v2/agent/' | wc -l
      0
$ git ls-files v2/agent/JOURNAL | grep -c '\.log$'
    222
$ grep -rhn '^```text[[:space:]]*scope' v2/agent/reports/*.md v2/docs/k6-*.md | wc -l
    159
$ git -C /tmp/kt333f-s1 status --porcelain | wc -l
       0          # sau commit §5
```

**Kiểm khôi phục từ tag** (chạy thật, không chỉ lý thuyết):

```console
$ git archive cycle1-raw-logs-untracked | tar -x -C /tmp/kt333f-restore
$ find /tmp/kt333f-restore -type f | wc -l
      36
$ # so sha256 từng tệp với giá trị đã ghi trước khi xoá
restore_sha256_ok=36 bad=0
```

### 4.2 Cổng (T0/T1, đúng D2-245)

| Bước | Lệnh | Kết quả (nguyên văn) |
|---|---|---|
| T0 | `./v2/bin/v2 check` | **PASS** (exit 0): `# gate-steps registered=44 ran=44 skipped=0 failed=0` — `# gate-wall 179209 ms`, `# gate-cpu user=287.670 s sys=84.375 s total=372.045 s`. Trong log có **26** dòng chứa `FAIL` và **111** dòng `ĐỎ` nhưng **tất cả** nằm trong **ca âm tự kiểm** (kỳ vọng ĐỎ: `# selftest 3d new-cmds-ok (sai): ĐÚNG (2 PASS + 3 FAIL, mã thoát 1)`, `ok - … vẫn ĐỎ`), **không** phải bước hỏng |
| Rules | `./v2/bin/v2 check-rules` | **PASS** (exit 0): `# verify-numbers: ok=5 fail=0 wall_ms=27159 law_wall_ms={1:1059,2:8952,3:449,7:149,8:16279}`; **26** dòng `ok - `; `ok - luat 2: pairs=127 pending=0`; `ok - luat 3: counters=2 used=2`; `ok - luat 8: cases=21` |
| T1 | `./v2/bin/v2 gate --dev` | **PASS** (exit 0): `v2 gate --dev: PASS (T1 — kiểm tích hợp nhanh, **KHÔNG** phải full gate; full gate = 'gate --fast')`; `# gate-steps registered=76 ran=62 skipped=14 failed=0`; đúng **14** dòng `--- …: BỎ QUA (T1 — chỉ chạy ở T2)`; `# gate-wall 907372 ms (907.4 s)`; `# gate-cpu user=1160.809 s sys=314.840 s total=1475.649 s` |
| Tham chiếu | `python3 v2/tools/verify-numbers.py --root .` (đo riêng, trước khi sửa gì) | **ok=5 fail=0** (`wall_ms=76005`, `law_wall_ms={1:3180,2:34217,3:1152,7:824,8:35856}`) |
| Tham chiếu | `python3 v2/tools/check-scope-table.py --root .` | **PASS** (bỏ qua **1**/21 câu = mốc 1, không tăng) |

---

## 5. Commit

`VIFAST_ROLE=lead git commit -m "[KT-333f] …"` — nội dung **đúng 2 tệp**: tệp journal này +
`v2/agent/archive/README.md` (`git add <đường-dẫn-cụ-thể>`, **không** `git add -A`). **Không** tệp
mã nào bị đổi; `main` không có tệp nào bị xoá/thêm (việc dọn là **tệp chưa theo dõi trên đĩa** +
một **ref tag** cấp repo). Hash của commit không chép vào chính tệp này (sửa nội dung tệp sẽ đổi
hash); lấy bằng `git log -1 --format=%H` trên nhánh `task/kt333f-logs` — base `b2f6e062`, commit
task là commit **kế tiếp**.

---

## 6. Nguyên nhân gốc & nợ mở (không giấu)

### 6.1 36 tệp này **sẽ sinh lại** ở mỗi lượt T2 — dọn một lần là chưa đủ

Chuỗi sinh: `v2/bin/lib/gate.sh:125` `lane_step sau2 45 "selftest qemu guard (KT-78)" step_guard`
→ `v2/bin/lib/steps_gate.sh:26` `bash "$V2_ROOT/test/boot/guard_selftest.sh"`
→ `v2/test/boot/guard_selftest.sh:31` `LOGDIR="${KT78_LOGDIR:-$V2_ROOT/agent/JOURNAL/kt78-guard}"`
→ `:80` `WORK="$LOGDIR/run-$STAMP"` (ignored bởi `.gitignore:21` `run-*/`) và
`:82` `TRANSCRIPT="$LOGDIR/selftest-$STAMP.log"` (**chưa** ignored).

⇒ **Mỗi lượt T2 = +1 tệp `??`** trong cây nơi T2 chạy (lead chạy T2 ở cây chính ⇒ cây chính lại
ồn 1 dòng; `KT-333f` không thể chặn từ đây). T1 **không** sinh tệp này vì `"selftest qemu guard
(KT-78)"` nằm trong `V2_T2_ONLY_STEPS` (`v2/bin/lib/consts.sh:16`) — đúng như đo: sau `gate --dev`
của task này, worktree vẫn **0** tệp `??`.

**Đề xuất (ngoài quyền T0/T1 của task này, cần lead/T2 xác nhận)**: cho bước 45 dùng `mktemp -d`
theo PID như D2-248 mục 2(b) đã làm cho 7 runner còn lại — tức truyền `--logdir` hoặc
`KT78_LOGDIR` trỏ ra ngoài repo (hoặc vào `target-task/` đã ignored). **An toàn đã kiểm**:
`v2/test/rules/counters.toml` + `v2/test/rules/counter-evidence.sh` chỉ trỏ tới tệp **lịch sử đang
theo dõi** `kt78-guard/selftest-20261117T015708Z.log` (giữ nguyên), nên log **mới** chuyển chỗ
không đụng ghim nào. Không tự sửa vì: (i) `guard_selftest.sh` là runner **chỉ T2 chạy** — sửa mà
không chạy T2 là bằng chứng rỗng; (ii) ngoài phạm vi 2 lựa chọn (A)/(B) của task.

### 6.2 5.625 tệp ignored (25,08 MiB) — giữ, cần quyết định riêng nếu muốn lấy lại đĩa

D2-230(1): **không xoá lịch sử `JOURNAL/` cũ khi chưa có quyết định riêng**. Nếu lead muốn lấy
lại ~24 MiB thì phạm vi rõ ràng: 4 thư mục `run-soak-*`/`soak/run-*` × 1.000 `boot-*.log`
(≈ 24,0 MB / 22,9 MiB), **sau khi** xác nhận `summary.txt`/`build-inputs`/`sha256` của các lượt soak đó đã
được commit (đúng D2-230). Không làm trong task này.

### 6.3 4 tệp `.log` `??` còn lại — ngoài phạm vi, **không** xoá

`out-b.log` (gốc repo, 221 B, 2026-11-14) và `review/video/build/longform/{out-a,out-b,phan-tich-demo-viscript}.log`
(2026-11-14). Không phải log của `JOURNAL/`, không phải bằng chứng cổng, thuộc pipeline video
(`review/video/`). Xoá là quyết định của lead/chủ pipeline video, **không** phải của task này —
nêu ra để `??` không còn tệp `.log` nào "vô chủ" mà không ai biết.

### 6.4 Hạn chế của chính task

- Việc dọn **thi hành trên cây chính** (`/Users/mps/dvchd/ViFastOs`), không phải trong worktree
  `/tmp/kt333f-s1` — vì worktree mới checkout **không** chứa tệp chưa theo dõi (§1.3). Thay đổi
  duy nhất ở cây chính: **xoá 36 tệp chưa theo dõi** (khôi phục được bằng tag); **không** đụng
  `HEAD`, chỉ mục, hay bất kỳ tệp đang theo dõi nào của cây đó.
- Worktree nhận task được tạo từ `main` @ `b1b5d6a0` (chậm 2 commit); trước khi đo đã
  **fast-forward** `task/kt333f-logs` lên đúng base giao `b2f6e062` (`git merge --ff-only main`,
  không có commit riêng nào bị viết lại), rồi mới chạy T0/T1.
- Tag là **ref cục bộ** (chưa push): lead cần `git push origin cycle1-raw-logs-untracked` (và
  `cycle1-raw-logs` nếu chưa) để lưu trữ lên remote.
- Không sửa: `spec/`, 19 tệp `[[legacy]]`, `v2/agent/{STATE,QUEUE,DECISIONS}.md`, `v2/bin/**`,
  `.github/**`, mã kernel/lib, `v2/test/kernel/check-wake-paths.py` (không đụng tới).
