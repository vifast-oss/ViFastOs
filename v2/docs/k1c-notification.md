<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# Notification — KT-27 (model-only `[TK]`)

**Trạng thái:** REVIEW (Agent 2 dòng OS soạn cho Agent 0/reviewer độc lập). **Không phải ABI.**
Nguồn sự thật của ABI vẫn là `spec/abi.toml` (D2-11); tệp này chỉ ghi **quy ước mô hình** đã dùng
trong `v2/sim/vifast-sim` cho **KT-27** (Notification), để KT-28/29/30 không phải suy lại.

**Nguồn ngữ nghĩa:** thiết kế §4.7 `:328` (*word 64 bit; Signal OR badge, đánh thức **một** waiter,
giao word, **xoá word**; Wait/Poll; bound notification với TCB **và** SchedContext; **không** "đánh
thức tất cả" — khác v1 có chủ ý*), QD §3.11, **D2-53 Q6** (bind Notification bằng **model API**,
**không** thêm chữ ký TCB vào ABI; chuyển cap thất bại là **đếm mềm**), **D2-55** (không đổi ABI),
`spec/abi.toml:107-110` (Send/Recv/NBRecv trên Notification), K1C-PLAN §5 KT-27 (DoD 1..8).

## 1. Dữ liệu (model-only `[TK]`)

```rust
// v2/sim/vifast-sim/src/kobj.rs
pub struct NotificationValue { pub word: u64, pub badge: u64 }

struct NotificationState {
    word: u64,               // OR badge của mọi Signal chưa được Wait/Poll lấy
    badge: u64,              // OR badge song song — seL4: Wait trả word *là* badge ⇒ word == badge
    waiter: Option<ObjId>,   // **một** waiter (không broadcast)
    bound_sc: Option<ObjId>, // SchedContext.Bind(sc, notification) — 1:1
}

// TcbData
notif: Option<(u64, u64)>,   // (word, badge) lần Wait/Poll gần nhất đã giao cho TCB
bound_notification: Option<ObjId>, // đã có từ KT-18; KT-27 siết 1:1 + unbind
```

| Hạng mục | Giá trị | Nguồn |
|---|---|---|
| `word` | **64 bit**, giữ nguyên bit cao (`1 << 63`) | thiết kế `:328`, KT-27 DoD 1/2 |
| badge khi `Signal` | `word \|= badge`, `badge \|= badge` | thiết kế `:328`, KT-27 DoD 2 |
| badge **0** | **hợp lệ**: có waiter thì vẫn đánh thức (giao `(0,0)`) | K1C-PLAN §5 KT-27 rủi ro (b) |
| waiter | tối đa **một**; TCB thứ hai ⇒ `Busy` | thiết kế `:328` (khác v1), KT-27 DoD 3 |
| xoá word | **chỉ** khi giao cho waiter/Poll; không ai chờ ⇒ giữ | KT-27 DoD 2/7 (C2) |

## 2. Hành vi

### 2.1 `Signal` — `Machine::notification_signal(ntfn, badge)`

`Send`/`NBSend` trên **cap Notification** = `Signal` với **badge của cap** (seL4; cap gốc không
badge ⇒ `0`). Nội dung `Message` **không** dùng (spec `:107` chỉ nói `Send` trên Notification =
Signal).

1. `word |= badge`, `badge_acc |= badge`.
2. Chọn **đúng một** người được đánh thức:
   - `waiter` của notification (TCB đang `BlockedOnNotification`), nếu có;
   - nếu không: TCB **bound** với notification đang `BlockedOnRecv` (quét `ObjId` tăng dần ⇒ tất
     định) — nhánh **model-only** theo K1C-PLAN §5 KT-27 DoD 5 (xem §5 giới hạn);
   - không ai ⇒ **giữ** word/badge cho `Wait`/`Poll` sau (không mất tín hiệu).
3. Người được đánh thức: ghi `(word, badge)` vào `TcbData.notif`, gỡ khỏi **mọi** hàng chờ
   (`unlink_tcb_queues`: waiter notification hoặc `Recv` của Endpoint), `→ Ready`, rồi **xoá**
   `word`/`badge` trên notification.

### 2.2 `Wait` — `Recv` trên cap Notification / `Machine::notification_wait(tcb, ntfn)`

- `word != 0` ⇒ **trả ngay**: giao `(word, badge)` cho TCB (`tcb_notification`), xoá cả hai; TCB
  giữ `Ready`/`Running`.
- `word == 0` ⇒ TCB `BlockedOnNotification{ntfn}`, ghi `waiter`; đã có waiter ⇒ `Busy`.
- `Recv` kèm `reply_cptr` trên Notification ⇒ `InvalidArgument` (Reply cap vô nghĩa ở đây).

### 2.3 `Poll` — `NBRecv` trên cap Notification / `Machine::notification_poll(ntfn)`

- `word != 0` ⇒ trả `(word, badge)` và **xoá**; `NBRecv` trả `Message { MR0 = word, badge }`.
- `word == 0` ⇒ `Busy` (`spec/abi.toml:110` "Poll: Busy khi không có message").
- `NBRecv` khi TCB **không chạy được** ⇒ `Busy` (giữ nguyên KT-25/D2-55).

### 2.4 Bound notification

