# KT-333d — Mô-đun hoá `v2/bin/v2` · khoá 2 lớp T0/T1/T2 · `boot/lib/env.sh` · `wt.sh` 4 slot · `v2 t0` · `agentctl lint/sync-queue` · `soak-x86_64-max` (D2-245/D2-248)

- **Task**: `KT-333d` (Chu kỳ 2, Đợt 2, P1, ≤ 3 h) — ngoài `v2/kernel/**`, `v2/lib/**`, `spec/`.
- **Nhánh**: `task/kt333d-v2lib`, base `main` @ `3a8219ac`.
- **Ngày**: 2026-11-23 (JST).
- **Quyết định**: **D2-245** (tối ưu quy trình Agent 2, `v2/bin/v2` phình 2.185 dòng),
  **D2-248** (khoá 2 lớp, `wt.sh` cache ấm, `v2 t0`, `boot/lib/env.sh`, `agentctl lint`), ràng buộc
  **D2-235(5)** (0 id `lane_step` trùng), **D2-241** (không CI GitHub), **D2-247** (DAG/mốc).
- **Ranh giới đã giữ**: **không** sửa `spec/`, **không** sửa 19 tệp `[[legacy]]`, **không** sửa
  `v2/agent/{STATE,QUEUE,DECISIONS}.md`, **không** `.github/**`; **không** nới ngưỡng; **không**
  `#[ignore]`/retry; `git add` theo đường dẫn cụ thể. Chỉ chạy **T0 + T1** (`gate --dev`) theo
  D2-245/D2-248; T2 do lead chạy.

## 0. Bảng tổng hợp (việc · bằng chứng · rủi ro)

| # | Việc (DoD) | Bằng chứng | Rủi ro còn lại |
|---|---|---|---|
| A | Tách `v2/bin/v2` (2.185 → **161** dòng) → `v2/bin/lib/*.sh`; cùng hành vi/số bước | `# gate-steps registered=45 ran=45 skipped=0 failed=0` **trước = sau** (nguyên văn §1.2); `CHECK_STEPS_EXPECTED=45`, `GATE_FAST_EXTRA_EXPECTED=31`, `T2_ONLY_STEPS_EXPECTED=14`; 0 id trùng (77 `lane_step`: 45 + 31 + 1 T1 smoke); `v2 check` PASS; `gate --dev` PASS | Đã sửa 2 checker phụ thuộc **vị trí** lời gọi (`check-selftest-wiring.py`, `selfbreak.sh`) — lý do §1.3 |
| B | Khoá 2 lớp: T2/nightly/soak EXCLUSIVE; **bỏ** khoá độc quyền ở T1; `measure_lock_is_held` cho T0 (cảnh báo) + pre-flight T2 `< 1 s` | `measure-lock.sh --selftest` **13/13** (4 ca `is_held`: giữ / trống / stale / chủ khác còn sống); `do_gate_dev` gọi `v2_lock_t1_guard`; `do_gate_fast` chỉ giữ khoá khi `V2_TIER != t1` | Pre-flight T2 **từ chối cây bẩn** — lead phải commit trước khi T2 (đúng D2-248); T1 của **mã cũ** (chưa merge) vẫn giữ khoá ⇒ T1 mới **chạy song song** khi chủ khoá là T1 (nhãn `gate --dev`), chỉ **nhường** khi chủ khoá là T2/nightly/soak (§7.2) |
| C | `v2/test/boot/lib/env.sh` cho **7 runner** (`LOGDIR`) + **3 `SOCK`** ngắn `< 104 B` | 7/7 runner `source`; `--print-logdir` 7/7 chạy với `TMPDIR` dài **125** byte; 3 `--print-sock` ra `/tmp/v2s-*` (29–65 byte); `env.sh --selfcheck` **8/8** (gồm bind AF_UNIX thật) | Không đổi biến ép cũ (`KT_LOGDIR` thắng, `KT257_LOGDIR`/`KT250_LOGDIR`/`KT337_LOGDIR`/`SOAK_*`, `KT75_LOGDIR`, `nopid`) — bảng §3.1 |
| D | `v2/tools/wt.sh` 4 slot + `target-task/slot-<n>` (cache ấm) + `df ≥ 5 GiB` | `wt.sh --selftest` **11/11** (cấp/xoá/đầy slot/df thấp/branch trùng/nhánh sai/list/prune/nhánh sẵn có) | Release **giữ** target dir (đúng thiết kế) ⇒ cần `wt.sh prune` khi đĩa thấp |
| E | `v2 t0` (T0 nhanh, in rõ không phải cổng) + `vifast-agentctl lint`/`sync-queue` | `v2 t0 --selftest` **11/11**; `agentctl lint --selftest` **12/12**; `sync-queue --selftest` **7/7**; `lint` trên sổ thật: **PASS 0 lỗi, 14 cảnh báo**; `sync-queue --dry-run`: `(104 task)` → `(106 task)` | Lệch `STATE.md` mặc định là **cảnh báo**, không đỏ — lý do §5.3 (STATE thuộc Agent 0, sub-agent bị cấm sửa); `--strict-state` để lead bật ĐỎ |
| F | `soak-x86_64-max` (`V2_X64_CPU=max`, 600 × 3) trong **`nightly --soak`** (không vào T2) | `v2 nightly --soak --dry-run` in `V2_X64_CPU=max bash v2/test/boot/soak-x86_64.sh 600 3`; `nightly --selftest` **20/20** (thêm 1 ca) | Không nhân đôi T2: không có `lane_step` nào gọi soak (§6.2) |

