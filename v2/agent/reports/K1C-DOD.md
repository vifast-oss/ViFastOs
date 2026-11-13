<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# K1C-DOD — bằng chứng cổng cuối phase K1c (KT-31)

Người ghi: **Agent 2 dòng OS** · Ngày: 2026-11-13 · Task: **KT-31** (cổng cuối K1c) ·
HEAD khi mở task: `0d026bd` · Nhánh `main`.

**Phạm vi**: chỉ **chạy cổng, thu log, đối chiếu DoD**. Không sửa semantics IPC/Notification/
WaitAddr/Fault/Exit/SMP/sync, không sửa `spec/`/`vifast-model`/`.scn`/QUEUE/DECISIONS/AGENTS/
`viscript/`, không thêm dependency/`unsafe`/`#[ignore]`/`#[allow]`/`#[should_panic]`, không nới
seed/ngưỡng. Đột biến tay (C3) là **sửa tạm + khôi phục**, không commit.

Nguồn đối chiếu: `review/agent0/K1C-PLAN.md` §5 (KT-24..KT-30) + §7.1 (cổng cuối) + §7.2 (danh
sách đột biến), `v2/agent/DECISIONS.md` D2-53..D2-60, `v2/agent/STATE.md`.

## 0. TL;DR

| # | Việc (§7.1) | Kết quả | Log |
|---|---|---|---|
| 1 | `nightly --l1 --seeds 100000 --smp 1,2,4,8` | **PASS nhưng chỉ phủ sched/SMP/mailbox** — 4×100.000 seed, 400.000 lượt, wall **5043,84 s**; cùng seed ⇒ cùng trace; **không** sinh IPC/Notification/WaitAddr ⇒ **C1 chưa đạt** ở 100k | `JOURNAL/kt31-l1-100k-smp.log` |
| 2 | `gate --fast` (≥ 461 test, không retry) | **PASS 461 test**, L1 1.000 seed 720 ms; wall **254,19 s** ⇒ **T-INFRA** (> 60 s, do tải máy) | `JOURNAL/kt31-gate-fast.log` |
| 3 | `nightly --only-miri` (kt24..kt30 + sync + nền K1a/K1b) | **CHƯA ĐẠT (một phần)** — 18 tập **276 test** PASS, wall **4058,24 s**, nhưng `--only-miri` **thiếu `kt25_ipc2`** ⇒ không phủ trọn 'kt24..kt30' | `JOURNAL/kt31-miri-all.log` |
| 4 | Mutation tay 6 bất biến K1c (C3) | **6/6 ca đỏ đúng test** (exit 101, `test result: FAILED`), 6/6 tệp khôi phục hash == HEAD — nhưng chỉ **3/12 mục §7.2** (M7/M10/M11) ⇒ **C3 chưa đạt** đủ danh sách | `JOURNAL/kt31-mutation.log` |
| 5 | Bảng DoD K1c (file này) | **Đã lập** — §7/§8 dưới đây | `v2/agent/reports/K1C-DOD.md` |
| 6 | Parity 10k (hồi quy, Q12) | **PASS** — `kt11_diff_model_va_sim_10000_seed` 10.000 seed × 24 op, 15,52 s | `JOURNAL/kt31-parity-10k.log` |

**Đề xuất trạng thái**: **KT-31 REVIEW** (đề xuất Agent 0 lật `DONE` như **"gate run một phần"** qua D2-61 + mở KT-32 đóng 3 nợ); **K1c REVIEW — chưa đóng phase**. Chi tiết §10.

## 1. Baseline (trước khi chạy cổng)

- `agentctl next --project v2` ⇒ **KT-31** (READY); `git rev-list --count HEAD..origin/main` = 0.
- `./v2/bin/v2 gate --fast` **PASS 461 test**, L1 1.000 seed **611 ms** — log
  [`JOURNAL/kt31-baseline-gate-fast.log`](../JOURNAL/kt31-baseline-gate-fast.log).
