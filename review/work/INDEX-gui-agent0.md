# Gói gửi Agent 0 — vòng vá 2026-11-12

## Đọc trước (1 tệp)
- `review/work/DIFF-SUMMARY.md` — việc đã làm, đối chiếu QD, 7 điểm cần xác nhận, đề nghị duyệt.

## Bắt buộc duyệt (theo QD §10.5)
- `docs/v2/ViFastOs-v2-thiet-ke.md` (bản đầy đủ, 1.131 dòng)
- `docs/viscript/ViScript-thiet-ke.md` (bản đầy đủ, 1.457 dòng)
- `review/work/ViFastOs-v2.diff` (+321 dòng) — nếu muốn đọc thay đổi
- `review/work/ViScript.diff` (+684 dòng)
- `v2/agent/QUEUE.md` (17 dòng KT, 5 khối tự chứa)
- `viscript/agent/QUEUE.md` (11 dòng ST)

## Cần duyệt vì là đặc tả mới
- `docs/viscript/SPEC-lex.md` (180 dòng) — nguồn từ vựng; ST-02a duyệt
- Phụ lục A trong `ViScript-thiet-ke.md` (~122 luật EBNF tự chứa)

## Nên xem để khớp quyết định
- `v2/agent/DECISIONS.md` — D-01..D-08 + D2-01..24
- `viscript/agent/DECISIONS.md` — VD-01..22
- `docs/v1/KNOWN-ISSUES.md` — 7 lỗi v1 → ca kiểm v2
- `AGENTS.md` — luật chơi, lệnh cổng, commit tag, duyệt APPROVED
- `review/work/CHECK.md` — kết quả kiểm tự động sau vá

## Không gửi kèm
- `docs/v1/agent2-report-*` (101 tệp), `docs/v1/decisions/`, `docs/v1/overnight-log.md`
- `review/src/`, `review/work/snapshot-*`, báo cáo A–D (chỉ gửi nếu Agent 0 hỏi)

## Đề nghị duyệt
1. Nội dung 2 tài liệu + SPEC-lex/Phụ lục A.
2. Ghi `APPROVED <YYYY-MM-DD> Agent 0` vào `v2/agent/QUEUE.md` dưới KT-04d và `viscript/agent/QUEUE.md` dưới ST-02a.
3. Xác nhận 3 điểm diễn giải ở DIFF-SUMMARY §4 (ST-02a, KT-04c/mock, exit_code trong ReadState).
