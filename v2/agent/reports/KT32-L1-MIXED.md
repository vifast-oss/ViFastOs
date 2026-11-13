<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# KT-32 — driver L1 **trộn IPC** cho K1c, đóng nợ **C1-100k** (D2-61)

Người ghi: **Agent 2 dòng OS** · Ngày: 2026-11-13/14 · Task: **KT-32** · Nhánh `main` ·
HEAD khi mở task `83df94a`, HEAD khi commit `efa787f` (hai commit `739fc3e`/`efa787f` của dòng
ViScript đến trong lúc chạy, **không chạm `v2/`**).

**Phạm vi**: **chỉ** driver test [`v2/sim/vifast-sim/tests/l1_k1c.rs`](../../sim/vifast-sim/tests/l1_k1c.rs).
Không sửa semantics `kobj/ipc/...`, không `.scn`/`spec`/`vifast-model`/QUEUE/DECISIONS/AGENTS/
`viscript/`, không `#[ignore]`/`unsafe`/dependency, không nới seed/ngưỡng/op (giữ `SO_OP = 64`).

Nguồn đối chiếu: `v2/agent/DECISIONS.md` **D2-61**; `reports/K1C-DOD.md` §2 + §9.2 (nợ C1);
`review/agent0/K1C-PLAN.md` §7.1 (cổng cuối) + §7.2; `tests/l1_k1c.rs` của KT-29.

## 0. TL;DR

| # | Việc KT-32 | Kết quả | Log |
|---|---|---|---|
| 1 | Mở rộng driver: 9 op cũ + 10 op K1c (Send/NBSend/Recv/Call/Reply, Signal/Wait, WaitAddr/WakeAddr/Store) | **Xong** — 19 nhánh op, mọi op đi qua cap/object **thật**; có nhánh fallback theo state | `tests/l1_k1c.rs` |
| 2 | 6 bất biến sau **mỗi** op + `ipc_sent > 0` + `sent == delivered+pending+dropped` + cùng seed ⇒ cùng trace | **Xong** — `check_sched/check_harts/check_ipc/check_waitaddr/check_mailboxes/check_caps` sau mỗi op (kể cả prologue); 2 lần/seed + `assert_eq!(trace, trace)` | `JOURNAL/kt32-l1-1k-mixed.log` |
| 3 | `nightly --l1 --seeds 100000 --smp 1,2,4,8` | **PASS** — 4 dòng `# L1 OK` smp 1/2/4/8, **400.000 lượt**, 0 panic | `JOURNAL/kt32-l1-100k-mixed.log` |
| 4 | `gate --fast` cuối (≥ 461 test) | **PASS 461 test**, fmt/clippy `-D warnings` sạch | `JOURNAL/kt32-gate-fast.log` |
| 5 | Review độc lập trước commit | **APPROVE** (reviewer `reviewer-kt32`, lead spawn; tự chạy 100/1k seed + gate) | §5 |

**Đề xuất trạng thái**: **KT-32 DONE** — nợ **C1** (driver L1 **trộn** + 100k×4) đã đóng bằng số
đo. **K1c vẫn REVIEW**: KT-33 còn **C3** (đủ 12 mutation §7.2) + Miri `kt25_ipc2` (D2-61).

## 1. Baseline (trước khi sửa)

- `agentctl next --project v2` ⇒ **KT-32** (READY); `git rev-list --count HEAD..origin/main` = 0.
- `./v2/bin/v2 gate --fast` **PASS 461 test**, wall **233,94 s** (máy tải), L1 cũ 1.000 seed
  **2.846 ms** — log [`JOURNAL/kt32-baseline-gate-fast.log`](../JOURNAL/kt32-baseline-gate-fast.log).
- Driver cũ (KT-29): 9 op sched/SMP/mail, **không** sinh Send/Recv/Call/Reply/Signal/WaitAddr ⇒
  `check_ipc` chỉ chạy "đường nhanh" (`ipc_sent == 0`), đúng như K1C-DOD §9.2 nêu.

## 2. Driver mới — 10 op K1c thêm (giữ nguyên 9 op cũ)