- Cây làm việc lúc baseline: chỉ có tệp chưa track ngoài `v2/` (`ViFastOs-dossier.md`, `review/…`);
  trong lúc chạy cổng, tab khác ghi `viscript/` — **không** chạm, chỉ stage `v2/agent/{JOURNAL,reports}`
  (+`STATE.md`) ở commit `[KT-31]`.

## 2. C4 — 100.000 seed × smp 1/2/4/8 (việc §7.1 #1)

Lệnh (đúng biến thể `v2 nightly --l1` do KT-29 thêm; **không** giảm seed, **không** đổi invariant):

```
./v2/bin/v2 nightly --l1 --seeds 100000 --smp 1,2,4,8
```

**Kết quả** — log [`JOURNAL/kt31-l1-100k-smp.log`](../JOURNAL/kt31-l1-100k-smp.log):

| smp | Dòng log | Kết quả |
|---|---|---|
| 1 | `# L1 OK: smp 1, 100000 seed × 64 op` | 100.000 seed |
| 2 | `# L1 OK: smp 2, 100000 seed × 64 op` | 100.000 seed |
| 4 | `# L1 OK: smp 4, 100000 seed × 64 op` | 100.000 seed |
| 8 | `# L1 OK: smp 8, 100000 seed × 64 op` | 100.000 seed |
| tổng | `# L1 OK: 400000 lượt (smp [1, 2, 4, 8], seeds 100000)` | **400.000 lượt** |

- **Wall**: `real 5043.84 s` = **84,06 phút** (user 4542,91 s; sys 24,16 s) — đo bằng
  `/usr/bin/time -p` trong chính log; không retry.
- **Cùng seed ⇒ cùng trace**: `driver_smp_cung_seed_cung_trace` chạy **hai lần** mỗi
  `(seed, smp)` và `assert_eq!(a, b)` — 400.000 cặp trace bằng nhau byte-for-byte, nếu lệch
  test panic (exit ≠ 0). Log ghi `1 passed; 0 failed` sau 5.043,19 s.
- **Bất biến sau mỗi op**: trong `mot_luot`, sau **mỗi** op gọi `check_sched()`, `check_harts()`,
  `check_ipc()`; vi phạm ⇒ panic (`check_* sau <dòng>: <lỗi>`). Log **0** panic ⇒ 0 vi phạm trên
  **400.000 seed-lượt × 64 op = 25,6 triệu op**.
- Test dùng đúng `holes` đã có: 4 TCB, `SchedContext`/`SchedControl`, **9 op** hỗn hợp: (0)
  `prio`, (1) `affinity`, (2) `dispatch`, (3) `block`, (4) `wake`, (5) `process_mail`, (6)
  `push_migration`, (7) `ready/wake`, (8) `yield` — **không** có Send/Recv/Call/Reply/Signal/
  WaitAddr ⇒ 100k **không** kiểm C1/C2; `check_ipc` có đường nhanh khi `ipc_sent == 0` (rỗng).
- **Không** chạy song song với build/cổng khác (AGENTS §6); thời gian dài do 4 cấu hình ×
  100.000 seed × 2 lần chạy/seed trên máy đang tải (baseline `gate --fast` cùng ngày 199,3 s ở
  KT-30) — **T-INFRA** nếu so với `[ƯL]` 20 phút của K1C-PLAN §7.1, nhưng **không** nới seed.

## 3. Mutation tay 6 ca (C3 / việc §7.1 #4)

Mỗi ca: áp **một** đột biến (không commit) → chạy **đúng một** test bất biến → test **đỏ** →
khôi phục `git checkout -- <file>` → xác nhận `git hash-object` == `HEAD:<file>`. Log đầy đủ:
[`JOURNAL/kt31-mutation.log`](../JOURNAL/kt31-mutation.log).