## 1. A — Tách `v2/bin/v2` → `v2/bin/lib/*.sh`

### 1.1 Bố cục (tự điều chỉnh so với đề xuất — lý do ngay dưới)

| Tệp | Dòng | Nội dung |
|---|---:|---|
| `v2/bin/v2` | **161** | dispatcher: gốc repo, nạp `measure-lock.sh` + 14 mảnh lib, `case` gọi **một** hàm/lệnh. Không còn thân lệnh. |
| `v2/bin/lib/consts.sh` | 24 | `V2_T2_ONLY_STEPS`, `CHECK_STEPS_EXPECTED=45`, `GATE_FAST_EXTRA_EXPECTED=31`, `T2_ONLY_STEPS_EXPECTED=14`, `SCAN_DIRS`/`R1_DIRS` |
| `v2/bin/lib/util.sh` | 299 | tìm cargo/rustup/python, ghim toolchain, `now_ms`/`step_time`/`run_step`, `rules` 5 luật + E1 |
| `v2/bin/lib/lock.sh` | 69 | `measure_lock_enter`/`leave_all` (KT-333c) + **lớp 2**: `v2_lock_t1_guard`, `v2_lock_t0_warn` |
| `v2/bin/lib/lane.sh` | 325 | `lane_step`/`lane_run`/`lane_fails`/`lane_failed`/`lane_count`, `.rc`/`.ms`/`.skip`, `lane_report`, `gate_summary` |
| `v2/bin/lib/steps_check.sh` | 100 | 40 hàm bước của `v2 check` + `step_py` + `check_k6_rules` |
| `v2/bin/lib/steps_gate.sh` | 239 | 31 hàm bước T2 + `l0_test_parallel` + `step_qemu_smoke` |
| `v2/bin/lib/steps_nightly.sh` | 169 | Miri + `run_nightly_l1_driver`/`run_nightly_miri` |
| `v2/bin/lib/steps_soak.sh` | 130 | `nightly_soak_*`, `nightly_step_exec`, **`soak-x86_64-max`** |
| `v2/bin/lib/nightly.sh` | 568 | `v2 nightly` (plan/run/status/selftest/parse) — giữ nguyên chú thích thiết kế |
| `v2/bin/lib/check.sh` | 86 | `do_check` (45 bước) |
| `v2/bin/lib/gate.sh` | 156 | `do_gate_dev` (T1) + `do_gate_fast` (T2) + **pre-flight** |
| `v2/bin/lib/t0.sh` | 298 | **`v2 t0`** (suy crate từ diff + fmt/clippy/test) + `t0 --selftest` |
| `v2/bin/lib/selftests.sh` | 40 | `run_t0_tooling` (measure-lock, wt.sh, env.sh, agentctl lint/sync-queue) |
| `v2/bin/lib/help.sh` | 59 | `usage()` + mục `t0` |

**Tự điều chỉnh (ghi lý do theo yêu cầu)**:
1. Đề xuất có `lib/check.sh`/`lib/gate.sh` trong danh sách của QUEUE nhưng bản giao cho sub-agent
   không nêu; tách `do_check`/`do_gate_*` khỏi `lane.sh` để `lane.sh` thuần cơ chế làn (325 dòng)
   — dễ review độc lập.
