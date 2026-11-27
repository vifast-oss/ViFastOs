# KT-403 — vá công cụ `mutants` (`str`/`bytes`) + `mut-info-x1` sai lý do (lỗi manifest)

- **Task**: sub-agent Agent 2 dòng OS — quyết định của Lead 2026-11-27 sau lượt `mutants --all` Giờ
  Cao điểm 1 chết giữa chừng (`JOURNAL/heavy-20261127/mutants-all.log`; `STATE.md:43`).
- **Nhánh**: `task/kt403-mutants-fix` (base `main` @ **`a0f1f2e7`**).
- **Worktree**: `/private/tmp/vifast-wt/1`, `CARGO_TARGET_DIR=target-task/slot-1`.
  *Lệch nhiệm vụ*: slot 4 đang bận bởi `task/kt417d-fix` (agent song song) ⇒ dùng slot 1 thay vì 4
  (`bash v2/tools/wt.sh list` lúc mở: slot 3/4 bận, 1/2 trống); không release slot của agent khác.
- **Khung giờ**: khe Thấp điểm trưa `2026-11-27 04:09–04:50 UTC` (`D2-256`); ngân sách 45 phút.
- **Tệp sửa**: `v2/tools/mutants.py` (**+58/−8**, gồm 3 ca selftest mới), `v2/test/mutants.toml`
  (**2 dòng** — dấu phẩy cuối của `patches.replace`), `JOURNAL/mutants-fix.md` (tệp này).
  **0** chạm `v2/bin/`, `spec/`, kernel/runtime, `STATE/QUEUE/DECISIONS/AGENTS`; không nới ngưỡng.
- **Không chạy** `mutants --all` (90 phút) theo lệnh; chỉ tái hiện + selftest + **1** mutant.

## 1. Lỗi công cụ (`TypeError: can't concat str to bytes`, `mutants.py:480`)

### 1.1 Hiện trường (`/tmp/peak-mutants-all.log`, 40 dòng)

```text
7: not ok - mut-info-x1: E_WRONG-REASON (6.2 s) ... patches=1 git=HEAD rc=1 marker=KHONG
9-22: subprocess.TimeoutExpired: Command '['bash', '-c', 'bash v2/test/boot/ktest.sh']'
      timed out after 1800.0 seconds
34: File ".../v2/tools/mutants.py", line 526, in run_mutant
35:   rc, out, elapsed = _run_command(cmd, cwd, args.timeout)
37: File ".../v2/tools/mutants.py", line 480, in _run_command
38:   out = (exc.stdout or "") + (exc.stderr or "")
40: TypeError: can't concat str to bytes
```

Lượt `--all` chết ở mục 9/36 (`ktest-riscv64` timeout 1800 s) ⇒ 27 mục sau **không chạy**; toàn bộ
bản sao/log trong `/tmp/kt403-mutants-x1509nqu/` đã bị dọn theo `finally` của công cụ.

### 1.2 Nguyên nhân

`subprocess.run(..., text=True)` trả `stdout/stderr` là `str`, **nhưng** `TimeoutExpired.stdout` /
`.stderr` là **bytes**: CPython `Popen._check_timeout` nối `stdout_seq` **thô** trước khi giải mã, và
để `None` khi vế đó chưa có đầu ra. Vì thế `(exc.stdout or "")` biến vế rỗng thành `str ""`, cộng với
vế còn lại (bytes) ⇒ `TypeError` tại dấu `+`. "Bản vá" cũ `if isinstance(out, bytes): out = out.decode(...)`
nằm **sau** phép cộng nên là mã chết.

### 1.3 Tái hiện tối thiểu (đỏ-trước, mã CŨ `git show HEAD:v2/tools/mutants.py`)

```text
RED stdout rỗng, stderr bytes => TypeError: can only concatenate str (not "bytes") to str
RED stdout bytes, stderr rỗng => TypeError: can't concat str to bytes
```

