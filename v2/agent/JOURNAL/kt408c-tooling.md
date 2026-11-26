# KT-408c — `mutants` git-tree thật trong bản sao (7/7 KILLED) + tick `Machine::now` riscv64 + 5 `SLOT_EP_*` netsrv

- **Task**: `KT-408c` (P1, Chặng 2 / W7a) — `v2/agent/QUEUE.md:455`; cơ sở `D2-261` (APPROVED 2026-11-26 Agent 0).
- **Người thi hành**: sub-agent **slot 3** (`QUEUE.md:543`); worktree đo được `/private/tmp/vifast-wt/2` (`# worktree /private/tmp/vifast-wt/2 · commit ca437fa886…`), ảnh `target-task/slot-2`.
- **Nhánh / commit / base**: `task/kt408c-tooling` @ **`baec4f59`**, parent `ca437fa8` (= base `main`). `git diff ca437fa8..baec4f59 --stat` = **8 tệp, +588 / −11**.
- **Khung giờ**: Khe Thấp điểm trưa `04:00–05:50 UTC` (`STATE.md`); mutants `04:07:44 → 04:27:13 UTC`, pmemsave `04:32:58` / `04:33:21 UTC`, `FINAL ROUND 2026-11-26T04:38:57Z`, commit `04:40:13 UTC` (`13:40:13 JST`), T1 xong `05:15 UTC` (`14:15 JST`).
- **Log bằng chứng (chép cùng commit)**: `kt408c-summary.log`, `kt408c-mutants-run.log`,
  `kt408c-pmemsave-before.log`, `kt408c-pmemsave-after.log`, `kt408c-ktest-final.log`.
