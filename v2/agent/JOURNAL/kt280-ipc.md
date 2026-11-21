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

(xem §4..§8 bên dưới — điền sau khi đo)
