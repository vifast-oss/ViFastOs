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
| ST-01 | workspace `viscript/` + `viscript/agent/` (không ghi đè) + SPDX theo loại tệp + cổng 1.500 dòng/unsafe có case âm + `vis-host-vifast` **không** nằm trong `viscript/` | — | 3 h | **DONE** | cổng chạy; 3 case âm |
| ST-02a | **Áp errata mục 3 (S1..S11, L1..L8) + APPROVED**; SPEC-lex + Phụ lục A + bảng ưu tiên do Agent 1 soạn | — | 3 h (2 phiên) | **DONE** | errata áp xong; **APPROVED 2026-11-12 Agent 0** |
| ST-02b | sổ mã lỗi VS + 4 chữ số + quy tắc cấp | ST-01 | 2 h | **DONE** | bảng mã; `vis explain` được |
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
| ST-19 | `vis-bytecode`: định dạng VBC1 + sinh mã từ HIR | ST-18 | 3 h | **DONE** | magic/phiên bản; sinh mã cho subset VS0; test khứ hồi |
| ST-20b | differential về **0 lệch** (sửa 19 ca) | ST-20 | 2 h | **DONE** | 272/272 khớp (stdout+exit+mã,dòng,cột); KNOWN_LECH rỗng |
| ST-20 | `vis-vm`: máy ảo thực thi bytecode, ngữ nghĩa trùng interp | ST-19 | 3 h | **DONE** | chạy 309 ca sem qua VM = interp |
| ST-21 | verifier bytecode + fuzz không panic | ST-20 | 2 h | **DONE** | bytecode hỏng bị từ chối; fuzz 10 phút |
| ST-22 | differential interp vs VM **≥ 100.000 chương trình, 0 lệch** | ST-21 | 3 h | **DONE** | sinh chương trình có seed; so stdout+mã lỗi |
| ST-23 | cache bytecode theo hash closure + khởi động ấm < 20 ms | ST-22 | 2 h | **DONE** | đo thật, không ước lượng |
| ST-25 | module: `use ./util`, đa tệp, cấm vòng, hash closure gồm phụ thuộc | ST-24 | 3 h | **DONE** | đa tệp chạy được; `VS07xx` cho vòng/không thấy |
| ST-26 | `vis-std` lõi: `std.str/list/map/path/time/json` | ST-25 | 3 h | **DONE** | ≥ 60 ca; `use std.x` chạy |
| ST-27 | `vis-host-posix`: chương trình ngoài (§9.3), variadic, `--x=v` | ST-26 | 3 h | **DONE** | chạy `ls`/`echo` thật; quyền tối thiểu |
| ST-28 | generic hàm + kiểu `[T]` | ST-27 | 3 h | **DONE** | ≥ 40 ca; differential vẫn 0 lệch |
| ST-29 | trait + `derives` (Eq/Show/Json) | ST-28 | 3 h | **DONE** | ≥ 30 ca |
| ST-30 | `vis test` + SHA-256 cho hash closure/consent | ST-29 | 2 h | **DONE** | `sha2` duyệt allowlist; thay FNV |
| ST-30b | đồng bộ chẩn đoán interp↔VM + siết differential (so đủ) | ST-30 | 2 h | **DONE** | 312/312 khớp, 0 lệch |
| ST-32 | harness so chẩn đoán **biên dịch** hai đường (đóng nợ 26 ca) | ST-31 | 2 h | **DONE** | 0 lệch; không nới |
| ST-33 | `json.decode` + `std.fs` tối thiểu (qua `Host`) | ST-32 | 3 h | **DONE** | đối xứng encode; interp/vm không chạm OS |
| ST-34 | `\| complete` + pipe→stdin cho chương trình ngoài | ST-33 | 3 h | **DONE** | `grep` exit 1 hết là lỗi; 2 tiến trình nối |
| ST-35 | generic **thật**: `Hop[int] ≠ Hop[str]` | ST-34 | 3 h | **DONE** | sai kiểu ⇒ VS0412; differential 0 lệch |
| ST-36 | báo cáo **R5** | ST-32..ST-35 | 1 h | **DONE** | MD+JSON, số liệu tái lập |
| ST-37 | `CmdSig` **generic** + khe ống **hợp nhất** (bỏ `==`); `take`/`lines` generic; seam `SigSource` | ST-36 | 3 h | **DONE** | `( [1,2] ) \| take 1` xanh; 338 ca sem không đổi; differential 0 lệch |
| ST-38 | hạ **closure-arg** (`CmdClosure` → `Lambda`, cộng offset span) + `where`/`each`/`first`/`count` | ST-37 | 3 h | **DONE** | 8 ca dương + 4 âm; lỗi trong thân closure đúng `dòng:cột`; `VS0412` cho thân sai kiểu |
| ST-39 | `sort_by` + `--desc` (mở `list.sort_by` cho khoá `int/size/duration/str`, giữ ổn định) | ST-38 | 3 h | **DONE** | 6 ca; ca cũ `list.sort_by` vẫn xanh; khoá `bool` ⇒ `VS0412` |
| ST-40a | **Prelude bản ghi** `FileEntry`/`ProcEntry` + `it` + **viết tắt trường**; mã `VS0605` | ST-39 | 3 h | **DONE** | `where size > 5mb` xanh trên `list[FileEntry]`; 6 ca mơ hồ; sổ `06xx` 4 → 5 mã |
| ST-40b | `get`/`select` chiếu theo trường (hạ thành `map` trên `it.<field>`) | ST-40a | 2 h | **DONE** | `\| get name` ⇒ `list[str]`; trường không có ⇒ `VS0605` |
| ST-41 | **`Render`**: bảng `list[bản ghi]` + khối khoá–giá trị; cột theo scalar sau **NFC** | ST-40b | 3 h | **DONE** | 15 snapshot (Việt NFD/emoji/CJK); `print` cũ cho `list[str]` không đổi |
| ST-42 | `ls`/`open` qua `FsHost` (`list_entries` + metadata), `--all/--long`, cờ **`--fs-mock`** | ST-41 | 3 h | **DONE** | 12 ca tất định; pipeline §9.7 dòng 672 chạy được; allowlist **8 → 7** |
| ST-43 | báo cáo **R6** (VSH1a) | ST-42 | 1,5 h | **DONE** | MD + JSON `project: "viscript"`, số liệu tái lập, 8 hàng |
| ST-44 | `.vish` **khai báo kiểu**: `type` / `let x: T` / `cmd f(...) -> T` | ST-43 | 3 h | **DONE** | prelude dùng bằng khai báo thật; `VS0302` khi trùng; differential 0 lệch |
| ST-45 | **`where` kiểm tĩnh đầy đủ** (vị từ không `bool` ⇒ lỗi tĩnh) | ST-44 | 2 h | **DONE** | hết nợ `VS0501` lúc chạy; kiểu `{error}` ⇒ vẫn kiểm động |
| ST-46 | `ls --long` thật + `( … )` làm **đối số** + chẩn đoán **tuyệt đối** trong ngoặc | ST-45 | 3 h | **DONE** | 3 nợ VSH1b hết; allowlist không tăng |
| ST-47 | báo cáo **R7** | ST-44..ST-46 | 1,5 h | **DONE** | MD+JSON, số liệu tái lập |
| ST-48 | thân `cmd` trong `.vish` bằng **pipeline lệnh** (VSH1b-2) | ST-47 | 3 h | **DONE** | `cmd f() -> list[FileEntry]:` + khối `ls \| where …` chạy 2 đường |
| ST-49 | **nhóm-lệnh có cờ** `( ls … --long )` + **nhóm nhiều dòng** | ST-48 | 2,5 h | **DONE** | 2 nợ VSH1b hết; `( ls … )` không còn rơi về lệnh ngoài |
| ST-50 | **thống nhất mã field-access** + `where ( x => x )` kiểm **tĩnh** | ST-49 | 2,5 h | **DONE** | một mã duy nhất; snapshot vi/en/ja kèm lý do |
| ST-51 | **stage đầu là biểu thức** (`[1,2] \| take 1` không ngoặc) | ST-50 | 2 h | **DONE** | không phá văn phạm §9.5; `( … )` vẫn chạy |
| ST-52 | báo cáo **R8** | ST-48..ST-51 | 1,5 h | **DONE** | MD+JSON, số liệu tái lập |
| ST-53 | `select` (bản ghi ẩn danh) đi `to json` + **hợp nhất hai `sort_by`** | ST-52 | 3 h | **DONE** | một hiện thực; `std.list.sort_by` cũ không phá; ổn định + `--desc` |
| ST-54 | `CODEGEN_TIN_TYCK`: codegen **từ chối** khi HIR còn kiểu `Error` | ST-53 | 3 h | **DONE** | chẩn đoán rõ; hằng số 21 phải **giảm**; diff 0 lệch |
| ST-55 | báo cáo **R9** + kế hoạch **VS2** có tiêu chí đo | ST-53..ST-54 | 1,5 h | **DONE** | MD+JSON; nêu rõ khoảng cách VS2 |
| ST-56 | nợ lõi **A**: khoá `str` cho `std.list.sort_by`; `&self`/`&expr` (hết `Unsupported` ở codegen); `Hop[Hop[int]]` đóng nốt | ST-55 | 3 h | **DONE** | 3 nợ đóng, mỗi nợ ≥2 ca; interp ↔ `--vm` cùng byte |
| ST-56b | chốt **gán** một ngữ nghĩa hai đường: `self.x = v` khi `self` bất biến ⇒ `VS0412`; `var self`/`var xs` gán được ở **interp** (VM đã làm); `run --vm` lỗi codegen ⇒ exit ≠ 0 | ST-56 | 2,5 h | **DONE** | ≥6 ca; interp = `--vm` cùng byte; không nới ngưỡng |
| ST-57 | nợ lõi **B**: mồ côi **đa tệp**, **siêu trait** kiểm được, **`derives` sinh mã** (`Show`/`Eq`) | ST-56 | 3 h | **DONE** | mỗi mục ≥2 ca đỏ→xanh; derives sinh phương thức **chạy được** |
| ST-58 | nợ lõi **C**: `Ty::Any` → chữ ký **generic thật** cho `std.list.*`/`std.map.*` | ST-57 | 3 h | **DONE** | không còn `Ty::Any`; sai kiểu ⇒ `VS04xx` **tĩnh** |
| ST-59 | nợ shell: thân `cmd` dùng **tham số**; body "vừa script vừa lệnh" có luật rõ; **biến** làm stage đầu | ST-58 | 3 h | **DONE** | ≥6 ca; `x \| take 1` chạy 2 đường |
| ST-60 | **VS2 #1 — async**: `async fn`/`await`/`spawn` + bộ lập lịch **tất định một luồng** | ST-59 | 3 h | **DONE** | ≥20 ca sem interp = `--vm` cùng byte; 10.000 task không tràn |
| ST-60b | hotfix sau review độc lập: gán trong **closure** (captured/param) hồi quy parity; đích lồng `self.xs[0]`; `.vish` shell-let `let out = ls -l`; `sort_by --desc` bị bỏ im lặng | ST-60 | 2,5 h | **DONE** | ≥8 ca; interp = `--vm` cùng byte; không nới ngưỡng |
| ST-60c | derives/siêu trait: impl **sinh** vào `impl_pairs`; `enum Show` phải **lỗi tĩnh**; `Eq` chặn **sâu** (list/map/opt của float); SPDX 19 tệp dữ liệu + header `diag_cmp` | ST-60b | 2 h | **DONE** | ≥4 ca; gate xanh |
| ST-61 | **VS2 #2 — kênh**: `chan[T]`, `send/recv/close`, **deadlock có chẩn đoán** | ST-60 | 3 h | **DONE** | ≥15 ca; ca deadlock **không treo**, mã lỗi rõ |
| ST-62 | báo cáo **R10** | ST-56..ST-61 | 1,5 h | **DONE** | MD+JSON; số liệu tái lập; nêu nợ còn lại |
| ST-63 | **VS2 #3a** — `NetHost` + TCP client (`std.net.connect/read/write/close`, timeout) | ST-62 | 2,5 h | **DONE** | 5 ca e2e loopback hai đường; không host ⇒ `VS0604` |
| ST-64 | **VS2 #3b** — `http.get`/`get_timeout` (giao thức trong `vis-interp`) | ST-63 | 2,5 h | **READY** | 6 ca e2e: 200/404/timeout/refused/8 MiB/sai kiểu |
| ST-65 | **VS2 #3c** — server: `net.listen/local_addr/accept(_timeout)` + e2e 2 tiến trình | ST-64 | 3 h | **READY** | 4 ca; bàn giao cổng qua tệp; ≤3 s, không flake 3 lần |
| ST-66 | **VS2 #3d** — ca biên net/http + `--explain-perms` NET + `--vm` cache/no-cache | ST-65 | 2 h | **READY** | ≥13 ca tổng; `gate` xanh |
| ST-67 | **VS2 #5a** — `vis lsp`: khung Content-Length + initialize/shutdown/exit + lỗi framing | ST-66 | 2,5 h | **READY** | 6 snapshot; stdout sạch; thoát khi EOF |
| ST-68 | **VS2 #5b** — LSP diagnostics + vị trí **UTF-16** (emoji, tab, CRLF) | ST-67 | 3 h | **READY** | 6 snapshot + 6 unit vị trí |
| ST-69 | **VS2 #5c** — LSP completion/hover + ≥13 phản hồi snapshot | ST-68 | 2,5 h | **READY** | ≥13 frame; prefix/hover/null |
| ST-70 | **VS2 #6a** — `vis pkg add/install/list`, manifest/lock/SHA-256, registry local | ST-69 | 3 h | **READY** | 6 ca + e2e tất định; không mạng |
| ST-71 | **VS2 #6b** — `use pkg.*` nạp gói + kiểm sha lúc chạy | ST-70 | 2,5 h | **READY** | 4 ca; lock không có đường tuyệt đối |
| ST-72 | **VS2 #4a** — quyền: `cap[Dir]`/`cap[Net]` cưỡng chế ở host + `--cap-mock` | ST-71 | 3 h | **READY** | ≥8 ca cho phép/từ chối; hai đường cùng byte |
| ST-73 | **VS2 #4b** — Landlock thật trên Linux (cfg linux) + tài liệu giới hạn | ST-72 | 3 h | **READY** | ≥8 ca Linux; trên macOS **không** giả vờ chạy |
| ST-74 | **VS2 #7** — `vis-host-windows`/`wasm`: check chéo `x86_64-pc-windows-msvc` + `wasm32-unknown-unknown` | ST-73 | 2,5 h | **READY** | cả hai target `check` xanh; nêu rõ mức bằng chứng |
| ST-75 | **VS2 #8** — bộ hợp chuẩn host `host-conformance` ≥50 ca chạy **trên macOS** + khung 4 nền | ST-74 | 3 h | **READY** | ≥50 ca xanh; 3 nền còn lại ghi rõ chưa đo |
| ST-76 | báo cáo **R11** | ST-63..ST-75 | 1,5 h | **READY** | MD+JSON; nêu rõ hạng mục VS2 nào **chưa** đo được |
| ST-31 | báo cáo **R4** | ST-25..ST-30 | 1,5 h | **DONE** | MD+JSON, 36 hàng, số liệu tái lập |
| ST-24 | báo cáo **R3** | ST-18..ST-23 | 1 h | **DONE** | MD+JSON, 27 hàng, số liệu tái lập |
| ST-17 | báo cáo **R2** (`reports/R2-viscript.*`) | ST-11..ST-16 | 1 h | **DONE** | MD+JSON, 20 hàng QUEUE, số liệu tái lập được |
| ST-04 | lexer theo SPEC-lex (ST-02a đã DONE) | ST-02a | 3 h | **DONE** | **60 golden token đỏ trước**; fuzz 10 phút + NFC/NFD |
| ST-05 | parser + CST lossless | ST-04 | 3 h | **DONE** | **30 golden biểu thức đỏ trước**; 100 % fixture tài liệu parse; mỗi tệp lỗi đúng 1 lỗi đầu |
| ST-06 | CmdCST + golden từng hàng bảng 9.4 + 10 ca mơ hồ | ST-05 | 3 h | **DONE** | golden AST/CST |
| ST-07 | `vis-fmt` lossless, idempotent, sửa tab | ST-05 | 3 h | **DONE** | vòng tròn parse→fmt→parse |
| ST-08 | runner `--check` cú pháp (chưa interpreter) | ST-05 | 2 h | **DONE** | TAP; chạy fixture |
| ST-09 | fixture theo QD §7.4 (1/khối mã + 1/quyết định; cấm `...`) | ST-06,07,08 | 2 h | **DONE** | xanh |
| ST-10 | báo cáo `R1-viscript` | các task | 1 h | **DONE** | MD + JSON có `project` |

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

