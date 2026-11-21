# KT-309 — riscv64 **cap transfer qua IPC buffer** (`extra_caps`)

Trạng thái: **đang làm** (nhánh `task/kt309-captransfer`, base `main` @ `00c509d5`), 2026-11-21.
Sub-agent `captransfer`. Quyết định: **D2-190** (hợp đồng K3), **D2-205**, **D2-211**, **D2-198**
(uỷ quyền commit trên nhánh task), **D2-05** (test đỏ trước), **D2-83** (core không đọc/ghi bộ
nhớ user), **D2-04/D2-22** (`spec/` đóng băng), **D2-109/D2-111** (tách ảnh khi thêm đường mới).

**Không** tuyên bố K3 đạt lại: K3 đã đóng theo D2-205; KT-309 chỉ **trả một nợ K3.5 đã công bố**
(`STATE.md` dòng `NỢ MỞ`: *"cap transfer qua IPC buffer"*) và **không** mở lại bất kỳ mục nào của
hợp đồng K3.

**Ghi chú đọc trước**: `v2/agent/DECISIONS.md` **không có** mục **D2-220** (tệp dừng ở **D2-219**);
đã đọc D2-190, D2-211, D2-219. Không sửa tệp DECISIONS (ngoài quyền của sub-agent).

## 1. Khảo sát trước (bắt buộc)

Đọc: `v2/kernel/vifast-core/src/{ipc,core_ipc,core_capxfer,core_tcb,syscall,invoke,machine,
machine_slot,tcbdata,objstate,footprint,tls_tests}.rs`,
`v2/kernel/vifast-plat-riscv64/src/{ipc_buffer,user,user_run,core_bridge}.rs`,
`v2/kernel/vifast-plat-user/lib.rs`, `v2/kernel/vifast-kbin-riscv64/src/ktest_ipc.rs`,
`v2/kernel/vifast-kbin-riscv64/init/{ipc_client_user,ipc_server_user}.rs`, `spec/abi.toml`,
`v2/agent/JOURNAL/{kt282-mrbuf,kt290-cfg,kt67b-journal,kt63-nhat-ky}.md`,
`v2/test/boot/{ktest.sh,expected.txt,selftest.sh}`.

