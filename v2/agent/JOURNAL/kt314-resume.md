# KT-314 — riscv64 **`Tcb.Resume`** (method 12) **tiêu thụ `TcbData.entry`**

Trạng thái: **đang làm** (nhánh `task/kt314-resume`, base `main` @ `a9f90972`), 2026-11-22.
Sub-agent `resume`. Quyết định: **D2-190** (hợp đồng K3), **D2-205** (K3 đã đóng), **D2-221/222**
(vòng 20 mở KT-313/KT-314), **D2-198** (uỷ quyền commit trên nhánh task), **D2-05** (test đỏ
trước), **D2-83** (core không đọc/ghi bộ nhớ user, không ghi CSR), **D2-104** (arity chính xác),
**D2-04/D2-22** (`spec/` đóng băng), **D2-11** (số method/hằng lấy từ spec, không chép tay),
**D2-25** (`Configure`/`Resume` xoá `exit_code`), **D2-49(d)** (bảng chuyển trạng thái 11×11).

**Không** tuyên bố K3 đạt lại: K3 đã đóng theo D2-205; KT-314 chỉ **trả một nợ K3.5 đã công bố**
(KT-311 §7.2: *"`entry?` mới **lưu** + đo được, **chưa** có đường tiêu thụ"*) và **không** mở lại
bất kỳ mục nào của hợp đồng K3.

## 1. Khảo sát trước (bắt buộc)

Đọc: `AGENTS.md`, `v2/AGENTS.md`, `v2/agent/DECISIONS.md` (D2-190/205/221/222), `spec/abi.toml`
(`[object].Tcb`, `[method_signature."Tcb.Resume"]`, `[method_signature.state]`),
`v2/kernel/vifast-core/src/{invoke,core_tcb,syscall,tcbdata,tcb,state_write,error}.rs`,
`v2/kernel/vifast-plat-riscv64/src/{user,user_run}.rs`,
`v2/kernel/vifast-kbin-riscv64/src/{ktest_ipc,ktest_inv}.rs`,
`v2/kernel/vifast-kbin-riscv64/init/ipc_client_user.rs`, `v2/test/boot/{ktest.sh,expected.txt,selftest.sh}`,
`v2/agent/JOURNAL/kt311-cfgfull.md`.

| Câu hỏi | Trả lời | Bằng chứng |
|---|---|---|
| `Tcb.Resume` có trong `spec/abi.toml` không? Method id? | **CÓ** — `[object].Tcb` method **12** `Resume`; chữ ký `params = ["tcb"]`, `returns = []` | `spec/abi.toml:181`, `:266..268` |
| Hằng sinh tự động? | `vifast_abi::TCB_RESUME = 12` (sinh từ spec — D2-11) | `v2/lib/vifast-abi/src/generated.rs:269` |
| `TcbData.entry` (KT-311) đọc ở đâu? | `Machine::tcb_entry(tcb)` — **chỉ** rig/TAP đọc; **không** consumer nào trong đường chạy | `core_tcb.rs:302` (trước KT-314) |
| `TcbState` có `Ready`/`Running`/`Inactive`? | **CÓ** — enum 11 trạng thái §4.4 (D2-33) | `tcb.rs:147..365` |
| `Resume` chuyển trạng thái thế nào? | Trước KT-314: `Machine::tcb_resume` **chỉ** xoá `suspended` + `exit_code` (model API, không đổi trạng thái, không có `sepc`) | `core_tcb.rs:653..669` |
| Bảng chuyển trạng thái cho phép gì? | `Inactive → Running` **CẤM**; `Ready → Running` **cho phép** ⇒ `Inactive` phải qua `start_tcb` (`Inactive → Ready`) rồi mới `Running` | `tcb.rs:367..437`, `state_write.rs:284` |
| Đường chạy U-mode nhận `sepc` từ đâu? | `user_run::run_task(loaded, tf)` → `user::enter_loaded(loaded, tf)` → `user_enter(tf)` (asm `csrw sepc`). `tf.sepc` do **caller** (rig) đặt: lượt đầu `tf.sepc = loaded.entry` (từ ELF), các lượt sau `*tf = user::saved_tf()` | `user_run.rs:84`, `user.rs:287`, `user.rs:1408` |
| Core có ghi được `sepc` không? | **KHÔNG** — D2-83: `sepc` là CSR của arch; core chỉ **giao** giá trị qua hook `SyscallEnv` | `syscall.rs:181..` (trait), `user.rs:1147` (impl) |
| Có tiền lệ "core đặt cờ, arch tiêu thụ"? | **CÓ** — KT-282 `ipc_buffer::request_write(tcb)` / `flush_pending()` trong `enter_loaded` | `user_run.rs:32..56`, `user.rs:305` |
| `InvocationStats` có ai dựng literal ngoài `stats()`? | **CÓ** — `ktest_inv.rs:213` (delta của TAP 15..18) ⇒ thêm trường phải sửa cả đó | `ktest_inv.rs:213..231` |

