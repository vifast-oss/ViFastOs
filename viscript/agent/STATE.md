# STATE — ViScript (vòng 2 / VS0)

Người ghi: **Agent 2** dòng ViScript. Cập nhật **sau mỗi bước**.
Khởi tạo: 2026-11-12 bởi người điều phối (vòng vá sau review). Cập nhật vòng 2 (VS0): 2026-11-13.

```
HEAD          : xem `git log -1` (vòng 2 mở ở `aae08d4`)
BUILD_ID      : —
Task          : **ST-42** (`ls`/`open` + `--fs-mock`) — xong, **chưa commit**.
Bước          : 5/5 (`FsHost::list_entries` → `ls`/`open` + vá ret theo prelude → `--fs-mock` → 14 ca + 4 fixture → gate PASS)
Nhánh         : main
Cổng gần nhất : `./vis-dev gate --fast` → **PASS** lúc **2026-11-13T10:49:20Z**; **606** test / sem **370** / parity **344/344** / diff 100k **0 lệch** (207,2 s)
Việc dở       : commit `[ST-42]` + push (đã được uỷ quyền)
                chờ Agent 0 cho commit `[ST-14]`.
```

## ST-13b — sửa 10 mục review ST-13 (reviewer3), 2026-11-13

| # | Mục review | Sửa | Khoá bằng |
|---|---|---|---|
| 1 | **BLOCKER** `Never`/return-tail | thêm `Type::Never`: `return` ⇒ `never`; `if`/`match` toàn nhánh phân kỳ ⇒ `never`; `unify_branch`/`coerce` bỏ qua `never` | `tyck_v2::never_vong_lap_va_for_theo_vs0` (4 ca) |
| 2 | **BLOCKER** panic tràn range | `iter_items`: `checked_add`, chạm `i64::MAX` ⇒ **dừng dải** (không lỗi, không panic) | `interp_v2::range_khong_tran_so` (3 ca) |
| 3 | **MAJOR** thân vòng lặp bị ép `unit` | `kiem_than_bo` (bỏ giá trị cuối) cho `for`/`while`/`loop`; `scope` **cho giá trị khối** (thống nhất interp) | `interp_v2::than_vong_lap_bo_gia_tri`, `tyck_v2` (3 ca) |
| 4 | **MAJOR** `elem_of_iter` lệch | `Str ⇒ char`; range VS0 **chỉ `int`** (`size`/`duration`/`char` ⇒ `VS0412`), span thật | `tyck_v2` (3 ca) + `interp_v2` |
| 5 | **MAJOR** số học bất đối xứng | runtime thêm `size/duration * percent`, `percent / percent ⇒ float`, `size%size`, `duration%duration`, `percent%percent`; tyck thêm 3 phép `%` | `interp_v2::so_hoc_doi_xung_voi_don_vi` (6 ca) |
| 6 | **MAJOR** bảng phương thức không kiểm kiểu | `filter`/`each` đòi **hàm 1 tham số** (filter trả `bool`), `sum` chỉ list số; **parser cho từ khoá sau `.`** ⇒ `m.has(..)` gọi được | `interp_v2::phuong_thuc_kiem_kieu_va_has` (4 ca) |
| 7 | **MAJOR** vị trí sai | `pos`: **tab = 4 cột**; lỗi `iter_items`/range dùng **span thật** (không còn `0,0`) | `interp_v2::vi_tri_tab_va_utf8` (tab giữa dòng = 4 cột; tiếng Việt/emoji theo scalar) |
| 8 | **MAJOR** `break`/`continue`/`?` ngoài ngữ cảnh | `loop_depth`/`fn_depth`: ngoài vòng/hàm ⇒ **`VS0501`** (chọn runtime, ghi JOURNAL) | `interp_v2::break_continue_va_try_ngoai_ngu_canh` |
| 9 | **MAJOR** `Show` | `DUR_UNITS` thêm **`w`**; `char` điều khiển **escape** (`'\n'`); đơn vị decimal trước binary (`1024kb`); bỏ mục trùng | `show_char` unit-test + `interp_v2::show_don_vi_va_char` |
| 10 | **Coverage + số liệu** | 9 test mới (7 `interp_v2` + 1 unit `show_char` + 1 nhóm `tyck_v2`); đính chính STATE/JOURNAL (4 test cũ; 227 = 32+18+96+20+45+16; 74→79 ca) | chính các tệp trên |

**Đỏ trước**: `JOURNAL/st13b-red-run.log` — `interp_v2` **6/7 test đỏ** (mục 2,3,5,6,7,9), `tyck_v2` **1/11**
(nhóm `SINH_TU_DONG`). Xanh sẵn: `break_continue_va_try_ngoai_ngu_canh` (đã ra mã lỗi ở đường khác) —
**khai rõ**, không giả vờ đỏ. Chụp bằng `git stash push -- crates/{vis-types,vis-interp,vis-syntax}/src`
trên harness cuối, rồi `stash pop`.

**Đo được**: **247** test (gồm 1 test `sem` của ST-14 đang dở **trong cây, chưa commit**);
`vis-dev gate --fast` PASS — 2026-11-12T23:14:55Z; fmt/clippy sạch.

**Lỗ hổng đã biết (mục C, chưa làm)**: `None(1)` không kiểm đối số; map literal trùng khoá (giữ cả hai,
`get` lấy bản đầu); `str.chars()` trả `list[str]` (nên **chưa** tạo được `char` từ chuỗi ⇒ test `Show` của
`char` điều khiển chỉ chạy qua unit-test); thứ tự đơn vị `size` (decimal trước binary — đã chốt);
`unify_branch` bất đối xứng `T`/`T?`; `BinOp` bỏ qua khi một vế `{error}`; **range bị vật chất hoá** (cap
`10^7` phần tử/lần — giữ nguyên, **không** nới); char literal có escape (`'\n'`) bị lexer từ chối
(**VS0105**) — lỗi **mới phát hiện**, ghi để ST sau.

## ST-42 — `ls`/`open` + `--fs-mock` (2026-11-13)

| # | Việc | Kết quả |
|---|---|---|
| 1 | **`FsHost` metadata** | thêm `MucFs` + `list_entries` **default `Err`**; `Runner` thật dùng `std::fs::metadata` |
| 2 | **`ls`** | `list[FileEntry]` (prelude), **sắp theo tên byte**; `--all` gồm `.`; **không đối số ⇒ `"."`**; `--long` **no-op** |
| 3 | **`open`** | ⇒ `str`; lỗi ⇒ **`VS0501`** nêu đường dẫn + lỗi |
| 4 | **`--fs-mock`** | JSON `dirs`/`files` tất định; `size` theo bảng lexer, `modified` = duration từ epoch; `FsMock` trong `vis-cli/src/fsmock.rs` |
| 5 | **Đo** | **606** test (592 + **14**); sem **370** không đổi; 4 fixture `test/cmd/fs/**`; **khối 672 end-to-end**, interp ↔ `--vm` cùng byte; **allowlist vẫn 6** |

 (2026-11-13)

| # | Việc | Kết quả |
|---|---|---|
| 1 | **Bảng** | `list[bản ghi]` cuối pipeline ⇒ tiêu đề = tên trường, `str` căn **trái**, số/`size`/`duration`/`percent` căn **phải**, 2 dấu cách giữa cột, giữ thứ tự cột của bản ghi đầu |
| 2 | **Khối + rỗng** | bản ghi **đơn** ⇒ khối `khoá  giá trị`; `list[bản ghi]` rỗng ⇒ `(rỗng)`; `list[str]`/`list[int]` rỗng **giữ `[]`** (hint từ bộ hạ) |
| 3 | **NFC, `print` không đổi** | độ rộng = **số `char` sau NFC** (`unicode-normalization` có sẵn) ⇒ NFD/NFC cùng bảng; `print` giữ nguyên từng byte (`boc_render` gọi builtin `render`) |
| 4 | **Cache** | `cache::PHIEN_BAN` thêm nhãn `+render-st41` để module cache cũ hết hiệu lực |
| 5 | **Đo** | **592** test (576 + **16**); sem **367 → 370** (+3 `vish/rt-*.vish`); parity **341 → 344**; `fuzz_mutate` 315 → **318**; `diag_cmp` 315/367 → **318/370**; **7 `.out`** đổi cùng lý do |

 (2026-11-13)

| # | Việc | Kết quả |
|---|---|---|
| 1 | **`get[T, K]`** | `get name` ⇒ `it => it.name`; `get it.name` tương đương; `get size` ⇒ `list[size]`; nối tiếp `get name | count` |
| 2 | **`select[T, M]`** | `select pid, name` ⇒ lambda trả **bản ghi ẩn danh**, kiểu `list[record#N]`; **giữ đúng thứ tự đối số** (`select name, pid` đảo) |
| 3 | **Một hiện thực** | `builtin::call` (`"get" | "select"` = map qua closure, `Host::call`) ⇒ interp **và** VM cùng mã; tyck tính `list[<kiểu thân>]` để không lộ `TypeRef K/M` |
| 4 | **Lỗi** | trường lạ (bản ghi thật) ⇒ **`VS0605`**; `get` không đối số ⇒ **`VS0602`**; `it` không phải bản ghi ⇒ **kiểm động** (không đỏ oan); list rỗng ⇒ `[]` |
| 5 | **Đo** | **576** test (563 + **13**); sem **363 → 367** (+4 `vish/gs-*.vish`); parity **337 → 341**; `fuzz_mutate` 311 → **315**; `diag_cmp` 311/363 → **315/367**; tách `lower/chieu.rs` (cmd.rs 1.390 dòng) |

**Nợ VS2**: `select` trả bản ghi ẩn danh ⇒ chưa in/ghi như `map` khi `to json`; muốn `map[str, Any]` cần `Type::Any`.

 bản ghi + `it` kiểu thật + `VS0605` (2026-11-13)

| # | Việc | Kết quả |
|---|---|---|
| 1 | **Prelude** | `vis-types/src/prelude.rs`: `FileEntry { name, size, modified, is_file }`, `ProcEntry { pid, name, cpu, mem }`; đăng ký **chỉ** `.vish` (`dang_ky_prelude` trước khi hạ item); `.vis` **không** thấy |
| 2 | **`it` kiểu thật** | `kieu_text` nhận literal bản ghi khớp prelude (tách `,` độ sâu 0), literal đơn vị (`5mb`/`90s`/`50pct`), `ten(...)` theo `cmd_sigs`, tên biến có kiểu ⇒ `where`/`sort_by` **kiểm tĩnh** |
| 3 | **`VS0605`** | `COMMAND_CODES` **4 → 5**; `BOOKS` 7, `all_codes()` **28 → 29**, `next_block()` **801**; test sửa kèm lý do |
| 4 | **Không đỏ oan** | Không suy được kiểu ⇒ giữ **kiểm động**; khối 672/1451 vẫn `ok`; allowlist **vẫn 6** |
| 5 | **Đo** | **563** test (550 + **13**); sem **359 → 363** (+4 `vish/pl-*.vish`); parity **333 → 337**; `fuzz_mutate` 307 → **311**; `diag_cmp` 307/359 → **311/363**; khởi động A/B: **10,06 → 10,14 ms** (+0,08 ms) |

**Ghi chú**: `.vish` **chưa** có cú pháp khai `type` ⇒ chưa thể có `VS0302` trùng prelude; ca `.vis` chứng minh
prelude **không** lộ sang `.vis` (người dùng khai `type FileEntry` riêng vẫn chạy). Nợ khi `.vish` có item khai kiểu.

 + `--desc` (2026-11-13)

| # | Việc | Kết quả |
|---|---|---|
| 1 | **`sort_by` lệnh shell** | `sort_by[T](xs, f: fn(T) -> K, --desc: bool = false) -> list[T]`; khoá `int`/`size`(byte)/`duration`(ns)/`percent`/`str`(byte UTF-8); **một** hiện thực `builtin::call` qua `Host::call` (interp + VM) |
| 2 | **Ổn định hai chiều** | đảo **comparator**, không đảo mảng (`sort_by` Rust ổn định) ⇒ khoá bằng nhau giữ thứ tự gốc **cả** `asc` và `desc` — có ca riêng trên khoá **`size`** |
| 3 | **`--desc` là cờ** | không bị khe ống chiếm; `--desc=false` hợp lệ; parser sửa: **thân closure dừng ở `--cờ`** (nếu nuốt, `x => x --desc` thành `-(-desc)`) |
| 4 | **Khoá lỗi** | `bool`/`map`/`record`/`list` ⇒ **`VS0412`** (kiểm ở lowering khi biết kiểu + `kiem_builtin` khi có ≥ 2 đối số); runtime chốt thứ hai `VS0501` |
| 5 | **Đo** | **550** test (538 + **12**); sem **355 → 359** (+4 `vish/sx-*.vish`); parity **329 → 333**; `fuzz_mutate` 303 → **307**; `diag_cmp` 303/355 → **307/359**; **allowlist vẫn 6** |

**Nợ VS2 (ghi JOURNAL)**: **hai** `sort_by` — shell (khoá nhiều kiểu + `--desc`) và `std.list.sort_by`
(khoá `fn(int)->int`); hợp nhất khi có generic + trait. Khối 672 chạy **thật** (cần `ls`) ⇒ **ST-42**.

 + `where`/`each`/`first`/`count` + sugar `it` (2026-11-13)

| # | Việc | Kết quả |
|---|---|---|
| 1 | **Hạ `CmdClosure`** | ⇒ `HExpr::Lambda`; **span tuyệt đối** bằng cách lex/parse lại thân với **đệm `" ".repeat(offset)`** (byte) ⇒ lỗi trong thân báo đúng `dòng:cột`, kể cả khi tệp có tiếng Việt + emoji trước closure |
| 2 | **4 lệnh mới** | `where(xs, f) -> list[T]` · `each(xs, f) -> ()` (**chỉ tác dụng phụ, stage cuối**) · `first(xs) -> T?` (rỗng ⇒ `none`) · `count(xs) -> int`; một nguồn `cmdsig` + một hiện thực `builtin::call` qua `Host::call` (cả hai đường) |
| 3 | **Sugar `it`** | `where size > 5mb` ⇒ `it => it.size > 5mb`; chỉ đổi **định danh trần chưa khai báo**, giữ `it` tường minh, giữ tên đã khai; `where 1` ⇒ **`VS0412`** (lỗi kiểm tĩnh) |
| 4 | **Sửa nhỏ** | `BUILTIN_VALUE_NAMES` **giữ 11** (thêm 4 tên làm `fn first(…)` bị `VS0302` — ghi JOURNAL); `lower_vach` hạ **vào bảng chung** để `RecId` bản ghi ẩn danh của nhóm không lệch bảng |
| 5 | **Đo** | **538** test (521 + **17**); sem **349 → 355** (+6 `vish/khe-*.vish`); parity **320 → 329**; `fuzz_mutate` 297 → **303**; `diag_cmp` 297/349 → **303/355**; **allowlist vẫn 6** (khối 672/1451 vẫn xanh) |

## ST-37 — `CmdSig` generic + khe ống hợp nhất (2026-11-13)

| # | Việc | Kết quả |
|---|---|---|
| 1 | **Seam `SigSource`** | `vis-types/src/cmdsig.rs`: `CmdSig { name, sym, generics, params, ret }` + `trait SigSource` + `BuiltinSigs` (3 builtin) + `lower_with_sigs(...)` cho ST-40/41 cắm nguồn khác **không** sửa `lower` |
| 2 | **`take` generic** | `take[T](xs: list[T], n: int) -> list[T]`; `echo`/`lines` giữ nguyên; **không** thêm `first`/`count` (ST-38) |
| 3 | **Khe ống hợp nhất** | chọn tham số vị trí **đầu tiên nhận được** (hợp nhất một chiều `hop_nhat_cmd`, buộc `T`), **không** đặt vào `--cờ`, **không** đè bind tường minh; `up_ty = {error}` ⇒ chọn **vị trí đầu**; không khớp ⇒ **`VS0603`** (mã cũ, không đổi) |
| 4 | **Stage `( … )`** | parser chỉ giữ token ⇒ hạ lại **văn bản trong ngoặc** (script, bọc `let`) ⇒ `kieu_stage` suy kiểu literal ⇒ `( [1,2] ) | take 1` chạy; **nhóm làm đối số**/không ngoặc vẫn lỗi — khoá + ghi nợ **VSH1b** |
| 5 | **Đo** | **521** test (507 + **14** `khe_ong.rs`); sem **346 → 349** (+3 `vish/khe-*.vish`); parity **320/320 → 323/323**; `fuzz_mutate` 294 → **297**; `diag_cmp` 294/346 → **297/349** |

## ST-36 — báo cáo **R5** chốt VS1c (2026-11-13)

| # | Việc | Kết quả |
|---|---|---|
| 1 | **Đo lại, không chép** | **507** test (21 diag + 96 syntax + 18 fmt + 143 types + 24 interp + 50 bytecode + 9 vm + 146 cli); sem **346 / 18 nhóm** (327 `.vis` + 19 `.vish`, 52 `.err`); `vis test` **346 ok**, `--vm` **346 ok / 6 SKIP** |
| 2 | **Differential + fuzz** | `DIFF tong=100000 loi_tinh=0 chay=100000 lech=0 giay=200.4 toc_do=499/s seed=3523158054`; fuzz parser **3.035.575 ca / 68.968.426 token / 50.593 ca/s / 0 vi phạm** (60,0 s) |
| 3 | **Bất biến** | parity **320/320** (26 ca lỗi biên dịch đã so, 0 lệch); `VBC1` **53 tag** không đổi; **7 sổ / 28 mã**; khởi động ấm **11,16–11,73 ms** |
| 4 | **Báo cáo** | `agent/reports/R5-viscript.md` (6 mục, bảng vòng 5 ST-32..ST-36, VD-64..VD-68, lỗ hổng đóng/mới, đối chiếu §19) + `.json` **đúng 11 khoá** + `.json.license`; `head = 58080a9`; **`report-check --all` PASS** (5 báo cáo, 49 hàng QUEUE) |

