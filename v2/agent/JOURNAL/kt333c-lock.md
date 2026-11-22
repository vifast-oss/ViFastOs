# KT-333c — Khoá độc quyền máy đo · logdir riêng theo phiên · `gpu-x86_64.sh` vào T2 · `nightly --soak` đủ N (D2-243 mục 3)

- **Task**: `KT-333c` (Chu kỳ 2, Đợt 2, P1, ≤ 2 h) — ngoài `v2/kernel/**` và `v2/lib/**`.
- **Nhánh**: `task/kt333c-lock`, base `main` @ `bfb9a13d`.
- **Ngày**: 2026-11-22 (JST).
- **Quyết định**: **D2-243 mục 3** (a/b/c), ràng buộc **D2-235(5)** (không trùng id `lane_step`).
- **Kết quả**: `v2/tools/measure-lock.sh` (9 ca tự kiểm PASS) + nối fail-closed vào
  `gate --dev`/`gate --fast`/`nightly`/3 `soak*.sh`; logdir **riêng theo phiên** cho 6 runner
  (ca âm: 12/12 đường dẫn khác nhau khi 12 tiến trình chạy song song); `gpu-x86_64.sh` (đo **21 s**
  ca dương / **33 s** dương+âm) vào T2 **kèm ca âm "không GPU"** (60 lỗi, không PASS giả);
  `nightly --soak` chạy đủ **3 arch, N đầy 1000/1000/600**, `--n`/`--runs` ghi rõ, `--status`
  đọc bằng chứng soak gần nhất; self-test nightly **19/19**.

---

## 1. Bối cảnh — ba vấn đề thật trong ngày

| # | Sự cố 2026-11-22 | Hậu quả | Việc trong KT-333c |
|---|---|---|---|
| 1 | Hai `gate --fast` chạy **song song** (15:31–15:45) | `git-guard` tranh `.git` (số worktree 77 → 78), QEMU chồng tải, **5 bước đỏ oan**, một số "−43,9 %" phải vô hiệu | **A** — khoá độc quyền **cấp máy đo**, fail-closed |
| 2 | Logdir dùng chung `$TMPDIR/k5-x86_64`, `k4-aarch64`, `k3-soak` | hai cổng ghi đè log của nhau | **B** — logdir `$V2_RUN_TMP`/`$TMPDIR` + **pid** |
| 3 | `gpu-x86_64.sh` **chưa** nằm trong T2; `v2 nightly --soak` chưa đủ cho `KT-348` | cổng xanh dù GPU x86_64 hỏng; soak không đủ N/arch | **C** + **D** |

Ràng buộc đã giữ: **không** sửa `spec/`, `v2/kernel/**`, `v2/lib/**`, `v2/agent/{STATE,QUEUE,DECISIONS}.md`;
**không** nới ngưỡng; **không** `#[ignore]`/retry; `lane_fails 999` **giữ nguyên**; `git add` theo
đường dẫn cụ thể.

---

## 2. A — `v2/tools/measure-lock.sh` (khoá độc quyền cấp máy đo)

### 2.1 Cơ chế (không phụ thuộc GNU coreutils — macOS bash 3.2)

| Thành phần | Cách làm | Vì sao |
|---|---|---|
| Giữ khoá | `mkdir` **nguyên tử** trên `${TMPDIR}/vifast-measure.lock` | macOS không có `flock`; `mkdir` là phép nguyên tử duy nhất có sẵn |
| Nội dung | `owner`: `pid:host:start_epoch:nhãn` | đủ để kiểm sống/chết, tuổi, ai giữ |
| Stale | PID **không còn sống** (`kill -0`) **hoặc** tuổi > **6 h** (`V2_MEASURE_LOCK_MAX_AGE=21600`) ⇒ `mv` thư mục rồi `rm` + **cảnh báo** | `mv` tránh hai tiến trình cùng phá xoá khoá vừa giành của nhau |
| Owner đọc không được | **fail-closed** (coi như đang giữ), chỉ phá khi thư mục > 6 h | khoá vừa `mkdir` nhưng chưa kịp ghi `owner` **không** được coi là trống |
| `release` | chỉ `rm` khi `pid` trong khoá **= PID của mình** | không xoá khoá của người khác (rc=3) |
| `status` | in ai giữ, host, nhãn, **bao lâu**, còn sống/stale | để người vận hành biết chờ ai |
| Lồng | cùng PID ⇒ cho qua (không tạo khoá mới) | `gate --dev` gọi `gate --fast` |
| Uỷ quyền | con nhận `V2_MEASURE_LOCK_DELEGATED_BY=<pid cha>` ⇒ dùng khoá của cha, `release` **không** xoá | `v2 nightly` giữ khoá, `soak*.sh` là tiến trình con |
| Thoát hiểm | `V2_MEASURE_LOCK=off` ⇒ cho qua + **cảnh báo to** | chỉ dùng máy khác; không phải đường né khoá trên máy đo |

