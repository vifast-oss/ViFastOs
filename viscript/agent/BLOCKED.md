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

Khi phát sinh (SPEC thiếu/mâu thuẫn): ghi ở đây kèm dẫn chứng (`tệp:dòng`), đề xuất + **ví dụ cụ thể**, rồi chuyển task kế.
**Không mục nào dưới đây chặn ST-04** — ST-04 đã chọn cách đọc bám §1/§3/§8 và ghi rõ trong code + `JOURNAL`.

| # | Từ task | Dẫn chứng | Vấn đề | Cách ST-04 đang làm | Đề xuất cho Agent 0 |
|---|---|---|---|---|---|
| **Q1** | ST-04 | `docs/viscript/SPEC-lex.md:176`, `:178` | Hai **ví dụ** §8 lược token trong danh sách kỳ vọng: `\tif => !VS0107 INDENT` (thiếu `KW(if)`) và `(1 +\n 2) => INT + INT` (thiếu `(` `)`). Đọc nguyên văn thì không thể dùng làm golden tự động | `.tok` ghi **danh sách đầy đủ** `!VS0107 INDENT KW(if)` và `( INT(1) + INT(2) )`, kèm dòng `//` chép nguyên văn ví dụ gốc ⇒ **siết**, không nới | ghi rõ ở §8 "danh sách kỳ vọng là đầy đủ" (hoặc đánh dấu `…` cho chỗ lược) để ST-05 khỏi lặp lại tranh luận |
| **Q2** | ST-04 | `SPEC-lex.md:23` + `ViScript-thiet-ke.md` §2.4 (bảng escape) | `char` **không** có escape: escape chỉ định nghĩa cho `"…"`, `"""…"""`, `b"…"`, `p"…"`. Hệ quả: `'\n'` là VS0105 (2 scalar) và **không viết được** literal dấu nháy đơn | lexer theo đúng §1/§3 (đúng 1 scalar, không escape) | quyết định vòng 2: hoặc thêm escape cho `char` (§1 + §3), hoặc ghi rõ "dùng `'` = U+0027 qua `\u{27}` trong chuỗi" |
| **Q3** | ST-04 | `SPEC-lex.md:33` vs `:129-131`, `:169-170` | §1 xếp `.. ..= $ ^ # ##` vào `PUNCT`, nhưng §5/§6/§8 đòi kind riêng (`RANGE`/`RANGE_EQ`, `DOLLAR`, `CARET`, `SET_LBRACE`) | `..`/`..=` là `Kind::Punct` (golden nhận bí danh `RANGE`/`RANGE_EQ`); `$`/`^`/`set{` là kind riêng; `#` **không bao giờ** thành `PUNCT` (chú thích thắng) | ghi một dòng ở §1: "`.. ..=` là PUNCT, golden §8 gọi RANGE/RANGE_EQ; `$ ^` là DOLLAR/CARET ở chế độ lệnh; `#` luôn là chú thích" |
| **Q4** | ST-04 | `SPEC-lex.md:28` + `:54` vs `:57` và `ViScript-thiet-ke.md` §2.4 | Phạm vi escape của `b"…"` **mâu thuẫn nguồn**: bảng §1 ("ASCII + `\xNN`") và §3 dòng 54 ("chỉ `\xNN`") nói chỉ có `\xNN`, còn danh sách escape chung ở §3 dòng 57 và thiết kế §2.4 lại xếp `b"…"` vào nhóm có đủ `\n \t … \u{…}` | ST-04 chọn cách đọc **chặt** (SPEC-lex đứng trước thiết kế): chỉ `\xNN`; `b"\n"`, `b"\\"`, `b"\u{41}"` ⇒ **VS0102**. Hệ quả: muốn một byte `\` phải viết `b"\x5c"` | chốt một câu ở §3: dạng `b"…"` có những escape nào (nếu cho đủ bộ thì sửa `decode_bytes` — một chỗ, có test) |
| **Q5** | ST-04 | `SPEC-lex.md:22` (`INT '.' [0-9]+`) vs `:42` (`_` chỉ giữa hai chữ số) | `_` **trong phần lẻ** của `FLOAT`: §1 viết phần lẻ là `[0-9]+` (không có `_`), nhưng §2 lại phát biểu luật `_` chung cho literal số | ST-04 áp luật chung của §2: `1.2_` / `1.2__3` ⇒ VS0104; `1.2_3` **hợp lệ** (FLOAT `1.2_3`, một `_` giữa hai chữ số). Nếu đọc §1 theo nghĩa hẹp thì `1.2_3` phải là `FLOAT(1.2)` + `IDENT(_3)` | chốt: hoặc cấm hẳn `_` trong phần lẻ (§1), hoặc cho phép theo §2 như hiện tại — sửa một chỗ trong `scan_number` |

**Phụ thuộc ngoài của ST-04 (cần Agent 0 ghi nhận, không phải BLOCKED):** `vis-syntax` thêm
`unicode-normalization` (MIT OR Apache-2.0 AND Unicode-3.0) + `unicode-ident` (như trên) + `tinyvec`
(Zlib OR Apache-2.0 OR MIT) — tất cả trong allowlist QD §9.3. Dùng cho NFC toàn tệp (VD-11) và UAX #31
(`IDENT`). Hệ quả: `reports/R1-viscript.md` (ST-10, DONE) ghi "0 phụ thuộc ngoài" **không còn đúng**;
đề nghị ghi lại ở báo cáo vòng sau.

## 4. Dòng OS **không** bị chặn bởi mục 1–2

`v2/agent/QUEUE.md` còn KT-05/KT-12 READY (đã xong, chờ lật); KT-09 BLOCKED; KT-04d/T26 DONE ⇒ dòng ViScript được chạy (D2-20). Hai dòng độc lập; KT-04d REVIEW không cản dòng ViScript.

Câu hỏi mở của dòng ViScript: **YC-14** trong `v2/agent/BLOCKED.md` §3 (duyệt ST-02a đúng mẫu, xung đột `04xx`, lật ST-02b/ST-03).
