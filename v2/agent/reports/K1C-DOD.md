<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# K1C-DOD — bằng chứng cổng cuối phase K1c (KT-31)

Người ghi: **Agent 2 dòng OS** · Ngày: 2026-11-13; **cập nhật 2026-11-14 (KT-32 §2b/§2c, KT-33
§3b/§4b, KT-34 §2d/§4c/§5c — đóng C2)** · Task: **KT-31** (cổng cuối K1c) + KT-33 (C3/Miri) +
**KT-34 (C2: cap Reply active)** · HEAD khi mở task: `0d026bd`; HEAD khi KT-33 mở: `16eb801`;
HEAD khi KT-34 mở: `43c2e3c` · Nhánh `main`.

**Phạm vi**: chỉ **chạy cổng, thu log, đối chiếu DoD**. Không sửa semantics IPC/Notification/
WaitAddr/Fault/Exit/SMP/sync, không sửa `spec/`/`vifast-model`/`.scn`/QUEUE/DECISIONS/AGENTS/
`viscript/`, không thêm dependency/`unsafe`/`#[ignore]`/`#[allow]`/`#[should_panic]`, không nới
seed/ngưỡng. Đột biến tay (C3) là **sửa tạm + khôi phục**, không commit.

Nguồn đối chiếu: `review/agent0/K1C-PLAN.md` §5 (KT-24..KT-30) + §7.1 (cổng cuối) + §7.2 (danh
sách đột biến), `v2/agent/DECISIONS.md` D2-53..D2-60, `v2/agent/STATE.md`.

## 0. TL;DR

| # | Việc (§7.1) | Kết quả | Log |
|---|---|---|---|
| 1 | `nightly --l1 --seeds 100000 --smp 1,2,4,8` | **ĐẠT** — KT-32 đã chạy lại driver **trộn IPC** (§2b): 4×100.000 seed, 400.000 lượt, **5.786.297** `ipc_sent`, 0 vi phạm, wall 6596,88 s. (KT-31 bản đầu chỉ phủ sched/SMP/mail — lịch sử) | `JOURNAL/kt32-l1-100k-mixed.log` (+ `kt31-l1-100k-smp.log`) |
| 2 | `gate --fast` (≥ 461 test, không retry) | **PASS 461 test**, L1 1.000 seed 720 ms; wall **254,19 s** ⇒ **T-INFRA** (> 60 s, do tải máy) | `JOURNAL/kt31-gate-fast.log` |
| 3 | `nightly --only-miri` (kt24..kt30 + sync + nền K1a/K1b) | **ĐẠT sau KT-33** — KT-31 chỉ 18 tập/276 test và **thiếu `kt25_ipc2`**; KT-33 đã thêm `kt25_ipc2` vào danh sách (v2/bin/v2) + chạy lại: **19 tập / 300 test**, 0 failed, wall **xem §4b** | `JOURNAL/kt33-miri-all.log` (+ `kt31-miri-all.log` cũ) |
| 4 | Mutation tay 6 bất biến K1c (C3) | **ĐẠT sau KT-33 — 12/12 mục §7.2**; KT-31 mới 6 ca (3/12 mục), KT-33 chạy đủ M1..M12: exit 101 + `test result: FAILED` đúng test + assert, 12/12 khôi phục hash == HEAD | `JOURNAL/kt33-mutation.log` (bảng §3b) |
| 5 | Bảng DoD K1c (file này) | **Đã lập** — §7/§8 dưới đây | `v2/agent/reports/K1C-DOD.md` |
| 6 | Parity 10k (hồi quy, Q12) | **PASS** — `kt11_diff_model_va_sim_10000_seed` 10.000 seed × 24 op, 15,52 s | `JOURNAL/kt31-parity-10k.log` |
| 7 | **KT-33**: siết driver (`ipc_sent>1` + `ipc_delivered>0` + panic `kiem` kèm `seed/smp`) + 10k×4 + gate | **PASS** — xem §2c/§5b: 10k×4 **40.000 lượt**, `sent>1` từng lượt, gate **461 test** | `JOURNAL/kt33-l1-10k.log`, `JOURNAL/kt33-gate-fast.log` |
| 8 | **KT-34**: từ chối cap `Reply` đang active (`Busy`) + bất biến `check_reply_links` + property C2 mọi thứ tự + 10k×4/100k×4 | **PASS** — §2d: 10k×4 **661 s**, 100k×4 **6306,38 s (105m06)**, 0 panic/0 vi phạm; test đỏ trước `JOURNAL/kt34-red-reply-reuse.log` | `JOURNAL/kt34-l1-10k.log`, `JOURNAL/kt34-l1-100k.log` |
| 9 | **KT-34** — gate cuối | **PASS 468 test / 62 suite**, L1 1.000 seed **275 ms**, wall **239,32 s** (wall host — §5c) | `JOURNAL/kt34-gate-fast.log` |
| 10 | **KT-34** — Miri 3 tập ảnh hưởng (`kt25_ipc2`/`kt27_notification`/`kt28_waitaddr`) | **PASS** 29 / 27 / 13 test, 0 failed, 111 / 66 / 116 s — §4c | `JOURNAL/kt34-miri-kt25.log`, `-kt27`, `-kt28` |

**Đề xuất trạng thái**: **KT-34 DONE** — **C2 ĐẠT** (§2d/§7: từ chối cap `Reply` active, bất biến
`BlockedOnReply ⇔ Reply.caller == Some(tcb)` sau **mỗi** op, property C2 theo **mọi** thứ tự bước,
100k×4 + Miri + gate xanh); **K1c DONE (đề xuất)** — C1/C2/C3/C4 + Miri + gate ĐẠT, nợ còn lại là
**model-only/K2/K3** (§9.1/§9.3/§9.4) **không** thuộc K1c. KT-31/32/33 DONE (D2-61/62/63).
Không lật QUEUE. Chi tiết §10.

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

### 2b. Cập nhật **sau KT-32** (2026-11-14) — nợ **C1-100k** đã đóng

KT-31 ghi C1 **chưa đạt** vì driver `l1_k1c` chỉ có 9 op sched/SMP/mail. **KT-32** mở rộng
**chỉ driver** đó (test-only, không sửa semantics `kobj/ipc/...`) thêm **10 op** K1c —
Send/NBSend/Recv/Call/Reply trên Endpoint/Reply **thật**, Signal/Wait trên Notification,
WaitAddr/WakeAddr/Store — rồi chạy lại **đúng** lệnh §7.1 (không giảm seed, không đổi invariant):

```
./v2/bin/v2 nightly --l1 --seeds 100000 --smp 1,2,4,8
```

