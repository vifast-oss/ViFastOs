<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# KT-25 — IPC core 2: Call/Reply/ReplyRecv/NBSendRecv/NBSendWait + 4 cap + vòng đời Reply

**Task**: K1C-PLAN §5 KT-25 · **D2-53 Q6** (thất bại chuyển cap = đếm **mềm**, không thêm bit
MessageInfo), **Q8** (Reply bị xoá ⇒ caller **`Cancelled`**), **Q15a** (`NBSendRecv` =
`NBSend` rồi `Recv`, NBSend lỗi **vẫn** Recv) · **D2-54** (runner `.scn`: `ep0`/`ep1` cùng một
Endpoint; `recv` không `reply=` ⇒ `rep0`).
**Vào**: READY · HEAD `47cea0d` ([KT-00] lật KT-24 DONE) · baseline `gate --fast` PASS
**336 test/49 suite**, L1 1.000 seed/999 trace 687 ms
([`kt25-baseline-gate-fast.log`](kt25-baseline-gate-fast.log)).
**Không** làm: Fault/Exit (KT-26), Notification word/badge/`Signal` (KT-27), WaitAddr (KT-28),
SMP/push (KT-29), `vifast-sync` (KT-30); không sửa `spec/`, `vifast-model`, `.scn`, QUEUE,
DECISIONS, AGENTS, `viscript/`; không dependency/`unsafe`/`#[allow]`/`#[ignore]`.

## 1. Đỏ trước (D2-05)

| Log | Nội dung |
|---|---|
| [`kt25-red-compile.log`](kt25-red-compile.log) | `tests/kt25_ipc2.rs` **không biên dịch**: **34 lỗi** = **31×`E0599`** (`Message::with_cap_ids`, `Machine::{tcb_set_recv_window, reply_recv, nb_send_recv, nb_send_wait, cap_transfer_stats, cap_transfer_failed, cap_transfer_installed, cap_unwrapped}`, `Step::{ReplyRecv, NBSendRecv, NBSendWait, SendCaps}`) + 3×`E0308` phụ thuộc API mới |
| [`kt25-red-semantics.log`](kt25-red-semantics.log) | probe **tạm (đã xoá)** dùng API KT-24: (1) `NBSend` message mang cap ⇒ `Err(NotSupported)` (KT-24 từ chối cap; KT-25 phải giao được); (2) `CNode.Delete` cap Reply khi caller `BlockedOnReply{reply:3}` ⇒ caller **vẫn `BlockedOnReply`** + `last_ipc_error = None` ⇒ **deadlock**, trái Q8 (`Cancelled`) |

`kt25-red-semantics.log` là đầu ra probe:

```
NBSend message có cap = Err(NotSupported) (kỳ vọng Ok, message vào đường IPC)
sau Delete Reply: caller state = BlockedOnReply { reply: 3 } (kỳ vọng Ready)
last_ipc_error = Ok(None) (kỳ vọng Some(Cancelled))
```

## 2. Sản phẩm

- `v2/sim/vifast-sim/src/ipc.rs` (+~120 dòng): `CapSnapshot` (ảnh chụp cap nguồn),
  `CapPlan { install, unwrap_badges, skipped }`, **`plan_cap_transfer`** — quyết định duy nhất
  theo vị trí: unwrap badge (cap tới chính endpoint + badge) / thiếu `RIGHT_GRANT` ⇒ mềm /
  ô đích có cap ⇒ mềm / còn lại cài; `MessageInfo::{with_caps, with_extra_caps_unwrapped}` (kiểm
  biên `caps≤4`, `extra≤0xF`); `Message::{with_cap_ids, caps, cap_count}`.
- `v2/sim/vifast-sim/src/kobj.rs`:
  - `TcbData.recv_window: Option<[SlotRef; 4]>` + model API `tcb_set_recv_window` (kiểm slot
    thật/không trùng) + `tcb_recv_window`;
  - `admit_message(msg, snaps)` + `resolve_caps` (cap chết ⇒ `InvalidCapability` **trước** khi
    cấp `msg_id`), `prepare_caps` (window chưa bind ⇒ `InvalidArgument` **trước** khi giao),
    `deliver_caps` (derive cap con **bỏ `RIGHT_GRANT`** giữ badge, cập nhật `MessageInfo` phía
    nhận + `badge` OR, xoá `Message.caps` đã cài), `deliver_message` (một cửa giao);
  - `QueuedMessage.snaps` (ảnh chụp cap đi theo message xếp hàng); `recv`/`nb_recv` **peek** hàng
    để kiểm window **trước** khi pop (message không mất khi lỗi cấu hình);
  - **`reply_recv`** (reply trước rồi recv, Reply mới cho `BlockedOnRecv`), **`nb_send_recv`**,
    **`nb_send_wait`** (Q15a; NBSend lỗi ⇒ vẫn Recv/Wait, trả `Ok(Some(err))`), `wait_notification_obj`
    (thân chung với `tcb_wait_notification`);
  - **vòng đời Reply**: `cancel_reply` (caller `Cancelled` + `Ready` qua một cửa
    `transition_tcb` ⇒ hook KT-21 trả donation), hook `delete`/`revoke_children`/Untyped-revoke,
    `purge_dead_refs` gỡ Reply cap chết khỏi Recv waiter;
  - bộ đếm `cap_transfer_installed/failed/unwrapped` + accessor (`cap_transfer_stats`);
  - `nb_recv` = **poll**: TCB không chạy được ⇒ `Busy` không đổi trạng thái (`.scn` 05).
