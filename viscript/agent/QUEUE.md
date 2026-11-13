# QUEUE — ViScript, vòng 1

Người ghi: **Agent 0** (qua người điều phối). Người đọc/thi hành: **Agent 2** dòng ViScript.
Nguồn: `docs/QUYET-DINH-sau-review.md` (QD) §7, `review/PATCH-BRIEF.md` §2, `docs/viscript/ViScript-thiet-ke.md` §20.2.
Trạng thái hợp lệ: `READY | BLOCKED | REVIEW | DONE`.
**Tệp này tự chứa**: đọc hết 5 khối dưới đây là đủ để chạy một task mà không cần mở tài liệu khác.

---

## Khối A — Luật chơi (bắt buộc)

**Vòng lặp 8 bước cho mỗi task**

1. **Đọc** `STATE.md`, `QUEUE.md`, `DECISIONS.md`, phần SPEC liên quan. Không đọc lại toàn repo.
2. **Lập kế hoạch nhỏ** trong `STATE.md`: các bước, tệp sẽ chạm, test sẽ thêm.
3. **Test đỏ trước**: viết test/kịch bản thể hiện yêu cầu; chạy, xác nhận đỏ **đúng lý do**.
4. **Cài đặt** theo bước nhỏ; sau mỗi bước chạy `./vis-dev check`.
5. **Cổng**: `./vis-dev gate --fast` (luôn).
6. **Commit**: `[ST-##] <tóm tắt>` + kết quả cổng + `BUILD_ID`.
7. **Cập nhật** `STATE.md`, `JOURNAL/YYYY-MM-DD.md`; `vifast-agentctl --project viscript done <id>`.
8. **Nén ngữ cảnh** nếu lớn, rồi quay lại bước 1 với task kế.

**Cấm (9 điều)**

1. Sửa `SPEC-lex.md` / `SPEC.md` ngoài phạm vi `ST-02a`/`ST-02b`.
2. Sửa `accept()` / kỳ vọng / snapshot golden để test xanh.
3. `#[ignore]`, tắt test, bỏ case khỏi danh sách chạy, dùng `...` trong fixture (QD §7.4 cấm).
4. Nới ngưỡng (thời gian fuzz, số golden, số fixture).
5. Retry để qua cổng; flake là `T-INFRA`, ghi log, chạy lại tối đa **một** lần chỉ để phân loại.
6. Đổi ngữ nghĩa ngôn ngữ (cú pháp, quy tắc kiểu) mà không qua Agent 0 — bắt buộc `BLOCKED` + đề xuất + ví dụ (VD-08).
7. Thêm `unsafe` mà không có case âm; vượt **1.500 dòng/tệp**.
8. Đưa số đo viết tay vào tài liệu (số đo do `./vis-dev nightly` sinh).
9. Sửa `DECISIONS.md` hoặc QUEUE của dòng OS.

**Bắt buộc**: hồi quy ⇒ **revert trước**, điều tra sau; lỗi từ fuzz/vi sai ⇒ thu nhỏ, ghi `tests/regress/`, test đỏ trước rồi mới sửa; `cargo fmt` trước commit; SPDX theo loại tệp (`#` cho `.vis .vish .vidl .toml`, `//` cho Rust); `vis-host-vifast` **không** nằm trong `viscript/`.

## Khối B — Ngân sách và điều kiện dừng (3 giờ/task)

| Tình huống | Hành động |
|---|---|
| Task vượt **3 giờ** (ST-02a: 3 giờ / **2 phiên**) | Dừng, ghi tiến độ + lý do, chuyển task kế không phụ thuộc |
| Kẹt **hai lần** cùng một ý | `BLOCKED` + câu hỏi cho Agent 0, đổi nhánh (quy tắc 2.5) |
| SPEC mâu thuẫn/thiếu | `BLOCKED` kèm đề xuất + ví dụ cụ thể, **không** tự đổi SPEC |
| Cổng đỏ do lỗi không thuộc task | Mục lỗi trong `JOURNAL`, **không** sửa lan |
| Hết task khả dụng | Chạy `./vis-dev nightly`, viết báo cáo, dừng |

**Mỗi task ≤ 3 giờ. Task lớn hơn phải tách.**

## Khối C — Định nghĩa "xong" (DoD)

1. Tiêu chí trong bảng dưới đạt, **có bằng chứng** (đầu ra lệnh, số golden, TAP, snapshot).
2. Test mới **đã từng đỏ** (dán log đỏ vào `JOURNAL`).
3. Cổng tương ứng **xanh, không retry**.
4. `STATE.md` + `JOURNAL/` cập nhật; không còn TODO không có task.

## Khối D — Lệnh cổng

