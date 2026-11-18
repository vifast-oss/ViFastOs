# KT-155 — đóng nốt N7: sim 0 chỗ `--strict`, máy kiểm allowlist, `--strict` vào cổng, N7-4

**Ngày**: 2026-11-18/19 · **Agent 2 (L8)** · nhánh `night/L8-n7sim` · HEAD trước task: `5dfe542c`
(tách từ `night/L7-n7fix`, thừa hưởng toàn bộ vá KT-154).
**Nguồn nợ**: `v2/agent/JOURNAL/kt154-n7fix.md` §11 (N7-2b, N7-4, "máy kiểm chỉ phủ 4/16") +
`v2/agent/JOURNAL/kt151-n7.md` §3.3.

## 0. Việc được giao (nguyên văn, trích)

> **(1) Hai chỗ `--strict` còn lại ở `v2/sim`** — `kobj.rs` quanh `tcb_resume_with_regs` và
> `process_refills`. Vá **đối xứng** với kernel … Sau đó `python3 v2/tools/check-wake-paths.py
> --strict` phải **0 chỗ**.
> **(2) Máy kiểm phải BẮT được đột biến** … Thay heuristic bằng **allowlist tường minh** …
> fail-closed … Kiểm bằng **đột biến thật**: thêm một đường wake thứ hai đi vòng qua mailbox ở
> `reply_with` ⇒ máy kiểm **phải đỏ**.
> **(3) Đưa `--strict` vào cổng** … thêm `run_step` gọi `check-wake-paths.py --strict` vào
> `do_check` trong `v2/bin/v2`.
> **(4) `wake_addr` chéo hart** … Làm biến thể `wake_from_unlink` + test riêng. Nếu quá tốn
> thời gian, **bỏ qua** và ghi nợ — (1)(2)(3) quan trọng hơn.

## 1. Kết luận trước (đọc nhanh)

- **(1) ĐẠT — vượt yêu cầu**: sim vá **13** đường (không chỉ 2) qua `Machine::wake_from` (sim)
  + `wake_step(tcb, unlink)`; `--strict` **2 → 0** chỗ. Lý do vá 13: allowlist ở (2) đòi **mọi**
  `transition_tcb(…, Ready)` ngoài cửa wake phải có tên trong allowlist; 11 đường IPC/Reply/
  Notification/finalise của sim là **đường wake thật** nên không thể nằm trong allowlist
  "không-wake" — phải vá. Đây cũng là nợ **N7-2b** (§11 KT-154).
- **(2) ĐẠT**: `--strict` thay heuristic bằng **allowlist tường minh** (17 chỗ không-wake có lý
  do + 6 cửa wake), fail-closed **hai chiều**; đột biến ở `reply_with` (kernel **và** sim) ⇒
  máy kiểm **ĐỎ** (`E_STRICT_TRANSITION`), khôi phục ⇒ xanh. `--selftest` **11/11**.
- **(3) ĐẠT**: `run_step "wake paths --strict (KT-155: allowlist)"` trong `do_check`; log
  `kt155-check-final3.log` có `--- wake paths --strict (KT-155: allowlist): OK`.
- **(4) ĐẠT**: `wake_from_unlink` (kernel + sim) cho `wake_addr`; test đỏ-trước 2 tệp
  (`kt155_n7addr.rs` kernel, `kt155_n7sim.rs` ca 5 sim).
- **Ba cổng**: `v2 check` **PASS**; `cargo test --workspace` **EXIT=0** (107 dòng
  `test result: ok`, 0 FAILED); `vifast-plat-common` **EXIT=0**.
- **Test cũ**: **không** đỏ (workspace 107/107 tệp ok; KT-154 7/7 vẫn xanh).
- **T-INFRA**: `v2 check` có lúc đỏ bước `self-test luật` vì **bash 3.2 hỏng heap** (SIGTRAP,
  `BUG IN CLIENT OF LIBMALLOC`) — **tái hiện trên cây base** (stash hết thay đổi), không phải
  mã của task; đã cách ly bằng subshell (§8).

## 2. (1) Vá sim — 13 đường qua `wake_from`, `--strict` 2 → 0

### 2.1 Thiết kế (đối xứng kernel KT-154)

`v2/sim/vifast-sim/src/kobj.rs`:

