# AGENTS.md — dòng ViScript (tóm tắt)

Bản đầy đủ: [`../AGENTS.md`](../AGENTS.md). Tệp này chỉ thêm phần riêng của `viscript/`.
Nguồn sự thật của vòng vá: [`../docs/QUYET-DINH-sau-review.md`](../docs/QUYET-DINH-sau-review.md) (**QD**), nhất là §7.

## 1. Thứ tự nguồn sự thật (áp cho ViScript)

1. **Code + test** trong `viscript/`: crate biên dịch được, golden, fuzz, TAP.
2. **`SPEC`** — [`../docs/viscript/SPEC-lex.md`](../docs/viscript/SPEC-lex.md) (từ vựng + EBNF tự chứa) và `SPEC.md` (do ST-02a/ST-02b sinh); sửa **chỉ** qua Agent 0.
3. [`agent/DECISIONS.md`](agent/DECISIONS.md) — VD-01..VD-10 + VD-11..VD-63 (QD §7, các câu Q1..Q5 của SPEC-lex, và các chỗ văn phạm chế độ lệnh/vis-fmt/vis-diag cần chốt).
4. Tài liệu thiết kế [`../docs/viscript/ViScript-thiet-ke.md`](../docs/viscript/ViScript-thiet-ke.md) — tham chiếu.

QD thắng tài liệu thiết kế.

## 2. Tài liệu và điều khiển

| Tệp | Vai trò |
|---|---|
| [`../docs/viscript/ViScript-thiet-ke.md`](../docs/viscript/ViScript-thiet-ke.md) | thiết kế đầy đủ (9.4 bảng xung đột chế độ lệnh, 14 cấu trúc crate, 20 quy trình, 22 quyết định) |
| [`../docs/viscript/SPEC-lex.md`](../docs/viscript/SPEC-lex.md) | đặc tả từ vựng + EBNF tự chứa — sản phẩm của **ST-02a** |
| [`agent/QUEUE.md`](agent/QUEUE.md) | vòng 1 ST-01..ST-10 + vòng 2 VS0 + vòng 3 khung VS1 (kèm 12b/13b/15b/20b) + vòng 4 VS1b ST-25..ST-31 (kèm 30b) — **36/36 `DONE`** |
| [`agent/DECISIONS.md`](agent/DECISIONS.md) | VD-01..VD-10 + Q1..Q8 + VD-11..VD-63 |
| [`agent/STATE.md`](agent/STATE.md), [`agent/BLOCKED.md`](agent/BLOCKED.md) | trạng thái + chặn (Agent 2 ghi) |
| `agent/JOURNAL/`, `agent/reports/` | nhật ký ngày, `R<n>-viscript.md` + `.json` |

## 3. Trạng thái duyệt hiện tại

- **ST-02a = APPROVED 2026-11-12 Agent 0 / DONE**: `SPEC-lex.md` + EBNF + bảng ưu tiên + 60 golden token + 30 golden biểu thức (bộ 30 nay ở `viscript/test/expr/golden.expr`, VD-34).
- **ST-04 = DONE** (`663eacb`): lexer `crates/vis-syntax` — 60/60 golden token (đỏ trước), fuzz 10 phút, NFC/NFD.
- **VÒNG 1 XONG (2026-11-12/13, 7 task trong 1 đêm)**: **ST-05** parser + CST lossless (`49f2539`; 30 golden biểu thức, 15/15 khối tài liệu), **ST-08** `vis check` TAP (`2971a72`), **ST-07** `vis-fmt` lossless/idempotent (`b149fe9`), **ST-06** CmdCST + `.vish` vào `vis check` (`590e9f7`; 33 golden chế độ lệnh), **ST-09** cổng `doc-examples` (`ba777e9`; 24 khối/17 xanh + allowlist 7 + 12 fixture QD), **ST-03** `vis-diag` scalar/render/vi-en-ja (`ada1ebc`; 20 snapshot render). **157** test, `gate --fast` PASS, không `#[ignore]`, mọi log đỏ có sha256 khớp. Quyết định: **VD-28..VD-41** (Q1..Q5 SPEC-lex, Unicode, 30 golden biểu thức, sổ cú pháp 02xx, fixture tài liệu, Phụ lục A, vis-fmt vòng 1, chế độ lệnh vòng 1, lỗ hổng đã biết).
- **VÒNG 4 — VS1b ĐÃ CHỐT (2026-11-13)**: **module** (`use ./x`, đa tệp, cấm vòng, hash closure gồm phụ thuộc, sổ `07xx`), **`vis-std` lõi** (`std.str/list/map/path/time/json`, 70 ca), **`vis-host-posix`** (chương trình ngoài §9.3, variadic, `--x=v`, `trait Ngoai` giữ biên OS), **generic `[T]`** (xoá kiểu + hợp nhất call-site), **trait/impl** (dispatch tĩnh, chạy cả interp và VM), **`vis test`** TAP + **SHA-256** thật cho hash nguồn/cache, **đồng bộ chẩn đoán** hai đường. Đo: **436 test**, **338 ca/18 nhóm**, parity **312/312**, differential **100k 0 lệch**; **VD-57..VD-63**; **R4**. Còn **VS2** (async/kênh/net/Landlock/LSP/gói) + 13 nợ trong `R4` §5.
- **VÒNG 3 — KHUNG VS1 ĐÃ CHỐT (2026-11-13)**