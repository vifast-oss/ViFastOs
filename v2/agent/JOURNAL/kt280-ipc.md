# KT-280 — riscv64 **IPC từ U-mode** (mốc microkernel)

Trạng thái: **DONE** (nhánh `task/kt280-ipc`, base `main` @ `35938d8c`), 2026-11-21.
Sub-agent `ipc`. Quyết định: **D2-190** (hợp đồng K3), **D2-205**, **D2-210**, **D2-198** (uỷ quyền
commit trên nhánh task), **D2-05** (test đỏ trước), **D2-109/D2-111** (tách ảnh khi thêm đường mới).

**Không** tuyên bố K3 đạt lại: K3 đã đóng theo D2-205; KT-280 chỉ thêm **một đường mới** (IPC
blocking từ U-mode) và **không** mở lại bất kỳ mục nào của hợp đồng K3.

## 1. Khảo sát trước (bắt buộc) — riscv64 đã có gì?

Đọc: `v2/kernel/vifast-core/src/{ipc,core_ipc,syscall,machine,state_write,core_sched}.rs`,
`v2/kernel/vifast-kbin-riscv64/src/{main,ktest_user,ktest_proc,proc_demo,ktest_capx}.rs`,
`v2/kernel/vifast-plat-riscv64/src/{user,user_run,core_bridge,ipc_buffer}.rs`,
`v2/kernel/vifast-plat-user/lib.rs`, `v2/docs/k3-riscv64.md` §16, `v2/docs/k35-procsrv.md`.

| Câu hỏi | Trả lời | Bằng chứng |
|---|---|---|
| `vifast-core` có `Tcb`/`Endpoint`/`Call`/`Send`/`Recv`? | **CÓ, đầy đủ** — `Machine::{call_msg,send,nb_send,recv,nb_recv,reply_msg,reply_recv,nb_send_recv,nb_send_wait}`, `Endpoint`/`Reply`/`Notification`, `BlockedOnSend{call}`→`BlockedOnReply`→`Ready`, `Reply.caller` liên kết ở `write_state` (KT-34) | `core_ipc.rs:381..760`, `state_write.rs:150..200` |
| `vifast-kbin-riscv64` có U-mode? | **CÓ** (KT-53): `sret` `sstatus.SPP=0`, `sscratch` trap stack per-hart, `ecall` `scause=8`; **KT-62** đưa **cả 16 syscall** `0..15` vào `Machine` thật qua `vifast_core::syscall::dispatch` | `plat-riscv64/src/user.rs:842..1010`, `core/syscall.rs:388` |
| `procsrv`/scheduler chạy được 2 task? | **CÓ** — `enable_multitask(true)` + `run_task`/`Stop`; `ktest-proc` chạy **3** tác vụ U-mode (procsrv/shell/child) round-robin, `ktest_capx` chạy **2** (memsrv/client), `ktest_vfs` chạy **4** (pager + 3 client) | `proc_demo.rs:625..701`, `ktest_capx.rs:100..120` |
| **IPC blocking từ U-mode đã có chưa?** | **CHƯA** — đây là lỗ hổng thật | xem dưới |

### 1.1 Lỗ hổng chính xác (vì sao "K3 có IPC nhưng chưa từ U-mode")

1. **`Call` trên Endpoint từ U-mode chưa từng chạy.** Mọi `call()` trong chương trình U-mode
   (`init/*.rs`) đều nhắm cap **không phải Endpoint** (invocation: `Frame.Map`, `Untyped.Retype`,
   `Tcb.SetTls`, `CNode.*`) ⇒ trả về **ngay**, không chặn. TAP 9 của `ktest-user`
   (`syscall ipc vong`) gọi `Call/Recv/Send` **từ kernel** (gọi thẳng `dispatch`), không phải từ
   U-mode. IPC U-mode hiện có (`shell`↔kernel FS, `procsrv`↔kernel) là **`NBSend`+`NBRecv` polling**
   (`shell_user.rs:711` `fs_call`: `send` + vòng `recv` + `yield` có ngân sách).
2. **Đường "tác vụ chặn" bị coi là kết thúc.** `on_u_exception` nhánh `SyscallAction::Switched`:
   - `MULTITASK == 0` ⇒ `PANIC: u-mode syscall khong the quay lai U-mode`;
   - `MULTITASK != 0` ⇒ `stop(3, 0)` ⇒ `Stop::Blocked`, và scheduler demo (`proc_demo.rs:673`)
     đánh dấu tác vụ **`exited = true`**.
3. **Không có đường resume sau syscall chặn.** Trapframe được lưu với `sepc` **trỏ vào `ecall`**
   (chưa `+4`), và không có chỗ nào nạp `a0..a6` từ vùng nhận IPC của TCB khi resume ⇒ nếu có ai
   resume, tác vụ sẽ **chạy lại `ecall`** thay vì nhận kết quả.

