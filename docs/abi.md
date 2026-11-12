<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# ABI v2 — tài liệu sinh tự động

SINH TỰ ĐỘNG bởi `v2/tools/vifast-gen` từ `spec/abi.toml` — **đừng sửa tay** (`gen-check` sẽ bắt).
`status = 'APPROVED'` · `abi_version = 1`

## Syscall (0..15)

| # | Tên | Tham số | Trả |
|---|---|---|---|
| 0 | `Call` | cap, MessageInfo, MR*, cap* | error, value |
| 1 | `Send` | cap, MessageInfo, MR*, cap* | error, value |
| 2 | `NBSend` | cap, MessageInfo, MR*, cap* | error, value |
| 3 | `Recv` | cap, reply_cptr? | error, value, MessageInfo, MR*, cap* |
| 4 | `NBRecv` | cap | error, value, MessageInfo, MR*, cap* |
| 5 | `ReplyRecv` | reply_cap, cap, MessageInfo, MR* | error, value, MessageInfo, MR* |
| 6 | `NBSendRecv` | send_cap, recv_cap, reply_cap, MessageInfo, MR* | error, value, MessageInfo, MR* |
| 7 | `NBSendWait` | send_cap, recv_cap, MessageInfo, MR* | error, value, MessageInfo, MR* |
| 8 | `Yield` |  | error, value |
| 9 | `WaitAddr` | mrs: addr, expected | error, value |
| 10 | `WakeAddr` | mrs: addr, n | error, value |
| 11 | `ClockRead` |  | error, value(ns) |
| 12 | `DebugPutStr` | user ptr, len | error, value |
| 13 | `DebugHalt` | code | không trả |
| 14 | `Exit` | code | không trả |
| 15 | `DebugGetInfo` |  | error, abi_version, min_compat, features, num_harts |

## Mã lỗi (1..19)

| # | Tên | Ghi chú |
|---|---|---|
| 1 | `InvalidCapability` | chỉ số tham số sai (a0/a1) hoặc cap không hợp lệ |
| 2 | `WrongObjectType` | chỉ số tham số sai hoặc loại object mong đợi |
| 3 | `InsufficientRights` | quyền còn thiếu (bit quyền) |
| 4 | `WxViolation` | địa chỉ vi phạm W^X |
| 5 | `InvalidArgument` | chỉ số tham số sai |
| 6 | `Alignment` | căn lề yêu cầu |
| 7 | `SlotOccupied` | slot đích đã có cap |
| 8 | `SlotEmpty` | slot nguồn rỗng |
| 9 | `NotEnoughMemory` | số byte còn thiếu |
| 10 | `AlreadyMapped` | địa chỉ ảo đã map |
| 11 | `NotMapped` | địa chỉ ảo chưa map |
| 12 | `MissingPageTable` | cấp bảng trang thiếu |
| 13 | `HasChildren` | số object con còn sống |
| 14 | `Busy` | tài nguyên đang bận (vd IRQ line) |
| 15 | `Cancelled` | thao tác bị huỷ do Suspend(Abort) |
| 16 | `Timeout` | hết timeout_ep |
| 17 | `IllegalOperation` | trạng thái TCB/object không cho phép |
| 18 | `NotSupported` | method/object reserved chưa cài |
| 19 | `RangeError` | chỉ số/chỉ số dải ngoài phạm vi |

## Object

| ID | Tên | Ổn định |
|---|---|---|
| 1 | `Untyped` | frozen |
| 2 | `CNode` | frozen |
| 3 | `Tcb` | frozen |
| 4 | `SchedContext` | frozen |
| 5 | `Endpoint` | frozen |
| 6 | `Notification` | frozen |
| 7 | `Reply` | frozen |
| 8 | `SchedControl` | core |
| 9 | `VSpace` | reserved |
| 10 | `PageTable` | reserved |
| 11 | `Frame` | reserved |
| 12 | `IrqControl` | reserved |
| 13 | `IrqHandler` | reserved |
| 14 | `IOSpace` | reserved |
| 15 | `VCpu` | reserved |
| 16 | `PowerControl` | reserved |

## Chữ ký method (stable-draft, D2-33)

| Method | Tham số | Trả |
|---|---|---|
| `CNode.Copy` | cnode, src, dst, rights? | — |
| `CNode.Delete` | cnode, slot | — |
| `CNode.Mint` | cnode, src, dst, rights?, badge? | — |
| `CNode.Move` | cnode, src, dst | — |
| `CNode.Revoke` | cnode, slot? | — |
| `SchedContext.Bind` | sched, target | — |
| `SchedControl.Configure` | sched_control, sched, budget?, period?, refills? | — |
| `Tcb.Configure` | tcb, cspace?, vspace?, entry?, ipc_buffer?, priority? | — |
| `Tcb.ReadState` | tcb | state, suspended, blocked_on_kind, exit_code |
| `Tcb.Resume` | tcb | — |
| `Tcb.SetFaultEp` | tcb, cap | — |
| `Tcb.Suspend` | tcb, mode | — |
| `Untyped.Retype` | cap, type, size_bits, flags | — |

## Trace (D2-40, `[trace]`)

`record_bytes = 32`, `little`; header `{'magic': 'VFT1', 'version': 1, 'count': 'u32', 'hash': 'fnv1a64'}`

| Trường | Offset | Bytes | Kiểu |
|---|---|---|---|
| `t_ns` | 0 | 8 | `u64` |
| `hart` | 8 | 2 | `u16` |
| `kind` | 10 | 2 | `u16` |
| `tcb` | 12 | 4 | `u32` |
| `a` | 16 | 8 | `u64` |
| `b` | 24 | 8 | `u64` |
