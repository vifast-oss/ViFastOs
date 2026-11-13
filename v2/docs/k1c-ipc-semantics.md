<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# Ngữ nghĩa IPC core 1+2 — KT-24/25 + **KT-34** (Reply active, C2) (model-only `[TK]`)

**Trạng thái:** REVIEW (Agent 2 dòng OS soạn cho Agent 0/reviewer độc lập). **Không phải ABI.**
Nguồn sự thật của ABI vẫn là `spec/abi.toml` (D2-11); tệp này chỉ ghi **quy ước mô hình** đã
dùng trong `v2/sim/vifast-sim` cho KT-24 (**IPC core 1**) và KT-25 (**IPC core 2**), để
KT-26..KT-28 không phải suy lại. **§15** ghi quy tắc **cap `Reply` đang active** của **KT-34**
(C2 của K1C-DOD, D2-63).

**Nguồn ngữ nghĩa:** `spec/abi.toml:31-49` (MessageInfo/MR/cap), `:105-121` (syscall),
thiết kế `docs/v2/ViFastOs-v2-thiet-ke.md:316-324` (MessageInfo/MR/4 cap/`ipc::transfer`/
invocation), **D2-53 Q6** (thất bại chuyển cap = đếm mềm), **Q7** (`no_reply` hai chiều),
**Q8** (Reply bị xoá ⇒ caller `Cancelled`), **Q11** (Call non-Endpoint `NotSupported`),
**Q15** (`NBSendRecv`/`NBSendWait`), K1C-PLAN §5 KT-24 (DoD 1..9) + KT-25 (DoD 1..10).

## 1. `MessageInfo` — 35 bit, đúng thứ tự bit của spec

| Trường | Bit | Biên | Nguồn |
|---|---|---|---|
| `label` | 0..19 | `< 2^20` | `spec/abi.toml` `label_bits = 20` |
| `caps` | 20..22 | `<= 4` | `max_caps = 4` (3 bit nhưng ABI chỉ cho 4) |
| `no_reply` | 23 | 0/1 | D2-53 Q7 |
| `extra_caps_unwrapped` | 24..27 | `<= 0xF` | `extra_caps_unwrapped_bits = 4` |
| `length` | 28..34 | `<= 64` | `max_words = 64` (7 bit nhưng ABI chỉ cho 64) |

`MessageInfo::new`/`unpack` **kiểm biên** và trả `InvalidArgument` khi vượt; **không** cắt bit im
lặng. `unpack` từ chối mọi bit ngoài 35 (`bits() & !((1u64<<35)-1) == 0`). Round-trip 1.000 giá trị
ngẫu nhiên (LCG) + biên âm được kiểm trong `tests/kt24_ipc.rs` (DoD 1).

## 2. `Message` — 64 MR + hai vùng nhận **riêng**

```
Message { info: MessageInfo, mr: [u64; 64], caps: Vec<CapId>, badge: u64, msg_id: u64 }   // [TK]
```

- `mr` giữ **byte-for-byte**; `info.length` = số word thật.
- Vùng nhận của TCB (`TcbData.ipc`, kiểu `ipc::IpcArea`) có **hai field riêng**, đúng
  `spec/abi.toml`:
  - `regs: [u64; 4]` — MR0..MR3 đi "thanh ghi" (`words_in_registers = 4`);
  - `buffer: [u64; 60]` — MR4..MR63 đi "IPC buffer model" (`words_in_ipc_buffer = 60`,
    `ipc_buffer_bytes = 4096`, căn 16 — mô hình chỉ giữ **word**, chưa cấp địa chỉ buffer);
  - `info/badge/msg_id` — metadata message **vừa nhận**.
- `mr[4]` **chỉ** có trong `buffer[0]`, không bị gộp vào vùng thanh ghi (DoD 2 kiểm riêng).
- `msg_id` do **kernel** cấp khi message được nhận vào đường IPC (`0` = chưa cấp); bộ đếm
  `ipc_sent` nên **không bao giờ tái dùng** id. Message fault `Exit` **không** có `msg_id`
  (không phải message IPC — xem §5).