```rust
fn wake_from(&mut self, tcb: ObjId) -> Result<(), KError> {
    let info = self.tcb_info_obj(tcb)?;
    let cheo_hart = self.sched.len() > 1 && info.home != self.current_hart;
    self.wake_step(tcb, false)?;
    if cheo_hart {
        self.push_mail(info.home, MailboxMsg::Wake(tcb))?;
    }
    Ok(())
}

fn wake_step(&mut self, tcb: ObjId, unlink: bool) -> Result<(), KError> {
    // … kiểm trạng thái như cũ …
    if unlink { self.unlink_tcb_queues(tcb); }
    self.transition_tcb(tcb, TcbState::Ready)
}
```

`wake` (công khai) và `apply_mail` giữ `unlink = true`; `wake_addr` dùng
`wake_from_unlink` (§6). 13 đường sản phẩm dùng `wake_from` (`unlink = false` — mỗi đường đã
tự gỡ bookkeeping của mình, y hệt lý do kernel KT-154 §5.1):

| # | `kobj.rs` (trước) | hàm | thay bằng |
|---|---|---|---|
| 1 | `5308` | `tcb_resume_with_regs` | `_ => self.wake_from(obj)?,` |
| 2 | `8058` | `process_refills` | `self.wake_from(tcb)?;` |
| 3 | `8351` | `cancel_reply_of` | `self.wake_from(donor)?;` |
| 4 | `8372` | `cancel_reply` | `self.wake_from(c)?;` |
| 5 | `8815` | `notification_signal` | `self.wake_from(target)?;` |
| 6 | `9286` | `deliver_queued` (fault Cancelled) | `self.wake_from(q.sender)?;` |
| 7 | `9301` | `deliver_queued` (thường) | `self.wake_from(q.sender)?;` |
| 8 | `9371` | `call_msg` (waiter) | `self.wake_from(waiter)?;` |
| 9 | `9507` | `endpoint_send` (waiter) | `self.wake_from(waiter)?;` |
| 10 | `9730` | `reply_with` (caller) | `self.wake_from(caller)?;` |
| 11 | `9910` | `exit` (Recv waiter) | `self.wake_from(w)?;` |
| 12 | `10035` | `inject_fault` (waiter) | `self.wake_from(w)?;` |
| 13 | `10788` | `wake_cancelled` (finalise) | `let _ = self.wake_from(tcb);` |

### 2.2 Đỏ-trước (cây **chưa** vá sim) — lệnh + log nguyên văn

```text
$ git stash push -- v2/sim/vifast-sim/src/kobj.rs
$ cargo test --manifest-path v2/Cargo.toml -p vifast-sim --test kt155_n7sim
test kt155_sim_cung_hart_khong_gui_wake ... ok
test kt155_sim_reply_cheo_hart_phai_gui_wake ... FAILED
test kt155_sim_refill_cheo_hart_phai_gui_wake ... FAILED
test kt155_sim_resume_cheo_hart_phai_gui_wake ... FAILED
  left: 0
 right: 1
  left: 0
 right: 1
  left: 0
 right: 1
test result: FAILED. 1 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
EXIT=101
```

(Log đầy đủ `kt155-logs/kt155-sim-red-before.log`.) **Loại cây**: worktree `night/L8-n7sim`
tại `5dfe542c` + chỉ tệp test mới (chưa vá dòng sản phẩm nào của sim) ⇒ đỏ là đỏ trên mã cũ.

### 2.3 Xanh-sau — lệnh + log nguyên văn

```text
$ git stash pop
$ cargo test --manifest-path v2/Cargo.toml -p vifast-sim --test kt155_n7sim
test kt155_sim_cung_hart_khong_gui_wake ... ok
test kt155_sim_refill_cheo_hart_phai_gui_wake ... ok
test kt155_sim_resume_cheo_hart_phai_gui_wake ... ok
test kt155_sim_reply_cheo_hart_phai_gui_wake ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
EXIT=0
```

(Log `kt155-logs/kt155-sim-green-after.log`.)

### 2.4 `--strict` trước/sau

**Trước** — cây trước task (`git archive 5dfe542c`, **không phải ảnh**), máy kiểm cũ:

