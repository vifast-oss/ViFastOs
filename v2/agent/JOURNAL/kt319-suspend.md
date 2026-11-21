# KT-319 — riscv64 **`Tcb.Suspend`** (method 3) qua invocation

Trạng thái: **đang làm** (nhánh `task/kt319-suspend`, base `main` @ `90ea39ef`), 2026-11-22.
Sub-agent `suspend`. Quyết định: **D2-190** (hợp đồng K3), **D2-205** (K3 đã đóng), **D2-222**
(vòng 21 mở KT-314/KT-315; KT-319 là nợ KT-314 §7.1), **D2-198** (uỷ quyền commit trên nhánh
task), **D2-05** (test đỏ trước), **D2-83** (core không đọc/ghi bộ nhớ user, không ghi CSR),
**D2-104** (arity chính xác), **D2-04/D2-22** (`spec/` đóng băng), **D2-11** (số method/hằng lấy
từ spec, không chép tay), **D2-25** (`Configure`/`Resume` xoá `exit_code`), **D2-49(d)** (bảng
chuyển trạng thái 11×11).

**Không** tuyên bố K3 đạt lại: K3 đã đóng theo D2-205; KT-319 chỉ **trả một nợ K3.5 đã công bố**
(KT-314 §7.1: *"`Tcb.Suspend` (method 3) vẫn chưa cài ở kênh invocation"*) và **không** mở lại
bất kỳ mục nào của hợp đồng K3.

## 1. Khảo sát trước (bắt buộc)

Đọc: `AGENTS.md`, `v2/AGENTS.md`, `v2/agent/DECISIONS.md` (D2-190/205/222; **D2-224 không có
trong tệp** — số cao nhất là D2-223), `spec/abi.toml` (`[object].Tcb`,
`[method_signature."Tcb.Suspend"]`, `[method_signature.state]`),
`v2/kernel/vifast-core/src/{invoke,core_tcb,tcb,state_write,error,syscall}.rs`,
`v2/kernel/vifast-core/src/resume_tests.rs`, `v2/kernel/vifast-kbin-riscv64/src/ktest_ipc.rs`,
`v2/kernel/vifast-kbin-riscv64/init/ipc_client_user.rs`,
`v2/kernel/vifast-plat-riscv64/src/ipc_buffer.rs`, `v2/test/boot/{ktest.sh,expected.txt,selftest.sh}`,
`v2/agent/JOURNAL/kt314-resume.md`.

| Câu hỏi | Trả lời | Bằng chứng |
|---|---|---|
| `Tcb.Suspend` có trong `spec/abi.toml` không? Method id? | **CÓ** — `[object].Tcb` method **3** `Suspend`, `args = "mode: Abort\|Defer"` | `spec/abi.toml:181` |
| Chữ ký? | `params = ["tcb", "mode"]`, `variants = { mode = ["Abort", "Defer"] }`, `returns = []` ⇒ **arity 1** (`mode` = MR0; `tcb` là chính cap đã gọi) | `spec/abi.toml:261..264` |
| Hằng sinh tự động? | `vifast_abi::TCB_SUSPEND = 3` (sinh từ spec — D2-11) | `v2/lib/vifast-abi/src/generated.rs:251` |
| `SuspendMode` có sẵn? | **CÓ** — enum `Abort`/`Defer` (KT-65), `Defer` ⇒ `InvalidArgument` (K1b chưa có `Restart`) | `tcb.rs:231..245` |
| Bảng chuyển trạng thái cho phép gì? | `Running → Inactive` **ALLOWED**, `Ready → Inactive` **ALLOWED**; `Ready → Ready` **CẤM**; `Running → Ready` ALLOWED | `tcb.rs:367..437` (`ALLOWED`) |
| `Resume` (KT-314) nhận trạng thái nào? | `Ready` ⇒ đi; `Inactive` **chưa từng chạy** (`exit_code = None`) ⇒ `start_tcb` rồi đi; còn lại ⇒ `IllegalOperation` **17** | `core_tcb.rs:318..340` |
| Model API `tcb_suspend` (KT-65) làm gì? | `Abort`: `BlockedOn*`/`Running` → `Ready` + cờ `suspended`; `Ready` giữ `Ready`; `Defer` ⇒ 5; `Inactive`/`Stopping`/`Dead` ⇒ 17 | `core_tcb.rs:671..691` |
| Có cửa chuyển `Inactive` sẵn? | **CÓ** — `Machine::transition_tcb(obj, NonReadyTarget::Inactive)` (cửa 0, bảng 11×11) | `state_write.rs:180..186` |
| `Tcb.ReadState` trả gì? | `{state, suspended, blocked_on_kind, exit_code}` — đo **độc lập** được trạng thái thật | `core_tcb.rs:573..582` |
| Ai dựng literal `InvocationStats`? | `ktest_inv.rs:213` ⇒ thêm trường `suspend` phải sửa cả đó | `ktest_inv.rs:213..231` |
| `resume_tests.rs` khẳng định `Suspend` **chưa** cài? | **CÓ** — `assert!(!invoke::is_whitelisted(ObjType::Tcb, 3))` ⇒ phải cập nhật (nay đã cài) | `resume_tests.rs:243` |

