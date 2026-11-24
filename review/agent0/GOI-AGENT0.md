<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# Gói Agent 0 — trả lời YC-02..05 (ViFastOs v2)

> **BẢN CŨ — đã được thay bởi [`DUYET-YC-02-05.md`](DUYET-YC-02-05.md) (Agent 0, 2026-11-12).** Các D2-29..33 trong tệp này không còn hiệu lực; dùng bản trong DUYET.

**Ngày:** 2026-11-12 · **HEAD:** `9552832` · **Nhánh:** `main`
**Người soạn:** người điều phối · **Người ký:** Agent 0

> Tệp này **tự chứa**: không cần đọc tệp nào khác trong `review/agent0/`.
> Đây **không phải duyệt**. Duyệt chỉ có giá trị bằng dòng
> `APPROVED <YYYY-MM-DD> Agent 0` trong `v2/agent/QUEUE.md` (AGENTS.md §8).
> Tệp này chỉ trả lời 5 câu hỏi đang chặn KT-04b/c/d và lật 3 ô trạng thái.

---

## 0. TL;DR — 5 quyết định, 3 ô sổ, 3 dòng luật

| # | Câu hỏi | Chốt đề xuất | Ghi ở đâu |
|---|---|---|---|
| YC-02 | `DebugGetInfo` số mấy? | **syscall 15**, reserved còn 16..31 | D2-29 |
| YC-03 | QD “0..13 như cũ” & `spec/` ở đâu? | 2 errata, không sửa QD/thiết kế | D2-30, D2-31 |
| YC-04 | KT-04b ghi `spec/` thế nào? | cho ghi **một lần** đúng draft 9 object 8..16; VSpace khách = method | D2-32 |
| YC-05 | `.scn` kiểm arity bằng gì? | draft chữ ký hẹp + enum TCB, duyệt ở KT-04d | D2-33 |
| — | 3 ô QUEUE | KT-04a DONE, KT-04b/c READY | `QUEUE.md:102-104` |

Nếu chỉ có 5 phút: ký D2-29..33 + lật 3 ô + sửa 3 dòng luật ở §3 là đủ.

---

## 1. Bối cảnh (5 dòng)

- KT-04a (`spec/abi.toml` lõi + bảng ánh xạ) **xong** `1766145`, nhưng sổ vẫn ghi READY.
- KT-04b (9 object reserved) **dừng đúng luật** vì 4 nguồn chỉ cho KT-04a ghi `spec/`; draft + máy kiểm đã có.
- KT-04c **đã commit** `9552832`: 21 `.scn` + checker + 3 fixture âm; `gate --fast` PASS.
- Phần arity của KT-04c **không kiểm được** vì `spec/` chỉ có `args` văn xuôi + 1 chữ ký method.
- Agent 2 hiện làm **KT-T26** (host test `vifast-fs`), không ngồi chờ; KT-05 vẫn BLOCKED chờ APPROVED.

---

## 2. Năm quyết định

### YC-02 — `DebugGetInfo` = syscall 15

**Hiện trạng.** `spec/abi.toml`: `[debug_info] status = "REVIEW"`, chưa có số; `[syscall_reserved] 15..31`;
`check_abi.py` đang ghim `reserved == 15..31` và `debug_info.status == "REVIEW"`.
D2-15/Q-B bắt buộc ABI có `DebugGetInfo`, nhưng QD §4 không cho số.

**Chốt đề xuất.** `DebugGetInfo` = **syscall 15**; dải reserved còn **16..31**.

**Lý do.** 15 là ô trống đầu tiên; giữ cụm debug cạnh `DebugPutStr` 12 / `DebugHalt` 13 / `Exit` 14;
không thêm object; không đổi 0..14; không phá `min_compat`.

**D2-29 — dán nguyên dòng này:**

> **D2-29** `DebugGetInfo` = **syscall 15**, dải reserved còn **16..31**. Lý do: giữ cụm debug,
> không thêm object, không đổi 0..14. `spec/abi.toml`: thêm `[syscall].15`, `[syscall_reserved].from = 16`,
> `[debug_info].number = 15` + `status = "frozen"`; `check_abi.py` cập nhật tương ứng. QD không sửa.

**Agent 2 sẽ làm sau khi ký:** thêm syscall 15 + sửa 2 hằng trong `check_abi.py`, cổng xanh.

**Nếu không đồng ý:** chọn (i) method `Debug.GetInfo` trên object mới id 17, hoặc (ii) để trống tới K3
(khi đó KT-04d duyệt thiếu một mục đóng băng — phải ghi ngoại lệ rõ).

---

### YC-03 — hai câu QD/thiết kế lệch code

#### 3a. “0..13 như cũ”