```text
$ git archive 5dfe542c | tar -x -C /tmp/kt155-before
$ git show 5dfe542c:v2/tools/check-wake-paths.py > /tmp/kt155-before-checker.py
$ python3 /tmp/kt155-before-checker.py --root /tmp/kt155-before --strict
# chuyển Ready: v2/sim/vifast-sim/src/kobj.rs:5308 (tcb_resume_with_regs)
# chuyển Ready: v2/sim/vifast-sim/src/kobj.rs:5737 (wake_step)
# chuyển Ready: v2/sim/vifast-sim/src/kobj.rs:8058 (process_refills)
E E_STRICT_TRANSITION 2 chỗ chuyển `TcbState::Ready` ngoài `wake_step` (nợ N7-2, không nằm trong cổng mặc định)
E E_STRICT_TRANSITION   v2/sim/vifast-sim/src/kobj.rs:5308 (tcb_resume_with_regs)
E E_STRICT_TRANSITION   v2/sim/vifast-sim/src/kobj.rs:8058 (process_refills)
# check-wake-paths: mode=v2 strict=1 errors=3 warnings=0
EXIT=1
```

**Sau** — cây đã vá, máy kiểm mới (allowlist):

```text
$ python3 v2/tools/check-wake-paths.py --strict
# kênh Wake: v2/kernel/vifast-core/src/core_sched.rs:224 (wake)
# kênh Wake: v2/kernel/vifast-core/src/core_sched.rs:248 (wake_from)
# kênh Wake: v2/kernel/vifast-core/src/core_sched.rs:263 (wake_from_unlink)
# kênh Wake: v2/sim/vifast-sim/src/kobj.rs:5718 (wake)
# kênh Wake: v2/sim/vifast-sim/src/kobj.rs:5740 (wake_from)
# kênh Wake: v2/sim/vifast-sim/src/kobj.rs:5754 (wake_from_unlink)
# kênh Migrate: v2/kernel/vifast-core/src/core_sched.rs:431 (push_step)
# kênh Migrate: v2/sim/vifast-sim/src/kobj.rs:6096 (push_step)
# kiểu thiết kế: `vifast-sync::Mailbox` (Msg::Wake/Reschedule/Migrate); kernel dùng `vifast-core::mailbox::MailboxMsg`; sim dùng `vifast-sim::mailbox::MailboxMsg`
# chuyển Ready không-wake: v2/kernel/vifast-core/src/core_ipc.rs:192 (ensure_started) — Inactive→Ready (khởi tạo TCB)
# chuyển Ready không-wake: v2/kernel/vifast-core/src/core_ipc.rs:700 (reply_with) — Running→Ready của chính replier
# chuyển Ready không-wake: v2/kernel/vifast-core/src/core_sched.rs:171 (dispatch_step) — Running→Ready (dispatch)
# cửa wake: v2/kernel/vifast-core/src/core_sched.rs:283 (wake_step)
# chuyển Ready không-wake: v2/kernel/vifast-core/src/core_sched.rs:294 (yield_current) — Running→Ready (yield)
# chuyển Ready không-wake: v2/kernel/vifast-core/src/core_tcb.rs:436 (transition_tcb) — nguyên thuỷ: preempt Running→Ready
# chuyển Ready không-wake: v2/kernel/vifast-core/src/core_tcb.rs:508 (tcb_suspend) — treo: Ready + suspended (không chạy)
# chuyển Ready không-wake: v2/kernel/vifast-kbin-riscv64/src/proc_demo.rs:209 (build_rig) — khởi tạo sản phẩm
# chuyển Ready không-wake: v2/kernel/vifast-kbin-riscv64/src/proc_demo.rs:383 (task_ready) — khởi tạo sản phẩm
# chuyển Ready không-wake: v2/kernel/vifast-kbin-riscv64/src/proc_rt.rs:200 (build_rig) — khởi tạo sản phẩm
# chuyển Ready không-wake: v2/kernel/vifast-kbin-riscv64/src/proc_rt.rs:325 (turn) — khởi tạo sản phẩm
# chuyển Ready không-wake: v2/kernel/vifast-plat-riscv64/src/core_bridge.rs:270 (boot_smoke) — khởi tạo sản phẩm
# chuyển Ready không-wake: v2/sim/vifast-sim/src/kobj.rs:5132 (transition_tcb) — nguyên thuỷ: preempt Running→Ready
# chuyển Ready không-wake: v2/sim/vifast-sim/src/kobj.rs:5218 (tcb_suspend) — treo: Ready + suspended (không chạy)
# chuyển Ready không-wake: v2/sim/vifast-sim/src/kobj.rs:5640 (dispatch_step) — Running→Ready (dispatch)
# cửa wake: v2/sim/vifast-sim/src/kobj.rs:5777 (wake_step)
# chuyển Ready không-wake: v2/sim/vifast-sim/src/kobj.rs:5790 (yield_current) — Running→Ready (yield)
# chuyển Ready không-wake: v2/sim/vifast-sim/src/kobj.rs:9774 (reply_with) — Running→Ready của chính replier
# chuyển Ready không-wake: v2/sim/vifast-sim/src/kobj.rs:10351 (ensure_started) — Inactive→Ready (khởi tạo TCB)
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0
EXIT=0
```