⇒ **Kết luận khảo sát**: (a) **không** cần đổi ABI — `Tcb.Suspend` đã có method **3** + chữ ký
arity **1** trong `spec/`; (b) bảng 11×11 **đã cho** `Running → Inactive` và `Ready → Inactive`
⇒ không cần cửa mới, không nới bảng; (c) `Resume` (KT-314) **đã** nhận `Inactive` chưa chạy ⇒
cặp `Suspend → Resume` chạy lại được **không** phải sửa `Resume`; (d) model API `tcb_suspend`
(KT-65) có ngữ nghĩa **khác** (đưa về `Ready` + cờ `suspended`, nhận cả `BlockedOn*`) ⇒ **không**
dùng lại cho invocation, để không đổi ngữ nghĩa `kill` của procsrv; (e) **không** đổi
`size_of::<Machine>()` (không thêm trường) ⇒ không phải sửa bốn dòng evidence `bytes=`/`sum=`/
`static=` trong `ktest.sh`.

## 2. Thiết kế

### 2.1 Ngữ nghĩa `Tcb.Suspend` (invocation, method 3)

Thứ tự **bắt buộc** (lỗi ⇒ **không** đổi gì):

1. Cap phải là `Tcb` sống; cap phải có `RIGHT_WRITE` (thiếu ⇒ `InsufficientRights` **3**, cùng
   luật `Tcb.Configure`/`Tcb.SetTls`/`Tcb.Resume` — D2-99(e)).
2. **Arity chính xác 1** (D2-104): `length` khác 1 ⇒ `InvalidArgument` **5**, có đếm `denied`.
3. **Mode**: `MR0 = 0` (`Abort`) ⇒ đi; `1` (`Defer`) ⇒ `InvalidArgument` **5** (K1b chưa có
   `Restart`, đúng ngữ nghĩa model `tcb_suspend`); giá trị khác ⇒ **5**.
4. **Trạng thái trước**: chỉ `Running`/`Ready` ⇒ đi; còn lại — `Inactive`, `BlockedOn*`,
   `Sleeping`, `Stopping`, `Dead` — ⇒ `IllegalOperation` **17**. (Khác model API KT-65: đường
   invocation **không** nhận `BlockedOn*` — treo một tác vụ đang chờ IPC là việc của
   `Reply`/`Notification`/`Exit`, không phải `Suspend`; đây là lựa chọn `[TK]` KT-319, ghi rõ.)
5. Chuyển `Inactive` qua **một** cửa `transition_tcb` (bảng 11×11 vẫn là nguồn sự thật).
6. **Giữ** `TcbData.entry` (để `Resume` sau đó chạy lại được — mốc thật của task) và **giữ**
   `exit_code` (D2-25: chỉ `Configure`/`Resume` xoá; tác vụ `Running`/`Ready` vốn `None`).