| Thao tác | Luật | Lỗi |
|---|---|---|
| `tcb_bind_notification(tcb, ntfn)` | TCB chưa bind **hoặc** bind lại cùng `ntfn` (no-op `Ok`) | TCB đã bind notification **khác** ⇒ `IllegalOperation` |
| | notification chưa bind TCB nào | notification đã bind TCB khác ⇒ `Busy` (**1:1**) |
| `tcb_unbind_notification(tcb)` | idempotent | — |
| TCB chết (`finalise_tcb`) | `bound_notification = None`; waiter của notification bị gỡ | — |
| `SchedContext.Bind(sc, notification)` | ghi `SchedData.bound = cap` **và** `Notification.bound_sc = sc` | notification đã bind **SC khác** ⇒ `IllegalOperation` |
| `SchedContext.Bind(sc, target_khác)` | gỡ `bound_sc` cũ trên notification (nếu có) | — |
| `CNode.Delete`/`Revoke` cap Notification | gỡ `SchedData.bound` trỏ cap đó + `bound_sc` tương ứng | — |

**Không** thêm `Tcb.BindNotification` vào `spec/` hay `v2/docs/abi-call-signatures.toml` (D2-53
Q6; spec `:143` chỉ có **tên** method 13/14, chưa có chữ ký). `Tcb.UnbindNotification` cũng chỉ là
model API.

## 3. Bất biến — `Machine::check_notifications`

Chạy sau **mỗi** public op (qua `sched_after_op`, cùng chỗ `check_sched`/`check_ipc`) và được test
gọi tường minh sau mỗi bước fuzz:

1. `waiter == Some(t)` ⇒ `t` là TCB sống đang `BlockedOnNotification{ntfn}`. Chiều ngược
   (`TcbState ⇒ waiter`) **không** kiểm: `transition_tcb` là cổng thấp được phép dựng trạng thái
   cho ma trận 11×11 (KT-23) mà không cần object thật; mọi đường IPC thật đều ghi waiter.
2. `badge == word` (hai trường cùng OR, cùng xoá — bắt lệch "xoá một bên").
3. `bound_sc == Some(sc)` ⇔ `sc` sống, là `SchedContext`, và `SchedData.bound` của nó là cap
   Notification **còn sống** trỏ đúng notification này (1:1 **hai chiều**).
4. `Tcb.bound_notification` trỏ Notification sống; mỗi notification bind **một** TCB.

`check_sc_refs` (KT-21/23) vẫn kiểm `SchedData.bound` còn sống đúng kiểu; KT-27 bổ sung dọn
`bound = None` khi cap chết (Delete/Revoke/Untyped) để bất biến đó không vỡ.

## 4. Không mất đánh thức (C2)

| Thứ tự | Kết quả |
|---|---|
| `Signal` **trước** `Wait` (không ai chờ) | word/badge còn; `Wait` sau thấy và trả ngay |
| `Wait` **trước** `Signal` | waiter được đánh thức đúng một lần, word giao rồi xoá |
| `Signal` badge 0 có waiter | vẫn đánh thức (giao `(0,0)`), không dùng 0 làm "chưa có" |
| `Signal` khi TCB bound đang `BlockedOnRecv` | TCB `Ready`, gỡ khỏi hàng `Recv` của Endpoint |
| TCB chết khi đang chờ | waiter gỡ, binding gỡ; `Signal` sau để word lại (không treo) |

## 5. Giới hạn / khác seL4 (ghi để K3 conformance kiểm)

1. **Badge lấy từ cap**: mô hình dùng `Cap.badge` (`mint`) làm badge của `Signal`; cap gốc ⇒ 0.
   `word == badge` theo đúng seL4 (`Wait` trả word làm badge) nhưng mô hình giữ **hai trường** để
   ABI `Recv` trả `value` + badge và để bất biến (2) bắt lệch.
2. **Bound notification đánh thức TCB `BlockedOnRecv`** là quy ước **model-only** theo K1C-PLAN §5
   KT-27 DoD 5. Trong seL4, notification bound chỉ tác động khi TCB chờ notification/ở trạng thái
   chạy; việc "signal đánh thức thread đang Recv Endpoint" **cần K3 conformance xác nhận** (nếu
   khác, sửa mô hình ở K3, không đổi ABI).
3. **Không broadcast**: nhiều bên chờ ⇒ mỗi bên một notification riêng (việc user space — thiết kế
   `:328`).
4. **IrqHandler.SetNotification** vẫn `reserved` (`spec/abi.toml:154`); KT-27 không cài.
5. Không timeout trên `Wait` (K6), không `NBSendWait` timeout — giữ nguyên KT-25.

## 6. Bằng chứng (KT-27)

| Hạng mục | Ở đâu |
|---|---|
| Test chính (word 64/OR/badge/1 waiter/xoá/Poll/bind 1:1/unbind/TCB chết/reset lỗi IPC) | `v2/sim/vifast-sim/tests/kt27_notification.rs` (**26 test**) |
| `.scn` 21 (đọc tệp, không sửa) | `v2/sim/vifast-sim/tests/kt27_scn.rs` (**1 test**) |
| Fuzz 1.000 seed × 64 op + cùng seed cùng trace | `kt27_notification::fuzz_1000_seed_64_op_bat_bien_va_tat_dinh` (`cfg(miri)` = 3 seed × 8 op) |
| Test đỏ trước | `v2/agent/JOURNAL/kt27-red-compile.log` (44 lỗi: 1×E0432 + 43×E0599), `kt27-red-semantics.log` |
| Cổng + Miri | `v2/agent/JOURNAL/kt27-gate-fast.log`, `kt27-miri.log` |
