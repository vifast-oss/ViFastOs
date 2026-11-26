# KT-413c — Gỡ trọn 2.162 LOC `sched.rs` `x86_64` → `Machine::dispatch(hart)` + 24 mục `# sched:` mới

- **Task**: `KT-413c` (P1, Chặng 2 / W7a) — `v2/agent/QUEUE.md:471`; cơ sở `D2-261` (APPROVED 2026-11-26 Agent 0).
- **Người thi hành**: sub-agent **slot 2** (`QUEUE.md:542`); worktree đo được `/private/tmp/vifast-wt/1`, ảnh `target-task/slot-1`.
- **Nhánh / commit / base**: `task/kt413c-sched` @ **`694f9d52`**, parent `ca437fa8` (= base `main`). `git diff ca437fa8..694f9d52 --stat` = **17 tệp, +897 / −3756**.
- **Khung giờ**: Khe Thấp điểm trưa `04:00–05:50 UTC` (`STATE.md`); log sớm nhất `04:28:09 UTC`, T1 xong `05:01:13 UTC`, commit `04:49:50 UTC` (`13:49:50 JST`). Mốc mở slot: **chưa kiểm**.
- **Log bằng chứng (chép cùng commit)**: `kt413c-runner-final.log`, `kt413c-mutant.log`,
  `kt413c-soak-selftest.log`.
