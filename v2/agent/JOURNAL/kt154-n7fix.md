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

## 5. (2) Vá — **một** cửa `wake_from`, 15/16 đường đi qua

### 5.1 Thiết kế

Thêm **một** cửa trong `v2/kernel/vifast-core/src/core_sched.rs`:

```rust
pub(crate) fn wake_from(&mut self, tcb: ObjId) -> Result<(), KError> {
    let info = self.tcb_data(tcb)?.snapshot();
    let cheo_hart = self.n_harts > 1 && info.home != self.current_hart;
    self.wake_step(tcb, false)?;
    if cheo_hart {
        self.push_mail(info.home, MailboxMsg::Wake(tcb))?;
    }
    Ok(())
}
```

`wake_step` (vẫn là **chỗ duy nhất** chuyển `blocked/sleeping → Ready`) nhận thêm cờ
`unlink`:

```rust
pub(crate) fn wake_step(&mut self, tcb: ObjId, unlink: bool) -> Result<(), KError> {
    let state = self.tcb_state(tcb)?;
    if state == TcbState::Ready { return Ok(()); }
    if !(state.is_blocked() || matches!(state, TcbState::Sleeping { .. })) {
        return Err(KError::IllegalOperation);
    }
    if unlink { self.unlink_tcb_queues(tcb); }
    self.transition_tcb(tcb, TcbState::Ready)
}
```

**Vì sao `unlink = false` ở đường sản phẩm** (không phải lựa chọn thẩm mỹ — đo được):

1. Mỗi chỗ gọi **đã tự gỡ đúng phần của mình** trước khi đánh thức: `endpoint_take_waiter`/
   `endpoint_pop` (waiter/sender), `reply_data_mut(...).caller.take()` (caller),
   `notification.waiter = None`, `tcb_resume_with_regs` đã `unlink_tcb_queues`. Gọi
   `unlink_tcb_queues` lần nữa là **thừa**.
2. `unlink_tcb_queues` gỡ **mọi** message của TCB khỏi **mọi** Endpoint — kể cả message
   `NBSend` **không liên quan** đang xếp hàng trên Endpoint khác ⇒ **mất message** (đổi ngữ
   nghĩa, trái yêu cầu "giữ nguyên ngữ nghĩa khi `home == current_hart`").
3. `unlink_tcb_queues` cộng `ipc_dropped` cho message bị gỡ. `finalise_endpoint_body` **đã**
   cộng `ipc_dropped` cho message của sender trước đó ⇒ gọi thêm sẽ **đếm hai lần** và
   `check_ipc` (`ipc_sent == ipc_delivered + pending + ipc_dropped`) sẽ đỏ.

**Vì sao vẫn `wake_step` cục bộ khi khác hart** (không hoãn hẳn sang hart `home`): 4 đường
`cancel_reply_object` / `finalise_endpoint_body` / `finalise_notification_body` /
`tcb_resume_with_regs` **không thể** hoãn — object đã bị finalise (không còn để chờ) hoặc
`Resume` phải có hiệu lực ngay; hoãn sẽ vi phạm bất biến C2 (`Reply.caller ⇔ BlockedOnReply`)
và `check_ipc`. Vì vậy cửa này **vừa** đánh thức hart `home` (message `Wake`) **vừa** chuyển
`Ready` cục bộ (nhất quán ngay). `wake()` công khai **giữ nguyên** ngữ nghĩa cũ (khác hart ⇒
**chỉ** post, không đổi trạng thái) — test KT-151 `kt151_core_wake_cheo_hart_...` vẫn xanh.

### 5.2 15 chỗ đã vá (diff nguyên văn ở `git show`)

