# AGENTS.md — dòng ViScript (tóm tắt)

Bản đầy đủ: [`../AGENTS.md`](../AGENTS.md). Tệp này chỉ thêm phần riêng của `viscript/`.
Nguồn sự thật của vòng vá: [`../docs/QUYET-DINH-sau-review.md`](../docs/QUYET-DINH-sau-review.md) (**QD**), nhất là §7.

## 1. Thứ tự nguồn sự thật (áp cho ViScript)

1. **Code + test** trong `viscript/`: crate biên dịch được, golden, fuzz, TAP.
2. **`SPEC`** — [`../docs/viscript/SPEC-lex.md`](../docs/viscript/SPEC-lex.md) (từ vựng + EBNF tự chứa) và `SPEC.md` (do ST-02a/ST-02b sinh); sửa **chỉ** qua Agent 0.
3. [`agent/DECISIONS.md`](agent/DECISIONS.md) — VD-01..VD-10 + VD-11..VD-22 (chốt từ QD §7).
4. Tài liệu thiết kế [`../docs/viscript/ViScript-thiet-ke.md`](../docs/viscript/ViScript-thiet-ke.md) — tham chiếu.

QD thắng tài liệu thiết kế.

## 2. Tài liệu và điều khiển

| Tệp | Vai trò |
|---|---|
| [`../docs/viscript/ViScript-thiet-ke.md`](../docs/viscript/ViScript-thiet-ke.md) | thiết kế đầy đủ (9.4 bảng xung đột chế độ lệnh, 14 cấu trúc crate, 20 quy trình, 22 quyết định) |
| [`../docs/viscript/SPEC-lex.md`](../docs/viscript/SPEC-lex.md) | đặc tả từ vựng + EBNF tự chứa — sản phẩm của **ST-02a** |
| [`agent/QUEUE.md`](agent/QUEUE.md) | 10 task vòng 1: ST-01..ST-10 (ST-02 tách a/b ⇒ 11 dòng) |
| [`agent/DECISIONS.md`](agent/DECISIONS.md) | VD-01..VD-10 + Q1..Q8 + VD-11..VD-22 |
| [`agent/STATE.md`](agent/STATE.md), [`agent/BLOCKED.md`](agent/BLOCKED.md) | trạng thái + chặn (Agent 2 ghi) |
| `agent/JOURNAL/`, `agent/reports/` | nhật ký ngày, `R<n>-viscript.md` + `.json` |

## 3. Trạng thái duyệt hiện tại

- **ST-02a = REVIEW**: `SPEC-lex.md` + EBNF tự chứa + bảng ưu tiên + 60 golden token + 30 golden biểu thức. **Chặn ST-04..ST-09**; **không** chặn `ST-02b`, `ST-03`, `ST-10`. Chờ dòng `APPROVED <YYYY-MM-DD> Agent 0` trong [`agent/QUEUE.md`](agent/QUEUE.md).
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
./vis-dev check
./vis-dev gate --fast
./vis-dev nightly
```

Commit: `[ST-##] <tóm tắt>`. Duyệt: dòng `APPROVED <YYYY-MM-DD> Agent 0` trong `agent/QUEUE.md`.