| Nhóm | Op | API | Cap/object (test-only, **hợp lệ**) |
|---|---|---|---|
| Send | 9 | `Machine::send` | Endpoint (chặn ⇒ `BlockedOnSend`) |
| NBSend | 18 | `Machine::nb_send` | Endpoint (không chặn) |
| Recv | 10 | `Machine::recv` | Endpoint + **có/không** cap Reply (`Option<SlotRef>`) |
| Call | 11 | `Machine::call` | Endpoint (1 MR) |
| Reply | 12 | `Machine::reply_msg` | Reply **đang có `caller` được ưu tiên** |
| Signal | 13 | `Machine::send` trên cap Notification | cap gốc + cap `mint` badge `0x0010` |
| Wait | 14 | `Machine::recv` trên cap Notification | cap gốc |
| WaitAddr | 15 | `Machine::wait_addr(tcb, frame, offset, expected)` | `expected = addr_word(frame, offset)` |
| WakeAddr | 16 | `Machine::wake_addr(frame, offset, n)` | `n ∈ {1,2}`, 4 offset (0/8/16/64) |
| Store | 17 | `Machine::set_addr_word` | ghi "từ" (mô hình bộ nhớ K2) |

Bố cục rig (CNode 16 ô, **giữ nguyên 9 slot của KT-29**): `0` Untyped, `1..2` Endpoint,
`4` SchedContext, `5` SchedControl, `6..7` Reply, `8..11` TCB (4), `12` Notification,
`13` cap `mint` badge. Mọi cap là **cap gốc đặt trong CNode** (không cap giả), `check_caps`
kiểm quan hệ cap ↔ object ↔ slot sau mỗi op.

**Hợp lệ theo state + fallback** (yêu cầu "không panic/không lỗi giả"):

- Mọi op IPC/Wait/WaitAddr chỉ gọi khi TCB `Ready|Running` (`ensure_started` của mô hình trả
  `IllegalOperation` cho state khác). `Rig::san_sang` chọn TCB chạy được theo seed; **cạn ứng
  viên** ⇒ `wake` **một** TCB rồi thử lại; vẫn cạn ⇒ dòng trace ghi `- (không TCB chạy được)`,
  **không** panic, **không** gọi op sai state.
- `WaitAddr` dùng `expected` = giá trị **hiện tại** (đọc rồi xếp hàng trong cùng hàm — đúng
  KT-28), nên không tạo `InvalidArgument` giả; `Store` đổi giá trị để lượt sau có động lực đánh
  thức thật.
- Prologue `NBSend` (không chặn ⇒ TCB vẫn `Ready`) bảo đảm `ipc_sent ≥ 1` **ngay từ seed đầu**;
  ~93% `ipc_sent` còn lại do 64 op ngẫu nhiên sinh (đo: 14,67 send/seed ở 100 seed; 14,68/seed ở
  1.000 seed).
- `Frame` là object **reserved** tới K2 nên khoá WaitAddr dùng **ObjId thô** (Untyped gốc) —
  đúng tiền lệ KT-28.

**Bất biến** (sau **mỗi** op, kể cả prologue; vi phạm ⇒ panic kèm dòng gây lỗi): `check_sched`,
`check_harts`, `check_ipc`, `check_waitaddr`, `check_mailboxes`, `check_caps`. Cuối **mỗi** lượt:
`ipc_sent > 0` và `ipc_sent == ipc_delivered + ipc_pending + ipc_dropped`. **Tất định**: seed quyết
định op + tham số; mỗi `(seed, smp)` chạy **hai lần** và `assert_eq!` cả **trace** (byte-for-byte)
lẫn **bộ đếm** `(ipc_sent, ipc_delivered, ipc_dropped)`.

## 3. Số đo L1

### 3.1 Quét 1k × smp 1/2/4/8 (`JOURNAL/kt32-l1-1k-mixed.log`)

| smp | `ipc_sent` | `ipc_delivered` | `ipc_dropped` | Kết quả |
|---|---|---|---|---|
| 1 | 14.684 | 2.751 | 9.747 | PASS |
| 2 | 14.577 | 2.788 | 9.530 | PASS |
| 4 | 14.539 | 2.793 | 9.393 | PASS |
| 8 | 14.615 | 2.912 | 9.334 | PASS |

Wall **62,54 s** (4.000 lượt). Không panic ⇒ **0** vi phạm trên 4.000 × 64 = **256.000 op**.

### 3.2 Quét 10k × smp 1/2/4/8 (`JOURNAL/kt32-l1-10k-mixed.log`)

| smp | `ipc_sent` | `ipc_delivered` | `ipc_dropped` | Kết quả |
|---|---|---|---|---|
| 1 | 144.977 | 27.850 | 95.461 | PASS |
| 2 | 144.873 | 28.154 | 93.917 | PASS |
| 4 | 144.552 | 28.174 | 92.928 | PASS |
| 8 | 145.058 | 28.405 | 93.239 | PASS |

Wall **503,02 s** (40.000 lượt = **2,56 triệu op**), 0 panic.