| # | `file:dòng` (trước) | hàm | thay bằng |
|---|---|---|---|
| 2 | `core_finalise.rs:39` | `cancel_reply_object` | `let _ = self.wake_from(c);` |
| 3 | `core_finalise.rs:99` | `finalise_endpoint_body` (Recv) | `let _ = self.wake_from(w);` |
| 4 | `core_finalise.rs:106` | `finalise_endpoint_body` (sender) | `let _ = self.wake_from(s);` |
| 5 | `core_finalise.rs:124` | `finalise_notification_body` | `let _ = self.wake_from(w);` |
| 6 | `core_ipc.rs:333` | `deliver_queued` (fault) | `self.wake_from(q.sender)?;` |
| 7 | `core_ipc.rs:347` | `deliver_queued` | `self.wake_from(q.sender)?;` |
| 8 | `core_ipc.rs:417` | `call_msg` | `self.wake_from(waiter)?;` |
| 9 | `core_ipc.rs:519` | `endpoint_send` | `self.wake_from(waiter)?;` |
| 10 | `core_ipc.rs:697` | `reply_with` | `self.wake_from(caller)?;` |
| 11 | `core_ipc.rs:803` | `exit` | `self.wake_from(w)?;` |
| 12 | `core_ipc.rs:888` | `inject_fault` | `self.wake_from(w)?;` |
| 13 | `core_ipc.rs:1105` | `cancel_reply_of` | `self.wake_from(donor)?;` |
| 14 | `core_sync.rs:61` | `notification_signal` | `self.wake_from(target)?;` |
| 15 | `core_tcb.rs:592` | `tcb_resume_with_regs` | `_ => self.wake_from(obj)?,` |
| 16 | `core_tcb.rs:745` | `process_refills` | `self.wake_from(tcb)?;` |

**15/16** — chỗ còn lại là `core_sched.rs:240` `wake_step`, tức **chính cửa** (không phải
đường cần vá). Kiểm lại bằng máy sau khi vá:

```text
$ grep -rn "transition_tcb(.*TcbState::Ready" v2/kernel/vifast-core/src/
v2/kernel/vifast-core/src/core_ipc.rs:192:            self.transition_tcb(tcb, TcbState::Ready)?;
v2/kernel/vifast-core/src/core_ipc.rs:700:            self.transition_tcb(tcb, TcbState::Ready)?;
v2/kernel/vifast-core/src/core_sched.rs:171:            self.transition_tcb(cur, TcbState::Ready)?;
v2/kernel/vifast-core/src/core_sched.rs:268:        self.transition_tcb(tcb, TcbState::Ready)
v2/kernel/vifast-core/src/core_sched.rs:279:        self.transition_tcb(cur, TcbState::Ready)?;
v2/kernel/vifast-core/src/tls_tests.rs:299:    r.m.transition_tcb(r.tcb, TcbState::Ready).expect("Ready");
v2/kernel/vifast-core/src/core_tcb.rs:436:                    self.transition_tcb(cur, TcbState::Ready)?;
v2/kernel/vifast-core/src/core_tcb.rs:508:            self.transition_tcb(obj, TcbState::Ready)?;
```

Từng dòng còn lại **không phải** đánh thức (đúng phân loại §3.3 `kt151-n7.md`):
`core_ipc.rs:192` `ensure_started` (`Inactive→Ready`); `core_ipc.rs:700` `reply_with`
(`Running→Ready` của **chính** replier); `core_sched.rs:171` `dispatch_step` và `:279`
`yield_current` (`Running→Ready`); `core_sched.rs:268` **`wake_step`** (cửa);
`core_tcb.rs:436` `transition_tcb` (nguyên thuỷ, `Running→Ready` khi preempt);
`core_tcb.rs:508` `tcb_suspend` (Suspend, không phải wake); `tls_tests.rs:299` mã test.

### 5.3 Đổi định nghĩa máy kiểm (Luật 2) — quét lại **toàn bộ**

`v2/tools/check-wake-paths.py` mức mặc định đòi "`MailboxMsg::Wake` chỉ phát từ `wake`".
Sau KT-154 có **hai cửa cùng một kênh**: `wake` (API công khai, đường hộp thư cũ) và
`wake_from` (cửa sản phẩm). Đã mở `WAKE_PRODUCER = ("wake", "wake_from")` — **không** thêm
cửa thứ ba; `Migrate` vẫn chỉ `push_step`. Đã **quét lại toàn bộ** `v2/kernel` + `v2/sim`
(không allowlist tệp) — kết quả §8. `--selftest` 7/7 vẫn xanh.

## 6. (3) Xanh — lệnh + log nguyên văn