Nhả khoá đi qua `measure_lock_release_if_top`: trap `EXIT` bị **subshell làn** thừa kế, mà bash 3.2
không có `$BASHPID` (`$$` trong subshell vẫn là PID cha) ⇒ nếu không chặn, làn nền đầu tiên xong sẽ
xoá khoá của cha.

### 2.2 Nối fail-closed

| Chỗ gọi | Hành vi | Dòng |
|---|---|---|
| `do_gate_fast` | `measure_lock_enter "gate --fast (T2)"` **ngay đầu** — bị từ chối ⇒ chưa chạy bước nào | `v2/bin/v2` |
| `do_gate_dev` | `measure_lock_enter "gate --dev (T1)"`; `do_gate_fast` lồng bên dưới không xin lần hai | `v2/bin/v2` |
| `do_nightly` (mọi mode **chạy thật**) | giữ khoá; export `V2_MEASURE_LOCK_DELEGATED_BY=$$` cho `soak*.sh` | `v2/bin/v2` |
| `soak.sh` | `. tools/measure-lock.sh` + `measure_lock_acquire` + trap chuỗi `qg_cleanup` | `v2/test/boot/soak.sh` |
| `soak-aarch64.sh` | như trên, trap chuỗi thêm `stop_soak_dns_server`/`stop_soak_tcp_server`/`rm SOCKDIR` | `v2/test/boot/soak-aarch64.sh` |
| `soak-x86_64.sh` | như trên | `v2/test/boot/soak-x86_64.sh` |

### 2.3 Bước T2 mới (id **78**, làn **D**, chỉ đọc/thuần shell)

```text
lane_step D 78 "selftest khoá máy đo (KT-333c)" step_measure_lock
step_measure_lock() { bash "$V2_ROOT/tools/measure-lock.sh" --selftest; }
```

`--selftest` dùng `V2_MEASURE_LOCK_DIR` trỏ **thư mục tạm của ca** ⇒ tuyệt đối không chạm khoá thật;
9 ca: giữ/status, **giữ lần hai ⇒ TỪ CHỐI**, stale pid, quá hạn 6 h, release sai pid, release đúng pid,
owner hỏng, `off`, release khi `off`.

---

## 3. B — Logdir riêng theo phiên

| Runner | Trước | Sau (mặc định) | Ép bằng |
|---|---|---|---|
| `run-x86_64.sh` | `$TMPDIR/k5-x86_64` | `$V2_RUN_TMP` hoặc `$TMPDIR/k5-x86_64.<pid>` | `KT_LOGDIR`, `KT257_LOGDIR` |
| `run-aarch64.sh` | `$TMPDIR/k4-aarch64` | `$V2_RUN_TMP` hoặc `$TMPDIR/k4-aarch64.<pid>` | `KT_LOGDIR`, `KT250_LOGDIR` |
| `soak.sh` | `$TMPDIR/k3-soak` | `$V2_RUN_TMP` hoặc `$TMPDIR/k3-soak.<pid>` | `KT_LOGDIR`, `SOAK_LOGDIR` |
| `soak-aarch64.sh` | `$TMPDIR/k4-aarch64-soak` | `$V2_RUN_TMP` hoặc `$TMPDIR/k4-aarch64-soak.<pid>` | `KT_LOGDIR`, `SOAK_AARCH64_LOGDIR` |
| `soak-x86_64.sh` | `$TMPDIR/k5-x86_64-soak` | `$V2_RUN_TMP` hoặc `$TMPDIR/k5-x86_64-soak.<pid>` | `KT_LOGDIR`, `SOAK_X86_64_LOGDIR` |
| `gpu-x86_64.sh` | `$TMPDIR/kt337-gpu-x86_64` | `$V2_RUN_TMP` hoặc `$TMPDIR/kt337-gpu-x86_64.<pid>` | `KT_LOGDIR`, `KT337_LOGDIR` |