### 3.3 Cổng cuối: 100k × smp 1/2/4/8 (`JOURNAL/kt32-l1-100k-mixed.log`)

Lệnh (đúng §7.1, **không** giảm seed, **không** đổi invariant):

```
./v2/bin/v2 nightly --l1 --seeds 100000 --smp 1,2,4,8
```

**Kết quả** — log [`JOURNAL/kt32-l1-100k-mixed.log`](../JOURNAL/kt32-l1-100k-mixed.log):

| smp | Dòng log | `ipc_sent` | `ipc_delivered` | `ipc_dropped` | `pending` cuối lượt |
|---|---|---|---|---|---|
| 1 | `# L1 OK: smp 1, 100000 seed × 64 op` | 1.447.334 | 279.209 | 949.749 | 218.376 |
| 2 | `# L1 OK: smp 2, 100000 seed × 64 op` | 1.447.459 | 280.812 | 937.672 | 228.975 |
| 4 | `# L1 OK: smp 4, 100000 seed × 64 op` | 1.445.714 | 282.294 | 931.485 | 231.935 |
| 8 | `# L1 OK: smp 8, 100000 seed × 64 op` | 1.445.790 | 281.271 | 929.634 | 234.885 |
| **tổng** | `# L1 OK: 400000 lượt (smp [1, 2, 4, 8], seeds 100000)` | **5.786.297** | **1.123.586** | **3.748.540** | **914.171** |

- **Wall**: `real 6596.88 s` = **109,95 phút** (1 h 49 m 57 s; `user 5934,13 s`; `sys 38,53 s`),
  đo bằng `/usr/bin/time -p` trong chính log; test binary `finished in 6596.43s`, `1 passed;
  0 failed`; **không** retry. `[ƯL]` từ 1k×4 (64 s) là ~107 phút ⇒ sát dự toán; chậm hơn `[ƯL]`
  20 phút của K1C-PLAN §7.1 vì 4 cấu hình × 100k seed × **2 lần** chạy/seed + máy tải — **T-INFRA**
  về dự toán, **không** nới seed/ngưỡng.
- **Cùng seed ⇒ cùng trace**: mỗi `(seed, smp)` chạy **hai lần** và `assert_eq!` cả trace
  (64 dòng, byte-for-byte) lẫn `(ipc_sent, ipc_delivered, ipc_dropped)` — 400.000 cặp bằng nhau;
  lệch ⇒ panic (exit ≠ 0). Log `1 passed; 0 failed`.
- **Bất biến sau mỗi op**: 6 `check_*` sau **mỗi** op, kể cả prologue ⇒ 400.000 lượt × 64 op ×
  2 lần chạy = **51,2 triệu lần kiểm**; log **0** panic ⇒ **0** vi phạm.
- **`ipc_sent` mỗi lượt**: 5.786.297 / 400.000 = **14,47 message/seed** (prologue đóng góp 1;
  ~13,5 còn lại do 64 op ngẫu nhiên sinh) ⇒ **`ipc_sent > 0` là chuyện thật của driver**, không
  chỉ tautology prologue (đo 100 seed: 14,67/seed; 1k: 14,58/seed).
- **Bảo toàn message**: `sent == delivered + pending + dropped` đúng ở **mọi** op; toàn cục
  `5.786.297 = 1.123.586 + 914.171 + 3.748.540`.

## 4. Gate cuối

- `./v2/bin/v2 gate --fast` (sau 100k, **không retry**): **PASS 461 test**, fmt + clippy
  `-D warnings` sạch, R1..R5 OK, gen-check ON; L1 cũ 1.000 seed **269 ms**; wall **124,25 s**
  (`real`; baseline cùng ngày 233,94 s do máy tải) — log
  [`JOURNAL/kt32-gate-fast.log`](../JOURNAL/kt32-gate-fast.log).
- Driver `l1_k1c` **cũng** được cổng chạy trong **L0** (tên test không bắt đầu `l1_`) ở mặc định
  **1.000 seed / smp 1**: `test driver_smp_cung_seed_cung_trace ... ok`, **4,71 s**, `1 passed`
  ⇒ driver xanh ngay ở ngưỡng cổng, L1 1.000 seed cũ **không hồi quy** (269 ms).
- Không chạy lại cổng lần hai sau lần PASS này (D2-07).

## 5. Review độc lập (trước commit)

Reviewer **`reviewer-kt32`** do lead spawn (read-only), verdict gửi lead + tác giả:

> **VERDICT: APPROVE** (không cần sửa driver; 100k cứ chạy tiếp, đừng dừng/restart).