**2 → 0**; kernel vẫn 0; sim về 0. Log `kt155-logs/kt155-strict-before.log` /
`kt155-strict-after.log`.

## 3. (2) Máy kiểm: allowlist tường minh thay heuristic

### 3.1 Vì sao (phát hiện của reviewer L6)

Heuristic cũ chỉ đếm hàm có `is_blocked()`/`Sleeping` trong thân ⇒ chỉ phủ **4/16** đường
(KT-154 §7). Đột biến ở `reply_with` (bỏ `wake_from`, gọi `transition_tcb` trực tiếp) **không**
bị bắt vì thân hàm không có hai chuỗi đó. Nay:

- `WAKE_DOORS` — **chỉ** `wake_step`/`wake_from`/`wake_from_unlink` (khoá theo `(tệp, hàm)`).
- `NON_WAKE_READY_SITES` — **17** chỗ `transition_tcb(…, Ready)` không phải wake, khoá
  `(tệp, hàm, thứ tự trong hàm)` + lý do: `ensure_started` (Inactive→Ready), `reply_with`
  (Running→Ready của replier), `dispatch_step`, `yield_current`, `transition_tcb` (nguyên thuỷ),
  `tcb_suspend` (Ready+suspended), `build_rig`/`task_ready`/`turn`/`boot_smoke` (khởi tạo sản
  phẩm) — 11 kernel + 6 sim.
- **Fail-closed hai chiều**: chỗ trong mã không có trong allowlist ⇒ `E_STRICT_TRANSITION`;
  mục trong allowlist không còn khớp ⇒ `E_STRICT_STALE`. Thêm: `transition_tcb` **nhiều dòng**
  (máy kiểm quét theo dòng không xếp loại được) ⇒ ĐỎ, không bỏ qua im lặng.

### 3.2 Đột biến thật — máy kiểm **ĐỎ** (log nguyên văn)

Đột biến kernel (`core_ipc.rs` `reply_with`): thay `self.wake_from(caller)?;` bằng
`self.transition_tcb(caller, TcbState::Ready)?;` — **đường wake thứ hai đi vòng qua mailbox**.

```text
$ python3 v2/tools/check-wake-paths.py --strict
E E_STRICT_TRANSITION 1 chỗ chuyển `TcbState::Ready` ngoài cửa wake (allowlist tường minh KT-155)
E E_STRICT_TRANSITION   v2/kernel/vifast-core/src/core_ipc.rs:701 (reply_with)
# check-wake-paths: mode=v2 strict=1 errors=2 warnings=0
EXIT=1
```

Đột biến sim (`kobj.rs` `reply_with`) — cùng khuôn:

```text
$ python3 v2/tools/check-wake-paths.py --strict
E E_STRICT_TRANSITION 1 chỗ chuyển `TcbState::Ready` ngoài cửa wake (allowlist tường minh KT-155)
E E_STRICT_TRANSITION   v2/sim/vifast-sim/src/kobj.rs:9760 (reply_with)
# check-wake-paths: mode=v2 strict=1 errors=2 warnings=0
EXIT=1
```

Khôi phục (`cp` bản sao / `git checkout --`) ⇒ `errors=0` (log
`kt155-logs/kt155-mutation-checker.log`, `kt155-mutation-checker-sim.log`).

### 3.3 `--selftest` 11/11

```text
$ python3 v2/tools/check-wake-paths.py --selftest
ok - v2 sạch ⇒ xanh
ok - v2 thêm `IpiKind::Wake` ở arch ⇒ đỏ
ok - v2 thiếu `wake_step` ⇒ đỏ (fail-closed)
ok - v2 `--strict` transition ngoài cửa wake ⇒ đỏ
ok - v2 `--strict` + allowlist khớp ⇒ xanh
ok - v2 `--strict` đột biến trong hàm đã allowlist ⇒ đỏ
ok - v2 `--strict` allowlist mục ⇒ đỏ
ok - v2 `--strict` transition nhiều dòng ⇒ đỏ (fail-closed)
ok - v1 ba kênh ⇒ đỏ
ok - v1 một kênh ⇒ xanh
ok - cây rỗng ⇒ đỏ (fail-closed)
# selftest: 11/11 ok
```

