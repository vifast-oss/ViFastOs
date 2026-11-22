# KT-329 — `Tcb.Suspend(Defer)` + `Tcb.Resume` (plan A, D2-228) — **DONE**

- **Vai**: sub-agent `defer` (lead = Agent 1). **Nhánh**: `task/kt329-defer`, base **`3bccfaa9`**
  (`main`, sau rebase — bản journal đầu khảo sát trên base `60a9e594`).
- **Ngày**: 2026-11-22. **Trạng thái**: **DONE** (commit `[KT-329]` trên `task/kt329-defer` — hash ở báo cáo lead). — cài theo **phương án A** lead chốt (D2-228):
  `Suspend(Defer)` + `Tcb.Resume` (12), **không** đổi ABI. Cổng: xem §9.
- **Không** tuyên bố K3 tái đạt; **không** push/merge.

## 0. Kết luận ngắn

`spec/abi.toml` **không** có method `Tcb.Restart` (khảo sát §1, nguyên văn). Lead chốt **A**
(D2-228): `Tcb.Restart` **bỏ**, thay bằng **`Tcb.Suspend(Defer)` + `Tcb.Resume` (12)**; `Restart`
chỉ còn là **tín hiệu nội bộ core→platform**. Vì vậy:

1. `Suspend(Defer)` trên `Ready`/`Running` **không** dừng ngay — chỉ đặt **cờ deferred 1 byte**;
2. **điểm an toàn** = cuối `syscall::dispatch` (sau khi syscall xong, trước khi về U-mode): cờ bật
   ⇒ chuyển `Inactive` như `Abort`, **giữ `entry`**, **xoá cờ**;
3. `Tcb.Resume` (12) là **đường khôi phục duy nhất** — xoá cờ `Defer` còn treo rồi chạy lại;
4. `Suspend(Abort)` **byte-identical** với KT-319; `mode = Defer` **không còn** trả 5;
5. **size-neutral**: `Machine` **184.792 B** trước **và** sau; `TcbData` 824 B; pin/trần/R1
   **không** đổi.

TAP 11 (ba mệnh đề a/b/c) xanh trong QEMU thật; ba đột biến bắt buộc đều **đỏ** (§8).

## 1. Khảo sát ABI (giữ nguyên từ bản journal chặn, đã đối chiếu lại base `3bccfaa9`)

| # | Câu hỏi | Trả lời | Nguồn (dòng) |
|---|---|---|---|
| 1 | `Tcb.Suspend` có `mode`? | **CÓ** — `args = "mode: Abort\|Defer"`; `params = ["tcb", "mode"]`, `returns = []` ⇒ arity 1, `MR0 = mode` | `spec/abi.toml:181`; `:261..264` |
| 2 | `Tcb.Restart` có method id? | **KHÔNG** — `[object].Tcb` đóng băng 16 method 0..15; không tên/số/chữ ký; `grep -rn "Restart" spec/` = 0 | `spec/abi.toml:181`; `:205..281` |
| 3 | `Restart` nhận tham số gì? | **N/A — không tồn tại**. Đường khôi phục duy nhất trong spec là `Tcb.Resume` (12): `params = ["tcb"]`, `returns = []` | `spec/abi.toml:266..268` |
| 4 | Bảng 11×11 có đủ cặp cho `Defer`? | **ĐỦ** — `Running → Inactive` và `Ready → Inactive` đều ALLOWED; **không** cần mở bảng | `v2/kernel/vifast-core/src/tcb.rs:387..432` |
| 5 | `BlockedOn*` có "điểm an toàn"? | Đường **invocation** `Suspend` **từ chối** `BlockedOn*` (17) như KT-319; điểm an toàn chỉ cần lo `Running`/`Ready` | `invoke.rs`; `core_tcb.rs` |

### 1.1 Trích nguyên văn `spec/abi.toml`

`spec/abi.toml:181` — `[object].Tcb` (không có `Restart`):

```toml
Tcb = { id = 3, stability = "frozen", method = { 0 = { name = "Configure" }, 1 = { name = "SetFaultEp" }, 2 = { name = "ReadState" }, 3 = { name = "Suspend", args = "mode: Abort|Defer" }, 4 = { name = "SetPriority" }, 5 = { name = "SetMcp" }, 6 = { name = "SetAffinity" }, 7 = { name = "SetSchedParams" }, 8 = { name = "SetTimeoutEp" }, 9 = { name = "ReadRegs" }, 10 = { name = "WriteRegs" }, 11 = { name = "CopyRegs" }, 12 = { name = "Resume" }, 13 = { name = "BindNotification" }, 14 = { name = "UnbindNotification" }, 15 = { name = "SetTls", note = "K6" } } }
```