2. Thêm `lib/t0.sh` + `lib/selftests.sh` (mục E cần 2 tệp mới; nếu nhét vào `util.sh` thì lại
   phình một tệp — trái mục tiêu D2-245).
3. `nightly.sh` (568 dòng) tách khỏi `steps_nightly.sh` (Miri) và `steps_soak.sh` (soak) — đúng
   tinh thần "mỗi tệp một mối quan tâm"; tổng dòng không đổi (dời, không viết lại).

### 1.2 Bất biến "cùng hành vi, cùng số bước" — bằng chứng nguyên văn

**Trước** khi tách (`./v2/bin/v2 check`, lưu `/tmp/kt333d-baseline-check.keep.log`):

```text
v2 check: PASS (L1 chạy trong gate --fast)
# gate-steps registered=45 ran=45 skipped=0 failed=0
```

**Sau** khi tách (toàn bộ gói A–F, `./v2/bin/v2 check`):

```text
v2 check: PASS (L1 chạy trong gate --fast)
# gate-steps registered=45 ran=45 skipped=0 failed=0
```

`diff` của hai dòng trên: **IDENTICAL** (0 khác biệt). Ba hằng số (nguyên văn từ `lib/consts.sh`):

```text
CHECK_STEPS_EXPECTED=45        # `check` (id 1..40 + 72 KT-331 + 73/74/75 KT-333b + 78 KT-333c)
GATE_FAST_EXTRA_EXPECTED=31    # `gate --fast` thêm 31 bước (id 41..64, 66, 67, 69, 71, 76, 77, 79)
T2_ONLY_STEPS_EXPECTED=14      # số bước T1 bỏ (khớp `V2_T2_ONLY_STEPS`)
```

Id `lane_step` sau khi tách: `1 2 3 … 64 66 67 69 70 71 72 73 74 75 76 77 78 79`
(= 45 bước `check` + 31 bước `gate --fast` + 1 bước T1 smoke id 70 = **77**), **0 id trùng**
(`sort | uniq -d` rỗng). Bước 65/68 không tồn tại trong bản gốc — giữ nguyên, không "lấp" số.

### 1.3 Vì sao phải sửa 2 checker (và vì sao đó **không** phải nới ngưỡng)

| Tệp | Sửa | Lý do |
|---|---|---|
| `v2/tools/check-selftest-wiring.py` | `tep_cong()` thêm `v2/bin/lib/*.sh`; **bỏ `v2/bin/lib` khỏi tập ứng viên** (`la_manh_cong`) | Sau khi tách, **mọi** lời gọi `--selftest` thật nằm trong `lib/*.sh`; nếu không quét chúng thì cả 32 script tự kiểm thành `E_ORPHAN` **oan**. Ngược lại, không được coi `lib/*.sh` là "script chạy độc lập" (chúng là mảnh `source`, có cả định nghĩa `*selftest` lẫn lời gọi của script khác) — nếu không mỗi mảnh thành 1 `E_ORPHAN` giả. `--selftest` của checker: **9/9**. |
| `v2/test/rules/selfbreak.sh` | `hash_real_tree()` băm thêm `v2/bin/lib/*.sh` | Phép thử "self-break **không** sửa cây thật" phải băm đúng nơi thân lệnh đang nằm; để nguyên thì nó chỉ bảo vệ dispatcher 161 dòng và **bỏ sót** 14 mảnh lib. `selfbreak.sh` chạy tay: **5/5 ca phá ĐỎ đúng mã; md5 cây thật KHÔNG đổi**. |

Không đổi ngưỡng nào; không bỏ ca; không `#[ignore]`.

## 2. B — Khoá 2 lớp (D2-248 mục 1)

### 2.1 Trước → sau

| Chỗ | Trước (KT-333c) | Sau (KT-333d) |
|---|---|---|
| `do_gate_fast` (T2) | `measure_lock_enter` **vô điều kiện** | `V2_TIER != t1` ⇒ **pre-flight** (`dirty`/`df`) rồi `measure_lock_enter`; `V2_TIER=t1` ⇒ không khoá |
| `do_gate_dev` (T1) | `measure_lock_enter "gate --dev (T1)"` | **Không** giành khoá: `v2_lock_t1_guard` — `measure_lock_is_held` + **nhãn**: chủ khoá là **T2/nightly/soak** ⇒ **nhường ngay** (rc 1 + in ai giữ); chủ khoá là **T1 mã cũ** ⇒ cảnh báo + chạy (2 T1, ≤ 2 QEMU) |
| `v2 t0` (T0) | — | `v2_lock_t0_warn` — **cảnh báo**, không chặn |
| `nightly`/`soak*.sh` | EXCLUSIVE | **giữ nguyên** EXCLUSIVE |