Bằng chứng reviewer **tự chạy** (không sửa gì, không `git add`/commit, không đụng `viscript/`):

- 100 seed × smp 1/2/4/8: PASS **5,22 s**; `ipc_sent 1467/100 seed` = 14,67/seed ⇒ ~93% do 64 op,
  không chỉ prologue.
- 1.000 seed × smp 1/2/4/8: PASS **64,03 s**; bộ đếm khớp **byte-for-byte** log 1k của tác giả
  (14684/2751/9747; 14577/2788/9530; 14539/2793/9393; 14615/2912/9334) ⇒ tất định **qua tiến trình**.
- `gate --fast`: PASS, exit 0, **461 test**, fmt/clippy `-D warnings` OK — chạy trên **đúng** tệp
  driver đã sửa (HEAD `efa787f`, hai commit ViScript không chạm `v2/`); `l1_k1c` trong L0 **14,63 s**,
  L1 cũ 706 ms.
- Đọc log 10k×4: PASS 503,02 s, 40.000 lượt, 0 panic.

Reviewer xác nhận: đủ **10 op mới** + **6 checker** sau mỗi op, tất định, chỉ **1 tệp test**, không
`#[ignore]`/`unsafe`/dependency, không nới seed. Gate của reviewer chạy ~23:30–23:33:48 nên
**chồng lấn ~2 phút** đầu 100k (12 core) — **T-INFRA**, không ảnh hưởng tính đúng, **không** chạy lại.

**Ghi nợ (reviewer + lead, chuyển KT-33)**:

1. `ipc_sent > 0` hiện **gần tautology** nhờ prologue ⇒ KT-33 nên siết thành `ipc_sent > 1` /
   `ipc_delivered > 0` mỗi lượt.
2. Panic của `kiem` chưa kèm `seed`/`smp` (chỉ có dòng op) ⇒ thêm ngữ cảnh ở KT-33.
3. **Không** claim **C2**: driver dùng lại **cùng cap Reply** cho nhiều `Recv`; model cho phép
   `reply_set_caller` ghi đè `caller` cũ ⇒ caller trước có thể **mất đường Reply** (lỗ hổng **có
   sẵn** của model, không phải C1) — đã ghi vào K1C-DOD §2b/§7.

## 6. Phần **chưa** phủ (không tô hồng)

1. **Model-only `[TK]`**: driver chạy trên mô hình một luồng; "SMP 1/2/4/8" là **mô hình tất
   định N hart** (D2-53 Q1), không phải SMP thật/thread/QEMU.
2. **`ipc_dropped` ≠ "mất message"**: phần lớn `ipc_dropped` là message bị **huỷ hợp lệ** khi
   TCB gửi đang chặn bị `wake`/huỷ (D2-25 — `unlink_tcb_queues` giữ bất biến
   `pushed == popped + removed + len`) và Reply không có `caller` (`reply_with` đếm
   `ipc_dropped`). Bất biến bảo toàn `sent == delivered + pending + dropped` **đúng sau mỗi op**
   (không phải "0 mất" theo nghĩa ngây thơ).
3. **Prologue NBSend** bảo đảm `ipc_sent ≥ 1`; nó là **setup** (1/seed), không phải op ngẫu
   nhiên — số `ipc_sent` còn lại (~13,7/seed) do 64 op sinh.
4. **Không** chạy driver này dưới Miri (I/O + 100k seed; D2-47(a)) và **không** chạm C3/Miri —
   hai nợ còn lại thuộc **KT-33** (D2-61). Báo cáo này **không** đóng K1c.
5. **Không** chạm `viscript/` (đang có thay đổi của tab khác); không cam kết gì về dòng ViScript.
6. Wall 100k chịu **T-INFRA** nếu máy tải (reviewer chạy `gate --fast` chồng lấn ~2 phút đầu;
   dòng ViScript mở ST-49) — **không** giảm seed, không retry.

## 7. Đề xuất

- **KT-32: DONE** — driver L1 **trộn** đã sinh Send/Recv/Call/Reply/Signal/Wait/WaitAddr/
  WakeAddr trên cap/object thật, 100k×4 **400.000 lượt** PASS, 6 bất biến sau mỗi op,
  `ipc_sent > 0` từng lượt, bảo toàn message cuối lượt, cùng seed ⇒ cùng trace, gate PASS.
  Đề nghị Agent 0 lật `DONE` + ghi nợ **C1-100k đã đóng** (D2-61) và **giữ K1c REVIEW**.
- **K1c: REVIEW** — còn **C3** (12 mutation §7.2) + Miri `kt25_ipc2` ⇒ **KT-33**.