- **Ngân sách**: **2,5 h** (`QUEUE.md:455`); thời lượng thực **chưa kiểm** (không có log mốc mở slot).
## 1. Phạm vi / DoD đã duyệt (`D2-261`)
(1) Vá `v2/tools/mutants.py` — tạo **git tree tạm trong bản sao** sau khi áp đột biến để 5 mục `ktest-riscv64`
qua `rev-gate` **thật** (không nới cổng) + 2 `marker` ngân sách cho `mut-big-bss`/`mut-pt-bss`, chứng minh
**7 mục cũ `E_WRONG-REASON` chuyển `KILLED`**; (2) sửa lỗi thật `Machine::now` luôn `0` trên `riscv64`, giữ
`D2-49(h)` tick đơn điệu, `pmemsave-ring.sh` ghi `t_ns` **tăng thật**; (3) mở rộng `check-boot-abi.py` canh 5
`SLOT_EP_*` của `netsrv_prod.rs` ↔ `proc_net.rs`; (4) `--bkl` do **Lead** nối lúc merge. 0 sửa `spec/`,
0 chạm `x86_64`; T2 (Lead) **PASS 81/81** @ `0f26bebc` (648,4 s; lượt 2 — lượt 1 đỏ `persist` do `T-INFRA`).
## 2. Đỏ-trước (D2-05)
`KT-408b` §6 mục 4 (dòng 244) ghi nợ: "`Machine::now` **luôn 0** trên ảnh riscv64 ⇒ `t_ns` của mọi bản
ghi trace = 0 … mất trục thời gian". Vá `pmemsave-ring.sh` **in đúng** mốc đầu (trước đây in hằng `0`, che
mất thứ cần chứng minh) rồi chạy trên ảnh **chưa vá** (`/tmp/kt408c-logs/pmemsave-before.log`):
```text
# records: n=20 hart=0 kinds=[2, 3] t_ns=[0..0]
Traceback (most recent call last):
  File "<stdin>", line 49, in <module>
AssertionError: t_ns KHÔNG tăng (mọi bản ghi cùng mốc 0) — `Machine::now` không được cấp tick (KT-408c)
pmemsave-ring.sh: kiểm cấu trúc ring trong tệp pmemsave FAIL
# pmemsave-ring.sh rc=1 tag=before out=/tmp/kt408c-trace-before
```
Đỏ **đúng lý do** (20 bản ghi 32 B giải mã được, ring/`heads` thật `3 → 20`, nhưng `t_ns` bất động) — không phải đỏ vì ring hỏng. Sau vá, **cùng** phép kiểm xanh: `# records: n=20 hart=0 kinds=[2, 3] t_ns=[12243911800..12318312000]` / `== pmemsave-ring: PASS … ==` / `# pmemsave-ring.sh rc=0 tag=after out=/tmp/kt408c-trace-after`.
## 3. Việc đã làm
1. **`v2/tools/mutants.py` +103 — `_git_snapshot()`**: sau khi áp đột biến lên **bản sao**, `git init && add -A && commit` **trong bản sao** với danh tính/ngày **cố định** (`vifast-mutants`, `2026-01-01`, `GIT_CONFIG_NOSYSTEM=1`, `core.hooksPath=/dev/null`, có `env.pop("VIFAST_ROLE")`); copy `.gitignore` gốc + ghim 2 liên kết `v2/kernel/target`/`v2/target`; **guard**: `if os.path.lexists(copy_root/.git): return False, "copy-has-git"` — thà mất verdict `E_WRONG-REASON` còn hơn ghi vào kho thật. Verdict có thêm `git=HEAD`.
2. **`v2/test/mutants.toml` ±30**: `marker = "budget-(aarch64|x86_64): .*VƯỢT"` cho `mut-big-bss`/`mut-pt-bss`; **suite đúng** cho `mut-df-round-up` + `mut-no-netsrv` → `product-riscv64` (mã đột biến chỉ biên dịch ở ảnh sản phẩm); 3 `patches.find` **trôi** sau KT-346 (`build.rs` dùng `cargo::cfgs`) — `mut-info-x1`, `mut-net-one-caller`, `mut-no-x0-out`.
3. **`Machine::now` (sửa lỗi thật)**: `machine.rs` +41 — `set_now(now)` **đơn điệu** (nhỏ hơn ⇒ giữ nguyên) + xử lý biên nạp lại `ScBudget::refill_due` cho **mọi** `SchedContext` sống, `now_ticks()`; `plat-riscv64/src/core_bridge.rs` +17 — `with_machine` gọi `m.set_now(now_ns())`, `now_ns()` = `Riscv64Virt.now_ns()` (`rdtime` ×100, **một** định nghĩa quy đổi).
4. **Test host**: `vifast-core/src/kt408c_tests.rs` (**mới** 124 dòng, `#![cfg(test)]`) — 3 ca: đơn điệu/idempotent, `t_ns` **tăng thật** theo `now` đúng offset `t_ns@0`, và biên `refill_due` giữ `check_budget` xanh.
5. **`v2/tools/check-boot-abi.py` +268**: `NETSRV_SLOTS` 5 hằng ↔ bảng copy cap của `proc_net.rs` (fail-closed: thiếu/trùng/ô 0/parse hỏng), `--selftest` 13 → **18** ca.
## 4. Bằng chứng
**7/7 `KILLED`** — `/tmp/kt408c-logs/SUMMARY.txt` §2 (nguyên văn 2 dòng, 5 dòng còn lại cùng khuôn):
```text
ok - mut-big-bss: KILLED (10.4 s) copy=/tmp/kt403-mutants-ta5aow_d/mut-mut-big-bss sites=2 git=HEAD rc=1 marker=ok log=…
ok - mut-no-netsrv: KILLED (217.3 s) copy=/tmp/kt403-mutants-u5df9hov/mut-mut-no-netsrv sites=2 git=HEAD rc=1 marker=ok log=…
```
| Mục | KILLED | sites | Mục | KILLED | sites |
|---|---:|---:|---|---:|---:|
| `mut-big-bss` | 10,4 s | 2 | `mut-leak-unlock` | 165,3 s | 11 |
| `mut-pt-bss` | 9,0 s | 8 | `mut-no-lock` | 163,3 s | 13 |
| `mut-no-irq` | 166,8 s | 3 | `mut-no-netsrv` | 217,3 s | 2 |
| `mut-df-round-up` | 36,1 s | 1 |  |  |  |
**Hai mục `SURVIVED` ở lượt chạy ĐẦU** — `/tmp/kt408c-logs/mutants-run.log` (giữ nguyên, không giấu):
```text
not ok - mut-no-netsrv: SURVIVED (161.1 s) copy=/tmp/kt403-mutants-8u007cd1/mut-mut-no-netsrv sites=2 git=HEAD rc=0 marker=KHONG
not ok - mut-df-round-up: SURVIVED (150.3 s) copy=/tmp/kt403-mutants-u74cyh3z/mut-mut-df-round-up sites=1 git=HEAD rc=0 marker=KHONG
```
Nguyên nhân (theo chú thích `mutants.toml` trong chính commit): suite `ktest-riscv64` **không biên dịch** nhánh bị đột biến ⇒ "SURVIVED oan"; đổi sang `product-riscv64` rồi chạy lại ⇒ `KILLED`. Log **không** in lý do — phần suy ra này dựa trên manifest, **chưa kiểm** bằng log.
**Công cụ** — `/tmp/kt408c-logs/FINAL.txt`: `# mutants lint: mutants=33 feature=32 declarations=52 covered=52 leftover=0 suites=11 problems=0`; `== SELFTEST mutants: OK — 9/9 ca (đỏ-trước cho chính công cụ) ==`; `check-boot-abi: selftest 18/18 ok`; `check-boot-abi: kernel 17 EP_* + 8 SLOT_*/LOOP_* + 2 mảng; userland 17 const EP_* trong 4 tệp; 16 bản sao khớp tên+giá trị; netsrv 5 ô SLOT_EP_* khớp bảng copy cap; 0 lệch`.
**`ktest` 9/9 trên bản sao sạch** — `/tmp/kt408c-logs/ktest-final.log` (`# bản sao /tmp/kt408c-verify2 @ b6f786e dirty=0`): `REV-GATE: ACCEPT inputs=80dac65e7bd423f6 current_inputs=80dac65e7bd423f6 head_rev=b6f786efeebd count=466`; `# KTEST: 9/9 boot sạch (smp=1, NBOOT=1, ảnh/boot=ktest ktest-guard ktest-user ktest-proc ktest-vblk ktest-con ktest-exec ktest-ipc ktest-netsrv)`; `# ktest.sh: PASS`.
## 5. T0 / T1
- **`v2 check`: PASS** — `/tmp/t1-kt408c.log:2` `== v2 check (HEAD baec4f59) ==` → dòng 53 `v2 check: PASS (L1 chạy trong gate --fast)`.
- **T1 `gate --dev`: PASS** (`/tmp/t1-kt408c.log`): `# gate-steps registered=82 ran=68 skipped=14 failed=0`;
  `# gate-wall 832308 ms (832.3 s)`; `v2 gate --dev: PASS (T1 — kiểm tích hợp nhanh, **KHÔNG** phải full gate;
  full gate = 'gate --fast')`; `T1_RC=0`. (Lượt chạy trước đó, lúc tôi bắt đầu viết journal, log còn dừng ở
  `# làn sau5/sau6 …` — **không** có kết quả cuối; số liệu trên là lượt đã chạy xong.)