Mỗi runner có `--print-logdir` (in đường dẫn đã giải rồi thoát, không chạy gì) để chứng minh ca âm.

### Ca âm — hai lần chạy song song không ghi cùng tệp (log nguyên văn)

6 runner × 2 lượt chạy **đồng thời** (12 tiến trình):

```text
=== lần 1 ===
/var/folders/.../T//k5-x86_64.79213
/var/folders/.../T//k4-aarch64.79214
/var/folders/.../T//k4-aarch64-soak.79216
/var/folders/.../T//kt337-gpu-x86_64.79218
/var/folders/.../T//k5-x86_64-soak.79217
/var/folders/.../T//k3-soak.79215
=== lần 2 ===
/var/folders/.../T//k4-aarch64.79252
/var/folders/.../T//k5-x86_64.79251
/var/folders/.../T//kt337-gpu-x86_64.79256
/var/folders/.../T//k4-aarch64-soak.79254
/var/folders/.../T//k5-x86_64-soak.79255
/var/folders/.../T//k3-soak.79253
số dòng: 12
số dòng duy nhất: 12
```

---

## 4. C — `gpu-x86_64.sh` vào T2 + ca âm "không GPU"

### 4.1 Đo thời gian (theo yêu cầu: đo **trước** khi thêm vào T2)

| Lần | Chế độ | Wall đo được | Kết quả |
|---|---|---:|---|
| 1 | ca dương (`--features ktest,gpu`) | **21 s** | 0 lỗi, PPM 921.615 B, `gpu_ppm.py` 23 kiểm PASS |
| 2 | dương + ca âm (lần đầu, dưới tải ngoài) | 131 s | **T-INFRA**: QEMU không tự thoát trong ngân sách 85 s, TAP 33 `net: dns error` — `loadavg_1m=5.97`, một tiến trình `node` khác chiếm 110 % CPU (agent song song `KT-335`/môi trường). Không phải lỗi mã. |
| 3 | dương + ca âm (phân loại lại **một lần**, D2-07) | **33 s** | **PASS**: dương 0 lỗi + ca âm `ok - ca âm (không GPU): bộ kiểm TỪ CHỐI ảnh không feature gpu (60 lỗi)` |

Trần 60 s ⇒ **đủ điều kiện** thêm vào T2 (kể cả ca âm đi kèm).

### 4.2 `lane_step` mới (id **79**) — **làn `sau6`**, không phải làn `D`

```text
lane_step sau6 79 "QEMU virtio-gpu x86_64 + ca âm không GPU (KT-337/KT-333c)" step_gpu_x86_64
step_gpu_x86_64() { KT337_NEGATIVE=1 bash "$V2_ROOT/test/boot/gpu-x86_64.sh"; }
```

**Vì sao lệch làn D (đề nghị của phiếu)**: làn `D` là làn nền **chỉ đọc**; `gpu-x86_64.sh` vừa
**build** vào `v2/kernel/target` vừa **mở QEMU**. Đặt ở làn nền sẽ chạy song song `main` (L0/L1) —
đua `kernel/target` (feature khác nhau ghi đè cùng đường ảnh) và chồng QEMU, đúng lớp lỗi D2-116.
`sau6` là làn đuôi đã dành cho QEMU x86_64 (59/60/63); thêm 79 giữ tối đa **2 QEMU** đồng thời
(sau5 + sau6), không đổi trần 3.

