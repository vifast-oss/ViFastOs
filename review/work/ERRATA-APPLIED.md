# ERRATA ĐÃ ÁP — theo `docs/DUYET-AGENT0-vong-va.md`

Ngày 2026-11-12. Người áp: Agent 1. Phạm vi: E1–E5, S1–S11, L1–L8, Q1–Q7.
Kết quả kiểm: xem `review/work/CHECK.md`. Chưa ghi APPROVED/D2-25 (việc của người điều phối, DUYET §5).

## 1. ViFastOs v2

| Mã | Đã sửa |
|---|---|
| E1 | Xoá dòng "Hàng chờ nằm trong header Frame…" ở §4.7; chỉ còn bảng băm arena |
| E2 | Exit: lưu `exit_code: Option<u64>`; có `fault_ep` → `BlockedOnSend{ep, call:false}` mang `FaultKind::Exit{code}`, procsrv `Recv` xong mới `Inactive`; không `fault_ep` → `Inactive`. Sửa §4.4, §4.6, §4.21 (dòng 1/14 + bảng Exit/kill/wait), §6.4. `Resume`/`Configure` xoá `exit_code`; `Suspend(Abort)` huỷ message nhưng giữ `exit_code` |
| E3 | `ThreadState::Sleeping { reason: SleepReason }`, thêm `enum SleepReason { Timeout, Budget }`; bảng ngân sách dùng đúng tên |
| E4 | Đóng băng thanh ghi: **riscv64 ngay**; aarch64/x86_64 khi port (K4'/K5'), chỉ thêm, không đổi riscv64 |
| E5 | Bỏ "(QD §5.3)" sau "3 QEMU", thay `[TK]` |

## 2. ViScript

| Mã | Đã sửa |
|---|---|
| S1 | §22 chép **nguyên văn** 26 mục từ `viscript/agent/DECISIONS.md`; thêm VD-23..VD-26; VD-02 "cùng HIR"; hai danh sách giống hệt (đã kiểm) |
| S2 | Q6: "bằng chứng nhận" → "điều khoản **bằng sáng chế** (patent grant)" |
| S3 | Bỏ link `review/work/PATCH-NOTES-viscript.md` khỏi đầu tài liệu; `docs/` không còn link `review/` |
| S4 | `cap[Net] @ "api.example.com"` dùng chuỗi host; `cap[Dir]` dùng `p"..."`; ghi rõ kiểm loại ở pha kiểm kiểu |
| S5 | Fence `extern cmd git(...)` đổi `python` → `vis` |
| S6 | `SIZE`, `DURATION`, `PERCENT` là terminal một token; bỏ `size_lit`/`duration_lit`/`percent_lit`, ghi chú đơn vị |
| S7 | `range_pattern := atom_pattern [ '..' [atom_pattern] | '..=' atom_pattern ]` (mở trên `18..`) |
| S8 | `NEWLINE` chỉ nối khi dòng **bắt đầu** bằng `|>` ở độ sâu 0; bỏ vế "dòng trước kết thúc bằng `|>`" |
| S9 | `break_stmt` và `continue_stmt` định nghĩa riêng |
| S10 | Ghi chú lambda `fn(...) -> T:` nhiều dòng chỉ ở cấp câu lệnh; trong ngoặc chỉ `=>`/một dòng |
| S11 | Thêm `shell_let := 'let' IDENT [':' type] '=' pipeline NEWLINE`; dùng trong `shell_item` (cả §9.3 và Phụ lục A) |

## 3. SPEC-lex

| Mã | Đã sửa |
|---|---|
| L1 | `PATH`: có escape, **có nội suy**; là literal `path` |
| L2 | `0b`/`0x` là tiền tố cơ số chỉ khi theo sau là chữ số hợp lệ; `0b` đơn lẻ = `SIZE(0,b)`; thêm golden `0b101`, `0b` |
| L3 | Golden `50%` = `INT(50) PUNCT(%)` (lỗi ở parser, gợi ý `50pct`); thêm ca vào 30 golden biểu thức ST-05 |
| L4 | `set{` → token `SET_LBRACE`; `set {` = `IDENT(set)` + `PUNCT({)`; golden sửa theo |
| L5 | Bỏ golden `if x:  y`; thay bằng ca thụt lề thật (`if x:\n    y\n  z => !VS0109`) |
| L6 | `@` luôn là `PUNCT(@)`; VS0110 chỉ còn `^` |
| L7 | `PERCENT` nhận cả `FLOAT` (`12.5pct`) |
| L8 | **Đúng 60 golden** sau khi sửa (đã đếm) |

## 4. QUEUE / DECISIONS / AGENTS

| Mã | Đã sửa |
|---|---|
| Q1 | ST-02a: tiêu chí "áp errata + APPROVED", **không phụ thuộc ST-01**, sau APPROVED là **DONE**; ST-04 thêm "60 golden token đỏ trước"; ST-05 thêm "30 golden biểu thức đỏ trước"; ST-09 giữ cổng doc-examples parse |
| Q2 | KT-08 phụ thuộc **KT-02**; tiêu chí harness + store + scheduler + kịch bản rỗng tất định; KT-09 phụ thuộc **KT-08 + KT-05**, nhận end-to-end Retype→Configure→Call/Reply; KT-05 phụ thuộc **KT-04d**; KT-12 phụ thuộc **KT-02** |
| Q3 | KT-04c: **21 kịch bản**, kiểm tĩnh, có ca "Exit khi procsrv chưa Recv"; KT-11 chạy ca Untyped/CNode của KT-04c |
| Q4 | Khối A: SPDX **theo loại tệp**, bỏ câu `//` cho mọi tệp |
| Q5 | D2-24 + bảng Phần IV dùng `n ≥ 3/p`; DECISIONS còn **đúng 24 bullet D2** (các lần grep 26 là do tham chiếu chéo, không phải mục trùng) |
| Q6 | AGENTS: bỏ tên SA-A/SA-B/SA-C; bỏ số tệp cứng 101; ghi "toàn bộ báo cáo agent2 của v1"; D2-25/VD-26 |
| Q7 | QUEUE v2 thêm ghi chú: KT-04d chỉ duyệt khi có sản phẩm KT-04a/b/c; duyệt tài liệu ghi bằng D2-25 |

## 5. Kiểm tra cuối

Xem `CHECK.md`: 4 kiểm DUYET §5 đều đạt — không còn "header Frame"; VD-01..VD-26 giống hệt hai nơi; `docs/` không còn link `review/`; DECISIONS đúng 24 D2 trước khi thêm D2-25.

## 6. Việc còn lại (người điều phối, DUYET §5)

1. Thêm D2-25 vào `v2/agent/DECISIONS.md`.
2. Đặt ST-02a `DONE` + dòng `APPROVED 2026-11-12 Agent 0` trong `viscript/agent/QUEUE.md`.
3. **Không** ghi gì dưới KT-04d.
4. Commit `[KT-00] vòng vá sau review + duyệt Agent 0`.
5. Giao Agent 2.
