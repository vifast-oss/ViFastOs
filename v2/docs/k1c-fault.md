<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# Fault, Exit và mô hình procsrv — KT-26 (model-only `[TK]`)

**Trạng thái:** REVIEW (Agent 2 dòng OS soạn cho Agent 0/reviewer độc lập). **Không phải ABI.**
Nguồn sự thật của ABI vẫn là `spec/abi.toml` (D2-11); tệp này chỉ ghi **quy ước mô hình** đã dùng
trong `v2/sim/vifast-sim` cho **KT-26** (fault + Exit + mô hình procsrv), để KT-27/28/29 không phải
suy lại.

**Nguồn ngữ nghĩa:** thiết kế §4.6 `:321` (`Fault: Call tới fault endpoint với Reply; trả lời Reply
để tiếp tục, có thể kèm thanh ghi mới`), §4.4 `:279` (không ô `resume`; message ghi thẳng vào
context + IPC buffer rồi mới `Ready` — chống đường mất message r319), §4.4 `:285` + D2-25 (`Exit`),
§6.4 `:585-600` (`wait`/`kill`), **D2-53 Q5** (`FaultKind` VM/unknown/user **model-only**; spec là
**tập con** tới K3), **Q10** (procsrv chỉ mô hình; shell K3.5), **D2-55** (không đổi ABI),
K1C-PLAN §5 KT-26 (DoD 1..8).

## 1. `FaultKind` — 4 biến thể, spec vẫn chỉ có `Exit`

```rust
pub enum FaultKind {              // v2/sim/vifast-sim/src/tcb.rs — model-only [TK]
    Exit { code: u64 },
    Vm { fault_addr: u64, access: FaultAccess },   // FaultAccess::{Read, Write, Execute}
    UnknownSyscall { nr: u64 },
    UserException { ip: u64, kind: u64 },
}
```

| Hạng mục | Giá trị | Nguồn |
|---|---|---|
| `spec/abi.toml` `[method_signature.fault_kind]` | **chỉ** `Exit { code: u64 }` | `spec/abi.toml:268-271` |
| Mô hình K1c | `Exit` + 3 biến thể **model-only** | D2-53 **Q5**(a), thiết kế `:321` |
| `name()` | `"Exit"`, `"Vm"`, `"UnknownSyscall"`, `"UserException"` | ổn định cho trace/`expect_states` |
| `render()` | `Exit{7}`, `Vm{fault_addr=…, access=Write}`, `UnknownSyscall{nr=…}`, `UserException{ip=…, kind=…}` | thay `Debug` trong `TcbState::render` |
| `is_exit()` / `exit_code()` | chỉ `Exit` là đường **Send, không Reply** (D2-25); `exit_code()` phục vụ procsrv | D2-25 |

**Không** thêm gì vào `spec/`: spec là **tập con** tới K3 (K3 conformance sẽ chốt normative — nếu
Agent 0 muốn normative thì là một D2 mới sửa `spec/abi.toml`, việc của Agent 0). Gen-check không
đổi vì `vifast-abi` sinh từ đúng `spec/`.

## 2. Fault ≠ Exit: **Call + Reply**, ghi thanh ghi **trước** `Ready`

### 2.1 Đường đi

`Machine::inject_fault(tcb, kind)` (**model API `[TK]`**, không phải syscall/ABI) mô phỏng trap khi
TCB `Ready`/`Running`:

- `kind == Exit` ⇒ `InvalidArgument` (dùng syscall `exit` — D2-25; **một** đường cho Exit).
- Không có `fault_ep` ⇒ `InvalidArgument`, **không** đổi trạng thái (K1c chỉ mô hình fault **có**
  handler; fault không handler để K3 chốt — ghi rõ để không tự bịa "thread chết").
- Cap `fault_ep` chết ⇒ `InvalidCapability` (cùng khuôn `exit`; trạng thái không đổi).
- Hàng gửi của `fault_ep` đầy **và** không có `Recv` waiter ⇒ `Busy`, **trước** mọi thay đổi
  (không giao nửa chừng, §4.17).