## ST-35 — generic **thật**: instantiation theo tổ hợp (2026-11-13)

| # | Việc | Kết quả |
|---|---|---|
| 1 | **Biểu diễn** | `TypeTable` thêm `rec_inst`/`enum_inst` + `rec_pos`/`enum_pos`: mỗi tổ hợp `Hop[int]` ⇒ **`RecId` riêng** (bản sao với trường đã thay `T`), cache theo `(tên, args)` ⇒ `Hop[int] ≠ Hop[str]` **tự nhiên** qua mọi so sánh có sẵn; **không** thêm biến thể `Type`, **không** đụng `codegen`/`VBC1` |
| 2 | **Lowering** | template `type`/`enum` generic giữ `TypeRef{"T"}` (`sym_generic`); chữ ký hàm generic cũng giữ `T` (thân hàm vẫn **xoá** ⇒ không đỏ ca cũ); `named_type` instantiation ⇒ `Record/Enum(id_inst)`; `Hop[T]` trong chữ ký ⇒ `TypeRef` để hợp nhất |
| 3 | **tyck** | `chuan()` (TypeRef người dùng ⇒ bản gốc khi đã xoá), `same_enum` (bản gốc ↔ inst cùng tên, **khác đối số vẫn khác**), `hop_nhat` thêm nhánh `TypeRef` người dùng + fallback **cấu trúc theo vị trí tham số** (buộc `T`, báo sai kiểu); `coerce_e` kiểm literal enum theo biến thể của **bản instantiation** |
| 4 | **Đo** | **507** test (482 + **25** `generic_that.rs`); sem **338 → 346** (+8 `test/sem/generic/gen-*.vis`, khoá **chạy** interp↔VM); parity **312/312 → 320/320**; `fuzz_mutate` **286 → 294**; `diag_cmp` 286 → 294, tổng 338 → 346 |
| 5 | **Nợ** | kiểu trả của hàm generic vẫn có thể là `{error}` nếu `T` chưa suy; chưa có instantiation **lồng nhau tường minh** (`Hop[Hop[int]]` chạy được nhưng không tạo id riêng cho lớp trong — an toàn vì payload xoá kiểu) |

## ST-34 — `| complete` + pipe→stdin (2026-11-13)