Code v1 (`crates/vifast-abi/src/lib.rs`) đánh số `0=Send, 1=Call, …, 14=CapRevoke`;
v2 là `0=Call, 1=Send, …, 14=Exit`. Đây là **đánh số lại**, không phải giữ số.
QD **cấm sửa**, nên ghi errata.

**D2-30 — dán nguyên dòng này:**

> **D2-30** Câu QD §4 “0..13 như cũ” là **ý niệm**, không phải giữ số. v2 đánh số lại;
> `v2/docs/abi-v1-v2-mapping.md` là nguồn đối chiếu từng số. QD giữ nguyên.

#### 3b. `spec/` ở đâu

Thiết kế §8 vẽ `v2/spec/`; AGENTS.md §1/§4.1, `v2/AGENTS.md`, QUEUE (KT-04a), `AGENT2-PROMPT` đều dùng
`spec/` ở **gốc repo**. Agent 2 đã tạo `spec/` ở gốc.

**D2-31 — dán nguyên dòng này:**

> **D2-31** `spec/` ở **gốc repo** là nguồn sự thật; cây `v2/spec/` trong thiết kế §8 là lỗi cây,
> không dùng. Không sửa thiết kế (cấm); errata tại đây.

**Hệ quả:** không sửa code; KT-04d gửi kèm 2 dòng này để người sau không “sửa lại cho khớp giấy”.

---

### YC-04 — KT-04b được ghi `spec/` một lần

**Hiện trạng.** Bốn nguồn chỉ cho **KT-04a** ghi `spec/`; KT-04b không có tên. Agent 2 đã làm đúng:
draft `v2/docs/abi-reserved-objects.toml` (9 object id 8..16, `unsupported = true`) + máy kiểm
`v2/test/abi/check_reserved.py`, và **dừng** trước `spec/`.

**Chốt đề xuất.** Cho KT-04b ghi `spec/abi.toml` **một lần**, đúng draft đã có (kèm D2-29).

**D2-32 — dán nguyên dòng này:**

> **D2-32** KT-04b được **một lần** ghi `spec/abi.toml` đúng draft `v2/docs/abi-reserved-objects.toml`
> và D2-29: 9 object id **8..16** (`SchedControl` 8, `VSpace` 9, `PageTable` 10, `Frame` 11,
> `IrqControl` 12, `IrqHandler` 13, `IOSpace` 14, `VCpu` 15, `PowerControl` 16),
> `stability = "reserved"`, `unsupported = true`, mọi method `unsupported = true`;
> `[object_reserved]` còn **17..31**. **VSpace khách = method `VSpace.Configure` (id 1)**,
> không thêm object `VSpaceGuest`. Agent 2 áp + cập nhật `check_abi.py`, `gate --fast` xanh,
> một commit `[KT-04b]`. Mọi sửa `spec/` khác vẫn theo D2-22.

**Bảng object reserved (đúng thứ tự thiết kế §4.21 sau Reply = 7):**

| ID | Object | Cài ở |
|---|---|---|
| 8 | `SchedControl` | K1b |
| 9 | `VSpace` | K2 |
| 10 | `PageTable` | K2 |
| 11 | `Frame` | K2 |
| 12 | `IrqControl` | K3 |
| 13 | `IrqHandler` | K3 |
| 14 | `IOSpace` | D3 |
| 15 | `VCpu` | H1 |
| 16 | `PowerControl` | P1 |
| 17..31 | trống, chưa cấp | — |

**Vì sao VSpace khách là method, không phải object:**
- Thiết kế §4.21 ghi ở **cột Method**: `VSpace | GetAsid (debug); tạo/khách: reserved`.
- QD §4: “Reserved số cho `VCpu.Configure`, `IOSpace.Attach`, `VSpace` khách” — “số” là **method id**.
- Tách object `VSpaceGuest` làm tăng bề mặt ABI mà không thêm khả năng.

**Vì sao cho Agent 2 ghi thay vì Agent 0 chép tay:**
- Draft đã viết + kiểm trước; thay đổi là chép cơ học, không phát minh số.
- Giữ cổng xanh trong **một** commit. Nếu Agent 0 chép `spec` trước, `check_abi.py` đỏ ngay
  (đang ghim `object_reserved 8..31`), rồi phải sửa checker bằng tay.

---

### YC-05 — chữ ký method + trạng thái TCB cho `.scn`

**Hiện trạng.** 21 `.scn` + checker + 3 fixture đã xong (`9552832`). Nhưng DoD đòi “đủ tham số”:
`spec/abi.toml` chỉ có `args` văn xuôi cho syscall và **một** chữ ký method (`Tcb.Suspend`).
`check_scn.py` không thể kiểm arity nếu không tự đặt ABI. Tương tự, `BlockedOnSend{FaultKind::Exit}`
chỉ là *note*, chưa có từ vựng trạng thái TCB.

