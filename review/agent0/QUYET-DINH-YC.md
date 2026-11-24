
> **ĐÃ ĐƯỢC AGENT 0 TRẢ LỜI (2026-11-12): xem [`DUYET-YC-02-05.md`](DUYET-YC-02-05.md).** Tệp này giữ làm lịch sử; D2 trong đây đã bị thay bởi bản trong DUYET.

<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# Quyết định YC-02..05 — bản chờ ký (2026-11-12)

Mỗi mục: **hiện trạng → đề xuất → lý do → nếu đồng ý ghi gì → nếu không thì sao**.
Đề xuất đều là `[TK]`/REVIEW, không phải sự thật đã duyệt.

---

## 1. YC-02 — `DebugGetInfo` lấy số nào?

**Hiện trạng.** `spec/abi.toml` có `[debug_info] status = "REVIEW"`, **chưa có số**; dải
`[syscall_reserved] 15..31`; `check_abi.py` ghim `reserved == 15..31` và `debug_info.status == "REVIEW"`.
D2-15/Q-B bắt buộc ABI có `DebugGetInfo`, nhưng QD §4 không cho số.

**Đề xuất: syscall 15.** `[syscall].15 = DebugGetInfo`; `[syscall_reserved] from = 16`;
`[debug_info] number = 15, status = "frozen"`.

**Lý do.**
- 15 là ô trống đầu tiên, giữ cụm debug cạnh `DebugPutStr` 12 / `DebugHalt` 13 / `Exit` 14.
- Không thêm object mới, không đổi 0..14, không phá `min_compat`.
- Nếu để trống tới K3 thì KT-05 sinh code thiếu một hàm đã nằm trong danh sách đóng băng.

**D2-29 (câu chữ mẫu):**
> **D2-29** `DebugGetInfo` = **syscall 15**, dải reserved còn **16..31**. Lý do: giữ cụm debug,
> không thêm object, không đổi 0..14. `spec/abi.toml`: thêm `[syscall].15`, `[syscall_reserved].from = 16`,
> `[debug_info].number = 15` + `status = "frozen"`; `check_abi.py` cập nhật tương ứng. QD không sửa.

**Không đồng ý thì chọn:**
- (i) method `Debug.GetInfo` trên object mới id 17 — cần thêm object + method, nặng hơn;
- (ii) giữ trống tới K3 — KT-04d duyệt thiếu một mục đóng băng, phải ghi rõ ngoại lệ.

---

## 2. YC-03 — hai câu QD/thiết kế lệch code v1

### 2a. "0..13 như cũ"

**Hiện trạng.** QD §4 viết "0..13 như cũ + 14 Exit". Code v1 (`crates/vifast-abi/src/lib.rs`)
đánh số `0=Send, 1=Call, …, 14=CapRevoke`; v2 là `0=Call, 1=Send, …, 14=Exit`. Đây là **đánh số lại**,
không phải giữ số.

**Đề xuất.** Ghi errata, **không** sửa QD (QD cấm sửa):
> **D2-30** Câu QD §4 "0..13 như cũ" là **ý niệm**, không phải giữ số. v2 đánh số lại; bảng
> `v2/docs/abi-v1-v2-mapping.md` là nguồn đối chiếu từng số. QD giữ nguyên.

### 2b. `spec/` ở đâu

**Hiện trạng.** Thiết kế §8 vẽ `v2/spec/`; AGENTS.md §1/§4.1, `v2/AGENTS.md`, QUEUE (KT-04a) và
`AGENT2-PROMPT` đều dùng `spec/` ở **gốc repo**. Agent 2 đã tạo `spec/` ở gốc.

**Đề xuất.**
> **D2-31** `spec/` ở **gốc repo** là nguồn sự thật; cây `v2/spec/` trong thiết kế §8 là lỗi cây,
> không dùng. Không sửa thiết kế (cấm); errata tại đây.

**Hệ quả:** không cần sửa code; KT-04d gửi kèm 2 dòng này để người sau không "sửa lại cho khớp giấy".

---

## 3. YC-04 — KT-04b ghi `spec/` thế nào?

**Hiện trạng.** Bốn nguồn chỉ cho **KT-04a** ghi `spec/`; KT-04b không có tên. Agent 2 đã làm đúng:
draft `v2/docs/abi-reserved-objects.toml` (9 object id 8..16, `unsupported = true`) + máy kiểm
`v2/test/abi/check_reserved.py`, và **dừng** trước `spec/`.

**Đề xuất: cho KT-04b ghi `spec/` một lần, đúng draft đã có.**
> **D2-32** KT-04b được **một lần** ghi `spec/abi.toml` đúng draft `v2/docs/abi-reserved-objects.toml`:
> 9 object id **8..16** (`SchedControl` 8, `VSpace` 9, `PageTable` 10, `Frame` 11, `IrqControl` 12,
> `IrqHandler` 13, `IOSpace` 14, `VCpu` 15, `PowerControl` 16), `stability = "reserved"`,
> `unsupported = true`, mọi method `unsupported = true`; `[object_reserved]` còn **17..31**.
> **VSpace khách = method `VSpace.Configure` (id 1)**, không thêm object `VSpaceGuest`.
> Agent 2 áp + cập nhật `check_abi.py`, `gate --fast` xanh, một commit `[KT-04b]`.
> Mọi sửa `spec/` khác vẫn theo D2-22.

**Vì sao chọn (a) thay vì Agent 0 chép tay.**
- Agent 0 chỉ cần ký 1 dòng; không phải đồng bộ `spec` + 2 máy kiểm + gate trong lúc vội.
- Draft đã được viết + kiểm trước; thay đổi là chép cơ học, không phát minh số mới.
- Giữ cổng xanh trong **một** commit; nếu Agent 0 chép spec trước, `check_abi.py` đỏ ngay
  (nó đang ghim `object_reserved 8..31`).