- `v2/sim/vifast-sim/src/scenario.rs`: `Step::{ReplyRecv, NBSendRecv, NBSendWait, SendCaps}`
  (+`Msg::words`) đủ `name`/`actor`/`execute`/`is_tcb_syscall`.
- `v2/sim/vifast-sim/tests/kt25_ipc2.rs` (**mới, 24 test**), `tests/kt25_scn.rs` (**mới, 1 test**).
- `v2/docs/k1c-ipc-semantics.md`: §10 chuyển 4 cap/receive window, §11 vòng đời Reply, §12
  `ReplyRecv`/NB*, §13 runner `.scn`, §14 giới hạn còn lại.

## 3. Đối chiếu DoD K1C-PLAN §5 KT-25

| DoD | Test |
|---|---|
| 1 `ReplyRecv` giao reply trước, không message ⇒ `BlockedOnRecv` Reply **mới**, không mất reply/message | `reply_recv_giao_reply_truoc_roi_recv`, `reply_recv_khong_co_message_thi_blocked_on_recv` |
| 2 `NBSendRecv`/`NBSendWait` hai nhánh (nhận sẵn sàng / NBSend không giao được ⇒ **vẫn** Recv/Wait) | `nb_send_recv_nhanh_gui_duoc_va_nhanh_khong_gui_duoc`, `nb_send_wait_hai_nhanh` |
| 3 4 cap: cài đủ, mất `RIGHT_GRANT`, giữ badge; 5 cap ⇒ `InvalidArgument` trước khi giao | `bon_cap_cai_du_bo_grant_giu_badge`, `nam_cap_bi_tu_choi_truoc_khi_giao` |
| 4 thất bại **mềm** (thiếu GRANT / ô đích có cap) + lỗi **cứng** khi cap nguồn chết | `thieu_grant_that_bai_mem_message_van_giao`, `o_dich_da_co_cap_that_bai_mem`, `cap_nguon_chet_loi_cung_truoc_khi_giao`, `cap_da_nhan_khong_chuyen_lai_duoc_vi_mat_grant` |
| 5 unwrap badge: `extra_caps_unwrapped`, `Message.badge` OR, window không bị chiếm | `unwrap_badge_khong_chiem_receive_window`, `hai_badge_or_vao_message_badge` |
| 6 vòng đời Reply: `Delete`/`Revoke` ⇒ caller `Cancelled`, `Reply.caller == None`, `Send(rep0)` ⇒ `InvalidCapability`; `.scn` 08 ⇒ `Delete` sau `Revoke` = `SlotEmpty` | `reply_bi_xoa_caller_cancelled`, `reply_bi_revoke_caller_cancelled_va_delete_slot_empty`, `revoke_cap_con_reply_de_slot_empty` |
| 7 donation không rò (3 nhánh: Delete/Revoke/`Suspend(Abort)`) — `check_sc_refs` sạch | `donation_khong_ro_khi_reply_bi_xoa`, `donation_khong_ro_khi_revoke_reply`, `donation_khong_ro_khi_suspend_abort_caller` |
| 8 `Step::{ReplyRecv, NBSendRecv, NBSendWait, SendCaps}` + runner `.scn` 05/07/08/15/16 — 0 lệch | `scenario_step_reply_recv_nb_cap`, `runner_scn_05_07_08_15_16_khong_lech` (`kt25_scn`) |
| 9 bảo toàn message (`check_ipc`) + 1.000 seed × 64 op, cùng seed cùng trace | `fuzz_1000_seed_64_op_ipc2` (`check_sched`/`check_cap`/`check_mailboxes`/`check_ipc` sau **mỗi** bước) |
| 10 `gate --fast` PASS; Miri `--test kt25_ipc2` PASS; không đụng `spec/` | [`kt25-gate-fast.log`](kt25-gate-fast.log), [`kt25-miri.log`](kt25-miri.log) |