`V2_T2_ONLY_STEPS` thêm bước này ⇒ T1 (`gate --dev`) bỏ **14** bước (trước 13), in rõ `BỎ QUA`.

### 4.3 Ca âm "không GPU" (ảnh **không** bật feature `gpu`)

- Build lại ảnh `--features ktest` (**không** `gpu`) vào `--target-dir` riêng (`$LOGDIR/target-no-gpu`)
  để **không** đè ảnh dương; boot **thật** cùng cấu hình QEMU (vẫn có `-device virtio-gpu-pci`).
- Bộ kiểm **phải ĐỎ**: log có `1..36` (không phải `1..37`), **0** dòng `# gpu …`, thiếu
  `ok 37 - gpu: 2d ppm` ⇒ **60 lỗi**; ca âm **PASS** (đúng kỳ vọng) và **không** có PPM.
- Nếu bộ kiểm lại PASS ảnh không GPU ⇒ in
  `not ok - ca âm (không GPU): bộ kiểm PASS ảnh không feature gpu — **PASS GIẢ**` và cổng **ĐỎ**.
- Log nguyên văn:

```text
# ---------------------------------------------------------------- ca âm: ảnh KHÔNG feature gpu
# build ảnh KTEST x86_64 KHÔNG feature gpu (ca âm)
# boot vifast-kbin-x86_64-no-gpu 189600 B (virtio-net + virtio-blk + virtio-gpu + monitor)
not ok - log phải có đúng một '1..37'
not ok - log có 36 dòng 'ok', đòi 37
not ok - thiếu 'ok 37 - gpu: 2d ppm'
not ok - dòng '# gpu pci: ' phải đúng một lần, thấy 0
…
# ca âm: không có PPM (ảnh không feature gpu không dựng framebuffer) — kiểm log là đủ
ok - ca âm (không GPU): bộ kiểm TỪ CHỐI ảnh không feature gpu (60 lỗi, log .../kt337-gpu-no-gpu.log)
# gpu-x86_64.sh: 0 lỗi
```

---

## 5. D — `v2 nightly --soak` đủ cho `KT-348`

### 5.1 N và tham số

| Arch | N **đầy** (nghiệm thu) | `--n` | `--runs` |
|---|---:|---|---:|
| riscv64 | **1000** | ghi rõ, mặc định rỗng = N đầy | mặc định **3** (1..3) |
| aarch64 | **1000** | | |
| x86_64 | **600** | | |

- **Không** âm thầm hạ N: N dưới mức nghiệm thu ⇒ kế hoạch in N **thật** + dòng
  `# CẢNH BÁO: N/luồng dưới mức nghiệm thu KT-348 — KHÔNG phải bằng chứng nghiệm thu`, lệnh ghi rõ
  `--smoke`, và runner in `# soak: SMOKE …`; nightly chỉ nhận smoke khi log có **`0 lỗi/N`** + dòng
  `# soak: SMOKE` và **không** có `# soak: FAIL`.
- Manifest ghi `soak_n_riscv64/soak_n_aarch64/soak_n_x86_64/soak_runs/soak_full`.
- `--status --soak` đọc **bằng chứng soak gần nhất** (manifest có bước phase `soak`, kể cả lượt
  `--all`); chưa có ⇒ exit 0 + nói rõ. `--dry-run` in N từng arch và tổng thời gian ước lượng
  **co theo N/luồng**.

### 5.2 `--dry-run` (log nguyên văn, rút gọn)

```text
   phạm vi: --soak (3 arch; N=1000/1000/600; runs=3)
   N: riscv64=1000 aarch64=1000 x86_64=600; runs=3 (N đầy KT-348: 1000/1000/600 × 3)
   1  soak      Soak riscv64 ảnh sản phẩm (N=1000 × 3 luồng)      320s
        $ bash v2/test/boot/soak.sh 1000 3
   2  soak      Soak aarch64 ảnh sản phẩm (N=1000 × 3 luồng)      260s
        $ bash v2/test/boot/soak-aarch64.sh 1000 3
   3  soak      Soak x86_64 ảnh sản phẩm (N=600 × 3 luồng)        220s
        $ bash v2/test/boot/soak-x86_64.sh 600 3
   tổng [ƯL]: 3 bước ~ 13 phút 20 giây (máy rảnh; không phải cam kết)
```