## 3. `ipc::transfer` — đường copy **duy nhất**

`ipc::transfer(&Message, &mut IpcArea) -> usize` chép `info`, MR (4 + 60), `badge`, `msg_id`;
**xoá sạch** vùng nhận trước khi chép (message ngắn không lộ dữ liệu message trước). Mọi lần giao
message (`send`/`nb_send`/`call`/`recv`/`nb_recv`, giao thẳng cho waiter hoặc pop khỏi hàng) đều
đi qua hàm này; `Machine::transfer_calls` (test-only) đếm số lần gọi và **luôn bằng**
`ipc_delivered`. `send`/`call`/`recv`/`reply` **không** copy MR tại chỗ (DoD 3).

## 4. `Send` / `NBSend` / `Recv` / `NBRecv` (Endpoint)

| Syscall | Có waiter `Recv`? | Hàng gửi đầy? | Người gửi | Người nhận |
|---|---|---|---|---|
| `Send` (chặn) | có ⇒ giao thẳng | — | giữ `Ready`/`Running` (giao xong) | `Ready`, nhận message |
| `Send` | không ⇒ xếp hàng | đầy ⇒ `Busy` **trước** mọi thay đổi | `BlockedOnSend{call:false}` | — |
| `NBSend` | có ⇒ giao thẳng | — | giữ `Ready`/`Running` | `Ready`, nhận message |
| `NBSend` | không ⇒ xếp hàng | đầy ⇒ `Busy` | **vẫn** `Ready`/`Running` (không chặn) | — |
| `Recv` | — | hàng có message ⇒ giao ngay; rỗng ⇒ `BlockedOnRecv{ep, reply}` | — | — |
| `NBRecv` | — | rỗng ⇒ `Busy` (không chặn); có ⇒ giao ngay và trả message | — | — |

- Thứ tự **FIFO** theo `msg_id`; sức chứa hàng gửi = `MAILBOX_CAP = 8` `[TK]` (KT-22), đầy ⇒
  backpressure `Busy`, **không** ghi đè/không mất message.
- `Call` (bản cũ `call(cap, mr)` và bản mới `call_msg(cap, Message)`): có waiter + Reply cap ⇒
  giao thẳng + caller `BlockedOnReply`; không waiter ⇒ xếp hàng + `BlockedOnSend{call:true}`;
  không có waiter nhưng **không** Reply cap ⇒ xem §5(b).
- `Send` trên **Reply** cap = trả lời (`Reply.caller` được đánh thức) — `.scn` 04 dùng
  `send cap=rep0`; `Send` trên `Notification` ⇒ `NotSupported` tới KT-27; loại khác ⇒
  `WrongObjectType` (`.scn` 06). Message của `Send(reply)` (và `reply` K1b với message rỗng) cũng
  đi qua `admit_message` + `ipc::transfer` tới **caller** **trước** khi caller `Ready`; không có
  caller ⇒ message bị bỏ và đếm `ipc_dropped` (không im lặng mất message).