7. Trả `(a0, a1) = (0, 0)` — `[method_signature."Tcb.Suspend"].returns = []`.

**Vì sao `Inactive` chứ không `Ready` + cờ `suspended`** (`[TK]` KT-319): (i) bảng 11×11 **đã
cho** `Running → Inactive`/`Ready → Inactive` nên không phải thêm cửa/ngoại lệ; (ii) `Resume`
(KT-314) **đã** nhận `Inactive` chưa chạy ⇒ cặp `Suspend`/`Resume` đủ mà **không** sửa `Resume`;
(iii) nếu dùng `Ready` + `suspended = true` thì `Resume` phải **xoá cờ** (nếu không, `sched_sync`
không đưa vào ready queue ⇒ tác vụ không bao giờ chạy lại) — đó là sửa ngữ nghĩa KT-314 ngoài
phạm vi; (iv) `suspended` là cờ của đường model `tcb_suspend` (KT-65 `kill`), giữ nguyên hành vi
cũ. Hệ quả: `Tcb.ReadState` sau `Suspend` = `state=Inactive suspended=0` — **không** phải
`Ready+suspended`; ghi rõ để người đọc không nhầm với KT-65.

### 2.2 Kênh tham số (không đổi ABI)

```text
U-mode: Call(cap TCB đích, {label = Tcb.Suspend, length = 1}, MR0 = mode)
  → core: kiểm cap/quyền/arity/mode/trạng thái → transition_tcb(Inactive)
  → trả (0, 0)                                  [U-mode: a0 = 0, a1 = 0]
```

Hằng/API mới: `invoke::TCB_SUSPEND` (= `vifast_abi::TCB_SUSPEND`),
`invoke::{SUSPEND_MODE_ABORT, SUSPEND_MODE_DEFER}`, `Machine::tcb_suspend_invocation`,
`InvocationStats::suspend` (bộ đếm TAP).

## 3. Kết quả

### 3.1 Tầng core (host test)

`cargo test -p vifast-core --lib` (chạy **trong `v2/kernel/`**):

```text
test result: ok. 89 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

**89** = **79** test của `main` + **10** test `suspend_tests::*` (danh sách ở §1 + 6 ca mới của
KT-319). Không sửa test cũ nào khác ngoài hai dòng khẳng định danh sách trắng trong
`resume_tests.rs` (`Suspend` nay **đã** cài, `SetPriority` vẫn **chưa**).

### 3.2 Tầng ảnh (`ktest-ipc`, TAP 8)

| Đo | Giá trị đo được | Bằng chứng (dòng log) |
|---|---|---|
| (a) `Suspend(Abort)` trên đích `Ready` trả `(0,0)` | `ok 8` | `# ipc: suspend ok=1 entry=0x40001502` |
| (b) `Tcb.ReadState` sau `Suspend` | `state=Inactive suspended=0 blocked_on_kind=None exit_code=None` | `# ipc: suspend state=Inactive suspended=0 calls=2 resumed=Running blocked=BlockedOnSend` |
| (c) `Suspend` rồi `Resume` ⇒ chạy lại **thật** | `Running` + tác vụ `Exit(0)` | `# ipc: resume ran entry=0x40001502` + `# u-mode exit: code=0 … tcb=10` |
| delta `Tcb.Suspend` **thành công** | **2** (không phải "≥ 1") | `calls=2` |
| ca âm `mode=1` (Defer) | **5** | `defer=5` |
| ca âm arity `0` / `2` | **5** / **5** | `arity=5 arity2=5` |
| ca âm tác vụ `Inactive` | **17** | `inactive=17` |
| ca âm tác vụ `BlockedOnSend` | **17** | `blocked=17` + `blocked=BlockedOnSend` (trạng thái **không** đổi) |
| ca âm thiếu `RIGHT_WRITE` | **3** | `ro=3` |
| `check_ipc` / `check_caps` sau toàn bộ | **1 / 1** | `# ipc final: … check_ipc=1 check_caps=1 …` |