`--n 5 --runs 1` ⇒ phạm vi `N=5/5/5; runs=1`, cảnh báo "KHÔNG phải bằng chứng nghiệm thu",
lệnh `--smoke 5 1`, tổng ước lượng 3 s.

### 5.3 Chạy thật **một lần** `--soak --n 5 --runs 1` (log nguyên văn)

```text
# measure-lock: ĐÃ GIỮ khoá máy đo (pid=3507 host=mps nhãn="nightly --soak (3 arch; N=5/5/5; runs=1)")
== v2 nightly: phạm vi --soak (3 arch; N=5/5/5; runs=1) ==
--- nightly 1: Soak riscv64 ảnh sản phẩm (N=5 × 1 luồng; SMOKE — không phải bằng chứng nghiệm thu)
--- nightly 1: PASS (18s; …/step-1-soak.log)
--- nightly 2: Soak aarch64 ảnh sản phẩm (N=5 × 1 luồng; SMOKE — không phải bằng chứng nghiệm thu)
--- nightly 2: PASS (35s; …/step-2-soak.log)
--- nightly 3: Soak x86_64 ảnh sản phẩm (N=5 × 1 luồng; SMOKE — không phải bằng chứng nghiệm thu)
--- nightly 3: PASS (78s; …/step-3-soak.log)
v2 nightly: PASS (3 bước; bằng chứng target-task/nightly/nightly-20261122-113810-bfb9a13d; STATUS.md đã sinh)
# measure-lock: đã nhả khoá (pid=3507)
```

Mỗi arch in `# soak: 5/5 lỗi=0` + `# soak: SMOKE` ⇒ nightly nhận là smoke PASS (không phải nghiệm thu).
**Không** chạy soak đầy (quá lâu). Sau khi lấy bằng chứng, `v2/agent/reports/STATUS.md` +
`v2/agent/JOURNAL/nightly/nightly-2026…` do lượt smoke sinh đã được **hoàn nguyên/xoá** — chúng
không phải bằng chứng nghiệm thu, không được ghi đè trạng thái thật (bằng chứng thô vẫn ở
`target-task/nightly/`, gitignored).

`--soak --status` sau lượt chạy đọc đúng manifest:

```text
# bằng chứng soak gần nhất: .../nightly-20261122-113810-bfb9a13d/manifest.tsv
   kết quả: PASS
#   soak_n_riscv64=5
#   soak_n_aarch64=5
#   soak_n_x86_64=5
#   soak_runs=1
#   soak_full=0
```

### 5.4 `--selftest` — 19/19 (thêm 5 ca soak)

```text
ok 8 - kế hoạch --soak: N đầy 3 arch (riscv64=1000, aarch64=1000, x86_64=600 × 3 luồng)
ok 9 - kế hoạch --soak --n 5 --runs 1: N ghi rõ + cảnh báo KHÔNG phải bằng chứng
ok 10 - âm: --soak --n abc bị từ chối (exit 2)
ok 11 - âm: --soak --runs 0 bị từ chối (exit 2)
ok 12 - dương: --soak --status khi chưa có bằng chứng ⇒ exit 0 + nói rõ
…
v2 nightly --selftest: PASS (19/19)
```

---

## 6. Số bước T2 trước/sau + hằng số

| Hằng số | Trước | Sau | Ghi chú |
|---|---:|---:|---|
| `CHECK_STEPS_EXPECTED` | 44 | **45** | + id 78 (làn D) |
| `GATE_FAST_EXTRA_EXPECTED` | 30 | **31** | + id 79 (làn sau6) |
| `T2_ONLY_STEPS_EXPECTED` | 13 | **14** | id 79 bỏ ở T1 |
| **Tổng bước T2** | **74** (`44 + 30`) | **76** (`45 + 31`) | chú thích trong `do_gate_fast` đã sửa đúng cùng dòng |
| T1 đăng ký | 75 | **77** | 76 + id 70 (QEMU smoke), bỏ 14 |