### 2.2 `measure_lock_is_held` (chỉ đọc, `v2/tools/measure-lock.sh`)

Hợp đồng: `0` = khoá **sống** (hoặc `owner` không đọc được ⇒ fail-closed), `1` = trống/stale/
`V2_MEASURE_LOCK=off`. **Không** giành, **không** phá stale, **không** tạo gì (khác `acquire`).
`--selftest` **13/13**: 4 ca mới cho `is_held`
(10) đang giữ ⇒ 0; (11) trống ⇒ 1; (12) stale `pid=999999` ⇒ 1 **và không bị phá** (chỉ `acquire`
mới phá); (13) chủ khoá là tiến trình **khác còn sống** ⇒ 0. Bản cũ 9 ca vẫn xanh.

### 2.3 Pre-flight T2 (`< 1 s`, D2-248)

Chạy **trước** bước nào của T2: `git status --porcelain` khác rỗng ⇒ TỪ CHỐI (in 10 dòng đầu);
`df -Pk` < `V2_PREFLIGHT_MIN_FREE_GIB` (mặc định **5 GiB**) ⇒ TỪ CHỐI + gợi ý `wt.sh prune`.
Không áp cho T1 (T1 chạy trên cây đang sửa — đúng mục đích T1). Pre-flight chỉ gồm **hai** lệnh rẻ (`git status --porcelain`, `df -Pk`) chạy **trước** khi
vào làn nào — không đụng cargo/QEMU, nên không cộng vào đường đo của T2.

## 3. C — `v2/test/boot/lib/env.sh` (7 runner `LOGDIR`, 3 `SOCK`)

### 3.1 Bảng runner → tham số (hành vi **y hệt** bản cũ)

| Runner | Trước | Sau | `--print-logdir` |
|---|---|---|---|
| `run-x86_64.sh` | `${KT_LOGDIR:-${KT257_LOGDIR:-…/k5-x86_64.$$}}` | `$(v2_env_logdir k5-x86_64 KT257_LOGDIR)` | có |
| `run-aarch64.sh` | `${KT_LOGDIR:-${KT250_LOGDIR:-…/k4-aarch64.$$}}` + `SOCK=$TMPDIR/kt283-mon-$$.sock` | `$(v2_env_logdir k4-aarch64 KT250_LOGDIR)` + `SOCKDIR="$(v2_env_sockdir kt283)"`, `SOCK="$(v2_env_sock … mon.sock)"` | có (+ `--print-sock`) |
| `gpu-x86_64.sh` | `${KT_LOGDIR:-${KT337_LOGDIR:-…/kt337-gpu-x86_64.$$}}` + `mktemp -d $TMPDIR/kt337s.XXXXXX` | `$(v2_env_logdir kt337-gpu-x86_64 KT337_LOGDIR)` + `v2_env_sockdir kt337` + `v2_env_sock` cho `mon.sock`/`mon-neg.sock` | có (+ `--print-sock`) |
| `soak.sh` | `${KT_LOGDIR:-${SOAK_LOGDIR:-…/k3-soak.$$}}` | `$(v2_env_logdir k3-soak SOAK_LOGDIR)` | có |
| `soak-aarch64.sh` | `${KT_LOGDIR:-${SOAK_AARCH64_LOGDIR:-…}}` + `mktemp -d /tmp/kt-sock.XXXXXX` | `$(v2_env_logdir k4-aarch64-soak SOAK_AARCH64_LOGDIR)` + `v2_env_sockdir kt` | có (+ `--print-sock`) |
| `soak-x86_64.sh` | `${KT_LOGDIR:-${SOAK_X86_64_LOGDIR:-…}}` | `$(v2_env_logdir k5-x86_64-soak SOAK_X86_64_LOGDIR)` | có |
| `persist.sh` | `${KT75_LOGDIR:-…/kt75-persist}` (**không** pid) | `$(v2_env_logdir kt75-persist KT75_LOGDIR nopid)` | **thêm** (KT-333c chưa có) |

