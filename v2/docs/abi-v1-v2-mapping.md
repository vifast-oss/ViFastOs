<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# Bảng ánh xạ v1 → v2 (KT-04a)

Trạng thái: **REVIEW** (cùng `spec/abi.toml`) — chưa duyệt; sau `APPROVED` chỉ Agent 0 sửa (D2-22).
Nguồn: `crates/` @ `ca2eaf5` (tag `v1-archive`), `crates/vifast-abi/src/lib.rs` (`SyscallId` 0..57, `KError`),
QD §4, thiết kế §4.21, `v2/agent/DECISIONS.md` (D2-10, D2-26, D2-28).

> **Hai câu của QD không dùng nguyên văn được** (đã kiểm bằng code v1):
> 1. QD §4 viết "0..13 **như cũ**" — **sai**: v1 đánh số `0=Send, 1=Call, …, 13=CapDelete, 14=CapRevoke`;
>    v2 đánh số `0=Call, 1=Send, 2=NBSend, …, 13=DebugHalt, 14=Exit`. Đây là **đánh số lại**, chỉ
>    giữ *ý niệm* chứ không giữ số. Bảng dưới ghi rõ từng số.
> 2. "22 KError → 19" so **enum v1 có `Ok`=0** (22 biến thể ⇒ 21 mã lỗi) với **19 mã lỗi v2 không có `Ok`**.
>    Vì vậy bảng dưới là "22 biến thể → (0 = thành công, 1..19 = mã lỗi)", không phải đổi tên 1-1.

## 1. 25 crate v1 → vị trí v2

| # | Crate v1 | Vị trí v2 (thiết kế §8) | Trạng thái |
|---|---|---|---|
| 1 | `vifast-abi` | `v2/lib/vifast-abi2` (sinh từ `spec/abi.toml` ở KT-05) | K1a |
| 2 | `vifast-pte` | `v2/lib/vifast-pte` | K2 |
| 3 | `vifast-maint` | `v2/lib/vifast-sync` (hộp thư theo hart, **unsafe**, danh sách đóng) | K1b |
| 4 | `vifast-elf` | `v2/lib/vifast-elf` | **port xong (KT-02)** |
| 5 | `vifast-virtio` | `v2/lib/vifast-virtio` | K6 |
| 6 | `vifast-acpi` | `v2/lib/vifast-acpi` | K5' |
| 7 | `vifast-arch` | `v2/kernel/vifast-plat-common` | K3 |
| 8 | `vifast-arch-riscv64` | `v2/kernel/vifast-plat-riscv64` | **K0.5 tối thiểu (KT-07)** |
| 9 | `vifast-arch-aarch64` | `v2/kernel/vifast-plat-aarch64` | K4' |
| 10 | `vifast-arch-x86_64` | `v2/kernel/vifast-plat-x86_64` | K5' |
| 11 | `vifast-kernel` | `v2/kernel/vifast-core` | K1a |
| 12 | `vifast-kernel-riscv64` | `v2/kernel/vifast-kbin-riscv64` | **K0.5 (KT-07)** |
| 13 | `vifast-kernel-aarch64` | `v2/kernel/vifast-kbin-aarch64` | K4' |
| 14 | `vifast-kernel-x86_64-uefi` | `v2/boot/vifast-boot` (UEFI) | sau K5' |
| 15 | `vifast-platform-riscv64` | gộp vào `v2/kernel/vifast-plat-riscv64` | K3 |
| 16 | `vifast-platform-aarch64` | `v2/kernel/vifast-plat-aarch64` | K4' |
| 17 | `vifast-platform-x86_64` | `v2/kernel/vifast-plat-x86_64` | K5' |
| 18 | `vifast-fs` | `v2/lib/vifast-fs` | **port xong (KT-02)** |
| 19 | `vifast-fsutil` | `v2/lib/vifast-fsutil` | **port xong (KT-02)** |
| 20 | `vifast-runtime` | `v2/user/vifast-urt` (Rust std cho OS) | K3.5/K6 |
| 21 | `vifast-user` | `v2/user/` (wrapper syscall; **nguồn quy ước gọi riscv64**) | K1a |
| 22 | `vifast-server` | `v2/user/{memsrv,procsrv,vfs}` | K6 |
| 23 | `vifast-client` | `v2/test/` (client thử) | K6 |
| 24 | `vifast-root` | `v2/user/init` + `v2/test/` | K3.5 |
| 25 | `vifast-bin` | `v2/tools/` + `v2/user/` (tool host) | liên tục |