**Chốt đề xuất.** Draft chữ ký **hẹp**, không mở toàn bộ ABI.

**D2-33 — dán nguyên dòng này:**

> **D2-33** KT-04c **không** kiểm arity từ `spec/`. Thay bằng draft
> `v2/docs/abi-call-signatures.toml` (`status = "REVIEW"`), chỉ gồm **các method xuất hiện trong 21 `.scn`**
> + enum trạng thái TCB của §4.4. `check_scn.py` kiểm theo draft; KT-04d gửi kèm draft để duyệt;
> sau APPROVED Agent 0 hợp nhất vào `spec/`. Không tự đặt chữ ký cho method ngoài phạm vi 21 ca.

**Phạm vi chữ ký tối thiểu** (đúng các method `.scn` đang gọi):
`Untyped.Retype`, `CNode.Copy`, `CNode.Mint`, `CNode.Revoke`, `CNode.Delete`,
`Tcb.Configure`, `Tcb.Suspend`, `Tcb.Resume`, `SchedContext.Bind`, `SchedControl.Configure`.

**Enum trạng thái TCB:** `Inactive | BlockedOnSend{ep, call, fault} | BlockedOnReply | Running | Ready`.

**Vì sao không ghi thẳng vào `spec/`:** chữ ký method là ABI đóng băng (D2-15); viết cho 10 method
trong 3 h dễ sai quyền/cap transfer. Draft REVIEW cho KT-04c kiểm được ngay, vẫn qua Agent 0 ở KT-04d.

---

## 3. Sửa sổ và luật (bắt buộc, nếu không Agent 2 lại dừng)

### 3a. `v2/agent/QUEUE.md` — lật đúng 3 ô

| Dòng | Hiện tại | Thành | Lý do |
|---|---|---|---|
| 102 KT-04a | `READY` | **`DONE`** | `1766145`, `check_abi.py` PASS |
| 103 KT-04b | `BLOCKED` | **`READY`** | D2-32 mở quyền ghi một lần |
| 104 KT-04c | `BLOCKED` | **`READY`** | `9552832`; còn draft chữ ký theo D2-33 |
| 105 KT-04d | `REVIEW` | giữ `REVIEW` | chưa gửi; chỉ đổi khi 04b/c xong |
| 106 KT-05 | `BLOCKED` | giữ `BLOCKED` | chờ APPROVED |

Sau khi Agent 2 áp xong, KT-04b/c mới chuyển `DONE`; đừng lật sớm.

### 3b. Ba dòng luật phải sửa cùng lúc (nếu không, Agent 2 viện luật và dừng tiếp)

**`AGENTS.md` §3, dòng `spec/`:**

Trước:
```text
| `spec/` | chỉ task được ghi rõ (KT-04a, ST-02a) | xem §4 |
```
Sau:
```text
| `spec/` | chỉ task được ghi rõ (KT-04a, KT-04b một lần theo D2-32, ST-02a) | xem §4 |
```

**`AGENTS.md` §4.1:**

Trước:
```text
1. **Sửa `spec/`** trừ task được ghi rõ trong QUEUE (`KT-04a`: `spec/abi.toml`, `spec/kernel.toml`; `ST-02a`: `docs/viscript/SPEC-lex.md`). Mọi thay đổi ABI/ngữ nghĩa khác phải qua Agent 0 (D2-04, D2-22).
```
Sau:
```text
1. **Sửa `spec/`** trừ task được ghi rõ trong QUEUE (`KT-04a`: `spec/abi.toml`, `spec/kernel.toml`; `KT-04b`: một lần theo D2-32, chỉ chép draft reserved + D2-29; `ST-02a`: `docs/viscript/SPEC-lex.md`). Mọi thay đổi ABI/ngữ nghĩa khác phải qua Agent 0 (D2-04, D2-22).
```

**`v2/agent/QUEUE.md` Khối A #1 (dòng 25):**

Trước:
```text
1. Sửa `spec/` — trừ `KT-04a` (được ghi `spec/abi.toml`, `spec/kernel.toml`).
```
Sau:
```text
1. Sửa `spec/` — trừ `KT-04a` (được ghi `spec/abi.toml`, `spec/kernel.toml`) và `KT-04b`
   (một lần theo D2-32: chỉ chép draft reserved + D2-29).
```

---

## 4. Tùy chọn nếu Agent 0 rời máy ngay (mặc định: KHÔNG)

**Mặc định không ủy quyền.** KT-05 chờ Agent 0 quay lại. Agent 2 chuyển sang
KT-06 / KT-08 / KT-12 / KT-T26 / KT-R1 và dòng ViScript — đủ việc nhiều giờ.