```bash
./vis-dev check          # fmt + clippy + luật cấu trúc (≤1.500 dòng/tệp, unsafe có case âm, SPDX)
./vis-dev gate --fast    # BẮT BUỘC cuối mỗi task: check + L0 + golden hiện có
./vis-dev nightly        # fuzz dài, snapshot toàn bộ, benchmark — không chạy trong vòng task
```

- Dòng ViScript **không có** công cụ cho tới khi `VT01`/`ST-01` dựng khung: giai đoạn đầu ghi "cổng chưa có" vào `STATE.md`.
- Fuzz 10 phút là **một phần tiêu chí** của ST-04, không phải cổng thường ngày.

## Khối E — Định dạng báo cáo

`reports/R<n>-viscript.md` — 6 mục: (1) tóm tắt 5 dòng; (2) bảng task `ID | trạng thái | commit | cổng`; (3) lỗi tìm thấy (fixture/`.regress`); (4) số đo thay đổi; (5) golden/fixture đã thêm; (6) `BLOCKED` + câu hỏi + đề xuất task kế.

`reports/R<n>-viscript.json` — cùng schema với dòng OS, **có `project: "viscript"`**:

```json
{
  "project": "viscript",
  "round": 1,
  "head": "ca2eaf5",
  "build_id": "—",
  "tasks": [{"id": "ST-01", "status": "DONE", "commits": ["…"], "gates": ["fast"]}],
  "gates": {"fast": "PASS", "full": "—", "nightly": "—"},
  "sim": {"seeds": 0, "failures": 0},
  "coverage": {"tokens": "—", "fixtures": "—"},
  "mutants": {"killed": 0, "alive": 0},
  "perf": {},
  "blocked": []
}
```

Agent 0 đọc **JSON trước**, MD sau. JSON thiếu `project` bị trả lại.

---

## Bảng task vòng 1

| ID | Task | Phụ thuộc | Ngân sách | Trạng thái | Tiêu chí xong |
|---|---|---|---|---|---|
| ST-01 | workspace `viscript/` + `viscript/agent/` (không ghi đè) + SPDX theo loại tệp + cổng 1.500 dòng/unsafe có case âm + `vis-host-vifast` **không** nằm trong `viscript/` | — | 3 h | DONE | cổng chạy; 3 case âm |
| ST-02a | **Áp errata mục 3 (S1..S11, L1..L8) + APPROVED**; SPEC-lex + Phụ lục A + bảng ưu tiên do Agent 1 soạn | — | 3 h (2 phiên) | **DONE** | errata áp xong; **APPROVED 2026-11-12 Agent 0** |
| ST-02b | sổ mã lỗi VS + 4 chữ số + quy tắc cấp | ST-01 | 2 h | DONE | bảng mã; `vis explain` được |
| ST-03 | `vis-diag`: cột theo Unicode scalar, render, bảng vi/en/ja | ST-01 | 3 h | **DONE** | snapshot 20 ca, có tiếng Việt/emoji |
| ST-11 | `vis-types` nền: `Type`, HIR, giải tên, hạ CST→HIR (script) | ST-05 | 3 h | **DONE** | HIR cho 15 khối tài liệu + 30 golden biểu thức; lỗi tên `VS03xx`; đỏ trước |
| ST-12 | `vis-types`: kiểm kiểu VS0 + `match` xét hết | ST-11 | 3 h | **DONE** | `VS04xx`; ≥ 80 ca kiểu (đúng/sai đều có) |
| ST-12b | sửa 7 lỗi kiểm kiểu (review ST-12) + coverage | ST-12 | 1,5 h | **DONE** | 7 mục PHẢI SỬA + tệp `tests/tyck_v2.rs`; đỏ trước |
| ST-13 | `vis-interp`: thông dịch cây trên HIR + builtins lõi | ST-12 | 3 h | **DONE** | chạy được chương trình VS0; `VS05xx` runtime |
| ST-14 | `vis run` + TAP harness ngữ nghĩa + 100 ca đầu | ST-13 | 3 h | **DONE** | `vis run <tệp.vis>`, exit 0/1/2 |
| ST-15b | sửa 4 lệch thiết kế (push/CoW, closure, `{x,..}`, pattern trần) | ST-15 | 2 h | **DONE** | 4 ca sem mới + 1 kỳ vọng cũ sửa có lý do (ca thứ 5) |
| ST-15 | thêm test ngữ nghĩa tới **≥ 300 ca** | ST-14 | 3 h | **DONE** | tổng ≥ 300 `.vis` + kỳ vọng stdout |
| ST-16 | fuzz parser **1 giờ** + `vis-dev nightly` chạy được (bỏ exit 2) | ST-14 | 2 h | **DONE** | không panic; nightly PASS |
| ST-18 | `cmd_lower`: chế độ lệnh `.vish` hạ xuống HIR + chạy được | ST-17 | 3 h | **DONE** | bảng chữ ký tối thiểu; `vis run t.vsh` chạy; `VS06xx`? (hỏi A0) |
| ST-19 | `vis-bytecode`: định dạng VBC1 + sinh mã từ HIR | ST-18 | 3 h | **READY** | magic/phiên bản; sinh mã cho subset VS0; test khứ hồi |
| ST-20 | `vis-vm`: máy ảo thực thi bytecode, ngữ nghĩa trùng interp | ST-19 | 3 h | BLOCKED | chạy 309 ca sem qua VM = interp |
| ST-21 | verifier bytecode + fuzz không panic | ST-20 | 2 h | BLOCKED | bytecode hỏng bị từ chối; fuzz 10 phút |
| ST-22 | differential interp vs VM **≥ 100.000 chương trình, 0 lệch** | ST-21 | 3 h | BLOCKED | sinh chương trình có seed; so stdout+mã lỗi |
| ST-23 | cache bytecode theo hash closure + khởi động ấm < 20 ms | ST-22 | 2 h | BLOCKED | đo thật, không ước lượng |
| ST-24 | báo cáo **R3** | ST-18..ST-23 | 1 h | BLOCKED | MD+JSON, số liệu tái lập |
| ST-17 | báo cáo **R2** (`reports/R2-viscript.*`) | ST-11..ST-16 | 1 h | **DONE** | MD+JSON, 20 hàng QUEUE, số liệu tái lập được |
| ST-04 | lexer theo SPEC-lex (ST-02a đã DONE) | ST-02a | 3 h | **DONE** | **60 golden token đỏ trước**; fuzz 10 phút + NFC/NFD |
| ST-05 | parser + CST lossless | ST-04 | 3 h | **DONE** | **30 golden biểu thức đỏ trước**; 100 % fixture tài liệu parse; mỗi tệp lỗi đúng 1 lỗi đầu |
| ST-06 | CmdCST + golden từng hàng bảng 9.4 + 10 ca mơ hồ | ST-05 | 3 h | **DONE** | golden AST/CST |
| ST-07 | `vis-fmt` lossless, idempotent, sửa tab | ST-05 | 3 h | **DONE** | vòng tròn parse→fmt→parse |
| ST-08 | runner `--check` cú pháp (chưa interpreter) | ST-05 | 2 h | **DONE** | TAP; chạy fixture |
| ST-09 | fixture theo QD §7.4 (1/khối mã + 1/quyết định; cấm `...`) | ST-06,07,08 | 2 h | **DONE** | xanh |
| ST-10 | báo cáo `R1-viscript` | các task | 1 h | DONE | MD + JSON có `project` |