### 3.4 Luật 2 — đổi định nghĩa máy kiểm ⇒ quét lại toàn bộ

`WAKE_PRODUCER` mở từ `("wake", "wake_from")` (KT-154) thành
`("wake", "wake_from", "wake_from_unlink")` (KT-155, N7-4) — vẫn **một** kênh
`MailboxMsg::Wake`, ba cửa. Đã quét lại **toàn bộ** `v2/kernel` + `v2/sim` (không allowlist
tệp): kết quả §2.4 (0 lỗi). `--selftest` 11/11.

## 4. (3) `--strict` vào cổng

`v2/bin/v2` `do_check`, cạnh hai bước KT-151:

```bash
run_step "wake paths (KT-151: một cơ chế wake)" "$PY" "$V2_ROOT/tools/check-wake-paths.py" || fails=$((fails + 1))
run_step "selftest wake paths (KT-151)" "$PY" "$V2_ROOT/tools/check-wake-paths.py" --selftest || fails=$((fails + 1))
run_step "wake paths --strict (KT-155: allowlist)" "$PY" "$V2_ROOT/tools/check-wake-paths.py" --strict || fails=$((fails + 1))
```

Log cổng (`kt155-logs/kt155-check-final3.log`):

```text
--- wake paths --strict (KT-155: allowlist)
...
--- wake paths --strict (KT-155: allowlist): OK
```

Đây là **thêm phép đo**, không nới ngưỡng nào.

## 5. (4) N7-4 — `wake_addr` chéo hart

`wake_addr` (kernel `core_sync.rs`, sim `kobj.rs`) trước đây gọi `wake_step(tcb, true)` trực
tiếp ⇒ waiter `BlockedOnAddress` ở hart khác **không** nhận `Wake` về `home`. Thêm
`wake_from_unlink` (như `wake_from` nhưng `unlink = true` — cần gỡ bucket WaitAddr) ở **cả hai**
tầng; `wake_addr` đi qua cửa đó.

**Đỏ-trước** (kernel, bỏ vá `core_sched.rs` + `core_sync.rs`):

```text
$ cargo test --manifest-path v2/Cargo.toml -p vifast-sim --test kt155_n7addr
test kt155_core_wake_addr_cheo_hart_phai_gui_wake ... FAILED
test kt155_core_wake_addr_cung_hart_khong_gui_wake ... ok
  left: 0
 right: 1
test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
EXIT=101
```

**Đỏ-trước** (sim, bỏ vá `kobj.rs` — phần N7-4):

```text
$ cargo test --manifest-path v2/Cargo.toml -p vifast-sim --test kt155_n7sim
test kt155_sim_cung_hart_khong_gui_wake ... ok
test kt155_sim_refill_cheo_hart_phai_gui_wake ... ok
test kt155_sim_resume_cheo_hart_phai_gui_wake ... ok
test kt155_sim_wake_addr_cheo_hart_phai_gui_wake ... FAILED
test kt155_sim_reply_cheo_hart_phai_gui_wake ... ok
  left: 0
 right: 1
test result: FAILED. 4 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
EXIT=101
```

**Xanh-sau** (cây đã vá): `kt155_n7addr` 2/2, `kt155_n7sim` 5/5 — xem §6 (workspace log).
Log: `kt155-logs/kt155-addr-red-before.log`, `kt155-sim-red-before-2.log`.

## 6. Ba cổng (bắt buộc)

```text
$ ./v2/bin/v2 check
...
--- wake paths --strict (KT-155: allowlist): OK
...
v2 check: PASS (L1 chạy trong gate --fast)
EXIT=0
```

(Log đầy đủ `kt155-logs/kt155-check-final3.log`.)

```text
$ cargo test --manifest-path v2/Cargo.toml --workspace
EXIT=0
$ grep -c "test result: ok" v2/agent/JOURNAL/kt155-logs/kt155-workspace-final.log
107
$ grep -n "FAILED\|failures:\|error\[" v2/agent/JOURNAL/kt155-logs/kt155-workspace-final.log
(không có dòng nào)
```

