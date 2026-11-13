# KT-22 — giới hạn mô hình: hộp thư FIFO **cố định** là mô hình tất định, không lock-free

**Trạng thái**: model-only `[TK]` (D2-49(f)); `vifast-sync`/MPSC thật + ack thế hệ + `loom`/stress
thuộc **K1c/SMP**; push migration/2 hart **hoãn** (D2-49(a)).

## Đã làm trong KT-22

1. `Mailbox<T>` (`v2/sim/vifast-sim/src/mailbox.rs`): FIFO sức chứa `MAILBOX_CAP = 8` `[TK]`;
   `try_push` khi đầy **trả lại** message (không ghi đè/panic/mất) ⇒ tầng `Machine` quy về
   `KError::Busy` (backpressure, §4.17 "đầy: người gửi xử lý hộp thư của mình rồi thử lại").
   Bộ đếm `pushed/popped/removed` + bất biến `pushed == popped + removed + len`.
2. Hàng gửi Endpoint chuyển từ `VecDeque` **không giới hạn** sang `Mailbox` cố định 8. `Call` đầy
   (khi không có server `Recv` để giao thẳng) ⇒ `Busy` **trước** khi ghi `last_mr`/đổi trạng thái;
   `Exit` có `fault_ep` đầy ⇒ `Busy` **trước** mọi thay đổi (không `exit_code`, không trả donation,
   không gỡ hàng chờ) — message fault không bị "mất một nửa".
3. Hộp thư theo hart (`HartMailbox` + `MailboxMsg {Wake, Reschedule, Migrate, Stop}`):
   `post_mail` = cạnh gửi; `process_mail` = cuối bước (drain FIFO, áp dụng đúng một lần, nhật ký
   theo thứ tự); `send_mail` = một bước ⇒ không có trạng thái "đã pop chưa áp dụng" (DoD 6).
   `Migrate` trên **1 hart** bị từ chối ngay `NotSupported` (không im lặng bỏ).
4. `check_mailboxes` chạy trong `sched_after_op` (sau **mỗi** public op, cùng chỗ `check_sched`):
   bất biến bộ đếm, `len ≤ cap`, "có message ⇒ không Recv waiter", mọi sender ở đúng
   `BlockedOnSend{ep, call/fault}`.

## Chưa làm (follow-up K1c/SMP)

- **Không** MPSC lock-free/atomic: sim 1 luồng; không `loom`, không stress đa luồng (D2-49(f)).
- **Không** ack thế hệ: "idempotent theo thế hệ/ack" (K1B-PLAN §5 KT-22 DoD 1) chưa áp dụng; K1b chỉ
  bảo đảm mỗi message trong hàng được xử lý **đúng một lần** khi `process_mail`.
- **Không** push migration/2 hart (`with_harts(2)`, `home` đổi, chọn hart rảnh nhỏ nhất): D2-49(a)
  hoãn sang K1c nơi có `smp 1/2/4/8`; `Migrate` hiện là **message bị từ chối** trên 1 hart.
- **Không** `TlbFlush`/`VmKick`/`PowerReq` (cần TLB/VCpu/Power của K1c/K2).
- **Không** kiểm 4 cap/badge/`ReplyRecv`/chủ sở hữu `Reply` khi giao message — K1c; hàng gửi chỉ
  giữ `(ObjId, Option<FaultKind>)` như trước KT-22.
- `Stop` chỉ đặt cờ `HartMailbox::stopped` (mô hình); **không** dừng scheduler/tắt hart thật.

## Follow-up đã ghi nhận (từ review độc lập)

- **K1c** — `check_mailboxes` đang mã hoá `call == fault.is_none()` (K1b chỉ có `Call` và fault
  `Exit`); thêm `Send` thường (`call: false, fault: None`) phải nới bất biến.
- **KT-23** — ca `Exit` `Busy` nên snapshot thêm donation/`SchedContext` + `Reply.caller` (KT-22
  chỉ kiểm `exit_code`/state/`len`).
- `Busy` của `Call` không hoàn nguyên `ensure_started` (`Inactive → Ready`, pre-existing từ KT-18);
  test dùng TCB đã `Ready` nên không phụ thuộc chi tiết này.
- **Miri**: `kt22_mailbox` PASS 10/10 trong 123 s; invariant dưới Miri dùng `cfg(miri)` 3 seed × 8
  op (stable 1.000 × 64) — không `#[ignore]`, gate/L1 vẫn 1.000 seed.

## Bất biến kiểm được

- `Mailbox::invariant()`: `pushed == popped + removed + len ≤ cap` — test 1.000 lần push khi đầy
  không làm đổi nội dung/hàng, pop giải phóng đúng một chỗ.
- `check_mailboxes` sau mỗi op + test invariant 1.000 seed × 64 op: message không mất/thêm trên cả
  đường `call`/`recv`/`exit`/`suspend` (huỷ message đếm vào `removed`), `check_sched` vẫn xanh.
