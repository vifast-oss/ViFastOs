# AGENTS.md — dòng ViScript (tóm tắt)

Bản đầy đủ: [`../AGENTS.md`](../AGENTS.md). Tệp này chỉ thêm phần riêng của `viscript/`.
Nguồn sự thật của vòng vá: [`../docs/QUYET-DINH-sau-review.md`](../docs/QUYET-DINH-sau-review.md) (**QD**), nhất là §7.

## 1. Thứ tự nguồn sự thật (áp cho ViScript)

1. **Code + test** trong `viscript/`: crate biên dịch được, golden, fuzz, TAP.
2. **`SPEC`** — [`../docs/viscript/SPEC-lex.md`](../docs/viscript/SPEC-lex.md) (từ vựng + EBNF tự chứa) và `SPEC.md` (do ST-02a/ST-02b sinh); sửa **chỉ** qua Agent 0.
3. [`agent/DECISIONS.md`](agent/DECISIONS.md) — VD-01..VD-10 + VD-11..VD-51 (QD §7, các câu Q1..Q5 của SPEC-lex, và các chỗ văn phạm chế độ lệnh/vis-fmt/vis-diag cần chốt).
4. Tài liệu thiết kế [`../docs/viscript/ViScript-thiet-ke.md`](../docs/viscript/ViScript-thiet-ke.md) — tham chiếu.

QD thắng tài liệu thiết kế.

## 2. Tài liệu và điều khiển

| Tệp | Vai trò |
|---|---|
| [`../docs/viscript/ViScript-thiet-ke.md`](../docs/viscript/ViScript-thiet-ke.md) | thiết kế đầy đủ (9.4 bảng xung đột chế độ lệnh, 14 cấu trúc crate, 20 quy trình, 22 quyết định) |
| [`../docs/viscript/SPEC-lex.md`](../docs/viscript/SPEC-lex.md) | đặc tả từ vựng + EBNF tự chứa — sản phẩm của **ST-02a** |
| [`agent/QUEUE.md`](agent/QUEUE.md) | vòng 1 ST-01..ST-10 (11 dòng) + vòng 2 ST-11..ST-17 (kèm ST-12b/13b/15b) — tất cả `DONE` |
| [`agent/DECISIONS.md`](agent/DECISIONS.md) | VD-01..VD-10 + Q1..Q8 + VD-11..VD-51 |
| [`agent/STATE.md`](agent/STATE.md), [`agent/BLOCKED.md`](agent/BLOCKED.md) | trạng thái + chặn (Agent 2 ghi) |
| `agent/JOURNAL/`, `agent/reports/` | nhật ký ngày, `R<n>-viscript.md` + `.json` |

## 3. Trạng thái duyệt hiện tại

- **ST-02a = APPROVED 2026-11-12 Agent 0 / DONE**: `SPEC-lex.md` + EBNF + bảng ưu tiên + 60 golden token + 30 golden biểu thức (bộ 30 nay ở `viscript/test/expr/golden.expr`, VD-34).
- **ST-04 = DONE** (`663eacb`): lexer `crates/vis-syntax` — 60/60 golden token (đỏ trước), fuzz 10 phút, NFC/NFD.
- **VÒNG 1 XONG (2026-11-12/13, 7 task trong 1 đêm)**: **ST-05** parser + CST lossless (`49f2539`; 30 golden biểu thức, 15/15 khối tài liệu), **ST-08** `vis check` TAP (`2971a72`), **ST-07** `vis-fmt` lossless/idempotent (`b149fe9`), **ST-06** CmdCST + `.vish` vào `vis check` (`590e9f7`; 33 golden chế độ lệnh), **ST-09** cổng `doc-examples` (`ba777e9`; 24 khối/17 xanh + allowlist 7 + 12 fixture QD), **ST-03** `vis-diag` scalar/render/vi-en-ja (`ada1ebc`; 20 snapshot render). **157** test, `gate --fast` PASS, không `#[ignore]`, mọi log đỏ có sha256 khớp. Quyết định: **VD-28..VD-41** (Q1..Q5 SPEC-lex, Unicode, 30 golden biểu thức, sổ cú pháp 02xx, fixture tài liệu, Phụ lục A, vis-fmt vòng 1, chế độ lệnh vòng 1, lỗ hổng đã biết).
- **VÒNG 2 — MỐC VS0 ĐÃ CHỐT (2026-11-13)**: `vis-types` (kiểu cơ bản + HIR + giải tên + kiểm kiểu + `match` xét hết), `vis-interp` (thông dịch cây + builtins lõi), `vis run` (exit 0/1/2, `--tap`), **309 ca ngữ nghĩa/16 nhóm**, **fuzz parser 1 giờ: 182.328.926 ca / 0 vi phạm**, `vis-dev nightly` chạy được (`gate --fast` + fuzz + vòng tròn fmt). **247 test**; quyết định **VD-42..VD-51**; báo cáo **R2** ở `agent/reports/`. Đối chiếu §19: ≥300 test `.vis` ✓, fuzz 1 giờ không panic ✓, vòng tròn fmt ✓, 100% fixture tài liệu parse ✓. Task kế tiếp (khi mở vòng 3): `cmd_lower` + `vis-types` chữ ký lệnh (VD-22), `vis check --lang`, `vis fmt` cho `.vish`, closure theo tham chiếu, nội suy lồng, `match`/`if` làm đối số.
- Không tự sửa `SPEC-lex.md`/`SPEC.md` ngoài phạm vi ST-02a/ST-02b. Không đổi ngữ nghĩa ngôn ngữ: mọi thay đổi cú pháp/quy tắc kiểu qua Agent 0 (VD-08).

## 4. Riêng của dòng ViScript

- Ưu tiên: **OS trước**; dòng ViScript chỉ lấy task khi `v2/agent/QUEUE.md` không còn task READY (D2-20). Ngoại lệ duy nhất: ST-02a (đặc tả, chỉ là tài liệu) chạy song song.
- Vòng 1 **chưa có** interpreter/kiểm kiểu: dừng ở lexer → parser/CST → `CmdCST` → `vis-fmt` → runner `--check` cú pháp.
- Parse chế độ lệnh **hai pha**: `vis-syntax` sinh `CmdCST` → `cmd_lower` (vis-types, dùng bảng chữ ký) → HIR. Tiêu chí "hai chế độ ra cùng AST" đọc là **cùng HIR**; `cmd_lower` là task vòng 2 (VD-22).
- `vis-host-vifast` **không** nằm trong `viscript/`.
- Luật cứng: ≤ 1.500 dòng/tệp; mọi `unsafe` có case âm; SPDX theo loại tệp (`#` cho `.vis .vish .vidl .toml`).
- Ngân sách 3 giờ/task (ST-02a: 3 giờ / 2 phiên).

## 5. Lệnh

```bash
./vis-dev check                        # fmt + clippy + luật + L0
./vis-dev gate --fast                  # cổng bắt buộc cuối mỗi task (có report-check + doc-examples)
./vis-dev nightly [--fuzz-minutes N]   # gate + fuzz parser N phút (mặc định 2) + vòng tròn fmt; --fuzz-minutes 60 cho đêm
```

Commit: `[ST-##] <tóm tắt>`. Duyệt: dòng `APPROVED <YYYY-MM-DD> Agent 0` trong `agent/QUEUE.md`.