- Có `Recv` waiter (procsrv đang chờ) ⇒ giao **thẳng** như đường nhanh của `call`/`exit`:
  TCB ⇒ `BlockedOnSend{ep: fault_ep, call: true, fault: Some(kind)}` **trước** khi giao (mọi
  chuyển trạng thái bên trong nằm trong bảng `ALLOWED`: `3→5` Reply, `3→1` Cancelled), waiter
  không cấp Reply cap ⇒ caller `Ready + last_ipc_error = Cancelled` (không treo vĩnh viễn — cùng
  khuôn D2-53 Q7(b)).
- Không có waiter ⇒ message fault (`msg: None`, **không** chiếm `msg_id`/`ipc_sent`) vào hàng gửi;
  TCB ⇒ `BlockedOnSend{ep, call: true, fault: Some(kind)}`.

Khi procsrv `Recv` (có Reply cap): message fault được giao **thẳng** cho vùng nhận của procsrv
(không có MR), `last_fault = Some((sender, kind))` được ghi trên TCB procsrv, caller ⇒
`BlockedOnReply{reply}` + SC donation (KT-21, nếu có). procsrv `Reply`/`ReplyRecv` với message mới:
`ipc::transfer` chép thanh ghi/MR vào TCB caller **trước** `transition_tcb(caller, Ready)` — không
có cửa sổ "Ready nhưng message chưa tới" (đường r319 của v1).

### 2.2 `tcb_resume_with_regs` — một cửa cho fault-resume

`Machine::tcb_resume_with_regs(tcb: SlotRef, msg: Message)` (**model API `[TK]`**):

- Nhận `BlockedOnReply{reply}` (fault đã giao; `Reply.caller` phải đúng TCB), hoặc
  `BlockedOnSend{call:true, fault:Some(k≠Exit)}` (fault còn trong hàng — **huỷ** message fault đang
  chờ, không phát lại IPC cũ), hoặc `Ready`/`Sleeping` (kiểu `Tcb.Resume` + thanh ghi).
- Ghi `msg` qua **cùng** `ipc::transfer` (một message thanh ghi đi qua `admit_message` +
  `deliver_message` ⇒ `ipc_sent`/`ipc_delivered` cùng tăng, `check_ipc` giữ nguyên) **trước** khi
  đổi trạng thái; chỉ về `Ready` (hoặc **giữ** `Sleeping{reason}` — bảng §4.5 `:309-310`) sau khi
  thanh ghi đã nằm trong TCB.
- **Không** phát lại Call/Send/Recv/Reply cũ; TCB về **đúng priority** (ready queue bucket theo
  `priority`/donation của nó — KT-19/21).
- **Không** đụng `exit_code` (khác `Tcb.Resume` của KT-18/25 — D2-25 chỉ buộc `Tcb.Resume`/
  `Configure` xoá `exit_code`; đường fault-resume phải giữ mã để procsrv còn đọc: nhánh
  `Suspend(Abort)` huỷ message Exit nhưng `ReadState.exit_code` vẫn đọc được).
- Message **không** mang cap (`InvalidArgument` nếu có): chuyển cap thuộc Call/Reply (D2-53 Q6).

`Machine::tcb_regs` là alias của `tcb_ipc_regs` (4 MR "thanh ghi") cho đường fault-resume;
`tcb_last_fault`/`take_last_fault` cho procsrv đọc `(sender, FaultKind)` vừa giao (message fault
không có MR nên đây là đường duy nhất).

## 3. `Exit` — D2-25, kiểm bằng 4 test