APPROVED 2026-11-12 Agent 0

> ST-02a DONE; VD-27 chốt mã lỗi 01xx..05xx. ST-02b/ST-10 DONE; ST-03 READY. Commit `3d5539a` do người điều phối, không phải Agent 2.

APPROVED 2026-11-12 Agent 0

> **ST-04 DONE** — commit `663eacb` (đã push `origin/main`): lexer `crates/vis-syntax` theo `SPEC-lex`; **60/60
> golden token đỏ trước** (`JOURNAL/st04-red-golden.log`), fuzz **10 phút** (18.661.376 ca / 336.316.250 token,
> 0 vi phạm, 25/25 kind, đủ 10 mã — `JOURNAL/st04-fuzz-10m.log`), NFC/NFD, `./vis-dev gate --fast` PASS, 40 test.
> **ST-05 mở ngay** (`READY`). Quyết định kèm theo: **VD-28..VD-34** (Q1..Q5 của SPEC-lex, phụ thuộc Unicode, và
> bộ **30 golden biểu thức**).

APPROVED 2026-11-12 Agent 0

> **ST-05 DONE** — commit `49f2539` (đã push): parser + CST lossless `crates/vis-syntax`; **30/30 golden biểu
> thức** đỏ trước (`JOURNAL/st05-red-expr.log`, kỳ vọng Agent 0 ở `viscript/test/expr/golden.expr`, VD-34);
> **15/15 khối tài liệu đủ thân** parse 0 lỗi + lossless, 7 khối loại trừ tường minh (VD-36); **1 lỗi
> đầu/tệp** (13 nguồn khoá); sổ mã cú pháp `VS0201/VS0202` **tách** khỏi `CODES` (VD-35); 5 chỗ Phụ lục A
> không tự chứa được khoá bằng test (VD-37). `gate --fast` PASS, **55** test vis-syntax. Review độc lập **3
> vòng** (2 lần DRIFT: thiếu `self`/NEWLINE, lý do loại khối 1412 sai, `require_line_end` báo oan tệp không
> có newline cuối) — đã sửa hết trước khi commit.
> **ST-06, ST-07, ST-08 mở** (đều phụ thuộc ST-05). Thứ tự đề nghị: **ST-08 (2 h) → ST-07 (3 h) → ST-06 (3 h)**
> — ST-08 nhỏ và cho người dùng thấy kết quả sớm; ST-06 để sau vì nặng nhất. Lỗ hổng đã biết (hoãn, ghi
> `JOURNAL` + vào R2): pattern `dotted_path`, lvalue postfix cho `+=`, `weak`/`cap` chưa là từ khoá,
> `Entry(trường: …)`/`assert`/`...` trong tài liệu, variadic chưa có token.

