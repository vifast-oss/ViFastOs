<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# KT-04d — Gói gửi Agent 0 duyệt ABI v2

**Ngày gửi**: 2026-11-12 · **Người soạn**: Agent 2 (dòng OS) · **Trạng thái**: **APPROVED 2026-11-12 Agent 0** (D2-35..39).

> **Bản 2 (sau review #9)** — bản gửi 18:29 đã bị thay bằng bản này. Các sửa: draft chữ ký theo đúng thiết kế
> (`Retype` có `type`; `ReadState` trả đủ `state/suspended/blocked_on_kind/exit_code`; `SchedControl.Configure`
> có `refills`; `SchedContext.Bind` nhận `target` là cap Reply/Notification theo thiết kế §4.5), `.scn` sửa các ca sai ngữ nghĩa
> (Exit **không trả** ⇒ token `halt`; 09/10 có peer thật; 16 lấy `InsufficientRights` từ Mint vượt quyền vì §4.6
> nói chuyển cap thất bại **không** mất message; 21 `nb_recv` sau `recv` là `Busy`), `expect_states` có **trường**
> (`BlockedOnSend{ep, call=false, fault=Exit{7}}`) thay vì chỉ kiểm tên, thêm **fixture âm arity** thứ 4,
> cross-check `spec ↔ draft` cho `Tcb.Suspend`. Comment `spec/` còn cũ — **Agent 0 sửa trong cửa sổ APPROVED**, không phải Agent 2.
Duyệt theo `AGENTS.md` §8: một dòng `APPROVED <YYYY-MM-DD> Agent 0` trong `v2/agent/QUEUE.md` dưới mục KT-04d.
APPROVED mở **KT-05**.

## 1. Gói gồm gì

| Thành phần | Tệp | Trạng thái |
|---|---|---|
| `spec/abi.toml` lõi (16 syscall 0..15, reserved 16..31, 19 mã lỗi, MessageInfo 35 bit, quy ước gọi riscv64, object 1..8 + reserved 9..16) | `spec/abi.toml` | `status = "REVIEW"` |
| 21 kịch bản ABI (chỉ bằng ABI, kiểm arity/trạng thái) | `v2/test/scn/*.scn` (21 tệp, 0 blocked) | PASS |
| Draft chữ ký method + enum TCB (D2-33) | `v2/docs/abi-call-signatures.toml` | `REVIEW` |
| Bảng ánh xạ v1→v2 đã sửa theo Khối F | `v2/docs/abi-v1-v2-mapping.md` | 16 syscall / 26 method / 23 đích / 20 bỏ |
| Máy kiểm | `v2/test/abi/check_abi.py`, `check_reserved.py`, `v2/test/scn/check_scn.py` (+4 fixture âm) | gắn `v2/bin/v2 check` |
| Ghi chú T26 (đèn host) | `v2/docs/kt-t26-analysis.md` | xem §5 |

## 2. 21 kịch bản (D2-27)

`01` cap copy · `02` mint+badge · `03` revoke children · `04` call/reply · `05` replyrecv · `06` cap sai (InvalidCapability/WrongObjectType) · `07` reply bị xoá trước khi trả lời · `08` reply bị revoke khi caller chờ · `09` suspend khi chờ (send) · `10` suspend khi chờ (reply) · `11` suspend(Abort) · `12` suspend(Abort) khi chờ reply · `13` exit không fault_ep · `14` exit có fault_ep · `15` chuyển 4 cap · `16` thiếu GRANT (InsufficientRights) · `17` call `no_reply` · `18` donation (`SchedControl.Configure` + `SchedContext.Bind`) · `19` Reply donation passive · **`20` exit khi procsrv chưa Recv — `expect_states = ["BlockedOnSend"]`** · `21` Notification signal/wait.

Kiểm tĩnh: `scn-check: PASS (21 tệp, 0 blocked, 11 nhóm)`; arity theo draft D2-33; 4 fixture âm đều bị bắt.

## 3. Checklist 5 mục/method (D2-33 phụ lục B)

Cột "mã lỗi" = tập con 19 mã (`spec/abi.toml`); "TCB trước → sau" chỉ ghi khi method đổi trạng thái TCB.

| Method | Chữ ký (params → returns) | Mã lỗi có thể trả | Tiền điều kiện | Hậu điều kiện | TCB trước → sau |
|---|---|---|---|---|---|
| `Untyped.Retype` | cap, type, size_bits, flags → (K1a chốt) | InvalidCapability, InvalidArgument, Alignment, NotEnoughMemory, RangeError, NotSupported (flags GUEST) | cap Untyped có Retype | object mới cấp từ vùng untyped, watermark tiến | không đổi |
| `CNode.Copy` | cnode, src, dst, rights? | InvalidCapability, InsufficientRights, SlotOccupied, SlotEmpty, RangeError | src có cap, có quyền Copy, dst trống hoặc cùng cap | dst giữ cap bản sao (mất quyền nếu không GRANT) | không đổi |
| `CNode.Mint` | cnode, src, dst, rights?, badge? | như Copy + InvalidArgument (rights) | như Copy; rights ⊆ quyền của src | dst là cap mới đã mint/badge | không đổi |
| `CNode.Move` | cnode, src, dst | như Copy | nguồn khác đích | src rỗng, dst giữ cap | không đổi |
| `CNode.Revoke` | cnode, slot? | InvalidCapability, IllegalOperation, HasChildren | cap gốc hoặc slot | mọi cap con (mint/copy từ nó) bị thu hồi; Reply đang chờ bị huỷ | caller giữ nguyên |
| `CNode.Delete` | cnode, slot | InvalidCapability, SlotEmpty, HasChildren | slot có cap, không phải cap đang chạy | slot rỗng; object finalise theo loại | không đổi |
| `Tcb.Configure` | tcb, cspace?, vspace?, entry?, ipc_buffer?, priority? | InvalidCapability, InvalidArgument, InsufficientRights, Alignment | tcb chưa Resume; cap đúng loại | trường được cập nhật | Inactive → Inactive |
| `Tcb.SetFaultEp` | tcb, cap | InvalidCapability, WrongObjectType | cap Endpoint có quyền Send | fault_ep = cap | không đổi |
| `Tcb.Suspend` | tcb, mode (Abort\|Defer) | InvalidCapability, IllegalOperation | mode hợp lệ | Abort: huỷ thao tác đang chờ (`Cancelled` cho đối tác); Defer: đánh dấu `suspended`, hoãn tới điểm an toàn | * → `suspended` (Defer) hoặc `Stopping`→`Dead` (Abort) |
| `Tcb.Resume` | tcb | InvalidCapability, IllegalOperation | tcb `suspended` hoặc chưa chạy | xoá cờ `suspended`, vào hàng chờ | `suspended` → Ready |
| `Tcb.ReadState` | tcb → state | InvalidCapability | cap TCB có Read | trả enum TCB (gồm `BlockedOnSend{ep, call, fault}`) | không đổi |
| `SchedContext.Bind` | sched, target | InvalidCapability, WrongObjectType, Busy | sched chưa gắn, target là Reply/Notification tồn tại | sched gắn với Reply/Notification (MCS donation) | không đổi |
| `SchedControl.Configure` | sched_control, sched, budget?, period?, refills? | InvalidCapability, InsufficientRights, InvalidArgument, RangeError | cap SchedControl; tham số trong biên | sched có budget/period | không đổi |

Enum trạng thái: `Inactive | Ready | Running{hart} | BlockedOnSend{ep, call, fault: Option<FaultKind>} | BlockedOnRecv{ep, reply} | BlockedOnReply{reply} | BlockedOnNotification{ntfn} | BlockedOnAddress{frame, offset} | Sleeping{reason: Timeout|Budget} | Stopping | Dead` + cờ `suspended`, `exit_code`.
`FaultKind::Exit{code}` (D2-25): Exit khi có `fault_ep` ⇒ TCB `BlockedOnSend{fault_ep, call=false, fault=Exit{code}}`, không có Reply; procsrv `Recv` xong mới `Inactive`.

## 4. D2-26..D2-28 (nội dung liên quan trong gói)

- **D2-26**: thư viện v1 dùng lại phải **port** vào `v2/lib/`, giữ SPDX + `PORTED-FROM: crates/<tên>@ca2eaf5`; **không** path-dependency sang `crates/`. Đã làm: `vifast-fs`, `vifast-fsutil`, `vifast-elf` (KT-02).
- **D2-27**: **21 kịch bản** = 20 ca QD §4 + "Exit khi procsrv chưa Recv" (QD §1 Q-A). Đã có đủ 21 tệp.
- **D2-28**: KT-02 chỉ port 3 crate nói trên; các crate v1 khác **không** port ở vòng 1 (bảng ánh xạ ghi vị trí dự kiến).

## 5. Những gì KHÔNG có trong gói (trung thực)

- **T26 chưa đóng**: chỉ có **đèn host** (`v2/lib/vifast-fs/tests/t26_read_your_own_write.rs`, 1.000 seed, ca âm mất ghi im lặng). Crate `vifast-fs` **không** có tầng VFS/`read_dir` và không có `smp` ⇒ tái hiện đầy đủ **YC-06** (chờ Agent 0 chọn: đổi DoD thành "đèn", hay chuyển sang KT-08/K3.5).
- **KT-04b/04c đã DONE**, nhưng `spec/abi.toml` vẫn `status = "REVIEW"` cho tới dòng `APPROVED`.
- **KT-05 chưa mở** (chờ APPROVED); chưa có golden file, chưa có gen-check.
- Không có `APPROVED` nào do Agent 2 ghi.

## 6. Đề nghị Agent 0

1. Duyệt (hoặc trả lại kèm lý do) bằng dòng `APPROVED <YYYY-MM-DD> Agent 0` dưới mục KT-04d trong `v2/agent/QUEUE.md`.
2. Xác nhận `DebugGetInfo` = syscall 15 (D2-29) và `SchedControl` = object lõi id 8 (D2-32) như đã áp.
3. Trả lời **YC-06** (phạm vi T26) — mục duy nhất còn mở.