| Câu hỏi | Trả lời | Bằng chứng |
|---|---|---|
| `extra_caps` nằm ở **bit nào** của `MessageInfo`? | Trường tên đầy đủ là **`extra_caps_unwrapped`**, **bit 24..27** (4 bit) của thanh ghi 35 bit; `layout = "label:0..19, caps:20..22, no_reply:23, extra_caps_unwrapped:24..27, length:28..34"` | `spec/abi.toml:44-48`, `ipc.rs:86,200` |
| `ipc.rs` **từ chối** ở đâu? | **KHÔNG** từ chối `extra_caps_unwrapped > 0`. `MessageInfo::new` chỉ từ chối `extra_caps_unwrapped >= 16` (biên 4 bit) — **giá trị 1..15 hợp lệ**. Tiền đề của phiếu (*"`ipc.rs` từ chối `extra_caps_unwrapped > 0`"*) **không đúng** với HEAD `00c509d5` | `ipc.rs:122-124`, `ipc.rs:169-175` |
| Vậy `extra_caps_unwrapped` đang **dùng** cho gì? | **Đã cài** từ KT-63: nó là **số cap đã unwrap** (badge OR vào message), do `deliver_caps` ghi (`with_extra_caps_unwrapped(plan.unwrapped())`). Sim pin ngữ nghĩa này | `core_capxfer.rs:119`, `v2/sim/vifast-sim/tests/kt25_ipc2.rs:486` (`"một cap unwrap"`), `kt63_core_cap_ipc.rs:249,280` |
| IPC buffer có chỗ cho **danh sách cap** không? | **CÓ, hai chỗ**: (a) **header** 10 word đã có **4 CPTR nguồn** (`send_caps`, word 6..9) — đường KT-67b cho **≤ 4** cap; (b) vùng **dự phòng** `byte 560..4096` **trống hoàn toàn** (KT-282 chỉ dùng `80..560` cho MR4..MR63) | `ipc.rs:578-586`, `kt282-mrbuf.md:35-42` |
| `CNode` có hàm `copy`/`mint` **dùng lại được** không? | **CÓ** — `Machine::copy_rights(src, dst, rights)` / `mint(src, dst, rights, badge)` / `derive(...)`; `derive` **đã** kiểm `check_cnode_slot` + `SlotOccupied` (7) và tạo cap con qua `push_cap`. Không cần viết lại | `machine.rs:892-975` |
| Quyền: caller phải có gì trên cap nguồn? | `RIGHT_GRANT` (bit 2, `spec` `[rights]`); `plan_cap_transfer` của KT-63 coi thiếu `RIGHT_GRANT` là **thất bại mềm**; cap cài sang bên nhận **bỏ** `RIGHT_GRANT` | `spec.rs:28`, `ipc.rs:322-330`, `core_capxfer.rs:104` |
| Quyền: receiver phải có gì trên CNode đích? | Model **không** có cap-quyền cho **cspace gốc** của một TCB (`TcbData.cspace: Option<ObjId>`, không phải `SlotRef` + quyền) ⇒ kiểm được **CNode còn sống** + **slot trong span** + **slot rỗng**; phần "`RIGHT_WRITE` trên CNode đích" **không biểu diễn được** trong model hiện tại (ghi nợ §7) | `tcbdata.rs:24`, `machine_slot.rs:12,68,88` |
| `Message` mang được cap **qua hàng chờ** không? | **CÓ** — `Message.caps: [CapId; 4]` + `snaps: [CapSnapshot; 4]` + `n_caps`/`n_snaps`; `resolve_caps` chụp ảnh lúc `admit`, `deliver_caps` cài lúc giao. Pool `ipc_msgs: [QueuedMsg; 24]` | `ipc.rs:400-470`, `core_capxfer.rs:20-40`, `machine.rs:62,220` |
| Ngân sách tĩnh còn bao nhiêu? | `size_of::<Machine>()` = **190.920 B** / pin **196.608** (headroom **5.688**); ảnh chính `.data+.bss` **246.936** / 262.144 | `tls_tests.rs:332-360`, `ktest.sh:588,601,631,791` |

⇒ **Kết luận khảo sát**: (a) `extra_caps_unwrapped` **đã có chủ** (unwrap badge, KT-63) — dùng nó
làm **số cap** là **đổi ngữ nghĩa ABI** (tên trường trong `spec` nói "unwrapped", sim pin ngữ
nghĩa unwrap) ⇒ **không làm**; (b) đường **≤ 4 cap** đã có (KT-67b, header `send_caps`); (c) chỗ
**trống** duy nhất cho danh sách cap **> 4** là vùng dự phòng `560..4096` của IPC buffer — **cùng
khuôn `[TK]`** mà KT-282 đã dùng cho MR4..MR63 (*"spec không chốt offset vùng MR"*).

## 2. Thiết kế (kênh hẹp — ghi rõ)

### 2.1 Vì sao **không** dùng `MessageInfo.extra_caps_unwrapped`

Phiếu ghi *"`extra_caps = n`"* như trường của `MessageInfo`. Nhưng trường 4 bit duy nhất ở
`24..27` là **`extra_caps_unwrapped`** và **đã được KT-63 cài** làm **số cap unwrap** (badge OR),
có test sim pin (`kt25_ipc2.rs:486`, `kt63_core_cap_ipc.rs:249,280`) và tên trường trong `spec`
nói rõ "unwrapped". Gán lại nó thành "số cap trong IPC buffer" là **đổi ngữ nghĩa ABI** ⇒ theo
luật của phiếu (*"nếu cần đổi ABI thì DỪNG, báo lead"*) **không tự làm**; thay vào đó **số `n`
nằm trong chính vùng dự phòng của IPC buffer** — đúng khuôn `[TK]` của KT-282/KT-290, **không**
chạm `spec/`, **không** đổi `MessageInfo` 35 bit. Ghi thành **nợ/đề xuất** ở §7 để lead quyết.

### 2.2 Layout IPC buffer (`[TK]` KT-309 — nối tiếp KT-282)