⇒ **Kết luận khảo sát**: (a) **không** cần đổi ABI — `Tcb.Resume` đã có số method 12 và chữ ký
arity **0** (không MR vào) trong `spec/`; (b) `TcbData.entry` đã có (KT-311) nhưng **chưa** có
consumer ⇒ KT-314 là consumer; (c) `sepc` **không** thuộc core (D2-83) ⇒ phải có **hook arch**
mới, đúng khuôn `SyscallEnv::ipc_header`/`ipc_buffer_words`; (d) `Inactive → Running` không có
trong bảng ⇒ phải `start_tcb` trước; (e) **không** đổi `size_of::<Machine>()` (không thêm trường
`TcbData`) ⇒ **không** phải sửa bốn dòng evidence `bytes=`/`sum=`/`static=` trong `ktest.sh` và
phép tính pin trong `tls_tests.rs` — tránh thêm một điểm xung đột merge với KT-312.

## 2. Thiết kế

### 2.1 Ngữ nghĩa `Tcb.Resume` (invocation, method 12)

Thứ tự **bắt buộc** (lỗi ⇒ **không** đổi gì):

1. Cap phải là `Tcb` sống; cap phải có `RIGHT_WRITE` (thiếu ⇒ `InsufficientRights` **3**, cùng
   luật `Tcb.Configure`/`Tcb.SetTls` — D2-99(e)).
2. **Arity chính xác 0** (D2-104): `length` khác 0 ⇒ `InvalidArgument` **5**, có đếm `denied`.
3. **Trạng thái trước**: `Ready` ⇒ đi; `Inactive` **chưa từng chạy** (`exit_code = None`) ⇒
   `start_tcb` rồi đi; còn lại — `Running`, `BlockedOn*`, `Sleeping`, `Stopping`, `Dead`, và
   `Inactive` **đã chạy** (`exit_code = Some`) — ⇒ `IllegalOperation` **17**.
4. **`TcbData.entry`**: `0` ⇒ `InvalidArgument` **5** (fail-closed, không đoán điểm vào). Kiểm
   **sau** trạng thái ⇒ TCB `Running` + `entry = 0` trả **17** (không phải 5).
5. Chuyển `Running{hart của caller}` qua **một** cửa `transition_tcb` (bảng 11×11 vẫn là nguồn
   sự thật), xoá `exit_code` (D2-25).
6. **Giao `entry` cho arch** qua hook `SyscallEnv::tcb_resume(tcb, entry)` — core **không** ghi
   `sepc` (D2-83). Arch giữ giá trị; `user_run::run_task` nạp `tf.sepc` ở lần vào U-mode **kế
   tiếp** của **đúng** TCB đó (một lần — `take` xoá).
7. Trả `(a0, a1) = (0, entry)` — **cùng** số với `TcbData.entry`, không có đường "trả 0 giả".

**Vì sao `Inactive` + `exit_code = Some` bị từ chối** (`[TK]` KT-314): một tác vụ **đã kết thúc**
không được **tự chạy lại** từ một invocation của tác vụ khác — đó là bất biến fail-closed. Muốn
chạy lại thì `Tcb.Configure` trước (D2-25: `Configure` xoá `exit_code`) rồi `Resume`; cửa đó
**vẫn mở**, nên không mất khả năng nào của model. Ghi rõ vì `Machine::tcb_resume` (model API cũ)
**không** có luật này — nó chỉ xoá cờ; KT-314 **không** sửa `tcb_resume` (giữ nguyên 20+ điểm
gọi/rig cũ), chỉ thêm cửa **invocation** mới.

