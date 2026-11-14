<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# R10-viscript — báo cáo vòng 10 (nợ lõi A/B/C + VS2 #1 async + VS2 #2 kênh), dòng ViScript

Agent 2 · **2026-11-14** · `head` = **`6192bbd`** (commit nền: `QUEUE@head` đủ **85 hàng**, 6 cột;
ST-56..ST-61 `DONE`, ST-62 `READY` — báo cáo này). Số đo chạy trên **mã `782f1b9`** (các commit sau
chỉ sửa `QUEUE.md`/`DECISIONS.md`). Bản máy đọc: [`R10-viscript.json`](R10-viscript.json) — **đúng
11 khoá** Khối E (`project = "viscript"`, `round = 10`). Giống tiền lệ R9: `head` = commit **nền** của
báo cáo (commit `[ST-62]` là commit kế tiếp; không thể tự chứa hash của chính nó).

## 1. Tóm tắt (5 dòng)

1. **Vòng 10 đóng 5 nợ lõi** bằng 5 task nhỏ (ST-56/56b/57/58/59): khoá `str` cho `std.list.sort_by`,
   `&self`/`ref` identity, mồ côi **đa tệp**, **siêu trait** một tầng, `derives` **sinh mã**, `Ty::Any`
   **xoá hẳn** bằng biến kiểu, một luật `let`, biến làm **stage đầu**, thân `cmd` khai báo + pipeline.
2. **VS2 #1 — async CÓ MẶT**: `async fn`/`await`/`spawn` + scheduler **một luồng, tất định** (builtin
   `$tac_vu/$spawn/$await`, **không** thêm tag `VBC1`); join cuối theo thứ tự tạo; lỗi thân ⇒ `VS0506`;
   10.000 task **319/168 ms**, hai đường **cùng byte**, không tràn. **Chưa** song song thật.
3. **VS2 #2 — kênh CÓ MẶT**: `chan[T](n)`, `send`/`recv`/`close`; deadlock `VS0505` **không treo**
   (~0,09 s), `send` sau `close` ⇒ `VS0504`, sai kiểu `VS0412`; 15 ca sem + 5 test Rust. **Chưa** `select`.
4. **Số đo**: **787 test**, sem **441/21 nhóm**, `vis test` **441 ok** (`--vm` 441 ok/7 SKIP), parity
   **395/395**, differential **100.000 / 0 lệch** (208,5 s, seed 3523158054), fuzz parser 60 s
   **2.661.866 ca / 0 vi phạm**, `VBC1` **53** tag, **7 sổ / 32 mã**, allowlist **6**.
5. **ViScript chưa hoàn thiện toàn bộ**: VS2 còn **6 hạng mục chưa có** (net/http, quyền+Landlock, LSP,
   gói/registry, windows/wasm, hợp chuẩn 4 nền) — kế hoạch vòng 11 **đã có** trong QUEUE (ST-63..ST-76).

## 2. Bảng vòng 10