**Khảo sát mới (quan trọng — khác §2)**: bảng 11×11 **đã cấm** `Inactive → Inactive` và
`Dead → Inactive`, nên **kiểm trạng thái** của đường invocation chỉ **quan sát được** ở những
trạng thái mà bảng **cho** đi tiếp — tức `BlockedOn*`/`Sleeping`. Vì vậy ảnh có thêm TCB đích thứ
tư ở **`BlockedOnSend`** (trên `Endpoint` **riêng**, không đụng luồng `Call`/`Reply` của TAP 1..7):
ca âm `BlockedOnSend ⇒ 17` là ca **duy nhất** làm đột biến (i) đỏ ở tầng ảnh (§6.1). Ca
`Inactive ⇒ 17` vẫn giữ vì nó chứng minh **mã lỗi** đúng, không phải chứng minh kiểm trạng thái.
Đây là **bổ sung** so với §2, không phải thiết kế lại: thứ tự kiểm, ngữ nghĩa và mã lỗi giữ nguyên.

### 3.3 Tầng harness

`bash v2/test/boot/ktest.sh --check-log ktest-ipc <log> <smp>` = **PASS** cho `smp=1,2,4`:
`plan=8 ok=8, 39 evidence`.

## 4. Log QEMU nguyên văn

Lệnh (đúng như `ktest.sh` dùng, qua `qemu_guard.sh` — **không** `perl -e 'alarm N'`):

```bash
qemu-system-riscv64 -machine virt,aia=none,aclint=on \
  -bios /opt/homebrew/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin \
  -kernel v2/kernel/target/riscv64gc-unknown-none-elf/release/vifast-kbin-ktest-ipc \
  -nographic -cpu rv64 -m 128M -smp 1 -no-reboot
```

`/tmp/kt319-logs/ipc-smp1-final.log`, **nguyên văn** từ dòng `TAP version 13` (OpenSBI in banner +
cấu hình hart phía trên, không liên quan phép kiểm):

