# AGENTS.md — dòng ViScript (tóm tắt)

Bản đầy đủ: [`../AGENTS.md`](../AGENTS.md). Tệp này chỉ thêm phần riêng của `viscript/`.
Nguồn sự thật của vòng vá: [`../docs/QUYET-DINH-sau-review.md`](../docs/QUYET-DINH-sau-review.md) (**QD**), nhất là §7.

## 1. Thứ tự nguồn sự thật (áp cho ViScript)

1. **Code + test** trong `viscript/`: crate biên dịch được, golden, fuzz, TAP.
2. **`SPEC`** — [`../docs/viscript/SPEC-lex.md`](../docs/viscript/SPEC-lex.md) (từ vựng + EBNF tự chứa) và `SPEC.md` (do ST-02a/ST-02b sinh); sửa **chỉ** qua Agent 0.
3. [`agent/DECISIONS.md`](agent/DECISIONS.md) — VD-01..VD-10 + VD-11..VD-88 (QD §7, các câu Q1..Q5 của SPEC-lex, và các chỗ văn phạm chế độ lệnh/vis-fmt/vis-diag cần chốt).
4. Tài liệu thiết kế [`../docs/viscript/ViScript-thiet-ke.md`](../docs/viscript/ViScript-thiet-ke.md) — tham chiếu.

QD thắng tài liệu thiết kế.

## 2. Tài liệu và điều khiển

| Tệp | Vai trò |
|---|---|
| [`../docs/viscript/ViScript-thiet-ke.md`](../docs/viscript/ViScript-thiet-ke.md) | thiết kế đầy đủ (9.4 bảng xung đột chế độ lệnh, 14 cấu trúc crate, 20 quy trình, 22 quyết định) |
| [`../docs/viscript/SPEC-lex.md`](../docs/viscript/SPEC-lex.md) | đặc tả từ vựng + EBNF tự chứa — sản phẩm của **ST-02a** |
| [`agent/QUEUE.md`](agent/QUEUE.md) | vòng 1 ST-01..ST-10 + vòng 2 VS0 + vòng 3 khung VS1 (kèm 12b/13b/15b/20b) + vòng 4 VS1b ST-25..ST-31 (kèm 30b) + vòng 5 VS1c ST-32..ST-36 + vòng 6 VSH1a ST-37..ST-43 + vòng 7 VSH1b ST-44..ST-47 + vòng 8 VSH1c ST-48..ST-52 + vòng 9 VSH2 ST-53..ST-55 — **61/61 `DONE`** |
| [`agent/DECISIONS.md`](agent/DECISIONS.md) | VD-01..VD-10 + Q1..Q8 + VD-11..VD-88 |
| [`agent/STATE.md`](agent/STATE.md), [`agent/BLOCKED.md`](agent/BLOCKED.md) | trạng thái + chặn (Agent 2 ghi) |
| `agent/JOURNAL/`, `agent/reports/` | nhật ký ngày, `R<n>-viscript.md` + `.json` |

## 3. Trạng thái duyệt hiện tại