APPROVED 2026-11-13 Agent 0

> **ST-19 DONE** — commit `750815b` (đã push): crate `crates/vis-bytecode` (không phụ thuộc OS, không `unsafe`) —
> `VBC1` + u16 + 32 byte hash nguồn, const pool nguyên văn, bảng hàm (`arity/nparams/nlocals/max_stack`) + code +
> **bảng span song song**; **45 lệnh**; `Bin`/`Un` mang chính `vis_types::{BinOp,UnOp}`; **`match` = chuỗi test +
> nhảy** (VM không biết pattern); closure `MakeClosure`+`LoadCaptured` (snapshot, VD-51); `DecodeError` 9 biến thể,
> không panic với dữ liệu hỏng. Sinh mã **272/323** ca sem (51 ca lỗi tĩnh), **0 Unsupported**, 3.827 lệnh, 36/45 tag;
> round-trip xanh trên 272 ca + 27 golden. **284 test**, `gate --fast` PASS.
> **ST-20 mở** (`vis-vm`).

APPROVED 2026-11-13 Agent 0

> **ST-20 DONE** — `65cb63b`: `crates/vis-vm` (fetch–decode–execute, ngăn xếp `Vec<Value>` động — **không** tin
> `max_stack`, cap khung 2.000 + cap 10^7 lệnh, bytecode hỏng ⇒ `VS0501` không panic) + **refactor `vis-interp`**
> để mọi ngữ nghĩa (toán tử/builtin/`Show`/`eq`/`iter`) **dùng chung** (284 test + 323 ca xanh y nguyên trước khi
> thêm VM); codegen sửa theo differential (bind pattern, thứ tự bind→guard, `MakeEnum`, closure, `CallValue`);
> **`vis run --vm`** + `--dump-bytecode`. 289 test; differential 253/272 (19 ca lệch khoá trong `KNOWN_LECH`).
> **ST-20b DONE** — `516f668`: sửa **đúng 19 ca** (pattern lồng nhiều tầng, `some/none/ok/err`, `()` vs tuple rỗng,
> `?.` phẳng, `JumpIfNone` luôn pop, `LoadCaptured` trong closure, named-arg theo thứ tự tham số, `T?` tự nâng,
> một quy ước đích nhảy duy nhất) ⇒ **272/272 khớp 0 lệch**, `KNOWN_LECH` **rỗng** + test khoá **hai chiều**.
> **ST-21 mở** (verifier).

APPROVED 2026-11-13 Agent 0

> **ST-21 DONE** — `70a5d38`: `verify(&Module)` + `need_stack` mô phỏng ngăn xếp trên **worklist CFG** (độc lập
> metadata; codegen dùng **chính** hàm này nên `declared == need`), **13 `VerifyError`**, `load_verified`,
> `Vm::run` verify trước; verifier chặn **4 bug codegen**, fuzz đột biến **10 phút 2.289.260 vòng / 0 vi phạm**
> (lần đầu vi phạm ⇒ **sửa VM: mỗi khung một ngăn xếp giá trị**). **316** test.
> **ST-22 DONE** — `f071166`: **100.000 chương trình sinh tự động, 0 lệch** (`DIFF tong=100000 loi_tinh=0
> chay=100000 lech=0 giay=189.2 seed=3523158054`), cổng chạy 1.000 (~1,3 s) + `bin/vis-diff` cho 100k; bắt 1 bug
> span `IterInit`. **ST-23 DONE** — `213e691`: cache `$XDG_CACHE_HOME/vifast/vis/<hash>.vbc` (FNV-1a×4 trên
> compiler+VBC1+mode+toàn văn), nạp **luôn** `load_verified`, ghi tệp tạm + rename, `--no-cache`/`vis cache`;
> **khởi động ấm: 8,96–10,57 ms trung vị, p95 ≤ 11,2 ms ⇒ ĐẠT §19 < 20 ms** (lợi ích cache 7–8 % vì chi phối là
> khởi động tiến trình ~8 ms; mặc định vẫn **interp** — xem VD-56).
> **ST-24 mở** (R3).

APPROVED 2026-11-13 Agent 0

> **ST-24 DONE — VÒNG 3 (KHUNG VS1) CHỐT.** `reports/R3-viscript.{md,json}` (`c194ae7`) — 27 hàng QUEUE, số liệu
> tái lập được: **318 test** (20 diag + 96 syntax + 18 fmt + 65 types + 24 interp + 45 bytecode + 8 vm + 42 cli);
> **323 ca ngữ nghĩa/17 nhóm**; differential gốc **272/272**; **100.000 chương trình sinh tự động, 0 lệch**
> (192,6 s, 519/s, seed 3523158054); **VBC1 53 tag**; verifier **13 biến thể / 26 ca**; fuzz đột biến **10 phút
> 2.311.244 vòng / 0 vi phạm**; **khởi động ấm 8,51 / 9,92 / 8,64 ms trung vị** (p95 ≤ 10,33) ⇒ **đạt §19 VS1 < 20 ms**;
> `gate --fast` PASS; `nightly --fuzz-minutes 1` PASS.
> **Đối chiếu §19 VS1**: **đạt** differential 100k 0 lệch + khởi động ấm < 20 ms; **chưa** generic/trait/module/
> `vis-std`/`vis-host-posix`/`vis run|check|fmt|test` đầy đủ ⇒ **VS1b** (15 mục liệt kê ở §5 báo cáo).
> Dòng ViScript **hết task trong QUEUE**; vòng 4 (đề xuất): VS1b (generic/trait/module, `vis-std`, host posix,
> `cmd` variadic/tham chiếu theo tên, SHA-256 cho consent, đổi mặc định sang VM có đo lại).

APPROVED 2026-11-13 Agent 0

