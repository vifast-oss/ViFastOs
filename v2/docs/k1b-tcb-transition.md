# K1b — Bảng chuyển trạng thái TCB (`ALLOWED` 11×11)

> **Model-only `[TK]` — KHÔNG phải spec, KHÔNG phải ABI.** Vị trí bảng do **D2-49(d)** chốt
> (`v2/agent/DECISIONS.md:85`): K1b tự chứa bảng trong `v2/sim/vifast-sim/src/tcb.rs` + tài liệu
> này; **Agent 0 review**, không sửa `spec/`. Khi K3 làm conformance mới bàn chuyển bảng thành
> normative (khi đó cần một D2 mới, theo D2-22).

- **Nguồn ngữ nghĩa**: thiết kế `docs/v2/ViFastOs-v2-thiet-ke.md` §4.4 (`:256-286`), bảng ngân
  sách §4.5 (`:302-311`), **D2-33** (enum đầy đủ, `BlockedOnRecv{ep, reply}`, `BlockedOnAddress.offset: u32`),
  **D2-25** (`Exit`, `Resume`/`Configure` xoá `exit_code`).
- **Code**: `v2/sim/vifast-sim/src/tcb.rs` — `pub const ALLOWED: [[bool; 11]; 11]`,
  `TcbState`, `TcbState::index()`, `TcbSnapshot`, `BlockedOnKind`, `SuspendMode`;
  cổng duy nhất: `Machine::transition_tcb(tcb, to)` (`v2/sim/vifast-sim/src/kobj.rs`).
- **Test**: `v2/sim/vifast-sim/tests/kt18_tcb.rs` — `bang_allowed_11x11_du_cap_va_cam` (đủ 121 cặp),
  `moi_duong_dung_mot_cua_transition_tcb` (đường thật `call/recv/reply/exit/suspend/finalise`).
- **Bất biến**: cặp cấm ⇒ `IllegalOperation`, **không** đổi trạng thái; mọi cặp ALLOWED phải có
  ít nhất một hit (`pair_hits` + `transition_coverage()`).

## 1. Thứ tự chỉ số (khớp `vifast_abi::TCB_STATES`, D2-33)

| # | Trạng thái | Hàng chờ / ý nghĩa |
|---|---|---|
| 0 | `Inactive` | TCB mới, chưa kích hoạt |
| 1 | `Ready` | trong ready queue, chỉ khi `!suspended` |
| 2 | `Running{hart}` | đang chạy trên hart (K1b 1 hart ⇒ `hart = 0`) |
| 3 | `BlockedOnSend{ep, call, fault}` | chờ gửi; `call = true` là `Call` chờ Reply, `fault = Some(Exit)` là giao fault |
| 4 | `BlockedOnRecv{ep, reply}` | chờ nhận; `reply: Option<ObjId>` là Reply cap server đặt sẵn (**D2-33**) |
| 5 | `BlockedOnReply{reply}` | chờ `Send(reply)` |
| 6 | `BlockedOnNotification{ntfn}` | chờ notification (§4.7) |
| 7 | `BlockedOnAddress{frame, offset: u32}` | chờ WaitOnAddress (§4.7) |
| 8 | `Sleeping{Timeout \| Budget}` | hết timeout / hết budget (§4.5) |
| 9 | `Stopping` | chờ hart khác dừng mình (finalise/migrate — K1c) |
| 10 | `Dead` | đã finalise, không ra được |

`suspended` và `exit_code` là cờ **trực giao** (D2-25/D2-33), không phải trạng thái: `Ready +
suspended` không nằm trong ready queue; `Sleeping + suspended` vẫn `Sleeping`.

## 2. Bảng `ALLOWED`

`x` = ALLOWED (đi qua cổng được), `.` = cặp cấm (`IllegalOperation`, **không** đổi trạng thái).
Hàng = nguồn, cột = đích.