| smp | Dòng log | `ipc_sent` | `ipc_delivered` | `ipc_dropped` |
|---|---|---|---|---|
| 1 | `# L1 OK: smp 1, 100000 seed × 64 op` | 1.447.334 | 279.209 | 949.749 |
| 2 | `# L1 OK: smp 2, 100000 seed × 64 op` | 1.447.459 | 280.812 | 937.672 |
| 4 | `# L1 OK: smp 4, 100000 seed × 64 op` | 1.445.714 | 282.294 | 931.485 |
| 8 | `# L1 OK: smp 8, 100000 seed × 64 op` | 1.445.790 | 281.271 | 929.634 |
| **tổng** | `# L1 OK: 400000 lượt (smp [1, 2, 4, 8], seeds 100000)` | **5.786.297** | **1.123.586** | **3.748.540** |

- **Wall 6596,88 s** (109,95 phút; `1 passed; 0 failed`; **0** panic; không retry), log
  [`JOURNAL/kt32-l1-100k-mixed.log`](../JOURNAL/kt32-l1-100k-mixed.log), báo cáo
  [`reports/KT32-L1-MIXED.md`](KT32-L1-MIXED.md).
- `ipc_sent` = **14,47 message/seed** (prologue `NBSend` 1/seed; ~13,5 do 64 op ngẫu nhiên sinh)
  ⇒ 100k **thật sự** đi qua đường IPC/Notification/WaitAddr; `check_ipc` **không còn rỗng**.
- Bất biến `ipc_sent == ipc_delivered + ipc_pending + ipc_dropped` đúng sau **mỗi** op trên
  **51,2 triệu** lần kiểm (400.000 lượt × 64 op × 2 lần chạy/seed); cùng seed ⇒ cùng trace (hai
  lần/seed, `assert_eq` cả trace lẫn bộ đếm).
- **`ipc_dropped` cao là CÓ ĐẾM, không phải "không mất message"**: chủ yếu là message bị **huỷ hợp
  lệ** theo D2-25 khi TCB gửi đang chặn bị `wake`/huỷ (`unlink_tcb_queues`, bất biến
  `pushed == popped + removed + len`) và Reply **không có `caller`** (`reply_with` đếm
  `ipc_dropped`) — bảo toàn vẫn đúng, không phải message "bốc hơi".
- **Không** claim **C2**: driver dùng lại **cùng cap `Reply`** cho nhiều `Recv`; model cho phép
  `reply_set_caller` ghi đè `caller` cũ ⇒ caller trước có thể **mất đường Reply** — lỗ hổng **có
  sẵn của model**, không phải C1 và **không** được tính là C2. C2 vẫn chỉ đạt ở mức mô hình 1
  luồng như §7.
- Nợ chuyển **KT-33** (lịch sử KT-32): `ipc_sent > 0` mỗi lượt **gần tautology** nhờ prologue
  (nên siết `ipc_sent > 1`/`ipc_delivered > 0`), panic của `kiem` chưa kèm `seed`/`smp`; **C3**
  (đủ 12 mutation §7.2) + Miri `kt25_ipc2` **vẫn mở** — K1c **chưa** DONE. **ĐÃ ĐÓNG ở KT-33**:
  driver siết + 10k×4 (§2c), 12/12 mutation (§3b), Miri có `kt25_ipc2` (§4b).


### 2c. **KT-33** — siết driver (đóng 2 nợ nhỏ của KT-32) + 10k×4

Nợ KT-32 §9: (i) `ipc_sent > 0` gần tautology nhờ prologue; (ii) panic của `kiem` không có
`seed`/`smp`. KT-33 sửa **chỉ** driver [`sim/vifast-sim/tests/l1_k1c.rs`](../../sim/vifast-sim/tests/l1_k1c.rs)
(test-only, +32/−18 dòng), **không** đổi semantics/seed/op:

| Sửa | Trước | Sau |
|---|---|---|
| Ngưỡng cuối lượt | `ipc_sent > 0` | **`ipc_sent > 1`** — prologue chỉ lo 1 message, 64 op còn lại phải sinh thêm (không còn tautology) |
| Giao message | (không kiểm) | **`ipc_delivered > 0`** ở mức **tổng mỗi smp** (một seed đơn lẻ có thể không giao được do hàng đầy/huỷ hợp lệ — không bắt oan) |
| Panic `kiem` | `check_X sau {dong}` | `check_X sau {dong} (seed {seed}, smp {harts})` — log đỏ tự đủ ngữ cảnh |

Lệnh + kết quả (log [`JOURNAL/kt33-l1-10k.log`](../JOURNAL/kt33-l1-10k.log)):

```
/usr/bin/time -p ./v2/bin/v2 nightly --l1 --seeds 10000 --smp 1,2,4,8
```

| smp | `ipc_sent` | `ipc_delivered` | `ipc_dropped` |
|---|---|---|---|
| 1 | 144.977 | 27.850 | 95.461 |
| 2 | 144.873 | 28.154 | 93.917 |
| 4 | 144.552 | 28.174 | 92.928 |
| 8 | 145.058 | 28.405 | 93.239 |
| **tổng** | **579.460** | **112.583** | **375.545** |

- **PASS**: `# L1 OK: 40000 lượt (smp [1, 2, 4, 8], seeds 10000)`, `1 passed; 0 failed`,
  wall **658,60 s** (10,98 phút; 40.000 lượt × 64 op = **2,56 triệu op**, 0 panic).
- Bộ đếm **trùng khít** bản 10k của KT-32 (144.977/27.850/95.461; …) ⇒ siết assert **không**
  đổi hành vi/trace (tất định giữ nguyên), chỉ làm bất biến mạnh hơn.
- `sent > 1` đúng **mọi** lượt (nếu seed nào chỉ có prologue, test panic ngay) và
  `delivered > 0` đúng **mọi** smp ⇒ **không** còn chỗ dựa vào tautology.
- Trong `gate --fast` cuối, driver cũng chạy ở L0 mặc định **1.000 seed / smp 1**:
  `driver_smp_cung_seed_cung_trace ... ok`, `1 passed`, **13,37 s** (§5b) — bản siết xanh ngay
  ở ngưỡng cổng.

### 2d. **KT-34** (2026-11-14) — C2: cap `Reply` **đang active** + bất biến liên kết Reply

**Lỗi mô hình có sẵn (không phải test-only — D2-63).** `Reply` mang **một** `caller`. Khi `Call`
gặp người nhận đang `Recv` có Reply cap, kernel bind `Reply.caller = Some(caller)`. Nếu **cùng**
`Reply` object bị dùng lại cho `Recv` **thứ hai** (kể cả qua cap khác trỏ cùng object), `Call` kế
tiếp ghi đè `caller` → TCB cũ vẫn `BlockedOnReply` nhưng Reply **không trỏ về nó** ⇒ **mất đường
Reply/đánh thức** (deadlock im lặng). Đây là vi phạm **C2** ở tầng IPC.

