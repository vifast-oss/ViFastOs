<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# R6-viscript — báo cáo vòng 6 (chốt VSH1a), dòng ViScript

Agent 2 · 2026-11-13 · `head` = **`654accb`** (head lúc thu số liệu; QUEUE@head có đủ **49 hàng**, 6 cột;
ST-37..ST-42 `DONE`, ST-43 `READY` báo cáo này). Số đo chạy trên **mã của `331dc58`** (commit `654accb` chỉ
chạm `QUEUE`/`DECISIONS`/`STATE` của Agent 0). Bản máy đọc: [`R6-viscript.json`](R6-viscript.json) — **đúng 11
khoá** Khối E (`project = "viscript"`, `round = 6`).

## 1. Tóm tắt (5 dòng)

1. **VSH1a chạy được đầu–cuối**: khe ống **hợp nhất** (generic `CmdSig` + `SigSource`), **closure-arg**
   thành `Lambda` với **span tuyệt đối**, `where`/`each`/`first`/`count`, sugar **`it`** + viết tắt trường,
   **prelude** `FileEntry`/`ProcEntry`, `get`/`select`, **`Render`** (bảng/khối), **`ls`/`open`** + `--fs-mock`.
2. **Khối 672 của §9.7 chạy THẬT**: `ls ./photos | where size > 5mb | sort_by modified --desc | take 10`
   ⇒ **bảng**, **interp ↔ `--vm` cùng byte** (dùng `--fs-mock`); đây là khối tài liệu **chạy thật** đầu tiên.
3. **Hai đường trùng ngữ nghĩa**: differential **100.000 chương trình / 0 lệch**; parity **344/344**
   (26 ca lỗi biên dịch đã so qua `compile()` — 0 lệch); payload generic vẫn xoá kiểu ⇒ `VBC1` **53 tag**
   **không đổi**.
4. **606 test** workspace (21 diag + 96 syntax + 18 fmt + 143 types + 24 interp + 50 bytecode + 9 vm +
   **245 cli**); **370 ca ngữ nghĩa / 18 nhóm** (**327** `.vis` + **43** `.vish`, 52 `.err`).
5. **`./vis-dev gate --fast` PASS** + **`./vis-dev nightly --fuzz-minutes 1` PASS** (fuzz parser
   **2.853.740 ca / 64.830.216 token / 0 vi phạm**, 60,0 s); khởi động ấm **11,4–11,6 ms** (debug) ⇒ **đạt**
   §19 `< 20 ms [ƯL]`; **allowlist doc-examples 8 → 6** (hai khối xanh nhờ ST-37).

## 2. Bảng task vòng 6 (ST-37 → ST-43)

| ID | Trạng thái | Commit | Cổng | Kết quả đo được |
|---|---|---|---|---|
| ST-37 | DONE | `f9e6db5` | fast | **khe ống hợp nhất** (chọn theo **khả năng nhận**, một chiều), `CmdSig`/`SigSource` trong `vis-types`, `( [1,2] ) | take 1` ⇒ `[1]`; **521 test**; allowlist **8 → 6** (khối 1451, 672) |
| ST-38 | DONE | `86f7804` | fast | **closure-arg** ⇒ `Lambda` (**span tuyệt đối**, đệm byte; ca tiếng Việt+emoji), `where`/`each`(**trả `()`, stage cuối**)/`first`(rỗng ⇒ `none`)/`count`, sugar `it`; **538 test**, sem 349 → 355 |
| ST-39 | DONE | `1c3f1d3` | fast | `sort_by` **shell** + `--desc` (**ổn định hai chiều**, đảo comparator), khoá `int/size`(**byte**)/`duration`(**ns**)/`str`; `bool` ⇒ `VS0412`; **550 test**, sem → 359 |
| ST-40a | DONE | `9b97d75` | fast | **prelude** `FileEntry`/`ProcEntry` (khai trong Rust, **chỉ** `.vish`), `it` **kiểu thật**, **`VS0605`** (`COMMAND_CODES` 4 → 5, `all_codes` 28 → 29); **563 test**, sem → 363; khởi động +0,08 ms |
| ST-40b | DONE | `942bcff` | fast | **`get`/`select`** — `select` ⇒ **bản ghi ẩn danh** giữ **đúng thứ tự đối số**; tách `lower/chieu.rs` (R1); **576 test**, sem → 367 |
| ST-41 | DONE | `f2c256f` | fast | **`Render`**: `list[bản ghi]` ⇒ **bảng** (str trái, số phải, **NFC = số `char`**), bản ghi đơn ⇒ **khối**; `(rỗng)` **chỉ** cho bản ghi; **`print` không đổi**; cache `+render-st41`; **592 test**, **7 `.out` đổi có lý do** |
| ST-42 | DONE | `331dc58` | fast | **`ls`/`open`** qua `FsHost::list_entries` (metadata; sắp tên byte; `--all`; `--long` no-op; `ls` trống ⇒ `"."`) + **`--fs-mock`** tất định; **khối 672 chạy thật**; **606 test** |
| ST-43 | READY → **báo cáo này** | — | fast | tệp này + `R6-viscript.json` (QUEUE@head ghi `READY`) |