- **T0 host 3 ca `kt408c_*`: chưa kiểm** — `--- L0 test: OK` trong T1 **không** chạy ca nào của `vifast-core`
  (log không có `kt408c_`, chỉ doc-test + 0-test binary), và không có log `cargo test` riêng cho crate này.
- **T2 (Lead)**: **PASS 81/81** trên ứng viên `0f26bebc` (648,4 s); lượt 1 đỏ **một** bước `persist`
  (`T-INFRA` host timing — xem Q7 trong `BLOCKED.md` §1b), chạy lại một lượt ⇒ PASS.
- **Đã merge vào `main`**: `296a294f [KT-408c] merge: git-tree thật cho mutants (7/7 KILLED) + Machine::now
  riscv64 + check-boot-abi 5 SLOT_EP` (`2026-11-26 14:15:25 JST` = `05:15:25 UTC`); **T2 trên ứng viên tích
  hợp: **PASS 81/81 @ `0f26bebc`** (648,4 s; xem §5).
## 6. Rủi ro + việc còn lại (trung thực)
1. **Sự cố git snapshot trong `/tmp` ghi vào repo THẬT** (nghiêm trọng nhất): `_git_snapshot` đời đầu gặp bản
sao có `.git` trỏ về kho thật ⇒ sinh commit **`217ca6ca` "KT-408c verify snapshot"** (author
`vifast-kt408c <kt408c@vifast.invalid>`, date `2026-01-01`, parent `ca437fa8`, **10 tệp +581/−12**, gồm cả
`.gitignore` và `v2/bin/lib/steps_check.sh`) **trong repo thật**. Đã xử lý: `git reset --mixed ca437fa8`
(reflog: `refs/heads/task/kt408c-tooling@{1}: reset: moving to ca437fa8`), **thêm guard `.git`** (§3.1), và
`baec4f59` **không** chứa `.gitignore`/`steps_check.sh`. Hiện `git status --porcelain` chỉ còn **19 mục
untracked có sẵn**, **0** tệp tracked bị sửa; `217ca6ca` là commit **treo** (không nhánh nào chứa, không là
tổ tiên của `main`).
2. **Bằng chứng `ktest` cũ gắn `head_rev=217ca6cada71`** (`SUMMARY.txt` §5) ⇒ **không hợp lệ**; đã chạy lại trên bản sao mới **`b6f786ef`** cùng fingerprint **`80dac65e7bd423f6`** ⇒ `ktest-final.log` ở §4. Khi trích bằng chứng phải dùng bản sau.
3. **3 đột biến "revive" chưa chạy**: **chưa đối chiếu được** — không có chuỗi `revive` trong `mutants.toml`/`mutants.py`/log `/tmp`, nên tôi ghi nguyên văn chỉ đạo của Lead và **không** tự suy ra số.
4. **Chưa chạy `v2 mutants --all`**: chỉ chạy 7 mục mục tiêu; 33 mục khai báo còn lại không được chạy lại trong task này ⇒ chưa chứng minh `--all` sạch.
5. **3 ca host `kt408c_*` chưa có log riêng** ⇒ **chưa kiểm** (T1 PASS nhưng bước `L0 test` không chạy ca nào của `vifast-core`; phải chạy `cargo test -p vifast-core kt408c` mới có bằng chứng).
6. **`MEASUREMENT_INVALID`**: `ktest-final.log` ghi `# MEASUREMENT_INVALID tag=before ly_do=qemu_foreign=1` + `WARNING: phép đo [before] KHÔNG HỢP LỆ (qemu_foreign=1) — số THỜI GIAN không dùng để kết luận (D2-116/OD-K6-6)`; `infra after … qemu_foreign=0 … verdict=OK`. Kết luận PASS/FAIL giữ nguyên, **số thời gian** thì không.
7. **T2 (Lead) PASS 81/81 @ `0f26bebc`**; phần (4) `--bkl` do Lead làm trên `main` ở **`885a9173`**, **không** nằm trong `baec4f59`.
8. **Bằng chứng thô còn ở `/tmp`**: `kt408c-trace-before/`, `kt408c-trace-after/` (`pmemsave-riscv64-ring.bin` 8192 B + `heads-counts.bin` 128 B + `.license` + log 4088 B) và `kt408c-logs/`; `/tmp` bị dọn là **mất bằng chứng** ⇒ phải chép vào `v2/agent/JOURNAL/` trước khi kết thúc task.
## 7. Tệp đã sửa / xoá / mới
| Tệp | Loại | Δ |
|---|---|---|
| `v2/kernel/vifast-core/src/kt408c_tests.rs` | **mới** | +124 (3 ca host) |
| `v2/kernel/vifast-core/src/machine.rs` | sửa | +41 (`set_now`/`now_ticks`) |
| `v2/kernel/vifast-core/src/lib.rs` | sửa | +4 (`mod kt408c_tests`) |
| `v2/kernel/vifast-plat-riscv64/src/core_bridge.rs` | sửa | +17 (`now_ns`, `set_now`) |
| `v2/tools/mutants.py` | sửa | +103 (`_git_snapshot` + guard) |
| `v2/tools/check-boot-abi.py` | sửa | +268 (5 `SLOT_EP_*`) |
| `v2/test/mutants.toml` | sửa | ±30 (2 marker, 2 suite, 3 `find`) |
| `v2/test/boot/pmemsave-ring.sh` | sửa | ±12 (in `first` thật + assert) |
