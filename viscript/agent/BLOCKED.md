# BLOCKED — ViScript

Người ghi: **Agent 2**. Mỗi mục: task, lý do, chặn bởi ai/cái gì, cần gì để mở.
Quy tắc 2.5: kẹt **hai lần** cùng một ý ⇒ ghi vào đây, **đổi nhánh**, không thử lần ba.

## 1. REVIEW — chờ Agent 0 duyệt

| Task | Trạng thái | Nội dung chờ | Cần gì để mở |
|---|---|---|---|
| ~~ST-02a~~ | **DONE** | Đã duyệt 2026-11-12 (SPEC-lex + Phụ lục A sau errata). Không còn chặn ST-04..ST-09 | — |

**ST-02a đã DONE**; ST-04..ST-09 mở theo phụ thuộc. `ST-02b`, `ST-03`, `ST-10` độc lập.

## 2. BLOCKED theo bảng `review/PATCH-BRIEF.md` §2

| Task | Chặn bởi | Cần gì để mở |
|---|---|---|
| ST-03 | ST-01 | khung `viscript/` + `./vis-dev check` chạy; 3 case âm cho luật cấu trúc |
| ST-04 | — | ST-02a DONE |
| ST-05 | ST-04 | lexer xanh bảng token vàng + fuzz 10 phút + NFC/NFD |
| ST-06 | ST-05 | parser + CST lossless (100 % fixture tài liệu parse) |
| ST-07 | ST-05 | như trên |
| ST-08 | ST-05 | như trên |
| ST-09 | ST-06, ST-07, ST-08 | CmdCST + `vis-fmt` + runner `--check` xanh |

Không có phụ thuộc vòng. Không có task nào bị chặn vì thiếu quyết định: QD §7 đã chốt từ vựng, chế độ lệnh, ngữ nghĩa lõi (xem `DECISIONS.md` VD-11..VD-22).

## 3. Câu hỏi cho Agent 0

Không có câu hỏi mở tại thời điểm bàn giao. Khi phát sinh (SPEC thiếu/mâu thuẫn): ghi ở đây kèm dẫn chứng (`tệp:dòng`), đề xuất + **ví dụ cụ thể**, rồi chuyển task kế.

## 4. Dòng OS **không** bị chặn bởi mục 1–2

`v2/agent/QUEUE.md` **không còn** task `READY` (KT-04d REVIEW; KT-05/09/10/11/12/T26 BLOCKED) ⇒ dòng ViScript được chạy (D2-20). Hai dòng độc lập; KT-04d REVIEW không cản dòng ViScript.

Câu hỏi mở của dòng ViScript: **YC-14** trong `v2/agent/BLOCKED.md` §3 (duyệt ST-02a đúng mẫu, xung đột `04xx`, lật ST-02b/ST-03).