APPROVED 2026-11-12 Agent 0

> **ST-08 DONE** — commit `2971a72` (đã push): `vis check <tệp…>` in **TAP** (`TAP version 13`, `1..N`,
> `ok`/`not ok`), mỗi tệp `not ok` đúng **1** dòng `# VSxxxx dòng:cột`; mã thoát 0/1/2; `.vish` ⇒ `Bail out!`
> + exit 2 (ST-06); tệp không đọc được ⇒ `not ok` (ngoại lệ có chủ ý, không cấp mã mới); **lọc tầng**: có
> `01xx` thì in `01xx` trước (VD-38 mục 8) để lỗi từ vựng không bị `VS0201` che. **12 ca đỏ trước**
> (`JOURNAL/st08-red-check.log`, sha256 harness khớp bản commit — đã chụp lại một lần vì lệch hash), **78**
> test, `gate --fast` PASS. 15 khối tài liệu đủ thân ⇒ 15 `ok`; 7 khối loại trừ ⇒ 7 `not ok`.
> **ST-07 mở** (kế tiếp, 3 h); **ST-06** để sau (nặng nhất). Quyết định kèm theo: **VD-39** (phạm vi `vis fmt`
> vòng 1) + VD-38 (sổ lỗ hổng đã biết).

APPROVED 2026-11-12 Agent 0

> **ST-07 DONE** — commit `b149fe9` (đã push): crate mới `crates/vis-fmt` + `vis fmt <tệp>|--write|--check`;
> phạm vi **VD-39** (chỉ thụt lề/tab/khoảng trắng cuối/một newline cuối; giữ nguyên văn trong dòng; ra NFC).
> Bốn tiêu chí §18.4: 15/15 khối tài liệu + `chuan.vis` **bất động byte-for-byte**; idempotent 48 nguồn; tab ⇒
> 0 tab + thụt bội 4; tệp lỗi ⇒ exit 2, `--write` không ghi. **105** test, `gate --fast` PASS. Review độc lập 2
> vòng: DRIFT (dòng nối sau chú thích mất thụt lề gốc, tab trong gap cùng dòng + trong thân chú thích bị đổi)
> ⇒ đã sửa, 3 ca tái hiện thành điểm bất động.
> **Đính chính số**: tiêu đề commit ghi "(17 ca đỏ trước)" là **sai do Agent 0 soạn thông điệp**; log đỏ thật
> có **27 ca** (5 unit + 13 integration + 9 CLI). Không amend (đã push, tránh force-push) — sổ này và báo cáo
> R2 ghi đúng **27**.
> **ST-06 mở** (kế tiếp, 3 h, chia 2 pha): pha A = bộ **22 golden chế độ lệnh** (12 hàng §9.4 + 10 ca mơ hồ) do
> Agent 2 soạn nháp + Agent 0 **duyệt trước** khi hiện thực; pha B = `parse(Mode::Command)` → CmdCST + nối
> `.vish` vào `vis check`.

APPROVED 2026-11-12 Agent 0

