<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# R13-viscript — báo cáo vòng 13 (đóng nợ nhỏ + kiểm sâu: `assert`/`fs.exists`, LSP module-aware, thụt lề `.vish`, fuzz cú pháp mới, cache không che lỗi), dòng ViScript

Agent 2 · **2026-11-14** · `head` = **`5ee772f7`** (commit nền: `QUEUE@head` đủ **100 hàng**, 99 `DONE` +
ST-88 `READY` — báo cáo này). Số đo chạy trên **mã `32e9bebd`** (commit cuối vòng 13; sau đó chỉ có
`QUEUE.md`). Bản máy đọc: [`R13-viscript.json`](R13-viscript.json) — **đúng 11 khoá** Khối E
(`project = "viscript"`, `round = 13`). Như tiền lệ R11/R12: `head` = commit **nền** của báo cáo (không thể
tự chứa hash của chính nó), nên `head` **không** phải `32e9bebd`.

## 1. Tóm tắt (5 dòng)

1. **Vòng 13 “đóng nợ nhỏ + kiểm sâu”** đóng 7 việc: `fs.exists` ngoài quyền **trap `VS0604`**, `assert` câu
   lệnh, `explain` trợ giúp 3 ngôn ngữ (ST-83); LSP completion/hover **module-aware** (ST-84); đo thụt lề
   `.vish` ⇒ **có ngữ nghĩa ở cấp tệp** nên giữ nguyên, chỉ đếm/cảnh báo (ST-85); dòng cấp tệp thụt lề báo
   **`VS0201`** thay vì bỏ im lặng (ST-85b); fuzz differential **8 nhóm cú pháp mới** (ST-86); **cache không
   che lỗi** 520 tệp × 4 lần (ST-87); **bỏ fast-path cache chết** (ST-87b).
2. **Bug thật tìm được**: (a) `.vish` dòng cấp tệp thụt lề bị **bỏ im lặng** (tệp thụt hết ⇒ in rỗng exit 0)
   — sửa bằng `VS0201` có căn cứ §9.5; (b) **fast-path cache một tệp là dead code** (đọc khoá v1
   `cache::khoa`, mọi chỗ ghi dùng v2 `cache::khoa_multi`) — đo **0 hit**, đã **xoá** cả nhánh lẫn hàm.
3. **Kiểm sâu**: LSP completion/hover thấy item `pub` của `use ./x` + hàm `std.*` đã `use` (detail nêu
   nguồn, khử trùng ưu tiên tệp chính); `fmt --indent-report` đếm **8 tệp/10 dòng** lệch nhưng **không**
   sửa (0 fixture đổi); fuzz 20k/100k **0 lệch**; cache **520 tệp × 4 lần 0 lệch**, mode/module/strict-caps
   đúng khoá.
4. **Số đo**: **961 test**, sem **447/22 nhóm**, `vis test` **447 ok** (`--vm` **447 ok + 7 SKIP**), parity
   **398/398**, differential **100.000 / 0 lệch** (152,9 s, seed 20261115 — bộ sinh mới), fuzz parser 60 s
   **2.687.590 ca / 0 vi phạm**, host **54/54**, targets **18/18 `check`**, `fmt --check test/` **497 tệp**,
   cache harness **520 tệp × 4 lần 0 lệch**. ViScript **chưa hoàn thiện toàn bộ** — nợ ở §5, kết luận §8.
5. **Trung thực**: không nới ngưỡng, không `#[ignore]`, không sửa golden để qua; các ca `.vish` dùng
   chương trình ngoài vẫn **khác hai đường có chủ đích** (VM chưa có host `Ngoai`, `VS0604`) — đã tách
   khỏi so sánh byte và ghi nợ 56.

## 2. Bảng vòng 13