```text
TAP version 13
1..8
# ipc rig: stvec=0x802000a0 untyped=0 core_untyped=1 cspace_c=17 cspace_s=18 cspace_alt=19 tcb_c=3 tcb_s=4 ep=5 reply=6 ipc_buf=16
# ipc: configure ok va=0x40020000
# ipc: configure err=0
# ipc: configure neg cap=18 method=18 outside=5 ok=1
# ipc configure: method=0 delta=3 denied=2 bound=1 va=0x40020000
# ipc call: blocked=1 state=BlockedOnSend call=1 ep_len=1 ep_mr=0x1234 label=0x280 stop=Blocked
# ipc server: recv=1 label=0x280 mr=0x1234 client=BlockedOnReply caller=3 server=Running stop=Yield
# ipc: server ran nr=1
# ipc reply: client=Ready reply_caller=0 server=BlockedOnRecv stop=Blocked
# ipc: umode call ok reply=23130
# ipc: mr count=8 mr7=0x28210007
# ipc mr: round=1 srv_len=8 srv_mr4=0x28200004 srv_mr7=0x28200007 cli_len=8 cli_mr4=0x28210004 cli_mr7=0x28210007
# ipc client: reply_label=0x281 reply_mr0=0x5a5a reply_mr1=1 const:label_reply=0x281 const:reply_value=0x5a5a
# ipc: server ran nr=2
# ipc mr: round=2 srv_len=64 srv_mr4=0x28200004 srv_mr63=0x2820003f
# ipc: mr count=64 mr63=0x2821003f
# ipc mr: round=2 cli_len=64 cli_mr4=0x28210004 cli_mr63=0x2821003f
# ipc: cap busy=7 ok=1
# ipc: cap used slot=5 copy=6
# sys: nr=14 err=0 value=0 tcb=4 state=Inactive src=core
# u-mode exit: code=0 faultkind=Exit{0} tcb=4 state=Inactive
# ipc caps: sent=1 received=1 slot=5 rights=0xb obj=16 ty=Frame alive=1 copy_slot=6 copy_obj=16 copy_ty=Frame same=1
# ipc: cap sent=1 slot=5
# ipc: cap neg grant=3 over=5 ok=1
# ipc: configure2 ok=1 entry=0x40000112 prio=4
# ipc: configure2 neg align=6 prio=5 cspace=2 vspace=18 ok=1
# ipc: configure2 entry=0x40000112 prio=4 cspace=17 vspace=0 calls=2 denied=5 ok=1
# ipc: resume ok=1 entry=0x40001502
# ipc: resume neg noentry=5 running=17 exited=17 ok=1
# ipc: neg empty=1 reply=18 ok=1
# ipc: neg mr over=5 ok=1
# ipc: suspend ok=1 entry=0x40001502
# ipc: suspend neg defer=5 arity=5 arity2=5 inactive=17 blocked=17 ro=3 ok=1
# sys: nr=14 err=0 value=0 tcb=3 state=Inactive src=core
# u-mode exit: code=0 faultkind=Exit{0} tcb=3 state=Inactive
# ipc: resume ok entry=0x40001502 state=Running
# ipc: resume neg noentry=5 state=Inactive
# ipc: suspend state=Inactive suspended=0 calls=2 resumed=Running blocked=BlockedOnSend
# ipc: resume ran entry=0x40001502
# sys: nr=14 err=0 value=0 tcb=10 state=Inactive src=core
# u-mode exit: code=0 faultkind=Exit{0} tcb=10 state=Inactive
# ipc buf: bound=1 va=0x40020000 mr_off=80 mr_words=60
ok 1 - ipc: umode call
ok 2 - ipc: server replied
ok 3 - ipc: mr4..mr7 via buffer
ok 4 - ipc: tcb configure via invocation
ok 5 - ipc: cap transfer
# ipc final: client_exit=0 server_exit=0 target_exit=0 reply_clean=1 check_ipc=1 check_caps=1 capture_client=1 capture_server=1 client=Inactive server=Inactive target=Inactive
ok 6 - ipc: tcb configure full
ok 7 - ipc: tcb resume
ok 8 - ipc: tcb suspend
PANIC: KT-319 ipc u-mode + mr buffer + tcb configure + cap transfer + configure full + tcb resume + tcb suspend xong (8 TAP, 0 fail)
```

`-smp 2` và `-smp 4` cho **cùng** kết quả `plan=8 ok=8` (`/tmp/kt319-logs/ipc-smp2.log`,
`/tmp/kt319-logs/ipc-smp4.log`); hart phụ vẫn park (`ktest_ipc::rust_secondary_main`) — phép kiểm
**không** phụ thuộc thứ tự/thời điểm giữa các hart (một hart chạy mọi lượt `run_turn`; xem
`v2/docs/k6-test-invariants.md`).

## 5. TAP + matrix

- `expected.txt`: `ktest-ipc 8 8` (trước: `7 7`), kèm khối chú thích KT-319.
- `manifest_ipc`: thêm **`ipc: tcb suspend`** (thứ tự 1..8).
- `panic_allow_re ktest-ipc`:
  `^PANIC: KT-319 ipc u-mode \+ mr buffer \+ tcb configure \+ cap transfer \+ configure full \+ tcb resume \+ tcb suspend xong \(8 TAP, 0 fail\)$`.
- Evidence mới (**3** dòng, **append** sau `# ipc: resume ran` để ordinal cũ **không** đổi):
  1. `^# ipc: suspend state=Inactive suspended=0 calls=2 resumed=Running blocked=BlockedOnSend$`
  2. `^# ipc: suspend ok=1 entry=0x[0-9a-f]+$`
  3. `^# ipc: suspend neg defer=5 arity=5 arity2=5 inactive=17 blocked=17 ro=3 ok=1$`