| Task | Commit | Kết quả (1 dòng) |
|---|---|---|
| ST-56 | `767b195` | nợ lõi A: khoá `str` cho `std.list.sort_by` (ổn định), `ref(x)` = identity hai đường, `&self` hạ như `self`, `Hop[Hop[int]]` **đã xanh từ trước** (khoá 5 biến thể); **747 test**, sem 376/18, parity 348/348, diff 100k/0 lệch (202,7 s) |
| ST-56b | `dd04afb` | một ngữ nghĩa **phép gán**: `self.x = v` bất biến ⇒ `VS0412` **cả hai đường**; `var self`/`var xs` gán được; codegen `x[i] = v` sửa `Set`+`Assign` (trước **im lặng** mất kết quả); `--vm` lỗi codegen exit ≠ 0; **756 test**, sem 382/18, parity 352/352 |
| ST-57 | `57ea7212` | nợ lõi B: **mồ côi đa tệp**, **siêu trait** một tầng (`VS0403`), `derives` **sinh mã** (Show/Eq, lượt 1c); **765 test**, sem 388/19, parity 356/356, diff 100k/0 lệch (198,5 s) |
| ST-58 | `abdc253` | nợ lõi C: **`Ty::Any` xoá hẳn** → `Var/List/Map` const-friendly; `kiem_std` hợp nhất (**16/38 chữ ký**); sai kiểu ⇒ `VS04xx` **tĩnh**; **770 test**, sem 388/19, parity 356/356 |
| ST-59 | `3e2a3cd` | nợ shell: **một nguồn luật `let`**; **biến làm stage đầu**; thân `cmd` khai báo + **một** pipeline (khai báo sau ⇒ `VS0201`); **771 test**, sem 394/19, parity 360/360 |
| ST-60 | `2542421` | **VS2 #1 async**: `async fn`/`await`/`spawn` + scheduler **FIFO một luồng** (`rt.rs` dùng chung hai đường, **không** thêm tag `VBC1`); join cuối theo thứ tự spawn; `VS0506`; 10k task **309/166 ms** lúc đóng; **774 test**, sem 416/20, parity 377/377 |
| ST-60b | `f158085` | hotfix review: closure bắt giá trị có **ô cục bộ riêng** (hết lệch parity), `()` = `PushUnit` thật, tyck **gốc đích gán** (`x.f[i]`), `.vish` shell-let, `std.*` từ chối `--tên` (`VS0401`); **782 test**, sem 419/20, parity 378/378 |
| ST-60c | `b254f03` | derives vào `impl_pairs` (hết `VS0403` oan), `enum derives Show` **sinh mã thật**, `Eq` chặn **sâu** list/map/opt; tách `tyck/sig.rs` + `bytecode/cerr.rs` (R1); **782 test**, sem 426/20, parity 381/381 |
| ST-61 | `782f1b9` | **VS2 #2 kênh**: `chan[T](n)`, `send/recv/close`; deadlock `VS0505` **không treo** (~0,09 s), `VS0504` gửi sau close, `recv` sau close trả nốt rồi `none`, sai kiểu `VS0412`; tách `tyck/expr/{kenh,phu}.rs` (R1); 15 ca sem + 5 test Rust; **787 test**, sem 441/21, parity 395/395, diff 100k/0 lệch (213,4 s) |
| ST-62 | (commit này) | báo cáo **R10**; `report-check --all` PASS; số liệu tái lập được (bảng §3) |

## 3. Số liệu chạy lại (mã `782f1b9`, đo 2026-11-14, trên macOS aarch64, `debug`)

| Phép đo | Kết quả | Lệnh |
|---|---|---|
| `cargo test --workspace` | **787** = 21 diag + 96 syntax + 18 fmt + 154 types + 24 interp + 62 bytecode + 9 vm + **403 cli** | `cd viscript && cargo test --workspace` |
| Ca ngữ nghĩa | **441 / 21 nhóm** (391 `.vis` + 50 `.vish`; **80** `.err`) | `ls test/sem/**` + `tests/sem.rs` (khoá 441/21) |
| `vis test` / `--vm` | **441 ok**, 0 `not ok` / **441 ok, 7 SKIP, 0 đỏ** | `vis test test/sem` / `vis test --vm test/sem` |
| Parity interp ↔ VM | **395/395** ca chạy được khớp byte + **46** ca lỗi biên dịch (tổng 441) | `cargo test -p vis-vm --test vs_interp` |
| `diag_cmp` ba tầng | **361 xanh / 46 lỗi biên dịch / 34 lỗi lúc chạy** (=441) | `cargo test -p vis-cli --test diag_cmp` |
| Differential | **`tong=100000 loi_tinh=0 chay=100000 lech=0 giay=208.5 toc_do=480/s seed=3523158054`** | `./bin/vis-diff` |
| Fuzz parser 60 s | **2.661.866 ca, 60.522.399 token, 44.364 ca/s, 1.008.706 token/s, 60,0 s, vi phạm 0** (PASS) | `cargo build --release -p vis-syntax --bin vis-parse-fuzz` + `target/release/vis-parse-fuzz --seconds 60 --seed 20261114` |
| `VBC1` | **53 tag** (không bump ở ST-56..ST-61) | đếm biến thể `Instr` trong `crates/vis-bytecode/src/format.rs` |
| Sổ mã | **7 sổ / 32 mã** (`VS0504`/`VS0505` mới ở ST-61; sổ `05xx` = 6 mã) | `vis explain --list` |
| Allowlist doc-examples | **6** (18 khối xanh + 6 ngoại lệ đỏ đúng) | `wc -l docs/viscript/doc-examples-allow.txt` (6 mục + 4 dòng chú thích) |
| 10k task async | **interp 319 ms, vm 168 ms**, 10.001 dòng stdout, hai đường **cùng byte** | `cargo test -p vis-cli --test async_10k -- --nocapture` |
| Cổng | `gate --fast` **PASS** (không retry); `fmt`/`clippy -D warnings`/R1..R4 OK, doc-examples 24 khối, report-check PASS | `./vis-dev gate --fast` |