7/7 runner `source` `lib/env.sh` — **0** chỗ lặp lại biểu thức `LOGDIR` (trước: 7 bản).

### 3.2 Ca âm — `TMPDIR` rất dài (125 byte) vẫn chạy được, socket **< 104 byte** (nguyên văn)

```text
TMPDIR len=125
run-x86_64.sh      rc=0 logdir=/tmp/xxx…/k5-x86_64.51518
run-aarch64.sh     rc=0 logdir=/tmp/xxx…/k4-aarch64.51529
gpu-x86_64.sh      rc=0 logdir=/tmp/xxx…/kt337-gpu-x86_64.51539
soak.sh            rc=0 logdir=/tmp/xxx…/k3-soak.51555
soak-aarch64.sh    rc=0 logdir=/tmp/xxx…/k4-aarch64-soak.51571
soak-x86_64.sh     rc=0 logdir=/tmp/xxx…/k5-x86_64-soak.51584
persist.sh         rc=0 logdir=/tmp/xxx…/kt75-persist
--- SOCK:
run-aarch64.sh     rc=0 sock=/tmp/v2s-kt283.D45Vr5/mon.sock          (30 byte)
gpu-x86_64.sh      rc=0 sock=/tmp/v2s-kt337.q2wRXY/mon.sock, …/mon-neg.sock (65 byte cả 2 dòng)
soak-aarch64.sh    rc=0 sock=/tmp/v2s-kt.fG0WLL/mon-1.sock           (29 byte)
```

(`xxx…` = 120 ký tự `x`; logdir **được phép** dài, chỉ **socket** cần ngắn.) `env.sh --selfcheck`
**8/8**, gồm ca (5) TMPDIR dài 186 byte ⇒ socket 31 byte; (6) **bind AF_UNIX thật** bằng python3
trên đường dẫn ngắn; (7) `v2_env_sock` **từ chối** đường dẫn > 103 byte. Ngưỡng `103` = `sun_path`
104 byte của macOS trừ NUL — fail-closed thay vì để QEMU chết mờ
(`Path must be less than 104 bytes`).

## 4. D — `v2/tools/wt.sh` (4 slot + cache ấm)

- `acquire <1..4> <nhánh> [base]` → `git worktree add` vào `/tmp/vifast-wt/<n>` (đổi bằng
  `V2_WT_BASE`), in `export CARGO_TARGET_DIR=<repo>/target-task/slot-<n>` + `V2_WT_SLOT` +
  `V2_WT_PATH` (dùng `eval "$(bash v2/tools/wt.sh acquire 1 task/x main)"`).
- **Từ chối** (rc 2): slot ngoài 1..4; slot đang bận; nhánh đã checkout ở worktree khác
  (một nhánh không thể ở hai worktree); nhánh sai `git check-ref-format --branch`; **`df` <
  `V2_WT_MIN_FREE_GIB` (mặc định 5 GiB)** — kiểm **trước** khi tạo gì.
- `release <slot>`: `git worktree remove --force`, **giữ nguyên** `target-task/slot-<n>` (cache
  ấm cho task kế); **không** xoá nhánh (nhánh có thể chưa merge).
- `list`: 4 slot + nhánh + target + đĩa trống. `prune`: `git worktree prune` + xoá thư mục **mồ
  côi** dưới `V2_WT_BASE` (không đụng worktree đang dùng).
- Chuẩn hoá đường dẫn bằng `pwd -P` (macOS: `/var` → `/private/var`, `//`) để so khớp
  `git worktree list` — nếu không, `prune` xoá nhầm worktree đang dùng (đã bắt được khi viết
  selftest).
- `--selftest` **11/11** trên **repo tạm** (`V2_WT_REPO`/`V2_WT_BASE`/`V2_WT_TARGET_BASE`), không
  chạm repo thật: cấp slot + cache; slot bận; slot 5; nhánh trùng; `df` thấp ⇒ không tạo worktree;
  nhánh sai; `list`; `release` giữ cache; `release` slot trống; nhánh sẵn có dùng lại; `prune`.

## 5. E — `v2 t0` + `vifast-agentctl lint`/`sync-queue`

### 5.1 `v2 t0 [crate …] [--crate <tên>] [--no-test] [--selftest]`