**49 hàng QUEUE** khớp `queue_at(654accb)`; **0 hàng `BLOCKED`/`REVIEW`** ⇒ `blocked[]` rỗng.

## 3. Số đo (mọi số tái lập bằng lệnh ghi kèm; chạy 2026-11-13, giờ UTC ghi trong ngoặc)

| Số | Giá trị | Lệnh |
|---|---|---|
| test workspace | **606** = 21 diag + 96 syntax + 18 fmt + 143 types + 24 interp + 50 bytecode + 9 vm + **245 cli** | `cargo test --workspace`; theo crate `cargo test -p <crate>` (10:5xZ) |
| ca ngữ nghĩa | **370 ca / 18 nhóm** | `find test/sem -name '*.vis' -o -name '*.vish' \| wc -l` |
| theo đuôi | **327 `.vis` + 43 `.vish`**; **52 `.err`** | `find test/sem -name '*.vis' \| wc -l` v.v. |
| `vis test` (interp) | **370 ok / 0 `not ok`**, exit 0; `1..370` | `vis test` |
| `vis test --vm` | **370 ok** (**6 SKIP** = ca cần host chương trình ngoài) **/ 0 đỏ** | `vis test --vm` |
| parity chẩn đoán | **344/344 khớp, 0 lệch** (so **đủ** `(mã, dòng, cột, thông điệp)`; **26** ca lỗi biên dịch so qua `compile()`) | `cargo test -p vis-vm --test vs_interp` |
| differential | `DIFF tong=100000 loi_tinh=0 chay=100000 lech=0 giay=208.0 toc_do=481/s seed=3523158054` | `./bin/vis-diff` |
| fuzz parser | **2.853.740 ca / 64.830.216 token / 47.562 ca/s / 0 vi phạm** (60,0 s) | `./vis-dev nightly --fuzz-minutes 1` (10:55Z) |
| fuzz đột biến verifier | `cargo test -p vis-vm --test fuzz_mutate` — **318 module**, 1,4 s | lệnh ghi kèm |
| VBC1 | **53 tag** (`Instr`, version 1) — **không đổi** suốt vòng 5 và 6 | `crates/vis-bytecode/src/format.rs` |
| sổ mã | **7 sổ**, `all_codes()` **29** (thêm **`VS0605`** ở `06xx`); `next_block()` **801** | `crates/vis-diag/src/lib.rs`, `tests/messages.rs` |
| khởi động ấm | **11,46 ms** (interp) · **11,58 ms** (`--vm` cache) · **11,40 ms** (`--vm --no-cache`) — trung vị 15 mẫu, debug | `vis run [--vm]` × 15 |
| khởi động + prelude (A/B) | **10,06 → 10,14 ms** (`.vish` rỗng; A/B xen kẽ 120 lần/bên) ⇒ **+0,08 ms** (~0,8 %) | ST-40a, ghi trong JOURNAL |
| allowlist doc-examples | **8 → 6** (khối 1451, 672 xanh **thật** nhờ ST-37; ST-42 làm 672 **chạy** được) | `test/doc-examples/doc_examples.py` (`ALLOW_COUNT = 6`) |
| cổng | **PASS** | `./vis-dev gate --fast` (10:49Z, và chạy lại sau khi ghi báo cáo) |
| nightly | **PASS** | `./vis-dev nightly --fuzz-minutes 1` (10:55Z) |