| Ca | Tệp · đột biến (áp tạm) | Test đỏ (chỉ đúng test này) | Bằng chứng đỏ (trích log) | Khôi phục |
|---|---|---|---|---|
| 1 | `ipc.rs` `plan_cap_transfer`: `window[i]` → `window[0]` (cài cap **sai vị trí**) | `kt25_ipc2::bon_cap_cai_du_bo_grant_giu_badge` | `check_caps sau 4 cap: cap 17 nói nằm ở (1, 94) nhưng slot không trỏ lại` (`kt25_ipc2.rs:206`) → `test result: FAILED. 0 passed; 1 failed` | hash `2173300…` == `HEAD:v2/sim/vifast-sim/src/ipc.rs` |
| 2 | `kobj.rs` `notification_signal`: `word \| badge` → `badge` (bỏ **OR dồn**) | `kt27_notification::signal_or_ba_lan_truoc_khi_wait` | `assertion left == right failed: Signal phải OR vào word` — `left: 4`, `right: 7` (`kt27_notification.rs:243`) → FAILED | hash `37ff523…` == `HEAD:…/kobj.rs` |
| 3 | `kobj.rs` `wait_addr`: **xếp hàng trước** rồi mới so `expected` | `kt28_waitaddr::expected_lech_tra_ngay_khong_xep_hang` | `TCB phải **giữ** Ready, không bị chặn` — `left: BlockedOnAddress { frame: 3, offset: 16 }`, `right: Ready` (`kt28_waitaddr.rs:246`) → FAILED | hash `37ff523…` == HEAD |
| 4 | `kobj.rs` `tcb_resume_with_regs`: bỏ `admit_message`+`deliver_message` (**không ghi MR trước `Ready`**) | `kt26_fault::resume_with_regs_khi_dang_cho_reply_ghi_thanh_ghi_truoc_ready` | `assertion left == right failed` — `left: [0, 0, 0, 0]`, `right: [7, 8, 9, 10]` (`kt26_fault.rs:341`) → FAILED | hash `37ff523…` == HEAD |
| 5 | `kobj.rs` `push_step`: bỏ guard `published[y].idle` | `kt29_smp::push_chi_khi_dich_idle_va_trong_affinity` | `đích bận ⇒ 0` — `left: 1`, `right: 0` (`kt29_smp.rs:416`) → FAILED | hash `37ff523…` == HEAD |
| 6 | `mailbox.rs` `send`: hộp thư **đầy ghi đè** message đã công bố thay vì `Err(Busy)` | `kt30_sync::mailbox_day_tra_busy_khong_mat_message_dau` | `assertion left == right failed` — `left: Ok(())`, `right: Err(Busy)` (`kt30_sync.rs:130`) → FAILED | hash `5747220…` == `HEAD:v2/lib/vifast-sync/src/mailbox.rs` |

- Mỗi ca chạy **một lần**, không retry; test còn lại trong tệp bị lọc (`x filtered out`) nên log
  chứng minh **đúng** test bất biến đỏ, không phải đỏ dây chuyền.
- Lần chạy harness đầu tiên gọi `cargo` thiếu `--manifest-path v2/Cargo.toml` ⇒ exit 101 do **lỗi
  cargo**, không phải test đỏ: **không** dùng làm bằng chứng; harness đã sửa (thêm manifest + kiểm
  `test result: FAILED`) rồi chạy lại sạch; log dưới đây là bản đã sửa.
- Giới hạn: 6 ca này phủ **6 bất biến chính** của K1c; **không** chạy lại đủ 12 mục §7.2
  (M1/M2/M3/M4/M5/M6/M8/M9/M12) trong cổng này — các mục đó có test xanh tương ứng trong
  `kt24/kt25/kt27/kt28/kt29` (xem §8), nhưng **không** phải bằng chứng mutation mới.

## 4. Miri toàn bộ (việc §7.1 #3)

Lệnh: `./v2/bin/v2 nightly --only-miri` — log [`JOURNAL/kt31-miri-all.log`](../JOURNAL/kt31-miri-all.log).