Nếu thật cần chạy qua đêm, ký thêm D2-34; **không khuyến nghị** cho ABI nền:

> **D2-34** Trong khoảng `<từ>`–`<đến>`, nếu Agent 0 không phản hồi, người điều phối được ghi
> `APPROVED <ngày> Agent 0` cho **KT-04d** khi đủ 5 điều kiện: (1) `gate --fast` xanh; (2) `check_scn`
> 21/21, 0 `blocked`; (3) `check_abi` + `check_reserved` xanh; (4) một subagent **không phải Agent 2**
> xác nhận checklist 5 mục/method; (5) không nới ngưỡng/sửa test. Ghi commit hash + kết quả 3 máy kiểm
> + tên reviewer vào `JOURNAL/`. Hết hạn quyền tự hết. Mọi sửa `spec/` sau APPROVED vẫn chỉ Agent 0.

Không có D2-34 thì **không ai** được ghi APPROVED thay, kể cả người điều phối.

---

## 5. Việc Agent 0 làm — checklist

1. [ ] Dán D2-29..D2-33 vào `v2/agent/DECISIONS.md` (sau D2-26, trước `---`).
2. [ ] Sửa 3 dòng luật ở §3b (`AGENTS.md` §3, §4.1; `QUEUE.md:25`).
3. [ ] Lật 3 ô QUEUE ở §3a.
4. [ ] (Tùy chọn) ký D2-34 nếu cần chạy qua đêm.
5. [ ] `git diff --check` và đọc lại diff; commit:
   ```text
   [KT-00] Agent 0 trả lời YC-02..05, mở KT-04b/c
   ```
6. [ ] `git push origin main`.

**Cổng sau bước này:** `./v2/bin/v2 gate --fast` vẫn **PASS** (chỉ sửa tài liệu/sổ).
`check_reserved.py` trên `spec/abi.toml` vẫn **đỏ 9 mục** — đúng, vì Agent 2 chưa áp D2-32.
Đừng “sửa cho xanh” bằng cách nới máy kiểm.

---

## 6. Sau khi ký, Agent 2 sẽ làm

1. Áp D2-32 + D2-29 vào `spec/abi.toml` (9 object + syscall 15) và cập nhật `check_abi.py`,
   `check_reserved.py`; `gate --fast` xanh; commit `[KT-04b]`.
2. Soạn `v2/docs/abi-call-signatures.toml` (REVIEW) + cập nhật `check_scn.py`; commit `[KT-04c]`.
3. Sửa 2 lỗi số liệu trong `v2/docs/abi-v1-v2-mapping.md` (26/23/20; escape `Abort|Defer`).
4. Gửi KT-04d: checklist 5 mục/method + 21 `.scn` + draft chữ ký + 9 object reserved.
5. Agent 0 duyệt: `APPROVED <ngày> Agent 0` dưới KT-04d; `spec/abi.toml` đổi `status = "APPROVED"`
   trong cùng cửa sổ. **APPROVED mở KT-05.**

---

## 7. Không làm

- Không sửa `docs/QUYET-DINH-sau-review.md`, `docs/v2/ViFastOs-v2-thiet-ke.md`,
  `docs/viscript/ViScript-thiet-ke.md`, `docs/viscript/SPEC-lex.md` (cấm tuyệt đối).
- Không ghi `APPROVED` cho KT-04d bây giờ — 04b/c chưa xong; duyệt bây giờ là duyệt thiếu sản phẩm.
- Không để Agent 2 tự ghi APPROVED, kể cả khi mọi test xanh.
- Không nới ngưỡng, không sửa test/kỳ vọng, không bỏ ca để cổng xanh.

---

## 8. Bằng chứng hiện trạng (18:15, 2026-11-12)

- `HEAD = 9552832` — `[KT-04c] 21 kịch bản .scn (chiếu §9.3) + kiểm tĩnh + 3 fixture âm`.
- `gate --fast`: PASS; `check_abi`: PASS; `check_scn`: PASS (21 tệp, 1 blocked, 10 nhóm);
  fixture âm: 3/3 bắt đúng.
- `check_reserved` trên `spec/abi.toml`: **đỏ 9 mục** (VSpace, PageTable, Frame, IrqControl,
  IrqHandler, IOSpace, VCpu, PowerControl, SchedControl) — đúng như dự kiến, chờ D2-32.
- Agent 2 đang KT-T26; `v2/lib/vifast-fs/tests/t26_read_your_own_write.rs` đã có, 3 test pass trong cổng.
- 4 tệp untracked có chủ ý (không thuộc gói này): `ViFastOs-dossier.md`, `review/GUI-AGENT0.md`,
  `review/work/Gui-Agent0-vong-va-2026-11-12.zip`, `review/work/snapshot-1543/`.