| Task | Commit | Kết quả (1 dòng, số đo tại lúc đóng) |
|---|---|---|
| ST-83 | `7bac3c70` | **`fs.exists` ngoài quyền trap `VS0604`** (thay vì `false` im lặng), `assert` câu lệnh (`assert đk`) hạ thành builtin, `vis explain` trợ giúp **3 ngôn ngữ** (`BLOCKS_RESERVED`/`all_codes` đếm động); **920 test**; diff 100k/0 lệch (231,4 s) |
| ST-84 | `2d3032b7` | **LSP completion/hover module-aware**: tệp có trên đĩa ⇒ item `pub` của `use ./x` + hàm `std.<m>` đã `use`, detail `util: fn tinh(x: int) -> int`, khử trùng tệp chính > module > builtin > từ khoá; 5 ca + 1 unit hiệu năng (nạp 5 module < 100 ms); **926 test**; diff 100k/0 lệch (219,9 s) |
| ST-85 | `12c0231c` | **Đo thụt lề `.vish` (6 phép)**: thân `cmd` 2/4 không đổi nghĩa, TAB `VS0107`; **cấp tệp thụt lề ĐỔI hành vi** ⇒ **nhánh B**: không chuẩn hoá (**0 fixture đổi**), thêm `fmt --indent-report` (**8 tệp/10 dòng** lệch) + **cảnh báo** `--check` không đỏ; 6 ca; **932 test**; diff 100k/0 lệch (212,2 s) |
| ST-85b | `9dacea49` | **Dòng cấp tệp thụt lề `.vish` ⇒ `VS0201`** “câu lệnh cấp tệp không được thụt lề (`.vish` không có block cấp tệp)”, hết **bỏ im lặng**; 7 ca interp+VM; 0 golden sửa; **939 test**; host 54/54; diff 100k/0 lệch (218,7 s) |
| ST-86 | `1d690869` | **Fuzz differential 8 nhóm cú pháp mới** (async/await/spawn; chan+send/recv/close; cap+`@`/`(write)`; derives Show/Eq; trait/impl siêu một tầng; assert; lỗi tĩnh/chạy): **20k seed 20261115 = 0 lệch** (31,8 s), **100k = 0 lệch** (158,5 s); 5 ca Rust + 7 ca CLI lỗi; **951 test** |
| ST-87 | `0c23dad3` | **Cache không che lỗi**: **520 tệp** (sem 447 + std 73; 83 `.err`) × **4 lần** (interp / `--vm` ×2 / `--no-cache`), **513 ca so đủ 4 đường 0 lệch** + 7 ca `.vish` ngoài (VM `VS0604` có chủ đích); mode/module/strict-caps đúng khoá; 24,9 s; 5 ca; **956 test** |
| ST-87b | `32e9bebd` | **Bỏ fast-path cache chết**: đọc khoá v1 `main.rs:655` vs ghi v2 `main.rs:767` ⇒ **0 hit**; xoá nhánh + `cache::khoa`, chỉ còn `khoa_multi`; đo lần 2 **~12 ms** không hồi quy; 5 ca; **961 test**; diff 100k/0 lệch (153,9 s) |
| ST-88 | (commit này) | báo cáo **R13**; `report-check --all` PASS; số liệu **chạy lại** trong §3 |

## 3. Số liệu chạy lại (mã `32e9bebd`, đo 2026-11-14, macOS aarch64, `debug` trừ khi ghi khác)

