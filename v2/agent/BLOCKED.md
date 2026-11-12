# BLOCKED — ViFastOs v2

Người ghi: **Agent 2**. Mỗi mục: task, lý do, chặn bởi ai/cái gì, cần gì để mở.
Quy tắc 2.5: kẹt **hai lần** cùng một ý ⇒ ghi vào đây, **đổi nhánh**, không thử lần ba.

## 1. REVIEW — chờ Agent 0 duyệt (không phải lỗi kỹ thuật)

| Task | Trạng thái | Nội dung chờ | Cần gì để mở |
|---|---|---|---|
| **KT-04d** | **REVIEW** | Checklist **5 mục/method** + **21 kịch bản** `.scn` viết chỉ bằng ABI (QD §4). Gửi kèm: `spec/abi.toml` lõi (Untyped, CNode, Tcb, SchedContext, Endpoint, Notification, Reply, MessageInfo, lỗi, quy ước gọi riscv64, layout slot 48 B `[TK]`), bảng ánh xạ v1→v2, danh sách object reserved, 21 `.scn` | Đúng một dòng `APPROVED <YYYY-MM-DD> Agent 0` trong `v2/agent/QUEUE.md`. **APPROVED mở KT-05.** Duyệt không bằng chat, không tự suy ra |

**O1 (Exit) — ĐÃ CHỐT, không còn là câu hỏi mở.** QD §1 Q-A: `Exit` là **syscall 14 `Exit(code)`**; kernel lưu `exit_code`, TCB thành **`BlockedOnSend { ep: fault_ep, call: false }`** mang `FaultKind::Exit{code}` — giao qua **hàng gửi của `fault_ep`, không có Reply**; procsrv `Recv` xong TCB **mới** thành `Inactive`; không có `fault_ep` thì `Inactive` ngay. `Resume`/`Configure` xoá `exit_code`. `wait` là RPC của procsrv; `kill` = procsrv `Tcb.Suspend(Abort)` rồi Revoke cap TCB. Dải syscall **15..31 reserved**. (Câu chữ cũ ở mục này nói "chuyển `Inactive` rồi gửi fault" — **sai so với D2-25**; đã sửa 2026-11-12 theo `docs/v2/ViFastOs-v2-thiet-ke.md:285` và `:469`.) Không mở lại O1.

## 2. BLOCKED theo bảng `review/PATCH-BRIEF.md` §1

| Task | Chặn bởi | Cần gì để mở |
|---|---|---|
| KT-03 | KT-02 chưa `DONE` (brief xếp BLOCKED) | KT-02 xong (`cargo build` workspace rỗng xanh) ⇒ chuyển READY |
| KT-04b | KT-04a | `abi.toml` lõi parse được + bảng ánh xạ đủ |
| KT-04c | KT-04a | như trên |
| KT-05 | KT-04a + **APPROVED KT-04d** | dòng `APPROVED <ngày> Agent 0`; gen-check chỉ bật sau duyệt |
| KT-07 | KT-02, KT-03 | khung `v2/` build được + `v2 gate --fast` khung chạy |
| KT-08 | KT-07 | 50 boot riscv64 0 lỗi, có log |
| KT-09 | KT-08 | kịch bản end-to-end Retype→Configure→Call/Reply chạy |
| KT-10 | KT-09 | invariant tổng byte + preempt xanh |
| KT-11 | KT-10 | property test cây + Revoke xanh |
| KT-12 | KT-08 | sim harness chạy được |

Không có phụ thuộc vòng. Không có task nào bị chặn vì thiếu spec/thiếu quyết định: QD đã trả lời Q1..Q12 (QD §9).

## 3. Câu hỏi cho Agent 0

### Q-01 — KT-04c là **20** hay **21** kịch bản? (mở 2026-11-12, bởi Agent 2)

- **Hiện tượng mâu thuẫn nguồn**:
  - **20**: `docs/QUYET-DINH-sau-review.md:176` (QD §4, bảng T04 tách: "KT-04c | 20 kịch bản") và `v2/agent/DECISIONS.md:77` (Phần IV: "số kịch bản ABI | **20** `.scn` | QD §4").
  - **21**: `v2/agent/QUEUE.md:104` ("**21 kịch bản** `.scn` … gồm ca **"Exit khi procsrv chưa Recv"**") và `docs/v2/ViFastOs-v2-thiet-ke.md:1068`; `review/PATCH-BRIEF.md` §1 ghi 20.
- **Vì sao không tự chọn**: số kịch bản là nội dung của bộ ABI gửi duyệt (KT-04d) và là đầu vào KT-05/KT-11. Tự chọn ⇒ có thể đóng băng ABI sai (đúng loại rủi ro reviewer nêu ở mục 6.1 của `JOURNAL/review-2026-11-12-1.md`).
- **Chặn gì**: chỉ chặn `KT-04c` (và con số trong checklist `KT-04d`). **Không** chặn `KT-01`/`KT-02`/`KT-T26`/`KT-R1`; cũng không chặn phần `abi.toml` lõi của `KT-04a` (15 syscall + 19 lỗi + method đã chốt ở QD §4).
- **Đề xuất**: giữ **21** (QUEUE + thiết kế, bản mới hơn sau vòng vá) và Agent 0 ghi một mục **D2-27** xác nhận "21 kịch bản, gồm ca Exit khi procsrv chưa Recv"; nếu chọn 20 thì QUEUE phải sửa dòng KT-04c và bỏ ca Exit-khi-chưa-Recv khỏi DoD. Không có phương án nào Agent 2 tự quyết.

Không có câu hỏi mở nào khác tại thời điểm này. Khi phát sinh: ghi ở đây kèm dẫn chứng (`tệp:dòng`), đề xuất cụ thể, và task kế sẽ làm thay.

## 4. Dòng ViScript **không** bị chặn bởi mục 1–2

`ST-01`, `ST-02b`, `ST-10` đang `READY` và độc lập với KT-04d/KT-03. Khi `v2/agent/QUEUE.md` hết task `READY` thì lấy task ViScript (D2-20); ngoại lệ duy nhất được chạy song song là `ST-02a` (chỉ là tài liệu).
