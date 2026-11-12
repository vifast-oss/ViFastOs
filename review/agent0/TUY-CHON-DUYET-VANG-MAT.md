
> **ĐÃ ĐƯỢC AGENT 0 TRẢ LỜI (2026-11-12): xem [`DUYET-YC-02-05.md`](DUYET-YC-02-05.md).** Tệp này giữ làm lịch sử; D2 trong đây đã bị thay bởi bản trong DUYET.

<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# Tùy chọn: duyệt khi Agent 0 vắng mặt

**Mặc định: KHÔNG ủy quyền.** Tệp này chỉ để Agent 0 chọn có ý thức, không phải gợi ý
rằng nên nới cổng. ABI là nền của K1a→K6; duyệt sai ở đây tốn hơn chờ vài giờ.

## Cách 1 — Chờ Agent 2 áp xong rồi duyệt thật (khuyến nghị nếu còn 30 phút)

Agent 2 đã viết 21 `.scn` trong ~1 phút; áp D2-32/33 + máy kiểm + gate có thể xong trong
**20–30 phút**. Agent 0 chờ, đọc diff, ghi `APPROVED` bình thường. Không cần cơ chế mới.

## Cách 2 — Không duyệt, để KT-05 chờ (an toàn nhất nếu phải đi ngay)

- KT-04d giữ `REVIEW`; KT-05 giữ `BLOCKED`.
- Agent 2 chuyển sang việc không phụ thuộc: **KT-06** (agentctl), **KT-08** (sim), **KT-12**
  (traceformat), **KT-T26** (đang làm), **KT-R1**, và dòng ViScript (`ST-01`, `ST-02b`, `ST-10`).
- Đủ việc nhiều giờ; không có chỗ nào buộc phải duyệt thay.
- Khi Agent 0 quay lại: duyệt KT-04d, mở KT-05.

## Cách 3 — Duyệt có điều kiện + hết hạn (chỉ khi thật cần chạy qua đêm)

Agent 0 ký **D2-34**, cho phép ghi dòng APPROVED cho KT-04d **sau khi** máy kiểm và một
review subagent độc lập xác nhận. Đây là ủy quyền thật, có thời hạn, không phải "coi như đã duyệt".

**Điều kiện bắt buộc (đủ cả 5):**
1. `./v2/bin/v2 gate --fast` xanh tại commit xin duyệt.
2. `python3 v2/test/scn/check_scn.py` → **21/21, 0 tệp `blocked`**.
3. `python3 v2/test/abi/check_abi.py` + `check_reserved.py` xanh.
4. Một subagent **không phải Agent 2** xác nhận checklist 5 mục/method (QD §4) không thiếu mục nào.
5. Không nới ngưỡng, không sửa test/kỳ vọng, không bỏ ca.

**D2-34 (câu chữ mẫu):**
> **D2-34** Trong khoảng `<từ>`–`<đến>`, nếu Agent 0 không phản hồi, người điều phối được ghi
> `APPROVED <ngày> Agent 0` cho **KT-04d** khi đủ 5 điều kiện trên; phải ghi kèm commit hash,
> kết quả 3 máy kiểm và tên review subagent vào `v2/agent/JOURNAL/`. Hết hạn quyền tự hết.
> Mọi sửa `spec/` sau APPROVED vẫn chỉ Agent 0 (D2-22). Quyền này **không** áp cho task khác.

**Rủi ro phải chấp nhận:**
- Người ghi APPROVED không phải Agent 0; nếu review subagent bỏ sót, ABI sai sẽ vào KT-05.
- "Checklist 5 mục" là đọc hiểu, không phải máy kiểm — đây là chỗ dễ lọt nhất.
- Nếu Agent 2 tự sửa `spec` khác draft đã review, hash/commit phải bị đối chiếu; lệch ⇒ huỷ duyệt.

## Ký hay không

- **Không ký** = xoá tệp này hoặc ghi `KHÔNG` vào cuối; Agent 2 hiểu là không có ủy quyền.
- **Ký** = thêm D2-34 vào `v2/agent/DECISIONS.md` đúng câu chữ trên (điền ngày), và nói rõ
  trong `QUEUE.md` cạnh KT-04d: `(D2-34: duyệt có điều kiện)`.
- Không có D2-34 thì **không ai** được ghi APPROVED thay, kể cả người điều phối.
