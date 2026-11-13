<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# Ngữ nghĩa IPC core 1 — KT-24 (model-only `[TK]`)

**Trạng thái:** REVIEW (Agent 2 dòng OS soạn cho Agent 0/reviewer độc lập). **Không phải ABI.**
Nguồn sự thật của ABI vẫn là `spec/abi.toml` (D2-11); tệp này chỉ ghi **quy ước mô hình** đã
dùng trong `v2/sim/vifast-sim` cho KT-24, để KT-25..KT-28 không phải suy lại.

**Nguồn ngữ nghĩa:** `spec/abi.toml:31-49` (MessageInfo/MR/cap), `:105-121` (syscall),
thiết kế `docs/v2/ViFastOs-v2-thiet-ke.md:316-324` (MessageInfo/MR/4 cap/`ipc::transfer`/
invocation), **D2-53 Q7** (`no_reply` hai chiều) và **Q11** (Call non-Endpoint `NotSupported`),
K1C-PLAN §5 KT-24 (DoD 1..9).

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