**Tái lập**: mọi con số trên đều do lệnh ở cột 3 sinh; không có số viết tay. Thứ tự đo:
`cargo test --workspace` → `vis test`/`--vm` → `async_10k` → differential → `gate --fast` → fuzz 60 s.

## 4. §19 — đối chiếu **VS2**: đã có mặt / chưa có + kế hoạch vòng 11

| # | Hạng mục VS2 | Trạng thái sau vòng 10 | Bằng chứng / tiêu chí đo | Kế hoạch |
|---|---|---|---|---|
| 1 | **Async có cấu trúc** | **CÓ MẶT MỘT PHẦN** | `async fn`/`await`/`spawn`, scheduler **một luồng tất định**, join cuối theo thứ tự tạo, lỗi thân `VS0506`; 10k task **319/168 ms**, hai đường cùng byte. **Chưa** nhường giữa thân ⇒ **chưa song song thật**, không GC | ST-60 (DONE); nợ ghi §5 |
| 2 | **Kênh** | **CÓ MẶT MỘT PHẦN** | `chan[T](n)`/`send`/`recv`/`close`; deadlock `VS0505` **0,085–0,104 s** (không treo); `VS0504`; 15 ca sem + 5 test Rust; hai đường **cùng byte**. **Chưa** `select`/timeout; drain chỉ task `ready` | ST-61 (DONE); nợ ghi §5 |
| 3 | **Net/HTTP** | **CHƯA CÓ** | Chưa có `NetHost`, chưa có `std.net`/`http.get` | Vòng 11: **ST-63..ST-66** (VD-100/VD-105), tiêu chí ≥13 ca e2e loopback |
| 4 | **Quyền + Landlock thật** | **CHƯA CÓ** | `@ path`/`--explain-perms` **chỉ in**, chưa cưỡng chế; chưa Landlock | Vòng 11: **ST-72/ST-73** (VD-103); macOS ghi rõ **chưa đo** |
| 5 | **LSP** | **CHƯA CÓ** | Chưa có `vis lsp` | Vòng 11: **ST-67..ST-69** (VD-102), ≥13 frame snapshot |
| 6 | **Gói + registry** | **CHƯA CÓ** | Chưa có `vis pkg`, manifest/lock/SHA-256 | Vòng 11: **ST-70/ST-71** (VD-101), registry **local** |
| 7 | **windows/wasm** | **CHƯA CÓ** | Chỉ host posix; chưa `cargo check` chéo hai target | Vòng 11: **ST-74** (VD-104), chỉ mức **check** |
| 8 | **Hợp chuẩn Host 4 nền** | **CHƯA CÓ** | Chưa có suite `host-conformance` | Vòng 11: **ST-75** (VD-104), ≥50 ca **trên macOS**, 3 nền ghi "chưa đo" |

**Kế hoạch vòng 11 đã có trong QUEUE** (ST-63..ST-76, 14 hàng `READY`, phụ thuộc tuần tự):
ST-63 `NetHost`+TCP client → ST-64 `http.get` → ST-65 server loopback → ST-66 biên net/http; ST-67 LSP khung
→ ST-68 diagnostics UTF-16 → ST-69 completion/hover; ST-70 `vis pkg` → ST-71 `use pkg.*`; ST-72 cưỡng chế
quyền → ST-73 Landlock; ST-74 cross-check windows/wasm; ST-75 `host-conformance`; ST-76 báo cáo **R11**.

## 5. Lỗ hổng còn mở (nợ có chủ đích — gộp từ ST-56..ST-61)

**Từ nợ lõi (ST-56/56b/57/58/59)**

1. **Shell-let chưa hạ**: `let out = ls -l` qua `check` nhưng `run` báo `out` chưa bind (ST-60b ghi).
2. **`match` không khớp nhánh ở ngữ cảnh giá trị** vẫn đẩy `false` thay vì `VS0501` — nghi **lệch
   interp ↔ VM** (ST-60b; chưa có ca đỏ vì chưa dựng được chương trình tối thiểu — ghi để mở).