- **Ngân sách**: **3 h** (`QUEUE.md:471`); thời lượng thực **chưa kiểm** (không có log mốc bắt đầu).
## 1. Phạm vi / DoD đã duyệt (`D2-261`)
Xoá sạch `kbin-x86_64/src/sched.rs` (1.376 LOC) + `plat-x86_64/src/sched.rs` (786) = **2.162 LOC**, thay
"ai chạy tiếp" bằng `Machine::dispatch(hart)` thật của `vifast-core`; **giữ TAP = 50** và đủ **12 ca âm có
răng (n..z)** trên định dạng `# sched: tcb=<ObjId> hart=<n> prio=<p> …`; tái dùng `# core smp:` của
`smp_suite::run_x64` (KT-413b), **không** viết bản thứ hai; thêm 1 đột biến `mut-dispatch-*`; **giữ** 2
profile `smp_suite` + `CR4.PCIDE` TẮT; 0 sửa `spec/`, 0 chạm `rv`/`a64`. T2 (Lead) **PASS 81/81** @ `0f26bebc` (648,4 s; lượt 2 — lượt 1 đỏ `persist` do `T-INFRA`).
## 2. Đỏ-trước (D2-05)
Kịch bản kiểm mới (`run-x86_64.sh::check_sched`) trích thành `/tmp/red-sched-check.sh`, chạy trên **log boot của ảnh TRƯỚC thay đổi** (`/tmp/old-sched.log`, 13 dòng `# sched:` kiểu cũ):
```text
$ bash /tmp/red-sched-check.sh old-sched.log      # người viết journal chạy lại, read-only
RED: dispatch lines=0 (cần 8)
RED: thiếu lượt dispatch đầu prio=3
RED: thiếu tổng kết ticks=8/tcbs=4/wakes=4
RED: thiếu block state
RED: thiếu wake state
RED: thiếu progress
RED: thiếu prio first=t<ObjId>
RED: thiếu sleep wakes=4
RC=1
```
Đỏ **đúng lý do** ("hợp đồng bằng chứng mới chưa tồn tại"), không phải đỏ vì assert: log cũ chỉ có
`# sched: switch a→b n=1`, `# sched: ticks=10 tasks=4`, `# sched: umode task ran n=… ring=3 …`,
`# sched: multi-hart harts=4 rounds=8 picks=[2, 2, 2, 2] first=[a, b, u, c] diff=3 double=0`,
`# sched: prio first=b viol=0 ticks=8 hard=0 picks=[2, 2, 2, 2]`. Đối chứng dương trên log **sau**
(`/tmp/new-sched-full.log`): `PASS: đủ hợp đồng # sched: mới`, `RC=0`.
## 3. Việc đã làm
1. **Xoá 2 tệp mô hình tự viết**: `kbin sched.rs` (−1.376: `TASK_SEQUENCE`, hàng đợi `MH_*`, 4 stack 16 KiB, preempt/umode counter) + `plat sched.rs` (−786: layout frame, `next_task`, `mh_*`). `FrameReturn` **chuyển** sang `plat-x86_64/src/exc.rs` (+28), chỉ còn `resume()`/`is_halt()` — nhánh `switch(rsp, rip)` gỡ cùng mô hình.
2. **`kbin-x86_64/src/dispatch.rs` (mới, 556 dòng)**: 8 lượt `Machine::dispatch(0)` trên **TCB thật** (`configure`/`resume`/`BlockedOnNotification`/`signal`) ⇒ TAP 22..25 (`sched: two tasks ran`, `sched: preempted`, `sched: block wake`, `sched: multi-hart`) + `report()` cho TAP 29/30. Pha 4 hart **không** chạy lại — dùng `# core smp:` của `smp_suite::run_x64`.
3. **Gỡ đường ngắt→scheduler**: `trap.rs` −16 (bỏ `sched::on_tick`/`on_tick_ap` + `FrameReturn::switch`); `umode.rs` −21 (bỏ `USER_TASK_RSP`, `sched_entry_va`, `__user_sched_entry`); `net.rs`/`main.rs`/`product.rs` đổi điểm gọi sang `crate::dispatch::{run,report}`.
4. **`core_sched.rs` +6**: móc `#[cfg(not(feature = "mut-dispatch-no-preempt"))]` quanh `preempt_current` (đột biến `kind = "feature"` lật cfg trên **bản sao**, không khai báo feature ở `Cargo.toml`).
5. **Runner**: `run-x86_64.sh` viết lại `check_sched()` (8 dòng dispatch + `prio=3 n=1` + `prio=2 n=3` + tổng kết + block/wake + `progress distinct=4` + `hart=0 picked`); 12 ca âm (n,p,q,r,s,t,u,v,w,x,y,z) trỏ định dạng mới; `soak-x86_64.sh` cùng bộ (mục 10/10b/10c/10e).
6. **Đột biến**: `mut-dispatch-no-preempt` (`suite = "ktest-x86_64"`, `wired = true`), `declarations` 52→53. Đo lại: `grep -c 'sched:'` — `run-x86_64.sh` **69→65**, `soak-x86_64.sh` **64→62** (con số "64 dòng soak" trong commit message là **số dòng phải viết lại**, đo trước khi sửa).
## 4. Bằng chứng
**Boot thật `/tmp/new-sched-full.log` (nguyên văn, 15 dòng = 8 lượt dispatch + 7 dòng tổng kết):**
```text
# sched: tcb=t4 hart=0 prio=3 state=Running n=1
# sched: tcb=t4 hart=0 prio=3 state=Running n=2
# sched: tcb=t5 hart=0 prio=2 state=Running n=3
# sched: tcb=t5 hart=0 prio=2 state=Running n=4
# sched: tcb=t4 hart=0 prio=3 state=Running n=5
# sched: tcb=t5 hart=0 prio=2 state=Running n=6
# sched: tcb=t3 hart=0 prio=1 state=Running n=7
# sched: tcb=t6 hart=0 prio=0 state=Running n=8
# sched: block tcb=t4 hart=0 state=BlockedOnNotification
# sched: wake tcb=t4 hart=0 state=Ready
# sched: ticks=8 tcbs=4 preempts=3 wakes=4 viol=0 sleep_viol=0
# sched: progress picks=8 distinct=4 delta=8
# sched: hart=0 picked=t4 n=3
# sched: prio first=t4 viol=0 ticks=8 preempts=3 picks=[1, 3, 3, 1]
# sched: sleep wakes=4 viol=0 ticks=8
```
**Runner `x86_64` — `/tmp/runner-kt413c-final2.log` (`EXIT=0`):**
```text
ok 6 - boot 1: TAP 50/50 + 6 dòng '# exc:' + marker 1 lần + halt sạch
ok 1 - ca âm (n): mất dòng '# sched:' ⇒ ĐỎ (# dòng '# sched: tcb=t<ObjId> hart=0 prio=<p> state=Running n=<k>' xuất hiện 0 lần, cần đúng 8)
ok 2 - ca dương (KT-413c): picks=[2, 4, 1, 1] first=[t4, t4, t1, t3] ⇒ XANH   ← đối chứng dương của runner, KHÔNG phải boot thật
# ca âm: 33/33 ĐỎ đúng như phải (+ 1 ca dương KT-413c)
# 39/39 kiểm sạch (boot=1, tests=50, exc=6)
```
**Soak `--selftest` — `/tmp/soak-selftest-final.log`** (chỉ selftest, **không** soak thật): `ok 28 - ca âm (thiếu dòng dispatch đầu (prio=3)) ⇒ ĐỎ (# dòng '# sched: tcb=… hart=0 prio=<p> state=Running n=<k>' xuất hiện 7 lần, cần đúng 8)` · `# soak-x86_64 --selftest: 58/58 ca đúng như phải (2 dương + 56 âm)`.
**Đột biến — `/tmp/mutant-kt413c.log`:** `ok - mut-dispatch-no-preempt: KILLED (318.6 s) … sites=1 rc=1 marker=ok` + `# mutants: chạy=1 killed=1 survived=0 wrong-reason=0 stale=0 chưa-nối=0`.
**`.bss` / ngân sách (`runner-kt413c-final2.log:9`)**: `# ktest # image=x86_64-ktest … file=340448 data+bss=102080 budget=262144 … smp_stacks=557056 bootpt=294912 bootstacks=20480 corespace=196608 … noload=1069056`.
## 5. T0 / T1
- **`v2 check`**: `v2 check: PASS (L1 chạy trong gate --fast)` (`t1-kt413c.log:53`, `HEAD 694f9d52`).
- **T1 `gate --dev`: PASS** (`/tmp/t1-kt413c.log`): `# gate-steps registered=82 ran=68 skipped=14 failed=0`; `# gate-wall 672851 ms (672.9 s)`; `v2 gate --dev: PASS (T1 — kiểm tích hợp nhanh, **KHÔNG** phải full gate; full gate = 'gate --fast')`; `T1_RC=0`.
- **T0 `v2 t0 vifast-kbin-x86_64`: FAIL — nợ có sẵn** (`/tmp/t0-kbin-final.log`): fmt OK; clippy `18 previous errors` (bin) / `1` (test); `E0152: found duplicate lang item panic_impl` (`no_std` bin + harness `std`). Bản sao cây HEAD **trước** thay đổi (`/tmp/clippy-head-baseline.log`) đỏ **cùng loại** với `19` (bin) / `10` (test) lỗi ⇒ **không** phải hồi quy (giảm sau khi xoá `sched.rs`).
- **T2 (Lead)**: **PASS 81/81** trên ứng viên `0f26bebc` (648,4 s). Lượt 1 đỏ **một** bước `persist`
  (`13` bản ghi `/pstate` thay vì 11–12) — `T-INFRA` host timing: 4 lần chạy riêng PASS, boot-sau-crash
  + `fsck` xanh; D2-07 → chạy lại **một** lượt ⇒ PASS.