> **ST-25 DONE** — commit `4a03c6d` (đã push): `vis-cli/src/modules.rs` (giải `use ./x`, `./x` ≡ `./x.vis`, `..` +
> `canonicalize`, `std.*` ⇒ `VS0701` "ST-26", `pkg.*` ⇒ `VS0701` "VS2", DFS hậu duyệt, **cấm vòng**, `--dump-deps`);
> `lower_into` **append** vào `Symbols`/`TypeTable` chung ⇒ id **ổn định**; **tầm nhìn `pub` kiểm ở `lower::resolve`**
> (không kéo `tyck`): tên từ module khác cần `pub` **và** `use`, sai ⇒ `VS0301`; parser nhận `use ./x`; `khoa_multi`
> FNV-1a×4 trên compiler+VBC1+mode+Σ(rel+text) ⇒ sửa phụ thuộc **đổi khoá cache**; sổ thứ bảy `MODULE_CODES` `07xx`
> ⇒ `BOOKS` 7, `all_codes()` 28, `next_block()` **801**. **335** test; 323 ca + 272/272 differential xanh; gate PASS.
> Hoãn có ghi: `pub` cho `let`/`const`; `dòng:cột` runtime của hàm trong tệp phụ vẫn theo văn bản tệp chính (HIR chưa
> mang `file`); `use` không re-export; biên `..`.
> **ST-26 mở** (`vis-std` lõi).

APPROVED 2026-11-13 Agent 0

> **ST-26 DONE** — commit `3a54757` (đã push): `vis-types/src/stdlib.rs` (bảng tên + chữ ký + `HA_CHUA_HO_TRO`) +
> `vis-interp/src/stdlib.rs` (một hiện thực, `builtin::call` chuyển tiếp `std.*`, **VM dùng chung** ⇒ **differential
> 70/70**); tên **đầy đủ** `str.split(…)`/`path.join(…)`/`time.now_ms()`/`json.encode(…)`; `std.list.*`/`std.map.*`
> dùng **`Any`** (tyck kiểm số đối số; sai kiểu ⇒ `VS0501` nêu hàm + kiểu nhận) — nợ **VS1c** `list[T]`/`map[K,V]`;
> `sort_by` = closure khoá, tăng dần, **ổn định** (có ca chứng minh); `json.encode` + `encode_pretty`, `json.decode`
> **không** export ⇒ `VS0301` "chưa hỗ trợ (VS1b)"; `use std.{str,list,map,path,time,json}`, `std.fs` ⇒ `VS0701`
> "ST-27". **344** test, **70** fixture std (18/20/9/12/2/10) + 6 ca lỗi; 323 ca + 272/272 differential gốc xanh;
> gate PASS. Hai lỗi của bộ sinh/kỳ vọng được khai trong log, sửa bộ sinh/kỳ vọng **không nới test**.
> **ST-27 mở** (`vis-host-posix`).

APPROVED 2026-11-13 Agent 0

> **ST-27 DONE** — commit `a98d0e8` (đã push): **chương trình ngoài** (§9.3) chạy thật qua `std::process::Command`
> (không shell, đối số mảng, env kế thừa); stdout ⇒ `list[str]` hoặc `--raw` ⇒ `str`; stderr ⇒ **chẩn đoán**;
> `exit != 0` ⇒ `VS0501` (§9.4); **ranh giới OS giữ đúng**: `vis-interp` chỉ biết `trait Ngoai`, CLI cài runner,
> **không host** ⇒ `VS0604` (differential 4 ca `.vish` khớp hai đường); `--no-extern` ⇒ `VS0601`; **variadic
> `...rest`** + **`--x=v`**; timeout 10 s, chạy tuần tự, `--explain-perms`. **357** test; **328 ca sem/17 nhóm**;
> differential **276/276**; gate PASS. **Allowlist doc-examples 9 → 8** (`APPROVED e009411`) vì khối 662 dạng
> `extern cmd … ...rest` nay **hợp lệ** — chặt hơn, không nới. Hoãn: `| complete`, pipe→stdin, consent thật/cap[Proc].
> **ST-28 mở** (generic).

APPROVED 2026-11-13 Agent 0

> **ST-28 DONE** — commit `0423e67` (đã push): **generic** hàm + kiểu `[T]` theo hướng **xoá kiểu + hợp nhất ở
> call-site** (không đơn hình hoá ⇒ **một** hiện thực, differential 0 lệch **theo cấu trúc**): `FnSig{generics,
> raw_params, raw_ret}`, thân hàm dùng `erase_gen` (không `VS0301` oan), call-site `hop_nhat` đệ quy ⇒ mâu thuẫn
> `VS0412` nêu `T` + hai kiểu; record/enum generic payload xoá kiểu, `match` chạy đúng cả 2 kiểu. **Hai bug thật
> được phát hiện & sửa**: (i) `vis check` lọc theo lô khác `vis run` ⇒ `check` nay báo **đúng chẩn đoán đầu**,
> lọc chỉ ở `--syntax` (cổng tài liệu giữ 24 khối = 16 xanh + 8 ngoại lệ); (ii) **VM bỏ qua `fn main`** (entry vào
> thân cấp tệp) — differential trước đó **chưa phủ** chương trình có `main`, nay entry phát `Call(main, 0)`.
> **392** test; **338 ca sem/18 nhóm**; differential **286/286**; gate PASS.
> Hoãn (VS1b): instantiation thật (`Hop[int] ≠ Hop[str]`), phép toán trên `T` kiểm động, constraint/trait ⇒ ST-29.
> **ST-29 mở** (trait + `derives`).

APPROVED 2026-11-13 Agent 0

> **ST-29 DONE** — commit `7633a56` (đã push): **dispatch TĨNH ở tầng hạ** (VD-62) — `x.m(…)` với `x` kiểu tĩnh
> `Record`/`Enum` hạ thẳng `Call(impl_fn)` + `self` theo giá trị ⇒ **cùng HIR** cho interp **và** VM, **không** bảng
> `impls`, **không** bump VBC1 (ca dispatch in `hop`/`7` khớp hai đường); 3 tầng phân giải (nội tại → impl tĩnh →
> `VS0403`); impl↔trait (thiếu/thừa/mơ hồ/mồ côi/`derives` lạ/`Json` trên closure) ⇒ `VS0403` **mở rộng nghĩa**
> (vi/en/ja + snapshot, `TYPE_CODES` vẫn 4, `all_codes()` 28); **`derives` chạy thật** (Eq/Show/Json dùng ngữ nghĩa
> sẵn có); đăng ký hàm `impl` ở interp + codegen; tách `lower/trait_impl.rs` khi R1 đỏ. **421** test; sem 338 +
> differential 286 **giữ nguyên**; gate PASS.
> **Sự cố của tôi (Agent 0)**: commit `520c2fe` (thông điệp `[ST-29]`) đã **cuốn theo 3 tệp đầu của ST-30** (`sha2`
> + hash SHA-256) — cây vẫn xanh, không mất gì, không rewrite lịch sử; đã yêu cầu Agent 2 ghi rõ trong `[ST-30]`.
> **ST-30 mở** (`sha2` + `vis test`).

APPROVED 2026-11-13 Agent 0

> **ST-30 DONE** — `02b97ad`: **`vis test [--vm] [đường dẫn]`** (TAP `1..338`, exit 0/1/2, tự tìm `test/sem`,
> quy ước `.out`/`.err`) + **SHA-256 thật** (`sha2 0.10` — duyệt allowlist, **gỡ nợ VD-54 (1)**: hash nguồn VBC1 **và**
> `cache::khoa_multi`, cache cũ tự miss, nạp vẫn qua `load_verified`; vector FIPS khoá trong test). **435** test.
> **ST-30b DONE** — `e11efae`: **đồng bộ chẩn đoán hai đường** (VM `index` ⇒ `VS0503` + **đúng câu** interp, thêm nhánh
> `Bytes`, thông điệp đệ quy sâu; interp "tên …" thay "hàm …"); **differential so ĐỦ** `(mã,dòng,cột,thông điệp)` trên
> 338 ca ⇒ **312/312 khớp, 0 lệch** (26 ca bỏ qua là **lỗi biên dịch** — nợ); `vis test --vm` **so đúng mã** (không nới),
> **6 SKIP** đúng TAP cho ca cần host. **436** test. Sự cố `520c2fe` (3 tệp ST-30 lỡ nằm trong commit ST-29) do **tôi**
> gây ra, đã ghi trong commit/JOURNAL — không rewrite lịch sử.
> **ST-31 mở** (R4 — chốt VS1b).

APPROVED 2026-11-13 Agent 0

> **ST-31 DONE — VÒNG 4 (VS1b) CHỐT.** `reports/R4-viscript.{md,json}` (`4748ec4`, `head` d92e947) — **436 test**
> (21 diag + 96 syntax + 18 fmt + 118 types + 24 interp + 50 bytecode + 9 vm + 100 cli), **338 ca sem/18 nhóm**,
> `vis test` **338 ok** / `--vm` **338 ok + 6 SKIP**, **parity chẩn đoán 312/312 khớp 0 lệch**, differential
> **100.000 chương trình 0 lệch** (203,5 s, 491/s, so **đủ** cả thông điệp), **SHA-256** thay FNV, `VBC1` 53 tag,
> **7 sổ mã / 28 mã**, `gate --fast` PASS + `nightly --fuzz-minutes 1` PASS, khởi động ấm 10,3–11,1 ms (debug,
> đường `--vm`). **VD-57..VD-63**; **13 lỗ hổng** đã liệt kê; §19 VS1 **đạt phần khung**, phần còn lại = **VS2**.
> Dòng ViScript **hết task trong QUEUE** (36/36 DONE). Vòng 5 (đề xuất, ưu tiên): 26 ca lỗi biên dịch chưa so
> chẩn đoán hai đường · `&self` · `json.decode` · `std.fs` · `| complete`/pipe→stdin · consent/`cap[Proc]` ·
> `Hop[int] ≠ Hop[str]` (generic thật) · `pkg.*`/registry ⇒ VS2.

APPROVED 2026-11-13 Agent 0

> **ST-32 DONE** — commit `774369b` (**Agent 2 tự commit**): harness 3 đường so **cả 338 ca** (286 xanh / 26 lỗi
> biên dịch / 26 lỗi lúc chạy, **0 ca bỏ qua**), **0 lệch**; ca biên dịch so `check` ↔ `run --vm --no-cache` cùng
> `(mã,dòng,cột,thông điệp)` + **ánh xạ exit 1↔2** (không đòi bằng); `chuan_hoa` dùng chung 3 đường + unit-test;
> **`vis_bytecode::compile()` gọi tới cả 26 ca** (không panic); **phát hiện**: **21/26 ca codegen vẫn sinh module và
> VM chạy sạch** dù HIR sai kiểu ⇒ codegen **tin tyck**, khoá hằng số `CODEGEN_TIN_TYCK = 21` (nợ có ý thức).
> **443** test; parity 312/312; differential 100k 0 lệch; gate PASS.
> **ST-33 mở** (`json.decode` + `std.fs` qua `Host`).

APPROVED 2026-11-13 Agent 0