## 4. §19 — VSH1a đạt tới đâu

| Hạng mục §19 | Trạng thái | Ghi chú |
|---|---|---|
| Khe ống hợp nhất (không so bằng kiểu) | **đạt** | `CmdSig` generic + `hop_nhat_cmd`; `( [1,2] ) | take 1` ⇒ `[1]` |
| Closure-arg + span tuyệt đối | **đạt** | lỗi trong thân báo đúng `dòng:cột`, kể cả tệp nhiều byte trước closure |
| `where`/`each`/`first`/`count` | **đạt** | `each` là **stage cuối** (nối tiếp ⇒ `VS0603`) |
| Sugar `it` + viết tắt trường | **đạt (một phần)** | đạt trên kiểu **suy được**; `where size` (kiểu không `bool`) còn `VS0501` lúc chạy ⇒ **VSH1b** |
| Prelude bản ghi + `VS0605` | **đạt** | `FileEntry`/`ProcEntry`; `.vish` **chưa** có cú pháp khai `type`/`cmd`/`let` có kiểu ⇒ prelude chỉ dùng qua literal ⇒ **VSH1b** |
| `sort_by --desc` (ổn định) | **đạt** | hai `sort_by` (shell vs `std.list`) ⇒ **VS2** |
| `get`/`select` | **đạt** | `select` ⇒ bản ghi ẩn danh; `to json` cho nó ⇒ **VS2** |
| `Render` (bảng/khối, NFC) | **đạt** | `print` cũ **không đổi**; `(rỗng)` chỉ cho bản ghi |
| `ls`/`open` + `--fs-mock` | **đạt** | khối **672 chạy thật**; `--long` no-op ⇒ **VSH1b** |
| Hiệu năng khởi động < 20 ms | **đạt** | 11,4–11,6 ms (debug) |

## 5. Lỗ hổng đã biết — đầy đủ

**Đã đóng trong vòng 6**: khe ống hợp nhất · `( [1,2] ) | take 1` · closure-arg + `where`/`each`/`first`/`count`
+ sugar `it` · `sort_by --desc` (ổn định) · prelude `FileEntry`/`ProcEntry` + **`VS0605`** · `get`/`select` ·
`Render` (bảng/khối, NFC) · `ls`/`open` + `--fs-mock` · **khối 672 chạy thật**.

**Còn mở** (⇒ **VSH1b** trừ khi ghi khác):

1. `.vish` **chưa** có cú pháp khai `cmd`/`type`/`let` có kiểu ⇒ prelude chỉ dùng được qua literal.
2. `[1,2] | take 1` **không ngoặc** ⇒ `VS0201` (stage đầu là biểu thức chưa được văn phạm §9.5 nhận).
3. `( … )` làm **đối số** ⇒ tách thành stage ⇒ `VS0603`.
4. Chẩn đoán **lúc chạy** trong `( … )` dùng vị trí **tương đối** trong ngoặc.
5. `--long` **no-op** (chưa đổi hiển thị/hành vi).
6. `select` trả **bản ghi ẩn danh** ⇒ chưa in/ghi như `map` khi `to json`/`save json`.
7. **Hai** `sort_by` (shell khoá nhiều kiểu + `--desc`; `std.list.sort_by` khoá `fn(int) -> int`).
8. `where size` (thân vị từ không phải `bool`) hiện **`VS0501` lúc chạy** (chưa kiểm tĩnh khi pipe `{error}`).
9. `CODEGEN_TIN_TYCK = 21` (tin tyck ở 21 chỗ sinh mã, chưa xoá hết).
10. **`Ty::Any`** chưa có ⇒ `map[str, Any]` chưa biểu diễn được.
11. **`&self`** (phương thức nhận `self` tham chiếu) chưa có.
12. **Mồ côi một tệp**: chưa có kiểm "mọi tệp đều được `use`/chạm".
13. **Siêu trait** (supertrait) chưa có.
14. **`derives` chưa sinh mã** (chỉ đánh dấu).
15. **`Hop[Hop[int]]`** (generic lồng) chưa hỗ trợ.
16. **consent / `cap[Proc]` / `cap[Dir]`** (cưỡng chế quyền) ⇒ **VS2**.
17. **`pkg.*` / registry** (gói) ⇒ **VS2**.

