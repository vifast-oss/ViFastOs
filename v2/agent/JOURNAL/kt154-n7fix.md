# KT-154 — N7-2: mọi đường đánh thức TCB phải tôn trọng hart `home`

**Ngày**: 2026-11-18/19 · **Agent 2 (L7)** · nhánh `night/L7-n7fix` · HEAD trước task: `4e6dcea1`
**Nguồn nợ**: `v2/agent/JOURNAL/kt151-n7.md` §3.3 + §7 (**N7-2**), D2-131.

## 0. Việc được giao (nguyên văn, trích)

> **KT-154 — vá N7-2: mọi đường đánh thức TCB phải tôn trọng hart `home`**
> (1) **Chứng minh lỗi trước (đỏ-trước)** … TCB `home=1`, đang `BlockedOnSend`/`BlockedOnReply`
> trên hart 0; một đường **không phải** `wake()` … làm nó `Ready`; đòi nó **không** nằm trong
> ready queue của hart 0 và **có** message `Wake` trong hộp thư hart 1.
> (2) **Vá** … (3) **Xanh** … (4) **Đột biến** … (5) **Máy kiểm** `--strict` trước/sau.

## 1. Kết luận trước (đọc nhanh)

- **(1) ĐỎ-TRƯỚC ĐẠT**: `v2/sim/vifast-sim/tests/kt154_n7fix.rs` — 7 ca; trên mã **chưa vá**
  3 ca chéo hart **đỏ** đúng lý do (`left: 0 right: 1` — hộp thư hart 1 **rỗng**), 4 ca
  cùng-hart/một-hart xanh. Log: `kt154-logs/kt154-red-before.log`.
- **(2) VÁ**: 15/16 đường (16 = 15 đường thật + `wake_step` vốn đã đúng) đi qua **một** cửa
  `Machine::wake_from` (kernel). Xem §4.
- **(3) XANH**: 7/7 ca mới; `cargo test --workspace` + `vifast-plat-common` — xem §6.
- **(4) ĐỘT BIẾN**: bỏ vá ở `reply_with` ⇒ ca Reply **đỏ** lại; khôi phục ⇒ xanh. §7.
- **(5) `--strict`**: **4 → 2** chỗ `E_STRICT_TRANSITION` (kernel về **0**; 2 còn lại ở **sim**,
  ngoài phạm vi task — §8).

## 2. Đo lại nợ trước khi vá (Luật 1: lệnh + output nguyên văn)

```text
$ python3 v2/tools/check-wake-paths.py --strict
# kênh Wake: v2/kernel/vifast-core/src/core_sched.rs:224 (wake)
# kênh Wake: v2/sim/vifast-sim/src/kobj.rs:5718 (wake)
# kênh Migrate: v2/kernel/vifast-core/src/core_sched.rs:388 (push_step)
# kênh Migrate: v2/sim/vifast-sim/src/kobj.rs:6056 (push_step)
# kiểu thiết kế: `vifast-sync::Mailbox` (Msg::Wake/Reschedule/Migrate); kernel dùng `vifast-core::mailbox::MailboxMsg`; sim dùng `vifast-sim::mailbox::MailboxMsg`
# chuyển Ready: v2/kernel/vifast-core/src/core_sched.rs:240 (wake_step)
# chuyển Ready: v2/kernel/vifast-core/src/core_tcb.rs:592 (tcb_resume_with_regs)
# chuyển Ready: v2/kernel/vifast-core/src/core_tcb.rs:745 (process_refills)
# chuyển Ready: v2/sim/vifast-sim/src/kobj.rs:5308 (tcb_resume_with_regs)
# chuyển Ready: v2/sim/vifast-sim/src/kobj.rs:5737 (wake_step)
# chuyển Ready: v2/sim/vifast-sim/src/kobj.rs:8058 (process_refills)
E E_STRICT_TRANSITION 4 chỗ chuyển `TcbState::Ready` ngoài `wake_step` (nợ N7-2, không nằm trong cổng mặc định)
E E_STRICT_TRANSITION   v2/kernel/vifast-core/src/core_tcb.rs:592 (tcb_resume_with_regs)
E E_STRICT_TRANSITION   v2/kernel/vifast-core/src/core_tcb.rs:745 (process_refills)
E E_STRICT_TRANSITION   v2/sim/vifast-sim/src/kobj.rs:5308 (tcb_resume_with_regs)
E E_STRICT_TRANSITION   v2/sim/vifast-sim/src/kobj.rs:8058 (process_refills)
# check-wake-paths: mode=v2 strict=1 errors=5 warnings=0
```

(Lưu ý: `--strict` chỉ đếm hàm có **guard** `is_blocked()`/`Sleeping` trong thân — nên nó
đo **4** chỗ, không phải 16. Danh sách **đầy đủ** 16 đường là §3.)

## 3. Danh sách 16 đường (kernel) — nguồn: `kt151-n7.md` §3.3, đối chiếu lại bằng máy

16 dòng `transition_tcb(…, TcbState::Ready)` trong `v2/kernel` là **đánh thức thật**
(nguồn `Blocked*`/`Sleeping`), gồm cả `wake_step` (đã đúng):