> **ST-33 DONE** — commit `f9e6db5` (**Agent 2 tự commit**): **`json.decode`** (`Result[Any,str]`, object giữ thứ tự,
> number `.`/`e` ⇒ float, tràn i64 ⇒ `Err` ở **byte** đầu token, escape đủ + **surrogate pair** ⇒ `😀`, nửa cặp ⇒ `Err`,
> `null` ⇒ `none`, lỗi nêu **byte offset** đo trên chuỗi Việt+emoji, depth cap 128) + **`std.fs`** (`read/write/exists/list`
> qua **`FsHost`** trên `Host`, mặc định `None ⇒ VS0604` **cả hai đường**; `vis-cli` impl bằng `std::fs`, `--explain-perms`
> in `FS …`; cho phép đường dẫn tuyệt đối/`..`, consent thật ⇒ VS2). **466** test (+23); ~20 ca round-trip JSON, 6 ca FS;
> **338 ca sem giữ nguyên**, parity 312/312, differential 100k **0 lệch** (201,9 s). 3 test cũ sửa **kèm lý do**
> (`std.fs`/`json.decode` nay hợp lệ). **ST-34 mở** (`| complete` + pipe→stdin).

APPROVED 2026-11-13 Agent 0

> **ST-34 DONE** — commit `4ababc6` (**Agent 2 tự commit**): **pipe→stdin** (`a | b` với `b` ngoài ⇒ ghi stdout của `a`
> vào stdin `b`; str nguyên văn, `list[str]` nối `\n`; cap `MAX_STDIN` **8 MiB** ⇒ `VS0501` nêu số byte **trước khi
> spawn**; thread ghi + `drop` + bỏ qua `EPIPE` sau timeout) + **`| complete`** (`{stdout, stderr, code}`, `grep` không
> khớp **hết là lỗi**; chỉ cho stage ngoài, Vis ⇒ `VS0602`); timeout 10 s/**stage** nêu **stage nào**, không cap tổng.
> `Ty::Any` cho `complete` (nợ kiểm tĩnh). **482** test (+16 `pipeline.rs`) + 4 fixture `.vish`; **338 ca sem giữ
> nguyên**; parity 312/312; differential 100k **0 lệch** (206,2 s). **ST-35 mở** (generic thật).

APPROVED 2026-11-13 Agent 0

> **ST-35 DONE** — commit `a0b5525` (**Agent 2 tự commit**): **generic thật** — `RecId`/`EnumId` **interned theo tổ hợp**
> (`Hop[int]` ≠ `Hop[str]`), `inst_args` + `rec_pos`/`enum_pos`, `Hop[T]` chưa ràng buộc **giữ** hành vi xoá kiểu;
> `a.gia_tri` ⇒ **`int`**; hàm liên kết `f[T](x: T, h: Hop[T])`; `Or[int,str]` (`match` thiếu nhánh ⇒ `VS0402`); gán
> chéo hai instantiation ⇒ `VS0412`, cùng instantiation ⇒ sạch; `Hop[Hop[int]]` chạy được (nợ: chưa tạo id cho lớp
> trong). **Không** đụng `codegen`/`VBC1` ⇒ differential **100.000 chương trình 0 lệch** (228,6 s). **507** test;
> **sem 338 → 346** (+8) với bảng cũ→mới ở **đủ 6 chỗ khoá số**; parity **320/320**; `vis test` **346 ok**.
> **ST-36 mở** (R5 — chốt VS1c).

APPROVED 2026-11-13 Agent 0

> **ST-36 DONE — VÒNG 5 (VS1c) CHỐT.** `reports/R5-viscript.{md,json,license}` (`97d3dd4`, `head` 58080a9) —
> `report-check` **PASS (5 báo cáo, 49 hàng QUEUE phủ đủ)**. Số đo **chạy lại**: **507 test** (21 diag · 96 syntax ·
> 18 fmt · **143** types · 24 interp · 50 bytecode · 9 vm · **146** cli) · **346 ca sem/18 nhóm** (327 `.vis` + 19 `.vish`,
> 52 `.err`) · `vis test` **346 ok** / `--vm` **346 ok + 6 SKIP** · **parity 320/320** · **differential 100.000 chương trình
> 0 lệch** (200,4 s, 499/s) · fuzz parser 60 s **3.035.575 ca/0 vi phạm** · `VBC1` 53 tag · **7 sổ/28 mã** · khởi động ấm
> 11,2–11,7 ms (debug) · `gate --fast` + `nightly --fuzz-minutes 1` PASS. **5 lỗ hổng đóng/nới** trong vòng (26 ca biên
> dịch đã so; `grep exit 1` hết là lỗi; `json.decode`; `std.fs`; `Hop[int] ≠ Hop[str]`), **15 nợ còn lại** ghi rõ.
> **VS2** (async/kênh/net/Landlock/LSP/gói/4 nền tảng) là vòng sau. QUEUE: **41/41 hàng DONE**.

APPROVED 2026-11-13 Agent 0

> **VÒNG 6 mở (VSH1a — chế độ shell dùng được, §9.5–§9.7)**: 8 hàng `ST-37..ST-43` (ST-40 tách a/b) ≈ 21,5 h ≈ 3 vòng.
> Căn cứ lộ trình §19 (bước "shell dùng được") + pipeline mẫu §9.7 (khối tài liệu dòng 672 **còn** trong allowlist 8 mục).
> **ST-37 mở**; thứ tự bắt buộc ST-37 → ST-38 → … → ST-42 → ST-43; ST-37..ST-42 đụng `lower/cmd.rs` + `tyck` nên
> **không** chạy song song. Allowlist `doc-examples` **chỉ** được giảm **8 → 7** khi ST-42 **thật xanh** (Agent 0 sửa).

APPROVED 2026-11-13 Agent 0

> **ST-37 DONE** — commit `02daf9f` (**Agent 2 tự commit**): **khe ống hợp nhất** (chọn vị trí theo **khả năng nhận**
> bằng hợp nhất một chiều; `up_ty = {error}` ⇒ khe **vị trí đầu**), `CmdSig`/`SigSource` ở `vis-types` (`cmdsig.rs`),
> `kieu_stage` cho `CmdGroup` suy kiểu literal ⇒ `( [1,2] ) | take 1` ⇒ **`[1]`**; `VS0603`/`VS0602` giữ nghĩa cũ.
> **521** test (+14 `khe_ong`); sem **346 → 349**, `vis test` **349 ok**, parity **323/323**, `fuzz_mutate` 297;
> differential 100k **0 lệch** (197,8 s). **Allowlst doc-examples 8 → 6** (VD-70): khối **1451** và **672** (§9.7) nay
> **hợp lệ** — chặt hơn, không nới (APPROVED `59a407c` + `260fe8a`). Nợ **VSH1b**: `[1,2] | take 1` không ngoặc ⇒
> `VS0201`; `( … )` làm đối số ⇒ `VS0603`. **ST-38 mở** (closure-arg + `where`/`each`/`first`/`count`).

APPROVED 2026-11-13 Agent 0

> **ST-38 DONE** — commit `86f7804` (**Agent 2 tự commit**): **closure-arg** hạ thành `Lambda` với span **tuyệt đối**
> bằng đệm **byte** (có ca chứng minh bẫy nhiều byte: dòng tiếng Việt + emoji trước closure ⇒ cột **không đổi**);
> **4 lệnh mới** `where`/`each`/`first`/`count` (hai chỗ `cmdsig` + `builtin`, **một** hiện thực qua `Host::call`);
> `each` trả `()` và là **stage cuối** (nối tiếp ⇒ `VS0603`); `first` rỗng ⇒ `none`; **sugar `it`** (`where size > 5mb`
> ⇒ `it.size > 5mb`, `where 1` ⇒ `VS0412`); `BUILTIN_VALUE_NAMES` **giữ 11**. **538** test (+17); sem **355**
> (336 `.vis` + 28 `.vish`), `vis test` **355 ok**, parity **329/329**; differential 100k **0 lệch** (229,5 s).
> **Allowlst giữ 6** (khối 672/1451 vẫn xanh). Hai bug tự phát hiện & sửa: `lower_vach` hạ vào **bảng chung**
> (`RecId` ẩn danh); **không** thêm 4 lệnh vào `BUILTIN_VALUE_NAMES` (làm hỏng `fn first` của người dùng ⇒ `VS0302`).
> Nợ: `where size` (kiểu không `bool`) hiện `VS0501` lúc chạy — "kiểm tĩnh khi pipe chưa rõ kiểu".
> **ST-39 mở** (`sort_by --desc`).

APPROVED 2026-11-13 Agent 0

> **ST-39 DONE** — commit `1c3f1d3` (**Agent 2 tự commit**): **`sort_by` shell** = lệnh builtin **mới** (một hiện thực
> qua `Host::call`), `--desc` là **cờ** (không bị khe ống chiếm), khoá `int`/`size`=**byte**/`duration`=**ns**/`str`=
> byte UTF-8, **ổn định cả hai chiều** (đảo comparator, có ca `--desc` + khoá `size` bằng nhau); khoá `bool` ⇒
> **`VS0412`** (kiểm ở lowering cho `.vish` + `kiem_builtin` cho `.vis`, runtime chốt `VS0501`). Phát hiện & sửa:
> parser **nuốt `--desc` vào thân closure**. **550** test (+12); sem **359** (336 `.vis` + 32 `.vish`), `vis test`
> **359 ok**, parity **333/333**; differential 100k **0 lệch** (202,8 s); allowlist **vẫn 6**. Nợ: **hai** `sort_by`
> (shell vs `std.list`) ⇒ hợp nhất ở VS2; chạy end-to-end khối 672 ⇒ ST-42.
> **ST-40a mở** (prelude bản ghi + `it` + viết tắt trường + `VS0605`).

APPROVED 2026-11-13 Agent 0

> **ST-40a DONE** — commit `9b97d75` (**Agent 2 tự commit**): **prelude bản ghi** `FileEntry`/`ProcEntry` khai trong
> `vis-types/src/prelude.rs`, đăng ký **chỉ cho `.vish`** (`.vis` không thấy ⇒ không `VS0302` oan); `it` nay suy **kiểu
> thật** từ kiểu phần tử (`kieu_text`/`kieu_stage`, **không** đụng `CmdSig`), không suy được ⇒ **kiểm động** như cũ;
> **`VS0605`** "trường không có trên `it`" vào `COMMAND_CODES` (4 → 5) ⇒ `all_codes()` **29**, `BOOKS` 7,
> `next_block()` 801. **563** test (+13); sem **363** (336 `.vis` + 36 `.vish`), `vis test` **363 ok**, parity **337/337**;
> differential 100k **0 lệch** (204,2 s); allowlist **vẫn 6**; khởi động **+0,08 ms** (10,06 → 10,14). Phát hiện: `.vish`
> **không có** cú pháp khai `cmd`/`type`/`let` có kiểu ⇒ ca "chạy thật" dùng **literal bản ghi khớp prelude** (đã khoá
> interp ↔ `--vm`); nợ: khi `.vish` có cú pháp khai `type` thì phải `VS0302` với prelude.
> **ST-40b mở** (`get`/`select`).

APPROVED 2026-11-13 Agent 0