⇒ Phạm vi KT-280 = **đúng ba việc**: (a) `sepc += 4` khi tác vụ chặn trong đa nhiệm; (b) nạp
`a0..a6` từ `tcb_ipc_area` khi resume; (c) một scheduler giữ tác vụ đang chặn sống và resume nó.
**Không** hạ phạm vi xuống "U-mode chạy được + một syscall" — U-mode đã có từ KT-53.

## 2. Thiết kế

Ảnh **mới** `vifast-kbin-ktest-ipc` (feature `ipc = ["ktest","user"]`, bin riêng) theo
D2-109/D2-111 (thêm đường mới ⇒ tách ảnh, không nới trần, `.text`/TAP của 6 ảnh cũ **không đổi**).

- Hai chương trình U-mode **riêng**, hai VSpace, hai cspace, hai TCB:
  - `ipc_client_user.rs`: `Call(EP, MessageInfo{label=REQ,length=1}, MR0=0x1234)` ⇒ chặn; nhận
    reply ⇒ in `# ipc: umode call ok reply=0x…`; rồi **hai ca âm** (slot rỗng ⇒ 1; `Call` trên cap
    `Reply` ⇒ 18) ⇒ in `# ipc: neg …`; `Exit(0)`.
  - `ipc_server_user.rs`: `Recv(EP, REPLY_SLOT)` ⇒ nhận message ⇒ `Send(REPLY_SLOT,
    MessageInfo{label=REPLY,length=2}, MR0=0x5a5a, MR1=nr)` ⇒ in `# ipc: server ran nr=1` ⇒ `Exit(0)`.
- Kernel: rig CNode/Endpoint/Reply/2 TCB/2 SC; scheduler 3 lượt tất định (client → server → client);
  TAP 1 = `ipc: umode call`, TAP 2 = `ipc: server replied`; kết thúc bằng **một** `PANIC` có chủ đích.

## 3. Kết quả

| Việc | Kết quả |
|---|---|
| `plat-riscv64/src/user.rs` — `sepc += 4` khi tác vụ chặn (nhánh `Switched`, đa nhiệm) | **DONE** (+7 dòng, 1.490 dòng — dưới R1) |
| `plat-riscv64/src/user_run.rs` — `apply_ipc_return` (nạp `a0..a6` từ `tcb_ipc_area`) | **DONE** (+35 dòng; tách khỏi `user.rs` để giữ R1) |
| `plat-user/lib.rs` — `SYS_SEND` (1) + `send()` cho server U-mode | **DONE** (+9 dòng) |
| Ảnh mới `vifast-kbin-ktest-ipc` (feature `ipc = ["ktest","user"]`) | **DONE** — 2 TAP, `.data+.bss` = **209.784 B** / 262.144 (headroom 52.360 B) |
| Hai chương trình U-mode `ipc_client_user.rs` / `ipc_server_user.rs` | **DONE** — 8.912 B / 8.896 B ELF |
| Runner: `expected.txt` + `ktest.sh` (LABELS/image_path/manifest/evidence/panic/build) | **DONE** — matrix **8 ảnh × smp 1/2/4 = 24 boot** |
| Runner: ca âm thường trực cho dòng `# ipc:` (`selftest.sh` ca 18) | **DONE** — 19/19 ca, đỏ đúng token `ev6` |
| `./v2/bin/v2 check` | **XANH** (exit 0, 113 s) |
| `./v2/bin/v2 gate --dev` (T1) | **PASS** (476,8 s) |
| Runner riscv64 `ktest.sh 1 matrix` | **XANH — 24/24 boot sạch** |
| Ba đột biến | **ĐỎ đúng cả ba** (§6) |

Commit: **`2f842f69`** — `[KT-280] IPC từ U-mode riscv64: client Call chặn → server U-mode
Recv/Reply → client resume (TAP 2/2, ảnh ktest-ipc thứ tám)` (11 tệp, +951/−3).

## 4. Log QEMU nguyên văn (smp=1, ảnh `ktest-ipc`, commit `2f842f69`)

