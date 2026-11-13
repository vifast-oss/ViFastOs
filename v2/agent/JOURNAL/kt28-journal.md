# KT-28 — WaitOnAddress: bảng băm `(frame, offset)`, Wait đọc giá trị + xếp hàng một lần, `WakeAddr(n)` (K1c)

Agent 2 dòng OS · 2026-11-13 · HEAD vào `8e36f55` ([KT-27] DONE) · nhánh `main`,
`git rev-list --count HEAD..origin/main` = **0** lúc bắt đầu ·
`gate --fast` baseline PASS **413 test/55 suite** (L1 1.000 seed 736 ms) —
[`kt28-baseline-gate-fast.log`](kt28-baseline-gate-fast.log).

## 1. Đối chiếu DoD (K1C-PLAN §5 KT-28)

| # | DoD | Bằng chứng |
|---|---|---|
| 1 | Bảng băm `[Vec<(ObjId, u32, ObjId)>; WAITADDR_BUCKETS]`, `WAITADDR_BUCKETS = 64` `[TK]` (Q3: hằng **model** vì `spec/kernel.toml` chưa tồn tại, **không** tạo); hash `(frame.wrapping_mul(0x9E37_79B1) ^ offset) % 64`; tìm waiter theo **(frame, offset) đầy đủ** | `kobj.rs`: `WAITADDR_BUCKETS`, `waitaddr_bucket`, field `address_waiters`; test `bang_bam_64_bucket_va_cung_khoa_cung_bucket` (1.000 cặp, trải ≥ 8 bucket) + `va_cham_bucket_khong_anh_huong_tim_kiem` (cặp va chạm có chủ ý `(ep,0)`/`(ep,64)`) |
| 2 | `wait_addr(tcb, frame, offset, expected)`: **một** hàm so word với `expected` (`InvalidArgument` nếu lệch, **không** xếp hàng) **và** xếp hàng `BlockedOnAddress{frame, offset}`; test "lệch giá trị ⇒ không để lại waiter" | `Machine::wait_addr`; test `expected_lech_tra_ngay_khong_xep_hang` (state `Ready`, `address_waiters_len()==0`, bucket rỗng); `xep_hang_mot_lan` (gọi lần hai ⇒ `IllegalOperation`, một waiter) |
| 3 | `wake_addr(frame, offset, n)`: đánh thức **tối đa n** waiter **cùng khoá** (FIFO), trả số đã đánh thức; `n = u32::MAX` = tất cả; waiter còn lại vẫn `BlockedOnAddress`; test 3 waiter + n=2 | `Machine::wake_addr`; test `wake_0_1_n_nhieu_va_fifo` (n=0/1/2/MAX, FIFO) + `hai_frame_offset_doc_lap` |
| 4 | **Không mất đánh thức** (C2) theo mọi thứ tự bước; Wake trước ⇒ trả 0 và không nuốt tín hiệu; 1.000 seed × 64 bước, 0 vi phạm | `khong_mat_danh_thuc_wake_truoc_va_sau` + `fuzz_1000_seed_64_op_bat_bien_va_tat_dinh` (1.000 seed × 64 op, `check_sched`/`check_ipc`/`check_waitaddr`/`check_caps`/`check_all_bytes` sau **mỗi** bước, 2 lượt/seed so trace) |
| 5 | **Đột biến**: "xếp hàng trước rồi mới so giá trị" ⇒ test (2) **đỏ**; log `JOURNAL` | [`kt28-mutation-red.log`](kt28-mutation-red.log): `check_waitaddr sau expected lệch: waiter khoá (3,16) của TCB 7 lệch trạng thái Ready` — đã khôi phục (sha256 `kobj.rs` trùng bản sạch) |
| 6 | `Suspend(Abort)`/`finalise` gỡ đúng **bucket** (không chỉ vector tuyến tính) — test đếm bucket rỗng | `unlink_tcb_queues` gỡ theo đúng bucket suy từ `BlockedOnAddress`; test `suspend_abort_go_dung_bucket`, `exit_va_finalise_go_waiter` (`waitaddr_bucket_len`=0, `waitaddr_nonempty_buckets`=0) |
| 7 | `check_sched` + `check_ipc` không bị ảnh hưởng; 1.000 seed giữ xanh | `sched_after_op` thêm `check_waitaddr` sau `check_notifications`; toàn bộ test KT-18/19/23/24/25/26/27 xanh; fuzz KT-28 kiểm cả 5 miền |
| 8 | `gate --fast` PASS; Miri `--test kt28_waitaddr` PASS | [`kt28-gate-fast.log`](kt28-gate-fast.log) **427 test/56 suite**, L1 1.000 seed **704 ms**, không retry; [`kt28-miri.log`](kt28-miri.log) **12/12** (105 s) + [`kt28-miri-lib.log`](kt28-miri-lib.log) 2 unit test |

