# AGENTS.md — dòng ViScript (tóm tắt)

Bản đầy đủ: [`../AGENTS.md`](../AGENTS.md). Tệp này chỉ thêm phần riêng của `viscript/`.
Nguồn sự thật của vòng vá: [`../docs/QUYET-DINH-sau-review.md`](../docs/QUYET-DINH-sau-review.md) (**QD**), nhất là §7.

## 1. Thứ tự nguồn sự thật (áp cho ViScript)

1. **Code + test** trong `viscript/`: crate biên dịch được, golden, fuzz, TAP.
2. **`SPEC`** — [`../docs/viscript/SPEC-lex.md`](../docs/viscript/SPEC-lex.md) (từ vựng + EBNF tự chứa) và `SPEC.md` (do ST-02a/ST-02b sinh); sửa **chỉ** qua Agent 0.
3. [`agent/DECISIONS.md`](agent/DECISIONS.md) — VD-01..VD-10 + VD-11..VD-56 (QD §7, các câu Q1..Q5 của SPEC-lex, và các chỗ văn phạm chế độ lệnh/vis-fmt/vis-diag cần chốt).
4. Tài liệu thiết kế [`../docs/viscript/ViScript-thiet-ke.md`](../docs/viscript/ViScript-thiet-ke.md) — tham chiếu.

QD thắng tài liệu thiết kế.

## 2. Tài liệu và điều khiển

| Tệp | Vai trò |
|---|---|
| [`../docs/viscript/ViScript-thiet-ke.md`](../docs/viscript/ViScript-thiet-ke.md) | thiết kế đầy đủ (9.4 bảng xung đột chế độ lệnh, 14 cấu trúc crate, 20 quy trình, 22 quyết định) |
| [`../docs/viscript/SPEC-lex.md`](../docs/viscript/SPEC-lex.md) | đặc tả từ vựng + EBNF tự chứa — sản phẩm của **ST-02a** |
| [`agent/QUEUE.md`](agent/QUEUE.md) | vòng 1 ST-01..ST-10 + vòng 2 ST-11..ST-17 (VS0, kèm 12b/13b/15b) + vòng 3 ST-18..ST-24 (khung VS1, kèm 20b) — tất cả `DONE` |
| [`agent/DECISIONS.md`](agent/DECISIONS.md) | VD-01..VD-10 + Q1..Q8 + VD-11..VD-56 |
| [`agent/STATE.md`](agent/STATE.md), [`agent/BLOCKED.md`](agent/BLOCKED.md) | trạng thái + chặn (Agent 2 ghi) |
| `agent/JOURNAL/`, `agent/reports/` | nhật ký ngày, `R<n>-viscript.md` + `.json` |

## 3. Trạng thái duyệt hiện tại

- **ST-02a = APPROVED 2026-11-12 Agent 0 / DONE**: `SPEC-lex.md` + EBNF + bảng ưu tiên + 60 golden token + 30 golden biểu thức (bộ 30 nay ở `viscript/test/expr/golden.expr`, VD-34).
- **ST-04 = DONE** (`663eacb`): lexer `crates/vis-syntax` — 60/60 golden token (đỏ trước), fuzz 10 phút, NFC/NFD.
- **VÒNG 1 XONG (2026-11-12/13, 7 task trong 1 đêm)**: **ST-05** parser + CST lossless (`49f2539`; 30 golden biểu thức, 15/15 khối tài liệu), **ST-08** `vis check` TAP (`2971a72`), **ST-07** `vis-fmt` lossless/idempotent (`b149fe9`), **ST-06** CmdCST + `.vish` vào `vis check` (`590e9f7`; 33 golden chế độ lệnh), **ST-09** cổng `doc-examples` (`ba777e9`; 24 khối/17 xanh + allowlist 7 + 12 fixture QD), **ST-03** `vis-diag` scalar/render/vi-en-ja (`ada1ebc`; 20 snapshot render). **157** test, `gate --fast` PASS, không `#[ignore]`, mọi log đỏ có sha256 khớp. Quyết định: **VD-28..VD-41** (Q1..Q5 SPEC-lex, Unicode, 30 golden biểu thức, sổ cú pháp 02xx, fixture tài liệu, Phụ lục A, vis-fmt vòng 1, chế độ lệnh vòng 1, lỗ hổng đã biết).
- **VÒNG 3 — KHUNG VS1 ĐÃ CHỐT (2026-11-13)**: `vis-bytecode` (**VBC1** 53 tag, const pool/bảng hàm/span), **`vis-vm`** (fetch–decode–execute, ngăn xếp theo khung, cap 2.000 khung/10^7 lệnh), **verifier** (13 biến thể, kiểm lại độc lập từ byte stream, `load_verified` là cửa duy nhất), **cache** `$XDG_CACHE_HOME/vifast/vis/<hash>.vbc` (FNV-1a×4 — **không** phải SHA-256, xem VD-54), **`vis run --vm`/`--dump-bytecode`/`--no-cache`**, `vis cache --path|--clear`, `bin/vis-diff`. Đo: **318 test**; **323 ca ngữ nghĩa/17 nhóm**; differential gốc **272/272**; **100.000 chương trình sinh tự động 0 lệch**; **khởi động ấm 8,51/9,92/8,64 ms trung vị (p95 ≤ 10,33) ⇒ đạt < 20 ms**; fuzz đột biến 10 phút 0 vi phạm. Quyết định **VD-52..VD-56**; báo cáo **R3**. **Còn VS1b** (generic/trait/module/`vis-std`/`vis-host-posix`, SHA-256 cho consent, variadic/tham chiếu theo tên, đổi mặc định sang VM có đo lại).
- **VÒNG 2 — MỐC VS0 ĐÃ CHỐT (2026-11-13)**: `vis-types` (HIR + kiểu + giải tên + kiểm kiểu + `match` xét hết),
  `vis-interp` (thông dịch cây + builtins lõi), `vis run` (exit 0/1/2, `--tap`), **309 ca ngữ nghĩa/16 nhóm**,
  **fuzz parser 1 giờ: 182.328.926 ca / 0 vi phạm**, `vis-dev nightly` chạy được. **247 test**; **VD-42..VD-51**; **R2**.
  Đối chiếu §19 VS0: ≥300 ca ✓ · fuzz 1 giờ ✓ · vòng tròn fmt ✓ · 100% fixture tài liệu parse ✓.
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
./bin/vis-diff --n 100000            # differential interp ↔ VM (cổng chỉ chạy 1.000)
vis cache --path | vis cache --clear # cache bytecode (mặc định dùng khi `--vm`)
```

Commit: `[ST-##] <tóm tắt>`. Duyệt: dòng `APPROVED <YYYY-MM-DD> Agent 0` trong `agent/QUEUE.md`.