```text
byte   0.. 80 : header 10 word (magic, MessageInfo bản ghi, receive window 4 CPTR, send caps 4 CPTR) — KT-67b
byte  80..560 : MR4..MR63 (60 word = 480 B) — KT-282
byte 560..568 : **extra_caps count n** (0..=4) — KT-309
byte 568..600 : **n CPTR nguồn** (4 × 8 B, CPTR trong CSpace **caller**) — KT-309
byte 600..608 : **extra_caps dst** (slot đích trong CSpace **receiver**; 0 = "slot trống đầu tiên") — KT-309
byte 608..4096: dự phòng (zero)
```

Hằng **một nguồn** ở `vifast_core::ipc` (`IPC_BUFFER_EXTRA_CAPS_OFFSET` = `IPC_BUFFER_MR_END` =
560, `IPC_BUFFER_EXTRA_CAPS_WORDS` = 5, `IPC_BUFFER_EXTRA_CAPS_DST_OFFSET` = 600,
`IPC_BUFFER_EXTRA_CAPS_END` = 608); shim U-mode dùng **cùng** số. **Không** đổi `spec/`.

### 2.3 Ngữ nghĩa

**Gửi** (`build_message`, caller đang chạy ⇒ arch đọc bộ nhớ user, D2-83):

1. `n = words[0]`; `n > 4` ⇒ **`InvalidArgument` (5)** — biên `[message].max_caps = 4`, **không**
   nới (đột biến **M2**).
2. Mỗi CPTR `i < n`: `cap_slot` trong CSpace **caller**; cap phải **sống** (chết/rỗng ⇒
   `InvalidCapability` 1).
3. Cap nguồn **thiếu `RIGHT_GRANT`** ⇒ **`InsufficientRights` (3)** — lỗi **cứng** ngay ở đường
   gửi (khác KT-63 "mềm" cho cap trong thanh ghi: ở đây caller **chủ động** khai `n` nên sai là
   lỗi) (đột biến **M1**).
4. Chụp ảnh (`CapSnapshot`) vào `Message.extra_snaps` — message **không** copy object, chỉ cap
   tham chiếu (cùng nguyên tắc KT-63).

**Nhận** (`deliver_extra_caps`, gọi từ `deliver_message` sau `deliver_caps`):

5. Slot đích: `dst` do **receiver** khai ở `byte 600` (đọc lúc `Recv` — receiver đang chạy);
   `dst == 0` ⇒ **slot trống đầu tiên** trong CSpace receiver (quét từ 1). Cap thứ `i` vào
   `dst + i`.
6. Slot đích **đã có cap** ⇒ **`SlotOccupied` (7)** — lỗi **cứng**, message **không** giao
   (đột biến **M3**).
7. Cap cài sang receiver **bỏ `RIGHT_GRANT`**, **giữ badge** nguồn (cùng luật KT-63/thiết kế §4.6).
8. Cap nguồn chết giữa lúc xếp hàng và lúc giao ⇒ **thất bại mềm** cho **riêng** cap đó
   (`cap_transfer_failed` tăng), message **vẫn** giao — cùng chuẩn KT-63.

### 2.4 Đo

`# ipc caps: sent=<n> received=<n> slot=<n> rights=0x…` — `sent` = số cap client khai, `received`
= số cap **thật** cài được vào CSpace server (kernel đếm), `slot` = slot đầu tiên, `rights` =
quyền **thật** của cap trong CSpace server (đọc lại từ bảng cap, **không** tin lời chương trình).
TAP mới: `ok 5 - ipc: cap transfer`.

### 2.5 Ca âm (trong cùng boot)

| # | Ca | Kỳ vọng |
|---|---|---|
| (i) | cap nguồn **thiếu `RIGHT_GRANT`** | `InsufficientRights` **3** |
| (ii) | `n = 5` (> `max_caps = 4`) | `InvalidArgument` **5** |
| (iii) | slot đích **đã bận** (receiver khai `dst` = ô đang có cap) | `SlotOccupied` **7** |

Giữ nguyên **toàn bộ** ca âm cũ của KT-280/KT-282/KT-290.
