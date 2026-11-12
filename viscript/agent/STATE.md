# STATE — ViScript (vòng 2 / VS0)

Người ghi: **Agent 2** dòng ViScript. Cập nhật **sau mỗi bước**.
Khởi tạo: 2026-11-12 bởi người điều phối (vòng vá sau review). Cập nhật vòng 2 (VS0): 2026-11-13.

```
HEAD          : xem `git log -1` (vòng 2 mở ở `aae08d4`)
BUILD_ID      : —
Task          : **ST-13b** (10 mục review ST-13) — **DONE, đã commit** `f1ac635` (2026-11-13).
                **ST-14** (`vis run` + TAP + 100 ca + nội suy `{expr}`) — **dừng giữa đường** theo lệnh
                Agent 0: mã + 100 fixture còn **trong cây, chưa commit** (chờ OK để commit `[ST-14]`).
Bước          : ST-13b 4/4 (đỏ trước → hiện thực → xanh → cổng → commit → push)
Nhánh         : main
Cổng gần nhất : `./vis-dev gate --fast` → `vis-dev gate --fast: PASS (L1 chưa có tới khi có sim ViScript)`
                (2026-11-12T23:22:24Z = 2026-11-13 08:22 JST; không retry; **247** test workspace:
                32 vis-cli + 18 vis-fmt + 96 vis-syntax + 20 vis-diag + 53 vis-types + 28 vis-interp)
Việc dở       : **ST-14** (xanh cục bộ: 100/100 ca `test/sem`, `vis run` có `--tap`, nội suy `{expr}`);
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