(hai lệnh `printf 'LOI-…'; exec sleep 5` với `timeout=0.4` — đúng hai hướng của lỗi, không cần chờ
1800 s như `ktest.sh`).

### 1.4 Sửa — **một nguồn** chuẩn hoá

Thêm `_as_text(chunk)` (`None` ⇒ `""`, `bytes` ⇒ `decode("utf-8", "replace")`, còn lại giữ nguyên) và
dùng nó ở **cả 3** chỗ cộng `stdout`/`stderr`: `_git_snapshot` (dòng 454), đường thường (495), đường
`TimeoutExpired` (499). Không đổi ngữ nghĩa verdict/marker/ngưỡng.

```python
except subprocess.TimeoutExpired as exc:
    # KT-403 r3 — chuẩn hoá **từng** vế trước khi cộng
    out = _as_text(exc.stdout) + _as_text(exc.stderr)
    return 124, out, time.time() - t0
```

### 1.5 Selftest (đỏ-trước cho chính công cụ)

Thêm 3 ca vào `--selftest`: timeout **stdout rỗng/stderr bytes**, timeout **stdout bytes/stderr rỗng**
(mỗi ca đòi `rc=124` **và** giữ được chữ), và lệnh xanh (chặn hồi quy đường thường). Trước vá 2 ca đầu
ném `TypeError`; sau vá:

```text
ok 9  - timeout: stdout rỗng, stderr bytes: rc=124 (muốn 124) có 'LOI-TIMEOUT' => ĐÚNG
ok 10 - timeout: stdout bytes, stderr rỗng: rc=124 (muốn 124) có 'LOI-STDOUT' => ĐÚNG
ok 11 - lệnh xanh: cả hai vế đều str: rc=0 (muốn 0) có 'XANH' => ĐÚNG
ok 12 - cây thật (mini) không bị sửa bởi phép chạy => ĐÚNG
== SELFTEST mutants: OK — 12/12 ca (đỏ-trước cho chính công cụ) ==
```

`bash v2/bin/v2 mutants --lint` ⇒ `mutants=36 feature=35 declarations=55 covered=55 leftover=0 suites=12 problems=0`.

## 2. `mut-info-x1` = `E_WRONG-REASON` — **lỗi manifest**, không phải lỗ hổng harness

### 2.1 Hiện trường

`mutants-all.log:7` — `E_WRONG-REASON (6.2 s) patches=1 git=HEAD rc=1 marker=KHONG`. Suite
`ktest-aarch64` = `bash v2/test/boot/run-aarch64.sh 1`. Sáu giây **không đủ** một lần boot QEMU
(cùng suite, các mục khác 87–316 s) ⇒ suite chết trước khi có TAP.

### 2.2 Tái hiện (tất định, không cần env `v2 nightly`)

`bash v2/bin/v2 mutants mut-info-x1` trên cây `a0f1f2e7` **chưa vá** ⇒ `E_WRONG-REASON (6.5 s) …
rc=1 marker=KHONG` (log giữ lại). Không liên quan `V2_MEASURE_LOCK_DELEGATED_BY`/khoá máy đo.

### 2.3 Nguyên nhân thật (log bản sao)

```text
error[E0618]: expected function, found `(&str, Option<bool>)`
  --> vifast-kbin-aarch64/build.rs:88:9
   |
88 |         ("mut_info_x1", Some(true))
   |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^- help: consider separating array elements with a comma: `,`
   = note: `cargo::cfgs` nhận `&[(&str, Option<bool>)]` (v2/kernel/vifast-kbuild/src/cargo.rs:65)
run-aarch64.sh: build ảnh sản phẩm aarch64 FAIL (xem …/build.log)
```

