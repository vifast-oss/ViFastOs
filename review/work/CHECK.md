# CHECK sau errata Agent 0 — 2026-11-12

## 4 kiểm bắt buộc (DUYET §5)

| # | Kiểm | Kết quả |
|---|---|---|
| 1 | Không còn chuỗi "header Frame" trong `docs/v2/` | **0** — ĐẠT |
| 2 | VD-01..VD-26 giống hệt giữa `ViScript-thiet-ke.md §22` và `viscript/agent/DECISIONS.md` | **26/26, text trùng khớp** — ĐẠT |
| 3 | Không còn link `review/` trong `docs/` | **0 tệp** — ĐẠT |
| 4 | `v2/agent/DECISIONS.md` đúng D2-01..D2-27 | **27/27 bullet** — ĐẠT |

## ViFastOs v2 (`docs/v2/ViFastOs-v2-thiet-ke.md`, 1.132 dòng)

- D2 rows 24/24; task KT 17/17; syscall 15/15 (có `| 14 | Exit(code)`).
- Exit: 4 chỗ dùng `BlockedOnSend`; 0 chỗ còn "dạng Send"; `ReadState` có `exit_code: Option<u64>`.
- `Sleeping { reason: SleepReason }` + `enum SleepReason`; freeze "riscv64 đóng băng ngay"; 0 "(QD §5.3)" sau 3 QEMU.
- KT-04c 21 kịch bản; KT-08 phụ thuộc KT-02; KT-05 phụ thuộc KT-04d; KT-12 phụ thuộc KT-02.
- 0 task T cũ; phụ lục Q1–Q12 + sổ lỗi v1.

## ViScript (`docs/viscript/ViScript-thiet-ke.md`, 1.465 dòng)

- 26 dòng VD; 11 dòng ST; 121 luật `:=` trong Phụ lục A.
- `size_lit`/`duration_lit`/`percent_lit`: 0; range mở `18..` có; `break_stmt`/`continue_stmt` riêng; `shell_let` có.
- `docs/` không link `review/`; Q6 "bằng sáng chế"; `cap[Net] @ "api.example.com"`.

## SPEC-lex (`docs/viscript/SPEC-lex.md`, 183 dòng)

- **60 golden** đúng; `SET_LBRACE`; `12.5pct`; `@` luôn `PUNCT(@)`; `50%` là `INT(50) PUNCT(%)` (lỗi parser).

## QUEUE / AGENTS

- QUEUE v2: KT-04c 21; KT-05←KT-04d; KT-08←KT-02; note KT-04d + D2-25.
- QUEUE ViScript: ST-02a không phụ thuộc; ST-04 "60 golden token đỏ trước"; ST-05 "30 golden biểu thức đỏ trước"; ST-09 cổng doc-examples.
- AGENTS: 0 "SA-A"; 0 số tệp cứng 101; D2-25/VD-26; 21 kịch bản ở KT-04d.

## Kết luận

**CHECK xanh.** D2-25 đã ghi; ST-02a `DONE` + `APPROVED 2026-11-12 Agent 0`; đủ điều kiện giao Agent 2.