`spec/abi.toml:261..268`:

```toml
[method_signature."Tcb.Suspend"]
params = ["tcb", "mode"]
variants = { mode = ["Abort", "Defer"] }
returns = []

[method_signature."Tcb.Resume"]
params = ["tcb"]
returns = []
```

### 1.2 Vì sao `Restart` **không** phải method ABI của v2

`v2/docs/abi-v1-v2-mapping.md:73`:

```
| 18 | TcbRestart | **bỏ** | thay bằng `Tcb.Suspend(Defer)` + `Resume`; `Restart` chỉ còn là tín hiệu nội bộ core→platform (§4.13) |
```

### 1.3 Quyết định của lead (D2-228) và hiệu lực

- D2-227 **có** trong `v2/agent/DECISIONS.md` (dòng 431) trên base `3bccfaa9` — bản journal đầu
  khảo sát base cũ `60a9e594` nên kết luận "thiếu D2-227" là **sai**; đã bỏ.
- D2-228 (lead, toàn quyền A0): duyệt **phương án A**; **không** mở `spec/`; `Tcb.Restart` bỏ,
  `Resume` là đường khôi phục. Không có thay đổi ABI nào trong task này.

## 2. Thiết kế đã cài (size-neutral, không đổi ABI)

### 2.1 Cờ deferred — 1 byte, **không** thêm trường

`TcbData` 824 B, `MAX_TCBS = 64`; thêm một trường ⇒ +512 B ⇒ vượt trần (headroom 40 B). Vì vậy
thay `suspended: bool` (1 B) bằng enum 1 B:

```rust
// v2/kernel/vifast-core/src/tcb.rs
pub(crate) enum SuspendFlag {
    None,
    Abort,  // đường model KT-65 — `TcbInfo.suspended = true` (giữ nguyên KT-314/KT-319)
    Defer,  // KT-329 — chờ điểm an toàn; `TcbInfo.suspended` **vẫn** false (vẫn xếp lịch được)
}
impl SuspendFlag {
    pub(crate) fn is_suspended(self) -> bool { matches!(self, SuspendFlag::Abort) }
}
```

`tcbdata.rs`: `pub suspend: SuspendFlag` (thay `suspended: bool`); `EMPTY` dùng `None`;
`snapshot()` ghi `suspended: self.suspend.is_suspended()`. Bốn chỗ sched đọc `suspended` đổi sang
`info.suspend.is_suspended()` (`core_sched.rs`), `core_tcb.rs` tương tự.

### 2.2 `Suspend(Defer)` — không đổi trạng thái ngay

