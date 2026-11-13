<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# R7-viscript — báo cáo vòng 7 (chốt VSH1b), dòng ViScript

Agent 2 · 2026-11-13 · `head` = **`426eb91`** (head lúc thu số liệu; QUEUE@head có đủ **53 hàng**, 6 cột;
ST-44..ST-46 `DONE`, ST-47 `READY` báo cáo này). Số đo chạy trên **mã của `2207ea4`** (commit `426eb91`
chỉ chạm `QUEUE`/`DECISIONS`/`STATE` của Agent 0). Bản máy đọc: [`R7-viscript.json`](R7-viscript.json) —
**đúng 11 khoá** Khối E (`project = "viscript"`, `round = 7`).

## 1. Tóm tắt (5 dòng)

1. **VSH1b chạy được đầu–cuối**: `.vish` có **khai báo kiểu** (`type`/`enum`/`cmd`/`let` ở đầu tệp, hạ bằng
   **đường script** span tuyệt đối, `cmd` thành `CmdSig` cho khe ống), **`where` kiểm tĩnh đầy đủ**
   (`VS0412` nêu kiểu thật), **`each` không kiểm kiểu trả** (giống `for`), **`ls --long` thật**
   (`FileEntryLong`: `mode`, `modified_time` UTC), **`( … )` làm đối số** = biểu thức, **chẩn đoán tuyệt đối**
   trong ngoặc.
2. **Hai đường trùng ngữ nghĩa**: differential **100.000 chương trình / 0 lệch**; parity **344/344**
   (26 ca lỗi biên dịch so qua `compile()`); `VBC1` **53 tag** **không đổi**.
3. **651 test** workspace (21 diag + 96 syntax + 18 fmt + 143 types + 24 interp + 50 bytecode + 9 vm +
   **290 cli**); **370 ca ngữ nghĩa / 18 nhóm** (**327** `.vis` + **43** `.vish`, 52 `.err`).
4. **`vis test`**: TAP `1..370` — **370 ok**; `vis test --vm` = **370 ok / 6 SKIP / 0 đỏ** (SKIP = ca cần
   host chương trình ngoài, VM không có host).
5. **`./vis-dev gate --fast` PASS** + **`./vis-dev nightly --fuzz-minutes 1` PASS** (fuzz parser
   **3.003.112 ca / 68.229.914 token / 0 vi phạm**, 60,0 s, 50.052 ca/s); khởi động ấm **11,1–11,3 ms**
   (debug) ⇒ **đạt** §19 `< 20 ms [ƯL]`; **allowlist doc-examples 6**.

## 2. Bảng task vòng 7 (ST-44 → ST-47)

| ID | Trạng thái | Commit | Cổng | Kết quả đo được |
|---|---|---|---|---|
| ST-44 | DONE | `32c5af5` | fast | **`.vish` khai báo kiểu**: vùng **item ở đầu tệp** (`type`/`enum`/`cmd`/`let` có kiểu) tách khỏi **stage**; tiền tố parse lại `Mode::Script` (**span tuyệt đối**), hạ vào **cùng** bảng ký hiệu/kiểu ⇒ `cmd f(...)` thành **`CmdSig`** cho khe ống; trùng tên (kể cả prelude) ⇒ **`VS0302`**; khai báo **sau** stage ⇒ **`VS0201`**; `f()` = lời gọi **0 đối số**; **622 test** |
| ST-45 | DONE | `ea42dd1` | fast | **`where` kiểm tĩnh đầy đủ** (thân phải `bool` ⇒ **`VS0412`** nêu **kiểu thật**: `size`/`int`/`FileEntry`); **`each` không kiểm kiểu trả** (khai `{error}`) nhưng **không nuốt** lỗi trường (`VS0403`/`VS0605`); kiểu không biết ⇒ **kiểm động**; tách `lower/sugar.rs` (R1); **636 test** |
| ST-46 | DONE | `2207ea4` | fast | **`ls --long` thật**: `MucFs.mode`, **`FileEntryLong`** (6 trường), `mode` = `rw-r--r--`, `modified_time` = **UTC `YYYY-MM-DD HH:MM`**; bảng mặc định **4 cột không đổi**; **nhóm-đối-số = biểu thức** (bỏ `VS0603`; sai kiểu ⇒ `VS0412`); **chẩn đoán tuyệt đối** trong `( … )` (đệm dấu cách, không `Indent` giả); **651 test**, 4 fixture `test/cmd/long/**` |
| ST-47 | READY → **báo cáo này** | — | fast | tệp này + `R7-viscript.json` (QUEUE@head ghi `READY`) |