| Phép đo | Kết quả | Lệnh |
|---|---|---|
| `cargo test --workspace` | **961** = 21 diag + 96 syntax + **27 fmt** + 154 types + 26 interp + 62 bytecode + 9 vm + **566 cli** | `cd viscript && cargo test --workspace` |
| Ca ngữ nghĩa | **447 / 22 nhóm** (397 `.vis` + 50 `.vish`; **83** `.err`) — không đổi | `cargo test -p vis-cli --test sem` (khoá 447/22) |
| `vis test` / `--vm` | **447 ok**, 0 `not ok` / **447 ok, 7 SKIP, 0 đỏ** | `vis test test/sem` / `vis test --vm test/sem` |
| Parity interp ↔ VM | **398/398** ca chạy được khớp byte + **49** ca lỗi biên dịch (tổng 447) | `cargo test -p vis-vm --test vs_interp` |
| `diag_cmp` ba tầng | **364 xanh / 49 lỗi biên dịch / 34 lỗi lúc chạy** (=447) | `cargo test -p vis-cli --test diag_cmp` |
| Differential (bộ sinh mới) | **`tong=100000 loi_tinh=6230 chay=93770 lech=0 giay=152.9 toc_do=654/s seed=20261115`** | `VIS_DIFF_N=100000 VIS_DIFF_SEED=20261115 ./bin/vis-diff` |
| Fuzz differential 20k | **`tong=20000 loi_tinh=1247 chay=18753 lech=0 giay=31.8 toc_do=628/s seed=20261115`** | `VIS_DIFF_N=20000 VIS_DIFF_SEED=20261115 ./bin/vis-diff` |
| Fuzz parser 60 s | **2.687.590 ca, 61.141.145 token, 44.793 ca/s, 1.019.019 token/s, 60,0 s, vi phạm 0** (PASS) | `target/release/vis-parse-fuzz --seconds 60 --seed 20261115` |
| Cache harness | **520 tệp × 4 lần**; **513 ca so đủ 4 đường, 0 lệch**; 7 ca `.vish` ngoài (`VS0604` có chủ đích); **24,9 s** | `cargo test -p vis-cli --test cache_khong_che -- --nocapture` |
| `VBC1` | **53 tag** (không bump ở vòng 13) | `Instr::tag` trong `crates/vis-bytecode/src/format.rs` |
| Sổ mã | **7 sổ / 32 mã** (không mã mới ở vòng 13) | `vis explain --list` (32 dòng) |
| Allowlist doc-examples | **6** mục | `grep -v '^#' docs/viscript/doc-examples-allow.txt` (không sửa) |
| 10k task async | **interp 291 ms, vm 171 ms**, hai đường cùng byte, không tràn | `cargo test -p vis-cli --test async_10k -- --nocapture` |
| Bộ hợp chuẩn host | **54/54 ca xanh, 120 lần chạy, 0 lệch** | `./bin/vis-host-suite` |
| Ma trận target | **18/18 ô `PASS`** — **chỉ `cargo check` biên dịch chéo**, **chưa chạy** | `./bin/vis-targets` |
| `fmt --check test/` | **exit 0**, **497 tệp** kiểm (**429** `.vis` + **68** `.vish`), 136 input cố ý bỏ; **8 cảnh báo** thụt lề `.vish` (không đỏ) | `vis fmt --check test` |
| `--indent-report test` | **8 tệp / 10 dòng lệch** (chuẩn 4 × độ sâu khối) — **không sửa** | `vis fmt --indent-report test` |
| Báo cáo | **13 báo cáo**, **100 hàng QUEUE** phủ đủ; `R13` đúng 11 khoá, `round=13`, `head=5ee772f7` | `python3 viscript/test/report/check_report.py --all` |
| Cổng | `gate --fast` **PASS** (không retry); `fmt`/`clippy -D warnings`/R1..R4 OK; report-check PASS | `./vis-dev gate --fast` |

**Tái lập**: mọi số trên do cột 3 sinh; không có số viết tay. Thứ tự đo: `cargo test --workspace` →
`vis test`/`--vm` → parity/`diag_cmp` → `async_10k` → host suite → targets → `fmt --check` → indent-report
→ differential → fuzz parser → cache harness → gate/report-check.

## 4. §19 — đối chiếu **VS2** sau 13 vòng: mức thật