Không crate nào bị **bỏ**; `vifast-kernel-x86_64-uefi` đổi vai trò thành `boot/vifast-boot` (UEFI 3 arch).
D2-26: chỉ `vifast-pte`, `vifast-elf`, `vifast-acpi`, `vifast-fs`, `vifast-fsutil`, `vifast-virtio` là "giữ từ v1";
D2-28 giới hạn KT-02 chỉ port `vifast-fs`, `vifast-fsutil`, `vifast-elf` (đã làm).

## 2. 58 syscall v1 → 16 syscall v2 + method

Cột "v2" = số syscall mới, hoặc `Method` (invocation qua `Call` với `label` = method id), hoặc `bỏ`.

| v1 # | v1 | v2 | Ghi chú |
|---|---|---|---|
| 0 | Send | **1 Send** | Notification: Signal; Reply: trả lời |
| 1 | Call | **0 Call** | đổi số |
| 2 | Recv | **3 Recv** | |
| 3 | Reply | `Send` trên Reply cap | Reply do **bên nhận** cấp (QD §3.2) |
| 4 | ReplyRecv | **5 ReplyRecv** | |
| 5 | Signal | **1 Send** (trên Notification) | gộp |
| 6 | Wait | **3 Recv** (trên Notification) | gộp |
| 7 | Poll | **4 NBRecv** | |
| 8 | Yield | **8 Yield** | |
| 9 | Retype | `Untyped.Retype` (0) | method |
| 10 | CapCopy | `CNode.Copy` (0) | method |
| 11 | CapMint | `CNode.Mint` (1) | method |
| 12 | CNodeMint | `CNode.Mint` (1) | gộp với CapMint |
| 13 | CapDelete | `CNode.Delete` (4) | method |
| 14 | CapRevoke | `CNode.Revoke` (5) | method; **v1 14 ≠ v2 14 (Exit)** |
| 15 | TcbConfigure | `Tcb.Configure` (0) | method |
| 16 | TcbResume | `Tcb.Resume` (12) | method |
| 17 | TcbSuspend | `Tcb.Suspend(mode)` (3) | thêm `mode = Abort\|Defer` |
| 18 | TcbRestart | **bỏ** | thay bằng `Tcb.Suspend(Defer)` + `Resume`; `Restart` chỉ còn là tín hiệu nội bộ core→platform (§4.13) |
| 19 | TcbSetPriority | `Tcb.SetPriority` (4) | method |
| 20 | TcbSetAffinity | `Tcb.SetAffinity` (6) | method |
| 21 | TcbBindNotification | `Tcb.BindNotification` (13) | method |
| 22 | SchedContextConfigure | `SchedControl.Configure` (0) | tách object |
| 23 | SchedContextBind | `SchedContext.Bind` (0) | method |
| 24 | PageTableMap | `PageTable.Map` | reserved (K2) |
| 25 | FrameMap | `Frame.Map` | reserved (K2) |
| 26 | FrameUnmap | `Frame.Unmap` | reserved (K2) |
| 27 | IoMap | `Frame.MapIo` | reserved (D3) |
| 28 | IoUnmap | `Frame.UnmapIo` | reserved (D3) |
| 29 | IrqGet | `IrqControl.GetLine` | reserved (K3) |
| 30 | IrqSetNotification | `IrqHandler.SetNotification` | reserved (K3) |
| 31 | IrqAck | `IrqHandler.Ack` | reserved (K3) |
| 32 | DebugWrite | **12 DebugPutStr** | đổi số |
| 33 | GetTime | **11 ClockRead** | đổi số/đổi tên |
| 34 | Exit | **14 Exit** | đổi số |
| 35 | Open | **bỏ** | VFS ở user space |
| 36 | Read | **bỏ** | VFS user |
| 37 | Write | **bỏ** | VFS user |
| 38 | Close | **bỏ** | VFS user |
| 39 | Stat | **bỏ** | VFS user |
| 40 | Seek | **bỏ** | VFS user |
| 41 | TcbSetIpcBuffer | `Tcb.Configure` (0) | trường `ipc_buffer` |
| 42 | FrameWrite | **bỏ** | `Frame.GetAddress` + copy user |
| 43 | BlockRead | **bỏ** | driver user |
| 44 | BlockWrite | **bỏ** | driver user |
| 45 | BlockFlush | **bỏ** | driver user |
| 46 | Sleep | **bỏ** | `Tcb.Suspend` + `timeout_ep`/user timer |
| 47 | ConsoleRead | **bỏ** | console ở user |
| 48 | CNodeInstall | `Tcb.Configure` (0) | trường `cspace_root` |
| 49 | TcbDestroy | **bỏ** | procsrv: `Suspend(Abort)` + Revoke |
| 50 | ObjectDestroy | **bỏ** | `CNode.Revoke`/`Delete` |
| 51 | TcbWait | **bỏ** | RPC procsrv (`wait`) |
| 52 | TcbInfo | `Tcb.ReadState` (2) | method |
| 53 | MemInfo | **bỏ** | Untyped watermark nội bộ |
| 54 | TimerInfo | **bỏ** | `ClockRead` phủ |
| 55 | ModuleRelease | **bỏ** | boot không nạp module archive ở K0.5 |
| 56 | TcbSetArgs | **bỏ** | procsrv (`Configure` mang `pc/sp`) |
| 57 | TcbGrantCap | `CNode.Mint`/`Move` | method |