In rõ **`TẦNG T0 (KHÔNG phải cổng; T1 = gate --dev, T2 = gate --fast)`**; gọi `v2_lock_t0_warn`
(có cổng giữ khoá ⇒ **CẢNH BÁO**, không chặn); `cargo fmt --all --check`; suy **crate bị chạm** từ
`git diff --name-only HEAD` + untracked (đi ngược tìm `Cargo.toml` gần nhất, **không** đoán bừa với
`v2/agent/*`); mỗi crate: `cargo clippy -p … --all-targets -- -D warnings` + `cargo test -p …`
(tắt bằng `--no-test`). `--selftest` **11/11** (suy luận crate, khử trùng, tên lạ rc=2, khoá
trống/đang giữ, banner, + `run_t0_tooling` 5/5 công cụ).

### 5.2 `vifast-agentctl lint` (mới; wrapper `v2/bin/vifast-agentctl`)

Kiểm **QUEUE**: mẫu mã `^KT-[0-9]+[a-z]?$` (v2) / `^ST-…$` (viscript); trạng thái ∈ 4 giá trị;
không trùng mã; ≥ 6 cột; phụ thuộc **tồn tại**; `READY` đủ phụ thuộc `DONE`; `BLOCKED` **không**
được đủ phụ thuộc `DONE` (phải lật READY); không vòng lặp. Kiểm **STATE**: khối hiện tại tồn tại;
có dòng `HEAD` + `TASK`; số `(N task)` khớp QUEUE **theo khoảng** trên chính dòng đó
(`KT-243..KT-345a`) hoặc tổng; HEAD khớp `git rev-parse --short HEAD`.

Trên **sổ thật** hôm nay:

```text
vifast-agentctl lint: PASS (0 lỗi, 14 cảnh báo)
```

14 cảnh báo = 2 mã lịch sử (`KT-T26`/`KT-R1`, đều `DONE`), 4 dòng 7 cột (ô DoD có `|` chưa
escape — đúng là khuyết điểm dữ liệu lịch sử), 6 phụ thuộc trỏ task không có trong sổ (đều `DONE`),
và 2 lệch `STATE` (`(104 task)` so với QUEUE **106** trong khoảng; `HEAD b845ac34` so với
`3a8219ac`).

### 5.3 Vì sao `lint` có **hai** mức (E đỏ / W cảnh báo) — và vì sao mặc định lệch STATE **không** đỏ

`E_*` = lỗi cấu trúc **agent sửa được ngay**. `W_*` = dữ liệu **lịch sử** (**không** sửa được nếu
không viết lại 338 dòng sổ — và `QUEUE.md`/`STATE.md` **thuộc Agent 0**, sub-agent bị **cấm** sửa
theo `AGENTS.md` §3) hoặc **lệch STATE** (việc của lead ở bước tích hợp). Nếu để mặc định ĐỎ thì
cổng quy trình của **mọi** sub-agent đỏ vì lý do ngoài quyền của họ — đúng loại "bỏ âm thầm phép
kiểm" cần tránh. Vì vậy:

- `--strict-state`: lệch HEAD/số DONE thành **`E_STATE_*`** (đỏ) — lead dùng trước khi merge.
- `--strict`: **mọi** cảnh báo thành đỏ (gồm mã lịch sử + cột + dep lịch sử).
- `lint --selftest` **12/12** (fixture trong RAM, không đọc sổ thật).

### 5.4 `vifast-agentctl sync-queue` (chỉ sửa **số đếm**)

Tái dùng bộ đếm của `lint`; **chỉ** thay số trong mẫu `(N task)` trên dòng `TASK` của **khối hiện
tại**; giữ nguyên văn xuôi, dòng `HEAD`, và mọi dòng ngoài khối. `--dry-run` in `-`/`+` mà không
ghi. Trên sổ thật:

```text
- TASK          : **KT-243..KT-345a DONE** (104 task). Chu kỳ 2: …
+ TASK          : **KT-243..KT-345a DONE** (106 task). Chu kỳ 2: …
```

`--selftest` **7/7** (khoảng / tổng / đã đúng không đổi byte / văn xuôi + số khác giữ nguyên /
HEAD giữ nguyên / thiếu mẫu ⇒ không sửa / dòng TASK ngoài khối không bị chạm). **Không** ghi
`STATE.md` trong task này (thuộc Agent 0/lead).

## 6. F — `soak-x86_64-max` trong `nightly --soak`

### 6.1 Kế hoạch (`v2 nightly --soak --dry-run`, nguyên văn)