**Không timeout** (D2-53 Q4 — spec thắng): không có `Sleeping{Timeout}` trong đường Wait; test
`khong_timeout_du_tick_rat_nhieu` (10 × 1.000 tick ảo, waiter vẫn `BlockedOnAddress`).

## 2. Test đỏ trước (D2-05)

- [`kt28-red-compile.log`](kt28-red-compile.log) — `tests/kt28_waitaddr.rs` **không biên dịch**:
  **24 lỗi** = `E0432` (`kobj::waitaddr_bucket`, `kobj::WAITADDR_BUCKETS`) + `E0599`
  (`Machine::{wait_addr, wake_addr, set_addr_word, addr_word, check_waitaddr,
  address_waiters_for, waitaddr_bucket_len, waitaddr_nonempty_buckets}`).
- [`kt28-red-semantics.log`](kt28-red-semantics.log) — probe **tạm (đã xoá)** trên mô hình trước
  KT-28: `tcb_wait_address` xếp hàng **vô điều kiện** (không có `expected`, không đọc giá trị);
  **không có** `WakeAddr` (chỉ `wake(tcb)` thủ công, không trả số); hàng chờ là `Vec` tuyến tính
  (không bucket).

## 3. Quyết định mô hình cần reviewer soi

1. **"Khoá" của mô hình một luồng = một hàm**: `wait_addr` đọc `addr_word` **và** push nút trong
   cùng hàm, không public op nào chen giữa (không `sched_after_op` giữa) ⇒ mô hình không có cửa
   sổ lost wake để biểu diễn; bằng chứng là property theo **thứ tự bước**, không phải thread thật
   (K1C-PLAN §5 KT-28 rủi ro (a); "Mutex std stress" thuộc K2/K3). `wake_addr` cũng chọn + gỡ nút
   trong cùng hàm.
2. **`WAITADDR_BUCKETS = 64` là hằng model `[TK]`** ghi ở `v2/docs/k1c-waitaddr.md` §1 —
   **không** tạo `spec/kernel.toml` (D2-53 Q3; spec chỉ Agent 0/task được ghi).
3. **`expected` lệch ⇒ `InvalidArgument`** (K1C-PLAN §5 KT-28 DoD 2). ABI 19 mã không có `EAGAIN`;
   conformance K3 muốn mã riêng thì ghi nợ (doc §6 câu 3).
4. **WaitAddr trên TCB `suspended` ⇒ `IllegalOperation`** (thay đổi so `tcb_wait_address` K1b):
   bất biến §4.4 "TCB trong hàng của X ⇔ trạng thái mang X **và** `!suspended`". Fuzz KT-28 phát
   hiện đúng lỗ này khi viết `check_waitaddr` (waiter của TCB `Ready + suspended`); đã chặn ở
   `wait_addr`. Toàn bộ test cũ xanh (KT-23 fuzz chỉ đếm `is_ok`).
5. **Giữ API K1b**: `tcb_wait_address`/`address_waiter`/`address_waiters_len` giữ nguyên chữ ký
   (KT-18/19/23 xanh); `tcb_wait_address` giờ đọc giá trị hiện tại rồi đi **cùng** `wait_addr`.
6. **`check_waitaddr` một chiều ở trạng thái**: `nút ⇒ TcbState` được kiểm; chiều ngược **không**
   (như `check_notifications`: `transition_tcb` là cổng thấp của ma trận 11×11 KT-23). Có unit
   test âm cho sai bucket / trùng / TCB chết / TCB suspended.
7. **`unlink_tcb_queues` + sweep nút chết theo bucket**: gỡ theo bucket suy từ trạng thái TCB;
   `debug_assert!` bắt phần sót (debug), `check_waitaddr` bắt mọi build sau public op.
8. **Frame chưa có (K2)**: khoá theo `ObjId` thô, **không** địa chỉ vật lý ⇒ mệnh đề "Frame chia
   sẻ hai tiến trình" (thiết kế `:750`) **chưa đạt**; unmap Frame đánh thức waiter `Cancelled`
   (thiết kế `:237`) cũng chưa (ghi nợ K2 trong doc §4).

## 4. Tệp thay đổi