| Nhánh | Hành vi | Test |
|---|---|---|
| Có `fault_ep` | TCB ⇒ `BlockedOnSend{ep: fault_ep, call: false, fault: Some(Exit{code})}`, message fault vào hàng gửi (không Reply, không `msg_id`) | `exit_co_fault_ep_thanh_blocked_on_send_call_false` |
| procsrv `Recv` xong | TCB ⇒ `Inactive`; `exit_code` vẫn đọc được | `exit_procsrv_recv_xong_moi_inactive` |
| Không `fault_ep` | `Inactive` **ngay**, `exit_code` giữ | `exit_khong_fault_ep_inactive_ngay` |
| `Suspend(Abort)` khi đang chờ giao Exit | message Exit bị **huỷ** (`ipc_dropped` không tăng vì đây là fault message), TCB `Ready + suspended`, `exit_code` **giữ** | `exit_suspend_abort_huy_message_giu_code` |

`Exit` **không** bao giờ `call = true`; fault ≠ Exit **phải** `call = true` — `check_mailboxes`
kiểm **cả hai chiều** sau mỗi public op (không nới bảng `ALLOWED` 11×11).

## 4. Mô hình procsrv `[TK]` — `{code, how}` (Q10, DoD 5)

`v2/sim/vifast-sim/src/procsrv_model.rs` — **không phải procsrv thật** (procsrv + shell là K3.5):
bảng tiến trình `track(tcb, cap)`, một `fault_ep`, một Reply window.

```rust
pub enum WaitHow { Exited, Cancelled, Faulted, Killed }
pub struct WaitResult { pub code: u64, pub how: WaitHow }
```

| Hàm | Ngữ nghĩa | Nguồn |
|---|---|---|
| `recv_exit(&mut Machine)` | `Recv` trên `fault_ep` (có Reply) rồi gom `(tcb, code)` của fault `Exit`; fault ≠ Exit ghi lịch sử, trả `None`; fault đã giao **trước** lời gọi được tiêu thụ qua `take_last_fault` (không `Recv` lần hai) | §6.4, D2-25 |
| `wait(tcb)` | `{code, how}`: `Exited` khi `exit_code` có + TCB `Inactive` (exit có/không `fault_ep`), `Cancelled` khi `Suspend(Abort)` huỷ message Exit (`exit_code` vẫn đọc), `Faulted` khi fault ≠ Exit chưa sửa (`BlockedOnReply`), `Killed` sau `kill`; còn chạy/chờ ⇒ `Busy` | §6.4 `:591`, Q10 |
| `kill(tcb)` | `Tcb.Suspend(Abort)` + `CNode.Revoke` cap TCB (chỉ mô hình; `Delete`+finalise là K3.5), ghi `Killed`; message Exit đang chờ bị huỷ nhưng `exit_code` giữ | §6.4 `:592`, Q10 |
| `resume_fault(tcb, regs)` | cho tiến trình bị fault chạy tiếp qua `tcb_resume_with_regs` (không phát lại IPC) | §4.4 `:279`, §6.4 `Cont` |

**Không** có shell/user binary, không ELF, không cấp `SchedContext` cho tiến trình con trong K1c.

## 5. `.scn` 13/14/20 chạy qua runner

`tests/kt26_scn.rs` đọc (không sửa) ba tệp và chạy:

- `13-exit-no-fault-ep.scn`: `Tcb.Configure` + `exit code=0 => halt` ⇒ TCB `Inactive` ngay,
  `exit_code = 0`.
- `14-exit-with-fault-ep.scn`: `Tcb.SetFaultEp` + `exit code=3 => halt` ⇒ `BlockedOnSend`; procsrv
  `Recv` `ok` ⇒ `Inactive`, `exit_code = 3`.
- `20-exit-procsrv-chua-recv.scn`: `expect_states = ["BlockedOnSend{ep, call=false, fault=Exit{7}}"]`
  kiểm **ngay sau `exit`, trước `Recv`** (D2-41) — khớp cả tên trạng thái và trường.

Quy ước runner (cùng `kt24_scn.rs`/`kt25_scn.rs`, D2-54): `ep0` (fault) và `ep1` (procsrv Recv)
trỏ **cùng** một Endpoint; `recv` không nêu `reply=` ⇒ cấp `rep0`; `exit ... => halt` ánh xạ
`Ok(())` → `halt`.

