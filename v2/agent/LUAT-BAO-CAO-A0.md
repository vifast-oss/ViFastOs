# LUẬT BÁO CÁO CỦA AGENT 0 (1–4)

**Nguồn**: phiếu quyết định Agent 0 các lượt 4 và 5 (2026-11-18). Tệp này chỉ **chép lại** luật;
không thêm, không bớt, không diễn giải rộng hơn văn bản gốc.

> **Đừng lẫn với "luật 1/2/3/7/8" của `v2/tools/verify-numbers.py`** (KT-80) — đó là năm luật **máy**
> kiểm số đo trong tài liệu. Bốn luật dưới đây là luật **cách viết báo cáo** cho Agent 0.

---

## Luật 1 — số phải dán nguyên văn

Mọi con số trong báo cáo phải được **dán nguyên văn** từ output công cụ, kèm **lệnh đã chạy**.
Không làm tròn, không tính lại, không chép từ trí nhớ, không lấy số của lần chạy khác.

Hệ quả đã gặp: dòng `HEAD` phải dán từ **cùng một lần chạy** `v2 status` với các dòng khác trong
cùng khối — không được ghép HEAD của lần chạy này với số task của lần chạy khác.

## Luật 2 — đổi định nghĩa/ phạm vi của một cổng ⇒ quét lại **toàn bộ** phạm vi

Mỗi khi định nghĩa hay phạm vi của một cổng thay đổi, phải **quét lại toàn bộ** phạm vi ấy (không
chỉ chỗ vừa sửa) và ghi kết quả quét vào `v2/agent/JOURNAL/`.

## Luật 3 — "đã chứng minh" phải kèm ba thứ

Chữ **"đã chứng minh"** / **"đã xác nhận"** chỉ được dùng khi **ngay sau đó** có đủ:

1. **lệnh đã chạy**,
2. **dòng output** (nguyên văn),
3. **loại cây**: *worktree sạch tại `<sha>`* hay *cây làm việc*.

Thiếu một trong ba ⇒ phải ghi **"chưa kiểm"**. Không được viết "đã chứng minh" rồi để trống.

## Luật 4 — vế "đỏ trên mã cũ" phải chạy trên **ảnh** build từ mã cũ

Mọi lỗi K7: vế 1 (đỏ trên mã cũ) phải chạy trên **ảnh build từ commit cũ**, không phải trên log
chép tay, không phải trên fixture của selftest. Phải dán **dòng log** + **sha ảnh**.

Log SAI trong selftest được giữ lại, nhưng nó chứng minh **checker**, không thay được vế này.

---

## Bảng đối chiếu nhanh

| luật | câu hỏi tự kiểm trước khi gửi |
|---|---|
| 1 | Con số này dán từ output nào? Lệnh nào? Cùng lần chạy với các số cạnh nó chưa? |
| 2 | Cổng này vừa đổi định nghĩa/phạm vi? Đã quét **toàn bộ** phạm vi chưa? Ghi JOURNAL chưa? |
| 3 | Chỗ nào viết "đã chứng minh"? Ngay sau nó có lệnh + output + loại cây chưa? |
| 4 | Vế "đỏ trên mã cũ" chạy trên **ảnh** build từ commit cũ chưa? Có sha ảnh chưa? |