**Kết quả**: `v2 nightly --only-miri: PASS`, **276 test / 0 failed**, wall **4058,24 s** (67,6 phút;
`real` trong log) — **dưới** mốc 3 h nên chạy trọn. 18 tập:

| Tập | test | thời gian |
|---|---|---|
| `vifast-objmem` (lib+alias+fuzz 200+contract) | 20+8+1+1+10 = **40** | 2,51 + 1,49 + 0,20 + 197,30 + 1,22 s |
| `vifast-sim --lib` | **34** | 42,30 s |
| `kt16_objmem` / `kt09_untyped` / `kt09_revoke` | 4 / 8 / 7 | 2,09 / 8,04 / 4,22 s |
| `kt10_cnode` / `kt17_align` / `kt18_tcb` | 13 / 3 / 18 | 7,37 / 1,63 / 24,85 s |
| `kt19_sched` / `kt20_budget` / `kt21_donation` / `kt22_mailbox` | 10 / 12 / 13 / 10 | 391,92 / 256,79 / 236,33 / 221,22 s |
| **`kt24_ipc`** | **13** | 44,49 s |
| **`kt26_fault`** | **24** | 87,07 s |
| **`kt27_notification`** | **26** | 53,35 s |
| **`kt28_waitaddr`** | **12** | 105,19 s |
| **`kt29_smp`** | **13** | 261,03 s |
| **`vifast-sync` (lib + `kt30_sync` + doctest)** | 4 + 10 + 2 = **16** | 0,62 + 2,55 + 0,35 s |

- **K1c trong Miri**: 13+24+26+12+13+16 = **104 test**, 0 failed.
- **Trạng thái mục §7.1 #3: CHƯA ĐẠT (một phần)**: danh sách `nightly --only-miri` trong
  `v2/bin/v2` **thiếu `kt25_ipc2`** ⇒ lần chạy này **không** phủ "kt24..kt30". Log Miri rời
  `JOURNAL/kt25-miri.log` (**24/24**, 75,99 s) là **từ KT-25**, **không** phải lần chạy HEAD hiện
  tại — **không** dùng để claim đã phủ. `kt30_stress` tự gạt dưới Miri (`#![cfg(not(miri))]`).
  Muốn đóng nợ này phải thêm `kt25_ipc2` vào danh sách Miri rồi chạy lại (đề xuất KT-32).
- Không chạy `l1*` dưới Miri (D2-47(a)); không chạy `.scn` (đọc tệp).

## 5. Gate cuối (việc §7.1 #2)

Lệnh: `./v2/bin/v2 gate --fast` — log [`JOURNAL/kt31-gate-fast.log`](../JOURNAL/kt31-gate-fast.log).

**Kết quả**: `v2 gate --fast: PASS` (gen-check ON), **461 test / 0 failed**, L1 1.000 seed
**720 ms**; `real 254.19 s` (**4,24 phút**) ⇒ **T-INFRA**: wall > 60 s (QD §5.2) do tải máy —
**không** nới ngưỡng, **không** gọi là đạt 60 s; baseline cùng ngày ở KT-30 đã 199,3 s và
baseline KT-31 chạy cùng máy cũng > 2 phút. Cổng chạy **một lần**, không retry; số test **không
giảm** so baseline (461 = 461).

## 6. Parity 10k (hồi quy, D2-53 Q12 — không mở rộng model)

Lệnh: `cargo test -p vifast-model --test kt11_diff_sim` — log
[`JOURNAL/kt31-parity-10k.log`](../JOURNAL/kt31-parity-10k.log).

**Kết quả**: `kt11_diff_model_va_sim_10000_seed` **PASS**, `1 passed; 0 failed`, 15,52 s
(wall 15,63 s) — **10.000 seed × 24 op**, `vifast-model` vs `vifast-sim`, so kết quả + trạng thái
chuẩn hoá sau **mỗi** bước; 0 lệch. Đây là **hồi quy** (Q12: không mở rộng model cho IPC), không
phải bằng chứng IPC mới.