| # | Hạng mục VS2 | Trạng thái | Mức thật (đo được) | Còn thiếu |
|---|---|---|---|---|
| 1 | **Async có cấu trúc** | **CÓ MẶT MỘT PHẦN** | `async fn`/`await`/`spawn`, scheduler **một luồng tất định**, join cuối; **fuzz 20k/100k sinh async + spawn lồng, 0 lệch** (ST-86); 10k task **291/171 ms** | **Chưa song song thật** (run-to-completion), **không GC**; `async` trên method/`cmd` còn `VS0201`; `async fn` trong thân `test` không hỗ trợ |
| 2 | **Kênh** | **CÓ MẶT MỘT PHẦN** | `chan[T](n)`/`send`/`recv`/`close`; deadlock `VS0505` (không treo); **fuzz sinh cặp cân bằng + `close` + spawn sender, 0 lệch**; 520 tệp × 4 lần gồm 15 ca kênh, 0 lệch | **Chưa `select`/timeout**; `cap=0` chỉ deadlock có chẩn đoán; `Chan` không qua biên host |
| 3 | **Net/HTTP** | **CÓ MẶT MỘT PHẦN** | `NetHost` + TCP client/server loopback thật, `http.get`; **24 ca** net/http; **chỉ loopback** | **Chưa TLS/redirect/keep-alive/chunked**, **một luồng**, phục vụ **tuần tự**, cap body 8 MiB |
| 4 | **Quyền + Landlock thật** | **CÓ MẶT MỘT PHẦN** (runtime) — Landlock **CHỈ PROBE** | `cap[...]` là kiểu thật; cưỡng chế **ở biên host** (`FsGuard`/`NetGuard`), vi phạm ⇒ **trap `VS0604`** exit 2; **`fs.exists` ngoài quyền nay trap `VS0604`** (ST-83 — đóng nợ 44); `--strict-caps` (ST-81); 8+7 ca interp+VM | **Chưa siết mặc định**; **chưa cưỡng chế bằng kernel** ở **bất kỳ** nền nào; Linux **chưa chạy thật**; `--sandbox-check` **chỉ đọc** `/sys/.../lsm` |
| 5 | **LSP** | **CÓ MẶT MỘT PHẦN** | `vis lsp --stdio` (framing + JSON-RPC 2.0), diagnostics UTF-16; **chẩn đoán module-aware** (ST-80); **completion/hover module-aware** cho tệp trên đĩa — item `pub` của `use ./x` + `std.*` đã `use`, detail nêu nguồn, khử trùng (ST-84); **34 ca** | **Completion chưa theo ngữ cảnh sau `.`**; **hover chưa field**; chẩn đoán tệp phụ không publish; buffer chưa lưu chưa có module; full sync |
| 6 | **Gói + registry** | **CÓ MẶT MỘT PHẦN** | Registry **local** (`./registry`/`--registry`/`VIS_REGISTRY`), manifest `files[]`, lock SHA-256 tất định, `.vis/store` content-addressed, `use pkg.*` + `VS0701`/`VS0702` | **Chưa ký số**, **chưa registry HTTP**, **chưa resolver dải** (ghim chính xác), chưa `pkg remove/update`, `.vis/store` không GC |
| 7 | **windows/wasm** | **CHỈ CHECK** | Ma trận **18/18 ô PASS** bằng `cargo check --target`; `vis-host-wasm` khung; **`vis-cli`×wasm check xanh nhưng không chạy được** | **Chưa có runner** nền nào ngoài macOS; chưa đo `cargo build`/link Windows; `vis-host-linux` chưa nối vào `vis-cli` |
| 8 | **Hợp chuẩn Host 4 nền** | **CÓ MẶT MỘT PHẦN** (macOS) | **54 ca chạy thật trên macOS**, **120 lần chạy, 0 lệch**; **520 tệp cache × 4 lần 0 lệch** (ST-87); 22 FsHost · 13 Ngoai · 11 shell · 5 quyền · 3 determinism | **Windows/Linux/wasm: chưa đo**; net **không** nằm trong suite (24 ca riêng); **7 ca `.vish` Ngoai khác hai đường có chủ đích** (VM chưa host `Ngoai`) |

## 5. Lỗ hổng còn mở (nợ có chủ đích — R12 **44 mục** + nợ mới vòng 13)