- `selftest.sh` thêm **ca (r)**: xoá dòng `# ipc: suspend state=Inactive` ⇒ runner phải **đỏ**
  kèm token `ev<ordinal>` (chứng minh dòng đó là evidence **bắt buộc**).
- `budget.sh` (ảnh `ktest-ipc`): `file=419976 data+bss=205352 budget=262144` ⇒ **dưới trần**.

Matrix `bash v2/test/boot/ktest.sh 1 matrix` (8 ảnh × smp 1/2/4 = 24 boot):

```text
# KTEST: 27/27 boot sạch (smp=matrix, NBOOT=1, ảnh/boot=ktest ktest-guard ktest-user ktest-proc ktest-vblk ktest-con ktest-exec ktest-ipc ktest-netsrv)
# ktest.sh: PASS
```

`ktest-ipc` **8/8** ở **cả ba** cấu hình; dòng evidence TAP 8 **giống hệt** nhau (nên phép kiểm
**không** phụ thuộc số hart):

```text
# ipc: suspend ok=1 entry=0x40001502
# ipc: suspend neg defer=5 arity=5 arity2=5 inactive=17 blocked=17 ro=3 ok=1
# ipc: suspend state=Inactive suspended=0 calls=2 resumed=Running blocked=BlockedOnSend
```

Thư mục log **riêng** của lần đo sạch: `/tmp/kt319-matrix/` (`ktest-ipc-smp{1,2,4}-1.log`).

**`selftest.sh` ca (r) — kiểm bằng đúng hai lệnh của ca** (không chạy được **toàn bộ**
`selftest.sh`: nó `die` khi thấy `ktest.sh` khác đang chạy, và lúc đó **phiên khác**
(`/tmp/kt318-s3`, KT-320) đang chạy matrix — không được giết tiến trình của phiên khác, AGENTS §9):

```text
$ bash v2/test/boot/ktest.sh --print-evidence ktest-ipc 1 | grep suspend
34	^# ipc: suspend state=Inactive suspended=0 calls=2 resumed=Running blocked=BlockedOnSend$
35	^# ipc: suspend ok=1 entry=0x[0-9a-f]+$
36	^# ipc: suspend neg defer=5 arity=5 arity2=5 inactive=17 blocked=17 ro=3 ok=1$

$ grep -v -F '# ipc: suspend state=Inactive' /tmp/kt319-matrix/ktest-ipc-smp1-1.log > log-no-suspend.log
$ bash v2/test/boot/ktest.sh --check-log ktest-ipc log-no-suspend.log 1
#   thiếu evidence#34: ^# ipc: suspend state=Inactive suspended=0 calls=2 resumed=Running blocked=BlockedOnSend$
not ok - check-log ktest-ipc smp=1: thiếu ev34, log log-no-suspend.log
```

⇒ ca (r) đỏ **đúng** token `ev34`, đúng lý do. Chạy **toàn bộ** `selftest.sh` để dành cho cổng
`--fast` khi không có phiên khác đo (ghi ở §7).

## 6. Đột biến

Mỗi ca: sửa **một** chỗ trong `v2/kernel/vifast-core/src/core_tcb.rs`, **build ảnh thật**,
**boot QEMU thật**, rồi khôi phục từ bản sao và chứng minh `cmp` + sha256 **ảnh** trùng khít bản
gốc. Ảnh gốc trước đột biến: sha256 `51844f2d362955fe4a30ace36fbe7d41590f95714aebc55b4b36ebbd5a99a460`.

### 6.1 Đột biến (i) — bỏ **kiểm trạng thái**

```diff
-        let state = self.tcb_state(obj)?;
-        if !matches!(state, TcbState::Running { .. } | TcbState::Ready) {
-            return Err(KError::IllegalOperation);
-        }
+        let _state = self.tcb_state(obj)?;
         self.transition_tcb(obj, NonReadyTarget::Inactive)?;
```