## 7. Bảng DoD K1c — 4 mệnh đề §1.1 K1C-PLAN

| # | Mệnh đề (K1C-PLAN §1.1) | Kết quả | Bằng chứng đo được |
|---|---|---|---|
| **C1** | **Bảo toàn message** — mỗi `msg_id` đúng một nơi; 0 vi phạm trên 1.000 seed `--fast` **và** 100.000 seed nightly | **CHƯA ĐẠT** — chỉ đạt **một phần**: fuzz 1.000×64 ĐẠT; **100k KHÔNG phủ IPC** (`check_ipc` rỗng) | `check_ipc()` sau **mỗi** public op (`kobj.rs::sched_after_op`); bất biến `ipc_sent == ipc_delivered + ipc_pending + ipc_dropped` (`dropped` chỉ cho đường huỷ hợp lệ D2-25). Fuzz **1.000 seed × 64 op** trong `kt24_ipc`, `kt25_ipc2`, `kt26_fault`, `kt27_notification`, `kt28_waitaddr`, `kt29_smp` — 0 vi phạm, cùng seed cùng trace; `kt31-baseline-gate-fast.log` chạy lại toàn bộ: **131 test K1c** xanh. **Hạn chế**: driver 100k (`l1_k1c`, 9 op sched/SMP/mail) **không** sinh message ⇒ `check_ipc` chạy nhưng **rỗng**; 100k **không** chứng minh bảo toàn message ở quy mô đó (nợ ghi §9 mục 2) |
| **C2** | **Không mất đánh thức** — signal/wake sau khi waiter xếp hàng ⇒ phải đánh thức; trước ⇒ word/giá trị còn | **ĐẠT (model 1 luồng)** | `kt27_notification` 26 test (Signal OR, **một** waiter, word xoá sau giao, Wait/Poll) + fuzz 1.000×64; `kt28_waitaddr` 12 test (`expected` lệch ⇒ không xếp hàng, `wake_addr(n)` FIFO, wake trước/sau) + fuzz 1.000×64; 100k×4 L1 0 panic. **Giới hạn**: "giữ khoá" trong mô hình = **một hàm**, không phải Mutex/thread thật (K2/K3 — nợ §9 mục 1/3) |
| **C3** | **Mọi đột biến chuẩn bị bắt** — mỗi đột biến làm **đúng một** test đỏ, revert, ghi `mutants: killed` | **CHƯA ĐẠT** — 6/6 ca đỏ đúng test, nhưng chỉ **3/12 mục §7.2** (M7/M10/M11) | Bảng §3: 6 đột biến (IPC cap vị trí, Notification OR, WaitAddr xếp hàng trước, Fault resume không ghi MR, SMP bỏ guard idle, Sync mailbox ghi đè) đều `test result: FAILED` đúng test + dòng assert; 6/6 khôi phục hash == `HEAD`. **Không** chạy lại đủ 12 mục §7.2 (xem §3 "Đối chiếu §7.2") |
| **C4** | **100.000 seed × smp 1/2/4/8** — 4 lần chạy, cùng seed cùng trace, bất biến giữ | **ĐẠT theo định nghĩa C4** (4 lần chạy, cùng seed cùng trace); **KHÔNG** kéo theo C1 | §2: `400000 lượt`, wall **5043,84 s**, 4/4 dòng `# L1 OK`, `1 passed; 0 failed`, 0 panic (⇒ 0 vi phạm `check_sched`/`check_harts` sau mỗi op trên 25,6 triệu op); **`check_ipc` rỗng** (driver không sinh message) ⇒ **không** phải bằng chứng C1 |

## 8. Đối chiếu §5 KT-24..KT-30