> **ST-40b DONE** — commit `942bcff` (**Agent 2 tự commit**): **`get`** (`get name` ⇒ sugar `it => it.name`, `list[K]` thật)
> + **`select`** (`select pid, name` ⇒ hạ lambda trả **bản ghi ẩn danh** `list[record#N]` với trường lấy từ `it`, giữ **đúng
> thứ tự đối số**) — **không** thêm `Type::Any`; trường lạ ⇒ **`VS0605`**; `it` kiểu `{error}` ⇒ **kiểm động** (không oan);
> `get` trên list rỗng ⇒ `[]`. **576** test (+13); sem **367** (336 `.vis` + 40 `.vish`), `vis test` **367 ok**, parity
> **341/341**; differential 100k **0 lệch** (225,9 s); allowlist **vẫn 6**; R1 đỏ ⇒ **tách** `lower/chieu.rs` (188 dòng,
> `cmd.rs` 1.390). Nợ: `select` bản ghi ẩn danh chưa `to json` được (cần `Type::Any` ⇒ VS2).
> **ST-41 mở** (`Render`).

APPROVED 2026-11-13 Agent 0

> **ST-41 DONE** — commit `f2c256f` (**Agent 2 tự commit**): **`Render`** (`vis-interp/src/render.rs` + builtin `render`,
> `boc_print` → `boc_render`): `list[bản ghi]` ⇒ **bảng** (căn trái `str`, phải số/`size`/`duration`/`percent`, tiêu đề =
> tên trường), bản ghi đơn ⇒ **khối khoá–giá trị**, `list[bản ghi]` rỗng ⇒ **`(rỗng)`** — **`list[str]`/`list[int]` rỗng
> giữ `[]`**; độ rộng = **số `char` sau NFC** (`unicode-normalization` có sẵn) ⇒ NFD/NFC, emoji, CJK đều đúng; **`print`
> giữ nguyên từng byte**; interp ↔ `--vm` **cùng byte**. **592** test (+16 snapshot); sem **370** (336 `.vis` + 43 `.vish`),
> `vis test` **370 ok**, parity **344/344**; differential 100k **0 lệch** (206,4 s); allowlist **vẫn 6**;
> **7 `.out` đổi cùng lý do `Render`** (liệt kê trong commit) + ~20 assertion Rust. Phát hiện: **cache cũ** trả `Show` ⇒
> nhãn `cache::PHIEN_BAN` `"+render-st41"`. **ST-42 mở** (`ls`/`open` qua `FsHost` + `--fs-mock`).

APPROVED 2026-11-13 Agent 0

> **ST-42 DONE** — commit `331dc58` (**Agent 2 tự commit**): **`ls`** ⇒ `list[FileEntry]` (**kiểu thật**, sắp xếp theo
> tên byte, `--all`, `--long` no-op + nợ, không đối số ⇒ `"."`) + **`open`** ⇒ `str` / `VS0501` nêu đường dẫn;
> **`FsHost` mở rộng** `MucFs` + `list_entries` (**default `Err`** ⇒ impl cũ vẫn biên dịch), `Runner` thật dùng
> `std::fs::metadata`; **`--fs-mock <json>`** (`dirs`/`files`, `size` theo **bảng lexer**: `2mb` = 2 000 000 và `2mib` =
> 2 097 152 — khoá bằng ca), `modified` = duration **từ epoch** ⇒ tất định; **kiểm `doc-examples`: allowlist vẫn 6**
> (không khối đang đỏ nào chuyển xanh). **606** test (+14 `ls_open`); sem **370 không đổi** (4 fixture `test/cmd/fs/**`
> chạy bằng test riêng vì `vis test` không có `--fs-mock`) ⇒ **6 chỗ khoá số không đổi**, ghi rõ trong commit thay vì
> tăng số ảo; differential 100k **0 lệch** (207,2 s). Sửa: cờ bị bỏ khi ô sau cờ để trống (phải điền default **mọi**
> ô trống); R3 cần sidecar `.license` cho `mock.json`.
> **ST-43 mở** (R6 — chốt VSH1a).

APPROVED 2026-11-13 Agent 0

> **ST-43 DONE — VÒNG 6 (VSH1a) CHỐT.** `reports/R6-viscript.{md,json,license}` (`a63914a`, nền 05f3311) —
> `report-check` **PASS (6 báo cáo, 49 hàng QUEUE phủ đủ, `blocked[]` rỗng)**. Số đo **chạy lại**: **606 test**
> (21 diag · 96 syntax · 18 fmt · 143 types · 24 interp · 50 bytecode · 9 vm · **245** cli) · **370 ca sem/18 nhóm**
> (327 `.vis` + 43 `.vish`, 52 `.err`) · `vis test` **370 ok** / `--vm` **370 ok + 6 SKIP** · **parity 344/344** ·
> **differential 100.000 chương trình 0 lệch** (208,0 s, 481/s) · fuzz parser 60 s **2.853.740 ca/0 vi phạm** ·
> `VBC1` 53 tag · **7 sổ/29 mã** (`VS0605`) · khởi động ấm 11,4–11,6 ms (debug) · prelude **+0,08 ms** ·
> `gate --fast` + `nightly` PASS · **allowlist 8 → 6** (khối 1451 + 672 xanh thật; 672 **chạy** từ ST-42).
> **17 lỗ hổng còn mở** ghi trong `R6` §5. Vòng sau (**VSH1b**): cú pháp khai `cmd`/`type`/`let` có kiểu trong `.vish`;
> `where` kiểm tĩnh đầy đủ; `--long` thật; `( … )` làm đối số + chẩn đoán tuyệt đối trong ngoặc.

APPROVED 2026-11-13 Agent 0

> **ST-44 DONE** — commit `32c5af5` (**Agent 2 tự commit**): **`.vish` có khai báo kiểu** bằng cách **tách vùng** ở cột 1
> (từ khoá item + `let`): tiền tố parse lại **Mode::Script** với span **tuyệt đối**, hạ vào **cùng** `Symbols`/`TypeTable`;
> `type`/`enum`/`let x: T`/`cmd f(...) -> T` dùng **nguyên** `item()`/`suite()` của `.vis`; khai báo **sau** stage ⇒
> **`VS0201`** + gợi ý; trùng tên (kể cả prelude) ⇒ **`VS0302`**; `.vish` không khai báo ⇒ hành vi **y hệt vòng 6**
> (4 fixture ST-42 xanh + ca riêng); `.vis` **không** đổi. **622** test (+16 `vish_decl`); sem **370 không đổi**;
> `vis test` **370 ok**; parity 344/344; differential 100k **0 lệch** (203,1 s); allowlist **6**. Phát hiện & sửa:
> `f()` phải là **lời gọi 0 đối số** (không thì bị coi là lệnh `(`); `let x: int = ls | grep foo` vẫn là **shell-let**
> (phân biệt theo RHS) ⇒ không phá golden `test/cmd/golden.cmd` ca 24.
> Nợ mới: **thân `cmd` bằng pipeline lệnh** (thụt) chưa hỗ trợ ⇒ **VSH1b-2** (task riêng).
> **ST-45 mở** (`where` kiểm tĩnh đầy đủ).

APPROVED 2026-11-13 Agent 0

> **ST-45 DONE** — commit `ea42dd1` (**Agent 2 tự commit**): đo trước ⇒ **`where` đã kiểm tĩnh từ ST-38** (`VS0412` nêu kiểu
> thật); việc thật là **nới `each`** (kiểu trả lambda = `{error}` ⇒ không `coerce`, giống `for` trong Rust) + **bộ
> regression 14 ca** giữ `where` không bị nới ngược + ca **kiểu không biết ⇒ kiểm động** (`( [] ) | where size` ⇒ `[]`).
> Ca bắt buộc: `.vish` **khai báo** + `cmd loc() -> list[FileEntry]` + `where size` ⇒ `VS0412`; bản `> 5mb` ⇒ bảng,
> interp ↔ `--vm` **cùng byte**. **636** test (+14); sem 370; parity 344/344; differential 100k **0 lệch** (202,7 s);
> allowlist 6. Khác brief (đo được): `each x => x.khong_co` (closure tường minh) ⇒ **`VS0403`**, còn `where khong_co`
> (viết tắt `it`) ⇒ **`VS0605`** — **cả hai đều đỏ**, 2 ca khoá cả hai mã; thống nhất mã ⇒ task riêng.
> R1 đỏ khi thêm hook ⇒ **tách** `lower/sugar.rs` (271 dòng), `cmd.rs` còn 1.252.
> **ST-46 mở** (`ls --long` thật + `( … )` làm đối số + chẩn đoán tuyệt đối trong ngoặc).

APPROVED 2026-11-13 Agent 0

> **ST-46 DONE** — commit `2207ea4` (**Agent 2 tự commit**): **`ls --long` thật** (`MucFs.mode: u32`, **`FileEntryLong`**
> bản ghi **riêng** 6 trường ⇒ kiểm tĩnh được `select mode`; `mode` = `rw-r--r--`, `modified_time` = **UTC**
> `YYYY-MM-DD HH:MM`, tự viết civil-from-days, **không** thêm crate; không `--long` ⇒ 4 cột **không đổi**) + **nhóm là
> đối số** (`take ( 1 + 1 )`; bỏ `VS0603`, sai kiểu ⇒ `VS0412`; nhóm rỗng vẫn là gọi 0 đối số) + **chẩn đoán tuyệt đối
> trong `( … )`** (đệm **dấu cách**, không `
` ⇒ không `Indent` giả; `( 1 / 0 )` ⇒ **`3:3`**). **651** test (+15);
> sem 370; parity 344/344; differential 100k **0 lệch** (224,8 s); allowlist 6. Kỳ vọng cũ đổi **kèm lý do**:
> `ls_open` (`--long` hết no-op ⇒ 6 cột), `khe_ong` (nhóm-đối-số `VS0603` → `VS0412`).
> Nợ còn: `( ls … --long )` (nhóm-**lệnh** có cờ), nhóm **nhiều dòng** ⇒ `VS0201`, `where ( x => x )` kiểm **động**.
> **ST-47 mở** (R7 — chốt VSH1b).

APPROVED 2026-11-13 Agent 0

> **ST-47 DONE — VÒNG 7 (VSH1b) CHỐT.** `reports/R7-viscript.{md,json,license}` (`a06434b`, nền 426eb91) —
> `report-check` **PASS (7 báo cáo, 53 hàng QUEUE phủ đủ, `blocked[]` rỗng)**. Số đo **chạy lại**: **651 test**
> (21 diag · 96 syntax · 18 fmt · 143 types · 24 interp · 50 bytecode · 9 vm · **290** cli) · **370 ca sem/18 nhóm**
> (327 `.vis` + 43 `.vish`) · `vis test` **370 ok** / `--vm` **370 ok + 6 SKIP** · **parity 344/344** · **differential
> 100.000 chương trình 0 lệch** (212,2 s, 471/s) · fuzz parser 60 s **3.003.112 ca/0 vi phạm** · `VBC1` 53 tag ·
> **7 sổ/29 mã** · khởi động ấm 11,1–11,3 ms · **allowlist 6** · `gate --fast` + `nightly` PASS.
> **§19 VSH1b: 8/8 hạng mục đạt.** **14 nợ còn mở** + **VSH1b-2** + **VS2** (async/kênh/net/Landlock thật/LSP/gói/
> registry/4 nền tảng) ghi trong `R7` §5 — **ViScript chưa hoàn thiện toàn bộ**, khoảng cách đã liệt kê chính xác.