| From \ To | In | Re | Ru | BS | BR | BP | BN | BA | Sl | St | De |
|---|---|---|---|---|---|---|---|---|---|---|---|
| **Inactive** | . | x | . | x | . | . | . | . | . | . | x |
| **Ready** | x | . | x | x | x | x | x | x | . | x | x |
| **Running** | x | x | . | x | x | x | x | x | x | x | x |
| **BlockedOnSend** | x | x | . | x | . | x | . | . | . | . | x |
| **BlockedOnRecv** | x | x | . | x | . | . | . | . | . | . | x |
| **BlockedOnReply** | x | x | . | x | . | . | . | . | . | . | x |
| **BlockedOnNotification** | x | x | . | x | . | . | . | . | . | . | x |
| **BlockedOnAddress** | x | x | . | x | . | . | . | . | . | . | x |
| **Sleeping** | x | x | . | x | . | . | . | . | . | . | x |
| **Stopping** | . | . | . | . | . | . | . | . | . | . | x |
| **Dead** | . | . | . | . | . | . | . | . | . | . | . |

Bảng có **48** cặp ALLOWED trên 121 ô = **46 cặp suy ra từ thiết kế** (§4.4/§4.5/D2-25/D2-33)
+ **2 cặp `Stopping` reserved** (`Ready → Stopping`, `Running → Stopping`; KT-19/K1c dùng, K1b 1 hart
chưa có đường nhập `Stopping`). Test `bang_allowed_11x11_du_cap_va_cam` kiểm đủ 121 ô, assert
`48 = 46 + 2`, và `transition_coverage()` ở đó là coverage **của bảng qua chính cổng** (ma trận gọi
cổng trực tiếp cho từng cặp) — **không** phải coverage thao tác. Coverage **thao tác** (đường
`call/recv/reply/exit/suspend/finalise` thật sự đi qua cổng) kiểm ở
`moi_duong_dung_mot_cua_transition_tcb` cho đúng các cặp K1b dùng; các cặp hàng `Running`/`Sleeping`
chỉ được thao tác dùng khi KT-19/KT-20 có scheduler/budget.

Ca cấm được viết **từ lý do thiết kế** ở `cap_cam_theo_thiet_ke_bi_tu_choi` (`Inactive→Running`,
`Sleeping→Running`, `BlockedOnReply→BlockedOnRecv`, `Dead→Ready`, `Ready→Sleeping`) để bảng không tự
chứng minh vòng tròn (kỳ vọng không lấy từ chính `ALLOWED`).

## 3. Lý do từng nhóm cặp (viết từ thiết kế, **không** suy ra từ bảng)

| Nguồn | Cặp | Lý do (`[TK]` nếu là diễn giải mô hình) |
|---|---|---|
| §4.4 `:272-282` | `Inactive → Ready` | Kích hoạt TCB. Mô hình K1b chưa có method `Activate`/syscall `Tcb.Activate` ⇒ **kích hoạt ngầm** khi TCB phát syscall IPC đầu tiên (`ensure_started`); `Inactive → Running` **cấm** (phải qua Ready) |
| D2-25, §4.4 `:284-286` | `Inactive → BlockedOnSend` | `Exit` có `fault_ep` trên TCB chưa chạy: message fault xếp vào hàng gửi `fault_ep` (`.scn` `exit` chạy từ TCB `Inactive`) |
| §4.8 `:336-344` | `Ready → Running`, `Running → Ready` | `pick` O(1) và preempt; thuộc KT-19 nhưng là cặp của bảng K1b |
| §4.4 | `Ready/Running → BlockedOnSend/Recv/Reply/Notification/Address` | Syscall chặn từ trạng thái chạy được; mô hình K1b chưa phân biệt "đang chạy" với "Ready trong hàng đợi" nên cả hai hàng đều vào được |
| §4.5 `:302-311` | `Running → Sleeping` | Hết budget ⇒ `Sleeping{reason = Budget}` (không thêm enum, QD §3.4) |
| §4.5 `:307-308` | `Sleeping → Ready` | Timer refill/timeout tới hạn |
| §4.4 `:276-277` | `BlockedOnSend → Ready` (không `call`), `BlockedOnSend → BlockedOnReply` (`call` + Reply cap), `BlockedOnRecv → Ready`, `BlockedOnReply → Ready` | Hoàn tất chờ: `Send` không cần Reply; `Call` được server `Recv` kèm Reply cap ⇒ chờ `Send(reply)`; server nhận được message; `Send(reply)` đánh thức người gọi |
| §4.4 `:279-281` | mọi `BlockedOn* → Ready` | `Suspend(Abort)`: gỡ khỏi hàng chờ **trước** khi bật cờ, TCB thành `Ready + suspended` |
| §4.5 `:309-310` | `Sleeping → Sleeping` (không chuyển) + `suspended` | `Suspend(Abort)` trên `Sleeping{Budget}` **giữ** `Sleeping` (không phải cặp chuyển trạng thái) |
| D2-25, QD §1 Q-A | `BlockedOnSend → Inactive` | Fault `Exit` được procsrv `Recv` xong ⇒ `Inactive` |
| D2-25 | `* → Inactive` (không `fault_ep`) | `Exit` không có `fault_ep` ⇒ `Inactive` ngay (từ `Inactive` thì không có chuyển trạng thái — chỉ đặt `exit_code`) |
| D2-25 | `*.live → BlockedOnSend` (kể cả cặp danh tính `BlockedOnSend → BlockedOnSend`) | `Exit` có `fault_ep`: message/hàng cũ bị **huỷ** rồi chuyển sang hàng gửi của `fault_ep`; đây là **cặp danh tính duy nhất** của bảng |
| §4.3 bảng Finalise (`:229`) | `* → Dead` | Finalise TCB: gỡ ready queue/hàng chờ, chuyển `Dead` **trước** khi object chết |
| §4.4 `:270` | `Ready/Running → Stopping` | `Stopping` là "chờ hart khác dừng mình" cho finalise/migrate; K1b **1 hart** (D2-49(a)) chưa có đường nào dùng, cặp vẫn có để hàng/cột phủ đủ (KT-19/K1c dùng) |
| §4.4 `:270` | `Stopping → Dead` | Dừng xong thì finalise |
| — | `Dead → *` | Trạng thái cuối, **không** ra được |