`core_tcb.rs::tcb_suspend_invocation`: sau kiểm quyền/arity/**trạng thái** (chỉ `Running`/`Ready`;
`BlockedOn*`/`Inactive`/`Stopping`/`Dead` ⇒ 17 — giữ nguyên KT-319):

- `Abort`: chuyển `Inactive` qua `transition_tcb` (một cửa), **giữ** `entry`/`exit_code`, xoá cờ
  `Defer` nếu có; `sched_after_op(true)`.
- `Defer`: nếu cờ đang `Abort` ⇒ 17 (không "hoãn" thêm được); ngược lại đặt cờ `Defer` và
  `sched_after_op(true)` — **không** đổi trạng thái, **không** đụng `entry`.
- `mode` khác `0/1` đã bị `invoke` từ chối ⇒ 5 (giữ nguyên).

### 2.3 Điểm an toàn — cuối `syscall::dispatch`

```rust
// v2/kernel/vifast-core/src/syscall.rs
pub fn dispatch(...) -> SyscallOutcome {
    let outcome = dispatch_inner(m, tcb, hart, nr, args, env);
    if matches!(&outcome, SyscallOutcome::Return(_)) && m.tcb_defer_safe_point(tcb) {
        return SyscallOutcome::Switched { state: TcbState::Inactive };
    }
    outcome
}
```

`tcb_defer_safe_point` (core_tcb.rs): nếu cờ `Defer` **và** trạng thái `Running`/`Ready` ⇒
`transition_tcb(Inactive)` (giữ `entry`), **xoá cờ**, `sched_after_op(true)`, trả `true`; mọi
trường hợp khác **xoá cờ fail-closed** và trả `false`. Chỉ áp cho `Return` — `Exit`/`Halt`/
`Switched` không phải điểm an toàn.

Arch riscv64 trên nhánh `Switched` đã `advance_pc(4)`, lưu `tf`, `stop(3,0)`, `exit_to_kernel`
(KT-280) ⇒ tác vụ dừng sạch **sau** `ecall`, `Resume` chạy tiếp đúng lệnh kế.

### 2.4 `Tcb.Resume` (12) — đường khôi phục duy nhất

`core_tcb.rs::tcb_resume_invocation`: giữ nguyên KT-314 (chỉ `Ready`/`Inactive` chưa chạy; đọc
`entry`, `Inactive ⇒ start_tcb`, đặt `Running{hart}`, xoá `exit_code`), thêm: nếu cờ `Defer` còn
treo ⇒ **xoá trước** khi chạy. Cờ `Abort` của model KT-65 **không** bị đụng (giữ ngữ nghĩa
KT-314).

### 2.5 Model API KT-65 / `Tcb.ReadState` / `invoke::stats`

- `Machine::tcb_suspend` (model KT-65) giữ nguyên: `Abort` dùng cờ `Abort` + `suspended=true`;
  `Defer` ở model vẫn ⇒ 5 (chưa có "điểm an toàn" cho đường model — ngoài phạm vi invocation).
  Ghi rõ trong doc.
- `tcb_resume` model đặt `SuspendFlag::None`; `tcb_resume_with_regs` xoá cờ qua
  `set_tcb_suspend_flag(None)`; `tcb_read_state` đọc `t.suspend.is_suspended()`.
- `invoke::InvocationStats` thêm **`deferred: u32`** (sau `suspend`) — `suspend` vẫn chỉ đếm
  `Abort` ⇒ TAP 9 `calls=2` **không đổi**; TAP 11 đòi `deferred == 3`.
- `SuspendMode` doc cập nhật (Defer **được nhận** ở đường invocation).

## 3. Size proof (bắt buộc — size-neutral)

Lệnh: `cargo test -p vifast-core footprint -- --nocapture` (workspace `v2/`), **trước** = đo trên
base `3bccfaa9` (stash toàn bộ thay đổi mã), **sau** = bản KT-329:

```
# trước (base 3bccfaa9)
TOTAL                          184792
test result: ok. 3 passed; 0 failed; 0 ignored; ...

# sau (KT-329)
TOTAL                          184792
test result: ok. 3 passed; 0 failed; 0 ignored; ...
```

`TcbData` vẫn **824 B** (`SuspendFlag` 1 B thay `bool` 1 B, không thêm trường); pin `196.608` /
trần `184.832` / `MAX_TCBS = 64` **không** đổi.

## 4. Test host (T0, đỏ-trước)

- Test đỏ-trước: `suspend_tests::suspend_defer_khong_doi_trang_thai_ngay` — trước khi cài:
  `left: Err(5)`, `right: Ok((0, 0))` (log `/tmp/kt329-red-defer.log`).
- Sau khi cài, `cargo test -p vifast-core --lib` = **97 passed / 0 failed** (94 cũ + 3 mới):
  - `suspend_defer_khong_doi_trang_thai_ngay` — `Defer` trên `Ready` ⇒ `Ok((0,0))`, trạng thái
    **không** đổi, `entry` giữ; mode 2/`u64::MAX` vẫn `Err(5)`.
  - `suspend_defer_dung_o_diem_an_toan` — cờ `Defer` + `entry`; `tcb_defer_safe_point` ⇒
    `Inactive` + `entry` giữ + cờ `None`; gọi lại là **no-op** (cờ đã tiêu thụ).
  - `resume_xoa_co_defer` — `Resume` (12) ⇒ `Ok((0, entry))`, `Running`, cờ `None`.
  - `suspend_defer_giu_kiem_trang_thai` — `BlockedOnSend`/`Inactive`/`Dead` ⇒ 17, không đổi
    trạng thái/không đặt cờ.
- `cargo clippy` sạch qua `v2 check` (§9); `cargo fmt` đã chạy cả hai workspace.

## 5. TAP 11 — chứng minh ba mệnh đề (a)(b)(c)

Ảnh `ktest-ipc` (11 TAP). Rig cấp **ba** TCB `Ready` + SC + cspace (`cnode_c`, ô 22/24/26; SC
23/25/27), **không** đặt `entry` — client U-mode ghi `entry? = defer_entry` rồi `Suspend(Defer)`:

- **T5** — (a) sau `Defer` kernel đọc core: **vẫn `Ready`**, `entry != 0`; (b) chạy **một** lượt:
  đúng **một** `Tcb.Configure` thành công (`cfg5=1`) rồi **điểm an toàn** treo ⇒ `Inactive`,
  `entry` giữ; (c) `Tcb.Resume` (12) ⇒ `Running` + trả `entry`, chạy tiếp **sau `ecall`** ⇒
  `Exit(0)`.
- **T6** — `Resume` **trước khi** chạy (cờ còn treo) ⇒ phải xoá cờ; hai syscall ⇒ `Exit(0)`.
  Đột biến (ii) "Resume không xoá cờ" ⇒ syscall 1 lại bị treo ⇒ `exit6` mất.
- **T7** — chạy lại **không** qua `Resume` (rig `set_running`): điểm an toàn đã xoá cờ ⇒ syscall 2
  chạy ⇒ `Exit(0)`. Đột biến (iii) "bỏ xoá cờ ở điểm an toàn" ⇒ syscall 2 bị treo lại ⇒ `exit7`
  mất — **có trần**, không vòng lặp vô hạn.

Bằng chứng kernel (in **sau** ba lượt; mọi số đọc từ core):

```
# ipc defer: state5=Ready state6=Ready state7=Ready calls=3 cfg5=1 st5a=Inactive entry5=0x40001a18 res5=0x40001a18 st5b=Running exit5=0 st5c=Inactive st6a=Running res6=0x40001a18 exit6=0 st7a=Inactive exit7=0 cfg7=2 stop5a=Blocked stop7a=Blocked
```

Client in: `# ipc: defer t5=0 t6=0 t7=0 entry=0x40001a18 ok=1`. Không có đường nào treo: lượt
`run_turn` luôn kết thúc bằng `Stop::Blocked`/`Exit`; ảnh có `PANIC` kết thúc.

## 6. Phần mã đã sửa (13 tệp, không có `spec/`)

| Tệp | Thay đổi |
|---|---|
| `v2/kernel/vifast-core/src/tcb.rs` | `SuspendFlag` 1 B + `is_suspended`; doc `SuspendMode` |
| `v2/kernel/vifast-core/src/tcbdata.rs` | `suspended: bool` → `suspend: SuspendFlag`; `snapshot` |
| `v2/kernel/vifast-core/src/core_sched.rs` | `set_tcb_suspend_flag` (thay `set_tcb_suspended`); 4 chỗ đọc cờ |
| `v2/kernel/vifast-core/src/core_tcb.rs` | `Suspend(Defer)` không đổi trạng thái; `tcb_defer_safe_point`; `Resume` xoá cờ; model/ReadState |
| `v2/kernel/vifast-core/src/syscall.rs` | wrapper `dispatch` + điểm an toàn |
| `v2/kernel/vifast-core/src/invoke.rs` | `InvocationStats.deferred`, đếm `Defer`, doc |
| `v2/kernel/vifast-core/src/suspend_tests.rs` | 3 test mới + sửa test Defer cũ |
| `v2/kernel/vifast-kbin-riscv64/init/ipc_client_user.rs` | pha 9b: `entry? = defer_entry`, `Defer` ×3; `defer_entry` (2 syscall rồi `Exit(0)`); bỏ ca âm `defer=5` |
| `v2/kernel/vifast-kbin-riscv64/src/ktest_ipc.rs` | TAP 11: gọi `rig::run_defer_tap`, `ok 11`, PANIC 11 TAP; `1..11` |
| `v2/kernel/vifast-kbin-riscv64/src/ktest_ipc_rig.rs` | cấp T5/T6/T7; `DeferTap` + `run_defer_tap` (R1) |
| `v2/test/boot/expected.txt` | `ktest-ipc 11 11` + khối KT-329 + sửa KT-319 (Defer hết là ca âm) |
| `v2/test/boot/ktest.sh` | manifest TAP 11, evidence `# ipc defer:`/`# ipc: defer`, PANIC 11 TAP |
| `v2/test/boot/selftest.sh` | ca âm (t): xoá dòng `# ipc defer:` ⇒ runner đỏ |

R1: `ktest_ipc.rs` **1.404** dòng (TAP 11 dời sang rig), `ktest_ipc_rig.rs` **656**,
`ipc_client_user.rs` **1.058**, `core_tcb.rs` **1.319** — tất cả ≤ 1.500. `init/user.rs` **không**
chạm.

## 7. QEMU — log nguyên văn (một boot `ktest-ipc`, smp=1)

```
# ipc: suspend ok=1 entry=0x40001a58
# ipc: suspend neg arity=5 arity2=5 inactive=17 blocked=17 ro=3 ok=1
# ipc: defer t5=0 t6=0 t7=0 entry=0x40001a18 ok=1
...
# ipc: suspend state=Inactive suspended=0 calls=2 resumed=Running blocked=BlockedOnSend
# ipc: resume ran entry=0x40001a58
# ipc: vspace ok obj=20 applied=20 match=1 root=0x804f8000 asid=1
# ipc: vspace clear ran=1
# ipc: vspace2 t1=10 t2=14 t0=11 v1=20 v2=21 v0=0 ap1=20 ap2=21 ap0=0 match2=0 diff=1 st1=Inactive st2=Running st0=Inactive
# ipc defer: state5=Ready state6=Ready state7=Ready calls=3 cfg5=1 st5a=Inactive entry5=0x40001a18 res5=0x40001a18 st5b=Running exit5=0 st5c=Inactive st6a=Running res6=0x40001a18 exit6=0 st7a=Inactive exit7=0 cfg7=2 stop5a=Blocked stop7a=Blocked
# ipc buf: bound=1 va=0x40020000 mr_off=80 mr_words=60
ok 1 - ipc: umode call
ok 2 - ipc: server replied
ok 3 - ipc: mr4..mr7 via buffer
ok 4 - ipc: tcb configure via invocation
ok 5 - ipc: cap transfer
# ipc final: client_exit=0 server_exit=0 target_exit=0 reply_clean=1 check_ipc=1 check_caps=1 capture_client=1 capture_server=1 client=Inactive server=Inactive target=Inactive
ok 6 - ipc: tcb configure full
ok 7 - ipc: tcb resume
ok 8 - ipc: tcb configure vspace
ok 9 - ipc: tcb suspend
ok 10 - ipc: tcb configure vspace clear + hai tcb
ok 11 - ipc: tcb suspend defer + resume
PANIC: KT-329 ipc u-mode + mr buffer + tcb configure + cap transfer + configure full + tcb resume + configure vspace + tcb suspend + vspace clear + hai tcb + suspend defer xong (11 TAP, 0 fail)
```

## 8. Ba đột biến bắt buộc — đều ĐỎ (log nguyên văn)

Mỗi đột biến: sửa đúng một chỗ trong `core_tcb.rs`, build lại ảnh `ktest-ipc`, một boot QEMU;
**không** treo (ảnh tự kết thúc bằng `PANIC ... CO LOI`).

### 8.1 (i) `Defer` dừng ngay như `Abort`

```
# ipc defer: state5=Inactive state6=Inactive state7=Inactive calls=3 cfg5=2 st5a=Inactive entry5=0x40001a18 res5=0x0 st5b=Inactive exit5=0 st5c=Inactive st6a=Running res6=0x40001a18 exit6=0 st7a=Inactive exit7=0 cfg7=2 stop5a=Exit stop7a=Exit
not ok 11 - ipc: tcb suspend defer + resume: said=1 calls=3 st5=Inactive st6=Inactive st7=Inactive entry5=0x40001a18 cfg5=2 st5a=Inactive entry5b=0x40001a18 res5=0x0 st5b=Inactive exit5=0 res6=0x40001a18 st6a=Running exit6=0 st7a=Inactive exit7=0 cfg7=2 check_ipc=1 check_caps=1
PANIC: KT-329 ipc u-mode + mr buffer + tcb configure + cap transfer + configure full + tcb resume + configure vspace + tcb suspend + vspace clear + hai tcb + suspend defer CO LOI
```

Hỏng đúng mệnh đề (a): `state5..7 = Inactive` (đáng lẽ `Ready`) ⇒ TAP 11 đỏ.

### 8.2 (ii) `Resume` không xoá cờ `Defer`

```
# ipc defer: state5=Ready state6=Ready state7=Ready calls=3 cfg5=1 st5a=Inactive entry5=0x40001a18 res5=0x40001a18 st5b=Running exit5=0 st5c=Inactive st6a=Running res6=0x40001a18 exit6=-1 st7a=Inactive exit7=0 cfg7=2 stop5a=Blocked stop7a=Blocked
not ok 11 - ipc: tcb suspend defer + resume: said=1 calls=3 st5=Ready st6=Ready st7=Ready entry5=0x40001a18 cfg5=1 st5a=Inactive entry5b=0x40001a18 res5=0x40001a18 st5b=Running exit5=0 res6=0x40001a18 st6a=Running exit6=-1 st7a=Inactive exit7=0 cfg7=2 check_ipc=1 check_caps=1
PANIC: KT-329 ipc u-mode + mr buffer + tcb configure + cap transfer + configure full + tcb resume + configure vspace + tcb suspend + vspace clear + hai tcb + suspend defer CO LOI
```

Hỏng đúng đột biến: `exit6=-1` (T6 bị treo lại ở syscall 1), T5/T7 vẫn qua.

### 8.3 (iii) Điểm an toàn **không** xoá cờ

```
# ipc defer: state5=Ready state6=Ready state7=Ready calls=3 cfg5=1 st5a=Inactive entry5=0x40001a18 res5=0x40001a18 st5b=Running exit5=0 st5c=Inactive st6a=Running res6=0x40001a18 exit6=0 st7a=Inactive exit7=-1 cfg7=2 stop5a=Blocked stop7a=Blocked
not ok 11 - ipc: tcb suspend defer + resume: said=1 calls=3 st5=Ready st6=Ready st7=Ready entry5=0x40001a18 cfg5=1 st5a=Inactive entry5b=0x40001a18 res5=0x40001a18 st5b=Running exit5=0 res6=0x40001a18 st6a=Running exit6=0 st7a=Inactive exit7=-1 cfg7=2 check_ipc=1 check_caps=1
PANIC: KT-329 ipc u-mode + mr buffer + tcb configure + cap transfer + configure full + tcb resume + configure vspace + tcb suspend + vspace clear + hai tcb + suspend defer CO LOI
```

Hỏng đúng đột biến: `exit7=-1` (T7 bị treo lại ở syscall 2), T5/T6 vẫn qua.

## 9. Cổng

| Cổng | Kết quả | Bằng chứng |
|---|---|---|
| `cargo test -p vifast-core --lib` | **97 passed / 0 failed** | §4 |
| `./v2/bin/v2 check` | **PASS** (fmt + clippy + luật) | `/tmp/kt329-check-rb2.log` |
| `bash v2/test/boot/ktest.sh 1 matrix` | **PASS** 27/27 boot sạch (9 ảnh × smp 1/2/4, `FAIL=0 TIMEOUT=0`) | `/tmp/kt329-matrix.log` |
| `bash v2/test/boot/selftest.sh` | **PASS** 29/29 ca (thêm ca (t) KT-329, `ev44`) | `/tmp/kt329-selftest.log` |
| `--check-log ktest-ipc` | **PASS** (`plan=11 ok=11`, 47 evidence) | `/tmp/kt329-ipc-final.log` |

## 10. Đột biến phạm vi tệp — xác nhận

- **Không** sửa `spec/` (trích §1 đọc-only; **không** mở ABI).
- **Không** sửa `v2/agent/{STATE,QUEUE,DECISIONS}.md`, **không** sửa `v2/bin/v2`.
- **Không** push/merge, **không** chạm `main`.
- **Không** nới ngưỡng (pin `196.608`, trần `184.832`, R1 `1.500`, `MAX_TCBS 64`).
- **Không** `#[ignore]`/retry/`cfg(target_arch)` mới.
- **Không** tuyên bố K3 tái đạt.

## 11. Bàn giao

- Commit `[KT-329] Tcb.Suspend(Defer) + Tcb.Resume: dừng ở điểm an toàn, size-neutral, TAP 11` trên nhánh `task/kt329-defer` — **chưa** push/merge (hash ở báo cáo lead).
- Log: `/tmp/kt329-ipc-final.log` (QEMU xanh), `/tmp/kt329-mut-{i,ii,iii}.log` (đột biến),
  `/tmp/kt329-check-rb1b.log`, `/tmp/kt329-matrix.log`, `/tmp/kt329-selftest.log`.