**"VSpace khách" là object hay method?**
- Thiết kế §4.21 ghi ở **cột Method**: `VSpace | GetAsid (debug); tạo/khách: reserved`.
- QD §4: "Reserved số cho `VCpu.Configure`, `IOSpace.Attach`, `VSpace` khách" — "số" là **method id**.
- Tách object `VSpaceGuest` làm tăng bề mặt ABI mà không thêm khả năng; **không chọn**.

**Không đồng ý thì chọn:**
- (b) Agent 0 tự chép draft vào `spec/` + sửa checker (mất ~15 phút, dễ sót);
- (c) hoãn KT-04b tới sau KT-04d — nhưng KT-04d sẽ duyệt thiếu 9 object của §4.21.

---

## 4. YC-05 — chữ ký method + trạng thái TCB cho `.scn`

**Hiện trạng.** 21 `.scn` + checker + 3 fixture đã xong (`9552832`). Nhưng DoD đòi "đủ tham số":
`spec/abi.toml` chỉ có `args` văn xuôi cho syscall và **một** chữ ký method (`Tcb.Suspend`).
`check_scn.py` không thể kiểm arity nếu không tự đặt ABI. Tương tự, `BlockedOnSend{FaultKind::Exit}`
chỉ là *note*, chưa có từ vựng trạng thái TCB để khẳng định "procsrv chưa Recv".

**Đề xuất: draft chữ ký hẹp, không mở toàn bộ ABI.**
> **D2-33** KT-04c **không** kiểm arity từ `spec/`. Thay bằng draft
> `v2/docs/abi-call-signatures.toml` (`status = "REVIEW"`), chỉ gồm **các method xuất hiện trong 21 `.scn`**
> + enum trạng thái TCB của §4.4. `check_scn.py` kiểm theo draft; KT-04d gửi kèm draft để duyệt;
> sau APPROVED Agent 0 hợp nhất vào `spec/`. Không tự đặt chữ ký cho method ngoài phạm vi 21 ca.

**Phạm vi chữ ký tối thiểu** (đúng các method `.scn` đang gọi):
`Untyped.Retype`, `CNode.Copy`, `CNode.Mint`, `CNode.Revoke`, `CNode.Delete`,
`Tcb.Configure`, `Tcb.Suspend`, `Tcb.Resume`, `SchedContext.Bind`, `SchedControl.Configure`.
Enum trạng thái: `Inactive | BlockedOnSend{ep, call, fault} | BlockedOnReply | Running | Ready`.

**Vì sao không chọn (a) ghi thẳng vào `spec/`:**
- Chữ ký method là ABI đóng băng (D2-15); viết cho 10 method trong 3 h dễ sai quyền/cap transfer.
- Draft REVIEW cho phép KT-04c kiểm được ngay, vẫn qua Agent 0 ở KT-04d.
- Không mở đường "KT-04c tự đặt ABI" — đúng D2-22.

**Không đồng ý thì chọn:**
- (a) cho KT-04c ghi `args/ret` thẳng vào `spec/` — nhanh nhưng Agent 0 phải duyệt kỹ hơn;
- (c) chấp nhận KT-04c **DONE một phần**, ghi rõ "arity chưa kiểm" trong KT-04d — yếu hơn DoD.

---

## 5. DebugGetInfo + reserved: thứ tự áp

1. Agent 0 ghi D2-29..D2-33 + lật QUEUE.
2. Agent 2 áp D2-32 (9 object) + D2-29 (syscall 15) + D2-33 (draft chữ ký) trong **một** commit
   `[KT-04b]`; cập nhật `check_abi.py`, `check_reserved.py`, `check_scn.py`; `gate --fast` xanh.
3. Agent 2 gửi KT-04d: checklist 5 mục/method + 21 `.scn` + draft chữ ký + danh sách object reserved.
4. Agent 0 duyệt: `APPROVED <ngày> Agent 0` dưới KT-04d; `spec/abi.toml` đổi `status = "APPROVED"`
   trong cùng cửa sổ (D2-22). **APPROVED mở KT-05.**

---

## 6. Lật sổ QUEUE (Agent 0 ghi, Agent 2 không sửa)

| Dòng | Hiện tại | Thành | Lý do |
|---|---|---|---|
| `QUEUE.md:102` KT-04a | READY | **DONE** | `1766145`, `check_abi.py` PASS |
| `QUEUE.md:103` KT-04b | BLOCKED | **READY** | D2-32 mở quyền ghi một lần |
| `QUEUE.md:104` KT-04c | BLOCKED | **READY** | `9552832`; còn draft chữ ký theo D2-33 |
| `QUEUE.md:105` KT-04d | REVIEW | **REVIEW** (giữ) | chưa gửi; chỉ đổi khi có sản phẩm 04b/c |
| `QUEUE.md:106` KT-05 | BLOCKED | **BLOCKED** (giữ) | chờ APPROVED |

Ghi chú: sau khi Agent 2 áp xong, KT-04b và KT-04c mới chuyển **DONE**; đừng lật sớm.

---

## 7. Không cần ký nhưng phải sửa trước KT-04d

`v2/docs/abi-v1-v2-mapping.md`:
- dòng 114–116 ghi `27 hàng method (24 đích) + 19 bỏ`; đếm đúng là **26 hàng (23 đích) + 20 bỏ**
  (hàng `18 TcbRestart` là **bỏ**, không phải method);
- dòng 72 có `Abort|Defer` chưa escape, làm hàng 17 vỡ bảng Markdown.

Việc này Agent 2 tự sửa, không đụng ABI; nếu không sửa thì gói KT-04d lại mang số sai như review #5 đã bắt.
