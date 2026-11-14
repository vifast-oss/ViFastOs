# STATE — ViScript (vòng 2 / VS0)

Người ghi: **Agent 2** dòng ViScript. Cập nhật **sau mỗi bước**.
Khởi tạo: 2026-11-12 bởi người điều phối (vòng vá sau review). Cập nhật vòng 2 (VS0): 2026-11-13.

```
HEAD          : xem `git log -1` (vòng 2 mở ở `aae08d4`)
BUILD_ID      : —
Task          : **ST-71 — VS2 #6b: `use pkg.*`** (nạp gói đã cài theo `vis.lock`, `rel` tổng hợp
                `pkg/<tên>/<ver>/…`, kiểm **sha** lúc chạy ⇒ `VS0701`; vòng qua gói ⇒ `VS0702`;
                5 ca CLI mới + cập nhật 1 ca cũ) — xong, đã đo + cổng + differential; **chưa commit**.
Bước          : 5/5 (red-first 6 ca + probe (5+1 đỏ) → `modules.rs`: `giai_pkg`/`tach_pkg`/`chon_tep_pkg`/
                `doc_tep_goi` + `rel_cua` pkg + `pkg.rs` mở `pub(crate)` → 11/11 + 18/18 xanh →
                workspace **854** → cổng → differential 100k → STATE/JOURNAL).
Nhánh         : main
Cổng gần nhất : `./vis-dev gate --fast` → **PASS** lúc **2026-11-14T09:10:18Z**; **854** test / sem
                **441/21 nhóm** / `vis test` **441 ok** (+`--vm` **441 ok/7 SKIP**) / parity **395/395** /
                differential **100.000 chương trình 0 lệch** (246,6 s) / **7 sổ-32 mã** (không mã mới) /
                `VBC1` **53** / allowlist **6**
Việc dở       : commit `[ST-71]` + push (đã được uỷ quyền)

## ST-71 — VS2 #6b: `use pkg.*` + kiểm sha lúc chạy, 2026-11-14

| # | Việc | Kết quả |
|---|---|---|
| 0 | **Nạp gói** | `modules.rs`: `use pkg.<tên>[.<module>]` — đọc `vis.lock` trong `main_dir`; thiếu lock/gói ⇒ `VS0701` **nêu tên** + gợi ý `vis pkg add <tên>`; `seg[1]` = tên gói, `seg[2..]` = module trong gói. **Không** sửa `vis-syntax`/SPEC-lex (DottedPath sẵn có). |
| 1 | **Kiểm sha** | Chỉ nhận tệp **có trong lock**; đọc bytes → **chuẩn hoá** (dùng lại `pkg::chuan_hoa`) → `file_sha` so `files[].sha256`; lệch ⇒ **`VS0701` "gói `x@v` bị sửa"**, exit 2, **không** chạy mã sai; thiếu tệp trong lock cũng `VS0701`. |
| 2 | **`rel` tổng hợp** | `rel_cua`: tệp dưới `<main_dir>/.vis/pkg/` ⇒ `pkg/<tên>/<ver>/<path>` (không `file_name` trần) ⇒ hai gói cùng `src/main.vis` khác khoá cache (`khoa_multi` băm `rel`+text) và chẩn đoán đúng tệp; `--dump-deps` in đúng rel này. |
| 3 | **Vòng qua gói** | Giữ cơ chế `mau` (`DangTham`) ⇒ A↔B báo **`VS0702`**, không treo; chữ ký `load_full` **không đổi** (2 call site `main.rs:132,464` không sửa). |
| 4 | **Chọn tệp** | Không có module ⇒ entry quy ước: `src/main.vis` → `main.vis` → tệp `.vis` đầu theo byte (lock **không** lưu `entry` — nợ); có module ⇒ `<m>.vis` hoặc `src/<m>.vis` (kể cả `.vish`), mơ hồ ⇒ lỗi. |
| 5 | **Ca (5 mới + 1 cập nhật)** | `use_pkg_chay_that` (interp + `--vm --no-cache`, cùng `100\n`), `pkg_bi_sua_bao_vs0701` (VS0701 "bị sửa", stdout rỗng), `pkg_chua_cai` (nêu `http` + `vis pkg add http`, cả khi lock có gói khác), `hai_goi_cung_ten_tep` (`--dump-deps` in `pkg/http/1.10.0/src/main.vis` + `pkg/util/0.1.0/src/main.vis`, kết quả 1101), `pkg_vong_lap` (A↔B ⇒ VS0702). **Cập nhật** `tests/modules.rs`: `pkg_bao_vs2` → `pkg_chua_cai_bao_vs0701` (SỬA KÈM LÝ DO, không xoá ca). |
| 6 | **Red-first** | `JOURNAL/st71-red-rust.log` sha256 `a29ba63a…` (pkg: **6 passed / 5 failed**; modules: **17 passed / 1 failed**); `JOURNAL/st71-red-sem.log` sha256 `f4fca55e…` (`vis run` app `use pkg.http` ⇒ "gói ngoài chưa hỗ trợ", exit 2). |
| 7 | **Đo** | **854** test = 849 + **5** CLI (21+96+18+154+26+62+9+**468**); sem **441/21** giữ nguyên; `vis test` **441 ok**; `--vm` **441 ok + 7 SKIP**; parity **395/395**; `diag_cmp` **361/46/34**; differential **`tong=100000 loi_tinh=0 chay=100000 lech=0 giay=246.6 toc_do=406/s seed=3523158054`**; `VBC1` **53**; **7 sổ/32 mã**; allowlist **6**; R1 max **1.491** (`modules.rs` **726**, `pkg.rs` **753**). |
| 8 | **Nợ** | Lock **không lưu `entry`** ⇒ `use pkg.<tên>` dùng quy ước `src/main.vis`/`main.vis`/tệp `.vis` đầu (nên bổ sung `entry` vào lock sau); chưa resolver **dải** version (`^`/`~`) — deps ghim đúng version; chưa **ký số**/registry HTTP; chưa `pkg remove`/`update`; lock tìm ở `main_dir` (không dò ngược); module con chỉ `<m>.vis`/`src/<m>.vis`; `.vis/store` không GC. |

## ST-70 — VS2 #6a: `vis pkg` (registry local, không mạng), 2026-11-14

| # | Việc | Kết quả |
|---|---|---|
| 0 | **Lệnh mới** | `main.rs`: `mod pkg;` + `Some("pkg") => pkg::run(&args[1..])` + `usage()` thêm `vis pkg add|install|list`; **không** đụng `cache/run/check/test/fmt/lsp`. `vis-cli/src/pkg.rs` **739** dòng (≤1.500). |
| 1 | **Registry + manifest** | Fixture `viscript/test/pkg/registry/{util@0.1.0, http@1.0.0/1.9.0/1.10.0}` (mỗi `manifest.json` kèm `.license`, mỗi `.vis` có SPDX). Manifest = `{name,version,entry,deps,files[{path,sha256}]}` — **tường minh**, không glob/`read_dir` khi cài; thiếu trường ⇒ lỗi **nêu tên trường** (`entry`/`files`/`name`/`version`); path chỉ component thường (không `..`/tuyệt đối/`\`), từ chối **symlink** + non-UTF-8; `entry` phải nằm trong `files`. |
| 2 | **Chuẩn hoá + băm** | Bỏ BOM, CRLF→LF, đúng **một** LF cuối **khi ingest**; `file_sha = SHA256(nội dung chuẩn hoá)`; `tree_sha = SHA256("vispkg1
" + Σ sorted(path+"
"+file_sha+"
"))`; dùng `vis_bytecode::source_hash` (SHA-256 thật, **không** thêm crate). |
| 3 | **Lock + store** | `vis.lock` JSON compact + LF: `{lock:1,packages:[{name,version,sha256,files:[{path,sha256}]}]}`, packages/files **sắp theo byte**, **không** timestamp/đường dẫn tuyệt đối. Nội dung file lưu `.vis/store/<file_sha>` (content-addressed) ⇒ **`install` chỉ đọc `vis.lock` + store, không mở registry**. Ghi lock **cuối** bằng tmp + rename (mẫu `cache.rs`). |
| 4 | **Tất định** | Validate **toàn bộ** manifest/hash + blob trước khi ghi gì; lệch hash ⇒ dừng **chưa ghi**; xoá đích rồi ghi từng tệp theo thứ tự sort (không tệp thừa); `add` merge lock theo tên (gói mới thắng); lock hỏng ⇒ exit 2, `.vis/pkg` **giữ nguyên**. |
| 5 | **Phiên bản** | So **theo số** (`1.10.0 > 1.9.0`, `1.0 = 1.0.0`); không ghi `@ver` ⇒ chọn **cao nhất** trong registry; `@ver` ⇒ đúng version; `deps` ghim version chính xác (đệ quy, chống vòng bằng `BTreeSet`). |
| 6 | **Ca (6 CLI + 2 unit)** | CLI `tests/pkg.rs`: `manifest_thieu_truong` (nêu `entry`, không tạo `vis.lock`/`.vis`), `add_roi_list` (1.10.0 cao nhất, `@1.0.0` đúng, `list` `tên ver sha[:12]` sắp tên), `add_keo_phu_thuoc` (http+util), `install_khong_can_registry` (xoá registry + pkg ⇒ install dựng lại), `cai_hai_lan_cung_byte` (add → hash cây → rm `.vis/pkg` → install ⇒ hash bằng, lock không đổi, không chứa tmpdir), `lock_hong` (exit 2, cây nguyên). Unit: `bam_chuan_hoa_crlf_bom` (CRLF/BOM cùng hash, đổi byte đổi hash, **ghim** `file 73cb3858…`/`tree 510fb9cc…`), `so_phien_ban_theo_so`. |
| 7 | **Red-first** | `JOURNAL/st70-red-sem.log` sha256 `0c402bd2…` (`vis pkg list` ⇒ usage + exit 2); `JOURNAL/st70-red-rust.log` sha256 `8d85f966…` (harness 6 ca: **0 passed / 6 failed**). |
| 8 | **Đo** | **849** test = 841 + **6** CLI + **2** unit (21+96+18+154+26+62+9+**463**); sem **441/21** giữ nguyên; `vis test` **441 ok**; `--vm` **441 ok + 7 SKIP**; parity **395/395**; `diag_cmp` **361/46/34**; differential **`tong=100000 loi_tinh=0 chay=100000 lech=0 giay=274.1 toc_do=365/s seed=3523158054`**; `VBC1` **53**; **7 sổ/32 mã**; allowlist **6**; R1 max **1.491**. |
| 9 | **Nợ** | **Chưa** `use pkg.*` (ST-71) — gói cài xong chưa được ngôn ngữ nhập; deps ghim **đúng** version (chưa resolver dải/`^`/`~`, chưa nhiều version cùng tên); chưa registry HTTP/ký số/`sha256` của manifest; `.vis/store` **không GC**; chưa `pkg remove/update`; `add` không dọn version cũ trong `.vis/pkg`; `VIS_REGISTRY` tương đối theo cwd; lock chỉ một version/tên. |

## ST-69 — VS2 #5c: LSP completion + hover (tổng 22 phản hồi LSP), 2026-11-14

| # | Việc | Kết quả |
|---|---|---|
| 0 | **R1 — tách tệp** | `lsp/vi_tri.rs` **135** (vị trí UTF-16 + 6 unit — chuyển từ `phan_tich`), `lsp/chan_doan.rs` **44** (dựng `publishDiagnostics`), `lsp/goi_y.rs` **171** (completion/hover), `mod.rs` **265**, `phan_tich.rs` **246**, `jsonrpc.rs` **162**, `khung.rs` **72** — tất cả ≤300; R1 toàn cây max **1.491**. Không `unsafe`, không crate mới. |
| 1 | **Nguồn gợi ý** | Item cấp tệp: fn **3**, record **22**, enum **13**, alias **25**, const **21**, trait **11**, extern **3** (detail None); biến/tham số **6** — **đọc HIR** (`HStmt::Let{ty}`/`Var{ty}` + tham số `HFn`), vì sym của `let` là `PatBind` **không mang kiểu**; `BUILTIN_VALUE_NAMES` (11) **3**; `KEYWORDS` (40) **14**. Khử trùng theo `label` (item > biến > builtin > từ khoá), sắp theo **byte** label, lọc **tiền tố** tại con trỏ, cap **200**. |
| 2 | **Hover** | Từ tại con trỏ (`vi_tri::byte_tu_vi_tri` + quét ident) → item ⇒ markdown ```vis
<chữ ký>
``` (`fn add(a: int, b: int) -> int`); biến ⇒ `xs: list[int]`; builtin ⇒ `<builtin print>`; từ khoá ⇒ `từ khoá: let`; không có ⇒ `result:null`. `range` = **đúng từ** (0-based UTF-16, tab = 1). |
| 3 | **Request** | `textDocument/completion` + `textDocument/hover` đọc `textDocument.uri` + `position`, dùng **văn bản đã lưu** từ `didOpen`/`didChange`; **chưa mở** ⇒ `[]` / `null` (không lỗi); trước `initialize` ⇒ `-32002`; capabilities **giữ nguyên** (snapshot ST-67 vẫn xanh). |
| 4 | **Ca mới (7)** | `lsp_completion_tien_to_add` (đúng 1 item `add`, kind 3, detail chữ ký), `lsp_completion_tien_to_le` (`len` 3 rồi `let` 14), `lsp_completion_sau_cham` (tiền tố rỗng ⇒ **toàn bộ** 50 mục = 11 builtin + 40 keyword − `ref` trùng; lý do: client tự lọc, rỗng thì không gợi ý gì), `lsp_hover_add` (markdown + range), `lsp_hover_khong_ton_tai` (`null`), `lsp_hover_bien_kieu` (`xs: list[int]`), `lsp_completion_chua_mo` (`[]`). |
| 5 | **Red-first** | `JOURNAL/st69-red-sem.log` sha256 `d4bc3de4…` (probe completion ⇒ **-32601**); `JOURNAL/st69-red-rust.log` sha256 `82d0923e…` (harness 22 ca: **15 passed / 7 failed**). Sau hiện thực: 22/22 + 6/6 xanh. |
| 6 | **Đo** | **841** test = 834 + **7** integration (21+96+18+154+26+62+9+**455**); sem **441/21** giữ nguyên; `vis test` **441 ok**; `--vm` **441 ok + 7 SKIP**; parity **395/395**; `diag_cmp` **361/46/34**; differential **`tong=100000 loi_tinh=0 chay=100000 lech=0 giay=214.1 toc_do=467/s seed=3523158054`**; `VBC1` **53**; **7 sổ/32 mã**; allowlist **6**. |
| 7 | **Nợ** | **Chưa module-aware** (một tệp, `std_mods` tắt, chưa giải `use ./x` — như ST-68); completion **chưa theo ngữ cảnh sau `.`** (trả toàn bộ, không theo kiểu biến/thành viên); hover chưa field/member; chưa signature help/snippet/`documentation`; `didChange` full sync; builtin detail None; chưa lọc biến theo phạm vi con trỏ (gộp mọi biến). |

## ST-68 — VS2 #5b: LSP diagnostics + vị trí UTF-16, 2026-11-14

| # | Việc | Kết quả |
|---|---|---|
| 1 | **Phân tích một tệp** | `lsp/phan_tich.rs` (**210** dòng): `lex` → `parse_voi` → `lower_into` → `check`, gộp **lex + parse + lower + tyck**, sắp theo `(span.start, mã)`; `severity` Error⇒**1**, Warning⇒**2**; `code` = `Code::short`; message **làm phẳng** `\n \r \t` như `main.rs::dong_chan_doan`. Mode theo đuôi URI (`.vish` ⇒ Command). |
| 2 | **Vị trí LSP** | `vi_tri_utf16(text_norm, byte)` (0-based, **UTF-16**, tab = **1**) + `byte_tu_vi_tri` (cho ST-69); **tính trên `lexed.text`** (đã NFC + CRLF→LF — ghi rõ doc-comment); span vắt dòng/rỗng ⇒ kẹp `end = (line, min(char+1, len dòng))`. |
| 3 | **Handler** | `textDocument/didOpen` + `didChange` (**full sync**, `contentChanges[last].text`) + `didClose` ⇒ **notification** `textDocument/publishDiagnostics` (`{uri, diagnostics:[{range,severity,code,source,message}]}`), mảng **rỗng** khi sạch/đóng. Capabilities giữ `textDocumentSync:1` (đã khớp full sync). |
| 4 | **R1** | `lsp/mod.rs` **291**, `phan_tich.rs` **210**, `jsonrpc.rs` **90**, `khung.rs` **72** — tất cả ≤300; R1 toàn cây max **1.491**. Không `unsafe`, không crate mới. |
| 5 | **Ca (7 snapshot mới + 6 unit)** | `lsp_chan_doan_did_open` (VS0412 `0:13–0:16` sev1), `lsp_chan_doan_sach` (`[]`), `lsp_chan_doan_did_change` (VS0201 `0:8–0:8`), `lsp_chan_doan_emoji_utf16` (**`0:13–0:14`**), `lsp_chan_doan_tab_mot_cot` (VS0107 `1:0–1:1` sev2 + VS0412 `1:14–1:17`), `lsp_chan_doan_vish` (VS0201 `0:0–0:1` "mong lệnh"), `lsp_chan_doan_did_close` (`[]`). Unit: emoji, tab, CRLF, kẹp cuối dòng, biên 0/cuối tệp, nhiều dòng. |
| 6 | **Red-first** | `JOURNAL/st68-red-sem.log` sha256 `701a0256…` (chỉ frame initialize, **không** publishDiagnostics); `JOURNAL/st68-red-rust.log` sha256 `d9ece3ec…` (**8 passed / 7 failed** trên harness 15 ca). |
| 7 | **Đo** | **834** test = 821 + **7** integration + **6** unit (21+96+18+154+26+62+9+**448**); sem **441/21** giữ nguyên; `vis test` **441 ok**; `--vm` **441 ok + 7 SKIP**; parity **395/395**; `diag_cmp` **361/46/34**; differential **`tong=100000 loi_tinh=0 chay=100000 lech=0 giay=313.5 toc_do=319/s seed=3523158054`**; `VBC1` **53**; **7 sổ/32 mã**; allowlist **6**. |
| 8 | **Nợ** | **Chưa module-aware**: `LowerOptions::default()` (không bật `std_mods`, không giải `use ./x`) ⇒ tệp dùng `std.*`/module có thể báo chẩn đoán **khác** `vis check`; vẫn chạy lower/tyck khi lex/parse có lỗi ⇒ có thể thêm chẩn đoán nhiễu; `didChange` chỉ **full sync**; `byte_tu_vi_tri` mới dùng trong unit test (ST-69 dùng); chưa pull diagnostics/version; **chưa** completion/hover (ST-69). |

## ST-67 — VS2 #5a: `vis lsp --stdio` khung LSP + JSON-RPC 2.0, 2026-11-14

| # | Việc | Kết quả |
|---|---|---|
| 0 | **R1** | 3 tệp mới: `lsp/mod.rs` **181**, `lsp/khung.rs` **72**, `lsp/jsonrpc.rs` **66** dòng (đều ≤300); `main.rs` **720** (<1.500). Không `unsafe`, không crate mới. |
| 1 | **Khung** | `khung::Doc<BufReader<StdinLock>>`: đọc header tới dòng trống, **bắt buộc** `Content-Length`, bỏ qua `Content-Type`/header lạ, `read_exact` đúng **N byte**, cap body **16 MiB**; ghi `Content-Length: {len}

{body}` + **flush**. EOF sạch ⇒ `Ok(None)`; header cụt ⇒ lỗi (stderr, exit 1). |
| 2 | **JSON-RPC** | `jsonrpc.rs` dựng `Value::Map` (giữ thứ tự) rồi mã hoá bằng `vis_interp::stdlib::json`; giải mã bằng `vis_interp::json::decode` — **không** serde/crate mới (tiền lệ `fsmock.rs:66`). Response `{jsonrpc,id,result}`, error `{jsonrpc,id,error{code,message,data}}`. |
| 3 | **Dispatch** | `initialize` ⇒ `capabilities{textDocumentSync:1, positionEncoding:"utf-16", hoverProvider:true, completionProvider{triggerCharacters:["."]}}` + `serverInfo{name:"vis"}` (**không** version/rootUri ⇒ snapshot tất định); `initialized` = notification **không frame**; `shutdown` ⇒ `result:null`; `exit` ⇒ **0** nếu đã shutdown, **1** nếu chưa; method lạ ⇒ **-32601**; body JSON hỏng ⇒ **-32700** (`id:null`, `data` = lỗi decode); trước `initialize` ⇒ **-32002**; object thiếu `method`/không phải object ⇒ **-32600** (thêm rẻ); EOF stdin ⇒ exit 0. |
| 4 | **Nối CLI** | `main.rs`: `mod lsp;`, nhánh `Some("lsp") => lsp::run(&args[1..])`, `usage()` thêm `vis lsp --stdio`; cờ lạ ⇒ stderr + **exit 2**. Nhánh LSP **không** `println!`/`xuat_stdout` — stdout chỉ có frame. |
| 5 | **Ca (8 test)** | `crates/vis-cli/tests/lsp.rs`: `lsp_khoi_tao`, `lsp_initialized_khong_frame`, `lsp_shutdown_exit_0`, `lsp_exit_truoc_shutdown_1`, `lsp_method_la_32601`, `lsp_body_hong_32700`, `lsp_truoc_initialize_32002`, `lsp_co_la_2`. Helper `khung(body)` + `chay(input)` bơm stdin, đọc hết stdout, **deadline 5 s** rồi kill. Snapshot **nguyên văn** từng frame (không parse lại); `Content-Length` khớp `body.len()` theo cấu tạo. |
| 6 | **Red-first** | `vis lsp` **chưa tồn tại**: `JOURNAL/st67-red-sem.log` sha256 `29887e3a…` (probe `vis lsp --stdio` ⇒ usage + **exit 2**); `JOURNAL/st67-red-rust.log` sha256 `588808c2…` (`cargo test -p vis-cli --test lsp` ⇒ **0 passed / 8 failed**, exit 101). |
| 7 | **Đo** | **821** test = 813 + **8** (21+96+18+154+26+62+9+**435**); sem **441/21** giữ nguyên; `vis test` **441 ok**; `--vm` **441 ok + 7 SKIP**; parity **395/395**; `diag_cmp` **361/46/34**; differential **`tong=100000 loi_tinh=0 chay=100000 lech=0 giay=282.6 toc_do=354/s seed=3523158054`**; `VBC1` **53**; **7 sổ/32 mã**; allowlist **6**; R1 max **1.491**. |
| 8 | **Khung byte mẫu (initialize)** | body **199 byte**: `{"jsonrpc":"2.0","id":1,"result":{"capabilities":{"textDocumentSync":1,"positionEncoding":"utf-16","hoverProvider":true,"completionProvider":{"triggerCharacters":["."]}},"serverInfo":{"name":"vis"}}}`; frame = `Content-Length: 199

` + body. |
| 9 | **Nợ** | **Chưa** có diagnostics/completion/hover thật (ST-68/69 — `hoverProvider:true` mới là khai báo trước); chưa đọc `params`; `positionEncoding` cố định `utf-16` (chưa thương lượng); chưa xử lý notification khác (vd `textDocument/didOpen`); `-32600` ngoài danh sách đề bài (thêm rẻ); EOF stdin ⇒ exit 0 (không theo spec LSP "chỉ exit khi có `exit`" — cần cho CI/test); một luồng; cap body 16 MiB. |

## ST-66 — VS2 #3d: ca biên net/http (24 ca, chốt hạng mục VS2 #3), 2026-11-14

| # | Việc | Kết quả |
|---|---|---|
| 0 | **R1** | Không chạm mã sản phẩm; R1 toàn cây max **1.491** (`lower/cmd.rs`). Test: `net.rs` **452**, `http.rs` **343**, `net_server.rs` **384** dòng (<1.500). |
| 1 | **`--vm` cache cho server** | `server_mot_lan` chạy **4 lượt** trên **cùng** tệp chương trình: `interp`, `--vm --no-cache`, `--vm` (cache lần 1), `--vm` (cache lần 2 = **cache hit**); mỗi lượt xoá tệp handshake, client Rust nhận `pong`; cả 4 exit 0, byte y hệt. Đo: **206 / 85 / 129 / 178 ms**. |
| 2 | **6 ca mới** | `tcp_doc_ghi_sau_close` (read/write sau `close` ⇒ `Err` "không tồn tại", exit 0), `tcp_read_cap_64` (server 1 MiB, `read(t,64,…)` lần 1/2 ⇒ đúng 64 byte 'A' mỗi lần), `server_hai_ket_noi_lien_tiep` (`accept` **2 lần** cùng listener ⇒ client 1 `mot`, client 2 `hai`), `http_path_query` (`GET /a/b?x=1&y=2` đúng target), `http_body_rong` (`204` + `CL=0` ⇒ `Ok("")` → stdout `()`), `tcp_explain_tong` (**≥4** dòng `NET listen/accept/connect/http GET`, stdout sạch, 2 đường cùng byte). |
| 3 | **`--explain-perms`** | **Không cần thêm mã**: `NET connect` (ST-63), `NET listen`/`NET accept` (ST-65), `NET http GET` (ST-64) đã in ra stderr. Ca tổng khoá lại **đủ 4 dòng** + stdout sạch + hai đường **cùng byte** (kể cả stderr). |
| 4 | **Bảng ca ≥13** | **24 ca** net/http/server (xem bảng đầy đủ ở `JOURNAL/2026-11-14.md` §ST-66 + báo cáo); **không** đặt ca vào `test/sem` (sem **441**). |
| 5 | **Chống flake** | Tuần tự hoá **cả 3** test binary (`net`, `http`, `net_server`) bằng `Mutex` — chỉ giảm nhiễu tiến trình/đo thời gian, **không** nới ngưỡng (vẫn 400 ms/3 s). |
| 6 | **Red-first — trung thực** | **Không có mã sản phẩm mới** ⇒ 6 ca mới chạy **xanh ngay** trên HEAD ST-65 (`6514fce1`); **không dựng đỏ giả**. `JOURNAL/st66-red-sem.log` sha256 `6b5590ba…` (13 dòng, ghi rõ 0 đỏ), `JOURNAL/st66-red-rust.log` sha256 `479c210f…` (`24/24 xanh`, `[cargo exit=0]`). |
| 7 | **Đo** | **813** test = 807 + **6** (21+96+18+154+26+62+9+**427**); sem **441/21** (391 `.vis` + 50 `.vish`, 80 `.err` — không đổi); `vis test` **441 ok**; `--vm` **441 ok + 7 SKIP**; parity **395/395**; `diag_cmp` **361/46/34**; differential **`tong=100000 loi_tinh=0 chay=100000 lech=0 giay=217.0 toc_do=461/s seed=3523158054`**; `VBC1` **53**; **7 sổ/32 mã**; allowlist **6**. |
| 8 | **Thời gian e2e (đo, tuần tự)** | `server_mot_lan`: **206 / 85 / 129 / 178 ms** (interp/vm-nocache/cache-1/cache-2); `server_hai_ket_noi_lien_tiep`: **66 / 173 ms**; `server_hai_tien_trinh`: **94 / 93 / 163 ms**; `accept_het_gio`: **267 / 160 ms** (≤400); `http_het_gio` + toàn bộ `http.rs` **~0,5 s**. |
| 9 | **Nợ** | Giữ nguyên nợ ST-63/64/65 (một luồng, không TLS/redirect/keep-alive/chunked, chỉ loopback, handle per-process, cap 8 MiB, bỏ `is_listening`). Thêm: **ST-66 không có "đỏ thật"** vì không thay đổi mã — vòng sau muốn red-first thật phải kèm thay đổi mã; tuần tự hoá test làm thời gian chạy tăng nhẹ. |

## ST-65 — VS2 #3c: server TCP (`std.net.listen/local_addr/accept(_timeout)/close_listen`), 2026-11-14

| # | Việc | Kết quả |
|---|---|---|
| 0 | **R1/headroom** | `vis-cli/src/net.rs` **157 → 235** dòng (≤400, không cần tách `net/listen.rs`); `builtin.rs` **1.383**, `interp.rs` 1.226; R1 toàn cây max **1.491** (`lower/cmd.rs`). Không có bước tách tệp. |
| 1 | **Host thật (CLI)** | `Net` thêm `nghe: Vec<(String, TcpListener)>` + `tiep_nghe` (handle `"L<n>"`, tách `"C<n>"`). `listen` bind loopback (`:0` cho phép), cổng bận (`AddrInUse`) ⇒ `Err` rõ "đang bận". `local_addr` ưu tiên listener rồi kết nối. `accept`: `timeout=0` ⇒ **chặn**; `>0` ⇒ `set_nonblocking(true)` + poll **5 ms** tới hạn, coi **cả** `WouldBlock` (macOS) **và** `TimedOut` là hết giờ; kết nối nhận được đăng ký `"C<n>"`. `close` đóng được **cả** `C…` lẫn `L…` (nên `close_listen` đi chung). |
| 2 | **Bảng chữ ký** | `FUNCS` **45 → 50** (+5): `net.listen`, `net.local_addr`, `net.accept`, `net.accept_timeout`, `net.close_listen`; `MODULES` giữ **9**. **Bỏ** `net.is_listening` — 5 hàm vừa đủ (đề bài cho phép "hoặc 4 hàm nếu không cần"; ở đây cần `accept_timeout` cho ca 3 nên vẫn 5). |
| 3 | **Thực thi** | Nhánh `std.net.*` server trong `vis-interp/src/stdlib.rs` qua `host.net()`; thiếu host ⇒ **`VS0604`**; `net.accept` không có timeout ⇒ `timeout=0` (chặn); lỗi mạng = `Err(str)` (dữ liệu, không trap); `close_listen` = `host.close`. VM dùng cùng nhánh ⇒ cùng byte. |
| 4 | **Ca (5 test Rust)** | `crates/vis-cli/tests/net_server.rs`: `server_mot_lan` (một tiến trình `vis` `listen(:0)` → `local_addr` → thread Rust nối → `accept` → `write("pong")`; chạy cả `run` và `--vm --no-cache`), `server_hai_tien_trinh` (A `vis run` server ghi cổng vào **tệp**; cha chờ tệp deadline 5 s/poll 10 ms; B `vis run` nối+đọc `pong`; **3 vòng**, mỗi vòng ≤3 s), `accept_het_gio` (`accept_timeout(L,80)` không ai nối ⇒ "quá thời gian", ≤400 ms), `cong_ban` (bind sẵn cổng ⇒ `Err`, exit 0), `server_khong_host` (in-process cả interp+VM ⇒ **`VS0604`**, cùng byte). |
| 5 | **Chống flake** | Cổng luôn `:0`; tệp handshake/PID dọn cuối test; `dung(child)` chờ ≤5 s rồi kill; **tuần tự hoá** các ca trong test binary bằng `Mutex` (chỉ giảm nhiễu tiến trình, **không** nới ngưỡng nào). |
| 6 | **Red-first (harness cuối)** | Tạm gỡ **3 tệp mã** (vis-types/stdlib.rs, vis-interp/stdlib.rs, vis-cli/net.rs) + khoá đếm: `JOURNAL/st65-red-sem.log` sha256 `a24acea4…` (probe `use std.net` + `net.listen` ⇒ **VS0301** "module `std.net` không có hàm `listen`" cả hai đường); `JOURNAL/st65-red-rust.log` sha256 `dcb7592c…` (`cargo test -p vis-cli --test net_server` ⇒ **0 passed / 5 failed**, exit 101). Khôi phục **5/5 sha256 khớp**, build lại, 5/5 xanh (3 lần liên tiếp). |
| 7 | **Đo** | **807** test = 802 + **5** `net_server.rs` (21+96+18+154+26+62+9+**421**); sem **441/21** (391 `.vis` + 50 `.vish`, 80 `.err` — không đổi); `vis test` **441 ok**; `--vm` **441 ok + 7 SKIP**; parity **395/395**; `diag_cmp` **361/46/34**; differential **`tong=100000 loi_tinh=0 chay=100000 lech=0 giay=257.7 toc_do=388/s seed=3523158054`**; `VBC1` **53**; **7 sổ/32 mã**; allowlist **6**; R1 max **1.491**. |
| 8 | **Thời gian e2e (đo, tuần tự)** | `server_mot_lan`: **103 ms** (interp) / **320 ms** (vm); `server_hai_tien_trinh` 3 vòng: **93 / 72 / 357 ms** (đều ≤3 s); `accept_het_gio`: **320 ms** (interp) / **201 ms** (vm), đều ≤400 ms. Máy đang bận (agent khác chạy cổng) nên số cao hơn lý thuyết — **giữ nguyên ngưỡng**, không nới. |
| 9 | **Nợ** | Mô hình **một luồng**: không `select`/epoll, không phục vụ **nhiều kết nối đồng thời** (accept tuần tự, mỗi lúc một client); không thread/task; chỉ loopback; handle per-process; không TLS; `is_listening` bỏ; backlog mặc định của `TcpListener::bind`; `accept_timeout` poll 5 ms (sai số ≤5 ms); `close`/`close_listen` dùng chung một đường. |

## ST-64 — VS2 #3b: `std.http.get`/`get_timeout` (giao thức trong `vis-interp`), 2026-11-14

| # | Việc | Kết quả |
|---|---|---|
| 1 | **Giao thức HTTP (mới)** | `vis-interp/src/net.rs` (**211** dòng, ≤300): tách URL `http://host[:port][/path][?query]` (**chỉ** `http://`, **chỉ loopback**); dựng `GET <target> HTTP/1.1
Host: <authority>
Connection: close

`; đọc tới **EOF** hoặc đủ `Content-Length`; cap **8 MiB** (mẫu `MAX_STDIN`) + cap header 64 KiB; tách header/body tại `

`; status **non-2xx** ⇒ `Err("HTTP <mã>")`; EOF khi thiếu body theo `Content-Length` ⇒ `Err` (không treo); body **byte** → `str` **lossy** (như `net.read`). |
| 2 | **Bảng chữ ký** | `MODULES` **8 → 9** (`http`); `FUNCS` **43 → 45** (+2): `http.get(str) -> Result[str,str]`, `http.get_timeout(str,int)`. **Không** thêm variant `Ty`; **bỏ** `http.body_len` — ca cap không cần, ghi rõ theo cho phép "2 hàm thôi và ghi rõ" của chỉ đạo. |
| 3 | **Thực thi dùng chung** | `vis-interp/src/stdlib.rs` nhánh `http.get`/`http.get_timeout`: `kiem` arity/kiểu tự động; `host.net()` thiếu ⇒ **`VS0604`**; gọi `crate::net::http_get`; mặc định **5000 ms**; lỗi HTTP/mạng là **`Err(str)`** (dữ liệu, không trap). VM dùng **cùng** hàm ⇒ cùng byte; **không** sửa codegen. |
| 4 | **`--explain-perms`** | Thêm **một** method **default no-op** `NetHost::ghi_nhat_ky(&mut self, dong: &str)` (không đổi impl cũ); `vis-interp/net.rs` gọi `ghi_nhat_ky("NET http GET <authority><target>")` trước khi đọc; CLI `Net` cài để in ra **stderr** khi `--explain-perms`. Đây là điểm thêm **ngoài** 7 method đã duyệt của trait — chỉ để tầng giao thức báo được dòng quyền. |
| 5 | **`use std.http`** | Đi qua `MODULES` (hết `VS0701`); `modules.rs` cập nhật thông điệp danh sách module (`…/fs/net/http`). Không đổi CLI host mạng (ST-63 đã nối đủ 4 điểm truyền host; HTTP chỉ dùng `NetHost` sẵn có). |
| 6 | **Ca (7 test Rust)** | `crates/vis-cli/tests/http.rs`: `http_200_byte` (body `"chào 😀"` đúng từng byte + **cache hit** của `--vm`), `http_404` (`Err` chứa 404, exit 0), `http_het_gio` (`get_timeout(…,80)` + server ngủ 300 ms ⇒ "quá thời gian", **≤400 ms**/đường), `http_tu_choi` (listener đóng ⇒ "bị từ chối"), `http_gioi_han` (CL = 8 MiB+1 ⇒ `Err` cap; CL=1000/body=3 + đóng ⇒ `Err`, không treo), `http_sai_kieu` (`http.get(42)` ⇒ **VS0412** tĩnh; thiếu `use` ⇒ **VS0501** lúc chạy), `http_duong_mac_dinh` (`GET / HTTP/1.1` + `Host` + `Connection: close`; `--explain-perms` in `NET http GET 127.0.0.1:…`). Mọi ca `run` ↔ `run --vm --no-cache` **so byte**; sem **441 giữ nguyên**. |
| 7 | **Red-first (harness cuối)** | Tạm gỡ đúng **7 tệp mã http** (giữ test), build: `JOURNAL/st64-red-sem.log` sha256 `c230b57e…` (13 dòng, probe `use std.http` ⇒ **`VS0701`** cả hai đường); `JOURNAL/st64-red-rust.log` sha256 `8115d00a…` (88 dòng, `cargo test -p vis-cli --test http` ⇒ **0 passed / 7 failed**, exit 101). Khôi phục: **8/8 sha256 khớp** (7 tệp + `net.rs`), 7/7 xanh. |
| 8 | **Đo** | **802** test = 793 + **7** `http.rs` + **2** unit `vis-interp::net` (21+96+18+154+**26**+62+9+**416**); sem **441/21** (391 `.vis` + 50 `.vish`, 80 `.err` — không đổi); `vis test` **441 ok**; `--vm` **441 ok + 7 SKIP**; parity **395/395**; `diag_cmp` **361/46/34**; differential **`tong=100000 loi_tinh=0 chay=100000 lech=0 giay=233.0 toc_do=429/s seed=3523158054`**; `VBC1` **53**; **7 sổ/32 mã**; allowlist **6**; R1 max **1.491**; `net.rs` **211** dòng (≤300). |
| 9 | **Sai khác so với đề bài (đo được, ghi rõ)** | (a) `vis check` có chẩn đoán **exit 1** (không phải 2 — 2 là dùng sai CLI); (b) thiếu `use std.http` ⇒ **`VS0501` lúc chạy** ("tên `http` chưa hỗ trợ"), **không** phải `VS0301`; `vis check` hiện **permissive** với tên cấp tệp (nợ có sẵn, không sửa trong ST-64); (c) `"chào 😀"` là **10 byte** UTF-8 (không phải 9) — test tính `Content-Length` từ `body.len()` thật; (d) `FUNCS` **+2** (45) thay vì +3 vì bỏ `http.body_len`. |
| 10 | **Nợ** | **Không** TLS, **không** redirect, **không** keep-alive, **không** chunked-transfer decoding, **không** header tuỳ biến/POST/HEAD; **chỉ** `http://` + loopback; body trả `str` **lossy** (chưa có kiểu byte); cap 8 MiB **cố định** (không cờ); `http.get` mặc định **5000 ms** (đổi bằng `get_timeout`); một request/lần gọi; `--explain-perms` đi qua hook `ghi_nhat_ky` (không nằm ở host). |


## ST-63 — VS2 #3a: `NetHost` + TCP client (`std.net.*`), 2026-11-14

| # | Việc | Kết quả |
|---|---|---|
| 0 | **R1 trước, code sau (bắt buộc)** | `crates/vis-interp/src/interp.rs` **1.486 → 1.217** dòng: chuyển **nguyên văn** khối gọi hàm (`eval_call`, `eval_args`, `call_value`, `call_closure(_trong)`, `call_hfn(_trong)`) sang mô-đun con mới `interp/goi.rs` (**287** dòng, `mod goi;`). Chỉ đổi **1 chữ** ngoài di chuyển: `eval_call` `fn` → `pub(crate) fn` (interp.rs còn gọi). `cargo fmt` + `gate --fast` **PASS** `2026-11-14T05:47:22Z` **trước** khi thêm net. |
| 1 | **`NetHost` + `Host::net`** | `vis-interp/src/builtin.rs`: trait `NetHost` cạnh `FsHost` (`connect/listen/local_addr/accept/read/write/close`; `read -> Result<Vec<u8>,String>` **byte**, `Ok(vec![])` = EOF); `Host::net()` mặc định `None` ⇒ `VS0604`. `Interp` thêm field `net` + `set_net`; `Vm` thêm field `net` + `set_net`; `impl Host::net` cho cả hai. `run_with_hosts` của interp nhận **3 host**; `vis_vm::run_with_hosts` mới (giữ `run_with_fs` làm wrapper). |
| 2 | **Bảng chữ ký `std.net`** | `vis-types/src/stdlib.rs`: `MODULES` **7 → 8** (thêm `net`); `FUNCS` **38 → 43**: `net.connect(str)`, `net.connect_timeout(str,int)`, `net.read(str,int,int)`, `net.write(str,str)`, `net.close(str)` (2 lỗi trả `Result[(),str]`). **Không** thêm variant `Ty` (handle = `str` mờ). Bỏ `is_open` (được phép "nếu rẻ; không thì thôi" — cần thêm method host mà trait đã duyệt không có). |
| 3 | **Thực thi chung** | `vis-interp/src/stdlib.rs` nhánh `net.*` (một hiện thực, VM dùng chung): `kiem` arity/kiểu tự động; `host.net()` thiếu ⇒ **`VS0604`** ("không có host"); lỗi mạng ⇒ `Err(str)` (dữ liệu, **không** trap); `read` giải mã **lossy** từ byte (nợ: chưa có API byte trong ngôn ngữ), EOF ⇒ `""`. **Không** sửa codegen (`std.*` hạ theo tên). |
| 4 | **CLI host mạng** | `vis-cli/src/net.rs` (**150** dòng): TCP thật bằng `std::net` (không `unsafe`, không crate mới); **chỉ loopback** (`127.0.0.1`/`localhost`, host khác ⇒ `Err`); `connect_timeout`; `set_read_timeout`; **cả** `WouldBlock` (macOS os error 35) **và** `TimedOut` ⇒ "quá thời gian"; handle `"C<n>"` mờ; `close` tay lạ ⇒ `Err`; `--explain-perms` in `NET connect 127.0.0.1:…` ra stderr. `listen`/`accept` trả `Err` rõ "để ST-65" (không giả vờ có server). |
| 5 | **Nối đủ 4 điểm truyền host** | `main.rs`: `mod net` + `use net::Net as HostNet`; truyền net ở **cả 4** chỗ: (a) nhánh **fast-path cache hit** (`--vm` + cache), (b) nhánh VM thường, (c) interp `.vish` (runner+fs), (d) interp `.vis` (fs). `modules.rs`: `use std.net` đi qua `MODULES`; thông điệp lỗi module nêu thêm `fs/net`. |
| 6 | **Ca (6 test Rust, không ca sem)** | `crates/vis-cli/tests/net.rs`: `tcp_noi_va_doc` (server gửi `hi`; + nhánh **cache hit** lần 2), `tcp_tu_choi` (listener đóng ⇒ `Err "bị từ chối"`, **exit 0**), `tcp_het_gio_doc` (server ngủ 300 ms, `read(t,64,80)` ⇒ "quá thời gian", **≤ 400 ms** cả hai đường), `tcp_tay_la` (`read/write/close` tay `C99` ⇒ `Err`, không panic), `tcp_khong_host` (in-process interp+VM ⇒ **`VS0604` cùng byte**), `tcp_explain_perms` (`--explain-perms` ⇒ stderr có `NET connect 127.0.0.1`). Mọi ca chạy `run` ↔ `run --vm --no-cache` **so byte**. Sem **441 giữ nguyên** (VD-105 (7)). |
| 7 | **Red-first** | `JOURNAL/st63-red-sem.log` sha256 `878056b1…` (14 dòng): tạm gỡ đúng 10 tệp mã net (giữ **harness cuối**) ⇒ probe `use std.net` **`VS0701`** cả hai đường; `JOURNAL/st63-red-rust.log` sha256 `cf8363ae…` (79 dòng): `cargo test -p vis-cli --test net` ⇒ **0 passed / 6 failed**, exit 101. Khôi phục 10 tệp từ bản lưu **sha256 khớp 10/10**, build lại, 6/6 xanh (chạy 3 lần liên tiếp 6/6). |
| 8 | **Đo** | **793** test (21+96+18+154+24+62+9+**409**; +6 `net.rs`); sem **441/21** (391 `.vis` + 50 `.vish`, 80 `.err` — không đổi); `vis test` **441 ok**; `--vm` **441 ok + 7 SKIP**; parity **395/395**; `diag_cmp` **361/46/34**; differential **`tong=100000 loi_tinh=0 chay=100000 lech=0 giay=215.3 toc_do=464/s seed=3523158054`**; `VBC1` **53**; **7 sổ/32 mã** (không mã mới); allowlist **6**; R1 max **1.491** (`lower/cmd.rs`). |
| 9 | **Nợ** | (a) `listen/local_addr/accept` = khung **ST-65** (hiện `Err` rõ); (b) **chỉ loopback**, chưa DNS/TLS/HTTP (ST-64..66); (c) `net.read` trả `str` **lossy** — chưa có kiểu byte trong ngôn ngữ; (d) `is_open` **bỏ** (không có trong trait duyệt); (e) `connect` mặc định timeout **1 s**; (f) handle **per-process**, không truyền qua tệp/tiến trình; (g) chưa có `--net-mock` (ca dựng server thật). |

## ST-62 — báo cáo **R10** (chốt vòng 10: nợ lõi A/B/C + async + kênh), 2026-11-14

| # | Việc | Kết quả |
|---|---|---|
| 1 | **Khuôn + phủ hàng** | Theo `R9-viscript.{md,json}` (Khối E); `QUEUE@head` **85 hàng** (**70 DONE + 15 READY**), 6 cột; `head` = **`6192bbd`** (commit nền — theo tiền lệ R9: `head` = cha commit báo cáo; commit `[ST-62]` là commit kế tiếp). `tasks[]` phủ **đủ 85**, `blocked[] = []` (không hàng BLOCKED/REVIEW). |
| 2 | **Đo lại toàn bộ (không chép)** | `cargo test --workspace` **787** (21+96+18+154+24+62+9+403); sem **441/21** (391 `.vis` + 50 `.vish`, **80** `.err`); `vis test` **441 ok**; `--vm` **441 ok/7 SKIP**; parity **395/395** (46 lỗi biên dịch; `diag_cmp` 361/46/34); differential **`tong=100000 loi_tinh=0 chay=100000 lech=0 giay=208.5 toc_do=480/s seed=3523158054`**; fuzz parser 60 s **2.661.866 ca / 60.522.399 token / 44.364 ca/s / 1.008.706 token/s / 0 vi phạm**; `VBC1` **53** tag; **7 sổ/32 mã**; allowlist **6**; 10k async **319/168 ms** (10.001 dòng, cùng byte). |
| 3 | **JSON** | đúng **11 khoá**, `round=10`, `head=6192bbd`, `project="viscript"`, 85 task; `perf` ghi async 10k + differential; `coverage.fixtures=441`; `gates.fast=PASS`. |
| 4 | **MD** | §1 tóm tắt 5 dòng; §2 bảng ST-56..ST-62 (hash + số đo); §3 bảng số liệu + **lệnh tái lập**; §4 **§19 đối chiếu VS2**: **2/8 có mặt một phần** (async — chưa song song thật; kênh — chưa `select`), **6/8 chưa** (net/http, quyền+Landlock, LSP, gói/registry, windows/wasm, hợp chuẩn 4 nền) + kế hoạch vòng 11 **ST-63..ST-76**; §5 **20 mục nợ** gộp ST-56..ST-61 (không bỏ mục nào); §6 **VD-89..VD-100** mỗi cái một dòng; §7 đề xuất. |
| 5 | **Máy kiểm** | `python3 test/report/check_report.py --all` ⇒ **PASS (10 báo cáo, 85 hàng QUEUE phủ đủ)**; không sửa `R1..R9`, `QUEUE.md`, `DECISIONS.md`. |
| 6 | **Cổng** | `./vis-dev gate --fast` ⇒ **PASS** `2026-11-14T05:33:50Z` (không retry), trong đó report-check **PASS 10 báo cáo/85 hàng**; 3 dòng cuối: `test result: ok. 0 passed; …` / `--- L0 test: OK` / `vis-dev gate --fast: PASS …`. |

## ST-61 — VS2 #2: kênh `chan[T]` (`send`/`recv`/`close`), deadlock `VS0505` không treo, 2026-11-14

| # | Việc | Kết quả |
|---|---|---|
| 1 | **Kiểm chứng lại (không viết lại)** | `cargo test --workspace` = **787** test, **0 failed** (21 diag + 96 syntax + 18 fmt + 154 types + 24 interp + 62 bytecode + 9 vm + **403 cli**; +5 so ST-60c = `vis-cli/tests/kenh.rs`). Chạy tay **15 ca** `test/sem/kenh/*.vis` bằng `vis run` và `vis run --vm --no-cache`: **15/15 khớp `.out`/`.err` từng byte**, hai đường **cùng byte**; 3 ca deadlock **0,085–0,104 s** (mốc < 1 s) ⇒ **không treo**. |
| 2 | **Log đỏ (red-first) trung thực** | `git stash push` **chỉ 4 thư mục mã** (`vis-interp/src`, `vis-types/src`, `vis-bytecode/src`, `vis-diag/src`), giữ test + fixture; `JOURNAL/st61-red-sem.log` sha256 `fb268bc4…`: **14/15 ĐỎ** (`VS0301` "kiểu `Chan` không có trong VS0" / `VS0501` "tên `chan` chưa hỗ trợ ở VS0"), 1 ca **đã xanh sẵn** = `chan-ten-nguoi-dung` (khoá `chan` làm **tên hàm người dùng** vẫn chạy — ca bảo vệ soft keyword, ghi rõ **không** giả vờ đỏ); `JOURNAL/st61-red-rust.log` sha256 `f5ac414a…`: **5/5 `#[test]` FAILED**, `cargo exit=101`. Pop stash ⇒ 10/10 sha256 mã **khớp trước** khi stash. |
| 3 | **R1 — buộc tách tệp (phát sinh)** | `tyck/expr.rs` **1.532** dòng > 1.500 ⇒ tách **2 mô-đun con**: `tyck/expr/kenh.rs` (63 dòng — `kiem_chan_new`, `kiem_chan_method`) và `tyck/expr/phu.rs` (39 dòng — 3 hàm thuần `is_percent_float`/`builtin_is_value`/`builtin_value_ty`); chuyển `impl FnSig { ret_raw }` sang `tyck/sig.rs`. Ngữ nghĩa **không đổi**, `expr.rs` còn **1.464** dòng. `gate --fast` **PASS**. |
| 4 | **Cổng** | `./vis-dev gate --fast` từ `/Users/mps/dvchd/ViFastOs` **PASS** `2026-11-14T05:12:52Z` (không retry); 3 dòng cuối: `test result: ok. 0 passed; …` / `--- L0 test: OK` / `vis-dev gate --fast: PASS`; R1..R4 OK, clippy OK, doc-examples **18 xanh + 6 ngoại lệ**, report-check 9 báo cáo/85 hàng. |
| 5 | **Differential** | `./bin/vis-diff` ⇒ `DIFF tong=100000 loi_tinh=0 chay=100000 lech=0 giay=213.4 toc_do=469/s seed=3523158054` (**0 lệch**). `JOURNAL/st61-diff-100k.log` sha256 `39091ef7…`. |
| 6 | **Số đo** | **787** test · sem **441/21 nhóm** (391 `.vis` + 50 `.vish`, **80** `.err`) · `vis test` **441 ok** · `--vm` **441 ok / 7 SKIP** · parity **395/395** (46 ca lỗi biên dịch; `diag_cmp` 361 xanh/46/34) · `VBC1` **53** (không bump) · **7 sổ / 32 mã** (`VS0504`/`VS0505` mới; `VS0506` giữ index 5) · allowlist **6**. |
| 7 | **Nợ (ghi `R10` §5)** | (a) **drain** chỉ chạy task ở hàng `ready` (**`spawn`**); task `Moi` do gọi `async fn` (chưa spawn/await) **không** được chạy ⇒ `recv` có thể `VS0505` dù còn Task; (b) `close` **idempotent** nhưng **không đánh thức** ai (không có wait-set — mô hình run-to-completion); (c) **không có `select`/timeout**; (d) `Chan` **không** truyền qua **biên host** (chỉ trong interp/VM); (e) `cap=0` thực tế **không dùng được** (cả hai phía chặn đều drain rồi `VS0505`); (f) dung lượng **âm** là lỗi **lúc chạy** `VS0501` (tyck chỉ đòi `int`); (g) `Chan` in `<kênh>` (không có `show` cấu trúc). |

## ST-60c — derives/siêu trait: impl sinh vào `impl_pairs`, enum Show sinh mã, Eq chặn sâu, 2026-11-14

| # | Việc | Sửa gì | Ca |
|---|---|---|---|
| 1 | **B3 — `impl TraitCon for T` bị `VS0403` oan khi `T derives Show`** | `sinh_derives` (lượt 1c) chạy **trước** `kiem_sieu_trait`; `sinh_derive` ghi `(kiểu, "Show"/"Eq")` vào `impl_pairs` **chỉ khi thật sự sinh được** (Eq bị bỏ vì `float`/hàm ⇒ **không** ghi ⇒ thiếu thật vẫn đỏ) | `trait/derive-show-sieu-trait` (xanh: `trait Hien: Show:` + `type P derives Show:` + `impl Hien for P` ⇒ `1`/`{x: 1}` hai đường); `trait/derive-sieu-thieu-show` (âm: bỏ `derives Show` ⇒ `VS0403`) |
| 2 | **B4 — `enum … derives Show` im lặng** | Chọn **sinh mã thật** (rẻ hơn lỗi tĩnh): `sinh_show_enum` — `fn show(self) -> str` trả **một** `InterpPart::Expr(self)` ⇒ đúng dạng `Value::Enum` lúc chạy (`A`, `B(2)`), **không** đệ quy vào phương thức; `derives Eq` cho enum **giữ nguyên** | `trait/derive-show-enum` (xanh: `m.show()`⇒`A`, `n.show()`/`print(n)`⇒`B(2)`); giữ xanh khối tài liệu **214** (`enum Shape derives Show`), `tests/trait.rs::derives_tren_enum`, `hir.rs` khối 214 (**6** item/**11** sym) |
| 3 | **B5 — `Eq` chỉ chặn `float` nông** | `co_float_sau` **đệ quy** qua `List`/`Map`/`Opt`/`Result`/`TypeRef{args}` (generic) — dùng **chung** cho `kiem_derives` (chẩn đoán) và `co_truong_khong_eq` (quyết định sinh mã) | 3 âm: `trait/derive-eq-list-float`, `derive-eq-map-float`, `derive-eq-opt-list-float` (`VS0403` nêu đúng kiểu); 1 xanh: `derive-eq-list-int` ⇒ `true` |
| 4 | **Số liệu MINOR** | Header `diag_cmp` **426/351/45/30**; `hir.rs` khối 214 ⇒ `(6, 0, 11)` (enum Show **sinh impl** thật: +1 item, +2 sym) | — |
| 5 | **Đo** | **782** test (21+96+18+154+24+62+9+**398**); sem **426/20 nhóm** (367 `.vis` + 50 `.vish`, **75** `.err`); `vis test` **426 ok**; `--vm` **426 ok + 7 SKIP**; parity **381/381** (45 ca lỗi biên dịch); differential **100.000/0 lệch** (248,0 s, 403/s, seed 3523158054); `VBC1` **53**; 7 sổ/30 mã; allowlist 6 | — |
| 6 | **Bất ngờ / nợ** | (a) `enum Show` **rẻ thật** — chỉ cần **một** phần nội suy (dùng `show` của **giá trị**), nên chọn sinh mã thay vì lỗi tĩnh: giữ được khối tài liệu 214 + `derives_tren_enum` xanh; (b) **`impl_pairs` là per-tệp** ⇒ siêu trait **bắc cầu** vẫn là nợ (`R10`), `impl` sinh từ derives ở **tệp khác** không thấy; (c) `co_float_sau` **không** soi **bản ghi lồng** (cần `TypeTable`, hàm thuần không có) — nợ; (d) `derives Json` vẫn **không** sinh mã (nợ cũ) | — |

## ST-60b — hotfix review độc lập: gán trong closure, đích lồng, shell-let, `std --tên`, 2026-11-14

| # | Việc | Sửa gì | Ca |
|---|---|---|---|
| 1 | **BLOCKER — gán trong closure** (hồi quy `dd04afb`) | Codegen: closure **bắt theo giá trị** nay **cấp ô cục bộ** cho từng biến bắt + **prologue** `LoadCaptured(i)` + `Assign(slot)` ⇒ **đọc và ghi** đều đi ô cục bộ (ghi không đụng biến ngoài, đọc lại thấy giá trị vừa ghi); **không** thêm tag `VBC1`, verifier/cache không đổi. Interp: đã đúng by-value — thêm nhánh `p.xs[0] = v` (một tầng trường) cho khớp codegen. Phụ: `()` trước đây mã hoá thành `Const(str "")` ⇒ VM in chuỗi rỗng còn interp in `()` — nay `PushUnit` **thật** (mọi chỗ giá trị đơn vị) | `gan_hai_duong`: `closure_gan_phan_tu_bien_bat` (**C1**), `closure_gan_bien_bat_co_tham_so` (**C2**), `closure_gan_truong_bien_bat`, `closure_long_hai_tang` (4 ca interp = `--vm` **byte-khớp**) |
| 2 | **`self.xs[0] = 9` / `p.xs[0] = 9` bị `VS0412` oan** | Tyck: **gốc đích gán** đi xuống qua `Field`/`Index` (`goc_dich`) thay vì chỉ nhận `Var` trực tiếp; đích hạ được = biến / `x.f` / `x[i]` / `x.f[i]` (`dich_ha_duoc`); codegen thêm đường `x.f[i] = v` (`Field`+`Set`+`With`+`Assign`), interp thêm nhánh tương ứng | 2 xanh (trong 4 ca trên: `var self`, `var p`), 1 âm: `gan_long_goc_let_bao_bat_bien` |
| 3 | **Đích lồng: thông điệp nêu đúng gốc** | `GAN_LONG` mới ("gán **lồng** … chưa hỗ trợ") khi **gốc đã là `var`**; gốc `let`/tham số vẫn nói **bất biến**; gốc không phải biến ⇒ "bản ghi bất biến". Áp cho cả thân `impl` (`kiem_gan_than`) | `gan_hai_duong`: `gan_long_goc_var_bao_long` (đòi chữ **lồng**, cấm câu let/tham số), `gan_long_goc_let_bao_bat_bien` (đòi let/tham số, cấm chữ lồng); sem `.err`: `loi/gan-long-goc-var`, `loi/gan-long-goc-let` |
| 4 | **`.vish` shell-let** (hồi quy nhẹ ST-59) | Luật `let` khai báo **một nguồn** nay đòi thêm: **mọi định danh trong RHS đã bind** (`dinh_danh_chua_bind`, bỏ tên trường sau `.`/khoá bản ghi) — `da_bind` = dựng sẵn + khai báo **trước** + **tham số `cmd`**. Parser giữ tập `da_bind` tăng dần; `tach`/`than_khai_lenh` cũng vậy. `let out = ls -l` ⇒ **shell-let**; `let b = a` (a đã bind) ⇒ **khai báo** | `gan_hai_duong::vish_shell_let_khong_thanh_khai_bao` (check xanh, run báo `out` **không** báo `l`); sem `.out`: `vish/cmd-than-khai-bien` |
| 5 | **`std.list.sort_by(…, --desc)` bỏ cờ im lặng** | `kiem_std` nhận `named` ⇒ **từ chối** mọi `--tên` (`VS0401`, nêu đúng tên); đường hạ `std_call_named` cũng **từ chối** (trước: `named: Vec::new()` nuốt cờ) — **không** thêm ngữ nghĩa `--desc` | `gan_hai_duong::std_khong_nhan_tham_so_ten` |
| 6 | **MINOR** | Header `diag_cmp` cập nhật **419/348/41/30** (số cũ 382/325/30/27); comment `khai.rs` "chỉ khi có `:`" đã sửa; **không** thêm SPDX cho `.out/.err` (đúng R3) | — |
| 7 | **Đo** | **782** test (21+96+18+154+24+62+9+**398**); sem **419/20 nhóm** (367 `.vis` + 50 `.vish`, 71 `.err`); `vis test` **419 ok**; `--vm` **419 ok + 7 SKIP**; parity **378/378** (41 ca lỗi biên dịch); differential **100.000/0 lệch**; `VBC1` **53** (không đổi); **7 sổ/30 mã** (không mã mới); allowlist **6** | — |
| 8 | **Bất ngờ / nợ** | (a) hồi quy C1/C2 **thật** và **cùng gốc**: `slot_cua` trả **chỉ số captured** cho `Assign`/`LoadLocal` ⇒ nay biến bắt có ô cục bộ riêng; (b) phát hiện thêm lỗi **cũ**: `()` mã hoá `Str("")` ⇒ lệch in ấn mọi hàm có giá trị đơn vị **ngầm** (đã sửa, `PushUnit`); (c) `if` **không** `else` trong **ngữ cảnh giá trị** cũng đã đúng `()`; **còn** nợ: `match` **không** khớp nhánh ở ngữ cảnh giá trị vẫn đẩy `false` thay vì `VS0501` (ghi `R10`); (d) shell-let **vẫn chưa hạ** ⇒ `let out = ls -l` xanh `check` nhưng `run` báo `out` chưa bind (nợ cũ); (e) `R1` buộc tách `FnSig` → `tyck/sig.rs` và `CodegenError` → `bytecode/cerr.rs`; (f) ca `std --tên` khoá ở **Rust** (HIR `{error}` không sinh bytecode hỏng) | — |

## ST-60 — VS2 #1: async (`async fn`/`await`/`spawn`), bộ lập lịch tất định một luồng, 2026-11-14

| # | Việc | Kết quả |
|---|---|---|
| 1 | **Không thêm tag `VBC1`** | Hạ async thành **builtin nội bộ** `$tac_vu`/`$spawn`/`$await` (tên có `$` ⇒ người dùng **không** viết được, `VBC1` **vẫn 53** tag, verifier/cache **không đổi**) + **một** runtime `vis-interp/src/rt.rs` dùng **chung** interp+VM. `Int` của ba builtin là **handle của host** (interp: `sym`; VM: chỉ số hàm bytecode) — **không** lộ ra chương trình |
| 2 | **Runtime `rt.rs`** | `Task{id, than, args, ten, span, trang_thai: Cell<Moi|DangChay|Xong>, kq: RefCell<Option<Result<Value, Trap>>>}`, `Scheduler{tasks: Vec<Rc<Task>>, ready: VecDeque, tiep, sau}`, `MAX_TASK_NEST = 256`; `await_task` (cache + chặn `await` chính nó ⇒ `VS0501` + chặn lồng), `join_all` **theo thứ tự tạo**, `wrap_task_error` ⇒ **`VS0506`** (message `tác vụ \`f\` lỗi: <mã gốc>: <msg>`, span = **lúc tạo**) |
| 3 | **Host** | `builtin::Host` thêm `sched(&self) -> &RefCell<Scheduler>`, `goi_than(fi, args, span)` (required), `ten_than(fi)` (default rỗng) — **2** hiện thực (interp/VM); `builtin::call` thêm nhánh `$…` kiểm **`Int` handle** phòng thủ (bytecode đột biến ⇒ `VS0501`, **không** panic); `$await` nhận `Value::Task` |
| 4 | **Kiểu** | `Task[T]` = `TypeRef{name:"Task",args:[T]}` (thêm `("Task",1)` ở `resolve_named`, **bỏ** `Task` khỏi `la_kieu_ngoai_vs0`); `FnSig.is_async`; gọi `async fn` ⇒ `Task[ret]`; `Await` đòi **đúng** `Task[T]` ⇒ `T`, sai ⇒ `VS0412`; `Spawn` chỉ **tên** hàm async cấp tệp, đối số **vị trí**; `async fn` làm **giá trị** ⇒ `VS0412`; luật `async` trên method/`cmd` ⇒ `VS0412` (**phòng thủ** — parser chưa nhận hai dạng này, xem #7) |
| 5 | **HIR/parser** | `HExpr::Spawn{callee,args,named,span}` (**một** nút mới + `span()`/`sexp()`); `spawn` = **soft keyword** (`Ident "spawn"` + token kế **Ident** ⇒ `NodeKind::SpawnExpr`; `s.spawn(1)` vẫn là **phương thức** — nhánh đặt **trước** nhánh `Ident` chung) |
| 6 | **Codegen/VM** | `Gen.async_fns: HashSet<u32>` (điền lúc cấp chỉ số hàm); call-site async ⇒ `Const(Int fi)` + args + `CallBuiltin($tac_vu, n+1)`; `Spawn` ⇒ `$spawn`; `Await` ⇒ `$await`; span = span nút. VM: `sched` + `goi_than` ⇒ `goi_ham_bytecode(VmFunc{idx,captured:vec![]})` + **join trước `take_stdout`**; interp: join trong `run_program` (**chỉ khi main thành công**) |
| 7 | **Ngữ nghĩa chốt** | gọi `async fn` **không** chạy thân ⇒ `Task`; `spawn` = tạo + `ready`; `await` `Moi` ⇒ chạy ngay, `DangChay` ⇒ `VS0501`, `Xong` ⇒ **cache**; cuối chương trình join **mọi** task theo **thứ tự tạo**; thân lỗi ⇒ `VS0506` áp **cả** `await` lẫn join; lồng > 256 ⇒ `VS0501` |
| 8 | **Sổ mã** | **`VS0506`** thêm **cuối** `RUNTIME_CODES` (3 → **4**, index 0..2 giữ nguyên) ⇒ `all_codes()` **29 → 30**; sửa đủ khoá sổ (`vis-diag` unit ×2, `messages.rs`) |
| 9 | **Ca** | nhóm **mới `test/sem/async/`** ⇒ **20 nhóm**, **22 ca** (15 xanh + 7 âm: `VS0412` ×4, `VS0201` ×1, `VS0506` ×2); **`async_10k.rs`** (10k task **thật**, không `#[ignore]`: interp **309 ms** / VM **166 ms**, hai đường **cùng byte**, đủ 10.001 dòng); **`async_vis.rs`** (2 ca khoá nợ: `async` trong `impl`/`cmd` ⇒ **`VS0201`**) |
| 10 | **Đo** | **774** test (21+96+18+154+24+62+9+**390**); sem **416/20** (367 `.vis` + 49 `.vish`, 69 `.err`); `vis test` **416 ok**; `--vm` **416 ok + 7 SKIP**; parity **377/377** (39 ca lỗi biên dịch); differential **100.000/0 lệch** (208,7 s); `VBC1` **53**; **7 sổ/30 mã**; allowlist **6** |
| 11 | **Bất ngờ / nợ** | (a) VD-95 (4) nói `async` trên method/`cmd` ⇒ `VS0412`, **đo thật**: parser chưa nhận ⇒ ra **`VS0201`** (cú pháp) **trước** tầng kiểu; luật `VS0412` giữ làm **phòng thủ**, ca khoá là `async_vis.rs` — **phải đổi kỳ vọng sang `VS0412`** khi parser mở hai dạng này; (b) `async fn` **không** nhận `--tên` (nợ); (c) **không** nhường giữa thân ⇒ **chưa** song song thật (chốt VD-90 (2)); (d) **không** GC (chu trình `Rc`); (e) `await` lồng sát 256 tầng sẽ chạm ngưỡng khung gọi; (f) 10k task trong **một** vòng lặp với `spawn` phẳng: 309/166 ms |

## ST-59 — nợ shell: `let` suy kiểu, biến làm **stage đầu**, thân `cmd` khai báo + lệnh, 2026-11-14

| # | Việc | Kết quả |
|---|---|---|
| 1 | **Luật `let` — một nguồn** | `let_la_khai` chuyển sang **`vis_syntax::let_la_khai`** (parser **và** tầng hạ dùng **cùng** luật; trước có **hai bản sao** lệch nhau): khai báo khi RHS **không** `\|`/`;`/`>`/`<`/`&`, **không** là **một từ trần** (trừ `true`/`false`/`none`), và parse được như **biểu thức script**; **bỏ** đòi chú thích kiểu ⇒ `let x = 1`, `let xs = [1, 2, 3]` ở vùng item **bind** |
| 2 | **Biến làm stage đầu** | `ha_stage`/`kieu_stage`: token đầu stage là **biến giá trị đã bind** (`Var/Param/PatBind/ElseBind` — **không** Builtin/Item/Generic) ⇒ **stage biểu thức** `Var{sym}`; định danh **chưa** bind vẫn đi đường **lệnh**/chương trình ngoài (§9.5 giữ nguyên: `git status`, `ls .`). `kieu_stage` trả **kiểu tĩnh** của biến (nếu có chú thích) ⇒ hook `where`/`get`/`select`/`sort_by` kiểm **tĩnh** theo kiểu biến |
| 3 | **Thân `cmd` khai báo + một pipeline** | `KieuThan::KhaiLenh` (mới) + `than_khai_lenh` **một nguồn** (`vis_syntax`): thân = **vùng `let …`** ở đầu + **đúng một** dòng cuối là pipeline/biểu thức; parser **gom raw** (không parse lệnh trên dòng khai báo ⇒ hết `VS0201: mong lệnh, thật [`) ; hạ bằng **hai Lowerer con** lồng khung (khai báo theo **Script**, cuối theo **Command**, `type_env` chia sẻ) ⇒ `ys \| take 1` thấy `ys`. Nhiều pipeline / khai báo **sau** pipeline ⇒ vẫn **`VS0201`** |
| 4 | **Ca** | **6 ca `test/sem/vish/`** (nhóm `vish` 43 → **49**): `let-suy-kieu` (+`.out`), `stage-dau-bien` (+`.out`), `than-khai-lenh` (+`.out`), `let-ngoai-vung-khai`/`stage-dau-chua-bind`/`than-hai-pipeline` (+`.err` = VS0201/VS0501/VS0201); **Rust**: `vish_cmd_body` đổi ca `VS0201` cũ thành **chạy được** (+ca khai báo **sau** pipeline vẫn `VS0201`) ⇒ **16** test tệp đó |
| 5 | **Đỏ trước** | `JOURNAL/st59-red-sem.log` sha256 `47d2dc32…` (chạy binary **trước** sửa: `let-suy-kieu` VS0201, `stage-dau-bien` VS0501 "chương trình ngoài `xs`", `than-khai-lenh` VS0201; 2 ca âm **đỏ sai mã**: `let-ngoai-vung-khai` VS0501 thay vì VS0201); `JOURNAL/st59-red-rust.log` sha256 `5a5bc6a8…` (`than_khai_bao_roi_lenh_chay_duoc` FAILED exit 2 ≠ 0; `sem` đỏ 4 ca `vish/`) |
| 6 | **Hợp đồng giữ nguyên** | thân **pipeline-only** và thân **biểu thức trần** của `cmd` **không đổi**; `let out = ls` / `let x = git` (một **từ trần**) vẫn là **shell-let** stage; `let x: int = ls \| grep foo` (có `\|`) vẫn stage (golden `test/cmd` không đổi) |
| 7 | **Đo** | **771** test (21+96+18+154+24+62+9+**387**); sem **394/19 nhóm** (345 `.vis` + **49** `.vish`, 62 `.err`); `vis test` **394 ok**; `--vm` **394 ok + 7 SKIP** (ca **định danh chưa bind** cần host ⇒ VM bỏ qua); parity **360/360** (34 ca lỗi biên dịch); differential **100.000/0 lệch** (202,8 s, 493/s, seed 3523158054); `VBC1` **53** tag; **7 sổ/29 mã** (không mã mới — dùng lại `VS0201`/`VS0501`); allowlist **6** |
| 8 | **Bất ngờ / nợ** | (a) luật `let` trước đây có **hai bản sao** (parser `parse/cmd.rs` + `lower/khai.rs`) — đã hợp nhất về `vis_syntax`; (b) `let` **sau** stage nay là **lỗi rõ** `VS0201` ("khai báo phải đứng trước stage") thay vì bị **bỏ im lặng** rồi `VS0501` khó hiểu; (c) shell-let (`let out = … \| …`) vẫn **chưa** hạ (nợ cũ, không thuộc ST-59); (d) `where it > 1` với biến **có chú thích** kiểm tĩnh được, **không** chú thích thì `{error}` (như lead cho phép); (e) nhóm nhiều pipeline + kiểu tĩnh **lấy stage đầu** vẫn **cắt** (nợ R9-10 phần còn lại); (f) `lower/cmd.rs` **1491** dòng (sát R1) |

## ST-58 — `Ty::Any` **xoá hẳn**: biến kiểu thật cho `std.*` (VD-94/VD-99), 2026-11-14

| # | Việc | Kết quả |
|---|---|---|
| 1 | **`Ty` mới** | `Ty::{Var(u8), List(&'static Ty), Map(&'static Ty, &'static Ty)}` thay `Any` (xoá **hẳn**); `const FUNCS` dùng **promotion** (`Ty::List(&Ty::Var(0))`) — không cần `static` có tên; bỏ `Ty::kieu()` (chỗ `else { continue }` im lặng) → `sang_type()` **tổng**: `Var(i)` ⇒ `TypeRef` tên `T`/`K`/`V` (tái dùng `TypeRef`, **không** thêm `Type::Param`) |
| 2 | **`kiem_std`** | hợp nhất **trái→phải** bằng `hop_nhat` + `thay_gen` sẵn có: biến xuất hiện 2 lần phải `same`, `{error}` không gán, sai hình dạng ⇒ `VS0412`, số đối số ⇒ `VS0401` (giữ); biến **không gán** ⇒ `{error}` (giữ `json.decode`, VD-66); **không mã mới** |
| 3 | **Ngoại lệ có ghi** | `list.sort_by[1]` = `Ty::FnKhoa` (lambda chưa chú thích ⇒ `{error}`; `Type` không có union `int|str`) ⇒ `continue` trong hợp nhất, **tầng chạy** kiểm (ST-56 giữ `ChiIntHoacStr`) — ca trộn `int`/`str` vẫn là `VS0501` **lúc chạy** |
| 4 | **16/38 chữ ký đổi** | `std.list` 8 (`list[T]`; `chunk -> list[list[T]]`), `std.map` 5 (`map[K,V]`, `keys/values`, `get_or`, `merge`, `remove`), `std.json` 3 (`encode/encode_pretty (T) -> str`, `decode (str) -> Var(2)`); `std.str/path/time/fs` **không đụng** |
| 5 | **Phía interp** | `kiem()` (`vis-interp/src/stdlib.rs`) thêm nhánh `Var/List/Map => true` (thân hàm tự kiểm như trước với `Any`) ⇒ **73** ca `test/std` + differential **không đổi hành vi** |
| 6 | **Ca** | +2 ca xanh `test/std/{list_generic_ret,map_generic_ret}.vis` (**khoá kiểu trả**: `list.sort([3,1,2]) -> list[int]`, `list.chunk -> list[list[int]]`, `map.get_or -> V`, `map.keys -> list[K]`); **3 ca âm tĩnh** (`list.concat` trộn kiểu, `list.sort("x")`, `map.get_or` sai `V`) khoá bằng **Rust** (`vis-cli/tests/stdlib.rs`) — `test/sem` **không nạp** `use std.*` (hạn chế đã biết từ ST-56, ghi JOURNAL); **4 ca Rust mới** `vis-types/tests/stdlib_ty.rs` (tổng `sang_type`, `Var` ∈ 0..3, đúng hình dạng 16 chữ ký, `sort_by` giữ `FnKhoa`) |
| 7 | **3 ca cũ dựa `Any`** | `vis-cli/tests/stdlib.rs`: `list.sort("x")` từ lỗi **chạy** `VS0501` → **tĩnh** `VS0412` (mạnh hơn; `check`=1 ↔ `run`/`--vm`=2); `sort_by_str.rs` ca **TRON** dựng danh sách trộn bằng `json.decode` từ dữ liệu **không kiểu** ⇒ vẫn là `VS0501` **lúc chạy** nêu cả `int`/`str`; `gan_hai_duong.rs` ca "kiểu không rõ" đổi sang **tham số generic** `fn gan[T](x: T)` (T bị xoá ⇒ `{error}`) — vẫn khoá hai đường **cùng** chẩn đoán lúc chạy |
| 8 | **Đỏ trước** | `JOURNAL/st58-red-sem.log` sha256 `fca3d833…` (3 ca âm: **1 chạy được**, **1 lỗi chạy `VS0501` thay vì tĩnh**, **1 in 1**; 2 ca xanh đã xanh — khoá kiểu trả); `JOURNAL/st58-red-rust.log` sha256 `b75a7728…` (target `stdlib_ty` **không biên dịch** vì thiếu `Ty::Var/List/Map` + `sang_type`; `sai_kieu_bi_tyck_bat_tinh` FAILED: `check` exit 0 ≠ 2) |
| 9 | **Đo** | **770** test (21+96+18+**154**+24+62+9+**386**); sem **388/19 nhóm** (345 `.vis` + 43 `.vish`, 59 `.err` — **không đổi**); `test/std` **73**; `vis test` **388 ok**; `--vm` **388 ok + 6 SKIP**; parity **356/356** (32 ca lỗi biên dịch); differential **100.000/0 lệch** (193,2 s, 518/s, seed 3523158054); `VBC1` **53** tag; **7 sổ/29 mã**; allowlist **6** |
| 10 | **Nợ / bất ngờ** | (a) `Ty::Fn` thật cho `sort_by` (nay `FnKhoa` = ngoại lệ); (b) map khoá **không-`str`** (tyck literal map vẫn đòi `str`); (c) thứ tự khoá map (nợ cũ); (d) 3 ca âm tĩnh nằm ở **Rust** thay vì `test/sem` vì harness sem chạy **in-process** không nạp `use std.*`; (e) **không** đổi `all_codes()` |

## ST-57 — mồ côi **đa tệp** + **siêu trait** + `derives` **sinh mã** (VD-93/VD-98), 2026-11-14

| # | Việc | Kết quả |
|---|---|---|
| 1 | **A1 — mồ côi đa tệp** | `TraitSig{name, supertraits, methods}` **công khai**; `LowerOptions.trait_ngoai`; `modules.rs` xuất `pub_traits` (trait `pub` của từng tệp) và nhập cho tệp `use`; `kiem_impl_khop` tra **trait nội tệp ∪ trait nhập**; **mồ côi thật** (trait **nhập** *và* kiểu **nhập**) ⇒ `VS0403` thông điệp riêng ("mồ côi: trait `X` và kiểu `Y` đều ở module khác"); trait nội tệp **hoặc** kiểu nội tệp ⇒ **hợp lệ** |
| 2 | **A2 — siêu trait** | `collect_trait` gom `supertraits` (cú pháp **`trait B: A:`** — parser đã sẵn); `collect_impl` ghi `impl_pairs (kiểu, trait)`; `kiem_sieu_trait` chạy **sau** vòng 1b ⇒ thứ tự `impl` tự do; thiếu ⇒ **`VS0403`** ("`impl B for …` thiếu `impl A for …`"); kiểm **một tầng** (bắc cầu = nợ) |
| 3 | **A3 — `derives` sinh mã** | Lượt **1c** (`sinh_derives`) **trước** mọi lượt hạ thân ⇒ `p.show()` ở câu **trước** khai báo vẫn phân giải; sinh `HImpl` **inherent** (`trait_name: None`) nối **cuối** `items`; tên nội bộ `<Kiểu>.show`/`<Kiểu>.eq` (không `VS0302`); người dùng viết tay **thắng** (kiểm `impls` trước khi sinh) |
| 4 | **`show` đúng byte** | Body `HExpr::Interp` dựng **đúng** `Value::show` của giá trị bản ghi **lúc chạy** — tên động **rỗng** ⇒ `{x: 1, ten: a}` (khớp **từng byte** `print(p)`; **không** phải `P(...)`); **không** dùng `+` nối chuỗi (tyck không có `(Add, Str, Str)`) |
| 5 | **`eq`** | Body `self == other` (hai đường gọi cùng `eq_value`); record **và** enum |
| 6 | **Không sinh mã** (`VS0403` **tĩnh**) | kiểu **generic**; `Eq` với trường `float`/`Fn` (số thực so sánh **không toàn phần**); `Json` giữ **kiểm-only**; **`enum show` cắt** (nợ — `enum eq` có) |
| 7 | **Ca** | **6 ca sem** (`trait/sieu-trait-du`, `sieu-trait-thieu` VS0403, `derive-show`, `derive-eq`, `derive-eq-float` VS0403, `derive-nguoi-dung-thang`) + **9 ca Rust** (7 `vis-types/tests/trait.rs`: siêu trait ×2, derives ×5; 2 `vis-cli/tests/modules.rs`: `mo-coi` xanh, `mo-coi-that` VS0403) — interp ↔ `--vm` **cùng byte** |
| 8 | **Đỏ trước** | `JOURNAL/st57-red-sem.log` sha256 `98461b07…` (6 ca: 4 đỏ đúng lý do; `sieu-trait-du` + `derive-nguoi-dung-thang` **đã xanh từ trước** — giữ làm ca khoá); `JOURNAL/st57-red-rust.log` sha256 `1a66594e…` (7/9 đỏ; `mo-coi-that` + `sieu-trait-du` xanh-already, `mo-coi-that` khoá **thông điệp**) |
| 9 | **Đo** | **765** test (21+96+18+**150**+24+62+9+**385**); sem **388/19 nhóm** (345 `.vis` + 43 `.vish`, 59 `.err`); `vis test` **388 ok**; `--vm` **388 ok + 6 SKIP**; parity **356/356** (32 ca lỗi biên dịch); differential **100.000/0 lệch** (198,5 s, 504/s, seed 3523158054); `VBC1` **53** tag (không bump); **7 sổ/29 mã**; allowlist **6** |
| 10 | **Bất ngờ / nợ** | (a) `enum show` **cắt**; (b) `derives Json` **chỉ kiểm**, không sinh; (c) siêu trait **bắc cầu** = nợ; (d) dispatch `impl` **xuyên tệp** vẫn per-tệp (bảng `impls` per-tệp); (e) tyck **chưa** kiểm kiểu trả phương thức `impl` (nợ ST-56b); (f) tên động bản ghi **rỗng** ở runtime (do literal) ⇒ `show` sinh theo dạng **ẩn danh**; (g) R1: tách `LowerOptions`/`NamePolicy` sang `lower/options.rs` (60 dòng) — `lower.rs` 1467 dòng |

## ST-56b — một ngữ nghĩa cho **phép gán** + exit codegen (VD-97), 2026-11-14

| # | Việc | Kết quả |
|---|---|---|
| 1 | **Tyck — `self` bất biến** | thân phương thức `impl` **chưa** được tyck đầy đủ (khung VS1) ⇒ thêm `kiem_impl`/`kiem_gan_than`: chỉ kiểm **đích gán**; `self` **không** `var` là **tham số bất biến** ⇒ `self.x = v` / `self[i] = v` ⇒ **`VS0412`** tại **phép gán** (giữ nhánh `kiem_dich` cho thân hàm thường); `var self` **được**; `let`/tham số thường giữ `VS0412`; `+=` trên trường/phần tử ⇒ `VS0412` (defense; hiện cú pháp đã chặn `VS0201`) |
| 2 | **Codegen — `x[i] = v`** | trước: `Set` + **`Pop`** ⇒ VM **im lặng** bỏ kết quả (`xs[0] = 5` không có tác dụng!). Nay: `LoadLocal` + index + value + `Set` + **`Assign(slot)`** ⇒ sửa **tại chỗ** (list theo chỉ số, map thay/thêm khoá); `x.f = v` giữ `With` + `Assign` (rebind, **giá trị**); base không phải biến `var`/gán ghép ⇒ `Unsupported("assign-index"/"assign-op")` (tyck đã chặn trước) |
| 3 | **Interp — cùng byte** | `gan_truong` (đúng `Instr::With`, **cùng** thông điệp "bản ghi `T` không có trường `f`"/"`with` không dùng được cho …") và `gan_chi_so` (đúng `Instr::Set`: list OOB ⇒ "chỉ số N ngoài phạm vi", map thay **hoặc thêm**, khác ⇒ "gán chỉ số không hợp lệ"); thứ tự đánh giá **giống codegen** (nạp ô → chỉ số → giá trị); ghi lại ô qua `ghi_o` (chốt 2 mutability) |
| 4 | **CLI — exit codegen** | `run --vm` gặp lỗi codegen: in `vis run --vm: codegen lỗi: …` **và** **exit 1** (trước: exit 0 — im lặng); `--tap` thêm `not ok 1 - <tệp> (codegen)`. Không ca sem nào dựa vào exit 0 (quét **0** ca có "codegen lỗi") |
| 5 | **Ca** | **6 ca sem** (`ban-ghi/self-gan-truong` VS0412, `ban-ghi/var-self-gan`, `ban-ghi/var-truong-gan`, `list/var-chi-so-gan`, `list/let-chi-so-gan` VS0412, `map/var-khoa-gan`) + **9 ca Rust** `crates/vis-cli/tests/gan_hai_duong.rs` (gồm **exit codegen**); interp ↔ `--vm` **cùng byte** ở mọi ca chạy được |
| 6 | **Đỏ trước** | `JOURNAL/st56b-red-sem.log` sha256 `75e9ba9238b76631b59256e680e5153ee81fc1546606078038a53a3b09adc67f` — 6 ca: **4 đỏ đúng lý do**, `list/let-chi-so-gan` **đã xanh từ trước** (giữ làm ca khoá); `JOURNAL/st56b-red-rust.log` sha256 `7e00290364340ec52ed1fd5d979bb974f47b63efd111a4f2a0cccfd99d920c7d` — **9/9 đỏ** |
| 7 | **Đo** | **756** test (21+96+18+143+24+62+9+**383**); sem **382/18 nhóm** (339 `.vis` + 43 `.vish`, 57 `.err`); `vis test` **382 ok**; `--vm` **382 ok + 6 SKIP**; parity **352/352** (30 ca lỗi biên dịch); differential **100.000/0 lệch** (194,7 s, 514/s, seed 3523158054); `VBC1` 53 tag; **7 sổ/29 mã**; allowlist **6**; `gate --fast` **PASS** `2026-11-13T22:17:37Z` |
| 8 | **Bất ngờ / nợ** | (a) **Thân `impl` chưa tyck** — chỉ đích gán được kiểm; kiểm kiểu **đầy đủ** cần **dispatch phương thức** trong tyck ⇒ **nợ riêng**. (b) `p.x += 1` là **lỗi cú pháp `VS0201`** (front end chặn **trước** tyck) — parity giữ. (c) `vis test`/`--vm` **0 ca** dựa vào exit-0 codegen |

## ST-56 — nợ lõi A: khoá `str` cho `std.list.sort_by` + `&self` + `ref` identity + `Hop[Hop[int]]` (2026-11-14)

| # | Việc | Kết quả |
|---|---|---|
| 1 | **Nợ 1 — khoá `str`** | `KieuKhoa::ChiIntHoacStr` trong **cùng** lõi `sap_xep_theo_khoa` (khoá 1 lần, **ổn định**): `int` **hoặc** `str`, **một** kiểu/lần gọi; trộn ⇒ `VS0501` "khoá trộn kiểu … (`int` và `str`)"; khoá khác (`bool`…) ⇒ `VS0501` nêu giá trị thật; **byte UTF-8** = đúng `cmp_value` ⇒ **nhất quán** shell `sort_by`; `Ty::FnIntInt` → **`Ty::FnKhoa`**, `ten()` = `fn(T) -> int | str`; ca `int` cũ không đổi |
| 2 | **Nợ 2 — `ref(expr)`** | `HExpr::Ref` hạ **identity** ở **cả** interp (`self.eval(expr)`) **và** codegen (`self.expr(expr)`) — hết `Unsupported("ref")`; tyck **giữ nguyên kiểu** trong (bỏ `Type::Error`). `ref(1)+1`=2, `ref(ref("a"))`=`"a"`, `ref(p).x`=trường, `[ref(2),ref(3)]`=`[2, 3]`; **CHƯA CÓ BORROW** (nợ VS2+): `x=2` sau `ref` ⇒ `r` vẫn `1`; `ref(1,2)` vẫn `VS0201` |
| 3 | **Nợ 3 — `&self`** | `param()` nhận `&self`/`& self` ⇒ **cùng** nút `Param` như `self`; lowering ⇒ `is_self`, **không** `is_var_self` ⇒ hạ **y như** `self`; chạy **cả** `impl` **và** `trait`; `fn bump(self)` cũ không đổi; `&self: int` vẫn `VS0201` |
| 4 | **`Hop[Hop[int]]` — ĐÃ XANH TỪ TRƯỚC** | probe HEAD `4f88ffa`: **5** biến thể chạy hai đường (annotation, `list[…]`, tham số hàm, `impl Hop[Hop[int]]`, ba tầng); **không sửa mã**, thêm **2 ca sem + 2 ca Rust khoá** (dương đủ biến thể; âm `VS0412` payload sâu sai kiểu) |
| 5 | **Sửa kèm (bắt buộc)** | *arity guard* ở `builtin::call` (`open`/`render`/`get`/`select`/`sort_by`): số base module `fuzz_mutate` 318 → 321 làm lộ **panic** `index out of bounds` khi bytecode **đột biến** gọi `render` rỗng; guard **không** đổi mã hợp lệ (front end đã kiểm số đối số); 3 lần fuzz **0 vi phạm** |
| 6 | **Đỏ trước** | `JOURNAL/st56-red-sem.log` sha256 `291123e73347c2342e0ede0f305ca626650bf9387590ea1cbbfd3923b37759cf` — 8 ca: **5 đỏ đúng lý do**, **3 xanh từ trước** (`ref-hai-doi-so` `VS0201`, 2 ca `hop-long-*`) — khai rõ, không giả vờ đỏ. `JOURNAL/st56-red-rust.log` sha256 `728659fd24d99339c9626ee2b7d88767be411744e66b7db144dbea09f8800f96` — 17 ca Rust: **11 đỏ** / 6 xanh-sẵn |
| 7 | **Đo** | **747** test (21+96+18+143+24+62+9+**374**); sem **376/18 nhóm** (333 `.vis` + 43 `.vish`, 55 `.err`); `vis test` **376 ok**; `--vm` **376 ok + 6 SKIP**; parity **348/348** (28 ca lỗi biên dịch); differential **100.000/0 lệch** (202,7 s, 493/s, seed 3523158054); `VBC1` **53** tag; **7 sổ/29 mã**; allowlist **6**; `gate --fast` **PASS** `2026-11-13T21:29Z` |
| 8 | **Bất ngờ (không sửa)** | (a) `self.field = v`: interp `VS0501` ↔ VM **thực thi** — lệch **có từ trước**, tái hiện với `fn bump(self)`; sửa = đổi ngữ nghĩa ⇒ chờ Agent 0. (b) `run --vm` codegen `Unsupported` ⇒ **exit 0** (nhánh trả `(…, 0)`). (c) Ca sem cho `std.list.sort_by` chuyển sang **`test/std/`** + Rust vì harness `test/sem` chạy **in-process** (không nạp `use std.*`) |

## ST-55 — báo cáo R9 (chốt VSH2 + kế hoạch VS2), 2026-11-14

| # | Việc | Kết quả |
|---|---|---|
| 1 | **R9** | `agent/reports/R9-viscript.{md,json,json.license}` — **11 khoá**, `round=9`, `head=8c8c36a`; `report-check --all` **PASS** (9 báo cáo, 61 hàng QUEUE) |
| 2 | **Số liệu** | **730** test (21+96+18+143+24+**62**+9+**357**); sem **370/18**; `vis test` **370 ok**, `--vm` **370 ok/6 SKIP**; parity **344/344**; differential **100k/0 lệch** (213,2 s, seed 3523158054); fuzz 1 phút **2.996.353 ca/0 vi phạm**; `VBC1` **53**; **7 sổ/29 mã**; **`CODEGEN_TIN_TYCK = 0`**; ấm **11,12/11,27/11,17 ms**; allowlist **6** |
| 3 | **Bảng vòng 9** | ST-53 `d68eb16`, ST-54 `4627fbb`, ST-55 (commit này); VD-86..VD-88 mỗi cái một dòng |
| 4 | **§19 + VS2** | bảng **8 hạng mục VS2** (async, kênh, net/http, Landlock thật, LSP, gói+registry, host windows/wasm, hợp chuẩn 4 nền) — mỗi mục **tiêu chí đo được + ước lượng**; nói rõ **ViScript chưa hoàn thiện toàn bộ** |
| 5 | **Lỗ hổng** | **đóng** 3 (`to json`, hợp nhất comparator, `CODEGEN_TIN_TYCK = 0`); **còn** **10 nợ có chủ đích** + **VS2** |
| 6 | **Đề xuất** | AGENTS §8 (câu R9), `viscript/AGENTS.md`, QUEUE vòng 10: ST-56 nợ lõi (tách ≤3 h), ST-57 miếng VS2 đầu (async+kênh), ST-58 R10 |

 (2026-11-14)

| # | Việc | Kết quả |
|---|---|---|
| 1 | **Dữ kiện** | Walker HIR 370 ca: **0 nút `HExpr::Error`** / **0 kiểu `{error}`** (cả 318 ca hợp lệ) ⇒ không thể chặn bằng soi kiểu |
| 2 | **Cửa duy nhất** | `compile_kiem(hir, symbols, text, chan_doan)`: lỗi `Severity::Error` đầu tiên ⇒ `Err(ChanDoan{short,dong,cot,msg})`; cảnh báo không chặn; `compile()` cũ = `compile_kiem(...,&[])` |
| 3 | **Phòng thủ** | `HExpr::Error` ⇒ `Err(Unsupported("error-expr"))` (trước phát chuỗi rỗng) |
| 4 | **Hằng số** | `CODEGEN_TIN_TYCK` **21 → 0**; `diag_cmp` dùng `compile_kiem` + đủ 3 tầng, **26/26 bị từ chối**, so đủ mã/dòng/cột/message |
| 5 | **Đo** | **718 → 730** test (+12 `tin_tyck.rs`); `vis test` **370 ok**; allowlist **6** |

 + hợp nhất `sort_by` (2026-11-14)

| # | Việc | Kết quả |
|---|---|---|
| 1 | **`to json`** | stage lệnh ⇒ `Call(std.json.encode, [prev])`, `--pretty` ⇒ `encode_pretty`; **không** cần `use std.json`; thiếu pipe/cờ lạ ⇒ `VS0602`; bản ghi **ẩn danh** đúng khoá/thứ tự |
| 2 | **Một lõi so sánh** | `sap_xep_theo_khoa` dùng chung: shell `sort_by` (`Moi`) + `std.list.sort_by` (`ChiInt`, giữ `FnIntInt`); ổn định, `--desc` đảo comparator |
| 3 | **R1** | chi tiết `to json` đặt trong `lower/sugar.rs` (`ha_to_json`), `cmd.rs` 1508 → **1454** dòng |
| 4 | **Đo** | **704 → 718** test (+14 `json_sort.rs`) + 3 `.vish` + `mock.json`; `vis test` **370 ok**; allowlist **6**; parity 344/344 |
| 5 | **Nợ** | khoá `str` cho `std.list.sort_by` (hợp đồng cũ `FnIntInt` giữ nguyên) |

 (chốt VSH1c), 2026-11-14

| # | Việc | Kết quả |
|---|---|---|
| 1 | **R8** | `agent/reports/R8-viscript.{md,json,json.license}` — **11 khoá**, `round=8`, `head=a44c357`; `report-check --all` **PASS** (8 báo cáo, 58 hàng QUEUE) |
| 2 | **Số liệu** | **704** test (21+96+18+143+24+50+9+343); sem **370/18**; `vis test` **370 ok**, `--vm` **370 ok/6 SKIP**; parity **344/344**; differential **100k/0 lệch** (211,5 s, seed 3523158054); fuzz 1 phút **3.023.954 ca/0 vi phạm**; `VBC1` **53**; **7 sổ/29 mã** (`VS0405` sổ **kiểu**); ấm **10,87/10,70/10,96 ms**; allowlist **6** |
| 3 | **Bảng vòng 8** | ST-48 `739fc3e`, ST-49 `e0c084a`, ST-50 `da50f28`, ST-51 `e721858`, ST-52 (commit này); VD-81..VD-85 mỗi cái một dòng |
| 4 | **Lỗ hổng** | **đóng** 5 (cmd pipeline, nhóm/cờ/nhiều dòng/một đường hạ, một mã `VS0405`, `where` tĩnh, stage đầu biểu thức); **còn** 9 mục + **nợ lõi 9** + **VS2** |
| 5 | **§19** | VSH1c DONE; **ViScript chưa hoàn thiện toàn bộ** — khoảng cách = nợ lõi + VS2 |
| 6 | **JOURNAL** | tệp **mới** `agent/JOURNAL/2026-11-14.md` (sang ngày; quy ước tệp theo ngày — ghi rõ) |

 (2026-11-13)

| # | Việc | Kết quả |
|---|---|---|
| 1 | **Văn phạm hẹp** | chỉ chế độ lệnh: dòng đầu `[`, `{`, **chuỗi** + có `|` độ sâu 0 + tiền tố `Script` sạch ⇒ stage đầu là **biểu thức**; không có `|` ⇒ `VS0201` |
| 2 | **Không nhận** | **số** (`1 \| …` vốn là lệnh ngoài) và **biến** (`x \| …` giữ nguyên lệnh ngoài) — tránh nhập nhằng §9.5 |
| 3 | **Hạ** | `CmdGroup` **không ngoặc** dùng lại đường `( … )`; **`{…}` bọc list 1 phần tử** ⇒ `where`/`take`/`select`/bảng chạy thật, interp ↔ `--vm` cùng byte |
| 4 | **Không hồi quy** | golden ca 24 + `(a \| b)` xanh; `take 1` vẫn lệnh (VS0602); `ls`/nhóm-ngoặc cũ y hệt; sửa **kèm lý do** ca `khe_ong` cũ |
| 5 | **Đo** | **692 → 704** test (+12 `stage_dau.rs`); `vis test` **370 ok**; parity 344/344; diff 100k 0 lệch; allowlist **6** |

 + `where` tĩnh (2026-11-13)

| # | Việc | Kết quả |
|---|---|---|
| 1 | **Một mã** | `VS0605` **đổi tên** thành **`VS0405`**, chuyển sang **`TYPE_CODES`** (trường là lỗi kiểu): `06xx` 5→**4**, `04xx` 4→**5**, `all_codes()` **vẫn 29**, `BOOKS` 7, `next_block()` 801 |
| 2 | **Ngữ nghĩa** | `kiem_field` phát **VS0405** cho bản ghi/enum; kiểu không phải bản ghi giữ **`VS0403`** (phương thức/khai báo); câu vi/en/ja trung tính, ngữ cảnh trong câu |
| 3 | **`where` tĩnh** | nhánh **`kiem_builtin("where")`** kiểm thân lambda (kể cả **trong ngoặc**) ra `bool` ⇒ **VS0412**; lambda trong ngoặc được gán kiểu tham số = pty ⇒ `x.khong_co` ra **VS0405** |
| 4 | **Dispatch** | tyck gọi `kiem_builtin` cho cả **lệnh shell** (`where`/`each`/`select`/`get`/`sort_by`) |
| 5 | **Đo** | **680 → 692** test (+12 `ma_truong.rs`); `vis test` **370 ok**; parity 344/344; diff 100k 0 lệch; tách `tyck/truong.rs` (R1); allowlist **6** |

 (2026-11-13)

| # | Việc | Kết quả |
|---|---|---|
| 1 | **Parser** | nhóm chứa `|` **hợp lệ** (bỏ chặn parse; lỗi nội dung ở tầng hạ); newline **không** kết thúc nhóm — quét tới `)` cùng độ sâu ⇒ `( 1 +
 1 )`, `( ls … |
 where … )` là **một** nhóm. Golden ca `ls \| (a \| b)` đổi `!ERR` → sexp hợp lệ **kèm lý do (APPROVED)** |
| 2 | **Hạ một đường** | **xoá** `lower_nhom_lenh` tự-dựng-Call; `lower_nhom` = newline→dấu cách (1 byte→1 byte), `parse_voi` span tuyệt đối, hạ **cả pipeline** bằng Lowerer con chung bảng (có `recs`/`file_long` của cha) ⇒ `( ls ./photos --long )` 6 cột, `( ls | where )` chạy, nhóm lồng chạy |
| 3 | **Kiểu tĩnh** | `kieu_stage` nhóm: biểu thức script phẳng hoá → kiểu stage **đầu** của pipeline ⇒ `( ls --long ) \| select mode` kiểm tĩnh; `( ls ) \| where` đúng |
| 4 | **Sugar** | `where` kết thúc vùng văn bản theo **span stage** (trong nhóm không nuốt `)`) |
| 5 | **Đo** | **666 → 680** test (+14 `nhom.rs`); 3 fixture `test/cmd/nhom/**`; interp ↔ `--vm` cùng byte; diff 100k 0 lệch; allowlist **6** |

 (2026-11-13)

| # | Việc | Kết quả |
|---|---|---|
| 1 | **Văn phạm** | thân `cmd` = **script** (ST-44 giữ nguyên) **hoặc đúng một pipeline lệnh**; parser thử Script trước, hỏng ⇒ thân lệnh (`CmdKhai` dừng sau `:`, dòng thân do parser lệnh parse) |
| 2 | **Hạ** | tiền tố parse bằng **filler** (`()` giữ nguyên byte ⇒ offset tuyệt đối); thân lệnh parse `dedent` **dịch span tuyệt đối**, hạ bằng Lowerer **con** chung bảng ⇒ `HItem::Fn.body = [giá trị cuối pipeline]` |
| 3 | **Lỗi rõ** | thân rỗng / lẫn script+lệnh (`let`+lệnh, lệnh+biểu thức) / >1 pipeline ⇒ **`VS0201`**; thân script cũ **không đổi** |
| 4 | **Tách R1** | `lower/than.rs` (`ha_than_cmd`, `lower_nhom_lenh`, `lower_than`, `ThanCtx`, `is_builtin_value`); `lower.rs` 1.4xx, `cmd.rs` 1.4xx |
| 5 | **Đo** | **651 → 666** test (+15 `vish_cmd_body.rs`); 3 fixture `test/cmd/body/**`; interp ↔ `--vm` **cùng byte**; diff 100k 0 lệch; allowlist **6** |

 (2026-11-13)

| # | Việc | Kết quả |
|---|---|---|
| 1 | **Report** | `agent/reports/R7-viscript.{md,json,json.license}`; `round = 7`, `head = 426eb91`, đúng **11 khoá**; `report-check --all` ⇒ **PASS** (7 báo cáo, **53** hàng QUEUE) |
| 2 | **Số đo** | **651** test (21+96+18+143+24+50+9+**290**); sem **370/18** (327 `.vis` + 43 `.vish`, 52 `.err`); `vis test` **370 ok**, `--vm` **370 ok/6 SKIP**; parity **344/344**; diff **100k / 0 lệch** (212,2 s); fuzz **3.003.112 ca / 0 vi phạm**; `VBC1` **53 tag**; **7 sổ / 29 mã**; khởi động **11,1–11,3 ms**; allowlist **6** |
| 3 | **Nội dung** | bảng vòng 7 (ST-44..ST-47) + **§19 VSH1b** (nêu rõ **chưa hoàn thiện toàn bộ ViScript** + đường còn lại: VSH1b-2, nợ lõi, VS2) + **14 nợ còn mở** + VD-77..80 + đề xuất (AGENTS §8, `viscript/AGENTS.md`, QUEUE vòng 8) |

 (2026-11-13)

| # | Việc | Kết quả |
|---|---|---|
| 1 | **`ls --long` thật** | `MucFs.mode: u32`; **`FileEntryLong`** (6 trường: thêm `mode`, `modified_time`); `mode` = `rw-r--r--`, `modified_time` = **UTC** `YYYY-MM-DD HH:MM` (civil-from-days, không thêm crate); bảng mặc định **4 cột không đổi** |
| 2 | **`( … )` đối số** | bỏ `VS0603`; nhóm-đối-số = **biểu thức** (`take ( 1 + 1 )`, `echo ( "a" )`), sai kiểu tĩnh ⇒ `VS0412`; nhóm rỗng = lời gọi 0 đối số; **stage** nhóm vẫn chạy (biểu thức **và** một lệnh `( ls ./photos )`) |
| 3 | **Chẩn đoán tuyệt đối** | `lower_vach` đệm **dấu cách** theo offset byte (không `\n` ⇒ không `Indent`/`Dedent` giả): `( 1 / 0 )` ở dòng 3 ⇒ `3:3` |
| 4 | **Đo** | **636 → 651** test (+15 `vsh1b.rs`); sem **370** không đổi; **4 fixture** `test/cmd/long/**`; parity 344/344; diff 100k 0 lệch; allowlist **6** |
| 5 | **Nợ** | `( ls … --long )` (nhóm-lệnh có cờ) chưa hạ; nhóm **nhiều dòng** chưa hỗ trợ; `where (x=>x)` kiểm **động** (chưa mở `kiem_builtin` cho `where`) |

, `each` không kiểm kiểu trả (2026-11-13)

| # | Việc | Kết quả |
|---|---|---|
| 1 | **Đo trước** | `where size`/`it.size`/`1`/`x => x` **đã** `VS0412` (từ ST-38, qua `coerce` thân lambda); `.vish` **có** tyck |
| 2 | **Nới `each`** | khai kiểu trả lambda = **`{error}` (any)** ⇒ thân `int`/bản ghi **sạch**; không nuốt lỗi trường (`VS0403`/`VS0605`) |
| 3 | **Ca bắt buộc** | `.vish` khai báo + `cmd` trả `list[FileEntry]` + `where size` ⇒ **`VS0412`** |
| 4 | **Đo** | **622 → 636** test (+14 `where_tinh.rs`); sem **370** không đổi; **3 fixture** `test/cmd/where/**`; allowlist **6** |
| 5 | **Tách tệp** | `lower/sugar.rs` (271 dòng) — `ha_sugar`/`ha_closure`/`arg_start`/`stage_end`/`kiem_it` rời `cmd.rs` (1.509 → **1.252** dòng) để giữ **R1** |

 (2026-11-13)

| # | Việc | Kết quả |
|---|---|---|
| 1 | **Tách vùng** | dòng **cột 1** mở đầu bằng `type/enum/cmd/fn/use/const/trait/impl/alias/newtype/extern` (hoặc `let` **có kiểu**, RHS không `|`/`;`/`>`/`<`/`&`) ⇒ **vùng khai báo** ở đầu tệp; hết vùng ⇒ stage; dòng thụt thuộc thân item |
| 2 | **Hạ** | tiền tố parse lại `Mode::Script` (span **tuyệt đối**), `lower_into` vào **cùng** bảng ký hiệu/kiểu ⇒ `cmd` đăng ký `CmdSig` cho khe ống; `run_shell_tu(mốc)` **bỏ** các dòng đã hạ |
| 3 | **Lỗi** | trùng tên (kể cả prelude `FileEntry`/`ProcEntry`) ⇒ **`VS0302`**; khai báo **sau** stage ⇒ **`VS0201`** kèm gợi ý; `( )` rỗng đứng riêng ⇒ `VS0201`; `f()` ⇒ **lời gọi 0 đối số** |
| 4 | **Không phá** | `.vish` **không** khai báo giữ nguyên vũng 6 (4 fixture `test/cmd/fs/**` xanh); `.vis` **không** đổi; parser Command chỉ thêm **khối raw** `CmdKhai`; `let x: int = ls | grep foo` **vẫn là shell-let** (golden ca 24) |
| 5 | **Đo** | **622** test (606 + **16** `vish_decl.rs`); sem **370** không đổi; **4 fixture** `test/cmd/decl/**`; nợ: **thân `cmd` pipeline** (script body chưa pipeline) ⇒ **VSH1b-2** |

 (2026-11-13)

| # | Việc | Kết quả |
|---|---|---|
| 1 | **Report** | `agent/reports/R6-viscript.{md,json,json.license}`; `round = 6`, `head = 654accb`, đúng **11 khoá**; `python3 test/report/check_report.py --all` → **PASS** (49 hàng QUEUE phủ đủ) |
| 2 | **Số đo** | **606** test (21+96+18+143+24+50+9+**245**); sem **370/18** (327 `.vis` + 43 `.vish`, 52 `.err`); `vis test` **370 ok**, `--vm` **370 ok/6 SKIP**; parity **344/344**; diff **100k / 0 lệch** (208,0 s); fuzz **2.853.740 ca / 0 vi phạm**; `VBC1` **53 tag**; **7 sổ / 29 mã**; khởi động ấm **11,4–11,6 ms** (+prelude **+0,08 ms**); allowlist **8 → 6** |
| 3 | **Nội dung** | bảng vòng 6 (ST-37..ST-43) + §19 (VSH1a đạt tới đâu) + **17 lỗ hổng còn mở** + VD-69..76 mỗi cái một dòng + đề xuất (AGENTS §8, QUEUE, BLOCKED) |
| 4 | **Sửa kèm lý do** | `sem.rs` sửa **chuỗi** "336 `.vis`" → **327 `.vis`** (chỉ là thông điệp; tổng 370 không đổi) |

 + `--fs-mock` (2026-11-13)

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
