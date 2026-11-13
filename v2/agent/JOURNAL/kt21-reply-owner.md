# KT-21 — giới hạn mô hình: `Reply` trên Reply object **không** kiểm chủ sở hữu cap

**Trạng thái**: model-only `[TK]` (D2-49(c)); phần **kiểm chủ sở hữu cap/`ReplyRecv`/4 cap** thuộc **K1c**.

## Hiện tượng

`Machine::reply(tcb, reply_slot)` của mô hình K1b nhận **bất kỳ** cap `Reply` nào; K1b **không**
kiểm TCB gọi `Reply` có đúng là server đã `Recv` message đó không (không có thông tin "Reply thuộc
về ai" trong `ReplyData { caller: Option<ObjId> }`). Vì thế một donee có thể gọi `Reply` trên một
Reply object **khác** (`caller = None` hoặc của message khác).

## Đã làm trong KT-21 (chặn hậu quả rò/treo SC)

1. `reply` **chỉ trả donation** (`return_donation`) khi `caller` của Reply **đúng là donor** của SC
   đang donate (`Machine::donation_donor_of`). Reply nhầm ⇒ **giữ** donation, **không** đánh thức
   nhầm ai; donor vẫn `BlockedOnReply` trên Reply **thật** của mình (vẫn giữ `caller`), nên lần
   Reply đúng sau đó trả SC và đánh thức donor bình thường.
2. Test cảnh báo: `kt21_donation::reply_nham_reply_object_khong_tra_donation` — Reply nhầm
   `rep[1]` ⇒ `sched_donated_to(sc0) == Some(server)`, donor vẫn `BlockedOnReply{rep0}`, server vẫn
   có ngân sách; Reply đúng `rep[0]` ⇒ donation về client + donor `Ready`.

## Chưa làm (follow-up K1c)

- **Không** từ chối `Reply` sai chủ sở hữu (thiếu thông tin trong `ReplyData`); K1c thêm kiểm cap
  (4 cap/badge/`ReplyRecv`) rồi mới trả `IllegalOperation`/`InvalidCapability`.
- Nếu donee Reply nhầm **rồi không bao giờ** Reply đúng: donor + SC vẫn bị giữ (đúng nghĩa "server
  chưa trả lời"); chỉ `Suspend(Abort)`/`Exit`/finalise donee mới huỷ donation + đánh thức donor
  (đã có test ở `suspend_abort_server_dang_donate_tra_sc_va_danh_thuc_client`).
- Mô hình **không** mô hình hoá server phục vụ nhiều client cùng lúc (một donee tại một thời điểm).