Kiểm trùng id `lane_step` (D2-235(5)): dãy id đã đăng ký là
`1..64, 66, 67, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79` — **0 id trùng** (65/68 chưa dùng).
`lane_fails 999` **giữ nguyên**; `do_check` đếm thêm `lane_failed 78` (id ngoài dải 1..40).

---

## 7. Đột biến — chứng minh khoá **có răng**

| Đột biến | Kỳ vọng | Quan sát (log nguyên văn) | Kết quả |
|---|---|---|---|
| **Bỏ `release`** (hàm release thành no-op, tiến trình giữ khoá **còn sống** — đúng tình huống hai cổng chồng nhau) | lần chạy sau **TỪ CHỐI** | xem §7.1 | ✅ |
| Hai tiến trình cùng xin khoá (không đột biến) | tiến trình thứ hai **TỪ CHỐI**, khoá còn của chủ cũ | xem §7.2 | ✅ |
| PID trong khoá đã chết (không release) | **tự phá** stale + cảnh báo (không treo máy) | xem §7.3 | ✅ |
| `release` sai PID | **KHÔNG** xoá (rc=3) | ca 5 của `--selftest` | ✅ |
| `V2_MEASURE_LOCK=off` | cho qua + **cảnh báo to** | ca 8 của `--selftest` | ✅ |

### 7.1 Đột biến "bỏ `release`" ⇒ lần sau TỪ CHỐI

```text
=== lần chạy sau, khi cổng-1 còn sống ===
# TỪ CHỐI ĐO (measure-lock): máy đo đang bận — KHÔNG chạy chồng cổng/soak.
#   chủ khoá: pid=21868 host=mps giữ 1s trước, nhãn="cổng-1 (đột biến bỏ release)"
#   hai cổng chạy song song làm hỏng số đo (sự cố KT-333a, 2026-11-22) ⇒ fail-closed.
cổng-2 rc=1

=== log cổng-1 ===
# measure-lock: ĐÃ GIỮ khoá máy đo (pid=21868 host=mps nhãn="cổng-1 (đột biến bỏ release)")
# cổng-1: đang chạy (giữ khoá, không nhả) — pid=21868
```

### 7.2 Giữ khoá lần hai ⇒ TỪ CHỐI (không đột biến)

```text
=== Tiến trình thứ hai xin khoá ===
# TỪ CHỐI ĐO (measure-lock): máy đo đang bận — KHÔNG chạy chồng cổng/soak.
#   chủ khoá: pid=78328 host=mps giữ 1s trước, nhãn="gate --fast (A)"
B: rc=1
=== status khi A đang giữ ===
measure-lock: ĐANG GIỮ pid=78328 host=mps từ 1s trước (nhãn="gate --fast (A)"; còn sống)
=== sau khi A nhả: status ===
measure-lock: TRỐNG (không ai giữ …/vifast-measure.lock)
```

### 7.3 PID chết không release ⇒ stale tự phá (không wedge máy)

```text
# CẢNH BÁO (measure-lock): phá khoá cũ: pid=21868 KHÔNG còn sống (host=mps nhãn="cổng-1 (đột biến bỏ release)")
# measure-lock: ĐÃ GIỮ khoá máy đo (pid=21991 host=mps nhãn="cổng-3")
# measure-lock: đã nhả khoá (pid=21991)
```

Fail-closed đã kiểm **end-to-end** (không tốn thời gian cổng): khi tiến trình khác giữ khoá,

```text
$ ./v2/bin/v2 gate --fast
# TỪ CHỐI ĐO (measure-lock): máy đo đang bận — KHÔNG chạy chồng cổng/soak.
…
v2 gate --fast: FAIL            (rc=1, chưa chạy bước nào)

$ bash v2/test/boot/soak.sh --smoke 1 1
# TỪ CHỐI ĐO …  soak rc=2

$ # con của nightly được uỷ quyền:
# measure-lock: dùng khoá của tiến trình cha pid=99744 (uỷ quyền qua V2_MEASURE_LOCK_DELEGATED_BY) — nhãn="cha"
con rc=0
```