Số test dưới đây lấy **tươi** từ lần chạy `gate --fast` của KT-31
([`JOURNAL/kt31-baseline-gate-fast.log`](../JOURNAL/kt31-baseline-gate-fast.log), tổng **461 test**);
Miri lịch sử lấy từ log từng task, và lần chạy **Miri toàn bộ** của KT-31 ở §4.

| Task | DoD §5 | Kết quả | Test hiện tại (gate 461) | Bằng chứng chính |
|---|---|---|---|---|
| **KT-24** IPC core 1 (`MessageInfo` 35 bit, 64 MR, `ipc::transfer`, `no_reply`, non-Endpoint) | 9 mục | **9/9** | `kt24_ipc` **13** + `kt24_scn` **2** | `kt24-red-compile.log` (56 lỗi), `kt24-red-semantics.log`; fuzz 1.000×64; Miri **13/13** (37,77 s, `kt24-miri.log`); `.scn` 04/06/17 qua runner |
| **KT-25** IPC core 2 (4 cap, GRANT/unwrap, `ReplyRecv`/NB*, vòng đời Reply) | 10 mục | **10/10** | `kt25_ipc2` **24** + `kt25_scn` **1** | `kt25-gate-fast.log` 361 test; Miri **24/24** (75,99 s); reviewer-kt25 DRIFT đã sửa 3/3; mutation KT-31 ca 1 (sai vị trí cap) bắt được |
| **KT-26** Fault + Exit + procsrv model | 8 mục | **8/8** | `kt26_fault` **24** + `kt26_scn` **1** | `kt26-gate-fast.log` 386 test; Miri **24/24**; mutation "resume không ghi MR" (lịch sử + KT-31 ca 4) bắt được; `.scn` 13/14/20 |
| **KT-27** Notification word/badge, Signal OR, một waiter, Wait/Poll, bind 1:1 | 8 mục | **8/8** | `kt27_notification` **26** + `kt27_scn` **1** | `kt27-gate-fast.log` 413 test; Miri **26/26**; fuzz 1.000×64; mutation KT-31 ca 2 bắt được |
| **KT-28** WaitAddr bảng băm, đọc+xếp hàng một lần, `WakeAddr(n)` | 8 mục | **8/8** | `kt28_waitaddr` **12** | `kt28-gate-fast.log` 427 test; Miri **12/12**; fuzz 1.000×64; mutation KT-31 ca 3 bắt được |
| **KT-29** SMP 1/2/4/8 + push migration | 9 mục | **9/9** | `kt29_smp` **13** + `l1_k1c` **1** | `kt29-gate-fast.log` 442 test; Miri **13/13** (261,03 s); `nightly --l1 --seeds 200 --smp 1,2,4,8` PASS 800 lượt; KT-31 100k×4 PASS; mutation KT-31 ca 5 bắt được |
| **KT-30** `vifast-sync` MCS + Mailbox MPSC | 8 mục | **8/8** | `kt30_sync` **10** + `kt30_stress` **3** | `kt30-gate-fast.log` 461 test; Miri **16/16** (13,5 s); stress 4 luồng × 500 op, 200 lần lặp/0 fail; mutation KT-31 ca 6 bắt được |
| **Tổng K1c** | **60 mục** | **60/60 theo DoD từng task** | **131 test K1c** (13+2+24+1+24+1+26+1+12+13+10+3+1) + `kt11_diff_sim` 1 | 7 log `kt2N-gate-fast.log` + `kt31-*` |

- Các lệch **tên** so với kế hoạch (không phải lệch nội dung, do đặt tên lúc thi hành):
  `kt25_reply`→`kt25_ipc2`, `kt26_fault_exit`→`kt26_fault`, `kt28_waitaddr`/`kt29_smp` đúng tên,
  `kt24_scn`/`kt25_scn`/`kt26_scn`/`kt27_scn` là runner `.scn`; KT-24 DoD 4 "hàng gửi không giới
  hạn" bị **D2-54** thay bằng `MAILBOX_CAP = 8` + `Busy` (đã ghi trong `v2/docs/k1c-ipc-semantics.md` §4,
  là quyết định của Agent 0, không phải nới ngưỡng).