## 6. VD-69 … VD-76 (mỗi cái một dòng)

| VD | Một dòng |
|---|---|
| VD-69 | Phạm vi vòng 6 (VSH1a): khe ống hợp nhất, closure-arg, `sort_by --desc`, prelude + `VS0605`, `print` không đổi, `ls`/`open` + `--fs-mock`, allowlist **8 → 7** chỉ khi thật xanh. |
| VD-70 | Khe ống chọn tham số vị trí **đầu tiên nhận được** kiểu vào (hợp nhất **một chiều**, `{error}` ⇒ vị trí đầu, không vào `--cờ`), `CmdSig`/`SigSource` ở `vis-types`, allowlist **8 → 6**. |
| VD-71 | Closure-arg ⇒ `Lambda`, **span tuyệt đối** bằng đệm byte; `where/each/first/count` khai hai chỗ + **một** hiện thực `Host::call`; `each` trả `()` là **stage cuối**; `where 1` ⇒ `VS0412`. |
| VD-72 | `sort_by` **shell** mới (`--desc` là cờ, ổn định hai chiều theo byte/ns/UTF-8; `bool` ⇒ `VS0412`); `std.list.sort_by` giữ nguyên — **hai** `sort_by` là nợ VS2. |
| VD-73 | Prelude `FileEntry`/`ProcEntry` **trong Rust**, chỉ `.vish`; `it` có kiểu thật; **`VS0605`** (`06xx` 4 → 5; `all_codes` 29; `BOOKS` 7; `next_block` 801). |
| VD-74 | `get`/`select`: `get` ⇒ sugar `it => it.<f>`; `select` ⇒ **bản ghi ẩn danh** giữ **đúng thứ tự đối số**; kiểu tính từ **thân lambda** ở `kiem_builtin`. |
| VD-75 | **`Render`**: `list[bản ghi]` ⇒ bảng (str trái, số phải, độ rộng = **số `char` sau NFC**), bản ghi ⇒ khối; `(rỗng)` **chỉ** cho bản ghi; `print` không đổi; cache `+render-st41`. |
| VD-76 | **`ls`/`open`**: `FsHost::list_entries` (metadata, **sắp tên byte**, không đệ quy), `ls` trống ⇒ `"."`, `--long` no-op, `--fs-mock` chỉ để test; `size` theo **bảng lexer** (thập phân). |

## 7. Đề xuất cho Agent 0

1. **AGENTS §8**: mọi hàng ST trong vòng 6 đã **DONE**; đề nghị đóng vòng 6 và mở **R7 (VSH1b)** với 4 việc
   ưu tiên: cú pháp khai `cmd`/`type`/`let` có kiểu trong `.vish`; `where` kiểm tĩnh đầy đủ (hết nợ `VS0501`);
   `--long` thật; `( … )` làm đối số + vị trí chẩn đoán tuyệt đối trong ngoặc.
2. **QUEUE còn thiếu cột?**: bảng hiện **49 hàng × 6 cột** khớp `queue_at(654accb)` — **không** thiếu cột;
   nhưng hàng `ST-43` nên đổi `READY → DONE` sau khi commit báo cáo này.
3. **Hàng BLOCKED nên mở lại**: hiện **0** hàng `BLOCKED`/`REVIEW`; các nợ VS2 nằm ở §5 (mục 6–8, 10–17),
   đề nghị tách thành task **ST-44+** thay vì để dạng `BLOCKED` chung.