Cặp **không** có nguồn thiết kế (ví dụ `Ready → Inactive` khi không `Exit`, `Sleeping → Running`,
`BlockedOnReply → BlockedOnRecv`) ⇒ **cấm**: mặc định an toàn, không mở "cho tiện".

## 4. Quy ước mô hình K1b (khác biệt có chủ ý, `[TK]`)

1. **Không có cặp danh tính** ngoài `BlockedOnSend → BlockedOnSend`: thao tác **không** đổi trạng
   thái (`Tcb.Resume`; `Suspend` trên `Ready`/`Sleeping`; `Recv` của server đã `Ready`) **không**
   gọi cổng. Nhờ vậy bảng không có ô "tự chuyển" nào bị dùng để né kiểm.
2. **Kích hoạt ngầm**: `Inactive → Ready` khi TCB phát `Call`/`Recv`/`Wait`/`WaitAddr` đầu tiên.
   Đây là chỗ mô hình K1b lệch seL4 (seL4 bắt buộc `Tcb_Configure` + `Restart`); KT-19 thêm
   `pick`/ready queue và có thể siết lại nếu Agent 0 muốn.
3. **`Exit` từ `Inactive`** (không `fault_ep`) chỉ đặt `exit_code`, **không** chuyển trạng thái —
   giữ hành vi cũ của `.scn`/test KT-09.
4. **`Tcb.Resume`** chỉ xoá `suspended` và `exit_code` (D2-25); **không bao giờ** phát lại
   `Call/Send/Recv/ReplyRecv` (§4.4 `:283`). `Tcb.Configure` cũng xoá `exit_code` (D2-25).
5. **`Suspend(Defer)`**: §4.4 `:282` chỉ hợp lệ với thao tác dài có `Restart`; K1b **chưa có**
   thao tác dài ⇒ **mọi** `Defer` trả `InvalidArgument`, trạng thái không đổi.
   ✅ **KT-23 đã chốt** (D2-52): `.scn` `12-suspend-abort-while-reply-wait.scn:13` sửa đúng
   **một** dòng `mode=Defer => InvalidArgument` — khớp bảng/cửa `tcb_suspend`, không đổi ABI;
   `scn-check` vẫn PASS **21 tệp/11 nhóm**. Đây là ngoại lệ **duy nhất** được phép sửa `.scn`
   của K1b (K1B-PLAN §5 KT-23 DoD 8 + D2-52).
   **Bằng chứng phủ bảng (KT-23)**: `kt23_k1b` đi **121/121** ô qua chính cổng
   `transition_tcb` — **46/46** cặp thiết kế có hit (100%), **73/73** cặp cấm bị thử và trả
   `IllegalOperation`, 2 cặp `Stopping` reserved đi được nhưng đếm riêng (`hit_table = 48`).