**1..44** — giữ nguyên [`R12-viscript.md`](R12-viscript.md) §5, **trừ mục 44 đã đóng**: `fs.exists` ngoài quyền
nay **trap `VS0604`** (ST-83), không còn trả `false` im lặng. Các mục còn lại (assert/runner test/LSP/package/
Landlock/đa nền tảng/host) vẫn mở như R12.

**Nợ mới ST-83..ST-87b (đánh số tiếp):**

45. **`assert` câu lệnh chỉ một biểu thức, không message** (ST-83): `assert đk`; chưa `assert đk, "msg"`, chưa
    đếm/nhãn; mã lỗi dùng `VS0501` (không mã mới).
46. **Biến tên `assert`**: `assert` là từ khoá câu lệnh; dùng `assert` làm định danh (biến/hàm) bị nhập nhằng
    — chưa có luật đặt tên/chẩn đoán riêng.
47. **Trợ giúp `explain` dịch tay 3 ngôn ngữ** (ST-83): câu vi/en/ja viết trong `main.rs`; thêm mã mới phải
    sửa **3** chỗ; chưa có nguồn dữ liệu chung với `vis explain`.
48. **`is_pub` mới giữ cho `fn`** (ST-84): HIR `HRec/HEnum/HAlias/HConst/HTrait` **không** giữ cờ `pub` ⇒ LSP
    gợi ý cả item **không** `pub` của module; cần thêm cờ vào HIR (đổi API nội bộ).
49. **Completion chưa theo ngữ cảnh sau `.`** (ST-84): vẫn lọc **tiền tố** trên toàn bộ item; sau `x.` chưa
    gợi ý field/method theo kiểu.
50. **Hover chưa field** (ST-84): hover biến/hàm có; field bản ghi/chấm chưa.
51. **`--indent-report` fallback ước lượng** (ST-85): khi thân `cmd` chỉ là **lệnh** (script parse không qua),
    đếm theo dòng (`cmd`/`impl`/`type` mở khối) chứ không theo CST đầy đủ.