Tổng (chốt theo Khối F + đếm lại bằng máy): **58 hàng** = **12 hàng ánh xạ vào syscall v2**
(dùng **9** số `{0,1,3,4,5,8,11,12,14}`) + **26 hàng thành method** (23 đích method khác nhau)
+ **20 hàng bỏ**. Bảng syscall v2 có **16** số: 9 số nhận ánh xạ từ v1 + **7 số mới**
`{2 NBSend, 6 NBSendRecv, 7 NBSendWait, 9 WaitAddr, 10 WakeAddr, 13 DebugHalt, 15 DebugGetInfo}`.
`DebugHalt` (13) là syscall **mới** (không có ở v1); `WaitAddr`/`WakeAddr` (9/10) mới (v1 dùng futex trong std shim, không qua ABI).
Dải **15..31 reserved**. `DebugGetInfo` **chưa có số** — xem `[debug_info]` trong `spec/abi.toml`.

## 3. 22 biến thể `KError` v1 → 0 + 19 mã v2

| v1 (`crates/vifast-abi/src/lib.rs:844`) | v2 | Ghi chú |
|---|---|---|
| `Ok` = 0 | `0` = thành công | |
| `InvalidCap` | 1 `InvalidCapability` | |
| `WrongType` | 2 `WrongObjectType` | |
| `NoRights` | 3 `InsufficientRights` | |
| — | 4 `WxViolation` | **mới** (W^X theo Frame) |
| `InvalidArg` | 5 `InvalidArgument` | |
| `Alignment` | 6 `Alignment` | |
| `SlotOccupied` | 7 `SlotOccupied` | |
| — | 8 `SlotEmpty` | **mới** (tách khỏi `InvalidCap`) |
| `OutOfMemory` | 9 `NotEnoughMemory` | đổi tên |
| `AlreadyMapped` | 10 `AlreadyMapped` | |
| `NotMapped` | 11 `NotMapped` | |
| `MissingPageTable` | 12 `MissingPageTable` | |
| `HasDependents` | 13 `HasChildren` | đổi tên |
| `IrqInUse` | 14 `Busy` | gộp |
| `Cancelled` | 15 `Cancelled` | |
| — | 16 `Timeout` | **mới** (`timeout_ep`) |
| `IllegalOperation` | 17 `IllegalOperation` | |
| `NotImplemented` | 18 `NotSupported` | đổi tên; method reserved trả mã này |
| `NotFound` | 17 `IllegalOperation` **hoặc** 19 `RangeError` | tuỳ chỗ: tra không thấy ⇒ `RangeError`; trạng thái sai ⇒ `IllegalOperation` |
| `NoCaller` | 17 `IllegalOperation` | Reply không có caller |
| `ObjectDestroyed` | 1 `InvalidCapability` | cap trỏ object đã bị xoá |
| `PrioTooHigh` | 5 `InvalidArgument` | |
| `WouldBlock` | 14 `Busy` | thao tác non-blocking chưa làm được |
| — | 19 `RangeError` | **mới** (chỉ số/dải) |

**7 mã v1 biến mất** (`NoCaller`, `ObjectDestroyed`, `IrqInUse`, `PrioTooHigh`, `NotImplemented`, `WouldBlock`,
`NotFound`) — mỗi mã có dòng thay thế ở trên, **không mã nào bị bỏ im lặng**. **6 mã mới**: `WxViolation`,
`SlotEmpty`, `NotEnoughMemory` (đổi tên từ `OutOfMemory`), `Timeout`, `NotSupported` (đổi tên), `RangeError`.
`BrokenPipe`, `Stopped` thuộc **giao thức user** (IDL procsrv/pipe), không phải kernel (QD §4).