**Sửa** (chỉ `v2/sim/vifast-sim/`; ngữ nghĩa **model-only `[TK]`**, ABI/`spec/` **không đổi**;
`Busy` là mã mô hình — seL4 thật không có mã này cho ca dùng lại reply):

| # | Vị trí | Quy tắc | Thứ tự kiểm |
|---|---|---|---|
| 1 | `recv(ep, reply=Some(r))` | `r` đang `caller=Some` ⇒ **`Busy`** | **trước** `reset_ipc_error`, trước pop message, trước mọi đổi trạng thái |
| 2 | `call_msg(ep)` (lớp 2) | Reply cap của waiter đã bị bind **qua cap khác** cùng object ⇒ **`Busy`** | **trước** `admit_message` (không cấp `msg_id`/`last_mr`), waiter vẫn chờ |
| 3 | `reply_recv` / `nb_send_recv` | Reply **mới** đang active ⇒ **`Busy`** | **trước** khi gửi reply (`reply_with`) — không reply nửa chừng; ngoại lệ hợp lệ `recv_reply == reply` (reply tiêu thụ `caller` trước) |
| 4 | `check_reply_links()` | `Reply.caller=Some(t)` ⇒ `t` sống + `BlockedOnReply{đúng reply}`; `BlockedOnReply{reply}` ⇒ `reply` sống + `caller == Some(tcb)` | sau **mỗi** public op (trong `sched_after_op`, cùng chỗ `check_ipc`; **một** lượt quét object) |
| 5 | `transition_tcb` (cổng thấp) | vào `BlockedOnReply` ⇒ bind `caller`; rời ⇒ gỡ `caller`; Reply đã bind người khác ⇒ **`IllegalOperation`** (không ghi đè) | trước khi ghi trạng thái; object không phải `Reply` sống thì bỏ qua (ma trận 11×11 KT-23 dựng id tượng trưng) |