3. Gán **lồng** sâu hơn một tầng trường (`x.f[i][j]`) là `VS0412` **có chủ đích** (ST-56b).
4. **Thân `impl` chưa được tyck đầy đủ** — mới kiểm **đích gán** (ST-56b).
5. **Siêu trait bắc cầu** (mới kiểm **một tầng**) (ST-57/ST-60c).
6. **`impl_pairs` per-tệp**: `impl` sinh từ `derives` ở **tệp khác** không thấy; dispatch `impl` xuyên tệp
   vẫn per-tệp (ST-57/ST-60c).
7. **`co_float_sau` không soi bản ghi lồng**: `Eq` derives với bản ghi chứa `float` lồng chưa chẩn đoán
   (ST-60c).
8. **`derives Json` chưa sinh mã** (chỉ kiểm tĩnh) (ST-57).
9. **`Ty::Fn` thật** cho `sort_by` (đang `Ty::FnKhoa` là ngoại lệ có ghi); **map khoá không-`str`**;
   **thứ tự khoá map** (ST-58).
10. **Nhóm nhiều pipeline** + kiểu tĩnh **lấy stage đầu** còn cắt; **lambda `where` không chú thích** ⇒
    `{error}` (kiểm động) (ST-59).
11. **`lower/cmd.rs` 1.491 dòng** — sát ngưỡng R1, cần tách trước khi thêm tính năng (ST-59).

**Từ async (ST-60/60b/60c)**

12. **Chưa song song thật**: scheduler **run-to-completion**, không nhường giữa thân, **không GC**
    (chu trình `Rc`); `await` lồng sát ngưỡng khung 256 (ST-60, VD-90/VD-95).
13. **`async` trên method/`cmd`** ⇒ parser ra **`VS0201`** (không phải `VS0412` như VD-95; đổi kỳ vọng khi
    parser mở hai dạng này); **`async fn` không nhận `--tên`** (ST-60).
14. **`p.x += 1` là lỗi cú pháp `VS0201`** (front end chặn trước tyck — giữ nguyên) (ST-56b).

**Từ kênh (ST-61)**

15. **Drain chỉ chạy task ở hàng `ready` (`spawn`)**: task `Moi` do **gọi** `async fn` (chưa spawn/await)
    **không** được chạy ⇒ `recv` có thể `VS0505` dù còn Task.
16. **`close` idempotent nhưng không đánh thức ai** (không có wait-set — cùng mô hình run-to-completion).
17. **Không có `select`/timeout** trên kênh.
18. **`Chan` không truyền qua biên host** (chỉ sống trong interp/VM một tiến trình).
19. **`cap = 0` thực tế không dùng được**: cả hai phía chặn đều drain rồi `VS0505`.
20. Dung lượng **âm** là lỗi **lúc chạy** `VS0501` (tyck chỉ đòi `int`); `Chan` in `<kênh>` (không có
    `show` cấu trúc).

## 6. VD-89..VD-100 (mỗi quyết định một dòng)

- **VD-89** — Phạm vi vòng 10: 6 task ST-56..ST-61 + ST-62 **tuần tự**, red-first/commit riêng/push sau
  cổng; **không** nới ngưỡng, **không** tăng allowlist; async/kênh **cùng byte** interp ↔ `--vm`.
- **VD-90** — Async: `async fn` gọi **không** chạy thân (trả `Task[T]`), `spawn` đưa vào hàng FIFO, `await`
  chạy tới hoàn tất + cache lần hai; join cuối theo **thứ tự tạo**; lỗi thân ⇒ `VS0506`; 10k task hàng
  **phẳng**; **nợ** run-to-completion (chưa song song thật).
- **VD-91** — Kênh: `chan[T](n)`, `n ≥ 0`; `send`/`recv() -> T?`/`close`; chặn ⇒ **drain FIFO** rồi
  `VS0505` (không treo); `send` sau close `VS0504`; `recv` sau close trả nốt rồi `none`; tyck khớp `T`.
- **VD-92** — `&self`/`&expr` **không mượn thật**: `&self` = `self`, `&expr` = identity giữ nguyên kiểu;
  codegen hết `Unsupported`; mượn/vòng đời là nợ VS2+.