```text
# ipc rig: stvec=0x802000a0 untyped=0 core_untyped=1 cspace_c=10 cspace_s=11 tcb_c=3 tcb_s=4 ep=5 reply=6
# ipc call: blocked=1 state=BlockedOnSend call=1 ep_len=1 ep_mr=0x1234 label=0x280 stop=Blocked
# ipc server: recv=1 label=0x280 mr=0x1234 client=BlockedOnReply caller=3 server=Running stop=Yield
# ipc: server ran nr=1
# ipc reply: server_exit=0 client=Ready reply_caller=0 server=Inactive stop=Exit
# ipc: umode call ok reply=23130
# ipc: neg empty=1 reply=18 ok=1
# ipc client: reply_label=0x281 reply_mr0=0x5a5a reply_mr1=1 const:label_reply=0x281 const:reply_value=0x5a5a
ok 1 - ipc: umode call
ok 2 - ipc: server replied
# ipc final: client_exit=0 server_exit=0 reply_clean=1 check_ipc=1 check_caps=1 capture_client=1 capture_server=1 client=Inactive server=Inactive
PANIC: KT-280 ipc u-mode xong (2 TAP, 0 fail)
```

Đọc dòng theo dòng (mỗi dòng là một phép đo, không phải lời khai):

1. `# ipc call:` — client U-mode gọi `Call` trên Endpoint **khi chưa có người nhận** ⇒ core đưa
   TCB sang `BlockedOnSend{call:true}` (đúng seL4: chặn ở **Send**), message **nằm trong hàng
   Endpoint** (`ep_len=1`, `ep_mr=0x1234`). `stop=Blocked` = nhánh `Switched` của
   `on_u_exception` chạy thật (trước KT-280 nhánh này là lưới an toàn chưa từng dùng).
2. `# ipc server:` — server U-mode `Recv` lấy message: caller chuyển
   `BlockedOnSend{call}` → **`BlockedOnReply`**, `Reply.caller = 3` (TCB client), message server
   nhận đúng `label=0x280 mr=0x1234`. `stop=Yield` = server nhường CPU **giữa** `Recv` và `Reply`
   để kernel quan sát được trạng thái trung gian này (bằng chứng trực tiếp, không suy luận).
3. `# ipc: server ran nr=1` — **chính chương trình U-mode** in (qua `DebugPutStr`), không phải
   kernel tự khai.
4. `# ipc reply:` — server `Send` trên cap `Reply` ⇒ client `Ready`, `Reply.caller` **sạch**
   (0), server `Inactive` (Exit 0).
5. `# ipc: umode call ok reply=23130` — **chính client U-mode** in sau khi **được resume**:
   `a0=0`, `a2` = `MessageInfo{label=0x281,length=2}`, `a3=0x5a5a` (= 23130), `a4=1`.
6. `# ipc: neg empty=1 reply=18 ok=1` — hai ca âm trong cùng boot: `Call` trên ô rỗng ⇒ **1**
   (`InvalidCapability`), `Call` trên cap `Reply` (không phải Endpoint) ⇒ **18** (`NotSupported`).
7. `# ipc client:` — kernel đọc **độc lập** vùng nhận IPC của client: `reply_label=0x281
   reply_mr0=0x5a5a reply_mr1=1` (không tin lời chương trình).
8. `# ipc final:` — `check_ipc`/`check_caps` xanh, `capture_client=1`/`capture_server=1` (output
   của **cả hai** chương trình U-mode được chụp đúng ô), hai TCB `Inactive`.

## 5. TAP + matrix

```text
$ bash v2/test/boot/ktest.sh 1 matrix
# expected[ktest-ipc]: plan=2 ok=2 ten-TAP=2
...
ok - ktest-ipc smp=1 boot 1 (2 TAP, 0s) log /tmp/kt280-ktest/ktest-ipc-smp1-1.log
ok - ktest-ipc smp=2 boot 1 (2 TAP, 0s) log /tmp/kt280-ktest/ktest-ipc-smp2-1.log
ok - ktest-ipc smp=4 boot 1 (2 TAP, 0s) log /tmp/kt280-ktest/ktest-ipc-smp4-1.log
# KTEST: 24/24 boot sạch (smp=matrix, NBOOT=1, ảnh/boot=ktest ktest-guard ktest-user ktest-proc ktest-vblk ktest-con ktest-exec ktest-ipc)
# ktest.sh: PASS
```

**Ca âm thường trực** trong `v2/test/boot/selftest.sh` (ca 18, thêm bởi KT-280): xoá đúng dòng
`# ipc: umode call ok reply=…` khỏi log `ktest-ipc` ⇒ runner **phải đỏ** với token `ev6` (ordinal
tra từ `--print-evidence`, không hardcode). Chạy thật:

```text
ok 18 - log ktest-ipc: xoá dòng '# ipc: umode call ok reply=' (evidence bắt buộc của KT-280)
#   runner đỏ vì [ev6]: not ok - check-log ktest-ipc smp=1: thiếu ev6, log …/log-ipc-no-call.log
# selftest: 19/19 ca đúng như kỳ vọng
# selftest.sh: PASS
```

`--check-log` (11 evidence bắt buộc):