### 2.2 Kênh `sepc` (không đổi ABI, không đổi `Machine`)

```text
U-mode: Call(cap TCB đích, {label = Tcb.Resume, length = 0})
  → core: kiểm cap/quyền/arity/trạng thái/entry → transition Running → xoá exit_code
  → hook SyscallEnv::tcb_resume(tcb, entry)      [arch: user.rs → user_run::arm_resume_pc]
  → trả (0, entry)                                [U-mode: a0 = 0, a1 = entry]
  → scheduler chạy TCB đích: run_task() → apply_resume_pc(tf) → tf.sepc = entry → sret
```

Hằng/API mới: `invoke::TCB_RESUME` (= `vifast_abi::TCB_RESUME`), `Machine::tcb_resume_invocation`,
`SyscallEnv::tcb_resume` (mặc định `false` — mọi `SyscallEnv` cũ giữ nguyên hành vi),
`user_run::{arm_resume_pc, apply_resume_pc}`, `user::bound_core_tcb`.

## 3. Kết quả

| Hạng mục | Kết quả |
|---|---|
| Core: `invoke::TCB_RESUME` + whitelist + `resume()` (quyền/arity/hart của caller) | **DONE** |
| Core: `Machine::tcb_resume_invocation` (trạng thái → entry → `Running` → xoá `exit_code`) | **DONE** — nguyên tử, lỗi không đổi gì |
| Core: hook `SyscallEnv::tcb_resume` (mặc định `false`) | **DONE** — D2-83: core không ghi CSR |
| Arch: `user_run::{arm_resume_pc, apply_resume_pc}` + `run_task` nạp `sepc` | **DONE** — một cửa duy nhất |
| Arch: `user::bound_core_tcb` + impl hook trong `UserMemAccess` | **DONE** |
| Ảnh test: client U-mode `Resume` (4 ca) + 2 TCB đích + lượt 10 chạy đích | **DONE** |
| TAP mới `ok 7 - ipc: tcb resume` | **DONE** — ảnh `ktest-ipc` **7/7** TAP, **36** evidence |
| Runner: `expected.txt` 6→7, `manifest_ipc` 7, 5 evidence mới, mẫu PANIC mới, `selftest.sh` ca (q) | **DONE** |
| `cargo test -p vifast-core` | **XANH — 79/79** (9 test mới cho KT-314) |
| `size_of::<Machine>()` | **KHÔNG ĐỔI** (184.792) — không thêm trường `TcbData` |
| `./v2/bin/v2 check` (cây **sạch**) | **XANH** |
| Runner riscv64 `ktest.sh 1 matrix` (cây **sạch**, `KT54_LOGDIR` riêng) | **XANH — 27/27 boot sạch** |
| `selftest.sh` | **XANH — 25/25 ca** (ca (q) mới) |
| Ba đột biến | **ĐỎ đúng cả ba** (§6) |

## 4. Log QEMU nguyên văn (smp=1, ảnh `ktest-ipc`, HEAD `8fe7cf86`)

```text
TAP version 13
1..7
# ipc rig: stvec=0x802000a0 untyped=0 core_untyped=1 cspace_c=11 cspace_s=12 cspace_alt=13 tcb_c=3 tcb_s=4 ep=5 reply=6 ipc_buf=10
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
# ipc caps: sent=1 received=1 slot=5 rights=0xb obj=10 ty=Frame alive=1 copy_slot=6 copy_obj=10 copy_ty=Frame same=1
# ipc: cap sent=1 slot=5
# ipc: cap neg grant=3 over=5 ok=1
# ipc: configure2 ok=1 entry=0x40000112 prio=4
# ipc: configure2 neg align=6 prio=5 cspace=2 vspace=18 ok=1
# ipc: configure2 entry=0x40000112 prio=4 cspace=11 vspace=0 calls=2 denied=5 ok=1
# ipc: resume ok=1 entry=0x40001140
# ipc: resume neg noentry=5 running=17 exited=17 ok=1
# ipc: resume ok entry=0x40001140 state=Running
# ipc: resume neg noentry=5 state=Inactive
# ipc: resume ran entry=0x40001140
# ipc: neg empty=1 reply=18 ok=1
# ipc: neg mr over=5 ok=1
# sys: nr=14 err=0 value=0 tcb=3 state=Inactive src=core
# u-mode exit: code=0 faultkind=Exit{0} tcb=3 state=Inactive
# ipc buf: bound=1 va=0x40020000 mr_off=80 mr_words=60
ok 1 - ipc: umode call
ok 2 - ipc: server replied
ok 3 - ipc: mr4..mr7 via buffer
ok 4 - ipc: tcb configure via invocation
ok 5 - ipc: cap transfer
# ipc final: client_exit=0 server_exit=0 target_exit=0 reply_clean=1 check_ipc=1 check_caps=1 capture_client=1 capture_server=1 client=Inactive server=Inactive target=Inactive
ok 6 - ipc: tcb configure full
ok 7 - ipc: tcb resume
PANIC: KT-314 ipc u-mode + mr buffer + tcb configure + cap transfer + configure full + tcb resume xong (7 TAP, 0 fail)
```