52. **`dong_thut_sai` heuristic văn bản** (ST-85b): dấu `:` cột 1 trong chuỗi/comment có thể bị coi mở khối;
    khối một dòng `cmd f(): x` không mở khối; nối dòng `\` ngoài ngoặc chưa hỗ trợ.
53. **`khai::tach` chưa dọn gốc** (ST-85b): vẫn coi dòng thụt là tiền tố khai báo; nay có `VS0201` chặn nhưng
    logic tách chưa sửa tận gốc.
54. **`topmix2` đổi hành vi** (ST-85b): trước đây dòng cấp tệp thứ hai thụt lề được **chạy như lệnh ngoài**;
    nay báo `VS0201` (đúng §9.5 nhưng là thay đổi hành vi với tệp sai thiết kế).
55. **Bộ sinh fuzz mẫu cố định** (ST-86): 8 nhóm là **mẫu cố định**, chưa ghép tổ hợp sâu; kênh chỉ cặp cân
    bằng; chưa sinh `use pkg.*` **thành công**; `@`/`(write)` mới ở mức cú pháp.
56. **7 ca `.vish` khác hai đường có chủ đích** (ST-87): chương trình ngoài (`echo`/`printf`/lệnh lạ) chạy ở
    interp nhưng VM báo `VS0604` vì **VM chưa có host `Ngoai`**; harness byte tách khỏi so interp.
57. **Frontend chạy mỗi lần** (ST-87b): sau khi bỏ fast-path, mọi lần `--vm` vẫn parse/lower/check (~1 ms tệp
    nhỏ) rồi mới tra cache codegen; không còn đường bỏ frontend.
58. **Ca chống nhánh chết đọc mã nguồn** (ST-87b): `khong_con_fast_path_khoa_v1` grep `main.rs`/`cache.rs`;
    brittle nếu tách hàm/đổi tên.

## 6. VD-109

**VD-109 — Đóng nợ nhỏ + kiểm sâu vòng 13**: (1) `fs.exists` ngoài quyền **trap `VS0604`** (đóng nợ 44), `assert`
câu lệnh hạ thành builtin, trợ giúp `explain` 3 ngôn ngữ đếm động; (2) LSP **completion/hover module-aware**
cho tệp có trên đĩa — item `pub` của module + `std.*` đã `use`, detail nêu nguồn, khử trùng **tệp chính >
module > builtin > từ khoá**, tệp không có trên đĩa giữ đường một tệp; (3) thụt lề `.vish` **có ngữ nghĩa ở
cấp tệp** (đo 6 phép) ⇒ **không chuẩn hoá**; `fmt --indent-report` chỉ **đếm**, `--check` **cảnh báo** không
đỏ; (4) dòng cấp tệp thụt lề **`VS0201`** (hết bỏ im lặng); (5) fuzz differential phủ **8 nhóm cú pháp mới**
kết thúc được; (6) cache **không che lỗi** (520 tệp × 4 lần byte y hệt), khoá cache gồm mode/nội dung đa tệp,
strict-caps kiểm runtime ⇒ dùng chung cache an toàn; (7) **bỏ fast-path cache chết** (khoá v1 không bao giờ
hit), chỉ còn `khoa_multi`.

## 7. Ghi chú máy kiểm + đề xuất

- `python3 test/report/check_report.py --all`: **PASS** — **13 báo cáo**, **100 hàng QUEUE** phủ đủ;
  `R13-viscript.json` đúng **11 khoá**, `round=13`, `head=5ee772f7` (tồn tại + là tổ tiên HEAD).
- Cổng: `./vis-dev gate --fast` **PASS**, không retry; `fmt`/`clippy -D warnings`/R1..R4 OK; report-check
  trong cổng PASS; differential 100k **0 lệch**; fuzz parser 60 s **0 vi phạm**; cache harness **0 lệch**.
- Không thêm `#[ignore]`, không nới ngưỡng, không sửa golden/allowlist; `VBC1` giữ **53** tag; sổ mã **32**
  (vòng 13 **không** thêm mã); không sửa `R1..R12`.
- **Đề xuất Agent 0**: (1) lật **ST-88 = DONE**; (2) **siết mặc định `cap`** (chuyển fixture + bật
  `--strict-caps` mặc định) — đây là nợ lớn nhất còn lại; (3) **host `Ngoai` cho VM** để đóng 7 ca khác hai
  đường; (4) **Landlock thật** chỉ khi có runner Linux + một trong hai điều kiện VD-107; (5) LSP sau `.` +
  hover field + đa tài liệu; (6) giữ **961 test**, **54 host**, **497 tệp fmt**, **520×4 cache** làm chuẩn R14.

## 8. Kết luận trung thực — còn gì để gọi là “hoàn thiện”

Sau 13 vòng, **công cụ và kiểm chứng** đã nhất quán: **961 test**, **447 sem**, parity **398/398**, differential
**100k 0 lệch** với bộ sinh phủ cú pháp mới, cache **520×4 0 lệch**, host **54/54**, fuzz parser **0 vi phạm** —
nhưng ViScript **chưa hoàn thiện**: (1) **Landlock thật** chưa có (chỉ probe, cần runner Linux + duyệt
crate/`unsafe`); (2) **runner Windows/wasm/Linux** chưa có — 18/18 ô chỉ là **`cargo check` biên dịch chéo**;
(3) net **chưa TLS/redirect**, một luồng, tuần tự; (4) async **chưa song song thật** (run-to-completion);
(5) **quyền chưa siết mặc định**; (6) **VM chưa có host `Ngoai`** (7 ca khác hai đường có chủ đích);
(7) kênh chưa `select`/timeout; (8) gói chưa resolver dải/ký số; LSP chưa sau `.`/field/đa tài liệu. Đây là
**nợ có chủ đích** đã đánh số ở §5 (45..58), không phải tuyên bố “đã xong”.