- Host test: `suspend_tests::suspend_blocked_illegal` **FAILED** (`left: Ok((0,0))`,
  `right: Err(17)`) — **9 passed / 1 failed**.
- Boot: **ĐỎ** (`/tmp/kt319-logs/ipc-mut-i.log`):

```text
not ok 8 - ipc: tcb suspend: said_ok=0 said_neg=0 calls=1 state=Ready state2=Ready state4=Inactive suspended=0 blocked=0 exit=1 resumed=Running ran=1 target_exit=0 check_ipc=1 check_caps=1
PANIC: KT-319 ipc u-mode + mr buffer + tcb configure + cap transfer + configure full + tcb resume + tcb suspend CO LOI
```

  Client thoát `117` (`EXIT_SUSPEND_BLOCKED`); `state4=Inactive` chứng minh tác vụ `BlockedOnSend`
  **bị treo** thay vì bị từ chối — đúng điều cần chứng minh.
- Khôi phục: `cmp` **OK**; sha256 `core_tcb.rs` = `a3562a72…`; **build lại ảnh** sha256
  `51844f2d…` = **đúng** ảnh trước đột biến.

### 6.2 Đột biến (ii) — bỏ **kiểm `mode`** (chỉ đường invocation)

```diff
-        if mode == SuspendMode::Defer {
-            return Err(KError::InvalidArgument);
-        }
+        let _ = mode;
```

(`tcb_suspend_invocation` mới là chỗ sửa — **không** đụng `Machine::tcb_suspend` của KT-65.)

- Host test: `suspend_tests::suspend_defer_va_mode_la_invalid_argument` **FAILED** — **9 passed /
  1 failed**.
- Boot: **ĐỎ** (`/tmp/kt319-logs/ipc-mut-ii.log`):

```text
not ok 8 - ipc: tcb suspend: said_ok=0 said_neg=0 calls=1 state=Inactive state2=Inactive state4=BlockedOnSend suspended=0 blocked=0 exit=1 resumed=Running ran=1 target_exit=0 check_ipc=1 check_caps=1
PANIC: KT-319 ipc u-mode + mr buffer + tcb configure + cap transfer + configure full + tcb resume + tcb suspend CO LOI
```

  Client thoát `111` (`EXIT_SUSPEND_DEFER`); `state=Inactive` chứng minh `Defer` **được nhận** và
  treo tác vụ `Ready`.
- Khôi phục: `cmp` **OK**; sha256 ảnh **trùng** `51844f2d…`.

## 7. Còn gì chưa xong / rủi ro

1. **`Defer` chưa có `Restart`** (K1b): `mode = Defer` hiện **bị từ chối** (5). Hỗ trợ thật cần
   `Restart`/đánh thức theo hạn — ngoài KT-319.
2. **Đường invocation không nhận `BlockedOn*`** (`[TK]` KT-319, khác model API KT-65): treo tác vụ
   đang chờ IPC là việc của `Reply`/`Notification`/`Exit`. Model `Machine::tcb_suspend` (KT-65)
   **giữ nguyên** hành vi cũ cho procsrv — hai ngữ nghĩa cùng tồn tại, đã ghi trong doc-comment.
3. **Cờ `suspended` không bật** ở đường invocation ⇒ `Tcb.ReadState` sau `Suspend` là
   `state=Inactive suspended=0` (§2.1: bật cờ trên `Inactive` sẽ buộc `Resume` phải xoá cờ — sửa
   ngữ nghĩa KT-314 ngoài phạm vi).
4. **`Tcb.ReadState` chưa có invocation** ⇒ phép đo (b) làm ở **kernel** bằng model API
   `Machine::tcb_read_state` (**đo độc lập**, không tin lời chương trình). Khi `ReadState` được
   cài ở kênh invocation thì có thể đo từ U-mode.