**53 hàng QUEUE** khớp `queue_at(426eb91)`; **0 hàng `BLOCKED`/`REVIEW`** ⇒ `blocked[]` rỗng.

## 3. Số đo (mọi số tái lập bằng lệnh ghi kèm; chạy 2026-11-13, giờ UTC ghi trong ngoặc)

| Số | Giá trị | Lệnh |
|---|---|---|
| test workspace | **651** = 21 diag + 96 syntax + 18 fmt + 143 types + 24 interp + 50 bytecode + 9 vm + **290 cli** | `cargo test --workspace`; theo crate `cargo test -p <crate>` (13:0xZ) |
| ca ngữ nghĩa | **370 ca / 18 nhóm** | `find test/sem -name '*.vis' -o -name '*.vish' \| wc -l` |
| theo đuôi | **327 `.vis` + 43 `.vish`**; **52 `.err`** | `find test/sem -name '*.vis' \| wc -l` v.v. |
| `vis test` (interp) | **370 ok / 0 `not ok`**, exit 0; `1..370` | `vis test` |
| `vis test --vm` | **370 ok** (**6 SKIP** = ca cần host chương trình ngoài) **/ 0 đỏ** | `vis test --vm` |
| parity chẩn đoán | **344/344 khớp, 0 lệch** (so **đủ** `(mã, dòng, cột, thông điệp)`; **26** ca lỗi biên dịch so qua `compile()`) | `cargo test -p vis-vm --test vs_interp` |
| differential | `DIFF tong=100000 loi_tinh=0 chay=100000 lech=0 giay=212.2 toc_do=471/s seed=3523158054` | `./bin/vis-diff` (13:0xZ) |
| fuzz parser | **3.003.112 ca / 68.229.914 token / 50.052 ca/s / 0 vi phạm** (60,0 s) | `./vis-dev nightly --fuzz-minutes 1` (13:10Z) |
| fuzz đột biến verifier | `cargo test -p vis-vm --test fuzz_mutate` — **318 module**, 1,4 s | lệnh ghi kèm |
| `diag_cmp` | **318/370** khớp, 0 lệch | `cargo test -p vis-cli --test diag_cmp` |
| VBC1 | **53 tag** (`Instr`, version 1) — **không đổi** suốt vòng 5, 6, 7 | `crates/vis-bytecode/src/format.rs` |
| sổ mã | **7 sổ**, `all_codes()` **29**; `next_block()` **801** | `crates/vis-diag/src/lib.rs`, `tests/messages.rs` |
| khởi động ấm | **11,15 ms** (interp) · **11,30 ms** (`--vm` cache) · **11,12 ms** (`--vm --no-cache`) — trung vị 15 mẫu, debug | `vis run [--vm]` × 15 |
| allowlist doc-examples | **6** (không đổi từ vòng 6) | `test/doc-examples/doc_examples.py` (`ALLOW_COUNT = 6`) |
| cổng | **PASS** | `./vis-dev gate --fast` (13:10Z, và chạy lại sau khi ghi báo cáo) |
| nightly | **PASS** | `./vis-dev nightly --fuzz-minutes 1` (13:10:35Z) |