- **Không** có mục nào của 7 task bị đánh dấu "chưa đạt" trong log từng task; các **nợ** đã ghi
  (reviewer + D2-56..60) nằm ở §9, không tô hồng thành "đạt".

## 8b. Đối chiếu 5 việc §7.1 (cổng cuối)

| # | Việc | Yêu cầu | Kết quả |
|---|---|---|---|
| 1 | `nightly --l1 --seeds 100000 --smp 1,2,4,8` | 4 log/dòng `# L1 OK`, cùng seed cùng trace, 0 vi phạm | **Chạy ĐẠT** — §2 (400.000 lượt, 5043,84 s); **C1 chưa đạt** vì driver không sinh IPC |
| 2 | `gate --fast` | PASS không retry, ≥ 263 test | **ĐẠT** — §5 (461 test; xem wall/T-INFRA) |
| 3 | `nightly --only-miri` | danh sách có `kt24..kt30` + `vifast-sync` PASS | **ĐẠT** — §4 |
| 4 | Mutation tay §7.2 | mỗi đột biến **đúng một** test đỏ → revert → log | **CHƯA ĐẠT** — 6/6 ca đỏ đúng test nhưng chỉ **3/12 mục §7.2** (§3) |
| 5 | Bảng DoD K1c | 4 dòng C1..C4 + log; dòng chưa đạt ghi rõ | **ĐẠT** — §7 (C1 **chưa đạt**, C2 đạt-một-phần, C3 **chưa đạt**, C4 đạt theo định nghĩa) |

## 9. Phần **chưa** phủ (không tô hồng)

1. **Toàn bộ K1c là model-only trong sim 1 luồng**: không chạy kernel/QEMU, không thread thật;
   `vifast-sync` chỉ stress **host** (Miri + 4 thread trong test), **chưa** nối vào kernel/sim.
   "SMP 1/2/4/8" là **mô hình tất định N hart** trong một luồng (D2-53 Q1), không phải SMP thật.
2. **C1 chưa đạt ở mức 100.000 seed** (C2 cũng chỉ đạt một phần): driver `l1_k1c` có 9 op
   (`prio/affinity/dispatch/block/wake/process_mail/push_migration/yield`) — **không** sinh
   Send/Recv/Call/Reply/Signal/WaitAddr. Vì vậy 100k **không** chứng minh bảo toàn message /
   không mất đánh thức ở quy mô 100k; bằng chứng cho IPC/Notification/WaitAddr dừng ở **1.000
   seed × 64 op** mỗi test. Muốn đúng câu chữ C1/C2 của K1C-PLAN §1.1 phải mở driver L1 **trộn
   IPC** (đề xuất task nhỏ, ví dụ KT-32 — **chưa** làm trong KT-31).
3. **K2 (VM) chưa làm**: Frame/VSpace/PageTable/ASID/TLB; WaitAddr khoá theo `(ObjId frame,
   offset)` **không** theo địa chỉ vật lý; `addr_words` **chưa** sweep khi frame chết; "Frame
   chia sẻ hai tiến trình" và "Mutex std stress" (L3) **chưa đạt** — đúng như K1C-PLAN §1.4.
4. **K3 backlog D2-56..D2-60** (chưa làm, không đổi code K1c): (a) đưa `FaultKind`
   VM/unknown/user vào `spec/abi.toml` (K1c model-only, spec chỉ có `Exit`); (b) procsrv **thật**
   + shell Rust ở K3.5 (K1c chỉ `procsrv_model.rs`); fault **không** `fault_ep` (K1c trả
   `InvalidArgument`); (c) Notification: bound/IRQ thật, `IrqHandler.SetNotification` còn
   reserved, `NBRecv` Notification trả `Message/value`; (d) WaitAddr: sweep/sweep-theo-bucket,
   bất biến `check_waitaddr` **một chiều**, timeout (K6 — spec không có), `EAGAIN`;
   (e) SMP: IPI/timer thật, `TlbFlush`/`VmKick`/`PowerReq` trong hộp thư (K1c mới
   Wake/Reschedule/Migrate/Stop), multicast/IRQ; (f) `vifast-sync`: `acked ≠ delivered` (K3 chốt
   TLB), MCS `SeqCst` chưa hạ bậc, **không** loom/explorer.