5. **Ảnh chỉ có một hart chạy thật** (hart phụ park): phép kiểm **không** phụ thuộc thứ tự hart,
   nhưng **không** chứng minh `Suspend` dưới SMP thật. Rủi ro: `tcb_suspend_invocation` **không**
   kiểm TCB có đang `Running` trên hart **khác** hay không — mô hình một-hart-một-lượt chưa lộ;
   nếu mở SMP thật cho ảnh U-mode cần luật "không `Suspend` tác vụ `Running` trên hart khác"
   (hoặc IPI preempt).
6. `ktest.sh` build lại toàn bộ 8 ảnh mỗi vòng (hành vi cũ, không phải nợ mới).
7. **Hạ tầng — LOGDIR mặc định dùng chung giữa các worktree** (`$TMPDIR/k3-ktest`, không theo
   worktree): lúc KT-319 chạy matrix, phiên `/tmp/kt318-s3` (KT-320) chạy **cùng lúc** vào **cùng**
   thư mục ⇒ log của KT-319 bị ghi đè **sau** khi kiểm (lần đo vẫn xanh vì mỗi log được kiểm ngay
   sau boot; nếu đọc trộn thì sẽ **đỏ**, không thể "xanh oan"). Đã đo lại **sạch** với
   `KT54_LOGDIR=/tmp/kt319-matrix` — cùng kết quả `27/27`. Đề nghị: `ktest.sh` mặc định LOGDIR
   theo `git rev-parse --git-common-dir`/worktree (hoặc `mktemp -d` riêng) để hai phiên song song
   không đạp lên nhau. **Không** tự sửa `ktest.sh` ngoài phạm vi KT-319.
8. **Giao thoa với KT-320 khi tích hợp (cần lead biết)**: `main` **đã tiến** trong lúc KT-319 làm
   — `main` nay là `5f52e5b4` (KT-320 "configure vspace"), trong khi nhánh này base
   `90ea39ef` như yêu cầu. KT-320 chạm **7 tệp trùng** với KT-319:
   `core_tcb.rs`, `invoke.rs`, `init/ipc_client_user.rs`, `src/ktest_ipc.rs`,
   `expected.txt`, `ktest.sh`, `selftest.sh`. Cả hai đều thêm **TAP 8** cho `ktest-ipc`
   (`tcb suspend` vs `tcb configure vspace`) ⇒ khi tích hợp **phải** đánh số lại: kế hoạch
   `1..9`, một bên thành **TAP 9** (manifest + 3 evidence + mẫu PANIC + `expected.txt` +
   ca selftest tương ứng), và `# ipc final`/`# ipc: suspend` vẫn độc lập nên không phải đổi nội
   dung. **KT-319 không merge/tự đánh số lại** (luật: không merge, không chạm `main`); đây là
   việc của lead ở bước tích hợp. Cổng xanh ghi ở §5 áp cho **nhánh này** (`task/kt319-suspend`),
   **không** phải cho ứng viên tích hợp sau khi trộn KT-320.

## 8. Xác nhận luật

- **Không** push, **không** merge, **không** chạm `main` (chỉ commit trên `task/kt319-suspend`).
- **Không** sửa `v2/agent/{STATE,QUEUE,DECISIONS}.md`, **không** sửa `v2/bin/v2`, **không** sửa
  `spec/` (D2-04/D2-22).
- **Không** nới ngưỡng (budget ảnh, `expected.txt`, số TAP, số evidence, timeout QEMU giữ
  nguyên); **không** `#[ignore]`/`#[should_panic]`/retry; **không** `cfg(target_arch)` trong
  `vifast-core`.
- `git add` **theo đường dẫn cụ thể** (không `git add -A`/`git add .`) — sự cố GH001.
- Bất biến phép kiểm: mọi lượt U-mode do kernel chạy tuần tự trên một hart; không phép đo nào phụ
  thuộc thứ tự/thời điểm giữa các hart (`v2/docs/k6-test-invariants.md`).
