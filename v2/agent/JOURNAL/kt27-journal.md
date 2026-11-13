# KT-27 — Notification: word 64 + badge, Signal OR, một waiter, Wait/Poll, bind 1:1 (K1c)

Agent 2 dòng OS · 2026-11-13 · HEAD vào `e5350f0` ([KT-00] lật KT-26 DONE) ·
`gate --fast` baseline PASS **386 test/53 suite** (L1 1.000 seed 719 ms) —
[`kt27-baseline-gate-fast.log`](kt27-baseline-gate-fast.log).

## 1. Đối chiếu DoD (K1C-PLAN §5 KT-27)

| # | DoD | Bằng chứng |
|---|---|---|
| 1 | `NotificationState { word: u64, badge: u64, waiter, bound_sc }` (`[TK]`) | `kobj.rs` (`NotificationState`); word 64 giữ bit cao `1<<63`: test `word_64_bit_cao_va_thap` |
| 2 | `signal(ntfn, badge)`: `word \|= badge`; có waiter ⇒ đánh thức **một**, giao word rồi `word = 0`; không waiter ⇒ giữ | `Machine::notification_signal`; test `signal_or_ba_lan_truoc_khi_wait`, `word_chi_xoa_khi_giao_cho_waiter`, `mot_waiter_duy_nhat_ba_tcb` |
| 3 | **Một waiter**: waiter thứ hai ⇒ `Busy` (giữ KT-18); 3 TCB chỉ 1 chờ được | test `mot_waiter_duy_nhat_ba_tcb` |
| 4 | `recv`/`nb_recv` trên cap Notification = **Wait**/**Poll**; word ≠ 0 trả ngay (word + badge), word = 0 ⇒ `BlockedOnNotification`/`Busy`; `.scn` 21 đúng 4 bước | `recv`/`nb_recv` nhánh `ObjType::Notification` (có `reset_ipc_error` như nhánh Endpoint); test `wait_immediate_khi_co_word`, `poll_rong_model_api_va_nb_recv_busy`, `nb_recv_tra_word_badge_va_xoa`, `recv_notification_xoa_last_ipc_error_cu`; `tests/kt27_scn.rs` |
| 5 | Bound: `tcb_bind_notification` + `SchedContext.Bind(sc, ntfn)` set `bound_sc`/`SchedData.bound`; signal đánh thức TCB `BlockedOnNotification` **và** `BlockedOnRecv`, trả badge; 2 nhánh | `notification_wait`/`notification_signal`/`sched_context_bind`; test `signal_danh_thuc_bound_tcb_dang_blocked_on_recv`, `signal_bound_khong_danh_thuc_tcb_bound_khac`, `bind_sched_context_notification_1_1` |
| 6 | Không mất đánh thức (C2): 1.000 seed × 64 bước trộn signal/wait/poll, bất biến sau **mỗi** bước, cùng seed cùng trace | `fuzz_1000_seed_64_op_bat_bien_va_tat_dinh` (`check_sched`/`check_ipc`/`check_notifications`/`check_caps`/`check_all_bytes` mỗi bước, 2 lượt/seed so trace) |
| 7 | Bất biến: word chỉ xoá khi giao cho waiter; `notification_waiter` khớp `TcbState`; notification chưa bind SC vẫn chạy | `Machine::check_notifications` (gọi trong `sched_after_op` sau **mỗi** public op + test tường minh); test `check_notifications_bat_waiter_lech`, `notification_khong_bind_sc_van_chay` |
| 8 | `gate --fast` PASS; Miri `--test kt27_notification` PASS; `.scn` 21 xanh | [`kt27-gate-fast.log`](kt27-gate-fast.log) **412 test/55 suite**, L1 606 ms; [`kt27-miri.log`](kt27-miri.log); `kt27_scn` 1/1 |

## 2. Test đỏ trước (D2-05)

- [`kt27-red-compile.log`](kt27-red-compile.log) — `tests/kt27_notification.rs` + `kt27_scn.rs`
  **không biên dịch**: **44 lỗi** = **1×`E0432`** (`kobj::NotificationValue`) + **43×`E0599`**
  (`Machine::{notification_signal, notification_word, notification_badge, notification_poll,
  notification_wait, notification_bound_sc, tcb_notification, take_tcb_notification,
  tcb_unbind_notification, sched_bound_notification, check_notifications}`, `Step::{Signal,
  WaitNotification, PollNotification}`).
- [`kt27-red-semantics.log`](kt27-red-semantics.log) — probe **tạm (đã xoá)** trên mô hình trước
  KT-27:
  1. `send(ntf0)` = `Err(NotSupported)` — chưa phải Signal;
  2. `recv(ntf0)` = `Err(WrongObjectType)` — chưa phân nhánh Wait;
  3. `tcb_wait_notification` chỉ đặt `waiter`, **không** có word/badge để OR/đọc/xoá;
  4. `tcb_bind_notification` cho phép bind **cùng một** notification cho **hai** TCB (vi phạm 1:1);
  5. `sched_context_bind(sc, ntf)` chỉ ghi `SchedData.bound`, không có `bound_sc`/1:1.

## 3. Quyết định mô hình cần reviewer soi

1. **`check_notifications` một chiều ở waiter**: `waiter ⇒ TcbState` được kiểm, chiều ngược
   (`TcbState ⇒ waiter`) **không** — vì `transition_tcb` là cổng thấp của ma trận 11×11 KT-23
   (dựng `BlockedOnNotification{ntfn}` với `ObjId` không tồn tại để phủ bảng), không thể buộc ghi
   waiter mà không phá KT-23. Mọi đường IPC thật (`notification_wait`, `signal`, `block_current`)
   đều ghi waiter; test âm `check_notifications_bat_waiter_lech` chứng minh chiều được kiểm bắt
   được lệch.
2. **`block_current(BlockedOnNotification)`** nay ghi `waiter` lên Notification (giữ bất biến
   `waiter ⇔ TcbState`); trả `Busy` nếu notification đã có waiter khác — không đổi chữ ký.
3. **`Signal` đánh thức TCB bound đang `BlockedOnRecv`** là model-only theo K1C-PLAN §5 KT-27 DoD
   5; seL4 conformance phải kiểm lại ở K3 (ghi ở `v2/docs/k1c-notification.md` §5).
4. **Badge 0 hợp lệ**: có waiter thì vẫn đánh thức (giao `(0,0)`), không dùng 0 làm "chưa có"
   (K1C-PLAN rủi ro (b)); test `badge_0_hop_le_va_van_danh_thuc_waiter`.
5. **Badge lấy từ `Cap.badge`** khi `Send`/`NBSend` trên Notification (seL4; cap gốc ⇒ 0); model
   API `notification_signal(ntfn, badge)` cho phép badge tường minh. `word == badge` là bất biến
   mô hình (hai trường cùng OR, cùng xoá) — giữ cả hai vì ABI `Recv` trả `value` + badge.
6. **1:1**: `tcb_bind_notification` — TCB đã bind notification khác ⇒ `IllegalOperation`;
   notification đã bind TCB khác ⇒ `Busy`; bind lại cùng notification ⇒ no-op `Ok`.
   `SchedContext.Bind(sc, ntf)` — ntf đã bind SC khác ⇒ `IllegalOperation`; SC đổi target ⇒ gỡ
   `bound_sc` cũ; `Delete`/`Revoke` cap Notification ⇒ gỡ `SchedData.bound` + `bound_sc`.
7. **Sửa kỳ vọng cũ `kt24_ipc`**: `Send` trên Notification từ `NotSupported` → `Ok` (Signal,
   badge cap 0) — chủ ý KT-27, không nới ngưỡng (không đổi số test).
8. **`NBRecv` trên Notification trả `Message`** (`MR0 = word`, `badge`); kết quả **không** đi qua
   `ipc::transfer` (Notification không phải message IPC) nên `check_ipc`/`transfer_calls` không
   đổi; TCB giữ `(word, badge)` ở `TcbData.notif` (`tcb_notification`/`take_tcb_notification`).
9. **Reviewer-kt27 yêu cầu (đã sửa)**: nhánh `recv` trên Notification trước đây `return` sớm
   **không** gọi `reset_ipc_error` (nhánh Endpoint và `nb_recv` Notification đều gọi) ⇒ Wait thành
   công giữ `last_ipc_error`/`last_fault` cũ, trái hợp đồng KT-26. Đã thêm `reset_ipc_error(tcb)`
   trước `notification_wait` + test `recv_notification_xoa_last_ipc_error_cu` (Call thiếu Reply ⇒
   `Cancelled` còn trên TCB, Recv Notification xong ⇒ `None`).
10. **Nợ/rủi ro (không chặn, reviewer nêu)**: `block_current(BlockedOnNotification)` ghi waiter mà
    **không** xét `word != 0`; `check_notifications` **không** kiểm `waiter ⇒ word == 0`. Đây là
    đường **test-primitive** (không phải syscall), nên ghi nợ thay vì siết (siết sẽ phá ma trận
    11×11 KT-23); follow-up: thêm `debug_assert`/kiểm khi K3 có đường thật.
11. **Nợ/rủi ro (không chặn, reviewer nêu)**: `notification_signal` ghi word/waiter **trước**
    `transition_tcb(target, Ready)?`; hiện **không chạm được** vì mọi cặp `BlockedOn* → Ready` đều
    ALLOWED, nhưng nếu sau này bảng đổi thì có thể để word đã giao mà TCB chưa `Ready`. Follow-up
    K3: đảo thứ tự/kiểm trước khi ghi.

## 4. Cổng và Miri

| Hạng mục | Kết quả | Log |
|---|---|---|
| `gate --fast` | **PASS 413 test/55 suite** (baseline 386/53 ⇒ **+27** = 26 `kt27_notification` + 1 `kt27_scn`), L1 1.000 seed/999 trace **738 ms** (đo bởi cổng), fmt/clippy `-D warnings` sạch, R1..R5 OK, `scn: OK` 21 tệp/11 nhóm | [`kt27-gate-fast.log`](kt27-gate-fast.log) |
| Miri | `--test kt27_notification` **PASS 26/26** (fuzz `cfg(miri)` 3 seed × 8 op; stable 1.000 × 64) | [`kt27-miri.log`](kt27-miri.log) |
| `check` | PASS (fmt/clippy/gen fixtures/build) | [`kt27-check.log`](kt27-check.log) |

## 5. Tệp thay đổi

- `v2/sim/vifast-sim/src/kobj.rs` — `NotificationState {word, badge, waiter, bound_sc}`;
  `NotificationValue` + `notification_message`; `TcbData.notif`;
  `notification_signal`/`notification_wait`/`notification_poll`/`notification_word`/
  `notification_badge`/`notification_bound_sc`; `tcb_notification`/`take_tcb_notification`;
  `notification_signal_via_cap`; `bound_tcb_blocked_on_recv`; `deliver_notification`;
  `recv`/`nb_recv` nhánh Notification; `send`/`nb_send` Notification = Signal;
  `tcb_bind_notification` 1:1 + `tcb_unbind_notification`; `sched_context_bind` ghi `bound_sc`
  1:1 + `sched_bound_notification`; `block_current` ghi waiter; `check_notifications` +
  gọi trong `sched_after_op`; `drop_sched_binding_to_cap`; dọn `bound`/`bound_sc` trong
  `purge_dead_refs`.
- `v2/sim/vifast-sim/src/scenario.rs` — `Step::{Signal, WaitNotification, PollNotification}` +
  `name`/`actor`/`is_tcb_syscall`/`execute`.
- `v2/sim/vifast-sim/tests/kt27_notification.rs` (**mới, 26 test**), `tests/kt27_scn.rs`
  (**mới, 1 test**); `tests/kt24_ipc.rs` (1 kỳ vọng Notification cũ → Signal).
- `v2/docs/k1c-notification.md` (**mới**) — model-only `[TK]`, giới hạn, quy ước.
- `v2/bin/v2` — thêm `kt27_notification` vào `nightly --only-miri`.
- `v2/agent/STATE.md`, `v2/agent/JOURNAL/kt27-*.log`, `v2/agent/JOURNAL/kt27-journal.md`.

**Không** chạm: `spec/`, `vifast-model`, `.scn`, QUEUE, DECISIONS, AGENTS, `viscript/`; không
dependency/`unsafe`/`#[allow]`/`#[ignore]`; không đổi ABI.

## 6. Review độc lập (reviewer-kt27)

- Lead spawn `reviewer-kt27` (review-only, chưa commit). Reviewer tự chạy `cargo test --workspace`
  **412 test/55 suite** (trước bản vá), `kt27_notification` 25/25 + `kt27_scn` 1/1, xác nhận
  word64/badge OR, một waiter, bind 1:1, bound `BlockedOnRecv` model-only, scope sạch, Miri 25/25.
- **Verdict: APPROVE** kèm **1 yêu cầu sửa** (đã xử lý): `recv` trên Notification `return` sớm
  không gọi `reset_ipc_error` ⇒ thêm + test `recv_notification_xoa_last_ipc_error_cu`.
- 2 rủi ro **không chặn** ghi thành nợ ở §3 mục 10/11 (block_current không xét `word != 0`;
  signal ghi word trước `transition_tcb`).
- Sau sửa: `gate --fast` **PASS 413 test/55 suite**, Miri **26/26**.

## 7. Đề xuất

**DONE** (reviewer-kt27 APPROVE + 1 yêu cầu đã sửa; chờ Agent 0 lật QUEUE). YC mới: (a) K3 conformance chốt
ngữ nghĩa bound notification đánh thức `BlockedOnRecv` (model-only hiện tại); (b) K3 chốt
`IrqHandler.SetNotification` (vẫn `reserved`); (c) K3 xem lại `NBRecv` Notification trả
`Message`/`value` + badge theo đúng seL4.