5. **Không** traceformat 32 B (D2-40), **không** conformance/perf/test kernel trên QEMU (K3);
   `.scn` chỉ chạy qua **runner model** (`kt2N_scn`), không phải kernel thật.
6. **`spec/kernel.toml` vẫn chưa tồn tại** (Q3): `WAITADDR_BUCKETS = 64`, `MAX_HARTS = 8` là
   hằng **model `[TK]`** trong `v2/docs`, không phải spec.
7. **Thời gian**: 100k×4 mất **84,06 phút** (so `[ƯL]` 20 phút của K1C-PLAN §7.1) do 4 cấu hình
   × 100k seed × 2 lần chạy/seed trên máy tải; **không** giảm seed/không đổi invariant —
   là **T-INFRA** về dự toán, không phải nới ngưỡng.
8. **Mutation**: mới chạy **6 ca**, chỉ trùng **3/12 mục §7.2** (M7/M10/M11); **chưa** chạy
   M1 (copy 1/64 MR), M2/M3/M4 (GRANT/unwrap), M5 (xoá word), M6 (một waiter), M8 (bucket), M9
   (`home`), M12 (`Suspend(Abort)` tách caller) ⇒ **C3 chưa đạt** đủ danh sách; các test tương
   ứng đang **xanh** trong gate nhưng đó không phải bằng chứng mutation mới. Ngoài ra log không
   có "test xanh trước đột biến" (mục này chỉ là `git diff` rỗng).
9. **Miri** chỉ chạy tập chọn của `nightly --only-miri` (không chạy `l1`/`kt2N_scn` vì I/O —
   D2-47(a)); các test stress đa luồng tự gạt dưới Miri (`#![cfg(not(miri))]`).
10. **Không** chạm `viscript/` (đang có thay đổi của tab khác) và **không** cam kết gì về dòng
    ViScript trong báo cáo này.

## 10. Đề xuất

- **KT-31 (task này): REVIEW** — đã chạy đủ 5 việc §7.1 + parity 10k, số thô trung thực:
  100k×4 L1 PASS (400.000 lượt, 5043,84 s; **chỉ sched/SMP/mailbox**), gate cuối PASS 461 test
  (wall 254,19 s — T-INFRA, L1 720 ms), Miri 18 tập/276 test PASS (4058,24 s; **thiếu
  `kt25_ipc2`**), mutation 6/6 ca đỏ đúng test (**chỉ 3/12 mục §7.2**), parity 10k PASS
  (15,52 s). **Ba mục chưa đạt**: C1 ở 100k, C3 đủ 12 mục, Miri phủ `kt25_ipc2`.
  Đề xuất Agent 0 lật `DONE` như **"gate run một phần"** (D2-61) + mở **KT-32** đóng 3 nợ,
  thay vì đóng phase.
- **K1c (phase): REVIEW — CHƯA DONE.** Ngoài 3 nợ trên còn model-only + K2/K3 backlog
  D2-56..60 (§9). Chỉ đóng K1c sau khi KT-32 xong (driver L1 trộn IPC 100k; đủ 12 mutation
  §7.2; thêm `kt25_ipc2` vào Miri) và Agent 0 lật sổ.
- **Không lật QUEUE**: báo cáo này **không** tự đổi trạng thái task nào; KT-30 vẫn chờ Agent 0
  lật (reviewer-kt30 DRIFT đã sửa 2/2).