```text
   phạm vi: --soak (3 arch + x86_64 -cpu max; N=1000/1000/600; runs=3)
   N: riscv64=1000 aarch64=1000 x86_64=600 x86_64-max=600; runs=3 (N đầy KT-348: 1000/1000/600/600 × 3)
   4  soak      Soak x86_64 -cpu max ảnh sản phẩm (N=600 × 3 luồng) 220s
        $ V2_X64_CPU=max bash v2/test/boot/soak-x86_64.sh 600 3
```

Bước in thêm `# nightly: soak x86_64 cpu=max (KT-323; N=… runs=…)` khi chạy thật; `soak-x86_64.sh`
đã có `V2_X64_CPU` (KT-323) và in `# cấu hình: … -cpu max …` + `# infra: cpu=max …`.

### 6.2 Không nhân đôi T2

`grep -R "soak" lib/steps_check.sh lib/steps_gate.sh lib/check.sh lib/gate.sh` **không** có lời gọi
soak nào; `soak-x86_64-max` chỉ xuất hiện trong `lib/steps_soak.sh` (được `lib/nightly.sh` gọi).
`nightly --selftest` **20/20** (thêm ca `V2_X64_CPU=max bash … 600 3`).

## 7. G — Cổng đã chạy

### 7.1 Bắt buộc cho sub-agent (T0 + T1)

| Lệnh | Kết quả |
|---|---|
| `./v2/bin/v2 check` | **PASS** — `# gate-steps registered=45 ran=45 skipped=0 failed=0` (trùng khít trước tách) |
| `./v2/bin/v2 gate --dev` (T1) | **PASS** — `# gate-steps registered=77 ran=64 skipped=13 failed=0`; `gate-wall 779.023 s` (dưới tải 4 agent); xem §7.2 + §7.5 (lượt 1 T-INFRA) |
| `bash v2/tools/wt.sh --selftest` | **PASS 11/11** |
| `./v2/bin/v2 t0 --selftest` | **PASS 11/11** (+ `T0 tooling: PASS 5/5`: measure-lock 13/13, wt.sh 11/11, env.sh 8/8, lint 12/12, sync-queue 7/7) |
| `./v2/bin/vifast-agentctl lint` | **PASS** (0 lỗi, 14 cảnh báo — §5.3 có lý do từng cảnh báo) |
| `bash v2/test/rules/selfbreak.sh` (bước T2 — chạy tay vì đã sửa `hash_real_tree`) | **5/5 ca phá ĐỎ đúng mã; cây thật không đổi** |
| `python3 v2/tools/check-selftest-wiring.py --selftest` | **9/9** |

### 7.2 `gate --dev` (T1) sau khi tách

T1 **không** còn giành khoá. Lượt chạy cổng gặp một worktree khác (**mã trước** KT-333d) còn giữ
khoá nhãn `gate --dev (T1)`; lớp 2 phân biệt được nhãn nên in:

```text
--- T1: một T1 KHÁC (mã trước KT-333d) đang giữ khoá — T1 không phải phép đo ⇒ tiếp tục (không chồng T2/nightly/soak).
```

và **chạy song song** (2 T1 ⇒ tối đa 2 QEMU, dưới trần 3 của AGENTS §9). Đây là nhánh **chuyển
tiếp**: sau khi merge, không T1 nào giành khoá nên nhánh này thành vô hại; T1 vẫn **nhường ngay**
khi chủ khoá là T2/nightly/soak (nhãn không chứa `gate --dev`).

Kết quả T1 (nguyên văn):

```text
--- QEMU smoke (T1: 7 ảnh × 1 boot): OK
# step-time QEMU smoke (T1: 7 ảnh × 1 boot) 261525 ms
# gate-steps registered=77 ran=64 skipped=13 failed=0
# gate-wall 779023 ms (779.0 s)
v2 gate --dev: PASS (T1 — kiểm tích hợp nhanh, **KHÔNG** phải full gate; full gate = 'gate --fast')
```