| Tệp | Nội dung |
|---|---|
| `v2/sim/vifast-sim/src/kobj.rs` | `WAITADDR_BUCKETS`/`waitaddr_bucket`; field `address_waiters` (bảng băm) + `addr_words`; `set_addr_word`/`addr_word`/`wait_addr`/`wake_addr`; `tcb_wait_address` đi qua `wait_addr`; `address_waiters_for`/`waitaddr_bucket_len`/`waitaddr_nonempty_buckets`; `check_waitaddr` + gọi trong `sched_after_op`; `unlink_tcb_queues`/sweep theo bucket; 2 unit test checker |
| `v2/sim/vifast-sim/tests/kt28_waitaddr.rs` | **12 test** (bucket/va chạm, expected lệch, một lần, wake 0/1/n/MAX FIFO, hai khoá, mất đánh thức 2 chiều, suspend/exit/finalise, không timeout, checker âm, fuzz 1.000×64) |
| `v2/docs/k1c-waitaddr.md` | model doc `[TK]`: dữ liệu, ngữ nghĩa, bất biến, giới hạn (K2/K3/K6), bằng chứng, câu hỏi reviewer |
| `v2/bin/v2` | thêm `kt28_waitaddr` vào danh sách Miri `nightly --only-miri` (D2-47(a)) |
| `v2/agent/JOURNAL/kt28-*`, `v2/agent/STATE.md` | log + trạng thái |

**Không chạm**: `spec/`, `v2/lib/vifast-model`, `v2/test/scn/*.scn`, QUEUE, DECISIONS, AGENTS,
`viscript/` (tab khác đang sửa — không stage).

## 5. Kết quả cổng

| Lệnh | Kết quả |
|---|---|
| `./v2/bin/v2 check` | **PASS** — [`kt28-check.log`](kt28-check.log) |
| `./v2/bin/v2 gate --fast` | **PASS** — **427 test/56 suite**, L1 1.000 seed **704 ms**, không retry — [`kt28-gate-fast.log`](kt28-gate-fast.log) (chạy lại sau khi sửa trình bày theo review; trước đó 746 ms) |
| `./v2/bin/v2 miri -p vifast-sim --test kt28_waitaddr` | **PASS 12/12** (105,04 s) — [`kt28-miri.log`](kt28-miri.log) |
| `./v2/bin/v2 miri -p vifast-sim --lib check_waitaddr` | **PASS 2/2** (1,19 s) — [`kt28-miri-lib.log`](kt28-miri-lib.log) |

Miri giảm seed/bước bằng `cfg(miri)` (**3 seed × 8 op**) — stable vẫn **1.000 seed × 64 op**
(`fuzz_1000_seed_64_op_bat_bien_va_tat_dinh`). Không `#[allow]`/`#[ignore]` mới.

## 6. Review độc lập — **APPROVE**

`reviewer-kt28` (lead spawn, review-only) verdict **APPROVE** 2026-11-13. Reviewer tự chạy
`kt28_waitaddr` **12/12**, `gate --fast` **427 test/56 suite**, L1 1000/999 **753 ms**; xác nhận
bucket/va chạm, expected lệch không xếp hàng, `WakeAddr` n=0/1/2/`u32::MAX`, `check_waitaddr` sau
mỗi op, API K1b giữ nguyên, phạm vi không chạm `spec/`/`viscript/`; Miri **12/12/105 s** chấp nhận
(reviewer không tái lập đủ trong slot, không thấy lỗi).

**Sửa theo review (trình bày, không đổi logic)**: `v2/agent/STATE.md` bỏ **heading `## KT-27`
lặp** do chèn KT-28; `Bước` đổi `7/8` → `9/10`; stage **đường dẫn cụ thể** (không `git add -A`).

## 7. Nợ đã ghi (không chặn — reviewer xác nhận)

1. `addr_words` **không** được sweep khi `frame` chết: chưa có vòng đời Frame (K2 Frame thật; K3
   xem lại cùng Map/Unmap Frame).
2. Bất biến `check_waitaddr` **một chiều** (`nút ⇒ TcbState`); chiều `TcbState ⇒ nút` không kiểm —
   cùng lý do `check_notifications`/ma trận 11×11 KT-23 (cổng thấp dựng trạng thái cho coverage).
3. `wake_addr` **bỏ** `sched_after_op` nếu `wake_step` lỗi giữa chừng (hiện **không** xảy ra: mọi
   nút đều `BlockedOnAddress` theo bất biến; nếu có lỗi thì public op trả lỗi trước khi kiểm).
4. docs §6: "giữ khoá = một hàm" **chấp nhận cho K1c**; `WaitAddr` trên TCB `suspended` ⇒
   `IllegalOperation` là **nợ K3 conformance** (đã ghi vào `v2/docs/k1c-waitaddr.md` §6); giữ
   `InvalidArgument` (EAGAIN để K3); `set_addr_word`/`addr_word` là **model API**.

## 8. Trạng thái

**REVIEW** — reviewer độc lập **APPROVE**; commit `[KT-28]` đã push; **không** tự DONE, **không**
lật QUEUE (Agent 0 lật).