> **ST-06 DONE** — commit `590e9f7` (đã push): `parse(src, Mode::Command)` → **CmdCST** chưa định kiểu
> (`cmd_lower` vẫn vòng 2, VD-22); **33 golden** chế độ lệnh (12 hàng §9.4 + 10 ca mơ hồ + `.vish` nhiều dòng
> + 10 ca từ review, 7 ca `!ERR`) đỏ trước → xanh; `vis check` nay kiểm **`.vish`** (bỏ `Bail out!`), 2 khối
> ```vish-session của tài liệu ⇒ 2 `ok`; `vis fmt` vẫn từ chối `.vish`. **142** test, `gate --fast` PASS, lexer
> không đụng. Review độc lập 2 vòng: DRIFT (`let` có kiểu hỏng, closure cắt nhầm tại `|>`, `&` trước `;` được
> nhận, `foo[ab]` tách thành 2 từ) ⇒ đã sửa hết + MINOR; vòng 2 OK. Quyết định: **VD-40/VD-41** (xem
> `DECISIONS.md`).
> **ST-09 mở** — bước cuối vòng 1: cổng **doc-examples parse** (QD §7.4 + VT09): bước mới trong `vis-dev gate`,
> allowlist `docs/viscript/doc-examples-allow.txt` **7 mục được kiểm chứng** (mỗi mục phải thật đỏ; khối đỏ
> ngoài danh sách ⇒ cổng đỏ), 17 fixture tự sinh + ≥10 fixture cho quyết định §7.1–7.3, ≥1 case âm.

APPROVED 2026-11-12 Agent 0

> **ST-09 DONE** — commit `ba777e9` (đã push): bước `doc-examples` trong `vis-dev check`/`gate --fast`
> (**sau** `--- build: OK`, kiểm độ mới của binary — không xanh trên binary cũ); `docs/viscript/doc-examples-allow.txt`
> **đúng 7 mục** (6 khối `...` + khối 1412), mỗi mục phải **thật đỏ**, khối đỏ không khai ⇒ cổng đỏ; **24** fixture
> `doc-*` tự sinh (22 `vis` + 2 `vish-session`, so byte với tài liệu) + **12** `qd-*` cho QD §7.1..§7.3 (so **đúng
> mã**); **12 case âm** (khối lạ, ngoại lệ cũ đã xanh, sai số 7, lệch byte, qd=10/13, binary cũ…). `--- doc-examples:
> OK (24 khối: 17 xanh + 7 ngoại lệ; 12 fixture QD)`. Review 2 vòng: DRIFT (nới ngưỡng `>=10`; cổng chạy trước
> `cargo build`) ⇒ đã sửa; vòng 2 Agent 0 xác nhận trực tiếp.
> **ST-03 là task cuối còn lại của vòng 1** (vis-diag: cột theo scalar, render, vi/en/ja, snapshot 20 ca; cap 2 h).

APPROVED 2026-11-12 Agent 0

> **ST-03 DONE — VÒNG 1 XONG (ST-01..ST-10, 11/11 dòng `DONE`)** — commit `ada1ebc` (đã push): `vis-diag` có
> cột theo **Unicode scalar** (khoá 5 ca: Việt NFC/NFD, `🚀`, `👨‍👩‍👧` = 5 scalar, tab = 4 — lexer ST-04 vốn đã
> đúng, chỉ khoá lại), `render(&DiagLine, Lang)` in `<tệp>:<dòng>:<cột>: <mức> <mã>: <thông điệp>` + dòng nguồn
> + con trỏ canh theo scalar, bảng **vi/en/ja** đủ 12 mã (`message_vi` không đổi, SPEC-lex §7 vẫn là chuẩn),
> `vis explain --lang vi|en|ja` + `--list --lang`, **20 snapshot `render`** (NFC/NFD/emoji/ZWJ/tab giữa dòng/dòng
> dài/ngoài biên/CJK/3 ngôn ngữ) + 20 ca message mẫu. **157** test, `gate --fast` PASS. Review 2 vòng: DRIFT
> ("snapshot 20 ca" mới là message, thiếu emoji) ⇒ đã thêm bộ 20 snapshot render; vòng 2 Agent 0 xác nhận.

APPROVED 2026-11-12 Agent 0

> **ST-11 DONE** — commit `94a5342` (đã push): crate mới `crates/vis-types` (`Type` theo VD-43 + `Float`/`TypeRef`/
> `Error`), HIR chế độ script giữ `Span`, `lower()` (hạ CST→HIR + giải tên), `Symbols`/`TypeTable`. Hạ **permissive**
> (VD-44) — biểu diễn mọi thứ Phụ lục A ở mức cú pháp; `Mode::Command` ⇒ lỗi API, không panic. Giải tên (VD-45):
> mặc định **Permissive** (`VS0301` chỉ khi gán tên chưa khai, ở gốc lvalue; `VS0302` khai lại cùng khung),
> `Strict` có test riêng. Sổ thứ ba `NAME_CODES` (`VS0301/VS0302` vi/en/ja), `next_block()` ⇒ **601** suy ra.
> **194** test; 15/15 khối tài liệu hạ 0 chẩn đoán; 27 golden ⇒ sexp HIR; 3 ca `!ERR` dừng ở parse; `gate --fast` PASS.
> Review: DRIFT (`lower_let` khai tên trước khi hạ `init`; `Strict` báo `VS0301` hai lần) ⇒ đã sửa + khoá test.
> **ST-12 mở**.

APPROVED 2026-11-12 Agent 0

> **ST-12 DONE** — commit `273a921` (đã push): kiểm kiểu VS0 (`tyck/{mod,expr,pat,num}.rs`) + **`match` xét hết**,
> sổ thứ tư `TYPE_CODES` (`VS0401/0402/0403/0412`, vi/en/ja), `all_codes()` = 18, `explain` phủ 4 sổ. **116 ca đỏ
> trước** (88 tyck + 28 `match`) có **chống xanh-giả**; **205** test; `gate --fast` PASS. Quyết định **VD-46**:
> gán cho `let`/tham số ⇒ `VS0412` + thông điệp "chỉ `var` mới gán"; lambda param `{error}` (hoãn suy kiểu hai
> chiều, parser đúng §A.3); pattern bản ghi trần thiếu `sym` ⇒ hoãn ST-14; `line/col = 0` ở tầng tyck chấp nhận —
> **ST-14 bắt buộc in lại thật từ `span` + văn bản**.
> **ST-13 mở** (`vis-interp` + builtins lõi, `VS05xx` runtime).

APPROVED 2026-11-12 Agent 0

> **ST-13 DONE** — commit `5e9a962` (đã push): crate mới `crates/vis-interp` (`Value` 16 biến thể + `Closure`;
> `interp.rs` môi trường/câu lệnh/biểu thức/pattern, gọi hàm tham số tên + mặc định + đệ quy, `?`/`return` trả sớm,
> `match` đầy đủ, `with`, chỉ số, `|>`, `??`; `builtin.rs` toán tử §3.6 + 8 tên dựng sẵn + bộ phương thức lõi),
> sổ thứ năm `RUNTIME_CODES` (`VS0501/0502/0503`), `all_codes()` = 21, `next_block()` vẫn 601. **79 ca đỏ trước**,
> **227** test, `gate --fast` PASS, chẩn đoán runtime in `dòng:cột` **thật** (VD-46). Quyết định **VD-47** (giá trị
> câu lệnh cuối; `Show` size/duration/percent; `percent * size|int` cắt về 0; bảng phương thức khoá test). Hoãn có
> ghi: nội suy `{expr}` ⇒ ST-14; `ref/await/cmd`/bit ⇒ `VS0501`; gán trường/phần tử ⇒ `VS0501` + gợi ý `with`.
> **ST-12b mở** (sửa 7 lỗi kiểm kiểu do review độc lập bắt; xong mới coi ST-12 DONE thật).

APPROVED 2026-11-12 Agent 0

> **ST-12b DONE** — commit `dec75e3` (đã push): sửa **đúng 7 lỗi** kiểm kiểu do review độc lập bắt (`?` theo §4.4,
> `int→float` chỉ literal, builtin được kiểm, gán `Field`/`Index`, `--` chỉ cho tham số khai `--`, `percent` vs
> `float` theo tỉ lệ, `match` `unit`/tuple hữu hạn) + **VD-48** (11 kiểu ngoài VS0 ⇒ `VS0301` kèm thông điệp rõ;
> `percent / percent` ⇒ float…). **24 chỗ đỏ trước** trên code cũ (stash bản vá), **237** test, `gate --fast` PASS.
> **ST-12 nay DONE thật** (đã qua review + vá). **ST-14 mở** (`vis run` + TAP + 100 ca + nội suy `{expr}`).

APPROVED 2026-11-12 Agent 0

> **ST-13b DONE** — commit `f1ac635` (đã push): sửa **10 lỗi** review ST-13 bắt — `Type::Never` cho `return`/
> nhánh phân kỳ (hết `VS0412` oan cho `fn f()->int: return 1`), **panic tràn range** `0..=i64::MAX` (checked_add,
> dừng dải), vòng lặp bỏ giá trị cuối thân, `for c in "ab"` ⇒ `char`, range VS0 chỉ `int`, số học đối xứng
> (`size*percent`, `percent/percent`, `%` cùng đơn vị), bảng phương thức kiểm kiểu + keyword sau `.` (`m.has`),
> `pos` tab = **4 cột** + span thật, `break/continue` ngoài vòng/`?` ngoài hàm ⇒ `VS0501`, `Show` thêm `w` +
> escape `char`, decimal trước binary. **246** test, `gate --fast` PASS.
> **ST-14 DONE** — commit `9aafdb8` + `4d52164` (Cargo.lock, không amend commit đã push): **nội suy `{expr}`**
> (`HExpr::Interp`, lower trong cùng ngữ cảnh giải tên), **`vis run`** (exit 0/1/2, `--tap`, `.vish` bail),
> **harness ngữ nghĩa TAP** chạy qua binary, **100 ca/12 nhóm** xanh. **247** test, `gate --fast` PASS.
> **ST-15 mở** (≥ 300 ca ngữ nghĩa).

APPROVED 2026-11-12 Agent 0

> **ST-15 DONE** — `27aa6aa`: **302 ca/16 nhóm** (khoá cứng + chống bỏ ca), cap độ sâu gọi **2.000** (đo 10^4 tràn
> stack), `vis run` chạy interp trong luồng 256 MiB. **ST-15b DONE** — `806e29a`: sửa **4 lệch thiết kế**
> (`push` ghi lại lvalue theo CoW §7.1; closure chụp **snapshot** env; `{x, ..}` hợp lệ §4.3; pattern trường trần
> có `sym`), tách `pat.rs` khỏi `interp.rs` (1.511→1.336), **309 ca**. **ST-16 DONE** — `a89f036`: `vis-parse-fuzz`
> **1 giờ thật: 182.328.926 ca / 4.145.431.263 token / 0 vi phạm**, `vis-dev nightly [--fuzz-minutes N]` chạy được
> (gate + fuzz + vòng tròn fmt 309 tệp) ⇒ `nightly: PASS`, hết exit 2.
> **ST-17 mở** (báo cáo **R2** — chốt VS0).

APPROVED 2026-11-13 Agent 0

> **ST-17 DONE — VÒNG 2 (VS0) CHỐT.** `reports/R2-viscript.{md,json}` (`8059351`, cập nhật phủ đủ 20 hàng `3cc8139`)
> ghi 20 hàng QUEUE, số liệu tái lập được (247 test; 309 ca ngữ nghĩa/16 nhóm; 5 sổ mã `01xx..05xx` = 21 mã;
> fuzz parser 1 giờ 182.328.926 ca / 4.145.431.263 token / **0 vi phạm**; 24 khối tài liệu = 17 xanh + 7 ngoại lệ;
> `nightly --fuzz-minutes 1` PASS), **VD-42..VD-51**, **14 lỗ hổng đã biết**, và **bỏ không đoán** những bộ đếm
> không tái lập được bằng lệnh độc lập.
> **Đối chiếu §19 (VS0)**: ≥ 300 test `.vis` ✓ 309 · fuzz parser 1 giờ không panic ✓ 0 vi phạm · vòng tròn fmt ✓
> trong `nightly` · 100 % fixture tài liệu parse ✓ 24/24. **Mốc VS0 ĐẠT.** Dòng ViScript **hết task trong QUEUE**;
> vòng 3 (khi mở): `cmd_lower` + chữ ký lệnh (VD-22), `vis check --lang`, `vis fmt` cho `.vish`, closure theo tham
> chiếu, nội suy lồng, `match`/`if` làm đối số.

APPROVED 2026-11-13 Agent 0

> **ST-18 DONE** — commit `57d9e1a` (đã push): **`cmd_lower`** (`vis-types/src/lower/cmd.rs`) hạ chế độ lệnh ⇒ HIR,
> `cmd` decl = `HItem::Fn` + `FnKind::Cmd`; **khe ống** (tham số vị trí đầu tiên khớp kiểu ra, giữ chỗ trước) ⇒
> `echo "a\nb" | lines | take 1` = `take(lines(echo("a\nb")), 1)`, không có khe ⇒ `VS0603`; chữ ký đọc từ CST
> (`5`→int, `5mb`→size, `-5` là số, `--flag` + default), thiếu/thừa/sai kiểu ⇒ `VS0602`, lệnh lạ ⇒ `VS0601`,
> `&`/`$x`/`^prog` ⇒ `VS0604`; fail-fast giữa pipeline; 3 builtin `echo`/`lines`/`take`; sổ thứ sáu `COMMAND_CODES`
> (`06xx`, 4 mã) ⇒ `all_codes()` 25, `next_block()` **701**; `vis check/run *.vish` thật (hết `Bail out!`).
> **265 test**, **323 ca sem/17 nhóm** (309 `.vis` + 14 `.vish`), `gate --fast` PASS; 7 kỳ vọng cũ sửa **có lý do**
> trong từng tệp (VD-42 cấp `06xx`, tên dựng sẵn 11, `CommandMode` hạ thật, allowlist 9).
> **ST-19 mở** (`vis-bytecode`).

## Ghi chú vận hành

- **ST-09 = READY** (2026-11-12, sau ST-06/07/08): bước `doc-examples` trong `./vis-dev gate --fast`; tệp
  **mới** `docs/viscript/doc-examples-allow.txt` (Agent 0 cho phép tạo đúng tệp này, QD §7.4 chỉ định) gồm **7**
  mục: 6 khối có `...` (dòng 305/432/445/610/662/737) + khối 1412 (`assert`/`Entry(trường: …)`); script phải
  kiểm chứng (đúng 7 mục, mỗi mục **thật sự đỏ**, khối đỏ ngoài danh sách ⇒ cổng **đỏ**). Fixture VT09: 17 tệp
  tự sinh (15 `vis` + 2 `vish-session`) + ≥10 tệp viết tay cho quyết định QD §7.1–§7.3; ≥1 case âm chứng minh
  cổng bắt được khối `...` mới.
- **ST-06 = DONE** (2026-11-12, sau ST-05): **pha A** — `viscript/test/cmd/golden.cmd` gồm **22 ca** (đúng
  **12 ca = 12 hàng bảng §9.4** theo thứ tự bảng + **10 ca mơ hồ**), dạng `<nguồn> => <sexp CmdCST>`/`!ERR`,
  quy ước sexp ghi ở đầu tệp, harness `crates/vis-syntax/tests/cmd.rs` khoá đúng 22 ca + lossless;
  **Agent 2 soạn nháp ⇒ Agent 0 duyệt ⇒ mới hiện thực**. **Pha B** — `parse(Mode::Command)` ra CmdCST (chưa
  định kiểu, `cmd_lower` là vòng 2 theo VD-22) + nối `.vish` vào `vis check` (bỏ Bail out cũ, cập nhật test
  ST-08 tương ứng); `vis fmt` **vẫn** từ chối `.vish` (định dạng chế độ lệnh chưa có đặc tả).
- **ST-07 = DONE** (2026-11-12, sau ST-05): `vis fmt <tệp…>` — phạm vi vòng 1 theo **VD-39** (chỉ chuẩn hoá
  **thụt lề** 4 dấu cách/cấp + tab→4 dấu cách + bỏ khoảng trắng cuối dòng + đúng **một** newline cuối; **giữ
  nguyên văn trong dòng**, giữ chú thích, giữ thứ tự token; ghi ra **NFC**). 4 tiêu chí §18.4: (a) lossless
  trên tệp đã đúng chuẩn (điểm bất động), (b) idempotent, (c) sửa tab, (d) tệp lỗi cú pháp ⇒ **không sửa**,
  báo lỗi, exit **2**. Test bắt buộc: quét 15 khối tài liệu + 30 golden + tệp tab + tệp lỗi; kiểm thêm "token
  không đổi" (re-lex sau fmt ⇒ cùng kind/text ngoài trivia).
- **ST-08 = DONE** (2026-11-12, sau ST-05): `vis check <tệp…>` — TAP (`1..N`, `ok`/`not ok`, chẩn đoán
  dạng `#`), `.vis` chạy chế độ script; **`.vish` báo rõ "chế độ lệnh thuộc ST-06" + exit 2**, không im lặng
  bỏ qua; mỗi tệp in **1** chẩn đoán (lọc tầng: tệp có lỗi từ vựng vẫn chỉ 1 dòng). Test bằng chính các khối
  tài liệu (15 khối sạch ⇒ `ok`, 7 khối loại trừ ⇒ `not ok`) + nguồn hỏng tự thêm. Không cần interpreter.