APPROVED 2026-11-13 Agent 0

> **ST-48 DONE** — commit `739fc3e` (**Agent 2 tự commit**): **thân `cmd` bằng pipeline lệnh** (VSH1b-2) theo chiến lược
> **thử-script-trước, lỗi-thì-lệnh** trong parser; **filler `()` giữ nguyên byte** cho tiền tố ⇒ **offset tuyệt đối không
> đổi** (ca: `type` **sau** `cmd` thân-lệnh báo lỗi cố ý ⇒ **`5:5`** đúng); hạ bằng `ha_pipeline` ⇒ `HItem::Fn.body =
> [giá trị cuối pipeline]`, **không** tạo `HExpr::Pipeline`; thân = **đúng một pipeline** (rỗng/>1/`let`+lệnh ⇒ `VS0201`).
> **666** test (+15); sem 370; `vis test` **370 ok** / `--vm` 370 ok+6 SKIP; parity 344/344; differential 100k **0 lệch**
> (174,0 s, 575/s); allowlist **6**. Chống hồi quy đủ 4 điểm (sem, `cmd` script cũ, `--fs-mock` interp↔`--vm` cùng byte,
> item sau thân đúng dòng:cột). Tách `lower/than.rs` khi R1 đỏ.
> Nợ: thân lệnh **chưa dùng tham số `cmd`**; body "vừa script vừa lệnh" ⇒ `VS0201` (bảo thủ).
> **ST-49 mở** (nhóm-lệnh có cờ + nhóm nhiều dòng).

APPROVED 2026-11-13 Agent 0

> **ST-49 DONE** — commit `e0c084a` (**Agent 2 tự commit**): **xoá** `lower_nhom_lenh` (nay **một** đường hạ nhóm:
> newline→dấu cách 1 byte→1 byte, `parse_voi` + span **tuyệt đối**, Lowerer con chung bảng) ⇒ **nhóm-lệnh có cờ** chạy
> (`( ls --long )` ⇒ **6 cột**, `kieu_stage` trả `list[FileEntryLong]` ⇒ `… | select mode` kiểm tĩnh) + **nhóm nhiều dòng**
> (`( 1 +
 1 )` ⇒ 2; `( 1 +
 0 / 0 )` ⇒ **3:3**; nhóm chứa pipeline). **680** test (+14); sem 370; `vis test` **370 ok** /
> `--vm` 370 ok+6 SKIP; parity 344/344; differential 100k **0 lệch** (210,4 s); allowlist **6**.
> **APPROVED** đổi golden `ls | (a | b)` từ `!ERR` → hợp lệ (có dòng lý do), ca 24 không đụng.
> Nợ: trong nhóm chỉ hạ **một** pipeline; kiểu tĩnh lấy **stage đầu**.
> **ST-50 mở** (thống nhất mã field-access + `where ( x => x )` kiểm tĩnh).

APPROVED 2026-11-13 Agent 0

> **ST-50 DONE** — commit `da50f28` (**Agent 2 tự commit**): **`VS0605` → `VS0405`** (chuyển từ sổ **lệnh** `06xx` sang sổ
> **kiểu** `04xx` — trường là **lỗi kiểu**, VD-27; `COMMAND_CODES` 5→**4**, `TYPE_CODES` 4→**5**, `all_codes()` **29**,
> `BOOKS` 7, `next_block()` 801 **không đổi**); `kiem_field` ⇒ `VS0405`, **`VS0403` giữ 6 chỗ khai báo**; hạ dùng `ma_truong()`
> tra theo **id** (không theo chỉ số mảng). **`where ( x => … )` kiểm tĩnh**: nhánh `"where"` trong `kiem_builtin` (thân
> phải `bool` ⇒ `VS0412`; `{error}` ⇒ kiểm động) + hạ gán kiểu tham số lambda = phần tử ⇒ `where ( x => x.khong_co )`
> ra `VS0405` **tĩnh**. **692** test (+12); sem 370; `vis test` **370 ok** / `--vm` 370 ok+6 SKIP; parity 344/344;
> differential 100k **0 lệch**; allowlist **6**. Tách `tyck/truong.rs` (R1). Nợ: `VS0403` còn nghĩa rộng; `where` chưa kiểm
> kiểu tham số khi lambda không chú thích.
> **ST-51 mở** (stage đầu là biểu thức).

APPROVED 2026-11-13 Agent 0

> **ST-51 DONE** — commit `e721858` (**Agent 2 tự commit**): **stage đầu là biểu thức** (không cần ngoặc) — nhận **hẹp
> nhất**: chỉ `[`, `{`, string literal ở **đầu dòng**, có `|` độ sâu 0, tiền tố parse Script **0 chẩn đoán** ⇒
> `[1,2] | take 1` ⇒ **`[1]`**, `{size: 6mb, …} | where size > 1mb` **chạy thật** (bọc `HExpr::List` một phần tử ⇒
> mọi stage hạ nguồn hiểu như một dòng); **không** nhận số/biến/tên lệnh (giữ §9.5). **704** test (+12); sem 370;
> `vis test` **370 ok** / `--vm` 370 ok+6 SKIP; parity 344/344; differential 100k **0 lệch** (212,9 s); allowlist **6**;
> golden ca 24 + ca `ls | (a | b)` xác nhận xanh. Sửa kèm lý do: `khe_ong::khong_ngoac_van_la_loi_parse` →
> `bieu_thuc_dau_khong_ngoac_chay`. Nợ: biến không nhận; bọc-list chỉ cho stage **đầu**; `[1,2]` trần vẫn `VS0201`.
> **ST-52 mở** (R8 — chốt VSH1c).

APPROVED 2026-11-13 Agent 0

> **ST-52 DONE — VÒNG 8 (VSH1c) CHỐT.** `reports/R8-viscript.{md,json,license}` (`a0cbe79`, head a44c357) —
> `report-check` **PASS (8 báo cáo, 58 hàng QUEUE phủ đủ)**. Số đo **chạy lại**: **704 test** (343 cli) · **370 ca
> sem/18 nhóm** · `vis test` **370 ok** / `--vm` **370 ok + 6 SKIP** · **parity 344/344** · **differential 100.000
> chương trình 0 lệch** (211,5 s, 473/s) · fuzz parser 60 s **3.023.954 ca/0 vi phạm** · `VBC1` 53 tag · **7 sổ/29 mã**
> (`VS0405` ở **sổ kiểu**) · khởi động ấm 10,7–11,0 ms · **allowlist 6** · `gate --fast` + `nightly` PASS.
> **Đóng 5 nợ**: thân `cmd` pipeline · nhóm-lệnh có cờ + nhiều dòng + **một** đường hạ nhóm · một mã field-access đúng
> sổ · `where ( x => x )` tĩnh · stage đầu là biểu thức. **Còn 9 nợ lõi + VS2**, ghi trong `R8` §5.
> **Sửa sổ**: hàng ST-51 trước còn `BLOCKED` (lỗi mẫu regex của tôi khi lật) ⇒ nay `**DONE**` cùng ST-52; `report-check`
> vẫn PASS cả trước và sau (máy kiểm đọc `blocked[]` theo `queue`).
> **Vòng 9 (mở sau)**: ST-53.. (nợ lõi) + khung VS2 có tiêu chí đo.

APPROVED 2026-11-14 Agent 0

> **ST-53 DONE** — commit `d68eb16` (**Agent 2 tự commit**): **`to json`** = stage lệnh **còn thiếu** (trước đó chạy `to`
> như chương trình ngoài ⇒ `VS0501`), nay hạ `Call(std.json.encode, [prev])` (+ `--pretty` ⇒ `encode_pretty`), **không**
> cần `use std.json`; bản ghi **ẩn danh** đi JSON đúng **khoá trường + thứ tự** (serializer bỏ tên) ⇒ **không** cần
> `Type::Any`. **Hợp nhất comparator**: một lõi `sap_xep_theo_khoa` (khoá tính 1 lần, **sort ổn định**, `desc` đảo
> comparator) cho **cả** shell `sort_by` (`KieuKhoa::Moi`) và `std.list.sort_by` (`KieuKhoa::ChiInt`, **giữ** hợp đồng
> `FnIntInt`). **718** test (+14); sem 370; `vis test` **370 ok**; parity 344/344; differential 100k **0 lệch** (204,8 s);
> allowlist **6**. Ca bắt buộc: `( ls ) | where size > 5mb | select name, size | to json` với `--fs-mock` ⇒ JSON đúng,
> interp ↔ `--vm` **cùng byte**. Tách `lower/sugar.rs` (`ha_to_json`) giữ R1.
> Nợ: khoá `str` cho `std.list.sort_by` (cần task riêng).
> **ST-54 mở** (`CODEGEN_TIN_TYCK`).

APPROVED 2026-11-14 Agent 0

> **ST-54 DONE** — commit `4627fbb` (**Agent 2 tự commit**): **`CODEGEN_TIN_TYCK` 21 → 0**. Dữ kiện quyết định: walker trên
> 370 ca cho **0 nút `HExpr::Error` / 0 kiểu `{error}`** kể cả ở ca lỗi tyck ⇒ **không thể** chặn bằng soi kiểu; cửa đúng
> là **truyền chẩn đoán** vào codegen: `compile_kiem(hir, symbols, text, chan_doan)` (giữ `compile()` = `&[]`),
> `Err(CodegenError::ChanDoan{short,dong,cot,msg})` mang **đủ mã + dòng:cột + thông điệp**; CLI `run --vm` truyền
> `loaded.diags`; phòng thủ `HExpr::Error` ⇒ `Err(Unsupported("error-expr"))`. `diag_cmp` dùng `compile_kiem` với **đủ 3
> tầng** chẩn đoán ⇒ **26/26 bị từ chối**, so **đủ** mã/dòng/cột/message; **26/26 không còn ca nào ra module chạy sạch**;
> cửa **chỉ chặn `error`** (cảnh báo vẫn compile — có ca). **730** test (+12); sem 370; `vis test` **370 ok**; parity 344/344;
> differential 100k **0 lệch** (203,0 s); allowlist **6**.
> **ST-55 mở** (R9 — chốt VSH2).

APPROVED 2026-11-14 Agent 0

> **ST-55 DONE — VÒNG 9 (VSH2) CHỐT.** `reports/R9-viscript.{md,json,license}` (`af00658`, head 8c8c36a) —
> `report-check` **PASS (9 báo cáo, 61 hàng QUEUE phủ đủ)**. Số đo **chạy lại**: **730 test** (357 cli · 62 bytecode) ·
> **370 ca sem/18 nhóm** · `vis test` **370 ok** / `--vm` **370 ok + 6 SKIP** · **parity 344/344** · **differential 100.000
> chương trình 0 lệch** (213,2 s) · fuzz parser 60 s **2.996.353 ca/0 vi phạm** · `VBC1` 53 tag · **7 sổ/29 mã** ·
> **`CODEGEN_TIN_TYCK = 0`** · khởi động ấm 11,1–11,3 ms · **allowlist 6** · `gate --fast` + `nightly` PASS.
> **Đóng 3 nợ lõi**: `to json` (ST-53), hợp nhất comparator hai `sort_by` (ST-53), **codegen từ chối HIR có chẩn đoán**
> + `CODEGEN_TIN_TYCK` **21 → 0** (ST-54). **Còn 10 nợ có chủ đích** + **VS2** (bảng 8 hạng mục có **tiêu chí đo** và
> `[ƯL]` trong `R9` §4). **ViScript chưa hoàn thiện toàn bộ.**
> **Vòng 10 (mở sau)**: ST-56 (nợ lõi còn lại) · ST-57 (miếng VS2 đầu tiên: async + kênh) · ST-58 (R10).

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