Trong đó: `kt154_n7fix` **7/7** (test cũ của KT-154 vẫn xanh), `kt155_n7addr` **2/2**,
`kt155_n7sim` **5/5**.

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common
EXIT=0
```

(Log `kt155-logs/kt155-plat-common-final.log`.)

**Test cũ có đỏ không?** **Không.** Workspace 107 dòng `test result: ok`, 0 dòng
`FAILED`/`failures:`/`error[`; không sửa kỳ vọng test cũ nào.

## 7. T-INFRA — bash 3.2 hỏng heap làm `self-test luật` đỏ oan

Trong lúc chạy cổng, `v2 check` có lúc đỏ bước `self-test luật` với **mọi** case exit 133
(`Trace/BPT trap: 5`). Điều tra:

- Crash report macOS: `BUG IN CLIENT OF LIBMALLOC: memory corruption of free block`,
  `SIGTRAP` trong `malloc_type_realloc` của **bash 3.2** (`/bin/bash`, macOS 27).
- **Tái hiện trên cây base**: `git stash` **toàn bộ** thay đổi của task rồi chạy `v2 check` ⇒
  vẫn `self-test luật: FAIL` (log `kt155-logs/kt155-check-base.log`) ⇒ **không phải** mã của
  task.
- Cùng bước chạy **độc lập** `bash v2/test/rules/run.sh` ⇒ `6/6 case đúng kỳ vọng`, EXIT=0
  (10 lần liên tiếp).
- Cách ly: chạy `rules "$V2_ROOT"` trong **subshell** (`if ( rules "$V2_ROOT" ); then …`) —
  cùng output, cùng mã thoát, chỉ cô lập hỏng heap của bash 3.2. Sau đó `v2 check` **PASS**
  (`kt155-check-final3.log`). Đây là **workaround hạ tầng**, không đổi ngưỡng/ngữ nghĩa cổng;
  ghi để lead/A0 quyết giữ hay thay bằng bash ≥ 4.

## 8. Tệp đã sửa/thêm

| tệp | loại |
|---|---|
| `v2/sim/vifast-sim/src/kobj.rs` | `wake_from` + `wake_step(tcb, unlink)` + `wake_from_unlink`; 13 đường → `wake_from`; `wake_addr` → `wake_from_unlink` |
| `v2/kernel/vifast-core/src/core_sched.rs` | `wake_from_unlink` (N7-4) |
| `v2/kernel/vifast-core/src/core_sync.rs` | `wake_addr` → `wake_from_unlink` |
| `v2/sim/vifast-sim/tests/kt155_n7sim.rs` | **mới** — 5 ca sim (Reply/Resume/Refill/WakeAddr chéo hart + cùng hart) |
| `v2/sim/vifast-sim/tests/kt155_n7addr.rs` | **mới** — 2 ca kernel `wake_addr` (N7-4) |
| `v2/tools/check-wake-paths.py` | allowlist tường minh + fail-closed hai chiều + chặn `transition_tcb` nhiều dòng; `WAKE_PRODUCER` thêm `wake_from_unlink`; selftest 11 ca |
| `v2/bin/v2` | `run_step` `--strict` trong `do_check`; workaround subshell cho bash 3.2 (T-INFRA) |
| `v2/agent/JOURNAL/kt155-logs/*` | log nguyên văn từng vế |
| `v2/agent/JOURNAL/kt155-n7-dong.md` | tệp này |

## 9. Việc còn lại (ghi để lead/A0 quyết)

- **`wake()` công khai vẫn "post-only"** khi khác hart (giữ nguyên từ KT-154 để không phá test
  KT-151): API công khai và cửa sản phẩm `wake_from` **khác nhau** ở chế độ chéo hart. Nếu A0
  muốn **một** ngữ nghĩa duy nhất thì phải đổi test KT-151 (task riêng).
- **`tcb_suspend`** vẫn chuyển `blocked → Ready` **không** gửi `Wake` (allowlist ghi lý do
  "treo: Ready + suspended (không chạy)"). Nếu về sau `Resume` cần đánh thức hart `home` thì
  phải mở task riêng — hiện `Tcb.Resume` chỉ `sched_sync`, không post `Wake`.
- **Bash 3.2** (T-INFRA §7): workaround subshell đã đủ cho cổng; nếu máy khác dùng bash ≥ 4 thì
  có thể bỏ workaround.
- **`--strict` allowlist là danh sách tường minh**: mọi chỗ `transition_tcb(…, Ready)` mới
  (kể cả non-wake) sẽ làm cổng ĐỎ cho tới khi thêm mục + lý do — đúng chủ ý fail-closed.