| # | Việc | Kết quả |
|---|---|---|
| 1 | **pipe→stdin** | `a \| b` (b ngoài) ⇒ đối số vị trí **thứ 2** của `exec.<ten>` là **stdin**: `list[str]` ⇒ nối `\n` **+ newline cuối** (chỉ khi nối list); `str` (`--raw`) ⇒ **nguyên văn**; `drop` để đóng stdin ⇒ `cat`/`grep` không treo |
| 2 | **`\| complete`** | Stage **giả**, chỉ sau chương trình ngoài ⇒ **đổi tên** callee `exec.[complete.][raw.]<ten>` (**không** đổi HIR/codegen); trả record `{stdout: list[str], stderr: list[str], code: int}`; exit≠0 **hết** là `VS0501`; trên stage Vis/`complete` có đối số ⇒ **`VS0602`** |
| 3 | **An toàn** | `MAX_STDIN` **8 MiB** (vượt ⇒ `VS0501` nêu **số byte thật** + giới hạn, **trước khi spawn**); ghi stdin bằng **thread riêng** + `wait_with_output` ⇒ không deadlock; `EPIPE` sau timeout bỏ qua; timeout 10 s/**stage**, thông điệp nêu **stage nào** |
| 4 | **Một nguồn ngữ nghĩa** | Toàn bộ quyết định ở `builtin::call` (nhánh `exec.`) mà **cả** interp **và** VM gọi ⇒ pipe vẫn là **giá trị**, codegen không đổi |
| 5 | **Đo** | **482** test (466 + **16** `pipeline.rs`); **4** fixture `.vish` trong `test/cmd/pipe/**`; VM không host ⇒ `VS0604` |

## ST-33 — `json.decode` + `std.fs` tối thiểu (qua `Host`), 2026-11-13

| # | Việc | Kết quả |
|---|---|---|
| 1 | **`FsHost` trên `Host`** | `trait FsHost` 4 hàm (`read/write/exists/list`) + `Host::fs()` mặc định `None` ⇒ **`VS0604`** (đúng mẫu `Ngoai`, VD-60 (2)); `Interp`/`Vm` giữ `Option<&mut dyn FsHost>`; `vis-interp`/`vis-vm` **không** biết `std::fs` |
| 2 | **`vis-cli` cài** | `ngoai::Runner` impl `FsHost` bằng `std::fs`; `--explain-perms` in `FS read|write|exists|list <p>` ra stderr; **cho phép** đường dẫn tuyệt đối/`..` (consent thật + `cap[Dir]` ⇒ **VS2**); lỗi OS ⇒ `Err(msg)`; `list` **sắp xếp**, **không** đệ quy |
| 3 | **`json.decode`** | `Result[Any, str]` (`Ty::Any` — bỏ kiểm tĩnh theo VD-59 (3)); object ⇒ `map[str, Any]` **giữ thứ tự**, trùng khoá ⇒ giá trị cuối/vị trí đầu; array ⇒ `list[Any]`; `.`/`e` ⇒ float, còn lại ⇒ int (**tràn i64 ⇒ lỗi**); escape **đủ** (`\" \\ \/ \b \f \n \r \t \uXXXX` + **surrogate pair**); `null` ⇒ `none` (`Opt(Error)`); lỗi ⇒ `Err("JSON không hợp lệ ở byte N: …")` **theo byte**, không panic; depth cap 128 |
| 4 | **Đối xứng** | `json.encode(decode(encode(x))) == encode(x)` cho int/float/bool/str/none/list/map/record(⇒map); **enum/Result chưa round-trip** (nợ JOURNAL) |
| 5 | **Đo** | **466** test (443 + **23** `std_fs_json`); ca không-host khoá **cả hai đường** (interp + VM ⇒ `VS0604`); 338 ca sem **giữ nguyên** (không fixture nào cần FS) |

## ST-32 — harness so chẩn đoán **biên dịch** hai đường (2026-11-13)

| # | Việc | Kết quả |
|---|---|---|
| 1 | **Ba tầng khoá cứng** | **286 xanh** / **26 lỗi biên dịch** / **26 lỗi lúc chạy** = **338**, **0 ca bỏ qua** (`diag_cmp::phan_loai_ba_tang_khoa_cung`) |
| 2 | **Hai đường cho ca biên dịch** | `check` ↔ `run --vm --no-cache`: **cùng** `(mã, dòng, cột, thông điệp)`; exit **ánh xạ** `1 ↔ 2` (không đòi bằng — đúng thiết kế); 26/26 khớp |
| 3 | **Chuẩn hoá escape** | **một** hàm `chuan_hoa` dùng chung 3 đường + unit-test riêng (`\n`, `\t`, `\\`); ca `loi/cu-phap.vis` (check in escape, run in xuống dòng thật) nay khớp |
| 4 | **Đường bytecode** | 26 ca biên dịch: `compile()` **không panic**; **21/26** codegen vẫn sinh module (VM chạy "sạch") ⇒ **codegen tin tyck**, front end là cổng — khoá `CODEGEN_TIN_TYCK = 21` |
| 5 | **Lỗi lúc chạy** | `check` **rỗng**; `run` = `run --vm`; **6 ca cần host** (4 xanh + 2 lỗi chạy) ⇒ `VS0604` ở VM, khoá đúng số ca |
| 6 | **Đo** | **443** test (436 + **6** `diag_cmp` + **1** `vis_test`); `vs_interp` phủ **cả 338** (312 + 26) |

## ST-31 — báo cáo `R4-viscript` (chốt VS1b), 2026-11-13

| # | Việc | Kết quả |
|---|---|---|
| 1 | **Đo lại toàn bộ** | 436 test (21+96+18+118+24+50+9+100); 338 ca/18 nhóm, 52 `.err`; `vis test` **338 ok**, `--vm` **338 ok/6 SKIP/0 đỏ`; parity **312/312, 0 lệch**; diff 100k `lech=0 giay=203.5 seed=3523158054`; fuzz parser 1 phút **3.098.833 ca/0 vi phạm**; warm start **10,31–11,08 ms**; `VBC1` **53 tag**; **7 sổ/28 mã**; SHA-256 vector FIPS |
| 2 | **`reports/R4-viscript.{md,json}`** | Đúng **11 khoá**, `round = 4`, `head = d92e947`, **36 hàng** QUEUE; bảng ST-25..ST-31 + ST-30b; VD-57..VD-63; **13 mục** lỗ hổng; đối chiếu §19 trung thực (đạt/VS2) |
| 3 | **`report-check`** | `PASS (1 báo cáo, 36 hàng QUEUE phủ đủ)` — hàng `ST-20b` nay 6 cột nên **có** trong JSON |
| 4 | **Cổng + nightly** | `gate --fast` **PASS** (06:07Z); `nightly --fuzz-minutes 1` **PASS** (06:09:38Z) |

## ST-30b — đồng bộ chẩn đoán interp ↔ VM (2026-11-13)

| # | Việc | Kết quả |
|---|---|---|
| 1 | **Đo đỏ** | Thêm ca `.err` vào `vs_interp` (bỏ `continue`) + `diff_100k` so thêm `thông điệp` ⇒ HEAD lộ **9 ca lệch / 3 nhóm**: (a) chỉ số/khoá map: VM `VS0501`+câu ngắn ↔ interp `VS0503`+câu đầy đủ; (b) đệ quy sâu: VM thiếu vế "— đệ quy không có điểm dừng?"; (c) tên lạ: VM "hàm" ↔ interp "tên" |
| 2 | **Sửa bên sai** | `vm.rs::index` dùng `IDX` (`VS0503`) + **đúng câu** interp (âm/ngoài phạm vi kèm `độ dài`/khoá map), thêm nhánh `Bytes`; `vm.rs` thêm vế đệ quy; `builtin.rs` "hàm" → "**tên**"; **không** nới test |
| 3 | **Siết differential** | `vs_interp`: so **đủ 338** ca (312 khớp / **0 lệch** / 26 bỏ qua vì lỗi **biên dịch**); ca mới `hai_duong_cung_so_chan_doan` khoá **cùng số** chẩn đoán (30 ca có chẩn đoán lúc chạy); `diff_100k` so `(mã, dòng, cột, **thông điệp**)` |
| 4 | **`vis test --vm`** | Bỏ khoá nới "chỉ exit" ⇒ so **đúng mã** như interp; chỉ còn SKIP ca cần **host chương trình ngoài** (6 ca, có lý do) |
| 5 | **Đo** | **436** test; `vis test` = **338 ok/0 đỏ**, `--vm` = **338 ok/6 SKIP/0 đỏ**; sem **338** giữ nguyên |

## ST-30 — SHA-256 thật + `vis test` (2026-11-13)

| # | Việc | Kết quả |
|---|---|---|
| 1 | **`sha2`** | Thêm `sha2 = "0.10"` (MIT OR Apache-2.0, Agent 0 duyệt allowlist) vào `vis-bytecode`; **thay** FNV-1a × 4 seed bằng **SHA-256 thật** trong `format::source_hash` ⇒ dùng cho **cả** `VBC1` (`hash nguồn`) **và** khoá cache (`cache::khoa_multi`). Cache vẫn nạp qua `load_verified` (không đổi). |
| 2 | **`vis test`** | `vis test [--vm] [thư mục|tệp]` — quét `test/sem/**`, in **TAP**: `TAP version 13`, `1..338`, `ok/not ok`, mỗi `not ok` kèm `# <tệp>:<dòng>:<cột>: <mã> <thông điệp>`; exit **0** toàn xanh, **1** có đỏ, **2** dùng sai. Không truyền đường dẫn ⇒ tự tìm `test/sem` (leo từ cwd). |
| 3 | **`--vm`** | Chạy đường VM (`--vm --no-cache`); ca cần **host chương trình ngoài** ⇒ `ok … # SKIP` (VM không có host — ST-27); ca `.err` khoá **exit** (mã chẩn đoán có thể khác — VM in một, interp in hai cho lỗi chỉ số; nợ ghi JOURNAL) |
| 4 | **Đo** | **435** test (421 + **5** `vis-bytecode/tests/hash.rs` + **9** `vis-cli/tests/vis_test.rs`); `vis test` = **338/338 ok** (interp), `--vm` = 338 ok + **4 SKIP**, 0 đỏ; sem 338 + differential 286 giữ nguyên |

**Lưu ý lịch sử**: phần `Cargo.lock`/`vis-bytecode/{Cargo.toml,format.rs}` của ST-30 **đã lỡ** nằm trong
commit `520c2fe` (thông điệp ghi ST-29) — **Agent 0 gây ra**, không phải nội dung ST-29; không rewrite lịch sử.

## ST-29 — trait + `derives` (2026-11-13)

| # | Việc | Kết quả |
|---|---|---|
| 1 | **`derives`** | `Eq`/`Show`/`Json` = **kiểm + đăng ký** (không sinh mã): `Eq` = so sánh cấu trúc, `Show` = `show`, `Json` = `json.encode` (đều có sẵn); derives lạ ⇒ `VS0403`; `Json` trên trường/biến thể **kiểu hàm** ⇒ `VS0403` |
| 2 | **`trait`/`impl`** | thu thập ở **lượt 1b** (sau predeclare ⇒ `impl` thấy được `type` khai sau); `impl Trait for Type` kiểm **khớp chữ ký** (thiếu/thừa ⇒ `VS0403`, sai số ⇒ `VS0401`, sai kiểu ⇒ `VS0412`); trait lạ ⇒ `VS0403` |
| 3 | **Dispatch tĩnh** | `x.m(…)` với `x` kiểu tĩnh **record/enum** và `m` không nội tại ⇒ hạ **thẳng** `Call(impl_fn)` (chèn `self` đầu) ⇒ **cùng HIR** cho interp và VM (**không** bảng `impls`, **không** bump VBC1); kiểu không tĩnh ⇒ `VS0403` "không phân giải được … (VS1b)" |
| 4 | **Nội tại thắng** | `[1,2].len()` vẫn là phương thức nội tại dù có `impl` cùng tên cho kiểu khác |
| 5 | **VS0403** | nghĩa mở rộng "trường, kiểu hoặc **phương thức** không tồn tại" (vi/en/ja) — **không** cấp `VS0404`, `TYPE_CODES` vẫn 4, `all_codes()` 28 |
| 6 | **Đo** | **421** test (392 + **22** `vis-types/tests/trait.rs` + **7** `vis-cli/tests/trait_cli.rs`); sem **338** / differential **286** giữ nguyên; gate PASS |

**Giới hạn (VS1b, ghi JOURNAL)**: `self` **theo giá trị** (`&self` cần tham chiếu = VS1); dispatch chỉ khi kiểu **tĩnh**;
mồ côi kiểm trong **một tệp** (module-cross-file = nợ); siêu trait chưa kiểm.

## ST-28 — generic hàm + kiểu `[T]` (2026-11-13)

| # | Việc | Kết quả |
|---|---|---|
| 1 | **Parser** | **Không sửa** — `generics()` đã có cho `fn`/`cmd`/`type`/`enum`/`trait`/`impl` (kiểm bằng 5 tệp thử). Văn phạm đúng: `type Hop[T]:` + trường thụt; `enum Or[A, B]:` + biến thể **trường tên**, **không** phẩy giữa biến thể; dựng bản ghi bằng **literal** `{gia_tri: 1}` + chú thích `let a: Hop[int]` |
| 2 | **Chiến lược** | **xoá kiểu (erase)** — một hiện thực cho interp **và** VM ⇒ differential 0 lệch **theo cấu trúc**; **không** đơn hình hoá (tránh bảng instantiation + nguy cơ hai đường lệch) |
| 3 | **Hạ** | `HRec`/`HEnum` thêm `generics`; `Lowerer.generic_scope`; trong `type`/`enum`, tham số kiểu ⇒ `{error}`; chú thích `Or[str,int]`/`Hop[T]` bỏ đối số ⇒ `Enum`/`Record` |
| 4 | **Tyck** | `FnSig` thêm `generics`/`raw_params`/`raw_ret`; `resolve_ann` giữ `TypeRef{"T"}` làm **dấu**; `erase_gen` dùng cho thân hàm; **`hop_nhat`** hợp nhất ở call-site ⇒ `T=int`; mâu thuẫn ⇒ `VS0412` nêu `T` + hai kiểu; kiểu trả thay `T` ⇒ giữ kiểm tĩnh ở ngữ cảnh |
| 5 | **Bug phát hiện** | (a) `vis check` **xanh** trong khi `vis run` `VS0412`/`VS0301` — bộ lọc `01xx/02xx/07xx` của ST-25; nay `check` mặc định báo **đúng chẩn đoán đầu** như `run`, lọc chỉ còn ở `--syntax` (cổng tài liệu, QD §7.4). (b) **VM bỏ qua `fn main`** (codegen trỏ entry vào thân cấp tệp) — nay entry gọi `main` nếu có, như interp |
| 6 | **Đo** | **392** test (357 + **31** `generic.rs` + **4** `generic_cli.rs`); **338** ca sem/18 nhóm (+10 `generic/*`); differential **286/286**; fuzz 286 module; gate PASS |

**Giới hạn VS1b (JOURNAL)**: `Hop[int]` ≡ `Hop[str]` (chưa có instantiation thật); phép toán trên `T` kiểm
**động** (mã hiện có của `+`); constraint/trait ⇒ **ST-29**.

## ST-27 — chương trình ngoài + variadic + `--x=v` (2026-11-13)

| # | Việc | Kết quả |
|---|---|---|
| 1 | **Host hook** | `trait Ngoai` + `Host::ngoai` (mặc định `None`) + `Interp.ngoai` + `run_with_ngoai` ⇒ **`vis-interp`/`vis-vm` không biết OS** (§14); không host ⇒ `VS0604` (có test riêng) |
| 2 | **Hạ `exec.*`** | lệnh **lạ** trong `.vish` ⇒ `HExpr::Call{Var "exec.<tên>"}` với `list[str]` từ thô; `--raw` ⇒ `exec.raw.<tên>` (trả `str`); `cho_ngoai=false` (`--no-extern`) ⇒ `VS0601` |
| 3 | **Runner CLI** | `vis-cli/src/ngoai.rs`: `std::process::Command` (mảng đối số, **không** `sh -c`), env kế thừa, stdin null, **timeout** 10 s (đổi `VIS_CMD_TIMEOUT_MS`), cap đồng thời **1**; `exit != 0` ⇒ `VS0501` kèm stderr; `--explain-perms` in `EXEC …` |
| 4 | **Variadic** | `...rest: str` (lexer: `..` + `.`) ⇒ `HParam.variadic`, kiểu `list[str]`, `FnSig.variadic` (arity tĩnh + chạy đều cho thêm đối số), interp gom phần còn lại ⇒ `x "a" "b" "c"` cho `rest = ["b","c"]` |
| 5 | **`--x=v`** | `echo --upper=true xin_chao` ⇒ `XIN_CHAO` (giá trị đổi theo kiểu cờ, không lấy khoảng trắng) |
| 6 | **Đo** | **357** test (344 + **13** `extern.rs`); **5** ca sem `.vish` mới (`ngoai-*`) ⇒ **328 ca / 17 nhóm**; differential **276/276**; fuzz 276 module; gate PASS 2026-11-13T04:55:41Z |
| 7 | **allowlist tài liệu** | **8** mục (Agent 0 APPROVED, chặt hơn): khối 662 `extern cmd git(…, ...rest: str)` nay hợp lệ nhờ variadic (VD-60) |

**Hermetic**: `/bin/echo`, `/usr/bin/printf`, `/usr/bin/true`, `/usr/bin/false`, `/bin/cat` — **`/bin/true|false` KHÔNG có trên macOS**, dùng `/usr/bin/*`; mỗi đường dẫn `assert!(exists)` trước khi dùng.

**Nợ**: `| complete` (VS1b — `grep` không khớp exit 1 hiện là lỗi), pipe→stdin của tiến trình kế (VS1b), consent thật + `cap[Proc]` (VS2).

## ST-26 — `std.*` lõi: str/list/map/path/time/json (2026-11-13)

| # | Việc | Kết quả |
|---|---|---|
| 1 | Bảng chữ ký `vis-types/src/stdlib.rs` | **33** hàm, tên **đầy đủ** `str.split`, `list.sort_by`, `time.now_ms`… ⇒ `str.join` và `path.join` không đụng nhau, không rò tên trần; `MODULES` = str/list/map/path/time/json; `HA_CHUA_HO_TRO` = `json.decode` |
| 2 | Ngữ nghĩa `vis-interp/src/stdlib.rs` | **một** hiện thực cho interp **và** VM (VD-55): `builtin::call` chuyển tiếp `std.*`; VM gọi cùng đường. Sai kiểu ⇒ `VS0501` nêu hàm + kiểu nhận |
| 3 | Tyck | `kiem_std`: kiểm **số đối số** + kiểu cụ thể (`str.*`/`path.*`/`time.*`); `list.*`/`map.*` dùng **`Any`** (chỉ số đối số) theo chỉ đạo Agent 0 — nợ generic VS1c |
| 4 | Hạ tên | `str.upper(…)` → `HExpr::Var{name:"std.str.upper"}` khi tệp đã `use std.str`; `json.decode` ⇒ `VS0301` "chưa hỗ trợ (VS1b)"; hàm lạ trong module ⇒ `VS0301` nêu tên |
| 5 | `use std.X` | `str/list/map/path/time/json` ⇒ nhập module (không phải tệp); `std.fs` ⇒ `VS0701` **ST-27**; `std.khong_co` ⇒ `VS0701` |
| 6 | `std.path` | **thuần chuỗi** (`join/basename/dirname/ext/stem/is_abs`), không chạm đĩa |
| 7 | `std.time` | đọc đồng hồ **trong Rust** (`SystemTime`/`Instant`), **không** thêm method vào `Host` |
| 8 | `std.json.encode` | compact + `encode_pretty`; map theo **thứ tự chèn**, bản ghi theo thứ tự trường, `none` ⇒ `null`, `Result` ⇒ `{"ok":…}`/`{"err":…}`, enum ⇒ `{"variant":…,"value":…}`; **decode hoãn** (VS1b) |
| 9 | Test | **344** workspace (335 + 9 `stdlib.rs`); **68** fixture `test/std` (`.vis` + `.out` viết theo thiết kế); differential **mọi** ca std interp ↔ VM: **0 lệch**; 323 ca sem + 272/272 vẫn xanh |

**Hàm**: `str.{split,join,trim,upper,lower,starts_with,ends_with,contains,repeat,replace}` ·
`list.{sort,sort_by,reverse,unique,concat,chunk,take,drop}` · `map.{keys,values,merge,get_or,remove}` ·
`path.{join,basename,dirname,ext,stem,is_abs}` · `time.{now_ms,monotonic_ms}` · `json.{encode,encode_pretty}`.

**Quyết định**: `sort_by` = **closure khoá**, tăng dần, **ổn định** (có ca chứng minh); `--desc` = nợ VS1b.
`std.fs` **không** làm (ST-27); `json.decode` **không** export ⇒ `VS0301` "chưa hỗ trợ (VS1b)".

## ST-25 — module đa tệp: `use ./util`, cấm vòng, `pub`, hash closure (2026-11-13)

| # | Việc | Kết quả |
|---|---|---|
| 1 | Cú pháp | `dotted_path` nhận **đường dẫn tương đối** `./util`, `./a/b` (dấu `.`/`/` trong node `DottedPath`); `use ./util` trước đó là **VS0201** |
| 2 | Bộ nạp `crates/vis-cli/src/modules.rs` | DFS **hậu duyệt** theo thứ tự `use` (tất định), mỗi tệp một lần, tệp chính cuối; **cấm vòng** `VS0702`; `use` trùng/mơ hồ `VS0703`; thiếu tệp / `std.*` (ST-26) / `pkg.*` (VS2) ⇒ `VS0701` |
| 3 | Mô hình xuất tên | **chỉ `pub`** (mặc định riêng tư), kiểm ở **tầng giải tên** (`lower::resolve` + `duoc_thay`) — **không** đụng `tyck`; `Symbols` thêm `mod_of`/`pub_of`/`mods` (song song với `syms`) |
| 4 | Hợp nhất | `lower_into(&mut Symbols, &mut TypeTable, …)` — item ghép theo hậu duyệt, `RecId`/`EnumId` **ổn định**; kiểu `pub` của phụ thuộc nạp vào bảng tên tệp `use` nó |
| 5 | Chẩn đoán | in **đúng tên tệp** (`util.vis:3:18: error VS0412: …`); tyck diag được tính lại `dòng:cột` theo tệp được gán |
| 6 | Hash closure | `cache::khoa_multi(files, mode)` = FNV-1a×4 trên (phiên bản compiler/VBC1, mode, **từng tệp** rel+text theo hậu duyệt); sửa phụ thuộc ⇒ khoá **đổi** (ca test chứng minh: 2 khoá `.vbc`) |
| 7 | CLI | `vis run` tự nạp module; `--dump-deps` in cây; `vis check` TAP `not ok - <path> (util.vis:3:18)`; cache áp cho `--vm` |
| 8 | Sổ mã | `MODULE_CODES` `07xx` (0701/0702/0703 vi/en/ja + snapshot 9 ca) ⇒ `BOOKS` 7, `all_codes()` **28**, `next_block()` **801**; 4 kỳ vọng cũ sửa **kèm lý do** |
| 9 | Test | **335** workspace (318 + 16 `modules.rs` + 1 snapshot); 323 ca sem; differential **272/272** |

**Hồi quy bắt được trong lúc làm** (đã sửa, không nới test): `vis check` phải giữ ngữ nghĩa cũ (script chỉ
cú pháp; tệp phụ luôn báo), `NamePolicy` **Permissive** cho tệp ngoài module (giữ `VS0501` runtime), tyck diag
phải tính lại `dòng:cột`. Ghi trong `JOURNAL/st25-red-module.log`.

**Nợ đã biết (VS1b)**: `pub` cho `let`/`const` **chưa** có; chẩn đoán **runtime** ở hàm thuộc tệp phụ vẫn tính
`dòng:cột` theo văn bản **tệp chính** (HIR chưa mang `file` của item); `use` không re-export; `..` được phép
nhưng chưa có chốt "không ra ngoài cây dự án".

## ST-24 — báo cáo **R3**, chốt VS1-khung (2026-11-13)

| # | Việc | Kết quả |
|---|---|---|
| 1 | `agent/reports/R3-viscript.json` | **đúng 11 khoá**, `round = 3`, `head = c427d64` (QUEUE@head có **27 hàng** 6 cột, `report-check` PASS) |
| 2 | `agent/reports/R3-viscript.md` | đủ `## 1.` … `## 6.`, > 800 ký tự; bảng task **ST-18..ST-24 (+ ST-20b)** với hash thật |
| 3 | Số đo **chạy lại** (03:03–03:20Z) | **318** test (20 diag + 96 syntax + 18 fmt + 65 types + 24 interp + 45 bytecode + 8 vm + 42 cli); **323 ca/17 nhóm** (309 `.vis` + 14 `.vish`, 51 `.err`); **272/272** differential; `DIFF tong=100000 loi_tinh=0 chay=100000 lech=0 giay=192.6 toc_do=519/s`; fuzz **2.311.244 vòng / 0 vi phạm / 602,3 s**; **53 tag** VBC1 (0..52 — *không phải 51*, đã ghi thật); verifier **13 biến thể**; ấm **8,51/9,92/8,64 ms** trung vị (p95 9,14/10,33/10,26) vs nguội **9,16/10,96/9,31 ms** |
| 4 | `report-check` | **PASS (3 báo cáo, 27 hàng QUEUE phủ đủ)** — lưu ý: `ST-20b` **chỉ ở MD** vì hàng QUEUE của nó **5 cột** (thiếu DoD) |
| 5 | Cổng | `gate --fast` PASS (03:18:11Z) · `nightly --fuzz-minutes 1` PASS (03:19:56Z) |

**Đối chiếu §19 VS1**: **đạt** differential 100k 0 lệch + khởi động ấm < 20 ms; **chưa** generic/trait/module/
`vis-std`/`vis-host-posix`/`vis run|check|fmt|test` đầy đủ ⇒ **VS1b**. Đề xuất sửa `AGENTS.md` §8,
`viscript/AGENTS.md` §3/§5, hàng `ST-20b`/`ST-24` trong `QUEUE.md` ghi ở §6 của báo cáo.

## ST-22 — differential trên chương trình **sinh tự động**: 100.000 chương trình, **0 lệch** (2026-11-13)

| # | Việc | Kết quả |
|---|---|---|
| 1 | Bộ sinh trong `crates/vis-vm/tests/diff_100k.rs` | subset VS0, **tất định** theo seed (LCG), ≤ **200 token**, không vòng lặp vô hạn, đệ quy có base (`fact(3)`) |
| 2 | Mỗi chương trình | `parse → lower → tyck → codegen → verify`; tĩnh sạch ⇒ chạy **interp** và **VM**, so `stdout` + `exit` + `(mã, dòng, cột)`; codegen/verify lỗi cũng tính **lệch** |
| 3 | **Chốt** | **100.000** chương trình · **0** lỗi tĩnh · **100.000** chạy · **0 lệch** · 189,2 s · ~528 chương trình/s · seed 3523158054 |
| 4 | Lựa chọn chạy | Cổng chạy **1.000** (~1,3 s); **100.000** qua `bin/vis-diff` (đã ghi log thật) — **không** nói cổng chạy 100k |

**Bug thật do differential bắt** (đã sửa): `IterInit` dùng span của **cả câu `for`** thay vì span của **biểu thức lặp** ⇒ chẩn đoán runtime lệch `cột` (6:10 vs 6:1) giữa interp và VM.

Log: `JOURNAL/st22-diff-100k.log`.

## ST-23 — cache bytecode theo hash closure + khởi động ấm (2026-11-13)

| # | Việc | Kết quả |
|---|---|---|
| 1 | `crates/vis-cli/src/cache.rs` | `~/.cache/vifast/vis/<hash>.vbc` (`$XDG_CACHE_HOME`), hash **FNV-1a×4** trên (phiên bản compiler, phiên bản VBC1, chế độ, **toàn văn nguồn**); nạp **luôn** qua `load_verified`; hỏng/lệch ⇒ biên dịch lại + ghi lại; ghi tệp tạm + `rename` |
| 2 | Cờ | `vis run --vm` dùng cache **mặc định**, `--no-cache` tắt; `vis cache --path|--clear`; cache hit **bỏ qua frontend** |
| 3 | Đo (220 mẫu) | ấm **8,96 ms** trung vị / **9,61 ms** p95 (`co-ban`), 10,57/11,17 (`fib`), 9,04/9,65 (`or-hai`); nguội 9,63–11,37 ms ⇒ **giảm 7–8 %** |
| 4 | Mục tiêu §19 `< 20 ms [ƯL]` | **đạt**, nhưng lợi ích **nhỏ** vì chi phối là **khởi động tiến trình (~8 ms)**, không phải frontend (~0,7 ms) |

**Lệch có chủ ý (báo Agent 0)**: `vis run` **mặc định vẫn interp** (đường tham chiếu); cache áp cho đường **`--vm`**.
Muốn đổi mặc định sang VM (và hưởng cache ở mọi lần chạy) thì cần Agent 0 chốt, vì nó đổi `steps` (đơn vị lệnh
vs bước ngữ nghĩa) trong output TAP.

Log: `JOURNAL/st23-warm.log`.

## ST-21 — verifier VBC1 + fuzz không panic (2026-11-13)

| # | Việc | Kết quả |
|---|---|---|
| 1 | `crates/vis-bytecode/src/verify.rs` | `verify(&Module)`, `need_stack(&[Instr], func)` (**một** hàm mô phỏng dùng chung codegen + verifier), `captured_used`, `load_verified(&[u8]) -> Result<Module, LoadError>` (ST-23 dùng thẳng), `LoadError{Decode,Verify}` |
| 2 | **13 biến thể** `VerifyError` | `BadVersion`, `BadEntry`, `BadConstIdx{func,pc,idx}`, `BadFuncIdx{…}`, `BadJump{func,pc,off}`, `StackUnderflow{func,pc}`, `StackMismatch{func,pc,a,b}`, `NeedStack{func,declared,need}`, `EmptyCode`, `FallsOffEnd`, `SpanMismatch`, `BadArity`, `ClosureFree{func,pc,nfree,used}`, `LocalOverflow{func,pc,slot,nlocals}` |
| 3 | **`max_stack`**: giữ trường, verifier **tính lại** (`need`) và **từ chối nếu khai nhỏ hơn** | Codegen ghi `max_stack` bằng **chính** `need_stack` ⇒ hết khoảng chênh; lệch = bug codegen (báo, không nới) |
| 4 | **Bắt buộc trên mọi đường nạp** | `Vm::run` **verify trước** (sai ⇒ `VS0501` "bytecode hỏng", không chạy); `vis run --vm` đi qua đó; cache ST-23 dùng `load_verified` |
| 5 | **Fuzz** | `crates/vis-vm/tests/fuzz_mutate.rs`: 272 module nguồn × (bit-flip/byte-flip/cắt cụt/chèn), `catch_unwind`, bất biến *không panic · kết thúc · độ sâu ≤ `need`* |
| 6 | Test | `tests/verify.rs` **26** ca (hợp lệ + từng biến thể + `load_verified` + `need_stack`); workspace **316** test; differential **272/272**; sem **323** ca xanh |

**Verifier bắt 4 bug thật của codegen** (đã sửa, không nới): `?.` nhánh `none` thiếu `Pop`; `IterInit`
khai 1→1 nhưng thực 1→0; `match` biểu thức nhánh cuối thiếu `Pop` subject; `test` cho `Wild`/`Bind`
không tiêu thụ giá trị. Ghi trong `JOURNAL/st21-red-verify.log`.

## ST-20b — differential interp ↔ VM về **0 lệch** (2026-11-13)

Sửa **đúng 19 ca** trong `KNOWN_LECH`, không cắt ca, không nới ngưỡng:

| # | Sửa gì | Ca |
|---|---|---|
| 1 | `test` pattern **lồng**: test con trước (mỗi con một `Dup`), test ngoài sau, gộp `BoolAnd` | 10 ca (`opt-nested`, `res-nested`, `result/*`, `match-*`) |
| 2 | `TestCtor` hiểu biến thể **dựng sẵn**: `some/none/ok/err` trên `Value::Opt`/`Res` (không phân biệt hoa/thường) | `match-some/none`, `result/*` |
| 3 | `TestTuple/TestList` nhận `()` là tuple rỗng | `match-unit-tra` |
| 4 | `?.` phẳng: `MakeSome` **không** bọc hai lớp `T?` | `hoi-cham-co` |
| 5 | `JumpIfNone/JumpIfSome` **luôn pop** (1→0) — trước đây chỉ pop khi nhảy ⇒ lệch ngăn xếp | `?.`/`??` + nhiều ca |
| 6 | gọi động trong closure: biến **bắt** phải `LoadCaptured` (không `LoadLocal`) | `closure-long` |
| 7 | tham số `--tên`: xếp đối số theo **thứ tự tham số**, chỗ trống `PushUnit` (52) | `mac-dinh-hai` |
| 8 | `let` có kiểu `T?`: tự nâng `T → T?` (`MakeSome`) khi init chưa phải `Opt` | `match-some` |
| 9 | quy ước nhảy **thống nhất**: đích = `chỉ_số_lệnh + offset` (sửa `decode` cho khớp VM/codegen) | toàn bộ |
| 10 | tách `codegen/pattern.rs` (module con) để giữ R1 ≤ 1.500 dòng | — |

**Kết quả**: **272/272 khớp** (`stdout` + `exit` + `(mã, dòng, cột)`); `KNOWN_LECH` = **rỗng** và test
khoá `so_biet == 0` + `khop == 272` ⇒ không ai thêm nợ lại được. **289** test xanh; gate PASS.

## ST-20 — `vis-vm`: máy ảo VBC1, ngữ nghĩa trùng interp (2026-11-13)

| # | Việc | Kết quả |
|---|---|---|
| 1 | **Tách ngữ nghĩa dùng chung** trong `vis-interp` | `builtin::{Host, BuiltinSig, SIGS, sig, builtin_name, bin, un, call, method}`; `value::{show, eq, cmp, iter_items, lit_value, exit_code}`; `interp::{Trap, Abort, R, pos, lit_value}` **công khai**; `ops::{bin, un}` là bí danh. `Interp::{bin,un,call_builtin,call_method,iter_items}` nay chỉ là **vỏ mỏng**; **284 test + 323 ca xanh y nguyên** sau refactor |
| 2 | Crate `crates/vis-vm` | `vm.rs` (fetch–decode–execute trên `Module`, khung `locals/captured/pc`, ngăn xếp **động** `Vec<Value>`, `iters` cho `for`, call/closure/pattern/interp/with/set), `dump.rs` (`--dump-bytecode`, histogram tag) |
| 3 | Cùng ngữ nghĩa | Cùng `Show`/builtin/bảng toán tử ⇒ cùng `stdout`; cùng mã+`dòng:cột` (từ `spans[i]` của module); cap khung **2.000** (VD-49) + cap **10^7 lệnh**, cùng mã `VS0501`; **không** `unsafe` |
| 4 | `vis run --vm` + `--dump-bytecode` | In kết quả/diagnostics **y hệt** interp; `steps` theo **đơn vị lệnh** (khác interp — đã ghi rõ) |
| 5 | **Differential** | `crates/vis-vm/tests/vs_interp.rs`: **253/272 khớp**; **19 lệch** khoá trong `KNOWN_LECH` (đúng danh sách: lệch thêm ⇒ đỏ, sửa mà không cập nhật ⇒ đỏ) |
| 6 | Lệnh mới trong VBC1 (vẫn **version 1** vì chưa ghi cache) | `Unwrap(45)`, `CallValue(46)`, `UnwrapOpt(47)`, `IsUnit(48)`, `BoolOr(49)`, `BoolAnd(50)` — tất cả để hoàn thiện codegen/pattern, **không** đổi ngữ nghĩa |

**19 ca lệch (ST-20b)**: pattern **lồng** (`some(some(x))`, `Ok((a,b))`, `A(1..3)`) 10 ca; named-arg đổi
thứ tự 1 ca (`ham/mac-dinh-hai`); `Show` của `none` + `?.` trên `none` lồng 3 ca; `match` trả `()` /
guard-or 4 ca; gọi kết quả biểu thức 1 ca (`ham/closure-long`).

**Đỏ trước** `JOURNAL/st20-red-vm.log`: VM thô **122 lệch** (bằng chứng khuyết điểm ST-19: codegen `match`
chỉ *kiểm* mà **không** bind pattern) → 88 → … → **253 khớp / 19 lệch**; hash log khớp bản commit.

## ST-19 — `vis-bytecode`: VBC1 + sinh mã từ HIR (2026-11-13)

| # | Việc | Kết quả |
|---|---|---|
| 1 | Crate mới `crates/vis-bytecode` (lib `vis_bytecode`; dep `vis-types`/`vis-syntax`/`vis-diag`) | `format.rs` (magic `VBC1`, `u16` version=1, **hash nguồn 32 byte**, const pool, bảng hàm), `encode.rs`/`decode.rs` (little-endian, độ dài tường minh, `DecodeError` **9 biến thể**), `codegen.rs` (HIR ⇒ `Module`) |
| 2 | Tập lệnh `Instr` | **45 tag** (hằng/biến/`LoadCaptured`, `Bin`/`Un` mang **chính** `vis_types::BinOp`/`UnOp`, `Call`/`CallBuiltin`/`CallMethod`, nhảy 6 loại, `Return`/`Halt`, dựng list/set/tuple/map/record/enum/opt/result/closure, `Field`/`Index`/`Set`/`With`/`Interp`, test pattern 6 loại + `BindLocal`, `Try`, `Dup`); mỗi lệnh có **span byte** (bảng song song), mỗi hàm có `arity`/`nparams`/`nlocals`/`max_stack` |
| 3 | **`match` = chuỗi test + nhảy** | VM **không** biết pattern; `Rest` không đầu/cuối ⇒ `CodegenError::Unsupported` |
| 4 | **Round-trip** | `decode(encode(m)) == m`, `encode(decode(encode(m))) == encode(m)`, `total_code() > 0`, `is_well_formed()` trên **272 ca sinh mã** (323 ca − 51 ca cố ý lỗi tĩnh) + **27 golden biểu thức hợp lệ** (30 − 3 ca cố ý lỗi) |
| 5 | Đo được | **284** test workspace (265 + 19 mới: 3 roundtrip + 11 decode_neg + 5 codegen); **3.827 lệnh** cho 272 ca (TB **14,1** lệnh/ca, max **46**), **36** tag đã dùng; sổ `06xx` giữ nguyên |

**Hash nguồn**: **FNV-1a 64 bit × 4 seed** ⇒ 32 byte (không thêm dep) — **không phải SHA-256**; ghi rõ trong `format.rs` + **lỗ hổng đã biết**: khi làm consent/quyền (§11.3) **phải** thay bằng SHA-256 thật (`sha2`, cần duyệt allowlist); **cache luôn phải qua verifier** (ST-21) — hàng rào bù (ghi trong doc của `decode`).

**Chưa làm (đúng phạm vi)**: VM + verifier (ST-20/ST-21); `max_stack` hiện là **ước lượng trên** (kẹp 0) — ST-21 kiểm lại độc lập.

**Đỏ trước** `JOURNAL/st19-red-vbc.log`: 2/3 roundtrip đỏ (encode thiếu toán hạng `CallBuiltin` ⇒ `Truncated`; kiểm đích nhảy sai ⇒ `BadJump(-10)`), sau đó 272 ca `ngăn xếp âm` ⇒ sửa cách ước lượng; khai rõ `decode_neg`/`codegen` **xanh sẵn** (viết sau khi sửa encode).

## ST-18 — `cmd_lower`: `.vish` chạy được (2026-11-13)

| # | Việc | Kết quả |
|---|---|---|
| 1 | **`cmd_lower`** (`crates/vis-types/src/lower/cmd.rs`, ~430 dòng) | CST chế độ lệnh ⇒ **cùng HIR**: mỗi dòng lệnh = pipeline lồng call; `cmd` decl hạ thành `HItem::Fn` với `FnKind::Cmd` (đúng thiết kế §9.2 "cmd là hàm bình thường"); `;` ⇒ nhiều `HStmt` tuần tự |
| 2 | **Khe ống** (Agent 0 duyệt (a)1) | tham số vị trí **đầu tiên** khớp kiểu với giá trị vào **giữ chỗ trước** (đối số vị trí không chiếm mất); `echo "a\nb" \| lines \| take 1` ⇒ `take(lines(echo("a\nb")), 1)`; không có khe khớp ⇒ `VS0603` |
| 3 | **Sổ thứ sáu `06xx`** | `VS0601` lệnh không tồn tại · `VS0602` sai đối số/kiểu · `VS0603` pipeline không nối · **`VS0604`** chưa hỗ trợ ở VS0 (`&`, `^…`, `$x`/`$env`) — vi/en/ja; `BOOKS` **6**, `all_codes()` = **25**, `next_block()` ⇒ **701** |
| 4 | **3 lệnh builtin** | `echo(a, b="", c="", --upper: bool=false) -> str` · `lines(s) -> list[str]` · `take(xs, n) -> list[str]`; **bỏ `pwd`** (không FS); variadic `...rest` **chưa có** ⇒ hạn chế đã biết |
| 5 | **CLI** | `vis check *.vish` = parse + **cmd_lower** + tyck (bỏ "parse-only"); `vis run *.vish` **chạy** qua interp (bỏ `Bail out!`); chẩn đoán `<tệp>:<dòng>:<cột>` thật |
| 6 | **Ca** | 14 ca `.vish` trong `test/sem/vish/` (pipeline 2–3 stage, `--flag`, `;`, VS0601/0602/0603/0604) + `cmd_lower.rs` (9 ca) + `cmd_run.rs` (9 ca) ⇒ `sem` **323 ca/17 nhóm** (309 + 14), workspace **265** test |

**Đỏ trước** `JOURNAL/st18-red-cmd.log`: 8/9 ca `cmd_run` đỏ trên binary cũ (`Bail out!`), 2 ca `cmd_lower`
hỏng trên code cũ (`LowerError::CommandMode`), 14 ca `.vish` đỏ; hash log khớp bản commit.

**5 kỳ vọng cũ phải sửa (kèm lý do trong tệp)**: `vis-diag` `all_codes` 21→25/`lookup(601)`/`next_block`
601→701; `vis-diag/tests/messages.rs` `lookup(601)`; `vis-types/tests/names.rs` 8→**11** tên dựng sẵn
(Agent 0 duyệt 3 lệnh); `vis-types/tests/hir.rs` `CommandMode` ⇒ hạ thật; `vis-cli/tests/explain.rs`
`VS0601` exit 0 / `VS0701` exit 2; `vis-cli/tests/check.rs` (lệnh ngoài bảng VS0 nay `not ok` + VS0601).

## ST-17 — báo cáo **R2** + chốt VS0 (2026-11-13)

| # | Việc | Kết quả |
|---|---|---|
| 1 | `agent/reports/R2-viscript.md` + `.json` (+ `.license`) | JSON **đúng 11 khoá** (`project=viscript`, `round=2`), `head = 0ce1adc` (QUEUE đủ 20 hàng; mọi task trước ST-17 `DONE`); MD đủ `## 1.`..`## 6.` |
| 2 | Bảng task + quyết định + lỗ hổng | **20 hàng** khớp `queue_at(0ce1adc)` (Agent 0 đã thêm cột DoD cho ST-03/ST-15b tại `0ce1adc`); VD-42..VD-51 mỗi cái 1 dòng; **14 lỗ hổng** đã biết liệt kê đủ |
| 3 | Số đo tái lập | 247 test (96+56+24+33+20+18); 309 ca sem/16 nhóm; 21 mã/5 sổ; fuzz 1 giờ **182.328.926 ca / 4.145.431.263 token / 0 vi phạm**; 24 khối tài liệu (17+7); nightly PASS — mỗi số ghi **lệnh + giờ chạy** |
| 4 | Máy kiểm báo cáo | `report-check: PASS (2 báo cáo, 20 hàng QUEUE phủ đủ)` — chạy **trong** `gate --fast` |
| 5 | **Chốt VS0 (§19)** | ≥300 ca `.vis` ✓ 309; fuzz parser 1 giờ không panic ✓ 0 vi phạm; vòng tròn fmt ✓ trong `nightly`; 100 % fixture tài liệu parse ✓ 24/24 |

**Đề xuất cho Agent 0** (tôi không sửa): `viscript/AGENTS.md` thêm dòng `nightly` + dải `VD-01..VD-51`;
`AGENTS.md` §8 nhắc vòng 2; `viscript/agent/QUEUE.md` thêm cột cho hàng **ST-03** và **ST-15b**.

## ST-16 — fuzz parser 1 giờ + `vis-dev nightly` (2026-11-13)

| # | Việc | Kết quả |
|---|---|---|
| 1 | **Fuzz parser** | `crates/vis-syntax/src/bin/vis_parse_fuzz.rs` (bin `vis-parse-fuzz`, PRNG xorshift tất định, 3 nguồn: ngẫu nhiên / đột biến corpus / soup token). Bất biến mỗi ca: **không panic** (`catch_unwind`), **CST lossless** (`cst.render(text) == text` khi parse sạch), **span hợp lệ** (biên ký tự, `end ≤ len`), lỗi ⇒ có chẩn đoán |
| 2 | **Chạy 1 giờ thật** | `JOURNAL/st16-fuzz-parse.log`: **182.328.926 ca / 4.145.431.263 token**, 50.647 ca/s, 3.600 s, **0 vi phạm** (chạy lại 60 s trên binary sau `fmt`: 2.716.321 ca, 0 vi phạm) |
| 3 | **`vis-dev nightly`** | `bin/vis-dev`: `nightly [--fuzz-minutes N]` = `gate --fast` (fmt/clippy/luật/build/L0 test) + **fuzz parser N phút** + **vòng tròn fmt** (fmt bất động trên 309 ca ngữ nghĩa, bỏ qua 45 ca cố ý hỏng cú pháp) ⇒ dòng cuối `vis-dev nightly: PASS`, exit 0. Mặc định N=2 phút; `--fuzz-minutes 60` cho chế độ đêm. **Không** nới/bỏ bước nào của `gate --fast` |
| 4 | Tài liệu | mục này + JOURNAL §ST-16 |

**Đo được**: `vis-dev nightly --fuzz-minutes 1` → **PASS** (L0 test OK, fuzz 2,7 triệu ca/0 vi phạm,
vòng tròn fmt 309 tệp bất động); `gate --fast` PASS — 2026-11-13T00:56:44Z; fmt/clippy sạch.

**Ghi chú**: `viscript/AGENTS.md` **thuộc Agent 0** — tôi **không** sửa; cần cập nhật dòng lệnh `nightly`
thì Agent 0 sửa (nội dung gợi ý: `./vis-dev nightly [--fuzz-minutes N]` = gate --fast + fuzz parser + vòng tròn fmt).

## ST-15b — 4 lệch thiết kế đã sửa (2026-11-13)

| # | Lệch | Sửa | Ca khoá (`test/sem`) |
|---|---|---|---|
| 1 | `list.push` không ghi lại lvalue (§7.1 CoW) | `eval_call`: `push` trên `Field{base: Var}` **`var`** ⇒ ghi kết quả về `env[sym]`; trên `let`/biểu thức tạm giữ "trả list mới" (ghi JOURNAL — không silently no-op) | `list/push` `[1, 2]`, `list/push-cow` (`a`=`[1]`, `b`=`[1, 4]`), `list/push-let` |
| 2 | Closure thoát hàm mất tham số đã bắt (§3.4/§7.3) | `HExpr::Lambda` **chụp snapshot env** tại điểm tạo (chọn snapshot thay vì chuỗi frame — ghi JOURNAL: biến `var` cập nhật **sau** khi tạo closure thì closure vẫn thấy giá trị lúc tạo) | `ham/closure-thoat` ⇒ `15` |
| 3 | `{x, ..}` bị `VS0402` oan (§4.3) | `tyck/pat.rs`: nhánh **có `..`** phủ mọi trường còn lại; **thiếu** trường mà **không** `..` vẫn `VS0402` | `match-nang-cao/record-mot` ⇒ `1`; `record-thieu` ⇒ `VS0402` |
| 4 | Pattern bản ghi **trần** không binding (HIR thiếu `sym`) | `lower/pat.rs`: trường trần hạ thành `HPat::Bind` **có `sym`** (hai chỗ: trường cuối + trường bị chặn bởi trường sau) | `match-nang-cao/record-pattern` ⇒ `3` |

**Sửa 1 kỳ vọng cũ kèm lý do** (bắt buộc): `crates/vis-types/tests/exhaustive.rs` ca `{name, ..}` từ
`!VS0402` → **`ok(int)`** (bản cũ khoá hành vi **trái §4.3**); thêm ca `{name}` (thiếu trường, không `..`)
⇒ `!VS0402`. Ghi rõ trong tệp + JOURNAL.

**Đỏ trước** (`JOURNAL/st15b-red-sem.log`): 5/7 ca mới đỏ (4 mục); 2 ca *xanh sẵn* khai rõ (`list/push-let`,
`record-thieu`). Test `sem` khoá **309 ca** (302 + 7).

**Đo được**: **247** test workspace; `gate --fast` PASS — 2026-11-13T00:56:44Z; fmt/clippy sạch.

## ST-15 — bộ ngữ nghĩa **302 ca / 16 nhóm** + cap độ sâu (2026-11-13)

| # | Việc | Kết quả |
|---|---|---|
| 1 | Khoá cứng `crates/vis-cli/tests/sem.rs` | **302 ca**, **16 nhóm**, mỗi nhóm ≥ 6; thêm **test chống bỏ ca** (đếm `.vis` trên đĩa = số ca chạy = 302); `.out`/`.err` được bóc dòng SPDX đầu trước khi so **byte** |
| 2 | 202 ca mới | 12 nhóm cũ lên **18 ca** mỗi nhóm (216) + 4 nhóm mới × 21 = **84**: `so-hoc-mep`, `match-nang-cao`, `long-du-lieu`, `chuoi-phuong-thuc` ⇒ **302 ca/16 nhóm** |
| 3 | **Cap độ sâu gọi** | `MAX_CALL_DEPTH` = **2.000** khung (đo: 10^4 **tràn stack** kể cả 256 MiB; 1.900 tầng chạy được, vượt cap ⇒ `VS0501`); `vis run` chạy interp trong **luồng 256 MiB** để đệ quy sâu không chết process. Ca: `ham/de-quy-1000` (in `1000`) + `ham/de-quy-vo-han` (⇒ `VS0501`, không crash) |
| 4 | Sửa nhỏ phát hiện khi viết ca | `()` pattern khớp `Value::Unit` (trước đây `match ()` không nhánh nào khớp ⇒ `VS0501` sai) |

**Đỏ trước** (`JOURNAL/st15-red-sem.log`): đổi khoá 100 → 302 **trước** khi thêm ca ⇒ runner ĐỎ
("phải đúng 300 ca, thật 100"). Trên harness cuối đã chụp lại bằng cách **nâng khoá tạm lên 400** ⇒
"phải đúng 400 ca, thật 302" — chứng minh hàng rào đếm ca; sau đó đặt lại **302**.

**Lệch thiết kế phát hiện (KHÔNG ghi kỳ vọng theo hiện thực — đã bỏ ca + báo Agent 0):**
1. **`list.push` trên `var` không đổi bản thân list** — thiết kế §7.1 ("ngữ nghĩa giá trị + copy-on-write":
   `var b = a; b.push(4)` ⇒ `b` đổi, `a` không) nhưng interp trả list mới, không ghi lại lvalue.
2. **Closure thoát khỏi hàm mất tham số đã bắt** — `fn make(n: int) -> fn(int) -> int: (x) => x + n;
   make(10)(5)` ⇒ `VS0501: tên 'n' chưa có giá trị`; interp dùng **một env phẳng** và khôi phục giá trị
   tham số khi hàm trả về (thiết kế §3.4/§7.3 cần bắt theo phạm vi từ vựng).
3. **Pattern bản ghi có `..` bị tyck từ chối** — `match p: {x, ..}` ⇒ `VS0402 "phải có đủ mọi trường"`;
   thiết kế §4.3 cho phép `..`.
4. **Pattern bản ghi trần không binding** — `match p: {x, y}` ⇒ `VS0501: tên 'x' chưa có giá trị`
   (lỗ hổng đã biết từ ST-13: HIR pattern bản ghi thiếu `sym`).
5. **`match`/`if` làm biểu thức trong **đối số gọi**** không parse (`print(match …)`) — giới hạn văn phạm;
   ca đã viết lại dùng `let v = match …`.

**Không phải lệch (ca của tôi sai, đã sửa):** `str + str` **không** có trong bảng §3.6 (nối chuỗi bằng nội
suy/`join`); `size + int` không hợp lệ (§3.6 chỉ `size ± size`); `float / 0.0` ⇒ `inf` theo IEEE (§3.6,
**không** panic); `char` in ra `'a'` (VD-47); `some(x)` chỉ là **pattern**, constructor là `Some(x)`;
map in khoá `str` có nháy (quyết định VD-47); `.len()` đếm **scalar** (tiếng Việt/emoji).

**Đo được**: **247** test workspace (1 test `sem` = 302 ca, chạy 2,3 s); `gate --fast` PASS —
2026-11-12T23:32:53Z; fmt/clippy sạch; tệp lớn nhất `test/sem` chỉ là fixture nhỏ.

## ST-14 — `vis run` + TAP ngữ nghĩa + 100 ca + nội suy `{expr}` (2026-11-13)

| # | Sản phẩm | Cách làm | Khoá bằng |
|---|---|---|---|
| 1 | **Nội suy `{expr}`** (lỗ hổng (a) VD-47) | `HExpr::Interp { parts: Vec<InterpPart::Str|Expr> }`; `lower` tách mảnh, hạ **từng** `{expr}` **bằng chính `Lowerer`** (chia sẻ `symbols`/`scope` ⇒ giải tên đúng ngữ cảnh) với CST của mảnh **đệm đúng `off` byte** (dòng 1 trắng + `\n` ở byte cuối) ⇒ `span` **tuyệt đối**, `dòng:cột` thật; `vis-interp` chỉ đánh giá rồi ghép — **không** parse lúc chạy | 6 ca `test/sem/chuoi/*` + `ban-ghi/nhieu-truong` |
| 2 | **`vis run <tệp.vis> [-- args…]`** | parse → lower → check → interp; stdout = chương trình, chẩn đoán ra **stderr** dạng `<tệp>:<dòng>:<cột>: error VSxxxx: …`; exit **0** xong, **1** lỗi runtime, **2** lỗi cú pháp/tên/kiểu (không chạy); `--tap` in TAP ra **stderr** (`ok 1 - <tệp> (steps=N)`); `.vish` ⇒ `Bail out!` (chế độ lệnh = VS1) | `crates/vis-cli/tests/sem.rs` (chạy **qua binary**) |
| 3 | **Harness TAP ngữ nghĩa** | `test/sem/<nhóm>/<ca>.vis` + `<ca>.out` (**byte** stdout) hoặc `<ca>.err` (mã lỗi); runner so **byte** + mã thoát, đọc `steps=N` **đòi > 0**, khoá **100 ca/12 nhóm, mỗi nhóm ≥ 6** | chính runner |
| 4 | **100 ca / 12 nhóm** | số học+đơn vị 9, chuỗi+nội suy 9, `list` 8, `map` 8, bản ghi+`with` 8, `enum`+`match` 9, `T?`/`??` 8, `Result`/`?` 8, hàm+closure+đệ quy 9, điều khiển 8, phạm vi 8, lỗi runtime/kiểu 8 | — |

**Đỏ trước (khai trung thực, `JOURNAL/st14-red-run.log`)**: hai ảnh chụp —
**Ảnh 1** (chưa có `vis run`): **100/100 ca đỏ vì *thiếu tính năng*** (không có đường chạy);
**Ảnh 2** (có `vis run`, chưa có nội suy): **8/100 đỏ** — 6 ca nội suy thật + **2 lỗi fixture của tôi**
(`has` là **từ khoá** ⇒ đổi fixture; `some(v)` trên giá trị chưa bọc ⇒ vá `bind_pat_bool`).
⇒ **92/100 ca xanh sẵn** (interp ST-13 đã làm được) — không giả vờ tất cả đỏ.

**Vá kèm**: `bind_pat_bool` cho `some(v)` khớp cả giá trị **chưa bọc** (`let x: int? = 3` giữ `3` trần ở
runtime) — lệch ngữ nghĩa giữa tyck (tự nâng `T → T?`) và interp, phát hiện bởi ca `optional/match-some`.

**Đo được**: **238** test workspace (237 + 1 `sem`); `vis-dev gate --fast` PASS — 2026-11-12T23:04:43Z;
fmt/clippy sạch; `vis run` chạy 100/100 ca, 0,66 s.

**Hoãn có ghi**: `.vish` (chế độ lệnh = VS1) ⇒ `Bail out!`; TAP in ra **stderr** (giữ stdout sạch cho
chương trình — TAP chuẩn thường ở stdout, ghi rõ để review); nội suy **không** đệ quy vào phần chữ
(`"{{x}}"` là chữ `{x}`); escape `\u{…}` chưa hỗ trợ.

## ST-12b — 7 lỗi kiểm kiểu của review độc lập (2026-11-13)

| # | Lỗi (review) | Sửa | Khoá bằng test (`tests/tyck_v2.rs`) |
|---|---|---|---|
| 1 | **BLOCKER** `?` không kiểm hàm trả gì | `HExpr::Try`: `T?` chỉ khi hàm trả `U?`, `Result` chỉ khi hàm trả `Result[…, …]`; ngoài hàm/lambda (`cur_ret = None`) cho phép; `E: Error` bỏ qua (ghi JOURNAL) | nhóm `TRY_KHONG_KHOP` (6 ca) |
| 2 | **BLOCKER** `int→float` cho **mọi** `int` | `coerce` **không** nâng nữa; thêm `coerce_e`/`coerce_lit` + `num::promote(…, l_lit, r_lit)` — chỉ nâng khi phía `int` là **literal** (§3.5) | `NANG_FLOAT` (7 ca) |
| 3 | **BLOCKER** builtin không kiểm | `kiem_call` nhận cả `sym = None` lẫn `SymKind::Builtin`; `kiem_builtin` kiểm **số lượng + kiểu** cho `print/len/str/Ok/Err/Some/None/ref` | `BUILTIN` (7 ca) |
| 4 | **MAJOR** gán `Field`/`Index` | trường bản ghi **bất biến** ⇒ `VS0412` + gợi ý `with`; `Index` chỉ gán được khi gốc là **`var`** | `GAN` (5 ca) |
| 5 | **MAJOR** `--` cho tham số vị trí | `FnSig.params` thêm cờ `named`; `--x` cho tham số vị trí ⇒ `VS0401`; trùng tên ⇒ `VS0401` | `TEN_THAM_SO` (4 ca) |
| 6 | **MAJOR** `percent` vs `float` | `is_percent_float` cho `== != < <= > >=` (§3.1/§3.6); cộng/trừ vẫn `VS0412` | `PERCENT_FLOAT` (4 ca) |
| 7 | **MAJOR** `match` trên `unit`/tuple | `unit` hữu hạn (nhánh `()`), tuple hữu hạn khi pattern cùng arity và **mọi thành phần bất khả bác** | `MATCH_HUU_HAN` (4 ca) |
| 8 | **VD-48** phạm vi | (a) kiểu ngoài VS0 (`AnyError/cap/ref/weak/Json/set/stream/Future/Task/datetime/bigint/decimal`) vẫn `VS0301` nhưng thông điệp nêu **"không có ở VS0 — vis-std/vòng 2"**; (b) `percent / percent ⇒ float`, `percent / int ⇒ percent`, `size % int`/`duration % int` giữ đơn vị | `PHEP_VS0` (5 ca) + `kieu_ngoai_vs0_bao_vs0301_ro_rang` |
| — | Kỳ vọng **sai** của ST-12 | `tests/tyck.rs`: `fn f(x:int?)->int: x?` từ `ok(int)` → **`!VS0412`** (ngoại lệ Agent 0 cho phép, có comment lý do) | — |
| — | Trùng chẩn đoán | lượt **khai báo** chạy ở chế độ `quiet` (không báo) — hết báo đôi `VS0301` cho cùng một chú thích | — |

**Đỏ trước (đúng nghĩa "fix")**: `JOURNAL/st12b-red-tyck.log` — chụp bằng cách **tạm gỡ bản vá**
(`git stash push -- crates/vis-types/src/tyck`) để chạy harness cuối trên code **trước sửa**:
**42 ca + 1 test thông điệp, 24 chỗ ĐỎ** đúng 7 hành vi sai; 10 test thì 9 đỏ, 1 xanh cố ý
(`phu_them_bieu_thuc_va_kieu` — 6 ca khoá hành vi **đúng** đã có). sha256 harness khớp bản commit
(sau `fmt`). `st12-red-tyck.log` chỉ là **bản ghi lịch sử của `273a921`**; đã **đính chính** dòng đếm
(5 nhóm tyck + 1 exhaustive = 6 đỏ, 2 xanh) mà **giữ nguyên đầu ra** gốc.

**Đo được**: **237** test workspace (227 + 10 mới: 8 test `tyck_v2` + 2 test); `./vis-dev gate --fast`
PASS — 2026-11-12T22:51:51Z; fmt/clippy sạch.

## ST-13 — kết quả (2026-11-13)

| Mục DoD (đề bài Agent 0) | Bằng chứng đo được |
|---|---|
| Crate mới `crates/vis-interp` (lib `vis_interp`, dep path, không crate ngoài) | `crates/vis-interp/{Cargo.toml,src/{lib,interp,builtin,value}.rs}` — dep `vis-types` + `vis-diag` + `vis-syntax` (path; **cần** cho `Span`/`Diagnostic`); lớn nhất `interp.rs` **1.365** dòng (trần 1.500) |
| API `run(text, hir, symbols, types, args) -> RunOutput { stdout, diagnostics, value, exit, steps }` | `lib.rs`; chạy `main` (đối số `args: list[str]`) nếu có, không thì câu lệnh cấp tệp; `exit` = 0 / int của `main` / 1 khi lỗi; `steps` để **chống xanh-giả** |
| `Value` đủ kiểu VS0 | `value.rs`: `Unit Int Float Size Duration Percent Str Bool Bytes Char Path List Map Record Enum Opt Res Range Fn Builtin`; map giữ **thứ tự chèn** (`Vec<(Value,Value)>`) |
| Ngữ nghĩa §3–§6 | `let` bất biến / `var` gán (gán `let` ⇒ `VS0412`, phòng thủ); bóng tên; `if/elif/else`; `while`; `loop/break/continue`; `for` (list + `..`/`..=` + str + map); `match` (guard, or-pattern, enum, `T?`, `Result`, bản ghi); `return`; `?` (trả sớm); `??`; `|>`; `with`; chỉ số (âm/ngoài ⇒ `VS0503` + gợi ý `last()`); gọi hàm + tham số tên `--b` + mặc định; closure bắt môi trường; đệ quy |
| Builtins + bộ phương thức **đã khoá** | `print` `len` `str` `Ok` `Err` `Some` `None` (`ref` = VS1 ⇒ `VS0501`); `list`: `len get first last push map filter each sum join`; `str`: `len upper lower split chars contains`; `map`: `len get has keys values`; ngoài tập ⇒ **`VS0501` "chưa hỗ trợ ở VS0"** (không panic) |
| Mã runtime `VS0501/0502/0503` + `span` + in `dòng:cột` | `vis-diag`: sổ thứ năm `RUNTIME_CODES` (vi/en/ja, snapshot 9 ca), `BOOKS` 5 sổ, `all_codes()` **21**; `interp.rs::pos()` tính `line/col` **thật** từ `text`+`span` (VD-46) — test `chan_doan_runtime_co_dong_cot_that` |
| **Đỏ trước** (D2-05): `tests/run.rs` ≥ 60 ca | `JOURNAL/st13-red-run.log` — **79/79 ca đỏ** (số học 21, điều khiển 12, hàm 12, tập hợp 17, kiểu 12, **giá trị đuôi 5**) khi `run()` trả khung rỗng; **1 test xanh cố ý** (`du_60_ca` chỉ đếm); chống xanh-giả: `steps > 0` cho **mọi** ca `out(...)`; sha256 harness khớp bản commit |
| **VD-47 — giá trị câu lệnh cuối** (sửa lỗ hổng (b), bắt buộc) | `exec_block`/`exec_body` + tyck `than_value`/`HStmt::{If,Match,Block}`: `if`/`match`/khối con ở **đuôi** cho giá trị; `if` **không** `else` ⇒ `()`; 5 ca mới trong `run.rs` (`gia_tri_cua_khoi_theo_vd47`) + 4 ca tyck (`tyck_methods.rs::gia_tri_cau_lenh_cuoi_vd47`) |
| Khoá bảng phương thức + `Show` | `tests/tyck_methods.rs` (8 ca: `len/get/sum/upper/keys/join/map` + `sort` ⇒ `VS0403`) và `tests/show.rs` (**5 snapshot**: scalar, size, duration, percent, record/enum/opt/Result/list/map) |
| Giữ 205 test cũ xanh | **227** test workspace (đính chính ST-13b: **32 cli + 18 fmt + 96 syntax + 20 diag + 45 types + 16 interp = 227**); **4 test cũ** sửa **kèm lý do** (VD-42 cấp `05xx`): `so_ten_03xx_khoa_theo_vd42`, `snapshot_6_ca_so_ten_03xx`, `explain_so_ten_03xx`, `explain_so_kieu_04xx` |
| Cổng xanh, không retry | `./vis-dev gate --fast: PASS` — 2026-11-12T22:40:05Z; **227** test; fmt/clippy sạch (`-D warnings`) |
| Không `unsafe`/`#[ignore]`/`unwrap` trên dữ liệu người dùng; SPDX | `grep` 0 `unsafe`/`#[ignore]`; `unwrap` chỉ ở chỗ đã chứng minh (`unwrap_or`), không trên dữ liệu người dùng; mọi tệp có SPDX (`#` cho `.toml`) |

**Chốt của Agent 0 — VD-47** (2026-11-13): (1) `size`/`duration` in **đơn vị lớn nhất chia hết**
(đúng ý thiết kế); (2) `percent` = `ratio*100` + `pct`, bỏ `.0`; (3) các dạng `record/enum/opt/Result/
char/bytes/fn` như đề xuất — **khoá bằng snapshot** (`tests/show.rs`, 5 test); (4) `percent * size|int`
**cắt về 0** — ghi JOURNAL là **quyết định**, không phải mặc định ngầm. **Bắt buộc sửa** lỗ hổng (b):
`if`/`match` câu lệnh cuối **phải cho giá trị** — đã sửa **trong `vis-interp`** (+ tyck để pipeline
nhận), khoá bằng 5 ca `run.rs` + 4 ca tyck. Tyck mở rộng bảng phương thức: **chấp nhận**, đã khoá bằng
`tyck_methods.rs`. Hoãn có ghi: (a) nội suy `{expr}` ⇒ **ST-14** (HIR phải mang phần nội suy); (c) `ref/
await/cmd`/bit ⇒ `VS0501`; (d) gán trường/phần tử ⇒ `VS0501` + gợi ý `with`; (e) pattern bản ghi trần.

**Việc phải biết:**
1. **`vis-syntax` là dep thứ ba** của `vis-interp` (ngoài `vis-types` + `vis-diag`): `Span`/`Diagnostic`
   nằm ở đó. Vẫn là path dep nội bộ, không crate ngoài.
2. **Đơn vị**: `size` = **byte**; `duration` = **nanosecond**; `percent` = **tỉ lệ 0..1** (theo §3.1/§3.6).
3. **`Show`** (VD-47, khoá ở `tests/show.rs`): `size`/`duration` đơn vị lớn nhất chia hết; `percent`
   `ratio*100`+`pct`; `record` ⇒ `User(name: An, age: 20)` (str không nháy, khoá map có nháy); `enum` ⇒
   `Circle(2)`/`Empty`; `opt` ⇒ `some(5)`/`none`; `Result` ⇒ `Ok(1)`/`Err(âm)`; `char` `'a'`; `bytes`
   `b"…"`; `path` nguyên văn; `fn` `<fn …>`; `()`.
4. **`percent * size|int` cắt về 0** — **quyết định** (VD-47 #4).
5. **4 test cũ sửa kèm lý do** (VD-42 buộc cấp `05xx`; đính chính ST-13b — thêm `so_ten_03xx_khoa_theo_vd42`
   và `explain_so_kieu_04xx`): `vis-diag/tests/messages.rs::snapshot_6_ca_so_ten_03xx`,
   `vis-cli/tests/explain.rs::{explain_so_ten_03xx, explain_so_kieu_04xx}` (giờ `VS0501` exit 0, mã chưa
   cấp là `VS0601`).

## ST-12 — kết quả (2026-11-13)

| Mục DoD (đề bài Agent 0) | Bằng chứng đo được |
|---|---|
| Crate `crates/vis-types` (không crate mới): `src/tyck/` tách tệp | `tyck/{mod.rs 650, expr.rs 792, pat.rs 404, num.rs 87}` — 4 tệp, lớn nhất 792 ≤ 1.500 |
| API `check(hir, symbols, types) -> CheckOutput { diags, types_of_expr, file_expr_types, item_types }` | `tyck/mod.rs`: `CheckOutput::is_clean`/`codes`/`value_type`; `pub use tyck::{check, CheckOutput}` trong `lib.rs` |
| Số học cùng đơn vị; `size*int`, `size/int`, `size*percent`; trộn đơn vị ⇒ `VS0412`; `%`/`/` cho 0 là runtime | `tyck/num.rs` (bảng 22 dòng) + `tests/tyck.rs` 25 ca `so_hoc_theo_don_vi` (gồm `5mb+1s`, `1+5mb`, `1/0` ⇒ ok, `5mb**2`, `50pct%2`) |
| So sánh cùng kiểu; `and/or/not` cần `bool`; `??` bỏ Option | `tyck/expr.rs::kiem_bin`; 16 ca `so_sanh_logic_va_coalesce` |
| `list/map/record/enum/T?/Result` đúng thành phần; `with`; trường lạ ⇒ `VS0403` | 17 ca `kien_truc_list_map_record_enum` (bản ghi so khớp **cấu trúc**, `with` trường lạ/sai kiểu, `.` trên `T?` ⇒ `VS0412`) |
| Gọi hàm: số đối số `VS0401` + kiểu đối số `VS0412` (kể cả tham số tên `--b` + mặc định) | 20 ca `ham_doi_so_va_optional`; `FnSig` giữ tên tham số + có mặc định ⇒ thiếu tham số **bắt buộc** mới `VS0401` |
| `match` xét hết ⇒ `VS0402` (enum/Option/`bool`; guard không tính; or-pattern tính cả hai) | `tyck/pat.rs::xet_het`; **28** ca `tests/exhaustive.rs` (enum, `bool`, `T?`, `Result`, vô hạn, bản ghi đủ trường, `..` ⇒ thiếu) |
| Tên kiểu: builtin VS0 + khai trong tệp ⇒ `Record/Enum`; lạ ⇒ `VS0301` | `tyck/mod.rs::resolve_ann`/`resolve_named`; ca `let x: Unknown = 1` ⇒ `!VS0301` |
| Sổ thứ tư `TYPE_CODES` (`VS0401/0402/0403/0412`) + 3 ngôn ngữ + snapshot; `lookup` 4 sổ; `lookup(501)=None`; `next_block()` **601** | `vis-diag`: `TYPE_CODES` + `BOOKS: [&[Code];4]` + `all_codes()` 18 mã; test `so_kieu_04xx_khoa_theo_vd42`, `snapshot_12_ca_so_kieu_04xx`; `khoi_ke_tiep_khong_tai_dung` vẫn **601**; `vis-cli` test `explain_so_kieu_04xx` |
| **Đỏ trước** (D2-05): `tests/tyck.rs` ≥ 80 ca + `tests/exhaustive.rs` ≥ 20 ca | `JOURNAL/st12-red-tyck.log` — **116/116 ca đỏ** (tyck **88**, exhaustive **28**) khi `check()` trả khung rỗng; **2 test xanh cố ý** (chỉ **đếm** ca: `du_80_ca`, `du_20_ca_match`); có **chống xanh-giả**: `ok(...)` mà `types_of_expr` rỗng ⇒ tính SAI |
| Giữ 194 test cũ xanh | **205** test workspace (194 + 11 mới: 8 tyck/exhaustive + 2 vis-diag + 1 vis-cli); 2 test cũ của ST-11 sửa **kèm lý do** (xem dưới) |
| Cổng xanh, không retry | `./vis-dev gate --fast: PASS` — 2026-11-12T22:14:32Z; **205** test; fmt/clippy sạch (`-D warnings`) |
| Không `unsafe`/`#[ignore]`/`panic` trên HIR hợp lệ; SPDX | `grep`: 0 `unsafe`/`#[ignore]`/`todo!`/`unwrap` trong `tyck/`; mọi tệp có `// SPDX-License-Identifier: MIT OR Apache-2.0` |

**Chốt của Agent 0 — VD-46** (2026-11-13): **Q1** gán cho `let`/tham số **giữ `VS0412`** ("chỉ `var` mới
gán được"; **không** cấp mã `04xx` mới, **không** mượn `05xx` — lỗi bất biến/kiểu, không phải runtime);
**Q2** tham số lambda **giữ `{error}`** ở VS0, **hoãn** suy kiểu hai chiều cho ST-14/R2, parser **không**
mở (đúng §A.3); **Q3** **hoãn** thêm `sym` vào HIR pattern bản ghi (ST-14/task riêng); **`line`/`col` = 0
chấp nhận** nhưng **ST-14 bắt buộc** in lại thật từ `span` + văn bản nguồn ở tầng `vis` (Agent 0 kiểm ở
DoD); sửa 2 test cũ **đúng** (VD-42). Danh sách "lỗ hổng đã biết" ở `JOURNAL/2026-11-12.md` §6.

**Việc phải biết:**
1. **Ba test cũ phải sửa kèm lý do** (VD-42 bắt buộc cấp `04xx`) — đính chính ST-12b: không phải **2**:
   `vis-diag/src/lib.rs::so_ten_03xx_khoa_theo_vd42` (số mã 14 → 18), `vis-diag/tests/messages.rs::snapshot_6_ca_so_ten_03xx`
   (`lookup(401)` nay `Some`) và `vis-cli/tests/explain.rs::explain_so_ten_03xx` (`VS0412` nay có trong
   `--list`, exit 0; mã **chưa** cấp là `VS0501`). Comment ghi rõ "Sửa kỳ vọng cũ kèm lý do: ST-12 đã cấp sổ kiểu".
2. **Chẩn đoán tyck có `line`/`col` = 0**: `check` chỉ nhận HIR (không có văn bản nguồn) nên không tính
   được dòng/cột; `span` vẫn đúng. Tầng `vis` (ST-14) in lại từ `span`. Ghi rõ trong `diag()`.
3. **3 khoảng trống của tầng dưới (không sửa được ở ST-12, cần quyết định)**:
   (a) **Kiểu cho tham số lambda**: parser VS0 **không nhận** `(x: int) =>` (⇒ `VS0201`), nên tham số
   lambda là `{error}`; suy kiểu lambda hai chiều thuộc ST-14 ⇒ ca `let f = (x) => x
f(1)` khoá hành vi
   **hiện tại** `ok({error})`.
   (b) **Pattern bản ghi dạng trần** `{name, age}`: HIR `HPat::Record` chỉ giữ `(String, Option<HPat>)`,
   **mất** `sym` của tên trần ⇒ tyck không gán được kiểu cho `name`/`age` (để `{error}`). Pattern có
   sub-pattern `{name: n}` thì parser cũng chưa nhận (`VS0201`). Đề xuất: ST-14 thêm `sym` vào HIR.
   (c) **Gán cho `let`/tham số** (thiết kế §4.1 cấm, **chưa có mã riêng** trong VD-42) ⇒ ST-12 báo
   **`VS0412`** với thông điệp "chỉ `var` mới gán được". Nếu muốn mã riêng thì cần cấp `05xx`.


## ST-11 — kết quả (2026-11-13)

| Mục DoD (QUEUE Khối C) | Bằng chứng đo được |
|---|---|
| Crate `vis-types` (lib `vis_types`, dep path `vis-syntax` + `vis-diag`, **không** crate ngoài) | `crates/vis-types/{Cargo.toml,src/{ty,hir,lower,lib}.rs,src/lower/{expr,pat,stmt}.rs}`; mọi tệp ≤ 1.500 dòng (lớn nhất `hir.rs` 1.112) |
| `Type` theo VD-43 (+ `float` §3.1) + tên hiển thị ổn định + `Display` | `ty.rs`: `Type` (**20** biến thể = 17 của VD-43 + `Float` (thiết kế §3.1, VD-44 #1) + `TypeRef` (VD-44 #2) + `Error`), `TypeTable` (RecDef/EnumDef/FieldDef/VariantDef), `Type::name`/`Display`/`TypeTable::show`; test `ten_hien_thi_on_dinh` |
| HIR chế độ script: `HItem`/`HStmt`/`HExpr`/`HPat` + `sexp` | `hir.rs`: 9 `HItem`, 14 `HStmt`, 23 `HExpr`, 10 `HPat`, `HCond`, `HBody`, `Lit`/`LitKind`, `BinOp`/`UnOp`/`AssignOp`; `sexp` theo VD-34 **không** nút `paren` |
| `lower(&Cst, &LexOutput) -> HirOutput { hir, diagnostics, symbols }` | `lower.rs` (+`types`, +`error` cho lỗi cấp API); hạ **permissive** (VD-44 #1) |
| Giải tên: khung + thân hàm + khối, shadowing hợp lệ, trùng ⇒ `VS0302`, tên lạ ⇒ `VS0301` | `Symbols`/`HSym`/`SymKind`; `VS0302` cho khai trùng **cùng khung**; `VS0301` cho **tên giá trị** (gán cho tên chưa khai ở mọi chế độ; mọi lần dùng ở `NamePolicy::Strict`); **tên kiểu** giữ `Type::TypeRef`, ST-11 **không** báo (VD-44 #2) |
| Điền kiểu literal + list/map/record/enum + `T?` | 12 ca literal (`tests/hir.rs::literal_duoc_dien_kieu`); `Record(RecId)` ẩn danh có trường; `Opt` cho `none`/`T?`; `Try` cho `x?` |
| **Đỏ trước** (D2-05): `tests/hir.rs` + `tests/names.rs` | `JOURNAL/st11-red-types.log` — **27 ca, 24 đỏ** (hir 7/10, names 17/17) khi thân `Lowerer::run` còn **rỗng**; **3 ca xanh cố ý** đã khai báo ngay trong header (tệp tồn tại, 22/15/7, 3 ca `!ERR` dừng ở parse); header có giờ + sha256 harness/fixture + lệnh; sha256 harness khớp bản commit |
| 15 khối ```vis đủ thân ⇒ **0 chẩn đoán** + đúng số item/khai báo | `tests/hir.rs::ha_15_khoi_tai_lieu_0_chan_doan_va_dung_so_khai_bao` (bảng `DOC_BLOCKS`: 15 dòng × (item, câu lệnh, khai báo)); khoá thêm **22/15/7** + danh sách 7 dòng loại trừ |
| 30 golden biểu thức ⇒ **sexp HIR** | `tests/hir.rs::golden_30_bieu_thuc_ha_ra_sexp_hir`: **27** ca hợp lệ so sexp (bỏ lớp `(paren …)`), **3** ca `!ERR` dừng ở parse (không lower) |
| Sổ mã `03xx` + `next_block()` ⇒ 601 | `vis-diag`: `NAME_CODES` (VS0301/VS0302, vi/en/ja), `BOOKS`, `all_codes()`, `BLOCKS_RESERVED`; `next_block()` **suy ra** từ khối đã cấp/dành trước; `lookup(401)`/`lookup(501)` = `None` (VD-44 #5) |
| `CODES` (10) + `SYNTAX_CODES` (2) **không đụng** | test cũ `so_khop_spec_lex_that` vẫn đọc thẳng SPEC-lex §7; `so_cu_phap_02xx_khoa_theo_vd35` giữ nguyên |
| Cổng xanh, không retry | `./vis-dev gate --fast: PASS` — 2026-11-12T21:52:22Z; **194** test (157 cũ + 37 mới) |
| Không `unwrap`/`todo!`/`#[ignore]`/`unsafe` trong `src/` | `grep` trên `crates/vis-types/src`: 0 (chỉ nhắc trong doc comment); test rác/7 khối loại trừ ⇒ không panic (`khong_panic_tren_7_khoi_loai_tru_va_rac`) |

**Chốt của Agent 0 = VD-45** (2026-11-13): **Q1** `NamePolicy::Permissive` là mặc định của ST-11
(`VS0301` chỉ khi **gán** cho tên chưa khai; `Strict` thành mặc định từ ST-13/14 — **đã có test riêng
cho cả 2 policy**); **Q2** đúng (kiểu khai trong tệp ⇒ `Record/Enum`, không khai ⇒ `TypeRef`);
**Q3** chấp nhận `HirOutput` 5 trường + `Type::Float`.

**Việc phải biết (ghi ở JOURNAL + báo Agent 0):**
1. `VS0301` **mặc định** (`NamePolicy::Permissive` — VD-45) chỉ báo khi **gán** cho tên chưa khai
   (kể cả gốc lvalue phức hợp `a.b = 1`); `NamePolicy::Strict` báo mọi lần dùng. Lý do: 30 golden và
   nhiều khối tài liệu dùng **tên tự do** (`a`, `b`, `xs`, `user`, `json`, `collect`, `pair`, `send`,
   `read_config`, `Config`, `s`) — nếu báo hết thì "15 khối + 30 golden hạ sạch" (VD-44 #1/#3) không
   thể đạt.
2. Tên kiểu **đã khai trong tệp** phân giải thành `Record(RecId)`/`Enum(EnumId)` (để bản ghi/enum
   literal có kiểu + `TypeTable::show` in tên); tên kiểu **không khai** ⇒ `TypeRef` (VD-44 #2, VD-45 Q2).
3. `lower` trên CST `Mode::Command` trả `LowerError::CommandMode` trong `HirOutput::error` (VD-22,
   VD-44 #6) — **không** dùng mã `VSxxxx` nào cho lỗi cấp API (VD-45 Q3 giữ nguyên).
4. Gate chạy **sau** thay đổi code/test cuối cùng (chỉ còn sửa `STATE.md`/`JOURNAL` sau đó): 2026-11-12T21:52:22Z, **194** test.
5. **Reviewer3 (DRIFT) — 2 MAJOR đã sửa + khoá bằng test**: (a) `lower_let` hạ **`init` trước** rồi mới khai
   `pat` (§4.1) — test `let_ha_gia_tri_truoc_khi_khai_ten` (file-level `let x = x + 1` ⇒ RHS `sym=None`;
   shadow trong khối ⇒ RHS frame ngoài, thân frame trong); (b) gán cho tên chưa khai chỉ báo **một**
   `VS0301` ở **cả** Permissive/Strict (cờ `suppress_name_diag` khi hạ đích) — test
   `gan_ten_la_chi_mot_vs0301_moi_dang` cho `x = 1`, `a.b = 1`, `xs[0] = 1`.

## Kế hoạch ST-04 (lập 2026-11-12, trước khi viết test đỏ)

| Bước | Việc | Tệp sẽ chạm | Đo được bằng |
|---|---|---|---|
| 1 | Baseline: `./vis-dev gate --fast` PASS trước khi sửa | — | log PASS (đã có) |
| 2 | Khung crate `vis-syntax` (đúng §14.2: lexer thuộc `vis-syntax`), `lex()` **rỗng** để test biên dịch được mà vẫn đỏ | `viscript/Cargo.toml`, `crates/vis-syntax/**` | `./vis-dev check` xanh, golden đỏ |
| 3 | 60 golden token **đỏ trước** (D2-05): chép `SPEC-lex` §8 thành `tests/lex/{script,cmd}.tok` + harness `tests/golden.rs` | `crates/vis-syntax/tests/**` | `cargo test -p vis-syntax --test golden` → 60 fail; log ở `JOURNAL/st04-red-golden.log` |
| 4 | Hiện thực lexer: token/punct/keyword, số + đơn vị, chuỗi/escape/nội suy, thụt lề NEWLINE/INDENT/DEDENT, chế độ lệnh, NFC, VS0101..VS0110 | `crates/vis-syntax/src/lex/**` | golden xanh 60/60 |
| 5 | Test phụ: NFC/NFD, INDENT/DEDENT, `\|>` nối dòng, vị trí line/col | `crates/vis-syntax/tests/{nfc,invariants,regress}.rs` | xanh (8 + 4 + 2 test) |
| 6 | Fuzz 10 phút (không `#[ignore]`) + invariant span/vị trí/không panic/NFC idempotent | `src/bin/vis_lex_fuzz.rs`, `src/lex/verify.rs` (bản ngắn trong cổng: `tests/invariants.rs`) | log `JOURNAL/st04-fuzz-10m.log` |
| 7 | `./vis-dev gate --fast` PASS, không retry | — | dòng cuối `vis-dev gate --fast: PASS` |
| 8 | Review độc lập (subagent review-only) → commit `[ST-04] …` → `pull --rebase` → push | `STATE.md`, `JOURNAL/2026-11-12.md` | `git log`, review verdict |

Ràng buộc đã đọc: SPEC-lex §8 là **nguồn kỳ vọng** (không sửa); 60 ca gồm cả ca lỗi; fuzz 10 phút là tiêu chí ST-04 (không phải cổng thường ngày); không nới ngưỡng, không `#[ignore]`.

## Ghi chú

- `ST-01` **DONE**: có `./vis-dev check` + `./vis-dev gate --fast` (fmt, clippy, R1 ≤1500 dòng, R2 cấm `unsafe`, R3 SPDX theo loại tệp, R4 `vis-host-vifast` ngoài `viscript/`, self-test **4 case âm**, build, L0 test).
- `ST-02a` **DONE** (APPROVED 2026-11-12 Agent 0). **ST-04 mở ngay**; 60 golden token là test đỏ đầu tiên của ST-04.
- `ST-02b`, `ST-10` **DONE**; `ST-04` **DONE về kỹ thuật** (2026-11-12, chờ Agent 0 lật sổ). `ST-03` (vis-diag)
  vẫn `READY` nhưng **không** phải task kế của dòng này: ưu tiên đường tới hạn là `ST-05` (parser/CST, phụ thuộc ST-04).
- Vòng 1 **không** có interpreter/kiểm kiểu; dừng ở lexer → parser/CST → `CmdCST` → `vis-fmt` → runner `--check` cú pháp.
- Commit `[ST-##]`; SPDX `#` cho `.vis .vish .vidl .toml`, `//` cho Rust; mọi `unsafe` có case âm; tệp ≤ 1.500 dòng.
- Ưu tiên hai dòng: OS trước (D2-20); dòng này chạy khi `v2/agent/QUEUE.md` không còn task `READY`, ST-02a đã DONE; ST-04 mở ngay.

## ST-04 — kết quả (2026-11-12)

| Mục DoD (QUEUE Khối C) | Bằng chứng đo được |
|---|---|
| Lexer theo `SPEC-lex` | `crates/vis-syntax/src/lex/{mod,token,lit,verify}.rs` — script + chế độ lệnh, `VS0101..VS0110` |
| **60 golden token đỏ trước** (D2-05) | 60/60 đỏ khi `lex()` còn rỗng: `JOURNAL/st04-red-golden.log` (mtime trước khi hiện thực) |
| Golden xanh | `golden: 60 ca XANH (178 token đã so)` — 48 ca script + 12 ca lệnh, đúng thứ tự §8 |
| Fuzz **10 phút** + NFC/NFD | `JOURNAL/st04-fuzz-10m.log` — 600,0 s, **18.661.376 ca / 336.316.250 token** (31.102 ca/s), `exit=0`, **0 vi phạm**, seed 20261112; `sha256` đầu log **khớp** `src/lex/{mod,lit}.rs` hiện tại (`947f2ffc…` / `817813a3…`), binary release build 14:42:40Z **trước** lượt chạy 14:42:45Z và `cargo build --release` báo `Finished` (không biên dịch lại) ⇒ log gắn đúng bản sẽ commit; `tests/nfc.rs` (8 ca); bất biến NFC idempotent nằm ở **fuzz** (`src/bin/vis_lex_fuzz.rs`), `src/lex/verify.rs` chỉ kiểm span/vị trí/INDENT-DEDENT |
| Cổng | `./vis-dev gate --fast: PASS` (không retry) |
| Test mới đã từng đỏ | golden 60/60 + hồi quy R-01 2/2 (`JOURNAL/st04-red-regress-r01.log`) |

**Phụ thuộc ngoài mới (cần Agent 0 ghi nhận):** `unicode-normalization` + `unicode-ident` (+ `tinyvec` kéo theo),
đều trong allowlist giấy phép QD §9.3 (MIT/Apache-2.0 + Unicode-3.0; tinyvec Zlib/MIT/Apache-2.0) — dùng cho
NFC toàn tệp (VD-11) và UAX #31. Hệ quả: con số "**0 phụ thuộc ngoài**" trong `reports/R1-viscript.md` (ST-10,
đã DONE) **không còn đúng**; Agent 2 không sửa báo cáo đã chốt — đề nghị Agent 0 cho ghi lại ở R2.

**Chỗ SPEC-lex bất định (đã ghi `BLOCKED.md` §3 Q1..Q5, không tự sửa SPEC, không chặn ST-04):** §8 #56/#58
lược bớt token trong danh sách kỳ vọng (golden ghi đầy đủ ⇒ siết); `char` không có escape (theo §1 + thiết kế
§2.4) nên không viết được `'\''`; §1 xếp `.. ..= $ ^ #` vào `PUNCT` trong khi §5/§6/§8 đòi kind riêng; và
**phạm vi escape của `b"…"` mâu thuẫn giữa §1/§3-dòng-54 (chỉ `\xNN`) với §3-dòng-57 + thiết kế §2.4 (đủ bộ)**
— ST-04 chọn cách đọc chặt của SPEC-lex.

## Kế hoạch ST-05 (lập 2026-11-13, trước khi viết test đỏ)

| Bước | Việc | Tệp sẽ chạm | Đo được bằng |
|---|---|---|---|
| 1 | API `parse(src, mode) -> ParseOutput { text, cst, diagnostics }` + khung CST, parser **rỗng** để test biên dịch được mà vẫn đỏ | `src/cst.rs`, `src/parse/**`, `src/lib.rs` | `tests/expr.rs` đỏ 30/30 |
| 2 | **30 golden biểu thức đỏ trước** (D2-05): harness đọc `test/expr/golden.expr` (Agent 0 soạn, VD-34) | `tests/expr.rs` | log `JOURNAL/st05-red-expr.log` |
| 3 | CST **lossless**: node + token lá, khoảng trắng = gap, chú thích gắn tường minh | `src/cst.rs` | `cst.render(text) == text` |
| 4 | Hiện thực parser Phụ lục A: item A.1, câu lệnh/khối A.2, biểu thức A.3 + ưu tiên A.5, kiểu A.5, pattern A.4 | `src/parse/{mod,expr}.rs` | golden 30/30 xanh |
| 5 | Fixture tài liệu: trích tự động 22 khối ```vis; **15 khối đủ thân** parse 0 lỗi + lossless; **7 khối loại trừ** tường minh theo **VD-36** | `tests/doc_fixtures.rs` | 22/15/7 khoá cứng |
| 6 | Mỗi tệp lỗi đúng 1 lỗi đầu (VD-35) + sổ mã cú pháp `SYNTAX_CODES` (`VS0201`/`VS0202`), `vis explain` tra cả hai sổ | `src/parse/**`, `vis-diag/src/lib.rs`, `vis-cli/src/main.rs`, `tests/errors.rs` | 3 ca `!ERR` + 13 nguồn hỏng |
| 7 | `cargo fmt` + `clippy -D warnings` + `./vis-dev gate --fast` PASS, cập nhật STATE/JOURNAL, review độc lập rồi commit | — | dòng cuối cổng |

## ST-05 — kết quả (2026-11-12T15:27Z)

| Mục DoD (QUEUE Khối C) | Bằng chứng đo được |
|---|---|
| **30 golden biểu thức đỏ trước** (D2-05) | 30/30 ĐỎ khi parser còn là khung: `JOURNAL/st05-red-expr.log` (header ghi giờ + `sha256` fixture/harness/parser + lệnh, `exit=101`) |
| Golden biểu thức xanh | `expr: 30 ca XANH (30 golden biểu thức, CST lossless từng ca)` — đúng quy ước sexp VD-34 |
| **Fixture tài liệu** | 22 khối ```vis trích tự động; **15/15 khối đủ thân** parse **0 lỗi** + lossless; **7 khối loại trừ tường minh** (VD-36): 6 khối `...` (dòng 305/432/445/610/662/737) + 1 khối 1412 (lỗi đầu `Entry(trường: …)` dòng 1429 — cú pháp ngoài §A.3; `assert` dòng 1438 là lỗ hổng thứ hai); máy kiểm đúng 22/15/7 (thêm/bớt là đỏ) |
| **Mỗi tệp lỗi đúng 1 lỗi đầu** | 3 ca `!ERR` của golden + **13** nguồn hỏng tự thêm (`tests/errors.rs`): mỗi tệp **đúng 1** chẩn đoán, mã `VS0201` (hoặc `VS0202` cho `50%`, có gợi ý `50pct`), lỗi không báo ở token đầu |
| **CST lossless** | `cst.render(&out.text) == out.text` cho 30 golden + 15 khối tài liệu (kể cả ca lỗi vẫn lossless) |
| Sổ mã cú pháp | `vis_diag::SYNTAX_CODES` = `VS0201` (chung) + `VS0202` (`50%` → `50pct`); `CODES` **không đổi** (vẫn đúng 10 mã 01xx của SPEC-lex §7); `lookup`/`lookup_str` tra cả hai sổ; `next_block()` = 301; `vis explain VS0201` chạy; `vis explain --list` liệt kê cả hai sổ |
| Cổng | `./vis-dev gate --fast: PASS` (**2026-11-12T16:00:19Z**, sau khi sửa review vòng 1–2; không retry); `cargo fmt` + `clippy -D warnings` sạch; **55** test vis-syntax (27 unit + 3 doc_fixtures + 3 errors + **5 vd37** + 1 expr + 1 golden + 4 invariants + 8 nfc + 2 regress + 1 doc-test) + 10 test cũ |

**5 chỗ văn phạm Phụ lục A mâu thuẫn với ví dụ tài liệu = VD-37 (Agent 0 chốt, commit `c45f682`); mỗi chỗ có test khoá ở `tests/vd37.rs` (gồm chiều ngược `x + = 1` không phải `+=`):**
A.1 viết `file := item*` nhưng 4 khối đủ thân có câu lệnh cấp tệp (290/342/535/549) ⇒ cấp tệp nhận
`(item | stmt)*`; A.2 `suite := simple_stmt NEWLINE` không parse được `if x: a else: b` một dòng (342/406) ⇒
khối một dòng dừng ở `else`/`elif` cùng dòng; A.3 cho `with` lấy `expr` sẽ nuốt `|>` (golden #17 đòi
`(u with age: 21) |> f`) ⇒ giá trị trường parse mức `or_expr`; A.2 có `+=` nhưng SPEC-lex §1 không có token
`+=` ⇒ parser nhận cặp **dính nhau** `+` `=` sau định danh; A.1 có `param := '...'` nhưng §1 không có token
`...` — khối dùng nó đã loại trừ theo VD-36 nên **không** hỗ trợ. Không thêm cú pháp nào khác ngoài A.1/A.2.

## Kế hoạch ST-08 (lập 2026-11-13, trước khi hiện thực)

| Bước | Việc | Tệp sẽ chạm | Đo được bằng |
|---|---|---|---|
| 1 | Fixture tĩnh (3 tệp sạch, 2 lỗi cú pháp, 1 lỗi từ vựng, 1 `.vish`) + harness TAP **đỏ trước** | `test/check/*.vis(.vish)`, `crates/vis-cli/tests/check.rs` | log đỏ 8/8 `JOURNAL/st08-red-check.log` |
| 2 | Hiện thực `vis check <tệp…>`: TAP 13, `ok`/`not ok` + **1** dòng `# VSxxxx <dòng>:<cột> <thông điệp>`, exit 0/1/2, `.vish` ⇒ `Bail out!` + 2 | `crates/vis-cli/src/main.rs`, `Cargo.toml`, `Cargo.lock` | 8 test xanh |
| 3 | Lọc tầng theo **VD-38 mục 8** (lỗi từ vựng không bị VS0201 che) + escape ký tự điều khiển để TAP là **một dòng** | `src/main.rs` | `# VS0101` cho tệp lỗi chuỗi |
| 4 | Test tài liệu: trích tự động 15 khối đủ thân (VD-36) vào thư mục tạm ⇒ tất cả `ok`, exit 0 | `tests/check.rs` | `1..15`, 15 ok |
| 5 | `cargo fmt` + `clippy -D warnings` + `./vis-dev gate --fast` PASS, cập nhật STATE/JOURNAL, review độc lập rồi commit | — | dòng cuối cổng |
| 6 | Commit `[ST-08]` (`git commit --only`) + push | `viscript/…` | `git log` |

## ST-08 — kết quả (2026-11-12T16:10Z)

| Mục DoD (QUEUE) | Bằng chứng đo được |
|---|---|
| **TAP** | mẫu thật: `TAP version 13` / `1..3` / `ok 1 - …` / `not ok 2 - …` + `# VS0201 3:8 …` / `not ok 3 - …` + `# VS0101 3:9 chuỗi chưa đóng`, exit 1 |
| **Đỏ trước** (D2-05) | 8/8 ca ĐỎ khi CLI chưa có nhánh `check`: `JOURNAL/st08-red-check.log` (header: giờ + sha256 harness + 7 fixture + lệnh, `exit=101`) |
| Chạy fixture | 3 tệp sạch ⇒ exit 0; 2 lỗi cú pháp + 1 lỗi từ vựng ⇒ exit 1, **mỗi tệp đúng 1** dòng `# VS…`; thứ tự tệp = thứ tự truyền vào |
| `.vish` | (lúc ST-08) `TAP version 13` + `Bail out! chế độ lệnh (.vish) thuộc ST-06 — chưa hỗ trợ`, **exit 2**; **ST-06 đã thay**: `.vish` giờ được `vis check` kiểm bình thường (mục ST-06 bên dưới) |
| Mã thoát | 0 sạch / 1 có tệp hỏng (kể cả không đọc được tệp: `not ok` + `# không đọc được: …`) / 2 dùng sai (không tham số, cờ lạ — usage ra **stderr**) |
| Fixture tài liệu | trích **tự động** 22 khối ```vis (loại 7 theo VD-36) ⇒ 15 tệp tạm, `1..15`, **15 ok**, exit 0 |
| Cổng | `./vis-dev gate --fast: PASS` (**2026-11-12T16:28:27Z**, sau khi sửa review vòng 1; không retry); `cargo fmt` + `clippy -D warnings` sạch; **78** test workspace (16 vis-cli: **12 check** + 4 explain; 55 vis-syntax; 7 vis-diag) |

**Ghi chú kỹ thuật:** `vis-cli` thêm phụ thuộc **path nội bộ** `vis-syntax` (không crate ngoài mới; `Cargo.lock` đổi).
Chẩn đoán TAP là **một dòng** nên CLI escape `\n`/`\r`/`\t` trong message (thông điệp parser có thể chứa token
NEWLINE thật) — **không** sửa parser (ST-05 đã DONE), chỉ sửa cách render của CLI.

## Kế hoạch ST-07 (lập 2026-11-13, trước khi hiện thực)

| Bước | Việc | Tệp sẽ chạm | Đo được bằng |
|---|---|---|---|
| 1 | Harness **đỏ trước**: `vis-fmt` (4 tiêu chí §18.4 + token + NFC) và `vis fmt` (CLI) | `crates/vis-fmt/tests/fmt.rs`, `crates/vis-cli/tests/fmt.rs`, `test/fmt/*.vis` | `JOURNAL/st07-red-fmt.log` (5/5 unit + 13/13 integration + 9/9 CLI đỏ, exit 101) |
| 2 | `vis-fmt` lib: dựng lại văn bản từ `Cst::tokens` + gap; thụt lề = 4 × độ sâu khối (`INDENT`/`DEDENT`); tab → 4; bỏ khoảng trắng cuối dòng; đúng 1 newline cuối; giữ nguyên văn trong dòng + chú thích; NFC từ `LexOutput.text` | `crates/vis-fmt/{Cargo.toml,src/lib.rs}` | 18 test `vis-fmt` xanh (5 unit + 13 integration) |
| 3 | CLI `vis fmt <tệp>` / `--write` / `--check`; lỗi cú pháp ⇒ exit 2 **không ghi**; `.vish` ⇒ `Bail out!` 2; usage 2 | `crates/vis-cli/src/main.rs`, `Cargo.toml`, `Cargo.lock` | 9 test `vis fmt` xanh |
| 4 | Kiểm **trước** 15 khối tài liệu: đều đã đúng chuẩn (không tab, không khoảng trắng cuối, thụt bội 4, LF, 1 newline cuối) ⇒ dùng làm fixture điểm bất động | `docs/…` (đọc) | test (a) 15/15 bất động |
| 5 | `cargo fmt` + `clippy -D warnings` + `cargo test --workspace` + `gate --fast`; log đỏ **chụp sau cùng** (harness+fixture cuối) | — | dòng cuối cổng |
| 6 | Review độc lập rồi commit `[ST-07]` (`git commit --only`) + push | `viscript/…` | `git log` |

## ST-07 — kết quả (2026-11-12T17:05Z)

| Tiêu chí §18.4 (VD-39) | Bằng chứng đo được |
|---|---|
| (a) **lossless** trên tệp đã đúng chuẩn | **15/15** khối tài liệu đủ thân là **điểm bất động** byte-for-byte (`diem_bat_dong_tai_lieu_15_khoi`); `test/fmt/chuan.vis` cũng bất động (kể cả khoảng trắng trước chú thích trong dòng) |
| (b) **idempotent** | `fmt(fmt(x)) == fmt(x)` trên **48** nguồn format được (15 khối + 27 golden hợp lệ + 6 fixture) + dòng chú thích/dòng nối; 3 nguồn `!ERR` của golden lỗi ổn định |
| (c) **tab → 4 dấu cách** | `tab.vis`: **phần thụt lề** không còn tab, thụt lề **bội số 4**, `parse` đầu ra **0 chẩn đoán** (tab 1 cấp → 4, 2 cấp → 8); tab **trong dòng** / **thân chú thích** giữ nguyên |
| (d) **lỗi cú pháp: không sửa** | `format("let x =\n")` ⇒ `FmtError { code: "VS0201", 3:8 }`; CLI exit **2**, stderr `# VS0201 …`, `--write` **không ghi** tệp (so byte trước/sau) |
| **token không đổi** | re-lex đầu ra ⇒ dãy `(kind, text)` của token **không phải trivia** trùng đầu vào trên cả 48 nguồn |
| **NFC** | `nfd.vis` (NFD) ⇒ đầu ra **NFC**, bằng `LexOutput.text`; NFC là điểm bất động |
| Cổng | `./vis-dev gate --fast: PASS` (**2026-11-12T17:20:07Z**, sau khi sửa 3 MAJOR review vòng 1; không retry); `cargo fmt` + `clippy -D warnings` sạch; **105** test workspace (25 vis-cli = 12 check + 4 explain + 9 fmt; 18 vis-fmt = 5 unit + 13 fmt; 55 vis-syntax; 7 vis-diag) |
| Dòng nối (review MAJOR-1) | dòng trong ngoặc / bắt đầu `|>` **giữ** thụt lề gốc, kể cả sau chú thích cùng dòng: 3 ca tái hiện của reviewer là **điểm bất động** (`foo(\n a, # c\n b,\n)`, `foo(\n # c\n a,\n)`, `a |> b # c\n |> f`) |
| Tab trong dòng (MAJOR-2/3) | tab trong gap **cùng dòng** và trong **thân chú thích** giữ nguyên văn (`let x =\t1`, `2\t# a\tb` bất động); chỉ tab **thụt lề** đổi 4 dấu cách |
| Tệp rỗng | `fmt("") == "\n"`, `fmt("\n") == "\n"` (đúng 1 newline cuối; chưa có định nghĩa "đã chuẩn" cho tệp rỗng — ghi JOURNAL, không chặn) |
| Broken pipe | `vis fmt tệp | head` / `vis check … | head` ⇒ **không panic**, im lặng thoát 0 (ghi stdout một lần qua `write_all`, `BrokenPipe` ⇒ `process::exit(0)`); test `khong_panic_khi_pipe_dong` (tệp 20.000 dòng) |

**Ghi chú phạm vi:** chỉ chuẩn hoá **thụt lề** (VD-39); dòng nối trong ngoặc/`|>` và khoảng trắng trong dòng
được giữ nguyên văn; CRLF ⇒ LF (lexer chuẩn hoá, `fmt` ghi ra LF); tệp rỗng ⇒ đúng một `\n`.

## ST-06 — CmdCST chế độ lệnh (2 pha; 2026-11-12T17:37Z)

**Pha A (golden, Agent 0 duyệt)**: `test/cmd/golden.cmd` + `tests/cmd.rs`; 12 hàng §9.4 đúng thứ tự +
10 ca mơ hồ + 1 ca `.vish` nhiều dòng = **23 ca**; log đỏ đầu tiên **22/22 ĐỎ** (`ca01..ca22`), 2 test bất
biến (đủ 23 ca/12 hàng + lossless) xanh. **VD-40** (5 chốt) do Agent 0 ban hành sau khi duyệt.

**Pha B (hiện thực)**: `src/parse/cmd.rs` (parser CmdCST chưa định kiểu) + `Cst::sexp_cmd()` + node
`CmdShell/CmdItems/CmdBg/CmdPipe/CmdStage/CmdGroup/CmdWord/CmdFlag/CmdStr/CmdVar/CmdEnv/CmdClosure/CmdLet`;
biểu thức trong `( … )` và thân closure **cắt theo span** rồi parse lại `Mode::Script`.

| Mục | Bằng chứng |
|---|---|
| Golden | **33/33 ca xanh** (`cargo test -p vis-syntax --test cmd` = 33 ca + 2 bất biến; sau review vòng 1 thêm #24..#33) |
| `.vish` vào `vis check` | bỏ `Bail out!`; `vis check` trên 2 khối ```vish-session trích tự động ⇒ **exit 0**, 2 `ok`; test `doc_cmd` (2 khối, 0 lỗi + lossless) |
| ST-08 cập nhật | `tests/check.rs`: `vish_bail_out_va_thoat_2` → `vish_duoc_kiem_bang_cmdcst` (13 test): `.vish` sạch (`^git`, pipeline, `let` có kiểu) ⇒ `ok`; `.vish` lỗi (`ls |>`) ⇒ `not ok` + đúng 1 `# VS…`; 2 khối ```vish-session qua CLI ⇒ 2 `ok` — **lý do ghi JOURNAL**; log ST-08 cũ (đã commit) có `sha256` của harness **trước** thay đổi, không còn khớp tệp hiện tại (lịch sử, đã khai) |
| Giữ nguyên | `vis fmt` **vẫn** từ chối `.vish` (định dạng chế độ lệnh chưa có đặc tả) — test cũ xanh; `vis explain` 4 test xanh; golden biểu thức/`CODES` không đụng |
| Đỏ trước (pha B, harness/fixture cuối) | `JOURNAL/st06-red-cmd.log`: `--test cmd` **32/33 ĐỎ** (ca22 xanh vì `VS0110` do **lexer** ST-04 bắt), `--test doc_cmd` **1/1 ĐỎ**, `--test check` **2/13 ĐỎ** (2 test `.vish` mới), 0 lỗi biên dịch; 2 test bất biến xanh |
| Cổng | `./vis-dev gate --fast: PASS` (**2026-11-12T18:10:39Z**, không retry); `cargo fmt` + `clippy -D warnings` sạch; **142** test workspace (26 vis-cli + 18 vis-fmt + 91 vis-syntax + 7 vis-diag) |

**Sửa so với pha A (có lý do, ghi JOURNAL):** #13 `>`/`<` ngoài ngoặc **không** phải lỗi mà là
`(word >)` chưa định kiểu (khối `vish-session` dùng `where size > 5mb`; nếu báo lỗi thì không thể parse
đúng 2 khối tài liệu); #22 `ls ^x` đổi thành `!ERR` (lexer ST-04 báo `VS0110` — `^` chỉ hợp lệ ở đầu
stage); thêm #23 (`.vish` nhiều dòng ⇒ `(shell …)`, `shell_let` ⇒ `(let …)`). Cả 3 đã được Agent 0
duyệt thành **VD-41** (`12b8823`).

**Review độc lập vòng 1 (`reviewer2`) — 4 MAJOR + MINOR, đã sửa hết:** (1) `let` **có kiểu** (`let x: int
= …` ⇒ `(let x int …)`, thiếu `=` ⇒ lỗi); (2) thân closure **không** dừng ở `|>` (`a |> f` ⇒ `(closure x
(|> a f))`); (3) `&` chỉ ở **cuối dòng** (`ls &; pwd` ⇒ lỗi); (4) vùng từ gộp theo span **liền kề**
(`foo[ab]`, `[ab]x`, `[ab][cd]` là **một** `(word …)`); MINOR: `flag=expr` (`--x=1` ⇒ `(flag --x 1)`),
group-stage không nhận arg (`ls | (a) b` ⇒ lỗi), `.vish` test chứng minh `Mode::Command` (13 test check +
2 khối tài liệu qua CLI), sửa câu chữ "22 ca"/Bail out. **Hoãn R2 (ghi JOURNAL):** `field_shorthand`;
`use`/`fn`/`cmd` là `shell_item` (A.7 có, 33 golden không phủ).

## ST-09 — cổng "doc-examples" + fixture VT09 (QD §7.4; 2026-11-12T18:23Z)

**Sản phẩm**

| Mục | Bằng chứng |
|---|---|
| Bước cổng mới | `./vis-dev` (`bin/vis-dev`) có `do_doc_examples` gọi lúc `check`/`gate --fast` (sau luật R1..R4) — in `--- doc-examples: OK (24 khối tài liệu: 17 xanh + 7 ngoại lệ đỏ đúng; 12 fixture QD §7.1..7.3)`; đỏ là cổng đỏ |
| Script cổng | `test/doc-examples/doc_examples.py`: `--regen` (sinh lại fixture), mặc định (kiểm fixture == khối), `--gate [--root DIR]` (chạy `vis check` trên **mọi** khối `.vis`/`.vish`) — kiểm allowlist: đúng **7** mục, mỗi mục **thật sự đỏ**, khối đỏ **không** khai báo ⇒ đỏ |
| Allowlist | `docs/viscript/doc-examples-allow.txt` (tệp mới duy nhất trong `docs/`, Agent 0 cho phép): **7 dòng** = 6 khối `...` (305/432/445/610/662/737) + khối 1412 (VD-36) |
| Fixture VT09 | `test/doc-examples/doc-*` = **24** khối tự sinh (22 ```vis + 2 ```vish-session): **17 xanh** + 7 ngoại lệ; `qd-*` = **12** fixture viết tay cho QD §7.1..7.3 (NFC/NFD, tab+VS0107, `50%`⇒VS0202, `|>` đầu dòng, `-5` số, `&` cuối dòng, `#` trong URL, `str(e.path)`, `time.now()`, `is_file()`, `1..2`, `1.`⇒VS0104) |
| Case âm | `test/fixtures/run_fixtures.sh` **1..12**: #6 khối đỏ ngoài allowlist; #7 ngoại lệ cũ đã xanh; #8 allowlist sai số 7; #9 fixture lệch byte; #10/#11 `qd-*` = 10/13; #12 binary `vis` cũ bị từ chối — mỗi ca đòi đúng thông điệp đỏ |
| Đỏ trước | `JOURNAL/st09-red-docexamples.log`: khi `bin/vis-dev` chưa có bước, ca #6 **not ok** + `vis-dev doc-examples` exit 2; `sha256` khớp `run_fixtures.sh` `cf3fffed…`, `doc_examples.py` `03c22b51…`, allowlist `cfd733a4…`, tài liệu `70da3dd5…` |
| Cổng | `./vis-dev gate --fast: PASS` (**2026-11-12T18:38:41Z**, không retry, **có** dòng `--- doc-examples: OK` **sau** `--- build: OK`); 142 test workspace giữ nguyên; fmt/clippy/R1..R4 sạch |

**Review vòng 1 (`reviewer2`) — đã sửa hết:** (MAJOR-1) `qd-*` phải **đúng 12** (không phải `>=10`), khối tài liệu **đúng 24** + **đúng 2** `vish-session`, sai là đỏ (muốn đổi phải qua Agent 0 — không tự nới); (MAJOR-2) bước `doc-examples` **dời xuống sau `--- build: OK`** và script chỉ dùng `target/debug/vis` khi binary **mới hơn mọi `.rs`** trong `crates/` (nếu không: tự `cargo build`, hoặc `--vis-bin` cũ ⇒ từ chối); (MINOR) thêm cờ `--temp-root` tường minh (miễn "đúng 7" chỉ cho cây tạm; `--root` vẫn đủ luật), in `--- doc-examples: FAIL` **một lần**, case #6 cô lập đúng luật allowlist (cây tạm có đủ 12 qd), gán mã chẩn đoán **theo từng tệp** khi parse TAP, thêm 3 ca âm (ngoại lệ cũ, allowlist sai số, fixture lệch byte) + 2 ca `qd` 10/13 + 1 ca binary cũ — tổng **12 case**.

**Sai lệch đã khai:** fixture `doc-*` mở đầu bằng dòng `SPDX` (R3 bắt buộc cho tệp `.vis/.vish` trong `viscript/`),
nên "fixture == khối" được kiểm **sau dòng SPDX** đúng byte (script báo lệch ⇒ đỏ); `qd-*` cũng vậy. Đếm khối là
**24** (thêm 2 ```vish-session) — Agent 0 yêu cầu "mọi khối ```vis + ```vish-session".

## ST-03 — chẩn đoán: cột theo scalar + `render` + bảng vi/en/ja (2026-11-12T18:45Z)

| Mục | Bằng chứng |
|---|---|
| Cột theo **Unicode scalar** | `crates/vis-syntax/tests/col_scalar.rs` (5 ca): tiếng Việt NFC **và** NFD (chuẩn hoá trước khi lex), emoji astral `🚀` = 1 scalar, emoji gia đình ZWJ = 5 scalar, tab = 4 cột, dòng theo scalar. **Lexer ST-04 đã đúng ngay từ đầu** (`bump_char` đếm `char`) ⇒ đây là test **khoá lại**, không sửa lexer (đã ghi JOURNAL) |
| `render` | `vis_diag::render(&DiagLine, Lang)`: `<tệp>:<dòng>:<cột>: <mức> <mã>: <thông điệp>` + dòng nguồn + con trỏ `^` canh **theo scalar**, tab = 4; nhãn mức vi/en/ja (`lỗi`/`error`/`エラー`). Test trong `crates/vis-diag/tests/messages.rs` |
| Bảng vi/en/ja | `Code` thêm `message_en`/`message_ja` (12 mã: 10 từ vựng + 2 cú pháp), `message_vi` **giữ nguyên** và test `so_khop_spec_lex_that` vẫn đọc §7; **20 ca message** + **20 ca `render`** snapshot (`messages.rs`, có tiếng Việt NFC/NFD, `🚀`, `👨‍👩‍👧`, tab giữa dòng, dòng dài, cột 0/ngoài biên, nhãn `lỗi`/`error`/`エラー`/`警告`) + test mọi mã đủ 3 ngôn ngữ |
| CLI | `vis explain [--lang vi\|en\|ja] <VSxxxx>` + `--list [--lang …]` (mặc định `vi`); test CLI: 3 ca mới (`explain_lang_en_va_ja`, `list_lang_in_du_ba_ngon_ngu_tung_ma`, `explain_lang_sai_hoac_thieu_gia_tri_la_loi_dung`) |
| Đỏ trước | `JOURNAL/st03-red-diag.log`: `messages` **6/7 ĐỎ** (en/ja rỗng, render rỗng), `explain` **3/7 ĐỎ** (`--lang` chưa có), 0 lỗi biên dịch; `col_scalar` **5/5 XANH ngay** (lexer đã đúng — log ghi rõ, không giấu); `sha256` khớp harness cuối (`messages` `aa9b85fd…`, `col_scalar` `213361b1…`, `explain` `d6c76400…`) |
| Cổng | `./vis-dev gate --fast: PASS` (**2026-11-12T18:57:42Z**, không retry); **157** test workspace (29 vis-cli = 13 check + 7 explain + 9 fmt; 18 vis-fmt; 96 vis-syntax; 14 vis-diag); fmt/clippy sạch |
| Sửa kèm (khai báo) | `test/doc-examples/doc_examples.py`: thêm `find_cargo()` (gate chạy trong PATH không có `cargo` — dò `$HOME/.cargo/bin/cargo`), phát hiện khi chạy cổng ST-03; hệ quả: log ST-09 đã commit (`ba777e9`) giữ `sha256` script **bản lúc đó** (`03c22b51…`) — bản ghi lịch sử, đã khai; không nới luật nào |

## Lịch sử trạng thái (mới nhất ở trên)

| Ngày | Task | Bước | HEAD | BUILD_ID | Cổng | Ghi chú |
|---|---|---|---|---|---|---|
| 2026-11-12 | ST-03 | 1/1 | `xem git log` | — | `gate --fast` PASS | Cột scalar (khoá 5 ca), `render` + con trỏ scalar, bảng vi/en/ja 12 mã + **20 ca message + 20 ca `render`** snapshot, `explain --lang/--list`; **157** test; đề nghị **DONE** |
| 2026-11-12 | ST-09 | 1/1 | `ba777e9` | — | `gate --fast` PASS | Cổng `doc-examples` (24 khối: 17 xanh + 7 ngoại lệ khai báo, allowlist 7 mục kiểm chứng), 24+12 fixture VT09, case âm #6; 142 test; đề nghị **DONE** |
| 2026-11-12 | ST-06 | 6/6 | `590e9f7` | — | `gate --fast` PASS | CmdCST chế độ lệnh: 33/33 golden, `.vish` vào `vis check` (bỏ Bail out), 2 khối vish-session 0 lỗi; 143 test workspace; đề nghị **DONE** |
| 2026-11-12 | ST-07 | 6/6 | `b149fe9` | — | `gate --fast` PASS | `vis-fmt` lossless/idempotent/tab: 15/15 khối tài liệu bất động, 48 nguồn idempotent, token+NFC+chú thích khoá; `vis fmt` 9 test; broken pipe không panic; 105 test workspace; đề nghị **DONE** |
| 2026-11-12 | ST-08 | 6/6 | `2971a72` | — | `gate --fast` PASS | `vis check` TAP: 3 sạch/2 lỗi cú pháp/1 lỗi từ vựng/1 `.vish`; exit 0/1/2; 15 khối tài liệu ok; 78 test workspace; đề nghị **DONE** |
| 2026-11-12 | ST-05 | 7/7 | `49f2539` | — | `gate --fast` PASS | parser + CST lossless: 30/30 golden biểu thức (đỏ trước), 15/15 khối đủ thân + 7 loại trừ (VD-36); 55 test; **DONE** (đã push) |
| 2026-11-12 | ST-04 | 8/8 | `663eacb` | — | `gate --fast` PASS | lexer `vis-syntax`: 60/60 golden (đỏ trước), fuzz 10 phút, NFC/NFD; 40 test; **DONE** (đã push) |
| 2026-11-12 | — | khởi tạo | `ca2eaf5` | — | — | bàn giao vòng 1: 10 task, 3 READY (ST-01, ST-02b, ST-10), ST-02a DONE; ST-04 READY |