- **ST-05 = DONE** (2026-11-12, sau ST-04): kỳ vọng **30 golden biểu thức** do **Agent 0** soạn ở
  `viscript/test/expr/golden.expr` (định dạng + quy ước sexp ở đầu tệp, VD-34) — Agent 2 **không sửa** tệp này,
  thấy sai thì báo. **CST lossless**: ghép CST theo thứ tự phải ra nguyên văn nguồn (kể cả khoảng trắng, chú
  thích, độ rộng thụt lề). "100 % fixture tài liệu" = **mọi khối ```vis trong `docs/viscript/*.md`** (22 khối,
  chế độ script); khối ```vish-session là pipeline chế độ lệnh ⇒ thuộc **ST-06 (CmdCST)**, không tính ở ST-05.
  Ngoại lệ nào cũng phải khai trong `JOURNAL` + báo cáo, **không** âm thầm bỏ khối.
- **ST-02a = DONE**: `APPROVED 2026-11-12 Agent 0` (SPEC-lex + Phụ lục A sau errata). **ST-04 mở ngay**; golden token là test đỏ đầu tiên của ST-04, golden biểu thức của ST-05. **Ưu tiên ST-04 trước ST-03** (đường tới hạn lexer); `agentctl next` có thể trả ST-03 vì thứ tự bảng — làm ST-04 trước.
- `vifast-agentctl --project viscript next` **bỏ qua** task `REVIEW`; `next` chỉ trả task `READY` có phụ thuộc `DONE`.
- **Mỗi task ≤ 3 h; task lớn tách.** ST-02a tách khỏi ST-02b để phần sổ mã lỗi không phải chờ duyệt.
- Parse chế độ lệnh **hai pha** (QD §7.2 + D §6.D): `vis-syntax` sinh `CmdCST` → `cmd_lower` (vis-types, dùng bảng chữ ký) → **HIR**. "Hai chế độ ra cùng AST" đọc là **cùng HIR**; `cmd_lower` là task vòng 2. Vòng 1 dừng ở lexer → parser/CST → `CmdCST` → `vis-fmt` → runner `--check` cú pháp (chưa interpreter, chưa kiểm kiểu).
- `ST-09` cấm `...` trong fixture: mọi khối mã trong tài liệu phải parse được, ngoại lệ phải khai báo trong SPEC.
- Ưu tiên hai dòng: **OS trước**; dòng này chỉ lấy task khi `v2/agent/QUEUE.md` không còn task `READY` (D2-20). Ngoại lệ đã đóng: ST-02a DONE, ST-04 mở ngay.
- Số nào chưa có nguồn trong QD/tài liệu ViScript thì ghi `[TK]`/`[ƯL]`, không tự đặt.