- **Khác K1C-PLAN §5 KT-24 DoD 4** ("hàng gửi của Endpoint không giới hạn"): K1b/**KT-22**
  (DONE, `67aa8bc`) đã chốt `MAILBOX_CAP = 8` cố định + backpressure `Busy` (model-only `[TK]`,
  seL4 giới hạn 1 message). KT-24 **giữ nguyên** KT-22 — không nới, không hạ, không đổi ngưỡng
  để qua cổng; ghi rõ để reviewer đối chiếu.

## 5. `no_reply` **hai chiều** (D2-53 Q7)

(a) **Caller đặt `no_reply = 1`** ⇒ **Call = Send**: trả `Ok` **ngay**, **không** sinh Reply,
không `BlockedOnReply`; bên nhận **thấy** cờ `no_reply = 1` trong message của mình.

(b) **Kernel thiếu Reply** (bên nhận đã `Recv` **không** kèm Reply cap) ⇒ message **vẫn giao**,
kernel **bật** `no_reply = 1` trên message người nhận thấy, caller nhận **`Cancelled`**:
- waiter đã chờ sẵn ⇒ `call_msg` trả `Err(Cancelled)` ngay;
- caller đã `Call` xếp hàng trước ⇒ khi message được giao (qua `Recv` sau), caller được đánh
  thức `Ready` và lỗi `Cancelled` ghi ở `TcbData.last_ipc_error` (đọc bằng
  `Machine::tcb_last_ipc_error`) — vì mô hình gọi syscall **đồng bộ**, `call` đã trả về trước
  khi kết quả giao được biết.
- Không bao giờ bind `Reply.caller` cho nhánh (b), không treo caller.

## 6. `Call` trên object **không** phải Endpoint (D2-53 Q11)

`Call` (`call`/`call_msg`) kiểm **đích trước** mọi thay đổi trạng thái: cap rỗng ⇒
`InvalidCapability`; object khác `Endpoint` (TCB/CNode/Untyped/Reply/…) ⇒ **`NotSupported`**,
**không** chặn, **không** Reply, `TcbState` **không đổi** (invocation + bảng dispatch
`label → method` thuộc K3). `Send` sai kiểu vẫn là `WrongObjectType` (khác `Call`).

## 7. Bất biến bảo toàn message (`Machine::check_ipc`, sau **mỗi** public op)

- mỗi message **người dùng** còn trong mô hình có `msg_id` trong `1..=ipc_sent` và **không trùng**
  giữa hàng gửi Endpoint và vùng nhận TCB (một `msg_id` đúng **một** nơi);
- `transfer_calls == ipc_delivered` (một đường copy);
- `ipc_sent == ipc_delivered + ipc_pending + ipc_dropped`.

`ipc_dropped` chỉ tăng khi message bị **huỷ** hợp lệ theo D2-25 (`Suspend(Abort)`/finalise TCB,
Revoke Untyped chứa Endpoint) hoặc `Send(reply)` không còn caller (reply bị bỏ — §4.4 `:281`).
Message đã giao cho TCB và bị message **sau** đè lên (`IpcArea` chỉ giữ message vừa nhận) thì coi
là **đã tiêu thụ** — vẫn đếm trong `ipc_delivered`, không còn trong "một nơi" của bất biến.

`check_ipc` chạy sau **mỗi public op IPC** qua `Machine::sched_after_op` (cùng chỗ KT-22 đặt
`check_mailboxes`); op không đi qua `sched_after_op` (`put_cap`/`retype`…) không đổi bộ đếm IPC
nên không cần quét lại. Đường nhanh `ipc_sent == 0` giữ L1 1.000 seed không chậm đi (số đo **chỉ** do cổng sinh — xem `v2/agent/JOURNAL/kt24-gate-fast.log`). Với chuỗi chỉ `send/recv/nb_*`/`call` (không abort),
`ipc_dropped = 0` và bất biến đúng nguyên văn K1C-PLAN §1.1 C1 `ipc_sent == ipc_delivered +
ipc_pending`. Message fault `Exit` **không** tính vào các bộ đếm này (không phải IPC message;
D2-25 vẫn đi qua hàng gửi và `Recv` chuyển người thoát `Inactive`).

## 8. Quy ước runner `.scn` của KT-24 (`tests/kt24_scn.rs`)

1. `caps` trong `.scn` là **khai báo tên**; runner dựng rig: `ep0` (bên gọi) và `ep1` (bên nhận)
   trỏ **cùng một** Endpoint — đúng cách 04/05/07/12/17 dùng cặp tên này.
2. `recv` **không** nêu `reply=` ⇒ runner cấp `rep0` làm Reply window (tệp 07 chứng minh `rep0`
   là Reply của `Call` khi `CNode.Delete slot=rep0` rồi `send cap=rep0 => InvalidCapability`).
3. Tệp 06: `c1` là slot **rỗng** (`call ⇒ InvalidCapability`), `c0` là cap CNode
   (`send ⇒ WrongObjectType`).
4. Runner **đọc tệp** và so kết quả từng bước với `=>` trong tệp; không sửa `.scn`.

## 9. Giới hạn có chủ ý của KT-24 (không tuyên bố đạt)

- **Không** chuyển cap: `Message.caps`/`MessageInfo.caps` > 0 bị từ chối `NotSupported` cho tới
  KT-25 (không im lặng bỏ cap); chưa có GRANT/bỏ GRANT/unwrap badge/receive window.
- **Không** `ReplyRecv`/`NBSendRecv`/`NBSendWait`, chưa có vòng đời `Reply` bị xoá/revoke (KT-25);
  `Notification` word/badge/Signal (KT-27); `WaitAddr`/`WakeAddr` (KT-28); SMP/push (KT-29);
  `vifast-sync` (KT-30).
- **Không** đổi ABI/`spec/`/`vifast-model`/`.scn`; không thêm bit `MessageInfo` ngoài spec
  (D2-15 mục 2): "thất bại chuyển cap" vẫn là **đếm mềm** ở KT-25 (D2-53 Q6), không có bit mới.
- IPC buffer model chỉ giữ **word** + metadata, chưa cấp 4 KiB/căn 16 thật (K2/K3 mới có Frame).

## 10. Chuyển **4 cap** + receive window (KT-25, D2-53 Q6)

**API model-only `[TK]`** (không thêm chữ ký ABI): `Machine::tcb_set_recv_window(tcb, [SlotRef; 4])`
bind **receive window** (CPTR 4 ô CSpace của thread nhận), `Machine::tcb_recv_window(tcb)` đọc
lại. Window luôn **4 ô**; **chưa bind** (`None`) khác **bind 4 ô rỗng** — message có cap mà window
chưa bind ⇒ `InvalidArgument` **trước** khi message được giao (lỗi cấu hình, message **không** mất).

**Đường chuyển**: message mang `caps: Vec<CapId>` nguồn (≤ `MAX_CAPS = 4`). Khi message vào đường
IPC, kernel **chụp ảnh** cap (`ipc::CapSnapshot { cap, obj, ty, rights, badge }`):

| Bước | Khi nào | Kết quả |
|---|---|---|
| `resolve_caps` | mọi message có cap, **trước** khi cấp `msg_id` | cap chết/không tồn tại ⇒ **`InvalidCapability`** (lỗi **cứng**, message không vào IPC) |
| `plan_cap_transfer` | trước khi giao (waiter đã biết, hoặc lúc `recv`/`nb_recv` **trước** khi pop) | quyết định cài/unwrap/bỏ theo bảng dưới |
| `deliver_caps` | lúc giao message (`ipc::transfer`) | `derive` cap con vào window: **bỏ `RIGHT_GRANT`**, giữ `badge`; cập nhật `MessageInfo` của message **nhận** |

**Bảng quyết định theo vị trí** (cap thứ `i` ứng ô window thứ `i`):

| Trường hợp | Xử lý | `MessageInfo` phía nhận |
|---|---|---|
| cap trỏ **chính Endpoint đang gọi** **và** có `badge` | **unwrap**: không chuyển cap, badge **OR** vào message | `extra_caps_unwrapped` += 1; khung window **không** bị chiếm |
| cap nguồn **thiếu `RIGHT_GRANT`** | **thất bại mềm** | `caps` = số cài được; `cap_transfer_failed` += 1 |
| ô window đích **đã có cap** | **thất bại mềm** (không đè cap cũ) | `caps` = số cài được; `cap_transfer_failed` += 1 |
| còn lại | cài cap con (mất `RIGHT_GRANT`, giữ `badge`) | `caps` = số cài được |

- **Không** thêm bit/cờ vào `MessageInfo` (35 bit giữ nguyên — D2-15 mục 2): "thất bại chuyển
  cap" là **bộ đếm mềm** `Machine::cap_transfer_failed` (đọc bằng `cap_transfer_failed()`/
  `cap_transfer_stats()`); `MessageInfo.caps` = **số cap cài được**, `extra_caps_unwrapped` = số
  cap unwrap. `caps > 4` hoặc số cap không khớp ⇒ `InvalidArgument` **trước** khi vào IPC.
- **Lỗi cứng** chỉ khi cap nguồn chết/không tồn tại (`InvalidCapability`) hoặc window chưa bind
  (`InvalidArgument`) — message **không** bị giao nửa chừng, `ipc_sent` không đổi.
- Cap nguồn chết **giữa lúc xếp hàng và lúc giao** (bên gửi `BlockedOnSend` bị revoke cap) ⇒
  thất bại **mềm** (không tạo cap mồ côi, message vẫn giao).
- `Message.caps` của message **đã giao** được **xoá** (cap đã nằm trong window của bên nhận);
  muốn gửi tiếp phải chỉ cap trong CSpace của mình — và cap đã nhận **mất `RIGHT_GRANT`** nên
  không chuyển tiếp được (thất bại mềm), đúng "bỏ GRANT".
- `badge` của message **nhận** = OR các badge unwrap (model-only `[TK]`); `transfer` xoá vùng
  nhận mỗi message nên badge **không** rò giữa các message.

## 11. Vòng đời `Reply` (KT-25, D2-53 Q8)

- Caller `Call` có Reply ⇒ `BlockedOnReply{reply}`; `Reply.caller = caller`. `Send` trên Reply
  (= trả lời) giao message + **trả donation** (KT-21) rồi đánh thức caller `Ready`.
- **Reply bị xoá** khi caller đang `BlockedOnReply` (`CNode.Delete` cap Reply, `CNode.Revoke` cap
  cha của nó, hoặc Reply nằm trong Untyped bị revoke) ⇒ caller được đánh thức **`Ready`** với
  `last_ipc_error = Some(Cancelled)` (**không deadlock**), `Reply.caller = None`; sau đó
  `Send` trên cap đó ⇒ `InvalidCapability` (`.scn` 07), `Delete` slot đã bị `Revoke` xoá ⇒
  `SlotEmpty` (`.scn` 08).
- **Model-only `[TK]`**: seL4 chỉ mất object khi Untyped bị revoke, nhưng `.scn` 07/08 xoá **cap**
  Reply; mô hình coi **một cap Reply bị xoá** = đường reply bị huỷ (object **không** finalise).
  Gọi nhiều lần là idempotent (`caller = None` sau lần đầu).
- **Donation không rò**: huỷ caller đi qua **một** cửa `transition_tcb` (`BlockedOnReply → Ready`)
  ⇒ hook KT-21 trả `SchedContext` về donor (`SchedContext.donated_to/donor = None`,
  `Tcb.donated_sc = None`); kiểm bằng `Machine::check_sc_refs()` sau mỗi nhánh
  (Delete/Revoke/`Suspend(Abort)` caller).
- `Reply.caller` chết (không còn sống) ⇒ `purge_dead_refs` gỡ về `None`. Reply cap đã chết mà
  vẫn nằm trong Recv waiter của Endpoint ⇒ gỡ thành "không cấp Reply": `Delete` gọi
  `clear_dead_reply_in_waiters()` (chỉ đụng **Reply cap chết trong waiter**, giữ nguyên ngữ nghĩa
  khác như KT-18 `Exit` với `fault_ep` chết ⇒ `InvalidCapability`), `Revoke` gỡ qua
  `purge_dead_refs`. `Call` sau đó đi nhánh Q7(b): message **vẫn giao**, caller `Cancelled` —
  không trả cứng `InvalidCapability` (`.scn` 07 tạo đúng trạng thái này).

## 12. `ReplyRecv` / `NBSendRecv` / `NBSendWait` (KT-25, D2-53 Q15a)

- **`ReplyRecv(tcb, reply, ep, msg, recv_reply)`**: giao **reply trước** (`reply_with`) rồi
  `recv`. Reply được giao **xong** trước khi `Recv` có thể chặn ⇒ không mất reply (K1C-PLAN §5
  KT-25 rủi ro (a)). Không có message ⇒ `BlockedOnRecv{ep, reply: recv_reply}` (Reply **mới**).
  Mọi tham số kiểm **trước** khi giao reply ⇒ lỗi cấu hình không để reply đã gửi nửa chừng.
- **`NBSendRecv`** = `NBSend` (không chặn) **rồi** `Recv` (chặn). Nếu `NBSend` **không giao
  được** (`Busy` khi hàng đầy, cap sai…) ⇒ **vẫn** `Recv`; message **không** bị kernel coi là
  mất (`ipc_sent` **không** đổi, người gọi vẫn giữ message để thử lại). Kết quả trả về
  `Ok(None)` = NBSend giao được, `Ok(Some(err))` = NBSend lỗi nhưng Recv đã chạy, `Err` = lỗi
  cứng của Recv.
- **`NBSendWait`** = `NBSend` rồi `Wait` trên Notification (model K1b: **một** waiter,
  `BlockedOnNotification`; word/badge/`Signal` đầy đủ thuộc KT-27). Cùng quy ước kết quả như
  `NBSendRecv`: NBSend lỗi ⇒ **vẫn** Wait.
- **`NBRecv` là poll**: TCB đang **không chạy được** (`BlockedOnReply`/`BlockedOnSend`/
  `BlockedOnRecv`/`BlockedOnNotification`…) ⇒ `Busy` **không** đổi trạng thái (poll không thể
  tạo chuyển trạng thái trái `ALLOWED`). `.scn` 05 dựa vào đúng ngữ nghĩa này (client poll
  Endpoint sau khi `ReplyRecv` của server đã lấy message của nó).

## 13. Quy ước runner `.scn` của KT-25 (`tests/kt25_scn.rs`)

1. `ep0`/`ep1` là **cùng một** Endpoint (D2-54); `recv`/`reply_recv` không nêu `reply=` ⇒ `rep0`.
2. `.scn` 08: `rep0` là **cap con** của cap Reply gốc (slot 1) ⇒ `CNode.Revoke cnode=c0` xoá
   được nó; `Delete slot=rep0` sau đó ⇒ `SlotEmpty`.
3. `.scn` 15: `c0,...` là **tên cap**; runner dựng 4 cap thật ở slot 30..33. **5 tên** ⇒
   `InvalidArgument` khi dựng message (không bao giờ vào IPC). Bước 2 của tệp là `send` cùng
   dạng ⇒ cùng kết quả.
4. `.scn` 16: `c1` là cap **thiếu** `RIGHT_GRANT` (`copy_rights(..., READ|WRITE)`) ⇒
   `Mint rights=WRITE|GRANT` trả `InsufficientRights`.
5. Runner **đọc tệp** (không sửa `.scn`) và so từng bước; tệp này **không** chạy dưới Miri
   (isolation chặn `open`) — Miri của KT-25 chạy `tests/kt25_ipc2.rs`, cùng quy ước
   `kt24_scn.rs`/`kt24_ipc.rs`.

## 14. Giới hạn còn lại của KT-25 (không tuyên bố đạt)

- **Chưa** có `Fault`/`Exit` model (KT-26), `Notification` word/badge/`Signal` (KT-27),
  `WaitAddr`/`WakeAddr` (KT-28), SMP/push migration (KT-29), `vifast-sync` (KT-30).
- Cap chuyển **chưa** mang theo "reply cap" (`RIGHT_GRANT_REPLY`) — K1c dùng `RIGHT_GRANT` cho
  mọi cap; quyền chi tiết theo loại object để K3 conformance.
- Receive reset: không có method "unbind" window (bind lại là thay); `Tcb.Configure` **không**
  đổi window (giữ D2-33: chưa có chữ ký ABI).
- Không đổi `spec/`, `vifast-model`, `.scn`, QUEUE/DECISIONS/AGENTS; không dependency/`unsafe`.

## 15. Cap `Reply` **đang active** — quy tắc C2 (KT-34, D2-63)

**Vấn đề (lỗi mô hình có sẵn, không phải lỗi test).** `Reply` object mang **một** `caller`. Khi
`Call` gặp người nhận đang `Recv` có Reply cap, kernel bind `Reply.caller = Some(caller)` và cho
caller `BlockedOnReply`. Nếu **cùng** `Reply` object đó bị dùng lại cho một lần `Recv` **thứ hai**
(kể cả qua **cap khác** trỏ cùng object), lần `Call` kế tiếp sẽ `caller = Some(mới)` — **ghi đè**
`caller` cũ. TCB cũ vẫn `BlockedOnReply` nhưng Reply **không còn trỏ về nó** ⇒ `Send`/`Reply`
không bao giờ đánh thức nó: **mất đường Reply** (deadlock im lặng). Đây là vi phạm C2 của
K1C-DOD ("không mất đánh thức") ở tầng IPC, không phải chỉ ở driver test.

### 15.1 Quy tắc (model-only `[TK]`, không phải ABI)

| Op | Điều kiện | Kết quả |
|---|---|---|
| `Recv(ep, reply=Some(r))` | `r` là `Reply` **đang** `caller = Some` | **`Busy`** — **trước** khi xoá `last_ipc_error`, trước khi pop message, trước mọi đổi trạng thái |
| `Call(ep)` tới waiter `Recv` có Reply cap `r` | `r` đang `caller = Some` **và** Call này **sẽ** bind (`no_reply = 0`) | **`Busy`** — trước `admit_message` (không cấp `msg_id`), waiter vẫn chờ, message **không** vào IPC |
| `Call no_reply = 1` (Call = Send) | như trên | **không** chặn: không bind Reply nên không thể ghi đè |
| `ReplyRecv(reply, ep, msg, recv_reply)` | `recv_reply` **khác** `reply` và đang `caller = Some` | **`Busy`** — **trước** khi giao reply (không gửi nửa chừng) |
| `ReplyRecv(reply, ep, msg, recv_reply = reply)` | — | **hợp lệ**: `reply_with` tiêu thụ `caller` trước, lúc `recv` chạy Reply đã rảnh |
| `NBSendRecv(..., recv_reply)` | `recv_reply` đang `caller = Some` | **`Busy`** — kiểm **trước** khi `NBSend`/`Recv` chạy (không gửi message rồi mới báo lỗi) |
| sau `Reply`/`Send(reply)` **thành công** | `caller = None` | cap **dùng lại được** ngay (cả hai chiều: `Recv` lại, rồi `Call` mới bind) |

- Lý do chọn **`Busy`** (không `InvalidArgument`): `Reply` đang bị **dùng** (bận tạm thời), không
  phải tham số sai — `Busy` là mã backpressure sẵn có của mô hình (`MAILBOX_CAP` đầy, waiter thứ
  hai của Notification) và **không** đổi trạng thái nào. `InvalidArgument` đã dùng cho lỗi cấu
  hình (`caps > 4`, receive window chưa bind, `WaitAddr` lệch `expected`).
- Object `Reply` **bị xoá/revoke** khi caller đang chờ vẫn theo §11 (caller `Cancelled`) — không
  đụng quy tắc này.
- **Không** phá `.scn` 07/08 (`Delete`/`Revoke` Reply khi caller đang `BlockedOnReply`) và
  **không** chặn `Recv` bình thường: server `Recv` khi Reply **chưa** bind (`caller = None`) là
  đường hợp lệ duy nhất và vẫn chạy y nguyên.

### 15.2 Bất biến C2 — `Machine::check_reply_links`

Chạy sau **mỗi** public op, trong `sched_after_op` (cùng chỗ `check_ipc`/`check_notifications`;
gom **một** hàm nên mỗi op chỉ thêm một lượt quét object):

1. `Reply.caller = Some(t)` ⇒ `t` là TCB **sống** và **đang** `BlockedOnReply{reply}` với `reply`
   **đúng** object này;
2. `TcbState::BlockedOnReply{reply}` ⇒ `reply` là `Reply` **sống** và `caller == Some(tcb)`.

Không có "trạng thái transition hợp lệ" ở **cuối** op: mọi đường bind/gỡ `caller`
(`call_msg`, `deliver_queued`, `reply_with`, `cancel_reply`, `unlink_tcb_queues`) nằm trong **một**
op; trạng thái nửa vời chỉ tồn tại bên trong op nên không bị kiểm.

**Cổng `transition_tcb` giữ liên kết** (cùng chỗ hook KT-21 donation, nên đường tắt cũng không
tạo được trạng thái sai):

- vào `BlockedOnReply{reply}` ⇒ `caller = Some(tcb)`; `Reply` **đã** bind người khác ⇒
  **`IllegalOperation`**, không ghi đè;
- rời `BlockedOnReply` ⇒ gỡ `caller` nếu đang là `tcb`;
- object **không** phải `Reply` sống thì bỏ qua — ma trận 11×11 (KT-23) dựng trạng thái bằng id
  tượng trưng, cùng quy ước chiều ngược của `check_notifications` (KT-27).

### 15.3 Bằng chứng

- `tests/kt25_ipc2.rs`: `recv_cap_reply_dang_active_tra_busy_roi_dung_lai_duoc` (Busy, caller cũ
  giữ nguyên, Reply đúng ⇒ `Ready`, rồi cap dùng lại được), `call_vao_waiter_co_reply_da_active_qua_cap_khac_tra_busy`
  (lớp chặn thứ hai ở `Call` qua **cap khác** cùng object), `cong_transition_giu_lien_ket_reply_va_khong_ghi_de_caller`
  (cổng bind/gỡ/từ chối ghi đè); log đỏ trước: `JOURNAL/kt34-red-reply-reuse.log`.
- Property trên **mọi** thứ tự bước (C2 hai chiều): `kt27_notification::property_signal_sau_khi_xep_hang_danh_thuc_moi_thu_tu`,
  `kt28_waitaddr::property_wakeaddr_sau_khi_xep_hang_danh_thuc_moi_thu_tu`.
- Driver `tests/l1_k1c.rs` (**KT-34**): chỉ cấp cap Reply **rảnh** cho `Recv` (không dùng lại cap
  active) và kiểm `check_reply_links` sau **mỗi** op; `./v2/bin/v2 nightly --l1 --seeds … --smp 1,2,4,8`
  (số đo chỉ do cổng sinh — xem `v2/agent/JOURNAL/kt34-l1-*.log`).

### 15.4 Giới hạn (không tuyên bố đạt)

- Mô hình **một luồng, một hart logic**: "đang active" là trạng thái `caller = Some` do op trước
  để lại, **không** phải tranh chấp nguyên tử thật (MCS lock/K2 — nợ mục 1/3 của K1C-DOD §9).
- `Reply` **chưa** có "reply cap" riêng trong ABI (`RIGHT_GRANT_REPLY`) và **chưa** có ngữ nghĩa
  `Call` nhiều tầng (nested reply) — K3 conformance.
- `Busy` là **model-only**: seL4 thật không có mã này cho trường hợp dùng lại reply; đây là cách
  mô hình **từ chối** thay vì ghi đè im lặng.