- **ST-02a = APPROVED 2026-11-12 Agent 0 / DONE**: `SPEC-lex.md` + EBNF + bảng ưu tiên + 60 golden token + 30 golden biểu thức (bộ 30 nay ở `viscript/test/expr/golden.expr`, VD-34).
- **ST-04 = DONE** (`663eacb`): lexer `crates/vis-syntax` — 60/60 golden token (đỏ trước), fuzz 10 phút, NFC/NFD.
- **VÒNG 1 XONG (2026-11-12/13, 7 task trong 1 đêm)**: **ST-05** parser + CST lossless (`49f2539`; 30 golden biểu thức, 15/15 khối tài liệu), **ST-08** `vis check` TAP (`2971a72`), **ST-07** `vis-fmt` lossless/idempotent (`b149fe9`), **ST-06** CmdCST + `.vish` vào `vis check` (`590e9f7`; 33 golden chế độ lệnh), **ST-09** cổng `doc-examples` (`ba777e9`; 24 khối/17 xanh + allowlist 7 + 12 fixture QD), **ST-03** `vis-diag` scalar/render/vi-en-ja (`ada1ebc`; 20 snapshot render). **157** test, `gate --fast` PASS, không `#[ignore]`, mọi log đỏ có sha256 khớp. Quyết định: **VD-28..VD-41** (Q1..Q5 SPEC-lex, Unicode, 30 golden biểu thức, sổ cú pháp 02xx, fixture tài liệu, Phụ lục A, vis-fmt vòng 1, chế độ lệnh vòng 1, lỗ hổng đã biết).
- **VÒNG 9 — VSH2 ĐÃ CHỐT (2026-11-14)**: **`to json`** (stage lệnh, hạ `std.json.encode`, không cần `use`), **một comparator** cho shell `sort_by` + `std.list.sort_by` (giữ `FnIntInt`), **`CODEGEN_TIN_TYCK` 21 → 0** (codegen **từ chối** khi front end có chẩn đoán `error`, `Err(ChanDoan)` mang mã+dòng:cột+thông điệp). Đo: **730 test**, **370 ca/18 nhóm**, `vis test` 370 ok, parity **344/344**, differential **100k 0 lệch**, allowlist **6**; **VD-86..VD-88**; **R9** (kèm **kế hoạch VS2 có tiêu chí đo**). Còn **10 nợ** + **VS2**.
- **VÒNG 8 — VSH1c ĐÃ CHỐT (2026-11-14)**: **thân `cmd` bằng pipeline lệnh** (thử-script-trước, filler giữ byte), **nhóm-lệnh có cờ** + **nhóm nhiều dòng** + **một** đường hạ nhóm, **`VS0405`** (một mã field-access, **đúng sổ kiểu**), **`where ( x => x )` kiểm tĩnh**, **stage đầu là biểu thức** (`[1,2] | take 1`). Đo: **704 test**, **370 ca/18 nhóm**, `vis test` 370 ok, parity **344/344**, differential **100k 0 lệch**, allowlist **6**; **VD-81..VD-85**; **R8**. Còn **9 nợ lõi** + **VS2**.
- **VÒNG 7 — VSH1b ĐÃ CHỐT (2026-11-13)**: `.vish` **khai báo kiểu** (`type`/`enum`/`let x: T`/`cmd f(...) -> T`, tách **vùng item/stage**), **`where` kiểm tĩnh đầy đủ** (`VS0412` nêu kiểu thật) + `each` **không** kiểm kiểu trả, **`ls --long` thật** (`FileEntryLong`, `mode`, `modified_time` UTC), **nhóm là đối số** (`take ( 1 + 1 )`), **chẩn đoán tuyệt đối** trong `( … )`. Đo: **651 test**, **370 ca/18 nhóm**, `vis test` 370 ok, parity **344/344**, differential **100k 0 lệch**, allowlist **6**; **VD-77..VD-80**; **R7**. Còn **VSH1b-2** + 14 nợ + **VS2**.
- **VÒNG 6 — VSH1a ĐÃ CHỐT (2026-11-13)**: chế độ shell **dùng được** theo §9.5–§9.7 — **khe ống hợp nhất** (bỏ so `==`, hợp nhất một chiều) ⇒ `( [1,2] ) | take 1`; **closure-arg** + `where`/`each`/`first`/`count` + **sugar vị từ ngầm `it`**; `sort_by --desc` **ổn định**; **prelude** `FileEntry`/`ProcEntry` + `it` kiểu thật + **`VS0605`**; `get`/`select`; **`Render`** (bảng `list[bản ghi]`, khối khoá–giá trị, NFC, `(rỗng)` chỉ cho bản ghi); `ls`/`open` qua `FsHost` + **`--fs-mock`** ⇒ **§9.7 khối 672 chạy thật**. Đo: **606 test**, **370 ca/18 nhóm**, `vis test` 370 ok, parity **344/344**, differential **100k 0 lệch**, **allowlist doc-examples 8 → 6**; **VD-69..VD-76**; **R6**. Còn **VSH1b** + **VS2** + 17 nợ trong `R6` §5.
- **VÒNG 5 — VS1c ĐÃ CHỐT (2026-11-13)**: **harness chẩn đoán 3 đường** (so cả 26 ca **lỗi biên dịch**, 0 lệch), **`json.decode`** + **`std.fs`** qua `FsHost` (interp/vm **không** chạm OS; không host ⇒ `VS0604` cả 2 đường), **pipe→stdin** + **`| complete`** (`grep` exit 1 hết là lỗi; `MAX_STDIN` 8 MiB), **generic thật** (`Hop[int] ≠ Hop[str]`, `RecId` interned theo tổ hợp, payload xoá kiểu ⇒ không bump `VBC1`). Đo: **507 test**, **346 ca/18 nhóm**, `vis test` 346 ok, parity **320/320**, differential **100k 0 lệch**; **VD-64..VD-68**; **R5**. Còn **VS2** + 15 nợ trong `R5` §4.
- **VÒNG 4 — VS1b ĐÃ CHỐT (2026-11-13)**: **module** (`use ./x`, đa tệp, cấm vòng, hash closure gồm phụ thuộc, sổ `07xx`), **`vis-std` lõi** (`std.str/list/map/path/time/json`, 70 ca), **`vis-host-posix`** (chương trình ngoài §9.3, variadic, `--x=v`, `trait Ngoai` giữ biên OS), **generic `[T]`** (xoá kiểu + hợp nhất call-site), **trait/impl** (dispatch tĩnh, chạy cả interp và VM), **`vis test`** TAP + **SHA-256** thật cho hash nguồn/cache, **đồng bộ chẩn đoán** hai đường. Đo: **436 test**, **338 ca/18 nhóm**, parity **312/312**, differential **100k 0 lệch**; **VD-57..VD-63**; **R4**. Còn **VS2** (async/kênh/net/Landlock/LSP/gói) + 13 nợ trong `R4` §5.
- **VÒNG 3 — KHUNG VS1 ĐÃ CHỐT (2026-11-13)**: `vis-bytecode` (**VBC1** 53 tag), **`vis-vm`** (ngăn xếp theo khung, cap 2.000 khung / 10^7 lệnh), **verifier** (13 biến thể, kiểm lại độc lập từ byte stream), **cache**, **`vis run --vm`**, `vis cache`, `bin/vis-diff`. Đo: **318 test**; differential **100.000 chương trình 0 lệch**; **khởi động ấm 8,5–9,9 ms** (< 20 ms); fuzz đột biến 10 phút 0 vi phạm. **VD-52..VD-56**; **R3**.