- **VD-93** — `derives` sinh **HIR ở `lower`** (Show/Eq), thứ tự trường, tất định; trường không derive
  được (`float`+Eq) ⇒ `VS04xx` **tĩnh**; người dùng viết tay **thắng**.
- **VD-94** — Bỏ `Ty::Any` bằng **biến kiểu** trong chữ ký `std.*` (`T/K/V`); tyck **hợp nhất** đối số,
  sai ⇒ `VS0401`; ngoại lệ có ghi; `all_codes()` không đổi (trừ mã ST-60/61).
- **VD-95** — Hiện thực async bằng **builtin nội bộ** `$tac_vu/$spawn/$await` + `rt.rs` chung; **không**
  thêm tag `VBC1`; `Task[T]`/`Chan[T]` = `TypeRef`; `spawn` chỉ **tên** hàm async cấp tệp, đối số vị trí;
  lỗi bọc `VS0506`; `MAX_TASK_NEST = 256`.
- **VD-96** — Hiện thực kênh: `$kenh` + **một** nhánh method `send/recv/close` cho cả hai đường; drain
  FIFO; **`VS0504`/`VS0505`** thêm sổ `05xx` (6 mã), `all_codes()` **29→32**, sửa **đủ 6 chỗ** khoá số ca;
  byte giống nhau nhờ cùng `rt.rs`/span nút gọi.
- **VD-97** — **Một ngữ nghĩa phép gán**: `self` bất biến ⇒ `VS0412` **cả hai đường**; `var self`/`var x`
  gán được; `x.f = v` = rebind giá trị, `x[i] = v` = sửa tại chỗ; interp = VM; `--vm` lỗi codegen ⇒ exit ≠ 0.
- **VD-98** — **Mồ côi đa tệp** (trait/kiểu khai trong chương trình ⇒ hợp lệ; mồ côi thật ⇒ `VS0403`) +
  **siêu trait** một tầng (`VS0403`) + **derives sinh mã** ở lượt 1c (`show` = `HExpr::Interp`, `eq` = `==`;
  enum show **cắt** — nợ; enum eq làm được); nhóm `trait/` ⇒ 19 nhóm.
- **VD-99** — `Ty` const-friendly giữ `Copy`: thêm `Var(u8)`/`List`/`Map`, **xoá hẳn** `Any` và `kieu()`;
  `sang_type()` **tổng**; `kiem_std` hợp nhất `hop_nhat`+`thay_gen`; **16/38** chữ ký đổi; ngoại lệ duy
  nhất `list.sort_by` = `Ty::FnKhoa`.
- **VD-100** — **Net/HTTP (vòng 11)**: `NetHost` (socket thô) ở host, **giao thức HTTP trong `vis-interp`**
  ⇒ hai đường cùng byte; chỉ **loopback**, timeout ≤ 400 ms; **không mã mới** (lỗi mạng = `Err`, không
  host = `VS0604`); R1 tách trước khi thêm; ≥13 ca e2e — nay là ST-63..ST-66.

## 7. Ghi chú máy kiểm + đề xuất

- `python3 test/report/check_report.py --all`: **PASS** — **10 báo cáo**, **85 hàng QUEUE** phủ đủ;
  `R10-viscript.json` đúng **11 khoá**, `round=10`, `head=6192bbd` (tồn tại + là tổ tiên HEAD).
- Cổng: `./vis-dev gate --fast` **PASS**, không retry; `fmt`/`clippy -D warnings`/R1..R4 OK; doc-examples
  **24 khối = 18 xanh + 6 ngoại lệ**; report-check trong cổng PASS. Differential **100k 0 lệch**.
- Không thêm `#[ignore]`, không nới ngưỡng, không sửa golden/allowlist; `VBC1` giữ **53** tag; sổ mã
  **32** (chỉ thêm `VS0504`/`VS0505` đúng VD-96).
- **Đề xuất Agent 0**: (1) lật `ST-62 = DONE`; (2) **mở ST-63** (NetHost) theo đúng thứ tự vòng 11;
  (3) khi mở parser cho `async` trên method/`cmd`, đổi kỳ vọng `async_vis.rs` từ `VS0201` → `VS0412`
  (đã ghi ở §5 mục 13); (4) giữ nợ §5 trong `R11`/`QUEUE` khi mở từng task vòng 11.