6. **`Suspend(Abort)` trên `Inactive`/`Stopping`/`Dead`** ⇒ `IllegalOperation` (không có gì để dừng).
7. **`Send(reply)` khi `Reply.caller = None`** (client đã bị `Abort` tách ra) ⇒ **thành công nhưng
   reply bị bỏ** (§4.4 `:281`), không đánh thức ai, không lỗi.
8. **Một waiter** cho Notification và bucket WaitOnAddress `(frame, offset)` — mô hình tối giản
   của K1b; `Signal`/`WakeAddr`/khoá theo địa chỉ vật lý thuộc **K1c/K2** (§4.7). K1b chưa có
   object `Frame` nên bucket dùng `ObjId` thô của frame.
9. **`Exit` từ mọi trạng thái sống** là quy ước **model-only**: D2-25 không giới hạn trạng thái
   nguồn (và `.scn`/test KT-09 gọi `Exit` trên TCB `Inactive`), nên bảng cho phép
   `*live → Inactive`/`*live → BlockedOnSend`; K1c (fault thật) sẽ siết "chỉ khi `Running`" nếu
   Agent 0 yêu cầu. `Exit` **luôn** kiểm `fault_ep` trước khi ghi `exit_code`/gỡ hàng chờ: cap
   `fault_ep` đã chết ⇒ `InvalidCapability` và TCB không đổi gì (reviewer-kt18 mục 1).
10. **`Send(reply)` khi `Reply.caller = None`**: **im lặng thành công** (reply bị bỏ), kể cả khi
    `caller` chưa từng được đặt — mô hình K1b không phân biệt hai ca; K1c sẽ trả `Cancelled` cho
    client và có thể tách hai đường.

## 5. Drift D2-33 đã sửa ở KT-18

| Drift trước KT-18 | Sau KT-18 |
|---|---|
| `BlockedOnRecv { ep }` — thiếu `reply` | `BlockedOnRecv { ep, reply: Option<ObjId> }` (đúng D2-33) |
| `BlockedOnAddress.offset: u64` | `offset: u32` (đúng thiết kế `:269` và D2-33) |
| `tcb_read_state` trả `TcbState` (1 giá trị) | trả `TcbSnapshot { state, suspended, blocked_on_kind, exit_code }` (`Tcb.ReadState`, 4 giá trị) |
| `set_tcb_state` nhận **mọi** cặp | `transition_tcb` + `const ALLOWED`, cặp cấm `IllegalOperation` |
| Không có `suspended`/`exit_code` trực giao đọc được | `suspended` + `exit_code` trong `TcbData`; `TcbInfo`/`TcbSnapshot` đọc được |
| `Suspend(Abort)` không tồn tại ⇒ `Reply.caller` treo | `Abort` gỡ hàng endpoint/Reply/Notification/address **trước** khi bật cờ |
| Không có finalise TCB | `finalise_tcb` chạy **trước** `destroy` khi Revoke Untyped: `Dead`, gỡ hàng chờ, trả `sched_context`, ghi log |

## 6. Việc của các task sau (không làm ở KT-18)

- **KT-19**: ready queue/`pick`/preempt — dùng `Ready ↔ Running`, `Resume` không đẩy lại vào hàng;
  finalise gỡ ready queue qua hook (K1b chỉ ghi log).
- **KT-20**: budget — `Running → Sleeping{Budget}`, `Sleeping → Ready` khi refill; thêm
  `schedule`/`consumed` vào hook finalise (trả refill queue).
- **KT-21**: donation — `priority`/`max_controlled_priority`/`sched_context` đã có trường + model API.
- **K1c**: notification `Signal`/`Wait`, `WakeAddr`, fault khác `Exit`, `Stopping` cho 2 hart.
- **K3**: nếu Agent 0 muốn bảng thành **normative** thì chuyển sang `spec/` bằng một D2 mới
  (D2-22), KT-18 không tự làm.