## 6. Bằng chứng và giới hạn

| Hạng mục | Bằng chứng |
|---|---|
| Test đỏ trước | `JOURNAL/kt26-red-compile.log` (53 lỗi: 51×`E0599` + 2×`E0432` cho `inject_fault`/`FaultKind` mới/`tcb_resume_with_regs`/`tcb_regs`/`FaultAccess`/`procsrv_model`), `JOURNAL/kt26-red-semantics.log` (probe tạm: `FaultKind` chỉ `Exit`, fault không có đường Call+Reply, `tcb_resume` xoá `exit_code` và không ghi thanh ghi) |
| Đột biến "resume không ghi MR" | `JOURNAL/kt26-mutation-resume.log` (43 dòng, đủ tên test) — bỏ `deliver_message` trong `tcb_resume_with_regs` ⇒ **3 test resume** đỏ đúng chỗ `left: [0,0,0,0]` (khôi phục nguyên trạng, `diff` rỗng) |
| `gate --fast` | PASS **386 test/53 suite** (+25: 24 `kt26_fault` + 1 `kt26_scn`), L1 1.000 seed/999 trace 713 ms (`JOURNAL/kt26-gate-fast.log`) |
| Miri | `--test kt26_fault` **PASS 24/24** (`JOURNAL/kt26-miri.log`); fuzz tự giảm còn 3 seed × 8 op dưới `cfg(miri)`, stable 1.000 × 64 |
| Fuzz | 1.000 seed × 64 op trộn `inject_fault`/`exit`/`recv_exit`/`resume_with_regs`/`suspend`/`resume`/`kill`/kích hoạt lại; `check_sched`/`check_mailboxes`/`check_ipc`/`check_caps`/`check_all_bytes` sau **mỗi** bước; cùng seed ⇒ cùng trace |
| Ba nhánh phụ | `procsrv_kill_revoke_xoa_cap_dan_xuat` (Revoke xoá cap dẫn xuất), `resume_with_regs_giu_sleeping_va_ghi_thanh_ghi` (nhánh `Sleeping`), `fault_donation_di_theo_call_va_tra_lai_khi_reply` (donation trên đường fault với procsrv **passive**) |

**Giới hạn của từng invariant (không tuyên bố quá):** `check_ipc` chỉ chứng minh bảo toàn message
**có `msg_id`** (message người dùng: `ipc_sent == delivered + pending + dropped`, mỗi `msg_id` một
nơi). Fault **không** có `msg_id` nên chiều *sender ↔ hàng gửi* của fault do **`check_mailboxes`**
chứng minh (message fault còn trong hàng ⇔ sender đúng `BlockedOnSend{ep, call, fault}`; fault
`Exit` cấm `call=true`, fault khác **bắt buộc** `call=true`), không phải `check_ipc`. Test đỏ của
đường "resume không ghi MR" là đột biến `deliver_message` (xem dòng dưới), không dựa vào
`check_ipc`.

**Giới hạn mô hình (ghi để K3 không hiểu nhầm):**

- Fault không `fault_ep` **chưa** được mô hình (K1c trả `InvalidArgument`); K3 chốt "thread chết
  hay chờ handler".
- procsrv là **mô hình**, không có shell, không ELF, không `SchedContext` riêng; `kill` chỉ
  Revoke cap (không finalise object).
- `tcb_resume_with_regs` **giữ** `exit_code` (khác `Tcb.Resume`); nếu K3 muốn `Resume` xoá cả mã
  trên đường fault thì phải sửa D2-25 trước, không sửa test.
- Thất bại chuyển cap (D2-53 Q6) **không** áp dụng cho message thanh ghi của fault-resume (message
  không mang cap).
- `FaultKind` VM/unknown/user **chưa** vào spec; K3 conformance là nơi chốt normative.