| # | `file:dòng` | hàm | vá? |
|---|---|---|---|
| 1 | `core_sched.rs:240` | `wake_step` | **không** — đây **là** cửa đúng |
| 2 | `core_finalise.rs:39` | `cancel_reply_object` | có |
| 3 | `core_finalise.rs:99` | `finalise_endpoint_body` (Recv waiter) | có |
| 4 | `core_finalise.rs:106` | `finalise_endpoint_body` (sender) | có |
| 5 | `core_finalise.rs:124` | `finalise_notification_body` | có |
| 6 | `core_ipc.rs:333` | `deliver_queued` (fault, `blocking`) | có |
| 7 | `core_ipc.rs:347` | `deliver_queued` (thường, `blocking`) | có |
| 8 | `core_ipc.rs:417` | `call_msg` (waiter) | có |
| 9 | `core_ipc.rs:519` | `endpoint_send` (waiter) | có |
| 10 | `core_ipc.rs:697` | `reply_with` (caller) | có |
| 11 | `core_ipc.rs:803` | `exit` (Recv waiter) | có |
| 12 | `core_ipc.rs:888` | `inject_fault` (Recv waiter) | có |
| 13 | `core_ipc.rs:1105` | `cancel_reply_of` (donor) | có |
| 14 | `core_sync.rs:61` | `notification_signal` (waiter) | có |
| 15 | `core_tcb.rs:592` | `tcb_resume_with_regs` (blocked → Ready) | có |
| 16 | `core_tcb.rs:745` | `process_refills` (`Sleeping{Budget}`) | có |

**Không** tính (không phải đánh thức — §3.3 `kt151-n7.md`): `ensure_started`
(`Inactive→Ready`), `dispatch_step`/`yield_current` (`Running→Ready`), `transition_tcb`
(nguyên thuỷ), `tcb_suspend` (`Ready + suspended`), `build_rig`/`task_ready`/`turn`/
`boot_smoke` (khởi tạo test/sản phẩm).

## 4. (1) Đỏ-trước — lệnh + log nguyên văn

```text
$ cargo test --manifest-path v2/Cargo.toml -p vifast-sim --test kt154_n7fix
   Compiling vifast-sim v0.1.0 (/private/tmp/night-L7-n7fix/v2/sim/vifast-sim)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.58s
     Running tests/kt154_n7fix.rs (v2/target/debug/deps/kt154_n7fix-fa90bbcca031b59c)

running 7 tests
test kt154_core_mot_hart_khong_gui_wake ... ok
test kt154_core_cung_hart_khong_gui_wake ... ok
test kt154_core_notification_cheo_hart_phai_gui_wake ... FAILED
test kt154_core_recv_giao_message_cheo_hart_phai_gui_wake ... FAILED
test kt154_core_reply_cheo_hart_phai_gui_wake ... FAILED
test kt154_core_reply_cung_hart_khong_gui_wake ... ok
test kt154_core_wake_inactive_van_loi ... ok

failures:

---- kt154_core_notification_cheo_hart_phai_gui_wake stdout ----

thread 'kt154_core_notification_cheo_hart_phai_gui_wake' (77525672) panicked at sim/vifast-sim/tests/kt154_n7fix.rs:157:9:
assertion `left == right` failed: Signal chéo hart: hộp thư hart 1 phải có đúng một message `Wake`
  left: 0
 right: 1
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- kt154_core_recv_giao_message_cheo_hart_phai_gui_wake stdout ----

thread 'kt154_core_recv_giao_message_cheo_hart_phai_gui_wake' (77525673) panicked at sim/vifast-sim/tests/kt154_n7fix.rs:157:9:
assertion `left == right` failed: Recv giao message chéo hart: hộp thư hart 1 phải có đúng một message `Wake`
  left: 0
 right: 1

---- kt154_core_reply_cheo_hart_phai_gui_wake stdout ----

thread 'kt154_core_reply_cheo_hart_phai_gui_wake' (77525674) panicked at sim/vifast-sim/tests/kt154_n7fix.rs:157:9:
assertion `left == right` failed: Reply chéo hart: hộp thư hart 1 phải có đúng một message `Wake`
  left: 0
 right: 1

failures:
    kt154_core_notification_cheo_hart_phai_gui_wake
    kt154_core_recv_giao_message_cheo_hart_phai_gui_wake
    kt154_core_reply_cheo_hart_phai_gui_wake

test result: FAILED. 4 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

**Loại cây**: worktree `night/L7-n7fix` tại `4e6dcea1` + **chỉ** tệp test mới (chưa vá dòng
sản phẩm nào) ⇒ đỏ là đỏ trên **mã cũ**, không phải ảnh build (test host, không QEMU).

**Điều đáng ghi**: cả 3 ca đỏ vì hộp thư hart 1 **rỗng** (`left: 0`), **không** vì TCB nằm
sai hàng — `sched_sync` đã đặt TCB `Ready` vào hàng của `home` (đúng hart). Nghĩa là triệu
chứng thật của N7-2 **không** phải "sai hàng" mà là "**hart `home` không được đánh thức**":
TCB vào hàng hart 1 nhưng hart 1 **không** nhận `MailboxMsg::Wake` ⇒ trên SMP thật nó có
thể không bao giờ chạy. Ghi lại vì câu chữ nhiệm vụ nói "đưa vào ready queue của hart hiện
tại" — đo được thì **không** phải vậy (`sched_sync` dùng `info.home`).