```text
$ bash v2/test/boot/ktest.sh --check-log ktest-ipc /tmp/kt280-final.log 1
ok - check-log ktest-ipc smp=1 (plan=2 ok=2, 11 evidence) log /tmp/kt280-final.log
# check-log: PASS
```

## 6. Đột biến (đỏ có kiểm soát) — cả ba ĐỎ đúng token

Script: `/tmp/kt280-mutations.sh` (sửa **một** chỗ → build → boot thật qua `qemu_guard.sh` →
`ktest.sh --check-log` phải đỏ → `git checkout --` trả tệp). Cây sạch sau cả ba.

| # | Đột biến | Kết quả log | Runner |
|---|---|---|---|
| M1 | server **không** `Reply` (bỏ `Send` trên cap Reply) | `# ipc reply: … client=BlockedOnReply reply_caller=3`; `applied=0`; `not ok 2`; `PANIC: KT-280 ipc u-mode CO LOI` — **không treo**, kernel phát hiện và dừng có chẩn đoán | **ĐỎ**: `thiếu TAP #2`, `thiếu evidence#5..#7` |
| M2 | endpoint **sai** (`SLOT_EP = 9`, ô rỗng) | `# ipc: client call err=1`; `blocked=0`; `not ok 1` + `not ok 2` | **ĐỎ**: `thiếu TAP #1/#2`, `thiếu evidence#2/#3` |
| M3 | `Call` **không cap** mà đòi **không lỗi** (`empty[0] != 0` thay vì `!= 1`) | `client_exit=84`; `not ok 2` | **ĐỎ**: `thiếu TAP #2`, `thiếu evidence#7/#9` |

M1 là ca "server không Reply" theo yêu cầu: thay vì treo 20 s, ảnh **phát hiện** client còn
`BlockedOnReply` (`apply_ipc_return` trả `false`) và in `not ok 2` + `PANIC … CO LOI` — đỏ có
kiểm soát, có chẩn đoán, không treo máy.

## 7. Còn gì chưa xong / rủi ro

1. **Không** làm (đúng phạm vi task): net server, nhiều client, preemption giữa hai tác vụ
   (scheduler của ảnh là **tuần tự tất định** 4 lượt, không phải scheduler ưu tiên đầy đủ).
2. **Không** tuyên bố K3 đạt lại — K3 đã đóng theo D2-205; KT-280 chỉ thêm một đường mới.
3. **Nợ**: `apply_ipc_return` chỉ nạp 4 MR thanh ghi (`a3..a6`); MR4..MR63 qua IPC buffer vẫn là
   nợ K3.5 đã ghi ở đầu `vifast-core/src/syscall.rs` (không đổi trong task này).
4. **Nợ**: `Stop::Blocked(TcbState::Inactive)` vẫn mang `Inactive` (bảng mã `decode` của
   `user_run.rs` không đổi) — ảnh KT-280 đọc trạng thái **thật** từ core
   (`Machine::tcb_state`) thay vì tin `Stop`; sửa `decode` là việc của task khác (chạm mọi ảnh).
5. **Rủi ro thấp**: `sepc += 4` chỉ chạy ở nhánh `Switched` **khi `MULTITASK != 0`**; đã kiểm
   **tất cả** ảnh đang dùng `enable_multitask(true)` (`ktest-proc`, `ktest-user`/`ktest_capx`,
   `ktest_capa`, `ktest_ring`, `ktest_vfs`, `ktest-exec`, `proc_rt`) — **không** ảnh nào resume
   tác vụ `Stop::Blocked` (chúng coi đó là lỗi/kết thúc), nên `sepc` đổi không ảnh hưởng; matrix
   24/24 xác nhận.
6. **Chưa chạy T2 `gate --fast`** (task yêu cầu `v2 check` + runner riscv64; T2 là việc của lead
   trên ứng viên tích hợp). Đã chạy **T1 `gate --dev` = PASS** (476,8 s) — T1 **không** phải full
   gate.

## 8. Xác nhận luật

- **Không** push, **không** merge, **không** chạm `main` (nhánh `task/kt280-ipc`, commit
  `2f842f69`).
- **Không** sửa `v2/agent/{STATE,QUEUE,DECISIONS}.md`, **không** sửa `v2/bin/v2`, **không** sửa
  `spec/`.
- **Không** nới ngưỡng, **không** `#[ignore]`/retry, **không** `cfg(target_arch)` trong
  `vifast-core`/`vifast-plat-common` (không chạm hai crate đó).
- Stage theo **đường dẫn cụ thể** (11 tệp), **không** `git add -A`.
- Cây sạch sau commit + sau ba đột biến (`git status --short` rỗng).