`registered=77` = 45 (`check`) + 31 (T2) + 1 (QEMU smoke id 70); `skipped=13` là **khuyết điểm có
từ trước** (không do lần tách này): mục thứ 14 của `V2_T2_ONLY_STEPS`
(`QEMU virtio-gpu x86_64 (KT-337/KT-333c)`) **không khớp** tên bước thật
(`QEMU virtio-gpu x86_64 + ca âm không GPU (KT-337/KT-333c)`) nên T1 **chạy** bước 79 (đo được
**47.728 ms**, PASS). Hằng số `T2_ONLY_STEPS_EXPECTED=14` giữ nguyên (không nới ngưỡng); đề xuất
một dòng cho lead: sửa mục 14 thành đúng tên bước ⇒ T1 bỏ đúng 14 bước, nhanh thêm ~48 s. **Không**
sửa trong task này vì nó đổi tập bước T1 (ngoài phạm vi "tách tệp giữ nguyên hành vi").

### 7.3 Commit

Commit `[KT-333d] tách v2/bin/v2 (2.185→161 dòng) + khoá 2 lớp + boot/lib/env.sh + wt.sh 4 slot +
v2 t0 + agentctl lint/sync-queue + soak-x86_64-max` (hash thật: `git log -1 --format=%h` trên nhánh
`task/kt333d-v2lib`; không chép hash vào chính tệp này để tránh vòng lặp sửa-sau-commit).

Cây sạch sau commit: `git status --porcelain` rỗng (đã kiểm trước khi bàn giao).

### 7.4 Lượt T1 #1 = T-INFRA (D2-07) — không phải hồi quy từ gói này

Lượt chạy T1 đầu tiên **ĐỎ** đúng một bước, không liên quan mã của task:

```text
not ok 5 - boot 1: # có 34 dòng 'ok', cần 35 # có 1 dòng 'not ok' # thiếu 'ok 35 - ...'
--- QEMU aarch64 (K4' bước 3): FAIL
# step-time QEMU aarch64 (K4' bước 3) 130414 ms (FAIL)
```

Log khách (`k4-aarch64.51240/boot-1.log`): `# net dns: elapsed_ms=6001 attempts=2 err=Timeout`,
`not ok 35 - net: dns` (DNS của slirp timeout dưới `loadavg_1m=9.70/5m=11.81`, ba agent khác đang
build/T1; guard log `qemu_foreign=0`, không có QEMU lạ). `env.sh` chỉ đổi **đường dẫn LOGDIR/SOCK**,
không chạm DNS; ảnh đã build + boot + 34/35 TAP. Phân loại **T-INFRA**, chạy lại **một lần** (D2-07)
⇒ **PASS** (lượt 2 ở §7.2); không retry thêm để "lấy xanh".

### 7.5 Rủi ro / việc của lead

1. **Pre-flight T2** từ chối cây bẩn + `df < 5 GiB` **trước** khi chạy bước nào (D2-248). Lead phải
   commit/stash ứng viên tích hợp trước `gate --fast`; đây là chủ ý, không phải ngưỡng mới.
2. **T1 nhường khi lớp 1 đang giữ**: nếu lead đang chạy T2/nightly/soak, T1 của sub-agent sẽ in
   `T1: … nhường ngay` và rc 1. Chờ khoá nhả rồi chạy lại (60 s) — không có `V2_MEASURE_LOCK=off`.
3. **`lint` mặc định không đỏ vì lệch STATE**: `STATE.md` đang lệch **thật** (`HEAD b845ac34` so
   với `3a8219ac`; `(104 task)` so với **106** trong khoảng `KT-243..KT-345a`). Lead chạy
   `./v2/bin/vifast-agentctl sync-queue` (sửa **số đếm**), cập nhật dòng `HEAD`, rồi
   `lint --strict-state` để chắc; `sync-queue` **không** được chạy bởi sub-agent (cấm sửa STATE).
4. **4 dòng QUEUE 7 cột** (dòng 162/197/414/425) do `|` chưa escape trong ô DoD — dữ liệu lịch
   sử; muốn sạch thì Agent 0 sửa `\|` (ngoài quyền task này).
5. **Lỗi có sẵn**: mục 14 `V2_T2_ONLY_STEPS` không khớp tên bước 79 ⇒ T1 chạy oan bước này (~48 s,
   §7.2). Sửa một dòng (mục 14 = tên bước thật) là xong; chưa sửa để không đổi tập bước T1 trong
   commit "tách tệp".
6. **`target-task/`** (cache slot) nằm ngoài git; `df` hiện ~8,7 GiB > 5 GiB nên `wt.sh acquire`
   chạy được, nhưng 4 slot build đầy có thể chạm ngưỡng — dùng `wt.sh prune`/xoá slot đã merge.