**Đọc năm dòng `# ipc: resume`**:

| Dòng | Nguồn | Nghĩa |
|---|---|---|
| `# ipc: resume ok=1 entry=0x40001140` | **client U-mode** in | `Resume` trả `(0, entry)`; `entry` = địa chỉ `resume_entry` **thật** của ảnh (`resume_entry as usize`, không chép tay) |
| `# ipc: resume neg noentry=5 running=17 exited=17 ok=1` | **client U-mode** in | ba ca âm: `entry = 0` ⇒ **5**; chính ta `Running` ⇒ **17**; server đã `Exit` ⇒ **17** |
| `# ipc: resume ok entry=0x40001140 state=Running` | **kernel** đo **độc lập** | `TcbData.entry` **thật** của TCB đích + trạng thái **thật** sau `Resume` |
| `# ipc: resume neg noentry=5 state=Inactive` | **kernel** đo **độc lập** | gọi lại **cùng** cửa core trên TCB đích `entry = 0` ⇒ 5, trạng thái **không** đổi |
| `# ipc: resume ran entry=0x40001140` | **tác vụ đích** in | nó **chạy thật** ở `resume_entry` (không phải `_start`) với `tf.sepc = 0` ⇒ chỉ hook arch nạp được `sepc` từ `TcbData.entry` |

`smp=2/4` in **cùng** năm dòng (cùng số `entry`).

## 5. TAP + matrix

- Ảnh `ktest-ipc`: **7/7 TAP**, **36** evidence (thêm 5), PANIC mẫu mới.
- `expected.txt`: `ktest-ipc 7 7` (6 → 7); `manifest_ipc` thêm `ipc: tcb resume`.
- `ktest.sh 1 matrix` (HEAD `8fe7cf86`, `KT54_LOGDIR=/tmp/kt314-logs`): **27/27 boot sạch**
  (9 ảnh × smp 1/2/4), `ktest.sh: PASS`.
- `selftest.sh`: **25/25 ca**, gồm ca (q) mới — xoá dòng `# ipc: resume ok entry=0x` ⇒ runner
  **đỏ** đúng token `ev<ordinal>`.

## 6. Đột biến (ba ca, mỗi ca sửa **một** chỗ rồi build + boot thật + `--check-log`)

| # | Đột biến | Kết quả | Bằng chứng (log QEMU nguyên văn) |
|---|---|---|---|
| M1 | Bỏ kiểm **`entry == 0`** trong `tcb_resume_invocation` | **ĐỎ** | `# ipc: resume neg noentry err=0` (kỳ vọng **5**) ⇒ client `exit(107)` ⇒ `PANIC: u-mode fault scause=0xc stval=0x0 sepc=0x0` (đích không được resume) ⇒ `not ok - check-log … ok-count=0/7 … ev31 ev32 ev33 ev34` |
| M2 | Bỏ kiểm **trạng thái** (`Ready`/`Inactive`) — chỉ còn kiểm `entry` | **ĐỎ** | `# ipc: resume neg running err=5` (kỳ vọng **17** — kiểm trạng thái phải **đứng trước** kiểm `entry`) ⇒ client `exit(108)` ⇒ `not ok - check-log … ok-count=0/7 … ev33 ev34` |
| M3 | Bỏ kiểm **`Inactive` đã chạy** (`exit_code = Some`) | **ĐỎ** | `# ipc: resume neg exited err=0` (kỳ vọng **17**) ⇒ client `exit(109)` ⇒ `not ok - check-log … ok-count=0/7 … ev33 ev34` |

