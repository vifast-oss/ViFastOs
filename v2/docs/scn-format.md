<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# Định dạng `.scn` — bản nháp `[TK]` (KT-04c)

Trạng thái: **REVIEW**. Thiết kế §9.3 chỉ cho **struct Rust** `Scenario{name, config, setup, threads, expect}`
+ `enum Step`; **không** có cú pháp tệp, schema hay version, và repo chưa có `.scn` nào. Tệp này chốt
**chiếu văn bản 1:1** của §9.3 để KT-08 (`vifast-scenario`) và KT-11 (`vifast-ktest`) cùng đọc (D2-13).

## 1. Quan hệ với §9.3

| §9.3 (Rust) | `.scn` (văn bản) |
|---|---|
| `Scenario.name` | `name = "..."` (trùng tên tệp, không dấu `.scn`) |
| `ConfigOverride` | `[config]` (hiện chỉ `harts`) + `caps = { <tên> = "<ObjectType>" }` |
| `Scenario.setup: [SetupStep]` | `setup = ["<bước>", ...]` |
| `ThreadProgram` | `[[thread]]` với `name` + `steps = ["<bước>", ...]` |
| `Expectation` | đuôi `=> ok` hoặc `=> <Tên mã lỗi>` của mỗi bước |

`enum Step` (§9.3): `Call, Send, Recv, ReplyRecv, Signal, Wait, WaitAddr, WakeAddr, Retype, Map, Unmap,
Revoke, Touch, ReadClock, Spin, Yield, SetAffinity, Suspend, Exit`.
**Chiếu sang ABI v2** (đây là chỗ §9.3 dùng tên v1): `Signal` = `Send` trên Notification, `Wait` = `Recv`
trên Notification, `Retype/Map/Unmap/Revoke/Suspend/SetAffinity` = invocation (`invoke <Object>.<Method>`),
`Touch/Spin` = thao tác bộ nhớ/thời gian của harness (chưa có bước tương ứng trong syscall), `ReadClock` =
`ClockRead`. Tên syscall dùng đúng 16 tên trong `spec/abi.toml` (`call, send, nb_send, recv, nb_recv,
reply_recv, nb_send_recv, nb_send_wait, yield, wait_addr, wake_addr, clock_read, debug_put_str, debug_halt, debug_get_info, exit`).

## 2. Cú pháp một bước

```
<op> [k=v ...] => <kết quả>
```
- `op`: một trong 16 syscall (viết `snake_case`) **hoặc** `invoke <Object>.<Method>`.
- `k=v`: tham số; `v` là số, hex, `true/false`, hoặc **tên cap** khai trong `caps`.
- kết quả: `ok` hoặc **một trong 19 mã lỗi** của `spec/abi.toml` (0 = thành công).

## 3. Ràng buộc máy kiểm (`v2/test/scn/check_scn.py`)

1. `version = 1`; `name` trùng tên tệp; `name`/`tags` không trùng toàn bộ 21 tệp.
2. `op` phải có thật: syscall 0..15 trong `spec/abi.toml`, hoặc `invoke` với `Object.Method` có trong
   `[object]` của spec (hoặc draft reserved `v2/docs/abi-reserved-objects.toml`).
3. `=>` phải là `ok` hoặc tên đúng một trong 19 mã lỗi.
4. Cap dùng ở bước phải khai trong `caps` (dataflow tối thiểu).
5. Đúng **21 tệp**; phủ **11 nhóm** (9 nhóm QD §4 + notification + nhóm mở rộng; xem mục 4); **bắt buộc** có ca `20-exit-procsrv-chua-recv`
   với một bước `exit`.
6. Tệp chưa viết được vì ABI thiếu phải ghi `blocked = "thiếu ABI: ..."` và **không** có bước;
   máy kiểm đếm và in ra (DoD KT-04c: "ca không viết được → BLOCKED 'thiếu ABI'").

**Arity + trạng thái (từ KT-04c)**: nguồn là draft `v2/docs/abi-call-signatures.toml` (D2-33, `status = "REVIEW"`):
máy kiểm đòi `invoke Object.Method` **có chữ ký** trong draft, **đủ tham số bắt buộc**, **không tham số lạ**, giá trị
`variants` đúng (vd `mode = Abort|Defer`); `expect_states` phải thuộc **enum TCB đầy đủ §4.4**. Danh sách method
giới hạn đúng các method 21 `.scn` dùng (D2-33).

**Ngữ nghĩa thời điểm (D2-41):** `expect_states` là **hậu điều kiện sau mỗi bước** của thread tương ứng,
kiểm ngay sau khi bước trả về; với `exit`/`debug_halt` kiểm ngay sau `halt`, **trước** khi procsrv `Recv`
xử lý xong. Phải khớp cả tên trạng thái và trường (`call=false`, `fault=Exit{code}` so mã). Không kiểm
giữa các bước, không suy diễn trạng thái khi chưa có interpreter (KT-09).

## 4. 21 kịch bản và nhóm phủ (QD §4)

| # | Tệp | Nhóm QD §4 |
|---|---|---|
| 01 | `01-cnode-copy` | cap copy |
| 02 | `02-cnode-mint-badge` | cap copy (mint) |
| 03 | `03-cnode-revoke-children` | cap revoke |
| 04 | `04-call-reply-basic` | call/reply |
| 05 | `05-replyrecv-server` | call/reply |
| 06 | `06-call-invalid-cap` | call/reply (lỗi) |
| 07 | `07-reply-deleted-before-reply` | reply bị xoá |
| 08 | `08-reply-revoked-while-blocked` | reply bị xoá |
| 09 | `09-suspend-while-blocked-on-send` | suspend khi chờ |
| 10 | `10-suspend-while-blocked-on-reply` | suspend khi chờ |
| 11 | `11-suspend-abort` | kill khi chờ |
| 12 | `12-suspend-abort-while-reply-wait` | kill khi chờ |
| 13 | `13-exit-no-fault-ep` | exit |
| 14 | `14-exit-with-fault-ep` | exit |
| 15 | `15-cap-transfer-four` | 4 cap |
| 16 | `16-cap-transfer-no-grant` | 4 cap |
| 17 | `17-call-no-reply-bit` | no_reply |
| 18 | `18-schedcontext-donation` | donation (`SchedControl.Configure` + `SchedContext.Bind`) |
| 19 | `19-reply-donation-passive` | donation |
| 20 | `20-exit-procsrv-chua-recv` | exit (bắt buộc) |
| 21 | `21-notification-signal-wait` | (thêm) Notification |