**Reviewer-kt25 verdict DRIFT (nhẹ) → đã sửa 3 điểm**:
1. Reply cap **chết** còn nằm trong Recv waiter sau `CNode.Delete` (chỉ `revoke` gọi
   `purge_dead_refs`) ⇒ `Call` sau đó trả **cứng** `InvalidCapability` thay vì Q7(b). Sửa:
   `clear_dead_reply_in_waiters()` (chỉ gỡ Reply cap chết khỏi waiter — **không** quét toàn bộ
   ref, giữ ngữ nghĩa KT-18 `Exit`+`fault_ep` chết ⇒ `InvalidCapability`) gọi cuối `delete` khi
   cap là `Reply`; test `delete_reply_dang_trong_recv_waiter_khong_lam_hong_call` + **mutation
   check** (bỏ helper ⇒ test đỏ `Err(InvalidCapability)` vs `Err(Cancelled)`, đã khôi phục).
2. Comment lỗi thời ở `admit_message` ("KT-25 chưa chuyển ⇒ `NotSupported`") — sửa đúng hành vi
   hiện tại (validate ≤4/khớp `MessageInfo.caps`, cap chết ⇒ `InvalidCapability`, mềm khi giao).
3. Số liệu cây cuối: gate **361 test/51 suite** (24 `kt25_ipc2` + 1 `kt25_scn`), Miri **24/24**. 

**Hồi quy tự soát**: `reply_with` lấy `Reply.caller` trước khi resolve cap của message reply ⇒ cap
chết/window chưa bind (lỗi cứng) làm **mất binding** trong khi caller vẫn `BlockedOnReply` (treo).
Đã sửa: **peek** caller → resolve/`prepare_caps` → chỉ `take` khi đã qua kiểm; test
`reply_loi_cung_khong_lam_mat_caller` (reply lỗi cứng ⇒ `InvalidCapability`, `reply_caller` vẫn
`Some(client)`, reply lại sau đó `Ok`).

## 4. Cổng + Miri (số đo do lệnh cổng sinh — QD §9 Q12)

- `./v2/bin/v2 gate --fast` — **PASS**, **361 test/51 suite** (KT-24: 336/49 ⇒ **+25** = 24
  `kt25_ipc2` + 1 `kt25_scn`), 0 failed, 1 doc-test ignored pre-existing; L1 **1.000 seed/999
  trace**, 708 ms (đo bởi cổng); gen-check ON, `scn: OK` 21 tệp/11 nhóm, R1..R5 OK, clippy
  `-D warnings` sạch. Log: [`kt25-gate-fast.log`](kt25-gate-fast.log).
- `./v2/bin/v2 miri test -p vifast-sim --test kt25_ipc2` — **PASS 24/24**, 75,99 s (2 warning
  pre-existing của `vifast-objmem`, không phải KT-25). Tệp `kt25_scn` **không** chạy Miri vì đọc
  tệp `.scn` (isolation chặn `open`) — cùng quy ước `kt24_scn.rs`/`kt24_ipc.rs`. Log:
  [`kt25-miri.log`](kt25-miri.log).

## 5. Quyết định mô hình cần reviewer soi (model-only `[TK]`, không đổi ABI)

1. **`NBRecv` = poll**: TCB đang không chạy được ⇒ `Busy` + **không** đổi trạng thái (thay vì
   `IllegalOperation` của KT-24). Cần cho `.scn` 05: client poll Endpoint khi đang `BlockedOnReply`
   sau `ReplyRecv` của server — poll không thể tạo chuyển trạng thái trái `ALLOWED`.
2. **Một cap Reply bị xoá = đường reply bị huỷ** (Q8): `Delete`/`Revoke` **một** cap Reply đang
   giữ caller ⇒ caller `Cancelled` (không chờ "cap cuối" — `.scn` 07/08 xoá cap, không revoke
   Untyped). Object Reply **không** finalise; idempotent.
3. **Window chưa bind + message có cap ⇒ `InvalidArgument`** (lỗi cấu hình) — ngoài hai lỗi cứng
   Q6 nêu (cap nguồn chết); message không bị giao nửa chừng.
4. Cap nguồn chết **giữa** lúc xếp hàng và lúc giao ⇒ **thất bại mềm** (không tạo cap mồ côi).
5. `Message.caps` của message **đã giao** bị xoá (cap đã nằm trong window bên nhận);
   `MessageInfo.caps` = số cap cài được (per seL4), `extra_caps_unwrapped` = số unwrap.
6. Cài cap **theo vị trí** (`caps[i] → window[i]`); một ô bận không chặn các ô sau.