## 4. §19 — VSH1b đạt tới đâu

| Hạng mục VSH1b | Trạng thái | Ghi chú |
|---|---|---|
| `.vish` có **khai báo kiểu** (`type`/`enum`/`cmd`/`let` có kiểu) | **đạt** | tách vùng item/stage; hạ bằng đường script **span tuyệt đối**; trùng prelude ⇒ `VS0302` |
| `cmd` trong `.vish` thành **chữ ký** cho khe ống/tyck | **đạt** | `f() | where size > 5mb` kiểm tĩnh; **thân `cmd` pipeline lệnh** ⇒ **VSH1b-2** |
| `where` **kiểm tĩnh đầy đủ** | **đạt** | thân phải `bool` ⇒ `VS0412` nêu kiểu thật; `it` không biết kiểu ⇒ kiểm động |
| `each` không kiểm kiểu trả | **đạt** | giống `for` trong Rust; lỗi trường/cú pháp vẫn báo |
| `ls --long` **thật** | **đạt** | `FileEntryLong`: `mode`, `modified_time` UTC; mặc định 4 cột |
| `( … )` làm **đối số** | **đạt** | bỏ `VS0603`; nhóm rỗng = gọi 0 đối số |
| Chẩn đoán **tuyệt đối** trong `( … )` | **đạt** | đệm dấu cách; `( 1 / 0 )` dòng 3 ⇒ `3:3` |
| Hiệu năng khởi động < 20 ms | **đạt** | 11,1–11,3 ms (debug) |

**ViScript CHƯA hoàn thiện toàn bộ** — VSH1b mới là **lớp shell có kiểu** trên lõi VS1; đường còn lại:

- **VSH1b-2**: thân `cmd` bằng **pipeline lệnh**; nhóm-**lệnh có cờ** (`( ls … --long )`); nhóm **nhiều dòng**;
  `where ( x => x )` kiểm động; **thống nhất hai mã field-access** (`VS0403` vs `VS0605`); `[1,2] | take 1`
  **không ngoặc** ⇒ `VS0201`; `select` bản ghi ẩn danh chưa `to json`; **hai `sort_by`**.
- **Nợ lõi**: `CODEGEN_TIN_TYCK = 21`; **`Ty::Any`**; **`&self`**; **mồ côi đa tệp**; **siêu trait**;
  **derives sinh mã**; **`Hop[Hop[int]]`** (generic lồng).
- **VS2**: **async/kênh/net** thật; **Landlock thật** (`consent`/`cap[Proc]`/`cap[Dir]`); **LSP**; **gói +
  registry** (`pkg.*`/registry); **4 nền tảng** (Linux/macOS/Windows/…); giao thức mạng, dịch vụ nền.

## 5. Lỗ hổng đã biết — bản cập nhật

**Đã đóng trong vòng 7**: `.vish` khai báo kiểu (ST-44) · `where` kiểm tĩnh đầy đủ + `each` không kiểm (ST-45) ·
`ls --long` thật · `( … )` làm đối số · chẩn đoán tuyệt đối trong ngoặc (ST-46).

**Còn mở**:

1. **Thân `cmd` bằng pipeline lệnh** (VSH1b-2) — thân hiện là **script**.
2. **Nhóm-lệnh có cờ** (`( ls … --long )`) chưa hạ; **nhóm nhiều dòng** ⇒ `VS0201`.
3. `where ( x => x )` kiểm **động** (tyck chưa kiểm call `where`).
4. **Hai mã field-access**: closure tường minh ⇒ `VS0403`; viết tắt `it` ⇒ `VS0605` — chưa thống nhất.
5. `[1,2] | take 1` **không ngoặc** ⇒ `VS0201` (stage đầu là biểu thức chưa được văn phạm nhận).
6. `select` trả **bản ghi ẩn danh** ⇒ chưa in/ghi như `map` khi `to json`/`save json`.
7. **Hai** `sort_by` (shell nhiều kiểu + `--desc`; `std.list.sort_by` khoá `fn(int) -> int`).
8. `CODEGEN_TIN_TYCK = 21` (tin tyck ở 21 chỗ sinh mã).
9. **`Ty::Any`** chưa có ⇒ `map[str, Any]` chưa biểu diễn được.
10. **`&self`** chưa có.
11. **Mồ côi đa tệp**: chưa kiểm "mọi tệp đều được `use`/chạm".
12. **Siêu trait** chưa có; **derives chưa sinh mã**; **`Hop[Hop[int]]`** chưa hỗ trợ.
13. **consent / `cap[Proc]` / `cap[Dir]`** (cưỡng chế quyền) ⇒ **VS2**.
14. **`pkg.*` / registry** (gói) ⇒ **VS2**.

## 6. VD-77 … VD-80 (mỗi cái một dòng)

| VD | Một dòng |
|---|---|
| VD-77 | Phạm vi VSH1b: `.vish` khai báo kiểu, `where` kiểm tĩnh, `ls --long`, nhóm làm đối số, chẩn đoán tuyệt đối — kèm danh sách nợ chuyển VSH1b-2/VS2. |
| VD-78 | **Khai báo trong `.vish`**: vùng item đầu tệp (dòng cột 1 + từ khoá, `let` có kiểu ⇒ chỉ khi RHS không `|`/`;`/`>`/`<`/`&`), tiền tố parse lại Script span tuyệt đối, trùng tên ⇒ `VS0302`, khai sau stage ⇒ `VS0201`, `f()` = gọi 0 đối số. |
| VD-79 | **`where`/`each`**: `where` kiểm ở **tyck** (`fn(T)->bool` ⇒ `VS0412` nêu kiểu thật); `each` khai `{error}` ⇒ **không** kiểm kiểu trả, vẫn báo lỗi khác; `it` không rõ kiểu ⇒ **kiểm động**; R1 ⇒ tách `lower/sugar.rs`. |
| VD-80 | **`ls --long` + nhóm**: `FileEntryLong` (`mode`, `modified_time` UTC, **không** thêm crate), `MucFs.mode: u32`; nhóm-đối-số = biểu thức (bỏ `VS0603`, sai kiểu ⇒ `VS0412`); đệm **dấu cách** cho span tuyệt đối. |

## 7. Đề xuất cho Agent 0

1. **AGENTS §8**: cả 4 task vòng 7 đã `DONE` ⇒ đóng **R7/VSH1b**, mở **vòng 8 (VSH1b-2 + VS2 mở màn)** với
   thứ tự ưu tiên: (i) thân `cmd` **pipeline lệnh**; (ii) nhóm-lệnh có cờ + nhóm nhiều dòng; (iii) `where`
   kiểm tĩnh cả **call** (hết `where (x=>x)` động); (iv) thống nhất **hai mã field-access**; (v) `[1,2] | take 1`
   không ngoặc.
2. **`viscript/AGENTS.md`** (Agent 0 giữ): cập nhật dòng trạng thái "VÒNG 7 (VSH1b) DONE — 651 test, 370 ca,
   parity 344/344, diff 100k 0 lệch, allowlist 6"; thêm nợ **VSH1b-2** và **VS2** như §4.
3. **QUEUE vòng 8**: tách **ST-48..ST-53** cho VSH1b-2 + nợ lõi (mỗi task ≤ 3 h); các mục VS2 để **BLOCKED**
   có lý do "chưa mở vòng VS2", **không** để chung một khối.
4. **Hàng QUEUE**: hiện **53 hàng × 6 cột** khớp `queue_at(426eb91)` — **không** thiếu cột; chỉ cần lật
   **ST-47 `READY → DONE`** sau commit báo cáo này; **0** hàng `BLOCKED`/`REVIEW` nên không có hàng nào cần mở lại.