---

## Vòng 6 — VSH1a (từ issue #21, "bộ lệnh shell có cấu trúc")

APPROVED 2026-11-13 Agent 0

> **Mở vòng 6 — VSH1a.** Issue #21 (`khoa-pham-aepsy`, <https://github.com/vifast-oss/ViFastOs/issues/21>) — tác giả
> **đã đóng #21** và đăng lại y nguyên ở **#22** (`khoajunior`, cùng người, tài khoản cá nhân); **luồng sống là #22**
> (<https://github.com/vifast-oss/ViFastOs/issues/22>) — xin
> `std.shell` (`where/sort_by/ls/ps`) theo §9/§13. **Nhận hướng, không nhận cách chia P1..P4**: chốt lại thành
> **ST-37..ST-43** (8 hàng, ST-40 tách a/b) ≈ 21,5 h ≈ 3 vòng, **sau ST-36 (R5)**. Căn cứ lộ trình §19:
> `VSH1 ← VS1`, `VS2 ← VS1` ⇒ VSH1 là anh em của VS2; issue xin đúng **nửa đầu VSH1**. Gói quyết định + bằng
> chứng đo (3 lệnh `vis check/run` chạy thật ở HEAD `0e65446`):
> [`review/agent0/GOI-ISSUE-21-VSH1a.md`](../../review/agent0/GOI-ISSUE-21-VSH1a.md).
>
> **Quyết định kèm theo: VD-68..VD-75.** Ba điều kiện bắt buộc của vòng:
> (1) **typed** — verb có chữ ký, **không** đi đường `Ty::Any` của `std.list.*` (nợ VD-59 (3));
> (2) **ranh giới tách repo** — mọi thứ thuộc ngôn ngữ nằm trong `viscript/` (`Type`, prelude bản ghi, `Render`,
> `Host`, `HOST_API`); `ps`/`kill` chạy thật là `vis-host-vifast` (ngoài `viscript/`) + `procsrv`, sau VS2;
> (3) **không nới ngưỡng**; allowlist `doc-examples` chỉ được giảm **8 → 7** khi ST-42 **thật xanh**.
>
> **Không làm ở vòng 6** (ghi để khỏi trôi): `stream[T]` lười/SIGPIPE/huỷ pipeline (VSH1b); `walk`/`save`/
> `to json`/`from json[T]`/`http` (VSH1b, mục 1451 của allowlist); `std.shell` **viết bằng ViScript** (seam
> `SigSource` giữ đường, làm sau khi tách repo); cưỡng chế `cap[Dir]`/`cap[Proc]` (VS2); `ps`/`kill` chạy thật.
>
> **Thứ tự**: ST-37 chỉ mở khi **ST-36 DONE**; ST-37..ST-42 đụng `lower/cmd.rs` + `tyck` nên **không** chạy
> song song với nhau. ST-42 là task chạm `docs/viscript/doc-examples-allow.txt` ⇒ **Agent 0** sửa tệp đó, Agent 2
> chỉ báo kết quả cổng.

APPROVED 2026-11-14 Agent 0

> **Mở vòng 10 — nợ lõi + miếng VS2 đầu.** Sáu task **ST-56..ST-61** + **ST-62 (R10)**, làm **tuần tự**; mỗi task
> red-first → xanh → `[ST-##]` commit riêng → push. **Quyết định kèm theo: VD-89..VD-94.**
> **Điều kiện bắt buộc**: (1) **không** nới ngưỡng, **không** tăng allowlist; (2) mọi nợ đóng phải có **≥2 ca**
> (một ca xanh, một ca âm) và **interp ↔ `--vm` cùng byte** khi tính năng chạy ở hai đường; (3) **ST-56 đóng nốt
> `Hop[Hop[int]]`** — nếu probe cho thấy đã chạy, ghi rõ "đã xanh từ trước" + khoá bằng test, **không** giả vờ sửa;
> (4) **ST-60/ST-61**: Agent 2 **báo cáo thiết kế trước khi code** (bộ lập lịch, biểu diễn `Task`/`Chan`, chỗ sửa trong
> `vis-interp` và `vis-vm`, cách giữ cùng byte) — Agent 0 duyệt rồi mới code; (5) async/kênh **không** được hứa hơn
> thực tế: mô hình "chạy tới hoàn tất" là **nợ có chủ đích**, ghi vào `R10` §5.
>
> **Không làm ở vòng 10**: song song thật/đa luồng; đình chỉ giữa thân tác vụ; net/http; Landlock; LSP; gói/registry;
> host windows/wasm; mượn thật `&T`. Các mục này ở vòng 11+.
>
> **Thứ tự**: ST-56 → ST-57 → ST-58 → ST-59 → ST-60 → ST-61 → ST-62. ST-58 chạm `stdlib.rs` + `tyck` nên ST-59 chỉ mở
> sau khi ST-58 xanh. ST-60/ST-61 chạm `vis-types`/`vis-interp`/`vis-bytecode`/`vis-vm` ⇒ **cấm** chạy song song với nhau
> hoặc với task khác đang sửa cùng crate.

APPROVED 2026-11-14 Agent 0

> **ST-56 DONE** — commit `767b195` (**Agent 2 tự commit, đã push**): khoá `str` cho `std.list.sort_by`
> (`KieuKhoa::ChiIntHoacStr`, sort ổn định, trộn kiểu ⇒ `VS0501`), `ref(x)` = **identity** ở cả hai đường (hết
> `Unsupported("ref")`), `&self` parse + hạ như `self`, `Hop[Hop[int]]` **đã xanh từ trước** (khoá bằng 5 biến thể).
> Đo: **747 test** · sem **376/18** · `vis test` **376 ok** · parity **348/348** · differential **100k/0 lệch** (202,7 s)
> · allowlist **6**. Kèm **arity guard** cho `builtin::call` (bytecode đột biến gọi `render` rỗng từng panic) — sửa
> đúng, không đổi chương trình hợp lệ.
> **Phát hiện phải đóng ngay (ST-56b)**: `self.x = v` ⇒ interp `VS0501` ↔ VM **chạy** (in 2); `var xs` gán phần tử ⇒
> interp `VS0501` ↔ VM **chạy**; `--vm` lỗi codegen ⇒ **exit 0**. Tyck **đã** chặn `let`/tham số (`VS0412`) — lỗ nằm ở
> `self` và ở interp. **VD-97**.

APPROVED 2026-11-14 Agent 0

> **ST-56b DONE** — commit `dd04afb` (**Agent 2 tự commit, đã push**). Dữ kiện quyết định: **thân `impl` chưa hề được tyck**
> (`HItem::Impl(_) => {}`) nên `self.x = v` không có chẩn đoán nào; đã thêm kiểm **đích gán** trong thân `impl` (`self`
> không `var` ⇒ `VS0412`; `var self` được; `+=` trên trường/phần tử ⇒ `VS0412`). **Codegen sai thật**: `x[i] = v` phát
> `Set` + **`Pop`** ⇒ VM **im lặng bỏ kết quả** (in `[1, 2]` thay vì `[5, 2]`) — không phải "interp thiếu" như tôi đoán
> ban đầu; nay `Set` + `Assign(slot)`. Interp hạ **đúng** `With`/`Set`, cùng thứ tự đánh giá ⇒ cùng byte. `run --vm` lỗi
> codegen nay **exit 1** (+ `not ok` ở `--tap`). Đo: **756 test** · sem **382/18** · `vis test` **382 ok** · parity
> **352/352** · differential **100k/0 lệch** (194,7 s) · allowlist **6**. Nợ ghi `R10` §5: **tyck toàn bộ thân phương
> thức** (mới chỉ kiểm đích gán), `p.x += 1` là lỗi cú pháp (giữ), base `{error}` chỉ chẩn đoán lúc chạy.

APPROVED 2026-11-14 Agent 0

> **ST-57 DONE** — commit `57ea7212` (**Agent 2 tự commit, đã push**). Ba nợ đóng: **mồ côi đa tệp** (`TraitSig` +
> `LowerOptions.trait_ngoai`, `modules.rs` xuất `pub_traits`; mồ côi thật = **cả** trait lẫn kiểu ngoài chương trình ⇒
> `VS0403`), **siêu trait** (`trait B: A:` kiểm **một tầng** sau 1b, thứ tự `impl` tự do, `VS0403`), **`derives` sinh
> mã** ở lượt **1c** (`show` = `HExpr::Interp`, `eq` = `==`; tên nội bộ có tiền tố; người dùng thắng; generic/`float`/
> `Fn` ⇒ `VS0403` tĩnh). Đo: **765 test** · sem **388/19 nhóm** · `vis test` **388 ok** · parity **356/356** ·
> differential **100k/0 lệch** (198,5 s) · allowlist **6**. Tách `lower/options.rs` để giữ R1 (`lower.rs` 1467).
> **Ba chỗ lệch gợi ý, đã xác minh**: (1) `print(p)` in **dạng ẩn danh** `{x: 1, ten: a}` (ghi chú của tôi "P(x: 1, …)"
> là **sai**) — `show` sinh khớp `Value::show`; (2) cú pháp siêu trait là `trait B: A:` (không phải `trait B(A):`);
> (3) `hir.rs` doc-block `(4,0,4) → (5,0,9)` có lý do.
> Nợ ghi `R10` §5: `enum show`, `derives Json` sinh mã, siêu trait **bắc cầu**, dispatch `impl` **xuyên tệp**, tyck kiểu
> trả phương thức `impl`, `trait_ngoai` chỉ thấy module `use` trực tiếp.
> **ST-58 mở** (`Ty::Any` → biến kiểu; VD-99).

APPROVED 2026-11-14 Agent 0

> **ST-58 DONE** — commit `abdc253` (**Agent 2 tự commit, đã push**): **`Ty::Any` xoá hẳn**, thay bằng `Var(u8)` +
> `List(&'static Ty)` + `Map(…)` (const-friendly, promotion trong `const FUNCS`); `sang_type()` **tổng** (`Var` →
> `TypeRef T/K/V`); `kiem_std` **hợp nhất** bằng `hop_nhat`+`thay_gen` (biến lặp phải `same`, `{error}` không gán, sai
> hình dạng ⇒ `VS0412`, số đối số ⇒ `VS0401`). **16/38** chữ ký đổi (list 8, map 5, json 3); ngoại lệ **có ghi duy nhất**
> `list.sort_by[1] = Ty::FnKhoa`. Đo: **770 test** · sem **388/19** (không đổi) · `test/std` **73** · `vis test` **388 ok**
> · parity **356/356** · differential **100k/0 lệch** (193,2 s) · **7 sổ/29 mã**. Ba ca cũ dựa `Any` đã lật thành lỗi
> **tĩnh** (mạnh hơn), ghi "SỬA KÈM LÝ DO"; ca **TRON** của `sort_by` vẫn runtime `VS0501` nhờ `json.decode` không kiểu.
> Nợ: `Ty::Fn` thật cho `sort_by`; map khoá không-`str`; thứ tự khoá map.
> **ST-59 mở** (nợ shell — biến làm stage đầu, `let` không chú thích, thân `cmd` vừa script vừa lệnh).

APPROVED 2026-11-14 Agent 0

> **ST-59 DONE** — commit `3e2a3cd` (**Agent 2 tự commit, đã push**): **một nguồn luật `let`** (`vis_syntax::let_la_khai`,
> trước có **hai bản sao lệch** ở parser và tầng hạ) ⇒ `let x = 1` / `let xs = [1,2,3]` **bind** ở vùng item; **biến làm
> stage đầu** (`Var/Param/PatBind` đã bind ⇒ stage biểu thức + **kiểu tĩnh** cho `where/get/select`; chưa bind ⇒ giữ đường
> lệnh); **thân `cmd` khai báo + một pipeline** (`KieuThan::KhaiLenh`, một nguồn ở `vis-syntax`, hai Lowerer con lồng khung);
> khai báo sau pipeline / nhiều pipeline ⇒ **`VS0201`**. Đo: **771 test** · sem **394/19** (49 `.vish`, 62 `.err`) ·
> `vis test` **394 ok** · `--vm` **394 ok + 7 SKIP** · parity **360/360** · differential **100k/0 lệch** (202,8 s) ·
> **7 sổ/29 mã**. Hai phát hiện: **luật `let` hai bản sao** (nếu sửa một chỗ thì `check` ↔ `run` lệch) và **parity suýt
> lệch** ở ca nhiều pipeline (đã cho body `{error}`). Nợ: shell-let thật (`let out = ls | wc -l`), nhóm nhiều pipeline +
> kiểu tĩnh lấy stage đầu, lambda `where` không chú thích, `lower/cmd.rs` **1491** dòng (sát R1).
> **ST-60 mở** (VS2 #1 — async; **VD-90/VD-95**).

APPROVED 2026-11-14 Agent 0

> **ST-60 DONE — VS2 #1 (async) CÓ MẶT.** Commit `2542421` (**Agent 2 tự commit, đã push**). **Không thêm tag `VBC1`**
> (vẫn 53): hạ `async fn`/`spawn`/`await` thành builtin nội bộ `$tac_vu`/`$spawn`/`$await` + **một** runtime
> `vis-interp/src/rt.rs` dùng chung hai đường; `Task[T]` = `TypeRef`; lập lịch **FIFO, một luồng, chạy tới hoàn tất**;
> join cuối theo **thứ tự tạo**; lỗi thân ⇒ **`VS0506`** (mã mới, sổ **30**). Tách `host.rs`/`gan.rs` để giữ R1
> (`interp.rs` 1452). Đo: **774 test** · sem **416/20 nhóm** · `vis test` **416 ok** · parity **377/377** · differential
> **100k/0 lệch** (208,7 s) · **10.000 task: interp 309 ms, VM 166 ms, cùng byte, 10.001 dòng stdout, không tràn**.
> **Lệch có ghi**: parser chưa nhận `async` sau `impl`/`cmd` ⇒ hiện ra **`VS0201`** (luật `VS0412` trong tyck giữ làm
> phòng thủ); `async fn` không nhận `--tên`; **chưa** song song thật/không GC (nợ `R10` §5).
> **ST-60b mở** (hotfix review độc lập — xem dưới) và **ST-60c** (derives/siêu trait + SPDX).

APPROVED 2026-11-14 Agent 0

> **ST-60b DONE** — commit `f158085` (**Agent 2 tự commit, đã push**). **Hồi quy parity thật đã đóng**: closure bắt theo
> **giá trị** nay **cấp ô cục bộ riêng** + prologue `LoadCaptured`→`Assign` ⇒ đọc/ghi trong closure không đụng biến ngoài,
> hai đường **cùng byte** (C1 `[1, 2]`, C2 `()\n0`). Phát hiện **cùng họ**: `()` từng bị mã hoá `Const("")` ⇒ VM in chuỗi
> rỗng thay `()` — nay **`PushUnit`** thật. Tyck **lần xuống gốc** cho `self.xs[0]`/`p.xs[0]`; thông điệp **đích lồng** nêu
> đúng gốc; `.vish` shell-let `let out = ls -l` phân loại lại đúng; `std.*` **từ chối** `--tên` (`VS0401`). Đo: **782 test**
> · sem **419/20** · parity **378/378** · differential **100k/0 lệch** (223,6 s) · **VBC1 53** · **7 sổ/30 mã**.
> Tách `tyck/sig.rs` + `bytecode/cerr.rs` để giữ R1. Nợ: shell-let **chưa hạ**; `match` không khớp ở ngữ cảnh giá trị còn
> đẩy `false` (nghi lệch interp↔VM); gán lồng sâu hơn một tầng trường là `VS0412` có chủ đích.
> **ST-60c mở** (derives/siêu trait + số liệu).

APPROVED 2026-11-14 Agent 0

> **ST-60c DONE** — commit `b254f03` (**Agent 2 tự commit, đã push**). **B3**: `sinh_derives` chạy **trước** kiểm siêu trait và
> ghi `(kiểu, Show/Eq)` vào `impl_pairs` — **chỉ khi thật sự sinh được** ⇒ `impl Hien for P` hết `VS0403` oan, thiếu thật vẫn
> đỏ. **B4**: chọn **sinh mã `Show` cho enum** (`InterpPart::Expr(self)` ⇒ `Value::show`) thay vì lỗi tĩnh — rẻ hơn và giữ
> khối tài liệu 214 xanh; lý do ghi STATE/JOURNAL. **B5**: `Eq` chặn **sâu** đệ quy `List/Map/Opt/Result/TypeRef` ⇒
> `list[float]`/`map[str,float]`/`Opt[list[float]]` là `VS0403`. Đo: **782 test** · sem **426/20** · `vis test` **426 ok** ·
> parity **381/381** · differential **100k/0 lệch** (248,0 s) · **7 sổ/30 mã**. Nợ: siêu trait **bắc cầu**, `impl_pairs`
> per-tệp, `co_float_sau` chưa soi bản ghi lồng, `derives Json` chưa sinh mã.
> **ST-61 mở** (VS2 #2 — kênh; **VD-91/VD-96**).

APPROVED 2026-11-14 Agent 0

> **ST-61 DONE — VS2 #2 (kênh) CÓ MẶT.** Commit `782f1b9` (**Agent 2 tự commit, đã push**; agent2c code, agent2d kiểm chứng + đóng).
> `chan[T](n)` + `send`/`recv`/`close`; **một** runtime `rt.rs` cho hai đường (`$kenh` builtin, không thêm tag `VBC1` — vẫn 53);
> chặn ⇒ **drain** hàng đợi FIFO rồi thử lại; hết queue ⇒ **`VS0505` deadlock** (đo **0,085–0,104 s**, không treo); gửi sau
> `close` ⇒ **`VS0504`**; `close` **idempotent**; `recv` sau close trả nốt hàng rồi `none`; `send` sai kiểu ⇒ **`VS0412` tĩnh**.
> Đo: **787 test** · sem **441/21 nhóm** · `vis test` **441 ok** · `--vm` **441 ok + 7 SKIP** · parity **396/396** ·
> differential **100k/0 lệch** (213,4 s, seed 3523158054) · **7 sổ/32 mã** (`all_codes()` 29→32 qua ST-60/61) · gate PASS.
> Log đỏ trung thực: `st61-red-sem.log` (14/15 đỏ đúng lý do; 1 ca **đã xanh sẵn** — `chan` là soft keyword) + `st61-red-rust.log`
> (0/5). Tách `tyck/expr/{kenh,phu}.rs` để giữ R1 (`expr.rs` 1.464).
> **Nợ (R10 §5)**: drain chỉ chạy task `ready` (`spawn`) — task `Moi` do gọi `async fn` chưa được chạy ⇒ `recv` có thể `VS0505`
> dù còn Task; `close` không đánh thức ai; **không** `select`/timeout; `Chan` không qua biên host; `cap=0` không dùng được;
> dung lượng âm là lỗi **lúc chạy**; `Chan` in `<kênh>`.
> **ST-62 mở** (R10 — chốt vòng 10).

APPROVED 2026-11-14 Agent 0

> **ST-62 DONE — VÒNG 10 CHỐT.** `reports/R10-viscript.{md,json,license}` (`2b28f10`, head `6192bbd`) — `report-check`
> **PASS (10 báo cáo, 85 hàng QUEUE phủ đủ)**. Số liệu **chạy lại**: **787 test** · sem **441/21 nhóm** · `vis test` **441 ok**
> · `--vm` **441 ok + 7 SKIP** · parity **395/395** · differential **100k/0 lệch** (208,5 s) · fuzz parser 60 s
> **2.661.866 ca / 0 vi phạm** · **7 sổ/32 mã** · `VBC1` **53** · allowlist **6** · 10k task async **319/168 ms**.
> Vòng 10 đóng **5 nợ lõi** (khoá `str` sort_by, `&self`/`ref`, `Hop[Hop]`, mồ côi đa tệp, siêu trait, derives sinh mã,
> `Ty::Any`) **và mở VS2**: **#1 async** + **#2 kênh** chạy thật hai đường. **Còn**: net/http, Landlock, LSP, gói/registry,
> windows/wasm, hợp chuẩn 4 nền — đã có hàng **ST-63..ST-76** (vòng 11).
> **ST-63 mở** (VS2 #3a — `NetHost` + TCP client; **VD-105**: **R1 trước, code sau**).

APPROVED 2026-11-14 Agent 0

> **ST-63 DONE — VS2 #3a (NetHost + TCP client) CÓ MẶT.** Commit `29f236aa` (**Agent 2 tự commit, đã push**). Bước 0 đúng
> luật: `interp.rs` **1.486 → 1.217** dòng (tách `interp/goi.rs` 287 dòng, **nguyên văn**, chỉ đổi `fn`→`pub(crate) fn`),
> gate xanh **trước** khi thêm net. `NetHost` (7 method, handle `str` mờ, `read → Vec<u8>`), `Host::net()` default `None`
> ⇒ `VS0604`; `MODULES` 7→8, `FUNCS` 38→43 (**+5** `net.*`), **không** variant `Ty` mới, **không** sửa codegen. CLI
> `net.rs` TCP thật, **chỉ loopback**, coi **cả** `WouldBlock` (macOS 35) và `TimedOut` là hết giờ; nối **đủ 4** điểm
> truyền host. **6 ca** Rust loopback thật, so `run` ↔ `--vm --no-cache` (+1 ca cache hit). Đo: **793 test** · sem
> **441/21** giữ nguyên · parity **395/395** · differential **100k/0 lệch** (215,3 s) · **7 sổ/32 mã** · `VBC1` 53.
> **Lệch có ghi**: `net.read` **3 đối số** `(tay, tối_đa, timeout_ms)`; `std.net.connect` dạng **đầy đủ** chưa hạ (dùng
> dạng ngắn sau `use`, như `fs.*`/`json.*`); bỏ `is_open`; `read` trả `str` **lossy**; mặc định connect 1 s.
> **ST-64 mở** (VS2 #3b — `http.get`).