---

## 8. Cổng đã chạy

| Lệnh | Kết quả |
|---|---|
| `bash -n v2/bin/v2` + `bash -n` 6 script runner | PASS |
| `bash v2/tools/measure-lock.sh --selftest` | **PASS (9/9 ca)** |
| `./v2/bin/v2 nightly --dry-run` | PASS (in đúng N + ước lượng) |
| `./v2/bin/v2 nightly --selftest` | **PASS (19/19)** |
| `./v2/bin/v2 check` | xem §9 |

---

## 9. `./v2/bin/v2 check`

```text
--- selftest khoá máy đo (KT-333c)
ok 1 - dương: acquire giữ khoá (pid=…) + status in đúng chủ/nhãn
…
measure-lock --selftest: PASS (9/9 ca)
--- selftest khoá máy đo (KT-333c): OK
# gate-steps registered=45 ran=45 skipped=0 failed=0
# step-time-total   116057 ms  TỔNG check
v2 check: PASS (L1 chạy trong gate --fast)
```

`registered=45` khớp `CHECK_STEPS_EXPECTED=45` (fail-closed nếu lệch), bước 78 xanh trong **1.295 ms**.
Lần chạy lại **sau khi stage** (bản cuối, chỉ sửa chú thích từ lần đầu): `v2 check: PASS`,
`# gate-steps registered=45 ran=45 skipped=0 failed=0`, `# step-time-total 89754 ms TỔNG check`.
Lưu ý: `v2 check` **không** chạy T2 (không có bước 41..79, không QEMU, không L0/L1) — theo đúng
định nghĩa AGENTS §6.1; $T2$ trên ứng viên bàn giao do lead chạy.

---

## 10. Tệp thay đổi

| Tệp | Thay đổi |
|---|---|
| `v2/tools/measure-lock.sh` | **mới** — khoá độc quyền cấp máy đo + `--selftest` 9 ca |
| `v2/bin/v2` | source khoá; `measure_lock_enter`/`leave`; nối `do_gate_fast`/`do_gate_dev`/`do_nightly`; `lane_step` 78 + 79; hằng số 45/31/14; `nightly --soak --n/--runs`, `--status --soak`, self-test 19 ca |
| `v2/test/boot/soak.sh` | logdir `+pid` + `--print-logdir`; nhận khoá + trap chuỗi |
| `v2/test/boot/soak-aarch64.sh` | như trên |
| `v2/test/boot/soak-x86_64.sh` | như trên; logdir ảnh phụ vẫn dưới `$LOGDIR` riêng |
| `v2/test/boot/run-x86_64.sh` | logdir `+pid` + `KT_LOGDIR` + `--print-logdir` |
| `v2/test/boot/run-aarch64.sh` | như trên |
| `v2/test/boot/gpu-x86_64.sh` | logdir `+pid` + `--print-logdir`; tách `boot_and_capture`; **ca âm không GPU** (`--negative`/`KT337_NEGATIVE=1`); hỗ trợ `KT_LOGDIR` |

---

## 11. Ghi chú / nợ

- **Lệch làn**: phiếu ghi làn `D`; đặt ở `sau6` vì script **build + QEMU** (làn nền chỉ-đọc). Đã
  ghi lý do ngay tại chỗ đăng ký và ở §4.2.
- **Khoá không phủ agent ngoài harness**: `KT-335` (agent song song) chạy QEMU bằng lệnh riêng,
  không đi qua `v2/bin/v2`/`soak*.sh` ⇒ khoá không chặn được nó. Sự cố lần 2 (131 s, QEMU không
  tự thoát) xảy ra khi `loadavg_1m=5.97` + một tiến trình `node` 110 % ⇒ ghi `T-INFRA`, phân loại
  lại **một lần** và xanh (33 s) — không phải retry để lấy xanh (D2-07).
- `--status --soak` hiện đọc manifest **cục bộ** (`target-task/nightly`, gitignored) + `STATUS.md`;
  nếu sau này bằng chứng soak được chuyển sang nơi khác thì phải sửa cùng `nightly_evid_root`.