- **Đã merge vào `main`**: `3bf315f5 [KT-413c] merge: xoá 2.162 LOC sched.rs x86_64 → Machine::dispatch(hart)
  trên TCB thật; 50 TAP + 12 ca âm; -64 KiB .bss` (`2026-11-26 14:15:24 JST` = `05:15:24 UTC`), merge **sau**
  khi tôi chốt T1. **T2 trên ứng viên tích hợp: PASS 81/81 @ `0f26bebc`** (648,4 s; xem §5).
## 6. Rủi ro + việc còn lại (trung thực)
1. **Mất phủ "preempt ring-3" trong kịch bản dispatch**: TAP 24 đổi nhãn `sched: umode task` → `sched: block wake` (`sched.rs@694f9d52^` so với `dispatch.rs@694f9d52`). Bằng chứng `ring=3 preempted=… resumed=… cs=0x1b` **không còn**; ring 3 nay chỉ còn dòng `# umode: ring3 ran nr=<n> ret=<v>` (mẫu runner đòi), **không** chứng minh preempt **khi đang ở CPL3**. Số TAP / `EXPECT_TESTS=50` không đổi.
2. **Xoá 1.169 dòng test host cùng logic**: `plat-x86_64/tests/{priority,runqueue,sched}.rs` = 8+10+10 = **28 ca**; chúng kiểm **mô hình đã xoá** nên không thay thế được, nhưng tổng ca host của `vifast-plat-x86_64` **giảm 28** và không có test host mới bù.
3. **Soak chỉ `--selftest 58/58`, chưa soak thật**: 64 dòng `# sched:` đã viết lại nhưng **chưa** chạy nhiều boot ⇒ chưa biết `preempts=3`/`picks=[1, 3, 3, 1]` có ổn định qua N boot (bài học KT-308/KT-313: 5/600 boot đỏ).
4. **`-64 KiB` trong commit message là `.bootstacks`, không phải `.data+.bss`**: 4 stack 16 KiB ở `#[link_section = ".bootstacks"]` (`sched.rs@694f9d52^` dòng 137/151-169), `.bootstacks` là **NOLOAD**, **không** cộng trần (`linker.ld:163-168`, `budget-x86_64.sh:338`). Đo **sau**: `bootstacks=20480`, `data+bss=102080`. **Trước** trên cùng cây: **chưa kiểm**; `bootstacks=86016` chỉ thấy ở log cây khác ⇒ **không** dùng làm delta.
5. **`mutants lint` chưa chạy cho nhánh này** (`declarations` 52→53, không có log lint) — **chưa kiểm**; chỉ chạy **1** đột biến, **không** `--all`.
6. **T2 đã PASS trên ứng viên `0f26bebc`**; T1 chạy trên `694f9d52` (chưa gộp `161ed302` của nhánh hardening — `check_abi` selftest ở cây này `13/13`, không phải `19/19`) ⇒ T2/merge phải kiểm lại trên ứng viên tích hợp.
7. **Ngoại cảnh**: runner ghi `qemu_own=0`; 3 boot ca âm (`k`,`o`,`cc`) **không tự thoát** (pid 42586/42943/43439) — đúng thiết kế runner, nhưng PID ấy phải dọn trước đợt sau (AGENTS §9).
## 7. Tệp đã sửa / xoá / mới
| Tệp | Loại | Δ |
|---|---|---|
| `kbin-x86_64/src/dispatch.rs` | **mới** | +556 |
| `kbin-x86_64/src/sched.rs`, `plat-x86_64/src/sched.rs` | **xoá** | −1.376, −786 |
| `plat-x86_64/tests/{priority,runqueue,sched}.rs` | **xoá** | −1.169 (28 ca) |
| `plat-x86_64/src/exc.rs` / `lib.rs` | sửa | +28 / ±7 |
| `kbin-x86_64/src/{main,net,product,trap,umode}.rs` | sửa | ±5 / ±4 / ±2 / ±16 / ±21 |
| `vifast-core/src/core_sched.rs` | sửa | +6 (móc đột biến) |
| `v2/test/boot/{run,soak}-x86_64.sh` | sửa | ±388 / ±270 |
| `v2/test/mutants.toml` | sửa | ±19 (`mut-dispatch-no-preempt`; 53) |