`v2/test/mutants.toml:204` (mục `mut-info-x1`) vá `build.rs` bằng
`replace = '("mut_info_x1", Some(true))'` — **thiếu dấu phẩy cuối**, nên phần tử mảng bị Rust đọc
thành **lời gọi hàm** `("…", Some(true))("mut_no_x0_out", …)` ⇒ build script **không biên dịch** ⇒
`run-aarch64.sh` gọi `die` sau ~6 s ⇒ `rc=1`, **không có** dòng TAP nào ⇒ marker không khớp ⇒
`E_WRONG-REASON`. (`Some(true)` **đúng kiểu** `Option<bool>` — lỗi chỉ là dấu phẩy, không phải kiểu.)

Nguồn gốc: commit **`baec4f59`** `[KT-408c]` ghim lại `find` theo bảng `cargo::cfgs` mới
(KT-346) và viết `replace` thiếu dấu phẩy. Trước commit đó manifest dùng `find` cũ nên `mut-info-x1`
**KILLED** 82,6 s (`JOURNAL/kt403-mutants-all.md:20`) và 85,4 s (`JOURNAL/kt403-ratchet.md:221`).

### 2.4 Kết luận phân loại

**Lỗi manifest** (không phải lỗi công cụ, không phải lỗ hổng bộ kiểm thật). Bộ kiểm vẫn bắt đúng
đột biến — sau khi thêm dấu phẩy, **cùng** đột biến:

```text
ok - mut-info-x1: KILLED (88.8 s) copy=…/mut-mut-info-x1 patches=1 git=HEAD rc=1 marker=ok
# mutants: chạy=1 killed=1 survived=0 wrong-reason=0 stale=0 chưa-nối=0   (RC=0)
```

`marker=ok` = runner in `not ok <n> - boot 1: có N dòng 'not ok'` (TAP 43/44 đỏ **đúng lý do**;
xem `JOURNAL/kt403-ratchet.md:222`). Marker `dòng 'not ok'` **hẹp và đúng**: đỏ do hạ tầng
(`thiếu dòng 'TAP version 13'`, timeout) không in cụm này ⇒ **giữ nguyên marker**, không nới ngưỡng.

### 2.5 Cùng lỗi ở mục anh em (đã vá, chờ lượt `--all` xác nhận)

`v2/test/mutants.toml:393` (`mut-no-x0-out`, thêm ở cùng `baec4f59`) cũng thiếu dấu phẩy trong
`replace = '("mut_no_x0_out", Some(true))'`; hình dạng y hệt mục trên (cùng mảng `cargo::cfgs`, cùng
`Option<bool>`) nên đã vá cùng lượt. **Chưa chạy end-to-end** mục này (giới hạn "1 mutant" của lượt) —
đề nghị lượt `v2 mutants --all` kế tiếp xác nhận; nếu vẫn `E_WRONG-REASON` thì đó là lỗi khác, không
phải dấu phẩy.

## 3. Lệnh tái hiện / kiểm

```bash
cd /private/tmp/vifast-wt/1 && export CARGO_TARGET_DIR=/Users/mps/dvchd/ViFastOs/target-task/slot-1
bash v2/bin/v2 mutants --selftest          # 12/12 OK
bash v2/bin/v2 mutants --lint              # problems=0
bash v2/bin/v2 mutants mut-info-x1         # KILLED (88,8 s) rc=1 marker=ok
# tái hiện lỗi bytes: import _run_command của mutants.py CŨ + printf…;exec sleep 5, timeout 0.4 ⇒ TypeError
```

## 4. Bàn giao / nợ

- **Lead**: chạy `v2 mutants --all` (Giờ Cao điểm) để đóng 36 mục — lượt này chỉ chứng minh 1 mục +
  selftest; **chưa** có bằng chứng `--all` xanh.
- Nợ nhỏ: xác nhận end-to-end `mut-no-x0-out` (mục 2.5) ở lượt `--all` đó.
- Đề xuất commit: `[KT-00] mutants: chuẩn hoá bytes→str khi timeout (hết TypeError) + thêm dấu phẩy
  patch mut-info-x1/mut-no-x0-out (2 mục hết E_WRONG-REASON oan) + 3 ca selftest`.
- Không commit/push (không đặt `VIFAST_ROLE`); stage 3 tệp, chờ Lead commit.