Mỗi ca: `git status` tệp đã chạm **nguyên vẹn** sau khôi phục (`cmp` với bản sao
`/tmp/kt314-core_tcb.orig`), và boot lại bản khôi phục ⇒ `ok - check-log ktest-ipc smp=1
(plan=7 ok=7, 36 evidence)`. Ba mã thoát `107/108/109` là ca âm của client — chứng minh **đúng**
ca âm bị phá, không phải đỏ vô tội vạ.

**Ghi chú M3**: client thoát ngay sau khi ca âm trả sai, nên log **không** có dòng
`# ipc: resume ran` thứ hai; nếu client tiếp tục, server (đã `Exit`, `entry` = `resume_entry` do
client cấu hình ở bước 1b) sẽ **chạy lại** ở `resume_entry` — đó chính là hành vi mà kiểm này
chặn. Đột biến vẫn **đỏ** vì mã lỗi đổi 17 → 0.

## 7. Còn gì chưa xong / rủi ro

1. **`Tcb.Suspend` (method 3) vẫn chưa cài** ở kênh invocation — ngoài phạm vi KT-314 (phiếu
   ghi rõ "Không làm"). `Resume` hiện chỉ nhận `Ready`/`Inactive`; một tác vụ `BlockedOn*` muốn
   resume phải đi qua đường `Reply`/`Notification` (đúng ngữ nghĩa IPC), **không** qua
   `Tcb.Resume` — nếu sau này cài `Suspend` thì cặp `Suspend`/`Resume` mới đủ (seL4).
2. **`Resume` preempt caller**: `transition_tcb(Running{hart của caller})` gọi
   `preempt_current` ⇒ caller (tác vụ giám sát) về `Ready` **ngay** trong syscall, đúng bất biến
   ≤1 `Running`/hart (KT-19 DoD 2). Ảnh test dựa vào đó (ba ca âm chạy **trước** đường xanh);
   một caller muốn tiếp tục chạy sau `Resume` phải được scheduler chọn lại — chưa có ảnh nào đo
   đường đó (ghi nợ, không phải lỗi).
3. **`Resume` trên hart khác**: `hart` lấy từ trạng thái **caller**; TCB đích có `home != hart`
   ⇒ `IllegalOperation` (đúng luật `write_state`). Chưa có ảnh đo `home` khác 0.
4. **`vspace?` vẫn chưa cài** (nợ KT-311 §7.1) — `Resume` **không** đụng VSpace; một tác vụ
   `Inactive` được resume chạy trong VSpace mà **arch** đang gắn cho ô của nó (`set_active_slot`),
   không phải VSpace của TCB (core không sở hữu — D2-83). Đây là giới hạn **đã biết** của mô
   hình một-VSpace-mỗi-ô; cần task riêng khi có `vspace?`.
5. **Chưa chạy T2 `gate --fast`** (task yêu cầu `v2 check` + runner riscv64; T2 là việc của lead
   trên ứng viên tích hợp).
6. **Không** tuyên bố K3 đạt lại — K3 đã đóng theo D2-205.

## 8. Xác nhận luật

- **Không** push, **không** merge, **không** chạm `main` (nhánh `task/kt314-resume`).
- **Không** sửa `v2/agent/{STATE,QUEUE,DECISIONS}.md`, **không** sửa `v2/bin/v2`, **không** sửa
  `spec/` (D2-04) — **không** cần đổi ABI: `Tcb.Resume` đã có method 12 + chữ ký arity 0 trong
  `spec/abi.toml`; kênh `sepc` là hook arch `[TK]`, `[method_signature."Tcb.Resume"]` **giữ
  nguyên**.
- **Không** nới ngưỡng (pin `196.608`, trần `.data+.bss` `262.144`, `size_of::<Machine>()`
  **không đổi**), **không** `#[ignore]`/retry, **không** `cfg(target_arch)` trong
  `vifast-core`/`vifast-plat-common`.
- Stage theo **đường dẫn cụ thể** (12 tệp + 2 tệp sửa), **không** `git add -A`.
- Cây sạch sau commit + sau ba đột biến.
- Commit: `9fdaf6ec` (mã + runner) và `8fe7cf86` (sửa build `ktest-user` + `entry` TCB server).