```text
$ cargo test --manifest-path v2/Cargo.toml -p vifast-sim --test kt154_n7fix
   Compiling vifast-core v0.1.0 (/private/tmp/night-L7-n7fix/v2/kernel/vifast-core)
   Compiling vifast-sim v0.1.0 (/private/tmp/night-L7-n7fix/v2/sim/vifast-sim)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2.49s
     Running tests/kt154_n7fix.rs (v2/target/debug/deps/kt154_n7fix-fa90bbcca031b59c)

running 7 tests
test kt154_core_wake_inactive_van_loi ... ok
test kt154_core_mot_hart_khong_gui_wake ... ok
test kt154_core_reply_cung_hart_khong_gui_wake ... ok
test kt154_core_reply_cheo_hart_phai_gui_wake ... ok
test kt154_core_cung_hart_khong_gui_wake ... ok
test kt154_core_recv_giao_message_cheo_hart_phai_gui_wake ... ok
test kt154_core_notification_cheo_hart_phai_gui_wake ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Toàn bộ workspace (log đầy đủ `kt154-logs/kt154-workspace.log`):

```text
$ cargo test --manifest-path v2/Cargo.toml --workspace
EXIT=0
$ grep -c "test result: ok" v2/agent/JOURNAL/kt154-logs/kt154-workspace.log
105
$ grep -n "FAILED\|failures:\|error\[" v2/agent/JOURNAL/kt154-logs/kt154-workspace.log
(không có dòng nào)
```

`vifast-plat-common` (log `kt154-logs/kt154-plat-common.log`):

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common
EXIT=0
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
```

**Không** test cũ nào đỏ ⇒ không phải dừng theo ràng buộc "quá 3 test cũ đỏ".

## 8. (5) Máy kiểm `--strict` trước/sau

**Trước** (cây trước vá = `git archive HEAD` tại commit `419db2dd`, **không phải ảnh**):

```text
$ git archive HEAD | tar -x -C /tmp/kt154-before
$ python3 v2/tools/check-wake-paths.py --root /tmp/kt154-before --strict
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

**Sau** (cây đã vá):

```text
$ python3 v2/tools/check-wake-paths.py --strict
# kênh Wake: v2/kernel/vifast-core/src/core_sched.rs:224 (wake)
# kênh Wake: v2/kernel/vifast-core/src/core_sched.rs:248 (wake_from)
# kênh Wake: v2/sim/vifast-sim/src/kobj.rs:5718 (wake)
# kênh Migrate: v2/kernel/vifast-core/src/core_sched.rs:416 (push_step)
# kênh Migrate: v2/sim/vifast-sim/src/kobj.rs:6056 (push_step)
# kiểu thiết kế: `vifast-sync::Mailbox` (Msg::Wake/Reschedule/Migrate); kernel dùng `vifast-core::mailbox::MailboxMsg`; sim dùng `vifast-sim::mailbox::MailboxMsg`
# chuyển Ready: v2/kernel/vifast-core/src/core_sched.rs:268 (wake_step)
# chuyển Ready: v2/sim/vifast-sim/src/kobj.rs:5308 (tcb_resume_with_regs)
# chuyển Ready: v2/sim/vifast-sim/src/kobj.rs:5737 (wake_step)
# chuyển Ready: v2/sim/vifast-sim/src/kobj.rs:8058 (process_refills)
E E_STRICT_TRANSITION 2 chỗ chuyển `TcbState::Ready` ngoài `wake_step` (nợ N7-2, không nằm trong cổng mặc định)
E E_STRICT_TRANSITION   v2/sim/vifast-sim/src/kobj.rs:5308 (tcb_resume_with_regs)
E E_STRICT_TRANSITION   v2/sim/vifast-sim/src/kobj.rs:8058 (process_refills)
# check-wake-paths: mode=v2 strict=1 errors=3 warnings=0
```

**4 → 2**; **kernel về 0**. Hai chỗ còn lại **đều ở `v2/sim`** (`tcb_resume_with_regs`,
`process_refills`) — **ngoài phạm vi** task (task chỉ định "16 đường trong `v2/kernel`").
Sim là **oracle**; vá sim là việc của task riêng (cùng hình dạng: thêm `wake_from` cho sim
+ 15 chỗ). Ghi nợ **N7-2b** ở §11.

Mức **mặc định** (nằm trong `v2 check`) sau khi vá: `errors=0` (§9).