`reply_with` vẫn **take `caller` trước** khi deliver message rồi mới `Ready` người gọi (thứ tự cũ,
không đổi) — nên lúc `recv` chạy, `caller` đã thành `None` (cơ sở của ngoại lệ #3).

**Test (đỏ trước → xanh)**: [`JOURNAL/kt34-red-reply-reuse.log`](../JOURNAL/kt34-red-reply-reuse.log)
(`left: Ok(())`, `right: Err(Busy)` trên code cũ — `Recv` lần hai **thành công** và ghi đè caller).

| Tệp | Test thêm | Nội dung |
|---|---|---|
| `tests/kt25_ipc2.rs` | `recv_cap_reply_dang_active_tra_busy_roi_dung_lai_duoc` | `Call` rồi `Recv` lần hai **cùng cap active** ⇒ `Busy`; caller cũ **vẫn** `BlockedOnReply` + **vẫn** là `caller`; `Reply` đúng ⇒ caller `Ready`, `caller=None`; **sau đó cap dùng lại được** (cả hai chiều) |
| | `call_vao_waiter_co_reply_da_active_qua_cap_khac_tra_busy` | Hai cap cùng `Reply` object: waiter nhận cap lúc `caller=None`, Reply bị bind **qua cap kia** ⇒ `Call` tới waiter đó `Busy` (không cấp `msg_id`), `Reply` xong ⇒ `Call` lại **thành công** |
| | `cong_transition_giu_lien_ket_reply_va_khong_ghi_de_caller` | Cổng `transition_tcb`: bind khi vào, gỡ khi rời, **từ chối** ghi đè caller ⇒ `check_reply_links` luôn `Ok` |
| | `call_truoc_recv_lan_dau_van_hop_le_chi_chan_lan_hai` | `Call` **trước** `Recv` đầu (hàng gửi, Reply chưa bind) ⇒ `Recv` **hợp lệ** (đường `.scn` 04/05); chỉ lần **thứ hai** mới `Busy` |
| | `reply_recv_kiem_reply_moi_truoc_khi_gui_va_dung_lai_chinh_reply_hop_le` | `ReplyRecv` Reply mới active ⇒ `Busy` **trước** khi gửi reply; dùng **chính** Reply vừa trả lời ⇒ hợp lệ |
| `tests/kt27_notification.rs` | `property_signal_sau_khi_xep_hang_danh_thuc_moi_thu_tu` | **6/6 hoán vị** {Wait t0, Wait t1, Signal}: Signal **sau** ⇒ waiter đã xếp hàng **Ready** + nhận `(word,badge)`; Signal **trước** ⇒ word còn, Wait thấy ngay |
| `tests/kt28_waitaddr.rs` | `property_wakeaddr_sau_khi_xep_hang_danh_thuc_moi_thu_tu` | **6/6 hoán vị** {Wait(expected=0), Store(1), Wake(1)}: Wake **sau** ⇒ `Ready` + trả 1; Wake **trước** ⇒ 0, không nuốt giá trị (Wait sau thấy giá trị mới ⇒ `InvalidArgument`) |
| `tests/kt22_mailbox.rs` | (sửa rig) | 1 → **3 Reply riêng** (`S_REP_EXTRA`): 3 `Recv` liên tiếp cần 3 `Reply` **chưa bind** — đúng one-shot; kỳ vọng FIFO/`pushed`/`popped` **giữ nguyên**, không nới test |
| `tests/l1_k1c.rs` | (driver) | `Recv` chỉ cấp Reply **rảnh** (`reply_caller(rep).is_none()`), còn lại `None`; thêm `check_reply_links` sau **mỗi** op (kể cả prologue) |

**10k×4** (`./v2/bin/v2 nightly --l1 --seeds 10000 --smp 1,2,4,8`, **không retry** — log
[`JOURNAL/kt34-l1-10k.log`](../JOURNAL/kt34-l1-10k.log)):

| smp | `ipc_sent` | `ipc_delivered` | `ipc_dropped` |
|---|---|---|---|
| 1 | 144.977 | 27.845 | 95.458 |
| 2 | 144.871 | 28.155 | 93.904 |
| 4 | 144.552 | 28.177 | 92.919 |
| 8 | 145.056 | 28.404 | 93.230 |
| **tổng** | **579.456** | **112.581** | **375.511** |

**PASS**: `# L1 OK: 40000 lượt (smp [1, 2, 4, 8], seeds 10000)`, `1 passed; 0 failed`, wall
**661 s** (11,02 phút); `ipc_sent > 1` **mọi** lượt, `ipc_delivered > 0` **mọi** smp,
0 panic/0 vi phạm `check_*` (gồm `check_reply_links`).

**100k×4** (`./v2/bin/v2 nightly --l1 --seeds 100000 --smp 1,2,4,8` — **không** nới seed, log
[`JOURNAL/kt34-l1-100k.log`](../JOURNAL/kt34-l1-100k.log)):

| smp | `ipc_sent` | `ipc_delivered` | `ipc_dropped` |
|---|---|---|---|
| 1 | 1.447.334 | 279.201 | 949.684 |
| 2 | 1.447.456 | 280.816 | 937.566 |
| 4 | 1.445.714 | 282.294 | 931.408 |
| 8 | 1.445.792 | 281.287 | 929.515 |
| **tổng** | **5.786.296** | **1.123.598** | **3.748.173** |

**PASS**: `# L1 OK: 400000 lượt (smp [1, 2, 4, 8], seeds 100000)`, `1 passed; 0 failed`, wall
**6306,38 s** (`real 105m06,38`; user 6010,62 s) — **không** nới seed/không đổi op (64). Cùng seed ⇒
cùng trace (test so `a == b` từng lượt, cả trace lẫn bộ đếm IPC); `ipc_sent > 1` **mọi** lượt;
`ipc_delivered > 0` **mọi** smp; **0 panic/0 vi phạm** `check_sched`/`check_ipc`/`check_waitaddr`/
`check_mailboxes`/`check_caps`/**`check_reply_links`** (mỗi op ⨯ 400.000 lượt × 64 op = **25,6
triệu** lần kiểm, chưa kể lần kiểm thứ hai của prologue). `ipc_delivered` 1.123.598 / `ipc_sent`
5.786.296 = **19,4 %** message được giao (phần còn lại là huỷ hợp lệ D2-25 + còn trong hàng) —
cùng bậc với 100k của KT-32 (5.786.297 / 1.123.586) ⇒ **hành vi IPC không đổi**, chỉ thêm bất biến
C2. Wall **6306,38 s** ≈ dự toán 20 phút của K1C-PLAN §7.1 **không đúng** (dự toán cũ **T-INFRA**,
không phải nới ngưỡng) — vẫn **dài hơn** 100k bản KT-32 (6596,88 s) **giảm nhẹ 290 s** dù thêm
`check_reply_links`, do máy rảnh hơn.

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

## 3b. Cập nhật **KT-33** (2026-11-14) — đủ **12/12** mutation §7.2

Mỗi ca: (0) chạy test đích **xanh trước** (log `ok. 1 passed`; các test khác `filtered out`);
(1) sửa **đúng một chỗ** trong nguồn (không commit); (2) chạy **đúng một** test đích;
(3) khôi phục `git checkout -- <file>` rồi xác nhận `git hash-object` == `HEAD:<file>` **và**
`git status --porcelain -- v2/` không còn tệp track bị sửa (trước ca sau). Log đầy đủ (kèm diff
từng ca + hash trước/sau): [`JOURNAL/kt33-mutation.log`](../JOURNAL/kt33-mutation.log).

| # | Tệp · đột biến (áp tạm, một chỗ) | Test đỏ (§7.2) | Bằng chứng đỏ (exit 101 + assert) | Khôi phục |
|---|---|---|---|---|
| **M1** | `ipc.rs` `transfer`: `if i == 32 { continue; }` — **bỏ copy 1** trong 64 MR | `kt24_ipc::message_64_mr_hai_vung_thanh_ghi_va_ipc_buffer` | `kt24_ipc.rs:339` `assertion left == right failed: MR 32 ở IPC buffer` — left **0**, right **4026531872**; `test result: FAILED. 0 passed; 1 failed` | `ipc.rs` == `2173300…` |
| **M2** | `ipc.rs` `plan_cap_transfer`: bỏ `c.rights & RIGHT_GRANT == 0` (chỉ còn `occupied[i]`) | `kt25_ipc2::thieu_grant_that_bai_mem_message_van_giao` | `kt25_ipc2.rs:367` `caps = số cap cài được` — left **4**, right **3** | `ipc.rs` == `2173300…` |
| **M3** | `kobj.rs` `deliver_caps`: `rights = snap.rights` (không bỏ `RIGHT_GRANT` trên cap nhận) | `kt25_ipc2::bon_cap_cai_du_bo_grant_giu_badge` | `kt25_ipc2.rs:308` `cap nhận được **bỏ** RIGHT_GRANT (thiết kế :319)` — left **4**, right **0** | `kobj.rs` == `37ff523…` |
| **M4** | `ipc.rs` `plan_cap_transfer`: bỏ nhánh **unwrap badge** (cap badge đi tiếp vào receive window) | `kt25_ipc2::unwrap_badge_khong_chiem_receive_window` | `kt25_ipc2.rs:481` `chỉ cap thường được cài` — left **2**, right **1** | `ipc.rs` == `2173300…` |
| **M5** | `kobj.rs` `notification_signal`: bỏ khối **xoá** `word`/`badge` sau khi giao | `kt27_notification::mot_waiter_duy_nhat_ba_tcb` (assert "giao cho waiter xong word = 0" — xem ghi chú (a)) | `kt27_notification.rs:332` `giao cho waiter xong word = 0` — left **165**, right **0** | `kobj.rs` == `37ff523…` |
| **M6** | `kobj.rs` `notification_wait`: bỏ cổng `busy` ⇒ **cho phép nhiều waiter kiểu v1** (xem ghi chú (b)) | `kt27_notification::mot_waiter_duy_nhat_ba_tcb` | `kt27_notification.rs:314` `waiter thứ hai ⇒ Busy` — left **Ok(())**, right **Err(Busy)** | `kobj.rs` == `37ff523…` |
| **M7** | `kobj.rs` `wait_addr`: **xếp hàng trước** (`transition_tcb`+push) rồi mới so `current != expected` | `kt28_waitaddr::expected_lech_tra_ngay_khong_xep_hang` | `kt28_waitaddr.rs:246` `TCB phải **giữ** Ready, không bị chặn` — left **BlockedOnAddress{frame:3, offset:16}**, right **Ready** | `kobj.rs` == `37ff523…` |
| **M8** | `kobj.rs` `wake_addr`: bỏ kiểm `(frame, offset)` — chọn mọi waiter trong bucket | `kt28_waitaddr::va_cham_bucket_khong_anh_huong_tim_kiem` | `kt28_waitaddr.rs:208` `assertion left == right failed` (số waiter được wake) — left **2**, right **1** | `kobj.rs` == `37ff523…` |
| **M9** | `kobj.rs` `apply_migrate`: không đặt `t.home = to` (giữ home cũ) | `kt29_smp::push_dung_mot_tcb_cuoi_hang_qua_migrate` | `kt29_smp.rs:281` `assertion left == right failed` (`home` sau migrate) — left **0**, right **1** | `kobj.rs` == `37ff523…` |
| **M10** | `kobj.rs` `tcb_resume_with_regs`: bỏ `resolve_caps`+`admit_message`+`deliver_message` (**không ghi MR trước Ready**, đường r319) | `kt26_fault::resume_with_regs_khi_dang_cho_reply_ghi_thanh_ghi_truoc_ready` | `kt26_fault.rs:341` `assertion left == right failed` — left **[0, 0, 0, 0]**, right **[7, 8, 9, 10]** | `kobj.rs` == `37ff523…` |
| **M11** | `mailbox.rs` `send`: nhánh đầy **ghi đè** message cũ rồi `Ok(())` thay vì `Err(Busy)` | `kt30_sync::mailbox_day_tra_busy_khong_mat_message_dau` | `kt30_sync.rs:130` `assertion left == right failed` — left **Ok(())**, right **Err(Busy)** | `mailbox.rs` == `5747220…` |
| **M12** | `kobj.rs` `unlink_tcb_queues`: bỏ nhánh `ObjType::Reply` (không tách `Reply.caller` khi `Suspend(Abort)` trên `BlockedOnReply`) | `kt18_tcb::suspend_abort_blocked_on_reply_va_reply_bi_bo` | `kt18_tcb.rs:472` `Reply.caller bị tách` — left **Ok(Some(7))**, right **Ok(None)** | `kobj.rs` == `37ff523…` |

- **Tổng**: **12/12** ca `exit 101`, `test result: FAILED. 0 passed; 1 failed`, đúng tên test
  §7.2, có dòng assert (`left/right`); **12/12** khôi phục hash == `HEAD:<file>` (M12 cũng là ca
  cuối, `v2/` sạch sau harness). Không ca nào đỏ do lỗi biên dịch: harness **chặn** log có
  `error[E…]`/`could not compile`, và mọi ca đều có bước "xanh trước đột biến".
- Ghi chú **(a) M5**: §7.2 ghi test "word xoá sau khi giao". Trong `kt27_notification`, đường
  **`Signal` xoá word sau khi giao** được assert ở `mot_waiter_duy_nhat_ba_tcb` (dòng 332,
  "giao cho waiter xong word = 0"); test tên `word_chi_xoa_khi_giao_cho_waiter` lại đi qua đường
  **`Wait`** (`notification_wait`, khối xoá nằm ở chỗ khác) nên nếu chỉ đột biến phía `Signal`
  thì test đó **vẫn xanh** — vì vậy ca chọn đúng chỗ + đúng assert của hành vi §7.2.
- Ghi chú **(b) M6**: mô hình v2 chỉ có **một** `waiter: Option<ObjId>` (QD §3.11, khác v1), nên
  "`Signal` đánh thức **tất cả** waiter" **không** biểu diễn được bằng một chỗ sửa (muốn có nhiều
  waiter phải đổi kiểu dữ liệu + `check_notifications` + nhiều đường). Ca này sửa **đúng chỗ duy
  nhất** mã hoá luật "một waiter" (`notification_wait`: cổng `busy`) — tức điều kiện mà v1
  không có — và test `mot_waiter_duy_nhat_ba_tcb` đỏ đúng assert "waiter thứ hai ⇒ Busy".
  Đây là **deviation câu chữ** so với §7.2 (không phải nới ngưỡng), ghi rõ để reviewer soi.
- **M11 dùng `unsafe` chỉ trong mutant tạm** (ghi đè slot của `Mailbox` để mô phỏng "đầy ⇒ ghi
  đè"); nó bị `git checkout -- v2/lib/vifast-sync/src/mailbox.rs` khôi phục ngay sau ca, **cây
  cuối không có `unsafe` mới** (`git diff HEAD -- …/mailbox.rs` rỗng; D2-17/D2-21 không đổi).
  `unsafe` trong log là **nội dung mutant đã bị huỷ**, không phải code sản phẩm.
- M5 và M6 cùng chạy một test nhưng **hai assert khác nhau** (dòng 332 vs 314), mỗi ca chạy
  **một lần**, không retry.

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
- **Trạng thái mục §7.1 #3 ở lần chạy KT-31: CHƯA ĐẠT (một phần)** — **ĐÃ ĐÓNG ở §4b (KT-33)**: danh sách `nightly --only-miri` trong
  `v2/bin/v2` **thiếu `kt25_ipc2`** ⇒ lần chạy này **không** phủ "kt24..kt30". Log Miri rời
  `JOURNAL/kt25-miri.log` (**24/24**, 75,99 s) là **từ KT-25**, **không** phải lần chạy HEAD hiện
  tại — **không** dùng để claim đã phủ. `kt30_stress` tự gạt dưới Miri (`#![cfg(not(miri))]`).
  Muốn đóng nợ này phải thêm `kt25_ipc2` vào danh sách Miri rồi chạy lại (đề xuất KT-32).
- Không chạy `l1*` dưới Miri (D2-47(a)); không chạy `.scn` (đọc tệp).


### 4b. Cập nhật **KT-33** (2026-11-14) — Miri **ĐẠT**, đã có `kt25_ipc2`

Thay đổi: `v2/bin/v2` thêm đúng **một dòng** vào danh sách `nightly --only-miri`
(`run_miri_test "vifast-sim kt25_ipc2" -p vifast-sim --test kt25_ipc2`) — đóng nợ D2-61
("Miri all thiếu `kt25_ipc2`"). Chạy lại **một mình** (không gate song song, D2-07 không retry),
log [`JOURNAL/kt33-miri-all.log`](../JOURNAL/kt33-miri-all.log):

```
/usr/bin/time -p ./v2/bin/v2 nightly --only-miri
```

**Kết quả**: `v2 nightly --only-miri: PASS`, **19 tập / 300 test / 0 failed**, wall
**4141,74 s** = **69,03 phút** (`real`; user 3912,95 s). 19 tập:

| Tập | test | thời gian |
|---|---|---|
| `vifast-objmem` (lib + alias + fuzz 200 + contract) | **40** | 202,72 s |
| `vifast-sim --lib` | **34** | 42,30 s |
| `kt16_objmem` / `kt09_untyped` / `kt09_revoke` | 4 / 8 / 7 | 2,09 / 8,04 / 4,22 s |
| `kt10_cnode` / `kt17_align` | 13 / 3 | 7,37 / 1,63 s |
| `kt18_tcb` | **18** | 24,85 s |
| `kt19_sched` / `kt20_budget` / `kt21_donation` / `kt22_mailbox` | 10 / 12 / 13 / 10 | 391,92 / 256,79 / 236,33 / 221,22 s |
| **`kt24_ipc`** | **13** | 44,49 s |
| **`kt25_ipc2`** (mới trong danh sách) | **24** | **89,28 s** |
| **`kt26_fault`** | **24** | 87,07 s |
| **`kt27_notification`** | **26** | 53,35 s |
| **`kt28_waitaddr`** | **12** | 105,19 s |
| **`kt29_smp`** | **13** | 261,03 s |
| **`vifast-sync`** (lib + `kt30_sync` + doctest) | 4 + 10 + 2 = **16** | 0,62 + 2,58 + 0,25 s |
| **Tổng** | **300** | **4141,74 s** |

- **K1c trong Miri**: 13+24+24+26+12+13+16 = **128 test**, 0 failed (**+24** so KT-31 nhờ
  `kt25_ipc2`). Mục **§7.1 #3: ĐẠT** — danh sách `--only-miri` giờ **có** `kt25_ipc2` và lần
  chạy này là **HEAD hiện tại** (không dùng log rời KT-25).
- `kt25_ipc2` dưới Miri: `running 24 tests` → `test result: ok. 24 passed; 0 failed`, 89,28 s
  (fuzz tự giảm còn 3 seed × 8 op bằng `cfg(miri)`; cảnh báo "integer-to-pointer cast" là của
  `vifast-objmem` có từ trước, **không** phải UB mới — Miri vẫn PASS).
- Wall 69,03 phút so `[ƯL]` ~20 phút của K1C-PLAN §7.1 là **T-INFRA** về dự toán (máy tải +
  Miri biên dịch từng target); **không** nới/thêm/bớt test để qua cổng.
- Vẫn **không** chạy `l1*` dưới Miri (D2-47(a)); không chạy `kt2N_scn` (đọc tệp).

### 4c. **KT-34** — Miri 3 tập **ảnh hưởng** (`kt25_ipc2` + `kt27_notification` + `kt28_waitaddr`)

Yêu cầu KT-34 chỉ là **các test ảnh hưởng** (không chạy lại toàn bộ 19 tập 69 phút của KT-33 —
không cần thiết cho diff này); chạy bằng `./v2/bin/v2 miri -p vifast-sim --test <tập>`
(**không** retry), cả ba **PASS**:

| Tập | Test | Thời gian (test) | Wall (`real`) | Log |
|---|---|---|---|---|
| `kt25_ipc2` (Reply lifecycle + 5 test KT-34) | **29** | 111,28 s | 3m51,31 s | [`JOURNAL/kt34-miri-kt25.log`](../JOURNAL/kt34-miri-kt25.log) |
| `kt27_notification` (+ property C2) | **27** | 66,02 s | 2m13,09 s | [`JOURNAL/kt34-miri-kt27.log`](../JOURNAL/kt34-miri-kt27.log) |
| `kt28_waitaddr` (+ property C2) | **13** | 115,91 s | 3m48,80 s | [`JOURNAL/kt34-miri-kt28.log`](../JOURNAL/kt34-miri-kt28.log) |

> Miri in cảnh báo "integer-to-pointer casts … might miss pointer bugs" như **mọi** lần chạy
> Miri của repo (objmem dùng arena) — không phải lỗi mới, không có UB report, 0 failed.

## 5. Gate cuối (việc §7.1 #2)

Lệnh: `./v2/bin/v2 gate --fast` — log [`JOURNAL/kt31-gate-fast.log`](../JOURNAL/kt31-gate-fast.log).

**Kết quả**: `v2 gate --fast: PASS` (gen-check ON), **461 test / 0 failed**, L1 1.000 seed
**720 ms**; `real 254.19 s` (**4,24 phút**) ⇒ **T-INFRA**: wall > 60 s (QD §5.2) do tải máy —
**không** nới ngưỡng, **không** gọi là đạt 60 s; baseline cùng ngày ở KT-30 đã 199,3 s và
baseline KT-31 chạy cùng máy cũng > 2 phút. Cổng chạy **một lần**, không retry; số test **không
giảm** so baseline (461 = 461). **Đính chính (KT-33, reviewer)**: QD §5.2 đặt ngưỡng 60 s cho
**L1 1.000 seed** (lần này **720 ms** ⇒ đạt); `real 254,19 s` là **wall host**, không phải ngưỡng
cổng — nhãn "T-INFRA" ở bản KT-31 là cách gọi tải host, không phải nới ngưỡng.


### 5b. Cập nhật **KT-33** — gate cuối sau khi siết driver

`./v2/bin/v2 gate --fast` (sau 10k×4 ở §2c, **không retry**), log
[`JOURNAL/kt33-gate-fast.log`](../JOURNAL/kt33-gate-fast.log):

**Kết quả**: `v2 gate --fast: PASS` (gen-check ON), **461 test / 0 failed** (62 suite — bằng
baseline, không thêm/bớt test), driver `l1_k1c` trong **L0** chạy bản siết: `1 passed` **13,37 s**;
L1 cũ 1.000 seed/999 trace **653 ms**. `real 221,41 s` (**3,69 phút**) là **wall host**, **không**
phải ngưỡng cổng: QD §5.2 đặt **60 s cho L1 1.000 seed** (lần này **653 ms** ⇒ **đạt**); wall host
221,41 s là thời gian dưới tải (baseline cùng ngày **223,87 s**, KT-32 từng **124,25 s**) — **không**
gắn T-INFRA cho cổng, không nới ngưỡng, không retry.

### 5c. **KT-34** — gate cuối (sau khi sửa C2 + driver)

`./v2/bin/v2 gate --fast` (**không retry**), log [`JOURNAL/kt34-gate-fast.log`](../JOURNAL/kt34-gate-fast.log):

**Kết quả**: `v2 gate --fast: PASS` (gen-check ON), **468 test / 0 failed / 62 suite** — đúng
**+7** test so baseline 461 (5 `kt25_ipc2` + 1 `kt27_notification` + 1 `kt28_waitaddr`); fmt/clippy
`-D warnings` sạch; L1 1.000 seed **275 ms** (ngưỡng QD §5.2 = 60 s ⇒ **đạt**); `real 239,32 s`
(**3m59**) là **wall host**, không phải ngưỡng cổng (baseline cùng ngày **236,86 s**) — **không**
nới ngưỡng/không retry. Driver `l1_k1c` bản KT-34 chạy 1k×4 riêng lúc làm bài
(`ipc_sent` 14.684/14.577/14.539/14.615; `ipc_delivered` 2.749/2.788/2.793/2.910).

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
| **C1** | **Bảo toàn message** — mỗi `msg_id` đúng một nơi; 0 vi phạm trên 1.000 seed `--fast` **và** 100.000 seed nightly | **ĐẠT** (KT-32 — §2b; KT-33 không đổi kết luận) — KT-31 chỉ đạt một phần (100k **không** phủ IPC); **KT-32** driver trộn IPC: 100k×4, `ipc_sent` **5.786.297**, `ipc_delivered` 1.123.586, `ipc_dropped` 3.748.540, 0 vi phạm bảo toàn trên **51,2 triệu** lần kiểm. `dropped` là **huỷ hợp lệ** (wake/huỷ TCB gửi đang chặn + Reply không `caller`, D2-25), **không** phải message "bốc hơi". | `check_ipc()` sau **mỗi** public op (`kobj.rs::sched_after_op`); bất biến `ipc_sent == ipc_delivered + ipc_pending + ipc_dropped`. Fuzz **1.000 seed × 64 op** trong `kt24_ipc`, `kt25_ipc2`, `kt26_fault`, `kt27_notification`, `kt28_waitaddr`, `kt29_smp` — 0 vi phạm, cùng seed cùng trace. 100k×4 (`kt32-l1-100k-mixed.log`): 14,47 message/seed (prologue 1 + ~13,5 do 64 op) ⇒ `check_ipc` **không còn rỗng**. **Giới hạn** (không tô hồng): mô hình 1 luồng, `dropped` là huỷ hợp lệ — không phải "0 message bị huỷ"; C2 không suy ra từ C1. |
| **C2** | **Không mất đánh thức** — signal/wake sau khi waiter xếp hàng ⇒ phải đánh thức; trước ⇒ word/giá trị còn | **ĐẠT (KT-34 — §2d)** | (a) **IPC/Reply**: cap `Reply` đang `caller=Some` bị **từ chối `Busy`** ở `recv`/`call_msg`/`reply_recv`/`nb_send_recv` **trước** mọi thay đổi ⇒ **không còn đường ghi đè `caller`**; bất biến **`check_reply_links`** (`Reply.caller=Some(t) ⇔ t BlockedOnReply` trên **đúng** object) chạy sau **mỗi** public op; cổng `transition_tcb` cũng giữ liên kết (vào bind, rời gỡ, ghi đè ⇒ `IllegalOperation`). Bằng chứng: 5 test `kt25_ipc2` (đỏ trước [`JOURNAL/kt34-red-reply-reuse.log`](../JOURNAL/kt34-red-reply-reuse.log): `left: Ok(())` vs `right: Err(Busy)`), property **6/6 hoán vị** ở `kt27_notification::property_signal_sau_khi_xep_hang_danh_thuc_moi_thu_tu` + `kt28_waitaddr::property_wakeaddr_sau_khi_xep_hang_danh_thuc_moi_thu_tu`; `.scn` 04/05/07/08 vẫn xanh (không đổi). (b) **100k×4** (§2d) với driver chỉ cấp Reply **rảnh** + invariant sau mỗi op: 400.000 lượt, `ipc_sent` ****5.786.296****, 0 panic/0 vi phạm `check_*`; 10k×4 **661 s**. **Giới hạn** (không tô hồng): mô hình **1 luồng**; "giữ khoá" = **một hàm**, không phải Mutex/thread thật (K2/K3 — nợ §9.1); 100k là bằng chứng **bất biến mô hình**, **không** phải kernel/QEMU; `Busy` là **model-only** (ABI/`spec/` không đổi). |
| **C3** | **Mọi đột biến chuẩn bị bắt** — mỗi đột biến làm **đúng một** test đỏ, revert, ghi `mutants: killed` | **ĐẠT — 12/12 mục §7.2** | Bảng **§3b** (KT-33): M1..M12, mỗi ca sửa **một chỗ**, chạy **đúng một** test đích, `exit 101` + `test result: FAILED` + dòng assert; **12/12** khôi phục `git hash-object` == `HEAD:<file>` (kobj `37ff523…`, ipc `2173300…`, mailbox `5747220…`). Log [`JOURNAL/kt33-mutation.log`](../JOURNAL/kt33-mutation.log). Hai deviation câu chữ §7.2 (M5 chọn đúng assert của đường `Signal`; M6: mô hình chỉ có **một** `waiter`, nên ca sửa cổng "một waiter" thay vì "wake all") đã ghi rõ ở §3b — chờ reviewer soi. |
| **C4** | **100.000 seed × smp 1/2/4/8** — 4 lần chạy, cùng seed cùng trace, bất biến giữ | **ĐẠT theo định nghĩa C4** (4 lần chạy, cùng seed cùng trace; cả bản sched/SMP lẫn bản trộn IPC của KT-32) | §2 (KT-31): `400000 lượt`, wall **5043,84 s**, 4/4 dòng `# L1 OK`, `1 passed; 0 failed`, 0 panic trên 25,6 triệu op (chỉ sched/SMP/mail — `check_ipc` rỗng); §2b (KT-32): chạy lại **cùng lệnh** với driver trộn IPC — 400.000 lượt, `ipc_sent` 5.786.297, 0 vi phạm trên **51,2 triệu** lần kiểm, cùng seed ⇒ cùng trace. C4 **không** tự kéo theo C1 — C1 có bằng chứng riêng (§2b). |

## 8. Đối chiếu §5 KT-24..KT-30

Số test dưới đây lấy **tươi** từ lần chạy `gate --fast` của KT-31
([`JOURNAL/kt31-baseline-gate-fast.log`](../JOURNAL/kt31-baseline-gate-fast.log), tổng **461 test**;
**KT-34**: tổng **468 test** — 5 `kt25_ipc2` + 1 `kt27` + 1 `kt28`, §5c);
Miri lịch sử lấy từ log từng task, và lần chạy **Miri toàn bộ** của KT-31 ở §4.

| Task | DoD §5 | Kết quả | Test hiện tại (gate 461 + 7 của KT-34) | Bằng chứng chính |
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
| 1 | `nightly --l1 --seeds 100000 --smp 1,2,4,8` | 4 log/dòng `# L1 OK`, cùng seed cùng trace, 0 vi phạm | **ĐẠT** — KT-31 §2 (sched/SMP/mail, 5043,84 s) **+ KT-32 §2b** (driver trộn IPC: 400.000 lượt, `ipc_sent` 5.786.297) ⇒ C1 ĐẠT |
| 2 | `gate --fast` | PASS không retry, ≥ 263 test | **ĐẠT** — §5/§5b/§5c (**468 test**; L1 275 ms ⇒ trong ngưỡng 60 s; wall 239 s là wall host) |
| 3 | `nightly --only-miri` | danh sách có `kt24..kt30` + `vifast-sync` PASS | **CHƯA ĐẠT ở KT-31** (thiếu `kt25_ipc2`) → **ĐẠT sau KT-33** — danh sách đã có `kt25_ipc2`, 19 tập/**300 test**, 0 failed (§4b) |
| 4 | Mutation tay §7.2 | mỗi đột biến **đúng một** test đỏ → revert → log | **CHƯA ĐẠT ở KT-31** (6 ca/3 mục) → **ĐẠT sau KT-33 — 12/12 mục §7.2** (§3b) |
| 5 | Bảng DoD K1c | 4 dòng C1..C4 + log; dòng chưa đạt ghi rõ | **ĐẠT** — §7 (sau KT-34: C1 **ĐẠT**, **C2 ĐẠT** — Reply active/invariant/100k, C3 **ĐẠT 12/12**, C4 ĐẠT theo định nghĩa) |

## 9. Phần **chưa** phủ (không tô hồng)

1. **Toàn bộ K1c là model-only trong sim 1 luồng**: không chạy kernel/QEMU, không thread thật;
   `vifast-sync` chỉ stress **host** (Miri + 4 thread trong test), **chưa** nối vào kernel/sim.
   "SMP 1/2/4/8" là **mô hình tất định N hart** trong một luồng (D2-53 Q1), không phải SMP thật.
2. **[ĐÃ ĐÓNG bởi KT-32 (C1) + KT-34 (C2) — §2b/§2d]** Lịch sử: KT-31 chạy 100k nhưng driver
   `l1_k1c` chỉ có 9 op sched/SMP/mail ⇒ **chưa** chứng minh bảo toàn message ở 100k. KT-32 mở
   driver trộn IPC và chạy lại **100k×4** ⇒ **C1 ĐẠT** (`ipc_sent` 5.786.297 bản KT-32).
   KT-34 đóng **C2**: cap `Reply` đang `caller=Some` bị **từ chối `Busy`** (không ghi đè caller),
   bất biến `check_reply_links` sau mỗi op, property C2 theo **mọi** thứ tự bước, driver chỉ cấp
   Reply **rảnh** + **100k×4** 0 vi phạm ⇒ **C2 ĐẠT** (§2d). **Giới hạn còn nguyên**: mô hình
   **1 luồng**, không Mutex/thread thật — nợ §9.1 (K2/K3), không phải nợ K1c.
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
8. **[ĐÃ ĐÓNG bởi KT-33 — §3b]** Mutation: KT-31 mới 6 ca (3/12 mục §7.2); **KT-33 chạy đủ
   M1..M12** — mỗi ca một chỗ sửa, đúng một test đích, `exit 101` + `test result: FAILED` + dòng
   assert, 12/12 khôi phục hash == `HEAD` ([`JOURNAL/kt33-mutation.log`](../JOURNAL/kt33-mutation.log)).
   Hai deviation **câu chữ** §7.2 (M5 chọn đúng assert của đường `Signal`; M6 mô hình chỉ có
   **một** waiter nên ca sửa cổng "một waiter" thay vì "wake all") ghi rõ ở §3b — **không** phải
   nới ngưỡng, nhưng reviewer cần soi.
9. **[ĐÃ ĐÓNG bởi KT-33 — §4b]** Miri: KT-31 thiếu `kt25_ipc2`; KT-33 đã thêm vào danh sách
   `nightly --only-miri` (`v2/bin/v2`) và chạy lại — **19 tập / 300 test**, 0 failed, có
   `kt25_ipc2` **24 test**. Vẫn **không** chạy `l1`/`kt2N_scn` dưới Miri (I/O — D2-47(a)); các
   test stress đa luồng tự gạt bằng `#![cfg(not(miri))]`.
10. **Không** chạm `viscript/` (đang có thay đổi của tab khác) và **không** cam kết gì về dòng
    ViScript trong báo cáo này.

## 10. Đề xuất

- **KT-31 (task gốc của báo cáo): DONE** theo **D2-61** ("gate run một phần") — đã chạy 5 việc
  §7.1 + parity 10k; 3 nợ chuyển KT-32/KT-33.
- **KT-32: DONE** — **C1-100k ĐẠT** (§2b): driver trộn IPC, 100k×4, `ipc_sent` **5.786.297**,
  0 vi phạm trên 51,2 triệu lần kiểm; **không** claim C2.
- **KT-33: DONE** — **C3 12/12** (§3b), **Miri #3 ĐẠT** (19 tập/300 test, có
  `kt25_ipc2` — §4b), driver siết `ipc_sent > 1` + `ipc_delivered > 0` + panic `kiem` kèm
  `seed/smp` (§2c), 10k×4 PASS (§2c), gate cuối PASS **461 test** (§5b).
- **KT-34 (task này): DONE** — **C2 ĐẠT** (§2d): `recv`/`call_msg`/`reply_recv`/`nb_send_recv`
  từ chối cap `Reply` đang `caller=Some` bằng `Busy` **trước** mọi thay đổi; bất biến
  `check_reply_links` sau **mỗi** public op; cổng `transition_tcb` giữ liên kết; property C2
  **mọi** thứ tự (kt27/kt28); **10k×4 661 s** + **100k×4** **6306,38 s (105m06)** + gate **468 test** (§5c)
  + Miri 3 tập ảnh hưởng (§4c). Reviewer độc lập `reviewer-kt34`: **APPROVE code**.
- **K1c (phase): DONE (đề xuất)** — **C1 ĐẠT** (KT-32), **C2 ĐẠT** (KT-34 — §2d),
  **C3 ĐẠT 12/12** (KT-33), **C4 ĐẠT theo định nghĩa** (4 lần 100k×4, cùng seed cùng trace), Miri
  ĐẠT (KT-33 toàn bộ + KT-34 3 tập ảnh hưởng), gate **468 test**. **Nợ còn lại KHÔNG thuộc K1c**:
  **model-only** (sim 1 luồng, không kernel/QEMU — §9.1), **K2** (VM/Frame/ASID/TLB — §9.3),
  **K3** (D2-56..D2-60 — §9.4) — báo cáo **không** được đọc như "K1c đã kiểm trên kernel/QEMU".
  Đề xuất Agent 0 lật **K1c DONE** và ghi D2 mới; **không** tự lật QUEUE.
- **Không lật QUEUE**: báo cáo không tự đổi trạng thái task nào; Agent 0 quyết định lật
  KT-31/KT-32/KT-33 và K1c.
